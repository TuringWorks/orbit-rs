//! OrbitWire value encoding
//!
//! Defines how different OrbitQL data types are encoded in the wire protocol

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;

/// Type tags for wire encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeTag {
    // Primitive types
    Null = 0x00,
    Boolean = 0x01,
    Int8 = 0x02,
    Int16 = 0x03,
    Int32 = 0x04,
    Int64 = 0x05,
    UInt8 = 0x06,
    UInt16 = 0x07,
    UInt32 = 0x08,
    UInt64 = 0x09,
    Float32 = 0x0A,
    Float64 = 0x0B,

    // String/Binary
    String = 0x10,
    Binary = 0x11,

    // Temporal
    Date = 0x20,
    Time = 0x21,
    Timestamp = 0x22,
    Duration = 0x23,
    Interval = 0x24,

    // Complex types
    Array = 0x30,
    Object = 0x31,
    Json = 0x32,

    // Special types
    Uuid = 0x40,
    Decimal = 0x41,

    // Spatial types
    Point = 0x50,
    LineString = 0x51,
    Polygon = 0x52,
    MultiPoint = 0x53,
    MultiLineString = 0x54,
    MultiPolygon = 0x55,
    Geometry = 0x56,

    // Vector type
    Vector = 0x60,

    // Graph types
    GraphNodeRef = 0x70,
    GraphEdgeRef = 0x71,
    GraphPath = 0x72,

    // Unknown
    Unknown = 0xFF,
}

impl TypeTag {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Self::Null,
            0x01 => Self::Boolean,
            0x02 => Self::Int8,
            0x03 => Self::Int16,
            0x04 => Self::Int32,
            0x05 => Self::Int64,
            0x06 => Self::UInt8,
            0x07 => Self::UInt16,
            0x08 => Self::UInt32,
            0x09 => Self::UInt64,
            0x0A => Self::Float32,
            0x0B => Self::Float64,
            0x10 => Self::String,
            0x11 => Self::Binary,
            0x20 => Self::Date,
            0x21 => Self::Time,
            0x22 => Self::Timestamp,
            0x23 => Self::Duration,
            0x24 => Self::Interval,
            0x30 => Self::Array,
            0x31 => Self::Object,
            0x32 => Self::Json,
            0x40 => Self::Uuid,
            0x41 => Self::Decimal,
            0x50 => Self::Point,
            0x51 => Self::LineString,
            0x52 => Self::Polygon,
            0x53 => Self::MultiPoint,
            0x54 => Self::MultiLineString,
            0x55 => Self::MultiPolygon,
            0x56 => Self::Geometry,
            0x60 => Self::Vector,
            0x70 => Self::GraphNodeRef,
            0x71 => Self::GraphEdgeRef,
            0x72 => Self::GraphPath,
            _ => Self::Unknown,
        }
    }
}

/// Wire value representation
#[derive(Debug, Clone, PartialEq)]
pub enum WireValue {
    Null,
    Boolean(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    String(String),
    Binary(Bytes),
    Date(i32),      // Days since epoch
    Time(i64),      // Microseconds since midnight
    Timestamp(i64), // Microseconds since epoch
    Duration(i64),  // Microseconds
    Interval {
        months: i32,
        days: i32,
        microseconds: i64,
    },
    Array(Vec<WireValue>),
    Object(HashMap<String, WireValue>),
    Json(String),
    Uuid([u8; 16]),
    Decimal {
        unscaled: i128,
        scale: u8,
    },
    Point {
        x: f64,
        y: f64,
    },
    LineString(Vec<(f64, f64)>),
    Polygon(Vec<Vec<(f64, f64)>>),
    MultiPoint(Vec<(f64, f64)>),
    MultiLineString(Vec<Vec<(f64, f64)>>),
    MultiPolygon(Vec<Vec<Vec<(f64, f64)>>>),
    Geometry(Bytes), // WKB encoded
    Vector(Vec<f32>),
    GraphNodeRef {
        table: String,
        id: String,
    },
    GraphEdgeRef {
        table: String,
        id: String,
        in_node: String,
        out_node: String,
    },
    GraphPath(GraphPathValue),
}

/// Graph path value
#[derive(Debug, Clone, PartialEq)]
pub struct GraphPathValue {
    pub nodes: Vec<GraphNodeValue>,
    pub edges: Vec<GraphEdgeValue>,
    pub length: u32,
    pub cost: Option<f64>,
}

/// Graph node value
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNodeValue {
    pub id: String,
    pub table: String,
    pub properties: HashMap<String, WireValue>,
}

/// Graph edge value
#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdgeValue {
    pub id: String,
    pub table: String,
    pub in_node: String,
    pub out_node: String,
    pub properties: HashMap<String, WireValue>,
}

