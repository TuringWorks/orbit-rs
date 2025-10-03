//! PostgreSQL Wire Protocol adapter for Orbit
//!
//! This module implements the PostgreSQL wire protocol, allowing psql and other
//! PostgreSQL clients to query Orbit actor state using SQL-like syntax.
//!
//! ## Supported Operations
//!
//! ### Actor Queries
//! ```sql
//! SELECT * FROM actors WHERE actor_id = 'user:123';
//! SELECT state FROM actors WHERE actor_type = 'UserActor';
//! ```
//!
//! ### Actor State Updates
//! ```sql
//! UPDATE actors SET state = '{"balance": 1000}' WHERE actor_id = 'account:456';
//! ```
//!
//! ### Actor Creation
//! ```sql
//! INSERT INTO actors (actor_id, actor_type, state) VALUES ('user:789', 'UserActor', '{}');
//! ```
//!
//! ## Features
//! - ✅ Startup message handling
//! - ✅ Trust authentication (no password)
//! - ✅ Simple query protocol
//! - ✅ Extended query protocol (Parse, Bind, Execute)
//! - ✅ Result set encoding (DataRow, RowDescription)
//! - ✅ Prepared statements
//! - ✅ SQL parsing (SELECT, INSERT, UPDATE, DELETE)

pub mod messages;
pub mod protocol;
pub mod server;
pub mod query_engine;

pub use messages::{BackendMessage, FrontendMessage, FieldDescription, TransactionStatus};
pub use protocol::PostgresWireProtocol;
pub use server::PostgresServer;
pub use query_engine::{QueryEngine, QueryResult};
