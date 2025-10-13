//! Stream processing engine with windowing and aggregation
//!
//! This module provides real-time stream processing capabilities with support for
//! time-based and count-based windows, various aggregation functions, and stateful
//! processing.

use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Window types for stream processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowType {
    /// Tumbling window - non-overlapping fixed-size windows
    Tumbling {
        /// Window size
        size: Duration,
    },
    /// Sliding window - overlapping windows that slide by a specified amount
    Sliding {
        /// Window size
        size: Duration,
        /// Slide interval
        slide: Duration,
    },
    /// Session window - dynamic windows based on activity gaps
    Session {
        /// Gap duration to end session
        gap: Duration,
    },
    /// Count-based window - fixed number of events
    Count {
        /// Number of events per window
        count: usize,
    },
}

/// Aggregation functions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationFunction {
    /// Count of events
    Count,
    /// Sum of numeric values
    Sum,
    /// Average of numeric values
    Avg,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// First value in window
    First,
    /// Last value in window
    Last,
    /// Collect all values into array
    Collect,
    /// Count distinct values
    CountDistinct,
}

/// Stream event for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Event timestamp
    pub timestamp: i64,
    /// Event key for grouping
    pub key: String,
    /// Event value
    pub value: serde_json::Value,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl StreamEvent {
    /// Create a new stream event
    pub fn new(key: String, value: serde_json::Value) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp_millis(),
            key,
            value,
            metadata: HashMap::new(),
        }
    }

    /// Extract numeric value from event
    pub fn as_number(&self) -> Option<f64> {
        self.value
            .as_f64()
            .or_else(|| self.value.as_i64().map(|i| i as f64))
    }
}

/// Window state for aggregation
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Window start time
    pub start_time: i64,
    /// Window end time
    pub end_time: i64,
    /// Events in this window
    pub events: VecDeque<StreamEvent>,
    /// Aggregated result
    pub result: Option<serde_json::Value>,
}

impl WindowState {
    fn new(start_time: i64, end_time: i64) -> Self {
        Self {
            start_time,
            end_time,
            events: VecDeque::new(),
            result: None,
        }
    }

    /// Add event to window
    fn add_event(&mut self, event: StreamEvent) {
        self.events.push_back(event);
    }

    /// Check if window contains timestamp
    #[allow(dead_code)] // Will be used for sliding/session window implementations
    fn contains(&self, timestamp: i64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }
}

/// Stream processor with windowing and aggregation
pub struct StreamProcessor {
    /// Window type
    window_type: WindowType,
    /// Aggregation function
    aggregation: AggregationFunction,
    /// Field to aggregate (if applicable)
    field: Option<String>,
    /// Active windows by key
    windows: Arc<RwLock<HashMap<String, Vec<WindowState>>>>,
    /// Processing statistics
    stats: Arc<RwLock<StreamStats>>,
}

impl StreamProcessor {
    /// Create a new stream processor
    pub fn new(window_type: WindowType, aggregation: AggregationFunction) -> Self {
        Self {
            window_type,
            aggregation,
            field: None,
            windows: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(StreamStats::default())),
        }
    }

    /// Set field to aggregate
    pub fn with_field(mut self, field: String) -> Self {
        self.field = Some(field);
        self
    }

    /// Process a stream event
    pub async fn process_event(&self, event: StreamEvent) -> OrbitResult<Vec<WindowResult>> {
        let key = event.key.clone();
        let mut results = Vec::new();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.events_processed += 1;
        }

        // Get or create windows for this key
        let mut windows = self.windows.write().await;
        let key_windows = windows.entry(key.clone()).or_insert_with(Vec::new);

        match &self.window_type {
            WindowType::Tumbling { size } => {
                let window_size_ms = size.as_millis() as i64;
                let window_start = (event.timestamp / window_size_ms) * window_size_ms;
                let window_end = window_start + window_size_ms;

                // Find or create window
                let window_exists = key_windows.iter().any(|w| w.start_time == window_start);
                if !window_exists {
                    key_windows.push(WindowState::new(window_start, window_end));
                }

                let window = key_windows
                    .iter_mut()
                    .find(|w| w.start_time == window_start)
                    .unwrap();

                window.add_event(event);

                // Check if window is complete (current time > window end)
                let current_time = chrono::Utc::now().timestamp_millis();
                if current_time >= window_end {
                    let result = self.compute_aggregation(&window.events)?;
                    results.push(WindowResult {
                        key: key.clone(),
                        start_time: window.start_time,
                        end_time: window.end_time,
                        count: window.events.len(),
                        result,
                    });

                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.windows_completed += 1;
                }
            }
            WindowType::Count { count } => {
                // Find or create current window
                if key_windows.is_empty() {
                    key_windows.push(WindowState::new(event.timestamp, 0));
                }

                let window = key_windows.last_mut().unwrap();
                window.add_event(event);

                // Check if window is complete
                if window.events.len() >= *count {
                    let result = self.compute_aggregation(&window.events)?;
                    results.push(WindowResult {
                        key: key.clone(),
                        start_time: window.start_time,
                        end_time: chrono::Utc::now().timestamp_millis(),
                        count: window.events.len(),
                        result,
                    });

                    // Create new window
                    key_windows.clear();
                    key_windows.push(WindowState::new(chrono::Utc::now().timestamp_millis(), 0));

                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.windows_completed += 1;
                }
            }
            WindowType::Sliding { .. } | WindowType::Session { .. } => {
                // TODO: Implement sliding and session windows
                return Err(OrbitError::internal(format!(
                    "Window type {:?} not yet implemented",
                    self.window_type
                )));
            }
        }

        Ok(results)
    }

    /// Compute aggregation over events
    fn compute_aggregation(
        &self,
        events: &VecDeque<StreamEvent>,
    ) -> OrbitResult<serde_json::Value> {
        if events.is_empty() {
            return Ok(serde_json::Value::Null);
        }

        match self.aggregation {
            AggregationFunction::Count => Ok(serde_json::json!(events.len())),
            AggregationFunction::Sum => {
                let sum: f64 = events.iter().filter_map(|e| e.as_number()).sum();
                Ok(serde_json::json!(sum))
            }
            AggregationFunction::Avg => {
                let values: Vec<f64> = events.iter().filter_map(|e| e.as_number()).collect();
                if values.is_empty() {
                    return Ok(serde_json::Value::Null);
                }
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                Ok(serde_json::json!(avg))
            }
            AggregationFunction::Min => {
                let min = events
                    .iter()
                    .filter_map(|e| e.as_number())
                    .min_by(|a, b| a.partial_cmp(b).unwrap());
                Ok(serde_json::json!(min))
            }
            AggregationFunction::Max => {
                let max = events
                    .iter()
                    .filter_map(|e| e.as_number())
                    .max_by(|a, b| a.partial_cmp(b).unwrap());
                Ok(serde_json::json!(max))
            }
            AggregationFunction::First => Ok(events.front().unwrap().value.clone()),
            AggregationFunction::Last => Ok(events.back().unwrap().value.clone()),
            AggregationFunction::Collect => {
                let values: Vec<_> = events.iter().map(|e| e.value.clone()).collect();
                Ok(serde_json::json!(values))
            }
            AggregationFunction::CountDistinct => {
                let unique: std::collections::HashSet<_> =
                    events.iter().map(|e| e.value.to_string()).collect();
                Ok(serde_json::json!(unique.len()))
            }
        }
    }

    /// Get processing statistics
    pub async fn get_stats(&self) -> StreamStats {
        self.stats.read().await.clone()
    }

    /// Clear all windows (for testing/cleanup)
    pub async fn clear(&self) {
        self.windows.write().await.clear();
    }
}

