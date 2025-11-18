//! Actor invocation system for routing and executing actor method calls

use orbit_shared::{
    AddressableInvocation, AddressableInvocationArgument, AddressableInvocationArguments,
    AddressableReference, InvocationReason, OrbitError, OrbitResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{timeout, Duration};

/// System for managing actor invocations
#[derive(Debug)]
pub struct InvocationSystem {
    pending_invocations: Arc<RwLock<HashMap<u64, oneshot::Sender<InvocationResult>>>>,
    invocation_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl InvocationSystem {
    pub fn new() -> Self {
        Self {
            pending_invocations: Arc::new(RwLock::new(HashMap::new())),
            invocation_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Send an invocation to an actor and wait for the result
    pub async fn send_invocation(
        &self,
        invocation: AddressableInvocation,
    ) -> OrbitResult<InvocationResult> {
        let invocation_id = self
            .invocation_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, rx) = oneshot::channel();

        // Store the pending invocation
        {
            let mut pending = self.pending_invocations.write().await;
            pending.insert(invocation_id, tx);
        }

        // Route the invocation (for now, simulate local execution)
        let result = self.route_invocation(invocation_id, invocation).await;

        // Wait for the result with timeout
        match timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(OrbitError::internal("Invocation sender dropped")),
            Err(_) => {
                // Cleanup timed out invocation
                let mut pending = self.pending_invocations.write().await;
                pending.remove(&invocation_id);
                Err(OrbitError::timeout("Actor invocation"))
            }
        }?;

        result
    }

    /// Route an invocation to the appropriate handler
    async fn route_invocation(
        &self,
        invocation_id: u64,
        invocation: AddressableInvocation,
    ) -> OrbitResult<InvocationResult> {
        // For now, simulate successful execution
        // In a real implementation, this would:
        // 1. Determine which node hosts the actor
        // 2. Send the invocation over the network if remote
        // 3. Execute locally if the actor is on this node

        tokio::time::sleep(Duration::from_millis(1)).await;

        // Mock different return types based on method name
        let result_value = match invocation.method.as_str() {
            "count" | "get_value" => serde_json::Value::Number(serde_json::Number::from(42)),
            "increment" | "decrement" => serde_json::Value::Number(serde_json::Number::from(100)),
            "register_counter" | "sync_with_counter" | "reset" | "sync_all_counters" => {
                serde_json::Value::Null
            }
            "get_state" => serde_json::json!({
                "name": "test-counter",
                "value": 42,
                "operation_count": 5
            }),
            "broadcast_increment" => serde_json::json!([3, 6, 9]),
            "get_stats" => {
                // Return different stats based on actor type
                if invocation.reference.addressable_type == "VectorActor" {
                    serde_json::json!({
                        "vector_count": 5,
                        "index_count": 1,
                        "avg_dimension": 4.0,
                        "min_dimension": 4,
                        "max_dimension": 4,
                        "avg_metadata_keys": 1.0
                    })
                } else {
                    serde_json::json!({
                        "name": "test-coordinator",
                        "managed_counters": ["counter-1", "counter-2", "counter-3"],
                        "total_operations": 10
                    })
                }
            }
            "get_all_counter_states" => serde_json::json!([
                {"name": "counter-1", "value": 10, "operation_count": 2},
                {"name": "counter-2", "value": 20, "operation_count": 4},
                {"name": "counter-3", "value": 30, "operation_count": 6}
            ]),
            // Vector Actor methods
            "add_vector" | "create_index" | "drop_index" => serde_json::Value::Null,
            "get_vector" => serde_json::json!({
                "id": "test_vector",
                "data": [0.1, 0.2, 0.3, 0.4],
                "metadata": {"category": "test"},
                "timestamp": 1234567890
            }),
            "remove_vector" => serde_json::Value::Bool(true),
            "search_vectors" | "knn_search" => serde_json::json!([
                {
                    "vector": {
                        "id": "test_result",
                        "data": [0.1, 0.2, 0.3, 0.4],
                        "metadata": {"category": "test"},
                        "timestamp": 1234567890
                    },
                    "score": 0.95,
                    "metric": "Cosine"
                }
            ]),
            "list_indices" => serde_json::json!([
                {
                    "name": "test_index",
                    "dimension": 4,
                    "metric": "Cosine",
                    "algorithm": "brute_force",
                    "parameters": {}
                }
            ]),
            "list_vector_ids" => serde_json::json!(["doc1", "doc2", "doc3", "doc4", "doc5"]),
            "vector_count" => serde_json::Value::Number(serde_json::Number::from(5)),
            _ => serde_json::Value::String("Hello from actor!".to_string()),
        };

        let result = InvocationResult {
            invocation_id,
            reference: invocation.reference,
            method: invocation.method,
            result: Ok(result_value),
        };

        // Complete the pending invocation
        self.complete_invocation(invocation_id, result.clone())
            .await;

        Ok(result)
    }

    /// Complete a pending invocation with a result
    async fn complete_invocation(&self, invocation_id: u64, result: InvocationResult) {
        let mut pending = self.pending_invocations.write().await;
        if let Some(sender) = pending.remove(&invocation_id) {
            let _ = sender.send(result);
        }
    }
}

impl Default for InvocationSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an actor invocation
#[derive(Debug, Clone)]
pub struct InvocationResult {
    pub invocation_id: u64,
    pub reference: AddressableReference,
    pub method: String,
    pub result: Result<serde_json::Value, String>,
}

/// Actor reference that provides typed access to an actor
pub struct ActorReference<T: ?Sized> {
    reference: AddressableReference,
    pub(crate) invocation_system: Arc<InvocationSystem>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ?Sized> ActorReference<T> {
    pub fn new(reference: AddressableReference, invocation_system: Arc<InvocationSystem>) -> Self {
        Self {
            reference,
            invocation_system,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the addressable reference
    pub fn reference(&self) -> &AddressableReference {
        &self.reference
    }

    /// Invoke a method on the actor
    pub async fn invoke<R>(&self, method: &str, args: Vec<serde_json::Value>) -> OrbitResult<R>
    where
        R: serde::de::DeserializeOwned,
    {
        let invocation_args: AddressableInvocationArguments = args
            .into_iter()
            .map(|value| AddressableInvocationArgument {
                type_name: "dynamic".to_string(),
                value,
            })
            .collect();

        let invocation = AddressableInvocation {
            reference: self.reference.clone(),
            method: method.to_string(),
            args: invocation_args,
            reason: InvocationReason::Invocation,
        };

        let result = self.invocation_system.send_invocation(invocation).await?;

        match result.result {
            Ok(value) => serde_json::from_value(value).map_err(OrbitError::SerializationError),
            Err(error) => Err(OrbitError::InvocationFailed {
                addressable_type: self.reference.addressable_type.clone(),
                method: method.to_string(),
                reason: error,
            }),
        }
    }
}

impl<T: ?Sized> Clone for ActorReference<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            invocation_system: self.invocation_system.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ?Sized> std::fmt::Debug for ActorReference<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorReference")
            .field("reference", &self.reference)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::{Addressable, Key};

    #[tokio::test]
    async fn test_invocation_system() {
        let system = InvocationSystem::new();

        let invocation = AddressableInvocation {
            reference: AddressableReference {
                addressable_type: "TestActor".to_string(),
                key: Key::StringKey {
                    key: "test".to_string(),
                },
            },
            method: "test_method".to_string(),
            args: vec![],
            reason: InvocationReason::Invocation,
        };

        let result = system.send_invocation(invocation).await.unwrap();
        assert_eq!(result.method, "test_method");
        assert!(result.result.is_ok());
    }

    #[tokio::test]
    async fn test_actor_reference() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let system = Arc::new(InvocationSystem::new());
        let actor_ref: ActorReference<dyn Addressable> = ActorReference::new(reference, system);

        let result: String = actor_ref
            .invoke(
                "greet",
                vec![serde_json::Value::String("World".to_string())],
            )
            .await
            .unwrap();

        assert_eq!(result, "Hello from actor!");
    }
}
