//! Unified storage backend for cross-protocol data sharing
//!
//! This module provides a `TableStorage` implementation that wraps `UnifiedStorageIntegration`,
//! enabling the unified cross-protocol storage to be used with existing SQL protocol servers.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::{
    executor::{ExtensionDefinition, IndexSchema, SchemaDefinition, TableSchema, ViewSchema},
    types::SqlValue,
};
use crate::unified_storage::UnifiedStorageIntegration;
use async_trait::async_trait;
use chrono::Timelike;
use orbit_engine::unified::{SqlAdapter, UniversalValue};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{StorageMetrics, StorageTransaction, TableStorage};

/// Unified table storage implementation that wraps UnifiedStorageIntegration
///
/// This adapter allows protocol servers (PostgreSQL, MySQL, etc.) to use the
/// unified cross-protocol storage system through the familiar `TableStorage` trait.
pub struct UnifiedTableStorage {
    /// Reference to the unified storage integration
    integration: Arc<UnifiedStorageIntegration>,
    /// SQL adapter for this storage instance
    sql_adapter: SqlAdapter,
    /// Protocol dialect (postgresql, mysql)
    dialect: String,
    /// In-memory schema cache (for fast lookups)
    table_schemas: RwLock<HashMap<String, TableSchema>>,
    index_schemas: RwLock<HashMap<String, IndexSchema>>,
    view_schemas: RwLock<HashMap<String, ViewSchema>>,
    schema_definitions: RwLock<HashMap<String, SchemaDefinition>>,
    extensions: RwLock<HashMap<String, ExtensionDefinition>>,
    settings: RwLock<HashMap<String, String>>,
    /// Metrics tracking
    read_ops: AtomicU64,
    write_ops: AtomicU64,
    delete_ops: AtomicU64,
}

impl UnifiedTableStorage {
    /// Create a new unified table storage for PostgreSQL
    pub fn postgres(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("postgresql");
        Self {
            integration,
            sql_adapter,
            dialect: "postgresql".to_string(),
            table_schemas: RwLock::new(HashMap::new()),
            index_schemas: RwLock::new(HashMap::new()),
            view_schemas: RwLock::new(HashMap::new()),
            schema_definitions: RwLock::new(HashMap::new()),
            extensions: RwLock::new(HashMap::new()),
            settings: RwLock::new(HashMap::new()),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            delete_ops: AtomicU64::new(0),
        }
    }

    /// Create a new unified table storage for MySQL
    pub fn mysql(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("mysql");
        Self {
            integration,
            sql_adapter,
            dialect: "mysql".to_string(),
            table_schemas: RwLock::new(HashMap::new()),
            index_schemas: RwLock::new(HashMap::new()),
            view_schemas: RwLock::new(HashMap::new()),
            schema_definitions: RwLock::new(HashMap::new()),
            extensions: RwLock::new(HashMap::new()),
            settings: RwLock::new(HashMap::new()),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            delete_ops: AtomicU64::new(0),
        }
    }

    /// Get the underlying integration
    pub fn integration(&self) -> &Arc<UnifiedStorageIntegration> {
        &self.integration
    }

    /// Get the dialect name
    pub fn dialect(&self) -> &str {
        &self.dialect
    }

