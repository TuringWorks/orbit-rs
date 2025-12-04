# MySQL 9.5.0 & MariaDB 12.2.1 Protocol and SQL Reference (Rust Version)

**Version:** 1.0  
**Target Audience:** LLM coding tools, database driver developers, protocol implementers  
**Languages:** Rust  

---

## 1. Overview

### MySQL 9.5.0

- **Release Date:** October 21, 2025
- **Type:** Innovation Release
- **Default Port:** 3306
- **Protocol Version:** MySQL Protocol (text and binary)
- **X Protocol Port:** 33060 (optional)

### MariaDB 12.2.1

- **Release Date:** November 21, 2025
- **Type:** Release Candidate (RC) Rolling Release
- **Default Port:** 3306
- **Protocol:** MySQL-compatible with extensions

### Rust Crates

```toml
[dependencies]
# Core dependencies
byteorder = "1.5"
bytes = "1.5"
tokio = { version = "1.35", features = ["full"] }

# Cryptography
sha1 = "0.10"
sha2 = "0.10"
md5 = "0.7"

# Optional: ed25519 for MariaDB auth
ed25519-dalek = "2.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "1.0"
```

---

## 2. Wire Protocol Specification

### 2.1 Packet Format

```rust
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write, Cursor};

/// Maximum MySQL packet payload size (16MB - 1)
pub const MAX_PACKET_SIZE: u32 = 0xFF_FF_FF;

/// MySQL packet header (4 bytes)
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub payload_length: u32,  // 3 bytes, little-endian
    pub sequence_id: u8,
}

impl PacketHeader {
    pub const SIZE: usize = 4;

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        
        let payload_length = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]);
        let sequence_id = buf[3];
        
        Ok(Self { payload_length, sequence_id })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let len_bytes = self.payload_length.to_le_bytes();
        writer.write_all(&[len_bytes[0], len_bytes[1], len_bytes[2], self.sequence_id])
    }
}

/// MySQL packet with header and payload
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(sequence_id: u8, payload: Vec<u8>) -> Self {
        Self {
            header: PacketHeader {
                payload_length: payload.len() as u32,
                sequence_id,
            },
            payload,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let header = PacketHeader::read(reader)?;
        let mut payload = vec![0u8; header.payload_length as usize];
        reader.read_exact(&mut payload)?;
        Ok(Self { header, payload })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        self.header.write(writer)?;
        writer.write_all(&self.payload)
    }
}
```

### 2.2 Data Type Encoding

```rust
/// MySQL protocol data types
pub mod types {
    use std::io::{self, Read, Write};
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

    /// Length-encoded integer reader/writer
    pub struct LenEnc;

    impl LenEnc {
        /// Read a length-encoded integer
        pub fn read_int<R: Read>(reader: &mut R) -> io::Result<u64> {
            let first = reader.read_u8()?;
            match first {
                0..=0xFA => Ok(first as u64),
                0xFC => Ok(reader.read_u16::<LittleEndian>()? as u64),
                0xFD => {
                    let mut buf = [0u8; 3];
                    reader.read_exact(&mut buf)?;
                    Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]) as u64)
                }
                0xFE => reader.read_u64::<LittleEndian>(),
                0xFB => Ok(u64::MAX), // NULL indicator
                0xFF => Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid lenenc prefix 0xFF")),
            }
        }

        /// Write a length-encoded integer
        pub fn write_int<W: Write>(writer: &mut W, value: u64) -> io::Result<()> {
            if value < 251 {
                writer.write_u8(value as u8)
            } else if value < 0x1_0000 {
                writer.write_u8(0xFC)?;
                writer.write_u16::<LittleEndian>(value as u16)
            } else if value < 0x100_0000 {
                writer.write_u8(0xFD)?;
                let bytes = (value as u32).to_le_bytes();
                writer.write_all(&bytes[..3])
            } else {
                writer.write_u8(0xFE)?;
                writer.write_u64::<LittleEndian>(value)
            }
        }

        /// Read a length-encoded string
        pub fn read_string<R: Read>(reader: &mut R) -> io::Result<String> {
            let len = Self::read_int(reader)? as usize;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            String::from_utf8(buf)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }

        /// Read a length-encoded byte slice
        pub fn read_bytes<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
            let len = Self::read_int(reader)? as usize;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Ok(buf)
        }

        /// Write a length-encoded string
        pub fn write_string<W: Write>(writer: &mut W, s: &str) -> io::Result<()> {
            Self::write_int(writer, s.len() as u64)?;
            writer.write_all(s.as_bytes())
        }
    }

    /// Read a NUL-terminated string
    pub fn read_nul_string<R: Read>(reader: &mut R) -> io::Result<String> {
        let mut buf = Vec::new();
        loop {
            let byte = reader.read_u8()?;
            if byte == 0 {
                break;
            }
            buf.push(byte);
        }
        String::from_utf8(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Read a fixed-length string
    pub fn read_fixed_string<R: Read>(reader: &mut R, len: usize) -> io::Result<String> {
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        // Trim trailing NULs
        while buf.last() == Some(&0) {
            buf.pop();
        }
        String::from_utf8(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Read rest of packet as string
    pub fn read_eof_string(data: &[u8], offset: usize) -> io::Result<String> {
        String::from_utf8(data[offset..].to_vec())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
```

---

## 3. Capability Flags

```rust
use bitflags::bitflags;

bitflags! {
    /// MySQL/MariaDB client capability flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilityFlags: u32 {
        const LONG_PASSWORD                  = 0x0000_0001;
        const FOUND_ROWS                     = 0x0000_0002;
        const LONG_FLAG                      = 0x0000_0004;
        const CONNECT_WITH_DB                = 0x0000_0008;
        const NO_SCHEMA                      = 0x0000_0010;
        const COMPRESS                       = 0x0000_0020;
        const ODBC                           = 0x0000_0040;
        const LOCAL_FILES                    = 0x0000_0080;
        const IGNORE_SPACE                   = 0x0000_0100;
        const PROTOCOL_41                    = 0x0000_0200;
        const INTERACTIVE                    = 0x0000_0400;
        const SSL                            = 0x0000_0800;
        const IGNORE_SIGPIPE                 = 0x0000_1000;
        const TRANSACTIONS                   = 0x0000_2000;
        const RESERVED                       = 0x0000_4000;
        const SECURE_CONNECTION              = 0x0000_8000;
        const MULTI_STATEMENTS               = 0x0001_0000;
        const MULTI_RESULTS                  = 0x0002_0000;
        const PS_MULTI_RESULTS               = 0x0004_0000;
        const PLUGIN_AUTH                    = 0x0008_0000;
        const CONNECT_ATTRS                  = 0x0010_0000;
        const PLUGIN_AUTH_LENENC_CLIENT_DATA = 0x0020_0000;
        const CAN_HANDLE_EXPIRED_PASSWORDS   = 0x0040_0000;
        const SESSION_TRACK                  = 0x0080_0000;
        const DEPRECATE_EOF                  = 0x0100_0000;
        const OPTIONAL_RESULTSET_METADATA    = 0x0200_0000;
        const ZSTD_COMPRESSION_ALGORITHM     = 0x0400_0000;
        const QUERY_ATTRIBUTES               = 0x0800_0000;
        const MULTI_FACTOR_AUTHENTICATION    = 0x1000_0000;
        const CAPABILITY_EXTENSION           = 0x2000_0000;
        const SSL_VERIFY_SERVER_CERT         = 0x4000_0000;
        const REMEMBER_OPTIONS               = 0x8000_0000;
    }
}

bitflags! {
    /// MariaDB extended capability flags (stored in reserved bytes)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MariaDbCapabilities: u32 {
        const PROGRESS                   = 0x0000_0001;
        const COM_MULTI                  = 0x0000_0002;
        const STMT_BULK_OPERATIONS       = 0x0000_0004;
        const EXTENDED_METADATA          = 0x0000_0008;
        const CACHE_METADATA             = 0x0000_0010;
        const BULK_UNIT_RESULTS          = 0x0000_0020;
    }
}

bitflags! {
    /// Server status flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StatusFlags: u16 {
        const IN_TRANS               = 0x0001;
        const AUTOCOMMIT             = 0x0002;
        const MORE_RESULTS_EXISTS    = 0x0008;
        const NO_GOOD_INDEX_USED     = 0x0010;
        const NO_INDEX_USED          = 0x0020;
        const CURSOR_EXISTS          = 0x0040;
        const LAST_ROW_SENT          = 0x0080;
        const DB_DROPPED             = 0x0100;
        const NO_BACKSLASH_ESCAPES   = 0x0200;
        const METADATA_CHANGED       = 0x0400;
        const QUERY_WAS_SLOW         = 0x0800;
        const PS_OUT_PARAMS          = 0x1000;
        const IN_TRANS_READONLY      = 0x2000;
        const SESSION_STATE_CHANGED  = 0x4000;
    }
}
```

