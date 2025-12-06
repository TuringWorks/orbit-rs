//! Query Result Cache
//!
//! This module provides LRU-based caching for query results to improve performance
//! for repeated queries. The cache supports:
//!
//! - Query normalization (parameter substitution for cache keys)
//! - TTL-based expiration
//! - Memory-bounded LRU eviction
//! - Per-table invalidation
//! - Cache statistics

// SqlValue imported for future use in typed cache entries
#[allow(unused_imports)]
use crate::protocols::postgres_wire::sql::types::SqlValue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for query result cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCacheConfig {
    /// Enable query result caching
    pub enabled: bool,
    /// Maximum number of cached queries
    pub max_entries: usize,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,
    /// Default TTL for cache entries
    pub default_ttl_seconds: u64,
    /// Cache SELECT queries only
    pub select_only: bool,
    /// Minimum query execution time (ms) to cache
    pub min_query_time_ms: u64,
}

impl Default for QueryCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            default_ttl_seconds: 60,
            select_only: true,
            min_query_time_ms: 10,
        }
    }
}

/// Cached query result
#[derive(Debug, Clone)]
pub struct CachedResult {
    /// Column names
    pub columns: Vec<String>,
    /// Result rows
    pub rows: Vec<Vec<Option<String>>>,
    /// Row count
    pub row_count: usize,
    /// When the result was cached
    pub cached_at: Instant,
    /// TTL for this entry
    pub ttl: Duration,
    /// Estimated size in bytes
    pub size_bytes: usize,
    /// Tables referenced by this query
    pub referenced_tables: Vec<String>,
    /// Query execution time when cached
    pub execution_time_ms: u64,
}

impl CachedResult {
    /// Check if the cached result has expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }

    /// Estimate memory size of the result
    fn estimate_size(columns: &[String], rows: &[Vec<Option<String>>]) -> usize {
        let mut size = 0;

        // Column names
        for col in columns {
            size += col.len() + std::mem::size_of::<String>();
        }

        // Rows
        for row in rows {
            for cell in row {
                size += std::mem::size_of::<Option<String>>();
                if let Some(s) = cell {
                    size += s.len();
                }
            }
        }

        size
    }
}

/// Normalized query key for cache lookup
#[derive(Debug, Clone, Eq)]
pub struct QueryKey {
    /// Normalized SQL (parameters replaced with placeholders)
    normalized_sql: String,
    /// Current database/schema context
    context: String,
}

impl QueryKey {
    /// Create a new query key from SQL
    pub fn new(sql: &str, database: &str, schema: &str) -> Self {
        let normalized = Self::normalize_sql(sql);
        Self {
            normalized_sql: normalized,
            context: format!("{}.{}", database, schema),
        }
    }

    /// Normalize SQL by replacing literal values with placeholders
    fn normalize_sql(sql: &str) -> String {
        let mut result = String::with_capacity(sql.len());
        let chars: Vec<char> = sql.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            // Skip string literals
            if c == '\'' {
                result.push_str("'?'");
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // Skip closing quote
                }
                continue;
            }

            // Replace numeric literals
            if c.is_ascii_digit()
                || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
            {
                result.push('?');
                if c == '-' {
                    i += 1;
                }
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                continue;
            }

            // Normalize whitespace
            if c.is_whitespace() {
                if !result.ends_with(' ') && !result.is_empty() {
                    result.push(' ');
                }
                i += 1;
                continue;
            }

            // Convert to uppercase for case-insensitive matching
            result.push(c.to_ascii_uppercase());
            i += 1;
        }

        result.trim().to_string()
    }
}

impl PartialEq for QueryKey {
    fn eq(&self, other: &Self) -> bool {
        self.normalized_sql == other.normalized_sql && self.context == other.context
    }
}

impl Hash for QueryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.normalized_sql.hash(state);
        self.context.hash(state);
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of cache insertions
    pub insertions: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Number of invalidations
    pub invalidations: u64,
    /// Current number of cached entries
    pub current_entries: usize,
    /// Current memory usage in bytes
    pub current_memory_bytes: usize,
}

