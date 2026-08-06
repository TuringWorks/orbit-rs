//! Local cluster lifecycle: start, stop, and observe an Orbit-RS dev cluster.
//!
//! Everything here is derived from things that can be checked:
//!
//! * which PID files `scripts/start-cluster.sh` wrote,
//! * whether those processes are actually alive, and for how long, from `ps`,
//! * which ports each live process was told to listen on, read from its own
//!   command line rather than recomputed from the script's port arithmetic,
//! * whether those ports currently accept a TCP connection.
//!
//! Nothing is reported that was not observed. In particular this module does
//! not read `orbit-server`'s `/api/v1/cluster/*` endpoints: those handlers
//! return fixed values (`cpu_usage: 45.2`, `uptime_seconds: 86400`,
//! `actor_count: 150`, `replication_factor: 3`) that are not measurements, and
//! surfacing them in a UI would present invented numbers as telemetry. Fields
//! this module cannot observe — a node's role, its actor count — are absent
//! rather than filled in.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long to wait for a port to accept a connection before calling it closed.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// Where `start-cluster.sh` keeps its state, relative to the repository root.
const PID_SUBDIR: &str = "cluster-data/pids";
const LOG_SUBDIR: &str = "cluster-data/logs";
const CLUSTER_SCRIPT: &str = "scripts/start-cluster.sh";
/// Combined stdout/stderr of the most recent start/stop invocation.
const CONTROL_LOG: &str = "cluster-data/logs/cluster-control.log";

/// The protocol flags `start-cluster.sh` passes to each node, paired with the
/// label shown in the UI. Read back off the live process's command line.
const PORT_FLAGS: [(&str, &str); 7] = [
    ("--postgres-port", "PostgreSQL"),
    ("--redis-port", "Redis"),
    ("--mysql-port", "MySQL"),
    ("--cql-port", "CQL"),
    ("--http-port", "HTTP"),
    ("--grpc-port", "gRPC"),
    ("--metrics-port", "Metrics"),
];

/// Failures managing a local cluster.
#[derive(Debug, thiserror::Error)]
pub enum ClusterError {
    #[error("Orbit-RS repository root not found. Set it in the cluster panel.")]
    RootNotSet,
    #[error("{0} is not an Orbit-RS checkout: {1} is missing")]
    NotARepository(PathBuf, &'static str),
    #[error("Cluster script failed: {0}")]
    ScriptFailed(String),
    #[error("IO error: {0}")]
    Io(String),
}

/// Whether a node's process is alive, and for how long.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProcessState {
    /// The PID from the pid file is alive. `uptime_seconds` comes from `ps`.
    Running { pid: u32, uptime_seconds: u64 },
    /// A pid file exists but that process is gone — a crash or an unclean stop.
    Exited { pid: u32 },
}

/// One listening port and whether it answered just now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub protocol: String,
    pub port: u16,
    pub reachable: bool,
}

/// A node discovered from the cluster's pid directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub process: ProcessState,
    /// Ports read from the running process's own command line.
    ///
    /// Empty when the process is not running: the ports it *would* use are a
    /// guess, and a guess rendered next to live ones would not be
    /// distinguishable from a measurement.
    pub endpoints: Vec<Endpoint>,
    pub log_file: String,
}

impl ClusterNode {
    /// A node counts as healthy only if it is alive *and* answering.
    ///
    /// A process that is up but whose listeners refuse connections is exactly
    /// the state worth spotting, so the two facts stay separate in the payload.
    pub fn is_serving(&self) -> bool {
        matches!(self.process, ProcessState::Running { .. })
            && !self.endpoints.is_empty()
            && self.endpoints.iter().any(|e| e.reachable)
    }
}

/// A point-in-time observation of the local cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub root: String,
    /// False when no pid directory exists — no cluster has been started here.
    pub initialized: bool,
    pub nodes: Vec<ClusterNode>,
    pub running_nodes: usize,
    pub serving_nodes: usize,
    pub checked_at: DateTime<Utc>,
}

/// Locates the repository and drives `start-cluster.sh`.
#[derive(Debug)]
pub struct ClusterManager {
    root: PathBuf,
}

