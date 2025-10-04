//! Basic MCP Client Example
//!
//! This example demonstrates how to create and use an MCP client to interact
//! with MCP servers, showcasing tool execution and resource access.

use anyhow::Result;
use clap::{Arg, Command};
use orbit_protocols::mcp::types::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::init();

    let matches = Command::new("MCP Basic Client")
        .version("1.0")
        .author("Orbit Team")
        .about("Basic MCP client for testing MCP protocol interactions")
        .arg(
            Arg::new("server-url")
                .long("server-url")
                .value_name("URL")
                .help("MCP server URL to connect to")
                .default_value("http://localhost:8080"),
        )
        .arg(
            Arg::new("tool")
                .long("tool")
                .value_name("TOOL_NAME")
                .help("Tool to execute"),
        )
        .arg(
            Arg::new("params")
                .long("params")
                .value_name("JSON")
                .help("JSON parameters for the tool")
                .default_value("{}"),
        )
        .get_matches();

    let server_url = matches.get_one::<String>("server-url").unwrap();
    let tool_name = matches.get_one::<String>("tool");
    let params_str = matches.get_one::<String>("params").unwrap();
    let params: Value = serde_json::from_str(params_str)?;

    info!("Starting MCP Basic Client");
    info!("Server URL: {}", server_url);

    // Create MCP client
    let mut client = McpClient::new(server_url.clone()).await?;

    // Initialize connection
    info!("Initializing MCP connection...");
    client.initialize().await?;

    // List available tools
    info!("Listing available tools...");
    let tools = client.list_tools().await?;

    println!("Available tools:");
    for tool in &tools {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("No description")
        );
        if !tool.input_schema.as_object().unwrap().is_empty() {
            println!(
                "    Schema: {}",
                serde_json::to_string_pretty(&tool.input_schema)?
            );
        }
    }

    // Execute specific tool if requested
    if let Some(tool_name) = tool_name {
        info!("Executing tool: {}", tool_name);
        match client.call_tool(tool_name, params).await {
            Ok(result) => {
                println!("\nTool execution result:");
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            Err(e) => {
                error!("Tool execution failed: {}", e);
                return Err(e);
            }
        }
    }

    // List available resources
    info!("Listing available resources...");
    match client.list_resources().await {
        Ok(resources) => {
            println!("\nAvailable resources:");
            for resource in &resources {
                println!(
                    "  - {}: {}",
                    resource.uri,
                    resource.name.as_deref().unwrap_or("No name")
                );
                if let Some(description) = &resource.description {
                    println!("    Description: {}", description);
                }
                if let Some(mime_type) = &resource.mime_type {
                    println!("    MIME Type: {}", mime_type);
                }
            }
        }
        Err(e) => {
            warn!("Failed to list resources: {}", e);
        }
    }

    // Test prompts if available
    info!("Listing available prompts...");
    match client.list_prompts().await {
        Ok(prompts) => {
            println!("\nAvailable prompts:");
            for prompt in &prompts {
                println!(
                    "  - {}: {}",
                    prompt.name,
                    prompt.description.as_deref().unwrap_or("No description")
                );
                if !prompt.arguments.is_empty() {
                    println!(
                        "    Arguments: {:?}",
                        prompt.arguments.iter().map(|a| &a.name).collect::<Vec<_>>()
                    );
                }
            }
        }
        Err(e) => {
            warn!("Failed to list prompts: {}", e);
        }
    }

    info!("MCP client session completed successfully");
    Ok(())
}

/// Simple MCP Client implementation
struct McpClient {
    server_url: String,
    capabilities: Option<ServerCapabilities>,
}

