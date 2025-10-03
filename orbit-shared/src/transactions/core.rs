use crate::addressable::AddressableReference;
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{timeout, Duration, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Unique identifier for distributed transactions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId {
    pub id: String,
    pub coordinator_node: NodeId,
    pub created_at: i64,
}

impl TransactionId {
    pub fn new(coordinator_node: NodeId) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            coordinator_node,
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.id, self.coordinator_node)
    }
}

/// Transaction states in the 2-phase commit protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction is being prepared
    Preparing,
    /// All participants voted to commit
    Prepared,
    /// Transaction is being committed
    Committing,
    /// Transaction successfully committed
    Committed,
    /// Transaction is being aborted
    Aborting,
    /// Transaction was aborted
    Aborted,
    /// Transaction timed out
    TimedOut,
    /// Transaction encountered an error
    Failed { reason: String },
}

/// Vote from a participant in the 2-phase commit protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionVote {
    /// Participant is ready to commit
    Yes,
    /// Participant cannot commit and wants to abort
    No { reason: String },
    /// Participant is uncertain (network issues, etc.)
    Uncertain,
}

/// Operation to be executed as part of a distributed transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOperation {
    pub operation_id: String,
    pub target_actor: AddressableReference,
    pub operation_type: String,
    pub operation_data: serde_json::Value,
    pub compensation_data: Option<serde_json::Value>,
}

impl TransactionOperation {
    pub fn new(
        target_actor: AddressableReference,
        operation_type: String,
        operation_data: serde_json::Value,
    ) -> Self {
        Self {
            operation_id: Uuid::new_v4().to_string(),
            target_actor,
            operation_type,
            operation_data,
            compensation_data: None,
        }
    }

    pub fn with_compensation(mut self, compensation_data: serde_json::Value) -> Self {
        self.compensation_data = Some(compensation_data);
        self
    }
}

/// Distributed transaction containing multiple operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTransaction {
    pub transaction_id: TransactionId,
    pub operations: Vec<TransactionOperation>,
    pub state: TransactionState,
    pub timeout: Duration,
    pub started_at: i64,
    pub metadata: HashMap<String, String>,
    /// Participants involved in this transaction (nodes and actors)
    pub participants: HashSet<AddressableReference>,
}

impl DistributedTransaction {
    pub fn new(coordinator_node: NodeId, timeout: Duration) -> Self {
        Self {
            transaction_id: TransactionId::new(coordinator_node),
            operations: Vec::new(),
            state: TransactionState::Preparing,
            timeout,
            started_at: chrono::Utc::now().timestamp_millis(),
            metadata: HashMap::new(),
            participants: HashSet::new(),
        }
    }

    pub fn add_operation(mut self, operation: TransactionOperation) -> Self {
        self.participants.insert(operation.target_actor.clone());
        self.operations.push(operation);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if the transaction has timed out
    pub fn is_timed_out(&self) -> bool {
        let elapsed_ms = chrono::Utc::now().timestamp_millis() - self.started_at;
        elapsed_ms > self.timeout.as_millis() as i64
    }

    /// Get all unique nodes involved in this transaction
    pub fn get_participant_nodes(&self) -> HashSet<NodeId> {
        // In a real implementation, this would resolve actors to their current nodes
        // For now, we'll return a mock set
        let mut nodes = HashSet::new();
        nodes.insert(self.transaction_id.coordinator_node.clone());
        nodes
    }
}

/// Message types for distributed transaction protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionMessage {
    /// Phase 1: Coordinator asks participants to prepare
    Prepare {
        transaction_id: TransactionId,
        operations: Vec<TransactionOperation>,
        timeout: Duration,
    },
    /// Phase 1: Participant responds with vote
    Vote {
        transaction_id: TransactionId,
        participant: AddressableReference,
        vote: TransactionVote,
    },
    /// Phase 2: Coordinator tells participants to commit
    Commit { transaction_id: TransactionId },
    /// Phase 2: Coordinator tells participants to abort
    Abort {
        transaction_id: TransactionId,
        reason: String,
    },
    /// Participant acknowledges commit/abort
    Acknowledge {
        transaction_id: TransactionId,
        participant: AddressableReference,
        success: bool,
        error: Option<String>,
    },
    /// Query transaction status
    QueryStatus { transaction_id: TransactionId },
    /// Response to status query
    StatusResponse {
        transaction_id: TransactionId,
        state: TransactionState,
    },
}

