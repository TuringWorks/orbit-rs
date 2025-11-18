//! SQL Query Optimization Framework
//!
//! This module provides a comprehensive query optimization framework supporting
//! both rule-based and cost-based optimization strategies. The optimizer works
//! by transforming the parsed AST into an optimized execution plan.
//!
//! ## Architecture
//!
//! - **Rule-Based Optimizer**: Applies predefined transformation rules
//! - **Cost-Based Optimizer**: Uses statistics to choose optimal execution plans
//! - **Execution Plan Generator**: Converts optimized AST to execution plans
//! - **Statistics Collector**: Maintains table and index statistics
//! - **Query Hint System**: Allows manual optimization hints

pub mod costs;
pub mod planner;
pub mod rules;
pub mod stats;
pub mod stats_enhanced;

use crate::error::ProtocolResult;
use crate::postgres_wire::sql::ast::Statement;

/// Query optimization hints that can be provided to guide the optimizer
#[derive(Debug, Clone, PartialEq)]
pub enum QueryHint {
    /// Force use of a specific index
    UseIndex { table: String, index: String },
    /// Disable use of indexes
    NoIndex { table: String },
    /// Force a specific join order
    JoinOrder { tables: Vec<String> },
    /// Force a specific join algorithm
    JoinMethod {
        left_table: String,
        right_table: String,
        method: JoinMethod,
    },
    /// Disable specific optimization
    NoOptimization { optimization: String },
    /// Set parallel execution level
    Parallel { workers: usize },
    /// Force materialization of subquery
    Materialize { subquery_alias: String },
}

/// Join methods that can be hinted
#[derive(Debug, Clone, PartialEq)]
pub enum JoinMethod {
    NestedLoop,
    Hash,
    Merge,
}

/// Container for query hints
#[derive(Debug, Clone, Default)]
pub struct QueryHints {
    pub hints: Vec<QueryHint>,
}

/// Query optimization configuration
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Enable rule-based optimization
    pub enable_rule_based: bool,

    /// Enable cost-based optimization
    pub enable_cost_based: bool,

    /// Maximum optimization passes
    pub max_passes: usize,

    /// Enable join reordering
    pub enable_join_reorder: bool,

    /// Enable predicate pushdown
    pub enable_predicate_pushdown: bool,

    /// Enable projection pushdown
    pub enable_projection_pushdown: bool,

    /// Enable constant folding
    pub enable_constant_folding: bool,

    /// Enable subquery optimization
    pub enable_subquery_optimization: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enable_rule_based: true,
            enable_cost_based: true,
            max_passes: 10,
            enable_join_reorder: true,
            enable_predicate_pushdown: true,
            enable_projection_pushdown: true,
            enable_constant_folding: true,
            enable_subquery_optimization: true,
        }
    }
}

/// Main query optimizer
pub struct QueryOptimizer {
    config: OptimizerConfig,
    rule_optimizer: rules::RuleBasedOptimizer,
    cost_optimizer: costs::CostBasedOptimizer,
    planner: planner::ExecutionPlanner,
    stats: stats::StatisticsCollector,
    hints: QueryHints,
}

