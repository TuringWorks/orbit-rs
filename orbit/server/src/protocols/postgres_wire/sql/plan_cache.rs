//! Query Plan Cache
//!
//! This module provides caching for query execution plans to avoid repeated parsing
//! and optimization. The cache supports:
//!
//! - Normalized query keys (same as query result cache)
//! - Plan invalidation on schema changes
//! - Statistics-based plan invalidation
//! - Memory-bounded LRU eviction

use crate::protocols::postgres_wire::sql::ast::Statement;
use crate::protocols::postgres_wire::sql::query_cache::QueryKey;
use crate::protocols::postgres_wire::sql::statistics::TableStatistics;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for plan cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCacheConfig {
    /// Enable plan caching
    pub enabled: bool,
    /// Maximum number of cached plans
    pub max_entries: usize,
    /// Plan TTL in seconds
    pub plan_ttl_seconds: u64,
    /// Invalidate plan if statistics change significantly
    pub invalidate_on_stats_change: bool,
    /// Statistics change threshold for invalidation (ratio)
    pub stats_change_threshold: f64,
}

impl Default for PlanCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 500,
            plan_ttl_seconds: 300, // 5 minutes
            invalidate_on_stats_change: true,
            stats_change_threshold: 0.2, // 20% change triggers invalidation
        }
    }
}

/// Query plan type
#[derive(Debug, Clone)]
pub enum QueryPlan {
    /// Sequential table scan
    SeqScan {
        table: String,
        filter: Option<String>,
        estimated_rows: usize,
    },
    /// Index scan
    IndexScan {
        table: String,
        index: String,
        filter: Option<String>,
        estimated_rows: usize,
    },
    /// Nested loop join
    NestedLoop {
        left: Box<QueryPlan>,
        right: Box<QueryPlan>,
        join_condition: String,
        estimated_rows: usize,
    },
    /// Hash join
    HashJoin {
        build: Box<QueryPlan>,
        probe: Box<QueryPlan>,
        join_condition: String,
        estimated_rows: usize,
    },
    /// Merge join
    MergeJoin {
        left: Box<QueryPlan>,
        right: Box<QueryPlan>,
        join_condition: String,
        estimated_rows: usize,
    },
    /// Sort operation
    Sort {
        input: Box<QueryPlan>,
        sort_keys: Vec<(String, bool)>, // (column, ascending)
        estimated_rows: usize,
    },
    /// Aggregation
    Aggregate {
        input: Box<QueryPlan>,
        group_by: Vec<String>,
        aggregates: Vec<String>,
        estimated_rows: usize,
    },
    /// Filter operation
    Filter {
        input: Box<QueryPlan>,
        condition: String,
        estimated_rows: usize,
    },
    /// Projection
    Project {
        input: Box<QueryPlan>,
        columns: Vec<String>,
        estimated_rows: usize,
    },
    /// Limit
    Limit {
        input: Box<QueryPlan>,
        count: usize,
        offset: usize,
    },
    /// Union
    Union {
        inputs: Vec<QueryPlan>,
        all: bool,
        estimated_rows: usize,
    },
    /// Vectorized execution hint
    Vectorized {
        input: Box<QueryPlan>,
        use_simd: bool,
        use_gpu: bool,
    },
}

