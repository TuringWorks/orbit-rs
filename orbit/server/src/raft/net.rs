//! A TCP transport for Raft messages, optionally over mutual TLS.
//!
//! Each peer has one outgoing connection, opened lazily and reopened with a pause after a failure.
//! A frame is a 4-byte big-endian length followed by the bincode of an [`Envelope`]. The first frame
//! on a connection is the cluster token; a peer that does not present it is disconnected. With TLS
//! configured, every connection is encrypted and both ends must present a certificate signed by the
//! cluster's CA. Messages that cannot be sent are dropped, which Raft tolerates.

use super::core::{Envelope, NodeId};
use super::node::{RaftNode, Transport};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{self, RootCertStore};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// The largest frame accepted: a snapshot of a whole database travels in one.
const MAX_FRAME: u32 = 1 << 30;
/// Messages queued per peer before new ones are dropped.
const QUEUE: usize = 4096;

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
type BoxStream = Box<dyn Stream>;

/// Certificates for node-to-node traffic. Each node needs a certificate, signed by `ca`, that is valid
/// as a TLS server and (the same or another one) as a TLS client; its subject alternative names must
/// include `server_name`, which is what a connecting node checks.
#[derive(Clone, Debug)]
pub struct RaftTls {
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub client_cert: PathBuf,
    pub client_key: PathBuf,
    pub ca: PathBuf,
    pub server_name: String,
}

#[derive(Clone)]
struct TlsParts {
    acceptor: TlsAcceptor,
    connector: TlsConnector,
    server_name: ServerName<'static>,
}

fn invalid(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
}

impl RaftTls {
    fn build(&self) -> std::io::Result<TlsParts> {
        let provider = crate::protocols::tls::provider();
        let mut roots = RootCertStore::empty();
        for c in crate::protocols::tls::load_certs(&self.ca)? {
            roots.add(c).map_err(invalid)?;
        }
        let roots = Arc::new(roots);
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            Arc::clone(&roots),
            Arc::clone(&provider),
        )
        .build()
        .map_err(invalid)?;
        let server = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(invalid)?
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                crate::protocols::tls::load_certs(&self.server_cert)?,
                crate::protocols::tls::load_private_key(&self.server_key)?,
            )
            .map_err(invalid)?;
        let client = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(invalid)?
            .with_root_certificates(roots)
            .with_client_auth_cert(
                crate::protocols::tls::load_certs(&self.client_cert)?,
                crate::protocols::tls::load_private_key(&self.client_key)?,
            )
            .map_err(invalid)?;
        Ok(TlsParts {
            acceptor: TlsAcceptor::from(Arc::new(server)),
            connector: TlsConnector::from(Arc::new(client)),
            server_name: ServerName::try_from(self.server_name.clone()).map_err(invalid)?,
        })
    }
}

pub struct TcpTransport {
    token: Arc<Vec<u8>>,
    addrs: Arc<RwLock<HashMap<NodeId, String>>>,
    queues: Mutex<HashMap<NodeId, mpsc::Sender<Envelope>>>,
    runtime: tokio::runtime::Handle,
    tls: Option<TlsParts>,
}

impl TcpTransport {
    pub fn new(peers: HashMap<NodeId, String>, token: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            token: Arc::new(token),
            addrs: Arc::new(RwLock::new(peers)),
            queues: Mutex::new(HashMap::new()),
            runtime: tokio::runtime::Handle::current(),
            tls: None,
        })
    }

    /// Like [`new`](Self::new), encrypting every connection and requiring certificates when `tls` is given.
    pub fn with_tls(
        peers: HashMap<NodeId, String>,
        token: Vec<u8>,
        tls: Option<&RaftTls>,
    ) -> std::io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            token: Arc::new(token),
            addrs: Arc::new(RwLock::new(peers)),
            queues: Mutex::new(HashMap::new()),
            runtime: tokio::runtime::Handle::current(),
            tls: tls.map(RaftTls::build).transpose()?,
        }))
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
        let tls = self.tls.clone();
        let conns: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let tracked = Arc::clone(&conns);
        let accept = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let (node, token, tls) = (node.clone(), Arc::clone(&token), tls.clone());
                let task = tokio::spawn(async move {
                    let stream: BoxStream = match tls {
                        Some(t) => match tokio::time::timeout(
                            Duration::from_secs(5),
                            t.acceptor.accept(stream),
                        )
                        .await
                        {
                            Ok(Ok(s)) => Box::new(s),
                            _ => return, // not a cluster node: no certificate, a wrong one, or no handshake
                        },
                        None => Box::new(stream),
                    };
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
                        tls: self.tls.clone(),
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
    tls: Option<TlsParts>,
}

impl Shared {
    async fn connect(&self, addr: &str) -> Option<BoxStream> {
        let tcp = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(addr))
            .await
            .ok()?
            .ok()?;
        let _ = tcp.set_nodelay(true);
        let mut stream: BoxStream = match &self.tls {
            Some(t) => {
                let s = tokio::time::timeout(
                    Duration::from_secs(5),
                    t.connector.connect(t.server_name.clone(), tcp),
                )
                .await
                .ok()?
                .ok()?;
                Box::new(s)
            }
            None => Box::new(tcp),
        };
        write_frame(&mut stream, &self.token).await.ok()?;
        Some(stream)
    }

    async fn write_loop(self, to: NodeId, mut rx: mpsc::Receiver<Envelope>) {
        let mut conn: Option<BoxStream> = None;
        while let Some(env) = rx.recv().await {
            let Ok(body) = bincode::serialize(&env) else {
                continue;
            };
            if conn.is_none() {
                let Some(addr) = self.addrs.read().expect("raft addresses").get(&to).cloned()
                else {
                    continue;
                };
                match self.connect(&addr).await {
                    Some(s) => conn = Some(s),
                    None => {
                        // pause so a dead peer does not turn every message into a connect attempt
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

async fn write_frame<W: AsyncWrite + Unpin + ?Sized>(
    s: &mut W,
    body: &[u8],
) -> std::io::Result<()> {
    s.write_all(&(body.len() as u32).to_be_bytes()).await?;
    s.write_all(body).await?;
    s.flush().await
}

async fn read_frame<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> std::io::Result<Vec<u8>> {
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

async fn read_loop(mut s: BoxStream, node: RaftNode, token: Arc<Vec<u8>>) -> std::io::Result<()> {
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
