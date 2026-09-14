//! Transport-independent strict decision execution with optional input enrichment.
//! Resource configuration is frozen with the host. Callers supply only raw events;
//! the embedding application owns authentication, cutoff selection and persistence.
use crate::{
    feature_pipeline::{FeatureEvidence, FeaturePipeline, FeaturePlan},
    CoreSource, DecisionEngine, DecisionRequest, DecisionResponse, EngineError, MetricsCollector,
    RuntimeDataSourceConfig, Schema, Value,
};
use corint_decision_runtime::datasource::DataSourceClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureDatasource {
    pub revision: String,
    pub config: RuntimeDataSourceConfig,
}

/// Operator-owned configuration; never include connection configuration in responses.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureHostConfig {
    pub plan: FeaturePlan,
    pub datasources: BTreeMap<String, FeatureDatasource>,
}

pub fn canonical_sha256(value: &impl Serialize) -> String {
    let mut value = serde_json::to_value(value).expect("serializable decision binding");
    value.sort_all_objects();
    format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()))
}

fn fail(code: &str, message: &str) -> EngineError {
    corint_decision_compiler::core::diagnostic("<decision-host>", "", "host", code, message).into()
}

impl FeatureHostConfig {
    /// Includes effective datasource configuration, not just its declared revision.
    pub fn binding_sha256(&self) -> String {
        canonical_sha256(self)
    }

    pub fn resources(&self) -> Vec<serde_json::Value> {
        self.plan
            .outputs
            .iter()
            .map(|output| {
                json!({
                    "kind":"feature", "id":output.definition.name,
                    "revision":self.plan.revision, "sha256":canonical_sha256(&output.definition)
                })
            })
            .collect()
    }

    fn validate(&self) -> Result<(), EngineError> {
        #[cfg(not(feature = "sqlx"))]
        if !self.datasources.is_empty() {
            return Err(fail(
                "E_FEATURE_CAPABILITY",
                "SQL input resources require the sqlx build feature",
            ));
        }
        if self.plan.format_version != "1"
            || self.plan.revision.trim().is_empty()
            || self.plan.outputs.is_empty()
            || self.plan.outputs.len() > 64
            || !(1..=60_000).contains(&self.plan.timeout_ms)
            || self.datasources.len() > 64
        {
            return Err(fail("E_FEATURE_PLAN", "Require bounded v1 feature plan"));
        }
        let revisions: BTreeMap<_, _> = self
            .datasources
            .iter()
            .map(|(name, binding)| (name.clone(), binding.revision.clone()))
            .collect();
        if revisions != self.plan.datasource_revisions {
            return Err(fail(
                "E_FEATURE_BINDING",
                "Datasource revisions differ from feature plan",
            ));
        }
        for (name, binding) in &self.datasources {
            if name.trim().is_empty()
                || name != &binding.config.name
                || binding.revision.trim().is_empty()
                || binding.config.query_cache_ttl_secs != 0
            {
                return Err(fail(
                    "E_FEATURE_BINDING",
                    "Require matching datasource names, revisions and fresh queries",
                ));
            }
            if !matches!(&binding.config.source_type, crate::DataSourceType::SQL(sql)
                if matches!(sql.provider, crate::SQLProvider::SQLite | crate::SQLProvider::PostgreSQL))
            {
                return Err(fail(
                    "E_FEATURE_CAPABILITY",
                    "Decision host currently supports SQLite/PostgreSQL aggregation sources",
                ));
            }
        }
        Ok(())
    }
}

enum Executor {
    Core(DecisionEngine),
    Features(FeaturePipeline),
}

pub struct DecisionHost {
    executor: Executor,
    feature_binding: Option<String>,
    resources: Vec<serde_json::Value>,
    admission: Semaphore,
}

/// Returned even when input enrichment or Core execution fails, so adapters can
/// retain the exact input/evidence without reconstructing an execution afterward.
pub struct HostExecution {
    pub result: Result<DecisionResponse, EngineError>,
    pub input_evidence: serde_json::Value,
    pub feature_evidence: Option<FeatureEvidence>,
    pub resources: Vec<serde_json::Value>,
}

impl DecisionHost {
    pub async fn new(
        sources: &[CoreSource],
        schema: Schema,
        features: Option<FeatureHostConfig>,
        enable_metrics: bool,
    ) -> Result<Self, EngineError> {
        let engine =
            DecisionEngine::from_core_with_metrics(sources, schema.clone(), enable_metrics)?;
        let Some(config) = features else {
            return Ok(Self {
                executor: Executor::Core(engine),
                feature_binding: None,
                resources: vec![],
                admission: Semaphore::new(64),
            });
        };
        config.validate()?;
        let feature_binding = Some(config.binding_sha256());
        let resources = config.resources();
        let datasources = tokio::time::timeout(Duration::from_secs(60), async {
            let mut clients = HashMap::new();
            for (name, binding) in config.datasources {
                let client = DataSourceClient::new(binding.config).await.map_err(|_| {
                    fail(
                        "E_FEATURE_CONNECTION",
                        "Feature datasource initialization failed",
                    )
                })?;
                clients.insert(name, (binding.revision, client));
            }
            Ok::<_, EngineError>(clients)
        })
        .await
        .map_err(|_| {
            fail(
                "E_FEATURE_CONNECTION",
                "Feature datasource initialization deadline exceeded",
            )
        })??;
        let plan_binding = config.plan.binding_sha256();
        let pipeline =
            FeaturePipeline::from_engine(engine, schema, config.plan, datasources, &plan_binding)?;
        Ok(Self {
            executor: Executor::Features(pipeline),
            feature_binding,
            resources,
            admission: Semaphore::new(64),
        })
    }

