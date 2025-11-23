//! Neo4j Protocol Adapter for Orbit-RS
//!
//! This module provides compatibility with Neo4j's Bolt protocol and Cypher query language,
//! with enhanced spatial capabilities for graph-based spatial queries.

pub mod cypher_spatial;

pub use cypher_spatial::{
    CypherSpatialExecutor, CypherSpatialParser, CypherSpatialResult, Node, NodeId, Path,
    Relationship, RelationshipId, SpatialIndex,
};
