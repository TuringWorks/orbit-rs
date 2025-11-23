//! ArangoDB HTTP API server for AQL protocol
//!
//! This module implements the ArangoDB HTTP API endpoints for AQL query execution
//! and document operations, providing compatibility with ArangoDB clients.

use crate::protocols::aql::query_engine::{AqlQueryEngine, AqlQueryResult};
use crate::protocols::aql::storage::AqlStorage;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// AQL cursor for streaming query results
#[derive(Debug, Clone)]
struct AqlCursor {
    id: String,
    query: String,
    result: AqlQueryResult,
    position: usize,
    has_more: bool,
}

    /// AQL query request
#[derive(Debug, Deserialize)]
struct AqlQueryRequest {
    query: String,
    #[serde(default)]
    bind_vars: HashMap<String, serde_json::Value>,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    count: bool,
    #[serde(default)]
    ttl: Option<u64>,
}

/// AQL cursor response
#[derive(Debug, Serialize)]
struct AqlCursorResponse {
    result: Vec<serde_json::Value>,
    has_more: bool,
    id: Option<String>,
    count: Option<usize>,
    cached: bool,
    extra: HashMap<String, serde_json::Value>,
}

/// ArangoDB HTTP API server
pub struct AqlHttpServer {
    bind_addr: String,
    storage: Arc<AqlStorage>,
    query_engine: Arc<AqlQueryEngine>,
    cursors: Arc<RwLock<HashMap<String, AqlCursor>>>,
}

impl AqlHttpServer {
    /// Create a new AQL HTTP server
    pub fn new(bind_addr: impl Into<String>, storage: Arc<AqlStorage>) -> Self {
        let query_engine = Arc::new(AqlQueryEngine::new());
        Self {
            bind_addr: bind_addr.into(),
            storage,
            query_engine,
            cursors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the HTTP server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await.map_err(|e| {
            error!("Failed to bind AQL HTTP server to {}: {}", self.bind_addr, e);
            ProtocolError::Other(format!("Failed to bind AQL HTTP server: {}", e))
        })?;

        info!("AQL/ArangoDB HTTP server listening on {}", self.bind_addr);

        let storage = self.storage.clone();
        let query_engine = self.query_engine.clone();
        let cursors = self.cursors.clone();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New AQL HTTP connection from {}", addr);
                    
                    let storage = storage.clone();
                    let query_engine = query_engine.clone();
                    let cursors = cursors.clone();

                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |req| {
                            handle_request(
                                req,
                                storage.clone(),
                                query_engine.clone(),
                                cursors.clone(),
                            )
                        });

                        if let Err(err) = http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            error!("Error serving connection: {}", err);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting AQL HTTP connection: {}", e);
                }
            }
        }
    }
}

/// Handle HTTP request
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    storage: Arc<AqlStorage>,
    query_engine: Arc<AqlQueryEngine>,
    cursors: Arc<RwLock<HashMap<String, AqlCursor>>>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    info!("AQL HTTP request: {} {}", method, path);

    // Parse request body if present
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            error!("Error reading request body: {}", e);
            return Ok(error_response(StatusCode::BAD_REQUEST, "Invalid request body"));
        }
    };

    // Route requests
    let response = if path.starts_with("/_api/cursor") {
        handle_cursor_request(&method, &path, body_bytes, query_engine, cursors).await
    } else if path.starts_with("/_api/collection") {
        handle_collection_request(&method, &path, body_bytes, storage).await
    } else if path.starts_with("/_api/document") {
        handle_document_request(&method, &path, body_bytes, storage).await
    } else if path == "/_api/version" {
        handle_version_request().await
    } else if path == "/_api/database" {
        handle_database_request(&method, &path, body_bytes).await
    } else {
        warn!("Unknown AQL endpoint: {}", path);
        error_response(StatusCode::NOT_FOUND, "Endpoint not found")
    };

    Ok(response)
}