impl WireValue {
    /// Encode value to bytes
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(64);
        self.encode_to(&mut buf);
        buf.freeze()
    }

    /// Encode value to a buffer
    pub fn encode_to(&self, buf: &mut BytesMut) {
        match self {
            WireValue::Null => {
                buf.put_u8(TypeTag::Null as u8);
            }
            WireValue::Boolean(v) => {
                buf.put_u8(TypeTag::Boolean as u8);
                buf.put_u8(if *v { 1 } else { 0 });
            }
            WireValue::Int8(v) => {
                buf.put_u8(TypeTag::Int8 as u8);
                buf.put_i8(*v);
            }
            WireValue::Int16(v) => {
                buf.put_u8(TypeTag::Int16 as u8);
                buf.put_i16(*v);
            }
            WireValue::Int32(v) => {
                buf.put_u8(TypeTag::Int32 as u8);
                buf.put_i32(*v);
            }
            WireValue::Int64(v) => {
                buf.put_u8(TypeTag::Int64 as u8);
                buf.put_i64(*v);
            }
            WireValue::UInt8(v) => {
                buf.put_u8(TypeTag::UInt8 as u8);
                buf.put_u8(*v);
            }
            WireValue::UInt16(v) => {
                buf.put_u8(TypeTag::UInt16 as u8);
                buf.put_u16(*v);
            }
            WireValue::UInt32(v) => {
                buf.put_u8(TypeTag::UInt32 as u8);
                buf.put_u32(*v);
            }
            WireValue::UInt64(v) => {
                buf.put_u8(TypeTag::UInt64 as u8);
                buf.put_u64(*v);
            }
            WireValue::Float32(v) => {
                buf.put_u8(TypeTag::Float32 as u8);
                buf.put_f32(*v);
            }
            WireValue::Float64(v) => {
                buf.put_u8(TypeTag::Float64 as u8);
                buf.put_f64(*v);
            }
            WireValue::String(v) => {
                buf.put_u8(TypeTag::String as u8);
                encode_string(buf, v);
            }
            WireValue::Binary(v) => {
                buf.put_u8(TypeTag::Binary as u8);
                buf.put_u32(v.len() as u32);
                buf.put_slice(v);
            }
            WireValue::Date(v) => {
                buf.put_u8(TypeTag::Date as u8);
                buf.put_i32(*v);
            }
            WireValue::Time(v) => {
                buf.put_u8(TypeTag::Time as u8);
                buf.put_i64(*v);
            }
            WireValue::Timestamp(v) => {
                buf.put_u8(TypeTag::Timestamp as u8);
                buf.put_i64(*v);
            }
            WireValue::Duration(v) => {
                buf.put_u8(TypeTag::Duration as u8);
                buf.put_i64(*v);
            }
            WireValue::Interval {
                months,
                days,
                microseconds,
            } => {
                buf.put_u8(TypeTag::Interval as u8);
                buf.put_i32(*months);
                buf.put_i32(*days);
                buf.put_i64(*microseconds);
            }
            WireValue::Array(arr) => {
                buf.put_u8(TypeTag::Array as u8);
                buf.put_u32(arr.len() as u32);
                for item in arr {
                    item.encode_to(buf);
                }
            }
            WireValue::Object(obj) => {
                buf.put_u8(TypeTag::Object as u8);
                buf.put_u32(obj.len() as u32);
                for (key, value) in obj {
                    encode_string(buf, key);
                    value.encode_to(buf);
                }
            }
            WireValue::Json(v) => {
                buf.put_u8(TypeTag::Json as u8);
                encode_string(buf, v);
            }
            WireValue::Uuid(v) => {
                buf.put_u8(TypeTag::Uuid as u8);
                buf.put_slice(v);
            }
            WireValue::Decimal { unscaled, scale } => {
                buf.put_u8(TypeTag::Decimal as u8);
                buf.put_i128(*unscaled);
                buf.put_u8(*scale);
            }
            WireValue::Point { x, y } => {
                buf.put_u8(TypeTag::Point as u8);
                buf.put_f64(*x);
                buf.put_f64(*y);
            }
            WireValue::LineString(points) => {
                buf.put_u8(TypeTag::LineString as u8);
                buf.put_u32(points.len() as u32);
                for (x, y) in points {
                    buf.put_f64(*x);
                    buf.put_f64(*y);
                }
            }
            WireValue::Polygon(rings) => {
                buf.put_u8(TypeTag::Polygon as u8);
                buf.put_u32(rings.len() as u32);
                for ring in rings {
                    buf.put_u32(ring.len() as u32);
                    for (x, y) in ring {
                        buf.put_f64(*x);
                        buf.put_f64(*y);
                    }
                }
            }
            WireValue::MultiPoint(points) => {
                buf.put_u8(TypeTag::MultiPoint as u8);
                buf.put_u32(points.len() as u32);
                for (x, y) in points {
                    buf.put_f64(*x);
                    buf.put_f64(*y);
                }
            }
            WireValue::MultiLineString(lines) => {
                buf.put_u8(TypeTag::MultiLineString as u8);
                buf.put_u32(lines.len() as u32);
                for line in lines {
                    buf.put_u32(line.len() as u32);
                    for (x, y) in line {
                        buf.put_f64(*x);
                        buf.put_f64(*y);
                    }
                }
            }
            WireValue::MultiPolygon(polygons) => {
                buf.put_u8(TypeTag::MultiPolygon as u8);
                buf.put_u32(polygons.len() as u32);
                for polygon in polygons {
                    buf.put_u32(polygon.len() as u32);
                    for ring in polygon {
                        buf.put_u32(ring.len() as u32);
                        for (x, y) in ring {
                            buf.put_f64(*x);
                            buf.put_f64(*y);
                        }
                    }
                }
            }
            WireValue::Geometry(wkb) => {
                buf.put_u8(TypeTag::Geometry as u8);
                buf.put_u32(wkb.len() as u32);
                buf.put_slice(wkb);
            }
            WireValue::Vector(v) => {
                buf.put_u8(TypeTag::Vector as u8);
                buf.put_u32(v.len() as u32);
                for f in v {
                    buf.put_f32(*f);
                }
            }
            WireValue::GraphNodeRef { table, id } => {
                buf.put_u8(TypeTag::GraphNodeRef as u8);
                encode_string(buf, table);
                encode_string(buf, id);
            }
            WireValue::GraphEdgeRef {
                table,
                id,
                in_node,
                out_node,
            } => {
                buf.put_u8(TypeTag::GraphEdgeRef as u8);
                encode_string(buf, table);
                encode_string(buf, id);
                encode_string(buf, in_node);
                encode_string(buf, out_node);
            }
            WireValue::GraphPath(path) => {
                buf.put_u8(TypeTag::GraphPath as u8);
                path.encode_to(buf);
            }
        }
    }

    /// Decode value from bytes
    pub fn decode(data: &mut Bytes) -> Result<Self, ValueDecodeError> {
        if data.remaining() < 1 {
            return Err(ValueDecodeError::InsufficientData);
        }

        let type_tag = TypeTag::from_byte(data.get_u8());

        match type_tag {
            TypeTag::Null => Ok(WireValue::Null),
            TypeTag::Boolean => {
                ensure_remaining(data, 1)?;
                Ok(WireValue::Boolean(data.get_u8() != 0))
            }
            TypeTag::Int8 => {
                ensure_remaining(data, 1)?;
                Ok(WireValue::Int8(data.get_i8()))
            }
            TypeTag::Int16 => {
                ensure_remaining(data, 2)?;
                Ok(WireValue::Int16(data.get_i16()))
            }
            TypeTag::Int32 => {
                ensure_remaining(data, 4)?;
                Ok(WireValue::Int32(data.get_i32()))
            }
            TypeTag::Int64 => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::Int64(data.get_i64()))
            }
            TypeTag::UInt8 => {
                ensure_remaining(data, 1)?;
                Ok(WireValue::UInt8(data.get_u8()))
            }
            TypeTag::UInt16 => {
                ensure_remaining(data, 2)?;
                Ok(WireValue::UInt16(data.get_u16()))
            }
            TypeTag::UInt32 => {
                ensure_remaining(data, 4)?;
                Ok(WireValue::UInt32(data.get_u32()))
            }
            TypeTag::UInt64 => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::UInt64(data.get_u64()))
            }
            TypeTag::Float32 => {
                ensure_remaining(data, 4)?;
                Ok(WireValue::Float32(data.get_f32()))
            }
            TypeTag::Float64 => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::Float64(data.get_f64()))
            }
            TypeTag::String => {
                let s = decode_string(data)?;
                Ok(WireValue::String(s))
            }
            TypeTag::Binary => {
                ensure_remaining(data, 4)?;
                let len = data.get_u32() as usize;
                ensure_remaining(data, len)?;
                Ok(WireValue::Binary(data.copy_to_bytes(len)))
            }
            TypeTag::Date => {
                ensure_remaining(data, 4)?;
                Ok(WireValue::Date(data.get_i32()))
            }
            TypeTag::Time => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::Time(data.get_i64()))
            }
            TypeTag::Timestamp => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::Timestamp(data.get_i64()))
            }
            TypeTag::Duration => {
                ensure_remaining(data, 8)?;
                Ok(WireValue::Duration(data.get_i64()))
            }
            TypeTag::Interval => {
                ensure_remaining(data, 16)?;
                Ok(WireValue::Interval {
                    months: data.get_i32(),
                    days: data.get_i32(),
                    microseconds: data.get_i64(),
                })
            }
            TypeTag::Array => {
                ensure_remaining(data, 4)?;
                let len = data.get_u32() as usize;
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len {
                    arr.push(WireValue::decode(data)?);
                }
                Ok(WireValue::Array(arr))
            }
            TypeTag::Object => {
                ensure_remaining(data, 4)?;
                let len = data.get_u32() as usize;
                let mut obj = HashMap::with_capacity(len);
                for _ in 0..len {
                    let key = decode_string(data)?;
                    let value = WireValue::decode(data)?;
                    obj.insert(key, value);
                }
                Ok(WireValue::Object(obj))
            }
            TypeTag::Json => {
                let s = decode_string(data)?;
                Ok(WireValue::Json(s))
            }
            TypeTag::Uuid => {
                ensure_remaining(data, 16)?;
                let mut uuid = [0u8; 16];
                data.copy_to_slice(&mut uuid);
                Ok(WireValue::Uuid(uuid))
            }
            TypeTag::Decimal => {
                ensure_remaining(data, 17)?;
                Ok(WireValue::Decimal {
                    unscaled: data.get_i128(),
                    scale: data.get_u8(),
                })
            }
            TypeTag::Point => {
                ensure_remaining(data, 16)?;
                Ok(WireValue::Point {
                    x: data.get_f64(),
                    y: data.get_f64(),
                })
            }
            TypeTag::Vector => {
                ensure_remaining(data, 4)?;
                let len = data.get_u32() as usize;
                ensure_remaining(data, len * 4)?;
                let mut vec = Vec::with_capacity(len);
                for _ in 0..len {
                    vec.push(data.get_f32());
                }
                Ok(WireValue::Vector(vec))
            }
            TypeTag::GraphNodeRef => {
                let table = decode_string(data)?;
                let id = decode_string(data)?;
                Ok(WireValue::GraphNodeRef { table, id })
            }
            TypeTag::GraphEdgeRef => {
                let table = decode_string(data)?;
                let id = decode_string(data)?;
                let in_node = decode_string(data)?;
                let out_node = decode_string(data)?;
                Ok(WireValue::GraphEdgeRef {
                    table,
                    id,
                    in_node,
                    out_node,
                })
            }
            TypeTag::GraphPath => {
                let path = GraphPathValue::decode(data)?;
                Ok(WireValue::GraphPath(path))
            }
            _ => Err(ValueDecodeError::UnsupportedType(type_tag as u8)),
        }
    }
}

