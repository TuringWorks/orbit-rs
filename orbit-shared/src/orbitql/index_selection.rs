//! Index selection engine for query optimization
//!
//! This module provides intelligent index selection and usage tracking
//! to automatically choose the most efficient indexes for query execution.
//! Implements the framework defined in Phase 9.3.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::ast::*;
use super::cost_model::{CostModel, QueryCost};
use super::statistics::{IndexStatistics, StatisticsManager};

/// Index selection engine for optimal index usage
pub struct IndexSelector {
    /// Available indexes metadata
    available_indexes: Arc<RwLock<HashMap<String, Vec<IndexMetadata>>>>,
    /// Index usage tracking
    usage_tracker: Arc<RwLock<IndexUsageTracker>>,
    /// Cost estimator for index operations
    cost_estimator: IndexCostEstimator,
    /// Statistics manager
    statistics_manager: Arc<RwLock<StatisticsManager>>,
}

/// Metadata for an available index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Unique identifier
    pub index_id: String,
    /// Table this index belongs to
    pub table_name: String,
    /// Indexed columns
    pub columns: Vec<String>,
    /// Index type
    pub index_type: IndexType,
    /// Whether index is unique
    pub is_unique: bool,
    /// Partial index condition (if any)
    pub filter_condition: Option<Expression>,
    /// Index size in bytes
    pub size_bytes: u64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last maintenance timestamp
    pub last_maintained: Option<chrono::DateTime<chrono::Utc>>,
}

/// Types of indexes supported
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndexType {
    /// B-Tree index for range queries
    BTree,
    /// Hash index for equality lookups
    Hash,
    /// Bitmap index for low-cardinality data
    Bitmap,
    /// Partial index with filter condition
    Partial,
    /// Expression index on computed values
    Expression,
    /// Covering index that includes non-key columns
    Covering { included_columns: Vec<String> },
    /// Vector index for similarity search
    Vector {
        dimensions: u32,
        algorithm: VectorIndexAlgorithm,
    },
}

/// Vector index algorithms
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VectorIndexAlgorithm {
    IvfFlat { lists: u32 },
    Hnsw { m: u32, ef_construction: u32 },
}

/// Index selection recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexChoice {
    /// Recommended index
    pub index: IndexMetadata,
    /// Estimated cost of using this index
    pub estimated_cost: QueryCost,
    /// Confidence in this recommendation (0.0 to 1.0)
    pub confidence: f64,
    /// Reason for selecting this index
    pub reason: String,
}

/// Index usage tracking
#[derive(Debug, Clone, Default)]
pub struct IndexUsageTracker {
    /// Usage statistics per index
    usage_stats: HashMap<String, IndexUsageStats>,
    /// Query patterns that used specific indexes
    query_patterns: HashMap<String, Vec<String>>, // pattern_hash -> index_ids
}

/// Usage statistics for an index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUsageStats {
    /// Number of times index was used
    pub usage_count: u64,
    /// Number of times index was considered but not used
    pub consideration_count: u64,
    /// Average selectivity achieved
    pub avg_selectivity: f64,
    /// Total rows scanned using this index
    pub total_rows_scanned: u64,
    /// Total execution time saved (estimated)
    pub time_saved_ms: u64,
    /// Last used timestamp
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    /// Performance metrics
    pub performance_metrics: IndexPerformanceMetrics,
}

/// Performance metrics for index usage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexPerformanceMetrics {
    /// Average lookup time in microseconds
    pub avg_lookup_time_us: f64,
    /// Cache hit rate for index pages
    pub cache_hit_rate: f64,
    /// Average I/O operations per lookup
    pub avg_io_operations: f64,
    /// Index maintenance overhead
    pub maintenance_overhead_ms: u64,
}

/// Cost estimator specifically for index operations
pub struct IndexCostEstimator {
    cost_model: CostModel,
    /// Base cost factors for different index types
    index_type_factors: HashMap<IndexType, f64>,
}

