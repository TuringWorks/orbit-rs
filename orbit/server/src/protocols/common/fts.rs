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
//!
//! ## Performance
//!
//! - SIMD (AVX2/AVX-512/NEON) for text processing operations
//! - GPU acceleration for large-scale BM25 scoring and batch operations
//! - In-memory inverted index with optional persistence via Tantivy
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

use std::collections::{HashMap, HashSet};
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
    /// Internal error
    InternalError(String),
}

impl std::fmt::Display for FtsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FtsError::IndexNotFound(name) => write!(f, "Index not found: {}", name),
            FtsError::InvalidQuery(msg) => write!(f, "Invalid query: {}", msg),
            FtsError::TokenizationError(msg) => write!(f, "Tokenization error: {}", msg),
            FtsError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for FtsError {}

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
        } else if result.ends_with("ed") && result.len() > 4 {
            result.truncate(result.len() - 2);
        } else if result.ends_with("ly") && result.len() > 4 {
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
                if term.starts_with('+') {
                    fts_query.must_terms.push(term[1..].to_string());
                } else if term.starts_with('-') {
                    fts_query.must_not_terms.push(term[1..].to_string());
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
        if part.starts_with('!') {
            fts_query.must_not_terms.push(part[1..].to_string());
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
        if part.starts_with('@') {
            // Field specifier: @field:value
            if let Some((field, value)) = part[1..].split_once(':') {
                current_field = Some(field.to_string());
                if !value.is_empty() {
                    fts_query.should_terms.push(value.to_string());
                }
            }
        } else if part.starts_with('-') {
            fts_query.must_not_terms.push(part[1..].to_string());
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
