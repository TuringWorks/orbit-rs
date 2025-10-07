//! AQL (ArangoDB Query Language) support for multi-model database operations
//!
//! This module provides comprehensive AQL parsing and execution capabilities,
//! supporting document, graph, and key-value operations through a unified
//! query language interface compatible with ArangoDB.
//!
//! ## Supported AQL Features
//!
//! ### Basic Operations
//! ```aql
//! FOR doc IN users
//!   FILTER doc.age > 25
//!   RETURN doc
//! ```
//!
//! ### Graph Traversals
//! ```aql
//! FOR vertex, edge, path IN 1..3 OUTBOUND 'users/john' GRAPH 'social'
//!   RETURN {vertex, edge, path}
//! ```
//!
//! ### Document Joins
//! ```aql
//! FOR user IN users
//!   FOR post IN posts
//!     FILTER post.author == user._key
//!     RETURN {user: user.name, post: post.title}
//! ```

pub mod aql_parser;
pub mod data_model;
pub mod graphrag_engine;
pub mod query_engine;

pub use aql_parser::{AqlParser, AqlQuery};
pub use data_model::{AqlCollection, AqlDocument, AqlValue};
pub use graphrag_engine::AqlGraphRAGEngine;
pub use query_engine::{AqlQueryEngine, AqlQueryResult};

// TODO: Add more AQL features
// - Advanced aggregation functions
// - Geospatial operations
// - Full-text search integration
// - User-defined functions
// - Streaming query results
