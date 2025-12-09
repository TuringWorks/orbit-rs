//! OrbitWire Protocol Implementation
//!
//! Custom binary wire protocol optimized for OrbitQL features.
//! Provides low-latency, multiplexed communication for CLI and desktop clients.
//!
//! # Features
//! - Multiplexed streams for concurrent queries
//! - First-class LIVE query subscription support
//! - Optimized encodings for graph paths, vectors, and spatial data
//! - Compression support (LZ4, Zstd, Snappy)
//!
//! # Port
//! Default: 50053

mod codec;
mod frame;
mod messages;
mod server;
mod session;
mod values;

pub use codec::*;
pub use frame::*;
pub use messages::*;
pub use server::OrbitWireServer;
pub use session::OrbitWireSession;
pub use values::*;

/// Default port for OrbitWire
pub const DEFAULT_ORBITWIRE_PORT: u16 = 50053;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Magic bytes for protocol identification
pub const MAGIC_BYTES: [u8; 4] = [0x4F, 0x52, 0x42, 0x57]; // "ORBW"

/// OrbitWire server configuration
#[derive(Debug, Clone)]
pub struct OrbitWireConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Maximum frame size (default: 16MB)
    pub max_frame_size: usize,
    /// Enable TLS
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS key path
    pub tls_key_path: Option<String>,
    /// Maximum concurrent streams per connection
    pub max_streams: u32,
    /// Keep-alive interval in seconds
    pub keepalive_interval_secs: u64,
    /// Default compression
    pub default_compression: CompressionType,
    /// Enable pipelining
    pub enable_pipelining: bool,
}

impl Default for OrbitWireConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: DEFAULT_ORBITWIRE_PORT,
            max_frame_size: 16 * 1024 * 1024, // 16MB
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            max_streams: 256,
            keepalive_interval_secs: 30,
            default_compression: CompressionType::None,
            enable_pipelining: true,
        }
    }
}

/// Compression types for OrbitWire
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionType {
    #[default]
    None,
    Lz4,
    Zstd,
    Snappy,
}

impl CompressionType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::None,
            1 => Self::Lz4,
            2 => Self::Zstd,
            3 => Self::Snappy,
            _ => Self::None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Lz4 => 1,
            Self::Zstd => 2,
            Self::Snappy => 3,
        }
    }
}
