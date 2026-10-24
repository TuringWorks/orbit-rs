//! PostgreSQL conformance harness: a sqllogictest corpus run over the PostgreSQL wire protocol.
//!
//! The corpus in `tests/conformance/slt/` is only meaningful if real PostgreSQL agrees with it, so
//! the same files run against two targets:
//!
//! * **Orbit** (default: an in-process server; or `ORBIT_CONFORMANCE_URL`) is *measured*. The pass
//!   count per file is checked against `tests/conformance/baseline.json`, a ratchet that may only
//!   move up. `CONFORMANCE_UPDATE_BASELINE=1` rewrites it.
//! * **Reference** (`ORBIT_CONFORMANCE_REFERENCE_URL`, e.g. `docker run postgres:18`) is the
//!   *oracle*. Every record must pass there; a failure is a bug in the corpus, not in Orbit.
//!
//! Both tests are `#[ignore]`d so `cargo test` stays hermetic; `make conformance` runs them.
//! Results land in `target/conformance/report.json`.

use async_trait::async_trait;
use sqllogictest::{AsyncDB, DBOutput, DefaultColumnType, Record, Runner};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

const BASELINE: &str = "conformance/baseline.json";

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/slt")
}

fn corpus_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("corpus directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "slt"))
        .collect();
    files.sort();
    files
}

/// A PostgreSQL connection speaking the simple query protocol; every value arrives as text.
struct PgDb {
    client: Client,
    name: &'static str,
}

impl PgDb {
    async fn connect(url: &str, name: &'static str) -> Result<Self, tokio_postgres::Error> {
        let (client, connection) = tokio_postgres::connect(url, NoTls).await.inspect_err(|e| {
            eprintln!("connect to {name}: {e:?}");
        })?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self { client, name })
    }
}

#[async_trait]
impl AsyncDB for PgDb {
    type Error = tokio_postgres::Error;
    type ColumnType = DefaultColumnType;

    async fn run(&mut self, sql: &str) -> Result<DBOutput<Self::ColumnType>, Self::Error> {
        let mut types = vec![];
        let mut rows = vec![];
        let mut affected = 0;
        for message in self.client.simple_query(sql).await? {
            match message {
                SimpleQueryMessage::Row(row) => {
                    types = vec![DefaultColumnType::Any; row.len()];
                    rows.push(
                        (0..row.len())
                            .map(|i| row.get(i).map_or_else(|| "NULL".into(), render))
                            .collect(),
                    );
                }
                SimpleQueryMessage::CommandComplete(n) => affected = n,
                _ => {}
            }
        }
        Ok(if types.is_empty() {
            DBOutput::StatementComplete(affected)
        } else {
            DBOutput::Rows { types, rows }
        })
    }

    async fn shutdown(&mut self) {}

    fn engine_name(&self) -> &str {
        self.name
    }

    async fn sleep(dur: std::time::Duration) {
        tokio::time::sleep(dur).await;
    }
}

