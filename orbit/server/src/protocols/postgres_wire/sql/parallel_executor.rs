//! Parallel Query Execution Engine
//!
//! This module provides parallel query processing capabilities:
//! - Query partitioning strategies (range, hash, round-robin)
//! - Work distribution across worker threads
//! - Parallel execution of independent operations
//! - Result merging with proper ordering
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  ParallelCoordinator                    │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
//! │  │ Partitioner │  │  Scheduler  │  │   Merger    │      │
//! │  └─────────────┘  └─────────────┘  └─────────────┘      │
//! │         │                │                │             │
//! │         ▼                ▼                ▼             │
//! │  ┌─────────────────────────────────────────────┐        │
//! │  │              Worker Pool (Tokio)            │        │
//! │  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐│        │
//! │  │  │Worker 1│ │Worker 2│ │Worker 3│ │Worker N││        │
//! │  │  └────────┘ └────────┘ └────────┘ └────────┘│        │
//! │  └─────────────────────────────────────────────┘        │
//! └─────────────────────────────────────────────────────────┘
//! ```

use crate::protocols::postgres_wire::sql::statistics::TableStatistics;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Semaphore};

/// Configuration for parallel query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    /// Enable parallel execution
    pub enabled: bool,
    /// Maximum number of worker threads
    pub max_workers: usize,
    /// Minimum rows to consider parallel execution
    pub min_rows_for_parallel: usize,
    /// Target rows per partition
    pub rows_per_partition: usize,
    /// Maximum partitions per query
    pub max_partitions: usize,
    /// Timeout for parallel operations (seconds)
    pub timeout_seconds: u64,
    /// Enable parallel scans
    pub parallel_scan: bool,
    /// Enable parallel aggregation
    pub parallel_aggregate: bool,
    /// Enable parallel sort
    pub parallel_sort: bool,
    /// Enable parallel join
    pub parallel_join: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_workers: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4)
                .max(2),
            min_rows_for_parallel: 10_000,
            rows_per_partition: 50_000,
            max_partitions: 16,
            timeout_seconds: 300,
            parallel_scan: true,
            parallel_aggregate: true,
            parallel_sort: true,
            parallel_join: true,
        }
    }
}

/// Partitioning strategy for parallel execution
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionStrategy {
    /// Partition by value ranges (good for sorted data)
    Range {
        column: String,
        boundaries: Vec<SqlValue>,
    },
    /// Partition by hash of key column (good for uniform distribution)
    Hash {
        column: String,
        num_partitions: usize,
    },
    /// Round-robin partitioning (simple, good for any data)
    RoundRobin { num_partitions: usize },
    /// Partition by predefined row ranges
    RowRange {
        ranges: Vec<(usize, usize)>, // (start, end) pairs
    },
}

/// A partition of work to be executed
#[derive(Debug, Clone)]
pub struct WorkPartition {
    /// Partition identifier
    pub id: usize,
    /// Partition strategy used
    pub strategy: PartitionStrategy,
    /// Estimated rows in this partition
    pub estimated_rows: usize,
    /// Filter condition for this partition (if range/hash partitioned)
    pub filter: Option<PartitionFilter>,
    /// Row range (if row-range partitioned)
    pub row_range: Option<(usize, usize)>,
}

/// Filter for partition-specific data
#[derive(Debug, Clone)]
pub struct PartitionFilter {
    pub column: String,
    pub condition: PartitionCondition,
}

#[derive(Debug, Clone)]
pub enum PartitionCondition {
    Range {
        min: Option<SqlValue>,
        max: Option<SqlValue>,
    },
    HashMod {
        divisor: usize,
        remainder: usize,
    },
}

/// Result from a single partition
#[derive(Debug, Clone)]
pub struct PartitionResult {
    /// Partition ID
    pub partition_id: usize,
    /// Result rows
    pub rows: Vec<HashMap<String, SqlValue>>,
    /// Execution time
    pub execution_time: Duration,
    /// Rows processed
    pub rows_processed: usize,
    /// Any errors encountered
    pub error: Option<String>,
}