---

## 4. Command Types

```rust
/// MySQL protocol commands
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    
    // MariaDB-specific
    StmtBulkExecute     = 0xFA,
    Multi               = 0xFE,
}

impl From<u8> for Command {
    fn from(byte: u8) -> Self {
        match byte {
            0x00 => Command::Sleep,
            0x01 => Command::Quit,
            0x02 => Command::InitDb,
            0x03 => Command::Query,
            0x16 => Command::StmtPrepare,
            0x17 => Command::StmtExecute,
            0x18 => Command::StmtSendLongData,
            0x19 => Command::StmtClose,
            0x1A => Command::StmtReset,
            0x1C => Command::StmtFetch,
            0x1F => Command::ResetConnection,
            0xFA => Command::StmtBulkExecute,
            _ => Command::Sleep, // Default fallback
        }
    }
}
```

---

## 5. Field Types

```rust
/// MySQL field/column types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Decimal     = 0x00,
    Tiny        = 0x01,
    Short       = 0x02,
    Long        = 0x03,
    Float       = 0x04,
    Double      = 0x05,
    Null        = 0x06,
    Timestamp   = 0x07,
    LongLong    = 0x08,
    Int24       = 0x09,
    Date        = 0x0A,
    Time        = 0x0B,
    DateTime    = 0x0C,
    Year        = 0x0D,
    NewDate     = 0x0E,
    VarChar     = 0x0F,
    Bit         = 0x10,
    Timestamp2  = 0x11,
    DateTime2   = 0x12,
    Time2       = 0x13,
    TypedArray  = 0x14,  // MySQL 8.0.17+
    Vector      = 0xF2,  // MySQL 9.0+
    Invalid     = 0xF3,
    Bool        = 0xF4,  // MySQL 8.0.17+
    Json        = 0xF5,
    NewDecimal  = 0xF6,
    Enum        = 0xF7,
    Set         = 0xF8,
    TinyBlob    = 0xF9,
    MediumBlob  = 0xFA,
    LongBlob    = 0xFB,
    Blob        = 0xFC,
    VarString   = 0xFD,
    String      = 0xFE,
    Geometry    = 0xFF,
}

impl FieldType {
    /// Check if this type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(self,
            FieldType::Tiny | FieldType::Short | FieldType::Long |
            FieldType::Float | FieldType::Double | FieldType::LongLong |
            FieldType::Int24 | FieldType::Decimal | FieldType::NewDecimal
        )
    }

    /// Check if this type is a string type
    pub fn is_string(&self) -> bool {
        matches!(self,
            FieldType::VarChar | FieldType::VarString | FieldType::String |
            FieldType::TinyBlob | FieldType::Blob | FieldType::MediumBlob |
            FieldType::LongBlob | FieldType::Enum | FieldType::Set
        )
    }

    /// Check if this type is a temporal type
    pub fn is_temporal(&self) -> bool {
        matches!(self,
            FieldType::Date | FieldType::Time | FieldType::DateTime |
            FieldType::Timestamp | FieldType::Year |
            FieldType::Timestamp2 | FieldType::DateTime2 | FieldType::Time2
        )
    }

    /// Get the size in bytes for fixed-size types
    pub fn size(&self) -> Option<usize> {
        match self {
            FieldType::Tiny => Some(1),
            FieldType::Short | FieldType::Year => Some(2),
            FieldType::Long | FieldType::Float | FieldType::Int24 => Some(4),
            FieldType::LongLong | FieldType::Double => Some(8),
            _ => None,
        }
    }
}

bitflags! {
    /// Column/field flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FieldFlags: u16 {
        const NOT_NULL          = 0x0001;
        const PRIMARY_KEY       = 0x0002;
        const UNIQUE_KEY        = 0x0004;
        const MULTIPLE_KEY      = 0x0008;
        const BLOB              = 0x0010;
        const UNSIGNED          = 0x0020;
        const ZEROFILL          = 0x0040;
        const BINARY            = 0x0080;
        const ENUM              = 0x0100;
        const AUTO_INCREMENT    = 0x0200;
        const TIMESTAMP         = 0x0400;
        const SET               = 0x0800;
        const NO_DEFAULT_VALUE  = 0x1000;
        const ON_UPDATE_NOW     = 0x2000;
        const NUM               = 0x8000;
    }
}
```

---

## 6. Response Packets

