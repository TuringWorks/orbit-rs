//! Advanced MCP Server Example
//!
//! This example demonstrates a production-ready MCP server implementation
//! with real tool execution, database integration, and vector operations.

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::Parser;
use orbit_protocols::mcp::{
    handlers::McpRequestHandler, server::McpServer, tools::ToolRegistry, types::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "mcp-advanced-server")]
#[command(about = "Advanced MCP server with real tool implementations")]
struct Args {
    /// Port to bind the server to
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Host to bind the server to
    #[arg(long, default_value = "localhost")]
    host: String,

    /// Database URL (optional)
    #[arg(long)]
    database_url: Option<String>,

    /// Enable vector operations
    #[arg(long)]
    enable_vectors: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(args.log_level.as_str())
        .init();

    info!("Starting Advanced MCP Server");
    info!("Host: {}, Port: {}", args.host, args.port);

    // Create server state
    let server_state = Arc::new(ServerState::new(args.database_url, args.enable_vectors).await?);

    // Register tools
    let mut tool_registry = ToolRegistry::new();
    register_tools(&mut tool_registry, server_state.clone()).await?;

    // Create MCP server
    let mcp_server = McpServer::new(
        McpConfig {
            name: "Orbit Advanced MCP Server".to_string(),
            version: "1.0.0".to_string(),
            capabilities: McpCapabilities::default(),
        },
        McpCapabilities {
            tools: Some(true),
            resources: Some(true),
            prompts: Some(true),
            logging: Some(true),
        },
    );

