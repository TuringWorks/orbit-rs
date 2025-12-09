//! Arrow Flight SQL codec for encoding/decoding messages
//!
//! Handles serialization between OrbitQL types and Arrow format

use super::messages::*;
use super::types::*;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;

/// Flight data encoder for Arrow record batches
pub struct FlightDataEncoder {
    schema: SchemaInfo,
    batch_size: usize,
    compression: Option<CompressionType>,
}

/// Compression types for Flight data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    None,
    Lz4Frame,
    Zstd,
}

impl Default for CompressionType {
    fn default() -> Self {
        Self::None
    }
}

impl FlightDataEncoder {
    pub fn new(schema: SchemaInfo) -> Self {
        Self {
            schema,
            batch_size: 65536, // Default 64K rows per batch
            compression: None,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_compression(mut self, compression: CompressionType) -> Self {
        self.compression = Some(compression);
        self
    }

    /// Encode schema to IPC format
    pub fn encode_schema(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(1024);

        // Write schema header
        buf.put_u32_le(0); // Continuation token (0 = no continuation)
        buf.put_u32_le(self.schema.fields.len() as u32);

        // Write each field
        for field in &self.schema.fields {
            // Field name (length-prefixed string)
            let name_bytes = field.name.as_bytes();
            buf.put_u32_le(name_bytes.len() as u32);
            buf.put_slice(name_bytes);

            // Data type
            buf.put_u8(arrow_type_to_byte(field.data_type));

            // Nullable flag
            buf.put_u8(if field.nullable { 1 } else { 0 });

            // Metadata count and entries
            buf.put_u32_le(field.metadata.len() as u32);
            for (key, value) in &field.metadata {
                let key_bytes = key.as_bytes();
                buf.put_u32_le(key_bytes.len() as u32);
                buf.put_slice(key_bytes);
                let value_bytes = value.as_bytes();
                buf.put_u32_le(value_bytes.len() as u32);
                buf.put_slice(value_bytes);
            }
        }

        // Schema metadata
        buf.put_u32_le(self.schema.metadata.len() as u32);
        for (key, value) in &self.schema.metadata {
            let key_bytes = key.as_bytes();
            buf.put_u32_le(key_bytes.len() as u32);
            buf.put_slice(key_bytes);
            let value_bytes = value.as_bytes();
            buf.put_u32_le(value_bytes.len() as u32);
            buf.put_slice(value_bytes);
        }

        buf.freeze()
    }

    /// Encode a row batch header
    pub fn encode_batch_header(&self, num_rows: usize) -> Bytes {
        let mut buf = BytesMut::with_capacity(32);

        // Batch header
        buf.put_u32_le(1); // Version
        buf.put_u64_le(num_rows as u64);
        buf.put_u32_le(self.schema.fields.len() as u32);

        // Compression type
        buf.put_u8(match self.compression {
            None | Some(CompressionType::None) => 0,
            Some(CompressionType::Lz4Frame) => 1,
            Some(CompressionType::Zstd) => 2,
        });

        buf.freeze()
    }
}

/// Flight data decoder for Arrow record batches
pub struct FlightDataDecoder {
    schema: Option<SchemaInfo>,
}

impl FlightDataDecoder {
    pub fn new() -> Self {
        Self { schema: None }
    }

    /// Decode schema from IPC format
    pub fn decode_schema(&mut self, data: &mut Bytes) -> Result<SchemaInfo, FlightSqlError> {
        if data.remaining() < 8 {
            return Err(FlightSqlError::invalid_argument("Schema data too short"));
        }

        let _continuation = data.get_u32_le();
        let num_fields = data.get_u32_le() as usize;

        let mut fields = Vec::with_capacity(num_fields);

        for _ in 0..num_fields {
            // Read field name
            if data.remaining() < 4 {
                return Err(FlightSqlError::invalid_argument("Invalid field data"));
            }
            let name_len = data.get_u32_le() as usize;
            if data.remaining() < name_len + 2 {
                return Err(FlightSqlError::invalid_argument("Invalid field name"));
            }
            let name_bytes = data.copy_to_bytes(name_len);
            let name = String::from_utf8(name_bytes.to_vec())
                .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in field name"))?;

            // Read data type
            let data_type = byte_to_arrow_type(data.get_u8());

            // Read nullable
            let nullable = data.get_u8() != 0;

            // Read metadata
            if data.remaining() < 4 {
                return Err(FlightSqlError::invalid_argument("Invalid metadata"));
            }
            let metadata_count = data.get_u32_le() as usize;
            let mut metadata = HashMap::new();

            for _ in 0..metadata_count {
                if data.remaining() < 4 {
                    return Err(FlightSqlError::invalid_argument("Invalid metadata entry"));
                }
                let key_len = data.get_u32_le() as usize;
                if data.remaining() < key_len + 4 {
                    return Err(FlightSqlError::invalid_argument("Invalid metadata key"));
                }
                let key = String::from_utf8(data.copy_to_bytes(key_len).to_vec())
                    .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in key"))?;

                let value_len = data.get_u32_le() as usize;
                if data.remaining() < value_len {
                    return Err(FlightSqlError::invalid_argument("Invalid metadata value"));
                }
                let value = String::from_utf8(data.copy_to_bytes(value_len).to_vec())
                    .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in value"))?;

                metadata.insert(key, value);
            }

            fields.push(FieldInfo {
                name,
                data_type,
                nullable,
                metadata,
            });
        }

        // Read schema metadata
        if data.remaining() < 4 {
            return Err(FlightSqlError::invalid_argument("Invalid schema metadata"));
        }
        let schema_metadata_count = data.get_u32_le() as usize;
        let mut schema_metadata = HashMap::new();

        for _ in 0..schema_metadata_count {
            if data.remaining() < 4 {
                return Err(FlightSqlError::invalid_argument(
                    "Invalid schema metadata entry",
                ));
            }
            let key_len = data.get_u32_le() as usize;
            if data.remaining() < key_len + 4 {
                return Err(FlightSqlError::invalid_argument(
                    "Invalid schema metadata key",
                ));
            }
            let key = String::from_utf8(data.copy_to_bytes(key_len).to_vec())
                .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in key"))?;

            let value_len = data.get_u32_le() as usize;
            if data.remaining() < value_len {
                return Err(FlightSqlError::invalid_argument(
                    "Invalid schema metadata value",
                ));
            }
            let value = String::from_utf8(data.copy_to_bytes(value_len).to_vec())
                .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in value"))?;

            schema_metadata.insert(key, value);
        }

        let schema = SchemaInfo {
            fields,
            metadata: schema_metadata,
        };

        self.schema = Some(schema.clone());
        Ok(schema)
    }

    /// Get the current schema
    pub fn schema(&self) -> Option<&SchemaInfo> {
        self.schema.as_ref()
    }
}

impl Default for FlightDataDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Command encoder for Flight SQL commands
pub struct CommandEncoder;

impl CommandEncoder {
    /// Encode a Flight SQL command to bytes
    pub fn encode(command: &FlightSqlCommand) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);

        match command {
            FlightSqlCommand::StatementQuery(cmd) => {
                buf.put_u8(0x01); // Command type
                encode_string(&mut buf, &cmd.query);
                encode_optional_bytes(&mut buf, cmd.transaction_id.as_ref());
            }
            FlightSqlCommand::StatementUpdate(cmd) => {
                buf.put_u8(0x02);
                encode_string(&mut buf, &cmd.query);
                encode_optional_bytes(&mut buf, cmd.transaction_id.as_ref());
            }
            FlightSqlCommand::CreatePreparedStatement(cmd) => {
                buf.put_u8(0x10);
                encode_string(&mut buf, &cmd.query);
                encode_optional_bytes(&mut buf, cmd.transaction_id.as_ref());
            }
            FlightSqlCommand::ClosePreparedStatement(cmd) => {
                buf.put_u8(0x11);
                buf.put_slice(&cmd.prepared_statement_handle);
            }
            FlightSqlCommand::PreparedStatementQuery(cmd) => {
                buf.put_u8(0x12);
                buf.put_slice(&cmd.prepared_statement_handle);
            }
            FlightSqlCommand::PreparedStatementUpdate(cmd) => {
                buf.put_u8(0x13);
                buf.put_slice(&cmd.prepared_statement_handle);
            }
            FlightSqlCommand::GetCatalogs(_) => {
                buf.put_u8(0x20);
            }
            FlightSqlCommand::GetDbSchemas(cmd) => {
                buf.put_u8(0x21);
                encode_optional_string(&mut buf, cmd.catalog.as_deref());
                encode_optional_string(&mut buf, cmd.db_schema_filter_pattern.as_deref());
            }
            FlightSqlCommand::GetTables(cmd) => {
                buf.put_u8(0x22);
                encode_optional_string(&mut buf, cmd.catalog.as_deref());
                encode_optional_string(&mut buf, cmd.db_schema_filter_pattern.as_deref());
                encode_optional_string(&mut buf, cmd.table_name_filter_pattern.as_deref());
                buf.put_u32_le(cmd.table_types.len() as u32);
                for tt in &cmd.table_types {
                    encode_string(&mut buf, tt);
                }
                buf.put_u8(if cmd.include_schema { 1 } else { 0 });
            }
            FlightSqlCommand::GetTableTypes(_) => {
                buf.put_u8(0x23);
            }
            FlightSqlCommand::GetPrimaryKeys(cmd) => {
                buf.put_u8(0x24);
                encode_optional_string(&mut buf, cmd.catalog.as_deref());
                encode_string(&mut buf, &cmd.db_schema);
                encode_string(&mut buf, &cmd.table);
            }
            FlightSqlCommand::GetSqlInfo(cmd) => {
                buf.put_u8(0x30);
                buf.put_u32_le(cmd.info.len() as u32);
                for info in &cmd.info {
                    buf.put_u32_le(*info);
                }
            }
            FlightSqlCommand::BeginTransaction(cmd) => {
                buf.put_u8(0x40);
                buf.put_u8(match cmd.isolation_level {
                    Some(IsolationLevel::ReadUncommitted) => 1,
                    Some(IsolationLevel::ReadCommitted) => 2,
                    Some(IsolationLevel::RepeatableRead) => 3,
                    Some(IsolationLevel::Serializable) => 4,
                    Some(IsolationLevel::Snapshot) => 5,
                    None => 0,
                });
            }
            FlightSqlCommand::EndTransaction(cmd) => {
                buf.put_u8(0x41);
                buf.put_slice(&cmd.transaction_id);
                buf.put_u8(match cmd.action {
                    EndTransactionAction::Commit => 1,
                    EndTransactionAction::Rollback => 2,
                });
            }
            FlightSqlCommand::BeginSavepoint(cmd) => {
                buf.put_u8(0x42);
                buf.put_slice(&cmd.transaction_id);
                encode_string(&mut buf, &cmd.name);
            }
            FlightSqlCommand::EndSavepoint(cmd) => {
                buf.put_u8(0x43);
                buf.put_slice(&cmd.transaction_id);
                buf.put_slice(&cmd.savepoint_id);
                buf.put_u8(match cmd.action {
                    EndSavepointAction::Release => 1,
                    EndSavepointAction::Rollback => 2,
                });
            }
            FlightSqlCommand::LiveQuery(cmd) => {
                buf.put_u8(0x50);
                encode_string(&mut buf, &cmd.query);
                buf.put_u8(if cmd.diff_mode { 1 } else { 0 });
            }
            FlightSqlCommand::KillLiveQuery(cmd) => {
                buf.put_u8(0x51);
                buf.put_slice(&cmd.subscription_id);
            }
            // Encode remaining commands
            _ => {
                buf.put_u8(0xFF); // Unknown command marker
            }
        }

        buf.freeze()
    }

