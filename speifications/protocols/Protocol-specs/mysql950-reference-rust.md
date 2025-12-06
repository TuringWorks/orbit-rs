# MySQL 9.5.0 Protocol and SQL Reference (Rust Edition)

**Version:** 1.0  
**Target:** MySQL 9.5.0 (Innovation Release, October 21, 2025)  
**Purpose:** Comprehensive reference for LLM coding tools creating MySQL-related code  
**Language:** Rust

---

## Table of Contents

1. [Overview](#1-overview)
2. [Wire Protocol](#2-wire-protocol)
3. [Protocol Data Types](#3-protocol-data-types)
4. [Connection Phase](#4-connection-phase)
5. [Command Phase](#5-command-phase)
6. [Result Set Protocol](#6-result-set-protocol)
7. [Prepared Statements](#7-prepared-statements)
8. [SQL Keywords](#8-sql-keywords)
9. [Data Types and Type Codes](#9-data-types-and-type-codes)
10. [Parser Architecture](#10-parser-architecture)
11. [AST Definitions](#11-ast-definitions)
12. [Error Handling](#12-error-handling)
13. [Authentication](#13-authentication)
14. [MySQL 9.5.0 New Features](#14-mysql-950-new-features)
15. [Implementation Libraries](#15-implementation-libraries)
16. [Complete Connection Example](#16-complete-connection-example)

---

## 1. Overview

### Version Information

- **Version:** MySQL 9.5.0
- **Release Type:** Innovation Release
- **Release Date:** October 21, 2025
- **Protocol Version:** 10 (classic protocol)
- **Default Port:** 3306
- **X Protocol Port:** 33060

### Cargo Dependencies

```toml
[dependencies]
byteorder = "1.5"
sha1 = "0.10"
sha2 = "0.10"
tokio = { version = "1.35", features = ["full"] }
bytes = "1.5"
thiserror = "1.0"
tracing = "0.1"
mysql_async = "0.34"        # High-level async client
sqlparser = "0.41"          # SQL parsing
```

---

## 2. Wire Protocol

### Packet Structure

All MySQL packets follow this structure:

```text
+----------------+------------------+----------------+
| Payload Length | Sequence ID      | Payload        |
| 3 bytes (LE)   | 1 byte           | n bytes        |
+----------------+------------------+----------------+
```

### Packet Header

```rust
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Write};

/// Maximum payload size for a single MySQL packet (16MB - 1)
pub const MAX_PACKET_LENGTH: usize = 0xFFFFFF; // 16,777,215 bytes

/// MySQL packet header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub payload_length: u32,
    pub sequence_id: u8,
}

impl PacketHeader {
    pub const SIZE: usize = 4;

    /// Parse header from bytes
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Packet header too short",
            ));
        }

        // Payload length is 3 bytes, little-endian
        let payload_length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
        let sequence_id = bytes[3];

        Ok(Self {
            payload_length,
            sequence_id,
        })
    }

    /// Write header to bytes
    pub fn to_bytes(&self) -> [u8; 4] {
        let len_bytes = self.payload_length.to_le_bytes();
        [len_bytes[0], len_bytes[1], len_bytes[2], self.sequence_id]
    }

    /// Create new header
    pub fn new(payload_length: usize, sequence_id: u8) -> Self {
        debug_assert!(payload_length <= MAX_PACKET_LENGTH);
        Self {
            payload_length: payload_length as u32,
            sequence_id,
        }
    }
}

/// Complete MySQL packet with header and payload
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(payload: Vec<u8>, sequence_id: u8) -> Self {
        Self {
            header: PacketHeader::new(payload.len(), sequence_id),
            payload,
        }
    }

    /// Serialize packet to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.payload.len());
        buf.extend_from_slice(&self.header.to_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }
}
```

---

## 3. Protocol Data Types

### Integer Types

```rust
/// Read/write utilities for MySQL protocol integers
pub mod integers {
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
    use std::io::{self, Cursor, Read, Write};

    /// Read 3-byte integer (little-endian)
    pub fn read_int3(bytes: &[u8]) -> u32 {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
    }

    /// Write 3-byte integer (little-endian)
    pub fn write_int3(value: u32) -> [u8; 3] {
        let bytes = value.to_le_bytes();
        [bytes[0], bytes[1], bytes[2]]
    }

    /// Read 6-byte integer (little-endian)
    pub fn read_int6(bytes: &[u8]) -> u64 {
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], 0, 0,
        ])
    }

    /// Write 6-byte integer (little-endian)
    pub fn write_int6(value: u64) -> [u8; 6] {
        let bytes = value.to_le_bytes();
        [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]]
    }
}
```

### Length-Encoded Integer

```rust
/// Length-encoded integer markers
pub const LENENC_NULL: u8 = 0xFB;
pub const LENENC_INT2: u8 = 0xFC;
pub const LENENC_INT3: u8 = 0xFD;
pub const LENENC_INT8: u8 = 0xFE;

/// Result of reading a length-encoded integer
#[derive(Debug, Clone, Copy)]
pub struct LenEncInt {
    pub value: u64,
    pub bytes_read: usize,
}

/// Read a length-encoded integer from bytes
pub fn read_lenenc_int(bytes: &[u8]) -> Option<LenEncInt> {
    if bytes.is_empty() {
        return None;
    }

    let first = bytes[0];

    match first {
        0x00..=0xFA => Some(LenEncInt {
            value: first as u64,
            bytes_read: 1,
        }),
        LENENC_INT2 if bytes.len() >= 3 => {
            let value = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
            Some(LenEncInt {
                value,
                bytes_read: 3,
            })
        }
        LENENC_INT3 if bytes.len() >= 4 => {
            let value = integers::read_int3(&bytes[1..]) as u64;
            Some(LenEncInt {
                value,
                bytes_read: 4,
            })
        }
        LENENC_INT8 if bytes.len() >= 9 => {
            let value = u64::from_le_bytes([
                bytes[1], bytes[2], bytes[3], bytes[4],
                bytes[5], bytes[6], bytes[7], bytes[8],
            ]);
            Some(LenEncInt {
                value,
                bytes_read: 9,
            })
        }
        _ => None,
    }
}

/// Write a length-encoded integer
pub fn write_lenenc_int(value: u64) -> Vec<u8> {
    if value < 0xFB {
        vec![value as u8]
    } else if value <= 0xFFFF {
        let bytes = (value as u16).to_le_bytes();
        vec![LENENC_INT2, bytes[0], bytes[1]]
    } else if value <= 0xFFFFFF {
        let bytes = integers::write_int3(value as u32);
        vec![LENENC_INT3, bytes[0], bytes[1], bytes[2]]
    } else {
        let bytes = value.to_le_bytes();
        vec![
            LENENC_INT8,
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lenenc_int_roundtrip() {
        for value in [0u64, 100, 250, 251, 0xFFFF, 0xFFFFFF, u64::MAX] {
            let encoded = write_lenenc_int(value);
            let decoded = read_lenenc_int(&encoded).unwrap();
            assert_eq!(decoded.value, value);
        }
    }
}
```

### String Types

```rust
/// MySQL string types
#[derive(Debug, Clone)]
pub struct MysqlString {
    pub data: Vec<u8>,
    pub bytes_read: usize,
}

impl MysqlString {
    /// Get as UTF-8 string (lossy conversion)
    pub fn as_str(&self) -> std::borrow::Cow<str> {
        String::from_utf8_lossy(&self.data)
    }

    /// Get as owned String
    pub fn into_string(self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.data)
    }
}

/// Read NUL-terminated string
pub fn read_nul_string(bytes: &[u8]) -> Option<MysqlString> {
    let nul_pos = bytes.iter().position(|&b| b == 0)?;
    Some(MysqlString {
        data: bytes[..nul_pos].to_vec(),
        bytes_read: nul_pos + 1,
    })
}

/// Read length-encoded string
pub fn read_lenenc_string(bytes: &[u8]) -> Option<MysqlString> {
    let len = read_lenenc_int(bytes)?;
    let start = len.bytes_read;
    let end = start + len.value as usize;

    if bytes.len() < end {
        return None;
    }

    Some(MysqlString {
        data: bytes[start..end].to_vec(),
        bytes_read: end,
    })
}

/// Read fixed-length string
pub fn read_fixed_string(bytes: &[u8], length: usize) -> Option<MysqlString> {
    if bytes.len() < length {
        return None;
    }

    Some(MysqlString {
        data: bytes[..length].to_vec(),
        bytes_read: length,
    })
}

/// Read rest-of-packet string (EOF string)
pub fn read_eof_string(bytes: &[u8]) -> MysqlString {
    MysqlString {
        data: bytes.to_vec(),
        bytes_read: bytes.len(),
    }
}

/// Write NUL-terminated string
pub fn write_nul_string(s: &str) -> Vec<u8> {
    let mut buf = s.as_bytes().to_vec();
    buf.push(0);
    buf
}

/// Write length-encoded string
pub fn write_lenenc_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut buf = write_lenenc_int(bytes.len() as u64);
    buf.extend_from_slice(bytes);
    buf
}
```

---

## 4. Connection Phase

### Server Greeting (Handshake Packet v10)

```rust
/// MySQL Server Handshake Packet (Protocol Version 10)
#[derive(Debug, Clone)]
pub struct HandshakeV10 {
    pub protocol_version: u8,
    pub server_version: String,
    pub connection_id: u32,
    pub auth_plugin_data_part1: [u8; 8],
    pub capability_flags: u32,
    pub character_set: u8,
    pub status_flags: u16,
    pub auth_plugin_data_part2: Vec<u8>,
    pub auth_plugin_name: String,
}

impl HandshakeV10 {
    /// Parse handshake packet from payload bytes
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut pos = 0;

        // Protocol version
        let protocol_version = payload[pos];
        pos += 1;

        if protocol_version != 10 {
            return Err(ProtocolError::UnsupportedProtocolVersion(protocol_version));
        }

        // Server version (NUL-terminated)
        let version_str = read_nul_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let server_version = version_str.as_str().to_string();
        pos += version_str.bytes_read;

        // Connection ID (4 bytes)
        let connection_id = u32::from_le_bytes([
            payload[pos], payload[pos + 1],
            payload[pos + 2], payload[pos + 3],
        ]);
        pos += 4;

        // Auth plugin data part 1 (8 bytes)
        let mut auth_plugin_data_part1 = [0u8; 8];
        auth_plugin_data_part1.copy_from_slice(&payload[pos..pos + 8]);
        pos += 8;

        // Filler (1 byte, always 0x00)
        pos += 1;

        // Capability flags (lower 2 bytes)
        let cap_lower = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        // Character set (1 byte)
        let character_set = payload[pos];
        pos += 1;

        // Status flags (2 bytes)
        let status_flags = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        // Capability flags (upper 2 bytes)
        let cap_upper = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        let capability_flags = (cap_lower as u32) | ((cap_upper as u32) << 16);

        // Auth plugin data length (1 byte)
        let auth_data_len = payload[pos] as usize;
        pos += 1;

        // Reserved (10 bytes)
        pos += 10;

        // Auth plugin data part 2
        let part2_len = auth_data_len.saturating_sub(8).max(13);
        let auth_plugin_data_part2 = payload[pos..pos + part2_len - 1].to_vec();
        pos += part2_len;

        // Auth plugin name (NUL-terminated)
        let plugin_str = read_nul_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let auth_plugin_name = plugin_str.as_str().to_string();

        Ok(Self {
            protocol_version,
            server_version,
            connection_id,
            auth_plugin_data_part1,
            capability_flags,
            character_set,
            status_flags,
            auth_plugin_data_part2,
            auth_plugin_name,
        })
    }

    /// Get complete auth scramble (20 bytes for native auth)
    pub fn auth_scramble(&self) -> Vec<u8> {
        let mut scramble = self.auth_plugin_data_part1.to_vec();
        scramble.extend_from_slice(&self.auth_plugin_data_part2);
        scramble
    }
}
```

### Capability Flags

```rust
use bitflags::bitflags;

bitflags! {
    /// MySQL client/server capability flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilityFlags: u32 {
        const LONG_PASSWORD                  = 0x00000001;
        const FOUND_ROWS                     = 0x00000002;
        const LONG_FLAG                      = 0x00000004;
        const CONNECT_WITH_DB                = 0x00000008;
        const NO_SCHEMA                      = 0x00000010;
        const COMPRESS                       = 0x00000020;
        const ODBC                           = 0x00000040;
        const LOCAL_FILES                    = 0x00000080;
        const IGNORE_SPACE                   = 0x00000100;
        const PROTOCOL_41                    = 0x00000200;
        const INTERACTIVE                    = 0x00000400;
        const SSL                            = 0x00000800;
        const IGNORE_SIGPIPE                 = 0x00001000;
        const TRANSACTIONS                   = 0x00002000;
        const RESERVED                       = 0x00004000;
        const SECURE_CONNECTION              = 0x00008000;
        const MULTI_STATEMENTS               = 0x00010000;
        const MULTI_RESULTS                  = 0x00020000;
        const PS_MULTI_RESULTS               = 0x00040000;
        const PLUGIN_AUTH                    = 0x00080000;
        const CONNECT_ATTRS                  = 0x00100000;
        const PLUGIN_AUTH_LENENC_CLIENT_DATA = 0x00200000;
        const CAN_HANDLE_EXPIRED_PASSWORDS   = 0x00400000;
        const SESSION_TRACK                  = 0x00800000;
        const DEPRECATE_EOF                  = 0x01000000;
        const OPTIONAL_RESULTSET_METADATA    = 0x02000000;
        const ZSTD_COMPRESSION_ALGORITHM     = 0x04000000;
        const QUERY_ATTRIBUTES               = 0x08000000;
        const MULTI_FACTOR_AUTHENTICATION    = 0x10000000;
        const CAPABILITY_EXTENSION           = 0x20000000;
        const SSL_VERIFY_SERVER_CERT         = 0x40000000;
        const REMEMBER_OPTIONS               = 0x80000000;
    }
}

impl Default for CapabilityFlags {
    fn default() -> Self {
        Self::PROTOCOL_41
            | Self::SECURE_CONNECTION
            | Self::LONG_PASSWORD
            | Self::TRANSACTIONS
            | Self::PLUGIN_AUTH
            | Self::DEPRECATE_EOF
            | Self::SESSION_TRACK
            | Self::PLUGIN_AUTH_LENENC_CLIENT_DATA
    }
}
```

### Handshake Response

```rust
/// Client handshake response (protocol 4.1)
#[derive(Debug)]
pub struct HandshakeResponse41 {
    pub capability_flags: CapabilityFlags,
    pub max_packet_size: u32,
    pub character_set: u8,
    pub username: String,
    pub auth_response: Vec<u8>,
    pub database: Option<String>,
    pub auth_plugin_name: String,
    pub connect_attrs: Option<Vec<(String, String)>>,
}

impl HandshakeResponse41 {
    /// Build the handshake response packet payload
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);

        // Capability flags (4 bytes)
        buf.extend_from_slice(&self.capability_flags.bits().to_le_bytes());

        // Max packet size (4 bytes)
        buf.extend_from_slice(&self.max_packet_size.to_le_bytes());

        // Character set (1 byte)
        buf.push(self.character_set);

        // Reserved (23 bytes of zeros)
        buf.extend_from_slice(&[0u8; 23]);

        // Username (NUL-terminated)
        buf.extend_from_slice(self.username.as_bytes());
        buf.push(0);

        // Auth response
        if self.capability_flags.contains(CapabilityFlags::PLUGIN_AUTH_LENENC_CLIENT_DATA) {
            buf.extend(write_lenenc_int(self.auth_response.len() as u64));
            buf.extend_from_slice(&self.auth_response);
        } else if self.capability_flags.contains(CapabilityFlags::SECURE_CONNECTION) {
            buf.push(self.auth_response.len() as u8);
            buf.extend_from_slice(&self.auth_response);
        }

        // Database (if CONNECT_WITH_DB)
        if let Some(ref db) = self.database {
            if self.capability_flags.contains(CapabilityFlags::CONNECT_WITH_DB) {
                buf.extend_from_slice(db.as_bytes());
                buf.push(0);
            }
        }

        // Auth plugin name (if PLUGIN_AUTH)
        if self.capability_flags.contains(CapabilityFlags::PLUGIN_AUTH) {
            buf.extend_from_slice(self.auth_plugin_name.as_bytes());
            buf.push(0);
        }

        // Connection attributes (if CONNECT_ATTRS)
        if let Some(ref attrs) = self.connect_attrs {
            if self.capability_flags.contains(CapabilityFlags::CONNECT_ATTRS) {
                let mut attr_buf = Vec::new();
                for (key, value) in attrs {
                    attr_buf.extend(write_lenenc_string(key));
                    attr_buf.extend(write_lenenc_string(value));
                }
                buf.extend(write_lenenc_int(attr_buf.len() as u64));
                buf.extend(attr_buf);
            }
        }

        buf
    }
}
```

---

## 5. Command Phase

### Command Types

```rust
/// MySQL command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Sleep               = 0x00,
    Quit                = 0x01,
    InitDb              = 0x02,
    Query               = 0x03,
    FieldList           = 0x04,
    CreateDb            = 0x05,
    DropDb              = 0x06,
    Refresh             = 0x07,
    Shutdown            = 0x08,
    Statistics          = 0x09,
    ProcessInfo         = 0x0A,
    Connect             = 0x0B,
    ProcessKill         = 0x0C,
    Debug               = 0x0D,
    Ping                = 0x0E,
    Time                = 0x0F,
    DelayedInsert       = 0x10,
    ChangeUser          = 0x11,
    BinlogDump          = 0x12,
    TableDump           = 0x13,
    ConnectOut          = 0x14,
    RegisterSlave       = 0x15,
    StmtPrepare         = 0x16,
    StmtExecute         = 0x17,
    StmtSendLongData    = 0x18,
    StmtClose           = 0x19,
    StmtReset           = 0x1A,
    SetOption           = 0x1B,
    StmtFetch           = 0x1C,
    Daemon              = 0x1D,
    BinlogDumpGtid      = 0x1E,
    ResetConnection     = 0x1F,
    Clone               = 0x20,
}

impl Command {
    /// Build a simple query packet
    pub fn query(sql: &str) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + sql.len());
        buf.push(Command::Query as u8);
        buf.extend_from_slice(sql.as_bytes());
        buf
    }

    /// Build a ping packet
    pub fn ping() -> Vec<u8> {
        vec![Command::Ping as u8]
    }

    /// Build a quit packet
    pub fn quit() -> Vec<u8> {
        vec![Command::Quit as u8]
    }

    /// Build an init_db packet
    pub fn init_db(database: &str) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + database.len());
        buf.push(Command::InitDb as u8);
        buf.extend_from_slice(database.as_bytes());
        buf
    }
}
```

---

## 6. Result Set Protocol

### Response Packet Types

```rust
/// Response packet type indicators
pub const PACKET_OK: u8 = 0x00;
pub const PACKET_LOCALINFILE: u8 = 0xFB;
pub const PACKET_EOF: u8 = 0xFE;
pub const PACKET_ERR: u8 = 0xFF;

/// Determine packet type from first byte
pub fn packet_type(first_byte: u8, payload_len: usize) -> PacketType {
    match first_byte {
        PACKET_OK => PacketType::Ok,
        PACKET_LOCALINFILE => PacketType::LocalInFile,
        PACKET_EOF if payload_len < 9 => PacketType::Eof,
        PACKET_ERR => PacketType::Err,
        _ => PacketType::ResultSet,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Ok,
    Eof,
    Err,
    LocalInFile,
    ResultSet,
}
```

### OK Packet

```rust
/// MySQL OK packet
#[derive(Debug, Clone)]
pub struct OkPacket {
    pub affected_rows: u64,
    pub last_insert_id: u64,
    pub status_flags: StatusFlags,
    pub warnings: u16,
    pub info: Option<String>,
    pub session_state_changes: Option<Vec<u8>>,
}

impl OkPacket {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut pos = 0;

        // Header (0x00)
        if payload[pos] != PACKET_OK {
            return Err(ProtocolError::UnexpectedPacketType);
        }
        pos += 1;

        // Affected rows
        let affected = read_lenenc_int(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let affected_rows = affected.value;
        pos += affected.bytes_read;

        // Last insert ID
        let insert_id = read_lenenc_int(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let last_insert_id = insert_id.value;
        pos += insert_id.bytes_read;

        // Status flags (2 bytes)
        let status_flags = StatusFlags::from_bits_truncate(
            u16::from_le_bytes([payload[pos], payload[pos + 1]])
        );
        pos += 2;

        // Warnings (2 bytes)
        let warnings = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        // Info string (rest of packet)
        let info = if pos < payload.len() {
            Some(String::from_utf8_lossy(&payload[pos..]).to_string())
        } else {
            None
        };

        Ok(Self {
            affected_rows,
            last_insert_id,
            status_flags,
            warnings,
            info,
            session_state_changes: None,
        })
    }
}
```

### Error Packet

```rust
/// MySQL error packet
#[derive(Debug, Clone)]
pub struct ErrPacket {
    pub error_code: u16,
    pub sql_state: String,
    pub error_message: String,
}

impl ErrPacket {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut pos = 0;

        // Header (0xFF)
        if payload[pos] != PACKET_ERR {
            return Err(ProtocolError::UnexpectedPacketType);
        }
        pos += 1;

        // Error code (2 bytes)
        let error_code = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        // SQL state marker '#' and state (6 bytes total)
        let sql_state = if payload[pos] == b'#' {
            pos += 1;
            let state = String::from_utf8_lossy(&payload[pos..pos + 5]).to_string();
            pos += 5;
            state
        } else {
            "HY000".to_string()
        };

        // Error message (rest of packet)
        let error_message = String::from_utf8_lossy(&payload[pos..]).to_string();

        Ok(Self {
            error_code,
            sql_state,
            error_message,
        })
    }
}

impl std::fmt::Display for ErrPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MySQL Error {}: {} (SQLSTATE: {})",
            self.error_code, self.error_message, self.sql_state
        )
    }
}

impl std::error::Error for ErrPacket {}
```

### Status Flags

```rust
bitflags! {
    /// Server status flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StatusFlags: u16 {
        const IN_TRANS              = 0x0001;
        const AUTOCOMMIT            = 0x0002;
        const MORE_RESULTS_EXISTS   = 0x0008;
        const NO_GOOD_INDEX_USED    = 0x0010;
        const NO_INDEX_USED         = 0x0020;
        const CURSOR_EXISTS         = 0x0040;
        const LAST_ROW_SENT         = 0x0080;
        const DB_DROPPED            = 0x0100;
        const NO_BACKSLASH_ESCAPES  = 0x0200;
        const METADATA_CHANGED      = 0x0400;
        const QUERY_WAS_SLOW        = 0x0800;
        const PS_OUT_PARAMS         = 0x1000;
        const IN_TRANS_READONLY     = 0x2000;
        const SESSION_STATE_CHANGED = 0x4000;
    }
}
```

### Column Definition

```rust
/// Column definition packet
#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub org_table: String,
    pub name: String,
    pub org_name: String,
    pub character_set: u16,
    pub column_length: u32,
    pub column_type: FieldType,
    pub flags: ColumnFlags,
    pub decimals: u8,
}

impl ColumnDefinition {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut pos = 0;

        // Catalog
        let catalog_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let catalog = catalog_str.as_str().to_string();
        pos += catalog_str.bytes_read;

        // Schema
        let schema_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let schema = schema_str.as_str().to_string();
        pos += schema_str.bytes_read;

        // Table
        let table_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let table = table_str.as_str().to_string();
        pos += table_str.bytes_read;

        // Org table
        let org_table_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let org_table = org_table_str.as_str().to_string();
        pos += org_table_str.bytes_read;

        // Name
        let name_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let name = name_str.as_str().to_string();
        pos += name_str.bytes_read;

        // Org name
        let org_name_str = read_lenenc_string(&payload[pos..])
            .ok_or(ProtocolError::MalformedPacket)?;
        let org_name = org_name_str.as_str().to_string();
        pos += org_name_str.bytes_read;

        // Fixed length fields marker (0x0C = 12)
        let _fixed_len = payload[pos];
        pos += 1;

        // Character set (2 bytes)
        let character_set = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        pos += 2;

        // Column length (4 bytes)
        let column_length = u32::from_le_bytes([
            payload[pos], payload[pos + 1],
            payload[pos + 2], payload[pos + 3],
        ]);
        pos += 4;

        // Column type (1 byte)
        let column_type = FieldType::try_from(payload[pos])
            .unwrap_or(FieldType::Unknown);
        pos += 1;

        // Flags (2 bytes)
        let flags = ColumnFlags::from_bits_truncate(
            u16::from_le_bytes([payload[pos], payload[pos + 1]])
        );
        pos += 2;

        // Decimals (1 byte)
        let decimals = payload[pos];

        Ok(Self {
            catalog,
            schema,
            table,
            org_table,
            name,
            org_name,
            character_set,
            column_length,
            column_type,
            flags,
            decimals,
        })
    }
}

bitflags! {
    /// Column flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ColumnFlags: u16 {
        const NOT_NULL        = 0x0001;
        const PRI_KEY         = 0x0002;
        const UNIQUE_KEY      = 0x0004;
        const MULTIPLE_KEY    = 0x0008;
        const BLOB            = 0x0010;
        const UNSIGNED        = 0x0020;
        const ZEROFILL        = 0x0040;
        const BINARY          = 0x0080;
        const ENUM            = 0x0100;
        const AUTO_INCREMENT  = 0x0200;
        const TIMESTAMP       = 0x0400;
        const SET             = 0x0800;
        const NO_DEFAULT      = 0x1000;
        const ON_UPDATE_NOW   = 0x2000;
        const NUM             = 0x8000;
    }
}
```

### Result Row

```rust
/// Text protocol result row
#[derive(Debug, Clone)]
pub struct TextRow {
    pub values: Vec<Option<Vec<u8>>>,
}

impl TextRow {
    pub fn parse(payload: &[u8], column_count: usize) -> Result<Self, ProtocolError> {
        let mut values = Vec::with_capacity(column_count);
        let mut pos = 0;

        for _ in 0..column_count {
            if pos >= payload.len() {
                return Err(ProtocolError::MalformedPacket);
            }

            if payload[pos] == LENENC_NULL {
                values.push(None);
                pos += 1;
            } else {
                let str_val = read_lenenc_string(&payload[pos..])
                    .ok_or(ProtocolError::MalformedPacket)?;
                values.push(Some(str_val.data));
                pos += str_val.bytes_read;
            }
        }

        Ok(Self { values })
    }

    /// Get value as string at index
    pub fn get_string(&self, idx: usize) -> Option<String> {
        self.values.get(idx)?.as_ref().map(|v| {
            String::from_utf8_lossy(v).to_string()
        })
    }

    /// Get value as i64 at index
    pub fn get_i64(&self, idx: usize) -> Option<i64> {
        let s = self.get_string(idx)?;
        s.parse().ok()
    }

    /// Get value as f64 at index
    pub fn get_f64(&self, idx: usize) -> Option<f64> {
        let s = self.get_string(idx)?;
        s.parse().ok()
    }
}
```

---

## 7. Prepared Statements

### Statement Prepare

```rust
/// COM_STMT_PREPARE_OK response
#[derive(Debug, Clone)]
pub struct StmtPrepareOk {
    pub statement_id: u32,
    pub num_columns: u16,
    pub num_params: u16,
    pub warning_count: u16,
}

impl StmtPrepareOk {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload[0] != 0x00 {
            return Err(ProtocolError::UnexpectedPacketType);
        }

        Ok(Self {
            statement_id: u32::from_le_bytes([
                payload[1], payload[2], payload[3], payload[4],
            ]),
            num_columns: u16::from_le_bytes([payload[5], payload[6]]),
            num_params: u16::from_le_bytes([payload[7], payload[8]]),
            // payload[9] is reserved (always 0x00)
            warning_count: u16::from_le_bytes([payload[10], payload[11]]),
        })
    }
}

/// Build COM_STMT_PREPARE packet
pub fn build_stmt_prepare(query: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + query.len());
    buf.push(Command::StmtPrepare as u8);
    buf.extend_from_slice(query.as_bytes());
    buf
}
```

### Statement Execute

```rust
/// Cursor types for prepared statement execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CursorType {
    NoCursor   = 0x00,
    ReadOnly   = 0x01,
    ForUpdate  = 0x02,
    Scrollable = 0x04,
}

/// Parameter binding for prepared statements
#[derive(Debug, Clone)]
pub struct ParamBind {
    pub field_type: FieldType,
    pub value: ParamValue,
    pub is_unsigned: bool,
}

#[derive(Debug, Clone)]
pub enum ParamValue {
    Null,
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Int64(i64),
    UInt64(u64),
    Float(f32),
    Double(f64),
    String(String),
    Bytes(Vec<u8>),
    Date { year: u16, month: u8, day: u8 },
    Time { is_negative: bool, days: u32, hours: u8, minutes: u8, seconds: u8, microseconds: u32 },
    DateTime { year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8, microseconds: u32 },
}

impl ParamBind {
    pub fn null() -> Self {
        Self {
            field_type: FieldType::Null,
            value: ParamValue::Null,
            is_unsigned: false,
        }
    }

    pub fn int(value: i64) -> Self {
        Self {
            field_type: FieldType::LongLong,
            value: ParamValue::Int64(value),
            is_unsigned: false,
        }
    }

    pub fn uint(value: u64) -> Self {
        Self {
            field_type: FieldType::LongLong,
            value: ParamValue::UInt64(value),
            is_unsigned: true,
        }
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self {
            field_type: FieldType::VarString,
            value: ParamValue::String(value.into()),
            is_unsigned: false,
        }
    }

    pub fn double(value: f64) -> Self {
        Self {
            field_type: FieldType::Double,
            value: ParamValue::Double(value),
            is_unsigned: false,
        }
    }

    pub fn bytes(value: Vec<u8>) -> Self {
        Self {
            field_type: FieldType::Blob,
            value: ParamValue::Bytes(value),
            is_unsigned: false,
        }
    }
}

/// Build COM_STMT_EXECUTE packet
pub fn build_stmt_execute(
    statement_id: u32,
    cursor_type: CursorType,
    params: &[ParamBind],
) -> Vec<u8> {
    let mut buf = Vec::new();

    // Command
    buf.push(Command::StmtExecute as u8);

    // Statement ID (4 bytes)
    buf.extend_from_slice(&statement_id.to_le_bytes());

    // Cursor flags (1 byte)
    buf.push(cursor_type as u8);

    // Iteration count (4 bytes, always 1)
    buf.extend_from_slice(&1u32.to_le_bytes());

    if !params.is_empty() {
        // NULL bitmap
        let bitmap_size = (params.len() + 7) / 8;
        let mut bitmap = vec![0u8; bitmap_size];
        for (i, param) in params.iter().enumerate() {
            if matches!(param.value, ParamValue::Null) {
                bitmap[i / 8] |= 1 << (i % 8);
            }
        }
        buf.extend_from_slice(&bitmap);

        // New params bound flag
        buf.push(0x01);

        // Parameter types (2 bytes each)
        for param in params {
            buf.push(param.field_type as u8);
            buf.push(if param.is_unsigned { 0x80 } else { 0x00 });
        }

        // Parameter values
        for param in params {
            match &param.value {
                ParamValue::Null => {}
                ParamValue::Int8(v) => buf.push(*v as u8),
                ParamValue::UInt8(v) => buf.push(*v),
                ParamValue::Int16(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::UInt16(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::Int32(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::UInt32(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::Int64(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::UInt64(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::Float(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::Double(v) => buf.extend_from_slice(&v.to_le_bytes()),
                ParamValue::String(s) => {
                    buf.extend(write_lenenc_string(s));
                }
                ParamValue::Bytes(b) => {
                    buf.extend(write_lenenc_int(b.len() as u64));
                    buf.extend_from_slice(b);
                }
                ParamValue::Date { year, month, day } => {
                    buf.push(4); // length
                    buf.extend_from_slice(&year.to_le_bytes());
                    buf.push(*month);
                    buf.push(*day);
                }
                ParamValue::DateTime { year, month, day, hour, minute, second, microseconds } => {
                    if *microseconds > 0 {
                        buf.push(11); // length
                    } else {
                        buf.push(7); // length
                    }
                    buf.extend_from_slice(&year.to_le_bytes());
                    buf.push(*month);
                    buf.push(*day);
                    buf.push(*hour);
                    buf.push(*minute);
                    buf.push(*second);
                    if *microseconds > 0 {
                        buf.extend_from_slice(&microseconds.to_le_bytes());
                    }
                }
                ParamValue::Time { is_negative, days, hours, minutes, seconds, microseconds } => {
                    if *microseconds > 0 {
                        buf.push(12); // length
                    } else {
                        buf.push(8); // length
                    }
                    buf.push(if *is_negative { 1 } else { 0 });
                    buf.extend_from_slice(&days.to_le_bytes());
                    buf.push(*hours);
                    buf.push(*minutes);
                    buf.push(*seconds);
                    if *microseconds > 0 {
                        buf.extend_from_slice(&microseconds.to_le_bytes());
                    }
                }
            }
        }
    }

    buf
}

/// Build COM_STMT_CLOSE packet
pub fn build_stmt_close(statement_id: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    buf.push(Command::StmtClose as u8);
    buf.extend_from_slice(&statement_id.to_le_bytes());
    buf
}
```

---

## 8. SQL Keywords

### Reserved Keywords

```rust
/// Check if a word is a MySQL reserved keyword
pub fn is_reserved_keyword(word: &str) -> bool {
    RESERVED_KEYWORDS.binary_search(&word.to_uppercase().as_str()).is_ok()
}

/// MySQL 9.5 reserved keywords (sorted for binary search)
pub const RESERVED_KEYWORDS: &[&str] = &[
    "ACCESSIBLE", "ADD", "ALL", "ALTER", "ANALYZE", "AND", "AS", "ASC",
    "ASENSITIVE", "BEFORE", "BETWEEN", "BIGINT", "BINARY", "BLOB", "BOTH",
    "BY", "CALL", "CASCADE", "CASE", "CHANGE", "CHAR", "CHARACTER", "CHECK",
    "COLLATE", "COLUMN", "CONDITION", "CONSTRAINT", "CONTINUE", "CONVERT",
    "CREATE", "CROSS", "CUBE", "CUME_DIST", "CURRENT_DATE", "CURRENT_TIME",
    "CURRENT_TIMESTAMP", "CURRENT_USER", "CURSOR", "DATABASE", "DATABASES",
    "DAY_HOUR", "DAY_MICROSECOND", "DAY_MINUTE", "DAY_SECOND", "DEC",
    "DECIMAL", "DECLARE", "DEFAULT", "DELAYED", "DELETE", "DENSE_RANK",
    "DESC", "DESCRIBE", "DETERMINISTIC", "DISTINCT", "DISTINCTROW", "DIV",
    "DOUBLE", "DROP", "DUAL", "EACH", "ELSE", "ELSEIF", "EMPTY", "ENCLOSED",
    "ESCAPED", "EXCEPT", "EXISTS", "EXIT", "EXPLAIN", "FALSE", "FETCH",
    "FIRST_VALUE", "FLOAT", "FLOAT4", "FLOAT8", "FOR", "FORCE", "FOREIGN",
    "FROM", "FULLTEXT", "FUNCTION", "GENERATED", "GET", "GRANT", "GROUP",
    "GROUPING", "GROUPS", "HAVING", "HIGH_PRIORITY", "HOUR_MICROSECOND",
    "HOUR_MINUTE", "HOUR_SECOND", "IF", "IGNORE", "IN", "INDEX", "INFILE",
    "INNER", "INOUT", "INSENSITIVE", "INSERT", "INT", "INT1", "INT2", "INT3",
    "INT4", "INT8", "INTEGER", "INTERSECT", "INTERVAL", "INTO",
    "IO_AFTER_GTIDS", "IO_BEFORE_GTIDS", "IS", "ITERATE", "JOIN",
    "JSON_TABLE", "KEY", "KEYS", "KILL", "LAG", "LAST_VALUE", "LATERAL",
    "LEAD", "LEADING", "LEAVE", "LEFT", "LIKE", "LIMIT", "LINEAR", "LINES",
    "LOAD", "LOCALTIME", "LOCALTIMESTAMP", "LOCK", "LONG", "LONGBLOB",
    "LONGTEXT", "LOOP", "LOW_PRIORITY", "MASTER_BIND",
    "MASTER_SSL_VERIFY_SERVER_CERT", "MATCH", "MAXVALUE", "MEDIUMBLOB",
    "MEDIUMINT", "MEDIUMTEXT", "MIDDLEINT", "MINUTE_MICROSECOND",
    "MINUTE_SECOND", "MOD", "MODIFIES", "NATURAL", "NOT",
    "NO_WRITE_TO_BINLOG", "NTH_VALUE", "NTILE", "NULL", "NUMERIC", "OF",
    "ON", "OPTIMIZE", "OPTIMIZER_COSTS", "OPTION", "OPTIONALLY", "OR",
    "ORDER", "OUT", "OUTER", "OUTFILE", "OVER", "PARALLEL", "PARTITION",
    "PERCENT_RANK", "PRECISION", "PRIMARY", "PROCEDURE", "PURGE", "RANGE",
    "RANK", "READ", "READS", "READ_WRITE", "REAL", "RECURSIVE", "REFERENCES",
    "REGEXP", "RELEASE", "RENAME", "REPEAT", "REPLACE", "REQUIRE", "RESIGNAL",
    "RESTRICT", "RETURN", "REVOKE", "RIGHT", "RLIKE", "ROW", "ROWS",
    "ROW_NUMBER", "SCHEMA", "SCHEMAS", "SECOND_MICROSECOND", "SELECT",
    "SENSITIVE", "SEPARATOR", "SET", "SHOW", "SIGNAL", "SMALLINT", "SPATIAL",
    "SPECIFIC", "SQL", "SQLEXCEPTION", "SQLSTATE", "SQLWARNING",
    "SQL_BIG_RESULT", "SQL_CALC_FOUND_ROWS", "SQL_SMALL_RESULT", "SSL",
    "STARTING", "STORED", "STRAIGHT_JOIN", "SYSTEM", "TABLE", "TERMINATED",
    "THEN", "TINYBLOB", "TINYINT", "TINYTEXT", "TO", "TRAILING", "TRIGGER",
    "TRUE", "UNDO", "UNION", "UNIQUE", "UNLOCK", "UNSIGNED", "UPDATE",
    "USAGE", "USE", "USING", "UTC_DATE", "UTC_TIME", "UTC_TIMESTAMP",
    "VALUES", "VARBINARY", "VARCHAR", "VARCHARACTER", "VARYING", "VECTOR",
    "VIRTUAL", "WHEN", "WHERE", "WHILE", "WINDOW", "WITH", "WRITE", "XOR",
    "YEAR_MONTH", "ZEROFILL",
];

/// Quote identifier if needed
pub fn quote_identifier(name: &str) -> String {
    if is_reserved_keyword(name) || name.contains(' ') || name.contains('-') {
        format!("`{}`", name.replace('`', "``"))
    } else {
        name.to_string()
    }
}
```

---

## 9. Data Types and Type Codes

### Field Types

```rust
/// MySQL field types (MYSQL_TYPE_*)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldType {
    Decimal     = 0,
    Tiny        = 1,
    Short       = 2,
    Long        = 3,
    Float       = 4,
    Double      = 5,
    Null        = 6,
    Timestamp   = 7,
    LongLong    = 8,
    Int24       = 9,
    Date        = 10,
    Time        = 11,
    DateTime    = 12,
    Year        = 13,
    NewDate     = 14,
    VarChar     = 15,
    Bit         = 16,
    Timestamp2  = 17,
    DateTime2   = 18,
    Time2       = 19,
    TypedArray  = 20,
    Invalid     = 243,
    Bool        = 244,
    Json        = 245,
    NewDecimal  = 246,
    Enum        = 247,
    Set         = 248,
    TinyBlob    = 249,
    MediumBlob  = 250,
    LongBlob    = 251,
    Blob        = 252,
    VarString   = 253,
    String      = 254,
    Geometry    = 255,
    
    // MySQL 9.0+ types
    Vector      = 242,
    
    Unknown     = 241,
}

impl TryFrom<u8> for FieldType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Decimal),
            1 => Ok(Self::Tiny),
            2 => Ok(Self::Short),
            3 => Ok(Self::Long),
            4 => Ok(Self::Float),
            5 => Ok(Self::Double),
            6 => Ok(Self::Null),
            7 => Ok(Self::Timestamp),
            8 => Ok(Self::LongLong),
            9 => Ok(Self::Int24),
            10 => Ok(Self::Date),
            11 => Ok(Self::Time),
            12 => Ok(Self::DateTime),
            13 => Ok(Self::Year),
            14 => Ok(Self::NewDate),
            15 => Ok(Self::VarChar),
            16 => Ok(Self::Bit),
            17 => Ok(Self::Timestamp2),
            18 => Ok(Self::DateTime2),
            19 => Ok(Self::Time2),
            20 => Ok(Self::TypedArray),
            242 => Ok(Self::Vector),
            243 => Ok(Self::Invalid),
            244 => Ok(Self::Bool),
            245 => Ok(Self::Json),
            246 => Ok(Self::NewDecimal),
            247 => Ok(Self::Enum),
            248 => Ok(Self::Set),
            249 => Ok(Self::TinyBlob),
            250 => Ok(Self::MediumBlob),
            251 => Ok(Self::LongBlob),
            252 => Ok(Self::Blob),
            253 => Ok(Self::VarString),
            254 => Ok(Self::String),
            255 => Ok(Self::Geometry),
            _ => Err(value),
        }
    }
}

impl FieldType {
    /// Get SQL type name
    pub fn sql_name(&self) -> &'static str {
        match self {
            Self::Decimal | Self::NewDecimal => "DECIMAL",
            Self::Tiny => "TINYINT",
            Self::Short => "SMALLINT",
            Self::Long => "INT",
            Self::Float => "FLOAT",
            Self::Double => "DOUBLE",
            Self::Null => "NULL",
            Self::Timestamp | Self::Timestamp2 => "TIMESTAMP",
            Self::LongLong => "BIGINT",
            Self::Int24 => "MEDIUMINT",
            Self::Date | Self::NewDate => "DATE",
            Self::Time | Self::Time2 => "TIME",
            Self::DateTime | Self::DateTime2 => "DATETIME",
            Self::Year => "YEAR",
            Self::VarChar | Self::VarString => "VARCHAR",
            Self::Bit => "BIT",
            Self::Bool => "BOOL",
            Self::Json => "JSON",
            Self::Enum => "ENUM",
            Self::Set => "SET",
            Self::TinyBlob => "TINYBLOB",
            Self::MediumBlob => "MEDIUMBLOB",
            Self::LongBlob => "LONGBLOB",
            Self::Blob => "BLOB",
            Self::String => "CHAR",
            Self::Geometry => "GEOMETRY",
            Self::Vector => "VECTOR",
            _ => "UNKNOWN",
        }
    }

    /// Check if type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Decimal | Self::NewDecimal | Self::Tiny | Self::Short |
            Self::Long | Self::Float | Self::Double | Self::LongLong |
            Self::Int24 | Self::Year | Self::Bit
        )
    }

    /// Check if type is string-like
    pub fn is_string(&self) -> bool {
        matches!(
            self,
            Self::VarChar | Self::VarString | Self::String |
            Self::TinyBlob | Self::Blob | Self::MediumBlob | Self::LongBlob |
            Self::Enum | Self::Set | Self::Json
        )
    }

    /// Check if type is temporal
    pub fn is_temporal(&self) -> bool {
        matches!(
            self,
            Self::Date | Self::NewDate | Self::Time | Self::Time2 |
            Self::DateTime | Self::DateTime2 | Self::Timestamp |
            Self::Timestamp2 | Self::Year
        )
    }
}
```

### Character Sets

```rust
/// MySQL character set constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Charset {
    Big5      = 1,
    Latin2    = 2,
    Dec8      = 3,
    Cp850     = 4,
    Latin1    = 8,
    Hp8       = 6,
    Koi8r     = 7,
    Swe7      = 10,
    Ascii     = 11,
    Ujis      = 12,
    Sjis      = 13,
    Cp1251    = 14,
    Hebrew    = 16,
    Tis620    = 18,
    Euckr     = 19,
    Latin7    = 20,
    Koi8u     = 22,
    Gb2312    = 24,
    Greek     = 25,
    Cp1250    = 26,
    Gbk       = 28,
    Cp1257    = 29,
    Latin5    = 30,
    Armscii8  = 32,
    Utf8Mb3   = 33,   // Deprecated
    Ucs2      = 35,
    Cp866     = 36,
    Keybcs2   = 37,
    Macce     = 38,
    Macroman  = 39,
    Cp852     = 40,
    Cp1256    = 57,
    Cp932     = 95,
    Eucjpms   = 97,
    Gb18030   = 248,
    Utf8Mb4   = 255,  // Recommended
    Binary    = 63,
}

impl Default for Charset {
    fn default() -> Self {
        Self::Utf8Mb4
    }
}
```

---

## 10. Parser Architecture

### SQL Parser using sqlparser-rs

```rust
use sqlparser::ast::{self, Statement};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;

/// Parse MySQL SQL statements
pub fn parse_sql(sql: &str) -> Result<Vec<Statement>, sqlparser::parser::ParserError> {
    let dialect = MySqlDialect {};
    Parser::parse_sql(&dialect, sql)
}

/// Extract table names from a statement
pub fn extract_tables(stmt: &Statement) -> Vec<String> {
    let mut tables = Vec::new();

    match stmt {
        Statement::Query(query) => {
            extract_tables_from_query(query, &mut tables);
        }
        Statement::Insert(insert) => {
            tables.push(insert.table_name.to_string());
        }
        Statement::Update { table, .. } => {
            if let ast::TableWithJoins { relation, .. } = table {
                if let ast::TableFactor::Table { name, .. } = relation {
                    tables.push(name.to_string());
                }
            }
        }
        Statement::Delete(delete) => {
            for table in &delete.tables {
                tables.push(table.to_string());
            }
            if let Some(from) = &delete.from {
                for twj in from {
                    if let ast::TableFactor::Table { name, .. } = &twj.relation {
                        tables.push(name.to_string());
                    }
                }
            }
        }
        _ => {}
    }

    tables
}

fn extract_tables_from_query(query: &ast::Query, tables: &mut Vec<String>) {
    if let ast::SetExpr::Select(select) = query.body.as_ref() {
        for twj in &select.from {
            extract_tables_from_table_factor(&twj.relation, tables);
            for join in &twj.joins {
                extract_tables_from_table_factor(&join.relation, tables);
            }
        }
    }
}

fn extract_tables_from_table_factor(factor: &ast::TableFactor, tables: &mut Vec<String>) {
    match factor {
        ast::TableFactor::Table { name, .. } => {
            tables.push(name.to_string());
        }
        ast::TableFactor::Derived { subquery, .. } => {
            extract_tables_from_query(subquery, tables);
        }
        ast::TableFactor::NestedJoin { table_with_joins, .. } => {
            extract_tables_from_table_factor(&table_with_joins.relation, tables);
            for join in &table_with_joins.joins {
                extract_tables_from_table_factor(&join.relation, tables);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_select() {
        let sql = "SELECT id, name FROM users WHERE id = 1";
        let stmts = parse_sql(sql).unwrap();
        assert_eq!(stmts.len(), 1);
        
        let tables = extract_tables(&stmts[0]);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_parse_join() {
        let sql = "SELECT u.name, o.total FROM users u JOIN orders o ON u.id = o.user_id";
        let stmts = parse_sql(sql).unwrap();
        
        let tables = extract_tables(&stmts[0]);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }
}
```

---

## 11. AST Definitions

### Statement Types

```rust
/// MySQL statement types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementType {
    // DML
    Select,
    Insert,
    Update,
    Delete,
    Replace,
    Merge,

    // DDL
    CreateTable,
    CreateIndex,
    CreateDatabase,
    CreateView,
    CreateFunction,
    CreateProcedure,
    CreateTrigger,
    CreateEvent,
    AlterTable,
    AlterDatabase,
    AlterView,
    AlterEvent,
    DropTable,
    DropIndex,
    DropDatabase,
    DropView,
    DropFunction,
    DropProcedure,
    DropTrigger,
    DropEvent,
    Truncate,
    Rename,

    // DCL
    Grant,
    Revoke,
    CreateUser,
    AlterUser,
    DropUser,
    RenameUser,
    SetPassword,

    // TCL
    StartTransaction,
    Commit,
    Rollback,
    Savepoint,
    ReleaseSavepoint,
    RollbackToSavepoint,
    SetTransaction,

    // Utility
    Use,
    Show,
    Describe,
    Explain,
    Set,
    Lock,
    Unlock,
    Prepare,
    Execute,
    Deallocate,
    Call,
    
    // Admin
    Analyze,
    Optimize,
    Repair,
    Check,
    Checksum,
    Flush,
    Kill,
    Load,
    
    Unknown,
}

impl StatementType {
    /// Determine statement type from SQL
    pub fn from_sql(sql: &str) -> Self {
        let sql_upper = sql.trim().to_uppercase();
        let first_word = sql_upper.split_whitespace().next().unwrap_or("");

        match first_word {
            "SELECT" => Self::Select,
            "INSERT" => Self::Insert,
            "UPDATE" => Self::Update,
            "DELETE" => Self::Delete,
            "REPLACE" => Self::Replace,
            "MERGE" => Self::Merge,
            "CREATE" => {
                let second = sql_upper.split_whitespace().nth(1).unwrap_or("");
                match second {
                    "TABLE" => Self::CreateTable,
                    "INDEX" | "UNIQUE" | "FULLTEXT" | "SPATIAL" => Self::CreateIndex,
                    "DATABASE" | "SCHEMA" => Self::CreateDatabase,
                    "VIEW" => Self::CreateView,
                    "FUNCTION" => Self::CreateFunction,
                    "PROCEDURE" => Self::CreateProcedure,
                    "TRIGGER" => Self::CreateTrigger,
                    "EVENT" => Self::CreateEvent,
                    "USER" => Self::CreateUser,
                    _ => Self::Unknown,
                }
            }
            "ALTER" => {
                let second = sql_upper.split_whitespace().nth(1).unwrap_or("");
                match second {
                    "TABLE" => Self::AlterTable,
                    "DATABASE" | "SCHEMA" => Self::AlterDatabase,
                    "VIEW" => Self::AlterView,
                    "EVENT" => Self::AlterEvent,
                    "USER" => Self::AlterUser,
                    _ => Self::Unknown,
                }
            }
            "DROP" => {
                let second = sql_upper.split_whitespace().nth(1).unwrap_or("");
                match second {
                    "TABLE" => Self::DropTable,
                    "INDEX" => Self::DropIndex,
                    "DATABASE" | "SCHEMA" => Self::DropDatabase,
                    "VIEW" => Self::DropView,
                    "FUNCTION" => Self::DropFunction,
                    "PROCEDURE" => Self::DropProcedure,
                    "TRIGGER" => Self::DropTrigger,
                    "EVENT" => Self::DropEvent,
                    "USER" => Self::DropUser,
                    _ => Self::Unknown,
                }
            }
            "TRUNCATE" => Self::Truncate,
            "RENAME" => Self::Rename,
            "GRANT" => Self::Grant,
            "REVOKE" => Self::Revoke,
            "START" | "BEGIN" => Self::StartTransaction,
            "COMMIT" => Self::Commit,
            "ROLLBACK" => Self::Rollback,
            "SAVEPOINT" => Self::Savepoint,
            "RELEASE" => Self::ReleaseSavepoint,
            "USE" => Self::Use,
            "SHOW" => Self::Show,
            "DESCRIBE" | "DESC" | "EXPLAIN" => Self::Explain,
            "SET" => Self::Set,
            "LOCK" => Self::Lock,
            "UNLOCK" => Self::Unlock,
            "PREPARE" => Self::Prepare,
            "EXECUTE" => Self::Execute,
            "DEALLOCATE" => Self::Deallocate,
            "CALL" => Self::Call,
            "ANALYZE" => Self::Analyze,
            "OPTIMIZE" => Self::Optimize,
            "REPAIR" => Self::Repair,
            "CHECK" => Self::Check,
            "CHECKSUM" => Self::Checksum,
            "FLUSH" => Self::Flush,
            "KILL" => Self::Kill,
            "LOAD" => Self::Load,
            _ => Self::Unknown,
        }
    }

    /// Check if statement modifies data
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Insert | Self::Update | Self::Delete | Self::Replace |
            Self::Merge | Self::CreateTable | Self::CreateIndex |
            Self::CreateDatabase | Self::AlterTable | Self::DropTable |
            Self::DropIndex | Self::DropDatabase | Self::Truncate |
            Self::Grant | Self::Revoke | Self::CreateUser | Self::DropUser |
            Self::Load
        )
    }

    /// Check if statement is transactional
    pub fn is_transactional(&self) -> bool {
        matches!(
            self,
            Self::StartTransaction | Self::Commit | Self::Rollback |
            Self::Savepoint | Self::ReleaseSavepoint | Self::RollbackToSavepoint |
            Self::SetTransaction
        )
    }
}
```

---

## 12. Error Handling

### Error Types

```rust
use thiserror::Error;

/// Protocol-level errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported protocol version: {0}")]
    UnsupportedProtocolVersion(u8),

    #[error("Malformed packet")]
    MalformedPacket,

    #[error("Unexpected packet type")]
    UnexpectedPacketType,

    #[error("Packet too large: {0} bytes")]
    PacketTooLarge(usize),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("MySQL error {code}: {message}")]
    Server {
        code: u16,
        sql_state: String,
        message: String,
    },
}

impl From<ErrPacket> for ProtocolError {
    fn from(err: ErrPacket) -> Self {
        Self::Server {
            code: err.error_code,
            sql_state: err.sql_state,
            message: err.error_message,
        }
    }
}

/// Common MySQL error codes
pub mod error_codes {
    pub const ER_DUP_KEY: u16 = 1022;
    pub const ER_ACCESS_DENIED_ERROR: u16 = 1045;
    pub const ER_NO_SUCH_TABLE: u16 = 1146;
    pub const ER_TABLE_EXISTS_ERROR: u16 = 1050;
    pub const ER_BAD_FIELD_ERROR: u16 = 1054;
    pub const ER_DUP_ENTRY: u16 = 1062;
    pub const ER_PARSE_ERROR: u16 = 1064;
    pub const ER_EMPTY_QUERY: u16 = 1065;
    pub const ER_LOCK_WAIT_TIMEOUT: u16 = 1205;
    pub const ER_LOCK_DEADLOCK: u16 = 1213;
    pub const ER_QUERY_INTERRUPTED: u16 = 1317;
    pub const ER_FOREIGN_KEY_CONSTRAINT: u16 = 1452;
    pub const ER_DATA_TOO_LONG: u16 = 1406;
    pub const ER_TRUNCATED_WRONG_VALUE: u16 = 1292;
}
```

---

## 13. Authentication

### Authentication Plugins

```rust
use sha1::{Sha1, Digest as Sha1Digest};
use sha2::{Sha256, Digest as Sha256Digest};

/// Authentication plugin names
pub mod auth_plugins {
    pub const NATIVE_PASSWORD: &str = "mysql_native_password";
    pub const CACHING_SHA2: &str = "caching_sha2_password";
    pub const SHA256_PASSWORD: &str = "sha256_password";
    pub const CLEAR_PASSWORD: &str = "mysql_clear_password";
}

/// Native password authentication (SHA1)
/// Deprecated but still supported
pub fn native_password_auth(password: &str, scramble: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return vec![];
    }

    // SHA1(password)
    let stage1 = Sha1::digest(password.as_bytes());
    
    // SHA1(SHA1(password))
    let stage2 = Sha1::digest(&stage1);
    
    // SHA1(scramble + SHA1(SHA1(password)))
    let mut hasher = Sha1::new();
    hasher.update(scramble);
    hasher.update(&stage2);
    let hash = hasher.finalize();
    
    // XOR with SHA1(password)
    stage1.iter()
        .zip(hash.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}

/// caching_sha2_password authentication
pub fn caching_sha2_auth(password: &str, nonce: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return vec![];
    }

    // SHA256(password)
    let digest1 = Sha256::digest(password.as_bytes());
    
    // SHA256(SHA256(password))
    let digest2 = Sha256::digest(&digest1);
    
    // SHA256(SHA256(SHA256(password)) + nonce)
    let mut hasher = Sha256::new();
    hasher.update(&digest2);
    hasher.update(nonce);
    let scramble = hasher.finalize();
    
    // XOR with SHA256(password)
    digest1.iter()
        .zip(scramble.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}

/// Perform authentication based on plugin name
pub fn authenticate(plugin: &str, password: &str, scramble: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    match plugin {
        auth_plugins::NATIVE_PASSWORD => Ok(native_password_auth(password, scramble)),
        auth_plugins::CACHING_SHA2 => Ok(caching_sha2_auth(password, scramble)),
        auth_plugins::SHA256_PASSWORD => Ok(caching_sha2_auth(password, scramble)),
        auth_plugins::CLEAR_PASSWORD => Ok(password.as_bytes().to_vec()),
        _ => Err(ProtocolError::AuthenticationFailed(
            format!("Unsupported auth plugin: {}", plugin)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_password() {
        let scramble = [1u8; 20];
        let result = native_password_auth("password", &scramble);
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn test_empty_password() {
        let result = native_password_auth("", &[1u8; 20]);
        assert!(result.is_empty());
    }
}
```

---

## 14. MySQL 9.5.0 New Features

### VECTOR Data Type

```rust
/// MySQL VECTOR type support (added in MySQL 9.0)
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    pub values: Vec<f32>,
}

impl Vector {
    /// Maximum vector dimension
    pub const MAX_DIMENSION: usize = 16383;
    
    /// Default dimension
    pub const DEFAULT_DIMENSION: usize = 2048;

    pub fn new(values: Vec<f32>) -> Result<Self, String> {
        if values.len() > Self::MAX_DIMENSION {
            return Err(format!(
                "Vector dimension {} exceeds maximum {}",
                values.len(),
                Self::MAX_DIMENSION
            ));
        }
        Ok(Self { values })
    }

    /// Parse from MySQL string format '[0.1, 0.2, 0.3]'
    pub fn from_mysql_string(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err("Vector must be enclosed in brackets".to_string());
        }

        let inner = &s[1..s.len() - 1];
        let values: Result<Vec<f32>, _> = inner
            .split(',')
            .map(|v| v.trim().parse::<f32>())
            .collect();

        match values {
            Ok(v) => Self::new(v),
            Err(e) => Err(format!("Failed to parse vector values: {}", e)),
        }
    }

    /// Convert to MySQL string format
    pub fn to_mysql_string(&self) -> String {
        let values_str: Vec<String> = self.values.iter().map(|v| v.to_string()).collect();
        format!("[{}]", values_str.join(", "))
    }

    /// Compute Euclidean distance
    pub fn euclidean_distance(&self, other: &Vector) -> f32 {
        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Compute cosine similarity
    pub fn cosine_similarity(&self, other: &Vector) -> f32 {
        let dot: f32 = self.values.iter().zip(other.values.iter()).map(|(a, b)| a * b).sum();
        let norm_a: f32 = self.values.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
        let norm_b: f32 = other.values.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// Compute dot product
    pub fn dot_product(&self, other: &Vector) -> f32 {
        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum()
    }
}

#[cfg(test)]
mod vector_tests {
    use super::*;

    #[test]
    fn test_vector_parsing() {
        let v = Vector::from_mysql_string("[1.0, 2.0, 3.0]").unwrap();
        assert_eq!(v.values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = Vector::new(vec![1.0, 0.0]).unwrap();
        let v2 = Vector::new(vec![1.0, 0.0]).unwrap();
        assert!((v1.cosine_similarity(&v2) - 1.0).abs() < 0.0001);
    }
}
```

### JSON Duality View Support

```sql
-- Example JSON Duality View (MySQL 9.4+)
-- Can be interacted with like a table but returns JSON documents

/*
CREATE JSON DUALITY VIEW customer_orders_view AS
SELECT JSON_OBJECT(
    'customerId' VALUE c.id,
    'name' VALUE c.name,
    'email' VALUE c.email,
    'orders' VALUE (
        SELECT JSON_ARRAYAGG(
            JSON_OBJECT(
                'orderId' VALUE o.id,
                'date' VALUE o.order_date,
                'total' VALUE o.total,
                'items' VALUE (
                    SELECT JSON_ARRAYAGG(
                        JSON_OBJECT(
                            'product' VALUE p.name,
                            'quantity' VALUE oi.quantity,
                            'price' VALUE oi.price
                        )
                    )
                    FROM order_items oi
                    JOIN products p ON oi.product_id = p.id
                    WHERE oi.order_id = o.id
                )
            )
        )
        FROM orders o
        WHERE o.customer_id = c.id
    )
)
FROM customers c;
*/
```

---

## 15. Implementation Libraries

### Recommended Crates

```toml
[dependencies]
# High-level async MySQL client
mysql_async = "0.34"

# Synchronous MySQL client
mysql = "25"

# SQL parsing
sqlparser = "0.41"

# Connection pooling
deadpool = { version = "0.10", features = ["rt_tokio_1"] }
bb8 = "0.8"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Cryptography for auth
sha1 = "0.10"
sha2 = "0.10"

# Binary protocol
byteorder = "1.5"
bytes = "1.5"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## 16. Complete Connection Example

```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Simple MySQL connection example
pub struct MysqlConnection {
    stream: TcpStream,
    sequence_id: u8,
    capabilities: CapabilityFlags,
}

impl MysqlConnection {
    /// Connect to MySQL server
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        database: Option<&str>,
    ) -> Result<Self, ProtocolError> {
        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr).await?;

        // Read server greeting
        let mut header_buf = [0u8; 4];
        stream.read_exact(&mut header_buf).await?;
        let header = PacketHeader::from_bytes(&header_buf)?;

        let mut payload = vec![0u8; header.payload_length as usize];
        stream.read_exact(&mut payload).await?;

        let handshake = HandshakeV10::parse(&payload)?;
        let sequence_id = header.sequence_id + 1;

        // Build client capabilities
        let mut capabilities = CapabilityFlags::default();
        if database.is_some() {
            capabilities |= CapabilityFlags::CONNECT_WITH_DB;
        }

        // Negotiate capabilities with server
        capabilities &= CapabilityFlags::from_bits_truncate(handshake.capability_flags);

        // Create authentication response
        let scramble = handshake.auth_scramble();
        let auth_response = authenticate(&handshake.auth_plugin_name, password, &scramble)?;

        let response = HandshakeResponse41 {
            capability_flags: capabilities,
            max_packet_size: MAX_PACKET_LENGTH as u32,
            character_set: Charset::Utf8Mb4 as u8,
            username: username.to_string(),
            auth_response,
            database: database.map(|s| s.to_string()),
            auth_plugin_name: handshake.auth_plugin_name.clone(),
            connect_attrs: None,
        };

        // Send handshake response
        let response_payload = response.build();
        let packet = Packet::new(response_payload, sequence_id);
        stream.write_all(&packet.to_bytes()).await?;

        // Read server response
        let mut header_buf = [0u8; 4];
        stream.read_exact(&mut header_buf).await?;
        let header = PacketHeader::from_bytes(&header_buf)?;

        let mut payload = vec![0u8; header.payload_length as usize];
        stream.read_exact(&mut payload).await?;

        match payload[0] {
            PACKET_OK => {
                Ok(Self {
                    stream,
                    sequence_id: 0,
                    capabilities,
                })
            }
            PACKET_ERR => {
                let err = ErrPacket::parse(&payload)?;
                Err(err.into())
            }
            _ => {
                // Handle auth switch or other responses
                Err(ProtocolError::AuthenticationFailed(
                    "Unexpected authentication response".to_string()
                ))
            }
        }
    }

    /// Execute a simple query
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, ProtocolError> {
        self.sequence_id = 0;

        // Send query command
        let payload = Command::query(sql);
        let packet = Packet::new(payload, self.sequence_id);
        self.stream.write_all(&packet.to_bytes()).await?;

        // Read response
        let mut header_buf = [0u8; 4];
        self.stream.read_exact(&mut header_buf).await?;
        let header = PacketHeader::from_bytes(&header_buf)?;

        let mut payload = vec![0u8; header.payload_length as usize];
        self.stream.read_exact(&mut payload).await?;

        match packet_type(payload[0], payload.len()) {
            PacketType::Ok => {
                let ok = OkPacket::parse(&payload)?;
                Ok(QueryResult::Ok(ok))
            }
            PacketType::Err => {
                let err = ErrPacket::parse(&payload)?;
                Err(err.into())
            }
            PacketType::ResultSet => {
                // Parse column count
                let col_count = read_lenenc_int(&payload)
                    .ok_or(ProtocolError::MalformedPacket)?;
                
                // Read column definitions
                let mut columns = Vec::with_capacity(col_count.value as usize);
                for _ in 0..col_count.value {
                    let (header, payload) = self.read_packet().await?;
                    let col = ColumnDefinition::parse(&payload)?;
                    columns.push(col);
                }

                // Read EOF or OK (with DEPRECATE_EOF)
                if !self.capabilities.contains(CapabilityFlags::DEPRECATE_EOF) {
                    let _ = self.read_packet().await?;
                }

                // Read rows
                let mut rows = Vec::new();
                loop {
                    let (_, payload) = self.read_packet().await?;
                    
                    if payload[0] == PACKET_EOF && payload.len() < 9 {
                        break;
                    }
                    if payload[0] == PACKET_OK && self.capabilities.contains(CapabilityFlags::DEPRECATE_EOF) {
                        break;
                    }
                    if payload[0] == PACKET_ERR {
                        let err = ErrPacket::parse(&payload)?;
                        return Err(err.into());
                    }

                    let row = TextRow::parse(&payload, columns.len())?;
                    rows.push(row);
                }

                Ok(QueryResult::ResultSet { columns, rows })
            }
            _ => Err(ProtocolError::UnexpectedPacketType),
        }
    }

    /// Read a single packet from the stream
    async fn read_packet(&mut self) -> Result<(PacketHeader, Vec<u8>), ProtocolError> {
        let mut header_buf = [0u8; 4];
        self.stream.read_exact(&mut header_buf).await?;
        let header = PacketHeader::from_bytes(&header_buf)?;

        let mut payload = vec![0u8; header.payload_length as usize];
        self.stream.read_exact(&mut payload).await?;

        Ok((header, payload))
    }

    /// Send ping to server
    pub async fn ping(&mut self) -> Result<(), ProtocolError> {
        self.sequence_id = 0;

        let packet = Packet::new(Command::ping(), self.sequence_id);
        self.stream.write_all(&packet.to_bytes()).await?;

        let (_, payload) = self.read_packet().await?;
        
        if payload[0] == PACKET_OK {
            Ok(())
        } else {
            let err = ErrPacket::parse(&payload)?;
            Err(err.into())
        }
    }

    /// Close connection gracefully
    pub async fn close(mut self) -> Result<(), ProtocolError> {
        let packet = Packet::new(Command::quit(), 0);
        self.stream.write_all(&packet.to_bytes()).await?;
        Ok(())
    }
}

/// Query result types
#[derive(Debug)]
pub enum QueryResult {
    Ok(OkPacket),
    ResultSet {
        columns: Vec<ColumnDefinition>,
        rows: Vec<TextRow>,
    },
}

// Usage example:
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let mut conn = MysqlConnection::connect(
//         "127.0.0.1", 3306, "root", "password", Some("test")
//     ).await?;
//
//     let result = conn.query("SELECT id, name FROM users LIMIT 10").await?;
//     
//     match result {
//         QueryResult::ResultSet { columns, rows } => {
//             for col in &columns {
//                 print!("{}\t", col.name);
//             }
//             println!();
//             
//             for row in &rows {
//                 for i in 0..columns.len() {
//                     print!("{}\t", row.get_string(i).unwrap_or_default());
//                 }
//                 println!();
//             }
//         }
//         QueryResult::Ok(ok) => {
//             println!("Affected rows: {}", ok.affected_rows);
//         }
//     }
//
//     conn.close().await?;
//     Ok(())
// }
```

---

## Appendix: Quick Reference

### Packet Format Reference

```text
[3 bytes LE: length] [1 byte: seq_id] [payload]
```

### Length-Encoded Integer

```text
0x00-0xFA: 1-byte value
0xFC: 2-byte follows (LE)
0xFD: 3-byte follows (LE)
0xFE: 8-byte follows (LE)
0xFB: NULL (in rows only)
```

### Common Commands

```text
0x01: COM_QUIT
0x02: COM_INIT_DB
0x03: COM_QUERY
0x0E: COM_PING
0x16: COM_STMT_PREPARE
0x17: COM_STMT_EXECUTE
0x19: COM_STMT_CLOSE
0x1F: COM_RESET_CONNECTION
```

### Response Types (first byte)

```text
0x00: OK packet
0xFB: LOCAL INFILE request
0xFE: EOF packet (if len < 9)
0xFF: ERR packet
else: Result set (column count)
```

### Default Settings

- Port: 3306
- X Protocol Port: 33060
- Character Set: utf8mb4 (255)
- Max Packet Size: 16MB - 1
- Byte Order: Little-endian

---

*Document Version: 1.0*  
*MySQL Version: 9.5.0*  
*Last Updated: December 2025*
