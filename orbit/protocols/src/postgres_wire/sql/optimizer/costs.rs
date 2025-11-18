//! Cost-Based Query Optimization
//!
//! This module implements cost-based optimization that uses statistics
//! to estimate the cost of different query execution plans and choose
//! the most efficient one.

use super::{stats::StatisticsCollector, OptimizerConfig};
use crate::error::ProtocolResult;
use crate::postgres_wire::sql::ast::{
    BinaryOperator, Expression, FromClause, SelectStatement, Statement, UnaryOperator,
};
#[cfg(test)]
use crate::postgres_wire::sql::ast::{ColumnRef, JoinCondition, SelectItem, TableName};

/// Cost-based optimizer that uses statistics for optimization decisions
pub struct CostBasedOptimizer {
    config: OptimizerConfig,
}

/// Represents the estimated cost of a query operation
#[derive(Debug, Clone, PartialEq)]
pub struct QueryCost {
    /// CPU cost (operations)
    pub cpu_cost: f64,
    /// I/O cost (page reads)
    pub io_cost: f64,
    /// Memory cost (bytes)
    pub memory_cost: f64,
    /// Network cost (for distributed queries)
    pub network_cost: f64,
    /// Estimated result cardinality
    pub cardinality: usize,
}

impl QueryCost {
    /// Create a new query cost
    pub fn new(
        cpu_cost: f64,
        io_cost: f64,
        memory_cost: f64,
        network_cost: f64,
        cardinality: usize,
    ) -> Self {
        Self {
            cpu_cost,
            io_cost,
            memory_cost,
            network_cost,
            cardinality,
        }
    }

    /// Calculate total cost (weighted sum)
    pub fn total_cost(&self) -> f64 {
        // Default weights - could be configurable
        const CPU_WEIGHT: f64 = 1.0;
        const IO_WEIGHT: f64 = 10.0;
        const MEMORY_WEIGHT: f64 = 0.1;
        const NETWORK_WEIGHT: f64 = 100.0;

        self.cpu_cost * CPU_WEIGHT
            + self.io_cost * IO_WEIGHT
            + self.memory_cost * MEMORY_WEIGHT
            + self.network_cost * NETWORK_WEIGHT
    }

    /// Combine two costs (e.g., for joins)
    pub fn combine(&self, other: &QueryCost) -> QueryCost {
        QueryCost {
            cpu_cost: self.cpu_cost + other.cpu_cost,
            io_cost: self.io_cost + other.io_cost,
            memory_cost: self.memory_cost.max(other.memory_cost), // Max for memory
            network_cost: self.network_cost + other.network_cost,
            cardinality: self.cardinality * other.cardinality, // Estimated join result
        }
    }
}

