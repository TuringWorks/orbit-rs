//! CQL type system and value representations

// from_str method name is intentional - not implementing FromStr trait
#![allow(clippy::should_implement_trait)]

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use serde::{Deserialize, Serialize};

/// CQL data types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CqlType {
    Custom(String),
    Ascii,
    Bigint,
    Blob,
    Boolean,
    Counter,
    Decimal,
    Double,
    Float,
    Int,
    Text,
    Timestamp,
    Uuid,
    Varchar,
    Varint,
    Timeuuid,
    Inet,
    Date,
    Time,
    Smallint,
    Tinyint,
    Duration,
    List(Box<CqlType>),
    Map(Box<CqlType>, Box<CqlType>),
    Set(Box<CqlType>),
    Tuple(Vec<CqlType>),
    Udt(String, Vec<(String, CqlType)>),
    /// Vector type for similarity search (dimension, element_type)
    Vector(usize, Box<CqlType>),
}

/// CQL value representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CqlValue {
    Null,
    Boolean(bool),
    Bigint(i64),
    Int(i32),
    Smallint(i16),
    Tinyint(i8),
    Text(String),
    Double(f64),
    Float(f32),
    Timestamp(i64), // milliseconds since epoch
    Uuid(String),   // UUID as string representation
    List(Vec<CqlValue>),
    Map(Vec<(CqlValue, CqlValue)>),
    Set(Vec<CqlValue>),
    Tuple(Vec<CqlValue>),
    /// Vector of floating point values for similarity search
    Vector(Vec<f32>),
}

/// Similarity function for vector search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimilarityFunction {
    /// Cosine similarity
    Cosine,
    /// Euclidean distance
    Euclidean,
    /// Dot product
    DotProduct,
}

/// CQL Event Types (for REGISTER)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CqlEventType {
    TopologyChange,
    StatusChange,
    SchemaChange,
}

impl CqlEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CqlEventType::TopologyChange => "TOPOLOGY_CHANGE",
            CqlEventType::StatusChange => "STATUS_CHANGE",
            CqlEventType::SchemaChange => "SCHEMA_CHANGE",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TOPOLOGY_CHANGE" => Some(CqlEventType::TopologyChange),
            "STATUS_CHANGE" => Some(CqlEventType::StatusChange),
            "SCHEMA_CHANGE" => Some(CqlEventType::SchemaChange),
            _ => None,
        }
    }
}

/// Schema Change Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaChangeType {
    Created,
    Updated,
    Dropped,
}

impl SchemaChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SchemaChangeType::Created => "CREATED",
            SchemaChangeType::Updated => "UPDATED",
            SchemaChangeType::Dropped => "DROPPED",
        }
    }
}

/// Topology Change Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopologyChangeType {
    NewNode,
    RemovedNode,
}

impl TopologyChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TopologyChangeType::NewNode => "NEW_NODE",
            TopologyChangeType::RemovedNode => "REMOVED_NODE",
        }
    }
}

/// Status Change Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusChangeType {
    Up,
    Down,
}

impl StatusChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StatusChangeType::Up => "UP",
            StatusChangeType::Down => "DOWN",
        }
    }
}

/// CQL Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CqlEvent {
    TopologyChange(TopologyChangeType, std::net::SocketAddr),
    StatusChange(StatusChangeType, std::net::SocketAddr),
    SchemaChange(SchemaChangeType, String, String, String), // change_type, keyspace, name, target_type
}


impl CqlType {
    /// Convert CQL type to Orbit SqlType
    pub fn to_sql_type(&self) -> ProtocolResult<SqlType> {
        match self {
            CqlType::Text | CqlType::Varchar | CqlType::Ascii => Ok(SqlType::Text),
            CqlType::Bigint | CqlType::Counter | CqlType::Varint => Ok(SqlType::BigInt),
            CqlType::Int => Ok(SqlType::Integer),
            CqlType::Smallint => Ok(SqlType::SmallInt),
            CqlType::Boolean => Ok(SqlType::Boolean),
            CqlType::Double => Ok(SqlType::DoublePrecision),
            CqlType::Float => Ok(SqlType::Real),
            CqlType::Uuid => Ok(SqlType::Uuid),
            _ => Ok(SqlType::Text),
        }
    }
}

