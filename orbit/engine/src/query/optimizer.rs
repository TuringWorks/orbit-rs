//! Query optimization
//!
//! Query optimizer for improving execution performance through various optimization passes:
//!
//! - **Filter Pushdown**: Move filters as close to data source as possible
//! - **Projection Pushdown**: Select only required columns early
//! - **Predicate Simplification**: Simplify and eliminate redundant predicates
//! - **Cost-Based Selection**: Choose optimal scan strategy (index vs table scan)
//! - **Hardware Acceleration**: Analyze and recommend optimal acceleration strategy
//! - **SIMD Enablement**: Identify operations that benefit from vectorization

use crate::error::EngineResult;
use crate::query::{ExecutionPlan, PlanNode, PlanNodeType, Query};
use crate::storage::FilterPredicate;
use orbit_compute::{OperationType, QueryAnalyzer, UniversalComputeCapabilities};

/// Query optimizer with hardware acceleration awareness
pub struct QueryOptimizer {
    /// Optimization level (0-3)
    /// - 0: No optimization
    /// - 1: Basic optimizations (filter pushdown, projection)
    /// - 2: Level 1 + predicate simplification
    /// - 3: Level 2 + cost-based optimization + hardware acceleration
    pub optimization_level: u8,
    /// Query analyzer for hardware acceleration recommendations
    query_analyzer: Option<QueryAnalyzer>,
}

impl QueryOptimizer {
    /// Create a new optimizer without hardware acceleration
    pub fn new(optimization_level: u8) -> Self {
        Self {
            optimization_level: optimization_level.min(3),
            query_analyzer: None,
        }
    }

    /// Create a new optimizer with hardware acceleration support
    ///
    /// Detects available compute capabilities and creates a QueryAnalyzer
    /// for intelligent acceleration recommendations.
    pub async fn new_with_acceleration(optimization_level: u8) -> EngineResult<Self> {
        let capabilities = orbit_compute::init_heterogeneous_compute()
            .await
            .map_err(|e| {
                crate::error::EngineError::Internal(format!(
                    "Failed to detect compute capabilities: {}",
                    e
                ))
            })?;

        Ok(Self {
            optimization_level: optimization_level.min(3),
            query_analyzer: Some(QueryAnalyzer::new(capabilities)),
        })
    }

