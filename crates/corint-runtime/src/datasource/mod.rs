//! Data Source Layer for Feature Engineering
//!
//! This module provides a unified interface for accessing feature data from multiple sources:
//! - Feature Store (Redis, Feast) - Pre-computed features
//! - OLAP Databases (ClickHouse, Druid) - Real-time analytical queries
//! - SQL Databases (PostgreSQL, MySQL) - Traditional relational databases

pub mod cache;
pub mod client;
pub mod config;
pub mod query;
mod feature_store;
mod olap;
mod sql;

pub use cache::{CacheStrategy, CachedResult};
pub use client::DataSourceClient;
pub use config::{DataSourceConfig, DataSourceType, FeatureStoreConfig, OLAPConfig, SQLConfig};
pub use query::{
    Aggregation, AggregationType, Filter, FilterOperator, Query, QueryResult, QueryType,
    RelativeWindow, TimeUnit, TimeWindow, TimeWindowType,
};
