//! Arrow Flight SQL type definitions
//!
//! Type mappings between OrbitQL and Apache Arrow

use std::collections::HashMap;

/// OrbitQL to Arrow type mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDataType {
    // Primitive types
    Null,
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float16,
    Float32,
    Float64,

    // String types
    Utf8,
    LargeUtf8,
    Binary,
    LargeBinary,

    // Temporal types
    Date32,
    Date64,
    Time32Millisecond,
    Time32Second,
    Time64Microsecond,
    Time64Nanosecond,
    TimestampSecond,
    TimestampMillisecond,
    TimestampMicrosecond,
    TimestampNanosecond,
    Duration,
    Interval,

    // Complex types
    List,
    LargeList,
    FixedSizeList,
    Struct,
    Map,
    Union,
    Dictionary,

    // Extended types
    Decimal128,
    Decimal256,
    FixedSizeBinary,

    // Geometry (extension types)
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon,
    Geometry,

    // Vector (extension type for embeddings)
    FixedSizeVector,
}

/// Schema field definition
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub data_type: ArrowDataType,
    pub nullable: bool,
    pub metadata: HashMap<String, String>,
}

impl FieldInfo {
    pub fn new(name: impl Into<String>, data_type: ArrowDataType, nullable: bool) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Schema definition for result sets
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub fields: Vec<FieldInfo>,
    pub metadata: HashMap<String, String>,
}

impl SchemaInfo {
    pub fn new(fields: Vec<FieldInfo>) -> Self {
        Self {
            fields,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Query execution handle
#[derive(Debug, Clone)]
pub struct QueryHandle {
    pub handle_id: [u8; 16],
    pub query: String,
    pub schema: Option<SchemaInfo>,
    pub created_at: u64,
    pub session_id: Option<String>,
}

impl QueryHandle {
    pub fn new(query: impl Into<String>) -> Self {
        let mut handle_id = [0u8; 16];
        // Generate random handle ID
        for byte in &mut handle_id {
            *byte = rand::random();
        }

        Self {
            handle_id,
            query: query.into(),
            schema: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            session_id: None,
        }
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Prepared statement handle
#[derive(Debug, Clone)]
pub struct PreparedStatementHandle {
    pub handle: [u8; 16],
    pub query: String,
    pub parameter_schema: Option<SchemaInfo>,
    pub result_schema: Option<SchemaInfo>,
    pub created_at: u64,
}

impl PreparedStatementHandle {
    pub fn new(query: impl Into<String>) -> Self {
        let mut handle = [0u8; 16];
        for byte in &mut handle {
            *byte = rand::random();
        }

        Self {
            handle,
            query: query.into(),
            parameter_schema: None,
            result_schema: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Transaction handle for Flight SQL
#[derive(Debug, Clone)]
pub struct TransactionHandle {
    pub transaction_id: [u8; 16],
    pub isolation_level: IsolationLevel,
    pub started_at: u64,
    pub savepoints: Vec<String>,
}

impl TransactionHandle {
    pub fn new(isolation_level: IsolationLevel) -> Self {
        let mut transaction_id = [0u8; 16];
        for byte in &mut transaction_id {
            *byte = rand::random();
        }

        Self {
            transaction_id,
            isolation_level,
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            savepoints: Vec::new(),
        }
    }

    pub fn add_savepoint(&mut self, name: impl Into<String>) {
        self.savepoints.push(name.into());
    }
}

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    ReadUncommitted,
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
    Snapshot,
}

/// LIVE query subscription handle
#[derive(Debug, Clone)]
pub struct LiveQueryHandle {
    pub subscription_id: [u8; 16],
    pub query: String,
    pub created_at: u64,
    pub last_event_id: Option<u64>,
}

impl LiveQueryHandle {
    pub fn new(query: impl Into<String>) -> Self {
        let mut subscription_id = [0u8; 16];
        for byte in &mut subscription_id {
            *byte = rand::random();
        }

        Self {
            subscription_id,
            query: query.into(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_event_id: None,
        }
    }
}

/// Catalog information for metadata queries
#[derive(Debug, Clone)]
pub struct CatalogInfo {
    pub catalog_name: String,
}

/// Database/Schema information
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    pub catalog_name: String,
    pub db_schema_name: String,
}

/// Table information
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub catalog_name: String,
    pub db_schema_name: String,
    pub table_name: String,
    pub table_type: TableType,
}

/// Table type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableType {
    Table,
    View,
    SystemTable,
    GlobalTemporary,
    LocalTemporary,
    Alias,
    Synonym,
}

impl TableType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TableType::Table => "TABLE",
            TableType::View => "VIEW",
            TableType::SystemTable => "SYSTEM TABLE",
            TableType::GlobalTemporary => "GLOBAL TEMPORARY",
            TableType::LocalTemporary => "LOCAL TEMPORARY",
            TableType::Alias => "ALIAS",
            TableType::Synonym => "SYNONYM",
        }
    }
}

/// Column information for metadata queries
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub catalog_name: String,
    pub db_schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub ordinal_position: i32,
    pub is_nullable: bool,
    pub data_type: ArrowDataType,
}

/// Primary key information
#[derive(Debug, Clone)]
pub struct PrimaryKeyInfo {
    pub catalog_name: String,
    pub db_schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub key_sequence: i32,
    pub key_name: Option<String>,
}

/// Foreign key information
#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub pk_catalog_name: String,
    pub pk_db_schema_name: String,
    pub pk_table_name: String,
    pub pk_column_name: String,
    pub fk_catalog_name: String,
    pub fk_db_schema_name: String,
    pub fk_table_name: String,
    pub fk_column_name: String,
    pub key_sequence: i32,
    pub fk_key_name: Option<String>,
    pub pk_key_name: Option<String>,
    pub update_rule: ForeignKeyRule,
    pub delete_rule: ForeignKeyRule,
}

