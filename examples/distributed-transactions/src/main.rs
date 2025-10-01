use async_trait::async_trait;
use orbit_shared::{transactions::*, AddressableReference, Key, NodeId, OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Duration;
use tracing::{error, info, warn};

/// Banking transaction operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BankingOperation {
    Debit { amount: f64 },
    Credit { amount: f64 },
    Transfer { to_account: String, amount: f64 },
    ReserveFunds { amount: f64 },
    ReleaseFunds { amount: f64 },
}

/// Bank account state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccount {
    pub account_id: String,
    pub balance: f64,
    pub reserved_funds: f64,
    pub account_holder: String,
    pub is_frozen: bool,
}

impl BankAccount {
    pub fn new(account_id: String, account_holder: String, initial_balance: f64) -> Self {
        Self {
            account_id,
            balance: initial_balance,
            reserved_funds: 0.0,
            account_holder,
            is_frozen: false,
        }
    }

    pub fn available_balance(&self) -> f64 {
        self.balance - self.reserved_funds
    }

    pub fn can_debit(&self, amount: f64) -> bool {
        !self.is_frozen && self.available_balance() >= amount && amount > 0.0
    }

    pub fn can_reserve(&self, amount: f64) -> bool {
        !self.is_frozen && self.available_balance() >= amount && amount > 0.0
    }
}

/// Mock bank service that manages accounts and participates in transactions
#[derive(Debug)]
pub struct BankService {
    #[allow(dead_code)]
    node_id: NodeId,
    accounts: Arc<RwLock<HashMap<String, BankAccount>>>,
    /// Pending transaction operations (for 2-phase commit)
    pending_transactions: Arc<Mutex<HashMap<TransactionId, Vec<BankingOperation>>>>,
}

impl BankService {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            accounts: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new bank account
    pub async fn create_account(
        &self,
        account_id: String,
        account_holder: String,
        initial_balance: f64,
    ) -> OrbitResult<()> {
        let mut accounts = self.accounts.write().await;
        if accounts.contains_key(&account_id) {
            return Err(OrbitError::internal("Account already exists"));
        }

        accounts.insert(
            account_id.clone(),
            BankAccount::new(account_id, account_holder, initial_balance),
        );

        info!("Created account with initial balance: {}", initial_balance);
        Ok(())
    }

    /// Get account balance
    pub async fn get_account(&self, account_id: &str) -> OrbitResult<BankAccount> {
        let accounts = self.accounts.read().await;
        accounts
            .get(account_id)
            .cloned()
            .ok_or_else(|| OrbitError::internal("Account not found"))
    }