impl GraphPathValue {
    pub fn encode_to(&self, buf: &mut BytesMut) {
        // Number of nodes
        buf.put_u32(self.nodes.len() as u32);
        for node in &self.nodes {
            encode_string(buf, &node.id);
            encode_string(buf, &node.table);
            buf.put_u32(node.properties.len() as u32);
            for (key, value) in &node.properties {
                encode_string(buf, key);
                value.encode_to(buf);
            }
        }

        // Number of edges
        buf.put_u32(self.edges.len() as u32);
        for edge in &self.edges {
            encode_string(buf, &edge.id);
            encode_string(buf, &edge.table);
            encode_string(buf, &edge.in_node);
            encode_string(buf, &edge.out_node);
            buf.put_u32(edge.properties.len() as u32);
            for (key, value) in &edge.properties {
                encode_string(buf, key);
                value.encode_to(buf);
            }
        }

        // Length and cost
        buf.put_u32(self.length);
        match self.cost {
            Some(c) => {
                buf.put_u8(1);
                buf.put_f64(c);
            }
            None => {
                buf.put_u8(0);
            }
        }
    }

    pub fn decode(data: &mut Bytes) -> Result<Self, ValueDecodeError> {
        ensure_remaining(data, 4)?;
        let num_nodes = data.get_u32() as usize;
        let mut nodes = Vec::with_capacity(num_nodes);

        for _ in 0..num_nodes {
            let id = decode_string(data)?;
            let table = decode_string(data)?;
            ensure_remaining(data, 4)?;
            let num_props = data.get_u32() as usize;
            let mut properties = HashMap::with_capacity(num_props);
            for _ in 0..num_props {
                let key = decode_string(data)?;
                let value = WireValue::decode(data)?;
                properties.insert(key, value);
            }
            nodes.push(GraphNodeValue {
                id,
                table,
                properties,
            });
        }

        ensure_remaining(data, 4)?;
        let num_edges = data.get_u32() as usize;
        let mut edges = Vec::with_capacity(num_edges);

        for _ in 0..num_edges {
            let id = decode_string(data)?;
            let table = decode_string(data)?;
            let in_node = decode_string(data)?;
            let out_node = decode_string(data)?;
            ensure_remaining(data, 4)?;
            let num_props = data.get_u32() as usize;
            let mut properties = HashMap::with_capacity(num_props);
            for _ in 0..num_props {
                let key = decode_string(data)?;
                let value = WireValue::decode(data)?;
                properties.insert(key, value);
            }
            edges.push(GraphEdgeValue {
                id,
                table,
                in_node,
                out_node,
                properties,
            });
        }

        ensure_remaining(data, 5)?;
        let length = data.get_u32();
        let has_cost = data.get_u8() != 0;
        let cost = if has_cost {
            ensure_remaining(data, 8)?;
            Some(data.get_f64())
        } else {
            None
        };

        Ok(GraphPathValue {
            nodes,
            edges,
            length,
            cost,
        })
    }
}

