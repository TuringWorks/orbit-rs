//! RocksDB-backed persistent storage for PostgreSQL query engine

//! This module provides a persistent storage backend for PostgreSQL tables using RocksDB.
//! It stores table schemas, data, and supports basic CRUD operations.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use async_trait::async_trait;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Column families for PostgreSQL data organization
const CF_TABLES: &str = "pg_tables"; // Table schemas
const CF_DATA: &str = "pg_data"; // Table data
const CF_INDEXES: &str = "pg_indexes"; // Index metadata (future use)
const CF_METADATA: &str = "pg_metadata"; // General metadata

/// Table schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDefinition>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub row_count: i64,
    /// Foreign keys declared on the table.
    ///
    /// Held here rather than per column so a composite key is one constraint
    /// checked as a whole, which is what `FOREIGN KEY (a, b) REFERENCES t(c, d)`
    /// means: the pair must match a row, not each column separately.
    #[serde(default)]
    pub foreign_keys: Vec<ForeignKey>,
}

/// A foreign key constraint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForeignKey {
    /// Columns of this table, in order.
    pub columns: Vec<String>,
    /// The table they refer to.
    pub table: String,
    /// Columns of that table, in the same order. Empty means its key.
    pub referenced: Vec<String>,
    /// What to do when a referenced row is deleted.
    pub on_delete: ReferentialAction,
    /// What to do when a referenced row's key changes.
    pub on_update: ReferentialAction,
    /// Whether the check may be postponed to the end of the transaction.
    pub deferrable: bool,
    /// How a partly-NULL key is treated.
    #[serde(default)]
    pub match_type: MatchType,
}

/// How a foreign key with some NULL columns is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchType {
    /// Any NULL part satisfies the constraint. PostgreSQL's default.
    #[default]
    Simple,
    /// Either every part is NULL or none is.
    Full,
    /// Non-NULL parts must match some row.
    Partial,
}

/// What a foreign key does when the row it refers to changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReferentialAction {
    /// Refuse the change while a row still refers to it.
    #[default]
    NoAction,
    /// Same as `NoAction` here: the check is not deferred.
    Restrict,
    /// Delete or update the referring rows too.
    Cascade,
    /// Clear the referring column.
    SetNull,
    /// Put the referring column back to its declared default.
    SetDefault,
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: ColumnType,
    pub nullable: bool,
    pub default_value: Option<JsonValue>,
    /// Whether the column is a primary key or is declared `UNIQUE`.
    ///
    /// Defaulted on read so schemas written before this field existed still
    /// deserialize; they simply carry no uniqueness constraint.
    #[serde(default)]
    pub unique: bool,
    /// The predicate of a `CHECK` constraint, as written.
    ///
    /// Kept as text because it is evaluated against each row at insert time by
    /// the same expression evaluator that runs a `WHERE` clause.
    #[serde(default)]
    pub check: Option<String>,
    /// The `table.column` a `REFERENCES` clause points at.
    #[serde(default)]
    pub references: Option<(String, String)>,
    /// The domain the column was declared with, if any.
    ///
    /// Kept so `ALTER DOMAIN` reaches tables that already use it: the check
    /// is read from the domain at write time rather than copied at
    /// `CREATE TABLE`.
    #[serde(default)]
    pub domain: Option<String>,
}

/// Supported column types (subset of PostgreSQL types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnType {
    Serial,       // Auto-incrementing integer
    Integer,      // 32-bit integer
    BigInt,       // 64-bit integer
    Text,         // Variable-length text
    Varchar(i32), // Variable-length text with limit
    Boolean,      // Boolean
    Json,         // JSON data
    Timestamp,    // Timestamp with timezone
    Double,       // Double precision float
    /// Exact decimal, with the precision and scale it was declared with.
    ///
    /// Without this a `NUMERIC(10,2)` column was stored as a `Double`: the
    /// declared scale was lost, so `10.50` read back as `10.5`, and a value
    /// that cannot be represented in binary floating point was rounded to one
    /// that can. `NUMERIC` exists to avoid exactly that.
    Numeric {
        /// Total digits, when declared.
        precision: Option<u8>,
        /// Digits after the point, when declared.
        scale: Option<u8>,
    },
}

