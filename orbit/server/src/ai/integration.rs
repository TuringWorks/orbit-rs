//! AI System Integration Example
//!
//! Example of how to integrate the AI system into the Orbit server.

use crate::ai::{
    AIMasterController, AIConfig, LearningMode, OptimizationLevel,
    IntelligentQueryOptimizer, PredictiveResourceManager, SmartStorageManager,
    AdaptiveTransactionManager,
};
use crate::ai::knowledge::AIKnowledgeBase;
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error, warn};

/// Initialize and start the AI system
pub async fn initialize_ai_system(config: Option<AIConfig>) -> Result<(Arc<AIMasterController>, JoinHandle<()>)> {
    let ai_config = config.unwrap_or_else(|| AIConfig {
        learning_mode: LearningMode::Continuous,
        optimization_level: OptimizationLevel::Balanced,
        predictive_scaling: true,
        autonomous_indexes: true,
        failure_prediction: true,
        energy_optimization: false,
    });

    info!("Initializing AI Master Controller");

    // Initialize AI Master Controller
    let ai_controller = AIMasterController::initialize(ai_config.clone()).await?;

    // Get knowledge base reference
    let knowledge_base = Arc::new(AIKnowledgeBase::new(&ai_config).await?);

    // Initialize and register subsystems
    if ai_config.autonomous_indexes {
        let query_optimizer = IntelligentQueryOptimizer::new(&ai_config, knowledge_base.clone()).await?;
        ai_controller
            .register_subsystem("query_optimizer".to_string(), Box::new(query_optimizer))
            .await?;
        info!("Registered Intelligent Query Optimizer");
    }

    if ai_config.predictive_scaling {
        let resource_manager = PredictiveResourceManager::new(&ai_config, knowledge_base.clone()).await?;
        ai_controller
            .register_subsystem("resource_manager".to_string(), Box::new(resource_manager))
            .await?;
        info!("Registered Predictive Resource Manager");
    }

    let storage_manager = SmartStorageManager::new(&ai_config, knowledge_base.clone()).await?;
    ai_controller
        .register_subsystem("storage_manager".to_string(), Box::new(storage_manager))
        .await?;
    info!("Registered Smart Storage Manager");

    let transaction_manager = AdaptiveTransactionManager::new(&ai_config, knowledge_base.clone()).await?;
    ai_controller
        .register_subsystem("transaction_manager".to_string(), Box::new(transaction_manager))
        .await?;
    info!("Registered Adaptive Transaction Manager");

    // Start AI control loop in background
    // Note: AIMasterController doesn't implement Clone, so we'll use Arc
    // For now, we'll just start the control loop directly
    let controller_arc = Arc::new(ai_controller);
    let controller_for_loop = controller_arc.clone();
    let ai_handle = tokio::spawn(async move {
        info!("Starting AI control loop");
        if let Err(e) = controller_for_loop.run_control_loop().await {
            error!(error = %e, "AI control loop error");
        }
    });

    info!("AI system initialized and running");

    // Return the Arc-wrapped controller
    Ok((controller_arc, ai_handle))
}

/// Example: Use query optimizer
pub async fn example_query_optimization(
    optimizer: &IntelligentQueryOptimizer,
    query: &str,
) -> Result<()> {
    info!("Optimizing query: {}", query);

    let optimized = optimizer.optimize_query(query).await?;

    info!(
        original_query = %optimized.original_query,
        estimated_improvement = %optimized.optimized_plan.estimated_improvement,
        confidence = %optimized.optimized_plan.confidence,
        "Query optimized"
    );

    if !optimized.recommended_indexes.is_empty() {
        info!(
            indexes = ?optimized.recommended_indexes,
            "Index recommendations generated"
        );
    }

    Ok(())
}

/// Example: Use workload predictor
pub async fn example_workload_prediction(
    resource_manager: &PredictiveResourceManager,
) -> Result<()> {
    let forecast = resource_manager
        .forecast_workload(tokio::time::Duration::from_secs(3600))
        .await?;

    info!(
        predicted_cpu = %forecast.predicted_cpu,
        predicted_memory = %forecast.predicted_memory,
        predicted_io = %forecast.predicted_io,
        confidence = %forecast.confidence,
        "Workload forecast generated"
    );

    Ok(())
}

/// Example: Use auto-tiering
pub async fn example_auto_tiering(
    storage_manager: &SmartStorageManager,
) -> Result<()> {
    let decisions = storage_manager.generate_tiering_decisions().await?;

    info!(
        decision_count = decisions.len(),
        "Generated tiering decisions"
    );

    for decision in decisions {
        info!(
            data_id = %decision.data_identifier,
            from_tier = ?decision.source_tier,
            to_tier = ?decision.target_tier,
            benefit = %decision.estimated_benefit,
            "Tiering decision"
        );
    }

    Ok(())
}

/// Example: Use deadlock prevention
pub async fn example_deadlock_prevention(
    transaction_manager: &AdaptiveTransactionManager,
    dependency_graph: &crate::ai::transaction::TransactionDependencyGraph,
) -> Result<()> {
    let predictions = transaction_manager.prevent_deadlocks(dependency_graph).await?;

    if !predictions.is_empty() {
        warn!(
            prediction_count = predictions.len(),
            "Deadlock predictions detected"
        );

        for prediction in predictions {
            info!(
                cycle = ?prediction.cycle,
                probability = %prediction.probability,
                impact = %prediction.impact_score,
                "Deadlock prediction"
            );
        }
    }

    Ok(())
}

