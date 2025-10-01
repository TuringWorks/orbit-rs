use crate::consensus::{
    AppendEntriesRequest, AppendEntriesResponse, RaftTransport, VoteRequest, VoteResponse,
};
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

/// gRPC service definitions for Raft consensus
pub mod raft_service {
    tonic::include_proto!("orbit.consensus");
}

use raft_service::{
    raft_consensus_client::RaftConsensusClient,
    raft_consensus_server::{RaftConsensus as RaftConsensusService, RaftConsensusServer},
    AppendEntriesRequestProto, AppendEntriesResponseProto, VoteRequestProto, VoteResponseProto,
};

/// gRPC-based Raft transport implementation
pub struct GrpcRaftTransport {
    /// This node's ID
    node_id: NodeId,
    /// Client connections to other nodes
    clients: Arc<RwLock<HashMap<NodeId, RaftConsensusClient<tonic::transport::Channel>>>>,
    /// Node address mapping
    node_addresses: Arc<RwLock<HashMap<NodeId, String>>>,
    /// Connection timeout
    connection_timeout: Duration,
    /// Request timeout
    request_timeout: Duration,
}

impl GrpcRaftTransport {
    pub fn new(
        node_id: NodeId,
        node_addresses: HashMap<NodeId, String>,
        connection_timeout: Duration,
        request_timeout: Duration,
    ) -> Self {
        Self {
            node_id,
            clients: Arc::new(RwLock::new(HashMap::new())),
            node_addresses: Arc::new(RwLock::new(node_addresses)),
            connection_timeout,
            request_timeout,
        }
    }

    /// Get or create a client connection to a node
    async fn get_client(
        &self,
        target_node: &NodeId,
    ) -> OrbitResult<RaftConsensusClient<tonic::transport::Channel>> {
        // Check if we already have a client
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(target_node) {
                return Ok(client.clone());
            }
        }

        // Get node address
        let address = {
            let addresses = self.node_addresses.read().await;
            addresses.get(target_node).cloned().ok_or_else(|| {
                OrbitError::cluster(format!("Address not found for node: {}", target_node))
            })?
        };

        // Create new connection
        debug!("Connecting to node {} at {}", target_node, address);

        let channel = tokio::time::timeout(
            self.connection_timeout,
            tonic::transport::Endpoint::from_shared(address.clone())
                .map_err(|e| OrbitError::internal(&format!("Invalid endpoint: {}", e)))?
                .connect(),
        )
        .await
        .map_err(|_| OrbitError::timeout(&format!("Connection timeout to {}", address)))?
        .map_err(|e| OrbitError::network(&format!("Connection failed to {}: {}", address, e)))?;

        let client = RaftConsensusClient::new(channel);

        // Cache the client
        {
            let mut clients = self.clients.write().await;
            clients.insert(target_node.clone(), client.clone());
        }

        debug!("Connected to node {} at {}", target_node, address);
        Ok(client)
    }

    /// Remove a client connection (on failure)
    async fn remove_client(&self, target_node: &NodeId) {
        let mut clients = self.clients.write().await;
        if clients.remove(target_node).is_some() {
            debug!("Removed client connection to node {}", target_node);
        }
    }

    /// Add or update node address
    pub async fn update_node_address(&self, node_id: NodeId, address: String) {
        let mut addresses = self.node_addresses.write().await;
        addresses.insert(node_id.clone(), address.clone());
        debug!("Updated address for node {} to {}", node_id, address);

        // Remove existing client to force reconnection
        self.remove_client(&node_id).await;
    }
}

