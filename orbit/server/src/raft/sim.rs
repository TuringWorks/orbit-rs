//! A deterministic cluster simulator for [`Raft`]: delayed, dropped and duplicated messages,
//! partitions, and crashes in which only persisted state survives. Safety is checked at every step
//! and, after the network heals, liveness and "no acknowledged write is lost".

use super::core::*;
use std::collections::{BTreeMap, BTreeSet};

/// What a node has made durable.
#[derive(Default, Clone)]
struct Disk {
    hard: HardState,
    snapshot: Option<SnapshotMeta>,
    /// Log entries after the snapshot point.
    log: Vec<Entry>,
    /// The state machine, durable together with its applied index: every applied payload in order.
    applied: Vec<(Index, Payload)>,
}

struct Node {
    raft: Option<Raft>, // None while crashed
    disk: Disk,
    restart_at: u64,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

#[derive(Clone)]
pub struct Faults {
    pub drop_percent: u64,
    pub dup_percent: u64,
    pub max_delay: u64,
    pub crash_percent_per_tick_per_1000: u64,
    pub partition_every: u64,
}

pub struct Sim {
    nodes: BTreeMap<NodeId, Node>,
    initial: Vec<NodeId>,
    inflight: Vec<(u64, Envelope)>,
    now: u64,
    rng: Rng,
    seed: u64,
    opts: Options,
    /// Nodes in different groups cannot talk.
    groups: BTreeMap<NodeId, u8>,
    /// Leader per term, for election safety.
    leaders: BTreeMap<Term, NodeId>,
    /// Every entry any node has applied, by index: it must never differ between nodes.
    committed: BTreeMap<Index, (Term, Payload)>,
    /// Proposals the leader reported applied: (payload) that must survive.
    acked: BTreeSet<Vec<u8>>,
    /// Proposals by (node, index, term) waiting to be applied by that node.
    proposed: BTreeMap<(NodeId, Index, Term), Vec<u8>>,
    next_payload: u64,
    faults: Faults,
}

impl Sim {
    pub fn new(n: u64, seed: u64) -> Self {
        let ids: Vec<NodeId> = (1..=n).collect();
        let opts = Options {
            election_ticks: 10,
            heartbeat_ticks: 2,
            max_batch: 8,
        };
        let mut s = Self {
            nodes: BTreeMap::new(),
            initial: ids.clone(),
            inflight: Vec::new(),
            now: 0,
            rng: Rng(seed | 1),
            seed,
            opts,
            groups: BTreeMap::new(),
            leaders: BTreeMap::new(),
            committed: BTreeMap::new(),
            acked: BTreeSet::new(),
            proposed: BTreeMap::new(),
            next_payload: 0,
            faults: Faults {
                drop_percent: 0,
                dup_percent: 0,
                max_delay: 2,
                crash_percent_per_tick_per_1000: 0,
                partition_every: 0,
            },
        };
        for id in ids {
            s.add_node(id, true);
        }
        s
    }

    fn add_node(&mut self, id: NodeId, bootstrap: bool) {
        let voters = if bootstrap {
            self.initial.clone()
        } else {
            Vec::new()
        };
        let raft = Raft::new(
            id,
            voters,
            HardState::default(),
            None,
            Vec::new(),
            0,
            self.opts.clone(),
            self.seed,
        );
        self.nodes.insert(
            id,
            Node {
                raft: Some(raft),
                disk: Disk::default(),
                restart_at: 0,
            },
        );
        self.groups.insert(id, 0);
    }

    fn restart(&mut self, id: NodeId) {
        let voters = self.initial.clone();
        let n = self.nodes.get_mut(&id).unwrap();
        let applied = n.disk.applied.last().map_or(0, |a| a.0);
        let bootstrap = if n.disk.log.is_empty()
            && n.disk.snapshot.is_none()
            && id <= self.initial.len() as u64
        {
            voters
        } else {
            Vec::new()
        };
        n.raft = Some(Raft::new(
            id,
            bootstrap,
            n.disk.hard.clone(),
            n.disk.snapshot.clone(),
            n.disk.log.clone(),
            applied,
            self.opts.clone(),
            self.seed.wrapping_add(self.now),
        ));
    }

    fn violation(&self, what: &str) -> ! {
        panic!(
            "seed {} tick {}: SAFETY VIOLATION: {what}",
            self.seed, self.now
        );
    }

