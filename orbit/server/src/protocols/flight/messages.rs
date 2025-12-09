//! Arrow Flight SQL message definitions
//!
//! Protocol messages for Arrow Flight SQL commands

use super::types::*;
use bytes::Bytes;
use std::collections::HashMap;

/// Flight SQL command types
#[derive(Debug, Clone)]
pub enum FlightSqlCommand {
    // Statement commands
    StatementQuery(StatementQuery),
    StatementUpdate(StatementUpdate),
    StatementSubstraitPlan(StatementSubstraitPlan),

    // Prepared statement commands
    PreparedStatementQuery(PreparedStatementQuery),
    PreparedStatementUpdate(PreparedStatementUpdate),
    CreatePreparedStatement(CreatePreparedStatement),
    ClosePreparedStatement(ClosePreparedStatement),

    // Catalog commands
    GetCatalogs(GetCatalogs),
    GetDbSchemas(GetDbSchemas),
    GetTables(GetTables),
    GetTableTypes(GetTableTypes),
    GetPrimaryKeys(GetPrimaryKeys),
    GetExportedKeys(GetExportedKeys),
    GetImportedKeys(GetImportedKeys),
    GetCrossReference(GetCrossReference),

    // Server info commands
    GetSqlInfo(GetSqlInfo),
    GetXdbcTypeInfo(GetXdbcTypeInfo),

    // Transaction commands
    BeginTransaction(BeginTransaction),
    EndTransaction(EndTransaction),
    BeginSavepoint(BeginSavepoint),
    EndSavepoint(EndSavepoint),

    // OrbitQL extensions
    LiveQuery(LiveQuery),
    KillLiveQuery(KillLiveQuery),
}

/// Execute a SQL statement
#[derive(Debug, Clone)]
pub struct StatementQuery {
    pub query: String,
    pub transaction_id: Option<[u8; 16]>,
}

/// Execute an update SQL statement
#[derive(Debug, Clone)]
pub struct StatementUpdate {
    pub query: String,
    pub transaction_id: Option<[u8; 16]>,
}

/// Execute a Substrait plan
#[derive(Debug, Clone)]
pub struct StatementSubstraitPlan {
    pub plan: Bytes,
    pub transaction_id: Option<[u8; 16]>,
}

/// Execute a prepared statement query
#[derive(Debug, Clone)]
pub struct PreparedStatementQuery {
    pub prepared_statement_handle: [u8; 16],
}

/// Execute a prepared statement update
#[derive(Debug, Clone)]
pub struct PreparedStatementUpdate {
    pub prepared_statement_handle: [u8; 16],
}

/// Create a prepared statement
#[derive(Debug, Clone)]
pub struct CreatePreparedStatement {
    pub query: String,
    pub transaction_id: Option<[u8; 16]>,
}

/// Close a prepared statement
#[derive(Debug, Clone)]
pub struct ClosePreparedStatement {
    pub prepared_statement_handle: [u8; 16],
}

/// Get catalogs
#[derive(Debug, Clone, Default)]
pub struct GetCatalogs {}

/// Get database schemas
#[derive(Debug, Clone)]
pub struct GetDbSchemas {
    pub catalog: Option<String>,
    pub db_schema_filter_pattern: Option<String>,
}

/// Get tables
#[derive(Debug, Clone)]
pub struct GetTables {
    pub catalog: Option<String>,
    pub db_schema_filter_pattern: Option<String>,
    pub table_name_filter_pattern: Option<String>,
    pub table_types: Vec<String>,
    pub include_schema: bool,
}

/// Get table types
#[derive(Debug, Clone, Default)]
pub struct GetTableTypes {}

/// Get primary keys
#[derive(Debug, Clone)]
pub struct GetPrimaryKeys {
    pub catalog: Option<String>,
    pub db_schema: String,
    pub table: String,
}

/// Get exported keys (foreign keys referencing this table)
#[derive(Debug, Clone)]
pub struct GetExportedKeys {
    pub catalog: Option<String>,
    pub db_schema: String,
    pub table: String,
}

