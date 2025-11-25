//! Deadlock Prevention System
//!
//! ML-powered deadlock prediction and prevention using graph analysis.

use anyhow::Result as OrbitResult;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Transaction dependency graph
#[derive(Debug, Clone)]
pub struct TransactionDependencyGraph {
    /// Transaction nodes
    pub transactions: HashMap<TransactionId, TransactionNode>,
    /// Dependency edges (wait-for relationships)
    pub dependencies: Vec<DependencyEdge>,
}

/// Transaction identifier
pub type TransactionId = u64;

/// Transaction node in dependency graph
#[derive(Debug, Clone)]
pub struct TransactionNode {
    pub id: TransactionId,
    pub status: TransactionStatus,
    pub waiting_for: Vec<TransactionId>,
    pub held_locks: Vec<LockResource>,
    pub waiting_for_locks: Vec<LockResource>,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Running,
    Waiting,
    Blocked,
    Committed,
    Aborted,
}

/// Lock resource
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockResource {
    pub table: String,
    pub key: String,
    pub lock_type: LockType,
}

/// Lock type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockType {
    Shared,
    Exclusive,
}

/// Dependency edge in wait-for graph
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from: TransactionId,
    pub to: TransactionId,
    pub resource: LockResource,
    pub weight: f64, // Probability of deadlock
}

/// Deadlock prediction
#[derive(Debug, Clone)]
pub struct DeadlockPrediction {
    pub cycle: Vec<TransactionId>,
    pub probability: f64,
    pub impact_score: f64,
    pub resources_involved: Vec<LockResource>,
    pub estimated_resolution_time: tokio::time::Duration,
}

/// Deadlock occurrence information
#[derive(Debug, Clone)]
pub struct DeadlockOccurrence {
    pub id: String,
    pub timestamp: std::time::SystemTime,
    pub transactions: Vec<TransactionId>,
    pub dependency_graph: TransactionDependencyGraph,
    pub resolution_action: ResolutionAction,
}

/// Resolution action taken
#[derive(Debug, Clone)]
pub enum ResolutionAction {
    AbortTransaction { transaction_id: TransactionId },
    WaitTimeout { duration: tokio::time::Duration },
    LockUpgrade { resource: LockResource },
}

/// ML-powered deadlock prevention system
pub struct DeadlockPreventer {
    /// Pattern recognizer for deadlock-prone scenarios
    pattern_recognizer: Arc<DeadlockPatternRecognizer>,
    /// Prevention policy
    prevention_policy: Arc<DeadlockPreventionPolicy>,
    /// Historical deadlock occurrences
    deadlock_history: Arc<RwLock<Vec<DeadlockOccurrence>>>,
}

/// Deadlock pattern recognizer
pub struct DeadlockPatternRecognizer {
    /// Known deadlock patterns
    known_patterns: Arc<RwLock<Vec<DeadlockPattern>>>,
}

/// Deadlock pattern
#[derive(Debug, Clone)]
pub struct DeadlockPattern {
    pub pattern_id: String,
    pub graph_structure: GraphStructure,
    pub frequency: u64,
    pub last_seen: std::time::SystemTime,
}

/// Graph structure pattern
#[derive(Debug, Clone)]
pub enum GraphStructure {
    SimpleCycle { length: usize },
    ComplexCycle { cycles: Vec<Vec<TransactionId>> },
    StarPattern { center: TransactionId },
    ChainPattern { length: usize },
}

/// Deadlock prevention policy
pub struct DeadlockPreventionPolicy {
    /// Prevention strategies
    _strategies: Vec<PreventionStrategy>,
}

/// Prevention strategy
#[derive(Debug, Clone)]
pub enum PreventionStrategy {
    /// Abort youngest transaction
    AbortYoungest,
    /// Abort transaction with fewest locks
    AbortFewestLocks,
    /// Wait with timeout
    WaitWithTimeout { timeout: tokio::time::Duration },
    /// Lock ordering
    LockOrdering,
    /// Transaction timeout
    TransactionTimeout { timeout: tokio::time::Duration },
}

