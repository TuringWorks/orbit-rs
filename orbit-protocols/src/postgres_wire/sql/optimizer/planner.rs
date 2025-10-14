//! Query Execution Planning
//!
//! This module converts optimized SQL AST into executable query plans.
//! The execution plan represents the physical operations needed to
//! execute the query efficiently.

use crate::error::ProtocolResult;
use crate::postgres_wire::sql::ast::{
    Assignment, DeleteStatement, Expression, FromClause, FunctionName, InsertSource,
    InsertStatement, JoinCondition, OrderByItem, SelectItem, SelectStatement, Statement,
    UpdateStatement,
};
#[cfg(test)]
use crate::postgres_wire::sql::ast::{ColumnRef, FunctionCall, TableName};
use std::fmt;

/// Physical execution plan for a query
#[derive(Debug, Clone)]
pub enum ExecutionPlan {
    /// Table scan operation
    TableScan {
        table: String,
        filter: Option<Expression>,
        projection: Vec<String>,
        estimated_rows: usize,
    },

    /// Index scan operation
    IndexScan {
        table: String,
        index: String,
        filter: Option<Expression>,
        projection: Vec<String>,
        estimated_rows: usize,
    },

    /// Nested loop join
    NestedLoopJoin {
        left: Box<ExecutionPlan>,
        right: Box<ExecutionPlan>,
        condition: Option<Expression>,
        join_type: JoinType,
        estimated_rows: usize,
    },

    /// Hash join
    HashJoin {
        build: Box<ExecutionPlan>, // Smaller relation for building hash table
        probe: Box<ExecutionPlan>, // Larger relation for probing
        condition: Option<Expression>,
        join_type: JoinType,
        estimated_rows: usize,
    },

    /// Merge join (requires sorted inputs)
    MergeJoin {
        left: Box<ExecutionPlan>,
        right: Box<ExecutionPlan>,
        condition: Option<Expression>,
        join_type: JoinType,
        estimated_rows: usize,
    },

    /// Sort operation
    Sort {
        input: Box<ExecutionPlan>,
        order_by: Vec<OrderByItem>,
        estimated_rows: usize,
    },

    /// Limit operation
    Limit {
        input: Box<ExecutionPlan>,
        count: Option<usize>,
        offset: Option<usize>,
        estimated_rows: usize,
    },

    /// Aggregation operation
    Aggregate {
        input: Box<ExecutionPlan>,
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateFunction>,
        having: Option<Expression>,
        estimated_rows: usize,
    },

    /// Insert operation
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        source: Box<ExecutionPlan>,
        estimated_rows: usize,
    },

    /// Update operation
    Update {
        table: String,
        set_expressions: Vec<Assignment>,
        filter: Option<Expression>,
        estimated_rows: usize,
    },

    /// Delete operation
    Delete {
        table: String,
        filter: Option<Expression>,
        estimated_rows: usize,
    },

    /// Values (constant table) operation
    Values {
        values: Vec<Vec<Expression>>,
        estimated_rows: usize,
    },

    /// Projection (SELECT list processing)
    Projection {
        input: Box<ExecutionPlan>,
        expressions: Vec<SelectItem>,
        estimated_rows: usize,
    },

    /// Filter (WHERE clause processing)
    Filter {
        input: Box<ExecutionPlan>,
        condition: Expression,
        estimated_rows: usize,
    },
}

/// Join type for execution plans
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
}

/// Aggregate function in execution plan
#[derive(Debug, Clone)]
pub struct AggregateFunction {
    pub function: AggregateFunctionType,
    pub argument: Option<Expression>,
    pub distinct: bool,
    pub alias: Option<String>,
}

/// Types of aggregate functions
#[derive(Debug, Clone)]
pub enum AggregateFunctionType {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    StdDev,
    Variance,
}

/// Execution plan generator
pub struct ExecutionPlanner {
    #[allow(dead_code)]
    next_plan_id: usize,
}

