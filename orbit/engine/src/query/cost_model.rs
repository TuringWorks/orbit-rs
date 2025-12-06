//! Cost-based query optimization
//!
//! Provides cost estimation for different query execution strategies
//! to enable intelligent query plan selection.

use super::statistics::StatisticsCollector;
use super::{PlanNode, PlanNodeType};
use serde::{Deserialize, Serialize};

/// Cost model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    /// Cost per row for sequential scan
    pub seq_scan_cost_per_row: f64,
    /// Cost per row for index scan
    pub index_scan_cost_per_row: f64,
    /// Cost per row for filter operation
    pub filter_cost_per_row: f64,
    /// Cost per row for projection
    pub projection_cost_per_row: f64,
    /// Cost per row for sort
    pub sort_cost_per_row: f64,
    /// Cost per row for hash join
    pub hash_join_cost_per_row: f64,
    /// Cost per row for nested loop join
    pub nested_loop_join_cost_per_row: f64,
    /// Cost for random I/O operation
    pub random_io_cost: f64,
    /// Cost for sequential I/O operation
    pub sequential_io_cost: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            seq_scan_cost_per_row: 1.0,
            index_scan_cost_per_row: 0.5,
            filter_cost_per_row: 0.1,
            projection_cost_per_row: 0.05,
            sort_cost_per_row: 2.0,
            hash_join_cost_per_row: 1.5,
            nested_loop_join_cost_per_row: 3.0,
            random_io_cost: 10.0,
            sequential_io_cost: 1.0,
        }
    }
}

/// Cost-based optimizer
pub struct CostBasedOptimizer {
    /// Cost model configuration
    cost_model: CostModel,
    /// Statistics collector
    stats_collector: StatisticsCollector,
}

impl CostBasedOptimizer {
    /// Create a new cost-based optimizer
    pub fn new() -> Self {
        Self {
            cost_model: CostModel::default(),
            stats_collector: StatisticsCollector::new(),
        }
    }

    /// Create optimizer with custom cost model
    pub fn with_cost_model(cost_model: CostModel) -> Self {
        Self {
            cost_model,
            stats_collector: StatisticsCollector::new(),
        }
    }

    /// Get mutable reference to statistics collector
    pub fn stats_collector_mut(&mut self) -> &mut StatisticsCollector {
        &mut self.stats_collector
    }

    /// Get reference to statistics collector
    pub fn stats_collector(&self) -> &StatisticsCollector {
        &self.stats_collector
    }

    /// Estimate cost for a plan node
    pub fn estimate_node_cost(&self, node: &PlanNode, table_name: &str) -> f64 {
        match node.node_type {
            PlanNodeType::TableScan => self.estimate_table_scan_cost(table_name),
            PlanNodeType::IndexScan => self.estimate_index_scan_cost(table_name),
            PlanNodeType::Filter => {
                self.cost_model.filter_cost_per_row * node.estimated_rows as f64
            }
            PlanNodeType::Projection => {
                self.cost_model.projection_cost_per_row * node.estimated_rows as f64
            }
            PlanNodeType::Sort => {
                // Sort cost is O(n log n)
                let n = node.estimated_rows as f64;
                self.cost_model.sort_cost_per_row * n * n.log2()
            }
            PlanNodeType::Join => self.estimate_join_cost(node),
            PlanNodeType::Aggregation | PlanNodeType::GroupBy => {
                // Aggregation typically requires a hash table
                self.cost_model.hash_join_cost_per_row * node.estimated_rows as f64
            }
        }
    }

    /// Estimate cost for table scan
    fn estimate_table_scan_cost(&self, table_name: &str) -> f64 {
        let row_count = self.stats_collector.estimate_row_count(table_name);
        let io_cost = self.cost_model.sequential_io_cost;
        let cpu_cost = self.cost_model.seq_scan_cost_per_row * row_count as f64;
        io_cost + cpu_cost
    }

