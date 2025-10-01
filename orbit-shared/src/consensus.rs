use crate::exception::OrbitResult;
use crate::mesh::NodeId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Raft node states
#[derive(Debug, Clone, PartialEq)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

/// Raft log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: RaftCommand,
    pub timestamp: i64,
}

/// Raft commands that can be replicated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftCommand {
    ElectLeader(NodeId),
    HeartBeat,
    CoordinatorAssignment {
        transaction_id: String,
        coordinator: NodeId,
    },
    NodeJoin(NodeId),
    NodeLeave(NodeId),
}

/// Vote request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    pub term: u64,
    pub candidate_id: NodeId,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

/// Vote response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    pub term: u64,
    pub vote_granted: bool,
    pub voter_id: NodeId,
}

/// Append entries request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: NodeId,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

/// Append entries response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
    pub follower_id: NodeId,
    pub last_log_index: u64,
}

/// Raft configuration
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// Election timeout range (randomized)
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    /// Heartbeat interval for leader
    pub heartbeat_interval: Duration,
    /// Maximum entries per append request
    pub max_entries_per_request: usize,
    /// Log compaction threshold
    pub log_compaction_threshold: usize,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            max_entries_per_request: 100,
            log_compaction_threshold: 1000,
        }
    }
}

/// Raft consensus implementation
pub struct RaftConsensus {
    /// Node configuration
    node_id: NodeId,
    cluster_nodes: Arc<RwLock<Vec<NodeId>>>,
    config: RaftConfig,

    /// Raft state
    state: Arc<RwLock<RaftState>>,
    current_term: Arc<RwLock<u64>>,
    voted_for: Arc<RwLock<Option<NodeId>>>,

    /// Log storage
    log: Arc<RwLock<Vec<LogEntry>>>,
    commit_index: Arc<RwLock<u64>>,
    last_applied: Arc<RwLock<u64>>,

    /// Leader state (only used when this node is leader)
    next_index: Arc<RwLock<HashMap<NodeId, u64>>>,
    match_index: Arc<RwLock<HashMap<NodeId, u64>>>,

    /// Election tracking
    last_heartbeat: Arc<RwLock<Instant>>,
    election_timeout: Arc<RwLock<Duration>>,
    votes_received: Arc<RwLock<HashMap<NodeId, bool>>>,

    /// Event handlers
    event_handlers: Arc<RwLock<Vec<Arc<dyn RaftEventHandler>>>>,
}

/// Raft event handler for leadership changes
#[async_trait]
pub trait RaftEventHandler: Send + Sync {
    async fn on_leader_elected(&self, leader_id: &NodeId, term: u64) -> OrbitResult<()>;
    async fn on_leader_lost(&self, former_leader_id: &NodeId, term: u64) -> OrbitResult<()>;
    async fn on_term_changed(&self, old_term: u64, new_term: u64) -> OrbitResult<()>;
}

/// Network transport for Raft messages
#[async_trait]
pub trait RaftTransport: Send + Sync {
    async fn send_vote_request(
        &self,
        target: &NodeId,
        request: VoteRequest,
    ) -> OrbitResult<VoteResponse>;
    async fn send_append_entries(
        &self,
        target: &NodeId,
        request: AppendEntriesRequest,
    ) -> OrbitResult<AppendEntriesResponse>;
    async fn broadcast_heartbeat(
        &self,
        nodes: &[NodeId],
        request: AppendEntriesRequest,
    ) -> OrbitResult<Vec<AppendEntriesResponse>>;
}

