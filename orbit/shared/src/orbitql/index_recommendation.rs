//! Index recommendation system for automated database optimization
//!
//! This module provides intelligent analysis of query patterns to recommend
//! optimal indexes, detect missing indexes, and identify redundant ones.
//! Implements the framework defined in Phase 9.3.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::ast::{BinaryOperator, Expression, FromClause, Statement};
#[cfg(test)]
use super::ast::{SelectField, SelectStatement};
use super::cost_model::{CostModel, QueryCost};
use super::index_selection::{IndexMetadata, IndexSelector, IndexType, IndexUsageStats};
use super::statistics::{StatisticsManager, TableStatistics};

/// Index recommendation engine that analyzes query patterns and suggests optimal indexes
pub struct IndexRecommendationEngine {
    /// Query pattern analyzer
    query_analyzer: QueryPatternAnalyzer,
    /// Cost model for impact estimation
    cost_model: CostModel,
    /// Statistics manager
    statistics_manager: Arc<RwLock<StatisticsManager>>,
    /// Index selector for current state analysis
    index_selector: Arc<IndexSelector>,
    /// Configuration
    config: RecommendationConfig,
}

/// Configuration for index recommendation system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationConfig {
    /// Minimum query frequency to consider for indexing
    pub min_query_frequency: u32,
    /// Minimum performance improvement threshold (as percentage)
    pub min_improvement_threshold: f64,
    /// Maximum number of recommendations to generate
    pub max_recommendations: usize,
    /// Analysis window in days
    pub analysis_window_days: u32,
    /// Minimum table size to consider for indexing (rows)
    pub min_table_size: u64,
    /// Maximum index overhead tolerance (as percentage of table size)
    pub max_index_overhead: f64,
}

impl Default for RecommendationConfig {
    fn default() -> Self {
        Self {
            min_query_frequency: 10,         // At least 10 occurrences
            min_improvement_threshold: 20.0, // 20% improvement
            max_recommendations: 20,         // Max 20 recommendations
            analysis_window_days: 7,         // Analyze last 7 days
            min_table_size: 1000,            // Tables with at least 1000 rows
            max_index_overhead: 25.0,        // Max 25% overhead
        }
    }
}

/// Query pattern analyzer that tracks query usage patterns
#[derive(Debug)]
pub struct QueryPatternAnalyzer {
    /// Tracked query patterns
    query_patterns: Arc<RwLock<HashMap<String, QueryPattern>>>,
    /// Column access patterns
    column_access_patterns: Arc<RwLock<HashMap<String, ColumnAccessPattern>>>,
    /// Join patterns
    join_patterns: Arc<RwLock<HashMap<String, JoinPattern>>>,
}

/// Represents a query pattern with usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPattern {
    /// Pattern hash for identification
    pub pattern_hash: String,
    /// Normalized query template
    pub query_template: String,
    /// Tables accessed
    pub tables: Vec<String>,
    /// Columns used in WHERE clauses
    pub where_columns: Vec<ColumnReference>,
    /// Columns used in ORDER BY
    pub order_columns: Vec<ColumnReference>,
    /// Columns used in GROUP BY
    pub group_columns: Vec<ColumnReference>,
    /// Join conditions
    pub joins: Vec<JoinCondition>,
    /// Usage frequency
    pub frequency: u32,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// First seen timestamp
    pub first_seen: DateTime<Utc>,
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    /// Performance characteristics
    pub performance_stats: QueryPerformanceStats,
}

/// Column reference in query patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnReference {
    pub table: String,
    pub column: String,
    pub operator: String, // =, >, <, LIKE, etc.
    pub selectivity_estimate: f64,
}

/// Join condition in query patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinCondition {
    pub left_table: String,
    pub left_column: String,
    pub right_table: String,
    pub right_column: String,
    pub join_type: String,
}

/// Column access pattern tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnAccessPattern {
    pub table: String,
    pub column: String,
    pub access_count: u32,
    pub filter_usage_count: u32,
    pub join_usage_count: u32,
    pub order_usage_count: u32,
    pub group_usage_count: u32,
    pub selectivity_history: Vec<f64>,
    pub last_updated: DateTime<Utc>,
}

/// Join pattern tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinPattern {
    pub left_table: String,
    pub right_table: String,
    pub join_columns: Vec<(String, String)>, // (left_col, right_col)
    pub frequency: u32,
    pub avg_selectivity: f64,
    pub last_seen: DateTime<Utc>,
}

/// Performance statistics for query patterns
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryPerformanceStats {
    pub min_execution_time_ms: f64,
    pub max_execution_time_ms: f64,
    pub p95_execution_time_ms: f64,
    pub rows_examined_avg: u64,
    pub rows_returned_avg: u64,
    pub index_usage_count: u32,
    pub full_table_scan_count: u32,
}

