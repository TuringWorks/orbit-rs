//! Query result caching with intelligent invalidation for OrbitQL
//!
//! This module provides sophisticated query result caching that can automatically
//! invalidate cached results when underlying data changes, with support for
//! partial cache updates and dependency tracking.

use crate::orbitql::executor::QueryResult;
use crate::orbitql::{QueryContext, QueryParams};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Cache configuration settings
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of cached queries
    pub max_entries: usize,
    /// Maximum memory usage for cache in MB
    pub max_memory_mb: usize,
    /// Default TTL for cached results
    pub default_ttl: Duration,
    /// Enable automatic cache warming
    pub enable_warming: bool,
    /// Enable compression for large results
    pub enable_compression: bool,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// Enable statistics tracking
    pub enable_statistics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_memory_mb: 1024,                    // 1GB
            default_ttl: Duration::from_secs(3600), // 1 hour
            enable_warming: true,
            enable_compression: true,
            eviction_policy: EvictionPolicy::LRU,
            enable_statistics: true,
        }
    }
}

/// Cache eviction policies
#[derive(Debug, Clone)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// Time-based expiration
    TTL,
    /// Adaptive replacement cache
    ARC,
}

/// Cache key that uniquely identifies a query and its parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Normalized query string
    pub query_hash: u64,
    /// Parameter hash
    pub param_hash: u64,
    /// Context hash (user, permissions, etc.)
    pub context_hash: u64,
    /// Query text for debugging
    pub query_text: String,
}

impl CacheKey {
    pub fn new(query: &str, params: &QueryParams, context: &QueryContext) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Normalize query by removing extra whitespace and converting to lowercase
        let normalized_query = normalize_query(query);
        normalized_query.hash(&mut hasher);
        let query_hash = hasher.finish();

        // Hash parameters
        hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{params:?}").hash(&mut hasher); // TODO: Implement proper parameter hashing
        let param_hash = hasher.finish();

        // Hash context (simplified)
        hasher = std::collections::hash_map::DefaultHasher::new();
        context.user_id.hash(&mut hasher);
        context.session_id.hash(&mut hasher);
        // Don't hash transaction_id as it would prevent caching across transactions
        let context_hash = hasher.finish();

        Self {
            query_hash,
            param_hash,
            context_hash,
            query_text: query.to_string(),
        }
    }
}

/// Cached query result with metadata
#[derive(Debug, Clone)]
pub struct CachedResult {
    /// The cached query result
    pub result: QueryResult,
    /// When this result was cached
    pub cached_at: Instant,
    /// When this result expires
    pub expires_at: Instant,
    /// Tables/collections this result depends on
    pub dependencies: HashSet<String>,
    /// Size in bytes (approximate)
    pub size_bytes: usize,
    /// Access statistics
    pub stats: AccessStats,
    /// Cache version for invalidation tracking
    pub version: u64,
}

/// Access statistics for cached entries
#[derive(Debug, Clone, Default)]
pub struct AccessStats {
    /// Number of times accessed
    pub hit_count: u64,
    /// Last access time
    pub last_accessed: Option<Instant>,
    /// Average access frequency
    pub access_frequency: f64,
}

/// Cache invalidation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Table/collection that changed
    pub table: String,
    /// Type of change
    pub change_type: ChangeType,
    /// Timestamp of change
    pub timestamp: SystemTime,
    /// Optional row/document IDs affected
    pub affected_rows: Option<Vec<String>>,
}

/// Types of data changes that trigger invalidation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
    Schema,
    Truncate,
}

/// Cache statistics and metrics
#[derive(Debug, Clone, Default)]
pub struct CacheStatistics {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total invalidations
    pub invalidations: u64,
    /// Total entries evicted
    pub evictions: u64,
    /// Current cache size in bytes
    pub size_bytes: usize,
    /// Number of cached entries
    pub entry_count: usize,
    /// Hit ratio (0.0 to 1.0)
    pub hit_ratio: f64,
}

impl CacheStatistics {
    pub fn update_hit_ratio(&mut self) {
        let total_requests = self.hits + self.misses;
        if total_requests > 0 {
            self.hit_ratio = self.hits as f64 / total_requests as f64;
        }
    }
}

