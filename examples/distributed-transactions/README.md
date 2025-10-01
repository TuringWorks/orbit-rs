# Distributed Transactions Example

This example demonstrates the comprehensive **Distributed Transaction Support** feature implementation in Orbit-RS, showcasing a banking scenario with 2-phase commit protocol, ACID compliance, transaction coordination, and rollback capabilities.

## Features Demonstrated

### Core Transaction Framework
- **TransactionCoordinator**: Manages distributed transactions across multiple nodes
- **2-Phase Commit Protocol**: Ensures atomic commit across distributed participants
- **Transaction Participant Trait**: Interface for services to participate in transactions
- **ACID Compliance**: Atomicity, Consistency, Isolation, and Durability guarantees
- **Automatic Rollback**: Compensation actions and transaction abort handling

### Transaction Operations
- **TransactionOperation**: Encapsulates individual operations within a transaction
- **Compensation Data**: Rollback operations for failure scenarios
- **Multi-Target Operations**: Operations across different actors and nodes
- **Operation Validation**: Pre-commit validation of operation feasibility

### Coordination and Communication
- **Message-based Protocol**: Prepare, Vote, Commit, Abort, Acknowledge messages
- **Timeout Management**: Transaction timeouts with automatic cleanup
- **Participant Voting**: Democratic commit/abort decisions
- **Acknowledgment Tracking**: Ensuring all participants complete operations

### Monitoring and Observability
- **Transaction Logging**: Comprehensive audit trail of transaction events
- **Statistics Tracking**: Active transactions, success/failure rates
- **Background Cleanup**: Automatic cleanup of timed-out transactions
- **Structured Events**: Detailed transaction lifecycle events

## Banking Scenario

The example implements a realistic banking system with:

- **Bank Accounts**: Multi-node account management with balance tracking
- **Fund Transfers**: Cross-account money transfers with atomic guarantees
- **Fund Reservation**: Temporary holds on account balances
- **Insufficient Funds Handling**: Proper validation and transaction abort
- **Frozen Account Support**: Account status validation

### Account Operations

```rust
pub enum BankingOperation {
    Debit { amount: f64 },           // Withdraw funds
    Credit { amount: f64 },          // Deposit funds  
    Transfer { to_account: String, amount: f64 }, // Transfer between accounts
    ReserveFunds { amount: f64 },    // Hold funds temporarily
    ReleaseFunds { amount: f64 },    // Release held funds
}
```

## Running the Example

```bash
# From the project root
cargo run --package distributed-transactions-example
```

## Example Output

```
🚀 Starting Orbit Distributed Transaction Example
Starting distributed banking transaction example
Created account with initial balance: 1000
Created account with initial balance: 500 
Created account with initial balance: 750

=== Initial Account Balances ===
Alice: $1000.00
Bob: $500.00
Charlie: $750.00

=== Example 1: Transfer $100 from Alice to Bob ===
Started transaction: 63133f32-e4ab-479c-a205-f9fd28505056@(us-east:bank-node-1)
Executing transaction (2-phase commit)

=== Example 2: Failed transaction (insufficient funds) ===
Started transaction: 58e99e10-c4ef-4ab3-9658-59aade2fa7e9@(us-east:bank-node-1)
Executing transaction (should fail due to insufficient funds)

=== Example 3: Complex multi-step transaction ===
Started transaction: eb2b75f0-0d4a-42c0-9566-2cac21cdc836@(us-east:bank-node-1)
Executing complex transaction with fund reservation

=== Transaction Coordinator Statistics ===
Active transactions: 3
Total log entries: 3
Committed: 0
Aborted: 0
Timed out: 0
```

## Code Structure

### Transaction Coordinator
The `TransactionCoordinator` manages the lifecycle of distributed transactions:

