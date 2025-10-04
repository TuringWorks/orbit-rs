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

pub mod rules;
pub mod costs;
pub mod planner;
pub mod stats;

use crate::postgres_wire::sql::ast::*;
use crate::error::ProtocolResult;

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
}

impl QueryOptimizer {
    /// Create a new query optimizer
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            rule_optimizer: rules::RuleBasedOptimizer::new(&config),
            cost_optimizer: costs::CostBasedOptimizer::new(&config),
            planner: planner::ExecutionPlanner::new(),
            stats: stats::StatisticsCollector::new(),
            config,
        }
    }

    /// Create a new optimizer with default configuration
    pub fn default() -> Self {
        Self::new(OptimizerConfig::default())
    }

    /// Optimize a SQL statement and return an execution plan
    pub fn optimize(&mut self, statement: Statement) -> ProtocolResult<planner::ExecutionPlan> {
        // Step 1: Apply rule-based optimizations
        let optimized_statement = if self.config.enable_rule_based {
            self.rule_optimizer.optimize(statement)?
        } else {
            statement
        };

        // Step 2: Apply cost-based optimizations
        let cost_optimized = if self.config.enable_cost_based {
            self.cost_optimizer.optimize(optimized_statement, &self.stats)?
        } else {
            optimized_statement
        };

        // Step 3: Generate execution plan
        self.planner.generate_plan(cost_optimized)
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

        steps.push(format!("Original query: {:#?}", statement));

        // Rule-based optimization steps
        if self.config.enable_rule_based {
            let (optimized, rule_steps) = self.rule_optimizer.optimize_with_steps(statement.clone())?;
            steps.extend(rule_steps);
            steps.push(format!("After rule-based optimization: {:#?}", optimized));
        }

        // Cost-based optimization steps
        if self.config.enable_cost_based {
            let statement = if self.config.enable_rule_based {
                self.rule_optimizer.optimize(statement)?
            } else {
                statement
            };
            
            let (optimized, cost_steps) = self.cost_optimizer.optimize_with_steps(statement, &self.stats)?;
            steps.extend(cost_steps);
            steps.push(format!("After cost-based optimization: {:#?}", optimized));
        }

        Ok(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}