/// Dependency tracker for cache invalidation
#[derive(Debug, Default)]
struct DependencyTracker {
    /// Maps table names to cache keys that depend on them
    table_dependencies: HashMap<String, HashSet<CacheKey>>,
    /// Maps cache keys to tables they depend on
    cache_dependencies: HashMap<CacheKey, HashSet<String>>,
}

impl DependencyTracker {
    /// Add dependency relationship
    fn add_dependency(&mut self, cache_key: CacheKey, tables: HashSet<String>) {
        for table in &tables {
            self.table_dependencies
                .entry(table.clone())
                .or_default()
                .insert(cache_key.clone());
        }
        self.cache_dependencies.insert(cache_key, tables);
    }

    /// Remove all dependencies for a cache key
    fn remove_dependencies(&mut self, cache_key: &CacheKey) {
        if let Some(tables) = self.cache_dependencies.remove(cache_key) {
            for table in tables {
                if let Some(cache_keys) = self.table_dependencies.get_mut(&table) {
                    cache_keys.remove(cache_key);
                    if cache_keys.is_empty() {
                        self.table_dependencies.remove(&table);
                    }
                }
            }
        }
    }

    /// Get all cache keys that depend on a table
    fn get_dependent_keys(&self, table: &str) -> Option<&HashSet<CacheKey>> {
        self.table_dependencies.get(table)
    }
}

/// Main query result cache with intelligent invalidation
pub struct QueryCache {
    /// Cache configuration
    config: CacheConfig,
    /// Cached results
    cache: Arc<RwLock<HashMap<CacheKey, CachedResult>>>,
    /// Dependency tracking
    dependencies: Arc<RwLock<DependencyTracker>>,
    /// Cache statistics
    statistics: Arc<RwLock<CacheStatistics>>,
    /// Current cache version for optimistic invalidation
    cache_version: Arc<RwLock<u64>>,
}

