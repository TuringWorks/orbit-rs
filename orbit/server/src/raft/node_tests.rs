//! Real nodes over real TCP on loopback, driving a toy state machine.

use super::core::*;
use super::net::TcpTransport;
use super::node::*;
use super::storage::{Loaded, MemStorage, RaftStorage, Save};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A state machine that records every command, with its index.
#[derive(Default)]
struct Toy {
    inner: Mutex<(Index, Vec<(Index, Vec<u8>)>)>,
}

impl Toy {
    fn values(&self) -> Vec<Vec<u8>> {
        self.inner
            .lock()
            .unwrap()
            .1
            .iter()
            .map(|(_, v)| v.clone())
            .collect()
    }
}

impl StateMachine for Toy {
    fn apply(&self, index: Index, data: Option<&[u8]>) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        g.0 = index;
        if let Some(d) = data {
            g.1.push((index, d.to_vec()));
        }
        Ok(())
    }
    fn applied_index(&self) -> Index {
        self.inner.lock().unwrap().0
    }
    fn snapshot(&self) -> Result<(Index, Vec<u8>), String> {
        let g = self.inner.lock().unwrap();
        Ok((g.0, serde_json::to_vec(&g.1).unwrap()))
    }
    fn restore(&self, index: Index, data: &[u8]) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        g.0 = index;
        g.1 = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Storage that outlives a node, so the node can be restarted from it.
#[derive(Clone, Default)]
struct SharedMem(Arc<Mutex<MemStorage>>);

impl RaftStorage for SharedMem {
    fn load(&self) -> Result<Loaded, String> {
        self.0.lock().unwrap().load()
    }
    fn save(&mut self, s: Save<'_>) -> Result<(), String> {
        self.0.lock().unwrap().save(s)
    }
}

static REG: Mutex<Vec<RaftNode>> = Mutex::new(Vec::new());

struct Member {
    node: RaftNode,
    sm: Arc<Toy>,
    storage: SharedMem,
    handle: Option<std::thread::JoinHandle<()>>,
    listener: super::net::ListenHandle,
    transport: Arc<TcpTransport>,
}