```rust
use thiserror::Error;

/// MySQL error type
#[derive(Error, Debug)]
pub enum MySqlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("MySQL error {code}: {message} (SQLSTATE: {sql_state})")]
    Server {
        code: u16,
        sql_state: String,
        message: String,
    },
    
    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// OK packet response
#[derive(Debug, Clone)]
pub struct OkPacket {
    pub affected_rows: u64,
    pub last_insert_id: u64,
    pub status_flags: StatusFlags,
    pub warnings: u16,
    pub info: Option<String>,
    pub session_state_info: Option<Vec<u8>>,
}

impl OkPacket {
    pub fn parse(data: &[u8]) -> Result<Self, MySqlError> {
        let mut cursor = std::io::Cursor::new(data);
        
        let header = cursor.read_u8()?;
        if header != 0x00 && header != 0xFE {
            return Err(MySqlError::Protocol("Invalid OK packet header".into()));
        }
        
        let affected_rows = types::LenEnc::read_int(&mut cursor)?;
        let last_insert_id = types::LenEnc::read_int(&mut cursor)?;
        let status_flags = StatusFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);
        let warnings = cursor.read_u16::<LittleEndian>()?;
        
        let pos = cursor.position() as usize;
        let info = if pos < data.len() {
            Some(types::read_eof_string(data, pos)?)
        } else {
            None
        };
        
        Ok(Self {
            affected_rows,
            last_insert_id,
            status_flags,
            warnings,
            info,
            session_state_info: None,
        })
    }
}

/// ERR packet response
#[derive(Debug, Clone)]
pub struct ErrPacket {
    pub error_code: u16,
    pub sql_state: String,
    pub error_message: String,
}

impl ErrPacket {
    pub fn parse(data: &[u8]) -> Result<Self, MySqlError> {
        if data[0] != 0xFF {
            return Err(MySqlError::Protocol("Invalid ERR packet header".into()));
        }
        
        let mut cursor = std::io::Cursor::new(&data[1..]);
        let error_code = cursor.read_u16::<LittleEndian>()?;
        
        // Skip '#' marker
        let _ = cursor.read_u8()?;
        
        // Read SQLSTATE (5 chars)
        let sql_state = types::read_fixed_string(&mut cursor, 5)?;
        
        // Rest is error message
        let pos = cursor.position() as usize + 1; // +1 for initial 0xFF
        let error_message = types::read_eof_string(data, pos)?;
        
        Ok(Self {
            error_code,
            sql_state,
            error_message,
        })
    }
    
    pub fn into_error(self) -> MySqlError {
        MySqlError::Server {
            code: self.error_code,
            sql_state: self.sql_state,
            message: self.error_message,
        }
    }
}

/// EOF packet (deprecated in CLIENT_DEPRECATE_EOF)
#[derive(Debug, Clone)]
pub struct EofPacket {
    pub warnings: u16,
    pub status_flags: StatusFlags,
}

impl EofPacket {
    pub fn parse(data: &[u8]) -> Result<Self, MySqlError> {
        if data[0] != 0xFE {
            return Err(MySqlError::Protocol("Invalid EOF packet header".into()));
        }
        
        let mut cursor = std::io::Cursor::new(&data[1..]);
        let warnings = cursor.read_u16::<LittleEndian>()?;
        let status_flags = StatusFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);
        
        Ok(Self { warnings, status_flags })
    }
}

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
    pub flags: FieldFlags,
    pub decimals: u8,
}

impl ColumnDefinition {
    pub fn parse(data: &[u8]) -> Result<Self, MySqlError> {
        let mut cursor = std::io::Cursor::new(data);
        
        let catalog = types::LenEnc::read_string(&mut cursor)?;
        let schema = types::LenEnc::read_string(&mut cursor)?;
        let table = types::LenEnc::read_string(&mut cursor)?;
        let org_table = types::LenEnc::read_string(&mut cursor)?;
        let name = types::LenEnc::read_string(&mut cursor)?;
        let org_name = types::LenEnc::read_string(&mut cursor)?;
        
        // Skip fixed length field (0x0C)
        let _ = types::LenEnc::read_int(&mut cursor)?;
        
        let character_set = cursor.read_u16::<LittleEndian>()?;
        let column_length = cursor.read_u32::<LittleEndian>()?;
        let column_type = FieldType::from(cursor.read_u8()?);
        let flags = FieldFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);
        let decimals = cursor.read_u8()?;
        
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

impl From<u8> for FieldType {
    fn from(byte: u8) -> Self {
        match byte {
            0x00 => FieldType::Decimal,
            0x01 => FieldType::Tiny,
            0x02 => FieldType::Short,
            0x03 => FieldType::Long,
            0x04 => FieldType::Float,
            0x05 => FieldType::Double,
            0x06 => FieldType::Null,
            0x07 => FieldType::Timestamp,
            0x08 => FieldType::LongLong,
            0x09 => FieldType::Int24,
            0x0A => FieldType::Date,
            0x0B => FieldType::Time,
            0x0C => FieldType::DateTime,
            0x0D => FieldType::Year,
            0xF2 => FieldType::Vector,
            0xF5 => FieldType::Json,
            0xF6 => FieldType::NewDecimal,
            0xFC => FieldType::Blob,
            0xFD => FieldType::VarString,
            0xFE => FieldType::String,
            0xFF => FieldType::Geometry,
            _ => FieldType::Invalid,
        }
    }
}
```

---

## 7. Authentication

### 7.1 mysql_native_password

```rust
use sha1::{Sha1, Digest};

/// MySQL native password authentication
/// SHA1(password) XOR SHA1(scramble + SHA1(SHA1(password)))
pub fn mysql_native_password(password: &str, scramble: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return Vec::new();
    }
    
    // Stage 1: SHA1(password)
    let stage1 = Sha1::digest(password.as_bytes());
    
    // Stage 2: SHA1(SHA1(password))
    let stage2 = Sha1::digest(&stage1);
    
    // Stage 3: SHA1(scramble + stage2)
    let mut hasher = Sha1::new();
    hasher.update(scramble);
    hasher.update(&stage2);
    let stage3 = hasher.finalize();
    
    // XOR stage1 with stage3
    stage1.iter()
        .zip(stage3.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}
```

### 7.2 caching_sha2_password

```rust
use sha2::{Sha256, Digest};

/// Caching SHA-256 authentication (MySQL 8.0+ default)
/// SHA256(password) XOR SHA256(SHA256(SHA256(password)) + scramble)
pub fn caching_sha2_password(password: &str, scramble: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return Vec::new();
    }
    
    // Stage 1: SHA256(password)
    let stage1 = Sha256::digest(password.as_bytes());
    
    // Stage 2: SHA256(stage1)
    let stage2 = Sha256::digest(&stage1);
    
    // Stage 3: SHA256(stage2 + scramble)
    let mut hasher = Sha256::new();
    hasher.update(&stage2);
    hasher.update(scramble);
    let stage3 = hasher.finalize();
    
    // XOR stage1 with stage3
    stage1.iter()
        .zip(stage3.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}

/// Fast auth result codes
pub const CACHING_SHA2_FAST_AUTH_SUCCESS: u8 = 0x03;
pub const CACHING_SHA2_PERFORM_FULL_AUTH: u8 = 0x04;
```

### 7.3 ed25519 (MariaDB)

```rust
use ed25519_dalek::{SigningKey, Signer};
use sha2::{Sha512, Digest};

/// MariaDB ed25519 authentication
pub struct Ed25519Auth;

impl Ed25519Auth {
    /// Generate signing key from password
    pub fn signing_key_from_password(password: &str) -> SigningKey {
        // Hash password with SHA-512 to get seed
        let hash = Sha512::digest(password.as_bytes());
        
        // Use first 32 bytes as ed25519 seed
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&hash[..32]);
        
        SigningKey::from_bytes(&seed)
    }
    
    /// Sign the server nonce
    pub fn sign_nonce(password: &str, nonce: &[u8]) -> Vec<u8> {
        let signing_key = Self::signing_key_from_password(password);
        let signature = signing_key.sign(nonce);
        signature.to_bytes().to_vec()
    }
    
    /// Get the public key for storage in mysql.user
    pub fn public_key(password: &str) -> Vec<u8> {
        let signing_key = Self::signing_key_from_password(password);
        signing_key.verifying_key().to_bytes().to_vec()
    }
}
```

---

## 8. SQL Data Types

### 8.1 Rust Value Types