impl IndexSelector {
    /// Create a new index selector
    pub fn new(statistics_manager: Arc<RwLock<StatisticsManager>>, cost_model: CostModel) -> Self {
        Self {
            available_indexes: Arc::new(RwLock::new(HashMap::new())),
            usage_tracker: Arc::new(RwLock::new(IndexUsageTracker::default())),
            cost_estimator: IndexCostEstimator::new(cost_model),
            statistics_manager,
        }
    }

    /// Select the best indexes for a query
    pub async fn select_best_indexes(&self, query: &Statement) -> Vec<IndexChoice> {
        match query {
            Statement::Select(select) => {
                let mut recommendations = Vec::new();

                // Analyze each table in the query
                for from_clause in &select.from {
                    if let FromClause::Table { name, .. } = from_clause {
                        let table_recommendations =
                            self.select_indexes_for_table(name, select).await;
                        recommendations.extend(table_recommendations);
                    }
                }

                // Sort by cost and select the best combination
                self.optimize_index_combination(recommendations).await
            }
            _ => Vec::new(), // Non-SELECT queries don't benefit from read indexes
        }
    }

    /// Select indexes for a specific table
    async fn select_indexes_for_table(
        &self,
        table_name: &str,
        select: &SelectStatement,
    ) -> Vec<IndexChoice> {
        let mut recommendations = Vec::new();

        let available_indexes = self.available_indexes.read().await;
        if let Some(table_indexes) = available_indexes.get(table_name) {
            for index in table_indexes {
                if let Some(choice) = self.evaluate_index_for_query(index, select).await {
                    recommendations.push(choice);
                }
            }
        }

        recommendations
    }

    /// Evaluate if an index is applicable for a query
    async fn evaluate_index_for_query(
        &self,
        index: &IndexMetadata,
        select: &SelectStatement,
    ) -> Option<IndexChoice> {
        // Check WHERE clause applicability
        let where_applicability = if let Some(where_clause) = &select.where_clause {
            self.check_where_clause_applicability(index, where_clause)
        } else {
            0.0 // No WHERE clause means index won't help much
        };

        // Check ORDER BY applicability
        let order_applicability = if !select.order_by.is_empty() {
            self.check_order_by_applicability(index, &select.order_by)
        } else {
            0.0
        };

        // Check JOIN condition applicability
        let join_applicability = if !select.join_clauses.is_empty() {
            self.check_join_applicability(index, &select.join_clauses)
        } else {
            0.0
        };

        // Use statistics manager to get table statistics for better cost estimation
        let table_stats = self.get_table_statistics(&index.table_name).await;
        let selectivity_factor = if let Some(stats) = table_stats {
            // Adjust applicability based on table size and cardinality
            if stats.row_count > 10000 {
                1.2 // Higher benefit for large tables
            } else if stats.row_count < 100 {
                0.5 // Lower benefit for small tables
            } else {
                1.0
            }
        } else {
            1.0 // Default if no statistics available
        };

        // Calculate overall applicability score
        let total_applicability =
            (where_applicability + order_applicability + join_applicability) * selectivity_factor;

        if total_applicability > 0.1 {
            // Threshold for considering an index
            let estimated_cost = self.cost_estimator.estimate_index_cost(index, select).await;

            Some(IndexChoice {
                index: index.clone(),
                estimated_cost,
                confidence: total_applicability,
                reason: self.generate_selection_reason(
                    where_applicability,
                    order_applicability,
                    join_applicability,
                ),
            })
        } else {
            None
        }
    }

