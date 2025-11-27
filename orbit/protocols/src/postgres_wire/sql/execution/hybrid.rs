//! Hybrid Storage Engine
//!
//! Implements tiered storage strategy combining row-based and columnar storage:
//! - Hot tier: Row-based for OLTP (writes, updates, deletes, point queries)
//! - Warm tier: Hybrid for mixed workloads
//! - Cold tier: Columnar for analytics (aggregations, scans, historical data)
//!
//! ## Design Patterns
//!
//! - **Strategy Pattern**: Pluggable storage engines
//! - **Facade Pattern**: Unified interface over multiple storage tiers
//! - **Observer Pattern**: Data lifecycle events
//! - **Command Pattern**: Storage tier migration

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

use super::{
    AggregateFunction, Column, ColumnBatch, ComparisonOp, NullBitmap, VectorizedExecutor,
    VectorizedExecutorConfig,
};
use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::types::SqlValue;

/// Primary key value that can be hashed and compared
///
/// Wraps SqlValue variants that are safe for use as primary keys
/// (excludes floating point types which don't have total ordering)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimaryKey {
    Null,
    Boolean(bool),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Text(String),
    Bytea(Vec<u8>),
}

impl PrimaryKey {
    /// Convert from SqlValue (if it's a valid primary key type)
    pub fn from_sql_value(value: &SqlValue) -> ProtocolResult<Self> {
        match value {
            SqlValue::Null => Ok(PrimaryKey::Null),
            SqlValue::Boolean(b) => Ok(PrimaryKey::Boolean(*b)),
            SqlValue::SmallInt(i) => Ok(PrimaryKey::SmallInt(*i)),
            SqlValue::Integer(i) => Ok(PrimaryKey::Integer(*i)),
            SqlValue::BigInt(i) => Ok(PrimaryKey::BigInt(*i)),
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(PrimaryKey::Text(s.clone()))
            }
            SqlValue::Bytea(b) => Ok(PrimaryKey::Bytea(b.clone())),

            // Reject types that can't be primary keys
            SqlValue::Real(_) | SqlValue::DoublePrecision(_) => Err(ProtocolError::PostgresError(
                "Floating point types cannot be used as primary keys".to_string(),
            )),
            SqlValue::Decimal(_) => Err(ProtocolError::PostgresError(
                "Decimal type not yet supported as primary key".to_string(),
            )),
            _ => Err(ProtocolError::PostgresError(format!(
                "Type {:?} cannot be used as primary key",
                value
            ))),
        }
    }

    /// Convert to SqlValue
    pub fn to_sql_value(&self) -> SqlValue {
        match self {
            PrimaryKey::Null => SqlValue::Null,
            PrimaryKey::Boolean(b) => SqlValue::Boolean(*b),
            PrimaryKey::SmallInt(i) => SqlValue::SmallInt(*i),
            PrimaryKey::Integer(i) => SqlValue::Integer(*i),
            PrimaryKey::BigInt(i) => SqlValue::BigInt(*i),
            PrimaryKey::Text(s) => SqlValue::Text(s.clone()),
            PrimaryKey::Bytea(b) => SqlValue::Bytea(b.clone()),
        }
    }
}

/// Storage tier types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTier {
    /// Hot tier: Row-based, optimized for writes/updates/deletes
    /// Typically: Last 24-48 hours of data
    Hot,

    /// Warm tier: Hybrid format, balanced for mixed workloads
    /// Typically: 2-30 days of data
    Warm,

    /// Cold tier: Columnar, optimized for analytics
    /// Typically: >30 days, archival, historical data
    Cold,
}

impl StorageTier {
    /// Get the typical retention period for this tier
    pub fn retention_period(&self) -> Duration {
        match self {
            StorageTier::Hot => Duration::from_secs(24 * 60 * 60), // 24 hours
            StorageTier::Warm => Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            StorageTier::Cold => Duration::from_secs(365 * 24 * 60 * 60), // 1 year
        }
    }

