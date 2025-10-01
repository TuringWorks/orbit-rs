use crate::addressable::AddressableReference;
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::transactions::*;
use async_trait::async_trait;
use futures::future::try_join_all;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{timeout, Instant};
use tonic::transport::{Channel, Endpoint};
use tonic::{Code, Request, Status};
use tracing::{debug, error, info, warn};

// Include the generated protobuf code
pub mod transaction_proto {
    tonic::include_proto!("orbit.transactions");
}

use transaction_proto::{transaction_service_client::TransactionServiceClient, *};

/// Configuration for gRPC transport
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Maximum number of concurrent connections per endpoint
    pub max_connections_per_endpoint: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Option<Duration>,
    /// Keep-alive timeout
    pub keep_alive_timeout: Option<Duration>,
    /// Maximum message size
    pub max_message_size: usize,
    /// Retry attempts for failed requests
    pub retry_attempts: u32,
    /// Retry backoff initial delay
    pub retry_backoff_initial: Duration,
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f64,
    /// Enable TCP keepalive
    pub tcp_keepalive: Option<Duration>,
    /// Enable HTTP2 adaptive window
    pub http2_adaptive_window: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 10,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            keep_alive_interval: Some(Duration::from_secs(30)),
            keep_alive_timeout: Some(Duration::from_secs(10)),
            max_message_size: 16 * 1024 * 1024, // 16MB
            retry_attempts: 3,
            retry_backoff_initial: Duration::from_millis(100),
            retry_backoff_multiplier: 2.0,
            tcp_keepalive: Some(Duration::from_secs(10)),
            http2_adaptive_window: true,
        }
    }
}

/// Connection pool for managing gRPC connections
#[derive(Debug)]
pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<String, TransactionServiceClient<Channel>>>>,
    config: TransportConfig,
    connection_metrics: Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
}

#[derive(Debug, Clone)]
struct ConnectionMetrics {
    created_at: Instant,
    last_used: Instant,
    request_count: u64,
    error_count: u64,
    average_latency_ms: f64,
}

impl ConnectionPool {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
            connection_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a connection to the specified endpoint
    pub async fn get_connection(
        &self,
        endpoint_url: &str,
    ) -> OrbitResult<TransactionServiceClient<Channel>> {
        // Check if connection already exists
        {
            let connections = self.connections.read().await;
            if let Some(client) = connections.get(endpoint_url) {
                // Update last used time
                if let Some(metrics) = self.connection_metrics.write().await.get_mut(endpoint_url) {
                    metrics.last_used = Instant::now();
                }
                return Ok(client.clone());
            }
        }

        // Create new connection
        self.create_connection(endpoint_url).await
    }

    /// Create a new connection to the endpoint
    async fn create_connection(
        &self,
        endpoint_url: &str,
    ) -> OrbitResult<TransactionServiceClient<Channel>> {
        debug!("Creating new gRPC connection to: {}", endpoint_url);

        let mut endpoint = Endpoint::from_shared(endpoint_url.to_string())
            .map_err(|e| OrbitError::internal(&format!("Invalid endpoint URL: {}", e)))?;

        // Configure endpoint
        endpoint = endpoint
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout);

        if let Some(_keep_alive) = self.config.keep_alive_interval {
            endpoint = endpoint.keep_alive_while_idle(true);
            if let Some(timeout) = self.config.keep_alive_timeout {
                endpoint = endpoint.keep_alive_timeout(timeout);
            }
        }

        if let Some(tcp_keepalive) = self.config.tcp_keepalive {
            endpoint = endpoint.tcp_keepalive(Some(tcp_keepalive));
        }

        // Create channel
        let channel = endpoint.connect().await.map_err(|e| {
            OrbitError::network(&format!("Failed to connect to {}: {}", endpoint_url, e))
        })?;

        let client = TransactionServiceClient::new(channel)
            .max_decoding_message_size(self.config.max_message_size)
            .max_encoding_message_size(self.config.max_message_size);

        // Store connection and metrics
        {
            let mut connections = self.connections.write().await;
            connections.insert(endpoint_url.to_string(), client.clone());
        }

