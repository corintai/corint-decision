//! List management module
//!
//! Provides functionality for managing and querying blocklists, allowlists,
//! and watchlists.

pub mod backend;
pub mod config;
pub mod loader;
pub mod service;

#[cfg(test)]
mod tests;

pub use backend::{FileBackend, ListBackend, MemoryBackend, PostgresBackend};
pub use config::{ListBackendType, ListConfig, ListsConfig};
pub use loader::ListLoader;
pub use service::ListService;
