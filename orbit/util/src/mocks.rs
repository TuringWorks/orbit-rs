//! Comprehensive mock framework for Orbit components
//!
//! This module provides mock implementations of OrbitClient, OrbitServer, and other
//! Orbit components for use in testing. The mocks can be configured to simulate various
//! scenarios including success, failure, and edge cases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for mock behavior
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Whether to simulate network delays
    pub simulate_network_delay: bool,
    /// Network delay duration (if enabled)
    pub network_delay: Duration,
    /// Failure rate (0.0 = no failures, 1.0 = all failures)
    pub failure_rate: f64,
    /// Maximum number of actors to keep in memory
    pub max_actors: usize,
    /// Whether to enable detailed logging
    pub verbose_logging: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            simulate_network_delay: false,
            network_delay: Duration::from_millis(10),
            failure_rate: 0.0,
            max_actors: 1000,
            verbose_logging: false,
        }
    }
}

/// Mock actor storage for simulating actor state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockActorState {
    pub actor_id: String,
    pub actor_type: String,
    pub state: serde_json::Value,
    pub message_count: u64,
    pub last_access: std::time::SystemTime,
    pub is_active: bool,
}

impl MockActorState {
    pub fn new(actor_id: String, actor_type: String) -> Self {
        Self {
            actor_id,
            actor_type,
            state: serde_json::json!({}),
            message_count: 0,
            last_access: std::time::SystemTime::now(),
            is_active: false,
        }
    }
}

/// Thread-safe mock actor registry
pub type MockActorRegistry = Arc<RwLock<HashMap<String, MockActorState>>>;

/// Mock invocation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockInvocationResult {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

impl MockInvocationResult {
    pub fn success<T: Serialize>(result: T) -> Self {
        Self {
            success: true,
            result: serde_json::to_value(result).ok(),
            error: None,
            execution_time_ms: 1,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.to_string()),
            execution_time_ms: 1,
        }
    }
}

/// Mock OrbitClient that simulates client-side operations
#[derive(Debug, Clone)]
pub struct MockOrbitClient {
    config: MockConfig,
    namespace: String,
    actor_registry: MockActorRegistry,
    node_id: Option<String>,
    connection_count: Arc<Mutex<u32>>,
}

impl MockOrbitClient {
    /// Create a new mock OrbitClient
    pub fn new(namespace: String) -> Self {
        Self::with_config(namespace, MockConfig::default())
    }

