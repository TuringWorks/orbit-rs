//! Message types for database actor communication
//!
//! This module defines the message protocols used for communication between
//! database actors, including request/response patterns, streaming operations,
//! and event notifications.

use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main message type for all database operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseMessage {
    /// Query operations
    Query(QueryRequest),
    /// Transaction operations
    Transaction(TransactionRequest),
    /// Schema operations
    Schema(SchemaRequest),
    /// Vector operations (pgvector)
    Vector(VectorRequest),
    /// TimescaleDB operations
    Timescale(TimescaleRequest),
    /// Connection management
    Connection(ConnectionRequest),
    /// Performance monitoring
    Monitoring(MonitoringRequest),
    /// Configuration updates
    Configuration(ConfigurationRequest),
    /// Health checks
    Health(HealthRequest),
}

/// Response message for database operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseResponse {
    /// Query results
    Query(QueryResponse),
    /// Transaction results
    Transaction(TransactionResponse),
    /// Schema operation results
    Schema(SchemaResponse),
    /// Vector operation results
    Vector(VectorResponse),
    /// TimescaleDB operation results
    Timescale(TimescaleResponse),
    /// Connection status
    Connection(ConnectionResponse),
    /// Monitoring data
    Monitoring(MonitoringResponse),
    /// Configuration confirmation
    Configuration(ConfigurationResponse),
    /// Health status
    Health(HealthResponse),
    /// Error responses
    Error(DatabaseError),
}

// Query Messages
// ==============

/// Query request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub parameters: Vec<JsonValue>,
    pub context: QueryContext,
    pub options: QueryOptions,
}

/// Query response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<JsonValue>>,
    pub rows_affected: Option<u64>,
    pub execution_time_ms: u64,
    pub query_plan: Option<String>,
    pub warnings: Vec<String>,
}

/// Query execution options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOptions {
    pub explain: bool,
    pub analyze: bool,
    pub include_plan: bool,
    pub fetch_size: Option<u32>,
    pub streaming: bool,
    pub readonly: bool,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub ordinal_position: i32,
}

// Transaction Messages
// ===================

/// Transaction request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionRequest {
    Begin {
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<TransactionAccessMode>,
        deferrable: Option<bool>,
    },
    Commit {
        transaction_id: String,
    },
    Rollback {
        transaction_id: String,
        to_savepoint: Option<String>,
    },
    CreateSavepoint {
        transaction_id: String,
        savepoint_name: String,
    },
    ReleaseSavepoint {
        transaction_id: String,
        savepoint_name: String,
    },
    SetTransactionSnapshot {
        transaction_id: String,
        snapshot_id: String,
    },
}

/// Transaction response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionResponse {
    Started {
        transaction_id: String,
        isolation_level: IsolationLevel,
        started_at: DateTime<Utc>,
    },
    Committed {
        transaction_id: String,
        committed_at: DateTime<Utc>,
        duration_ms: u64,
    },
    RolledBack {
        transaction_id: String,
        rolled_back_at: DateTime<Utc>,
        reason: String,
    },
    SavepointCreated {
        transaction_id: String,
        savepoint_name: String,
    },
    SavepointReleased {
        transaction_id: String,
        savepoint_name: String,
    },
    SnapshotSet {
        transaction_id: String,
        snapshot_id: String,
    },
}

/// Transaction access modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionAccessMode {
    ReadWrite,
    ReadOnly,
}

// Schema Messages
// ==============

/// Schema operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaRequest {
    CreateTable {
        table_definition: TableDefinition,
        if_not_exists: bool,
    },
    DropTable {
        table_name: String,
        if_exists: bool,
        cascade: bool,
    },
    AlterTable {
        table_name: String,
        alterations: Vec<TableAlteration>,
    },
    CreateIndex {
        index_definition: IndexDefinition,
        if_not_exists: bool,
    },
    DropIndex {
        index_name: String,
        if_exists: bool,
        cascade: bool,
    },
    CreateView {
        view_definition: ViewDefinition,
        or_replace: bool,
    },
    DropView {
        view_name: String,
        if_exists: bool,
        cascade: bool,
    },
    CreateSchema {
        schema_name: String,
        if_not_exists: bool,
        authorization: Option<String>,
    },
    DropSchema {
        schema_name: String,
        if_exists: bool,
        cascade: bool,
    },
    CreateExtension {
        extension_name: String,
        if_not_exists: bool,
        schema: Option<String>,
        version: Option<String>,
    },
    DropExtension {
        extension_name: String,
        if_exists: bool,
        cascade: bool,
    },
    GetTableInfo {
        table_name: String,
    },
    ListTables {
        schema_name: Option<String>,
    },
    GetIndexInfo {
        table_name: String,
    },
}

