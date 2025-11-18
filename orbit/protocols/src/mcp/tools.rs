//! MCP tools implementation

use super::types::{McpTool, McpToolResult};
use std::collections::HashMap;

/// Get all available MCP tools
pub fn get_available_tools() -> Vec<McpTool> {
    vec![]
}

/// Execute an MCP tool
pub async fn execute_tool(
    tool_name: &str,
    _parameters: HashMap<String, serde_json::Value>,
) -> McpToolResult {
    McpToolResult::error(format!("Tool '{tool_name}' not yet implemented"))
}
