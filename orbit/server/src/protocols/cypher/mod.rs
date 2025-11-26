//! Cypher/Bolt Protocol adapter for Neo4j compatibility
//!
//! This module implements the Bolt protocol and Cypher query language support,
//! allowing Neo4j drivers to query actor relationships as a graph.
//!
//! ## Supported Cypher Queries
//!
//! ### Actor Relationship Queries
//! ```cypher
//! MATCH (a:Actor {id: 'user:123'})-[:CALLS]->(b:Actor)
//! RETURN a, b
//! ```
//!
//! ### Pattern Matching
//! ```cypher
//! MATCH (a:Actor)-[:SUPERVISES*1..3]->(b:Actor)
//! WHERE a.type = 'SupervisorActor'
//! RETURN a.id, b.id
//! ```

pub mod bolt;
pub mod bolt_protocol;
pub mod cypher_parser;
pub mod graph_engine;
pub mod graphrag_procedures;
pub mod server;
#[cfg(feature = "storage-rocksdb")]
pub mod storage;
pub mod types;

// #[cfg(test)]
// mod tests;

pub use bolt::BoltProtocol;
pub use bolt_protocol::BoltProtocolHandler;
pub use cypher_parser::CypherParser;
pub use graph_engine::GraphEngine;
pub use graphrag_procedures::BoltGraphRAGProcedures;
pub use server::CypherServer;
#[cfg(feature = "storage-rocksdb")]
pub use storage::CypherGraphStorage;
pub use types::{GraphNode, GraphRelationship};

// TODO: Implement full Bolt protocol
// - Bolt v4/v5 handshake
// - HELLO, LOGON messages
// - RUN, PULL, DISCARD messages
// - Cypher query parsing
// - Graph result encoding
// - Transaction support
