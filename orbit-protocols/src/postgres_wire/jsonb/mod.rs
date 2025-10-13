//! JSONB binary codec and utilities
//!
//! This module provides a lightweight binary representation for JSONB values using CBOR
//! for compact storage and fast (de)serialization. It also exposes helper functions to
//! convert between serde_json::Value and binary form.

use serde_json::Value as JsonValue;
use serde_cbor;
use crate::error::{ProtocolError, ProtocolResult};

/// Encode a serde_json::Value into CBOR bytes for compact storage.
pub fn encode_jsonb(value: &JsonValue) -> ProtocolResult<Vec<u8>> {
    serde_cbor::to_vec(value).map_err(|e| ProtocolError::SerializationError(format!("CBOR encode failed: {}", e)))
}

/// Decode CBOR bytes back into serde_json::Value.
pub fn decode_jsonb(bytes: &[u8]) -> ProtocolResult<JsonValue> {
    serde_cbor::from_slice(bytes).map_err(|e| ProtocolError::SerializationError(format!("CBOR decode failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_simple() {
        let v = json!({"name": "alice", "age": 30, "tags": ["rust", "db"]});
        let b = encode_jsonb(&v).expect("encode");
        let v2 = decode_jsonb(&b).expect("decode");
        assert_eq!(v, v2);
    }

    #[test]
    fn roundtrip_nested() {
        let v = json!({"a": {"b": {"c": [1,2,3]}}, "x": true});
        let b = encode_jsonb(&v).expect("encode");
        let v2 = decode_jsonb(&b).expect("decode");
        assert_eq!(v, v2);
    }
}
