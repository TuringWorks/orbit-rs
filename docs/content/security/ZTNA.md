# Zero Trust Network Access (ZTNA) in Orbit-RS

Orbit-RS implements a Zero Trust Network Access (ZTNA) architecture primarily through **Mutual TLS (mTLS)** for all protocol interactions. This ensures that every client attempting to connect to the Orbit cluster must present a valid, trusted certificate, verifying their identity before any data exchange occurs.

## Overview

Core principles implemented in Orbit:
*   **Encrypted Transit**: All data in transit is encrypted via TLS 1.3.
*   **Strong Identity**: Clients are authenticated via X.509 certificates.
*   **Default Deny**: Connections without valid certificates are rejected at the TCP handshake level.

## Supported Protocols

ZTNA/mTLS is supported on:
*   **PostgreSQL Wire Protocol** (Port 5432)
*   **gRPC Control Plane** (Port 50051)
*   **OrbitWire** (Cluster IPC)

## Configuration

Enable TLS in your `orbit.toml`:

```toml
[server.tls]
enabled = true
# Path to the server's certificate file (PEM format)
cert_file = "/path/to/server_cert.pem"
# Path to the server's private key file (PEM format)
key_file = "/path/to/server_key.pem"
# Path to the CA certificate that signed client certificates (Required for mTLS)
ca_cert_file = "/path/to/ca_cert.pem"
# Enforce client certificate validation (mTLS)
require_client_cert = true
```

### Certificate Generation (Quick Start)

For testing or development, you can generate self-signed certificates using OpenSSL:

1.  **Generate CA**:
    ```bash
    openssl req -x509 -newkey rsa:4096 -keyout ca_key.pem -out ca_cert.pem -days 365 -nodes -subj "/CN=OrbitCA"
    ```
2.  **Generate Server Cert**:
    ```bash
    openssl req -newkey rsa:4096 -keyout server_key.pem -out server.csr -nodes -subj "/CN=localhost"
    openssl x509 -req -in server.csr -CA ca_cert.pem -CAkey ca_key.pem -out server_cert.pem -set_serial 01 -days 365
    ```
3.  **Generate Client Cert**:
    ```bash
    openssl req -newkey rsa:4096 -keyout client_key.pem -out client.csr -nodes -subj "/CN=client"
    openssl x509 -req -in client.csr -CA ca_cert.pem -CAkey ca_key.pem -out client_cert.pem -set_serial 02 -days 365
    ```

## Connecting Clients

### PostgreSQL Clients (`psql`)

When connecting via standard PostgreSQL tools, provide the client certificate and key:

```bash
psql "host=localhost port=5432 user=orbit dbname=default sslmode=verify-full sslrootcert=ca_cert.pem sslcert=client_cert.pem sslkey=client_key.pem"
```

### gRPC Clients (Rust/Tonic)

Configure your Tonic client with `tls_config`:

```rust
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

let ca_cert = std::fs::read("ca_cert.pem")?;
let client_cert = std::fs::read("client_cert.pem")?;
let client_key = std::fs::read("client_key.pem")?;

let identity = Identity::from_pem(client_cert, client_key);
let tls = ClientTlsConfig::new()
    .domain_name("localhost")
    .ca_certificate(Certificate::from_pem(ca_cert))
    .identity(identity);

let channel = Channel::from_static("https://localhost:50051")
    .tls_config(tls)?
    .connect()
    .await?;
```
