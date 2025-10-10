//! Cost model for query optimization
//!
//! This module provides comprehensive cost estimation for different types of
//! database operations including CPU, I/O, memory, and network costs.
//! Implements the cost model framework defined in Phase 9.1.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::statistics::{IndexStatistics, TableStatistics};
use crate::orbitql::ast::*;

/// Comprehensive cost breakdown for query operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryCost {
    /// CPU processing cost (in arbitrary units)
    pub cpu_cost: f64,
    /// I/O cost for disk operations
    pub io_cost: f64,
    /// Memory usage cost
    pub memory_cost: f64,
    /// Network transfer cost for distributed operations
    pub network_cost: f64,
    /// Total estimated execution time in milliseconds
    pub total_time_ms: f64,
}

impl QueryCost {
    /// Create a new empty cost
    pub fn new() -> Self {
        Self {
            cpu_cost: 0.0,
            io_cost: 0.0,
            memory_cost: 0.0,
            network_cost: 0.0,
            total_time_ms: 0.0,
        }
    }

    /// Calculate total cost across all dimensions
    pub fn total_cost(&self) -> f64 {
        self.cpu_cost + self.io_cost + self.memory_cost + self.network_cost
    }

    /// Combine two costs
    pub fn combine(&self, other: &QueryCost) -> QueryCost {
        QueryCost {
            cpu_cost: self.cpu_cost + other.cpu_cost,
            io_cost: self.io_cost + other.io_cost,
            memory_cost: self.memory_cost + other.memory_cost,
            network_cost: self.network_cost + other.network_cost,
            total_time_ms: self.total_time_ms + other.total_time_ms,
        }
    }

    /// Scale cost by a factor (for cardinality adjustments)
    pub fn scale(&self, factor: f64) -> QueryCost {
        QueryCost {
            cpu_cost: self.cpu_cost * factor,
            io_cost: self.io_cost * factor,
            memory_cost: self.memory_cost * factor,
            network_cost: self.network_cost * factor,
            total_time_ms: self.total_time_ms * factor,
        }
    }
}

impl Default for QueryCost {
    fn default() -> Self {
        Self::new()
    }
}

/// Cost model configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModelConfig {
    /// CPU cost per tuple processed
    pub cpu_tuple_cost: f64,
    /// CPU cost per operator invocation
    pub cpu_operator_cost: f64,
    /// CPU cost per index tuple processed
    pub cpu_index_tuple_cost: f64,

    /// Random page cost (disk seeks)
    pub random_page_cost: f64,
    /// Sequential page cost (sequential reads)
    pub seq_page_cost: f64,
    /// Page size in bytes
    pub page_size: u32,

    /// Memory cost per MB allocated
    pub memory_cost_per_mb: f64,
    /// Work memory size in MB
    pub work_mem_mb: u32,

    /// Network cost per byte transferred
    pub network_cost_per_byte: f64,
    /// Network latency per round trip in ms
    pub network_latency_ms: f64,

    /// Parallel worker cost multiplier
    pub parallel_tuple_cost: f64,
    /// Parallel setup cost
    pub parallel_setup_cost: f64,

    /// JIT compilation threshold
    pub jit_above_cost: f64,
    /// JIT optimization cost
    pub jit_optimize_above_cost: f64,
}

impl Default for CostModelConfig {
    fn default() -> Self {
        Self {
            // CPU costs (PostgreSQL-inspired defaults)
            cpu_tuple_cost: 0.01,
            cpu_operator_cost: 0.0025,
            cpu_index_tuple_cost: 0.005,

            // I/O costs
            random_page_cost: 4.0,
            seq_page_cost: 1.0,
            page_size: 8192, // 8KB pages

            // Memory costs
            memory_cost_per_mb: 0.1,
            work_mem_mb: 4, // 4MB default work memory

            // Network costs
            network_cost_per_byte: 0.0001,
            network_latency_ms: 1.0,

            // Parallel costs
            parallel_tuple_cost: 0.1,
            parallel_setup_cost: 1000.0,

            // JIT costs
            jit_above_cost: 100000.0,
            jit_optimize_above_cost: 500000.0,
        }
    }
}