impl CqlValue {
    /// Convert CQL value to Orbit SqlValue
    /// Collection types are stored as JSON strings in SQL
    pub fn to_sql_value(&self) -> ProtocolResult<SqlValue> {
        match self {
            CqlValue::Null => Ok(SqlValue::Null),
            CqlValue::Boolean(b) => Ok(SqlValue::Boolean(*b)),
            CqlValue::Bigint(i) => Ok(SqlValue::BigInt(*i)),
            CqlValue::Int(i) => Ok(SqlValue::Integer(*i)),
            CqlValue::Smallint(i) => Ok(SqlValue::SmallInt(*i)),
            CqlValue::Tinyint(i) => Ok(SqlValue::SmallInt(*i as i16)),
            CqlValue::Text(s) => Ok(SqlValue::Text(s.clone())),
            CqlValue::Uuid(s) => {
                let uuid = uuid::Uuid::parse_str(s)
                    .map_err(|e| ProtocolError::ConversionError(e.to_string()))?;
                Ok(SqlValue::Uuid(uuid))
            }
            CqlValue::Double(f) => Ok(SqlValue::DoublePrecision(*f)),
            CqlValue::Float(f) => Ok(SqlValue::Real(*f)),
            CqlValue::Timestamp(ts) => {
                let seconds = *ts / 1000;
                let nanos = ((*ts % 1000) * 1_000_000) as u32;
                let dt = chrono::DateTime::from_timestamp(seconds, nanos).ok_or_else(|| {
                    ProtocolError::ConversionError("Invalid timestamp".to_string())
                })?;
                Ok(SqlValue::Timestamp(dt.naive_utc()))
            }
            // Collection types stored as JSON
            CqlValue::List(items) => {
                let json = serde_json::to_string(items)
                    .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
                Ok(SqlValue::Text(json))
            }
            CqlValue::Map(entries) => {
                let json = serde_json::to_string(entries)
                    .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
                Ok(SqlValue::Text(json))
            }
            CqlValue::Set(items) => {
                let json = serde_json::to_string(items)
                    .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
                Ok(SqlValue::Text(json))
            }
            CqlValue::Tuple(items) => {
                let json = serde_json::to_string(items)
                    .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
                Ok(SqlValue::Text(json))
            }
            CqlValue::Vector(values) => {
                // Vectors stored as JSON arrays for SQL compatibility
                let json = serde_json::to_string(values)
                    .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
                Ok(SqlValue::Text(json))
            }
        }
    }

