//! Shared Full-Text Search Module
//!
//! This module provides protocol-agnostic full-text search capabilities that can be used
//! by all OrbitRS protocols (PostgreSQL, MySQL, CQL, Redis, MongoDB, OrbitQL).
//!
//! ## Features
//!
//! - **Unified Text Processing**: SIMD-accelerated tokenization and stemming
//! - **BM25 Ranking**: Standard relevance scoring with GPU acceleration support
//! - **Protocol Adapters**: Translation layers for protocol-specific query syntax
//! - **Index Management**: Create, update, and delete FTS indexes
//! - **Dual Engine Support**: Choose between SharedFtsEngine (SIMD/in-memory) or TantivyFtsEngine (disk-backed)
//!
//! ## Performance
//!
//! - SIMD (AVX2/AVX-512/NEON) for text processing operations
//! - GPU acceleration for large-scale BM25 scoring and batch operations
//! - In-memory inverted index OR disk-backed Tantivy index (configurable)
//!
//! ## Protocol Integration
//!
//! | Protocol | Query Syntax | Index Type |
//! |----------|-------------|------------|
//! | PostgreSQL | tsvector @@ tsquery | GIN |
//! | MySQL | MATCH() AGAINST() | FULLTEXT |
//! | CQL | CONTAINS, LIKE | SASI/SAI |
//! | Redis | FT.SEARCH | RedisSearch |
//! | MongoDB | $text | Text Index |
//! | OrbitQL | SEARCH() | FTS Index |
//!
//! ## Engine Selection
//!
//! Configure in `orbit-server.toml`:
//! ```toml
//! [fts]
//! engine = "shared"  # "shared" (SIMD/in-memory) or "tantivy" (disk-backed)
//! ```

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result type for FTS operations
pub type FtsResult<T> = Result<T, FtsError>;

/// FTS error types
#[derive(Debug, Clone)]
pub enum FtsError {
    /// Index not found
    IndexNotFound(String),
    /// Invalid query syntax
    InvalidQuery(String),
    /// Tokenization error
    TokenizationError(String),
    /// IO error
    IoError(String),
    /// Persistence error
    PersistenceError(String),
    /// Internal error
    InternalError(String),
}

impl std::fmt::Display for FtsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FtsError::IndexNotFound(name) => write!(f, "Index not found: {}", name),
            FtsError::InvalidQuery(msg) => write!(f, "Invalid query: {}", msg),
            FtsError::TokenizationError(msg) => write!(f, "Tokenization error: {}", msg),
            FtsError::IoError(msg) => write!(f, "IO error: {}", msg),
            FtsError::PersistenceError(msg) => write!(f, "Persistence error: {}", msg),
            FtsError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for FtsError {}

impl From<std::io::Error> for FtsError {
    fn from(err: std::io::Error) -> Self {
        FtsError::IoError(err.to_string())
    }
}

// ============================================================================
// FTS Engine Type Selection
// ============================================================================

/// The type of FTS engine to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtsEngineType {
    /// SharedFtsEngine: Custom SIMD-accelerated in-memory engine
    /// - Fastest tokenization (AVX2/NEON)
    /// - GPU-accelerated BM25 scoring
    /// - Best for: High-throughput, low-latency workloads
    #[default]
    Shared,
    /// TantivyFtsEngine: Disk-backed Tantivy-based engine
    /// - Persistent indexes on disk
    /// - Battle-tested Lucene-like implementation
    /// - Best for: Large indexes, durability requirements
    Tantivy,
}

impl std::str::FromStr for FtsEngineType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shared" | "simd" | "memory" | "in-memory" => Ok(FtsEngineType::Shared),
            "tantivy" | "disk" | "persistent" => Ok(FtsEngineType::Tantivy),
            _ => Err(format!(
                "Unknown FTS engine type: {}. Use 'shared' or 'tantivy'",
                s
            )),
        }
    }
}

impl std::fmt::Display for FtsEngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FtsEngineType::Shared => write!(f, "shared"),
            FtsEngineType::Tantivy => write!(f, "tantivy"),
        }
    }
}

// ============================================================================
// Unified FTS Engine Trait
// ============================================================================

/// Unified search result returned by all FTS engines
#[derive(Debug, Clone)]
pub struct UnifiedSearchResult {
    /// Document ID
    pub doc_id: String,
    /// BM25 relevance score
    pub score: f32,
    /// Matched field values
    pub fields: HashMap<String, String>,
    /// Highlighted snippets (if supported)
    pub highlights: Vec<String>,
}

/// Unified FTS engine trait that both SharedFtsEngine and TantivyFtsEngine implement
#[async_trait]
pub trait UnifiedFtsEngine: Send + Sync {
    /// Get the engine type
    fn engine_type(&self) -> FtsEngineType;

    /// Create a new index
    async fn create_index(&self, name: &str, fields: &[String]) -> FtsResult<()>;

    /// Drop an index
    async fn drop_index(&self, name: &str) -> FtsResult<bool>;

    /// Index a document
    async fn index_document(
        &self,
        index_name: &str,
        doc_id: &str,
        fields: HashMap<String, String>,
    ) -> FtsResult<()>;

    /// Remove a document from an index
    async fn remove_document(&self, index_name: &str, doc_id: &str) -> FtsResult<bool>;

    /// Search an index with a unified query
    async fn search(
        &self,
        index_name: &str,
        query: &FtsQuery,
    ) -> FtsResult<Vec<UnifiedSearchResult>>;

    /// Commit pending changes (for engines that support it)
    async fn commit(&self, index_name: &str) -> FtsResult<()>;

    /// Get index statistics
    async fn get_stats(&self, index_name: &str) -> FtsResult<UnifiedIndexStats>;

    /// List all indexes
    async fn list_indexes(&self) -> Vec<String>;

    /// Flush indexes to disk (if supported)
    async fn flush(&self) -> FtsResult<()>;
}

/// Unified index statistics
#[derive(Debug, Clone)]
pub struct UnifiedIndexStats {
    /// Number of documents in the index
    pub num_docs: u64,
    /// Number of unique terms
    pub num_terms: u64,
    /// Average document length
    pub avg_doc_len: f32,
    /// Index size in bytes (0 for in-memory only)
    pub size_bytes: u64,
}

// ============================================================================
// Core Data Structures
// ============================================================================

/// Configuration for the shared FTS engine
#[derive(Debug, Clone)]
pub struct SharedFtsConfig {
    /// Default language for text analysis
    pub default_language: String,
    /// Enable stemming
    pub enable_stemming: bool,
    /// Minimum term length for indexing
    pub min_term_length: usize,
    /// Maximum term length for indexing
    pub max_term_length: usize,
    /// Stop words to filter
    pub stop_words: HashSet<String>,
    /// BM25 k1 parameter (term frequency saturation)
    pub bm25_k1: f32,
    /// BM25 b parameter (document length normalization)
    pub bm25_b: f32,
    /// Enable SIMD acceleration for text processing
    pub enable_simd: bool,
    /// Enable GPU acceleration for scoring
    pub enable_gpu: bool,
    /// Enable disk persistence for indexes
    pub enable_persistence: bool,
    /// Directory for persisted indexes (if persistence is enabled)
    pub persistence_dir: Option<PathBuf>,
    /// Auto-flush interval in seconds (0 = manual flush only)
    pub auto_flush_secs: u64,
}

