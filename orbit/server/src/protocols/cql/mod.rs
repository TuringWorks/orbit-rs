//! # CQL (Cassandra Query Language) Protocol Adapter
//!
//! This module provides a Cassandra-compatible CQL protocol adapter for Orbit-RS.
//! It implements the CQL wire protocol and allows Cassandra clients to interact
//! with Orbit's distributed storage system.
//!
//! ## Features
//!
//! - Full CQL 3.x wire protocol support
//! - Common CQL commands (SELECT, INSERT, UPDATE, DELETE, CREATE TABLE, etc.)
//! - Cassandra-style consistency levels
//! - Type system mapping (CQL types ↔ Orbit SqlValue)
//! - Prepared statements and batching
//! - Authentication support
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_server::protocols::cql::{CqlAdapter, CqlConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = CqlConfig {
//!     listen_addr: "127.0.0.1:9042".parse()?,
//!     max_connections: 1000,
//!     authentication_enabled: false,
//!     protocol_version: 4,
//!     username: None,
//!     password: None,
//! };
//!
//! let adapter = CqlAdapter::new(config).await?;
//! adapter.start().await?;
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod parser;
pub mod protocol;
pub mod types;

pub use adapter::CqlAdapter;
pub use parser::{CqlParser, CqlStatement, ComparisonOperator};
pub use protocol::{CqlFrame, CqlOpcode, ConsistencyLevel};
pub use types::{CqlType, CqlValue};

/// CQL configuration
#[derive(Debug, Clone)]
pub struct CqlConfig {
    /// Address to listen on (default: 127.0.0.1:9042)
    pub listen_addr: std::net::SocketAddr,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Enable authentication
    pub authentication_enabled: bool,
    /// CQL protocol version (default: 4)
    pub protocol_version: u8,
    /// Username for authentication (if authentication_enabled is true)
    pub username: Option<String>,
    /// Password for authentication (if authentication_enabled is true)
    pub password: Option<String>,
}

impl Default for CqlConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:9042".parse().unwrap(),
            max_connections: 1000,
            authentication_enabled: false,
            protocol_version: 4,
            username: None,
            password: None,
        }
    }
}