impl ClusterManager {
    /// Verify `root` is an Orbit-RS checkout with the cluster script present.
    ///
    /// # Errors
    /// Returns [`ClusterError::NotARepository`] when a required path is absent.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, ClusterError> {
        let root = root.into();
        for (relative, label) in [
            ("Cargo.toml", "Cargo.toml"),
            ("orbit", "the orbit/ directory"),
            (CLUSTER_SCRIPT, "scripts/start-cluster.sh"),
        ] {
            if !root.join(relative).exists() {
                return Err(ClusterError::NotARepository(root, label));
            }
        }
        Ok(Self { root })
    }

    /// Walk up from `start` looking for an Orbit-RS checkout.
    pub fn discover(start: &Path) -> Option<Self> {
        start
            .ancestors()
            .find_map(|candidate| Self::new(candidate).ok())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn pid_dir(&self) -> PathBuf {
        self.root.join(PID_SUBDIR)
    }

    /// Observe the cluster: which nodes exist, which are alive, what answers.
    ///
    /// # Errors
    /// Returns [`ClusterError::Io`] when the pid directory exists but cannot
    /// be read.
    pub async fn status(&self) -> Result<ClusterStatus, ClusterError> {
        let pid_dir = self.pid_dir();
        let checked_at = Utc::now();

        if !pid_dir.is_dir() {
            return Ok(ClusterStatus {
                root: self.root.display().to_string(),
                initialized: false,
                nodes: Vec::new(),
                running_nodes: 0,
                serving_nodes: 0,
                checked_at,
            });
        }

        let mut pids = read_pid_files(&pid_dir)?;
        pids.sort_by(|a, b| a.0.cmp(&b.0));

        let live = inspect_processes(pids.iter().map(|(_, pid)| *pid)).await;

        let mut nodes = Vec::with_capacity(pids.len());
        for (node_id, pid) in pids {
            let log_file = self
                .root
                .join(LOG_SUBDIR)
                .join(format!("{node_id}.log"))
                .display()
                .to_string();

            let node = match live.get(&pid) {
                Some(process) => ClusterNode {
                    node_id,
                    process: ProcessState::Running {
                        pid,
                        uptime_seconds: process.uptime_seconds,
                    },
                    endpoints: probe_endpoints(&process.command_line).await,
                    log_file,
                },
                None => ClusterNode {
                    node_id,
                    process: ProcessState::Exited { pid },
                    endpoints: Vec::new(),
                    log_file,
                },
            };
            nodes.push(node);
        }

        let running_nodes = nodes
            .iter()
            .filter(|n| matches!(n.process, ProcessState::Running { .. }))
            .count();
        let serving_nodes = nodes.iter().filter(|n| n.is_serving()).count();

        Ok(ClusterStatus {
            root: self.root.display().to_string(),
            initialized: true,
            nodes,
            running_nodes,
            serving_nodes,
            checked_at,
        })
    }

    /// Start an `size`-node cluster.
    ///
    /// Returns as soon as the script is running, not when the cluster is up:
    /// the script builds `orbit-server` in release mode first, which can take
    /// minutes. Poll [`ClusterManager::status`] to see nodes come up, and read
    /// [`ClusterManager::control_log`] for the script's own output.
    ///
    /// # Errors
    /// Returns [`ClusterError::ScriptFailed`] when the script cannot be spawned.
    pub async fn start(&self, size: u8) -> Result<(), ClusterError> {
        if size == 0 {
            return Err(ClusterError::ScriptFailed(
                "cluster size must be at least 1".to_string(),
            ));
        }
        self.spawn_script(&[size.to_string()]).await
    }

    /// Stop every node recorded in the pid directory.
    ///
    /// # Errors
    /// Returns [`ClusterError::ScriptFailed`] when the script exits non-zero.
    pub async fn stop(&self) -> Result<(), ClusterError> {
        self.run_script(&["--stop".to_string()]).await
    }

    /// Last `lines` lines of a node's log.
    ///
    /// # Errors
    /// Returns [`ClusterError::Io`] when the log file cannot be read.
    pub fn node_log(&self, node_id: &str, lines: usize) -> Result<String, ClusterError> {
        let path = self.root.join(LOG_SUBDIR).join(format!("{node_id}.log"));
        read_tail(&path, lines)
    }

    /// Last `lines` lines of output from the most recent start/stop.
    ///
    /// # Errors
    /// Returns [`ClusterError::Io`] when the log exists but cannot be read.
    pub fn control_log(&self, lines: usize) -> Result<String, ClusterError> {
        read_tail(&self.root.join(CONTROL_LOG), lines)
    }

    /// Run the script and wait for it, surfacing its output on failure.
    async fn run_script(&self, args: &[String]) -> Result<(), ClusterError> {
        let output = tokio::process::Command::new("bash")
            .arg(CLUSTER_SCRIPT)
            .args(args)
            .current_dir(&self.root)
            .output()
            .await
            .map_err(|e| ClusterError::ScriptFailed(e.to_string()))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(ClusterError::ScriptFailed(format!(
            "{} exited with {}: {}",
            CLUSTER_SCRIPT,
            output.status,
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        )))
    }

    /// Spawn the script detached, tee-ing its output into the control log.
    async fn spawn_script(&self, args: &[String]) -> Result<(), ClusterError> {
        let log_path = self.root.join(CONTROL_LOG);
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ClusterError::Io(e.to_string()))?;
        }

        let log = std::fs::File::create(&log_path).map_err(|e| ClusterError::Io(e.to_string()))?;
        let errors = log
            .try_clone()
            .map_err(|e| ClusterError::Io(e.to_string()))?;

        tokio::process::Command::new("bash")
            .arg(CLUSTER_SCRIPT)
            .args(args)
            .current_dir(&self.root)
            .stdout(log)
            .stderr(errors)
            .stdin(std::process::Stdio::null())
            .spawn()
            .map_err(|e| ClusterError::ScriptFailed(e.to_string()))?;

        Ok(())
    }
}

