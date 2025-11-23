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
// pub mod local_invocation;  // Disabled due to compilation issues
// pub mod server;  // Moved to orbit_server::protocols
pub mod simple_local;
pub mod spatial_commands;
pub mod types;

pub use crate::protocols::vector_store::{
    SimilarityMetric, Vector, VectorActor, VectorActorMethods, VectorIndexConfig,
    VectorSearchParams, VectorSearchResult, VectorStats,
};
pub use actors::{
    HashActor, HashActorMethods, KeyValueActor, KeyValueActorMethods, ListActor, ListActorMethods,
    PubSubActor, PubSubActorMethods,
};
pub use codec::RespCodec;
pub use commands::CommandHandler;
// pub use server::RespServer;  // Moved to orbit_server::protocols
pub use spatial_commands::{
    GeofenceDefinition, GeofenceEngine, RedisSpatialCommands, RedisValue, SpatialDataSet,
};
pub use types::{RespArray, RespValue};
// pub use local_invocation::{LocalActorRegistry, RespInvocationSystem, create_resp_orbit_client};  // Disabled
pub use simple_local::SimpleLocalRegistry;
