//! RESP (Redis Serialization Protocol) adapter for Orbit
//!
//! This module implements RESP2 and RESP3 protocols, allowing Redis clients
//! to interact with Orbit actors as if they were Redis data structures.
//!
//! ## Supported Commands
//!
//! ### Actor State Commands
//! - `GET <actor_id>` - Get actor state
//! - `SET <actor_id> <value>` - Set actor state
//! - `DEL <actor_id>` - Delete/deactivate actor
//! - `EXISTS <actor_id>` - Check if actor exists
//!
//! ### Hash Commands (for structured actor state)
//! - `HGET <actor_id> <field>`
//! - `HSET <actor_id> <field> <value>`
//! - `HGETALL <actor_id>`
//!
//! ### Pub/Sub (for actor events)
//! - `PUBLISH <channel> <message>`
//! - `SUBSCRIBE <channel>`
//! - `PSUBSCRIBE <pattern>`
//!
//! ### List Commands (for actor collections)
//! - `LPUSH <actor_id> <value>`
//! - `RPUSH <actor_id> <value>`
//! - `LRANGE <actor_id> <start> <stop>`

pub mod actors;
pub mod codec;
pub mod commands;
pub mod server;
pub mod types;

pub use actors::{KeyValueActor, HashActor, ListActor, PubSubActor};
pub use codec::RespCodec;
pub use commands::CommandHandler;
pub use server::RespServer;
pub use types::{RespValue, RespArray};
