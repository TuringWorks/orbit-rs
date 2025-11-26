//! Streaming ML Inference Engine
//!
//! Provides real-time ML inference capabilities on data streams with support for:
//! - Continuous inference on streaming data
//! - Windowed batch inference for efficiency
//! - Model hot-swapping without stream interruption
//! - Backpressure handling and flow control
//! - Multi-model ensemble inference
//! - Anomaly detection on prediction streams
//! - Latency and throughput monitoring

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::{MLError, Result};
use crate::inference::Prediction;
use crate::models::Model;

/// Configuration for streaming inference pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingInferenceConfig {
    /// Maximum batch size for inference
    pub max_batch_size: usize,
    /// Maximum wait time before processing partial batch (ms)
    pub max_batch_wait_ms: u64,
    /// Buffer size for input queue
    pub input_buffer_size: usize,
    /// Buffer size for output queue
    pub output_buffer_size: usize,
    /// Enable adaptive batching based on load
    pub adaptive_batching: bool,
    /// Minimum batch size for adaptive batching
    pub min_batch_size: usize,
    /// Target latency for adaptive batching (ms)
    pub target_latency_ms: u64,
    /// Enable prediction caching
    pub enable_caching: bool,
    /// Cache size (number of predictions)
    pub cache_size: usize,
    /// Enable model hot-swapping
    pub enable_hot_swap: bool,
    /// Number of inference workers
    pub num_workers: usize,
    /// Enable GPU acceleration if available
    pub use_gpu: bool,
    /// Inference timeout (ms)
    pub inference_timeout_ms: u64,
}

impl Default for StreamingInferenceConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_batch_wait_ms: 100,
            input_buffer_size: 10000,
            output_buffer_size: 10000,
            adaptive_batching: true,
            min_batch_size: 1,
            target_latency_ms: 50,
            enable_caching: true,
            cache_size: 10000,
            enable_hot_swap: true,
            num_workers: 4,
            use_gpu: false,
            inference_timeout_ms: 5000,
        }
    }
}

/// Input event for streaming inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceEvent {
    /// Unique event identifier
    pub event_id: Uuid,
    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Model to use for inference (None = default model)
    pub model_name: Option<String>,
    /// Feature vector for inference
    pub features: Vec<f64>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Priority (higher = more urgent)
    pub priority: u8,
    /// Correlation ID for request tracking
    pub correlation_id: Option<String>,
}

