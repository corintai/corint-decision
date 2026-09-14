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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_at_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<FeatureFreshness>,
    pub definition: FeatureDefinition,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureFreshness {
    pub entity: String,
    pub key_field: String,
    pub key: String,
    pub watermark_field: String,
    pub max_lag_seconds: u32,
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
    pub plan_revision: String,
    pub datasource_revisions: BTreeMap<String, String>,
    pub as_of: i64,
    pub values: BTreeMap<String, Value>,
    pub freshness: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug)]
pub struct FeatureDecision {
    pub response: DecisionResponse,
    pub evidence: FeatureEvidence,
    /// Caller may explicitly retain this complete input with replay::capture.
    pub replay_event: HashMap<String, Value>,
}
pub(crate) struct PreparedFeatureInput {
    pub event: HashMap<String, Value>,
    pub evidence: FeatureEvidence,
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
        let engine = DecisionEngine::from_core(sources, schema.clone())?;
        Self::from_engine(engine, schema, plan, datasources, expected_binding)
    }

    pub(crate) fn from_engine(
        engine: DecisionEngine,
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
        // Outputs are unavailable in the raw event for EVERY feature, including
        // references to another output. Dependencies use features.<name> instead.
        for output in &plan.outputs {
            if let Some(expression) = output
                .definition
                .expression
                .as_ref()
                .and_then(|c| c.expression.as_deref())
            {
                let ast = corint_decision_dsl_parser::ExpressionParser::parse(expression)
                    .map_err(|_| fail("/outputs", "E_FEATURE_INPUT", "Invalid expression"))?;
                let mut pending = vec![&ast];
                while let Some(node) = pending.pop() {
                    use corint_decision_model::ast::Expression;
                    match node {
                        Expression::FieldAccess(path)
                            if path.first().is_some_and(|p| p == "event") =>
                        {
                            validate_raw_path(&raw_schema, &path[1..], true)?;
                        }
                        Expression::Binary { left, right, .. } => {
                            pending.push(left);
                            pending.push(right);
                        }
                        Expression::Unary { operand, .. } => pending.push(operand),
                        Expression::FunctionCall { args, .. } => pending.extend(args),
                        _ => (),
                    }
                }
            }
            let identifier = |name: &str| {
                !name.is_empty()
                    && name.len() <= 128
                    && name.bytes().enumerate().all(|(i, b)| {
                        b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit()
                    })
            };
            if output
                .available_at_field
                .as_ref()
                .is_some_and(|f| !identifier(f))
                || output.freshness.as_ref().is_some_and(|f| {
                    !identifier(&f.entity)
                        || !identifier(&f.key_field)
                        || !identifier(&f.watermark_field)
                        || f.key.len() > 256
                        || f.max_lag_seconds > 86400
                })
                || (output.available_at_field.is_some() || output.freshness.is_some())
                    && output.definition.aggregation.is_none()
            {
                return Err(fail(
                    "/outputs",
                    "E_FEATURE_FRESHNESS",
                    "Invalid availability or watermark contract",
                ));
            }
            if let Some(field) = &output.available_at_field {
                executor.set_availability_field(output.definition.name.clone(), field.clone());
            }
            if let Some(config) = &output.definition.aggregation {
                validate_template(&raw_schema, &config.dimension_value)?;
                if let Some(when) = &config.when {
                    for filter in corint_decision_runtime::feature::validated_filters(when)
                        .map_err(|_| {
                            fail("/outputs", "E_FEATURE_PLAN", "Invalid aggregation filter")
                        })?
                    {
                        if let Value::String(text) = filter.value {
                            validate_template(&raw_schema, &text)?;
                        }
                    }
                }
            }
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
        let prepared = self.prepare_input(event, as_of).await?;
        let request = DecisionRequest::new(prepared.event.clone());
        let response = self
            .engine
            .decide(if trace { request.with_trace() } else { request })
            .await?;
        Ok(FeatureDecision {
            response,
            replay_event: prepared.event,
            evidence: prepared.evidence,
        })
    }

    pub(crate) fn engine(&self) -> &DecisionEngine {
        &self.engine
    }

    pub(crate) async fn prepare_input(
        &self,
        event: HashMap<String, Value>,
        as_of: i64,
    ) -> Result<PreparedFeatureInput, EngineError> {
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
        let (values, freshness) = tokio::time::timeout(Duration::from_millis(self.plan.timeout_ms), async {
            use corint_decision_runtime::datasource::query::{Query, QueryType, Filter, FilterOperator};
            let mut freshness = BTreeMap::new();
            for output in &self.plan.outputs {
                let mut evidence = serde_json::json!({"watermark_checked":false,"availability_filtered":output.available_at_field.is_some()});
                if let Some(contract) = &output.freshness {
                    let source = &output.definition.aggregation.as_ref().unwrap().datasource;
                    let rows = self.executor.query_datasource(source, Query {
                        query_type: QueryType::RawEvents, entity: contract.entity.clone(),
                        filters: vec![Filter { field: contract.key_field.clone(), operator: FilterOperator::Eq, value: Value::String(contract.key.clone()) }],
                        time_window: None, aggregations: vec![], group_by: vec![], limit: Some(2),
                    }).await.map_err(|_| fail("/outputs", "E_FEATURE_FRESHNESS", "Watermark query failed"))?;
                    let watermark = match rows.rows.as_slice() {
                        [row] => match row.get(&contract.watermark_field) {
                            Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= chrono::Utc::now().timestamp() as f64 => *n as i64,
                            _ => return Err(fail("/outputs", "E_FEATURE_FRESHNESS", "Invalid source watermark")),
                        },
                        _ => return Err(fail("/outputs", "E_FEATURE_FRESHNESS", "Expected exactly one watermark")),
                    };
                    if watermark < as_of.saturating_sub(i64::from(contract.max_lag_seconds)) {
                        return Err(fail("/outputs", "E_FEATURE_FRESHNESS", "Source watermark is stale"));
                    }
                    evidence["watermark_checked"] = true.into();
                    evidence["watermark_unix_seconds"] = watermark.into();
                    evidence["max_lag_seconds"] = contract.max_lag_seconds.into();
                }
                freshness.insert(output.field.clone(), evidence);
            }
            let values = self.executor.execute_features_at(&names, &context, as_of).await
                .map_err(|_| fail("/outputs", "E_FEATURE_EXECUTION", "Feature calculation failed; no decision was executed"))?;
            Ok::<_, EngineError>((values, freshness))
        }).await.map_err(|_| fail("/outputs", "E_FEATURE_TIMEOUT", "Feature deadline exceeded"))??;
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
        Ok(PreparedFeatureInput {
            event: enriched,
            evidence: FeatureEvidence {
                format_version: "1",
                binding_sha256: self.plan.binding_sha256(),
                plan_revision: self.plan.revision.clone(),
                datasource_revisions: self.plan.datasource_revisions.clone(),
                as_of,
                values: evidence,
                freshness,
            },
        })
    }
}