impl ExecutionPlanner {
    /// Create a new execution planner
    pub fn new() -> Self {
        Self { next_plan_id: 1 }
    }

    /// Generate an execution plan for a statement
    pub fn generate_plan(&mut self, statement: Statement) -> ProtocolResult<ExecutionPlan> {
        match statement {
            Statement::Select(select) => self.plan_select(*select),
            Statement::Insert(insert) => self.plan_insert(insert),
            Statement::Update(update) => self.plan_update(update),
            Statement::Delete(delete) => self.plan_delete(delete),
            _ => Err(crate::error::ProtocolError::Other(
                "Unsupported statement type for execution planning".to_string(),
            )),
        }
    }

    /// Plan a SELECT statement
    fn plan_select(&mut self, select: SelectStatement) -> ProtocolResult<ExecutionPlan> {
        // Start with the base table scan or join
        let mut plan = if let Some(from_clause) = select.from_clause {
            self.plan_from_clause(from_clause)?
        } else {
            // SELECT without FROM (e.g., SELECT 1 + 2)
            ExecutionPlan::Values {
                values: vec![vec![]],
                estimated_rows: 1,
            }
        };

        // Add WHERE clause as filter
        if let Some(where_clause) = select.where_clause {
            plan = ExecutionPlan::Filter {
                estimated_rows: plan.estimated_rows() / 2, // Rough estimate
                condition: where_clause,
                input: Box::new(plan),
            };
        }

        // Add GROUP BY and aggregations
        if select.group_by.is_some() || self.has_aggregates(&select.select_list) {
            plan = self.plan_aggregation(
                plan,
                select.group_by.unwrap_or_default(),
                &select.select_list,
                select.having,
            )?;
        }

        // Add ORDER BY
        if let Some(order_by) = select.order_by {
            plan = ExecutionPlan::Sort {
                estimated_rows: plan.estimated_rows(),
                input: Box::new(plan),
                order_by,
            };
        }

        // Add LIMIT/OFFSET
        if let Some(limit) = select.limit {
            let count = match limit.count {
                Some(Expression::Literal(crate::postgres_wire::sql::types::SqlValue::Integer(
                    n,
                ))) => Some(n as usize),
                _ => None,
            };
            let offset = select.offset.map(|n| n as usize);

            plan = ExecutionPlan::Limit {
                estimated_rows: count
                    .unwrap_or(plan.estimated_rows())
                    .min(plan.estimated_rows()),
                input: Box::new(plan),
                count,
                offset,
            };
        }

        // Add final projection
        plan = ExecutionPlan::Projection {
            estimated_rows: plan.estimated_rows(),
            input: Box::new(plan),
            expressions: select.select_list,
        };

        Ok(plan)
    }

    /// Plan FROM clause (tables and joins)
    fn plan_from_clause(&mut self, from_clause: FromClause) -> ProtocolResult<ExecutionPlan> {
        match from_clause {
            FromClause::Table { name, alias: _, .. } => {
                Ok(ExecutionPlan::TableScan {
                    table: name.full_name(),
                    filter: None,
                    projection: vec!["*".to_string()],
                    estimated_rows: 1000, // Default estimate
                })
            }
            FromClause::Join {
                left,
                right,
                join_type,
                condition,
            } => {
                let left_plan = self.plan_from_clause(*left)?;
                let right_plan = self.plan_from_clause(*right)?;

                // Choose join algorithm (simplified - just use nested loop for now)
                // Note: join_type is already the correct type from AST

                Ok(ExecutionPlan::NestedLoopJoin {
                    estimated_rows: left_plan.estimated_rows() * right_plan.estimated_rows() / 10,
                    left: Box::new(left_plan),
                    right: Box::new(right_plan),
                    condition: match condition {
                        JoinCondition::On(expr) => Some(expr),
                        _ => None, // Natural and Using joins not fully implemented
                    },
                    join_type: match join_type {
                        crate::postgres_wire::sql::ast::JoinType::Inner => JoinType::Inner,
                        crate::postgres_wire::sql::ast::JoinType::LeftOuter => JoinType::LeftOuter,
                        crate::postgres_wire::sql::ast::JoinType::RightOuter => {
                            JoinType::RightOuter
                        }
                        crate::postgres_wire::sql::ast::JoinType::FullOuter => JoinType::FullOuter,
                        crate::postgres_wire::sql::ast::JoinType::Cross => JoinType::Cross,
                        _ => JoinType::Inner, // Default for unsupported join types
                    },
                })
            }
            FromClause::Subquery { query, alias: _ } => self.plan_select(*query),
            FromClause::Values { values, alias: _ } => Ok(ExecutionPlan::Values {
                estimated_rows: values.len(),
                values,
            }),
            FromClause::TableFunction {
                function: _,
                alias: _,
            } => {
                // Placeholder for table functions
                Ok(ExecutionPlan::TableScan {
                    table: "table_function".to_string(),
                    filter: None,
                    projection: vec!["*".to_string()],
                    estimated_rows: 100,
                })
            }
        }
    }

