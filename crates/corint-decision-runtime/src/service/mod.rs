//! Service integration module
//!
//! Transport-independent service adapter interface and concrete HTTP bindings.
//! Internal/external deployment does not define separate types. Other modules
//! provide protocol interfaces and mocks; applications supply production adapters.

pub mod client;
pub mod database;
pub mod grpc;
pub mod http;
pub mod mq;
pub mod redis;

pub use client::{ServiceClient, ServiceRequest, ServiceResponse};
pub use database::{DatabaseClient, DatabaseQuery, MockDatabaseClient};
pub use grpc::{GrpcClient, MockGrpcClient};
pub use http::{HttpClient, HttpMethod, MockHttpClient};
pub use mq::{MockMqClient, MqClient, MqDriver, MqMessage, PublishedMessage};
pub use redis::{MockRedisClient, RedisClient, RedisCommand};

/// Configurable HTTP connector shared by internal and external services.
pub mod http_service;
pub use http_service::{
    HttpServiceClient, HttpServiceConfig, ServiceAuth, ServiceOperation, ServiceResponseMapping,
};
