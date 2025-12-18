//! Decision result persistence
//!
//! Asynchronously persists decision results to database tables for:
//! - Problem investigation and troubleshooting
//! - Decision review and analysis
//! - Historical data tracking
//!
//! Tables:
//! - risk_decisions: Main decision results
//! - rule_executions: Individual rule execution logs

use crate::error::{Result, RuntimeError};
use crate::result::DecisionResult;
use corint_core::ast::Action;
use corint_core::Value;
use serde_json;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Rule execution record for persistence
#[derive(Debug, Clone)]
pub struct RuleExecutionRecord {
    /// Request ID (links to risk_decisions)
    pub request_id: String,

    /// Ruleset ID (for grouping rules in trace)
    pub ruleset_id: Option<String>,

    /// Rule ID
    pub rule_id: String,

    /// Rule name (optional)
    pub rule_name: Option<String>,

    /// Whether the rule was triggered
    pub triggered: bool,

    /// Rule score (if triggered)
    pub score: Option<i32>,

    /// Execution time in milliseconds
    pub execution_time_ms: Option<u64>,

    /// Feature values used in this rule
    pub feature_values: Option<HashMap<String, Value>>,

    /// Rule conditions (optional) - legacy string format
    pub rule_conditions: Option<serde_json::Value>,

    /// Structured conditions as JSON array for detailed tracing
    /// Each element is a structured object with type, expression, nested conditions, etc.
    pub conditions_json: Option<String>,

    /// Condition group JSON for new all/any format
    pub condition_group_json: Option<String>,
}

/// Decision result record for persistence
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// Request ID (unique identifier)
    pub request_id: String,

    /// Optional business event ID
    pub event_id: Option<String>,

    /// Pipeline ID that processed this decision
    pub pipeline_id: String,

    /// Risk score
    pub risk_score: i32,

    /// Decision action
    pub decision: Action,

    /// Decision reason/explanation
    pub decision_reason: Option<String>,

    /// Triggered rule IDs
    pub triggered_rules: Vec<String>,

    /// Individual rule scores
    pub rule_scores: HashMap<String, i32>,

    /// Feature values used in decision
    pub feature_values: Option<HashMap<String, Value>>,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Rule execution records
    pub rule_executions: Vec<RuleExecutionRecord>,
}

/// Async decision result writer that queues writes to avoid blocking decision execution
pub struct DecisionResultWriter {
    /// Channel sender for queuing decision records
    sender: mpsc::UnboundedSender<DecisionRecord>,
}

impl DecisionResultWriter {
    /// Create a new decision result writer with a database connection pool
    #[cfg(feature = "sqlx")]
    pub fn new(pool: sqlx::PgPool) -> Self {
        tracing::info!("Creating DecisionResultWriter with database connection pool");

        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn background task to process decision records
        tokio::spawn(async move {
            Self::process_records(receiver, pool).await;
        });

        tracing::info!("DecisionResultWriter created, background task spawned");

        Self { sender }
    }

    /// Create a new decision result writer without database (no-op)
    #[cfg(not(feature = "sqlx"))]
    pub fn new() -> Self {
        let (sender, _receiver) = mpsc::unbounded_channel();
        Self { sender }
    }

    /// Write a decision result record asynchronously
    pub fn write_decision(&self, record: DecisionRecord) -> Result<()> {
        self.sender.send(record).map_err(|e| {
            RuntimeError::RuntimeError(format!("Failed to queue decision record: {}", e))
        })
    }

    /// Process decision records in background
    #[cfg(feature = "sqlx")]
    async fn process_records(
        mut receiver: mpsc::UnboundedReceiver<DecisionRecord>,
        pool: sqlx::PgPool,
    ) {
        tracing::info!("Decision result writer background task started");

        while let Some(record) = receiver.recv().await {
            tracing::debug!(
                "Processing decision record for request_id: {}",
                record.request_id
            );

            match Self::write_to_database(&pool, &record).await {
                Ok(()) => {
                    tracing::info!(
                        "Successfully persisted decision record for request_id: {}",
                        record.request_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to write decision record to database for request_id {}: {}",
                        record.request_id,
                        e
                    );
                    tracing::error!("Error details: {:?}", e);
                }
            }
        }

        tracing::warn!("Decision result writer background task ended (channel closed)");
    }

