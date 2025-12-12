//! CQL wire protocol implementation
//!
//! This module implements the Cassandra wire protocol (version 4).

use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;

/// CQL protocol version
pub const PROTOCOL_VERSION: u8 = 4;

/// CQL frame header size (9 bytes)
pub const FRAME_HEADER_SIZE: usize = 9;

/// Frame flags
pub const FLAG_COMPRESSION: u8 = 0x01;
pub const FLAG_TRACING: u8 = 0x02;
pub const FLAG_CUSTOM_PAYLOAD: u8 = 0x04;
pub const FLAG_WARNING: u8 = 0x08;

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    None,
    Snappy,
    Lz4,
}

/// CQL frame
#[derive(Debug, Clone)]
pub struct CqlFrame {
    /// Protocol version
    pub version: u8,
    /// Flags
    pub flags: u8,
    /// Stream ID
    pub stream: i16,
    /// Opcode
    pub opcode: CqlOpcode,
    /// Frame body
    pub body: Bytes,
}

impl CqlFrame {
    /// Create a new frame
    pub fn new(opcode: CqlOpcode, body: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            flags: 0,
            stream: 0,
            opcode,
            body,
        }
    }

    /// Create a response frame
    pub fn response(stream: i16, opcode: CqlOpcode, body: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION | 0x80, // Set response bit
            flags: 0,
            stream,
            opcode,
            body,
        }
    }

    /// Encode frame to bytes
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + self.body.len());

        // Header
        buf.put_u8(self.version);
        buf.put_u8(self.flags);
        buf.put_i16(self.stream);
        buf.put_u8(self.opcode as u8);
        buf.put_u32(self.body.len() as u32);

        // Body
        buf.put(self.body.clone());

        buf
    }

    /// Decode frame from bytes
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.len() < FRAME_HEADER_SIZE {
            return Err(ProtocolError::IncompleteFrame);
        }

        let version = buf.get_u8();
        let flags = buf.get_u8();
        let stream = buf.get_i16();
        let opcode_byte = buf.get_u8();
        let length = buf.get_u32() as usize;

        if buf.len() < length {
            return Err(ProtocolError::IncompleteFrame);
        }

        let opcode = CqlOpcode::from_u8(opcode_byte)?;
        let body = buf.copy_to_bytes(length);

        Ok(Self {
            version,
            flags,
            stream,
            opcode,
            body,
        })
    }

    /// Check if this is a request frame
    pub fn is_request(&self) -> bool {
        self.version & 0x80 == 0
    }

    /// Check if this is a response frame
    pub fn is_response(&self) -> bool {
        self.version & 0x80 != 0
    }
}

/// CQL opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CqlOpcode {
    /// ERROR response
    Error = 0x00,
    /// STARTUP request
    Startup = 0x01,
    /// READY response
    Ready = 0x02,
    /// AUTHENTICATE response
    Authenticate = 0x03,
    /// OPTIONS request
    Options = 0x05,
    /// SUPPORTED response
    Supported = 0x06,
    /// QUERY request
    Query = 0x07,
    /// RESULT response
    Result = 0x08,
    /// PREPARE request
    Prepare = 0x09,
    /// EXECUTE request
    Execute = 0x0A,
    /// REGISTER request
    Register = 0x0B,
    /// EVENT response
    Event = 0x0C,
    /// BATCH request
    Batch = 0x0D,
    /// AUTH_CHALLENGE response
    AuthChallenge = 0x0E,
    /// AUTH_RESPONSE request
    AuthResponse = 0x0F,
    /// AUTH_SUCCESS response
    AuthSuccess = 0x10,
}

impl CqlOpcode {
    /// Convert u8 to opcode
    pub fn from_u8(byte: u8) -> ProtocolResult<Self> {
        match byte {
            0x00 => Ok(CqlOpcode::Error),
            0x01 => Ok(CqlOpcode::Startup),
            0x02 => Ok(CqlOpcode::Ready),
            0x03 => Ok(CqlOpcode::Authenticate),
            0x05 => Ok(CqlOpcode::Options),
            0x06 => Ok(CqlOpcode::Supported),
            0x07 => Ok(CqlOpcode::Query),
            0x08 => Ok(CqlOpcode::Result),
            0x09 => Ok(CqlOpcode::Prepare),
            0x0A => Ok(CqlOpcode::Execute),
            0x0B => Ok(CqlOpcode::Register),
            0x0C => Ok(CqlOpcode::Event),
            0x0D => Ok(CqlOpcode::Batch),
            0x0E => Ok(CqlOpcode::AuthChallenge),
            0x0F => Ok(CqlOpcode::AuthResponse),
            0x10 => Ok(CqlOpcode::AuthSuccess),
            _ => Err(ProtocolError::InvalidOpcode(byte)),
        }
    }
}

