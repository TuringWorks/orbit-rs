//! Schema Analysis and Caching
//!
//! This module provides schema discovery, caching, and analysis capabilities
//! for intelligent SQL generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Schema Analyzer for discovering and caching database schemas
pub struct SchemaAnalyzer {
    /// Cached schema information
    schema_cache: Arc<RwLock<SchemaCache>>,
    /// Statistics collector
    #[allow(dead_code)]
    statistics_collector: StatisticsCollector,
    /// Optional storage backend for schema discovery
    storage: Option<Arc<dyn crate::protocols::common::storage::TableStorage>>,
}

impl SchemaAnalyzer {
    /// Create a new schema analyzer
    pub fn new() -> Self {
        Self {
            schema_cache: Arc::new(RwLock::new(SchemaCache::new())),
            statistics_collector: StatisticsCollector::new(),
            storage: None,
        }
    }

    /// Create a new schema analyzer with storage backend
    pub fn with_storage(storage: Arc<dyn crate::protocols::common::storage::TableStorage>) -> Self {
        Self {
            schema_cache: Arc::new(RwLock::new(SchemaCache::new())),
            statistics_collector: StatisticsCollector::new(),
            storage: Some(storage),
        }
    }

    /// Get schema for a table (from cache or discovery)
    pub async fn get_table_schema(&self, table_name: &str) -> Option<TableSchema> {
        // Check cache first
        {
            let cache = self.schema_cache.read().unwrap();
            if let Some(schema) = cache.tables.get(table_name) {
                // Check if cache is still valid (not expired)
                if cache.is_valid() {
                    return Some(schema.clone());
                }
            }
        }

        // Cache miss or expired - would discover from Orbit-RS here
        // For now, return None (would be populated by actual discovery)
        None
    }