impl QueryPlan {
    /// Get estimated rows for this plan node
    pub fn estimated_rows(&self) -> usize {
        match self {
            QueryPlan::SeqScan { estimated_rows, .. } => *estimated_rows,
            QueryPlan::IndexScan { estimated_rows, .. } => *estimated_rows,
            QueryPlan::NestedLoop { estimated_rows, .. } => *estimated_rows,
            QueryPlan::HashJoin { estimated_rows, .. } => *estimated_rows,
            QueryPlan::MergeJoin { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Sort { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Aggregate { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Filter { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Project { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Limit { count, input, .. } => (*count).min(input.estimated_rows()),
            QueryPlan::Union { estimated_rows, .. } => *estimated_rows,
            QueryPlan::Vectorized { input, .. } => input.estimated_rows(),
        }
    }

    /// Get estimated cost (simplified model)
    pub fn estimated_cost(&self) -> f64 {
        match self {
            QueryPlan::SeqScan { estimated_rows, .. } => *estimated_rows as f64,
            QueryPlan::IndexScan { estimated_rows, .. } => (*estimated_rows as f64) * 0.25, // Index is cheaper
            QueryPlan::NestedLoop { left, right, .. } => {
                left.estimated_cost() + (left.estimated_rows() as f64 * right.estimated_cost())
            }
            QueryPlan::HashJoin { build, probe, .. } => {
                build.estimated_cost() + probe.estimated_cost() + build.estimated_rows() as f64
            }
            QueryPlan::MergeJoin { left, right, .. } => {
                left.estimated_cost() + right.estimated_cost()
            }
            QueryPlan::Sort { input, .. } => {
                let rows = input.estimated_rows() as f64;
                input.estimated_cost() + rows * rows.log2()
            }
            QueryPlan::Aggregate { input, .. } => input.estimated_cost() + input.estimated_rows() as f64,
            QueryPlan::Filter { input, .. } => input.estimated_cost(),
            QueryPlan::Project { input, .. } => input.estimated_cost(),
            QueryPlan::Limit { input, .. } => input.estimated_cost(),
            QueryPlan::Union { inputs, .. } => inputs.iter().map(|p| p.estimated_cost()).sum(),
            QueryPlan::Vectorized { input, use_simd, use_gpu } => {
                let base_cost = input.estimated_cost();
                if *use_gpu {
                    base_cost * 0.1 // GPU is 10x faster for large data
                } else if *use_simd {
                    base_cost * 0.25 // SIMD is 4x faster
                } else {
                    base_cost
                }
            }
        }
    }

    /// Format plan as text (for EXPLAIN)
    pub fn format(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        match self {
            QueryPlan::SeqScan { table, filter, estimated_rows } => {
                let filter_str = filter.as_ref().map(|f| format!(" [filter: {}]", f)).unwrap_or_default();
                format!("{}SeqScan on {}{} (rows={})", prefix, table, filter_str, estimated_rows)
            }
            QueryPlan::IndexScan { table, index, filter, estimated_rows } => {
                let filter_str = filter.as_ref().map(|f| format!(" [filter: {}]", f)).unwrap_or_default();
                format!("{}IndexScan using {} on {}{} (rows={})", prefix, index, table, filter_str, estimated_rows)
            }
            QueryPlan::NestedLoop { left, right, join_condition, estimated_rows } => {
                format!("{}NestedLoop [{}] (rows={})\n{}\n{}",
                    prefix, join_condition, estimated_rows,
                    left.format(indent + 2),
                    right.format(indent + 2))
            }
            QueryPlan::HashJoin { build, probe, join_condition, estimated_rows } => {
                format!("{}HashJoin [{}] (rows={})\n{}\n{}",
                    prefix, join_condition, estimated_rows,
                    build.format(indent + 2),
                    probe.format(indent + 2))
            }
            QueryPlan::MergeJoin { left, right, join_condition, estimated_rows } => {
                format!("{}MergeJoin [{}] (rows={})\n{}\n{}",
                    prefix, join_condition, estimated_rows,
                    left.format(indent + 2),
                    right.format(indent + 2))
            }
            QueryPlan::Sort { input, sort_keys, estimated_rows } => {
                let keys_str = sort_keys.iter()
                    .map(|(k, asc)| format!("{} {}", k, if *asc { "ASC" } else { "DESC" }))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}Sort [{}] (rows={})\n{}",
                    prefix, keys_str, estimated_rows,
                    input.format(indent + 2))
            }
            QueryPlan::Aggregate { input, group_by, aggregates, estimated_rows } => {
                let group_str = if group_by.is_empty() { String::new() } else { format!(" GROUP BY {}", group_by.join(", ")) };
                format!("{}Aggregate [{}]{} (rows={})\n{}",
                    prefix, aggregates.join(", "), group_str, estimated_rows,
                    input.format(indent + 2))
            }
            QueryPlan::Filter { input, condition, estimated_rows } => {
                format!("{}Filter [{}] (rows={})\n{}",
                    prefix, condition, estimated_rows,
                    input.format(indent + 2))
            }
            QueryPlan::Project { input, columns, estimated_rows } => {
                format!("{}Project [{}] (rows={})\n{}",
                    prefix, columns.join(", "), estimated_rows,
                    input.format(indent + 2))
            }
            QueryPlan::Limit { input, count, offset } => {
                format!("{}Limit {} (offset {})\n{}",
                    prefix, count, offset,
                    input.format(indent + 2))
            }
            QueryPlan::Union { inputs, all, estimated_rows } => {
                let union_type = if *all { "Union All" } else { "Union" };
                let inputs_str = inputs.iter()
                    .map(|p| p.format(indent + 2))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}{} (rows={})\n{}", prefix, union_type, estimated_rows, inputs_str)
            }
            QueryPlan::Vectorized { input, use_simd, use_gpu } => {
                let accel = match (*use_gpu, *use_simd) {
                    (true, _) => "GPU",
                    (_, true) => "SIMD",
                    _ => "None",
                };
                format!("{}Vectorized [accel={}]\n{}",
                    prefix, accel, input.format(indent + 2))
            }
        }
    }
}

/// Cached query plan entry
#[derive(Debug, Clone)]
pub struct CachedPlan {
    /// The query plan
    pub plan: QueryPlan,
    /// Original parsed statement
    pub statement: Statement,
    /// When the plan was created
    pub created_at: Instant,
    /// TTL for this plan
    pub ttl: Duration,
    /// Statistics snapshot used for planning
    pub stats_snapshot: HashMap<String, TableStatsSnapshot>,
    /// Number of times this plan was used
    pub use_count: u64,
    /// Average execution time when using this plan
    pub avg_execution_time_ms: f64,
}

/// Snapshot of table statistics for invalidation checking
#[derive(Debug, Clone)]
pub struct TableStatsSnapshot {
    pub row_count: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl CachedPlan {
    /// Check if the plan has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Check if statistics have changed significantly
    pub fn stats_changed(&self, current_stats: &HashMap<String, &TableStatistics>, threshold: f64) -> bool {
        for (table, snapshot) in &self.stats_snapshot {
            if let Some(current) = current_stats.get(table) {
                let old_count = snapshot.row_count as f64;
                let new_count = current.row_count as f64;

                if old_count > 0.0 {
                    let change_ratio = (new_count - old_count).abs() / old_count;
                    if change_ratio > threshold {
                        return true;
                    }
                } else if new_count > 0.0 {
                    return true;
                }
            }
        }
        false
    }

    /// Record plan execution
    pub fn record_execution(&mut self, execution_time_ms: u64) {
        let count = self.use_count as f64;
        self.avg_execution_time_ms = (self.avg_execution_time_ms * count + execution_time_ms as f64) / (count + 1.0);
        self.use_count += 1;
    }
}

/// Plan cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanCacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of plans created
    pub plans_created: u64,
    /// Number of invalidations
    pub invalidations: u64,
    /// Current number of cached plans
    pub current_entries: usize,
}

impl PlanCacheStats {
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

/// Query plan cache
pub struct PlanCache {
    /// Cached plans
    cache: Arc<RwLock<HashMap<QueryKey, CachedPlan>>>,
    /// LRU order
    lru_order: Arc<RwLock<VecDeque<QueryKey>>>,
    /// Cache statistics
    stats: Arc<RwLock<PlanCacheStats>>,
    /// Configuration
    config: PlanCacheConfig,
}

impl PlanCache {
    /// Create a new plan cache
    pub fn new(config: PlanCacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(PlanCacheStats::default())),
            config,
        }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(PlanCacheConfig::default())
    }

    /// Get a cached plan
    pub async fn get(
        &self,
        key: &QueryKey,
        current_stats: Option<&HashMap<String, &TableStatistics>>,
    ) -> Option<CachedPlan> {
        if !self.config.enabled {
            return None;
        }

        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(plan) = cache.get_mut(key) {
            // Check expiration
            if plan.is_expired() {
                cache.remove(key);
                stats.current_entries = cache.len();
                stats.misses += 1;
                return None;
            }

            // Check statistics change
            if self.config.invalidate_on_stats_change {
                if let Some(cs) = current_stats {
                    if plan.stats_changed(cs, self.config.stats_change_threshold) {
                        cache.remove(key);
                        stats.current_entries = cache.len();
                        stats.invalidations += 1;
                        stats.misses += 1;
                        return None;
                    }
                }
            }

            // Update LRU
            let mut lru = self.lru_order.write().await;
            lru.retain(|k| k != key);
            lru.push_back(key.clone());

            stats.hits += 1;
            return Some(plan.clone());
        }

        stats.misses += 1;
        None
    }

    /// Put a plan in the cache
    pub async fn put(
        &self,
        key: QueryKey,
        plan: QueryPlan,
        statement: Statement,
        table_stats: HashMap<String, TableStatsSnapshot>,
    ) {
        if !self.config.enabled {
            return;
        }

        let cached = CachedPlan {
            plan,
            statement,
            created_at: Instant::now(),
            ttl: Duration::from_secs(self.config.plan_ttl_seconds),
            stats_snapshot: table_stats,
            use_count: 0,
            avg_execution_time_ms: 0.0,
        };

        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;

        // Evict if at capacity
        while cache.len() >= self.config.max_entries {
            if let Some(oldest_key) = lru.pop_front() {
                cache.remove(&oldest_key);
            } else {
                break;
            }
        }

        cache.insert(key.clone(), cached);
        lru.push_back(key);

        stats.plans_created += 1;
        stats.current_entries = cache.len();
    }

    /// Invalidate all plans for a table
    pub async fn invalidate_table(&self, table_name: &str) {
        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;

        // Find plans that reference this table
        let keys_to_remove: Vec<QueryKey> = cache.iter()
            .filter(|(_, plan)| plan.stats_snapshot.contains_key(table_name))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            cache.remove(&key);
            lru.retain(|k| k != &key);
            stats.invalidations += 1;
        }

        stats.current_entries = cache.len();
    }

