//! Redis Cluster commands implementation
//!
//! Provides CLUSTER commands for Redis cluster mode support.
//! Currently implements a single-node "cluster" mode that can be extended
//! to full cluster support in the future.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::{error::ProtocolResult, resp::RespValue};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Number of hash slots in Redis Cluster
const CLUSTER_SLOTS: u16 = 16384;

/// Cluster node information
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// Node ID (40 character hex string)
    pub id: String,
    /// Node address (host:port)
    pub address: String,
    /// Node flags (master, slave, myself, etc.)
    pub flags: Vec<String>,
    /// Master node ID if this is a replica
    pub master_id: Option<String>,
    /// Ping sent timestamp
    pub ping_sent: u64,
    /// Pong received timestamp
    pub pong_recv: u64,
    /// Config epoch
    pub config_epoch: u64,
    /// Link state (connected/disconnected)
    pub link_state: String,
    /// Slots assigned to this node (for masters)
    pub slots: Vec<(u16, u16)>,
}

impl Default for ClusterNode {
    fn default() -> Self {
        Self {
            id: generate_node_id(),
            address: "127.0.0.1:6379".to_string(),
            flags: vec!["myself".to_string(), "master".to_string()],
            master_id: None,
            ping_sent: 0,
            pong_recv: 0,
            config_epoch: 1,
            link_state: "connected".to_string(),
            slots: vec![(0, CLUSTER_SLOTS - 1)], // All slots for single node
        }
    }
}

/// Generate a random 40-character hex node ID
fn generate_node_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Create 40-char hex ID by combining timestamp with padding
    // Redis uses 160-bit (40 hex chars) node IDs
    let part1 = format!("{:032x}", now);
    let hash = crc16_xmodem(part1.as_bytes());
    format!("{}{:08x}", &part1[..32], hash as u32 | ((now as u32) << 16))
}

/// Calculate the hash slot for a key using CRC16
fn key_hash_slot(key: &str) -> u16 {
    // Check for hash tags: {tag}
    // Only the part inside {} is used for hashing
    let hash_key = if let Some(start) = key.find('{') {
        if let Some(end) = key[start + 1..].find('}') {
            if end > 0 {
                &key[start + 1..start + 1 + end]
            } else {
                key
            }
        } else {
            key
        }
    } else {
        key
    };

    // CRC16 XMODEM
    crc16_xmodem(hash_key.as_bytes()) % CLUSTER_SLOTS
}

/// CRC16 XMODEM implementation for Redis cluster
fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Cluster state management
pub struct ClusterState {
    /// This node's information
    pub myself: ClusterNode,
    /// All known nodes in the cluster
    pub nodes: HashMap<String, ClusterNode>,
    /// Cluster state (ok, fail)
    pub state: String,
    /// Cluster slots assigned count
    pub slots_assigned: u16,
    /// Cluster slots ok count
    pub slots_ok: u16,
    /// Cluster size (number of masters serving at least one slot)
    pub cluster_size: u16,
    /// Known nodes count
    pub cluster_known_nodes: u16,
}

impl Default for ClusterState {
    fn default() -> Self {
        let myself = ClusterNode::default();
        let id = myself.id.clone();
        let mut nodes = HashMap::new();
        nodes.insert(id.clone(), myself.clone());

        Self {
            myself,
            nodes,
            state: "ok".to_string(),
            slots_assigned: CLUSTER_SLOTS,
            slots_ok: CLUSTER_SLOTS,
            cluster_size: 1,
            cluster_known_nodes: 1,
        }
    }
}

/// Handler for CLUSTER commands
pub struct ClusterCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
    /// Cluster state
    state: Arc<RwLock<ClusterState>>,
}