/// Consistency level for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[derive(Default)]
pub enum ConsistencyLevel {
    /// Any (write only)
    Any = 0x0000,
    /// One replica
    #[default]
    One = 0x0001,
    /// Two replicas
    Two = 0x0002,
    /// Three replicas
    Three = 0x0003,
    /// Quorum (majority)
    Quorum = 0x0004,
    /// All replicas
    All = 0x0005,
    /// Local quorum
    LocalQuorum = 0x0006,
    /// Each quorum
    EachQuorum = 0x0007,
    /// Serial (lightweight transaction)
    Serial = 0x0008,
    /// Local serial
    LocalSerial = 0x0009,
    /// Local one
    LocalOne = 0x000A,
}

impl ConsistencyLevel {
    /// Convert u16 to consistency level
    pub fn from_u16(value: u16) -> ProtocolResult<Self> {
        match value {
            0x0000 => Ok(ConsistencyLevel::Any),
            0x0001 => Ok(ConsistencyLevel::One),
            0x0002 => Ok(ConsistencyLevel::Two),
            0x0003 => Ok(ConsistencyLevel::Three),
            0x0004 => Ok(ConsistencyLevel::Quorum),
            0x0005 => Ok(ConsistencyLevel::All),
            0x0006 => Ok(ConsistencyLevel::LocalQuorum),
            0x0007 => Ok(ConsistencyLevel::EachQuorum),
            0x0008 => Ok(ConsistencyLevel::Serial),
            0x0009 => Ok(ConsistencyLevel::LocalSerial),
            0x000A => Ok(ConsistencyLevel::LocalOne),
            _ => Err(ProtocolError::InvalidConsistencyLevel(value)),
        }
    }
}

/// Query parameters
#[derive(Debug, Clone)]
pub struct QueryParameters {
    /// Consistency level
    pub consistency: ConsistencyLevel,
    /// Values for bound parameters
    pub values: Option<Vec<Bytes>>,
    /// Skip metadata in response
    pub skip_metadata: bool,
    /// Page size
    pub page_size: Option<i32>,
    /// Paging state
    pub paging_state: Option<Bytes>,
    /// Serial consistency
    pub serial_consistency: Option<ConsistencyLevel>,
    /// Default timestamp
    pub default_timestamp: Option<i64>,
}

impl Default for QueryParameters {
    fn default() -> Self {
        Self {
            consistency: ConsistencyLevel::One,
            values: None,
            skip_metadata: false,
            page_size: None,
            paging_state: None,
            serial_consistency: None,
            default_timestamp: None,
        }
    }
}

impl QueryParameters {
    /// Decode query parameters from bytes
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.remaining() < 2 {
            return Err(ProtocolError::IncompleteFrame);
        }
        let consistency = ConsistencyLevel::from_u16(buf.get_u16())?;

        if buf.remaining() < 1 {
            return Err(ProtocolError::IncompleteFrame);
        }
        let flags = buf.get_u8();

        let values = if flags & 0x01 != 0 {
            if buf.remaining() < 2 {
                return Err(ProtocolError::IncompleteFrame);
            }
            let count = buf.get_u16();
            let mut vals = Vec::with_capacity(count as usize);
            for _ in 0..count {
                if buf.remaining() < 4 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let len = buf.get_i32();
                if len >= 0 {
                    if buf.remaining() < len as usize {
                        return Err(ProtocolError::IncompleteFrame);
                    }
                    let value = buf.copy_to_bytes(len as usize);
                    vals.push(value);
                } else {
                    vals.push(Bytes::new());
                }
            }
            Some(vals)
        } else {
            None
        };

        let skip_metadata = flags & 0x02 != 0;

        let page_size = if flags & 0x04 != 0 {
            if buf.remaining() < 4 {
                return Err(ProtocolError::IncompleteFrame);
            }
            Some(buf.get_i32())
        } else {
            None
        };

        let paging_state = if flags & 0x08 != 0 {
            if buf.remaining() < 4 {
                return Err(ProtocolError::IncompleteFrame);
            }
            let len = buf.get_i32();
            if len < 0 {
                return Err(ProtocolError::IncompleteFrame);
            }
            if buf.remaining() < len as usize {
                return Err(ProtocolError::IncompleteFrame);
            }
            Some(buf.copy_to_bytes(len as usize))
        } else {
            None
        };