/// Cardinality estimation for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardinalityEstimate {
    /// Estimated number of rows
    pub rows: u64,
    /// Confidence in the estimate (0.0 to 1.0)
    pub confidence: f64,
    /// Width of each row in bytes
    pub width: u32,
}

impl CardinalityEstimate {
    pub fn new(rows: u64, width: u32) -> Self {
        Self {
            rows,
            confidence: 0.5, // Default confidence
            width,
        }
    }
}

/// Cost model implementation
pub struct CostModel {
    /// Configuration parameters
    config: CostModelConfig,
    /// Cached cost calculations
    cost_cache: HashMap<String, QueryCost>,
}

impl CostModel {
    /// Create a new cost model with default configuration
    pub fn new() -> Self {
        Self::with_config(CostModelConfig::default())
    }

    /// Create a cost model with custom configuration
    pub fn with_config(config: CostModelConfig) -> Self {
        Self {
            config,
            cost_cache: HashMap::new(),
        }
    }

    /// Calculate cost for a table scan operation
    pub fn calculate_scan_cost(
        &self,
        table_stats: &TableStatistics,
        selectivity: f64,
    ) -> (QueryCost, CardinalityEstimate) {
        let pages = table_stats.page_count;
        let tuples = table_stats.row_count;
        let output_tuples = (tuples as f64 * selectivity) as u64;

        let mut cost = QueryCost::new();

        // I/O cost - sequential scan of all pages
        cost.io_cost = pages as f64 * self.config.seq_page_cost;

        // CPU cost - process each tuple
        cost.cpu_cost = tuples as f64 * self.config.cpu_tuple_cost;

        // Memory cost - minimal for sequential scan
        let memory_mb = (table_stats.avg_row_size as f64 * 1000.0) / (1024.0 * 1024.0);
        cost.memory_cost = memory_mb * self.config.memory_cost_per_mb;

        // Estimate time
        cost.total_time_ms = cost.io_cost * 10.0 + cost.cpu_cost * 0.1;

        let cardinality = CardinalityEstimate::new(output_tuples, table_stats.avg_row_size);

        (cost, cardinality)
    }

    /// Calculate cost for an index scan operation
    pub fn calculate_index_scan_cost(
        &self,
        table_stats: &TableStatistics,
        index_stats: &IndexStatistics,
        selectivity: f64,
    ) -> (QueryCost, CardinalityEstimate) {
        let index_pages = index_stats.leaf_pages;
        let output_tuples = (table_stats.row_count as f64 * selectivity) as u64;

        let mut cost = QueryCost::new();

        // I/O cost - index pages plus table pages for qualifying rows
        let index_io_cost = index_pages as f64 * self.config.seq_page_cost;
        let table_io_cost = output_tuples as f64 * self.config.random_page_cost / 100.0; // Assume 1% page hit rate
        cost.io_cost = index_io_cost + table_io_cost;

        // CPU cost - process index tuples plus table tuples
        let index_cpu = index_stats.distinct_keys as f64 * self.config.cpu_index_tuple_cost;
        let table_cpu = output_tuples as f64 * self.config.cpu_tuple_cost;
        cost.cpu_cost = index_cpu + table_cpu;

        // Memory cost
        let memory_mb =
            (table_stats.avg_row_size as f64 * output_tuples as f64) / (1024.0 * 1024.0);
        cost.memory_cost = memory_mb * self.config.memory_cost_per_mb;

        // Estimate time - index scans are generally faster
        cost.total_time_ms = cost.io_cost * 8.0 + cost.cpu_cost * 0.1;

        let cardinality = CardinalityEstimate::new(output_tuples, table_stats.avg_row_size);

        (cost, cardinality)
    }