impl RaftConsensus {
    pub fn new(node_id: NodeId, cluster_nodes: Vec<NodeId>, config: RaftConfig) -> Self {
        let election_timeout = Self::random_election_timeout(&config);

        Self {
            node_id,
            cluster_nodes: Arc::new(RwLock::new(cluster_nodes)),
            config,
            state: Arc::new(RwLock::new(RaftState::Follower)),
            current_term: Arc::new(RwLock::new(0)),
            voted_for: Arc::new(RwLock::new(None)),
            log: Arc::new(RwLock::new(Vec::new())),
            commit_index: Arc::new(RwLock::new(0)),
            last_applied: Arc::new(RwLock::new(0)),
            next_index: Arc::new(RwLock::new(HashMap::new())),
            match_index: Arc::new(RwLock::new(HashMap::new())),
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
            election_timeout: Arc::new(RwLock::new(election_timeout)),
            votes_received: Arc::new(RwLock::new(HashMap::new())),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn random_election_timeout(config: &RaftConfig) -> Duration {
        let min_ms = config.election_timeout_min.as_millis() as u64;
        let max_ms = config.election_timeout_max.as_millis() as u64;
        let random_ms = min_ms + (fastrand::u64(0..=(max_ms - min_ms)));
        Duration::from_millis(random_ms)
    }

    pub async fn add_event_handler(&self, handler: Arc<dyn RaftEventHandler>) {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(handler);
    }

    /// Start the Raft consensus algorithm
    pub async fn start(&self, transport: Arc<dyn RaftTransport>) -> OrbitResult<()> {
        info!("Starting Raft consensus for node: {}", self.node_id);

        // Start election timer
        self.start_election_timer(transport.clone()).await;

        // Start heartbeat sender (if leader)
        self.start_heartbeat_sender(transport).await;

        Ok(())
    }

    /// Check if this node is the current leader
    pub async fn is_leader(&self) -> bool {
        let state = self.state.read().await;
        *state == RaftState::Leader
    }

    /// Get current leader ID
    pub async fn get_leader(&self) -> Option<NodeId> {
        // In a full implementation, this would track the current leader
        // For now, return self if this node is leader
        if self.is_leader().await {
            Some(self.node_id.clone())
        } else {
            None
        }
    }

    /// Get current term
    pub async fn get_current_term(&self) -> u64 {
        *self.current_term.read().await
    }

    /// Get cluster nodes
    pub async fn get_cluster_nodes(&self) -> Vec<NodeId> {
        let nodes = self.cluster_nodes.read().await;
        nodes.clone()
    }

    /// Start election timer
    async fn start_election_timer(&self, transport: Arc<dyn RaftTransport>) {
        let consensus = self.clone();

        tokio::spawn(async move {
            loop {
                let timeout = {
                    let election_timeout = consensus.election_timeout.read().await;
                    *election_timeout
                };

                tokio::time::sleep(timeout).await;

                // Check if we need to start an election
                let should_start_election = {
                    let state = consensus.state.read().await;
                    let last_heartbeat = consensus.last_heartbeat.read().await;
                    let time_since_heartbeat = last_heartbeat.elapsed();

                    (*state == RaftState::Follower || *state == RaftState::Candidate)
                        && time_since_heartbeat > timeout
                };

                if should_start_election {
                    if let Err(e) = consensus.start_election(transport.clone()).await {
                        error!("Failed to start election: {}", e);
                    }
                }
            }
        });
    }

    /// Start heartbeat sender for leader
    async fn start_heartbeat_sender(&self, transport: Arc<dyn RaftTransport>) {
        let consensus = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(consensus.config.heartbeat_interval);

            loop {
                interval.tick().await;

                let is_leader = consensus.is_leader().await;
                if is_leader {
                    if let Err(e) = consensus.send_heartbeats(transport.clone()).await {
                        error!("Failed to send heartbeats: {}", e);
                    }
                }
            }
        });
    }