/// Foreign key action rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForeignKeyRule {
    Cascade,
    #[default]
    Restrict,
    SetNull,
    NoAction,
    SetDefault,
}

/// Server information
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub arrow_version: String,
    pub flight_sql_version: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "Orbit-RS".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            arrow_version: "55.0.0".to_string(),
            flight_sql_version: "3".to_string(),
        }
    }
}

/// Map OrbitQL types to Arrow types
pub fn orbitql_to_arrow_type(orbitql_type: &str) -> ArrowDataType {
    match orbitql_type.to_uppercase().as_str() {
        "BOOLEAN" | "BOOL" => ArrowDataType::Boolean,
        "TINYINT" | "INT8" => ArrowDataType::Int8,
        "SMALLINT" | "INT16" => ArrowDataType::Int16,
        "INTEGER" | "INT" | "INT32" => ArrowDataType::Int32,
        "BIGINT" | "INT64" => ArrowDataType::Int64,
        "FLOAT" | "REAL" | "FLOAT32" => ArrowDataType::Float32,
        "DOUBLE" | "DOUBLE PRECISION" | "FLOAT64" => ArrowDataType::Float64,
        "DECIMAL" | "NUMERIC" => ArrowDataType::Decimal128,
        "VARCHAR" | "TEXT" | "STRING" | "CHAR" => ArrowDataType::Utf8,
        "BINARY" | "BYTEA" | "BLOB" => ArrowDataType::Binary,
        "DATE" => ArrowDataType::Date32,
        "TIME" => ArrowDataType::Time64Microsecond,
        "TIMESTAMP" | "DATETIME" => ArrowDataType::TimestampMicrosecond,
        "DURATION" | "INTERVAL" => ArrowDataType::Duration,
        "UUID" => ArrowDataType::FixedSizeBinary,
        "JSON" | "JSONB" | "OBJECT" => ArrowDataType::Utf8, // JSON as string
        "ARRAY" => ArrowDataType::List,
        "POINT" => ArrowDataType::Point,
        "LINESTRING" => ArrowDataType::LineString,
        "POLYGON" => ArrowDataType::Polygon,
        "GEOMETRY" => ArrowDataType::Geometry,
        "VECTOR" => ArrowDataType::FixedSizeVector,
        _ => ArrowDataType::Utf8, // Default to string
    }
}
