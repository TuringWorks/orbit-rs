//! Does data written over the wire survive the server being killed?
//!
//! The conformance harness connects to a server that is already running, so it
//! cannot tell a durable store from a hash map. This test owns the server
//! process: it writes rows, kills it with `SIGKILL` so no shutdown hook and no
//! destructor gets to run, starts it again over the same directory, and reads
//! the rows back.
//!
//! Run it explicitly — it builds nothing and needs the server binary:
//!
//! ```bash
//! cargo test -p orbit-integration-tests --test pg_crash_durability -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use tokio_postgres::{Client, NoTls};

const HOST: &str = "127.0.0.1";
const USER: &str = "orbit";
/// Away from the default 5432 so a server someone is already running does not
/// answer these queries and make the test pass without proving anything.
const PORT: u16 = 55432;

/// Where the workspace is, derived from this crate rather than the working
/// directory, so the test runs the same from anywhere.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the tests crate has a parent")
        .to_path_buf()
}

fn server_binary() -> PathBuf {
    workspace_root().join("target/debug/orbit-server")
}

/// Derive this test's configuration from the shipped one.
///
/// Starting from the real file rather than a hand-written minimal one means
/// the test keeps working when the schema grows a required field, and it means
/// the settings under test are the settings operators actually get. Only the
/// data directory, the port, and the other protocols are changed.
fn write_config(directory: &Path) -> PathBuf {
    let shipped = workspace_root().join("config/orbit-server.toml");
    let text = std::fs::read_to_string(&shipped)
        .unwrap_or_else(|e| panic!("read {}: {e}", shipped.display()));
    let mut config: toml::Value = toml::from_str(&text).expect("the shipped configuration parses");

    let data_dir = directory.join("data");

    let protocols = config
        .get_mut("protocols")
        .and_then(toml::Value::as_table_mut)
        .expect("[protocols]");
    for (name, protocol) in protocols.iter_mut() {
        let Some(table) = protocol.as_table_mut() else {
            continue;
        };
        // Everything but PostgreSQL is off, so this test never contends for a
        // port with a server the developer is already running.
        let is_postgres = name == "postgresql";
        table.insert("enabled".into(), toml::Value::Boolean(is_postgres));
        if is_postgres {
            table.insert("port".into(), toml::Value::Integer(i64::from(PORT)));
        }
    }

    let unified = config
        .get_mut("unified_storage")
        .and_then(toml::Value::as_table_mut)
        .expect("[unified_storage]");
    unified.insert("enabled".into(), toml::Value::Boolean(true));
    unified.insert(
        "data_dir".into(),
        toml::Value::String(data_dir.to_string_lossy().into_owned()),
    );

    let warm = unified
        .get_mut("warm_tier")
        .and_then(toml::Value::as_table_mut)
        .expect("[unified_storage.warm_tier]");
    warm.insert(
        "data_dir".into(),
        toml::Value::String(data_dir.join("rocksdb").to_string_lossy().into_owned()),
    );
    // The setting under test. Asserted rather than assumed, so the test still
    // means something if the shipped default is ever turned back off.
    assert_eq!(
        warm.get("sync_wal"),
        Some(&toml::Value::Boolean(true)),
        "the shipped configuration should acknowledge writes only once they are \
         on disk; this test cannot prove durability without it"
    );

    let path = directory.join("orbit-server.toml");
    std::fs::write(&path, toml::to_string(&config).expect("serialize"))
        .expect("write the configuration");
    path
}

/// Start the server and wait until it answers on the PostgreSQL port.
fn start_server(config: &Path, log: &Path) -> Child {
    let output = std::fs::File::create(log).expect("create the log file");
    let errors = output.try_clone().expect("clone the log handle");

    // Ports come from the command line, not the configuration file, because
    // `apply_cli_overrides` writes clap's defaults over whatever the file said
    // — a flag that was never passed still wins. Every port is moved out of
    // the way so this test never collides with a server already running, and
    // `--data-dir` keeps the other protocols' stores inside the test directory
    // instead of scattering them through the working directory.
    let directory = config.parent().expect("the config has a parent");
    let child = Command::new(server_binary())
        .arg("--config")
        .arg(config)
        .args(["--bind", HOST])
        .args(["--postgres-port", &PORT.to_string()])
        .args(["--redis-port", "56379"])
        .args(["--mysql-port", "53306"])
        .args(["--cql-port", "59042"])
        .args(["--grpc-port", "50151"])
        .args(["--http-port", "58080"])
        .args(["--metrics-port", "59090"])
        .arg("--data-dir")
        .arg(directory.join("data"))
        .stdout(output)
        .stderr(errors)
        .spawn()
        .unwrap_or_else(|e| {
            panic!(
                "could not start {}: {e}. Build it first: cargo build -p orbit-server",
                server_binary().display()
            )
        });

    child
}