/// Schema operation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaResponse {
    TableCreated {
        table_name: String,
        columns: Vec<ColumnDefinition>,
    },
    TableDropped {
        table_name: String,
    },
    TableAltered {
        table_name: String,
        alterations_applied: Vec<String>,
    },
    IndexCreated {
        index_name: String,
        table_name: String,
        index_type: String,
    },
    IndexDropped {
        index_name: String,
    },
    ViewCreated {
        view_name: String,
    },
    ViewDropped {
        view_name: String,
    },
    SchemaCreated {
        schema_name: String,
    },
    SchemaDropped {
        schema_name: String,
    },
    ExtensionCreated {
        extension_name: String,
        version: String,
    },
    ExtensionDropped {
        extension_name: String,
    },
    TableInfo {
        table_info: TableInfo,
    },
    TableList {
        tables: Vec<TableSummary>,
    },
    IndexInfo {
        indexes: Vec<IndexInfo>,
    },
}

/// Table definition for CREATE TABLE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDefinition {
    pub name: String,
    pub schema: Option<String>,
    pub columns: Vec<ColumnDefinition>,
    pub constraints: Vec<TableConstraint>,
    pub options: HashMap<String, String>,
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub constraints: Vec<ColumnConstraint>,
}

/// Table constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableConstraint {
    PrimaryKey {
        columns: Vec<String>,
        name: Option<String>,
    },
    ForeignKey {
        columns: Vec<String>,
        referenced_table: String,
        referenced_columns: Vec<String>,
        name: Option<String>,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },
    Unique {
        columns: Vec<String>,
        name: Option<String>,
    },
    Check {
        expression: String,
        name: Option<String>,
    },
    Exclude {
        elements: Vec<ExcludeElement>,
        name: Option<String>,
    },
}

/// Column constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnConstraint {
    NotNull,
    Unique,
    PrimaryKey,
    References {
        table: String,
        column: String,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },
    Check {
        expression: String,
    },
    Default {
        value: String,
    },
}

/// Referential actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

/// Exclude constraint elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludeElement {
    pub column: String,
    pub operator: String,
}

/// Table alterations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableAlteration {
    AddColumn {
        column: ColumnDefinition,
    },
    DropColumn {
        column_name: String,
        cascade: bool,
    },
    AlterColumn {
        column_name: String,
        alteration: ColumnAlteration,
    },
    AddConstraint {
        constraint: TableConstraint,
    },
    DropConstraint {
        constraint_name: String,
        cascade: bool,
    },
    RenameTable {
        new_name: String,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
}

/// Column alterations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnAlteration {
    SetDataType {
        data_type: String,
        using_expression: Option<String>,
    },
    SetDefault {
        default_value: String,
    },
    DropDefault,
    SetNotNull,
    DropNotNull,
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<IndexColumn>,
    pub index_type: IndexType,
    pub unique: bool,
    pub concurrent: bool,
    pub where_clause: Option<String>,
    pub options: HashMap<String, String>,
}

/// Index column specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexColumn {
    pub column_name: String,
    pub sort_order: Option<SortOrder>,
    pub nulls_order: Option<NullsOrder>,
    pub operator_class: Option<String>,
}

/// Index types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    BTree,
    Hash,
    Gist,
    Gin,
    Spgist,
    Brin,
    /// pgvector indexes
    Ivfflat,
    Hnsw,
}

/// Sort order for indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// NULLS order for indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NullsOrder {
    First,
    Last,
}