    pub fn leader(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .filter_map(|(id, n)| {
                n.raft
                    .as_ref()
                    .filter(|r| r.is_leader())
                    .map(|r| (r.term(), *id))
            })
            .max()
            .map(|(_, id)| id)
    }

    fn reachable(&self, a: NodeId, b: NodeId) -> bool {
        self.groups.get(&a) == self.groups.get(&b)
    }

    /// Handle everything the node's core produced, as the real driver does.
    fn process_ready(&mut self, id: NodeId) {
        loop {
            let (ready, snapshot_nodes) = {
                let Some(raft) = self.nodes.get_mut(&id).and_then(|n| n.raft.as_mut()) else {
                    return;
                };
                let r = raft.ready();
                if r.is_empty() {
                    return;
                }
                (r, ())
            };
            let _ = snapshot_nodes;
            // 1. persist
            {
                let n = self.nodes.get_mut(&id).unwrap();
                if let Some(h) = &ready.hard_state {
                    n.disk.hard = h.clone();
                }
                if let Some(t) = ready.truncate_from {
                    n.disk.log.retain(|e| e.index < t);
                }
                n.disk.log.extend(ready.append.iter().cloned());
                if let Some((meta, data)) = &ready.restore {
                    // the snapshot carries the whole state machine
                    let applied: Vec<(Index, Payload)> = serde_json::from_slice(data).unwrap();
                    n.disk.applied = applied;
                    n.disk.snapshot = Some(meta.clone());
                    n.disk.log.clear();
                }
            }
            // 2. send
            for m in ready.messages {
                if self.rng.chance(self.faults_drop()) || !self.reachable(m.from, m.to) {
                    continue;
                }
                let copies = if self.rng.chance(self.faults_dup()) {
                    2
                } else {
                    1
                };
                for _ in 0..copies {
                    let delay = 1 + self.rng.below(self.faults_delay());
                    self.inflight.push((self.now + delay, m.clone()));
                }
            }
            // 3. snapshots requested by the leader
            for peer in ready.snapshot_requests {
                let n = self.nodes.get_mut(&id).unwrap();
                let applied_to = n.disk.applied.last().map_or(0, |a| a.0);
                let data = serde_json::to_vec(&n.disk.applied).unwrap();
                if let Some(raft) = n.raft.as_mut() {
                    if let Some(meta) = raft.snapshot_meta_at(applied_to) {
                        raft.send_snapshot(peer, meta, data);
                    }
                }
            }
            // 4. apply
            for e in ready.committed {
                self.apply(id, e);
            }
        }
    }

    fn apply(&mut self, id: NodeId, e: Entry) {
        // safety: all nodes apply the same entry at an index
        match self.committed.get(&e.index) {
            Some((t, p)) if *t != e.term || *p != e.payload => self.violation(&format!(
                "node {id} applied index {} as ({}, {:?}) but another node applied ({t}, {p:?})",
                e.index, e.term, e.payload
            )),
            Some(_) => {}
            None => {
                self.committed.insert(e.index, (e.term, e.payload.clone()));
            }
        }
        let n = self.nodes.get_mut(&id).unwrap();
        let expected = n.disk.applied.last().map_or(0, |a| a.0) + 1;
        if e.index != expected {
            let msg = format!("node {id} applied index {} after {}", e.index, expected - 1);
            self.violation(&msg);
        }
        n.disk.applied.push((e.index, e.payload.clone()));
        // an entry this node proposed and has now applied is acknowledged to its client
        if let Some(data) = self.proposed.remove(&(id, e.index, e.term)) {
            if matches!(&e.payload, Payload::Data(d) if *d == data) {
                self.acked.insert(data);
            }
        }
    }

    fn faults_drop(&self) -> u64 {
        self.faults.drop_percent
    }
    fn faults_dup(&self) -> u64 {
        self.faults.dup_percent
    }
    fn faults_delay(&self) -> u64 {
        self.faults.max_delay
    }