/// Merged result from parallel execution
#[derive(Debug, Clone)]
pub struct ParallelResult {
    /// Combined rows from all partitions
    pub rows: Vec<HashMap<String, SqlValue>>,
    /// Total execution time (wall clock)
    pub total_time: Duration,
    /// Sum of all partition execution times
    pub cpu_time: Duration,
    /// Number of partitions used
    pub num_partitions: usize,
    /// Number of workers used
    pub num_workers: usize,
    /// Individual partition results (for debugging)
    pub partition_stats: Vec<PartitionStats>,
}

#[derive(Debug, Clone)]
pub struct PartitionStats {
    pub partition_id: usize,
    pub rows_processed: usize,
    pub execution_time: Duration,
}

/// Parallel execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParallelStats {
    /// Total queries executed in parallel
    pub queries_parallelized: u64,
    /// Total queries executed serially (not worth parallelizing)
    pub queries_serial: u64,
    /// Total partitions created
    pub total_partitions: u64,
    /// Total speedup achieved (sum of cpu_time / wall_time ratios)
    pub total_speedup: f64,
    /// Average speedup
    pub avg_speedup: f64,
}

/// Work partitioner - divides work into partitions
pub struct WorkPartitioner {
    config: ParallelConfig,
}

impl WorkPartitioner {
    pub fn new(config: ParallelConfig) -> Self {
        Self { config }
    }

    /// Determine if a query should be parallelized
    pub fn should_parallelize(&self, estimated_rows: usize, operation: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        if estimated_rows < self.config.min_rows_for_parallel {
            return false;
        }

        match operation {
            "scan" => self.config.parallel_scan,
            "aggregate" => self.config.parallel_aggregate,
            "sort" => self.config.parallel_sort,
            "join" => self.config.parallel_join,
            _ => true,
        }
    }

    /// Create partitions for a table scan
    pub fn partition_scan(
        &self,
        table_name: &str,
        estimated_rows: usize,
        table_stats: Option<&TableStatistics>,
    ) -> Vec<WorkPartition> {
        let num_partitions = self.calculate_partition_count(estimated_rows);

        if num_partitions <= 1 {
            return vec![WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            }];
        }

        // Try to use range partitioning if we have statistics with histograms
        if let Some(stats) = table_stats {
            if let Some(partition_column) = self.find_partition_column(stats) {
                if let Some(col_stats) = stats.get_column_stats(&partition_column) {
                    if col_stats.histogram.is_some() {
                        return self.create_range_partitions(
                            table_name,
                            &partition_column,
                            estimated_rows,
                            num_partitions,
                            col_stats,
                        );
                    }
                }
            }
        }