        {
            let mut metrics = self.connection_metrics.write().await;
            metrics.insert(
                endpoint_url.to_string(),
                ConnectionMetrics {
                    created_at: Instant::now(),
                    last_used: Instant::now(),
                    request_count: 0,
                    error_count: 0,
                    average_latency_ms: 0.0,
                },
            );
        }

        info!("Created new gRPC connection to: {}", endpoint_url);
        Ok(client)
    }

    /// Update connection metrics
    async fn update_metrics(&self, endpoint_url: &str, latency_ms: f64, success: bool) {
        if let Some(metrics) = self.connection_metrics.write().await.get_mut(endpoint_url) {
            metrics.last_used = Instant::now();
            metrics.request_count += 1;

            if !success {
                metrics.error_count += 1;
            }

            // Update average latency using exponential moving average
            if metrics.request_count == 1 {
                metrics.average_latency_ms = latency_ms;
            } else {
                let alpha = 0.1; // Smoothing factor
                metrics.average_latency_ms =
                    alpha * latency_ms + (1.0 - alpha) * metrics.average_latency_ms;
            }
        }
    }

    /// Clean up idle connections
    pub async fn cleanup_idle_connections(&self, max_idle_time: Duration) -> OrbitResult<()> {
        let now = Instant::now();
        let mut to_remove = Vec::new();
        let max_connection_age = Duration::from_secs(3600); // 1 hour max age

        {
            let metrics = self.connection_metrics.read().await;
            for (endpoint, metric) in metrics.iter() {
                // Remove if idle too long OR too old
                if now.duration_since(metric.last_used) > max_idle_time 
                    || now.duration_since(metric.created_at) > max_connection_age {
                    to_remove.push(endpoint.clone());
                }
            }
        }

        if !to_remove.is_empty() {
            let mut connections = self.connections.write().await;
            let mut metrics = self.connection_metrics.write().await;

            for endpoint in &to_remove {
                connections.remove(endpoint);
                metrics.remove(endpoint);
            }

            info!("Cleaned up {} idle connections", to_remove.len());
        }

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionPoolStats {
        let connections = self.connections.read().await;
        let metrics = self.connection_metrics.read().await;

        let mut total_requests = 0;
        let mut total_errors = 0;
        let mut average_latency = 0.0;

        for metric in metrics.values() {
            total_requests += metric.request_count;
            total_errors += metric.error_count;
            average_latency += metric.average_latency_ms;
        }

        if !metrics.is_empty() {
            average_latency /= metrics.len() as f64;
        }

        ConnectionPoolStats {
            total_connections: connections.len(),
            total_requests,
            total_errors,
            average_latency_ms: average_latency,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionPoolStats {
    pub total_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub average_latency_ms: f64,
}

/// gRPC-based transaction message sender
pub struct GrpcTransactionMessageSender {
    node_id: NodeId,
    connection_pool: Arc<ConnectionPool>,
    node_resolver: Arc<dyn NodeResolver>,
    config: TransportConfig,
}

/// Trait for resolving node IDs to endpoint URLs
#[async_trait]
pub trait NodeResolver: Send + Sync {
    /// Resolve a node ID to its gRPC endpoint URL
    async fn resolve_node(&self, node_id: &NodeId) -> OrbitResult<String>;

    /// Resolve an addressable reference to its hosting node
    async fn resolve_addressable(&self, addressable: &AddressableReference) -> OrbitResult<NodeId>;
}

impl GrpcTransactionMessageSender {
    pub fn new(
        node_id: NodeId,
        node_resolver: Arc<dyn NodeResolver>,
        config: TransportConfig,
    ) -> Self {
        Self {
            node_id,
            connection_pool: Arc::new(ConnectionPool::new(config.clone())),
            node_resolver,
            config,
        }
    }

    /// Send message with retry logic
    async fn send_with_retry<F, Fut, T>(&self, endpoint_url: &str, operation: F) -> OrbitResult<T>
    where
        F: Fn(TransactionServiceClient<Channel>) -> Fut + Clone + Send + Sync,
        Fut: std::future::Future<Output = Result<T, Status>> + Send,
        T: Send,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.retry_attempts {
            let client = self.connection_pool.get_connection(endpoint_url).await?;
            let start_time = Instant::now();

            match timeout(self.config.request_timeout, operation(client)).await {
                Ok(Ok(result)) => {
                    let latency = start_time.elapsed().as_millis() as f64;
                    self.connection_pool
                        .update_metrics(endpoint_url, latency, true)
                        .await;
                    return Ok(result);
                }
                Ok(Err(status)) => {
                    let latency = start_time.elapsed().as_millis() as f64;
                    self.connection_pool
                        .update_metrics(endpoint_url, latency, false)
                        .await;

                    last_error = Some(OrbitError::network(&format!(
                        "gRPC error: {} - {}",
                        status.code(),
                        status.message()
                    )));

                    // Don't retry on certain error types
                    if matches!(
                        status.code(),
                        Code::InvalidArgument | Code::NotFound | Code::PermissionDenied
                    ) {
                        break;
                    }
                }
                Err(_) => {
                    let latency = start_time.elapsed().as_millis() as f64;
                    self.connection_pool
                        .update_metrics(endpoint_url, latency, false)
                        .await;

                    last_error = Some(OrbitError::timeout("gRPC request timeout"));
                }
            }

            // Exponential backoff before retry
            if attempt < self.config.retry_attempts {
                let delay = Duration::from_millis(
                    (self.config.retry_backoff_initial.as_millis() as f64
                        * self.config.retry_backoff_multiplier.powi(attempt as i32))
                        as u64,
                );
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| OrbitError::network("Max retries exceeded")))
    }

    /// Start background maintenance tasks
    pub async fn start_background_tasks(&self) -> OrbitResult<()> {
        let connection_pool = Arc::clone(&self.connection_pool);

        // Connection cleanup task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                if let Err(e) = connection_pool
                    .cleanup_idle_connections(Duration::from_secs(600))
                    .await
                {
                    error!("Connection cleanup failed: {}", e);
                }
            }
        });

        info!("gRPC transport background tasks started");
        Ok(())
    }

    /// Get transport statistics
    pub async fn get_stats(&self) -> ConnectionPoolStats {
        self.connection_pool.get_stats().await
    }
}

