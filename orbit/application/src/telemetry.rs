use crate::{AppError, Service, TelemetryConfig};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug)]
pub struct TelemetryManager {
    _config: TelemetryConfig,
}

impl TelemetryManager {
    pub async fn new(config: TelemetryConfig) -> Result<Self, AppError> {
        Ok(Self { _config: config })
    }

    pub async fn initialize(&self) -> Result<(), AppError> {
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct TelemetryService {
    _manager: Arc<TelemetryManager>,
}

impl TelemetryService {
    pub fn new(manager: Arc<TelemetryManager>) -> Self {
        Self { _manager: manager }
    }
}

#[async_trait]
impl Service for TelemetryService {
    fn name(&self) -> &str {
        "telemetry"
    }

    async fn start(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        true
    }
}
