use crate::{
    annotations::{AutowireMode, BeanDefinition, BeanScope, Component},
    SpringError, SpringResult,
};
use dashmap::DashMap;
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Type-erased bean instance
type BeanInstance = Arc<RwLock<dyn Component>>;

/// Bean factory function
type BeanFactory = Box<dyn Fn() -> SpringResult<BeanInstance> + Send + Sync>;

/// Dependency injection container
pub struct Container {
    /// Bean instances (singletons)
    beans: DashMap<String, BeanInstance>,

    /// Bean definitions
    definitions: DashMap<String, BeanDefinition>,

    /// Bean factories for prototype scope
    factories: DashMap<String, BeanFactory>,

    /// Type to bean name mapping
    type_registry: DashMap<TypeId, Vec<String>>,

    /// Bean dependencies graph
    dependencies: DashMap<String, Vec<String>>,

    /// Container state
    state: Arc<RwLock<ContainerState>>,

    /// Autowire mode
    autowire_mode: AutowireMode,

    /// Container configuration
    config: ContainerConfig,
}

/// Container state
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerState {
    Initializing,
    Running,
    Stopping,
    Stopped,
}

/// Container configuration
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub allow_circular_dependencies: bool,
    pub lazy_initialization: bool,
    pub auto_wire_candidates: bool,
    pub dependency_check: bool,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            allow_circular_dependencies: false,
            lazy_initialization: false,
            auto_wire_candidates: true,
            dependency_check: true,
        }
    }
}

impl Container {
    /// Create a new container
    pub fn new() -> Self {
        Self::with_config(ContainerConfig::default())
    }

    /// Create a new container with configuration
    pub fn with_config(config: ContainerConfig) -> Self {
        Self {
            beans: DashMap::new(),
            definitions: DashMap::new(),
            factories: DashMap::new(),
            type_registry: DashMap::new(),
            dependencies: DashMap::new(),
            state: Arc::new(RwLock::new(ContainerState::Initializing)),
            autowire_mode: AutowireMode::default(),
            config,
        }
    }

