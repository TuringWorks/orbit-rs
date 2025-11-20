//! MySQL protocol commands and responses

use super::packet::{write_lenenc_int, write_lenenc_string, write_null_string};
use super::types::MySqlType;
use bytes::{BufMut, Bytes, BytesMut};
use crate::error::{ProtocolError, ProtocolResult};

/// MySQL command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MySqlCommand {
    /// COM_SLEEP
    Sleep = 0x00,
    /// COM_QUIT
    Quit = 0x01,
    /// COM_INIT_DB
    InitDb = 0x02,
    /// COM_QUERY
    Query = 0x03,
    /// COM_FIELD_LIST
    FieldList = 0x04,
    /// COM_CREATE_DB
    CreateDb = 0x05,
    /// COM_DROP_DB
    DropDb = 0x06,
    /// COM_REFRESH
    Refresh = 0x07,
    /// COM_PING
    Ping = 0x0E,
    /// COM_STATISTICS
    Statistics = 0x09,
    /// COM_STMT_PREPARE
    StmtPrepare = 0x16,
    /// COM_STMT_EXECUTE
    StmtExecute = 0x17,
    /// COM_STMT_CLOSE
    StmtClose = 0x19,
    /// COM_STMT_RESET
    StmtReset = 0x1A,
}

impl MySqlCommand {
    pub fn from_u8(byte: u8) -> ProtocolResult<Self> {
        match byte {
            0x00 => Ok(MySqlCommand::Sleep),
            0x01 => Ok(MySqlCommand::Quit),
            0x02 => Ok(MySqlCommand::InitDb),
            0x03 => Ok(MySqlCommand::Query),
            0x04 => Ok(MySqlCommand::FieldList),
            0x05 => Ok(MySqlCommand::CreateDb),
            0x06 => Ok(MySqlCommand::DropDb),
            0x07 => Ok(MySqlCommand::Refresh),
            0x0E => Ok(MySqlCommand::Ping),
            0x16 => Ok(MySqlCommand::StmtPrepare),
            0x17 => Ok(MySqlCommand::StmtExecute),
            0x19 => Ok(MySqlCommand::StmtClose),
            0x1A => Ok(MySqlCommand::StmtReset),
            _ => Err(ProtocolError::InvalidOpcode(byte)),
        }
    }
}

/// MySQL packet types
pub struct MySqlPacket;

impl MySqlPacket {
    /// Build OK packet
    pub fn ok(affected_rows: u64, last_insert_id: u64) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(0x00); // OK packet header
        write_lenenc_int(&mut buf, affected_rows);
        write_lenenc_int(&mut buf, last_insert_id);
        buf.put_u16_le(0x0002); // Server status (autocommit)
        buf.put_u16_le(0); // Warnings
        buf.freeze()
    }

    /// Build ERROR packet
    pub fn error(error_code: u16, message: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(0xFF); // ERR packet header
        buf.put_u16_le(error_code);
        buf.put_u8(b'#'); // SQL state marker
        buf.put(&b"HY000"[..]); // Generic SQL state
        buf.put(message.as_bytes());
        buf.freeze()
    }

    /// Build EOF packet
    pub fn eof() -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(0xFE); // EOF packet header
        buf.put_u16_le(0); // Warnings
        buf.put_u16_le(0x0002); // Server status (autocommit)
        buf.freeze()
    }

    /// Build column definition packet
    pub fn column_definition(
        catalog: &str,
        schema: &str,
        table: &str,
        org_table: &str,
        name: &str,
        org_name: &str,
        column_type: MySqlType,
    ) -> Bytes {
        let mut buf = BytesMut::new();

        // Catalog
        write_lenenc_string(&mut buf, catalog);
        // Schema
        write_lenenc_string(&mut buf, schema);
        // Table alias
        write_lenenc_string(&mut buf, table);
        // Org table
        write_lenenc_string(&mut buf, org_table);
        // Column alias
        write_lenenc_string(&mut buf, name);
        // Org column name
        write_lenenc_string(&mut buf, org_name);

        // Length of fixed fields (always 0x0C)
        write_lenenc_int(&mut buf, 0x0C);

        // Character set (utf8mb4 = 0x21)
        buf.put_u16_le(0x21);
        // Column length
        buf.put_u32_le(255);
        // Column type
        buf.put_u8(column_type as u8);
        // Flags
        buf.put_u16_le(0);
        // Decimals
        buf.put_u8(0);
        // Filler
        buf.put_u16(0);

        buf.freeze()
    }

    /// Build result set row (text protocol)
    pub fn text_row(values: &[Option<String>]) -> Bytes {
        let mut buf = BytesMut::new();

        for value in values {
            match value {
                Some(v) => write_lenenc_string(&mut buf, v),
                None => buf.put_u8(0xFB), // NULL
            }
        }

        buf.freeze()
    }
}

/// Server capabilities
pub const CLIENT_PROTOCOL_41: u32 = 0x00000200;
pub const CLIENT_SECURE_CONNECTION: u32 = 0x00008000;
pub const CLIENT_PLUGIN_AUTH: u32 = 0x00080000;
pub const CLIENT_CONNECT_WITH_DB: u32 = 0x00000008;

/// Build initial handshake packet
pub fn build_handshake(connection_id: u32, server_version: &str) -> Bytes {
    let mut buf = BytesMut::new();

    // Protocol version
    buf.put_u8(10);

    // Server version (null-terminated)
    write_null_string(&mut buf, server_version);

    // Connection ID
    buf.put_u32_le(connection_id);

    // Auth plugin data part 1 (8 bytes)
    buf.put_u64(0x1122334455667788);

    // Filler
    buf.put_u8(0);

    // Capability flags (lower 2 bytes)
    buf.put_u16_le((CLIENT_PROTOCOL_41 | CLIENT_SECURE_CONNECTION) as u16);

    // Character set (utf8mb4)
    buf.put_u8(0x21);

    // Status flags
    buf.put_u16_le(0x0002);

    // Capability flags (upper 2 bytes)
    buf.put_u16_le(((CLIENT_PROTOCOL_41 | CLIENT_SECURE_CONNECTION | CLIENT_PLUGIN_AUTH) >> 16) as u16);

    // Auth plugin data length
    buf.put_u8(21);

    // Reserved (10 bytes)
    buf.put(&[0u8; 10][..]);

    // Auth plugin data part 2 (13 bytes)
    buf.put(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0x00][..]);

    // Auth plugin name
    write_null_string(&mut buf, "mysql_native_password");

    buf.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_packet() {
        let packet = MySqlPacket::ok(1, 0);
        assert_eq!(packet[0], 0x00); // OK header
    }

    #[test]
    fn test_error_packet() {
        let packet = MySqlPacket::error(1064, "Syntax error");
        assert_eq!(packet[0], 0xFF); // ERR header
    }

    #[test]
    fn test_command_conversion() {
        assert_eq!(MySqlCommand::from_u8(0x03).unwrap(), MySqlCommand::Query);
        assert_eq!(MySqlCommand::from_u8(0x01).unwrap(), MySqlCommand::Quit);
    }
}