    /// Decode a Flight SQL command from bytes
    pub fn decode(data: &mut Bytes) -> Result<FlightSqlCommand, FlightSqlError> {
        if data.remaining() < 1 {
            return Err(FlightSqlError::invalid_argument("Empty command data"));
        }

        let command_type = data.get_u8();

        match command_type {
            0x01 => {
                let query = decode_string(data)?;
                let transaction_id = decode_optional_bytes::<16>(data)?;
                Ok(FlightSqlCommand::StatementQuery(StatementQuery {
                    query,
                    transaction_id,
                }))
            }
            0x02 => {
                let query = decode_string(data)?;
                let transaction_id = decode_optional_bytes::<16>(data)?;
                Ok(FlightSqlCommand::StatementUpdate(StatementUpdate {
                    query,
                    transaction_id,
                }))
            }
            0x10 => {
                let query = decode_string(data)?;
                let transaction_id = decode_optional_bytes::<16>(data)?;
                Ok(FlightSqlCommand::CreatePreparedStatement(
                    CreatePreparedStatement {
                        query,
                        transaction_id,
                    },
                ))
            }
            0x11 => {
                let handle = decode_bytes::<16>(data)?;
                Ok(FlightSqlCommand::ClosePreparedStatement(
                    ClosePreparedStatement {
                        prepared_statement_handle: handle,
                    },
                ))
            }
            0x12 => {
                let handle = decode_bytes::<16>(data)?;
                Ok(FlightSqlCommand::PreparedStatementQuery(
                    PreparedStatementQuery {
                        prepared_statement_handle: handle,
                    },
                ))
            }
            0x13 => {
                let handle = decode_bytes::<16>(data)?;
                Ok(FlightSqlCommand::PreparedStatementUpdate(
                    PreparedStatementUpdate {
                        prepared_statement_handle: handle,
                    },
                ))
            }
            0x20 => Ok(FlightSqlCommand::GetCatalogs(GetCatalogs {})),
            0x21 => {
                let catalog = decode_optional_string(data)?;
                let pattern = decode_optional_string(data)?;
                Ok(FlightSqlCommand::GetDbSchemas(GetDbSchemas {
                    catalog,
                    db_schema_filter_pattern: pattern,
                }))
            }
            0x30 => {
                if data.remaining() < 4 {
                    return Err(FlightSqlError::invalid_argument("Invalid GetSqlInfo"));
                }
                let count = data.get_u32_le() as usize;
                let mut info = Vec::with_capacity(count);
                for _ in 0..count {
                    if data.remaining() < 4 {
                        return Err(FlightSqlError::invalid_argument("Invalid info code"));
                    }
                    info.push(data.get_u32_le());
                }
                Ok(FlightSqlCommand::GetSqlInfo(GetSqlInfo { info }))
            }
            0x40 => {
                if data.remaining() < 1 {
                    return Err(FlightSqlError::invalid_argument("Invalid BeginTransaction"));
                }
                let level_byte = data.get_u8();
                let isolation_level = match level_byte {
                    0 => None,
                    1 => Some(IsolationLevel::ReadUncommitted),
                    2 => Some(IsolationLevel::ReadCommitted),
                    3 => Some(IsolationLevel::RepeatableRead),
                    4 => Some(IsolationLevel::Serializable),
                    5 => Some(IsolationLevel::Snapshot),
                    _ => None,
                };
                Ok(FlightSqlCommand::BeginTransaction(BeginTransaction {
                    isolation_level,
                }))
            }
            0x41 => {
                let transaction_id = decode_bytes::<16>(data)?;
                if data.remaining() < 1 {
                    return Err(FlightSqlError::invalid_argument("Invalid EndTransaction"));
                }
                let action = match data.get_u8() {
                    1 => EndTransactionAction::Commit,
                    _ => EndTransactionAction::Rollback,
                };
                Ok(FlightSqlCommand::EndTransaction(EndTransaction {
                    transaction_id,
                    action,
                }))
            }
            0x50 => {
                let query = decode_string(data)?;
                if data.remaining() < 1 {
                    return Err(FlightSqlError::invalid_argument("Invalid LiveQuery"));
                }
                let diff_mode = data.get_u8() != 0;
                Ok(FlightSqlCommand::LiveQuery(LiveQuery { query, diff_mode }))
            }
            0x51 => {
                let subscription_id = decode_bytes::<16>(data)?;
                Ok(FlightSqlCommand::KillLiveQuery(KillLiveQuery {
                    subscription_id,
                }))
            }
            _ => Err(FlightSqlError::invalid_argument(format!(
                "Unknown command type: 0x{:02x}",
                command_type
            ))),
        }
    }
}

