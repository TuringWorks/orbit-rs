use crate::{MetricsFormatter, OrbitMetrics, PrometheusConfig, PrometheusError, PrometheusResult};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};

/// Prometheus HTTP server state
#[derive(Clone)]
pub struct ServerState {
    pub metrics: Arc<OrbitMetrics>,
    pub config: PrometheusConfig,
}

/// Prometheus HTTP server
pub struct PrometheusServer {
    state: ServerState,
    shutdown_token: tokio_util::sync::CancellationToken,
}

impl PrometheusServer {
    /// Create a new Prometheus server
    pub fn new(metrics: Arc<OrbitMetrics>, config: PrometheusConfig) -> Self {
        let state = ServerState { metrics, config };

        Self {
            state,
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Start the HTTP server
    pub async fn start(&self) -> PrometheusResult<()> {
        let bind_address = self.state.config.bind_address();
        info!("🚀 Starting Prometheus HTTP server on {}", bind_address);

        let app = self.create_router();

        let listener = TcpListener::bind(&bind_address).await.map_err(|e| {
            PrometheusError::server_startup(format!("Failed to bind to {}: {}", bind_address, e))
        })?;

        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_token.cancelled().await;
                    info!("📡 Prometheus server shutdown requested");
                })
                .await
            {
                warn!("Prometheus server error: {}", e);
            }
        });

        info!("✅ Prometheus HTTP server started successfully");
        Ok(())
    }

    /// Stop the HTTP server
    pub async fn stop(&self) -> PrometheusResult<()> {
        info!("🛑 Stopping Prometheus HTTP server");
        self.shutdown_token.cancel();
        Ok(())
    }

    /// Create the router with all endpoints
    fn create_router(&self) -> Router {
        Router::new()
            .route(&self.state.config.export.metrics_path, get(metrics_handler))
            .route(&self.state.config.export.health_path, get(health_handler))
            .route(
                &self.state.config.export.dashboard_path,
                get(dashboard_handler),
            )
            .route("/", get(root_handler))
            .with_state(self.state.clone())
    }

    /// Get shutdown token
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown_token.clone()
    }
}

/// Root endpoint handler
async fn root_handler(State(state): State<ServerState>) -> impl IntoResponse {
    let info = format!(
        "Orbit Prometheus Exporter\\n\\nEndpoints:\\n- {} (metrics)\\n- {} (health)\\n- {} (dashboard)",
        state.config.export.metrics_path,
        state.config.export.health_path,
        state.config.export.dashboard_path
    );

    (StatusCode::OK, info)
}

/// Metrics endpoint handler
async fn metrics_handler(State(state): State<ServerState>) -> Result<Response, PrometheusError> {
    match state.config.export.format {
        crate::ExportFormat::Prometheus => {
            let formatted = MetricsFormatter::format_prometheus(&state.metrics)?;
            Ok((
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                formatted,
            )
                .into_response())
        }
        crate::ExportFormat::Json => {
            let formatted = MetricsFormatter::format_json(&state.metrics)?;
            Ok((
                StatusCode::OK,
                [("content-type", "application/json")],
                formatted,
            )
                .into_response())
        }
        crate::ExportFormat::OpenMetrics => {
            // For now, fallback to Prometheus format
            let formatted = MetricsFormatter::format_prometheus(&state.metrics)?;
            Ok((
                StatusCode::OK,
                [(
                    "content-type",
                    "application/openmetrics-text; version=1.0.0; charset=utf-8",
                )],
                formatted,
            )
                .into_response())
        }
    }
}

/// Health check endpoint handler
async fn health_handler(State(state): State<ServerState>) -> impl IntoResponse {
    let summary = match state.metrics.get_summary() {
        Ok(summary) => summary,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Health check failed").into_response()
        }
    };

    let health_info = serde_json::json!({
        "status": "UP",
        "metrics": {
            "total": summary.total_metrics,
            "custom": summary.custom_metrics_count,
            "last_updated": summary.last_updated
        },
        "timestamp": chrono::Utc::now()
    });

    (
        StatusCode::OK,
        [("content-type", "application/json")],
        health_info.to_string(),
    )
        .into_response()
}

/// Dashboard endpoint handler
async fn dashboard_handler(State(_state): State<ServerState>) -> impl IntoResponse {
    // Simple HTML dashboard
    let dashboard_html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Orbit Metrics Dashboard</title>
    <meta charset="utf-8">
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { background: #2196F3; color: white; padding: 20px; margin-bottom: 20px; }
        .metric { margin: 10px 0; padding: 10px; background: #f5f5f5; }
        .status { color: #4CAF50; font-weight: bold; }
    </style>
</head>
<body>
    <div class="header">
        <h1>🚀 Orbit Prometheus Dashboard</h1>
        <p>Real-time metrics monitoring</p>
    </div>
    
    <div class="metric">
        <h3>📊 Metrics Status</h3>
        <p class="status">✅ OPERATIONAL</p>
        <p>Metrics collection is running normally</p>
    </div>
    
    <div class="metric">
        <h3>🔗 Quick Links</h3>
        <ul>
            <li><a href="/metrics">📈 Prometheus Metrics</a></li>
            <li><a href="/health">💚 Health Check</a></li>
        </ul>
    </div>
    
    <div class="metric">
        <h3>ℹ️ Information</h3>
        <p>This is a simplified dashboard. In production, you would integrate with Grafana or similar tools.</p>
    </div>
</body>
</html>
    "#;

    (
        StatusCode::OK,
        [("content-type", "text/html")],
        dashboard_html,
    )
}

/// Convert PrometheusError to HTTP response
impl IntoResponse for PrometheusError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            PrometheusError::Configuration { .. } => (StatusCode::BAD_REQUEST, self.to_string()),
            PrometheusError::Collection { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            PrometheusError::Network { .. } => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        (status, message).into_response()
    }
}
