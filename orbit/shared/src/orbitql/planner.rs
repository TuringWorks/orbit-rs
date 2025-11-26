//! Query planning engine for OrbitQL
//!
//! This module provides the query planner that converts optimized AST
//! into executable query execution plans.

#[cfg(test)]
use crate::orbitql::ast::BinaryOperator;
use crate::orbitql::ast::{
    AggregateFunction, DeleteStatement, EdgeDirection, Expression, FromClause, GraphPath,
    GraphPattern, GraphStep, InsertStatement, JoinType, SelectField, SelectStatement,
    SortDirection, Statement, TimeRange, TimeSeriesAggregation, TimeWindow, TraverseStatement,
    UpdateStatement,
};
use crate::orbitql::optimizer::OptimizationError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Query planning errors
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanningError {
    UnsupportedOperation(String),
    InvalidPlan(String),
    CostEstimationError(String),
    ResourceConstraintViolation(String),
    OptimizationError(OptimizationError),
}

impl fmt::Display for PlanningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanningError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {msg}"),
            PlanningError::InvalidPlan(msg) => write!(f, "Invalid plan: {msg}"),
            PlanningError::CostEstimationError(msg) => write!(f, "Cost estimation error: {msg}"),
            PlanningError::ResourceConstraintViolation(msg) => {
                write!(f, "Resource constraint violation: {msg}")
            }
            PlanningError::OptimizationError(err) => write!(f, "Optimization error: {err}"),
        }
    }
}

impl std::error::Error for PlanningError {}

impl From<OptimizationError> for PlanningError {
    fn from(error: OptimizationError) -> Self {
        PlanningError::OptimizationError(error)
    }
}

/// Data model types for cross-model operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataModel {
    Document,
    Graph,
    TimeSeries,
    Vector,
}

/// Execution plan node types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanNode {
    TableScan {
        table: String,
        columns: Vec<String>,
        filter: Option<Expression>,
    },
    Filter {
        condition: Expression,
        input: Box<PlanNode>,
    },
    Join {
        join_type: JoinType,
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        condition: Expression,
    },
    /// Cross-model join for joining data from different data models
    CrossModelJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        left_model: DataModel,
        right_model: DataModel,
        left_key: Expression,
        right_key: Expression,
        join_type: JoinType,
    },
    Aggregation {
        input: Box<PlanNode>,
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateExpression>,
    },
    Sort {
        input: Box<PlanNode>,
        expressions: Vec<SortExpression>,
    },
    Limit {
        input: Box<PlanNode>,
        count: u64,
        offset: Option<u64>,
    },
    GraphTraversal {
        pattern: GraphPattern,
        start_nodes: Vec<Expression>,
        max_depth: Option<u32>,
    },
    TimeSeriesQuery {
        metric: String,
        range: TimeRange,
        aggregation: Option<TimeSeriesAggregation>,
        window: Option<TimeWindow>,
    },
}

/// Aggregate expression in execution plan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateExpression {
    pub function: AggregateFunction,
    pub expression: Option<Expression>,
    pub distinct: bool,
    pub alias: Option<String>,
}

/// Sort expression in execution plan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortExpression {
    pub expression: Expression,
    pub direction: SortDirection,
}

/// Complete execution plan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub root: PlanNode,
    pub estimated_cost: f64,
    pub estimated_rows: u64,
}

/// Cost estimation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostStats {
    pub table_stats: HashMap<String, TableStats>,
    pub index_stats: HashMap<String, IndexStats>,
}

/// Table statistics for cost estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub row_count: u64,
    pub size_bytes: u64,
    pub column_stats: HashMap<String, ColumnStats>,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub unique_values: u64,
    pub selectivity: f64,
    pub size_bytes: u64,
}

/// Column statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub unique_values: u64,
    pub null_count: u64,
    pub min_value: Option<serde_json::Value>,
    pub max_value: Option<serde_json::Value>,
}

/// OrbitQL query planner
pub struct QueryPlanner {
    cost_stats: CostStats,
}

impl QueryPlanner {
    pub fn new() -> Self {
        Self {
            cost_stats: CostStats {
                table_stats: HashMap::new(),
                index_stats: HashMap::new(),
            },
        }
    }

