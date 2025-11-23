//! OrbitQL Protocol Adapter
//!
//! This module provides protocol-specific adaptations for OrbitQL, including custom
//! executors and protocol-specific query handling.
//!
//! The core OrbitQL implementation has been unified in orbit-shared.

// Re-export the unified OrbitQL from orbit-shared
pub use orbit_shared::orbitql::{
    ast::AggregateFunction,
    ast::CreateStatement,
    ast::DeleteStatement,
    ast::FetchClause,

    ast::InsertStatement,
    ast::SelectField,
    // Core AST types
    ast::Statement,
    ast::TimeDuration,
    ast::TimeExpression,
    ast::TimeRange,
    ast::TimeUnit,
    ast::TimeWindow,
    ast::UpdateStatement,
    ast::WhenClause,
    // LSP types
    create_default_schema,
    start_lsp_server,
    // Distributed execution types
    ActorNodeInfo,
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
    FromClause,
    FunctionInfo,
    IndexInfo,
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
    SelectStatement,
    // Spatial types
    SpatialFunctionCategory,
    SpatialFunctionInfo,
    SpatialFunctionRegistry,
    SpatialParameter,
    SpatialParameterType,
    SpatialReturnType,
    StreamingConfig,
    StreamingQueryBuilder,
    StreamingQueryExecutor,
    StreamingRow,

    TableInfo,
    TableType,

    Token,
    TokenType,

    UnaryOperator,
    SPATIAL_FUNCTIONS,
};

// Protocol-specific extensions remain here
pub mod executor;

// Re-export protocol-specific types
pub use executor::{ExecutionContext, ExecutionResult, OrbitQLExecutor};
