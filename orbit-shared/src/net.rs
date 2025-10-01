use crate::{addressable::AddressableReference, mesh::NodeId, router::Route};
use serde::{Deserialize, Serialize};

/// Reason for an invocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationReason {
    Invocation = 0,
    Rerouted = 1,
}

impl From<i32> for InvocationReason {
    fn from(value: i32) -> Self {
        match value {
            0 => InvocationReason::Invocation,
            1 => InvocationReason::Rerouted,
            _ => InvocationReason::Invocation, // Default fallback
        }
    }
}

impl From<InvocationReason> for i32 {
    fn from(reason: InvocationReason) -> Self {
        reason as i32
    }
}

/// Target for a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageTarget {
    Unicast { target_node: NodeId },
    RoutedUnicast { route: Route },
}

/// Content of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Error { 
        description: Option<String> 
    },
    ConnectionInfoRequest,
    ConnectionInfoResponse { 
        node_id: NodeId 
    },
    InvocationRequest {
        destination: AddressableReference,
        method: String,
        arguments: String, // JSON-serialized arguments
        reason: InvocationReason,
    },
    InvocationResponse { 
        data: String // JSON-serialized response
    },
    InvocationResponseError {
        description: Option<String>,
        platform: Option<String>,
    },
}

/// A message in the Orbit system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: MessageContent,
    pub message_id: Option<i64>,
    pub source: Option<NodeId>,
    pub target: Option<MessageTarget>,
    pub attempts: i64,
}

impl Message {
    pub fn new(content: MessageContent) -> Self {
        Self {
            content,
            message_id: None,
            source: None,
            target: None,
            attempts: 0,
        }
    }

    pub fn with_id(mut self, message_id: i64) -> Self {
        self.message_id = Some(message_id);
        self
    }

    pub fn with_source(mut self, source: NodeId) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_target(mut self, target: MessageTarget) -> Self {
        self.target = Some(target);
        self
    }

    /// Get the destination string for routing purposes
    pub fn destination(&self) -> String {
        match &self.content {
            MessageContent::InvocationRequest { destination, .. } => {
                destination.key.to_string()
            },
            MessageContent::InvocationResponse { .. } => {
                self.target
                    .as_ref()
                    .map(|t| match t {
                        MessageTarget::Unicast { target_node } => target_node.to_string(),
                        MessageTarget::RoutedUnicast { route } => {
                            route.hops.first()
                                .map(|node| node.to_string())
                                .unwrap_or_default()
                        }
                    })
                    .unwrap_or_default()
            },
            _ => String::new(),
        }
    }

    /// Increment the attempt counter
    pub fn increment_attempts(mut self) -> Self {
        self.attempts += 1;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{addressable::Key, mesh::NodeId};

    #[test]
    fn test_invocation_reason_conversion() {
        assert_eq!(InvocationReason::from(0), InvocationReason::Invocation);
        assert_eq!(InvocationReason::from(1), InvocationReason::Rerouted);
        assert_eq!(i32::from(InvocationReason::Invocation), 0);
        assert_eq!(i32::from(InvocationReason::Rerouted), 1);
    }

    #[test]
    fn test_message_creation() {
        let content = MessageContent::ConnectionInfoRequest;
        let message = Message::new(content)
            .with_id(123)
            .with_source(NodeId::generate("test".to_string()));

        assert_eq!(message.message_id, Some(123));
        assert!(message.source.is_some());
        assert_eq!(message.attempts, 0);
    }

    #[test]
    fn test_message_destination() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey { key: "test-key".to_string() },
        };

        let content = MessageContent::InvocationRequest {
            destination: reference,
            method: "test".to_string(),
            arguments: "{}".to_string(),
            reason: InvocationReason::Invocation,
        };

        let message = Message::new(content);
        assert_eq!(message.destination(), "test-key");
    }

    #[test]
    fn test_message_increment_attempts() {
        let message = Message::new(MessageContent::ConnectionInfoRequest)
            .increment_attempts()
            .increment_attempts();
        
        assert_eq!(message.attempts, 2);
    }
}