```rust
use std::time::{Duration, SystemTime};

/// MySQL value representation
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f32),
    Double(f64),
    Decimal(String),
    String(String),
    Bytes(Vec<u8>),
    Date { year: u16, month: u8, day: u8 },
    Time { negative: bool, days: u32, hours: u8, minutes: u8, seconds: u8, microseconds: u32 },
    DateTime { year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8, microseconds: u32 },
    Json(serde_json::Value),
    Vector(Vec<f32>),  // MySQL 9.0+
    Geometry(Vec<u8>),
}

impl Value {
    /// Convert to SQL string representation
    pub fn to_sql(&self) -> String {
        match self {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            Value::Int(i) => i.to_string(),
            Value::UInt(u) => u.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Double(d) => d.to_string(),
            Value::Decimal(s) => s.clone(),
            Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            Value::Bytes(b) => format!("X'{}'", hex::encode(b)),
            Value::Date { year, month, day } => format!("'{:04}-{:02}-{:02}'", year, month, day),
            Value::Time { negative, days, hours, minutes, seconds, microseconds } => {
                let sign = if *negative { "-" } else { "" };
                if *microseconds > 0 {
                    format!("'{}{}:{:02}:{:02}.{:06}'", sign, days * 24 + *hours as u32, minutes, seconds, microseconds)
                } else {
                    format!("'{}{}:{:02}:{:02}'", sign, days * 24 + *hours as u32, minutes, seconds)
                }
            }
            Value::DateTime { year, month, day, hour, minute, second, microseconds } => {
                if *microseconds > 0 {
                    format!("'{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}'", year, month, day, hour, minute, second, microseconds)
                } else {
                    format!("'{:04}-{:02}-{:02} {:02}:{:02}:{:02}'", year, month, day, hour, minute, second)
                }
            }
            Value::Json(v) => format!("'{}'", v.to_string().replace('\'', "''")),
            Value::Vector(v) => format!("STRING_TO_VECTOR('[{}]')", 
                v.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
            Value::Geometry(b) => format!("X'{}'", hex::encode(b)),
        }
    }
    
    /// Check if value is NULL
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    
    /// Try to convert to i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::UInt(u) => (*u).try_into().ok(),
            Value::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }
    
    /// Try to convert to String
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Binary protocol value reader
impl Value {
    pub fn read_binary(data: &[u8], field: &ColumnDefinition) -> Result<Self, MySqlError> {
        let mut cursor = std::io::Cursor::new(data);
        
        match field.column_type {
            FieldType::Null => Ok(Value::Null),
            
            FieldType::Tiny => {
                let v = cursor.read_u8()?;
                if field.flags.contains(FieldFlags::UNSIGNED) {
                    Ok(Value::UInt(v as u64))
                } else {
                    Ok(Value::Int(v as i8 as i64))
                }
            }
            
            FieldType::Short | FieldType::Year => {
                let v = cursor.read_u16::<LittleEndian>()?;
                if field.flags.contains(FieldFlags::UNSIGNED) {
                    Ok(Value::UInt(v as u64))
                } else {
                    Ok(Value::Int(v as i16 as i64))
                }
            }
            
            FieldType::Long | FieldType::Int24 => {
                let v = cursor.read_u32::<LittleEndian>()?;
                if field.flags.contains(FieldFlags::UNSIGNED) {
                    Ok(Value::UInt(v as u64))
                } else {
                    Ok(Value::Int(v as i32 as i64))
                }
            }
            
            FieldType::LongLong => {
                let v = cursor.read_u64::<LittleEndian>()?;
                if field.flags.contains(FieldFlags::UNSIGNED) {
                    Ok(Value::UInt(v))
                } else {
                    Ok(Value::Int(v as i64))
                }
            }
            
            FieldType::Float => Ok(Value::Float(cursor.read_f32::<LittleEndian>()?)),
            FieldType::Double => Ok(Value::Double(cursor.read_f64::<LittleEndian>()?)),
            
            FieldType::Date => {
                let len = cursor.read_u8()?;
                if len == 0 {
                    Ok(Value::Date { year: 0, month: 0, day: 0 })
                } else {
                    Ok(Value::Date {
                        year: cursor.read_u16::<LittleEndian>()?,
                        month: cursor.read_u8()?,
                        day: cursor.read_u8()?,
                    })
                }
            }
            
            FieldType::DateTime | FieldType::Timestamp => {
                let len = cursor.read_u8()?;
                if len == 0 {
                    Ok(Value::DateTime { year: 0, month: 0, day: 0, hour: 0, minute: 0, second: 0, microseconds: 0 })
                } else {
                    let year = cursor.read_u16::<LittleEndian>()?;
                    let month = cursor.read_u8()?;
                    let day = cursor.read_u8()?;
                    let (hour, minute, second, microseconds) = if len > 4 {
                        let h = cursor.read_u8()?;
                        let m = cursor.read_u8()?;
                        let s = cursor.read_u8()?;
                        let us = if len > 7 { cursor.read_u32::<LittleEndian>()? } else { 0 };
                        (h, m, s, us)
                    } else {
                        (0, 0, 0, 0)
                    };
                    Ok(Value::DateTime { year, month, day, hour, minute, second, microseconds })
                }
            }
            
            FieldType::Json => {
                let bytes = types::LenEnc::read_bytes(&mut cursor)?;
                let json = serde_json::from_slice(&bytes)
                    .map_err(|e| MySqlError::Protocol(format!("Invalid JSON: {}", e)))?;
                Ok(Value::Json(json))
            }
            
            FieldType::Vector => {
                let bytes = types::LenEnc::read_bytes(&mut cursor)?;
                let floats: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok(Value::Vector(floats))
            }
            
            _ => {
                // String types
                let bytes = types::LenEnc::read_bytes(&mut cursor)?;
                if field.flags.contains(FieldFlags::BINARY) {
                    Ok(Value::Bytes(bytes))
                } else {
                    Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
                }
            }
        }
    }
}
```

### 8.2 VECTOR Type (MySQL 9.0+)

```rust
/// MySQL VECTOR type support
#[derive(Debug, Clone, PartialEq)]
pub struct MySqlVector {
    pub dimensions: usize,
    pub values: Vec<f32>,
}

impl MySqlVector {
    /// Maximum vector dimensions
    pub const MAX_DIMENSIONS: usize = 16383;
    
    /// Default dimensions
    pub const DEFAULT_DIMENSIONS: usize = 2048;
    
    /// Create new vector from values
    pub fn new(values: Vec<f32>) -> Result<Self, MySqlError> {
        if values.len() > Self::MAX_DIMENSIONS {
            return Err(MySqlError::Protocol(format!(
                "Vector dimension {} exceeds maximum {}",
                values.len(),
                Self::MAX_DIMENSIONS
            )));
        }
        Ok(Self {
            dimensions: values.len(),
            values,
        })
    }
    
    /// Parse from text format: "[1.0, 2.0, 3.0]"
    pub fn from_text(text: &str) -> Result<Self, MySqlError> {
        let text = text.trim();
        if !text.starts_with('[') || !text.ends_with(']') {
            return Err(MySqlError::Protocol("Vector must be enclosed in []".into()));
        }
        
        let inner = &text[1..text.len()-1];
        let values: Result<Vec<f32>, _> = inner
            .split(',')
            .map(|s| s.trim().parse())
            .collect();
        
        let values = values.map_err(|e| MySqlError::Protocol(format!("Invalid vector value: {}", e)))?;
        Self::new(values)
    }
    
    /// Convert to text format
    pub fn to_text(&self) -> String {
        format!("[{}]", self.values.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", "))
    }
    
    /// Convert to binary format
    pub fn to_binary(&self) -> Vec<u8> {
        self.values.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect()
    }
    
    /// Parse from binary format
    pub fn from_binary(data: &[u8]) -> Result<Self, MySqlError> {
        if data.len() % 4 != 0 {
            return Err(MySqlError::Protocol("Invalid vector binary data".into()));
        }
        
        let values: Vec<f32> = data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        
        Self::new(values)
    }
    
    /// Compute Euclidean distance to another vector
    pub fn euclidean_distance(&self, other: &MySqlVector) -> Result<f64, MySqlError> {
        if self.dimensions != other.dimensions {
            return Err(MySqlError::Protocol("Vector dimensions must match".into()));
        }
        
        let sum: f64 = self.values.iter()
            .zip(other.values.iter())
            .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
            .sum();
        
        Ok(sum.sqrt())
    }
    
    /// Compute cosine similarity with another vector
    pub fn cosine_similarity(&self, other: &MySqlVector) -> Result<f64, MySqlError> {
        if self.dimensions != other.dimensions {
            return Err(MySqlError::Protocol("Vector dimensions must match".into()));
        }
        
        let dot: f64 = self.values.iter()
            .zip(other.values.iter())
            .map(|(a, b)| *a as f64 * *b as f64)
            .sum();
        
        let mag_a: f64 = self.values.iter().map(|a| (*a as f64).powi(2)).sum::<f64>().sqrt();
        let mag_b: f64 = other.values.iter().map(|b| (*b as f64).powi(2)).sum::<f64>().sqrt();
        
        Ok(dot / (mag_a * mag_b))
    }
}
```

---

## 9. SQL Keywords

