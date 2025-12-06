# MongoDB Wire Protocol Complete Reference for Rust Implementation

## Document Purpose

This document provides a comprehensive reference for implementing MongoDB wire protocol clients and drivers in Rust. It covers the binary wire protocol, BSON serialization, command structures, query operators, aggregation framework, authentication mechanisms, and all aspects necessary for a complete implementation.

---

## Table of Contents

1. [Protocol Overview](#1-protocol-overview)
2. [Wire Protocol Message Format](#2-wire-protocol-message-format)
3. [OP_MSG Protocol](#3-op_msg-protocol)
4. [BSON Specification](#4-bson-specification)
5. [Command Reference](#5-command-reference)
6. [Query Operators](#6-query-operators)
7. [Update Operators](#7-update-operators)
8. [Aggregation Pipeline](#8-aggregation-pipeline)
9. [Authentication](#9-authentication)
10. [Rust Implementation](#10-rust-implementation)

---

## 1. Protocol Overview

### 1.1 Architecture

MongoDB uses a binary protocol over TCP for client-server communication:

- **Transport Layer**: TCP connections (default port 27017), optionally with TLS
- **Message Layer**: Binary messages with headers and payloads
- **Serialization Layer**: BSON (Binary JSON) for document encoding
- **Command Layer**: Database commands expressed as BSON documents

### 1.2 Byte Order

All multi-byte integers use **little-endian** byte order.

```rust
fn write_i32_le(value: i32) -> [u8; 4] {
    value.to_le_bytes()
}

fn read_i32_le(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes[0..4].try_into().unwrap())
}
```

### 1.3 Wire Version Capabilities

| Wire Version | MongoDB Version | Key Features |
|--------------|-----------------|--------------|
| 6 | 3.4 | OP_MSG, compression |
| 7 | 3.6 | Sessions, causal consistency |
| 8 | 4.0 | Multi-document transactions |
| 13 | 5.0 | Versioned API |
| 17 | 6.0 | Queryable encryption |
| 21 | 7.0 | Latest features |

---

## 2. Wire Protocol Message Format

### 2.1 Standard Message Header

Every MongoDB message begins with a 16-byte header:

```
+----------------+----------------+----------------+----------------+
|   messageLength (4 bytes)      |   requestID (4 bytes)          |
+----------------+----------------+----------------+----------------+
|   responseTo (4 bytes)         |   opCode (4 bytes)             |
+----------------+----------------+----------------+----------------+
```

```rust
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MsgHeader {
    pub message_length: i32,
    pub request_id: i32,
    pub response_to: i32,
    pub op_code: i32,
}

impl MsgHeader {
    pub const SIZE: usize = 16;
    
    pub fn serialize(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.message_length.to_le_bytes());
        buf[4..8].copy_from_slice(&self.request_id.to_le_bytes());
        buf[8..12].copy_from_slice(&self.response_to.to_le_bytes());
        buf[12..16].copy_from_slice(&self.op_code.to_le_bytes());
        buf
    }
    
    pub fn deserialize(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() < 16 {
            return Err(ProtocolError::InsufficientData);
        }
        Ok(Self {
            message_length: i32::from_le_bytes(buf[0..4].try_into().unwrap()),
            request_id: i32::from_le_bytes(buf[4..8].try_into().unwrap()),
            response_to: i32::from_le_bytes(buf[8..12].try_into().unwrap()),
            op_code: i32::from_le_bytes(buf[12..16].try_into().unwrap()),
        })
    }
}
```

### 2.2 Opcodes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum OpCode {
    OpReply = 1,           // Deprecated
    OpUpdate = 2001,       // Deprecated
    OpInsert = 2002,       // Deprecated
    OpQuery = 2004,        // Deprecated
    OpGetMore = 2005,      // Deprecated
    OpDelete = 2006,       // Deprecated
    OpKillCursors = 2007,  // Deprecated
    OpCompressed = 2012,   // Compression wrapper
    OpMsg = 2013,          // Modern - use this
}
```

---

## 3. OP_MSG Protocol

### 3.1 OP_MSG Structure

```
+------------------------------------------------------------------+
|             Standard Message Header (16 bytes)                   |
+----------------+----------------+
|   flagBits (4 bytes)           |
+----------------+----------------+
|   Section 0: Body (kind=0) - BSON document                       |
+------------------------------------------------------------------+
|   Section 1..N: Document Sequence (kind=1) [optional]            |
+------------------------------------------------------------------+
|   checksum (4 bytes) [optional]                                  |
+------------------------------------------------------------------+
```

### 3.2 Flag Bits

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpMsgFlags: u32 {
        const CHECKSUM_PRESENT = 1 << 0;
        const MORE_TO_COME = 1 << 1;
        const EXHAUST_ALLOWED = 1 << 16;
    }
}
```

### 3.3 Complete OP_MSG Implementation

```rust
#[derive(Debug, Clone)]
pub struct OpMsg {
    pub header: MsgHeader,
    pub flags: OpMsgFlags,
    pub body: Document,
    pub document_sequences: Vec<DocumentSequence>,
    pub checksum: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DocumentSequence {
    pub identifier: String,
    pub documents: Vec<Document>,
}

impl OpMsg {
    pub fn new(command: Document) -> Self {
        Self {
            header: MsgHeader {
                message_length: 0,
                request_id: next_request_id(),
                response_to: 0,
                op_code: OpCode::OpMsg as i32,
            },
            flags: OpMsgFlags::empty(),
            body: command,
            document_sequences: Vec::new(),
            checksum: None,
        }
    }
    
    pub fn with_documents(mut self, identifier: &str, docs: Vec<Document>) -> Self {
        self.document_sequences.push(DocumentSequence {
            identifier: identifier.to_string(),
            documents: docs,
        });
        self
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // Flags
        payload.extend(&self.flags.bits().to_le_bytes());
        
        // Body section (kind 0)
        payload.push(0);
        payload.extend(self.body.to_bytes());
        
        // Document sequences (kind 1)
        for seq in &self.document_sequences {
            payload.push(1);
            let mut seq_data = Vec::new();
            seq_data.extend(seq.identifier.as_bytes());
            seq_data.push(0);
            for doc in &seq.documents {
                seq_data.extend(doc.to_bytes());
            }
            payload.extend(&((4 + seq_data.len()) as i32).to_le_bytes());
            payload.extend(seq_data);
        }
        
        let message_length = MsgHeader::SIZE + payload.len();
        
        let mut message = Vec::with_capacity(message_length);
        let mut header = self.header;
        header.message_length = message_length as i32;
        message.extend(header.serialize());
        message.extend(payload);
        
        message
    }
}

static REQUEST_ID_COUNTER: std::sync::atomic::AtomicI32 = 
    std::sync::atomic::AtomicI32::new(1);

pub fn next_request_id() -> i32 {
    REQUEST_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
```

---

## 4. BSON Specification

### 4.1 BSON Type Codes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BsonType {
    EndOfDocument = 0x00,
    Double = 0x01,
    String = 0x02,
    Document = 0x03,
    Array = 0x04,
    Binary = 0x05,
    Undefined = 0x06,      // Deprecated
    ObjectId = 0x07,
    Boolean = 0x08,
    DateTime = 0x09,
    Null = 0x0A,
    Regex = 0x0B,
    DbPointer = 0x0C,      // Deprecated
    JavaScript = 0x0D,
    Symbol = 0x0E,         // Deprecated
    JavaScriptWithScope = 0x0F,
    Int32 = 0x10,
    Timestamp = 0x11,
    Int64 = 0x12,
    Decimal128 = 0x13,
    MinKey = 0xFF,
    MaxKey = 0x7F,
}
```

### 4.2 Binary Subtypes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BinarySubtype {
    Generic = 0x00,
    Function = 0x01,
    BinaryOld = 0x02,
    UuidOld = 0x03,
    Uuid = 0x04,
    Md5 = 0x05,
    Encrypted = 0x06,
    CompressedTimeSeries = 0x07,
    Sensitive = 0x08,
    UserDefined = 0x80,
}
```

### 4.3 BSON Value Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Bson {
    Double(f64),
    String(String),
    Document(Document),
    Array(Vec<Bson>),
    Binary { subtype: BinarySubtype, data: Vec<u8> },
    ObjectId(ObjectId),
    Boolean(bool),
    DateTime(chrono::DateTime<chrono::Utc>),
    Null,
    Regex { pattern: String, options: String },
    JavaScript(String),
    JavaScriptWithScope { code: String, scope: Document },
    Int32(i32),
    Timestamp(Timestamp),
    Int64(i64),
    Decimal128([u8; 16]),
    MinKey,
    MaxKey,
    Undefined,
}
```

### 4.4 ObjectId

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId([u8; 12]);

impl ObjectId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        static RANDOM: std::sync::OnceLock<[u8; 5]> = std::sync::OnceLock::new();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        
        let random = RANDOM.get_or_init(|| {
            let mut bytes = [0u8; 5];
            getrandom::getrandom(&mut bytes).unwrap();
            bytes
        });
        
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let counter_bytes = counter.to_be_bytes();
        
        let mut bytes = [0u8; 12];
        bytes[0..4].copy_from_slice(&timestamp.to_be_bytes());
        bytes[4..9].copy_from_slice(random);
        bytes[9..12].copy_from_slice(&counter_bytes[1..4]);
        
        Self(bytes)
    }
    
    pub fn timestamp(&self) -> u32 {
        u32::from_be_bytes(self.0[0..4].try_into().unwrap())
    }
    
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    
    pub fn from_hex(s: &str) -> Result<Self, ParseError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 12 {
            return Err(ParseError::InvalidLength);
        }
        Ok(Self(bytes.try_into().unwrap()))
    }
}
```

### 4.5 Timestamp

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    pub increment: u32,
    pub timestamp: u32,
}

impl Timestamp {
    pub fn to_i64(self) -> i64 {
        ((self.timestamp as i64) << 32) | (self.increment as i64)
    }
    
    pub fn from_i64(value: i64) -> Self {
        Self {
            increment: value as u32,
            timestamp: (value >> 32) as u32,
        }
    }
}
```

### 4.6 Document

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    inner: Vec<(String, Bson)>,
}

impl Document {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }
    
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Bson>) {
        let key = key.into();
        if let Some(pos) = self.inner.iter().position(|(k, _)| k == &key) {
            self.inner[pos].1 = value.into();
        } else {
            self.inner.push((key, value.into()));
        }
    }
    
    pub fn get(&self, key: &str) -> Option<&Bson> {
        self.inner.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    
    pub fn remove(&mut self, key: &str) -> Option<Bson> {
        if let Some(pos) = self.inner.iter().position(|(k, _)| k == key) {
            Some(self.inner.remove(pos).1)
        } else {
            None
        }
    }
    
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Bson)> {
        self.inner.iter().map(|(k, v)| (k, v))
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut content = Vec::new();
        for (key, value) in &self.inner {
            value.write_to(&mut content, key);
        }
        content.push(0);
        
        let size = 4 + content.len();
        let mut buf = Vec::with_capacity(size);
        buf.extend(&(size as i32).to_le_bytes());
        buf.extend(content);
        buf
    }
}

#[macro_export]
macro_rules! doc {
    () => { Document::new() };
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut doc = Document::new();
        $(doc.insert($key, $value);)*
        doc
    }};
}
```

---

## 5. Command Reference

### 5.1 CRUD Commands

#### find

```javascript
{
    "find": "<collection>",
    "filter": <document>,
    "sort": <document>,
    "projection": <document>,
    "skip": <int64>,
    "limit": <int64>,
    "batchSize": <int32>,
    "hint": <document|string>,
    "readConcern": <document>,
    "collation": <document>,
    "allowDiskUse": <boolean>,
    "$db": "<database>"
}
```

#### insert

```javascript
{
    "insert": "<collection>",
    "documents": [<document>, ...],
    "ordered": <boolean>,
    "writeConcern": <document>,
    "$db": "<database>"
}
```

#### update

```javascript
{
    "update": "<collection>",
    "updates": [
        {
            "q": <query>,
            "u": <document|pipeline>,
            "upsert": <boolean>,
            "multi": <boolean>,
            "arrayFilters": [<document>, ...],
            "hint": <document|string>
        }
    ],
    "ordered": <boolean>,
    "writeConcern": <document>,
    "$db": "<database>"
}
```

#### delete

```javascript
{
    "delete": "<collection>",
    "deletes": [
        {
            "q": <query>,
            "limit": <0|1>
        }
    ],
    "ordered": <boolean>,
    "writeConcern": <document>,
    "$db": "<database>"
}
```

### 5.2 Cursor Commands

#### getMore

```javascript
{
    "getMore": <cursorId>,
    "collection": "<collection>",
    "batchSize": <int32>,
    "$db": "<database>"
}
```

#### killCursors

```javascript
{
    "killCursors": "<collection>",
    "cursors": [<cursorId>, ...],
    "$db": "<database>"
}
```

### 5.3 Aggregation

```javascript
{
    "aggregate": "<collection>",
    "pipeline": [<stage>, ...],
    "cursor": { "batchSize": <int32> },
    "allowDiskUse": <boolean>,
    "$db": "<database>"
}
```

### 5.4 Index Commands

#### createIndexes

```javascript
{
    "createIndexes": "<collection>",
    "indexes": [
        {
            "key": <document>,
            "name": "<name>",
            "unique": <boolean>,
            "sparse": <boolean>,
            "expireAfterSeconds": <int32>,
            "partialFilterExpression": <document>
        }
    ],
    "$db": "<database>"
}
```

### 5.5 Administrative Commands

```javascript
// listDatabases
{ "listDatabases": 1, "$db": "admin" }

// listCollections
{ "listCollections": 1, "$db": "<database>" }

// ping
{ "ping": 1, "$db": "admin" }

// hello (handshake)
{
    "hello": 1,
    "client": {
        "driver": { "name": "rust-mongodb", "version": "1.0.0" },
        "os": { "type": "Linux" }
    },
    "$db": "admin"
}
```

---

## 6. Query Operators

### 6.1 Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$eq` | Equals | `{ field: { $eq: value } }` |
| `$ne` | Not equals | `{ field: { $ne: value } }` |
| `$gt` | Greater than | `{ field: { $gt: value } }` |
| `$gte` | Greater or equal | `{ field: { $gte: value } }` |
| `$lt` | Less than | `{ field: { $lt: value } }` |
| `$lte` | Less or equal | `{ field: { $lte: value } }` |
| `$in` | In array | `{ field: { $in: [v1, v2] } }` |
| `$nin` | Not in array | `{ field: { $nin: [v1, v2] } }` |

### 6.2 Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$and` | Logical AND | `{ $and: [{expr1}, {expr2}] }` |
| `$or` | Logical OR | `{ $or: [{expr1}, {expr2}] }` |
| `$not` | Negates | `{ field: { $not: {expr} } }` |
| `$nor` | Neither | `{ $nor: [{expr1}, {expr2}] }` |

### 6.3 Element Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$exists` | Field exists | `{ field: { $exists: true } }` |
| `$type` | Field type | `{ field: { $type: "string" } }` |

### 6.4 Array Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$all` | Contains all | `{ arr: { $all: [v1, v2] } }` |
| `$elemMatch` | Element match | `{ arr: { $elemMatch: {conds} } }` |
| `$size` | Array size | `{ arr: { $size: 3 } }` |

### 6.5 Evaluation Operators

| Operator | Description |
|----------|-------------|
| `$expr` | Aggregation expression |
| `$regex` | Regular expression |
| `$text` | Text search |
| `$mod` | Modulo operation |
| `$where` | JavaScript expression |

### 6.6 Geospatial Operators

| Operator | Description |
|----------|-------------|
| `$geoWithin` | Within geometry |
| `$geoIntersects` | Intersects geometry |
| `$near` | Near point |
| `$nearSphere` | Near point on sphere |

---

## 7. Update Operators

### 7.1 Field Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$set` | Set field | `{ $set: { field: value } }` |
| `$unset` | Remove field | `{ $unset: { field: "" } }` |
| `$setOnInsert` | Set on insert | `{ $setOnInsert: { field: value } }` |
| `$rename` | Rename field | `{ $rename: { old: "new" } }` |
| `$inc` | Increment | `{ $inc: { field: 1 } }` |
| `$mul` | Multiply | `{ $mul: { field: 2 } }` |
| `$min` | Update if less | `{ $min: { field: value } }` |
| `$max` | Update if greater | `{ $max: { field: value } }` |
| `$currentDate` | Current date | `{ $currentDate: { field: true } }` |

### 7.2 Array Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$push` | Append | `{ $push: { arr: value } }` |
| `$pull` | Remove matching | `{ $pull: { arr: query } }` |
| `$pullAll` | Remove all | `{ $pullAll: { arr: [v1, v2] } }` |
| `$pop` | Remove first/last | `{ $pop: { arr: 1 } }` |
| `$addToSet` | Add unique | `{ $addToSet: { arr: value } }` |
| `$` | First match | `{ "arr.$": value }` |
| `$[]` | All elements | `{ "arr.$[]": value }` |
| `$[<id>]` | Filtered | `{ "arr.$[elem]": value }` |

#### Push Modifiers

```javascript
{ $push: { scores: { 
    $each: [90, 92], 
    $sort: -1, 
    $slice: 5,
    $position: 0
} } }
```

---

## 8. Aggregation Pipeline

### 8.1 Pipeline Stages

| Stage | Description |
|-------|-------------|
| `$match` | Filter documents |
| `$project` | Reshape documents |
| `$group` | Group and aggregate |
| `$sort` | Sort documents |
| `$limit` | Limit results |
| `$skip` | Skip documents |
| `$unwind` | Deconstruct array |
| `$lookup` | Join collections |
| `$addFields` | Add fields |
| `$out` | Write to collection |
| `$merge` | Merge to collection |
| `$facet` | Multiple pipelines |
| `$bucket` | Categorize |
| `$graphLookup` | Recursive lookup |
| `$sample` | Random sample |
| `$count` | Count documents |
| `$unionWith` | Union collections |
| `$setWindowFields` | Window functions |

### 8.2 Stage Examples

```javascript
// $match
{ $match: { status: "active", age: { $gte: 21 } } }

// $project
{ $project: { _id: 0, name: 1, total: { $add: ["$a", "$b"] } } }

// $group
{ $group: { 
    _id: "$category", 
    total: { $sum: "$amount" },
    count: { $sum: 1 },
    avg: { $avg: "$price" }
} }

// $lookup
{ $lookup: {
    from: "orders",
    localField: "customer_id",
    foreignField: "customer_id",
    as: "orders"
} }

// $unwind
{ $unwind: { path: "$tags", preserveNullAndEmptyArrays: true } }
```

### 8.3 Aggregation Operators

#### Arithmetic
`$abs`, `$add`, `$ceil`, `$divide`, `$exp`, `$floor`, `$ln`, `$log`, `$mod`, `$multiply`, `$pow`, `$round`, `$sqrt`, `$subtract`, `$trunc`

#### Array
`$arrayElemAt`, `$concatArrays`, `$filter`, `$first`, `$in`, `$indexOfArray`, `$isArray`, `$last`, `$map`, `$range`, `$reduce`, `$reverseArray`, `$size`, `$slice`, `$zip`

#### Boolean
`$and`, `$not`, `$or`

#### Comparison
`$cmp`, `$eq`, `$gt`, `$gte`, `$lt`, `$lte`, `$ne`

#### Conditional
`$cond`, `$ifNull`, `$switch`

#### Date
`$dateAdd`, `$dateDiff`, `$dateFromParts`, `$dateFromString`, `$dateToParts`, `$dateToString`, `$dayOfMonth`, `$dayOfWeek`, `$dayOfYear`, `$hour`, `$minute`, `$month`, `$second`, `$week`, `$year`

#### String
`$concat`, `$indexOfBytes`, `$ltrim`, `$regexFind`, `$regexMatch`, `$replaceOne`, `$rtrim`, `$split`, `$strcasecmp`, `$strLenBytes`, `$substr`, `$toLower`, `$toUpper`, `$trim`

#### Accumulators (for $group)
`$addToSet`, `$avg`, `$count`, `$first`, `$last`, `$max`, `$min`, `$push`, `$stdDevPop`, `$stdDevSamp`, `$sum`

---

## 9. Authentication

### 9.1 SCRAM-SHA-256 Flow

```
Client                          Server
  |                               |
  |-- saslStart (mechanism) ----->|
  |<-- server-first-message ------|
  |-- saslContinue (proof) ------>|
  |<-- server-final-message ------|
```

```javascript
// saslStart
{
    "saslStart": 1,
    "mechanism": "SCRAM-SHA-256",
    "payload": <binary>,
    "autoAuthorize": 1,
    "$db": "admin"
}

// saslContinue
{
    "saslContinue": 1,
    "conversationId": <int>,
    "payload": <binary>,
    "$db": "admin"
}
```

### 9.2 Read/Write Concerns

```rust
// Read Concern
pub enum ReadConcernLevel {
    Local,
    Majority,
    Available,
    Linearizable,
    Snapshot,
}

// Write Concern
pub struct WriteConcern {
    pub w: WriteConcernW,        // 0, 1, "majority", or tag
    pub w_timeout: Option<i32>,
    pub journal: Option<bool>,
}
```

### 9.3 Read Preference

```rust
pub enum ReadPreference {
    Primary,
    PrimaryPreferred,
    Secondary,
    SecondaryPreferred,
    Nearest,
}
```

---

## 10. Rust Implementation

### 10.1 Client Structure

```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct Client {
    stream: TcpStream,
    options: ClientOptions,
}

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub hosts: Vec<String>,
    pub app_name: Option<String>,
    pub connect_timeout: std::time::Duration,
    pub read_preference: ReadPreference,
    pub write_concern: WriteConcern,
    pub read_concern: ReadConcern,
}

impl Client {
    pub async fn connect(uri: &str) -> Result<Self, Error> {
        let options = parse_uri(uri)?;
        let stream = TcpStream::connect(&options.hosts[0]).await?;
        
        let mut client = Self { stream, options };
        client.handshake().await?;
        
        Ok(client)
    }
    
    async fn handshake(&mut self) -> Result<(), Error> {
        let hello = doc! {
            "hello" => 1,
            "helloOk" => true,
            "$db" => "admin"
        };
        
        let response = self.send_command(hello).await?;
        // Process server capabilities
        Ok(())
    }
    
    pub async fn send_command(&mut self, cmd: Document) -> Result<Document, Error> {
        let msg = OpMsg::new(cmd);
        let bytes = msg.serialize();
        
        self.stream.write_all(&bytes).await?;
        
        // Read response
        let mut header = [0u8; 16];
        self.stream.read_exact(&mut header).await?;
        
        let header = MsgHeader::deserialize(&header)?;
        let payload_size = (header.message_length as usize) - 16;
        
        let mut payload = vec![0u8; payload_size];
        self.stream.read_exact(&mut payload).await?;
        
        // Parse response
        let mut full = header.serialize().to_vec();
        full.extend(payload);
        
        let response = OpMsg::deserialize(&full)?;
        Ok(response.body)
    }
    
    pub fn database(&self, name: &str) -> Database {
        Database { client: self, name: name.to_string() }
    }
}
```

### 10.2 Database and Collection

```rust
pub struct Database<'a> {
    client: &'a Client,
    name: String,
}

impl<'a> Database<'a> {
    pub fn collection(&self, name: &str) -> Collection {
        Collection { 
            database: self, 
            name: name.to_string() 
        }
    }
}

pub struct Collection<'a> {
    database: &'a Database<'a>,
    name: String,
}

impl<'a> Collection<'a> {
    pub async fn find(&self, filter: Document) -> Result<Cursor, Error> {
        let cmd = doc! {
            "find" => &self.name,
            "filter" => filter,
            "$db" => &self.database.name
        };
        
        let response = self.database.client.send_command(cmd).await?;
        Cursor::from_response(response)
    }
    
    pub async fn insert_one(&self, doc: Document) -> Result<InsertResult, Error> {
        let cmd = doc! {
            "insert" => &self.name,
            "documents" => vec![doc],
            "$db" => &self.database.name
        };
        
        self.database.client.send_command(cmd).await?;
        Ok(InsertResult { /* ... */ })
    }
    
    pub async fn update_one(
        &self, 
        filter: Document, 
        update: Document
    ) -> Result<UpdateResult, Error> {
        let cmd = doc! {
            "update" => &self.name,
            "updates" => vec![doc! {
                "q" => filter,
                "u" => update,
                "multi" => false
            }],
            "$db" => &self.database.name
        };
        
        self.database.client.send_command(cmd).await?;
        Ok(UpdateResult { /* ... */ })
    }
    
    pub async fn delete_one(&self, filter: Document) -> Result<DeleteResult, Error> {
        let cmd = doc! {
            "delete" => &self.name,
            "deletes" => vec![doc! {
                "q" => filter,
                "limit" => 1
            }],
            "$db" => &self.database.name
        };
        
        self.database.client.send_command(cmd).await?;
        Ok(DeleteResult { /* ... */ })
    }
}
```

### 10.3 Query Builder

```rust
pub struct Query {
    filters: Vec<Document>,
}

impl Query {
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }
    
    pub fn eq(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => value.into() });
        self
    }
    
    pub fn ne(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => { "$ne" => value.into() } });
        self
    }
    
    pub fn gt(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => { "$gt" => value.into() } });
        self
    }
    
    pub fn gte(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => { "$gte" => value.into() } });
        self
    }
    
    pub fn lt(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => { "$lt" => value.into() } });
        self
    }
    
    pub fn lte(mut self, field: &str, value: impl Into<Bson>) -> Self {
        self.filters.push(doc! { field => { "$lte" => value.into() } });
        self
    }
    
    pub fn in_values(mut self, field: &str, values: Vec<Bson>) -> Self {
        self.filters.push(doc! { field => { "$in" => values } });
        self
    }
    
    pub fn exists(mut self, field: &str, exists: bool) -> Self {
        self.filters.push(doc! { field => { "$exists" => exists } });
        self
    }
    
    pub fn regex(mut self, field: &str, pattern: &str) -> Self {
        self.filters.push(doc! { field => { "$regex" => pattern } });
        self
    }
    
    pub fn or(mut self, queries: Vec<Query>) -> Self {
        let docs: Vec<Bson> = queries.into_iter()
            .map(|q| Bson::Document(q.build()))
            .collect();
        self.filters.push(doc! { "$or" => docs });
        self
    }
    
    pub fn build(self) -> Document {
        if self.filters.is_empty() {
            Document::new()
        } else if self.filters.len() == 1 {
            self.filters.into_iter().next().unwrap()
        } else {
            let docs: Vec<Bson> = self.filters.into_iter()
                .map(Bson::Document)
                .collect();
            doc! { "$and" => docs }
        }
    }
}

// Usage:
// let filter = Query::new()
//     .eq("status", "active")
//     .gte("age", 18)
//     .build();
```

### 10.4 Update Builder

```rust
pub struct UpdateBuilder {
    ops: Document,
}

impl UpdateBuilder {
    pub fn new() -> Self {
        Self { ops: Document::new() }
    }
    
    pub fn set(mut self, field: &str, value: impl Into<Bson>) -> Self {
        let set = self.ops.get("$set")
            .and_then(|v| match v { Bson::Document(d) => Some(d.clone()), _ => None })
            .unwrap_or_default();
        let mut set = set;
        set.insert(field, value.into());
        self.ops.insert("$set", Bson::Document(set));
        self
    }
    
    pub fn unset(mut self, field: &str) -> Self {
        let unset = self.ops.get("$unset")
            .and_then(|v| match v { Bson::Document(d) => Some(d.clone()), _ => None })
            .unwrap_or_default();
        let mut unset = unset;
        unset.insert(field, "");
        self.ops.insert("$unset", Bson::Document(unset));
        self
    }
    
    pub fn inc(mut self, field: &str, value: impl Into<Bson>) -> Self {
        let inc = self.ops.get("$inc")
            .and_then(|v| match v { Bson::Document(d) => Some(d.clone()), _ => None })
            .unwrap_or_default();
        let mut inc = inc;
        inc.insert(field, value.into());
        self.ops.insert("$inc", Bson::Document(inc));
        self
    }
    
    pub fn push(mut self, field: &str, value: impl Into<Bson>) -> Self {
        let push = self.ops.get("$push")
            .and_then(|v| match v { Bson::Document(d) => Some(d.clone()), _ => None })
            .unwrap_or_default();
        let mut push = push;
        push.insert(field, value.into());
        self.ops.insert("$push", Bson::Document(push));
        self
    }
    
    pub fn build(self) -> Document {
        self.ops
    }
}

// Usage:
// let update = UpdateBuilder::new()
//     .set("name", "John")
//     .inc("visits", 1)
//     .build();
```

### 10.5 Error Types

```rust
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Protocol(ProtocolError),
    Bson(BsonError),
    CommandError { code: i32, message: String },
    AuthenticationError(String),
    NoServerAvailable,
    Timeout,
}

#[derive(Debug)]
pub enum ProtocolError {
    InsufficientData,
    InvalidMessage,
    UnknownOpcode(i32),
    ChecksumMismatch,
}

#[derive(Debug)]
pub enum BsonError {
    InsufficientData,
    InvalidCString,
    InvalidUtf8,
    UnknownType(u8),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
```

---

## Appendix A: Connection String Format

```
mongodb://[username:password@]host[:port][/database][?options]
mongodb+srv://[username:password@]host[/database][?options]
```

### Common Options

| Option | Type | Description |
|--------|------|-------------|
| `replicaSet` | string | Replica set name |
| `authSource` | string | Auth database |
| `authMechanism` | string | SCRAM-SHA-256, etc. |
| `w` | int/string | Write concern |
| `readPreference` | string | Read preference mode |
| `connectTimeoutMS` | int | Connection timeout |
| `socketTimeoutMS` | int | Socket timeout |
| `maxPoolSize` | int | Max connections |
| `retryWrites` | bool | Retry writes |
| `retryReads` | bool | Retry reads |
| `tls` | bool | Enable TLS |

---

## Appendix B: Quick Reference

### BSON Types

| Type | Code | Size |
|------|------|------|
| Double | 0x01 | 8 bytes |
| String | 0x02 | Variable |
| Document | 0x03 | Variable |
| Array | 0x04 | Variable |
| Binary | 0x05 | Variable |
| ObjectId | 0x07 | 12 bytes |
| Boolean | 0x08 | 1 byte |
| DateTime | 0x09 | 8 bytes |
| Null | 0x0A | 0 bytes |
| Int32 | 0x10 | 4 bytes |
| Timestamp | 0x11 | 8 bytes |
| Int64 | 0x12 | 8 bytes |
| Decimal128 | 0x13 | 16 bytes |

### Error Codes (Common)

| Code | Name |
|------|------|
| 11000 | DuplicateKey |
| 43 | CursorNotFound |
| 50 | MaxTimeMSExpired |
| 91 | ShutdownInProgress |
| 112 | WriteConflict |
| 211 | KeyNotFound |
| 251 | NoSuchTransaction |
| 262 | ExceededTimeLimit |

---

*Document Version: 1.0*  
*Target: Rust Implementation*
