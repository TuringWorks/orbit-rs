//! Automatic Index Recommendation Engine
//!
//! This module analyzes query patterns and workload characteristics to
//! recommend optimal indexes for improved query performance.
//!
//! ## Features
//!
//! - Query workload analysis
//! - Index benefit estimation
//! - Index cost estimation (storage, write overhead)
//! - Multi-column index suggestions
//! - Covering index recommendations
//! - Duplicate/redundant index detection

use crate::protocols::postgres_wire::sql::ast::{
    BinaryOperator, Expression, FromClause, OrderByItem, SelectStatement, SortDirection, Statement,
};
use crate::protocols::postgres_wire::sql::statistics::TableStatistics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for index advisor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexAdvisorConfig {
    /// Enable index recommendations
    pub enabled: bool,
    /// Minimum query count for a pattern to be considered
    pub min_query_count: usize,
    /// Minimum improvement ratio to recommend an index
    pub min_improvement_ratio: f64,
    /// Maximum indexes to recommend per table
    pub max_indexes_per_table: usize,
    /// Consider covering indexes
    pub recommend_covering_indexes: bool,
    /// Maximum columns in a multi-column index
    pub max_columns_per_index: usize,
    /// Consider partial indexes
    pub recommend_partial_indexes: bool,
}

impl Default for IndexAdvisorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_query_count: 10,
            min_improvement_ratio: 2.0,
            max_indexes_per_table: 10,
            recommend_covering_indexes: true,
            max_columns_per_index: 4,
            recommend_partial_indexes: true,
        }
    }
}

/// Recommended index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecommendation {
    /// Table name
    pub table_name: String,
    /// Columns to index (in order)
    pub columns: Vec<String>,
    /// Index type
    pub index_type: IndexType,
    /// Estimated improvement ratio
    pub improvement_ratio: f64,
    /// Estimated storage cost (bytes)
    pub storage_cost: usize,
    /// Write overhead factor (1.0 = no overhead)
    pub write_overhead: f64,
    /// Queries that would benefit
    pub benefiting_queries: usize,
    /// Reason for recommendation
    pub reason: String,
    /// Priority (higher = more important)
    pub priority: u32,
    /// Optional partial index condition
    pub partial_condition: Option<String>,
    /// Whether this is a covering index
    pub is_covering: bool,
    /// Additional columns for covering (INCLUDE clause)
    pub include_columns: Vec<String>,
}

impl IndexRecommendation {
    /// Generate CREATE INDEX statement
    pub fn to_sql(&self) -> String {
        let index_name = format!("idx_{}_{}", self.table_name, self.columns.join("_"));

        let columns_sql = self.columns.join(", ");

        let mut sql = format!(
            "CREATE INDEX {} ON {} USING {} ({})",
            index_name,
            self.table_name,
            self.index_type.to_sql(),
            columns_sql
        );

        if !self.include_columns.is_empty() {
            sql.push_str(&format!(" INCLUDE ({})", self.include_columns.join(", ")));
        }

        if let Some(condition) = &self.partial_condition {
            sql.push_str(&format!(" WHERE {}", condition));
        }

        sql.push(';');
        sql
    }
}

/// Index type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    /// B-tree index (default, good for equality and range)
    BTree,
    /// Hash index (good for equality only)
    Hash,
    /// GiST index (good for geometric data)
    GiST,
    /// GIN index (good for arrays, full-text search)
    GIN,
    /// BRIN index (good for large, naturally ordered tables)
    Brin,
}

impl IndexType {
    fn to_sql(&self) -> &'static str {
        match self {
            IndexType::BTree => "btree",
            IndexType::Hash => "hash",
            IndexType::GiST => "gist",
            IndexType::GIN => "gin",
            IndexType::Brin => "brin",
        }
    }
}

/// Query pattern for analysis
#[derive(Debug, Clone)]
pub struct QueryPattern {
    /// Table name
    pub table_name: String,
    /// Columns used in WHERE clause with operators
    pub filter_columns: Vec<(String, FilterOperator)>,
    /// Columns used in ORDER BY
    pub order_by_columns: Vec<(String, bool)>, // (column, ascending)
    /// Columns used in GROUP BY
    pub group_by_columns: Vec<String>,
    /// Columns in SELECT list
    pub select_columns: Vec<String>,
    /// Join columns
    pub join_columns: Vec<String>,
    /// Query count
    pub count: usize,
    /// Average execution time (ms)
    pub avg_execution_time_ms: f64,
}

