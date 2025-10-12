//! Protocol buffer converters between Rust domain objects and protobuf messages

use crate::*;
use chrono::{DateTime, Utc};
use orbit_shared::*;
use prost_types::Timestamp;

/// Convert between Rust Key enum and KeyProto
pub struct KeyConverter;

impl KeyConverter {
    pub fn to_proto(key: &Key) -> KeyProto {
        let key_oneof = match key {
            Key::StringKey { key } => key_proto::Key::StringKey(key.clone()),
            Key::Int32Key { key } => key_proto::Key::Int32Key(*key),
            Key::Int64Key { key } => key_proto::Key::Int64Key(*key),
            Key::NoKey => key_proto::Key::NoKey(NoKeyProto {}),
        };
        KeyProto {
            key: Some(key_oneof),
        }
    }

    pub fn from_proto(proto: &KeyProto) -> OrbitResult<Key> {
        match &proto.key {
            Some(key_proto::Key::StringKey(k)) => Ok(Key::StringKey { key: k.clone() }),
            Some(key_proto::Key::Int32Key(k)) => Ok(Key::Int32Key { key: *k }),
            Some(key_proto::Key::Int64Key(k)) => Ok(Key::Int64Key { key: *k }),
            Some(key_proto::Key::NoKey(_)) => Ok(Key::NoKey),
            None => Err(OrbitError::internal("Missing key in KeyProto")),
        }
    }
}

/// Convert between Rust NodeId and NodeIdProto
pub struct NodeIdConverter;

impl NodeIdConverter {
    pub fn to_proto(node_id: &NodeId) -> NodeIdProto {
        NodeIdProto {
            key: node_id.key.clone(),
            namespace: node_id.namespace.clone(),
        }
    }

    pub fn from_proto(proto: &NodeIdProto) -> NodeId {
        NodeId {
            key: proto.key.clone(),
            namespace: proto.namespace.clone(),
        }
    }
}

/// Convert between Rust AddressableReference and AddressableReferenceProto
pub struct AddressableReferenceConverter;

impl AddressableReferenceConverter {
    pub fn to_proto(reference: &AddressableReference) -> AddressableReferenceProto {
        AddressableReferenceProto {
            addressable_type: reference.addressable_type.clone(),
            key: Some(KeyConverter::to_proto(&reference.key)),
        }
    }

    pub fn from_proto(proto: &AddressableReferenceProto) -> OrbitResult<AddressableReference> {
        let key = proto
            .key
            .as_ref()
            .ok_or_else(|| OrbitError::internal("Missing key in AddressableReferenceProto"))?;

        Ok(AddressableReference {
            addressable_type: proto.addressable_type.clone(),
            key: KeyConverter::from_proto(key)?,
        })
    }
}

/// Convert between Rust `DateTime<Utc>` and protobuf Timestamp
pub struct TimestampConverter;

impl TimestampConverter {
    pub fn to_proto(dt: &DateTime<Utc>) -> Timestamp {
        Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        }
    }

    pub fn from_proto(timestamp: &Timestamp) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32).unwrap_or_else(Utc::now)
    }
}

/// Convert between Rust InvocationReason and InvocationReasonProto
pub struct InvocationReasonConverter;

impl InvocationReasonConverter {
    pub fn to_proto(reason: &InvocationReason) -> InvocationReasonProto {
        match reason {
            InvocationReason::Invocation => InvocationReasonProto::Invocation,
            InvocationReason::Rerouted => InvocationReasonProto::Rerouted,
        }
    }

    pub fn from_proto(proto: InvocationReasonProto) -> InvocationReason {
        match proto {
            InvocationReasonProto::Invocation => InvocationReason::Invocation,
            InvocationReasonProto::Rerouted => InvocationReason::Rerouted,
        }
    }
}

/// Convert between Rust NodeStatus and NodeStatusProto
pub struct NodeStatusConverter;

impl NodeStatusConverter {
    pub fn to_proto(status: &NodeStatus) -> NodeStatusProto {
        match status {
            NodeStatus::Active => NodeStatusProto::Active,
            NodeStatus::Draining => NodeStatusProto::Draining,
            NodeStatus::Stopped => NodeStatusProto::Stopped,
        }
    }