/// Handle cursor (AQL query) requests
async fn handle_cursor_request(
    method: &Method,
    path: &str,
    body: Bytes,
    query_engine: Arc<AqlQueryEngine>,
    cursors: Arc<RwLock<HashMap<String, AqlCursor>>>,
) -> Response<Full<Bytes>> {
    match method {
        &Method::POST => {
            // Create new cursor (execute query)
            match serde_json::from_slice::<AqlQueryRequest>(&body) {
                Ok(query_req) => {
                    match query_engine.execute_query(&query_req.query).await {
                        Ok(result) => {
                            let cursor_id = format!("cursor_{}", uuid::Uuid::new_v4());
                            let cursor = AqlCursor {
                                id: cursor_id.clone(),
                                query: query_req.query.clone(),
                                result: result.clone(),
                                position: 0,
                                has_more: result.data.len() > query_req.batch_size.unwrap_or(100),
                            };

                            cursors.write().await.insert(cursor_id.clone(), cursor);

                            let batch_size = query_req.batch_size.unwrap_or(100);
                            let result_data: Vec<serde_json::Value> = result
                                .data
                                .iter()
                                .take(batch_size)
                                .map(|v| aql_value_to_json(v))
                                .collect();

                            let response = AqlCursorResponse {
                                result: result_data,
                                has_more: result.data.len() > batch_size,
                                id: Some(cursor_id),
                                count: if query_req.count {
                                    Some(result.data.len())
                                } else {
                                    None
                                },
                                cached: false,
                                extra: result.metadata
                                    .iter()
                                    .map(|(k, v)| (k.clone(), aql_value_to_json(v)))
                                    .collect(),
                            };

                            json_response(StatusCode::CREATED, &response)
                        }
                        Err(e) => {
                            error!("AQL query execution error: {}", e);
                            error_response(StatusCode::BAD_REQUEST, &format!("Query error: {}", e))
                        }
                    }
                }
                Err(e) => {
                    error!("Invalid AQL query request: {}", e);
                    error_response(StatusCode::BAD_REQUEST, "Invalid query request")
                }
            }
        }
        &Method::PUT => {
            // Get more results from cursor
            let cursor_id = path.strip_prefix("/_api/cursor/").unwrap_or("");
            if cursor_id.is_empty() {
                return error_response(StatusCode::BAD_REQUEST, "Missing cursor ID");
            }

            let mut cursors_guard = cursors.write().await;
            if let Some(cursor) = cursors_guard.get_mut(cursor_id) {
                let batch_size = 100; // Default batch size
                let start = cursor.position;
                let end = (start + batch_size).min(cursor.result.data.len());

                let result_data: Vec<serde_json::Value> = cursor
                    .result
                    .data
                    .iter()
                    .skip(start)
                    .take(batch_size)
                    .map(|v| aql_value_to_json(v))
                    .collect();

                cursor.position = end;
                cursor.has_more = end < cursor.result.data.len();

                let response = AqlCursorResponse {
                    result: result_data,
                    has_more: cursor.has_more,
                    id: Some(cursor_id.to_string()),
                    count: None,
                    cached: false,
                    extra: HashMap::new(),
                };

                json_response(StatusCode::OK, &response)
            } else {
                error_response(StatusCode::NOT_FOUND, "Cursor not found")
            }
        }
        &Method::DELETE => {
            // Delete cursor
            let cursor_id = path.strip_prefix("/_api/cursor/").unwrap_or("");
            if cursor_id.is_empty() {
                return error_response(StatusCode::BAD_REQUEST, "Missing cursor ID");
            }

            let mut cursors_guard = cursors.write().await;
            if cursors_guard.remove(cursor_id).is_some() {
                json_response(StatusCode::OK, &serde_json::json!({"error": false}))
            } else {
                error_response(StatusCode::NOT_FOUND, "Cursor not found")
            }
        }
        _ => error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    }
}

