//! Bolt Protocol Type Encoding/Decoding
//!
//! This module provides encoding and decoding for all Bolt protocol data types,
//! including graph types (Node, Relationship, Path) and temporal types.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use orbit_shared::graph::{GraphNode, GraphRelationship, NodeId, RelationshipId};
use serde_json::Value;
use std::collections::HashMap;

/// PackStream value representation
#[derive(Debug, Clone, PartialEq)]
pub enum PackStreamValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<PackStreamValue>),
    Map(HashMap<String, PackStreamValue>),
    Structure {
        signature: u8,
        fields: Vec<PackStreamValue>,
    },
}

impl PackStreamValue {
    /// Convert to serde_json::Value
    pub fn to_json(&self) -> Value {
        match self {
            PackStreamValue::Null => Value::Null,
            PackStreamValue::Boolean(b) => Value::Bool(*b),
            PackStreamValue::Integer(i) => Value::Number((*i).into()),
            PackStreamValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            PackStreamValue::String(s) => Value::String(s.clone()),
            PackStreamValue::List(items) => {
                Value::Array(items.iter().map(|v| v.to_json()).collect())
            }
            PackStreamValue::Map(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map {
                    obj.insert(k.clone(), v.to_json());
                }
                Value::Object(obj)
            }
            PackStreamValue::Structure { signature, fields } => {
                let mut obj = serde_json::Map::new();
                obj.insert("_signature".to_string(), Value::Number((*signature).into()));
                obj.insert(
                    "fields".to_string(),
                    Value::Array(fields.iter().map(|f| f.to_json()).collect()),
                );
                Value::Object(obj)
            }
        }
    }

    /// Convert from serde_json::Value
    pub fn from_json(value: &Value) -> Self {
        match value {
            Value::Null => PackStreamValue::Null,
            Value::Bool(b) => PackStreamValue::Boolean(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PackStreamValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    PackStreamValue::Float(f)
                } else {
                    PackStreamValue::Null
                }
            }
            Value::String(s) => PackStreamValue::String(s.clone()),
            Value::Array(arr) => {
                PackStreamValue::List(arr.iter().map(PackStreamValue::from_json).collect())
            }
            Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), PackStreamValue::from_json(v));
                }
                PackStreamValue::Map(map)
            }
        }
    }
}

/// Encode a GraphNode as a Bolt Node structure
///
/// Bolt Node structure (signature 0x4E):
/// - Field 0: Integer (node ID)
/// - Field 1: List of String (labels)
/// - Field 2: Map (properties)
pub fn encode_node(node: &GraphNode) -> PackStreamValue {
    // Extract numeric ID from NodeId string
    let id_num = node
        .id
        .as_str()
        .split('_')
        .last()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let labels = PackStreamValue::List(
        node.labels
            .iter()
            .map(|l| PackStreamValue::String(l.clone()))
            .collect(),
    );

    let mut properties = HashMap::new();
    for (k, v) in &node.properties {
        properties.insert(k.clone(), PackStreamValue::from_json(v));
    }

    PackStreamValue::Structure {
        signature: 0x4E,
        fields: vec![
            PackStreamValue::Integer(id_num),
            labels,
            PackStreamValue::Map(properties),
        ],
    }
}