/// A live process as `ps` reported it.
struct LiveProcess {
    uptime_seconds: u64,
    command_line: String,
}

/// Read `node-N.pid` files, skipping anything unparseable.
fn read_pid_files(pid_dir: &Path) -> Result<Vec<(String, u32)>, ClusterError> {
    let entries = std::fs::read_dir(pid_dir).map_err(|e| ClusterError::Io(e.to_string()))?;

    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "pid" {
                return None;
            }
            let node_id = path.file_stem()?.to_str()?.to_string();
            let pid = std::fs::read_to_string(&path).ok()?.trim().parse().ok()?;
            Some((node_id, pid))
        })
        .collect())
}

/// Ask `ps` which of `pids` are alive, how long they have run, and with what
/// arguments. One invocation covers every node.
async fn inspect_processes(
    pids: impl IntoIterator<Item = u32>,
) -> HashMap<u32, LiveProcess> {
    let list: Vec<String> = pids.into_iter().map(|pid| pid.to_string()).collect();
    if list.is_empty() {
        return HashMap::new();
    }

    let output = tokio::process::Command::new("ps")
        .args(["-o", "pid=,etime=,args=", "-p", &list.join(",")])
        .output()
        .await;

    let Ok(output) = output else {
        tracing::warn!("could not run ps to inspect cluster processes");
        return HashMap::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_ps_line)
        .collect()
}

/// Parse one `pid etime args...` line.
fn parse_ps_line(line: &str) -> Option<(u32, LiveProcess)> {
    let mut fields = line.trim().splitn(3, char::is_whitespace);
    let pid = fields.next()?.trim().parse().ok()?;
    let elapsed = fields.next()?.trim();
    let command_line = fields.next().unwrap_or_default().trim().to_string();

    Some((
        pid,
        LiveProcess {
            uptime_seconds: parse_etime(elapsed)?,
            command_line,
        },
    ))
}

/// Convert `ps` elapsed time — `mm:ss`, `hh:mm:ss` or `dd-hh:mm:ss` — to seconds.
fn parse_etime(etime: &str) -> Option<u64> {
    let (days, clock) = match etime.split_once('-') {
        Some((days, rest)) => (days.parse::<u64>().ok()?, rest),
        None => (0, etime),
    };

    let parts: Vec<u64> = clock
        .split(':')
        .map(|part| part.parse::<u64>())
        .collect::<Result<_, _>>()
        .ok()?;

    let clock_seconds = match parts.as_slice() {
        [minutes, seconds] => minutes * 60 + seconds,
        [hours, minutes, seconds] => hours * 3600 + minutes * 60 + seconds,
        _ => return None,
    };

    Some(days * 86_400 + clock_seconds)
}

/// Read the `--*-port` flags out of a node's command line and probe each one.
async fn probe_endpoints(command_line: &str) -> Vec<Endpoint> {
    let args: Vec<&str> = command_line.split_whitespace().collect();

    let declared: Vec<(&str, u16)> = PORT_FLAGS
        .iter()
        .filter_map(|(flag, label)| {
            let index = args.iter().position(|arg| arg == flag)?;
            let port = args.get(index + 1)?.parse().ok()?;
            Some((*label, port))
        })
        .collect();

    // Probed concurrently: a node with several closed ports would otherwise
    // cost one full timeout per port, and the status call is on the UI's
    // refresh path.
    let mut probes = tokio::task::JoinSet::new();
    for (index, (protocol, port)) in declared.into_iter().enumerate() {
        probes.spawn(async move {
            (
                index,
                Endpoint {
                    protocol: protocol.to_string(),
                    port,
                    reachable: is_port_open(port).await,
                },
            )
        });
    }

    let mut probed: Vec<(usize, Endpoint)> = Vec::with_capacity(probes.len());
    while let Some(joined) = probes.join_next().await {
        match joined {
            Ok(result) => probed.push(result),
            Err(e) => tracing::warn!("port probe task failed: {e}"),
        }
    }

    // Restore the PORT_FLAGS order, which completion order does not preserve.
    probed.sort_by_key(|(index, _)| *index);
    probed.into_iter().map(|(_, endpoint)| endpoint).collect()
}