#[async_trait]
impl RaftTransport for GrpcRaftTransport {
    async fn send_vote_request(
        &self,
        target: &NodeId,
        request: VoteRequest,
    ) -> OrbitResult<VoteResponse> {
        let mut client = self.get_client(target).await?;

        let proto_request = VoteRequestProto {
            term: request.term,
            candidate_id: request.candidate_id.to_string(),
            last_log_index: request.last_log_index,
            last_log_term: request.last_log_term,
        };

        let grpc_request = Request::new(proto_request);

        let result =
            tokio::time::timeout(self.request_timeout, client.request_vote(grpc_request)).await;

        match result {
            Ok(Ok(response)) => {
                let proto_response = response.into_inner();
                Ok(VoteResponse {
                    term: proto_response.term,
                    vote_granted: proto_response.vote_granted,
                    voter_id: NodeId::from_string(&proto_response.voter_id),
                })
            }
            Ok(Err(status)) => {
                warn!("Vote request to {} failed: {}", target, status);
                self.remove_client(target).await;
                Err(OrbitError::network(&format!(
                    "Vote request failed: {}",
                    status
                )))
            }
            Err(_) => {
                warn!("Vote request to {} timed out", target);
                self.remove_client(target).await;
                Err(OrbitError::timeout(&format!(
                    "Vote request timeout to {}",
                    target
                )))
            }
        }
    }

    async fn send_append_entries(
        &self,
        target: &NodeId,
        request: AppendEntriesRequest,
    ) -> OrbitResult<AppendEntriesResponse> {
        let mut client = self.get_client(target).await?;

        let proto_request = AppendEntriesRequestProto {
            term: request.term,
            leader_id: request.leader_id.to_string(),
            prev_log_index: request.prev_log_index,
            prev_log_term: request.prev_log_term,
            entries: request
                .entries
                .into_iter()
                .map(|entry| raft_service::LogEntryProto {
                    term: entry.term,
                    index: entry.index,
                    command: serde_json::to_string(&entry.command).unwrap_or_default(),
                    timestamp: entry.timestamp,
                })
                .collect(),
            leader_commit: request.leader_commit,
        };

        let grpc_request = Request::new(proto_request);

        let result =
            tokio::time::timeout(self.request_timeout, client.append_entries(grpc_request)).await;

        match result {
            Ok(Ok(response)) => {
                let proto_response = response.into_inner();
                Ok(AppendEntriesResponse {
                    term: proto_response.term,
                    success: proto_response.success,
                    follower_id: NodeId::from_string(&proto_response.follower_id),
                    last_log_index: proto_response.last_log_index,
                })
            }
            Ok(Err(status)) => {
                warn!("Append entries to {} failed: {}", target, status);
                self.remove_client(target).await;
                Err(OrbitError::network(&format!(
                    "Append entries failed: {}",
                    status
                )))
            }
            Err(_) => {
                warn!("Append entries to {} timed out", target);
                self.remove_client(target).await;
                Err(OrbitError::timeout(&format!(
                    "Append entries timeout to {}",
                    target
                )))
            }
        }
    }

    async fn broadcast_heartbeat(
        &self,
        nodes: &[NodeId],
        request: AppendEntriesRequest,
    ) -> OrbitResult<Vec<AppendEntriesResponse>> {
        let tasks: Vec<_> = nodes
            .iter()
            .map(|node| {
                let node = node.clone();
                let request = request.clone();
                let transport = self.clone();

                tokio::spawn(async move {
                    match transport.send_append_entries(&node, request).await {
                        Ok(response) => Some(response),
                        Err(e) => {
                            debug!("Heartbeat to {} failed: {}", node, e);
                            None
                        }
                    }
                })
            })
            .collect();

        let mut responses = Vec::new();
        for task in tasks {
            if let Ok(Some(response)) = task.await {
                responses.push(response);
            }
        }

        Ok(responses)
    }
}

impl Clone for GrpcRaftTransport {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            clients: Arc::clone(&self.clients),
            node_addresses: Arc::clone(&self.node_addresses),
            connection_timeout: self.connection_timeout,
            request_timeout: self.request_timeout,
        }
    }
}

/// gRPC service handler for Raft consensus
pub struct GrpcRaftHandler {
    consensus: Arc<crate::consensus::RaftConsensus>,
}

impl GrpcRaftHandler {
    pub fn new(consensus: Arc<crate::consensus::RaftConsensus>) -> Self {
        Self { consensus }
    }

    /// Create a gRPC server with this handler
    pub fn into_server(self) -> RaftConsensusServer<Self> {
        RaftConsensusServer::new(self)
    }
}