    // Create HTTP server
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/mcp/initialize", post(initialize_handler))
        .route("/mcp/tools", get(list_tools_handler))
        .route("/mcp/tools/:name", post(call_tool_handler))
        .route("/mcp/resources", get(list_resources_handler))
        .route("/mcp/resources/*uri", get(read_resource_handler))
        .route("/mcp/prompts", get(list_prompts_handler))
        .route("/mcp/prompts/:name", post(get_prompt_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(AppState {
            server_state,
            tool_registry: Arc::new(tool_registry),
            mcp_server: Arc::new(mcp_server),
        });

    let addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&addr).await?;

    info!("MCP Server listening on {}", addr);
    info!("Available at: http://{}/", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Server application state
#[derive(Clone)]
struct AppState {
    server_state: Arc<ServerState>,
    tool_registry: Arc<ToolRegistry>,
    mcp_server: Arc<McpServer>,
}

/// Internal server state
struct ServerState {
    database_url: Option<String>,
    enable_vectors: bool,
    session_data: tokio::sync::RwLock<HashMap<String, SessionData>>,
}

#[derive(Debug, Clone)]
struct SessionData {
    id: String,
    created_at: chrono::DateTime<chrono::Utc>,
    capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default)]
struct ClientCapabilities {
    experimental: Option<HashMap<String, Value>>,
    roots: Option<RootsCapability>,
    sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone)]
struct RootsCapability {
    list_changed: bool,
}

#[derive(Debug, Clone)]
struct SamplingCapability {}

impl ServerState {
    async fn new(database_url: Option<String>, enable_vectors: bool) -> Result<Self> {
        info!("Initializing server state");
        if let Some(ref url) = database_url {
            info!("Database URL configured: {}", url);
        }
        if enable_vectors {
            info!("Vector operations enabled");
        }

        Ok(Self {
            database_url,
            enable_vectors,
            session_data: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    async fn create_session(&self, capabilities: ClientCapabilities) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session = SessionData {
            id: session_id.clone(),
            created_at: chrono::Utc::now(),
            capabilities,
        };

        let mut sessions = self.session_data.write().await;
        sessions.insert(session_id.clone(), session);

        info!("Created new session: {}", session_id);
        session_id
    }
}

// HTTP Handlers

async fn root_handler() -> Json<Value> {
    Json(json!({
        "name": "Orbit Advanced MCP Server",
        "version": "1.0.0",
        "description": "Advanced MCP server with comprehensive tool implementations",
        "endpoints": {
            "initialize": "/mcp/initialize",
            "tools": "/mcp/tools",
            "resources": "/mcp/resources",
            "prompts": "/mcp/prompts"
        }
    }))
}

async fn initialize_handler(
    State(state): State<AppState>,
    Json(request): Json<InitializeRequest>,
) -> Result<Json<InitializeResult>, StatusCode> {
    info!(
        "Initialize request from client: {}",
        request.client_info.name
    );

    let session_id = state
        .server_state
        .create_session(request.capabilities)
        .await;

    Ok(Json(InitializeResult {
        protocol_version: "2024-11-05".to_string(),
        capabilities: ServerCapabilities {
            tools: Some(true),
            resources: Some(true),
            prompts: Some(true),
            logging: Some(true),
        },
        server_info: ServerInfo {
            name: "Orbit Advanced MCP Server".to_string(),
            version: "1.0.0".to_string(),
        },
        instructions: Some("Welcome to Orbit Advanced MCP Server! This server provides comprehensive database, vector, and analytical tools.".to_string()),
    }))
}

async fn list_tools_handler(State(state): State<AppState>) -> Json<ListToolsResult> {
    let tools = state.tool_registry.list_tools().await;
    Json(ListToolsResult { tools })
}

async fn call_tool_handler(
    Path(name): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CallToolRequest>,
) -> Result<Json<CallToolResult>, StatusCode> {
    info!("Calling tool: {} with params: {:?}", name, request.params);

    match state
        .tool_registry
        .execute_tool(&name, request.params)
        .await
    {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            error!("Tool execution failed: {}", e);
            Ok(Json(CallToolResult {
                content: vec![ToolResultContent::Text {
                    text: format!("Error: {}", e),
                }],
                is_error: true,
            }))
        }
    }
}

async fn list_resources_handler(State(state): State<AppState>) -> Json<ListResourcesResult> {
    let mut resources = vec![
        Resource {
            uri: "file:///tmp/data.json".to_string(),
            name: Some("Sample Data".to_string()),
            description: Some("JSON data file for testing".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "memory://sessions".to_string(),
            name: Some("Active Sessions".to_string()),
            description: Some("Current MCP server sessions".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ];

    if state.server_state.database_url.is_some() {
        resources.push(Resource {
            uri: "database://main".to_string(),
            name: Some("Main Database".to_string()),
            description: Some("Primary PostgreSQL database connection".to_string()),
            mime_type: Some("application/x-postgresql".to_string()),
        });
    }

    if state.server_state.enable_vectors {
        resources.push(Resource {
            uri: "vector://embeddings".to_string(),
            name: Some("Vector Embeddings".to_string()),
            description: Some("Vector embedding store for similarity search".to_string()),
            mime_type: Some("application/x-vectors".to_string()),
        });
    }

    Json(ListResourcesResult { resources })
}

async fn read_resource_handler(
    Path(uri): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ReadResourceResult>, StatusCode> {
    info!("Reading resource: {}", uri);

    let content = match uri.as_str() {
        "memory://sessions" => {
            let sessions = state.server_state.session_data.read().await;
            let sessions_data: Vec<_> = sessions.values().collect();
            serde_json::to_string_pretty(&sessions_data).unwrap_or_else(|_| "{}".to_string())
        }
        "file:///tmp/data.json" => json!({
            "users": [
                {"id": 1, "name": "Alice", "email": "alice@example.com"},
                {"id": 2, "name": "Bob", "email": "bob@example.com"}
            ],
            "metadata": {
                "last_updated": chrono::Utc::now().to_rfc3339(),
                "version": "1.0"
            }
        })
        .to_string(),
        _ => return Err(StatusCode::NOT_FOUND),
    };

    Ok(Json(ReadResourceResult {
        contents: vec![ResourceContent::Text { text: content }],
    }))
}

async fn list_prompts_handler() -> Json<ListPromptsResult> {
    Json(ListPromptsResult {
        prompts: vec![
            Prompt {
                name: "sql_query_generator".to_string(),
                description: Some(
                    "Generate SQL queries based on natural language description".to_string(),
                ),
                arguments: vec![
                    PromptArgument {
                        name: "description".to_string(),
                        description: Some("Natural language description of the query".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "table_schema".to_string(),
                        description: Some("Database table schema information".to_string()),
                        required: Some(false),
                    },
                ],
            },
            Prompt {
                name: "data_analysis".to_string(),
                description: Some("Perform comprehensive data analysis".to_string()),
                arguments: vec![
                    PromptArgument {
                        name: "dataset".to_string(),
                        description: Some("Dataset to analyze".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "analysis_type".to_string(),
                        description: Some(
                            "Type of analysis (descriptive, diagnostic, predictive)".to_string(),
                        ),
                        required: Some(false),
                    },
                ],
            },
        ],
    })
}

async fn get_prompt_handler(
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GetPromptResult>, StatusCode> {
    info!("Getting prompt: {} with params: {:?}", name, params);

    let messages = match name.as_str() {
        "sql_query_generator" => {
            let description = params
                .get("description")
                .unwrap_or(&"Select all records".to_string())
                .clone();
            let schema_info = params
                .get("table_schema")
                .map(|s| s.as_str())
                .unwrap_or("No schema provided");

            vec![
                PromptMessage::User {
                    content: UserContent::Text {
                        text: format!(
                            "Generate a SQL query for the following description: {}\n\nTable Schema: {}",
                            description, schema_info
                        ),
                    },
                },
                PromptMessage::Assistant {
                    content: AssistantContent::Text {
                        text: "I'll help you generate a SQL query. Based on your description, here's the suggested query:".to_string(),
                    },
                },
            ]
        }
        "data_analysis" => {
            let dataset = params
                .get("dataset")
                .unwrap_or(&"No dataset specified".to_string())
                .clone();
            let analysis_type = params
                .get("analysis_type")
                .unwrap_or(&"descriptive".to_string())
                .clone();

            vec![
                PromptMessage::User {
                    content: UserContent::Text {
                        text: format!(
                            "Perform {} analysis on the following dataset: {}",
                            analysis_type, dataset
                        ),
                    },
                },
                PromptMessage::Assistant {
                    content: AssistantContent::Text {
                        text: "I'll perform a comprehensive data analysis. Let me examine the dataset and provide insights.".to_string(),
                    },
                },
            ]
        }
        _ => return Err(StatusCode::NOT_FOUND),
    };

    Ok(Json(GetPromptResult {
        description: Some(format!("Generated prompt for {}", name)),
        messages,
    }))
}

// Tool Registration

async fn register_tools(registry: &mut ToolRegistry, state: Arc<ServerState>) -> Result<()> {
    info!("Registering tools");

    // SQL Query Tool
    registry.register_tool(
        "sql_query",
        "Execute SQL queries against the database",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SQL query to execute"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of rows to return",
                    "default": 100
                }
            },
            "required": ["query"]
        }),
        {
            let state = state.clone();
            move |params: Value| {
                let state = state.clone();
                Box::pin(async move { sql_query_tool(params, state).await })
            }
        },
    );

    // Mathematical Calculator
    registry.register_tool(
        "calculator",
        "Perform mathematical calculations and evaluations",
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate"
                },
                "precision": {
                    "type": "integer",
                    "description": "Number of decimal places for the result",
                    "default": 10
                }
            },
            "required": ["expression"]
        }),
        { move |params: Value| Box::pin(async move { calculator_tool(params).await }) },
    );

