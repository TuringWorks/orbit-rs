//! MySQL Unit Tests
//!
//! Unit tests for MySQL protocol components

use orbit_protocols::mysql::auth::{AuthPlugin, AuthState, MySqlAuth};
use orbit_protocols::mysql::protocol::{map_error_to_mysql_code, error_codes};
use orbit_protocols::mysql::types::MySqlType;
use orbit_protocols::error::ProtocolError;

// ============================================================================
// Authentication Tests
// ============================================================================

#[test]
fn test_auth_plugin_conversion() {
    assert_eq!(AuthPlugin::NativePassword.as_str(), "mysql_native_password");
    assert_eq!(AuthPlugin::ClearPassword.as_str(), "mysql_clear_password");
    assert_eq!(AuthPlugin::CachingSha2Password.as_str(), "caching_sha2_password");
    
    assert_eq!(
        AuthPlugin::from_str("mysql_native_password"),
        Some(AuthPlugin::NativePassword)
    );
    assert_eq!(
        AuthPlugin::from_str("mysql_clear_password"),
        Some(AuthPlugin::ClearPassword)
    );
    assert_eq!(
        AuthPlugin::from_str("caching_sha2_password"),
        Some(AuthPlugin::CachingSha2Password)
    );
    assert_eq!(AuthPlugin::from_str("unknown"), None);
}

#[test]
fn test_auth_state_transitions() {
    let auth = MySqlAuth::new(AuthPlugin::NativePassword);
    assert_eq!(auth.state(), &AuthState::AwaitingHandshake);
}

#[test]
fn test_auth_with_credentials() {
    let auth = MySqlAuth::with_credentials(
        AuthPlugin::NativePassword,
        Some("admin".to_string()),
        Some("password123".to_string()),
    );
    assert_eq!(auth.state(), &AuthState::AwaitingHandshake);
}

#[test]
fn test_native_password_hash() {
    let password = "test123";
    let auth_data = [0u8; 20];
    let hash = MySqlAuth::compute_native_password_hash(password, &auth_data);
    assert_eq!(hash.len(), 32); // SHA256 produces 32 bytes
}

#[test]
fn test_clear_password_validation() {
    let auth = MySqlAuth::with_credentials(
        AuthPlugin::ClearPassword,
        Some("admin".to_string()),
        Some("password123".to_string()),
    );
    
    // Test with correct password
    let correct_auth = b"password123\0";
    assert!(auth.validate_clear_password(correct_auth));
    
    // Test with incorrect password
    let incorrect_auth = b"wrongpassword\0";
    assert!(!auth.validate_clear_password(incorrect_auth));
    
    // Test with no null terminator
    let no_null = b"password123";
    assert!(auth.validate_clear_password(no_null));
    
    // Test with empty auth (should fail when password is set)
    let empty_auth = b"";
    assert!(!auth.validate_clear_password(empty_auth));
}

// ============================================================================
// Error Code Mapping Tests
// ============================================================================

#[test]
fn test_error_code_mapping() {
    // Parse errors
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::ParseError("test".to_string())),
        error_codes::ER_PARSE_ERROR
    );
    
    // Table not found
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("table does not exist".to_string())),
        error_codes::ER_NO_SUCH_TABLE
    );
    
    // Column not found
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("column not found".to_string())),
        error_codes::ER_BAD_FIELD
    );
    
    // Duplicate entry
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("duplicate entry".to_string())),
        error_codes::ER_DUP_ENTRY
    );
    
    // Authentication error
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::AuthenticationError("access denied".to_string())),
        error_codes::ER_ACCESS_DENIED
    );
    
    // Invalid opcode
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::InvalidOpcode(0xFF)),
        error_codes::ER_UNKNOWN_COM_ERROR
    );
    
    // Invalid statement
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::InvalidStatement("test".to_string())),
        error_codes::ER_PARSE_ERROR
    );
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[test]
fn test_mysql_type_from_u8() {
    assert_eq!(MySqlType::from_u8(0x01), Some(MySqlType::Tiny));
    assert_eq!(MySqlType::from_u8(0x03), Some(MySqlType::Long));
    assert_eq!(MySqlType::from_u8(0x08), Some(MySqlType::LongLong));
    assert_eq!(MySqlType::from_u8(0x0F), Some(MySqlType::VarChar));
    assert_eq!(MySqlType::from_u8(0xFF), Some(MySqlType::Geometry));
    assert_eq!(MySqlType::from_u8(0x99), None); // Invalid type
}

#[test]
fn test_mysql_type_to_sql_type() {
    use orbit_protocols::postgres_wire::sql::types::SqlType;
    
    assert_eq!(MySqlType::Tiny.to_sql_type(), SqlType::SmallInt);
    assert_eq!(MySqlType::Long.to_sql_type(), SqlType::Integer);
    assert_eq!(MySqlType::LongLong.to_sql_type(), SqlType::BigInt);
    assert_eq!(MySqlType::Float.to_sql_type(), SqlType::Real);
    assert_eq!(MySqlType::Double.to_sql_type(), SqlType::DoublePrecision);
    assert_eq!(MySqlType::VarChar.to_sql_type(), SqlType::Text);
}

// ============================================================================
// Parameter Counting Tests
// ============================================================================

#[test]
fn test_count_parameters() {
    use orbit_protocols::mysql::adapter::MySqlAdapter;
    
    // No parameters
    assert_eq!(MySqlAdapter::count_parameters("SELECT * FROM users"), 0);
    
    // Single parameter
    assert_eq!(MySqlAdapter::count_parameters("SELECT * FROM users WHERE id = ?"), 1);
    
    // Multiple parameters
    assert_eq!(
        MySqlAdapter::count_parameters("INSERT INTO users (id, name) VALUES (?, ?)"),
        2
    );
    
    // Many parameters
    assert_eq!(
        MySqlAdapter::count_parameters("SELECT * FROM users WHERE id IN (?, ?, ?, ?, ?)"),
        5
    );
    
    // Parameter in string literal (simple implementation counts all '?')
    // Note: In a full implementation, we'd parse SQL properly to ignore string literals
    // For now, the simple implementation counts all '?' characters, including in strings
    assert_eq!(
        MySqlAdapter::count_parameters("SELECT * FROM users WHERE name = 'test?'"),
        1  // Simple implementation counts the '?' even in string literal
    );
    
    // Edge case: multiple question marks
    assert_eq!(
        MySqlAdapter::count_parameters("SELECT ?, ?, ? FROM users"),
        3
    );
}

// ============================================================================
// Enhanced Error Code Mapping Tests
// ============================================================================

#[test]
fn test_map_error_to_mysql_code_enhanced() {
    use orbit_protocols::mysql::protocol::{map_error_to_mysql_code, error_codes};
    use orbit_protocols::error::ProtocolError;
    
    // Test new error codes
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("database not found".to_string())),
        error_codes::ER_BAD_DB_ERROR
    );
    
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("wrong value count".to_string())),
        error_codes::ER_WRONG_VALUE_COUNT
    );
    
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("database access denied".to_string())),
        error_codes::ER_DBACCESS_DENIED_ERROR
    );
    
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("table access denied".to_string())),
        error_codes::ER_TABLEACCESS_DENIED_ERROR
    );
    
    assert_eq!(
        map_error_to_mysql_code(&ProtocolError::PostgresError("column access denied".to_string())),
        error_codes::ER_COLUMNACCESS_DENIED_ERROR
    );
}