    /// Create an execution plan from an optimized AST
    pub fn plan(&self, stmt: Statement) -> Result<ExecutionPlan, PlanningError> {
        match stmt {
            Statement::Select(select) => self.plan_select(select),
            Statement::Insert(insert) => self.plan_insert(insert),
            Statement::Update(update) => self.plan_update(update),
            Statement::Delete(delete) => self.plan_delete(delete),
            Statement::Traverse(traverse) => self.plan_traverse(traverse),
            Statement::Transaction(_) => {
                // Transaction statements don't need complex planning
                Ok(ExecutionPlan {
                    root: PlanNode::TableScan {
                        table: "_transaction".to_string(),
                        columns: vec![],
                        filter: None,
                    },
                    estimated_cost: 0.1,
                    estimated_rows: 0,
                })
            }
            Statement::Relate(_) => {
                // RELATE creates graph edges
                Ok(ExecutionPlan {
                    root: PlanNode::TableScan {
                        table: "_relate".to_string(),
                        columns: vec![],
                        filter: None,
                    },
                    estimated_cost: 1.0,
                    estimated_rows: 1,
                })
            }
            Statement::Live(live) => self.plan_select(*live.query),
            _ => Err(PlanningError::UnsupportedOperation(
                "Statement type not yet supported in planner".to_string(),
            )),
        }
    }

    /// Plan a SELECT statement
    fn plan_select(&self, stmt: SelectStatement) -> Result<ExecutionPlan, PlanningError> {
        // Start with FROM clause
        let mut root = if stmt.from.is_empty() {
            // SELECT without FROM (e.g., SELECT 1)
            return Ok(ExecutionPlan {
                root: PlanNode::TableScan {
                    table: "_dual".to_string(), // Virtual table for constants
                    columns: vec![],
                    filter: None,
                },
                estimated_cost: 1.0,
                estimated_rows: 1,
            });
        } else {
            self.plan_from_clause(&stmt.from[0])?
        };

        // Add additional FROM clauses as cross joins
        for from_clause in stmt.from.iter().skip(1) {
            let right = self.plan_from_clause(from_clause)?;
            root = PlanNode::Join {
                join_type: JoinType::Cross,
                left: Box::new(root),
                right: Box::new(right),
                condition: Expression::Literal(crate::orbitql::QueryValue::Boolean(true)),
            };
        }

        // Add JOIN clauses
        for join in stmt.join_clauses {
            let right = self.plan_from_clause(&join.target)?;
            root = PlanNode::Join {
                join_type: join.join_type,
                left: Box::new(root),
                right: Box::new(right),
                condition: join.condition,
            };
        }

        // Add WHERE filter
        if let Some(where_condition) = stmt.where_clause {
            root = PlanNode::Filter {
                condition: where_condition,
                input: Box::new(root),
            };
        }

        // Add GROUP BY aggregation, or aggregation without GROUP BY (e.g., SELECT COUNT(*))
        let aggregates = self.extract_aggregates_from_fields(&stmt.fields)?;
        if !stmt.group_by.is_empty() || !aggregates.is_empty() {
            root = PlanNode::Aggregation {
                input: Box::new(root),
                group_by: stmt.group_by,
                aggregates,
            };
        }

        // Add HAVING filter (after aggregation)
        if let Some(having_condition) = stmt.having {
            root = PlanNode::Filter {
                condition: having_condition,
                input: Box::new(root),
            };
        }

        // Add ORDER BY sort
        if !stmt.order_by.is_empty() {
            let sort_exprs: Vec<SortExpression> = stmt
                .order_by
                .into_iter()
                .map(|order| SortExpression {
                    expression: order.expression,
                    direction: order.direction,
                })
                .collect();

            root = PlanNode::Sort {
                input: Box::new(root),
                expressions: sort_exprs,
            };
        }

        // Add LIMIT/OFFSET
        if stmt.limit.is_some() || stmt.offset.is_some() {
            root = PlanNode::Limit {
                input: Box::new(root),
                count: stmt.limit.unwrap_or(u64::MAX),
                offset: stmt.offset,
            };
        }

        // Estimate cost and cardinality
        let (estimated_cost, estimated_rows) = self.estimate_plan_cost(&root)?;

        Ok(ExecutionPlan {
            root,
            estimated_cost,
            estimated_rows,
        })
    }

    /// Plan a FROM clause
    fn plan_from_clause(&self, from: &FromClause) -> Result<PlanNode, PlanningError> {
        match from {
            FromClause::Table { name, .. } => {
                Ok(PlanNode::TableScan {
                    table: name.clone(),
                    columns: vec![], // TODO: Determine columns from schema
                    filter: None,
                })
            }
            FromClause::Graph { pattern, .. } => {
                Ok(PlanNode::GraphTraversal {
                    pattern: pattern.clone(),
                    start_nodes: vec![], // TODO: Determine start nodes
                    max_depth: None,
                })
            }
            FromClause::TimeSeries { metric, range, .. } => Ok(PlanNode::TimeSeriesQuery {
                metric: metric.clone(),
                range: range.clone(),
                aggregation: None,
                window: None,
            }),
            FromClause::Subquery { query, .. } => {
                // Recursively plan the subquery
                let subplan = self.plan_select(*query.clone())?;
                Ok(subplan.root)
            }
        }
    }

