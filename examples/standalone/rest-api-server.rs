//! # REST API Example
//!
//! This example demonstrates how to use the Orbit REST API server.
//!
//! ## Running
//!
//! ```bash
//! cargo run --example rest-api-server
//! ```
//!
//! ## Testing
//!
//! Once running, you can test the endpoints:
//!
//! ```bash
//! # Health check
//! curl http://localhost:8080/health
//!
//! # OpenAPI documentation
//! curl http://localhost:8080/openapi.json
//!
//! # List actors
//! curl http://localhost:8080/api/v1/actors
//!
//! # Create actor
//! curl -X POST http://localhost:8080/api/v1/actors \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "actor_type": "GreeterActor",
//!     "key": {"StringKey": {"key": "greeter-1"}},
//!     "initial_state": {"greetings": 0}
//!   }'
//!
//! # Get actor state
//! curl http://localhost:8080/api/v1/actors/GreeterActor/greeter-1
//!
//! # Invoke actor method
//! curl -X POST http://localhost:8080/api/v1/actors/GreeterActor/greeter-1/invoke \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "method": "greet",
//!     "args": ["World"],
//!     "timeout_ms": 5000
//!   }'
//!
//! # Update actor state
//! curl -X PUT http://localhost:8080/api/v1/actors/GreeterActor/greeter-1 \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "state": {"greetings": 42},
//!     "strategy": "merge"
//!   }'
//!
//! # Begin transaction
//! curl -X POST http://localhost:8080/api/v1/transactions \
//!   -H "Content-Type: application/json" \
//!   -d '{"timeout_ms": 30000}'
//!
//! # WebSocket connection (using websocat)
//! websocat ws://localhost:8080/api/v1/ws/actors/GreeterActor/greeter-1
//! websocat ws://localhost:8080/api/v1/ws/events
//! ```

use orbit_client::OrbitClient;
use orbit_protocols::rest::{RestApiServer, RestApiServerBuilder};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Orbit REST API server example");

    // Create OrbitClient (in a real scenario, this would connect to an Orbit cluster)
    // For this example, we'll use a builder pattern
    let orbit_client = OrbitClient::builder()
        .with_namespace("example")
        .with_url("http://localhost:50056") // Default Orbit server address
        .build()
        .await?;

    info!("Connected to Orbit cluster");

    // Create REST API server with custom configuration
    let server = RestApiServerBuilder::new()
        .bind_address("0.0.0.0:8080")
        .enable_cors(true)
        .enable_tracing(true)
        .api_prefix("/api/v1")
        .build(orbit_client);

    // Get WebSocket handler for broadcasting events
    let ws_handler = server.ws_handler();

    // Spawn a task to simulate broadcasting events
    tokio::spawn(async move {
        use orbit_protocols::rest::WebSocketMessage;
        use std::time::Duration;

        tokio::time::sleep(Duration::from_secs(5)).await;

        info!("Broadcasting example events");

        // Simulate actor activation event
        ws_handler
            .broadcast_event(WebSocketMessage::ActorActivated {
                actor_type: "GreeterActor".to_string(),
                key: serde_json::json!({"StringKey": {"key": "greeter-1"}}),
                node_id: "node-1".to_string(),
            })
            .await;

        tokio::time::sleep(Duration::from_secs(5)).await;

        // Simulate state change event
        ws_handler
            .broadcast_event(WebSocketMessage::ActorStateChanged {
                actor_type: "GreeterActor".to_string(),
                key: serde_json::json!({"StringKey": {"key": "greeter-1"}}),
                state: serde_json::json!({"greetings": 1}),
            })
            .await;
    });

    info!("REST API server starting on http://0.0.0.0:8080");
    info!("OpenAPI documentation: http://localhost:8080/openapi.json");
    info!("Health check: http://localhost:8080/health");
    info!("API endpoints: http://localhost:8080/api/v1/");

    // Run the server
    server.run().await?;

    Ok(())
}
