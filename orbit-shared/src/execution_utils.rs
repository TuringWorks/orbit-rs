//! Common execution utilities and patterns to reduce complexity in executor implementations

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// Result type for execution operations
pub type ExecutionResult<T> = Result<T, ExecutionError>;

/// Common execution errors
#[derive(Debug, thiserror::Error, Clone, Serialize, Deserialize)]
pub enum ExecutionError {
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },

    #[error("Execution timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    #[error("Validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Execution context that carries metadata through the execution pipeline
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub request_id: String,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub metadata: HashMap<String, String>,
    pub start_time: std::time::Instant,
}

impl ExecutionContext {
    pub fn new(request_id: String) -> Self {
        Self {
            request_id,
            user_id: None,
            tenant_id: None,
            metadata: HashMap::new(),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_tenant(mut self, tenant_id: String) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

/// Generic command pattern for reducing executor complexity
#[async_trait]
pub trait Command<R> {
    /// Execute the command with the given context
    async fn execute(&self, context: &ExecutionContext) -> ExecutionResult<R>;

    /// Validate the command before execution
    fn validate(&self) -> Result<(), Vec<String>> {
        Ok(())
    }

    /// Get command description for logging/debugging
    fn description(&self) -> &str;
}

/// Command executor that handles common execution patterns
pub struct CommandExecutor {
    timeout_ms: Option<u64>,
    max_concurrent: Option<usize>,
}

impl CommandExecutor {
    pub fn new() -> Self {
        Self {
            timeout_ms: None,
            max_concurrent: None,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn with_concurrency_limit(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent = Some(max_concurrent);
        self
    }

    /// Execute a single command with error handling and timeout
    pub async fn execute_command<R, C>(
        &self,
        command: C,
        context: ExecutionContext,
    ) -> ExecutionResult<R>
    where
        C: Command<R>,
        R: Send + 'static,
    {
        // Validate first
        if let Err(errors) = command.validate() {
            return Err(ExecutionError::ValidationFailed { errors });
        }

        let execution_future = command.execute(&context);

        // Apply timeout if configured
        if let Some(timeout_ms) = self.timeout_ms {
            let timeout_duration = std::time::Duration::from_millis(timeout_ms);

            match tokio::time::timeout(timeout_duration, execution_future).await {
                Ok(result) => result,
                Err(_) => Err(ExecutionError::Timeout { timeout_ms }),
            }
        } else {
            execution_future.await
        }
    }

    /// Execute multiple commands concurrently with optional limits
    pub async fn execute_batch<R, C>(
        &self,
        commands: Vec<C>,
        context: ExecutionContext,
    ) -> Vec<ExecutionResult<R>>
    where
        C: Command<R> + Send + 'static,
        R: Send + 'static,
    {
        let futures: Vec<_> = commands
            .into_iter()
            .map(|cmd| {
                let ctx = context.clone();
                async move { self.execute_command(cmd, ctx).await }
            })
            .collect();

        if let Some(max_concurrent) = self.max_concurrent {
            // Use a semaphore to limit concurrency
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
            let futures: Vec<_> = futures
                .into_iter()
                .map(|future| {
                    let permit = semaphore.clone();
                    async move {
                        let _permit = permit.acquire().await.unwrap();
                        future.await
                    }
                })
                .collect();

            futures::future::join_all(futures).await
        } else {
            futures::future::join_all(futures).await
        }
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Pipeline execution pattern for chaining operations
/// Type alias to simplify complex pipeline step type
type PipelineStep<T> =
    Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = ExecutionResult<T>> + Send>> + Send + Sync>;

pub struct ExecutionPipeline<T> {
    steps: Vec<PipelineStep<T>>,
}

impl<T> ExecutionPipeline<T>
where
    T: Send + 'static,
{
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step<F, Fut>(mut self, step: F) -> Self
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ExecutionResult<T>> + Send + 'static,
    {
        self.steps
            .push(Box::new(move |input| Box::pin(step(input))));
        self
    }

    pub async fn execute(self, initial_input: T) -> ExecutionResult<T> {
        let mut current = initial_input;

        for step in self.steps {
            current = step(current).await?;
        }

        Ok(current)
    }
}

impl<T> Default for ExecutionPipeline<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Execution metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub average_duration_ms: f64,
    pub max_duration_ms: u64,
    pub min_duration_ms: u64,
}

impl ExecutionMetrics {
    pub fn new() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_duration_ms: 0.0,
            max_duration_ms: 0,
            min_duration_ms: u64::MAX,
        }
    }

    pub fn record_execution(&mut self, duration: std::time::Duration, success: bool) {
        let duration_ms = duration.as_millis() as u64;

        self.total_executions += 1;
        if success {
            self.successful_executions += 1;
        } else {
            self.failed_executions += 1;
        }

        self.max_duration_ms = self.max_duration_ms.max(duration_ms);
        self.min_duration_ms = self.min_duration_ms.min(duration_ms);

        // Update rolling average
        let previous_avg = self.average_duration_ms;
        self.average_duration_ms = (previous_avg * (self.total_executions - 1) as f64
            + duration_ms as f64)
            / self.total_executions as f64;
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.total_executions as f64
        }
    }
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommand {
        should_fail: bool,
        delay_ms: u64,
    }

    #[async_trait]
    impl Command<String> for TestCommand {
        async fn execute(&self, _context: &ExecutionContext) -> ExecutionResult<String> {
            if self.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            }

            if self.should_fail {
                Err(ExecutionError::Internal {
                    message: "Test failure".to_string(),
                })
            } else {
                Ok("success".to_string())
            }
        }

        fn description(&self) -> &str {
            "test_command"
        }
    }

    #[tokio::test]
    async fn test_command_execution_success() {
        let executor = CommandExecutor::new();
        let command = TestCommand {
            should_fail: false,
            delay_ms: 0,
        };
        let context = ExecutionContext::new("test-123".to_string());

        let result = executor.execute_command(command, context).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_command_execution_timeout() {
        let executor = CommandExecutor::new().with_timeout(100);
        let command = TestCommand {
            should_fail: false,
            delay_ms: 200,
        };
        let context = ExecutionContext::new("test-123".to_string());

        let result = executor.execute_command(command, context).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::Timeout { timeout_ms } => assert_eq!(timeout_ms, 100),
            _ => panic!("Expected timeout error"),
        }
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        let pipeline = ExecutionPipeline::new()
            .add_step(|input: String| async move { Ok(format!("{}-step1", input)) })
            .add_step(|input: String| async move { Ok(format!("{}-step2", input)) });

        let result = pipeline.execute("initial".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "initial-step1-step2");
    }

    #[test]
    fn test_execution_metrics() {
        let mut metrics = ExecutionMetrics::new();

        metrics.record_execution(std::time::Duration::from_millis(100), true);
        metrics.record_execution(std::time::Duration::from_millis(200), false);

        assert_eq!(metrics.total_executions, 2);
        assert_eq!(metrics.successful_executions, 1);
        assert_eq!(metrics.failed_executions, 1);
        assert_eq!(metrics.success_rate(), 0.5);
        assert_eq!(metrics.average_duration_ms, 150.0);
    }
}
