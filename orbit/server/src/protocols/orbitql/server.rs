/*! OrbitQL HTTP API server

This module implements a lightweight HTTP server for executing OrbitQL queries,
modeled after the AQL HTTP server pattern. It exposes a simple JSON API that
accepts a query string and returns result columns, rows, and basic execution
statistics.

Endpoints:
- POST /api/orbitql/query
  Body: {"query": "<orbitql query string>"}
  Response: {
  "columns": [...],
  "rows": [...],
  "rows_processed": <number>,
  "execution_time_ms": <number>,
  "index_hits": <number>,
  "gpu_acceleration_used": <bool>
  }

- GET /health
  Response: 200 OK with {"status": "ok"}

TLS:
If TLS is configured in the server config, the listener will accept TLS and
serve HTTP/1 over TLS using rustls. Mutual TLS (mTLS) support is controlled by
global TLS configuration (see crate::protocols::tls).

*/

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::orbitql::executor::OrbitQLExecutor;
use crate::protocols::orbitql::{Parser, Statement};
use crate::protocols::tls::OrbitTlsAcceptor;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::OnceCell;
use tracing::{error, info, warn};

/// HTTP server for OrbitQL protocol
pub struct OrbitQLServer {
    bind_addr: String,
    tls_acceptor: Option<OrbitTlsAcceptor>,
    executor: Arc<OrbitQLExecutor>,
}

impl OrbitQLServer {
    /// Create a new OrbitQL server that listens on the given address (host:port)
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            tls_acceptor: None,
            executor: Arc::new(OrbitQLExecutor::new()),
        }
    }

    /// Attach TLS configuration (if provided)
    pub fn with_tls_config(mut self, tls_config: Option<crate::config::TlsConfig>) -> Self {
        if tls_config.is_some() {
            self.tls_acceptor =
                Some(OrbitTlsAcceptor::new(&tls_config).expect("Invalid TLS configuration"));
        }
        self
    }

    /// Start the HTTP server and serve requests until failure
    pub async fn run(&self) -> ProtocolResult<()> {
        // Bind the TCP listener
        let listener = TcpListener::bind(&self.bind_addr).await.map_err(|e| {
            error!(
                "Failed to bind OrbitQL HTTP server to {}: {}",
                self.bind_addr, e
            );
            ProtocolError::Other(format!("Failed to bind OrbitQL HTTP server: {}", e))
        })?;

        info!("OrbitQL HTTP server listening on {}", self.bind_addr);

        // Clone shared state for the accept loop
        let tls_acceptor = self.tls_acceptor.clone();
        let executor = self.executor.clone();

        // Parser shared among connections (created lazily once)
        static PARSER: OnceCell<Arc<Parser>> = OnceCell::const_new();
        let _ = PARSER
            .get_or_init(|| async { Arc::new(Parser::new()) })
            .await;

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New OrbitQL HTTP connection from {}", addr);

                    let tls_acceptor = tls_acceptor.clone();
                    let executor = executor.clone();

                    tokio::spawn(async move {
                        // Wrap with TLS if configured
                        if let Some(acceptor) = tls_acceptor {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    let io = TokioIo::new(tls_stream);
                                    let service =
                                        service_fn(move |req: Request<hyper::body::Incoming>| {
                                            handle_request(req, executor.clone())
                                        });

                                    if let Err(err) =
                                        http1::Builder::new().serve_connection(io, service).await
                                    {
                                        error!(
                                            "Error serving OrbitQL TLS HTTP connection: {}",
                                            err
                                        );
                                    }
                                }
                                Err(e) => error!("OrbitQL TLS handshake failed: {}", e),
                            }
                        } else {
                            let io = TokioIo::new(stream);
                            let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                                handle_request(req, executor.clone())
                            });

                            if let Err(err) =
                                http1::Builder::new().serve_connection(io, service).await
                            {
                                error!("Error serving OrbitQL HTTP connection: {}", err);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting OrbitQL HTTP connection: {}", e);
                }
            }
        }
    }
}

/// Request payload for OrbitQL query execution
#[derive(Debug, Deserialize)]
struct OrbitQLQueryRequest {
    /// OrbitQL query string
    query: String,
}

/// Standard success response envelope
#[derive(Debug, Serialize)]
struct OrbitQLQueryResponse {
    columns: Vec<String>,
    rows: serde_json::Value,
    rows_processed: usize,
    execution_time_ms: u64,
    index_hits: usize,
    gpu_acceleration_used: bool,
}

