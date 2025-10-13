//! Distributed tracing support for Orbit
//!
//! This module provides distributed tracing capabilities compatible with OpenTelemetry
//! and other tracing systems for tracking request flows across distributed systems.

use crate::{PrometheusError, PrometheusResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Distributed tracing manager
pub struct TracingManager {
    /// Active traces
    traces: Arc<RwLock<HashMap<String, Trace>>>,
    /// Completed traces (for sampling)
    completed_traces: Arc<RwLock<Vec<Trace>>>,
    /// Tracing configuration
    config: TracingConfig,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
    /// Maximum traces to retain in memory
    pub max_traces: usize,
    /// Export endpoint for traces
    pub export_endpoint: Option<String>,
    /// Service name for trace attribution
    pub service_name: String,
    /// Enable detailed span attributes
    pub detailed_spans: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_rate: 1.0,
            max_traces: 1000,
            export_endpoint: None,
            service_name: "orbit-server".to_string(),
            detailed_spans: true,
        }
    }
}

/// Distributed trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    /// Unique trace identifier
    pub trace_id: String,
    /// Parent trace ID (if part of a larger trace)
    pub parent_trace_id: Option<String>,
    /// Service name
    pub service_name: String,
    /// Operation name
    pub operation_name: String,
    /// Start timestamp
    pub start_time: DateTime<Utc>,
    /// End timestamp
    pub end_time: Option<DateTime<Utc>>,
    /// Trace duration
    pub duration: Option<Duration>,
    /// Spans within this trace
    pub spans: Vec<Span>,
    /// Trace tags/attributes
    pub tags: HashMap<String, String>,
    /// Trace status
    pub status: TraceStatus,
    /// Error information if failed
    pub error: Option<TraceError>,
}

/// Span within a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique span identifier
    pub span_id: String,
    /// Parent span ID (if nested)
    pub parent_span_id: Option<String>,
    /// Span name/operation
    pub name: String,
    /// Start timestamp
    pub start_time: DateTime<Utc>,
    /// End timestamp
    pub end_time: Option<DateTime<Utc>>,
    /// Span duration
    pub duration: Option<Duration>,
    /// Span kind
    pub kind: SpanKind,
    /// Span attributes
    pub attributes: HashMap<String, String>,
    /// Span events
    pub events: Vec<SpanEvent>,
    /// Span status
    pub status: SpanStatus,
}

/// Span kind indicating the role in the trace
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpanKind {
    /// Internal operation
    Internal,
    /// Server handling a request
    Server,
    /// Client making a request
    Client,
    /// Producer sending a message
    Producer,
    /// Consumer receiving a message
    Consumer,
}

/// Span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event attributes
    pub attributes: HashMap<String, String>,
}

/// Span status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span encountered an error
    Error,
    /// Span status is unset/unknown
    Unset,
}

/// Trace status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TraceStatus {
    /// Trace is in progress
    InProgress,
    /// Trace completed successfully
    Completed,
    /// Trace failed with error
    Failed,
    /// Trace was cancelled
    Cancelled,
}

/// Trace error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceError {
    /// Error message
    pub message: String,
    /// Error type
    pub error_type: String,
    /// Stack trace if available
    pub stack_trace: Option<String>,
}

impl TracingManager {
    /// Create a new tracing manager
    pub fn new(config: TracingConfig) -> Self {
        Self {
            traces: Arc::new(RwLock::new(HashMap::new())),
            completed_traces: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Start a new trace
    pub async fn start_trace(
        &self,
        trace_id: String,
        operation_name: String,
        tags: HashMap<String, String>,
    ) -> PrometheusResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check sampling
        if !self.should_sample() {
            return Ok(());
        }

        let trace = Trace {
            trace_id: trace_id.clone(),
            parent_trace_id: None,
            service_name: self.config.service_name.clone(),
            operation_name,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            spans: Vec::new(),
            tags,
            status: TraceStatus::InProgress,
            error: None,
        };

        let mut traces = self.traces.write().await;
        traces.insert(trace_id, trace);

        Ok(())
    }

    /// End a trace
    pub async fn end_trace(
        &self,
        trace_id: &str,
        status: TraceStatus,
        error: Option<TraceError>,
    ) -> PrometheusResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut traces = self.traces.write().await;
        
        if let Some(trace) = traces.remove(trace_id) {
            let mut completed_trace = trace;
            completed_trace.end_time = Some(Utc::now());
            completed_trace.duration = completed_trace.end_time.as_ref().map(|end| {
                (*end - completed_trace.start_time)
                    .to_std()
                    .unwrap_or(Duration::from_secs(0))
            });
            completed_trace.status = status;
            completed_trace.error = error;

            let mut completed = self.completed_traces.write().await;
            completed.push(completed_trace);

            // Trim to max size
            while completed.len() > self.config.max_traces {
                completed.remove(0);
            }
        }

        Ok(())
    }

    /// Add a span to a trace
    pub async fn add_span(
        &self,
        trace_id: &str,
        span: Span,
    ) -> PrometheusResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut traces = self.traces.write().await;
        
        if let Some(trace) = traces.get_mut(trace_id) {
            trace.spans.push(span);
        }