    pub fn from_proto(proto: NodeStatusProto) -> NodeStatus {
        match proto {
            NodeStatusProto::Active => NodeStatus::Active,
            NodeStatusProto::Draining => NodeStatus::Draining,
            NodeStatusProto::Stopped => NodeStatus::Stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_converter_string_key() {
        let key = Key::StringKey {
            key: "test_key".to_string(),
        };

        let proto = KeyConverter::to_proto(&key);
        let converted_back = KeyConverter::from_proto(&proto).unwrap();

        assert_eq!(key, converted_back);

        // Verify proto structure
        match &proto.key {
            Some(key_proto::Key::StringKey(k)) => assert_eq!(k, "test_key"),
            _ => panic!("Expected StringKey variant"),
        }
    }

    #[test]
    fn test_key_converter_int32_key() {
        let key = Key::Int32Key { key: 12345 };

        let proto = KeyConverter::to_proto(&key);
        let converted_back = KeyConverter::from_proto(&proto).unwrap();

        assert_eq!(key, converted_back);

        match &proto.key {
            Some(key_proto::Key::Int32Key(k)) => assert_eq!(*k, 12345),
            _ => panic!("Expected Int32Key variant"),
        }
    }

    #[test]
    fn test_key_converter_int64_key() {
        let key = Key::Int64Key { key: 9876543210i64 };

        let proto = KeyConverter::to_proto(&key);
        let converted_back = KeyConverter::from_proto(&proto).unwrap();

        assert_eq!(key, converted_back);

        match &proto.key {
            Some(key_proto::Key::Int64Key(k)) => assert_eq!(*k, 9876543210i64),
            _ => panic!("Expected Int64Key variant"),
        }
    }

    #[test]
    fn test_key_converter_no_key() {
        let key = Key::NoKey;

        let proto = KeyConverter::to_proto(&key);
        let converted_back = KeyConverter::from_proto(&proto).unwrap();

        assert_eq!(key, converted_back);

        match &proto.key {
            Some(key_proto::Key::NoKey(_)) => {} // Expected
            _ => panic!("Expected NoKey variant"),
        }
    }

    #[test]
    fn test_key_converter_missing_key() {
        let proto = KeyProto { key: None };

        let result = KeyConverter::from_proto(&proto);
        assert!(result.is_err());

        match result.unwrap_err() {
            OrbitError::Internal(message) => {
                assert!(message.contains("Missing key in KeyProto"));
            }
            _ => panic!("Expected Internal error"),
        }
    }

    #[test]
    fn test_node_id_converter() {
        let node_id = NodeId {
            key: "node123".to_string(),
            namespace: "test_namespace".to_string(),
        };

        let proto = NodeIdConverter::to_proto(&node_id);
        let converted_back = NodeIdConverter::from_proto(&proto);

        assert_eq!(node_id.key, converted_back.key);
        assert_eq!(node_id.namespace, converted_back.namespace);

        // Verify proto fields
        assert_eq!(proto.key, "node123");
        assert_eq!(proto.namespace, "test_namespace");
    }

    #[test]
    fn test_node_id_converter_empty_fields() {
        let node_id = NodeId {
            key: String::new(),
            namespace: String::new(),
        };

        let proto = NodeIdConverter::to_proto(&node_id);
        let converted_back = NodeIdConverter::from_proto(&proto);

        assert_eq!(node_id, converted_back);
    }

    #[test]
    fn test_addressable_reference_converter() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test_instance".to_string(),
            },
        };

        let proto = AddressableReferenceConverter::to_proto(&reference);
        let converted_back = AddressableReferenceConverter::from_proto(&proto).unwrap();

        assert_eq!(reference, converted_back);

