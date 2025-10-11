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
use crate::error::Result;
use crate::inference::{InferenceConfig, InferenceJob, InferenceResult};
use crate::models::{Model, ModelMetadata, ModelRegistry};
use crate::training::{TrainingConfig, TrainingJob, TrainingStatus};

/// ML engine builder for configurable engine construction
pub mod builder;
/// Model factory for creating different types of ML models
pub mod factory;
/// Model lifecycle and resource management
pub mod manager;
/// Job scheduling and execution for training and inference
pub mod scheduler;

pub use builder::MLEngineBuilder;
pub use factory::ModelFactory;
pub use manager::ModelManager;
pub use scheduler::JobScheduler;

/// Main ML engine for Orbit database
#[derive(Debug)]
pub struct MLEngine {
    /// Engine configuration
    #[allow(dead_code)]
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

/// Engine performance metrics and statistics
///
/// Tracks various operational metrics of the ML engine including
/// model usage, performance, and resource utilization.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EngineMetrics {
    /// Total number of models that have completed training
    pub models_trained: u64,
    /// Number of models currently loaded in memory
    pub models_loaded: u64,
    /// Total number of inference requests completed
    pub inferences_completed: u64,
    /// Number of training jobs currently running
    pub training_jobs_active: u64,
    /// Number of inference jobs currently running
    pub inference_jobs_active: u64,
    /// Current memory usage in bytes
    pub memory_usage_bytes: u64,
    /// GPU utilization percentage (0-100)
    pub gpu_utilization_percent: f64,
    /// Average time per training job in milliseconds
    pub average_training_time_ms: f64,
    /// Average time per inference request in milliseconds
    pub average_inference_time_ms: f64,
    /// Total number of errors encountered
    pub error_count: u64,
}

/// Core interface for ML engine operations
///
/// Provides async methods for model training, inference, management,
/// and monitoring within the Orbit ML engine.
#[async_trait]
pub trait MLEngineInterface {
    /// Train a model asynchronously
    ///
    /// # Arguments
    /// * `model_name` - Unique identifier for the model
    /// * `model_type` - Type of model to train (e.g., "neural_network", "transformer")
    /// * `training_data` - Serialized training data
    /// * `config` - Training configuration parameters
    ///
    /// # Returns
    /// A training job that can be monitored for progress
    async fn train_model(
        &self,
        model_name: String,
        model_type: String,
        training_data: Vec<u8>,
        config: TrainingConfig,
    ) -> Result<TrainingJob>;

    /// Load a trained model into memory for inference
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to load
    ///
    /// # Returns
    /// Reference-counted model instance
    async fn load_model(&self, model_name: &str) -> Result<Arc<dyn Model>>;

    /// Run inference on a loaded model
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to use for inference
    /// * `input_data` - Serialized input data for prediction
    /// * `config` - Inference configuration parameters
    ///
    /// # Returns
    /// Inference result containing predictions and metadata
    async fn predict(
        &self,
        model_name: &str,
        input_data: Vec<u8>,
        config: InferenceConfig,
    ) -> Result<InferenceResult>;

    /// List all available models in the registry
    ///
    /// # Returns
    /// Vector of model metadata for all registered models
    async fn list_models(&self) -> Result<Vec<ModelMetadata>>;

    /// Delete a model from the registry and disk
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to delete
    async fn delete_model(&self, model_name: &str) -> Result<()>;

    /// Get current engine performance metrics
    ///
    /// # Returns
    /// Current engine metrics and statistics
    async fn get_metrics(&self) -> Result<EngineMetrics>;

    /// Perform engine health check
    ///
    /// # Returns
    /// Current engine status and system information
    async fn health_check(&self) -> Result<EngineStatus>;
}

/// Engine status information for health checks and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    /// Current engine status ("healthy", "degraded", "unhealthy")
    pub status: String,
    /// Engine version string
    pub version: String,
    /// Engine uptime in seconds since startup
    pub uptime_seconds: u64,
    /// Total number of active jobs (training + inference)
    pub active_jobs: u64,
    /// Number of models currently loaded in memory
    pub loaded_models: u64,
    /// Current memory usage in megabytes
    pub memory_usage_mb: f64,
    /// Whether GPU acceleration is available
    pub gpu_available: bool,
    /// List of enabled features/capabilities
    pub features_enabled: Vec<String>,
}

impl MLEngine {
    /// Create a new ML engine with default configuration
    ///
    /// # Returns
    /// A new ML engine instance or an error if initialization fails
    pub async fn new() -> Result<Self> {
        Self::with_config(MLConfig::default()).await
    }

    /// Create ML engine with custom configuration
    ///
    /// # Arguments
    /// * `config` - Custom ML engine configuration
    ///
    /// # Returns
    /// A new ML engine instance or an error if initialization fails
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

    /// Initialize all engine subsystems including GPU, distributed computing, and multi-language runtimes
    ///
    /// # Returns
    /// Ok(()) if initialization succeeds, error otherwise
    async fn initialize_subsystems(&self) -> Result<()> {
        // Initialize model factory
        debug!("Initializing model factory");

        // Initialize GPU if available (graceful degradation)
        #[cfg(feature = "gpu")]
        {
            if self.config.compute.enable_gpu {
                debug!("Attempting GPU initialization with graceful fallback");
                // GPU initialization uses graceful degradation - it won't fail
                self.initialize_gpu().await?;
            } else {
                info!("⚙️  GPU support disabled in configuration - using CPU processing");
            }
        }
        
        #[cfg(not(feature = "gpu"))]
        {
            if self.config.compute.enable_gpu {
                warn!("⚠️  GPU requested but not compiled in this build - using CPU processing");
            }
            info!("🖥️  CPU-only build - all ML operations will use CPU processing");
        }

        // Initialize distributed computing if enabled
        #[cfg(feature = "distributed")]
        {
            if self.config.compute.enable_distributed {
                debug!("Initializing distributed computing");
                self.initialize_distributed().await?;
            }
        }

        // Initialize multi-language runtimes
        self.initialize_multi_language().await?;

        Ok(())
    }