/// Does something accept a TCP connection on this port right now?
async fn is_port_open(port: u16) -> bool {
    tokio::time::timeout(
        PORT_PROBE_TIMEOUT,
        tokio::net::TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map(|result| result.is_ok())
    .unwrap_or(false)
}

/// Last `lines` lines of a file, or a clear message when there is no file yet.
fn read_tail(path: &Path, lines: usize) -> Result<String, ClusterError> {
    if !path.exists() {
        return Ok(format!("No log at {}", path.display()));
    }

    let content = std::fs::read_to_string(path).map_err(|e| ClusterError::Io(e.to_string()))?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etime_parses_every_ps_layout() {
        assert_eq!(parse_etime("00:42"), Some(42));
        assert_eq!(parse_etime("06:49:18"), Some(6 * 3600 + 49 * 60 + 18));
        assert_eq!(parse_etime("2-03:00:00"), Some(2 * 86_400 + 3 * 3600));
        assert_eq!(parse_etime(""), None);
        assert_eq!(parse_etime("not-a-time"), None);
    }

    #[test]
    fn ps_lines_split_into_pid_uptime_and_command() {
        let (pid, process) =
            parse_ps_line("  1234 01:02:03 ./target/release/orbit-server --node-id node-1")
                .expect("a well formed ps line parses");
        assert_eq!(pid, 1234);
        assert_eq!(process.uptime_seconds, 3723);
        assert!(process.command_line.ends_with("--node-id node-1"));
    }

    #[test]
    fn a_ps_line_without_a_command_still_yields_uptime() {
        let (pid, process) = parse_ps_line("77 00:05").expect("pid and etime suffice");
        assert_eq!(pid, 77);
        assert_eq!(process.uptime_seconds, 5);
        assert!(process.command_line.is_empty());
    }

    #[tokio::test]
    async fn endpoints_come_from_the_command_line_not_from_port_arithmetic() {
        let command = "./target/release/orbit-server --node-id node-2 --grpc-port 50052 \
                       --http-port 8081 --postgres-port 5433 --redis-port 6380 \
                       --mysql-port 3307 --cql-port 9043 --metrics-port 9091";
        let endpoints = probe_endpoints(command).await;

        let by_protocol: HashMap<&str, u16> = endpoints
            .iter()
            .map(|e| (e.protocol.as_str(), e.port))
            .collect();

        assert_eq!(by_protocol.get("PostgreSQL"), Some(&5433));
        assert_eq!(by_protocol.get("Redis"), Some(&6380));
        assert_eq!(by_protocol.get("CQL"), Some(&9043));
        assert_eq!(endpoints.len(), PORT_FLAGS.len());
    }

    #[tokio::test]
    async fn a_command_line_without_port_flags_reports_no_endpoints() {
        assert!(probe_endpoints("./target/release/orbit-server").await.is_empty());
    }

    #[test]
    fn a_node_that_is_up_but_answering_nothing_is_not_serving() {
        let node = ClusterNode {
            node_id: "node-1".to_string(),
            process: ProcessState::Running {
                pid: 1,
                uptime_seconds: 10,
            },
            endpoints: vec![Endpoint {
                protocol: "Redis".to_string(),
                port: 6379,
                reachable: false,
            }],
            log_file: String::new(),
        };
        assert!(!node.is_serving());
    }

    #[test]
    fn rejecting_a_directory_that_is_not_an_orbit_checkout() {
        let error = ClusterManager::new(std::env::temp_dir())
            .expect_err("the temp dir is not an Orbit-RS checkout");
        assert!(matches!(error, ClusterError::NotARepository(..)));
    }
}

/// Tests that need the real repository and real processes.
///
/// These verify the parts that cannot be checked from a unit test: that a pid
/// file written on disk is discovered, that `ps` on this machine reports the
/// process, and that a port with a real listener behind it probes as reachable
/// while a port without one does not.
///
/// ```text
/// cargo test --manifest-path orbit/desktop/src-tauri/Cargo.toml -- --ignored --test-threads=1
/// ```
#[cfg(test)]
mod live_tests {
    use super::*;

    /// A port pair well outside the ranges `start-cluster.sh` uses, so a real
    /// cluster running alongside this test cannot be mistaken for the fixture.
    const LISTENING_PORT: u16 = 47731;
    const SILENT_PORT: u16 = 47732;

    /// Removes the fixture's pid file even if an assertion panics, so a failed
    /// run cannot leave a stale node in the user's cluster panel.
    struct Fixture {
        pid_file: PathBuf,
        child: std::process::Child,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_file(&self.pid_file);
        }
    }

    fn repo_root() -> ClusterManager {
        // CARGO_MANIFEST_DIR is <repo>/orbit/desktop/src-tauri.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ClusterManager::discover(&manifest).expect("tests run inside the orbit-rs checkout")
    }

    /// Spawn a process that holds one port open and leaves the cluster's port
    /// flags on its own command line, which is where `status` reads them from.
    fn spawn_fixture(manager: &ClusterManager, node_id: &str) -> Fixture {
        let script = format!(
            "import socket, time, sys\n\
             s = socket.socket()\n\
             s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\n\
             s.bind(('127.0.0.1', {LISTENING_PORT}))\n\
             s.listen(8)\n\
             time.sleep(600)\n"
        );

        let child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg("--postgres-port")
            .arg(LISTENING_PORT.to_string())
            .arg("--redis-port")
            .arg(SILENT_PORT.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("python3 is needed to stand up the fixture listener");

        let pid_dir = manager.pid_dir();
        std::fs::create_dir_all(&pid_dir).expect("create pid dir");
        let pid_file = pid_dir.join(format!("{node_id}.pid"));
        std::fs::write(&pid_file, child.id().to_string()).expect("write pid file");

        Fixture { pid_file, child }
    }

    #[tokio::test]
    #[ignore = "spawns a real process and binds a real port"]
    async fn status_reports_a_live_process_and_distinguishes_open_from_closed_ports() {
        let manager = repo_root();
        let node_id = "node-test-fixture";
        let fixture = spawn_fixture(&manager, node_id);

        // Give the listener a moment to reach listen(2).
        tokio::time::sleep(Duration::from_millis(500)).await;

        let status = manager.status().await.expect("status should read the pid dir");
        assert!(status.initialized, "the pid directory exists");

        let node = status
            .nodes
            .iter()
            .find(|n| n.node_id == node_id)
            .expect("the fixture's pid file should be discovered");

        let ProcessState::Running { pid, .. } = node.process else {
            panic!("the fixture process is alive, so it must report as running");
        };
        assert_eq!(pid, fixture.child.id(), "the reported pid is the fixture's");

        let by_protocol: HashMap<&str, &Endpoint> = node
            .endpoints
            .iter()
            .map(|e| (e.protocol.as_str(), e))
            .collect();

        // Both flags were on the command line, so both are reported...
        assert_eq!(by_protocol.len(), 2, "two --*-port flags were passed");
        // ...but only the bound one answers. This is the distinction the panel
        // relies on to show "running but not serving".
        assert!(
            by_protocol["PostgreSQL"].reachable,
            "port {LISTENING_PORT} has a live listener"
        );
        assert!(
            !by_protocol["Redis"].reachable,
            "port {SILENT_PORT} has nothing bound to it"
        );
        assert!(node.is_serving(), "one reachable port counts as serving");
    }

    #[tokio::test]
    #[ignore = "spawns a real process and binds a real port"]
    async fn a_pid_file_whose_process_has_gone_reports_as_exited() {
        let manager = repo_root();
        let node_id = "node-test-exited";

        let pid_dir = manager.pid_dir();
        std::fs::create_dir_all(&pid_dir).expect("create pid dir");
        let pid_file = pid_dir.join(format!("{node_id}.pid"));

        // Start a process, record it, then let it finish: the pid file now
        // points at something that no longer exists, exactly as it would after
        // a node crashed.
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("spawn a process that exits immediately");
        std::fs::write(&pid_file, child.id().to_string()).expect("write pid file");
        let _ = child.wait();
        tokio::time::sleep(Duration::from_millis(300)).await;

        let status = manager.status().await.expect("status");
        let node = status
            .nodes
            .iter()
            .find(|n| n.node_id == node_id)
            .expect("the pid file should still be discovered");

        assert!(
            matches!(node.process, ProcessState::Exited { .. }),
            "a dead pid must not be reported as running, got {:?}",
            node.process
        );
        assert!(node.endpoints.is_empty(), "no ports are claimed for a dead node");
        assert!(!node.is_serving());

        let _ = std::fs::remove_file(&pid_file);
    }
}
