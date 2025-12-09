//! Arrow Flight SQL Protocol Implementation
//!
//! This module provides Arrow Flight SQL support for OrbitQL queries,
//! enabling high-performance columnar data transport over gRPC.
//!
//! # Features
//! - Zero-copy data transfer with Apache Arrow
//! - Streaming support for large result sets
//! - LIVE query subscriptions via DoExchange
//! - Prepared statement support
//! - Transaction management
//!
//! # Port
//! Default: 50052

mod codec;
mod messages;
mod server;
mod session;
mod types;

pub use codec::*;
pub use messages::*;
pub use server::FlightSqlServer;
pub use session::FlightSession;
pub use types::*;

/// Default port for Arrow Flight SQL
pub const DEFAULT_FLIGHT_PORT: u16 = 50052;

/// Flight SQL server configuration
#[derive(Debug, Clone)]
pub struct FlightConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Maximum message size (default: 64MB)
    pub max_message_size: usize,
    /// Enable TLS
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS key path
    pub tls_key_path: Option<String>,
    /// Maximum concurrent streams per connection
    pub max_concurrent_streams: u32,
    /// Keep-alive interval in seconds
    pub keepalive_interval_secs: u64,
}

impl Default for FlightConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: DEFAULT_FLIGHT_PORT,
            max_message_size: 64 * 1024 * 1024, // 64MB
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            max_concurrent_streams: 100,
            keepalive_interval_secs: 60,
        }
    }
}
