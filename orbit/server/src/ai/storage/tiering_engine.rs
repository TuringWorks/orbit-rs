//! Auto-Tiering Engine
//!
//! Automatic data tiering based on ML-learned access patterns.

use anyhow::Result as OrbitResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Access pattern for data tiering analysis
#[derive(Debug, Clone)]
pub struct AccessPattern {
    pub data_id: String,
    pub table: String,
    pub current_tier: StorageTier,
    pub read_count: u64,
    pub write_count: u64,
    pub last_access: std::time::SystemTime,
    pub access_frequency: f64, // Accesses per hour
    pub data_size: u64,
}

/// Storage tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTier {
    Hot,     // Fast, expensive storage
    Warm,    // Medium speed, medium cost
    Cold,    // Slow, cheap storage
    Archive, // Very slow, very cheap storage
}

/// Tiering decision
#[derive(Debug, Clone)]
pub struct TieringDecision {
    pub data_identifier: String,
    pub source_tier: StorageTier,
    pub target_tier: StorageTier,
    pub migration_priority: u32,
    pub estimated_benefit: f64,
    pub migration_cost: f64,
    pub confidence: f64,
}

/// Access pattern classification
#[derive(Debug, Clone)]
pub enum AccessClassification {
    Hot,     // Very frequent access
    Warm,    // Moderate access
    Cold,    // Infrequent access
    Archive, // Rarely accessed
}

/// Automatic data tiering engine
pub struct AutoTieringEngine {
    /// Access pattern analyzer
    access_patterns: Arc<RwLock<HashMap<String, AccessPattern>>>,
    /// Tiering cost analyzer
    cost_analyzer: Arc<TieringCostAnalyzer>,
}

/// Tiering cost analyzer
pub struct TieringCostAnalyzer {
    /// Cost per GB per tier
    tier_costs: HashMap<StorageTier, f64>,
    /// Migration costs
    migration_costs: HashMap<(StorageTier, StorageTier), f64>,
}

impl TieringCostAnalyzer {
    pub fn new() -> Self {
        let mut tier_costs = HashMap::new();
        tier_costs.insert(StorageTier::Hot, 0.10); // $0.10/GB/month
        tier_costs.insert(StorageTier::Warm, 0.05); // $0.05/GB/month
        tier_costs.insert(StorageTier::Cold, 0.01); // $0.01/GB/month
        tier_costs.insert(StorageTier::Archive, 0.001); // $0.001/GB/month

        let mut migration_costs = HashMap::new();
        // Migration costs (one-time)
        migration_costs.insert((StorageTier::Hot, StorageTier::Warm), 0.01);
        migration_costs.insert((StorageTier::Hot, StorageTier::Cold), 0.02);
        migration_costs.insert((StorageTier::Warm, StorageTier::Cold), 0.01);
        migration_costs.insert((StorageTier::Warm, StorageTier::Hot), 0.01);
        migration_costs.insert((StorageTier::Cold, StorageTier::Warm), 0.01);
        migration_costs.insert((StorageTier::Cold, StorageTier::Hot), 0.02);

        Self {
            tier_costs,
            migration_costs,
        }
    }

    /// Analyze cost-benefit of tier migration
    pub fn analyze_migration(
        &self,
        pattern: &AccessPattern,
        target_tier: StorageTier,
    ) -> MigrationAnalysis {
        let current_cost = self.tier_costs.get(&pattern.current_tier).unwrap_or(&0.0);
        let target_cost = self.tier_costs.get(&target_tier).unwrap_or(&0.0);

        let monthly_cost_savings =
            (current_cost - target_cost) * (pattern.data_size as f64 / 1_000_000_000.0);

        let migration_cost = self
            .migration_costs
            .get(&(pattern.current_tier, target_tier))
            .copied()
            .unwrap_or(0.0)
            * (pattern.data_size as f64 / 1_000_000_000.0);

        let net_benefit = monthly_cost_savings - migration_cost;

        // Payback period in months
        let payback_period = if monthly_cost_savings > 0.0 {
            migration_cost / monthly_cost_savings
        } else {
            f64::INFINITY
        };

        MigrationAnalysis {
            net_benefit,
            monthly_savings: monthly_cost_savings,
            migration_cost,
            payback_period,
            confidence: if payback_period < 3.0 {
                0.9
            } else if payback_period < 6.0 {
                0.7
            } else {
                0.5
            },
        }
    }
}

/// Migration analysis
#[derive(Debug, Clone)]
pub struct MigrationAnalysis {
    pub net_benefit: f64,
    pub monthly_savings: f64,
    pub migration_cost: f64,
    pub payback_period: f64, // months
    pub confidence: f64,
}

