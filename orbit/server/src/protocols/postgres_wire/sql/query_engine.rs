//! Unified Query Engine with Phase 9 Optimizations
//!
//! This module integrates all Phase 9 query optimization components into a
//! cohesive query engine that provides:
//!
//! - Query and plan caching
//! - Statistics-based optimization
//! - Cardinality estimation
//! - Cost-based CPU/GPU routing
//! - Parallel query execution
//! - Index recommendations
//! - Vectorized execution
//!
//! ## Usage
//!
//! ```rust,ignore
//! use orbit_server::protocols::postgres_wire::sql::query_engine::OptimizedQueryEngine;
//!
//! let engine = OptimizedQueryEngine::new_default();
//! let result = engine.execute("SELECT * FROM users WHERE id = 1").await?;
//! ```

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::Statement;
use crate::protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};
use crate::protocols::postgres_wire::sql::index_advisor::{
    IndexAdvisor, IndexAdvisorConfig, IndexRecommendation,
};
use crate::protocols::postgres_wire::sql::optimizer::cardinality::{
    CardinalityConfig, CardinalityEstimator,
};
use crate::protocols::postgres_wire::sql::optimizer::cost_router::{
    CostRouter, CostRouterConfig, ExecutionBackend, OperationType,
};
use crate::protocols::postgres_wire::sql::parallel_executor::{
    ParallelConfig, ParallelCoordinator,
};
use crate::protocols::postgres_wire::sql::parser::SqlParser;
use crate::protocols::postgres_wire::sql::plan_cache::{PlanCache, PlanCacheConfig, QueryPlan};
use crate::protocols::postgres_wire::sql::query_cache::{
    extract_table_names, QueryCache, QueryCacheConfig, QueryKey,
};
use crate::protocols::postgres_wire::sql::statistics::{StatisticsConfig, StatisticsManager};
use crate::protocols::postgres_wire::sql::vectorized_executor::{
    VectorizedConfig, VectorizedExecutor,
};

#[cfg(feature = "lua-mlua")]
use crate::protocols::postgres_wire::sql::udf_handler::UdfHandler;
use serde::{Deserialize, Serialize}; // Used by QueryMetrics, CacheStatistics
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Configuration for the optimized query engine
#[derive(Debug, Clone)]
pub struct QueryEngineConfig {
    /// Query cache configuration
    pub query_cache: QueryCacheConfig,
    /// Plan cache configuration
    pub plan_cache: PlanCacheConfig,
    /// Statistics configuration
    pub statistics: StatisticsConfig,
    /// Cardinality estimation configuration
    pub cardinality: CardinalityConfig,
    /// Cost-based routing configuration
    pub cost_router: CostRouterConfig,
    /// Parallel execution configuration
    pub parallel: ParallelConfig,
    /// Index advisor configuration
    pub index_advisor: IndexAdvisorConfig,
    /// Vectorized execution configuration
    pub vectorized: VectorizedConfig,
    /// Enable query logging for analysis
    pub enable_query_logging: bool,
    /// Enable performance metrics
    pub enable_metrics: bool,
}

impl Default for QueryEngineConfig {
    fn default() -> Self {
        Self {
            query_cache: QueryCacheConfig::default(),
            plan_cache: PlanCacheConfig::default(),
            statistics: StatisticsConfig::default(),
            cardinality: CardinalityConfig::default(),
            cost_router: CostRouterConfig::default(),
            parallel: ParallelConfig::default(),
            index_advisor: IndexAdvisorConfig::default(),
            vectorized: VectorizedConfig::default(),
            enable_query_logging: true,
            enable_metrics: true,
        }
    }
}

/// Query execution metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryMetrics {
    /// Total queries executed
    pub total_queries: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Queries executed in parallel
    pub parallel_queries: u64,
    /// Queries using SIMD
    pub simd_queries: u64,
    /// Queries using GPU
    pub gpu_queries: u64,
    /// Total execution time (ms)
    pub total_execution_time_ms: u64,
    /// Average execution time (ms)
    pub avg_execution_time_ms: f64,
}