// Helper functions for encoding

fn encode_string(buf: &mut BytesMut, s: &str) {
    let bytes = s.as_bytes();
    buf.put_u32_le(bytes.len() as u32);
    buf.put_slice(bytes);
}

fn encode_optional_string(buf: &mut BytesMut, s: Option<&str>) {
    match s {
        Some(s) => {
            buf.put_u8(1);
            encode_string(buf, s);
        }
        None => {
            buf.put_u8(0);
        }
    }
}

fn encode_optional_bytes(buf: &mut BytesMut, bytes: Option<&[u8; 16]>) {
    match bytes {
        Some(b) => {
            buf.put_u8(1);
            buf.put_slice(b);
        }
        None => {
            buf.put_u8(0);
        }
    }
}

// Helper functions for decoding

fn decode_string(data: &mut Bytes) -> Result<String, FlightSqlError> {
    if data.remaining() < 4 {
        return Err(FlightSqlError::invalid_argument("String length missing"));
    }
    let len = data.get_u32_le() as usize;
    if data.remaining() < len {
        return Err(FlightSqlError::invalid_argument("String data incomplete"));
    }
    String::from_utf8(data.copy_to_bytes(len).to_vec())
        .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 string"))
}

fn decode_optional_string(data: &mut Bytes) -> Result<Option<String>, FlightSqlError> {
    if data.remaining() < 1 {
        return Err(FlightSqlError::invalid_argument("Optional flag missing"));
    }
    if data.get_u8() == 0 {
        Ok(None)
    } else {
        decode_string(data).map(Some)
    }
}

