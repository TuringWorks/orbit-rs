//! Performance benchmarking suite for transaction and batch processing
//!
//! This module contains performance-related code originally from
//! orbit-shared/src/transactions/performance.rs, moved here to centralize
//! all benchmark-related functionality.

use orbit_shared::LegacyOrbitError;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};

/// Performance benchmark error type
#[derive(Debug, thiserror::Error)]
pub enum PerformanceError {
    #[error("Timeout error: {0}")]
    Timeout(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<LegacyOrbitError> for PerformanceError {
    fn from(err: LegacyOrbitError) -> Self {
        PerformanceError::Internal(err.to_string())
    }
}

pub type PerformanceResult<T> = Result<T, PerformanceError>;

/// Type alias for batch processor function
type BatchProcessorFn<T> = dyn Fn(Vec<T>) -> futures::future::BoxFuture<'static, PerformanceResult<Vec<bool>>>
    + Send
    + Sync;

/// Configuration for batch processing benchmarks
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of operations in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before flushing a batch
    pub max_wait_time: Duration,
    /// Minimum batch size to trigger processing
    pub min_batch_size: usize,
    /// Enable adaptive batch sizing
    pub adaptive_sizing: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_wait_time: Duration::from_millis(10),
            min_batch_size: 10,
            adaptive_sizing: true,
        }
    }
}

/// A batched operation for benchmarking
#[derive(Debug, Clone)]
pub struct BatchedOperation<T> {
    pub operation: T,
    pub timestamp: Instant,
    pub priority: u8,
}

impl<T> BatchedOperation<T> {
    pub fn new(operation: T) -> Self {
        Self {
            operation,
            timestamp: Instant::now(),
            priority: 0,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Batch processor for performance benchmarks
pub struct BatchProcessor<T>
where
    T: Clone + Send + Sync + 'static,
{
    config: BatchConfig,
    queue: Arc<Mutex<VecDeque<BatchedOperation<T>>>>,
    processor: Arc<BatchProcessorFn<T>>,
    stats: Arc<RwLock<BatchStats>>,
}

impl<T> BatchProcessor<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new<F>(config: BatchConfig, processor: F) -> Self
    where
        F: Fn(Vec<T>) -> futures::future::BoxFuture<'static, PerformanceResult<Vec<bool>>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            config,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            processor: Arc::new(processor),
            stats: Arc::new(RwLock::new(BatchStats::default())),
        }
    }

    /// Add an operation to the batch queue
    pub async fn add(&self, operation: T) -> PerformanceResult<()> {
        let batched = BatchedOperation::new(operation);
        let mut queue = self.queue.lock().await;
        queue.push_back(batched);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.current_queue_size = queue.len();

        Ok(())
    }

    /// Add a high-priority operation
    pub async fn add_priority(&self, operation: T, priority: u8) -> PerformanceResult<()> {
        let batched = BatchedOperation::new(operation).with_priority(priority);
        let mut queue = self.queue.lock().await;

        // Insert based on priority
        let pos = queue
            .iter()
            .position(|op| op.priority < priority)
            .unwrap_or(queue.len());
        queue.insert(pos, batched);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.current_queue_size = queue.len();

        Ok(())
    }

    /// Process a batch
    async fn process_batch(&self) -> PerformanceResult<()> {
        let batch_start = Instant::now();

        // Collect batch
        let operations: Vec<T> = {
            let mut queue = self.queue.lock().await;
            let batch_size = self.config.max_batch_size.min(queue.len());

            if batch_size == 0 {
                return Ok(());
            }

            let batch: Vec<T> = queue.drain(0..batch_size).map(|op| op.operation).collect();

            batch
        };

        let batch_count = operations.len();

        // Process batch
        let results = (self.processor)(operations).await?;

        let batch_duration = batch_start.elapsed();

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_batches += 1;
        stats.total_processed += batch_count;
        stats.successful_operations += results.iter().filter(|&&r| r).count();
        stats.average_batch_size = ((stats.average_batch_size * (stats.total_batches - 1) as f64)
            + batch_count as f64)
            / stats.total_batches as f64;
        stats.average_processing_time_ms = ((stats.average_processing_time_ms
            * (stats.total_batches - 1) as f64)
            + batch_duration.as_millis() as f64)
            / stats.total_batches as f64;

        tracing::debug!(
            "Processed batch of {} operations in {:?}",
            batch_count,
            batch_duration
        );

        Ok(())
    }

