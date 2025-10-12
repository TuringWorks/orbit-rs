use crate::{AppError, EventBus};
use std::sync::Arc;
use tracing::info;

/// Lifecycle management for the application
#[derive(Debug)]
pub struct LifecycleManager {
    #[allow(dead_code)]
    shutdown_token: tokio_util::sync::CancellationToken,
    #[allow(dead_code)]
    event_bus: Arc<EventBus>,
}

impl LifecycleManager {
    pub fn new(
        shutdown_token: tokio_util::sync::CancellationToken,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            shutdown_token,
            event_bus,
        }
    }

    pub async fn start(&self) -> Result<(), AppError> {
        info!("Lifecycle manager started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), AppError> {
        info!("Lifecycle manager stopped");
        Ok(())
    }
}