    /// Calculate cost for a join operation
    pub fn calculate_join_cost(
        &self,
        left_card: &CardinalityEstimate,
        right_card: &CardinalityEstimate,
        join_type: &JoinType,
        selectivity: f64,
    ) -> (QueryCost, CardinalityEstimate) {
        let mut cost = QueryCost::new();

        let left_rows = left_card.rows as f64;
        let right_rows = right_card.rows as f64;
        let output_rows = (left_rows * right_rows * selectivity) as u64;

        match join_type {
            JoinType::Inner | JoinType::Left | JoinType::Right | JoinType::Full => {
                // Hash join cost model
                // Build hash table for smaller relation
                let (build_rows, probe_rows) = if left_rows < right_rows {
                    (left_rows, right_rows)
                } else {
                    (right_rows, left_rows)
                };

                // CPU cost: build + probe
                cost.cpu_cost = build_rows * self.config.cpu_tuple_cost * 2.0
                    + probe_rows * self.config.cpu_tuple_cost * 1.5;

                // Memory cost for hash table
                let hash_table_size = build_rows * (left_card.width + right_card.width) as f64;
                let memory_mb = hash_table_size / (1024.0 * 1024.0);
                cost.memory_cost = memory_mb * self.config.memory_cost_per_mb;

                // I/O cost if hash table doesn't fit in memory
                if memory_mb > self.config.work_mem_mb as f64 {
                    cost.io_cost = (build_rows + probe_rows) * self.config.seq_page_cost;
                }

                cost.total_time_ms = cost.cpu_cost * 0.2 + cost.io_cost * 10.0;
            }
            JoinType::Cross => {
                // Cartesian product - very expensive
                cost.cpu_cost = left_rows * right_rows * self.config.cpu_tuple_cost;
                cost.memory_cost = (left_rows * right_rows * 64.0) / (1024.0 * 1024.0)
                    * self.config.memory_cost_per_mb;
                cost.total_time_ms = cost.cpu_cost * 0.5;
            }
            JoinType::Graph => {
                // Graph join cost depends on graph traversal complexity
                cost.cpu_cost = left_rows * right_rows * self.config.cpu_tuple_cost * 0.1; // Assume graph reduces complexity
                cost.memory_cost = (left_rows * right_rows * 32.0) / (1024.0 * 1024.0)
                    * self.config.memory_cost_per_mb;
                cost.total_time_ms = cost.cpu_cost * 0.3 + cost.io_cost * 8.0;
            }
        }

        let output_width = left_card.width + right_card.width;
        let cardinality = CardinalityEstimate::new(output_rows, output_width);

        (cost, cardinality)
    }

    /// Calculate cost for an aggregation operation
    pub fn calculate_aggregate_cost(
        &self,
        input_card: &CardinalityEstimate,
        group_by_columns: usize,
        aggregate_functions: usize,
    ) -> (QueryCost, CardinalityEstimate) {
        let mut cost = QueryCost::new();

        let input_rows = input_card.rows as f64;
        let output_rows = if group_by_columns > 0 {
            // Estimate distinct groups (very rough heuristic)
            (input_rows / 10.0).max(1.0) as u64
        } else {
            1 // Single aggregate result
        };

        // CPU cost for processing and grouping
        cost.cpu_cost =
            input_rows * self.config.cpu_tuple_cost * (1.0 + aggregate_functions as f64 * 0.5);

        // Memory cost for hash table (if grouping)
        if group_by_columns > 0 {
            let hash_table_size = output_rows as f64 * input_card.width as f64;
            let memory_mb = hash_table_size / (1024.0 * 1024.0);
            cost.memory_cost = memory_mb * self.config.memory_cost_per_mb;

            // I/O cost if doesn't fit in memory
            if memory_mb > self.config.work_mem_mb as f64 {
                cost.io_cost = input_rows * self.config.seq_page_cost;
            }
        }

        cost.total_time_ms = cost.cpu_cost * 0.3 + cost.io_cost * 10.0;

        let output_width = if group_by_columns > 0 {
            input_card.width
        } else {
            64 // Typical aggregate result size
        };

        let cardinality = CardinalityEstimate::new(output_rows, output_width);

        (cost, cardinality)
    }