        let serial_consistency = if flags & 0x10 != 0 {
            if buf.remaining() < 2 {
                return Err(ProtocolError::IncompleteFrame);
            }
            Some(ConsistencyLevel::from_u16(buf.get_u16())?)
        } else {
            None
        };

        let default_timestamp = if flags & 0x20 != 0 {
            if buf.remaining() < 8 {
                return Err(ProtocolError::IncompleteFrame);
            }
            Some(buf.get_i64())
        } else {
            None
        };

        Ok(Self {
            consistency,
            values,
            skip_metadata,
            page_size,
            paging_state,
            serial_consistency,
            default_timestamp,
        })
    }
}

/// Result kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ResultKind {
    /// Void result
    Void = 0x0001,
    /// Rows result
    Rows = 0x0002,
    /// Set keyspace result
    SetKeyspace = 0x0003,
    /// Prepared statement result
    Prepared = 0x0004,
    /// Schema change result
    SchemaChange = 0x0005,
}

/// Build a READY response
pub fn build_ready_response(stream: i16) -> CqlFrame {
    CqlFrame::response(stream, CqlOpcode::Ready, Bytes::new())
}

/// Build a SUPPORTED response
pub fn build_supported_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();

    // String map of supported options
    let options: HashMap<String, Vec<String>> = [
        ("CQL_VERSION".to_string(), vec!["3.4.5".to_string()]),
        (
            "COMPRESSION".to_string(),
            vec!["snappy".to_string(), "lz4".to_string()],
        ),
    ]
    .iter()
    .cloned()
    .collect();

    // Encode string multimap
    body.put_u16(options.len() as u16);
    for (key, values) in options {
        write_string(&mut body, &key);
        body.put_u16(values.len() as u16);
        for value in values {
            write_string(&mut body, &value);
        }
    }

    CqlFrame::response(stream, CqlOpcode::Supported, body.freeze())
}

/// Build a RESULT response with VOID
pub fn build_void_result(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Void as i32);
    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with empty ROWS
pub fn build_empty_rows_result(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);
    // Metadata: flags=0, columns_count=0
    body.put_i32(0); // flags
    body.put_i32(0); // columns_count
                     // Row count: 0
    body.put_i32(0);
    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system.local data
