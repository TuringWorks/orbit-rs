//! ACL (Access Control List) command handlers for Redis RESP protocol
//!
//! This module implements Redis ACL commands for user management and access control.
//!
//! ## References
//! - Redis ACL Spec: `specifications/protocols/redis-resp-protocol-specification.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/redis>

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolError;
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// ACL categories for command classification
const ACL_CATEGORIES: &[&str] = &[
    "admin",
    "bitmap",
    "blocking",
    "connection",
    "dangerous",
    "fast",
    "geo",
    "hash",
    "hyperloglog",
    "keyspace",
    "list",
    "pubsub",
    "read",
    "scripting",
    "set",
    "slow",
    "sortedset",
    "stream",
    "string",
    "transaction",
    "write",
];

/// User ACL entry
#[derive(Debug, Clone)]
pub struct AclUser {
    /// User name
    pub name: String,
    /// Is user enabled
    pub enabled: bool,
    /// Password hashes (SHA256)
    pub passwords: Vec<String>,
    /// Whether the user has no password requirement
    pub nopass: bool,
    /// Allowed command categories (+ prefix means allow, - means deny)
    pub categories: HashSet<String>,
    /// Allowed commands
    pub commands: HashSet<String>,
    /// Denied commands
    pub denied_commands: HashSet<String>,
    /// Key patterns the user can access
    pub keys: Vec<String>,
    /// Channel patterns the user can access (for pub/sub)
    pub channels: Vec<String>,
}

impl Default for AclUser {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            passwords: Vec::new(),
            nopass: false,
            categories: HashSet::new(),
            commands: HashSet::new(),
            denied_commands: HashSet::new(),
            keys: Vec::new(),
            channels: Vec::new(),
        }
    }
}

impl AclUser {
    /// Create a default user with full access
    pub fn default_user() -> Self {
        let mut user = Self {
            name: "default".to_string(),
            enabled: true,
            nopass: true,
            ..Default::default()
        };
        user.categories.insert("+@all".to_string());
        user.keys.push("*".to_string());
        user.channels.push("*".to_string());
        user
    }

    /// Format user as ACL rule string
    pub fn to_acl_string(&self) -> String {
        let mut parts = vec![format!("user {}", self.name)];

        if self.enabled {
            parts.push("on".to_string());
        } else {
            parts.push("off".to_string());
        }

        if self.nopass {
            parts.push("nopass".to_string());
        }

        for pwd in &self.passwords {
            parts.push(format!("#{}", pwd));
        }

        for cat in &self.categories {
            parts.push(cat.clone());
        }

        for cmd in &self.commands {
            parts.push(format!("+{}", cmd));
        }

        for cmd in &self.denied_commands {
            parts.push(format!("-{}", cmd));
        }

        for key in &self.keys {
            parts.push(format!("~{}", key));
        }

        for channel in &self.channels {
            parts.push(format!("&{}", channel));
        }

        parts.join(" ")
    }
}

/// ACL security log entry
#[derive(Debug, Clone)]
pub struct AclLogEntry {
    pub count: u64,
    pub reason: String,
    pub context: String,
    pub object: String,
    pub username: String,
    pub client_info: String,
    pub timestamp: u64,
}

/// ACL manager for storing user configurations
pub struct AclManager {
    /// All users
    users: RwLock<HashMap<String, AclUser>>,
    /// Security log entries
    log: RwLock<Vec<AclLogEntry>>,
    /// Maximum log entries
    max_log_entries: usize,
}

impl AclManager {
    pub fn new() -> Self {
        let mut users = HashMap::new();
        users.insert("default".to_string(), AclUser::default_user());

        Self {
            users: RwLock::new(users),
            log: RwLock::new(Vec::new()),
            max_log_entries: 128,
        }
    }

    /// Get a user by name
    pub async fn get_user(&self, username: &str) -> Option<AclUser> {
        self.users.read().await.get(username).cloned()
    }

    /// Set or create a user
    pub async fn set_user(&self, user: AclUser) {
        self.users.write().await.insert(user.name.clone(), user);
    }

    /// Delete a user
    pub async fn del_user(&self, username: &str) -> bool {
        if username == "default" {
            return false; // Cannot delete default user
        }
        self.users.write().await.remove(username).is_some()
    }