impl Default for SharedFtsConfig {
    fn default() -> Self {
        Self {
            default_language: "english".to_string(),
            enable_stemming: true,
            min_term_length: 2,
            max_term_length: 100,
            stop_words: default_stop_words(),
            bm25_k1: 1.2,
            bm25_b: 0.75,
            enable_simd: true,
            enable_gpu: false,
            enable_persistence: false,
            persistence_dir: None,
            auto_flush_secs: 0,
        }
    }
}

/// Unified FTS configuration that applies to all engines
#[derive(Debug, Clone)]
pub struct UnifiedFtsConfig {
    /// Which FTS engine to use
    pub engine_type: FtsEngineType,
    /// Directory for FTS index storage
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
    /// Enable SIMD acceleration (SharedFtsEngine only)
    pub enable_simd: bool,
    /// Enable GPU acceleration (SharedFtsEngine only)
    pub enable_gpu: bool,
    /// BM25 k1 parameter
    pub bm25_k1: f32,
    /// BM25 b parameter
    pub bm25_b: f32,
}

impl Default for UnifiedFtsConfig {
    fn default() -> Self {
        Self {
            engine_type: FtsEngineType::Shared,
            index_dir: PathBuf::from("./data/fts"),
            max_memory: 100_000_000, // 100MB
            num_threads: 4,
            default_language: "english".to_string(),
            enable_cache: true,
            cache_size: 1000,
            enable_simd: true,
            enable_gpu: false,
            bm25_k1: 1.2,
            bm25_b: 0.75,
        }
    }
}

/// A document in the FTS index
#[derive(Debug, Clone)]
pub struct FtsDocument {
    /// Unique document identifier
    pub id: String,
    /// Field name to text content mapping
    pub fields: HashMap<String, String>,
    /// Optional metadata (for protocol-specific data)
    pub metadata: Option<serde_json::Value>,
}

/// Result from an FTS search
#[derive(Debug, Clone)]
pub struct FtsSearchResult {
    /// Document ID
    pub doc_id: String,
    /// BM25 relevance score
    pub score: f32,
    /// Matched field values
    pub fields: HashMap<String, String>,
    /// Term frequency for matched terms
    pub term_frequencies: HashMap<String, u32>,
    /// Snippet highlights (if requested)
    pub highlights: Vec<String>,
}

/// FTS query with protocol-agnostic representation
#[derive(Debug, Clone)]
pub struct FtsQuery {
    /// Query type
    pub query_type: FtsQueryType,
    /// Terms to search (positive match)
    pub must_terms: Vec<String>,
    /// Terms that should match (boost)
    pub should_terms: Vec<String>,
    /// Terms to exclude
    pub must_not_terms: Vec<String>,
    /// Phrase queries (exact sequence)
    pub phrases: Vec<Vec<String>>,
    /// Fields to search (None = all fields)
    pub fields: Option<Vec<String>>,
    /// Maximum results to return
    pub limit: usize,
    /// Results to skip
    pub offset: usize,
    /// Minimum score threshold
    pub min_score: Option<f32>,
}

/// Query type variants
#[derive(Debug, Clone, PartialEq)]
pub enum FtsQueryType {
    /// Natural language query (all terms are optional)
    Natural,
    /// Boolean query with explicit operators
    Boolean,
    /// Phrase query (exact match)
    Phrase,
    /// Prefix query (term*)
    Prefix,
    /// Fuzzy query (term~)
    Fuzzy { distance: u8 },
    /// Wildcard query (te?t, t*st)
    Wildcard,
}

impl Default for FtsQuery {
    fn default() -> Self {
        Self {
            query_type: FtsQueryType::Natural,
            must_terms: Vec::new(),
            should_terms: Vec::new(),
            must_not_terms: Vec::new(),
            phrases: Vec::new(),
            fields: None,
            limit: 100,
            offset: 0,
            min_score: None,
        }
    }
}

// ============================================================================
// SIMD-Accelerated Text Processing
// ============================================================================

/// SIMD-accelerated text processor
pub struct TextProcessor {
    config: SharedFtsConfig,
}

impl TextProcessor {
    pub fn new(config: SharedFtsConfig) -> Self {
        Self { config }
    }

