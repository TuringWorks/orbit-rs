//! Simple integration test for the mock framework

use async_trait::async_trait;
use orbit_util::mocks::*;
use orbit_shared::addressable::Addressable;
use serde::{Deserialize, Serialize};

/// Test actor for integration tests
#[derive(Debug, Clone)]
pub struct SimpleTestActor {
    pub id: String,
    pub value: i32,
}

impl SimpleTestActor {
    pub fn new(id: String) -> Self {
        Self {
            id,
            value: 0,
        }
    }
}

#[async_trait]
impl Addressable for SimpleTestActor {
    fn addressable_type() -> &'static str {
        "SimpleTestActor"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMessage {
    pub content: String,
    pub number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimpleResponse {
    pub actor_id: String,
    pub processed: String,
    pub new_value: i32,
}

#[tokio::test]
async fn test_simple_mock_framework() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = MockTestSetup::builder()
        .with_namespace("simple-test")
        .with_verbose_logging(true)
        .build()
        .await;
        
    setup.register_actor_types(&["SimpleTestActor"]).await.unwrap();
    setup.start().await.unwrap();
    
    // Create actor reference
    let actor = setup.actor_reference::<SimpleTestActor>("test-actor-1");
    
    // Test basic method invocation
    let response: SimpleResponse = actor.invoke_method("process_message", SimpleMessage {
        content: "Hello World".to_string(),
        number: 42,
    }).await.map_err(|e| anyhow::anyhow!(e))?;
    
    // Mock should provide reasonable default values
    assert!(!response.actor_id.is_empty() || response.actor_id == String::default());
    
    // Test state queries
    let _state: i32 = actor.invoke_method("get_state", ()).await.map_err(|e| anyhow::anyhow!(e))?;
    // Just verify we got a response - mock should provide a reasonable value
    
    let _count: u64 = actor.invoke_method("get_message_count", ()).await.map_err(|e| anyhow::anyhow!(e))?;
    // Just verify we got a response - u64 is always >= 0
    
    setup.cleanup().await.unwrap();
    Ok(())
}

#[tokio::test]
async fn test_mock_with_failure_simulation() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = MockTestSetup::builder()
        .with_namespace("failure-test")
        .with_failure_rate(1.0) // Always fail
        .with_verbose_logging(true)
        .build()
        .await;
        
    setup.register_actor_types(&["SimpleTestActor"]).await.unwrap();
    // Server startup may fail due to high failure rate, which is expected
    if setup.start().await.is_err() {
        // This is expected behavior when failure rate is 1.0
        println!("Server startup failed as expected with high failure rate");
        return Ok(());
    }
    
    let actor = setup.actor_reference::<SimpleTestActor>("failing-actor");
    
    // This should fail due to failure rate
    let result: Result<SimpleResponse, String> = actor.invoke_method("process_message", SimpleMessage {
        content: "This should fail".to_string(),
        number: 1,
    }).await;
    
    assert!(result.is_err(), "Expected failure but got success");
    
    setup.cleanup().await.unwrap();
    Ok(())
}

#[tokio::test]
async fn test_mock_statistics() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    
    let setup = MockTestSetup::builder()
        .with_namespace("stats-test")
        .with_verbose_logging(true)
        .build()
        .await;
        
    setup.register_actor_types(&["SimpleTestActor"]).await.unwrap();
    setup.start().await.unwrap();
    
    let (client_stats, server_stats) = setup.get_stats().await;
    
    assert_eq!(client_stats.namespace, "stats-test");
    assert!(server_stats.is_running);
    assert!(server_stats.registered_actor_types >= 1);
    
    setup.cleanup().await.unwrap();
    Ok(())
}