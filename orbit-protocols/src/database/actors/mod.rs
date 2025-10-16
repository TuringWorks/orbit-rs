//! Database Actors Module
//!
//! This module contains the actor implementations for PostgreSQL, pgvector, and TimescaleDB
//! database operations. Each actor provides specialized functionality while maintaining
//! a consistent interface for database operations.

use super::*;
use crate::database::messages::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use orbit_shared::addressable::Addressable;
use orbit_shared::exception::{OrbitError, OrbitResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub mod postgres;
pub mod timescale;
pub mod vector;

pub use postgres::PostgresActor;
pub use timescale::TimescaleActor;
pub use vector::VectorDatabaseActor;

/// Common trait for all database actors
#[async_trait]
pub trait DatabaseActor: Addressable + Send + Sync {
    /// Initialize the database connection and resources
    async fn initialize(&mut self, config: DatabaseConfig) -> OrbitResult<()>;

    /// Shutdown the actor gracefully
    async fn shutdown(&mut self) -> OrbitResult<()>;

    /// Check if the actor is healthy and ready to handle requests
    async fn is_healthy(&self) -> bool;

    /// Get current performance metrics
    async fn get_metrics(&self) -> DatabaseMetrics;

    /// Handle a database message
    async fn handle_message(&mut self, message: DatabaseMessage) -> OrbitResult<DatabaseResponse>;

    /// Execute a raw SQL query
    async fn execute_sql(
        &mut self,
        sql: &str,
        parameters: Vec<JsonValue>,
    ) -> OrbitResult<QueryResponse>;

    /// Test database connectivity
    async fn test_connection(&self) -> OrbitResult<bool>;
}

/// Base actor state shared by all database actors
#[derive(Debug)]
pub struct BaseActorState {
    pub actor_id: String,
    pub config: DatabaseConfig,
    pub metrics: Arc<RwLock<DatabaseMetrics>>,
    pub is_initialized: bool,
    pub created_at: DateTime<Utc>,
    pub last_activity: Arc<RwLock<DateTime<Utc>>>,
    pub active_transactions: Arc<RwLock<HashMap<String, TransactionInfo>>>,
    pub connection_pool_status: Arc<RwLock<ConnectionPoolStatus>>,
    pub query_cache: Arc<RwLock<HashMap<String, CachedQuery>>>,
}

/// Transaction information
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub transaction_id: String,
    pub isolation_level: IsolationLevel,
    pub access_mode: TransactionAccessMode,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub savepoints: Vec<String>,
    pub is_readonly: bool,
}

/// Connection pool status information
#[derive(Debug, Clone)]
pub struct ConnectionPoolStatus {
    pub total_connections: u32,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub pending_requests: u32,
    pub max_connections: u32,
    pub last_error: Option<String>,
    pub last_error_time: Option<DateTime<Utc>>,
}

/// Cached query information
#[derive(Debug, Clone)]
pub struct CachedQuery {
    pub query_hash: String,
    pub query_plan: String,
    pub execution_count: u64,
    pub average_duration_ms: f64,
    pub last_executed: DateTime<Utc>,
    pub cached_at: DateTime<Utc>,
}

