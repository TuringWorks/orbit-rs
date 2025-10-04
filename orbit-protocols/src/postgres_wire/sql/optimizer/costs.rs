//! Cost-Based Query Optimization
//!
//! This module implements cost-based optimization that uses statistics
//! to estimate the cost of different query execution plans and choose
//! the most efficient one.

use crate::postgres_wire::sql::ast::*;
use crate::error::ProtocolResult;
use super::{OptimizerConfig, stats::StatisticsCollector};

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
    pub fn new(cpu_cost: f64, io_cost: f64, memory_cost: f64, network_cost: f64, cardinality: usize) -> Self {
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

        self.cpu_cost * CPU_WEIGHT +
        self.io_cost * IO_WEIGHT +
        self.memory_cost * MEMORY_WEIGHT +
        self.network_cost * NETWORK_WEIGHT
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
    pub fn optimize(&mut self, statement: Statement, stats: &StatisticsCollector) -> ProtocolResult<Statement> {
        match statement {
            Statement::Select(select) => {
                let optimized_select = self.optimize_select(select, stats)?;
                Ok(Statement::Select(optimized_select))
            }
            // For other statement types, return as-is for now
            other => Ok(other),
        }
    }

    /// Apply cost-based optimizations with step tracking
    pub fn optimize_with_steps(&mut self, statement: Statement, stats: &StatisticsCollector) -> ProtocolResult<(Statement, Vec<String>)> {
        let mut steps = Vec::new();

        match statement {
            Statement::Select(select) => {
                let (optimized_select, select_steps) = self.optimize_select_with_steps(select, stats)?;
                steps.extend(select_steps);
                Ok((Statement::Select(optimized_select), steps))
            }
            other => Ok((other, steps)),
        }
    }

    /// Optimize SELECT statement using cost-based techniques
    fn optimize_select(&mut self, select: SelectStatement, stats: &StatisticsCollector) -> ProtocolResult<SelectStatement> {
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
    fn optimize_select_with_steps(&mut self, select: SelectStatement, stats: &StatisticsCollector) -> ProtocolResult<(SelectStatement, Vec<String>)> {
        let mut steps = Vec::new();
        let mut optimized = select;

        // 1. Join reordering
        if self.config.enable_join_reorder {
            let before = optimized.clone();
            optimized = self.optimize_join_order(optimized, stats)?;
            if format!("{:?}", before) != format!("{:?}", optimized) {
                steps.push("Applied cost-based join reordering".to_string());
            }
        }

        // 2. Access method optimization
        let before = optimized.clone();
        optimized = self.optimize_access_methods(optimized, stats)?;
        if format!("{:?}", before) != format!("{:?}", optimized) {
            steps.push("Optimized access methods".to_string());
        }

        Ok((optimized, steps))
    }

    /// Optimize join order based on cost estimates
    fn optimize_join_order(&self, select: SelectStatement, stats: &StatisticsCollector) -> ProtocolResult<SelectStatement> {
        // For now, this is a placeholder implementation
        // In a full implementation, this would:
        // 1. Enumerate all possible join orders
        // 2. Estimate cost for each order
        // 3. Choose the lowest-cost order
        // 4. Consider dynamic programming for larger numbers of tables
        Ok(select)
    }

    /// Optimize access methods (table scan vs index scan)
    fn optimize_access_methods(&self, select: SelectStatement, stats: &StatisticsCollector) -> ProtocolResult<SelectStatement> {
        // Placeholder implementation
        // Would analyze WHERE conditions and choose appropriate access methods
        Ok(select)
    }

    /// Optimize join algorithms
    fn optimize_join_algorithms(&self, select: SelectStatement, stats: &StatisticsCollector) -> ProtocolResult<SelectStatement> {
        // Placeholder implementation
        // Would choose between nested loop, hash join, and merge join
        Ok(select)
    }

    /// Estimate the cost of a table scan
    pub fn estimate_table_scan_cost(&self, table_name: &str, stats: &StatisticsCollector) -> QueryCost {
        if let Some(table_stats) = stats.get_table_stats(table_name) {
            let pages = (table_stats.row_count as f64 / table_stats.rows_per_page as f64).ceil();
            QueryCost::new(
                table_stats.row_count as f64, // CPU cost proportional to rows
                pages, // I/O cost proportional to pages
                0.0, // Memory cost minimal for scan
                0.0, // No network cost for local scan
                table_stats.row_count,
            )
        } else {
            // Default estimates for unknown tables
            QueryCost::new(1000.0, 100.0, 0.0, 0.0, 1000)
        }
    }

    /// Estimate the cost of an index scan
    pub fn estimate_index_scan_cost(&self, table_name: &str, index_name: &str, selectivity: f64, stats: &StatisticsCollector) -> QueryCost {
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
    pub fn estimate_nested_loop_join_cost(&self, left_cost: &QueryCost, right_cost: &QueryCost) -> QueryCost {
        QueryCost::new(
            left_cost.cpu_cost + (left_cost.cardinality as f64 * right_cost.cpu_cost),
            left_cost.io_cost + (left_cost.cardinality as f64 * right_cost.io_cost),
            left_cost.memory_cost.max(right_cost.memory_cost),
            left_cost.network_cost + right_cost.network_cost,
            (left_cost.cardinality as f64 * right_cost.cardinality as f64 * 0.1) as usize, // Assume 10% join selectivity
        )
    }

    /// Estimate the cost of a hash join
    pub fn estimate_hash_join_cost(&self, left_cost: &QueryCost, right_cost: &QueryCost) -> QueryCost {
        // Hash join: build hash table from smaller relation, probe with larger
        let (build_cost, probe_cost) = if left_cost.cardinality <= right_cost.cardinality {
            (left_cost, right_cost)
        } else {
            (right_cost, left_cost)
        };

        QueryCost::new(
            build_cost.cpu_cost + probe_cost.cpu_cost + (build_cost.cardinality + probe_cost.cardinality) as f64,
            build_cost.io_cost + probe_cost.io_cost,
            build_cost.cardinality as f64 * 8.0, // Memory for hash table (8 bytes per row estimate)
            build_cost.network_cost + probe_cost.network_cost,
            (build_cost.cardinality as f64 * probe_cost.cardinality as f64 * 0.1) as usize,
        )
    }

    /// Estimate the cost of a merge join
    pub fn estimate_merge_join_cost(&self, left_cost: &QueryCost, right_cost: &QueryCost) -> QueryCost {
        // Assume inputs are already sorted, or add sort cost if needed
        QueryCost::new(
            left_cost.cpu_cost + right_cost.cpu_cost + (left_cost.cardinality + right_cost.cardinality) as f64,
            left_cost.io_cost + right_cost.io_cost,
            (left_cost.cardinality.max(right_cost.cardinality)) as f64 * 4.0, // Memory for merge
            left_cost.network_cost + right_cost.network_cost,
            (left_cost.cardinality as f64 * right_cost.cardinality as f64 * 0.1) as usize,
        )
    }

    /// Estimate selectivity of a predicate
    pub fn estimate_selectivity(&self, expr: &Expression, stats: &StatisticsCollector) -> f64 {
        match expr {
            Expression::Binary { left: _, operator, right: _ } => {
                match operator {
                    BinaryOperator::Equal => 0.1, // Equality typically has good selectivity
                    BinaryOperator::NotEqual => 0.9,
                    BinaryOperator::LessThan | BinaryOperator::LessThanOrEqual |
                    BinaryOperator::GreaterThan | BinaryOperator::GreaterThanOrEqual => 0.33,
                    BinaryOperator::And => 0.5, // Conservative estimate
                    BinaryOperator::Or => 0.75,
                    _ => 0.5, // Default
                }
            }
            Expression::Unary { operator: UnaryOperator::Not, .. } => 0.9, // NOT is usually high selectivity
            _ => 0.5, // Default selectivity
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres_wire::sql::optimizer::stats::{StatisticsCollector, TableStatistics};

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
        stats.update_table_stats("test_table", TableStatistics {
            row_count: 10000,
            rows_per_page: 100,
            average_row_size: 50,
            null_frac: 0.1,
            distinct_values: 1000,
        });
        
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
            right: Box::new(Expression::Literal(crate::postgres_wire::sql::types::SqlValue::Integer(42))),
        };
        
        let selectivity = optimizer.estimate_selectivity(&expr, &stats);
        assert_eq!(selectivity, 0.1); // Expected selectivity for equality
    }
}