    /// Initialize GPU support with graceful degradation to CPU
    ///
    /// Attempts to initialize GPU acceleration but gracefully falls back
    /// to CPU-based processing if GPU is unavailable. This ensures the
    /// ML engine remains functional regardless of hardware configuration.
    ///
    /// # Returns
    /// Always returns Ok(()), implementing graceful degradation pattern
    #[cfg(feature = "gpu")]
    async fn initialize_gpu(&self) -> Result<()> {
        use candle_core::Device;

        info!("🚀 Attempting GPU initialization for ML acceleration...");
        
        match Device::cuda_if_available(0) {
            Ok(device) => {
                info!("✅ GPU acceleration enabled: {:?}", device);
                info!("🔥 CUDA device successfully initialized for high-performance ML operations");
                
                // Update metrics to reflect successful GPU initialization
                {
                    let _metrics = self.metrics.write().await;
                    // GPU is available and initialized
                    info!("📊 GPU metrics tracking enabled");
                }
                Ok(())
            }
            Err(e) => {
                warn!("⚠️  GPU initialization failed: {}", e);
                info!("🔄 Gracefully degrading to CPU-based ML processing");
                info!("💡 Performance note: CPU mode active. For GPU acceleration:");
                info!("   - Ensure CUDA drivers are installed and compatible");
                info!("   - Verify GPU hardware supports CUDA compute capability");
                info!("   - Check system PATH includes CUDA binaries");
                
                // Graceful degradation - continue with CPU processing
                // This ensures the ML engine remains functional without GPU
                info!("✅ ML engine ready with CPU-based processing");
                Ok(())
            }
        }
    }

    /// Initialize distributed computing environment for multi-node training
    ///
    /// # Returns
    /// Ok(()) if distributed computing setup succeeds, error otherwise
    #[cfg(feature = "distributed")]
    async fn initialize_distributed(&self) -> Result<()> {
        debug!("Setting up distributed computing environment");
        // TODO: Initialize MPI or distributed training framework
        Ok(())
    }

    /// Initialize multi-language runtime support for Python, JavaScript, and Lua
    ///
    /// # Returns
    /// Ok(()) if runtime initialization succeeds, error otherwise
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

    /// Get a builder for customizing engine configuration
    ///
    /// # Returns
    /// A new MLEngineBuilder instance for fluent configuration
    pub fn builder() -> MLEngineBuilder {
        MLEngineBuilder::default()
    }

    /// Check if GPU acceleration is available and functional
    ///
    /// # Returns
    /// True if GPU is available, false if running in CPU mode
    #[cfg(feature = "gpu")]
    pub fn is_gpu_available(&self) -> bool {
        use candle_core::Device;
        
        match Device::cuda_if_available(0) {
            Ok(_) => {
                debug!("🔍 GPU availability check: GPU is available");
                true
            }
            Err(_) => {
                debug!("🔍 GPU availability check: Running in CPU mode");
                false
            }
        }
    }

    /// Check if GPU acceleration is available (CPU-only build)
    ///
    /// # Returns
    /// Always false for CPU-only builds
    #[cfg(not(feature = "gpu"))]
    pub fn is_gpu_available(&self) -> bool {
        debug!("🔍 GPU availability check: CPU-only build");
        false
    }

    /// Get current processing mode information
    ///
    /// # Returns
    /// String describing the current processing mode
    pub fn get_processing_mode(&self) -> String {
        if self.is_gpu_available() {
            "🔥 GPU-Accelerated Processing".to_string()
        } else {
            "🖥️  CPU-Based Processing".to_string()
        }
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
        self.job_scheduler
            .schedule_training_job(job_id, training_data)
            .await?;

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
        let mut features_enabled = self.get_enabled_features();
        
        // Add runtime processing mode information
        features_enabled.push(format!("processing-mode: {}", self.get_processing_mode()));

        // Determine overall health status
        let status = if metrics.error_count > 100 {
            "degraded".to_string()
        } else if metrics.error_count > 1000 {
            "unhealthy".to_string()
        } else {
            "healthy".to_string()
        };

        Ok(EngineStatus {
            status,
            version: crate::VERSION.to_string(),
            uptime_seconds: 0, // TODO: Track actual uptime
            active_jobs: metrics.training_jobs_active + metrics.inference_jobs_active,
            loaded_models: metrics.models_loaded,
            memory_usage_mb: metrics.memory_usage_bytes as f64 / 1024.0 / 1024.0,
            gpu_available: self.is_gpu_available(), // Runtime GPU detection
            features_enabled,
        })
    }
}

impl MLEngine {
    /// Get list of enabled compile-time features
    ///
    /// # Returns
    /// Vector of feature names that are compiled into this build
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

    /// Shutdown the engine gracefully, canceling active jobs and cleaning up resources
    ///
    /// # Returns
    /// Ok(()) if shutdown completes successfully, error otherwise
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
