//! # Orbit Client Spring
//!
//! This crate provides Spring Framework-inspired patterns and utilities for Rust applications
//! integrated with the Orbit ecosystem. It offers dependency injection, configuration management,
//! and annotation-driven programming similar to Spring Boot.
//!
//! ## Features
//!
//! - **Dependency Injection**: IoC container with automatic dependency resolution
//! - **Configuration Management**: Type-safe configuration with validation
//! - **Component Model**: @Component, @Service, @Repository-like annotations via traits
//! - **Application Context**: Centralized application state and lifecycle management
//! - **Orbit Integration**: Seamless integration with Orbit client functionality
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_client_spring::{ApplicationContext, Component, Service};
//! use async_trait::async_trait;
//!
//! #[derive(Debug, Clone)]
//! pub struct UserService {
//!     // service implementation
//! }
//!
//! #[async_trait]
//! impl Service for UserService {
//!     fn name(&self) -> &'static str {
//!         "UserService"
//!     }
//!     
//!     async fn initialize(&mut self) -> Result<(), SpringError> {
//!         Ok(())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut context = ApplicationContext::new();
//!     context.register_service(UserService {}).await?;
//!     context.start().await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod annotations;
pub mod config;
pub mod container;
pub mod context;
pub mod error;
pub mod integration;
pub mod metrics;
pub mod scheduler;

// Java Spring Boot integration modules
pub mod grpc_server;
pub mod http_server;
pub mod java_integration;
pub mod jni_bindings;

// Re-exports for convenience
pub use annotations::*;
pub use config::*;
pub use container::*;
pub use context::*;
pub use error::*;
pub use integration::*;
pub use metrics::*;
pub use scheduler::*;

// Java Spring Boot integration re-exports
pub use grpc_server::*;
pub use http_server::*;
pub use java_integration::*;
pub use jni_bindings::*;
