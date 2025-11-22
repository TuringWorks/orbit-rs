//! REST API Server with Natural Language Query Support
//!
//! This example demonstrates how to set up a REST API server with
//! natural language query capabilities using the MCP server.

use orbit_client::OrbitClient;
use orbit_protocols::mcp::{
    integration::OrbitMcpIntegration,
    server::McpServer,
    McpCapabilities, McpConfig,
};
use orbit_protocols::postgres_wire::query_engine::QueryEngine;
use orbit_protocols::rest::{RestApiConfig, RestApiServer};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Orbit-RS REST API with Natural Language Queries ===\n");

    // Create Orbit client
    let orbit_client = OrbitClient::builder()
        .with_namespace("api")
        .build()
        .await?;

    // Create query engine
    let query_engine = Arc::new(QueryEngine::new());

    // Create MCP integration
    let integration = Arc::new(OrbitMcpIntegration::new(query_engine));

    // Create MCP server with integration
    let mcp_config = McpConfig::default();
    let mcp_capabilities = McpCapabilities::default();
    let mcp_server = Arc::new(McpServer::with_orbit_integration(
        mcp_config,
        mcp_capabilities,
        integration,
    ));

    // Create REST API server with MCP support
    let rest_config = RestApiConfig {
        bind_address: "0.0.0.0:8080".to_string(),
        enable_cors: true,
        enable_tracing: true,
        api_prefix: "/api/v1".to_string(),
    };

    let server = RestApiServer::with_mcp(orbit_client, rest_config, mcp_server);

    println!("Starting REST API server on http://0.0.0.0:8080");
    println!("Natural Language Query endpoints:");
    println!("  POST /api/v1/query/natural-language");
    println!("  POST /api/v1/query/generate-sql");
    println!("\nOpenAPI documentation: http://0.0.0.0:8080/openapi.json");
    println!("Health check: http://0.0.0.0:8080/health\n");

    // Start server
    server.run().await?;

    Ok(())
}

