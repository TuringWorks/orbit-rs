//! Integration tests for CQL and MySQL protocol adapters
//!
//! These tests verify that both protocol adapters can be instantiated,
//! configured, and integrated with the Orbit engine.

#[cfg(test)]
mod cql_tests {
    use orbit_protocols::cql::{CqlAdapter, CqlConfig};

    #[tokio::test]
    async fn test_cql_adapter_creation() {
        let config = CqlConfig {
            listen_addr: "127.0.0.1:19042".parse().unwrap(), // Use different port for testing
            max_connections: 100,
            authentication_enabled: false,
            protocol_version: 4,
            username: None,
            password: None,
        };

        let adapter = CqlAdapter::new(config).await;
        assert!(
            adapter.is_ok(),
            "CQL adapter should be created successfully"
        );
    }

    #[tokio::test]
    async fn test_cql_config_defaults() {
        let config = CqlConfig::default();

        assert_eq!(config.listen_addr.port(), 9042);
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.authentication_enabled, false);
        assert_eq!(config.protocol_version, 4);
    }

    #[test]
    fn test_cql_parser_basic() {
        use orbit_protocols::cql::{CqlParser, CqlStatement};

        let parser = CqlParser::new();

        // Test simple SELECT
        let result = parser.parse("SELECT * FROM users");
        assert!(result.is_ok());

        if let Ok(CqlStatement::Select { table, .. }) = result {
            assert_eq!(table, "users");
        }

        // Test simple INSERT
        let result = parser.parse("INSERT INTO users (id, name) VALUES (1, 'Alice')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cql_type_conversions() {
        use orbit_protocols::cql::CqlValue;
        use orbit_protocols::postgres_wire::sql::types::SqlValue;

        // Test basic type conversions
        let cql_int = CqlValue::Int(42);
        let sql_val = cql_int.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::Integer(42)));

        let cql_text = CqlValue::Text("hello".to_string());
        let sql_val = cql_text.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::Text(_)));

        let cql_bool = CqlValue::Boolean(true);
        let sql_val = cql_bool.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::Boolean(true)));
    }
}

#[cfg(test)]
mod mysql_tests {
    use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};

    #[tokio::test]
    async fn test_mysql_adapter_creation() {
        let config = MySqlConfig {
            listen_addr: "127.0.0.1:13306".parse().unwrap(), // Use different port for testing
            max_connections: 100,
            authentication_enabled: false,
            server_version: "8.0.0-Orbit-Test".to_string(),
            username: None,
            password: None,
        };

        let adapter = MySqlAdapter::new(config).await;
        assert!(
            adapter.is_ok(),
            "MySQL adapter should be created successfully"
        );
    }

    #[tokio::test]
    async fn test_mysql_config_defaults() {
        let config = MySqlConfig::default();

        assert_eq!(config.listen_addr.port(), 3306);
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.authentication_enabled, false);
        assert_eq!(config.server_version, "Orbit-DB 1.0.0 (MySQL-compatible)");
    }

    #[test]
    fn test_mysql_type_conversions() {
        use orbit_protocols::mysql::MySqlValue;
        use orbit_protocols::postgres_wire::sql::types::SqlValue;

        // Test basic type conversions
        let mysql_int = MySqlValue::Int(42);
        let sql_val = mysql_int.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::Integer(42)));

        let mysql_string = MySqlValue::String("hello".to_string());
        let sql_val = mysql_string.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::Text(_)));

        let mysql_double = MySqlValue::Double(3.14);
        let sql_val = mysql_double.to_sql_value().unwrap();
        assert!(matches!(sql_val, SqlValue::DoublePrecision(_)));
    }

    #[test]
    fn test_mysql_packet_encoding() {
        use bytes::Bytes;
        use orbit_protocols::mysql::packet::MySqlPacket;

        let packet = MySqlPacket::new(1, Bytes::from("test payload"));
        let encoded = packet.encode();

        // Check header
        assert_eq!(encoded.len(), 4 + "test payload".len());
        assert_eq!(encoded[3], 1); // sequence_id
    }

    #[test]
    fn test_mysql_command_conversion() {
        use orbit_protocols::mysql::protocol::MySqlCommand;

        assert_eq!(MySqlCommand::from_u8(0x03).unwrap(), MySqlCommand::Query);
        assert_eq!(MySqlCommand::from_u8(0x01).unwrap(), MySqlCommand::Quit);
        assert_eq!(MySqlCommand::from_u8(0x0E).unwrap(), MySqlCommand::Ping);
        assert!(MySqlCommand::from_u8(0xFF).is_err());
    }
}

#[cfg(test)]
mod integration_tests {
    use orbit_protocols::cql::{CqlAdapter, CqlConfig};
    use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};

    #[tokio::test]
    async fn test_both_adapters_can_coexist() {
        // Test that both adapters can be created simultaneously
        let cql_config = CqlConfig {
            listen_addr: "127.0.0.1:19043".parse().unwrap(),
            max_connections: 100,
            authentication_enabled: false,
            protocol_version: 4,
            username: None,
            password: None,
        };

        let mysql_config = MySqlConfig {
            listen_addr: "127.0.0.1:13307".parse().unwrap(),
            max_connections: 100,
            authentication_enabled: false,
            server_version: "8.0.0-Orbit-Test".to_string(),
            username: None,
            password: None,
        };

        let cql_adapter = CqlAdapter::new(cql_config).await;
        let mysql_adapter = MySqlAdapter::new(mysql_config).await;

        assert!(cql_adapter.is_ok(), "CQL adapter should be created");
        assert!(mysql_adapter.is_ok(), "MySQL adapter should be created");
    }

    #[test]
    fn test_error_types_are_compatible() {
        use orbit_protocols::error::ProtocolError;

        // Test that error types work correctly
        let err1 = ProtocolError::CqlError("test error".to_string());
        assert!(err1.to_string().contains("CQL protocol error"));

        let err2 = ProtocolError::ConnectionClosed;
        assert!(err2.to_string().contains("Connection closed"));

        let err3 = ProtocolError::InvalidOpcode(0xFF);
        assert!(err3.to_string().contains("Invalid opcode"));
    }
}
