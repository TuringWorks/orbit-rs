//! Typestate pattern for compile-time state machines
//!
//! Uses Rust's type system to enforce valid state transitions at compile time,
//! making invalid states unrepresentable.

use crate::error::{OrbitError, OrbitResult};
use std::marker::PhantomData;

// ===== State Marker Types =====

/// Marker trait for connection states
pub trait ConnectionState {}

/// Connection is uninitialized
pub struct Uninitialized;
impl ConnectionState for Uninitialized {}

/// Connection is configured but not connected
pub struct Configured;
impl ConnectionState for Configured {}

/// Connection is actively connected
pub struct Connected;
impl ConnectionState for Connected {}

/// Connection is closed
pub struct Closed;
impl ConnectionState for Closed {}

// ===== Connection with Typestate =====

/// Database connection that enforces state transitions at compile time
pub struct DatabaseConnection<S: ConnectionState> {
    connection_string: String,
    handle: Option<String>, // Simulated connection handle
    _state: PhantomData<S>,
}

impl DatabaseConnection<Uninitialized> {
    /// Create a new uninitialized connection
    pub fn new() -> Self {
        Self {
            connection_string: String::new(),
            handle: None,
            _state: PhantomData,
        }
    }

    /// Configure the connection (transitions to Configured state)
    pub fn configure(self, connection_string: String) -> DatabaseConnection<Configured> {
        DatabaseConnection {
            connection_string,
            handle: None,
            _state: PhantomData,
        }
    }
}

impl DatabaseConnection<Configured> {
    /// Attempt to connect (transitions to Connected state)
    pub fn connect(mut self) -> OrbitResult<DatabaseConnection<Connected>> {
        if self.connection_string.is_empty() {
            return Err(OrbitError::configuration("Connection string cannot be empty"));
        }

        // Simulate connection
        self.handle = Some(format!("handle-{}", uuid::Uuid::new_v4()));

        Ok(DatabaseConnection {
            connection_string: self.connection_string,
            handle: self.handle,
            _state: PhantomData,
        })
    }

    /// Get connection string for inspection
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

impl DatabaseConnection<Connected> {
    /// Execute a query (only available in Connected state)
    pub fn execute(&self, query: &str) -> OrbitResult<String> {
        let handle = self.handle.as_ref().ok_or_else(|| {
            OrbitError::internal("Connection handle not available")
        })?;

        Ok(format!("Executed '{}' on {}", query, handle))
    }

    /// Get connection handle
    pub fn handle(&self) -> &str {
        self.handle.as_ref().unwrap()
    }

    /// Close the connection (transitions to Closed state)
    pub fn close(self) -> DatabaseConnection<Closed> {
        DatabaseConnection {
            connection_string: self.connection_string,
            handle: None,
            _state: PhantomData,
        }
    }
}

impl DatabaseConnection<Closed> {
    /// Cannot execute queries on closed connection (compile error if attempted)
    /// This demonstrates the power of typestate - invalid operations don't exist