    /// Start batch processing loop
    pub async fn start(&self) {
        let processor = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(processor.config.max_wait_time);

            loop {
                interval.tick().await;

                let should_process = {
                    let queue = processor.queue.lock().await;
                    queue.len() >= processor.config.min_batch_size
                        || (!queue.is_empty()
                            && queue
                                .front()
                                .map(|op| op.timestamp.elapsed() >= processor.config.max_wait_time)
                                .unwrap_or(false))
                };

                if should_process {
                    if let Err(e) = processor.process_batch().await {
                        tracing::warn!("Batch processing failed: {}", e);
                    }
                }
            }
        });

        tracing::info!("Batch processor started");
    }

    /// Get batch statistics
    pub async fn get_stats(&self) -> BatchStats {
        self.stats.read().await.clone()
    }

    /// Get current queue size
    pub async fn queue_size(&self) -> usize {
        self.queue.lock().await.len()
    }
}

impl<T> Clone for BatchProcessor<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            queue: Arc::clone(&self.queue),
            processor: Arc::clone(&self.processor),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Statistics for batch processing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchStats {
    pub total_operations: usize,
    pub total_batches: usize,
    pub total_processed: usize,
    pub successful_operations: usize,
    pub current_queue_size: usize,
    pub average_batch_size: f64,
    pub average_processing_time_ms: f64,
}

// Re-export connection pool types from orbit-shared for benchmarking
pub use orbit_shared::pooling::{
    AdvancedConnectionPool as ConnectionPool,
    AdvancedPoolConfig as ConnectionPoolConfig,
    ConnectionPoolMetrics as PoolStats,
};

/// Resource manager for limiting resource usage during benchmarks
pub struct ResourceManager {
    /// Maximum memory usage (in bytes)
    max_memory: usize,
    /// Current memory usage estimate
    current_memory: Arc<RwLock<usize>>,
    /// Maximum number of concurrent operations
    max_concurrent: usize,
    /// Current concurrency semaphore
    concurrency_limiter: Arc<Semaphore>,
}

impl ResourceManager {
    pub fn new(max_memory: usize, max_concurrent: usize) -> Self {
        Self {
            max_memory,
            current_memory: Arc::new(RwLock::new(0)),
            max_concurrent,
            concurrency_limiter: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Acquire resources for an operation
    pub async fn acquire(&self, memory_estimate: usize) -> PerformanceResult<ResourceGuard> {
        // Acquire concurrency permit
        let permit = Arc::clone(&self.concurrency_limiter)
            .acquire_owned()
            .await
            .map_err(|e| PerformanceError::Internal(format!("Concurrency limiter error: {}", e)))?;

        // Check memory availability
        let mut current = self.current_memory.write().await;
        if *current + memory_estimate > self.max_memory {
            return Err(PerformanceError::Internal(
                "Memory limit exceeded".to_string(),
            ));
        }

        *current += memory_estimate;

        Ok(ResourceGuard {
            memory_estimate,
            current_memory: Arc::clone(&self.current_memory),
            _permit: permit,
        })
    }

    /// Get current resource usage
    pub async fn current_usage(&self) -> (usize, usize) {
        let memory = *self.current_memory.read().await;
        let concurrent = self.max_concurrent - self.concurrency_limiter.available_permits();
        (memory, concurrent)
    }
}

/// RAII guard for resource usage
pub struct ResourceGuard {
    memory_estimate: usize,
    current_memory: Arc<RwLock<usize>>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        let memory_estimate = self.memory_estimate;
        let current_memory = Arc::clone(&self.current_memory);

        tokio::spawn(async move {
            let mut current = current_memory.write().await;
            *current = current.saturating_sub(memory_estimate);
        });
    }
}

/// Performance benchmark suite for transaction processing
pub struct PerformanceBenchmarkSuite {
    config: PerformanceBenchmarkConfig,
}

/// Configuration for performance benchmarks
#[derive(Debug, Clone)]
pub struct PerformanceBenchmarkConfig {
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub batch_sizes: Vec<usize>,
    pub concurrent_operations: Vec<usize>,
}

impl Default for PerformanceBenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            batch_sizes: vec![10, 50, 100, 500],
            concurrent_operations: vec![1, 5, 10, 20],
        }
    }
}