    /// Check how well an index applies to WHERE clause conditions
    fn check_where_clause_applicability(
        &self,
        index: &IndexMetadata,
        where_clause: &Expression,
    ) -> f64 {
        match where_clause {
            Expression::Binary {
                operator: BinaryOperator::And,
                left,
                right,
            } => {
                // For AND conditions, take the maximum applicability
                let left_score = self.check_where_clause_applicability(index, left);
                let right_score = self.check_where_clause_applicability(index, right);
                left_score.max(right_score)
            }
            Expression::Binary {
                operator: BinaryOperator::Or,
                left,
                right,
            } => {
                // For OR conditions, average the applicabilities
                let left_score = self.check_where_clause_applicability(index, left);
                let right_score = self.check_where_clause_applicability(index, right);
                (left_score + right_score) / 2.0
            }
            Expression::Binary {
                left,
                operator,
                right: _,
            } => {
                if let Expression::Identifier(column) = left.as_ref() {
                    if index.columns.contains(column) {
                        // Index column is used in WHERE clause
                        match operator {
                            BinaryOperator::Equal => {
                                if matches!(index.index_type, IndexType::Hash | IndexType::BTree) {
                                    0.9 // High applicability for equality
                                } else {
                                    0.3
                                }
                            }
                            BinaryOperator::GreaterThan
                            | BinaryOperator::LessThan
                            | BinaryOperator::GreaterThanOrEqual
                            | BinaryOperator::LessThanOrEqual => {
                                if matches!(index.index_type, IndexType::BTree) {
                                    0.8 // High applicability for range queries with B-Tree
                                } else {
                                    0.1
                                }
                            }
                            BinaryOperator::Like => {
                                if matches!(index.index_type, IndexType::BTree) {
                                    0.6 // Moderate applicability for LIKE with B-Tree
                                } else {
                                    0.1
                                }
                            }
                            _ => 0.2, // Other operators have lower applicability
                        }
                    } else {
                        0.0 // Column not covered by index
                    }
                } else {
                    0.0 // Not a simple column reference
                }
            }
            _ => 0.0, // Other expression types
        }
    }

    /// Get table statistics from the statistics manager
    async fn get_table_statistics(
        &self,
        table_name: &str,
    ) -> Option<super::statistics::TableStatistics> {
        let stats_manager = self.statistics_manager.read().await;
        stats_manager.get_table_statistics(table_name).await
    }

    /// Record index usage statistics
    pub async fn record_index_usage(
        &self,
        index_id: &str,
        used: bool,
        selectivity: f64,
        lookup_time_us: f64,
    ) {
        let mut tracker = self.usage_tracker.write().await;
        let stats = tracker
            .usage_stats
            .entry(index_id.to_string())
            .or_insert_with(|| IndexUsageStats {
                usage_count: 0,
                consideration_count: 0,
                avg_selectivity: 0.0,
                total_rows_scanned: 0,
                time_saved_ms: 0,
                last_used: None,
                performance_metrics: IndexPerformanceMetrics::default(),
            });

        stats.consideration_count += 1;
        if used {
            stats.usage_count += 1;
            stats.last_used = Some(chrono::Utc::now());
            stats.avg_selectivity = (stats.avg_selectivity * (stats.usage_count - 1) as f64
                + selectivity)
                / stats.usage_count as f64;

            // Update performance metrics
            let metrics = &mut stats.performance_metrics;
            metrics.avg_lookup_time_us =
                (metrics.avg_lookup_time_us * (stats.usage_count - 1) as f64 + lookup_time_us)
                    / stats.usage_count as f64;
        }
    }

    /// Get index usage statistics
    pub async fn get_index_usage_stats(&self, index_id: &str) -> Option<IndexUsageStats> {
        let tracker = self.usage_tracker.read().await;
        tracker.usage_stats.get(index_id).cloned()
    }

    /// Get table indexes for analysis
    pub async fn get_table_indexes(&self, table_name: &str) -> Vec<IndexMetadata> {
        let indexes = self.available_indexes.read().await;
        indexes.get(table_name).cloned().unwrap_or_default()
    }

    /// Check if statistics manager is available
    pub async fn has_statistics(&self, table_name: &str) -> bool {
        self.get_table_statistics(table_name).await.is_some()
    }

    /// Check how well an index applies to ORDER BY clauses
    fn check_order_by_applicability(
        &self,
        index: &IndexMetadata,
        order_by: &[OrderByClause],
    ) -> f64 {
        if order_by.is_empty() {
            return 0.0;
        }

        let mut applicable_columns = 0;
        let mut total_columns = 0;

        for order_clause in order_by {
            total_columns += 1;
            if let Expression::Identifier(column) = &order_clause.expression {
                if index.columns.contains(column) {
                    applicable_columns += 1;
                }
            }
        }

        if applicable_columns > 0 && matches!(index.index_type, IndexType::BTree) {
            (applicable_columns as f64 / total_columns as f64) * 0.7 // B-Tree indexes help with sorting
        } else {
            0.0
        }
    }

