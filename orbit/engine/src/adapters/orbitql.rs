//! OrbitQL Protocol Adapter
//!
//! This adapter bridges OrbitQL queries to the unified orbit-engine storage layer,
//! enabling multi-model queries across all data stored in the engine.
//!
//! OrbitQL is a unified query language that combines document, graph, time-series,
//! and key-value operations in a single query.

use super::{AdapterContext, CommandResult, ProtocolAdapter, TransactionAdapter};
use crate::error::{EngineError, EngineResult};
use crate::storage::{AccessPattern, DataType, FilterPredicate, Row, SqlValue, TableSchema};
use async_trait::async_trait;
use orbit_shared::orbitql::{
    ast::{
        BinaryOperator, CreateDefinition, CreateObjectType, CreateStatement, DeleteStatement,
        Expression, FromClause, InsertStatement, InsertValues, SelectStatement,
        Statement, UnaryOperator, UpdateStatement,
    },
    Lexer, Parser, QueryValue,
};
use std::collections::HashMap;
use std::sync::Arc;

/// OrbitQL adapter for orbit-engine
///
/// Provides a unified query interface for multi-model data access
pub struct OrbitQLAdapter {
    context: AdapterContext,
    transaction_adapter: TransactionAdapter,
}

impl OrbitQLAdapter {
    /// Create a new OrbitQL adapter
    pub fn new(context: AdapterContext) -> Self {
        Self {
            transaction_adapter: TransactionAdapter::new(context.clone()),
            context,
        }
    }

    /// Execute an OrbitQL query string
    ///
    /// # Example
    /// ```ignore
    /// let result = adapter.execute_query(
    ///     "SELECT * FROM users WHERE age > 18"
    /// ).await?;
    /// ```
    pub async fn execute_query(&self, query: &str) -> EngineResult<CommandResult> {
        // Parse the OrbitQL query
        let statement = self.parse_query(query)?;

        // Execute the parsed statement
        self.execute_statement(statement).await
    }

    /// Parse OrbitQL query string into AST
    fn parse_query(&self, query: &str) -> EngineResult<Statement> {
        let mut lexer = Lexer::new(query);
        let tokens = lexer
            .tokenize()
            .map_err(|e| EngineError::query(format!("Lexer error: {}", e)))?;

        let mut parser = Parser::new(tokens);
        parser
            .parse()
            .map_err(|e| EngineError::query(format!("Parse error: {}", e)))
    }

    /// Execute a parsed statement
    async fn execute_statement(&self, statement: Statement) -> EngineResult<CommandResult> {
        match statement {
            Statement::Select(select) => self.execute_select(select).await,
            Statement::Insert(insert) => self.execute_insert(insert).await,
            Statement::Update(update) => self.execute_update(update).await,
            Statement::Delete(delete) => self.execute_delete(delete).await,
            Statement::Create(create) => self.execute_create(create).await,
            _ => Err(EngineError::not_supported(format!(
                "Statement type not yet supported: {:?}",
                statement
            ))),
        }
    }

    /// Execute SELECT statement
    async fn execute_select(&self, select: SelectStatement) -> EngineResult<CommandResult> {
        // Extract table name from FROM clause
        let table_name = if select.from.is_empty() {
            return Err(EngineError::query("SELECT requires FROM clause"));
        } else {
            match &select.from[0] {
                FromClause::Table { name, .. } => name.clone(),
                _ => {
                    return Err(EngineError::not_supported(
                        "Only simple table FROM clauses supported currently",
                    ))
                }
            }
        };

        // Convert WHERE clause to FilterPredicate
        let filter = select
            .where_clause
            .map(|expr| self.expression_to_filter(&expr))
            .transpose()?;

        // Create access pattern
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter,
        };

        // Execute query
        let result = self.context.storage.query(&table_name, pattern).await?;

        // Extract rows from result
        let mut rows = match result {
            crate::storage::QueryResult::Rows(rows) => rows,
            crate::storage::QueryResult::ColumnBatch(_) => {
                return Err(EngineError::query(
                    "OrbitQL does not support column batch results yet",
                ))
            }
            crate::storage::QueryResult::Aggregate(val) => {
                // Convert aggregate to single row
                let mut row = HashMap::new();
                row.insert("result".to_string(), val);
                vec![row]
            }
            crate::storage::QueryResult::Empty => vec![],
        };

        // Apply LIMIT and OFFSET
        if let Some(offset) = select.offset {
            if offset as usize >= rows.len() {
                rows.clear();
            } else {
                rows = rows.split_off(offset as usize);
            }
        }
        if let Some(limit) = select.limit {
            rows.truncate(limit as usize);
        }

