//! Query optimization
//!
//! Query optimizer for improving execution performance through various optimization passes:
//!
//! - **Filter Pushdown**: Move filters as close to data source as possible
//! - **Projection Pushdown**: Select only required columns early
//! - **Predicate Simplification**: Simplify and eliminate redundant predicates
//! - **Cost-Based Selection**: Choose optimal scan strategy (index vs table scan)
//! - **SIMD Enablement**: Identify operations that benefit from vectorization

use crate::query::{ExecutionPlan, PlanNode, PlanNodeType, Query};
use crate::error::EngineResult;
use crate::storage::FilterPredicate;

/// Query optimizer
pub struct QueryOptimizer {
    /// Optimization level (0-3)
    /// - 0: No optimization
    /// - 1: Basic optimizations (filter pushdown, projection)
    /// - 2: Level 1 + predicate simplification
    /// - 3: Level 2 + cost-based optimization
    pub optimization_level: u8,
}

impl QueryOptimizer {
    /// Create a new optimizer
    pub fn new(optimization_level: u8) -> Self {
        Self {
            optimization_level: optimization_level.min(3),
        }
    }

    /// Optimize a query and produce an execution plan
    ///
    /// Applies optimization passes based on the configured optimization level.
    pub fn optimize(&self, query: &Query) -> EngineResult<ExecutionPlan> {
        let mut plan = self.build_initial_plan(query);

        if self.optimization_level == 0 {
            return Ok(plan);
        }

        // Level 1: Basic optimizations
        if self.optimization_level >= 1 {
            plan = self.apply_filter_pushdown(plan, query);
            plan = self.apply_projection_pushdown(plan, query);
        }

        // Level 2: Predicate optimizations
        if self.optimization_level >= 2 {
            plan = self.simplify_predicates(plan, query);
        }

        // Level 3: Cost-based optimizations
        if self.optimization_level >= 3 {
            plan = self.apply_cost_based_optimization(plan, query);
        }

        // Always check for SIMD opportunities
        plan.uses_simd = self.can_use_simd(query);

        Ok(plan)
    }

    /// Build initial execution plan before optimizations
    fn build_initial_plan(&self, query: &Query) -> ExecutionPlan {
        let mut nodes = Vec::new();
        let mut estimated_rows = 1000; // Default estimate

        // 1. Table scan node
        nodes.push(PlanNode {
            node_type: PlanNodeType::TableScan,
            estimated_rows,
            children: vec![],
        });

        // 2. Filter node (if present)
        if query.filter.is_some() {
            estimated_rows = (estimated_rows as f64 * 0.3) as usize; // Assume 30% selectivity
            nodes.push(PlanNode {
                node_type: PlanNodeType::Filter,
                estimated_rows,
                children: vec![],
            });
        }

        // 3. Projection node (if present)
        if let Some(ref projection) = query.projection {
            let selectivity = projection.len() as f64 / 10.0; // Assume 10 columns total
            estimated_rows = (estimated_rows as f64 * selectivity) as usize;
            nodes.push(PlanNode {
                node_type: PlanNodeType::Projection,
                estimated_rows,
                children: vec![],
            });
        }

        let estimated_cost = self.estimate_cost(&nodes);

        ExecutionPlan {
            nodes,
            estimated_cost,
            uses_simd: false,
        }
    }

    /// Apply filter pushdown optimization
    ///
    /// Move filters as close to the data source as possible to reduce
    /// the amount of data processed by subsequent operations.
    fn apply_filter_pushdown(&self, mut plan: ExecutionPlan, _query: &Query) -> ExecutionPlan {
        // Reorder nodes to push filters down
        let has_filter = plan.nodes.iter().any(|n| n.node_type == PlanNodeType::Filter);
        let has_scan = plan.nodes.iter().any(|n| n.node_type == PlanNodeType::TableScan);

        if has_filter && has_scan {
            // Ensure filter comes immediately after scan
            plan.nodes.sort_by_key(|node| match node.node_type {
                PlanNodeType::TableScan => 0,
                PlanNodeType::Filter => 1,
                PlanNodeType::Projection => 2,
                _ => 3,
            });
        }

        plan.estimated_cost = self.estimate_cost(&plan.nodes);
        plan
    }

    /// Apply projection pushdown optimization
    ///
    /// Select only required columns as early as possible to reduce
    /// memory bandwidth and processing overhead.
    fn apply_projection_pushdown(&self, mut plan: ExecutionPlan, _query: &Query) -> ExecutionPlan {
        // Projection should come early but after filter
        plan.nodes.sort_by_key(|node| match node.node_type {
            PlanNodeType::TableScan => 0,
            PlanNodeType::Filter => 1,
            PlanNodeType::Projection => 2,
            PlanNodeType::Aggregation => 3,
            _ => 4,
        });

        plan.estimated_cost = self.estimate_cost(&plan.nodes);
        plan
    }

    /// Simplify predicates
    ///
    /// Eliminate redundant predicates and simplify complex expressions.
    fn simplify_predicates(&self, mut plan: ExecutionPlan, query: &Query) -> ExecutionPlan {
        if let Some(ref filter) = query.filter {
            // Check for always-true or always-false predicates
            if self.is_trivial_predicate(filter) {
                // Remove filter node if predicate is always true
                plan.nodes.retain(|n| n.node_type != PlanNodeType::Filter);
            }
        }

        plan.estimated_cost = self.estimate_cost(&plan.nodes);
        plan
    }