/// Handle collection requests
async fn handle_collection_request(
    method: &Method,
    path: &str,
    _body: Bytes,
    storage: Arc<AqlStorage>,
) -> Response<Full<Bytes>> {
    match method {
        &Method::GET => {
            // List collections
            // Simplified: return empty list for now
            let collections: Vec<serde_json::Value> = vec![];
            json_response(StatusCode::OK, &collections)
        }
        &Method::POST => {
            // Create collection
            // Simplified: return success
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "error": false,
                    "id": "collection_1",
                    "name": "new_collection"
                }),
            )
        }
        _ => error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    }
}

/// Handle document requests
async fn handle_document_request(
    method: &Method,
    path: &str,
    body: Bytes,
    storage: Arc<AqlStorage>,
) -> Response<Full<Bytes>> {
    // Parse collection and key from path: /_api/document/{collection}/{key}
    let parts: Vec<&str> = path.strip_prefix("/_api/document/").unwrap_or("").split('/').collect();
    
    match method {
        &Method::GET => {
            // Get document
            if parts.len() >= 2 {
                let collection = parts[0];
                let key = parts[1];
                // Simplified: return empty document
                json_response(StatusCode::OK, &serde_json::json!({"_key": key, "_id": format!("{}/{}", collection, key)}))
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid document path")
            }
        }
        &Method::POST => {
            // Create document
            if parts.len() >= 1 {
                let collection = parts[0];
                match serde_json::from_slice::<serde_json::Value>(&body) {
                    Ok(doc) => {
                        json_response(
                            StatusCode::CREATED,
                            &serde_json::json!({
                                "_id": format!("{}/{}", collection, "new_doc"),
                                "_key": "new_doc",
                                "_rev": "1"
                            }),
                        )
                    }
                    Err(e) => {
                        error!("Invalid document JSON: {}", e);
                        error_response(StatusCode::BAD_REQUEST, "Invalid JSON")
                    }
                }
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid collection path")
            }
        }
        &Method::PUT => {
            // Update document
            if parts.len() >= 2 {
                json_response(StatusCode::OK, &serde_json::json!({"error": false}))
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid document path")
            }
        }
        &Method::DELETE => {
            // Delete document
            if parts.len() >= 2 {
                json_response(StatusCode::OK, &serde_json::json!({"error": false}))
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid document path")
            }
        }
        _ => error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    }
}

/// Handle version request
async fn handle_version_request() -> Response<Full<Bytes>> {
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "server": "orbit-rs",
            "version": "1.0.0",
            "license": "Apache-2.0"
        }),
    )
}

/// Handle database requests
async fn handle_database_request(
    method: &Method,
    _path: &str,
    _body: Bytes,
) -> Response<Full<Bytes>> {
    match method {
        &Method::GET => {
            // List databases
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "result": ["_system"],
                    "error": false
                }),
            )
        }
        _ => error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    }
}

/// Convert AQL value to JSON value
fn aql_value_to_json(value: &crate::protocols::aql::data_model::AqlValue) -> serde_json::Value {
    use crate::protocols::aql::data_model::AqlValue;
    match value {
        AqlValue::Null => serde_json::Value::Null,
        AqlValue::Bool(b) => serde_json::Value::Bool(*b),
        AqlValue::Number(n) => serde_json::Value::Number(n.clone()),
        AqlValue::String(s) => serde_json::Value::String(s.clone()),
        AqlValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(aql_value_to_json).collect())
        }
        AqlValue::Object(obj) => {
            serde_json::Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), aql_value_to_json(v)))
                    .collect(),
            )
        }
        AqlValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
    }
}

/// Create JSON response
fn json_response(status: StatusCode, data: &impl Serialize) -> Response<Full<Bytes>> {
    match serde_json::to_string(data) {
        Ok(json) => Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from("Internal server error")))
                    .unwrap()
            }),
        Err(e) => {
            error!("JSON serialization error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Serialization error")
        }
    }
}

/// Create error response
fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    let error_json = serde_json::json!({
        "error": true,
        "errorMessage": message,
        "code": status.as_u16(),
        "errorNum": status.as_u16()
    });

    json_response(status, &error_json)
}

