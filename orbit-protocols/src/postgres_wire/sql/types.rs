//! SQL Type System and Values
//!
//! This module defines the SQL type system including all ANSI SQL data types
//! and runtime values, with extensions for PostgreSQL and vector types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQL Data Types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlType {
    // Numeric types
    Boolean,
    SmallInt,
    Integer,
    BigInt,
    Decimal {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    Numeric {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    Real,
    DoublePrecision,

    // Character types
    Char(Option<u32>),
    Varchar(Option<u32>),
    Text,

    // Binary types
    Bytea,

    // Date and time types
    Date,
    Time {
        with_timezone: bool,
    },
    Timestamp {
        with_timezone: bool,
    },
    Interval,

    // JSON types
    Json,
    Jsonb,

    // Array types
    Array {
        element_type: Box<SqlType>,
        dimensions: Option<u32>,
    },

    // Composite types
    Composite {
        type_name: String,
    },

    // Range types
    Range {
        element_type: Box<SqlType>,
    },

    // Network address types
    Inet,
    Cidr,
    Macaddr,
    Macaddr8,

    // UUID type
    Uuid,

    // XML type
    Xml,

    // Geometric types
    Point,
    Line,
    Lseg,
    Box,
    Path,
    Polygon,
    Circle,

    // Full text search
    Tsvector,
    Tsquery,

    // Vector types (pgvector extension)
    Vector {
        dimensions: Option<u32>,
    },
    HalfVec {
        dimensions: Option<u32>,
    },
    SparseVec {
        dimensions: Option<u32>,
    },

    // Custom/User-defined types
    Custom {
        type_name: String,
    },

    // Domain types
    Domain {
        domain_name: String,
        base_type: Box<SqlType>,
    },
}

/// SQL Runtime Values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SqlValue {
    #[default]
    Null,
    Boolean(bool),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Decimal(rust_decimal::Decimal),
    Real(f32),
    DoublePrecision(f64),
    Char(String),
    Varchar(String),
    Text(String),
    Bytea(Vec<u8>),
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    TimeWithTimezone(chrono::DateTime<chrono::Utc>),
    Timestamp(chrono::NaiveDateTime),
    TimestampWithTimezone(chrono::DateTime<chrono::Utc>),
    Interval(PostgresInterval),
    Json(serde_json::Value),
    Jsonb(serde_json::Value),
    Array(Vec<SqlValue>),
    Composite(HashMap<String, SqlValue>),
    Range(Box<SqlRange>),
    Inet(std::net::IpAddr),
    Cidr(IpNet),
    Macaddr([u8; 6]),
    Macaddr8([u8; 8]),
    Uuid(uuid::Uuid),
    Xml(String),
    Point(f64, f64),
    Line(f64, f64, f64),          // Ax + By + C = 0
    Lseg((f64, f64), (f64, f64)), // Line segment: start and end points
    Box((f64, f64), (f64, f64)),  // Rectangle: upper-right and lower-left corners
    Path {
        points: Vec<(f64, f64)>,
        open: bool,
    },
    Polygon(Vec<(f64, f64)>),
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    Tsvector(Vec<TsVectorElement>),
    Tsquery(String), // Simplified representation
    Vector(Vec<f32>),
    HalfVec(Vec<f32>),          // Using f32 for now until half crate is added
    SparseVec(Vec<(u32, f32)>), // (index, value) pairs
    Custom {
        type_name: String,
        data: Vec<u8>,
    },
}

/// PostgreSQL interval type representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostgresInterval {
    pub months: i32,
    pub days: i32,
    pub microseconds: i64,
}

/// SQL Range type representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlRange {
    pub lower: Option<SqlValue>,
    pub upper: Option<SqlValue>,
    pub lower_inclusive: bool,
    pub upper_inclusive: bool,
}

/// Network type representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpNet {
    pub addr: std::net::IpAddr,
    pub prefix_len: u8,
}

/// Full text search vector element
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TsVectorElement {
    pub lexeme: String,
    pub positions: Vec<u16>,
    pub weight: Option<char>, // A, B, C, or D
}

