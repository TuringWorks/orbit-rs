//! User, role, and privilege management for Neo4j
//!
//! Implements authentication, authorization, and access control

use crate::protocols::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Username
    pub username: String,
    /// Password hash (in production, use bcrypt or similar)
    pub password_hash: String,
    /// Whether the user must change password on next login
    pub password_change_required: bool,
    /// User status
    pub status: UserStatus,
    /// Assigned roles
    pub roles: HashSet<String>,
}

/// User status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// User is active
    Active,
    /// User is suspended
    Suspended,
}

impl User {
    /// Create a new user
    pub fn new(username: String, password_hash: String) -> Self {
        Self {
            username,
            password_hash,
            password_change_required: false,
            status: UserStatus::Active,
            roles: HashSet::new(),
        }
    }

    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }

    /// Add a role to the user
    pub fn add_role(&mut self, role: String) {
        self.roles.insert(role);
    }

    /// Remove a role from the user
    pub fn remove_role(&mut self, role: &str) -> bool {
        self.roles.remove(role)
    }
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name
    pub name: String,
    /// Role description
    pub description: Option<String>,
    /// Granted privileges
    pub privileges: HashSet<Privilege>,
}

impl Role {
    /// Create a new role
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            privileges: HashSet::new(),
        }
    }

    /// Add a privilege to the role
    pub fn grant_privilege(&mut self, privilege: Privilege) {
        self.privileges.insert(privilege);
    }

    /// Remove a privilege from the role
    pub fn revoke_privilege(&mut self, privilege: &Privilege) -> bool {
        self.privileges.remove(privilege)
    }

    /// Check if role has a specific privilege
    pub fn has_privilege(&self, privilege: &Privilege) -> bool {
        self.privileges.contains(privilege)
    }
}

/// Privilege type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Privilege {
    /// Database access
    Access { database: String },
    /// Read data
    Read { database: String, graph: Option<String> },
    /// Write data
    Write { database: String, graph: Option<String> },
    /// Create/drop indexes
    Index { database: String },
    /// Create/drop constraints
    Constraint { database: String },
    /// Create/drop tokens (labels, relationship types, property keys)
    CreateToken { database: String },
    /// Delete tokens
    DeleteToken { database: String },
    /// Traverse graph
    Traverse { database: String, graph: Option<String> },
    /// Execute procedures
    Execute { procedure: String },
    /// Execute boosted procedures
    ExecuteBoosted { procedure: String },
    /// Database management
    DatabaseManagement,
    /// User management
    UserManagement,
    /// Role management
    RoleManagement,
    /// Privilege management
    PrivilegeManagement,
}

/// Privilege action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegeAction {
    /// Grant privilege
    Grant,
    /// Deny privilege
    Deny,
}

/// User and role manager
#[derive(Debug)]
pub struct SecurityManager {
    /// Registered users
    users: HashMap<String, User>,
    /// Defined roles
    roles: HashMap<String, Role>,
    /// Built-in roles
    builtin_roles: HashSet<String>,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new() -> Self {
        let mut manager = Self {
            users: HashMap::new(),
            roles: HashMap::new(),
            builtin_roles: HashSet::new(),
        };

        // Create built-in roles
        manager.create_builtin_roles();
        
        // Create default admin user
        manager.create_default_admin();

        manager
    }

    /// Create built-in roles
    fn create_builtin_roles(&mut self) {
        // Admin role - full access
        let mut admin = Role::new("admin".to_string());
        admin.grant_privilege(Privilege::DatabaseManagement);
        admin.grant_privilege(Privilege::UserManagement);
        admin.grant_privilege(Privilege::RoleManagement);
        admin.grant_privilege(Privilege::PrivilegeManagement);
        self.roles.insert("admin".to_string(), admin);
        self.builtin_roles.insert("admin".to_string());

        // Reader role - read-only access
        let mut reader = Role::new("reader".to_string());
        reader.grant_privilege(Privilege::Read {
            database: "*".to_string(),
            graph: None,
        });
        reader.grant_privilege(Privilege::Traverse {
            database: "*".to_string(),
            graph: None,
        });
        self.roles.insert("reader".to_string(), reader);
        self.builtin_roles.insert("reader".to_string());

        // Editor role - read and write access
        let mut editor = Role::new("editor".to_string());
        editor.grant_privilege(Privilege::Read {
            database: "*".to_string(),
            graph: None,
        });
        editor.grant_privilege(Privilege::Write {
            database: "*".to_string(),
            graph: None,
        });
        editor.grant_privilege(Privilege::Traverse {
            database: "*".to_string(),
            graph: None,
        });
        self.roles.insert("editor".to_string(), editor);
        self.builtin_roles.insert("editor".to_string());

        // Architect role - schema management
        let mut architect = Role::new("architect".to_string());
        architect.grant_privilege(Privilege::Index {
            database: "*".to_string(),
        });
        architect.grant_privilege(Privilege::Constraint {
            database: "*".to_string(),
        });
        architect.grant_privilege(Privilege::CreateToken {
            database: "*".to_string(),
        });
        self.roles.insert("architect".to_string(), architect);
        self.builtin_roles.insert("architect".to_string());
    }

