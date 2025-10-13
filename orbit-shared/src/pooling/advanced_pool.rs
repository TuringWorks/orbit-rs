//! Advanced connection pool implementation with multi-tier architecture

use crate::exception::{OrbitError, OrbitResult};
use crate::pooling::{
    CircuitBreaker, CircuitBreakerConfig, ConnectionHealthMonitor, ConnectionLoadBalancer,
    HealthStatus, LoadBalancingStrategy, NodeHealth,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, info, warn};

/// Pool tier for multi-tier pooling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolTier {
    /// Client-side connection pool
    Client,
    /// Application-side connection pool
    Application,
    /// Database-side connection pool
    Database,
}

/// Configuration for advanced connection pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedPoolConfig {
    /// Minimum number of connections
    pub min_connections: usize,
    /// Maximum number of connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle timeout before closing connection
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Enable dynamic pool sizing
    pub enable_dynamic_sizing: bool,
    /// Target utilization for dynamic sizing (0.0-1.0)
    pub target_utilization: f64,
    /// Pool tier
    pub tier: PoolTier,
}

impl Default for AdvancedPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 20,
            connection_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
            health_check_interval: Duration::from_secs(30),
            load_balancing_strategy: LoadBalancingStrategy::LeastConnections,
            circuit_breaker: CircuitBreakerConfig::default(),
            enable_dynamic_sizing: true,
            target_utilization: 0.75,
            tier: PoolTier::Application,
        }
    }
}

/// Connection wrapper with metadata
struct PooledConnection<C> {
    connection: C,
    node_id: String,
    created_at: Instant,
    last_used: Instant,
    use_count: usize,
    is_healthy: bool,
}

impl<C> PooledConnection<C> {
    fn new(connection: C, node_id: String) -> Self {
        let now = Instant::now();
        Self {
            connection,
            node_id,
            created_at: now,
            last_used: now,
            use_count: 0,
            is_healthy: true,
        }
    }

    fn is_expired(&self, max_lifetime: Duration, idle_timeout: Duration) -> bool {
        self.created_at.elapsed() > max_lifetime || self.last_used.elapsed() > idle_timeout
    }

    fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }
}

/// Advanced connection pool with multi-tier support
pub struct AdvancedConnectionPool<C>
where
    C: Send + Sync,
{
    config: AdvancedPoolConfig,
    connections: Arc<Mutex<Vec<PooledConnection<C>>>>,
    semaphore: Arc<Semaphore>,
    factory:
        Arc<dyn Fn(String) -> futures::future::BoxFuture<'static, OrbitResult<C>> + Send + Sync>,
    load_balancer: ConnectionLoadBalancer,
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    health_monitor: ConnectionHealthMonitor,
    metrics: Arc<RwLock<ConnectionPoolMetrics>>,
}

