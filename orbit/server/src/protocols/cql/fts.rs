// CQL Full-Text Search Integration
//
// Implements Cassandra-compatible secondary index features:
// - SASI (SSTable Attached Secondary Index) - compatible text search
// - SAI (Storage Attached Index) - modern text search
// - Support for CONTAINS, LIKE, and CONTAINS KEY operators
// - Analyzer modes: standard, non_tokenizing, case_insensitive

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// CQL FTS integration (SASI/SAI compatible)
pub struct CqlFts {
    /// In-memory text storage for FTS
    text_store: Arc<RwLock<HashMap<String, SasiIndex>>>,
}

/// Index mode (SASI compatible)
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzerMode {
    /// Standard tokenizing analyzer (word-based)
    Standard,
    /// Non-tokenizing analyzer (prefix/suffix search)
    NonTokenizing,
    /// Case-insensitive matching
    CaseInsensitive,
}

/// Index configuration
#[derive(Debug, Clone)]
pub struct SasiIndexConfig {
    pub mode: AnalyzerMode,
    pub analyzed: bool,
    pub max_compaction_flush_memory_in_mb: usize,
    pub case_sensitive: bool,
}

impl Default for SasiIndexConfig {
    fn default() -> Self {
        Self {
            mode: AnalyzerMode::Standard,
            analyzed: true,
            max_compaction_flush_memory_in_mb: 1024,
            case_sensitive: true,
        }
    }
}

/// SASI-compatible text index
struct SasiIndex {
    /// Index configuration
    config: SasiIndexConfig,
    /// Column being indexed
    column: String,
    /// Documents: partition_key -> field value
    documents: HashMap<String, String>,
    /// Inverted index: term -> [partition_keys]
    inverted_index: HashMap<String, Vec<String>>,
    /// Prefix index for LIKE 'prefix%' queries
    prefix_index: HashMap<String, Vec<String>>,
    /// Suffix index for LIKE '%suffix' queries
    suffix_index: HashMap<String, Vec<String>>,
}

impl SasiIndex {
    fn new(column: String, config: SasiIndexConfig) -> Self {
        Self {
            config,
            column,
            documents: HashMap::new(),
            inverted_index: HashMap::new(),
            prefix_index: HashMap::new(),
            suffix_index: HashMap::new(),
        }
    }

