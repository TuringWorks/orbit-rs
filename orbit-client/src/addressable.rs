//! Client-side addressable management and lifecycle

use async_trait::async_trait;
use orbit_shared::{Addressable, AddressableReference, NodeId, OrbitError, OrbitResult};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};

/// Reason for actor deactivation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeactivationReason {
    Timeout,
    Lease,
    Node,
    Explicit,
}

impl fmt::Display for DeactivationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeactivationReason::Timeout => write!(f, "Timeout"),
            DeactivationReason::Lease => write!(f, "Lease"),
            DeactivationReason::Node => write!(f, "Node"),
            DeactivationReason::Explicit => write!(f, "Explicit"),
        }
    }
}

/// Actor context providing access to system services
#[derive(Debug, Clone)]
pub struct ActorContext {
    pub reference: AddressableReference,
    pub client_node_id: Option<NodeId>,
}

impl ActorContext {
    pub fn new(reference: AddressableReference) -> Self {
        Self {
            reference,
            client_node_id: None,
        }
    }

    pub fn with_client_node_id(mut self, node_id: NodeId) -> Self {
        self.client_node_id = Some(node_id);
        self
    }
}

/// Trait for actor lifecycle callbacks
#[async_trait]
pub trait ActorLifecycle: Send + Sync {
    /// Called when the actor is first activated
    async fn on_activate(&self, _context: &ActorContext) -> OrbitResult<()> {
        Ok(())
    }

    /// Called when the actor is deactivated
    async fn on_deactivate(
        &self,
        _context: &ActorContext,
        _reason: DeactivationReason,
    ) -> OrbitResult<()> {
        Ok(())
    }
}

/// Trait implemented by all actor implementations
#[async_trait]
pub trait ActorImplementation: ActorLifecycle + Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn type_id(&self) -> TypeId {
        std::any::TypeId::of::<Self>()
    }
}

/// Actor instance with metadata
pub struct ActorInstance {
    pub reference: AddressableReference,
    pub implementation: Arc<dyn ActorImplementation>,
    pub context: ActorContext,
    pub created_at: Instant,
    pub last_accessed: Arc<Mutex<Instant>>,
    pub is_active: Arc<Mutex<bool>>,
}

impl ActorInstance {
    pub fn new(
        reference: AddressableReference,
        implementation: Arc<dyn ActorImplementation>,
        context: ActorContext,
    ) -> Self {
        let now = Instant::now();
        Self {
            reference,
            implementation,
            context,
            created_at: now,
            last_accessed: Arc::new(Mutex::new(now)),
            is_active: Arc::new(Mutex::new(true)),
        }
    }

    pub async fn touch(&self) {
        let mut last_accessed = self.last_accessed.lock().await;
        *last_accessed = Instant::now();
    }

    pub async fn deactivate(&self, reason: DeactivationReason) -> OrbitResult<()> {
        let mut is_active = self.is_active.lock().await;
        if *is_active {
            *is_active = false;
            self.implementation
                .on_deactivate(&self.context, reason)
                .await?;
        }
        Ok(())
    }

    pub async fn is_active(&self) -> bool {
        *self.is_active.lock().await
    }

    pub async fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub async fn idle_time(&self) -> Duration {
        let last_accessed = self.last_accessed.lock().await;
        last_accessed.elapsed()
    }
}

/// Registry for managing actor instances
#[derive(Clone)]
pub struct ActorRegistry {
    instances: Arc<RwLock<HashMap<AddressableReference, Arc<ActorInstance>>>>,
    constructors: Arc<RwLock<HashMap<String, Arc<dyn ActorConstructor>>>>,
}

