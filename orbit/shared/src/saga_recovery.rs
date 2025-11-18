use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::recovery::{RecoveryEventHandler, TransactionRecoveryManager};
use crate::saga::{SagaExecution, SagaId, SagaOrchestrator, SagaState};
use crate::transaction_log::PersistentTransactionLogger;
use crate::transactions::TransactionId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for saga recovery integration
#[derive(Debug, Clone)]
pub struct SagaRecoveryConfig {
    /// Maximum time to wait for saga recovery
    pub max_recovery_time: Duration,
    /// Recovery checkpoint interval
    pub checkpoint_interval: Duration,
    /// Enable automatic saga failover
    pub enable_automatic_failover: bool,
    /// Maximum number of recovery attempts
    pub max_recovery_attempts: u32,
}

impl Default for SagaRecoveryConfig {
    fn default() -> Self {
        Self {
            max_recovery_time: Duration::from_secs(600), // 10 minutes
            checkpoint_interval: Duration::from_secs(30),
            enable_automatic_failover: true,
            max_recovery_attempts: 3,
        }
    }
}

/// Saga recovery checkpoint containing execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaRecoveryCheckpoint {
    pub saga_id: SagaId,
    pub execution_id: String,
    pub coordinator_node: NodeId,
    pub saga_state: SagaState,
    pub definition_id: String,
    pub completed_steps: Vec<String>,
    pub failed_steps: Vec<String>,
    pub compensated_steps: Vec<String>,
    pub current_step: Option<String>,
    pub context_data: HashMap<String, serde_json::Value>,
    pub started_at: i64,
    pub last_updated: i64,
    pub recovery_attempts: u32,
}

impl From<&SagaExecution> for SagaRecoveryCheckpoint {
    fn from(execution: &SagaExecution) -> Self {
        Self {
            saga_id: execution.saga_id.clone(),
            execution_id: execution.execution_id.clone(),
            coordinator_node: execution.saga_id.coordinator_node.clone(),
            saga_state: execution.state.clone(),
            definition_id: execution.definition_id.clone(),
            completed_steps: execution.completed_steps.clone(),
            failed_steps: execution.failed_steps.clone(),
            compensated_steps: execution.compensated_steps.clone(),
            current_step: execution.current_step.clone(),
            context_data: execution.context.data.clone(),
            started_at: execution.started_at,
            last_updated: chrono::Utc::now().timestamp_millis(),
            recovery_attempts: 0,
        }
    }
}

/// Saga recovery manager that handles saga failures and coordinator transitions
pub struct SagaRecoveryManager {
    node_id: NodeId,
    config: SagaRecoveryConfig,
    saga_orchestrator: Arc<SagaOrchestrator>,
    recovery_manager: Arc<TransactionRecoveryManager>,
    checkpoints: Arc<RwLock<HashMap<SagaId, SagaRecoveryCheckpoint>>>,
    logger: Arc<dyn PersistentTransactionLogger>,
}

impl SagaRecoveryManager {
    pub fn new(
        node_id: NodeId,
        config: SagaRecoveryConfig,
        saga_orchestrator: Arc<SagaOrchestrator>,
        recovery_manager: Arc<TransactionRecoveryManager>,
        logger: Arc<dyn PersistentTransactionLogger>,
    ) -> Self {
        Self {
            node_id,
            config,
            saga_orchestrator,
            recovery_manager,
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            logger,
        }
    }

    /// Create a recovery checkpoint for a saga execution
    pub async fn create_saga_checkpoint(&self, execution: &SagaExecution) -> OrbitResult<()> {
        let checkpoint = SagaRecoveryCheckpoint::from(execution);

        // Store checkpoint in memory
        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints.insert(checkpoint.saga_id.clone(), checkpoint.clone());
        }

        // Persist checkpoint to log
        self.persist_saga_checkpoint(&checkpoint).await?;