fn decode_bytes<const N: usize>(data: &mut Bytes) -> Result<[u8; N], FlightSqlError> {
    if data.remaining() < N {
        return Err(FlightSqlError::invalid_argument("Bytes data incomplete"));
    }
    let mut arr = [0u8; N];
    data.copy_to_slice(&mut arr);
    Ok(arr)
}

fn decode_optional_bytes<const N: usize>(
    data: &mut Bytes,
) -> Result<Option<[u8; N]>, FlightSqlError> {
    if data.remaining() < 1 {
        return Err(FlightSqlError::invalid_argument("Optional flag missing"));
    }
    if data.get_u8() == 0 {
        Ok(None)
    } else {
        decode_bytes::<N>(data).map(Some)
    }
}

// Arrow type byte encoding

fn arrow_type_to_byte(t: ArrowDataType) -> u8 {
    match t {
        ArrowDataType::Null => 0,
        ArrowDataType::Boolean => 1,
        ArrowDataType::Int8 => 2,
        ArrowDataType::Int16 => 3,
        ArrowDataType::Int32 => 4,
        ArrowDataType::Int64 => 5,
        ArrowDataType::UInt8 => 6,
        ArrowDataType::UInt16 => 7,
        ArrowDataType::UInt32 => 8,
        ArrowDataType::UInt64 => 9,
        ArrowDataType::Float16 => 10,
        ArrowDataType::Float32 => 11,
        ArrowDataType::Float64 => 12,
        ArrowDataType::Utf8 => 13,
        ArrowDataType::LargeUtf8 => 14,
        ArrowDataType::Binary => 15,
        ArrowDataType::LargeBinary => 16,
        ArrowDataType::Date32 => 17,
        ArrowDataType::Date64 => 18,
        ArrowDataType::Time32Millisecond => 19,
        ArrowDataType::Time32Second => 20,
        ArrowDataType::Time64Microsecond => 21,
        ArrowDataType::Time64Nanosecond => 22,
        ArrowDataType::TimestampSecond => 23,
        ArrowDataType::TimestampMillisecond => 24,
        ArrowDataType::TimestampMicrosecond => 25,
        ArrowDataType::TimestampNanosecond => 26,
        ArrowDataType::Duration => 27,
        ArrowDataType::Interval => 28,
        ArrowDataType::List => 29,
        ArrowDataType::LargeList => 30,
        ArrowDataType::FixedSizeList => 31,
        ArrowDataType::Struct => 32,
        ArrowDataType::Map => 33,
        ArrowDataType::Union => 34,
        ArrowDataType::Dictionary => 35,
        ArrowDataType::Decimal128 => 36,
        ArrowDataType::Decimal256 => 37,
        ArrowDataType::FixedSizeBinary => 38,
        ArrowDataType::Point => 100,
        ArrowDataType::LineString => 101,
        ArrowDataType::Polygon => 102,
        ArrowDataType::MultiPoint => 103,
        ArrowDataType::MultiLineString => 104,
        ArrowDataType::MultiPolygon => 105,
        ArrowDataType::Geometry => 106,
        ArrowDataType::FixedSizeVector => 110,
    }
}

