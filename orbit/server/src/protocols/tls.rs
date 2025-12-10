use crate::config::TlsConfig;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::{self, pki_types::CertificateDer, pki_types::PrivateKeyDer, ServerConfig};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

#[derive(Clone)]
pub struct OrbitTlsAcceptor {
    acceptor: Option<TlsAcceptor>,
    config: Option<TlsConfig>,
}

impl OrbitTlsAcceptor {
    pub fn new(config: &Option<TlsConfig>) -> std::io::Result<Self> {
        if let Some(tls_config) = config {
            if !tls_config.enabled {
                return Ok(Self {
                    acceptor: None,
                    config: None,
                });
            }

            let certs = load_certs(&tls_config.cert_file)?;
            let key = load_private_key(&tls_config.key_file)?;

            // TODO: Implement client cert validation (mTLS) if require_client_cert is true
            // This requires loading the CA cert and setting up a verifier.
            // For now, we'll stick to server-side TLS primarily, but structure is here for mTLS.
           
             let mut server_config = if tls_config.require_client_cert {
                 if let Some(ca_path) = &tls_config.ca_cert_file {
                      let ca_certs = load_certs(ca_path)?;
                      let mut root_store = rustls::RootCertStore::empty();
                      for cert in ca_certs {
                          root_store.add(cert).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
                      }
                      
                      let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store)).build()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

                      ServerConfig::builder()
                        .with_client_cert_verifier(verifier)
                        .with_single_cert(certs, key)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?
                 } else {
                      return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "CA cert file required for client cert validation"));
                 }
            } else {
                 ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(certs, key)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?
            };
            
            server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()]; // Default ALPN

            Ok(Self {
                acceptor: Some(TlsAcceptor::from(Arc::new(server_config))),
                config: Some(tls_config.clone()),
            })
        } else {
            Ok(Self {
                acceptor: None,
                config: None,
            })
        }
    }

    pub async fn accept(&self, stream: TcpStream) -> std::io::Result<TlsStreamOrPlain> {
        if let Some(acceptor) = &self.acceptor {
            let stream = acceptor.accept(stream).await?;
            Ok(TlsStreamOrPlain::Tls(stream))
        } else {
            Ok(TlsStreamOrPlain::Plain(stream))
        }
    }
    
    pub fn is_enabled(&self) -> bool {
        self.acceptor.is_some()
    }
}

pub enum TlsStreamOrPlain {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl tokio::io::AsyncRead for TlsStreamOrPlain {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            TlsStreamOrPlain::Plain(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            TlsStreamOrPlain::Tls(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for TlsStreamOrPlain {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            TlsStreamOrPlain::Plain(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
            TlsStreamOrPlain::Tls(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
         match self.get_mut() {
            TlsStreamOrPlain::Plain(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            TlsStreamOrPlain::Tls(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
         match self.get_mut() {
            TlsStreamOrPlain::Plain(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            TlsStreamOrPlain::Tls(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}


fn load_certs(path: &Path) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
}

fn load_private_key(path: &Path) -> std::io::Result<PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // First try pkcs8
    if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut reader).next().transpose()? {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    
    // Rewind or re-open
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // Try rsa
    if let Some(key) = rustls_pemfile::rsa_private_keys(&mut reader).next().transpose()? {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }
    
     // Rewind or re-open
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Try ec
    if let Some(key) = rustls_pemfile::ec_private_keys(&mut reader).next().transpose()? {
       return Ok(PrivateKeyDer::Sec1(key));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("No valid private key found in {}", path.display()),
    ))
}