/// Index recommendation with detailed analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecommendation {
    /// Recommendation ID
    pub id: String,
    /// Target table
    pub table_name: String,
    /// Recommended index
    pub index_spec: RecommendedIndex,
    /// Expected performance improvement
    pub expected_improvement: PerformanceImprovement,
    /// Supporting query patterns
    pub supporting_queries: Vec<String>,
    /// Impact analysis
    pub impact_analysis: IndexImpactAnalysis,
    /// Priority score (higher = more important)
    pub priority_score: f64,
    /// Recommendation confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Recommended index specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedIndex {
    pub index_name: String,
    pub index_type: IndexType,
    pub columns: Vec<String>,
    pub included_columns: Vec<String>,    // For covering indexes
    pub filter_condition: Option<String>, // For partial indexes
    pub estimated_size_bytes: u64,
}

/// Performance improvement estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceImprovement {
    /// Expected query speedup factor (e.g., 3.0 = 3x faster)
    pub speedup_factor: f64,
    /// Expected reduction in execution time (milliseconds)
    pub time_savings_ms: f64,
    /// Expected reduction in I/O operations
    pub io_reduction_percentage: f64,
    /// Expected CPU savings
    pub cpu_savings_percentage: f64,
    /// Number of queries that would benefit
    pub affected_query_count: u32,
}

/// Index impact analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexImpactAnalysis {
    /// Storage overhead in bytes
    pub storage_overhead_bytes: u64,
    /// Maintenance cost increase
    pub maintenance_cost_factor: f64,
    /// Write performance impact (percentage degradation)
    pub write_impact_percentage: f64,
    /// Memory usage increase
    pub memory_usage_increase_mb: f64,
    /// Risk assessment
    pub risk_level: RiskLevel,
}

/// Risk level for index recommendations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,    // Minimal impact, high benefit
    Medium, // Moderate impact, good benefit
    High,   // Significant impact, uncertain benefit
}

/// Redundant index detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundantIndexAnalysis {
    /// Redundant indexes found
    pub redundant_indexes: Vec<RedundantIndexGroup>,
    /// Potentially unused indexes
    pub unused_indexes: Vec<UnusedIndex>,
    /// Total storage that could be reclaimed
    pub reclaimable_storage_bytes: u64,
}

/// Group of redundant indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundantIndexGroup {
    pub primary_index: IndexMetadata,
    pub redundant_indexes: Vec<IndexMetadata>,
    pub reason: String,
    pub recommended_action: String,
}

/// Unused index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedIndex {
    pub index: IndexMetadata,
    pub last_used: Option<DateTime<Utc>>,
    pub usage_stats: IndexUsageStats,
    pub recommendation: String,
}

impl IndexRecommendationEngine {
    /// Create a new index recommendation engine
    pub fn new(
        cost_model: CostModel,
        statistics_manager: Arc<RwLock<StatisticsManager>>,
        index_selector: Arc<IndexSelector>,
    ) -> Self {
        Self {
            query_analyzer: QueryPatternAnalyzer::new(),
            cost_model,
            statistics_manager,
            index_selector,
            config: RecommendationConfig::default(),
        }
    }

    /// Create engine with custom configuration
    pub fn with_config(
        cost_model: CostModel,
        statistics_manager: Arc<RwLock<StatisticsManager>>,
        index_selector: Arc<IndexSelector>,
        config: RecommendationConfig,
    ) -> Self {
        Self {
            query_analyzer: QueryPatternAnalyzer::new(),
            cost_model,
            statistics_manager,
            index_selector,
            config,
        }
    }

    /// Record a query execution for pattern analysis
    pub async fn record_query_execution(
        &self,
        query: &Statement,
        execution_time_ms: f64,
        rows_examined: u64,
        rows_returned: u64,
        used_indexes: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.query_analyzer
            .record_query(
                query,
                execution_time_ms,
                rows_examined,
                rows_returned,
                used_indexes,
            )
            .await
    }

    /// Generate index recommendations based on query patterns
    pub async fn generate_recommendations(
        &self,
    ) -> Result<Vec<IndexRecommendation>, Box<dyn std::error::Error>> {
        let mut recommendations = Vec::new();

        // Analyze query patterns
        let query_patterns = self
            .query_analyzer
            .get_significant_patterns(&self.config)
            .await;

        for pattern in query_patterns {
            // Generate recommendations for this pattern
            let pattern_recommendations = self.recommend_indexes_for_pattern(&pattern).await?;
            recommendations.extend(pattern_recommendations);
        }

        // Deduplicate and prioritize recommendations
        let optimized_recommendations = self.optimize_recommendations(recommendations).await;

        // Limit to max recommendations
        Ok(optimized_recommendations
            .into_iter()
            .take(self.config.max_recommendations)
            .collect())
    }

