//! Explicit, version-bound input enrichment before strict Core execution.
//! This does not enable dynamic Feature syntax or caller-supplied trusted values.
use crate::{
    CoreSource, DecisionEngine, DecisionRequest, DecisionResponse, EngineError, Schema, Value,
};
use corint_decision_compiler::core::{diagnostic, validate_core_input};
use corint_decision_runtime::{
    context::ExecutionContext,
    datasource::DataSourceClient,
    feature::{FeatureDefinition, FeatureExecutor, FeatureType},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureInput {
    pub field: String,
    pub definition: FeatureDefinition,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeaturePlan {
    pub format_version: String,
    pub revision: String,
    pub datasource_revisions: BTreeMap<String, String>,
    pub timeout_ms: u64,
    pub outputs: Vec<FeatureInput>,
}
impl FeaturePlan {
    pub fn binding_sha256(&self) -> String {
        let mut canonical = serde_json::to_value(self).expect("feature plan JSON");
        canonical.sort_all_objects();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&canonical).unwrap())
        )
    }
}

#[derive(Debug, Serialize)]
pub struct FeatureEvidence {
    pub format_version: &'static str,
    pub binding_sha256: String,
    pub as_of: i64,
    pub values: BTreeMap<String, Value>,
}
#[derive(Debug)]
pub struct FeatureDecision {
    pub response: DecisionResponse,
    pub evidence: FeatureEvidence,
    /// Caller may explicitly retain this complete input with replay::capture.
    pub replay_event: HashMap<String, Value>,
}
pub struct FeaturePipeline {
    engine: DecisionEngine,
    raw_schema: Schema,
    executor: FeatureExecutor,
    plan: FeaturePlan,
}
fn fail(field: &str, code: &str, message: &str) -> EngineError {
    diagnostic("<feature-plan>", field, "feature", code, message).into()
}
impl FeaturePipeline {
    /// Target revisions are operator-owned deployment identifiers, not database
    /// contents or authenticated server attestations. They must match exactly.
    pub fn new(
        sources: &[CoreSource],
        schema: Schema,
        plan: FeaturePlan,
        datasources: HashMap<String, (String, DataSourceClient)>,
        expected_binding: &str,
    ) -> Result<Self, EngineError> {
        if plan.format_version != "1"
            || plan.revision.trim().is_empty()
            || plan.outputs.is_empty()
            || plan.outputs.len() > 64
            || !(1..=60_000).contains(&plan.timeout_ms)
        {
            return Err(fail(
                "",
                "E_FEATURE_PLAN",
                "Require v1, revision, 1–64 outputs and timeout 1–60000 ms",
            ));
        }
        if plan.binding_sha256() != expected_binding {
            return Err(fail("", "E_FEATURE_BINDING", "Feature plan changed"));
        }
        let mut executor = FeatureExecutor::new();
        let mut actual_revisions = BTreeMap::new();
        for (name, (revision, client)) in datasources {
            if revision.trim().is_empty() || client.query_cache_ttl_secs() != 0 {
                return Err(fail(
                    "/datasource_revisions",
                    "E_FEATURE_FRESHNESS",
                    "Require explicit revision and fresh query reads",
                ));
            }
            actual_revisions.insert(name.clone(), revision);
            executor.add_datasource(name, client).map_err(|_| {
                fail(
                    "/datasource_revisions",
                    "E_FEATURE_TARGET",
                    "Invalid datasource capability",
                )
            })?;
        }
        if actual_revisions != plan.datasource_revisions {
            return Err(fail(
                "/datasource_revisions",
                "E_FEATURE_BINDING",
                "Target datasource revisions differ",
            ));
        }
        let mut raw_schema = schema.clone();
        let mut names = BTreeSet::new();
        let mut fields = BTreeSet::new();
        for output in &plan.outputs {
            let field = schema.fields.get(&output.field).ok_or_else(|| {
                fail(
                    "/outputs",
                    "E_FEATURE_INPUT",
                    "Output must bind a declared input field",
                )
            })?;
            if !field.required
                || field.field_type != crate::FieldType::Number
                || !fields.insert(output.field.clone())
                || !names.insert(output.definition.name.clone())
            {
                return Err(fail(
                    "/outputs",
                    "E_FEATURE_INPUT",
                    "Outputs require unique required numeric fields and feature names",
                ));
            }
            if !output.definition.enabled
                || !matches!(
                    output.definition.feature_type,
                    FeatureType::Aggregation | FeatureType::Expression
                )
            {
                return Err(fail(
                    "/outputs",
                    "E_FEATURE_CAPABILITY",
                    "Only enabled aggregation/expression features are admitted",
                ));
            }
            if let Some(config) = &output.definition.aggregation {
                if config.window.is_none()
                    || !plan.datasource_revisions.contains_key(&config.datasource)
                {
                    return Err(fail(
                        "/outputs",
                        "E_FEATURE_TARGET",
                        "Aggregation needs a window and bound datasource",
                    ));
                }
            }
            raw_schema.fields.remove(&output.field);
        }
        executor
            .register_features(plan.outputs.iter().map(|v| v.definition.clone()).collect())
            .map_err(|_| {
                fail(
                    "/outputs",
                    "E_FEATURE_PLAN",
                    "Invalid feature definitions, dependencies or backend capability",
                )
            })?;
        let engine = DecisionEngine::from_core(sources, schema)?;
        Ok(Self {
            engine,
            raw_schema,
            executor,
            plan,
        })
    }

    /// Cutoff is supplied separately by the host. Raw request validation happens
    /// before any database call; callers cannot inject a bound output field.
    pub async fn decide(
        &self,
        event: HashMap<String, Value>,
        as_of: i64,
        trace: bool,
    ) -> Result<FeatureDecision, EngineError> {
        validate_core_input(&self.raw_schema, &event)?;
        let names: Vec<_> = self
            .plan
            .outputs
            .iter()
            .map(|o| o.definition.name.clone())
            .collect();
        let context = ExecutionContext::new(corint_decision_runtime::context::ContextInput::new(
            event.clone(),
        ))?;
        let values = tokio::time::timeout(
            Duration::from_millis(self.plan.timeout_ms),
            self.executor.execute_features_at(&names, &context, as_of),
        )
        .await
        .map_err(|_| fail("/outputs", "E_FEATURE_TIMEOUT", "Feature deadline exceeded"))?
        .map_err(|_| {
            fail(
                "/outputs",
                "E_FEATURE_EXECUTION",
                "Feature calculation failed; no decision was executed",
            )
        })?;
        let mut enriched = event;
        let mut evidence = BTreeMap::new();
        for output in &self.plan.outputs {
            let value = values
                .get(&output.definition.name)
                .ok_or_else(|| fail("/outputs", "E_FEATURE_MISSING", "Feature result missing"))?;
            if !matches!(value, Value::Number(n) if n.is_finite()) {
                return Err(fail(
                    "/outputs",
                    "E_FEATURE_VALUE",
                    "Feature must produce a finite number; null has no implicit default",
                ));
            }
            enriched.insert(output.field.clone(), value.clone());
            evidence.insert(output.field.clone(), value.clone());
        }
        let request = DecisionRequest::new(enriched.clone());
        let response = self
            .engine
            .decide(if trace { request.with_trace() } else { request })
            .await?;
        Ok(FeatureDecision {
            response,
            replay_event: enriched,
            evidence: FeatureEvidence {
                format_version: "1",
                binding_sha256: self.plan.binding_sha256(),
                as_of,
                values: evidence,
            },
        })
    }
}
