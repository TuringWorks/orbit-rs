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
pub mod cypher_parser;
pub mod graph_engine;
pub mod graphrag_procedures;
pub mod server;
pub mod storage;

pub use bolt::BoltProtocol;
pub use cypher_parser::CypherParser;
pub use graph_engine::GraphEngine;
pub use graphrag_procedures::BoltGraphRAGProcedures;
pub use server::CypherServer;
pub use storage::CypherGraphStorage;

// TODO: Implement full Bolt protocol
// - Bolt v4/v5 handshake
// - HELLO, LOGON messages
// - RUN, PULL, DISCARD messages
// - Cypher query parsing
// - Graph result encoding
// - Transaction support
