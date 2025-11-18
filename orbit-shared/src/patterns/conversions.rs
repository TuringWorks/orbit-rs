//! Conversion patterns using From/Into and smart pointer idioms
//!
//! Demonstrates type conversions, Cow (Clone-on-Write), and other
//! zero-cost abstraction patterns in Rust.

use crate::error::{OrbitError, OrbitResult};
use std::borrow::Cow;
use std::sync::Arc;

// ===== From/Into Pattern for Error Types =====

/// Domain-specific error type
#[derive(Debug, Clone)]
pub enum QueryError {
    ParseError(String),
    ValidationError(String),
    ExecutionError(String),
}

impl From<std::io::Error> for QueryError {
    fn from(err: std::io::Error) -> Self {
        QueryError::ExecutionError(format!("IO error: {}", err))
    }
}

impl From<std::num::ParseIntError> for QueryError {
    fn from(err: std::num::ParseIntError) -> Self {
        QueryError::ParseError(format!("Parse error: {}", err))
    }
}

impl From<QueryError> for OrbitError {
    fn from(err: QueryError) -> Self {
        match err {
            QueryError::ParseError(msg) => OrbitError::query(msg),
            QueryError::ValidationError(msg) => OrbitError::validation(msg),
            QueryError::ExecutionError(msg) => OrbitError::internal(msg),
        }
    }
}

// ===== From/Into for Configuration Types =====

/// Database connection configuration
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
}

/// Connection string representation
#[derive(Debug, Clone)]
pub struct ConnectionString(String);

impl From<DbConfig> for ConnectionString {
    fn from(config: DbConfig) -> Self {
        ConnectionString(format!(
            "{}:{}@{}:{}/{}",
            config.username, "***", config.host, config.port, config.database
        ))
    }
}

impl From<ConnectionString> for String {
    fn from(conn: ConnectionString) -> Self {
        conn.0
    }
}

// Implementing Into is not necessary as it's automatically derived from From
// This demonstrates the From/Into symmetry

// ===== Cow (Clone-on-Write) Pattern =====

/// Function that accepts both owned and borrowed strings
pub fn process_text(text: Cow<'_, str>) -> String {
    // Only clones if modification is needed
    if text.contains("PLACEHOLDER") {
        text.replace("PLACEHOLDER", "actual_value")
    } else {
        text.into_owned()
    }
}

/// Configuration that uses Cow for efficient string handling
#[derive(Debug, Clone)]
pub struct Config<'a> {
    pub name: Cow<'a, str>,
    pub description: Cow<'a, str>,
    pub tags: Vec<Cow<'a, str>>,
}

impl<'a> Config<'a> {
    /// Create config with borrowed strings (zero-copy)
    pub fn borrowed(name: &'a str, description: &'a str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            description: Cow::Borrowed(description),
            tags: Vec::new(),
        }
    }

    /// Create config with owned strings
    pub fn owned(name: String, description: String) -> Self {
        Self {
            name: Cow::Owned(name),
            description: Cow::Owned(description),
            tags: Vec::new(),
        }
    }

    /// Add tag (accepts both owned and borrowed)
    pub fn add_tag(&mut self, tag: impl Into<Cow<'a, str>>) {
        self.tags.push(tag.into());
    }

    /// Convert to owned version (useful for 'static lifetime)
    pub fn into_owned(self) -> Config<'static> {
        Config {
            name: Cow::Owned(self.name.into_owned()),
            description: Cow::Owned(self.description.into_owned()),
            tags: self
                .tags
                .into_iter()
                .map(|t| Cow::Owned(t.into_owned()))
                .collect(),
        }
    }
}

// ===== Smart Pointer Conversions =====

/// Resource that can be shared
#[derive(Debug, Clone)]
pub struct SharedResource {
    pub id: String,
    pub data: Vec<u8>,
}

impl SharedResource {
    pub fn new(id: String, data: Vec<u8>) -> Self {
        Self { id, data }
    }

    /// Convert to Arc for sharing across threads
    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Create shared directly
    pub fn new_shared(id: String, data: Vec<u8>) -> Arc<Self> {
        Arc::new(Self { id, data })
    }
}

impl From<SharedResource> for Arc<SharedResource> {
    fn from(resource: SharedResource) -> Self {
        Arc::new(resource)
    }
}

// ===== AsRef/AsMut Pattern =====

/// Type that can be viewed as a slice
pub struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsMut<[u8]> for Buffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl From<Buffer> for Vec<u8> {
    fn from(buffer: Buffer) -> Self {
        buffer.data
    }
}

// Generic function that accepts anything that can be viewed as a byte slice
pub fn hash_data<T: AsRef<[u8]>>(data: T) -> u64 {
    let bytes = data.as_ref();
    // Simple hash for demonstration
    bytes.iter().fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64))
}

// ===== TryFrom/TryInto Pattern =====

/// Validated port number
#[derive(Debug, Clone, Copy)]
pub struct Port(u16);

impl TryFrom<u16> for Port {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == 0 {
            Err("Port cannot be 0".to_string())
        } else {
            Ok(Port(value))
        }
    }
}

impl From<Port> for u16 {
    fn from(port: Port) -> Self {
        port.0
    }
}

/// Safe percentage type (0-100)
#[derive(Debug, Clone, Copy)]
pub struct Percentage(f64);

impl TryFrom<f64> for Percentage {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !(0.0..=100.0).contains(&value) {
            Err(format!("Percentage must be between 0 and 100, got {}", value))
        } else {
            Ok(Percentage(value))
        }
    }
}

impl From<Percentage> for f64 {
    fn from(p: Percentage) -> Self {
        p.0
    }
}

// ===== Builder Pattern with Into =====