/// Value decoding errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueDecodeError {
    InsufficientData,
    InvalidUtf8,
    UnsupportedType(u8),
}

impl std::fmt::Display for ValueDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueDecodeError::InsufficientData => write!(f, "Insufficient data"),
            ValueDecodeError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
            ValueDecodeError::UnsupportedType(t) => write!(f, "Unsupported type: 0x{:02x}", t),
        }
    }
}

impl std::error::Error for ValueDecodeError {}

// Helper functions

fn encode_string(buf: &mut BytesMut, s: &str) {
    let bytes = s.as_bytes();
    buf.put_u32(bytes.len() as u32);
    buf.put_slice(bytes);
}

fn decode_string(data: &mut Bytes) -> Result<String, ValueDecodeError> {
    ensure_remaining(data, 4)?;
    let len = data.get_u32() as usize;
    ensure_remaining(data, len)?;
    String::from_utf8(data.copy_to_bytes(len).to_vec()).map_err(|_| ValueDecodeError::InvalidUtf8)
}

fn ensure_remaining(data: &Bytes, n: usize) -> Result<(), ValueDecodeError> {
    if data.remaining() < n {
        Err(ValueDecodeError::InsufficientData)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_roundtrip() {
        let values = vec![
            WireValue::Null,
            WireValue::Boolean(true),
            WireValue::Int32(42),
            WireValue::Int64(-123456789),
            WireValue::Float64(3.14159),
            WireValue::String("Hello, World!".to_string()),
        ];

        for value in values {
            let encoded = value.encode();
            let mut data = encoded;
            let decoded = WireValue::decode(&mut data).unwrap();
            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_array_roundtrip() {
        let value = WireValue::Array(vec![
            WireValue::Int32(1),
            WireValue::Int32(2),
            WireValue::String("three".to_string()),
        ]);

        let encoded = value.encode();
        let mut data = encoded;
        let decoded = WireValue::decode(&mut data).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_vector_roundtrip() {
        let value = WireValue::Vector(vec![0.1, 0.2, 0.3, 0.4, 0.5]);

        let encoded = value.encode();
        let mut data = encoded;
        let decoded = WireValue::decode(&mut data).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_point_roundtrip() {
        let value = WireValue::Point {
            x: -122.4194,
            y: 37.7749,
        };

        let encoded = value.encode();
        let mut data = encoded;
        let decoded = WireValue::decode(&mut data).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_graph_node_ref_roundtrip() {
        let value = WireValue::GraphNodeRef {
            table: "users".to_string(),
            id: "user:123".to_string(),
        };

        let encoded = value.encode();
        let mut data = encoded;
        let decoded = WireValue::decode(&mut data).unwrap();
        assert_eq!(value, decoded);
    }
}
