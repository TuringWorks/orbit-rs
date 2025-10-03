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
//! ### Real-time
//! - `WS /api/v1/ws/actors/{id}` - WebSocket for actor events
//! - `WS /api/v1/ws/events` - WebSocket for system events

pub mod handlers;
pub mod models;
pub mod server;
pub mod websocket;

pub use handlers::*;
pub use models::*;
pub use server::RestApiServer;
pub use websocket::WebSocketHandler;
