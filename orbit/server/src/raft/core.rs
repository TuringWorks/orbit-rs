//! A Raft consensus core with no I/O: leader election (with pre-vote and leader stickiness), log
//! replication with conflict hints, commit rules, snapshots for lagging followers and single-server
//! membership changes.
//!
//! The caller owns time, storage and the network. It calls [`Raft::tick`] at a fixed interval,
//! feeds received messages to [`Raft::step`], proposes with [`Raft::propose`], and after each of
//! those calls drains [`Raft::ready`]. Everything in a `Ready` must be handled in this order:
//! persist the hard state and log changes, then send the messages, then apply the committed
//! entries. Because there are no threads or sockets here, a cluster can be simulated
//! deterministically, with message loss, reordering, partitions and crashes.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type NodeId = u64;
pub type Term = u64;
pub type Index = u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Payload {
    /// Appended by a new leader so entries of earlier terms can be committed.
    Noop,
    Data(Vec<u8>),
    /// The complete voting membership from this entry on.
    Config(Vec<NodeId>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub term: Term,
    pub index: Index,
    pub payload: Payload,
}

/// What a snapshot stands for: the log is replaced up to `index`, whose entry had `term`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub index: Index,
    pub term: Term,
    pub voters: Vec<NodeId>,
}

/// The part of the state that must survive a restart before any reply is sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardState {
    pub term: Term,
    pub voted_for: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub from: NodeId,
    pub to: NodeId,
    pub term: Term,
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    PreVote {
        last_index: Index,
        last_term: Term,
    },
    PreVoteReply {
        granted: bool,
    },
    Vote {
        last_index: Index,
        last_term: Term,
    },
    VoteReply {
        granted: bool,
    },
    Append {
        prev_index: Index,
        prev_term: Term,
        entries: Vec<Entry>,
        commit: Index,
    },
    /// `match_index` is the last index known to match the leader's log; on a rejection `hint` is the
    /// last index at which the follower thinks the logs may agree.
    AppendReply {
        success: bool,
        match_index: Index,
        hint: Index,
    },
    Snapshot {
        meta: SnapshotMeta,
        data: Vec<u8>,
    },
    SnapshotReply {
        match_index: Index,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Follower,
    PreCandidate,
    Candidate,
    Leader,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// A node waits at least this many ticks without hearing from a leader before it campaigns;
    /// the actual wait is random in `[election_ticks, 2 * election_ticks)`.
    pub election_ticks: u64,
    pub heartbeat_ticks: u64,
    /// The most entries sent in one append.
    pub max_batch: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            election_ticks: 10,
            heartbeat_ticks: 2,
            max_batch: 256,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposeError {
    NotLeader {
        leader: Option<NodeId>,
    },
    /// A membership change is already in flight, or the change is not one server at a time.
    BadConfigChange(String),
}

/// What the caller has to do after driving the core. See the module documentation for the order.
#[derive(Default, Debug)]
pub struct Ready {
    pub hard_state: Option<HardState>,
    /// Delete log entries with index at or above this before appending.
    pub truncate_from: Option<Index>,
    pub append: Vec<Entry>,
    /// A snapshot from the leader to install; the log is empty after it.
    pub restore: Option<(SnapshotMeta, Vec<u8>)>,
    pub messages: Vec<Envelope>,
    /// Entries that are committed and not yet handed out, in order.
    pub committed: Vec<Entry>,
    /// Followers that need a snapshot: build one and pass it to [`Raft::send_snapshot`].
    pub snapshot_requests: Vec<NodeId>,
}

impl Ready {
    pub fn is_empty(&self) -> bool {
        self.hard_state.is_none()
            && self.truncate_from.is_none()
            && self.append.is_empty()
            && self.restore.is_none()
            && self.messages.is_empty()
            && self.committed.is_empty()
            && self.snapshot_requests.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flow {
    /// The follower's position is not known: one append at a time.
    Probe,
    /// The follower is keeping up: entries are sent without waiting for each reply.
    Replicate,
}

#[derive(Clone, Debug)]
struct Progress {
    next: Index,
    matched: Index,
    flow: Flow,
    /// A probe is outstanding; nothing more is sent until the reply or the next heartbeat.
    paused: bool,
    snapshot_pending: bool,
    last_ack_tick: u64,
}

impl Progress {
    fn new(next: Index) -> Self {
        Self {
            next,
            matched: 0,
            flow: Flow::Probe,
            paused: false,
            snapshot_pending: false,
            last_ack_tick: 0,
        }
    }
}

/// A small deterministic generator, so a seeded simulation replays exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

pub struct Raft {
    id: NodeId,
    opts: Options,
    voters: Vec<NodeId>,
    role: Role,
    term: Term,
    voted_for: Option<NodeId>,
    leader: Option<NodeId>,

    /// Index and term of the entry just before `log[0]` (the snapshot point).
    base_index: Index,
    base_term: Term,
    /// Voters as of the snapshot point.
    base_voters: Vec<NodeId>,
    log: Vec<Entry>,
    commit: Index,
    /// Entries up to here have been handed out in a `Ready`.
    handed_out: Index,

    election_elapsed: u64,
    heartbeat_elapsed: u64,
    election_timeout: u64,
    now: u64,
    votes: BTreeSet<NodeId>,
    denied: BTreeSet<NodeId>,
    progress: BTreeMap<NodeId, Progress>,
    /// Voters that answered since the last quorum check (leader only).
    active: BTreeSet<NodeId>,
    /// Index of an uncommitted membership change, if any.
    pending_config: Option<Index>,
    rng: Rng,

    // output
    out_hard: bool,
    out_truncate: Option<Index>,
    out_append: Vec<Entry>,
    out_restore: Option<(SnapshotMeta, Vec<u8>)>,
    out_messages: Vec<Envelope>,
    out_snapshot_requests: Vec<NodeId>,
}

impl Raft {
    /// Start a node from what it persisted. `voters` is the initial membership of a fresh cluster
    /// (empty for a node that will be added to a running one); it is ignored once the log or a
    /// snapshot carries a configuration. `applied` is how far the state machine already is.
    pub fn new(
        id: NodeId,
        voters: Vec<NodeId>,
        hard: HardState,
        snapshot: Option<SnapshotMeta>,
        entries: Vec<Entry>,
        applied: Index,
        opts: Options,
        seed: u64,
    ) -> Self {
        let (base_index, base_term, base_voters) = match snapshot {
            Some(m) => (m.index, m.term, m.voters),
            None => (0, 0, voters),
        };
        let mut r = Self {
            id,
            opts,
            voters: base_voters.clone(),
            role: Role::Follower,
            term: hard.term,
            voted_for: hard.voted_for,
            leader: None,
            base_index,
            base_term,
            base_voters,
            log: entries,
            commit: applied.max(base_index),
            handed_out: applied.max(base_index),
            election_elapsed: 0,
            heartbeat_elapsed: 0,
            election_timeout: 0,
            now: 0,
            votes: BTreeSet::new(),
            denied: BTreeSet::new(),
            progress: BTreeMap::new(),
            active: BTreeSet::new(),
            pending_config: None,
            rng: Rng(seed ^ (id.wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1),
            out_hard: false,
            out_truncate: None,
            out_append: Vec::new(),
            out_restore: None,
            out_messages: Vec::new(),
            out_snapshot_requests: Vec::new(),
        };
        // A fresh cluster's membership is recorded as the first log entry, the same on every founding
        // node, so that it survives a restart like any other configuration. A node that joins a
        // running cluster starts with no voters and learns the membership from the leader.
        if r.base_index == 0 && r.log.is_empty() && !r.base_voters.is_empty() {
            let mut founders = r.base_voters.clone();
            founders.sort_unstable();
            let seed_entry = Entry {
                term: 1,
                index: 1,
                payload: Payload::Config(founders),
            };
            r.log.push(seed_entry.clone());
            r.out_append.push(seed_entry);
            if r.term == 0 {
                r.term = 1;
                r.out_hard = true;
            }
            r.commit = r.commit.max(1);
        }
        r.recompute_voters();
        r.reset_election_timeout();
        r
    }

    // ---- inspection ----

    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn role(&self) -> Role {
        self.role
    }
    pub fn term(&self) -> Term {
        self.term
    }
    pub fn leader(&self) -> Option<NodeId> {
        self.leader
    }
    pub fn is_leader(&self) -> bool {
        self.role == Role::Leader
    }
    pub fn commit_index(&self) -> Index {
        self.commit
    }
    pub fn last_index(&self) -> Index {
        self.base_index + self.log.len() as Index
    }
    pub fn voters(&self) -> &[NodeId] {
        &self.voters
    }
    pub fn snapshot_index(&self) -> Index {
        self.base_index
    }

    /// Term of the entry at `index`, if it is in the log or is the snapshot point.
    pub fn term_at(&self, index: Index) -> Option<Term> {
        if index == self.base_index {
            return Some(self.base_term);
        }
        if index < self.base_index || index > self.last_index() {
            return None;
        }
        Some(self.log[(index - self.base_index - 1) as usize].term)
    }

    fn last_term(&self) -> Term {
        self.term_at(self.last_index()).unwrap_or(0)
    }

    /// The entries `from..=to` (clamped to the log).
    pub fn entries(&self, from: Index, to: Index) -> Vec<Entry> {
        let from = from.max(self.base_index + 1);
        let to = to.min(self.last_index());
        if from > to {
            return Vec::new();
        }
        self.log[(from - self.base_index - 1) as usize..(to - self.base_index) as usize].to_vec()
    }

    /// Whether this leader has heard from a majority recently enough to answer reads without asking
    /// anyone: the check happens within half the election timeout, a margin for unsynchronised clocks.
    pub fn lease_valid(&self) -> bool {
        if self.role != Role::Leader {
            return false;
        }
        let quorum = self.quorum();
        let mut acks: Vec<u64> = self
            .voters
            .iter()
            .map(|v| {
                if *v == self.id {
                    self.now
                } else {
                    self.progress.get(v).map_or(0, |p| p.last_ack_tick)
                }
            })
            .collect();
        acks.sort_unstable_by(|a, b| b.cmp(a));
        match acks.get(quorum - 1) {
            Some(t) => self.now.saturating_sub(*t) < (self.opts.election_ticks / 2).max(1),
            None => false,
        }
    }

    fn quorum(&self) -> usize {
        self.voters.len() / 2 + 1
    }

    fn is_voter(&self, id: NodeId) -> bool {
        self.voters.contains(&id)
    }

    // ---- driving ----

    pub fn tick(&mut self) {
        self.now += 1;
        match self.role {
            Role::Leader => {
                self.heartbeat_elapsed += 1;
                self.election_elapsed += 1;
                if self.election_elapsed >= self.opts.election_ticks {
                    // check quorum: a leader that cannot reach a majority stops leading
                    self.election_elapsed = 0;
                    let reached = self
                        .voters
                        .iter()
                        .filter(|v| **v == self.id || self.active.contains(v))
                        .count();
                    self.active.clear();
                    if reached < self.quorum() {
                        let t = self.term;
                        self.become_follower(t, None);
                        return;
                    }
                }
                if self.heartbeat_elapsed >= self.opts.heartbeat_ticks {
                    self.heartbeat_elapsed = 0;
                    for p in self.progress.values_mut() {
                        p.paused = false;
                    }
                    self.broadcast_append();
                }
            }
            _ => {
                self.election_elapsed += 1;
                if self.is_voter(self.id) && self.election_elapsed >= self.election_timeout {
                    self.campaign();
                }
            }
        }
    }

    /// Append `data` to the replicated log. Returns the index it will have if it commits.
    pub fn propose(&mut self, data: Vec<u8>) -> Result<Index, ProposeError> {
        if self.role != Role::Leader {
            return Err(ProposeError::NotLeader {
                leader: self.leader,
            });
        }
        Ok(self.append_local(Payload::Data(data)))
    }

    /// Change the membership by exactly one server. Takes effect when the entry is appended.
    pub fn propose_config(&mut self, voters: Vec<NodeId>) -> Result<Index, ProposeError> {
        if self.role != Role::Leader {
            return Err(ProposeError::NotLeader {
                leader: self.leader,
            });
        }
        if self.pending_config.is_some() {
            return Err(ProposeError::BadConfigChange(
                "another membership change is not committed yet".into(),
            ));
        }
        let now: BTreeSet<_> = self.voters.iter().copied().collect();
        let next: BTreeSet<_> = voters.iter().copied().collect();
        if next.is_empty() || now.symmetric_difference(&next).count() != 1 {
            return Err(ProposeError::BadConfigChange(
                "add or remove exactly one server at a time".into(),
            ));
        }
        let mut sorted: Vec<NodeId> = next.into_iter().collect();
        sorted.sort_unstable();
        let idx = self.append_local(Payload::Config(sorted));
        self.pending_config = Some(idx);
        Ok(idx)
    }

    /// Give the leader a snapshot to send to `peer` (see `Ready::snapshot_requests`). `meta.index`
    /// must be an index the state machine has applied.
    pub fn send_snapshot(&mut self, peer: NodeId, meta: SnapshotMeta, data: Vec<u8>) {
        if self.role != Role::Leader {
            return;
        }
        let Some(p) = self.progress.get_mut(&peer) else {
            return;
        };
        p.snapshot_pending = true;
        let term = self.term;
        self.out_messages.push(Envelope {
            from: self.id,
            to: peer,
            term,
            body: Body::Snapshot { meta, data },
        });
    }

    /// The snapshot asked for by `Ready::snapshot_requests` could not be built; ask again later.
    pub fn snapshot_failed(&mut self, peer: NodeId) {
        if let Some(p) = self.progress.get_mut(&peer) {
            p.snapshot_pending = false;
        }
    }

    /// Drop log entries up to and including `index` (which the state machine has applied) to bound
    /// the log. Followers that still need them are sent a snapshot instead.
    pub fn compact(&mut self, index: Index) {
        if index <= self.base_index || index > self.commit.min(self.handed_out) {
            return;
        }
        let Some(term) = self.term_at(index) else {
            return;
        };
        let voters = self.config_at(index);
        let drop = (index - self.base_index) as usize;
        self.log.drain(..drop);
        self.base_index = index;
        self.base_term = term;
        self.base_voters = voters;
    }

    pub fn snapshot_meta_at(&self, index: Index) -> Option<SnapshotMeta> {
        Some(SnapshotMeta {
            index,
            term: self.term_at(index)?,
            voters: self.config_at(index),
        })
    }

    /// The voting membership in force at `index`.
    fn config_at(&self, index: Index) -> Vec<NodeId> {
        let upto = (index.saturating_sub(self.base_index)) as usize;
        for e in self.log[..upto.min(self.log.len())].iter().rev() {
            if let Payload::Config(v) = &e.payload {
                return v.clone();
            }
        }
        self.base_voters.clone()
    }

    pub fn ready(&mut self) -> Ready {
        let committed = if self.commit > self.handed_out {
            let from = self.handed_out + 1;
            let es = self.entries(from, self.commit);
            self.handed_out = self.commit;
            es
        } else {
            Vec::new()
        };
        Ready {
            hard_state: std::mem::take(&mut self.out_hard).then_some(HardState {
                term: self.term,
                voted_for: self.voted_for,
            }),
            truncate_from: self.out_truncate.take(),
            append: std::mem::take(&mut self.out_append),
            restore: self.out_restore.take(),
            messages: std::mem::take(&mut self.out_messages),
            committed,
            snapshot_requests: std::mem::take(&mut self.out_snapshot_requests),
        }
    }

    // ---- state changes ----

    fn reset_election_timeout(&mut self) {
        let e = self.opts.election_ticks.max(1);
        self.election_timeout = e + self.rng.next() % e;
        self.election_elapsed = 0;
    }

    fn set_term(&mut self, term: Term) {
        if term != self.term {
            self.term = term;
            self.voted_for = None;
            self.out_hard = true;
        }
    }

    fn become_follower(&mut self, term: Term, leader: Option<NodeId>) {
        self.set_term(term);
        self.role = Role::Follower;
        self.leader = leader;
        self.votes.clear();
        self.denied.clear();
        self.progress.clear();
        self.active.clear();
        self.reset_election_timeout();
    }

    fn campaign(&mut self) {
        // pre-vote: ask whether we could win before disturbing anyone with a higher term
        self.role = Role::PreCandidate;
        self.leader = None;
        self.votes.clear();
        self.denied.clear();
        self.votes.insert(self.id);
        self.reset_election_timeout();
        if self.votes.len() >= self.quorum() {
            self.become_candidate();
            return;
        }
        let (li, lt) = (self.last_index(), self.last_term());
        let term = self.term + 1;
        for v in self.voters.clone() {
            if v != self.id {
                self.send(
                    v,
                    term,
                    Body::PreVote {
                        last_index: li,
                        last_term: lt,
                    },
                );
            }
        }
    }

    fn become_candidate(&mut self) {
        let term = self.term + 1;
        self.set_term(term);
        self.role = Role::Candidate;
        self.leader = None;
        self.voted_for = Some(self.id);
        self.out_hard = true;
        self.votes.clear();
        self.denied.clear();
        self.votes.insert(self.id);
        self.reset_election_timeout();
        if self.votes.len() >= self.quorum() {
            self.become_leader();
            return;
        }
        let (li, lt) = (self.last_index(), self.last_term());
        for v in self.voters.clone() {
            if v != self.id {
                self.send(
                    v,
                    term,
                    Body::Vote {
                        last_index: li,
                        last_term: lt,
                    },
                );
            }
        }
    }

    fn become_leader(&mut self) {
        self.role = Role::Leader;
        self.leader = Some(self.id);
        self.votes.clear();
        self.election_elapsed = 0;
        self.heartbeat_elapsed = 0;
        let next = self.last_index() + 1;
        self.progress = self
            .voters
            .iter()
            .filter(|v| **v != self.id)
            .map(|v| (*v, Progress::new(next)))
            .collect();
        self.active.clear();
        // a membership change from an earlier leader may still be uncommitted
        self.pending_config = self
            .log
            .iter()
            .rev()
            .find(|e| matches!(e.payload, Payload::Config(_)) && e.index > self.commit)
            .map(|e| e.index);
        self.append_local(Payload::Noop);
    }

    fn append_local(&mut self, payload: Payload) -> Index {
        let index = self.last_index() + 1;
        let e = Entry {
            term: self.term,
            index,
            payload,
        };
        self.log.push(e.clone());
        self.out_append.push(e);
        self.recompute_voters();
        self.sync_progress();
        self.maybe_commit();
        self.broadcast_append();
        index
    }

    /// Voters follow the last configuration entry in the log, committed or not.
    fn recompute_voters(&mut self) {
        let v = self.config_at(self.last_index());
        self.voters = v;
    }

    fn sync_progress(&mut self) {
        if self.role != Role::Leader {
            return;
        }
        let next = self.last_index() + 1;
        let voters = self.voters.clone();
        self.progress.retain(|id, _| voters.contains(id));
        for v in voters {
            if v != self.id {
                self.progress
                    .entry(v)
                    .or_insert_with(|| Progress::new(next));
            }
        }
    }

    fn maybe_commit(&mut self) {
        if self.role != Role::Leader {
            return;
        }
        let mut matched: Vec<Index> = self
            .voters
            .iter()
            .map(|v| {
                if *v == self.id {
                    self.last_index()
                } else {
                    self.progress.get(v).map_or(0, |p| p.matched)
                }
            })
            .collect();
        matched.sort_unstable_by(|a, b| b.cmp(a));
        let Some(&n) = matched.get(self.quorum() - 1) else {
            return;
        };
        // only an entry of the leader's own term is committed by counting replicas
        if n > self.commit && self.term_at(n) == Some(self.term) {
            self.commit = n;
            if let Some(pc) = self.pending_config {
                if pc <= self.commit {
                    self.pending_config = None;
                    if !self.is_voter(self.id) {
                        // this server has been removed: hand over by stepping down
                        let t = self.term;
                        self.become_follower(t, None);
                    }
                }
            }
        }
    }

    // ---- messages ----

    fn send(&mut self, to: NodeId, term: Term, body: Body) {
        self.out_messages.push(Envelope {
            from: self.id,
            to,
            term,
            body,
        });
    }

    fn broadcast_append(&mut self) {
        let peers: Vec<NodeId> = self.progress.keys().copied().collect();
        for p in peers {
            self.send_append(p);
        }
    }

    fn send_append(&mut self, to: NodeId) {
        let (next, flow, paused, snap) = match self.progress.get(&to) {
            Some(p) => (p.next, p.flow, p.paused, p.snapshot_pending),
            None => return,
        };
        if snap || (flow == Flow::Probe && paused) {
            return;
        }
        let prev_index = next - 1;
        if prev_index < self.base_index {
            // the entries it needs are compacted away
            if let Some(p) = self.progress.get_mut(&to) {
                p.snapshot_pending = true;
            }
            self.out_snapshot_requests.push(to);
            return;
        }
        let prev_term = self.term_at(prev_index).unwrap_or(0);
        let last = self.last_index();
        let upto = last.min(next + self.opts.max_batch as Index - 1);
        let entries = self.entries(next, upto);
        let n = entries.len() as Index;
        let body = Body::Append {
            prev_index,
            prev_term,
            entries,
            commit: self.commit,
        };
        let term = self.term;
        self.send(to, term, body);
        if let Some(p) = self.progress.get_mut(&to) {
            match p.flow {
                Flow::Replicate => p.next += n,
                Flow::Probe => p.paused = true,
            }
        }
    }

    pub fn step(&mut self, m: Envelope) {
        // 1. terms
        if m.term > self.term {
            match &m.body {
                Body::PreVote { .. } => {} // a prospective term: nothing changes until a real vote
                Body::PreVoteReply { granted: true } => {}
                Body::Vote { .. } if self.in_lease() => return, // leader stickiness
                Body::Append { .. } | Body::Snapshot { .. } => {
                    self.become_follower(m.term, Some(m.from))
                }
                _ => self.become_follower(m.term, None),
            }
        } else if m.term < self.term {
            match m.body {
                Body::Append { .. } | Body::Snapshot { .. } => {
                    // an old leader: its next reply carries our term so it steps down
                    let t = self.term;
                    self.send(
                        m.from,
                        t,
                        Body::AppendReply {
                            success: false,
                            match_index: 0,
                            hint: 0,
                        },
                    );
                }
                Body::PreVote { .. } => {
                    let t = self.term;
                    self.send(m.from, t, Body::PreVoteReply { granted: false });
                }
                Body::Vote { .. } => {
                    let t = self.term;
                    self.send(m.from, t, Body::VoteReply { granted: false });
                }
                _ => {}
            }
            return;
        }
        // 2. by kind
        match m.body {
            Body::PreVote {
                last_index,
                last_term,
            } => self.on_prevote(m.from, m.term, last_index, last_term),
            Body::PreVoteReply { granted } => self.on_prevote_reply(m.from, m.term, granted),
            Body::Vote {
                last_index,
                last_term,
            } => self.on_vote(m.from, last_index, last_term),
            Body::VoteReply { granted } => self.on_vote_reply(m.from, granted),
            Body::Append {
                prev_index,
                prev_term,
                entries,
                commit,
            } => self.on_append(m.from, prev_index, prev_term, entries, commit),
            Body::AppendReply {
                success,
                match_index,
                hint,
            } => self.on_append_reply(m.from, success, match_index, hint),
            Body::Snapshot { meta, data } => self.on_snapshot(m.from, meta, data),
            Body::SnapshotReply { match_index } => self.on_snapshot_reply(m.from, match_index),
        }
    }

    /// Someone is leading and has been heard from lately: a vote request is ignored.
    fn in_lease(&self) -> bool {
        self.leader.is_some() && self.election_elapsed < self.opts.election_ticks
    }

    fn log_up_to_date(&self, last_index: Index, last_term: Term) -> bool {
        last_term > self.last_term()
            || (last_term == self.last_term() && last_index >= self.last_index())
    }

    fn on_prevote(&mut self, from: NodeId, term: Term, last_index: Index, last_term: Term) {
        let grant =
            term > self.term && !self.in_lease() && self.log_up_to_date(last_index, last_term);
        let reply_term = if grant { term } else { self.term };
        self.send(from, reply_term, Body::PreVoteReply { granted: grant });
    }

    fn on_prevote_reply(&mut self, from: NodeId, term: Term, granted: bool) {
        if self.role != Role::PreCandidate || term != self.term + 1 && granted {
            return;
        }
        if granted {
            self.votes.insert(from);
            if self.votes.len() >= self.quorum() {
                self.become_candidate();
            }
        } else {
            self.denied.insert(from);
        }
    }

    fn on_vote(&mut self, from: NodeId, last_index: Index, last_term: Term) {
        let free = self.voted_for.is_none() || self.voted_for == Some(from);
        let grant = free && self.log_up_to_date(last_index, last_term) && self.is_voter(from);
        if grant {
            self.voted_for = Some(from);
            self.out_hard = true;
            self.election_elapsed = 0;
        }
        let t = self.term;
        self.send(from, t, Body::VoteReply { granted: grant });
    }

    fn on_vote_reply(&mut self, from: NodeId, granted: bool) {
        if self.role != Role::Candidate {
            return;
        }
        if granted {
            self.votes.insert(from);
            if self.votes.len() >= self.quorum() {
                self.become_leader();
            }
        } else {
            self.denied.insert(from);
            if self.denied.len() >= self.quorum() {
                let t = self.term;
                self.become_follower(t, None);
            }
        }
    }

    fn on_append(
        &mut self,
        from: NodeId,
        prev_index: Index,
        prev_term: Term,
        entries: Vec<Entry>,
        leader_commit: Index,
    ) {
        if self.role != Role::Follower {
            let t = self.term;
            self.become_follower(t, Some(from));
        }
        self.leader = Some(from);
        self.election_elapsed = 0;
        let term = self.term;
        if prev_index < self.commit {
            // we already have everything up to commit; tell the leader where we are
            let c = self.commit;
            self.send(
                from,
                term,
                Body::AppendReply {
                    success: true,
                    match_index: c,
                    hint: 0,
                },
            );
            return;
        }
        if self.term_at(prev_index) != Some(prev_term) {
            let hint = if prev_index > self.last_index() {
                self.last_index()
            } else {
                // skip back over the whole conflicting term
                let bad = self.term_at(prev_index).unwrap_or(0);
                let mut i = prev_index;
                while i > self.commit.max(self.base_index + 1) && self.term_at(i - 1) == Some(bad) {
                    i -= 1;
                }
                (i - 1).max(self.commit)
            };
            self.send(
                from,
                term,
                Body::AppendReply {
                    success: false,
                    match_index: 0,
                    hint,
                },
            );
            return;
        }
        let last_new = prev_index + entries.len() as Index;
        let mut changed_config = false;
        for e in entries {
            if e.index <= self.last_index() {
                if self.term_at(e.index) == Some(e.term) {
                    continue;
                }
                // a conflict: drop our entry and everything after it
                debug_assert!(
                    e.index > self.commit,
                    "a committed entry must never be overwritten"
                );
                let keep = (e.index - self.base_index - 1) as usize;
                self.log.truncate(keep);
                self.out_truncate = Some(self.out_truncate.map_or(e.index, |t| t.min(e.index)));
                self.out_append.retain(|a| a.index < e.index);
                changed_config = true;
            }
            if matches!(e.payload, Payload::Config(_)) {
                changed_config = true;
            }
            self.log.push(e.clone());
            self.out_append.push(e);
        }
        if changed_config {
            self.recompute_voters();
        }
        if leader_commit > self.commit {
            self.commit = leader_commit.min(last_new);
        }
        self.send(
            from,
            term,
            Body::AppendReply {
                success: true,
                match_index: last_new,
                hint: 0,
            },
        );
    }

    fn on_append_reply(&mut self, from: NodeId, success: bool, match_index: Index, hint: Index) {
        if self.role != Role::Leader {
            return;
        }
        let now = self.now;
        let Some(p) = self.progress.get_mut(&from) else {
            return;
        };
        p.last_ack_tick = now;
        self.active.insert(from);
        if success {
            if match_index > p.matched {
                p.matched = match_index;
            }
            p.next = p.next.max(p.matched + 1);
            p.flow = Flow::Replicate;
            p.paused = false;
            let more = p.next <= self.last_index();
            let before = self.commit;
            self.maybe_commit();
            if self.commit > before {
                // let every follower learn the new commit index now rather than at the next heartbeat
                self.broadcast_append();
            } else if more {
                self.send_append(from);
            }
        } else {
            // the log did not match: step back to where the follower says it may agree
            p.flow = Flow::Probe;
            p.paused = false;
            p.next = (hint + 1)
                .min(p.next.saturating_sub(1))
                .max(p.matched + 1)
                .max(1);
            self.send_append(from);
        }
    }

    fn on_snapshot(&mut self, from: NodeId, meta: SnapshotMeta, data: Vec<u8>) {
        if self.role != Role::Follower {
            let t = self.term;
            self.become_follower(t, Some(from));
        }
        self.leader = Some(from);
        self.election_elapsed = 0;
        let term = self.term;
        if meta.index <= self.commit {
            let c = self.commit;
            self.send(from, term, Body::SnapshotReply { match_index: c });
            return;
        }
        // the snapshot replaces the log
        self.log.clear();
        self.out_append.clear();
        self.out_truncate = Some(0);
        self.base_index = meta.index;
        self.base_term = meta.term;
        self.base_voters = meta.voters.clone();
        self.commit = meta.index;
        self.handed_out = meta.index;
        self.recompute_voters();
        let reply = meta.index;
        self.out_restore = Some((meta, data));
        self.send(from, term, Body::SnapshotReply { match_index: reply });
    }

    fn on_snapshot_reply(&mut self, from: NodeId, match_index: Index) {
        if self.role != Role::Leader {
            return;
        }
        let now = self.now;
        let Some(p) = self.progress.get_mut(&from) else {
            return;
        };
        p.last_ack_tick = now;
        self.active.insert(from);
        p.snapshot_pending = false;
        p.matched = p.matched.max(match_index);
        p.next = p.matched + 1;
        p.flow = Flow::Probe;
        p.paused = false;
        self.maybe_commit();
        self.send_append(from);
    }
}
