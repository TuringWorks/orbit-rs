//! MCP request handlers

use super::types::{McpError, McpRequest, McpResponse};
use serde_json::json;

/// Handle MCP requests
pub async fn handle_request(request: McpRequest) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(&request),
        "tools/list" => handle_tools_list(&request),
        "tools/call" => handle_tool_call(&request).await,
        "resources/list" => handle_resources_list(&request),
        "resources/read" => handle_resource_read(&request).await,
        "prompts/list" => handle_prompts_list(&request),
        "prompts/get" => handle_prompt_get(&request).await,
        _ => McpResponse::error(request.id, McpError::MethodNotFound(request.method)),
    }
}

/// Handle initialize request
fn handle_initialize(request: &McpRequest) -> McpResponse {
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
fn handle_tools_list(request: &McpRequest) -> McpResponse {
    let tools = super::tools::get_available_tools();
    let result = json!({ "tools": tools });
    McpResponse::success(request.id.clone(), result)
}

/// Handle tool call request
async fn handle_tool_call(request: &McpRequest) -> McpResponse {
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
fn handle_resources_list(request: &McpRequest) -> McpResponse {
    let result = json!({ "resources": [] });
    McpResponse::success(request.id.clone(), result)
}

/// Handle resource read request
async fn handle_resource_read(request: &McpRequest) -> McpResponse {
    McpResponse::error(
        request.id.clone(),
        McpError::MethodNotFound("resources/read not implemented".to_string()),
    )
}

/// Handle prompts list request
fn handle_prompts_list(request: &McpRequest) -> McpResponse {
    let result = json!({ "prompts": [] });
    McpResponse::success(request.id.clone(), result)
}

/// Handle prompt get request
async fn handle_prompt_get(request: &McpRequest) -> McpResponse {
    McpResponse::error(
        request.id.clone(),
        McpError::MethodNotFound("prompts/get not implemented".to_string()),
    )
}
