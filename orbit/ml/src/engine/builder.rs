use crate::config::MLConfig;
use crate::engine::MLEngine;
use crate::error::Result;

/// Builder for configuring and constructing ML engines
///
/// Provides a fluent interface for setting up ML engine configuration
/// and creating properly initialized ML engine instances.
#[derive(Default)]
pub struct MLEngineBuilder {
    config: Option<MLConfig>,
}

impl MLEngineBuilder {
    /// Create a new ML engine builder with default configuration
    ///
    /// # Returns
    /// A new builder instance ready for configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration for the ML engine
    ///
    /// # Arguments
    /// * `config` - ML engine configuration parameters
    ///
    /// # Returns
    /// Builder instance with configuration applied for method chaining
    pub fn with_config(mut self, config: MLConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the configured ML engine
    ///
    /// # Returns
    /// A fully initialized ML engine instance or an error if construction fails
    ///
    /// # Errors
    /// Returns an error if the ML engine fails to initialize with the given configuration
    pub async fn build(self) -> Result<MLEngine> {
        let config = self.config.unwrap_or_default();
        MLEngine::with_config(config).await
    }
}
