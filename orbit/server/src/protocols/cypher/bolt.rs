//! Legacy Bolt protocol stub
//!
//! **Note**: This module is deprecated. Use [`BoltProtocolHandler`](super::bolt_protocol::BoltProtocolHandler)
//! for the complete Bolt v4.4 implementation with:
//! - PackStream encoding/decoding
//! - Transaction management (BEGIN/COMMIT/ROLLBACK)
//! - Authentication (HELLO with auth token)
//! - Streaming results (RUN/PULL/DISCARD)
//!
//! This stub is kept for backwards compatibility only.

use crate::protocols::error::ProtocolResult;

/// Legacy Bolt protocol handler (deprecated)
///
/// Use [`BoltProtocolHandler`](super::bolt_protocol::BoltProtocolHandler) instead.
#[deprecated(
    since = "0.1.0",
    note = "Use BoltProtocolHandler from bolt_protocol module instead"
)]
pub struct BoltProtocol {
    version: u32,
}

#[allow(deprecated)]
impl BoltProtocol {
    /// Create a new Bolt protocol handler
    pub fn new() -> Self {
        Self { version: 0 }
    }

    /// Handle Bolt handshake
    pub async fn handshake(&mut self, _versions: &[u32]) -> ProtocolResult<u32> {
        self.version = 4; // Bolt 4.0
        Ok(self.version)
    }

    /// Get the negotiated version
    pub fn version(&self) -> u32 {
        self.version
    }
}

#[allow(deprecated)]
impl Default for BoltProtocol {
    fn default() -> Self {
        Self::new()
    }
}
