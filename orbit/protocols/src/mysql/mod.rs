//! # MySQL Protocol Adapter
//!
//! This module provides a MySQL-compatible protocol adapter for Orbit-RS.
//! It implements the MySQL wire protocol and allows MySQL clients to interact
//! with Orbit's distributed storage system.
//!
//! ## Features
//!
//! - MySQL wire protocol 4.1+ support
//! - Authentication (native password, clear text)
//! - Common SQL commands (SELECT, INSERT, UPDATE, DELETE, etc.)
//! - Prepared statements
//! - Multiple result sets
//! - Transaction support
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = MySqlConfig {
//!     listen_addr: "127.0.0.1:3306".parse()?,
//!     max_connections: 1000,
//!     authentication_enabled: false,
//!     server_version: "8.0.0-Orbit".to_string(),
//!     username: None,
//!     password: None,
//! };
//!
//! let adapter = MySqlAdapter::new(config).await?;
//! adapter.start().await?;
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod auth;
pub mod packet;
pub mod protocol;
pub mod types;

pub use adapter::MySqlAdapter;
pub use protocol::{MySqlCommand, MySqlPacket};
pub use types::{MySqlType, MySqlValue};

/// MySQL configuration
#[derive(Debug, Clone)]
pub struct MySqlConfig {
    /// Address to listen on (default: 127.0.0.1:3306)
    pub listen_addr: std::net::SocketAddr,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Enable authentication
    pub authentication_enabled: bool,
    /// Server version string
    pub server_version: String,
    /// Username for authentication (if authentication_enabled is true)
    pub username: Option<String>,
    /// Password for authentication (if authentication_enabled is true)
    pub password: Option<String>,
}

impl Default for MySqlConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:3306".parse().unwrap(),
            max_connections: 1000,
            authentication_enabled: false,
            server_version: "8.0.0-Orbit".to_string(),
            username: None,
            password: None,
        }
    }
}
