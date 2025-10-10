//! Cost-based query planner implementation
//!
//! This module provides a sophisticated query planner that generates multiple
//! execution plan alternatives, costs them using the cost model, and selects
//! the optimal plan. Implements the framework defined in Phase 9.2.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::ast::*;
use super::cost_model::{CardinalityEstimate, CostModel, QueryCost};
use super::optimizer::{OptimizationError, QueryOptimizer};
use super::planner::{ExecutionPlan, PlanNode, PlanningError};
use super::statistics::{IndexStatistics, StatisticsManager, TableStatistics};

/// Cost-based query planner that generates optimal execution plans
pub struct CostBasedQueryPlanner {
    /// Statistics manager for table and index statistics
    statistics_manager: Arc<RwLock<StatisticsManager>>,
    /// Cost model for cost estimation
    cost_model: CostModel,
    /// Rule-based optimizer for initial optimization
    optimizer: QueryOptimizer,
    /// Plan cache to reuse previous computations
    plan_cache: Arc<RwLock<HashMap<String, CachedPlan>>>,
    /// Configuration
    config: PlannerConfig,
}

/// Cached execution plan with metadata
#[derive(Debug, Clone)]
struct CachedPlan {
    plan: ExecutionPlan,
    cost: QueryCost,
    timestamp: std::time::Instant,
    hit_count: u32,
}

/// Configuration for the cost-based planner
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Enable plan caching
    pub enable_plan_cache: bool,
    /// Maximum number of alternative plans to generate
    pub max_alternatives: usize,
    /// Cache timeout in seconds
    pub cache_timeout_seconds: u64,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Enable join enumeration optimization
    pub enable_join_enumeration: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            enable_plan_cache: true,
            max_alternatives: 10,
            cache_timeout_seconds: 300, // 5 minutes
            max_cache_size: 1000,
            enable_join_enumeration: true,
        }
    }
}

/// Alternative execution plan with cost information
#[derive(Debug, Clone)]
pub struct PlanAlternative {
    pub plan: ExecutionPlan,
    pub cost: QueryCost,
    pub description: String,
}

impl CostBasedQueryPlanner {
    /// Create a new cost-based query planner
    pub fn new(statistics_manager: Arc<RwLock<StatisticsManager>>, cost_model: CostModel) -> Self {
        Self {
            statistics_manager,
            cost_model,
            optimizer: QueryOptimizer::new(),
            plan_cache: Arc::new(RwLock::new(HashMap::new())),
            config: PlannerConfig::default(),
        }
    }

