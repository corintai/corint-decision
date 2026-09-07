//! CORINT Runtime - Execution engine for CORINT IR programs
//!
//! This crate provides the runtime execution engine that executes
//! compiled IR programs.

pub mod context;
pub mod datasource;
pub mod engine;
pub mod error;
pub mod executor;
pub mod feature;
pub mod lists;
pub mod observability;
pub mod result;
pub mod service;
pub mod storage;
pub mod validation;

// Re-export main types
pub use context::{ContextInput, ExecutionContext};
pub use datasource::{
    Aggregation, AggregationType, DataSourceClient, DataSourceConfig, DataSourceType, Filter,
    FilterOperator, Query, QueryResult, QueryType, RelativeWindow, TimeUnit, TimeWindow,
    TimeWindowType,
};
pub use engine::PipelineExecutor;
pub use error::{Result, RuntimeError};
pub use executor::Executor;
pub use feature::FeatureExtractor;
pub use lists::{ListBackend, ListService, MemoryBackend};
pub use observability::{Metrics, MetricsCollector};
pub use result::{
    ConclusionTrace, ConditionTrace, DecisionRecord, DecisionResult, DecisionResultWriter,
    ExecutionResult, ExecutionTrace, PipelineTrace, RuleExecutionRecord, RuleTrace, RulesetTrace,
    StepTrace,
};
pub use service::{
    HttpServiceClient, HttpServiceConfig, ServiceAuth, ServiceClient, ServiceOperation,
    ServiceRequest, ServiceResponse, ServiceResponseMapping,
};
pub use storage::{Event, EventFilter, InMemoryStorage, Storage, TimeRange};
