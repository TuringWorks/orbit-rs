//! OrbitQL Protocol Adapter
//!
//! This module provides protocol-specific adaptations for OrbitQL, including custom
//! executors and protocol-specific query handling.
//!
//! The core OrbitQL implementation has been unified in orbit-shared.

// Re-export the unified OrbitQL from orbit-shared
pub use orbit_shared::orbitql::{
    // LSP types
    create_default_schema,
    start_lsp_server,
    // Distributed execution types
    ActorNodeInfo,
    AggregateFunction,
    BinaryOperator,
    // Cache types
    CacheConfig,
    CacheHealth,
    CacheKey,
    CacheStatistics,
    CachedQueryExecutor,
    // Streaming types
    ChangeNotification,
    ChangeType,
    ColumnInfo,
    CreateStatement,
    DeleteStatement,
    DistributedExecutionContext,
    DistributedExecutionPlan,
    DistributedQueryExecutor,
    DistributedQueryPlanner,
    // Executor types
    ExecutionError,
    // Profiler types
    ExecutionPhase,
    // Planner types
    ExecutionPlan,
    ExecutionStats,
    Expression,
    FetchClause,

    FromClause,
    FunctionInfo,
    IndexInfo,
    InsertStatement,
    InvalidationEvent,
    JoinClause,
    JoinType,
    LSPConfig,
    // Lexer types
    Lexer,
    LiveQueryConfig,
    LiveQuerySubscription,
    NodeCapabilities,
    NodeType,

    // Optimizer types
    OptimizationRule,
    OptimizationSuggestion,
    OrbitQLEngine,
    OrbitQLLanguageServer,
    OrderByClause,
    ParameterInfo,
    // Parser types
    ParseError,
    Parser,

    PerformanceBottleneck,
    PlanNode,
    ProfilerConfig,
    QueryCache,

    QueryContext,
    QueryExecutor,
    QueryOptimizer,

    // Core engine types
    QueryParams,
    QueryPlanner,

    QueryProfile,
    QueryProfiler,
    QueryResult,

    QueryResultStream,
    QueryStats,
    QueryValue,
    ResourceUsage,

    SchemaInfo,
    SelectField,
    SelectStatement,
    // Spatial types
    SpatialFunctionCategory,
    SpatialFunctionInfo,
    SpatialFunctionRegistry,
    SpatialParameter,
    SpatialParameterType,
    SpatialReturnType,
    // Core AST types
    Statement,
    StreamingConfig,
    StreamingQueryBuilder,
    StreamingQueryExecutor,
    StreamingRow,

    TableInfo,
    TableType,

    TimeDuration,
    TimeExpression,
    TimeRange,
    TimeUnit,
    TimeWindow,
    Token,
    TokenType,

    UnaryOperator,
    UpdateStatement,
    WhenClause,
    SPATIAL_FUNCTIONS,
};

// Protocol-specific extensions remain here
pub mod executor;

// Re-export protocol-specific types
pub use executor::{ExecutionContext, ExecutionResult, OrbitQLExecutor};