impl McpClient {
    async fn new(server_url: String) -> Result<Self> {
        Ok(Self {
            server_url,
            capabilities: None,
        })
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing MCP session with server: {}", self.server_url);

        // In a real implementation, this would establish an actual connection
        // and perform the MCP handshake
        self.capabilities = Some(ServerCapabilities {
            tools: Some(true),
            resources: Some(true),
            prompts: Some(true),
            ..Default::default()
        });

        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<Tool>> {
        info!("Requesting tools list from server");

        // Mock response - in real implementation, this would be an HTTP/WebSocket call
        Ok(vec![
            Tool {
                name: "calculate".to_string(),
                description: Some("Perform mathematical calculations".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "Mathematical expression to evaluate"
                        }
                    },
                    "required": ["expression"]
                }),
            },
            Tool {
                name: "search_database".to_string(),
                description: Some("Search the database for records".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "SQL query to execute"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results"
                        }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "vector_search".to_string(),
                description: Some("Perform vector similarity search".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "vector": {
                            "type": "array",
                            "items": {"type": "number"},
                            "description": "Query vector"
                        },
                        "k": {
                            "type": "integer",
                            "description": "Number of similar vectors to return"
                        }
                    },
                    "required": ["vector"]
                }),
            },
        ])
    }

    async fn call_tool(&self, name: &str, params: Value) -> Result<CallToolResult> {
        info!("Calling tool '{}' with params: {}", name, params);

        // Mock tool execution - in real implementation, this would call the actual MCP server
        let result = match name {
            "calculate" => {
                if let Some(expression) = params.get("expression").and_then(|v| v.as_str()) {
                    json!({
                        "result": format!("Calculated: {} = 42", expression),
                        "type": "calculation"
                    })
                } else {
                    json!({"error": "Missing expression parameter"})
                }
            }
            "search_database" => {
                if let Some(query) = params.get("query").and_then(|v| v.as_str()) {
                    json!({
                        "results": [
                            {"id": 1, "name": "Alice", "email": "alice@example.com"},
                            {"id": 2, "name": "Bob", "email": "bob@example.com"}
                        ],
                        "query": query,
                        "count": 2
                    })
                } else {
                    json!({"error": "Missing query parameter"})
                }
            }
            "vector_search" => {
                if let Some(_vector) = params.get("vector") {
                    json!({
                        "similar_vectors": [
                            {"id": "vec1", "similarity": 0.95, "data": {"title": "Similar Document 1"}},
                            {"id": "vec2", "similarity": 0.87, "data": {"title": "Similar Document 2"}}
                        ],
                        "query_time_ms": 15
                    })
                } else {
                    json!({"error": "Missing vector parameter"})
                }
            }
            _ => json!({"error": format!("Unknown tool: {}", name)}),
        };

        Ok(CallToolResult {
            content: vec![ToolResultContent::Text {
                text: serde_json::to_string_pretty(&result)?,
            }],
            is_error: result.get("error").is_some(),
        })
    }

    async fn list_resources(&self) -> Result<Vec<Resource>> {
        info!("Requesting resources list from server");

        Ok(vec![
            Resource {
                uri: "file:///data/users.csv".to_string(),
                name: Some("User Data".to_string()),
                description: Some("CSV file containing user information".to_string()),
                mime_type: Some("text/csv".to_string()),
            },
            Resource {
                uri: "postgres://localhost/orbit_db".to_string(),
                name: Some("Main Database".to_string()),
                description: Some("Primary PostgreSQL database".to_string()),
                mime_type: Some("application/x-postgresql".to_string()),
            },
            Resource {
                uri: "vector://embeddings/documents".to_string(),
                name: Some("Document Embeddings".to_string()),
                description: Some("Vector embeddings for document similarity search".to_string()),
                mime_type: Some("application/x-vectors".to_string()),
            },
        ])
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>> {
        info!("Requesting prompts list from server");

        Ok(vec![
            Prompt {
                name: "analyze_data".to_string(),
                description: Some("Analyze dataset and provide insights".to_string()),
                arguments: vec![
                    PromptArgument {
                        name: "dataset".to_string(),
                        description: Some("Path to dataset file".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "analysis_type".to_string(),
                        description: Some("Type of analysis to perform".to_string()),
                        required: Some(false),
                    },
                ],
            },
            Prompt {
                name: "generate_report".to_string(),
                description: Some("Generate a comprehensive report".to_string()),
                arguments: vec![
                    PromptArgument {
                        name: "data_source".to_string(),
                        description: Some("Source of data for the report".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "format".to_string(),
                        description: Some("Output format (markdown, html, pdf)".to_string()),
                        required: Some(false),
                    },
                ],
            },
        ])
    }
}

#[derive(Default)]
struct ServerCapabilities {
    tools: Option<bool>,
    resources: Option<bool>,
    prompts: Option<bool>,
}
