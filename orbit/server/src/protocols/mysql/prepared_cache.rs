// Prepared statement cache for MySQL protocol
//
// Provides caching of prepared statements to improve performance
// by avoiding repeated parsing and planning of common queries.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cached prepared statement
#[derive(Debug, Clone)]
pub struct CachedStatement {
    /// Statement ID
    pub statement_id: u32,
    /// SQL query text
    pub query: String,
    /// Parameter count
    pub param_count: u16,
    /// Column count
    pub column_count: u16,
    /// Last access time
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
}

/// Prepared statement cache configuration
#[derive(Debug, Clone)]
pub struct PreparedStatementCacheConfig {
    /// Maximum number of cached statements
    pub max_size: usize,
    /// Time-to-live for cached statements
    pub ttl: Duration,
    /// Enable LRU eviction
    pub enable_lru: bool,
}

impl Default for PreparedStatementCacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
            enable_lru: true,
        }
    }
}

/// Prepared statement cache
pub struct PreparedStatementCache {
    config: PreparedStatementCacheConfig,
    statements: Arc<RwLock<HashMap<u32, CachedStatement>>>,
    query_to_id: Arc<RwLock<HashMap<String, u32>>>,
    next_id: Arc<RwLock<u32>>,
}

impl PreparedStatementCache {
    /// Create a new prepared statement cache
    pub fn new(config: PreparedStatementCacheConfig) -> Self {
        Self {
            config,
            statements: Arc::new(RwLock::new(HashMap::new())),
            query_to_id: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Get or create a prepared statement
    pub fn get_or_prepare(&self, query: &str, param_count: u16, column_count: u16) -> u32 {
        // Check if statement already exists
        {
            let query_map = self.query_to_id.read().unwrap();
            if let Some(&statement_id) = query_map.get(query) {
                // Update access time and count
                if let Ok(mut statements) = self.statements.write() {
                    if let Some(stmt) = statements.get_mut(&statement_id) {
                        stmt.last_accessed = Instant::now();
                        stmt.access_count += 1;
                    }
                }
                return statement_id;
            }
        }

        // Create new statement
        let statement_id = {
            let mut next_id = self.next_id.write().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let cached_stmt = CachedStatement {
            statement_id,
            query: query.to_string(),
            param_count,
            column_count,
            last_accessed: Instant::now(),
            access_count: 1,
        };

        // Check if we need to evict
        self.maybe_evict();

        // Insert new statement
        self.statements
            .write()
            .unwrap()
            .insert(statement_id, cached_stmt);
        self.query_to_id
            .write()
            .unwrap()
            .insert(query.to_string(), statement_id);

        statement_id
    }

    /// Get a cached statement by ID
    pub fn get(&self, statement_id: u32) -> Option<CachedStatement> {
        let mut statements = self.statements.write().unwrap();
        if let Some(stmt) = statements.get_mut(&statement_id) {
            stmt.last_accessed = Instant::now();
            stmt.access_count += 1;
            Some(stmt.clone())
        } else {
            None
        }
    }

    /// Remove a statement from cache
    pub fn remove(&self, statement_id: u32) {
        let mut statements = self.statements.write().unwrap();
        if let Some(stmt) = statements.remove(&statement_id) {
            self.query_to_id.write().unwrap().remove(&stmt.query);
        }
    }

    /// Clear expired statements
    pub fn clear_expired(&self) {
        let now = Instant::now();
        let mut statements = self.statements.write().unwrap();
        let mut query_map = self.query_to_id.write().unwrap();

        statements.retain(|_, stmt| {
            let keep = now.duration_since(stmt.last_accessed) < self.config.ttl;
            if !keep {
                query_map.remove(&stmt.query);
            }
            keep
        });
    }

    /// Evict least recently used statement if cache is full
    fn maybe_evict(&self) {
        let statements = self.statements.read().unwrap();
        if statements.len() >= self.config.max_size && self.config.enable_lru {
            drop(statements); // Release read lock

            // Find LRU statement
            let statements = self.statements.read().unwrap();
            if let Some((&lru_id, _)) = statements.iter().min_by_key(|(_, stmt)| stmt.last_accessed)
            {
                drop(statements); // Release read lock
                self.remove(lru_id);
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let statements = self.statements.read().unwrap();
        CacheStats {
            size: statements.len(),
            max_size: self.config.max_size,
            total_accesses: statements.values().map(|s| s.access_count).sum(),
        }
    }

    /// Clear all cached statements
    pub fn clear(&self) {
        self.statements.write().unwrap().clear();
        self.query_to_id.write().unwrap().clear();
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub total_accesses: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = PreparedStatementCache::new(PreparedStatementCacheConfig::default());
        let stats = cache.stats();
        assert_eq!(stats.size, 0);
    }

    #[test]
    fn test_get_or_prepare() {
        let cache = PreparedStatementCache::new(PreparedStatementCacheConfig::default());

        let id1 = cache.get_or_prepare("SELECT * FROM users", 0, 3);
        let id2 = cache.get_or_prepare("SELECT * FROM users", 0, 3);

        // Same query should return same ID
        assert_eq!(id1, id2);

        let stats = cache.stats();
        assert_eq!(stats.size, 1);
        assert_eq!(stats.total_accesses, 2);
    }

    #[test]
    fn test_cache_eviction() {
        let config = PreparedStatementCacheConfig {
            max_size: 2,
            ttl: Duration::from_secs(3600),
            enable_lru: true,
        };
        let cache = PreparedStatementCache::new(config);

        cache.get_or_prepare("SELECT 1", 0, 1);
        cache.get_or_prepare("SELECT 2", 0, 1);
        cache.get_or_prepare("SELECT 3", 0, 1);

        let stats = cache.stats();
        assert_eq!(stats.size, 2); // Should evict oldest
    }

    #[test]
    fn test_statement_removal() {
        let cache = PreparedStatementCache::new(PreparedStatementCacheConfig::default());

        let id = cache.get_or_prepare("SELECT * FROM users", 0, 3);
        assert!(cache.get(id).is_some());

        cache.remove(id);
        assert!(cache.get(id).is_none());
    }
}