impl QueryOptimizer {
    /// Create a new query optimizer
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            rule_optimizer: rules::RuleBasedOptimizer::new(&config),
            cost_optimizer: costs::CostBasedOptimizer::new(&config),
            planner: planner::ExecutionPlanner::new(),
            stats: stats::StatisticsCollector::new(),
            hints: QueryHints::default(),
            config,
        }
    }

    /// Create a new optimizer with default configuration
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(OptimizerConfig::default())
    }

    /// Set query hints for optimization
    pub fn set_hints(&mut self, hints: QueryHints) {
        self.hints = hints;
    }

    /// Add a single hint
    pub fn add_hint(&mut self, hint: QueryHint) {
        self.hints.hints.push(hint);
    }

    /// Clear all hints
    pub fn clear_hints(&mut self) {
        self.hints = QueryHints::default();
    }

    /// Get current hints
    pub fn get_hints(&self) -> &QueryHints {
        &self.hints
    }

    /// Optimize a SQL statement and return an execution plan
    pub fn optimize(&mut self, statement: Statement) -> ProtocolResult<planner::ExecutionPlan> {
        // Step 1: Check for optimization-disabling hints
        if self.has_hint_no_optimization() {
            // Generate plan without optimization
            return self.planner.generate_plan(statement);
        }

        // Step 2: Apply rule-based optimizations
        let optimized_statement = if self.config.enable_rule_based {
            self.rule_optimizer.optimize(statement)?
        } else {
            statement
        };

        // Step 3: Apply cost-based optimizations
        let cost_optimized = if self.config.enable_cost_based {
            self.cost_optimizer
                .optimize(optimized_statement, &self.stats)?
        } else {
            optimized_statement
        };

        // Step 4: Generate execution plan
        let mut plan = self.planner.generate_plan(cost_optimized)?;

        // Step 5: Apply hints to the execution plan
        plan = self.apply_hints_to_plan(plan)?;

        Ok(plan)
    }

    /// Check if there's a hint to disable optimization
    fn has_hint_no_optimization(&self) -> bool {
        self.hints.hints.iter().any(|h| {
            matches!(
                h,
                QueryHint::NoOptimization { optimization } if optimization == "all"
            )
        })
    }

    /// Apply hints to an execution plan
    fn apply_hints_to_plan(
        &self,
        mut plan: planner::ExecutionPlan,
    ) -> ProtocolResult<planner::ExecutionPlan> {
        for hint in &self.hints.hints {
            plan = self.apply_single_hint(plan, hint)?;
        }
        Ok(plan)
    }

    /// Apply a single hint to an execution plan
    fn apply_single_hint(
        &self,
        plan: planner::ExecutionPlan,
        hint: &QueryHint,
    ) -> ProtocolResult<planner::ExecutionPlan> {
        match hint {
            QueryHint::UseIndex { table, index } => {
                // Convert table scans to index scans where applicable
                self.convert_to_index_scan(plan, table, index)
            }
            QueryHint::JoinMethod {
                left_table,
                right_table,
                method,
            } => {
                // Convert join algorithms based on hint
                self.convert_join_method(plan, left_table, right_table, method)
            }
            QueryHint::Parallel { workers: _ } => {
                // Note parallel execution level (execution engine would use this)
                // For now, just add metadata to the plan description
                Ok(plan) // Placeholder - would be used by execution engine
            }
            _ => Ok(plan), // Other hints don't modify the plan structure
        }
    }

    /// Convert table scans to index scans based on hint
    #[allow(clippy::only_used_in_recursion)]
    fn convert_to_index_scan(
        &self,
        plan: planner::ExecutionPlan,
        table_name: &str,
        index_name: &str,
    ) -> ProtocolResult<planner::ExecutionPlan> {
        match plan {
            planner::ExecutionPlan::TableScan {
                table,
                filter,
                projection,
                estimated_rows,
            } if table == table_name => Ok(planner::ExecutionPlan::IndexScan {
                table,
                index: index_name.to_string(),
                filter,
                projection,
                estimated_rows,
            }),
            // Recursively apply to child nodes
            planner::ExecutionPlan::NestedLoopJoin {
                left,
                right,
                condition,
                join_type,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::NestedLoopJoin {
                left: Box::new(self.convert_to_index_scan(*left, table_name, index_name)?),
                right: Box::new(self.convert_to_index_scan(*right, table_name, index_name)?),
                condition,
                join_type,
                estimated_rows,
            }),
            planner::ExecutionPlan::HashJoin {
                build,
                probe,
                condition,
                join_type,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::HashJoin {
                build: Box::new(self.convert_to_index_scan(*build, table_name, index_name)?),
                probe: Box::new(self.convert_to_index_scan(*probe, table_name, index_name)?),
                condition,
                join_type,
                estimated_rows,
            }),
            planner::ExecutionPlan::MergeJoin {
                left,
                right,
                condition,
                join_type,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::MergeJoin {
                left: Box::new(self.convert_to_index_scan(*left, table_name, index_name)?),
                right: Box::new(self.convert_to_index_scan(*right, table_name, index_name)?),
                condition,
                join_type,
                estimated_rows,
            }),
            planner::ExecutionPlan::Filter {
                input,
                condition,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::Filter {
                input: Box::new(self.convert_to_index_scan(*input, table_name, index_name)?),
                condition,
                estimated_rows,
            }),
            planner::ExecutionPlan::Projection {
                input,
                expressions,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::Projection {
                input: Box::new(self.convert_to_index_scan(*input, table_name, index_name)?),
                expressions,
                estimated_rows,
            }),
            planner::ExecutionPlan::Sort {
                input,
                order_by,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::Sort {
                input: Box::new(self.convert_to_index_scan(*input, table_name, index_name)?),
                order_by,
                estimated_rows,
            }),
            planner::ExecutionPlan::Limit {
                input,
                count,
                offset,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::Limit {
                input: Box::new(self.convert_to_index_scan(*input, table_name, index_name)?),
                count,
                offset,
                estimated_rows,
            }),
            planner::ExecutionPlan::Aggregate {
                input,
                group_by,
                aggregates,
                having,
                estimated_rows,
            } => Ok(planner::ExecutionPlan::Aggregate {
                input: Box::new(self.convert_to_index_scan(*input, table_name, index_name)?),
                group_by,
                aggregates,
                having,
                estimated_rows,
            }),
            // For other plan types, return as-is
            other => Ok(other),
        }
    }

    /// Convert join method based on hint
    fn convert_join_method(
        &self,
        plan: planner::ExecutionPlan,
        _left_table: &str,
        _right_table: &str,
        method: &JoinMethod,
    ) -> ProtocolResult<planner::ExecutionPlan> {
        match plan {
            planner::ExecutionPlan::NestedLoopJoin {
                left,
                right,
                condition,
                join_type,
                estimated_rows,
            } if matches!(method, JoinMethod::Hash) => {
                // Convert to hash join
                Ok(planner::ExecutionPlan::HashJoin {
                    build: left,
                    probe: right,
                    condition,
                    join_type,
                    estimated_rows,
                })
            }
            planner::ExecutionPlan::HashJoin {
                build,
                probe,
                condition,
                join_type,
                estimated_rows,
            } if matches!(method, JoinMethod::NestedLoop) => {
                // Convert to nested loop join
                Ok(planner::ExecutionPlan::NestedLoopJoin {
                    left: build,
                    right: probe,
                    condition,
                    join_type,
                    estimated_rows,
                })
            }
            // For other cases, return as-is
            other => Ok(other),
        }
    }

    /// Update table statistics for cost-based optimization
    pub fn update_statistics(&mut self, table_name: &str, stats: stats::TableStatistics) {
        self.stats.update_table_stats(table_name, stats);
    }

    /// Get current optimizer statistics
    pub fn get_statistics(&self) -> &stats::StatisticsCollector {
        &self.stats
    }

    /// Explain query optimization steps
    pub fn explain_optimization(&mut self, statement: Statement) -> ProtocolResult<Vec<String>> {
        let mut steps = Vec::new();

        steps.push(format!("Original query: {statement:#?}"));

        // Rule-based optimization steps
        if self.config.enable_rule_based {
            let (optimized, rule_steps) =
                self.rule_optimizer.optimize_with_steps(statement.clone())?;
            steps.extend(rule_steps);
            steps.push(format!("After rule-based optimization: {optimized:#?}"));
        }

        // Cost-based optimization steps
        if self.config.enable_cost_based {
            let statement = if self.config.enable_rule_based {
                self.rule_optimizer.optimize(statement)?
            } else {
                statement
            };

            let (optimized, cost_steps) = self
                .cost_optimizer
                .optimize_with_steps(statement, &self.stats)?;
            steps.extend(cost_steps);
            steps.push(format!("After cost-based optimization: {optimized:#?}"));
        }

        Ok(steps)
    }

    /// Generate EXPLAIN output for a query
    pub fn explain_query(
        &mut self,
        statement: Statement,
        analyze: bool,
        verbose: bool,
        costs: bool,
    ) -> ProtocolResult<String> {
        let plan = self.optimize(statement)?;
        let mut output = String::new();

        if verbose {
            output.push_str("Query Execution Plan (Verbose)\n");
            output.push_str("================================\n\n");
        } else {
            output.push_str("Query Execution Plan\n");
            output.push_str("====================\n\n");
        }

        self.format_plan_node(&plan, 0, costs, verbose, &mut output);

        if analyze {
            output.push_str("\n\nExecution Statistics:\n");
            output
                .push_str("  Note: ANALYZE not yet implemented - no actual execution performed\n");
        }

        if costs {
            let total_cost = self.estimate_plan_cost(&plan);
            output.push_str(&format!("\n\nTotal Estimated Cost: {total_cost:.2}\n"));
        }

        Ok(output)
    }

    /// Format a plan node for EXPLAIN output
    fn format_plan_node(
        &self,
        plan: &planner::ExecutionPlan,
        indent: usize,
        costs: bool,
        verbose: bool,
        output: &mut String,
    ) {
        let prefix = "  ".repeat(indent);
        let description = plan.describe();
        let estimated_rows = plan.estimated_rows();

        if costs {
            let node_cost = self.estimate_node_cost(plan);
            output.push_str(&format!(
                "{prefix}-> {description} (cost={node_cost:.2}, rows={estimated_rows})\n"
            ));
        } else {
            output.push_str(&format!(
                "{prefix}-> {description} (rows={estimated_rows})\n"
            ));
        }

        if verbose {
            self.add_verbose_details(plan, indent + 1, output);
        }

        // Recursively format child nodes
        match plan {
            planner::ExecutionPlan::NestedLoopJoin { left, right, .. }
            | planner::ExecutionPlan::HashJoin {
                build: left,
                probe: right,
                ..
            }
            | planner::ExecutionPlan::MergeJoin { left, right, .. } => {
                self.format_plan_node(left, indent + 1, costs, verbose, output);
                self.format_plan_node(right, indent + 1, costs, verbose, output);
            }
            planner::ExecutionPlan::Sort { input, .. }
            | planner::ExecutionPlan::Limit { input, .. }
            | planner::ExecutionPlan::Aggregate { input, .. }
            | planner::ExecutionPlan::Projection { input, .. }
            | planner::ExecutionPlan::Filter { input, .. } => {
                self.format_plan_node(input, indent + 1, costs, verbose, output);
            }
            planner::ExecutionPlan::Insert { source, .. } => {
                self.format_plan_node(source, indent + 1, costs, verbose, output);
            }
            _ => {}
        }
    }

    /// Add verbose details about a plan node
    fn add_verbose_details(
        &self,
        plan: &planner::ExecutionPlan,
        indent: usize,
        output: &mut String,
    ) {
        let prefix = "  ".repeat(indent);
        match plan {
            planner::ExecutionPlan::TableScan {
                table,
                filter,
                projection,
                ..
            } => {
                output.push_str(&format!("{prefix}Table: {table}\n"));
                output.push_str(&format!(
                    "{}Projection: {}\n",
                    prefix,
                    projection.join(", ")
                ));
                if let Some(f) = filter {
                    output.push_str(&format!("{prefix}Filter: {f:?}\n"));
                }
            }
            planner::ExecutionPlan::IndexScan {
                table,
                index,
                filter,
                ..
            } => {
                output.push_str(&format!("{prefix}Table: {table}\n"));
                output.push_str(&format!("{prefix}Index: {index}\n"));
                if let Some(f) = filter {
                    output.push_str(&format!("{prefix}Filter: {f:?}\n"));
                }
            }
            _ => {}
        }
    }

    /// Estimate the cost of an entire execution plan
    fn estimate_plan_cost(&self, plan: &planner::ExecutionPlan) -> f64 {
        match plan {
            planner::ExecutionPlan::TableScan { table, .. } => {
                let cost = self
                    .cost_optimizer
                    .estimate_table_scan_cost(table, &self.stats);
                cost.total_cost()
            }
            planner::ExecutionPlan::IndexScan { table, .. } => {
                // Assume 10% selectivity for index scans
                let cost =
                    self.cost_optimizer
                        .estimate_index_scan_cost(table, "index", 0.1, &self.stats);
                cost.total_cost()
            }
            planner::ExecutionPlan::NestedLoopJoin { left, right, .. } => {
                let left_cost = self.estimate_plan_cost(left);
                let right_cost = self.estimate_plan_cost(right);
                left_cost + right_cost * left.estimated_rows() as f64
            }
            planner::ExecutionPlan::HashJoin {
                build: left,
                probe: right,
                ..
            }
            | planner::ExecutionPlan::MergeJoin { left, right, .. } => {
                let left_cost = self.estimate_plan_cost(left);
                let right_cost = self.estimate_plan_cost(right);
                left_cost + right_cost
            }
            planner::ExecutionPlan::Sort { input, .. } => {
                let input_cost = self.estimate_plan_cost(input);
                let n = input.estimated_rows() as f64;
                input_cost + n * n.log2()
            }
            planner::ExecutionPlan::Aggregate { input, .. }
            | planner::ExecutionPlan::Projection { input, .. }
            | planner::ExecutionPlan::Filter { input, .. }
            | planner::ExecutionPlan::Limit { input, .. } => self.estimate_plan_cost(input),
            planner::ExecutionPlan::Insert { source, .. } => self.estimate_plan_cost(source),
            _ => 100.0, // Default cost for other node types
        }
    }

    /// Estimate the cost of a single node (not including children)
    fn estimate_node_cost(&self, plan: &planner::ExecutionPlan) -> f64 {
        match plan {
            planner::ExecutionPlan::TableScan { table, .. } => {
                let cost = self
                    .cost_optimizer
                    .estimate_table_scan_cost(table, &self.stats);
                cost.total_cost()
            }
            planner::ExecutionPlan::IndexScan { table, .. } => {
                let cost =
                    self.cost_optimizer
                        .estimate_index_scan_cost(table, "index", 0.1, &self.stats);
                cost.total_cost()
            }
            planner::ExecutionPlan::Sort { input, .. } => {
                let n = input.estimated_rows() as f64;
                n * n.log2()
            }
            planner::ExecutionPlan::HashJoin { build, probe, .. } => {
                (build.estimated_rows() + probe.estimated_rows()) as f64
            }
            _ => plan.estimated_rows() as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres_wire::sql::ast::{
        BinaryOperator, ColumnRef, Expression, FromClause, SelectItem, SelectStatement, Statement,
        TableName,
    };

    #[test]
    fn test_optimizer_creation() {
        let optimizer = QueryOptimizer::default();
        assert!(optimizer.config.enable_rule_based);
        assert!(optimizer.config.enable_cost_based);
    }

    #[test]
    fn test_optimizer_config() {
        let config = OptimizerConfig {
            enable_rule_based: false,
            enable_cost_based: true,
            max_passes: 5,
            ..Default::default()
        };

        let optimizer = QueryOptimizer::new(config);
        assert!(!optimizer.config.enable_rule_based);
        assert!(optimizer.config.enable_cost_based);
        assert_eq!(optimizer.config.max_passes, 5);
    }

    #[test]
    fn test_explain_query_basic() {
        let mut optimizer = QueryOptimizer::default();

        // Create a simple SELECT statement
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

        let stmt = Statement::Select(Box::new(select));
        let result = optimizer.explain_query(stmt, false, false, false);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Query Execution Plan"));
        assert!(output.contains("Table Scan on users"));
    }

    #[test]
    fn test_explain_query_with_costs() {
        let mut optimizer = QueryOptimizer::default();

        // Add some statistics
        optimizer.update_statistics(
            "products",
            stats::TableStatistics {
                row_count: 1000,
                rows_per_page: 100,
                average_row_size: 50,
                null_frac: 0.1,
                distinct_values: 500,
            },
        );

        let select = SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Table {
                name: TableName::new("products"),
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

        let stmt = Statement::Select(Box::new(select));
        let result = optimizer.explain_query(stmt, false, false, true);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Query Execution Plan"));
        assert!(output.contains("cost="));
        assert!(output.contains("Total Estimated Cost"));
    }

    #[test]
    fn test_explain_query_verbose() {
        let mut optimizer = QueryOptimizer::default();

        let select = SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Table {
                name: TableName::new("orders"),
                alias: None,
            }),
            where_clause: Some(Expression::Binary {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    name: "status".to_string(),
                })),
                operator: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text("pending".to_string()),
                )),
            }),
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
            for_clause: None,
        };

        let stmt = Statement::Select(Box::new(select));
        let result = optimizer.explain_query(stmt, false, true, false);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Verbose"));
    }

    #[test]
    fn test_plan_cost_estimation() {
        let optimizer = QueryOptimizer::default();

        let plan = planner::ExecutionPlan::TableScan {
            table: "test_table".to_string(),
            filter: None,
            projection: vec!["*".to_string()],
            estimated_rows: 1000,
        };

        let cost = optimizer.estimate_plan_cost(&plan);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_query_hints_basic() {
        let mut optimizer = QueryOptimizer::default();

        // Test adding hints
        optimizer.add_hint(QueryHint::UseIndex {
            table: "users".to_string(),
            index: "idx_email".to_string(),
        });

        assert_eq!(optimizer.get_hints().hints.len(), 1);

        // Test clearing hints
        optimizer.clear_hints();
        assert_eq!(optimizer.get_hints().hints.len(), 0);
    }

    #[test]
    fn test_use_index_hint() {
        let mut optimizer = QueryOptimizer::default();

        optimizer.add_hint(QueryHint::UseIndex {
            table: "users".to_string(),
            index: "idx_email".to_string(),
        });

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

        let stmt = Statement::Select(Box::new(select));
        let result = optimizer.optimize(stmt);

        assert!(result.is_ok());
        let plan = result.unwrap();

        // Check that the plan uses an index scan
        match plan {
            planner::ExecutionPlan::Projection { input, .. } => match input.as_ref() {
                planner::ExecutionPlan::IndexScan { table, index, .. } => {
                    assert_eq!(table, "users");
                    assert_eq!(index, "idx_email");
                }
                _ => panic!("Expected IndexScan, got: {:?}", input),
            },
            _ => panic!("Expected Projection plan"),
        }
    }

    #[test]
    fn test_no_optimization_hint() {
        let mut optimizer = QueryOptimizer::default();

        optimizer.add_hint(QueryHint::NoOptimization {
            optimization: "all".to_string(),
        });

        let select = SelectStatement {
            with: None,
            distinct: None,
            select_list: vec![SelectItem::Wildcard],
            from_clause: Some(FromClause::Table {
                name: TableName::new("products"),
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

        let stmt = Statement::Select(Box::new(select));
        let result = optimizer.optimize(stmt);

        assert!(result.is_ok());
    }

    #[test]
    fn test_join_method_hint() {
        let optimizer = QueryOptimizer::default();

        // Create a nested loop join plan
        let left = planner::ExecutionPlan::TableScan {
            table: "orders".to_string(),
            filter: None,
            projection: vec!["*".to_string()],
            estimated_rows: 100,
        };

        let right = planner::ExecutionPlan::TableScan {
            table: "customers".to_string(),
            filter: None,
            projection: vec!["*".to_string()],
            estimated_rows: 50,
        };

        let join_plan = planner::ExecutionPlan::NestedLoopJoin {
            left: Box::new(left),
            right: Box::new(right),
            condition: None,
            join_type: planner::JoinType::Inner,
            estimated_rows: 150,
        };

        // Apply hint to convert to hash join
        let hint = QueryHint::JoinMethod {
            left_table: "orders".to_string(),
            right_table: "customers".to_string(),
            method: JoinMethod::Hash,
        };

        let result = optimizer.apply_single_hint(join_plan, &hint);
        assert!(result.is_ok());

        let converted = result.unwrap();
        match converted {
            planner::ExecutionPlan::HashJoin { .. } => {
                // Success - converted to hash join
            }
            _ => panic!("Expected HashJoin after applying hint"),
        }
    }
}