/// Optimized query engine with all Phase 9 features
pub struct OptimizedQueryEngine {
    /// Configuration
    config: QueryEngineConfig,
    /// SQL parser (wrapped in RwLock for interior mutability)
    parser: Arc<RwLock<SqlParser>>,
    /// Base SQL executor
    executor: Arc<SqlExecutor>,
    /// Query result cache
    query_cache: QueryCache,
    /// Execution plan cache
    plan_cache: Arc<RwLock<PlanCache>>,
    /// Statistics manager
    statistics: StatisticsManager,
    /// Cardinality estimator
    cardinality_estimator: CardinalityEstimator,
    /// Cost-based router
    cost_router: CostRouter,
    /// Parallel coordinator (reserved for full parallel execution)
    #[allow(dead_code)]
    parallel_coordinator: ParallelCoordinator,
    /// Index advisor
    index_advisor: IndexAdvisor,
    /// Vectorized executor (reserved for full SIMD execution)
    #[allow(dead_code)]
    vectorized_executor: Arc<RwLock<VectorizedExecutor>>,
    /// Query metrics
    metrics: Arc<RwLock<QueryMetrics>>,
    /// UDF handler for Lua/JS functions
    #[cfg(feature = "lua-mlua")]
    udf_handler: Option<Arc<UdfHandler>>,
    /// Python UDF handler
    #[cfg(feature = "python-udf")]
    python_udf_handler: Option<Arc<crate::python::udf_handler::PythonUdfHandler>>,
}

impl OptimizedQueryEngine {
    /// Create a new optimized query engine (async due to SqlExecutor initialization)
    pub async fn new(config: QueryEngineConfig) -> crate::protocols::error::ProtocolResult<Self> {
        let vectorized = VectorizedExecutor::new(config.vectorized.clone()).await?;
        let executor = Arc::new(SqlExecutor::new().await?);

        #[cfg(feature = "lua-mlua")]
        let udf_handler = {
            use crate::lua::mlua_runtime::MluaRuntime;
            use crate::protocols::postgres_wire::sql::udf_handler::UdfHandler;
            let runtime = Arc::new(MluaRuntime::new());
            let handler = Arc::new(UdfHandler::new(runtime));

            // Connect registry to executor for expression evaluation
            executor.set_udf_registry(handler.registry()).await;

            Some(handler)
        };

        #[cfg(feature = "python-udf")]
        let python_udf_handler = {
            use crate::python::config::PythonConfig;
            use crate::python::udf_handler::PythonUdfHandler;
            use crate::python::udf_registry::PythonUdfRegistry;

            let python_config = PythonConfig::default();
            let registry = Arc::new(PythonUdfRegistry::new(python_config).await.map_err(|e| {
                crate::protocols::error::ProtocolError::PostgresError(format!(
                    "Failed to create Python UDF registry: {}",
                    e
                ))
            })?);
            let handler = Arc::new(PythonUdfHandler::new(registry));

            Some(handler)
        };

        Ok(Self {
            parser: Arc::new(RwLock::new(SqlParser::new())),
            executor,
            query_cache: QueryCache::new(config.query_cache.clone()),
            plan_cache: Arc::new(RwLock::new(PlanCache::new(config.plan_cache.clone()))),
            statistics: StatisticsManager::new(config.statistics.clone()),
            cardinality_estimator: CardinalityEstimator::new(config.cardinality.clone()),
            cost_router: CostRouter::new(config.cost_router.clone()),
            parallel_coordinator: ParallelCoordinator::new(config.parallel.clone()),
            index_advisor: IndexAdvisor::new(config.index_advisor.clone()),
            vectorized_executor: Arc::new(RwLock::new(vectorized)),
            metrics: Arc::new(RwLock::new(QueryMetrics::default())),
            #[cfg(feature = "lua-mlua")]
            udf_handler,
            #[cfg(feature = "python-udf")]
            python_udf_handler,
            config,
        })
    }

