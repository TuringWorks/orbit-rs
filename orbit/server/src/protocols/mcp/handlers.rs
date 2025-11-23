//! MCP request handlers

use super::server::McpServer;
use super::types::{McpError, McpRequest, McpResponse};
use serde_json::json;
use std::sync::Arc;

/// Handle MCP requests
pub async fn handle_request(request: McpRequest, server: Option<Arc<McpServer>>) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(&request),
        "tools/list" => handle_tools_list(&request),
        "tools/call" => handle_tool_call(&request, server.as_ref()).await,
        "resources/list" => handle_resources_list(&request),
        "resources/read" => handle_resource_read(&request, server.as_ref()).await,
        "prompts/list" => handle_prompts_list(&request),
        "prompts/get" => handle_prompt_get(&request, server.as_ref()).await,
        _ => McpResponse::error(request.id, McpError::MethodNotFound(request.method)),
    }
}

/// Handle initialize request
pub fn handle_initialize(request: &McpRequest) -> McpResponse {
    let result = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {
                "listChanged": true
            },
            "resources": {
                "subscribe": true,
                "listChanged": true
            },
            "prompts": {
                "listChanged": true
            },
            "logging": {}
        },
        "serverInfo": {
            "name": "orbit-mcp-server",
            "version": "0.1.0"
        }
    });

    McpResponse::success(request.id.clone(), result)
}

/// Handle tools list request
pub fn handle_tools_list(request: &McpRequest) -> McpResponse {
    let tools = super::tools::get_available_tools();
    let result = json!({ "tools": tools });
    McpResponse::success(request.id.clone(), result)
}

/// Handle tool call request
pub async fn handle_tool_call(request: &McpRequest, _server: Option<&Arc<McpServer>>) -> McpResponse {
    if let (Some(name), Some(arguments)) = (
        request.params.get("name").and_then(|v| v.as_str()),
        request.params.get("arguments").and_then(|v| v.as_object()),
    ) {
        let arguments_map = arguments
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let tool_result = super::tools::execute_tool(name, arguments_map).await;
        let result = json!({ "content": [tool_result] });
        McpResponse::success(request.id.clone(), result)
    } else {
        McpResponse::error(
            request.id.clone(),
            McpError::InvalidRequest("Missing tool name or arguments".to_string()),
        )
    }
}

/// Handle resources list request
pub fn handle_resources_list(request: &McpRequest) -> McpResponse {
    let resources = vec![
        json!({
            "uri": "memory://actors",
            "name": "Active Actors",
            "description": "List of active actors in the Orbit-RS system",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "memory://schemas",
            "name": "Database Schemas",
            "description": "Database table schemas and metadata",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "memory://metrics",
            "name": "System Metrics",
            "description": "System performance metrics and statistics",
            "mimeType": "application/json"
        }),
    ];
    let result = json!({ "resources": resources });
    McpResponse::success(request.id.clone(), result)
}

/// Handle resource read request
pub async fn handle_resource_read(request: &McpRequest, server: Option<&Arc<McpServer>>) -> McpResponse {
    if let Some(uri) = request.params.get("uri").and_then(|v| v.as_str()) {
        // Parse resource URI
        if uri.starts_with("memory://") {
            // Memory resource - return system information
            let resource_type = uri.strip_prefix("memory://").unwrap_or("");
            match resource_type {
                "actors" => {
                    // Return actor information - try to get real data if server is available
                    let content = if let Some(server) = server {
                        if let Some(ref _integration) = server.orbit_integration {
                            // Try to get actual actor data
                            json!({
                                "type": "actor_list",
                                "description": "List of active actors in the Orbit-RS system",
                                "note": "Actor data available through Orbit-RS integration",
                                "integration_available": true
                            })
                        } else {
                            json!({
                                "type": "actor_list",
                                "description": "List of active actors in the system",
                                "note": "Use tools/actor_list to get actual actor data",
                                "integration_available": false
                            })
                        }
                    } else {
                        json!({
                            "type": "actor_list",
                            "description": "List of active actors in the system",
                            "note": "Use tools/actor_list to get actual actor data"
                        })
                    };
                    
                    let result = json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": content.to_string()
                        }]
                    });
                    McpResponse::success(request.id.clone(), result)
                }
                "schemas" => {
                    // Return schema information - try to get real schema data if server is available
                    let content = if let Some(server) = server {
                        if let Some(ref _integration) = server.orbit_integration {
                            // Try to get actual schema data
                            json!({
                                "type": "schema_list",
                                "description": "Database schemas and metadata",
                                "note": "Use tools/describe_schema or tools/list_tables to get actual schema data",
                                "integration_available": true,
                                "schema_analyzer_available": true
                            })
                        } else {
                            json!({
                                "type": "schema_list",
                                "description": "Database schemas",
                                "note": "Use tools/describe_schema to get actual schema data",
                                "integration_available": false
                            })
                        }
                    } else {
                        json!({
                            "type": "schema_list",
                            "description": "Database schemas",
                            "note": "Use tools/describe_schema to get actual schema data"
                        })
                    };
                    
                    let result = json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": content.to_string()
                        }]
                    });
                    McpResponse::success(request.id.clone(), result)
                }
                "metrics" => {
                    // Return metrics information
                    let content = json!({
                        "type": "metrics",
                        "description": "System performance metrics and statistics",
                        "note": "Use tools/analyze_data to get actual metrics",
                        "available_metrics": [
                            "query_count",
                            "execution_time",
                            "error_count",
                            "connection_count"
                        ]
                    });
                    
                    let result = json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": content.to_string()
                        }]
                    });
                    McpResponse::success(request.id.clone(), result)
                }
                _ => {
                    let result = json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "text/plain",
                            "text": format!("Resource '{}' not found. Available resources: actors, schemas, metrics", resource_type)
                        }]
                    });
                    McpResponse::success(request.id.clone(), result)
                }
            }
        } else {
            McpResponse::error(
                request.id.clone(),
                McpError::InvalidRequest(format!("Unsupported resource URI: {}", uri)),
            )
        }
    } else {
        McpResponse::error(
            request.id.clone(),
            McpError::InvalidRequest("Missing 'uri' parameter".to_string()),
        )
    }
}

