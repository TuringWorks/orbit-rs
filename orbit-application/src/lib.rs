//! # Orbit Application Framework
//!
//! This crate provides application-level utilities, configuration management,
//! and lifecycle management for Orbit-RS applications.
//!
//! ## Features
//!
//! - **Application Lifecycle**: Startup, shutdown, and graceful termination
//! - **Configuration Management**: Environment-based configuration with validation
//! - **Service Management**: Service registration, discovery, and health checks
//! - **Event System**: Application-wide event broadcasting and handling
//! - **Plugin Architecture**: Dynamic plugin loading and management
//!
//! ## Example
//!
//! ```rust,no_run
//! use orbit_application::{Application, AppConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AppConfig::from_env()?;
//!     let mut app = Application::new(config).await?;
//!     
//!     app.start().await?;
//!     app.run().await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod error;
pub mod event;
pub mod health;
pub mod lifecycle;
pub mod plugin;
pub mod service;
pub mod telemetry;

pub use config::*;
pub use error::*;
pub use event::*;
pub use health::*;
pub use lifecycle::*;
pub use plugin::*;
pub use service::*;
pub use telemetry::*;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Main application instance that coordinates all components
#[derive(Debug)]
pub struct Application {
    config: AppConfig,
    lifecycle: LifecycleManager,
    service_registry: Arc<RwLock<ServiceRegistry>>,
    event_bus: Arc<EventBus>,
    plugin_manager: Arc<RwLock<PluginManager>>,
    health_checker: Arc<HealthChecker>,
    telemetry: Arc<TelemetryManager>,
    shutdown_token: tokio_util::sync::CancellationToken,
}

impl Application {
    /// Create a new application instance
    pub async fn new(config: AppConfig) -> Result<Self, AppError> {
        info!("Initializing Orbit application: {}", config.name);

        let shutdown_token = tokio_util::sync::CancellationToken::new();
        let event_bus = Arc::new(EventBus::new());
        let service_registry = Arc::new(RwLock::new(ServiceRegistry::new()));
        let plugin_manager = Arc::new(RwLock::new(PluginManager::new()));
        let health_checker = Arc::new(HealthChecker::new());
        let telemetry = Arc::new(TelemetryManager::new(config.telemetry.clone()).await?);
        let lifecycle = LifecycleManager::new(shutdown_token.clone(), event_bus.clone());

        Ok(Self {
            config,
            lifecycle,
            service_registry,
            event_bus,
            plugin_manager,
            health_checker,
            telemetry,
            shutdown_token,
        })
    }

    /// Start the application
    pub async fn start(&mut self) -> Result<(), AppError> {
        info!("Starting Orbit application: {}", self.config.name);

        // Initialize telemetry first
        self.telemetry.initialize().await?;

        // Load and initialize plugins
        if !self.config.plugins.is_empty() {
            let mut plugin_mgr = self.plugin_manager.write().await;
            for plugin_config in &self.config.plugins {
                plugin_mgr.load_plugin(plugin_config).await?;
            }
        }

        // Start lifecycle manager
        self.lifecycle.start().await?;

        // Register core services
        self.register_core_services().await?;

        // Emit application started event
        self.event_bus
            .publish(AppEvent::Started {
                app_name: self.config.name.clone(),
                version: self.config.version.clone(),
            })
            .await?;

        info!("✅ Orbit application started successfully");
        Ok(())
    }

    /// Run the application (blocks until shutdown)
    pub async fn run(&self) -> Result<(), AppError> {
        info!("Running Orbit application: {}", self.config.name);

        // Set up shutdown signal handling
        self.setup_shutdown_handlers();

        // Wait for shutdown signal
        self.shutdown_token.cancelled().await;

        info!("Shutdown signal received, stopping application...");
        self.shutdown().await?;

        Ok(())
    }

    /// Shutdown the application gracefully
    pub async fn shutdown(&self) -> Result<(), AppError> {
        info!("Shutting down Orbit application: {}", self.config.name);

        // Emit shutdown event
        self.event_bus
            .publish(AppEvent::Stopping {
                app_name: self.config.name.clone(),
            })
            .await?;

        // Stop services
        let service_registry = self.service_registry.read().await;
        service_registry.shutdown_all().await?;
        drop(service_registry);

        // Stop plugins
        let mut plugin_mgr = self.plugin_manager.write().await;
        plugin_mgr.shutdown_all().await?;
        drop(plugin_mgr);

        // Stop lifecycle manager
        self.lifecycle.stop().await?;

        // Shutdown telemetry
        self.telemetry.shutdown().await?;

        // Emit stopped event
        self.event_bus
            .publish(AppEvent::Stopped {
                app_name: self.config.name.clone(),
            })
            .await?;

        info!("✅ Orbit application stopped gracefully");
        Ok(())
    }

    /// Get application configuration
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get service registry
    pub fn services(&self) -> Arc<RwLock<ServiceRegistry>> {
        self.service_registry.clone()
    }

    /// Get event bus
    pub fn events(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }

    /// Get health checker
    pub fn health(&self) -> Arc<HealthChecker> {
        self.health_checker.clone()
    }

    /// Get telemetry manager
    pub fn telemetry(&self) -> Arc<TelemetryManager> {
        self.telemetry.clone()
    }

    /// Register a service
    pub async fn register_service(&self, service: Arc<dyn Service>) -> Result<(), AppError> {
        let mut registry = self.service_registry.write().await;
        registry.register(service).await
    }

    /// Get a service by name
    pub async fn get_service(&self, name: &str) -> Option<Arc<dyn Service>> {
        let registry = self.service_registry.read().await;
        registry.get(name)
    }

    async fn register_core_services(&self) -> Result<(), AppError> {
        // Register health check service
        let health_service = Arc::new(HealthService::new(
            self.health_checker.clone(),
            self.config.health.clone(),
        ));
        self.register_service(health_service).await?;

        // Register telemetry service
        let telemetry_service = Arc::new(TelemetryService::new(self.telemetry.clone()));
        self.register_service(telemetry_service).await?;

        Ok(())
    }

    fn setup_shutdown_handlers(&self) {
        let token = self.shutdown_token.clone();

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};

                let mut sigterm =
                    signal(SignalKind::terminate()).expect("Failed to listen for SIGTERM");
                let mut sigint =
                    signal(SignalKind::interrupt()).expect("Failed to listen for SIGINT");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, initiating shutdown");
                        token.cancel();
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, initiating shutdown");
                        token.cancel();
                    }
                }
            }

            #[cfg(not(unix))]
            {
                use tokio::signal;

                match signal::ctrl_c().await {
                    Ok(()) => {
                        info!("Received Ctrl+C, initiating shutdown");
                        token.cancel();
                    }
                    Err(err) => {
                        error!("Unable to listen for shutdown signal: {}", err);
                    }
                }
            }
        });
    }
}

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    Started { app_name: String, version: String },
    Stopping { app_name: String },
    Stopped { app_name: String },
    ServiceRegistered { service_name: String },
    ServiceUnregistered { service_name: String },
    HealthCheckFailed { service_name: String, error: String },
    ConfigReloaded { changes: Vec<String> },
}