/// Result of window computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResult {
    /// Key that was aggregated
    pub key: String,
    /// Window start time
    pub start_time: i64,
    /// Window end time
    pub end_time: i64,
    /// Number of events in window
    pub count: usize,
    /// Aggregated result
    pub result: serde_json::Value,
}

/// Stream processing statistics
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    pub events_processed: u64,
    pub windows_completed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tumbling_window_count() {
        let processor = StreamProcessor::new(
            WindowType::Tumbling {
                size: Duration::from_secs(1),
            },
            AggregationFunction::Count,
        );

        // Add events in same window
        let event1 = StreamEvent::new("key1".to_string(), serde_json::json!(1));
        let event2 = StreamEvent::new("key1".to_string(), serde_json::json!(2));

        processor.process_event(event1).await.unwrap();
        processor.process_event(event2).await.unwrap();

        let stats = processor.get_stats().await;
        assert_eq!(stats.events_processed, 2);
    }

    #[tokio::test]
    async fn test_count_window() {
        let processor =
            StreamProcessor::new(WindowType::Count { count: 3 }, AggregationFunction::Sum);

        // Add 3 events to complete window
        for i in 1..=3 {
            let event = StreamEvent::new("key1".to_string(), serde_json::json!(i));
            let results = processor.process_event(event).await.unwrap();

            if i == 3 {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].count, 3);
                assert_eq!(results[0].result, serde_json::json!(6.0)); // 1+2+3
            }
        }
    }

    #[tokio::test]
    async fn test_aggregation_functions() {
        let events = vec![
            StreamEvent::new("key1".to_string(), serde_json::json!(1.0)),
            StreamEvent::new("key1".to_string(), serde_json::json!(2.0)),
            StreamEvent::new("key1".to_string(), serde_json::json!(3.0)),
        ];
        let events_deque: VecDeque<_> = events.into_iter().collect();

        // Test Sum
        let processor =
            StreamProcessor::new(WindowType::Count { count: 3 }, AggregationFunction::Sum);
        let sum = processor.compute_aggregation(&events_deque).unwrap();
        assert_eq!(sum, serde_json::json!(6.0));

        // Test Avg
        let processor =
            StreamProcessor::new(WindowType::Count { count: 3 }, AggregationFunction::Avg);
        let avg = processor.compute_aggregation(&events_deque).unwrap();
        assert_eq!(avg, serde_json::json!(2.0));

        // Test Min
        let processor =
            StreamProcessor::new(WindowType::Count { count: 3 }, AggregationFunction::Min);
        let min = processor.compute_aggregation(&events_deque).unwrap();
        assert_eq!(min, serde_json::json!(1.0));

        // Test Max
        let processor =
            StreamProcessor::new(WindowType::Count { count: 3 }, AggregationFunction::Max);
        let max = processor.compute_aggregation(&events_deque).unwrap();
        assert_eq!(max, serde_json::json!(3.0));
    }

    #[test]
    fn test_stream_event_creation() {
        let event = StreamEvent::new("test".to_string(), serde_json::json!(42));
        assert_eq!(event.key, "test");
        assert_eq!(event.as_number(), Some(42.0));
    }
}