    /// Register a singleton bean
    pub async fn register_singleton<T: Component + 'static>(
        &self,
        name: impl Into<String>,
        component: T,
    ) -> SpringResult<()> {
        let name = name.into();
        let type_id = TypeId::of::<T>();

        // Create bean definition
        let definition = BeanDefinition::new(name.clone()).scope(BeanScope::Singleton);

        // Create bean instance
        let instance: BeanInstance = Arc::new(RwLock::new(component));

        // Store bean
        self.beans.insert(name.clone(), instance);
        self.definitions.insert(name.clone(), definition);

        // Register type mapping
        self.type_registry
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(name.clone());

        debug!("Registered singleton bean: {}", name);
        Ok(())
    }

    /// Register a prototype bean factory
    pub fn register_prototype<T, F>(&self, name: impl Into<String>, factory: F) -> SpringResult<()>
    where
        T: Component + 'static,
        F: Fn() -> SpringResult<T> + Send + Sync + 'static,
    {
        let name = name.into();
        let type_id = TypeId::of::<T>();

        // Create bean definition
        let definition = BeanDefinition::new(name.clone()).scope(BeanScope::Prototype);

        // Create factory wrapper
        let bean_factory: BeanFactory = Box::new(move || {
            let component = factory()?;
            let instance: BeanInstance = Arc::new(RwLock::new(component));
            Ok(instance)
        });

        // Store factory and definition
        self.factories.insert(name.clone(), bean_factory);
        self.definitions.insert(name.clone(), definition);

        // Register type mapping
        self.type_registry
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(name.clone());

        debug!("Registered prototype bean factory: {}", name);
        Ok(())
    }

    /// Get bean by name
    pub async fn get_bean(&self, name: &str) -> SpringResult<BeanInstance> {
        // Check if it's a singleton
        if let Some(bean) = self.beans.get(name) {
            return Ok(bean.clone());
        }

        // Check if it's a prototype
        if let Some(factory) = self.factories.get(name) {
            let instance = factory()?;
            return Ok(instance);
        }

        Err(SpringError::bean_not_found(name))
    }

    /// Get bean by type
    pub async fn get_bean_by_type<T: 'static>(&self) -> SpringResult<BeanInstance> {
        let type_id = TypeId::of::<T>();

        if let Some(bean_names) = self.type_registry.get(&type_id) {
            if bean_names.is_empty() {
                return Err(SpringError::bean_not_found(format!(
                    "type: {}",
                    std::any::type_name::<T>()
                )));
            }

            // If multiple beans of same type, look for primary
            if bean_names.len() > 1 {
                for name in bean_names.iter() {
                    if let Some(definition) = self.definitions.get(name) {
                        if definition.primary {
                            return self.get_bean(name).await;
                        }
                    }
                }

                // No primary found, return error for ambiguous beans
                return Err(SpringError::dependency_injection(format!(
                    "Multiple beans of type {} found, no primary bean specified",
                    std::any::type_name::<T>()
                )));
            }

            // Single bean found
            let bean_name = &bean_names[0];
            return self.get_bean(bean_name).await;
        }

        Err(SpringError::bean_not_found(format!(
            "type: {}",
            std::any::type_name::<T>()
        )))
    }

    /// Check if bean exists
    pub fn contains_bean(&self, name: &str) -> bool {
        self.beans.contains_key(name) || self.factories.contains_key(name)
    }

    /// Get all bean names
    pub fn get_bean_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        for entry in self.beans.iter() {
            names.push(entry.key().clone());
        }

        for entry in self.factories.iter() {
            names.push(entry.key().clone());
        }

        names.sort();
        names
    }

    /// Get bean definition
    pub fn get_bean_definition(&self, name: &str) -> SpringResult<BeanDefinition> {
        self.definitions
            .get(name)
            .map(|def| def.clone())
            .ok_or_else(|| SpringError::bean_not_found(name))
    }

    /// Start the container
    pub async fn start(&self) -> SpringResult<()> {
        let mut state = self.state.write().await;

        if *state != ContainerState::Initializing {
            return Err(SpringError::lifecycle(
                "start",
                format!("Cannot start container in state: {:?}", *state),
            ));
        }

        info!("Starting dependency injection container");

        // Validate dependencies
        self.validate_dependencies()?;

        // Initialize singleton beans
        self.initialize_singletons().await?;

        *state = ContainerState::Running;
        info!("✅ Dependency injection container started successfully");

        Ok(())
    }

    /// Stop the container
    pub async fn stop(&self) -> SpringResult<()> {
        let mut state = self.state.write().await;

        if *state != ContainerState::Running {
            warn!("Container is not running, current state: {:?}", *state);
            return Ok(());
        }

        *state = ContainerState::Stopping;
        info!("Stopping dependency injection container");

        // Destroy all singleton beans
        for entry in self.beans.iter() {
            let bean_name = entry.key();
            let bean_instance = entry.value();

            debug!("Destroying bean: {}", bean_name);

            match bean_instance.write().await.destroy().await {
                Ok(()) => debug!("✅ Bean destroyed successfully: {}", bean_name),
                Err(e) => warn!("⚠️  Failed to destroy bean {}: {}", bean_name, e),
            }
        }

        *state = ContainerState::Stopped;
        info!("✅ Dependency injection container stopped");

        Ok(())
    }

    /// Get container state
    pub async fn state(&self) -> ContainerState {
        self.state.read().await.clone()
    }

    /// Validate dependencies for circular references
    fn validate_dependencies(&self) -> SpringResult<()> {
        if !self.config.dependency_check {
            return Ok(());
        }

        debug!("Validating bean dependencies");

        // Build dependency graph
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for entry in self.definitions.iter() {
            let bean_name = entry.key().clone();
            let definition = entry.value();
            graph.insert(bean_name, definition.dependencies.clone());
        }

        // Check for circular dependencies
        for bean_name in graph.keys() {
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            if self.has_circular_dependency(bean_name, &graph, &mut visited, &mut path) {
                if !self.config.allow_circular_dependencies {
                    return Err(SpringError::circular_dependency(path.join(" -> ")));
                } else {
                    warn!(
                        "Circular dependency detected but allowed: {}",
                        path.join(" -> ")
                    );
                }
            }
        }

        debug!("✅ Dependency validation completed");
        Ok(())
    }

    /// Check for circular dependencies using DFS
    fn has_circular_dependency(
        &self,
        bean_name: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        if path.contains(&bean_name.to_string()) {
            path.push(bean_name.to_string());
            return true;
        }

        if visited.contains(bean_name) {
            return false;
        }

        visited.insert(bean_name.to_string());
        path.push(bean_name.to_string());

        if let Some(dependencies) = graph.get(bean_name) {
            for dep in dependencies {
                if self.has_circular_dependency(dep, graph, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
    }

    /// Initialize singleton beans
    async fn initialize_singletons(&self) -> SpringResult<()> {
        debug!("Initializing singleton beans");

        for entry in self.beans.iter() {
            let bean_name = entry.key();
            let bean_instance = entry.value();

            debug!("Initializing bean: {}", bean_name);

            match bean_instance.write().await.initialize().await {
                Ok(()) => debug!("✅ Bean initialized successfully: {}", bean_name),
                Err(e) => {
                    return Err(SpringError::lifecycle(
                        "initialization",
                        format!("Failed to initialize bean {}: {}", bean_name, e),
                    ));
                }
            }
        }

        debug!("✅ All singleton beans initialized");
        Ok(())
    }

    /// Get container statistics
    pub fn stats(&self) -> ContainerStats {
        ContainerStats {
            singleton_count: self.beans.len(),
            prototype_count: self.factories.len(),
            type_registrations: self.type_registry.len(),
            total_beans: self.beans.len() + self.factories.len(),
        }
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

/// Container statistics
#[derive(Debug, Clone)]
pub struct ContainerStats {
    pub singleton_count: usize,
    pub prototype_count: usize,
    pub type_registrations: usize,
    pub total_beans: usize,
}