    fn check_invariants(&mut self) {
        // election safety
        for (id, n) in &self.nodes {
            if let Some(r) = &n.raft {
                if r.is_leader() {
                    match self.leaders.get(&r.term()) {
                        Some(other) if other != id => {
                            let msg = format!("two leaders in term {}: {other} and {id}", r.term());
                            panic!(
                                "seed {} tick {}: SAFETY VIOLATION: {msg}",
                                self.seed, self.now
                            );
                        }
                        _ => {
                            self.leaders.insert(r.term(), *id);
                        }
                    }
                }
            }
        }
        // log matching: the same index and term means the same entry
        let mut seen: BTreeMap<(Index, Term), &Payload> = BTreeMap::new();
        for (id, n) in &self.nodes {
            for e in &n.disk.log {
                if let Some(p) = seen.insert((e.index, e.term), &e.payload) {
                    if *p != e.payload {
                        panic!("seed {} tick {}: SAFETY VIOLATION: log matching broken at ({}, {}) on node {id}", self.seed, self.now, e.index, e.term);
                    }
                }
            }
        }
    }

    pub fn propose(&mut self) {
        let Some(leader) = self.leader() else { return };
        self.next_payload += 1;
        let data = self.next_payload.to_be_bytes().to_vec();
        let n = self.nodes.get_mut(&leader).unwrap();
        let raft = n.raft.as_mut().unwrap();
        let term = raft.term();
        if let Ok(index) = raft.propose(data.clone()) {
            self.proposed.insert((leader, index, term), data);
        }
        self.process_ready(leader);
    }

    pub fn step(&mut self) {
        self.now += 1;
        let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        // crashes and restarts
        for id in &ids {
            let down = self.nodes[id].raft.is_none();
            if down && self.nodes[id].restart_at <= self.now {
                self.restart(*id);
            } else if !down && self.rng.below(1000) < self.faults.crash_percent_per_tick_per_1000 {
                let n = self.nodes.get_mut(id).unwrap();
                n.raft = None; // volatile state is lost; the disk stays
                n.restart_at = self.now + 5 + self.rng.below(40);
            }
        }
        // partitions
        if self.faults.partition_every > 0 && self.now.is_multiple_of(self.faults.partition_every) {
            if self.rng.chance(50) {
                for id in &ids {
                    let g = self.rng.below(2) as u8;
                    self.groups.insert(*id, g);
                }
            } else {
                for id in &ids {
                    self.groups.insert(*id, 0);
                }
            }
        }
        // deliver what is due
        let now = self.now;
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.inflight)
            .into_iter()
            .partition(|(t, _)| *t <= now);
        self.inflight = later;
        let mut due = due;
        // arbitrary order within a tick
        for i in (1..due.len()).rev() {
            let j = self.rng.below(i as u64 + 1) as usize;
            due.swap(i, j);
        }
        for (_, m) in due {
            if !self.reachable(m.from, m.to) {
                continue;
            }
            if let Some(raft) = self.nodes.get_mut(&m.to).and_then(|n| n.raft.as_mut()) {
                raft.step(m.clone());
            }
            self.process_ready(m.to);
        }
        for id in &ids {
            if let Some(raft) = self.nodes.get_mut(id).and_then(|n| n.raft.as_mut()) {
                raft.tick();
            }
            self.process_ready(*id);
        }
        self.check_invariants();
    }

    pub fn heal(&mut self) {
        self.faults = Faults {
            drop_percent: 0,
            dup_percent: 0,
            max_delay: 2,
            crash_percent_per_tick_per_1000: 0,
            partition_every: 0,
        };
        for id in self.nodes.keys().copied().collect::<Vec<_>>() {
            self.groups.insert(id, 0);
        }
    }

    pub fn run(&mut self, ticks: u64, propose_every: u64) {
        for t in 0..ticks {
            if propose_every > 0 && t % propose_every == 0 {
                self.propose();
            }
            self.step();
        }
    }

    #[allow(dead_code)]
    pub fn crash_all_down(&self) -> bool {
        self.nodes.values().all(|n| n.raft.is_none())
    }

    /// After healing: every node has applied the same sequence, and nothing acknowledged is missing.
    pub fn assert_converged(&mut self, extra_ticks: u64) {
        // wake any crashed node and let the cluster settle
        for n in self.nodes.values_mut() {
            n.restart_at = 0;
        }
        for _ in 0..extra_ticks {
            self.propose_if_leader_idle();
            self.step();
        }
        let leader = self.leader().unwrap_or_else(|| {
            panic!(
                "seed {}: no leader {extra_ticks} ticks after healing\n{}",
                self.seed,
                self.dump()
            )
        });
        let reference: Vec<(Index, Payload)> = self.nodes[&leader].disk.applied.clone();
        for (id, n) in &self.nodes {
            if !self.is_member(*id) {
                continue;
            }
            let applied = &n.disk.applied;
            if applied != &reference {
                let have: Vec<Index> = applied.iter().map(|a| a.0).collect();
                panic!(
                    "seed {}: node {id} has applied {} entries ({:?}...) but the leader {leader} has {}",
                    self.seed,
                    applied.len(),
                    have.iter().rev().take(3).collect::<Vec<_>>(),
                    reference.len()
                );
            }
        }
        let all: BTreeSet<Vec<u8>> = reference
            .iter()
            .filter_map(|(_, p)| {
                if let Payload::Data(d) = p {
                    Some(d.clone())
                } else {
                    None
                }
            })
            .collect();
        for a in &self.acked {
            assert!(
                all.contains(a),
                "seed {}: an acknowledged write was lost: {a:?}",
                self.seed
            );
        }
    }