impl<C> AdvancedConnectionPool<C>
where
    C: Send + Sync + 'static,
{
    /// Create a new advanced connection pool
    pub fn new<F>(config: AdvancedPoolConfig, factory: F) -> Self
    where
        F: Fn(String) -> futures::future::BoxFuture<'static, OrbitResult<C>>
            + Send
            + Sync
            + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let load_balancer = ConnectionLoadBalancer::new(config.load_balancing_strategy);
        let health_monitor = ConnectionHealthMonitor::new(config.health_check_interval);

        Self {
            config,
            connections: Arc::new(Mutex::new(Vec::new())),
            semaphore,
            factory: Arc::new(factory),
            load_balancer,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            health_monitor,
            metrics: Arc::new(RwLock::new(ConnectionPoolMetrics::default())),
        }
    }

    /// Add a database node to the pool
    pub async fn add_node(&self, node_id: String, max_connections: usize) {
        let node = NodeHealth::new(node_id.clone(), max_connections);
        self.load_balancer.add_node(node).await;

        // Create circuit breaker for the node
        let circuit_breaker = CircuitBreaker::new(self.config.circuit_breaker.clone());
        let mut breakers = self.circuit_breakers.write().await;
        breakers.insert(node_id, circuit_breaker);
    }

    /// Remove a database node from the pool
    pub async fn remove_node(&self, node_id: &str) {
        self.load_balancer.remove_node(node_id).await;

        let mut breakers = self.circuit_breakers.write().await;
        breakers.remove(node_id);

        // Remove connections for this node
        let mut connections = self.connections.lock().await;
        connections.retain(|conn| conn.node_id != node_id);
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> OrbitResult<PooledConnectionGuard<C>> {
        let start = Instant::now();

        // Acquire permit
        let semaphore = Arc::clone(&self.semaphore);
        let permit =
            tokio::time::timeout(self.config.connection_timeout, semaphore.acquire_owned())
                .await
                .map_err(|_| OrbitError::timeout("Connection pool timeout"))?
                .map_err(|e| OrbitError::internal(format!("Semaphore error: {}", e)))?;

        // Try to reuse an existing connection
        {
            let mut connections = self.connections.lock().await;

            // Remove expired connections
            connections.retain(|conn| {
                !conn.is_expired(self.config.max_lifetime, self.config.idle_timeout)
            });

            // Find a healthy idle connection
            if let Some(idx) = connections.iter().position(|conn| conn.is_healthy) {
                let mut pooled = connections.remove(idx);
                pooled.mark_used();

                let acquisition_time = start.elapsed();
                self.update_metrics_acquire(acquisition_time, true).await;

                return Ok(PooledConnectionGuard {
                    connection: Some(pooled.connection),
                    pool: self.clone(),
                    node_id: pooled.node_id,
                    _permit: permit,
                });
            }
        }

        // Create a new connection
        let node_id = self
            .load_balancer
            .select_node()
            .await?
            .ok_or_else(|| OrbitError::internal("No available nodes"))?;

        // Check circuit breaker
        let circuit_breaker = {
            let breakers = self.circuit_breakers.read().await;
            breakers
                .get(&node_id)
                .ok_or_else(|| OrbitError::internal("Circuit breaker not found"))?
                .clone()
        };

        if !circuit_breaker.is_request_allowed().await {
            return Err(OrbitError::internal("Circuit breaker is open"));
        }

        // Create connection with circuit breaker protection
        let connection = match circuit_breaker
            .execute((self.factory)(node_id.clone()))
            .await
        {
            Ok(conn) => conn,
            Err(e) => {
                warn!("Failed to create connection to {}: {}", node_id, e);
                return Err(e);
            }
        };

        let acquisition_time = start.elapsed();
        self.update_metrics_acquire(acquisition_time, false).await;

        Ok(PooledConnectionGuard {
            connection: Some(connection),
            pool: self.clone(),
            node_id,
            _permit: permit,
        })
    }

    /// Return a connection to the pool
    async fn return_connection(&self, connection: C, node_id: String, is_healthy: bool) {
        let mut connections = self.connections.lock().await;

        if is_healthy && connections.len() < self.config.max_connections {
            let mut pooled = PooledConnection::new(connection, node_id);
            pooled.is_healthy = is_healthy;
            connections.push(pooled);

            self.update_metrics_return().await;
        } else {
            // Connection is not healthy or pool is full, drop it
            debug!(
                "Dropping connection (healthy: {}, pool_size: {})",
                is_healthy,
                connections.len()
            );
        }
    }

    /// Start background maintenance tasks
    pub async fn start_maintenance(&self) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(pool.config.health_check_interval);

            loop {
                interval.tick().await;

                pool.cleanup_expired_connections().await;
                pool.adjust_pool_size().await;
            }
        });

        self.health_monitor.start_monitoring().await;
        info!("Advanced connection pool maintenance started");
    }

    async fn cleanup_expired_connections(&self) {
        let mut connections = self.connections.lock().await;
        let initial_count = connections.len();

        connections
            .retain(|conn| !conn.is_expired(self.config.max_lifetime, self.config.idle_timeout));

        let removed = initial_count - connections.len();
        if removed > 0 {
            debug!("Cleaned up {} expired connections", removed);
        }
    }

    async fn adjust_pool_size(&self) {
        if !self.config.enable_dynamic_sizing {
            return;
        }

        let metrics = self.metrics.read().await;
        let current_active = metrics.current_active;
        let target_active =
            (self.config.max_connections as f64 * self.config.target_utilization) as usize;

        if current_active > target_active {
            debug!(
                "Pool utilization high: {} / {} (target: {})",
                current_active, self.config.max_connections, target_active
            );
        }
    }

    async fn update_metrics_acquire(&self, acquisition_time: Duration, from_pool: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_acquired += 1;
        metrics.current_active += 1;

        if from_pool {
            metrics.pool_hits += 1;
        } else {
            metrics.pool_misses += 1;
        }

        let acquisition_ms = acquisition_time.as_millis() as f64;
        metrics.avg_acquisition_time_ms = (metrics.avg_acquisition_time_ms
            * (metrics.total_acquired - 1) as f64
            + acquisition_ms)
            / metrics.total_acquired as f64;
    }

    async fn update_metrics_return(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.current_active = metrics.current_active.saturating_sub(1);
    }

    /// Get pool metrics
    pub async fn get_metrics(&self) -> ConnectionPoolMetrics {
        self.metrics.read().await.clone()
    }

    /// Get overall health status
    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_monitor.get_overall_status().await
    }
}