    /// Encode CQL value to bytes (for wire protocol)
    pub fn encode(&self) -> ProtocolResult<Vec<u8>> {
        use bytes::{BufMut, BytesMut};
        let mut buf = BytesMut::new();

        match self {
            CqlValue::Null => {
                buf.put_i32(-1); // NULL marker
            }
            CqlValue::Boolean(b) => {
                buf.put_i32(1);
                buf.put_u8(if *b { 1 } else { 0 });
            }
            CqlValue::Int(i) => {
                buf.put_i32(4);
                buf.put_i32(*i);
            }
            CqlValue::Bigint(i) => {
                buf.put_i32(8);
                buf.put_i64(*i);
            }
            CqlValue::Smallint(i) => {
                buf.put_i32(2);
                buf.put_i16(*i);
            }
            CqlValue::Tinyint(i) => {
                buf.put_i32(1);
                buf.put_i8(*i);
            }
            CqlValue::Float(f) => {
                buf.put_i32(4);
                buf.put_f32(*f);
            }
            CqlValue::Double(f) => {
                buf.put_i32(8);
                buf.put_f64(*f);
            }
            CqlValue::Text(s) => {
                let bytes = s.as_bytes();
                buf.put_i32(bytes.len() as i32);
                buf.put(bytes);
            }
            CqlValue::Uuid(s) => {
                // UUID is 16 bytes when encoded as binary
                // For now, we'll encode the string representation
                let bytes = s.as_bytes();
                buf.put_i32(bytes.len() as i32);
                buf.put(bytes);
            }
            CqlValue::Timestamp(ts) => {
                buf.put_i32(8);
                buf.put_i64(*ts);
            }
            // Collection types: encode as length-prefixed items
            CqlValue::List(items) => {
                let mut items_buf = BytesMut::new();
                items_buf.put_i32(items.len() as i32);
                for item in items {
                    let item_bytes = item.encode()?;
                    items_buf.put_i32(item_bytes.len() as i32);
                    items_buf.put(&item_bytes[..]);
                }
                buf.put_i32(items_buf.len() as i32);
                buf.put(items_buf.freeze());
            }
            CqlValue::Set(items) => {
                let mut items_buf = BytesMut::new();
                items_buf.put_i32(items.len() as i32);
                for item in items {
                    let item_bytes = item.encode()?;
                    items_buf.put_i32(item_bytes.len() as i32);
                    items_buf.put(&item_bytes[..]);
                }
                buf.put_i32(items_buf.len() as i32);
                buf.put(items_buf.freeze());
            }
            CqlValue::Map(entries) => {
                let mut map_buf = BytesMut::new();
                map_buf.put_i32(entries.len() as i32);
                for (key, value) in entries {
                    let key_bytes = key.encode()?;
                    map_buf.put_i32(key_bytes.len() as i32);
                    map_buf.put(&key_bytes[..]);
                    let val_bytes = value.encode()?;
                    map_buf.put_i32(val_bytes.len() as i32);
                    map_buf.put(&val_bytes[..]);
                }
                buf.put_i32(map_buf.len() as i32);
                buf.put(map_buf.freeze());
            }
            CqlValue::Tuple(items) => {
                let mut tuple_buf = BytesMut::new();
                tuple_buf.put_i32(items.len() as i32);
                for item in items {
                    let item_bytes = item.encode()?;
                    tuple_buf.put_i32(item_bytes.len() as i32);
                    tuple_buf.put(&item_bytes[..]);
                }
                buf.put_i32(tuple_buf.len() as i32);
                buf.put(tuple_buf.freeze());
            }
            CqlValue::Vector(values) => {
                // Vector encoding: length (i32) + dimension (i32) + float values
                let vec_size = 4 + values.len() * 4; // dimension + floats
                buf.put_i32(vec_size as i32);
                buf.put_i32(values.len() as i32);
                for val in values {
                    buf.put_f32(*val);
                }
            }
        }

        Ok(buf.to_vec())
    }
}

