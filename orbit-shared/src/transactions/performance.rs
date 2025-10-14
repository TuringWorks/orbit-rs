use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Type alias for batch processor function
type BatchProcessorFn<T> =
    dyn Fn(Vec<T>) -> futures::future::BoxFuture<'static, OrbitResult<Vec<bool>>> + Send + Sync;

/// Configuration for batch processing
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

/// A batched operation
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

/// Batch processor for operations
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
        F: Fn(Vec<T>) -> futures::future::BoxFuture<'static, OrbitResult<Vec<bool>>>
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
    pub async fn add(&self, operation: T) -> OrbitResult<()> {
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
    pub async fn add_priority(&self, operation: T, priority: u8) -> OrbitResult<()> {
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
    async fn process_batch(&self) -> OrbitResult<()> {
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

        debug!(
            "Processed batch of {} operations in {:?}",
            batch_count, batch_duration
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
                        warn!("Batch processing failed: {}", e);
                    }
                }
            }
        });

        info!("Batch processor started");
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

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Minimum number of idle connections
    pub min_idle: usize,
    /// Maximum number of connections
    pub max_size: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Idle timeout before closing connection
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_idle: 2,
            max_size: 10,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(60),
            max_lifetime: Duration::from_secs(3600),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// A pooled connection
#[derive(Debug)]
pub struct PooledConnection<C> {
    connection: C,
    created_at: Instant,
    last_used: Instant,
    use_count: usize,
}

impl<C> PooledConnection<C> {
    pub fn new(connection: C) -> Self {
        let now = Instant::now();
        Self {
            connection,
            created_at: now,
            last_used: now,
            use_count: 0,
        }
    }

    pub fn connection(&mut self) -> &mut C {
        self.last_used = Instant::now();
        self.use_count += 1;
        &mut self.connection
    }

    pub fn is_expired(&self, max_lifetime: Duration, idle_timeout: Duration) -> bool {
        self.created_at.elapsed() > max_lifetime || self.last_used.elapsed() > idle_timeout
    }
}

/// Generic connection pool
pub struct ConnectionPool<C>
where
    C: Send + Sync,
{
    config: ConnectionPoolConfig,
    connections: Arc<Mutex<Vec<PooledConnection<C>>>>,
    semaphore: Arc<Semaphore>,
    factory: Arc<dyn Fn() -> futures::future::BoxFuture<'static, OrbitResult<C>> + Send + Sync>,
    stats: Arc<RwLock<PoolStats>>,
}

impl<C> ConnectionPool<C>
where
    C: Send + Sync + 'static,
{
    pub fn new<F>(config: ConnectionPoolConfig, factory: F) -> Self
    where
        F: Fn() -> futures::future::BoxFuture<'static, OrbitResult<C>> + Send + Sync + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(config.max_size));

        Self {
            config,
            connections: Arc::new(Mutex::new(Vec::new())),
            semaphore,
            factory: Arc::new(factory),
            stats: Arc::new(RwLock::new(PoolStats::default())),
        }
    }

    /// Get a connection from the pool
    pub async fn get(&self) -> OrbitResult<C> {
        // Acquire permit
        let _permit = timeout(self.config.connect_timeout, self.semaphore.acquire())
            .await
            .map_err(|_| OrbitError::timeout("Connection pool timeout"))?
            .map_err(|e| OrbitError::internal(format!("Semaphore error: {e}")))?;

        // Try to get an existing connection
        {
            let mut connections = self.connections.lock().await;

            // Remove expired connections
            connections.retain(|conn| {
                !conn.is_expired(self.config.max_lifetime, self.config.idle_timeout)
            });

            if let Some(mut pooled_conn) = connections.pop() {
                let conn = std::mem::replace(
                    pooled_conn.connection(),
                    unsafe { std::mem::zeroed() }, // Temporary placeholder
                );

                // Update stats
                let mut stats = self.stats.write().await;
                stats.total_acquired += 1;
                stats.current_active += 1;
                stats.current_idle = connections.len();

                // Note: In a real implementation, we'd need a proper way to extract the connection
                // For now, this is a simplified version
                return Ok(conn);
            }
        }

        // Create new connection
        let start = Instant::now();
        let connection = (self.factory)().await?;
        let create_duration = start.elapsed();

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_created += 1;
        stats.total_acquired += 1;
        stats.current_active += 1;
        stats.average_create_time_ms = ((stats.average_create_time_ms
            * (stats.total_created - 1) as f64)
            + create_duration.as_millis() as f64)
            / stats.total_created as f64;

        Ok(connection)
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, connection: C) {
        let mut connections = self.connections.lock().await;

        if connections.len() < self.config.max_size {
            connections.push(PooledConnection::new(connection));

            // Update stats
            let mut stats = self.stats.write().await;
            stats.current_active = stats.current_active.saturating_sub(1);
            stats.current_idle = connections.len();
        }
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        self.stats.read().await.clone()
    }

    /// Start background maintenance tasks
    pub async fn start_maintenance(&self) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(pool.config.health_check_interval);

            loop {
                interval.tick().await;

                // Clean up expired connections
                let mut connections = pool.connections.lock().await;
                let initial_count = connections.len();
                connections.retain(|conn| {
                    !conn.is_expired(pool.config.max_lifetime, pool.config.idle_timeout)
                });
                let removed = initial_count - connections.len();

                if removed > 0 {
                    debug!("Cleaned up {} expired connections", removed);
                    let mut stats = pool.stats.write().await;
                    stats.current_idle = connections.len();
                }
            }
        });

        info!("Connection pool maintenance started");
    }
}

impl<C> Clone for ConnectionPool<C>
where
    C: Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connections: Arc::clone(&self.connections),
            semaphore: Arc::clone(&self.semaphore),
            factory: Arc::clone(&self.factory),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolStats {
    pub total_created: usize,
    pub total_acquired: usize,
    pub current_active: usize,
    pub current_idle: usize,
    pub average_create_time_ms: f64,
}

/// Resource manager for limiting resource usage
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
    pub async fn acquire(&self, memory_estimate: usize) -> OrbitResult<ResourceGuard> {
        // Acquire concurrency permit
        let permit = Arc::clone(&self.concurrency_limiter)
            .acquire_owned()
            .await
            .map_err(|e| OrbitError::internal(format!("Concurrency limiter error: {e}")))?;

        // Check memory availability
        let mut current = self.current_memory.write().await;
        if *current + memory_estimate > self.max_memory {
            return Err(OrbitError::internal("Memory limit exceeded"));
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
}
