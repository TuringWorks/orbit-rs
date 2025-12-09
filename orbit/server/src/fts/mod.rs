// Full-Text Search Engine Module
//
// This module provides full-text search capabilities using Tantivy for multiple protocols:
// - PostgreSQL: tsvector/tsquery, GIN indexes
// - MySQL: FULLTEXT indexes, MATCH() AGAINST()
// - MongoDB: Text indexes, $text operator
// - Redis: FT.CREATE, FT.SEARCH (RedisSearch compatibility)

pub mod engine;
pub mod index_manager;
pub mod query_parser;
pub mod ranking;
pub mod schema;

pub use engine::FtsEngine;
pub use index_manager::IndexManager;
pub use query_parser::QueryParser;
pub use ranking::RankingEngine;
pub use schema::FtsSchema;

use std::path::PathBuf;

/// FTS configuration
#[derive(Debug, Clone)]
pub struct FtsConfig {
    /// Index storage directory
    pub index_dir: PathBuf,
    /// Maximum memory for indexing (bytes)
    pub max_memory: usize,
    /// Number of indexing threads
    pub num_threads: usize,
    /// Default language for text analysis
    pub default_language: String,
    /// Enable query caching
    pub enable_cache: bool,
    /// Cache size (number of queries)
    pub cache_size: usize,
}

impl Default for FtsConfig {
    fn default() -> Self {
        Self {
            index_dir: PathBuf::from("./data/fts"),
            max_memory: 100_000_000, // 100MB
            num_threads: 4,          // Default to 4 threads
            default_language: "english".to_string(),
            enable_cache: true,
            cache_size: 1000,
        }
    }
}

/// FTS search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document ID
    pub doc_id: String,
    /// Relevance score (BM25)
    pub score: f32,
    /// Matched fields
    pub fields: Vec<(String, String)>,
    /// Highlighted snippets
    pub highlights: Vec<String>,
}

/// FTS index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Number of documents
    pub num_docs: u64,
    /// Index size in bytes
    pub size_bytes: u64,
    /// Number of indexed fields
    pub num_fields: usize,
    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}