impl Member {
    fn stop(&mut self) {
        self.node.stop();
        self.listener.abort();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn free_ports(n: usize) -> Vec<u16> {
    let ls: Vec<_> = (0..n)
        .map(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap())
        .collect();
    ls.iter().map(|l| l.local_addr().unwrap().port()).collect()
}

async fn start(
    id: NodeId,
    voters: Vec<NodeId>,
    addrs: &HashMap<NodeId, String>,
    sm: Arc<Toy>,
    storage: SharedMem,
    keep_log: u64,
) -> Member {
    let transport = TcpTransport::new(addrs.clone(), b"test-token".to_vec());
    let mut cfg = NodeConfig::new(id, voters);
    cfg.tick = Duration::from_millis(20);
    cfg.options.election_ticks = 25; // 0.5 to 1 s: tolerant of a loaded machine
    cfg.keep_log = keep_log;
    let (node, handle) = RaftNode::start(
        cfg,
        Box::new(storage.clone()),
        sm.clone(),
        transport.clone(),
    )
    .unwrap();
    let listener = transport.listen(&addrs[&id], node.clone()).await.unwrap();
    REG.lock().unwrap().push(node.clone());
    Member {
        node,
        sm,
        storage,
        handle: Some(handle),
        listener,
        transport,
    }
}

async fn cluster(n: u64, keep_log: u64) -> (Vec<Member>, HashMap<NodeId, String>) {
    let ports = free_ports(n as usize);
    let addrs: HashMap<NodeId, String> = (1..=n)
        .map(|i| (i, format!("127.0.0.1:{}", ports[i as usize - 1])))
        .collect();
    let voters: Vec<NodeId> = (1..=n).collect();
    let mut ms = Vec::new();
    for i in 1..=n {
        ms.push(
            start(
                i,
                voters.clone(),
                &addrs,
                Arc::new(Toy::default()),
                SharedMem::default(),
                keep_log,
            )
            .await,
        );
    }
    (ms, addrs)
}

async fn wait_until(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let st: Vec<String> = REG
        .lock()
        .unwrap()
        .iter()
        .map(|n| format!("{:?}", n.status()))
        .collect();
    panic!("timed out waiting for: {what}\n{}", st.join("\n"));
}

fn leader_of(ms: &[Member]) -> Option<usize> {
    ms.iter()
        .position(|m| m.node.status().serving && !m.node.status().stopped)
}

fn val(i: u32) -> Vec<u8> {
    i.to_be_bytes().to_vec()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_nodes_elect_a_leader_and_replicate() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, _) = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    for i in 0..20 {
        ms[l].node.propose(val(i)).await.unwrap();
    }
    let want: Vec<Vec<u8>> = (0..20).map(val).collect();
    wait_until("every node applied the writes", 10, || {
        ms.iter().all(|m| m.sm.values() == want)
    })
    .await;
    // a follower refuses and says who leads
    let f = (0..3).find(|i| *i != l).unwrap();
    match ms[f].node.propose(val(99)).await {
        Err(RaftError::NotLeader { leader }) => assert_eq!(leader, Some(l as u64 + 1)),
        other => panic!("{other:?}"),
    }
    for m in &mut ms {
        m.stop();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn losing_the_leader_loses_no_acknowledged_write_and_the_old_leader_rejoins() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, addrs) = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    for i in 0..10 {
        ms[l].node.propose(val(i)).await.unwrap();
    }
    ms[l].stop();
    let rest: Vec<usize> = (0..3).filter(|i| *i != l).collect();
    wait_until("a new leader among the survivors", 15, || {
        rest.iter().any(|i| ms[*i].node.status().serving)
    })
    .await;
    let nl = *rest.iter().find(|i| ms[**i].node.status().serving).unwrap();
    for i in 10..20 {
        ms[nl].node.propose(val(i)).await.unwrap();
    }
    let want: Vec<Vec<u8>> = (0..20).map(val).collect();
    wait_until("survivors hold everything", 10, || {
        rest.iter().all(|i| ms[*i].sm.values() == want)
    })
    .await;
    // the old leader comes back from its own storage and catches up
    let (sm, storage) = (ms[l].sm.clone(), ms[l].storage.clone());
    let voters = vec![1, 2, 3];
    ms[l] = start(l as u64 + 1, voters, &addrs, sm, storage, 10_000).await;
    wait_until("the old leader caught up", 15, || ms[l].sm.values() == want).await;
    assert!(!ms[l].node.status().serving || ms[l].node.status().leader == Some(l as u64 + 1));
    for m in &mut ms {
        m.stop();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_minority_cannot_commit_anything() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, _) = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    ms[l].node.propose(val(1)).await.unwrap();
    for i in 0..3 {
        if i != l {
            ms[i].stop();
        }
    }
    let r = tokio::time::timeout(Duration::from_millis(1500), ms[l].node.propose(val(2))).await;
    assert!(
        matches!(r, Err(_) | Ok(Err(_))),
        "a lone node must not commit: {r:?}"
    );
    assert!(!ms[l].sm.values().contains(&val(2)));
    ms[l].stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_that_missed_a_compacted_log_catches_up_from_a_snapshot() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, addrs) = cluster(3, 20).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    let lag = (0..3).find(|i| *i != l).unwrap();
    ms[lag].stop();
    for i in 0..300 {
        ms[l].node.propose(val(i)).await.unwrap();
    }
    wait_until("the leader compacted its log", 10, || {
        ms[l].node.status().applied > 200
    })
    .await;
    // the lagging node returns with an empty state machine and storage: only a snapshot can bring it up to date
    ms[lag] = start(
        lag as u64 + 1,
        vec![1, 2, 3],
        &addrs,
        Arc::new(Toy::default()),
        SharedMem::default(),
        20,
    )
    .await;
    let want: Vec<Vec<u8>> = (0..300).map(val).collect();
    wait_until("the returning node holds all 300 writes", 20, || {
        ms[lag].sm.values() == want
    })
    .await;
    for m in &mut ms {
        m.stop();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_whole_cluster_restarts_from_its_storage_with_nothing_lost() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, addrs) = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    for i in 0..30 {
        ms[l].node.propose(val(i)).await.unwrap();
    }
    let want: Vec<Vec<u8>> = (0..30).map(val).collect();
    wait_until("replicated", 10, || {
        ms.iter().all(|m| m.sm.values() == want)
    })
    .await;
    let kept: Vec<(Arc<Toy>, SharedMem)> = ms
        .iter()
        .map(|m| (m.sm.clone(), m.storage.clone()))
        .collect();
    for m in &mut ms {
        m.stop();
    }
    let mut ms = Vec::new();
    for (i, (sm, storage)) in kept.into_iter().enumerate() {
        ms.push(start(i as u64 + 1, vec![1, 2, 3], &addrs, sm, storage, 10_000).await);
    }
    wait_until("a serving leader after the restart", 15, || {
        leader_of(&ms).is_some()
    })
    .await;
    let l = leader_of(&ms).unwrap();
    ms[l].node.propose(val(1000)).await.unwrap();
    let mut want = want;
    want.push(val(1000));
    wait_until(
        "everything is still there and the new write too",
        10,
        || ms.iter().all(|m| m.sm.values() == want),
    )
    .await;
    for m in &mut ms {
        m.stop();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_server_can_be_added_and_removed_while_running() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let (mut ms, mut addrs) = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || leader_of(&ms).is_some()).await;
    let l = leader_of(&ms).unwrap();
    for i in 0..10 {
        ms[l].node.propose(val(i)).await.unwrap();
    }
    // a fourth server joins with no voters of its own and learns everything from the leader
    let port = free_ports(1)[0];
    addrs.insert(4, format!("127.0.0.1:{port}"));
    let joiner = start(
        4,
        vec![],
        &addrs,
        Arc::new(Toy::default()),
        SharedMem::default(),
        10_000,
    )
    .await;
    // every existing node learns where it listens, as the server does when a node is added
    for m in &ms {
        m.transport.set_addr(4, addrs[&4].clone());
    }
    ms.push(joiner);
    ms[l]
        .node
        .change_membership(vec![1, 2, 3, 4])
        .await
        .unwrap_or_else(|e| panic!("add: {e}"));
    let want: Vec<Vec<u8>> = (0..10).map(val).collect();
    wait_until("the new server caught up", 15, || ms[3].sm.values() == want).await;
    // and one is removed again
    let l = leader_of(&ms).unwrap();
    let victim = (0..4).find(|i| *i != l).unwrap();
    let remaining: Vec<NodeId> = (1..=4u64).filter(|v| *v != victim as u64 + 1).collect();
    ms[l]
        .node
        .change_membership(remaining.clone())
        .await
        .unwrap();
    wait_until("the leader sees three voters", 10, || {
        ms[l].node.status().voters == remaining
    })
    .await;
    ms[l].node.propose(val(77)).await.unwrap();
    for m in &mut ms {
        m.stop();
    }
}
