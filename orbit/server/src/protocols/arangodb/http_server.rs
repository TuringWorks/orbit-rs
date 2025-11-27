//! ArangoDB HTTP Server Implementation
//!
//! Handles incoming ArangoDB HTTP API requests.

#![cfg(feature = "protocol-arangodb")]

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use orbit_protocols::arangodb::ArangoHttpProtocol;
use serde_json::Value;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// ArangoDB Server State
#[derive(Clone)]
struct ServerState {
    protocol: Arc<ArangoHttpProtocol>,
}

/// ArangoDB HTTP Server
pub struct ArangoServer {
    bind_address: String,
    protocol: Arc<ArangoHttpProtocol>,
}

impl ArangoServer {
    /// Create a new ArangoDB server
    pub fn new(bind_address: String) -> Self {
        Self {
            bind_address,
            protocol: Arc::new(ArangoHttpProtocol::new()),
        }
    }

    /// Start the server
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let state = ServerState {
            protocol: self.protocol.clone(),
        };

        let app = Router::new()
            .route("/_api/cursor", post(handle_cursor))
            .route("/_api/version", get(handle_version))
            .with_state(state);

        let listener = TcpListener::bind(&self.bind_address).await?;
        info!("🥑 ArangoDB server listening on {}", self.bind_address);

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Handle POST /_api/cursor
async fn handle_cursor(State(state): State<ServerState>, Json(payload): Json<Value>) -> Response {
    // Convert payload to string for protocol handler
    let body = payload.to_string();

    match state.protocol.handle_cursor_request(&body).await {
        Ok(response_body) => {
            // Parse back to JSON to return as Json response
            // This is a bit inefficient (Json -> String -> Json -> String) but keeps protocol decoupled
            match serde_json::from_str::<Value>(&response_body) {
                Ok(json) => Json(json).into_response(),
                Err(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid response JSON").into_response()
                }
            }
        }
        Err(e) => {
            error!("ArangoDB request failed: {}", e);
            // Return ArangoDB style error
            let error_response = serde_json::json!({
                "error": true,
                "errorMessage": e.to_string(),
                "code": 500,
                "errorNum": 500
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
        }
    }
}

/// Handle GET /_api/version
async fn handle_version() -> Response {
    let version_response = serde_json::json!({
        "server": "arango",
        "version": "3.10.0", // Pretend to be ArangoDB 3.10
        "license": "community"
    });
    Json(version_response).into_response()
}