/// Query builder that accepts flexible input types
#[derive(Debug, Clone)]
pub struct Query {
    table: String,
    columns: Vec<String>,
    filter: Option<String>,
    limit: Option<usize>,
}

impl Query {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            columns: Vec::new(),
            filter: None,
            limit: None,
        }
    }

    pub fn select(mut self, columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.columns = columns.into_iter().map(|c| c.into()).collect();
        self
    }

    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn build(self) -> String {
        let columns = if self.columns.is_empty() {
            "*".to_string()
        } else {
            self.columns.join(", ")
        };

        let mut query = format!("SELECT {} FROM {}", columns, self.table);

        if let Some(filter) = self.filter {
            query.push_str(&format!(" WHERE {}", filter));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        query
    }
}

// ===== Deref Coercion Pattern =====

use std::ops::Deref;

/// Cached value that automatically dereferences to inner type
pub struct Cached<T> {
    value: T,
    cached_at: std::time::Instant,
}

impl<T> Cached<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            cached_at: std::time::Instant::now(),
        }
    }

    pub fn age(&self) -> std::time::Duration {
        self.cached_at.elapsed()
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for Cached<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> From<T> for Cached<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

// ===== Cow Usage Examples =====

/// Efficiently handles both owned and borrowed data
pub fn normalize_path(path: &str) -> Cow<'_, str> {
    if path.starts_with('/') {
        // Already normalized, return borrowed
        Cow::Borrowed(path)
    } else {
        // Needs modification, return owned
        Cow::Owned(format!("/{}", path))
    }
}

/// Processes data efficiently with Cow
pub fn process_data(data: Cow<'_, [u8]>) -> Vec<u8> {
    if data.iter().all(|&b| b.is_ascii()) {
        // No modification needed
        data.into_owned()
    } else {
        // Need to filter non-ASCII
        data.iter().filter(|&&b| b.is_ascii()).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversions() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let query_err: QueryError = io_err.into();

        match query_err {
            QueryError::ExecutionError(msg) => assert!(msg.contains("IO error")),
            _ => panic!("Wrong error type"),
        }

        let orbit_err: OrbitError = query_err.into();
        assert!(matches!(orbit_err, OrbitError::Internal { .. }));
    }

    #[test]
    fn test_config_conversions() {
        let config = DbConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "user".to_string(),
        };

        let conn_str: ConnectionString = config.into();
        let string: String = conn_str.into();
        assert!(string.contains("localhost:5432"));
    }

    #[test]
    fn test_cow_efficiency() {
        let borrowed = "test string";
        let result = process_text(Cow::Borrowed(borrowed));
        assert_eq!(result, "test string");

        let with_placeholder = "test PLACEHOLDER string";
        let result = process_text(Cow::Borrowed(with_placeholder));
        assert_eq!(result, "test actual_value string");
    }

    #[test]
    fn test_config_cow() {
        let mut config = Config::borrowed("test", "description");
        config.add_tag("tag1");
        config.add_tag("tag2".to_string());

        assert_eq!(config.tags.len(), 2);

        let owned = config.into_owned();
        assert!(matches!(owned.name, Cow::Owned(_)));
    }

    #[test]
    fn test_shared_resource() {
        let resource = SharedResource::new("res1".to_string(), vec![1, 2, 3]);
        let shared: Arc<SharedResource> = resource.into();

        assert_eq!(shared.id, "res1");
        assert_eq!(Arc::strong_count(&shared), 1);

        let cloned = Arc::clone(&shared);
        assert_eq!(Arc::strong_count(&shared), 2);
    }

    #[test]
    fn test_buffer_conversions() {
        let data = vec![1, 2, 3, 4, 5];
        let buffer: Buffer = data.into();

        assert_eq!(buffer.len(), 5);

        let hash = hash_data(&buffer);
        assert_ne!(hash, 0);

        let data_back: Vec<u8> = buffer.into();
        assert_eq!(data_back, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_try_from() {
        let port = Port::try_from(8080).unwrap();
        assert_eq!(u16::from(port), 8080);

        assert!(Port::try_from(0).is_err());

        let percentage = Percentage::try_from(75.5).unwrap();
        assert_eq!(f64::from(percentage), 75.5);

        assert!(Percentage::try_from(101.0).is_err());
        assert!(Percentage::try_from(-1.0).is_err());
    }

    #[test]
    fn test_query_builder() {
        let query = Query::new("users")
            .select(vec!["id", "name", "email"])
            .filter("age > 18")
            .limit(10)
            .build();

        assert!(query.contains("SELECT id, name, email"));
        assert!(query.contains("FROM users"));
        assert!(query.contains("WHERE age > 18"));
        assert!(query.contains("LIMIT 10"));
    }

    #[test]
    fn test_cached_deref() {
        let cached = Cached::new(String::from("hello"));

        // Can use string methods directly due to Deref
        assert_eq!(cached.len(), 5);
        assert!(cached.starts_with("he"));

        assert!(cached.age().as_millis() < 100);
    }

    #[test]
    fn test_normalize_path() {
        let absolute = "/home/user";
        let normalized = normalize_path(absolute);
        assert!(matches!(normalized, Cow::Borrowed(_)));
        assert_eq!(normalized, "/home/user");

        let relative = "home/user";
        let normalized = normalize_path(relative);
        assert!(matches!(normalized, Cow::Owned(_)));
        assert_eq!(normalized, "/home/user");
    }

    #[test]
    fn test_process_data_cow() {
        let ascii_data = b"hello world";
        let result = process_data(Cow::Borrowed(ascii_data));
        assert_eq!(result, ascii_data.to_vec());

        let mixed_data = vec![72, 101, 108, 108, 111, 255, 32, 119];
        let result = process_data(Cow::Owned(mixed_data.clone()));
        assert!(!result.contains(&255));
    }
}