```rust
use std::collections::HashSet;
use once_cell::sync::Lazy;

/// MySQL 9.5 reserved keywords
pub static RESERVED_KEYWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "ACCESSIBLE", "ADD", "ALL", "ALTER", "ANALYZE", "AND", "AS", "ASC",
        "ASENSITIVE", "BEFORE", "BETWEEN", "BIGINT", "BINARY", "BLOB", "BOTH",
        "BY", "CALL", "CASCADE", "CASE", "CHANGE", "CHAR", "CHARACTER", "CHECK",
        "COLLATE", "COLUMN", "CONDITION", "CONSTRAINT", "CONTINUE", "CONVERT",
        "CREATE", "CROSS", "CUBE", "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP",
        "CURRENT_USER", "CURSOR", "DATABASE", "DATABASES", "DAY_HOUR", "DAY_MICROSECOND",
        "DAY_MINUTE", "DAY_SECOND", "DEC", "DECIMAL", "DECLARE", "DEFAULT", "DELAYED",
        "DELETE", "DENSE_RANK", "DESC", "DESCRIBE", "DETERMINISTIC", "DISTINCT",
        "DISTINCTROW", "DIV", "DOUBLE", "DROP", "DUAL", "EACH", "ELSE", "ELSEIF",
        "ENCLOSED", "ESCAPED", "EXCEPT", "EXISTS", "EXIT", "EXPLAIN", "FALSE", "FETCH",
        "FIRST_VALUE", "FLOAT", "FOR", "FORCE", "FOREIGN", "FROM", "FULLTEXT",
        "FUNCTION", "GENERATED", "GET", "GRANT", "GROUP", "GROUPS", "HAVING",
        "HIGH_PRIORITY", "HOUR_MICROSECOND", "HOUR_MINUTE", "HOUR_SECOND", "IF",
        "IGNORE", "IN", "INDEX", "INFILE", "INNER", "INOUT", "INSENSITIVE", "INSERT",
        "INT", "INTEGER", "INTERSECT", "INTERVAL", "INTO", "IS", "ITERATE", "JOIN",
        "JSON_TABLE", "KEY", "KEYS", "KILL", "LAG", "LAST_VALUE", "LATERAL", "LEAD",
        "LEADING", "LEAVE", "LEFT", "LIKE", "LIMIT", "LINEAR", "LINES", "LOAD",
        "LOCALTIME", "LOCALTIMESTAMP", "LOCK", "LONG", "LONGBLOB", "LONGTEXT", "LOOP",
        "LOW_PRIORITY", "MATCH", "MAXVALUE", "MEDIUMBLOB", "MEDIUMINT", "MEDIUMTEXT",
        "MINUTE_MICROSECOND", "MINUTE_SECOND", "MOD", "MODIFIES", "NATURAL", "NOT",
        "NO_WRITE_TO_BINLOG", "NTH_VALUE", "NTILE", "NULL", "NUMERIC", "OF", "ON",
        "OPTIMIZE", "OPTION", "OPTIONALLY", "OR", "ORDER", "OUT", "OUTER", "OUTFILE",
        "OVER", "PARTITION", "PERCENT_RANK", "PRECISION", "PRIMARY", "PROCEDURE",
        "PURGE", "RANGE", "RANK", "READ", "READS", "READ_WRITE", "REAL", "RECURSIVE",
        "REFERENCES", "REGEXP", "RELEASE", "RENAME", "REPEAT", "REPLACE", "REQUIRE",
        "RESIGNAL", "RESTRICT", "RETURN", "REVOKE", "RIGHT", "RLIKE", "ROW", "ROWS",
        "ROW_NUMBER", "SCHEMA", "SCHEMAS", "SECOND_MICROSECOND", "SELECT", "SENSITIVE",
        "SEPARATOR", "SET", "SHOW", "SIGNAL", "SMALLINT", "SPATIAL", "SPECIFIC", "SQL",
        "SQLEXCEPTION", "SQLSTATE", "SQLWARNING", "SQL_BIG_RESULT", "SQL_CALC_FOUND_ROWS",
        "SQL_SMALL_RESULT", "SSL", "STARTING", "STORED", "STRAIGHT_JOIN", "SYSTEM",
        "TABLE", "TERMINATED", "THEN", "TINYBLOB", "TINYINT", "TINYTEXT", "TO",
        "TRAILING", "TRIGGER", "TRUE", "UNDO", "UNION", "UNIQUE", "UNLOCK", "UNSIGNED",
        "UPDATE", "USAGE", "USE", "USING", "UTC_DATE", "UTC_TIME", "UTC_TIMESTAMP",
        "VALUES", "VARBINARY", "VARCHAR", "VARYING", "VECTOR", "VIRTUAL", "WHEN",
        "WHERE", "WHILE", "WINDOW", "WITH", "WRITE", "XOR", "YEAR_MONTH", "ZEROFILL",
    ].into_iter().collect()
});

/// Check if identifier is a reserved keyword
pub fn is_reserved_keyword(identifier: &str) -> bool {
    RESERVED_KEYWORDS.contains(identifier.to_uppercase().as_str())
}

/// Quote identifier if it's a reserved keyword or contains special characters
pub fn quote_identifier(identifier: &str) -> String {
    if is_reserved_keyword(identifier) 
        || identifier.contains(' ')
        || identifier.contains('-')
        || identifier.chars().next().map_or(false, |c| c.is_ascii_digit())
    {
        format!("`{}`", identifier.replace('`', "``"))
    } else {
        identifier.to_string()
    }
}
```

---

## 10. AST Definitions

```rust
/// AST node types
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    // Statements
    Select(Box<SelectStmt>),
    Insert(Box<InsertStmt>),
    Update(Box<UpdateStmt>),
    Delete(Box<DeleteStmt>),
    CreateTable(Box<CreateTableStmt>),
    AlterTable(Box<AlterTableStmt>),
    DropTable(Box<DropTableStmt>),
    Merge(Box<MergeStmt>),
    
    // Expressions
    ColumnRef(ColumnRef),
    Const(ConstValue),
    FuncCall(Box<FuncCall>),
    BinaryExpr(Box<BinaryExpr>),
    UnaryExpr(Box<UnaryExpr>),
    Subquery(Box<Subquery>),
    CaseExpr(Box<CaseExpr>),
    ParamRef(ParamRef),
    
    // Table references
    TableRef(TableRef),
    JoinExpr(Box<JoinExpr>),
    DerivedTable(Box<DerivedTable>),
    Cte(Cte),
}

/// SELECT statement
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectStmt {
    pub distinct: bool,
    pub select_list: Vec<SelectItem>,
    pub from_clause: Vec<Node>,
    pub where_clause: Option<Box<Node>>,
    pub group_by: Vec<Node>,
    pub having: Option<Box<Node>>,
    pub window_clause: Vec<WindowDef>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<Limit>,
    pub for_update: bool,
    pub set_op: Option<SetOp>,
}

/// SELECT item (target column)
#[derive(Debug, Clone, PartialEq)]
pub struct SelectItem {
    pub expr: Node,
    pub alias: Option<String>,
}

impl SelectItem {
    pub fn expr(expr: Node) -> Self {
        Self { expr, alias: None }
    }
    
    pub fn aliased(expr: Node, alias: impl Into<String>) -> Self {
        Self { expr, alias: Some(alias.into()) }
    }
    
    pub fn star() -> Self {
        Self {
            expr: Node::ColumnRef(ColumnRef::star()),
            alias: None,
        }
    }
}

