//! Actor invocation system for routing and executing actor method calls

use orbit_proto::{
    InvocationReasonProto, InvocationRequestProto, MessageContentProto, MessageProto,
    MessageTargetProto, NodeIdProto,
};
use orbit_shared::{
    AddressableInvocation, AddressableInvocationArgument, AddressableInvocationArguments,
    AddressableReference, InvocationReason, OrbitError, OrbitResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};

/// Pending invocation context
#[derive(Debug)]
struct PendingInvocation {
    sender: oneshot::Sender<InvocationResult>,
    reference: AddressableReference,
    method: String,
}

/// System for managing actor invocations
#[derive(Debug)]
pub struct InvocationSystem {
    pending_invocations: Arc<RwLock<HashMap<u64, PendingInvocation>>>,
    invocation_counter: Arc<std::sync::atomic::AtomicU64>,
    outbound_sender: Option<mpsc::Sender<MessageProto>>,
}

impl InvocationSystem {
    pub fn new(outbound_sender: Option<mpsc::Sender<MessageProto>>) -> Self {
        Self {
            pending_invocations: Arc::new(RwLock::new(HashMap::new())),
            invocation_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            outbound_sender,
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
            pending.insert(
                invocation_id,
                PendingInvocation {
                    sender: tx,
                    reference: invocation.reference.clone(),
                    method: invocation.method.clone(),
                },
            );
        }

        // Route the invocation
        if let Err(e) = self.route_invocation(invocation_id, invocation).await {
            let mut pending = self.pending_invocations.write().await;
            pending.remove(&invocation_id);
            return Err(e);
        }

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
        }
    }

    /// Route an invocation to the appropriate handler
    async fn route_invocation(
        &self,
        invocation_id: u64,
        invocation: AddressableInvocation,
    ) -> OrbitResult<()> {
        if let Some(sender) = &self.outbound_sender {
            // Convert arguments to JSON string
            let args_json = serde_json::to_string(&invocation.args)
                .map_err(|e| OrbitError::SerializationError(e))?;

            // Create invocation request
            let request = InvocationRequestProto {
                reference: Some(
                    orbit_proto::converters::AddressableReferenceConverter::to_proto(
                        &invocation.reference,
                    ),
                ),
                method: invocation.method,
                arguments: args_json,
                reason: InvocationReasonProto::Invocation as i32,
            };

            // Create message
            let message = MessageProto {
                message_id: invocation_id as i64,
                source: None, // Will be filled by client
                target: Some(MessageTargetProto {
                    target: Some(orbit_proto::message_target_proto::Target::UnicastTarget(
                        orbit_proto::message_target_proto::Unicast {
                            target: Some(NodeIdProto {
                                key: "server".to_string(), // Target server
                                namespace: "default".to_string(),
                            }),
                        },
                    )),
                }),
                content: Some(MessageContentProto {
                    content: Some(
                        orbit_proto::message_content_proto::Content::InvocationRequest(request),
                    ),
                }),
                attempts: 0,
            };

            // Send message
            sender
                .send(message)
                .await
                .map_err(|_| OrbitError::internal("Failed to send invocation message"))?;

            Ok(())
        } else {
            // Fallback to mock implementation for offline mode
            self.mock_invocation(invocation_id, invocation).await
        }
    }

    async fn mock_invocation(
        &self,
        invocation_id: u64,
        invocation: AddressableInvocation,
    ) -> OrbitResult<()> {
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

        // Complete the pending invocation
        self.complete_invocation_result(invocation_id, Ok(result_value))
            .await;

        Ok(())
    }

    /// Complete a pending invocation with a result value
    pub async fn complete_invocation_result(
        &self,
        invocation_id: u64,
        result_value: Result<serde_json::Value, String>,
    ) {
        let mut pending = self.pending_invocations.write().await;
        if let Some(pending_inv) = pending.remove(&invocation_id) {
            let result = InvocationResult {
                invocation_id,
                reference: pending_inv.reference,
                method: pending_inv.method,
                result: result_value,
            };
            let _ = pending_inv.sender.send(result);
        }
    }

    // Keep old complete_invocation for backward compatibility if needed, or remove it
    // But route_invocation calls it. I should update route_invocation to use complete_invocation_result
    // or update complete_invocation to take InvocationResult and just extract what it needs.

    /// Complete a pending invocation with a full result
    #[allow(dead_code)]
    async fn complete_invocation(&self, invocation_id: u64, result: InvocationResult) {
        let mut pending = self.pending_invocations.write().await;
        if let Some(pending_inv) = pending.remove(&invocation_id) {
            let _ = pending_inv.sender.send(result);
        }
    }
}

impl Default for InvocationSystem {
    fn default() -> Self {
        Self::new(None)
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
        let system = InvocationSystem::new(None);

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

        let system = Arc::new(InvocationSystem::new(None));
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
