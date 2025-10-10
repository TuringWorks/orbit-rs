//! # OrbitQL - Unified Multi-Model Query Language
//!
//! OrbitQL is a unified query language that combines document, graph, time-series,
//! and key-value operations in a single query, inspired by SurrealQL but optimized
//! for distributed actor systems.
//!
//! ## Features
//!
//! - **Multi-Model Operations**: Query across documents, graphs, and time-series in one query
//! - **Cross-Model JOINs**: Relate data between different data models seamlessly
//! - **Distributed Execution**: Query planning optimized for actor-based systems
//! - **Real-time Subscriptions**: Live query support with change notifications
//! - **ACID Transactions**: Multi-model transaction support
//!
//! ## Example Usage
//!
//! ```orbitql
//! SELECT
//!     user.name,
//!     user.profile.* AS profile,
//!     ->follows->user.name AS friends,
//!     metrics[cpu_usage WHERE timestamp > time::now() - 1h] AS recent_cpu
//! FROM users AS user
//! WHERE user.active = true AND user.age > 18
//! RELATE user->viewed->product SET timestamp = time::now()
//! FETCH friends, recent_cpu;
//! ```

pub mod ast;
pub mod benchmark;
pub mod cache;
pub mod cost_based_planner;
pub mod executor;
pub mod index_recommendation;
pub mod index_recommendation;
pub mod index_selection;
pub mod index_selection;
pub mod lexer;
pub mod lsp;
pub mod optimizer;
pub mod parallel_execution;
pub mod parser;
pub mod planner;
pub mod profiler;
pub mod query_cache;
pub mod spatial;
pub mod statistics;
pub mod streaming;
pub mod vectorized_execution;

// Re-export core types
pub use ast::*;
pub use cache::{
    CacheConfig, CacheHealth, CacheKey, CacheStatistics, CachedQueryExecutor, InvalidationEvent,
    QueryCache,
};
pub use distributed::{
    ActorNodeInfo, DistributedExecutionContext, DistributedExecutionPlan, DistributedQueryExecutor,
    DistributedQueryPlanner, NodeCapabilities, NodeType,
};
pub use executor::{ExecutionError, QueryExecutor, QueryResult};
pub use lexer::{Lexer, Token, TokenType};
pub use lsp::{
    create_default_schema, start_lsp_server, ColumnInfo, FunctionInfo, IndexInfo, LSPConfig,
    OrbitQLLanguageServer, ParameterInfo, SchemaInfo, TableInfo, TableType,
};
pub use optimizer::{OptimizationRule, QueryOptimizer};
pub use parser::{ParseError, Parser};
pub use planner::{ExecutionPlan, PlanNode, QueryPlanner};
pub use profiler::{
    ExecutionPhase, ExecutionStats, OptimizationSuggestion, PerformanceBottleneck, ProfilerConfig,
    QueryProfile, QueryProfiler, ResourceUsage,
};
pub use spatial::{
    SpatialFunctionCategory, SpatialFunctionInfo, SpatialFunctionRegistry, SpatialParameter,
    SpatialParameterType, SpatialReturnType, SPATIAL_FUNCTIONS,
};
pub use streaming::{
    ChangeNotification, ChangeType, LiveQueryConfig, LiveQuerySubscription, QueryResultStream,
    StreamingConfig, StreamingQueryBuilder, StreamingQueryExecutor, StreamingRow,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Query execution parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParams {
    params: HashMap<String, QueryValue>,
}

impl QueryParams {
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    pub fn set<T: Into<QueryValue>>(mut self, key: &str, value: T) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&QueryValue> {
        self.params.get(key)
    }
}

impl Default for QueryParams {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified value type for OrbitQL
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<QueryValue>),
    Object(HashMap<String, QueryValue>),
    DateTime(DateTime<Utc>),
    Duration(std::time::Duration),
    Uuid(Uuid),
}

impl From<bool> for QueryValue {
    fn from(v: bool) -> Self {
        QueryValue::Boolean(v)
    }
}

impl From<i32> for QueryValue {
    fn from(v: i32) -> Self {
        QueryValue::Integer(v as i64)
    }
}

impl From<i64> for QueryValue {
    fn from(v: i64) -> Self {
        QueryValue::Integer(v)
    }
}

impl From<f32> for QueryValue {
    fn from(v: f32) -> Self {
        QueryValue::Float(v as f64)
    }
}

impl From<f64> for QueryValue {
    fn from(v: f64) -> Self {
        QueryValue::Float(v)
    }
}

