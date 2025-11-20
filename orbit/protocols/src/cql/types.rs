//! CQL type system and value representations

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::types::{SqlType, SqlValue};

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
#[derive(Debug, Clone, PartialEq)]
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
                let dt = chrono::DateTime::from_timestamp(seconds, nanos)
                    .ok_or_else(|| ProtocolError::ConversionError("Invalid timestamp".to_string()))?;
                Ok(SqlValue::Timestamp(dt.naive_utc()))
            }
        }
    }
}