    /// List all users
    pub async fn list_users(&self) -> Vec<AclUser> {
        self.users.read().await.values().cloned().collect()
    }

    /// Get user names
    pub async fn user_names(&self) -> Vec<String> {
        self.users.read().await.keys().cloned().collect()
    }

    /// Add log entry
    pub async fn add_log_entry(&self, entry: AclLogEntry) {
        let mut log = self.log.write().await;
        log.push(entry);
        while log.len() > self.max_log_entries {
            log.remove(0);
        }
    }

    /// Get log entries
    pub async fn get_log(&self, count: Option<usize>) -> Vec<AclLogEntry> {
        let log = self.log.read().await;
        let count = count.unwrap_or(10).min(log.len());
        log.iter().rev().take(count).cloned().collect()
    }

    /// Clear log
    pub async fn clear_log(&self) {
        self.log.write().await.clear();
    }
}

impl Default for AclManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AclCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
    acl_manager: Arc<AclManager>,
}

impl AclCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            acl_manager: Arc::new(AclManager::new()),
        }
    }

    fn get_string_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<String> {
        args.get(index).and_then(|v| v.as_string()).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "ERR invalid argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }

    /// ACL CAT [category] - List available ACL categories or commands in a category
    async fn cmd_acl_cat(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            // Return all categories
            let categories: Vec<RespValue> = ACL_CATEGORIES
                .iter()
                .map(|c| RespValue::BulkString(Bytes::from(c.as_bytes().to_vec())))
                .collect();
            debug!("ACL CAT -> {} categories", categories.len());
            return Ok(RespValue::Array(categories));
        }

        let category = self.get_string_arg(args, 0, "ACL CAT")?.to_lowercase();

        // Return commands in the category (simplified - just return category name)
        let commands = match category.as_str() {
            "string" => vec!["GET", "SET", "APPEND", "GETRANGE", "SETRANGE", "STRLEN", "INCR", "DECR", "MGET", "MSET"],
            "hash" => vec!["HGET", "HSET", "HDEL", "HEXISTS", "HGETALL", "HKEYS", "HVALS", "HLEN", "HINCRBY"],
            "list" => vec!["LPUSH", "RPUSH", "LPOP", "RPOP", "LRANGE", "LLEN", "LINDEX", "LSET"],
            "set" => vec!["SADD", "SREM", "SMEMBERS", "SCARD", "SISMEMBER", "SUNION", "SINTER", "SDIFF"],
            "sortedset" => vec!["ZADD", "ZREM", "ZRANGE", "ZSCORE", "ZRANGEBYSCORE", "ZCARD", "ZCOUNT"],
            "stream" => vec!["XADD", "XREAD", "XRANGE", "XLEN", "XGROUP", "XREADGROUP", "XACK", "XPENDING"],
            "pubsub" => vec!["PUBLISH", "SUBSCRIBE", "UNSUBSCRIBE", "PSUBSCRIBE", "PUNSUBSCRIBE"],
            "connection" => vec!["PING", "ECHO", "AUTH", "SELECT", "QUIT"],
            "admin" => vec!["ACL", "INFO", "DBSIZE", "FLUSHDB", "FLUSHALL", "CONFIG"],
            "read" => vec!["GET", "HGET", "LRANGE", "SMEMBERS", "ZRANGE", "XREAD"],
            "write" => vec!["SET", "HSET", "LPUSH", "SADD", "ZADD", "XADD", "DEL"],
            "fast" => vec!["GET", "SET", "PING", "INCR", "DECR"],
            "slow" => vec!["KEYS", "SCAN", "SORT", "LRANGE", "SMEMBERS"],
            "dangerous" => vec!["KEYS", "FLUSHDB", "FLUSHALL", "DEBUG", "SHUTDOWN"],
            "keyspace" => vec!["DEL", "EXISTS", "EXPIRE", "TTL", "KEYS", "RENAME", "TYPE"],
            _ => {
                return Err(ProtocolError::RespError(format!(
                    "ERR Unknown category '{}'",
                    category
                )));
            }
        };

        let resp_commands: Vec<RespValue> = commands
            .iter()
            .map(|c| RespValue::BulkString(Bytes::from(c.as_bytes().to_vec())))
            .collect();

        debug!("ACL CAT {} -> {} commands", category, resp_commands.len());
        Ok(RespValue::Array(resp_commands))
    }

    /// ACL DELUSER username [username ...] - Delete users
    async fn cmd_acl_deluser(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'acl deluser' command".to_string(),
            ));
        }

        let mut deleted = 0;
        for i in 0..args.len() {
            let username = self.get_string_arg(args, i, "ACL DELUSER")?;
            if self.acl_manager.del_user(&username).await {
                deleted += 1;
            }
        }

        debug!("ACL DELUSER -> {} deleted", deleted);
        Ok(RespValue::Integer(deleted))
    }

    /// ACL GENPASS [bits] - Generate a random password
    async fn cmd_acl_genpass(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let bits = if args.is_empty() {
            256
        } else {
            let bits_str = self.get_string_arg(args, 0, "ACL GENPASS")?;
            bits_str.parse::<usize>().unwrap_or(256).min(1024)
        };

        let bytes_needed = (bits + 7) / 8;
        let password: String = (0..bytes_needed)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();

        // Truncate to exact bit length (4 bits per hex char)
        let hex_chars = (bits + 3) / 4;
        let password = password.chars().take(hex_chars).collect::<String>();

        debug!("ACL GENPASS {} -> {} chars", bits, password.len());
        Ok(RespValue::BulkString(Bytes::from(password.into_bytes())))
    }

    /// ACL GETUSER username - Get user ACL details
    async fn cmd_acl_getuser(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'acl getuser' command".to_string(),
            ));
        }

        let username = self.get_string_arg(args, 0, "ACL GETUSER")?;

        match self.acl_manager.get_user(&username).await {
            Some(user) => {
                let mut result = Vec::new();

                // flags
                result.push(RespValue::BulkString(Bytes::from("flags")));
                let mut flags = Vec::new();
                if user.enabled {
                    flags.push(RespValue::BulkString(Bytes::from("on")));
                } else {
                    flags.push(RespValue::BulkString(Bytes::from("off")));
                }
                if user.nopass {
                    flags.push(RespValue::BulkString(Bytes::from("nopass")));
                }
                result.push(RespValue::Array(flags));

                // passwords
                result.push(RespValue::BulkString(Bytes::from("passwords")));
                let passwords: Vec<RespValue> = user
                    .passwords
                    .iter()
                    .map(|p| RespValue::BulkString(Bytes::from(p.as_bytes().to_vec())))
                    .collect();
                result.push(RespValue::Array(passwords));

                // commands
                result.push(RespValue::BulkString(Bytes::from("commands")));
                let mut cmd_str = String::new();
                for cat in &user.categories {
                    if !cmd_str.is_empty() {
                        cmd_str.push(' ');
                    }
                    cmd_str.push_str(cat);
                }
                for cmd in &user.commands {
                    if !cmd_str.is_empty() {
                        cmd_str.push(' ');
                    }
                    cmd_str.push('+');
                    cmd_str.push_str(cmd);
                }
                for cmd in &user.denied_commands {
                    if !cmd_str.is_empty() {
                        cmd_str.push(' ');
                    }
                    cmd_str.push('-');
                    cmd_str.push_str(cmd);
                }
                result.push(RespValue::BulkString(Bytes::from(cmd_str.into_bytes())));

                // keys
                result.push(RespValue::BulkString(Bytes::from("keys")));
                let keys: Vec<RespValue> = user
                    .keys
                    .iter()
                    .map(|k| RespValue::BulkString(Bytes::from(k.as_bytes().to_vec())))
                    .collect();
                result.push(RespValue::Array(keys));

                // channels
                result.push(RespValue::BulkString(Bytes::from("channels")));
                let channels: Vec<RespValue> = user
                    .channels
                    .iter()
                    .map(|c| RespValue::BulkString(Bytes::from(c.as_bytes().to_vec())))
                    .collect();
                result.push(RespValue::Array(channels));

                debug!("ACL GETUSER {} -> found", username);
                Ok(RespValue::Array(result))
            }
            None => {
                debug!("ACL GETUSER {} -> not found", username);
                Ok(RespValue::NullArray)
            }
        }
    }

    /// ACL LIST - List all users with their ACL rules
    async fn cmd_acl_list(&self) -> ProtocolResult<RespValue> {
        let users = self.acl_manager.list_users().await;
        let rules: Vec<RespValue> = users
            .iter()
            .map(|u| RespValue::BulkString(Bytes::from(u.to_acl_string().into_bytes())))
            .collect();

        debug!("ACL LIST -> {} users", rules.len());
        Ok(RespValue::Array(rules))
    }

    /// ACL LOAD - Reload ACL configuration (stub)
    async fn cmd_acl_load(&self) -> ProtocolResult<RespValue> {
        debug!("ACL LOAD -> OK (stub)");
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL LOG [count | RESET] - Show security log
    async fn cmd_acl_log(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if !args.is_empty() {
            let arg = self.get_string_arg(args, 0, "ACL LOG")?.to_uppercase();
            if arg == "RESET" {
                self.acl_manager.clear_log().await;
                debug!("ACL LOG RESET -> OK");
                return Ok(RespValue::SimpleString("OK".to_string()));
            }
        }

        let count: Option<usize> = if args.is_empty() {
            None
        } else {
            self.get_string_arg(args, 0, "ACL LOG")?
                .parse()
                .ok()
        };

        let entries = self.acl_manager.get_log(count).await;
        let resp_entries: Vec<RespValue> = entries
            .iter()
            .map(|e| {
                RespValue::Array(vec![
                    RespValue::BulkString(Bytes::from("count")),
                    RespValue::Integer(e.count as i64),
                    RespValue::BulkString(Bytes::from("reason")),
                    RespValue::BulkString(Bytes::from(e.reason.as_bytes().to_vec())),
                    RespValue::BulkString(Bytes::from("context")),
                    RespValue::BulkString(Bytes::from(e.context.as_bytes().to_vec())),
                    RespValue::BulkString(Bytes::from("object")),
                    RespValue::BulkString(Bytes::from(e.object.as_bytes().to_vec())),
                    RespValue::BulkString(Bytes::from("username")),
                    RespValue::BulkString(Bytes::from(e.username.as_bytes().to_vec())),
                ])
            })
            .collect();

        debug!("ACL LOG -> {} entries", resp_entries.len());
        Ok(RespValue::Array(resp_entries))
    }

    /// ACL SAVE - Save ACL configuration (stub)
    async fn cmd_acl_save(&self) -> ProtocolResult<RespValue> {
        debug!("ACL SAVE -> OK (stub)");
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL SETUSER username [rule ...] - Create or modify user
    async fn cmd_acl_setuser(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'acl setuser' command".to_string(),
            ));
        }

        let username = self.get_string_arg(args, 0, "ACL SETUSER")?;

        // Get existing user or create new one
        let mut user = self
            .acl_manager
            .get_user(&username)
            .await
            .unwrap_or_else(|| AclUser {
                name: username.clone(),
                ..Default::default()
            });

        // Parse rules
        for i in 1..args.len() {
            let rule = self.get_string_arg(args, i, "ACL SETUSER")?;
            let rule_lower = rule.to_lowercase();

            match rule_lower.as_str() {
                "on" => user.enabled = true,
                "off" => user.enabled = false,
                "nopass" => user.nopass = true,
                "resetpass" => {
                    user.passwords.clear();
                    user.nopass = false;
                }
                "reset" => {
                    user = AclUser {
                        name: username.clone(),
                        ..Default::default()
                    };
                }
                "allkeys" | "~*" => user.keys.push("*".to_string()),
                "allchannels" | "&*" => user.channels.push("*".to_string()),
                "allcommands" | "+@all" => {
                    user.categories.insert("+@all".to_string());
                }
                "nocommands" | "-@all" => {
                    user.categories.clear();
                    user.commands.clear();
                    user.categories.insert("-@all".to_string());
                }
                _ => {
                    // Handle category rules
                    if rule.starts_with("+@") || rule.starts_with("-@") {
                        user.categories.insert(rule.clone());
                    }
                    // Handle command rules
                    else if rule.starts_with('+') {
                        user.commands.insert(rule[1..].to_uppercase());
                    } else if rule.starts_with('-') {
                        user.denied_commands.insert(rule[1..].to_uppercase());
                    }
                    // Handle key patterns
                    else if rule.starts_with('~') {
                        user.keys.push(rule[1..].to_string());
                    }
                    // Handle channel patterns
                    else if rule.starts_with('&') {
                        user.channels.push(rule[1..].to_string());
                    }
                    // Handle password hash
                    else if rule.starts_with('#') {
                        user.passwords.push(rule[1..].to_string());
                    }
                    // Handle plaintext password (hash it)
                    else if rule.starts_with('>') {
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(rule[1..].as_bytes());
                        let hash = format!("{:x}", hasher.finalize());
                        user.passwords.push(hash);
                    }
                    // Remove password
                    else if rule.starts_with('<') {
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(rule[1..].as_bytes());
                        let hash = format!("{:x}", hasher.finalize());
                        user.passwords.retain(|p| p != &hash);
                    }
                }
            }
        }

        self.acl_manager.set_user(user).await;

        debug!("ACL SETUSER {} -> OK", username);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL USERS - List all usernames
    async fn cmd_acl_users(&self) -> ProtocolResult<RespValue> {
        let usernames = self.acl_manager.user_names().await;
        let resp_names: Vec<RespValue> = usernames
            .iter()
            .map(|n| RespValue::BulkString(Bytes::from(n.as_bytes().to_vec())))
            .collect();

        debug!("ACL USERS -> {} users", resp_names.len());
        Ok(RespValue::Array(resp_names))
    }

    /// ACL WHOAMI - Get current user (always "default" for now)
    async fn cmd_acl_whoami(&self) -> ProtocolResult<RespValue> {
        debug!("ACL WHOAMI -> default");
        Ok(RespValue::BulkString(Bytes::from("default")))
    }

    /// ACL DRYRUN username command [arg ...] - Test command against user ACL
    async fn cmd_acl_dryrun(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'acl dryrun' command".to_string(),
            ));
        }

        let username = self.get_string_arg(args, 0, "ACL DRYRUN")?;
        let command = self.get_string_arg(args, 1, "ACL DRYRUN")?.to_uppercase();

        match self.acl_manager.get_user(&username).await {
            Some(user) => {
                if !user.enabled {
                    return Ok(RespValue::BulkString(Bytes::from(
                        "This user is disabled".as_bytes().to_vec(),
                    )));
                }

                // Check if command is allowed (simplified check)
                if user.categories.contains(&"+@all".to_string()) {
                    if !user.denied_commands.contains(&command) {
                        debug!("ACL DRYRUN {} {} -> OK", username, command);
                        return Ok(RespValue::SimpleString("OK".to_string()));
                    }
                }

                if user.commands.contains(&command) {
                    debug!("ACL DRYRUN {} {} -> OK", username, command);
                    return Ok(RespValue::SimpleString("OK".to_string()));
                }

                Ok(RespValue::BulkString(Bytes::from(
                    format!("This user has no permissions to run the '{}' command", command).into_bytes(),
                )))
            }
            None => Err(ProtocolError::RespError(format!(
                "ERR User '{}' not found",
                username
            ))),
        }
    }

    /// ACL HELP - Get help on ACL subcommands
    async fn cmd_acl_help(&self) -> ProtocolResult<RespValue> {
        let help = vec![
            "ACL <subcommand> [<arg> [value] [opt] ...]. Subcommands are:",
            "CAT [<category>]",
            "    List all commands that belong to <category>, or all command categories",
            "    when no category is specified.",
            "DELUSER <username> [<username> ...]",
            "    Delete a list of users.",
            "DRYRUN <username> <command> [<arg> ...]",
            "    Test if a user has permission to run the given command.",
            "GENPASS [<bits>]",
            "    Generate a secure password. The optional `bits` argument can",
            "    be used to specify the size (default 256).",
            "GETUSER <username>",
            "    Get the user's details.",
            "LIST",
            "    List the current ACL rules in ACL config file format.",
            "LOAD",
            "    Reload the ACLs from the configured ACL file.",
            "LOG [<count> | RESET]",
            "    List latest events denied because of ACL.",
            "SAVE",
            "    Save the current ACLs to the configured ACL file.",
            "SETUSER <username> <property> [<property> ...]",
            "    Create or modify a user.",
            "USERS",
            "    List all registered usernames.",
            "WHOAMI",
            "    Return the current connection username.",
            "HELP",
            "    Prints this help.",
        ];

        let resp_help: Vec<RespValue> = help
            .iter()
            .map(|h| RespValue::BulkString(Bytes::from(h.as_bytes().to_vec())))
            .collect();

        Ok(RespValue::Array(resp_help))
    }
}

