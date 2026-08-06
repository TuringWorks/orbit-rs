//! Protocol buffer conversions between Rust domain objects and protobuf messages.
//!
//! Conversions are expressed as [`From`]/[`TryFrom`] implementations so they compose
//! with `?`, `.into()`, and iterator adapters. The `*Converter` structs are thin,
//! stable wrappers kept for call sites that prefer a named function.
//!
//! Fallible directions (`proto -> domain`) return [`OrbitError`] rather than
//! substituting a default: a protobuf message with a missing `oneof` or an
//! out-of-range timestamp is malformed input, and inventing a plausible value for
//! it would hide the corruption instead of reporting it.

use crate::{
    key_proto, AddressableReferenceProto, InvocationReasonProto, KeyProto, NoKeyProto, NodeIdProto,
    NodeStatusProto,
};
use chrono::{DateTime, Utc};
use orbit_shared::{
    AddressableReference, InvocationReason, Key, NodeId, NodeStatus, OrbitError, OrbitResult,
};
use prost_types::Timestamp;

impl From<&Key> for KeyProto {
    fn from(key: &Key) -> Self {
        let key_oneof = match key {
            Key::StringKey { key } => key_proto::Key::StringKey(key.clone()),
            Key::Int32Key { key } => key_proto::Key::Int32Key(*key),
            Key::Int64Key { key } => key_proto::Key::Int64Key(*key),
            Key::NoKey => key_proto::Key::NoKey(NoKeyProto {}),
        };
        Self {
            key: Some(key_oneof),
        }
    }
}

impl TryFrom<&KeyProto> for Key {
    type Error = OrbitError;

    fn try_from(proto: &KeyProto) -> Result<Self, Self::Error> {
        match &proto.key {
            Some(key_proto::Key::StringKey(k)) => Ok(Self::StringKey { key: k.clone() }),
            Some(key_proto::Key::Int32Key(k)) => Ok(Self::Int32Key { key: *k }),
            Some(key_proto::Key::Int64Key(k)) => Ok(Self::Int64Key { key: *k }),
            Some(key_proto::Key::NoKey(_)) => Ok(Self::NoKey),
            None => Err(OrbitError::internal("Missing key in KeyProto")),
        }
    }
}

impl From<&NodeId> for NodeIdProto {
    fn from(node_id: &NodeId) -> Self {
        Self {
            key: node_id.key.clone(),
            namespace: node_id.namespace.clone(),
        }
    }
}

impl From<&NodeIdProto> for NodeId {
    fn from(proto: &NodeIdProto) -> Self {
        Self {
            key: proto.key.clone(),
            namespace: proto.namespace.clone(),
        }
    }
}

impl From<&AddressableReference> for AddressableReferenceProto {
    fn from(reference: &AddressableReference) -> Self {
        Self {
            addressable_type: reference.addressable_type.clone(),
            key: Some((&reference.key).into()),
        }
    }
}

impl TryFrom<&AddressableReferenceProto> for AddressableReference {
    type Error = OrbitError;

    fn try_from(proto: &AddressableReferenceProto) -> Result<Self, Self::Error> {
        proto
            .key
            .as_ref()
            .ok_or_else(|| OrbitError::internal("Missing key in AddressableReferenceProto"))
            .and_then(Key::try_from)
            .map(|key| Self {
                addressable_type: proto.addressable_type.clone(),
                key,
            })
    }
}

impl From<&InvocationReason> for InvocationReasonProto {
    fn from(reason: &InvocationReason) -> Self {
        match reason {
            InvocationReason::Invocation => Self::Invocation,
            InvocationReason::Rerouted => Self::Rerouted,
        }
    }
}

impl From<InvocationReasonProto> for InvocationReason {
    fn from(proto: InvocationReasonProto) -> Self {
        match proto {
            InvocationReasonProto::Invocation => Self::Invocation,
            InvocationReasonProto::Rerouted => Self::Rerouted,
        }
    }
}

impl From<&NodeStatus> for NodeStatusProto {
    fn from(status: &NodeStatus) -> Self {
        match status {
            NodeStatus::Active => Self::Active,
            NodeStatus::Draining => Self::Draining,
            NodeStatus::Stopped => Self::Stopped,
        }
    }
}

impl From<NodeStatusProto> for NodeStatus {
    fn from(proto: NodeStatusProto) -> Self {
        match proto {
            NodeStatusProto::Active => Self::Active,
            NodeStatusProto::Draining => Self::Draining,
            NodeStatusProto::Stopped => Self::Stopped,
        }
    }
}

/// Convert between Rust [`Key`] and [`KeyProto`].
pub struct KeyConverter;

impl KeyConverter {
    /// Encode a domain key as its protobuf representation.
    #[must_use]
    pub fn to_proto(key: &Key) -> KeyProto {
        key.into()
    }

    /// Decode a protobuf key.
    ///
    /// # Errors
    /// Returns [`OrbitError::Internal`] if the `key` oneof is unset.
    pub fn from_proto(proto: &KeyProto) -> OrbitResult<Key> {
        Key::try_from(proto)
    }
}

/// Convert between Rust [`NodeId`] and [`NodeIdProto`].
pub struct NodeIdConverter;

impl NodeIdConverter {
    /// Encode a node id as its protobuf representation.
    #[must_use]
    pub fn to_proto(node_id: &NodeId) -> NodeIdProto {
        node_id.into()
    }

    /// Decode a protobuf node id. This conversion is total.
    #[must_use]
    pub fn from_proto(proto: &NodeIdProto) -> NodeId {
        proto.into()
    }
}

/// Convert between Rust [`AddressableReference`] and [`AddressableReferenceProto`].
pub struct AddressableReferenceConverter;

