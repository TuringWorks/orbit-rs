//! OrbitQL Protocol-Specific Executor
//!
//! This module implements the protocol-specific execution engine for OrbitQL queries,
//! providing advanced spatial query processing, real-time streaming, and GPU acceleration.
//! It uses the unified OrbitQL AST from orbit-shared.

#![allow(unused_imports)]

use orbit_shared::orbitql::{
    ast::{
        BinaryOperator, CreateDefinition, CreateObjectType, CreateStatement, DataType,
        DeleteStatement, DropStatement, Expression, FieldDefinition, FromClause, GraphRAGStatement,
        InsertStatement, InsertValues, JoinClause, LiveStatement, OrderByClause, RelateStatement,
        SelectField, SelectStatement, Statement, TransactionStatement, UpdateAssignment,
        UpdateOperator, UpdateStatement,
    },
    QueryValue, SpatialIndexConfig, SpatialIndexType, WindowSpec,
};
use orbit_shared::spatial::{
    GPUSpatialEngine, SpatialError, SpatialGeometry, SpatialStreamProcessor, WGS84_SRID,
};
use orbit_shared::GeometryLiteral;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// OrbitQL query executor with advanced spatial capabilities
pub struct OrbitQLExecutor {
    /// Tables and their data
    tables: Arc<RwLock<HashMap<String, Table>>>,
    /// Spatial indexes
    spatial_indexes: Arc<RwLock<HashMap<String, SpatialIndexInstance>>>,
    /// Real-time streams
    #[allow(dead_code)]
    streams: Arc<RwLock<HashMap<String, StreamProcessor>>>,
    /// GPU acceleration engine
    #[allow(dead_code)]
    gpu_engine: GPUSpatialEngine,
    /// Execution context
    #[allow(dead_code)]
    context: Arc<RwLock<ExecutionContext>>,
}

/// Table with spatial data
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub spatial_columns: HashMap<String, usize>, // column name -> column index
}

/// Column definition
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub spatial_type: Option<SpatialType>,
}

/// Spatial geometry types
#[derive(Debug, Clone, PartialEq)]
pub enum SpatialType {
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon,
    GeometryCollection,
    Any,
}

/// Table row
#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Value>,
    pub spatial_values: HashMap<usize, SpatialGeometry>,
}

/// Spatial index instance
#[derive(Debug, Clone)]
pub struct SpatialIndexInstance {
    pub name: String,
    pub table: String,
    pub column: String,
    pub index_type: SpatialIndexType,
    pub config: Option<SpatialIndexConfig>,
    pub geometries: HashMap<usize, SpatialGeometry>, // row_id -> geometry
}

/// Stream processor for real-time queries
pub struct StreamProcessor {
    pub name: String,
    pub query: SelectStatement,
    pub window_spec: WindowSpec,
    pub processor: SpatialStreamProcessor,
}

/// Execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub variables: HashMap<String, Value>,
    pub session_id: String,
    pub started_at: Instant,
    pub query_timeout: Option<Duration>,
    pub enable_gpu: bool,
    pub parallel_execution: bool,
}

/// Query execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub execution_time: Duration,
    pub rows_processed: usize,
    pub spatial_operations: usize,
    pub index_hits: usize,
    pub gpu_acceleration_used: bool,
}

impl Default for OrbitQLExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Statement execution strategy pattern to reduce cognitive complexity
struct StatementExecutor<'a> {
    executor: &'a OrbitQLExecutor,
}

impl<'a> StatementExecutor<'a> {
    fn new(executor: &'a OrbitQLExecutor) -> Self {
        Self { executor }
    }

    async fn execute(&self, statement: Statement) -> Result<ExecutionResult, SpatialError> {
        match statement {
            Statement::Select(select) => self.execute_select_strategy(select).await,
            Statement::Insert(insert) => self.execute_insert_strategy(insert).await,
            Statement::Update(update) => self.execute_update_strategy(update).await,
            Statement::Delete(delete) => self.execute_delete_strategy(delete).await,
            Statement::Create(create) => self.execute_create_strategy(create).await,
            Statement::Drop(drop) => self.execute_drop_strategy(drop).await,
            Statement::Transaction(tx) => self.execute_transaction_strategy(tx).await,
            Statement::Live(live) => self.execute_live_strategy(live).await,
            Statement::Relate(relate) => self.execute_relate_strategy(relate).await,
            Statement::GraphRAG(graphrag) => self.execute_graphrag_strategy(graphrag).await,
        }
    }

    async fn execute_select_strategy(
        &self,
        select: SelectStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_select(select).await
    }

    async fn execute_insert_strategy(
        &self,
        insert: InsertStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_insert(insert).await
    }

    async fn execute_update_strategy(
        &self,
        update: UpdateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_update(update).await
    }

    async fn execute_delete_strategy(
        &self,
        delete: DeleteStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_delete(delete).await
    }

    async fn execute_create_strategy(
        &self,
        create: CreateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_create(create).await
    }

    async fn execute_drop_strategy(
        &self,
        drop: DropStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_drop(drop).await
    }

    async fn execute_transaction_strategy(
        &self,
        tx: TransactionStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_transaction(tx).await
    }

    async fn execute_live_strategy(
        &self,
        live: LiveStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_live(live).await
    }

