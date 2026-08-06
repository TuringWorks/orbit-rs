//! REST API server implementation

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use orbit_client::OrbitClient;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use super::{
    handlers::{self, ApiState},
    websocket::WebSocketHandler,
};
use crate::protocols::error::{ProtocolError, ProtocolResult};

/// REST API server configuration
#[derive(Debug, Clone)]
pub struct RestApiConfig {
    /// Server bind address
    pub bind_address: String,

    /// Enable CORS (Cross-Origin Resource Sharing)
    pub enable_cors: bool,

    /// Enable request tracing
    pub enable_tracing: bool,

    /// API version prefix (default: "/api/v1")
    pub api_prefix: String,
}

impl Default for RestApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".to_string(),
            enable_cors: true,
            enable_tracing: true,
            api_prefix: "/api/v1".to_string(),
        }
    }
}

/// REST API server
pub struct RestApiServer {
    config: RestApiConfig,
    orbit_client: Arc<OrbitClient>,
    ws_handler: Arc<WebSocketHandler>,
    /// Optional MCP server for natural language queries
    mcp_server: Option<Arc<crate::protocols::mcp::server::McpServer>>,
    /// SQL engine backing `/sql`, `/tables` and `/schemas`.
    ///
    /// Without one those endpoints report that SQL is unavailable rather than
    /// answering with example data.
    query_engine: Option<Arc<crate::protocols::postgres_wire::QueryEngine>>,
}

impl RestApiServer {
    /// Create a new REST API server
    pub fn new(orbit_client: OrbitClient, config: RestApiConfig) -> Self {
        Self {
            config,
            orbit_client: Arc::new(orbit_client),
            ws_handler: Arc::new(WebSocketHandler::new()),
            mcp_server: None,
            query_engine: None,
        }
    }

    /// Attach the SQL engine that `/sql` and the catalogue endpoints will use.
    ///
    /// Share the engine's storage with the other protocols so a table created
    /// over PostgreSQL is visible here.
    #[must_use]
    pub fn with_query_engine(
        mut self,
        query_engine: Arc<crate::protocols::postgres_wire::QueryEngine>,
    ) -> Self {
        self.query_engine = Some(query_engine);
        self
    }

    /// Create a new REST API server with MCP support
    pub fn with_mcp(
        orbit_client: OrbitClient,
        config: RestApiConfig,
        mcp_server: Arc<crate::protocols::mcp::server::McpServer>,
    ) -> Self {
        Self {
            config,
            orbit_client: Arc::new(orbit_client),
            ws_handler: Arc::new(WebSocketHandler::new()),
            mcp_server: Some(mcp_server),
            query_engine: None,
        }
    }

    /// Create server with default configuration
    pub fn with_defaults(orbit_client: OrbitClient) -> Self {
        Self::new(orbit_client, RestApiConfig::default())
    }