    /// Create default admin user
    fn create_default_admin(&mut self) {
        let mut admin_user = User::new("neo4j".to_string(), "neo4j".to_string());
        admin_user.password_change_required = true;
        admin_user.add_role("admin".to_string());
        self.users.insert("neo4j".to_string(), admin_user);
    }

    // User Management

    /// Create a new user
    pub fn create_user(&mut self, username: String, password_hash: String) -> ProtocolResult<()> {
        if self.users.contains_key(&username) {
            return Err(ProtocolError::CypherError(format!(
                "User '{}' already exists",
                username
            )));
        }

        let user = User::new(username.clone(), password_hash);
        self.users.insert(username, user);
        Ok(())
    }

    /// Alter a user
    pub fn alter_user(
        &mut self,
        username: &str,
        new_password_hash: Option<String>,
        password_change_required: Option<bool>,
        status: Option<UserStatus>,
    ) -> ProtocolResult<()> {
        let user = self.users.get_mut(username).ok_or_else(|| {
            ProtocolError::CypherError(format!("User '{}' does not exist", username))
        })?;

        if let Some(password) = new_password_hash {
            user.password_hash = password;
        }
        if let Some(change_required) = password_change_required {
            user.password_change_required = change_required;
        }
        if let Some(new_status) = status {
            user.status = new_status;
        }

        Ok(())
    }

    /// Drop a user
    pub fn drop_user(&mut self, username: &str) -> ProtocolResult<()> {
        if username == "neo4j" {
            return Err(ProtocolError::CypherError(
                "Cannot drop the default admin user".to_string(),
            ));
        }

        if self.users.remove(username).is_none() {
            return Err(ProtocolError::CypherError(format!(
                "User '{}' does not exist",
                username
            )));
        }
        Ok(())
    }

    /// Get a user
    pub fn get_user(&self, username: &str) -> Option<&User> {
        self.users.get(username)
    }

    /// List all users
    pub fn list_users(&self) -> Vec<&User> {
        self.users.values().collect()
    }

    // Role Management

    /// Create a new role
    pub fn create_role(&mut self, name: String) -> ProtocolResult<()> {
        if self.roles.contains_key(&name) {
            return Err(ProtocolError::CypherError(format!(
                "Role '{}' already exists",
                name
            )));
        }

        let role = Role::new(name.clone());
        self.roles.insert(name, role);
        Ok(())
    }