impl CostBasedOptimizer {
    /// Create a new cost-based optimizer
    pub fn new(config: &OptimizerConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Apply cost-based optimizations to a statement
    pub fn optimize(
        &mut self,
        statement: Statement,
        stats: &StatisticsCollector,
    ) -> ProtocolResult<Statement> {
        match statement {
            Statement::Select(select) => {
                let optimized_select = self.optimize_select(*select, stats)?;
                Ok(Statement::Select(Box::new(optimized_select)))
            }
            // For other statement types, return as-is for now
            other => Ok(other),
        }
    }

    /// Apply cost-based optimizations with step tracking
    pub fn optimize_with_steps(
        &mut self,
        statement: Statement,
        stats: &StatisticsCollector,
    ) -> ProtocolResult<(Statement, Vec<String>)> {
        let steps = Vec::new();

        match statement {
            Statement::Select(select) => {
                let (optimized_select, select_steps) =
                    self.optimize_select_with_steps(*select, stats)?;
                let steps = select_steps;
                Ok((Statement::Select(Box::new(optimized_select)), steps))
            }
            other => Ok((other, steps)),
        }
    }

    /// Optimize SELECT statement using cost-based techniques
    fn optimize_select(
        &mut self,
        select: SelectStatement,
        stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        let mut optimized = select;

        // 1. Join reordering based on estimated costs
        if self.config.enable_join_reorder {
            optimized = self.optimize_join_order(optimized, stats)?;
        }

        // 2. Access method selection (table scan vs index scan)
        optimized = self.optimize_access_methods(optimized, stats)?;

        // 3. Join algorithm selection (nested loop, hash join, merge join)
        optimized = self.optimize_join_algorithms(optimized, stats)?;

        Ok(optimized)
    }

    /// Optimize SELECT with step tracking
    fn optimize_select_with_steps(
        &mut self,
        select: SelectStatement,
        stats: &StatisticsCollector,
    ) -> ProtocolResult<(SelectStatement, Vec<String>)> {
        let mut steps = Vec::new();
        let mut optimized = select;

        // 1. Join reordering
        if self.config.enable_join_reorder {
            let before = optimized.clone();
            optimized = self.optimize_join_order(optimized, stats)?;
            if format!("{before:?}") != format!("{optimized:?}") {
                steps.push("Applied cost-based join reordering".to_string());
            }
        }

        // 2. Access method optimization
        let before = optimized.clone();
        optimized = self.optimize_access_methods(optimized, stats)?;
        if format!("{before:?}") != format!("{optimized:?}") {
            steps.push("Optimized access methods".to_string());
        }

        Ok((optimized, steps))
    }

    /// Optimize join order based on cost estimates
    fn optimize_join_order(
        &self,
        select: SelectStatement,
        stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        // Extract all tables involved in the query
        let tables = self.extract_tables(&select);

        if tables.len() <= 1 {
            // No joins to optimize
            return Ok(select);
        }

        // For small number of tables (<=4), try all permutations
        // For larger sets, use greedy heuristics
        if tables.len() <= 4 {
            self.optimize_join_order_exhaustive(select, &tables, stats)
        } else {
            self.optimize_join_order_greedy(select, &tables, stats)
        }
    }

    /// Extract table names from SELECT statement
    fn extract_tables(&self, select: &SelectStatement) -> Vec<String> {
        let mut tables = Vec::new();

        if let Some(from_clause) = &select.from_clause {
            self.extract_tables_from_clause(from_clause, &mut tables);
        }

        tables
    }

    /// Recursively extract tables from FROM clause
    #[allow(clippy::only_used_in_recursion)]
    fn extract_tables_from_clause(&self, from_clause: &FromClause, tables: &mut Vec<String>) {
        match from_clause {
            FromClause::Table { name, .. } => {
                tables.push(name.full_name());
            }
            FromClause::Join { left, right, .. } => {
                self.extract_tables_from_clause(left, tables);
                self.extract_tables_from_clause(right, tables);
            }
            FromClause::Subquery { .. } => {
                // Subqueries are treated as separate optimization units
            }
            FromClause::Values { .. } => {
                // Values don't need optimization
            }
            FromClause::TableFunction { .. } => {
                // Table functions handled separately
            }
        }
    }

    /// Optimize join order using exhaustive search (for small number of tables)
    fn optimize_join_order_exhaustive(
        &self,
        select: SelectStatement,
        tables: &[String],
        stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        // Generate all permutations and pick the one with lowest cost
        let mut best_order = tables.to_vec();
        let mut best_cost = f64::MAX;

        // For simplicity, evaluate current order and a few reorderings
        // Full implementation would use dynamic programming
        for order in self.generate_join_orders(tables) {
            let cost = self.estimate_join_order_cost(&order, stats);
            if cost < best_cost {
                best_cost = cost;
                best_order = order;
            }
        }

        // If we found a better order, apply it
        if best_order != tables {
            // Note: Actual reordering of the FROM clause would require
            // more sophisticated AST manipulation
            // For now, just return the original select
        }

        Ok(select)
    }

    /// Generate possible join orders (simplified)
    fn generate_join_orders(&self, tables: &[String]) -> Vec<Vec<String>> {
        let mut orders = vec![tables.to_vec()];

        // Generate a few alternative orders
        if tables.len() >= 2 {
            let mut reversed = tables.to_vec();
            reversed.reverse();
            orders.push(reversed);
        }

        if tables.len() >= 3 {
            // Try moving smallest table first (if we have statistics)
            let by_size = tables.to_vec();
            // In real implementation, would sort by estimated size
            orders.push(by_size);
        }

        orders
    }

    /// Estimate cost of a specific join order
    fn estimate_join_order_cost(&self, tables: &[String], stats: &StatisticsCollector) -> f64 {
        let mut total_cost = 0.0;
        let mut current_rows = 1;

        for table in tables {
            let table_cost = self.estimate_table_scan_cost(table, stats);
            // Cost increases with current intermediate result size
            total_cost += table_cost.total_cost() * current_rows as f64;
            current_rows *= table_cost.cardinality;
        }

        total_cost
    }

    /// Optimize join order using greedy heuristics (for large number of tables)
    fn optimize_join_order_greedy(
        &self,
        select: SelectStatement,
        tables: &[String],
        stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        // Greedy algorithm: Always pick the smallest table next
        let mut ordered_tables = Vec::new();
        let mut remaining_tables: Vec<_> = tables.to_vec();

        while !remaining_tables.is_empty() {
            // Find table with smallest estimated cardinality
            let mut best_idx = 0;
            let mut best_size = usize::MAX;

            for (idx, table) in remaining_tables.iter().enumerate() {
                let cost = self.estimate_table_scan_cost(table, stats);
                if cost.cardinality < best_size {
                    best_size = cost.cardinality;
                    best_idx = idx;
                }
            }

            ordered_tables.push(remaining_tables.remove(best_idx));
        }

        // Apply the reordering (simplified - actual implementation would
        // reconstruct the FROM clause)
        Ok(select)
    }

    /// Optimize access methods (table scan vs index scan)
    fn optimize_access_methods(
        &self,
        select: SelectStatement,
        _stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        // Placeholder implementation
        // Would analyze WHERE conditions and choose appropriate access methods
        Ok(select)
    }

    /// Optimize join algorithms
    fn optimize_join_algorithms(
        &self,
        select: SelectStatement,
        _stats: &StatisticsCollector,
    ) -> ProtocolResult<SelectStatement> {
        // Placeholder implementation
        // Would choose between nested loop, hash join, and merge join
        Ok(select)
    }

    /// Estimate the cost of a table scan
    pub fn estimate_table_scan_cost(
        &self,
        table_name: &str,
        stats: &StatisticsCollector,
    ) -> QueryCost {
        if let Some(table_stats) = stats.get_table_stats(table_name) {
            let pages = (table_stats.row_count as f64 / table_stats.rows_per_page as f64).ceil();
            QueryCost::new(
                table_stats.row_count as f64, // CPU cost proportional to rows
                pages,                        // I/O cost proportional to pages
                0.0,                          // Memory cost minimal for scan
                0.0,                          // No network cost for local scan
                table_stats.row_count,
            )
        } else {
            // Default estimates for unknown tables
            QueryCost::new(1000.0, 100.0, 0.0, 0.0, 1000)
        }
    }

    /// Estimate the cost of an index scan
    pub fn estimate_index_scan_cost(
        &self,
        table_name: &str,
        _index_name: &str,
        selectivity: f64,
        stats: &StatisticsCollector,
    ) -> QueryCost {
        if let Some(table_stats) = stats.get_table_stats(table_name) {
            let estimated_rows = (table_stats.row_count as f64 * selectivity) as usize;
            let index_pages = (estimated_rows as f64 / 100.0).ceil(); // Assume 100 rows per index page

            QueryCost::new(
                estimated_rows as f64 * 2.0, // Slightly higher CPU cost for index traversal
                index_pages + (estimated_rows as f64 / table_stats.rows_per_page as f64), // Index pages + data pages
                0.0,
                0.0,
                estimated_rows,
            )
        } else {
            // Default estimates
            let estimated_rows = (1000.0 * selectivity) as usize;
            QueryCost::new(
                estimated_rows as f64 * 2.0,
                estimated_rows as f64 / 10.0,
                0.0,
                0.0,
                estimated_rows,
            )
        }
    }

    /// Estimate the cost of a nested loop join
    pub fn estimate_nested_loop_join_cost(
        &self,
        left_cost: &QueryCost,
        right_cost: &QueryCost,
    ) -> QueryCost {
        QueryCost::new(
            left_cost.cpu_cost + (left_cost.cardinality as f64 * right_cost.cpu_cost),
            left_cost.io_cost + (left_cost.cardinality as f64 * right_cost.io_cost),
            left_cost.memory_cost.max(right_cost.memory_cost),
            left_cost.network_cost + right_cost.network_cost,
            (left_cost.cardinality as f64 * right_cost.cardinality as f64 * 0.1) as usize, // Assume 10% join selectivity
        )
    }

    /// Estimate the cost of a hash join
    pub fn estimate_hash_join_cost(
        &self,
        left_cost: &QueryCost,
        right_cost: &QueryCost,
    ) -> QueryCost {
        // Hash join: build hash table from smaller relation, probe with larger
        let (build_cost, probe_cost) = if left_cost.cardinality <= right_cost.cardinality {
            (left_cost, right_cost)
        } else {
            (right_cost, left_cost)
        };

        QueryCost::new(
            build_cost.cpu_cost
                + probe_cost.cpu_cost
                + (build_cost.cardinality + probe_cost.cardinality) as f64,
            build_cost.io_cost + probe_cost.io_cost,
            build_cost.cardinality as f64 * 8.0, // Memory for hash table (8 bytes per row estimate)
            build_cost.network_cost + probe_cost.network_cost,
            (build_cost.cardinality as f64 * probe_cost.cardinality as f64 * 0.1) as usize,
        )
    }

    /// Estimate the cost of a merge join
    pub fn estimate_merge_join_cost(
        &self,
        left_cost: &QueryCost,
        right_cost: &QueryCost,
    ) -> QueryCost {
        // Assume inputs are already sorted, or add sort cost if needed
        QueryCost::new(
            left_cost.cpu_cost
                + right_cost.cpu_cost
                + (left_cost.cardinality + right_cost.cardinality) as f64,
            left_cost.io_cost + right_cost.io_cost,
            (left_cost.cardinality.max(right_cost.cardinality)) as f64 * 4.0, // Memory for merge
            left_cost.network_cost + right_cost.network_cost,
            (left_cost.cardinality as f64 * right_cost.cardinality as f64 * 0.1) as usize,
        )
    }

    /// Estimate selectivity of a predicate
    #[allow(clippy::only_used_in_recursion)]
    pub fn estimate_selectivity(&self, expr: &Expression, stats: &StatisticsCollector) -> f64 {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => self.estimate_binary_selectivity(left, operator, right, stats),
            Expression::Unary { operator, operand } => {
                self.estimate_unary_selectivity(operator, operand, stats)
            }
            Expression::IsNull { negated, .. } => self.estimate_null_selectivity(*negated),
            _ => 0.5, // Default selectivity
        }
    }