/// Handle prompts list request
pub fn handle_prompts_list(request: &McpRequest) -> McpResponse {
    let prompts = vec![
        json!({
            "name": "system_analysis",
            "description": "Analyze the Orbit-RS system and provide insights",
            "arguments": []
        }),
        json!({
            "name": "query_help",
            "description": "Get help writing natural language queries",
            "arguments": []
        }),
        json!({
            "name": "schema_exploration",
            "description": "Explore database schemas and structure",
            "arguments": []
        }),
    ];
    let result = json!({ "prompts": prompts });
    McpResponse::success(request.id.clone(), result)
}

/// Handle prompt get request
pub async fn handle_prompt_get(request: &McpRequest, server: Option<&Arc<McpServer>>) -> McpResponse {
    if let Some(name) = request.params.get("name").and_then(|v| v.as_str()) {
        // Check if server integration is available for dynamic prompts
        let has_integration = server
            .and_then(|s| s.orbit_integration.as_ref())
            .is_some();
        
        match name {
            "system_analysis" => {
                let prompt_text = if has_integration {
                    "Analyze the Orbit-RS system and provide insights about:\n1. System health and performance\n2. Active actors and their states\n3. Database schemas and data distribution\n4. Recommendations for optimization\n\nUse the available MCP tools to gather real-time system information."
                } else {
                    "Analyze the Orbit-RS system and provide insights about:\n1. System health and performance\n2. Active actors and their states\n3. Database schemas and data distribution\n4. Recommendations for optimization"
                };
                
                let result = json!({
                    "messages": [
                        {
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": prompt_text
                            }
                        }
                    ]
                });
                McpResponse::success(request.id.clone(), result)
            }
            "query_help" => {
                let prompt_text = if has_integration {
                    "Help me write a natural language query to interact with Orbit-RS:\n\nQuery Types:\n- Data Retrieval: 'Show me all users from California'\n- Data Analysis: 'What are the top 10 products by revenue?'\n- Statistical Analysis: 'Analyze the distribution of customer ages'\n- Schema Exploration: 'What tables are available?'\n\nYou can use natural language and I'll convert it to SQL automatically using the MCP tools."
                } else {
                    "Help me write a natural language query to:\n- Retrieve data from tables\n- Analyze data patterns\n- Get system information\n\nExamples:\n- 'Show me all users from California'\n- 'What are the top 10 products by revenue?'\n- 'Analyze the distribution of customer ages'"
                };
                
                let result = json!({
                    "messages": [
                        {
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": prompt_text
                            }
                        }
                    ]
                });
                McpResponse::success(request.id.clone(), result)
            }
            "schema_exploration" => {
                let prompt_text = if has_integration {
                    "Explore the Orbit-RS database schema. You can ask questions like:\n- 'What tables are available?' (use list_tables tool)\n- 'Show me the schema for the users table' (use describe_schema tool)\n- 'What columns does the products table have?' (use describe_schema tool)\n- 'List all indexes' (use describe_schema tool)\n\nI have access to real-time schema information through the MCP integration."
                } else {
                    "Explore the database schema by asking questions like:\n- 'What tables are available?'\n- 'Show me the schema for the users table'\n- 'What columns does the products table have?'\n- 'List all indexes'"
                };
                
                let result = json!({
                    "messages": [
                        {
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": prompt_text
                            }
                        }
                    ]
                });
                McpResponse::success(request.id.clone(), result)
            }
            _ => {
                McpResponse::error(
                    request.id.clone(),
                    McpError::InvalidRequest(format!("Unknown prompt: {}. Available prompts: system_analysis, query_help, schema_exploration", name)),
                )
            }
        }
    } else {
        McpResponse::error(
            request.id.clone(),
            McpError::InvalidRequest("Missing 'name' parameter".to_string()),
        )
    }
}