    /// Check if this tier is suitable for a given data age
    pub fn is_suitable_for_age(&self, age: Duration) -> bool {
        match self {
            StorageTier::Hot => age < Duration::from_secs(48 * 60 * 60),
            StorageTier::Warm => {
                age >= Duration::from_secs(48 * 60 * 60)
                    && age < Duration::from_secs(30 * 24 * 60 * 60)
            }
            StorageTier::Cold => age >= Duration::from_secs(30 * 24 * 60 * 60),
        }
    }
}

/// Workload classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadType {
    /// OLTP: Point queries, writes, updates, deletes
    Transactional,

    /// OLAP: Aggregations, scans, analytical queries
    Analytical,

    /// Mixed: Both OLTP and OLAP patterns
    Mixed,
}

/// Query access pattern
#[derive(Debug, Clone, PartialEq)]
pub enum AccessPattern {
    /// Point lookup by primary key
    PointLookup { key: SqlValue },

    /// Sequential scan with optional filter
    Scan {
        time_range: Option<TimeRange>,
        filter: Option<FilterPredicate>,
    },

    /// Aggregation query
    Aggregation {
        function: AggregateFunction,
        column: String,
        filter: Option<FilterPredicate>,
    },

    /// Insert operation
    Insert { row_count: usize },

    /// Update operation
    Update {
        filter: FilterPredicate,
        estimated_rows: usize,
    },

    /// Delete operation
    Delete {
        filter: FilterPredicate,
        estimated_rows: usize,
    },
}

/// Time range for queries
#[derive(Debug, Clone, PartialEq)]
pub struct TimeRange {
    pub start: SystemTime,
    pub end: SystemTime,
}

impl TimeRange {
    pub fn new(start: SystemTime, end: SystemTime) -> Self {
        Self { start, end }
    }

    /// Check if this range overlaps with hot tier (last 24-48 hours)
    pub fn overlaps_hot(&self) -> bool {
        let hot_cutoff = SystemTime::now() - Duration::from_secs(48 * 60 * 60);
        self.end >= hot_cutoff
    }

    /// Check if this range overlaps with warm tier (2-30 days)
    pub fn overlaps_warm(&self) -> bool {
        let warm_start = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        let warm_end = SystemTime::now() - Duration::from_secs(2 * 24 * 60 * 60);
        self.start <= warm_end && self.end >= warm_start
    }

    /// Check if this range includes cold tier (>30 days)
    pub fn overlaps_cold(&self) -> bool {
        let cold_cutoff = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        self.start < cold_cutoff
    }

    /// Get the duration of this time range
    pub fn duration(&self) -> Duration {
        self.end
            .duration_since(self.start)
            .unwrap_or(Duration::ZERO)
    }
}

/// Filter predicate
#[derive(Debug, Clone, PartialEq)]
pub struct FilterPredicate {
    pub column: String,
    pub operator: ComparisonOp,
    pub value: SqlValue,
}

/// Row-based storage for hot tier (OLTP)
#[derive(Debug, Clone)]
pub struct RowBasedStore {
    /// Table name
    #[allow(dead_code)]
    table_name: String,

    /// Rows stored in row-major format
    rows: Vec<Row>,

    /// Column schema
    schema: Vec<ColumnSchema>,

    /// Primary key index
    primary_index: HashMap<PrimaryKey, usize>,

    /// Creation timestamp
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<SqlValue>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

impl RowBasedStore {
    pub fn new(table_name: String, schema: Vec<ColumnSchema>) -> Self {
        Self {
            table_name,
            rows: Vec::new(),
            schema,
            primary_index: HashMap::new(),
            created_at: SystemTime::now(),
        }
    }