    /// Extract aggregate expressions from SELECT fields
    fn extract_aggregates_from_fields(
        &self,
        fields: &[SelectField],
    ) -> Result<Vec<AggregateExpression>, PlanningError> {
        let mut aggregates = Vec::new();

        for field in fields {
            if let SelectField::Expression { expr, alias } = field {
                if let Some(Expression::Aggregate {
                    function,
                    expression,
                    distinct,
                }) = self.extract_aggregate_from_expression(expr)
                {
                    aggregates.push(AggregateExpression {
                        function: function.clone(),
                        expression: expression.as_ref().map(|e| (**e).clone()),
                        distinct: *distinct,
                        alias: alias.clone(),
                    });
                }
            }
        }

        Ok(aggregates)
    }

    /// Extract aggregate function from expression
    fn extract_aggregate_from_expression<'a>(
        &self,
        expr: &'a Expression,
    ) -> Option<&'a Expression> {
        match expr {
            Expression::Aggregate { .. } => Some(expr),
            _ => None,
        }
    }

    /// Plan INSERT statement
    fn plan_insert(&self, _stmt: InsertStatement) -> Result<ExecutionPlan, PlanningError> {
        // TODO: Implement INSERT planning
        Err(PlanningError::UnsupportedOperation(
            "INSERT planning not yet implemented".to_string(),
        ))
    }

    /// Plan UPDATE statement
    fn plan_update(&self, _stmt: UpdateStatement) -> Result<ExecutionPlan, PlanningError> {
        // TODO: Implement UPDATE planning
        Err(PlanningError::UnsupportedOperation(
            "UPDATE planning not yet implemented".to_string(),
        ))
    }

    /// Plan DELETE statement
    fn plan_delete(&self, stmt: DeleteStatement) -> Result<ExecutionPlan, PlanningError> {
        let mut root = PlanNode::TableScan {
            table: stmt.from,
            columns: vec![], // For DELETE, we need all columns
            filter: None,
        };

        if let Some(where_condition) = stmt.where_clause {
            root = PlanNode::Filter {
                condition: where_condition,
                input: Box::new(root),
            };
        }

        let (estimated_cost, estimated_rows) = self.estimate_plan_cost(&root)?;

        Ok(ExecutionPlan {
            root,
            estimated_cost,
            estimated_rows,
        })
    }

    /// Plan TRAVERSE statement for graph traversal
    fn plan_traverse(&self, stmt: TraverseStatement) -> Result<ExecutionPlan, PlanningError> {
        // Create a GraphPattern from the TRAVERSE statement
        let graph_pattern = GraphPattern {
            path: GraphPath {
                steps: vec![
                    // Start node
                    GraphStep::Node {
                        label: None,
                        properties: None,
                    },
                    // Edge traversal
                    GraphStep::Edge {
                        direction: EdgeDirection::Outgoing,
                        label: Some(stmt.edge_type),
                        properties: None,
                    },
                    // Target nodes
                    GraphStep::Node {
                        label: None,
                        properties: None,
                    },
                ],
            },
            where_clause: stmt.where_clause,
        };

        let root = PlanNode::GraphTraversal {
            pattern: graph_pattern,
            start_nodes: vec![stmt.from_node],
            max_depth: stmt.max_depth,
        };

        let (estimated_cost, estimated_rows) = self.estimate_plan_cost(&root)?;

        Ok(ExecutionPlan {
            root,
            estimated_cost,
            estimated_rows,
        })
    }

    /// Estimate the cost and cardinality of a plan
    fn estimate_plan_cost(&self, node: &PlanNode) -> Result<(f64, u64), PlanningError> {
        match node {
            PlanNode::TableScan { table, .. } => {
                let stats = self.cost_stats.table_stats.get(table);
                let rows = stats.map_or(1000, |s| s.row_count); // Default estimate
                let cost = rows as f64 * 1.0; // Sequential scan cost
                Ok((cost, rows))
            }
            PlanNode::Filter { input, .. } => {
                let (input_cost, input_rows) = self.estimate_plan_cost(input)?;
                let selectivity = 0.1; // Default selectivity estimate
                let output_rows = (input_rows as f64 * selectivity) as u64;
                let cost = input_cost + input_rows as f64 * 0.1; // Filter cost per row
                Ok((cost, output_rows))
            }
            PlanNode::Join { left, right, .. } => {
                let (left_cost, left_rows) = self.estimate_plan_cost(left)?;
                let (right_cost, right_rows) = self.estimate_plan_cost(right)?;
                let output_rows = left_rows * right_rows / 10; // Join selectivity estimate
                let cost = left_cost + right_cost + (left_rows * right_rows) as f64 * 0.01;
                Ok((cost, output_rows))
            }
            PlanNode::Sort { input, .. } => {
                let (input_cost, input_rows) = self.estimate_plan_cost(input)?;
                let cost = input_cost + input_rows as f64 * (input_rows as f64).log2() * 0.01;
                Ok((cost, input_rows))
            }
            PlanNode::Limit { input, count, .. } => {
                let (input_cost, input_rows) = self.estimate_plan_cost(input)?;
                let output_rows = (*count).min(input_rows);
                Ok((input_cost, output_rows))
            }
            PlanNode::Aggregation { input, .. } => {
                let (input_cost, input_rows) = self.estimate_plan_cost(input)?;
                let output_rows = input_rows / 10; // Aggregation typically reduces rows
                let cost = input_cost + input_rows as f64 * 0.1;
                Ok((cost, output_rows))
            }
            PlanNode::GraphTraversal { .. } => {
                // Graph traversal cost estimation is complex
                // For now, use a simple heuristic
                Ok((1000.0, 100))
            }
            PlanNode::TimeSeriesQuery { .. } => {
                // Time series query cost depends on time range
                // For now, use a simple heuristic
                Ok((500.0, 1000))
            }
            PlanNode::CrossModelJoin {
                left,
                right,
                left_model,
                right_model,
                ..
            } => {
                let (left_cost, left_rows) = self.estimate_plan_cost(left)?;
                let (right_cost, right_rows) = self.estimate_plan_cost(right)?;

                // Cross-model joins have higher overhead due to schema unification
                let model_conversion_cost = match (left_model, right_model) {
                    (DataModel::Document, DataModel::Document) => 1.0,
                    (DataModel::Graph, DataModel::Document)
                    | (DataModel::Document, DataModel::Graph) => 1.5,
                    (DataModel::TimeSeries, DataModel::Document)
                    | (DataModel::Document, DataModel::TimeSeries) => 1.3,
                    (DataModel::Graph, DataModel::TimeSeries)
                    | (DataModel::TimeSeries, DataModel::Graph) => 2.0,
                    _ => 1.5,
                };

                let output_rows = (left_rows * right_rows / 10).max(1); // Join selectivity estimate
                let cost = (left_cost + right_cost + (left_rows * right_rows) as f64 * 0.01)
                    * model_conversion_cost;
                Ok((cost, output_rows))
            }
        }
    }

    /// Update cost statistics
    pub fn update_stats(&mut self, stats: CostStats) {
        self.cost_stats = stats;
    }

    /// Get current cost statistics
    pub fn get_stats(&self) -> &CostStats {
        &self.cost_stats
    }
}