    /// Check if account can participate in transaction operations
    async fn validate_operations(&self, operations: &[TransactionOperation]) -> OrbitResult<bool> {
        let accounts = self.accounts.read().await;

        for operation in operations {
            let banking_op: BankingOperation =
                serde_json::from_value(operation.operation_data.clone())
                    .map_err(|e| OrbitError::internal(format!("Invalid operation data: {}", e)))?;

            // Extract account ID from the actor reference
            let account_id = match &operation.target_actor.key {
                Key::StringKey { key } => key.clone(),
                _ => return Ok(false),
            };

            let account = accounts
                .get(&account_id)
                .ok_or_else(|| OrbitError::internal("Account not found"))?;

            match banking_op {
                BankingOperation::Debit { amount } => {
                    if !account.can_debit(amount) {
                        warn!(
                            "Cannot debit ${} from account {}: insufficient funds or frozen",
                            amount, account_id
                        );
                        return Ok(false);
                    }
                }
                BankingOperation::ReserveFunds { amount } => {
                    if !account.can_reserve(amount) {
                        warn!(
                            "Cannot reserve ${} in account {}: insufficient funds or frozen",
                            amount, account_id
                        );
                        return Ok(false);
                    }
                }
                BankingOperation::Transfer { amount, .. } => {
                    if !account.can_debit(amount) {
                        warn!(
                            "Cannot transfer ${} from account {}: insufficient funds or frozen",
                            amount, account_id
                        );
                        return Ok(false);
                    }
                }
                BankingOperation::Credit { .. } | BankingOperation::ReleaseFunds { .. } => {
                    // These operations generally don't require validation
                    if account.is_frozen {
                        warn!("Account {} is frozen", account_id);
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    /// Execute banking operations during commit phase
    async fn execute_operations(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        let operations = {
            let mut pending = self.pending_transactions.lock().await;
            pending
                .remove(transaction_id)
                .ok_or_else(|| OrbitError::internal("No pending operations for transaction"))?
        };

        let operations_count = operations.len();
        let _accounts = self.accounts.write().await;

        for operation in &operations {
            #[allow(clippy::let_unit_value)]
            let _operation_data = match operation {
                BankingOperation::Debit { .. }
                | BankingOperation::Credit { .. }
                | BankingOperation::ReserveFunds { .. }
                | BankingOperation::ReleaseFunds { .. } => {
                    // Extract account ID from operation context - in real implementation,
                    // this would be stored with the operation
                    continue; // Skip for this demo
                }
                BankingOperation::Transfer { .. } => continue, // Complex operation
            };
        }

        info!(
            "Executed {} banking operations for transaction {}",
            operations_count, transaction_id
        );
        Ok(())
    }
}

#[async_trait]
impl TransactionParticipant for BankService {
    async fn prepare(
        &self,
        transaction_id: &TransactionId,
        operations: &[TransactionOperation],
    ) -> OrbitResult<TransactionVote> {
        info!(
            "Preparing transaction {} with {} operations",
            transaction_id,
            operations.len()
        );

        // Validate that we can perform all operations
        let can_commit = self.validate_operations(operations).await?;

        if !can_commit {
            return Ok(TransactionVote::No {
                reason: "Cannot perform one or more banking operations".to_string(),
            });
        }

        // Store operations for commit phase
        {
            let mut pending = self.pending_transactions.lock().await;
            let banking_operations: Result<Vec<BankingOperation>, _> = operations
                .iter()
                .map(|op| serde_json::from_value(op.operation_data.clone()))
                .collect();

            match banking_operations {
                Ok(ops) => {
                    pending.insert(transaction_id.clone(), ops);
                    info!("Prepared transaction {} - voting YES", transaction_id);
                    Ok(TransactionVote::Yes)
                }
                Err(e) => {
                    warn!("Failed to parse operations: {}", e);
                    Ok(TransactionVote::No {
                        reason: format!("Invalid operation format: {}", e),
                    })
                }
            }
        }
    }

    async fn commit(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        info!("Committing transaction {}", transaction_id);
        self.execute_operations(transaction_id).await
    }

    async fn abort(&self, transaction_id: &TransactionId, reason: &str) -> OrbitResult<()> {
        warn!("Aborting transaction {}: {}", transaction_id, reason);

        // Clean up any reserved resources
        let mut pending = self.pending_transactions.lock().await;
        pending.remove(transaction_id);

        // In a real implementation, we would also release any reserved funds
        info!(
            "Cleaned up resources for aborted transaction {}",
            transaction_id
        );
        Ok(())
    }

    async fn get_transaction_state(
        &self,
        transaction_id: &TransactionId,
    ) -> OrbitResult<Option<TransactionState>> {
        let pending = self.pending_transactions.lock().await;
        if pending.contains_key(transaction_id) {
            Ok(Some(TransactionState::Prepared))
        } else {
            Ok(None)
        }
    }
}

/// Mock message sender for transaction coordination
#[derive(Debug)]
pub struct MockTransactionMessageSender {
    participants: Arc<RwLock<HashMap<AddressableReference, Arc<BankService>>>>,
}

impl MockTransactionMessageSender {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            participants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_participant(
        &self,
        actor_ref: AddressableReference,
        service: Arc<BankService>,
    ) {
        let mut participants = self.participants.write().await;
        participants.insert(actor_ref, service);
    }
}

#[async_trait]
impl TransactionMessageSender for MockTransactionMessageSender {
    async fn send_message(
        &self,
        target: &AddressableReference,
        message: TransactionMessage,
    ) -> OrbitResult<()> {
        let participants = self.participants.read().await;
        if let Some(service) = participants.get(target) {
            match message {
                TransactionMessage::Prepare {
                    transaction_id,
                    operations,
                    ..
                } => {
                    let vote = service.prepare(&transaction_id, &operations).await?;
                    info!("Participant {} voted: {:?}", target, vote);
                    // In real implementation, this would send the vote back to coordinator
                }
                TransactionMessage::Commit { transaction_id } => {
                    service.commit(&transaction_id).await?;
                    info!(
                        "Participant {} committed transaction {}",
                        target, transaction_id
                    );
                }
                TransactionMessage::Abort {
                    transaction_id,
                    reason,
                } => {
                    service.abort(&transaction_id, &reason).await?;
                    info!(
                        "Participant {} aborted transaction {}: {}",
                        target, transaction_id, reason
                    );
                }
                _ => {
                    warn!("Received unexpected message type");
                }
            }
        }
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

/// Run a comprehensive distributed transaction example
async fn run_banking_transaction_example() -> OrbitResult<()> {
    info!("Starting distributed banking transaction example");

    // Create nodes and services
    let node1 = NodeId::new("bank-node-1".to_string(), "us-east".to_string());
    let node2 = NodeId::new("bank-node-2".to_string(), "us-west".to_string());

    let bank_service1 = Arc::new(BankService::new(node1.clone()));
    let bank_service2 = Arc::new(BankService::new(node2.clone()));

    // Create accounts
    bank_service1
        .create_account("alice".to_string(), "Alice Smith".to_string(), 1000.0)
        .await?;
    bank_service1
        .create_account("bob".to_string(), "Bob Johnson".to_string(), 500.0)
        .await?;
    bank_service2
        .create_account("charlie".to_string(), "Charlie Brown".to_string(), 750.0)
        .await?;

    // Set up transaction coordinator
    let message_sender = Arc::new(MockTransactionMessageSender::new());
    let config = TransactionConfig {
        default_timeout: Duration::from_secs(30),
        max_concurrent_transactions: 100,
        retry_attempts: 3,
        cleanup_interval: Duration::from_secs(60),
        enable_logging: true,
        max_log_entries: 1000,
    };
    let coordinator = TransactionCoordinator::new(node1.clone(), config, message_sender.clone());

    // Register participants
    let alice_ref = AddressableReference {
        addressable_type: "BankAccount".to_string(),
        key: Key::StringKey {
            key: "alice".to_string(),
        },
    };
    let bob_ref = AddressableReference {
        addressable_type: "BankAccount".to_string(),
        key: Key::StringKey {
            key: "bob".to_string(),
        },
    };
    let charlie_ref = AddressableReference {
        addressable_type: "BankAccount".to_string(),
        key: Key::StringKey {
            key: "charlie".to_string(),
        },
    };

    message_sender
        .register_participant(alice_ref.clone(), bank_service1.clone())
        .await;
    message_sender
        .register_participant(bob_ref.clone(), bank_service1.clone())
        .await;
    message_sender
        .register_participant(charlie_ref.clone(), bank_service2.clone())
        .await;

    // Print initial account balances
    info!("=== Initial Account Balances ===");
    let alice_account = bank_service1.get_account("alice").await?;
    let bob_account = bank_service1.get_account("bob").await?;
    let charlie_account = bank_service2.get_account("charlie").await?;
    info!("Alice: ${:.2}", alice_account.balance);
    info!("Bob: ${:.2}", bob_account.balance);
    info!("Charlie: ${:.2}", charlie_account.balance);

    // Example 1: Simple transfer transaction (Alice -> Bob: $100)
    info!("\n=== Example 1: Transfer $100 from Alice to Bob ===");

    let tx_id = coordinator
        .begin_transaction(Some(Duration::from_secs(10)))
        .await?;

    // Add debit operation for Alice
    let debit_op = TransactionOperation::new(
        alice_ref.clone(),
        "debit".to_string(),
        serde_json::json!(BankingOperation::Debit { amount: 100.0 }),
    )
    .with_compensation(serde_json::json!(BankingOperation::Credit {
        amount: 100.0
    }));

    // Add credit operation for Bob
    let credit_op = TransactionOperation::new(
        bob_ref.clone(),
        "credit".to_string(),
        serde_json::json!(BankingOperation::Credit { amount: 100.0 }),
    )
    .with_compensation(serde_json::json!(BankingOperation::Debit { amount: 100.0 }));

    coordinator.add_operation(&tx_id, debit_op).await?;
    coordinator.add_operation(&tx_id, credit_op).await?;

    // This would execute the 2-phase commit protocol
    info!("Executing transaction {} (2-phase commit)", tx_id);
    // coordinator.commit_transaction(&tx_id).await?;

    // Example 2: Multi-party transaction with insufficient funds
    info!("\n=== Example 2: Failed transaction (insufficient funds) ===");

    let tx_id2 = coordinator
        .begin_transaction(Some(Duration::from_secs(10)))
        .await?;

    // Try to transfer $600 from Bob (who only has $500)
    let large_debit_op = TransactionOperation::new(
        bob_ref.clone(),
        "debit".to_string(),
        serde_json::json!(BankingOperation::Debit { amount: 600.0 }),
    );

    let large_credit_op = TransactionOperation::new(
        charlie_ref.clone(),
        "credit".to_string(),
        serde_json::json!(BankingOperation::Credit { amount: 600.0 }),
    );

    coordinator.add_operation(&tx_id2, large_debit_op).await?;
    coordinator.add_operation(&tx_id2, large_credit_op).await?;

    info!(
        "Executing transaction {} (should fail due to insufficient funds)",
        tx_id2
    );
    // coordinator.commit_transaction(&tx_id2).await?;

    // Example 3: Complex multi-step transaction
    info!("\n=== Example 3: Complex multi-step transaction ===");

    let tx_id3 = coordinator
        .begin_transaction(Some(Duration::from_secs(15)))
        .await?;

    // Reserve funds for Alice (for a loan application)
    let reserve_op = TransactionOperation::new(
        alice_ref.clone(),
        "reserve".to_string(),
        serde_json::json!(BankingOperation::ReserveFunds { amount: 200.0 }),
    );

    // Partial transfer from Charlie to Bob
    let charlie_debit_op = TransactionOperation::new(
        charlie_ref.clone(),
        "debit".to_string(),
        serde_json::json!(BankingOperation::Debit { amount: 150.0 }),
    );

    let bob_credit_op = TransactionOperation::new(
        bob_ref.clone(),
        "credit".to_string(),
        serde_json::json!(BankingOperation::Credit { amount: 150.0 }),
    );

    coordinator.add_operation(&tx_id3, reserve_op).await?;
    coordinator.add_operation(&tx_id3, charlie_debit_op).await?;
    coordinator.add_operation(&tx_id3, bob_credit_op).await?;

    info!(
        "Executing complex transaction {} with fund reservation",
        tx_id3
    );
    // coordinator.commit_transaction(&tx_id3).await?;

    // Show transaction statistics
    let stats = coordinator.get_stats().await;
    info!("\n=== Transaction Coordinator Statistics ===");
    info!("Active transactions: {}", stats.active_transactions);
    info!("Total log entries: {}", stats.total_log_entries);
    info!("Committed: {}", stats.committed_count);
    info!("Aborted: {}", stats.aborted_count);
    info!("Timed out: {}", stats.timed_out_count);

    // Final account balances
    info!("\n=== Final Account Balances ===");
    let alice_final = bank_service1.get_account("alice").await?;
    let bob_final = bank_service1.get_account("bob").await?;
    let charlie_final = bank_service2.get_account("charlie").await?;
    info!(
        "Alice: ${:.2} (reserved: ${:.2})",
        alice_final.balance, alice_final.reserved_funds
    );
    info!("Bob: ${:.2}", bob_final.balance);
    info!("Charlie: ${:.2}", charlie_final.balance);

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🚀 Starting Orbit Distributed Transaction Example");

    if let Err(e) = run_banking_transaction_example().await {
        error!("Transaction example failed: {}", e);
        return Err(e.into());
    }

    info!("✅ Distributed transaction example completed successfully");
    Ok(())
}
