# Load Balancing Orbit-RS with ZTNA

When running Orbit-RS in a Zero Trust Network Access (ZTNA) environment with Mutual TLS (mTLS) enabled, load balancing requires specific configuration to ensure end-to-end security and correct certificate validation.

## Core Principle: Layer 4 TCP Passthrough

Because Orbit-RS servers enforce mTLS (requiring client certificates), the load balancer **cannot** terminate TLS. It must operate in **Layer 4 (TCP) Passthrough** mode.

*   **Client** --(Encrypted mTLS)--> **Load Balancer** --(Encrypted mTLS)--> **Orbit Server**

The load balancer simply forwards TCP packets. It does not inspect the traffic content, nor does it present its own certificate to the client.

## 1. Using `orbit-lb` (Lightweight Load Balancer)

Orbit provides a built-in, lightweight TCP proxy (`orbit-lb`) designed for development and testing. It natively supports TCP passthrough.

### Usage
```bash
# Start 3 Orbit nodes on ports 5433, 5434, 5435
orbit-server --config node1.toml # Port 5433
orbit-server --config node2.toml # Port 5434
orbit-server --config node3.toml # Port 5435

# Start Load Balancer listening on standard 5432
orbit-lb --postgres 5432:5433,5434,5435
```

### ZTNA Compatibility
`orbit-lb` is fully compatible with ZTNA out of the box. Point your mTLS-configured client to the load balancer port (e.g., `5432`). Ensure your server certificates include the load balancer's hostname/IP in their **Subject Alternative Names (SANs)**.

## 2. Using HAProxy (Production)

For production, HAProxy is recommended. Configure a `listen` block in `mode tcp`.

```haproxy
listen orbit_postgres
    bind *:5432
    mode tcp
    option tcplog
    balance roundrobin
    
    # TCP passthrough - no 'ssl crt' here
    server node1 10.0.0.1:5432 check port 9091 inter 5s fall 3 rise 2
    server node2 10.0.0.2:5432 check port 9091 inter 5s fall 3 rise 2
    server node3 10.0.0.3:5432 check port 9091 inter 5s fall 3 rise 2
```

> **Note:** Use the separate HTTP health check port (default 9091) for `check`. Do not try to health-check the mTLS port with simple TCP checks if strictly requiring certs, though purely TCP-connect checks often suffice.

## 3. Using Nginx (Production)

Use the `stream` module for TCP load balancing.

```nginx
stream {
    upstream orbit_cluster {
        server 10.0.0.1:5432;
        server 10.0.0.2:5432;
        server 10.0.0.3:5432;
    }

    server {
        listen 5432;
        proxy_pass orbit_cluster;
        proxy_timeout 600s;
        proxy_connect_timeout 5s;
    }
}
```

## Certificate Requirements

When connecting via a load balancer, the client performs hostname verification against the **Load Balancer's Address**.

*   **Server Certificates:** Must include the Load Balancer's DNS name or IP address in the `Subject Alt Name (SAN)` field.
    *   *Example:* If LB is `lb.orbit.internal`, server certs must have `DNS:lb.orbit.internal`.

If this is not configured, clients will reject the connection with `Certificate unknown` or `Hostname mismatch` errors.