    /// Tokenize text with SIMD acceleration
    ///
    /// Uses SIMD to find word boundaries faster than character-by-character iteration.
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        if self.config.enable_simd && text.len() >= 64 {
            self.tokenize_simd(text)
        } else {
            self.tokenize_scalar(text)
        }
    }

    /// Scalar tokenization fallback
    fn tokenize_scalar(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| {
                !s.is_empty()
                    && s.len() >= self.config.min_term_length
                    && s.len() <= self.config.max_term_length
                    && !self.config.stop_words.contains(*s)
            })
            .map(|s| {
                if self.config.enable_stemming {
                    self.stem_word(s)
                } else {
                    s.to_string()
                }
            })
            .collect()
    }

    /// SIMD-accelerated tokenization using batch processing
    #[cfg(target_arch = "x86_64")]
    fn tokenize_simd(&self, text: &str) -> Vec<String> {
        // For x86_64, we use AVX2 if available
        if is_x86_feature_detected!("avx2") {
            self.tokenize_avx2(text)
        } else {
            self.tokenize_scalar(text)
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn tokenize_simd(&self, text: &str) -> Vec<String> {
        // ARM NEON is always available on aarch64
        self.tokenize_neon(text)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    fn tokenize_simd(&self, text: &str) -> Vec<String> {
        self.tokenize_scalar(text)
    }

    /// AVX2 tokenization - find word boundaries using SIMD
    #[cfg(target_arch = "x86_64")]
    fn tokenize_avx2(&self, text: &str) -> Vec<String> {
        use std::arch::x86_64::*;

        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut tokens = Vec::new();
        let mut word_start: Option<usize> = None;

        // Process 32 bytes at a time with AVX2
        let mut i = 0;

        // SIMD boundary detection for ASCII alphanumeric
        unsafe {
            // Characters that are NOT word characters (spaces, punctuation)
            let space = _mm256_set1_epi8(b' ' as i8);
            let zero = _mm256_set1_epi8(b'0' as i8);
            let nine = _mm256_set1_epi8(b'9' as i8);
            let upper_a = _mm256_set1_epi8(b'A' as i8);
            let upper_z = _mm256_set1_epi8(b'Z' as i8);
            let lower_a = _mm256_set1_epi8(b'a' as i8);
            let lower_z = _mm256_set1_epi8(b'z' as i8);

            while i + 32 <= len {
                let chunk = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);

                // Check if characters are alphanumeric
                let is_digit = _mm256_and_si256(
                    _mm256_cmpgt_epi8(chunk, _mm256_sub_epi8(zero, _mm256_set1_epi8(1))),
                    _mm256_cmpgt_epi8(_mm256_add_epi8(nine, _mm256_set1_epi8(1)), chunk),
                );
                let is_upper = _mm256_and_si256(
                    _mm256_cmpgt_epi8(chunk, _mm256_sub_epi8(upper_a, _mm256_set1_epi8(1))),
                    _mm256_cmpgt_epi8(_mm256_add_epi8(upper_z, _mm256_set1_epi8(1)), chunk),
                );
                let is_lower = _mm256_and_si256(
                    _mm256_cmpgt_epi8(chunk, _mm256_sub_epi8(lower_a, _mm256_set1_epi8(1))),
                    _mm256_cmpgt_epi8(_mm256_add_epi8(lower_z, _mm256_set1_epi8(1)), chunk),
                );

                let is_alnum = _mm256_or_si256(_mm256_or_si256(is_digit, is_upper), is_lower);
                let mask = _mm256_movemask_epi8(is_alnum) as u32;

                // Process each byte in this chunk based on the mask
                for j in 0..32 {
                    let is_word_char = (mask >> j) & 1 != 0;
                    let pos = i + j;

                    if is_word_char {
                        if word_start.is_none() {
                            word_start = Some(pos);
                        }
                    } else if let Some(start) = word_start {
                        self.extract_token(text, start, pos, &mut tokens);
                        word_start = None;
                    }
                }

                i += 32;
            }
        }

        // Handle remaining bytes
        while i < len {
            let c = bytes[i] as char;
            let is_word_char = c.is_alphanumeric();

            if is_word_char {
                if word_start.is_none() {
                    word_start = Some(i);
                }
            } else if let Some(start) = word_start {
                self.extract_token(text, start, i, &mut tokens);
                word_start = None;
            }
            i += 1;
        }

        // Handle final word
        if let Some(start) = word_start {
            self.extract_token(text, start, len, &mut tokens);
        }

        tokens
    }

    /// NEON tokenization for ARM64
    #[cfg(target_arch = "aarch64")]
    fn tokenize_neon(&self, text: &str) -> Vec<String> {
        use std::arch::aarch64::*;

        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut tokens = Vec::new();
        let mut word_start: Option<usize> = None;

        let mut i = 0;

        unsafe {
            let zero = vdupq_n_u8(b'0');
            let nine = vdupq_n_u8(b'9');
            let upper_a = vdupq_n_u8(b'A');
            let upper_z = vdupq_n_u8(b'Z');
            let lower_a = vdupq_n_u8(b'a');
            let lower_z = vdupq_n_u8(b'z');

            while i + 16 <= len {
                let chunk = vld1q_u8(bytes.as_ptr().add(i));

                // Check ranges
                let is_digit = vandq_u8(vcgeq_u8(chunk, zero), vcleq_u8(chunk, nine));
                let is_upper = vandq_u8(vcgeq_u8(chunk, upper_a), vcleq_u8(chunk, upper_z));
                let is_lower = vandq_u8(vcgeq_u8(chunk, lower_a), vcleq_u8(chunk, lower_z));
                let is_alnum = vorrq_u8(vorrq_u8(is_digit, is_upper), is_lower);

                // Extract mask manually
                let mut mask_bytes = [0u8; 16];
                vst1q_u8(mask_bytes.as_mut_ptr(), is_alnum);

                for (j, &b) in mask_bytes.iter().enumerate() {
                    let is_word_char = b != 0;
                    let pos = i + j;

                    if is_word_char {
                        if word_start.is_none() {
                            word_start = Some(pos);
                        }
                    } else if let Some(start) = word_start {
                        self.extract_token(text, start, pos, &mut tokens);
                        word_start = None;
                    }
                }

                i += 16;
            }
        }

        // Handle remaining bytes
        while i < len {
            let c = bytes[i] as char;
            let is_word_char = c.is_alphanumeric();

            if is_word_char {
                if word_start.is_none() {
                    word_start = Some(i);
                }
            } else if let Some(start) = word_start {
                self.extract_token(text, start, i, &mut tokens);
                word_start = None;
            }
            i += 1;
        }

        if let Some(start) = word_start {
            self.extract_token(text, start, len, &mut tokens);
        }

        tokens
    }

    /// Extract a token from text and add to tokens vector
    fn extract_token(&self, text: &str, start: usize, end: usize, tokens: &mut Vec<String>) {
        let word = &text[start..end];
        let word_lower = word.to_lowercase();

        if word_lower.len() >= self.config.min_term_length
            && word_lower.len() <= self.config.max_term_length
            && !self.config.stop_words.contains(&word_lower)
        {
            let token = if self.config.enable_stemming {
                self.stem_word(&word_lower)
            } else {
                word_lower
            };
            tokens.push(token);
        }
    }

    /// Simple Porter-like stemmer
    fn stem_word(&self, word: &str) -> String {
        let mut result = word.to_string();

        // Simple suffix stripping rules
        if result.ends_with("ing") && result.len() > 5 {
            result.truncate(result.len() - 3);
        } else if (result.ends_with("ed") || result.ends_with("ly")) && result.len() > 4 {
            result.truncate(result.len() - 2);
        } else if result.ends_with("ies") && result.len() > 4 {
            result.truncate(result.len() - 3);
            result.push('y');
        } else if result.ends_with("es") && result.len() > 4 {
            result.truncate(result.len() - 2);
        } else if result.ends_with('s') && result.len() > 3 && !result.ends_with("ss") {
            result.truncate(result.len() - 1);
        }

        result
    }

    /// Batch tokenize multiple documents (more efficient)
    pub fn tokenize_batch(&self, texts: &[&str]) -> Vec<Vec<String>> {
        texts.iter().map(|t| self.tokenize(t)).collect()
    }
}

// ============================================================================
// Inverted Index
// ============================================================================

/// Posting entry in the inverted index
#[derive(Debug, Clone)]
pub struct Posting {
    /// Document ID
    pub doc_id: String,
    /// Term frequency in this document
    pub term_freq: u32,
    /// Field where term appears
    pub field: String,
    /// Positions within the field (for phrase queries)
    pub positions: Vec<u32>,
}

/// Statistics for a term
#[derive(Debug, Clone, Default)]
pub struct TermStats {
    /// Document frequency (number of docs containing term)
    pub doc_freq: u32,
    /// Total term frequency across all documents
    pub total_freq: u64,
}

/// In-memory inverted index
pub struct InvertedIndex {
    /// Term -> Postings
    postings: HashMap<String, Vec<Posting>>,
    /// Term statistics
    term_stats: HashMap<String, TermStats>,
    /// Document ID -> Document length (for BM25)
    doc_lengths: HashMap<String, u32>,
    /// Total number of documents
    num_docs: u64,
    /// Average document length
    avg_doc_len: f32,
    /// Document storage
    documents: HashMap<String, FtsDocument>,
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self {
            postings: HashMap::new(),
            term_stats: HashMap::new(),
            doc_lengths: HashMap::new(),
            num_docs: 0,
            avg_doc_len: 0.0,
            documents: HashMap::new(),
        }
    }

    /// Add a document to the index
    pub fn add_document(
        &mut self,
        doc: FtsDocument,
        tokens_by_field: HashMap<String, Vec<String>>,
    ) {
        // Calculate document length
        let doc_len: u32 = tokens_by_field.values().map(|t| t.len() as u32).sum();
        self.doc_lengths.insert(doc.id.clone(), doc_len);

        // Update average document length
        let old_total = self.avg_doc_len * self.num_docs as f32;
        self.num_docs += 1;
        self.avg_doc_len = (old_total + doc_len as f32) / self.num_docs as f32;

        // Index each field
        for (field, tokens) in tokens_by_field {
            let mut term_positions: HashMap<String, Vec<u32>> = HashMap::new();

            for (pos, token) in tokens.iter().enumerate() {
                term_positions
                    .entry(token.clone())
                    .or_default()
                    .push(pos as u32);
            }

            for (term, positions) in term_positions {
                let term_freq = positions.len() as u32;
                let posting = Posting {
                    doc_id: doc.id.clone(),
                    term_freq,
                    field: field.clone(),
                    positions,
                };

                // Update postings
                self.postings.entry(term.clone()).or_default().push(posting);

                // Update term stats
                let stats = self.term_stats.entry(term).or_default();
                stats.doc_freq += 1;
                stats.total_freq += term_freq as u64;
            }
        }

        // Store document
        self.documents.insert(doc.id.clone(), doc);
    }

    /// Remove a document from the index
    pub fn remove_document(&mut self, doc_id: &str) -> bool {
        if let Some(doc_len) = self.doc_lengths.remove(doc_id) {
            // Update average document length
            if self.num_docs > 1 {
                let old_total = self.avg_doc_len * self.num_docs as f32;
                self.num_docs -= 1;
                self.avg_doc_len = (old_total - doc_len as f32) / self.num_docs as f32;
            } else {
                self.num_docs = 0;
                self.avg_doc_len = 0.0;
            }

            // Remove from postings
            for postings in self.postings.values_mut() {
                postings.retain(|p| p.doc_id != doc_id);
            }

            // Remove empty terms
            self.postings.retain(|_, v| !v.is_empty());

            // Update term stats
            for (term, postings) in &self.postings {
                if let Some(stats) = self.term_stats.get_mut(term) {
                    stats.doc_freq = postings.len() as u32;
                    stats.total_freq = postings.iter().map(|p| p.term_freq as u64).sum();
                }
            }

            self.documents.remove(doc_id);
            true
        } else {
            false
        }
    }

    /// Get postings for a term
    pub fn get_postings(&self, term: &str) -> Option<&Vec<Posting>> {
        self.postings.get(term)
    }

    /// Get term statistics
    pub fn get_term_stats(&self, term: &str) -> Option<&TermStats> {
        self.term_stats.get(term)
    }

    /// Get document by ID
    pub fn get_document(&self, doc_id: &str) -> Option<&FtsDocument> {
        self.documents.get(doc_id)
    }

    /// Get all documents
    pub fn get_all_documents(&self) -> impl Iterator<Item = &FtsDocument> {
        self.documents.values()
    }

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            num_docs: self.num_docs,
            num_terms: self.postings.len() as u64,
            avg_doc_len: self.avg_doc_len,
        }
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub num_docs: u64,
    pub num_terms: u64,
    pub avg_doc_len: f32,
}

