//! A TCP transport for Raft messages.
//!
//! Each peer has one outgoing connection, opened lazily and reopened with a pause after a failure.
//! A frame is a 4-byte big-endian length followed by the bincode of an [`Envelope`]. The first frame
//! on a connection is the cluster token; a peer that does not present it is disconnected. Messages
//! that cannot be sent are dropped, which Raft tolerates.

use super::core::{Envelope, NodeId};
use super::node::{RaftNode, Transport};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// The largest frame accepted: a snapshot of a whole database travels in one.
const MAX_FRAME: u32 = 1 << 30;
/// Messages queued per peer before new ones are dropped.
const QUEUE: usize = 4096;

pub struct TcpTransport {
    token: Arc<Vec<u8>>,
    addrs: Arc<RwLock<HashMap<NodeId, String>>>,
    queues: Mutex<HashMap<NodeId, mpsc::Sender<Envelope>>>,
    runtime: tokio::runtime::Handle,
}

impl TcpTransport {
    pub fn new(peers: HashMap<NodeId, String>, token: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            token: Arc::new(token),
            addrs: Arc::new(RwLock::new(peers)),
            queues: Mutex::new(HashMap::new()),
            runtime: tokio::runtime::Handle::current(),
        })
    }

    /// Learn where a peer listens (a node added to the cluster).
    pub fn set_addr(&self, id: NodeId, addr: String) {
        self.addrs.write().expect("raft addresses").insert(id, addr);
    }

    pub fn addrs(&self) -> HashMap<NodeId, String> {
        self.addrs.read().expect("raft addresses").clone()
    }

    /// Accept connections from peers and hand their messages to `node`.
    pub async fn listen(
        self: &Arc<Self>,
        addr: &str,
        node: RaftNode,
    ) -> std::io::Result<ListenHandle> {
        let listener = TcpListener::bind(addr).await?;
        let token = Arc::clone(&self.token);
        let conns: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let tracked = Arc::clone(&conns);
        let accept = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let (node, token) = (node.clone(), Arc::clone(&token));
                let task = tokio::spawn(async move {
                    let _ = read_loop(stream, node, token).await;
                });
                let mut c = tracked.lock().expect("raft connections");
                c.retain(|h| !h.is_finished());
                c.push(task.abort_handle());
            }
        });
        Ok(ListenHandle { accept, conns })
    }
}

/// The listening socket and the connections accepted on it; `abort` closes all of them.
pub struct ListenHandle {
    accept: tokio::task::JoinHandle<()>,
    conns: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl ListenHandle {
    pub fn abort(&self) {
        self.accept.abort();
        for h in self.conns.lock().expect("raft connections").drain(..) {
            h.abort();
        }
    }
}

impl Transport for TcpTransport {
    fn send(&self, env: Envelope) {
        let to = env.to;
        let tx = {
            let mut q = self.queues.lock().expect("raft queues");
            match q.get(&to) {
                Some(tx) => tx.clone(),
                None => {
                    let (tx, rx) = mpsc::channel(QUEUE);
                    q.insert(to, tx.clone());
                    let shared = Shared {
                        token: Arc::clone(&self.token),
                        addrs: Arc::clone(&self.addrs),
                    };
                    self.runtime.spawn(shared.write_loop(to, rx));
                    tx
                }
            }
        };
        let _ = tx.try_send(env); // a full queue drops the message
    }
}

/// The parts of the transport a peer's writer task needs.
struct Shared {
    token: Arc<Vec<u8>>,
    addrs: Arc<RwLock<HashMap<NodeId, String>>>,
}

impl Shared {
    async fn write_loop(self, to: NodeId, mut rx: mpsc::Receiver<Envelope>) {
        let mut conn: Option<TcpStream> = None;
        while let Some(env) = rx.recv().await {
            let Ok(body) = bincode::serialize(&env) else {
                continue;
            };
            if conn.is_none() {
                let Some(addr) = self.addrs.read().expect("raft addresses").get(&to).cloned()
                else {
                    continue;
                };
                match tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await
                {
                    Ok(Ok(mut s)) => {
                        let _ = s.set_nodelay(true);
                        if write_frame(&mut s, &self.token).await.is_ok() {
                            conn = Some(s);
                        }
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        while rx.try_recv().is_ok() {}
                        continue;
                    }
                }
            }
            if let Some(s) = conn.as_mut() {
                if write_frame(s, &body).await.is_err() {
                    conn = None;
                }
            }
        }
    }
}

async fn write_frame(s: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    s.write_all(&(body.len() as u32).to_be_bytes()).await?;
    s.write_all(body).await?;
    s.flush().await
}

async fn read_frame(s: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len);
    if len > MAX_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn read_loop(mut s: TcpStream, node: RaftNode, token: Arc<Vec<u8>>) -> std::io::Result<()> {
    let _ = s.set_nodelay(true);
    // the first frame must be the cluster token
    let first = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut s))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "no token"))??;
    if first != *token {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "bad cluster token",
        ));
    }
    loop {
        let frame = read_frame(&mut s).await?;
        match bincode::deserialize::<Envelope>(&frame) {
            Ok(env) => node.deliver(env),
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bad message",
                ))
            }
        }
    }
}