/// Filter operator type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilterOperator {
    Equality,
    Range,
    Like,
    In,
    IsNull,
    Other,
}

/// Workload analyzer for query patterns
pub struct WorkloadAnalyzer {
    /// Query patterns by table
    patterns: HashMap<String, Vec<QueryPattern>>,
    /// Pattern fingerprints to avoid duplicates
    fingerprints: HashMap<String, usize>,
}

impl WorkloadAnalyzer {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            fingerprints: HashMap::new(),
        }
    }

    /// Analyze a query and update patterns
    pub fn analyze_query(&mut self, statement: &Statement, execution_time_ms: f64) {
        if let Statement::Select(select) = statement {
            if let Some(pattern) = self.extract_pattern(select) {
                self.update_pattern(pattern, execution_time_ms);
            }
        }
    }

    /// Extract pattern from SELECT statement
    fn extract_pattern(&self, select: &SelectStatement) -> Option<QueryPattern> {
        // Get table name from FROM clause
        let table_name = match &select.from_clause {
            Some(FromClause::Table { name, .. }) => name.full_name(),
            Some(FromClause::Join { left, .. }) => {
                // Use left table for joins
                if let FromClause::Table { name, .. } = left.as_ref() {
                    name.full_name()
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        // Extract filter columns from WHERE
        let filter_columns = select
            .where_clause
            .as_ref()
            .map(|expr| self.extract_filter_columns(expr))
            .unwrap_or_default();

        // Extract ORDER BY columns
        let order_by_columns = select
            .order_by
            .as_ref()
            .map(|items| self.extract_order_by_columns(items))
            .unwrap_or_default();

        // Extract GROUP BY columns
        let group_by_columns = select
            .group_by
            .as_ref()
            .map(|exprs| self.extract_column_names(exprs))
            .unwrap_or_default();

        // Extract SELECT columns
        let select_columns = self.extract_select_columns(&select.select_list);

        // Extract join columns
        let join_columns = self.extract_join_columns(&select.from_clause);

        Some(QueryPattern {
            table_name,
            filter_columns,
            order_by_columns,
            group_by_columns,
            select_columns,
            join_columns,
            count: 1,
            avg_execution_time_ms: 0.0,
        })
    }

    /// Extract filter columns from WHERE clause
    fn extract_filter_columns(&self, expr: &Expression) -> Vec<(String, FilterOperator)> {
        let mut columns = Vec::new();
        self.extract_filter_columns_recursive(expr, &mut columns);
        columns
    }

    fn extract_filter_columns_recursive(
        &self,
        expr: &Expression,
        columns: &mut Vec<(String, FilterOperator)>,
    ) {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                // Check for column = value patterns
                if let Some(col) = self.extract_column_name(left) {
                    let op = match operator {
                        BinaryOperator::Equal => FilterOperator::Equality,
                        BinaryOperator::LessThan
                        | BinaryOperator::LessThanOrEqual
                        | BinaryOperator::GreaterThan
                        | BinaryOperator::GreaterThanOrEqual => FilterOperator::Range,
                        BinaryOperator::Like | BinaryOperator::ILike => FilterOperator::Like,
                        BinaryOperator::In => FilterOperator::In,
                        _ => FilterOperator::Other,
                    };
                    columns.push((col, op));
                }

                // Also check right side (for cases like 5 = column)
                if let Some(col) = self.extract_column_name(right) {
                    if matches!(operator, BinaryOperator::Equal) {
                        columns.push((col, FilterOperator::Equality));
                    }
                }

                // Recurse for AND/OR
                if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                    self.extract_filter_columns_recursive(left, columns);
                    self.extract_filter_columns_recursive(right, columns);
                }
            }
            Expression::IsNull { expr, .. } => {
                if let Some(col) = self.extract_column_name(expr) {
                    columns.push((col, FilterOperator::IsNull));
                }
            }
            Expression::In { expr, .. } => {
                if let Some(col) = self.extract_column_name(expr) {
                    columns.push((col, FilterOperator::In));
                }
            }
            Expression::Between { expr, .. } => {
                if let Some(col) = self.extract_column_name(expr) {
                    columns.push((col, FilterOperator::Range));
                }
            }
            _ => {}
        }
    }

    fn extract_column_name(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Column(col_ref) => Some(col_ref.name.clone()),
            _ => None,
        }
    }

    fn extract_order_by_columns(&self, items: &[OrderByItem]) -> Vec<(String, bool)> {
        items
            .iter()
            .filter_map(|item| {
                self.extract_column_name(&item.expression).map(|col| {
                    let ascending = match item.direction {
                        Some(SortDirection::Descending) => false,
                        _ => true, // Default to ascending
                    };
                    (col, ascending)
                })
            })
            .collect()
    }

    fn extract_column_names(&self, exprs: &[Expression]) -> Vec<String> {
        exprs
            .iter()
            .filter_map(|expr| self.extract_column_name(expr))
            .collect()
    }

    fn extract_select_columns(
        &self,
        _select_list: &[crate::protocols::postgres_wire::sql::ast::SelectItem],
    ) -> Vec<String> {
        // Simplified - would need full implementation
        Vec::new()
    }

    fn extract_join_columns(&self, from_clause: &Option<FromClause>) -> Vec<String> {
        let mut columns = Vec::new();

        if let Some(FromClause::Join {
            condition: crate::protocols::postgres_wire::sql::ast::JoinCondition::On(expr),
            ..
        }) = from_clause
        {
            self.extract_join_columns_recursive(expr, &mut columns);
        }

        columns
    }

    fn extract_join_columns_recursive(&self, expr: &Expression, columns: &mut Vec<String>) {
        if let Expression::Binary {
            left,
            operator,
            right,
        } = expr
        {
            if matches!(operator, BinaryOperator::Equal) {
                if let Some(col) = self.extract_column_name(left) {
                    columns.push(col);
                }
                if let Some(col) = self.extract_column_name(right) {
                    columns.push(col);
                }
            }
            if matches!(operator, BinaryOperator::And) {
                self.extract_join_columns_recursive(left, columns);
                self.extract_join_columns_recursive(right, columns);
            }
        }
    }

    /// Update pattern with new observation
    fn update_pattern(&mut self, pattern: QueryPattern, execution_time_ms: f64) {
        let fingerprint = self.compute_fingerprint(&pattern);

        if let Some(&idx) = self.fingerprints.get(&fingerprint) {
            // Update existing pattern
            if let Some(patterns) = self.patterns.get_mut(&pattern.table_name) {
                if let Some(existing) = patterns.get_mut(idx) {
                    let n = existing.count as f64;
                    existing.avg_execution_time_ms =
                        (existing.avg_execution_time_ms * n + execution_time_ms) / (n + 1.0);
                    existing.count += 1;
                }
            }
        } else {
            // Add new pattern
            let table = pattern.table_name.clone();
            let idx = self.patterns.entry(table.clone()).or_default().len();

            let mut new_pattern = pattern;
            new_pattern.avg_execution_time_ms = execution_time_ms;

            self.patterns.entry(table).or_default().push(new_pattern);
            self.fingerprints.insert(fingerprint, idx);
        }
    }

    fn compute_fingerprint(&self, pattern: &QueryPattern) -> String {
        format!(
            "{}:{}:{}:{}",
            pattern.table_name,
            pattern
                .filter_columns
                .iter()
                .map(|(c, _)| c.as_str())
                .collect::<Vec<_>>()
                .join(","),
            pattern
                .order_by_columns
                .iter()
                .map(|(c, _)| c.as_str())
                .collect::<Vec<_>>()
                .join(","),
            pattern.group_by_columns.join(","),
        )
    }

    /// Get patterns for a table
    pub fn get_patterns(&self, table_name: &str) -> Vec<&QueryPattern> {
        self.patterns
            .get(table_name)
            .map(|ps| ps.iter().collect())
            .unwrap_or_default()
    }

    /// Get all patterns
    pub fn all_patterns(&self) -> impl Iterator<Item = &QueryPattern> {
        self.patterns.values().flatten()
    }
}

