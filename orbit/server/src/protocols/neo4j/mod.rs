//! Neo4j Protocol Server
//!
//! Handles Neo4j Bolt protocol connections.
//! This module provides compatibility with Neo4j's Bolt protocol and Cypher query language,
//! with enhanced spatial capabilities for graph-based spatial queries.

#[cfg(feature = "protocol-neo4j")]
pub mod bolt_server;
#[cfg(feature = "protocol-neo4j")]
pub use bolt_server::BoltServer;

pub mod cypher_spatial;

pub use cypher_spatial::{
    CypherSpatialExecutor, CypherSpatialParser, CypherSpatialResult, Node, NodeId, Path,
    Relationship, RelationshipId, SpatialIndex,
};