    /// Check how well an index applies to JOIN conditions
    fn check_join_applicability(&self, index: &IndexMetadata, joins: &[JoinClause]) -> f64 {
        let mut max_applicability: f64 = 0.0;

        for join in joins {
            let join_applicability = match &join.condition {
                Expression::Binary {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                } => {
                    // Check if either side of the join condition uses this index
                    let left_matches = if let Expression::Identifier(col) = left.as_ref() {
                        index.columns.contains(col)
                    } else {
                        false
                    };

                    let right_matches = if let Expression::Identifier(col) = right.as_ref() {
                        index.columns.contains(col)
                    } else {
                        false
                    };

                    if left_matches || right_matches {
                        0.8 // High applicability for join conditions
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };

            max_applicability = max_applicability.max(join_applicability);
        }

        max_applicability
    }

    /// Generate a human-readable reason for index selection
    fn generate_selection_reason(
        &self,
        where_score: f64,
        order_score: f64,
        join_score: f64,
    ) -> String {
        let mut reasons = Vec::new();

        if where_score > 0.5 {
            reasons.push("efficient WHERE clause filtering");
        }
        if order_score > 0.3 {
            reasons.push("ORDER BY optimization");
        }
        if join_score > 0.3 {
            reasons.push("JOIN condition optimization");
        }

        if reasons.is_empty() {
            "general query acceleration".to_string()
        } else {
            format!("Recommended for: {}", reasons.join(", "))
        }
    }

    /// Optimize the combination of selected indexes
    async fn optimize_index_combination(
        &self,
        mut recommendations: Vec<IndexChoice>,
    ) -> Vec<IndexChoice> {
        // Sort by cost (lower is better) and confidence (higher is better)
        recommendations.sort_by(|a, b| {
            let cost_cmp = a
                .estimated_cost
                .total_cost()
                .partial_cmp(&b.estimated_cost.total_cost())
                .unwrap_or(std::cmp::Ordering::Equal);

            if cost_cmp == std::cmp::Ordering::Equal {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                cost_cmp
            }
        });

        // Remove redundant indexes (simplified logic)
        let mut optimized = Vec::new();
        let mut covered_columns = std::collections::HashSet::new();

        for recommendation in recommendations {
            let mut is_redundant = true;
            for column in &recommendation.index.columns {
                if !covered_columns.contains(column) {
                    is_redundant = false;
                    covered_columns.insert(column.clone());
                }
            }

            if !is_redundant || optimized.len() < 3 {
                // Keep at least some recommendations
                optimized.push(recommendation);
            }
        }

        optimized
    }

    /// Add an index to the available indexes
    pub async fn add_index(&self, table_name: String, index: IndexMetadata) {
        let mut indexes = self.available_indexes.write().await;
        indexes
            .entry(table_name)
            .or_insert_with(Vec::new)
            .push(index);
    }

    /// Remove an index from available indexes
    pub async fn remove_index(&self, table_name: &str, index_id: &str) {
        let mut indexes = self.available_indexes.write().await;
        if let Some(table_indexes) = indexes.get_mut(table_name) {
            table_indexes.retain(|idx| idx.index_id != index_id);
        }
    }
}

impl IndexCostEstimator {
    /// Create a new index cost estimator
    pub fn new(cost_model: CostModel) -> Self {
        let mut index_type_factors = HashMap::new();
        index_type_factors.insert(IndexType::BTree, 1.0); // Base factor
        index_type_factors.insert(IndexType::Hash, 0.5); // Hash indexes are faster for equality
        index_type_factors.insert(IndexType::Bitmap, 0.3); // Bitmap indexes are very efficient for low cardinality
        index_type_factors.insert(IndexType::Partial, 0.8); // Partial indexes are selective

        Self {
            cost_model,
            index_type_factors,
        }
    }

    /// Estimate the cost of using an index for a query
    pub async fn estimate_index_cost(
        &self,
        index: &IndexMetadata,
        _select: &SelectStatement,
    ) -> QueryCost {
        // Create mock statistics for cost estimation
        let index_stats = IndexStatistics {
            index_id: index.index_id.clone(),
            selectivity: 0.01, // Assume 1% selectivity
            clustering_factor: 0.9,
            tree_height: 3,
            leaf_pages: (index.size_bytes / 8192).max(1), // Assume 8KB pages
            distinct_keys: 10000,                         // Mock value
            index_size: index.size_bytes,
            last_updated: chrono::Utc::now(),
        };

        let table_stats = super::statistics::TableStatistics {
            row_count: 100000, // Mock value
            page_count: 1000,
            avg_row_size: 100,
            null_fraction: 0.1,
            distinct_values: 80000,
            most_common_values: vec![],
            histogram: vec![],
            last_analyzed: chrono::Utc::now(),
            column_statistics: HashMap::new(),
        };

        let (mut cost, _) =
            self.cost_model
                .calculate_index_scan_cost(&table_stats, &index_stats, 0.01);

        // Apply index type factor
        if let Some(&factor) = self.index_type_factors.get(&index.index_type) {
            cost = cost.scale(factor);
        }

        cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbitql::QueryValue;

    #[tokio::test]
    async fn test_index_selector_creation() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let selector = IndexSelector::new(stats_manager, cost_model);

        // Test that selector was created successfully
        let table_indexes = selector.get_table_indexes("test_table").await;
        assert!(table_indexes.is_empty());
    }

    #[tokio::test]
    async fn test_add_and_remove_index() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let selector = IndexSelector::new(stats_manager, cost_model);

        let index = IndexMetadata {
            index_id: "idx_test".to_string(),
            table_name: "test_table".to_string(),
            columns: vec!["id".to_string()],
            index_type: IndexType::BTree,
            is_unique: true,
            filter_condition: None,
            size_bytes: 1024 * 1024,
            created_at: chrono::Utc::now(),
            last_maintained: None,
        };

        // Add index
        selector
            .add_index("test_table".to_string(), index.clone())
            .await;
        let indexes = selector.get_table_indexes("test_table").await;
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].index_id, "idx_test");