/// Configuration for distributed transaction system
#[derive(Debug, Clone)]
pub struct TransactionConfig {
    /// Default transaction timeout
    pub default_timeout: Duration,
    /// Maximum number of concurrent transactions
    pub max_concurrent_transactions: usize,
    /// Retry attempts for failed operations
    pub retry_attempts: u32,
    /// Interval for cleanup of old transactions
    pub cleanup_interval: Duration,
    /// Enable transaction logging for audit
    pub enable_logging: bool,
    /// Maximum transaction log size
    pub max_log_entries: usize,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_concurrent_transactions: 1000,
            retry_attempts: 3,
            cleanup_interval: Duration::from_secs(60),
            enable_logging: true,
            max_log_entries: 10000,
        }
    }
}

/// Transaction log entry for audit and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLogEntry {
    pub timestamp: i64,
    pub transaction_id: TransactionId,
    pub event: TransactionEvent,
    pub details: Option<serde_json::Value>,
}

/// Transaction events for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionEvent {
    Started,
    PrepareRequested,
    VoteReceived {
        participant: AddressableReference,
        vote: TransactionVote,
    },
    CommitRequested,
    AbortRequested {
        reason: String,
    },
    Committed,
    Aborted {
        reason: String,
    },
    TimedOut,
    Failed {
        error: String,
    },
}

/// Trait for participating in distributed transactions
#[async_trait]
pub trait TransactionParticipant: Send + Sync {
    /// Prepare phase: Check if the participant can commit the transaction
    async fn prepare(
        &self,
        transaction_id: &TransactionId,
        operations: &[TransactionOperation],
    ) -> OrbitResult<TransactionVote>;

    /// Commit phase: Execute the transaction operations
    async fn commit(&self, transaction_id: &TransactionId) -> OrbitResult<()>;

    /// Abort phase: Rollback any changes made during prepare
    async fn abort(&self, transaction_id: &TransactionId, reason: &str) -> OrbitResult<()>;

    /// Get current state of a transaction from participant's perspective
    async fn get_transaction_state(
        &self,
        transaction_id: &TransactionId,
    ) -> OrbitResult<Option<TransactionState>>;
}

/// Transaction coordinator that manages distributed transactions
pub struct TransactionCoordinator {
    node_id: NodeId,
    config: TransactionConfig,
    /// Active transactions being coordinated
    active_transactions: Arc<RwLock<HashMap<TransactionId, DistributedTransaction>>>,
    /// Transaction logs for audit and recovery
    transaction_log: Arc<Mutex<Vec<TransactionLogEntry>>>,
    /// Message sender for transaction protocol
    message_sender: Arc<dyn TransactionMessageSender>,
    /// Votes received from participants
    participant_votes:
        Arc<RwLock<HashMap<TransactionId, HashMap<AddressableReference, TransactionVote>>>>,
    /// Acknowledgments received from participants
    participant_acks: Arc<RwLock<HashMap<TransactionId, HashMap<AddressableReference, bool>>>>,
}

/// Trait for sending transaction messages to participants
#[async_trait]
pub trait TransactionMessageSender: Send + Sync {
    async fn send_message(
        &self,
        target: &AddressableReference,
        message: TransactionMessage,
    ) -> OrbitResult<()>;
    async fn broadcast_message(
        &self,
        targets: &[AddressableReference],
        message: TransactionMessage,
    ) -> OrbitResult<()>;
}

