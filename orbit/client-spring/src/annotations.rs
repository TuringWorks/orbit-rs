use crate::{SpringError, SpringResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;

/// Trait for components that can be managed by the Spring container
/// Similar to @Component annotation in Spring
#[async_trait]
pub trait Component: Send + Sync + Debug {
    /// Returns the component name/identifier
    fn name(&self) -> &'static str;

    /// Initialize the component (called during container startup)
    async fn initialize(&mut self) -> SpringResult<()> {
        Ok(())
    }

    /// Cleanup the component (called during container shutdown)
    async fn destroy(&mut self) -> SpringResult<()> {
        Ok(())
    }

    /// Get component as Any for dynamic casting
    fn as_any(&self) -> &dyn Any;

    /// Get mutable component as Any for dynamic casting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Trait for service components
/// Similar to @Service annotation in Spring
#[async_trait]
pub trait Service: Component {
    /// Returns the service version
    fn version(&self) -> &'static str {
        "1.0.0"
    }

    /// Check if the service is healthy
    fn is_healthy(&self) -> bool {
        true
    }

    /// Get service metrics/statistics
    fn metrics(&self) -> ServiceMetrics {
        ServiceMetrics::default()
    }
}

/// Trait for repository components
/// Similar to @Repository annotation in Spring
#[async_trait]
pub trait Repository<T>: Component
where
    T: Send + Sync + Clone,
{
    /// Find an entity by ID
    async fn find_by_id(&self, id: &str) -> SpringResult<Option<T>>;

    /// Find all entities
    async fn find_all(&self) -> SpringResult<Vec<T>>;

    /// Save an entity
    async fn save(&mut self, entity: T) -> SpringResult<T>;

    /// Delete an entity by ID
    async fn delete_by_id(&mut self, id: &str) -> SpringResult<bool>;

    /// Check if an entity exists by ID
    async fn exists_by_id(&self, id: &str) -> SpringResult<bool>;

    /// Count total entities
    async fn count(&self) -> SpringResult<u64>;
}

/// Trait for configuration beans
/// Similar to @Configuration annotation in Spring
pub trait Configuration: Send + Sync + Debug {
    /// Returns the configuration name
    fn name(&self) -> &'static str;

    /// Validate the configuration
    fn validate(&self) -> SpringResult<()> {
        Ok(())
    }

    /// Get configuration properties as a map
    fn properties(&self) -> std::collections::HashMap<String, ConfigValue>;

    /// Get a specific property value
    fn get_property(&self, key: &str) -> Option<&ConfigValue>;
}

/// Configuration value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(std::collections::HashMap<String, ConfigValue>),
}

impl ConfigValue {
    pub fn as_string(&self) -> SpringResult<&String> {
        match self {
            ConfigValue::String(s) => Ok(s),
            _ => Err(SpringError::validation("type", "Expected string value")),
        }
    }

    pub fn as_integer(&self) -> SpringResult<i64> {
        match self {
            ConfigValue::Integer(i) => Ok(*i),
            _ => Err(SpringError::validation("type", "Expected integer value")),
        }
    }

    pub fn as_float(&self) -> SpringResult<f64> {
        match self {
            ConfigValue::Float(f) => Ok(*f),
            _ => Err(SpringError::validation("type", "Expected float value")),
        }
    }

    pub fn as_boolean(&self) -> SpringResult<bool> {
        match self {
            ConfigValue::Boolean(b) => Ok(*b),
            _ => Err(SpringError::validation("type", "Expected boolean value")),
        }
    }
}

/// Service metrics data structure
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ServiceMetrics {
    pub requests_total: u64,
    pub requests_per_second: f64,
    pub error_rate: f64,
    pub average_response_time_ms: f64,
    pub uptime_seconds: u64,
    pub memory_usage_bytes: u64,
}

/// Bean scope enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BeanScope {
    /// Single instance per container (default)
    #[default]
    Singleton,
    /// New instance per request
    Prototype,
    /// New instance per request (alias for Prototype)
    Request,
    /// New instance per session
    Session,
    /// New instance per application
    Application,
}

/// Bean definition metadata
#[derive(Debug, Clone)]
pub struct BeanDefinition {
    pub name: String,
    pub scope: BeanScope,
    pub lazy_init: bool,
    pub dependencies: Vec<String>,
    pub primary: bool,
    pub description: Option<String>,
}

impl BeanDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: BeanScope::default(),
            lazy_init: false,
            dependencies: Vec::new(),
            primary: false,
            description: None,
        }
    }

    pub fn scope(mut self, scope: BeanScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn lazy(mut self) -> Self {
        self.lazy_init = true;
        self
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn depends_on(mut self, dependency: impl Into<String>) -> Self {
        self.dependencies.push(dependency.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Auto-wiring modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutowireMode {
    /// No autowiring
    No,
    /// Autowire by name
    ByName,
    /// Autowire by type
    #[default]
    ByType,
    /// Autowire by constructor
    Constructor,
    /// Auto-detect autowiring mode
    Autodetect,
}