pub fn build_system_local_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata
    // Flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 13 (adding broadcast_address, listen_address, rpc_address)
    body.put_i32(13);

    // Global table spec
    write_string(&mut body, "system"); // keyspace
    write_string(&mut body, "local"); // table

    // Column specs (name, type)
    write_string(&mut body, "key");
    body.put_u16(0x000D); // text

    write_string(&mut body, "cluster_name");
    body.put_u16(0x000D); // text

    write_string(&mut body, "partitioner");
    body.put_u16(0x000D); // text

    write_string(&mut body, "cql_version");
    body.put_u16(0x000D); // text

    write_string(&mut body, "release_version");
    body.put_u16(0x000D); // text

    write_string(&mut body, "data_center");
    body.put_u16(0x000D); // text

    write_string(&mut body, "rack");
    body.put_u16(0x000D); // text

    write_string(&mut body, "tokens");
    body.put_u16(0x0022); // set
    body.put_u16(0x000D); // <text>

    write_string(&mut body, "schema_version");
    body.put_u16(0x000C); // uuid

    write_string(&mut body, "host_id");
    body.put_u16(0x000C); // uuid

    // Additional critical columns for cassandra-driver
    write_string(&mut body, "broadcast_address");
    body.put_u16(0x0010); // inet

    write_string(&mut body, "listen_address");
    body.put_u16(0x0010); // inet

    write_string(&mut body, "rpc_address");
    body.put_u16(0x0010); // inet

    // Row count: 1
    body.put_i32(1);

    // Row 1 values
    // key: 'local'
    body.put_i32(5); // len
    body.put(&b"local"[..]);

    // cluster_name: 'orbit'
    body.put_i32(5); // len
    body.put(&b"orbit"[..]);

    // partitioner
    let part = "org.apache.cassandra.dht.Murmur3Partitioner";
    body.put_i32(part.len() as i32);
    body.put(part.as_bytes());

    // cql_version: '3.4.4'
    let cql_ver = "3.4.4";
    body.put_i32(cql_ver.len() as i32);
    body.put(cql_ver.as_bytes());

    // release_version: '4.0.0'
    let rel_ver = "4.0.0";
    body.put_i32(rel_ver.len() as i32);
    body.put(rel_ver.as_bytes());

    // data_center: 'datacenter1'
    let dc = "datacenter1";
    body.put_i32(dc.len() as i32);
    body.put(dc.as_bytes());

    // rack: 'rack1'
    let rack = "rack1";
    body.put_i32(rack.len() as i32);
    body.put(rack.as_bytes());

    // tokens: {'0'}
    // set<text> is encoded as: [n][len][bytes]...
    // But wait, value encoding is: [len][bytes].
    // bytes for set is: [n][len][bytes]...
    // So: [total_len][n][len][bytes]...
    // total_len = 4 (n) + 4 (len) + 1 (byte) = 9 bytes.
    body.put_i32(9); // total value len
    body.put_i32(1); // 1 element
    body.put_i32(1); // len of element
    body.put(&b"0"[..]);

    // schema_version: uuid (16 bytes)
    body.put_i32(16); // len
                      // random uuid
    body.put(&b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"[..]);

    // host_id: uuid (16 bytes)
    body.put_i32(16); // len
                      // random uuid (different from schema_version just in case)
    body.put(&b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01"[..]);

    // broadcast_address: 127.0.0.1 (IPv4 = 4 bytes)
    body.put_i32(4); // len
    body.put(&[127, 0, 0, 1][..]);

    // listen_address: 127.0.0.1 (IPv4 = 4 bytes)
    body.put_i32(4); // len
    body.put(&[127, 0, 0, 1][..]);

    // rpc_address: 127.0.0.1 (IPv4 = 4 bytes)
    body.put_i32(4); // len
    body.put(&[127, 0, 0, 1][..]);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system.peers_v2 data (empty)
pub fn build_system_peers_v2_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata
    // Flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 7
    body.put_i32(7);

    // Global table spec
    write_string(&mut body, "system"); // keyspace
    write_string(&mut body, "peers_v2"); // table

    // Column specs (name, type)
    write_string(&mut body, "peer");
    body.put_u16(0x0010); // inet

    write_string(&mut body, "peer_port");
    body.put_u16(0x0009); // int

    write_string(&mut body, "data_center");
    body.put_u16(0x000D); // text

    write_string(&mut body, "rack");
    body.put_u16(0x000D); // text

    write_string(&mut body, "tokens");
    body.put_u16(0x0022); // set
    body.put_u16(0x000D); // <text>

    write_string(&mut body, "schema_version");
    body.put_u16(0x000C); // uuid

    write_string(&mut body, "host_id");
    body.put_u16(0x000C); // uuid

    // Row count: 0
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.keyspaces data
pub fn build_system_schema_keyspaces_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 3 (keyspace_name, durable_writes, replication)
    body.put_i32(3);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "keyspaces");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text

    write_string(&mut body, "durable_writes");
    body.put_u16(0x0004); // boolean

    write_string(&mut body, "replication");
    body.put_u16(0x0021); // map<text, text>
    body.put_u16(0x000D); // key type: text
    body.put_u16(0x000D); // value type: text

    // Row count: 2 (system, system_schema)
    body.put_i32(2);

    // Helper function to write a CQL map<text, text> value
    // Format: [total_len][n_pairs][key_len][key_bytes][val_len][val_bytes]...
    fn write_map_value(body: &mut BytesMut, pairs: &[(&str, &str)]) {
        let mut map_body = BytesMut::new();
        map_body.put_i32(pairs.len() as i32);
        for (key, val) in pairs {
            map_body.put_i32(key.len() as i32);
            map_body.put(key.as_bytes());
            map_body.put_i32(val.len() as i32);
            map_body.put(val.as_bytes());
        }
        body.put_i32(map_body.len() as i32);
        body.put(map_body);
    }

    // Row 1: system keyspace
    // keyspace_name: 'system'
    body.put_i32(6);
    body.put(&b"system"[..]);
    // durable_writes: true (1 byte)
    body.put_i32(1);
    body.put_u8(1);
    // replication: {'class': 'LocalStrategy'}
    write_map_value(&mut body, &[("class", "LocalStrategy")]);

    // Row 2: system_schema keyspace
    // keyspace_name: 'system_schema'
    body.put_i32(13);
    body.put(&b"system_schema"[..]);
    // durable_writes: true (1 byte)
    body.put_i32(1);
    body.put_u8(1);
    // replication: {'class': 'LocalStrategy'}
    write_map_value(&mut body, &[("class", "LocalStrategy")]);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.tables data (empty)
pub fn build_system_schema_tables_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 2 (keyspace_name, table_name)
    body.put_i32(2);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "tables");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text

    write_string(&mut body, "table_name");
    body.put_u16(0x000D); // text

    // Row count: 0 (empty - no tables defined yet)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.columns data (empty)