    /// Convert SqlValue to UniversalValue
    fn sql_to_universal(value: &SqlValue) -> UniversalValue {
        match value {
            SqlValue::Null => UniversalValue::Null,
            SqlValue::Boolean(b) => UniversalValue::Bool(*b),
            SqlValue::SmallInt(i) => UniversalValue::Int(*i as i64),
            SqlValue::Integer(i) => UniversalValue::Int(*i as i64),
            SqlValue::BigInt(i) => UniversalValue::Int(*i),
            SqlValue::Decimal(d) => UniversalValue::Float(d.to_string().parse().unwrap_or(0.0)),
            SqlValue::Real(f) => UniversalValue::Float(*f as f64),
            SqlValue::DoublePrecision(f) => UniversalValue::Float(*f),
            SqlValue::Char(s) | SqlValue::Varchar(s) | SqlValue::Text(s) => {
                UniversalValue::String(s.clone())
            }
            SqlValue::Bytea(b) => UniversalValue::Bytes(b.clone()),
            SqlValue::Date(d) => {
                // Convert to days since epoch
                let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                let days = (*d - epoch).num_days() as i32;
                UniversalValue::Date(days)
            }
            SqlValue::Time(t) => {
                // Convert to nanoseconds since midnight using Timelike trait
                let secs = t.hour() as i64 * 3600 + t.minute() as i64 * 60 + t.second() as i64;
                let nanos = secs * 1_000_000_000 + t.nanosecond() as i64;
                UniversalValue::Time(nanos)
            }
            SqlValue::Timestamp(ts) => {
                UniversalValue::Timestamp(ts.and_utc().timestamp_millis())
            }
            SqlValue::TimestampWithTimezone(ts) => {
                UniversalValue::Timestamp(ts.timestamp_millis())
            }
            SqlValue::TimeWithTimezone(ts) => UniversalValue::Timestamp(ts.timestamp_millis()),
            SqlValue::Json(v) | SqlValue::Jsonb(v) => {
                // Convert JSON to UniversalValue
                Self::json_to_universal(v)
            }
            SqlValue::Array(arr) => {
                UniversalValue::List(arr.iter().map(Self::sql_to_universal).collect())
            }
            SqlValue::Composite(map) => {
                let btree: BTreeMap<String, UniversalValue> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::sql_to_universal(v)))
                    .collect();
                UniversalValue::Map(btree)
            }
            SqlValue::Uuid(u) => UniversalValue::Uuid(*u.as_bytes()),
            SqlValue::Point(x, y) => UniversalValue::Point { lat: *y, lon: *x },
            SqlValue::Polygon(points) => UniversalValue::Polygon(points.clone()),
            SqlValue::Vector(v) => UniversalValue::Vector(v.clone()),
            SqlValue::HalfVec(v) => UniversalValue::Vector(v.clone()),
            SqlValue::Inet(ip) => UniversalValue::String(ip.to_string()),
            SqlValue::Macaddr(mac) => {
                UniversalValue::String(format!(
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                ))
            }
            SqlValue::Macaddr8(mac) => {
                UniversalValue::String(format!(
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], mac[6], mac[7]
                ))
            }
            SqlValue::Xml(s) => UniversalValue::String(s.clone()),
            SqlValue::Interval(i) => {
                // Store as duration in microseconds
                let total_us = i.microseconds
                    + (i.days as i64 * 24 * 60 * 60 * 1_000_000)
                    + (i.months as i64 * 30 * 24 * 60 * 60 * 1_000_000);
                UniversalValue::Duration(total_us * 1000) // Convert to nanoseconds
            }
            SqlValue::Cidr(net) => {
                UniversalValue::String(format!("{}/{}", net.addr, net.prefix_len))
            }
            SqlValue::Range(range) => {
                let mut map = BTreeMap::new();
                if let Some(ref lower) = range.lower {
                    map.insert("lower".to_string(), Self::sql_to_universal(lower));
                }
                if let Some(ref upper) = range.upper {
                    map.insert("upper".to_string(), Self::sql_to_universal(upper));
                }
                map.insert(
                    "lower_inclusive".to_string(),
                    UniversalValue::Bool(range.lower_inclusive),
                );
                map.insert(
                    "upper_inclusive".to_string(),
                    UniversalValue::Bool(range.upper_inclusive),
                );
                UniversalValue::Map(map)
            }
            SqlValue::Line(a, b, c) => {
                let mut map = BTreeMap::new();
                map.insert("a".to_string(), UniversalValue::Float(*a));
                map.insert("b".to_string(), UniversalValue::Float(*b));
                map.insert("c".to_string(), UniversalValue::Float(*c));
                UniversalValue::Map(map)
            }
            SqlValue::Lseg(start, end) => {
                let mut map = BTreeMap::new();
                map.insert(
                    "start".to_string(),
                    UniversalValue::Point {
                        lat: start.1,
                        lon: start.0,
                    },
                );
                map.insert(
                    "end".to_string(),
                    UniversalValue::Point {
                        lat: end.1,
                        lon: end.0,
                    },
                );
                UniversalValue::Map(map)
            }
            SqlValue::Box(upper_right, lower_left) => UniversalValue::BoundingBox {
                min_lat: lower_left.1,
                min_lon: lower_left.0,
                max_lat: upper_right.1,
                max_lon: upper_right.0,
            },
            SqlValue::Path { points, open } => {
                let mut map = BTreeMap::new();
                map.insert(
                    "points".to_string(),
                    UniversalValue::Polygon(points.clone()),
                );
                map.insert("open".to_string(), UniversalValue::Bool(*open));
                UniversalValue::Map(map)
            }
            SqlValue::Circle { center, radius } => {
                let mut map = BTreeMap::new();
                map.insert(
                    "center".to_string(),
                    UniversalValue::Point {
                        lat: center.1,
                        lon: center.0,
                    },
                );
                map.insert("radius".to_string(), UniversalValue::Float(*radius));
                UniversalValue::Map(map)
            }
            SqlValue::Tsvector(elements) => {
                // Store as JSON-like structure
                let list: Vec<UniversalValue> = elements
                    .iter()
                    .map(|e| {
                        let mut map = BTreeMap::new();
                        map.insert("lexeme".to_string(), UniversalValue::String(e.lexeme.clone()));
                        map.insert(
                            "positions".to_string(),
                            UniversalValue::List(
                                e.positions
                                    .iter()
                                    .map(|p| UniversalValue::Int(*p as i64))
                                    .collect(),
                            ),
                        );
                        if let Some(w) = e.weight {
                            map.insert("weight".to_string(), UniversalValue::String(w.to_string()));
                        }
                        UniversalValue::Map(map)
                    })
                    .collect();
                UniversalValue::List(list)
            }
            SqlValue::Tsquery(s) => UniversalValue::String(s.clone()),
            SqlValue::SparseVec(pairs) => {
                let list: Vec<UniversalValue> = pairs
                    .iter()
                    .map(|(idx, val)| {
                        let mut map = BTreeMap::new();
                        map.insert("index".to_string(), UniversalValue::Int(*idx as i64));
                        map.insert("value".to_string(), UniversalValue::Float(*val as f64));
                        UniversalValue::Map(map)
                    })
                    .collect();
                UniversalValue::List(list)
            }
            SqlValue::Custom { type_name, data } => {
                let mut map = BTreeMap::new();
                map.insert(
                    "type_name".to_string(),
                    UniversalValue::String(type_name.clone()),
                );
                map.insert("data".to_string(), UniversalValue::Bytes(data.clone()));
                UniversalValue::Map(map)
            }
        }
    }

    /// Convert JSON Value to UniversalValue
    fn json_to_universal(value: &serde_json::Value) -> UniversalValue {
        match value {
            serde_json::Value::Null => UniversalValue::Null,
            serde_json::Value::Bool(b) => UniversalValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    UniversalValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    UniversalValue::Float(f)
                } else {
                    UniversalValue::String(n.to_string())
                }
            }
            serde_json::Value::String(s) => UniversalValue::String(s.clone()),
            serde_json::Value::Array(arr) => {
                UniversalValue::List(arr.iter().map(Self::json_to_universal).collect())
            }
            serde_json::Value::Object(obj) => {
                let btree: BTreeMap<String, UniversalValue> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_universal(v)))
                    .collect();
                UniversalValue::Map(btree)
            }
        }
    }

    /// Convert UniversalValue to SqlValue
    fn universal_to_sql(value: &UniversalValue) -> SqlValue {
        match value {
            UniversalValue::Null => SqlValue::Null,
            UniversalValue::Bool(b) => SqlValue::Boolean(*b),
            UniversalValue::Int(i) => SqlValue::BigInt(*i),
            UniversalValue::Float(f) => SqlValue::DoublePrecision(*f),
            UniversalValue::String(s) => SqlValue::Text(s.clone()),
            UniversalValue::Bytes(b) => SqlValue::Bytea(b.clone()),
            UniversalValue::List(list) => {
                SqlValue::Array(list.iter().map(Self::universal_to_sql).collect())
            }
            UniversalValue::Map(map) => {
                let hashmap: HashMap<String, SqlValue> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::universal_to_sql(v)))
                    .collect();
                SqlValue::Composite(hashmap)
            }
            UniversalValue::Set(set) => {
                SqlValue::Array(set.iter().map(Self::universal_to_sql).collect())
            }
            UniversalValue::SortedSet(ss) => {
                // Convert to JSON representation
                let json_arr: Vec<serde_json::Value> = ss
                    .iter()
                    .map(|(v, score)| {
                        serde_json::json!({
                            "value": Self::universal_to_json(v),
                            "score": score
                        })
                    })
                    .collect();
                SqlValue::Json(serde_json::Value::Array(json_arr))
            }
            UniversalValue::Timestamp(ts) => {
                let dt = chrono::DateTime::from_timestamp_millis(*ts)
                    .unwrap_or_default()
                    .naive_utc();
                SqlValue::Timestamp(dt)
            }
            UniversalValue::Date(days) => {
                let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(*days as i64);
                SqlValue::Date(date)
            }
            UniversalValue::Time(nanos) => {
                let secs = (*nanos / 1_000_000_000) as u32;
                let nano = (*nanos % 1_000_000_000) as u32;
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nano)
                    .unwrap_or_default();
                SqlValue::Time(time)
            }
            UniversalValue::Duration(nanos) => {
                let microseconds = *nanos / 1000;
                SqlValue::Interval(crate::protocols::postgres_wire::sql::types::PostgresInterval {
                    months: 0,
                    days: 0,
                    microseconds,
                })
            }
            UniversalValue::Node { id, labels, properties } => {
                let mut map = BTreeMap::new();
                map.insert("id".to_string(), UniversalValue::String(id.clone()));
                map.insert(
                    "labels".to_string(),
                    UniversalValue::List(
                        labels.iter().map(|l| UniversalValue::String(l.clone())).collect(),
                    ),
                );
                map.insert("properties".to_string(), UniversalValue::Map(properties.clone()));
                SqlValue::Json(Self::universal_to_json(&UniversalValue::Map(map)))
            }
            UniversalValue::Relationship { id, rel_type, start_node, end_node, properties } => {
                let mut map = BTreeMap::new();
                map.insert("id".to_string(), UniversalValue::String(id.clone()));
                map.insert("type".to_string(), UniversalValue::String(rel_type.clone()));
                map.insert("start_node".to_string(), UniversalValue::String(start_node.clone()));
                map.insert("end_node".to_string(), UniversalValue::String(end_node.clone()));
                map.insert("properties".to_string(), UniversalValue::Map(properties.clone()));
                SqlValue::Json(Self::universal_to_json(&UniversalValue::Map(map)))
            }
            UniversalValue::Path(path) => {
                SqlValue::Json(serde_json::Value::Array(
                    path.iter().map(Self::universal_to_json).collect(),
                ))
            }
            UniversalValue::Point { lat, lon } => SqlValue::Point(*lon, *lat),
            UniversalValue::Polygon(points) => SqlValue::Polygon(points.clone()),
            UniversalValue::BoundingBox { min_lat, min_lon, max_lat, max_lon } => {
                SqlValue::Box((*max_lon, *max_lat), (*min_lon, *min_lat))
            }
            UniversalValue::Vector(v) => SqlValue::Vector(v.clone()),
            UniversalValue::Uuid(bytes) => {
                SqlValue::Uuid(uuid::Uuid::from_bytes(*bytes))
            }
        }
    }

    /// Convert UniversalValue to serde_json::Value
    fn universal_to_json(value: &UniversalValue) -> serde_json::Value {
        match value {
            UniversalValue::Null => serde_json::Value::Null,
            UniversalValue::Bool(b) => serde_json::Value::Bool(*b),
            UniversalValue::Int(i) => serde_json::Value::Number((*i).into()),
            UniversalValue::Float(f) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap_or(0.into()))
            }
            UniversalValue::String(s) => serde_json::Value::String(s.clone()),
            UniversalValue::Bytes(b) => {
                serde_json::Value::String(base64::prelude::Engine::encode(
                    &base64::prelude::BASE64_STANDARD,
                    b,
                ))
            }
            UniversalValue::List(list) => {
                serde_json::Value::Array(list.iter().map(Self::universal_to_json).collect())
            }
            UniversalValue::Map(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::universal_to_json(v)))
                    .collect();
                serde_json::Value::Object(obj)
            }
            _ => {
                // For other types, convert to string representation
                serde_json::Value::String(format!("{:?}", value))
            }
        }
    }

    /// Convert HashMap<String, SqlValue> to BTreeMap<String, UniversalValue>
    fn sql_row_to_universal(row: &HashMap<String, SqlValue>) -> BTreeMap<String, UniversalValue> {
        row.iter()
            .map(|(k, v)| (k.clone(), Self::sql_to_universal(v)))
            .collect()
    }

    /// Convert BTreeMap<String, UniversalValue> to HashMap<String, SqlValue>
    fn universal_row_to_sql(row: &BTreeMap<String, UniversalValue>) -> HashMap<String, SqlValue> {
        row.iter()
            .map(|(k, v)| (k.clone(), Self::universal_to_sql(v)))
            .collect()
    }

    /// Get the primary key column name for a table
    async fn get_primary_key(&self, table_name: &str) -> Option<String> {
        let schemas = self.table_schemas.read().await;
        schemas.get(table_name).and_then(|schema| {
            // Check constraints for PRIMARY KEY or look for "id" column
            schema
                .columns
                .iter()
                .find(|c| c.constraints.iter().any(|constraint| {
                    constraint.to_uppercase().contains("PRIMARY")
                }))
                .map(|c| c.name.clone())
                .or_else(|| {
                    // Fall back to first column named "id" or first column
                    schema.columns.iter()
                        .find(|c| c.name.to_lowercase() == "id")
                        .or_else(|| schema.columns.first())
                        .map(|c| c.name.clone())
                })
        })
    }
}