// ============================================================================
// BM25 Scorer with GPU Acceleration
// ============================================================================

/// BM25 scoring engine
pub struct Bm25Scorer {
    k1: f32,
    b: f32,
    enable_gpu: bool,
}

impl Bm25Scorer {
    pub fn new(k1: f32, b: f32, enable_gpu: bool) -> Self {
        Self { k1, b, enable_gpu }
    }

    /// Calculate BM25 score for a document
    pub fn score(&self, query_terms: &[String], doc_id: &str, index: &InvertedIndex) -> f32 {
        let doc_len = *index.doc_lengths.get(doc_id).unwrap_or(&1) as f32;
        let avg_doc_len = index.avg_doc_len.max(1.0);
        let num_docs = index.num_docs as f32;

        let mut score = 0.0;

        for term in query_terms {
            if let Some(postings) = index.get_postings(term) {
                // Find posting for this document
                if let Some(posting) = postings.iter().find(|p| p.doc_id == doc_id) {
                    let tf = posting.term_freq as f32;
                    let df = postings.len() as f32;

                    // IDF = log((N - df + 0.5) / (df + 0.5))
                    let idf = ((num_docs - df + 0.5) / (df + 0.5) + 1.0).ln();

                    // BM25 term score
                    let numerator = tf * (self.k1 + 1.0);
                    let denominator =
                        tf + self.k1 * (1.0 - self.b + self.b * (doc_len / avg_doc_len));

                    score += idf * (numerator / denominator);
                }
            }
        }

        score
    }

    /// Score multiple documents in batch (can use GPU)
    pub fn score_batch(
        &self,
        query_terms: &[String],
        doc_ids: &[String],
        index: &InvertedIndex,
    ) -> Vec<(String, f32)> {
        if self.enable_gpu && doc_ids.len() > 1000 {
            self.score_batch_gpu(query_terms, doc_ids, index)
        } else {
            self.score_batch_cpu(query_terms, doc_ids, index)
        }
    }

    /// CPU batch scoring with SIMD
    fn score_batch_cpu(
        &self,
        query_terms: &[String],
        doc_ids: &[String],
        index: &InvertedIndex,
    ) -> Vec<(String, f32)> {
        doc_ids
            .iter()
            .map(|doc_id| {
                let score = self.score(query_terms, doc_id, index);
                (doc_id.clone(), score)
            })
            .collect()
    }

    /// GPU batch scoring (placeholder - would integrate with orbit-compute)
    fn score_batch_gpu(
        &self,
        query_terms: &[String],
        doc_ids: &[String],
        index: &InvertedIndex,
    ) -> Vec<(String, f32)> {
        // TODO: Integrate with orbit_compute for GPU acceleration
        // For now, fall back to CPU
        self.score_batch_cpu(query_terms, doc_ids, index)
    }
}

impl Default for Bm25Scorer {
    fn default() -> Self {
        Self::new(1.2, 0.75, false)
    }
}

// ============================================================================
// Shared FTS Engine
// ============================================================================

/// Main shared FTS engine
pub struct SharedFtsEngine {
    #[allow(dead_code)]
    config: SharedFtsConfig,
    processor: TextProcessor,
    scorer: Bm25Scorer,
    indexes: Arc<RwLock<HashMap<String, InvertedIndex>>>,
}