    /// Plan INSERT statement
    fn plan_insert(&mut self, insert: InsertStatement) -> ProtocolResult<ExecutionPlan> {
        let source_plan = match insert.source {
            InsertSource::Values(value_lists) => ExecutionPlan::Values {
                estimated_rows: value_lists.len(),
                values: value_lists,
            },
            InsertSource::Query(query) => self.plan_select(*query)?,
            InsertSource::DefaultValues => {
                ExecutionPlan::Values {
                    estimated_rows: 1,
                    values: vec![vec![]], // Empty row - use defaults
                }
            }
        };

        Ok(ExecutionPlan::Insert {
            table: insert.table.full_name(),
            columns: insert.columns,
            estimated_rows: source_plan.estimated_rows(),
            source: Box::new(source_plan),
        })
    }

    /// Plan UPDATE statement
    fn plan_update(&mut self, update: UpdateStatement) -> ProtocolResult<ExecutionPlan> {
        Ok(ExecutionPlan::Update {
            table: update.table.full_name(),
            set_expressions: update.set,
            filter: update.where_clause,
            estimated_rows: 100, // Rough estimate
        })
    }

    /// Plan DELETE statement
    fn plan_delete(&mut self, delete: DeleteStatement) -> ProtocolResult<ExecutionPlan> {
        Ok(ExecutionPlan::Delete {
            table: delete.table.full_name(),
            filter: delete.where_clause,
            estimated_rows: 100, // Rough estimate
        })
    }

    /// Plan aggregation operations
    fn plan_aggregation(
        &mut self,
        input: ExecutionPlan,
        group_by: Vec<Expression>,
        select_list: &[SelectItem],
        having: Option<Expression>,
    ) -> ProtocolResult<ExecutionPlan> {
        let aggregates = self.extract_aggregates(select_list)?;

        Ok(ExecutionPlan::Aggregate {
            estimated_rows: if group_by.is_empty() {
                1
            } else {
                input.estimated_rows() / 10
            },
            input: Box::new(input),
            group_by,
            aggregates,
            having,
        })
    }

    /// Check if select list contains aggregate functions
    fn has_aggregates(&self, select_list: &[SelectItem]) -> bool {
        select_list.iter().any(|item| {
            if let SelectItem::Expression { expr, .. } = item {
                self.expression_has_aggregate(expr)
            } else {
                false
            }
        })
    }

    /// Check if an expression contains aggregate functions
    #[allow(clippy::only_used_in_recursion)]
    fn expression_has_aggregate(&self, expr: &Expression) -> bool {
        match expr {
            Expression::Function(func_call) => {
                let name = match &func_call.name {
                    FunctionName::Simple(n) => n,
                    FunctionName::Qualified { name, .. } => name,
                };
                matches!(
                    name.to_uppercase().as_str(),
                    "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"
                )
            }
            Expression::Binary { left, right, .. } => {
                self.expression_has_aggregate(left) || self.expression_has_aggregate(right)
            }
            Expression::Unary { operand, .. } => self.expression_has_aggregate(operand),
            _ => false,
        }
    }

