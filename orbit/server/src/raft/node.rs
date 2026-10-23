//! Runs a [`Raft`] core against real storage, a state machine and a network.
//!
//! The node owns one thread that holds the core, so persistence and state-machine calls can block
//! without any async plumbing. Everything else talks to it through channels: the network delivers
//! messages with [`RaftNode::deliver`], clients call [`RaftNode::propose`], and state is observed
//! through [`RaftNode::status`].

use super::core::*;
use super::storage::{RaftStorage, Save};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, watch};

/// The replicated state machine a node drives. All methods are called from the node's thread.
pub trait StateMachine: Send + Sync + 'static {
    /// Apply the entry at `index`. `None` is an entry with no command (a leader's no-op or a
    /// membership change): only the position advances. Must be durable, together with the index,
    /// before it returns.
    fn apply(&self, index: Index, data: Option<&[u8]>) -> Result<(), String>;
    /// The last index applied (and durable).
    fn applied_index(&self) -> Index;
    /// A consistent image of the whole state at the returned applied index.
    fn snapshot(&self) -> Result<(Index, Vec<u8>), String>;
    /// Replace the whole state with a snapshot taken at `index`.
    fn restore(&self, index: Index, data: &[u8]) -> Result<(), String>;
}

/// Sends a message to a peer. Delivery is best effort; Raft retries what matters.
pub trait Transport: Send + Sync + 'static {
    fn send(&self, env: Envelope);
}

#[derive(Debug, Clone, PartialEq)]
pub enum RaftError {
    NotLeader {
        leader: Option<NodeId>,
    },
    /// Leadership was lost before the outcome of the proposal was known; it may still commit.
    Uncertain(String),
    /// Another leader's entry took the place of this proposal; it did not commit.
    Overwritten,
    Invalid(String),
    Stopped,
}

impl std::fmt::Display for RaftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaftError::NotLeader { leader: Some(l) } => write!(f, "not the leader; node {l} is"),
            RaftError::NotLeader { leader: None } => {
                write!(f, "not the leader and no leader is known")
            }
            RaftError::Uncertain(why) => write!(f, "outcome unknown: {why}"),
            RaftError::Overwritten => write!(
                f,
                "the write was replaced by another leader's and did not commit"
            ),
            RaftError::Invalid(why) => write!(f, "{why}"),
            RaftError::Stopped => write!(f, "the node has stopped"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub id: NodeId,
    pub role: Role,
    pub term: Term,
    pub leader: Option<NodeId>,
    pub commit: Index,
    pub applied: Index,
    pub voters: Vec<NodeId>,
    /// The node leads, has applied everything committed before its term, and a majority answered
    /// recently: it may serve reads and writes.
    pub serving: bool,
    pub stopped: bool,
}

#[derive(Clone)]
pub struct NodeConfig {
    pub id: NodeId,
    /// Founding membership of a new cluster, or empty for a node that joins a running one.
    pub voters: Vec<NodeId>,
    pub tick: Duration,
    pub options: Options,
    /// Keep roughly this many applied entries; older ones are dropped from the log.
    pub keep_log: u64,
    pub seed: u64,
}

impl NodeConfig {
    pub fn new(id: NodeId, voters: Vec<NodeId>) -> Self {
        Self {
            id,
            voters,
            tick: Duration::from_millis(50),
            options: Options::default(),
            keep_log: 10_000,
            seed: id,
        }
    }
}

enum Cmd {
    Net(Envelope),
    Propose(Vec<u8>, oneshot::Sender<Result<(), RaftError>>),
    Config(Vec<NodeId>, oneshot::Sender<Result<(), RaftError>>),
    Stop,
}

#[derive(Clone)]
pub struct RaftNode {
    tx: mpsc::Sender<Cmd>,
    status: watch::Receiver<Status>,
}

