//! RESP protocol command modules
//!
//! Splits the large command handler into focused modules for better maintainability

pub mod connection;
pub mod graph;
pub mod graphrag;
pub mod hash;
pub mod list;
pub mod pubsub;
pub mod server;
pub mod set;
pub mod sorted_set;
pub mod string;
pub mod string_persistent;
// pub mod string_simple; // Replaced by full string implementation
pub mod time_series;
pub mod traits;
pub mod vector;

// Re-export the main command handler
pub use self::handler::CommandHandler;

mod handler {
    use super::traits::CommandHandler as CommandHandlerTrait;
    use std::sync::Arc;
    use tracing::{debug, warn};

    use super::{
        connection::ConnectionCommands, graph::GraphCommands, graphrag::GraphRAGCommands,
        hash::HashCommands, list::ListCommands, pubsub::PubSubCommands, server::ServerCommands,
        set::SetCommands, sorted_set::SortedSetCommands, string::StringCommands,
        time_series::TimeSeriesCommands, vector::VectorCommands,
    };
    use crate::protocols::error::ProtocolResult;
    use crate::protocols::resp::simple_local::SimpleLocalRegistry;
    use crate::protocols::{error::ProtocolError, resp::RespValue};

    /// Command categories for organizing command dispatch
    #[derive(Debug, Clone)]
    pub enum CommandCategory {
        Connection,
        String,
        Hash,
        List,
        PubSub,
        Set,
        SortedSet,
        Vector,
        TimeSeries,
        Graph,
        GraphRAG,
        Server,
        Unknown,
    }

    /// Main command handler that delegates to specialized command modules
    pub struct CommandHandler {
        local_registry: Arc<SimpleLocalRegistry>,

        // Specialized command handlers
        connection: ConnectionCommands,
        string: StringCommands,
        hash: HashCommands,
        list: ListCommands,
        pubsub: PubSubCommands,
        set: SetCommands,
        sorted_set: SortedSetCommands,
        vector: VectorCommands,
        time_series: TimeSeriesCommands,
        graph: GraphCommands,
        graphrag: GraphRAGCommands,
        server: ServerCommands,
    }

    impl CommandHandler {
        /// Create a new command handler with all specialized modules
        pub fn new() -> Self {
            Self::new_with_persistence(None)
        }

        /// Create a new command handler with optional persistent storage
        pub fn new_with_persistence(
            persistent_storage: Option<
                Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider>,
            >,
        ) -> Self {
            let local_registry = if let Some(provider) = persistent_storage {
                Arc::new(SimpleLocalRegistry::with_persistence(provider))
            } else {
                Arc::new(SimpleLocalRegistry::new())
            };

            Self {
                connection: ConnectionCommands::new(local_registry.clone()),
                string: StringCommands::new(local_registry.clone()),
                hash: HashCommands::new(local_registry.clone()),
                list: ListCommands::new(local_registry.clone()),
                pubsub: PubSubCommands::new(local_registry.clone()),
                set: SetCommands::new(local_registry.clone()),
                sorted_set: SortedSetCommands::new(local_registry.clone()),
                vector: VectorCommands::new(local_registry.clone()),
                time_series: TimeSeriesCommands::new(local_registry.clone()),
                graph: GraphCommands::new(local_registry.clone()),
                graphrag: GraphRAGCommands::new(local_registry.clone()),
                server: ServerCommands::new(local_registry.clone()),
                local_registry,
            }
        }