impl SqlType {
    /// Check if this type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            SqlType::SmallInt
                | SqlType::Integer
                | SqlType::BigInt
                | SqlType::Decimal { .. }
                | SqlType::Numeric { .. }
                | SqlType::Real
                | SqlType::DoublePrecision
        )
    }

    /// Check if this type is character-based
    pub fn is_character(&self) -> bool {
        matches!(self, SqlType::Char(_) | SqlType::Varchar(_) | SqlType::Text)
    }

    /// Check if this type is date/time related
    pub fn is_datetime(&self) -> bool {
        matches!(
            self,
            SqlType::Date | SqlType::Time { .. } | SqlType::Timestamp { .. } | SqlType::Interval
        )
    }

    /// Check if this type is a vector type
    pub fn is_vector(&self) -> bool {
        matches!(
            self,
            SqlType::Vector { .. } | SqlType::HalfVec { .. } | SqlType::SparseVec { .. }
        )
    }

    /// Check if this type can be cast to another type
    pub fn can_cast_to(&self, target: &SqlType) -> bool {
        if self == target {
            return true;
        }

        match (self, target) {
            // Numeric conversions
            (SqlType::SmallInt, SqlType::Integer | SqlType::BigInt) => true,
            (SqlType::Integer, SqlType::BigInt) => true,
            (SqlType::Real, SqlType::DoublePrecision) => true,

            // Character conversions
            (SqlType::Char(_), SqlType::Varchar(_) | SqlType::Text) => true,
            (SqlType::Varchar(_), SqlType::Text) => true,

            // Array element type compatibility
            (
                SqlType::Array {
                    element_type: e1, ..
                },
                SqlType::Array {
                    element_type: e2, ..
                },
            ) => e1.can_cast_to(e2),

            // JSON conversions
            (SqlType::Json, SqlType::Jsonb) => true,
            (SqlType::Jsonb, SqlType::Json) => true,

            _ => false,
        }
    }

    /// Get the PostgreSQL OID for this type
    pub fn postgres_oid(&self) -> u32 {
        match self {
            SqlType::Boolean => 16,
            SqlType::SmallInt => 21,
            SqlType::Integer => 23,
            SqlType::BigInt => 20,
            SqlType::Real => 700,
            SqlType::DoublePrecision => 701,
            SqlType::Char(_) => 1042,
            SqlType::Varchar(_) => 1043,
            SqlType::Text => 25,
            SqlType::Bytea => 17,
            SqlType::Date => 1082,
            SqlType::Time {
                with_timezone: false,
            } => 1083,
            SqlType::Time {
                with_timezone: true,
            } => 1266,
            SqlType::Timestamp {
                with_timezone: false,
            } => 1114,
            SqlType::Timestamp {
                with_timezone: true,
            } => 1184,
            SqlType::Interval => 1186,
            SqlType::Numeric { .. } => 1700,
            SqlType::Json => 114,
            SqlType::Jsonb => 3802,
            SqlType::Uuid => 2950,
            SqlType::Inet => 869,
            SqlType::Cidr => 650,
            SqlType::Array { .. } => 2277,      // Generic array type
            SqlType::Vector { .. } => 16388,    // Custom OID for vector
            SqlType::HalfVec { .. } => 16389,   // Custom OID for halfvec
            SqlType::SparseVec { .. } => 16390, // Custom OID for sparsevec
            _ => 0,                             // Unknown type
        }
    }

    /// Get the size in bytes for fixed-size types
    pub fn size(&self) -> Option<i16> {
        match self {
            SqlType::Boolean => Some(1),
            SqlType::SmallInt => Some(2),
            SqlType::Integer => Some(4),
            SqlType::BigInt => Some(8),
            SqlType::Real => Some(4),
            SqlType::DoublePrecision => Some(8),
            SqlType::Date => Some(4),
            SqlType::Time { .. } => Some(8),
            SqlType::Timestamp { .. } => Some(8),
            SqlType::Uuid => Some(16),
            SqlType::Char(Some(n)) => Some(*n as i16),
            SqlType::Vector {
                dimensions: Some(d),
            } => Some((*d as i16) * 4),
            SqlType::HalfVec {
                dimensions: Some(d),
            } => Some((*d as i16) * 2),
            _ => None, // Variable length
        }
    }
}