/// Standard error response envelope
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

/// Handle a single HTTP request
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    executor: Arc<OrbitQLExecutor>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    match (method, path.as_str()) {
        (Method::GET, "/health") => Ok(json_response(
            StatusCode::OK,
            &serde_json::json!({ "status": "ok" }),
        )),

        (Method::POST, "/api/orbitql/query") => {
            // Collect request body
            let body_bytes = match req.collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => {
                    error!("Error reading OrbitQL request body: {}", e);
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid request body",
                    ));
                }
            };

            // Parse JSON
            let parsed: OrbitQLQueryRequest = match serde_json::from_slice(&body_bytes) {
                Ok(p) => p,
                Err(e) => {
                    warn!("OrbitQL invalid JSON payload: {}", e);
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON payload",
                    ));
                }
            };

            // Parse OrbitQL statement
            let lexer = crate::protocols::orbitql::Lexer::new();
            let tokens = match lexer.tokenize(&parsed.query) {
                Ok(t) => t,
                Err(e) => {
                    warn!("OrbitQL lex error: {:?}", e);
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("Lex error: {:?}", e),
                    ));
                }
            };
            let mut parser = Parser::new();
            let stmt: Statement = match parser.parse(tokens) {
                Ok(s) => s,
                Err(e) => {
                    warn!("OrbitQL parse error: {:?}", e);
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("Parse error: {:?}", e),
                    ));
                }
            };

            // Execute OrbitQL statement
            match executor.execute(stmt).await {
                Ok(result) => {
                    // Convert rows to JSON array of values (avoid serializing Row directly)
                    let rows_json = serde_json::Value::Array(
                        result
                            .rows
                            .iter()
                            .map(|row| {
                                // Serialize row values; fall back to null if any error
                                serde_json::to_value(&row.values).unwrap_or(serde_json::Value::Null)
                            })
                            .collect(),
                    );
                    // Extract column names
                    let column_names: Vec<String> =
                        result.columns.iter().map(|c| c.name.clone()).collect();

                    // Execution time in milliseconds
                    let exec_ms = result
                        .execution_time
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX);

                    let response = OrbitQLQueryResponse {
                        columns: column_names,
                        rows: rows_json,
                        rows_processed: result.rows_processed,
                        execution_time_ms: exec_ms,
                        index_hits: result.index_hits,
                        gpu_acceleration_used: result.gpu_acceleration_used,
                    };

                    Ok(json_response(StatusCode::OK, &response))
                }
                Err(e) => {
                    error!("OrbitQL execution error: {:?}", e);
                    Ok(error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Execution error: {:?}", e),
                    ))
                }
            }
        }

        // 404 for everything else
        _ => Ok(error_response(StatusCode::NOT_FOUND, "Not found")),
    }
}

/// Create a JSON response with the given status code and serializable payload
fn json_response<T: serde::Serialize>(status: StatusCode, value: &T) -> Response<Full<Bytes>> {
    let body = match serde_json::to_vec(value) {
        Ok(v) => v,
        Err(e) => {
            let fallback = serde_json::json!({"error": format!("Serialization error: {}", e)});
            serde_json::to_vec(&fallback).unwrap_or_else(|_| b"{}".to_vec())
        }
    };

    let mut res = Response::new(Full::new(Bytes::from(body)));
    *res.status_mut() = status;
    res
}

/// Create an error JSON response with the given status code and message
fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    json_response(
        status,
        &ErrorResponse {
            error: message.into(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        // Smoke test for JSON response helper
        let res = json_response(StatusCode::OK, &serde_json::json!({"status": "ok"}));
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_error_response() {
        let res = error_response(StatusCode::BAD_REQUEST, "oops");
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_query_request_deserialize() {
        let body = br#"{"query":"SELECT * FROM table"}"#;
        let parsed: OrbitQLQueryRequest = serde_json::from_slice(body).unwrap();
        assert_eq!(parsed.query, "SELECT * FROM table");
    }

    #[tokio::test]
    async fn test_query_response_serialize() {
        let response = OrbitQLQueryResponse {
            columns: vec!["id".into(), "name".into()],
            rows: serde_json::json!([[1, "a"], [2, "b"]]),
            rows_processed: 2,
            execution_time_ms: 10,
            index_hits: 0,
            gpu_acceleration_used: false,
        };
        let bytes = serde_json::to_vec(&response).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value.get("columns").is_some());
        assert!(value.get("rows").is_some());
    }
}