    /// Insert a row
    pub fn insert(&mut self, values: Vec<SqlValue>) -> ProtocolResult<()> {
        if values.len() != self.schema.len() {
            return Err(ProtocolError::PostgresError(format!(
                "Expected {} values, got {}",
                self.schema.len(),
                values.len()
            )));
        }

        let row = Row {
            values: values.clone(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        // Add to primary index (assuming first column is primary key)
        let row_idx = self.rows.len();
        if !values.is_empty() {
            let key = PrimaryKey::from_sql_value(&values[0])?;
            self.primary_index.insert(key, row_idx);
        }

        self.rows.push(row);
        Ok(())
    }

    /// Get row by primary key
    pub fn get(&self, key: &SqlValue) -> Option<&Row> {
        let pk = PrimaryKey::from_sql_value(key).ok()?;
        self.primary_index
            .get(&pk)
            .and_then(|&idx| self.rows.get(idx))
    }

    /// Update rows matching filter
    pub fn update(
        &mut self,
        filter: &FilterPredicate,
        updates: HashMap<String, SqlValue>,
    ) -> ProtocolResult<usize> {
        let mut updated_count = 0;

        // First, find matching row indices
        let mut matching_indices = Vec::new();
        for (idx, row) in self.rows.iter().enumerate() {
            if self.matches_filter(row, filter)? {
                matching_indices.push(idx);
            }
        }

        // Then, update the matching rows
        for idx in matching_indices {
            if let Some(row) = self.rows.get_mut(idx) {
                for (col_name, new_value) in &updates {
                    if let Some(col_idx) = self.schema.iter().position(|s| &s.name == col_name) {
                        row.values[col_idx] = new_value.clone();
                        row.updated_at = SystemTime::now();
                        updated_count += 1;
                    }
                }
            }
        }

        Ok(updated_count)
    }

    /// Delete rows matching filter
    pub fn delete(&mut self, filter: &FilterPredicate) -> ProtocolResult<usize> {
        let initial_len = self.rows.len();

        // Mark rows for deletion
        let mut to_delete = Vec::new();
        for (idx, row) in self.rows.iter().enumerate() {
            if self.matches_filter(row, filter)? {
                to_delete.push(idx);
            }
        }

        // Remove in reverse order to maintain indices
        for &idx in to_delete.iter().rev() {
            self.rows.remove(idx);
        }

        // Rebuild primary index
        self.rebuild_primary_index();

        Ok(initial_len - self.rows.len())
    }

    /// Scan all rows with optional filter
    pub fn scan(&self, filter: Option<&FilterPredicate>) -> ProtocolResult<Vec<&Row>> {
        let mut results = Vec::new();

        for row in &self.rows {
            if let Some(f) = filter {
                if self.matches_filter(row, f)? {
                    results.push(row);
                }
            } else {
                results.push(row);
            }
        }

        Ok(results)
    }

    /// Convert to columnar format
    pub fn to_columnar(&self) -> ProtocolResult<ColumnBatch> {
        if self.rows.is_empty() {
            return Err(ProtocolError::PostgresError(
                "No rows to convert".to_string(),
            ));
        }

        let row_count = self.rows.len();
        let col_count = self.schema.len();

        let mut columns = Vec::new();
        let mut null_bitmaps = Vec::new();

        for col_idx in 0..col_count {
            let mut column_values = Vec::new();
            let mut null_bitmap = NullBitmap::new_all_valid(row_count);

            for (row_idx, row) in self.rows.iter().enumerate() {
                match &row.values[col_idx] {
                    SqlValue::Null => {
                        null_bitmap.set_null(row_idx);
                        column_values.push(0); // Placeholder
                    }
                    SqlValue::Integer(v) => column_values.push(*v),
                    SqlValue::BigInt(v) => column_values.push(*v as i32), // Simplified
                    _ => column_values.push(0),                           // Handle other types
                }
            }

            columns.push(Column::Int32(column_values));
            null_bitmaps.push(null_bitmap);
        }

        Ok(ColumnBatch {
            columns,
            null_bitmaps,
            row_count,
            column_names: Some(self.schema.iter().map(|s| s.name.clone()).collect()),
        })
    }

    /// Get row count
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get data age (time since creation)
    pub fn data_age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or(Duration::ZERO)
    }

    // Private helper methods

    fn matches_filter(&self, row: &Row, filter: &FilterPredicate) -> ProtocolResult<bool> {
        let col_idx = self
            .schema
            .iter()
            .position(|s| s.name == filter.column)
            .ok_or_else(|| {
                ProtocolError::PostgresError(format!("Column {} not found", filter.column))
            })?;

        let value = &row.values[col_idx];

        Ok(match filter.operator {
            ComparisonOp::Equal => value == &filter.value,
            ComparisonOp::NotEqual => value != &filter.value,
            ComparisonOp::LessThan => self.compare_values(value, &filter.value)? < 0,
            ComparisonOp::LessThanOrEqual => self.compare_values(value, &filter.value)? <= 0,
            ComparisonOp::GreaterThan => self.compare_values(value, &filter.value)? > 0,
            ComparisonOp::GreaterThanOrEqual => self.compare_values(value, &filter.value)? >= 0,
        })
    }

    fn compare_values(&self, a: &SqlValue, b: &SqlValue) -> ProtocolResult<i32> {
        match (a, b) {
            (SqlValue::Integer(x), SqlValue::Integer(y)) => Ok(x.cmp(y) as i32),
            (SqlValue::BigInt(x), SqlValue::BigInt(y)) => Ok(x.cmp(y) as i32),
            (SqlValue::SmallInt(x), SqlValue::SmallInt(y)) => Ok(x.cmp(y) as i32),
            (SqlValue::Text(x), SqlValue::Text(y)) => Ok(x.cmp(y) as i32),
            _ => Err(ProtocolError::PostgresError(
                "Cannot compare values of different types".to_string(),
            )),
        }
    }

    fn rebuild_primary_index(&mut self) {
        self.primary_index.clear();
        for (idx, row) in self.rows.iter().enumerate() {
            if !row.values.is_empty() {
                if let Ok(key) = PrimaryKey::from_sql_value(&row.values[0]) {
                    self.primary_index.insert(key, idx);
                }
            }
        }
    }
}

/// Hybrid storage manager
pub struct HybridStorageManager {
    /// Table name
    #[allow(dead_code)]
    table_name: String,