pub fn build_system_schema_columns_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 4 (keyspace_name, table_name, column_name, type)
    body.put_i32(4);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "columns");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "table_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "column_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "type");
    body.put_u16(0x000D); // text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.types data (empty)
pub fn build_system_schema_types_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 3 (keyspace_name, type_name, field_names)
    body.put_i32(3);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "types");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "type_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "field_names");
    body.put_u16(0x0020); // list<text>
    body.put_u16(0x000D); // element type: text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.functions data (empty)
pub fn build_system_schema_functions_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 3 (keyspace_name, function_name, argument_types)
    body.put_i32(3);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "functions");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "function_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "argument_types");
    body.put_u16(0x0020); // list<text>
    body.put_u16(0x000D); // element type: text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.aggregates data (empty)
pub fn build_system_schema_aggregates_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 3 (keyspace_name, aggregate_name, argument_types)
    body.put_i32(3);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "aggregates");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "aggregate_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "argument_types");
    body.put_u16(0x0020); // list<text>
    body.put_u16(0x000D); // element type: text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Helper to write a string in [short] format (u16 length + bytes)
pub fn write_string(buf: &mut BytesMut, s: &str) {
    buf.put_u16(s.len() as u16);
    buf.put(s.as_bytes());
}

/// Helper to write a string list in [short] format (u16 count + strings)
pub fn write_string_list(buf: &mut BytesMut, list: &[String]) {
    buf.put_u16(list.len() as u16);
    for s in list {
        write_string(buf, s);
    }
}

/// Build a RESULT response with system_schema.views data (empty)
pub fn build_system_schema_views_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 3 (keyspace_name, view_name, base_table_name)
    body.put_i32(3);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "views");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "view_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "base_table_name");
    body.put_u16(0x000D); // text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.indexes data (empty)
pub fn build_system_schema_indexes_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 4 (keyspace_name, table_name, index_name, kind)
    body.put_i32(4);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "indexes");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "table_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "index_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "kind");
    body.put_u16(0x000D); // text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_schema.triggers data (empty)
pub fn build_system_schema_triggers_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);
    // Column count: 4 (keyspace_name, table_name, trigger_name, options)
    body.put_i32(4);

    // Global table spec
    write_string(&mut body, "system_schema");
    write_string(&mut body, "triggers");

    // Column specs
    write_string(&mut body, "keyspace_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "table_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "trigger_name");
    body.put_u16(0x000D); // text
    write_string(&mut body, "options");
    body.put_u16(0x0021); // map<text, text>
    body.put_u16(0x000D); // key type: text
    body.put_u16(0x000D); // value type: text

    // Row count: 0 (empty)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// Build a RESULT response with system_virtual_schema tables (empty)
/// These are Cassandra 4.0+ virtual tables for internal metrics
pub fn build_system_virtual_schema_response(stream: i16, table_name: &str) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Rows as i32);

    // Determine table type from full table name
    let table_suffix = table_name
        .strip_prefix("system_virtual_schema.")
        .unwrap_or(table_name);

    // Metadata flags: 0x0001 (Global_tables_spec)
    body.put_i32(0x0001);

    match table_suffix {
        "keyspaces" => {
            // Column count: 1 (keyspace_name)
            body.put_i32(1);
            // Global table spec
            write_string(&mut body, "system_virtual_schema");
            write_string(&mut body, "keyspaces");
            // Column specs
            write_string(&mut body, "keyspace_name");
            body.put_u16(0x000D); // text
        }
        "tables" => {
            // Column count: 3 (keyspace_name, table_name, comment)
            body.put_i32(3);
            // Global table spec
            write_string(&mut body, "system_virtual_schema");
            write_string(&mut body, "tables");
            // Column specs
            write_string(&mut body, "keyspace_name");
            body.put_u16(0x000D); // text
            write_string(&mut body, "table_name");
            body.put_u16(0x000D); // text
            write_string(&mut body, "comment");
            body.put_u16(0x000D); // text
        }
        "columns" => {
            // Column count: 5 (keyspace_name, table_name, column_name, clustering_order, type)
            body.put_i32(5);
            // Global table spec
            write_string(&mut body, "system_virtual_schema");
            write_string(&mut body, "columns");
            // Column specs
            write_string(&mut body, "keyspace_name");
            body.put_u16(0x000D); // text
            write_string(&mut body, "table_name");
            body.put_u16(0x000D); // text
            write_string(&mut body, "column_name");
            body.put_u16(0x000D); // text
            write_string(&mut body, "clustering_order");
            body.put_u16(0x000D); // text
            write_string(&mut body, "type");
            body.put_u16(0x000D); // text
        }
        _ => {
            // Generic empty response for unknown virtual schema tables
            body.put_i32(1);
            write_string(&mut body, "system_virtual_schema");
            write_string(&mut body, table_suffix);
            write_string(&mut body, "name");
            body.put_u16(0x000D); // text
        }
    }

    // Row count: 0 (empty for all virtual schema tables)
    body.put_i32(0);

    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// CQL error codes (from Cassandra native protocol v4)
