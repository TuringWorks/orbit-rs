//! Connection management for Orbit Desktop
//! 
//! This module handles connections to various database types including:
//! - PostgreSQL (standard SQL)
//! - OrbitQL (native Orbit protocol)
//! - Redis (key-value operations)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Connection information provided by user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub name: String,
    pub connection_type: ConnectionType,
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub connection_timeout: Option<u64>,
    pub additional_params: HashMap<String, String>,
}

/// Types of database connections supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    PostgreSQL,
    OrbitQL,
    Redis,
}

/// Connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// A managed database connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub info: ConnectionInfo,
    pub status: ConnectionStatus,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub query_count: u64,
}

/// Connection manager handles all database connections
#[derive(Default)]
pub struct ConnectionManager {
    connections: HashMap<String, Connection>,
    active_connections: HashMap<String, Box<dyn DatabaseConnection>>,
}

/// Trait for database connections
pub trait DatabaseConnection: Send + Sync {
    fn connection_type(&self) -> ConnectionType;
    fn is_connected(&self) -> bool;
    fn disconnect(&mut self) -> Result<(), ConnectionError>;
}

/// Connection errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),
    #[error("Connection timeout")]
    Timeout,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new connection
    pub async fn create_connection(&mut self, info: ConnectionInfo) -> Result<String, ConnectionError> {
        let connection_id = Uuid::new_v4().to_string();
        
        // Test the connection first
        let status = self.test_connection(&info).await?;
        
        let connection = Connection {
            id: connection_id.clone(),
            info: info.clone(),
            status,
            created_at: Utc::now(),
            last_used: None,
            query_count: 0,
        };

        // Store the connection
        self.connections.insert(connection_id.clone(), connection);
        
        // Create the actual database connection
        let db_connection = self.create_database_connection(&info).await?;
        self.active_connections.insert(connection_id.clone(), db_connection);

        Ok(connection_id)
    }

    /// Test a connection without storing it
    pub async fn test_connection(&self, info: &ConnectionInfo) -> Result<ConnectionStatus, ConnectionError> {
        match info.connection_type {
            ConnectionType::PostgreSQL => self.test_postgresql_connection(info).await,
            ConnectionType::OrbitQL => self.test_orbitql_connection(info).await,
            ConnectionType::Redis => self.test_redis_connection(info).await,
        }
    }

    /// List all stored connections
    pub async fn list_connections(&self) -> Vec<Connection> {
        self.connections.values().cloned().collect()
    }

    /// Get a specific connection
    pub async fn get_connection(&self, connection_id: &str) -> Option<&Connection> {
        self.connections.get(connection_id)
    }

    /// Disconnect a connection
    pub async fn disconnect(&mut self, connection_id: &str) -> Result<(), ConnectionError> {
        if let Some(mut db_connection) = self.active_connections.remove(connection_id) {
            db_connection.disconnect()?;
        }

        if let Some(connection) = self.connections.get_mut(connection_id) {
            connection.status = ConnectionStatus::Disconnected;
        }

        Ok(())
    }

    /// Delete a connection entirely
    pub async fn delete_connection(&mut self, connection_id: &str) -> Result<(), ConnectionError> {
        // First disconnect if connected
        self.disconnect(connection_id).await.ok();
        
        // Remove from storage
        self.connections.remove(connection_id);
        
        Ok(())
    }

    /// Update connection usage statistics
    pub async fn update_usage(&mut self, connection_id: &str) {
        if let Some(connection) = self.connections.get_mut(connection_id) {
            connection.last_used = Some(Utc::now());
            connection.query_count += 1;
        }
    }

    /// Get active database connection for query execution
    pub fn get_database_connection(&self, connection_id: &str) -> Option<&Box<dyn DatabaseConnection>> {
        self.active_connections.get(connection_id)
    }

    // Private helper methods

    async fn create_database_connection(&self, info: &ConnectionInfo) -> Result<Box<dyn DatabaseConnection>, ConnectionError> {
        match info.connection_type {
            ConnectionType::PostgreSQL => {
                let conn = PostgreSQLConnection::new(info).await?;
                Ok(Box::new(conn))
            },
            ConnectionType::OrbitQL => {
                let conn = OrbitQLConnection::new(info).await?;
                Ok(Box::new(conn))
            },
            ConnectionType::Redis => {
                let conn = RedisConnection::new(info).await?;
                Ok(Box::new(conn))
            },
        }
    }

    async fn test_postgresql_connection(&self, info: &ConnectionInfo) -> Result<ConnectionStatus, ConnectionError> {
        // Build connection string
        let connection_string = format!(
            "host={} port={} user={} password={} dbname={}",
            info.host,
            info.port,
            info.username.as_ref().unwrap_or(&"postgres".to_string()),
            info.password.as_ref().unwrap_or(&"".to_string()),
            info.database.as_ref().unwrap_or(&"postgres".to_string())
        );

        match tokio_postgres::connect(&connection_string, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                // Spawn the connection task
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("PostgreSQL connection error: {}", e);
                    }
                });
                
                // Test a simple query
                match client.simple_query("SELECT 1").await {
                    Ok(_) => Ok(ConnectionStatus::Connected),
                    Err(e) => Ok(ConnectionStatus::Error(format!("Query test failed: {}", e))),
                }
            },
            Err(e) => Ok(ConnectionStatus::Error(format!("Connection failed: {}", e))),
        }
    }

    async fn test_orbitql_connection(&self, info: &ConnectionInfo) -> Result<ConnectionStatus, ConnectionError> {
        // For now, just test if we can reach the host and port
        let addr = format!("{}:{}", info.host, info.port);
        
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(&addr)
        ).await {
            Ok(Ok(_)) => Ok(ConnectionStatus::Connected),
            Ok(Err(e)) => Ok(ConnectionStatus::Error(format!("Connection failed: {}", e))),
            Err(_) => Ok(ConnectionStatus::Error("Connection timeout".to_string())),
        }
    }

    async fn test_redis_connection(&self, info: &ConnectionInfo) -> Result<ConnectionStatus, ConnectionError> {
        let redis_url = format!(
            "redis://{}:{}/",
            info.host,
            info.port
        );

        match redis::Client::open(redis_url) {
            Ok(client) => {
                match client.get_connection() {
                    Ok(mut conn) => {
                        // Test with PING command
                        match redis::cmd("PING").query::<String>(&mut conn) {
                            Ok(response) if response == "PONG" => Ok(ConnectionStatus::Connected),
                            Ok(response) => Ok(ConnectionStatus::Error(format!("Unexpected response: {}", response))),
                            Err(e) => Ok(ConnectionStatus::Error(format!("Redis command failed: {}", e))),
                        }
                    },
                    Err(e) => Ok(ConnectionStatus::Error(format!("Connection failed: {}", e))),
                }
            },
            Err(e) => Ok(ConnectionStatus::Error(format!("Invalid Redis URL: {}", e))),
        }
    }
}