    /// Create a new mock OrbitClient with custom configuration
    pub fn with_config(namespace: String, config: MockConfig) -> Self {
        Self {
            config,
            namespace,
            actor_registry: Arc::new(RwLock::new(HashMap::new())),
            node_id: Some(format!("mock-node-{}", uuid::Uuid::new_v4())),
            connection_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Simulate network delay if configured
    async fn simulate_network_delay(&self) {
        if self.config.simulate_network_delay {
            tokio::time::sleep(self.config.network_delay).await;
        }
    }

    /// Check if operation should fail based on failure rate
    fn should_fail(&self) -> bool {
        if self.config.failure_rate <= 0.0 {
            return false;
        }
        if self.config.failure_rate >= 1.0 {
            return true;
        }
        rand::random::<f64>() < self.config.failure_rate
    }

    /// Get or create actor state
    pub async fn get_or_create_actor(&self, actor_id: &str, actor_type: &str) -> MockActorState {
        let mut registry = self.actor_registry.write().await;

        if let Some(actor) = registry.get_mut(actor_id) {
            actor.last_access = std::time::SystemTime::now();
            if !actor.is_active {
                actor.is_active = true;
                if self.config.verbose_logging {
                    eprintln!("🔄 Mock: Activated actor {} ({})", actor_id, actor_type);
                }
            }
            actor.clone()
        } else {
            let mut actor = MockActorState::new(actor_id.to_string(), actor_type.to_string());
            actor.is_active = true;

            if self.config.verbose_logging {
                eprintln!("✨ Mock: Created new actor {} ({})", actor_id, actor_type);
            }

            registry.insert(actor_id.to_string(), actor.clone());
            actor
        }
    }

    /// Update actor state
    pub async fn update_actor_state(
        &self,
        actor_id: &str,
        new_state: serde_json::Value,
    ) -> Result<(), String> {
        let mut registry = self.actor_registry.write().await;

        if let Some(actor) = registry.get_mut(actor_id) {
            actor.state = new_state;
            actor.message_count += 1;
            actor.last_access = std::time::SystemTime::now();

            if self.config.verbose_logging {
                eprintln!("📝 Mock: Updated actor {} state", actor_id);
            }

            Ok(())
        } else {
            Err(format!("Actor {} not found", actor_id))
        }
    }

    /// Simulate actor method invocation
    pub async fn invoke_actor_method<T, R>(
        &self,
        actor_id: &str,
        actor_type: &str,
        method: &str,
        args: T,
    ) -> Result<R, String>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de> + Default,
    {
        self.simulate_network_delay().await;

        if self.should_fail() {
            return Err(format!(
                "Mock failure: Actor invocation failed for {}.{}",
                actor_type, method
            ));
        }

        let actor = self.get_or_create_actor(actor_id, actor_type).await;

        if self.config.verbose_logging {
            eprintln!(
                "📞 Mock: Invoking {}.{}() on actor {}",
                actor_type, method, actor_id
            );
        }

        // Simulate different method behaviors
        let result = match method {
            "get_state" => serde_json::from_value(actor.state.clone()).unwrap_or_default(),
            "get_message_count" => {
                serde_json::from_value(serde_json::json!(actor.message_count)).unwrap_or_default()
            }
            "process_message" | "send_message" => {
                // Update message count and return a mock response
                let new_count = actor.message_count + 1;
                self.update_actor_state(
                    actor_id,
                    serde_json::json!({
                        "message_count": new_count,
                        "last_message": args
                    }),
                )
                .await
                .ok();

                serde_json::from_value(serde_json::json!({
                    "processed": true,
                    "actor_id": actor_id,
                    "response_time_ms": 1
                }))
                .unwrap_or_default()
            }
            "increment" => {
                // Mock counter increment
                let current_value: i64 = actor
                    .state
                    .get("value")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let increment_amount: i64 =
                    serde_json::from_value(serde_json::to_value(args).unwrap_or_default())
                        .unwrap_or(1);
                let new_value = current_value + increment_amount;
                let new_count = actor.message_count + 1;

                self.update_actor_state(
                    actor_id,
                    serde_json::json!({
                        "value": new_value,
                        "message_count": new_count
                    }),
                )
                .await
                .ok();

                serde_json::from_value(serde_json::json!({
                    "new_value": new_value,
                    "message_count": new_count
                }))
                .unwrap_or_default()
            }
            "get_value" => {
                let value: i64 = actor
                    .state
                    .get("value")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                serde_json::from_value(serde_json::json!(value)).unwrap_or_default()
            }
            "set_state" | "set_value" => {
                self.update_actor_state(actor_id, serde_json::to_value(args).unwrap_or_default())
                    .await
                    .ok();
                serde_json::from_value(serde_json::json!(())).unwrap_or_default()
            }
            "reset" => {
                self.update_actor_state(
                    actor_id,
                    serde_json::json!({
                        "value": 0,
                        "message_count": 0
                    }),
                )
                .await
                .ok();
                serde_json::from_value(serde_json::json!(())).unwrap_or_default()
            }
            _ => {
                // Generic method - return default response
                R::default()
            }
        };

        Ok(result)
    }

    /// Simulate actor deactivation
    pub async fn deactivate_actor(&self, actor_id: &str) -> Result<(), String> {
        self.simulate_network_delay().await;

        if self.should_fail() {
            return Err("Mock failure: Actor deactivation failed".to_string());
        }

        let mut registry = self.actor_registry.write().await;
        if let Some(actor) = registry.get_mut(actor_id) {
            actor.is_active = false;

            if self.config.verbose_logging {
                eprintln!("😴 Mock: Deactivated actor {}", actor_id);
            }

            Ok(())
        } else {
            Err(format!("Actor {} not found", actor_id))
        }
    }

    /// Get client statistics
    pub async fn get_stats(&self) -> MockClientStats {
        let registry = self.actor_registry.read().await;
        let active_actors = registry.values().filter(|a| a.is_active).count();
        let total_actors = registry.len();

        MockClientStats {
            namespace: self.namespace.clone(),
            node_id: self.node_id.clone(),
            active_actors,
            total_actors,
            connection_count: *self.connection_count.lock().unwrap(),
        }
    }

    /// Simulate shutdown
    pub async fn shutdown(&self) -> Result<(), String> {
        if self.config.verbose_logging {
            eprintln!("🛑 Mock: Shutting down OrbitClient");
        }

        let mut registry = self.actor_registry.write().await;
        for actor in registry.values_mut() {
            actor.is_active = false;
        }

        Ok(())
    }
}

/// Mock client statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockClientStats {
    pub namespace: String,
    pub node_id: Option<String>,
    pub active_actors: usize,
    pub total_actors: usize,
    pub connection_count: u32,
}

/// Mock OrbitServer for server-side testing
#[derive(Debug, Clone)]
pub struct MockOrbitServer {
    config: MockConfig,
    port: u16,
    actor_registry: MockActorRegistry,
    registered_types: Arc<Mutex<Vec<String>>>,
    is_running: Arc<Mutex<bool>>,
}

impl MockOrbitServer {
    /// Create a new mock OrbitServer
    pub fn new(port: u16) -> Self {
        Self::with_config(port, MockConfig::default())
    }