        Ok(())
    }

    /// Create a new span
    pub fn create_span(
        &self,
        span_id: String,
        name: String,
        kind: SpanKind,
        attributes: HashMap<String, String>,
    ) -> Span {
        Span {
            span_id,
            parent_span_id: None,
            name,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            kind,
            attributes,
            events: Vec::new(),
            status: SpanStatus::Unset,
        }
    }

    /// Complete a span
    pub fn complete_span(&self, mut span: Span, status: SpanStatus) -> Span {
        span.end_time = Some(Utc::now());
        span.duration = span.end_time.as_ref().map(|end| {
            (*end - span.start_time)
                .to_std()
                .unwrap_or(Duration::from_secs(0))
        });
        span.status = status;
        span
    }

    /// Add an event to a span
    pub fn add_span_event(
        &self,
        span: &mut Span,
        name: String,
        attributes: HashMap<String, String>,
    ) {
        span.events.push(SpanEvent {
            name,
            timestamp: Utc::now(),
            attributes,
        });
    }

    /// Get an active trace
    pub async fn get_trace(&self, trace_id: &str) -> PrometheusResult<Option<Trace>> {
        let traces = self.traces.read().await;
        Ok(traces.get(trace_id).cloned())
    }

    /// Get completed traces
    pub async fn get_completed_traces(
        &self,
        limit: Option<usize>,
    ) -> PrometheusResult<Vec<Trace>> {
        let completed = self.completed_traces.read().await;
        
        let result: Vec<_> = if let Some(limit) = limit {
            completed.iter().rev().take(limit).cloned().collect()
        } else {
            completed.iter().rev().cloned().collect()
        };

        Ok(result)
    }

    /// Get traces by status
    pub async fn get_traces_by_status(
        &self,
        status: TraceStatus,
        limit: usize,
    ) -> PrometheusResult<Vec<Trace>> {
        let completed = self.completed_traces.read().await;
        
        let filtered: Vec<_> = completed
            .iter()
            .filter(|t| t.status == status)
            .rev()
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Get slow traces (above threshold)
    pub async fn get_slow_traces(
        &self,
        threshold_ms: u64,
        limit: usize,
    ) -> PrometheusResult<Vec<Trace>> {
        let completed = self.completed_traces.read().await;
        
        let filtered: Vec<_> = completed
            .iter()
            .filter(|t| {
                t.duration
                    .map(|d| d.as_millis() as u64 > threshold_ms)
                    .unwrap_or(false)
            })
            .rev()
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Export traces to external system
    pub async fn export_traces(&self) -> PrometheusResult<()> {
        if let Some(_endpoint) = &self.config.export_endpoint {
            // In production, this would export to the configured endpoint
            // For now, this is a stub
        }
        Ok(())
    }

    /// Clear completed traces
    pub async fn clear_completed_traces(&self) -> PrometheusResult<()> {
        let mut completed = self.completed_traces.write().await;
        completed.clear();
        Ok(())
    }

    /// Check if trace should be sampled
    fn should_sample(&self) -> bool {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.config.sampling_rate
    }

    /// Get configuration
    pub fn config(&self) -> &TracingConfig {
        &self.config
    }
}

impl Default for TracingManager {
    fn default() -> Self {
        Self::new(TracingConfig::default())
    }
}

/// Helper function to generate a trace ID
pub fn generate_trace_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:032x}", rng.gen::<u128>())
}

/// Helper function to generate a span ID
pub fn generate_span_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:016x}", rng.gen::<u64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracing_manager_creation() {
        let manager = TracingManager::default();
        assert!(manager.config().enabled);
    }

    #[tokio::test]
    async fn test_start_and_end_trace() {
        let manager = TracingManager::default();
        let trace_id = generate_trace_id();
        
        manager
            .start_trace(
                trace_id.clone(),
                "test_operation".to_string(),
                HashMap::new(),
            )
            .await
            .unwrap();

        let trace = manager.get_trace(&trace_id).await.unwrap();
        assert!(trace.is_some());
        assert_eq!(trace.unwrap().status, TraceStatus::InProgress);

        manager
            .end_trace(&trace_id, TraceStatus::Completed, None)
            .await
            .unwrap();

        let active_trace = manager.get_trace(&trace_id).await.unwrap();
        assert!(active_trace.is_none());

        let completed = manager.get_completed_traces(Some(1)).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, TraceStatus::Completed);
    }

    #[tokio::test]
    async fn test_span_creation() {
        let manager = TracingManager::default();
        
        let span = manager.create_span(
            generate_span_id(),
            "test_span".to_string(),
            SpanKind::Internal,
            HashMap::new(),
        );

        assert_eq!(span.name, "test_span");
        assert_eq!(span.kind, SpanKind::Internal);
        assert_eq!(span.status, SpanStatus::Unset);
    }

    #[tokio::test]
    async fn test_complete_span() {
        let manager = TracingManager::default();
        
        let span = manager.create_span(
            generate_span_id(),
            "test_span".to_string(),
            SpanKind::Server,
            HashMap::new(),
        );

        let completed_span = manager.complete_span(span, SpanStatus::Ok);
        assert_eq!(completed_span.status, SpanStatus::Ok);
        assert!(completed_span.end_time.is_some());
        assert!(completed_span.duration.is_some());
    }

    #[tokio::test]
    async fn test_slow_traces() {
        let manager = TracingManager::default();
        
        // Create a fast trace
        let fast_trace_id = generate_trace_id();
        manager
            .start_trace(
                fast_trace_id.clone(),
                "fast_op".to_string(),
                HashMap::new(),
            )
            .await
            .unwrap();
        manager
            .end_trace(&fast_trace_id, TraceStatus::Completed, None)
            .await
            .unwrap();

        // Get slow traces (threshold = 100ms)
        let slow_traces = manager.get_slow_traces(100, 10).await.unwrap();
        // Should be empty since we didn't add any slow traces
        assert_eq!(slow_traces.len(), 0);
    }
}
