//! Set command handlers for Redis RESP protocol
//!
//! This module implements Redis set commands (SADD, SREM, SMEMBERS, etc.)
//! that operate on set data structures.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::error::{ProtocolError, ProtocolResult};
use crate::resp::{actors::SetActor, RespValue};
use async_trait::async_trait;
use bytes::Bytes;
use orbit_client::OrbitClient;
use orbit_shared::Key;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::debug;

pub struct SetCommands {
    base: BaseCommandHandler,
}

impl SetCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        let local_registry = Arc::new(crate::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    // Helper methods for argument parsing
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

    fn validate_arg_count(
        &self,
        command_name: &str,
        args: &[RespValue],
        expected: usize,
    ) -> ProtocolResult<()> {
        if args.len() != expected {
            return Err(ProtocolError::RespError(format!(
                "ERR wrong number of arguments for '{}' command",
                command_name.to_lowercase()
            )));
        }
        Ok(())
    }

    /// SADD key member [member ...] - Add members to a set
    async fn cmd_sadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sadd' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "SADD")?;
        let mut members = Vec::new();
        for i in 1..args.len() {
            members.push(self.get_string_arg(args, i, "SADD")?);
        }

        let result = self
            .base
            .local_registry
            .execute_set(&key, "sadd", &[serde_json::to_value(&members).unwrap()])
            .await;

        let added_count: Result<usize, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        match added_count {
            Ok(count) => {
                debug!("SADD {} {:?} -> {} added", key, members, count);
                Ok(RespValue::Integer(count as i64))
            }
            Err(_) => Ok(RespValue::Integer(0)),
        }
    }

    /// SREM key member [member ...] - Remove members from a set
    async fn cmd_srem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'srem' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "SREM")?;
        let mut members = Vec::new();
        for i in 1..args.len() {
            members.push(self.get_string_arg(args, i, "SREM")?);
        }

        let result = self
            .base
            .local_registry
            .execute_set(&key, "srem", &[serde_json::to_value(&members).unwrap()])
            .await;

        let removed_count: Result<usize, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        match removed_count {
            Ok(count) => {
                debug!("SREM {} {:?} -> {} removed", key, members, count);
                Ok(RespValue::Integer(count as i64))
            }
            Err(_) => Ok(RespValue::Integer(0)),
        }
    }

    /// SMEMBERS key - Get all members of a set
    async fn cmd_smembers(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SMEMBERS", args, 1)?;

        let key = self.get_string_arg(args, 0, "SMEMBERS")?;

        let result = self
            .base
            .local_registry
            .execute_set(&key, "smembers", &[])
            .await;

        let members_result: Result<Vec<String>, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        match members_result {
            Ok(members) => {
                let result: Vec<RespValue> = members
                    .into_iter()
                    .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
                    .collect();
                debug!("SMEMBERS {} -> {} members", key, result.len());
                Ok(RespValue::Array(result))
            }
            Err(_) => Ok(RespValue::Array(vec![])),
        }
    }

    /// SCARD key - Get the cardinality (size) of a set
    async fn cmd_scard(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SCARD", args, 1)?;

        let key = self.get_string_arg(args, 0, "SCARD")?;

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_set(&key, "scard", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let size: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!("SCARD {} -> {}", key, size);
        Ok(RespValue::Integer(size))
    }

    /// SISMEMBER key member - Check if member exists in set
    async fn cmd_sismember(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SISMEMBER", args, 2)?;

        let key = self.get_string_arg(args, 0, "SISMEMBER")?;
        let member = self.get_string_arg(args, 1, "SISMEMBER")?;

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_set(&key, "sismember", &[serde_json::to_value(member.clone()).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let is_member: bool = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(false);

        debug!("SISMEMBER {} {} -> {}", key, member, is_member);
        Ok(RespValue::Integer(if is_member { 1 } else { 0 }))
    }

    /// SUNION key [key ...] - Get union of multiple sets
    async fn cmd_sunion(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sunion' command".to_string(),
            ));
        }

        let mut result_set = HashSet::<String>::new();

        for i in 0..args.len() {
            let key = self.get_string_arg(args, i, "SUNION")?;

            // Get SetActor reference
            let actor_ref = self
                .base
                .orbit_client
                .actor_reference::<SetActor>(Key::StringKey { key: key.clone() })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                for member in members {
                    result_set.insert(member);
                }
                debug!("SUNION: Added members from key {}", key);
            }
        }

        let result: Vec<RespValue> = result_set
            .into_iter()
            .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
            .collect();

        debug!("SUNION: Final result has {} members", result.len());
        Ok(RespValue::Array(result))
    }

    /// SINTER key [key ...] - Get intersection of multiple sets
    async fn cmd_sinter(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sinter' command".to_string(),
            ));
        }

        let mut result_set: Option<HashSet<String>> = None;

        for i in 0..args.len() {
            let key = self.get_string_arg(args, i, "SINTER")?;

            // Get SetActor reference
            let actor_ref = self
                .base
                .orbit_client
                .actor_reference::<SetActor>(Key::StringKey { key: key.clone() })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                let current_set: HashSet<String> = members.into_iter().collect();

                match result_set {
                    None => {
                        result_set = Some(current_set);
                        debug!("SINTER: Initialized with members from key {}", key);
                    }
                    Some(ref mut existing_set) => {
                        let intersection: HashSet<String> =
                            existing_set.intersection(&current_set).cloned().collect();
                        *existing_set = intersection;
                        debug!("SINTER: After intersection with key {}", key);
                    }
                }
            } else {
                // If any set is empty or missing, intersection is empty
                result_set = Some(HashSet::new());
                break;
            }
        }

        let result: Vec<RespValue> = result_set
            .unwrap_or_default()
            .into_iter()
            .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
            .collect();

        debug!("SINTER: Final result has {} members", result.len());
        Ok(RespValue::Array(result))
    }

    /// SDIFF key [key ...] - Get difference of multiple sets
    async fn cmd_sdiff(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sdiff' command".to_string(),
            ));
        }

        let first_key = self.get_string_arg(args, 0, "SDIFF")?;
        let mut result_set = self.get_initial_set(&first_key).await?;

        // Remove members from subsequent sets
        for i in 1..args.len() {
            let key = self.get_string_arg(args, i, "SDIFF")?;
            self.remove_members_from_key(&mut result_set, &key).await;
        }

        let result: Vec<RespValue> = result_set
            .into_iter()
            .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
            .collect();

        debug!("SDIFF: Final result has {} members", result.len());
        Ok(RespValue::Array(result))
    }

    /// Get the initial set for SDIFF operation
    async fn get_initial_set(&self, first_key: &str) -> ProtocolResult<HashSet<String>> {
        let actor_ref = self
            .base
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: first_key.to_string(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

        match members_result {
            Ok(members) => {
                let result_set: HashSet<String> = members.into_iter().collect();
                debug!(
                    "SDIFF: Started with {} members from key {}",
                    result_set.len(),
                    first_key
                );
                Ok(result_set)
            }
            Err(_) => {
                debug!("SDIFF: Failed to get members from first key {}", first_key);
                Ok(HashSet::new())
            }
        }
    }

    /// Remove members from a specific key
    async fn remove_members_from_key(&self, result_set: &mut HashSet<String>, key: &str) {
        let actor_ref = self
            .base
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key.to_string(),
            })
            .await;

        if let Ok(actor_ref) = actor_ref {
            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                for member in members {
                    result_set.remove(&member);
                }
                debug!(
                    "SDIFF: After removing members from key {}, {} members remain",
                    key,
                    result_set.len()
                );
            }
        }
    }
}

#[async_trait]
impl CommandHandler for SetCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "SADD" => self.cmd_sadd(args).await,
            "SREM" => self.cmd_srem(args).await,
            "SMEMBERS" => self.cmd_smembers(args).await,
            "SCARD" => self.cmd_scard(args).await,
            "SISMEMBER" => self.cmd_sismember(args).await,
            "SUNION" => self.cmd_sunion(args).await,
            "SINTER" => self.cmd_sinter(args).await,
            "SDIFF" => self.cmd_sdiff(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown set command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "SADD",
            "SREM",
            "SMEMBERS",
            "SCARD",
            "SISMEMBER",
            "SUNION",
            "SINTER",
            "SDIFF",
        ]
    }
}