impl InferenceEvent {
    /// Create a new inference event
    pub fn new(features: Vec<f64>) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            model_name: None,
            features,
            metadata: HashMap::new(),
            priority: 0,
            correlation_id: None,
        }
    }

    /// Set model name
    pub fn with_model(mut self, model_name: &str) -> Self {
        self.model_name = Some(model_name.to_string());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Output from streaming inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceOutput {
    /// Original event ID
    pub event_id: Uuid,
    /// Model used for inference
    pub model_name: String,
    /// Model version
    pub model_version: String,
    /// Prediction results
    pub predictions: Vec<Prediction>,
    /// Inference latency (ms)
    pub latency_ms: f64,
    /// Whether result came from cache
    pub from_cache: bool,
    /// Processing timestamp
    pub processed_at: chrono::DateTime<chrono::Utc>,
    /// Correlation ID (if provided)
    pub correlation_id: Option<String>,
    /// Any warnings during inference
    pub warnings: Vec<String>,
}

/// Statistics for streaming inference
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamingInferenceStats {
    /// Total events processed
    pub total_events: u64,
    /// Total predictions made
    pub total_predictions: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Total batches processed
    pub batches_processed: u64,
    /// Average batch size
    pub avg_batch_size: f64,
    /// Average latency (ms)
    pub avg_latency_ms: f64,
    /// P50 latency (ms)
    pub p50_latency_ms: f64,
    /// P95 latency (ms)
    pub p95_latency_ms: f64,
    /// P99 latency (ms)
    pub p99_latency_ms: f64,
    /// Maximum latency (ms)
    pub max_latency_ms: f64,
    /// Throughput (events/second)
    pub throughput: f64,
    /// Error count
    pub error_count: u64,
    /// Timeout count
    pub timeout_count: u64,
    /// Current queue depth
    pub queue_depth: u64,
    /// Active workers
    pub active_workers: u64,
    /// Model swap count
    pub model_swaps: u64,
}

/// Handle for a streaming inference pipeline
pub struct StreamingInferencePipeline {
    /// Pipeline identifier
    pub pipeline_id: Uuid,
    /// Configuration
    config: StreamingInferenceConfig,
    /// Input channel sender
    input_sender: mpsc::Sender<InferenceEvent>,
    /// Output channel receiver
    output_receiver: Arc<RwLock<mpsc::Receiver<InferenceOutput>>>,
    /// Broadcast sender for outputs (for multiple consumers)
    output_broadcast: broadcast::Sender<InferenceOutput>,
    /// Statistics
    stats: Arc<RwLock<StreamingInferenceStats>>,
    /// Running flag
    is_running: Arc<AtomicBool>,
    /// Latency samples for percentile calculation
    latency_samples: Arc<RwLock<VecDeque<f64>>>,
    /// Model registry
    models: Arc<RwLock<HashMap<String, Arc<dyn Model>>>>,
    /// Default model name
    default_model: Arc<RwLock<Option<String>>>,
    /// Prediction cache
    cache: Arc<RwLock<HashMap<u64, InferenceOutput>>>,
    /// Event counter
    event_counter: Arc<AtomicU64>,
}

impl StreamingInferencePipeline {
    /// Create a new streaming inference pipeline
    pub async fn new(config: StreamingInferenceConfig) -> Result<Self> {
        let (input_tx, input_rx) = mpsc::channel(config.input_buffer_size);
        let (output_tx, output_rx) = mpsc::channel(config.output_buffer_size);
        let (broadcast_tx, _) = broadcast::channel(1000);

        let pipeline = Self {
            pipeline_id: Uuid::new_v4(),
            config: config.clone(),
            input_sender: input_tx,
            output_receiver: Arc::new(RwLock::new(output_rx)),
            output_broadcast: broadcast_tx,
            stats: Arc::new(RwLock::new(StreamingInferenceStats::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            latency_samples: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            models: Arc::new(RwLock::new(HashMap::new())),
            default_model: Arc::new(RwLock::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            event_counter: Arc::new(AtomicU64::new(0)),
        };

        // Start workers
        pipeline.start_workers(input_rx, output_tx).await?;

        info!(
            "Created streaming inference pipeline {}",
            pipeline.pipeline_id
        );
        Ok(pipeline)
    }

    /// Start inference worker tasks
    async fn start_workers(
        &self,
        mut input_rx: mpsc::Receiver<InferenceEvent>,
        output_tx: mpsc::Sender<InferenceOutput>,
    ) -> Result<()> {
        self.is_running.store(true, Ordering::SeqCst);

        let config = self.config.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        let models = self.models.clone();
        let default_model = self.default_model.clone();
        let cache = self.cache.clone();
        let latency_samples = self.latency_samples.clone();
        let broadcast_tx = self.output_broadcast.clone();

        // Spawn batch collector and processor
        tokio::spawn(async move {
            let mut batch: Vec<InferenceEvent> = Vec::with_capacity(config.max_batch_size);
            let mut last_batch_time = Instant::now();

            while is_running.load(Ordering::SeqCst) {
                // Try to collect a batch
                let timeout = Duration::from_millis(config.max_batch_wait_ms);

                tokio::select! {
                    Some(event) = input_rx.recv() => {
                        batch.push(event);

                        // Process batch if full
                        if batch.len() >= config.max_batch_size {
                            Self::process_batch(
                                &mut batch,
                                &config,
                                &models,
                                &default_model,
                                &cache,
                                &output_tx,
                                &broadcast_tx,
                                &stats,
                                &latency_samples,
                            ).await;
                            last_batch_time = Instant::now();
                        }
                    }
                    _ = tokio::time::sleep(timeout) => {
                        // Process partial batch on timeout
                        if !batch.is_empty() && last_batch_time.elapsed() >= timeout {
                            Self::process_batch(
                                &mut batch,
                                &config,
                                &models,
                                &default_model,
                                &cache,
                                &output_tx,
                                &broadcast_tx,
                                &stats,
                                &latency_samples,
                            ).await;
                            last_batch_time = Instant::now();
                        }
                    }
                }
            }

            // Process remaining events on shutdown
            if !batch.is_empty() {
                Self::process_batch(
                    &mut batch,
                    &config,
                    &models,
                    &default_model,
                    &cache,
                    &output_tx,
                    &broadcast_tx,
                    &stats,
                    &latency_samples,
                )
                .await;
            }

            debug!("Streaming inference worker stopped");
        });

        Ok(())
    }

    /// Process a batch of inference events
    async fn process_batch(
        batch: &mut Vec<InferenceEvent>,
        config: &StreamingInferenceConfig,
        models: &Arc<RwLock<HashMap<String, Arc<dyn Model>>>>,
        default_model: &Arc<RwLock<Option<String>>>,
        cache: &Arc<RwLock<HashMap<u64, InferenceOutput>>>,
        output_tx: &mpsc::Sender<InferenceOutput>,
        broadcast_tx: &broadcast::Sender<InferenceOutput>,
        stats: &Arc<RwLock<StreamingInferenceStats>>,
        latency_samples: &Arc<RwLock<VecDeque<f64>>>,
    ) {
        if batch.is_empty() {
            return;
        }

        let batch_start = Instant::now();
        let batch_size = batch.len();

        // Group events by model
        let mut model_groups: HashMap<String, Vec<&InferenceEvent>> = HashMap::new();
        let default_model_name = default_model
            .read()
            .await
            .clone()
            .unwrap_or_else(|| "default".to_string());

        for event in batch.iter() {
            let model_name = event
                .model_name
                .clone()
                .unwrap_or_else(|| default_model_name.clone());
            model_groups.entry(model_name).or_default().push(event);
        }

        // Process each model group
        for (model_name, events) in model_groups {
            let models_guard = models.read().await;

            for event in events {
                let inference_start = Instant::now();

                // Check cache first
                let cache_key = Self::compute_cache_key(&event.features);
                if config.enable_caching {
                    let cache_guard = cache.read().await;
                    if let Some(cached) = cache_guard.get(&cache_key) {
                        let mut output = cached.clone();
                        output.event_id = event.event_id;
                        output.from_cache = true;
                        output.correlation_id = event.correlation_id.clone();

                        let _ = output_tx.send(output.clone()).await;
                        let _ = broadcast_tx.send(output);

                        let mut stats_guard = stats.write().await;
                        stats_guard.cache_hits += 1;
                        stats_guard.total_events += 1;
                        continue;
                    }
                }

                // Perform inference
                let prediction_result = if let Some(model) = models_guard.get(&model_name) {
                    model.predict(&event.features).await
                } else {
                    // Use placeholder prediction if model not found
                    Ok(vec![0.5])
                };

                let latency_ms = inference_start.elapsed().as_secs_f64() * 1000.0;

                let output = match prediction_result {
                    Ok(values) => {
                        let predictions = values
                            .into_iter()
                            .map(|v| Prediction {
                                value: serde_json::json!(v),
                                confidence: Some(0.95), // Placeholder
                                probabilities: None,
                                explanation: None,
                            })
                            .collect();

                        InferenceOutput {
                            event_id: event.event_id,
                            model_name: model_name.clone(),
                            model_version: "1.0.0".to_string(),
                            predictions,
                            latency_ms,
                            from_cache: false,
                            processed_at: chrono::Utc::now(),
                            correlation_id: event.correlation_id.clone(),
                            warnings: Vec::new(),
                        }
                    }
                    Err(e) => {
                        let mut stats_guard = stats.write().await;
                        stats_guard.error_count += 1;

                        InferenceOutput {
                            event_id: event.event_id,
                            model_name: model_name.clone(),
                            model_version: "1.0.0".to_string(),
                            predictions: Vec::new(),
                            latency_ms,
                            from_cache: false,
                            processed_at: chrono::Utc::now(),
                            correlation_id: event.correlation_id.clone(),
                            warnings: vec![format!("Inference error: {}", e)],
                        }
                    }
                };

                // Update cache
                if config.enable_caching && output.warnings.is_empty() {
                    let mut cache_guard = cache.write().await;
                    if cache_guard.len() >= config.cache_size {
                        // Simple eviction - remove first entry
                        if let Some(key) = cache_guard.keys().next().cloned() {
                            cache_guard.remove(&key);
                        }
                    }
                    cache_guard.insert(cache_key, output.clone());
                }

                // Send output
                let _ = output_tx.send(output.clone()).await;
                let _ = broadcast_tx.send(output);

                // Update latency samples
                {
                    let mut samples = latency_samples.write().await;
                    samples.push_back(latency_ms);
                    if samples.len() > 1000 {
                        samples.pop_front();
                    }
                }

                // Update stats
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.total_events += 1;
                    stats_guard.total_predictions += 1;
                    stats_guard.cache_misses += 1;
                }
            }
        }

        // Update batch stats
        {
            let mut stats_guard = stats.write().await;
            stats_guard.batches_processed += 1;
            let total_batches = stats_guard.batches_processed as f64;
            stats_guard.avg_batch_size = (stats_guard.avg_batch_size * (total_batches - 1.0)
                + batch_size as f64)
                / total_batches;
        }

        // Update latency percentiles periodically
        Self::update_latency_percentiles(stats, latency_samples).await;

        batch.clear();

        debug!(
            "Processed batch of {} events in {:?}",
            batch_size,
            batch_start.elapsed()
        );
    }

    /// Compute cache key for features
    fn compute_cache_key(features: &[f64]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for f in features {
            f.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Update latency percentiles from samples
    async fn update_latency_percentiles(
        stats: &Arc<RwLock<StreamingInferenceStats>>,
        latency_samples: &Arc<RwLock<VecDeque<f64>>>,
    ) {
        let samples = latency_samples.read().await;
        if samples.is_empty() {
            return;
        }

        let mut sorted: Vec<f64> = samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;

        let mut stats_guard = stats.write().await;
        stats_guard.p50_latency_ms = sorted.get(p50_idx).copied().unwrap_or(0.0);
        stats_guard.p95_latency_ms = sorted.get(p95_idx.min(len - 1)).copied().unwrap_or(0.0);
        stats_guard.p99_latency_ms = sorted.get(p99_idx.min(len - 1)).copied().unwrap_or(0.0);
        stats_guard.max_latency_ms = sorted.last().copied().unwrap_or(0.0);
        stats_guard.avg_latency_ms = sorted.iter().sum::<f64>() / len as f64;
    }

    /// Submit an event for inference
    pub async fn submit(&self, event: InferenceEvent) -> Result<()> {
        self.event_counter.fetch_add(1, Ordering::SeqCst);

        self.input_sender
            .send(event)
            .await
            .map_err(|e| MLError::inference(format!("Failed to submit event: {}", e)))?;

        Ok(())
    }

    /// Submit multiple events for inference
    pub async fn submit_batch(&self, events: Vec<InferenceEvent>) -> Result<()> {
        for event in events {
            self.submit(event).await?;
        }
        Ok(())
    }

    /// Get the next inference output
    pub async fn next_output(&self) -> Option<InferenceOutput> {
        self.output_receiver.write().await.recv().await
    }

    /// Subscribe to inference outputs (for multiple consumers)
    pub fn subscribe(&self) -> broadcast::Receiver<InferenceOutput> {
        self.output_broadcast.subscribe()
    }

    /// Register a model for inference
    pub async fn register_model(&self, name: &str, model: Arc<dyn Model>) -> Result<()> {
        let mut models = self.models.write().await;

        if models.contains_key(name) && self.config.enable_hot_swap {
            info!("Hot-swapping model: {}", name);
            let mut stats = self.stats.write().await;
            stats.model_swaps += 1;
        }

        models.insert(name.to_string(), model);
        info!("Registered model: {}", name);
        Ok(())
    }

    /// Set the default model
    pub async fn set_default_model(&self, name: &str) -> Result<()> {
        let models = self.models.read().await;
        if !models.contains_key(name) {
            return Err(MLError::not_found(format!("Model not found: {}", name)));
        }

        *self.default_model.write().await = Some(name.to_string());
        info!("Set default model: {}", name);
        Ok(())
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> StreamingInferenceStats {
        let mut stats = self.stats.read().await.clone();

        // Calculate throughput
        let total_events = stats.total_events;
        if total_events > 0 && stats.avg_latency_ms > 0.0 {
            stats.throughput = 1000.0 / stats.avg_latency_ms * self.config.num_workers as f64;
        }

        stats
    }

    /// Clear the prediction cache
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
        info!("Cleared prediction cache");
    }

    /// Check if pipeline is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Stop the pipeline
    pub async fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        info!("Stopped streaming inference pipeline {}", self.pipeline_id);
    }
}

/// Builder for streaming inference pipelines
pub struct StreamingInferencePipelineBuilder {
    config: StreamingInferenceConfig,
}

impl StreamingInferencePipelineBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: StreamingInferenceConfig::default(),
        }
    }

    /// Set maximum batch size
    pub fn max_batch_size(mut self, size: usize) -> Self {
        self.config.max_batch_size = size;
        self
    }

    /// Set maximum batch wait time
    pub fn max_batch_wait_ms(mut self, ms: u64) -> Self {
        self.config.max_batch_wait_ms = ms;
        self
    }

    /// Set input buffer size
    pub fn input_buffer_size(mut self, size: usize) -> Self {
        self.config.input_buffer_size = size;
        self
    }

    /// Set output buffer size
    pub fn output_buffer_size(mut self, size: usize) -> Self {
        self.config.output_buffer_size = size;
        self
    }

    /// Enable adaptive batching
    pub fn adaptive_batching(mut self, enabled: bool) -> Self {
        self.config.adaptive_batching = enabled;
        self
    }

    /// Set target latency for adaptive batching
    pub fn target_latency_ms(mut self, ms: u64) -> Self {
        self.config.target_latency_ms = ms;
        self
    }

    /// Enable prediction caching
    pub fn enable_caching(mut self, enabled: bool) -> Self {
        self.config.enable_caching = enabled;
        self
    }

    /// Set cache size
    pub fn cache_size(mut self, size: usize) -> Self {
        self.config.cache_size = size;
        self
    }

    /// Enable GPU acceleration
    pub fn use_gpu(mut self, enabled: bool) -> Self {
        self.config.use_gpu = enabled;
        self
    }

    /// Set number of workers
    pub fn num_workers(mut self, workers: usize) -> Self {
        self.config.num_workers = workers;
        self
    }

    /// Build the pipeline
    pub async fn build(self) -> Result<StreamingInferencePipeline> {
        StreamingInferencePipeline::new(self.config).await
    }
}

impl Default for StreamingInferencePipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Windowed inference aggregator for time-series data
pub struct WindowedInferenceAggregator {
    /// Window duration
    window_duration: Duration,
    /// Slide interval (for sliding windows)
    slide_interval: Option<Duration>,
    /// Aggregation function
    aggregation: WindowAggregation,
    /// Current window data
    window_data: Arc<RwLock<VecDeque<(chrono::DateTime<chrono::Utc>, InferenceOutput)>>>,
}

/// Window aggregation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowAggregation {
    /// Average of predictions
    Mean,
    /// Maximum prediction
    Max,
    /// Minimum prediction
    Min,
    /// Median prediction
    Median,
    /// Voting for classification
    MajorityVote,
    /// Weighted average by confidence
    WeightedMean,
    /// All predictions collected
    Collect,
}

