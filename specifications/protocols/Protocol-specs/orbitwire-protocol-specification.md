# OrbitWire Protocol Specification

## Document Information

| Field | Value |
|-------|-------|
| **Version** | 1.0.0 |
| **Status** | Draft |
| **Created** | 2025-12-08 |
| **Authors** | Orbit-RS Team |
| **License** | BSD-3-Clause OR MIT |

---

## Table of Contents

1. [Overview](#overview)
2. [Design Goals](#design-goals)
3. [Protocol Architecture](#protocol-architecture)
4. [Connection Lifecycle](#connection-lifecycle)
5. [Message Format](#message-format)
6. [Message Types](#message-types)
7. [Authentication](#authentication)
8. [Query Execution](#query-execution)
9. [Prepared Statements](#prepared-statements)
10. [Transactions](#transactions)
11. [LIVE Queries](#live-queries)
12. [Graph Operations](#graph-operations)
13. [Vector Operations](#vector-operations)
14. [Error Handling](#error-handling)
15. [Compression](#compression)
16. [Implementation Guide](#implementation-guide)
17. [Wire Format Reference](#wire-format-reference)

---

## Overview

### Purpose

OrbitWire is a purpose-built binary wire protocol designed specifically for Orbit-RS and OrbitQL. It provides optimal performance for the unique features of OrbitQL including LIVE queries, graph traversals, vector operations, and real-time subscriptions.

### Key Features

| Feature | Description |
|---------|-------------|
| **Lightweight** | Minimal overhead for small queries |
| **Streaming Native** | First-class support for LIVE queries and subscriptions |
| **Graph Optimized** | Efficient serialization for graph paths and traversals |
| **Vector Support** | Native encoding for high-dimensional vectors |
| **Multiplexed** | Multiple concurrent queries on single connection |
| **Bidirectional** | Full duplex communication for real-time updates |
| **Extensible** | Version-negotiated feature extensions |

### Protocol Stack

```text
┌─────────────────────────────────────────────────────────────┐
│                    Client Application                       │
├─────────────────────────────────────────────────────────────┤
│                   OrbitWire Client SDK                      │
├─────────────────────────────────────────────────────────────┤
│                   OrbitWire Protocol                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Framing   │  │  Messages   │  │    Serialization    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    TLS 1.3 (optional)                       │
├─────────────────────────────────────────────────────────────┤
│                       TCP / QUIC                            │
└─────────────────────────────────────────────────────────────┘
```

---

## Design Goals

### Primary Goals

1. **Performance**: Sub-millisecond overhead for simple queries
2. **Real-Time**: Native support for LIVE queries and push notifications
3. **Multi-Model**: Efficient encoding for graphs, vectors, and documents
4. **Simplicity**: Easy to implement in any language
5. **Debuggability**: Human-readable debug mode available

### Non-Goals

1. **SQL Compatibility**: Not designed to emulate PostgreSQL/MySQL wire protocols
2. **Legacy Support**: Fresh design without backwards compatibility constraints
3. **Universal Client**: Optimized specifically for OrbitQL features

---

## Protocol Architecture

### Connection Model

```text
┌──────────────────────────────────────────────────────────────┐
│                        Connection                            │
│  ┌────────────────────────────────────────────────────────┐  │ 
│  │                    Control Channel                     │  │
│  │  - Authentication                                      │  │
│  │  - Connection settings                                 │  │
│  │  - Heartbeat/Keepalive                                 │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│  │   Query 1   │  │   Query 2   │  │   Query N   │  ...      │
│  │   Stream    │  │   Stream    │  │   Stream    │           │
│  └─────────────┘  └─────────────┘  └─────────────┘           │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │               Subscription Channels                    │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                 │  │
│  │  │ LIVE 1  │  │ LIVE 2  │  │ LIVE N  │  ...            │  │
│  │  └─────────┘  └─────────┘  └─────────┘                 │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### Stream Multiplexing

Each query/subscription gets a unique stream ID, allowing multiple concurrent operations on a single TCP connection.

```text
┌────────────────────────────────────────────────────────────┐
│                     TCP Connection                         │
├────────────────────────────────────────────────────────────┤
│ Stream 0 (Control)  │ Auth, Settings, Heartbeat            │
├────────────────────────────────────────────────────────────┤
│ Stream 1            │ SELECT * FROM users                  │
├────────────────────────────────────────────────────────────┤
│ Stream 2            │ LIVE SELECT * FROM orders            │
├────────────────────────────────────────────────────────────┤
│ Stream 3            │ INSERT INTO logs ...                 │
├────────────────────────────────────────────────────────────┤
│ Stream N            │ ...                                  │
└────────────────────────────────────────────────────────────┘
```

---

## Connection Lifecycle

### Connection Establishment

```text
Client                                           Server
   │                                                │
   │──── TCP Connect ──────────────────────────────▶│
   │                                                │
   │──── Hello ────────────────────────────────────▶│
   │        { version, capabilities, compression }  │
   │                                                │
   │◀─── HelloAck ──────────────────────────────-───│
   │        { version, capabilities, compression }  │
   │                                                │
   │──── Authenticate ─────────────────────────────▶│
   │        { method, credentials }                 │
   │                                                │
   │◀─── AuthResult ────────────────────────────────│
   │        { success, token, user_info }           │
   │                                                │
   │     Connection Established                     │
   │                                                │
```

### Normal Operation

```text
Client                                           Server
   │                                                │
   │──── Query ────────────────────────────────────▶│
   │        { stream_id: 1, sql: "SELECT ..." }     │
   │                                                │
   │◀─── RowDescription ────────────────────────────│
   │        { stream_id: 1, columns: [...] }        │
   │                                                │
   │◀─── DataRow ───────────────────────────────────│
   │        { stream_id: 1, values: [...] }         │
   │                                                │
   │◀─── DataRow ───────────────────────────────────│
   │        { stream_id: 1, values: [...] }         │
   │                                                │
   │◀─── CommandComplete ───────────────────────────│
   │        { stream_id: 1, tag: "SELECT", rows: 2 }│
   │                                                │
```

### Connection Termination

```text
Client                                           Server
   │                                                │
   │──── Terminate ────────────────────────────────▶│
   │                                                │
   │◀─── TerminateAck ─────────────────────────-────│
   │                                                │
   │──── TCP Close ────────────────────────────────▶│
   │                                                │
```

---

## Message Format

### Frame Structure

All messages are wrapped in a frame:

```text
┌────────────────────────────────────────────────────────────┐
│                        Frame Header                        │
├────────────┬────────────┬────────────┬─────────────────────┤
│   Magic    │  Version   │   Flags    │      Length         │
│  (2 bytes) │  (1 byte)  │  (1 byte)  │     (4 bytes)       │
├────────────┴────────────┴────────────┴─────────────────────┤
│                      Stream ID (4 bytes)                   │
├────────────────────────────────────────────────────────────┤
│                    Message Type (2 bytes)                  │
├────────────────────────────────────────────────────────────┤
│                                                            │
│                     Payload (variable)                     │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Frame Header Fields

| Field | Size | Description |
|-------|------|-------------|
| Magic | 2 bytes | `0x4F52` ("OR" for Orbit) |
| Version | 1 byte | Protocol version (currently 0x01) |
| Flags | 1 byte | Frame flags (compression, etc.) |
| Length | 4 bytes | Payload length (big-endian) |
| Stream ID | 4 bytes | Stream identifier (0 = control) |
| Message Type | 2 bytes | Type of message |

### Frame Flags

| Bit | Name | Description |
|-----|------|-------------|
| 0 | COMPRESSED | Payload is compressed |
| 1 | ENCRYPTED | Payload is encrypted |
| 2 | CONTINUED | Message continues in next frame |
| 3 | END_STREAM | Last message for this stream |
| 4-7 | Reserved | Future use |

### Rust Frame Implementation

```rust
use bytes::{Buf, BufMut, Bytes, BytesMut};

pub const MAGIC: u16 = 0x4F52; // "OR"
pub const VERSION: u8 = 0x01;

#[derive(Debug, Clone)]
pub struct Frame {
    pub flags: FrameFlags,
    pub stream_id: u32,
    pub message_type: MessageType,
    pub payload: Bytes,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameFlags(u8);

impl FrameFlags {
    pub const NONE: Self = Self(0);
    pub const COMPRESSED: Self = Self(1 << 0);
    pub const ENCRYPTED: Self = Self(1 << 1);
    pub const CONTINUED: Self = Self(1 << 2);
    pub const END_STREAM: Self = Self(1 << 3);

    pub fn is_compressed(&self) -> bool {
        self.0 & Self::COMPRESSED.0 != 0
    }

    pub fn is_end_stream(&self) -> bool {
        self.0 & Self::END_STREAM.0 != 0
    }
}

impl Frame {
    pub const HEADER_SIZE: usize = 14; // 2 + 1 + 1 + 4 + 4 + 2

    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u16(MAGIC);
        buf.put_u8(VERSION);
        buf.put_u8(self.flags.0);
        buf.put_u32(self.payload.len() as u32);
        buf.put_u32(self.stream_id);
        buf.put_u16(self.message_type as u16);
        buf.put_slice(&self.payload);
    }

    pub fn decode(buf: &mut impl Buf) -> Result<Self, DecodeError> {
        if buf.remaining() < Self::HEADER_SIZE {
            return Err(DecodeError::Incomplete);
        }

        let magic = buf.get_u16();
        if magic != MAGIC {
            return Err(DecodeError::InvalidMagic(magic));
        }

        let version = buf.get_u8();
        if version != VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        let flags = FrameFlags(buf.get_u8());
        let length = buf.get_u32() as usize;
        let stream_id = buf.get_u32();
        let message_type = MessageType::try_from(buf.get_u16())?;

        if buf.remaining() < length {
            return Err(DecodeError::Incomplete);
        }

        let payload = buf.copy_to_bytes(length);

        Ok(Frame {
            flags,
            stream_id,
            message_type,
            payload,
        })
    }
}
```

---

## Message Types

### Message Type Registry

| Code | Name | Direction | Description |
|------|------|-----------|-------------|
| **Connection** | | | |
| 0x0001 | Hello | C→S | Initial connection |
| 0x0002 | HelloAck | S→C | Connection accepted |
| 0x0003 | Authenticate | C→S | Authentication request |
| 0x0004 | AuthResult | S→C | Authentication result |
| 0x0005 | Terminate | C→S | Close connection |
| 0x0006 | TerminateAck | S→C | Connection closed |
| 0x0007 | Ping | Both | Keepalive ping |
| 0x0008 | Pong | Both | Keepalive pong |
| **Query** | | | |
| 0x0100 | Query | C→S | Execute query |
| 0x0101 | Parse | C→S | Parse query (prepared) |
| 0x0102 | Bind | C→S | Bind parameters |
| 0x0103 | Execute | C→S | Execute prepared |
| 0x0104 | Cancel | C→S | Cancel query |
| 0x0105 | Sync | C→S | Sync after pipelined |
| **Results** | | | |
| 0x0200 | RowDescription | S→C | Column metadata |
| 0x0201 | DataRow | S→C | Row data |
| 0x0202 | CommandComplete | S→C | Query complete |
| 0x0203 | ParseComplete | S→C | Parse complete |
| 0x0204 | BindComplete | S→C | Bind complete |
| 0x0205 | EmptyQueryResponse | S→C | Empty query |
| 0x0206 | ReadyForQuery | S→C | Ready for next |
| **Transactions** | | | |
| 0x0300 | Begin | C→S | Begin transaction |
| 0x0301 | Commit | C→S | Commit transaction |
| 0x0302 | Rollback | C→S | Rollback transaction |
| 0x0303 | Savepoint | C→S | Create savepoint |
| 0x0304 | ReleaseSavepoint | C→S | Release savepoint |
| 0x0305 | RollbackToSavepoint | C→S | Rollback to savepoint |
| 0x0306 | TransactionStatus | S→C | Transaction state |
| **LIVE Queries** | | | |
| 0x0400 | Subscribe | C→S | Start LIVE query |
| 0x0401 | Unsubscribe | C→S | Stop LIVE query |
| 0x0402 | LiveInsert | S→C | Row inserted |
| 0x0403 | LiveUpdate | S→C | Row updated |
| 0x0404 | LiveDelete | S→C | Row deleted |
| 0x0405 | LiveDiff | S→C | Row diff |
| 0x0406 | SubscriptionActive | S→C | Subscription started |
| 0x0407 | SubscriptionEnded | S→C | Subscription ended |
| **Graph** | | | |
| 0x0500 | GraphPath | S→C | Graph path result |
| 0x0501 | GraphNode | S→C | Single node |
| 0x0502 | GraphEdge | S→C | Single edge |
| 0x0503 | GraphTraversalStart | S→C | Traversal started |
| 0x0504 | GraphTraversalEnd | S→C | Traversal ended |
| **Vector** | | | |
| 0x0600 | VectorResult | S→C | Vector search result |
| 0x0601 | VectorBatch | S→C | Batch of vectors |
| **Errors** | | | |
| 0x0F00 | Error | S→C | Error response |
| 0x0F01 | Warning | S→C | Warning notice |
| 0x0F02 | Notice | S→C | Informational notice |

### Rust Message Type Implementation

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // Connection
    Hello = 0x0001,
    HelloAck = 0x0002,
    Authenticate = 0x0003,
    AuthResult = 0x0004,
    Terminate = 0x0005,
    TerminateAck = 0x0006,
    Ping = 0x0007,
    Pong = 0x0008,

    // Query
    Query = 0x0100,
    Parse = 0x0101,
    Bind = 0x0102,
    Execute = 0x0103,
    Cancel = 0x0104,
    Sync = 0x0105,

    // Results
    RowDescription = 0x0200,
    DataRow = 0x0201,
    CommandComplete = 0x0202,
    ParseComplete = 0x0203,
    BindComplete = 0x0204,
    EmptyQueryResponse = 0x0205,
    ReadyForQuery = 0x0206,

    // Transactions
    Begin = 0x0300,
    Commit = 0x0301,
    Rollback = 0x0302,
    Savepoint = 0x0303,
    ReleaseSavepoint = 0x0304,
    RollbackToSavepoint = 0x0305,
    TransactionStatus = 0x0306,

    // LIVE Queries
    Subscribe = 0x0400,
    Unsubscribe = 0x0401,
    LiveInsert = 0x0402,
    LiveUpdate = 0x0403,
    LiveDelete = 0x0404,
    LiveDiff = 0x0405,
    SubscriptionActive = 0x0406,
    SubscriptionEnded = 0x0407,

    // Graph
    GraphPath = 0x0500,
    GraphNode = 0x0501,
    GraphEdge = 0x0502,
    GraphTraversalStart = 0x0503,
    GraphTraversalEnd = 0x0504,

    // Vector
    VectorResult = 0x0600,
    VectorBatch = 0x0601,

    // Errors
    Error = 0x0F00,
    Warning = 0x0F01,
    Notice = 0x0F02,
}
```

---

## Authentication

### Hello Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    /// Protocol version (major.minor)
    pub version: (u8, u8),

    /// Client capabilities
    pub capabilities: Capabilities,

    /// Preferred compression algorithms
    pub compression: Vec<CompressionAlgorithm>,

    /// Client identifier
    pub client_id: String,

    /// Client version
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    /// Supports LIVE queries
    pub live_queries: bool,

    /// Supports vector operations
    pub vector_search: bool,

    /// Supports graph traversals
    pub graph_traversal: bool,

    /// Supports prepared statements
    pub prepared_statements: bool,

    /// Supports transactions
    pub transactions: bool,

    /// Supports pipelining
    pub pipelining: bool,

    /// Maximum concurrent streams
    pub max_streams: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Zstd,
    Snappy,
}
```

### HelloAck Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAck {
    /// Server's protocol version
    pub version: (u8, u8),

    /// Negotiated capabilities
    pub capabilities: Capabilities,

    /// Selected compression
    pub compression: CompressionAlgorithm,

    /// Server identifier
    pub server_id: String,

    /// Server version
    pub server_version: String,

    /// Supported auth methods
    pub auth_methods: Vec<AuthMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// No authentication
    None,
    /// Username/password
    Plain,
    /// SCRAM-SHA-256
    ScramSha256,
    /// JWT token
    Token,
    /// Certificate-based (mTLS)
    Certificate,
}
```

### Authenticate Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authenticate {
    pub method: AuthMethod,
    pub credentials: AuthCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthCredentials {
    Plain {
        username: String,
        password: String,
    },
    ScramSha256 {
        username: String,
        client_first_message: Vec<u8>,
    },
    Token {
        token: String,
    },
    Certificate {
        // Certificate provided via TLS
    },
}
```

### AuthResult Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub error: Option<AuthError>,
    pub session_token: Option<String>,
    pub user_info: Option<UserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub roles: Vec<String>,
    pub default_namespace: Option<String>,
    pub default_database: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthError {
    pub code: String,
    pub message: String,
}
```

---

## Query Execution

### Query Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// SQL/OrbitQL query text
    pub sql: String,

    /// Query parameters (positional $1, $2 or named $name)
    pub params: Vec<Value>,

    /// Execution options
    pub options: QueryOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOptions {
    /// Maximum rows to return (0 = unlimited)
    pub limit: u64,

    /// Timeout in milliseconds (0 = no timeout)
    pub timeout_ms: u64,

    /// Transaction ID (if part of transaction)
    pub transaction_id: Option<String>,

    /// Fetch size for streaming
    pub fetch_size: u32,

    /// Include execution plan
    pub explain: bool,
}
```

### RowDescription Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowDescription {
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,

    /// Table name (if applicable)
    pub table: Option<String>,

    /// Column data type
    pub data_type: DataType,

    /// Is nullable
    pub nullable: bool,

    /// Type-specific metadata
    pub metadata: ColumnMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnMetadata {
    None,
    String { max_length: Option<u32> },
    Decimal { precision: u8, scale: u8 },
    Vector { dimensions: u32, distance_type: DistanceType },
    Array { element_type: Box<DataType> },
    Graph { node_labels: Vec<String>, edge_types: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    Null,
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Decimal,
    String,
    Bytes,
    Date,
    Time,
    Timestamp,
    TimestampTz,
    Duration,
    Interval,
    Uuid,
    Json,
    Array,
    Object,
    Vector,
    Geometry,
    Point,
    GraphNode,
    GraphEdge,
    GraphPath,
}
```

### DataRow Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRow {
    /// Row values (None = NULL)
    pub values: Vec<Option<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    Decimal(Decimal),
    String(String),
    Bytes(Vec<u8>),
    Date(i32),           // Days since epoch
    Time(i64),           // Nanoseconds since midnight
    Timestamp(i64),      // Nanoseconds since epoch
    TimestampTz(i64, i16), // Timestamp + offset minutes
    Duration(i64),       // Nanoseconds
    Interval(Interval),
    Uuid([u8; 16]),
    Json(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
    Vector(Vec<f32>),
    Geometry(Vec<u8>),   // WKB
    Point { x: f64, y: f64 },
    GraphNode(GraphNode),
    GraphEdge(GraphEdge),
    GraphPath(GraphPath),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interval {
    pub months: i32,
    pub days: i32,
    pub nanoseconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decimal {
    pub precision: u8,
    pub scale: u8,
    pub value: i128,
}
```

### CommandComplete Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandComplete {
    /// Command tag (SELECT, INSERT, UPDATE, DELETE, etc.)
    pub tag: String,

    /// Rows affected/returned
    pub rows: u64,

    /// Execution time in microseconds
    pub execution_time_us: u64,

    /// Execution plan (if explain was requested)
    pub plan: Option<QueryPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub nodes: Vec<PlanNode>,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub operation: String,
    pub table: Option<String>,
    pub index: Option<String>,
    pub cost: f64,
    pub rows: u64,
    pub children: Vec<PlanNode>,
}
```

---

## Prepared Statements

### Parse Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parse {
    /// Statement name (empty for unnamed)
    pub name: String,

    /// Query text with $1, $2 parameter placeholders
    pub query: String,

    /// Parameter type hints (optional)
    pub param_types: Vec<DataType>,
}
```

### ParseComplete Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseComplete {
    /// Statement handle
    pub statement_id: String,

    /// Inferred parameter types
    pub param_types: Vec<DataType>,

    /// Result column info
    pub columns: Vec<ColumnInfo>,
}
```

### Bind Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bind {
    /// Statement to bind to
    pub statement_id: String,

    /// Portal name (for cursors)
    pub portal: String,

    /// Parameter values
    pub params: Vec<Option<Value>>,

    /// Result format (text/binary per column)
    pub result_formats: Vec<Format>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Format {
    Text,
    Binary,
}
```

### Execute Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execute {
    /// Portal to execute
    pub portal: String,

    /// Maximum rows to return (0 = all)
    pub max_rows: u32,
}
```

---

## Transactions

### Begin Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Begin {
    /// Isolation level
    pub isolation: IsolationLevel,

    /// Read-only transaction
    pub read_only: bool,

    /// Deferrable (for serializable read-only)
    pub deferrable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
    Snapshot,
}
```

### TransactionStatus Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatus {
    /// Transaction ID
    pub transaction_id: String,

    /// Current state
    pub state: TransactionState,

    /// Active savepoints
    pub savepoints: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransactionState {
    /// No active transaction
    Idle,
    /// Transaction in progress
    InTransaction,
    /// Transaction failed, awaiting rollback
    Failed,
}
```

### Savepoint Messages

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Savepoint {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSavepoint {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackToSavepoint {
    pub name: String,
}
```

---

## LIVE Queries

### Subscribe Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscribe {
    /// LIVE SELECT query
    pub query: String,

    /// Query parameters
    pub params: Vec<Value>,

    /// Subscription options
    pub options: SubscriptionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionOptions {
    /// Include initial data
    pub fetch_initial: bool,

    /// Return diffs instead of full rows
    pub diff_mode: bool,

    /// Debounce interval (milliseconds)
    pub debounce_ms: u32,

    /// Maximum pending updates before backpressure
    pub buffer_size: u32,
}
```

### SubscriptionActive Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionActive {
    /// Subscription ID
    pub subscription_id: String,

    /// Column metadata
    pub columns: Vec<ColumnInfo>,
}
```

### LiveInsert/Update/Delete Messages

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveInsert {
    /// Subscription ID
    pub subscription_id: String,

    /// Inserted row
    pub row: DataRow,

    /// Insertion timestamp
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveUpdate {
    /// Subscription ID
    pub subscription_id: String,

    /// Primary key of updated row
    pub key: Vec<Value>,

    /// Updated row (full)
    pub row: DataRow,

    /// Update timestamp
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveDelete {
    /// Subscription ID
    pub subscription_id: String,

    /// Primary key of deleted row
    pub key: Vec<Value>,

    /// Deletion timestamp
    pub timestamp: i64,
}
```

### LiveDiff Message

For diff mode, returns only changed fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveDiff {
    /// Subscription ID
    pub subscription_id: String,

    /// Primary key
    pub key: Vec<Value>,

    /// Change type
    pub change_type: ChangeType,

    /// Changed fields (column index -> new value)
    pub changes: Vec<(u16, Option<Value>)>,

    /// Change timestamp
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}
```

### Unsubscribe Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unsubscribe {
    /// Subscription ID to cancel
    pub subscription_id: String,
}
```

---

## Graph Operations

### GraphNode Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Node ID (table:id format)
    pub id: String,

    /// Node labels
    pub labels: Vec<String>,

    /// Node properties
    pub properties: Vec<(String, Value)>,
}
```

### GraphEdge Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Edge ID
    pub id: String,

    /// Edge type
    pub edge_type: String,

    /// Source node ID
    pub from: String,

    /// Target node ID
    pub to: String,

    /// Edge properties
    pub properties: Vec<(String, Value)>,
}
```

### GraphPath Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// Nodes in path order
    pub nodes: Vec<GraphNode>,

    /// Edges connecting nodes
    pub edges: Vec<GraphEdge>,

    /// Total path length
    pub length: u32,

    /// Path cost (for weighted traversals)
    pub cost: Option<f64>,
}
```

### GraphPath Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPathMessage {
    /// Path result
    pub path: GraphPath,

    /// Additional metadata
    pub metadata: GraphPathMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPathMetadata {
    /// Traversal algorithm used
    pub algorithm: String,

    /// Depth of traversal
    pub depth: u32,

    /// Nodes visited (for debugging)
    pub nodes_visited: u64,
}
```

---

## Vector Operations

### VectorResult Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorResult {
    /// Record ID
    pub id: String,

    /// Distance/similarity score
    pub score: f32,

    /// Score type
    pub score_type: ScoreType,

    /// Associated row data
    pub row: DataRow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ScoreType {
    /// Lower is better (distance)
    Distance,
    /// Higher is better (similarity)
    Similarity,
}
```

### VectorBatch Message

For efficient batch vector operations:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorBatch {
    /// Number of vectors
    pub count: u32,

    /// Vector dimensions
    pub dimensions: u32,

    /// Packed vector data (count * dimensions floats)
    pub vectors: Vec<f32>,

    /// IDs for each vector
    pub ids: Vec<String>,

    /// Scores for each vector
    pub scores: Vec<f32>,
}
```

---

## Error Handling

### Error Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    /// Error severity
    pub severity: ErrorSeverity,

    /// Error code (e.g., "42P01" for undefined table)
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Detailed description
    pub detail: Option<String>,

    /// Hint for resolution
    pub hint: Option<String>,

    /// Position in query (1-indexed)
    pub position: Option<u32>,

    /// Internal position
    pub internal_position: Option<u32>,

    /// Internal query
    pub internal_query: Option<String>,

    /// Where in source
    pub where_: Option<String>,

    /// Schema name
    pub schema: Option<String>,

    /// Table name
    pub table: Option<String>,

    /// Column name
    pub column: Option<String>,

    /// Data type name
    pub data_type: Option<String>,

    /// Constraint name
    pub constraint: Option<String>,

    /// File name (for debugging)
    pub file: Option<String>,

    /// Line number (for debugging)
    pub line: Option<u32>,

    /// Routine name (for debugging)
    pub routine: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Fatal - connection will be closed
    Fatal,
    /// Panic - server error
    Panic,
    /// Error - statement failed
    Error,
    /// Warning - statement completed with issues
    Warning,
    /// Notice - informational
    Notice,
    /// Debug - debugging info
    Debug,
    /// Info - informational
    Info,
    /// Log - log message
    Log,
}
```

### Error Codes

OrbitWire uses PostgreSQL-compatible error codes with OrbitQL extensions:

| Class | Code | Description |
|-------|------|-------------|
| **00** | 00000 | Success |
| **01** | 01000 | Warning |
| **02** | 02000 | No data |
| **22** | 22000 | Data exception |
| **23** | 23505 | Unique violation |
| **40** | 40001 | Serialization failure |
| **42** | 42601 | Syntax error |
| **42** | 42P01 | Undefined table |
| **42** | 42703 | Undefined column |
| **OQ** | OQ001 | OrbitQL parse error |
| **OQ** | OQ002 | Graph traversal error |
| **OQ** | OQ003 | Vector index error |
| **OQ** | OQ004 | LIVE query error |

---

## Compression

### Supported Algorithms

| Algorithm | Code | Use Case |
|-----------|------|----------|
| None | 0x00 | Low latency |
| LZ4 | 0x01 | Balanced |
| Zstd | 0x02 | High compression |
| Snappy | 0x03 | Fast compression |

### Compression Implementation

```rust
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use zstd::stream::{encode_all, decode_all};

pub trait Compressor: Send + Sync {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;
}

pub struct Lz4Compressor;

impl Compressor for Lz4Compressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(compress_prepend_size(data))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        decompress_size_prepended(data)
            .map_err(|e| CompressionError::DecompressError(e.to_string()))
    }
}

pub struct ZstdCompressor {
    level: i32,
}

impl Compressor for ZstdCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        encode_all(data, self.level)
            .map_err(|e| CompressionError::CompressError(e.to_string()))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        decode_all(data)
            .map_err(|e| CompressionError::DecompressError(e.to_string()))
    }
}
```

---

## Implementation Guide

### Project Structure

```
orbit/server/src/protocols/orbitwire/
├── mod.rs                    # Module exports
├── server.rs                 # Server implementation
├── connection.rs             # Connection handling
├── codec.rs                  # Frame encoding/decoding
├── messages/                 # Message types
│   ├── mod.rs
│   ├── connection.rs         # Hello, Auth messages
│   ├── query.rs              # Query, Parse, Bind, Execute
│   ├── result.rs             # RowDescription, DataRow
│   ├── transaction.rs        # Begin, Commit, Rollback
│   ├── live.rs               # Subscribe, LiveInsert, etc.
│   ├── graph.rs              # GraphPath, GraphNode
│   ├── vector.rs             # VectorResult, VectorBatch
│   └── error.rs              # Error message
├── handler.rs                # Message handlers
├── compression.rs            # Compression support
├── auth.rs                   # Authentication
└── types.rs                  # Data types and values
```

### Cargo Dependencies

```toml
[dependencies]
# Async I/O
tokio = { version = "1.48", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }

# Serialization
bytes = "1.7"
serde = { version = "1.0", features = ["derive"] }
rmp-serde = "1.3"  # MessagePack
bincode = "1.3"    # Alternative binary format

# Compression
lz4_flex = "0.11"
zstd = "0.13"

# Utilities
parking_lot = "0.12"
dashmap = "6"
uuid = { version = "1.0", features = ["v4"] }
tracing = "0.1"
```

### Server Implementation

```rust
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

pub struct OrbitWireServer {
    engine: Arc<OrbitQLEngine>,
    auth_manager: Arc<AuthManager>,
    config: OrbitWireConfig,
}

pub struct OrbitWireConfig {
    pub listen_addr: SocketAddr,
    pub max_connections: usize,
    pub max_streams_per_connection: u32,
    pub idle_timeout: Duration,
    pub compression_threshold: usize,
}

impl OrbitWireServer {
    pub async fn run(self) -> Result<(), ServerError> {
        let listener = TcpListener::bind(self.config.listen_addr).await?;
        tracing::info!("OrbitWire server listening on {}", self.config.listen_addr);

        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::debug!("New connection from {}", addr);

            let engine = self.engine.clone();
            let auth_manager = self.auth_manager.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, engine, auth_manager, config).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    engine: Arc<OrbitQLEngine>,
    auth_manager: Arc<AuthManager>,
    config: OrbitWireConfig,
) -> Result<(), ConnectionError> {
    let mut framed = Framed::new(stream, OrbitWireCodec::new());

    // Perform handshake
    let (capabilities, compression) = perform_handshake(&mut framed, &config).await?;

    // Authenticate
    let session = authenticate(&mut framed, &auth_manager).await?;

    // Create connection handler
    let handler = ConnectionHandler::new(engine, session, capabilities, compression);

    // Handle messages
    handler.run(framed).await
}
```

### Codec Implementation

```rust
use tokio_util::codec::{Decoder, Encoder};

pub struct OrbitWireCodec {
    max_frame_size: usize,
    compressor: Option<Box<dyn Compressor>>,
}

impl Decoder for OrbitWireCodec {
    type Item = Frame;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < Frame::HEADER_SIZE {
            return Ok(None);
        }

        // Peek at length
        let length = u32::from_be_bytes([src[4], src[5], src[6], src[7]]) as usize;

        if src.len() < Frame::HEADER_SIZE + length {
            // Reserve capacity
            src.reserve(Frame::HEADER_SIZE + length - src.len());
            return Ok(None);
        }

        // Decode frame
        let frame = Frame::decode(src)?;

        // Decompress if needed
        if frame.flags.is_compressed() {
            if let Some(ref compressor) = self.compressor {
                let decompressed = compressor.decompress(&frame.payload)?;
                return Ok(Some(Frame {
                    payload: decompressed.into(),
                    ..frame
                }));
            }
        }

        Ok(Some(frame))
    }
}

impl Encoder<Frame> for OrbitWireCodec {
    type Error = CodecError;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut frame = frame;

        // Compress if beneficial
        if let Some(ref compressor) = self.compressor {
            if frame.payload.len() > 1024 {
                let compressed = compressor.compress(&frame.payload)?;
                if compressed.len() < frame.payload.len() {
                    frame.payload = compressed.into();
                    frame.flags = FrameFlags(frame.flags.0 | FrameFlags::COMPRESSED.0);
                }
            }
        }

        frame.encode(dst);
        Ok(())
    }
}
```

### Connection Handler

```rust
pub struct ConnectionHandler {
    engine: Arc<OrbitQLEngine>,
    session: Session,
    capabilities: Capabilities,
    compressor: Option<Box<dyn Compressor>>,
    streams: DashMap<u32, StreamState>,
    subscriptions: DashMap<String, SubscriptionState>,
    transaction: RwLock<Option<TransactionState>>,
}

impl ConnectionHandler {
    pub async fn run(
        self,
        mut framed: Framed<TcpStream, OrbitWireCodec>,
    ) -> Result<(), ConnectionError> {
        loop {
            tokio::select! {
                // Incoming frame
                frame = framed.next() => {
                    match frame {
                        Some(Ok(frame)) => {
                            self.handle_frame(&mut framed, frame).await?;
                        }
                        Some(Err(e)) => {
                            return Err(ConnectionError::Codec(e));
                        }
                        None => {
                            // Connection closed
                            break;
                        }
                    }
                }

                // Outgoing subscription updates
                update = self.recv_subscription_update() => {
                    self.send_live_update(&mut framed, update).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_frame(
        &self,
        framed: &mut Framed<TcpStream, OrbitWireCodec>,
        frame: Frame,
    ) -> Result<(), ConnectionError> {
        match frame.message_type {
            MessageType::Query => {
                self.handle_query(framed, frame).await
            }
            MessageType::Parse => {
                self.handle_parse(framed, frame).await
            }
            MessageType::Bind => {
                self.handle_bind(framed, frame).await
            }
            MessageType::Execute => {
                self.handle_execute(framed, frame).await
            }
            MessageType::Subscribe => {
                self.handle_subscribe(framed, frame).await
            }
            MessageType::Unsubscribe => {
                self.handle_unsubscribe(framed, frame).await
            }
            MessageType::Begin => {
                self.handle_begin(framed, frame).await
            }
            MessageType::Commit => {
                self.handle_commit(framed, frame).await
            }
            MessageType::Rollback => {
                self.handle_rollback(framed, frame).await
            }
            MessageType::Terminate => {
                self.handle_terminate(framed, frame).await?;
                return Err(ConnectionError::Terminated);
            }
            MessageType::Ping => {
                self.handle_ping(framed, frame).await
            }
            _ => {
                self.send_error(framed, frame.stream_id, "Unsupported message type").await
            }
        }
    }

    async fn handle_query(
        &self,
        framed: &mut Framed<TcpStream, OrbitWireCodec>,
        frame: Frame,
    ) -> Result<(), ConnectionError> {
        let query: Query = rmp_serde::from_slice(&frame.payload)
            .map_err(|e| ConnectionError::Decode(e.to_string()))?;

        let stream_id = frame.stream_id;

        // Execute query
        match self.engine.execute(&query.sql, query.params).await {
            Ok(result) => {
                // Send RowDescription
                self.send_row_description(framed, stream_id, &result.columns).await?;

                // Stream rows
                for row in result.rows {
                    self.send_data_row(framed, stream_id, &row).await?;
                }

                // Send CommandComplete
                self.send_command_complete(framed, stream_id, &result).await?;
            }
            Err(e) => {
                self.send_error(framed, stream_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }
}
```

---

## Wire Format Reference

### Value Encoding

Values are encoded using MessagePack with type tags:

| Type Tag | Value Type | Format |
|----------|------------|--------|
| 0x00 | Null | (empty) |
| 0x01 | Bool | 1 byte (0/1) |
| 0x02 | Int8 | 1 byte signed |
| 0x03 | Int16 | 2 bytes BE |
| 0x04 | Int32 | 4 bytes BE |
| 0x05 | Int64 | 8 bytes BE |
| 0x06 | Float32 | 4 bytes IEEE754 |
| 0x07 | Float64 | 8 bytes IEEE754 |
| 0x08 | Decimal | precision + scale + i128 |
| 0x09 | String | length + UTF-8 bytes |
| 0x0A | Bytes | length + raw bytes |
| 0x0B | Date | 4 bytes (days) |
| 0x0C | Time | 8 bytes (nanos) |
| 0x0D | Timestamp | 8 bytes (nanos since epoch) |
| 0x0E | TimestampTz | 8 bytes + 2 bytes offset |
| 0x0F | Duration | 8 bytes (nanos) |
| 0x10 | Interval | months + days + nanos |
| 0x11 | UUID | 16 bytes |
| 0x12 | Json | length + UTF-8 JSON |
| 0x13 | Array | count + elements |
| 0x14 | Object | count + key-value pairs |
| 0x15 | Vector | dims + f32 array |
| 0x16 | Geometry | length + WKB |
| 0x17 | Point | f64 x + f64 y |
| 0x18 | GraphNode | id + labels + props |
| 0x19 | GraphEdge | id + type + from + to + props |
| 0x1A | GraphPath | nodes + edges |

### Example Wire Trace

```
# Client sends Query
>>> Frame {
      magic: 0x4F52,
      version: 0x01,
      flags: 0x00,
      length: 45,
      stream_id: 1,
      message_type: 0x0100 (Query),
      payload: {
        sql: "SELECT * FROM users WHERE id = $1",
        params: [Value::Int64(42)],
        options: { limit: 0, timeout_ms: 5000 }
      }
    }

# Server sends RowDescription
<<< Frame {
      stream_id: 1,
      message_type: 0x0200 (RowDescription),
      payload: {
        columns: [
          { name: "id", data_type: Int64, nullable: false },
          { name: "name", data_type: String, nullable: false },
          { name: "email", data_type: String, nullable: true }
        ]
      }
    }

# Server sends DataRow
<<< Frame {
      stream_id: 1,
      message_type: 0x0201 (DataRow),
      payload: {
        values: [
          Value::Int64(42),
          Value::String("Alice"),
          Value::String("alice@example.com")
        ]
      }
    }

# Server sends CommandComplete
<<< Frame {
      stream_id: 1,
      message_type: 0x0202 (CommandComplete),
      flags: END_STREAM,
      payload: {
        tag: "SELECT",
        rows: 1,
        execution_time_us: 1234
      }
    }
```

---

## References

- [PostgreSQL Wire Protocol](https://www.postgresql.org/docs/current/protocol.html) - Reference for message flow patterns
- [TDS Protocol](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/) - Inspiration for features
- [MessagePack](https://msgpack.org/) - Serialization format
- [HTTP/2](https://httpwg.org/specs/rfc9113.html) - Stream multiplexing concepts

---

*OrbitWire Protocol Specification v1.0.0 - December 2025*
