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

pub mod apoc_procedures;
pub mod bolt;
pub mod bolt_protocol;
pub mod cypher_functions;
pub mod cypher_parser;
pub mod db_procedures;
pub mod graph_algorithms_procedures;
pub mod graph_engine;
pub mod graphrag_procedures;
pub mod server;
#[cfg(feature = "storage-rocksdb")]
pub mod storage;
pub mod types;

// #[cfg(test)]
// mod tests;

#[allow(deprecated)]
pub use bolt::BoltProtocol;
pub use bolt_protocol::BoltProtocolHandler;
pub use cypher_functions::{CypherFunctions, FunctionContext};
pub use cypher_parser::{BinaryOperator, CypherParser, Expression, UnaryOperator};
pub use apoc_procedures::{ApocProcedures, is_apoc_procedure};
pub use db_procedures::DbProcedures;
pub use graph_algorithms_procedures::GraphAlgorithmProcedures;
pub use graph_engine::GraphEngine;
pub use graphrag_procedures::BoltGraphRAGProcedures;
pub use server::CypherServer;
#[cfg(feature = "storage-rocksdb")]
pub use storage::{CypherGraphStorage, CypherStorageProvider};
pub use types::{GraphNode, GraphRelationship};

// Bolt Protocol v4.4 Implementation Status (see bolt_protocol.rs):
// - [x] Bolt v4/v5 handshake
// - [x] HELLO, LOGON messages
// - [x] RUN, PULL, DISCARD messages
// - [x] Cypher query parsing
// - [x] Graph result encoding
// - [x] Transaction support (BEGIN/COMMIT/ROLLBACK)
