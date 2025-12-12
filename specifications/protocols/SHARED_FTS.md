# Shared Full-Text Search Specification

**Last Updated**: 2025-12-10
**Module**: `orbit/server/src/protocols/common/fts.rs`
**Status**: Production Ready

## Overview

The shared FTS module provides protocol-agnostic full-text search capabilities that can be used by all OrbitRS protocols (PostgreSQL, MySQL, CQL, Redis, MongoDB, OrbitQL). It features SIMD-accelerated text processing and GPU-ready BM25 scoring.

## Protocol Integration Status

| Protocol | Query Syntax | Index Type | Integration Status |
|----------|-------------|------------|-------------------|
| **PostgreSQL** | `tsvector @@ tsquery` | GIN | ✅ Ready |
| **MySQL** | `MATCH() AGAINST()` | FULLTEXT | ✅ Ready |
| **CQL** | `CONTAINS`, `LIKE` | SASI/SAI | ✅ Ready |
| **Redis** | `FT.SEARCH`, `FT.CREATE` | RedisSearch | ✅ Ready |
| **MongoDB** | `$text` | Text Index | ✅ Ready |
| **OrbitQL** | `SEARCH()` | FTS Index | ✅ Ready |

## Architecture

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│ PostgreSQL  │  │   MySQL     │  │     CQL     │  │    Redis    │  │   MongoDB   │  │  OrbitQL    │
│   ts_query  │  │MATCH AGAINST│  │ CONTAINS/   │  │ FT.SEARCH   │  │   $text     │  │  SEARCH()   │
│             │  │             │  │    LIKE     │  │             │  │             │  │             │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │                │                │                │
       │  parse_postgres_tsquery()       │  parse_cql_query()              │                │
       │                │                │                │                │                │
       └────────────────┴────────────────┴────────────────┴────────────────┴────────────────┘
                                                   │
                                    ┌──────────────▼──────────────┐
                                    │         FtsQuery            │
                                    │   (Protocol-Agnostic AST)   │
                                    │                             │
                                    │  - must_terms               │
                                    │  - should_terms             │
                                    │  - must_not_terms           │
                                    │  - phrases                  │
                                    │  - query_type               │
                                    └──────────────┬──────────────┘
                                                   │
                                    ┌──────────────▼──────────────┐
                                    │      SharedFtsEngine        │
                                    │                             │
                                    │  ┌────────────────────────┐ │
                                    │  │    TextProcessor       │ │
                                    │  │  (SIMD Tokenization)   │ │
                                    │  │  - AVX2 (x86_64)       │ │
                                    │  │  - NEON (ARM64)        │ │
                                    │  │  - Stemming            │ │
                                    │  │  - Stop word removal   │ │
                                    │  └────────────────────────┘ │
                                    │                             │
                                    │  ┌────────────────────────┐ │
                                    │  │    InvertedIndex       │ │
                                    │  │  - Postings lists      │ │
                                    │  │  - Term statistics     │ │
                                    │  │  - Doc lengths         │ │
                                    │  └────────────────────────┘ │
                                    │                             │
                                    │  ┌────────────────────────┐ │
                                    │  │     Bm25Scorer         │ │
                                    │  │  - CPU scoring         │ │
                                    │  │  - GPU batch scoring   │ │
                                    │  │  (via orbit-compute)   │ │
                                    │  └────────────────────────┘ │
                                    └─────────────────────────────┘