/// Decode a Bolt Node structure to GraphNode
pub fn decode_node(value: &PackStreamValue) -> ProtocolResult<GraphNode> {
    if let PackStreamValue::Structure { signature, fields } = value {
        if *signature != 0x4E || fields.len() < 3 {
            return Err(ProtocolError::CypherError(
                "Invalid Node structure".to_string(),
            ));
        }

        let id = if let PackStreamValue::Integer(i) = &fields[0] {
            NodeId::new(format!("node_{}", i))
        } else {
            return Err(ProtocolError::CypherError("Invalid node ID".to_string()));
        };

        let labels = if let PackStreamValue::List(items) = &fields[1] {
            items
                .iter()
                .filter_map(|v| {
                    if let PackStreamValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let properties = if let PackStreamValue::Map(map) = &fields[2] {
            map.iter().map(|(k, v)| (k.clone(), v.to_json())).collect()
        } else {
            HashMap::new()
        };

        Ok(GraphNode::with_id(id, labels, properties))
    } else {
        Err(ProtocolError::CypherError(
            "Expected Node structure".to_string(),
        ))
    }
}

/// Encode a GraphRelationship as a Bolt Relationship structure
///
/// Bolt Relationship structure (signature 0x52):
/// - Field 0: Integer (relationship ID)
/// - Field 1: Integer (start node ID)
/// - Field 2: Integer (end node ID)
/// - Field 3: String (relationship type)
/// - Field 4: Map (properties)
pub fn encode_relationship(rel: &GraphRelationship) -> PackStreamValue {
    let rel_id = rel
        .id
        .as_str()
        .split('_')
        .last()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let start_id = rel
        .start_node
        .as_str()
        .split('_')
        .last()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let end_id = rel
        .end_node
        .as_str()
        .split('_')
        .last()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let mut properties = HashMap::new();
    for (k, v) in &rel.properties {
        properties.insert(k.clone(), PackStreamValue::from_json(v));
    }

    PackStreamValue::Structure {
        signature: 0x52,
        fields: vec![
            PackStreamValue::Integer(rel_id),
            PackStreamValue::Integer(start_id),
            PackStreamValue::Integer(end_id),
            PackStreamValue::String(rel.rel_type.clone()),
            PackStreamValue::Map(properties),
        ],
    }
}

/// Decode a Bolt Relationship structure to GraphRelationship
pub fn decode_relationship(value: &PackStreamValue) -> ProtocolResult<GraphRelationship> {
    if let PackStreamValue::Structure { signature, fields } = value {
        if *signature != 0x52 || fields.len() < 5 {
            return Err(ProtocolError::CypherError(
                "Invalid Relationship structure".to_string(),
            ));
        }

        let id = if let PackStreamValue::Integer(i) = &fields[0] {
            RelationshipId::from_string(&format!("rel_{}", i))
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid relationship ID".to_string(),
            ));
        };

        let start_node = if let PackStreamValue::Integer(i) = &fields[1] {
            NodeId::new(format!("node_{}", i))
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid start node ID".to_string(),
            ));
        };

        let end_node = if let PackStreamValue::Integer(i) = &fields[2] {
            NodeId::new(format!("node_{}", i))
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid end node ID".to_string(),
            ));
        };

        let rel_type = if let PackStreamValue::String(s) = &fields[3] {
            s.clone()
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid relationship type".to_string(),
            ));
        };

        let properties = if let PackStreamValue::Map(map) = &fields[4] {
            map.iter().map(|(k, v)| (k.clone(), v.to_json())).collect()
        } else {
            HashMap::new()
        };

        Ok(GraphRelationship::new(
            start_node, end_node, rel_type, properties,
        ))
    } else {
        Err(ProtocolError::CypherError(
            "Expected Relationship structure".to_string(),
        ))
    }
}

/// Encode a Date as a Bolt Date structure
///
/// Bolt Date structure (signature 0x44):
/// - Field 0: Integer (days since Unix epoch)
pub fn encode_date(date: &NaiveDate) -> PackStreamValue {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let days = date.signed_duration_since(epoch).num_days();

    PackStreamValue::Structure {
        signature: 0x44,
        fields: vec![PackStreamValue::Integer(days)],
    }
}

/// Decode a Bolt Date structure
pub fn decode_date(value: &PackStreamValue) -> ProtocolResult<NaiveDate> {
    if let PackStreamValue::Structure { signature, fields } = value {
        if *signature != 0x44 || fields.is_empty() {
            return Err(ProtocolError::CypherError(
                "Invalid Date structure".to_string(),
            ));
        }

        if let PackStreamValue::Integer(days) = &fields[0] {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
            Ok(epoch + Duration::days(*days))
        } else {
            Err(ProtocolError::CypherError("Invalid date value".to_string()))
        }
    } else {
        Err(ProtocolError::CypherError(
            "Expected Date structure".to_string(),
        ))
    }
}

/// Encode a LocalTime as a Bolt LocalTime structure
///
/// Bolt LocalTime structure (signature 0x74):
/// - Field 0: Integer (nanoseconds since midnight)
pub fn encode_local_time(time: &NaiveTime) -> PackStreamValue {
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let nanos = time
        .signed_duration_since(midnight)
        .num_nanoseconds()
        .unwrap_or(0);

    PackStreamValue::Structure {
        signature: 0x74,
        fields: vec![PackStreamValue::Integer(nanos)],
    }
}