    /// Drop a role
    pub fn drop_role(&mut self, name: &str) -> ProtocolResult<()> {
        if self.builtin_roles.contains(name) {
            return Err(ProtocolError::CypherError(format!(
                "Cannot drop built-in role '{}'",
                name
            )));
        }

        if self.roles.remove(name).is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Role '{}' does not exist",
                name
            )));
        }

        // Remove role from all users
        for user in self.users.values_mut() {
            user.remove_role(name);
        }

        Ok(())
    }

    /// Get a role
    pub fn get_role(&self, name: &str) -> Option<&Role> {
        self.roles.get(name)
    }

    /// List all roles
    pub fn list_roles(&self) -> Vec<&Role> {
        self.roles.values().collect()
    }

    /// Grant a role to a user
    pub fn grant_role(&mut self, username: &str, role_name: &str) -> ProtocolResult<()> {
        if !self.roles.contains_key(role_name) {
            return Err(ProtocolError::CypherError(format!(
                "Role '{}' does not exist",
                role_name
            )));
        }

        let user = self.users.get_mut(username).ok_or_else(|| {
            ProtocolError::CypherError(format!("User '{}' does not exist", username))
        })?;

        user.add_role(role_name.to_string());
        Ok(())
    }

    /// Revoke a role from a user
    pub fn revoke_role(&mut self, username: &str, role_name: &str) -> ProtocolResult<()> {
        let user = self.users.get_mut(username).ok_or_else(|| {
            ProtocolError::CypherError(format!("User '{}' does not exist", username))
        })?;

        if !user.remove_role(role_name) {
            return Err(ProtocolError::CypherError(format!(
                "User '{}' does not have role '{}'",
                username, role_name
            )));
        }

        Ok(())
    }

    // Privilege Management

    /// Grant a privilege to a role
    pub fn grant_privilege(&mut self, role_name: &str, privilege: Privilege) -> ProtocolResult<()> {
        if self.builtin_roles.contains(role_name) {
            return Err(ProtocolError::CypherError(format!(
                "Cannot modify built-in role '{}'",
                role_name
            )));
        }

        let role = self.roles.get_mut(role_name).ok_or_else(|| {
            ProtocolError::CypherError(format!("Role '{}' does not exist", role_name))
        })?;

        role.grant_privilege(privilege);
        Ok(())
    }

    /// Revoke a privilege from a role
    pub fn revoke_privilege(&mut self, role_name: &str, privilege: &Privilege) -> ProtocolResult<()> {
        if self.builtin_roles.contains(role_name) {
            return Err(ProtocolError::CypherError(format!(
                "Cannot modify built-in role '{}'",
                role_name
            )));
        }

        let role = self.roles.get_mut(role_name).ok_or_else(|| {
            ProtocolError::CypherError(format!("Role '{}' does not exist", role_name))
        })?;

        if !role.revoke_privilege(privilege) {
            return Err(ProtocolError::CypherError(
                "Role does not have this privilege".to_string(),
            ));
        }

        Ok(())
    }

    /// List all privileges for a role
    pub fn list_privileges(&self, role_name: &str) -> ProtocolResult<Vec<&Privilege>> {
        let role = self.roles.get(role_name).ok_or_else(|| {
            ProtocolError::CypherError(format!("Role '{}' does not exist", role_name))
        })?;

        Ok(role.privileges.iter().collect())
    }

    /// Check if a user has a specific privilege
    pub fn user_has_privilege(&self, username: &str, privilege: &Privilege) -> bool {
        if let Some(user) = self.users.get(username) {
            for role_name in &user.roles {
                if let Some(role) = self.roles.get(role_name) {
                    if role.has_privilege(privilege) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user() {
        let mut manager = SecurityManager::new();
        assert!(manager.create_user("alice".to_string(), "hash123".to_string()).is_ok());
        assert_eq!(manager.list_users().len(), 2); // neo4j + alice
    }

    #[test]
    fn test_alter_user() {
        let mut manager = SecurityManager::new();
        manager.create_user("alice".to_string(), "hash123".to_string()).unwrap();
        
        assert!(manager.alter_user(
            "alice",
            Some("newhash".to_string()),
            Some(true),
            None
        ).is_ok());

        let user = manager.get_user("alice").unwrap();
        assert_eq!(user.password_hash, "newhash");
        assert!(user.password_change_required);
    }

    #[test]
    fn test_create_role() {
        let mut manager = SecurityManager::new();
        assert!(manager.create_role("custom_role".to_string()).is_ok());
    }

    #[test]
    fn test_grant_revoke_role() {
        let mut manager = SecurityManager::new();
        manager.create_user("alice".to_string(), "hash123".to_string()).unwrap();
        
        assert!(manager.grant_role("alice", "reader").is_ok());
        assert!(manager.get_user("alice").unwrap().has_role("reader"));

        assert!(manager.revoke_role("alice", "reader").is_ok());
        assert!(!manager.get_user("alice").unwrap().has_role("reader"));
    }

    #[test]
    fn test_grant_privilege() {
        let mut manager = SecurityManager::new();
        manager.create_role("custom_role".to_string()).unwrap();
        
        let privilege = Privilege::Read {
            database: "testdb".to_string(),
            graph: None,
        };

        assert!(manager.grant_privilege("custom_role", privilege.clone()).is_ok());
        assert!(manager.get_role("custom_role").unwrap().has_privilege(&privilege));
    }

    #[test]
    fn test_builtin_roles() {
        let manager = SecurityManager::new();
        assert!(manager.get_role("admin").is_some());
        assert!(manager.get_role("reader").is_some());
        assert!(manager.get_role("editor").is_some());
        assert!(manager.get_role("architect").is_some());
    }

    #[test]
    fn test_cannot_drop_builtin_role() {
        let mut manager = SecurityManager::new();
        assert!(manager.drop_role("admin").is_err());
    }

    #[test]
    fn test_user_has_privilege() {
        let mut manager = SecurityManager::new();
        manager.create_user("alice".to_string(), "hash123".to_string()).unwrap();
        manager.grant_role("alice", "reader").is_ok();

        let privilege = Privilege::Read {
            database: "*".to_string(),
            graph: None,
        };

        assert!(manager.user_has_privilege("alice", &privilege));
    }
}
