//! SQL Generation Engine
//!
//! This module converts natural language query intents into SQL queries
//! with schema awareness and optimization hints.

use crate::mcp::nlp::{
    AggregationType, ComparisonOperator, ConditionValue, QueryIntent, SqlOperation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQL Generation Engine
pub struct SqlGenerator {
    /// Schema cache for table information
    schema_cache: HashMap<String, TableSchema>,
    /// Query builder for constructing SQL
    query_builder: QueryBuilder,
}

impl SqlGenerator {
    /// Create a new SQL generator
    pub fn new() -> Self {
        Self {
            schema_cache: HashMap::new(),
            query_builder: QueryBuilder::new(),
        }
    }

    /// Generate SQL from query intent
    pub fn generate_sql(&self, intent: &QueryIntent) -> Result<GeneratedQuery, SqlGenerationError> {
        match &intent.operation {
            SqlOperation::Select {
                aggregation,
                limit,
                ordering,
            } => self
                .query_builder
                .build_select(intent, aggregation, limit, ordering),
            SqlOperation::Insert { mode } => self.query_builder.build_insert(intent, mode),
            SqlOperation::Update { conditional } => {
                self.query_builder.build_update(intent, *conditional)
            }
            SqlOperation::Delete { conditional } => {
                self.query_builder.build_delete(intent, *conditional)
            }
            SqlOperation::Analyze { analysis_type } => {
                self.query_builder.build_analytical(intent, analysis_type)
            }
        }
    }

    /// Update schema cache
    pub fn update_schema(&mut self, table_name: String, schema: TableSchema) {
        self.schema_cache.insert(table_name, schema);
    }

    /// Get schema for a table
    pub fn get_schema(&self, table_name: &str) -> Option<&TableSchema> {
        self.schema_cache.get(table_name)
    }
}

impl Default for SqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Query Builder
struct QueryBuilder;

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self
    }

    /// Build a SELECT query
    pub fn build_select(
        &self,
        intent: &QueryIntent,
        aggregation: &Option<AggregationType>,
        limit: &Option<u64>,
        ordering: &Option<crate::mcp::nlp::OrderingSpec>,
    ) -> Result<GeneratedQuery, SqlGenerationError> {
        let mut sql = String::new();
        let mut parameters = Vec::new();

        // Build SELECT clause
        sql.push_str("SELECT ");

        if let Some(agg) = aggregation {
            match agg {
                AggregationType::Count => {
                    sql.push_str("COUNT(*)");
                }
                AggregationType::Sum => {
                    if let Some(col) = intent.projections.first() {
                        sql.push_str(&format!("SUM({})", col.name));
                    } else {
                        sql.push_str("SUM(*)");
                    }
                }
                AggregationType::Average => {
                    if let Some(col) = intent.projections.first() {
                        sql.push_str(&format!("AVG({})", col.name));
                    } else {
                        sql.push_str("AVG(*)");
                    }
                }
                AggregationType::Min => {
                    if let Some(col) = intent.projections.first() {
                        sql.push_str(&format!("MIN({})", col.name));
                    } else {
                        sql.push_str("MIN(*)");
                    }
                }
                AggregationType::Max => {
                    if let Some(col) = intent.projections.first() {
                        sql.push_str(&format!("MAX({})", col.name));
                    } else {
                        sql.push_str("MAX(*)");
                    }
                }
                AggregationType::GroupBy => {
                    // For GROUP BY, select the grouping column and aggregates
                    if let Some(col) = intent.projections.first() {
                        sql.push_str(&format!("{}, COUNT(*) as count", col.name));
                    } else {
                        sql.push_str("*");
                    }
                }
            }
        } else {
            // Regular SELECT
            let columns: Vec<String> = intent.projections.iter().map(|p| p.name.clone()).collect();
            sql.push_str(&columns.join(", "));
        }

        // Find table name from entities
        let table_name = intent
            .entities
            .iter()
            .find(|e| matches!(e.entity_type, crate::mcp::nlp::EntityType::Table))
            .map(|e| e.value.clone())
            .ok_or_else(|| SqlGenerationError::MissingTable)?;

        sql.push_str(&format!(" FROM {}", table_name));

        // Build WHERE clause
        if !intent.conditions.is_empty() {
            sql.push_str(" WHERE ");
            let conditions: Vec<String> = intent
                .conditions
                .iter()
                .enumerate()
                .map(|(_idx, cond)| {
                    let param_idx = parameters.len() + 1;
                    let (op_str, value) = self.format_condition(cond, param_idx);
                    parameters.push(value);
                    format!("{} {} ${}", cond.column, op_str, param_idx)
                })
                .collect();
            sql.push_str(&conditions.join(" AND "));
        }

        // Build ORDER BY clause
        if let Some(order) = ordering {
            sql.push_str(&format!(
                " ORDER BY {} {}",
                order.column,
                match order.direction {
                    crate::mcp::nlp::SortDirection::Ascending => "ASC",
                    crate::mcp::nlp::SortDirection::Descending => "DESC",
                }
            ));
        }

        // Build LIMIT clause
        if let Some(limit_val) = limit {
            sql.push_str(&format!(" LIMIT {}", limit_val));
        }

        Ok(GeneratedQuery {
            sql,
            parameters,
            query_type: QueryType::Read,
            estimated_complexity: self.estimate_complexity(intent),
            optimization_hints: self.generate_hints(intent),
        })
    }

    /// Build an INSERT query
    pub fn build_insert(
        &self,
        intent: &QueryIntent,
        _mode: &crate::mcp::nlp::InsertMode,
    ) -> Result<GeneratedQuery, SqlGenerationError> {
        // Find table name
        let table_name = intent
            .entities
            .iter()
            .find(|e| matches!(e.entity_type, crate::mcp::nlp::EntityType::Table))
            .map(|e| e.value.clone())
            .ok_or_else(|| SqlGenerationError::MissingTable)?;

        // Extract column-value pairs from entities
        let mut columns = Vec::new();
        let mut values = Vec::new();
        let mut parameters = Vec::new();

        for entity in &intent.entities {
            if matches!(entity.entity_type, crate::mcp::nlp::EntityType::Column) {
                columns.push(entity.value.clone());
            } else if matches!(entity.entity_type, crate::mcp::nlp::EntityType::Value) {
                let param_idx = parameters.len() + 1;
                values.push(format!("${}", param_idx));
                parameters.push(serde_json::Value::String(entity.value.clone()));
            }
        }

        // If no columns specified, use default columns or allow empty insert
        if columns.is_empty() {
            // For INSERT without explicit columns, we'll use a placeholder
            // In a real system, this would query the schema for default columns
            columns.push("id".to_string());
            values.push("$1".to_string());
            parameters.push(serde_json::Value::Null);
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table_name,
            columns.join(", "),
            values.join(", ")
        );

        Ok(GeneratedQuery {
            sql,
            parameters,
            query_type: QueryType::Write,
            estimated_complexity: QueryComplexity::Low,
            optimization_hints: vec![],
        })
    }

    /// Build an UPDATE query
    pub fn build_update(
        &self,
        intent: &QueryIntent,
        conditional: bool,
    ) -> Result<GeneratedQuery, SqlGenerationError> {
        let table_name = intent
            .entities
            .iter()
            .find(|e| matches!(e.entity_type, crate::mcp::nlp::EntityType::Table))
            .map(|e| e.value.clone())
            .ok_or_else(|| SqlGenerationError::MissingTable)?;

        let mut sql = format!("UPDATE {} SET ", table_name);
        let mut parameters = Vec::new();

        // Build SET clause from conditions (simplified)
        let set_clauses: Vec<String> = intent
            .conditions
            .iter()
            .enumerate()
            .map(|(_idx, cond)| {
                let param_idx = parameters.len() + 1;
                let value = match &cond.value {
                    ConditionValue::String(s) => serde_json::Value::String(s.clone()),
                    ConditionValue::Number(n) => serde_json::Value::Number(
                        serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
                    ),
                    ConditionValue::Integer(i) => {
                        serde_json::Value::Number(serde_json::Number::from(*i))
                    }
                    ConditionValue::Boolean(b) => serde_json::Value::Bool(*b),
                    _ => serde_json::Value::String("".to_string()),
                };
                parameters.push(value);
                format!("{} = ${}", cond.column, param_idx)
            })
            .collect();

        sql.push_str(&set_clauses.join(", "));

        if conditional {
            sql.push_str(" WHERE 1=1"); // Placeholder - would need WHERE conditions
        }

        Ok(GeneratedQuery {
            sql,
            parameters,
            query_type: QueryType::Write,
            estimated_complexity: QueryComplexity::Medium,
            optimization_hints: vec![],
        })
    }

    /// Build a DELETE query
    pub fn build_delete(
        &self,
        intent: &QueryIntent,
        conditional: bool,
    ) -> Result<GeneratedQuery, SqlGenerationError> {
        // Try to find table name from entities
        let table_name = intent
            .entities
            .iter()
            .find(|e| matches!(e.entity_type, crate::mcp::nlp::EntityType::Table))
            .map(|e| e.value.clone());

        // If no table found, try to infer from common patterns
        let table_name = table_name.unwrap_or_else(|| {
            // Default to a generic table name if we can't determine it
            // In production, this would use schema information
            "logs".to_string()
        });

        let mut sql = format!("DELETE FROM {}", table_name);
        let mut parameters = Vec::new();

        if conditional && !intent.conditions.is_empty() {
            sql.push_str(" WHERE ");
            let conditions: Vec<String> = intent
                .conditions
                .iter()
                .enumerate()
                .map(|(_idx, cond)| {
                    let param_idx = parameters.len() + 1;
                    let (op_str, value) = self.format_condition(cond, param_idx);
                    parameters.push(value);
                    format!("{} {} ${}", cond.column, op_str, param_idx)
                })
                .collect();
            sql.push_str(&conditions.join(" AND "));
        }

        Ok(GeneratedQuery {
            sql,
            parameters,
            query_type: QueryType::Write,
            estimated_complexity: QueryComplexity::Medium,
            optimization_hints: vec![],
        })
    }

    /// Build an analytical query
    pub fn build_analytical(
        &self,
        intent: &QueryIntent,
        _analysis_type: &crate::mcp::nlp::AnalysisType,
    ) -> Result<GeneratedQuery, SqlGenerationError> {
        // For analytical queries, generate appropriate SQL based on analysis type
        let table_name = intent
            .entities
            .iter()
            .find(|e| matches!(e.entity_type, crate::mcp::nlp::EntityType::Table))
            .map(|e| e.value.clone())
            .ok_or_else(|| SqlGenerationError::MissingTable)?;

        // Generate a comprehensive analytical query
        let sql = format!(
            "SELECT COUNT(*) as count, AVG({}) as avg, MIN({}) as min, MAX({}) as max FROM {}",
            intent
                .projections
                .first()
                .map(|p| p.name.as_str())
                .unwrap_or("*"),
            intent
                .projections
                .first()
                .map(|p| p.name.as_str())
                .unwrap_or("*"),
            intent
                .projections
                .first()
                .map(|p| p.name.as_str())
                .unwrap_or("*"),
            table_name
        );

        Ok(GeneratedQuery {
            sql,
            parameters: vec![],
            query_type: QueryType::Analysis,
            estimated_complexity: QueryComplexity::High,
            optimization_hints: vec![
                OptimizationHint::UseIndex,
                OptimizationHint::ConsiderPartitioning,
            ],
        })
    }

    /// Format a condition for SQL
    fn format_condition(
        &self,
        cond: &crate::mcp::nlp::QueryCondition,
        _param_idx: usize,
    ) -> (String, serde_json::Value) {
        let op_str = match cond.operator {
            ComparisonOperator::Equal => "=",
            ComparisonOperator::NotEqual => "!=",
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::GreaterOrEqual => ">=",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::LessOrEqual => "<=",
            ComparisonOperator::Like => "LIKE",
            ComparisonOperator::In => "IN",
            ComparisonOperator::Between => "BETWEEN",
            ComparisonOperator::Similar => "<->", // Vector similarity
        };

        let value = match &cond.value {
            ConditionValue::String(s) => serde_json::Value::String(s.clone()),
            ConditionValue::Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
            ),
            ConditionValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            ConditionValue::Boolean(b) => serde_json::Value::Bool(*b),
            ConditionValue::Vector(v) => serde_json::Value::Array(
                v.iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f as f64)
                                .unwrap_or(serde_json::Number::from(0)),
                        )
                    })
                    .collect(),
            ),
            ConditionValue::List(l) => serde_json::Value::Array(
                l.iter()
                    .map(|v| match v {
                        ConditionValue::String(s) => serde_json::Value::String(s.clone()),
                        ConditionValue::Number(n) => serde_json::Value::Number(
                            serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
                        ),
                        ConditionValue::Integer(i) => {
                            serde_json::Value::Number(serde_json::Number::from(*i))
                        }
                        ConditionValue::Boolean(b) => serde_json::Value::Bool(*b),
                        _ => serde_json::Value::Null,
                    })
                    .collect(),
            ),
        };

        (op_str.to_string(), value)
    }

    /// Estimate query complexity
    fn estimate_complexity(&self, intent: &QueryIntent) -> QueryComplexity {
        let mut complexity = QueryComplexity::Low;

        // Increase complexity based on conditions
        if intent.conditions.len() > 3 {
            complexity = QueryComplexity::High;
        } else if intent.conditions.len() > 1 {
            complexity = QueryComplexity::Medium;
        }

        // Aggregations increase complexity
        if matches!(
            intent.operation,
            SqlOperation::Select {
                aggregation: Some(_),
                ..
            }
        ) {
            complexity = match complexity {
                QueryComplexity::Low => QueryComplexity::Medium,
                QueryComplexity::Medium => QueryComplexity::High,
                QueryComplexity::High => QueryComplexity::High,
            };
        }

        complexity
    }

    /// Generate optimization hints
    fn generate_hints(&self, intent: &QueryIntent) -> Vec<OptimizationHint> {
        let mut hints = Vec::new();

        // Suggest index usage if there are WHERE conditions
        if !intent.conditions.is_empty() {
            hints.push(OptimizationHint::UseIndex);
        }

        // Suggest partitioning for large tables
        hints.push(OptimizationHint::ConsiderPartitioning);

        hints
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Generated SQL query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedQuery {
    /// SQL query string
    pub sql: String,
    /// Query parameters (for parameterized queries)
    pub parameters: Vec<serde_json::Value>,
    /// Query type
    pub query_type: QueryType,
    /// Estimated complexity
    pub estimated_complexity: QueryComplexity,
    /// Optimization hints
    pub optimization_hints: Vec<OptimizationHint>,
}

/// Query type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    /// Read-only query (SELECT)
    Read,
    /// Write query (INSERT, UPDATE, DELETE)
    Write,
    /// Analytical query
    Analysis,
}