/// Get imported keys (foreign keys in this table)
#[derive(Debug, Clone)]
pub struct GetImportedKeys {
    pub catalog: Option<String>,
    pub db_schema: String,
    pub table: String,
}

/// Get cross-reference between two tables
#[derive(Debug, Clone)]
pub struct GetCrossReference {
    pub pk_catalog: Option<String>,
    pub pk_db_schema: String,
    pub pk_table: String,
    pub fk_catalog: Option<String>,
    pub fk_db_schema: String,
    pub fk_table: String,
}

/// Get SQL server info
#[derive(Debug, Clone)]
pub struct GetSqlInfo {
    pub info: Vec<u32>,
}

/// Get XDBC type info
#[derive(Debug, Clone)]
pub struct GetXdbcTypeInfo {
    pub data_type: Option<i32>,
}

/// Begin a transaction
#[derive(Debug, Clone)]
pub struct BeginTransaction {
    pub isolation_level: Option<IsolationLevel>,
}

/// End a transaction
#[derive(Debug, Clone)]
pub struct EndTransaction {
    pub transaction_id: [u8; 16],
    pub action: EndTransactionAction,
}

/// End transaction action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndTransactionAction {
    Commit,
    Rollback,
}

/// Begin a savepoint
#[derive(Debug, Clone)]
pub struct BeginSavepoint {
    pub transaction_id: [u8; 16],
    pub name: String,
}

/// End a savepoint
#[derive(Debug, Clone)]
pub struct EndSavepoint {
    pub transaction_id: [u8; 16],
    pub savepoint_id: [u8; 16],
    pub action: EndSavepointAction,
}

/// End savepoint action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndSavepointAction {
    Release,
    Rollback,
}

/// OrbitQL LIVE query subscription
#[derive(Debug, Clone)]
pub struct LiveQuery {
    pub query: String,
    pub diff_mode: bool,
}

/// Kill a LIVE query subscription
#[derive(Debug, Clone)]
pub struct KillLiveQuery {
    pub subscription_id: [u8; 16],
}

/// Flight SQL result types
#[derive(Debug, Clone)]
pub enum FlightSqlResult {
    /// Query result with record batches
    QueryResult(QueryResult),
    /// Update result with affected rows
    UpdateResult(UpdateResult),
    /// Prepared statement created
    PreparedStatementCreated(PreparedStatementCreated),
    /// Transaction started
    TransactionStarted(TransactionStarted),
    /// Transaction ended
    TransactionEnded(TransactionEnded),
    /// Savepoint created
    SavepointCreated(SavepointCreated),
    /// Savepoint ended
    SavepointEnded(SavepointEnded),
    /// LIVE query subscription created
    LiveQuerySubscribed(LiveQuerySubscribed),
    /// LIVE query event
    LiveQueryEvent(LiveQueryEvent),
    /// LIVE query killed
    LiveQueryKilled(LiveQueryKilled),
    /// Catalog result
    CatalogResult(CatalogResult),
    /// Error
    Error(FlightSqlError),
}

/// Query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub schema: SchemaInfo,
    pub total_records: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Update result
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub affected_rows: i64,
}

/// Prepared statement created response
#[derive(Debug, Clone)]
pub struct PreparedStatementCreated {
    pub handle: PreparedStatementHandle,
}

/// Transaction started response
#[derive(Debug, Clone)]
pub struct TransactionStarted {
    pub transaction_id: [u8; 16],
}

/// Transaction ended response
#[derive(Debug, Clone)]
pub struct TransactionEnded {
    pub transaction_id: [u8; 16],
}

/// Savepoint created response
#[derive(Debug, Clone)]
pub struct SavepointCreated {
    pub savepoint_id: [u8; 16],
}

/// Savepoint ended response
#[derive(Debug, Clone)]
pub struct SavepointEnded {
    pub savepoint_id: [u8; 16],
}

/// LIVE query subscribed response
#[derive(Debug, Clone)]
pub struct LiveQuerySubscribed {
    pub subscription_id: [u8; 16],
    pub schema: SchemaInfo,
}