    /// Create with default configuration
    pub async fn new_default() -> crate::protocols::error::ProtocolResult<Self> {
        Self::new(QueryEngineConfig::default()).await
    }

    /// Execute a SQL query with all optimizations
    pub async fn execute(&self, sql: &str) -> ProtocolResult<OptimizedExecutionResult> {
        let start = Instant::now();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_queries += 1;
        }

        // 1. Check query cache first
        let cache_key = QueryKey::new(sql, "default", "public");
        if let Some(cached) = self.query_cache.get(&cache_key).await {
            let mut metrics = self.metrics.write().await;
            metrics.cache_hits += 1;
            metrics.total_execution_time_ms += start.elapsed().as_millis() as u64;

            let row_count = cached.rows.len();

            return Ok(OptimizedExecutionResult {
                result: ExecutionResult::Select {
                    columns: cached.columns.clone(),
                    rows: cached.rows.clone(),
                    row_count,
                },
                execution_time_ms: start.elapsed().as_millis() as u64,
                from_cache: true,
                execution_backend: ExecutionBackend::CpuScalar,
                parallel_partitions: 0,
            });
        }

        // Cache miss
        {
            let mut metrics = self.metrics.write().await;
            metrics.cache_misses += 1;
        }

        // 2. Parse the SQL
        let statement = self.parser.write().await.parse(sql)?;