/// Empty strings are written `(empty)` in sqllogictest.
fn render(value: &str) -> String {
    if value.is_empty() {
        "(empty)".into()
    } else {
        value.to_owned()
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct FileResult {
    passed: u32,
    failed: u32,
    /// First few failures, `line: reason`, for the report only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    failures: Vec<String>,
}

/// Runs every record in `file`, counting rather than stopping at the first failure.
async fn run_file(url: &str, name: &'static str, file: &Path) -> FileResult {
    let url = url.to_owned();
    let mut runner = Runner::new(move || {
        let url = url.clone();
        async move { PgDb::connect(&url, name).await }
    });
    let records = sqllogictest::parse_file::<DefaultColumnType>(file).expect("corpus parses");
    let mut result = FileResult::default();
    for record in records {
        if !matches!(record, Record::Statement { .. } | Record::Query { .. }) {
            continue;
        }
        let at = match &record {
            Record::Statement { loc, .. } | Record::Query { loc, .. } => loc.line(),
            _ => 0,
        };
        match runner.run_async(record).await {
            Ok(_) => result.passed += 1,
            Err(e) => {
                result.failed += 1;
                if result.failures.len() < 5 {
                    let why = e.to_string();
                    result
                        .failures
                        .push(format!("{at}: {}", why.lines().next().unwrap_or("")));
                }
            }
        }
    }
    runner.shutdown_async().await;
    result
}

/// `outer: inner: innermost`, since the database's own message is several `source()`s down.
fn error_chain(e: &(dyn std::error::Error + 'static)) -> String {
    let mut text = e.to_string();
    let mut next = e.source();
    while let Some(cause) = next {
        text.push_str(&format!(": {cause}"));
        next = cause.source();
    }
    text
}

/// A failure on one line, trimmed, so the report stays readable.
fn one_line(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(260).collect()
}

async fn run_corpus(url: &str, name: &'static str) -> BTreeMap<String, FileResult> {
    let mut out = BTreeMap::new();
    for file in corpus_files() {
        let key = file.file_name().unwrap().to_string_lossy().into_owned();
        out.insert(key, run_file(url, name, &file).await);
    }
    out
}

/// An in-process Orbit PostgreSQL server on a free port.
async fn start_orbit() -> String {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let server = orbit_server::protocols::PostgresServer::new(format!("127.0.0.1:{port}"))
        .with_auth(orbit_server::protocols::postgres_wire::auth::UserStore::new(), true);
    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("orbit server stopped: {e}");
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    format!("host=127.0.0.1 port={port} user=test password=test dbname=test")
}

fn write_report(target: &str, results: &BTreeMap<String, FileResult>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/conformance");
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_string_pretty(results).unwrap();
    std::fs::write(dir.join(format!("{target}.json")), json).unwrap();
}

fn print_table(title: &str, results: &BTreeMap<String, FileResult>) {
    println!("\n{title}");
    for (file, r) in results {
        let total = r.passed + r.failed;
        println!(
            "  {file:<28} {:>4}/{total:<4} {}",
            r.passed,
            r.failures.first().map_or("", |f| f)
        );
    }
    let (p, t) = results
        .values()
        .fold((0, 0), |(p, t), r| (p + r.passed, t + r.passed + r.failed));
    println!(
        "  {:<28} {p:>4}/{t:<4} ({:.1}%)",
        "TOTAL",
        100.0 * f64::from(p) / f64::from(t.max(1))
    );
}

/// Orbit's pass count per file must not drop below the committed baseline.
#[tokio::test]
#[ignore = "conformance measurement; run with `make conformance`"]
async fn orbit_does_not_regress() {
    let url = match std::env::var("ORBIT_CONFORMANCE_URL") {
        Ok(url) => url,
        Err(_) => start_orbit().await,
    };
    let results = run_corpus(&url, "orbit").await;
    write_report("orbit", &results);
    print_table("Orbit", &results);

    let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE);
    if std::env::var_os("CONFORMANCE_UPDATE_BASELINE").is_some() {
        let floor: BTreeMap<_, _> = results.iter().map(|(k, v)| (k.clone(), v.passed)).collect();
        std::fs::write(
            &baseline_path,
            serde_json::to_string_pretty(&floor).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    let baseline: BTreeMap<String, u32> = serde_json::from_str(
        &std::fs::read_to_string(&baseline_path).unwrap_or_else(|_| "{}".into()),
    )
    .expect("baseline.json is valid");
    let regressions: Vec<String> = baseline
        .iter()
        .filter(|(file, floor)| results.get(*file).map_or(0, |r| r.passed) < **floor)
        .map(|(file, floor)| {
            format!(
                "{file}: {} < baseline {floor}",
                results.get(file).map_or(0, |r| r.passed)
            )
        })
        .collect();
    assert!(
        regressions.is_empty(),
        "conformance regressed:\n{}",
        regressions.join("\n")
    );
}

/// The corpus is only trustworthy if real PostgreSQL passes all of it.
#[tokio::test]
#[ignore = "needs ORBIT_CONFORMANCE_REFERENCE_URL pointing at a real PostgreSQL 18"]
async fn reference_postgres_passes_the_whole_corpus() {
    let Ok(url) = std::env::var("ORBIT_CONFORMANCE_REFERENCE_URL") else {
        panic!("set ORBIT_CONFORMANCE_REFERENCE_URL, e.g. `make conformance-reference`");
    };
    let results = run_corpus(&url, "postgres").await;
    write_report("reference", &results);
    print_table("Reference PostgreSQL", &results);
    let bad: Vec<_> = results
        .iter()
        .filter(|(_, r)| r.failed > 0)
        .map(|(f, r)| format!("{f}: {:?}", r.failures))
        .collect();
    assert!(
        bad.is_empty(),
        "corpus disagrees with real PostgreSQL:\n{}",
        bad.join("\n")
    );
}
