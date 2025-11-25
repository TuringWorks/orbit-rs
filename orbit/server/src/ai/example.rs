//! Example usage of AI-Native Database Features
//!
//! This module demonstrates how to initialize and use the AI system.

use crate::ai::{AIMasterController, AIConfig, LearningMode, OptimizationLevel};
use anyhow::Result;

/// Example: Initialize AI system with default configuration
pub async fn example_initialize_ai() -> Result<AIMasterController> {
    let config = AIConfig {
        learning_mode: LearningMode::Continuous,
        optimization_level: OptimizationLevel::Balanced,
        predictive_scaling: true,
        autonomous_indexes: true,
        failure_prediction: true,
        energy_optimization: false,
    };

    let ai_controller = AIMasterController::initialize(config).await?;

    // Register subsystems (these would be initialized separately)
    // ai_controller.register_subsystem("query_optimizer", ...).await?;
    // ai_controller.register_subsystem("storage_manager", ...).await?;
    // ai_controller.register_subsystem("resource_manager", ...).await?;
    // ai_controller.register_subsystem("transaction_manager", ...).await?;

    Ok(ai_controller)
}

/// Example: Start AI control loop in background
pub async fn example_start_ai_loop(controller: &AIMasterController) -> Result<()> {
    // Start control loop in background task
    let controller_clone = controller.clone();
    tokio::spawn(async move {
        if let Err(e) = controller_clone.run_control_loop().await {
            tracing::error!(error = %e, "AI control loop error");
        }
    });

    Ok(())
}