    pub fn feature_binding_sha256(&self) -> Option<&str> {
        self.feature_binding.as_deref()
    }
    pub fn required_resources(&self) -> &[serde_json::Value] {
        &self.resources
    }

    pub fn metrics(&self) -> Arc<MetricsCollector> {
        match &self.executor {
            Executor::Core(engine) => engine.metrics(),
            Executor::Features(pipeline) => pipeline.engine().metrics(),
        }
    }

    /// `as_of` is Unix seconds selected by the trusted host, never an HTTP field.
    pub async fn decide(
        &self,
        event: HashMap<String, Value>,
        as_of: i64,
        trace: bool,
    ) -> HostExecution {
        let started = std::time::Instant::now();
        let mut input = json!(event);
        let mut evidence = None;
        let mut resources = Vec::new();
        let mut result = match &self.executor {
            Executor::Core(engine) => {
                let request = DecisionRequest::new(event);
                engine
                    .decide(if trace { request.with_trace() } else { request })
                    .await
            }
            Executor::Features(pipeline) => {
                input = json!({"format_version":"1", "raw_event":event,
                    "event":null, "feature_evidence":null, "as_of":as_of,
                    "feature_binding_sha256":self.feature_binding});
                match self.admission.try_acquire() {
                    Err(_) => Err(fail("E_HOST_BUSY", "Feature decision admission full")),
                    Ok(_permit) => match pipeline.prepare_input(event, as_of).await {
                        Err(error) => Err(error),
                        Ok(prepared) => {
                            input["event"] = json!(prepared.event);
                            input["feature_evidence"] = json!(prepared.evidence);
                            evidence = Some(prepared.evidence);
                            resources = self.resources.clone();
                            let request = DecisionRequest::new(prepared.event);
                            pipeline
                                .engine()
                                .decide(if trace { request.with_trace() } else { request })
                                .await
                        }
                    },
                }
            }
        };
        if let Ok(response) = &mut result {
            response.processing_time_ms = started.elapsed().as_millis() as u64;
        }
        input.sort_all_objects();
        HostExecution {
            result,
            input_evidence: input,
            feature_evidence: evidence,
            resources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<CoreSource>, Schema, FeatureHostConfig) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/cdl_core");
        let read = |name: &str| CoreSource {
            path: name.into(),
            yaml: std::fs::read_to_string(root.join(name)).unwrap(),
        };
        let schema =
            corint_decision_compiler::core::parse_core_input_schema(&read("input-schema.yaml"))
                .unwrap();
        let sources = [
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .iter()
        .map(|name| read(name))
        .collect();
        let config = serde_json::from_value(json!({"plan":{"format_version":"1","revision":"constant-v1","datasource_revisions":{},"timeout_ms":1000,"outputs":[{"field":"amount","definition":{"name":"constant","type":"expression","expression":"1100"}}]},"datasources":{}})).unwrap();
        (sources, schema, config)
    }
    #[tokio::test]
    async fn bounded_admission_releases_and_disabled_metrics_apply_to_enriched_decisions() {
        let (sources, schema, config) = fixture();
        let host = DecisionHost::new(&sources, schema, Some(config), false)
            .await
            .unwrap();
        let permit = host.admission.acquire_many(64).await.unwrap();
        let rejected = host.decide(HashMap::new(), 120, false).await;
        assert!(rejected
            .result
            .unwrap_err()
            .to_string()
            .contains("E_HOST_BUSY"));
        assert!(rejected.feature_evidence.is_none());
        assert!(rejected.input_evidence["event"].is_null());
        drop(permit);
        let accepted = host.decide(HashMap::new(), 120, true).await;
        assert_eq!(accepted.result.unwrap().result.score, 60);
        assert_eq!(
            accepted.feature_evidence.unwrap().values["amount"],
            Value::Number(1100.0)
        );
        assert!(!host.metrics().enabled());
        assert!(host.metrics().histogram_names().is_empty());
        assert_eq!(host.admission.available_permits(), 64);
    }
    #[test]
    fn invalid_host_configuration_is_rejected_before_connecting() {
        let (_, _, config) = fixture();
        let mut bad = config.clone();
        bad.plan.timeout_ms = 0;
        assert!(bad.validate().is_err());
        let mut bad = config.clone();
        bad.plan
            .datasource_revisions
            .insert("missing".into(), "v1".into());
        assert!(bad.validate().is_err());
        let mut configured = config.clone();
        configured
            .plan
            .datasource_revisions
            .insert("events".into(), "v1".into());
        configured.datasources.insert("events".into(), serde_json::from_value(json!({"revision":"v1","config":{"name":"events","type":"sql","provider":"postgresql","connection_string":"postgres://unused/unused","database":"test","query_cache_ttl_secs":10}})).unwrap());
        assert!(configured.validate().is_err());
        configured
            .datasources
            .get_mut("events")
            .unwrap()
            .config
            .query_cache_ttl_secs = 0;
        assert_eq!(configured.validate().is_ok(), cfg!(feature = "sqlx"));
        assert_ne!(configured.binding_sha256(), config.binding_sha256());
    }
}
