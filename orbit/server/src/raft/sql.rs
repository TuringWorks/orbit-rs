//! Connects the durable SQL engine to Raft: a replicated store where a commit goes through the
//! consensus log, every node applies committed transactions to its own RocksDB store and in-memory
//! state, and only the leader takes writes.

use super::core::{Index, NodeId};
use super::net::{ListenHandle, TcpTransport};
use super::node::{NodeConfig, RaftError, RaftNode, StateMachine};
use super::storage::RocksRaftStorage;
use crate::protocols::postgres_wire::sql::durable::{
    DurableEngine, Loaded, Op, Persist, RocksPersist,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};

/// The engine, filled in once it has been opened over the store that needs this cell. The engine
/// owns its store, so the store only holds the engine weakly: otherwise the two would keep each
/// other (and the database lock) alive forever.
pub type EngineCell = Arc<OnceLock<Weak<DurableEngine>>>;
/// The node, filled in once it has started with a state machine that needs the engine.
pub type NodeCell = Arc<OnceLock<RaftNode>>;

/// Applies committed log entries to the SQL store and the engine's in-memory state.
pub struct SqlStateMachine {
    store: Arc<RocksPersist>,
    engine: EngineCell,
    /// The last entry applied. It can run ahead of the store's own index across entries that carry no
    /// transaction (a leader's no-op, a membership change); those need not be written.
    applied: AtomicU64,
    /// Told where nodes listen, as that is learned from the log or a snapshot.
    on_peer: Arc<dyn Fn(NodeId, &str) + Send + Sync>,
}

impl SqlStateMachine {
    pub fn new(
        store: Arc<RocksPersist>,
        engine: EngineCell,
        on_peer: Arc<dyn Fn(NodeId, &str) + Send + Sync>,
    ) -> Self {
        let applied = AtomicU64::new(store.applied_index());
        Self {
            store,
            engine,
            applied,
            on_peer,
        }
    }

    fn engine(&self) -> Result<Arc<DurableEngine>, String> {
        self.engine
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| "the SQL engine is not open".to_string())
    }
}

impl StateMachine for SqlStateMachine {
    fn apply(&self, index: Index, data: Option<&[u8]>) -> Result<(), String> {
        if let Some(bytes) = data {
            let ops: Vec<Op> =
                serde_json::from_slice(bytes).map_err(|e| format!("decode transaction: {e}"))?;
            // durable first, together with the log index, then visible to queries
            self.store.apply(&ops, index)?;
            for op in &ops {
                if let Op::NodeAddr { id, addr } = op {
                    (self.on_peer)(*id, addr);
                }
            }
            self.engine()?.apply_replicated(&ops)?;
        }
        self.applied.store(index, Ordering::SeqCst);
        Ok(())
    }

    fn applied_index(&self) -> Index {
        self.applied.load(Ordering::SeqCst)
    }

    fn snapshot(&self) -> Result<(Index, Vec<u8>), String> {
        let (index, loaded) = self.store.snapshot()?;
        Ok((
            index,
            serde_json::to_vec(&loaded).map_err(|e| format!("encode snapshot: {e}"))?,
        ))
    }

    fn restore(&self, index: Index, data: &[u8]) -> Result<(), String> {
        let loaded: Loaded =
            serde_json::from_slice(data).map_err(|e| format!("decode snapshot: {e}"))?;
        self.store.restore(index, &loaded)?;
        for (id, addr) in &loaded.peers {
            (self.on_peer)(*id, addr);
        }
        self.engine()?.reload(loaded)?;
        self.applied.store(index, Ordering::SeqCst);
        Ok(())
    }
}

/// The engine's store when it runs in a cluster.
pub struct RaftPersist {
    store: Arc<RocksPersist>,
    node: NodeCell,
    engine: EngineCell,
    /// The leadership term for which sequences were last resumed above what any node may have used.
    resumed_term: AtomicU64,
    /// Serve reads from a replica too, accepting that they can be slightly behind the leader.
    stale_reads: bool,
}