    /// Tokenize text based on analyzer mode
    fn tokenize(&self, text: &str) -> Vec<String> {
        let text = if self.config.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

        match self.config.mode {
            AnalyzerMode::Standard => {
                // Standard tokenizer: split on whitespace and punctuation
                text.split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
            AnalyzerMode::NonTokenizing => {
                // Non-tokenizing: use full value as-is
                vec![text]
            }
            AnalyzerMode::CaseInsensitive => {
                // Case insensitive: lowercase tokens
                text.to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
        }
    }

    /// Add document to index
    fn add_document(&mut self, partition_key: &str, value: &str) {
        let normalized_value = if self.config.case_sensitive {
            value.to_string()
        } else {
            value.to_lowercase()
        };

        // Store document
        self.documents
            .insert(partition_key.to_string(), value.to_string());

        // Index tokens
        let tokens = self.tokenize(value);
        for token in &tokens {
            self.inverted_index
                .entry(token.clone())
                .or_insert_with(Vec::new)
                .push(partition_key.to_string());
        }

        // Build prefix index (for LIKE 'prefix%')
        for prefix_len in 1..=normalized_value.len().min(10) {
            let prefix = &normalized_value[..prefix_len];
            self.prefix_index
                .entry(prefix.to_string())
                .or_insert_with(Vec::new)
                .push(partition_key.to_string());
        }

        // Build suffix index (for LIKE '%suffix')
        for suffix_len in 1..=normalized_value.len().min(10) {
            let start = normalized_value.len().saturating_sub(suffix_len);
            let suffix = &normalized_value[start..];
            self.suffix_index
                .entry(suffix.to_string())
                .or_insert_with(Vec::new)
                .push(partition_key.to_string());
        }
    }

    /// Delete document from index
    fn delete_document(&mut self, partition_key: &str) -> bool {
        if let Some(value) = self.documents.remove(partition_key) {
            // Remove from inverted index
            let tokens = self.tokenize(&value);
            for token in tokens {
                if let Some(keys) = self.inverted_index.get_mut(&token) {
                    keys.retain(|k| k != partition_key);
                }
            }

            // Remove from prefix/suffix indices
            for keys in self.prefix_index.values_mut() {
                keys.retain(|k| k != partition_key);
            }
            for keys in self.suffix_index.values_mut() {
                keys.retain(|k| k != partition_key);
            }

            true
        } else {
            false
        }
    }

    /// CONTAINS search (term must be present)
    fn search_contains(&self, term: &str) -> Vec<SearchResult> {
        let term = if self.config.case_sensitive {
            term.to_string()
        } else {
            term.to_lowercase()
        };

        let partition_keys = self
            .inverted_index
            .get(&term)
            .cloned()
            .unwrap_or_default();

        partition_keys
            .into_iter()
            .filter_map(|pk| {
                self.documents.get(&pk).map(|value| SearchResult {
                    partition_key: pk,
                    value: value.clone(),
                    score: 1.0,
                })
            })
            .collect()
    }

    /// LIKE search with pattern matching
    fn search_like(&self, pattern: &str) -> Vec<SearchResult> {
        let pattern = if self.config.case_sensitive {
            pattern.to_string()
        } else {
            pattern.to_lowercase()
        };

        // Determine pattern type
        let starts_with_wildcard = pattern.starts_with('%');
        let ends_with_wildcard = pattern.ends_with('%');

        let core_pattern = pattern.trim_matches('%');

        let partition_keys: Vec<String> = if starts_with_wildcard && ends_with_wildcard {
            // %term% - contains anywhere
            self.documents
                .iter()
                .filter(|(_, v)| {
                    let v = if self.config.case_sensitive {
                        v.to_string()
                    } else {
                        v.to_lowercase()
                    };
                    v.contains(core_pattern)
                })
                .map(|(k, _)| k.clone())
                .collect()
        } else if starts_with_wildcard {
            // %suffix - ends with
            self.suffix_index
                .get(core_pattern)
                .cloned()
                .unwrap_or_else(|| {
                    // Fallback to full scan
                    self.documents
                        .iter()
                        .filter(|(_, v)| {
                            let v = if self.config.case_sensitive {
                                v.to_string()
                            } else {
                                v.to_lowercase()
                            };
                            v.ends_with(core_pattern)
                        })
                        .map(|(k, _)| k.clone())
                        .collect()
                })
        } else if ends_with_wildcard {
            // prefix% - starts with
            self.prefix_index
                .get(core_pattern)
                .cloned()
                .unwrap_or_else(|| {
                    // Fallback to full scan
                    self.documents
                        .iter()
                        .filter(|(_, v)| {
                            let v = if self.config.case_sensitive {
                                v.to_string()
                            } else {
                                v.to_lowercase()
                            };
                            v.starts_with(core_pattern)
                        })
                        .map(|(k, _)| k.clone())
                        .collect()
                })
        } else {
            // Exact match
            self.documents
                .iter()
                .filter(|(_, v)| {
                    let v = if self.config.case_sensitive {
                        v.to_string()
                    } else {
                        v.to_lowercase()
                    };
                    v == core_pattern
                })
                .map(|(k, _)| k.clone())
                .collect()
        };

        partition_keys
            .into_iter()
            .filter_map(|pk| {
                self.documents.get(&pk).map(|value| SearchResult {
                    partition_key: pk,
                    value: value.clone(),
                    score: 1.0,
                })
            })
            .collect()
    }

    /// Full-text search with TF-IDF scoring
    fn search_fulltext(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_terms = self.tokenize(query);
        if query_terms.is_empty() {
            return vec![];
        }

        let num_docs = self.documents.len() as f32;
        let mut doc_scores: HashMap<String, f32> = HashMap::new();

        for term in &query_terms {
            if let Some(partition_keys) = self.inverted_index.get(term) {
                // IDF = log(N / df)
                let idf = (num_docs / partition_keys.len() as f32).ln().max(0.0) + 1.0;

                for pk in partition_keys {
                    *doc_scores.entry(pk.clone()).or_insert(0.0) += idf;
                }
            }
        }

        // Sort by score
        let mut results: Vec<(String, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results
            .into_iter()
            .take(limit)
            .filter_map(|(pk, score)| {
                self.documents.get(&pk).map(|value| SearchResult {
                    partition_key: pk,
                    value: value.clone(),
                    score,
                })
            })
            .collect()
    }
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub partition_key: String,
    pub value: String,
    pub score: f32,
}

impl CqlFts {
    pub fn new() -> Self {
        Self {
            text_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create SASI index
    /// CQL: CREATE CUSTOM INDEX ON table(column) USING 'org.apache.cassandra.index.sasi.SASIIndex'
    pub async fn create_sasi_index(
        &self,
        table_name: &str,
        column: &str,
        config: SasiIndexConfig,
    ) -> anyhow::Result<()> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let mut store = self.text_store.write().await;
        store.insert(index_name, SasiIndex::new(column.to_string(), config));

        Ok(())
    }

    /// Create SAI index (newer, simpler API)
    /// CQL: CREATE CUSTOM INDEX ON table(column) USING 'StorageAttachedIndex'
    pub async fn create_sai_index(
        &self,
        table_name: &str,
        column: &str,
        case_sensitive: bool,
    ) -> anyhow::Result<()> {
        let config = SasiIndexConfig {
            mode: AnalyzerMode::Standard,
            analyzed: true,
            case_sensitive,
            ..Default::default()
        };

        self.create_sasi_index(table_name, column, config).await
    }

    /// Add document to index
    pub async fn add_document(
        &self,
        table_name: &str,
        column: &str,
        partition_key: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let mut store = self.text_store.write().await;
        if let Some(index) = store.get_mut(&index_name) {
            index.add_document(partition_key, value);
        }

        Ok(())
    }

    /// Delete document from index
    pub async fn delete_document(
        &self,
        table_name: &str,
        column: &str,
        partition_key: &str,
    ) -> anyhow::Result<bool> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let mut store = self.text_store.write().await;
        if let Some(index) = store.get_mut(&index_name) {
            return Ok(index.delete_document(partition_key));
        }

        Ok(false)
    }

    /// CONTAINS search
    /// CQL: SELECT * FROM table WHERE column CONTAINS 'term'
    pub async fn search_contains(
        &self,
        table_name: &str,
        column: &str,
        term: &str,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(index.search_contains(term));
        }

        Ok(vec![])
    }

    /// LIKE search
    /// CQL: SELECT * FROM table WHERE column LIKE 'pattern%'
    pub async fn search_like(
        &self,
        table_name: &str,
        column: &str,
        pattern: &str,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(index.search_like(pattern));
        }

        Ok(vec![])
    }

    /// Full-text search with scoring
    /// CQL: SELECT * FROM table WHERE column : 'search query'
    pub async fn search_fulltext(
        &self,
        table_name: &str,
        column: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(index.search_fulltext(query, limit));
        }

        Ok(vec![])
    }

