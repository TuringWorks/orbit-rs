//! MCP protocol types and definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;

/// MCP error types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", content = "message")]
pub enum McpError {
    /// Invalid request format or parameters
    InvalidRequest(String),

    /// Method not found
    MethodNotFound(String),

    /// Authentication required
    AuthenticationRequired,

    /// Permission denied
    PermissionDenied(String),

    /// Resource not found
    ResourceNotFound(String),

    /// Rate limit exceeded
    RateLimitExceeded,

    /// Internal server error
    InternalError(String),

    /// Tool execution error
    ToolError(String),

    /// SQL execution error
    SqlError(String),

    /// Actor system error
    ActorError(String),
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::InvalidRequest(msg) => write!(f, "Invalid request: {msg}"),
            McpError::MethodNotFound(method) => write!(f, "Method not found: {method}"),
            McpError::AuthenticationRequired => write!(f, "Authentication required"),
            McpError::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            McpError::ResourceNotFound(resource) => write!(f, "Resource not found: {resource}"),
            McpError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            McpError::InternalError(msg) => write!(f, "Internal error: {msg}"),
            McpError::ToolError(msg) => write!(f, "Tool error: {msg}"),
            McpError::SqlError(msg) => write!(f, "SQL error: {msg}"),
            McpError::ActorError(msg) => write!(f, "Actor error: {msg}"),
        }
    }
}

impl std::error::Error for McpError {}

/// MCP request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,

    /// Request ID
    pub id: Option<serde_json::Value>,

    /// Method name
    pub method: String,

    /// Method parameters
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// MCP response message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResponse {
    Success {
        jsonrpc: String,
        id: Option<serde_json::Value>,
        result: serde_json::Value,
    },
    Error {
        jsonrpc: String,
        id: Option<serde_json::Value>,
        error: McpError,
    },
}

impl McpResponse {
    /// Create a success response
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self::Success {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        }
    }

    /// Create an error response
    pub fn error(id: Option<serde_json::Value>, error: McpError) -> Self {
        Self::Error {
            jsonrpc: "2.0".to_string(),
            id,
            error,
        }
    }
}

/// MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name
    pub name: String,

    /// Tool description
    pub description: String,

    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,

    /// Whether the tool is dangerous (requires confirmation)
    #[serde(default)]
    pub dangerous: bool,
}

/// MCP tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Whether the tool execution was successful
    pub success: bool,

    /// Result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,

    /// Error message if execution failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Metadata about the execution
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl McpToolResult {
    /// Create a successful tool result
    pub fn success(content: serde_json::Value) -> Self {
        Self {
            success: true,
            content: Some(content),
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a successful tool result with metadata
    pub fn success_with_metadata(
        content: serde_json::Value,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            success: true,
            content: Some(content),
            error: None,
            metadata,
        }
    }

    /// Create an error tool result
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            content: None,
            error: Some(error),
            metadata: HashMap::new(),
        }
    }
}

/// MCP resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Resource URI
    pub uri: String,

    /// Resource name
    pub name: String,

    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP resource content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    /// Resource URI
    pub uri: String,

    /// MIME type
    pub mime_type: String,

    /// Resource content (can be text or binary data encoded as base64)
    #[serde(flatten)]
    pub content: McpResourceData,
}

/// MCP resource data (text or binary)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResourceData {
    Text { text: String },
    Blob { blob: String }, // base64 encoded
}

/// MCP prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    /// Prompt name
    pub name: String,

    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Prompt arguments schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// MCP prompt result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Prompt messages
    pub messages: Vec<McpPromptMessage>,
}

/// MCP prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptMessage {
    /// Message role (user, assistant, system)
    pub role: String,

    /// Message content
    pub content: McpPromptContent,
}

/// MCP prompt content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpPromptContent {
    Text {
        #[serde(rename = "type")]
        content_type: String, // "text"
        text: String,
    },
    Image {
        #[serde(rename = "type")]
        content_type: String, // "image"
        data: String, // base64 encoded
        mime_type: String,
    },
}

/// Actor query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorQueryParams {
    /// Actor ID to query
    pub actor_id: Option<String>,

    /// Actor type filter
    pub actor_type: Option<String>,

    /// State query (SQL-like filter)
    pub state_query: Option<String>,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Result offset
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

/// Actor creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorCreateParams {
    /// Actor ID (must be unique)
    pub actor_id: String,

    /// Actor type
    pub actor_type: String,

    /// Initial state (JSON object)
    #[serde(default)]
    pub initial_state: serde_json::Value,

    /// Actor configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// Actor message parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMessageParams {
    /// Target actor ID
    pub actor_id: String,

    /// Message type/method
    pub message_type: String,

    /// Message payload
    #[serde(default)]
    pub payload: serde_json::Value,

    /// Whether to wait for response
    #[serde(default)]
    pub wait_for_response: bool,

    /// Response timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

/// SQL query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlQueryParams {
    /// SQL query string
    pub query: String,

    /// Query parameters (for parameterized queries)
    #[serde(default)]
    pub parameters: Vec<serde_json::Value>,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Whether to include query plan/analysis
    #[serde(default)]
    pub include_plan: bool,
}

/// Vector search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchParams {
    /// Query vector
    pub vector: Vec<f32>,

    /// Table/collection to search
    pub table: String,

    /// Vector column name
    #[serde(default = "default_vector_column")]
    pub vector_column: String,

    /// Number of nearest neighbors
    #[serde(default = "default_k")]
    pub k: usize,

    /// Distance metric (l2, cosine, inner_product)
    #[serde(default = "default_distance_metric")]
    pub distance_metric: String,

    /// Additional WHERE clause filters
    pub filters: Option<String>,
}

fn default_vector_column() -> String {
    "embedding".to_string()
}

fn default_k() -> usize {
    10
}

fn default_distance_metric() -> String {
    "l2".to_string()
}