impl Default for WorkloadAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Index advisor engine
pub struct IndexAdvisor {
    config: IndexAdvisorConfig,
    workload: Arc<RwLock<WorkloadAnalyzer>>,
    existing_indexes: Arc<RwLock<HashMap<String, Vec<ExistingIndex>>>>,
}

/// Existing index information
#[derive(Debug, Clone)]
pub struct ExistingIndex {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub index_type: IndexType,
    pub is_unique: bool,
    pub is_primary: bool,
}

impl IndexAdvisor {
    /// Create a new index advisor
    pub fn new(config: IndexAdvisorConfig) -> Self {
        Self {
            config,
            workload: Arc::new(RwLock::new(WorkloadAnalyzer::new())),
            existing_indexes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(IndexAdvisorConfig::default())
    }

    /// Record a query for workload analysis
    pub async fn record_query(&self, statement: &Statement, execution_time_ms: f64) {
        if !self.config.enabled {
            return;
        }

        let mut workload = self.workload.write().await;
        workload.analyze_query(statement, execution_time_ms);
    }

    /// Register an existing index
    pub async fn register_existing_index(&self, index: ExistingIndex) {
        let mut indexes = self.existing_indexes.write().await;
        indexes
            .entry(index.table_name.clone())
            .or_default()
            .push(index);
    }

    /// Generate index recommendations
    pub async fn recommend(
        &self,
        table_name: &str,
        table_stats: Option<&TableStatistics>,
    ) -> Vec<IndexRecommendation> {
        if !self.config.enabled {
            return Vec::new();
        }

        let workload = self.workload.read().await;
        let existing = self.existing_indexes.read().await;
        let existing_indexes = existing.get(table_name).cloned().unwrap_or_default();

        let patterns = workload.get_patterns(table_name);

        // Filter patterns with sufficient query count
        let significant_patterns: Vec<_> = patterns
            .into_iter()
            .filter(|p| p.count >= self.config.min_query_count)
            .collect();

        if significant_patterns.is_empty() {
            return Vec::new();
        }

        let mut recommendations = Vec::new();

        // Analyze each pattern
        for pattern in significant_patterns {
            // Generate single-column index recommendations
            for (column, operator) in &pattern.filter_columns {
                if self.is_column_indexed(&existing_indexes, std::slice::from_ref(column)) {
                    continue;
                }

                let index_type = self.suggest_index_type(operator);
                let improvement =
                    self.estimate_improvement(pattern, std::slice::from_ref(column), table_stats);

                if improvement >= self.config.min_improvement_ratio {
                    recommendations.push(IndexRecommendation {
                        table_name: table_name.to_string(),
                        columns: vec![column.clone()],
                        index_type,
                        improvement_ratio: improvement,
                        storage_cost: self.estimate_storage(table_stats, std::slice::from_ref(column)),
                        write_overhead: 1.1, // 10% write overhead for single column
                        benefiting_queries: pattern.count,
                        reason: format!("Frequently filtered by {} with {:?}", column, operator),
                        priority: self.calculate_priority(improvement, pattern.count),
                        partial_condition: None,
                        is_covering: false,
                        include_columns: Vec::new(),
                    });
                }
            }

            // Consider multi-column indexes for equality + range patterns
            if pattern.filter_columns.len() >= 2 {
                let equality_cols: Vec<_> = pattern
                    .filter_columns
                    .iter()
                    .filter(|(_, op)| *op == FilterOperator::Equality)
                    .map(|(c, _)| c.clone())
                    .collect();

                let range_cols: Vec<_> = pattern
                    .filter_columns
                    .iter()
                    .filter(|(_, op)| *op == FilterOperator::Range)
                    .map(|(c, _)| c.clone())
                    .collect();

                // Best practice: equality columns first, then range
                let mut multi_cols = equality_cols;
                multi_cols.extend(range_cols);
                multi_cols.truncate(self.config.max_columns_per_index);

                if multi_cols.len() >= 2 && !self.is_column_indexed(&existing_indexes, &multi_cols)
                {
                    let improvement = self.estimate_improvement(pattern, &multi_cols, table_stats);

                    if improvement >= self.config.min_improvement_ratio {
                        recommendations.push(IndexRecommendation {
                            table_name: table_name.to_string(),
                            columns: multi_cols.clone(),
                            index_type: IndexType::BTree,
                            improvement_ratio: improvement,
                            storage_cost: self.estimate_storage(table_stats, &multi_cols),
                            write_overhead: 1.0 + 0.05 * multi_cols.len() as f64,
                            benefiting_queries: pattern.count,
                            reason: "Composite index for multi-column filter".to_string(),
                            priority: self.calculate_priority(improvement, pattern.count),
                            partial_condition: None,
                            is_covering: false,
                            include_columns: Vec::new(),
                        });
                    }
                }
            }

            // Consider covering indexes
            if self.config.recommend_covering_indexes && !pattern.select_columns.is_empty() {
                let filter_cols: Vec<_> = pattern
                    .filter_columns
                    .iter()
                    .map(|(c, _)| c.clone())
                    .collect();

                if !filter_cols.is_empty() {
                    let include_cols: Vec<_> = pattern
                        .select_columns
                        .iter()
                        .filter(|c| !filter_cols.contains(c))
                        .cloned()
                        .collect();

                    if !include_cols.is_empty() {
                        let improvement =
                            self.estimate_improvement(pattern, &filter_cols, table_stats) * 1.3;

                        if improvement >= self.config.min_improvement_ratio {
                            recommendations.push(IndexRecommendation {
                                table_name: table_name.to_string(),
                                columns: filter_cols.clone(),
                                index_type: IndexType::BTree,
                                improvement_ratio: improvement,
                                storage_cost: self.estimate_storage(
                                    table_stats,
                                    &[filter_cols.clone(), include_cols.clone()].concat(),
                                ),
                                write_overhead: 1.2,
                                benefiting_queries: pattern.count,
                                reason: "Covering index to avoid heap lookups".to_string(),
                                priority: self.calculate_priority(improvement, pattern.count),
                                partial_condition: None,
                                is_covering: true,
                                include_columns: include_cols,
                            });
                        }
                    }
                }
            }

            // Consider index for ORDER BY
            if !pattern.order_by_columns.is_empty() {
                let order_cols: Vec<_> = pattern
                    .order_by_columns
                    .iter()
                    .map(|(c, _)| c.clone())
                    .collect();

                if !self.is_column_indexed(&existing_indexes, &order_cols) {
                    let improvement = self.estimate_sort_improvement(table_stats);

                    if improvement >= self.config.min_improvement_ratio {
                        recommendations.push(IndexRecommendation {
                            table_name: table_name.to_string(),
                            columns: order_cols.clone(),
                            index_type: IndexType::BTree,
                            improvement_ratio: improvement,
                            storage_cost: self.estimate_storage(table_stats, &order_cols),
                            write_overhead: 1.1,
                            benefiting_queries: pattern.count,
                            reason: "Index for ORDER BY optimization".to_string(),
                            priority: self.calculate_priority(improvement, pattern.count),
                            partial_condition: None,
                            is_covering: false,
                            include_columns: Vec::new(),
                        });
                    }
                }
            }
        }

        // Deduplicate and sort by priority
        self.deduplicate_recommendations(&mut recommendations);
        recommendations.sort_by(|a, b| b.priority.cmp(&a.priority));
        recommendations.truncate(self.config.max_indexes_per_table);

        recommendations
    }

    /// Detect redundant existing indexes
    pub async fn detect_redundant_indexes(&self, table_name: &str) -> Vec<RedundantIndex> {
        let existing = self.existing_indexes.read().await;
        let indexes = existing.get(table_name).cloned().unwrap_or_default();

        let mut redundant = Vec::new();

        for (i, idx1) in indexes.iter().enumerate() {
            for idx2 in indexes.iter().skip(i + 1) {
                // Check if idx1 is a prefix of idx2 (making idx1 redundant)
                if self.is_prefix(&idx1.columns, &idx2.columns)
                    && !idx1.is_primary
                    && !idx1.is_unique
                {
                    redundant.push(RedundantIndex {
                        redundant_index: idx1.name.clone(),
                        superseded_by: idx2.name.clone(),
                        reason: format!("Index {} is a prefix of {}", idx1.name, idx2.name),
                    });
                }

                // Check reverse
                if self.is_prefix(&idx2.columns, &idx1.columns)
                    && !idx2.is_primary
                    && !idx2.is_unique
                {
                    redundant.push(RedundantIndex {
                        redundant_index: idx2.name.clone(),
                        superseded_by: idx1.name.clone(),
                        reason: format!("Index {} is a prefix of {}", idx2.name, idx1.name),
                    });
                }
            }
        }

        redundant
    }

    /// Check if columns are already indexed
    fn is_column_indexed(&self, existing: &[ExistingIndex], columns: &[String]) -> bool {
        existing.iter().any(|idx| {
            // Check if existing index covers these columns (as a prefix)
            columns.len() <= idx.columns.len()
                && columns.iter().zip(&idx.columns).all(|(a, b)| a == b)
        })
    }

    fn is_prefix(&self, shorter: &[String], longer: &[String]) -> bool {
        shorter.len() < longer.len() && shorter.iter().zip(longer).all(|(a, b)| a == b)
    }

    fn suggest_index_type(&self, operator: &FilterOperator) -> IndexType {
        match operator {
            FilterOperator::Equality => IndexType::Hash, // Hash is faster for equality
            FilterOperator::Range | FilterOperator::Like => IndexType::BTree,
            FilterOperator::In => IndexType::BTree,
            FilterOperator::IsNull => IndexType::BTree,
            FilterOperator::Other => IndexType::BTree,
        }
    }

    fn estimate_improvement(
        &self,
        _pattern: &QueryPattern,
        _columns: &[String],
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        let row_count = table_stats.map(|s| s.row_count).unwrap_or(10_000) as f64;

        // Simple model: improvement = log2(row_count) for indexed access vs full scan
        let scan_cost = row_count;
        let index_cost = row_count.log2().max(1.0) * 10.0; // Index lookup + some heap reads

        // Adjust based on filter selectivity
        let selectivity = 0.1; // Assume 10% selectivity
        let filtered_rows = row_count * selectivity;

        if filtered_rows < 100.0 {
            // Very selective - index is highly beneficial
            scan_cost / index_cost * 2.0
        } else {
            scan_cost / (index_cost + filtered_rows)
        }
    }

    fn estimate_sort_improvement(&self, table_stats: Option<&TableStatistics>) -> f64 {
        let row_count = table_stats.map(|s| s.row_count).unwrap_or(10_000) as f64;

        // Sort cost without index: O(n log n)
        let sort_cost = row_count * row_count.log2();

        // With index: O(n) for reading in order
        let indexed_cost = row_count;

        sort_cost / indexed_cost
    }

    fn estimate_storage(&self, table_stats: Option<&TableStatistics>, columns: &[String]) -> usize {
        let row_count = table_stats.map(|s| s.row_count).unwrap_or(10_000);

        // Rough estimate: 20 bytes per column per row for B-tree
        let bytes_per_row = 20 * columns.len();

        // Add ~40% overhead for B-tree structure
        (row_count * bytes_per_row * 14) / 10
    }

    fn calculate_priority(&self, improvement: f64, query_count: usize) -> u32 {
        // Priority = improvement * log(query_count + 1)
        let count_factor = (query_count as f64 + 1.0).ln();
        (improvement * count_factor * 100.0) as u32
    }

    fn deduplicate_recommendations(&self, recommendations: &mut Vec<IndexRecommendation>) {
        recommendations.sort_by(|a, b| a.columns.cmp(&b.columns));
        recommendations.dedup_by(|a, b| a.columns == b.columns);
    }

    /// Get workload summary
    pub async fn workload_summary(&self) -> WorkloadSummary {
        let workload = self.workload.read().await;
        let patterns: Vec<_> = workload.all_patterns().collect();

        let total_queries: usize = patterns.iter().map(|p| p.count).sum();
        let unique_patterns = patterns.len();

        let top_tables: Vec<_> = {
            let mut table_counts: HashMap<&str, usize> = HashMap::new();
            for p in &patterns {
                *table_counts.entry(&p.table_name).or_default() += p.count;
            }
            let mut counts: Vec<_> = table_counts.into_iter().collect();
            counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            counts
                .into_iter()
                .take(10)
                .map(|(t, c)| (t.to_string(), c))
                .collect()
        };

        WorkloadSummary {
            total_queries,
            unique_patterns,
            top_tables,
        }
    }
}

impl Default for IndexAdvisor {
    fn default() -> Self {
        Self::new_default()
    }
}

/// Redundant index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundantIndex {
    pub redundant_index: String,
    pub superseded_by: String,
    pub reason: String,
}