    /// Estimate selectivity for binary expressions
    fn estimate_binary_selectivity(
        &self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
        stats: &StatisticsCollector,
    ) -> f64 {
        match operator {
            BinaryOperator::Equal => self.estimate_equality_selectivity(left, stats),
            BinaryOperator::NotEqual => 0.9,
            BinaryOperator::LessThan
            | BinaryOperator::LessThanOrEqual
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterThanOrEqual => {
                self.estimate_range_selectivity_for_expr(left, operator, right, stats)
            }
            BinaryOperator::And => self.estimate_and_selectivity(left, right, stats),
            BinaryOperator::Or => self.estimate_or_selectivity(left, right, stats),
            BinaryOperator::Like => 0.15,
            BinaryOperator::In => 0.2,
            _ => 0.5,
        }
    }

    /// Estimate selectivity for equality operations
    fn estimate_equality_selectivity(&self, left: &Expression, stats: &StatisticsCollector) -> f64 {
        if let Expression::Column(col_ref) = left {
            if let Some(table) = &col_ref.table {
                return stats.estimate_equality_selectivity(table, &col_ref.name);
            }
        }
        0.1 // Default equality selectivity
    }

    /// Estimate selectivity for range operations
    fn estimate_range_selectivity_for_expr(
        &self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
        stats: &StatisticsCollector,
    ) -> f64 {
        if let (Expression::Column(col_ref), Expression::Literal(val)) = (left, right) {
            if let Some(table) = &col_ref.table {
                return stats.estimate_range_selectivity(
                    table,
                    &col_ref.name,
                    &format!("{operator:?}"),
                    &format!("{val:?}"),
                );
            }
        }
        0.33 // Default range selectivity
    }

