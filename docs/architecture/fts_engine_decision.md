# Full-Text Search Engine Decision Document

**Date**: 2025-12-08
**Decision**: Tantivy
**Status**: Recommended for Approval

---

## Decision Summary

After evaluating 6 Rust full-text search libraries, we recommend **Tantivy** as the primary FTS engine for OrbitRS's compatibility layer (PostgreSQL, MySQL, MongoDB).

---

## Evaluation Summary

| Library | Score | Recommendation |
|---------|-------|----------------|
| **Tantivy** | **9.3/10** | ✅ **Recommended** |
| MeiliSearch | 8.3/10 | ❌ Not embeddable |
| Sonic | 7.7/10 | ❌ Too limited |
| pg_search | 7.0/10 | ❌ PostgreSQL-only |
| lnx | 6.7/10 | ❌ Less mature |
| tinysearch | 5.7/10 | ❌ Static only |

---

## Why Tantivy?

### Strengths

1. **Comprehensive Features** (10/10)
   - Fuzzy search, phrase search, proximity search
   - BM25 ranking algorithm
   - Faceted search
   - Multiple query types
   - Language-specific analyzers

2. **Performance** (9/10)
   - Near-Lucene speeds
   - > 10,000 docs/sec indexing
   - < 10ms query latency
   - Efficient memory usage

3. **Maturity** (9/10)
   - Battle-tested in production
   - Powers Quickwit (distributed search)
   - Powers Toshi (Elasticsearch alternative)
   - Active development since 2015

4. **Pure Rust** (10/10)
   - Memory-safe
   - No C/C++ dependencies
   - Easy integration

5. **Flexibility** (10/10)
   - Adaptable to PostgreSQL, MySQL, MongoDB
   - Custom schema support
   - Extensible tokenizers

### Trade-offs

**Accepted**:
- ~5MB binary size (acceptable for features)
- Medium API complexity (manageable)
- Index management required (standard for FTS)

**Rejected Alternatives**:
- **MeiliSearch**: Standalone server, not embeddable
- **Sonic**: Too limited (no ranking, basic features)
- **pg_search**: PostgreSQL-only, limited scope
- **lnx**: Less mature, smaller community
- **tinysearch**: Static indexes only, too basic

---

## Protocol Mapping

### PostgreSQL → Tantivy

**Features**:
- `tsvector` → Tantivy document
- `tsquery` → Tantivy query
- `@@` operator → Tantivy search
- GIN index → Tantivy index
- `ts_rank()` → BM25 score

**Example**:
```sql
-- PostgreSQL
SELECT * FROM docs
WHERE to_tsvector('english', content) @@ to_tsquery('search & query');

-- Tantivy (internal)
query_parser.parse_query("search AND query")
```

### MySQL → Tantivy

**Features**:
- FULLTEXT index → Tantivy index
- `MATCH() AGAINST()` → Tantivy search
- Boolean mode → Boolean query
- Natural language → Standard query

**Example**:
```sql
-- MySQL
SELECT * FROM articles
WHERE MATCH(title, body) AGAINST('+must -exclude' IN BOOLEAN MODE);

-- Tantivy (internal)
BooleanQuery::new(must, must_not, should)
```

### MongoDB → Tantivy

**Features**:
- Text index → Tantivy index
- `$text` operator → Tantivy search
- Language support → Tantivy analyzers
- Text score → BM25 score

**Example**:
```javascript
// MongoDB
db.articles.find({ $text: { $search: "search query" } });

// Tantivy (internal)
query_parser.parse_query("search query")
```

---

## Implementation Strategy

### Phase 1: Foundation (Weeks 1-2)
- Integrate Tantivy
- Create FTS engine module
- Implement index management
- Add schema registry

### Phase 2: PostgreSQL (Weeks 3-4)
- `tsvector`/`tsquery` types
- `@@` operator
- GIN index support
- Ranking functions

### Phase 3: MySQL (Weeks 5-6)
- FULLTEXT index
- `MATCH() AGAINST()`
- Boolean mode
- Relevance scoring

### Phase 4: MongoDB (Weeks 7-8)
- Text indexes
- `$text` operator
- Language support
- Text score metadata

### Phase 5: Advanced (Weeks 9-10)
- Fuzzy search
- Phrase search
- Faceted search
- Highlighting

### Phase 6: Optimization (Weeks 11-12)
- Performance tuning
- Testing
- Documentation
- Benchmarking

---

## Performance Targets

| Metric | Target | Tantivy Capability |
|--------|--------|-------------------|
| Indexing speed | > 10,000 docs/sec | ✅ Achievable |
| Query latency (p95) | < 10ms | ✅ Achievable |
| Throughput | > 1,000 queries/sec | ✅ Achievable |
| Index size | < 50% of data | ✅ Typical ~30% |
| Memory per 1M docs | < 100MB | ✅ Achievable |

---

## Security Considerations

**Protections**:
1. Query sanitization (escape special chars)
2. Query length limits
3. Result count limits
4. Execution time limits
5. Memory limits

**Implementation**:
```rust
pub struct SearchLimits {
    max_query_length: 1000,
    max_results: 10000,
    max_query_time: Duration::from_secs(5),
}
```

---

## Dependencies

```toml
[dependencies]
tantivy = "0.21"
tantivy-jieba = "0.10"  # Chinese support
tantivy-analysis = "0.21"  # Additional languages

[features]
fts = ["dep:tantivy"]
fts-chinese = ["dep:tantivy-jieba"]
```

**Binary Size Impact**: ~5MB

---

## Risks & Mitigation

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Performance insufficient | High | Low | Benchmarking, optimization |
| Index size too large | Medium | Low | Compression, tuning |
| Query compatibility | Medium | Medium | Extensive testing |
| Memory usage | Medium | Low | Limits, monitoring |

---

## Success Criteria

**Functional**:
- [ ] PostgreSQL FTS queries work correctly
- [ ] MySQL FULLTEXT searches work correctly
- [ ] MongoDB text searches work correctly
- [ ] All query types supported
- [ ] Ranking produces relevant results

**Performance**:
- [ ] Meets all performance targets
- [ ] No memory leaks
- [ ] Scales to millions of documents
- [ ] Concurrent searches supported

**Quality**:
- [ ] 90%+ test coverage
- [ ] Zero security vulnerabilities
- [ ] Complete documentation
- [ ] Published benchmarks

---

## Approval

**Recommended**: ✅ Approve Tantivy integration

**Next Steps**:
1. Add Tantivy dependency
2. Create FTS module structure
3. Implement PostgreSQL FTS (Phase 2)
4. Benchmark and validate

---

## References

- [Tantivy GitHub](https://github.com/quickwit-oss/tantivy)
- [Tantivy Documentation](https://docs.rs/tantivy/)
- [Quickwit (Tantivy-based)](https://quickwit.io/)
- [Toshi (Tantivy-based)](https://github.com/toshi-search/Toshi)