#[async_trait]
impl RaftConsensusService for GrpcRaftHandler {
    async fn request_vote(
        &self,
        request: Request<VoteRequestProto>,
    ) -> Result<Response<VoteResponseProto>, Status> {
        let proto_request = request.into_inner();

        let vote_request = VoteRequest {
            term: proto_request.term,
            candidate_id: NodeId::from_string(&proto_request.candidate_id),
            last_log_index: proto_request.last_log_index,
            last_log_term: proto_request.last_log_term,
        };

        match self.consensus.handle_vote_request(vote_request).await {
            Ok(vote_response) => {
                let proto_response = VoteResponseProto {
                    term: vote_response.term,
                    vote_granted: vote_response.vote_granted,
                    voter_id: vote_response.voter_id.to_string(),
                };
                Ok(Response::new(proto_response))
            }
            Err(e) => {
                error!("Failed to handle vote request: {}", e);
                Err(Status::internal(format!("Vote request failed: {}", e)))
            }
        }
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequestProto>,
    ) -> Result<Response<AppendEntriesResponseProto>, Status> {
        let proto_request = request.into_inner();

        let entries: Vec<crate::consensus::LogEntry> = proto_request
            .entries
            .into_iter()
            .map(|entry| {
                let command = serde_json::from_str(&entry.command)
                    .unwrap_or(crate::consensus::RaftCommand::HeartBeat);

                crate::consensus::LogEntry {
                    term: entry.term,
                    index: entry.index,
                    command,
                    timestamp: entry.timestamp,
                }
            })
            .collect();

        let append_request = AppendEntriesRequest {
            term: proto_request.term,
            leader_id: NodeId::from_string(&proto_request.leader_id),
            prev_log_index: proto_request.prev_log_index,
            prev_log_term: proto_request.prev_log_term,
            entries,
            leader_commit: proto_request.leader_commit,
        };

        match self.consensus.handle_append_entries(append_request).await {
            Ok(append_response) => {
                let proto_response = AppendEntriesResponseProto {
                    term: append_response.term,
                    success: append_response.success,
                    follower_id: append_response.follower_id.to_string(),
                    last_log_index: append_response.last_log_index,
                };
                Ok(Response::new(proto_response))
            }
            Err(e) => {
                error!("Failed to handle append entries: {}", e);
                Err(Status::internal(format!("Append entries failed: {}", e)))
            }
        }
    }
}

/// Helper function to start a Raft gRPC server
pub async fn start_raft_server(
    consensus: Arc<crate::consensus::RaftConsensus>,
    bind_address: &str,
) -> OrbitResult<()> {
    let handler = GrpcRaftHandler::new(consensus);
    let server = handler.into_server();

    let addr = bind_address
        .parse()
        .map_err(|e| OrbitError::configuration(format!("Invalid bind address: {}", e)))?;

    info!("Starting Raft gRPC server on {}", addr);

    tonic::transport::Server::builder()
        .add_service(server)
        .serve(addr)
        .await
        .map_err(|e| OrbitError::network(&format!("Failed to start Raft server: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::{RaftConfig, RaftConsensus};
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_grpc_transport_creation() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let mut node_addresses = HashMap::new();
        node_addresses.insert(
            NodeId::new("peer-1".to_string(), "default".to_string()),
            "http://localhost:50051".to_string(),
        );

        let transport = GrpcRaftTransport::new(
            node_id,
            node_addresses,
            Duration::from_secs(5),
            Duration::from_secs(10),
        );

        // Test address update
        transport
            .update_node_address(
                NodeId::new("peer-2".to_string(), "default".to_string()),
                "http://localhost:50052".to_string(),
            )
            .await;

        let addresses = transport.node_addresses.read().await;
        assert_eq!(addresses.len(), 2);
    }

    #[tokio::test]
    async fn test_raft_handler_creation() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let cluster_nodes = vec![node_id.clone()];
        let consensus = Arc::new(RaftConsensus::new(
            node_id,
            cluster_nodes,
            RaftConfig::default(),
        ));

        let handler = GrpcRaftHandler::new(consensus);
        let _server = handler.into_server();

        // Server created successfully
        assert!(true);
    }
}