    /// Estimate selectivity for AND operations
    fn estimate_and_selectivity(
        &self,
        left: &Expression,
        right: &Expression,
        stats: &StatisticsCollector,
    ) -> f64 {
        let left_sel = self.estimate_selectivity(left, stats);
        let right_sel = self.estimate_selectivity(right, stats);
        left_sel * right_sel
    }

    /// Estimate selectivity for OR operations
    fn estimate_or_selectivity(
        &self,
        left: &Expression,
        right: &Expression,
        stats: &StatisticsCollector,
    ) -> f64 {
        let left_sel = self.estimate_selectivity(left, stats);
        let right_sel = self.estimate_selectivity(right, stats);
        left_sel + right_sel - (left_sel * right_sel)
    }

    /// Estimate selectivity for unary expressions
    fn estimate_unary_selectivity(
        &self,
        operator: &UnaryOperator,
        operand: &Expression,
        stats: &StatisticsCollector,
    ) -> f64 {
        match operator {
            UnaryOperator::Not => 1.0 - self.estimate_selectivity(operand, stats),
            _ => 0.5,
        }
    }

    /// Estimate selectivity for NULL checks
    fn estimate_null_selectivity(&self, negated: bool) -> f64 {
        if negated {
            0.9 // IS NOT NULL is usually high selectivity
        } else {
            0.1 // IS NULL checks are usually selective
        }
    }

