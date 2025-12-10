use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Configuration for a TCP proxy service
#[derive(Debug)]
pub struct ProxyConfig {
    pub name: String,
    pub listen_port: u16,
    pub backends: Vec<SocketAddr>,
    pub counter: AtomicUsize,
}

impl ProxyConfig {
    pub fn new(name: &str, listen_port: u16, backends: Vec<SocketAddr>) -> Self {
        Self {
            name: name.to_string(),
            listen_port,
            backends,
            counter: AtomicUsize::new(0),
        }
    }

    pub fn next_backend(&self) -> SocketAddr {
        if self.backends.is_empty() {
            // Should be handled by validation, but safe fallback
            return "127.0.0.1:0".parse().unwrap();
        }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        self.backends[idx]
    }
}

/// Run the proxy server
pub async fn run_proxy(config: Arc<ProxyConfig>, bind_addr: &str, verbose: bool) -> std::io::Result<()> {
    let addr = format!("{}:{}", bind_addr, config.listen_port);
    let listener = TcpListener::bind(&addr).await?;

    if verbose {
        println!(
            "[{}] Listening on {} -> {:?}",
            config.name, addr, config.backends
        );
    }

    loop {
        let (client, _) = listener.accept().await?;
        let backend_addr = config.next_backend();
        let name = config.name.clone();
        
        tokio::spawn(async move {
            proxy_connection(client, backend_addr, name, verbose).await;
        });
    }
}

async fn proxy_connection(
    mut client: TcpStream,
    backend_addr: SocketAddr,
    name: String,
    verbose: bool,
) {
    let client_addr = client.peer_addr().ok();

    let mut backend = match TcpStream::connect(backend_addr).await {
        Ok(s) => s,
        Err(e) => {
            if verbose {
                eprintln!("[{}] Failed to connect to {}: {}", name, backend_addr, e);
            }
            return;
        }
    };

    if verbose {
        println!(
            "[{}] {} -> {}",
            name,
            client_addr.map(|a| a.to_string()).unwrap_or_default(),
            backend_addr
        );
    }

    let (mut client_read, mut client_write) = client.split();
    let (mut backend_read, mut backend_write) = backend.split();

    let client_to_backend = async {
        let mut buf = [0u8; 8192];
        loop {
            match client_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if backend_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let backend_to_client = async {
        let mut buf = [0u8; 8192];
        loop {
            match backend_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if client_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    tokio::select! {
        _ = client_to_backend => {}
        _ = backend_to_client => {}
    }
}
