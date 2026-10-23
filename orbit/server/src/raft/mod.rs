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