    /// Calculate cost for a sort operation
    pub fn calculate_sort_cost(
        &self,
        input_card: &CardinalityEstimate,
        sort_columns: usize,
    ) -> (QueryCost, CardinalityEstimate) {
        let mut cost = QueryCost::new();

        let input_rows = input_card.rows as f64;

        // CPU cost - O(n log n) for sorting
        cost.cpu_cost =
            input_rows * input_rows.log2() * self.config.cpu_tuple_cost * sort_columns as f64;

        // Memory cost for sort buffer
        let sort_buffer_size = input_rows * input_card.width as f64;
        let memory_mb = sort_buffer_size / (1024.0 * 1024.0);
        cost.memory_cost = memory_mb * self.config.memory_cost_per_mb;

        // I/O cost for external sort if data doesn't fit in memory
        if memory_mb > self.config.work_mem_mb as f64 {
            // External merge sort - multiple passes
            let passes = (memory_mb / self.config.work_mem_mb as f64).ceil();
            cost.io_cost = input_rows * passes * self.config.seq_page_cost;
        }

        cost.total_time_ms = cost.cpu_cost * 0.2 + cost.io_cost * 10.0;

        // Sort doesn't change cardinality or width
        let cardinality = input_card.clone();

        (cost, cardinality)
    }

    /// Calculate cost for a parallel operation
    pub fn calculate_parallel_cost(
        &self,
        base_cost: &QueryCost,
        base_card: &CardinalityEstimate,
        parallel_workers: u32,
    ) -> (QueryCost, CardinalityEstimate) {
        if parallel_workers <= 1 {
            return (base_cost.clone(), base_card.clone());
        }

        let mut parallel_cost = QueryCost::new();

        // Setup cost
        parallel_cost.cpu_cost = self.config.parallel_setup_cost;

        // Parallel processing cost - reduced by worker count but with overhead
        let efficiency = 0.8; // 80% parallel efficiency
        let speedup = parallel_workers as f64 * efficiency;

        parallel_cost.cpu_cost += base_cost.cpu_cost / speedup;
        parallel_cost.io_cost = base_cost.io_cost / speedup;
        parallel_cost.memory_cost = base_cost.memory_cost * parallel_workers as f64; // Each worker needs memory
        parallel_cost.network_cost = base_cost.network_cost; // Network cost doesn't scale

        parallel_cost.total_time_ms =
            base_cost.total_time_ms / speedup + self.config.parallel_setup_cost * 0.1;

        (parallel_cost, base_card.clone())
    }

    /// Calculate cost for network operations (distributed queries)
    pub fn calculate_network_cost(&self, data_size_bytes: u64, round_trips: u32) -> QueryCost {
        let mut cost = QueryCost::new();

        // Data transfer cost
        cost.network_cost = data_size_bytes as f64 * self.config.network_cost_per_byte;

        // Latency cost
        cost.network_cost += round_trips as f64 * self.config.network_latency_ms;

        // Network operations are primarily time-bound
        cost.total_time_ms = round_trips as f64 * self.config.network_latency_ms
            + (data_size_bytes as f64 / (100.0 * 1024.0 * 1024.0)) * 1000.0; // 100 MB/s network

        cost
    }

    /// Estimate if JIT compilation would be beneficial
    pub fn should_use_jit(&self, total_cost: &QueryCost) -> bool {
        total_cost.total_cost() > self.config.jit_above_cost
    }

