//! Universal data types for cross-protocol storage
//!
//! This module defines the canonical data representation that bridges all protocol paradigms,
//! enabling data written via one protocol (Redis, PostgreSQL, MySQL, CQL, Cypher, AQL, REST)
//! to be accessible through all other protocols.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Unique identifier for any record in the unified storage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId {
    /// Namespace/collection/table name (e.g., "users", "orders", "cache")
    pub namespace: String,
    /// Primary key/identifier within the namespace
    pub key: String,
}

impl RecordId {
    /// Create a new record ID
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            key: key.into(),
        }
    }

    /// Create a storage key for RocksDB
    pub fn to_storage_key(&self) -> String {
        format!("{}:{}", self.namespace, self.key)
    }

    /// Parse a storage key back to RecordId
    pub fn from_storage_key(key: &str) -> Option<Self> {
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(Self {
                namespace: parts[0].to_string(),
                key: parts[1].to_string(),
            })
        } else {
            None
        }
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.key)
    }
}

/// Universal value type that all protocols can map to/from
///
/// This enum represents all possible value types across different database paradigms:
/// - Key-value (Redis): String, List, Set, Hash
/// - Relational (PostgreSQL, MySQL): Scalar types, NULL
/// - Wide-column (CQL): Same as relational with collections
/// - Graph (Cypher, AQL): Nodes, Relationships, Paths
/// - Document (REST): Nested JSON structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UniversalValue {
    // Primitive types
    /// Null/None value
    Null,
    /// Boolean value
    Bool(bool),
    /// 64-bit signed integer
    Int(i64),
    /// 64-bit floating point
    Float(f64),
    /// UTF-8 string
    String(String),
    /// Binary data
    Bytes(Vec<u8>),

    // Collection types
    /// Ordered list of values
    List(Vec<UniversalValue>),
    /// Key-value map (sorted for deterministic serialization)
    Map(BTreeMap<String, UniversalValue>),
    /// Unordered set of unique values (stored as sorted list for determinism)
    Set(Vec<UniversalValue>),
    /// Sorted set with scores (value, score)
    SortedSet(Vec<(UniversalValue, f64)>),

    // Temporal types
    /// Unix timestamp in milliseconds
    Timestamp(i64),
    /// Date as days since epoch (1970-01-01)
    Date(i32),
    /// Time as nanoseconds since midnight
    Time(i64),
    /// Duration in nanoseconds
    Duration(i64),

    // Graph types (for Cypher/AQL)
    /// Graph node with labels and properties
    Node {
        id: String,
        labels: Vec<String>,
        properties: BTreeMap<String, UniversalValue>,
    },
    /// Graph relationship/edge
    Relationship {
        id: String,
        rel_type: String,
        start_node: String,
        end_node: String,
        properties: BTreeMap<String, UniversalValue>,
    },
    /// Graph path (alternating nodes and relationships)
    Path(Vec<UniversalValue>),

    // Geospatial types
    /// 2D point (latitude, longitude)
    Point { lat: f64, lon: f64 },
    /// Polygon as list of (lat, lon) points
    Polygon(Vec<(f64, f64)>),
    /// Geographic bounding box
    BoundingBox {
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    },

    // AI/ML types
    /// Vector embedding (for similarity search)
    Vector(Vec<f32>),

    // UUID type
    /// UUID as 128-bit value
    Uuid([u8; 16]),
}

impl UniversalValue {
    /// Check if the value is null
    pub fn is_null(&self) -> bool {
        matches!(self, UniversalValue::Null)
    }