impl From<String> for QueryValue {
    fn from(v: String) -> Self {
        QueryValue::String(v)
    }
}

impl From<&str> for QueryValue {
    fn from(v: &str) -> Self {
        QueryValue::String(v.to_string())
    }
}

impl From<DateTime<Utc>> for QueryValue {
    fn from(v: DateTime<Utc>) -> Self {
        QueryValue::DateTime(v)
    }
}

impl From<Uuid> for QueryValue {
    fn from(v: Uuid) -> Self {
        QueryValue::Uuid(v)
    }
}

/// Query execution context
#[derive(Debug, Clone, Default)]
pub struct QueryContext {
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub transaction_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub trace_enabled: bool,
}

/// Query execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryStats {
    pub execution_time_ms: u64,
    pub rows_examined: u64,
    pub rows_returned: u64,
    pub index_hits: u64,
    pub cache_hits: u64,
    pub network_calls: u64,
}

/// Main OrbitQL engine with profiling support
pub struct OrbitQLEngine {
    pub lexer: Lexer,
    pub parser: Parser,
    pub optimizer: QueryOptimizer,
    pub planner: QueryPlanner,
    pub executor: QueryExecutor,
    pub profiler: Option<QueryProfiler>,
}

impl OrbitQLEngine {
    pub fn new() -> Self {
        Self {
            lexer: Lexer::new(),
            parser: Parser::new(),
            optimizer: QueryOptimizer::new(),
            planner: QueryPlanner::new(),
            executor: QueryExecutor::new(),
            profiler: None,
        }
    }

    /// Create engine with profiling enabled
    pub fn with_profiler(mut self, config: ProfilerConfig) -> Self {
        self.profiler = Some(QueryProfiler::new(config));
        self
    }

