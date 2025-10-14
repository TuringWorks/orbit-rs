//! Distributed query execution engine for OrbitQL
//!
//! This module provides distributed query execution capabilities that can
//! parallelize query processing across multiple actor nodes with intelligent
//! data locality optimization.

use crate::orbitql::ast::AggregateFunction;
use crate::orbitql::executor::QueryResult;
use crate::orbitql::planner::{ExecutionPlan, PlanNode, QueryPlanner};
// Temporarily disable ActorId since it's not defined in this crate yet
// use crate::ActorId;
type ActorId = String; // Placeholder

// Add placeholder types for OrbitResult and OrbitError
type OrbitResult<T> = Result<T, OrbitError>;

#[derive(Debug, thiserror::Error)]
pub enum OrbitError {
    #[error("Execution error: {0}")]
    Execution(String),
}

impl OrbitError {
    pub fn execution(msg: String) -> Self {
        OrbitError::Execution(msg)
    }
}
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info};
use uuid::Uuid;

/// Distributed execution context
#[derive(Debug, Clone)]
pub struct DistributedExecutionContext {
    /// Unique execution ID
    pub execution_id: Uuid,
    /// Actor nodes available for execution
    pub available_nodes: Vec<ActorNodeInfo>,
    /// Data locality hints for optimization
    pub data_locality: HashMap<String, Vec<ActorId>>,
    /// Resource constraints
    pub resource_limits: ResourceLimits,
    /// Execution timeout
    pub timeout: Duration,
}

/// Information about an actor node for distributed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorNodeInfo {
    pub actor_id: ActorId,
    pub node_type: NodeType,
    pub capabilities: NodeCapabilities,
    pub current_load: f64, // 0.0 to 1.0
    pub average_latency_ms: f64,
    pub data_affinity: Vec<String>, // Data collections this node has affinity for
}

/// Types of nodes in the distributed system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Document storage and processing node
    Document,
    /// Graph processing node
    Graph,
    /// Time-series processing node
    TimeSeries,
    /// General compute node
    Compute,
    /// Coordinator node
    Coordinator,
}

/// Capabilities of a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub can_process_documents: bool,
    pub can_process_graphs: bool,
    pub can_process_timeseries: bool,
    pub can_join_data: bool,
    pub can_aggregate: bool,
    pub max_parallel_tasks: u32,
    pub memory_gb: f64,
    pub cpu_cores: u32,
}

/// Resource limits for distributed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_nodes: u32,
    pub max_parallel_tasks: u32,
    pub max_memory_gb: f64,
    pub max_network_bandwidth_mbps: f64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_nodes: 10,
            max_parallel_tasks: 100,
            max_memory_gb: 32.0,
            max_network_bandwidth_mbps: 1000.0,
        }
    }
}

/// Distributed execution plan node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistributedPlanNode {
    /// Execute on local coordinator
    Local(PlanNode),
    /// Execute on a remote actor node
    Remote {
        target_actor: ActorId,
        plan: PlanNode,
        data_transfer: Option<DataTransferPlan>,
    },
    /// Parallel execution across multiple nodes
    Parallel {
        sub_plans: Vec<DistributedPlanNode>,
        merge_strategy: MergeStrategy,
    },
    /// Data exchange between nodes
    Exchange {
        source_actor: ActorId,
        target_actor: ActorId,
        exchange_type: ExchangeType,
        partitioning: PartitioningStrategy,
    },
}

/// Strategy for merging parallel results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Simple concatenation of results
    Concat,
    /// Merge sorted results
    MergeSorted { sort_keys: Vec<String> },
    /// Union with deduplication
    Union,
    /// Join results
    Join { join_keys: Vec<String> },
    /// Aggregate results
    Aggregate {
        group_by: Vec<String>,
        functions: Vec<AggregateFunction>,
    },
}

/// Data exchange types between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExchangeType {
    /// Broadcast data to all nodes
    Broadcast,
    /// Redistribute data based on partitioning
    Redistribute,
    /// Gather data from multiple nodes to one
    Gather,
    /// Point-to-point data transfer
    PointToPoint,
}

/// Partitioning strategies for data distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitioningStrategy {
    /// Hash-based partitioning
    Hash {
        columns: Vec<String>,
        partitions: u32,
    },
    /// Range-based partitioning
    Range {
        column: String,
        ranges: Vec<PartitionRange>,
    },
    /// Round-robin partitioning
    RoundRobin { partitions: u32 },
    /// Random partitioning
    Random { partitions: u32 },
}