impl AutoTieringEngine {
    /// Create a new auto-tiering engine
    pub fn new() -> Self {
        Self {
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
            cost_analyzer: Arc::new(TieringCostAnalyzer::new()),
        }
    }

    /// Record data access
    pub async fn record_access(
        &self,
        data_id: String,
        table: String,
        is_write: bool,
        data_size: u64,
    ) -> OrbitResult<()> {
        let mut patterns = self.access_patterns.write().await;

        let pattern = patterns.entry(data_id.clone()).or_insert_with(|| {
            AccessPattern {
                data_id: data_id.clone(),
                table,
                current_tier: StorageTier::Hot, // Default to hot
                read_count: 0,
                write_count: 0,
                last_access: std::time::SystemTime::now(),
                access_frequency: 0.0,
                data_size,
            }
        });

        if is_write {
            pattern.write_count += 1;
        } else {
            pattern.read_count += 1;
        }
        pattern.last_access = std::time::SystemTime::now();

        // Update access frequency
        self.update_access_frequency(pattern).await?;

        Ok(())
    }

    /// Update access frequency for a pattern
    async fn update_access_frequency(&self, pattern: &mut AccessPattern) -> OrbitResult<()> {
        let now = std::time::SystemTime::now();
        let elapsed = now.duration_since(pattern.last_access).unwrap_or_default();
        let hours_elapsed = elapsed.as_secs() as f64 / 3600.0;

        if hours_elapsed > 0.0 {
            let total_accesses = pattern.read_count + pattern.write_count;
            pattern.access_frequency = total_accesses as f64 / hours_elapsed.max(1.0);
        }

        Ok(())
    }

    /// Generate tiering decisions based on access patterns
    pub async fn generate_tiering_decisions(&self) -> OrbitResult<Vec<TieringDecision>> {
        let patterns = self.access_patterns.read().await;
        let mut decisions = Vec::new();

        for pattern in patterns.values() {
            // Classify access pattern
            let classification = self.classify_access_pattern(pattern);

            // Determine optimal tier
            let optimal_tier = self.determine_optimal_tier(&classification, pattern);

            // Check if migration is needed
            if pattern.current_tier != optimal_tier {
                // Analyze cost-benefit
                let analysis = self.cost_analyzer.analyze_migration(pattern, optimal_tier);

                if analysis.net_benefit > 0.0 && analysis.confidence > 0.7 {
                    decisions.push(TieringDecision {
                        data_identifier: pattern.data_id.clone(),
                        source_tier: pattern.current_tier,
                        target_tier: optimal_tier,
                        migration_priority: self.calculate_priority(&analysis, pattern),
                        estimated_benefit: analysis.net_benefit,
                        migration_cost: analysis.migration_cost,
                        confidence: analysis.confidence,
                    });
                }
            }
        }

        // Sort by priority
        decisions.sort_by(|a, b| b.migration_priority.cmp(&a.migration_priority));

        info!(
            decision_count = decisions.len(),
            "Generated tiering decisions"
        );

        Ok(decisions)
    }

    /// Classify access pattern
    fn classify_access_pattern(&self, pattern: &AccessPattern) -> AccessClassification {
        // Classify based on access frequency
        if pattern.access_frequency > 100.0 {
            AccessClassification::Hot
        } else if pattern.access_frequency > 10.0 {
            AccessClassification::Warm
        } else if pattern.access_frequency > 1.0 {
            AccessClassification::Cold
        } else {
            AccessClassification::Archive
        }
    }

    /// Determine optimal tier for access pattern
    fn determine_optimal_tier(
        &self,
        classification: &AccessClassification,
        _pattern: &AccessPattern,
    ) -> StorageTier {
        match classification {
            AccessClassification::Hot => StorageTier::Hot,
            AccessClassification::Warm => StorageTier::Warm,
            AccessClassification::Cold => StorageTier::Cold,
            AccessClassification::Archive => StorageTier::Archive,
        }
    }

    /// Calculate migration priority
    fn calculate_priority(&self, analysis: &MigrationAnalysis, pattern: &AccessPattern) -> u32 {
        // Higher priority for:
        // - Higher net benefit
        // - Larger data size
        // - Shorter payback period

        let benefit_score = (analysis.net_benefit * 1000.0) as u32;
        let size_score = (pattern.data_size / 1_000_000) as u32; // MB
        let payback_score = if analysis.payback_period < 1.0 {
            100
        } else if analysis.payback_period < 3.0 {
            50
        } else {
            10
        };

        benefit_score + size_score + payback_score
    }
}

impl Default for AutoTieringEngine {
    fn default() -> Self {
        Self::new()
    }
}
