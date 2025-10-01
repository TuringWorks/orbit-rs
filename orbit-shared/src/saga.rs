use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::transaction_log::PersistentTransactionLogger;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unique identifier for a saga instance
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SagaId {
    pub id: String,
    pub coordinator_node: NodeId,
}

impl SagaId {
    pub fn new(coordinator_node: NodeId) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            coordinator_node,
        }
    }
}

impl fmt::Display for SagaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.coordinator_node)
    }
}

/// Current state of a saga execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SagaState {
    /// Saga is being initialized
    Initializing,
    /// Saga is executing forward steps
    Executing,
    /// Saga completed successfully
    Completed,
    /// Saga failed and is compensating
    Compensating,
    /// Saga was aborted after compensation
    Aborted,
    /// Saga failed and could not be compensated
    Failed,
    /// Saga is paused (can be resumed)
    Paused,
}

/// Result of executing a saga step
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepResult {
    /// Step completed successfully
    Success,
    /// Step failed with retriable error
    RetryableFailure(String),
    /// Step failed with non-retriable error
    PermanentFailure(String),
    /// Step was skipped (conditional execution)
    Skipped,
}

/// Configuration for saga execution
#[derive(Debug, Clone)]
pub struct SagaConfig {
    /// Maximum execution time for the entire saga
    pub max_execution_time: Duration,
    /// Default timeout for individual steps
    pub default_step_timeout: Duration,
    /// Maximum retry attempts per step
    pub max_retry_attempts: u32,
    /// Retry backoff strategy
    pub retry_backoff_initial: Duration,
    pub retry_backoff_multiplier: f64,
    /// Enable parallel execution where possible
    pub enable_parallel_execution: bool,
    /// Checkpoint interval for state persistence
    pub checkpoint_interval: Duration,
    /// Maximum compensation time
    pub max_compensation_time: Duration,
}

impl Default for SagaConfig {
    fn default() -> Self {
        Self {
            max_execution_time: Duration::from_secs(3600), // 1 hour
            default_step_timeout: Duration::from_secs(300), // 5 minutes
            max_retry_attempts: 3,
            retry_backoff_initial: Duration::from_millis(100),
            retry_backoff_multiplier: 2.0,
            enable_parallel_execution: true,
            checkpoint_interval: Duration::from_secs(30),
            max_compensation_time: Duration::from_secs(1800), // 30 minutes
        }
    }
}

/// Metadata for a saga step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStepMetadata {
    /// Unique step identifier
    pub step_id: String,
    /// Human-readable step name
    pub step_name: String,
    /// Step description
    pub description: Option<String>,
    /// Step timeout (overrides default)
    pub timeout: Option<Duration>,
    /// Maximum retry attempts for this step
    pub max_retries: Option<u32>,
    /// Whether this step can be executed in parallel with others
    pub parallel_eligible: bool,
    /// Dependencies that must complete before this step
    pub dependencies: Vec<String>,
    /// Tags for categorization and monitoring
    pub tags: HashMap<String, String>,
}

/// A saga step definition with forward and compensation actions
#[async_trait]
pub trait SagaStep: Send + Sync {
    /// Execute the forward action of this step
    async fn execute(&self, context: &SagaContext) -> OrbitResult<StepResult>;

    /// Execute the compensation action for this step
    async fn compensate(&self, context: &SagaContext) -> OrbitResult<StepResult>;

    /// Get metadata for this step
    fn metadata(&self) -> &SagaStepMetadata;

    /// Check if this step should be executed based on context
    async fn should_execute(&self, context: &SagaContext) -> OrbitResult<bool> {
        let _ = context;
        Ok(true) // Default implementation: always execute
    }

    /// Prepare the step for execution (resource allocation, validation, etc.)
    async fn prepare(&self, context: &SagaContext) -> OrbitResult<()> {
        let _ = context;
        Ok(()) // Default implementation: no preparation needed
    }

    /// Clean up after step execution (resource deallocation, etc.)
    async fn cleanup(&self, context: &SagaContext) -> OrbitResult<()> {
        let _ = context;
        Ok(()) // Default implementation: no cleanup needed
    }
}