    /// Apply cost-based optimization
    ///
    /// Choose between table scan and index scan based on estimated cost.
    fn apply_cost_based_optimization(&self, mut plan: ExecutionPlan, query: &Query) -> ExecutionPlan {
        // If we have a highly selective filter, consider using index scan
        if let Some(ref filter) = query.filter {
            let selectivity = self.estimate_selectivity(filter);

            // Use index scan if selectivity < 10%
            if selectivity < 0.1 {
                for node in &mut plan.nodes {
                    if node.node_type == PlanNodeType::TableScan {
                        node.node_type = PlanNodeType::IndexScan;
                        // Index scan reduces estimated rows significantly
                        node.estimated_rows = (node.estimated_rows as f64 * selectivity) as usize;
                    }
                }
            }
        }

        plan.estimated_cost = self.estimate_cost(&plan.nodes);
        plan
    }

    /// Check if SIMD can be used for this query
    fn can_use_simd(&self, query: &Query) -> bool {
        // SIMD benefits:
        // 1. Numeric filters (equality, comparison)
        // 2. Aggregations (sum, count, min, max, avg)
        // 3. Large datasets (>100 rows)

        if let Some(ref filter) = query.filter {
            self.filter_uses_numeric_comparison(filter)
        } else {
            // Even without filter, projection of numeric columns benefits from SIMD
            true
        }
    }

    /// Check if filter uses numeric comparison
    fn filter_uses_numeric_comparison(&self, filter: &FilterPredicate) -> bool {
        match filter {
            FilterPredicate::Eq(_, val) |
            FilterPredicate::Ne(_, val) |
            FilterPredicate::Lt(_, val) |
            FilterPredicate::Le(_, val) |
            FilterPredicate::Gt(_, val) |
            FilterPredicate::Ge(_, val) => {
                // Check if value is numeric
                matches!(
                    val,
                    crate::storage::SqlValue::Int16(_)
                        | crate::storage::SqlValue::Int32(_)
                        | crate::storage::SqlValue::Int64(_)
                        | crate::storage::SqlValue::Float32(_)
                        | crate::storage::SqlValue::Float64(_)
                )
            }
            FilterPredicate::And(filters) | FilterPredicate::Or(filters) => {
                filters.iter().any(|f| self.filter_uses_numeric_comparison(f))
            }
            FilterPredicate::Not(filter) => self.filter_uses_numeric_comparison(filter),
        }
    }

    /// Check if predicate is trivial (always true/false)
    fn is_trivial_predicate(&self, _filter: &FilterPredicate) -> bool {
        // Placeholder - could implement constant folding
        // e.g., "1 = 1" -> true, "1 = 2" -> false
        false
    }

    /// Estimate filter selectivity (fraction of rows that pass filter)
    fn estimate_selectivity(&self, filter: &FilterPredicate) -> f64 {
        match filter {
            // Equality is typically very selective
            FilterPredicate::Eq(_, _) => 0.05,
            // Inequality is less selective
            FilterPredicate::Ne(_, _) => 0.95,
            // Range predicates have medium selectivity
            FilterPredicate::Lt(_, _) |
            FilterPredicate::Le(_, _) |
            FilterPredicate::Gt(_, _) |
            FilterPredicate::Ge(_, _) => 0.33,
            // AND reduces selectivity (multiplicative)
            FilterPredicate::And(filters) => {
                filters.iter()
                    .map(|f| self.estimate_selectivity(f))
                    .product()
            }
            // OR increases selectivity (probabilistic union)
            FilterPredicate::Or(filters) => {
                let combined = filters.iter()
                    .map(|f| 1.0 - self.estimate_selectivity(f))
                    .product::<f64>();
                1.0 - combined
            }
            // NOT inverts selectivity
            FilterPredicate::Not(filter) => {
                1.0 - self.estimate_selectivity(filter)
            }
        }
    }

    /// Estimate execution cost based on plan nodes
    fn estimate_cost(&self, nodes: &[PlanNode]) -> f64 {
        let mut cost = 0.0;

        for node in nodes {
            cost += match node.node_type {
                // Table scan is expensive (full table read)
                PlanNodeType::TableScan => node.estimated_rows as f64 * 1.0,
                // Index scan is cheaper (targeted read)
                PlanNodeType::IndexScan => node.estimated_rows as f64 * 0.1,
                // Filter has moderate cost
                PlanNodeType::Filter => node.estimated_rows as f64 * 0.5,
                // Projection is cheap (just column selection)
                PlanNodeType::Projection => node.estimated_rows as f64 * 0.1,
                // Aggregation requires full pass
                PlanNodeType::Aggregation => node.estimated_rows as f64 * 0.8,
                // Join is very expensive
                PlanNodeType::Join => node.estimated_rows as f64 * 2.0,
                // Sort is expensive
                PlanNodeType::Sort => node.estimated_rows as f64 * 1.5,
            };
        }

        cost
    }
}