/// Partition range definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRange {
    pub start: String, // Serialized value
    pub end: String,   // Serialized value
}

/// Data transfer plan for moving data between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransferPlan {
    pub estimated_size_mb: f64,
    pub compression: bool,
    pub encryption: bool,
    pub priority: TransferPriority,
}

/// Priority levels for data transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Distributed query planner
pub struct DistributedQueryPlanner {
    #[allow(dead_code)]
    local_planner: QueryPlanner,
    cluster_topology: Arc<RwLock<ClusterTopology>>,
    cost_model: DistributedCostModel,
}

/// Cluster topology information
#[derive(Debug, Clone)]
pub struct ClusterTopology {
    pub nodes: HashMap<ActorId, ActorNodeInfo>,
    pub network_latency: HashMap<(ActorId, ActorId), Duration>,
    pub data_placement: HashMap<String, Vec<ActorId>>, // Collection -> Node mappings
}

impl Default for ClusterTopology {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterTopology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            network_latency: HashMap::new(),
            data_placement: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node_info: ActorNodeInfo) {
        self.nodes.insert(node_info.actor_id.clone(), node_info);
    }
}

/// Cost model for distributed operations
#[derive(Debug, Clone)]
pub struct DistributedCostModel {
    pub network_cost_per_mb: f64,
    pub cpu_cost_per_operation: f64,
    pub memory_cost_per_mb: f64,
    pub latency_penalty: f64,
}

impl Default for DistributedCostModel {
    fn default() -> Self {
        Self {
            network_cost_per_mb: 10.0,
            cpu_cost_per_operation: 1.0,
            memory_cost_per_mb: 0.1,
            latency_penalty: 100.0,
        }
    }
}

impl DistributedQueryPlanner {
    pub fn new(local_planner: QueryPlanner) -> Self {
        Self {
            local_planner,
            cluster_topology: Arc::new(RwLock::new(ClusterTopology {
                nodes: HashMap::new(),
                network_latency: HashMap::new(),
                data_placement: HashMap::new(),
            })),
            cost_model: DistributedCostModel::default(),
        }
    }

    /// Create distributed execution plan from a local execution plan
    pub async fn create_distributed_plan(
        &self,
        local_plan: ExecutionPlan,
        context: DistributedExecutionContext,
    ) -> OrbitResult<DistributedExecutionPlan> {
        let topology = self.cluster_topology.read().await;

        // Analyze the local plan for parallelization opportunities
        let distributed_nodes = self
            .parallelize_plan(local_plan.root, &context, &topology)
            .await?;

        // Optimize data movement and placement
        let optimized_nodes = self
            .optimize_data_placement(distributed_nodes, &topology)
            .await?;

        // Estimate execution cost
        let estimated_cost = self
            .estimate_distributed_cost(&optimized_nodes, &topology)
            .await;

        Ok(DistributedExecutionPlan {
            execution_id: context.execution_id,
            root: optimized_nodes.clone(),
            estimated_cost,
            resource_requirements: self.calculate_resource_requirements(&optimized_nodes).await,
            data_locality_score: self
                .calculate_data_locality_score(&optimized_nodes, &topology)
                .await,
        })
    }