/// View definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    pub name: String,
    pub schema: Option<String>,
    pub query: String,
    pub materialized: bool,
    pub options: HashMap<String, String>,
}

/// Table information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub columns: Vec<ColumnInfo>,
    pub constraints: Vec<ConstraintInfo>,
    pub indexes: Vec<IndexInfo>,
    pub row_count: Option<u64>,
    pub size_bytes: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Table summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSummary {
    pub name: String,
    pub schema: String,
    pub table_type: String,
    pub row_count: Option<u64>,
    pub size_bytes: Option<u64>,
}

/// Constraint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    pub name: String,
    pub constraint_type: String,
    pub definition: String,
    pub columns: Vec<String>,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub index_type: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    pub size_bytes: Option<u64>,
}

// Vector Messages (pgvector)
// ==========================

/// Vector operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorRequest {
    CreateVectorTable {
        table_name: String,
        dimension: usize,
        columns: Vec<ColumnDefinition>,
        options: VectorTableOptions,
    },
    CreateVectorIndex {
        table_name: String,
        column_name: String,
        index_type: VectorIndexType,
        options: VectorIndexOptions,
    },
    InsertVectors {
        table_name: String,
        vectors: Vec<VectorRecord>,
    },
    SearchSimilar {
        table_name: String,
        query_vector: Vec<f32>,
        similarity_metric: SimilarityMetric,
        limit: usize,
        filter: Option<String>,
    },
    UpdateVector {
        table_name: String,
        id: JsonValue,
        vector: Vec<f32>,
        metadata: Option<HashMap<String, JsonValue>>,
    },
    DeleteVector {
        table_name: String,
        id: JsonValue,
    },
    GetVectorStats {
        table_name: String,
        column_name: String,
    },
    BulkSearch {
        table_name: String,
        query_vectors: Vec<Vec<f32>>,
        similarity_metric: SimilarityMetric,
        limit: usize,
        filter: Option<String>,
    },
}

/// Vector operation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorResponse {
    VectorTableCreated {
        table_name: String,
        dimension: usize,
        columns: Vec<String>,
    },
    VectorIndexCreated {
        index_name: String,
        table_name: String,
        column_name: String,
        index_type: VectorIndexType,
    },
    VectorsInserted {
        count: usize,
        failed_records: Vec<VectorInsertError>,
    },
    SimilaritySearchResults {
        results: Vec<VectorSearchResult>,
        execution_time_ms: u64,
        index_used: Option<String>,
    },
    VectorUpdated {
        id: JsonValue,
        previous_vector: Option<Vec<f32>>,
    },
    VectorDeleted {
        id: JsonValue,
        deleted: bool,
    },
    VectorStats {
        stats: VectorColumnStats,
    },
    BulkSearchResults {
        results: Vec<Vec<VectorSearchResult>>,
        execution_time_ms: u64,
    },
}

/// Vector table creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorTableOptions {
    pub storage_format: VectorStorageFormat,
    pub compression: bool,
    pub auto_index: bool,
    pub auto_index_threshold: Option<usize>,
}

/// Vector index creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexOptions {
    pub lists: Option<u32>,           // For IVFFLAT
    pub m: Option<u32>,               // For HNSW
    pub ef_construction: Option<u32>, // For HNSW
    pub ef_search: Option<u32>,       // For HNSW
    pub distance_function: SimilarityMetric,
    pub concurrent: bool,
}

/// Vector record for insertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: Option<JsonValue>,
    pub vector: Vec<f32>,
    pub metadata: HashMap<String, JsonValue>,
}

/// Vector search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub id: JsonValue,
    pub vector: Option<Vec<f32>>,
    pub metadata: HashMap<String, JsonValue>,
    pub distance: f32,
    pub similarity_score: f32,
}

/// Vector insert error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorInsertError {
    pub record_index: usize,
    pub error: String,
    pub record: VectorRecord,
}

/// Similarity metrics for vector search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimilarityMetric {
    L2,           // Euclidean distance
    Cosine,       // Cosine similarity
    InnerProduct, // Dot product
}