impl ActorRegistry {
    pub fn new() -> Self {
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            constructors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_constructor<T>(&self, constructor: Arc<dyn ActorConstructor>)
    where
        T: Addressable + 'static,
    {
        let type_name = T::addressable_type();
        let mut constructors = self.constructors.write().await;
        constructors.insert(type_name.to_string(), constructor);
    }

    pub async fn get_or_create_instance(
        &self,
        reference: AddressableReference,
    ) -> OrbitResult<Arc<ActorInstance>> {
        // First try to get existing instance
        {
            let instances = self.instances.read().await;
            if let Some(instance) = instances.get(&reference) {
                instance.touch().await;
                return Ok(instance.clone());
            }
        }

        // Create new instance
        let constructors = self.constructors.read().await;
        let constructor = constructors
            .get(&reference.addressable_type)
            .ok_or_else(|| OrbitError::AddressableNotFound {
                reference: reference.to_string(),
            })?;

        let context = ActorContext::new(reference.clone());
        let implementation = constructor.construct(&context).await?;
        let instance = Arc::new(ActorInstance::new(
            reference.clone(),
            implementation,
            context,
        ));

        // Activate the instance
        instance
            .implementation
            .on_activate(&instance.context)
            .await?;

        // Store the instance
        {
            let mut instances = self.instances.write().await;
            instances.insert(reference, instance.clone());
        }

        Ok(instance)
    }

    pub async fn deactivate_instance(
        &self,
        reference: &AddressableReference,
        reason: DeactivationReason,
    ) -> OrbitResult<()> {
        let mut instances = self.instances.write().await;
        if let Some(instance) = instances.remove(reference) {
            instance.deactivate(reason).await?;
        }
        Ok(())
    }

    pub async fn cleanup_idle_instances(&self, max_idle: Duration) -> OrbitResult<()> {
        let mut instances = self.instances.write().await;
        let mut to_remove = Vec::new();

        for (reference, instance) in instances.iter() {
            if instance.idle_time().await > max_idle {
                to_remove.push(reference.clone());
            }
        }

        for reference in to_remove {
            if let Some(instance) = instances.remove(&reference) {
                instance.deactivate(DeactivationReason::Timeout).await?;
            }
        }

        Ok(())
    }
}

impl Default for ActorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for constructing actor instances
#[async_trait]
pub trait ActorConstructor: Send + Sync {
    async fn construct(&self, context: &ActorContext) -> OrbitResult<Arc<dyn ActorImplementation>>;
}

/// Default constructor that creates instances using a closure
pub struct DefaultActorConstructor<F, T>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: ActorImplementation + 'static,
{
    factory: F,
}

impl<F, T> DefaultActorConstructor<F, T>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: ActorImplementation + 'static,
{
    pub fn new(factory: F) -> Self {
        Self { factory }
    }
}

#[async_trait]
impl<F, T> ActorConstructor for DefaultActorConstructor<F, T>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: ActorImplementation + 'static,
{
    async fn construct(
        &self,
        _context: &ActorContext,
    ) -> OrbitResult<Arc<dyn ActorImplementation>> {
        let instance = (self.factory)();
        Ok(Arc::new(instance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::Key;

    struct TestActor;

    #[async_trait]
    impl ActorLifecycle for TestActor {}

    #[async_trait]
    impl ActorImplementation for TestActor {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    impl Addressable for TestActor {
        fn addressable_type() -> &'static str {
            "TestActor"
        }
    }

    #[tokio::test]
    async fn test_actor_registry() {
        let registry = ActorRegistry::new();
        let constructor = Arc::new(DefaultActorConstructor::new(|| TestActor));

        registry
            .register_constructor::<TestActor>(constructor)
            .await;

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let instance = registry
            .get_or_create_instance(reference.clone())
            .await
            .unwrap();
        assert!(instance.is_active().await);
    }

    #[tokio::test]
    async fn test_actor_deactivation() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let context = ActorContext::new(reference.clone());
        let implementation = Arc::new(TestActor);
        let instance = ActorInstance::new(reference, implementation, context);

        assert!(instance.is_active().await);
        instance
            .deactivate(DeactivationReason::Explicit)
            .await
            .unwrap();
        assert!(!instance.is_active().await);
    }
}
