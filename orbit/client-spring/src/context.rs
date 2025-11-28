use crate::{
    annotations::{Component, Service},
    config::AppConfig,
    container::{Container, ContainerConfig},
    SpringError, SpringResult,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Spring-like application context that manages the application lifecycle
/// and provides dependency injection capabilities
#[derive(Debug)]
pub struct ApplicationContext {
    /// Dependency injection container
    container: Container,

    /// Application configuration
    config: AppConfig,

    /// Context state
    state: Arc<RwLock<ContextState>>,

    /// Shutdown signal
    shutdown_token: tokio_util::sync::CancellationToken,
}

/// Application context state
#[derive(Debug, Clone, PartialEq)]
pub enum ContextState {
    Initializing,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

impl ApplicationContext {
    /// Create a new application context
    pub fn new() -> Self {
        Self::with_config(AppConfig::default())
    }

    /// Create a new application context with configuration
    pub fn with_config(config: AppConfig) -> Self {
        let container_config = ContainerConfig {
            allow_circular_dependencies: false,
            lazy_initialization: false,
            auto_wire_candidates: true,
            dependency_check: true,
        };

        Self {
            container: Container::with_config(container_config),
            config,
            state: Arc::new(RwLock::new(ContextState::Initializing)),
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Create application context from environment
    pub fn from_env() -> SpringResult<Self> {
        let config = AppConfig::from_env()?;
        Ok(Self::with_config(config))
    }

    /// Register a singleton service
    pub async fn register_service<T: Service + 'static>(&self, service: T) -> SpringResult<()> {
        let service_name = service.name().to_string();
        info!("Registering service: {}", service_name);

        self.container
            .register_singleton(service_name, service)
            .await?;

        Ok(())
    }

    /// Register a singleton component
    pub async fn register_component<T: Component + 'static>(
        &self,
        name: impl Into<String>,
        component: T,
    ) -> SpringResult<()> {
        let component_name = name.into();
        info!("Registering component: {}", component_name);

        self.container
            .register_singleton(component_name, component)
            .await?;

        Ok(())
    }

    /// Register a prototype factory
    pub fn register_prototype<T, F>(&self, name: impl Into<String>, factory: F) -> SpringResult<()>
    where
        T: Component + 'static,
        F: Fn() -> SpringResult<T> + Send + Sync + 'static,
    {
        let component_name = name.into();
        info!("Registering prototype factory: {}", component_name);

        self.container.register_prototype(component_name, factory)?;

        Ok(())
    }

    /// Get a service by name
    pub async fn get_service(&self, name: &str) -> SpringResult<Arc<RwLock<dyn Component>>> {
        self.container.get_bean(name).await
    }

    /// Get a service by type
    pub async fn get_service_by_type<T: 'static>(
        &self,
    ) -> SpringResult<Arc<RwLock<dyn Component>>> {
        self.container.get_bean_by_type::<T>().await
    }

    /// Check if a service exists
    pub fn contains_service(&self, name: &str) -> bool {
        self.container.contains_bean(name)
    }

    /// Get all service names
    pub fn get_service_names(&self) -> Vec<String> {
        self.container.get_bean_names()
    }

    /// Start the application context
    pub async fn start(&self) -> SpringResult<()> {
        let mut state = self.state.write().await;

        if *state != ContextState::Initializing {
            return Err(SpringError::context(format!(
                "Cannot start context in state: {:?}",
                *state
            )));
        }

        *state = ContextState::Starting;
        drop(state);

        info!("🚀 Starting Orbit Spring Application Context");
        info!(
            "Application: {} v{}",
            self.config.application.name, self.config.application.version
        );
        info!(
            "Active profiles: {:?}",
            self.config.application.profiles.active
        );

        // Start the dependency injection container
        match self.container.start().await {
            Ok(()) => {
                let mut state = self.state.write().await;
                *state = ContextState::Running;
                info!("✅ Application context started successfully");
            }
            Err(e) => {
                let mut state = self.state.write().await;
                *state = ContextState::Failed(e.to_string());
                return Err(e);
            }
        }

        Ok(())
    }

    /// Stop the application context
    pub async fn stop(&self) -> SpringResult<()> {
        let mut state = self.state.write().await;

        match *state {
            ContextState::Running => {
                *state = ContextState::Stopping;
            }
            ContextState::Stopped => {
                warn!("Application context is already stopped");
                return Ok(());
            }
            _ => {
                warn!("Cannot stop context in state: {:?}", *state);
                return Ok(());
            }
        }

        drop(state);

        info!("🛑 Stopping Orbit Spring Application Context");

        // Signal shutdown
        self.shutdown_token.cancel();

        // Stop the dependency injection container
        match self.container.stop().await {
            Ok(()) => {
                let mut state = self.state.write().await;
                *state = ContextState::Stopped;
                info!("✅ Application context stopped gracefully");
            }
            Err(e) => {
                let mut state = self.state.write().await;
                *state = ContextState::Failed(format!("Failed to stop: {}", e));
                warn!("❌ Failed to stop application context: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Run the application (blocks until shutdown)
    pub async fn run(&self) -> SpringResult<()> {
        // Start the context if not already started
        if *self.state.read().await == ContextState::Initializing {
            self.start().await?;
        }

        let state = self.state.read().await;
        if *state != ContextState::Running {
            return Err(SpringError::context(format!(
                "Cannot run context in state: {:?}",
                *state
            )));
        }
        drop(state);

        info!("🎯 Application context is running");

        // Setup shutdown signal handlers
        self.setup_signal_handlers();

        // Wait for shutdown signal
        self.shutdown_token.cancelled().await;

        info!("📡 Shutdown signal received");

        // Stop the context
        self.stop().await?;

        Ok(())
    }

    /// Get the current context state
    pub async fn state(&self) -> ContextState {
        self.state.read().await.clone()
    }

    /// Get the application configuration
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get container statistics
    pub fn container_stats(&self) -> crate::container::ContainerStats {
        self.container.stats()
    }

    /// Check if the context is running
    pub async fn is_running(&self) -> bool {
        *self.state.read().await == ContextState::Running
    }

    /// Check if the context is stopped
    pub async fn is_stopped(&self) -> bool {
        matches!(
            *self.state.read().await,
            ContextState::Stopped | ContextState::Failed(_)
        )
    }

    /// Get shutdown token for external components
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown_token.clone()
    }

    /// Refresh the context (restart all components)
    pub async fn refresh(&self) -> SpringResult<()> {
        info!("🔄 Refreshing application context");

        // Stop the context
        self.stop().await?;

        // Reset state to initializing
        let mut state = self.state.write().await;
        *state = ContextState::Initializing;
        drop(state);

        // Reset shutdown token
        self.shutdown_token.cancel();

        // Start again
        self.start().await?;

        info!("✅ Application context refreshed successfully");
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

    /// Perform health check on all registered services
    pub async fn health_check(&self) -> HealthCheckResult {
        debug!("Performing health check on all services");

        let mut healthy_services = Vec::new();
        let mut unhealthy_services = Vec::new();

        let service_names = self.get_service_names();

        for service_name in service_names {
            match self.get_service(&service_name).await {
                Ok(service_instance) => {
                    let _service = service_instance.read().await;

                    // For now, we'll assume all components are healthy
                    // In a real implementation, we would check if the component implements Service
                    // and call is_healthy() method
                    healthy_services.push(service_name);
                }
                Err(_) => {
                    unhealthy_services.push(service_name);
                }
            }
        }

        let overall_health = unhealthy_services.is_empty();
        let total_services = healthy_services.len() + unhealthy_services.len();

        HealthCheckResult {
            healthy: overall_health,
            healthy_services,
            unhealthy_services,
            total_services,
        }
    }
}

impl Default for ApplicationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub healthy_services: Vec<String>,
    pub unhealthy_services: Vec<String>,
    pub total_services: usize,
}

impl HealthCheckResult {
    /// Get health status as string
    pub fn status(&self) -> &'static str {
        if self.healthy {
            "UP"
        } else {
            "DOWN"
        }
    }

    /// Get health score (0.0 to 1.0)
    pub fn score(&self) -> f64 {
        if self.total_services == 0 {
            1.0
        } else {
            self.healthy_services.len() as f64 / self.total_services as f64
        }
    }
}