impl SqlValue {
    /// Get the SQL type of this value
    pub fn sql_type(&self) -> SqlType {
        match self {
            SqlValue::Null => SqlType::Text, // Default for null
            SqlValue::Boolean(_) => SqlType::Boolean,
            SqlValue::SmallInt(_) => SqlType::SmallInt,
            SqlValue::Integer(_) => SqlType::Integer,
            SqlValue::BigInt(_) => SqlType::BigInt,
            SqlValue::Decimal(_) => SqlType::Decimal {
                precision: None,
                scale: None,
            },
            SqlValue::Real(_) => SqlType::Real,
            SqlValue::DoublePrecision(_) => SqlType::DoublePrecision,
            SqlValue::Char(_) => SqlType::Char(None),
            SqlValue::Varchar(_) => SqlType::Varchar(None),
            SqlValue::Text(_) => SqlType::Text,
            SqlValue::Bytea(_) => SqlType::Bytea,
            SqlValue::Date(_) => SqlType::Date,
            SqlValue::Time(_) => SqlType::Time {
                with_timezone: false,
            },
            SqlValue::TimeWithTimezone(_) => SqlType::Time {
                with_timezone: true,
            },
            SqlValue::Timestamp(_) => SqlType::Timestamp {
                with_timezone: false,
            },
            SqlValue::TimestampWithTimezone(_) => SqlType::Timestamp {
                with_timezone: true,
            },
            SqlValue::Interval(_) => SqlType::Interval,
            SqlValue::Json(_) => SqlType::Json,
            SqlValue::Jsonb(_) => SqlType::Jsonb,
            SqlValue::Array(values) => {
                let element_type = if values.is_empty() {
                    SqlType::Text
                } else {
                    values[0].sql_type()
                };
                SqlType::Array {
                    element_type: Box::new(element_type),
                    dimensions: Some(1),
                }
            }
            SqlValue::Composite(_) => SqlType::Composite {
                type_name: "record".to_string(),
            },
            SqlValue::Range(_) => SqlType::Range {
                element_type: Box::new(SqlType::Text),
            },
            SqlValue::Inet(_) => SqlType::Inet,
            SqlValue::Cidr(_) => SqlType::Cidr,
            SqlValue::Macaddr(_) => SqlType::Macaddr,
            SqlValue::Macaddr8(_) => SqlType::Macaddr8,
            SqlValue::Uuid(_) => SqlType::Uuid,
            SqlValue::Xml(_) => SqlType::Xml,
            SqlValue::Point(_, _) => SqlType::Point,
            SqlValue::Line(_, _, _) => SqlType::Line,
            SqlValue::Lseg(_, _) => SqlType::Lseg,
            SqlValue::Box(_, _) => SqlType::Box,
            SqlValue::Path { .. } => SqlType::Path,
            SqlValue::Polygon(_) => SqlType::Polygon,
            SqlValue::Circle { .. } => SqlType::Circle,
            SqlValue::Tsvector(_) => SqlType::Tsvector,
            SqlValue::Tsquery(_) => SqlType::Tsquery,
            SqlValue::Vector(v) => SqlType::Vector {
                dimensions: Some(v.len() as u32),
            },
            SqlValue::HalfVec(v) => SqlType::HalfVec {
                dimensions: Some(v.len() as u32),
            },
            SqlValue::SparseVec(_) => SqlType::SparseVec { dimensions: None },
            SqlValue::Custom { type_name, .. } => SqlType::Custom {
                type_name: type_name.clone(),
            },
        }
    }

    /// Check if this value is null
    pub fn is_null(&self) -> bool {
        matches!(self, SqlValue::Null)
    }