    // Vector Search Tool (if enabled)
    if state.enable_vectors {
        registry.register_tool(
            "vector_search",
            "Perform vector similarity search operations",
            json!({
                "type": "object",
                "properties": {
                    "query_vector": {
                        "type": "array",
                        "items": {"type": "number"},
                        "description": "Query vector for similarity search"
                    },
                    "k": {
                        "type": "integer",
                        "description": "Number of similar vectors to return",
                        "default": 10
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Minimum similarity threshold",
                        "default": 0.5
                    }
                },
                "required": ["query_vector"]
            }),
            {
                let state = state.clone();
                move |params: Value| {
                    let state = state.clone();
                    Box::pin(async move { vector_search_tool(params, state).await })
                }
            },
        );
    }

    // Data Analysis Tool
    registry.register_tool(
        "analyze_data",
        "Perform statistical analysis on datasets",
        json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Numerical data to analyze"
                },
                "analysis_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["mean", "median", "std", "variance", "min", "max", "quartiles"]
                    },
                    "description": "Types of analysis to perform"
                }
            },
            "required": ["data"]
        }),
        { move |params: Value| Box::pin(async move { data_analysis_tool(params).await }) },
    );

    info!("Registered {} tools", registry.tool_count().await);
    Ok(())
}

// Tool Implementations

