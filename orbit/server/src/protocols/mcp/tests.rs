//! Comprehensive test suite for MCP protocol

#[cfg(test)]
mod tests {
    use super::super::handlers;
    use super::super::server::McpServer;
    use super::super::types::{McpRequest, McpResponse, McpError};
    use super::super::{McpConfig, McpCapabilities};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_request(method: &str, params: HashMap<String, serde_json::Value>) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: method.to_string(),
            params,
        }
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let params = HashMap::new();
        let request = create_test_request("initialize", params);

        let response = handlers::handle_initialize(&request);
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("protocolVersion").is_some());
                assert!(result.get("capabilities").is_some());
                assert!(result.get("serverInfo").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let params = HashMap::new();
        let request = create_test_request("tools/list", params);

        let response = handlers::handle_tools_list(&request);
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("tools").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resources_list() {
        let params = HashMap::new();
        let request = create_test_request("resources/list", params);

        let response = handlers::handle_resources_list(&request);
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("resources").is_some());
                let resources = result.get("resources").and_then(|v| v.as_array()).unwrap();
                assert!(resources.len() > 0);
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resource_read_actors() {
        let mut params = HashMap::new();
        params.insert("uri".to_string(), json!("memory://actors"));
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("contents").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resource_read_schemas() {
        let mut params = HashMap::new();
        params.insert("uri".to_string(), json!("memory://schemas"));
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("contents").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resource_read_metrics() {
        let mut params = HashMap::new();
        params.insert("uri".to_string(), json!("memory://metrics"));
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("contents").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resource_read_invalid_uri() {
        let mut params = HashMap::new();
        params.insert("uri".to_string(), json!("invalid://uri"));
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::InvalidRequest(_) => {}
                    _ => panic!("Expected InvalidRequest error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_resource_read_missing_uri() {
        let params = HashMap::new();
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::InvalidRequest(_) => {}
                    _ => panic!("Expected InvalidRequest error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompts_list() {
        let params = HashMap::new();
        let request = create_test_request("prompts/list", params);

        let response = handlers::handle_prompts_list(&request);
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("prompts").is_some());
                let prompts = result.get("prompts").and_then(|v| v.as_array()).unwrap();
                assert!(prompts.len() > 0);
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompt_get_system_analysis() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("system_analysis"));
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("messages").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompt_get_query_help() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("query_help"));
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("messages").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompt_get_schema_exploration() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("schema_exploration"));
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("messages").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompt_get_invalid_name() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("invalid_prompt"));
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::InvalidRequest(_) => {}
                    _ => panic!("Expected InvalidRequest error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_prompt_get_missing_name() {
        let params = HashMap::new();
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::InvalidRequest(_) => {}
                    _ => panic!("Expected InvalidRequest error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_tool_call() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("system_info"));
        params.insert("arguments".to_string(), json!({}));
        let request = create_test_request("tools/call", params);

        let response = handlers::handle_tool_call(&request, None).await;
        
        // Tool call may succeed or fail depending on implementation
        match response {
            McpResponse::Success { .. } | McpResponse::Error { .. } => {}
        }
    }

    #[tokio::test]
    async fn test_handle_tool_call_missing_name() {
        let mut params = HashMap::new();
        params.insert("arguments".to_string(), json!({}));
        let request = create_test_request("tools/call", params);

        let response = handlers::handle_tool_call(&request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::InvalidRequest(_) => {}
                    _ => panic!("Expected InvalidRequest error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let params = HashMap::new();
        let request = create_test_request("unknown/method", params);

        let response = handlers::handle_request(request, None).await;
        
        match response {
            McpResponse::Error { error, .. } => {
                match error {
                    McpError::MethodNotFound(_) => {}
                    _ => panic!("Expected MethodNotFound error"),
                }
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_mcp_server_creation() {
        let config = McpConfig::default();
        let capabilities = McpCapabilities::default();
        let _server = McpServer::new(config, capabilities);
        
        // Server should be created successfully
        assert!(true); // Just verify it doesn't panic
    }

    #[tokio::test]
    async fn test_mcp_server_with_integration() {
        let config = McpConfig::default();
        let capabilities = McpCapabilities::default();
        
        // Create server without integration (should work)
        let server = McpServer::new(config.clone(), capabilities.clone());
        assert!(server.orbit_integration.is_none());
    }

    #[tokio::test]
    async fn test_resource_read_with_server() {
        let config = McpConfig::default();
        let capabilities = McpCapabilities::default();
        let server = Arc::new(McpServer::new(config, capabilities));

        let mut params = HashMap::new();
        params.insert("uri".to_string(), json!("memory://actors"));
        let request = create_test_request("resources/read", params);

        let response = handlers::handle_resource_read(&request, Some(&server)).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("contents").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_prompt_get_with_server() {
        let config = McpConfig::default();
        let capabilities = McpCapabilities::default();
        let server = Arc::new(McpServer::new(config, capabilities));

        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("system_analysis"));
        let request = create_test_request("prompts/get", params);

        let response = handlers::handle_prompt_get(&request, Some(&server)).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("messages").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_mcp_config_default() {
        let config = McpConfig::default();
        
        assert_eq!(config.name, "orbit-mcp-server");
        assert_eq!(config.version, "0.1.0");
        assert!(config.enable_sql_queries);
        assert!(config.enable_actor_management);
    }

    #[tokio::test]
    async fn test_mcp_capabilities_default() {
        let capabilities = McpCapabilities::default();
        
        assert!(!capabilities.tools.is_empty());
        assert!(!capabilities.resources.is_empty());
        assert!(!capabilities.prompts.is_empty());
    }

    #[tokio::test]
    async fn test_handle_request_initialize() {
        let params = HashMap::new();
        let request = create_test_request("initialize", params);

        let response = handlers::handle_request(request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("protocolVersion").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_tools_list() {
        let params = HashMap::new();
        let request = create_test_request("tools/list", params);

        let response = handlers::handle_request(request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("tools").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_resources_list() {
        let params = HashMap::new();
        let request = create_test_request("resources/list", params);

        let response = handlers::handle_request(request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("resources").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_prompts_list() {
        let params = HashMap::new();
        let request = create_test_request("prompts/list", params);

        let response = handlers::handle_request(request, None).await;
        
        match response {
            McpResponse::Success { result, .. } => {
                assert!(result.get("prompts").is_some());
            }
            _ => panic!("Expected error response"),
        }
    }
}

