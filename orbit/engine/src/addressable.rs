use serde::{Deserialize, Serialize};
use std::fmt;

/// Reason for invoking an addressable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationReason {
    /// Direct invocation
    Invocation = 0,
    /// Rerouted invocation
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
            Key::StringKey { key } => write!(f, "{key}"),
            Key::Int32Key { key } => write!(f, "{key}"),
            Key::Int64Key { key } => write!(f, "{key}"),
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
    pub node_id: crate::cluster::NodeId,
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
    use crate::cluster::NodeId;
    use chrono::Utc;

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
    fn test_key_equality() {
        let key1 = Key::StringKey {
            key: "test".to_string(),
        };
        let key2 = Key::StringKey {
            key: "test".to_string(),
        };
        let key3 = Key::StringKey {
            key: "other".to_string(),
        };

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key1, Key::NoKey);
        assert_ne!(Key::Int32Key { key: 42 }, Key::Int64Key { key: 42 });
    }

    #[test]
    fn test_key_serialization() {
        let keys = vec![
            Key::StringKey {
                key: "test".to_string(),
            },
            Key::Int32Key { key: 42 },
            Key::Int64Key { key: 1000 },
            Key::NoKey,
        ];

        for key in keys {
            let serialized = serde_json::to_string(&key).unwrap();
            let deserialized: Key = serde_json::from_str(&serialized).unwrap();
            assert_eq!(key, deserialized);
        }
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

        let reference_nokey = AddressableReference {
            addressable_type: "SingletonActor".to_string(),
            key: Key::NoKey,
        };
        assert_eq!(reference_nokey.to_string(), "SingletonActor:no-key");
    }

    #[test]
    fn test_addressable_reference_equality_and_hash() {
        use std::collections::HashSet;

        let ref1 = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "key1".to_string(),
            },
        };

        let ref2 = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "key1".to_string(),
            },
        };

        let ref3 = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "key2".to_string(),
            },
        };

        assert_eq!(ref1, ref2);
        assert_ne!(ref1, ref3);

        let mut set = HashSet::new();
        set.insert(ref1.clone());
        assert!(set.contains(&ref2));
        assert!(!set.contains(&ref3));
    }

    #[test]
    fn test_namespaced_addressable_reference() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let namespaced = NamespacedAddressableReference {
            namespace: "production".to_string(),
            addressable_reference: reference,
        };

        assert_eq!(namespaced.namespace, "production");
        assert_eq!(
            namespaced.addressable_reference.addressable_type,
            "TestActor"
        );
    }

    #[test]
    fn test_addressable_invocation_argument() {
        let arg = AddressableInvocationArgument {
            value: serde_json::Value::String("test_value".to_string()),
            type_name: "String".to_string(),
        };

        let serialized = serde_json::to_string(&arg).unwrap();
        let deserialized: AddressableInvocationArgument =
            serde_json::from_str(&serialized).unwrap();

        match (&arg.value, &deserialized.value) {
            (serde_json::Value::String(a), serde_json::Value::String(b)) => assert_eq!(a, b),
            _ => panic!("Value mismatch"),
        }
        assert_eq!(arg.type_name, deserialized.type_name);
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

    #[test]
    fn test_addressable_invocation_inequality() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let invocation1 = AddressableInvocation {
            reference: reference.clone(),
            method: "method1".to_string(),
            args: vec![],
            reason: InvocationReason::Invocation,
        };

        let invocation2 = AddressableInvocation {
            reference,
            method: "method2".to_string(), // Different method
            args: vec![],
            reason: InvocationReason::Invocation,
        };

        assert_ne!(invocation1, invocation2);
    }

    #[test]
    fn test_addressable_invocation_complex_args() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::Int32Key { key: 42 },
        };

        let complex_args = vec![
            AddressableInvocationArgument {
                value: serde_json::json!({"key": "value", "number": 123}),
                type_name: "ComplexType".to_string(),
            },
            AddressableInvocationArgument {
                value: serde_json::json!([1, 2, 3, 4]),
                type_name: "Vec<i32>".to_string(),
            },
        ];

        let invocation = AddressableInvocation {
            reference,
            method: "complex_method".to_string(),
            args: complex_args,
            reason: InvocationReason::Invocation,
        };

        // Test serialization
        let serialized = serde_json::to_string(&invocation).unwrap();
        let deserialized: AddressableInvocation = serde_json::from_str(&serialized).unwrap();

        assert_eq!(invocation, deserialized);
    }

    #[test]
    fn test_addressable_lease() {
        let now = Utc::now();
        let expires = now + chrono::Duration::minutes(30);
        let renew = now + chrono::Duration::minutes(25);

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let lease = AddressableLease {
            reference: reference.clone(),
            node_id: NodeId::new("node-1".to_string(), "default".to_string()),
            expires_at: expires,
            renew_at: renew,
        };

        assert_eq!(lease.reference, reference);
        assert_eq!(lease.node_id.key, "node-1");
        assert!(lease.renew_at < lease.expires_at);

        // Test serialization
        let serialized = serde_json::to_string(&lease).unwrap();
        let deserialized: AddressableLease = serde_json::from_str(&serialized).unwrap();

        assert_eq!(lease.reference, deserialized.reference);
        assert_eq!(lease.node_id, deserialized.node_id);
    }

    #[test]
    fn test_different_key_types_in_reference() {
        let string_ref = AddressableReference {
            addressable_type: "StringActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let int32_ref = AddressableReference {
            addressable_type: "Int32Actor".to_string(),
            key: Key::Int32Key { key: 42 },
        };

        let int64_ref = AddressableReference {
            addressable_type: "Int64Actor".to_string(),
            key: Key::Int64Key {
                key: 9223372036854775807,
            }, // i64::MAX
        };

        let no_key_ref = AddressableReference {
            addressable_type: "SingletonActor".to_string(),
            key: Key::NoKey,
        };

        // Test that all references work correctly
        assert_eq!(string_ref.to_string(), "StringActor:test");
        assert_eq!(int32_ref.to_string(), "Int32Actor:42");
        assert_eq!(int64_ref.to_string(), "Int64Actor:9223372036854775807");
        assert_eq!(no_key_ref.to_string(), "SingletonActor:no-key");

        // Test that they're all different
        assert_ne!(string_ref, int32_ref);
        assert_ne!(int32_ref, int64_ref);
        assert_ne!(int64_ref, no_key_ref);
    }

    // Test trait marker behaviors
    struct TestActor;

    impl Addressable for TestActor {
        fn addressable_type() -> &'static str {
            "TestActor"
        }
    }

    impl ActorWithStringKey for TestActor {}

    #[test]
    fn test_addressable_trait() {
        assert_eq!(TestActor::addressable_type(), "TestActor");
    }
}