    /// Get current configuration
    pub fn get_config(&self) -> &CostModelConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: CostModelConfig) {
        self.config = config;
        // Clear cache when configuration changes
        self.cost_cache.clear();
    }

    /// Clear cost cache
    pub fn clear_cache(&mut self) {
        self.cost_cache.clear();
    }
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_sample_table_stats() -> TableStatistics {
        TableStatistics {
            row_count: 100000,
            page_count: 1000,
            avg_row_size: 100,
            null_fraction: 0.1,
            distinct_values: 80000,
            most_common_values: vec![],
            histogram: vec![],
            last_analyzed: Utc::now(),
            column_statistics: std::collections::HashMap::new(),
        }
    }

    fn create_sample_index_stats() -> IndexStatistics {
        IndexStatistics {
            index_id: "test_idx".to_string(),
            selectivity: 0.01,
            clustering_factor: 0.9,
            tree_height: 3,
            leaf_pages: 100,
            distinct_keys: 80000,
            index_size: 1024 * 1024,
            last_updated: Utc::now(),
        }
    }

    #[test]
    fn test_query_cost_operations() {
        let cost1 = QueryCost {
            cpu_cost: 100.0,
            io_cost: 200.0,
            memory_cost: 50.0,
            network_cost: 25.0,
            total_time_ms: 1000.0,
        };

        let cost2 = QueryCost {
            cpu_cost: 50.0,
            io_cost: 100.0,
            memory_cost: 25.0,
            network_cost: 10.0,
            total_time_ms: 500.0,
        };

        let combined = cost1.combine(&cost2);
        assert_eq!(combined.cpu_cost, 150.0);
        assert_eq!(combined.total_cost(), 535.0);

        let scaled = cost1.scale(2.0);
        assert_eq!(scaled.cpu_cost, 200.0);
    }

    #[test]
    fn test_scan_cost_calculation() {
        let cost_model = CostModel::new();
        let table_stats = create_sample_table_stats();

        let (cost, cardinality) = cost_model.calculate_scan_cost(&table_stats, 1.0);

        assert!(cost.io_cost > 0.0);
        assert!(cost.cpu_cost > 0.0);
        assert_eq!(cardinality.rows, 100000);
    }

    #[test]
    fn test_index_scan_cost() {
        let cost_model = CostModel::new();
        let table_stats = create_sample_table_stats();
        let index_stats = create_sample_index_stats();

        let (cost, cardinality) =
            cost_model.calculate_index_scan_cost(&table_stats, &index_stats, 0.1);

        assert!(cost.total_cost() > 0.0);
        assert_eq!(cardinality.rows, 10000); // 10% of 100k rows
    }

    #[test]
    fn test_join_cost_calculation() {
        let cost_model = CostModel::new();

        let left_card = CardinalityEstimate::new(1000, 50);
        let right_card = CardinalityEstimate::new(10000, 100);

        let (cost, cardinality) =
            cost_model.calculate_join_cost(&left_card, &right_card, &JoinType::Inner, 0.01);

        assert!(cost.cpu_cost > 0.0);
        assert!(cost.memory_cost > 0.0);
        assert!(cardinality.rows <= left_card.rows * right_card.rows);
    }

    #[test]
    fn test_aggregate_cost() {
        let cost_model = CostModel::new();
        let input_card = CardinalityEstimate::new(100000, 64);

        let (cost, cardinality) = cost_model.calculate_aggregate_cost(&input_card, 2, 3);

        assert!(cost.cpu_cost > 0.0);
        assert!(cardinality.rows < input_card.rows); // Aggregation reduces rows
    }

    #[test]
    fn test_sort_cost() {
        let cost_model = CostModel::new();
        let input_card = CardinalityEstimate::new(50000, 80);

        let (cost, cardinality) = cost_model.calculate_sort_cost(&input_card, 2);

        assert!(cost.cpu_cost > 0.0);
        assert_eq!(cardinality.rows, input_card.rows); // Sort doesn't change row count
    }

    #[test]
    fn test_parallel_cost() {
        let cost_model = CostModel::new();

        let base_cost = QueryCost {
            cpu_cost: 1000.0,
            io_cost: 500.0,
            memory_cost: 100.0,
            network_cost: 50.0,
            total_time_ms: 5000.0,
        };

        let base_card = CardinalityEstimate::new(100000, 64);

        let (parallel_cost, _) = cost_model.calculate_parallel_cost(&base_cost, &base_card, 4);

        // Parallel cost should be lower due to speedup
        assert!(parallel_cost.cpu_cost < base_cost.cpu_cost);
        assert!(parallel_cost.total_time_ms < base_cost.total_time_ms);
    }

    #[test]
    fn test_jit_threshold() {
        let cost_model = CostModel::new();

        let low_cost = QueryCost {
            cpu_cost: 1000.0,
            io_cost: 500.0,
            memory_cost: 100.0,
            network_cost: 50.0,
            total_time_ms: 1000.0,
        };

        let high_cost = QueryCost {
            cpu_cost: 100000.0,
            io_cost: 50000.0,
            memory_cost: 10000.0,
            network_cost: 5000.0,
            total_time_ms: 60000.0,
        };

        assert!(!cost_model.should_use_jit(&low_cost));
        assert!(cost_model.should_use_jit(&high_cost));
    }
}