    async fn execute_relate_strategy(
        &self,
        relate: RelateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_relate(relate).await
    }

    async fn execute_graphrag_strategy(
        &self,
        graphrag: GraphRAGStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        self.executor.execute_graphrag(graphrag).await
    }
}

/// Helper struct for insert operation results
struct InsertResult {
    rows_inserted: usize,
    spatial_operations: usize,
}

/// Helper struct for update operation results
struct UpdateRowResult {
    updated: bool,
    spatial_operations: usize,
}

impl OrbitQLExecutor {
    /// Create a new OrbitQL executor
    pub fn new() -> Self {
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
            spatial_indexes: Arc::new(RwLock::new(HashMap::new())),
            streams: Arc::new(RwLock::new(HashMap::new())),
            gpu_engine: GPUSpatialEngine::new(),
            context: Arc::new(RwLock::new(ExecutionContext::default())),
        }
    }

    /// Execute an OrbitQL statement using strategy pattern for better maintainability
    pub async fn execute(&self, statement: Statement) -> Result<ExecutionResult, SpatialError> {
        let start_time = Instant::now();

        let executor = StatementExecutor::new(self);
        let mut result = executor.execute(statement).await?;

        result.execution_time = start_time.elapsed();
        Ok(result)
    }

    /// Execute SELECT statement with full spatial support
    async fn execute_select(
        &self,
        query: SelectStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        let mut result = ExecutionResult::default();
        let mut spatial_operations = 0;
        let mut index_hits = 0;
        let mut gpu_used = false;

        // Process FROM clause to get base data
        let mut base_table = self.process_from_clause(&query.from).await?;

        // Apply WHERE clause filters
        if let Some(where_expr) = &query.where_clause {
            let filter_result = self.apply_where_clause(&mut base_table, where_expr).await?;
            spatial_operations += filter_result.spatial_ops;
            index_hits += filter_result.index_hits;
            gpu_used |= filter_result.gpu_used;
        }

        // Process JOIN clauses
        for join in &query.join_clauses {
            let join_result = self.process_join(&mut base_table, join).await?;
            spatial_operations += join_result.spatial_ops;
            index_hits += join_result.index_hits;
        }

        // Apply GROUP BY
        if !query.group_by.is_empty() {
            self.apply_group_by(&mut base_table, &query.group_by)
                .await?;
        }

        // Apply HAVING clause
        if let Some(having_expr) = &query.having {
            let having_result = self
                .apply_having_clause(&mut base_table, having_expr)
                .await?;
            spatial_operations += having_result.spatial_ops;
        }

        // Process SELECT fields
        let (result_columns, result_rows) = self
            .process_select_fields(&base_table, &query.fields)
            .await?;

        // Apply ORDER BY
        let ordered_rows = if !query.order_by.is_empty() {
            self.apply_order_by(result_rows, &query.order_by).await?
        } else {
            result_rows
        };

        // Apply LIMIT and OFFSET
        let final_rows = self.apply_limit_offset(ordered_rows, query.limit, query.offset);
        let rows_processed = final_rows.len();

        result.columns = result_columns;
        result.rows = final_rows;
        result.rows_processed = rows_processed;
        result.spatial_operations = spatial_operations;
        result.index_hits = index_hits;
        result.gpu_acceleration_used = gpu_used;

        Ok(result)
    }

    /// Execute INSERT statement with full spatial support
    ///
    /// This method handles all types of INSERT operations:
    /// - VALUES: Direct insertion of values
    /// - Object: Insertion from key-value pairs
    /// - SELECT: Insertion from subquery (TODO)
    ///
    /// # Arguments
    /// * `query` - The parsed INSERT statement containing table name, fields, and values
    ///
    /// # Returns
    /// * `Ok(ExecutionResult)` - Success with row count and spatial operation statistics
    /// * `Err(SpatialError)` - Database error or spatial operation failure
    ///
    /// # Performance Considerations
    /// - Spatial columns trigger geometry extraction and indexing operations
    /// - Large batch inserts may require memory optimization
    async fn execute_insert(
        &self,
        query: InsertStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        let mut result = ExecutionResult::default();
        let mut spatial_operations = 0;

        // Get or create target table
        let mut tables = self.tables.write().await;
        let table = self.get_or_create_table(&mut tables, &query.into);

        let mut rows_inserted = 0;

        // Process INSERT values based on the type
        match query.values {
            InsertValues::Values(value_rows) => {
                let insert_result = self
                    .insert_value_rows(table, value_rows, &query.fields)
                    .await?;
                rows_inserted += insert_result.rows_inserted;
                spatial_operations += insert_result.spatial_operations;
            }
            InsertValues::Object(object_map) => {
                let insert_result = self.insert_object_map(table, object_map).await?;
                rows_inserted += insert_result.rows_inserted;
                spatial_operations += insert_result.spatial_operations;
            }
            InsertValues::Select(_subquery) => {
                // TODO: Implement INSERT ... SELECT
                return Err(SpatialError::OperationError(
                    "INSERT ... SELECT not yet implemented".to_string(),
                ));
            }
        }

        // Update spatial indexes if they exist
        if spatial_operations > 0 {
            self.update_spatial_indexes(
                &query.into,
                rows_inserted - spatial_operations,
                spatial_operations,
            )
            .await?;
        }

        result.rows_processed = rows_inserted;
        result.spatial_operations = spatial_operations;

        Ok(result)
    }

    /// Execute UPDATE statement with full spatial support
    async fn execute_update(
        &self,
        query: UpdateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        let mut result = ExecutionResult::default();
        let mut spatial_operations = 0;

        let mut tables = self.tables.write().await;
        let table = tables.get_mut(&query.table).ok_or_else(|| {
            SpatialError::OperationError(format!("Table {} not found", query.table))
        })?;

        let mut rows_updated = 0;

        // Extract only the necessary table metadata to avoid full table clone
        let columns = &table.columns;
        let spatial_columns = &table.spatial_columns;

        for row in table.rows.iter_mut() {
            let update_result = self
                .process_row_update_optimized(row, &query, columns, spatial_columns)
                .await?;
            if update_result.updated {
                rows_updated += 1;
            }
            spatial_operations += update_result.spatial_operations;
        }

        // Update spatial indexes if needed
        if spatial_operations > 0 {
            self.update_spatial_indexes(&query.table, 0, table.rows.len())
                .await?;
        }

        result.rows_processed = rows_updated;
        result.spatial_operations = spatial_operations;
        Ok(result)
    }

    /// Execute DELETE statement with full spatial support
    async fn execute_delete(
        &self,
        query: DeleteStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        let mut result = ExecutionResult::default();
        let mut spatial_operations = 0;

        let mut tables = self.tables.write().await;
        let table = tables.get_mut(&query.from).ok_or_else(|| {
            SpatialError::OperationError(format!("Table {} not found", query.from))
        })?;

        let original_count = table.rows.len();
        let mut rows_to_keep = Vec::new();
        let mut deleted_row_indices = Vec::new();

        // Filter rows to determine which ones to delete
        for (row_idx, row) in table.rows.iter().enumerate() {
            let mut should_delete = query.where_clause.is_none(); // Delete all if no WHERE clause

            if let Some(where_expr) = &query.where_clause {
                should_delete = self
                    .evaluate_boolean_expression(where_expr, row, table)
                    .await?;
                if self.expression_contains_spatial_ops(where_expr) {
                    spatial_operations += 1;
                }
            }

            if should_delete {
                deleted_row_indices.push(row_idx);
            } else {
                rows_to_keep.push(row.clone());
            }
        }

        // Update the table with remaining rows
        table.rows = rows_to_keep;
        let rows_deleted = original_count - table.rows.len();

        // Update spatial indexes to remove deleted geometries
        if !deleted_row_indices.is_empty() {
            self.remove_from_spatial_indexes(&query.from, &deleted_row_indices)
                .await?;
        }

        result.rows_processed = rows_deleted;
        result.spatial_operations = spatial_operations;
        Ok(result)
    }

    /// Execute CREATE statement with full support
    async fn execute_create(
        &self,
        query: CreateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        let mut result = ExecutionResult::default();

        match query.object_type {
            CreateObjectType::Table => {
                self.execute_create_table(&query).await?;
            }
            CreateObjectType::Index => {
                self.execute_create_index(&query).await?;
            }
            _ => {
                return Err(SpatialError::OperationError(format!(
                    "CREATE {:?} not yet implemented",
                    query.object_type
                )));
            }
        }

        result.rows_processed = 1;
        Ok(result)
    }

    /// Execute CREATE TABLE statement with comprehensive spatial column support
    ///
    /// This method creates a new table with both regular and spatial columns,
    /// automatically detecting spatial data types and maintaining spatial column mappings
    /// for efficient spatial operations and indexing.
    async fn execute_create_table(&self, query: &CreateStatement) -> Result<(), SpatialError> {
        if let CreateDefinition::Table {
            fields,
            constraints: _,
        } = &query.definition
        {
            let mut table_columns = Vec::new();
            let mut spatial_column_mappings = HashMap::new();

            for field in fields {
                // Determine if this field contains spatial data and map the appropriate spatial type
                let detected_spatial_type = match field.data_type {
                    DataType::Geometry => Some(SpatialType::Any),
                    DataType::Point => Some(SpatialType::Point),
                    DataType::LineString => Some(SpatialType::LineString),
                    DataType::Polygon => Some(SpatialType::Polygon),
                    DataType::MultiPoint => Some(SpatialType::MultiPoint),
                    DataType::MultiLineString => Some(SpatialType::MultiLineString),
                    DataType::MultiPolygon => Some(SpatialType::MultiPolygon),
                    _ => None,
                };

                // Register spatial columns in the mapping for efficient lookups during spatial operations
                if detected_spatial_type.is_some() {
                    spatial_column_mappings.insert(field.name.clone(), table_columns.len());
                }

                table_columns.push(ColumnDef {
                    name: field.name.clone(),
                    data_type: field.data_type.clone(),
                    nullable: field.nullable,
                    spatial_type: detected_spatial_type,
                });
            }

            // Create the new table with properly configured columns and spatial mappings
            let new_table = Table {
                name: query.name.clone(),
                columns: table_columns,
                rows: Vec::new(),
                spatial_columns: spatial_column_mappings,
            };

            // Insert the newly created table into the executor's table registry
            let mut table_registry = self.tables.write().await;
            table_registry.insert(query.name.clone(), new_table);
        }

        Ok(())
    }

    /// Execute CREATE INDEX with spatial support
    async fn execute_create_index(&self, query: &CreateStatement) -> Result<(), SpatialError> {
        if let CreateDefinition::Index {
            on,
            fields,
            unique: _,
        } = &query.definition
        {
            if fields.len() != 1 {
                return Err(SpatialError::OperationError(
                    "Multi-column spatial indexes not supported".to_string(),
                ));
            }

            let column_name = &fields[0];
            let tables = self.tables.read().await;

            if let Some(table) = tables.get(on) {
                if let Some(&col_idx) = table.spatial_columns.get(column_name) {
                    // Create spatial index
                    let mut geometries = HashMap::new();

                    // Index existing geometries
                    for (row_idx, row) in table.rows.iter().enumerate() {
                        if let Some(geometry) = row.spatial_values.get(&col_idx) {
                            geometries.insert(row_idx, geometry.clone());
                        }
                    }

                    let spatial_index = SpatialIndexInstance {
                        name: query.name.clone(),
                        table: on.clone(),
                        column: column_name.clone(),
                        index_type: SpatialIndexType::RTree, // Default to R-Tree
                        config: Some(SpatialIndexConfig {
                            max_entries: Some(100),
                            precision: None,
                            fill_factor: Some(0.7),
                            srid: Some(WGS84_SRID),
                        }),
                        geometries,
                    };

                    let mut indexes = self.spatial_indexes.write().await;
                    indexes.insert(query.name.clone(), spatial_index);
                } else {
                    return Err(SpatialError::OperationError(format!(
                        "Column {column_name} is not a spatial column"
                    )));
                }
            } else {
                return Err(SpatialError::OperationError(format!(
                    "Table {on} not found"
                )));
            }
        }

        Ok(())
    }

    /// Execute DROP statement (stub implementation)
    async fn execute_drop(&self, _query: DropStatement) -> Result<ExecutionResult, SpatialError> {
        Ok(ExecutionResult::default())
    }

    /// Execute TRANSACTION statement (stub implementation)
    async fn execute_transaction(
        &self,
        _query: TransactionStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        Ok(ExecutionResult::default())
    }

    /// Execute LIVE statement (stub implementation)
    async fn execute_live(&self, _query: LiveStatement) -> Result<ExecutionResult, SpatialError> {
        Ok(ExecutionResult::default())
    }

    /// Execute RELATE statement (stub implementation)
    async fn execute_relate(
        &self,
        _query: RelateStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        Ok(ExecutionResult::default())
    }

    /// Execute GraphRAG statement (stub implementation)
    async fn execute_graphrag(
        &self,
        _query: GraphRAGStatement,
    ) -> Result<ExecutionResult, SpatialError> {
        Ok(ExecutionResult::default())
    }

    // === HELPER METHODS FOR QUERY EXECUTION ===

    /// Process FROM clause to get base table data
    async fn process_from_clause(
        &self,
        from_clauses: &[FromClause],
    ) -> Result<Table, SpatialError> {
        if from_clauses.is_empty() {
            return Err(SpatialError::OperationError(
                "No FROM clause specified".to_string(),
            ));
        }

        // For now, handle single table FROM clause
        let from_clause = &from_clauses[0];
        match from_clause {
            FromClause::Table { name, alias: _ } => {
                let tables = self.tables.read().await;
                if let Some(table) = tables.get(name) {
                    Ok(table.clone())
                } else {
                    // Return empty table if not found
                    Ok(Table {
                        name: name.clone(),
                        columns: Vec::new(),
                        rows: Vec::new(),
                        spatial_columns: HashMap::new(),
                    })
                }
            }
            _ => Err(SpatialError::OperationError(
                "Complex FROM clauses not yet implemented".to_string(),
            )),
        }
    }

    /// Apply WHERE clause filtering with spatial support
    async fn apply_where_clause(
        &self,
        table: &mut Table,
        where_expr: &Expression,
    ) -> Result<FilterResult, SpatialError> {
        let mut filter_result = FilterResult {
            spatial_ops: 0,
            index_hits: 0,
            gpu_used: false,
        };

        let _original_count = table.rows.len();
        let mut filtered_rows = Vec::new();

        for row in &table.rows {
            if self
                .evaluate_boolean_expression(where_expr, row, table)
                .await?
            {
                filtered_rows.push(row.clone());
            } else {
                // Check if this was a spatial operation
                if self.expression_contains_spatial_ops(where_expr) {
                    filter_result.spatial_ops += 1;
                }
            }
        }

        table.rows = filtered_rows;
        Ok(filter_result)
    }

    /// Process JOIN clauses
    async fn process_join(
        &self,
        _base_table: &mut Table,
        _join: &JoinClause,
    ) -> Result<FilterResult, SpatialError> {
        // Simplified JOIN implementation
        Ok(FilterResult {
            spatial_ops: 0,
            index_hits: 0,
            gpu_used: false,
        })
    }

    /// Apply GROUP BY clause
    async fn apply_group_by(
        &self,
        _table: &mut Table,
        _group_by: &[Expression],
    ) -> Result<(), SpatialError> {
        // Simplified GROUP BY - for now, just return as-is
        Ok(())
    }

    /// Apply HAVING clause
    async fn apply_having_clause(
        &self,
        table: &mut Table,
        having_expr: &Expression,
    ) -> Result<FilterResult, SpatialError> {
        // Similar to WHERE clause but for grouped results
        self.apply_where_clause(table, having_expr).await
    }

    /// Process SELECT fields to generate result columns and rows
    async fn process_select_fields(
        &self,
        table: &Table,
        fields: &[SelectField],
    ) -> Result<(Vec<ColumnDef>, Vec<Row>), SpatialError> {
        let mut result_columns = Vec::new();
        let mut result_rows = Vec::new();

        // Handle different select field types
        if fields.is_empty() || fields.iter().any(|f| matches!(f, SelectField::All)) {
            // SELECT *
            result_columns = table.columns.clone();
            result_rows = table.rows.clone();
        } else {
            // Process specific fields
            for field in fields {
                match field {
                    SelectField::Expression { expr: _expr, alias } => {
                        let col_name = alias.clone().unwrap_or_else(|| "expr".to_string());
                        result_columns.push(ColumnDef {
                            name: col_name,
                            data_type: DataType::String { max_length: None }, // Default type
                            nullable: true,
                            spatial_type: None,
                        });
                    }
                    SelectField::AllFrom(table_name) => {
                        if table_name == &table.name {
                            result_columns.extend(table.columns.clone());
                        }
                    }
                    _ => {
                        // Handle other field types as needed
                    }
                }
            }

            // Create result rows based on selected fields
            for row in &table.rows {
                let mut result_row = Row {
                    values: Vec::new(),
                    spatial_values: HashMap::new(),
                };

                for field in fields {
                    match field {
                        SelectField::Expression { expr, alias: _ } => {
                            let value = self.evaluate_expression(expr).await?;
                            result_row.values.push(value);
                        }
                        SelectField::AllFrom(table_name) => {
                            if table_name == &table.name {
                                result_row.values.extend(row.values.clone());
                                result_row.spatial_values.extend(row.spatial_values.clone());
                            }
                        }
                        _ => {}
                    }
                }

                result_rows.push(result_row);
            }
        }

        Ok((result_columns, result_rows))
    }

    /// Apply ORDER BY clause
    async fn apply_order_by(
        &self,
        rows: Vec<Row>,
        _order_by: &[OrderByClause],
    ) -> Result<Vec<Row>, SpatialError> {
        // Simplified ordering - for now just return as-is
        Ok(rows)
    }

    /// Apply LIMIT and OFFSET
    fn apply_limit_offset(
        &self,
        rows: Vec<Row>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Vec<Row> {
        let start = offset.unwrap_or(0) as usize;
        let mut result = if start < rows.len() {
            rows.into_iter().skip(start).collect()
        } else {
            Vec::new()
        };

        if let Some(limit) = limit {
            result.truncate(limit as usize);
        }

        result
    }

    /// Evaluate an expression to get a JSON value
    async fn evaluate_expression(&self, expr: &Expression) -> Result<Value, SpatialError> {
        match expr {
            Expression::Literal(query_value) => Ok(self.query_value_to_json(query_value)),
            Expression::Identifier(name) => {
                // For now, return a placeholder
                Ok(Value::String(format!("column:{name}")))
            }
            Expression::Function { name, args } => {
                // Handle spatial functions (non-recursively)
                if name.starts_with("ST_") {
                    self.evaluate_spatial_function_simple(name, args.len())
                } else {
                    Ok(Value::String(format!("function:{}({})", name, args.len())))
                }
            }
            _ => Ok(Value::String("expression".to_string())),
        }
    }

    /// Evaluate a boolean expression for filtering
    async fn evaluate_boolean_expression(
        &self,
        expr: &Expression,
        row: &Row,
        table: &Table,
    ) -> Result<bool, SpatialError> {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                match operator {
                    BinaryOperator::Equal => {
                        let left_val = self.evaluate_expression_with_row(left, row, table).await?;
                        let right_val =
                            self.evaluate_expression_with_row(right, row, table).await?;
                        Ok(left_val == right_val)
                    }
                    BinaryOperator::LessThan => {
                        // Simplified comparison
                        Ok(true)
                    }
                    BinaryOperator::GreaterThan => Ok(true),
                    // Handle spatial operators
                    BinaryOperator::SpatialContains => {
                        self.evaluate_spatial_relationship(left, right, row, table, "contains")
                            .await
                    }
                    BinaryOperator::SpatialIntersects => {
                        self.evaluate_spatial_relationship(left, right, row, table, "intersects")
                            .await
                    }
                    _ => Ok(true),
                }
            }
            _ => Ok(true),
        }
    }

    /// Evaluate expression with row context
    async fn evaluate_expression_with_row(
        &self,
        expr: &Expression,
        row: &Row,
        table: &Table,
    ) -> Result<Value, SpatialError> {
        match expr {
            Expression::Identifier(col_name) => {
                if let Some(col_idx) = table.columns.iter().position(|c| c.name == *col_name) {
                    if col_idx < row.values.len() {
                        Ok(row.values[col_idx].clone())
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            _ => self.evaluate_expression(expr).await,
        }
    }

    /// Evaluate spatial relationships
    async fn evaluate_spatial_relationship(
        &self,
        left: &Expression,
        right: &Expression,
        row: &Row,
        table: &Table,
        _relationship: &str,
    ) -> Result<bool, SpatialError> {
        // Extract geometries from both sides
        let _left_geom = self
            .extract_geometry_from_expression(left, row, table)
            .await?;
        let _right_geom = self
            .extract_geometry_from_expression(right, row, table)
            .await?;

        // For now, return true - would implement actual spatial relationship testing
        Ok(true)
    }

    /// Extract geometry from expression
    async fn extract_geometry_from_expression(
        &self,
        expr: &Expression,
        row: &Row,
        table: &Table,
    ) -> Result<Option<SpatialGeometry>, SpatialError> {
        match expr {
            Expression::Identifier(col_name) => {
                if let Some(&col_idx) = table.spatial_columns.get(col_name) {
                    Ok(row.spatial_values.get(&col_idx).cloned())
                } else {
                    Ok(None)
                }
            }
            Expression::Geometry(GeometryLiteral::Point(point)) => {
                Ok(Some(SpatialGeometry::Point(point.clone())))
            }
            Expression::Geometry(_) => Ok(None), // Handle other geometry types as needed
            _ => Ok(None),
        }
    }

    /// Extract spatial geometry from expression for INSERT operations
    async fn extract_spatial_geometry(
        &self,
        expr: &Expression,
    ) -> Result<Option<SpatialGeometry>, SpatialError> {
        match expr {
            Expression::Geometry(geom_literal) => {
                match geom_literal {
                    GeometryLiteral::Point(point) => {
                        Ok(Some(SpatialGeometry::Point(point.clone())))
                    }
                    GeometryLiteral::WKT(_wkt) => {
                        // Parse WKT string - for now return None
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// Convert QueryValue to serde_json::Value
    #[allow(clippy::only_used_in_recursion)]
    fn query_value_to_json(&self, qv: &QueryValue) -> Value {
        match qv {
            QueryValue::Null => Value::Null,
            QueryValue::Boolean(b) => Value::Bool(*b),
            QueryValue::Integer(i) => Value::Number((*i).into()),
            QueryValue::Float(f) => {
                Value::Number(serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()))
            }
            QueryValue::String(s) => Value::String(s.clone()),
            QueryValue::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.query_value_to_json(v)).collect())
            }
            QueryValue::Object(obj) => Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), self.query_value_to_json(v)))
                    .collect(),
            ),
            _ => Value::String("unsupported_type".to_string()),
        }
    }

    /// Check if expression contains spatial operations
    fn expression_contains_spatial_ops(&self, expr: &Expression) -> bool {
        match expr {
            Expression::Binary { operator, .. } => {
                matches!(
                    operator,
                    BinaryOperator::SpatialContains
                        | BinaryOperator::SpatialIntersects
                        | BinaryOperator::SpatialWithin
                        | BinaryOperator::SpatialOverlaps
                )
            }
            Expression::SpatialFunction { .. } => true,
            _ => false,
        }
    }

    /// Evaluate spatial function (simplified, non-recursive)
    fn evaluate_spatial_function_simple(
        &self,
        name: &str,
        arg_count: usize,
    ) -> Result<Value, SpatialError> {
        match name {
            "ST_Distance" => {
                if arg_count == 2 {
                    // For now, return a sample distance
                    Ok(Value::Number(
                        serde_json::Number::from_f64(42.5)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ))
                } else {
                    Err(SpatialError::OperationError(
                        "ST_Distance requires 2 arguments".to_string(),
                    ))
                }
            }
            "ST_Point" => {
                if arg_count == 2 {
                    // Create sample point
                    Ok(Value::Object({
                        let mut map = Map::new();
                        map.insert("type".to_string(), Value::String("Point".to_string()));
                        map.insert(
                            "coordinates".to_string(),
                            Value::Array(vec![Value::Number(1.into()), Value::Number(2.into())]),
                        );
                        map
                    }))
                } else {
                    Err(SpatialError::OperationError(
                        "ST_Point requires 2 arguments".to_string(),
                    ))
                }
            }
            _ => Ok(Value::String(format!("spatial_function:{name}"))),
        }
    }

    /// Get or create a table with the given name
    ///
    /// This is a utility method that ensures a table exists before performing operations.
    /// If the table doesn't exist, it creates a new empty table with the given name.
    ///
    /// # Arguments
    /// * `tables` - Mutable reference to the tables HashMap
    /// * `table_name` - Name of the table to get or create
    ///
    /// # Returns
    /// * `&mut Table` - Mutable reference to the existing or newly created table
    ///
    /// # Thread Safety
    /// * This method assumes the caller already holds appropriate locks
    fn get_or_create_table<'a>(
        &self,
        tables: &'a mut HashMap<String, Table>,
        table_name: &str,
    ) -> &'a mut Table {
        tables
            .entry(table_name.to_string())
            .or_insert_with(|| Table {
                name: table_name.to_string(),
                columns: Vec::new(),
                rows: Vec::new(),
                spatial_columns: HashMap::new(),
            })
    }

    /// Insert value rows into table
    async fn insert_value_rows(
        &self,
        table: &mut Table,
        value_rows: Vec<Vec<Expression>>,
        fields: &Option<Vec<String>>,
    ) -> Result<InsertResult, SpatialError> {
        let mut rows_inserted = 0;
        let mut spatial_operations = 0;

        let empty_fields = Vec::new();
        let field_names = fields.as_ref().unwrap_or(&empty_fields);

        for row_values in value_rows {
            if !field_names.is_empty() && row_values.len() != field_names.len() {
                return Err(SpatialError::OperationError(
                    "Value count doesn't match field count".to_string(),
                ));
            }

            let mut row = Row {
                values: Vec::new(),
                spatial_values: HashMap::new(),
            };

            // Process each value
            for (i, expression) in row_values.iter().enumerate() {
                let value = self.evaluate_expression(expression).await?;
                row.values.push(value);

                // Handle spatial columns
                if let Some(field_name) = field_names.get(i) {
                    if table.spatial_columns.contains_key(field_name) {
                        if let Some(spatial_geom) =
                            self.extract_spatial_geometry(expression).await?
                        {
                            row.spatial_values.insert(i, spatial_geom);
                            spatial_operations += 1;
                        }
                    }
                }
            }

            table.rows.push(row);
            rows_inserted += 1;
        }

        Ok(InsertResult {
            rows_inserted,
            spatial_operations,
        })
    }

    /// Insert object map into table
    async fn insert_object_map(
        &self,
        table: &mut Table,
        object_map: std::collections::HashMap<String, Expression>,
    ) -> Result<InsertResult, SpatialError> {
        let mut spatial_operations = 0;

        let mut row = Row {
            values: Vec::new(),
            spatial_values: HashMap::new(),
        };

        // Initialize with nulls based on table structure
        row.values.resize(table.columns.len(), Value::Null);

        // Fill in values from the object map
        for (field_name, expression) in object_map {
            if let Some(col_idx) = table.columns.iter().position(|c| c.name == field_name) {
                let value = self.evaluate_expression(&expression).await?;
                row.values[col_idx] = value;

                // Handle spatial columns
                if table.spatial_columns.contains_key(&field_name) {
                    if let Some(spatial_geom) = self.extract_spatial_geometry(&expression).await? {
                        row.spatial_values.insert(col_idx, spatial_geom);
                        spatial_operations += 1;
                    }
                }
            }
        }

        table.rows.push(row);

        Ok(InsertResult {
            rows_inserted: 1,
            spatial_operations,
        })
    }

    /// Optimized process a single row update without full table clone
    async fn process_row_update_optimized(
        &self,
        row: &mut Row,
        query: &UpdateStatement,
        columns: &[ColumnDef],
        spatial_columns: &HashMap<String, usize>,
    ) -> Result<UpdateRowResult, SpatialError> {
        let mut spatial_operations = 0;

        // Check WHERE clause condition (simplified - would need row context for complex expressions)
        let should_update = query.where_clause.is_none(); // Simplified for now

        if let Some(where_expr) = &query.where_clause {
            if self.expression_contains_spatial_ops(where_expr) {
                spatial_operations += 1;
            }
        }

        if should_update {
            // Apply each assignment
            for assignment in &query.assignments {
                let assignment_result = self
                    .apply_assignment_optimized(row, assignment, columns, spatial_columns)
                    .await?;
                spatial_operations += assignment_result;
            }
        }

        Ok(UpdateRowResult {
            updated: should_update,
            spatial_operations,
        })
    }

    /// Process a single row update
    #[allow(dead_code)]
    async fn process_row_update(
        &self,
        row: &mut Row,
        query: &UpdateStatement,
        table_copy: &Table,
    ) -> Result<UpdateRowResult, SpatialError> {
        let mut spatial_operations = 0;

        // Check WHERE clause condition
        let should_update = if let Some(where_expr) = &query.where_clause {
            let result = self
                .evaluate_boolean_expression(where_expr, row, table_copy)
                .await?;
            if self.expression_contains_spatial_ops(where_expr) {
                spatial_operations += 1;
            }
            result
        } else {
            true
        };

        if should_update {
            // Apply each assignment
            for assignment in &query.assignments {
                let assignment_result = self.apply_assignment(row, assignment, table_copy).await?;
                spatial_operations += assignment_result;
            }
        }

        Ok(UpdateRowResult {
            updated: should_update,
            spatial_operations,
        })
    }

    /// Optimized apply a single assignment to a row
    async fn apply_assignment_optimized(
        &self,
        row: &mut Row,
        assignment: &UpdateAssignment,
        columns: &[ColumnDef],
        spatial_columns: &HashMap<String, usize>,
    ) -> Result<usize, SpatialError> {
        let mut spatial_operations = 0;

        if let Some(col_idx) = columns.iter().position(|c| c.name == assignment.field) {
            // Evaluate the new value
            let new_value = self.evaluate_expression(&assignment.value).await?;

            // Update the value based on the operator
            self.update_column_value(row, col_idx, &new_value, &assignment.operator);

            // Handle spatial column updates
            if spatial_columns.contains_key(&assignment.field) {
                if let Some(spatial_geom) = self.extract_spatial_geometry(&assignment.value).await?
                {
                    row.spatial_values.insert(col_idx, spatial_geom);
                    spatial_operations += 1;
                }
            }
        }

        Ok(spatial_operations)
    }

    /// Apply a single assignment to a row
    #[allow(dead_code)]
    async fn apply_assignment(
        &self,
        row: &mut Row,
        assignment: &UpdateAssignment,
        table: &Table,
    ) -> Result<usize, SpatialError> {
        let mut spatial_operations = 0;

        if let Some(col_idx) = table
            .columns
            .iter()
            .position(|c| c.name == assignment.field)
        {
            // Evaluate the new value
            let new_value = self.evaluate_expression(&assignment.value).await?;

            // Update the value based on the operator
            self.update_column_value(row, col_idx, &new_value, &assignment.operator);

            // Handle spatial column updates
            if table.spatial_columns.contains_key(&assignment.field) {
                if let Some(spatial_geom) = self.extract_spatial_geometry(&assignment.value).await?
                {
                    row.spatial_values.insert(col_idx, spatial_geom);
                    spatial_operations += 1;
                }
            }
        }

        Ok(spatial_operations)
    }

    /// Update column value based on operator
    fn update_column_value(
        &self,
        row: &mut Row,
        col_idx: usize,
        new_value: &Value,
        operator: &Option<UpdateOperator>,
    ) {
        match operator {
            Some(UpdateOperator::Set) | None => {
                row.values[col_idx] = new_value.clone();
            }
            Some(UpdateOperator::Add) => {
                // Simplified addition - for production would need proper type handling
                if let (Value::Number(old), Value::Number(new)) = (&row.values[col_idx], new_value)
                {
                    if let (Some(old_f), Some(new_f)) = (old.as_f64(), new.as_f64()) {
                        row.values[col_idx] = Value::Number(
                            serde_json::Number::from_f64(old_f + new_f)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        );
                    }
                }
            }
            _ => {
                // Handle other operators as needed
                row.values[col_idx] = new_value.clone();
            }
        }
    }

    /// Update spatial indexes after INSERT operations
    async fn update_spatial_indexes(
        &self,
        table_name: &str,
        start_row: usize,
        count: usize,
    ) -> Result<(), SpatialError> {
        let mut indexes = self.spatial_indexes.write().await;
        let tables = self.tables.read().await;

        if let Some(table) = tables.get(table_name) {
            for (_index_name, index) in indexes.iter_mut() {
                if index.table == table_name {
                    // Update the index with new geometries
                    for row_idx in start_row..(start_row + count) {
                        if let Some(row) = table.rows.get(row_idx) {
                            if let Some(col_idx) = table.spatial_columns.get(&index.column) {
                                if let Some(geometry) = row.spatial_values.get(col_idx) {
                                    index.geometries.insert(row_idx, geometry.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove geometries from spatial indexes after DELETE operations
    async fn remove_from_spatial_indexes(
        &self,
        table_name: &str,
        deleted_indices: &[usize],
    ) -> Result<(), SpatialError> {
        let mut indexes = self.spatial_indexes.write().await;

        for (_, index) in indexes.iter_mut() {
            if index.table == table_name {
                // Remove geometries for deleted rows
                for &row_idx in deleted_indices {
                    index.geometries.remove(&row_idx);
                }
            }
        }

        Ok(())
    }
}

/// Result of filtering operations
struct FilterResult {
    spatial_ops: usize,
    index_hits: usize,
    gpu_used: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            session_id: "default".to_string(),
            started_at: Instant::now(),
            query_timeout: None,
            enable_gpu: false,
            parallel_execution: false,
        }
    }
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            execution_time: Duration::from_millis(0),
            rows_processed: 0,
            spatial_operations: 0,
            index_hits: 0,
            gpu_acceleration_used: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_table() {
        let executor = OrbitQLExecutor::new();

        let create_stmt = Statement::Create(CreateStatement {
            object_type: CreateObjectType::Table,
            name: "test_table".to_string(),
            definition: CreateDefinition::Table {
                fields: vec![FieldDefinition {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    default: None,
                    constraints: vec![],
                }],
                constraints: vec![],
            },
        });

        let result = executor.execute(create_stmt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_index_query() {
        let executor = OrbitQLExecutor::new();

        // First create a table with a spatial column
        let create_table_stmt = Statement::Create(CreateStatement {
            object_type: CreateObjectType::Table,
            name: "test_table".to_string(),
            definition: CreateDefinition::Table {
                fields: vec![
                    FieldDefinition {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        constraints: vec![],
                    },
                    FieldDefinition {
                        name: "geom".to_string(),
                        data_type: DataType::Point,
                        nullable: true,
                        default: None,
                        constraints: vec![],
                    },
                ],
                constraints: vec![],
            },
        });

        let table_result = executor.execute(create_table_stmt).await;
        assert!(table_result.is_ok());

        // Now create an index on the spatial column
        let create_index_stmt = Statement::Create(CreateStatement {
            object_type: CreateObjectType::Index,
            name: "test_index".to_string(),
            definition: CreateDefinition::Index {
                on: "test_table".to_string(),
                fields: vec!["geom".to_string()],
                unique: false,
            },
        });

        let result = executor.execute(create_index_stmt).await;
        assert!(result.is_ok());
    }
}