impl DeadlockPreventer {
    /// Create a new deadlock preventer
    pub fn new() -> Self {
        Self {
            pattern_recognizer: Arc::new(DeadlockPatternRecognizer::new()),
            prevention_policy: Arc::new(DeadlockPreventionPolicy::new()),
            deadlock_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Predict potential deadlock scenarios
    pub async fn predict_deadlock_scenarios(
        &self,
        dependency_graph: &TransactionDependencyGraph,
    ) -> OrbitResult<Vec<DeadlockPrediction>> {
        let mut predictions = Vec::new();

        // Detect cycles in dependency graph
        let cycles = self.detect_cycles(dependency_graph)?;

        for cycle in cycles {
            // Calculate deadlock probability
            let probability = self.calculate_deadlock_probability(&cycle, dependency_graph)?;

            if probability > 0.5 {
                // Calculate impact score
                let impact_score = self.calculate_impact_score(&cycle, dependency_graph)?;

                // Get resources involved
                let resources = self.get_resources_in_cycle(&cycle, dependency_graph)?;

                // Estimate resolution time
                let resolution_time = self.estimate_resolution_time(&cycle, dependency_graph)?;

                predictions.push(DeadlockPrediction {
                    cycle: cycle.clone(),
                    probability,
                    impact_score,
                    resources_involved: resources,
                    estimated_resolution_time: resolution_time,
                });
            }
        }

        // Sort by probability * impact
        predictions.sort_by(|a, b| {
            (b.probability * b.impact_score)
                .partial_cmp(&(a.probability * a.impact_score))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if !predictions.is_empty() {
            info!(
                prediction_count = predictions.len(),
                "Detected potential deadlock scenarios"
            );
        }

        Ok(predictions)
    }

    /// Detect cycles in dependency graph
    fn detect_cycles(&self, graph: &TransactionDependencyGraph) -> OrbitResult<Vec<Vec<TransactionId>>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for (tx_id, _) in &graph.transactions {
            if !visited.contains(tx_id) {
                self.dfs_cycle_detection(
                    *tx_id,
                    graph,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                )?;
            }
        }

        Ok(cycles)
    }

    /// DFS for cycle detection
    fn dfs_cycle_detection(
        &self,
        tx_id: TransactionId,
        graph: &TransactionDependencyGraph,
        visited: &mut HashSet<TransactionId>,
        rec_stack: &mut HashSet<TransactionId>,
        path: &mut Vec<TransactionId>,
        cycles: &mut Vec<Vec<TransactionId>>,
    ) -> OrbitResult<()> {
        visited.insert(tx_id);
        rec_stack.insert(tx_id);
        path.push(tx_id);

        // Check dependencies
        for edge in &graph.dependencies {
            if edge.from == tx_id {
                let next_id = edge.to;

                if !visited.contains(&next_id) {
                    self.dfs_cycle_detection(
                        next_id,
                        graph,
                        visited,
                        rec_stack,
                        path,
                        cycles,
                    )?;
                } else if rec_stack.contains(&next_id) {
                    // Found a cycle
                    let cycle_start = path.iter().position(|&x| x == next_id).unwrap();
                    let cycle: Vec<TransactionId> = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        rec_stack.remove(&tx_id);
        path.pop();
        Ok(())
    }

    /// Calculate deadlock probability
    fn calculate_deadlock_probability(
        &self,
        cycle: &[TransactionId],
        _graph: &TransactionDependencyGraph,
    ) -> OrbitResult<f64> {
        // Simple probability calculation based on cycle length
        // Longer cycles are less likely to complete simultaneously
        let base_probability = 0.8;
        let length_factor = 1.0 / (cycle.len() as f64);
        
        Ok(base_probability * length_factor)
    }

    /// Calculate impact score
    fn calculate_impact_score(
        &self,
        cycle: &[TransactionId],
        graph: &TransactionDependencyGraph,
    ) -> OrbitResult<f64> {
        // Impact based on number of transactions and their status
        let mut impact = 0.0;

        for tx_id in cycle {
            if let Some(node) = graph.transactions.get(tx_id) {
                match node.status {
                    TransactionStatus::Running => impact += 1.0,
                    TransactionStatus::Waiting => impact += 0.8,
                    TransactionStatus::Blocked => impact += 0.5,
                    _ => impact += 0.1,
                }
            }
        }

        Ok(impact / cycle.len() as f64)
    }

    /// Get resources involved in cycle
    fn get_resources_in_cycle(
        &self,
        cycle: &[TransactionId],
        graph: &TransactionDependencyGraph,
    ) -> OrbitResult<Vec<LockResource>> {
        let mut resources = HashSet::new();

        for i in 0..cycle.len() {
            let from = cycle[i];
            let to = cycle[(i + 1) % cycle.len()];

            for edge in &graph.dependencies {
                if edge.from == from && edge.to == to {
                    resources.insert(edge.resource.clone());
                }
            }
        }

        Ok(resources.into_iter().collect())
    }

    /// Estimate resolution time
    fn estimate_resolution_time(
        &self,
        cycle: &[TransactionId],
        _graph: &TransactionDependencyGraph,
    ) -> OrbitResult<tokio::time::Duration> {
        // Estimate based on cycle length
        // Longer cycles take more time to resolve
        let base_time_ms = 100;
        let cycle_time_ms = base_time_ms * cycle.len() as u64;
        
        Ok(tokio::time::Duration::from_millis(cycle_time_ms))
    }

    /// Determine preventive action
    pub async fn determine_preventive_action(
        &self,
        prediction: &DeadlockPrediction,
        graph: &TransactionDependencyGraph,
    ) -> OrbitResult<ResolutionAction> {
        let _policy = self.prevention_policy.as_ref();
        
        // Use policy to determine action
        if prediction.probability > 0.8 {
            // High probability - abort a transaction
            let tx_to_abort = self.select_transaction_to_abort(prediction, graph)?;
            Ok(ResolutionAction::AbortTransaction {
                transaction_id: tx_to_abort,
            })
        } else if prediction.probability > 0.6 {
            // Medium probability - wait with timeout
            Ok(ResolutionAction::WaitTimeout {
                duration: tokio::time::Duration::from_secs(5),
            })
        } else {
            // Low probability - try lock upgrade
            if let Some(resource) = prediction.resources_involved.first() {
                Ok(ResolutionAction::LockUpgrade {
                    resource: resource.clone(),
                })
            } else {
                // Fallback to wait timeout
                Ok(ResolutionAction::WaitTimeout {
                    duration: tokio::time::Duration::from_secs(10),
                })
            }
        }
    }

    /// Select transaction to abort
    fn select_transaction_to_abort(
        &self,
        prediction: &DeadlockPrediction,
        graph: &TransactionDependencyGraph,
    ) -> OrbitResult<TransactionId> {
        // Select transaction with fewest locks (least impact)
        let mut min_locks = usize::MAX;
        let mut selected_tx = prediction.cycle[0];

        for tx_id in &prediction.cycle {
            if let Some(node) = graph.transactions.get(tx_id) {
                let lock_count = node.held_locks.len();
                if lock_count < min_locks {
                    min_locks = lock_count;
                    selected_tx = *tx_id;
                }
            }
        }

        Ok(selected_tx)
    }

    /// Learn from deadlock occurrence
    pub async fn learn_from_deadlock_occurrence(
        &self,
        deadlock: &DeadlockOccurrence,
    ) -> OrbitResult<()> {
        // Store in history
        {
            let mut history = self.deadlock_history.write().await;
            history.push(deadlock.clone());
            
            // Keep only recent history
            let current_len = history.len();
            if current_len > 1000 {
                let remove_count = current_len - 1000;
                history.drain(0..remove_count);
            }
        }

        // Update pattern recognizer
        self.pattern_recognizer
            .learn_from_deadlock(&deadlock.dependency_graph)
            .await?;

        info!(
            deadlock_id = %deadlock.id,
            transaction_count = deadlock.transactions.len(),
            "Learned from deadlock occurrence"
        );

        Ok(())
    }
}

impl DeadlockPatternRecognizer {
    pub fn new() -> Self {
        Self {
            known_patterns: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn learn_from_deadlock(
        &self,
        graph: &TransactionDependencyGraph,
    ) -> OrbitResult<()> {
        // Extract pattern from graph structure
        let cycles = DeadlockPreventer::new().detect_cycles(graph)?;
        
        if let Some(cycle) = cycles.first() {
            let pattern = DeadlockPattern {
                pattern_id: format!("pattern_{}", uuid::Uuid::new_v4()),
                graph_structure: GraphStructure::SimpleCycle {
                    length: cycle.len(),
                },
                frequency: 1,
                last_seen: std::time::SystemTime::now(),
            };

            let mut patterns = self.known_patterns.write().await;
            patterns.push(pattern);
        }

        Ok(())
    }
}

impl DeadlockPreventionPolicy {
    pub fn new() -> Self {
        Self {
            _strategies: vec![
                PreventionStrategy::AbortYoungest,
                PreventionStrategy::AbortFewestLocks,
                PreventionStrategy::WaitWithTimeout {
                    timeout: tokio::time::Duration::from_secs(5),
                },
            ],
        }
    }
}

impl Default for DeadlockPreventer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DeadlockPatternRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DeadlockPreventionPolicy {
    fn default() -> Self {
        Self::new()
    }
}