impl RaftPersist {
    pub fn new(
        store: Arc<RocksPersist>,
        node: NodeCell,
        engine: EngineCell,
        stale_reads: bool,
    ) -> Self {
        Self {
            store,
            node,
            engine,
            resumed_term: AtomicU64::new(0),
            stale_reads,
        }
    }

    fn not_leader(&self, what: &str) -> String {
        match self.node.get().map(|n| n.status()) {
            Some(s) => match s.leader {
                Some(l) if l != s.id => {
                    format!("[25006] {what}: this node is not the leader; node {l} is")
                }
                _ => format!(
                    "[25006] {what}: this node is not the leader and no leader is available"
                ),
            },
            None => format!("[25006] {what}: the cluster node is not running"),
        }
    }
}

fn describe(e: RaftError) -> String {
    match &e {
        RaftError::NotLeader { .. } => format!("[25006] {e}"),
        RaftError::Uncertain(_) => format!("[40003] {e}"),
        RaftError::Overwritten => format!("[40001] {e}"),
        RaftError::Stopped => format!("[57P01] {e}"),
        RaftError::Invalid(_) => format!("[XX000] {e}"),
    }
}

#[async_trait]
impl Persist for RaftPersist {
    async fn commit(&self, ops: &[Op]) -> Result<(), String> {
        let node = self.node.get().ok_or_else(|| self.not_leader("commit"))?;
        let bytes = serde_json::to_vec(ops).map_err(|e| format!("encode transaction: {e}"))?;
        node.propose(bytes).await.map_err(describe)
    }

    fn load(&self) -> Result<Loaded, String> {
        self.store.load()
    }

    fn backup(&self, dest: &std::path::Path) -> Result<(), String> {
        self.store.backup(dest)
    }

    fn replicated(&self) -> bool {
        true
    }

    fn check_writable(&self) -> Result<(), String> {
        let status = self.node.get().map(|n| n.status());
        match status {
            Some(s) if s.serving => {
                // a new leader must hand out sequence values above anything the previous leader may have used
                // before it takes its first write
                if self.resumed_term.swap(s.term, Ordering::SeqCst) != s.term {
                    if let Some(e) = self.engine.get().and_then(Weak::upgrade) {
                        e.became_leader();
                    }
                }
                Ok(())
            }
            _ => Err(self.not_leader("writes are only accepted by the leader")),
        }
    }

    fn check_readable(&self) -> Result<(), String> {
        match self.node.get().map(|n| n.status()) {
            Some(s) if s.serving => Ok(()),
            Some(_) if self.stale_reads => Ok(()),
            _ => Err(self.not_leader("reads are served by the leader")),
        }
    }
}

/// What a node needs to join (or found) a cluster.
pub struct ClusterConfig {
    pub id: NodeId,
    /// Where this node accepts messages from the others.
    pub listen: String,
    /// Every node known at start, this one included, by id.
    pub peers: HashMap<NodeId, String>,
    /// Start with no voters of its own, to be added to a running cluster.
    pub join: bool,
    /// Shared secret every node presents when it connects.
    pub token: Vec<u8>,
    pub data_dir: PathBuf,
    pub stale_reads: bool,
    pub keep_log: u64,
}

/// A running cluster node: the engine to give the PostgreSQL port, and the controls for operating it.
pub struct Cluster {
    pub engine: Arc<DurableEngine>,
    pub node: RaftNode,
    transport: Arc<TcpTransport>,
    _listener: ListenHandle,
    _thread: std::thread::JoinHandle<()>,
}