    /// Clear all cached plans
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut lru = self.lru_order.write().await;
        let mut stats = self.stats.write().await;

        let count = cache.len();
        cache.clear();
        lru.clear();

        stats.invalidations += count as u64;
        stats.current_entries = 0;
    }

    /// Record plan execution
    pub async fn record_execution(&self, key: &QueryKey, execution_time_ms: u64) {
        let mut cache = self.cache.write().await;
        if let Some(plan) = cache.get_mut(key) {
            plan.record_execution(execution_time_ms);
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> PlanCacheStats {
        self.stats.read().await.clone()
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Default for PlanCache {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::ast::SelectStatement;

    fn create_test_statement() -> Statement {
        Statement::Select(Box::new(SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![],
            from_clause: None,
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
            for_clause: None,
            traverse: None,
            set_operation: None,
        }))
    }

    #[test]
    fn test_plan_cost_estimation() {
        let seq_scan = QueryPlan::SeqScan {
            table: "users".to_string(),
            filter: None,
            estimated_rows: 1000,
        };

        let index_scan = QueryPlan::IndexScan {
            table: "users".to_string(),
            index: "users_pkey".to_string(),
            filter: None,
            estimated_rows: 1000,
        };

        // Index scan should be cheaper
        assert!(index_scan.estimated_cost() < seq_scan.estimated_cost());
    }

    #[test]
    fn test_plan_formatting() {
        let plan = QueryPlan::HashJoin {
            build: Box::new(QueryPlan::SeqScan {
                table: "users".to_string(),
                filter: None,
                estimated_rows: 100,
            }),
            probe: Box::new(QueryPlan::SeqScan {
                table: "orders".to_string(),
                filter: Some("status = 'active'".to_string()),
                estimated_rows: 500,
            }),
            join_condition: "users.id = orders.user_id".to_string(),
            estimated_rows: 500,
        };

        let formatted = plan.format(0);
        assert!(formatted.contains("HashJoin"));
        assert!(formatted.contains("users"));
        assert!(formatted.contains("orders"));
    }

    #[tokio::test]
    async fn test_plan_cache() {
        let cache = PlanCache::new_default();
        let key = QueryKey::new("SELECT * FROM users", "test", "public");

        let plan = QueryPlan::SeqScan {
            table: "users".to_string(),
            filter: None,
            estimated_rows: 100,
        };

        let statement = create_test_statement();

        cache.put(
            key.clone(),
            plan,
            statement,
            HashMap::new(),
        ).await;

        let cached = cache.get(&key, None).await;
        assert!(cached.is_some());

        let stats = cache.stats().await;
        assert_eq!(stats.plans_created, 1);
        assert_eq!(stats.hits, 1);
    }

    #[tokio::test]
    async fn test_plan_invalidation() {
        let cache = PlanCache::new_default();
        let key = QueryKey::new("SELECT * FROM users", "test", "public");

        let plan = QueryPlan::SeqScan {
            table: "users".to_string(),
            filter: None,
            estimated_rows: 100,
        };

        let mut table_stats = HashMap::new();
        table_stats.insert("users".to_string(), TableStatsSnapshot {
            row_count: 100,
            timestamp: chrono::Utc::now(),
        });

        cache.put(
            key.clone(),
            plan,
            create_test_statement(),
            table_stats,
        ).await;

        assert!(cache.get(&key, None).await.is_some());

        cache.invalidate_table("users").await;

        assert!(cache.get(&key, None).await.is_none());
    }

    #[test]
    fn test_vectorized_plan_cost() {
        let base_plan = QueryPlan::SeqScan {
            table: "large_table".to_string(),
            filter: Some("value > 100".to_string()),
            estimated_rows: 100000,
        };

        let simd_plan = QueryPlan::Vectorized {
            input: Box::new(base_plan.clone()),
            use_simd: true,
            use_gpu: false,
        };

        let gpu_plan = QueryPlan::Vectorized {
            input: Box::new(base_plan.clone()),
            use_simd: false,
            use_gpu: true,
        };

        assert!(simd_plan.estimated_cost() < base_plan.estimated_cost());
        assert!(gpu_plan.estimated_cost() < simd_plan.estimated_cost());
    }
}