    /// Create a new mock OrbitServer with custom configuration
    pub fn with_config(port: u16, config: MockConfig) -> Self {
        Self {
            config,
            port,
            actor_registry: Arc::new(RwLock::new(HashMap::new())),
            registered_types: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Register an actor type
    pub async fn register_actor_type(&self, actor_type: &str) -> Result<(), String> {
        let mut types = self.registered_types.lock().unwrap();
        if !types.contains(&actor_type.to_string()) {
            types.push(actor_type.to_string());

            if self.config.verbose_logging {
                eprintln!("📋 Mock: Registered actor type {}", actor_type);
            }
        }
        Ok(())
    }

    /// Start the mock server
    pub async fn start(&self) -> Result<(), String> {
        if self.should_fail() {
            return Err("Mock failure: Server startup failed".to_string());
        }

        *self.is_running.lock().unwrap() = true;

        if self.config.verbose_logging {
            eprintln!("🚀 Mock: OrbitServer started on port {}", self.port);
        }

        Ok(())
    }

    /// Stop the mock server
    pub async fn stop(&self) -> Result<(), String> {
        *self.is_running.lock().unwrap() = false;

        if self.config.verbose_logging {
            eprintln!("🛑 Mock: OrbitServer stopped");
        }

        Ok(())
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> MockServerStats {
        let registry = self.actor_registry.read().await;
        let types = self.registered_types.lock().unwrap();

        MockServerStats {
            port: self.port,
            is_running: self.is_running(),
            registered_actor_types: types.len(),
            active_actors: registry.values().filter(|a| a.is_active).count(),
            total_actors: registry.len(),
        }
    }

    fn should_fail(&self) -> bool {
        if self.config.failure_rate <= 0.0 {
            return false;
        }
        if self.config.failure_rate >= 1.0 {
            return true;
        }
        rand::random::<f64>() < self.config.failure_rate
    }
}

/// Mock server statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockServerStats {
    pub port: u16,
    pub is_running: bool,
    pub registered_actor_types: usize,
    pub active_actors: usize,
    pub total_actors: usize,
}

/// Mock actor reference for testing
#[derive(Debug, Clone)]
pub struct MockActorReference {
    actor_id: String,
    actor_type: String,
    client: MockOrbitClient,
}

impl MockActorReference {
    pub fn new(actor_id: String, actor_type: String, client: MockOrbitClient) -> Self {
        Self {
            actor_id,
            actor_type,
            client,
        }
    }

    /// Invoke a method on the mock actor
    pub async fn invoke_method<T, R>(&self, method: &str, args: T) -> Result<R, String>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de> + Default,
    {
        self.client
            .invoke_actor_method(&self.actor_id, &self.actor_type, method, args)
            .await
    }
}

/// Builder for mock test setups
pub struct MockTestSetupBuilder {
    client_config: MockConfig,
    server_config: MockConfig,
    namespace: String,
    server_port: u16,
}

impl Default for MockTestSetupBuilder {
    fn default() -> Self {
        Self {
            client_config: MockConfig::default(),
            server_config: MockConfig::default(),
            namespace: "test".to_string(),
            server_port: 9999,
        }
    }
}

impl MockTestSetupBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    pub fn with_server_port(mut self, port: u16) -> Self {
        self.server_port = port;
        self
    }

    pub fn with_client_config(mut self, config: MockConfig) -> Self {
        self.client_config = config;
        self
    }

    pub fn with_server_config(mut self, config: MockConfig) -> Self {
        self.server_config = config;
        self
    }

    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.client_config.failure_rate = rate;
        self.server_config.failure_rate = rate;
        self
    }

    pub fn with_network_delay(mut self, delay: Duration) -> Self {
        self.client_config.simulate_network_delay = true;
        self.client_config.network_delay = delay;
        self.server_config.simulate_network_delay = true;
        self.server_config.network_delay = delay;
        self
    }

    pub fn with_verbose_logging(mut self, enabled: bool) -> Self {
        self.client_config.verbose_logging = enabled;
        self.server_config.verbose_logging = enabled;
        self
    }