/// LIVE query event
#[derive(Debug, Clone)]
pub struct LiveQueryEvent {
    pub subscription_id: [u8; 16],
    pub event_id: u64,
    pub event_type: LiveQueryEventType,
    pub action: LiveQueryAction,
}

/// LIVE query event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveQueryEventType {
    Insert,
    Update,
    Delete,
}

/// LIVE query action
#[derive(Debug, Clone)]
pub enum LiveQueryAction {
    /// Full record data
    Full,
    /// Only changed fields (for DIFF mode)
    Diff { changed_fields: Vec<String> },
}

/// LIVE query killed response
#[derive(Debug, Clone)]
pub struct LiveQueryKilled {
    pub subscription_id: [u8; 16],
}

/// Catalog result
#[derive(Debug, Clone)]
pub struct CatalogResult {
    pub result_type: CatalogResultType,
}

/// Catalog result type
#[derive(Debug, Clone)]
pub enum CatalogResultType {
    Catalogs(Vec<CatalogInfo>),
    DbSchemas(Vec<DatabaseInfo>),
    Tables(Vec<TableInfo>),
    TableTypes(Vec<String>),
    PrimaryKeys(Vec<PrimaryKeyInfo>),
    ExportedKeys(Vec<ForeignKeyInfo>),
    ImportedKeys(Vec<ForeignKeyInfo>),
    CrossReference(Vec<ForeignKeyInfo>),
    SqlInfo(HashMap<u32, SqlInfoValue>),
    XdbcTypeInfo(Vec<XdbcTypeInfo>),
}

/// SQL info value types
#[derive(Debug, Clone)]
pub enum SqlInfoValue {
    String(String),
    Bool(bool),
    Int64(i64),
    Int32(i32),
    StringList(Vec<String>),
    Int32Bitmask(i32),
    Int32List(Vec<i32>),
}

/// XDBC type information
#[derive(Debug, Clone)]
pub struct XdbcTypeInfo {
    pub type_name: String,
    pub data_type: i32,
    pub column_size: Option<i32>,
    pub literal_prefix: Option<String>,
    pub literal_suffix: Option<String>,
    pub create_params: Option<String>,
    pub nullable: i32,
    pub case_sensitive: bool,
    pub searchable: i32,
    pub unsigned_attribute: Option<bool>,
    pub fixed_prec_scale: bool,
    pub auto_increment: Option<bool>,
    pub local_type_name: Option<String>,
    pub minimum_scale: Option<i32>,
    pub maximum_scale: Option<i32>,
    pub sql_data_type: i32,
    pub datetime_subcode: Option<i32>,
    pub num_prec_radix: Option<i32>,
    pub interval_precision: Option<i32>,
}

/// Flight SQL error
#[derive(Debug, Clone)]
pub struct FlightSqlError {
    pub code: FlightSqlErrorCode,
    pub message: String,
    pub sql_state: Option<String>,
    pub vendor_code: Option<i32>,
}

/// Flight SQL error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightSqlErrorCode {
    // General errors
    Unknown,
    Internal,
    InvalidArgument,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Unavailable,
    DataLoss,
    Unauthenticated,

    // SQL-specific errors
    SyntaxError,
    SemanticError,
    ConstraintViolation,
    TransactionError,
    DeadlockDetected,
    SerializationFailure,
    TimeoutExpired,
}

impl FlightSqlError {
    pub fn new(code: FlightSqlErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            sql_state: None,
            vendor_code: None,
        }
    }

    pub fn with_sql_state(mut self, sql_state: impl Into<String>) -> Self {
        self.sql_state = Some(sql_state.into());
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::Internal, message)
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::NotFound, message)
    }

    pub fn syntax_error(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::SyntaxError, message).with_sql_state("42000")
    }

    pub fn unauthenticated(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::Unauthenticated, message)
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::PermissionDenied, message)
    }

    pub fn transaction_error(message: impl Into<String>) -> Self {
        Self::new(FlightSqlErrorCode::TransactionError, message).with_sql_state("25000")
    }
}