/// Execution context passed to saga steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaContext {
    pub saga_id: SagaId,
    pub execution_id: String,
    pub node_id: NodeId,
    pub data: HashMap<String, serde_json::Value>,
    pub step_results: HashMap<String, StepResult>,
    pub started_at: i64,
    pub current_step: Option<String>,
    pub retry_count: u32,
}

impl SagaContext {
    pub fn new(saga_id: SagaId, node_id: NodeId) -> Self {
        Self {
            saga_id,
            execution_id: Uuid::new_v4().to_string(),
            node_id,
            data: HashMap::new(),
            step_results: HashMap::new(),
            started_at: chrono::Utc::now().timestamp_millis(),
            current_step: None,
            retry_count: 0,
        }
    }

    /// Set data value in the context
    pub fn set_data(&mut self, key: String, value: serde_json::Value) {
        self.data.insert(key, value);
    }

    /// Get data value from the context
    pub fn get_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Get typed data value from the context
    pub fn get_typed_data<T>(&self, key: &str) -> OrbitResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        match self.data.get(key) {
            Some(value) => {
                let typed_value = serde_json::from_value(value.clone()).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize context data: {}", e))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }
}

/// Definition of a complete saga workflow
#[derive(Clone)]
pub struct SagaDefinition {
    pub saga_id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub config: SagaConfig,
    pub steps: Vec<Arc<dyn SagaStep>>,
    pub tags: HashMap<String, String>,
}

impl SagaDefinition {
    pub fn new(name: String, version: String) -> Self {
        Self {
            saga_id: Uuid::new_v4().to_string(),
            name,
            description: None,
            version,
            config: SagaConfig::default(),
            steps: Vec::new(),
            tags: HashMap::new(),
        }
    }

    /// Add a step to the saga definition
    pub fn add_step(mut self, step: Arc<dyn SagaStep>) -> Self {
        self.steps.push(step);
        self
    }

    /// Set saga configuration
    pub fn with_config(mut self, config: SagaConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a tag to the saga
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }

    /// Validate the saga definition
    pub fn validate(&self) -> OrbitResult<()> {
        if self.steps.is_empty() {
            return Err(OrbitError::internal("Saga must have at least one step"));
        }

        // Check for duplicate step IDs
        let mut step_ids = std::collections::HashSet::new();
        for step in &self.steps {
            let step_id = &step.metadata().step_id;
            if !step_ids.insert(step_id.clone()) {
                return Err(OrbitError::internal(format!(
                    "Duplicate step ID: {}",
                    step_id
                )));
            }
        }

        // Validate dependencies
        for step in &self.steps {
            for dep in &step.metadata().dependencies {
                if !step_ids.contains(dep) {
                    return Err(OrbitError::internal(format!(
                        "Step {} has invalid dependency: {}",
                        step.metadata().step_id,
                        dep
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Execution state of a saga instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaExecution {
    pub saga_id: SagaId,
    pub definition_id: String,
    pub execution_id: String,
    pub state: SagaState,
    pub context: SagaContext,
    pub completed_steps: Vec<String>,
    pub failed_steps: Vec<String>,
    pub compensated_steps: Vec<String>,
    pub current_step: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub error_message: Option<String>,
    pub retry_count: u32,
}

impl SagaExecution {
    pub fn new(saga_id: SagaId, definition_id: String, context: SagaContext) -> Self {
        Self {
            saga_id: saga_id.clone(),
            definition_id,
            execution_id: context.execution_id.clone(),
            state: SagaState::Initializing,
            context,
            completed_steps: Vec::new(),
            failed_steps: Vec::new(),
            compensated_steps: Vec::new(),
            current_step: None,
            started_at: chrono::Utc::now().timestamp_millis(),
            completed_at: None,
            error_message: None,
            retry_count: 0,
        }
    }

    /// Mark a step as completed
    pub fn mark_step_completed(&mut self, step_id: String) {
        if !self.completed_steps.contains(&step_id) {
            self.completed_steps.push(step_id);
        }
    }

    /// Mark a step as failed
    pub fn mark_step_failed(&mut self, step_id: String, error: String) {
        if !self.failed_steps.contains(&step_id) {
            self.failed_steps.push(step_id);
        }
        self.error_message = Some(error);
    }

    /// Mark a step as compensated
    pub fn mark_step_compensated(&mut self, step_id: String) {
        if !self.compensated_steps.contains(&step_id) {
            self.compensated_steps.push(step_id);
        }
    }

    /// Check if all steps are completed
    pub fn is_completed(&self) -> bool {
        matches!(self.state, SagaState::Completed)
    }

    /// Check if saga has failed
    pub fn has_failed(&self) -> bool {
        matches!(self.state, SagaState::Failed | SagaState::Aborted)
    }
}

/// Statistics for saga execution
#[derive(Debug, Clone)]
pub struct SagaStats {
    pub total_sagas: u64,
    pub completed_sagas: u64,
    pub failed_sagas: u64,
    pub compensated_sagas: u64,
    pub active_sagas: u64,
    pub average_execution_time_ms: f64,
    pub total_steps_executed: u64,
    pub total_steps_compensated: u64,
}

/// Event handler for saga lifecycle events
#[async_trait]
pub trait SagaEventHandler: Send + Sync {
    /// Called when a saga starts
    async fn on_saga_started(&self, execution: &SagaExecution) -> OrbitResult<()>;

    /// Called when a saga completes successfully
    async fn on_saga_completed(&self, execution: &SagaExecution) -> OrbitResult<()>;

    /// Called when a saga fails
    async fn on_saga_failed(&self, execution: &SagaExecution) -> OrbitResult<()>;

    /// Called when a step starts
    async fn on_step_started(&self, execution: &SagaExecution, step_id: &str) -> OrbitResult<()>;

    /// Called when a step completes
    async fn on_step_completed(
        &self,
        execution: &SagaExecution,
        step_id: &str,
        result: &StepResult,
    ) -> OrbitResult<()>;

    /// Called when compensation starts
    async fn on_compensation_started(&self, execution: &SagaExecution) -> OrbitResult<()>;

    /// Called when compensation completes
    async fn on_compensation_completed(&self, execution: &SagaExecution) -> OrbitResult<()>;
}

/// The main saga orchestrator that manages saga execution
pub struct SagaOrchestrator {
    node_id: NodeId,
    definitions: Arc<RwLock<HashMap<String, SagaDefinition>>>,
    executions: Arc<RwLock<HashMap<SagaId, SagaExecution>>>,
    logger: Arc<dyn PersistentTransactionLogger>,
    event_handlers: Arc<RwLock<Vec<Arc<dyn SagaEventHandler>>>>,
    execution_queue: Arc<Mutex<VecDeque<SagaId>>>,
    stats: Arc<RwLock<SagaStats>>,
}

impl SagaOrchestrator {
    pub fn new(node_id: NodeId, logger: Arc<dyn PersistentTransactionLogger>) -> Self {
        Self {
            node_id,
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
            logger,
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            execution_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(SagaStats {
                total_sagas: 0,
                completed_sagas: 0,
                failed_sagas: 0,
                compensated_sagas: 0,
                active_sagas: 0,
                average_execution_time_ms: 0.0,
                total_steps_executed: 0,
                total_steps_compensated: 0,
            })),
        }
    }

    /// Register a saga definition
    pub async fn register_saga_definition(&self, definition: SagaDefinition) -> OrbitResult<()> {
        definition.validate()?;

        let definition_name = definition.name.clone();
        let mut definitions = self.definitions.write().await;
        definitions.insert(definition_name.clone(), definition);

        info!("Registered saga definition: {}", definition_name);
        Ok(())
    }

    /// Add an event handler
    pub async fn add_event_handler(&self, handler: Arc<dyn SagaEventHandler>) {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(handler);
    }

    /// Start a saga execution
    pub async fn start_saga(
        &self,
        definition_id: &str,
        initial_context: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<SagaId> {
        let _definition = {
            let definitions = self.definitions.read().await;
            definitions
                .get(definition_id)
                .ok_or_else(|| {
                    OrbitError::internal(format!("Saga definition not found: {}", definition_id))
                })?
                .clone()
        };

        let saga_id = SagaId::new(self.node_id.clone());
        let mut context = SagaContext::new(saga_id.clone(), self.node_id.clone());

        // Set initial context data
        for (key, value) in initial_context {
            context.set_data(key, value);
        }

        let execution = SagaExecution::new(saga_id.clone(), definition_id.to_string(), context);

        // Store execution
        {
            let mut executions = self.executions.write().await;
            executions.insert(saga_id.clone(), execution.clone());
        }

        // Add to execution queue
        {
            let mut queue = self.execution_queue.lock().await;
            queue.push_back(saga_id.clone());
        }

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_saga_started(&execution).await {
                error!("Saga event handler failed: {}", e);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_sagas += 1;
            stats.active_sagas += 1;
        }

        info!("Started saga execution: {}", saga_id);
        Ok(saga_id)
    }

    /// Execute a saga step by step
    pub async fn execute_saga(&self, saga_id: &SagaId) -> OrbitResult<SagaExecution> {
        let (definition, mut execution) = {
            let definitions = self.definitions.read().await;
            let executions = self.executions.read().await;

            let execution = executions
                .get(saga_id)
                .ok_or_else(|| OrbitError::internal("Saga execution not found"))?
                .clone();

            let definition = definitions
                .get(&execution.definition_id)
                .ok_or_else(|| OrbitError::internal("Saga definition not found"))?
                .clone();

            (definition, execution)
        };

        execution.state = SagaState::Executing;
        self.update_execution(&execution).await?;

        info!("Executing saga: {}", saga_id);

        // Execute steps in sequence (parallel execution would be more complex)
        for step in &definition.steps {
            let step_id = step.metadata().step_id.clone();
            execution.current_step = Some(step_id.clone());
            execution.context.current_step = Some(step_id.clone());

            // Notify event handlers
            let handlers = self.event_handlers.read().await;
            for handler in handlers.iter() {
                if let Err(e) = handler.on_step_started(&execution, &step_id).await {
                    error!("Saga event handler failed: {}", e);
                }
            }

            debug!("Executing step: {}", step_id);

            // Check if step should be executed
            let should_execute = step.should_execute(&execution.context).await?;
            if !should_execute {
                debug!("Skipping step: {}", step_id);
                continue;
            }

            // Prepare step
            step.prepare(&execution.context).await?;

            // Execute step with retries
            let result = self
                .execute_step_with_retries(step.as_ref(), &execution.context, &definition.config)
                .await;

            match result {
                Ok(StepResult::Success) => {
                    execution.mark_step_completed(step_id.clone());
                    execution
                        .context
                        .step_results
                        .insert(step_id.clone(), StepResult::Success);

                    // Update statistics
                    {
                        let mut stats = self.stats.write().await;
                        stats.total_steps_executed += 1;
                    }

                    // Notify event handlers
                    for handler in handlers.iter() {
                        if let Err(e) = handler
                            .on_step_completed(&execution, &step_id, &StepResult::Success)
                            .await
                        {
                            error!("Saga event handler failed: {}", e);
                        }
                    }

                    debug!("Step completed successfully: {}", step_id);
                }
                Ok(StepResult::Skipped) => {
                    debug!("Step skipped: {}", step_id);
                    continue;
                }
                Ok(StepResult::RetryableFailure(msg)) | Ok(StepResult::PermanentFailure(msg)) => {
                    let error_msg = msg;

                    execution.mark_step_failed(step_id.clone(), error_msg.clone());
                    execution.state = SagaState::Compensating;

                    warn!("Step failed: {} - {}", step_id, error_msg);

                    // Start compensation process
                    self.compensate_saga(&mut execution, &definition).await?;
                    return Ok(execution);
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    execution.mark_step_failed(step_id.clone(), error_msg.clone());
                    execution.state = SagaState::Compensating;

                    warn!("Step failed: {} - {}", step_id, error_msg);

                    // Start compensation process
                    self.compensate_saga(&mut execution, &definition).await?;
                    return Ok(execution);
                }
            }

            // Cleanup step
            step.cleanup(&execution.context).await?;
        }

        // All steps completed successfully
        execution.state = SagaState::Completed;
        execution.completed_at = Some(chrono::Utc::now().timestamp_millis());
        execution.current_step = None;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.completed_sagas += 1;
            stats.active_sagas = stats.active_sagas.saturating_sub(1);
        }

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_saga_completed(&execution).await {
                error!("Saga event handler failed: {}", e);
            }
        }

        info!("Saga completed successfully: {}", saga_id);
        self.update_execution(&execution).await?;

        Ok(execution)
    }

    /// Execute a step with retry logic
    async fn execute_step_with_retries(
        &self,
        step: &dyn SagaStep,
        context: &SagaContext,
        config: &SagaConfig,
    ) -> OrbitResult<StepResult> {
        let max_retries = step
            .metadata()
            .max_retries
            .unwrap_or(config.max_retry_attempts);
        let mut retry_count = 0;
        let mut backoff_delay = config.retry_backoff_initial;

        loop {
            match step.execute(context).await {
                Ok(StepResult::Success) | Ok(StepResult::Skipped) => {
                    return Ok(StepResult::Success);
                }
                Ok(StepResult::PermanentFailure(msg)) => {
                    return Ok(StepResult::PermanentFailure(msg));
                }
                Ok(StepResult::RetryableFailure(msg)) => {
                    if retry_count >= max_retries {
                        return Ok(StepResult::PermanentFailure(format!(
                            "Max retries exceeded: {}",
                            msg
                        )));
                    }

                    retry_count += 1;
                    debug!(
                        "Retrying step {} (attempt {}): {}",
                        step.metadata().step_id,
                        retry_count,
                        msg
                    );

                    tokio::time::sleep(backoff_delay).await;
                    backoff_delay = Duration::from_millis(
                        (backoff_delay.as_millis() as f64 * config.retry_backoff_multiplier) as u64,
                    );
                }
                Err(_) => {
                    if retry_count >= max_retries {
                        return Ok(StepResult::PermanentFailure(
                            "Max retries exceeded".to_string(),
                        ));
                    }

                    retry_count += 1;
                    debug!(
                        "Retrying step {} (attempt {})",
                        step.metadata().step_id,
                        retry_count
                    );

                    tokio::time::sleep(backoff_delay).await;
                    backoff_delay = Duration::from_millis(
                        (backoff_delay.as_millis() as f64 * config.retry_backoff_multiplier) as u64,
                    );
                }
            }
        }
    }

    /// Compensate a failed saga by running compensation actions
    async fn compensate_saga(
        &self,
        execution: &mut SagaExecution,
        definition: &SagaDefinition,
    ) -> OrbitResult<()> {
        info!("Starting compensation for saga: {}", execution.saga_id);

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_compensation_started(execution).await {
                error!("Saga event handler failed: {}", e);
            }
        }

        // Compensate completed steps in reverse order
        let completed_steps = execution.completed_steps.clone();
        for step_id in completed_steps.iter().rev() {
            if let Some(step) = definition
                .steps
                .iter()
                .find(|s| &s.metadata().step_id == step_id)
            {
                debug!("Compensating step: {}", step_id);

                match step.compensate(&execution.context).await {
                    Ok(StepResult::Success) => {
                        execution.mark_step_compensated(step_id.clone());

                        // Update statistics
                        {
                            let mut stats = self.stats.write().await;
                            stats.total_steps_compensated += 1;
                        }

                        debug!("Step compensated successfully: {}", step_id);
                    }
                    Ok(_) | Err(_) => {
                        error!("Failed to compensate step: {}", step_id);
                        execution.state = SagaState::Failed;
                        execution.completed_at = Some(chrono::Utc::now().timestamp_millis());

                        // Update statistics
                        {
                            let mut stats = self.stats.write().await;
                            stats.failed_sagas += 1;
                            stats.active_sagas = stats.active_sagas.saturating_sub(1);
                        }

                        // Notify event handlers
                        for handler in handlers.iter() {
                            if let Err(e) = handler.on_saga_failed(execution).await {
                                error!("Saga event handler failed: {}", e);
                            }
                        }

                        return Ok(());
                    }
                }
            }
        }

        // All compensations completed
        execution.state = SagaState::Aborted;
        execution.completed_at = Some(chrono::Utc::now().timestamp_millis());

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.compensated_sagas += 1;
            stats.active_sagas = stats.active_sagas.saturating_sub(1);
        }

        // Notify event handlers
        for handler in handlers.iter() {
            if let Err(e) = handler.on_compensation_completed(execution).await {
                error!("Saga event handler failed: {}", e);
            }
        }

        info!("Compensation completed for saga: {}", execution.saga_id);
        Ok(())
    }

    /// Update saga execution state
    async fn update_execution(&self, execution: &SagaExecution) -> OrbitResult<()> {
        let mut executions = self.executions.write().await;
        executions.insert(execution.saga_id.clone(), execution.clone());
        Ok(())
    }

    /// Get saga execution by ID
    pub async fn get_execution(&self, saga_id: &SagaId) -> OrbitResult<Option<SagaExecution>> {
        let executions = self.executions.read().await;
        Ok(executions.get(saga_id).cloned())
    }

    /// Get saga statistics
    pub async fn get_stats(&self) -> SagaStats {
        self.stats.read().await.clone()
    }

    /// Start the orchestrator background processing
    pub async fn start(&self) -> OrbitResult<()> {
        info!("Starting saga orchestrator");

        // Start background task for processing saga queue
        let orchestrator = self.clone();
        tokio::spawn(async move {
            orchestrator.process_saga_queue().await;
        });

        Ok(())
    }

    /// Process saga execution queue
    async fn process_saga_queue(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            let saga_id = {
                let mut queue = self.execution_queue.lock().await;
                queue.pop_front()
            };

            if let Some(saga_id) = saga_id {
                if let Err(e) = self.execute_saga(&saga_id).await {
                    error!("Failed to execute saga {}: {}", saga_id, e);
                }
            }
        }
    }
}

impl Clone for SagaOrchestrator {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            definitions: Arc::clone(&self.definitions),
            executions: Arc::clone(&self.executions),
            logger: Arc::clone(&self.logger),
            event_handlers: Arc::clone(&self.event_handlers),
            execution_queue: Arc::clone(&self.execution_queue),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_log::{PersistentLogConfig, SqliteTransactionLogger};
    use tempfile::tempdir;

    // Mock saga step for testing
    struct MockSagaStep {
        metadata: SagaStepMetadata,
        should_succeed: bool,
        should_fail_compensation: bool,
    }

    #[async_trait]
    impl SagaStep for MockSagaStep {
        async fn execute(&self, _context: &SagaContext) -> OrbitResult<StepResult> {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if self.should_succeed {
                Ok(StepResult::Success)
            } else {
                Ok(StepResult::PermanentFailure("Mock failure".to_string()))
            }
        }

        async fn compensate(&self, _context: &SagaContext) -> OrbitResult<StepResult> {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if self.should_fail_compensation {
                Err(OrbitError::internal("Compensation failed"))
            } else {
                Ok(StepResult::Success)
            }
        }

        fn metadata(&self) -> &SagaStepMetadata {
            &self.metadata
        }
    }

    impl MockSagaStep {
        fn new(step_id: String, should_succeed: bool) -> Self {
            Self {
                metadata: SagaStepMetadata {
                    step_id: step_id.clone(),
                    step_name: format!("Mock Step {}", step_id),
                    description: Some("A mock step for testing".to_string()),
                    timeout: None,
                    max_retries: None,
                    parallel_eligible: false,
                    dependencies: Vec::new(),
                    tags: HashMap::new(),
                },
                should_succeed,
                should_fail_compensation: false,
            }
        }
    }

    async fn create_test_orchestrator() -> SagaOrchestrator {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("saga_test.db");

        let log_config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = Arc::new(SqliteTransactionLogger::new(log_config).await.unwrap());
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());

        SagaOrchestrator::new(node_id, logger)
    }

    #[tokio::test]
    async fn test_saga_definition_creation() {
        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(MockSagaStep::new("step1".to_string(), true)))
            .add_step(Arc::new(MockSagaStep::new("step2".to_string(), true)))
            .with_tag("category".to_string(), "test".to_string());

        assert_eq!(definition.name, "test-saga");
        assert_eq!(definition.version, "1.0");
        assert_eq!(definition.steps.len(), 2);
        assert!(definition.tags.contains_key("category"));
    }

    #[tokio::test]
    async fn test_saga_definition_validation() {
        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string());

        // Empty saga should fail validation
        assert!(definition.validate().is_err());

        let definition =
            definition.add_step(Arc::new(MockSagaStep::new("step1".to_string(), true)));

        // Single step saga should pass validation
        assert!(definition.validate().is_ok());
    }

    #[tokio::test]
    async fn test_saga_orchestrator_registration() {
        let orchestrator = create_test_orchestrator().await;

        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(MockSagaStep::new("step1".to_string(), true)));

        let result = orchestrator.register_saga_definition(definition).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_successful_saga_execution() {
        let orchestrator = create_test_orchestrator().await;

        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(MockSagaStep::new("step1".to_string(), true)))
            .add_step(Arc::new(MockSagaStep::new("step2".to_string(), true)));

        orchestrator
            .register_saga_definition(definition)
            .await
            .unwrap();

        let saga_id = orchestrator
            .start_saga("test-saga", HashMap::new())
            .await
            .unwrap();
        let execution = orchestrator.execute_saga(&saga_id).await.unwrap();

        assert!(execution.is_completed());
        assert_eq!(execution.completed_steps.len(), 2);
        assert!(execution.failed_steps.is_empty());
    }

    #[tokio::test]
    async fn test_saga_failure_and_compensation() {
        let orchestrator = create_test_orchestrator().await;

        let definition = SagaDefinition::new("test-saga".to_string(), "1.0".to_string())
            .add_step(Arc::new(MockSagaStep::new("step1".to_string(), true)))
            .add_step(Arc::new(MockSagaStep::new("step2".to_string(), false))); // This step will fail

        orchestrator
            .register_saga_definition(definition)
            .await
            .unwrap();

        let saga_id = orchestrator
            .start_saga("test-saga", HashMap::new())
            .await
            .unwrap();
        let execution = orchestrator.execute_saga(&saga_id).await.unwrap();

        assert!(execution.has_failed());
        assert_eq!(execution.completed_steps.len(), 1); // Only first step completed
        assert_eq!(execution.failed_steps.len(), 1); // Second step failed
        assert_eq!(execution.compensated_steps.len(), 1); // First step was compensated
    }

    #[tokio::test]
    async fn test_saga_context() {
        let saga_id = SagaId::new(NodeId::new("test".to_string(), "default".to_string()));
        let node_id = NodeId::new("test".to_string(), "default".to_string());
        let mut context = SagaContext::new(saga_id, node_id);

        // Test data operations
        context.set_data("key1".to_string(), serde_json::json!("value1"));
        context.set_data("key2".to_string(), serde_json::json!(42));

        assert_eq!(
            context.get_data("key1").unwrap(),
            &serde_json::json!("value1")
        );
        assert_eq!(context.get_data("key2").unwrap(), &serde_json::json!(42));
        assert!(context.get_data("key3").is_none());

        // Test typed data operations
        let string_value: Option<String> = context.get_typed_data("key1").unwrap();
        assert_eq!(string_value, Some("value1".to_string()));

        let int_value: Option<i32> = context.get_typed_data("key2").unwrap();
        assert_eq!(int_value, Some(42));
    }
}