impl SimilarityFunction {
    /// Parse similarity function from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cosine" | "cos" => Some(SimilarityFunction::Cosine),
            "euclidean" | "l2" => Some(SimilarityFunction::Euclidean),
            "dot_product" | "dot" => Some(SimilarityFunction::DotProduct),
            _ => None,
        }
    }

    /// Get the string name of this similarity function
    pub fn as_str(&self) -> &'static str {
        match self {
            SimilarityFunction::Cosine => "cosine",
            SimilarityFunction::Euclidean => "euclidean",
            SimilarityFunction::DotProduct => "dot_product",
        }
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    // ============ CqlType Tests ============

    #[test]
    fn test_cql_type_to_sql_type_text_types() {
        assert_eq!(CqlType::Text.to_sql_type().unwrap(), SqlType::Text);
        assert_eq!(CqlType::Varchar.to_sql_type().unwrap(), SqlType::Text);
        assert_eq!(CqlType::Ascii.to_sql_type().unwrap(), SqlType::Text);
    }

    #[test]
    fn test_cql_type_to_sql_type_numeric_types() {
        assert_eq!(CqlType::Bigint.to_sql_type().unwrap(), SqlType::BigInt);
        assert_eq!(CqlType::Counter.to_sql_type().unwrap(), SqlType::BigInt);
        assert_eq!(CqlType::Varint.to_sql_type().unwrap(), SqlType::BigInt);
        assert_eq!(CqlType::Int.to_sql_type().unwrap(), SqlType::Integer);
        assert_eq!(CqlType::Smallint.to_sql_type().unwrap(), SqlType::SmallInt);
        assert_eq!(CqlType::Boolean.to_sql_type().unwrap(), SqlType::Boolean);
        assert_eq!(
            CqlType::Double.to_sql_type().unwrap(),
            SqlType::DoublePrecision
        );
        assert_eq!(CqlType::Float.to_sql_type().unwrap(), SqlType::Real);
    }

    #[test]
    fn test_cql_type_to_sql_type_special_types() {
        assert_eq!(CqlType::Uuid.to_sql_type().unwrap(), SqlType::Uuid);
        // Blob and other types default to Text
        assert_eq!(CqlType::Blob.to_sql_type().unwrap(), SqlType::Text);
        assert_eq!(CqlType::Timestamp.to_sql_type().unwrap(), SqlType::Text);
    }

    #[test]
    fn test_cql_type_collection_types() {
        let list_type = CqlType::List(Box::new(CqlType::Text));
        assert_eq!(list_type.to_sql_type().unwrap(), SqlType::Text);

        let map_type = CqlType::Map(Box::new(CqlType::Text), Box::new(CqlType::Int));
        assert_eq!(map_type.to_sql_type().unwrap(), SqlType::Text);

        let set_type = CqlType::Set(Box::new(CqlType::Int));
        assert_eq!(set_type.to_sql_type().unwrap(), SqlType::Text);
    }

    #[test]
    fn test_cql_type_vector() {
        let vector_type = CqlType::Vector(128, Box::new(CqlType::Float));
        assert_eq!(vector_type.to_sql_type().unwrap(), SqlType::Text);
    }

    // ============ CqlValue Tests ============

    #[test]
    fn test_cql_value_null_conversion() {
        let val = CqlValue::Null;
        assert_eq!(val.to_sql_value().unwrap(), SqlValue::Null);
    }

    #[test]
    fn test_cql_value_boolean_conversion() {
        assert_eq!(
            CqlValue::Boolean(true).to_sql_value().unwrap(),
            SqlValue::Boolean(true)
        );
        assert_eq!(
            CqlValue::Boolean(false).to_sql_value().unwrap(),
            SqlValue::Boolean(false)
        );
    }

    #[test]
    fn test_cql_value_integer_conversions() {
        assert_eq!(
            CqlValue::Bigint(9223372036854775807)
                .to_sql_value()
                .unwrap(),
            SqlValue::BigInt(9223372036854775807)
        );
        assert_eq!(
            CqlValue::Int(2147483647).to_sql_value().unwrap(),
            SqlValue::Integer(2147483647)
        );
        assert_eq!(
            CqlValue::Smallint(32767).to_sql_value().unwrap(),
            SqlValue::SmallInt(32767)
        );
        assert_eq!(
            CqlValue::Tinyint(127).to_sql_value().unwrap(),
            SqlValue::SmallInt(127)
        );
    }

    #[test]
    fn test_cql_value_float_conversions() {
        match CqlValue::Double(3.14159).to_sql_value().unwrap() {
            SqlValue::DoublePrecision(f) => assert!((f - 3.14159).abs() < 0.00001),
            _ => panic!("Expected DoublePrecision"),
        }
        match CqlValue::Float(2.718).to_sql_value().unwrap() {
            SqlValue::Real(f) => assert!((f - 2.718).abs() < 0.001),
            _ => panic!("Expected Real"),
        }
    }

    #[test]
    fn test_cql_value_text_conversion() {
        assert_eq!(
            CqlValue::Text("Hello, World!".to_string())
                .to_sql_value()
                .unwrap(),
            SqlValue::Text("Hello, World!".to_string())
        );
    }

    #[test]
    fn test_cql_value_uuid_conversion() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let val = CqlValue::Uuid(uuid_str.to_string()).to_sql_value().unwrap();
        match val {
            SqlValue::Uuid(uuid) => assert_eq!(uuid.to_string(), uuid_str),
            _ => panic!("Expected Uuid"),
        }
    }

    #[test]
    fn test_cql_value_uuid_invalid() {
        let result = CqlValue::Uuid("invalid-uuid".to_string()).to_sql_value();
        assert!(result.is_err());
    }

    #[test]
    fn test_cql_value_timestamp_conversion() {
        let timestamp_ms = 1704067200000i64; // 2024-01-01 00:00:00 UTC
        let val = CqlValue::Timestamp(timestamp_ms).to_sql_value().unwrap();
        match val {
            SqlValue::Timestamp(dt) => {
                assert_eq!(dt.and_utc().timestamp(), 1704067200);
            }
            _ => panic!("Expected Timestamp"),
        }
    }

    #[test]
    fn test_cql_value_list_conversion() {
        let list = CqlValue::List(vec![CqlValue::Int(1), CqlValue::Int(2), CqlValue::Int(3)]);
        let val = list.to_sql_value().unwrap();
        match val {
            SqlValue::Text(json) => {
                let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed.len(), 3);
            }
            _ => panic!("Expected Text (JSON)"),
        }
    }

    #[test]
    fn test_cql_value_map_conversion() {
        let map = CqlValue::Map(vec![
            (CqlValue::Text("key1".to_string()), CqlValue::Int(100)),
            (CqlValue::Text("key2".to_string()), CqlValue::Int(200)),
        ]);
        let val = map.to_sql_value().unwrap();
        match val {
            SqlValue::Text(json) => {
                // It's serialized as array of tuples
                let _parsed: Vec<(serde_json::Value, serde_json::Value)> =
                    serde_json::from_str(&json).unwrap();
            }
            _ => panic!("Expected Text (JSON)"),
        }
    }

    #[test]
    fn test_cql_value_set_conversion() {
        let set = CqlValue::Set(vec![
            CqlValue::Text("a".to_string()),
            CqlValue::Text("b".to_string()),
        ]);
        let val = set.to_sql_value().unwrap();
        assert!(matches!(val, SqlValue::Text(_)));
    }

    #[test]
    fn test_cql_value_tuple_conversion() {
        let tuple = CqlValue::Tuple(vec![CqlValue::Int(1), CqlValue::Text("hello".to_string())]);
        let val = tuple.to_sql_value().unwrap();
        assert!(matches!(val, SqlValue::Text(_)));
    }

    #[test]
    fn test_cql_value_vector_conversion() {
        let vec = CqlValue::Vector(vec![1.0, 2.0, 3.0, 4.0]);
        let val = vec.to_sql_value().unwrap();
        match val {
            SqlValue::Text(json) => {
                let parsed: Vec<f32> = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed, vec![1.0, 2.0, 3.0, 4.0]);
            }
            _ => panic!("Expected Text (JSON)"),
        }
    }

    // ============ CqlValue Encoding Tests ============

    #[test]
    fn test_encode_null() {
        let encoded = CqlValue::Null.encode().unwrap();
        assert_eq!(encoded, vec![0xff, 0xff, 0xff, 0xff]); // -1 as i32
    }

    #[test]
    fn test_encode_boolean() {
        let encoded_true = CqlValue::Boolean(true).encode().unwrap();
        assert_eq!(encoded_true.len(), 5); // 4 bytes length + 1 byte value
        assert_eq!(encoded_true[4], 1);

        let encoded_false = CqlValue::Boolean(false).encode().unwrap();
        assert_eq!(encoded_false[4], 0);
    }

    #[test]
    fn test_encode_int() {
        let encoded = CqlValue::Int(42).encode().unwrap();
        assert_eq!(encoded.len(), 8); // 4 bytes length + 4 bytes value
    }

    #[test]
    fn test_encode_bigint() {
        let encoded = CqlValue::Bigint(1234567890123).encode().unwrap();
        assert_eq!(encoded.len(), 12); // 4 bytes length + 8 bytes value
    }

    #[test]
    fn test_encode_smallint() {
        let encoded = CqlValue::Smallint(1000).encode().unwrap();
        assert_eq!(encoded.len(), 6); // 4 bytes length + 2 bytes value
    }

    #[test]
    fn test_encode_tinyint() {
        let encoded = CqlValue::Tinyint(42).encode().unwrap();
        assert_eq!(encoded.len(), 5); // 4 bytes length + 1 byte value
    }

    #[test]
    fn test_encode_float() {
        let encoded = CqlValue::Float(3.14).encode().unwrap();
        assert_eq!(encoded.len(), 8); // 4 bytes length + 4 bytes value
    }

    #[test]
    fn test_encode_double() {
        let encoded = CqlValue::Double(3.14159265359).encode().unwrap();
        assert_eq!(encoded.len(), 12); // 4 bytes length + 8 bytes value
    }

    #[test]
    fn test_encode_text() {
        let text = "Hello, CQL!";
        let encoded = CqlValue::Text(text.to_string()).encode().unwrap();
        assert_eq!(encoded.len(), 4 + text.len()); // 4 bytes length + text bytes
    }

    #[test]
    fn test_encode_uuid() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let encoded = CqlValue::Uuid(uuid.to_string()).encode().unwrap();
        assert_eq!(encoded.len(), 4 + uuid.len());
    }

    #[test]
    fn test_encode_timestamp() {
        let encoded = CqlValue::Timestamp(1704067200000).encode().unwrap();
        assert_eq!(encoded.len(), 12); // 4 bytes length + 8 bytes timestamp
    }

    #[test]
    fn test_encode_list() {
        let list = CqlValue::List(vec![CqlValue::Int(1), CqlValue::Int(2)]);
        let encoded = list.encode().unwrap();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_encode_set() {
        let set = CqlValue::Set(vec![CqlValue::Text("a".to_string())]);
        let encoded = set.encode().unwrap();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_encode_map() {
        let map = CqlValue::Map(vec![(CqlValue::Text("key".to_string()), CqlValue::Int(42))]);
        let encoded = map.encode().unwrap();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_encode_tuple() {
        let tuple = CqlValue::Tuple(vec![CqlValue::Int(1), CqlValue::Text("x".to_string())]);
        let encoded = tuple.encode().unwrap();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_encode_vector() {
        let vec = CqlValue::Vector(vec![1.0, 2.0, 3.0]);
        let encoded = vec.encode().unwrap();
        // 4 bytes total length + 4 bytes dimension + 3 * 4 bytes floats = 4 + 4 + 12 = 20
        assert_eq!(encoded.len(), 20);
    }

    // ============ SimilarityFunction Tests ============

    #[test]
    fn test_similarity_function_from_str() {
        assert_eq!(
            SimilarityFunction::from_str("cosine"),
            Some(SimilarityFunction::Cosine)
        );
        assert_eq!(
            SimilarityFunction::from_str("cos"),
            Some(SimilarityFunction::Cosine)
        );
        assert_eq!(
            SimilarityFunction::from_str("COSINE"),
            Some(SimilarityFunction::Cosine)
        );

        assert_eq!(
            SimilarityFunction::from_str("euclidean"),
            Some(SimilarityFunction::Euclidean)
        );
        assert_eq!(
            SimilarityFunction::from_str("l2"),
            Some(SimilarityFunction::Euclidean)
        );
        assert_eq!(
            SimilarityFunction::from_str("L2"),
            Some(SimilarityFunction::Euclidean)
        );

        assert_eq!(
            SimilarityFunction::from_str("dot_product"),
            Some(SimilarityFunction::DotProduct)
        );
        assert_eq!(
            SimilarityFunction::from_str("dot"),
            Some(SimilarityFunction::DotProduct)
        );
    }

    #[test]
    fn test_similarity_function_from_str_invalid() {
        assert_eq!(SimilarityFunction::from_str("invalid"), None);
        assert_eq!(SimilarityFunction::from_str(""), None);
        assert_eq!(SimilarityFunction::from_str("manhattan"), None);
    }

    #[test]
    fn test_similarity_function_as_str() {
        assert_eq!(SimilarityFunction::Cosine.as_str(), "cosine");
        assert_eq!(SimilarityFunction::Euclidean.as_str(), "euclidean");
        assert_eq!(SimilarityFunction::DotProduct.as_str(), "dot_product");
    }

    #[test]
    fn test_similarity_function_roundtrip() {
        for func in [
            SimilarityFunction::Cosine,
            SimilarityFunction::Euclidean,
            SimilarityFunction::DotProduct,
        ] {
            let str_repr = func.as_str();
            let parsed = SimilarityFunction::from_str(str_repr).unwrap();
            assert_eq!(func, parsed);
        }
    }

    // ============ CqlValue Equality Tests ============

    #[test]
    fn test_cql_value_equality() {
        assert_eq!(CqlValue::Null, CqlValue::Null);
        assert_eq!(CqlValue::Boolean(true), CqlValue::Boolean(true));
        assert_ne!(CqlValue::Boolean(true), CqlValue::Boolean(false));
        assert_eq!(CqlValue::Int(42), CqlValue::Int(42));
        assert_ne!(CqlValue::Int(42), CqlValue::Int(43));
        assert_eq!(
            CqlValue::Text("hello".to_string()),
            CqlValue::Text("hello".to_string())
        );
        assert_ne!(
            CqlValue::Text("hello".to_string()),
            CqlValue::Text("world".to_string())
        );
    }

    // ============ CqlType Equality Tests ============

    #[test]
    fn test_cql_type_equality() {
        assert_eq!(CqlType::Text, CqlType::Text);
        assert_ne!(CqlType::Text, CqlType::Int);

        let list1 = CqlType::List(Box::new(CqlType::Int));
        let list2 = CqlType::List(Box::new(CqlType::Int));
        let list3 = CqlType::List(Box::new(CqlType::Text));
        assert_eq!(list1, list2);
        assert_ne!(list1, list3);

        let map1 = CqlType::Map(Box::new(CqlType::Text), Box::new(CqlType::Int));
        let map2 = CqlType::Map(Box::new(CqlType::Text), Box::new(CqlType::Int));
        assert_eq!(map1, map2);
    }

    // ============ CqlValue Serialization Tests ============

    #[test]
    fn test_cql_value_serde() {
        let values = vec![
            CqlValue::Null,
            CqlValue::Boolean(true),
            CqlValue::Int(42),
            CqlValue::Bigint(1234567890),
            CqlValue::Text("hello".to_string()),
            CqlValue::Double(3.14),
            CqlValue::Float(2.71),
            CqlValue::List(vec![CqlValue::Int(1), CqlValue::Int(2)]),
            CqlValue::Vector(vec![1.0, 2.0, 3.0]),
        ];

        for val in values {
            let json = serde_json::to_string(&val).unwrap();
            let parsed: CqlValue = serde_json::from_str(&json).unwrap();
            assert_eq!(val, parsed);
        }
    }
}
