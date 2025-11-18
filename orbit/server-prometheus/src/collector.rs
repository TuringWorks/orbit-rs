use crate::{metrics::OrbitMetrics, PrometheusConfig, PrometheusResult};
use std::sync::Arc;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Metrics collector that periodically gathers metrics
pub struct MetricsCollector {
    metrics: Arc<OrbitMetrics>,
    config: PrometheusConfig,
    shutdown_token: tokio_util::sync::CancellationToken,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(metrics: Arc<OrbitMetrics>, config: PrometheusConfig) -> Self {
        Self {
            metrics,
            config,
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Start the metrics collection loop
    pub async fn start(&self) -> PrometheusResult<()> {
        info!("🚀 Starting Prometheus metrics collector");

        let mut collection_interval = interval(self.config.metrics.collection_interval);
        let metrics = self.metrics.clone();
        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = collection_interval.tick() => {
                        if let Err(e) = Self::collect_metrics(&metrics).await {
                            warn!("Failed to collect metrics: {}", e);
                        }
                    }
                    _ = shutdown_token.cancelled() => {
                        info!("📡 Metrics collector shutdown requested");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the metrics collector
    pub async fn stop(&self) -> PrometheusResult<()> {
        info!("🛑 Stopping Prometheus metrics collector");
        self.shutdown_token.cancel();
        Ok(())
    }

    /// Collect all metrics
    async fn collect_metrics(metrics: &Arc<OrbitMetrics>) -> PrometheusResult<()> {
        debug!("📊 Collecting metrics");

        // Simulate collecting various metrics
        Self::collect_system_metrics(metrics).await?;
        Self::collect_application_metrics(metrics).await?;

        debug!("✅ Metrics collection completed");
        Ok(())
    }

    /// Collect system-level metrics
    async fn collect_system_metrics(metrics: &Arc<OrbitMetrics>) -> PrometheusResult<()> {
        // Simulate memory usage collection
        metrics.update_memory_usage(1024 * 1024 * 256); // 256MB

        // Simulate CPU usage collection
        metrics.update_cpu_usage(15.5); // 15.5%

        // Simulate connection count
        metrics.update_connections(42);

        Ok(())
    }

    /// Collect application-specific metrics
    async fn collect_application_metrics(metrics: &Arc<OrbitMetrics>) -> PrometheusResult<()> {
        // Simulate database connections
        metrics.update_db_connections(8, 2); // 8 active, 2 idle

        // Simulate cache metrics
        metrics.record_cache_hit("redis");
        metrics.update_cache_size("redis", 1024 * 1024 * 10); // 10MB

        Ok(())
    }

    /// Get shutdown token for external coordination
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown_token.clone()
    }
}