/// Row data for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub values: HashMap<String, JsonValue>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Query condition for WHERE clauses
#[derive(Debug, Clone)]
pub struct QueryCondition {
    pub column: String,
    pub operator: String, // =, !=, <, >, LIKE, etc.
    pub value: JsonValue,
}

/// Trait for persistent table storage operations
#[async_trait]
pub trait PersistentTableStorage: Send + Sync {
    /// Create a new table with the given schema
    async fn create_table(&self, schema: TableSchema) -> ProtocolResult<()>;

    /// Drop an existing table
    async fn drop_table(&self, table_name: &str) -> ProtocolResult<()>;

    /// Check if a table exists
    async fn table_exists(&self, table_name: &str) -> ProtocolResult<bool>;

    /// Get table schema
    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>>;

    /// List all tables
    async fn list_tables(&self) -> ProtocolResult<Vec<String>>;

    /// Insert a row into a table
    async fn insert_row(&self, table_name: &str, row: TableRow) -> ProtocolResult<String>;

    /// Update rows in a table
    async fn update_rows(
        &self,
        table_name: &str,
        set_values: HashMap<String, JsonValue>,
        conditions: Vec<QueryCondition>,
    ) -> ProtocolResult<i64>;

    /// Delete rows from a table
    async fn delete_rows(
        &self,
        table_name: &str,
        conditions: Vec<QueryCondition>,
    ) -> ProtocolResult<i64>;

    /// Select rows from a table
    async fn select_rows(
        &self,
        table_name: &str,
        columns: Vec<String>, // Empty for SELECT *
        conditions: Vec<QueryCondition>,
        limit: Option<i64>,
    ) -> ProtocolResult<Vec<TableRow>>;
}

/// RocksDB implementation of PersistentTableStorage
pub struct RocksDbTableStorage {
    db: Arc<DB>,
    // Cache for table schemas to avoid frequent RocksDB reads
    schema_cache: Arc<RwLock<HashMap<String, TableSchema>>>,
}