impl RaftNode {
    /// Load what the node persisted and start its thread.
    pub fn start(
        cfg: NodeConfig,
        mut storage: Box<dyn RaftStorage>,
        sm: Arc<dyn StateMachine>,
        transport: Arc<dyn Transport>,
    ) -> Result<(RaftNode, std::thread::JoinHandle<()>), String> {
        let loaded = storage.load()?;
        let applied = sm.applied_index();
        let core = Raft::new(
            cfg.id,
            cfg.voters.clone(),
            loaded.hard,
            loaded.base,
            loaded.entries,
            applied,
            cfg.options.clone(),
            cfg.seed,
        );
        let (tx, rx) = mpsc::channel();
        let (status_tx, status_rx) = watch::channel(Status {
            id: cfg.id,
            role: Role::Follower,
            term: core.term(),
            leader: None,
            commit: core.commit_index(),
            applied,
            voters: core.voters().to_vec(),
            serving: false,
            stopped: false,
        });
        let mut driver = Driver {
            core,
            storage,
            sm,
            transport,
            cfg,
            status_tx,
            waiters: HashMap::new(),
            leader_start: None,
            was_leader: false,
        };
        let handle = std::thread::Builder::new()
            .name(format!("raft-{}", driver.cfg.id))
            .spawn(move || driver.run(rx))
            .map_err(|e| format!("start raft thread: {e}"))?;
        Ok((
            RaftNode {
                tx,
                status: status_rx,
            },
            handle,
        ))
    }

    /// Hand a message received from the network to the node.
    pub fn deliver(&self, env: Envelope) {
        let _ = self.tx.send(Cmd::Net(env));
    }

    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    /// Replicate `data`. Returns when it is committed and applied on this node.
    pub async fn propose(&self, data: Vec<u8>) -> Result<(), RaftError> {
        let (done, wait) = oneshot::channel();
        self.tx
            .send(Cmd::Propose(data, done))
            .map_err(|_| RaftError::Stopped)?;
        wait.await.map_err(|_| RaftError::Stopped)?
    }

