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

    /// Create a new unified table storage for CQL (Cassandra Query Language)
    pub fn cql(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("cql");
        Self {
            integration,
            sql_adapter,
            dialect: "cql".to_string(),
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

    /// Create a new unified table storage for Redis key-value operations
    pub fn redis(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("redis");
        Self {
            integration,
            sql_adapter,
            dialect: "redis".to_string(),
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

    /// Create a new unified table storage for AQL (ArangoDB Query Language)
    pub fn aql(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("aql");
        Self {
            integration,
            sql_adapter,
            dialect: "aql".to_string(),
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

    /// Create a new unified table storage for Cypher (Neo4j graph queries)
    pub fn cypher(integration: Arc<UnifiedStorageIntegration>) -> Self {
        let sql_adapter = integration.sql_adapter("cypher");
        Self {
            integration,
            sql_adapter,
            dialect: "cypher".to_string(),
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
            SqlValue::Timestamp(ts) => UniversalValue::Timestamp(ts.and_utc().timestamp_millis()),
            SqlValue::TimestampWithTimezone(ts) => UniversalValue::Timestamp(ts.timestamp_millis()),
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
            SqlValue::Macaddr(mac) => UniversalValue::String(format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            )),
            SqlValue::Macaddr8(mac) => UniversalValue::String(format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], mac[6], mac[7]
            )),
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
                        map.insert(
                            "lexeme".to_string(),
                            UniversalValue::String(e.lexeme.clone()),
                        );
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

            // Object Identifier types - store as integers
            SqlValue::Oid(oid)
            | SqlValue::Regclass(oid)
            | SqlValue::Regcollation(oid)
            | SqlValue::Regconfig(oid)
            | SqlValue::Regdictionary(oid)
            | SqlValue::Regnamespace(oid)
            | SqlValue::Regoper(oid)
            | SqlValue::Regoperator(oid)
            | SqlValue::Regproc(oid)
            | SqlValue::Regprocedure(oid)
            | SqlValue::Regrole(oid)
            | SqlValue::Regtype(oid) => UniversalValue::Int(*oid as i64),

            // PostgreSQL-specific types
            SqlValue::PgLsn(lsn) => UniversalValue::Int(*lsn as i64),
            SqlValue::PgSnapshot(s) => UniversalValue::String(s.clone()),

            SqlValue::Custom { type_name, data } => {
                let mut map = BTreeMap::new();
                map.insert(
                    "type_name".to_string(),
                    UniversalValue::String(type_name.clone()),
                );
                map.insert("data".to_string(), UniversalValue::Bytes(data.clone()));
                UniversalValue::Map(map)
            }
            // Handle any other new variants as string/debug representation
            _ => UniversalValue::String(format!("{:?}", value)),
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
                SqlValue::Interval(
                    crate::protocols::postgres_wire::sql::types::PostgresInterval {
                        months: 0,
                        days: 0,
                        microseconds,
                    },
                )
            }
            UniversalValue::Node {
                id,
                labels,
                properties,
            } => {
                let mut map = BTreeMap::new();
                map.insert("id".to_string(), UniversalValue::String(id.clone()));
                map.insert(
                    "labels".to_string(),
                    UniversalValue::List(
                        labels
                            .iter()
                            .map(|l| UniversalValue::String(l.clone()))
                            .collect(),
                    ),
                );
                map.insert(
                    "properties".to_string(),
                    UniversalValue::Map(properties.clone()),
                );
                SqlValue::Json(Self::universal_to_json(&UniversalValue::Map(map)))
            }
            UniversalValue::Relationship {
                id,
                rel_type,
                start_node,
                end_node,
                properties,
            } => {
                let mut map = BTreeMap::new();
                map.insert("id".to_string(), UniversalValue::String(id.clone()));
                map.insert("type".to_string(), UniversalValue::String(rel_type.clone()));
                map.insert(
                    "start_node".to_string(),
                    UniversalValue::String(start_node.clone()),
                );
                map.insert(
                    "end_node".to_string(),
                    UniversalValue::String(end_node.clone()),
                );
                map.insert(
                    "properties".to_string(),
                    UniversalValue::Map(properties.clone()),
                );
                SqlValue::Json(Self::universal_to_json(&UniversalValue::Map(map)))
            }
            UniversalValue::Path(path) => SqlValue::Json(serde_json::Value::Array(
                path.iter().map(Self::universal_to_json).collect(),
            )),
            UniversalValue::Point { lat, lon } => SqlValue::Point(*lon, *lat),
            UniversalValue::Polygon(points) => SqlValue::Polygon(points.clone()),
            UniversalValue::BoundingBox {
                min_lat,
                min_lon,
                max_lat,
                max_lon,
            } => SqlValue::Box((*max_lon, *max_lat), (*min_lon, *min_lat)),
            UniversalValue::Vector(v) => SqlValue::Vector(v.clone()),
            UniversalValue::Uuid(bytes) => SqlValue::Uuid(uuid::Uuid::from_bytes(*bytes)),
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
            UniversalValue::Bytes(b) => serde_json::Value::String(base64::prelude::Engine::encode(
                &base64::prelude::BASE64_STANDARD,
                b,
            )),
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
                .find(|c| {
                    c.constraints
                        .iter()
                        .any(|constraint| constraint.to_uppercase().contains("PRIMARY"))
                })
                .map(|c| c.name.clone())
                .or_else(|| {
                    // Fall back to first column named "id" or first column
                    schema
                        .columns
                        .iter()
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

        Ok(rows
            .into_iter()
            .map(|r| Self::universal_row_to_sql(&r))
            .collect())
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

        // NOTE: Conditional deletion is NOT supported through this method due to
        // async_trait lifetime limitations with Fn trait objects.
        // Callers that need conditional deletion should use direct key-based deletion
        // through the underlying storage instead.
        // For now, this deletes ALL rows (same behavior as memory backend).
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
            UnifiedTableStorage::sql_to_universal(&SqlValue::DoublePrecision(std::f64::consts::PI)),
            UniversalValue::Float(std::f64::consts::PI)
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
            UnifiedTableStorage::universal_to_sql(&UniversalValue::Float(std::f64::consts::PI)),
            SqlValue::DoublePrecision(std::f64::consts::PI)
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

// =============================================================================
// PersistentTableStorage Implementation
// =============================================================================
// This implementation allows UnifiedTableStorage to be used with QueryEngine
// which requires the PersistentTableStorage trait for PostgreSQL operations.

#[cfg(feature = "storage-rocksdb")]
mod persistent_storage_impl {
    use super::*;
    use crate::protocols::postgres_wire::persistent_storage::{
        ColumnDefinition, ColumnType, PersistentTableStorage, QueryCondition, TableRow,
        TableSchema as PersistentTableSchema,
    };
    use crate::protocols::postgres_wire::sql::types::SqlType;
    use serde_json::Value as JsonValue;

    impl UnifiedTableStorage {
        /// Convert from PersistentTableStorage TableSchema to SQL executor TableSchema
        fn persistent_schema_to_sql_schema(schema: &PersistentTableSchema) -> TableSchema {
            use crate::protocols::postgres_wire::sql::executor::ColumnSchema;

            let columns = schema
                .columns
                .iter()
                .map(|col| {
                    let mut constraints = Vec::new();
                    let data_type = match col.data_type {
                        ColumnType::Serial => {
                            constraints.push("SERIAL".to_string());
                            SqlType::Integer
                        } // Serial is auto-incrementing integer
                        ColumnType::Integer => SqlType::Integer,
                        ColumnType::BigInt => SqlType::BigInt,
                        ColumnType::Text => SqlType::Text,
                        ColumnType::Varchar(n) => SqlType::Varchar(Some(n as u32)),
                        ColumnType::Boolean => SqlType::Boolean,
                        ColumnType::Json => SqlType::Json,
                        ColumnType::Double => SqlType::DoublePrecision,
                        ColumnType::Timestamp => SqlType::Timestamp {
                            with_timezone: false,
                        },
                    };
                    ColumnSchema {
                        name: col.name.clone(),
                        data_type,
                        nullable: col.nullable,
                        default: None,
                        constraints,
                        generated: None,
                    }
                })
                .collect();

            TableSchema {
                name: schema.name.clone(),
                columns,
                constraints: Vec::new(),
                indexes: Vec::new(),
            }
        }

        /// Convert from SQL executor TableSchema to PersistentTableStorage TableSchema
        fn sql_schema_to_persistent_schema(schema: &TableSchema) -> PersistentTableSchema {
            let columns = schema
                .columns
                .iter()
                .map(|col| {
                    let data_type = if col.constraints.contains(&"SERIAL".to_string()) {
                        ColumnType::Serial
                    } else {
                        match &col.data_type {
                            SqlType::Integer | SqlType::SmallInt => ColumnType::Integer,
                            SqlType::BigInt => ColumnType::BigInt,
                            SqlType::Text => ColumnType::Text,
                            SqlType::Varchar(Some(n)) => ColumnType::Varchar(*n as i32),
                            SqlType::Varchar(None) => ColumnType::Varchar(255),
                            SqlType::Boolean => ColumnType::Boolean,
                            SqlType::Json | SqlType::Jsonb => ColumnType::Json,
                            SqlType::Timestamp { .. } => ColumnType::Timestamp,
                            _ => ColumnType::Text, // Default fallback
                        }
                    };

                    ColumnDefinition {
                        name: col.name.clone(),
                        data_type,
                        nullable: col.nullable,
                        default_value: None,
                    }
                })
                .collect();

            PersistentTableSchema {
                name: schema.name.clone(),
                columns,
                created_at: chrono::Utc::now(),
                row_count: 0,
            }
        }

        /// Convert JsonValue to SqlValue for row data
        fn json_to_sql_value(value: &JsonValue) -> SqlValue {
            match value {
                JsonValue::Null => SqlValue::Null,
                JsonValue::Bool(b) => SqlValue::Boolean(*b),
                JsonValue::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        SqlValue::BigInt(i)
                    } else if let Some(f) = n.as_f64() {
                        SqlValue::DoublePrecision(f)
                    } else {
                        SqlValue::Text(n.to_string())
                    }
                }
                JsonValue::String(s) => SqlValue::Text(s.clone()),
                JsonValue::Array(_) | JsonValue::Object(_) => SqlValue::Json(value.clone()),
            }
        }

        /// Convert SqlValue to JsonValue for row data
        fn sql_value_to_json(value: &SqlValue) -> JsonValue {
            match value {
                SqlValue::Null => JsonValue::Null,
                SqlValue::Boolean(b) => JsonValue::Bool(*b),
                SqlValue::SmallInt(i) => JsonValue::Number((*i as i64).into()),
                SqlValue::Integer(i) => JsonValue::Number((*i as i64).into()),
                SqlValue::BigInt(i) => JsonValue::Number((*i).into()),
                SqlValue::Real(f) => serde_json::Number::from_f64(*f as f64)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null),
                SqlValue::DoublePrecision(f) => serde_json::Number::from_f64(*f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null),
                SqlValue::Text(s) => JsonValue::String(s.clone()),
                SqlValue::Varchar(s) => JsonValue::String(s.clone()),
                SqlValue::Char(s) => JsonValue::String(s.clone()),
                SqlValue::Json(v) | SqlValue::Jsonb(v) => v.clone(),
                _ => JsonValue::String(value.to_postgres_string()),
            }
        }
    }

    #[async_trait]
    impl PersistentTableStorage for UnifiedTableStorage {
        async fn create_table(&self, schema: PersistentTableSchema) -> ProtocolResult<()> {
            let sql_schema = Self::persistent_schema_to_sql_schema(&schema);
            self.store_table_schema(&sql_schema, None).await
        }

        async fn drop_table(&self, table_name: &str) -> ProtocolResult<()> {
            self.remove_table_schema(table_name, None).await?;
            Ok(())
        }

        async fn table_exists(&self, table_name: &str) -> ProtocolResult<bool> {
            let schema = TableStorage::get_table_schema(self, table_name).await?;
            Ok(schema.is_some())
        }

        async fn get_table_schema(
            &self,
            table_name: &str,
        ) -> ProtocolResult<Option<PersistentTableSchema>> {
            let schema = TableStorage::get_table_schema(self, table_name).await?;
            Ok(schema.map(|s| Self::sql_schema_to_persistent_schema(&s)))
        }

        async fn list_tables(&self) -> ProtocolResult<Vec<String>> {
            let schemas = self.list_table_schemas().await?;
            Ok(schemas.into_iter().map(|s| s.name).collect())
        }

        async fn insert_row(&self, table_name: &str, row: TableRow) -> ProtocolResult<String> {
            // Convert TableRow to HashMap<String, SqlValue>
            let sql_row: HashMap<String, SqlValue> = row
                .values
                .iter()
                .map(|(k, v)| (k.clone(), Self::json_to_sql_value(v)))
                .collect();

            TableStorage::insert_row(self, table_name, &sql_row, None).await?;
            self.write_ops.fetch_add(1, Ordering::Relaxed);

            // Generate a simple row ID (in real implementation, this would be from auto-increment)
            Ok(uuid::Uuid::new_v4().to_string())
        }

        async fn update_rows(
            &self,
            table_name: &str,
            set_values: HashMap<String, JsonValue>,
            conditions: Vec<QueryCondition>,
        ) -> ProtocolResult<i64> {
            // Convert set_values to SqlValue
            let updates: HashMap<String, SqlValue> = set_values
                .iter()
                .map(|(k, v)| (k.clone(), Self::json_to_sql_value(v)))
                .collect();

            // Create condition filter
            let condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>> =
                if conditions.is_empty() {
                    None
                } else {
                    let conds = conditions.clone();
                    Some(Box::new(move |row: &HashMap<String, SqlValue>| {
                        conds.iter().all(|cond| {
                            if let Some(row_value) = row.get(&cond.column) {
                                let cond_value = Self::json_to_sql_value(&cond.value);
                                match cond.operator.as_str() {
                                    "=" | "==" => row_value == &cond_value,
                                    "!=" | "<>" => row_value != &cond_value,
                                    _ => true, // Skip complex operators for now
                                }
                            } else {
                                false
                            }
                        })
                    }))
                };

            let count =
                TableStorage::update_rows(self, table_name, &updates, condition, None).await?;
            self.write_ops.fetch_add(1, Ordering::Relaxed);
            Ok(count as i64)
        }

        async fn delete_rows(
            &self,
            table_name: &str,
            conditions: Vec<QueryCondition>,
        ) -> ProtocolResult<i64> {
            // Create condition filter
            let condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>> =
                if conditions.is_empty() {
                    None
                } else {
                    let conds = conditions.clone();
                    Some(Box::new(move |row: &HashMap<String, SqlValue>| {
                        conds.iter().all(|cond| {
                            if let Some(row_value) = row.get(&cond.column) {
                                let cond_value = Self::json_to_sql_value(&cond.value);
                                match cond.operator.as_str() {
                                    "=" | "==" => row_value == &cond_value,
                                    "!=" | "<>" => row_value != &cond_value,
                                    _ => true,
                                }
                            } else {
                                false
                            }
                        })
                    }))
                };

            let count = TableStorage::delete_rows(self, table_name, condition, None).await?;
            self.delete_ops.fetch_add(1, Ordering::Relaxed);
            Ok(count as i64)
        }

        async fn select_rows(
            &self,
            table_name: &str,
            columns: Vec<String>,
            conditions: Vec<QueryCondition>,
            limit: Option<i64>,
        ) -> ProtocolResult<Vec<TableRow>> {
            self.read_ops.fetch_add(1, Ordering::Relaxed);

            // Get all data from the table
            let all_rows = self.get_table_data(table_name).await?;

            // Filter rows based on conditions
            let filtered: Vec<_> = all_rows
                .into_iter()
                .filter(|row| {
                    conditions.iter().all(|cond| {
                        if let Some(row_value) = row.get(&cond.column) {
                            let cond_value = Self::json_to_sql_value(&cond.value);
                            match cond.operator.as_str() {
                                "=" | "==" => row_value == &cond_value,
                                "!=" | "<>" => row_value != &cond_value,
                                "<" => {
                                    if let (SqlValue::BigInt(a), SqlValue::BigInt(b)) =
                                        (row_value, &cond_value)
                                    {
                                        a < b
                                    } else {
                                        false
                                    }
                                }
                                ">" => {
                                    if let (SqlValue::BigInt(a), SqlValue::BigInt(b)) =
                                        (row_value, &cond_value)
                                    {
                                        a > b
                                    } else {
                                        false
                                    }
                                }
                                _ => true,
                            }
                        } else {
                            conditions.is_empty()
                        }
                    })
                })
                .take(limit.unwrap_or(i64::MAX) as usize)
                .collect();

            // Convert to TableRow format with column projection
            let result = filtered
                .into_iter()
                .map(|row| {
                    let values: HashMap<String, JsonValue> = if columns.is_empty() {
                        // SELECT * - return all columns
                        row.iter()
                            .map(|(k, v)| (k.clone(), Self::sql_value_to_json(v)))
                            .collect()
                    } else {
                        // SELECT specific columns
                        columns
                            .iter()
                            .filter_map(|col| {
                                row.get(col)
                                    .map(|v| (col.clone(), Self::sql_value_to_json(v)))
                            })
                            .collect()
                    };

                    TableRow {
                        values,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    }
                })
                .collect();

            Ok(result)
        }
    }
}

// =============================================================================
// REDIS DATA PROVIDER IMPLEMENTATION
// =============================================================================
// This implementation allows UnifiedTableStorage to be used as a Redis data provider
// for key-value operations with TTL support.

#[cfg(feature = "storage-rocksdb")]
mod redis_provider_impl {
    use super::*;
    use crate::protocols::persistence::redis_data::{
        RedisDataMetrics, RedisDataProvider, RedisValue,
    };
    use orbit_shared::OrbitResult;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Internal table name for Redis key-value storage
    const REDIS_KV_TABLE: &str = "__redis_kv";

    /// Unified Redis data provider that implements RedisDataProvider trait
    /// using UnifiedTableStorage as the backing store.
    pub struct UnifiedRedisDataProvider {
        storage: Arc<UnifiedTableStorage>,
        metrics: tokio::sync::RwLock<RedisDataMetrics>,
    }

    impl UnifiedRedisDataProvider {
        /// Create a new unified Redis data provider
        pub fn new(storage: Arc<UnifiedTableStorage>) -> Self {
            Self {
                storage,
                metrics: tokio::sync::RwLock::new(RedisDataMetrics::default()),
            }
        }

        /// Ensure the internal Redis KV table exists
        async fn ensure_table_exists(&self) -> OrbitResult<()> {
            use crate::protocols::postgres_wire::sql::executor::ColumnSchema;
            use crate::protocols::postgres_wire::sql::types::SqlType;

            // Check if table exists
            let schemas = self.storage.table_schemas.read().await;
            if schemas.contains_key(REDIS_KV_TABLE) {
                return Ok(());
            }
            drop(schemas);

            // Create the table schema
            let schema = TableSchema {
                name: REDIS_KV_TABLE.to_string(),
                columns: vec![
                    ColumnSchema {
                        name: "key".to_string(),
                        data_type: SqlType::Text,
                        nullable: false,
                        default: None,
                        constraints: vec!["PRIMARY KEY".to_string()],
                        generated: None,
                    },
                    ColumnSchema {
                        name: "value".to_string(),
                        data_type: SqlType::Text,
                        nullable: false,
                        default: None,
                        constraints: vec![],
                        generated: None,
                    },
                    ColumnSchema {
                        name: "expiration".to_string(),
                        data_type: SqlType::BigInt,
                        nullable: true,
                        default: None,
                        constraints: vec![],
                        generated: None,
                    },
                ],
                indexes: vec![],
                constraints: vec![],
            };

            self.storage
                .store_table_schema(&schema, None)
                .await
                .map_err(|e| orbit_shared::OrbitError::storage(e.to_string()))?;

            Ok(())
        }

        fn current_timestamp() -> u64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }
    }

    #[async_trait]
    impl RedisDataProvider for UnifiedRedisDataProvider {
        async fn initialize(&self) -> OrbitResult<()> {
            self.ensure_table_exists().await
        }

        async fn shutdown(&self) -> OrbitResult<()> {
            Ok(())
        }

        async fn get(&self, key: &str) -> OrbitResult<Option<RedisValue>> {
            self.ensure_table_exists().await?;

            let mut metrics = self.metrics.write().await;
            metrics.get_operations += 1;
            drop(metrics);

            // Query the KV table
            let rows = self
                .storage
                .get_table_data(REDIS_KV_TABLE)
                .await
                .map_err(|e| orbit_shared::OrbitError::storage(e.to_string()))?;

            for row in rows {
                if let Some(SqlValue::Text(k)) = row.get("key") {
                    if k == key {
                        let value = match row.get("value") {
                            Some(SqlValue::Text(v)) => v.clone(),
                            _ => continue,
                        };

                        let expiration = match row.get("expiration") {
                            Some(SqlValue::BigInt(exp)) => Some(*exp as u64),
                            Some(SqlValue::Integer(exp)) => Some(*exp as u64),
                            Some(SqlValue::Null) | None => None,
                            _ => None,
                        };

                        let redis_value = RedisValue {
                            data: value,
                            expiration,
                        };

                        // Check if expired
                        if redis_value.is_expired() {
                            // Delete expired key
                            let _ = self.delete(key).await;
                            return Ok(None);
                        }

                        return Ok(Some(redis_value));
                    }
                }
            }

            Ok(None)
        }

        async fn set(&self, key: &str, value: RedisValue) -> OrbitResult<()> {
            self.ensure_table_exists().await?;

            let mut metrics = self.metrics.write().await;
            metrics.set_operations += 1;
            drop(metrics);

            // First delete existing key if present
            let _ = self.delete(key).await;

            // Insert new row
            let mut row = HashMap::new();
            row.insert("key".to_string(), SqlValue::Text(key.to_string()));
            row.insert("value".to_string(), SqlValue::Text(value.data));
            row.insert(
                "expiration".to_string(),
                match value.expiration {
                    Some(exp) => SqlValue::BigInt(exp as i64),
                    None => SqlValue::Null,
                },
            );

            self.storage
                .insert_row(REDIS_KV_TABLE, &row, None)
                .await
                .map_err(|e| orbit_shared::OrbitError::storage(e.to_string()))?;

            Ok(())
        }

        async fn delete(&self, key: &str) -> OrbitResult<bool> {
            self.ensure_table_exists().await?;

            let mut metrics = self.metrics.write().await;
            metrics.delete_operations += 1;
            drop(metrics);

            // Delete by key directly using the underlying storage
            let deleted = self
                .storage
                .integration
                .storage()
                .delete(REDIS_KV_TABLE, key)
                .await
                .is_ok();

            Ok(deleted)
        }

        async fn exists(&self, key: &str) -> OrbitResult<bool> {
            Ok(self.get(key).await?.is_some())
        }

        async fn mget(&self, keys: &[String]) -> OrbitResult<Vec<Option<RedisValue>>> {
            let mut results = Vec::with_capacity(keys.len());
            for key in keys {
                results.push(self.get(key).await?);
            }
            Ok(results)
        }

        async fn mset(&self, values: HashMap<String, RedisValue>) -> OrbitResult<()> {
            for (key, value) in values {
                self.set(&key, value).await?;
            }
            Ok(())
        }

        async fn keys(&self, pattern: &str) -> OrbitResult<Vec<String>> {
            self.ensure_table_exists().await?;

            let rows = self
                .storage
                .get_table_data(REDIS_KV_TABLE)
                .await
                .map_err(|e| orbit_shared::OrbitError::storage(e.to_string()))?;

            let mut result = Vec::new();
            let now = Self::current_timestamp();

            for row in rows {
                if let Some(SqlValue::Text(key)) = row.get("key") {
                    // Check expiration
                    let expired = match row.get("expiration") {
                        Some(SqlValue::BigInt(exp)) => now >= *exp as u64,
                        _ => false,
                    };

                    if !expired {
                        // Simple glob pattern matching
                        if pattern == "*" || key.contains(&pattern.replace('*', "")) {
                            result.push(key.clone());
                        }
                    }
                }
            }

            Ok(result)
        }

        async fn cleanup_expired(&self) -> OrbitResult<u64> {
            self.ensure_table_exists().await?;

            let now = Self::current_timestamp();

            // Fetch all rows and find expired ones
            let rows = self
                .storage
                .get_table_data(REDIS_KV_TABLE)
                .await
                .map_err(|e| orbit_shared::OrbitError::storage(e.to_string()))?;

            // Collect keys to delete
            let mut keys_to_delete = Vec::new();
            for row in &rows {
                if let Some(SqlValue::BigInt(exp)) = row.get("expiration") {
                    if now >= *exp as u64 {
                        if let Some(SqlValue::Text(key)) = row.get("key") {
                            keys_to_delete.push(key.clone());
                        }
                    }
                }
            }

            // Delete each expired key
            let count = keys_to_delete.len();
            for key in keys_to_delete {
                let _ = self
                    .storage
                    .integration
                    .storage()
                    .delete(REDIS_KV_TABLE, &key)
                    .await;
            }

            let mut metrics = self.metrics.write().await;
            metrics.expired_keys_cleaned += count as u64;

            Ok(count as u64)
        }

        async fn metrics(&self) -> OrbitResult<RedisDataMetrics> {
            let metrics = self.metrics.read().await;
            let keys = self.keys("*").await?;

            Ok(RedisDataMetrics {
                get_operations: metrics.get_operations,
                set_operations: metrics.set_operations,
                delete_operations: metrics.delete_operations,
                expired_keys_cleaned: metrics.expired_keys_cleaned,
                total_keys: keys.len(),
                keys_with_ttl: 0, // Would need to count separately
            })
        }

        async fn incr(&self, key: &str, delta: i64) -> OrbitResult<i64> {
            let current = self.get(key).await?;
            let new_value = match current {
                Some(v) => {
                    let num: i64 = v.data.parse().unwrap_or(0);
                    num + delta
                }
                None => delta,
            };

            self.set(key, RedisValue::new(new_value.to_string()))
                .await?;
            Ok(new_value)
        }

        async fn append(&self, key: &str, value: &str) -> OrbitResult<usize> {
            let current = self.get(key).await?;
            let new_value = match current {
                Some(v) => format!("{}{}", v.data, value),
                None => value.to_string(),
            };
            let len = new_value.len();
            self.set(key, RedisValue::new(new_value)).await?;
            Ok(len)
        }

        async fn strlen(&self, key: &str) -> OrbitResult<usize> {
            match self.get(key).await? {
                Some(v) => Ok(v.data.len()),
                None => Ok(0),
            }
        }

        async fn setnx(&self, key: &str, value: RedisValue) -> OrbitResult<bool> {
            if self.exists(key).await? {
                Ok(false)
            } else {
                self.set(key, value).await?;
                Ok(true)
            }
        }

        async fn getset(&self, key: &str, value: RedisValue) -> OrbitResult<Option<String>> {
            let old = self.get(key).await?.map(|v| v.data);
            self.set(key, value).await?;
            Ok(old)
        }
    }
}

#[cfg(feature = "storage-rocksdb")]
pub use redis_provider_impl::UnifiedRedisDataProvider;

// =============================================================================
// AQL STORAGE IMPLEMENTATION
// =============================================================================
// This provides a unified AQL storage wrapper that mimics AqlStorage API.

#[cfg(feature = "storage-rocksdb")]
mod aql_storage_impl {
    use super::*;
    use crate::protocols::aql::data_model::{AqlCollection, AqlDocument, AqlValue};

    /// Internal table names for AQL storage
    const AQL_COLLECTIONS_TABLE: &str = "__aql_collections";
    const AQL_DOCUMENTS_TABLE: &str = "__aql_documents";

    /// Unified AQL storage that provides AqlStorage-compatible API
    /// using UnifiedTableStorage as the backing store.
    pub struct UnifiedAqlStorage {
        storage: Arc<UnifiedTableStorage>,
    }

    impl UnifiedAqlStorage {
        /// Create a new unified AQL storage
        pub fn new(storage: Arc<UnifiedTableStorage>) -> Self {
            Self { storage }
        }

        /// Ensure the internal AQL tables exist
        async fn ensure_tables_exist(&self) -> ProtocolResult<()> {
            use crate::protocols::postgres_wire::sql::executor::ColumnSchema;
            use crate::protocols::postgres_wire::sql::types::SqlType;

            // Check and create collections table
            let schemas = self.storage.table_schemas.read().await;
            let has_collections = schemas.contains_key(AQL_COLLECTIONS_TABLE);
            let has_documents = schemas.contains_key(AQL_DOCUMENTS_TABLE);
            drop(schemas);

            if !has_collections {
                let schema = TableSchema {
                    name: AQL_COLLECTIONS_TABLE.to_string(),
                    columns: vec![
                        ColumnSchema {
                            name: "name".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec!["PRIMARY KEY".to_string()],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "data".to_string(),
                            data_type: SqlType::Json,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                    ],
                    indexes: vec![],
                    constraints: vec![],
                };
                self.storage.store_table_schema(&schema, None).await?;
            }

            if !has_documents {
                let schema = TableSchema {
                    name: AQL_DOCUMENTS_TABLE.to_string(),
                    columns: vec![
                        ColumnSchema {
                            name: "id".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec!["PRIMARY KEY".to_string()],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "collection".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "key".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "data".to_string(),
                            data_type: SqlType::Json,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                    ],
                    indexes: vec![],
                    constraints: vec![],
                };
                self.storage.store_table_schema(&schema, None).await?;
            }

            Ok(())
        }

        /// Initialize the AQL storage
        pub async fn initialize(&self) -> ProtocolResult<()> {
            self.ensure_tables_exist().await
        }

        /// Store a collection
        pub async fn store_collection(&self, collection: AqlCollection) -> ProtocolResult<()> {
            self.ensure_tables_exist().await?;

            let data = serde_json::to_value(&collection).map_err(|e| {
                ProtocolError::SerializationError(format!("Failed to serialize collection: {}", e))
            })?;

            let mut row = HashMap::new();
            row.insert("name".to_string(), SqlValue::Text(collection.name.clone()));
            row.insert("data".to_string(), SqlValue::Json(data));

            // Delete existing collection by name directly using the underlying storage
            // (don't use delete_rows which doesn't support conditions)
            let _ = self
                .storage
                .integration
                .storage()
                .delete(AQL_COLLECTIONS_TABLE, &collection.name)
                .await;

            self.storage
                .insert_row(AQL_COLLECTIONS_TABLE, &row, None)
                .await
        }

        /// Get a collection by name
        pub async fn get_collection(&self, name: &str) -> ProtocolResult<Option<AqlCollection>> {
            self.ensure_tables_exist().await?;

            let rows = self.storage.get_table_data(AQL_COLLECTIONS_TABLE).await?;

            for row in rows {
                if let Some(SqlValue::Text(n)) = row.get("name") {
                    if n == name {
                        // Handle both Json and Composite types (the latter comes from
                        // unified storage's round-trip conversion through UniversalValue::Map)
                        let data_value = match row.get("data") {
                            Some(SqlValue::Json(data)) => Some(data.clone()),
                            Some(SqlValue::Composite(map)) => {
                                // Convert Composite back to JSON
                                Some(Self::composite_to_json(map))
                            }
                            _ => None,
                        };

                        if let Some(data) = data_value {
                            let collection: AqlCollection =
                                serde_json::from_value(data).map_err(|e| {
                                    ProtocolError::SerializationError(format!(
                                        "Failed to deserialize collection: {}",
                                        e
                                    ))
                                })?;
                            return Ok(Some(collection));
                        }
                    }
                }
            }

            Ok(None)
        }

        /// Convert a SqlValue::Composite back to serde_json::Value
        fn composite_to_json(map: &HashMap<String, SqlValue>) -> serde_json::Value {
            let json_map: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), Self::sql_value_to_json(v)))
                .collect();
            serde_json::Value::Object(json_map)
        }

        /// Convert SqlValue to serde_json::Value
        fn sql_value_to_json(value: &SqlValue) -> serde_json::Value {
            match value {
                SqlValue::Null => serde_json::Value::Null,
                SqlValue::Boolean(b) => serde_json::Value::Bool(*b),
                SqlValue::SmallInt(i) => serde_json::Value::Number((*i as i64).into()),
                SqlValue::Integer(i) => serde_json::Value::Number((*i as i64).into()),
                SqlValue::BigInt(i) => serde_json::Value::Number((*i).into()),
                SqlValue::Real(f) => serde_json::Number::from_f64(*f as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                SqlValue::DoublePrecision(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                SqlValue::Char(s) | SqlValue::Varchar(s) | SqlValue::Text(s) => {
                    serde_json::Value::String(s.clone())
                }
                SqlValue::Json(v) | SqlValue::Jsonb(v) => v.clone(),
                SqlValue::Array(arr) => {
                    serde_json::Value::Array(arr.iter().map(Self::sql_value_to_json).collect())
                }
                SqlValue::Composite(map) => Self::composite_to_json(map),
                _ => serde_json::Value::Null, // For types that don't have a direct JSON equivalent
            }
        }

        /// Store a document
        pub async fn store_document(&self, doc: AqlDocument) -> ProtocolResult<()> {
            self.ensure_tables_exist().await?;

            let collection_name = doc.id.split('/').next().unwrap_or("default").to_string();

            let data = serde_json::to_value(&doc).map_err(|e| {
                ProtocolError::SerializationError(format!("Failed to serialize document: {}", e))
            })?;

            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Text(doc.id.clone()));
            row.insert("collection".to_string(), SqlValue::Text(collection_name));
            row.insert("key".to_string(), SqlValue::Text(doc.key.clone()));
            row.insert("data".to_string(), SqlValue::Json(data));

            // Delete existing document by key directly using the underlying storage
            // (don't use delete_rows which doesn't support conditions)
            let _ = self
                .storage
                .integration
                .storage()
                .delete(AQL_DOCUMENTS_TABLE, &doc.id)
                .await;

            self.storage
                .insert_row(AQL_DOCUMENTS_TABLE, &row, None)
                .await
        }

        /// Get a document by collection and key
        pub async fn get_document(
            &self,
            collection: &str,
            key: &str,
        ) -> ProtocolResult<Option<AqlDocument>> {
            self.ensure_tables_exist().await?;

            let rows = self.storage.get_table_data(AQL_DOCUMENTS_TABLE).await?;

            for row in rows {
                let matches_collection =
                    matches!(row.get("collection"), Some(SqlValue::Text(c)) if c == collection);
                let matches_key = matches!(row.get("key"), Some(SqlValue::Text(k)) if k == key);

                if matches_collection && matches_key {
                    // Handle both Json and Composite types (the latter comes from
                    // unified storage's round-trip conversion through UniversalValue::Map)
                    let data_value = match row.get("data") {
                        Some(SqlValue::Json(data)) => Some(data.clone()),
                        Some(SqlValue::Composite(map)) => {
                            // Convert Composite back to JSON
                            Some(Self::composite_to_json(map))
                        }
                        _ => None,
                    };

                    if let Some(data) = data_value {
                        let doc: AqlDocument = serde_json::from_value(data).map_err(|e| {
                            ProtocolError::SerializationError(format!(
                                "Failed to deserialize document: {}",
                                e
                            ))
                        })?;
                        return Ok(Some(doc));
                    }
                }
            }

            Ok(None)
        }

        /// Get all documents in a collection
        pub async fn get_collection_documents(
            &self,
            collection: &str,
        ) -> ProtocolResult<Vec<AqlDocument>> {
            self.ensure_tables_exist().await?;

            let rows = self.storage.get_table_data(AQL_DOCUMENTS_TABLE).await?;
            let mut result = Vec::new();

            for row in rows {
                if let Some(SqlValue::Text(c)) = row.get("collection") {
                    if c == collection {
                        // Handle both Json and Composite types
                        let data_value = match row.get("data") {
                            Some(SqlValue::Json(data)) => Some(data.clone()),
                            Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                            _ => None,
                        };

                        if let Some(data) = data_value {
                            if let Ok(doc) = serde_json::from_value::<AqlDocument>(data) {
                                result.push(doc);
                            }
                        }
                    }
                }
            }

            Ok(result)
        }

        /// Shutdown the storage
        pub async fn shutdown(&self) -> ProtocolResult<()> {
            Ok(())
        }

        /// Delete a document by collection and key
        pub async fn delete_document(&self, collection: &str, key: &str) -> ProtocolResult<bool> {
            self.ensure_tables_exist().await?;

            // Construct the document ID
            let doc_id = format!("{}/{}", collection, key);

            // Delete the document using the underlying storage
            let result = self
                .storage
                .integration
                .storage()
                .delete(AQL_DOCUMENTS_TABLE, &doc_id)
                .await;

            Ok(result.is_ok())
        }

        /// Update a document (merge with existing data)
        pub async fn update_document(
            &self,
            collection: &str,
            key: &str,
            updates: HashMap<String, AqlValue>,
        ) -> ProtocolResult<Option<AqlDocument>> {
            self.ensure_tables_exist().await?;

            // Get existing document
            if let Some(mut doc) = self.get_document(collection, key).await? {
                // Merge updates
                for (field, value) in updates {
                    doc.data.insert(field, value);
                }
                // Update revision
                doc.revision = format!("_{}", chrono::Utc::now().timestamp());

                // Store the updated document
                self.store_document(doc.clone()).await?;
                Ok(Some(doc))
            } else {
                Ok(None)
            }
        }

        /// Check if a document exists
        pub async fn document_exists(&self, collection: &str, key: &str) -> bool {
            if self.ensure_tables_exist().await.is_err() {
                return false;
            }

            self.get_document(collection, key)
                .await
                .map(|doc| doc.is_some())
                .unwrap_or(false)
        }
    }

    /// Implement AqlStorageProvider trait for UnifiedAqlStorage
    #[async_trait]
    impl crate::protocols::aql::AqlStorageProvider for UnifiedAqlStorage {
        async fn initialize(&self) -> ProtocolResult<()> {
            UnifiedAqlStorage::initialize(self).await
        }

        async fn store_collection(&self, collection: AqlCollection) -> ProtocolResult<()> {
            UnifiedAqlStorage::store_collection(self, collection).await
        }

        async fn get_collection(&self, name: &str) -> ProtocolResult<Option<AqlCollection>> {
            UnifiedAqlStorage::get_collection(self, name).await
        }

        async fn store_document(&self, doc: AqlDocument) -> ProtocolResult<()> {
            UnifiedAqlStorage::store_document(self, doc).await
        }

        async fn get_document(
            &self,
            collection: &str,
            key: &str,
        ) -> ProtocolResult<Option<AqlDocument>> {
            UnifiedAqlStorage::get_document(self, collection, key).await
        }

        async fn get_collection_documents(
            &self,
            collection: &str,
        ) -> ProtocolResult<Vec<AqlDocument>> {
            UnifiedAqlStorage::get_collection_documents(self, collection).await
        }

        async fn delete_document(&self, collection: &str, key: &str) -> ProtocolResult<bool> {
            UnifiedAqlStorage::delete_document(self, collection, key).await
        }

        async fn update_document(
            &self,
            collection: &str,
            key: &str,
            updates: HashMap<String, AqlValue>,
        ) -> ProtocolResult<Option<AqlDocument>> {
            UnifiedAqlStorage::update_document(self, collection, key, updates).await
        }

        async fn document_exists(&self, collection: &str, key: &str) -> bool {
            UnifiedAqlStorage::document_exists(self, collection, key).await
        }

        async fn shutdown(&self) -> ProtocolResult<()> {
            UnifiedAqlStorage::shutdown(self).await
        }
    }
}

#[cfg(feature = "storage-rocksdb")]
pub use aql_storage_impl::UnifiedAqlStorage;

// =============================================================================
// CYPHER GRAPH STORAGE IMPLEMENTATION
// =============================================================================
// This provides a unified Cypher storage wrapper that mimics CypherGraphStorage API.

#[cfg(feature = "storage-rocksdb")]
mod cypher_storage_impl {
    use super::*;
    use crate::protocols::cypher::types::{GraphNode, GraphRelationship};

    /// Internal table names for Cypher storage
    const CYPHER_NODES_TABLE: &str = "__cypher_nodes";
    const CYPHER_RELATIONSHIPS_TABLE: &str = "__cypher_relationships";

    /// Unified Cypher storage that provides CypherGraphStorage-compatible API
    /// using UnifiedTableStorage as the backing store.
    pub struct UnifiedCypherStorage {
        storage: Arc<UnifiedTableStorage>,
    }

    impl UnifiedCypherStorage {
        /// Create a new unified Cypher storage
        pub fn new(storage: Arc<UnifiedTableStorage>) -> Self {
            Self { storage }
        }

        /// Convert a SqlValue::Composite back to serde_json::Value
        /// This handles the case where JSON data goes through the memory backend
        /// and gets converted to UniversalValue::Map then back to SqlValue::Composite
        fn composite_to_json(map: &HashMap<String, SqlValue>) -> serde_json::Value {
            let json_map: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), Self::sql_value_to_json(v)))
                .collect();
            serde_json::Value::Object(json_map)
        }

        /// Convert SqlValue to serde_json::Value
        fn sql_value_to_json(value: &SqlValue) -> serde_json::Value {
            match value {
                SqlValue::Null => serde_json::Value::Null,
                SqlValue::Boolean(b) => serde_json::Value::Bool(*b),
                SqlValue::SmallInt(i) => serde_json::Value::Number((*i as i64).into()),
                SqlValue::Integer(i) => serde_json::Value::Number((*i as i64).into()),
                SqlValue::BigInt(i) => serde_json::Value::Number((*i).into()),
                SqlValue::Real(f) => serde_json::Number::from_f64(*f as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                SqlValue::DoublePrecision(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                SqlValue::Char(s) | SqlValue::Varchar(s) | SqlValue::Text(s) => {
                    serde_json::Value::String(s.clone())
                }
                SqlValue::Json(v) | SqlValue::Jsonb(v) => v.clone(),
                SqlValue::Array(arr) => {
                    serde_json::Value::Array(arr.iter().map(Self::sql_value_to_json).collect())
                }
                SqlValue::Composite(map) => Self::composite_to_json(map),
                _ => serde_json::Value::Null,
            }
        }

        /// Ensure the internal Cypher tables exist
        async fn ensure_tables_exist(&self) -> ProtocolResult<()> {
            use crate::protocols::postgres_wire::sql::executor::ColumnSchema;
            use crate::protocols::postgres_wire::sql::types::SqlType;

            let schemas = self.storage.table_schemas.read().await;
            let has_nodes = schemas.contains_key(CYPHER_NODES_TABLE);
            let has_relationships = schemas.contains_key(CYPHER_RELATIONSHIPS_TABLE);
            drop(schemas);

            if !has_nodes {
                let schema = TableSchema {
                    name: CYPHER_NODES_TABLE.to_string(),
                    columns: vec![
                        ColumnSchema {
                            name: "id".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec!["PRIMARY KEY".to_string()],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "labels".to_string(),
                            data_type: SqlType::Json,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "properties".to_string(),
                            data_type: SqlType::Json,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                    ],
                    indexes: vec![],
                    constraints: vec![],
                };
                self.storage.store_table_schema(&schema, None).await?;
            }

            if !has_relationships {
                let schema = TableSchema {
                    name: CYPHER_RELATIONSHIPS_TABLE.to_string(),
                    columns: vec![
                        ColumnSchema {
                            name: "id".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec!["PRIMARY KEY".to_string()],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "start_node".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "end_node".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "rel_type".to_string(),
                            data_type: SqlType::Text,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                        ColumnSchema {
                            name: "properties".to_string(),
                            data_type: SqlType::Json,
                            nullable: false,
                            default: None,
                            constraints: vec![],
                            generated: None,
                        },
                    ],
                    indexes: vec![],
                    constraints: vec![],
                };
                self.storage.store_table_schema(&schema, None).await?;
            }

            Ok(())
        }

        /// Initialize the Cypher storage
        pub async fn initialize(&self) -> ProtocolResult<()> {
            self.ensure_tables_exist().await
        }

        /// Store a node
        pub async fn store_node(&self, node: GraphNode) -> ProtocolResult<()> {
            self.ensure_tables_exist().await?;

            let labels = serde_json::to_value(&node.labels).map_err(|e| {
                ProtocolError::SerializationError(format!("Failed to serialize labels: {}", e))
            })?;

            let properties = serde_json::to_value(&node.properties).map_err(|e| {
                ProtocolError::SerializationError(format!("Failed to serialize properties: {}", e))
            })?;

            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Text(node.id.clone()));
            row.insert("labels".to_string(), SqlValue::Json(labels));
            row.insert("properties".to_string(), SqlValue::Json(properties));

            // Delete existing node by ID directly using the underlying storage
            // (don't use delete_rows which doesn't support conditions)
            let _ = self
                .storage
                .integration
                .storage()
                .delete(CYPHER_NODES_TABLE, &node.id)
                .await;

            self.storage
                .insert_row(CYPHER_NODES_TABLE, &row, None)
                .await
        }

        /// Get a node by ID
        pub async fn get_node(&self, node_id: &str) -> ProtocolResult<Option<GraphNode>> {
            self.ensure_tables_exist().await?;

            let rows = self.storage.get_table_data(CYPHER_NODES_TABLE).await?;

            for row in rows {
                if let Some(SqlValue::Text(id)) = row.get("id") {
                    if id == node_id {
                        // Handle SqlValue::Json, SqlValue::Composite, and SqlValue::Array (memory backend round-trip)
                        let labels_value = match row.get("labels") {
                            Some(SqlValue::Json(v)) => Some(v.clone()),
                            Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                            Some(SqlValue::Array(arr)) => {
                                // Convert SqlValue::Array back to JSON array
                                Some(serde_json::Value::Array(
                                    arr.iter().map(Self::sql_value_to_json).collect(),
                                ))
                            }
                            _ => None,
                        };
                        let labels: Vec<String> = labels_value
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default();

                        let properties_value = match row.get("properties") {
                            Some(SqlValue::Json(v)) => Some(v.clone()),
                            Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                            _ => None,
                        };
                        let properties: HashMap<String, serde_json::Value> = properties_value
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default();

                        return Ok(Some(GraphNode {
                            id: id.clone(),
                            labels,
                            properties,
                        }));
                    }
                }
            }

            Ok(None)
        }

        /// Get all nodes
        pub async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
            self.ensure_tables_exist().await?;

            let rows = self.storage.get_table_data(CYPHER_NODES_TABLE).await?;
            let mut result = Vec::new();

            for row in rows {
                if let Some(SqlValue::Text(id)) = row.get("id") {
                    // Handle SqlValue::Json, SqlValue::Composite, and SqlValue::Array (memory backend round-trip)
                    let labels_value = match row.get("labels") {
                        Some(SqlValue::Json(v)) => Some(v.clone()),
                        Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                        Some(SqlValue::Array(arr)) => {
                            // Convert SqlValue::Array back to JSON array
                            Some(serde_json::Value::Array(
                                arr.iter().map(Self::sql_value_to_json).collect(),
                            ))
                        }
                        _ => None,
                    };
                    let labels: Vec<String> = labels_value
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();

                    let properties_value = match row.get("properties") {
                        Some(SqlValue::Json(v)) => Some(v.clone()),
                        Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                        _ => None,
                    };
                    let properties: HashMap<String, serde_json::Value> = properties_value
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();

                    result.push(GraphNode {
                        id: id.clone(),
                        labels,
                        properties,
                    });
                }
            }

            Ok(result)
        }

        /// Store a relationship
        pub async fn store_relationship(&self, rel: GraphRelationship) -> ProtocolResult<()> {
            self.ensure_tables_exist().await?;

            let properties = serde_json::to_value(&rel.properties).map_err(|e| {
                ProtocolError::SerializationError(format!("Failed to serialize properties: {}", e))
            })?;

            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Text(rel.id.clone()));
            row.insert(
                "start_node".to_string(),
                SqlValue::Text(rel.start_node.clone()),
            );
            row.insert("end_node".to_string(), SqlValue::Text(rel.end_node.clone()));
            row.insert("rel_type".to_string(), SqlValue::Text(rel.rel_type.clone()));
            row.insert("properties".to_string(), SqlValue::Json(properties));

            // Delete existing relationship by ID directly using the underlying storage
            // (don't use delete_rows which doesn't support conditions)
            let _ = self
                .storage
                .integration
                .storage()
                .delete(CYPHER_RELATIONSHIPS_TABLE, &rel.id)
                .await;

            self.storage
                .insert_row(CYPHER_RELATIONSHIPS_TABLE, &row, None)
                .await
        }

        /// Get a relationship by ID
        pub async fn get_relationship(
            &self,
            rel_id: &str,
        ) -> ProtocolResult<Option<GraphRelationship>> {
            self.ensure_tables_exist().await?;

            let rows = self
                .storage
                .get_table_data(CYPHER_RELATIONSHIPS_TABLE)
                .await?;

            for row in rows {
                if let Some(SqlValue::Text(id)) = row.get("id") {
                    if id == rel_id {
                        let start_node = match row.get("start_node") {
                            Some(SqlValue::Text(s)) => s.clone(),
                            _ => continue,
                        };

                        let end_node = match row.get("end_node") {
                            Some(SqlValue::Text(s)) => s.clone(),
                            _ => continue,
                        };

                        let rel_type = match row.get("rel_type") {
                            Some(SqlValue::Text(s)) => s.clone(),
                            _ => continue,
                        };

                        // Handle both SqlValue::Json and SqlValue::Composite (memory backend round-trip)
                        let properties_value = match row.get("properties") {
                            Some(SqlValue::Json(v)) => Some(v.clone()),
                            Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                            _ => None,
                        };
                        let properties: HashMap<String, serde_json::Value> = properties_value
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default();

                        return Ok(Some(GraphRelationship {
                            id: id.clone(),
                            start_node,
                            end_node,
                            rel_type,
                            properties,
                        }));
                    }
                }
            }

            Ok(None)
        }

        /// Get all relationships
        pub async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
            self.ensure_tables_exist().await?;

            let rows = self
                .storage
                .get_table_data(CYPHER_RELATIONSHIPS_TABLE)
                .await?;
            let mut result = Vec::new();

            for row in rows {
                if let Some(SqlValue::Text(id)) = row.get("id") {
                    let start_node = match row.get("start_node") {
                        Some(SqlValue::Text(s)) => s.clone(),
                        _ => continue,
                    };

                    let end_node = match row.get("end_node") {
                        Some(SqlValue::Text(s)) => s.clone(),
                        _ => continue,
                    };

                    let rel_type = match row.get("rel_type") {
                        Some(SqlValue::Text(s)) => s.clone(),
                        _ => continue,
                    };

                    // Handle both SqlValue::Json and SqlValue::Composite (memory backend round-trip)
                    let properties_value = match row.get("properties") {
                        Some(SqlValue::Json(v)) => Some(v.clone()),
                        Some(SqlValue::Composite(map)) => Some(Self::composite_to_json(map)),
                        _ => None,
                    };
                    let properties: HashMap<String, serde_json::Value> = properties_value
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();

                    result.push(GraphRelationship {
                        id: id.clone(),
                        start_node,
                        end_node,
                        rel_type,
                        properties,
                    });
                }
            }

            Ok(result)
        }

        /// Shutdown the storage
        pub async fn shutdown(&self) -> ProtocolResult<()> {
            Ok(())
        }
    }

    /// Implement CypherStorageProvider trait for UnifiedCypherStorage
    #[async_trait]
    impl crate::protocols::cypher::CypherStorageProvider for UnifiedCypherStorage {
        async fn initialize(&self) -> ProtocolResult<()> {
            UnifiedCypherStorage::initialize(self).await
        }

        async fn store_node(&self, node: GraphNode) -> ProtocolResult<()> {
            UnifiedCypherStorage::store_node(self, node).await
        }

        async fn get_node(&self, node_id: &str) -> ProtocolResult<Option<GraphNode>> {
            UnifiedCypherStorage::get_node(self, node_id).await
        }

        async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
            UnifiedCypherStorage::get_all_nodes(self).await
        }

        async fn store_relationship(&self, rel: GraphRelationship) -> ProtocolResult<()> {
            UnifiedCypherStorage::store_relationship(self, rel).await
        }

        async fn get_relationship(
            &self,
            rel_id: &str,
        ) -> ProtocolResult<Option<GraphRelationship>> {
            UnifiedCypherStorage::get_relationship(self, rel_id).await
        }

        async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
            UnifiedCypherStorage::get_all_relationships(self).await
        }

        async fn shutdown(&self) -> ProtocolResult<()> {
            UnifiedCypherStorage::shutdown(self).await
        }
    }
}