    /// Extract aggregate functions from select list
    fn extract_aggregates(
        &self,
        select_list: &[SelectItem],
    ) -> ProtocolResult<Vec<AggregateFunction>> {
        let mut aggregates = Vec::new();

        for item in select_list {
            if let SelectItem::Expression { expr, alias } = item {
                if let Some(agg) = self.extract_aggregate_from_expression(expr, alias.clone())? {
                    aggregates.push(agg);
                }
            }
        }

        Ok(aggregates)
    }

    /// Extract aggregate function from a single expression
    fn extract_aggregate_from_expression(
        &self,
        expr: &Expression,
        alias: Option<String>,
    ) -> ProtocolResult<Option<AggregateFunction>> {
        if let Expression::Function(func_call) = expr {
            let name = match &func_call.name {
                FunctionName::Simple(n) => n,
                FunctionName::Qualified { name, .. } => name,
            };
            let function = match name.to_uppercase().as_str() {
                "COUNT" => AggregateFunctionType::Count,
                "SUM" => AggregateFunctionType::Sum,
                "AVG" => AggregateFunctionType::Avg,
                "MIN" => AggregateFunctionType::Min,
                "MAX" => AggregateFunctionType::Max,
                _ => return Ok(None),
            };

            let argument = func_call.args.first().cloned();

            return Ok(Some(AggregateFunction {
                function,
                argument,
                distinct: func_call.distinct,
                alias,
            }));
        }

        Ok(None)
    }

    /// Get next unique plan ID
    fn _next_id(&mut self) -> usize {
        let id = self.next_plan_id;
        self.next_plan_id += 1;
        id
    }
}

impl ExecutionPlan {
    /// Get estimated number of rows this plan will produce
    pub fn estimated_rows(&self) -> usize {
        match self {
            ExecutionPlan::TableScan { estimated_rows, .. }
            | ExecutionPlan::IndexScan { estimated_rows, .. }
            | ExecutionPlan::NestedLoopJoin { estimated_rows, .. }
            | ExecutionPlan::HashJoin { estimated_rows, .. }
            | ExecutionPlan::MergeJoin { estimated_rows, .. }
            | ExecutionPlan::Sort { estimated_rows, .. }
            | ExecutionPlan::Limit { estimated_rows, .. }
            | ExecutionPlan::Aggregate { estimated_rows, .. }
            | ExecutionPlan::Insert { estimated_rows, .. }
            | ExecutionPlan::Update { estimated_rows, .. }
            | ExecutionPlan::Delete { estimated_rows, .. }
            | ExecutionPlan::Values { estimated_rows, .. }
            | ExecutionPlan::Projection { estimated_rows, .. }
            | ExecutionPlan::Filter { estimated_rows, .. } => *estimated_rows,
        }
    }

    /// Get a human-readable description of this plan
    pub fn describe(&self) -> String {
        match self {
            ExecutionPlan::TableScan { table, filter, .. } => {
                if filter.is_some() {
                    format!("Table Scan on {} (filtered)", table)
                } else {
                    format!("Table Scan on {}", table)
                }
            }
            ExecutionPlan::IndexScan { table, index, .. } => {
                format!("Index Scan on {}.{}", table, index)
            }
            ExecutionPlan::NestedLoopJoin { join_type, .. } => {
                format!("{:?} Nested Loop Join", join_type)
            }
            ExecutionPlan::HashJoin { join_type, .. } => {
                format!("{:?} Hash Join", join_type)
            }
            ExecutionPlan::MergeJoin { join_type, .. } => {
                format!("{:?} Merge Join", join_type)
            }
            ExecutionPlan::Sort { .. } => "Sort".to_string(),
            ExecutionPlan::Limit { .. } => "Limit".to_string(),
            ExecutionPlan::Aggregate { .. } => "Aggregate".to_string(),
            ExecutionPlan::Insert { table, .. } => format!("Insert into {}", table),
            ExecutionPlan::Update { table, .. } => format!("Update {}", table),
            ExecutionPlan::Delete { table, .. } => format!("Delete from {}", table),
            ExecutionPlan::Values { .. } => "Values".to_string(),
            ExecutionPlan::Projection { .. } => "Projection".to_string(),
            ExecutionPlan::Filter { .. } => "Filter".to_string(),
        }
    }
}

