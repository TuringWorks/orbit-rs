// Parse Functions Module
//
// Implements parsing functions for OrbitQL:
// - parse::email::user() - Extract user part from email
// - parse::email::domain() - Extract domain from email
// - parse::url::domain() - Extract domain from URL
// - parse::url::fragment() - Extract fragment from URL
// - parse::url::host() - Extract host from URL
// - parse::url::path() - Extract path from URL
// - parse::url::port() - Extract port from URL
// - parse::url::query() - Extract query string from URL

use regex::Regex;
use serde_json::{json, Value};
use url::Url;

use crate::protocols::{ProtocolError, ProtocolResult};

lazy_static::lazy_static! {
    static ref EMAIL_REGEX: Regex = Regex::new(r"^([^@]+)@([^@]+)$").unwrap();
}

/// Extract user part from email address
///
/// Usage: parse::email::user('john.doe@example.com')
/// Returns: 'john.doe'
pub fn email_user(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::email::user() expects 1 argument (email)".to_string(),
        ));
    }

    let email = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Email must be a string".to_string()))?;

    if let Some(captures) = EMAIL_REGEX.captures(email) {
        let user = captures.get(1).unwrap().as_str();
        Ok(json!(user))
    } else {
        Ok(Value::Null)
    }
}

/// Extract domain part from email address
///
/// Usage: parse::email::domain('john.doe@example.com')
/// Returns: 'example.com'
pub fn email_domain(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::email::domain() expects 1 argument (email)".to_string(),
        ));
    }

    let email = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Email must be a string".to_string()))?;

    if let Some(captures) = EMAIL_REGEX.captures(email) {
        let domain = captures.get(2).unwrap().as_str();
        Ok(json!(domain))
    } else {
        Ok(Value::Null)
    }
}

/// Extract domain from URL
///
/// Usage: parse::url::domain('https://www.example.com:8080/path?query=1#fragment')
/// Returns: 'example.com'
pub fn url_domain(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::domain() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    if let Some(domain) = url.domain() {
        Ok(json!(domain))
    } else {
        Ok(Value::Null)
    }
}

/// Extract fragment from URL
///
/// Usage: parse::url::fragment('https://example.com/path#section1')
/// Returns: 'section1'
pub fn url_fragment(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::fragment() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    if let Some(fragment) = url.fragment() {
        Ok(json!(fragment))
    } else {
        Ok(Value::Null)
    }
}

/// Extract host from URL
///
/// Usage: parse::url::host('https://www.example.com:8080/path')
/// Returns: 'www.example.com'
pub fn url_host(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::host() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    if let Some(host) = url.host_str() {
        Ok(json!(host))
    } else {
        Ok(Value::Null)
    }
}

/// Extract path from URL
///
/// Usage: parse::url::path('https://example.com/api/v1/users?id=123')
/// Returns: '/api/v1/users'
pub fn url_path(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::path() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    Ok(json!(url.path()))
}

/// Extract port from URL
///
/// Usage: parse::url::port('https://example.com:8080/path')
/// Returns: 8080
pub fn url_port(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::port() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    if let Some(port) = url.port() {
        Ok(json!(port))
    } else {
        Ok(Value::Null)
    }
}

/// Extract query string from URL
///
/// Usage: parse::url::query('https://example.com/path?key1=value1&key2=value2')
/// Returns: 'key1=value1&key2=value2'
pub fn url_query(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "parse::url::query() expects 1 argument (url)".to_string(),
        ));
    }

    let url_str = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("URL must be a string".to_string()))?;

    let url = Url::parse(url_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid URL: {}", e)))?;

    if let Some(query) = url.query() {
        Ok(json!(query))
    } else {
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_user() {
        assert_eq!(
            email_user(&[json!("john.doe@example.com")]).unwrap(),
            json!("john.doe")
        );
        assert_eq!(
            email_user(&[json!("admin@company.co.uk")]).unwrap(),
            json!("admin")
        );
        assert_eq!(email_user(&[json!("invalid")]).unwrap(), Value::Null);
    }

    #[test]
    fn test_email_domain() {
        assert_eq!(
            email_domain(&[json!("john.doe@example.com")]).unwrap(),
            json!("example.com")
        );
        assert_eq!(
            email_domain(&[json!("admin@company.co.uk")]).unwrap(),
            json!("company.co.uk")
        );
        assert_eq!(email_domain(&[json!("invalid")]).unwrap(), Value::Null);
    }

    #[test]
    fn test_url_domain() {
        assert_eq!(
            url_domain(&[json!("https://www.example.com:8080/path")]).unwrap(),
            json!("www.example.com")
        );
        assert_eq!(
            url_domain(&[json!("http://example.com")]).unwrap(),
            json!("example.com")
        );
    }

    #[test]
    fn test_url_fragment() {
        assert_eq!(
            url_fragment(&[json!("https://example.com/path#section1")]).unwrap(),
            json!("section1")
        );
        assert_eq!(
            url_fragment(&[json!("https://example.com/path")]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_url_host() {
        assert_eq!(
            url_host(&[json!("https://www.example.com:8080/path")]).unwrap(),
            json!("www.example.com")
        );
        assert_eq!(
            url_host(&[json!("http://localhost:3000")]).unwrap(),
            json!("localhost")
        );
    }

    #[test]
    fn test_url_path() {
        assert_eq!(
            url_path(&[json!("https://example.com/api/v1/users?id=123")]).unwrap(),
            json!("/api/v1/users")
        );
        assert_eq!(
            url_path(&[json!("https://example.com/")]).unwrap(),
            json!("/")
        );
    }

    #[test]
    fn test_url_port() {
        assert_eq!(
            url_port(&[json!("https://example.com:8080/path")]).unwrap(),
            json!(8080)
        );
        assert_eq!(
            url_port(&[json!("http://example.com/path")]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_url_query() {
        assert_eq!(
            url_query(&[json!("https://example.com/path?key1=value1&key2=value2")]).unwrap(),
            json!("key1=value1&key2=value2")
        );
        assert_eq!(
            url_query(&[json!("https://example.com/path")]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_error_handling() {
        // Wrong number of arguments
        assert!(email_user(&[]).is_err());
        assert!(url_domain(&[]).is_err());

        // Non-string arguments
        assert!(email_user(&[json!(123)]).is_err());
        assert!(url_domain(&[json!(123)]).is_err());

        // Invalid URL
        assert!(url_domain(&[json!("not a url")]).is_err());
    }
}