    pub async fn build(self) -> MockTestSetup {
        let server = MockOrbitServer::with_config(self.server_port, self.server_config);
        let client = MockOrbitClient::with_config(self.namespace, self.client_config);

        MockTestSetup {
            server,
            client,
            server_port: self.server_port,
        }
    }
}

/// Complete mock test setup with client and server
pub struct MockTestSetup {
    pub server: MockOrbitServer,
    pub client: MockOrbitClient,
    pub server_port: u16,
}

impl MockTestSetup {
    /// Create a new mock test setup with default configuration
    pub async fn new() -> Self {
        MockTestSetupBuilder::new().build().await
    }

    /// Create a new mock test setup with builder pattern
    pub fn builder() -> MockTestSetupBuilder {
        MockTestSetupBuilder::new()
    }

    /// Start both server and client (simulated)
    pub async fn start(&self) -> Result<(), String> {
        self.server.start().await?;
        // Client is already "started" when created
        Ok(())
    }

    /// Create a mock actor reference
    pub fn actor_reference<T>(&self, actor_id: impl Into<String>) -> MockActorReference
    where
        T: 'static,
    {
        let type_name = std::any::type_name::<T>()
            .split("::")
            .last()
            .unwrap_or("UnknownActor")
            .to_string();

        MockActorReference::new(actor_id.into(), type_name, self.client.clone())
    }

    /// Register actor types on the server
    pub async fn register_actor_types(&self, types: &[&str]) -> Result<(), String> {
        for actor_type in types {
            self.server.register_actor_type(actor_type).await?;
        }
        Ok(())
    }

    /// Cleanup resources
    pub async fn cleanup(&self) -> Result<(), String> {
        self.client.shutdown().await?;
        self.server.stop().await?;
        Ok(())
    }

    /// Get combined statistics
    pub async fn get_stats(&self) -> (MockClientStats, MockServerStats) {
        let client_stats = self.client.get_stats().await;
        let server_stats = self.server.get_stats().await;
        (client_stats, server_stats)
    }
}

/// Convenience macro for creating mock test setups
#[macro_export]
macro_rules! mock_test_setup {
    () => {
        $crate::mocks::MockTestSetup::new().await
    };

    (namespace: $ns:expr) => {
        $crate::mocks::MockTestSetup::builder()
            .with_namespace($ns)
            .build()
            .await
    };

    (namespace: $ns:expr, port: $port:expr) => {
        $crate::mocks::MockTestSetup::builder()
            .with_namespace($ns)
            .with_server_port($port)
            .build()
            .await
    };

    (verbose: true) => {
        $crate::mocks::MockTestSetup::builder()
            .with_verbose_logging(true)
            .build()
            .await
    };
}

/// Convenience macro for skipping tests when conditions aren't met
#[macro_export]
macro_rules! skip_test_if {
    ($condition:expr, $reason:expr) => {
        if $condition {
            eprintln!("⏭️  Skipping test: {}", $reason);
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_basic_operations() {
        let client = MockOrbitClient::new("test".to_string());

        // Test actor invocation
        let result: String = client
            .invoke_actor_method("test-actor", "TestActor", "get_state", ())
            .await
            .unwrap();
        assert_eq!(result, String::default());

        // Test actor creation
        let actor = client.get_or_create_actor("test-actor", "TestActor").await;
        assert_eq!(actor.actor_id, "test-actor");
        assert_eq!(actor.actor_type, "TestActor");
    }

    #[tokio::test]
    async fn test_mock_server_operations() {
        let server = MockOrbitServer::new(8080);

        assert!(!server.is_running());

        server.register_actor_type("TestActor").await.unwrap();
        server.start().await.unwrap();

        assert!(server.is_running());

        server.stop().await.unwrap();
        assert!(!server.is_running());
    }

    #[tokio::test]
    async fn test_mock_test_setup() {
        let setup = MockTestSetup::new().await;
        setup.start().await.unwrap();

        // Test actor reference creation
        let actor = setup.actor_reference::<String>("test-actor");
        let result: String = actor.invoke_method("get_state", ()).await.unwrap();
        assert_eq!(result, String::default());

        setup.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_failure_simulation() {
        let config = MockConfig {
            failure_rate: 1.0, // Always fail
            ..Default::default()
        };

        let client = MockOrbitClient::with_config("test".to_string(), config);

        let result: Result<String, _> = client
            .invoke_actor_method("test-actor", "TestActor", "get_state", ())
            .await;
        assert!(result.is_err());
    }
}
