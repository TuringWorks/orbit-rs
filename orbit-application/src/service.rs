use crate::AppError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[async_trait]
pub trait Service: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<(), AppError>;
    async fn stop(&self) -> Result<(), AppError>;
    fn is_healthy(&self) -> bool;
}

#[derive(Default)]
pub struct ServiceRegistry {
    services: HashMap<String, Arc<dyn Service>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub async fn register(&mut self, service: Arc<dyn Service>) -> Result<(), AppError> {
        self.services.insert(service.name().to_string(), service);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Service>> {
        self.services.get(name).cloned()
    }

    pub async fn shutdown_all(&self) -> Result<(), AppError> {
        Ok(())
    }
}

impl fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("services", &self.services.keys().collect::<Vec<_>>())
            .finish()
    }
}
