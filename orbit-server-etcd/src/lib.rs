//! # Orbit Server etcd
//!
//! This crate provides comprehensive etcd integration for Orbit servers, including
//! service discovery, configuration management, distributed coordination, and
//! leader election capabilities.
//!
//! ## Features
//!
//! - **Service Discovery**: Automatic service registration and discovery
//! - **Configuration Management**: Distributed configuration with watch capabilities
//! - **Leader Election**: Distributed leader election with automatic failover
//! - **Health Monitoring**: Service health checking and monitoring
//! - **Key-Value Store**: Distributed key-value storage with TTL support
//! - **Coordination**: Distributed locks and coordination primitives
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_server_etcd::{EtcdClient, EtcdConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = EtcdConfig::default();
//!     let client = EtcdClient::new(config).await?;
//!     
//!     // Register service
//!     client.register_service("my-service", "127.0.0.1:8080").await?;
//!     
//!     // Discover services
//!     let services = client.discover_services("my-service").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod coordination;
pub mod discovery;
pub mod election;
pub mod error;
pub mod health;
pub mod watcher;

pub use client::*;
pub use config::*;
pub use coordination::*;
pub use discovery::*;
pub use election::*;
pub use error::*;
pub use health::*;
pub use watcher::*;