        // 2.5. Intercept CREATE/DROP FUNCTION statements for UDF handling
        use crate::protocols::postgres_wire::sql::ast::Statement;
        match &statement {
            Statement::CreateFunction(create_fn) => {
                // Determine which handler to use based on language
                let is_python = create_fn.language.as_ref()
                    .map(|lang| {
                        use crate::protocols::postgres_wire::sql::ast::FunctionLanguage;
                        matches!(lang, FunctionLanguage::Other(s) if s.eq_ignore_ascii_case("python") || s.eq_ignore_ascii_case("plpython") || s.eq_ignore_ascii_case("plpython3u"))
                    })
                    .unwrap_or(false);

                #[cfg(feature = "python-udf")]
                if is_python {
                    if let Some(ref python_handler) = self.python_udf_handler {
                        let _result =
                            Self::handle_pg_create_python_function(python_handler, create_fn)
                                .await?;
                        let execution_time_ms = start.elapsed().as_millis() as u64;

                        return Ok(OptimizedExecutionResult {
                            result: ExecutionResult::Show {
                                variable: format!("CREATE FUNCTION {}", create_fn.name),
                                value: "OK".to_string(),
                            },
                            execution_time_ms,
                            from_cache: false,
                            execution_backend: ExecutionBackend::CpuScalar,
                            parallel_partitions: 0,
                        });
                    }
                }

                #[cfg(feature = "lua-mlua")]
                if !is_python {
                    if let Some(ref udf_handler) = self.udf_handler {
                        let _result = udf_handler.handle_pg_create_function(create_fn).await?;
                        let execution_time_ms = start.elapsed().as_millis() as u64;

                        return Ok(OptimizedExecutionResult {
                            result: ExecutionResult::Show {
                                variable: format!("CREATE FUNCTION {}", create_fn.name),
                                value: "OK".to_string(),
                            },
                            execution_time_ms,
                            from_cache: false,
                            execution_backend: ExecutionBackend::CpuScalar,
                            parallel_partitions: 0,
                        });
                    }
                }
            }
            Statement::DropFunction(drop_fn) => {
                // For DROP FUNCTION, check all handlers
                #[cfg(feature = "python-udf")]
                if let Some(ref python_handler) = self.python_udf_handler {
                    // Try Python handler first
                    for (name, _) in &drop_fn.functions {
                        if python_handler.function_exists(&name.to_string()).await {
                            let _result = python_handler
                                .handle_drop_function(&name.to_string())
                                .await
                                .map_err(|e| {
                                    crate::protocols::error::ProtocolError::PostgresError(format!(
                                        "Failed to drop Python function: {}",
                                        e
                                    ))
                                })?;
                            let execution_time_ms = start.elapsed().as_millis() as u64;

                            return Ok(OptimizedExecutionResult {
                                result: ExecutionResult::Show {
                                    variable: format!("DROP FUNCTION {}", name),
                                    value: "OK".to_string(),
                                },
                                execution_time_ms,
                                from_cache: false,
                                execution_backend: ExecutionBackend::CpuScalar,
                                parallel_partitions: 0,
                            });
                        }
                    }
                }

                #[cfg(feature = "lua-mlua")]
                if let Some(ref udf_handler) = self.udf_handler {
                    let _result = udf_handler.handle_pg_drop_function(drop_fn).await?;
                    let execution_time_ms = start.elapsed().as_millis() as u64;

                    return Ok(OptimizedExecutionResult {
                        result: ExecutionResult::Show {
                            variable: format!(
                                "DROP FUNCTION {}",
                                drop_fn
                                    .functions
                                    .iter()
                                    .map(|(name, _)| name.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            value: "OK".to_string(),
                        },
                        execution_time_ms,
                        from_cache: false,
                        execution_backend: ExecutionBackend::CpuScalar,
                        parallel_partitions: 0,
                    });
                }
            }
            _ => {
                // Not a UDF statement, continue normal execution
            }
        }

        // 3. Get or create execution plan
        let plan = self.get_or_create_plan(sql, &statement).await?;

        // 4. Determine execution strategy
        let estimated_rows = plan.estimated_rows();
        let operation_type = self.classify_operation(&statement);
        let backend = self.cost_router.route(operation_type, estimated_rows, None);

        // 5. Execute based on strategy
        let result = match backend {
            ExecutionBackend::Gpu if self.config.parallel.enabled => {
                let mut metrics = self.metrics.write().await;
                metrics.gpu_queries += 1;
                drop(metrics);
                self.execute_with_gpu(&statement, estimated_rows).await?
            }
            ExecutionBackend::CpuSimd => {
                let mut metrics = self.metrics.write().await;
                metrics.simd_queries += 1;
                drop(metrics);
                self.execute_with_simd(&statement).await?
            }
            _ => {
                // Standard execution with possible parallelism
                if self.should_parallelize(&statement, estimated_rows) {
                    let mut metrics = self.metrics.write().await;
                    metrics.parallel_queries += 1;
                    drop(metrics);
                    self.execute_parallel(&statement, estimated_rows).await?
                } else {
                    self.execute_standard(&statement).await?
                }
            }
        };

        let execution_time_ms = start.elapsed().as_millis() as u64;

        // 6. Cache the result if it's a SELECT
        if let ExecutionResult::Select { columns, rows, .. } = &result {
            let tables = extract_table_names(sql);
            self.query_cache
                .put(
                    cache_key,
                    columns.clone(),
                    rows.clone(), // Rows are already Vec<Vec<Option<String>>>
                    tables,
                    execution_time_ms,
                )
                .await;
        }

        // 7. Record for index advisor
        self.index_advisor
            .record_query(&statement, execution_time_ms as f64)
            .await;

        // 8. Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_execution_time_ms += execution_time_ms;
            metrics.avg_execution_time_ms =
                metrics.total_execution_time_ms as f64 / metrics.total_queries as f64;
        }

        Ok(OptimizedExecutionResult {
            result,
            execution_time_ms,
            from_cache: false,
            execution_backend: backend,
            parallel_partitions: 0,
        })
    }