    /// Update schema cache with discovered schema
    pub fn update_schema(&self, schema: TableSchema) {
        let mut cache = self.schema_cache.write().unwrap();
        let old_schema = cache.tables.insert(schema.name.clone(), schema.clone());
        cache.last_updated = SystemTime::now();

        // Log schema update
        if old_schema.is_some() {
            tracing::debug!("Updated schema for table: {}", schema.name);
        } else {
            tracing::debug!("Added new schema for table: {}", schema.name);
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, SystemTime) {
        let cache = self.schema_cache.read().unwrap();
        (cache.tables.len(), cache.last_updated)
    }

    /// Get all cached table schemas
    pub fn list_tables(&self) -> Vec<String> {
        let cache = self.schema_cache.read().unwrap();
        cache.tables.keys().cloned().collect()
    }

    /// Get column statistics for a table/column
    pub fn get_column_statistics(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Option<ColumnStatistics> {
        let cache = self.schema_cache.read().unwrap();
        cache
            .statistics
            .get(&format!("{}.{}", table_name, column_name))
            .cloned()
    }

    /// Discover schema for a table from Orbit-RS storage
    pub async fn discover_schema(&self, table_name: &str) -> Result<TableSchema, SchemaError> {
        if let Some(ref storage) = self.storage {
            // Query Orbit-RS storage for table schema
            let pg_schema = storage
                .get_table_schema(table_name)
                .await
                .map_err(|e| SchemaError::DiscoveryError(e.to_string()))?;

            if let Some(pg_schema) = pg_schema {
                // Convert PostgreSQL TableSchema to MCP TableSchema
                let mcp_schema = self.convert_pg_schema_to_mcp(pg_schema);

                // Update cache
                self.update_schema(mcp_schema.clone());

                Ok(mcp_schema)
            } else {
                Err(SchemaError::TableNotFound(table_name.to_string()))
            }
        } else {
            Err(SchemaError::DiscoveryNotImplemented)
        }
    }

    /// Refresh schema cache from Orbit-RS storage
    pub async fn refresh_cache(&self) -> Result<(), SchemaError> {
        if let Some(ref storage) = self.storage {
            // Query Orbit-RS for all table schemas
            let schemas = storage
                .list_table_schemas()
                .await
                .map_err(|e| SchemaError::DiscoveryError(e.to_string()))?;

            let mut cache = self.schema_cache.write().unwrap();
            for pg_schema in schemas {
                let mcp_schema = self.convert_pg_schema_to_mcp(pg_schema);
                cache.tables.insert(mcp_schema.name.clone(), mcp_schema);
            }
            cache.last_updated = SystemTime::now();
            Ok(())
        } else {
            let mut cache = self.schema_cache.write().unwrap();
            cache.last_updated = SystemTime::now();
            Ok(())
        }
    }

    /// Convert PostgreSQL TableSchema to MCP TableSchema
    fn convert_pg_schema_to_mcp(
        &self,
        pg_schema: crate::protocols::postgres_wire::sql::executor::TableSchema,
    ) -> TableSchema {
        let columns: Vec<ColumnInfo> = pg_schema
            .columns
            .iter()
            .map(|col| {
                // Convert SqlType to string representation
                let data_type_str = match &col.data_type {
                    crate::protocols::postgres_wire::sql::types::SqlType::Integer => {
                        "INTEGER".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::BigInt => {
                        "BIGINT".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Text => {
                        "TEXT".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Varchar(Some(n)) => {
                        format!("VARCHAR({})", n)
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Varchar(None) => {
                        "VARCHAR".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Boolean => {
                        "BOOLEAN".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Json => {
                        "JSON".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Jsonb => {
                        "JSONB".to_string()
                    }
                    crate::protocols::postgres_wire::sql::types::SqlType::Timestamp {
                        with_timezone,
                    } => {
                        if *with_timezone {
                            "TIMESTAMPTZ".to_string()
                        } else {
                            "TIMESTAMP".to_string()
                        }
                    }
                    _ => format!("{:?}", col.data_type), // Default: use debug format
                };

                // Check if column has NOT NULL constraint
                let has_not_null = col.constraints.iter().any(|c| c == "NOT NULL");
                let is_primary_key = col.constraints.iter().any(|c| c == "PRIMARY KEY");

                ColumnInfo {
                    name: col.name.clone(),
                    data_type: data_type_str,
                    nullable: !has_not_null,
                    default_value: col.default.as_ref().map(|v| format!("{:?}", v)),
                    is_primary_key,
                    is_indexed: false, // TODO: Check indexes
                }
            })
            .collect();

        TableSchema {
            name: pg_schema.name.clone(),
            columns,
            constraints: vec![], // TODO: Convert table constraints
            indexes: vec![],     // TODO: Convert indexes
            row_estimate: None,
            data_size_estimate: None,
        }
    }
}

impl Default for SchemaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema cache
#[derive(Debug, Clone)]
pub struct SchemaCache {
    /// Table schemas
    pub tables: HashMap<String, TableSchema>,
    /// Index information
    pub indexes: HashMap<String, IndexInfo>,
    /// Table relationships
    pub relationships: Vec<TableRelationship>,
    /// Column statistics
    pub statistics: HashMap<String, ColumnStatistics>,
    /// Last update time
    pub last_updated: SystemTime,
}

impl SchemaCache {
    /// Create a new schema cache
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            indexes: HashMap::new(),
            relationships: Vec::new(),
            statistics: HashMap::new(),
            last_updated: SystemTime::now(),
        }
    }

    /// Check if cache is still valid (not expired)
    pub fn is_valid(&self) -> bool {
        // Cache is valid for 5 minutes
        const CACHE_TTL_SECONDS: u64 = 300;

        self.last_updated
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|now| now.as_secs() - d.as_secs() < CACHE_TTL_SECONDS)
            })
            .unwrap_or(false)
    }
}

/// Table schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name
    pub name: String,
    /// Column information
    pub columns: Vec<ColumnInfo>,
    /// Constraints
    pub constraints: Vec<ConstraintInfo>,
    /// Indexes
    pub indexes: Vec<IndexInfo>,
    /// Estimated row count
    pub row_estimate: Option<u64>,
    /// Estimated data size in bytes
    pub data_size_estimate: Option<u64>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Whether column is nullable
    pub nullable: bool,
    /// Default value (if any)
    pub default_value: Option<String>,
    /// Whether column is a primary key
    pub is_primary_key: bool,
    /// Whether column is indexed
    pub is_indexed: bool,
}

/// Constraint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    /// Constraint name
    pub name: String,
    /// Constraint type
    pub constraint_type: ConstraintType,
    /// Columns involved
    pub columns: Vec<String>,
}

/// Constraint type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    PrimaryKey,
    ForeignKey {
        referenced_table: String,
        referenced_column: String,
    },
    Unique,
    Check {
        expression: String,
    },
    NotNull,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    /// Index name
    pub name: String,
    /// Table name
    pub table_name: String,
    /// Columns in index
    pub columns: Vec<String>,
    /// Index type
    pub index_type: IndexType,
    /// Whether index is unique
    pub is_unique: bool,
}

/// Index type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    BTree,
    Hash,
    Gist,
    Gin,
    Brin,
}