impl CacheStats {
    /// Calculate hit ratio
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// LRU Query Result Cache
pub struct QueryCache {
    /// Cached results
    cache: Arc<RwLock<HashMap<QueryKey, CachedResult>>>,
    /// LRU order (most recent at back)
    lru_order: Arc<RwLock<VecDeque<QueryKey>>>,
    /// Table to queries mapping for invalidation
    table_queries: Arc<RwLock<HashMap<String, Vec<QueryKey>>>>,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Configuration
    config: QueryCacheConfig,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new(config: QueryCacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            table_queries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
            config,
        }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(QueryCacheConfig::default())
    }

    /// Get a cached result
    pub async fn get(&self, key: &QueryKey) -> Option<CachedResult> {
        if !self.config.enabled {
            return None;
        }

        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(result) = cache.get(key) {
            if result.is_expired() {
                // Remove expired entry
                cache.remove(key);
                stats.current_entries = cache.len();
                stats.misses += 1;
                return None;
            }

            // Update LRU order
            let mut lru = self.lru_order.write().await;
            lru.retain(|k| k != key);
            lru.push_back(key.clone());

            stats.hits += 1;
            return Some(result.clone());
        }

        stats.misses += 1;
        None
    }

    /// Put a result in the cache
    pub async fn put(
        &self,
        key: QueryKey,
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
        referenced_tables: Vec<String>,
        execution_time_ms: u64,
    ) {
        if !self.config.enabled {
            return;
        }

        // Check minimum execution time
        if execution_time_ms < self.config.min_query_time_ms {
            return;
        }

        let size_bytes = CachedResult::estimate_size(&columns, &rows);

        // Check if single result is too large
        if size_bytes > self.config.max_memory_bytes / 2 {
            return;
        }

        let row_count = rows.len();
        let result = CachedResult {
            columns,
            rows,
            row_count,
            cached_at: Instant::now(),
            ttl: Duration::from_secs(self.config.default_ttl_seconds),
            size_bytes,
            referenced_tables: referenced_tables.clone(),
            execution_time_ms,
        };

        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;
        let mut table_queries = self.table_queries.write().await;

        // Evict if necessary
        while stats.current_memory_bytes + size_bytes > self.config.max_memory_bytes
            || cache.len() >= self.config.max_entries
        {
            if let Some(oldest_key) = lru.pop_front() {
                if let Some(old_result) = cache.remove(&oldest_key) {
                    stats.current_memory_bytes -= old_result.size_bytes;
                    stats.evictions += 1;
                }
            } else {
                break;
            }
        }

        // Insert new entry
        cache.insert(key.clone(), result);
        lru.push_back(key.clone());

        // Track table references for invalidation
        for table in &referenced_tables {
            table_queries
                .entry(table.clone())
                .or_insert_with(Vec::new)
                .push(key.clone());
        }

        stats.insertions += 1;
        stats.current_entries = cache.len();
        stats.current_memory_bytes += size_bytes;
    }

    /// Invalidate all cache entries for a table
    pub async fn invalidate_table(&self, table_name: &str) {
        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;
        let mut table_queries = self.table_queries.write().await;

        if let Some(keys) = table_queries.remove(table_name) {
            for key in keys {
                if let Some(result) = cache.remove(&key) {
                    stats.current_memory_bytes -= result.size_bytes;
                    stats.invalidations += 1;
                }
                lru.retain(|k| k != &key);
            }
            stats.current_entries = cache.len();
        }
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;
        let mut table_queries = self.table_queries.write().await;

        let count = cache.len();
        cache.clear();
        lru.clear();
        table_queries.clear();

        stats.invalidations += count as u64;
        stats.current_entries = 0;
        stats.current_memory_bytes = 0;
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new_default()
    }
}