impl Cluster {
    pub async fn start(cfg: ClusterConfig) -> Result<Cluster, String> {
        let store = Arc::new(RocksPersist::open(cfg.data_dir.join("sql"))?);
        let mut peers = cfg.peers.clone();
        // addresses learned earlier (from the log) take the place of the flags for nodes added since
        for (id, addr) in store.load()?.peers {
            peers.entry(id).or_insert(addr);
        }
        let transport = TcpTransport::new(peers.clone(), cfg.token.clone());
        let engine_cell: EngineCell = Arc::default();
        let node_cell: NodeCell = Arc::default();
        let t = Arc::clone(&transport);
        let sm = Arc::new(SqlStateMachine::new(
            Arc::clone(&store),
            Arc::clone(&engine_cell),
            Arc::new(move |id, addr| t.set_addr(id, addr.to_string())),
        ));
        let persist = RaftPersist::new(
            Arc::clone(&store),
            Arc::clone(&node_cell),
            Arc::clone(&engine_cell),
            cfg.stale_reads,
        );
        let engine = Arc::new(DurableEngine::open(Box::new(persist))?);
        let _ = engine_cell.set(Arc::downgrade(&engine));

        let voters: Vec<NodeId> = if cfg.join {
            Vec::new()
        } else {
            peers.keys().copied().collect()
        };
        let mut node_cfg = NodeConfig::new(cfg.id, voters);
        node_cfg.keep_log = cfg.keep_log;
        let raft_store = RocksRaftStorage::open(cfg.data_dir.join("raft"))?;
        let (node, thread) =
            RaftNode::start(node_cfg, Box::new(raft_store), sm, transport.clone())?;
        let _ = node_cell.set(node.clone());
        let listener = transport
            .listen(&cfg.listen, node.clone())
            .await
            .map_err(|e| format!("listen on {}: {e}", cfg.listen))?;
        Ok(Cluster {
            engine,
            node,
            transport,
            _listener: listener,
            _thread: thread,
        })
    }

    /// One line of JSON describing this node's view of the cluster.
    pub fn status_json(&self) -> String {
        let s = self.node.status();
        serde_json::json!({
            "id": s.id,
            "role": format!("{:?}", s.role).to_lowercase(),
            "term": s.term,
            "leader": s.leader,
            "voters": s.voters,
            "commit": s.commit,
            "applied": s.applied,
            "serving": s.serving,
            "addresses": self.transport.addrs(),
        })
        .to_string()
    }

    /// Add a server as a voter. Run it on the leader; the new node must already be running with `join`.
    pub async fn add_node(&self, id: NodeId, addr: &str) -> Result<(), String> {
        let status = self.node.status();
        if status.voters.contains(&id) {
            return Err(format!("node {id} is already a member"));
        }
        // the address goes through the log first, so every node (and any future leader) can reach the new one
        self.transport.set_addr(id, addr.to_string());
        let ops = [Op::NodeAddr {
            id,
            addr: addr.to_string(),
        }];
        let bytes = serde_json::to_vec(&ops).map_err(|e| e.to_string())?;
        self.node.propose(bytes).await.map_err(|e| e.to_string())?;
        let mut voters = status.voters;
        voters.push(id);
        voters.sort_unstable();
        self.node
            .change_membership(voters)
            .await
            .map_err(|e| e.to_string())
    }

    /// Remove a voter. Run it on the leader.
    pub async fn remove_node(&self, id: NodeId) -> Result<(), String> {
        let status = self.node.status();
        if !status.voters.contains(&id) {
            return Err(format!("node {id} is not a member"));
        }
        let voters: Vec<NodeId> = status.voters.into_iter().filter(|v| *v != id).collect();
        self.node
            .change_membership(voters)
            .await
            .map_err(|e| e.to_string())
    }
}

#[async_trait]
impl crate::protocols::postgres_wire::query_engine::ClusterAdmin for Cluster {
    fn status(&self) -> String {
        self.status_json()
    }
    async fn add_node(&self, id: u64, addr: &str) -> Result<(), String> {
        Cluster::add_node(self, id, addr).await
    }
    async fn remove_node(&self, id: u64) -> Result<(), String> {
        Cluster::remove_node(self, id).await
    }
}
