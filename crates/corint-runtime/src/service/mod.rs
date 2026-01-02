//! Service integration module
//!
//! Provides async interfaces for calling internal services:
//! - HTTP microservices (ms_http)
//! - gRPC microservices (ms_grpc)
//! - Message queues (mq)
//! - Databases (legacy support)
//! - Redis (legacy support)

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
