//! MySQL type system and value representations

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};

/// MySQL data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MySqlType {
    /// DECIMAL
    Decimal = 0x00,
    /// TINY (TINYINT)
    Tiny = 0x01,
    /// SHORT (SMALLINT)
    Short = 0x02,
    /// LONG (INT)
    Long = 0x03,
    /// FLOAT
    Float = 0x04,
    /// DOUBLE
    Double = 0x05,
    /// NULL
    Null = 0x06,
    /// TIMESTAMP
    Timestamp = 0x07,
    /// LONGLONG (BIGINT)
    LongLong = 0x08,
    /// INT24 (MEDIUMINT)
    Int24 = 0x09,
    /// DATE
    Date = 0x0A,
    /// TIME
    Time = 0x0B,
    /// DATETIME
    DateTime = 0x0C,
    /// YEAR
    Year = 0x0D,
    /// VARCHAR
    VarChar = 0x0F,
    /// BIT
    Bit = 0x10,
    /// JSON
    Json = 0xF5,
    /// NEWDECIMAL
    NewDecimal = 0xF6,
    /// ENUM
    Enum = 0xF7,
    /// SET
    Set = 0xF8,
    /// TINY_BLOB
    TinyBlob = 0xF9,
    /// MEDIUM_BLOB
    MediumBlob = 0xFA,
    /// LONG_BLOB
    LongBlob = 0xFB,
    /// BLOB
    Blob = 0xFC,
    /// VAR_STRING
    VarString = 0xFD,
    /// STRING
    String = 0xFE,
    /// GEOMETRY
    Geometry = 0xFF,
}

impl MySqlType {
    /// Convert MySQL type to Orbit SqlType
    pub fn to_sql_type(&self) -> SqlType {
        match self {
            MySqlType::Tiny => SqlType::SmallInt,
            MySqlType::Short => SqlType::SmallInt,
            MySqlType::Long | MySqlType::Int24 => SqlType::Integer,
            MySqlType::LongLong => SqlType::BigInt,
            MySqlType::Float => SqlType::Real,
            MySqlType::Double => SqlType::DoublePrecision,
            MySqlType::Decimal | MySqlType::NewDecimal => SqlType::Decimal {
                precision: None,
                scale: None,
            },
            MySqlType::VarChar | MySqlType::VarString | MySqlType::String => SqlType::Text,
            MySqlType::Blob
            | MySqlType::TinyBlob
            | MySqlType::MediumBlob
            | MySqlType::LongBlob => SqlType::Bytea,
            MySqlType::Date => SqlType::Date,
            MySqlType::Time => SqlType::Time {
                with_timezone: false,
            },
            MySqlType::DateTime | MySqlType::Timestamp => SqlType::Timestamp {
                with_timezone: false,
            },
            MySqlType::Json => SqlType::Json,
            _ => SqlType::Text,
        }
    }

    /// Convert from u8
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(MySqlType::Decimal),
            0x01 => Some(MySqlType::Tiny),
            0x02 => Some(MySqlType::Short),
            0x03 => Some(MySqlType::Long),
            0x04 => Some(MySqlType::Float),
            0x05 => Some(MySqlType::Double),
            0x06 => Some(MySqlType::Null),
            0x07 => Some(MySqlType::Timestamp),
            0x08 => Some(MySqlType::LongLong),
            0x09 => Some(MySqlType::Int24),
            0x0A => Some(MySqlType::Date),
            0x0B => Some(MySqlType::Time),
            0x0C => Some(MySqlType::DateTime),
            0x0D => Some(MySqlType::Year),
            0x0F => Some(MySqlType::VarChar),
            0x10 => Some(MySqlType::Bit),
            0xF5 => Some(MySqlType::Json),
            0xF6 => Some(MySqlType::NewDecimal),
            0xF7 => Some(MySqlType::Enum),
            0xF8 => Some(MySqlType::Set),
            0xF9 => Some(MySqlType::TinyBlob),
            0xFA => Some(MySqlType::MediumBlob),
            0xFB => Some(MySqlType::LongBlob),
            0xFC => Some(MySqlType::Blob),
            0xFD => Some(MySqlType::VarString),
            0xFE => Some(MySqlType::String),
            0xFF => Some(MySqlType::Geometry),
            _ => None,
        }
    }
}

/// MySQL value representation
#[derive(Debug, Clone, PartialEq)]
pub enum MySqlValue {
    Null,
    Int(i64),
    UInt(u64),
    Float(f32),
    Double(f64),
    String(String),
    Bytes(Vec<u8>),
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    DateTime(chrono::NaiveDateTime),
}

impl MySqlValue {
    /// Convert MySQL value to Orbit SqlValue
    pub fn to_sql_value(&self) -> ProtocolResult<SqlValue> {
        match self {
            MySqlValue::Null => Ok(SqlValue::Null),
            MySqlValue::Int(i) => {
                if *i >= i32::MIN as i64 && *i <= i32::MAX as i64 {
                    Ok(SqlValue::Integer(*i as i32))
                } else {
                    Ok(SqlValue::BigInt(*i))
                }
            }
            MySqlValue::UInt(u) => {
                if *u <= i64::MAX as u64 {
                    Ok(SqlValue::BigInt(*u as i64))
                } else {
                    Ok(SqlValue::BigInt(i64::MAX))
                }
            }
            MySqlValue::Float(f) => Ok(SqlValue::Real(*f)),
            MySqlValue::Double(f) => Ok(SqlValue::DoublePrecision(*f)),
            MySqlValue::String(s) => Ok(SqlValue::Text(s.clone())),
            MySqlValue::Bytes(b) => Ok(SqlValue::Bytea(b.clone())),
            MySqlValue::Date(d) => Ok(SqlValue::Date(*d)),
            MySqlValue::Time(t) => Ok(SqlValue::Time(*t)),
            MySqlValue::DateTime(dt) => Ok(SqlValue::Timestamp(*dt)),
        }
    }

    /// Convert Orbit SqlValue to MySQL value
    pub fn from_sql_value(value: &SqlValue) -> ProtocolResult<Self> {
        match value {
            SqlValue::Null => Ok(MySqlValue::Null),
            SqlValue::Boolean(b) => Ok(MySqlValue::Int(if *b { 1 } else { 0 })),
            SqlValue::SmallInt(i) => Ok(MySqlValue::Int(*i as i64)),
            SqlValue::Integer(i) => Ok(MySqlValue::Int(*i as i64)),
            SqlValue::BigInt(i) => Ok(MySqlValue::Int(*i)),
            SqlValue::Real(f) => Ok(MySqlValue::Float(*f)),
            SqlValue::DoublePrecision(f) => Ok(MySqlValue::Double(*f)),
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(MySqlValue::String(s.clone()))
            }
            SqlValue::Bytea(b) => Ok(MySqlValue::Bytes(b.clone())),
            SqlValue::Date(d) => Ok(MySqlValue::Date(*d)),
            SqlValue::Time(t) => Ok(MySqlValue::Time(*t)),
            SqlValue::Timestamp(dt) => Ok(MySqlValue::DateTime(*dt)),
            _ => Ok(MySqlValue::String(format!("{:?}", value))),
        }
    }
}