impl Default for QueryPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_creation() {
        let planner = QueryPlanner::new();
        assert_eq!(planner.get_stats().table_stats.len(), 0);
    }

    #[test]
    fn test_plan_simple_select() {
        let planner = QueryPlanner::new();

        let stmt = Statement::Select(SelectStatement {
            with_clauses: Vec::new(),
            fields: vec![SelectField::All],
            from: vec![FromClause::Table {
                name: "users".to_string(),
                alias: None,
            }],
            where_clause: None,
            join_clauses: vec![],
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
            fetch: vec![],
            timeout: None,
        });

        let plan = planner.plan(stmt).unwrap();

        if let PlanNode::TableScan { table, .. } = plan.root {
            assert_eq!(table, "users");
        } else {
            panic!("Expected table scan");
        }
    }

    #[test]
    fn test_plan_select_with_where() {
        let planner = QueryPlanner::new();

        let stmt = Statement::Select(SelectStatement {
            with_clauses: Vec::new(),
            fields: vec![SelectField::All],
            from: vec![FromClause::Table {
                name: "users".to_string(),
                alias: None,
            }],
            where_clause: Some(Expression::Binary {
                left: Box::new(Expression::Identifier("age".to_string())),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(crate::orbitql::QueryValue::Integer(18))),
            }),
            join_clauses: vec![],
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
            fetch: vec![],
            timeout: None,
        });

        let plan = planner.plan(stmt).unwrap();

        if let PlanNode::Filter { input, .. } = plan.root {
            if let PlanNode::TableScan { table, .. } = input.as_ref() {
                assert_eq!(table, "users");
            } else {
                panic!("Expected table scan as input to filter");
            }
        } else {
            panic!("Expected filter node");
        }
    }

    #[test]
    fn test_plan_delete() {
        let planner = QueryPlanner::new();

        let stmt = Statement::Delete(DeleteStatement {
            from: "users".to_string(),
            where_clause: Some(Expression::Binary {
                left: Box::new(Expression::Identifier("active".to_string())),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(crate::orbitql::QueryValue::Boolean(
                    false,
                ))),
            }),
        });

        let plan = planner.plan(stmt).unwrap();

        if let PlanNode::Filter { input, .. } = plan.root {
            if let PlanNode::TableScan { table, .. } = input.as_ref() {
                assert_eq!(table, "users");
            } else {
                panic!("Expected table scan as input to filter");
            }
        } else {
            panic!("Expected filter node");
        }
    }
}