    /// Convert to PostgreSQL wire format string representation
    pub fn to_postgres_string(&self) -> String {
        match self {
            SqlValue::Null => "".to_string(),
            SqlValue::Boolean(b) => {
                if *b {
                    "t".to_string()
                } else {
                    "f".to_string()
                }
            }
            SqlValue::SmallInt(i) => i.to_string(),
            SqlValue::Integer(i) => i.to_string(),
            SqlValue::BigInt(i) => i.to_string(),
            SqlValue::Decimal(d) => d.to_string(),
            SqlValue::Real(f) => f.to_string(),
            SqlValue::DoublePrecision(f) => f.to_string(),
            SqlValue::Char(s) | SqlValue::Varchar(s) | SqlValue::Text(s) => s.clone(),
            SqlValue::Bytea(bytes) => {
                format!("\\x{}", hex::encode(bytes))
            }
            SqlValue::Date(d) => d.format("%Y-%m-%d").to_string(),
            SqlValue::Time(t) => t.format("%H:%M:%S%.f").to_string(),
            SqlValue::TimeWithTimezone(dt) => dt.format("%H:%M:%S%.f%z").to_string(),
            SqlValue::Timestamp(dt) => dt.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
            SqlValue::TimestampWithTimezone(dt) => dt.format("%Y-%m-%d %H:%M:%S%.f%z").to_string(),
            SqlValue::Interval(interval) => format!(
                "{}mons {}days {}microseconds",
                interval.months, interval.days, interval.microseconds
            ),
            SqlValue::Json(v) | SqlValue::Jsonb(v) => v.to_string(),
            SqlValue::Array(values) => {
                let elements: Vec<String> = values.iter().map(|v| v.to_postgres_string()).collect();
                format!("{{{}}}", elements.join(","))
            }
            SqlValue::Uuid(u) => u.to_string(),
            SqlValue::Vector(v) => {
                let elements: Vec<String> = v.iter().map(|f| f.to_string()).collect();
                format!("[{}]", elements.join(","))
            }
            SqlValue::HalfVec(v) => {
                let elements: Vec<String> = v.iter().map(|f| f.to_string()).collect();
                format!("[{}]", elements.join(","))
            }
            SqlValue::Point(x, y) => format!("({},{})", x, y),
            _ => format!("{:?}", self), // Fallback for complex types
        }
    }

    /// Attempt to cast this value to another SQL type
    pub fn cast_to(&self, target_type: &SqlType) -> Result<SqlValue, String> {
        if self.sql_type().can_cast_to(target_type) {
            match (self, target_type) {
                (SqlValue::SmallInt(i), SqlType::Integer) => Ok(SqlValue::Integer(*i as i32)),
                (SqlValue::SmallInt(i), SqlType::BigInt) => Ok(SqlValue::BigInt(*i as i64)),
                (SqlValue::Integer(i), SqlType::BigInt) => Ok(SqlValue::BigInt(*i as i64)),
                (SqlValue::Real(f), SqlType::DoublePrecision) => {
                    Ok(SqlValue::DoublePrecision(*f as f64))
                }
                (SqlValue::Char(s), SqlType::Varchar(_)) => Ok(SqlValue::Varchar(s.clone())),
                (SqlValue::Char(s), SqlType::Text) => Ok(SqlValue::Text(s.clone())),
                (SqlValue::Varchar(s), SqlType::Text) => Ok(SqlValue::Text(s.clone())),
                (SqlValue::Json(v), SqlType::Jsonb) => Ok(SqlValue::Jsonb(v.clone())),
                (SqlValue::Jsonb(v), SqlType::Json) => Ok(SqlValue::Json(v.clone())),
                _ => Ok(self.clone()), // Same type or already handled
            }
        } else {
            Err(format!(
                "Cannot cast {:?} to {:?}",
                self.sql_type(),
                target_type
            ))
        }
    }

    /// Parse a string value into a SQL value of the specified type
    pub fn parse_string(s: &str, sql_type: &SqlType) -> Result<SqlValue, String> {
        if s.is_empty() || s.eq_ignore_ascii_case("null") {
            return Ok(SqlValue::Null);
        }

        match sql_type {
            SqlType::Boolean => match s.to_lowercase().as_str() {
                "t" | "true" | "1" | "yes" | "on" => Ok(SqlValue::Boolean(true)),
                "f" | "false" | "0" | "no" | "off" => Ok(SqlValue::Boolean(false)),
                _ => Err(format!("Invalid boolean value: {}", s)),
            },
            SqlType::SmallInt => s
                .parse::<i16>()
                .map(SqlValue::SmallInt)
                .map_err(|e| e.to_string()),
            SqlType::Integer => s
                .parse::<i32>()
                .map(SqlValue::Integer)
                .map_err(|e| e.to_string()),
            SqlType::BigInt => s
                .parse::<i64>()
                .map(SqlValue::BigInt)
                .map_err(|e| e.to_string()),
            SqlType::Real => s
                .parse::<f32>()
                .map(SqlValue::Real)
                .map_err(|e| e.to_string()),
            SqlType::DoublePrecision => s
                .parse::<f64>()
                .map(SqlValue::DoublePrecision)
                .map_err(|e| e.to_string()),
            SqlType::Char(_) => Ok(SqlValue::Char(s.to_string())),
            SqlType::Varchar(_) => Ok(SqlValue::Varchar(s.to_string())),
            SqlType::Text => Ok(SqlValue::Text(s.to_string())),
            SqlType::Json => serde_json::from_str(s)
                .map(SqlValue::Json)
                .map_err(|e| e.to_string()),
            SqlType::Jsonb => serde_json::from_str(s)
                .map(SqlValue::Jsonb)
                .map_err(|e| e.to_string()),
            SqlType::Vector { .. } => {
                // Parse vector format: [1.0, 2.0, 3.0] or 1.0,2.0,3.0
                let cleaned = s.trim_matches('[').trim_matches(']');
                let values: Result<Vec<f32>, _> = cleaned
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect();
                values.map(SqlValue::Vector).map_err(|e| e.to_string())
            }
            _ => Err(format!("Parsing not implemented for type {:?}", sql_type)),
        }
    }
}