/// Workload summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadSummary {
    pub total_queries: usize,
    pub unique_patterns: usize,
    pub top_tables: Vec<(String, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::ast::{ColumnRef, SelectItem, TableName};

    fn make_select(table: &str, where_clause: Option<Expression>) -> Statement {
        Statement::Select(Box::new(SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Table {
                name: TableName::new(table),
                alias: None,
            }),
            where_clause,
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

    fn make_eq_filter(column: &str) -> Expression {
        Expression::Binary {
            left: Box::new(Expression::Column(ColumnRef {
                table: None,
                name: column.to_string(),
            })),
            operator: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(
                crate::protocols::postgres_wire::sql::types::SqlValue::Integer(1),
            )),
        }
    }

    #[test]
    fn test_workload_analyzer() {
        let mut analyzer = WorkloadAnalyzer::new();

        let stmt = make_select("users", Some(make_eq_filter("id")));
        analyzer.analyze_query(&stmt, 10.0);
        analyzer.analyze_query(&stmt, 12.0);
        analyzer.analyze_query(&stmt, 8.0);

        let patterns = analyzer.get_patterns("users");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].count, 3);
        assert!((patterns[0].avg_execution_time_ms - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_filter_column_extraction() {
        let analyzer = WorkloadAnalyzer::new();

        // Simple equality
        let eq_filter = make_eq_filter("user_id");
        let columns = analyzer.extract_filter_columns(&eq_filter);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].0, "user_id");
        assert_eq!(columns[0].1, FilterOperator::Equality);

        // AND condition
        let and_filter = Expression::Binary {
            left: Box::new(make_eq_filter("a")),
            operator: BinaryOperator::And,
            right: Box::new(make_eq_filter("b")),
        };
        let columns = analyzer.extract_filter_columns(&and_filter);
        assert_eq!(columns.len(), 2);
    }

    #[tokio::test]
    async fn test_index_advisor() {
        let advisor = IndexAdvisor::new(IndexAdvisorConfig {
            min_query_count: 2,
            ..Default::default()
        });

        // Record some queries
        let stmt = make_select("users", Some(make_eq_filter("email")));
        for _ in 0..5 {
            advisor.record_query(&stmt, 50.0).await;
        }

        let recommendations = advisor.recommend("users", None).await;
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].columns.contains(&"email".to_string()));
    }

    #[test]
    fn test_index_recommendation_to_sql() {
        let rec = IndexRecommendation {
            table_name: "users".to_string(),
            columns: vec!["email".to_string()],
            index_type: IndexType::BTree,
            improvement_ratio: 5.0,
            storage_cost: 10000,
            write_overhead: 1.1,
            benefiting_queries: 100,
            reason: "Test".to_string(),
            priority: 100,
            partial_condition: None,
            is_covering: false,
            include_columns: Vec::new(),
        };

        let sql = rec.to_sql();
        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("users"));
        assert!(sql.contains("email"));
        assert!(sql.contains("btree"));
    }

    #[test]
    fn test_covering_index_to_sql() {
        let rec = IndexRecommendation {
            table_name: "orders".to_string(),
            columns: vec!["user_id".to_string()],
            index_type: IndexType::BTree,
            improvement_ratio: 3.0,
            storage_cost: 20000,
            write_overhead: 1.2,
            benefiting_queries: 50,
            reason: "Covering".to_string(),
            priority: 80,
            partial_condition: None,
            is_covering: true,
            include_columns: vec!["total".to_string(), "status".to_string()],
        };

        let sql = rec.to_sql();
        assert!(sql.contains("INCLUDE (total, status)"));
    }

    #[tokio::test]
    async fn test_redundant_index_detection() {
        let advisor = IndexAdvisor::new_default();

        // Register some indexes
        advisor
            .register_existing_index(ExistingIndex {
                name: "idx_users_email".to_string(),
                table_name: "users".to_string(),
                columns: vec!["email".to_string()],
                index_type: IndexType::BTree,
                is_unique: false,
                is_primary: false,
            })
            .await;

        advisor
            .register_existing_index(ExistingIndex {
                name: "idx_users_email_name".to_string(),
                table_name: "users".to_string(),
                columns: vec!["email".to_string(), "name".to_string()],
                index_type: IndexType::BTree,
                is_unique: false,
                is_primary: false,
            })
            .await;

        let redundant = advisor.detect_redundant_indexes("users").await;
        assert_eq!(redundant.len(), 1);
        assert_eq!(redundant[0].redundant_index, "idx_users_email");
    }
}