    /// Write decision and rule execution records to database
    #[cfg(feature = "sqlx")]
    async fn write_to_database(pool: &sqlx::PgPool, record: &DecisionRecord) -> Result<()> {
        tracing::debug!(
            "Starting database transaction for request_id: {}",
            record.request_id
        );

        // Start transaction
        let mut tx = pool.begin().await.map_err(|e| {
            tracing::error!("Failed to begin transaction: {}", e);
            RuntimeError::RuntimeError(format!("Failed to begin transaction: {}", e))
        })?;

        // 1. Insert into risk_decisions table
        let decision_str = format!("{:?}", record.decision).to_lowercase();
        let triggered_rules_array: Vec<&str> =
            record.triggered_rules.iter().map(|s| s.as_str()).collect();

        // Convert rule_scores to JSONB
        let rule_scores_json = serde_json::to_value(&record.rule_scores).map_err(|e| {
            RuntimeError::RuntimeError(format!("Failed to serialize rule_scores: {}", e))
        })?;

        // Convert feature_values to JSONB
        let feature_values_json = if let Some(ref fv) = record.feature_values {
            Some(serde_json::to_value(fv).map_err(|e| {
                RuntimeError::RuntimeError(format!("Failed to serialize feature_values: {}", e))
            })?)
        } else {
            None
        };

        let insert_result = sqlx::query(
            r#"
            INSERT INTO risk_decisions (
                request_id, event_id, pipeline_id, risk_score, decision, decision_reason,
                triggered_rules, rule_scores, feature_values, processing_time_ms
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (request_id) DO UPDATE SET
                event_id = EXCLUDED.event_id,
                pipeline_id = EXCLUDED.pipeline_id,
                risk_score = EXCLUDED.risk_score,
                decision = EXCLUDED.decision,
                decision_reason = EXCLUDED.decision_reason,
                triggered_rules = EXCLUDED.triggered_rules,
                rule_scores = EXCLUDED.rule_scores,
                feature_values = EXCLUDED.feature_values,
                processing_time_ms = EXCLUDED.processing_time_ms
            "#,
        )
        .bind(&record.request_id)
        .bind(record.event_id.as_deref())
        .bind(&record.pipeline_id)
        .bind(record.risk_score)
        .bind(&decision_str)
        .bind(record.decision_reason.as_deref())
        .bind(&triggered_rules_array)
        .bind(&rule_scores_json)
        .bind(feature_values_json.as_ref())
        .bind(record.processing_time_ms as i32)
        .execute(&mut *tx)
        .await;

        match insert_result {
            Ok(result) => {
                let rows_affected = result.rows_affected();
                if rows_affected > 0 {
                    tracing::info!(
                        "Inserted risk_decision for request_id: {} (rows_affected: {})",
                        record.request_id,
                        rows_affected
                    );
                } else {
                    // ON CONFLICT DO UPDATE may return 0 if all values are the same
                    tracing::debug!("Updated risk_decision for request_id: {} (rows_affected: {}, likely ON CONFLICT UPDATE with no changes)", 
                        record.request_id, rows_affected);
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to insert risk_decision for request_id {}: {}",
                    record.request_id,
                    e
                );
                tracing::error!("SQL error details: {:?}", e);
                return Err(RuntimeError::RuntimeError(format!(
                    "Failed to insert risk_decision: {}",
                    e
                )));
            }
        }

        // 2. Insert rule execution records
        for rule_exec in &record.rule_executions {
            let feature_values_json = if let Some(ref fv) = rule_exec.feature_values {
                Some(serde_json::to_value(fv).map_err(|e| {
                    RuntimeError::RuntimeError(format!(
                        "Failed to serialize rule feature_values: {}",
                        e
                    ))
                })?)
            } else {
                None
            };

            let exec_result = sqlx::query(
                r#"
                INSERT INTO rule_executions (
                    request_id, rule_id, rule_name, triggered, score,
                    execution_time_ms, feature_values, rule_conditions
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(&rule_exec.request_id)
            .bind(&rule_exec.rule_id)
            .bind(rule_exec.rule_name.as_deref())
            .bind(rule_exec.triggered)
            .bind(rule_exec.score)
            .bind(rule_exec.execution_time_ms.map(|t| t as i32))
            .bind(feature_values_json.as_ref())
            .bind(rule_exec.rule_conditions.as_ref())
            .execute(&mut *tx)
            .await;

            match exec_result {
                Ok(result) => {
                    let rows_affected = result.rows_affected();
                    if rows_affected > 0 {
                        tracing::debug!(
                            "Inserted rule_execution for rule_id: {} (rows_affected: {})",
                            rule_exec.rule_id,
                            rows_affected
                        );
                    } else {
                        tracing::warn!("rule_execution insert returned rows_affected=0 for rule_id: {} (this should not happen)", 
                            rule_exec.rule_id);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to insert rule_execution for rule_id {}: {}",
                        rule_exec.rule_id,
                        e
                    );
                    return Err(RuntimeError::RuntimeError(format!(
                        "Failed to insert rule_execution: {}",
                        e
                    )));
                }
            }
        }

        tracing::info!(
            "Inserted {} rule_execution records for request_id: {}",
            record.rule_executions.len(),
            record.request_id
        );

        // Commit transaction
        let commit_result = tx.commit().await;
        match commit_result {
            Ok(()) => {
                tracing::info!(
                    "Transaction committed successfully for request_id: {}",
                    record.request_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to commit transaction for request_id {}: {}",
                    record.request_id,
                    e
                );
                return Err(RuntimeError::RuntimeError(format!(
                    "Failed to commit transaction: {}",
                    e
                )));
            }
        }

        Ok(())
    }
}

impl Default for DecisionResultWriter {
    #[cfg(feature = "sqlx")]
    fn default() -> Self {
        // Create a no-op writer if no pool is provided
        let (sender, _receiver) = mpsc::unbounded_channel();
        Self { sender }
    }

    #[cfg(not(feature = "sqlx"))]
    fn default() -> Self {
        let (sender, _receiver) = mpsc::unbounded_channel();
        Self { sender }
    }
}

/// Convert DecisionResult to DecisionRecord
impl DecisionRecord {
    /// Create decision record from decision result
    pub fn from_decision_result(
        request_id: String,
        event_id: Option<String>,
        pipeline_id: String,
        result: &DecisionResult,
        processing_time_ms: u64,
        rule_executions: Vec<RuleExecutionRecord>,
    ) -> Self {
        // Extract rule scores from context if available
        let mut rule_scores = HashMap::new();
        if let Some(Value::Object(scores)) = result.context.get("rule_scores") {
            for (rule_id, score_val) in scores {
                if let Value::Number(score) = score_val {
                    rule_scores.insert(rule_id.clone(), *score as i32);
                }
            }
        }

        // Extract feature values from context if available
        let feature_values = result.context.get("feature_values").and_then(|v| {
            if let Value::Object(fv) = v {
                Some(fv.clone())
            } else {
                None
            }
        });

        Self {
            request_id,
            event_id,
            pipeline_id,
            risk_score: result.score,
            decision: result.action.clone().unwrap_or(Action::Approve),
            decision_reason: if result.explanation.is_empty() {
                None
            } else {
                Some(result.explanation.clone())
            },
            triggered_rules: result.triggered_rules.clone(),
            rule_scores,
            feature_values,
            processing_time_ms,
            rule_executions,
        }
    }
}
