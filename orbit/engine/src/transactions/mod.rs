// Transaction module organization
//
// This module provides a comprehensive distributed transaction system with:
// - 2-Phase Commit Protocol for ACID compliance
// - Saga Pattern for long-running workflows
// - Distributed locks with deadlock detection
// - Security features (authentication & encryption)
// - Performance optimizations (batching, pooling)
// - Metrics integration for observability

/// Core distributed transaction types and 2-phase commit
pub mod core;
/// Distributed locks with deadlock detection
pub mod locks;
/// Transaction and saga metrics collection
pub mod metrics;
/// Performance optimizations (batching, pooling)
pub mod performance;
/// Security features (authentication, authorization, audit)
pub mod security;

// Re-export core transaction types
pub use core::{
    DistributedTransaction, TransactionConfig, TransactionCoordinator, TransactionEvent,
    TransactionId, TransactionLogEntry, TransactionMessage, TransactionMessageSender,
    TransactionOperation, TransactionParticipant, TransactionState, TransactionVote,
};

// Re-export advanced features
pub use locks::{
    DeadlockCycle, DeadlockDetector, DistributedLock, DistributedLockManager, LockId,
    LockManagerConfig, LockMode, LockOwner, LockRequest, LockStatus,
};

pub use metrics::{
    LockMetrics, LockStats, SagaMetrics, SagaStats, TransactionMetrics,
    TransactionMetricsAggregator, TransactionStats,
};

pub use performance::{
    BatchConfig, BatchProcessor, BatchStats,
    // TODO: Re-enable when pooling module is added
    // ConnectionPool, ConnectionPoolConfig, PoolStats,
    ResourceManager,
};

pub use security::{
    AuditLogEntry, AuditLogger, AuthToken, AuthenticationProvider, AuthorizationProvider,
    InMemoryAuditLogger, InMemoryAuthProvider, ScopeBasedAuthorizationProvider, SecurityContext,
    TransactionPermission, TransactionSecurityManager,
};