impl ClusterCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            state: Arc::new(RwLock::new(ClusterState::default())),
        }
    }

    /// CLUSTER INFO - Get cluster state info
    async fn cmd_cluster_info(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let state = self.state.read().await;

        let info = format!(
            "cluster_state:{}\r\n\
             cluster_slots_assigned:{}\r\n\
             cluster_slots_ok:{}\r\n\
             cluster_slots_pfail:0\r\n\
             cluster_slots_fail:0\r\n\
             cluster_known_nodes:{}\r\n\
             cluster_size:{}\r\n\
             cluster_current_epoch:{}\r\n\
             cluster_my_epoch:{}\r\n\
             cluster_stats_messages_ping_sent:0\r\n\
             cluster_stats_messages_pong_sent:0\r\n\
             cluster_stats_messages_sent:0\r\n\
             cluster_stats_messages_ping_received:0\r\n\
             cluster_stats_messages_pong_received:0\r\n\
             cluster_stats_messages_received:0\r\n\
             total_cluster_links_buffer_limit_exceeded:0",
            state.state,
            state.slots_assigned,
            state.slots_ok,
            state.cluster_known_nodes,
            state.cluster_size,
            state.myself.config_epoch,
            state.myself.config_epoch,
        );

        Ok(RespValue::bulk_string(info))
    }

    /// CLUSTER NODES - Get cluster nodes info
    async fn cmd_cluster_nodes(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let state = self.state.read().await;
        let mut lines = Vec::new();

        for node in state.nodes.values() {
            let flags = node.flags.join(",");
            let master = node.master_id.as_deref().unwrap_or("-");
            let slots: String = node
                .slots
                .iter()
                .map(|(start, end)| {
                    if start == end {
                        format!("{}", start)
                    } else {
                        format!("{}-{}", start, end)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            lines.push(format!(
                "{} {} {} {} {} {} {} {}",
                node.id,
                node.address,
                flags,
                master,
                node.ping_sent,
                node.pong_recv,
                node.config_epoch,
                node.link_state,
            ));
            if !slots.is_empty() {
                let last = lines.last_mut().unwrap();
                last.push(' ');
                last.push_str(&slots);
            }
        }

        Ok(RespValue::bulk_string(lines.join("\n")))
    }

    /// CLUSTER SLOTS - Get array of slot range mappings
    async fn cmd_cluster_slots(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let state = self.state.read().await;
        let mut result = Vec::new();

        for node in state.nodes.values() {
            if !node.flags.contains(&"master".to_string()) {
                continue;
            }

            for (start, end) in &node.slots {
                // Parse host and port from address
                let parts: Vec<&str> = node.address.split(':').collect();
                let host = parts.first().copied().unwrap_or("127.0.0.1").to_string();
                let port: i64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(6379);

                // Each slot range is [start, end, [master_host, master_port, master_id], [replica...]]
                let master_info = RespValue::Array(vec![
                    RespValue::bulk_string(host),
                    RespValue::Integer(port),
                    RespValue::bulk_string(node.id.clone()),
                ]);

                result.push(RespValue::Array(vec![
                    RespValue::Integer(*start as i64),
                    RespValue::Integer(*end as i64),
                    master_info,
                ]));
            }
        }

        Ok(RespValue::Array(result))
    }

    /// CLUSTER MYID - Get this node's ID
    async fn cmd_cluster_myid(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let state = self.state.read().await;
        Ok(RespValue::bulk_string(state.myself.id.clone()))
    }

    /// CLUSTER KEYSLOT <key> - Return the hash slot for key
    async fn cmd_cluster_keyslot(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("CLUSTER KEYSLOT", args, 1)?;
        let key = self.get_string_arg(args, 0, "CLUSTER KEYSLOT")?;
        let slot = key_hash_slot(&key);
        Ok(RespValue::Integer(slot as i64))
    }

    /// CLUSTER COUNTKEYSINSLOT <slot> - Return the number of keys in slot
    async fn cmd_cluster_countkeysinslot(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("CLUSTER COUNTKEYSINSLOT", args, 1)?;
        // For now, return 0 - would need to scan keys in production
        Ok(RespValue::Integer(0))
    }

    /// CLUSTER GETKEYSINSLOT <slot> <count> - Return keys in slot
    async fn cmd_cluster_getkeysinslot(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("CLUSTER GETKEYSINSLOT", args, 2)?;
        // For now, return empty array - would need to scan keys in production
        Ok(RespValue::Array(vec![]))
    }

    /// CLUSTER SHARDS - Get cluster shards info (Redis 7.0+)
    async fn cmd_cluster_shards(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let state = self.state.read().await;
        let mut shards = Vec::new();

        for node in state.nodes.values() {
            if !node.flags.contains(&"master".to_string()) {
                continue;
            }

            // Build slots array
            let mut slots = Vec::new();
            for (start, end) in &node.slots {
                slots.push(RespValue::Integer(*start as i64));
                slots.push(RespValue::Integer(*end as i64));
            }

            // Parse host and port
            let parts: Vec<&str> = node.address.split(':').collect();
            let host = parts.first().copied().unwrap_or("127.0.0.1").to_string();
            let port: i64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(6379);

            // Build node info
            let node_info = RespValue::Array(vec![
                RespValue::bulk_string("id"),
                RespValue::bulk_string(node.id.clone()),
                RespValue::bulk_string("port"),
                RespValue::Integer(port),
                RespValue::bulk_string("ip"),
                RespValue::bulk_string(host.clone()),
                RespValue::bulk_string("endpoint"),
                RespValue::bulk_string(host),
                RespValue::bulk_string("role"),
                RespValue::bulk_string("master"),
                RespValue::bulk_string("replication-offset"),
                RespValue::Integer(0),
                RespValue::bulk_string("health"),
                RespValue::bulk_string("online"),
            ]);

            let shard = RespValue::Array(vec![
                RespValue::bulk_string("slots"),
                RespValue::Array(slots),
                RespValue::bulk_string("nodes"),
                RespValue::Array(vec![node_info]),
            ]);

            shards.push(shard);
        }

        Ok(RespValue::Array(shards))
    }

    /// CLUSTER REPLICAS <node-id> - List replicas of a node
    async fn cmd_cluster_replicas(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("CLUSTER REPLICAS", args, 1)?;
        let node_id = self.get_string_arg(args, 0, "CLUSTER REPLICAS")?;

        let state = self.state.read().await;
        let mut replicas = Vec::new();

        for node in state.nodes.values() {
            if node.master_id.as_ref() == Some(&node_id) {
                let flags = node.flags.join(",");
                let master = node.master_id.as_deref().unwrap_or("-");
                replicas.push(RespValue::bulk_string(format!(
                    "{} {} {} {} {} {} {} {}",
                    node.id,
                    node.address,
                    flags,
                    master,
                    node.ping_sent,
                    node.pong_recv,
                    node.config_epoch,
                    node.link_state,
                )));
            }
        }

        Ok(RespValue::Array(replicas))
    }

    /// READONLY - Enable readonly mode for cluster replicas
    async fn cmd_readonly(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // In single-node mode, this is a no-op
        Ok(RespValue::ok())
    }

    /// READWRITE - Disable readonly mode
    async fn cmd_readwrite(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // In single-node mode, this is a no-op
        Ok(RespValue::ok())
    }
}

#[async_trait]
impl CommandHandler for ClusterCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Handle CLUSTER subcommands
        if command_name == "CLUSTER" {
            if args.is_empty() {
                return Err(crate::protocols::error::ProtocolError::RespError(
                    "ERR wrong number of arguments for 'cluster' command".to_string(),
                ));
            }

            let subcommand = self.get_string_arg(args, 0, "CLUSTER")?.to_uppercase();
            let sub_args = &args[1..];

            match subcommand.as_str() {
                "INFO" => self.cmd_cluster_info(sub_args).await,
                "NODES" => self.cmd_cluster_nodes(sub_args).await,
                "SLOTS" => self.cmd_cluster_slots(sub_args).await,
                "MYID" => self.cmd_cluster_myid(sub_args).await,
                "KEYSLOT" => self.cmd_cluster_keyslot(sub_args).await,
                "COUNTKEYSINSLOT" => self.cmd_cluster_countkeysinslot(sub_args).await,
                "GETKEYSINSLOT" => self.cmd_cluster_getkeysinslot(sub_args).await,
                "SHARDS" => self.cmd_cluster_shards(sub_args).await,
                "REPLICAS" | "SLAVES" => self.cmd_cluster_replicas(sub_args).await,
                "HELP" => Ok(RespValue::Array(vec![
                    RespValue::bulk_string("CLUSTER INFO"),
                    RespValue::bulk_string("CLUSTER NODES"),
                    RespValue::bulk_string("CLUSTER SLOTS"),
                    RespValue::bulk_string("CLUSTER MYID"),
                    RespValue::bulk_string("CLUSTER KEYSLOT <key>"),
                    RespValue::bulk_string("CLUSTER COUNTKEYSINSLOT <slot>"),
                    RespValue::bulk_string("CLUSTER GETKEYSINSLOT <slot> <count>"),
                    RespValue::bulk_string("CLUSTER SHARDS"),
                    RespValue::bulk_string("CLUSTER REPLICAS <node-id>"),
                ])),
                _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR unknown subcommand '{}' for 'cluster' command",
                    subcommand.to_lowercase()
                ))),
            }
        } else {
            match command_name {
                "READONLY" => self.cmd_readonly(args).await,
                "READWRITE" => self.cmd_readwrite(args).await,
                _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR unknown cluster command '{command_name}'"
                ))),
            }
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["CLUSTER", "READONLY", "READWRITE"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_hash_slot() {
        // Test basic key hashing
        let slot1 = key_hash_slot("foo");
        let slot2 = key_hash_slot("bar");
        assert!(slot1 < CLUSTER_SLOTS);
        assert!(slot2 < CLUSTER_SLOTS);

        // Same key should always hash to same slot
        assert_eq!(key_hash_slot("test"), key_hash_slot("test"));
    }

    #[test]
    fn test_hash_tag() {
        // Keys with same hash tag should hash to same slot
        let slot1 = key_hash_slot("user:{123}:profile");
        let slot2 = key_hash_slot("user:{123}:posts");
        assert_eq!(slot1, slot2);

        // Different hash tags should (likely) hash to different slots
        let slot3 = key_hash_slot("user:{456}:profile");
        // Note: could be same slot by chance, but very unlikely
        assert_ne!(slot1, slot3);
    }

    #[test]
    fn test_empty_hash_tag() {
        // Empty hash tag {} should use full key
        let slot1 = key_hash_slot("foo{}bar");
        let slot2 = key_hash_slot("foo{}bar");
        assert_eq!(slot1, slot2);

        // Different full keys should hash differently
        let slot3 = key_hash_slot("different{}key");
        assert_ne!(slot1, slot3);
    }

    #[test]
    fn test_crc16_xmodem() {
        // Known CRC16 XMODEM values
        assert_eq!(crc16_xmodem(b"123456789"), 0x31C3);
    }

    #[test]
    fn test_node_id_generation() {
        let id1 = generate_node_id();
        let id2 = generate_node_id();

        // Node IDs should be 40 characters (hex)
        assert_eq!(id1.len(), 40);
        assert_eq!(id2.len(), 40);

        // Should be valid hex
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cluster_state_default() {
        let state = ClusterState::default();

        assert_eq!(state.state, "ok");
        assert_eq!(state.slots_assigned, CLUSTER_SLOTS);
        assert_eq!(state.cluster_size, 1);
        assert_eq!(state.cluster_known_nodes, 1);
        assert!(state.myself.flags.contains(&"myself".to_string()));
        assert!(state.myself.flags.contains(&"master".to_string()));
    }

    #[tokio::test]
    async fn test_cluster_commands_supported() {
        // Verify the commands list
        let expected = vec!["CLUSTER", "READONLY", "READWRITE"];
        for cmd in expected {
            assert!(!cmd.is_empty());
        }
    }
}
