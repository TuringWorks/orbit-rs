//! Neo4j Protocol Server
//!
//! Handles Neo4j Bolt protocol connections.
//! This module provides compatibility with Neo4j's Bolt protocol and Cypher query language,
//! with enhanced spatial capabilities for graph-based spatial queries.

#[cfg(feature = "protocol-neo4j")]
pub mod bolt_server;
#[cfg(feature = "protocol-neo4j")]
pub use bolt_server::BoltServer;

#[cfg(feature = "protocol-neo4j")]
pub mod bolt_messages;
#[cfg(feature = "protocol-neo4j")]
pub mod bolt_types;
#[cfg(feature = "protocol-neo4j")]
pub mod bolt_writer;
#[cfg(feature = "protocol-neo4j")]
pub mod spatial_types;
#[cfg(feature = "protocol-neo4j")]
pub mod spatial_functions;
#[cfg(feature = "protocol-neo4j")]
pub mod graph_functions;
#[cfg(feature = "protocol-neo4j")]
pub mod schema;
#[cfg(feature = "protocol-neo4j")]
pub mod database_admin;
#[cfg(feature = "protocol-neo4j")]
pub mod security;

pub mod cypher_spatial;

pub use cypher_spatial::{
    CypherSpatialExecutor, CypherSpatialParser, CypherSpatialResult, Node, NodeId, Path,
    Relationship, RelationshipId, SpatialIndex,
};