    /// Change the voting membership by one server. Returns when the change is committed.
    pub async fn change_membership(&self, voters: Vec<NodeId>) -> Result<(), RaftError> {
        let (done, wait) = oneshot::channel();
        self.tx
            .send(Cmd::Config(voters, done))
            .map_err(|_| RaftError::Stopped)?;
        wait.await.map_err(|_| RaftError::Stopped)?
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

struct Driver {
    core: Raft,
    storage: Box<dyn RaftStorage>,
    sm: Arc<dyn StateMachine>,
    transport: Arc<dyn Transport>,
    cfg: NodeConfig,
    status_tx: watch::Sender<Status>,
    /// Proposals waiting to be applied: index -> (term proposed in, completion).
    waiters: HashMap<Index, (Term, oneshot::Sender<Result<(), RaftError>>)>,
    /// Index of this leader's first entry (its no-op): the node serves once it has applied it.
    leader_start: Option<Index>,
    was_leader: bool,
}

impl Driver {
    fn run(&mut self, rx: mpsc::Receiver<Cmd>) {
        let mut next_tick = Instant::now() + self.cfg.tick;
        self.publish();
        loop {
            let wait = next_tick.saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(cmd) => {
                    if !self.handle(cmd) {
                        return self.stop();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return self.stop(),
            }
            while let Ok(cmd) = rx.try_recv() {
                if !self.handle(cmd) {
                    return self.stop();
                }
            }
            let now = Instant::now();
            if now >= next_tick {
                self.core.tick();
                // do not try to catch up after a long pause: one tick per interval
                next_tick = now + self.cfg.tick;
            }
            if let Err(e) = self.pump() {
                eprintln!("raft node {} stopping: {e}", self.cfg.id);
                return self.stop();
            }
            self.publish();
        }
    }

    fn stop(&mut self) {
        for (_, (_, tx)) in self.waiters.drain() {
            let _ = tx.send(Err(RaftError::Stopped));
        }
        let mut s = self.status_tx.borrow().clone();
        s.stopped = true;
        s.serving = false;
        s.role = Role::Follower;
        let _ = self.status_tx.send(s);
    }

    /// Returns false to stop the node.
    fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Net(env) => self.core.step(env),
            Cmd::Propose(data, done) => match self.core.propose(data) {
                Ok(index) => {
                    self.waiters.insert(index, (self.core.term(), done));
                }
                Err(ProposeError::NotLeader { leader }) => {
                    let _ = done.send(Err(RaftError::NotLeader { leader }));
                }
                Err(ProposeError::BadConfigChange(why)) => {
                    let _ = done.send(Err(RaftError::Invalid(why)));
                }
            },
            Cmd::Config(voters, done) => match self.core.propose_config(voters) {
                Ok(index) => {
                    self.waiters.insert(index, (self.core.term(), done));
                }
                Err(ProposeError::NotLeader { leader }) => {
                    let _ = done.send(Err(RaftError::NotLeader { leader }));
                }
                Err(ProposeError::BadConfigChange(why)) => {
                    let _ = done.send(Err(RaftError::Invalid(why)));
                }
            },
            Cmd::Stop => return false,
        }
        true
    }

    /// Drain everything the core wants done: persist, send, then apply.
    fn pump(&mut self) -> Result<(), String> {
        loop {
            let rd = self.core.ready();
            if rd.is_empty() {
                break;
            }
            self.persist(&rd)?;
            for m in rd.messages {
                self.transport.send(m);
            }
            if let Some((meta, data)) = &rd.restore {
                self.sm.restore(meta.index, data)?;
            }
            for peer in rd.snapshot_requests {
                let (index, data) = self.sm.snapshot()?;
                match self.core.snapshot_meta_at(index) {
                    Some(meta) => self.core.send_snapshot(peer, meta, data),
                    None => self.core.snapshot_failed(peer),
                }
            }
            for e in rd.committed {
                self.apply(e)?;
            }
            self.compact_if_due()?;
        }
        self.track_leadership();
        Ok(())
    }

    fn persist(&mut self, rd: &Ready) -> Result<(), String> {
        let base = rd.restore.as_ref().map(|(m, _)| m);
        if rd.hard_state.is_none()
            && rd.truncate_from.is_none()
            && rd.append.is_empty()
            && base.is_none()
        {
            return Ok(());
        }
        self.storage.save(Save {
            hard: rd.hard_state.as_ref(),
            truncate_from: rd.truncate_from,
            append: &rd.append,
            base,
        })
    }

    fn apply(&mut self, e: Entry) -> Result<(), String> {
        match &e.payload {
            Payload::Data(d) => self.sm.apply(e.index, Some(d))?,
            Payload::Noop | Payload::Config(_) => self.sm.apply(e.index, None)?,
        }
        if let Some((term, done)) = self.waiters.remove(&e.index) {
            let _ = done.send(if term == e.term {
                Ok(())
            } else {
                Err(RaftError::Overwritten)
            });
        }
        Ok(())
    }

    fn compact_if_due(&mut self) -> Result<(), String> {
        let applied = self.sm.applied_index();
        let base = self.core.snapshot_index();
        if self.cfg.keep_log == 0 || applied <= base + self.cfg.keep_log * 2 {
            return Ok(());
        }
        let upto = applied - self.cfg.keep_log;
        if let Some(meta) = self.core.snapshot_meta_at(upto) {
            self.core.compact(upto);
            if self.core.snapshot_index() == upto {
                self.storage.save(Save {
                    base: Some(&meta),
                    ..Save::default()
                })?;
            }
        }
        Ok(())
    }

    /// Note when this node starts leading, and fail proposals when it stops.
    fn track_leadership(&mut self) {
        let leading = self.core.is_leader();
        if leading && !self.was_leader {
            // the no-op appended on election is the last entry now
            self.leader_start = Some(self.core.last_index());
        }
        if !leading && self.was_leader {
            self.leader_start = None;
            for (_, (_, tx)) in self.waiters.drain() {
                let _ = tx.send(Err(RaftError::Uncertain(
                    "leadership was lost before the write was confirmed".into(),
                )));
            }
        }
        if !leading && !self.waiters.is_empty() {
            // a follower cannot have proposals pending (they were failed above or are stale)
            let leader = self.core.leader();
            let stale: Vec<Index> = self.waiters.keys().copied().collect();
            for i in stale {
                if let Some((_, tx)) = self.waiters.remove(&i) {
                    let _ = tx.send(Err(RaftError::NotLeader { leader }));
                }
            }
        }
        self.was_leader = leading;
    }

    fn publish(&mut self) {
        let applied = self.sm.applied_index();
        let serving = self.core.is_leader()
            && self.leader_start.map_or(false, |s| applied >= s)
            && self.core.lease_valid();
        let next = Status {
            id: self.cfg.id,
            role: self.core.role(),
            term: self.core.term(),
            leader: self.core.leader(),
            commit: self.core.commit_index(),
            applied,
            voters: self.core.voters().to_vec(),
            serving,
            stopped: false,
        };
        if *self.status_tx.borrow() != next {
            let _ = self.status_tx.send(next);
        }
    }
}