    /// Hot tier storage (row-based)
    hot_store: Arc<RwLock<RowBasedStore>>,

    /// Warm tier storage (hybrid - for future implementation)
    #[allow(dead_code)]
    warm_store: Arc<RwLock<Option<ColumnBatch>>>,

    /// Cold tier storage (Iceberg-based for long-term archival)
    #[cfg(feature = "iceberg-cold")]
    cold_store: Option<Arc<crate::postgres_wire::sql::execution::iceberg_cold::IcebergColdStore>>,

    /// Cold tier storage (simple columnar fallback when Iceberg disabled)
    #[cfg(not(feature = "iceberg-cold"))]
    cold_store: Arc<RwLock<Option<ColumnBatch>>>,

    /// Vectorized executor for columnar operations
    vectorized_executor: VectorizedExecutor,

    /// Configuration
    config: HybridStorageConfig,
}

#[derive(Debug, Clone)]
pub struct HybridStorageConfig {
    /// Hot tier age threshold (migrate to warm after this)
    pub hot_to_warm_threshold: Duration,

    /// Warm tier age threshold (migrate to cold after this)
    pub warm_to_cold_threshold: Duration,

    /// Enable automatic tiering
    pub auto_tiering: bool,

    /// Background migration enabled
    pub background_migration: bool,
}

impl Default for HybridStorageConfig {
    fn default() -> Self {
        Self {
            hot_to_warm_threshold: Duration::from_secs(48 * 60 * 60), // 48 hours
            warm_to_cold_threshold: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            auto_tiering: true,
            background_migration: true,
        }
    }
}

impl HybridStorageManager {
    pub fn new(table_name: String, schema: Vec<ColumnSchema>, config: HybridStorageConfig) -> Self {
        Self {
            table_name: table_name.clone(),
            hot_store: Arc::new(RwLock::new(RowBasedStore::new(table_name, schema))),
            warm_store: Arc::new(RwLock::new(None)),
            #[cfg(feature = "iceberg-cold")]
            cold_store: None,
            #[cfg(not(feature = "iceberg-cold"))]
            cold_store: Arc::new(RwLock::new(None)),
            vectorized_executor: VectorizedExecutor::with_config(
                VectorizedExecutorConfig::default(),
            ),
            config,
        }
    }

    /// Set the Iceberg cold store (optional - for archival tier)
    #[cfg(feature = "iceberg-cold")]
    pub fn with_cold_store(
        mut self,
        cold_store: Arc<crate::postgres_wire::sql::execution::iceberg_cold::IcebergColdStore>,
    ) -> Self {
        self.cold_store = Some(cold_store);
        self
    }

