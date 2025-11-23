//! Server-Sent Events (SSE) support for real-time data streaming
//!
//! This module provides SSE endpoints for streaming CDC events and other
//! real-time data to browsers and clients that prefer SSE over WebSockets.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
};
use futures::Stream;
use orbit_shared::cdc::{CdcFilter, CdcSubscription};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tracing::info;

use super::handlers::ApiState;

/// SSE subscription parameters
#[derive(Debug, Deserialize)]
pub struct SseParams {
    /// Tables to watch (comma-separated)
    pub tables: Option<String>,
    /// Operation types to watch (comma-separated: insert,update,delete)
    pub operations: Option<String>,
    /// Include old values in events
    #[serde(default = "default_true")]
    pub include_old: bool,
    /// Include new values in events
    #[serde(default = "default_true")]
    pub include_new: bool,
}

fn default_true() -> bool {
    true
}

/// SSE message format
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SseMessage {
    /// CDC event
    #[serde(rename = "cdc_event")]
    CdcEvent {
        event_id: String,
        timestamp: i64,
        operation: String,
        table: String,
        row_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        new_values: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        old_values: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transaction_id: Option<String>,
        lsn: u64,
    },
    /// Connection status
    #[serde(rename = "status")]
    Status { message: String },
    /// Error message
    #[serde(rename = "error")]
    Error { message: String },
}

/// SSE handler for CDC events
pub async fn handle_cdc_events(
    Query(params): Query<SseParams>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    info!("New SSE connection for CDC events");

    // Build CDC filter from parameters
    let mut filter = CdcFilter::all();

    if let Some(tables) = params.tables {
        let table_list: Vec<String> = tables
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !table_list.is_empty() {
            filter.tables = Some(table_list);
        }
    }

    if let Some(operations) = params.operations {
        let op_list: Vec<_> = operations
            .split(',')
            .filter_map(|s| {
                let op = s.trim().to_lowercase();
                match op.as_str() {
                    "insert" => Some(orbit_shared::cdc::DmlOperation::Insert),
                    "update" => Some(orbit_shared::cdc::DmlOperation::Update),
                    "delete" => Some(orbit_shared::cdc::DmlOperation::Delete),
                    "truncate" => Some(orbit_shared::cdc::DmlOperation::Truncate),
                    _ => None,
                }
            })
            .collect();
        if !op_list.is_empty() {
            filter.operations = Some(op_list);
        }
    }

    // Create CDC subscription
    let subscription = CdcSubscription {
        filter,
        include_old_values: params.include_old,
        include_new_values: params.include_new,
        ..Default::default()
    };

    // Get CDC coordinator from state (we'll need to add this to ApiState)
    // For now, we'll simulate with a placeholder
    let stream = create_event_stream(subscription, state);

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Create SSE event stream from CDC subscription
fn create_event_stream(
    subscription: CdcSubscription,
    _state: ApiState,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let subscription_id = subscription.subscription_id;

    async_stream::stream! {
        // Send initial status message
        let status = SseMessage::Status {
            message: format!("Connected with subscription ID: {}", subscription_id),
        };

        if let Ok(json) = serde_json::to_string(&status) {
            yield Ok(Event::default().data(json).event("status"));
        }

        // TODO: Get actual CDC stream from coordinator
        // This is a placeholder that would be replaced with:
        // let mut cdc_stream = state.cdc_coordinator.subscribe(subscription).await?;

        // Simulate events for demonstration
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        for i in 0..3 {
            interval.tick().await;

            let event = SseMessage::CdcEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                operation: "Insert".to_string(),
                table: "demo_table".to_string(),
                row_id: format!("{}", i),
                new_values: Some(serde_json::json!({
                    "id": i,
                    "name": format!("Item {}", i)
                })),
                old_values: None,
                transaction_id: None,
                lsn: i as u64,
            };

            if let Ok(json) = serde_json::to_string(&event) {
                yield Ok(Event::default().data(json).event("cdc_event"));
            }
        }

        info!("SSE stream ended for subscription {}", subscription_id);
    }
}

/// SSE handler for query results
pub async fn handle_query_stream(
    Query(_params): Query<QueryStreamParams>,
    State(_state): State<ApiState>,
) -> impl IntoResponse {
    info!("New SSE connection for query results");

    let stream = async_stream::stream! {
        // Send initial status
        let status = SseMessage::Status {
            message: "Query stream connected".to_string(),
        };

        if let Ok(json) = serde_json::to_string(&status) {
            yield Ok::<Event, Infallible>(Event::default().data(json).event("status"));
        }

        // TODO: Execute streaming query and send results
        // This would integrate with the StreamingQueryExecutor
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Query stream parameters
#[derive(Debug, Deserialize)]
pub struct QueryStreamParams {
    /// SQL query to execute
    pub query: String,
    /// Batch size for results
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_batch_size() -> usize {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_params_parsing() {
        let params = SseParams {
            tables: Some("users,orders".to_string()),
            operations: Some("insert,update".to_string()),
            include_old: true,
            include_new: true,
        };

        assert!(params.tables.is_some());
        assert!(params.operations.is_some());
    }

    #[test]
    fn test_sse_message_serialization() {
        let msg = SseMessage::CdcEvent {
            event_id: "123".to_string(),
            timestamp: 1234567890,
            operation: "Insert".to_string(),
            table: "users".to_string(),
            row_id: "1".to_string(),
            new_values: Some(serde_json::json!({"id": 1})),
            old_values: None,
            transaction_id: None,
            lsn: 1,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"cdc_event\""));
        assert!(json.contains("\"operation\":\"Insert\""));
    }
}