    /// Parallelize a local execution plan
    fn parallelize_plan<'a>(
        &'a self,
        plan_node: PlanNode,
        context: &'a DistributedExecutionContext,
        topology: &'a ClusterTopology,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = OrbitResult<DistributedPlanNode>> + Send + 'a>,
    > {
        Box::pin(async move {
            match plan_node {
                PlanNode::TableScan {
                    table,
                    columns,
                    filter,
                } => {
                    // Find nodes that have this table's data
                    if let Some(data_nodes) = topology.data_placement.get(&table) {
                        if data_nodes.len() > 1 {
                            // Parallelize across multiple nodes
                            let sub_plans: Vec<DistributedPlanNode> = data_nodes
                                .iter()
                                .map(|actor_id| DistributedPlanNode::Remote {
                                    target_actor: actor_id.clone(),
                                    plan: PlanNode::TableScan {
                                        table: table.clone(),
                                        columns: columns.clone(),
                                        filter: filter.clone(),
                                    },
                                    data_transfer: None, // No data transfer needed for table scans
                                })
                                .collect();

                            return Ok(DistributedPlanNode::Parallel {
                                sub_plans,
                                merge_strategy: MergeStrategy::Union,
                            });
                        }
                    }

                    // Fallback to local execution
                    Ok(DistributedPlanNode::Local(PlanNode::TableScan {
                        table,
                        columns,
                        filter,
                    }))
                }

                PlanNode::Join {
                    left,
                    right,
                    join_type,
                    condition,
                } => {
                    // Recursively parallelize left and right sides
                    let left_distributed =
                        Box::new(self.parallelize_plan(*left, context, topology).await?);
                    let right_distributed =
                        Box::new(self.parallelize_plan(*right, context, topology).await?);

                    // Determine best join strategy (broadcast, hash, etc.)
                    let join_strategy = self
                        .choose_join_strategy(&left_distributed, &right_distributed, topology)
                        .await;

                    match join_strategy {
                        JoinStrategy::Broadcast => {
                            // Broadcast smaller side to all nodes processing larger side
                            Ok(DistributedPlanNode::Parallel {
                                sub_plans: vec![
                                    *left_distributed,
                                    DistributedPlanNode::Exchange {
                                        source_actor: "source".to_string(), // TODO: Determine actual source
                                        target_actor: "target".to_string(), // TODO: Determine actual target
                                        exchange_type: ExchangeType::Broadcast,
                                        partitioning: PartitioningStrategy::RoundRobin {
                                            partitions: 1,
                                        },
                                    },
                                    *right_distributed,
                                ],
                                merge_strategy: MergeStrategy::Join { join_keys: vec![] }, // TODO: Extract join keys
                            })
                        }
                        JoinStrategy::Hash => {
                            // Hash partition both sides on join keys
                            Ok(DistributedPlanNode::Parallel {
                                sub_plans: vec![*left_distributed, *right_distributed],
                                merge_strategy: MergeStrategy::Join { join_keys: vec![] }, // TODO: Extract join keys
                            })
                        }
                        JoinStrategy::Local => {
                            // Keep as local join
                            Ok(DistributedPlanNode::Local(PlanNode::Join {
                                left: Box::new(self.extract_local_plan(*left_distributed)?),
                                right: Box::new(self.extract_local_plan(*right_distributed)?),
                                join_type,
                                condition,
                            }))
                        }
                    }
                }

                PlanNode::Aggregation {
                    input,
                    group_by: _,
                    aggregates,
                } => {
                    // Parallelize the input
                    let input_distributed =
                        Box::new(self.parallelize_plan(*input, context, topology).await?);

                    // Two-phase aggregation: local aggregation on each node, then global aggregation
                    Ok(DistributedPlanNode::Parallel {
                        sub_plans: vec![
                            *input_distributed,
                            DistributedPlanNode::Exchange {
                                source_actor: "source".to_string(), // TODO: Determine actual source
                                target_actor: "target".to_string(), // TODO: Determine actual target
                                exchange_type: ExchangeType::Gather,
                                partitioning: PartitioningStrategy::Hash {
                                    columns: vec![], // Fix: group_by contains Expressions, not Strings
                                    partitions: context.available_nodes.len() as u32,
                                },
                            },
                        ],
                        merge_strategy: MergeStrategy::Aggregate {
                            group_by: vec![], // TODO: Convert Vec<Expression> to Vec<String>
                            functions: aggregates.into_iter().map(|agg| agg.function).collect(),
                        },
                    })
                }

                _ => {
                    // For other plan types, keep local for now
                    Ok(DistributedPlanNode::Local(plan_node))
                }
            }
        })
    }

    /// Choose optimal join strategy based on data size and distribution
    async fn choose_join_strategy(
        &self,
        _left: &DistributedPlanNode,
        _right: &DistributedPlanNode,
        _topology: &ClusterTopology,
    ) -> JoinStrategy {
        // TODO: Implement sophisticated join strategy selection
        // Consider factors like:
        // - Estimated cardinality of left and right sides
        // - Data locality and distribution
        // - Network bandwidth
        // - Available memory on nodes

        JoinStrategy::Hash // Default for now
    }