```

## Core Data Structures

### FtsQuery - Protocol-Agnostic Query AST

```rust
pub struct FtsQuery {
    /// Query type (Natural, Boolean, Phrase, Prefix, Fuzzy, Wildcard)
    pub query_type: FtsQueryType,
    /// Terms that MUST match (AND)
    pub must_terms: Vec<String>,
    /// Terms that SHOULD match (OR with boost)
    pub should_terms: Vec<String>,
    /// Terms that MUST NOT match (NOT)
    pub must_not_terms: Vec<String>,
    /// Phrase queries (exact sequence)
    pub phrases: Vec<Vec<String>>,
    /// Fields to search (None = all fields)
    pub fields: Option<Vec<String>>,
    /// Maximum results
    pub limit: usize,
    /// Results to skip
    pub offset: usize,
    /// Minimum score threshold
    pub min_score: Option<f32>,
}
```

### FtsDocument - Document Structure

```rust
pub struct FtsDocument {
    /// Unique document identifier
    pub id: String,
    /// Field name to text content mapping
    pub fields: HashMap<String, String>,
    /// Optional metadata (for protocol-specific data)
    pub metadata: Option<serde_json::Value>,
}
```

### FtsSearchResult - Search Result

```rust
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
```

## SIMD Text Processing

### Architecture-Specific Optimizations

| Architecture | SIMD Instructions | Throughput |
|--------------|-------------------|------------|
| x86_64 (AVX2) | 256-bit vectors (32 bytes/op) | ~10GB/s |
| x86_64 (AVX-512) | 512-bit vectors (64 bytes/op) | ~18GB/s |
| ARM64 (NEON) | 128-bit vectors (16 bytes/op) | ~6GB/s |

### Tokenization Algorithm

The SIMD tokenizer uses parallel character classification:

1. **Load 32 bytes** (AVX2) or 16 bytes (NEON) at once
2. **Classify characters** using SIMD range comparisons:
   - `[0-9]`: digit
   - `[A-Z]`: uppercase letter
   - `[a-z]`: lowercase letter
3. **Create word boundary mask** from alphanumeric classification
4. **Extract tokens** based on boundary transitions

```rust
// AVX2 Example (simplified)
let chunk = _mm256_loadu_si256(ptr);
let is_digit = _mm256_and_si256(
    _mm256_cmpgt_epi8(chunk, zero_minus_1),
    _mm256_cmpgt_epi8(nine_plus_1, chunk),
);
let is_alpha = /* similar for A-Z and a-z */;
let is_alnum = _mm256_or_si256(is_digit, is_alpha);
let mask = _mm256_movemask_epi8(is_alnum);
```

## BM25 Scoring

### Formula

```
BM25(D, Q) = Σ IDF(qi) × (f(qi, D) × (k1 + 1)) / (f(qi, D) + k1 × (1 - b + b × |D|/avgdl))
```

Where:
- `f(qi, D)`: term frequency of qi in document D
- `|D|`: document length
- `avgdl`: average document length
- `k1`: term frequency saturation (default: 1.2)
- `b`: length normalization (default: 0.75)
- `IDF(qi) = ln((N - n(qi) + 0.5) / (n(qi) + 0.5) + 1)`

### GPU Acceleration

For large-scale scoring (>1000 documents), the scorer can use GPU via `orbit-compute`:

```rust
// GPU batch scoring interface
fn score_batch_gpu(
    &self,
    query_terms: &[String],
    doc_ids: &[String],
    index: &InvertedIndex,
) -> Vec<(String, f32)>;
```

GPU acceleration benefits:
- **Parallel IDF computation** across all terms
- **Vectorized TF lookup** for batch documents
- **Fused multiply-add** for score accumulation

## Protocol Query Parsers

### MySQL: `parse_mysql_query(query, mode)`

```rust
// Natural mode
parse_mysql_query("hello world", MysqlSearchMode::Natural)
// -> FtsQuery { should_terms: ["hello", "world"], query_type: Natural }

// Boolean mode
parse_mysql_query("+must -exclude optional", MysqlSearchMode::Boolean)
// -> FtsQuery { must_terms: ["must"], must_not_terms: ["exclude"], should_terms: ["optional"] }
```

### PostgreSQL: `parse_postgres_tsquery(query)`

```rust
parse_postgres_tsquery("fat & cat & !dog")
// -> FtsQuery { should_terms: ["fat", "cat"], must_not_terms: ["dog"] }
```

### Redis: `parse_redis_query(query)`

```rust
parse_redis_query("@title:hello -exclude world")
// -> FtsQuery { should_terms: ["hello", "world"], must_not_terms: ["exclude"], fields: ["title"] }
```

### CQL: `parse_cql_query(query, query_type)`

```rust
// CONTAINS
parse_cql_query("fox", CqlQueryType::Contains)
// -> FtsQuery { must_terms: ["fox"], query_type: Natural }

// LIKE 'prefix%'
parse_cql_query("pre%", CqlQueryType::Like)
// -> FtsQuery { must_terms: ["pre"], query_type: Prefix }
```

### MongoDB: `parse_mongodb_text_query(query)`

```rust
parse_mongodb_text_query("\"exact phrase\" -exclude optional")
// -> FtsQuery { phrases: [["exact", "phrase"]], must_not_terms: ["exclude"], should_terms: ["optional"] }
```

## API Reference

### SharedFtsEngine

```rust
impl SharedFtsEngine {
    /// Create a new shared FTS engine
    pub fn new(config: SharedFtsConfig) -> Self;

