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
// pub mod string; // TODO: fix this module
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
        connection::ConnectionCommands,
        // graph::GraphCommands, // TODO: fix
        // graphrag::GraphRAGCommands, // TODO: fix
        // string::StringCommands, // TODO: Implement string commands
        hash::HashCommands,
        list::ListCommands,
        // pubsub::PubSubCommands, // TODO: fix
        // server::ServerCommands, // TODO: fix
        set::SetCommands,
        sorted_set::SortedSetCommands,
        // time_series::TimeSeriesCommands, // TODO: fix
        // vector::VectorCommands, // TODO: fix
    };
    use crate::error::ProtocolResult;
    use crate::resp::simple_local::SimpleLocalRegistry;
    use crate::{error::ProtocolError, resp::RespValue};
    use orbit_client::OrbitClient;

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
        #[allow(dead_code)] // Reserved for future use
        orbit_client: Arc<OrbitClient>,
        #[allow(dead_code)] // Reserved for future use
        local_registry: Arc<SimpleLocalRegistry>,

        // Specialized command handlers
        connection: ConnectionCommands,
        // string: StringCommands, // TODO
        hash: HashCommands,
        list: ListCommands,
        // pubsub: PubSubCommands, // TODO: fix
        set: SetCommands,
        sorted_set: SortedSetCommands,
        // vector: VectorCommands, // TODO: fix
        // time_series: TimeSeriesCommands, // TODO: fix
        // graph: GraphCommands, // TODO: fix
        // graphrag: GraphRAGCommands, // TODO: fix
        // server: ServerCommands, // TODO: fix
    }

    impl CommandHandler {
        /// Create a new command handler with all specialized modules
        pub fn new(orbit_client: OrbitClient) -> Self {
            let orbit_client = Arc::new(orbit_client);
            let local_registry = Arc::new(SimpleLocalRegistry::new());

            Self {
                connection: ConnectionCommands::new(orbit_client.clone()),
                // string: StringCommands::new(orbit_client.clone(), local_registry.clone()), // TODO
                hash: HashCommands::new(orbit_client.clone()),
                list: ListCommands::new(orbit_client.clone()),
                // pubsub: PubSubCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                set: SetCommands::new(orbit_client.clone()),
                sorted_set: SortedSetCommands::new(orbit_client.clone()),
                // vector: VectorCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                // time_series: TimeSeriesCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                // graph: GraphCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                // graphrag: GraphRAGCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                // server: ServerCommands::new(orbit_client.clone(), local_registry.clone()), // TODO: fix
                orbit_client,
                local_registry,
            }
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
                    // CommandHandlerTrait::handle(&self.string, &command_name, &args).await // TODO
                    Err(ProtocolError::RespError(format!(
                        "ERR string commands not yet modularized: {}",
                        command_name
                    )))
                }
                CommandCategory::Hash => {
                    CommandHandlerTrait::handle(&self.hash, &command_name, &args).await
                }
                CommandCategory::List => {
                    CommandHandlerTrait::handle(&self.list, &command_name, &args).await
                }
                CommandCategory::PubSub => Err(ProtocolError::RespError(
                    "ERR PubSub commands not available".to_string(),
                )),
                CommandCategory::Set => {
                    CommandHandlerTrait::handle(&self.set, &command_name, &args).await
                }
                CommandCategory::SortedSet => {
                    CommandHandlerTrait::handle(&self.sorted_set, &command_name, &args).await
                }
                CommandCategory::Vector => Err(ProtocolError::RespError(
                    "ERR Vector commands not available".to_string(),
                )),
                CommandCategory::TimeSeries => Err(ProtocolError::RespError(
                    "ERR TimeSeries commands not available".to_string(),
                )),
                CommandCategory::Graph => Err(ProtocolError::RespError(
                    "ERR Graph commands not available".to_string(),
                )),
                CommandCategory::GraphRAG => Err(ProtocolError::RespError(
                    "ERR GraphRAG commands not available".to_string(),
                )),
                CommandCategory::Server => Err(ProtocolError::RespError(
                    "ERR Server commands not available".to_string(),
                )),
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
                | "PERSIST" | "PEXPIRE" | "PTTL" | "RANDOMKEY" | "RENAME" | "TYPE" | "UNLINK" => {
                    CommandCategory::String
                }

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