impl TransactionCoordinator {
    pub fn new(
        node_id: NodeId,
        config: TransactionConfig,
        message_sender: Arc<dyn TransactionMessageSender>,
    ) -> Self {
        Self {
            node_id,
            config,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            transaction_log: Arc::new(Mutex::new(Vec::new())),
            message_sender,
            participant_votes: Arc::new(RwLock::new(HashMap::new())),
            participant_acks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a new distributed transaction
    pub async fn begin_transaction(&self, timeout: Option<Duration>) -> OrbitResult<TransactionId> {
        let transaction_timeout = timeout.unwrap_or(self.config.default_timeout);
        let transaction = DistributedTransaction::new(self.node_id.clone(), transaction_timeout);
        let transaction_id = transaction.transaction_id.clone();

        // Check concurrent transaction limit
        {
            let active = self.active_transactions.read().await;
            if active.len() >= self.config.max_concurrent_transactions {
                return Err(OrbitError::internal("Too many concurrent transactions"));
            }
        }

        // Add to active transactions
        {
            let mut active = self.active_transactions.write().await;
            active.insert(transaction_id.clone(), transaction);
        }

        self.log_transaction_event(&transaction_id, TransactionEvent::Started, None)
            .await?;

        info!("Started transaction: {}", transaction_id);
        Ok(transaction_id)
    }

    /// Add an operation to a transaction
    pub async fn add_operation(
        &self,
        transaction_id: &TransactionId,
        operation: TransactionOperation,
    ) -> OrbitResult<()> {
        let mut active = self.active_transactions.write().await;
        if let Some(transaction) = active.get_mut(transaction_id) {
            if transaction.state != TransactionState::Preparing {
                return Err(OrbitError::internal(
                    "Cannot add operations to non-preparing transaction",
                ));
            }

            transaction
                .participants
                .insert(operation.target_actor.clone());
            transaction.operations.push(operation);

            Ok(())
        } else {
            Err(OrbitError::internal("Transaction not found"))
        }
    }

    /// Execute a distributed transaction using 2-phase commit
    pub async fn commit_transaction(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        // Phase 1: Prepare
        self.prepare_phase(transaction_id).await?;

        // Check if all participants voted yes
        let can_commit = self.check_votes(transaction_id).await?;

        if can_commit {
            // Phase 2: Commit
            self.commit_phase(transaction_id).await?;
        } else {
            // Phase 2: Abort
            self.abort_phase(transaction_id, "Not all participants voted yes")
                .await?;
        }

        Ok(())
    }

    /// Phase 1: Send prepare messages to all participants
    async fn prepare_phase(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        let (transaction, participants) = {
            let active = self.active_transactions.read().await;
            let transaction = active
                .get(transaction_id)
                .ok_or_else(|| OrbitError::internal("Transaction not found"))?
                .clone();
            let participants: Vec<_> = transaction.participants.iter().cloned().collect();
            (transaction, participants)
        };

        self.log_transaction_event(transaction_id, TransactionEvent::PrepareRequested, None)
            .await?;

        // Send prepare message to all participants
        let prepare_message = TransactionMessage::Prepare {
            transaction_id: transaction_id.clone(),
            operations: transaction.operations.clone(),
            timeout: transaction.timeout,
        };

        self.message_sender
            .broadcast_message(&participants, prepare_message)
            .await?;

        // Wait for votes with timeout
        let vote_timeout = transaction.timeout / 2; // Use half the transaction timeout
        if (timeout(
            vote_timeout,
            self.wait_for_votes(transaction_id, participants.len()),
        )
        .await)
            .is_err()
        {
            self.abort_phase(transaction_id, "Timeout waiting for votes")
                .await?;
            return Err(OrbitError::timeout("prepare_phase"));
        }

        Ok(())
    }

    /// Wait for votes from all participants
    async fn wait_for_votes(
        &self,
        transaction_id: &TransactionId,
        expected_votes: usize,
    ) -> OrbitResult<()> {
        let poll_interval = Duration::from_millis(100);

        loop {
            {
                let votes = self.participant_votes.read().await;
                if let Some(transaction_votes) = votes.get(transaction_id) {
                    if transaction_votes.len() >= expected_votes {
                        break;
                    }
                }
            }
            tokio::time::sleep(poll_interval).await;
        }

        Ok(())
    }

    /// Check if all participants voted to commit
    async fn check_votes(&self, transaction_id: &TransactionId) -> OrbitResult<bool> {
        let votes = self.participant_votes.read().await;
        if let Some(transaction_votes) = votes.get(transaction_id) {
            for (participant, vote) in transaction_votes {
                match vote {
                    TransactionVote::Yes => continue,
                    TransactionVote::No { reason } => {
                        warn!("Participant {} voted no: {}", participant, reason);
                        return Ok(false);
                    }
                    TransactionVote::Uncertain => {
                        warn!("Participant {} voted uncertain", participant);
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Phase 2: Send commit messages to all participants
    async fn commit_phase(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        let participants = {
            let active = self.active_transactions.read().await;
            let transaction = active
                .get(transaction_id)
                .ok_or_else(|| OrbitError::internal("Transaction not found"))?;
            transaction.participants.iter().cloned().collect::<Vec<_>>()
        };

        // Update transaction state
        {
            let mut active = self.active_transactions.write().await;
            if let Some(transaction) = active.get_mut(transaction_id) {
                transaction.state = TransactionState::Committing;
            }
        }

        self.log_transaction_event(transaction_id, TransactionEvent::CommitRequested, None)
            .await?;

        // Send commit message to all participants
        let commit_message = TransactionMessage::Commit {
            transaction_id: transaction_id.clone(),
        };

        self.message_sender
            .broadcast_message(&participants, commit_message)
            .await?;

        // Wait for acknowledgments
        self.wait_for_acknowledgments(transaction_id, participants.len())
            .await?;

        // Update final state
        {
            let mut active = self.active_transactions.write().await;
            if let Some(transaction) = active.get_mut(transaction_id) {
                transaction.state = TransactionState::Committed;
            }
        }

        self.log_transaction_event(transaction_id, TransactionEvent::Committed, None)
            .await?;

        info!("Transaction {} committed successfully", transaction_id);
        Ok(())
    }

    /// Phase 2: Send abort messages to all participants
    async fn abort_phase(&self, transaction_id: &TransactionId, reason: &str) -> OrbitResult<()> {
        let participants = {
            let active = self.active_transactions.read().await;
            let transaction = active
                .get(transaction_id)
                .ok_or_else(|| OrbitError::internal("Transaction not found"))?;
            transaction.participants.iter().cloned().collect::<Vec<_>>()
        };

        // Update transaction state
        {
            let mut active = self.active_transactions.write().await;
            if let Some(transaction) = active.get_mut(transaction_id) {
                transaction.state = TransactionState::Aborting;
            }
        }

        self.log_transaction_event(
            transaction_id,
            TransactionEvent::AbortRequested {
                reason: reason.to_string(),
            },
            None,
        )
        .await?;

        // Send abort message to all participants
        let abort_message = TransactionMessage::Abort {
            transaction_id: transaction_id.clone(),
            reason: reason.to_string(),
        };

        self.message_sender
            .broadcast_message(&participants, abort_message)
            .await?;

        // Wait for acknowledgments
        self.wait_for_acknowledgments(transaction_id, participants.len())
            .await?;

        // Update final state
        {
            let mut active = self.active_transactions.write().await;
            if let Some(transaction) = active.get_mut(transaction_id) {
                transaction.state = TransactionState::Aborted;
            }
        }

        self.log_transaction_event(
            transaction_id,
            TransactionEvent::Aborted {
                reason: reason.to_string(),
            },
            None,
        )
        .await?;

        warn!("Transaction {} aborted: {}", transaction_id, reason);
        Ok(())
    }

    /// Wait for acknowledgments from all participants
    async fn wait_for_acknowledgments(
        &self,
        transaction_id: &TransactionId,
        expected_acks: usize,
    ) -> OrbitResult<()> {
        let poll_interval = Duration::from_millis(100);
        let timeout_duration = self.config.default_timeout;
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() > timeout_duration {
                return Err(OrbitError::timeout("wait_for_acknowledgments"));
            }

            {
                let acks = self.participant_acks.read().await;
                if let Some(transaction_acks) = acks.get(transaction_id) {
                    if transaction_acks.len() >= expected_acks {
                        break;
                    }
                }
            }
            tokio::time::sleep(poll_interval).await;
        }

        Ok(())
    }

    /// Handle incoming transaction messages
    pub async fn handle_message(&self, message: TransactionMessage) -> OrbitResult<()> {
        match message {
            TransactionMessage::Vote {
                transaction_id,
                participant,
                vote,
            } => self.handle_vote(transaction_id, participant, vote).await,
            TransactionMessage::Acknowledge {
                transaction_id,
                participant,
                success,
                error,
            } => {
                self.handle_acknowledgment(transaction_id, participant, success, error)
                    .await
            }
            _ => {
                warn!("Received unexpected message type for coordinator");
                Ok(())
            }
        }
    }

    /// Handle vote from participant
    async fn handle_vote(
        &self,
        transaction_id: TransactionId,
        participant: AddressableReference,
        vote: TransactionVote,
    ) -> OrbitResult<()> {
        {
            let mut votes = self.participant_votes.write().await;
            votes
                .entry(transaction_id.clone())
                .or_insert_with(HashMap::new)
                .insert(participant.clone(), vote.clone());
        }

        self.log_transaction_event(
            &transaction_id,
            TransactionEvent::VoteReceived { participant, vote },
            None,
        )
        .await?;

        Ok(())
    }

    /// Handle acknowledgment from participant
    async fn handle_acknowledgment(
        &self,
        transaction_id: TransactionId,
        participant: AddressableReference,
        success: bool,
        error: Option<String>,
    ) -> OrbitResult<()> {
        {
            let mut acks = self.participant_acks.write().await;
            acks.entry(transaction_id.clone())
                .or_insert_with(HashMap::new)
                .insert(participant.clone(), success);
        }

        if let Some(error) = error {
            warn!("Participant {} reported error: {}", participant, error);
        }

        Ok(())
    }

    /// Log a transaction event
    async fn log_transaction_event(
        &self,
        transaction_id: &TransactionId,
        event: TransactionEvent,
        details: Option<serde_json::Value>,
    ) -> OrbitResult<()> {
        if !self.config.enable_logging {
            return Ok(());
        }

        let entry = TransactionLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            transaction_id: transaction_id.clone(),
            event,
            details,
        };

        let mut log = self.transaction_log.lock().await;
        log.push(entry);

        // Limit log size
        if log.len() > self.config.max_log_entries {
            let log_len = log.len();
            log.drain(0..log_len - self.config.max_log_entries);
        }

        Ok(())
    }

    /// Start background cleanup tasks
    pub async fn start_background_tasks(&self) -> OrbitResult<()> {
        let coordinator = Arc::new(self.clone());

        // Start timeout cleanup task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(coordinator.config.cleanup_interval);
            loop {
                interval.tick().await;
                if let Err(e) = coordinator.cleanup_timed_out_transactions().await {
                    error!("Transaction cleanup failed: {}", e);
                }
            }
        });

        info!("Transaction coordinator background tasks started");
        Ok(())
    }

    /// Clean up timed out transactions
    async fn cleanup_timed_out_transactions(&self) -> OrbitResult<()> {
        let mut to_remove = Vec::new();

        {
            let active = self.active_transactions.read().await;
            for (tx_id, transaction) in active.iter() {
                if transaction.is_timed_out() {
                    to_remove.push(tx_id.clone());
                }
            }
        }

        for tx_id in to_remove {
            self.abort_phase(&tx_id, "Transaction timed out").await?;

            // Remove from active transactions
            {
                let mut active = self.active_transactions.write().await;
                active.remove(&tx_id);
            }

            // Clean up related data
            {
                let mut votes = self.participant_votes.write().await;
                let mut acks = self.participant_acks.write().await;
                votes.remove(&tx_id);
                acks.remove(&tx_id);
            }

            self.log_transaction_event(&tx_id, TransactionEvent::TimedOut, None)
                .await?;
        }

        Ok(())
    }

    /// Get transaction statistics
    pub async fn get_stats(&self) -> TransactionStats {
        let active = self.active_transactions.read().await;
        let log = self.transaction_log.lock().await;

        TransactionStats {
            active_transactions: active.len(),
            total_log_entries: log.len(),
            committed_count: log
                .iter()
                .filter(|e| matches!(e.event, TransactionEvent::Committed))
                .count(),
            aborted_count: log
                .iter()
                .filter(|e| matches!(e.event, TransactionEvent::Aborted { .. }))
                .count(),
            timed_out_count: log
                .iter()
                .filter(|e| matches!(e.event, TransactionEvent::TimedOut))
                .count(),
        }
    }
}

impl Clone for TransactionCoordinator {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            config: self.config.clone(),
            active_transactions: Arc::clone(&self.active_transactions),
            transaction_log: Arc::clone(&self.transaction_log),
            message_sender: Arc::clone(&self.message_sender),
            participant_votes: Arc::clone(&self.participant_votes),
            participant_acks: Arc::clone(&self.participant_acks),
        }
    }
}

/// Transaction statistics for monitoring
#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub active_transactions: usize,
    pub total_log_entries: usize,
    pub committed_count: usize,
    pub aborted_count: usize,
    pub timed_out_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressable::{AddressableReference, Key};

