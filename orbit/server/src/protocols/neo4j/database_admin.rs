//! Database administration for Neo4j
//!
//! Implements database lifecycle management commands

use crate::protocols::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseState {
    /// Database is online and accepting queries
    Online,
    /// Database is offline
    Offline,
    /// Database is starting
    Starting,
    /// Database is stopping
    Stopping,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database name
    pub name: String,
    /// Current state
    pub state: DatabaseState,
    /// Whether this is the default database
    pub is_default: bool,
    /// Database options
    pub options: HashMap<String, String>,
}

impl DatabaseConfig {
    /// Create a new database configuration
    pub fn new(name: String, is_default: bool) -> Self {
        Self {
            name,
            state: DatabaseState::Online,
            is_default,
            options: HashMap::new(),
        }
    }
}

/// Database manager
#[derive(Debug)]
pub struct DatabaseManager {
    /// Active databases
    databases: HashMap<String, DatabaseConfig>,
    /// Default database name
    default_database: String,
}

impl DatabaseManager {
    /// Create a new database manager
    pub fn new() -> Self {
        let mut databases = HashMap::new();
        let default_name = "neo4j".to_string();

        // Create default database
        databases.insert(
            default_name.clone(),
            DatabaseConfig::new(default_name.clone(), true),
        );

        Self {
            databases,
            default_database: default_name,
        }
    }

    /// Create a new database
    pub fn create_database(
        &mut self,
        name: String,
        options: HashMap<String, String>,
    ) -> ProtocolResult<()> {
        if self.databases.contains_key(&name) {
            return Err(ProtocolError::CypherError(format!(
                "Database '{}' already exists",
                name
            )));
        }

        let mut config = DatabaseConfig::new(name.clone(), false);
        config.options = options;
        self.databases.insert(name, config);
        Ok(())
    }

    /// Drop a database
    pub fn drop_database(&mut self, name: &str) -> ProtocolResult<()> {
        if name == self.default_database {
            return Err(ProtocolError::CypherError(
                "Cannot drop the default database".to_string(),
            ));
        }

        if self.databases.remove(name).is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Database '{}' does not exist",
                name
            )));
        }
        Ok(())
    }

    /// Start a database
    pub fn start_database(&mut self, name: &str) -> ProtocolResult<()> {
        let db = self.databases.get_mut(name).ok_or_else(|| {
            ProtocolError::CypherError(format!("Database '{}' does not exist", name))
        })?;

        match db.state {
            DatabaseState::Online => {
                return Err(ProtocolError::CypherError(format!(
                    "Database '{}' is already online",
                    name
                )));
            }
            DatabaseState::Starting | DatabaseState::Stopping => {
                return Err(ProtocolError::CypherError(format!(
                    "Database '{}' is in transition state",
                    name
                )));
            }
            DatabaseState::Offline => {
                db.state = DatabaseState::Online;
            }
        }
        Ok(())
    }

    /// Stop a database
    pub fn stop_database(&mut self, name: &str) -> ProtocolResult<()> {
        if name == self.default_database {
            return Err(ProtocolError::CypherError(
                "Cannot stop the default database".to_string(),
            ));
        }

        let db = self.databases.get_mut(name).ok_or_else(|| {
            ProtocolError::CypherError(format!("Database '{}' does not exist", name))
        })?;

        match db.state {
            DatabaseState::Offline => {
                return Err(ProtocolError::CypherError(format!(
                    "Database '{}' is already offline",
                    name
                )));
            }
            DatabaseState::Starting | DatabaseState::Stopping => {
                return Err(ProtocolError::CypherError(format!(
                    "Database '{}' is in transition state",
                    name
                )));
            }
            DatabaseState::Online => {
                db.state = DatabaseState::Offline;
            }
        }
        Ok(())
    }

    /// Get a database configuration
    pub fn get_database(&self, name: &str) -> Option<&DatabaseConfig> {
        self.databases.get(name)
    }

    /// List all databases
    pub fn list_databases(&self) -> Vec<&DatabaseConfig> {
        self.databases.values().collect()
    }

    /// Get the default database
    pub fn get_default_database(&self) -> &DatabaseConfig {
        self.databases.get(&self.default_database).unwrap()
    }

    /// Set the default database
    pub fn set_default_database(&mut self, name: &str) -> ProtocolResult<()> {
        if !self.databases.contains_key(name) {
            return Err(ProtocolError::CypherError(format!(
                "Database '{}' does not exist",
                name
            )));
        }

        // Update old default
        if let Some(old_default) = self.databases.get_mut(&self.default_database) {
            old_default.is_default = false;
        }

        // Set new default
        if let Some(new_default) = self.databases.get_mut(name) {
            new_default.is_default = true;
        }

        self.default_database = name.to_string();
        Ok(())
    }
}

impl Default for DatabaseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_database() {
        let mut manager = DatabaseManager::new();
        assert!(manager
            .create_database("testdb".to_string(), HashMap::new())
            .is_ok());
        assert_eq!(manager.list_databases().len(), 2); // neo4j + testdb
    }

    #[test]
    fn test_duplicate_database() {
        let mut manager = DatabaseManager::new();
        manager
            .create_database("testdb".to_string(), HashMap::new())
            .unwrap();
        assert!(manager
            .create_database("testdb".to_string(), HashMap::new())
            .is_err());
    }

    #[test]
    fn test_drop_database() {
        let mut manager = DatabaseManager::new();
        manager
            .create_database("testdb".to_string(), HashMap::new())
            .unwrap();
        assert!(manager.drop_database("testdb").is_ok());
        assert_eq!(manager.list_databases().len(), 1);
    }

    #[test]
    fn test_cannot_drop_default() {
        let mut manager = DatabaseManager::new();
        assert!(manager.drop_database("neo4j").is_err());
    }

    #[test]
    fn test_start_stop_database() {
        let mut manager = DatabaseManager::new();
        manager
            .create_database("testdb".to_string(), HashMap::new())
            .unwrap();

        assert!(manager.stop_database("testdb").is_ok());
        assert_eq!(
            manager.get_database("testdb").unwrap().state,
            DatabaseState::Offline
        );

        assert!(manager.start_database("testdb").is_ok());
        assert_eq!(
            manager.get_database("testdb").unwrap().state,
            DatabaseState::Online
        );
    }

    #[test]
    fn test_default_database() {
        let manager = DatabaseManager::new();
        let default = manager.get_default_database();
        assert_eq!(default.name, "neo4j");
        assert!(default.is_default);
    }

    #[test]
    fn test_set_default_database() {
        let mut manager = DatabaseManager::new();
        manager
            .create_database("testdb".to_string(), HashMap::new())
            .unwrap();

        assert!(manager.set_default_database("testdb").is_ok());
        assert_eq!(manager.get_default_database().name, "testdb");
        assert!(!manager.get_database("neo4j").unwrap().is_default);
    }
}