    /// Create optimizer with explicit capabilities
    pub fn new_with_capabilities(
        optimization_level: u8,
        capabilities: UniversalComputeCapabilities,
    ) -> Self {
        Self {
            optimization_level: optimization_level.min(3),
            query_analyzer: Some(QueryAnalyzer::new(capabilities)),
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

        // Level 3: Cost-based optimizations + hardware acceleration
        if self.optimization_level >= 3 {
            plan = self.apply_cost_based_optimization(plan, query);

            // Analyze query for hardware acceleration opportunities
            if let Some(ref analyzer) = self.query_analyzer {
                let operations = self.query_to_operations(query, &plan);
                match analyzer.analyze_operations(&operations) {
                    Ok(analysis) => {
                        let old_cost = plan.estimated_cost;

                        tracing::debug!(
                            "Query analysis: complexity={:.2}, strategy={:?}, confidence={:.2}",
                            analysis.complexity_score,
                            analysis.recommended_strategy,
                            analysis.confidence
                        );

                        plan.acceleration_strategy = Some(analysis.recommended_strategy);
                        plan.query_analysis = Some(analysis);

                        // Recalculate cost with acceleration strategy
                        plan.estimated_cost = self
                            .estimate_cost_with_strategy(&plan.nodes, plan.acceleration_strategy);

                        tracing::debug!(
                            "Cost estimate updated: {:.2} -> {:.2} ({}x speedup)",
                            old_cost,
                            plan.estimated_cost,
                            old_cost / plan.estimated_cost.max(0.001)
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to analyze query for acceleration: {}", e);
                    }
                }
            }
        }

        // Always check for SIMD opportunities (deprecated path)
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
            acceleration_strategy: None,
            query_analysis: None,
        }
    }

    /// Apply filter pushdown optimization
    ///
    /// Move filters as close to the data source as possible to reduce
    /// the amount of data processed by subsequent operations.
    fn apply_filter_pushdown(&self, mut plan: ExecutionPlan, _query: &Query) -> ExecutionPlan {
        // Reorder nodes to push filters down
        let has_filter = plan
            .nodes
            .iter()
            .any(|n| n.node_type == PlanNodeType::Filter);
        let has_scan = plan
            .nodes
            .iter()
            .any(|n| n.node_type == PlanNodeType::TableScan);

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
    fn apply_cost_based_optimization(
        &self,
        mut plan: ExecutionPlan,
        query: &Query,
    ) -> ExecutionPlan {
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
            FilterPredicate::Eq(_, val)
            | FilterPredicate::Ne(_, val)
            | FilterPredicate::Lt(_, val)
            | FilterPredicate::Le(_, val)
            | FilterPredicate::Gt(_, val)
            | FilterPredicate::Ge(_, val) => {
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
            FilterPredicate::And(filters) | FilterPredicate::Or(filters) => filters
                .iter()
                .any(|f| self.filter_uses_numeric_comparison(f)),
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
            FilterPredicate::Lt(_, _)
            | FilterPredicate::Le(_, _)
            | FilterPredicate::Gt(_, _)
            | FilterPredicate::Ge(_, _) => 0.33,
            // AND reduces selectivity (multiplicative)
            FilterPredicate::And(filters) => filters
                .iter()
                .map(|f| self.estimate_selectivity(f))
                .product(),
            // OR increases selectivity (probabilistic union)
            FilterPredicate::Or(filters) => {
                let combined = filters
                    .iter()
                    .map(|f| 1.0 - self.estimate_selectivity(f))
                    .product::<f64>();
                1.0 - combined
            }
            // NOT inverts selectivity
            FilterPredicate::Not(filter) => 1.0 - self.estimate_selectivity(filter),
        }
    }

    /// Estimate execution cost based on plan nodes (without acceleration)
    fn estimate_cost(&self, nodes: &[PlanNode]) -> f64 {
        self.estimate_cost_with_strategy(nodes, None)
    }

    /// Estimate execution cost with acceleration strategy
    ///
    /// Adjusts cost estimates based on the acceleration strategy:
    /// - CPU SIMD: 2-3x speedup for numeric operations
    /// - GPU: 5-10x speedup for large datasets with parallel operations
    /// - Neural Engine: 3-5x speedup for ML-like operations
    /// - Hybrid: 4-8x speedup combining multiple accelerators
    fn estimate_cost_with_strategy(
        &self,
        nodes: &[PlanNode],
        strategy: Option<orbit_compute::AccelerationStrategy>,
    ) -> f64 {
        let mut base_cost = 0.0;

        // Calculate base cost for each operation
        for node in nodes {
            base_cost += match node.node_type {
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
                // GroupBy requires grouping + aggregation
                PlanNodeType::GroupBy => node.estimated_rows as f64 * 1.2,
                // Join is very expensive
                PlanNodeType::Join => node.estimated_rows as f64 * 2.0,
                // Sort is expensive
                PlanNodeType::Sort => node.estimated_rows as f64 * 1.5,
            };
        }

        // Apply acceleration multiplier
        let acceleration_factor = match strategy {
            Some(orbit_compute::AccelerationStrategy::CpuSimd) => {
                // CPU SIMD provides 2-3x speedup for numeric operations
                0.4 // 40% of base cost (2.5x speedup)
            }
            Some(orbit_compute::AccelerationStrategy::Gpu) => {
                // GPU provides 5-10x speedup for large parallel workloads
                0.15 // 15% of base cost (6.7x speedup)
            }
            Some(orbit_compute::AccelerationStrategy::NeuralEngine) => {
                // Neural Engine provides 3-5x speedup for tensor operations
                0.25 // 25% of base cost (4x speedup)
            }
            Some(orbit_compute::AccelerationStrategy::Hybrid) => {
                // Hybrid combines multiple accelerators for optimal performance
                0.2 // 20% of base cost (5x speedup)
            }
            Some(orbit_compute::AccelerationStrategy::None) | None => {
                // No acceleration
                1.0 // 100% of base cost
            }
        };

        base_cost * acceleration_factor
    }

    /// Convert Query and ExecutionPlan to OperationType vector for QueryAnalyzer
    ///
    /// Maps database query operations to acceleration-aware operation types.
    fn query_to_operations(&self, query: &Query, plan: &ExecutionPlan) -> Vec<OperationType> {
        let mut operations = Vec::new();

        // Always start with scan
        operations.push(OperationType::Scan);

        // Add operations based on plan nodes
        for node in &plan.nodes {
            match node.node_type {
                PlanNodeType::Filter => operations.push(OperationType::Filter),
                PlanNodeType::Projection => operations.push(OperationType::Project),
                PlanNodeType::Aggregation => operations.push(OperationType::Aggregate),
                PlanNodeType::GroupBy => operations.push(OperationType::GroupBy),
                PlanNodeType::Sort => operations.push(OperationType::Sort),
                PlanNodeType::Join => operations.push(OperationType::Join),
                _ => {} // TableScan and IndexScan already covered by initial Scan
            }
        }

        // Add limit if present
        if query.limit.is_some() {
            operations.push(OperationType::Limit);
        }

        operations
    }
}
