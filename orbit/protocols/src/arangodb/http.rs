//! ArangoDB HTTP Protocol Implementation
//!
//! Implements the ArangoDB HTTP API for AQL query execution.
//! References: https://www.arangodb.com/docs/stable/http/aql-query-cursor-accessing-cursors.html

use crate::arangodb::aql_spatial::{AQLSpatialParser, AQLSpatialResult};
use crate::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// ArangoDB HTTP Protocol Handler
pub struct ArangoHttpProtocol {
    parser: Arc<AQLSpatialParser>,
}

#[derive(Debug, Deserialize)]
pub struct CursorCreateRequest {
    pub query: String,
    #[serde(default)]
    pub bind_vars: Option<Value>,
    #[serde(default)]
    pub count: bool,
    #[serde(default)]
    pub batch_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CursorResponse {
    pub result: Vec<Value>,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    pub error: bool,
    pub code: u16,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: bool,
    #[serde(rename = "errorNum")]
    pub error_num: u32,
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    pub code: u16,
}

impl ArangoHttpProtocol {
    pub fn new() -> Self {
        Self {
            parser: Arc::new(AQLSpatialParser::new()),
        }
    }

    /// Handle POST /_api/cursor request (Execute AQL)
    pub async fn handle_cursor_request(&self, body: &str) -> ProtocolResult<String> {
        let request: CursorCreateRequest = serde_json::from_str(body)
            .map_err(|e| ProtocolError::DecodingError(format!("Invalid JSON: {}", e)))?;

        // Execute query using AQL parser
        // For MVP, we only support basic spatial queries handled by AQLSpatialParser
        let result = self
            .parser
            .execute_query(&request.query)
            .await
            .map_err(|e| ProtocolError::ExecutionError(e.to_string()))?;

        // Convert AQL result to JSON response
        let json_result = self.convert_result_to_json(result);

        let response = CursorResponse {
            result: vec![json_result], // Wrap in array as ArangoDB returns list of results
            has_more: false,           // No pagination for MVP
            count: if request.count { Some(1) } else { None },
            error: false,
            code: 201,
        };

        serde_json::to_string(&response).map_err(|e| {
            ProtocolError::EncodingError(format!("Failed to serialize response: {}", e))
        })
    }

    fn convert_result_to_json(&self, result: AQLSpatialResult) -> Value {
        match result {
            AQLSpatialResult::Boolean(b) => Value::Bool(b),
            AQLSpatialResult::Number(n) => serde_json::json!(n),
            AQLSpatialResult::String(s) => Value::String(s),
            AQLSpatialResult::Array(arr) => Value::Array(
                arr.into_iter()
                    .map(|item| self.convert_result_to_json(item))
                    .collect(),
            ),
            AQLSpatialResult::Object(obj) => Value::Object(obj),
            AQLSpatialResult::Geometry(geom) => serde_json::json!(geom), // Assuming SpatialGeometry implements Serialize
            AQLSpatialResult::Documents(docs) => {
                // Convert documents to JSON
                let doc_values: Vec<Value> = docs
                    .into_iter()
                    .map(|doc| {
                        let mut map = doc.data;
                        map.insert("_id".to_string(), Value::String(doc.id));
                        Value::Object(map)
                    })
                    .collect();
                Value::Array(doc_values)
            }
            AQLSpatialResult::Null => Value::Null,
        }
    }
}

impl Default for ArangoHttpProtocol {
    fn default() -> Self {
        Self::new()
    }
}
