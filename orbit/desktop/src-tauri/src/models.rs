//! ML model catalogue.
//!
//! [`ModelManager::get_ml_functions`] returns a static reference list of the ML
//! SQL functions Orbit-RS exposes — documentation, not measurements.
//!
//! The per-connection operations are not implemented: nothing here queries a
//! server for the models it actually holds. They report that plainly instead of
//! returning invented models, because a fabricated accuracy figure or a delete
//! that reports success without deleting anything is worse than a visible gap.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Shared message for the operations that have no implementation behind them.
const NOT_IMPLEMENTED: &str =
    "Model management is not implemented in this build: the desktop app has no client for \
     Orbit's model catalogue yet. Use ML SQL functions from the query editor instead.";

#[derive(Debug, Serialize, Deserialize)]
pub struct MLModel {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub status: ModelStatus,
    pub accuracy: Option<f64>,
    pub created_at: String,
    pub last_trained: Option<String>,
    pub features: Vec<String>,
    pub target: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ModelStatus {
    Training,
    Ready,
    Error,
    Deleted,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MLFunction {
    pub name: String,
    pub category: String,
    pub description: String,
    pub parameters: Vec<FunctionParameter>,
    pub example: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

// Type alias for compatibility
pub type ModelInfo = MLModel;
pub type MLFunctionInfo = MLFunction;

// Model manager for handling ML models
#[derive(Default)]
pub struct ModelManager;

impl ModelManager {
    pub fn new() -> Self {
        ModelManager
    }

    /// Models registered on a connection.
    ///
    /// # Errors
    /// Always: there is no catalogue client behind this yet. Returning an error
    /// keeps invented models off the screen.
    pub async fn get_models(&self, _connection_id: &str) -> Result<Vec<MLModel>, String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    /// The ML SQL functions Orbit-RS exposes, as reference documentation.
    pub async fn get_ml_functions(&self) -> Result<Vec<MLFunction>, String> {
        let functions = vec![
            MLFunction {
                name: "ML_XGBOOST".to_string(),
                category: "Boosting".to_string(),
                description:
                    "XGBoost gradient boosting algorithm for classification and regression"
                        .to_string(),
                parameters: vec![
                    FunctionParameter {
                        name: "features".to_string(),
                        param_type: "ARRAY".to_string(),
                        required: true,
                        description: "Array of feature columns".to_string(),
                    },
                    FunctionParameter {
                        name: "target".to_string(),
                        param_type: "COLUMN".to_string(),
                        required: true,
                        description: "Target column for prediction".to_string(),
                    },
                ],
                example: "SELECT ML_XGBOOST(ARRAY[age, income], approved) FROM loans;".to_string(),
            },
            MLFunction {
                name: "ML_LIGHTGBM".to_string(),
                category: "Boosting".to_string(),
                description: "LightGBM gradient boosting framework".to_string(),
                parameters: vec![
                    FunctionParameter {
                        name: "features".to_string(),
                        param_type: "ARRAY".to_string(),
                        required: true,
                        description: "Array of feature columns".to_string(),
                    },
                    FunctionParameter {
                        name: "target".to_string(),
                        param_type: "COLUMN".to_string(),
                        required: true,
                        description: "Target column for prediction".to_string(),
                    },
                ],
                example: "SELECT ML_LIGHTGBM(ARRAY[feature1, feature2], target) FROM data;"
                    .to_string(),
            },
            MLFunction {
                name: "ML_CATBOOST".to_string(),
                category: "Boosting".to_string(),
                description: "CatBoost gradient boosting with categorical feature support"
                    .to_string(),
                parameters: vec![
                    FunctionParameter {
                        name: "features".to_string(),
                        param_type: "ARRAY".to_string(),
                        required: true,
                        description: "Array of feature columns".to_string(),
                    },
                    FunctionParameter {
                        name: "target".to_string(),
                        param_type: "COLUMN".to_string(),
                        required: true,
                        description: "Target column for prediction".to_string(),
                    },
                ],
                example:
                    "SELECT ML_CATBOOST(ARRAY[cat_feature, num_feature], outcome) FROM dataset;"
                        .to_string(),
            },
        ];

        Ok(functions)
    }

    /// Delete a model.
    ///
    /// # Errors
    /// Always. Reporting success for a deletion that never happened would tell
    /// the user a model is gone while it is still there.
    pub async fn delete_model(&self, _connection_id: &str, _model_id: &str) -> Result<(), String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    /// Details for one model.
    ///
    /// # Errors
    /// Always; see [`ModelManager::get_models`].
    pub async fn get_model_info(
        &self,
        _connection_id: &str,
        _model_name: &str,
    ) -> Result<MLModel, String> {
        Err(NOT_IMPLEMENTED.to_string())
    }
}
