//! Protocol implementations and servers
//!
//! This module contains all protocol implementations that were previously in orbit_protocols.
//! It includes both the protocol logic and the server wrappers.

// Protocol implementations
pub mod aql;
pub mod arangodb;
pub mod bolt;
pub mod common;
pub mod cql;
pub mod cypher;
pub mod error;
pub mod graph_database;
pub mod graphrag;
pub mod mcp;
pub mod ml;
pub mod mysql;
pub mod neo4j;
pub mod orbitql;
pub mod persistence;
pub mod postgres_wire;
pub mod resp;
pub mod rest;
pub mod time_series;
pub mod vector_store;

// Server wrappers (these use the protocol implementations above)
mod cql_server;
mod mysql_server;
mod postgres_server;
mod resp_server;

pub use cql_server::CqlServer;
pub use mysql_server::MySqlServer;
pub use postgres_server::PostgresServer;
pub use resp_server::RespServer;

// Re-export commonly used types
pub use error::{ProtocolError, ProtocolResult};