impl BaseActorState {
    /// Create a new base actor state
    pub fn new(actor_id: String, config: DatabaseConfig) -> Self {
        let now = Utc::now();
        let max_connections = config.pool.max_connections;

        Self {
            actor_id,
            config,
            metrics: Arc::new(RwLock::new(DatabaseMetrics {
                pool_size: 0,
                active_connections: 0,
                idle_connections: 0,
                pending_requests: 0,
                queries_per_second: 0.0,
                average_query_time_ms: 0.0,
                slow_queries_count: 0,
                active_transactions: 0,
                committed_transactions: 0,
                rollback_count: 0,
                error_rate: 0.0,
                last_error: None,
                cache_hit_ratio: 0.0,
                cache_size_bytes: 0,
                vector_searches_per_second: None,
                average_vector_search_time_ms: None,
                chunks_created: None,
                compression_ratio: None,
                hypertables_count: None,
            })),
            is_initialized: false,
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            connection_pool_status: Arc::new(RwLock::new(ConnectionPoolStatus {
                total_connections: 0,
                active_connections: 0,
                idle_connections: 0,
                pending_requests: 0,
                max_connections,
                last_error: None,
                last_error_time: None,
            })),
            query_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update last activity timestamp
    pub async fn update_activity(&self) {
        let mut last_activity = self.last_activity.write().await;
        *last_activity = Utc::now();
    }

    /// Get current metrics
    pub async fn get_current_metrics(&self) -> DatabaseMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Update metrics
    pub async fn update_metrics<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut DatabaseMetrics),
    {
        let mut metrics = self.metrics.write().await;
        update_fn(&mut *metrics);
    }

    /// Start a new transaction
    pub async fn start_transaction(
        &self,
        isolation_level: IsolationLevel,
        access_mode: TransactionAccessMode,
    ) -> String {
        let transaction_id = utils::generate_transaction_id();
        let now = Utc::now();

        let transaction_info = TransactionInfo {
            transaction_id: transaction_id.clone(),
            isolation_level,
            access_mode: access_mode.clone(),
            started_at: now,
            last_activity: now,
            savepoints: Vec::new(),
            is_readonly: matches!(access_mode, TransactionAccessMode::ReadOnly),
        };

        let mut transactions = self.active_transactions.write().await;
        transactions.insert(transaction_id.clone(), transaction_info);

        // Update metrics
        self.update_metrics(|metrics| {
            metrics.active_transactions = transactions.len() as u32;
        })
        .await;

        transaction_id
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        let mut transactions = self.active_transactions.write().await;

        if let Some(_transaction_info) = transactions.remove(transaction_id) {
            // Update metrics
            self.update_metrics(|metrics| {
                metrics.active_transactions = transactions.len() as u32;
                metrics.committed_transactions += 1;
            })
            .await;

            debug!("Transaction {} committed successfully", transaction_id);
            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(
        &self,
        transaction_id: &str,
        reason: &str,
    ) -> OrbitResult<()> {
        let mut transactions = self.active_transactions.write().await;

        if let Some(_transaction_info) = transactions.remove(transaction_id) {
            // Update metrics
            self.update_metrics(|metrics| {
                metrics.active_transactions = transactions.len() as u32;
                metrics.rollback_count += 1;
            })
            .await;

            warn!("Transaction {} rolled back: {}", transaction_id, reason);
            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Add a savepoint to a transaction
    pub async fn add_savepoint(
        &self,
        transaction_id: &str,
        savepoint_name: String,
    ) -> OrbitResult<()> {
        let mut transactions = self.active_transactions.write().await;

        if let Some(transaction_info) = transactions.get_mut(transaction_id) {
            transaction_info.savepoints.push(savepoint_name);
            transaction_info.last_activity = Utc::now();
            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Update connection pool status
    pub async fn update_pool_status<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut ConnectionPoolStatus),
    {
        let mut status = self.connection_pool_status.write().await;
        update_fn(&mut *status);
    }

    /// Cache a query plan
    pub async fn cache_query(&self, query_hash: String, plan: String, duration_ms: f64) {
        let mut cache = self.query_cache.write().await;

        let _cached_query = if let Some(existing) = cache.get_mut(&query_hash) {
            existing.execution_count += 1;
            existing.average_duration_ms = (existing.average_duration_ms
                * (existing.execution_count - 1) as f64
                + duration_ms)
                / existing.execution_count as f64;
            existing.last_executed = Utc::now();
            existing.clone()
        } else {
            let new_cached = CachedQuery {
                query_hash: query_hash.clone(),
                query_plan: plan,
                execution_count: 1,
                average_duration_ms: duration_ms,
                last_executed: Utc::now(),
                cached_at: Utc::now(),
            };
            cache.insert(query_hash, new_cached.clone());
            new_cached
        };

        // Update cache metrics
        let cache_size = cache.len();
        drop(cache);

        self.update_metrics(|metrics| {
            metrics.cache_size_bytes = (cache_size * std::mem::size_of::<CachedQuery>()) as u64;
        })
        .await;
    }

    /// Get a cached query plan
    pub async fn get_cached_query(&self, query_hash: &str) -> Option<CachedQuery> {
        let cache = self.query_cache.read().await;
        cache.get(query_hash).cloned()
    }

    /// Clean up expired transactions
    pub async fn cleanup_expired_transactions(&self, timeout: std::time::Duration) -> usize {
        let mut transactions = self.active_transactions.write().await;
        let now = Utc::now();
        let timeout_duration =
            chrono::Duration::from_std(timeout).unwrap_or(chrono::Duration::hours(1));

        let expired_ids: Vec<String> = transactions
            .iter()
            .filter(|(_, tx)| now.signed_duration_since(tx.last_activity) > timeout_duration)
            .map(|(id, _)| id.clone())
            .collect();

        let expired_count = expired_ids.len();
        for id in expired_ids {
            transactions.remove(&id);
            warn!("Cleaned up expired transaction: {}", id);
        }

        if expired_count > 0 {
            self.update_metrics(|metrics| {
                metrics.active_transactions = transactions.len() as u32;
                metrics.rollback_count += expired_count as u64;
            })
            .await;
        }

        expired_count
    }
}

/// Actor lifecycle management utilities
pub mod lifecycle {
    use super::*;

    /// Handle actor activation
    pub async fn handle_activation<T: DatabaseActor>(
        actor: &mut T,
        config: DatabaseConfig,
    ) -> OrbitResult<()> {
        info!("Activating database actor");

        // Initialize the actor
        actor.initialize(config).await?;

        // Verify health
        if !actor.is_healthy().await {
            return Err(OrbitError::internal(
                "Actor failed health check after initialization",
            ));
        }

        info!("Database actor activated successfully");
        Ok(())
    }

    /// Handle actor deactivation
    pub async fn handle_deactivation<T: DatabaseActor>(actor: &mut T) -> OrbitResult<()> {
        info!("Deactivating database actor");

        // Shutdown gracefully
        actor.shutdown().await?;

        info!("Database actor deactivated successfully");
        Ok(())
    }

    /// Periodic health check for database actors
    pub async fn periodic_health_check<T: DatabaseActor>(actor: &T) -> HealthStatus {
        let mut components = HashMap::new();
        let overall_healthy = actor.is_healthy().await;

        // Check database connection
        let connection_healthy = actor.test_connection().await.unwrap_or(false);
        components.insert(
            "database_connection".to_string(),
            ComponentHealth {
                state: if connection_healthy {
                    HealthState::Healthy
                } else {
                    HealthState::Unhealthy
                },
                message: if connection_healthy {
                    Some("Database connection is healthy".to_string())
                } else {
                    Some("Database connection failed".to_string())
                },
                last_check: Utc::now(),
                metrics: HashMap::new(),
            },
        );

        // Get current metrics for health assessment
        let metrics = actor.get_metrics().await;

        // Check connection pool health
        let pool_healthy = metrics.active_connections > 0 && metrics.error_rate < 0.1;
        components.insert(
            "connection_pool".to_string(),
            ComponentHealth {
                state: if pool_healthy {
                    HealthState::Healthy
                } else {
                    HealthState::Degraded
                },
                message: Some(format!(
                    "Active connections: {}, Error rate: {:.2}%",
                    metrics.active_connections,
                    metrics.error_rate * 100.0
                )),
                last_check: Utc::now(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert(
                        "active_connections".to_string(),
                        JsonValue::Number(metrics.active_connections.into()),
                    );
                    m.insert(
                        "error_rate".to_string(),
                        JsonValue::Number(
                            serde_json::Number::from_f64(metrics.error_rate).unwrap(),
                        ),
                    );
                    m
                },
            },
        );

        // Check performance metrics
        let performance_healthy = metrics.average_query_time_ms < 1000.0; // < 1 second average
        components.insert(
            "performance".to_string(),
            ComponentHealth {
                state: if performance_healthy {
                    HealthState::Healthy
                } else {
                    HealthState::Degraded
                },
                message: Some(format!(
                    "Average query time: {:.2}ms, QPS: {:.2}",
                    metrics.average_query_time_ms, metrics.queries_per_second
                )),
                last_check: Utc::now(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert(
                        "avg_query_time_ms".to_string(),
                        JsonValue::Number(
                            serde_json::Number::from_f64(metrics.average_query_time_ms).unwrap(),
                        ),
                    );
                    m.insert(
                        "queries_per_second".to_string(),
                        JsonValue::Number(
                            serde_json::Number::from_f64(metrics.queries_per_second).unwrap(),
                        ),
                    );
                    m
                },
            },
        );

        let overall_state =
            if overall_healthy && connection_healthy && pool_healthy && performance_healthy {
                HealthState::Healthy
            } else if connection_healthy {
                HealthState::Degraded
            } else {
                HealthState::Unhealthy
            };

        HealthStatus {
            overall: overall_state,
            components,
            last_check: Utc::now(),
        }
    }
}

/// Message handling utilities
pub mod message_handling {
    use super::*;

    /// Route database messages to appropriate handlers
    pub async fn route_message<T: DatabaseActor>(
        actor: &mut T,
        message: DatabaseMessage,
    ) -> OrbitResult<DatabaseResponse> {
        // Update activity timestamp
        actor.get_metrics().await; // This will update internal state

        match message {
            DatabaseMessage::Query(req) => {
                let response = handle_query_message(actor, req).await?;
                Ok(DatabaseResponse::Query(response))
            }
            DatabaseMessage::Transaction(req) => {
                let response = handle_transaction_message(actor, req).await?;
                Ok(DatabaseResponse::Transaction(response))
            }
            DatabaseMessage::Schema(req) => {
                let response = handle_schema_message(actor, req).await?;
                Ok(DatabaseResponse::Schema(response))
            }
            DatabaseMessage::Connection(req) => {
                let response = handle_connection_message(actor, req).await?;
                Ok(DatabaseResponse::Connection(response))
            }
            DatabaseMessage::Monitoring(req) => {
                let response = handle_monitoring_message(actor, req).await?;
                Ok(DatabaseResponse::Monitoring(response))
            }
            DatabaseMessage::Health(req) => {
                let response = handle_health_message(actor, req).await?;
                Ok(DatabaseResponse::Health(response))
            }
            DatabaseMessage::Configuration(req) => {
                let response = handle_configuration_message(actor, req).await?;
                Ok(DatabaseResponse::Configuration(response))
            }
            // Forward specialized messages to concrete actor implementations
            DatabaseMessage::Vector(_) | DatabaseMessage::Timescale(_) => {
                actor.handle_message(message).await
            }
        }
    }

    async fn handle_query_message<T: DatabaseActor>(
        actor: &mut T,
        request: QueryRequest,
    ) -> OrbitResult<QueryResponse> {
        let start_time = std::time::Instant::now();

        // Execute the SQL query
        let result = actor.execute_sql(&request.sql, request.parameters).await;

        let _duration_ms = start_time.elapsed().as_millis() as u64;

        // Update metrics
        let _metrics = actor.get_metrics().await;
        // Note: In a real implementation, you'd update metrics here

        result
    }

    async fn handle_transaction_message<T: DatabaseActor>(
        _actor: &mut T,
        request: TransactionRequest,
    ) -> OrbitResult<TransactionResponse> {
        // Handle transaction operations
        match request {
            TransactionRequest::Begin {
                isolation_level, ..
            } => {
                let transaction_id = utils::generate_transaction_id();
                Ok(TransactionResponse::Started {
                    transaction_id,
                    isolation_level: isolation_level.unwrap_or(IsolationLevel::ReadCommitted),
                    started_at: Utc::now(),
                })
            }
            TransactionRequest::Commit { transaction_id } => {
                Ok(TransactionResponse::Committed {
                    transaction_id,
                    committed_at: Utc::now(),
                    duration_ms: 0, // TODO: Track actual duration
                })
            }
            TransactionRequest::Rollback { transaction_id, .. } => {
                Ok(TransactionResponse::RolledBack {
                    transaction_id,
                    rolled_back_at: Utc::now(),
                    reason: "User requested rollback".to_string(),
                })
            }
            TransactionRequest::CreateSavepoint {
                transaction_id,
                savepoint_name,
            } => Ok(TransactionResponse::SavepointCreated {
                transaction_id,
                savepoint_name,
            }),
            TransactionRequest::ReleaseSavepoint {
                transaction_id,
                savepoint_name,
            } => Ok(TransactionResponse::SavepointReleased {
                transaction_id,
                savepoint_name,
            }),
            TransactionRequest::SetTransactionSnapshot {
                transaction_id,
                snapshot_id,
            } => Ok(TransactionResponse::SnapshotSet {
                transaction_id,
                snapshot_id,
            }),
        }
    }

    async fn handle_schema_message<T: DatabaseActor>(
        _actor: &mut T,
        _request: SchemaRequest,
    ) -> OrbitResult<SchemaResponse> {
        // Placeholder implementation
        Err(OrbitError::internal(
            "Schema operations not yet implemented",
        ))
    }

    async fn handle_connection_message<T: DatabaseActor>(
        actor: &mut T,
        request: ConnectionRequest,
    ) -> OrbitResult<ConnectionResponse> {
        match request {
            ConnectionRequest::TestConnection => {
                let is_connected = actor.test_connection().await.unwrap_or(false);
                if is_connected {
                    Ok(ConnectionResponse::ConnectionOk {
                        database: "orbit".to_string(),
                        version: "PostgreSQL 15.0".to_string(),
                        connection_count: 1,
                    })
                } else {
                    Err(OrbitError::internal("Database connection test failed"))
                }
            }
            ConnectionRequest::GetConnectionStatus => Ok(ConnectionResponse::ConnectionStatus {
                status: ConnectionStatus {
                    database: "orbit".to_string(),
                    host: "localhost".to_string(),
                    port: 5432,
                    username: "postgres".to_string(),
                    connected: actor.test_connection().await.unwrap_or(false),
                    connection_time: Some(Utc::now()),
                    active_connections: 1,
                    idle_connections: 0,
                    max_connections: 20,
                    last_error: None,
                },
            }),
            _ => Err(OrbitError::internal(
                "Connection operation not yet implemented",
            )),
        }
    }

    async fn handle_monitoring_message<T: DatabaseActor>(
        actor: &mut T,
        request: MonitoringRequest,
    ) -> OrbitResult<MonitoringResponse> {
        match request {
            MonitoringRequest::GetMetrics => {
                let metrics = actor.get_metrics().await;
                Ok(MonitoringResponse::Metrics {
                    metrics,
                    collected_at: Utc::now(),
                })
            }
            _ => Err(OrbitError::internal(
                "Monitoring operation not yet implemented",
            )),
        }
    }

    async fn handle_health_message<T: DatabaseActor>(
        actor: &mut T,
        request: HealthRequest,
    ) -> OrbitResult<HealthResponse> {
        match request {
            HealthRequest::CheckHealth => {
                let start_time = std::time::Instant::now();
                let is_healthy = actor.is_healthy().await;
                let duration = start_time.elapsed().as_millis() as u64;

                if is_healthy {
                    Ok(HealthResponse::Healthy {
                        checked_at: Utc::now(),
                        response_time_ms: duration,
                    })
                } else {
                    Err(OrbitError::internal("Health check failed"))
                }
            }
            HealthRequest::GetHealthStatus => {
                let status = lifecycle::periodic_health_check(actor).await;
                Ok(HealthResponse::HealthStatus { status })
            }
            HealthRequest::RunDiagnostics => {
                // Run comprehensive diagnostics
                let start_time = std::time::Instant::now();
                let mut tests = Vec::new();

                // Test database connection
                let connection_test_start = std::time::Instant::now();
                let connection_ok = actor.test_connection().await.unwrap_or(false);
                tests.push(DiagnosticTest {
                    name: "Database Connection".to_string(),
                    category: "Connectivity".to_string(),
                    status: if connection_ok {
                        TestStatus::Passed
                    } else {
                        TestStatus::Failed
                    },
                    message: if connection_ok {
                        Some("Database connection successful".to_string())
                    } else {
                        Some("Database connection failed".to_string())
                    },
                    duration_ms: connection_test_start.elapsed().as_millis() as u64,
                    details: HashMap::new(),
                });

                // Test query execution
                let query_test_start = std::time::Instant::now();
                let query_result = actor.execute_sql("SELECT 1", vec![]).await;
                tests.push(DiagnosticTest {
                    name: "Query Execution".to_string(),
                    category: "Functionality".to_string(),
                    status: if query_result.is_ok() {
                        TestStatus::Passed
                    } else {
                        TestStatus::Failed
                    },
                    message: query_result.as_ref().err().map(|e| e.to_string()),
                    duration_ms: query_test_start.elapsed().as_millis() as u64,
                    details: HashMap::new(),
                });

                let total_duration = start_time.elapsed().as_millis() as u64;
                let tests_passed = tests
                    .iter()
                    .filter(|t| matches!(t.status, TestStatus::Passed))
                    .count() as u32;
                let tests_failed = tests
                    .iter()
                    .filter(|t| matches!(t.status, TestStatus::Failed))
                    .count() as u32;
                let tests_run = tests.len() as u32;

                Ok(HealthResponse::Diagnostics {
                    results: DiagnosticResults {
                        tests_run,
                        tests_passed,
                        tests_failed,
                        tests_skipped: 0,
                        results: tests,
                        run_at: Utc::now(),
                        duration_ms: total_duration,
                    },
                })
            }
        }
    }

    async fn handle_configuration_message<T: DatabaseActor>(
        _actor: &mut T,
        _request: ConfigurationRequest,
    ) -> OrbitResult<ConfigurationResponse> {
        Err(OrbitError::internal(
            "Configuration operations not yet implemented",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_base_actor_state_creation() {
        let config = DatabaseConfig::default();
        let state = BaseActorState::new("test-actor".to_string(), config);

        assert_eq!(state.actor_id, "test-actor");
        assert!(!state.is_initialized);
        assert!(state.active_transactions.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_transaction_management() {
        let config = DatabaseConfig::default();
        let state = BaseActorState::new("test-actor".to_string(), config);

        // Start a transaction
        let tx_id = state
            .start_transaction(
                IsolationLevel::ReadCommitted,
                TransactionAccessMode::ReadWrite,
            )
            .await;

        assert!(!tx_id.is_empty());
        assert_eq!(state.active_transactions.read().await.len(), 1);

        // Commit the transaction
        state.commit_transaction(&tx_id).await.unwrap();
        assert!(state.active_transactions.read().await.is_empty());

        // Check metrics updated
        let metrics = state.get_current_metrics().await;
        assert_eq!(metrics.committed_transactions, 1);
    }

    #[tokio::test]
    async fn test_query_caching() {
        let config = DatabaseConfig::default();
        let state = BaseActorState::new("test-actor".to_string(), config);

        let query_hash = "test_hash".to_string();
        let query_plan = "SELECT 1".to_string();

        // Cache a query
        state
            .cache_query(query_hash.clone(), query_plan.clone(), 150.0)
            .await;

        // Retrieve cached query
        let cached = state.get_cached_query(&query_hash).await;
        assert!(cached.is_some());

        let cached_query = cached.unwrap();
        assert_eq!(cached_query.query_plan, query_plan);
        assert_eq!(cached_query.execution_count, 1);
        assert!((cached_query.average_duration_ms - 150.0).abs() < f64::EPSILON);
    }
}
