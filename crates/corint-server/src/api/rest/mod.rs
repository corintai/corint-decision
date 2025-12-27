//! REST API implementation
//!
//! Modular REST API with clean separation of concerns:
//! - types: Request/response type definitions
//! - extractors: Custom request extractors and middleware
//! - conversions: Type conversion utilities
//! - handlers: API endpoint handlers
//! - router: Router creation and configuration
//! - tests: Unit tests for all components

mod conversions;
mod extractors;
mod handlers;
mod router;
mod tests;
pub mod types;

// Re-export public API
pub use extractors::JsonExtractor;
pub use router::{create_router, create_router_with_metrics};
pub use types::{
    AppState, AppStateWithMetrics, CognitionPayload, DecideRequestPayload, DecideResponsePayload,
    DecisionPayload, EvidencePayload, HealthResponse, ReloadResponse, RequestOptions,
    ScoresPayload,
};