#[async_trait]
impl TableStorage for UnifiedTableStorage {
    async fn initialize(&self) -> ProtocolResult<()> {
        // Storage is already initialized in UnifiedStorageIntegration
        Ok(())
    }

    async fn shutdown(&self) -> ProtocolResult<()> {
        // No special shutdown needed
        Ok(())
    }

    async fn metrics(&self) -> StorageMetrics {
        let unified_metrics = self.integration.metrics().await;
        StorageMetrics {
            read_operations: self.read_ops.load(Ordering::Relaxed),
            write_operations: self.write_ops.load(Ordering::Relaxed),
            delete_operations: self.delete_ops.load(Ordering::Relaxed),
            read_latency_avg: unified_metrics.read_latency_avg,
            write_latency_avg: unified_metrics.write_latency_avg,
            delete_latency_avg: unified_metrics.delete_latency_avg,
            error_count: unified_metrics.error_count,
            memory_usage_bytes: unified_metrics.memory_usage_bytes,
            disk_usage_bytes: 0, // Not tracked at unified level
            cache_hit_rate: 0.0, // Not tracked at unified level
            compaction_count: 0, // Not tracked at unified level
        }
    }

    async fn begin_transaction(&self) -> ProtocolResult<StorageTransaction> {
        // Create a new transaction context
        Ok(StorageTransaction {
            id: uuid::Uuid::new_v4().to_string(),
            isolation_level: "read_committed".to_string(),
            read_timestamp: std::time::Instant::now(),
            write_buffer: HashMap::new(),
        })
    }