impl std::fmt::Display for SqlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_postgres_string())
    }
}

impl std::fmt::Display for SqlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlType::Boolean => write!(f, "BOOLEAN"),
            SqlType::SmallInt => write!(f, "SMALLINT"),
            SqlType::Integer => write!(f, "INTEGER"),
            SqlType::BigInt => write!(f, "BIGINT"),
            SqlType::Decimal {
                precision: Some(p),
                scale: Some(s),
            } => write!(f, "DECIMAL({},{})", p, s),
            SqlType::Decimal {
                precision: Some(p),
                scale: None,
            } => write!(f, "DECIMAL({})", p),
            SqlType::Decimal { .. } => write!(f, "DECIMAL"),
            SqlType::Numeric {
                precision: Some(p),
                scale: Some(s),
            } => write!(f, "NUMERIC({},{})", p, s),
            SqlType::Numeric {
                precision: Some(p),
                scale: None,
            } => write!(f, "NUMERIC({})", p),
            SqlType::Numeric { .. } => write!(f, "NUMERIC"),
            SqlType::Real => write!(f, "REAL"),
            SqlType::DoublePrecision => write!(f, "DOUBLE PRECISION"),
            SqlType::Char(Some(n)) => write!(f, "CHAR({})", n),
            SqlType::Char(None) => write!(f, "CHAR"),
            SqlType::Varchar(Some(n)) => write!(f, "VARCHAR({})", n),
            SqlType::Varchar(None) => write!(f, "VARCHAR"),
            SqlType::Text => write!(f, "TEXT"),
            SqlType::Bytea => write!(f, "BYTEA"),
            SqlType::Date => write!(f, "DATE"),
            SqlType::Time {
                with_timezone: true,
            } => write!(f, "TIME WITH TIME ZONE"),
            SqlType::Time {
                with_timezone: false,
            } => write!(f, "TIME"),
            SqlType::Timestamp {
                with_timezone: true,
            } => write!(f, "TIMESTAMP WITH TIME ZONE"),
            SqlType::Timestamp {
                with_timezone: false,
            } => write!(f, "TIMESTAMP"),
            SqlType::Interval => write!(f, "INTERVAL"),
            SqlType::Json => write!(f, "JSON"),
            SqlType::Jsonb => write!(f, "JSONB"),
            SqlType::Array {
                element_type,
                dimensions: Some(d),
            } => write!(f, "{}[{}]", element_type, d),
            SqlType::Array {
                element_type,
                dimensions: None,
            } => write!(f, "{}[]", element_type),
            SqlType::Uuid => write!(f, "UUID"),
            SqlType::Vector {
                dimensions: Some(d),
            } => write!(f, "VECTOR({})", d),
            SqlType::Vector { dimensions: None } => write!(f, "VECTOR"),
            SqlType::HalfVec {
                dimensions: Some(d),
            } => write!(f, "HALFVEC({})", d),
            SqlType::HalfVec { dimensions: None } => write!(f, "HALFVEC"),
            SqlType::SparseVec {
                dimensions: Some(d),
            } => write!(f, "SPARSEVEC({})", d),
            SqlType::SparseVec { dimensions: None } => write!(f, "SPARSEVEC"),
            SqlType::Custom { type_name } => write!(f, "{}", type_name),
            _ => write!(f, "{:?}", self),
        }
    }
}