// Concrete database connection implementations

/// PostgreSQL connection
pub struct PostgreSQLConnection {
    client: Option<tokio_postgres::Client>,
    connected: bool,
}

impl PostgreSQLConnection {
    pub async fn new(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let connection_string = format!(
            "host={} port={} user={} password={} dbname={}",
            info.host,
            info.port,
            info.username.as_ref().unwrap_or(&"postgres".to_string()),
            info.password.as_ref().unwrap_or(&"".to_string()),
            info.database.as_ref().unwrap_or(&"postgres".to_string())
        );

        match tokio_postgres::connect(&connection_string, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                // Spawn the connection task
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("PostgreSQL connection error: {}", e);
                    }
                });
                
                Ok(Self {
                    client: Some(client),
                    connected: true,
                })
            },
            Err(e) => Err(ConnectionError::ConnectionFailed(e.to_string())),
        }
    }

    pub async fn execute_query(&self, query: &str) -> Result<Vec<tokio_postgres::Row>, ConnectionError> {
        if let Some(client) = &self.client {
            match client.query(query, &[]).await {
                Ok(rows) => Ok(rows),
                Err(e) => Err(ConnectionError::ConnectionFailed(e.to_string())),
            }
        } else {
            Err(ConnectionError::ConnectionNotFound("PostgreSQL client not available".to_string()))
        }
    }
}

