use crate::{ApplicationContext, SpringError, SpringResult};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// HTTP server for Java Spring Boot integration
pub struct SpringHttpServer {
    context: Arc<RwLock<ApplicationContext>>,
    config: HttpServerConfig,
    shutdown_token: tokio_util::sync::CancellationToken,
}

/// HTTP server configuration
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    pub host: String,
    pub port: u16,
    pub base_path: String,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8090,
            base_path: "/orbit".to_string(),
        }
    }
}

/// Application server state
#[derive(Clone)]
struct ServerState {
    context: Arc<RwLock<ApplicationContext>>,
}

impl SpringHttpServer {
    /// Create a new HTTP server
    pub fn new(context: Arc<RwLock<ApplicationContext>>, config: HttpServerConfig) -> Self {
        Self {
            context,
            config,
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Start the HTTP server
    pub async fn start(&self) -> SpringResult<()> {
        let bind_address = format!("{}:{}", self.config.host, self.config.port);
        info!("🚀 Starting Orbit Spring HTTP server on {}", bind_address);

        let state = ServerState {
            context: self.context.clone(),
        };

        let app = self.create_router(state);

        let listener = TcpListener::bind(&bind_address).await.map_err(|e| {
            SpringError::context(format!("Failed to bind to {}: {}", bind_address, e))
        })?;

        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_token.cancelled().await;
                    info!("📡 HTTP server shutdown requested");
                })
                .await
            {
                warn!("HTTP server error: {}", e);
            }
        });

        info!("✅ Orbit Spring HTTP server started successfully");
        Ok(())
    }

    /// Stop the HTTP server
    pub async fn stop(&self) -> SpringResult<()> {
        info!("🛑 Stopping Orbit Spring HTTP server");
        self.shutdown_token.cancel();
        Ok(())
    }

    /// Create the router with all endpoints
    fn create_router(&self, state: ServerState) -> Router {
        let base_path = &self.config.base_path;

        Router::new()
            // Service management endpoints
            .route(&format!("{}/services", base_path), get(list_services))
            .route(&format!("{}/services/:name", base_path), get(get_service))
            .route(
                &format!("{}/services/:name", base_path),
                post(register_service),
            )
            .route(
                &format!("{}/services/:name", base_path),
                delete(unregister_service),
            )
            // Configuration endpoints
            .route(&format!("{}/config", base_path), get(get_config))
            .route(&format!("{}/config/:key", base_path), get(get_config_value))
            .route(&format!("{}/config/:key", base_path), put(set_config_value))
            // Health and status endpoints
            .route(&format!("{}/health", base_path), get(health_check))
            .route(&format!("{}/status", base_path), get(status_check))
            .route(&format!("{}/metrics", base_path), get(get_metrics))
            // Context management endpoints (temporarily disabled due to lifetime issues)
            // .route(&format!("{}/context/start", base_path), post(start_context))
            // .route(&format!("{}/context/stop", base_path), post(stop_context))
            // .route(
            //     &format!("{}/context/refresh", base_path),
            //     post(refresh_context),
            // )
            // Root endpoint
            .route("/", get(root_handler))
            .with_state(state)
    }

    /// Get shutdown token
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown_token.clone()
    }
}

// ============================================================================
// HTTP Endpoint Handlers
// ============================================================================

/// Root endpoint handler
async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "Orbit Spring HTTP Bridge",
        "version": "1.0.0",
        "description": "HTTP API for Java Spring Boot integration with Orbit services",
        "endpoints": {
            "services": "/orbit/services",
            "config": "/orbit/config",
            "health": "/orbit/health",
            "status": "/orbit/status",
            "metrics": "/orbit/metrics"
        }
    }))
}

/// List all registered services
async fn list_services(
    State(state): State<ServerState>,
) -> Result<Json<ServiceListResponse>, SpringError> {
    let context = state.context.read().await;
    let service_names = context.get_service_names();
    let count = service_names.len();

    Ok(Json(ServiceListResponse {
        services: service_names,
        count,
    }))
}

/// Get service details
async fn get_service(
    State(state): State<ServerState>,
    Path(name): Path<String>,
) -> Result<Json<ServiceResponse>, SpringError> {
    let context = state.context.read().await;

    if context.contains_service(&name) {
        Ok(Json(ServiceResponse {
            name: name.clone(),
            status: "registered".to_string(),
            healthy: true, // Simplified for now
        }))
    } else {
        Err(SpringError::bean_not_found(name))
    }
}

/// Register a new service
async fn register_service(
    State(_state): State<ServerState>,
    Path(name): Path<String>,
    Json(request): Json<RegisterServiceRequest>,
) -> Result<Json<ServiceResponse>, SpringError> {
    info!("Registering service: {} with config: {:?}", name, request);

    // In a real implementation, we would register the service with the context
    // For now, we'll just return a success response

    Ok(Json(ServiceResponse {
        name,
        status: "registered".to_string(),
        healthy: true,
    }))
}

/// Unregister a service
async fn unregister_service(
    State(_state): State<ServerState>,
    Path(name): Path<String>,
) -> Result<Json<ServiceResponse>, SpringError> {
    info!("Unregistering service: {}", name);

    // In a real implementation, we would unregister the service from the context

    Ok(Json(ServiceResponse {
        name,
        status: "unregistered".to_string(),
        healthy: false,
    }))
}