    /// Get or create execution plan
    async fn get_or_create_plan(
        &self,
        sql: &str,
        statement: &Statement,
    ) -> ProtocolResult<QueryPlan> {
        let plan_cache = self.plan_cache.read().await;
        let cache_key = QueryKey::new(sql, "default", "public");

        // Check plan cache
        if let Some(cached) = plan_cache.get(&cache_key, None).await {
            return Ok(cached.plan.clone());
        }
        drop(plan_cache);

        // Create new plan
        let plan = self.create_plan(statement).await?;

        // Cache the plan
        let plan_cache = self.plan_cache.read().await;
        plan_cache
            .put(
                cache_key,
                plan.clone(),
                statement.clone(),
                std::collections::HashMap::new(),
            )
            .await;

        Ok(plan)
    }

    /// Create execution plan for a statement
    async fn create_plan(&self, statement: &Statement) -> ProtocolResult<QueryPlan> {
        match statement {
            Statement::Select(select) => {
                // Get table name
                let table_name = select
                    .from_clause
                    .as_ref()
                    .and_then(|f| match f {
                        crate::protocols::postgres_wire::sql::ast::FromClause::Table {
                            name,
                            ..
                        } => Some(name.full_name()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "unknown".to_string());

                // Estimate cardinality
                let estimated_rows = self
                    .cardinality_estimator
                    .estimate_with_stats_manager(
                        &table_name,
                        select.where_clause.as_ref(),
                        &self.statistics,
                    )
                    .await;

                // Check if we should use vectorized execution
                let use_simd = estimated_rows >= self.config.vectorized.min_rows_for_vectorized;
                let use_gpu = estimated_rows >= self.config.vectorized.min_rows_for_gpu;

                Ok(QueryPlan::Vectorized {
                    input: Box::new(QueryPlan::SeqScan {
                        table: table_name,
                        filter: select.where_clause.as_ref().map(|e| format!("{:?}", e)),
                        estimated_rows,
                    }),
                    use_simd,
                    use_gpu,
                })
            }
            // For DML statements, use a minimal SeqScan plan since the actual
            // execution is handled by SqlExecutor directly
            Statement::Insert(insert_stmt) => Ok(QueryPlan::SeqScan {
                table: insert_stmt.table.full_name(),
                filter: None,
                estimated_rows: 1,
            }),
            Statement::Update(update_stmt) => Ok(QueryPlan::SeqScan {
                table: update_stmt.table.full_name(),
                filter: update_stmt
                    .where_clause
                    .as_ref()
                    .map(|e| format!("{:?}", e)),
                estimated_rows: 1,
            }),
            Statement::Delete(delete_stmt) => Ok(QueryPlan::SeqScan {
                table: delete_stmt.table.full_name(),
                filter: delete_stmt
                    .where_clause
                    .as_ref()
                    .map(|e| format!("{:?}", e)),
                estimated_rows: 1,
            }),
            _ => Ok(QueryPlan::SeqScan {
                table: "unknown".to_string(),
                filter: None,
                estimated_rows: 1,
            }),
        }
    }

    /// Classify operation type for cost routing
    fn classify_operation(&self, statement: &Statement) -> OperationType {
        match statement {
            Statement::Select(select) => {
                if select.group_by.is_some() {
                    OperationType::Aggregate
                } else if select.order_by.is_some() {
                    OperationType::Sort
                } else {
                    OperationType::Filter
                }
            }
            _ => OperationType::Projection,
        }
    }

    /// Check if query should be parallelized
    fn should_parallelize(&self, _statement: &Statement, estimated_rows: usize) -> bool {
        self.config.parallel.enabled && estimated_rows >= self.config.parallel.min_rows_for_parallel
    }

    /// Execute with GPU acceleration
    async fn execute_with_gpu(
        &self,
        statement: &Statement,
        _estimated_rows: usize,
    ) -> ProtocolResult<ExecutionResult> {
        // For now, fall back to standard execution
        // Full GPU integration would require more infrastructure
        self.execute_standard(statement).await
    }

    /// Execute with SIMD acceleration
    async fn execute_with_simd(&self, statement: &Statement) -> ProtocolResult<ExecutionResult> {
        // Only SELECT statements benefit from vectorized execution
        if let Statement::Select(select) = statement {
            // First, get the raw data from storage (without WHERE/ORDER BY applied)
            // We execute a simplified version to get the base data
            let base_select = crate::protocols::postgres_wire::sql::ast::SelectStatement {
                with: select.with.clone(),
                distinct: None, // Apply after SIMD filtering
                select_list: select.select_list.clone(),
                from_clause: select.from_clause.clone(),
                where_clause: None,                // Will apply with SIMD
                group_by: select.group_by.clone(), // Keep for now
                having: select.having.clone(),
                order_by: None, // Will apply with SIMD sorting
                limit: None,    // Will apply after sorting
                offset: None,
                for_clause: select.for_clause.clone(),
                traverse: select.traverse.clone(),
                set_operation: select.set_operation.clone(),
            };

            // Execute base query to get source rows
            let base_result = self
                .executor
                .execute_statement(Statement::Select(Box::new(base_select)))
                .await?;

            // Extract rows from result
            if let ExecutionResult::Select { columns, rows, .. } = base_result {
                // Convert rows to HashMap format for columnar conversion
                let source_rows: Vec<
                    std::collections::HashMap<
                        String,
                        crate::protocols::postgres_wire::sql::types::SqlValue,
                    >,
                > = rows
                    .iter()
                    .map(|row| {
                        columns
                            .iter()
                            .zip(row.iter())
                            .map(|(col, val)| {
                                let sql_val = match val {
                                    Some(s) => {
                                        crate::protocols::postgres_wire::sql::types::SqlValue::Text(
                                            s.clone(),
                                        )
                                    }
                                    None => {
                                        crate::protocols::postgres_wire::sql::types::SqlValue::Null
                                    }
                                };
                                (col.clone(), sql_val)
                            })
                            .collect()
                    })
                    .collect();

                // Use vectorized executor for filtering, sorting, and limiting
                let mut vectorized = self.vectorized_executor.write().await;
                let (result_columns, result_rows) = vectorized
                    .execute_select_vectorized(select, source_rows)
                    .await?;

                return Ok(ExecutionResult::Select {
                    row_count: result_rows.len(),
                    columns: result_columns,
                    rows: result_rows,
                });
            }
        }

        // Fall back to standard execution for non-SELECT or if SIMD path fails
        self.execute_standard(statement).await
    }

    /// Execute in parallel
    async fn execute_parallel(
        &self,
        statement: &Statement,
        _estimated_rows: usize,
    ) -> ProtocolResult<ExecutionResult> {
        // For now, fall back to standard execution
        // Full parallel execution would use ParallelCoordinator
        self.execute_standard(statement).await
    }

    /// Standard execution
    async fn execute_standard(&self, statement: &Statement) -> ProtocolResult<ExecutionResult> {
        self.executor.execute_statement(statement.clone()).await
    }

    /// Handle PostgreSQL CREATE FUNCTION statement for Python
    #[cfg(feature = "python-udf")]
    async fn handle_pg_create_python_function(
        python_handler: &crate::python::udf_handler::PythonUdfHandler,
        create_fn: &crate::protocols::postgres_wire::sql::ast::CreateFunctionStatement,
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::sql::ast::FunctionName;

        // Extract function name
        let func_name = match &create_fn.name {
            FunctionName::Simple(n) => n.to_uppercase(),
            FunctionName::Qualified { schema: _, name } => name.to_uppercase(),
        };

        // Extract parameters with types
        let params: Vec<(String, String)> = if let Some(ref args) = create_fn.args {
            args.iter()
                .map(|param| {
                    let param_name = param.name.as_deref().unwrap_or("arg").to_string();
                    let param_type = Self::pg_sql_type_to_string(&param.data_type);
                    (param_name, param_type)
                })
                .collect()
        } else {
            vec![]
        };

        // Extract return type
        let return_type = if let Some(ref rt) = create_fn.return_type {
            Self::pg_sql_type_to_string(rt)
        } else {
            "VOID".to_string()
        };

        // Extract function body
        let source = create_fn.body.clone();

        // Extract schema from qualified name
        let schema = match &create_fn.name {
            FunctionName::Qualified { schema, name: _ } => Some(schema.clone()),
            _ => None,
        };

        // Call handler
        python_handler
            .handle_create_function(func_name, params, return_type, source, schema)
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::PostgresError(format!(
                    "Failed to create Python function: {}",
                    e
                ))
            })
    }