/// Set operations (UNION, INTERSECT, EXCEPT)
#[derive(Debug, Clone, PartialEq)]
pub struct SetOp {
    pub op_type: SetOpType,
    pub all: bool,
    pub right: Box<SelectStmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOpType {
    Union,
    Intersect,
    Except,
}

/// INSERT statement
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStmt {
    pub table: TableRef,
    pub columns: Option<Vec<String>>,
    pub source: InsertSource,
    pub on_duplicate: Option<Vec<Assignment>>,
    pub returning: Option<Vec<SelectItem>>,
    pub ignore: bool,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    Values(Vec<Vec<Node>>),
    Select(Box<SelectStmt>),
    Set(Vec<Assignment>),
}

/// UPDATE statement
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStmt {
    pub tables: Vec<Node>,
    pub set_clause: Vec<Assignment>,
    pub where_clause: Option<Box<Node>>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<Limit>,
    pub ignore: bool,
}

/// DELETE statement
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStmt {
    pub tables: Vec<TableRef>,
    pub from_clause: Vec<Node>,
    pub using_clause: Option<Vec<Node>>,
    pub where_clause: Option<Box<Node>>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<Limit>,
    pub ignore: bool,
}

/// MERGE statement (MySQL 8.0.25+)
#[derive(Debug, Clone, PartialEq)]
pub struct MergeStmt {
    pub target: TableRef,
    pub source: Node,
    pub on_condition: Box<Node>,
    pub when_clauses: Vec<MergeWhenClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeWhenClause {
    pub matched: bool,
    pub condition: Option<Box<Node>>,
    pub action: MergeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MergeAction {
    Update(Vec<Assignment>),
    Delete,
    Insert { columns: Option<Vec<String>>, values: Vec<Node> },
}

/// Column reference
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRef {
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub column: String,
}

impl ColumnRef {
    pub fn new(column: impl Into<String>) -> Self {
        Self {
            catalog: None,
            schema: None,
            table: None,
            column: column.into(),
        }
    }
    
    pub fn qualified(table: impl Into<String>, column: impl Into<String>) -> Self {
        Self {
            catalog: None,
            schema: None,
            table: Some(table.into()),
            column: column.into(),
        }
    }
    
    pub fn star() -> Self {
        Self::new("*")
    }
    
    pub fn table_star(table: impl Into<String>) -> Self {
        Self::qualified(table, "*")
    }
}

/// Constant value
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Hex(Vec<u8>),
    Bit(Vec<bool>),
}

impl ConstValue {
    pub fn null() -> Node {
        Node::Const(ConstValue::Null)
    }
    
    pub fn bool(v: bool) -> Node {
        Node::Const(ConstValue::Bool(v))
    }
    
    pub fn int(v: i64) -> Node {
        Node::Const(ConstValue::Integer(v))
    }
    
    pub fn float(v: f64) -> Node {
        Node::Const(ConstValue::Float(v))
    }
    
    pub fn string(v: impl Into<String>) -> Node {
        Node::Const(ConstValue::String(v.into()))
    }
}

/// Function call
#[derive(Debug, Clone, PartialEq)]
pub struct FuncCall {
    pub name: String,
    pub schema: Option<String>,
    pub args: Vec<Node>,
    pub distinct: bool,
    pub over: Option<WindowSpec>,
    pub filter: Option<Box<Node>>,
}

impl FuncCall {
    pub fn new(name: impl Into<String>, args: Vec<Node>) -> Self {
        Self {
            name: name.into(),
            schema: None,
            args,
            distinct: false,
            over: None,
            filter: None,
        }
    }
    
    pub fn count_star() -> Self {
        Self::new("COUNT", vec![Node::ColumnRef(ColumnRef::star())])
    }
    
    pub fn count(expr: Node) -> Self {
        Self::new("COUNT", vec![expr])
    }
    
    pub fn sum(expr: Node) -> Self {
        Self::new("SUM", vec![expr])
    }
    
    pub fn avg(expr: Node) -> Self {
        Self::new("AVG", vec![expr])
    }
    
    pub fn max(expr: Node) -> Self {
        Self::new("MAX", vec![expr])
    }
    
    pub fn min(expr: Node) -> Self {
        Self::new("MIN", vec![expr])
    }
}

/// Binary expression
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Node,
    pub right: Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add, Sub, Mul, Div, Mod, IntDiv,
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge, NullSafeEq,
    Like, NotLike, Regexp, NotRegexp,
    // Logical
    And, Or, Xor,
    // Bitwise
    BitAnd, BitOr, BitXor, ShiftLeft, ShiftRight,
    // JSON
    JsonExtract, JsonUnquoteExtract,
}

impl BinaryExpr {
    pub fn new(op: BinaryOp, left: Node, right: Node) -> Self {
        Self { op, left, right }
    }
    
    pub fn eq(left: Node, right: Node) -> Node {
        Node::BinaryExpr(Box::new(Self::new(BinaryOp::Eq, left, right)))
    }
    
    pub fn ne(left: Node, right: Node) -> Node {
        Node::BinaryExpr(Box::new(Self::new(BinaryOp::Ne, left, right)))
    }
    
    pub fn and(left: Node, right: Node) -> Node {
        Node::BinaryExpr(Box::new(Self::new(BinaryOp::And, left, right)))
    }
    
    pub fn or(left: Node, right: Node) -> Node {
        Node::BinaryExpr(Box::new(Self::new(BinaryOp::Or, left, right)))
    }
}

/// Unary expression
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not, Neg, BitNot, IsNull, IsNotNull, IsTrue, IsFalse,
}

/// Subquery
#[derive(Debug, Clone, PartialEq)]
pub struct Subquery {
    pub query: SelectStmt,
    pub subquery_type: SubqueryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubqueryType {
    Scalar,
    Exists,
    In,
    NotIn,
    Any,
    All,
    Some,
}

/// CASE expression
#[derive(Debug, Clone, PartialEq)]
pub struct CaseExpr {
    pub operand: Option<Node>,
    pub when_clauses: Vec<WhenClause>,
    pub else_clause: Option<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenClause {
    pub condition: Node,
    pub result: Node,
}

/// Parameter reference (?)
#[derive(Debug, Clone, PartialEq)]
pub struct ParamRef {
    pub index: usize,  // 1-based
}

/// Table reference
#[derive(Debug, Clone, PartialEq)]
pub struct TableRef {
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub alias: Option<String>,
    pub index_hints: Vec<IndexHint>,
    pub partitions: Vec<String>,
}

impl TableRef {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            catalog: None,
            schema: None,
            table: table.into(),
            alias: None,
            index_hints: Vec::new(),
            partitions: Vec::new(),
        }
    }
    