#[cfg(feature = "storage-rocksdb")]
pub use cypher_storage_impl::UnifiedCypherStorage;

// =============================================================================
// TESTS FOR UNIFIED STORAGE PROVIDERS
// =============================================================================

#[cfg(test)]
#[cfg(feature = "storage-rocksdb")]
mod storage_provider_tests {
    use super::*;
    use crate::protocols::aql::data_model::{
        AqlCollection, AqlDocument, AqlValue, CollectionStatus, CollectionType,
    };
    use crate::protocols::aql::storage::AqlStorage;
    use crate::protocols::aql::AqlStorageProvider;
    use crate::protocols::cypher::storage::CypherGraphStorage;
    use crate::protocols::cypher::types::{GraphNode, GraphRelationship};
    use crate::protocols::cypher::CypherStorageProvider;
    use crate::unified_storage::{UnifiedStorageIntegration, UnifiedStorageIntegrationConfig};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Create a unified storage integration for testing
    async fn create_unified_storage() -> (Arc<UnifiedStorageIntegration>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = UnifiedStorageIntegrationConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            enable_ttl_expiration: false,
            ttl_check_interval_secs: 60,
            max_scan_limit: 1000,
            // Use memory backend for testing - this avoids RocksDB setup
            use_memory_backend: true,
        };
        let integration = UnifiedStorageIntegration::with_config(config)
            .await
            .expect("Failed to create unified storage");
        (Arc::new(integration), temp_dir)
    }

    /// Create isolated RocksDB AQL storage for testing
    async fn create_isolated_aql_storage() -> (Arc<AqlStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(AqlStorage::new(temp_dir.path()));
        storage.initialize().await.unwrap();
        (storage, temp_dir)
    }

    /// Create isolated RocksDB Cypher storage for testing
    async fn create_isolated_cypher_storage() -> (Arc<CypherGraphStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(CypherGraphStorage::new(temp_dir.path()));
        storage.initialize().await.unwrap();
        (storage, temp_dir)
    }

    // =========================================================================
    // AQL Storage Provider Tests - Testing trait-based storage switching
    // =========================================================================

    #[tokio::test]
    async fn test_aql_isolated_storage_basic_operations() {
        let (storage, _temp_dir) = create_isolated_aql_storage().await;
        let provider: Arc<dyn AqlStorageProvider> = storage;

        // Test collection operations
        let collection = AqlCollection {
            name: "test_users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };

        provider.store_collection(collection.clone()).await.unwrap();

        let retrieved = provider.get_collection("test_users").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test_users");

        // Test document operations
        let mut data = HashMap::new();
        data.insert("name".to_string(), AqlValue::String("Alice".to_string()));
        let doc = AqlDocument::new("test_users", "alice".to_string(), data);

        provider.store_document(doc.clone()).await.unwrap();

        let retrieved_doc = provider.get_document("test_users", "alice").await.unwrap();
        assert!(retrieved_doc.is_some());
        assert_eq!(retrieved_doc.unwrap().key, "alice");
    }

    #[tokio::test]
    async fn test_aql_unified_storage_basic_operations() {
        let (integration, _temp_dir) = create_unified_storage().await;

        // Create unified AQL storage
        let aql_storage = UnifiedTableStorage::aql(integration.clone());
        let unified_aql = Arc::new(UnifiedAqlStorage::new(Arc::new(aql_storage)));
        unified_aql.initialize().await.unwrap();

        let provider: Arc<dyn AqlStorageProvider> = unified_aql;

        // Test collection operations (same API as isolated storage)
        let collection = AqlCollection {
            name: "unified_users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };

        provider.store_collection(collection.clone()).await.unwrap();

        let retrieved = provider.get_collection("unified_users").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "unified_users");

        // Test document operations
        let mut data = HashMap::new();
        data.insert("name".to_string(), AqlValue::String("Bob".to_string()));
        let doc = AqlDocument::new("unified_users", "bob".to_string(), data);

        provider.store_document(doc.clone()).await.unwrap();

        let retrieved_doc = provider.get_document("unified_users", "bob").await.unwrap();
        assert!(retrieved_doc.is_some());
        assert_eq!(retrieved_doc.unwrap().key, "bob");
    }

    #[tokio::test]
    async fn test_aql_storage_mode_switching() {
        // First, use isolated storage
        let (isolated_storage, _temp_dir1) = create_isolated_aql_storage().await;
        let isolated_provider: Arc<dyn AqlStorageProvider> = isolated_storage;

        let collection = AqlCollection {
            name: "switch_test".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };

        isolated_provider
            .store_collection(collection.clone())
            .await
            .unwrap();
        let mut data = HashMap::new();
        data.insert(
            "value".to_string(),
            AqlValue::String("isolated".to_string()),
        );
        let doc = AqlDocument::new("switch_test", "key1".to_string(), data);
        isolated_provider.store_document(doc).await.unwrap();

        // Verify data in isolated storage
        let retrieved = isolated_provider
            .get_document("switch_test", "key1")
            .await
            .unwrap();
        assert!(retrieved.is_some());

        // Now, use unified storage (separate storage - data won't transfer)
        let (integration, _temp_dir2) = create_unified_storage().await;
        let aql_storage = UnifiedTableStorage::aql(integration.clone());
        let unified_aql = Arc::new(UnifiedAqlStorage::new(Arc::new(aql_storage)));
        unified_aql.initialize().await.unwrap();
        let unified_provider: Arc<dyn AqlStorageProvider> = unified_aql;

        // Unified storage starts fresh - no data from isolated
        let retrieved_unified = unified_provider
            .get_document("switch_test", "key1")
            .await
            .unwrap();
        assert!(retrieved_unified.is_none());

        // Store new data in unified storage
        let mut data2 = HashMap::new();
        data2.insert("value".to_string(), AqlValue::String("unified".to_string()));
        let doc2 = AqlDocument::new("switch_test", "key2".to_string(), data2);
        unified_provider
            .store_collection(collection.clone())
            .await
            .unwrap();
        unified_provider.store_document(doc2).await.unwrap();

        // Verify unified storage has its own data
        let retrieved_unified2 = unified_provider
            .get_document("switch_test", "key2")
            .await
            .unwrap();
        assert!(retrieved_unified2.is_some());

        // Original isolated storage still has its data
        let original_data = isolated_provider
            .get_document("switch_test", "key1")
            .await
            .unwrap();
        assert!(original_data.is_some());
    }

    // =========================================================================
    // Cypher Storage Provider Tests - Testing trait-based storage switching
    // =========================================================================

    #[tokio::test]
    async fn test_cypher_isolated_storage_basic_operations() {
        let (storage, _temp_dir) = create_isolated_cypher_storage().await;
        let provider: Arc<dyn CypherStorageProvider> = storage;

        // Test node operations
        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Alice"));

        let node = GraphNode {
            id: "user:1".to_string(),
            labels: vec!["User".to_string()],
            properties: props,
        };

        provider.store_node(node.clone()).await.unwrap();

        let retrieved = provider.get_node("user:1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_node = retrieved.unwrap();
        assert_eq!(retrieved_node.id, "user:1");
        assert_eq!(retrieved_node.labels, vec!["User"]);

        // Test relationship operations
        let mut rel_props = HashMap::new();
        rel_props.insert("since".to_string(), serde_json::json!("2024"));

        let rel = GraphRelationship {
            id: "rel:1".to_string(),
            start_node: "user:1".to_string(),
            end_node: "user:2".to_string(),
            rel_type: "KNOWS".to_string(),
            properties: rel_props,
        };

        provider.store_relationship(rel.clone()).await.unwrap();

        let retrieved_rel = provider.get_relationship("rel:1").await.unwrap();
        assert!(retrieved_rel.is_some());
        assert_eq!(retrieved_rel.unwrap().rel_type, "KNOWS");
    }

    #[tokio::test]
    async fn test_cypher_unified_storage_basic_operations() {
        let (integration, _temp_dir) = create_unified_storage().await;

        // Create unified Cypher storage
        let cypher_storage = UnifiedTableStorage::cypher(integration.clone());
        let unified_cypher = Arc::new(UnifiedCypherStorage::new(Arc::new(cypher_storage)));
        unified_cypher.initialize().await.unwrap();

        let provider: Arc<dyn CypherStorageProvider> = unified_cypher;

        // Test node operations (same API as isolated storage)
        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Bob"));

        let node = GraphNode {
            id: "unified_user:1".to_string(),
            labels: vec!["Person".to_string()],
            properties: props,
        };

        provider.store_node(node.clone()).await.unwrap();

        let retrieved = provider.get_node("unified_user:1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_node = retrieved.unwrap();
        assert_eq!(retrieved_node.id, "unified_user:1");
        assert_eq!(retrieved_node.labels, vec!["Person"]);

        // Test get_all_nodes
        let all_nodes = provider.get_all_nodes().await.unwrap();
        assert!(!all_nodes.is_empty());
    }

    #[tokio::test]
    async fn test_cypher_storage_mode_switching() {
        // First, use isolated storage
        let (isolated_storage, _temp_dir1) = create_isolated_cypher_storage().await;
        let isolated_provider: Arc<dyn CypherStorageProvider> = isolated_storage;

        let mut props = HashMap::new();
        props.insert("storage_type".to_string(), serde_json::json!("isolated"));

        let node = GraphNode {
            id: "switch_node:1".to_string(),
            labels: vec!["TestNode".to_string()],
            properties: props,
        };

        isolated_provider.store_node(node.clone()).await.unwrap();

        // Verify data in isolated storage
        let retrieved = isolated_provider.get_node("switch_node:1").await.unwrap();
        assert!(retrieved.is_some());

        // Now switch to unified storage (separate storage - data won't transfer)
        let (integration, _temp_dir2) = create_unified_storage().await;
        let cypher_storage = UnifiedTableStorage::cypher(integration.clone());
        let unified_cypher = Arc::new(UnifiedCypherStorage::new(Arc::new(cypher_storage)));
        unified_cypher.initialize().await.unwrap();
        let unified_provider: Arc<dyn CypherStorageProvider> = unified_cypher;

        // Unified storage starts fresh
        let retrieved_unified = unified_provider.get_node("switch_node:1").await.unwrap();
        assert!(retrieved_unified.is_none());

        // Store new data in unified storage
        let mut props2 = HashMap::new();
        props2.insert("storage_type".to_string(), serde_json::json!("unified"));

        let node2 = GraphNode {
            id: "switch_node:2".to_string(),
            labels: vec!["UnifiedNode".to_string()],
            properties: props2,
        };

        unified_provider.store_node(node2).await.unwrap();

        // Verify unified storage has its own data
        let retrieved_unified2 = unified_provider.get_node("switch_node:2").await.unwrap();
        assert!(retrieved_unified2.is_some());

        // Original isolated storage still has its data
        let original_data = isolated_provider.get_node("switch_node:1").await.unwrap();
        assert!(original_data.is_some());
    }

    // =========================================================================
    // Cross-Protocol Tests - Verify data isolation or sharing as configured
    // =========================================================================

    #[tokio::test]
    async fn test_unified_storage_cross_protocol_isolation() {
        // Create a single unified storage integration
        let (integration, _temp_dir) = create_unified_storage().await;

        // Create AQL storage using this integration
        let aql_storage = UnifiedTableStorage::aql(integration.clone());
        let unified_aql = Arc::new(UnifiedAqlStorage::new(Arc::new(aql_storage)));
        unified_aql.initialize().await.unwrap();

        // Create Cypher storage using the SAME integration
        let cypher_storage = UnifiedTableStorage::cypher(integration.clone());
        let unified_cypher = Arc::new(UnifiedCypherStorage::new(Arc::new(cypher_storage)));
        unified_cypher.initialize().await.unwrap();

        // Store data in AQL
        let collection = AqlCollection {
            name: "cross_test".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        unified_aql.store_collection(collection).await.unwrap();

        let mut aql_data = HashMap::new();
        aql_data.insert(
            "source".to_string(),
            AqlValue::String("aql_protocol".to_string()),
        );
        let aql_doc = AqlDocument::new("cross_test", "doc1".to_string(), aql_data);
        unified_aql.store_document(aql_doc).await.unwrap();

        // Store data in Cypher
        let mut cypher_props = HashMap::new();
        cypher_props.insert("source".to_string(), serde_json::json!("cypher_protocol"));

        let cypher_node = GraphNode {
            id: "cross_node:1".to_string(),
            labels: vec!["CrossTest".to_string()],
            properties: cypher_props,
        };
        unified_cypher.store_node(cypher_node).await.unwrap();

        // Verify AQL data
        let aql_retrieved = unified_aql
            .get_document("cross_test", "doc1")
            .await
            .unwrap();
        assert!(aql_retrieved.is_some());

        // Verify Cypher data
        let cypher_retrieved = unified_cypher.get_node("cross_node:1").await.unwrap();
        assert!(cypher_retrieved.is_some());

        // Each protocol maintains its own namespace
        // AQL data is NOT directly visible in Cypher and vice versa
        // (they use different internal tables: __aql_* vs __cypher_*)
        let cypher_nodes = unified_cypher.get_all_nodes().await.unwrap();
        assert_eq!(cypher_nodes.len(), 1); // Only the Cypher node, not AQL document
    }

    #[tokio::test]
    async fn test_aql_provider_get_collection_documents() {
        let (integration, _temp_dir) = create_unified_storage().await;
        let aql_storage = UnifiedTableStorage::aql(integration.clone());
        let unified_aql = Arc::new(UnifiedAqlStorage::new(Arc::new(aql_storage)));
        unified_aql.initialize().await.unwrap();

        let provider: Arc<dyn AqlStorageProvider> = unified_aql;

        // Create collection
        let collection = AqlCollection {
            name: "docs_test".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        provider.store_collection(collection).await.unwrap();

        // Store multiple documents
        for i in 0..5 {
            let mut data = HashMap::new();
            data.insert(
                "index".to_string(),
                AqlValue::Number(serde_json::Number::from(i)),
            );
            let doc = AqlDocument::new("docs_test", format!("doc{}", i), data);
            provider.store_document(doc).await.unwrap();
        }

        // Verify all documents retrieved
        let all_docs = provider
            .get_collection_documents("docs_test")
            .await
            .unwrap();
        assert_eq!(all_docs.len(), 5);
    }

    #[tokio::test]
    async fn test_cypher_provider_get_all_relationships() {
        let (integration, _temp_dir) = create_unified_storage().await;
        let cypher_storage = UnifiedTableStorage::cypher(integration.clone());
        let unified_cypher = Arc::new(UnifiedCypherStorage::new(Arc::new(cypher_storage)));
        unified_cypher.initialize().await.unwrap();

        let provider: Arc<dyn CypherStorageProvider> = unified_cypher;

        // Create nodes
        for i in 0..3 {
            let node = GraphNode {
                id: format!("rel_test_node:{}", i),
                labels: vec!["Node".to_string()],
                properties: HashMap::new(),
            };
            provider.store_node(node).await.unwrap();
        }

        // Create relationships
        for i in 0..2 {
            let rel = GraphRelationship {
                id: format!("rel_test:{}", i),
                start_node: format!("rel_test_node:{}", i),
                end_node: format!("rel_test_node:{}", i + 1),
                rel_type: "CONNECTS".to_string(),
                properties: HashMap::new(),
            };
            provider.store_relationship(rel).await.unwrap();
        }

        // Verify all relationships retrieved
        let all_rels = provider.get_all_relationships().await.unwrap();
        assert_eq!(all_rels.len(), 2);
    }
}