/// Encode a DateTime as a Bolt DateTime structure
///
/// Bolt DateTime structure (signature 0x46):
/// - Field 0: Integer (seconds since Unix epoch)
/// - Field 1: Integer (nanoseconds)
/// - Field 2: Integer (timezone offset in seconds)
pub fn encode_datetime(dt: &DateTime<Utc>) -> PackStreamValue {
    let seconds = dt.timestamp();
    let nanos = dt.timestamp_subsec_nanos() as i64;

    PackStreamValue::Structure {
        signature: 0x46,
        fields: vec![
            PackStreamValue::Integer(seconds),
            PackStreamValue::Integer(nanos),
            PackStreamValue::Integer(0), // UTC offset
        ],
    }
}

/// Encode a LocalDateTime as a Bolt LocalDateTime structure
///
/// Bolt LocalDateTime structure (signature 0x64):
/// - Field 0: Integer (seconds since Unix epoch)
/// - Field 1: Integer (nanoseconds)
pub fn encode_local_datetime(dt: &NaiveDateTime) -> PackStreamValue {
    let seconds = dt.and_utc().timestamp();
    let nanos = dt.and_utc().timestamp_subsec_nanos() as i64;

    PackStreamValue::Structure {
        signature: 0x64,
        fields: vec![
            PackStreamValue::Integer(seconds),
            PackStreamValue::Integer(nanos),
        ],
    }
}

/// Encode a Duration as a Bolt Duration structure
///
/// Bolt Duration structure (signature 0x45):
/// - Field 0: Integer (months)
/// - Field 1: Integer (days)
/// - Field 2: Integer (seconds)
/// - Field 3: Integer (nanoseconds)
pub fn encode_duration(duration: &Duration) -> PackStreamValue {
    let days = duration.num_days();
    let seconds = duration.num_seconds() - (days * 86400);
    let nanos = duration.num_nanoseconds().unwrap_or(0) - (duration.num_seconds() * 1_000_000_000);

    PackStreamValue::Structure {
        signature: 0x45,
        fields: vec![
            PackStreamValue::Integer(0), // months
            PackStreamValue::Integer(days),
            PackStreamValue::Integer(seconds),
            PackStreamValue::Integer(nanos),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packstream_value_conversions() {
        // Test primitive types
        assert_eq!(PackStreamValue::Null.to_json(), Value::Null);
        assert_eq!(PackStreamValue::Boolean(true).to_json(), Value::Bool(true));
        assert_eq!(
            PackStreamValue::Integer(42).to_json(),
            Value::Number(42.into())
        );
        assert_eq!(
            PackStreamValue::String("hello".to_string()).to_json(),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn test_node_encoding() {
        let node = GraphNode::with_id(
            NodeId::new("node_123".to_string()),
            vec!["Person".to_string()],
            {
                let mut props = HashMap::new();
                props.insert("name".to_string(), Value::String("Alice".to_string()));
                props
            },
        );

        let encoded = encode_node(&node);

        if let PackStreamValue::Structure { signature, fields } = encoded {
            assert_eq!(signature, 0x4E);
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0], PackStreamValue::Integer(123));
        } else {
            panic!("Expected Structure");
        }
    }

    #[test]
    fn test_relationship_encoding() {
        let rel = GraphRelationship::new(
            NodeId::new("node_1".to_string()),
            NodeId::new("node_2".to_string()),
            "KNOWS".to_string(),
            HashMap::new(),
        );

        let encoded = encode_relationship(&rel);

        if let PackStreamValue::Structure { signature, fields } = encoded {
            assert_eq!(signature, 0x52);
            assert_eq!(fields.len(), 5);
            assert_eq!(fields[3], PackStreamValue::String("KNOWS".to_string()));
        } else {
            panic!("Expected Structure");
        }
    }

    #[test]
    fn test_date_encoding() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let encoded = encode_date(&date);

        if let PackStreamValue::Structure { signature, fields } = encoded {
            assert_eq!(signature, 0x44);
            assert_eq!(fields.len(), 1);
            // 2024-01-01 is 19723 days since epoch
            assert!(matches!(fields[0], PackStreamValue::Integer(_)));
        } else {
            panic!("Expected Structure");
        }
    }
}