#[async_trait]
impl CommandHandler for AclCommands {
    async fn handle(&self, _command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // ACL commands are handled as ACL <subcommand>
        if args.is_empty() {
            return self.cmd_acl_help().await;
        }

        let subcommand = self.get_string_arg(args, 0, "ACL")?.to_uppercase();
        let sub_args = &args[1..];

        match subcommand.as_str() {
            "CAT" => self.cmd_acl_cat(sub_args).await,
            "DELUSER" => self.cmd_acl_deluser(sub_args).await,
            "DRYRUN" => self.cmd_acl_dryrun(sub_args).await,
            "GENPASS" => self.cmd_acl_genpass(sub_args).await,
            "GETUSER" => self.cmd_acl_getuser(sub_args).await,
            "HELP" => self.cmd_acl_help().await,
            "LIST" => self.cmd_acl_list().await,
            "LOAD" => self.cmd_acl_load().await,
            "LOG" => self.cmd_acl_log(sub_args).await,
            "SAVE" => self.cmd_acl_save().await,
            "SETUSER" => self.cmd_acl_setuser(sub_args).await,
            "USERS" => self.cmd_acl_users().await,
            "WHOAMI" => self.cmd_acl_whoami().await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR Unknown ACL subcommand '{}'",
                subcommand
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["ACL"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::traits::BaseCommandHandler;

    #[tokio::test]
    async fn test_acl_cat() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = AclCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            acl_manager: Arc::new(AclManager::new()),
        };

        // Test ACL CAT with no args returns categories
        let args = vec![RespValue::BulkString(Bytes::from("CAT"))];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(cats) => assert!(!cats.is_empty()),
            _ => panic!("Expected array"),
        }

        // Test ACL CAT with category
        let args = vec![
            RespValue::BulkString(Bytes::from("CAT")),
            RespValue::BulkString(Bytes::from("string")),
        ];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_acl_setuser_getuser() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = AclCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            acl_manager: Arc::new(AclManager::new()),
        };

