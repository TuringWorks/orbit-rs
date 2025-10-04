//! MCP server implementation

use super::{McpCapabilities, McpConfig};

/// MCP server
#[allow(dead_code)]
pub struct McpServer {
    config: McpConfig,
    capabilities: McpCapabilities,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(config: McpConfig) -> Self {
        Self {
            config,
            capabilities: McpCapabilities::new(),
        }
    }
}