#[async_trait]
impl TransactionMessageSender for GrpcTransactionMessageSender {
    async fn send_message(
        &self,
        target: &AddressableReference,
        message: TransactionMessage,
    ) -> OrbitResult<()> {
        // Resolve target to node
        let target_node = self.node_resolver.resolve_addressable(target).await?;
        let endpoint_url = self.node_resolver.resolve_node(&target_node).await?;

        // Convert message to protobuf
        let proto_message = convert_message_to_proto(message)?;

        // Create request
        let request = TransactionMessageRequest {
            sender_node_id: self.node_id.to_string(),
            target_actor_type: target.addressable_type.clone(),
            target_actor_key: extract_key_string(&target.key),
            message: Some(proto_message),
            timeout_ms: self.config.request_timeout.as_millis() as i64,
        };

        // Send message with retry
        let request_clone = request.clone();
        let operation = move |mut client: TransactionServiceClient<Channel>| {
            let req = request_clone.clone();
            async move {
                client
                    .send_transaction_message(Request::new(req))
                    .await
                    .map(|r| r.into_inner())
            }
        };

        let response = self.send_with_retry(&endpoint_url, operation).await?;

        if !response.success {
            return Err(OrbitError::network(&format!(
                "Transaction message failed: {}",
                response.error_message
            )));
        }

        debug!(
            "Sent transaction message to {} ({}ms)",
            target, response.processing_time_ms
        );
        Ok(())
    }