    /// Analyze existing indexes for redundancy and unused indexes
    pub async fn analyze_index_redundancy(
        &self,
    ) -> Result<RedundantIndexAnalysis, Box<dyn std::error::Error>> {
        let mut redundant_groups = Vec::new();
        let mut unused_indexes = Vec::new();
        let mut reclaimable_storage = 0u64;

        // Get all tables with indexes
        let tables = self.get_all_indexed_tables().await;

        for table in tables {
            let table_indexes = self.index_selector.get_table_indexes(&table).await;

            // Find redundant indexes
            let table_redundant = self.find_redundant_indexes(&table_indexes).await;
            redundant_groups.extend(table_redundant);

            // Find unused indexes
            let table_unused = self.find_unused_indexes(&table_indexes).await;
            unused_indexes.extend(table_unused);
        }

        // Calculate reclaimable storage
        for group in &redundant_groups {
            for index in &group.redundant_indexes {
                reclaimable_storage += index.size_bytes;
            }
        }

        for unused in &unused_indexes {
            reclaimable_storage += unused.index.size_bytes;
        }

        Ok(RedundantIndexAnalysis {
            redundant_indexes: redundant_groups,
            unused_indexes,
            reclaimable_storage_bytes: reclaimable_storage,
        })
    }

    /// Generate comprehensive index report
    pub async fn generate_index_report(&self) -> Result<IndexReport, Box<dyn std::error::Error>> {
        let recommendations = self.generate_recommendations().await?;
        let redundancy_analysis = self.analyze_index_redundancy().await?;
        let query_patterns = self.query_analyzer.get_all_patterns().await;

        Ok(IndexReport {
            recommendations,
            redundancy_analysis,
            query_pattern_summary: self.summarize_query_patterns(query_patterns),
            generated_at: Utc::now(),
        })
    }

    // Private helper methods

    async fn recommend_indexes_for_pattern(
        &self,
        pattern: &QueryPattern,
    ) -> Result<Vec<IndexRecommendation>, Box<dyn std::error::Error>> {
        let mut recommendations = Vec::new();

        // Check if pattern meets minimum criteria
        if pattern.frequency < self.config.min_query_frequency {
            return Ok(recommendations);
        }

        // Analyze WHERE clause columns for single-column indexes
        for col_ref in &pattern.where_columns {
            if let Some(recommendation) =
                self.recommend_single_column_index(pattern, col_ref).await?
            {
                recommendations.push(recommendation);
            }
        }

        // Analyze multi-column indexes for WHERE clauses
        if pattern.where_columns.len() > 1 {
            if let Some(recommendation) = self.recommend_multi_column_index(pattern).await? {
                recommendations.push(recommendation);
            }
        }

        // Analyze covering indexes for frequent SELECT patterns
        if let Some(recommendation) = self.recommend_covering_index(pattern).await? {
            recommendations.push(recommendation);
        }

        // Analyze ORDER BY indexes
        for col_ref in &pattern.order_columns {
            if let Some(recommendation) = self.recommend_order_index(pattern, col_ref).await? {
                recommendations.push(recommendation);
            }
        }

        Ok(recommendations)
    }

    async fn recommend_single_column_index(
        &self,
        pattern: &QueryPattern,
        col_ref: &ColumnReference,
    ) -> Result<Option<IndexRecommendation>, Box<dyn std::error::Error>> {
        // Check if table is large enough
        let stats_manager = self.statistics_manager.read().await;
        if let Some(table_stats) = stats_manager.get_table_statistics(&col_ref.table).await {
            if table_stats.row_count < self.config.min_table_size {
                return Ok(None);
            }

            // Estimate improvement
            let current_cost = self
                .estimate_current_query_cost(pattern, &table_stats)
                .await;
            let index_cost = self
                .estimate_index_query_cost(
                    pattern,
                    &col_ref.table,
                    std::slice::from_ref(&col_ref.column),
                )
                .await;

            let speedup = current_cost.total_cost() / index_cost.total_cost();

            if speedup < (1.0 + self.config.min_improvement_threshold / 100.0) {
                return Ok(None);
            }

            // Create recommendation
            let index_spec = RecommendedIndex {
                index_name: format!("idx_{}_{}", col_ref.table, col_ref.column),
                index_type: self.determine_optimal_index_type(&col_ref.operator),
                columns: vec![col_ref.column.clone()],
                included_columns: vec![],
                filter_condition: None,
                estimated_size_bytes: self
                    .estimate_index_size(&table_stats, std::slice::from_ref(&col_ref.column)),
            };

            let improvement = PerformanceImprovement {
                speedup_factor: speedup,
                time_savings_ms: pattern.avg_execution_time_ms * (speedup - 1.0) / speedup,
                io_reduction_percentage: 80.0, // Estimate
                cpu_savings_percentage: 50.0,  // Estimate
                affected_query_count: pattern.frequency,
            };

            let impact = self.analyze_index_impact(&table_stats, &index_spec).await;

            let recommendation = IndexRecommendation {
                id: format!(
                    "rec_{}_{}_{}",
                    Utc::now().timestamp(),
                    col_ref.table,
                    col_ref.column
                ),
                table_name: col_ref.table.clone(),
                index_spec,
                expected_improvement: improvement,
                supporting_queries: vec![pattern.pattern_hash.clone()],
                impact_analysis: impact,
                priority_score: self.calculate_priority_score(speedup, pattern.frequency),
                confidence: 0.8, // High confidence for single-column indexes
                created_at: Utc::now(),
            };

            Ok(Some(recommendation))
        } else {
            Ok(None)
        }
    }