impl QueryCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            dependencies: Arc::new(RwLock::new(DependencyTracker::default())),
            statistics: Arc::new(RwLock::new(CacheStatistics::default())),
            cache_version: Arc::new(RwLock::new(0)),
        }
    }

    /// Get cached result if available and valid
    pub async fn get(&self, key: &CacheKey) -> Option<QueryResult> {
        let mut cache = self.cache.write().await;
        let mut stats = self.statistics.write().await;

        if let Some(cached) = cache.get_mut(key) {
            // Check if result is still valid
            if cached.expires_at > Instant::now() {
                // Update access statistics
                cached.stats.hit_count += 1;
                cached.stats.last_accessed = Some(Instant::now());
                cached.stats.access_frequency =
                    cached.stats.hit_count as f64 / cached.cached_at.elapsed().as_secs_f64();

                stats.hits += 1;
                stats.update_hit_ratio();

                return Some(cached.result.clone());
            } else {
                // Entry expired, remove it
                self.remove_from_cache_internal(&mut cache, key).await;
            }
        }

        stats.misses += 1;
        stats.update_hit_ratio();
        None
    }

    /// Store result in cache
    pub async fn put(
        &self,
        key: CacheKey,
        result: QueryResult,
        dependencies: HashSet<String>,
        ttl: Option<Duration>,
    ) {
        let ttl = ttl.unwrap_or(self.config.default_ttl);
        let now = Instant::now();
        let size_bytes = estimate_result_size(&result);

        // Check if we need to make space
        self.ensure_cache_space(size_bytes).await;

        let cached_result = CachedResult {
            result,
            cached_at: now,
            expires_at: now + ttl,
            dependencies: dependencies.clone(),
            size_bytes,
            stats: AccessStats::default(),
            version: *self.cache_version.read().await,
        };

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(key.clone(), cached_result);
        }

        // Update dependencies
        {
            let mut deps = self.dependencies.write().await;
            deps.add_dependency(key, dependencies);
        }

        // Update statistics
        {
            let mut stats = self.statistics.write().await;
            stats.entry_count = self.cache.read().await.len();
            stats.size_bytes += size_bytes;
        }
    }

    /// Invalidate cache entries based on data changes
    pub async fn invalidate(&self, event: InvalidationEvent) {
        let keys_to_invalidate = {
            let deps = self.dependencies.read().await;
            deps.get_dependent_keys(&event.table)
                .cloned()
                .unwrap_or_default()
        };

        if !keys_to_invalidate.is_empty() {
            let mut cache = self.cache.write().await;
            let mut stats = self.statistics.write().await;
            let _keys_clone: Vec<_> = keys_to_invalidate.iter().cloned().collect();

            for key in &keys_to_invalidate {
                if let Some(cached) = cache.remove(key) {
                    stats.size_bytes = stats.size_bytes.saturating_sub(cached.size_bytes);
                    stats.invalidations += 1;
                }
            }

            // Update dependencies
            let mut deps = self.dependencies.write().await;
            for key in keys_to_invalidate {
                deps.remove_dependencies(&key);
            }

            // Increment cache version for optimistic invalidation
            *self.cache_version.write().await += 1;
        }
    }

    /// Smart invalidation based on query analysis
    pub async fn smart_invalidate(&self, event: InvalidationEvent) {
        match event.change_type {
            ChangeType::Insert => {
                // Inserts may affect aggregation queries and some WHERE clauses
                self.invalidate_selective(&event.table, |cached| {
                    // TODO: Analyze query to see if it could be affected by inserts
                    cached.query_text.contains("COUNT")
                        || cached.query_text.contains("SUM")
                        || cached.query_text.contains("AVG")
                        || cached.query_text.contains("GROUP BY")
                })
                .await;
            }
            ChangeType::Update => {
                // Updates may affect many types of queries
                self.invalidate_selective(&event.table, |_| true).await;
            }
            ChangeType::Delete => {
                // Deletes affect most queries
                self.invalidate_selective(&event.table, |_| true).await;
            }
            ChangeType::Schema => {
                // Schema changes invalidate all queries on the table
                self.invalidate(event).await;
            }
            ChangeType::Truncate => {
                // Truncate invalidates everything
                self.invalidate(event).await;
            }
        }
    }

    /// Selective invalidation based on query analysis
    async fn invalidate_selective<F>(&self, table: &str, predicate: F)
    where
        F: Fn(&CacheKey) -> bool,
    {
        let keys_to_check = {
            let deps = self.dependencies.read().await;
            deps.get_dependent_keys(table).cloned().unwrap_or_default()
        };

        let keys_to_invalidate: Vec<CacheKey> = keys_to_check
            .into_iter()
            .filter(|key| predicate(key))
            .collect();

        if !keys_to_invalidate.is_empty() {
            let mut cache = self.cache.write().await;
            let mut stats = self.statistics.write().await;

            for key in &keys_to_invalidate {
                if let Some(cached) = cache.remove(key) {
                    stats.size_bytes = stats.size_bytes.saturating_sub(cached.size_bytes);
                    stats.entry_count = stats.entry_count.saturating_sub(1);
                    stats.invalidations += 1;
                }
            }

            // Update dependencies
            let mut deps = self.dependencies.write().await;
            for key in keys_to_invalidate {
                deps.remove_dependencies(&key);
            }
        }
    }

    /// Clear all cached results
    pub async fn clear(&self) {
        self.cache.write().await.clear();
        *self.dependencies.write().await = DependencyTracker::default();
        let mut stats = self.statistics.write().await;
        stats.entry_count = 0;
        stats.size_bytes = 0;
        *self.cache_version.write().await += 1;
    }

    /// Get cache statistics
    pub async fn get_statistics(&self) -> CacheStatistics {
        self.statistics.read().await.clone()
    }

    /// Warm cache with frequently accessed queries
    pub async fn warm_cache(&self, _queries: Vec<(String, QueryParams, QueryContext)>) {
        if !self.config.enable_warming {}

        // TODO: Implement cache warming by executing queries and storing results
        // This would be done in the background to avoid impacting performance
    }

    /// Ensure there's enough space in the cache
    async fn ensure_cache_space(&self, needed_bytes: usize) {
        let stats = self.statistics.read().await;
        let max_bytes = self.config.max_memory_mb * 1024 * 1024;

        if stats.size_bytes + needed_bytes > max_bytes
            || stats.entry_count >= self.config.max_entries
        {
            drop(stats); // Release read lock
            self.evict_entries(needed_bytes).await;
        }
    }

    /// Evict entries based on configured policy
    async fn evict_entries(&self, needed_bytes: usize) {
        let mut cache = self.cache.write().await;
        let mut stats = self.statistics.write().await;
        let mut bytes_freed = 0;

        // Collect entries for eviction based on policy
        let mut entries: Vec<_> = cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        match self.config.eviction_policy {
            EvictionPolicy::LRU => {
                entries.sort_by(|a, b| {
                    let a_last = a.1.stats.last_accessed.unwrap_or(a.1.cached_at);
                    let b_last = b.1.stats.last_accessed.unwrap_or(b.1.cached_at);
                    a_last.cmp(&b_last)
                });
            }
            EvictionPolicy::LFU => {
                entries.sort_by(|a, b| a.1.stats.hit_count.cmp(&b.1.stats.hit_count));
            }
            EvictionPolicy::TTL => {
                entries.sort_by(|a, b| a.1.expires_at.cmp(&b.1.expires_at));
            }
            EvictionPolicy::ARC => {
                // TODO: Implement ARC eviction
                entries.sort_by(|a, b| {
                    a.1.stats
                        .access_frequency
                        .partial_cmp(&b.1.stats.access_frequency)
                        .unwrap()
                });
            }
        }

        // Evict entries until we have enough space
        for (key, cached) in entries {
            if bytes_freed >= needed_bytes && cache.len() < self.config.max_entries {
                break;
            }

            bytes_freed += cached.size_bytes;
            cache.remove(&key);
            stats.evictions += 1;
        }

        stats.entry_count = cache.len();
        stats.size_bytes = stats.size_bytes.saturating_sub(bytes_freed);
    }

    /// Remove entry from cache (internal helper)
    async fn remove_from_cache_internal(
        &self,
        cache: &mut HashMap<CacheKey, CachedResult>,
        key: &CacheKey,
    ) {
        if let Some(cached) = cache.remove(key) {
            let mut stats = self.statistics.write().await;
            stats.size_bytes = stats.size_bytes.saturating_sub(cached.size_bytes);
            stats.entry_count = stats.entry_count.saturating_sub(1);

            // Update dependencies
            let mut deps = self.dependencies.write().await;
            deps.remove_dependencies(key);
        }
    }

    /// Get cache health information
    pub async fn get_health(&self) -> CacheHealth {
        let stats = self.statistics.read().await;
        let cache_size = self.cache.read().await.len();

        CacheHealth {
            hit_ratio: stats.hit_ratio,
            memory_usage_percent: (stats.size_bytes as f64
                / (self.config.max_memory_mb * 1024 * 1024) as f64)
                * 100.0,
            entry_count: cache_size,
            max_entries: self.config.max_entries,
            eviction_rate: if stats.hits + stats.misses > 0 {
                stats.evictions as f64 / (stats.hits + stats.misses) as f64
            } else {
                0.0
            },
            invalidation_rate: if stats.hits + stats.misses > 0 {
                stats.invalidations as f64 / (stats.hits + stats.misses) as f64
            } else {
                0.0
            },
        }
    }
}