impl DatabaseConnection for PostgreSQLConnection {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::PostgreSQL
    }

    fn is_connected(&self) -> bool {
        self.connected && self.client.is_some()
    }

    fn disconnect(&mut self) -> Result<(), ConnectionError> {
        self.client = None;
        self.connected = false;
        Ok(())
    }
}

/// OrbitQL connection
pub struct OrbitQLConnection {
    // This would connect to the actual Orbit instance
    // For now, we'll use HTTP client as a placeholder
    client: reqwest::Client,
    base_url: String,
    connected: bool,
}

impl OrbitQLConnection {
    pub async fn new(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let base_url = format!("http://{}:{}", info.host, info.port);
        let client = reqwest::Client::new();
        
        // Test connection with a health check
        let health_url = format!("{}/health", base_url);
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(Self {
                    client,
                    base_url,
                    connected: true,
                })
            },
            Ok(response) => Err(ConnectionError::ConnectionFailed(format!("Health check failed: {}", response.status()))),
            Err(e) => Err(ConnectionError::NetworkError(e.to_string())),
        }
    }

    pub async fn execute_orbitql(&self, query: &str) -> Result<serde_json::Value, ConnectionError> {
        if !self.connected {
            return Err(ConnectionError::ConnectionNotFound("Not connected".to_string()));
        }

        let query_url = format!("{}/query", self.base_url);
        let request_body = serde_json::json!({
            "query": query
        });

        match self.client.post(&query_url).json(&request_body).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(result) => Ok(result),
                        Err(e) => Err(ConnectionError::NetworkError(e.to_string())),
                    }
                } else {
                    Err(ConnectionError::ConnectionFailed(format!("Query failed: {}", response.status())))
                }
            },
            Err(e) => Err(ConnectionError::NetworkError(e.to_string())),
        }
    }
}

impl DatabaseConnection for OrbitQLConnection {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::OrbitQL
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn disconnect(&mut self) -> Result<(), ConnectionError> {
        self.connected = false;
        Ok(())
    }
}

/// Redis connection
pub struct RedisConnection {
    connection: Option<redis::Connection>,
    connected: bool,
}

impl RedisConnection {
    pub async fn new(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let redis_url = format!("redis://{}:{}/", info.host, info.port);

        match redis::Client::open(redis_url) {
            Ok(client) => {
                match client.get_connection() {
                    Ok(conn) => {
                        Ok(Self {
                            connection: Some(conn),
                            connected: true,
                        })
                    },
                    Err(e) => Err(ConnectionError::ConnectionFailed(e.to_string())),
                }
            },
            Err(e) => Err(ConnectionError::InvalidConfiguration(e.to_string())),
        }
    }

    pub fn execute_redis_command(&mut self, cmd: &str, args: &[&str]) -> Result<redis::Value, ConnectionError> {
        if let Some(conn) = &mut self.connection {
            let mut redis_cmd = redis::cmd(cmd);
            for arg in args {
                redis_cmd.arg(*arg);
            }
            
            match redis_cmd.query::<redis::Value>(conn) {
                Ok(value) => Ok(value),
                Err(e) => Err(ConnectionError::ConnectionFailed(e.to_string())),
            }
        } else {
            Err(ConnectionError::ConnectionNotFound("Redis connection not available".to_string()))
        }
    }
}

impl DatabaseConnection for RedisConnection {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::Redis
    }

    fn is_connected(&self) -> bool {
        self.connected && self.connection.is_some()
    }

    fn disconnect(&mut self) -> Result<(), ConnectionError> {
        self.connection = None;
        self.connected = false;
        Ok(())
    }
}