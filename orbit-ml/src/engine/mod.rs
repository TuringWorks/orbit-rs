//! Core ML engine for Orbit database.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::MLConfig;
use crate::error::{MLError, Result};
use crate::models::{Model, ModelMetadata, ModelRegistry};
use crate::training::{TrainingConfig, TrainingJob, TrainingStatus};
use crate::inference::{InferenceConfig, InferenceJob, InferenceResult};

pub mod builder;
pub mod factory;
pub mod manager;
pub mod scheduler;

pub use builder::MLEngineBuilder;
pub use factory::ModelFactory;
pub use manager::ModelManager;
pub use scheduler::JobScheduler;

/// Main ML engine for Orbit database
#[derive(Debug)]
pub struct MLEngine {
    /// Engine configuration
    config: MLConfig,
    
    /// Model registry for managing trained models
    model_registry: Arc<ModelRegistry>,
    
    /// Model manager for lifecycle operations
    model_manager: Arc<ModelManager>,
    
    /// Job scheduler for training and inference
    job_scheduler: Arc<JobScheduler>,
    
    /// Active training jobs
    training_jobs: Arc<DashMap<Uuid, TrainingJob>>,
    
    /// Active inference jobs
    inference_jobs: Arc<DashMap<Uuid, InferenceJob>>,
    
    /// Engine metrics
    metrics: Arc<RwLock<EngineMetrics>>,
}

/// Engine performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EngineMetrics {
    pub models_trained: u64,
    pub models_loaded: u64,
    pub inferences_completed: u64,
    pub training_jobs_active: u64,
    pub inference_jobs_active: u64,
    pub memory_usage_bytes: u64,
    pub gpu_utilization_percent: f64,
    pub average_training_time_ms: f64,
    pub average_inference_time_ms: f64,
    pub error_count: u64,
}

#[async_trait]
pub trait MLEngineInterface {
    /// Train a model asynchronously
    async fn train_model(
        &self,
        model_name: String,
        model_type: String,
        training_data: Vec<u8>,
        config: TrainingConfig,
    ) -> Result<TrainingJob>;

    /// Load a trained model
    async fn load_model(&self, model_name: &str) -> Result<Arc<dyn Model>>;

    /// Run inference on a model
    async fn predict(
        &self,
        model_name: &str,
        input_data: Vec<u8>,
        config: InferenceConfig,
    ) -> Result<InferenceResult>;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelMetadata>>;

    /// Delete a model
    async fn delete_model(&self, model_name: &str) -> Result<()>;

    /// Get engine metrics
    async fn get_metrics(&self) -> Result<EngineMetrics>;

    /// Health check
    async fn health_check(&self) -> Result<EngineStatus>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_jobs: u64,
    pub loaded_models: u64,
    pub memory_usage_mb: f64,
    pub gpu_available: bool,
    pub features_enabled: Vec<String>,
}

impl MLEngine {
    /// Create a new ML engine
    pub async fn new() -> Result<Self> {
        Self::with_config(MLConfig::default()).await
    }

    /// Create ML engine with custom configuration
    pub async fn with_config(config: MLConfig) -> Result<Self> {
        info!("Initializing ML engine with config: {:?}", config);

        let model_registry = Arc::new(ModelRegistry::new().await?);
        let model_manager = Arc::new(ModelManager::new(Arc::clone(&model_registry)).await?);
        let job_scheduler = Arc::new(JobScheduler::new(&config).await?);

        let engine = Self {
            config,
            model_registry,
            model_manager,
            job_scheduler,
            training_jobs: Arc::new(DashMap::new()),
            inference_jobs: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(EngineMetrics::default())),
        };

        // Initialize subsystems
        engine.initialize_subsystems().await?;

        info!("ML engine initialized successfully");
        Ok(engine)
    }

    /// Initialize engine subsystems
    async fn initialize_subsystems(&self) -> Result<()> {
        // Initialize model factory
        debug!("Initializing model factory");

        // Initialize GPU if available
        #[cfg(feature = "gpu")]
        {
            if self.config.enable_gpu {
                debug!("Initializing GPU support");
                self.initialize_gpu().await?;
            }
        }

        // Initialize distributed computing if enabled
        #[cfg(feature = "distributed")]
        {
            if self.config.enable_distributed {
                debug!("Initializing distributed computing");
                self.initialize_distributed().await?;
            }
        }

        // Initialize multi-language runtimes
        self.initialize_multi_language().await?;

        Ok(())
    }

    /// Initialize GPU support
    #[cfg(feature = "gpu")]
    async fn initialize_gpu(&self) -> Result<()> {
        use candle_core::Device;
        
        match Device::cuda_if_available(0) {
            Ok(_device) => {
                info!("GPU support initialized successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to initialize GPU: {}", e);
                Err(MLError::gpu(format!("GPU initialization failed: {}", e)))
            }
        }
    }

    /// Initialize distributed computing
    #[cfg(feature = "distributed")]
    async fn initialize_distributed(&self) -> Result<()> {
        debug!("Setting up distributed computing environment");
        // TODO: Initialize MPI or distributed training framework
        Ok(())
    }

    /// Initialize multi-language runtimes
    async fn initialize_multi_language(&self) -> Result<()> {
        #[cfg(feature = "python")]
        {
            debug!("Initializing Python runtime");
            // TODO: Initialize Python interpreter
        }

        #[cfg(feature = "javascript")]
        {
            debug!("Initializing JavaScript runtime");
            // TODO: Initialize V8/Deno runtime
        }

        #[cfg(feature = "lua")]
        {
            debug!("Initializing Lua runtime");
            // TODO: Initialize Lua runtime
        }

        Ok(())
    }

    /// Get engine builder
    pub fn builder() -> MLEngineBuilder {
        MLEngineBuilder::default()
    }
}