    /// Get index info
    pub async fn get_index_info(
        &self,
        table_name: &str,
        column: &str,
    ) -> anyhow::Result<Option<IndexInfo>> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(Some(IndexInfo {
                name: index_name,
                column: index.column.clone(),
                mode: format!("{:?}", index.config.mode),
                num_documents: index.documents.len(),
                num_terms: index.inverted_index.len(),
            }));
        }

        Ok(None)
    }

    /// Drop index
    pub async fn drop_index(&self, table_name: &str, column: &str) -> anyhow::Result<bool> {
        let index_name = format!("{}_{}_sasi", table_name, column);

        let mut store = self.text_store.write().await;
        Ok(store.remove(&index_name).is_some())
    }
}

impl Default for CqlFts {
    fn default() -> Self {
        Self::new()
    }
}

/// Index information
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub column: String,
    pub mode: String,
    pub num_documents: usize,
    pub num_terms: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sasi_contains() {
        let fts = CqlFts::new();

        fts.create_sasi_index("articles", "content", SasiIndexConfig::default())
            .await
            .unwrap();

        fts.add_document("articles", "content", "1", "The quick brown fox jumps")
            .await
            .unwrap();
        fts.add_document("articles", "content", "2", "A lazy dog sleeps")
            .await
            .unwrap();
        fts.add_document("articles", "content", "3", "The fox is quick and clever")
            .await
            .unwrap();

        let results = fts.search_contains("articles", "content", "fox").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_sasi_like() {
        let fts = CqlFts::new();

        fts.create_sasi_index(
            "users",
            "email",
            SasiIndexConfig {
                mode: AnalyzerMode::NonTokenizing,
                case_sensitive: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        fts.add_document("users", "email", "1", "alice@example.com")
            .await
            .unwrap();
        fts.add_document("users", "email", "2", "bob@example.com")
            .await
            .unwrap();
        fts.add_document("users", "email", "3", "charlie@test.org")
            .await
            .unwrap();

        // Prefix search
        let results = fts.search_like("users", "email", "alice%").await.unwrap();
        assert_eq!(results.len(), 1);

        // Suffix search
        let results = fts.search_like("users", "email", "%example.com").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_fulltext_search() {
        let fts = CqlFts::new();

        fts.create_sasi_index("posts", "body", SasiIndexConfig::default())
            .await
            .unwrap();

        fts.add_document("posts", "body", "1", "Rust is a systems programming language")
            .await
            .unwrap();
        fts.add_document("posts", "body", "2", "Python is great for data science")
            .await
            .unwrap();
        fts.add_document("posts", "body", "3", "Rust and Python are both popular")
            .await
            .unwrap();

        let results = fts
            .search_fulltext("posts", "body", "rust programming", 10)
            .await
            .unwrap();

        // Should find docs mentioning rust or programming, with doc 1 scoring highest
        assert!(!results.is_empty());
        assert_eq!(results[0].partition_key, "1");
    }
}
