//! Model Context Protocol (MCP) Server for Orbit
//!
//! This module implements an MCP server that exposes Orbit's capabilities
//! to AI agents and other MCP clients. It provides tools for:
//!
//! - Actor management (create, query, send messages)
//! - SQL query execution against the actor system
//! - System monitoring and health checks
//! - Vector operations and similarity search
//!
//! ## MCP Protocol Overview
//!
//! MCP (Model Context Protocol) is a standardized protocol for AI agents
//! to interact with external systems. This implementation follows the
//! MCP specification to provide a secure and standardized interface.
//!
//! ## Security
//!
//! - All operations are authenticated and authorized
//! - Read-only operations are clearly separated from write operations
//! - SQL injection protection through parameterized queries
//! - Rate limiting and resource quotas

pub mod handlers;
pub mod server;
pub mod tools;
pub mod types;

pub use server::McpServer;
pub use types::{McpError, McpRequest, McpResponse, McpResult, McpTool};

use serde::{Deserialize, Serialize};

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server name and version
    pub name: String,
    pub version: String,

    /// Security configuration
    pub require_authentication: bool,
    pub allowed_origins: Vec<String>,

    /// Resource limits
    pub max_concurrent_requests: usize,
    pub request_timeout_ms: u64,
    pub max_query_result_size: usize,

    /// Feature flags
    pub enable_sql_queries: bool,
    pub enable_actor_management: bool,
    pub enable_vector_operations: bool,
    pub enable_system_monitoring: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            name: "orbit-mcp-server".to_string(),
            version: "0.1.0".to_string(),
            require_authentication: true,
            allowed_origins: vec!["*".to_string()],
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
            max_query_result_size: 10_000,
            enable_sql_queries: true,
            enable_actor_management: true,
            enable_vector_operations: true,
            enable_system_monitoring: true,
        }
    }
}

/// MCP server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilities {
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub prompts: Vec<String>,
}

impl Default for McpCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl McpCapabilities {
    pub fn new() -> Self {
        Self {
            tools: vec![
                "actor_create".to_string(),
                "actor_delete".to_string(),
                "actor_list".to_string(),
                "actor_search".to_string(),
                "vector_create".to_string(),
                "vector_query".to_string(),
                "vector_upsert".to_string(),
                "vector_delete".to_string(),
                "system_info".to_string(),
                "system_stats".to_string(),
            ],
            resources: vec!["memory://actors".to_string()],
            prompts: vec!["system_analysis".to_string()],
        }
    }
}