    /// Enable or disable profiling
    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.profiler.is_none() {
                self.profiler = Some(QueryProfiler::new(ProfilerConfig::default()));
            }
        } else {
            self.profiler = None;
        }
    }

    /// Execute an OrbitQL query with optional profiling
    pub async fn execute(
        &mut self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<QueryResult, ExecutionError> {
        if self.profiler.is_some() {
            let profile_id = self
                .profiler
                .as_mut()
                .unwrap()
                .start_profiling(query.to_string());
            self.execute_with_profiling_steps(query, params, context, profile_id)
                .await
        } else {
            self.execute_without_profiling(query, params, context).await
        }
    }

    /// Execute query without profiling (faster path)
    async fn execute_without_profiling(
        &mut self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<QueryResult, ExecutionError> {
        // Tokenize the query
        let tokens = self.lexer.tokenize(query)?;

        // Parse into AST
        let ast = self.parser.parse(tokens)?;

        // Optimize the query
        let optimized_ast = self.optimizer.optimize(ast)?;

        // Create execution plan
        let plan = self.planner.plan(optimized_ast)?;

        // Execute the plan
        self.executor.execute(plan, params, context).await
    }

    /// Execute query with detailed profiling (new approach)
    async fn execute_with_profiling_steps(
        &mut self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
        profile_id: Uuid,
    ) -> Result<QueryResult, ExecutionError> {
        // Tokenize the query with profiling
        self.profiler.as_mut().unwrap().start_phase(
            profile_id,
            "Lexing",
            "Tokenizing query string",
        );
        let tokens = self.lexer.tokenize(query)?;
        self.profiler.as_mut().unwrap().end_phase(profile_id);

        // Parse into AST with profiling
        self.profiler.as_mut().unwrap().start_phase(
            profile_id,
            "Parsing",
            "Building abstract syntax tree",
        );
        let ast = self.parser.parse(tokens)?;
        self.profiler.as_mut().unwrap().end_phase(profile_id);

        // Optimize the query with profiling
        self.profiler.as_mut().unwrap().start_phase(
            profile_id,
            "Optimization",
            "Applying query optimization rules",
        );
        let optimized_ast = self.optimizer.optimize(ast)?;
        self.profiler.as_mut().unwrap().end_phase(profile_id);

        // Create execution plan with profiling
        self.profiler.as_mut().unwrap().start_phase(
            profile_id,
            "Planning",
            "Creating execution plan",
        );
        let plan = self.planner.plan(optimized_ast)?;
        self.profiler.as_mut().unwrap().end_phase(profile_id);

        // Execute the plan with profiling
        self.profiler.as_mut().unwrap().start_phase(
            profile_id,
            "Execution",
            "Executing query plan",
        );
        let result = self.executor.execute(plan, params, context).await;
        self.profiler.as_mut().unwrap().end_phase(profile_id);

        // Finish profiling (this consumes the profile)
        if let Some(_profile) = self.profiler.as_mut().unwrap().finish_profiling(profile_id) {
            // Profile data is available but we don't return it with the result
            // It could be stored for later analysis or returned separately
        }

        result
    }

    /// Explain query execution plan
    pub async fn explain(
        &mut self,
        query: &str,
        _params: QueryParams,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let tokens = self.lexer.tokenize(query)?;
        let ast = self.parser.parse(tokens)?;
        let optimized_ast = self.optimizer.optimize(ast)?;
        Ok(self.planner.plan(optimized_ast)?)
    }

    /// Execute query with profiling and return detailed analysis
    pub async fn explain_analyze(
        &mut self,
        query: &str,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<(QueryResult, QueryProfile), ExecutionError> {
        // Ensure profiling is enabled temporarily
        let had_profiler = self.profiler.is_some();
        if !had_profiler {
            self.profiler = Some(QueryProfiler::new(ProfilerConfig::default()));
        }

        let profile_id = self
            .profiler
            .as_mut()
            .unwrap()
            .start_profiling(query.to_string());
        let profiler = self.profiler.as_mut().unwrap();

        // Execute with profiling (similar to execute_with_profiling but capture profile)
        profiler.start_phase(profile_id, "Lexing", "Tokenizing query string");
        let tokens = self.lexer.tokenize(query)?;
        profiler.end_phase(profile_id);

        profiler.start_phase(profile_id, "Parsing", "Building abstract syntax tree");
        let ast = self.parser.parse(tokens)?;
        profiler.end_phase(profile_id);

        profiler.start_phase(
            profile_id,
            "Optimization",
            "Applying query optimization rules",
        );
        let optimized_ast = self.optimizer.optimize(ast)?;
        profiler.end_phase(profile_id);

        profiler.start_phase(profile_id, "Planning", "Creating execution plan");
        let plan = self.planner.plan(optimized_ast)?;
        profiler.end_phase(profile_id);

        profiler.start_phase(profile_id, "Execution", "Executing query plan");
        let result = self.executor.execute(plan, params, context).await;
        profiler.end_phase(profile_id);

        // Get the profile before finishing
        let profile = profiler.finish_profiling(profile_id).ok_or_else(|| {
            ExecutionError::Internal("Failed to capture query profile".to_string())
        })?;

        // Remove profiler if it wasn't there originally
        if !had_profiler {
            self.profiler = None;
        }

        match result {
            Ok(query_result) => Ok((query_result, profile)),
            Err(e) => Err(e),
        }
    }

    /// Get profiling report for the last executed query (if available)
    pub fn get_last_profile_report(&self) -> Option<String> {
        // This would need to be implemented to store the last profile
        // For now, return None as profiles are consumed
        None
    }

    /// Validate query syntax without execution
    pub fn validate(&mut self, query: &str) -> Result<(), ParseError> {
        let tokens = self.lexer.tokenize(query)?;
        let _ast = self.parser.parse(tokens)?;
        Ok(())
    }
}

impl Default for OrbitQLEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    pub mod integration_tests;

    // Original unit tests
    use super::*;

    #[test]
    fn test_query_value_conversions() {
        assert_eq!(QueryValue::from(42), QueryValue::Integer(42));
        assert_eq!(
            QueryValue::from(std::f64::consts::PI),
            QueryValue::Float(std::f64::consts::PI)
        );
        assert_eq!(
            QueryValue::from("hello"),
            QueryValue::String("hello".to_string())
        );
        assert_eq!(QueryValue::from(true), QueryValue::Boolean(true));
    }

    #[test]
    fn test_query_params() {
        let params = QueryParams::new()
            .set("name", "Alice")
            .set("age", 30)
            .set("active", true);

        assert_eq!(
            params.get("name"),
            Some(&QueryValue::String("Alice".to_string()))
        );
        assert_eq!(params.get("age"), Some(&QueryValue::Integer(30)));
        assert_eq!(params.get("active"), Some(&QueryValue::Boolean(true)));
    }

    #[tokio::test]
    async fn test_orbitql_engine_creation() {
        let mut engine = OrbitQLEngine::new();

        // Test basic validation - this should succeed with our basic parser implementation
        let result = engine.validate("SELECT * FROM users");
        // The parser should successfully parse this basic query
        assert!(
            result.is_ok(),
            "Expected parsing to succeed, got: {:?}",
            result
        );
    }
}