    /// Try to get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            UniversalValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i64
    pub fn as_int(&self) -> Option<i64> {
        match self {
            UniversalValue::Int(i) => Some(*i),
            UniversalValue::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to get as f64
    pub fn as_float(&self) -> Option<f64> {
        match self {
            UniversalValue::Float(f) => Some(*f),
            UniversalValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get as string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            UniversalValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as bytes reference
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            UniversalValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Try to get as list reference
    pub fn as_list(&self) -> Option<&Vec<UniversalValue>> {
        match self {
            UniversalValue::List(l) => Some(l),
            _ => None,
        }
    }

    /// Try to get as map reference
    pub fn as_map(&self) -> Option<&BTreeMap<String, UniversalValue>> {
        match self {
            UniversalValue::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Try to get as vector embedding reference
    pub fn as_vector(&self) -> Option<&[f32]> {
        match self {
            UniversalValue::Vector(v) => Some(v),
            _ => None,
        }
    }

    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            UniversalValue::Null => "null",
            UniversalValue::Bool(_) => "bool",
            UniversalValue::Int(_) => "int",
            UniversalValue::Float(_) => "float",
            UniversalValue::String(_) => "string",
            UniversalValue::Bytes(_) => "bytes",
            UniversalValue::List(_) => "list",
            UniversalValue::Map(_) => "map",
            UniversalValue::Set(_) => "set",
            UniversalValue::SortedSet(_) => "sorted_set",
            UniversalValue::Timestamp(_) => "timestamp",
            UniversalValue::Date(_) => "date",
            UniversalValue::Time(_) => "time",
            UniversalValue::Duration(_) => "duration",
            UniversalValue::Node { .. } => "node",
            UniversalValue::Relationship { .. } => "relationship",
            UniversalValue::Path(_) => "path",
            UniversalValue::Point { .. } => "point",
            UniversalValue::Polygon(_) => "polygon",
            UniversalValue::BoundingBox { .. } => "bounding_box",
            UniversalValue::Vector(_) => "vector",
            UniversalValue::Uuid(_) => "uuid",
        }
    }

    /// Convert to a Map value (for record storage)
    pub fn into_map(self) -> BTreeMap<String, UniversalValue> {
        match self {
            UniversalValue::Map(m) => m,
            other => {
                let mut map = BTreeMap::new();
                map.insert("value".to_string(), other);
                map
            }
        }
    }
}

impl Default for UniversalValue {
    fn default() -> Self {
        UniversalValue::Null
    }
}

impl From<bool> for UniversalValue {
    fn from(v: bool) -> Self {
        UniversalValue::Bool(v)
    }
}

impl From<i64> for UniversalValue {
    fn from(v: i64) -> Self {
        UniversalValue::Int(v)
    }
}

impl From<i32> for UniversalValue {
    fn from(v: i32) -> Self {
        UniversalValue::Int(v as i64)
    }
}

impl From<f64> for UniversalValue {
    fn from(v: f64) -> Self {
        UniversalValue::Float(v)
    }
}

impl From<String> for UniversalValue {
    fn from(v: String) -> Self {
        UniversalValue::String(v)
    }
}

impl From<&str> for UniversalValue {
    fn from(v: &str) -> Self {
        UniversalValue::String(v.to_string())
    }
}

impl From<Vec<u8>> for UniversalValue {
    fn from(v: Vec<u8>) -> Self {
        UniversalValue::Bytes(v)
    }
}

impl From<Vec<UniversalValue>> for UniversalValue {
    fn from(v: Vec<UniversalValue>) -> Self {
        UniversalValue::List(v)
    }
}

impl From<BTreeMap<String, UniversalValue>> for UniversalValue {
    fn from(v: BTreeMap<String, UniversalValue>) -> Self {
        UniversalValue::Map(v)
    }
}

impl From<Vec<f32>> for UniversalValue {
    fn from(v: Vec<f32>) -> Self {
        UniversalValue::Vector(v)
    }
}

/// Metadata associated with a universal record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMetadata {
    /// When the record was created (Unix timestamp millis)
    pub created_at: i64,
    /// When the record was last updated (Unix timestamp millis)
    pub updated_at: i64,
    /// Version number for optimistic concurrency control
    pub version: u64,
    /// Optional TTL expiration timestamp (Unix timestamp millis)
    pub ttl: Option<i64>,
    /// Which protocol originally created this record
    pub source_protocol: String,
    /// Schema version (for migrations)
    pub schema_version: Option<u32>,
}

impl RecordMetadata {
    /// Create new metadata with current timestamp
    pub fn new(source_protocol: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            created_at: now,
            updated_at: now,
            version: 1,
            ttl: None,
            source_protocol: source_protocol.into(),
            schema_version: None,
        }
    }

    /// Create metadata with TTL
    pub fn with_ttl(source_protocol: impl Into<String>, ttl_ms: i64) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            created_at: now,
            updated_at: now,
            version: 1,
            ttl: Some(now + ttl_ms),
            source_protocol: source_protocol.into(),
            schema_version: None,
        }
    }

    /// Check if the record has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            chrono::Utc::now().timestamp_millis() > ttl
        } else {
            false
        }
    }

    /// Increment version for update
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp_millis();
        self.version += 1;
    }
}