    /// Get final statistics
    pub fn final_stats(&self) -> String {
        format!("Connection to '{}' closed", self.connection_string)
    }
}

// ===== Builder with Typestate =====

/// Marker trait for builder states
pub trait BuilderState {}

pub struct Incomplete;
impl BuilderState for Incomplete {}

pub struct Complete;
impl BuilderState for Complete {}

/// Configuration builder that enforces required fields at compile time
pub struct ConfigBuilder<S: BuilderState> {
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    _state: PhantomData<S>,
}

impl ConfigBuilder<Incomplete> {
    pub fn new() -> Self {
        Self {
            host: None,
            port: None,
            username: None,
            password: None,
            _state: PhantomData,
        }
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = Some(host);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn username(mut self, username: String) -> Self {
        self.username = Some(username);
        self
    }

    /// Setting password transitions to Complete state
    pub fn password(self, password: String) -> ConfigBuilder<Complete> {
        ConfigBuilder {
            host: self.host,
            port: self.port,
            username: self.username,
            password: Some(password),
            _state: PhantomData,
        }
    }
}

impl ConfigBuilder<Complete> {
    /// Build is only available when all required fields are set
    pub fn build(self) -> DatabaseConfig {
        DatabaseConfig {
            host: self.host.unwrap_or_else(|| "localhost".to_string()),
            port: self.port.unwrap_or(5432),
            username: self.username.unwrap_or_else(|| "admin".to_string()),
            password: self.password.unwrap(), // Safe because we're in Complete state
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

// ===== Transaction with Typestate =====

pub struct TxPending;
pub struct TxActive;
pub struct TxCommitted;
pub struct TxRolledBack;

pub trait TxState {}
impl TxState for TxPending {}
impl TxState for TxActive {}
impl TxState for TxCommitted {}
impl TxState for TxRolledBack {}

pub struct Transaction<S: TxState> {
    id: String,
    operations: Vec<String>,
    _state: PhantomData<S>,
}

impl Transaction<TxPending> {
    pub fn new(id: String) -> Self {
        Self {
            id,
            operations: Vec::new(),
            _state: PhantomData,
        }
    }

    pub fn begin(self) -> Transaction<TxActive> {
        Transaction {
            id: self.id,
            operations: self.operations,
            _state: PhantomData,
        }
    }
}

impl Transaction<TxActive> {
    pub fn execute(&mut self, operation: String) {
        self.operations.push(operation);
    }

    pub fn commit(self) -> Transaction<TxCommitted> {
        Transaction {
            id: self.id,
            operations: self.operations,
            _state: PhantomData,
        }
    }

    pub fn rollback(self) -> Transaction<TxRolledBack> {
        Transaction {
            id: self.id,
            operations: Vec::new(), // Clear operations on rollback
            _state: PhantomData,
        }
    }
}

impl Transaction<TxCommitted> {
    pub fn operations(&self) -> &[String] {
        &self.operations
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Transaction<TxRolledBack> {
    pub fn id(&self) -> &str {
        &self.id
    }
}

// ===== Macro for defining typestate machines =====

#[macro_export]
macro_rules! define_typestate_machine {
    (
        $name:ident {
            states: { $($state:ident),* $(,)? }
            transitions: {
                $($from:ident => $to:ident via $method:ident),* $(,)?
            }
        }
    ) => {
        $(
            pub struct $state;
            impl $crate::patterns::typestate::State for $state {}
        )*

        pub trait State {}
    };
}

pub trait State {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_typestate() {
        let conn = DatabaseConnection::new()
            .configure("postgres://localhost/test".to_string())
            .connect()
            .unwrap();

        let result = conn.execute("SELECT 1").unwrap();
        assert!(result.contains("SELECT 1"));

        let closed = conn.close();
        let stats = closed.final_stats();
        assert!(stats.contains("closed"));

        // This would be a compile error:
        // closed.execute("SELECT 1");
    }

    #[test]
    fn test_builder_typestate() {
        let config = ConfigBuilder::new()
            .host("postgres.example.com".to_string())
            .port(5432)
            .username("admin".to_string())
            .password("secret".to_string())
            .build();

        assert_eq!(config.host, "postgres.example.com");
        assert_eq!(config.password, "secret");

        // This would be a compile error (no password set):
        // let incomplete = ConfigBuilder::new().host("localhost".to_string());
        // let config = incomplete.build();
    }

    #[test]
    fn test_transaction_typestate() {
        let tx = Transaction::new("tx-123".to_string());
        let mut tx = tx.begin();

        tx.execute("INSERT INTO users VALUES (1)".to_string());
        tx.execute("INSERT INTO users VALUES (2)".to_string());

        let committed = tx.commit();
        assert_eq!(committed.operations().len(), 2);

        // This would be a compile error (can't execute on committed tx):
        // committed.execute("INSERT INTO users VALUES (3)".to_string());
    }

    #[test]
    fn test_transaction_rollback() {
        let tx = Transaction::new("tx-456".to_string());
        let mut tx = tx.begin();

        tx.execute("INSERT INTO users VALUES (1)".to_string());

        let rolled_back = tx.rollback();
        assert_eq!(rolled_back.id(), "tx-456");

        // This would be a compile error:
        // rolled_back.commit();
    }
}