        // Create a user
        let args = vec![
            RespValue::BulkString(Bytes::from("SETUSER")),
            RespValue::BulkString(Bytes::from("testuser")),
            RespValue::BulkString(Bytes::from("on")),
            RespValue::BulkString(Bytes::from("nopass")),
            RespValue::BulkString(Bytes::from("+@all")),
            RespValue::BulkString(Bytes::from("~*")),
        ];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());

        // Get the user
        let args = vec![
            RespValue::BulkString(Bytes::from("GETUSER")),
            RespValue::BulkString(Bytes::from("testuser")),
        ];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(user_info) => assert!(!user_info.is_empty()),
            _ => panic!("Expected array"),
        }
    }

    #[tokio::test]
    async fn test_acl_whoami() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = AclCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            acl_manager: Arc::new(AclManager::new()),
        };

        let args = vec![RespValue::BulkString(Bytes::from("WHOAMI"))];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::BulkString(name) => assert_eq!(name.as_ref(), b"default"),
            _ => panic!("Expected bulk string"),
        }
    }

    #[tokio::test]
    async fn test_acl_users() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = AclCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            acl_manager: Arc::new(AclManager::new()),
        };

        let args = vec![RespValue::BulkString(Bytes::from("USERS"))];
        let result = handler.handle("ACL", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(users) => {
                assert!(!users.is_empty());
                // Default user should exist
            }
            _ => panic!("Expected array"),
        }
    }
}