        // Fall back to row-range partitioning
        self.create_row_range_partitions(estimated_rows, num_partitions)
    }

    /// Create partitions for aggregation
    pub fn partition_aggregate(
        &self,
        estimated_rows: usize,
        group_by_columns: &[String],
    ) -> Vec<WorkPartition> {
        let num_partitions = self.calculate_partition_count(estimated_rows);

        if num_partitions <= 1 || group_by_columns.is_empty() {
            return vec![WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            }];
        }

        // Use hash partitioning on the first group-by column for better aggregation
        let hash_column = group_by_columns[0].clone();
        self.create_hash_partitions(&hash_column, estimated_rows, num_partitions)
    }

    /// Create partitions for sorting
    pub fn partition_sort(&self, estimated_rows: usize, _sort_column: &str) -> Vec<WorkPartition> {
        let num_partitions = self.calculate_partition_count(estimated_rows);

        if num_partitions <= 1 {
            return vec![WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            }];
        }

        // For sorting, use row-range partitioning then merge-sort results
        self.create_row_range_partitions(estimated_rows, num_partitions)
    }

    /// Calculate optimal number of partitions
    fn calculate_partition_count(&self, estimated_rows: usize) -> usize {
        if estimated_rows < self.config.min_rows_for_parallel {
            return 1;
        }

        let partitions_by_rows = (estimated_rows / self.config.rows_per_partition).max(1);
        let partitions_by_workers = self.config.max_workers;

        partitions_by_rows
            .min(partitions_by_workers)
            .min(self.config.max_partitions)
    }

    /// Find best column for range partitioning
    fn find_partition_column(&self, stats: &TableStatistics) -> Option<String> {
        // Prefer columns with histograms and good distribution
        for (col_name, col_stats) in &stats.columns {
            if col_stats.histogram.is_some() && col_stats.distinct_count > 10 {
                return Some(col_name.clone());
            }
        }
        None
    }

    /// Create range-based partitions
    fn create_range_partitions(
        &self,
        _table_name: &str,
        column: &str,
        estimated_rows: usize,
        num_partitions: usize,
        col_stats: &crate::protocols::postgres_wire::sql::statistics::ColumnStatistics,
    ) -> Vec<WorkPartition> {
        let rows_per_partition = estimated_rows / num_partitions;
        let mut partitions = Vec::with_capacity(num_partitions);

        // Use histogram boundaries if available
        if let Some(histogram) = &col_stats.histogram {
            let boundaries = &histogram.boundaries;
            let step = boundaries.len() / num_partitions;

            for i in 0..num_partitions {
                let min_idx = i * step;
                let max_idx = if i == num_partitions - 1 {
                    boundaries.len() - 1
                } else {
                    (i + 1) * step
                };

                let min_val = if i == 0 {
                    None
                } else {
                    Some(SqlValue::DoublePrecision(boundaries[min_idx]))
                };

                let max_val = if i == num_partitions - 1 {
                    None
                } else {
                    Some(SqlValue::DoublePrecision(boundaries[max_idx]))
                };

                partitions.push(WorkPartition {
                    id: i,
                    strategy: PartitionStrategy::Range {
                        column: column.to_string(),
                        boundaries: vec![],
                    },
                    estimated_rows: rows_per_partition,
                    filter: Some(PartitionFilter {
                        column: column.to_string(),
                        condition: PartitionCondition::Range {
                            min: min_val,
                            max: max_val,
                        },
                    }),
                    row_range: None,
                });
            }
        } else {
            // Fall back to row-range if no histogram
            return self.create_row_range_partitions(estimated_rows, num_partitions);
        }

        partitions
    }

    /// Create hash-based partitions
    fn create_hash_partitions(
        &self,
        column: &str,
        estimated_rows: usize,
        num_partitions: usize,
    ) -> Vec<WorkPartition> {
        let rows_per_partition = estimated_rows / num_partitions;

        (0..num_partitions)
            .map(|i| WorkPartition {
                id: i,
                strategy: PartitionStrategy::Hash {
                    column: column.to_string(),
                    num_partitions,
                },
                estimated_rows: rows_per_partition,
                filter: Some(PartitionFilter {
                    column: column.to_string(),
                    condition: PartitionCondition::HashMod {
                        divisor: num_partitions,
                        remainder: i,
                    },
                }),
                row_range: None,
            })
            .collect()
    }

    /// Create row-range partitions
    fn create_row_range_partitions(
        &self,
        estimated_rows: usize,
        num_partitions: usize,
    ) -> Vec<WorkPartition> {
        let rows_per_partition = estimated_rows / num_partitions;
        let remainder = estimated_rows % num_partitions;

        let mut partitions = Vec::with_capacity(num_partitions);
        let mut current_row = 0;

        for i in 0..num_partitions {
            let extra = if i < remainder { 1 } else { 0 };
            let partition_rows = rows_per_partition + extra;
            let end_row = current_row + partition_rows;

            partitions.push(WorkPartition {
                id: i,
                strategy: PartitionStrategy::RowRange {
                    ranges: vec![(current_row, end_row)],
                },
                estimated_rows: partition_rows,
                filter: None,
                row_range: Some((current_row, end_row)),
            });

            current_row = end_row;
        }

        partitions
    }
}

/// Result merger - combines results from parallel partitions
pub struct ResultMerger;