impl fmt::Display for ExecutionPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (rows: {})", self.describe(), self.estimated_rows())
    }
}

impl Default for ExecutionPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres_wire::sql::types::SqlValue;

    #[test]
    fn test_execution_planner_creation() {
        let planner = ExecutionPlanner::new();
        assert_eq!(planner.next_plan_id, 1);
    }

    #[test]
    fn test_table_scan_plan() {
        let mut planner = ExecutionPlanner::new();

        // Create a simple SELECT * FROM users
        let select = SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Table {
                name: TableName::new("users"),
                alias: None,
            }),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
            for_clause: None,
        };

        let plan = planner.plan_select(select).unwrap();

        match plan {
            ExecutionPlan::Projection { input, .. } => match input.as_ref() {
                ExecutionPlan::TableScan { table, .. } => {
                    assert_eq!(table, "users");
                }
                _ => panic!("Expected TableScan as input to Projection"),
            },
            _ => panic!("Expected Projection plan"),
        }
    }

    #[test]
    fn test_insert_plan() {
        let mut planner = ExecutionPlanner::new();

        let insert = InsertStatement {
            table: TableName::new("users"),
            columns: Some(vec!["name".to_string(), "email".to_string()]),
            source: InsertSource::Values(vec![vec![
                Expression::Literal(SqlValue::Text("John".to_string())),
                Expression::Literal(SqlValue::Text("john@example.com".to_string())),
            ]]),
            on_conflict: None,
            returning: None,
        };

        let plan = planner.plan_insert(insert).unwrap();

        match plan {
            ExecutionPlan::Insert { table, columns, .. } => {
                assert_eq!(table, "users");
                assert_eq!(columns.as_ref().unwrap().len(), 2);
            }
            _ => panic!("Expected Insert plan"),
        }
    }

    #[test]
    fn test_aggregate_detection() {
        let planner = ExecutionPlanner::new();

        let select_list = vec![SelectItem::Expression {
            expr: Expression::Function(Box::new(FunctionCall {
                name: FunctionName::Simple("COUNT".to_string()),
                args: vec![], // Simplified for test
                distinct: false,
                order_by: None,
                filter: None,
            })),
            alias: None,
        }];

        assert!(planner.has_aggregates(&select_list));

        let non_agg_list = vec![SelectItem::Expression {
            expr: Expression::Column(ColumnRef {
                table: None,
                name: "id".to_string(),
            }),
            alias: None,
        }];

        assert!(!planner.has_aggregates(&non_agg_list));
    }

    #[test]
    fn test_execution_plan_estimated_rows() {
        let plan = ExecutionPlan::TableScan {
            table: "test".to_string(),
            filter: None,
            projection: vec!["*".to_string()],
            estimated_rows: 5000,
        };

        assert_eq!(plan.estimated_rows(), 5000);
    }

    #[test]
    fn test_execution_plan_description() {
        let plan = ExecutionPlan::TableScan {
            table: "users".to_string(),
            filter: None,
            projection: vec!["*".to_string()],
            estimated_rows: 1000,
        };

        assert_eq!(plan.describe(), "Table Scan on users");
        assert_eq!(plan.to_string(), "Table Scan on users (rows: 1000)");
    }
}
