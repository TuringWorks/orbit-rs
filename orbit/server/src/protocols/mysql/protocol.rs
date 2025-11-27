//! MySQL protocol commands and responses

use super::packet::{write_lenenc_int, write_lenenc_string, write_null_string};
use super::types::MySqlType;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::{BufMut, Bytes, BytesMut};

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
    /// COM_SET_OPTION
    SetOption = 0x1B,
    /// COM_RESET_CONNECTION
    ResetConnection = 0x1F,
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
            0x1B => Ok(MySqlCommand::SetOption),
            0x1F => Ok(MySqlCommand::ResetConnection),
            _ => Err(ProtocolError::InvalidOpcode(byte)),
        }
    }
}

/// MySQL error codes (from MySQL 8.0 error reference)
pub mod error_codes {
    /// Access denied for user
    pub const ER_ACCESS_DENIED: u16 = 1045;
    /// No database selected
    pub const ER_NO_DB: u16 = 1046;
    /// Unknown command
    pub const ER_UNKNOWN_COM_ERROR: u16 = 1047;
    /// Bad database error
    pub const ER_BAD_DB_ERROR: u16 = 1049;
    /// Table already exists
    pub const ER_TABLE_EXISTS: u16 = 1050;
    /// Unknown table
    pub const ER_BAD_TABLE: u16 = 1051;
    /// Unknown column
    pub const ER_BAD_FIELD: u16 = 1054;
    /// Wrong value count
    pub const ER_WRONG_VALUE_COUNT: u16 = 1058;
    /// Duplicate entry
    pub const ER_DUP_ENTRY: u16 = 1062;
    /// Syntax error
    pub const ER_PARSE_ERROR: u16 = 1064;
    /// Syntax error (alias)
    pub const ER_SYNTAX_ERROR: u16 = 1064;
    /// Database access denied
    pub const ER_DBACCESS_DENIED_ERROR: u16 = 1044;
    /// Table access denied
    pub const ER_TABLEACCESS_DENIED_ERROR: u16 = 1142;
    /// Column access denied
    pub const ER_COLUMNACCESS_DENIED_ERROR: u16 = 1143;
    /// Illegal grant for table
    pub const ER_ILLEGAL_GRANT_FOR_TABLE: u16 = 1144;
    /// Non-existing grant
    pub const ER_NONEXISTING_GRANT: u16 = 1145;
    /// Table doesn't exist
    pub const ER_NO_SUCH_TABLE: u16 = 1146;
    /// Cannot user
    pub const ER_CANNOT_USER: u16 = 1396;
    /// Too many user connections
    pub const ER_TOO_MANY_USER_CONNECTIONS: u16 = 1203;
    /// Unknown MySQL error
    pub const ER_UNKNOWN_ERROR: u16 = 2000;
}

/// Map ProtocolError to MySQL error code
pub fn map_error_to_mysql_code(error: &crate::protocols::error::ProtocolError) -> u16 {
    use crate::protocols::error::ProtocolError;
    use error_codes::*;

    match error {
        ProtocolError::ParseError(_) => ER_PARSE_ERROR,
        ProtocolError::PostgresError(msg) => {
            // Check for specific SQL errors
            let msg_lower = msg.to_lowercase();
            if msg_lower.contains("does not exist") || msg_lower.contains("not found") {
                if msg_lower.contains("table") {
                    ER_NO_SUCH_TABLE
                } else if msg_lower.contains("column") || msg_lower.contains("field") {
                    ER_BAD_FIELD
                } else if msg_lower.contains("database") || msg_lower.contains("schema") {
                    ER_BAD_DB_ERROR
                } else {
                    ER_BAD_TABLE
                }
            } else if msg_lower.contains("already exists") || msg_lower.contains("duplicate") {
                ER_DUP_ENTRY
            } else if msg_lower.contains("syntax") || msg_lower.contains("parse") {
                ER_SYNTAX_ERROR
            } else if msg_lower.contains("table exists") {
                ER_TABLE_EXISTS
            } else if msg_lower.contains("wrong value count") || msg_lower.contains("column count")
            {
                ER_WRONG_VALUE_COUNT
            } else if msg_lower.contains("access denied") || msg_lower.contains("permission") {
                if msg_lower.contains("database") {
                    ER_DBACCESS_DENIED_ERROR
                } else if msg_lower.contains("table") {
                    ER_TABLEACCESS_DENIED_ERROR
                } else if msg_lower.contains("column") {
                    ER_COLUMNACCESS_DENIED_ERROR
                } else {
                    ER_ACCESS_DENIED
                }
            } else {
                ER_UNKNOWN_ERROR
            }
        }
        ProtocolError::AuthenticationError(_) => ER_ACCESS_DENIED,
        ProtocolError::AuthorizationError(_) => ER_ACCESS_DENIED,
        ProtocolError::InvalidOpcode(_) => ER_UNKNOWN_COM_ERROR,
        ProtocolError::IncompleteFrame => ER_UNKNOWN_ERROR,
        ProtocolError::InvalidUtf8(_) => ER_PARSE_ERROR,
        ProtocolError::InvalidStatement(_) => ER_PARSE_ERROR,
        ProtocolError::ConnectionError(_) => ER_UNKNOWN_ERROR,
        ProtocolError::ConnectionClosed => ER_UNKNOWN_ERROR,
        _ => ER_UNKNOWN_ERROR,
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

    /// Build ERROR packet from a ProtocolError
    pub fn error_from_protocol_error(error: &crate::protocols::error::ProtocolError) -> Bytes {
        let error_code = map_error_to_mysql_code(error);
        let mut message = error.to_string();

        // Sanitize error messages: Remove "PostgreSQL protocol error" and replace with MySQL-appropriate text
        message = message.replace("PostgreSQL protocol error: ", "");
        message = message.replace("PostgreSQL protocol error", "MySQL error");
        message = message.replace("PostgresError", "MySQL error");

        Self::error(error_code, &message)
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
    buf.put_u16_le(
        ((CLIENT_PROTOCOL_41 | CLIENT_SECURE_CONNECTION | CLIENT_PLUGIN_AUTH) >> 16) as u16,
    );

    // Auth plugin data length
    buf.put_u8(21);

    // Reserved (10 bytes)
    buf.put(&[0u8; 10][..]);

    // Auth plugin data part 2 (13 bytes)
    buf.put(
        &[
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0x00,
        ][..],
    );

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