        debug!(
            "Created saga recovery checkpoint for: {}",
            checkpoint.saga_id
        );
        Ok(())
    }

    /// Update an existing saga checkpoint
    pub async fn update_saga_checkpoint(
        &self,
        saga_id: &SagaId,
        execution: &SagaExecution,
    ) -> OrbitResult<()> {
        let mut checkpoints = self.checkpoints.write().await;
        if let Some(checkpoint) = checkpoints.get_mut(saga_id) {
            // Update checkpoint with current execution state
            checkpoint.saga_state = execution.state.clone();
            checkpoint.completed_steps = execution.completed_steps.clone();
            checkpoint.failed_steps = execution.failed_steps.clone();
            checkpoint.compensated_steps = execution.compensated_steps.clone();
            checkpoint.current_step = execution.current_step.clone();
            checkpoint.context_data = execution.context.data.clone();
            checkpoint.last_updated = chrono::Utc::now().timestamp_millis();

            // Persist updated checkpoint
            self.persist_saga_checkpoint(checkpoint).await?;

            debug!("Updated saga recovery checkpoint for: {}", saga_id);
        } else {
            return Err(OrbitError::internal("Saga checkpoint not found"));
        }

        Ok(())
    }

    /// Remove saga checkpoint after completion
    pub async fn remove_saga_checkpoint(&self, saga_id: &SagaId) -> OrbitResult<()> {
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.remove(saga_id);
        debug!("Removed saga recovery checkpoint for: {}", saga_id);
        Ok(())
    }

    /// Recover sagas from a failed coordinator
    pub async fn recover_sagas_from_coordinator(
        &self,
        failed_coordinator: &NodeId,
    ) -> OrbitResult<()> {
        info!(
            "Recovering sagas from failed coordinator: {}",
            failed_coordinator
        );

        let sagas_to_recover = {
            let checkpoints = self.checkpoints.read().await;
            checkpoints
                .values()
                .filter(|checkpoint| &checkpoint.coordinator_node == failed_coordinator)
                .filter(|checkpoint| self.is_recoverable_state(&checkpoint.saga_state))
                .cloned()
                .collect::<Vec<_>>()
        };

        if sagas_to_recover.is_empty() {
            info!(
                "No sagas to recover from coordinator: {}",
                failed_coordinator
            );
            return Ok(());
        }

        info!(
            "Found {} sagas to recover from coordinator: {}",
            sagas_to_recover.len(),
            failed_coordinator
        );

        for checkpoint in sagas_to_recover {
            if let Err(e) = self.recover_saga_execution(&checkpoint).await {
                error!("Failed to recover saga {}: {}", checkpoint.saga_id, e);
            }
        }

        Ok(())
    }

    /// Check if saga state is recoverable
    fn is_recoverable_state(&self, state: &SagaState) -> bool {
        matches!(
            state,
            SagaState::Initializing
                | SagaState::Executing
                | SagaState::Compensating
                | SagaState::Paused
        )
    }

    /// Recover a specific saga execution
    async fn recover_saga_execution(&self, checkpoint: &SagaRecoveryCheckpoint) -> OrbitResult<()> {
        info!("Recovering saga execution: {}", checkpoint.saga_id);

        // Validate recovery can proceed
        self.validate_recovery_attempts(checkpoint)?;

        // Reconstruct saga execution from checkpoint
        let recovered_execution = self.reconstruct_saga_execution(checkpoint).await?;

        // Execute recovery strategy based on saga state
        self.execute_recovery_strategy(checkpoint, &recovered_execution)
            .await?;

        // Update recovery attempts counter
        self.update_recovery_attempts(&checkpoint.saga_id).await;

        info!("Saga recovery completed: {}", checkpoint.saga_id);
        Ok(())
    }

    /// Validate that recovery attempts are within limits
    fn validate_recovery_attempts(&self, checkpoint: &SagaRecoveryCheckpoint) -> OrbitResult<()> {
        if checkpoint.recovery_attempts >= self.config.max_recovery_attempts {
            error!(
                "Max recovery attempts exceeded for saga: {}",
                checkpoint.saga_id
            );
            return Err(OrbitError::internal("Max recovery attempts exceeded"));
        }
        Ok(())
    }

    /// Execute the appropriate recovery strategy based on saga state
    async fn execute_recovery_strategy(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
        recovered_execution: &SagaExecution,
    ) -> OrbitResult<()> {
        match checkpoint.saga_state {
            SagaState::Initializing => {
                self.handle_initializing_saga_recovery(checkpoint, recovered_execution)
                    .await
            }
            SagaState::Executing => {
                self.handle_executing_saga_recovery(checkpoint, recovered_execution)
                    .await
            }
            SagaState::Compensating => {
                self.handle_compensating_saga_recovery(checkpoint, recovered_execution)
                    .await
            }
            SagaState::Paused => {
                self.handle_paused_saga_recovery(checkpoint, recovered_execution)
                    .await
            }
            _ => {
                self.handle_non_recoverable_saga(checkpoint);
                Ok(())
            }
        }
    }

    /// Handle recovery of initializing saga
    async fn handle_initializing_saga_recovery(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
        recovered_execution: &SagaExecution,
    ) -> OrbitResult<()> {
        info!("Restarting initializing saga: {}", checkpoint.saga_id);
        self.restart_saga_execution(recovered_execution).await
    }

    /// Handle recovery of executing saga
    async fn handle_executing_saga_recovery(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
        recovered_execution: &SagaExecution,
    ) -> OrbitResult<()> {
        info!("Resuming executing saga: {}", checkpoint.saga_id);
        self.resume_saga_execution(recovered_execution).await
    }

    /// Handle recovery of compensating saga
    async fn handle_compensating_saga_recovery(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
        recovered_execution: &SagaExecution,
    ) -> OrbitResult<()> {
        info!("Continuing saga compensation: {}", checkpoint.saga_id);
        self.continue_saga_compensation(recovered_execution).await
    }

    /// Handle recovery of paused saga
    async fn handle_paused_saga_recovery(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
        recovered_execution: &SagaExecution,
    ) -> OrbitResult<()> {
        info!(
            "Saga is paused, will resume when requested: {}",
            checkpoint.saga_id
        );
        self.store_paused_saga(recovered_execution).await
    }

    /// Handle non-recoverable saga states
    fn handle_non_recoverable_saga(&self, checkpoint: &SagaRecoveryCheckpoint) {
        debug!(
            "Saga {} is in non-recoverable state: {:?}",
            checkpoint.saga_id, checkpoint.saga_state
        );
    }

    /// Update recovery attempts counter for a saga
    async fn update_recovery_attempts(&self, saga_id: &SagaId) {
        let mut checkpoints = self.checkpoints.write().await;
        if let Some(checkpoint) = checkpoints.get_mut(saga_id) {
            checkpoint.recovery_attempts += 1;
            checkpoint.last_updated = chrono::Utc::now().timestamp_millis();
        }
    }

    /// Reconstruct saga execution from checkpoint
    async fn reconstruct_saga_execution(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
    ) -> OrbitResult<SagaExecution> {
        use crate::saga::{SagaContext, SagaExecution};

        // Reconstruct saga context
        let mut context = SagaContext::new(checkpoint.saga_id.clone(), self.node_id.clone());
        context.data = checkpoint.context_data.clone();
        context.current_step = checkpoint.current_step.clone();

        // Reconstruct execution state
        let mut execution = SagaExecution::new(
            checkpoint.saga_id.clone(),
            checkpoint.definition_id.clone(),
            context,
        );

        execution.state = checkpoint.saga_state.clone();
        execution.completed_steps = checkpoint.completed_steps.clone();
        execution.failed_steps = checkpoint.failed_steps.clone();
        execution.compensated_steps = checkpoint.compensated_steps.clone();
        execution.current_step = checkpoint.current_step.clone();
        execution.started_at = checkpoint.started_at;

        Ok(execution)
    }

    /// Restart saga execution from the beginning
    async fn restart_saga_execution(&self, execution: &SagaExecution) -> OrbitResult<()> {
        // Create a fresh saga execution
        let saga_id = self
            .saga_orchestrator
            .start_saga(&execution.definition_id, execution.context.data.clone())
            .await?;

        info!(
            "Restarted saga with new ID: {} (original: {})",
            saga_id, execution.saga_id
        );
        Ok(())
    }

    /// Resume saga execution from current step
    async fn resume_saga_execution(&self, execution: &SagaExecution) -> OrbitResult<()> {
        // Submit execution to orchestrator for continuation
        self.saga_orchestrator
            .execute_saga(&execution.saga_id)
            .await?;
        info!("Resumed saga execution: {}", execution.saga_id);
        Ok(())
    }

    /// Continue saga compensation process
    async fn continue_saga_compensation(&self, execution: &SagaExecution) -> OrbitResult<()> {
        // The orchestrator should handle compensation continuation
        self.saga_orchestrator
            .execute_saga(&execution.saga_id)
            .await?;
        info!("Continued saga compensation: {}", execution.saga_id);
        Ok(())
    }

    /// Store paused saga for later resumption
    async fn store_paused_saga(&self, execution: &SagaExecution) -> OrbitResult<()> {
        // Keep the saga checkpoint for later resumption
        info!("Stored paused saga: {}", execution.saga_id);
        Ok(())
    }

    /// Persist saga checkpoint to storage
    async fn persist_saga_checkpoint(
        &self,
        checkpoint: &SagaRecoveryCheckpoint,
    ) -> OrbitResult<()> {
        // In a real implementation, this would persist to the transaction log
        // For now, we'll just log the operation
        debug!("Persisting saga checkpoint: {}", checkpoint.saga_id);
        Ok(())
    }

    /// Start the saga recovery manager
    pub async fn start(&self) -> OrbitResult<()> {
        info!("Starting saga recovery manager");

        // Register as a recovery event handler
        let handler = Arc::new(SagaRecoveryEventHandler::new(self.clone()));
        self.recovery_manager.add_event_handler(handler).await;

        // Start background tasks
        self.start_background_tasks().await?;

        info!("Saga recovery manager started");
        Ok(())
    }

    /// Start background recovery tasks
    async fn start_background_tasks(&self) -> OrbitResult<()> {
        let recovery_manager = self.clone();

        // Checkpoint management task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(recovery_manager.config.checkpoint_interval);
            loop {
                interval.tick().await;
                if let Err(e) = recovery_manager.cleanup_old_checkpoints().await {
                    error!("Saga checkpoint cleanup failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Clean up old saga checkpoints
    async fn cleanup_old_checkpoints(&self) -> OrbitResult<()> {
        let cutoff_time = chrono::Utc::now().timestamp_millis() - (24 * 60 * 60 * 1000); // 24 hours

        let mut checkpoints = self.checkpoints.write().await;
        let mut to_remove = Vec::new();

        for (saga_id, checkpoint) in checkpoints.iter() {
            // Remove checkpoints for completed sagas or very old ones
            if matches!(
                checkpoint.saga_state,
                SagaState::Completed | SagaState::Aborted | SagaState::Failed
            ) || checkpoint.last_updated < cutoff_time
            {
                to_remove.push(saga_id.clone());
            }
        }

        for saga_id in to_remove {
            checkpoints.remove(&saga_id);
            debug!("Cleaned up old saga checkpoint: {}", saga_id);
        }

        Ok(())
    }

    /// Get saga recovery statistics
    pub async fn get_recovery_stats(&self) -> SagaRecoveryStats {
        let checkpoints = self.checkpoints.read().await;

        let mut stats = SagaRecoveryStats {
            total_checkpoints: checkpoints.len() as u64,
            active_recoveries: 0,
            completed_recoveries: 0,
            failed_recoveries: 0,
        };

        for checkpoint in checkpoints.values() {
            match checkpoint.saga_state {
                SagaState::Initializing | SagaState::Executing | SagaState::Compensating => {
                    stats.active_recoveries += 1;
                }
                SagaState::Completed => {
                    stats.completed_recoveries += 1;
                }
                SagaState::Failed | SagaState::Aborted => {
                    stats.failed_recoveries += 1;
                }
                _ => {}
            }
        }

        stats
    }
}

impl Clone for SagaRecoveryManager {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            config: self.config.clone(),
            saga_orchestrator: Arc::clone(&self.saga_orchestrator),
            recovery_manager: Arc::clone(&self.recovery_manager),
            checkpoints: Arc::clone(&self.checkpoints),
            logger: Arc::clone(&self.logger),
        }
    }
}

/// Statistics for saga recovery operations
#[derive(Debug, Clone)]
pub struct SagaRecoveryStats {
    pub total_checkpoints: u64,
    pub active_recoveries: u64,
    pub completed_recoveries: u64,
    pub failed_recoveries: u64,
}

/// Event handler that integrates saga recovery with transaction recovery
struct SagaRecoveryEventHandler {
    saga_recovery_manager: SagaRecoveryManager,
}

impl SagaRecoveryEventHandler {
    fn new(saga_recovery_manager: SagaRecoveryManager) -> Self {
        Self {
            saga_recovery_manager,
        }
    }
}

#[async_trait]
impl RecoveryEventHandler for SagaRecoveryEventHandler {
    async fn on_coordinator_failure(&self, failed_coordinator: &NodeId) -> OrbitResult<()> {
        info!(
            "Handling coordinator failure for sagas: {}",
            failed_coordinator
        );
        self.saga_recovery_manager
            .recover_sagas_from_coordinator(failed_coordinator)
            .await
    }

    async fn on_recovery_start(&self, _transaction_id: &TransactionId) -> OrbitResult<()> {
        // Transaction recovery started - sagas may be affected
        debug!("Transaction recovery started");
        Ok(())
    }

    async fn on_recovery_complete(
        &self,
        _transaction_id: &TransactionId,
        success: bool,
    ) -> OrbitResult<()> {
        if success {
            debug!("Transaction recovery completed successfully");
        } else {
            warn!("Transaction recovery failed");
        }
        Ok(())
    }

    async fn on_coordinator_elected(&self, new_coordinator: &NodeId) -> OrbitResult<()> {
        info!("New coordinator elected: {}", new_coordinator);
        // Could trigger saga recovery handoff if needed
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saga::SagaDefinition;
    use crate::transaction_log::{PersistentLogConfig, SqliteTransactionLogger};
    use tempfile::tempdir;

    // Mock cluster manager for testing
    struct MockClusterManager {
        nodes: Vec<NodeId>,
        current_leader: Option<NodeId>,
    }

    #[async_trait]
    impl crate::recovery::ClusterManager for MockClusterManager {
        async fn get_cluster_nodes(&self) -> OrbitResult<Vec<NodeId>> {
            Ok(self.nodes.clone())
        }

        async fn is_leader(&self, node_id: &NodeId) -> OrbitResult<bool> {
            Ok(self.current_leader.as_ref() == Some(node_id))
        }

        async fn start_election(&self, _candidate: &NodeId) -> OrbitResult<bool> {
            Ok(true) // Always win elections in tests
        }

        async fn report_coordinator_failure(
            &self,
            _failed_coordinator: &NodeId,
        ) -> OrbitResult<()> {
            Ok(())
        }

        async fn get_cluster_config(&self) -> OrbitResult<crate::recovery::ClusterConfig> {
            Ok(crate::recovery::ClusterConfig {
                total_nodes: self.nodes.len(),
                majority_threshold: self.nodes.len() / 2 + 1,
                current_leader: self.current_leader.clone(),
            })
        }
    }

    async fn create_test_recovery_manager() -> (
        SagaRecoveryManager,
        SagaOrchestrator,
        TransactionRecoveryManager,
    ) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("saga_recovery_test.db");

        let log_config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger: Arc<dyn PersistentTransactionLogger> =
            Arc::new(SqliteTransactionLogger::new(log_config).await.unwrap());
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());

        let cluster_manager = Arc::new(MockClusterManager {
            nodes: vec![node_id.clone()],
            current_leader: Some(node_id.clone()),
        });

        let saga_orchestrator =
            Arc::new(SagaOrchestrator::new(node_id.clone(), Arc::clone(&logger)));
        let recovery_manager = Arc::new(TransactionRecoveryManager::new(
            node_id.clone(),
            crate::recovery::RecoveryConfig::default(),
            Arc::clone(&logger),
            cluster_manager,
        ));

        let saga_recovery_manager = SagaRecoveryManager::new(
            node_id,
            SagaRecoveryConfig::default(),
            Arc::clone(&saga_orchestrator),
            Arc::clone(&recovery_manager),
            Arc::clone(&logger),
        );

        (
            saga_recovery_manager,
            (*saga_orchestrator).clone(),
            (*recovery_manager).clone(),
        )
    }

    #[tokio::test]
    async fn test_saga_checkpoint_creation() {
        let (recovery_manager, orchestrator, _) = create_test_recovery_manager().await;

        // Create mock saga step for testing
        struct MockSagaStep {
            metadata: crate::saga::SagaStepMetadata,
        }

        #[async_trait]
        impl crate::saga::SagaStep for MockSagaStep {
            async fn execute(
                &self,
                _context: &crate::saga::SagaContext,
            ) -> OrbitResult<crate::saga::StepResult> {
                Ok(crate::saga::StepResult::Success)
            }

            async fn compensate(
                &self,
                _context: &crate::saga::SagaContext,
            ) -> OrbitResult<crate::saga::StepResult> {
                Ok(crate::saga::StepResult::Success)
            }

            fn metadata(&self) -> &crate::saga::SagaStepMetadata {
                &self.metadata
            }
        }

        let step = MockSagaStep {
            metadata: crate::saga::SagaStepMetadata {
                step_id: "step1".to_string(),
                step_name: "Test Step".to_string(),
                description: None,
                timeout: None,
                max_retries: None,
                parallel_eligible: false,
                dependencies: vec![],
                tags: HashMap::new(),
            },
        };

        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(step));

        orchestrator
            .register_saga_definition(definition)
            .await
            .unwrap();
        let saga_id = orchestrator
            .start_saga("test-saga", HashMap::new())
            .await
            .unwrap();

        let execution = orchestrator.get_execution(&saga_id).await.unwrap().unwrap();
        recovery_manager
            .create_saga_checkpoint(&execution)
            .await
            .unwrap();

        let checkpoints = recovery_manager.checkpoints.read().await;
        assert!(checkpoints.contains_key(&saga_id));
    }

    #[tokio::test]
    async fn test_saga_recovery_stats() {
        let (recovery_manager, orchestrator, _) = create_test_recovery_manager().await;

        // Create mock saga step for testing
        struct MockSagaStep {
            metadata: crate::saga::SagaStepMetadata,
        }

        #[async_trait]
        impl crate::saga::SagaStep for MockSagaStep {
            async fn execute(
                &self,
                _context: &crate::saga::SagaContext,
            ) -> OrbitResult<crate::saga::StepResult> {
                Ok(crate::saga::StepResult::Success)
            }

            async fn compensate(
                &self,
                _context: &crate::saga::SagaContext,
            ) -> OrbitResult<crate::saga::StepResult> {
                Ok(crate::saga::StepResult::Success)
            }

            fn metadata(&self) -> &crate::saga::SagaStepMetadata {
                &self.metadata
            }
        }

        let step = MockSagaStep {
            metadata: crate::saga::SagaStepMetadata {
                step_id: "step1".to_string(),
                step_name: "Test Step".to_string(),
                description: None,
                timeout: None,
                max_retries: None,
                parallel_eligible: false,
                dependencies: vec![],
                tags: HashMap::new(),
            },
        };

        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(step));

        orchestrator
            .register_saga_definition(definition)
            .await
            .unwrap();
        let saga_id = orchestrator
            .start_saga("test-saga", HashMap::new())
            .await
            .unwrap();

        let execution = orchestrator.get_execution(&saga_id).await.unwrap().unwrap();
        recovery_manager
            .create_saga_checkpoint(&execution)
            .await
            .unwrap();

        let stats = recovery_manager.get_recovery_stats().await;
        assert_eq!(stats.total_checkpoints, 1);
    }
}