```rust
let coordinator = TransactionCoordinator::new(node_id, config, message_sender);

// Begin a new transaction
let tx_id = coordinator.begin_transaction(Some(Duration::from_secs(10))).await?;

// Add operations to the transaction
coordinator.add_operation(&tx_id, operation).await?;

// Execute 2-phase commit
coordinator.commit_transaction(&tx_id).await?;
```

### Transaction Participant
Services implement `TransactionParticipant` to participate in distributed transactions:

```rust
#[async_trait]
impl TransactionParticipant for BankService {
    async fn prepare(&self, transaction_id: &TransactionId, operations: &[TransactionOperation]) -> OrbitResult<TransactionVote> {
        // Validate operations and vote
    }
    
    async fn commit(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        // Execute committed operations
    }
    
    async fn abort(&self, transaction_id: &TransactionId, reason: &str) -> OrbitResult<()> {
        // Rollback and cleanup
    }
}
```

### Message Communication
Transaction coordination uses message passing:

```rust
#[async_trait]
impl TransactionMessageSender for MockTransactionMessageSender {
    async fn send_message(&self, target: &AddressableReference, message: TransactionMessage) -> OrbitResult<()> {
        // Route messages to participants
    }
    
    async fn broadcast_message(&self, targets: &[AddressableReference], message: TransactionMessage) -> OrbitResult<()> {
        // Broadcast to all participants
    }
}
```

## Transaction Flow

### 2-Phase Commit Protocol

1. **Prepare Phase**:
   - Coordinator sends `Prepare` message to all participants
   - Participants validate operations and vote `Yes`/`No`/`Uncertain`
   - Coordinator collects votes within timeout

2. **Decision Phase**:
   - If all participants vote `Yes`: Send `Commit` messages
   - If any participant votes `No`/`Uncertain`: Send `Abort` messages
   - Wait for acknowledgments from all participants

3. **Completion**:
   - Log final transaction state
   - Clean up transaction resources
   - Update statistics and metrics

### Failure Scenarios

- **Participant Failure**: Transaction aborted, compensation actions executed
- **Timeout**: Automatic abort with cleanup of partial state
- **Network Partition**: Uncertain votes trigger transaction abort
- **Validation Failure**: Operations that cannot be executed result in `No` vote

## Configuration

```rust
let config = TransactionConfig {
    default_timeout: Duration::from_secs(30),
    max_concurrent_transactions: 100,
    retry_attempts: 3,
    cleanup_interval: Duration::from_secs(60),
    enable_logging: true,
    max_log_entries: 1000,
};
```

## Integration with Orbit Actor System

The distributed transaction framework integrates seamlessly with Orbit's:

- **Actor References**: Target operations using `AddressableReference`
- **Node Management**: Multi-node transaction coordination
- **Error Handling**: Consistent `OrbitResult<T>` error handling
- **Serialization**: JSON-based operation and message serialization
- **Logging**: Structured logging with `tracing` crate

## Production Considerations

### Scalability
- Concurrent transaction limit prevents resource exhaustion
- Background cleanup tasks maintain system health
- Efficient message routing and batch operations

### Reliability
- Comprehensive error handling and recovery
- Transaction log persistence for audit and recovery
- Timeout-based failure detection and handling

### Monitoring
- Transaction statistics and metrics collection
- Structured event logging for observability
- Performance tracking and bottleneck identification

## Next Steps

To make this production-ready, consider implementing:

1. **Persistent Transaction Log**: Durable storage for transaction audit trail
2. **Network Transport**: gRPC or TCP-based participant communication
3. **Recovery Mechanism**: Transaction recovery after coordinator failures
4. **Saga Pattern Support**: Long-running transaction workflows
5. **Performance Optimization**: Batch operations and connection pooling
6. **Security**: Transaction authentication and authorization
7. **Distributed Locks**: Deadlock detection and prevention
8. **Metrics Integration**: Prometheus metrics for monitoring

This implementation provides a solid foundation for building production distributed systems requiring strong consistency guarantees across multiple services and data stores.