    fn propose_if_leader_idle(&mut self) {
        // a few writes after healing prove the cluster accepts new work
        if self.now.is_multiple_of(20) {
            self.propose();
        }
    }

    /// Voters according to the leader.
    fn is_member(&self, id: NodeId) -> bool {
        match self.leader().and_then(|l| self.nodes[&l].raft.as_ref()) {
            Some(r) => r.voters().contains(&id),
            None => true,
        }
    }

    pub fn acked_count(&self) -> usize {
        self.acked.len()
    }

    #[allow(dead_code)]
    pub fn committed_count(&self) -> usize {
        self.committed.len()
    }

    // ---- membership changes ----

    pub fn add_voter(&mut self, id: NodeId) -> bool {
        if !self.nodes.contains_key(&id) {
            self.add_node(id, false);
        }
        let Some(l) = self.leader() else { return false };
        let mut voters: Vec<NodeId> = self.nodes[&l].raft.as_ref().unwrap().voters().to_vec();
        if !voters.contains(&id) {
            voters.push(id);
        }
        let ok = self
            .nodes
            .get_mut(&l)
            .unwrap()
            .raft
            .as_mut()
            .unwrap()
            .propose_config(voters)
            .is_ok();
        self.process_ready(l);
        ok
    }

    pub fn remove_voter(&mut self, id: NodeId) -> bool {
        let Some(l) = self.leader() else { return false };
        let voters: Vec<NodeId> = self.nodes[&l]
            .raft
            .as_ref()
            .unwrap()
            .voters()
            .iter()
            .copied()
            .filter(|v| *v != id)
            .collect();
        let ok = self
            .nodes
            .get_mut(&l)
            .unwrap()
            .raft
            .as_mut()
            .unwrap()
            .propose_config(voters)
            .is_ok();
        self.process_ready(l);
        ok
    }

    pub fn compact_all(&mut self, keep: u64) {
        for n in self.nodes.values_mut() {
            let Some(raft) = n.raft.as_mut() else {
                continue;
            };
            let applied = n.disk.applied.last().map_or(0, |a| a.0);
            if applied > keep {
                let upto = applied - keep;
                if upto > raft.snapshot_index() {
                    if let Some(meta) = raft.snapshot_meta_at(upto) {
                        raft.compact(upto);
                        if raft.snapshot_index() == upto {
                            n.disk.log.retain(|e| e.index > upto);
                            n.disk.snapshot = Some(meta);
                        }
                    }
                }
            }
        }
    }

    pub fn set_faults(&mut self, f: Faults) {
        self.faults = f;
    }

    pub fn voters_of_leader(&self) -> Vec<NodeId> {
        self.leader()
            .and_then(|l| self.nodes[&l].raft.as_ref().map(|r| r.voters().to_vec()))
            .unwrap_or_default()
    }

    /// Cut one node off from the others.
    pub fn isolate(&mut self, id: NodeId) {
        for other in self.nodes.keys().copied().collect::<Vec<_>>() {
            self.groups.insert(other, 0);
        }
        self.groups.insert(id, 1);
    }

    pub fn dump(&self) -> String {
        self.nodes
            .iter()
            .map(|(id, n)| match &n.raft {
                None => format!("  node {id}: DOWN"),
                Some(r) => format!(
                    "  node {id}: {:?} term {} leader {:?} voters {:?} last {} commit {} snap {}",
                    r.role(),
                    r.term(),
                    r.leader(),
                    r.voters(),
                    r.last_index(),
                    r.commit_index(),
                    r.snapshot_index()
                ),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[allow(dead_code)]
    pub fn is_down(&self, id: NodeId) -> bool {
        self.nodes[&id].raft.is_none()
    }
}
