use crate::net::InvocationReason;
use serde::{Deserialize, Serialize};
use std::fmt;

pub type AddressableType = String;

/// A key that uniquely identifies an addressable within its type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    StringKey { key: String },
    Int32Key { key: i32 },
    Int64Key { key: i64 },
    NoKey,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::StringKey { key } => write!(f, "{}", key),
            Key::Int32Key { key } => write!(f, "{}", key),
            Key::Int64Key { key } => write!(f, "{}", key),
            Key::NoKey => write!(f, "no-key"),
        }
    }
}

/// Reference to an addressable (type + key)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddressableReference {
    pub addressable_type: AddressableType,
    pub key: Key,
}

impl fmt::Display for AddressableReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.addressable_type, self.key)
    }
}

/// Namespaced reference to an addressable
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespacedAddressableReference {
    pub namespace: String,
    pub addressable_reference: AddressableReference,
}

/// Arguments for an addressable invocation
/// Each argument is a tuple of (value, type_name) for serialization purposes
pub type AddressableInvocationArguments = Vec<AddressableInvocationArgument>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressableInvocationArgument {
    pub value: serde_json::Value,
    pub type_name: String,
}

/// An invocation of a method on an addressable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressableInvocation {
    /// A reference to the addressable
    pub reference: AddressableReference,
    /// The method being called
    pub method: String,
    /// The arguments being passed
    pub args: AddressableInvocationArguments,
    /// Reason the invocation call was sent
    pub reason: InvocationReason,
}

impl PartialEq for AddressableInvocation {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference
            && self.method == other.method
            && self.args.len() == other.args.len()
            && self
                .args
                .iter()
                .zip(&other.args)
                .all(|(a, b)| a.value == b.value && a.type_name == b.type_name)
    }
}

/// A lease for an addressable indicating its current location and lifetime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressableLease {
    pub reference: AddressableReference,
    pub node_id: crate::mesh::NodeId,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub renew_at: chrono::DateTime<chrono::Utc>,
}

/// Trait marker for addressable actors
pub trait Addressable: Send + Sync {
    fn addressable_type() -> &'static str
    where
        Self: Sized;
}

/// Trait for addressables with no key
pub trait ActorWithNoKey: Addressable {}

/// Trait for addressables with string keys
pub trait ActorWithStringKey: Addressable {}

/// Trait for addressables with i32 keys  
pub trait ActorWithInt32Key: Addressable {}

/// Trait for addressables with i64 keys
pub trait ActorWithInt64Key: Addressable {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_display() {
        assert_eq!(
            Key::StringKey {
                key: "test".to_string()
            }
            .to_string(),
            "test"
        );
        assert_eq!(Key::Int32Key { key: 42 }.to_string(), "42");
        assert_eq!(Key::Int64Key { key: 1000 }.to_string(), "1000");
        assert_eq!(Key::NoKey.to_string(), "no-key");
    }

    #[test]
    fn test_addressable_reference_display() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-key".to_string(),
            },
        };
        assert_eq!(reference.to_string(), "TestActor:test-key");
    }

    #[test]
    fn test_addressable_invocation_equality() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let invocation1 = AddressableInvocation {
            reference: reference.clone(),
            method: "test_method".to_string(),
            args: vec![AddressableInvocationArgument {
                value: serde_json::Value::String("test".to_string()),
                type_name: "String".to_string(),
            }],
            reason: InvocationReason::Invocation,
        };

        let invocation2 = AddressableInvocation {
            reference,
            method: "test_method".to_string(),
            args: vec![AddressableInvocationArgument {
                value: serde_json::Value::String("test".to_string()),
                type_name: "String".to_string(),
            }],
            reason: InvocationReason::Invocation,
        };

        assert_eq!(invocation1, invocation2);
    }
}
