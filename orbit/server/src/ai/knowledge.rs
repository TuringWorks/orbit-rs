//! AI Knowledge Base
//!
//! Stores patterns, models, and decision history for the AI system.

use crate::ai::{SystemState, LearningMode};
use crate::ai::controller::AIConfig;
use anyhow::Result as OrbitResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Knowledge base storing patterns, models, and decisions
pub struct AIKnowledgeBase {
    /// Stored patterns for query optimization, storage, etc.
    patterns: Arc<RwLock<HashMap<String, KnowledgePattern>>>,
    /// Historical observations
    observations: Arc<RwLock<Vec<SystemObservation>>>,
    /// Configuration
    _config: AIConfig,
    /// Maximum number of observations to keep
    max_observations: usize,
}

/// Knowledge pattern stored in the knowledge base
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub features: HashMap<String, f64>,
    pub outcome: PatternOutcome,
    pub confidence: f64,
    pub last_updated: std::time::SystemTime,
    pub usage_count: u64,
}

/// Type of pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    QueryPattern,
    StoragePattern,
    ResourcePattern,
    TransactionPattern,
    FailurePattern,
}

/// Outcome of a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternOutcome {
    pub success: bool,
    pub performance_improvement: f64,
    pub resource_savings: f64,
    pub details: HashMap<String, String>,
}

/// System observation for learning
#[derive(Debug, Clone)]
pub struct SystemObservation {
    pub state: SystemState,
    pub decisions_made: Vec<String>,
    pub outcomes: Vec<ObservationOutcome>,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct ObservationOutcome {
    pub decision_id: String,
    pub success: bool,
    pub actual_benefit: f64,
    pub predicted_benefit: f64,
}

impl AIKnowledgeBase {
    /// Create a new knowledge base
    pub async fn new(config: &AIConfig) -> OrbitResult<Self> {
        info!("Initializing AI Knowledge Base");

        let max_observations = match config.learning_mode {
            LearningMode::Continuous => 10000,
            LearningMode::Lightweight => 1000,
            LearningMode::PerTenant => 5000,
            LearningMode::Disabled => 0,
        };

        Ok(Self {
            patterns: Arc::new(RwLock::new(HashMap::new())),
            observations: Arc::new(RwLock::new(Vec::new())),
            _config: config.clone(),
            max_observations,
        })
    }

    /// Update knowledge base with new observations
    pub async fn update_observations(&self, state: &SystemState) -> OrbitResult<()> {
        let observation = SystemObservation {
            state: state.clone(),
            decisions_made: Vec::new(),
            outcomes: Vec::new(),
            timestamp: std::time::SystemTime::now(),
        };

        let mut observations = self.observations.write().await;
        observations.push(observation);

        // Trim old observations if we exceed the limit
        let current_len = observations.len();
        if current_len > self.max_observations {
            let remove_count = current_len - self.max_observations;
            observations.drain(0..remove_count);
        }

        debug!(
            observation_count = observations.len(),
            "Updated knowledge base observations"
        );

        Ok(())
    }

    /// Store a knowledge pattern
    pub async fn store_pattern(&self, pattern: KnowledgePattern) -> OrbitResult<()> {
        let mut patterns = self.patterns.write().await;
        patterns.insert(pattern.pattern_id.clone(), pattern.clone());

        debug!(
            pattern_id = %pattern.pattern_id,
            pattern_type = ?pattern.pattern_type,
            "Stored knowledge pattern"
        );

        Ok(())
    }

    /// Retrieve a pattern by ID
    pub async fn get_pattern(&self, pattern_id: &str) -> Option<KnowledgePattern> {
        let patterns = self.patterns.read().await;
        patterns.get(pattern_id).cloned()
    }

    /// Find similar patterns
    pub async fn find_similar_patterns(
        &self,
        pattern_type: PatternType,
        features: &HashMap<String, f64>,
        max_results: usize,
    ) -> Vec<KnowledgePattern> {
        let patterns = self.patterns.read().await;
        let mut candidates: Vec<_> = patterns
            .values()
            .filter(|p| p.pattern_type == pattern_type)
            .cloned()
            .collect();

        // Simple similarity based on feature overlap
        // TODO: Implement more sophisticated similarity metric
        candidates.sort_by(|a, b| {
            let similarity_a = Self::calculate_similarity(features, &a.features);
            let similarity_b = Self::calculate_similarity(features, &b.features);
            similarity_b.partial_cmp(&similarity_a).unwrap()
        });

        candidates.into_iter().take(max_results).collect()
    }

    /// Calculate similarity between two feature sets
    fn calculate_similarity(features1: &HashMap<String, f64>, features2: &HashMap<String, f64>) -> f64 {
        let mut common_features = 0;
        let mut total_diff = 0.0;

        for (key, value1) in features1 {
            if let Some(value2) = features2.get(key) {
                common_features += 1;
                total_diff += (value1 - value2).abs();
            }
        }

        if common_features == 0 {
            return 0.0;
        }

        let avg_diff = total_diff / common_features as f64;
        1.0 / (1.0 + avg_diff) // Convert to similarity score
    }

    /// Get recent observations for learning
    pub async fn get_recent_observations(
        &self,
        count: usize,
    ) -> Vec<SystemObservation> {
        let observations = self.observations.read().await;
        let start = observations.len().saturating_sub(count);
        observations[start..].to_vec()
    }

    /// Update pattern with new outcome
    pub async fn update_pattern_outcome(
        &self,
        pattern_id: &str,
        outcome: PatternOutcome,
    ) -> OrbitResult<()> {
        let mut patterns = self.patterns.write().await;
        if let Some(pattern) = patterns.get_mut(pattern_id) {
            pattern.outcome = outcome;
            pattern.last_updated = std::time::SystemTime::now();
            pattern.usage_count += 1;
        }

        Ok(())
    }

    /// Get statistics about stored patterns
    pub async fn get_statistics(&self) -> KnowledgeBaseStatistics {
        let patterns = self.patterns.read().await;
        let observations = self.observations.read().await;

        KnowledgeBaseStatistics {
            total_patterns: patterns.len(),
            pattern_types: patterns
                .values()
                .fold(HashMap::new(), |mut acc, p| {
                    *acc.entry(format!("{:?}", p.pattern_type)).or_insert(0) += 1;
                    acc
                }),
            total_observations: observations.len(),
            average_confidence: patterns
                .values()
                .map(|p| p.confidence)
                .sum::<f64>()
                / patterns.len().max(1) as f64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KnowledgeBaseStatistics {
    pub total_patterns: usize,
    pub pattern_types: HashMap<String, usize>,
    pub total_observations: usize,
    pub average_confidence: f64,
}