    async fn commit_transaction(&self, _tx: &StorageTransaction) -> ProtocolResult<()> {
        // For now, operations are committed immediately
        // Full transaction support would require changes to UnifiedStorage
        Ok(())
    }

    async fn rollback_transaction(&self, _tx: &StorageTransaction) -> ProtocolResult<()> {
        // For now, this is a no-op since operations are immediate
        Ok(())
    }

    // Schema Operations

    async fn store_table_schema(
        &self,
        schema: &TableSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut schemas = self.table_schemas.write().await;
        schemas.insert(schema.name.clone(), schema.clone());
        Ok(())
    }

    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let schemas = self.table_schemas.read().await;
        Ok(schemas.get(table_name).cloned())
    }

    async fn list_table_schemas(&self) -> ProtocolResult<Vec<TableSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let schemas = self.table_schemas.read().await;
        Ok(schemas.values().cloned().collect())
    }

    async fn remove_table_schema(
        &self,
        table_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let mut schemas = self.table_schemas.write().await;
        Ok(schemas.remove(table_name).is_some())
    }

    // Data Operations

    async fn insert_row(
        &self,
        table_name: &str,
        row: &HashMap<String, SqlValue>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let universal_row = Self::sql_row_to_universal(row);
        let primary_key = self
            .get_primary_key(table_name)
            .await
            .unwrap_or_else(|| "id".to_string());

        self.sql_adapter
            .insert(table_name, universal_row, &primary_key)
            .await
            .map_err(|e| ProtocolError::Other(format!("Storage error: {}", e)))?;
        Ok(())
    }

    async fn insert_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, SqlValue>],
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        for row in rows {
            self.insert_row(table_name, row, tx).await?;
        }
        Ok(())
    }

    async fn get_table_data(
        &self,
        table_name: &str,
    ) -> ProtocolResult<Vec<HashMap<String, SqlValue>>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let rows = self
            .sql_adapter
            .select(table_name, None, None, None, None, None)
            .await
            .map_err(|e| ProtocolError::Other(format!("Storage error: {}", e)))?;

        Ok(rows.into_iter().map(|r| Self::universal_row_to_sql(&r)).collect())
    }

    async fn update_rows(
        &self,
        table_name: &str,
        updates: &HashMap<String, SqlValue>,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let universal_updates = Self::sql_row_to_universal(updates);

        // Note: condition filtering is not fully supported through SqlAdapter
        // For now, update all matching rows
        let count = self
            .sql_adapter
            .update(table_name, universal_updates, None)
            .await
            .map_err(|e| ProtocolError::Other(format!("Storage error: {}", e)))?;

        Ok(count as usize)
    }

    async fn delete_rows(
        &self,
        table_name: &str,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);

        // Note: condition filtering is not fully supported through SqlAdapter
        // For now, delete all matching rows
        let count = self
            .sql_adapter
            .delete(table_name, None)
            .await
            .map_err(|e| ProtocolError::Other(format!("Storage error: {}", e)))?;

        Ok(count as usize)
    }

    async fn truncate_table(
        &self,
        table_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.delete_rows(table_name, None, tx).await?;
        Ok(())
    }

    // Index Operations

    async fn store_index(
        &self,
        index: &IndexSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut indexes = self.index_schemas.write().await;
        indexes.insert(index.name.clone(), index.clone());
        Ok(())
    }

    async fn get_index(&self, index_name: &str) -> ProtocolResult<Option<IndexSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let indexes = self.index_schemas.read().await;
        Ok(indexes.get(index_name).cloned())
    }

    async fn list_table_indexes(&self, table_name: &str) -> ProtocolResult<Vec<IndexSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let indexes = self.index_schemas.read().await;
        Ok(indexes
            .values()
            .filter(|i| i.table == table_name)
            .cloned()
            .collect())
    }

    async fn remove_index(
        &self,
        index_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let mut indexes = self.index_schemas.write().await;
        Ok(indexes.remove(index_name).is_some())
    }

    // View Operations

    async fn store_view(
        &self,
        view: &ViewSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut views = self.view_schemas.write().await;
        views.insert(view.name.clone(), view.clone());
        Ok(())
    }

    async fn get_view(&self, view_name: &str) -> ProtocolResult<Option<ViewSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let views = self.view_schemas.read().await;
        Ok(views.get(view_name).cloned())
    }

    async fn list_views(&self) -> ProtocolResult<Vec<ViewSchema>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let views = self.view_schemas.read().await;
        Ok(views.values().cloned().collect())
    }

    async fn remove_view(
        &self,
        view_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let mut views = self.view_schemas.write().await;
        Ok(views.remove(view_name).is_some())
    }

    // Schema Operations

    async fn store_schema(
        &self,
        schema: &SchemaDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut schemas = self.schema_definitions.write().await;
        schemas.insert(schema.name.clone(), schema.clone());
        Ok(())
    }

    async fn get_schema(&self, schema_name: &str) -> ProtocolResult<Option<SchemaDefinition>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let schemas = self.schema_definitions.read().await;
        Ok(schemas.get(schema_name).cloned())
    }

    async fn list_schemas(&self) -> ProtocolResult<Vec<SchemaDefinition>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let schemas = self.schema_definitions.read().await;
        Ok(schemas.values().cloned().collect())
    }

    async fn remove_schema(
        &self,
        schema_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let mut schemas = self.schema_definitions.write().await;
        Ok(schemas.remove(schema_name).is_some())
    }

    // Extension Operations

    async fn store_extension(
        &self,
        extension: &ExtensionDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut extensions = self.extensions.write().await;
        extensions.insert(extension.name.clone(), extension.clone());
        Ok(())
    }

    async fn get_extension(
        &self,
        extension_name: &str,
    ) -> ProtocolResult<Option<ExtensionDefinition>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let extensions = self.extensions.read().await;
        Ok(extensions.get(extension_name).cloned())
    }

    async fn list_extensions(&self) -> ProtocolResult<Vec<ExtensionDefinition>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let extensions = self.extensions.read().await;
        Ok(extensions.values().cloned().collect())
    }

    async fn remove_extension(
        &self,
        extension_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let mut extensions = self.extensions.write().await;
        Ok(extensions.remove(extension_name).is_some())
    }

    // Configuration Operations

    async fn store_setting(
        &self,
        key: &str,
        value: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let mut settings = self.settings.write().await;
        settings.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> ProtocolResult<Option<String>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let settings = self.settings.read().await;
        Ok(settings.get(key).cloned())
    }

    async fn list_settings(&self) -> ProtocolResult<HashMap<String, String>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let settings = self.settings.read().await;
        Ok(settings.clone())
    }

    // Maintenance Operations

    async fn checkpoint(&self) -> ProtocolResult<()> {
        // No-op for unified storage (handled internally)
        Ok(())
    }

    async fn compact(&self) -> ProtocolResult<()> {
        // No-op for unified storage (handled internally)
        Ok(())
    }

    async fn storage_size(&self) -> ProtocolResult<u64> {
        let metrics = self.integration.metrics().await;
        Ok(metrics.memory_usage_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_storage::UnifiedStorageIntegrationConfig;

    #[tokio::test]
    async fn test_sql_to_universal_conversion() {
        // Test basic types
        assert_eq!(
            UnifiedTableStorage::sql_to_universal(&SqlValue::Null),
            UniversalValue::Null
        );
        assert_eq!(
            UnifiedTableStorage::sql_to_universal(&SqlValue::Boolean(true)),
            UniversalValue::Bool(true)
        );
        assert_eq!(
            UnifiedTableStorage::sql_to_universal(&SqlValue::BigInt(42)),
            UniversalValue::Int(42)
        );
        assert_eq!(
            UnifiedTableStorage::sql_to_universal(&SqlValue::DoublePrecision(3.14)),
            UniversalValue::Float(3.14)
        );
        assert_eq!(
            UnifiedTableStorage::sql_to_universal(&SqlValue::Text("hello".to_string())),
            UniversalValue::String("hello".to_string())
        );
    }

    #[tokio::test]
    async fn test_universal_to_sql_conversion() {
        // Test basic types
        assert_eq!(
            UnifiedTableStorage::universal_to_sql(&UniversalValue::Null),
            SqlValue::Null
        );
        assert_eq!(
            UnifiedTableStorage::universal_to_sql(&UniversalValue::Bool(true)),
            SqlValue::Boolean(true)
        );
        assert_eq!(
            UnifiedTableStorage::universal_to_sql(&UniversalValue::Int(42)),
            SqlValue::BigInt(42)
        );
        assert_eq!(
            UnifiedTableStorage::universal_to_sql(&UniversalValue::Float(3.14)),
            SqlValue::DoublePrecision(3.14)
        );
        assert_eq!(
            UnifiedTableStorage::universal_to_sql(&UniversalValue::String("hello".to_string())),
            SqlValue::Text("hello".to_string())
        );
    }

    #[tokio::test]
    async fn test_unified_table_storage_creation() {
        let config = UnifiedStorageIntegrationConfig {
            use_memory_backend: true,
            ..Default::default()
        };

        let integration = UnifiedStorageIntegration::with_config(config)
            .await
            .unwrap();
        let storage = UnifiedTableStorage::postgres(Arc::new(integration));

        assert_eq!(storage.dialect(), "postgresql");
    }
}