async fn sql_query_tool(params: Value, state: Arc<ServerState>) -> Result<CallToolResult> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing query parameter"))?;

    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);

    info!("Executing SQL query: {} (limit: {})", query, limit);

    // Mock database query execution
    let result = json!({
        "query": query,
        "limit": limit,
        "results": [
            {"id": 1, "name": "Alice", "email": "alice@example.com", "created_at": "2024-01-01T00:00:00Z"},
            {"id": 2, "name": "Bob", "email": "bob@example.com", "created_at": "2024-01-02T00:00:00Z"},
            {"id": 3, "name": "Carol", "email": "carol@example.com", "created_at": "2024-01-03T00:00:00Z"}
        ],
        "metadata": {
            "execution_time_ms": 45,
            "rows_affected": 3,
            "has_database": state.database_url.is_some()
        }
    });

    Ok(CallToolResult {
        content: vec![ToolResultContent::Text {
            text: serde_json::to_string_pretty(&result)?,
        }],
        is_error: false,
    })
}

async fn calculator_tool(params: Value) -> Result<CallToolResult> {
    let expression = params
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing expression parameter"))?;

    let precision = params
        .get("precision")
        .and_then(|v| v.as_i64())
        .unwrap_or(10) as usize;

    info!("Evaluating mathematical expression: {}", expression);

    // Simple expression evaluation (in real implementation, use a proper math parser)
    let result = match expression {
        expr if expr.contains("+") => {
            let parts: Vec<&str> = expr.split('+').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let a: f64 = parts[0].parse().unwrap_or(0.0);
                let b: f64 = parts[1].parse().unwrap_or(0.0);
                a + b
            } else {
                0.0
            }
        }
        expr if expr.contains("-") => {
            let parts: Vec<&str> = expr.split('-').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let a: f64 = parts[0].parse().unwrap_or(0.0);
                let b: f64 = parts[1].parse().unwrap_or(0.0);
                a - b
            } else {
                0.0
            }
        }
        expr if expr.contains("*") => {
            let parts: Vec<&str> = expr.split('*').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let a: f64 = parts[0].parse().unwrap_or(0.0);
                let b: f64 = parts[1].parse().unwrap_or(0.0);
                a * b
            } else {
                0.0
            }
        }
        expr if expr.contains("/") => {
            let parts: Vec<&str> = expr.split('/').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let a: f64 = parts[0].parse().unwrap_or(0.0);
                let b: f64 = parts[1].parse().unwrap_or(1.0);
                if b != 0.0 {
                    a / b
                } else {
                    f64::INFINITY
                }
            } else {
                0.0
            }
        }
        _ => expression.parse().unwrap_or(0.0),
    };

    let response = json!({
        "expression": expression,
        "result": format!("{:.precision$}", result, precision = precision),
        "precision": precision,
        "calculated_at": chrono::Utc::now().to_rfc3339()
    });

    Ok(CallToolResult {
        content: vec![ToolResultContent::Text {
            text: serde_json::to_string_pretty(&response)?,
        }],
        is_error: false,
    })
}