impl RocksDbTableStorage {
    /// Create a new RocksDB table storage instance
    pub fn new<P: AsRef<Path>>(path: P) -> ProtocolResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define column families for PostgreSQL data organization
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_TABLES, Options::default()),
            ColumnFamilyDescriptor::new(CF_DATA, Options::default()),
            ColumnFamilyDescriptor::new(CF_INDEXES, Options::default()),
            ColumnFamilyDescriptor::new(CF_METADATA, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors).map_err(|e| {
            error!("Failed to open PostgreSQL RocksDB: {}", e);
            ProtocolError::PostgresError(format!("Failed to open persistent storage: {}", e))
        })?;

        let storage = Self {
            db: Arc::new(db),
            schema_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        info!("PostgreSQL RocksDB storage initialized successfully");
        Ok(storage)
    }

    /// Get column family handle
    fn get_cf(&self, cf_name: &str) -> ProtocolResult<&rocksdb::ColumnFamily> {
        self.db.cf_handle(cf_name).ok_or_else(|| {
            error!("Column family not found: {}", cf_name);
            ProtocolError::PostgresError(format!("Column family not found: {}", cf_name))
        })
    }

    /// Generate a unique row ID
    fn generate_row_id(&self, table_name: &str) -> String {
        format!("{}_{}", table_name, uuid::Uuid::new_v4().simple())
    }

    /// Create table key for schema storage
    fn table_schema_key(&self, table_name: &str) -> String {
        format!("schema:{}", table_name)
    }

    /// Create row key for data storage
    fn table_row_key(&self, table_name: &str, row_id: &str) -> String {
        format!("data:{}:{}", table_name, row_id)
    }

    /// Evaluate query condition against a row
    fn matches_condition(&self, row: &TableRow, condition: &QueryCondition) -> bool {
        // Try both original case and uppercase for compatibility
        let row_value = match row
            .values
            .get(&condition.column)
            .or_else(|| row.values.get(&condition.column.to_uppercase()))
        {
            Some(value) => value,
            None => return false, // Column doesn't exist in row
        };

        match condition.operator.as_str() {
            "=" => row_value == &condition.value,
            "!=" => row_value != &condition.value,
            "<" => self.compare_values(row_value, &condition.value) == std::cmp::Ordering::Less,
            "<=" => self.compare_values(row_value, &condition.value) != std::cmp::Ordering::Greater,
            ">" => self.compare_values(row_value, &condition.value) == std::cmp::Ordering::Greater,
            ">=" => self.compare_values(row_value, &condition.value) != std::cmp::Ordering::Less,
            "LIKE" => {
                // Simple LIKE implementation (case-insensitive contains)
                if let (JsonValue::String(row_str), JsonValue::String(pattern)) =
                    (row_value, &condition.value)
                {
                    let pattern = pattern.replace('%', "");
                    row_str.to_lowercase().contains(&pattern.to_lowercase())
                } else {
                    false
                }
            }
            _ => {
                debug!("Unsupported operator: {}", condition.operator);
                false
            }
        }
    }

    /// Compare two JSON values for ordering
    fn compare_values(&self, a: &JsonValue, b: &JsonValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            (JsonValue::Number(n1), JsonValue::Number(n2)) => n1
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&n2.as_f64().unwrap_or(0.0))
                .unwrap_or(Ordering::Equal),
            (JsonValue::String(s1), JsonValue::String(s2)) => s1.cmp(s2),
            (JsonValue::Bool(b1), JsonValue::Bool(b2)) => b1.cmp(b2),
            _ => Ordering::Equal, // For other types, consider equal
        }
    }

    /// Load all table schemas into cache
    #[allow(dead_code)] // Reserved for future schema caching optimization
    async fn load_schemas_to_cache(&self) -> ProtocolResult<()> {
        let tables_cf = self.get_cf(CF_TABLES)?;
        let mut cache = self.schema_cache.write().await;

        // Iterate through all schema keys
        let iter = self.db.iterator_cf(tables_cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item.map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to read schema: {}", e))
            })?;

            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with("schema:") {
                let table_name = key_str.strip_prefix("schema:").unwrap();
                let schema: TableSchema = serde_json::from_slice(&value).map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to deserialize schema: {}", e))
                })?;
                cache.insert(table_name.to_string(), schema);
            }
        }

        debug!("Loaded {} table schemas to cache", cache.len());
        Ok(())
    }
}

