//! Observe compiler-verified VM boundaries; never evaluate expressions.
use crate::context::ExecutionContext;
use crate::error::{Result, RuntimeError};
use crate::result::{CoreConditionOutcome, CoreConditionTrace, CoreSkipReason};
use corint_decision_model::ir::condition_map::*;
use corint_decision_model::ir::Program;
use corint_decision_model::Value;
use std::collections::HashMap;

pub(super) struct ConditionObserver {
    maps: Vec<ConditionMap>,
    entered: Vec<Vec<bool>>,
    results: Vec<Vec<Option<bool>>>,
    invocation: u64,
    starts: HashMap<usize, Vec<(usize, usize)>>,
    ends: HashMap<usize, Vec<(usize, usize)>>,
}

impl ConditionObserver {
    pub(super) fn new(program: &Program, ctx: &mut ExecutionContext) -> Result<Option<Self>> {
        if ctx.result.variables.get(TRACE_ENABLED) != Some(&Value::Bool(true)) {
            return Ok(None);
        }
        let Some(json) = program.metadata.custom.get(CONDITION_MAP) else {
            return Ok(None);
        };
        let maps: Vec<ConditionMap> = serde_json::from_str(json).map_err(|e| {
            RuntimeError::InvalidOperation(format!("Invalid Core condition map: {e}"))
        })?;
        let mut starts: HashMap<_, Vec<_>> = HashMap::new();
        let mut ends: HashMap<_, Vec<_>> = HashMap::new();
        for (i, map) in maps.iter().enumerate() {
            for (j, node) in map.nodes.iter().enumerate() {
                starts.entry(node.start).or_default().push((i, j));
                ends.entry(node.end).or_default().push((i, j));
            }
        }
        let invocation = match ctx.result.variables.get(TRACE_INVOCATION) {
            Some(Value::Number(n)) => *n as u64,
            _ => 0,
        };
        ctx.store_variable(
            TRACE_INVOCATION.into(),
            Value::Number((invocation + 1) as f64),
        );
        Ok(Some(Self {
            entered: maps.iter().map(|m| vec![false; m.nodes.len()]).collect(),
            results: maps.iter().map(|m| vec![None; m.nodes.len()]).collect(),
            maps,
            invocation,
            starts,
            ends,
        }))
    }

    pub(super) fn observe(&mut self, pc: usize, ctx: &ExecutionContext) -> Result<()> {
        if let Some(nodes) = self.ends.get(&pc) {
            for &(i, j) in nodes {
                if self.entered[i][j] && self.results[i][j].is_none() {
                    let Value::Bool(result) = ctx.peek()? else {
                        return Err(RuntimeError::InvalidOperation(
                            "Non-boolean Core condition result".into(),
                        ));
                    };
                    self.results[i][j] = Some(*result);
                }
            }
        }
        if let Some(nodes) = self.starts.get(&pc) {
            for &(i, j) in nodes {
                self.entered[i][j] = true;
            }
        }
        Ok(())
    }

    pub(super) fn finish(self, program: &Program, ctx: &mut ExecutionContext) -> Result<()> {
        let mut records = match ctx.result.variables.remove(CONDITION_TRACE) {
            Some(Value::Array(records)) => records,
            _ => Vec::new(),
        };
        for (i, map) in self.maps.into_iter().enumerate() {
            for (j, node) in map.nodes.into_iter().enumerate() {
                let outcome = match self.results[i][j] {
                    Some(result) => CoreConditionOutcome::Evaluated { result },
                    None if self.entered[i][j] => {
                        return Err(RuntimeError::InvalidOperation(
                            "Incomplete Core condition observation".into(),
                        ))
                    }
                    None => CoreConditionOutcome::Skipped {
                        reason: if self.entered[i][0] {
                            CoreSkipReason::ShortCircuit
                        } else {
                            CoreSkipReason::NotReached
                        },
                    },
                };
                let record = CoreConditionTrace {
                    source: program.metadata.custom["core_source"].clone(),
                    resource_type: program.metadata.source_type.clone(),
                    resource_id: program.metadata.source_id.clone(),
                    invocation: self.invocation,
                    field_path: map.field_path.clone(),
                    node_path: node.node_path,
                    outcome,
                };
                // Preserve integer identifiers across the runtime's f64-only Value.
                records.push(Value::String(
                    serde_json::to_string(&record).expect("trace JSON"),
                ));
            }
        }
        ctx.store_variable(CONDITION_TRACE.into(), Value::Array(records));
        Ok(())
    }
}
