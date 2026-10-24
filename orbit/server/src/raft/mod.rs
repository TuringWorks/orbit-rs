//! Raft consensus for the replicated SQL store: a pure core ([`core`]), the storage it persists to
//! ([`storage`]), the driver that runs it ([`node`]) and a TCP transport ([`net`]).

pub mod core;
pub mod net;
pub mod node;
#[cfg(feature = "storage-rocksdb")]
pub mod sql;
pub mod storage;

/// The tests that run real nodes use real timers and sockets; running them one at a time keeps a busy test run from
/// starving a node past its election timeout.
/// Ports for tests that run real nodes, taken from a range the operating system never hands out as ephemeral
/// ports, so two tests (or a test and an unrelated socket) cannot pick the same one.
#[cfg(test)]
pub(crate) fn test_ports(n: usize) -> Vec<u16> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let base = 20_000 + (std::process::id() % 200) as u16 * 100;
    let mut out = Vec::new();
    while out.len() < n {
        // Keep the counter wider than the port offset: the full Raft test
        // suite can allocate enough ports to overflow an AtomicU16 in debug
        // builds. Reuse the bounded per-process range after checking it is
        // currently free.
        let p = base + (NEXT.fetch_add(1, Ordering::SeqCst) % 80) as u16;
        if std::net::TcpListener::bind(("127.0.0.1", p)).is_ok() {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
pub(crate) static TCP_TESTS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
mod node_tests;
#[cfg(test)]
mod sim;
#[cfg(all(test, feature = "storage-rocksdb"))]
mod sql_tests;
#[cfg(test)]
mod tests;
