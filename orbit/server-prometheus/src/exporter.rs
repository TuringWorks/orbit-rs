use crate::{
    collector::MetricsCollector, metrics::OrbitMetrics, server::PrometheusServer, PrometheusConfig,
    PrometheusResult,
};
use std::sync::Arc;
use tracing::{error, info};

/// Main Prometheus exporter that coordinates all components
pub struct PrometheusExporter {
    config: PrometheusConfig,
    metrics: Arc<OrbitMetrics>,
    server: Option<PrometheusServer>,
    collector: Option<MetricsCollector>,
    shutdown_token: tokio_util::sync::CancellationToken,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub async fn new(config: PrometheusConfig) -> PrometheusResult<Self> {
        // Validate configuration
        config.validate()?;

        // Initialize metrics
        let metrics = Arc::new(OrbitMetrics::new());

        info!("🚀 Initializing Prometheus exporter");
        info!("📊 Metrics endpoint: {}", config.metrics_url());
        info!("💚 Health endpoint: {}", config.health_url());

        Ok(Self {
            config,
            metrics,
            server: None,
            collector: None,
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        })
    }

    /// Create exporter with default configuration
    pub async fn with_defaults() -> PrometheusResult<Self> {
        let config = PrometheusConfig::default();
        Self::new(config).await
    }

    /// Start the Prometheus exporter
    pub async fn start(&mut self) -> PrometheusResult<()> {
        info!("🚀 Starting Prometheus exporter");

        // Start metrics collector
        if self.config.metrics.collect_system_metrics
            || self.config.metrics.collect_app_metrics
            || self.config.metrics.collect_custom_metrics
        {
            let collector = MetricsCollector::new(self.metrics.clone(), self.config.clone());
            collector.start().await?;
            self.collector = Some(collector);
            info!("📊 Metrics collector started");
        }

        // Start HTTP server
        let server = PrometheusServer::new(self.metrics.clone(), self.config.clone());
        server.start().await?;
        self.server = Some(server);
        info!("🌐 HTTP server started on {}", self.config.bind_address());

        info!("✅ Prometheus exporter started successfully");
        Ok(())
    }

    /// Stop the Prometheus exporter
    pub async fn stop(&self) -> PrometheusResult<()> {
        info!("🛑 Stopping Prometheus exporter");

        // Signal shutdown
        self.shutdown_token.cancel();

        // Stop server
        if let Some(server) = &self.server {
            server.stop().await?;
        }

        // Stop collector
        if let Some(collector) = &self.collector {
            collector.stop().await?;
        }

        info!("✅ Prometheus exporter stopped gracefully");
        Ok(())
    }

    /// Run the exporter (blocks until shutdown)
    pub async fn run(&self) -> PrometheusResult<()> {
        info!("🎯 Prometheus exporter is running");

        // Setup signal handlers
        self.setup_signal_handlers();

        // Wait for shutdown
        self.shutdown_token.cancelled().await;

        info!("📡 Shutdown signal received");
        self.stop().await?;

        Ok(())
    }

    /// Setup signal handlers for graceful shutdown
    fn setup_signal_handlers(&self) {
        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};

                let mut sigterm =
                    signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
                let mut sigint =
                    signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("📡 Received SIGTERM");
                        shutdown_token.cancel();
                    }
                    _ = sigint.recv() => {
                        info!("📡 Received SIGINT (Ctrl+C)");
                        shutdown_token.cancel();
                    }
                }
            }

            #[cfg(windows)]
            {
                use tokio::signal;

                if let Err(e) = signal::ctrl_c().await {
                    error!("Failed to listen for Ctrl+C: {}", e);
                    return;
                }
                info!("📡 Received Ctrl+C");
                shutdown_token.cancel();
            }
        });
    }

    /// Get reference to metrics for external use
    pub fn metrics(&self) -> &Arc<OrbitMetrics> {
        &self.metrics
    }

    /// Get reference to configuration
    pub fn config(&self) -> &PrometheusConfig {
        &self.config
    }

    /// Get shutdown token
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown_token.clone()
    }

    /// Check if the exporter is running
    pub fn is_running(&self) -> bool {
        !self.shutdown_token.is_cancelled()
    }

    /// Get exporter status
    pub fn status(&self) -> ExporterStatus {
        ExporterStatus {
            running: self.is_running(),
            server_running: self.server.is_some(),
            collector_running: self.collector.is_some(),
            metrics_count: self
                .metrics
                .get_summary()
                .map(|s| s.total_metrics)
                .unwrap_or(0),
            bind_address: self.config.bind_address(),
        }
    }

    /// Perform health check
    pub async fn health_check(&self) -> PrometheusResult<HealthCheckResult> {
        let metrics_summary = self.metrics.get_summary()?;

        Ok(HealthCheckResult {
            status: if self.is_running() {
                "UP".to_string()
            } else {
                "DOWN".to_string()
            },
            metrics_count: metrics_summary.total_metrics,
            last_updated: metrics_summary.last_updated,
            endpoints: vec![self.config.metrics_url(), self.config.health_url()],
        })
    }
}

/// Exporter status information
#[derive(Debug, Clone)]
pub struct ExporterStatus {
    pub running: bool,
    pub server_running: bool,
    pub collector_running: bool,
    pub metrics_count: usize,
    pub bind_address: String,
}

/// Health check result
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheckResult {
    pub status: String,
    pub metrics_count: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub endpoints: Vec<String>,
}

/// Builder for creating Prometheus exporters
pub struct PrometheusExporterBuilder {
    config: PrometheusConfig,
}

impl PrometheusExporterBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: PrometheusConfig::default(),
        }
    }

    /// Set the server host
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.server.host = host.into();
        self
    }

    /// Set the server port
    pub fn port(mut self, port: u16) -> Self {
        self.config.server.port = port;
        self
    }

    /// Set the metrics path
    pub fn metrics_path(mut self, path: impl Into<String>) -> Self {
        self.config.export.metrics_path = path.into();
        self
    }

    /// Enable/disable system metrics collection
    pub fn collect_system_metrics(mut self, enabled: bool) -> Self {
        self.config.metrics.collect_system_metrics = enabled;
        self
    }

    /// Enable/disable dashboard
    pub fn enable_dashboard(mut self, enabled: bool) -> Self {
        self.config.dashboard.enabled = enabled;
        self
    }

    /// Set collection interval
    pub fn collection_interval(mut self, interval: std::time::Duration) -> Self {
        self.config.metrics.collection_interval = interval;
        self
    }

    /// Build the exporter
    pub async fn build(self) -> PrometheusResult<PrometheusExporter> {
        PrometheusExporter::new(self.config).await
    }
}

impl Default for PrometheusExporterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