        // Verify proto structure
        assert_eq!(proto.addressable_type, "TestActor");
        assert!(proto.key.is_some());
    }

    #[test]
    fn test_addressable_reference_converter_different_key_types() {
        let test_cases = vec![
            Key::StringKey {
                key: "string_test".to_string(),
            },
            Key::Int32Key { key: 42 },
            Key::Int64Key { key: 123456789 },
            Key::NoKey,
        ];

        for key in test_cases {
            let reference = AddressableReference {
                addressable_type: "TestActor".to_string(),
                key,
            };

            let proto = AddressableReferenceConverter::to_proto(&reference);
            let converted_back = AddressableReferenceConverter::from_proto(&proto).unwrap();

            assert_eq!(reference, converted_back);
        }
    }

    #[test]
    fn test_addressable_reference_converter_missing_key() {
        let proto = AddressableReferenceProto {
            addressable_type: "TestActor".to_string(),
            key: None,
        };

        let result = AddressableReferenceConverter::from_proto(&proto);
        assert!(result.is_err());

        match result.unwrap_err() {
            OrbitError::Internal(message) => {
                assert!(message.contains("Missing key in AddressableReferenceProto"));
            }
            _ => panic!("Expected Internal error"),
        }
    }

    #[test]
    fn test_timestamp_converter() {
        let dt = Utc::now();

        let proto = TimestampConverter::to_proto(&dt);
        let converted_back = TimestampConverter::from_proto(&proto);

        // Allow for small differences due to precision
        let diff = (dt.timestamp_millis() - converted_back.timestamp_millis()).abs();
        assert!(diff < 1000, "Timestamp difference too large: {} ms", diff);
    }

    #[test]
    fn test_timestamp_converter_specific_datetime() {
        // Test with a specific known datetime
        let dt = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let proto = TimestampConverter::to_proto(&dt);
        let converted_back = TimestampConverter::from_proto(&proto);

        assert_eq!(dt.timestamp(), converted_back.timestamp());
        // Check nanoseconds separately due to potential precision differences
        let nano_diff = (dt.timestamp_subsec_nanos() as i64
            - converted_back.timestamp_subsec_nanos() as i64)
            .abs();
        assert!(
            nano_diff < 1_000_000,
            "Nanosecond precision difference too large"
        );
    }

    #[test]
    fn test_timestamp_converter_invalid_timestamp() {
        // Test with invalid timestamp (should fall back to current time)
        let invalid_proto = Timestamp {
            seconds: -1,
            nanos: -1,
        };

        let converted = TimestampConverter::from_proto(&invalid_proto);

        // Should not panic and should return a valid datetime
        let now = Utc::now();
        let diff = (now.timestamp() - converted.timestamp()).abs();
        assert!(
            diff < 10,
            "Fallback timestamp should be close to current time"
        );
    }

    #[test]
    fn test_invocation_reason_converter() {
        let test_cases = vec![
            (
                InvocationReason::Invocation,
                InvocationReasonProto::Invocation,
            ),
            (InvocationReason::Rerouted, InvocationReasonProto::Rerouted),
        ];

        for (reason, expected_proto) in test_cases {
            let proto = InvocationReasonConverter::to_proto(&reason);
            assert_eq!(proto, expected_proto);

            let converted_back = InvocationReasonConverter::from_proto(proto);
            assert_eq!(reason, converted_back);
        }
    }

    #[test]
    fn test_node_status_converter() {
        let test_cases = vec![
            (NodeStatus::Active, NodeStatusProto::Active),
            (NodeStatus::Draining, NodeStatusProto::Draining),
            (NodeStatus::Stopped, NodeStatusProto::Stopped),
        ];

        for (status, expected_proto) in test_cases {
            let proto = NodeStatusConverter::to_proto(&status);
            assert_eq!(proto, expected_proto);

            let converted_back = NodeStatusConverter::from_proto(proto);
            assert_eq!(status, converted_back);
        }
    }

    #[test]
    fn test_roundtrip_conversions() {
        // Test complex roundtrip with nested conversions
        let reference = AddressableReference {
            addressable_type: "ComplexActor".to_string(),
            key: Key::StringKey {
                key: "complex_test".to_string(),
            },
        };

        let node_id = NodeId {
            key: "complex_node".to_string(),
            namespace: "test_ns".to_string(),
        };

        let dt = Utc::now();
        let reason = InvocationReason::Rerouted;
        let status = NodeStatus::Draining;

        // Convert to proto
        let ref_proto = AddressableReferenceConverter::to_proto(&reference);
        let node_proto = NodeIdConverter::to_proto(&node_id);
        let dt_proto = TimestampConverter::to_proto(&dt);
        let reason_proto = InvocationReasonConverter::to_proto(&reason);
        let status_proto = NodeStatusConverter::to_proto(&status);

        // Convert back
        let ref_back = AddressableReferenceConverter::from_proto(&ref_proto).unwrap();
        let node_back = NodeIdConverter::from_proto(&node_proto);
        let dt_back = TimestampConverter::from_proto(&dt_proto);
        let reason_back = InvocationReasonConverter::from_proto(reason_proto);
        let status_back = NodeStatusConverter::from_proto(status_proto);

        // Verify all conversions
        assert_eq!(reference, ref_back);
        assert_eq!(node_id, node_back);
        assert_eq!(reason, reason_back);
        assert_eq!(status, status_back);

        // DateTime comparison with tolerance
        let dt_diff = (dt.timestamp_millis() - dt_back.timestamp_millis()).abs();
        assert!(dt_diff < 1000);
    }
}