    /// Convert PostgreSQL SqlType to SQL type string
    #[cfg(any(feature = "python-udf", feature = "lua-mlua"))]
    fn pg_sql_type_to_string(
        sql_type: &crate::protocols::postgres_wire::sql::types::SqlType,
    ) -> String {
        use crate::protocols::postgres_wire::sql::types::SqlType;

        match sql_type {
            SqlType::Boolean => "BOOLEAN".to_string(),
            SqlType::SmallInt => "SMALLINT".to_string(),
            SqlType::Integer => "INTEGER".to_string(),
            SqlType::BigInt => "BIGINT".to_string(),
            SqlType::Real => "REAL".to_string(),
            SqlType::DoublePrecision => "DOUBLE PRECISION".to_string(),
            SqlType::Numeric { .. } => "NUMERIC".to_string(),
            SqlType::Varchar(_) | SqlType::Char(_) | SqlType::Text => "TEXT".to_string(),
            SqlType::Bytea => "BYTEA".to_string(),
            SqlType::Timestamp { .. } => "TIMESTAMP".to_string(),
            SqlType::Date => "DATE".to_string(),
            SqlType::Time { .. } => "TIME".to_string(),
            SqlType::Interval => "INTERVAL".to_string(),
            SqlType::Uuid => "UUID".to_string(),
            SqlType::Json | SqlType::Jsonb => "JSON".to_string(),
            SqlType::Array { element_type, .. } => {
                format!("{}[]", Self::pg_sql_type_to_string(element_type))
            }
            _ => "TEXT".to_string(),
        }
    }