    async fn recommend_multi_column_index(
        &self,
        pattern: &QueryPattern,
    ) -> Result<Option<IndexRecommendation>, Box<dyn std::error::Error>> {
        if pattern.where_columns.len() < 2 {
            return Ok(None);
        }

        // Group columns by table
        let mut table_columns: HashMap<String, Vec<String>> = HashMap::new();
        for col_ref in &pattern.where_columns {
            table_columns
                .entry(col_ref.table.clone())
                .or_default()
                .push(col_ref.column.clone());
        }

        // Create multi-column index recommendation for the table with most columns
        if let Some((table, columns)) = table_columns.iter().max_by_key(|(_, cols)| cols.len()) {
            if columns.len() >= 2 {
                let stats_manager = self.statistics_manager.read().await;
                if let Some(table_stats) = stats_manager.get_table_statistics(table).await {
                    let index_spec = RecommendedIndex {
                        index_name: format!("idx_{table}_composite"),
                        index_type: IndexType::BTree, // Multi-column indexes are typically B-Tree
                        columns: columns.clone(),
                        included_columns: vec![],
                        filter_condition: None,
                        estimated_size_bytes: self.estimate_index_size(&table_stats, columns),
                    };

                    let improvement = PerformanceImprovement {
                        speedup_factor: 5.0, // Composite indexes can provide significant improvement
                        time_savings_ms: pattern.avg_execution_time_ms * 0.8,
                        io_reduction_percentage: 90.0,
                        cpu_savings_percentage: 70.0,
                        affected_query_count: pattern.frequency,
                    };

                    let impact = self.analyze_index_impact(&table_stats, &index_spec).await;

                    let recommendation = IndexRecommendation {
                        id: format!("rec_composite_{}_{}", Utc::now().timestamp(), table),
                        table_name: table.clone(),
                        index_spec,
                        expected_improvement: improvement,
                        supporting_queries: vec![pattern.pattern_hash.clone()],
                        impact_analysis: impact,
                        priority_score: self.calculate_priority_score(5.0, pattern.frequency),
                        confidence: 0.9, // High confidence for composite indexes
                        created_at: Utc::now(),
                    };

                    return Ok(Some(recommendation));
                }
            }
        }

        Ok(None)
    }

    async fn recommend_covering_index(
        &self,
        _pattern: &QueryPattern,
    ) -> Result<Option<IndexRecommendation>, Box<dyn std::error::Error>> {
        // Simplified implementation - in production this would analyze SELECT columns
        // and recommend covering indexes when beneficial
        Ok(None)
    }

    async fn recommend_order_index(
        &self,
        pattern: &QueryPattern,
        col_ref: &ColumnReference,
    ) -> Result<Option<IndexRecommendation>, Box<dyn std::error::Error>> {
        // ORDER BY indexes are beneficial when there's no existing index
        let stats_manager = self.statistics_manager.read().await;
        if let Some(table_stats) = stats_manager.get_table_statistics(&col_ref.table).await {
            if table_stats.row_count >= self.config.min_table_size {
                let index_spec = RecommendedIndex {
                    index_name: format!("idx_{}_order_{}", col_ref.table, col_ref.column),
                    index_type: IndexType::BTree, // B-Tree is optimal for ORDER BY
                    columns: vec![col_ref.column.clone()],
                    included_columns: vec![],
                    filter_condition: None,
                    estimated_size_bytes: self
                        .estimate_index_size(&table_stats, std::slice::from_ref(&col_ref.column)),
                };

                let improvement = PerformanceImprovement {
                    speedup_factor: 3.0, // ORDER BY can benefit significantly from indexes
                    time_savings_ms: pattern.avg_execution_time_ms * 0.67,
                    io_reduction_percentage: 60.0,
                    cpu_savings_percentage: 80.0, // Eliminates sorting
                    affected_query_count: pattern.frequency,
                };

                let impact = self.analyze_index_impact(&table_stats, &index_spec).await;

                let recommendation = IndexRecommendation {
                    id: format!("rec_order_{}_{}", Utc::now().timestamp(), col_ref.column),
                    table_name: col_ref.table.clone(),
                    index_spec,
                    expected_improvement: improvement,
                    supporting_queries: vec![pattern.pattern_hash.clone()],
                    impact_analysis: impact,
                    priority_score: self.calculate_priority_score(3.0, pattern.frequency),
                    confidence: 0.7,
                    created_at: Utc::now(),
                };

                return Ok(Some(recommendation));
            }
        }

        Ok(None)
    }