impl SharedFtsEngine {
    /// Create a new shared FTS engine
    pub fn new(config: SharedFtsConfig) -> Self {
        let processor = TextProcessor::new(config.clone());
        let scorer = Bm25Scorer::new(config.bm25_k1, config.bm25_b, config.enable_gpu);

        Self {
            config,
            processor,
            scorer,
            indexes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new index
    pub async fn create_index(&self, name: &str) -> FtsResult<()> {
        let mut indexes = self.indexes.write().await;
        indexes.insert(name.to_string(), InvertedIndex::new());
        Ok(())
    }

    /// Drop an index
    pub async fn drop_index(&self, name: &str) -> FtsResult<bool> {
        let mut indexes = self.indexes.write().await;
        Ok(indexes.remove(name).is_some())
    }

    /// Index a document
    pub async fn index_document(&self, index_name: &str, doc: FtsDocument) -> FtsResult<()> {
        let mut indexes = self.indexes.write().await;
        let index = indexes
            .get_mut(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        // Tokenize each field
        let mut tokens_by_field = HashMap::new();
        for (field, text) in &doc.fields {
            let tokens = self.processor.tokenize(text);
            tokens_by_field.insert(field.clone(), tokens);
        }

        index.add_document(doc, tokens_by_field);
        Ok(())
    }

    /// Remove a document from an index
    pub async fn remove_document(&self, index_name: &str, doc_id: &str) -> FtsResult<bool> {
        let mut indexes = self.indexes.write().await;
        let index = indexes
            .get_mut(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        Ok(index.remove_document(doc_id))
    }

    /// Search an index
    pub async fn search(
        &self,
        index_name: &str,
        query: FtsQuery,
    ) -> FtsResult<Vec<FtsSearchResult>> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        // Tokenize query terms
        let mut all_query_terms: Vec<String> = Vec::new();
        for term in &query.must_terms {
            all_query_terms.extend(self.processor.tokenize(term));
        }
        for term in &query.should_terms {
            all_query_terms.extend(self.processor.tokenize(term));
        }

        if all_query_terms.is_empty() {
            return Ok(Vec::new());
        }

        // Find candidate documents
        let mut candidate_docs: HashSet<String> = HashSet::new();
        for term in &all_query_terms {
            if let Some(postings) = index.get_postings(term) {
                for posting in postings {
                    // Filter by field if specified
                    if let Some(ref fields) = query.fields {
                        if !fields.contains(&posting.field) {
                            continue;
                        }
                    }
                    candidate_docs.insert(posting.doc_id.clone());
                }
            }
        }

        // Handle must_not terms
        let must_not_tokens: Vec<String> = query
            .must_not_terms
            .iter()
            .flat_map(|t| self.processor.tokenize(t))
            .collect();

        for term in &must_not_tokens {
            if let Some(postings) = index.get_postings(term) {
                for posting in postings {
                    candidate_docs.remove(&posting.doc_id);
                }
            }
        }

        // Score candidates
        let doc_ids: Vec<String> = candidate_docs.into_iter().collect();
        let mut scored_docs = self.scorer.score_batch(&all_query_terms, &doc_ids, index);

        // Filter by minimum score
        if let Some(min_score) = query.min_score {
            scored_docs.retain(|(_, score)| *score >= min_score);
        }

        // Sort by score descending
        scored_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply offset and limit
        let results: Vec<FtsSearchResult> = scored_docs
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .filter_map(|(doc_id, score)| {
                index.get_document(&doc_id).map(|doc| {
                    // Calculate term frequencies
                    let mut term_frequencies = HashMap::new();
                    for term in &all_query_terms {
                        if let Some(postings) = index.get_postings(term) {
                            if let Some(posting) = postings.iter().find(|p| p.doc_id == doc_id) {
                                term_frequencies.insert(term.clone(), posting.term_freq);
                            }
                        }
                    }

                    FtsSearchResult {
                        doc_id: doc_id.clone(),
                        score,
                        fields: doc.fields.clone(),
                        term_frequencies,
                        highlights: Vec::new(), // TODO: Implement highlighting
                    }
                })
            })
            .collect();

        Ok(results)
    }

    /// Get index statistics
    pub async fn get_stats(&self, index_name: &str) -> FtsResult<IndexStats> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        Ok(index.stats())
    }

    /// List all indexes
    pub async fn list_indexes(&self) -> Vec<String> {
        self.indexes.read().await.keys().cloned().collect()
    }
}

impl Default for SharedFtsEngine {
    fn default() -> Self {
        Self::new(SharedFtsConfig::default())
    }
}

#[async_trait]
impl UnifiedFtsEngine for SharedFtsEngine {
    fn engine_type(&self) -> FtsEngineType {
        FtsEngineType::Shared
    }

    async fn create_index(&self, name: &str, _fields: &[String]) -> FtsResult<()> {
        let mut indexes = self.indexes.write().await;
        indexes.insert(name.to_string(), InvertedIndex::new());
        Ok(())
    }

    async fn drop_index(&self, name: &str) -> FtsResult<bool> {
        let mut indexes = self.indexes.write().await;
        Ok(indexes.remove(name).is_some())
    }

    async fn index_document(
        &self,
        index_name: &str,
        doc_id: &str,
        fields: HashMap<String, String>,
    ) -> FtsResult<()> {
        let doc = FtsDocument {
            id: doc_id.to_string(),
            fields,
            metadata: None,
        };
        // Use the existing method
        let mut indexes = self.indexes.write().await;
        let index = indexes
            .get_mut(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let mut tokens_by_field = HashMap::new();
        for (field, text) in &doc.fields {
            let tokens = self.processor.tokenize(text);
            tokens_by_field.insert(field.clone(), tokens);
        }

        index.add_document(doc, tokens_by_field);
        Ok(())
    }

    async fn remove_document(&self, index_name: &str, doc_id: &str) -> FtsResult<bool> {
        let mut indexes = self.indexes.write().await;
        let index = indexes
            .get_mut(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        Ok(index.remove_document(doc_id))
    }

    async fn search(
        &self,
        index_name: &str,
        query: &FtsQuery,
    ) -> FtsResult<Vec<UnifiedSearchResult>> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        // Tokenize query terms
        let mut all_query_terms: Vec<String> = Vec::new();
        for term in &query.must_terms {
            all_query_terms.extend(self.processor.tokenize(term));
        }
        for term in &query.should_terms {
            all_query_terms.extend(self.processor.tokenize(term));
        }

        if all_query_terms.is_empty() {
            return Ok(Vec::new());
        }

        // Find candidate documents
        let mut candidate_docs: HashSet<String> = HashSet::new();
        for term in &all_query_terms {
            if let Some(postings) = index.get_postings(term) {
                for posting in postings {
                    if let Some(ref fields) = query.fields {
                        if !fields.contains(&posting.field) {
                            continue;
                        }
                    }
                    candidate_docs.insert(posting.doc_id.clone());
                }
            }
        }

        // Handle must_not terms
        let must_not_tokens: Vec<String> = query
            .must_not_terms
            .iter()
            .flat_map(|t| self.processor.tokenize(t))
            .collect();

        for term in &must_not_tokens {
            if let Some(postings) = index.get_postings(term) {
                for posting in postings {
                    candidate_docs.remove(&posting.doc_id);
                }
            }
        }

        // Score candidates
        let doc_ids: Vec<String> = candidate_docs.into_iter().collect();
        let mut scored_docs = self.scorer.score_batch(&all_query_terms, &doc_ids, index);

        // Filter by minimum score
        if let Some(min_score) = query.min_score {
            scored_docs.retain(|(_, score)| *score >= min_score);
        }

        // Sort by score descending
        scored_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply offset and limit
        let results: Vec<UnifiedSearchResult> = scored_docs
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .filter_map(|(doc_id, score)| {
                index.get_document(&doc_id).map(|doc| UnifiedSearchResult {
                    doc_id: doc_id.clone(),
                    score,
                    fields: doc.fields.clone(),
                    highlights: Vec::new(),
                })
            })
            .collect();

        Ok(results)
    }

    async fn commit(&self, _index_name: &str) -> FtsResult<()> {
        // SharedFtsEngine is in-memory, commit is a no-op
        // If persistence is enabled, this would flush to disk
        if self.config.enable_persistence {
            self.flush().await?;
        }
        Ok(())
    }

    async fn get_stats(&self, index_name: &str) -> FtsResult<UnifiedIndexStats> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let stats = index.stats();
        Ok(UnifiedIndexStats {
            num_docs: stats.num_docs,
            num_terms: stats.num_terms,
            avg_doc_len: stats.avg_doc_len,
            size_bytes: 0, // In-memory, no disk size
        })
    }

    async fn list_indexes(&self) -> Vec<String> {
        self.indexes.read().await.keys().cloned().collect()
    }

    async fn flush(&self) -> FtsResult<()> {
        if !self.config.enable_persistence {
            return Ok(());
        }

        let persistence_dir = self.config.persistence_dir.as_ref().ok_or_else(|| {
            FtsError::PersistenceError("Persistence directory not configured".to_string())
        })?;

        // Create persistence directory if it doesn't exist
        std::fs::create_dir_all(persistence_dir)?;

        let indexes = self.indexes.read().await;
        for (name, index) in indexes.iter() {
            let index_file = persistence_dir.join(format!("{}.fts.json", name));
            let data = serde_json::to_string(&SerializableIndex::from(index))
                .map_err(|e| FtsError::PersistenceError(e.to_string()))?;
            std::fs::write(&index_file, data)?;
        }

        Ok(())
    }
}

// ============================================================================
// Tantivy FTS Engine Wrapper
// ============================================================================

/// Tantivy-based FTS engine that wraps the legacy FTS module
pub struct TantivyFtsEngine {
    config: UnifiedFtsConfig,
    indexes: Arc<RwLock<HashMap<String, TantivyIndexHandle>>>,
}

/// Handle to a Tantivy index
struct TantivyIndexHandle {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<RwLock<tantivy::IndexWriter>>,
    schema: tantivy::schema::Schema,
    field_map: HashMap<String, tantivy::schema::Field>,
}

impl TantivyFtsEngine {
    /// Create a new Tantivy FTS engine
    pub fn new(config: UnifiedFtsConfig) -> FtsResult<Self> {
        std::fs::create_dir_all(&config.index_dir)?;
        Ok(Self {
            config,
            indexes: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl UnifiedFtsEngine for TantivyFtsEngine {
    fn engine_type(&self) -> FtsEngineType {
        FtsEngineType::Tantivy
    }

    async fn create_index(&self, name: &str, fields: &[String]) -> FtsResult<()> {
        use tantivy::schema::*;

        let index_path = self.config.index_dir.join(name);
        std::fs::create_dir_all(&index_path)?;

        // Build schema with requested fields
        let mut schema_builder = Schema::builder();
        let mut field_map = HashMap::new();

        // Add doc_id field
        let doc_id_field = schema_builder.add_text_field("_doc_id", STRING | STORED);
        field_map.insert("_doc_id".to_string(), doc_id_field);

        // Add text fields
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        for field_name in fields {
            let field = schema_builder.add_text_field(field_name, text_options.clone());
            field_map.insert(field_name.clone(), field);
        }

        let schema = schema_builder.build();

        // Create Tantivy index
        let index = tantivy::Index::create_in_dir(&index_path, schema.clone())
            .map_err(|e| FtsError::InternalError(e.to_string()))?;

        // Create writer
        let writer = index
            .writer(self.config.max_memory)
            .map_err(|e| FtsError::InternalError(e.to_string()))?;

        // Create reader
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e: tantivy::TantivyError| FtsError::InternalError(e.to_string()))?;

        let handle = TantivyIndexHandle {
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
            schema,
            field_map,
        };

        self.indexes.write().await.insert(name.to_string(), handle);
        Ok(())
    }

    async fn drop_index(&self, name: &str) -> FtsResult<bool> {
        let removed = self.indexes.write().await.remove(name).is_some();

        if removed {
            let index_path = self.config.index_dir.join(name);
            if index_path.exists() {
                std::fs::remove_dir_all(&index_path)?;
            }
        }

        Ok(removed)
    }

    async fn index_document(
        &self,
        index_name: &str,
        doc_id: &str,
        fields: HashMap<String, String>,
    ) -> FtsResult<()> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let mut doc = tantivy::TantivyDocument::new();

        // Add doc_id
        if let Some(&field) = handle.field_map.get("_doc_id") {
            doc.add_text(field, doc_id);
        }

        // Add other fields
        for (field_name, value) in fields {
            if let Some(&field) = handle.field_map.get(&field_name) {
                doc.add_text(field, &value);
            }
        }

        let writer = handle.writer.write().await;
        writer
            .add_document(doc)
            .map_err(|e| FtsError::InternalError(e.to_string()))?;

        Ok(())
    }

    async fn remove_document(&self, index_name: &str, doc_id: &str) -> FtsResult<bool> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        if let Some(&field) = handle.field_map.get("_doc_id") {
            let term = tantivy::Term::from_field_text(field, doc_id);
            let writer = handle.writer.write().await;
            writer.delete_term(term);
            return Ok(true);
        }

        Ok(false)
    }

    async fn search(
        &self,
        index_name: &str,
        query: &FtsQuery,
    ) -> FtsResult<Vec<UnifiedSearchResult>> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let searcher = handle.reader.searcher();

        // Build query string from FtsQuery
        let mut query_parts: Vec<String> = Vec::new();

        for term in &query.must_terms {
            query_parts.push(format!("+{}", term));
        }
        for term in &query.should_terms {
            query_parts.push(term.clone());
        }
        for term in &query.must_not_terms {
            query_parts.push(format!("-{}", term));
        }

        if query_parts.is_empty() {
            return Ok(Vec::new());
        }

        let query_string = query_parts.join(" ");

        // Get searchable fields
        let search_fields: Vec<tantivy::schema::Field> = if let Some(ref field_names) = query.fields
        {
            field_names
                .iter()
                .filter_map(|name| handle.field_map.get(name).copied())
                .collect()
        } else {
            handle
                .field_map
                .iter()
                .filter(|(name, _)| *name != "_doc_id")
                .map(|(_, &field)| field)
                .collect()
        };

        let query_parser = tantivy::query::QueryParser::for_index(&handle.index, search_fields);
        let tantivy_query = query_parser
            .parse_query(&query_string)
            .map_err(|e| FtsError::InvalidQuery(e.to_string()))?;

        let top_docs = searcher
            .search(
                &tantivy_query,
                &tantivy::collector::TopDocs::with_limit(query.limit).and_offset(query.offset),
            )
            .map_err(|e| FtsError::InternalError(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FtsError::InternalError(e.to_string()))?;

            let mut fields_map = HashMap::new();
            let mut doc_id = String::new();

            for (field, _) in handle.schema.fields() {
                let field_entry = handle.schema.get_field_entry(field);
                let field_name = field_entry.name().to_string();

                for value in doc.get_all(field) {
                    // Use the Value trait to access as_str
                    use tantivy::schema::Value;
                    if let Some(text) = value.as_str() {
                        if field_name == "_doc_id" {
                            doc_id = text.to_string();
                        } else {
                            fields_map.insert(field_name.clone(), text.to_string());
                        }
                    }
                }
            }

            if let Some(min_score) = query.min_score {
                if score < min_score {
                    continue;
                }
            }

            results.push(UnifiedSearchResult {
                doc_id,
                score,
                fields: fields_map,
                highlights: Vec::new(),
            });
        }

        Ok(results)
    }

    async fn commit(&self, index_name: &str) -> FtsResult<()> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let mut writer = handle.writer.write().await;
        writer
            .commit()
            .map_err(|e| FtsError::InternalError(e.to_string()))?;

        Ok(())
    }

    async fn get_stats(&self, index_name: &str) -> FtsResult<UnifiedIndexStats> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .ok_or_else(|| FtsError::IndexNotFound(index_name.to_string()))?;

        let searcher = handle.reader.searcher();
        let num_docs = searcher.num_docs();

        // Calculate index size
        let index_path = self.config.index_dir.join(index_name);
        let size_bytes = calculate_dir_size(&index_path).unwrap_or(0);

        Ok(UnifiedIndexStats {
            num_docs,
            num_terms: 0, // Tantivy doesn't expose this easily
            avg_doc_len: 0.0,
            size_bytes,
        })
    }