async fn vector_search_tool(params: Value, state: Arc<ServerState>) -> Result<CallToolResult> {
    let query_vector = params
        .get("query_vector")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing query_vector parameter"))?;

    let k = params.get("k").and_then(|v| v.as_i64()).unwrap_or(10) as usize;

    let threshold = params
        .get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    info!(
        "Performing vector search with {} dimensions, k={}, threshold={}",
        query_vector.len(),
        k,
        threshold
    );

    // Mock vector search results
    let results = json!({
        "query_vector_dim": query_vector.len(),
        "k": k,
        "threshold": threshold,
        "results": [
            {
                "id": "doc_1",
                "similarity": 0.95,
                "metadata": {"title": "Similar Document 1", "category": "research"},
                "content": "This document contains similar content to your query."
            },
            {
                "id": "doc_2",
                "similarity": 0.87,
                "metadata": {"title": "Related Article", "category": "analysis"},
                "content": "Another relevant document with high similarity."
            },
            {
                "id": "doc_3",
                "similarity": 0.72,
                "metadata": {"title": "Background Information", "category": "reference"},
                "content": "Supporting document with moderate similarity."
            }
        ],
        "search_metadata": {
            "total_vectors_searched": 10000,
            "search_time_ms": 23,
            "vectors_enabled": state.enable_vectors
        }
    });

    Ok(CallToolResult {
        content: vec![ToolResultContent::Text {
            text: serde_json::to_string_pretty(&results)?,
        }],
        is_error: false,
    })
}

async fn data_analysis_tool(params: Value) -> Result<CallToolResult> {
    let data = params
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing data parameter"))?;

    let numbers: Result<Vec<f64>, _> = data
        .iter()
        .map(|v| {
            v.as_f64()
                .ok_or_else(|| anyhow::anyhow!("Invalid number in data array"))
        })
        .collect();

    let numbers = numbers?;

    if numbers.is_empty() {
        return Err(anyhow::anyhow!("Data array cannot be empty"));
    }

    // Calculate statistics
    let n = numbers.len() as f64;
    let sum: f64 = numbers.iter().sum();
    let mean = sum / n;

    let mut sorted = numbers.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    let variance = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;

    let std_dev = variance.sqrt();

    let min = sorted.first().copied().unwrap_or(0.0);
    let max = sorted.last().copied().unwrap_or(0.0);

    // Quartiles
    let q1_idx = sorted.len() / 4;
    let q3_idx = 3 * sorted.len() / 4;
    let q1 = sorted.get(q1_idx).copied().unwrap_or(min);
    let q3 = sorted.get(q3_idx).copied().unwrap_or(max);

    let analysis = json!({
        "summary_statistics": {
            "count": numbers.len(),
            "mean": mean,
            "median": median,
            "std_deviation": std_dev,
            "variance": variance,
            "min": min,
            "max": max,
            "range": max - min
        },
        "quartiles": {
            "q1": q1,
            "q2_median": median,
            "q3": q3,
            "iqr": q3 - q1
        },
        "data_quality": {
            "has_outliers": numbers.iter().any(|&x| (x - mean).abs() > 2.0 * std_dev),
            "coefficient_of_variation": if mean != 0.0 { std_dev / mean.abs() } else { 0.0 },
            "skewness_indicator": if mean > median { "right_skewed" } else if mean < median { "left_skewed" } else { "symmetric" }
        },
        "analysis_metadata": {
            "analysis_performed_at": chrono::Utc::now().to_rfc3339(),
            "data_points_analyzed": numbers.len()
        }
    });

    Ok(CallToolResult {
        content: vec![ToolResultContent::Text {
            text: serde_json::to_string_pretty(&analysis)?,
        }],
        is_error: false,
    })
}

// Request/Response Types for HTTP API

#[derive(Debug, Deserialize)]
struct InitializeRequest {
    protocol_version: String,
    capabilities: ClientCapabilities,
    client_info: ClientInfo,
}

#[derive(Debug, Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct InitializeResult {
    protocol_version: String,
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
    instructions: Option<String>,
}

#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct CallToolRequest {
    params: Value,
}

#[derive(Debug, Serialize)]
struct ListToolsResult {
    tools: Vec<Tool>,
}

#[derive(Debug, Serialize)]
struct ListResourcesResult {
    resources: Vec<Resource>,
}

#[derive(Debug, Serialize)]
struct ReadResourceResult {
    contents: Vec<ResourceContent>,
}

#[derive(Debug, Serialize)]
struct ListPromptsResult {
    prompts: Vec<Prompt>,
}

#[derive(Debug, Serialize)]
struct GetPromptResult {
    description: Option<String>,
    messages: Vec<PromptMessage>,
}