/// Vector column statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorColumnStats {
    pub table_name: String,
    pub column_name: String,
    pub dimension: usize,
    pub total_vectors: u64,
    pub null_vectors: u64,
    pub average_norm: Option<f64>,
    pub index_info: Option<VectorIndexInfo>,
}

/// Vector index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexInfo {
    pub index_name: String,
    pub index_type: VectorIndexType,
    pub size_bytes: u64,
    pub build_time_ms: Option<u64>,
    pub index_options: HashMap<String, JsonValue>,
}

// TimescaleDB Messages
// ===================

/// TimescaleDB operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimescaleRequest {
    CreateHypertable {
        table_name: String,
        time_column: String,
        partitioning_column: Option<String>,
        chunk_time_interval: Duration,
        options: HypertableOptions,
    },
    DropHypertable {
        table_name: String,
        cascade: bool,
    },
    AddCompressionPolicy {
        table_name: String,
        compress_after: Duration,
        options: CompressionOptions,
    },
    RemoveCompressionPolicy {
        table_name: String,
    },
    AddRetentionPolicy {
        table_name: String,
        drop_after: Duration,
    },
    RemoveRetentionPolicy {
        table_name: String,
    },
    CreateContinuousAggregate {
        view_name: String,
        query: String,
        options: ContinuousAggregateOptions,
    },
    DropContinuousAggregate {
        view_name: String,
        cascade: bool,
    },
    RefreshContinuousAggregate {
        view_name: String,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    },
    CompressChunk {
        chunk_name: String,
    },
    DecompressChunk {
        chunk_name: String,
    },
    DropChunks {
        table_name: String,
        older_than: DateTime<Utc>,
        cascade: bool,
    },
    GetHypertableStats {
        table_name: String,
    },
    GetChunkStats {
        table_name: String,
        chunk_name: Option<String>,
    },
    AddDataRetentionJob {
        table_name: String,
        schedule_interval: Duration,
        config: HashMap<String, JsonValue>,
    },
    RemoveJob {
        job_id: i32,
    },
}

/// TimescaleDB operation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimescaleResponse {
    HypertableCreated {
        table_name: String,
        time_column: String,
        partitioning_column: Option<String>,
        chunk_time_interval: Duration,
    },
    HypertableDropped {
        table_name: String,
    },
    CompressionPolicyAdded {
        table_name: String,
        job_id: i32,
        compress_after: Duration,
    },
    CompressionPolicyRemoved {
        table_name: String,
        job_id: i32,
    },
    RetentionPolicyAdded {
        table_name: String,
        job_id: i32,
        drop_after: Duration,
    },
    RetentionPolicyRemoved {
        table_name: String,
        job_id: i32,
    },
    ContinuousAggregateCreated {
        view_name: String,
        materialized: bool,
    },
    ContinuousAggregateDropped {
        view_name: String,
    },
    ContinuousAggregateRefreshed {
        view_name: String,
        rows_affected: u64,
        refresh_time_ms: u64,
    },
    ChunkCompressed {
        chunk_name: String,
        compression_ratio: f64,
        bytes_saved: u64,
    },
    ChunkDecompressed {
        chunk_name: String,
    },
    ChunksDropped {
        table_name: String,
        chunks_dropped: Vec<String>,
        rows_deleted: u64,
    },
    HypertableStats {
        stats: HypertableStats,
    },
    ChunkStats {
        stats: Vec<ChunkStats>,
    },
    DataRetentionJobAdded {
        job_id: i32,
        table_name: String,
        schedule_interval: Duration,
    },
    JobRemoved {
        job_id: i32,
    },
}

/// Hypertable creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypertableOptions {
    pub associated_schema_name: Option<String>,
    pub associated_table_prefix: Option<String>,
    pub chunk_sizing_func: Option<String>,
    pub create_default_indexes: bool,
    pub if_not_exists: bool,
}

/// Compression options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionOptions {
    pub segment_by: Vec<String>,
    pub order_by: Vec<OrderBySpec>,
}

/// Order by specification for compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBySpec {
    pub column: String,
    pub direction: SortOrder,
    pub nulls: Option<NullsOrder>,
}