fn byte_to_arrow_type(b: u8) -> ArrowDataType {
    match b {
        0 => ArrowDataType::Null,
        1 => ArrowDataType::Boolean,
        2 => ArrowDataType::Int8,
        3 => ArrowDataType::Int16,
        4 => ArrowDataType::Int32,
        5 => ArrowDataType::Int64,
        6 => ArrowDataType::UInt8,
        7 => ArrowDataType::UInt16,
        8 => ArrowDataType::UInt32,
        9 => ArrowDataType::UInt64,
        10 => ArrowDataType::Float16,
        11 => ArrowDataType::Float32,
        12 => ArrowDataType::Float64,
        13 => ArrowDataType::Utf8,
        14 => ArrowDataType::LargeUtf8,
        15 => ArrowDataType::Binary,
        16 => ArrowDataType::LargeBinary,
        17 => ArrowDataType::Date32,
        18 => ArrowDataType::Date64,
        19 => ArrowDataType::Time32Millisecond,
        20 => ArrowDataType::Time32Second,
        21 => ArrowDataType::Time64Microsecond,
        22 => ArrowDataType::Time64Nanosecond,
        23 => ArrowDataType::TimestampSecond,
        24 => ArrowDataType::TimestampMillisecond,
        25 => ArrowDataType::TimestampMicrosecond,
        26 => ArrowDataType::TimestampNanosecond,
        27 => ArrowDataType::Duration,
        28 => ArrowDataType::Interval,
        29 => ArrowDataType::List,
        30 => ArrowDataType::LargeList,
        31 => ArrowDataType::FixedSizeList,
        32 => ArrowDataType::Struct,
        33 => ArrowDataType::Map,
        34 => ArrowDataType::Union,
        35 => ArrowDataType::Dictionary,
        36 => ArrowDataType::Decimal128,
        37 => ArrowDataType::Decimal256,
        38 => ArrowDataType::FixedSizeBinary,
        100 => ArrowDataType::Point,
        101 => ArrowDataType::LineString,
        102 => ArrowDataType::Polygon,
        103 => ArrowDataType::MultiPoint,
        104 => ArrowDataType::MultiLineString,
        105 => ArrowDataType::MultiPolygon,
        106 => ArrowDataType::Geometry,
        110 => ArrowDataType::FixedSizeVector,
        _ => ArrowDataType::Utf8, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_encode_decode() {
        let schema = SchemaInfo::new(vec![
            FieldInfo::new("id", ArrowDataType::Int64, false),
            FieldInfo::new("name", ArrowDataType::Utf8, true),
            FieldInfo::new("score", ArrowDataType::Float64, true),
        ]);

        let encoder = FlightDataEncoder::new(schema.clone());
        let encoded = encoder.encode_schema();

        let mut decoder = FlightDataDecoder::new();
        let mut data = encoded;
        let decoded = decoder.decode_schema(&mut data).unwrap();

        assert_eq!(decoded.fields.len(), 3);
        assert_eq!(decoded.fields[0].name, "id");
        assert_eq!(decoded.fields[0].data_type, ArrowDataType::Int64);
        assert!(!decoded.fields[0].nullable);
    }

    #[test]
    fn test_command_encode_decode() {
        let cmd = FlightSqlCommand::StatementQuery(StatementQuery {
            query: "SELECT * FROM users".to_string(),
            transaction_id: None,
        });

        let encoded = CommandEncoder::encode(&cmd);
        let mut data = encoded;
        let decoded = CommandEncoder::decode(&mut data).unwrap();

        match decoded {
            FlightSqlCommand::StatementQuery(q) => {
                assert_eq!(q.query, "SELECT * FROM users");
                assert!(q.transaction_id.is_none());
            }
            _ => panic!("Wrong command type decoded"),
        }
    }

    #[test]
    fn test_live_query_encode_decode() {
        let cmd = FlightSqlCommand::LiveQuery(LiveQuery {
            query: "LIVE SELECT * FROM orders WHERE status = 'pending'".to_string(),
            diff_mode: true,
        });

        let encoded = CommandEncoder::encode(&cmd);
        let mut data = encoded;
        let decoded = CommandEncoder::decode(&mut data).unwrap();

        match decoded {
            FlightSqlCommand::LiveQuery(q) => {
                assert!(q.query.contains("LIVE SELECT"));
                assert!(q.diff_mode);
            }
            _ => panic!("Wrong command type decoded"),
        }
    }
}