async fn wait_until_listening(log: &Path) {
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        if tokio::net::TcpStream::connect((HOST, PORT)).await.is_ok() {
            // Listening is not the same as ready to answer a query.
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let tail = std::fs::read_to_string(log).unwrap_or_default();
    panic!(
        "the server never listened on {PORT}. Log:\n{}",
        tail.lines().rev().take(40).collect::<Vec<_>>().join("\n")
    );
}

async fn connect() -> Client {
    let mut config = tokio_postgres::Config::new();
    config
        .host(HOST)
        .port(PORT)
        .user(USER)
        .password(USER)
        .connect_timeout(Duration::from_secs(5));

    let (client, connection) = config.connect(NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// Kill the process outright. No signal handler, no flush, no `Drop` — the
/// state on disk is whatever the writes already put there.
fn kill_hard(mut child: Child) {
    unsafe {
        libc::kill(child.id() as i32, libc::SIGKILL);
    }
    let _ = child.wait();
}

#[tokio::test]
#[ignore = "owns a server process and a data directory; run explicitly"]
async fn rows_written_over_the_wire_survive_sigkill() {
    let directory = workspace_root().join("target/crash-durability");
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("create the test directory");

    let config = write_config(&directory);
    let first_log = directory.join("first.log");
    let second_log = directory.join("second.log");

    // --- First life: create a table and write rows. ---
    let server = start_server(&config, &first_log);
    wait_until_listening(&first_log).await;

    {
        let client = connect().await;
        client
            .simple_query("CREATE TABLE durable (id INT PRIMARY KEY, note TEXT)")
            .await
            .expect("create the table");

        for id in 1..=25 {
            client
                .simple_query(&format!(
                    "INSERT INTO durable (id, note) VALUES ({id}, 'row-{id}')"
                ))
                .await
                .unwrap_or_else(|e| panic!("insert {id}: {e}"));
        }

        let before = count(&client).await;
        assert_eq!(before, 25, "the rows should be there before the kill");
    }

    kill_hard(server);

    // --- Second life: same directory, nothing was flushed on the way out. ---
    let server = start_server(&config, &second_log);
    wait_until_listening(&second_log).await;

    let client = connect().await;
    let after = count(&client).await;
    assert_eq!(
        after, 25,
        "every acknowledged row must still be there after SIGKILL, found {after}"
    );

    // The rows must also be intact, not merely counted: a store that returns
    // the right number of damaged rows has still lost the data.
    let notes = simple_column(&client, "SELECT note FROM durable ORDER BY id").await;
    let expected: Vec<String> = (1..=25).map(|id| format!("row-{id}")).collect();
    assert_eq!(notes, expected, "the rows came back damaged or reordered");

    // The schema has to survive as more than a list of column names: if the
    // constraints were lost, the table would accept a duplicate key and the
    // damage would only show up later, as two rows with the same identity.
    let duplicate = client
        .simple_query("INSERT INTO durable (id, note) VALUES (1, 'impostor')")
        .await;
    assert!(
        duplicate.is_err(),
        "the primary key did not survive the restart: a duplicate id was accepted"
    );
    assert_eq!(
        count(&client).await,
        25,
        "the rejected insert must not have landed"
    );

    kill_hard(server);
    let _ = std::fs::remove_dir_all(&directory);
}

/// Read one column of one row as text.
///
/// The simple-query protocol returns everything as text, which keeps this test
/// from depending on which type OID the server picks for `COUNT(*)` — that is
/// the conformance suite's job, not this one's.
async fn simple_column(client: &Client, sql: &str) -> Vec<String> {
    client
        .simple_query(sql)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
        .into_iter()
        .filter_map(|message| match message {
            tokio_postgres::SimpleQueryMessage::Row(row) => {
                Some(row.get(0).unwrap_or_default().to_string())
            }
            _ => None,
        })
        .collect()
}

async fn count(client: &Client) -> i64 {
    let values = simple_column(client, "SELECT COUNT(*) FROM durable").await;
    values
        .first()
        .unwrap_or_else(|| panic!("COUNT(*) returned no row"))
        .parse()
        .unwrap_or_else(|e| panic!("COUNT(*) returned {values:?}, which is not a number: {e}"))
}