    pub fn with_schema(schema: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            catalog: None,
            schema: Some(schema.into()),
            table: table.into(),
            alias: None,
            index_hints: Vec::new(),
            partitions: Vec::new(),
        }
    }
    
    pub fn aliased(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexHint {
    pub hint_type: IndexHintType,
    pub for_clause: Option<IndexHintFor>,
    pub indexes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexHintType {
    Use,
    Ignore,
    Force,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexHintFor {
    Join,
    OrderBy,
    GroupBy,
}

/// JOIN expression
#[derive(Debug, Clone, PartialEq)]
pub struct JoinExpr {
    pub join_type: JoinType,
    pub left: Node,
    pub right: Node,
    pub on_condition: Option<Box<Node>>,
    pub using_columns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,  // MariaDB only
    Cross,
    Natural,
    StraightJoin,
}

impl JoinExpr {
    pub fn inner(left: Node, right: Node, on_condition: Node) -> Self {
        Self {
            join_type: JoinType::Inner,
            left,
            right,
            on_condition: Some(Box::new(on_condition)),
            using_columns: None,
        }
    }
    
    pub fn left(left: Node, right: Node, on_condition: Node) -> Self {
        Self {
            join_type: JoinType::Left,
            left,
            right,
            on_condition: Some(Box::new(on_condition)),
            using_columns: None,
        }
    }
}

/// Derived table (subquery in FROM clause)
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedTable {
    pub subquery: SelectStmt,
    pub alias: String,
    pub column_aliases: Option<Vec<String>>,
    pub lateral: bool,
}

/// Common Table Expression (CTE)
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    pub name: String,
    pub columns: Option<Vec<String>>,
    pub query: SelectStmt,
    pub recursive: bool,
    pub materialized: Option<bool>,
}

/// Window specification
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub name: Option<String>,
    pub partition_by: Vec<Node>,
    pub order_by: Vec<OrderByItem>,
    pub frame: Option<WindowFrame>,
}

/// Window definition
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDef {
    pub name: String,
    pub spec: WindowSpec,
}

/// Window frame
#[derive(Debug, Clone, PartialEq)]
pub struct WindowFrame {
    pub frame_type: WindowFrameType,
    pub start: WindowFrameBound,
    pub end: Option<WindowFrameBound>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFrameType {
    Rows,
    Range,
    Groups,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowFrameBound {
    CurrentRow,
    UnboundedPreceding,
    UnboundedFollowing,
    Preceding(Box<Node>),
    Following(Box<Node>),
}

/// ORDER BY item
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expr: Node,
    pub direction: SortDirection,
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsOrder {
    First,
    Last,
}

impl OrderByItem {
    pub fn asc(expr: Node) -> Self {
        Self { expr, direction: SortDirection::Asc, nulls: None }
    }
    
    pub fn desc(expr: Node) -> Self {
        Self { expr, direction: SortDirection::Desc, nulls: None }
    }
}

/// LIMIT clause
#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    pub count: Box<Node>,
    pub offset: Option<Box<Node>>,
}

/// Assignment (SET clause)
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: ColumnRef,
    pub value: Node,
}
```

---

## 11. Connection Implementation

```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

/// MySQL connection
pub struct Connection {
    stream: BufReader<BufWriter<TcpStream>>,
    sequence_id: u8,
    capabilities: CapabilityFlags,
    server_version: String,
    connection_id: u32,
    charset: u8,
}

impl Connection {
    /// Connect to MySQL/MariaDB server
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        database: Option<&str>,
    ) -> Result<Self, MySqlError> {
        let stream = TcpStream::connect((host, port)).await?;
        let stream = BufReader::new(BufWriter::new(stream));
        
        let mut conn = Self {
            stream,
            sequence_id: 0,
            capabilities: CapabilityFlags::empty(),
            server_version: String::new(),
            connection_id: 0,
            charset: 255, // utf8mb4
        };
        
        // Read server greeting
        conn.read_handshake().await?;
        
        // Send authentication response
        conn.send_auth_response(user, password, database).await?;
        
        // Read auth result
        conn.read_auth_result().await?;
        
        Ok(conn)
    }
    
    async fn read_handshake(&mut self) -> Result<(), MySqlError> {
        let packet = self.read_packet().await?;
        
        let mut cursor = std::io::Cursor::new(&packet.payload);
        
        // Protocol version
        let protocol = cursor.read_u8()?;
        if protocol != 10 {
            return Err(MySqlError::Protocol(format!(
                "Unsupported protocol version: {}", protocol
            )));
        }
        
        // Server version
        self.server_version = types::read_nul_string(&mut cursor)?;
        
        // Connection ID
        self.connection_id = cursor.read_u32::<LittleEndian>()?;
        
        // Auth plugin data part 1 (8 bytes)
        let mut scramble = [0u8; 20];
        cursor.read_exact(&mut scramble[..8])?;
        
        // Filler
        let _ = cursor.read_u8()?;
        
        // Capability flags (lower 2 bytes)
        let cap_lower = cursor.read_u16::<LittleEndian>()?;
        
        // Character set
        self.charset = cursor.read_u8()?;
        
        // Status flags
        let _ = cursor.read_u16::<LittleEndian>()?;
        
        // Capability flags (upper 2 bytes)
        let cap_upper = cursor.read_u16::<LittleEndian>()?;
        
        self.capabilities = CapabilityFlags::from_bits_truncate(
            (cap_upper as u32) << 16 | cap_lower as u32
        );
        
        // Auth plugin data length
        let auth_len = cursor.read_u8()?;
        
        // Reserved (10 bytes)
        cursor.set_position(cursor.position() + 10);
        
        // Auth plugin data part 2
        if self.capabilities.contains(CapabilityFlags::SECURE_CONNECTION) {
            let part2_len = std::cmp::max(13, auth_len as usize - 8);
            cursor.read_exact(&mut scramble[8..8 + part2_len - 1])?;
        }
        
        Ok(())
    }
    
    async fn send_auth_response(
        &mut self,
        user: &str,
        password: &str,
        database: Option<&str>,
    ) -> Result<(), MySqlError> {
        let mut buf = Vec::new();
        
        // Client capabilities
        let mut caps = CapabilityFlags::PROTOCOL_41 
            | CapabilityFlags::SECURE_CONNECTION
            | CapabilityFlags::PLUGIN_AUTH
            | CapabilityFlags::DEPRECATE_EOF;
        
        if database.is_some() {
            caps |= CapabilityFlags::CONNECT_WITH_DB;
        }
        
        buf.write_u32::<LittleEndian>(caps.bits())?;
        
        // Max packet size
        buf.write_u32::<LittleEndian>(MAX_PACKET_SIZE)?;
        
        // Character set
        buf.write_u8(255)?; // utf8mb4
        
        // Reserved (23 bytes)
        buf.extend_from_slice(&[0u8; 23]);
        
        // Username
        buf.extend_from_slice(user.as_bytes());
        buf.push(0);
        
        // Auth response (mysql_native_password for now)
        // In real implementation, use the scramble from handshake
        let auth_response = mysql_native_password(password, &[0u8; 20]);
        buf.write_u8(auth_response.len() as u8)?;
        buf.extend_from_slice(&auth_response);
        
        // Database
        if let Some(db) = database {
            buf.extend_from_slice(db.as_bytes());
            buf.push(0);
        }
        
        // Auth plugin name
        buf.extend_from_slice(b"mysql_native_password\0");
        
        self.write_packet(&buf).await?;
        
        Ok(())
    }
    
    async fn read_auth_result(&mut self) -> Result<(), MySqlError> {
        let packet = self.read_packet().await?;
        
        match packet.payload.first() {
            Some(0x00) | Some(0xFE) => Ok(()), // OK or EOF (auth switch not implemented)
            Some(0xFF) => {
                let err = ErrPacket::parse(&packet.payload)?;
                Err(err.into_error())
            }
            _ => Err(MySqlError::Protocol("Unexpected auth response".into())),
        }
    }
    
    /// Execute a text query
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, MySqlError> {
        // Send COM_QUERY
        let mut buf = vec![Command::Query as u8];
        buf.extend_from_slice(sql.as_bytes());
        
        self.sequence_id = 0;
        self.write_packet(&buf).await?;
        
        // Read response
        let packet = self.read_packet().await?;
        
        match packet.payload.first() {
            Some(0x00) => {
                let ok = OkPacket::parse(&packet.payload)?;
                Ok(QueryResult::Ok(ok))
            }
            Some(0xFF) => {
                let err = ErrPacket::parse(&packet.payload)?;
                Err(err.into_error())
            }
            _ => {
                // Result set
                let column_count = types::LenEnc::read_int(
                    &mut std::io::Cursor::new(&packet.payload)
                )? as usize;
                
                let columns = self.read_column_definitions(column_count).await?;
                let rows = self.read_rows(&columns).await?;
                
                Ok(QueryResult::ResultSet { columns, rows })
            }
        }
    }
    
    async fn read_column_definitions(&mut self, count: usize) -> Result<Vec<ColumnDefinition>, MySqlError> {
        let mut columns = Vec::with_capacity(count);
        
        for _ in 0..count {
            let packet = self.read_packet().await?;
            columns.push(ColumnDefinition::parse(&packet.payload)?);
        }
        
        // Read EOF packet (unless CLIENT_DEPRECATE_EOF)
        if !self.capabilities.contains(CapabilityFlags::DEPRECATE_EOF) {
            let _ = self.read_packet().await?;
        }
        
        Ok(columns)
    }
    
    async fn read_rows(&mut self, columns: &[ColumnDefinition]) -> Result<Vec<Vec<Value>>, MySqlError> {
        let mut rows = Vec::new();
        
        loop {
            let packet = self.read_packet().await?;
            
            // Check for EOF or OK packet
            if packet.payload.first() == Some(&0xFE) && packet.payload.len() < 9 {
                break;
            }
            if packet.payload.first() == Some(&0x00) {
                break;
            }
            
            // Parse text result row
            let mut cursor = std::io::Cursor::new(&packet.payload);
            let mut row = Vec::with_capacity(columns.len());
            
            for col in columns {
                let value = if packet.payload[cursor.position() as usize] == 0xFB {
                    cursor.set_position(cursor.position() + 1);
                    Value::Null
                } else {
                    let s = types::LenEnc::read_string(&mut cursor)?;
                    // Convert based on column type
                    match col.column_type {
                        FieldType::Tiny | FieldType::Short | FieldType::Long | 
                        FieldType::LongLong | FieldType::Int24 => {
                            if col.flags.contains(FieldFlags::UNSIGNED) {
                                Value::UInt(s.parse().unwrap_or(0))
                            } else {
                                Value::Int(s.parse().unwrap_or(0))
                            }
                        }
                        FieldType::Float => Value::Float(s.parse().unwrap_or(0.0)),
                        FieldType::Double => Value::Double(s.parse().unwrap_or(0.0)),
                        _ => Value::String(s),
                    }
                };
                row.push(value);
            }
            
            rows.push(row);
        }
        
        Ok(rows)
    }
    
    async fn read_packet(&mut self) -> Result<Packet, MySqlError> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header).await?;
        
        let len = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
        self.sequence_id = header[3];
        
        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload).await?;
        
        Ok(Packet {
            header: PacketHeader {
                payload_length: len as u32,
                sequence_id: self.sequence_id,
            },
            payload,
        })
    }
    
    async fn write_packet(&mut self, data: &[u8]) -> Result<(), MySqlError> {
        let header = [
            (data.len() & 0xFF) as u8,
            ((data.len() >> 8) & 0xFF) as u8,
            ((data.len() >> 16) & 0xFF) as u8,
            self.sequence_id,
        ];
        
        self.stream.get_mut().write_all(&header).await?;
        self.stream.get_mut().write_all(data).await?;
        self.stream.get_mut().flush().await?;
        
        self.sequence_id = self.sequence_id.wrapping_add(1);
        
        Ok(())
    }
    
    /// Close connection gracefully
    pub async fn close(mut self) -> Result<(), MySqlError> {
        self.sequence_id = 0;
        self.write_packet(&[Command::Quit as u8]).await?;
        Ok(())
    }
}