    /// Extract local plan from distributed plan (fallback)
    fn extract_local_plan(&self, distributed_plan: DistributedPlanNode) -> OrbitResult<PlanNode> {
        match distributed_plan {
            DistributedPlanNode::Local(plan) => Ok(plan),
            _ => Err(OrbitError::execution(
                "Cannot extract local plan from distributed node".to_string(),
            )),
        }
    }

    /// Optimize data placement to minimize network traffic
    async fn optimize_data_placement(
        &self,
        plan: DistributedPlanNode,
        _topology: &ClusterTopology,
    ) -> OrbitResult<DistributedPlanNode> {
        // TODO: Implement data placement optimization
        // - Co-locate related operations on the same nodes
        // - Minimize data movement between nodes
        // - Consider data compression and caching

        Ok(plan) // Placeholder
    }

    /// Estimate cost of distributed execution
    fn estimate_distributed_cost<'a>(
        &'a self,
        plan: &'a DistributedPlanNode,
        _topology: &'a ClusterTopology,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = f64> + 'a + Send>> {
        Box::pin(async move {
            match plan {
                DistributedPlanNode::Local(_) => 100.0, // Base local cost
                DistributedPlanNode::Remote { data_transfer, .. } => {
                    let mut cost = 200.0; // Base remote execution cost
                    if let Some(transfer) = data_transfer {
                        cost += transfer.estimated_size_mb * self.cost_model.network_cost_per_mb;
                    }
                    cost
                }
                DistributedPlanNode::Parallel { sub_plans, .. } => {
                    let mut total_cost = 0.0;
                    for sub_plan in sub_plans {
                        total_cost += self.estimate_distributed_cost(sub_plan, _topology).await;
                    }
                    total_cost * 0.8 // Parallelization efficiency discount
                }
                DistributedPlanNode::Exchange { exchange_type, .. } => {
                    match exchange_type {
                        ExchangeType::Broadcast => 1000.0, // Expensive due to multiple copies
                        ExchangeType::Redistribute => 500.0,
                        ExchangeType::Gather => 300.0,
                        ExchangeType::PointToPoint => 100.0,
                    }
                }
            }
        })
    }

    /// Calculate resource requirements for distributed plan
    async fn calculate_resource_requirements(
        &self,
        _plan: &DistributedPlanNode,
    ) -> ResourceRequirements {
        // TODO: Implement resource requirement calculation
        ResourceRequirements {
            estimated_nodes: 1,
            estimated_memory_gb: 1.0,
            estimated_cpu_cores: 1,
            estimated_network_mb: 100.0,
        }
    }

    /// Calculate data locality score (higher is better)
    async fn calculate_data_locality_score(
        &self,
        _plan: &DistributedPlanNode,
        _topology: &ClusterTopology,
    ) -> f64 {
        // TODO: Implement data locality scoring
        // Consider how much data needs to be moved vs. locally processed
        0.8 // Placeholder score
    }

    /// Update cluster topology information
    pub async fn update_topology(&self, topology: ClusterTopology) {
        *self.cluster_topology.write().await = topology;
    }
}

/// Join strategies for distributed execution
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum JoinStrategy {
    /// Broadcast smaller relation to all nodes
    Broadcast,
    /// Hash partition both relations on join keys
    Hash,
    /// Keep join local (no distribution)
    Local,
}

/// Distributed execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedExecutionPlan {
    pub execution_id: Uuid,
    pub root: DistributedPlanNode,
    pub estimated_cost: f64,
    pub resource_requirements: ResourceRequirements,
    pub data_locality_score: f64,
}

/// Resource requirements for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub estimated_nodes: u32,
    pub estimated_memory_gb: f64,
    pub estimated_cpu_cores: u32,
    pub estimated_network_mb: f64,
}

/// Distributed query executor
pub struct DistributedQueryExecutor {
    coordinator_id: ActorId,
    execution_context: Arc<RwLock<Option<DistributedExecutionContext>>>,
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionState>>>,
    task_semaphore: Arc<Semaphore>,
}

/// State of a distributed execution
#[derive(Debug, Clone)]
pub struct ExecutionState {
    pub execution_id: Uuid,
    pub status: ExecutionStatus,
    pub start_time: Instant,
    pub nodes_involved: Vec<ActorId>,
    pub partial_results: HashMap<ActorId, QueryResult>,
    pub error: Option<String>,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Planning,
    Executing,
    Merging,
    Completed,
    Failed,
    Cancelled,
}