/// Performance benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarkResult {
    pub name: String,
    pub batch_size: usize,
    pub concurrent_operations: usize,
    pub avg_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub success_rate: f64,
    pub resource_utilization: ResourceUtilization,
}

/// Resource utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub connection_pool_utilization: f64,
}

impl PerformanceBenchmarkSuite {
    pub fn new(config: Option<PerformanceBenchmarkConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }

    /// Run comprehensive performance benchmark suite
    pub async fn run_full_suite(&self) -> PerformanceResult<Vec<PerformanceBenchmarkResult>> {
        let mut results = Vec::new();

        // Batch processing benchmarks
        let batch_results = self.benchmark_batch_processing().await?;
        results.extend(batch_results);

        // Connection pool benchmarks
        let pool_results = self.benchmark_connection_pools().await?;
        results.extend(pool_results);

        // Resource management benchmarks
        let resource_results = self.benchmark_resource_management().await?;
        results.extend(resource_results);

        Ok(results)
    }

    async fn benchmark_batch_processing(
        &self,
    ) -> PerformanceResult<Vec<PerformanceBenchmarkResult>> {
        let mut results = Vec::new();

        for &batch_size in &self.config.batch_sizes {
            let config = BatchConfig {
                max_batch_size: batch_size,
                min_batch_size: batch_size / 2,
                max_wait_time: Duration::from_millis(10),
                adaptive_sizing: false,
            };

            let processor = BatchProcessor::new(config, |operations: Vec<i32>| {
                Box::pin(async move {
                    // Simulate processing time
                    tokio::time::sleep(Duration::from_micros(operations.len() as u64)).await;
                    Ok(vec![true; operations.len()])
                })
            });

            processor.start().await;

            let start = Instant::now();

            // Add operations
            for i in 0..self.config.iterations {
                processor.add(i as i32).await?;
            }

            // Wait for processing to complete
            while processor.queue_size().await > 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            let elapsed = start.elapsed();
            let stats = processor.get_stats().await;

            results.push(PerformanceBenchmarkResult {
                name: format!("BatchProcessing_Size_{}", batch_size),
                batch_size,
                concurrent_operations: 1,
                avg_latency_ms: stats.average_processing_time_ms,
                throughput_ops_per_sec: self.config.iterations as f64 / elapsed.as_secs_f64(),
                success_rate: stats.successful_operations as f64 / stats.total_operations as f64,
                resource_utilization: ResourceUtilization {
                    memory_usage_mb: 10.0,   // Placeholder
                    cpu_usage_percent: 50.0, // Placeholder
                    connection_pool_utilization: 0.0,
                },
            });
        }

        Ok(results)
    }

    async fn benchmark_connection_pools(
        &self,
    ) -> PerformanceResult<Vec<PerformanceBenchmarkResult>> {
        let mut results = Vec::new();

        for &pool_size in &[5, 10, 20] {
            let config = ConnectionPoolConfig {
                max_connections: pool_size,
                min_connections: pool_size / 2,
                ..Default::default()
            };

            let pool = ConnectionPool::new(config, |node_id: String| {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(format!("connection_{}_{}", node_id, uuid::Uuid::new_v4()))
                })
            });

            pool.start_maintenance().await;

            let start = Instant::now();

            // Simulate concurrent connection usage
            let mut handles = Vec::new();
            for _ in 0..self.config.iterations {
                let pool_clone = pool.clone();
                handles.push(tokio::spawn(async move {
                    let _conn = pool_clone.acquire().await?;
                    tokio::time::sleep(Duration::from_micros(100)).await;
                    Ok::<(), PerformanceError>(())
                }));
            }