/// Query result
#[derive(Debug)]
pub enum QueryResult {
    Ok(OkPacket),
    ResultSet {
        columns: Vec<ColumnDefinition>,
        rows: Vec<Vec<Value>>,
    },
}

impl QueryResult {
    pub fn affected_rows(&self) -> u64 {
        match self {
            QueryResult::Ok(ok) => ok.affected_rows,
            QueryResult::ResultSet { rows, .. } => rows.len() as u64,
        }
    }
    
    pub fn last_insert_id(&self) -> Option<u64> {
        match self {
            QueryResult::Ok(ok) if ok.last_insert_id > 0 => Some(ok.last_insert_id),
            _ => None,
        }
    }
}
```

---

## 12. MySQL 9.5.0 & MariaDB 12.2.1 New Features

### MySQL 9.5.0 Features

```rust
/// JavaScript stored program example (MySQL Enterprise)
pub const JS_FUNCTION_EXAMPLE: &str = r#"
CREATE FUNCTION gcd(a INT, b INT)
RETURNS INT
LANGUAGE JAVASCRIPT AS
$$
    let x = Math.abs(a);
    let y = Math.abs(b);
    while (y) {
        let t = y;
        y = x % y;
        x = t;
    }
    return x;
$$
"#;

/// JSON Duality View example (MySQL 9.4+ Enterprise)
pub const JSON_DUALITY_VIEW_EXAMPLE: &str = r#"
CREATE JSON DUALITY VIEW customers_dv AS
SELECT JSON_OBJECT(
    'id', c.customer_id,
    'name', c.name,
    'orders', (
        SELECT JSON_ARRAYAGG(
            JSON_OBJECT('orderId', o.order_id, 'amount', o.total)
        )
        FROM orders o WHERE o.customer_id = c.customer_id
    )
)
FROM customers c
"#;

/// VECTOR type example
pub const VECTOR_TABLE_EXAMPLE: &str = r#"
CREATE TABLE embeddings (
    id INT PRIMARY KEY AUTO_INCREMENT,
    content TEXT,
    embedding VECTOR(1536)
);

INSERT INTO embeddings (content, embedding) 
VALUES ('Hello world', STRING_TO_VECTOR('[0.1, 0.2, ...]'));

-- Find similar vectors
SELECT id, content 
FROM embeddings 
ORDER BY VECTOR_DISTANCE(embedding, STRING_TO_VECTOR('[0.15, 0.25, ...]'))
LIMIT 10;
"#;
```

### MariaDB 12.2.1 Features

```rust
/// MariaDB associative arrays (Oracle compatibility)
pub const ASSOCIATIVE_ARRAY_EXAMPLE: &str = r#"
DECLARE
    TYPE salary_map IS TABLE OF DECIMAL(10,2) INDEX BY VARCHAR(100);
    salaries salary_map;
BEGIN
    salaries('Alice') := 75000.00;
    salaries('Bob') := 82000.00;
    
    IF salaries.EXISTS('Alice') THEN
        SELECT salaries('Alice') AS alice_salary;
    END IF;
    
    -- Iterate
    DECLARE
        emp_name VARCHAR(100);
    BEGIN
        emp_name := salaries.FIRST;
        WHILE emp_name IS NOT NULL LOOP
            SELECT emp_name, salaries(emp_name);
            emp_name := salaries.NEXT(emp_name);
        END LOOP;
    END;
END
"#;

/// MariaDB Oracle-style outer join
pub const ORACLE_OUTER_JOIN_EXAMPLE: &str = r#"
-- In Oracle mode (sql_mode='ORACLE')
SELECT e.name, d.dept_name
FROM employees e, departments d
WHERE e.dept_id = d.dept_id(+)  -- LEFT OUTER JOIN
"#;

/// MariaDB extended JSON (no depth limit in 12.2)
pub const DEEP_JSON_EXAMPLE: &str = r#"
-- MariaDB 12.2 removes the 32-level depth limit
SELECT JSON_EXTRACT(data, '$.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.l26.l27.l28.l29.l30.l31.l32.l33.l34.l35.value')
FROM documents
"#;
```

---

## 13. Cargo.toml Complete Dependencies

```toml
[package]
name = "mysql-protocol"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core
byteorder = "1.5"
bytes = "1.5"
bitflags = "2.4"
thiserror = "1.0"
once_cell = "1.19"
hex = "0.4"

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Cryptography
sha1 = "0.10"
sha2 = "0.10"
md5 = "0.7"
ed25519-dalek = { version = "2.1", optional = true }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[features]
default = []
mariadb-ed25519 = ["ed25519-dalek"]

[dev-dependencies]
tokio-test = "0.4"
```

---

## References

- MySQL 9.5 Release Notes: <https://dev.mysql.com/doc/relnotes/mysql/9.5/en/>
- MariaDB 12.2 Release Notes: <https://mariadb.com/docs/release-notes/community-server/12.2/12.2.1>
- MySQL Protocol Documentation: <https://dev.mysql.com/doc/dev/mysql-server/latest/PAGE_PROTOCOL.html>
- MariaDB Protocol Differences: <https://mariadb.com/kb/en/mariadb-protocol-differences-with-mysql/>

---

*Document Version: 1.0 | Generated for LLM coding tools*