impl DistributedQueryExecutor {
    pub fn new(coordinator_id: ActorId, max_concurrent_executions: usize) -> Self {
        Self {
            coordinator_id,
            execution_context: Arc::new(RwLock::new(None)),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            task_semaphore: Arc::new(Semaphore::new(max_concurrent_executions)),
        }
    }

    /// Plan a distributed query - placeholder implementation
    pub async fn plan_distributed_query(
        &self,
        _query: &str,
    ) -> OrbitResult<DistributedExecutionPlan> {
        // TODO: Implement actual query planning logic
        // This is a placeholder that creates a minimal plan
        Ok(DistributedExecutionPlan {
            execution_id: Uuid::new_v4(),
            root: DistributedPlanNode::Local(PlanNode::TableScan {
                table: "placeholder".to_string(),
                columns: vec![],
                filter: None,
            }),
            estimated_cost: 1.0,
            resource_requirements: ResourceRequirements {
                estimated_nodes: 1,
                estimated_memory_gb: 1.0,
                estimated_cpu_cores: 1,
                estimated_network_mb: 0.0,
            },
            data_locality_score: 1.0,
        })
    }

    /// Execute distributed query plan
    pub async fn execute_distributed(
        &self,
        plan: DistributedExecutionPlan,
        context: DistributedExecutionContext,
    ) -> OrbitResult<QueryResult> {
        let _permit =
            self.task_semaphore.acquire().await.map_err(|_| {
                OrbitError::execution("Failed to acquire execution permit".to_string())
            })?;

        // Initialize execution state
        let execution_state = ExecutionState {
            execution_id: plan.execution_id,
            status: ExecutionStatus::Executing,
            start_time: Instant::now(),
            nodes_involved: self.extract_involved_nodes(&plan.root),
            partial_results: HashMap::new(),
            error: None,
        };

        self.active_executions
            .write()
            .await
            .insert(plan.execution_id, execution_state);
        *self.execution_context.write().await = Some(context);

        // Execute the distributed plan
        let result = self.execute_plan_node(plan.root).await;

        // Update execution state
        let mut executions = self.active_executions.write().await;
        if let Some(state) = executions.get_mut(&plan.execution_id) {
            match &result {
                Ok(_) => state.status = ExecutionStatus::Completed,
                Err(e) => {
                    state.status = ExecutionStatus::Failed;
                    state.error = Some(e.to_string());
                }
            }
        }

        result
    }

