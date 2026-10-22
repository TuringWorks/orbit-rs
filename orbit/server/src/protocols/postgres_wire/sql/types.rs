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
    Name, // OID 19
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
    Money,

    // Character types
    Char(Option<u32>),
    Varchar(Option<u32>),
    Text,

    // Binary types
    Bytea,
    Bit(Option<u32>),
    BitVarying(Option<u32>),

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
    JsonPath,

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
    MultiRange {
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

    // Object Identifier types
    Oid,
    Regclass,
    Regcollation,
    Regconfig,
    Regdictionary,
    Regnamespace,
    Regoper,
    Regoperator,
    Regproc,
    Regprocedure,
    Regrole,
    Regtype,

    // PostgreSQL-specific types
    PgLsn,
    PgSnapshot,
    Xid,        // Transaction ID
    AclItem,    // Access Control List
    PgNodeTree, // Internal node tree
    Int2Vector,
    OidVector,

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
    Name(String),
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
    BitString(String), // '101010'
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    TimeWithTimezone(chrono::DateTime<chrono::Utc>),
    Timestamp(chrono::NaiveDateTime),
    TimestampWithTimezone(chrono::DateTime<chrono::Utc>),
    Interval(PostgresInterval),
    Json(serde_json::Value),
    Jsonb(serde_json::Value),
    JsonPath(String),
    Array(Vec<SqlValue>),
    Composite(HashMap<String, SqlValue>),
    Range(Box<SqlRange>),
    MultiRange(Vec<SqlRange>),
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

    // Object Identifier types (stored as u32)
    Oid(u32),
    Regclass(u32),
    Regcollation(u32),
    Regconfig(u32),
    Regdictionary(u32),
    Regnamespace(u32),
    Regoper(u32),
    Regoperator(u32),
    Regproc(u32),
    Regprocedure(u32),
    Regrole(u32),
    Regtype(u32),

    // PostgreSQL-specific types
    PgLsn(u64),         // Log sequence number
    PgSnapshot(String), // Transaction snapshot (simplified as string)
    Xid(u32),
    AclItem(String),    // Simplification
    PgNodeTree(String), // Simplification
    Int2Vector(Vec<i16>),
    OidVector(Vec<u32>),

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

            (
                SqlType::MultiRange { element_type: e1 },
                SqlType::MultiRange { element_type: e2 },
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
            SqlType::Money => 790,
            SqlType::Char(_) => 1042,
            SqlType::Varchar(_) => 1043,
            SqlType::Text => 25,
            SqlType::Bytea => 17,
            SqlType::Bit(_) => 1560,
            SqlType::BitVarying(_) => 1562,
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
            SqlType::JsonPath => 4072,
            SqlType::Uuid => 2950,
            SqlType::Inet => 869,
            SqlType::Cidr => 650,
            SqlType::Array { .. } => 2277,      // Generic array type
            SqlType::Vector { .. } => 16388,    // Custom OID for vector
            SqlType::HalfVec { .. } => 16389,   // Custom OID for halfvec
            SqlType::SparseVec { .. } => 16390, // Custom OID for sparsevec

            // Object Identifier types
            SqlType::Oid => 26,
            SqlType::Regproc => 24,
            SqlType::Regprocedure => 2202,
            SqlType::Regoper => 2203,
            SqlType::Regoperator => 2204,
            SqlType::Regclass => 2205,
            SqlType::Regtype => 2206,
            SqlType::Regrole => 4096,
            SqlType::Regnamespace => 4089,
            SqlType::Regconfig => 3734,
            SqlType::Regdictionary => 3769,
            SqlType::Regcollation => 4191,

            // PostgreSQL-specific types
            SqlType::PgLsn => 3220,
            SqlType::PgSnapshot => 5038,
            SqlType::Name => 19,
            SqlType::Xid => 28,
            SqlType::AclItem => 1033,
            SqlType::PgNodeTree => 194,
            SqlType::Int2Vector => 22,
            SqlType::OidVector => 30,

            SqlType::MultiRange { element_type } => match **element_type {
                SqlType::Integer => 4451,        // int4multirange
                SqlType::Numeric { .. } => 4532, // nummultirange
                SqlType::Timestamp {
                    with_timezone: false,
                } => 4533, // tsmultirange
                SqlType::Timestamp {
                    with_timezone: true,
                } => 4534, // tstzmultirange
                SqlType::Date => 4535,           // datemultirange
                SqlType::BigInt => 4536,         // int8multirange
                _ => 0,                          // Unknown/Custom multirange
            },

            _ => 0, // Unknown type
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
            SqlType::Money => Some(8),
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

/// The [`SqlType`] a type name stands for, for the types a domain may be
/// built on.
///
/// Returns `None` for anything unrecognised, so a domain over a type this
/// module cannot construct fails the cast rather than silently becoming text.
#[must_use]
pub fn named_sql_type(name: &str) -> Option<SqlType> {
    let bare = name.split('(').next().unwrap_or(name).trim().to_uppercase();
    Some(match bare.as_str() {
        "INT2" | "SMALLINT" => SqlType::SmallInt,
        "INT" | "INT4" | "INTEGER" => SqlType::Integer,
        "INT8" | "BIGINT" => SqlType::BigInt,
        "REAL" | "FLOAT4" => SqlType::Real,
        "DOUBLE" | "DOUBLE PRECISION" | "FLOAT" | "FLOAT8" => SqlType::DoublePrecision,
        "BOOL" | "BOOLEAN" => SqlType::Boolean,
        "TEXT" => SqlType::Text,
        _ => return None,
    })
}

impl SqlValue {
    /// Get the SQL type of this value
    pub fn sql_type(&self) -> SqlType {
        match self {
            SqlValue::Null => SqlType::Text, // Default for null
            SqlValue::Boolean(_) => SqlType::Boolean,
            SqlValue::Name(_) => SqlType::Name,
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
            SqlValue::BitString(_) => SqlType::Bit(None), // Default to BIT without length
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
            SqlValue::JsonPath(_) => SqlType::JsonPath,
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
            SqlValue::MultiRange(ranges) => {
                let element_type = if ranges.is_empty() {
                    SqlType::Text
                } else {
                    // Infer from first range's lower or upper if present
                    ranges[0]
                        .lower
                        .as_ref()
                        .map(|v| v.sql_type())
                        .or_else(|| ranges[0].upper.as_ref().map(|v| v.sql_type()))
                        .unwrap_or(SqlType::Text)
                };
                SqlType::MultiRange {
                    element_type: Box::new(element_type),
                }
            }
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

            // Object Identifier types
            SqlValue::Oid(_) => SqlType::Oid,
            SqlValue::Regclass(_) => SqlType::Regclass,
            SqlValue::Regcollation(_) => SqlType::Regcollation,
            SqlValue::Regconfig(_) => SqlType::Regconfig,
            SqlValue::Regdictionary(_) => SqlType::Regdictionary,
            SqlValue::Regnamespace(_) => SqlType::Regnamespace,
            SqlValue::Regoper(_) => SqlType::Regoper,
            SqlValue::Regoperator(_) => SqlType::Regoperator,
            SqlValue::Regproc(_) => SqlType::Regproc,
            SqlValue::Regprocedure(_) => SqlType::Regprocedure,
            SqlValue::Regrole(_) => SqlType::Regrole,
            SqlValue::Regtype(_) => SqlType::Regtype,

            // PostgreSQL-specific types
            SqlValue::PgLsn(_) => SqlType::PgLsn,
            SqlValue::PgSnapshot(_) => SqlType::PgSnapshot,
            SqlValue::Xid(_) => SqlType::Xid,
            SqlValue::AclItem(_) => SqlType::AclItem,
            SqlValue::PgNodeTree(_) => SqlType::PgNodeTree,
            SqlValue::Int2Vector(_) => SqlType::Int2Vector,
            SqlValue::OidVector(_) => SqlType::OidVector,

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
            SqlValue::Name(s) => s.clone(),
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
            SqlValue::JsonPath(s) => s.clone(),
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
            SqlValue::MultiRange(ranges) => {
                let elements: Vec<String> = ranges
                    .iter()
                    .map(|r| {
                        let lower = r
                            .lower
                            .as_ref()
                            .map(|v| v.to_postgres_string())
                            .unwrap_or_default();
                        let upper = r
                            .upper
                            .as_ref()
                            .map(|v| v.to_postgres_string())
                            .unwrap_or_default();
                        // Construct range string depending on bounds
                        // Note: Simplification here, assuming standard range format [lower,upper)
                        let start_bracket = if r.lower_inclusive { '[' } else { '(' };
                        let end_bracket = if r.upper_inclusive { ']' } else { ')' };
                        format!("{}{},{}{}", start_bracket, lower, upper, end_bracket)
                    })
                    .collect();
                format!("{{{}}}", elements.join(","))
            }
            SqlValue::Point(x, y) => format!("({x},{y})"),

            // Object Identifier types
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
            | SqlValue::Regtype(oid) => oid.to_string(),

            // PostgreSQL-specific types
            SqlValue::PgLsn(lsn) => format!("{:X}/{:X}", lsn >> 32, lsn & 0xFFFFFFFF),
            SqlValue::PgSnapshot(s) => s.clone(),
            SqlValue::Xid(x) => x.to_string(),
            SqlValue::AclItem(s) | SqlValue::PgNodeTree(s) => s.clone(),
            SqlValue::Int2Vector(v) => {
                let elements: Vec<String> = v.iter().map(|i| i.to_string()).collect();
                elements.join(" ")
            }
            SqlValue::OidVector(v) => {
                let elements: Vec<String> = v.iter().map(|i| i.to_string()).collect();
                elements.join(" ")
            }

            _ => format!("{self:?}"), // Fallback for complex types
        }
    }

    /// Attempt to cast this value to another SQL type
    pub fn cast_to(&self, target_type: &SqlType) -> Result<SqlValue, String> {
        // Handle text to interval/timestamp/vector casting
        match (self, target_type) {
            (SqlValue::Text(s), SqlType::Interval)
            | (SqlValue::Varchar(s), SqlType::Interval)
            | (SqlValue::Char(s), SqlType::Interval) => {
                // Parse interval string (e.g., "1 hour", "30 minutes", "1 day")
                return Self::parse_interval(s);
            }
            (SqlValue::Text(s), SqlType::Timestamp { with_timezone })
            | (SqlValue::Varchar(s), SqlType::Timestamp { with_timezone })
            | (SqlValue::Char(s), SqlType::Timestamp { with_timezone }) => {
                // Parse timestamp string
                return Self::parse_timestamp(s, *with_timezone);
            }
            (SqlValue::Text(s), SqlType::Date)
            | (SqlValue::Varchar(s), SqlType::Date)
            | (SqlValue::Char(s), SqlType::Date) => {
                // Parse date string
                return Self::parse_date(s);
            }
            (SqlValue::Text(s), SqlType::Vector { .. })
            | (SqlValue::Varchar(s), SqlType::Vector { .. })
            | (SqlValue::Char(s), SqlType::Vector { .. }) => {
                // Parse vector string: [1.0, 2.0, 3.0] or 1.0,2.0,3.0
                let cleaned = s.trim().trim_matches('[').trim_matches(']');
                let values: Result<Vec<f32>, _> = cleaned
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect();
                return values
                    .map(SqlValue::Vector)
                    .map_err(|e| format!("Invalid vector format: {}", e));
            }
            (SqlValue::Text(s), SqlType::HalfVec { .. })
            | (SqlValue::Varchar(s), SqlType::HalfVec { .. })
            | (SqlValue::Char(s), SqlType::HalfVec { .. }) => {
                // Parse halfvec string
                let cleaned = s.trim().trim_matches('[').trim_matches(']');
                let values: Result<Vec<f32>, _> = cleaned
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect();
                return values
                    .map(SqlValue::HalfVec)
                    .map_err(|e| format!("Invalid halfvec format: {}", e));
            }
            _ => {}
        }

        // Every type has a text representation in PostgreSQL, and text parses
        // back to the numeric and boolean types. `can_cast_to` did not allow
        // either direction, so `amount::text` and `CAST('7' AS INTEGER)` —
        // both routine in generated SQL — were refused.
        if matches!(self, SqlValue::Null) {
            return Ok(SqlValue::Null);
        }
        match target_type {
            SqlType::Text => return Ok(SqlValue::Text(self.to_postgres_string())),
            SqlType::Varchar(_) => return Ok(SqlValue::Varchar(self.to_postgres_string())),
            SqlType::Char(_) => return Ok(SqlValue::Char(self.to_postgres_string())),
            SqlType::SmallInt | SqlType::Integer | SqlType::BigInt => {
                if let SqlValue::Text(text) | SqlValue::Varchar(text) | SqlValue::Char(text) = self
                {
                    let parsed = text
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| format!("invalid input syntax for integer: \"{text}\""))?;
                    return Ok(match target_type {
                        SqlType::SmallInt => SqlValue::SmallInt(parsed as i16),
                        SqlType::Integer => SqlValue::Integer(parsed as i32),
                        _ => SqlValue::BigInt(parsed),
                    });
                }
            }
            SqlType::Real | SqlType::DoublePrecision => {
                if let SqlValue::Text(text) | SqlValue::Varchar(text) | SqlValue::Char(text) = self
                {
                    let parsed = text.trim().parse::<f64>().map_err(|_| {
                        format!("invalid input syntax for double precision: \"{text}\"")
                    })?;
                    return Ok(match target_type {
                        SqlType::Real => SqlValue::Real(parsed as f32),
                        _ => SqlValue::DoublePrecision(parsed),
                    });
                }
            }
            // `NUMERIC` and `JSON` were reachable as column types but not as
            // cast targets, and once the parser accepted them the conversion
            // still had to exist.
            SqlType::Numeric { .. } | SqlType::Decimal { .. } => {
                use std::str::FromStr;
                let parsed = match self {
                    SqlValue::Text(text) | SqlValue::Varchar(text) | SqlValue::Char(text) => {
                        rust_decimal::Decimal::from_str(text.trim())
                            .map_err(|_| format!("invalid input syntax for numeric: \"{text}\""))?
                    }
                    SqlValue::SmallInt(v) => rust_decimal::Decimal::from(*v),
                    SqlValue::Integer(v) => rust_decimal::Decimal::from(*v),
                    SqlValue::BigInt(v) => rust_decimal::Decimal::from(*v),
                    SqlValue::Decimal(v) => *v,
                    SqlValue::Real(v) => rust_decimal::Decimal::from_str(&v.to_string())
                        .map_err(|_| format!("cannot represent {v} as numeric"))?,
                    SqlValue::DoublePrecision(v) => rust_decimal::Decimal::from_str(&v.to_string())
                        .map_err(|_| format!("cannot represent {v} as numeric"))?,
                    other => return Err(format!("cannot cast {:?} to numeric", other.sql_type())),
                };
                // A declared scale is applied, so `1.5::NUMERIC(10,2)` reads
                // back as `1.50` rather than losing the trailing zero.
                let mut scaled = parsed;
                if let SqlType::Numeric {
                    scale: Some(scale), ..
                }
                | SqlType::Decimal {
                    scale: Some(scale), ..
                } = target_type
                {
                    // `rescale` rather than `round_dp`: rounding alone leaves
                    // `1.5` at one decimal place, and PostgreSQL renders a
                    // declared scale in full.
                    scaled.rescale(u32::from(*scale));
                }
                return Ok(SqlValue::Decimal(scaled));
            }
            SqlType::Json | SqlType::Jsonb => {
                if let SqlValue::Text(text) | SqlValue::Varchar(text) | SqlValue::Char(text) = self
                {
                    let parsed: serde_json::Value = serde_json::from_str(text)
                        .map_err(|e| format!("invalid input syntax for json: {e}"))?;
                    return Ok(match target_type {
                        SqlType::Jsonb => SqlValue::Jsonb(parsed),
                        _ => SqlValue::Json(parsed),
                    });
                }
                if let SqlValue::Json(v) | SqlValue::Jsonb(v) = self {
                    return Ok(match target_type {
                        SqlType::Jsonb => SqlValue::Jsonb(v.clone()),
                        _ => SqlValue::Json(v.clone()),
                    });
                }
            }
            SqlType::Boolean => {
                if let SqlValue::Text(text) | SqlValue::Varchar(text) | SqlValue::Char(text) = self
                {
                    return match text.trim().to_ascii_lowercase().as_str() {
                        "t" | "true" | "yes" | "on" | "1" => Ok(SqlValue::Boolean(true)),
                        "f" | "false" | "no" | "off" | "0" => Ok(SqlValue::Boolean(false)),
                        other => Err(format!("invalid input syntax for boolean: \"{other}\"")),
                    };
                }
            }
            _ => {}
        }

        // A cast to a domain is a cast to what the domain is built on. The
        // name is resolved through the registry the query engine keeps,
        // because this function has no catalogue; a name that is not a known
        // domain still fails below rather than passing the value through.
        if let SqlType::Custom { type_name } = target_type {
            if let Some(base) = crate::protocols::postgres_wire::domains::base_of(type_name) {
                if let Some(resolved) = named_sql_type(&base) {
                    return self.cast_to(&resolved);
                }
            }
        }

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
                (SqlValue::Text(t), SqlType::JsonPath) => Ok(SqlValue::JsonPath(t.clone())),
                (SqlValue::Varchar(t), SqlType::JsonPath) => Ok(SqlValue::JsonPath(t.clone())),
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

    /// Parse an interval string like "1 hour", "30 minutes", "1 day"
    pub fn parse_interval(s: &str) -> Result<SqlValue, String> {
        let s = s.trim();
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() != 2 {
            return Err(format!("Invalid interval format: {}", s));
        }

        let value: i32 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid interval value: {}", parts[0]))?;
        let unit = parts[1].to_lowercase();

        // Convert to PostgreSQL interval representation (months, days, microseconds)
        let (months, days, microseconds) = match unit.as_str() {
            "microsecond" | "microseconds" => (0, 0, value as i64),
            "millisecond" | "milliseconds" => (0, 0, value as i64 * 1000),
            "second" | "seconds" => (0, 0, value as i64 * 1_000_000),
            "minute" | "minutes" => (0, 0, value as i64 * 60 * 1_000_000),
            "hour" | "hours" => (0, 0, value as i64 * 3600 * 1_000_000),
            "day" | "days" => (0, value, 0),
            "week" | "weeks" => (0, value * 7, 0),
            "month" | "months" => (value, 0, 0),
            "year" | "years" => (value * 12, 0, 0),
            _ => return Err(format!("Unknown interval unit: {}", unit)),
        };

        Ok(SqlValue::Interval(PostgresInterval {
            months,
            days,
            microseconds,
        }))
    }

    /// Parse a timestamp string
    pub fn parse_timestamp(s: &str, _with_timezone: bool) -> Result<SqlValue, String> {
        use chrono::NaiveDateTime;

        // Try parsing common timestamp formats
        let formats = vec![
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M:%S%.f",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M:%S%.f",
        ];

        for format in formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s, format) {
                return Ok(SqlValue::Timestamp(dt));
            }
        }

        Err(format!("Invalid timestamp format: {}", s))
    }

    /// Parse a date string
    pub fn parse_date(s: &str) -> Result<SqlValue, String> {
        use chrono::NaiveDate;

        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(SqlValue::Date)
            .map_err(|_| format!("Invalid date format: {}", s))
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
                _ => Err(format!("Invalid boolean value: {s}")),
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
            SqlType::JsonPath => Ok(SqlValue::JsonPath(s.to_string())),
            SqlType::Vector { .. } => {
                // Parse vector format: [1.0, 2.0, 3.0] or 1.0,2.0,3.0
                let cleaned = s.trim_matches('[').trim_matches(']');
                let values: Result<Vec<f32>, _> = cleaned
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect();
                values.map(SqlValue::Vector).map_err(|e| e.to_string())
            }
            _ => Err(format!("Parsing not implemented for type {sql_type:?}")),
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
            } => write!(f, "DECIMAL({p},{s})"),
            SqlType::Decimal {
                precision: Some(p),
                scale: None,
            } => write!(f, "DECIMAL({p})"),
            SqlType::Decimal { .. } => write!(f, "DECIMAL"),
            SqlType::Numeric {
                precision: Some(p),
                scale: Some(s),
            } => write!(f, "NUMERIC({p},{s})"),
            SqlType::Numeric {
                precision: Some(p),
                scale: None,
            } => write!(f, "NUMERIC({p})"),
            SqlType::Numeric { .. } => write!(f, "NUMERIC"),
            SqlType::Real => write!(f, "REAL"),
            SqlType::DoublePrecision => write!(f, "DOUBLE PRECISION"),
            SqlType::Money => write!(f, "MONEY"),
            SqlType::Char(Some(n)) => write!(f, "CHAR({n})"),
            SqlType::Char(None) => write!(f, "CHAR"),
            SqlType::Varchar(Some(n)) => write!(f, "VARCHAR({n})"),
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
            } => write!(f, "{element_type}[{d}]"),
            SqlType::Array {
                element_type,
                dimensions: None,
            } => write!(f, "{element_type}[]"),
            SqlType::Uuid => write!(f, "UUID"),
            SqlType::Vector {
                dimensions: Some(d),
            } => write!(f, "VECTOR({d})"),
            SqlType::Vector { dimensions: None } => write!(f, "VECTOR"),
            SqlType::HalfVec {
                dimensions: Some(d),
            } => write!(f, "HALFVEC({d})"),
            SqlType::HalfVec { dimensions: None } => write!(f, "HALFVEC"),
            SqlType::SparseVec {
                dimensions: Some(d),
            } => write!(f, "SPARSEVEC({d})"),
            SqlType::SparseVec { dimensions: None } => write!(f, "SPARSEVEC"),
            SqlType::Custom { type_name } => write!(f, "{type_name}"),
            _ => write!(f, "{self:?}"),
        }
    }
}
