//! Service integration module
//!
//! Provides async interfaces for calling external services
//! (databases, Redis, REST APIs, etc.).

pub mod client;
pub mod database;
pub mod http;
pub mod redis;

pub use client::{ServiceClient, ServiceRequest, ServiceResponse};
pub use database::{DatabaseClient, DatabaseQuery, MockDatabaseClient};
pub use http::{HttpClient, HttpMethod, MockHttpClient};
pub use redis::{MockRedisClient, RedisClient, RedisCommand};