    async fn broadcast_message(
        &self,
        targets: &[AddressableReference],
        message: TransactionMessage,
    ) -> OrbitResult<()> {
        if targets.is_empty() {
            return Ok(());
        }

        // Group targets by node to minimize network calls
        let mut targets_by_node: HashMap<NodeId, Vec<&AddressableReference>> = HashMap::new();

        for target in targets {
            let target_node = self.node_resolver.resolve_addressable(target).await?;
            targets_by_node.entry(target_node).or_default().push(target);
        }

        // Send to each node concurrently
        let send_tasks: Vec<_> = targets_by_node
            .into_iter()
            .map(|(node_id, node_targets)| {
                let message = message.clone();
                let node_resolver = Arc::clone(&self.node_resolver);
                let connection_pool = Arc::clone(&self.connection_pool);
                let config = self.config.clone();
                let sender_node_id = self.node_id.clone();

                async move {
                    let endpoint_url = node_resolver.resolve_node(&node_id).await?;
                    let proto_message = convert_message_to_proto(message)?;

                    let targets_proto: Vec<Target> = node_targets
                        .iter()
                        .map(|target| Target {
                            actor_type: target.addressable_type.clone(),
                            actor_key: extract_key_string(&target.key),
                        })
                        .collect();

                    let request = BroadcastMessageRequest {
                        sender_node_id: sender_node_id.to_string(),
                        targets: targets_proto,
                        message: Some(proto_message),
                        timeout_ms: config.request_timeout.as_millis() as i64,
                    };

                    let client = connection_pool.get_connection(&endpoint_url).await?;
                    let response = timeout(
                        config.request_timeout,
                        client
                            .clone()
                            .broadcast_transaction_message(Request::new(request)),
                    )
                    .await
                    .map_err(|_| OrbitError::timeout("broadcast timeout"))?
                    .map_err(|e| OrbitError::network(&format!("Broadcast failed: {}", e)))?
                    .into_inner();

                    if response.failed_sends > 0 {
                        warn!(
                            "Broadcast had {} failures: {:?}",
                            response.failed_sends, response.errors
                        );
                    }

                    OrbitResult::Ok(response.successful_sends as usize)
                }
            })
            .collect();

        // Execute all broadcasts concurrently
        let results: Vec<_> = try_join_all(send_tasks).await?;
        let total_successful: usize = results.iter().sum();

        info!(
            "Broadcast completed: {} successful sends to {} targets",
            total_successful,
            targets.len()
        );
        Ok(())
    }
}

/// Convert transaction message to protobuf format
fn convert_message_to_proto(message: TransactionMessage) -> OrbitResult<TransactionMessageProto> {
    let message_type = match message {
        TransactionMessage::Prepare {
            transaction_id,
            operations,
            timeout,
        } => transaction_message_proto::MessageType::Prepare(PrepareMessage {
            transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            operations: operations
                .into_iter()
                .map(convert_operation_to_proto)
                .collect(),
            timeout_ms: timeout.as_millis() as i64,
        }),
        TransactionMessage::Vote {
            transaction_id,
            participant,
            vote,
        } => transaction_message_proto::MessageType::Vote(VoteMessage {
            transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            participant_type: participant.addressable_type,
            participant_key: extract_key_string(&participant.key),
            vote: Some(convert_vote_to_proto(vote)),
        }),
        TransactionMessage::Commit { transaction_id } => {
            transaction_message_proto::MessageType::Commit(CommitMessage {
                transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            })
        }
        TransactionMessage::Abort {
            transaction_id,
            reason,
        } => transaction_message_proto::MessageType::Abort(AbortMessage {
            transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            reason,
        }),
        TransactionMessage::Acknowledge {
            transaction_id,
            participant,
            success,
            error,
        } => transaction_message_proto::MessageType::Acknowledge(AckMessage {
            transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            participant_type: participant.addressable_type,
            participant_key: extract_key_string(&participant.key),
            success,
            error,
        }),
        TransactionMessage::QueryStatus { transaction_id } => {
            transaction_message_proto::MessageType::QueryStatus(QueryStatusMessage {
                transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            })
        }
        TransactionMessage::StatusResponse {
            transaction_id,
            state,
        } => transaction_message_proto::MessageType::StatusResponse(StatusResponseMessage {
            transaction_id: Some(convert_transaction_id_to_proto(transaction_id)),
            state: Some(convert_state_to_proto(state)),
        }),
    };

    Ok(TransactionMessageProto {
        message_type: Some(message_type),
    })
}