/// Cache health metrics
#[derive(Debug, Clone)]
pub struct CacheHealth {
    pub hit_ratio: f64,
    pub memory_usage_percent: f64,
    pub entry_count: usize,
    pub max_entries: usize,
    pub eviction_rate: f64,
    pub invalidation_rate: f64,
}

/// Normalize query string for consistent caching
fn normalize_query(query: &str) -> String {
    query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Estimate the memory size of a query result
fn estimate_result_size(_result: &QueryResult) -> usize {
    // This is a rough estimation
    // In a real implementation, you'd traverse the result structure
    1024 // Placeholder: 1KB per result
}

/// Cache-aware query executor wrapper
pub struct CachedQueryExecutor<T> {
    inner_executor: T,
    cache: Arc<QueryCache>,
}

impl<T> CachedQueryExecutor<T>
where
    T: QueryExecutorTrait + Send + Sync,
{
    pub fn new(inner_executor: T, cache_config: CacheConfig) -> Self {
        Self {
            inner_executor,
            cache: Arc::new(QueryCache::new(cache_config)),
        }
    }

    /// Execute query with caching
    pub async fn execute_cached(
        &self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<QueryResult, crate::orbitql::ExecutionError> {
        let cache_key = CacheKey::new(query, &params, &context);

        // Try to get from cache first
        if let Some(cached_result) = self.cache.get(&cache_key).await {
            return Ok(cached_result);
        }

        // Execute query
        let result = self.inner_executor.execute(query, params, context).await?;

        // Analyze query to determine dependencies
        let dependencies = analyze_query_dependencies(query);

        // Store in cache
        self.cache
            .put(cache_key, result.clone(), dependencies, None)
            .await;

        Ok(result)
    }

    /// Notify cache of data changes
    pub async fn notify_data_change(&self, event: InvalidationEvent) {
        self.cache.smart_invalidate(event).await;
    }

    /// Get cache statistics
    pub async fn get_cache_statistics(&self) -> CacheStatistics {
        self.cache.get_statistics().await
    }

    /// Get cache health
    pub async fn get_cache_health(&self) -> CacheHealth {
        self.cache.get_health().await
    }
}

/// Trait for query executors to enable caching wrapper
#[allow(async_fn_in_trait)]
pub trait QueryExecutorTrait {
    async fn execute(
        &self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<QueryResult, crate::orbitql::ExecutionError>;
}

/// Analyze query to determine table dependencies
fn analyze_query_dependencies(query: &str) -> HashSet<String> {
    let mut dependencies = HashSet::new();

    // Simple analysis - look for FROM, JOIN, and table references
    let normalized = query.to_lowercase();

    // TODO: Implement proper SQL/OrbitQL parsing for dependency extraction
    // This is a simplified version that looks for common patterns

    if let Some(from_pos) = normalized.find(" from ") {
        let after_from = &normalized[from_pos + 6..];
        if let Some(next_keyword) = after_from
            .find(" where ")
            .or_else(|| after_from.find(" order "))
            .or_else(|| after_from.find(" group "))
            .or_else(|| after_from.find(" limit "))
            .or_else(|| after_from.find(";"))
        {
            let table_part = &after_from[..next_keyword];
            let table_name = table_part.split_whitespace().next().unwrap_or("");
            if !table_name.is_empty() {
                dependencies.insert(table_name.to_string());
            }
        }
    }

    // Look for JOIN clauses
    let mut search_pos = 0;
    while let Some(join_pos) = normalized[search_pos..].find(" join ") {
        let absolute_pos = search_pos + join_pos + 6;
        let after_join = &normalized[absolute_pos..];
        if let Some(next_space) = after_join.find(' ') {
            let table_name = &after_join[..next_space];
            if !table_name.is_empty() {
                dependencies.insert(table_name.to_string());
            }
        }
        search_pos = absolute_pos + 1;
    }

    // Also look for table aliases (e.g., "users u")
    if let Some(from_pos) = normalized.find(" from ") {
        let after_from = &normalized[from_pos + 6..];
        let words: Vec<&str> = after_from.split_whitespace().collect();
        if !words.is_empty() {
            dependencies.insert(words[0].to_string());
        }
    }

    dependencies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_creation() {
        let query = "SELECT * FROM users WHERE active = true";
        let params = QueryParams::new();
        let context = QueryContext::default();

        let key1 = CacheKey::new(query, &params, &context);
        let key2 = CacheKey::new(query, &params, &context);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_query_normalization() {
        let query1 = "  SELECT   *   FROM   users  ";
        let query2 = "select * from users";

        assert_eq!(normalize_query(query1), normalize_query(query2));
    }

    #[test]
    fn test_dependency_analysis() {
        let query = "SELECT u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id";
        let deps = analyze_query_dependencies(query);

        assert!(deps.contains("users"));
        assert!(deps.contains("posts"));
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key = CacheKey {
            query_hash: 123,
            param_hash: 456,
            context_hash: 789,
            query_text: "test query".to_string(),
        };

        // Initially empty
        assert!(cache.get(&key).await.is_none());

        // Store result
        let result = QueryResult::default();
        let deps = HashSet::from(["users".to_string()]);
        cache.put(key.clone(), result, deps, None).await;

        // Should now be cached
        assert!(cache.get(&key).await.is_some());

        // Invalidate
        let event = InvalidationEvent {
            event_id: Uuid::new_v4(),
            table: "users".to_string(),
            change_type: ChangeType::Update,
            timestamp: SystemTime::now(),
            affected_rows: None,
        };

        cache.invalidate(event).await;

        // Should be invalidated
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key = CacheKey {
            query_hash: 123,
            param_hash: 456,
            context_hash: 789,
            query_text: "test query".to_string(),
        };

        // Miss
        cache.get(&key).await;
        let stats = cache.get_statistics().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Store and hit
        let result = QueryResult::default();
        let deps = HashSet::new();
        cache.put(key.clone(), result, deps, None).await;
        cache.get(&key).await;

        let stats = cache.get_statistics().await;
        assert_eq!(stats.hits, 1);
        assert!(stats.hit_ratio > 0.0);
    }
}
