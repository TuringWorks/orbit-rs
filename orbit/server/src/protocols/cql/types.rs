//! CQL type system and value representations

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
    List(Vec<CqlValue>),
    Map(Vec<(CqlValue, CqlValue)>),
    Set(Vec<CqlValue>),
    Tuple(Vec<CqlValue>),
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
        }

        Ok(buf.to_vec())
    }
}
