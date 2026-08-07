//! Common functionality shared across all protocol adapters
//!
//! This module provides common storage abstractions, utilities, and types
//! that can be used by any protocol adapter (MySQL, CQL, PostgreSQL, etc.)
//!
//! ## Modules
//!
//! - `cancel`: Cancelling a statement that is already running
//! - `formatting`: Common formatting utilities for query results
//! - `fts`: Shared full-text search engine with SIMD/GPU acceleration
//! - `graph_algorithms`: Shared graph algorithms (BFS, DFS, Dijkstra, PageRank, etc.)
//! - `storage`: Common storage abstractions

pub mod cancel;
pub mod formatting;
#[cfg(feature = "fts")]
pub mod fts;
pub mod graph_algorithms;
pub mod storage;

// Re-export commonly used types
#[cfg(feature = "fts")]
pub use fts::{
    CqlQueryType, FtsDocument, FtsError, FtsQuery, FtsQueryType, FtsResult, FtsSearchResult,
    MysqlSearchMode, SharedFtsConfig, SharedFtsEngine,
};
pub use graph_algorithms::{Graph, GraphEdge, GraphNode};
