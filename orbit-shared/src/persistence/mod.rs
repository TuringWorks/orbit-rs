//! Persistence layer abstractions and traits
//!
//! This module provides trait-based abstractions for implementing persistence
//! providers with minimal code duplication.

pub mod snapshot;
pub mod traits;

// Re-export snapshot types for backward compatibility
pub use snapshot::*;

// Re-export specific traits to avoid ambiguity
pub use traits::{
    AutoMetrics, BulkOperations, DataDirectoryProvider, IsolationLevel, KeyEncoder,
    MetricsCollector, ProviderHealth, ProviderLifecycle, TransactionContext, TransactionId,
    TransactionManager,
};
