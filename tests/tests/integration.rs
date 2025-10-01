use async_trait::async_trait;
use orbit_client::{OrbitClient, OrbitClientConfig};
use orbit_server::{OrbitServer, OrbitServerConfig};
use orbit_shared::{
    addressable::{Addressable, AddressableReference, Key},
    exception::{OrbitError, OrbitResult},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_test;
use tracing::{info, warn};

/// Test actor for integration tests
#[derive(Debug, Clone)]
pub struct TestActor {
    pub id: String,
    pub state: i32,
    pub messages_received: u64,
}

impl TestActor {
    pub fn new(id: String) -> Self {
        Self {
            id,
            state: 0,
            messages_received: 0,
        }
    }
}

#[async_trait]
impl Addressable for TestActor {
    fn addressable_type() -> &'static str {
        "TestActor"
    }

    async fn on_activate(&mut self) -> OrbitResult<()> {
        info!("TestActor '{}' activated", self.id);
        Ok(())
    }

    async fn on_deactivate(&mut self) -> OrbitResult<()> {
        info!("TestActor '{}' deactivated with {} messages received", self.id, self.messages_received);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMessage {
    pub content: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResponse {
    pub actor_id: String,
    pub processed_content: String,
    pub new_state: i32,
    pub total_messages: u64,
}

pub trait TestActorMethods {
    async fn process_message(&mut self, message: TestMessage) -> OrbitResult<TestResponse>;
    async fn get_state(&self) -> OrbitResult<i32>;
    async fn set_state(&mut self, new_state: i32) -> OrbitResult<()>;
    async fn get_message_count(&self) -> OrbitResult<u64>;
    async fn reset(&mut self) -> OrbitResult<()>;
}

impl TestActorMethods for TestActor {
    async fn process_message(&mut self, message: TestMessage) -> OrbitResult<TestResponse> {
        self.messages_received += 1;
        self.state += message.value;
        
        let response = TestResponse {
            actor_id: self.id.clone(),
            processed_content: format!("Processed: {}", message.content),
            new_state: self.state,
            total_messages: self.messages_received,
        };
        
        info!("TestActor '{}' processed message: {:?}", self.id, message);
        Ok(response)
    }

    async fn get_state(&self) -> OrbitResult<i32> {
        Ok(self.state)
    }

    async fn set_state(&mut self, new_state: i32) -> OrbitResult<()> {
        self.state = new_state;
        Ok(())
    }

    async fn get_message_count(&self) -> OrbitResult<u64> {
        Ok(self.messages_received)
    }

    async fn reset(&mut self) -> OrbitResult<()> {
        self.state = 0;
        self.messages_received = 0;
        Ok(())
    }
}

/// Test setup helper
pub struct TestSetup {
    pub server: OrbitServer,
    pub client: OrbitClient,
    pub server_port: u16,
}

impl TestSetup {
    pub async fn new() -> anyhow::Result<Self> {
        let server_port = 9000; // Use different port for tests
        
        let server_config = OrbitServerConfig::builder()
            .port(server_port)
            .build();
        
        let server = OrbitServer::new(server_config);
        server.register_addressable_type::<TestActor>().await?;
        
        // Start server in background
        let server_clone = server.clone();
        tokio::spawn(async move {
            if let Err(e) = server_clone.start().await {
                warn!("Test server error: {}", e);
            }
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(300)).await;

        let client_config = OrbitClientConfig::builder()
            .server_url(format!("http://localhost:{}", server_port))
            .build();
        
        let client = OrbitClient::new(client_config).await?;
        
        Ok(Self {
            server,
            client,
            server_port,
        })
    }

    pub async fn cleanup(&self) -> anyhow::Result<()> {
        self.client.shutdown().await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_basic_actor_creation_and_messaging() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    // Create actor reference
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("test-actor-1".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Test basic message processing
    let response: TestResponse = actor.invoke_method("process_message", TestMessage {
        content: "Hello World".to_string(),
        value: 5,
    }).await?;
    
    assert_eq!(response.new_state, 5);
    assert_eq!(response.total_messages, 1);
    assert_eq!(response.processed_content, "Processed: Hello World");
    
    // Test state queries
    let state: i32 = actor.invoke_method("get_state", ()).await?;
    assert_eq!(state, 5);
    
    let count: u64 = actor.invoke_method("get_message_count", ()).await?;
    assert_eq!(count, 1);
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_multiple_actors() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    // Create multiple actor references
    let actor1_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("test-actor-1".to_string()),
    );
    let actor2_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("test-actor-2".to_string()),
    );
    
    let actor1 = setup.client.actor_reference::<TestActor>(&actor1_ref);
    let actor2 = setup.client.actor_reference::<TestActor>(&actor2_ref);
    
    // Send different messages to each actor
    let response1: TestResponse = actor1.invoke_method("process_message", TestMessage {
        content: "Message to Actor 1".to_string(),
        value: 10,
    }).await?;
    
    let response2: TestResponse = actor2.invoke_method("process_message", TestMessage {
        content: "Message to Actor 2".to_string(),
        value: 20,
    }).await?;
    
    assert_eq!(response1.new_state, 10);
    assert_eq!(response2.new_state, 20);
    
    // Verify actors maintain separate state
    let state1: i32 = actor1.invoke_method("get_state", ()).await?;
    let state2: i32 = actor2.invoke_method("get_state", ()).await?;
    
    assert_eq!(state1, 10);
    assert_eq!(state2, 20);
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_actor_state_persistence() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("persistent-actor".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Send multiple messages to build up state
    for i in 1..=5 {
        let _: TestResponse = actor.invoke_method("process_message", TestMessage {
            content: format!("Message {}", i),
            value: i,
        }).await?;
    }
    
    // Verify cumulative state
    let final_state: i32 = actor.invoke_method("get_state", ()).await?;
    let expected_state = (1..=5).sum::<i32>(); // 1+2+3+4+5 = 15
    assert_eq!(final_state, expected_state);
    
    let message_count: u64 = actor.invoke_method("get_message_count", ()).await?;
    assert_eq!(message_count, 5);
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_actor_deactivation() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("deactivation-test".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Interact with actor
    let _: TestResponse = actor.invoke_method("process_message", TestMessage {
        content: "Test message".to_string(),
        value: 42,
    }).await?;
    
    // Explicitly deactivate actor
    setup.client.deactivate_actor(&actor_ref).await?;
    
    // Actor should be deactivated successfully
    // Note: In a full implementation, we might test reactivation here
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_concurrent_operations() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("concurrent-test".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Send multiple concurrent messages
    let mut tasks = Vec::new();
    for i in 1..=10 {
        let actor_clone = actor.clone();
        let task = tokio::spawn(async move {
            let response: TestResponse = actor_clone.invoke_method("process_message", TestMessage {
                content: format!("Concurrent message {}", i),
                value: i,
            }).await.unwrap();
            response
        });
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let mut responses = Vec::new();
    for task in tasks {
        let response = task.await?;
        responses.push(response);
    }
    
    // Verify all messages were processed
    assert_eq!(responses.len(), 10);
    
    let final_message_count: u64 = actor.invoke_method("get_message_count", ()).await?;
    assert_eq!(final_message_count, 10);
    
    let final_state: i32 = actor.invoke_method("get_state", ()).await?;
    let expected_state = (1..=10).sum::<i32>(); // 55
    assert_eq!(final_state, expected_state);
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_client_server_connection() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    // Test that client is properly connected
    // This is verified by the fact that we can create actor references
    // and perform operations
    
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("connection-test".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Simple operation to verify connection
    let state: i32 = actor.invoke_method("get_state", ()).await?;
    assert_eq!(state, 0); // Initial state
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = TestSetup::new().await?;
    
    let actor_ref = AddressableReference::new(
        "TestActor".to_string(),
        Key::new("error-test".to_string()),
    );
    
    let actor = setup.client.actor_reference::<TestActor>(&actor_ref);
    
    // Test normal operation first
    let _: TestResponse = actor.invoke_method("process_message", TestMessage {
        content: "Valid message".to_string(),
        value: 1,
    }).await?;
    
    // TODO: Add tests for error conditions when we have proper error handling
    // For now, just verify that basic operations work
    
    setup.cleanup().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Run all integration tests
    println!("Running Orbit Integration Tests");
    
    test_basic_actor_creation_and_messaging().await?;
    println!("✓ Basic actor creation and messaging");
    
    test_multiple_actors().await?;
    println!("✓ Multiple actors");
    
    test_actor_state_persistence().await?;
    println!("✓ Actor state persistence");
    
    test_actor_deactivation().await?;
    println!("✓ Actor deactivation");
    
    test_concurrent_operations().await?;
    println!("✓ Concurrent operations");
    
    test_client_server_connection().await?;
    println!("✓ Client-server connection");
    
    test_error_handling().await?;
    println!("✓ Error handling");
    
    println!("\nAll integration tests passed! ✅");
    Ok(())
}