    /// Start election process
    async fn start_election(&self, transport: Arc<dyn RaftTransport>) -> OrbitResult<()> {
        info!(
            "Starting election for term: {}",
            self.get_current_term().await + 1
        );

        // Transition to candidate
        {
            let mut state = self.state.write().await;
            *state = RaftState::Candidate;
        }

        // Increment term and vote for self
        let new_term = {
            let mut term = self.current_term.write().await;
            *term += 1;
            *term
        };

        {
            let mut voted_for = self.voted_for.write().await;
            *voted_for = Some(self.node_id.clone());
        }

        // Reset votes
        {
            let mut votes = self.votes_received.write().await;
            votes.clear();
            votes.insert(self.node_id.clone(), true); // Vote for self
        }

        // Get log information for vote request
        let (last_log_index, last_log_term) = {
            let log = self.log.read().await;
            if log.is_empty() {
                (0, 0)
            } else {
                let last_entry = log.last().unwrap();
                (last_entry.index, last_entry.term)
            }
        };

        // Send vote requests to all other nodes
        let cluster_nodes = self.cluster_nodes.read().await;
        let mut vote_tasks = Vec::new();

        for node in cluster_nodes.iter() {
            if node != &self.node_id {
                let request = VoteRequest {
                    term: new_term,
                    candidate_id: self.node_id.clone(),
                    last_log_index,
                    last_log_term,
                };

                let node = node.clone();
                let transport = transport.clone();
                let _consensus = self.clone();

                let task = tokio::spawn(async move {
                    match transport.send_vote_request(&node, request).await {
                        Ok(response) => Some((node, response)),
                        Err(e) => {
                            warn!("Failed to get vote from {}: {}", node, e);
                            None
                        }
                    }
                });

                vote_tasks.push(task);
            }
        }

        // Collect votes with timeout
        let election_timeout = *self.election_timeout.read().await;
        let vote_results = tokio::time::timeout(election_timeout, async {
            let mut results = Vec::new();
            for task in vote_tasks {
                if let Ok(Some(result)) = task.await {
                    results.push(result);
                }
            }
            results
        })
        .await
        .unwrap_or_default();

        // Process vote responses
        let mut vote_count = 1; // Self vote
        for (node_id, response) in vote_results {
            if response.term > new_term {
                // Higher term discovered, step down
                self.step_down(response.term).await?;
                return Ok(());
            }

            if response.vote_granted {
                vote_count += 1;
                let mut votes = self.votes_received.write().await;
                votes.insert(node_id, true);
            }
        }

        // Check if we won the election
        let cluster_size = cluster_nodes.len();
        let majority = cluster_size / 2 + 1;

        if vote_count >= majority {
            self.become_leader().await?;
        } else {
            // Election failed, return to follower
            self.step_down(new_term).await?;
        }

        Ok(())
    }

    /// Become leader
    async fn become_leader(&self) -> OrbitResult<()> {
        info!(
            "Becoming leader for term: {}",
            self.get_current_term().await
        );

        {
            let mut state = self.state.write().await;
            *state = RaftState::Leader;
        }

        // Initialize leader state
        let log_len = {
            let log = self.log.read().await;
            log.len() as u64
        };

        let cluster_nodes = self.cluster_nodes.read().await;
        {
            let mut next_index = self.next_index.write().await;
            let mut match_index = self.match_index.write().await;

            for node in cluster_nodes.iter() {
                if node != &self.node_id {
                    next_index.insert(node.clone(), log_len + 1);
                    match_index.insert(node.clone(), 0);
                }
            }
        }

        // Notify event handlers
        let current_term = self.get_current_term().await;
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_leader_elected(&self.node_id, current_term).await {
                error!("Leader elected event handler failed: {}", e);
            }
        }