pub mod error_codes {
    /// Server error (generic)
    pub const SERVER_ERROR: i32 = 0x0000;
    /// Protocol error
    pub const PROTOCOL_ERROR: i32 = 0x000A;
    /// Bad credentials
    pub const BAD_CREDENTIALS: i32 = 0x0100;
    /// Unavailable exception
    pub const UNAVAILABLE: i32 = 0x1000;
    /// Overloaded
    pub const OVERLOADED: i32 = 0x1100;
    /// Is bootstrapping
    pub const IS_BOOTSTRAPPING: i32 = 0x1200;
    /// Truncate error
    pub const TRUNCATE_ERROR: i32 = 0x1300;
    /// Write timeout
    pub const WRITE_TIMEOUT: i32 = 0x2000;
    /// Read timeout
    pub const READ_TIMEOUT: i32 = 0x2100;
    /// Syntax error
    pub const SYNTAX_ERROR: i32 = 0x2200;
    /// Unauthorized
    pub const UNAUTHORIZED: i32 = 0x2300;
    /// Invalid (invalid query, invalid request, etc.)
    pub const INVALID: i32 = 0x2400;
    /// Config error
    pub const CONFIG_ERROR: i32 = 0x2500;
    /// Already exists (keyspace, table, etc.)
    pub const ALREADY_EXISTS: i32 = 0x2600;
    /// Unprepared (prepared statement not found)
    pub const UNPREPARED: i32 = 0x2700;
}

/// Map ProtocolError to CQL error code
pub fn map_error_to_cql_code(error: &crate::protocols::error::ProtocolError) -> i32 {
    use crate::protocols::error::ProtocolError;
    use error_codes::*;

    match error {
        ProtocolError::ParseError(_) => SYNTAX_ERROR,
        ProtocolError::CqlError(_) => PROTOCOL_ERROR,
        ProtocolError::PostgresError(msg) => {
            // Check for specific SQL errors
            let msg_lower = msg.to_lowercase();
            if msg_lower.contains("does not exist") || msg_lower.contains("not found") {
                INVALID
            } else if msg_lower.contains("already exists") || msg_lower.contains("duplicate") {
                // ALREADY_EXISTS requires [keyspace, table] which we don't have here
                // Map to INVALID to avoid client crash due to missing fields
                INVALID
            } else if msg_lower.contains("syntax") || msg_lower.contains("parse") {
                SYNTAX_ERROR
            } else if msg_lower.contains("unauthorized") || msg_lower.contains("permission") {
                UNAUTHORIZED
            } else if msg_lower.contains("timeout") {
                // Timeouts require extra fields, map to SERVER_ERROR for safety
                SERVER_ERROR
            } else {
                INVALID
            }
        }
        ProtocolError::AuthenticationError(_) => BAD_CREDENTIALS,
        ProtocolError::AuthorizationError(_) => UNAUTHORIZED,
        ProtocolError::InvalidOpcode(_) => PROTOCOL_ERROR,
        ProtocolError::InvalidConsistencyLevel(_) => PROTOCOL_ERROR,
        ProtocolError::IncompleteFrame => PROTOCOL_ERROR,
        ProtocolError::InvalidUtf8(_) => PROTOCOL_ERROR,
        ProtocolError::InvalidStatement(_) => SYNTAX_ERROR,
        // UNAVAILABLE requires extra fields, map to SERVER_ERROR
        ProtocolError::ConnectionError(_) => SERVER_ERROR,
        ProtocolError::ConnectionClosed => SERVER_ERROR,
        _ => SERVER_ERROR,
    }
}

