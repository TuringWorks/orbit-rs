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

#![warn(missing_docs)]

pub mod error;
pub mod resp;
pub mod postgres_wire;
pub mod cypher;
pub mod rest;

pub use error::{ProtocolError, ProtocolResult};