        Ok(())
    }

    /// Step down from candidate/leader to follower
    async fn step_down(&self, new_term: u64) -> OrbitResult<()> {
        let old_term = {
            let mut term = self.current_term.write().await;
            let old = *term;
            *term = new_term;
            old
        };

        {
            let mut state = self.state.write().await;
            *state = RaftState::Follower;
        }

        {
            let mut voted_for = self.voted_for.write().await;
            *voted_for = None;
        }

        // Notify event handlers
        if old_term != new_term {
            let handlers = self.event_handlers.read().await;
            for handler in handlers.iter() {
                if let Err(e) = handler.on_term_changed(old_term, new_term).await {
                    error!("Term changed event handler failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Send heartbeats to all followers
    async fn send_heartbeats(&self, transport: Arc<dyn RaftTransport>) -> OrbitResult<()> {
        let cluster_nodes = self.cluster_nodes.read().await;
        let current_term = self.get_current_term().await;
        let commit_index = *self.commit_index.read().await;

        // Get previous log entry info
        let (prev_log_index, prev_log_term) = {
            let log = self.log.read().await;
            if log.is_empty() {
                (0, 0)
            } else {
                let last_entry = log.last().unwrap();
                (last_entry.index, last_entry.term)
            }
        };

        let request = AppendEntriesRequest {
            term: current_term,
            leader_id: self.node_id.clone(),
            prev_log_index,
            prev_log_term,
            entries: vec![], // Heartbeat - no entries
            leader_commit: commit_index,
        };

        let other_nodes: Vec<NodeId> = cluster_nodes
            .iter()
            .filter(|node| *node != &self.node_id)
            .cloned()
            .collect();

        let responses = transport.broadcast_heartbeat(&other_nodes, request).await?;

        // Process heartbeat responses
        for response in responses {
            if response.term > current_term {
                self.step_down(response.term).await?;
                return Ok(());
            }
        }

        Ok(())
    }

    /// Handle incoming vote request
    pub async fn handle_vote_request(&self, request: VoteRequest) -> OrbitResult<VoteResponse> {
        let current_term = self.get_current_term().await;
        let voted_for = self.voted_for.read().await.clone();

        // Reply false if term < currentTerm
        if request.term < current_term {
            return Ok(VoteResponse {
                term: current_term,
                vote_granted: false,
                voter_id: self.node_id.clone(),
            });
        }

        // If term > currentTerm, update term and step down
        if request.term > current_term {
            self.step_down(request.term).await?;
        }

        // Check if we can grant vote
        let can_vote = voted_for.is_none() || voted_for == Some(request.candidate_id.clone());

        let log_up_to_date = {
            let log = self.log.read().await;
            if log.is_empty() {
                true
            } else {
                let last_entry = log.last().unwrap();
                request.last_log_term > last_entry.term
                    || (request.last_log_term == last_entry.term
                        && request.last_log_index >= last_entry.index)
            }
        };

        let vote_granted = can_vote && log_up_to_date;

        if vote_granted {
            let mut voted_for_write = self.voted_for.write().await;
            *voted_for_write = Some(request.candidate_id.clone());
        }

        Ok(VoteResponse {
            term: request.term,
            vote_granted,
            voter_id: self.node_id.clone(),
        })
    }

    /// Handle incoming append entries request
    pub async fn handle_append_entries(
        &self,
        request: AppendEntriesRequest,
    ) -> OrbitResult<AppendEntriesResponse> {
        let current_term = self.get_current_term().await;

        // Reply false if term < currentTerm
        if request.term < current_term {
            return Ok(AppendEntriesResponse {
                term: current_term,
                success: false,
                follower_id: self.node_id.clone(),
                last_log_index: 0,
            });
        }

        // Update heartbeat timestamp
        {
            let mut last_heartbeat = self.last_heartbeat.write().await;
            *last_heartbeat = Instant::now();
        }

        // If term >= currentTerm, convert to follower
        if request.term >= current_term {
            self.step_down(request.term).await?;
        }

        // Reset election timeout
        {
            let mut election_timeout = self.election_timeout.write().await;
            *election_timeout = Self::random_election_timeout(&self.config);
        }

        // Handle log consistency check and append entries
        // (Simplified implementation - full Raft log handling would go here)

        Ok(AppendEntriesResponse {
            term: request.term,
            success: true,
            follower_id: self.node_id.clone(),
            last_log_index: request.prev_log_index + request.entries.len() as u64,
        })
    }
}

impl Clone for RaftConsensus {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            cluster_nodes: Arc::clone(&self.cluster_nodes),
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            current_term: Arc::clone(&self.current_term),
            voted_for: Arc::clone(&self.voted_for),
            log: Arc::clone(&self.log),
            commit_index: Arc::clone(&self.commit_index),
            last_applied: Arc::clone(&self.last_applied),
            next_index: Arc::clone(&self.next_index),
            match_index: Arc::clone(&self.match_index),
            last_heartbeat: Arc::clone(&self.last_heartbeat),
            election_timeout: Arc::clone(&self.election_timeout),
            votes_received: Arc::clone(&self.votes_received),
            event_handlers: Arc::clone(&self.event_handlers),
        }
    }
}