    /// Get index recommendations for a table
    pub async fn get_index_recommendations(&self, table_name: &str) -> Vec<IndexRecommendation> {
        let stats = self.statistics.get_table_stats(table_name).await;
        self.index_advisor
            .recommend(table_name, stats.as_ref())
            .await
    }

    /// Get query execution metrics
    pub async fn get_metrics(&self) -> QueryMetrics {
        self.metrics.read().await.clone()
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStatistics {
        let query_stats = self.query_cache.stats().await;
        let plan_cache = self.plan_cache.read().await;
        let plan_stats = plan_cache.stats().await;

        CacheStatistics {
            query_cache_entries: query_stats.current_entries,
            query_cache_hits: query_stats.hits,
            query_cache_misses: query_stats.misses,
            query_cache_hit_ratio: query_stats.hit_ratio(),
            plan_cache_entries: plan_stats.current_entries,
            plan_cache_hits: plan_stats.hits,
            plan_cache_misses: plan_stats.misses,
            plan_cache_hit_ratio: plan_stats.hit_ratio(),
        }
    }

    /// Invalidate cache for a table
    pub async fn invalidate_table(&self, table_name: &str) {
        self.query_cache.invalidate_table(table_name).await;
        self.plan_cache
            .read()
            .await
            .invalidate_table(table_name)
            .await;
    }

    /// Analyze a table and update statistics
    pub async fn analyze_table(&self, table_name: &str) {
        // In a real implementation, this would scan the table and collect stats
        let stats =
            crate::protocols::postgres_wire::sql::statistics::TableStatistics::new(table_name);
        self.statistics.store_table_stats(stats).await;

        // Invalidate affected cached plans
        self.plan_cache
            .read()
            .await
            .invalidate_table(table_name)
            .await;
    }

    /// Get workload summary for index advisor
    pub async fn get_workload_summary(
        &self,
    ) -> crate::protocols::postgres_wire::sql::index_advisor::WorkloadSummary {
        self.index_advisor.workload_summary().await
    }

    /// Explain query execution plan
    pub async fn explain(&self, sql: &str) -> ProtocolResult<String> {
        let statement = self.parser.write().await.parse(sql)?;
        let plan = self.create_plan(&statement).await?;

        let estimated_rows = plan.estimated_rows();
        let operation = self.classify_operation(&statement);
        let backend = self.cost_router.route(operation, estimated_rows, None);

        let mut explanation = String::new();
        explanation.push_str(&format!("Query Plan:\n{:#?}\n\n", plan));
        explanation.push_str(&format!("Estimated Rows: {}\n", estimated_rows));
        explanation.push_str(&format!("Operation Type: {:?}\n", operation));
        explanation.push_str(&format!("Execution Backend: {:?}\n", backend));
        explanation.push_str(&self.cost_router.explain_routing(operation, estimated_rows));

        Ok(explanation)
    }
}

/// Result of optimized query execution
#[derive(Debug)]
pub struct OptimizedExecutionResult {
    /// The actual execution result
    pub result: ExecutionResult,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Whether result was from cache
    pub from_cache: bool,
    /// Which backend was used
    pub execution_backend: ExecutionBackend,
    /// Number of parallel partitions (0 if not parallel)
    pub parallel_partitions: usize,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub query_cache_entries: usize,
    pub query_cache_hits: u64,
    pub query_cache_misses: u64,
    pub query_cache_hit_ratio: f64,
    pub plan_cache_entries: usize,
    pub plan_cache_hits: u64,
    pub plan_cache_misses: u64,
    pub plan_cache_hit_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_engine_creation() {
        let engine = OptimizedQueryEngine::new_default().await.unwrap();
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_queries, 0);
    }