    /// Estimate cost for index scan
    fn estimate_index_scan_cost(&self, table_name: &str) -> f64 {
        let row_count = self.stats_collector.estimate_row_count(table_name);
        // Index scan has random I/O for each row
        let io_cost = self.cost_model.random_io_cost * row_count as f64;
        let cpu_cost = self.cost_model.index_scan_cost_per_row * row_count as f64;
        io_cost + cpu_cost
    }

    /// Estimate cost for join operation
    fn estimate_join_cost(&self, node: &PlanNode) -> f64 {
        if node.children.len() < 2 {
            return 0.0;
        }

        let left_rows = node.children[0].estimated_rows as f64;
        let right_rows = node.children[1].estimated_rows as f64;

        // Use hash join cost model
        // Build hash table on smaller relation, probe with larger
        let build_cost = left_rows.min(right_rows) * self.cost_model.hash_join_cost_per_row;
        let probe_cost = left_rows.max(right_rows) * self.cost_model.filter_cost_per_row;

        build_cost + probe_cost
    }

    /// Estimate total plan cost
    pub fn estimate_plan_cost(&self, nodes: &[PlanNode], table_name: &str) -> f64 {
        nodes
            .iter()
            .map(|node| {
                let node_cost = self.estimate_node_cost(node, table_name);
                let children_cost: f64 = node
                    .children
                    .iter()
                    .map(|child| self.estimate_node_cost(child, table_name))
                    .sum();
                node_cost + children_cost
            })
            .sum()
    }

    /// Choose between table scan and index scan
    pub fn choose_scan_method(&self, table_name: &str, has_index: bool) -> PlanNodeType {
        if !has_index {
            return PlanNodeType::TableScan;
        }

        let table_scan_cost = self.estimate_table_scan_cost(table_name);
        let index_scan_cost = self.estimate_index_scan_cost(table_name);

        if index_scan_cost < table_scan_cost {
            PlanNodeType::IndexScan
        } else {
            PlanNodeType::TableScan
        }
    }

    /// Estimate selectivity for a predicate
    pub fn estimate_selectivity(&self, table_name: &str, column_name: &str) -> f64 {
        self.stats_collector
            .estimate_selectivity(table_name, column_name)
    }
}

impl Default for CostBasedOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::statistics::TableStatistics;
    use std::collections::HashMap;

    #[test]
    fn test_cost_estimation() {
        let mut optimizer = CostBasedOptimizer::new();

        // Add table statistics
        let table_stats = TableStatistics {
            table_name: "users".to_string(),
            row_count: 10000,
            avg_row_size: 256,
            table_size: 2560000,
            column_stats: HashMap::new(),
            last_updated: 0,
        };

        optimizer
            .stats_collector_mut()
            .update_table_stats(table_stats);

        // Table scan should be cheaper for small tables
        let table_scan_cost = optimizer.estimate_table_scan_cost("users");
        assert!(table_scan_cost > 0.0);
    }

    #[test]
    fn test_scan_method_selection() {
        let mut optimizer = CostBasedOptimizer::new();

        let table_stats = TableStatistics {
            table_name: "users".to_string(),
            row_count: 1000,
            avg_row_size: 256,
            table_size: 256000,
            column_stats: HashMap::new(),
            last_updated: 0,
        };

        optimizer
            .stats_collector_mut()
            .update_table_stats(table_stats);

        // For small tables, table scan is usually preferred
        let scan_method = optimizer.choose_scan_method("users", true);
        assert!(matches!(
            scan_method,
            PlanNodeType::TableScan | PlanNodeType::IndexScan
        ));
    }

    #[test]
    fn test_join_cost_estimation() {
        let optimizer = CostBasedOptimizer::new();

        let left_child = PlanNode {
            node_type: PlanNodeType::TableScan,
            estimated_rows: 1000,
            children: vec![],
        };

        let right_child = PlanNode {
            node_type: PlanNodeType::TableScan,
            estimated_rows: 100,
            children: vec![],
        };

        let join_node = PlanNode {
            node_type: PlanNodeType::Join,
            estimated_rows: 500,
            children: vec![left_child, right_child],
        };

        let cost = optimizer.estimate_join_cost(&join_node);
        assert!(cost > 0.0);
    }
}