    /// Execute a query using the appropriate storage tier(s)
    pub async fn execute(&self, access_pattern: AccessPattern) -> ProtocolResult<QueryResult> {
        match access_pattern {
            AccessPattern::PointLookup { key } => self.execute_point_lookup(key).await,

            AccessPattern::Scan { time_range, filter } => {
                self.execute_scan(time_range, filter).await
            }

            AccessPattern::Aggregation {
                function,
                column,
                filter,
            } => self.execute_aggregation(function, column, filter).await,

            AccessPattern::Insert { row_count: _ } => {
                // Inserts always go to hot tier
                Ok(QueryResult::Modified { rows_affected: 0 })
            }

            AccessPattern::Update {
                filter,
                estimated_rows: _,
            } => self.execute_update(filter).await,

            AccessPattern::Delete {
                filter,
                estimated_rows: _,
            } => self.execute_delete(filter).await,
        }
    }

    /// Insert into hot tier
    pub async fn insert(&self, values: Vec<SqlValue>) -> ProtocolResult<()> {
        let mut hot = self.hot_store.write().await;
        hot.insert(values)
    }

    /// Scan all data from hot tier (simple scan for TableStorage compatibility)
    pub async fn scan_all(&self) -> ProtocolResult<Vec<Vec<SqlValue>>> {
        let hot = self.hot_store.read().await;
        let rows = hot.scan(None)?;
        Ok(rows.into_iter().map(|row| row.values.clone()).collect())
    }

    /// Migrate hot data to warm/cold tiers
    pub async fn migrate_tiers(&self) -> ProtocolResult<MigrationStats> {
        let mut stats = MigrationStats::default();

        // Check if hot tier needs migration
        let hot_age = {
            let hot = self.hot_store.read().await;
            hot.data_age()
        };

        if hot_age > self.config.hot_to_warm_threshold {
            // Migrate hot → cold (skip warm for now)
            let hot_data = {
                let hot = self.hot_store.read().await;
                hot.to_columnar()?
            };

            stats.rows_migrated = hot_data.row_count;

            // When Iceberg is enabled, write to Iceberg table
            #[cfg(feature = "iceberg-cold")]
            if let Some(ref cold_store) = self.cold_store {
                // Write to Iceberg (Phase 3: full implementation pending)
                // For now, this will return an error indicating write path not ready
                let _write_result = cold_store.write(&hot_data).await;
                // Note: We don't fail migration if write fails - just log it
                // In production, this would be handled by a background archival process
            }

            // Fallback: store in simple columnar format
            #[cfg(not(feature = "iceberg-cold"))]
            {
                let mut cold = self.cold_store.write().await;
                *cold = Some(hot_data);
            }

            // Clear hot tier after migration
            // (In production, this would be more sophisticated)
        }

        Ok(stats)
    }

    // Private execution methods

    async fn execute_point_lookup(&self, key: SqlValue) -> ProtocolResult<QueryResult> {
        // Try hot tier first
        let hot = self.hot_store.read().await;
        if let Some(row) = hot.get(&key) {
            return Ok(QueryResult::Rows {
                rows: vec![row.values.clone()],
                column_names: hot.schema.iter().map(|s| s.name.clone()).collect(),
            });
        }

        // For now, return empty if not in hot
        // In production, would check warm and cold tiers
        Ok(QueryResult::Rows {
            rows: vec![],
            column_names: vec![],
        })
    }