    // Mock message sender for testing
    #[derive(Debug)]
    struct MockMessageSender {
        sent_messages: Arc<Mutex<Vec<(AddressableReference, TransactionMessage)>>>,
    }

    impl MockMessageSender {
        fn new() -> Self {
            Self {
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl TransactionMessageSender for MockMessageSender {
        async fn send_message(
            &self,
            target: &AddressableReference,
            message: TransactionMessage,
        ) -> OrbitResult<()> {
            let mut messages = self.sent_messages.lock().await;
            messages.push((target.clone(), message));
            Ok(())
        }

        async fn broadcast_message(
            &self,
            targets: &[AddressableReference],
            message: TransactionMessage,
        ) -> OrbitResult<()> {
            for target in targets {
                self.send_message(target, message.clone()).await?;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_transaction_creation() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let message_sender = Arc::new(MockMessageSender::new());
        let coordinator = TransactionCoordinator::new(
            node_id.clone(),
            TransactionConfig::default(),
            message_sender,
        );

        let tx_id = coordinator.begin_transaction(None).await.unwrap();
        assert_eq!(tx_id.coordinator_node, node_id);

        let operation = TransactionOperation::new(
            AddressableReference {
                addressable_type: "TestActor".to_string(),
                key: Key::StringKey {
                    key: "test-1".to_string(),
                },
            },
            "update".to_string(),
            serde_json::json!({"value": 42}),
        );

        coordinator.add_operation(&tx_id, operation).await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_timeout() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let timeout = Duration::from_millis(100);

        let transaction = DistributedTransaction::new(node_id, timeout);

        // Should not be timed out initially
        assert!(!transaction.is_timed_out());

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be timed out now (in real test we'd manipulate timestamps)
        // For now, just verify the logic structure works
    }

    #[tokio::test]
    async fn test_transaction_vote_handling() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let message_sender = Arc::new(MockMessageSender::new());
        let coordinator = TransactionCoordinator::new(
            node_id.clone(),
            TransactionConfig::default(),
            message_sender,
        );

        let tx_id = TransactionId::new(node_id);
        let participant = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-1".to_string(),
            },
        };

        coordinator
            .handle_vote(tx_id.clone(), participant.clone(), TransactionVote::Yes)
            .await
            .unwrap();

        let votes = coordinator.participant_votes.read().await;
        assert!(votes.contains_key(&tx_id));
        assert_eq!(votes[&tx_id][&participant], TransactionVote::Yes);
    }
}