/// Continuous aggregate options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousAggregateOptions {
    pub materialized: bool,
    pub refresh_lag: Option<Duration>,
    pub refresh_interval: Option<Duration>,
    pub max_interval_per_job: Option<Duration>,
}

/// Hypertable statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypertableStats {
    pub table_name: String,
    pub owner: String,
    pub num_dimensions: i32,
    pub num_chunks: i64,
    pub compression_enabled: bool,
    pub total_chunks_compressed: i64,
    pub approximate_row_count: Option<i64>,
    pub uncompressed_heap_size: Option<i64>,
    pub compressed_heap_size: Option<i64>,
}

/// Chunk statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStats {
    pub chunk_name: String,
    pub table_name: String,
    pub range_start: Option<DateTime<Utc>>,
    pub range_end: Option<DateTime<Utc>>,
    pub is_compressed: bool,
    pub compressed_heap_size: Option<i64>,
    pub uncompressed_heap_size: Option<i64>,
    pub compression_ratio: Option<f64>,
    pub number_of_rows: Option<i64>,
}

// Connection, Monitoring, Configuration Messages
// ==============================================

/// Connection management request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionRequest {
    TestConnection,
    GetConnectionStatus,
    ResetConnections,
    SetConnectionParameter { parameter: String, value: String },
    GetConnectionParameters,
}

/// Connection management response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionResponse {
    ConnectionOk {
        database: String,
        version: String,
        connection_count: u32,
    },
    ConnectionStatus {
        status: ConnectionStatus,
    },
    ConnectionsReset {
        old_count: u32,
        new_count: u32,
    },
    ParameterSet {
        parameter: String,
        value: String,
        requires_restart: bool,
    },
    Parameters {
        parameters: HashMap<String, String>,
    },
}

/// Connection status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub database: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub connected: bool,
    pub connection_time: Option<DateTime<Utc>>,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub max_connections: u32,
    pub last_error: Option<String>,
}

/// Monitoring request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitoringRequest {
    GetMetrics,
    GetQueryStats,
    GetSlowQueries {
        limit: Option<usize>,
        min_duration_ms: Option<u64>,
    },
    GetActiveQueries,
    GetLockInfo,
    ResetStats,
}

/// Monitoring response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitoringResponse {
    Metrics {
        metrics: DatabaseMetrics,
        collected_at: DateTime<Utc>,
    },
    QueryStats {
        stats: Vec<QueryStatistic>,
    },
    SlowQueries {
        queries: Vec<SlowQuery>,
    },
    ActiveQueries {
        queries: Vec<ActiveQuery>,
    },
    LockInfo {
        locks: Vec<LockInfo>,
    },
    StatsReset {
        reset_at: DateTime<Utc>,
    },
}

/// Query statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStatistic {
    pub query_hash: String,
    pub query_text: Option<String>,
    pub calls: u64,
    pub total_time_ms: f64,
    pub mean_time_ms: f64,
    pub min_time_ms: f64,
    pub max_time_ms: f64,
    pub stddev_time_ms: f64,
    pub rows: u64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Slow query information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQuery {
    pub query_text: String,
    pub duration_ms: u64,
    pub executed_at: DateTime<Utc>,
    pub database: String,
    pub username: String,
    pub client_address: Option<String>,
    pub rows_examined: Option<u64>,
    pub rows_returned: Option<u64>,
}

/// Active query information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveQuery {
    pub pid: i32,
    pub query_text: String,
    pub state: String,
    pub started_at: DateTime<Utc>,
    pub database: String,
    pub username: String,
    pub client_address: Option<String>,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
}

/// Lock information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub lock_type: String,
    pub database: String,
    pub relation: Option<String>,
    pub page: Option<i32>,
    pub tuple: Option<i16>,
    pub virtualxid: Option<String>,
    pub transactionid: Option<String>,
    pub mode: String,
    pub granted: bool,
    pub pid: i32,
}

/// Configuration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationRequest {
    UpdateDatabaseConfig { config: DatabaseConfig },
    GetDatabaseConfig,
    ValidateConfig { config: DatabaseConfig },
    ReloadConfig,
}

/// Configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationResponse {
    ConfigUpdated {
        updated_at: DateTime<Utc>,
        changes: Vec<ConfigurationChange>,
    },
    CurrentConfig {
        config: DatabaseConfig,
    },
    ValidationResult {
        valid: bool,
        errors: Vec<String>,
        warnings: Vec<String>,
    },
    ConfigReloaded {
        reloaded_at: DateTime<Utc>,
    },
}

/// Configuration change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationChange {
    pub setting: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub requires_restart: bool,
}

/// Health check request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthRequest {
    CheckHealth,
    GetHealthStatus,
    RunDiagnostics,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthResponse {
    Healthy {
        checked_at: DateTime<Utc>,
        response_time_ms: u64,
    },
    HealthStatus {
        status: HealthStatus,
    },
    Diagnostics {
        results: DiagnosticResults,
    },
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall: HealthState,
    pub components: HashMap<String, ComponentHealth>,
    pub last_check: DateTime<Utc>,
}

/// Health state enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub state: HealthState,
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
    pub metrics: HashMap<String, JsonValue>,
}

/// Diagnostic test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResults {
    pub tests_run: u32,
    pub tests_passed: u32,
    pub tests_failed: u32,
    pub tests_skipped: u32,
    pub results: Vec<DiagnosticTest>,
    pub run_at: DateTime<Utc>,
    pub duration_ms: u64,
}

/// Individual diagnostic test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticTest {
    pub name: String,
    pub category: String,
    pub status: TestStatus,
    pub message: Option<String>,
    pub duration_ms: u64,
    pub details: HashMap<String, JsonValue>,
}

/// Test status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Warning,
}

// Default implementations
// ======================

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            explain: false,
            analyze: false,
            include_plan: false,
            fetch_size: None,
            streaming: false,
            readonly: false,
        }
    }
}

impl Default for VectorTableOptions {
    fn default() -> Self {
        Self {
            storage_format: VectorStorageFormat::Vector,
            compression: false,
            auto_index: true,
            auto_index_threshold: Some(1000),
        }
    }
}

impl Default for VectorIndexOptions {
    fn default() -> Self {
        Self {
            lists: None,
            m: None,
            ef_construction: None,
            ef_search: None,
            distance_function: SimilarityMetric::Cosine,
            concurrent: true,
        }
    }
}

impl Default for HypertableOptions {
    fn default() -> Self {
        Self {
            associated_schema_name: None,
            associated_table_prefix: None,
            chunk_sizing_func: None,
            create_default_indexes: true,
            if_not_exists: false,
        }
    }
}

impl Default for ContinuousAggregateOptions {
    fn default() -> Self {
        Self {
            materialized: true,
            refresh_lag: Some(Duration::from_secs(3600)),
            refresh_interval: Some(Duration::from_secs(3600)),
            max_interval_per_job: Some(Duration::from_secs(86400)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let query_req = QueryRequest {
            sql: "SELECT * FROM users".to_string(),
            parameters: vec![],
            context: QueryContext::default(),
            options: QueryOptions::default(),
        };

        let msg = DatabaseMessage::Query(query_req);
        let json = serde_json::to_string(&msg).unwrap();
        let _deserialized: DatabaseMessage = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_vector_request_serialization() {
        let vector_req = VectorRequest::SearchSimilar {
            table_name: "documents".to_string(),
            query_vector: vec![0.1, 0.2, 0.3],
            similarity_metric: SimilarityMetric::Cosine,
            limit: 10,
            filter: None,
        };

        let msg = DatabaseMessage::Vector(vector_req);
        let json = serde_json::to_string(&msg).unwrap();
        let _deserialized: DatabaseMessage = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_timescale_request_serialization() {
        let ts_req = TimescaleRequest::CreateHypertable {
            table_name: "metrics".to_string(),
            time_column: "timestamp".to_string(),
            partitioning_column: Some("device_id".to_string()),
            chunk_time_interval: Duration::from_secs(86400),
            options: HypertableOptions::default(),
        };

        let msg = DatabaseMessage::Timescale(ts_req);
        let json = serde_json::to_string(&msg).unwrap();
        let _deserialized: DatabaseMessage = serde_json::from_str(&json).unwrap();
    }
}