    /// Execute a single distributed plan node
    fn execute_plan_node(
        &self,
        node: DistributedPlanNode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = OrbitResult<QueryResult>> + Send + '_>>
    {
        Box::pin(async move {
            match node {
                DistributedPlanNode::Local(_plan) => {
                    // Execute locally using standard query executor
                    // TODO: Use actual local executor
                    Ok(QueryResult::default())
                }

                DistributedPlanNode::Remote {
                    target_actor, plan, ..
                } => {
                    // Send execution request to remote actor
                    self.execute_remote(target_actor, plan).await
                }

                DistributedPlanNode::Parallel {
                    sub_plans,
                    merge_strategy,
                } => {
                    // Execute all sub-plans sequentially for now to avoid Send issues
                    // TODO: Implement proper parallel execution with Send-safe futures
                    let mut results = Vec::new();

                    for sub_plan in sub_plans {
                        let result = self.execute_plan_node(sub_plan).await?;
                        results.push(result);
                    }

                    // Merge results according to strategy
                    self.merge_results(results, merge_strategy).await
                }

                DistributedPlanNode::Exchange {
                    source_actor,
                    target_actor,
                    exchange_type,
                    ..
                } => {
                    // Handle data exchange between nodes
                    self.execute_exchange(source_actor, target_actor, exchange_type)
                        .await
                }
            }
        })
    }

    /// Execute query on remote actor
    async fn execute_remote(
        &self,
        target_actor: ActorId,
        _plan: PlanNode,
    ) -> OrbitResult<QueryResult> {
        // TODO: Implement actual remote execution via actor system
        // This would involve:
        // 1. Serializing the plan
        // 2. Sending to target actor
        // 3. Waiting for response
        // 4. Handling timeouts and failures

        debug!("Executing remote query on actor {:?}", target_actor);

        // Placeholder implementation
        Ok(QueryResult::default())
    }

    /// Execute data exchange between actors
    async fn execute_exchange(
        &self,
        source_actor: ActorId,
        target_actor: ActorId,
        exchange_type: ExchangeType,
    ) -> OrbitResult<QueryResult> {
        // TODO: Implement data exchange
        debug!(
            "Executing data exchange {:?} from {:?} to {:?}",
            exchange_type, source_actor, target_actor
        );

        Ok(QueryResult::default())
    }

    /// Merge results from parallel execution
    async fn merge_results(
        &self,
        results: Vec<QueryResult>,
        merge_strategy: MergeStrategy,
    ) -> OrbitResult<QueryResult> {
        match merge_strategy {
            MergeStrategy::Concat => {
                // Simple concatenation
                let merged = QueryResult::default();
                for _result in results {
                    // TODO: Implement result concatenation
                }
                Ok(merged)
            }

            MergeStrategy::Union => {
                // Union with deduplication
                // TODO: Implement union logic
                Ok(QueryResult::default())
            }

            MergeStrategy::MergeSorted { sort_keys: _ } => {
                // Merge pre-sorted results
                // TODO: Implement sorted merge
                Ok(QueryResult::default())
            }

            MergeStrategy::Join { join_keys: _ } => {
                // Join results
                // TODO: Implement result joining
                Ok(QueryResult::default())
            }

            MergeStrategy::Aggregate {
                group_by: _,
                functions: _,
            } => {
                // Aggregate results
                // TODO: Implement result aggregation
                Ok(QueryResult::default())
            }
        }
    }

    /// Extract actor IDs involved in the plan
    #[allow(clippy::only_used_in_recursion)]
    fn extract_involved_nodes(&self, node: &DistributedPlanNode) -> Vec<ActorId> {
        let mut nodes = Vec::new();

        match node {
            DistributedPlanNode::Remote { target_actor, .. } => {
                nodes.push(target_actor.clone());
            }
            DistributedPlanNode::Parallel { sub_plans, .. } => {
                for sub_plan in sub_plans {
                    nodes.extend(self.extract_involved_nodes(sub_plan));
                }
            }
            DistributedPlanNode::Exchange {
                source_actor,
                target_actor,
                ..
            } => {
                nodes.push(source_actor.clone());
                nodes.push(target_actor.clone());
            }
            _ => {}
        }

        nodes
    }

    /// Get status of active executions
    pub async fn get_execution_status(&self, execution_id: Uuid) -> Option<ExecutionState> {
        self.active_executions
            .read()
            .await
            .get(&execution_id)
            .cloned()
    }

    /// Cancel a running execution
    pub async fn cancel_execution(&self, execution_id: Uuid) -> OrbitResult<()> {
        let mut executions = self.active_executions.write().await;
        if let Some(state) = executions.get_mut(&execution_id) {
            state.status = ExecutionStatus::Cancelled;
            info!("Cancelled execution {}", execution_id);
            Ok(())
        } else {
            Err(OrbitError::execution(format!(
                "Execution {execution_id} not found"
            )))
        }
    }
}

// Placeholder Clone implementation for demonstration
impl Clone for DistributedQueryExecutor {
    fn clone(&self) -> Self {
        Self {
            coordinator_id: self.coordinator_id.clone(),
            execution_context: self.execution_context.clone(),
            active_executions: self.active_executions.clone(),
            task_semaphore: self.task_semaphore.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_planner_creation() {
        let local_planner = QueryPlanner::new();
        let _distributed_planner = DistributedQueryPlanner::new(local_planner);

        // Test that planner can be created
        // No assertion needed for constructor test
    }

    #[tokio::test]
    async fn test_distributed_executor_creation() {
        let coordinator_id = "test-coordinator".to_string();
        let _executor = DistributedQueryExecutor::new(coordinator_id, 10);

        // Test that executor can be created
        // No assertion needed for constructor test
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_nodes, 10);
        assert_eq!(limits.max_parallel_tasks, 100);
    }

    #[test]
    fn test_cost_model_default() {
        let cost_model = DistributedCostModel::default();
        assert_eq!(cost_model.network_cost_per_mb, 10.0);
        assert_eq!(cost_model.cpu_cost_per_operation, 1.0);
    }
}
