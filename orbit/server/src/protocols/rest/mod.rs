//! REST API for Orbit actor system
//!
//! Provides HTTP/WebSocket endpoints for actor management with OpenAPI documentation.
//!
//! ## Endpoints
//!
//! ### Actor Management
//! - `GET /api/v1/actors` - List all actors
//! - `GET /api/v1/actors/{id}` - Get actor state
//! - `POST /api/v1/actors` - Create actor
//! - `PUT /api/v1/actors/{id}` - Update actor state
//! - `DELETE /api/v1/actors/{id}` - Deactivate actor
//!
//! ### Actor Invocation
//! - `POST /api/v1/actors/{id}/invoke` - Invoke actor method
//!
//! ### Transactions
//! - `POST /api/v1/transactions` - Begin transaction
//! - `POST /api/v1/transactions/{id}/commit` - Commit transaction
//! - `POST /api/v1/transactions/{id}/abort` - Abort transaction
//!
//! ### Natural Language Queries
//! - `POST /api/v1/query/natural-language` - Execute natural language query
//! - `POST /api/v1/query/generate-sql` - Generate SQL from natural language
//!
//! ### Real-time
//! - `WS /api/v1/ws/actors/{id}` - WebSocket for actor events
//! - `WS /api/v1/ws/events` - WebSocket for system events

pub mod handlers;
pub mod models;
pub mod server;
pub mod sse;
pub mod websocket;

pub use handlers::{ApiState, PaginationParams, list_actors, get_actor, create_actor, update_actor, delete_actor, invoke_actor, begin_transaction, commit_transaction, abort_transaction, health_check, openapi_spec};
pub use models::{CreateActorRequest, InvokeActorRequest, UpdateActorStateRequest, SuccessResponse, ErrorResponse, ActorInfo, BeginTransactionRequest, TransactionOperation, TransactionInfo, PagedResponse, WebSocketMessage, SubscribeRequest, NaturalLanguageQueryRequest, NaturalLanguageQueryResponse, QueryResults, VisualizationHint, QueryMetadata};
pub use server::RestApiServer;
pub use sse::{SseParams, SseMessage, handle_cdc_events, handle_query_stream, QueryStreamParams};
pub use websocket::WebSocketHandler;