impl AddressableReferenceConverter {
    /// Encode an addressable reference as its protobuf representation.
    #[must_use]
    pub fn to_proto(reference: &AddressableReference) -> AddressableReferenceProto {
        reference.into()
    }

    /// Decode a protobuf addressable reference.
    ///
    /// # Errors
    /// Returns [`OrbitError::Internal`] if the nested key is missing or malformed.
    pub fn from_proto(proto: &AddressableReferenceProto) -> OrbitResult<AddressableReference> {
        AddressableReference::try_from(proto)
    }
}

/// Convert between Rust `DateTime<Utc>` and protobuf [`Timestamp`].
pub struct TimestampConverter;

impl TimestampConverter {
    /// Encode a UTC timestamp as its protobuf representation.
    #[must_use]
    pub fn to_proto(dt: &DateTime<Utc>) -> Timestamp {
        Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        }
    }

    /// Decode a protobuf timestamp.
    ///
    /// # Errors
    /// Returns [`OrbitError::Internal`] when the seconds/nanos pair is not a
    /// representable instant. An unrepresentable timestamp is malformed input; it
    /// is reported rather than replaced with the current time, which would silently
    /// restamp the record with its decode time.
    pub fn from_proto(timestamp: &Timestamp) -> OrbitResult<DateTime<Utc>> {
        u32::try_from(timestamp.nanos)
            .ok()
            .and_then(|nanos| DateTime::from_timestamp(timestamp.seconds, nanos))
            .ok_or_else(|| {
                OrbitError::internal(format!(
                    "Timestamp out of range: seconds={}, nanos={}",
                    timestamp.seconds, timestamp.nanos
                ))
            })
    }
}

/// Convert between Rust [`InvocationReason`] and [`InvocationReasonProto`].
pub struct InvocationReasonConverter;

impl InvocationReasonConverter {
    /// Encode an invocation reason as its protobuf representation.
    #[must_use]
    pub fn to_proto(reason: &InvocationReason) -> InvocationReasonProto {
        reason.into()
    }

    /// Decode a protobuf invocation reason. This conversion is total.
    #[must_use]
    pub fn from_proto(proto: InvocationReasonProto) -> InvocationReason {
        proto.into()
    }
}

/// Convert between Rust [`NodeStatus`] and [`NodeStatusProto`].
pub struct NodeStatusConverter;

impl NodeStatusConverter {
    /// Encode a node status as its protobuf representation.
    #[must_use]
    pub fn to_proto(status: &NodeStatus) -> NodeStatusProto {
        status.into()
    }

    /// Decode a protobuf node status. This conversion is total.
    #[must_use]
    pub fn from_proto(proto: NodeStatusProto) -> NodeStatus {
        proto.into()
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
            OrbitError::Internal { message, .. } => {
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
            OrbitError::Internal { message, .. } => {
                assert!(message.contains("Missing key in AddressableReferenceProto"));
            }
            _ => panic!("Expected Internal error"),
        }
    }

    #[test]
    fn test_timestamp_converter() {
        let dt = Utc::now();

        let proto = TimestampConverter::to_proto(&dt);
        let converted_back = TimestampConverter::from_proto(&proto).unwrap();

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
        let converted_back = TimestampConverter::from_proto(&proto).unwrap();

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
    fn test_trait_conversions_roundtrip_every_key_variant() {
        let keys = [
            Key::StringKey {
                key: "k".to_string(),
            },
            Key::Int32Key { key: -7 },
            Key::Int64Key { key: i64::MIN },
            Key::NoKey,
        ];

        for key in keys {
            let proto: KeyProto = (&key).into();
            assert_eq!(key, Key::try_from(&proto).unwrap());
        }
    }

    #[test]
    fn test_trait_conversions_match_converter_structs() {
        let reference = AddressableReference {
            addressable_type: "Actor".to_string(),
            key: Key::Int64Key { key: 42 },
        };
        let node_id = NodeId {
            key: "node".to_string(),
            namespace: "ns".to_string(),
        };

        let via_trait: AddressableReferenceProto = (&reference).into();
        assert_eq!(
            via_trait,
            AddressableReferenceConverter::to_proto(&reference)
        );
        assert_eq!(
            reference,
            AddressableReference::try_from(&via_trait).unwrap()
        );

        let node_proto: NodeIdProto = (&node_id).into();
        assert_eq!(node_proto, NodeIdConverter::to_proto(&node_id));
        assert_eq!(node_id, NodeId::from(&node_proto));

        assert_eq!(
            NodeStatusProto::from(&NodeStatus::Draining),
            NodeStatusConverter::to_proto(&NodeStatus::Draining)
        );
        assert_eq!(
            InvocationReasonProto::from(&InvocationReason::Rerouted),
            InvocationReasonConverter::to_proto(&InvocationReason::Rerouted)
        );
    }

    #[test]
    fn test_timestamp_converter_rejects_malformed_timestamp() {
        // Negative nanos are not representable; the decoder must report that rather
        // than substitute the current time, which would restamp the record.
        let cases = [
            Timestamp {
                seconds: -1,
                nanos: -1,
            },
            Timestamp {
                seconds: i64::MAX,
                nanos: 0,
            },
        ];

        for invalid_proto in cases {
            match TimestampConverter::from_proto(&invalid_proto) {
                Err(OrbitError::Internal { message, .. }) => {
                    assert!(
                        message.contains("Timestamp out of range"),
                        "unexpected message: {message}"
                    );
                }
                other => panic!("Expected an out-of-range error, got {other:?}"),
            }
        }
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
        let dt_back = TimestampConverter::from_proto(&dt_proto).unwrap();
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