/// Build an ERROR response
pub fn build_error_response(stream: i16, code: i32, message: &str) -> CqlFrame {
    tracing::error!(
        "Building error response: stream={}, code={:#x}, message='{}'",
        stream,
        code,
        message
    );
    let mut body = BytesMut::new();
    body.put_i32(code);
    write_string(&mut body, message);
    CqlFrame::response(stream, CqlOpcode::Error, body.freeze())
}

/// Build an ERROR response from a ProtocolError
pub fn build_error_from_protocol_error(
    stream: i16,
    error: &crate::protocols::error::ProtocolError,
) -> CqlFrame {
    let error_code = map_error_to_cql_code(error);
    let message = error.to_string();
    build_error_response(stream, error_code, &message)
}

/// Write a CQL string (2-byte length + UTF-8 bytes)

/// Read a CQL string
pub fn read_string(buf: &mut Bytes) -> ProtocolResult<String> {
    if buf.remaining() < 2 {
        return Err(ProtocolError::IncompleteFrame);
    }
    let len = buf.get_u16();
    if buf.remaining() < len as usize {
        return Err(ProtocolError::IncompleteFrame);
    }
    let bytes = buf.copy_to_bytes(len as usize);
    String::from_utf8(bytes.to_vec()).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))
}

/// Read a CQL string map
pub fn read_string_map(buf: &mut Bytes) -> ProtocolResult<HashMap<String, String>> {
    if buf.remaining() < 2 {
        return Err(ProtocolError::IncompleteFrame);
    }
    let count = buf.get_u16();
    let mut map = HashMap::new();
    for _ in 0..count {
        let key = read_string(buf)?;
        let value = read_string(buf)?;
        map.insert(key, value);
    }
    Ok(map)
}

/// Read a CQL string list
pub fn read_string_list(buf: &mut Bytes) -> ProtocolResult<Vec<String>> {
    if buf.remaining() < 2 {
        return Err(ProtocolError::IncompleteFrame);
    }
    let count = buf.get_u16();
    let mut list = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let value = read_string(buf)?;
        list.push(value);
    }
    Ok(list)
}

/// Compress data using the specified algorithm
pub fn compress_data(data: &[u8], algorithm: CompressionAlgorithm) -> ProtocolResult<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Snappy => {
            let mut encoder = snap::raw::Encoder::new();
            encoder
                .compress_vec(data)
                .map_err(|e| ProtocolError::CqlError(format!("Snappy compression failed: {}", e)))
        }
        CompressionAlgorithm::Lz4 => {
            // LZ4 block compression
            Ok(lz4_flex::compress_prepend_size(data))
        }
    }
}

/// Decompress data using the specified algorithm
pub fn decompress_data(data: &[u8], algorithm: CompressionAlgorithm) -> ProtocolResult<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Snappy => {
            let mut decoder = snap::raw::Decoder::new();
            decoder
                .decompress_vec(data)
                .map_err(|e| ProtocolError::CqlError(format!("Snappy decompression failed: {}", e)))
        }
        CompressionAlgorithm::Lz4 => {
            // LZ4 block decompression
            lz4_flex::decompress_size_prepended(data)
                .map_err(|e| ProtocolError::CqlError(format!("LZ4 decompression failed: {}", e)))
        }
    }
}

/// Create a compressed response frame
pub fn create_compressed_response(
    stream: i16,
    opcode: CqlOpcode,
    body: Bytes,
    compression: CompressionAlgorithm,
) -> ProtocolResult<CqlFrame> {
    if compression == CompressionAlgorithm::None {
        return Ok(CqlFrame::response(stream, opcode, body));
    }

    let compressed_body = compress_data(&body, compression)?;
    let mut frame = CqlFrame::response(stream, opcode, Bytes::from(compressed_body));
    frame.flags |= FLAG_COMPRESSION;
    Ok(frame)
}