/// SQL Info codes (subset of Flight SQL spec)
pub mod sql_info {
    // Server information
    pub const FLIGHT_SQL_SERVER_NAME: u32 = 0;
    pub const FLIGHT_SQL_SERVER_VERSION: u32 = 1;
    pub const FLIGHT_SQL_SERVER_ARROW_VERSION: u32 = 2;
    pub const FLIGHT_SQL_SERVER_READ_ONLY: u32 = 3;
    pub const FLIGHT_SQL_SERVER_SQL: u32 = 4;
    pub const FLIGHT_SQL_SERVER_SUBSTRAIT: u32 = 5;
    pub const FLIGHT_SQL_SERVER_SUBSTRAIT_MIN_VERSION: u32 = 6;
    pub const FLIGHT_SQL_SERVER_SUBSTRAIT_MAX_VERSION: u32 = 7;
    pub const FLIGHT_SQL_SERVER_TRANSACTION: u32 = 8;
    pub const FLIGHT_SQL_SERVER_CANCEL: u32 = 9;
    pub const FLIGHT_SQL_SERVER_STATEMENT_TIMEOUT: u32 = 100;
    pub const FLIGHT_SQL_SERVER_TRANSACTION_TIMEOUT: u32 = 101;

    // SQL support
    pub const SQL_DDL_CATALOG: u32 = 500;
    pub const SQL_DDL_SCHEMA: u32 = 501;
    pub const SQL_DDL_TABLE: u32 = 502;
    pub const SQL_IDENTIFIER_CASE: u32 = 503;
    pub const SQL_IDENTIFIER_QUOTE_CHAR: u32 = 504;
    pub const SQL_QUOTED_IDENTIFIER_CASE: u32 = 505;
    pub const SQL_ALL_TABLES_ARE_SELECTABLE: u32 = 506;
    pub const SQL_NULL_ORDERING: u32 = 507;
    pub const SQL_KEYWORDS: u32 = 508;
    pub const SQL_NUMERIC_FUNCTIONS: u32 = 509;
    pub const SQL_STRING_FUNCTIONS: u32 = 510;
    pub const SQL_SYSTEM_FUNCTIONS: u32 = 511;
    pub const SQL_DATETIME_FUNCTIONS: u32 = 512;
    pub const SQL_SEARCH_STRING_ESCAPE: u32 = 513;
    pub const SQL_EXTRA_NAME_CHARACTERS: u32 = 514;
    pub const SQL_SUPPORTS_COLUMN_ALIASING: u32 = 515;
    pub const SQL_NULL_PLUS_NULL_IS_NULL: u32 = 516;
    pub const SQL_SUPPORTS_CONVERT: u32 = 517;
    pub const SQL_SUPPORTS_TABLE_CORRELATION_NAMES: u32 = 518;
    pub const SQL_SUPPORTS_DIFFERENT_TABLE_CORRELATION_NAMES: u32 = 519;
    pub const SQL_SUPPORTS_EXPRESSIONS_IN_ORDER_BY: u32 = 520;
    pub const SQL_SUPPORTS_ORDER_BY_UNRELATED: u32 = 521;
    pub const SQL_SUPPORTED_GROUP_BY: u32 = 522;
    pub const SQL_SUPPORTS_LIKE_ESCAPE_CLAUSE: u32 = 523;
    pub const SQL_SUPPORTS_NON_NULLABLE_COLUMNS: u32 = 524;
    pub const SQL_SUPPORTED_GRAMMAR: u32 = 525;
    pub const SQL_ANSI92_SUPPORTED_LEVEL: u32 = 526;
    pub const SQL_SUPPORTS_INTEGRITY_ENHANCEMENT_FACILITY: u32 = 527;
    pub const SQL_OUTER_JOINS_SUPPORT_LEVEL: u32 = 528;
    pub const SQL_SCHEMA_TERM: u32 = 529;
    pub const SQL_PROCEDURE_TERM: u32 = 530;
    pub const SQL_CATALOG_TERM: u32 = 531;
    pub const SQL_CATALOG_AT_START: u32 = 532;
    pub const SQL_SCHEMAS_SUPPORTED_ACTIONS: u32 = 533;
    pub const SQL_CATALOGS_SUPPORTED_ACTIONS: u32 = 534;
    pub const SQL_SUPPORTED_POSITIONED_COMMANDS: u32 = 535;
    pub const SQL_SELECT_FOR_UPDATE_SUPPORTED: u32 = 536;
    pub const SQL_STORED_PROCEDURES_SUPPORTED: u32 = 537;
    pub const SQL_SUPPORTED_SUBQUERIES: u32 = 538;
    pub const SQL_CORRELATED_SUBQUERIES_SUPPORTED: u32 = 539;
    pub const SQL_SUPPORTED_UNIONS: u32 = 540;
    pub const SQL_MAX_BINARY_LITERAL_LENGTH: u32 = 541;
    pub const SQL_MAX_CHAR_LITERAL_LENGTH: u32 = 542;
    pub const SQL_MAX_COLUMN_NAME_LENGTH: u32 = 543;
    pub const SQL_MAX_COLUMNS_IN_GROUP_BY: u32 = 544;
    pub const SQL_MAX_COLUMNS_IN_INDEX: u32 = 545;
    pub const SQL_MAX_COLUMNS_IN_ORDER_BY: u32 = 546;
    pub const SQL_MAX_COLUMNS_IN_SELECT: u32 = 547;
    pub const SQL_MAX_COLUMNS_IN_TABLE: u32 = 548;
    pub const SQL_MAX_CONNECTIONS: u32 = 549;
    pub const SQL_MAX_CURSOR_NAME_LENGTH: u32 = 550;
    pub const SQL_MAX_INDEX_LENGTH: u32 = 551;
    pub const SQL_DB_SCHEMA_NAME_LENGTH: u32 = 552;
    pub const SQL_MAX_PROCEDURE_NAME_LENGTH: u32 = 553;
    pub const SQL_MAX_CATALOG_NAME_LENGTH: u32 = 554;
    pub const SQL_MAX_ROW_SIZE: u32 = 555;
    pub const SQL_MAX_ROW_SIZE_INCLUDES_BLOBS: u32 = 556;
    pub const SQL_MAX_STATEMENT_LENGTH: u32 = 557;
    pub const SQL_MAX_STATEMENTS: u32 = 558;
    pub const SQL_MAX_TABLE_NAME_LENGTH: u32 = 559;
    pub const SQL_MAX_TABLES_IN_SELECT: u32 = 560;
    pub const SQL_MAX_USERNAME_LENGTH: u32 = 561;
    pub const SQL_DEFAULT_TRANSACTION_ISOLATION: u32 = 562;
    pub const SQL_TRANSACTIONS_SUPPORTED: u32 = 563;
    pub const SQL_SUPPORTED_TRANSACTIONS_ISOLATION_LEVELS: u32 = 564;
    pub const SQL_DATA_DEFINITION_CAUSES_TRANSACTION_COMMIT: u32 = 565;
    pub const SQL_DATA_DEFINITIONS_IN_TRANSACTIONS_IGNORED: u32 = 566;
    pub const SQL_SUPPORTED_RESULT_SET_TYPES: u32 = 567;
    pub const SQL_SUPPORTED_CONCURRENCIES_FOR_RESULT_SET_TYPE: u32 = 568;
    pub const SQL_BATCH_UPDATES_SUPPORTED: u32 = 569;
    pub const SQL_SAVEPOINTS_SUPPORTED: u32 = 570;
    pub const SQL_NAMED_PARAMETERS_SUPPORTED: u32 = 571;
    pub const SQL_LOCATORS_UPDATE_COPY: u32 = 572;
    pub const SQL_STORED_FUNCTIONS_USING_CALL_SYNTAX_SUPPORTED: u32 = 573;
}