    async fn list_indexes(&self) -> Vec<String> {
        self.indexes.read().await.keys().cloned().collect()
    }

    async fn flush(&self) -> FtsResult<()> {
        let indexes = self.indexes.read().await;
        for (name, handle) in indexes.iter() {
            let mut writer = handle.writer.write().await;
            writer.commit().map_err(|e| {
                FtsError::PersistenceError(format!("Failed to commit index {}: {}", name, e))
            })?;
        }
        Ok(())
    }
}

/// Calculate directory size recursively
fn calculate_dir_size(path: &PathBuf) -> FtsResult<u64> {
    let mut size = 0u64;

    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                size += calculate_dir_size(&entry.path())?;
            } else {
                size += metadata.len();
            }
        }
    }

    Ok(size)
}

// ============================================================================
// Serialization for SharedFtsEngine persistence
// ============================================================================

/// Serializable version of InvertedIndex for JSON persistence
#[derive(serde::Serialize, serde::Deserialize)]
struct SerializableIndex {
    postings: HashMap<String, Vec<SerializablePosting>>,
    doc_lengths: HashMap<String, u32>,
    num_docs: u64,
    avg_doc_len: f32,
    documents: HashMap<String, SerializableDocument>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SerializablePosting {
    doc_id: String,
    term_freq: u32,
    field: String,
    positions: Vec<u32>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SerializableDocument {
    id: String,
    fields: HashMap<String, String>,
}

impl From<&InvertedIndex> for SerializableIndex {
    fn from(index: &InvertedIndex) -> Self {
        let postings: HashMap<String, Vec<SerializablePosting>> = index
            .postings
            .iter()
            .map(|(term, posts)| {
                (
                    term.clone(),
                    posts
                        .iter()
                        .map(|p| SerializablePosting {
                            doc_id: p.doc_id.clone(),
                            term_freq: p.term_freq,
                            field: p.field.clone(),
                            positions: p.positions.clone(),
                        })
                        .collect(),
                )
            })
            .collect();

        let documents: HashMap<String, SerializableDocument> = index
            .documents
            .iter()
            .map(|(id, doc)| {
                (
                    id.clone(),
                    SerializableDocument {
                        id: doc.id.clone(),
                        fields: doc.fields.clone(),
                    },
                )
            })
            .collect();

        SerializableIndex {
            postings,
            doc_lengths: index.doc_lengths.clone(),
            num_docs: index.num_docs,
            avg_doc_len: index.avg_doc_len,
            documents,
        }
    }
}

// ============================================================================
// FTS Engine Factory
// ============================================================================

/// Factory for creating FTS engines based on configuration
pub struct FtsEngineFactory;

impl FtsEngineFactory {
    /// Create an FTS engine based on configuration
    pub fn create(config: UnifiedFtsConfig) -> FtsResult<Arc<dyn UnifiedFtsEngine>> {
        match config.engine_type {
            FtsEngineType::Shared => {
                let shared_config = SharedFtsConfig {
                    default_language: config.default_language.clone(),
                    enable_stemming: true,
                    min_term_length: 2,
                    max_term_length: 100,
                    stop_words: default_stop_words(),
                    bm25_k1: config.bm25_k1,
                    bm25_b: config.bm25_b,
                    enable_simd: config.enable_simd,
                    enable_gpu: config.enable_gpu,
                    enable_persistence: true,
                    persistence_dir: Some(config.index_dir.clone()),
                    auto_flush_secs: 0,
                };
                Ok(Arc::new(SharedFtsEngine::new(shared_config)))
            }
            FtsEngineType::Tantivy => Ok(Arc::new(TantivyFtsEngine::new(config)?)),
        }
    }
}

// ============================================================================
// Protocol Query Parsers
// ============================================================================

/// Parse MySQL MATCH AGAINST query
pub fn parse_mysql_query(query: &str, mode: MysqlSearchMode) -> FtsQuery {
    match mode {
        MysqlSearchMode::Natural => {
            let terms: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
            FtsQuery {
                query_type: FtsQueryType::Natural,
                should_terms: terms,
                ..Default::default()
            }
        }
        MysqlSearchMode::Boolean => {
            let mut fts_query = FtsQuery {
                query_type: FtsQueryType::Boolean,
                ..Default::default()
            };

            for term in query.split_whitespace() {
                if let Some(stripped) = term.strip_prefix('+') {
                    fts_query.must_terms.push(stripped.to_string());
                } else if let Some(stripped) = term.strip_prefix('-') {
                    fts_query.must_not_terms.push(stripped.to_string());
                } else if term.starts_with('"') && term.ends_with('"') {
                    let phrase: Vec<String> = term
                        .trim_matches('"')
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    fts_query.phrases.push(phrase);
                } else {
                    fts_query.should_terms.push(term.to_string());
                }
            }

            fts_query
        }
        MysqlSearchMode::QueryExpansion => {
            // Query expansion treated as natural with expansion flag
            let terms: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
            FtsQuery {
                query_type: FtsQueryType::Natural,
                should_terms: terms,
                ..Default::default()
            }
        }
    }
}

/// MySQL search modes
#[derive(Debug, Clone, Copy)]
pub enum MysqlSearchMode {
    Natural,
    Boolean,
    QueryExpansion,
}

/// Parse PostgreSQL tsquery
pub fn parse_postgres_tsquery(query: &str) -> FtsQuery {
    let mut fts_query = FtsQuery {
        query_type: FtsQueryType::Boolean,
        ..Default::default()
    };

    // Simple tsquery parser: word & word | !word
    for part in query.split_whitespace() {
        let part = part.trim_matches(|c| c == '&' || c == '|' || c == '(' || c == ')');
        if let Some(stripped) = part.strip_prefix('!') {
            fts_query.must_not_terms.push(stripped.to_string());
        } else if part.contains(':') {
            // Has prefix like "fat:*"
            let term = part.split(':').next().unwrap_or(part);
            fts_query.should_terms.push(term.to_string());
        } else if !part.is_empty() {
            fts_query.should_terms.push(part.to_string());
        }
    }

    fts_query
}

/// Parse Redis FT.SEARCH query
pub fn parse_redis_query(query: &str) -> FtsQuery {
    let mut fts_query = FtsQuery {
        query_type: FtsQueryType::Boolean,
        ..Default::default()
    };

    // Redis query syntax: @field:term | -term | (term1 | term2)
    let mut current_field: Option<String> = None;

    for part in query.split_whitespace() {
        if let Some(stripped) = part.strip_prefix('@') {
            // Field specifier: @field:value
            if let Some((field, value)) = stripped.split_once(':') {
                current_field = Some(field.to_string());
                if !value.is_empty() {
                    fts_query.should_terms.push(value.to_string());
                }
            }
        } else if let Some(stripped) = part.strip_prefix('-') {
            fts_query.must_not_terms.push(stripped.to_string());
        } else if part.starts_with('"') && part.ends_with('"') {
            let phrase: Vec<String> = part
                .trim_matches('"')
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            fts_query.phrases.push(phrase);
        } else {
            let term = part.trim_matches(|c| c == '(' || c == ')' || c == '|');
            if !term.is_empty() {
                fts_query.should_terms.push(term.to_string());
            }
        }
    }

    if let Some(field) = current_field {
        fts_query.fields = Some(vec![field]);
    }

    fts_query
}

/// Parse CQL CONTAINS/LIKE query
pub fn parse_cql_query(query: &str, query_type: CqlQueryType) -> FtsQuery {
    match query_type {
        CqlQueryType::Contains => FtsQuery {
            query_type: FtsQueryType::Natural,
            must_terms: vec![query.to_string()],
            ..Default::default()
        },
        CqlQueryType::Like => {
            // LIKE patterns: %suffix, prefix%, %contains%
            let has_prefix_wildcard = query.starts_with('%');
            let has_suffix_wildcard = query.ends_with('%');
            let core = query.trim_matches('%');

            if has_prefix_wildcard && has_suffix_wildcard {
                // Contains anywhere
                FtsQuery {
                    query_type: FtsQueryType::Natural,
                    should_terms: vec![core.to_string()],
                    ..Default::default()
                }
            } else if has_suffix_wildcard {
                // Prefix match
                FtsQuery {
                    query_type: FtsQueryType::Prefix,
                    must_terms: vec![core.to_string()],
                    ..Default::default()
                }
            } else {
                // Suffix or exact
                FtsQuery {
                    query_type: FtsQueryType::Natural,
                    must_terms: vec![core.to_string()],
                    ..Default::default()
                }
            }
        }
        CqlQueryType::Fulltext => {
            let terms: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
            FtsQuery {
                query_type: FtsQueryType::Natural,
                should_terms: terms,
                ..Default::default()
            }
        }
    }
}

/// CQL query types
#[derive(Debug, Clone, Copy)]
pub enum CqlQueryType {
    Contains,
    Like,
    Fulltext,
}

/// Parse MongoDB $text query
pub fn parse_mongodb_text_query(query: &str) -> FtsQuery {
    let mut fts_query = FtsQuery {
        query_type: FtsQueryType::Natural,
        ..Default::default()
    };

    // MongoDB $text: "phrase" or -term or term
    // First extract quoted phrases
    let mut remaining = query.to_string();

    // Find all quoted phrases
    while let Some(start) = remaining.find('"') {
        if let Some(end) = remaining[start + 1..].find('"') {
            let phrase_content = &remaining[start + 1..start + 1 + end];
            let phrase: Vec<String> = phrase_content
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            if !phrase.is_empty() {
                fts_query.phrases.push(phrase);
            }
            // Remove the phrase from remaining text
            remaining = format!(
                "{}{}",
                &remaining[..start],
                &remaining[start + 1 + end + 1..]
            );
        } else {
            break;
        }
    }

    // Process remaining terms
    for part in remaining.split_whitespace() {
        if part.starts_with('-') && part.len() > 1 {
            fts_query.must_not_terms.push(part[1..].to_string());
        } else if !part.is_empty() {
            fts_query.should_terms.push(part.to_string());
        }
    }

    fts_query
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Default English stop words
fn default_stop_words() -> HashSet<String> {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is",
        "it", "its", "of", "on", "that", "the", "to", "was", "were", "will", "with", "the", "this",
        "but", "they", "have", "had", "what", "when", "where", "who", "which", "why", "how",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_scalar() {
        let config = SharedFtsConfig {
            enable_simd: false,
            enable_stemming: false,
            ..Default::default()
        };
        let processor = TextProcessor::new(config);

        let tokens = processor.tokenize("The quick brown fox jumps over the lazy dog");
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        // "the" should be filtered as stop word
        assert!(!tokens.contains(&"the".to_string()));
    }

    #[test]
    fn test_tokenizer_simd() {
        let config = SharedFtsConfig {
            enable_simd: true,
            enable_stemming: false,
            ..Default::default()
        };
        let processor = TextProcessor::new(config);

        // Long text to trigger SIMD path
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let tokens = processor.tokenize(&text);
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
    }

    #[test]
    fn test_stemming() {
        let config = SharedFtsConfig {
            enable_simd: false,
            enable_stemming: true,
            ..Default::default()
        };
        let processor = TextProcessor::new(config);

        let tokens = processor.tokenize("running jumping flying");
        assert!(tokens.contains(&"runn".to_string())); // "running" -> "runn"
        assert!(tokens.contains(&"jump".to_string())); // "jumping" -> "jump"
        assert!(tokens.contains(&"fly".to_string())); // "flying" -> "fly"
    }

    #[test]
    fn test_bm25_scoring() {
        let scorer = Bm25Scorer::default();
        let mut index = InvertedIndex::new();

        // Add test documents
        let doc1 = FtsDocument {
            id: "1".to_string(),
            fields: [(
                "content".to_string(),
                "rust programming language".to_string(),
            )]
            .into_iter()
            .collect(),
            metadata: None,
        };
        let doc2 = FtsDocument {
            id: "2".to_string(),
            fields: [(
                "content".to_string(),
                "python programming language".to_string(),
            )]
            .into_iter()
            .collect(),
            metadata: None,
        };

        let config = SharedFtsConfig::default();
        let processor = TextProcessor::new(config);

        let tokens1 = processor.tokenize("rust programming language");
        let tokens2 = processor.tokenize("python programming language");

        index.add_document(
            doc1,
            [("content".to_string(), tokens1)].into_iter().collect(),
        );
        index.add_document(
            doc2,
            [("content".to_string(), tokens2)].into_iter().collect(),
        );

        // Search for "rust"
        let score1 = scorer.score(&["rust".to_string()], "1", &index);
        let score2 = scorer.score(&["rust".to_string()], "2", &index);

        // Doc 1 should score higher for "rust"
        assert!(score1 > score2);
    }

    #[tokio::test]
    async fn test_shared_fts_engine() {
        let engine = SharedFtsEngine::default();

        // Create index
        engine.create_index("test").await.unwrap();

        // Index documents
        engine
            .index_document(
                "test",
                FtsDocument {
                    id: "1".to_string(),
                    fields: [("content".to_string(), "rust programming".to_string())]
                        .into_iter()
                        .collect(),
                    metadata: None,
                },
            )
            .await
            .unwrap();

        engine
            .index_document(
                "test",
                FtsDocument {
                    id: "2".to_string(),
                    fields: [("content".to_string(), "python programming".to_string())]
                        .into_iter()
                        .collect(),
                    metadata: None,
                },
            )
            .await
            .unwrap();

        // Search
        let query = FtsQuery {
            should_terms: vec!["rust".to_string()],
            ..Default::default()
        };
        let results = engine.search("test", query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "1");
    }

    #[test]
    fn test_mysql_query_parser() {
        // Natural mode
        let query = parse_mysql_query("hello world", MysqlSearchMode::Natural);
        assert_eq!(query.should_terms, vec!["hello", "world"]);

        // Boolean mode
        let query = parse_mysql_query("+must -exclude optional", MysqlSearchMode::Boolean);
        assert_eq!(query.must_terms, vec!["must"]);
        assert_eq!(query.must_not_terms, vec!["exclude"]);
        assert_eq!(query.should_terms, vec!["optional"]);
    }

    #[test]
    fn test_postgres_tsquery_parser() {
        let query = parse_postgres_tsquery("fat & cat & !dog");
        assert!(query.should_terms.contains(&"fat".to_string()));
        assert!(query.should_terms.contains(&"cat".to_string()));
        assert!(query.must_not_terms.contains(&"dog".to_string()));
    }

    #[test]
    fn test_redis_query_parser() {
        let query = parse_redis_query("@title:hello -exclude world");
        assert!(query.should_terms.contains(&"hello".to_string()));
        assert!(query.should_terms.contains(&"world".to_string()));
        assert!(query.must_not_terms.contains(&"exclude".to_string()));
        assert_eq!(query.fields, Some(vec!["title".to_string()]));
    }

    #[test]
    fn test_mongodb_text_query_parser() {
        let query = parse_mongodb_text_query("\"exact phrase\" -exclude optional");
        assert_eq!(query.phrases, vec![vec!["exact", "phrase"]]);
        assert!(query.must_not_terms.contains(&"exclude".to_string()));
        assert!(query.should_terms.contains(&"optional".to_string()));
    }
}