        /// Load data from persistent storage on startup
        pub async fn load_from_persistence(&self) {
            self.local_registry
                .load_from_persistence()
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to load data from persistent storage: {}", e);
                });
        }

        /// Handle a RESP command by delegating to the appropriate module
        pub async fn handle_command(&self, command: RespValue) -> ProtocolResult<RespValue> {
            let (command_name, args) = self.parse_command(command)?;
            let category = self.get_command_category(&command_name);

            debug!(
                "Executing command: {} (category: {:?}) with {} args",
                command_name,
                category,
                args.len()
            );

            match category {
                CommandCategory::Connection => {
                    CommandHandlerTrait::handle(&self.connection, &command_name, &args).await
                }
                CommandCategory::String => {
                    CommandHandlerTrait::handle(&self.string, &command_name, &args).await
                }
                CommandCategory::Hash => {
                    CommandHandlerTrait::handle(&self.hash, &command_name, &args).await
                }
                CommandCategory::List => {
                    CommandHandlerTrait::handle(&self.list, &command_name, &args).await
                }
                CommandCategory::PubSub => {
                    CommandHandlerTrait::handle(&self.pubsub, &command_name, &args).await
                }
                CommandCategory::Set => {
                    CommandHandlerTrait::handle(&self.set, &command_name, &args).await
                }
                CommandCategory::SortedSet => {
                    CommandHandlerTrait::handle(&self.sorted_set, &command_name, &args).await
                }
                CommandCategory::Vector => {
                    CommandHandlerTrait::handle(&self.vector, &command_name, &args).await
                }
                CommandCategory::TimeSeries => {
                    CommandHandlerTrait::handle(&self.time_series, &command_name, &args).await
                }
                CommandCategory::Graph => {
                    CommandHandlerTrait::handle(&self.graph, &command_name, &args).await
                }
                CommandCategory::GraphRAG => {
                    CommandHandlerTrait::handle(&self.graphrag, &command_name, &args).await
                }
                CommandCategory::Server => {
                    CommandHandlerTrait::handle(&self.server, &command_name, &args).await
                }
                CommandCategory::Unknown => {
                    warn!("Unknown command: {}", command_name);
                    Err(ProtocolError::RespError(format!(
                        "ERR unknown command '{command_name}'"
                    )))
                }
            }
        }

        /// Parse command and extract name and arguments
        fn parse_command(&self, command: RespValue) -> ProtocolResult<(String, Vec<RespValue>)> {
            let args = match command {
                RespValue::Array(args) => args,
                _ => {
                    return Err(ProtocolError::RespError(
                        "Command must be an array".to_string(),
                    ))
                }
            };

            if args.is_empty() {
                return Err(ProtocolError::RespError("Empty command".to_string()));
            }

            let command_name = args[0]
                .as_string()
                .ok_or_else(|| {
                    ProtocolError::RespError("Command name must be a string".to_string())
                })?
                .to_uppercase();

            Ok((command_name, args[1..].to_vec()))
        }

        /// Categorize command for dispatch
        fn get_command_category(&self, command_name: &str) -> CommandCategory {
            match command_name {
                // Connection commands
                "PING" | "ECHO" | "SELECT" | "AUTH" | "QUIT" => CommandCategory::Connection,

                // String/Key commands
                "GET" | "SET" | "DEL" | "EXISTS" | "TTL" | "EXPIRE" | "KEYS" | "APPEND"
                | "GETRANGE" | "GETSET" | "MGET" | "MSET" | "SETEX" | "SETRANGE" | "STRLEN"
                | "INCR" | "DECR" | "INCRBY" | "DECRBY" | "SETNX" | "PERSIST" | "PEXPIRE"
                | "PTTL" | "RANDOMKEY" | "RENAME" | "TYPE" | "UNLINK" => CommandCategory::String,

                // Hash commands
                "HGET" | "HSET" | "HGETALL" | "HMGET" | "HMSET" | "HDEL" | "HEXISTS" | "HKEYS"
                | "HVALS" | "HLEN" | "HINCRBY" => CommandCategory::Hash,

                // List commands
                "LPUSH" | "RPUSH" | "LPOP" | "RPOP" | "LRANGE" | "LLEN" | "LINDEX" | "LSET"
                | "LREM" | "LTRIM" | "LINSERT" | "BLPOP" | "BRPOP" => CommandCategory::List,

                // Pub/Sub commands
                "PUBLISH" | "SUBSCRIBE" | "UNSUBSCRIBE" | "PSUBSCRIBE" | "PUNSUBSCRIBE"
                | "PUBSUB" => CommandCategory::PubSub,

                // Set commands
                "SADD" | "SREM" | "SMEMBERS" | "SCARD" | "SISMEMBER" | "SUNION" | "SINTER"
                | "SDIFF" => CommandCategory::Set,

                // Sorted Set commands
                "ZADD" | "ZREM" | "ZCARD" | "ZSCORE" | "ZINCRBY" | "ZRANGE" | "ZRANGEBYSCORE"
                | "ZCOUNT" | "ZRANK" => CommandCategory::SortedSet,

                // Vector commands
                cmd if cmd.starts_with("VECTOR.") || cmd.starts_with("FT.") => {
                    CommandCategory::Vector
                }

                // Time Series commands
                cmd if cmd.starts_with("TS.") => CommandCategory::TimeSeries,

                // Graph commands
                cmd if cmd.starts_with("GRAPH.") => CommandCategory::Graph,

                // GraphRAG commands
                cmd if cmd.starts_with("GRAPHRAG.") => CommandCategory::GraphRAG,

                // Server commands
                "INFO" | "DBSIZE" | "FLUSHDB" | "FLUSHALL" | "COMMAND" => CommandCategory::Server,

                _ => CommandCategory::Unknown,
            }
        }
    }
}