impl ResultMerger {
    /// Compare two SqlValues for ordering
    fn compare_sql_values(a: &SqlValue, b: &SqlValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            // Numeric comparisons
            (SqlValue::SmallInt(a), SqlValue::SmallInt(b)) => a.cmp(b),
            (SqlValue::Integer(a), SqlValue::Integer(b)) => a.cmp(b),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => a.cmp(b),
            (SqlValue::Real(a), SqlValue::Real(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SqlValue::DoublePrecision(a), SqlValue::DoublePrecision(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }

            // String comparisons
            (SqlValue::Text(a), SqlValue::Text(b)) => a.cmp(b),
            (SqlValue::Varchar(a), SqlValue::Varchar(b)) => a.cmp(b),
            (SqlValue::Char(a), SqlValue::Char(b)) => a.cmp(b),

            // Null handling
            (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
            (SqlValue::Null, _) => Ordering::Greater, // Nulls sort last
            (_, SqlValue::Null) => Ordering::Less,

            // Cross-type: convert to string for comparison
            _ => a.to_postgres_string().cmp(&b.to_postgres_string()),
        }
    }

    /// Merge results from multiple partitions (simple concatenation)
    pub fn merge_unordered(results: Vec<PartitionResult>) -> ParallelResult {
        let start = Instant::now();
        let mut all_rows = Vec::new();
        let mut cpu_time = Duration::ZERO;
        let mut partition_stats = Vec::new();

        for result in &results {
            if result.error.is_none() {
                all_rows.extend(result.rows.clone());
            }
            cpu_time += result.execution_time;
            partition_stats.push(PartitionStats {
                partition_id: result.partition_id,
                rows_processed: result.rows_processed,
                execution_time: result.execution_time,
            });
        }

        ParallelResult {
            rows: all_rows,
            total_time: start.elapsed(),
            cpu_time,
            num_partitions: results.len(),
            num_workers: results.len(), // Approximation
            partition_stats,
        }
    }

    /// Merge results maintaining sort order
    pub fn merge_sorted(
        results: Vec<PartitionResult>,
        sort_column: &str,
        ascending: bool,
    ) -> ParallelResult {
        let start = Instant::now();
        let mut cpu_time = Duration::ZERO;
        let mut partition_stats = Vec::new();

        // Collect all rows
        let mut all_rows: Vec<HashMap<String, SqlValue>> = Vec::new();
        for result in &results {
            if result.error.is_none() {
                all_rows.extend(result.rows.clone());
            }
            cpu_time += result.execution_time;
            partition_stats.push(PartitionStats {
                partition_id: result.partition_id,
                rows_processed: result.rows_processed,
                execution_time: result.execution_time,
            });
        }

        // Sort merged results
        all_rows.sort_by(|a, b| {
            let val_a = a.get(sort_column);
            let val_b = b.get(sort_column);

            let ordering = match (val_a, val_b) {
                (Some(a), Some(b)) => Self::compare_sql_values(a, b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            if ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });

        ParallelResult {
            rows: all_rows,
            total_time: start.elapsed(),
            cpu_time,
            num_partitions: results.len(),
            num_workers: results.len(),
            partition_stats,
        }
    }

    /// Merge aggregation results
    pub fn merge_aggregates(
        results: Vec<PartitionResult>,
        group_by_columns: &[String],
        aggregates: &[AggregateSpec],
    ) -> ParallelResult {
        let start = Instant::now();
        let mut cpu_time = Duration::ZERO;
        let mut partition_stats = Vec::new();

        // Group by key -> accumulated values
        let mut grouped: HashMap<String, HashMap<String, AggregateAccumulator>> = HashMap::new();

        for result in &results {
            cpu_time += result.execution_time;
            partition_stats.push(PartitionStats {
                partition_id: result.partition_id,
                rows_processed: result.rows_processed,
                execution_time: result.execution_time,
            });

            if result.error.is_some() {
                continue;
            }

            for row in &result.rows {
                // Build group key
                let group_key = group_by_columns
                    .iter()
                    .map(|col| {
                        row.get(col)
                            .map(|v| v.to_postgres_string())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join("|");

                let accumulators = grouped.entry(group_key).or_default();

                // Accumulate each aggregate
                for agg in aggregates {
                    let acc = accumulators
                        .entry(agg.output_name.clone())
                        .or_insert_with(|| AggregateAccumulator::new(&agg.function));

                    if let Some(value) = row.get(&agg.input_column) {
                        acc.accumulate(value);
                    }
                }
            }
        }

        // Finalize aggregates
        let mut final_rows = Vec::new();
        for (group_key, accumulators) in grouped {
            let mut row = HashMap::new();

            // Add group-by columns
            let key_parts: Vec<&str> = group_key.split('|').collect();
            for (i, col) in group_by_columns.iter().enumerate() {
                if i < key_parts.len() {
                    row.insert(col.clone(), SqlValue::Text(key_parts[i].to_string()));
                }
            }

            // Add aggregate results
            for (name, acc) in accumulators {
                row.insert(name, acc.finalize());
            }

            final_rows.push(row);
        }

        ParallelResult {
            rows: final_rows,
            total_time: start.elapsed(),
            cpu_time,
            num_partitions: results.len(),
            num_workers: results.len(),
            partition_stats,
        }
    }
}

/// Aggregate specification for parallel aggregation
#[derive(Debug, Clone)]
pub struct AggregateSpec {
    pub function: AggregateFunction,
    pub input_column: String,
    pub output_name: String,
}

#[derive(Debug, Clone)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    // New aggregate functions
    ArrayAgg,
    StringAgg { delimiter: String },
    BoolAnd,
    BoolOr,
}

/// Accumulator for parallel aggregation
#[derive(Debug, Clone)]
struct AggregateAccumulator {
    function: AggregateFunction,
    count: i64,
    sum: f64,
    min: Option<SqlValue>,
    max: Option<SqlValue>,
    // New aggregate state
    values: Vec<SqlValue>,   // For ARRAY_AGG
    bool_result: Option<bool>, // For BOOL_AND/BOOL_OR
}

impl AggregateAccumulator {
    fn new(function: &AggregateFunction) -> Self {
        Self {
            function: function.clone(),
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
            values: Vec::new(),
            bool_result: None,
        }
    }

    fn accumulate(&mut self, value: &SqlValue) {
        self.count += 1;

        if let Some(f) = Self::to_f64(value) {
            self.sum += f;
        }

        // Update min
        if self.min.is_none()
            || Self::compare_values(value, self.min.as_ref().unwrap()) == std::cmp::Ordering::Less
        {
            self.min = Some(value.clone());
        }

        // Update max
        if self.max.is_none()
            || Self::compare_values(value, self.max.as_ref().unwrap())
                == std::cmp::Ordering::Greater
        {
            self.max = Some(value.clone());
        }

        // Accumulate for ARRAY_AGG and STRING_AGG (skip NULLs)
        if !value.is_null() {
            match &self.function {
                AggregateFunction::ArrayAgg | AggregateFunction::StringAgg { .. } => {
                    self.values.push(value.clone());
                }
                AggregateFunction::BoolAnd => {
                    if let SqlValue::Boolean(b) = value {
                        self.bool_result = Some(self.bool_result.unwrap_or(true) && *b);
                    }
                }
                AggregateFunction::BoolOr => {
                    if let SqlValue::Boolean(b) = value {
                        self.bool_result = Some(self.bool_result.unwrap_or(false) || *b);
                    }
                }
                _ => {}
            }
        }
    }

    fn compare_values(a: &SqlValue, b: &SqlValue) -> std::cmp::Ordering {
        ResultMerger::compare_sql_values(a, b)
    }

    fn finalize(&self) -> SqlValue {
        match &self.function {
            AggregateFunction::Count => SqlValue::BigInt(self.count),
            AggregateFunction::Sum => SqlValue::DoublePrecision(self.sum),
            AggregateFunction::Avg => {
                if self.count > 0 {
                    SqlValue::DoublePrecision(self.sum / self.count as f64)
                } else {
                    SqlValue::Null
                }
            }
            AggregateFunction::Min => self.min.clone().unwrap_or(SqlValue::Null),
            AggregateFunction::Max => self.max.clone().unwrap_or(SqlValue::Null),
            AggregateFunction::ArrayAgg => {
                if self.values.is_empty() {
                    SqlValue::Null
                } else {
                    SqlValue::Array(self.values.clone())
                }
            }
            AggregateFunction::StringAgg { delimiter } => {
                if self.values.is_empty() {
                    SqlValue::Null
                } else {
                    let strings: Vec<String> = self.values
                        .iter()
                        .map(|v| v.to_postgres_string())
                        .collect();
                    SqlValue::Text(strings.join(delimiter))
                }
            }
            AggregateFunction::BoolAnd => {
                self.bool_result.map(SqlValue::Boolean).unwrap_or(SqlValue::Null)
            }
            AggregateFunction::BoolOr => {
                self.bool_result.map(SqlValue::Boolean).unwrap_or(SqlValue::Null)
            }
        }
    }

    fn to_f64(value: &SqlValue) -> Option<f64> {
        match value {
            SqlValue::SmallInt(i) => Some(*i as f64),
            SqlValue::Integer(i) => Some(*i as f64),
            SqlValue::BigInt(i) => Some(*i as f64),
            SqlValue::Real(f) => Some(*f as f64),
            SqlValue::DoublePrecision(f) => Some(*f),
            SqlValue::Decimal(d) => d.to_string().parse().ok(),
            _ => None,
        }
    }
}

/// Parallel query coordinator
pub struct ParallelCoordinator {
    config: ParallelConfig,
    partitioner: WorkPartitioner,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<ParallelStats>>,
}

impl ParallelCoordinator {
    /// Create a new parallel coordinator
    pub fn new(config: ParallelConfig) -> Self {
        let max_workers = config.max_workers;
        Self {
            partitioner: WorkPartitioner::new(config.clone()),
            semaphore: Arc::new(Semaphore::new(max_workers)),
            stats: Arc::new(RwLock::new(ParallelStats::default())),
            config,
        }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(ParallelConfig::default())
    }

    /// Execute a scan operation in parallel
    pub async fn execute_parallel_scan<F, Fut>(
        &self,
        table_name: &str,
        estimated_rows: usize,
        table_stats: Option<&TableStatistics>,
        executor: F,
    ) -> ParallelResult
    where
        F: Fn(WorkPartition) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = PartitionResult> + Send,
    {
        if !self.partitioner.should_parallelize(estimated_rows, "scan") {
            // Execute serially
            let mut stats = self.stats.write().await;
            stats.queries_serial += 1;

            let partition = WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            };

            let result = executor(partition).await;
            return ResultMerger::merge_unordered(vec![result]);
        }

        // Create partitions
        let partitions = self
            .partitioner
            .partition_scan(table_name, estimated_rows, table_stats);

        // Execute in parallel
        self.execute_partitions(partitions, executor).await
    }

    /// Execute an aggregation in parallel
    pub async fn execute_parallel_aggregate<F, Fut>(
        &self,
        estimated_rows: usize,
        group_by_columns: &[String],
        aggregates: &[AggregateSpec],
        executor: F,
    ) -> ParallelResult
    where
        F: Fn(WorkPartition) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = PartitionResult> + Send,
    {
        if !self
            .partitioner
            .should_parallelize(estimated_rows, "aggregate")
        {
            let mut stats = self.stats.write().await;
            stats.queries_serial += 1;

            let partition = WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            };

            let result = executor(partition).await;
            return ResultMerger::merge_aggregates(vec![result], group_by_columns, aggregates);
        }

        let partitions = self
            .partitioner
            .partition_aggregate(estimated_rows, group_by_columns);
        let results = self.execute_partitions_raw(partitions, executor).await;

        ResultMerger::merge_aggregates(results, group_by_columns, aggregates)
    }

    /// Execute a sort in parallel
    pub async fn execute_parallel_sort<F, Fut>(
        &self,
        estimated_rows: usize,
        sort_column: &str,
        ascending: bool,
        executor: F,
    ) -> ParallelResult
    where
        F: Fn(WorkPartition) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = PartitionResult> + Send,
    {
        if !self.partitioner.should_parallelize(estimated_rows, "sort") {
            let mut stats = self.stats.write().await;
            stats.queries_serial += 1;

            let partition = WorkPartition {
                id: 0,
                strategy: PartitionStrategy::RoundRobin { num_partitions: 1 },
                estimated_rows,
                filter: None,
                row_range: Some((0, estimated_rows)),
            };

            let result = executor(partition).await;
            return ResultMerger::merge_sorted(vec![result], sort_column, ascending);
        }

        let partitions = self.partitioner.partition_sort(estimated_rows, sort_column);
        let results = self.execute_partitions_raw(partitions, executor).await;

        ResultMerger::merge_sorted(results, sort_column, ascending)
    }

    /// Execute partitions in parallel and merge results
    async fn execute_partitions<F, Fut>(
        &self,
        partitions: Vec<WorkPartition>,
        executor: F,
    ) -> ParallelResult
    where
        F: Fn(WorkPartition) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = PartitionResult> + Send,
    {
        let results = self.execute_partitions_raw(partitions, executor).await;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.queries_parallelized += 1;
        stats.total_partitions += results.len() as u64;

        ResultMerger::merge_unordered(results)
    }

    /// Execute partitions and return raw results
    async fn execute_partitions_raw<F, Fut>(
        &self,
        partitions: Vec<WorkPartition>,
        executor: F,
    ) -> Vec<PartitionResult>
    where
        F: Fn(WorkPartition) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = PartitionResult> + Send,
    {
        let num_partitions = partitions.len();
        let (tx, mut rx) = mpsc::channel(num_partitions);

        for partition in partitions {
            let semaphore = Arc::clone(&self.semaphore);
            let executor = executor.clone();
            let tx = tx.clone();
            let timeout = Duration::from_secs(self.config.timeout_seconds);

            tokio::spawn(async move {
                // Acquire worker slot
                let _permit = semaphore.acquire().await.unwrap();

                // Execute with timeout
                let result = tokio::time::timeout(timeout, executor(partition.clone())).await;

                let partition_result = match result {
                    Ok(r) => r,
                    Err(_) => PartitionResult {
                        partition_id: partition.id,
                        rows: vec![],
                        execution_time: timeout,
                        rows_processed: 0,
                        error: Some("Timeout".to_string()),
                    },
                };

                let _ = tx.send(partition_result).await;
            });
        }

        drop(tx); // Close sender

        // Collect results
        let mut results = Vec::with_capacity(num_partitions);
        while let Some(result) = rx.recv().await {
            results.push(result);
        }

        // Sort by partition ID for consistent ordering
        results.sort_by_key(|r| r.partition_id);
        results
    }

    /// Get parallel execution statistics
    pub async fn stats(&self) -> ParallelStats {
        self.stats.read().await.clone()
    }

    /// Check if parallel execution is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get maximum workers
    pub fn max_workers(&self) -> usize {
        self.config.max_workers
    }
}

impl Default for ParallelCoordinator {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_count_calculation() {
        let config = ParallelConfig {
            min_rows_for_parallel: 1000,
            rows_per_partition: 10_000,
            max_partitions: 8,
            max_workers: 4,
            ..Default::default()
        };
        let partitioner = WorkPartitioner::new(config);

        // Small dataset - no partitioning
        let partitions = partitioner.partition_scan("test", 500, None);
        assert_eq!(partitions.len(), 1);

        // Medium dataset
        let partitions = partitioner.partition_scan("test", 25_000, None);
        assert!(partitions.len() >= 2);
        assert!(partitions.len() <= 4);

        // Large dataset
        let partitions = partitioner.partition_scan("test", 100_000, None);
        assert!(partitions.len() >= 2);
    }

    #[test]
    fn test_row_range_partitioning() {
        let config = ParallelConfig {
            min_rows_for_parallel: 100,
            rows_per_partition: 1000,
            max_partitions: 4,
            max_workers: 4,
            ..Default::default()
        };
        let partitioner = WorkPartitioner::new(config);

        let partitions = partitioner.create_row_range_partitions(10_000, 4);

        assert_eq!(partitions.len(), 4);

        // Check that partitions cover all rows
        let total_rows: usize = partitions.iter().map(|p| p.estimated_rows).sum();
        assert_eq!(total_rows, 10_000);

        // Check that ranges are contiguous
        for i in 0..partitions.len() - 1 {
            let current_end = partitions[i].row_range.unwrap().1;
            let next_start = partitions[i + 1].row_range.unwrap().0;
            assert_eq!(current_end, next_start);
        }
    }

    #[test]
    fn test_hash_partitioning() {
        let config = ParallelConfig::default();
        let partitioner = WorkPartitioner::new(config);

        let partitions = partitioner.create_hash_partitions("user_id", 10_000, 4);

        assert_eq!(partitions.len(), 4);

        for (i, partition) in partitions.iter().enumerate() {
            assert_eq!(partition.id, i);
            if let Some(filter) = &partition.filter {
                if let PartitionCondition::HashMod { divisor, remainder } = &filter.condition {
                    assert_eq!(*divisor, 4);
                    assert_eq!(*remainder, i);
                }
            }
        }
    }

    #[test]
    fn test_should_parallelize() {
        let config = ParallelConfig {
            enabled: true,
            min_rows_for_parallel: 10_000,
            parallel_scan: true,
            parallel_aggregate: true,
            parallel_sort: false, // Disabled
            ..Default::default()
        };
        let partitioner = WorkPartitioner::new(config);

        // Below threshold
        assert!(!partitioner.should_parallelize(5_000, "scan"));

        // Above threshold, enabled operation
        assert!(partitioner.should_parallelize(50_000, "scan"));
        assert!(partitioner.should_parallelize(50_000, "aggregate"));

        // Above threshold, disabled operation
        assert!(!partitioner.should_parallelize(50_000, "sort"));
    }

    #[test]
    fn test_merge_unordered() {
        let results = vec![
            PartitionResult {
                partition_id: 0,
                rows: vec![
                    [("id".to_string(), SqlValue::Integer(1))]
                        .into_iter()
                        .collect(),
                    [("id".to_string(), SqlValue::Integer(2))]
                        .into_iter()
                        .collect(),
                ],
                execution_time: Duration::from_millis(100),
                rows_processed: 2,
                error: None,
            },
            PartitionResult {
                partition_id: 1,
                rows: vec![[("id".to_string(), SqlValue::Integer(3))]
                    .into_iter()
                    .collect()],
                execution_time: Duration::from_millis(50),
                rows_processed: 1,
                error: None,
            },
        ];

        let merged = ResultMerger::merge_unordered(results);

        assert_eq!(merged.rows.len(), 3);
        assert_eq!(merged.num_partitions, 2);
        assert_eq!(merged.cpu_time, Duration::from_millis(150));
    }

    #[test]
    fn test_merge_sorted() {
        let results = vec![
            PartitionResult {
                partition_id: 0,
                rows: vec![
                    [("value".to_string(), SqlValue::Integer(3))]
                        .into_iter()
                        .collect(),
                    [("value".to_string(), SqlValue::Integer(1))]
                        .into_iter()
                        .collect(),
                ],
                execution_time: Duration::from_millis(100),
                rows_processed: 2,
                error: None,
            },
            PartitionResult {
                partition_id: 1,
                rows: vec![[("value".to_string(), SqlValue::Integer(2))]
                    .into_iter()
                    .collect()],
                execution_time: Duration::from_millis(50),
                rows_processed: 1,
                error: None,
            },
        ];

        let merged = ResultMerger::merge_sorted(results, "value", true);

        assert_eq!(merged.rows.len(), 3);

        // Check order
        let values: Vec<i32> = merged
            .rows
            .iter()
            .filter_map(|r| r.get("value"))
            .filter_map(|v| match v {
                SqlValue::Integer(i) => Some(*i),
                _ => None,
            })
            .collect();

        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_aggregate_accumulator() {
        let mut acc = AggregateAccumulator::new(&AggregateFunction::Sum);

        acc.accumulate(&SqlValue::Integer(10));
        acc.accumulate(&SqlValue::Integer(20));
        acc.accumulate(&SqlValue::Integer(30));

        if let SqlValue::DoublePrecision(sum) = acc.finalize() {
            assert!((sum - 60.0).abs() < 0.001);
        } else {
            panic!("Expected DoublePrecision");
        }
    }

    #[tokio::test]
    async fn test_parallel_coordinator() {
        let coordinator = ParallelCoordinator::new(ParallelConfig {
            enabled: true,
            max_workers: 2,
            min_rows_for_parallel: 100,
            rows_per_partition: 500,
            ..Default::default()
        });

        // Simple executor that returns partition ID
        let executor = |partition: WorkPartition| async move {
            PartitionResult {
                partition_id: partition.id,
                rows: vec![[(
                    "partition".to_string(),
                    SqlValue::Integer(partition.id as i32),
                )]
                .into_iter()
                .collect()],
                execution_time: Duration::from_millis(10),
                rows_processed: 1,
                error: None,
            }
        };

        let result = coordinator
            .execute_parallel_scan("test", 2000, None, executor)
            .await;

        assert!(result.num_partitions >= 2);
        assert!(!result.rows.is_empty());
    }
}