    /// Create a new index
    pub async fn create_index(&self, name: &str) -> FtsResult<()>;

    /// Drop an index
    pub async fn drop_index(&self, name: &str) -> FtsResult<bool>;

    /// Index a document
    pub async fn index_document(&self, index_name: &str, doc: FtsDocument) -> FtsResult<()>;

    /// Remove a document from an index
    pub async fn remove_document(&self, index_name: &str, doc_id: &str) -> FtsResult<bool>;

    /// Search an index
    pub async fn search(&self, index_name: &str, query: FtsQuery) -> FtsResult<Vec<FtsSearchResult>>;

    /// Get index statistics
    pub async fn get_stats(&self, index_name: &str) -> FtsResult<IndexStats>;

    /// List all indexes
    pub async fn list_indexes(&self) -> Vec<String>;
}
```

### SharedFtsConfig

```rust
pub struct SharedFtsConfig {
    /// Default language for text analysis
    pub default_language: String,          // default: "english"
    /// Enable stemming
    pub enable_stemming: bool,             // default: true
    /// Minimum term length for indexing
    pub min_term_length: usize,            // default: 2
    /// Maximum term length for indexing
    pub max_term_length: usize,            // default: 100
    /// Stop words to filter
    pub stop_words: HashSet<String>,       // default: English stop words
    /// BM25 k1 parameter
    pub bm25_k1: f32,                       // default: 1.2
    /// BM25 b parameter
    pub bm25_b: f32,                        // default: 0.75
    /// Enable SIMD acceleration
    pub enable_simd: bool,                 // default: true
    /// Enable GPU acceleration
    pub enable_gpu: bool,                  // default: false
}
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Tokenize (scalar) | O(n) | Character-by-character |
| Tokenize (SIMD) | O(n/32) | 32 bytes per iteration (AVX2) |
| Index document | O(t) | t = number of unique terms |
| Search | O(q × d × log(n)) | q = query terms, d = matching docs |
| BM25 scoring (CPU) | O(q × d) | Per-document scoring |
| BM25 scoring (GPU) | O(q) | Parallel across documents |

## Usage Examples

### Basic Usage

```rust
use crate::protocols::common::fts::{SharedFtsEngine, FtsDocument, FtsQuery};

// Create engine
let engine = SharedFtsEngine::default();

// Create index
engine.create_index("articles").await?;

// Index documents
engine.index_document("articles", FtsDocument {
    id: "1".to_string(),
    fields: [("title".to_string(), "Rust Programming".to_string())].into(),
    metadata: None,
}).await?;

// Search
let query = FtsQuery {
    should_terms: vec!["rust".to_string()],
    ..Default::default()
};
let results = engine.search("articles", query).await?;
```

### Protocol Integration

```rust
// MySQL integration
let mysql_query = parse_mysql_query("database +programming", MysqlSearchMode::Boolean);
let results = engine.search("articles", mysql_query).await?;

// PostgreSQL integration
let pg_query = parse_postgres_tsquery("database & programming");
let results = engine.search("articles", pg_query).await?;

// MongoDB integration
let mongo_query = parse_mongodb_text_query("\"database programming\" -nosql");
let results = engine.search("articles", mongo_query).await?;
```

## Testing

Run tests with:

```bash
cargo test -p orbit-server fts
```

Test categories:
- Tokenization (scalar and SIMD)
- Stemming
- BM25 scoring
- Query parsing (all protocol syntaxes)
- Index operations (create, drop, add, remove)
- Search functionality

## Future Enhancements

### Planned Features
- [ ] Highlighting with snippet extraction
- [ ] Phrase query with slop
- [ ] Fuzzy matching (Levenshtein distance)
- [ ] Wildcard queries
- [ ] Faceted search
- [ ] Query suggestions/autocomplete
- [ ] Multi-language support
- [ ] Synonyms expansion

### GPU Acceleration Roadmap
- [ ] CUDA kernel for BM25 scoring
- [ ] Metal shader for macOS
- [ ] Vulkan compute for cross-platform
- [ ] Batch tokenization on GPU

## References

- Robertson, S., & Zaragoza, H. (2009). The Probabilistic Relevance Framework: BM25 and Beyond.
- Porter, M. F. (1980). An Algorithm for Suffix Stripping.
- Intel AVX2 Intrinsics Guide
- ARM NEON Intrinsics Reference