    /// Build the axum router
    fn build_router(&self) -> Router {
        let state = ApiState {
            orbit_client: self.orbit_client.clone(),
            mcp_server: self.mcp_server.clone(),
            query_engine: self.query_engine.clone(),
        };

        // API v1 routes
        // Note: Axum 0.7+ uses {param} syntax instead of :param
        let api_routes = Router::new()
            // Actor management
            .route("/actors", get(handlers::list_actors))
            .route("/actors", post(handlers::create_actor))
            .route("/actors/{actor_type}/{key}", get(handlers::get_actor))
            .route("/actors/{actor_type}/{key}", put(handlers::update_actor))
            .route("/actors/{actor_type}/{key}", delete(handlers::delete_actor))
            .route(
                "/actors/{actor_type}/{key}/invoke",
                post(handlers::invoke_actor),
            )
            // SQL Query endpoints
            .route("/sql", post(handlers::execute_sql_query))
            .route("/sql/batch", post(handlers::execute_batch_sql))
            .route("/tables", get(handlers::list_tables))
            .route("/tables/{schema}/{table}", get(handlers::describe_table))
            .route(
                "/tables/{schema}/{table}/indexes",
                get(handlers::list_indexes),
            )
            .route("/schemas", get(handlers::list_schemas))
            .route("/queries/history", get(handlers::get_query_history))
            .route("/stats", get(handlers::get_database_stats))
            .route("/config", get(handlers::get_server_config))
            // Cluster management
            .route("/cluster/nodes", get(handlers::list_cluster_nodes))
            .route("/cluster/status", get(handlers::get_cluster_status))
            // Transactions
            .route("/transactions", post(handlers::begin_transaction))
            .route(
                "/transactions/{transaction_id}/commit",
                post(handlers::commit_transaction),
            )
            .route(
                "/transactions/{transaction_id}/abort",
                post(handlers::abort_transaction),
            )
            // Natural Language Queries (disabled - handlers not implemented)
            // .route(
            //     "/query/natural-language",
            //     post(handlers::natural_language_query),
            // )
            // .route(
            //     "/query/generate-sql",
            //     post(handlers::generate_sql_from_natural_language),
            // )
            // WebSocket endpoints
            .route(
                "/ws/actors/{actor_type}/{key}",
                get(WebSocketHandler::handle_actor_socket),
            )
            .route("/ws/events", get(WebSocketHandler::handle_events_socket))
            .with_state(state);

        // Root router with versioned API
        let mut app = Router::new()
            .route("/health", get(handlers::health_check))
            .route("/openapi.json", get(handlers::openapi_spec))
            .nest(&self.config.api_prefix, api_routes);

        // Add CORS layer
        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            app = app.layer(cors);
        }

        // Add tracing layer
        if self.config.enable_tracing {
            app = app.layer(TraceLayer::new_for_http());
        }

        app
    }

    /// Start the REST API server
    pub async fn run(self) -> ProtocolResult<()> {
        let addr: SocketAddr = self
            .config
            .bind_address
            .parse()
            .map_err(|e| ProtocolError::RestError(format!("Invalid bind address: {}", e)))?;

        let router = self.build_router();

        info!("Starting REST API server on {}", addr);
        info!(
            "OpenAPI documentation available at http://{}/openapi.json",
            addr
        );
        info!("Health check available at http://{}/health", addr);
        if self.mcp_server.is_some() {
            info!("Natural language query endpoints enabled:");
            info!("  POST http://{}/api/v1/query/natural-language", addr);
            info!("  POST http://{}/api/v1/query/generate-sql", addr);
        }

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ProtocolError::RestError(format!("Failed to bind: {}", e)))?;

        axum::serve(listener, router)
            .await
            .map_err(|e| ProtocolError::RestError(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Get a reference to the WebSocket handler for event broadcasting
    pub fn ws_handler(&self) -> Arc<WebSocketHandler> {
        self.ws_handler.clone()
    }
}

/// Builder for REST API server
pub struct RestApiServerBuilder {
    config: RestApiConfig,
}

impl RestApiServerBuilder {
    pub fn new() -> Self {
        Self {
            config: RestApiConfig::default(),
        }
    }

    pub fn bind_address(mut self, address: impl Into<String>) -> Self {
        self.config.bind_address = address.into();
        self
    }

    pub fn enable_cors(mut self, enable: bool) -> Self {
        self.config.enable_cors = enable;
        self
    }

    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.config.enable_tracing = enable;
        self
    }

    pub fn api_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.api_prefix = prefix.into();
        self
    }

    pub fn build(self, orbit_client: OrbitClient) -> RestApiServer {
        RestApiServer::new(orbit_client, self.config)
    }
}

impl Default for RestApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RestApiConfig::default();
        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
        assert_eq!(config.api_prefix, "/api/v1");
    }

    #[test]
    fn test_builder() {
        let builder = RestApiServerBuilder::new()
            .bind_address("127.0.0.1:3000")
            .enable_cors(false)
            .api_prefix("/v2");

        assert_eq!(builder.config.bind_address, "127.0.0.1:3000");
        assert!(!builder.config.enable_cors);
        assert_eq!(builder.config.api_prefix, "/v2");
    }
}