/// Get configuration
async fn get_config(State(state): State<ServerState>) -> Result<Json<ConfigResponse>, SpringError> {
    let context = state.context.read().await;
    let config = context.config();

    // Convert config to a simple map for JSON serialization
    let mut config_map = HashMap::new();
    config_map.insert(
        "application.name".to_string(),
        config.application.name.clone(),
    );
    config_map.insert(
        "application.version".to_string(),
        config.application.version.clone(),
    );
    config_map.insert("server.port".to_string(), config.server.port.to_string());

    Ok(Json(ConfigResponse { config: config_map }))
}

/// Get specific configuration value
async fn get_config_value(
    State(state): State<ServerState>,
    Path(key): Path<String>,
) -> Result<Json<ConfigValueResponse>, SpringError> {
    let context = state.context.read().await;
    let config = context.config();

    let value = match key.as_str() {
        "application.name" => Some(config.application.name.clone()),
        "application.version" => Some(config.application.version.clone()),
        "server.port" => Some(config.server.port.to_string()),
        _ => None,
    };

    match value {
        Some(v) => Ok(Json(ConfigValueResponse { key, value: v })),
        None => Err(SpringError::configuration(format!(
            "Configuration key not found: {}",
            key
        ))),
    }
}

/// Set configuration value
async fn set_config_value(
    State(_state): State<ServerState>,
    Path(key): Path<String>,
    Json(request): Json<SetConfigRequest>,
) -> Result<Json<ConfigValueResponse>, SpringError> {
    info!("Setting config value: {} = {}", key, request.value);

    // In a real implementation, we would update the configuration

    Ok(Json(ConfigValueResponse {
        key,
        value: request.value,
    }))
}

/// Health check endpoint
async fn health_check(
    State(state): State<ServerState>,
) -> Result<Json<HealthResponse>, SpringError> {
    let context = state.context.read().await;
    let health_result = context.health_check().await;

    Ok(Json(HealthResponse {
        status: if health_result.healthy { "UP" } else { "DOWN" }.to_string(),
        services: health_result.healthy_services,
        unhealthy_services: health_result.unhealthy_services,
        timestamp: chrono::Utc::now(),
    }))
}

/// Status check endpoint
async fn status_check(
    State(state): State<ServerState>,
) -> Result<Json<StatusResponse>, SpringError> {
    let context = state.context.read().await;
    let running = context.is_running().await;
    let stats = context.container_stats();

    Ok(Json(StatusResponse {
        status: if running { "RUNNING" } else { "STOPPED" }.to_string(),
        uptime: "N/A".to_string(), // Would calculate actual uptime
        services: stats.total_beans,
        memory: "N/A".to_string(), // Would get actual memory usage
    }))
}

/// Get metrics endpoint
async fn get_metrics(
    State(_state): State<ServerState>,
) -> Result<Json<MetricsResponse>, SpringError> {
    // In a real implementation, we would collect actual metrics
    Ok(Json(MetricsResponse {
        requests_total: 100,
        requests_per_second: 5.0,
        memory_usage_mb: 128,
        cpu_usage_percent: 15.0,
    }))
}

/// Start context endpoint
#[allow(dead_code)]
async fn start_context(
    State(state): State<ServerState>,
) -> Result<Json<ContextResponse>, SpringError> {
    let context = state.context.read().await;
    context.start().await?;

    Ok(Json(ContextResponse {
        status: "started".to_string(),
        message: "Application context started successfully".to_string(),
    }))
}

/// Stop context endpoint
#[allow(dead_code)]
async fn stop_context(
    State(state): State<ServerState>,
) -> Result<Json<ContextResponse>, SpringError> {
    let context = state.context.read().await;
    context.stop().await?;

    Ok(Json(ContextResponse {
        status: "stopped".to_string(),
        message: "Application context stopped successfully".to_string(),
    }))
}

/// Refresh context endpoint
#[allow(dead_code)]
async fn refresh_context(
    State(state): State<ServerState>,
) -> Result<Json<ContextResponse>, SpringError> {
    let context = state.context.read().await;
    context.refresh().await?;

    Ok(Json(ContextResponse {
        status: "refreshed".to_string(),
        message: "Application context refreshed successfully".to_string(),
    }))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct ServiceListResponse {
    services: Vec<String>,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceResponse {
    name: String,
    status: String,
    healthy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegisterServiceRequest {
    service_type: String,
    address: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigResponse {
    config: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigValueResponse {
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SetConfigRequest {
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    services: Vec<String>,
    unhealthy_services: Vec<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusResponse {
    status: String,
    uptime: String,
    services: usize,
    memory: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MetricsResponse {
    requests_total: u64,
    requests_per_second: f64,
    memory_usage_mb: u64,
    cpu_usage_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContextResponse {
    status: String,
    message: String,
}

// ============================================================================
// Error Handling
// ============================================================================

/// Convert SpringError to HTTP response
impl IntoResponse for SpringError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            SpringError::BeanNotFound { .. } => (StatusCode::NOT_FOUND, self.to_string()),
            SpringError::Configuration { .. } => (StatusCode::BAD_REQUEST, self.to_string()),
            SpringError::Context { .. } => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let error_response = serde_json::json!({
            "error": message,
            "status": status.as_u16()
        });

        (status, Json(error_response)).into_response()
    }
}