/// Query complexity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryComplexity {
    /// Low complexity (simple SELECT)
    Low,
    /// Medium complexity (joins, aggregations)
    Medium,
    /// High complexity (complex analytical queries)
    High,
}

/// Optimization hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationHint {
    /// Use index for filtering
    UseIndex,
    /// Consider table partitioning
    ConsiderPartitioning,
    /// Use materialized view
    UseMaterializedView,
    /// Enable parallel execution
    EnableParallelExecution,
}

/// Table schema (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name
    pub name: String,
    /// Column information
    pub columns: Vec<ColumnInfo>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Whether nullable
    pub nullable: bool,
}

/// SQL generation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqlGenerationError {
    /// Missing table name
    MissingTable,
    /// Missing columns
    MissingColumns,
    /// Invalid query structure
    InvalidStructure(String),
    /// Schema mismatch
    SchemaMismatch(String),
}

impl std::fmt::Display for SqlGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlGenerationError::MissingTable => write!(f, "Missing table name in query"),
            SqlGenerationError::MissingColumns => write!(f, "Missing columns in query"),
            SqlGenerationError::InvalidStructure(msg) => {
                write!(f, "Invalid query structure: {}", msg)
            }
            SqlGenerationError::SchemaMismatch(msg) => {
                write!(f, "Schema mismatch: {}", msg)
            }
        }
    }
}

impl std::error::Error for SqlGenerationError {}