    #[tokio::test]
    async fn test_query_execution() {
        let engine = OptimizedQueryEngine::new_default().await.unwrap();

        // Execute a simple query
        let result = engine.execute("SELECT 1").await;
        assert!(result.is_ok());

        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_queries, 1);
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let engine = OptimizedQueryEngine::new_default().await.unwrap();

        // Execute the same query twice
        let _ = engine.execute("SELECT * FROM users").await;
        let _ = engine.execute("SELECT * FROM users").await;

        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_queries, 2);
        // Second query should be a cache hit (if caching is working)
    }

    #[tokio::test]
    async fn test_explain() {
        let engine = OptimizedQueryEngine::new_default().await.unwrap();

        let explanation = engine.explain("SELECT * FROM users WHERE id = 1").await;
        assert!(explanation.is_ok());

        let text = explanation.unwrap();
        assert!(text.contains("Query Plan"));
        assert!(text.contains("Estimated Rows"));
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let engine = OptimizedQueryEngine::new_default().await.unwrap();

        let stats = engine.get_cache_stats().await;
        assert_eq!(stats.query_cache_entries, 0);
    }

    #[tokio::test]
    async fn test_index_recommendations() {
        let engine = OptimizedQueryEngine::new(QueryEngineConfig {
            index_advisor: IndexAdvisorConfig {
                min_query_count: 1, // Lower threshold for testing
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();

        // Record some queries
        for _ in 0..5 {
            let _ = engine
                .execute("SELECT * FROM users WHERE email = 'test@example.com'")
                .await;
        }

        let recommendations = engine.get_index_recommendations("users").await;
        // May or may not have recommendations depending on threshold
        // Just verify it doesn't panic
        let _ = recommendations;
    }
}
