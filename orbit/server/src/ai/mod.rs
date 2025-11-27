//! AI-Native Database Features for Orbit-RS
//!
//! This module provides comprehensive AI capabilities that embed artificial intelligence
//! deeply into the database architecture, enabling autonomous optimization, intelligent
//! data management, predictive scaling, and ML-powered query acceleration.
//!
//! ## Architecture
//!
//! The AI system consists of:
//! - **AI Master Controller**: Central orchestrator for all AI features
//! - **Knowledge Base**: Stores patterns, models, and decision history
//! - **Decision Engine**: Policy and rule-based decision making
//! - **Learning Engine**: Continuous model improvement
//! - **Intelligent Subsystems**: Query optimizer, storage manager, resource manager, transaction manager

pub mod controller;
pub mod decision;
pub mod knowledge;
pub mod learning;
pub mod optimizer;
pub mod resource;
pub mod storage;
pub mod transaction;

// Integration module (available by default)
pub mod integration;

// Re-export optimizer submodules
pub use optimizer::cost_model::{ExecutionCost, ExecutionMetrics, QueryPlan};
pub use optimizer::index_advisor::{IndexRecommendation, IndexType};
pub use optimizer::pattern_classifier::{OptimizationStrategy, QueryClass};

pub use controller::{AIConfig, AIMasterController, SystemState};
pub use decision::{AIDecision, DecisionEngine};
pub use knowledge::{AIKnowledgeBase, KnowledgePattern};
pub use learning::{LearningConfig, LearningEngine};
pub use optimizer::{IntelligentQueryOptimizer, OptimizedPlan, OptimizedQuery};
pub use resource::{
    PredictiveResourceManager, ResourceDemand, WorkloadForecast, WorkloadPredictor,
};
pub use storage::{AutoTieringEngine, SmartStorageManager, StorageTier, TieringDecision};
pub use transaction::{
    AdaptiveTransactionManager, DeadlockPrediction, DeadlockPreventer, IsolationLevel,
    ResolutionAction, TransactionDependencyGraph, TransactionId,
};

use anyhow::Result as OrbitResult;

/// Common AI subsystem trait
#[async_trait::async_trait]
pub trait AISubsystem: Send + Sync {
    /// Handle an AI decision
    async fn handle_decision(&self, decision: AIDecision) -> OrbitResult<()>;

    /// Get subsystem metrics
    async fn get_metrics(&self) -> OrbitResult<AISubsystemMetrics>;

    /// Shutdown subsystem
    async fn shutdown(&self) -> OrbitResult<()>;
}

/// Metrics for AI subsystems
#[derive(Debug, Clone)]
pub struct AISubsystemMetrics {
    pub decisions_made: u64,
    pub optimizations_applied: u64,
    pub prediction_accuracy: f64,
    pub average_benefit: f64,
}

/// Learning mode for AI system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningMode {
    /// Continuous learning from all operations
    Continuous,
    /// Lightweight learning for resource-constrained environments
    Lightweight,
    /// Per-tenant learning for multi-tenant deployments
    PerTenant,
    /// Disabled - no learning
    Disabled,
}

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// Aggressive optimization
    Aggressive,
    /// Balanced optimization
    Balanced,
    /// Conservative optimization
    Conservative,
}