    async fn optimize_recommendations(
        &self,
        mut recommendations: Vec<IndexRecommendation>,
    ) -> Vec<IndexRecommendation> {
        // Remove duplicates and merge similar recommendations
        recommendations.sort_by(|a, b| {
            b.priority_score
                .partial_cmp(&a.priority_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Simple deduplication by table + columns
        let mut seen = HashSet::new();
        recommendations.retain(|rec| {
            let key = format!("{}_{}", rec.table_name, rec.index_spec.columns.join("_"));
            seen.insert(key)
        });

        recommendations
    }

    async fn find_redundant_indexes(&self, indexes: &[IndexMetadata]) -> Vec<RedundantIndexGroup> {
        let mut redundant_groups = Vec::new();

        // Simple redundancy detection: if one index is a prefix of another
        for (i, index1) in indexes.iter().enumerate() {
            for (j, index2) in indexes.iter().enumerate() {
                if i != j && self.is_redundant(index1, index2) {
                    // Create redundant group
                    redundant_groups.push(RedundantIndexGroup {
                        primary_index: index2.clone(), // Keep the more comprehensive index
                        redundant_indexes: vec![index1.clone()],
                        reason: "Index is a prefix of another index".to_string(),
                        recommended_action: "Drop the redundant index".to_string(),
                    });
                }
            }
        }

        redundant_groups
    }

    async fn find_unused_indexes(&self, indexes: &[IndexMetadata]) -> Vec<UnusedIndex> {
        let mut unused_indexes = Vec::new();

        for index in indexes {
            if let Some(usage_stats) = self
                .index_selector
                .get_index_usage_stats(&index.index_id)
                .await
            {
                // Consider unused if not used in the last 30 days or very low usage
                let is_unused = usage_stats
                    .last_used
                    .is_none_or(|last_used| Utc::now() - last_used > Duration::days(30))
                    || usage_stats.usage_count < 5; // Very low usage

                if is_unused {
                    unused_indexes.push(UnusedIndex {
                        index: index.clone(),
                        last_used: usage_stats.last_used,
                        usage_stats,
                        recommendation: "Consider dropping this unused index".to_string(),
                    });
                }
            } else {
                // No usage stats available, assume unused
                unused_indexes.push(UnusedIndex {
                    index: index.clone(),
                    last_used: None,
                    usage_stats: IndexUsageStats {
                        usage_count: 0,
                        consideration_count: 0,
                        avg_selectivity: 0.0,
                        total_rows_scanned: 0,
                        time_saved_ms: 0,
                        last_used: None,
                        performance_metrics: Default::default(),
                    },
                    recommendation: "No usage statistics available - likely unused".to_string(),
                });
            }
        }

        unused_indexes
    }

    // Helper methods

    fn is_redundant(&self, index1: &IndexMetadata, index2: &IndexMetadata) -> bool {
        // Check if index1 is a prefix of index2
        if index1.columns.len() >= index2.columns.len() {
            return false;
        }

        index1
            .columns
            .iter()
            .zip(index2.columns.iter())
            .all(|(col1, col2)| col1 == col2)
    }

    fn determine_optimal_index_type(&self, operator: &str) -> IndexType {
        match operator {
            "=" => IndexType::Hash,                      // Hash is optimal for equality
            ">" | "<" | ">=" | "<=" => IndexType::BTree, // B-Tree for range queries
            "LIKE" => IndexType::BTree,                  // B-Tree for pattern matching
            _ => IndexType::BTree,                       // Default to B-Tree
        }
    }

    fn estimate_index_size(&self, table_stats: &TableStatistics, columns: &[String]) -> u64 {
        // Rough estimate: 64 bytes per row per column + 20% overhead
        let row_size_estimate = columns.len() as u64 * 64;
        let total_size = table_stats.row_count * row_size_estimate;
        (total_size as f64 * 1.2) as u64 // 20% overhead
    }

    async fn analyze_index_impact(
        &self,
        table_stats: &TableStatistics,
        index_spec: &RecommendedIndex,
    ) -> IndexImpactAnalysis {
        let storage_overhead = index_spec.estimated_size_bytes;
        let table_size = table_stats.row_count * table_stats.avg_row_size as u64;
        let overhead_percentage = (storage_overhead as f64 / table_size as f64) * 100.0;

        let risk_level = if overhead_percentage > 50.0 {
            RiskLevel::High
        } else if overhead_percentage > 20.0 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        IndexImpactAnalysis {
            storage_overhead_bytes: storage_overhead,
            maintenance_cost_factor: 1.05 + (index_spec.columns.len() as f64 * 0.02), // More columns = higher maintenance
            write_impact_percentage: index_spec.columns.len() as f64 * 2.0, // Rough estimate
            memory_usage_increase_mb: (storage_overhead as f64 / (1024.0 * 1024.0)) * 0.1, // Assume 10% of index size in memory
            risk_level,
        }
    }

    fn calculate_priority_score(&self, speedup_factor: f64, frequency: u32) -> f64 {
        // Priority = speedup * log(frequency) * confidence_factor
        speedup_factor * (frequency as f64).ln() * 0.8
    }

    async fn estimate_current_query_cost(
        &self,
        _pattern: &QueryPattern,
        table_stats: &TableStatistics,
    ) -> QueryCost {
        // Simplified cost estimation for current query without index
        let selectivity = 0.1; // Assume 10% selectivity
        let (cost, _) = self
            .cost_model
            .calculate_scan_cost(table_stats, selectivity);
        cost
    }

    async fn estimate_index_query_cost(
        &self,
        _pattern: &QueryPattern,
        _table: &str,
        _columns: &[String],
    ) -> QueryCost {
        // Simplified cost estimation with proposed index
        QueryCost {
            cpu_cost: 50.0,
            io_cost: 100.0,
            memory_cost: 20.0,
            network_cost: 0.0,
            total_time_ms: 200.0,
        }
    }

    async fn get_all_indexed_tables(&self) -> Vec<String> {
        // Mock implementation - in reality would query metadata
        vec![
            "users".to_string(),
            "orders".to_string(),
            "products".to_string(),
        ]
    }

    fn summarize_query_patterns(&self, _patterns: Vec<QueryPattern>) -> QueryPatternSummary {
        QueryPatternSummary {
            total_patterns: 0,
            most_frequent_tables: vec![],
            most_common_operations: vec![],
        }
    }
}

impl Default for QueryPatternAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryPatternAnalyzer {
    pub fn new() -> Self {
        Self {
            query_patterns: Arc::new(RwLock::new(HashMap::new())),
            column_access_patterns: Arc::new(RwLock::new(HashMap::new())),
            join_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn record_query(
        &self,
        query: &Statement,
        execution_time_ms: f64,
        rows_examined: u64,
        rows_returned: u64,
        used_indexes: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Extract pattern from query
        let pattern_hash = self.generate_pattern_hash(query);
        let normalized_query = self.normalize_query(query);

        let mut patterns = self.query_patterns.write().await;
        let pattern = patterns
            .entry(pattern_hash.clone())
            .or_insert_with(|| QueryPattern {
                pattern_hash: pattern_hash.clone(),
                query_template: normalized_query,
                tables: self.extract_tables(query),
                where_columns: self.extract_where_columns(query),
                order_columns: self.extract_order_columns(query),
                group_columns: self.extract_group_columns(query),
                joins: self.extract_joins(query),
                frequency: 0,
                avg_execution_time_ms: 0.0,
                first_seen: Utc::now(),
                last_seen: Utc::now(),
                performance_stats: QueryPerformanceStats::default(),
            });

        // Update pattern statistics
        pattern.frequency += 1;
        pattern.avg_execution_time_ms =
            (pattern.avg_execution_time_ms * (pattern.frequency - 1) as f64 + execution_time_ms)
                / pattern.frequency as f64;
        pattern.last_seen = Utc::now();

        // Update performance stats
        let stats = &mut pattern.performance_stats;
        if execution_time_ms < stats.min_execution_time_ms || stats.min_execution_time_ms == 0.0 {
            stats.min_execution_time_ms = execution_time_ms;
        }
        if execution_time_ms > stats.max_execution_time_ms {
            stats.max_execution_time_ms = execution_time_ms;
        }
        stats.rows_examined_avg = (stats.rows_examined_avg * (pattern.frequency - 1) as u64
            + rows_examined)
            / pattern.frequency as u64;
        stats.rows_returned_avg = (stats.rows_returned_avg * (pattern.frequency - 1) as u64
            + rows_returned)
            / pattern.frequency as u64;

        // Update index usage stats
        if !used_indexes.is_empty() {
            stats.index_usage_count += 1;
        } else {
            stats.full_table_scan_count += 1;
        }

        // Update column access patterns
        self.update_column_access_patterns(query).await;

        // Update join patterns
        self.update_join_patterns(query).await;

        Ok(())
    }

    pub async fn get_significant_patterns(
        &self,
        config: &RecommendationConfig,
    ) -> Vec<QueryPattern> {
        let patterns = self.query_patterns.read().await;
        let cutoff_date = Utc::now() - Duration::days(config.analysis_window_days as i64);

        patterns
            .values()
            .filter(|p| p.frequency >= config.min_query_frequency && p.last_seen >= cutoff_date)
            .cloned()
            .collect()
    }

    pub async fn get_all_patterns(&self) -> Vec<QueryPattern> {
        let patterns = self.query_patterns.read().await;
        patterns.values().cloned().collect()
    }

    // Helper methods for query analysis

    fn generate_pattern_hash(&self, query: &Statement) -> String {
        // Generate a hash that represents the query pattern (not the literal values)
        format!(
            "pattern_{}",
            query.to_string().chars().take(20).collect::<String>()
        )
    }

    fn normalize_query(&self, _query: &Statement) -> String {
        // Normalize query by replacing literals with placeholders
        "SELECT ... FROM ... WHERE ... = ?".to_string()
    }

    fn extract_tables(&self, query: &Statement) -> Vec<String> {
        match query {
            Statement::Select(select) => select
                .from
                .iter()
                .filter_map(|from| match from {
                    FromClause::Table { name, .. } => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        }
    }

    fn extract_where_columns(&self, query: &Statement) -> Vec<ColumnReference> {
        match query {
            Statement::Select(select) => {
                if let Some(where_clause) = &select.where_clause {
                    self.extract_column_references_from_expression(where_clause, "WHERE")
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn extract_order_columns(&self, query: &Statement) -> Vec<ColumnReference> {
        match query {
            Statement::Select(select) => {
                select
                    .order_by
                    .iter()
                    .filter_map(|order| {
                        if let Expression::Identifier(col) = &order.expression {
                            Some(ColumnReference {
                                table: "unknown".to_string(), // Would need table resolution
                                column: col.clone(),
                                operator: "ORDER".to_string(),
                                selectivity_estimate: 1.0,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn extract_group_columns(&self, query: &Statement) -> Vec<ColumnReference> {
        match query {
            Statement::Select(select) => {
                select
                    .group_by
                    .iter()
                    .filter_map(|expr| {
                        if let Expression::Identifier(col) = expr {
                            Some(ColumnReference {
                                table: "unknown".to_string(),
                                column: col.clone(),
                                operator: "GROUP".to_string(),
                                selectivity_estimate: 0.1, // Groups typically reduce cardinality
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn extract_joins(&self, query: &Statement) -> Vec<JoinCondition> {
        match query {
            Statement::Select(select) => {
                select
                    .join_clauses
                    .iter()
                    .filter_map(|join| {
                        if let Expression::Binary {
                            left,
                            operator: BinaryOperator::Equal,
                            right,
                        } = &join.condition
                        {
                            if let (
                                Expression::Identifier(left_col),
                                Expression::Identifier(right_col),
                            ) = (left.as_ref(), right.as_ref())
                            {
                                return Some(JoinCondition {
                                    left_table: "unknown".to_string(), // Would need resolution
                                    left_column: left_col.clone(),
                                    right_table: "unknown".to_string(),
                                    right_column: right_col.clone(),
                                    join_type: format!("{:?}", join.join_type),
                                });
                            }
                        }
                        None
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn extract_column_references_from_expression(
        &self,
        expr: &Expression,
        _context: &str,
    ) -> Vec<ColumnReference> {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                let mut refs = Vec::new();
                if let (Expression::Identifier(col), Expression::Literal(_)) =
                    (left.as_ref(), right.as_ref())
                {
                    refs.push(ColumnReference {
                        table: "unknown".to_string(),
                        column: col.clone(),
                        operator: format!("{operator:?}"),
                        selectivity_estimate: self.estimate_operator_selectivity(operator),
                    });
                }
                refs
            }
            _ => vec![],
        }
    }

    fn estimate_operator_selectivity(&self, operator: &BinaryOperator) -> f64 {
        match operator {
            BinaryOperator::Equal => 0.01,
            BinaryOperator::GreaterThan | BinaryOperator::LessThan => 0.33,
            BinaryOperator::GreaterThanOrEqual | BinaryOperator::LessThanOrEqual => 0.33,
            BinaryOperator::Like => 0.1,
            _ => 0.1,
        }
    }

    /// Update column access patterns based on query analysis
    async fn update_column_access_patterns(&self, query: &Statement) {
        let mut patterns = self.column_access_patterns.write().await;

        let where_cols = self.extract_where_columns(query);
        let order_cols = self.extract_order_columns(query);
        let group_cols = self.extract_group_columns(query);

        // Track WHERE column usage
        for col_ref in where_cols {
            let key = format!("{}.{}", col_ref.table, col_ref.column);
            let pattern = patterns.entry(key).or_insert_with(|| ColumnAccessPattern {
                table: col_ref.table.clone(),
                column: col_ref.column.clone(),
                access_count: 0,
                filter_usage_count: 0,
                join_usage_count: 0,
                order_usage_count: 0,
                group_usage_count: 0,
                selectivity_history: Vec::new(),
                last_updated: Utc::now(),
            });

            pattern.access_count += 1;
            pattern.filter_usage_count += 1;
            pattern
                .selectivity_history
                .push(col_ref.selectivity_estimate);
            pattern.last_updated = Utc::now();
        }

        // Track ORDER BY column usage
        for col_ref in order_cols {
            let key = format!("{}.{}", col_ref.table, col_ref.column);
            let pattern = patterns.entry(key).or_insert_with(|| ColumnAccessPattern {
                table: col_ref.table.clone(),
                column: col_ref.column.clone(),
                access_count: 0,
                filter_usage_count: 0,
                join_usage_count: 0,
                order_usage_count: 0,
                group_usage_count: 0,
                selectivity_history: Vec::new(),
                last_updated: Utc::now(),
            });

            pattern.access_count += 1;
            pattern.order_usage_count += 1;
            pattern.last_updated = Utc::now();
        }

        // Track GROUP BY column usage
        for col_ref in group_cols {
            let key = format!("{}.{}", col_ref.table, col_ref.column);
            let pattern = patterns.entry(key).or_insert_with(|| ColumnAccessPattern {
                table: col_ref.table.clone(),
                column: col_ref.column.clone(),
                access_count: 0,
                filter_usage_count: 0,
                join_usage_count: 0,
                order_usage_count: 0,
                group_usage_count: 0,
                selectivity_history: Vec::new(),
                last_updated: Utc::now(),
            });

            pattern.access_count += 1;
            pattern.group_usage_count += 1;
            pattern.last_updated = Utc::now();
        }
    }

    /// Update join patterns based on query analysis
    async fn update_join_patterns(&self, query: &Statement) {
        let mut patterns = self.join_patterns.write().await;

        let joins = self.extract_joins(query);

        for join in joins {
            let key = format!(
                "{}.{}:{}.{}",
                join.left_table, join.left_column, join.right_table, join.right_column
            );

            let pattern = patterns.entry(key).or_insert_with(|| JoinPattern {
                left_table: join.left_table.clone(),
                right_table: join.right_table.clone(),
                join_columns: vec![(join.left_column.clone(), join.right_column.clone())],
                frequency: 0,
                avg_selectivity: 0.5, // Default estimate
                last_seen: Utc::now(),
            });

            pattern.frequency += 1;
            pattern.last_seen = Utc::now();

            // Update join columns if not already present
            let join_col_pair = (join.left_column, join.right_column);
            if !pattern.join_columns.contains(&join_col_pair) {
                pattern.join_columns.push(join_col_pair);
            }
        }
    }

    /// Get column access patterns for analysis
    pub async fn get_column_access_patterns(&self) -> HashMap<String, ColumnAccessPattern> {
        self.column_access_patterns.read().await.clone()
    }

    /// Get join patterns for analysis
    pub async fn get_join_patterns(&self) -> HashMap<String, JoinPattern> {
        self.join_patterns.read().await.clone()
    }

    /// Get column access pattern statistics
    pub async fn get_column_usage_stats(&self) -> usize {
        self.column_access_patterns.read().await.len()
    }

    /// Get join pattern statistics  
    pub async fn get_join_usage_stats(&self) -> usize {
        self.join_patterns.read().await.len()
    }
}

/// Comprehensive index report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexReport {
    pub recommendations: Vec<IndexRecommendation>,
    pub redundancy_analysis: RedundantIndexAnalysis,
    pub query_pattern_summary: QueryPatternSummary,
    pub generated_at: DateTime<Utc>,
}

/// Summary of query patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPatternSummary {
    pub total_patterns: usize,
    pub most_frequent_tables: Vec<String>,
    pub most_common_operations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QueryValue;

    #[tokio::test]
    async fn test_recommendation_engine_creation() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let index_selector = Arc::new(IndexSelector::new(
            stats_manager.clone(),
            cost_model.clone(),
        ));

        let engine = IndexRecommendationEngine::new(cost_model, stats_manager, index_selector);
        assert_eq!(engine.config.min_query_frequency, 10);
    }

    #[tokio::test]
    async fn test_query_pattern_recording() {
        let analyzer = QueryPatternAnalyzer::new();

        let query = Statement::Select(SelectStatement {
            with_clauses: Vec::new(),
            fields: vec![SelectField::All],
            from: vec![FromClause::Table {
                name: "users".to_string(),
                alias: None,
            }],
            where_clause: Some(Expression::Binary {
                left: Box::new(Expression::Identifier("age".to_string())),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(QueryValue::Integer(18))),
            }),
            join_clauses: vec![],
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
            fetch: vec![],
            timeout: None,
        });

        analyzer
            .record_query(&query, 150.0, 1000, 50, &[])
            .await
            .unwrap();

        let config = RecommendationConfig::default();
        let patterns = analyzer.get_significant_patterns(&config).await;
        assert_eq!(patterns.len(), 0); // Frequency too low with default config
    }

    #[test]
    fn test_risk_level_assessment() {
        let risk = RiskLevel::Low;
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_priority_calculation() {
        let engine = IndexRecommendationEngine {
            query_analyzer: QueryPatternAnalyzer::new(),
            cost_model: CostModel::new(),
            statistics_manager: Arc::new(RwLock::new(StatisticsManager::default())),
            index_selector: Arc::new(IndexSelector::new(
                Arc::new(RwLock::new(StatisticsManager::default())),
                CostModel::new(),
            )),
            config: RecommendationConfig::default(),
        };

        let priority = engine.calculate_priority_score(3.0, 100);
        assert!(priority > 0.0);
    }
}
