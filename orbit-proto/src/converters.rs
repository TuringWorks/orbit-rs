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

/// Convert between Rust DateTime<Utc> and protobuf Timestamp
pub struct TimestampConverter;

impl TimestampConverter {
    pub fn to_proto(dt: &DateTime<Utc>) -> Timestamp {
        Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        }
    }

    pub fn from_proto(timestamp: &Timestamp) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
            .unwrap_or_else(|| Utc::now())
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