/// Helper to extract table names from SQL
pub fn extract_table_names(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let sql_upper = sql.to_uppercase();
    let words: Vec<&str> = sql_upper.split_whitespace().collect();

    let mut i = 0;
    while i < words.len() {
        // Look for FROM, JOIN, INTO, UPDATE table references
        if (words[i] == "FROM" || words[i] == "JOIN" || words[i] == "INTO" || words[i] == "UPDATE")
            && i + 1 < words.len() {
                let table = words[i + 1]
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
                    .to_string();
                if !table.is_empty() && !tables.contains(&table) {
                    tables.push(table);
                }
            }
        i += 1;
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_normalization() {
        let sql1 = "SELECT * FROM users WHERE id = 123";
        let sql2 = "SELECT * FROM users WHERE id = 456";
        let sql3 = "select * from users where id = 789";

        let key1 = QueryKey::new(sql1, "test", "public");
        let key2 = QueryKey::new(sql2, "test", "public");
        let key3 = QueryKey::new(sql3, "test", "public");

        // All should normalize to the same key
        assert_eq!(key1.normalized_sql, key2.normalized_sql);
        assert_eq!(key2.normalized_sql, key3.normalized_sql);
    }

    #[test]
    fn test_query_normalization_strings() {
        let sql1 = "SELECT * FROM users WHERE name = 'Alice'";
        let sql2 = "SELECT * FROM users WHERE name = 'Bob'";

        let key1 = QueryKey::new(sql1, "test", "public");
        let key2 = QueryKey::new(sql2, "test", "public");

        assert_eq!(key1.normalized_sql, key2.normalized_sql);
    }

    #[tokio::test]
    async fn test_cache_put_get() {
        let cache = QueryCache::new_default();
        let key = QueryKey::new("SELECT * FROM users", "test", "public");

        let columns = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec![Some("1".to_string()), Some("Alice".to_string())],
            vec![Some("2".to_string()), Some("Bob".to_string())],
        ];

        cache
            .put(
                key.clone(),
                columns.clone(),
                rows.clone(),
                vec!["USERS".to_string()],
                100, // execution time > min threshold
            )
            .await;

        let result = cache.get(&key).await;
        assert!(result.is_some());

        let cached = result.unwrap();
        assert_eq!(cached.columns, columns);
        assert_eq!(cached.row_count, 2);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = QueryCache::new_default();
        let key = QueryKey::new("SELECT * FROM users", "test", "public");

        cache
            .put(
                key.clone(),
                vec!["id".to_string()],
                vec![vec![Some("1".to_string())]],
                vec!["USERS".to_string()],
                100,
            )
            .await;

        assert!(cache.get(&key).await.is_some());

        cache.invalidate_table("USERS").await;

        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = QueryCache::new_default();
        let key = QueryKey::new("SELECT * FROM users", "test", "public");

        // Miss
        let _ = cache.get(&key).await;

        // Put
        cache
            .put(
                key.clone(),
                vec!["id".to_string()],
                vec![vec![Some("1".to_string())]],
                vec!["USERS".to_string()],
                100,
            )
            .await;

        // Hit
        let _ = cache.get(&key).await;

        let stats = cache.stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.insertions, 1);
        assert!(stats.hit_ratio() > 0.4);
    }

    #[test]
    fn test_extract_table_names() {
        let sql = "SELECT * FROM users JOIN orders ON users.id = orders.user_id";
        let tables = extract_table_names(sql);

        assert!(tables.contains(&"USERS".to_string()));
        assert!(tables.contains(&"ORDERS".to_string()));
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let config = QueryCacheConfig {
            max_entries: 2,
            ..Default::default()
        };
        let cache = QueryCache::new(config);

        // Add 3 entries with distinct table names (not just numbers that get normalized)
        // Using distinct names like users, orders, products instead of t1, t2, t3
        let tables = ["users", "orders", "products"];

        for table in &tables {
            let key = QueryKey::new(&format!("SELECT * FROM {}", table), "test", "public");
            cache
                .put(
                    key,
                    vec!["id".to_string()],
                    vec![vec![Some("1".to_string())]],
                    vec![table.to_uppercase()],
                    100,
                )
                .await;
        }

        // First entry (users) should be evicted
        let key1 = QueryKey::new("SELECT * FROM users", "test", "public");
        assert!(cache.get(&key1).await.is_none());

        // Last two should still exist
        let key2 = QueryKey::new("SELECT * FROM orders", "test", "public");
        let key3 = QueryKey::new("SELECT * FROM products", "test", "public");
        assert!(cache.get(&key2).await.is_some());
        assert!(cache.get(&key3).await.is_some());
    }
}