#[async_trait]
impl MLEngineInterface for MLEngine {
    async fn train_model(
        &self,
        model_name: String,
        model_type: String,
        training_data: Vec<u8>,
        config: TrainingConfig,
    ) -> Result<TrainingJob> {
        info!("Starting training job for model: {}", model_name);

        let job_id = Uuid::new_v4();
        let job = TrainingJob {
            id: job_id,
            model_name: model_name.clone(),
            model_type,
            status: TrainingStatus::Queued,
            config,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            progress: 0.0,
            loss: None,
            metrics: HashMap::new(),
        };

        // Store job
        self.training_jobs.insert(job_id, job.clone());

        // Schedule job
        self.job_scheduler.schedule_training_job(job_id, training_data).await?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.training_jobs_active += 1;
        }

        debug!("Training job {} queued for model {}", job_id, model_name);
        Ok(job)
    }

    async fn load_model(&self, model_name: &str) -> Result<Arc<dyn Model>> {
        debug!("Loading model: {}", model_name);

        let model = self.model_manager.load_model(model_name).await?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.models_loaded += 1;
        }

        info!("Model {} loaded successfully", model_name);
        Ok(model)
    }

    async fn predict(
        &self,
        model_name: &str,
        input_data: Vec<u8>,
        config: InferenceConfig,
    ) -> Result<InferenceResult> {
        debug!("Running inference on model: {}", model_name);

        let job_id = Uuid::new_v4();
        let job = InferenceJob {
            id: job_id,
            model_name: model_name.to_string(),
            input_data,
            config,
            created_at: chrono::Utc::now(),
        };

        // Store job
        self.inference_jobs.insert(job_id, job);

        // Run inference
        let result = self.job_scheduler.run_inference(job_id).await?;

        // Clean up job
        self.inference_jobs.remove(&job_id);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.inferences_completed += 1;
        }

        debug!("Inference completed for model: {}", model_name);
        Ok(result)
    }

    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        debug!("Listing available models");
        self.model_registry.list_models().await
    }

    async fn delete_model(&self, model_name: &str) -> Result<()> {
        info!("Deleting model: {}", model_name);
        self.model_manager.delete_model(model_name).await
    }

    async fn get_metrics(&self) -> Result<EngineMetrics> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    async fn health_check(&self) -> Result<EngineStatus> {
        let metrics = self.metrics.read().await;
        let features_enabled = self.get_enabled_features();
        
        Ok(EngineStatus {
            status: "healthy".to_string(),
            version: crate::VERSION.to_string(),
            uptime_seconds: 0, // TODO: Track actual uptime
            active_jobs: metrics.training_jobs_active + metrics.inference_jobs_active,
            loaded_models: metrics.models_loaded,
            memory_usage_mb: metrics.memory_usage_bytes as f64 / 1024.0 / 1024.0,
            gpu_available: cfg!(feature = "gpu"),
            features_enabled,
        })
    }
}

impl MLEngine {
    /// Get list of enabled features
    fn get_enabled_features(&self) -> Vec<String> {
        let mut features = Vec::new();
        
        if cfg!(feature = "neural-networks") {
            features.push("neural-networks".to_string());
        }
        if cfg!(feature = "transformers") {
            features.push("transformers".to_string());
        }
        if cfg!(feature = "graph-neural-networks") {
            features.push("graph-neural-networks".to_string());
        }
        if cfg!(feature = "python") {
            features.push("python".to_string());
        }
        if cfg!(feature = "javascript") {
            features.push("javascript".to_string());
        }
        if cfg!(feature = "lua") {
            features.push("lua".to_string());
        }
        if cfg!(feature = "gpu") {
            features.push("gpu".to_string());
        }
        if cfg!(feature = "distributed") {
            features.push("distributed".to_string());
        }
        
        features
    }

    /// Shutdown the engine gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ML engine");

        // Cancel active jobs
        for entry in self.training_jobs.iter() {
            let job_id = *entry.key();
            warn!("Cancelling training job: {}", job_id);
        }

        for entry in self.inference_jobs.iter() {
            let job_id = *entry.key();
            warn!("Cancelling inference job: {}", job_id);
        }

        // Shutdown subsystems
        // TODO: Implement proper shutdown for all subsystems

        info!("ML engine shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = MLEngine::new().await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let engine = MLEngine::new().await.unwrap();
        let status = engine.health_check().await.unwrap();
        assert_eq!(status.status, "healthy");
    }

    #[tokio::test]
    async fn test_list_models_empty() {
        let engine = MLEngine::new().await.unwrap();
        let models = engine.list_models().await.unwrap();
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_metrics() {
        let engine = MLEngine::new().await.unwrap();
        let metrics = engine.get_metrics().await.unwrap();
        assert_eq!(metrics.models_trained, 0);
        assert_eq!(metrics.inferences_completed, 0);
    }
}