    async fn execute_scan(
        &self,
        time_range: Option<TimeRange>,
        filter: Option<FilterPredicate>,
    ) -> ProtocolResult<QueryResult> {
        let mut all_rows = Vec::new();
        let mut column_names = Vec::new();

        // Scan hot tier if needed
        if time_range
            .as_ref()
            .map(|r| r.overlaps_hot())
            .unwrap_or(true)
        {
            let hot = self.hot_store.read().await;
            let rows = hot.scan(filter.as_ref())?;
            all_rows.extend(rows.iter().map(|r| r.values.clone()));
            column_names = hot.schema.iter().map(|s| s.name.clone()).collect();
        }

        // TODO: Scan warm tier

        // Scan cold tier using Iceberg (if available)
        #[cfg(feature = "iceberg-cold")]
        if time_range
            .as_ref()
            .map(|r| r.overlaps_cold())
            .unwrap_or(true)
        {
            if let Some(ref cold_store) = self.cold_store {
                // Query Iceberg cold tier
                let arrow_batches = cold_store.scan(filter.as_ref()).await?;

                // Convert Arrow batches to rows
                for arrow_batch in arrow_batches {
                    let column_batch =
                        crate::postgres_wire::sql::execution::iceberg_cold::arrow_to_column_batch(
                            &arrow_batch,
                        )?;

                    // Extract column names if not yet set
                    if column_names.is_empty() {
                        if let Some(ref names) = column_batch.column_names {
                            column_names = names.clone();
                        }
                    }

                    // Convert ColumnBatch to rows (transpose columnar → row format)
                    for row_idx in 0..column_batch.row_count {
                        let mut row_values = Vec::new();
                        for (col_idx, column) in column_batch.columns.iter().enumerate() {
                            let null_bitmap = &column_batch.null_bitmaps[col_idx];

                            if null_bitmap.is_null(row_idx) {
                                row_values.push(SqlValue::Null);
                            } else {
                                let value = match column {
                                    Column::Bool(vals) => SqlValue::Boolean(vals[row_idx]),
                                    Column::Int16(vals) => SqlValue::SmallInt(vals[row_idx]),
                                    Column::Int32(vals) => SqlValue::Integer(vals[row_idx]),
                                    Column::Int64(vals) => SqlValue::BigInt(vals[row_idx]),
                                    Column::Float32(vals) => SqlValue::Real(vals[row_idx]),
                                    Column::Float64(vals) => {
                                        SqlValue::DoublePrecision(vals[row_idx])
                                    }
                                    Column::String(vals) => SqlValue::Text(vals[row_idx].clone()),
                                    Column::Binary(vals) => SqlValue::Bytea(vals[row_idx].clone()),
                                };
                                row_values.push(value);
                            }
                        }
                        all_rows.push(row_values);
                    }
                }
            }
        }

        Ok(QueryResult::Rows {
            rows: all_rows,
            column_names,
        })
    }

    async fn execute_aggregation(
        &self,
        function: AggregateFunction,
        column: String,
        _filter: Option<FilterPredicate>,
    ) -> ProtocolResult<QueryResult> {
        // For aggregations, prefer cold tier (columnar + SIMD + Iceberg metadata pruning)
        #[cfg(feature = "iceberg-cold")]
        if let Some(ref cold_store) = self.cold_store {
            // Use IcebergColdStore's aggregate method (combines metadata pruning + SIMD)
            let result = cold_store
                .aggregate(&column, function, _filter.as_ref())
                .await?;
            return Ok(QueryResult::Scalar { value: result });
        }

        // Fallback to simple columnar cold tier (when Iceberg disabled)
        #[cfg(not(feature = "iceberg-cold"))]
        {
            let cold = self.cold_store.read().await;

            if let Some(ref batch) = *cold {
                // Find column index
                let col_idx = batch
                    .column_names
                    .as_ref()
                    .and_then(|names| names.iter().position(|n| n == &column))
                    .ok_or_else(|| {
                        ProtocolError::PostgresError(format!("Column {} not found", column))
                    })?;

                // Execute aggregation using vectorized executor
                let result = self
                    .vectorized_executor
                    .execute_aggregation(batch, col_idx, function)?;

                return Ok(QueryResult::Scalar { value: result });
            }
        }

        // Fallback to hot tier
        // (Would need to implement row-based aggregation)
        Err(ProtocolError::PostgresError(
            "Aggregation not implemented for hot tier".to_string(),
        ))
    }

    async fn execute_update(&self, filter: FilterPredicate) -> ProtocolResult<QueryResult> {
        // Updates go to hot tier
        let mut hot = self.hot_store.write().await;
        let rows_affected = hot.update(&filter, HashMap::new())?;

        Ok(QueryResult::Modified { rows_affected })
    }