#[async_trait]
impl PersistentTableStorage for RocksDbTableStorage {
    async fn create_table(&self, schema: TableSchema) -> ProtocolResult<()> {
        let key = self.table_schema_key(&schema.name);

        // Check if table already exists
        if self.table_exists(&schema.name).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' already exists",
                schema.name
            )));
        }

        let tables_cf = self.get_cf(CF_TABLES)?;

        // Serialize and store schema
        let schema_json = serde_json::to_vec(&schema).map_err(|e| {
            ProtocolError::PostgresError(format!("Failed to serialize schema: {}", e))
        })?;

        self.db
            .put_cf(tables_cf, key.as_bytes(), &schema_json)
            .map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to store table schema: {}", e))
            })?;

        let table_name = schema.name.clone();

        // Update cache
        let mut cache = self.schema_cache.write().await;
        cache.insert(schema.name.clone(), schema);

        info!("Created table: {}", table_name);
        Ok(())
    }

    async fn drop_table(&self, table_name: &str) -> ProtocolResult<()> {
        // Check if table exists
        if !self.table_exists(table_name).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table_name
            )));
        }

        let tables_cf = self.get_cf(CF_TABLES)?;
        let data_cf = self.get_cf(CF_DATA)?;

        // Delete schema
        let schema_key = self.table_schema_key(table_name);
        self.db
            .delete_cf(tables_cf, schema_key.as_bytes())
            .map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to delete table schema: {}", e))
            })?;

        // Delete all data for this table
        let data_prefix = format!("data:{}:", table_name);
        let iter = self.db.iterator_cf(data_cf, rocksdb::IteratorMode::Start);
        let mut keys_to_delete = Vec::new();

        for item in iter {
            let (key, _) = item
                .map_err(|e| ProtocolError::PostgresError(format!("Failed to read data: {}", e)))?;

            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with(&data_prefix) {
                keys_to_delete.push(key.to_vec());
            }
        }

        // Delete all collected keys
        for key in keys_to_delete {
            self.db.delete_cf(data_cf, &key).map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to delete table data: {}", e))
            })?;
        }

        // Update cache
        let mut cache = self.schema_cache.write().await;
        cache.remove(table_name);

        info!("Dropped table: {}", table_name);
        Ok(())
    }

    async fn table_exists(&self, table_name: &str) -> ProtocolResult<bool> {
        // Check cache first
        {
            let cache = self.schema_cache.read().await;
            if cache.contains_key(table_name) {
                return Ok(true);
            }
        }

        // Check RocksDB
        let tables_cf = self.get_cf(CF_TABLES)?;
        let key = self.table_schema_key(table_name);

        match self.db.get_cf(tables_cf, key.as_bytes()) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(ProtocolError::PostgresError(format!(
                "Failed to check table existence: {}",
                e
            ))),
        }
    }

    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>> {
        // Check cache first
        {
            let cache = self.schema_cache.read().await;
            if let Some(schema) = cache.get(table_name) {
                return Ok(Some(schema.clone()));
            }
        }

        // Load from RocksDB
        let tables_cf = self.get_cf(CF_TABLES)?;
        let key = self.table_schema_key(table_name);

        match self.db.get_cf(tables_cf, key.as_bytes()) {
            Ok(Some(data)) => {
                let schema: TableSchema = serde_json::from_slice(&data).map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to deserialize schema: {}", e))
                })?;

                // Update cache
                let mut cache = self.schema_cache.write().await;
                cache.insert(table_name.to_string(), schema.clone());

                Ok(Some(schema))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ProtocolError::PostgresError(format!(
                "Failed to get table schema: {}",
                e
            ))),
        }
    }

    async fn list_tables(&self) -> ProtocolResult<Vec<String>> {
        let cache = self.schema_cache.read().await;
        Ok(cache.keys().cloned().collect())
    }

    async fn insert_row(&self, table_name: &str, row: TableRow) -> ProtocolResult<String> {
        // Verify table exists
        if !self.table_exists(table_name).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table_name
            )));
        }

        let data_cf = self.get_cf(CF_DATA)?;
        let row_id = self.generate_row_id(table_name);
        let key = self.table_row_key(table_name, &row_id);

        // Serialize row
        let row_json = serde_json::to_vec(&row)
            .map_err(|e| ProtocolError::PostgresError(format!("Failed to serialize row: {}", e)))?;

        // Store row
        self.db
            .put_cf(data_cf, key.as_bytes(), &row_json)
            .map_err(|e| ProtocolError::PostgresError(format!("Failed to store row: {}", e)))?;

        debug!("Inserted row {} into table {}", row_id, table_name);
        Ok(row_id)
    }

    async fn update_rows(
        &self,
        table_name: &str,
        set_values: HashMap<String, JsonValue>,
        conditions: Vec<QueryCondition>,
    ) -> ProtocolResult<i64> {
        // First, select rows that match conditions
        let matching_rows = self
            .select_rows(table_name, vec![], conditions, None)
            .await?;
        let mut updated_count = 0;

        let data_cf = self.get_cf(CF_DATA)?;
        let now = chrono::Utc::now();

        for row in matching_rows {
            // Update the row values
            let mut updated_row = row;
            for (column, value) in &set_values {
                updated_row.values.insert(column.clone(), value.clone());
            }
            updated_row.updated_at = now;

            // Generate key and store updated row
            let row_id = self.generate_row_id(table_name);
            let key = self.table_row_key(table_name, &row_id);

            let row_json = serde_json::to_vec(&updated_row).map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to serialize updated row: {}", e))
            })?;

            self.db
                .put_cf(data_cf, key.as_bytes(), &row_json)
                .map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to update row: {}", e))
                })?;

            updated_count += 1;
        }

        debug!("Updated {} rows in table {}", updated_count, table_name);
        Ok(updated_count)
    }

    async fn delete_rows(
        &self,
        table_name: &str,
        conditions: Vec<QueryCondition>,
    ) -> ProtocolResult<i64> {
        let data_cf = self.get_cf(CF_DATA)?;
        let data_prefix = format!("data:{}:", table_name);
        let iter = self.db.iterator_cf(data_cf, rocksdb::IteratorMode::Start);

        let mut keys_to_delete = Vec::new();
        let mut deleted_count = 0;

        for item in iter {
            let (key, value) = item
                .map_err(|e| ProtocolError::PostgresError(format!("Failed to read data: {}", e)))?;

            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with(&data_prefix) {
                // Deserialize row and check conditions
                let row: TableRow = serde_json::from_slice(&value).map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to deserialize row: {}", e))
                })?;

                // Check if row matches all conditions
                let matches = conditions.is_empty()
                    || conditions
                        .iter()
                        .all(|condition| self.matches_condition(&row, condition));

                if matches {
                    keys_to_delete.push(key.to_vec());
                    deleted_count += 1;
                }
            }
        }

        // Delete all matching keys
        for key in keys_to_delete {
            self.db.delete_cf(data_cf, &key).map_err(|e| {
                ProtocolError::PostgresError(format!("Failed to delete row: {}", e))
            })?;
        }

        debug!("Deleted {} rows from table {}", deleted_count, table_name);
        Ok(deleted_count)
    }

    async fn select_rows(
        &self,
        table_name: &str,
        columns: Vec<String>,
        conditions: Vec<QueryCondition>,
        limit: Option<i64>,
    ) -> ProtocolResult<Vec<TableRow>> {
        let data_cf = self.get_cf(CF_DATA)?;
        let data_prefix = format!("data:{}:", table_name);
        let iter = self.db.iterator_cf(data_cf, rocksdb::IteratorMode::Start);

        let mut matching_rows = Vec::new();
        let mut count = 0;

        for item in iter {
            if let Some(limit_val) = limit {
                if count >= limit_val {
                    break;
                }
            }

            let (key, value) = item
                .map_err(|e| ProtocolError::PostgresError(format!("Failed to read data: {}", e)))?;

            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with(&data_prefix) {
                // Deserialize row
                let mut row: TableRow = serde_json::from_slice(&value).map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to deserialize row: {}", e))
                })?;

                // Check conditions
                let matches = conditions.is_empty()
                    || conditions
                        .iter()
                        .all(|condition| self.matches_condition(&row, condition));

                if matches {
                    // Filter columns if specified
                    if !columns.is_empty() {
                        let mut filtered_values = HashMap::new();
                        for column in &columns {
                            // Try both original case and uppercase for compatibility
                            let value = row
                                .values
                                .get(column)
                                .or_else(|| row.values.get(&column.to_uppercase()))
                                .or_else(|| row.values.get(&column.to_lowercase()));
                            if let Some(value) = value {
                                filtered_values.insert(column.clone(), value.clone());
                            }
                        }
                        row.values = filtered_values;
                    }

                    matching_rows.push(row);
                    count += 1;
                }
            }
        }

        debug!(
            "Selected {} rows from table {}",
            matching_rows.len(),
            table_name
        );
        Ok(matching_rows)
    }
}

// Add uuid dependency
use uuid;
