//! # Orbit Protocol Adapters
//!
//! This crate provides protocol adapters that allow existing database clients
//! to interact with the Orbit distributed actor system:
//!
//! - **RESP (Redis Protocol)**: Redis-compatible commands for actor state and pub/sub
//! - **PostgreSQL Wire Protocol**: SQL-like queries for actor data
//! - **Cypher/Bolt Protocol**: Graph queries for actor relationships
//! - **REST API**: HTTP/WebSocket interface with OpenAPI documentation
//!
//! ## Architecture
//!
//! Each protocol adapter translates external protocol commands into Orbit actor
//! operations, providing a familiar interface for developers already using these
//! databases.

#![allow(missing_docs)]

pub mod aql;
pub mod arangodb;
pub mod bolt;
pub mod cypher;
pub mod error;
pub mod graph_database;
pub mod graphrag;
pub mod mcp;
pub mod ml; // New ML module
pub mod neo4j;
pub mod orbitql;
pub mod persistence;
pub mod postgres_wire;
pub mod resp;
pub mod time_series;
pub mod vector_store;

// Test utilities for comprehensive testing
#[cfg(test)]
pub mod test_utils;

pub use error::{ProtocolError, ProtocolResult};