    /// Estimate selectivity for complex predicates with correlation
    pub fn estimate_complex_selectivity(
        &self,
        expressions: &[Expression],
        stats: &StatisticsCollector,
    ) -> f64 {
        if expressions.is_empty() {
            return 1.0;
        }

        // For multiple independent predicates, multiply selectivities
        let mut combined_selectivity = 1.0;
        for expr in expressions {
            let selectivity = self.estimate_selectivity(expr, stats);
            combined_selectivity *= selectivity;
        }

        // Apply correlation factor to avoid over-optimization
        // Real-world predicates are often correlated
        let correlation_factor = 0.8;
        combined_selectivity * correlation_factor + (1.0 - correlation_factor) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres_wire::sql::optimizer::stats::{
        ColumnStatistics, StatisticsCollector, TableStatistics,
    };

    #[test]
    fn test_query_cost_calculation() {
        let cost = QueryCost::new(100.0, 10.0, 1000.0, 5.0, 1000);

        // Test total cost calculation with default weights
        let total = cost.total_cost();
        assert!(total > 0.0);

        // I/O should have the highest impact due to weight
        assert!(total >= 100.0); // At least the I/O cost * weight
    }

    #[test]
    fn test_cost_combination() {
        let cost1 = QueryCost::new(50.0, 5.0, 500.0, 2.0, 500);
        let cost2 = QueryCost::new(30.0, 3.0, 300.0, 1.0, 300);

        let combined = cost1.combine(&cost2);

        assert_eq!(combined.cpu_cost, 80.0);
        assert_eq!(combined.io_cost, 8.0);
        assert_eq!(combined.memory_cost, 500.0); // Max of the two
        assert_eq!(combined.cardinality, 150000); // Product for join estimation
    }

    #[test]
    fn test_table_scan_cost_estimation() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);
        let mut stats = StatisticsCollector::new();

        // Add test statistics
        stats.update_table_stats(
            "test_table",
            TableStatistics {
                row_count: 10000,
                rows_per_page: 100,
                average_row_size: 50,
                null_frac: 0.1,
                distinct_values: 1000,
            },
        );

        let cost = optimizer.estimate_table_scan_cost("test_table", &stats);