fn convert_transaction_id_to_proto(id: TransactionId) -> TransactionIdProto {
    TransactionIdProto {
        id: id.id,
        coordinator_node: id.coordinator_node.to_string(),
        created_at: id.created_at,
    }
}

fn convert_operation_to_proto(op: TransactionOperation) -> TransactionOperationProto {
    TransactionOperationProto {
        operation_id: op.operation_id,
        target_actor_type: op.target_actor.addressable_type,
        target_actor_key: extract_key_string(&op.target_actor.key),
        operation_type: op.operation_type,
        operation_data: serde_json::to_string(&op.operation_data).unwrap_or_default(),
        compensation_data: op
            .compensation_data
            .map(|d| serde_json::to_string(&d).unwrap_or_default()),
    }
}

fn convert_vote_to_proto(vote: TransactionVote) -> TransactionVoteProto {
    let (vote_type, reason) = match vote {
        TransactionVote::Yes => (transaction_vote_proto::VoteType::Yes, None),
        TransactionVote::No { reason } => (transaction_vote_proto::VoteType::No, Some(reason)),
        TransactionVote::Uncertain => (transaction_vote_proto::VoteType::Uncertain, None),
    };

    TransactionVoteProto {
        vote_type: vote_type as i32,
        reason,
    }
}

fn convert_state_to_proto(state: TransactionState) -> TransactionStateProto {
    let (state_type, details) = match state {
        TransactionState::Preparing => (transaction_state_proto::StateType::Preparing, None),
        TransactionState::Prepared => (transaction_state_proto::StateType::Prepared, None),
        TransactionState::Committing => (transaction_state_proto::StateType::Committing, None),
        TransactionState::Committed => (transaction_state_proto::StateType::Committed, None),
        TransactionState::Aborting => (transaction_state_proto::StateType::Aborting, None),
        TransactionState::Aborted => (transaction_state_proto::StateType::Aborted, None),
        TransactionState::TimedOut => (transaction_state_proto::StateType::TimedOut, None),
        TransactionState::Failed { reason } => {
            (transaction_state_proto::StateType::Failed, Some(reason))
        }
    };

    TransactionStateProto {
        state_type: state_type as i32,
        details,
    }
}

fn extract_key_string(key: &crate::addressable::Key) -> String {
    match key {
        crate::addressable::Key::StringKey { key } => key.clone(),
        crate::addressable::Key::Int32Key { key } => key.to_string(),
        crate::addressable::Key::Int64Key { key } => key.to_string(),
        crate::addressable::Key::NoKey => "no-key".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressable::Key;

    // Mock node resolver for testing
    struct MockNodeResolver;

    #[async_trait]
    impl NodeResolver for MockNodeResolver {
        async fn resolve_node(&self, node_id: &NodeId) -> OrbitResult<String> {
            Ok(format!("http://{}:8080", node_id.key))
        }

        async fn resolve_addressable(
            &self,
            _addressable: &AddressableReference,
        ) -> OrbitResult<NodeId> {
            Ok(NodeId::new("test-node".to_string(), "default".to_string()))
        }
    }

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let config = TransportConfig::default();
        let pool = ConnectionPool::new(config);

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_message_conversion() {
        let transaction_id =
            TransactionId::new(NodeId::new("test".to_string(), "default".to_string()));
        let message = TransactionMessage::Commit {
            transaction_id: transaction_id.clone(),
        };

        let proto = convert_message_to_proto(message).unwrap();
        assert!(proto.message_type.is_some());

        if let Some(transaction_message_proto::MessageType::Commit(commit)) = proto.message_type {
            assert!(commit.transaction_id.is_some());
            assert_eq!(commit.transaction_id.unwrap().id, transaction_id.id);
        } else {
            panic!("Expected commit message");
        }
    }

    #[test]
    fn test_key_extraction() {
        let string_key = Key::StringKey {
            key: "test".to_string(),
        };
        assert_eq!(extract_key_string(&string_key), "test");

        let int_key = Key::Int32Key { key: 42 };
        assert_eq!(extract_key_string(&int_key), "42");

        let no_key = Key::NoKey;
        assert_eq!(extract_key_string(&no_key), "no-key");
    }
}
