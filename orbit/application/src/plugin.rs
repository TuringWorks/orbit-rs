use crate::{AppError, PluginConfig};

#[derive(Debug, Default)]
pub struct PluginManager;

impl PluginManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn load_plugin(&mut self, _config: &PluginConfig) -> Result<(), AppError> {
        Ok(())
    }

    pub async fn shutdown_all(&mut self) -> Result<(), AppError> {
        Ok(())
    }
}