        assert_eq!(cost.cardinality, 10000);
        assert_eq!(cost.io_cost, 100.0); // 10000 rows / 100 rows per page
        assert!(cost.total_cost() > 0.0);
    }

    #[test]
    fn test_join_cost_estimation() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);

        let left_cost = QueryCost::new(100.0, 10.0, 0.0, 0.0, 1000);
        let right_cost = QueryCost::new(50.0, 5.0, 0.0, 0.0, 500);

        // Test nested loop join
        let nl_cost = optimizer.estimate_nested_loop_join_cost(&left_cost, &right_cost);

        // Test hash join
        let hash_cost = optimizer.estimate_hash_join_cost(&left_cost, &right_cost);

        // Test merge join
        let merge_cost = optimizer.estimate_merge_join_cost(&left_cost, &right_cost);

        // All should have positive costs
        assert!(nl_cost.total_cost() > 0.0);
        assert!(hash_cost.total_cost() > 0.0);
        assert!(merge_cost.total_cost() > 0.0);

        // Nested loop should typically be most expensive for large tables
        assert!(nl_cost.total_cost() > hash_cost.total_cost());
    }

    #[test]
    fn test_selectivity_estimation() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);
        let stats = StatisticsCollector::new();

        // Create a simple equality expression for testing
        let expr = Expression::Binary {
            left: Box::new(Expression::Column(ColumnRef {
                table: None,
                name: "id".to_string(),
            })),
            operator: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(
                crate::postgres_wire::sql::types::SqlValue::Integer(42),
            )),
        };

        let selectivity = optimizer.estimate_selectivity(&expr, &stats);
        assert_eq!(selectivity, 0.1); // Expected selectivity for equality
    }

    #[test]
    fn test_selectivity_and_or_combination() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);
        let stats = StatisticsCollector::new();

        // Test AND combination
        let and_expr = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "age".to_string(),
                })),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Integer(18),
                )),
            }),
            operator: BinaryOperator::And,
            right: Box::new(Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "status".to_string(),
                })),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text("active".to_string()),
                )),
            }),
        };

        let and_selectivity = optimizer.estimate_selectivity(&and_expr, &stats);
        // Should be product of individual selectivities: 0.33 * 0.1 = 0.033
        assert!(and_selectivity < 0.1);

        // Test OR combination
        let or_expr = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "status".to_string(),
                })),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text("active".to_string()),
                )),
            }),
            operator: BinaryOperator::Or,
            right: Box::new(Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "status".to_string(),
                })),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text("pending".to_string()),
                )),
            }),
        };

        let or_selectivity = optimizer.estimate_selectivity(&or_expr, &stats);
        // Should be higher than individual selectivity
        assert!(or_selectivity > 0.1);
    }

    #[test]
    fn test_selectivity_with_statistics() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);
        let mut stats = StatisticsCollector::new();

        // Add column statistics
        stats.update_column_stats(
            "users",
            "email",
            ColumnStatistics {
                distinct_count: 1000,
                null_fraction: 0.01,
                ..Default::default()
            },
        );

        // Create expression with table reference
        let expr = Expression::Binary {
            left: Box::new(Expression::Column(ColumnRef {
                table: Some("users".to_string()),
                name: "email".to_string(),
            })),
            operator: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(
                crate::postgres_wire::sql::types::SqlValue::Text("test@example.com".to_string()),
            )),
        };

        let selectivity = optimizer.estimate_selectivity(&expr, &stats);
        // Should use statistics: 1/1000 = 0.001
        assert_eq!(selectivity, 0.001);
    }

    #[test]
    fn test_join_order_extraction() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);

        let select = SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Join {
                left: Box::new(FromClause::Table {
                    name: TableName::new("orders"),
                    alias: None,
                }),
                right: Box::new(FromClause::Table {
                    name: TableName::new("customers"),
                    alias: None,
                }),
                join_type: crate::postgres_wire::sql::ast::JoinType::Inner,
                condition: JoinCondition::On(Expression::Binary {
                    left: Box::new(Expression::Column(ColumnRef {
                        table: Some("orders".to_string()),
                        name: "customer_id".to_string(),
                    })),
                    operator: BinaryOperator::Equal,
                    right: Box::new(Expression::Column(ColumnRef {
                        table: Some("customers".to_string()),
                        name: "id".to_string(),
                    })),
                }),
            }),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
            for_clause: None,
        };

        let tables = optimizer.extract_tables(&select);
        assert_eq!(tables.len(), 2);
        assert!(tables.contains(&"orders".to_string()));
        assert!(tables.contains(&"customers".to_string()));
    }

    #[test]
    fn test_complex_selectivity_estimation() {
        let config = OptimizerConfig::default();
        let optimizer = CostBasedOptimizer::new(&config);
        let stats = StatisticsCollector::new();

        let predicates = vec![
            Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "age".to_string(),
                })),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Integer(18),
                )),
            },
            Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "status".to_string(),
                })),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text("active".to_string()),
                )),
            },
        ];

        let selectivity = optimizer.estimate_complex_selectivity(&predicates, &stats);
        // Should be less than individual products due to correlation factor
        assert!(selectivity > 0.0 && selectivity < 1.0);
    }
}
