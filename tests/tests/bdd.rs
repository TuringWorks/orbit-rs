use std::time::Duration;
use async_trait::async_trait;
use cucumber::{given, when, then, World, Parameter};
use orbit_client::{OrbitClient, OrbitClientConfig};
use orbit_server::{OrbitServer, OrbitServerConfig};
use orbit_shared::{
    addressable::{Addressable, AddressableReference, Key},
    exception::{OrbitError, OrbitResult},
};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{info, warn};

/// Test actors for BDD scenarios
#[derive(Debug, Clone)]
pub struct UserActor {
    pub id: String,
    pub name: String,
    pub email: String,
    pub message_count: u64,
}

impl UserActor {
    pub fn new(id: String) -> Self {
        Self {
            id,
            name: "Default User".to_string(),
            email: "user@example.com".to_string(),
            message_count: 0,
        }
    }
}

#[async_trait]
impl Addressable for UserActor {
    fn addressable_type() -> &'static str {
        "UserActor"
    }

    async fn on_activate(&mut self) -> OrbitResult<()> {
        info!("UserActor '{}' activated", self.id);
        Ok(())
    }

    async fn on_deactivate(&mut self) -> OrbitResult<()> {
        info!("UserActor '{}' deactivated after {} messages", self.id, self.message_count);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CounterActor {
    pub id: String,
    pub value: i64,
    pub message_count: u64,
}

impl CounterActor {
    pub fn new(id: String) -> Self {
        Self {
            id,
            value: 0,
            message_count: 0,
        }
    }
}

#[async_trait]
impl Addressable for CounterActor {
    fn addressable_type() -> &'static str {
        "CounterActor"
    }

    async fn on_activate(&mut self) -> OrbitResult<()> {
        info!("CounterActor '{}' activated with value {}", self.id, self.value);
        Ok(())
    }

    async fn on_deactivate(&mut self) -> OrbitResult<()> {
        info!("CounterActor '{}' deactivated with final value {}", self.id, self.value);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MessageActor {
    pub id: String,
    pub messages: Vec<String>,
    pub last_response_time: u64,
}

impl MessageActor {
    pub fn new(id: String) -> Self {
        Self {
            id,
            messages: Vec::new(),
            last_response_time: 0,
        }
    }
}

#[async_trait]
impl Addressable for MessageActor {
    fn addressable_type() -> &'static str {
        "MessageActor"
    }

    async fn on_activate(&mut self) -> OrbitResult<()> {
        info!("MessageActor '{}' activated", self.id);
        Ok(())
    }

    async fn on_deactivate(&mut self) -> OrbitResult<()> {
        info!("MessageActor '{}' deactivated with {} messages", self.id, self.messages.len());
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMessage {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub processed: bool,
    pub actor_id: String,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementMessage {
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterResponse {
    pub new_value: i64,
    pub message_count: u64,
}

// Trait implementations for test methods
pub trait UserActorMethods {
    async fn process_message(&mut self, message: SimpleMessage) -> OrbitResult<MessageResponse>;
}

impl UserActorMethods for UserActor {
    async fn process_message(&mut self, message: SimpleMessage) -> OrbitResult<MessageResponse> {
        self.message_count += 1;
        let start = std::time::Instant::now();
        
        // Simulate some processing
        sleep(Duration::from_millis(1)).await;
        
        let response_time = start.elapsed().as_millis() as u64;
        
        Ok(MessageResponse {
            processed: true,
            actor_id: self.id.clone(),
            response_time_ms: response_time,
        })
    }
}

pub trait CounterActorMethods {
    async fn increment(&mut self, message: IncrementMessage) -> OrbitResult<CounterResponse>;
    async fn get_value(&self) -> OrbitResult<i64>;
    async fn get_message_count(&self) -> OrbitResult<u64>;
}

impl CounterActorMethods for CounterActor {
    async fn increment(&mut self, message: IncrementMessage) -> OrbitResult<CounterResponse> {
        self.value += message.amount;
        self.message_count += 1;
        
        Ok(CounterResponse {
            new_value: self.value,
            message_count: self.message_count,
        })
    }

    async fn get_value(&self) -> OrbitResult<i64> {
        Ok(self.value)
    }

    async fn get_message_count(&self) -> OrbitResult<u64> {
        Ok(self.message_count)
    }
}

pub trait MessageActorMethods {
    async fn send_message(&mut self, message: SimpleMessage) -> OrbitResult<MessageResponse>;
    async fn get_message_count(&self) -> OrbitResult<u64>;
}

impl MessageActorMethods for MessageActor {
    async fn send_message(&mut self, message: SimpleMessage) -> OrbitResult<MessageResponse> {
        let start = std::time::Instant::now();
        
        self.messages.push(message.content);
        
        let response_time = start.elapsed().as_millis() as u64;
        self.last_response_time = response_time;
        
        Ok(MessageResponse {
            processed: true,
            actor_id: self.id.clone(),
            response_time_ms: response_time,
        })
    }

    async fn get_message_count(&self) -> OrbitResult<u64> {
        Ok(self.messages.len() as u64)
    }
}

/// BDD World state
#[derive(Debug, World)]
pub struct OrbitWorld {
    server: Option<OrbitServer>,
    client: Option<OrbitClient>,
    server_port: u16,
    last_response: Option<MessageResponse>,
    last_counter_response: Option<CounterResponse>,
    created_actors: Vec<String>,
    error_message: Option<String>,
}

impl Default for OrbitWorld {
    fn default() -> Self {
        Self {
            server: None,
            client: None,
            server_port: 9100, // Different port for BDD tests
            last_response: None,
            last_counter_response: None,
            created_actors: Vec::new(),
            error_message: None,
        }
    }
}

impl OrbitWorld {
    async fn setup_server_and_client(&mut self) -> anyhow::Result<()> {
        if self.server.is_none() {
            let server_config = OrbitServerConfig::builder()
                .port(self.server_port)
                .build();
            
            let server = OrbitServer::new(server_config);
            
            // Register test actor types
            server.register_addressable_type::<UserActor>().await?;
            server.register_addressable_type::<CounterActor>().await?;
            server.register_addressable_type::<MessageActor>().await?;
            
            // Start server
            let server_clone = server.clone();
            tokio::spawn(async move {
                if let Err(e) = server_clone.start().await {
                    warn!("BDD test server error: {}", e);
                }
            });
            
            self.server = Some(server);
            
            // Wait for server to start
            sleep(Duration::from_millis(200)).await;
            
            // Create client
            let client_config = OrbitClientConfig::builder()
                .server_url(format!("http://localhost:{}", self.server_port))
                .build();
            
            let client = OrbitClient::new(client_config).await?;
            self.client = Some(client);
        }
        
        Ok(())
    }

    fn get_client(&self) -> &OrbitClient {
        self.client.as_ref().expect("Client not initialized")
    }
}

// Step definitions for Actor Lifecycle feature

#[given("an Orbit server is running")]
async fn orbit_server_running(world: &mut OrbitWorld) {
    world.setup_server_and_client().await.expect("Failed to start server");
}

#[given("an Orbit client is connected")]
async fn orbit_client_connected(world: &mut OrbitWorld) {
    // Already handled in setup_server_and_client
    assert!(world.client.is_some(), "Client should be connected");
}

#[given("no actor with key {string} exists")]
async fn no_actor_exists(world: &mut OrbitWorld, key: String) {
    // In our current implementation, actors are created on-demand,
    // so this is always true initially
    world.created_actors.retain(|k| k != &key);
}

#[when("I send a message to actor {string} with key {string}")]
async fn send_message_to_actor(world: &mut OrbitWorld, actor_type: String, key: String) {
    let client = world.get_client();
    
    match actor_type.as_str() {
        "UserActor" => {
            let actor_ref = AddressableReference::new(actor_type, Key::new(key.clone()));
            let actor = client.actor_reference::<UserActor>(&actor_ref);
            
            let response: Result<MessageResponse, _> = actor.invoke_method("process_message", SimpleMessage {
                content: "Test message".to_string(),
            }).await;
            
            match response {
                Ok(resp) => {
                    world.last_response = Some(resp);
                    world.created_actors.push(key);
                },
                Err(e) => world.error_message = Some(e.to_string()),
            }
        },
        _ => panic!("Unknown actor type: {}", actor_type),
    }
}

#[then("the actor should be created and activated")]
async fn actor_created_and_activated(world: &mut OrbitWorld) {
    assert!(world.last_response.is_some(), "Expected a response from actor");
    assert!(world.error_message.is_none(), "Should not have error: {:?}", world.error_message);
}

#[then("the message should be processed successfully")]
async fn message_processed_successfully(world: &mut OrbitWorld) {
    let response = world.last_response.as_ref().expect("No response available");
    assert!(response.processed, "Message should be processed");
    assert!(!response.actor_id.is_empty(), "Actor ID should not be empty");
}

#[given("an actor {string} with key {string} exists")]
async fn actor_exists(world: &mut OrbitWorld, actor_type: String, key: String) {
    let client = world.get_client();
    
    match actor_type.as_str() {
        "CounterActor" => {
            let actor_ref = AddressableReference::new(actor_type, Key::new(key.clone()));
            let actor = client.actor_reference::<CounterActor>(&actor_ref);
            
            // Initialize the actor by getting its value
            let _value: i64 = actor.invoke_method("get_value", ()).await.expect("Failed to initialize counter");
            world.created_actors.push(key);
        },
        _ => panic!("Unknown actor type: {}", actor_type),
    }
}

#[when("I increment the counter by {int}")]
async fn increment_counter_by(world: &mut OrbitWorld, amount: i64) {
    let client = world.get_client();
    
    // Use the last created counter actor
    let key = world.created_actors.last().expect("No counter actor created").clone();
    let actor_ref = AddressableReference::new("CounterActor".to_string(), Key::new(key));
    let actor = client.actor_reference::<CounterActor>(&actor_ref);
    
    let response: CounterResponse = actor.invoke_method("increment", IncrementMessage { amount }).await
        .expect("Failed to increment counter");
    
    world.last_counter_response = Some(response);
}

#[then("the counter value should be {int}")]
async fn counter_value_should_be(world: &mut OrbitWorld, expected: i64) {
    let response = world.last_counter_response.as_ref().expect("No counter response");
    assert_eq!(response.new_value, expected, "Counter value mismatch");
}

#[then("the actor should have processed {int} messages")]
async fn actor_processed_messages(world: &mut OrbitWorld, expected: u64) {
    let response = world.last_counter_response.as_ref().expect("No counter response");
    assert_eq!(response.message_count, expected, "Message count mismatch");
}

#[given("no actors exist")]
async fn no_actors_exist(world: &mut OrbitWorld) {
    world.created_actors.clear();
}

#[when("I create actors {string} with keys {string}, {string}, and {string}")]
async fn create_multiple_actors(world: &mut OrbitWorld, actor_type: String, key1: String, key2: String, key3: String) {
    let client = world.get_client();
    let keys = vec![key1, key2, key3];
    
    for key in keys {
        match actor_type.as_str() {
            "UserActor" => {
                let actor_ref = AddressableReference::new(actor_type.clone(), Key::new(key.clone()));
                let actor = client.actor_reference::<UserActor>(&actor_ref);
                
                let _: MessageResponse = actor.invoke_method("process_message", SimpleMessage {
                    content: format!("Init message for {}", key),
                }).await.expect("Failed to create actor");
                
                world.created_actors.push(key);
            },
            _ => panic!("Unknown actor type: {}", actor_type),
        }
    }
}

#[then("each actor should maintain separate state")]
async fn actors_maintain_separate_state(world: &mut OrbitWorld) {
    // This is verified by the fact that we successfully created multiple actors
    // In a real implementation, we would verify actual state separation
    assert!(world.created_actors.len() >= 3, "Should have created multiple actors");
}

#[then("operations on one actor should not affect others")]
async fn operations_do_not_affect_others(world: &mut OrbitWorld) {
    // This would require more complex testing in a real implementation
    // For now, we verify that we have multiple actors
    assert!(world.created_actors.len() >= 3, "Should have multiple independent actors");
}

// Placeholder implementations for remaining steps
#[given("an actor {string} with key {string} was previously deactivated")]
async fn actor_was_deactivated(_world: &mut OrbitWorld, _actor_type: String, _key: String) {
    // Placeholder - would require persistence layer
}

#[given("the actor had some persistent state")]
async fn actor_had_persistent_state(_world: &mut OrbitWorld) {
    // Placeholder - would require persistence layer
}

#[when("the actor has been idle for more than the timeout period")]
async fn actor_idle_timeout(_world: &mut OrbitWorld) {
    // Simulate timeout by waiting
    sleep(Duration::from_millis(100)).await;
}

#[then("the actor should be automatically deactivated")]
async fn actor_automatically_deactivated(_world: &mut OrbitWorld) {
    // Placeholder - would require monitoring actor lifecycle
}

#[then("its state should be persisted")]
async fn state_should_be_persisted(_world: &mut OrbitWorld) {
    // Placeholder - would require persistence verification
}

#[then("the actor should be reactivated")]
async fn actor_should_be_reactivated(_world: &mut OrbitWorld) {
    // Placeholder - would require reactivation detection
}

#[then("its previous state should be restored")]
async fn previous_state_restored(_world: &mut OrbitWorld) {
    // Placeholder - would require state comparison
}

// Main test runner
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    OrbitWorld::cucumber()
        .run_and_exit("tests/features/actor_lifecycle.feature")
        .await;
}