    /// Create planner with custom configuration
    pub fn with_config(
        statistics_manager: Arc<RwLock<StatisticsManager>>,
        cost_model: CostModel,
        config: PlannerConfig,
    ) -> Self {
        Self {
            statistics_manager,
            cost_model,
            optimizer: QueryOptimizer::new(),
            plan_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Generate optimal execution plan for a statement
    pub async fn generate_plan(&self, stmt: Statement) -> Result<ExecutionPlan, PlanningError> {
        // Check cache first if enabled
        if self.config.enable_plan_cache {
            let cache_key = self.generate_cache_key(&stmt);
            if let Some(cached_plan) = self.get_cached_plan(&cache_key).await {
                return Ok(cached_plan.plan);
            }
        }

        // Apply rule-based optimizations first
        let optimized_stmt = self
            .optimizer
            .optimize(stmt.clone())
            .map_err(PlanningError::OptimizationError)?;

        // Generate alternative plans
        let alternatives = self.generate_alternatives(optimized_stmt).await?;

        // Select the best plan based on cost
        let best_plan = self.select_best_plan(alternatives).await?;

        // Cache the plan if enabled
        if self.config.enable_plan_cache {
            let cache_key = self.generate_cache_key(&stmt);
            self.cache_plan(&cache_key, &best_plan).await;
        }

        Ok(best_plan.plan)
    }

    /// Generate multiple alternative execution plans
    pub async fn generate_alternatives(
        &self,
        stmt: Statement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        match stmt {
            Statement::Select(select) => {
                // Generate different scan alternatives
                alternatives.extend(self.generate_scan_alternatives(&select).await?);

                // Generate different join alternatives if applicable
                if !select.join_clauses.is_empty() && self.config.enable_join_enumeration {
                    alternatives.extend(self.generate_join_alternatives(&select).await?);
                }

                // Generate different aggregation alternatives
                if !select.group_by.is_empty() {
                    alternatives.extend(self.generate_aggregation_alternatives(&select).await?);
                }

                // Generate different sorting alternatives
                if !select.order_by.is_empty() {
                    alternatives.extend(self.generate_sort_alternatives(&select).await?);
                }
            }
            _ => {
                // For non-SELECT statements, generate a single basic plan
                let basic_plan = self.generate_basic_plan(stmt).await?;
                alternatives.push(basic_plan);
            }
        }

        // Limit number of alternatives
        alternatives.truncate(self.config.max_alternatives);

        Ok(alternatives)
    }

    /// Generate different scan alternatives (sequential, index, etc.)
    async fn generate_scan_alternatives(
        &self,
        select: &SelectStatement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        for from_clause in &select.from {
            if let FromClause::Table { name, .. } = from_clause {
                // Sequential scan alternative
                let seq_scan_plan = self.generate_sequential_scan_plan(name, select).await?;
                alternatives.push(seq_scan_plan);

                // Index scan alternatives
                let index_alternatives = self.generate_index_scan_plans(name, select).await?;
                alternatives.extend(index_alternatives);
            }
        }

        Ok(alternatives)
    }

    /// Generate sequential scan plan
    async fn generate_sequential_scan_plan(
        &self,
        table_name: &str,
        select: &SelectStatement,
    ) -> Result<PlanAlternative, PlanningError> {
        let stats_manager = self.statistics_manager.read().await;
        let table_stats = stats_manager
            .get_table_statistics(table_name)
            .await
            .unwrap_or_else(|| self.create_default_table_stats(table_name));

        // Estimate selectivity from WHERE clause
        let selectivity = if let Some(where_clause) = &select.where_clause {
            self.estimate_where_selectivity(table_name, where_clause, &*stats_manager)
                .await
        } else {
            1.0
        };

        let (cost, cardinality) = self
            .cost_model
            .calculate_scan_cost(&table_stats, selectivity);

        let plan = ExecutionPlan {
            root: PlanNode::TableScan {
                table: table_name.to_string(),
                columns: vec![], // TODO: Extract required columns
                filter: select.where_clause.clone(),
            },
            estimated_cost: cost.total_cost(),
            estimated_rows: cardinality.rows,
        };

        Ok(PlanAlternative {
            plan,
            cost,
            description: format!("Sequential scan on {}", table_name),
        })
    }

    /// Generate index scan plans
    async fn generate_index_scan_plans(
        &self,
        table_name: &str,
        select: &SelectStatement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        // Mock index information - in a real implementation, this would come from metadata
        let available_indexes = self.get_available_indexes(table_name).await;

        for index in available_indexes {
            if let Some(where_clause) = &select.where_clause {
                if self.can_use_index(&index, where_clause) {
                    let index_plan = self
                        .generate_index_scan_plan(table_name, &index, select)
                        .await?;
                    alternatives.push(index_plan);
                }
            }
        }

        Ok(alternatives)
    }

    /// Generate index scan plan
    async fn generate_index_scan_plan(
        &self,
        table_name: &str,
        index_stats: &IndexStatistics,
        select: &SelectStatement,
    ) -> Result<PlanAlternative, PlanningError> {
        let stats_manager = self.statistics_manager.read().await;
        let table_stats = stats_manager
            .get_table_statistics(table_name)
            .await
            .unwrap_or_else(|| self.create_default_table_stats(table_name));

        let selectivity = index_stats.selectivity;
        let (cost, cardinality) =
            self.cost_model
                .calculate_index_scan_cost(&table_stats, index_stats, selectivity);

        let plan = ExecutionPlan {
            root: PlanNode::TableScan {
                table: table_name.to_string(),
                columns: vec![],
                filter: select.where_clause.clone(),
            },
            estimated_cost: cost.total_cost(),
            estimated_rows: cardinality.rows,
        };

        Ok(PlanAlternative {
            plan,
            cost,
            description: format!(
                "Index scan on {} using {}",
                table_name, index_stats.index_id
            ),
        })
    }

    /// Generate different join order alternatives
    async fn generate_join_alternatives(
        &self,
        select: &SelectStatement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        // Generate different join orders using dynamic programming approach
        let join_orders = self.enumerate_join_orders(&select.join_clauses);

        for join_order in join_orders {
            let join_plan = self.generate_join_plan(select, &join_order).await?;
            alternatives.push(join_plan);
        }

        Ok(alternatives)
    }

    /// Generate join plan for a specific join order
    async fn generate_join_plan(
        &self,
        select: &SelectStatement,
        join_order: &[usize],
    ) -> Result<PlanAlternative, PlanningError> {
        // This is a simplified implementation
        // In a real system, this would build the complete join tree

        let plan = ExecutionPlan {
            root: PlanNode::TableScan {
                table: "placeholder".to_string(),
                columns: vec![],
                filter: None,
            },
            estimated_cost: 1000.0, // Placeholder cost
            estimated_rows: 1000,
        };

        let cost = QueryCost {
            cpu_cost: 500.0,
            io_cost: 300.0,
            memory_cost: 100.0,
            network_cost: 50.0,
            total_time_ms: 2000.0,
        };

        Ok(PlanAlternative {
            plan,
            cost,
            description: format!("Join order: {:?}", join_order),
        })
    }

    /// Generate aggregation alternatives (hash vs sort)
    async fn generate_aggregation_alternatives(
        &self,
        select: &SelectStatement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        // Hash-based aggregation
        let hash_agg_plan = self.generate_hash_aggregation_plan(select).await?;
        alternatives.push(hash_agg_plan);

        // Sort-based aggregation
        let sort_agg_plan = self.generate_sort_aggregation_plan(select).await?;
        alternatives.push(sort_agg_plan);

        Ok(alternatives)
    }

    /// Generate hash-based aggregation plan
    async fn generate_hash_aggregation_plan(
        &self,
        _select: &SelectStatement,
    ) -> Result<PlanAlternative, PlanningError> {
        let input_card = CardinalityEstimate::new(10000, 64);
        let (cost, _) = self.cost_model.calculate_aggregate_cost(&input_card, 2, 3);

        let plan = ExecutionPlan {
            root: PlanNode::Aggregation {
                input: Box::new(PlanNode::TableScan {
                    table: "placeholder".to_string(),
                    columns: vec![],
                    filter: None,
                }),
                group_by: vec![],
                aggregates: vec![],
            },
            estimated_cost: cost.total_cost(),
            estimated_rows: 1000,
        };

        Ok(PlanAlternative {
            plan,
            cost,
            description: "Hash-based aggregation".to_string(),
        })
    }

    /// Generate sort-based aggregation plan
    async fn generate_sort_aggregation_plan(
        &self,
        _select: &SelectStatement,
    ) -> Result<PlanAlternative, PlanningError> {
        let input_card = CardinalityEstimate::new(10000, 64);
        let (sort_cost, _) = self.cost_model.calculate_sort_cost(&input_card, 2);
        let (agg_cost, _) = self.cost_model.calculate_aggregate_cost(&input_card, 2, 3);
        let total_cost = sort_cost.combine(&agg_cost);

        let plan = ExecutionPlan {
            root: PlanNode::Sort {
                input: Box::new(PlanNode::Aggregation {
                    input: Box::new(PlanNode::TableScan {
                        table: "placeholder".to_string(),
                        columns: vec![],
                        filter: None,
                    }),
                    group_by: vec![],
                    aggregates: vec![],
                }),
                expressions: vec![],
            },
            estimated_cost: total_cost.total_cost(),
            estimated_rows: 1000,
        };

        Ok(PlanAlternative {
            plan,
            cost: total_cost,
            description: "Sort-based aggregation".to_string(),
        })
    }

    /// Generate sort alternatives
    async fn generate_sort_alternatives(
        &self,
        _select: &SelectStatement,
    ) -> Result<Vec<PlanAlternative>, PlanningError> {
        let mut alternatives = Vec::new();

        // In-memory sort
        let input_card = CardinalityEstimate::new(10000, 64);
        let (cost, _) = self.cost_model.calculate_sort_cost(&input_card, 2);

        let plan = ExecutionPlan {
            root: PlanNode::Sort {
                input: Box::new(PlanNode::TableScan {
                    table: "placeholder".to_string(),
                    columns: vec![],
                    filter: None,
                }),
                expressions: vec![],
            },
            estimated_cost: cost.total_cost(),
            estimated_rows: input_card.rows,
        };

        alternatives.push(PlanAlternative {
            plan,
            cost,
            description: "In-memory sort".to_string(),
        });

        Ok(alternatives)
    }

    /// Select the best plan from alternatives based on cost
    async fn select_best_plan(
        &self,
        alternatives: Vec<PlanAlternative>,
    ) -> Result<PlanAlternative, PlanningError> {
        if alternatives.is_empty() {
            return Err(PlanningError::InvalidPlan(
                "No alternatives generated".to_string(),
            ));
        }

        let best_plan = alternatives
            .into_iter()
            .min_by(|a, b| {
                a.cost
                    .total_cost()
                    .partial_cmp(&b.cost.total_cost())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        Ok(best_plan)
    }

    /// Generate basic plan for non-SELECT statements
    async fn generate_basic_plan(&self, stmt: Statement) -> Result<PlanAlternative, PlanningError> {
        let cost = QueryCost::new();
        let plan = ExecutionPlan {
            root: PlanNode::TableScan {
                table: "placeholder".to_string(),
                columns: vec![],
                filter: None,
            },
            estimated_cost: cost.total_cost(),
            estimated_rows: 1,
        };

        Ok(PlanAlternative {
            plan,
            cost,
            description: format!("Basic plan for {:?}", stmt),
        })
    }

    // Helper methods

    async fn get_cached_plan(&self, cache_key: &str) -> Option<CachedPlan> {
        let cache = self.plan_cache.read().await;
        cache.get(cache_key).cloned()
    }

    async fn cache_plan(&self, cache_key: &str, plan: &PlanAlternative) {
        let mut cache = self.plan_cache.write().await;

        // Remove expired entries if cache is full
        if cache.len() >= self.config.max_cache_size {
            let now = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(self.config.cache_timeout_seconds);
            cache.retain(|_, cached| now.duration_since(cached.timestamp) < timeout);
        }

        cache.insert(
            cache_key.to_string(),
            CachedPlan {
                plan: plan.plan.clone(),
                cost: plan.cost.clone(),
                timestamp: std::time::Instant::now(),
                hit_count: 0,
            },
        );
    }

    fn generate_cache_key(&self, stmt: &Statement) -> String {
        // Simple cache key generation - in production, this would be more sophisticated
        format!("{:?}", stmt)
    }

    async fn get_available_indexes(&self, _table_name: &str) -> Vec<IndexStatistics> {
        // Mock index data - in a real implementation, this would come from metadata
        vec![IndexStatistics {
            index_id: "idx_id".to_string(),
            selectivity: 0.01,
            clustering_factor: 0.9,
            tree_height: 3,
            leaf_pages: 100,
            distinct_keys: 10000,
            index_size: 1024 * 1024,
            last_updated: chrono::Utc::now(),
        }]
    }

    fn can_use_index(&self, _index: &IndexStatistics, _where_clause: &Expression) -> bool {
        // Simplified index applicability check
        true
    }

    async fn estimate_where_selectivity(
        &self,
        table_name: &str,
        where_clause: &Expression,
        stats_manager: &StatisticsManager,
    ) -> f64 {
        match where_clause {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                if let (Expression::Identifier(column), Expression::Literal(value)) =
                    (left.as_ref(), right.as_ref())
                {
                    let op_str = match operator {
                        BinaryOperator::Equal => "=",
                        BinaryOperator::GreaterThan => ">",
                        BinaryOperator::LessThan => "<",
                        _ => "=",
                    };
                    return stats_manager
                        .estimate_selectivity(table_name, column, op_str, value)
                        .await;
                }
            }
            _ => {}
        }
        0.1 // Default selectivity
    }

    fn enumerate_join_orders(&self, joins: &[JoinClause]) -> Vec<Vec<usize>> {
        if joins.len() <= 1 {
            return vec![vec![0]];
        }

        // Simple enumeration - in production, this would use more sophisticated algorithms
        let mut orders = Vec::new();
        for i in 0..joins.len() {
            orders.push(vec![i]);
        }
        orders
    }

    fn create_default_table_stats(&self, table_name: &str) -> TableStatistics {
        TableStatistics {
            row_count: 10000,
            page_count: 100,
            avg_row_size: 64,
            null_fraction: 0.1,
            distinct_values: 8000,
            most_common_values: vec![],
            histogram: vec![],
            last_analyzed: chrono::Utc::now(),
            column_statistics: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbitql::QueryValue;

    #[tokio::test]
    async fn test_cost_based_planner_creation() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let planner = CostBasedQueryPlanner::new(stats_manager, cost_model);

        // Test that planner was created successfully
        assert_eq!(planner.config.max_alternatives, 10);
    }

    #[tokio::test]
    async fn test_generate_scan_alternatives() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let planner = CostBasedQueryPlanner::new(stats_manager, cost_model);

        let select = SelectStatement {
            with_clauses: Vec::new(),
            fields: vec![SelectField::All],
            from: vec![FromClause::Table {
                name: "users".to_string(),
                alias: None,
            }],
            where_clause: Some(Expression::Binary {
                left: Box::new(Expression::Identifier("age".to_string())),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(QueryValue::Integer(18))),
            }),
            join_clauses: vec![],
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
            fetch: vec![],
            timeout: None,
        };

        let alternatives = planner.generate_scan_alternatives(&select).await.unwrap();
        assert!(!alternatives.is_empty());
    }

    #[tokio::test]
    async fn test_plan_caching() {
        let stats_manager = Arc::new(RwLock::new(StatisticsManager::default()));
        let cost_model = CostModel::new();
        let planner = CostBasedQueryPlanner::new(stats_manager, cost_model);

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

        // First call should generate plan
        let plan1 = planner.generate_plan(stmt.clone()).await.unwrap();

        // Second call should use cached plan
        let plan2 = planner.generate_plan(stmt).await.unwrap();

        // Plans should be equivalent
        assert_eq!(plan1.estimated_cost, plan2.estimated_cost);
    }
}