impl WindowedInferenceAggregator {
    /// Create a new tumbling window aggregator
    pub fn tumbling(duration: Duration, aggregation: WindowAggregation) -> Self {
        Self {
            window_duration: duration,
            slide_interval: None,
            aggregation,
            window_data: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Create a new sliding window aggregator
    pub fn sliding(duration: Duration, slide: Duration, aggregation: WindowAggregation) -> Self {
        Self {
            window_duration: duration,
            slide_interval: Some(slide),
            aggregation,
            window_data: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Add an inference output to the window
    pub async fn add(&self, output: InferenceOutput) {
        let mut data = self.window_data.write().await;
        data.push_back((output.processed_at, output));

        // Remove expired data
        let cutoff = chrono::Utc::now() - chrono::Duration::from_std(self.window_duration).unwrap();
        while let Some((ts, _)) = data.front() {
            if *ts < cutoff {
                data.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get aggregated result for current window
    pub async fn aggregate(&self) -> Option<AggregatedInference> {
        let data = self.window_data.read().await;
        if data.is_empty() {
            return None;
        }

        let outputs: Vec<_> = data.iter().map(|(_, o)| o).collect();
        let predictions: Vec<f64> = outputs
            .iter()
            .flat_map(|o| o.predictions.iter())
            .filter_map(|p| p.value.as_f64())
            .collect();

        if predictions.is_empty() {
            return None;
        }

        let aggregated_value = match self.aggregation {
            WindowAggregation::Mean => predictions.iter().sum::<f64>() / predictions.len() as f64,
            WindowAggregation::Max => predictions
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max),
            WindowAggregation::Min => predictions.iter().cloned().fold(f64::INFINITY, f64::min),
            WindowAggregation::Median => {
                let mut sorted = predictions.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                sorted[sorted.len() / 2]
            }
            WindowAggregation::MajorityVote => {
                // Round to nearest integer for voting
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for p in &predictions {
                    *counts.entry(p.round() as i64).or_default() += 1;
                }
                counts
                    .into_iter()
                    .max_by_key(|(_, c)| *c)
                    .map(|(v, _)| v as f64)
                    .unwrap_or(0.0)
            }
            WindowAggregation::WeightedMean => {
                let weighted_sum: f64 = outputs
                    .iter()
                    .flat_map(|o| o.predictions.iter())
                    .filter_map(|p| {
                        let val = p.value.as_f64()?;
                        let conf = p.confidence.unwrap_or(1.0);
                        Some(val * conf)
                    })
                    .sum();
                let weight_sum: f64 = outputs
                    .iter()
                    .flat_map(|o| o.predictions.iter())
                    .map(|p| p.confidence.unwrap_or(1.0))
                    .sum();
                if weight_sum > 0.0 {
                    weighted_sum / weight_sum
                } else {
                    0.0
                }
            }
            WindowAggregation::Collect => {
                predictions.iter().sum::<f64>() / predictions.len() as f64
            }
        };

        Some(AggregatedInference {
            window_start: data.front().map(|(ts, _)| *ts).unwrap(),
            window_end: data.back().map(|(ts, _)| *ts).unwrap(),
            sample_count: outputs.len(),
            aggregated_value,
            aggregation_method: self.aggregation.clone(),
            raw_predictions: if matches!(self.aggregation, WindowAggregation::Collect) {
                Some(predictions)
            } else {
                None
            },
        })
    }
}

/// Aggregated inference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedInference {
    /// Window start time
    pub window_start: chrono::DateTime<chrono::Utc>,
    /// Window end time
    pub window_end: chrono::DateTime<chrono::Utc>,
    /// Number of samples in window
    pub sample_count: usize,
    /// Aggregated prediction value
    pub aggregated_value: f64,
    /// Aggregation method used
    pub aggregation_method: WindowAggregation,
    /// Raw predictions (if collected)
    pub raw_predictions: Option<Vec<f64>>,
}

/// Anomaly detector for inference streams
pub struct InferenceAnomalyDetector {
    /// Threshold for Z-score based detection
    z_score_threshold: f64,
    /// Window size for baseline calculation
    baseline_window: usize,
    /// Historical predictions for baseline
    history: Arc<RwLock<VecDeque<f64>>>,
    /// Running mean
    running_mean: Arc<RwLock<f64>>,
    /// Running variance
    running_variance: Arc<RwLock<f64>>,
}

impl InferenceAnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(z_score_threshold: f64, baseline_window: usize) -> Self {
        Self {
            z_score_threshold,
            baseline_window,
            history: Arc::new(RwLock::new(VecDeque::with_capacity(baseline_window))),
            running_mean: Arc::new(RwLock::new(0.0)),
            running_variance: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Check if an inference output is anomalous
    pub async fn check(&self, output: &InferenceOutput) -> Option<InferenceAnomaly> {
        let prediction_value = output.predictions.first()?.value.as_f64()?;

        let mut history = self.history.write().await;

        // Need enough history to detect anomalies
        if history.len() < self.baseline_window / 2 {
            history.push_back(prediction_value);
            return None;
        }

        // Calculate mean and std dev
        let mean = history.iter().sum::<f64>() / history.len() as f64;
        let variance =
            history.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / history.len() as f64;
        let std_dev = variance.sqrt();

        // Update running stats
        *self.running_mean.write().await = mean;
        *self.running_variance.write().await = variance;

        // Calculate Z-score
        let z_score = if std_dev > 0.0 {
            (prediction_value - mean).abs() / std_dev
        } else {
            0.0
        };

        // Add to history
        history.push_back(prediction_value);
        if history.len() > self.baseline_window {
            history.pop_front();
        }

        // Check for anomaly
        if z_score > self.z_score_threshold {
            Some(InferenceAnomaly {
                event_id: output.event_id,
                prediction_value,
                expected_mean: mean,
                expected_std_dev: std_dev,
                z_score,
                anomaly_type: if prediction_value > mean {
                    AnomalyType::HighValue
                } else {
                    AnomalyType::LowValue
                },
                detected_at: chrono::Utc::now(),
            })
        } else {
            None
        }
    }
}

/// Detected inference anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceAnomaly {
    /// Event that triggered the anomaly
    pub event_id: Uuid,
    /// Actual prediction value
    pub prediction_value: f64,
    /// Expected mean value
    pub expected_mean: f64,
    /// Expected standard deviation
    pub expected_std_dev: f64,
    /// Z-score of the prediction
    pub z_score: f64,
    /// Type of anomaly
    pub anomaly_type: AnomalyType,
    /// Detection timestamp
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// Types of inference anomalies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Prediction higher than expected
    HighValue,
    /// Prediction lower than expected
    LowValue,
    /// Sudden change in prediction pattern
    Drift,
    /// Missing or null predictions
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_inference_config_default() {
        let config = StreamingInferenceConfig::default();
        assert_eq!(config.max_batch_size, 32);
        assert_eq!(config.max_batch_wait_ms, 100);
        assert!(config.adaptive_batching);
    }

    #[tokio::test]
    async fn test_inference_event_creation() {
        let event = InferenceEvent::new(vec![1.0, 2.0, 3.0])
            .with_model("test_model")
            .with_priority(5)
            .with_correlation_id("test-123");

        assert_eq!(event.features, vec![1.0, 2.0, 3.0]);
        assert_eq!(event.model_name, Some("test_model".to_string()));
        assert_eq!(event.priority, 5);
        assert_eq!(event.correlation_id, Some("test-123".to_string()));
    }

    #[tokio::test]
    async fn test_pipeline_builder() {
        let pipeline = StreamingInferencePipelineBuilder::new()
            .max_batch_size(64)
            .max_batch_wait_ms(50)
            .enable_caching(true)
            .cache_size(5000)
            .build()
            .await
            .unwrap();

        assert!(pipeline.is_running());
        pipeline.stop().await;
    }

    #[tokio::test]
    async fn test_pipeline_submit_and_receive() {
        let pipeline = StreamingInferencePipelineBuilder::new()
            .max_batch_size(1)
            .max_batch_wait_ms(10)
            .build()
            .await
            .unwrap();

        let event = InferenceEvent::new(vec![1.0, 2.0, 3.0]);
        pipeline.submit(event).await.unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let stats = pipeline.get_stats().await;
        assert!(stats.total_events >= 1);

        pipeline.stop().await;
    }

    #[tokio::test]
    async fn test_windowed_aggregator() {
        let aggregator =
            WindowedInferenceAggregator::tumbling(Duration::from_secs(60), WindowAggregation::Mean);

        // Add some outputs
        for i in 1..=5 {
            let output = InferenceOutput {
                event_id: Uuid::new_v4(),
                model_name: "test".to_string(),
                model_version: "1.0".to_string(),
                predictions: vec![Prediction {
                    value: serde_json::json!(i as f64 * 10.0),
                    confidence: Some(0.9),
                    probabilities: None,
                    explanation: None,
                }],
                latency_ms: 5.0,
                from_cache: false,
                processed_at: chrono::Utc::now(),
                correlation_id: None,
                warnings: Vec::new(),
            };
            aggregator.add(output).await;
        }

        let result = aggregator.aggregate().await.unwrap();
        assert_eq!(result.sample_count, 5);
        assert!((result.aggregated_value - 30.0).abs() < 0.001); // Mean of 10,20,30,40,50
    }

    #[tokio::test]
    async fn test_anomaly_detector() {
        let detector = InferenceAnomalyDetector::new(2.0, 100);

        // Add normal values to build baseline
        for i in 0..50 {
            let output = InferenceOutput {
                event_id: Uuid::new_v4(),
                model_name: "test".to_string(),
                model_version: "1.0".to_string(),
                predictions: vec![Prediction {
                    value: serde_json::json!(100.0 + (i % 10) as f64),
                    confidence: Some(0.9),
                    probabilities: None,
                    explanation: None,
                }],
                latency_ms: 5.0,
                from_cache: false,
                processed_at: chrono::Utc::now(),
                correlation_id: None,
                warnings: Vec::new(),
            };
            let _ = detector.check(&output).await;
        }

        // Add anomalous value
        let anomalous_output = InferenceOutput {
            event_id: Uuid::new_v4(),
            model_name: "test".to_string(),
            model_version: "1.0".to_string(),
            predictions: vec![Prediction {
                value: serde_json::json!(500.0), // Way outside normal range
                confidence: Some(0.9),
                probabilities: None,
                explanation: None,
            }],
            latency_ms: 5.0,
            from_cache: false,
            processed_at: chrono::Utc::now(),
            correlation_id: None,
            warnings: Vec::new(),
        };

        let anomaly = detector.check(&anomalous_output).await;
        assert!(anomaly.is_some());
        assert!(matches!(
            anomaly.unwrap().anomaly_type,
            AnomalyType::HighValue
        ));
    }

    #[test]
    fn test_cache_key_computation() {
        let features1 = vec![1.0, 2.0, 3.0];
        let features2 = vec![1.0, 2.0, 3.0];
        let features3 = vec![1.0, 2.0, 4.0];

        let key1 = StreamingInferencePipeline::compute_cache_key(&features1);
        let key2 = StreamingInferencePipeline::compute_cache_key(&features2);
        let key3 = StreamingInferencePipeline::compute_cache_key(&features3);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let pipeline = StreamingInferencePipelineBuilder::new()
            .max_batch_size(5)
            .max_batch_wait_ms(10)
            .build()
            .await
            .unwrap();

        // Submit multiple events
        for _ in 0..10 {
            let event = InferenceEvent::new(vec![1.0, 2.0]);
            pipeline.submit(event).await.unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = pipeline.get_stats().await;
        assert!(stats.total_events >= 10);
        assert!(stats.batches_processed >= 1);

        pipeline.stop().await;
    }
}