impl Default for RecordMetadata {
    fn default() -> Self {
        Self::new("unknown")
    }
}

/// A universal record that can be stored and retrieved by any protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalRecord {
    /// Unique identifier
    pub id: RecordId,
    /// The actual data
    pub value: UniversalValue,
    /// Record metadata
    pub metadata: RecordMetadata,
}

impl UniversalRecord {
    /// Create a new record
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: UniversalValue,
        source_protocol: impl Into<String>,
    ) -> Self {
        Self {
            id: RecordId::new(namespace, key),
            value,
            metadata: RecordMetadata::new(source_protocol),
        }
    }

    /// Create a new record with TTL
    pub fn with_ttl(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: UniversalValue,
        source_protocol: impl Into<String>,
        ttl_ms: i64,
    ) -> Self {
        Self {
            id: RecordId::new(namespace, key),
            value,
            metadata: RecordMetadata::with_ttl(source_protocol, ttl_ms),
        }
    }

    /// Check if the record has expired
    pub fn is_expired(&self) -> bool {
        self.metadata.is_expired()
    }

    /// Get a field from the record if it's a Map
    pub fn get_field(&self, field: &str) -> Option<&UniversalValue> {
        match &self.value {
            UniversalValue::Map(m) => m.get(field),
            _ => None,
        }
    }

    /// Set a field in the record (only if it's a Map)
    pub fn set_field(&mut self, field: impl Into<String>, value: UniversalValue) {
        if let UniversalValue::Map(ref mut m) = self.value {
            m.insert(field.into(), value);
            self.metadata.touch();
        }
    }
}

/// Result type for universal storage operations
#[derive(Debug, Clone)]
pub enum UniversalResult {
    /// Operation succeeded with no return value
    Ok,
    /// Operation returned a single record
    Record(UniversalRecord),
    /// Operation returned multiple records
    Records(Vec<UniversalRecord>),
    /// Operation returned a single value (for GET on simple types)
    Value(UniversalValue),
    /// Operation returned multiple values
    Values(Vec<UniversalValue>),
    /// Operation returned a count
    Count(u64),
    /// Operation returned nothing (not found)
    Empty,
    /// Transaction ID for begin transaction
    TransactionId(String),
}

impl UniversalResult {
    /// Check if the result is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, UniversalResult::Empty)
    }

    /// Try to get as a single record
    pub fn into_record(self) -> Option<UniversalRecord> {
        match self {
            UniversalResult::Record(r) => Some(r),
            _ => None,
        }
    }

    /// Try to get as multiple records
    pub fn into_records(self) -> Vec<UniversalRecord> {
        match self {
            UniversalResult::Records(rs) => rs,
            UniversalResult::Record(r) => vec![r],
            _ => Vec::new(),
        }
    }

    /// Try to get as a single value
    pub fn into_value(self) -> Option<UniversalValue> {
        match self {
            UniversalResult::Value(v) => Some(v),
            UniversalResult::Record(r) => Some(r.value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_id() {
        let id = RecordId::new("users", "alice");
        assert_eq!(id.to_storage_key(), "users:alice");

        let parsed = RecordId::from_storage_key("users:alice").unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_universal_value_conversions() {
        let v: UniversalValue = 42i64.into();
        assert_eq!(v.as_int(), Some(42));

        let v: UniversalValue = "hello".into();
        assert_eq!(v.as_str(), Some("hello"));

        let v: UniversalValue = vec![1.0f32, 2.0, 3.0].into();
        assert_eq!(v.as_vector(), Some(&[1.0f32, 2.0, 3.0][..]));
    }

    #[test]
    fn test_universal_record() {
        let record = UniversalRecord::new(
            "users",
            "alice",
            UniversalValue::Map({
                let mut m = BTreeMap::new();
                m.insert("name".to_string(), "Alice".into());
                m.insert("email".to_string(), "alice@example.com".into());
                m
            }),
            "postgresql",
        );

        assert_eq!(record.id.namespace, "users");
        assert_eq!(record.id.key, "alice");
        assert_eq!(
            record.get_field("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
    }

    #[test]
    fn test_metadata_expiration() {
        let meta = RecordMetadata::with_ttl("redis", -1000); // Already expired
        assert!(meta.is_expired());

        let meta = RecordMetadata::with_ttl("redis", 3600000); // 1 hour from now
        assert!(!meta.is_expired());
    }
}