fn validate_raw_path(schema: &Schema, path: &[String], numeric: bool) -> Result<(), EngineError> {
    let mut schema = schema;
    let mut field_type = None;
    for (index, name) in path.iter().enumerate() {
        let field = schema.fields.get(name).ok_or_else(|| {
            fail(
                "/outputs",
                "E_FEATURE_INPUT",
                "Feature references an undeclared or generated raw input",
            )
        })?;
        field_type = Some(&field.field_type);
        if index + 1 < path.len() {
            schema = match &field.field_type {
                crate::FieldType::Object {
                    schema: Some(schema),
                } => schema,
                _ => {
                    return Err(fail(
                        "/outputs",
                        "E_FEATURE_INPUT",
                        "Nested input needs an explicit object schema",
                    ))
                }
            };
        }
    }
    if !matches!(field_type, Some(crate::FieldType::Number)) && numeric
        || !numeric
            && !matches!(
                field_type,
                Some(
                    crate::FieldType::Number | crate::FieldType::String | crate::FieldType::Boolean
                )
            )
    {
        return Err(fail(
            "/outputs",
            "E_FEATURE_INPUT",
            "Feature operand has an incompatible raw input type",
        ));
    }
    Ok(())
}
fn validate_template(schema: &Schema, template: &str) -> Result<(), EngineError> {
    let mut rest = template;
    while let Some(start) = rest.find("${").or_else(|| rest.find("{event.")) {
        let body = &rest[start + if rest[start..].starts_with('$') { 2 } else { 1 }..];
        let end = body
            .find('}')
            .ok_or_else(|| fail("/outputs", "E_FEATURE_INPUT", "Unclosed template"))?;
        let path = body[..end].strip_prefix("event.").unwrap_or(&body[..end]);
        validate_raw_path(
            schema,
            &path.split('.').map(str::to_owned).collect::<Vec<_>>(),
            false,
        )?;
        rest = &body[end + 1..];
    }
    if let Some(path) = template.strip_prefix("event.") {
        validate_raw_path(
            schema,
            &path.split('.').map(str::to_owned).collect::<Vec<_>>(),
            false,
        )?;
    }
    Ok(())
}
