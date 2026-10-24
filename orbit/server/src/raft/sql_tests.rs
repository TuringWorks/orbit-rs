//! Three SQL engines in a Raft cluster: real TCP, real RocksDB stores.

use super::core::NodeId;
use super::net::{ListenHandle, TcpTransport};
use super::node::{NodeConfig, RaftNode};
use super::sql::{EngineCell, NodeCell, RaftPersist, SqlStateMachine};
use super::storage::RocksRaftStorage;
use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::query_engine::QueryResult;
use crate::protocols::postgres_wire::sql::durable::{DurableEngine, RocksPersist, Session};
use crate::protocols::postgres_wire::sql::parser::SqlParser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct SqlNode {
    engine: Arc<DurableEngine>,
    node: RaftNode,
    listener: ListenHandle,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl SqlNode {
    fn stop(&mut self) {
        self.node.stop();
        self.listener.abort();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn free_ports(n: usize) -> Vec<u16> {
    crate::raft::test_ports(n)
}

async fn start(
    id: NodeId,
    voters: Vec<NodeId>,
    addrs: &HashMap<NodeId, String>,
    dir: PathBuf,
    keep_log: u64,
) -> SqlNode {
    let store = Arc::new(RocksPersist::open(dir.join("sql")).unwrap());
    let engine_cell: EngineCell = Arc::default();
    let node_cell: NodeCell = Arc::default();
    let sm = Arc::new(SqlStateMachine::new(
        Arc::clone(&store),
        Arc::clone(&engine_cell),
        Arc::new(|_, _| {}),
    ));
    let persist = RaftPersist::new(
        Arc::clone(&store),
        Arc::clone(&node_cell),
        Arc::clone(&engine_cell),
        true,
    );
    let engine = Arc::new(DurableEngine::open(Box::new(persist)).unwrap());
    engine_cell.set(Arc::downgrade(&engine)).ok();
    let transport = TcpTransport::new(addrs.clone(), b"sql-test".to_vec());
    let mut cfg = NodeConfig::new(id, voters);
    cfg.tick = Duration::from_millis(20);
    cfg.options.election_ticks = 25;
    cfg.keep_log = keep_log;
    let raft_store = RocksRaftStorage::open(dir.join("raft")).unwrap();
    let (node, handle) = RaftNode::start(cfg, Box::new(raft_store), sm, transport.clone()).unwrap();
    node_cell.set(node.clone()).ok();
    let listener = transport.listen(&addrs[&id], node.clone()).await.unwrap();
    SqlNode {
        engine,
        node,
        listener,
        handle: Some(handle),
    }
}

struct Cluster {
    nodes: Vec<SqlNode>,
    addrs: HashMap<NodeId, String>,
    dirs: Vec<tempfile::TempDir>,
}

async fn cluster(n: u64, keep_log: u64) -> Cluster {
    let ports = free_ports(n as usize);
    let addrs: HashMap<NodeId, String> = (1..=n)
        .map(|i| (i, format!("127.0.0.1:{}", ports[i as usize - 1])))
        .collect();
    let dirs: Vec<_> = (0..n).map(|_| tempfile::tempdir().unwrap()).collect();
    let voters: Vec<NodeId> = (1..=n).collect();
    let mut nodes = Vec::new();
    for i in 1..=n {
        nodes.push(
            start(
                i,
                voters.clone(),
                &addrs,
                dirs[i as usize - 1].path().to_path_buf(),
                keep_log,
            )
            .await,
        );
    }
    Cluster { nodes, addrs, dirs }
}

async fn wait_until(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

fn serving(c: &Cluster) -> Option<usize> {
    c.nodes
        .iter()
        .position(|n| n.node.status().serving && !n.node.status().stopped)
}

async fn sql(e: &DurableEngine, text: &str) -> ProtocolResult<QueryResult> {
    let mut s = Session::default();
    let mut last = QueryResult::Command {
        tag: "EMPTY".into(),
    };
    for stmt in SqlParser::new().parse_multiple(text).unwrap() {
        last = e.execute(&mut s, &stmt, text).await?;
    }
    Ok(last)
}

async fn rows(e: &DurableEngine, text: &str) -> Vec<Vec<String>> {
    try_rows(e, text)
        .await
        .unwrap_or_else(|| panic!("query failed: {text}"))
}

/// `None` when the query fails, for example because the table has not been replicated here yet.
async fn try_rows(e: &DurableEngine, text: &str) -> Option<Vec<Vec<String>>> {
    match sql(e, text).await.ok()?.untyped() {
        QueryResult::Select { rows, .. } => Some(
            rows.into_iter()
                .map(|r| r.into_iter().map(|c| c.unwrap_or_default()).collect())
                .collect(),
        ),
        other => panic!("{other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writes_on_the_leader_reach_every_node_and_followers_refuse_writes() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let mut c = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || serving(&c).is_some()).await;
    let l = serving(&c).unwrap();
    sql(
        &c.nodes[l].engine,
        "CREATE TABLE t (id SERIAL PRIMARY KEY, v VARCHAR UNIQUE)",
    )
    .await
    .unwrap();
    for i in 0..20 {
        sql(
            &c.nodes[l].engine,
            &format!("INSERT INTO t (v) VALUES ('v{i}')"),
        )
        .await
        .unwrap();
    }
    let want: Vec<Vec<String>> = (1..=20)
        .map(|i| vec![i.to_string(), format!("v{}", i - 1)])
        .collect();
    for n in &c.nodes {
        let e = Arc::clone(&n.engine);
        let mut seen = Vec::new();
        wait_until("a node holds all rows", 10, || {
            seen = futures_lite_block(try_rows(&e, "SELECT id, v FROM t ORDER BY id"))
                .unwrap_or_default();
            seen == want
        })
        .await;
    }
    let f = (0..3).find(|i| *i != l).unwrap();
    let err = sql(&c.nodes[f].engine, "INSERT INTO t (v) VALUES ('nope')")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("[25006]") && err.contains("not the leader"),
        "{err}"
    );
    // a replica enforces the same uniqueness (it holds the same indexes) when it later leads
    for n in &mut c.nodes {
        n.stop();
    }
}

fn futures_lite_block<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn failover_loses_no_acknowledged_write_and_reuses_no_id() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let mut c = cluster(3, 10_000).await;
    wait_until("a serving leader", 10, || serving(&c).is_some()).await;
    let l = serving(&c).unwrap();
    sql(
        &c.nodes[l].engine,
        "CREATE TABLE t (id SERIAL PRIMARY KEY, n INTEGER)",
    )
    .await
    .unwrap();
    let mut acked: Vec<i64> = Vec::new();
    for i in 0..15 {
        if let QueryResult::Returning { rows, .. } = sql(
            &c.nodes[l].engine,
            &format!("INSERT INTO t (n) VALUES ({i}) RETURNING id"),
        )
        .await
        .unwrap()
        {
            acked.push(rows[0][0].clone().unwrap().parse().unwrap());
        }
    }
    c.nodes[l].stop();
    let rest: Vec<usize> = (0..3).filter(|i| *i != l).collect();
    wait_until("a new leader", 15, || {
        rest.iter().any(|i| c.nodes[*i].node.status().serving)
    })
    .await;
    let nl = *rest
        .iter()
        .find(|i| c.nodes[**i].node.status().serving)
        .unwrap();
    for i in 15..30 {
        if let QueryResult::Returning { rows, .. } = sql(
            &c.nodes[nl].engine,
            &format!("INSERT INTO t (n) VALUES ({i}) RETURNING id"),
        )
        .await
        .unwrap()
        {
            acked.push(rows[0][0].clone().unwrap().parse().unwrap());
        }
    }
    let mut unique = acked.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        acked.len(),
        "no id was handed out twice across the failover: {acked:?}"
    );
    let survivors = &rest;
    for i in survivors {
        let e = Arc::clone(&c.nodes[*i].engine);
        wait_until("survivor holds every acknowledged row", 10, || {
            let got = futures_lite_block(try_rows(&e, "SELECT id FROM t ORDER BY id"))
                .unwrap_or_default();
            let ids: Vec<i64> = got.iter().map(|r| r[0].parse().unwrap()).collect();
            acked.iter().all(|a| ids.contains(a))
        })
        .await;
    }
    // the old leader returns from its own disk and catches up
    let dir = c.dirs[l].path().to_path_buf();
    let voters = vec![1, 2, 3];
    let addrs = c.addrs.clone();
    drop(c.nodes.remove(l));
    c.nodes
        .insert(l, start(l as u64 + 1, voters, &addrs, dir, 10_000).await);
    let e = Arc::clone(&c.nodes[l].engine);
    wait_until("the old leader caught up", 20, || {
        futures_lite_block(try_rows(&e, "SELECT count(*) FROM t"))
            == Some(vec![vec!["30".to_string()]])
    })
    .await;
    for n in &mut c.nodes {
        n.stop();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_with_an_empty_disk_catches_up_from_a_snapshot_of_the_database() {
    let _serial = crate::raft::TCP_TESTS.lock().await;
    let mut c = cluster(3, 20).await;
    wait_until("a serving leader", 10, || serving(&c).is_some()).await;
    let l = serving(&c).unwrap();
    sql(
        &c.nodes[l].engine,
        "CREATE TABLE t (id SERIAL PRIMARY KEY, v VARCHAR)",
    )
    .await
    .unwrap();
    sql(
        &c.nodes[l].engine,
        "CREATE TABLE u (id SERIAL PRIMARY KEY, t_id INTEGER REFERENCES t (id) ON DELETE CASCADE)",
    )
    .await
    .unwrap();
    let lag = (0..3).find(|i| *i != l).unwrap();
    c.nodes[lag].stop();
    for i in 0..120 {
        sql(
            &c.nodes[l].engine,
            &format!("INSERT INTO t (v) VALUES ('row{i}')"),
        )
        .await
        .unwrap();
    }
    sql(
        &c.nodes[l].engine,
        "INSERT INTO u (t_id) VALUES (1), (2), (3)",
    )
    .await
    .unwrap();
    sql(&c.nodes[l].engine, "DELETE FROM t WHERE id = 2")
        .await
        .unwrap();
    wait_until("the leader compacted its log", 10, || {
        c.nodes[l].node.status().applied > 100
    })
    .await;
    // wipe the lagging node's disk: only a snapshot can bring it back
    let dir = tempfile::tempdir().unwrap();
    let addrs = c.addrs.clone();
    drop(c.nodes.remove(lag));
    c.nodes.insert(
        lag,
        start(
            lag as u64 + 1,
            vec![1, 2, 3],
            &addrs,
            dir.path().to_path_buf(),
            20,
        )
        .await,
    );
    c.dirs.push(dir);
    let e = Arc::clone(&c.nodes[lag].engine);
    wait_until("the snapshot arrived", 30, || {
        futures_lite_block(try_rows(&e, "SELECT count(*) FROM t"))
            == Some(vec![vec!["119".to_string()]])
    })
    .await;
    assert_eq!(
        rows(&e, "SELECT t_id FROM u ORDER BY id").await,
        vec![vec!["1".to_string()], vec!["3".to_string()]],
        "the cascade delete and the foreign key came through"
    );
    for n in &mut c.nodes {
        n.stop();
    }
}
