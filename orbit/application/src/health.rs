use crate::{config::HealthConfig, AppError, Service};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct HealthChecker;

impl HealthChecker {
    pub fn new() -> Self {
        Self
    }
}

pub struct HealthService {
    _checker: Arc<HealthChecker>,
    _config: HealthConfig,
}

impl HealthService {
    pub fn new(checker: Arc<HealthChecker>, config: HealthConfig) -> Self {
        Self {
            _checker: checker,
            _config: config,
        }
    }
}

#[async_trait]
impl Service for HealthService {
    fn name(&self) -> &str {
        "health"
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