    async fn execute_delete(&self, filter: FilterPredicate) -> ProtocolResult<QueryResult> {
        // Deletes go to hot tier
        let mut hot = self.hot_store.write().await;
        let rows_affected = hot.delete(&filter)?;

        Ok(QueryResult::Modified { rows_affected })
    }
}

/// Query result
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Rows returned
    Rows {
        rows: Vec<Vec<SqlValue>>,
        column_names: Vec<String>,
    },

    /// Scalar result (aggregation)
    Scalar { value: SqlValue },

    /// Modified rows (INSERT/UPDATE/DELETE)
    Modified { rows_affected: usize },
}

/// Migration statistics
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    pub rows_migrated: usize,
    pub bytes_migrated: usize,
    pub duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_tier_age_suitability() {
        // Recent data → Hot
        let age_1h = Duration::from_secs(60 * 60);
        assert!(StorageTier::Hot.is_suitable_for_age(age_1h));
        assert!(!StorageTier::Warm.is_suitable_for_age(age_1h));
        assert!(!StorageTier::Cold.is_suitable_for_age(age_1h));

        // 5 days old → Warm
        let age_5d = Duration::from_secs(5 * 24 * 60 * 60);
        assert!(!StorageTier::Hot.is_suitable_for_age(age_5d));
        assert!(StorageTier::Warm.is_suitable_for_age(age_5d));
        assert!(!StorageTier::Cold.is_suitable_for_age(age_5d));

        // 60 days old → Cold
        let age_60d = Duration::from_secs(60 * 24 * 60 * 60);
        assert!(!StorageTier::Hot.is_suitable_for_age(age_60d));
        assert!(!StorageTier::Warm.is_suitable_for_age(age_60d));
        assert!(StorageTier::Cold.is_suitable_for_age(age_60d));
    }

    #[tokio::test]
    async fn test_row_based_store_insert() {
        let schema = vec![
            ColumnSchema {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                nullable: false,
            },
            ColumnSchema {
                name: "name".to_string(),
                data_type: "TEXT".to_string(),
                nullable: true,
            },
        ];

        let mut store = RowBasedStore::new("test_table".to_string(), schema);

        // Insert a row
        let values = vec![SqlValue::Integer(1), SqlValue::Text("Alice".to_string())];
        assert!(store.insert(values).is_ok());
        assert_eq!(store.row_count(), 1);

        // Get by primary key
        let row = store.get(&SqlValue::Integer(1));
        assert!(row.is_some());
    }

    #[tokio::test]
    async fn test_row_based_store_scan() {
        let schema = vec![
            ColumnSchema {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                nullable: false,
            },
            ColumnSchema {
                name: "age".to_string(),
                data_type: "INTEGER".to_string(),
                nullable: false,
            },
        ];

        let mut store = RowBasedStore::new("users".to_string(), schema);

        // Insert multiple rows
        store
            .insert(vec![SqlValue::Integer(1), SqlValue::Integer(25)])
            .unwrap();
        store
            .insert(vec![SqlValue::Integer(2), SqlValue::Integer(30)])
            .unwrap();
        store
            .insert(vec![SqlValue::Integer(3), SqlValue::Integer(35)])
            .unwrap();

        // Scan with filter
        let filter = FilterPredicate {
            column: "age".to_string(),
            operator: ComparisonOp::GreaterThan,
            value: SqlValue::Integer(28),
        };

        let results = store.scan(Some(&filter)).unwrap();
        assert_eq!(results.len(), 2); // age 30 and 35
    }

    #[tokio::test]
    async fn test_hybrid_storage_manager() {
        let schema = vec![ColumnSchema {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            nullable: false,
        }];

        let manager =
            HybridStorageManager::new("test".to_string(), schema, HybridStorageConfig::default());

        // Insert into hot tier
        manager.insert(vec![SqlValue::Integer(1)]).await.unwrap();

        // Point lookup
        let result = manager
            .execute(AccessPattern::PointLookup {
                key: SqlValue::Integer(1),
            })
            .await
            .unwrap();

        match result {
            QueryResult::Rows { rows, .. } => assert_eq!(rows.len(), 1),
            _ => panic!("Expected rows"),
        }
    }
}