        Ok(CommandResult::Rows(rows))
    }

    /// Execute INSERT statement
    async fn execute_insert(&self, insert: InsertStatement) -> EngineResult<CommandResult> {
        let table_name = &insert.into;

        // Convert InsertValues to rows
        let rows: Vec<Row> = match insert.values {
            InsertValues::Object(obj) => {
                // Single row from object
                let row = obj
                    .into_iter()
                    .map(|(k, v)| Ok((k, self.expression_to_sql_value(&v)?)))
                    .collect::<EngineResult<HashMap<String, SqlValue>>>()?;
                vec![row]
            }
            InsertValues::Values(values) => {
                // Multiple rows from values
                let field_names = insert.fields.ok_or_else(|| {
                    EngineError::query("INSERT VALUES requires field names")
                })?;

                values
                    .into_iter()
                    .map(|value_exprs| {
                        let mut row = HashMap::new();
                        for (i, expr) in value_exprs.into_iter().enumerate() {
                            if i < field_names.len() {
                                row.insert(
                                    field_names[i].clone(),
                                    self.expression_to_sql_value(&expr)?,
                                );
                            }
                        }
                        Ok(row)
                    })
                    .collect::<EngineResult<Vec<_>>>()?
            }
            InsertValues::Select(_) => {
                return Err(EngineError::not_supported(
                    "INSERT ... SELECT not yet supported",
                ))
            }
        };

        // Insert rows
        let mut count = 0;
        for row in rows {
            self.context.storage.insert_row(table_name, row).await?;
            count += 1;
        }

        Ok(CommandResult::RowsAffected(count))
    }

    /// Execute UPDATE statement
    async fn execute_update(&self, update: UpdateStatement) -> EngineResult<CommandResult> {
        let table_name = &update.table;

        // Convert WHERE clause
        let filter = update
            .where_clause
            .map(|expr| self.expression_to_filter(&expr))
            .transpose()?
            .unwrap_or_else(|| FilterPredicate::And(vec![])); // Match all if no WHERE

        // Convert assignments to updates
        let updates: HashMap<String, SqlValue> = update
            .assignments
            .into_iter()
            .map(|assignment| {
                Ok((
                    assignment.field,
                    self.expression_to_sql_value(&assignment.value)?,
                ))
            })
            .collect::<EngineResult<_>>()?;

        // Execute update
        let affected = self
            .context
            .storage
            .update(table_name, filter, updates)
            .await?;

        Ok(CommandResult::RowsAffected(affected))
    }

    /// Execute DELETE statement
    async fn execute_delete(&self, delete: DeleteStatement) -> EngineResult<CommandResult> {
        let table_name = &delete.from;

        // Convert WHERE clause
        let filter = delete
            .where_clause
            .map(|expr| self.expression_to_filter(&expr))
            .transpose()?
            .unwrap_or_else(|| FilterPredicate::And(vec![])); // Match all if no WHERE

        // Execute delete
        let affected = self.context.storage.delete(table_name, filter).await?;

        Ok(CommandResult::RowsAffected(affected))
    }

    /// Execute CREATE statement
    async fn execute_create(&self, create: CreateStatement) -> EngineResult<CommandResult> {
        match create.object_type {
            CreateObjectType::Table => {
                let table_name = &create.name;

                // Extract table definition
                let (fields, _constraints) = match create.definition {
                    CreateDefinition::Table {
                        fields,
                        constraints,
                    } => (fields, constraints),
                    _ => {
                        return Err(EngineError::query(
                            "Expected table definition for CREATE TABLE",
                        ))
                    }
                };

                // Convert field definitions to column definitions
                let columns = fields
                    .into_iter()
                    .map(|field| crate::storage::ColumnDef {
                        name: field.name,
                        data_type: self.orbitql_datatype_to_engine_datatype(&field.data_type),
                        nullable: field.nullable,
                    })
                    .collect();

                // Create schema
                let schema = TableSchema {
                    name: table_name.clone(),
                    columns,
                    primary_key: vec![], // TODO: Extract from constraints
                };

                // Create table
                self.context.storage.create_table(schema).await?;

                Ok(CommandResult::Ok)
            }
            _ => Err(EngineError::not_supported(format!(
                "CREATE {:?} not yet supported",
                create.object_type
            ))),
        }
    }

    /// Convert OrbitQL expression to FilterPredicate
    fn expression_to_filter(&self, expr: &Expression) -> EngineResult<FilterPredicate> {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                match operator {
                    BinaryOperator::Equal => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Eq(field, value))
                    }
                    BinaryOperator::NotEqual => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Ne(field, value))
                    }
                    BinaryOperator::LessThan => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Lt(field, value))
                    }
                    BinaryOperator::LessThanOrEqual => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Le(field, value))
                    }
                    BinaryOperator::GreaterThan => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Gt(field, value))
                    }
                    BinaryOperator::GreaterThanOrEqual => {
                        let field = self.extract_field_name(left)?;
                        let value = self.expression_to_sql_value(right)?;
                        Ok(FilterPredicate::Ge(field, value))
                    }
                    BinaryOperator::And => {
                        let left_filter = self.expression_to_filter(left)?;
                        let right_filter = self.expression_to_filter(right)?;
                        Ok(FilterPredicate::And(vec![left_filter, right_filter]))
                    }
                    BinaryOperator::Or => {
                        let left_filter = self.expression_to_filter(left)?;
                        let right_filter = self.expression_to_filter(right)?;
                        Ok(FilterPredicate::Or(vec![left_filter, right_filter]))
                    }
                    _ => Err(EngineError::not_supported(format!(
                        "Operator {:?} not supported in WHERE clause",
                        operator
                    ))),
                }
            }
            Expression::Unary { operator, operand } => match operator {
                UnaryOperator::Not => {
                    let inner = self.expression_to_filter(operand)?;
                    Ok(FilterPredicate::Not(Box::new(inner)))
                }
                _ => Err(EngineError::not_supported(format!(
                    "Unary operator {:?} not supported",
                    operator
                ))),
            },
            _ => Err(EngineError::query(format!(
                "Cannot convert expression to filter: {:?}",
                expr
            ))),
        }
    }

    /// Extract field name from expression
    fn extract_field_name(&self, expr: &Expression) -> EngineResult<String> {
        match expr {
            Expression::Identifier(name) => Ok(name.clone()),
            Expression::FieldAccess { object, field } => {
                // For nested field access like user.name
                Ok(format!("{}.{}", self.extract_field_name(object)?, field))
            }
            _ => Err(EngineError::query(format!(
                "Expected identifier, got: {:?}",
                expr
            ))),
        }
    }

    /// Convert OrbitQL expression to SqlValue
    fn expression_to_sql_value(&self, expr: &Expression) -> EngineResult<SqlValue> {
        match expr {
            Expression::Literal(lit) => self.query_value_to_sql_value(lit),
            Expression::Identifier(s) => Ok(SqlValue::String(s.clone())),
            _ => Err(EngineError::query(format!(
                "Cannot convert expression to value: {:?}",
                expr
            ))),
        }
    }

    /// Convert QueryValue to SqlValue
    fn query_value_to_sql_value(&self, value: &QueryValue) -> EngineResult<SqlValue> {
        match value {
            QueryValue::Null => Ok(SqlValue::Null),
            QueryValue::Boolean(b) => Ok(SqlValue::Boolean(*b)),
            QueryValue::Integer(i) => Ok(SqlValue::Int64(*i)),
            QueryValue::Float(f) => Ok(SqlValue::Float64(*f)),
            QueryValue::String(s) => Ok(SqlValue::String(s.clone())),
            QueryValue::Array(arr) => {
                // Serialize array as JSON
                let json = serde_json::to_string(arr)
                    .map_err(|e| EngineError::serialization(e.to_string()))?;
                Ok(SqlValue::String(json))
            }
            QueryValue::Object(obj) => {
                // Serialize object as JSON
                let json = serde_json::to_string(obj)
                    .map_err(|e| EngineError::serialization(e.to_string()))?;
                Ok(SqlValue::String(json))
            }
            QueryValue::Timestamp(ts) => Ok(SqlValue::Timestamp(
                std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(ts.timestamp() as u64),
            )),
            _ => Err(EngineError::not_supported(format!(
                "QueryValue type not supported: {:?}",
                value
            ))),
        }
    }

    /// Convert OrbitQL DataType to engine DataType
    fn orbitql_datatype_to_engine_datatype(
        &self,
        orbitql_type: &orbit_shared::orbitql::ast::DataType,
    ) -> DataType {
        use orbit_shared::orbitql::ast::DataType as OQL;

        match orbitql_type {
            OQL::String => DataType::String,
            OQL::Integer => DataType::Int64,
            OQL::Float => DataType::Float64,
            OQL::Boolean => DataType::Boolean,
            OQL::Timestamp => DataType::Timestamp,
            OQL::Binary => DataType::Binary,
            _ => DataType::String, // Default to string for complex types
        }
    }
}

#[async_trait]
impl ProtocolAdapter for OrbitQLAdapter {
    fn protocol_name(&self) -> &'static str {
        "OrbitQL"
    }

    async fn initialize(&mut self) -> EngineResult<()> {
        // No initialization needed
        Ok(())
    }

    async fn shutdown(&mut self) -> EngineResult<()> {
        // No cleanup needed
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HybridStorageManager;

    #[tokio::test]
    async fn test_orbitql_adapter_creation() {
        let storage = Arc::new(HybridStorageManager::new_in_memory());
        let context = AdapterContext::new(storage as Arc<dyn crate::storage::TableStorage>);

        let adapter = OrbitQLAdapter::new(context);
        assert_eq!(adapter.protocol_name(), "OrbitQL");
    }
}
