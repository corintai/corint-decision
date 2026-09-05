//! Strict Core resource calls share one VM and keep caller and callee results isolated.
use super::*;
use corint_decision_model::ir::condition_map::{CONDITION_TRACE, TRACE_ENABLED, TRACE_INVOCATION};

fn shared(key: &str) -> bool {
    [
        CONDITION_TRACE,
        TRACE_ENABLED,
        TRACE_INVOCATION,
        "__core_rule_executions__",
        "__core_calls__",
    ]
    .contains(&key)
}

impl PipelineExecutor {
    pub(super) async fn execute_core_call(
        &self,
        kind: &str,
        id: &str,
        input: &crate::ContextInput,
        ctx: &mut ExecutionContext,
    ) -> Result<()> {
        let program = self.core_programs.get(id).ok_or_else(|| {
            RuntimeError::InvalidOperation(format!("Unknown Core resource: {id}"))
        })?;
        if program.metadata.source_type != kind {
            return Err(RuntimeError::InvalidOperation(
                "Core resource kind mismatch".into(),
            ));
        }
        let mut local = ExecutionResult::new();
        local.variables = ctx
            .result
            .variables
            .iter()
            .filter(|(k, _)| shared(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut path = match ctx.result.variables.get("__core_call_path__") {
            Some(Value::Array(p)) => p.clone(),
            _ => vec![],
        };
        if path.len() >= 17 {
            return Err(RuntimeError::InvalidOperation(
                "Core call depth exceeded".into(),
            ));
        }
        path.push(Value::String(id.into()));
        local
            .variables
            .insert("__core_call_path__".into(), Value::Array(path));
        if kind == "ruleset" {
            let (rules, _) = self
                .ruleset_programs
                .as_ref()
                .and_then(|p| p.get(id))
                .ok_or_else(|| {
                    RuntimeError::InvalidOperation("Missing Core ruleset registry".into())
                })?;
            for rule in rules {
                // A rule has no signal/actions; score and matched IDs accumulate locally.
                let previous = local.score;
                let result = Box::pin(self.execute_with_result(rule, input.clone(), local)).await?;
                let mut variables = result.context;
                let mut records = match variables.remove("__core_rule_executions__") {
                    Some(Value::Array(v)) => v,
                    _ => vec![],
                };
                records.push(Value::Object(HashMap::from([
                    ("ruleset_id".into(), Value::String(id.into())),
                    (
                        "rule_id".into(),
                        Value::String(rule.metadata.source_id.clone()),
                    ),
                    (
                        "triggered".into(),
                        Value::Bool(result.triggered_rules.contains(&rule.metadata.source_id)),
                    ),
                    (
                        "score".into(),
                        Value::Number(f64::from(result.score) - f64::from(previous)),
                    ),
                ])));
                variables.insert("__core_rule_executions__".into(), Value::Array(records));
                local = ExecutionResult::new();
                local.score = result.score;
                local.triggered_rules = result.triggered_rules;
                local.variables = variables;
            }
        }
        let result = Box::pin(self.execute_with_result(program, input.clone(), local)).await?;
        let skipped = kind == "pipeline" && result.signal.is_none();
        for (key, value) in &result.context {
            if shared(key) {
                ctx.store_variable(key.clone(), value.clone());
            }
        }
        let mut output = HashMap::from([(
            "status".into(),
            Value::String(if skipped { "skipped" } else { "completed" }.into()),
        )]);
        if !skipped {
            ctx.result.score = ctx.result.score.checked_add(result.score).ok_or_else(|| {
                RuntimeError::InvalidOperation("E_SCORE_OVERFLOW: i32 aggregate overflow".into())
            })?;
            ctx.result
                .triggered_rules
                .extend(result.triggered_rules.clone());
            output.insert("score".into(), Value::Number(f64::from(result.score)));
            output.insert("total_score".into(), Value::Number(f64::from(result.score)));
            if kind == "rule" {
                let triggered = result.triggered_rules.contains(&id.to_owned());
                output.insert("matched".into(), Value::Bool(triggered));
                let mut records = match ctx.result.variables.remove("__core_rule_executions__") {
                    Some(Value::Array(v)) => v,
                    _ => vec![],
                };
                records.push(Value::Object(HashMap::from([
                    ("ruleset_id".into(), Value::Null),
                    ("rule_id".into(), Value::String(id.into())),
                    ("triggered".into(), Value::Bool(triggered)),
                    ("score".into(), Value::Number(f64::from(result.score))),
                ])));
                ctx.store_variable("__core_rule_executions__".into(), Value::Array(records));
            } else {
                let signal = result.signal.as_ref().ok_or_else(|| {
                    RuntimeError::InvalidOperation("Missing Core conclusion".into())
                })?;
                output.insert(
                    "signal".into(),
                    Value::String(format!("{signal:?}").to_lowercase()),
                );
            }
        }
        self.record_core_call(
            id,
            if skipped { "skipped" } else { "completed" },
            if skipped { None } else { Some(&result) },
            ctx,
        )?;
        ctx.store_variable(
            format!("__ruleset_result__.{id}"),
            Value::Object(output.clone()),
        );
        if kind == "ruleset" {
            ctx.store_variable("__last_ruleset_result__".into(), Value::Object(output));
        }
        Ok(())
    }

    pub(super) fn record_core_call(
        &self,
        id: &str,
        status: &str,
        result: Option<&DecisionResult>,
        ctx: &mut ExecutionContext,
    ) -> Result<()> {
        if ctx.result.variables.get(TRACE_ENABLED) != Some(&Value::Bool(true)) {
            return Ok(());
        }
        let program = self
            .core_programs
            .get(id)
            .ok_or_else(|| RuntimeError::InvalidOperation("Missing Core trace resource".into()))?;
        let mut path = match ctx.result.variables.get("__core_call_path__") {
            Some(Value::Array(p)) => p
                .iter()
                .filter_map(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        path.push(id.to_owned());
        let trace = crate::result::CoreCallTrace {
            source: program.metadata.custom["core_source"].clone(),
            resource_type: program.metadata.source_type.clone(),
            resource_id: id.into(),
            call_path: path,
            status: status.into(),
            score: result.map(|r| r.score),
            signal: result
                .and_then(|r| r.signal.as_ref())
                .map(|s| format!("{s:?}").to_lowercase()),
            actions: result.map(|r| r.actions.clone()).unwrap_or_default(),
        };
        let mut records = match ctx.result.variables.remove("__core_calls__") {
            Some(Value::Array(v)) => v,
            _ => vec![],
        };
        records.push(Value::String(
            serde_json::to_string(&trace).expect("Core trace JSON"),
        ));
        ctx.store_variable("__core_calls__".into(), Value::Array(records));
        Ok(())
    }
}
