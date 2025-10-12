//! # Orbit Server Prometheus
//!
//! This crate provides comprehensive Prometheus metrics integration for Orbit servers.
//! It includes metrics collection, export endpoints, custom metrics, and dashboard
//! generation capabilities.
//!
//! ## Features
//!
//! - **Metrics Collection**: Automatic collection of Orbit server metrics
//! - **Prometheus Export**: HTTP endpoint for Prometheus scraping
//! - **Custom Metrics**: Support for application-specific metrics
//! - **Dashboard Generation**: Grafana dashboard templates
//! - **Health Monitoring**: Service health and availability metrics
//! - **Performance Tracking**: Request latency, throughput, and error rates
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_server_prometheus::{PrometheusExporter, PrometheusConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = PrometheusConfig::default();
//!     let mut exporter = PrometheusExporter::new(config).await?;
//!     
//!     exporter.start().await?;
//!     exporter.run().await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod collector;
pub mod config;
pub mod dashboard;
pub mod error;
pub mod exporter;
pub mod metrics;
pub mod server;

pub use collector::*;
pub use config::*;
pub use dashboard::*;
pub use error::*;
pub use exporter::*;
pub use metrics::*;
pub use server::*;