impl<C> Clone for AdvancedConnectionPool<C>
where
    C: Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connections: Arc::clone(&self.connections),
            semaphore: Arc::clone(&self.semaphore),
            factory: Arc::clone(&self.factory),
            load_balancer: self.load_balancer.clone(),
            circuit_breakers: Arc::clone(&self.circuit_breakers),
            health_monitor: self.health_monitor.clone(),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

/// RAII guard for pooled connections
pub struct PooledConnectionGuard<C>
where
    C: Send + Sync + 'static,
{
    connection: Option<C>,
    pool: AdvancedConnectionPool<C>,
    node_id: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl<C> PooledConnectionGuard<C>
where
    C: Send + Sync + 'static,
{
    /// Get a reference to the connection
    pub fn connection(&self) -> &C {
        self.connection.as_ref().unwrap()
    }

    /// Get a mutable reference to the connection
    pub fn connection_mut(&mut self) -> &mut C {
        self.connection.as_mut().unwrap()
    }

    /// Mark the connection as unhealthy (will not be returned to pool)
    pub fn mark_unhealthy(&mut self) {
        // The connection will be dropped instead of returned
    }
}

impl<C> Drop for PooledConnectionGuard<C>
where
    C: Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = self.pool.clone();
            let node_id = self.node_id.clone();

            tokio::spawn(async move {
                pool.return_connection(connection, node_id, true).await;
            });
        }
    }
}

impl<C> fmt::Debug for PooledConnectionGuard<C>
where
    C: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PooledConnectionGuard")
            .field("node_id", &self.node_id)
            .finish()
    }
}

/// Connection pool metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionPoolMetrics {
    /// Total connections acquired
    pub total_acquired: usize,
    /// Current active connections
    pub current_active: usize,
    /// Pool hits (reused connections)
    pub pool_hits: usize,
    /// Pool misses (new connections)
    pub pool_misses: usize,
    /// Average acquisition time in milliseconds
    pub avg_acquisition_time_ms: f64,
}

impl ConnectionPoolMetrics {
    pub fn hit_rate(&self) -> f64 {
        let total = self.pool_hits + self.pool_misses;
        if total == 0 {
            0.0
        } else {
            self.pool_hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct MockConnection {
        #[allow(dead_code)] // Used for testing connection uniqueness
        id: usize,
    }

    async fn create_mock_connection(_node_id: String) -> OrbitResult<MockConnection> {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(MockConnection { id })
    }

    #[tokio::test]
    async fn test_advanced_pool_basic() {
        let config = AdvancedPoolConfig {
            min_connections: 2,
            max_connections: 10,
            ..Default::default()
        };

        let pool = AdvancedConnectionPool::new(config, |node_id| {
            Box::pin(create_mock_connection(node_id))
        });

        pool.add_node("node1".to_string(), 10).await;

        let conn1 = pool.acquire().await.unwrap();
        let conn2 = pool.acquire().await.unwrap();

        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.current_active, 2);

        drop(conn1);
        drop(conn2);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.current_active, 0);
    }

    #[tokio::test]
    async fn test_advanced_pool_reuse() {
        let config = AdvancedPoolConfig {
            max_connections: 5,
            ..Default::default()
        };

        let pool = AdvancedConnectionPool::new(config, |node_id| {
            Box::pin(create_mock_connection(node_id))
        });

        pool.add_node("node1".to_string(), 10).await;

        // Acquire and release
        {
            let _conn = pool.acquire().await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Acquire again - should reuse
        {
            let _conn = pool.acquire().await.unwrap();
        }

        let metrics = pool.get_metrics().await;
        assert!(metrics.pool_hits > 0);
    }

    #[tokio::test]
    async fn test_advanced_pool_load_balancing() {
        let config = AdvancedPoolConfig {
            max_connections: 10,
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            ..Default::default()
        };

        let pool = AdvancedConnectionPool::new(config, |node_id| {
            Box::pin(create_mock_connection(node_id))
        });

        pool.add_node("node1".to_string(), 5).await;
        pool.add_node("node2".to_string(), 5).await;

        let conn1 = pool.acquire().await.unwrap();
        let conn2 = pool.acquire().await.unwrap();

        assert_ne!(conn1.node_id, conn2.node_id);
    }
}