/// Table relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRelationship {
    /// Source table
    pub from_table: String,
    /// Source column
    pub from_column: String,
    /// Target table
    pub to_table: String,
    /// Target column
    pub to_column: String,
    /// Relationship type
    pub relationship_type: RelationshipType,
}

/// Relationship type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    OneToOne,
    OneToMany,
    ManyToMany,
}

/// Column statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Column name (fully qualified: table.column)
    pub column_name: String,
    /// Data type
    pub data_type: String,
    /// Null count
    pub null_count: u64,
    /// Distinct count
    pub distinct_count: Option<u64>,
    /// Minimum value (if numeric)
    pub min_value: Option<serde_json::Value>,
    /// Maximum value (if numeric)
    pub max_value: Option<serde_json::Value>,
    /// Most common values
    pub most_common_values: Vec<(serde_json::Value, u64)>,
    /// Histogram buckets (for numeric columns)
    pub histogram: Option<Vec<HistogramBucket>>,
}

/// Histogram bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    /// Lower bound
    pub lower: f64,
    /// Upper bound
    pub upper: f64,
    /// Count in bucket
    pub count: u64,
}

/// Statistics Collector
pub struct StatisticsCollector;

impl StatisticsCollector {
    /// Create a new statistics collector
    pub fn new() -> Self {
        Self
    }

    /// Collect statistics for a column
    pub async fn collect_column_statistics(
        &self,
        _table_name: &str,
        _column_name: &str,
    ) -> Result<ColumnStatistics, SchemaError> {
        // TODO: Query Orbit-RS for column statistics
        Err(SchemaError::StatisticsNotImplemented)
    }
}

impl Default for StatisticsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaError {
    /// Table not found
    TableNotFound(String),
    /// Column not found
    ColumnNotFound(String),
    /// Schema discovery not yet implemented
    DiscoveryNotImplemented,
    /// Schema discovery error
    DiscoveryError(String),
    /// Statistics collection not yet implemented
    StatisticsNotImplemented,
    /// Cache error
    CacheError(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::TableNotFound(name) => write!(f, "Table not found: {}", name),
            SchemaError::ColumnNotFound(name) => write!(f, "Column not found: {}", name),
            SchemaError::DiscoveryNotImplemented => {
                write!(f, "Schema discovery not yet implemented")
            }
            SchemaError::DiscoveryError(msg) => {
                write!(f, "Schema discovery error: {}", msg)
            }
            SchemaError::StatisticsNotImplemented => {
                write!(f, "Statistics collection not yet implemented")
            }
            SchemaError::CacheError(msg) => write!(f, "Cache error: {}", msg),
        }
    }
}

impl std::error::Error for SchemaError {}