        // Remove index
        selector.remove_index("test_table", "idx_test").await;
        let indexes = selector.get_table_indexes("test_table").await;
        assert!(indexes.is_empty());
    }

    #[tokio::test]
    async fn test_where_clause_applicability() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let selector = IndexSelector::new(stats_manager, cost_model);

        let btree_index = IndexMetadata {
            index_id: "idx_btree".to_string(),
            table_name: "test_table".to_string(),
            columns: vec!["age".to_string()],
            index_type: IndexType::BTree,
            is_unique: false,
            filter_condition: None,
            size_bytes: 1024 * 1024,
            created_at: chrono::Utc::now(),
            last_maintained: None,
        };

        // Test equality condition
        let where_clause = Expression::Binary {
            left: Box::new(Expression::Identifier("age".to_string())),
            operator: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(QueryValue::Integer(25))),
        };

        let applicability = selector.check_where_clause_applicability(&btree_index, &where_clause);
        assert!(applicability > 0.8); // Should be highly applicable

        // Test range condition
        let where_clause = Expression::Binary {
            left: Box::new(Expression::Identifier("age".to_string())),
            operator: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(QueryValue::Integer(18))),
        };

        let applicability = selector.check_where_clause_applicability(&btree_index, &where_clause);
        assert!(applicability > 0.7); // Should be applicable for B-Tree
    }

    #[tokio::test]
    async fn test_index_usage_tracking() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let selector = IndexSelector::new(stats_manager, cost_model);

        // Record usage
        selector
            .record_index_usage(
                "idx_test",
                "SELECT * FROM users WHERE age > ?",
                0.1,
                1000,
                50,
            )
            .await;

        // Get usage stats
        let stats = selector.get_index_usage_stats("idx_test").await.unwrap();
        assert_eq!(stats.usage_count, 1);
        assert_eq!(stats.avg_selectivity, 0.1);
        assert_eq!(stats.total_rows_scanned, 1000);
    }
}