            // Wait for all operations to complete
            for handle in handles {
                handle
                    .await
                    .map_err(|e| PerformanceError::Internal(e.to_string()))??;
            }

            let elapsed = start.elapsed();
            let metrics = pool.get_metrics().await;

            results.push(PerformanceBenchmarkResult {
                name: format!("ConnectionPool_Size_{}", pool_size),
                batch_size: 1,
                concurrent_operations: pool_size,
                avg_latency_ms: metrics.avg_acquisition_time_ms,
                throughput_ops_per_sec: self.config.iterations as f64 / elapsed.as_secs_f64(),
                success_rate: 1.0, // Simplified
                resource_utilization: ResourceUtilization {
                    memory_usage_mb: 5.0 * pool_size as f64,
                    cpu_usage_percent: 30.0,
                    connection_pool_utilization: metrics.current_active as f64 / pool_size as f64,
                },
            });
        }

        Ok(results)
    }

    async fn benchmark_resource_management(
        &self,
    ) -> PerformanceResult<Vec<PerformanceBenchmarkResult>> {
        let mut results = Vec::new();

        let manager = ResourceManager::new(1024 * 1024 * 100, 50); // 100MB, 50 concurrent

        let start = Instant::now();
        let mut handles = Vec::new();

        for _ in 0..self.config.iterations {
            let manager_clone = &manager;
            handles.push(async move {
                let _guard = manager_clone.acquire(1024).await?;
                tokio::time::sleep(Duration::from_micros(50)).await;
                Ok::<(), PerformanceError>(())
            });
        }

        // Execute all operations
        futures::future::try_join_all(handles).await?;

        let elapsed = start.elapsed();
        let (memory_usage, concurrent_usage) = manager.current_usage().await;

        results.push(PerformanceBenchmarkResult {
            name: "ResourceManagement".to_string(),
            batch_size: 1,
            concurrent_operations: concurrent_usage,
            avg_latency_ms: elapsed.as_millis() as f64 / self.config.iterations as f64,
            throughput_ops_per_sec: self.config.iterations as f64 / elapsed.as_secs_f64(),
            success_rate: 1.0, // Simplified
            resource_utilization: ResourceUtilization {
                memory_usage_mb: memory_usage as f64 / (1024.0 * 1024.0),
                cpu_usage_percent: 40.0, // Placeholder
                connection_pool_utilization: 0.0,
            },
        });

        Ok(results)
    }
}

/// Helper function to run quick performance benchmark
pub async fn quick_performance_benchmark() -> PerformanceResult<Vec<PerformanceBenchmarkResult>> {
    let config = PerformanceBenchmarkConfig {
        iterations: 50,
        warmup_iterations: 5,
        batch_sizes: vec![10, 50],
        concurrent_operations: vec![1, 5],
    };

    let suite = PerformanceBenchmarkSuite::new(Some(config));
    suite.run_full_suite().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_processor() {
        let config = BatchConfig {
            max_batch_size: 10,
            max_wait_time: Duration::from_millis(50),
            min_batch_size: 5,
            adaptive_sizing: false,
        };

        let processor = BatchProcessor::new(config, |operations: Vec<i32>| {
            Box::pin(async move {
                let results = vec![true; operations.len()];
                Ok(results)
            })
        });

        processor.start().await;

        // Add operations
        for i in 0..15 {
            processor.add(i).await.unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = processor.get_stats().await;
        assert!(stats.total_batches > 0);
        assert_eq!(stats.total_operations, 15);
    }

    #[tokio::test]
    async fn test_resource_manager() {
        let manager = ResourceManager::new(1000, 5);

        // Acquire resources
        let _guard1 = manager.acquire(300).await.unwrap();
        let _guard2 = manager.acquire(300).await.unwrap();

        let (memory, concurrent) = manager.current_usage().await;
        assert_eq!(memory, 600);
        assert_eq!(concurrent, 2);

        // Should fail when exceeding memory limit
        let result = manager.acquire(500).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_quick_performance_benchmark() {
        let result = quick_performance_benchmark().await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert!(!results.is_empty());
    }
}
