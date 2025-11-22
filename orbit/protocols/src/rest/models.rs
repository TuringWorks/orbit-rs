//! REST API request/response models with OpenAPI documentation

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Actor creation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateActorRequest {
    /// Actor type (e.g., "GreeterActor", "CounterActor")
    #[schema(example = "GreeterActor")]
    pub actor_type: String,

    /// Actor key (string, int32, int64, or null)
    #[schema(example = json!({"StringKey": {"key": "my-actor"}}))]
    pub key: serde_json::Value,

    /// Initial state (optional)
    #[schema(example = json!({"count": 0}))]
    pub initial_state: Option<serde_json::Value>,
}

/// Actor invocation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvokeActorRequest {
    /// Method name to invoke
    #[schema(example = "greet")]
    pub method: String,

    /// Method arguments as JSON array
    #[schema(example = json!(["World"]))]
    pub args: Vec<serde_json::Value>,

    /// Optional timeout in milliseconds
    #[schema(example = 5000)]
    pub timeout_ms: Option<u64>,
}

/// Actor state update request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateActorStateRequest {
    /// New state as JSON object
    #[schema(example = json!({"count": 42, "message": "updated"}))]
    pub state: serde_json::Value,

    /// Optional merge strategy: "replace" or "merge"
    #[schema(example = "merge")]
    pub strategy: Option<String>,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SuccessResponse<T> {
    /// Success flag
    pub success: bool,

    /// Response data
    pub data: T,

    /// Optional message
    pub message: Option<String>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error flag
    pub error: bool,

    /// Error code
    #[schema(example = "ACTOR_NOT_FOUND")]
    pub code: String,

    /// Human-readable error message
    #[schema(example = "Actor with ID 'my-actor' not found")]
    pub message: String,

    /// Optional error details
    pub details: Option<serde_json::Value>,
}

/// Actor information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActorInfo {
    /// Actor type
    pub actor_type: String,

    /// Actor key
    pub key: serde_json::Value,

    /// Actor state
    pub state: serde_json::Value,

    /// Node ID where actor is hosted
    pub node_id: Option<String>,

    /// Actor status: "active", "inactive", "deactivating"
    #[schema(example = "active")]
    pub status: String,

    /// Last activity timestamp (ISO 8601)
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_activity: Option<String>,
}

/// Transaction creation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BeginTransactionRequest {
    /// Optional transaction timeout in milliseconds
    #[schema(example = 30000)]
    pub timeout_ms: Option<u64>,

    /// Optional transaction metadata
    pub metadata: Option<serde_json::Value>,
}

/// Transaction operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionOperation {
    /// Target actor type
    pub actor_type: String,

    /// Target actor key
    pub key: serde_json::Value,

    /// Method to invoke
    #[schema(example = "debit")]
    pub method: String,

    /// Method arguments
    #[schema(example = json!([100]))]
    pub args: Vec<serde_json::Value>,

    /// Optional compensation data
    pub compensation: Option<serde_json::Value>,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    /// Transaction ID
    pub transaction_id: String,

    /// Transaction status: "preparing", "prepared", "committing", "committed", "aborting", "aborted"
    #[schema(example = "committed")]
    pub status: String,

    /// Operations in this transaction
    pub operations: Vec<TransactionOperation>,

    /// Creation timestamp
    pub created_at: String,

    /// Completion timestamp (if completed)
    pub completed_at: Option<String>,
}

/// Paginated list response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PagedResponse<T> {
    /// Items in current page
    pub items: Vec<T>,

    /// Total number of items
    pub total: usize,

    /// Current page number (0-indexed)
    pub page: usize,

    /// Page size
    pub page_size: usize,

    /// Total number of pages
    pub total_pages: usize,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// Actor state changed
    ActorStateChanged {
        actor_type: String,
        key: serde_json::Value,
        state: serde_json::Value,
    },

    /// Actor activated
    ActorActivated {
        actor_type: String,
        key: serde_json::Value,
        node_id: String,
    },

    /// Actor deactivated
    ActorDeactivated {
        actor_type: String,
        key: serde_json::Value,
    },

    /// Transaction event
    TransactionEvent {
        transaction_id: String,
        status: String,
        message: Option<String>,
    },

    /// System event
    SystemEvent {
        event_type: String,
        data: serde_json::Value,
    },

    /// Subscription acknowledgment
    SubscriptionAck {
        subscription_id: String,
        filters: Vec<String>,
    },

    /// Error message
    Error { code: String, message: String },
}

/// WebSocket subscription request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscribeRequest {
    /// Event types to subscribe to
    pub event_types: Vec<String>,

    /// Optional filters (actor_type, key patterns, etc.)
    pub filters: Option<serde_json::Value>,
}

/// Natural language query request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NaturalLanguageQueryRequest {
    /// Natural language query
    #[schema(example = "Show me all users from California")]
    pub query: String,

    /// Maximum number of results to return
    #[schema(example = 100)]
    pub limit: Option<usize>,

    /// Maximum number of preview rows
    #[schema(example = 10)]
    pub max_preview_rows: Option<usize>,

    /// Whether to execute the query or just generate SQL
    #[schema(example = true)]
    pub execute: Option<bool>,
}

/// Natural language query response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NaturalLanguageQueryResponse {
    /// Generated SQL query
    pub sql: String,

    /// Query parameters (for parameterized queries)
    pub parameters: Vec<serde_json::Value>,

    /// Query type (read, write, analysis)
    pub query_type: String,

    /// Estimated complexity
    pub complexity: String,

    /// Optimization hints
    pub optimization_hints: Vec<String>,

    /// Query results (if executed)
    pub results: Option<QueryResults>,

    /// Processing metadata
    pub metadata: QueryMetadata,
}

/// Query results
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryResults {
    /// Human-readable summary
    pub summary: String,

    /// Data preview (first N rows)
    pub data_preview: Vec<serde_json::Value>,

    /// Total row count
    pub total_rows: usize,

    /// Whether full result is available in preview
    pub full_result_available: bool,

    /// Continuation token for pagination
    pub continuation_token: Option<String>,

    /// Statistical summary
    pub statistics: Option<serde_json::Value>,

    /// Visualization hints
    pub visualization_hints: Vec<VisualizationHint>,
}

/// Visualization hint
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VisualizationHint {
    /// Visualization type
    pub viz_type: String,

    /// Recommended columns
    pub columns: Vec<String>,

    /// Description
    pub description: String,
}

/// Query metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryMetadata {
    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// NLP processing time in milliseconds
    pub nlp_time_ms: Option<u64>,

    /// SQL generation time in milliseconds
    pub sql_generation_time_ms: Option<u64>,

    /// Query execution time in milliseconds (if executed)
    pub execution_time_ms: Option<u64>,

    /// Result processing time in milliseconds (if executed)
    pub result_processing_time_ms: Option<u64>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

impl<T: Serialize> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: Some(message.into()),
        }
    }
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: true,
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error: true,
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

impl<T> PagedResponse<T> {
    pub fn new(items: Vec<T>, total: usize, page: usize, page_size: usize) -> Self {
        let total_pages = total.div_ceil(page_size);
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}