/// Decode a potentially compressed frame
pub fn decode_with_compression(
    mut buf: Bytes,
    compression: CompressionAlgorithm,
) -> ProtocolResult<CqlFrame> {
    if buf.len() < FRAME_HEADER_SIZE {
        return Err(ProtocolError::IncompleteFrame);
    }

    let version = buf.get_u8();
    let flags = buf.get_u8();
    let stream = buf.get_i16();
    let opcode_byte = buf.get_u8();
    let length = buf.get_u32() as usize;

    if buf.len() < length {
        return Err(ProtocolError::IncompleteFrame);
    }

    let opcode = CqlOpcode::from_u8(opcode_byte)?;
    let mut body_bytes = buf.copy_to_bytes(length);

    // Decompress if compression flag is set
    if flags & FLAG_COMPRESSION != 0 && compression != CompressionAlgorithm::None {
        let decompressed = decompress_data(&body_bytes, compression)?;
        body_bytes = Bytes::from(decompressed);
    }

    Ok(CqlFrame {
        version,
        flags,
        stream,
        opcode,
        body: body_bytes,
    })
}

use crate::protocols::cql::types::CqlEvent;

/// Register message
#[derive(Debug, Clone)]
pub struct RegisterMessage {
    pub event_types: Vec<String>,
}

impl RegisterMessage {
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.remaining() < 2 {
            return Err(ProtocolError::IncompleteFrame);
        }
        let count = buf.get_u16();
        let mut event_types = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if buf.remaining() < 2 {
                return Err(ProtocolError::IncompleteFrame);
            }
            let len = buf.get_u16() as usize;
            if buf.remaining() < len {
                return Err(ProtocolError::IncompleteFrame);
            }
            let s = buf.copy_to_bytes(len);
            let s_str = String::from_utf8(s.to_vec())
                .map_err(|e| ProtocolError::ConversionError(e.to_string()))?;
            event_types.push(s_str);
        }

        Ok(Self { event_types })
    }
}

/// Build an EVENT response
pub fn build_event_response(stream: i16, event: CqlEvent) -> ProtocolResult<CqlFrame> {
    let mut body = BytesMut::new();

    match event {
        CqlEvent::TopologyChange(change_type, addr) => {
            write_string(&mut body, "TOPOLOGY_CHANGE");
            write_string(&mut body, change_type.as_str());
            // Inet encoding: [1 byte len][4 or 16 bytes]
            // + [4 bytes port]
            match addr.ip() {
                std::net::IpAddr::V4(ipv4) => {
                    body.put_u8(4);
                    body.put(&ipv4.octets()[..]);
                }
                std::net::IpAddr::V6(ipv6) => {
                    body.put_u8(16);
                    body.put(&ipv6.octets()[..]);
                }
            }
            body.put_i32(addr.port() as i32);
        }
        CqlEvent::StatusChange(change_type, addr) => {
            write_string(&mut body, "STATUS_CHANGE");
            write_string(&mut body, change_type.as_str());
            match addr.ip() {
                std::net::IpAddr::V4(ipv4) => {
                    body.put_u8(4);
                    body.put(&ipv4.octets()[..]);
                }
                std::net::IpAddr::V6(ipv6) => {
                    body.put_u8(16);
                    body.put(&ipv6.octets()[..]);
                }
            }
            body.put_i32(addr.port() as i32);
        }
        CqlEvent::SchemaChange(change_type, keyspace, name, target_type) => {
            write_string(&mut body, "SCHEMA_CHANGE");
            write_string(&mut body, change_type.as_str());
            write_string(&mut body, &target_type);
            write_string(&mut body, &keyspace);

            // KEYSPACE target only needs keyspace
            // TABLE, TYPE, etc need name as well
            if target_type != "KEYSPACE" {
                write_string(&mut body, &name);
            }
        }
    }

    Ok(CqlFrame::response(stream, CqlOpcode::Event, body.freeze()))
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = CqlFrame::new(CqlOpcode::Query, Bytes::from("SELECT * FROM users"));
        let encoded = frame.encode();
        let decoded = CqlFrame::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.opcode, CqlOpcode::Query);
        assert_eq!(decoded.body, Bytes::from("SELECT * FROM users"));
    }

    #[test]
    fn test_opcode_conversion() {
        assert_eq!(CqlOpcode::from_u8(0x07).unwrap(), CqlOpcode::Query);
        assert_eq!(CqlOpcode::from_u8(0x08).unwrap(), CqlOpcode::Result);
        assert!(CqlOpcode::from_u8(0xFF).is_err());
    }

    #[test]
    fn test_consistency_level() {
        assert_eq!(
            ConsistencyLevel::from_u16(0x0001).unwrap(),
            ConsistencyLevel::One
        );
        assert_eq!(
            ConsistencyLevel::from_u16(0x0004).unwrap(),
            ConsistencyLevel::Quorum
        );
    }
}
