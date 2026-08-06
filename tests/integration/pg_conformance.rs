//! PostgreSQL wire-protocol conformance harness.
//!
//! Drives `orbit-server` with `tokio-postgres` — a conforming client, not a
//! bespoke one — and records which protocol features actually work. The point
//! is a re-measurable number: run it before and after a change and compare.
//!
//! A check that fails is reported, not skipped, so the gap stays visible.
//!
//! ```text
//! ./target/debug/orbit-server --dev-mode --data-dir /tmp/pgconf --config config/orbit-server.toml &
//! cargo test -p orbit-integration-tests --test pg_conformance -- --ignored --nocapture
//! ```
//!
//! `orbit-server` auto-registers an unknown user with password == username, so
//! `orbit`/`orbit` connects to a fresh dev server.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use tokio_postgres::{Client, NoTls};

/// Where the server under test is listening.
const HOST: &str = "127.0.0.1";
const PORT: u16 = 5432;
const USER: &str = "orbit";

/// One conformance area, so the report groups related failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Area {
    Connection,
    SimpleQuery,
    ExtendedQuery,
    Portals,
    Types,
    Transactions,
    Catalog,
    Copy,
    Notify,
    Errors,
}

impl Area {
    fn name(self) -> &'static str {
        match self {
            Area::Connection => "connection",
            Area::SimpleQuery => "simple query",
            Area::ExtendedQuery => "extended query",
            Area::Portals => "portals / cursors",
            Area::Types => "data types",
            Area::Transactions => "transactions",
            Area::Catalog => "catalog (pg_catalog)",
            Area::Copy => "COPY",
            Area::Notify => "LISTEN / NOTIFY",
            Area::Errors => "error reporting",
        }
    }
}

/// The outcome of one check.
struct Outcome {
    area: Area,
    name: &'static str,
    passed: bool,
    detail: String,
}

/// Collects outcomes and renders the report.
#[derive(Default)]
struct Report {
    outcomes: Vec<Outcome>,
}

impl Report {
    fn record(&mut self, area: Area, name: &'static str, result: Result<(), String>) {
        let (passed, detail) = match result {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        };
        self.outcomes.push(Outcome {
            area,
            name,
            passed,
            detail,
        });
    }

    fn passed(&self) -> usize {
        self.outcomes.iter().filter(|o| o.passed).count()
    }

    fn render(&self) -> String {
        let mut by_area: BTreeMap<Area, Vec<&Outcome>> = BTreeMap::new();
        for outcome in &self.outcomes {
            by_area.entry(outcome.area).or_default().push(outcome);
        }

        let mut out = String::new();
        let _ = writeln!(out, "\nPostgreSQL protocol conformance");
        let _ = writeln!(out, "================================");

        for (area, outcomes) in &by_area {
            let passed = outcomes.iter().filter(|o| o.passed).count();
            let _ = writeln!(out, "\n{} — {}/{}", area.name(), passed, outcomes.len());
            for outcome in outcomes {
                let mark = if outcome.passed { "PASS" } else { "FAIL" };
                let _ = writeln!(out, "  [{mark}] {}", outcome.name);
                if !outcome.passed {
                    let detail = outcome.detail.replace('\n', " ");
                    let detail = detail.chars().take(300).collect::<String>();
                    let _ = writeln!(out, "         {detail}");
                }
            }
        }

        let total = self.outcomes.len();
        let passed = self.passed();
        let _ = writeln!(
            out,
            "\nTOTAL {passed}/{total} ({:.0}%)",
            (passed as f64 / total as f64) * 100.0
        );
        out
    }
}

/// Render an error with its causes.
///
/// `tokio_postgres::Error` renders as the useless "db error"; the server's
/// message is one level down. A conformance report whose failures all read
/// "db error" cannot be acted on.
fn describe<E: std::error::Error>(error: E) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

async fn connect() -> Result<Client, String> {
    let mut config = tokio_postgres::Config::new();
    config
        .host(HOST)
        .port(PORT)
        .user(USER)
        .password(USER)
        .connect_timeout(std::time::Duration::from_secs(5));

    let (client, connection) = config
        .connect(NoTls)
        .await
        .map_err(|e| format!("connect: {}", describe(e)))?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// Connect and expose the connection's asynchronous notifications.
///
/// `tokio_postgres` surfaces NotificationResponse only through the polled
/// connection object, so the connection task forwards payloads on a channel.
async fn connect_with_notifications(
) -> Result<(Client, tokio::sync::mpsc::UnboundedReceiver<String>), String> {
    use futures_util::{future, stream, StreamExt};

    let mut config = tokio_postgres::Config::new();
    config
        .host(HOST)
        .port(PORT)
        .user(USER)
        .password(USER)
        .connect_timeout(std::time::Duration::from_secs(5));

    let (client, mut connection) = config
        .connect(NoTls)
        .await
        .map_err(|e| format!("connect: {}", describe(e)))?;

    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let stream = stream::poll_fn(move |cx| connection.poll_message(cx));
    tokio::spawn(stream.for_each(move |message| {
        if let Ok(tokio_postgres::AsyncMessage::Notification(notification)) = message {
            let _ = sender.send(notification.payload().to_string());
        }
        future::ready(())
    }));

    Ok((client, receiver))
}

/// Best-effort cleanup that must not mask the failure under test.
async fn drop_table(client: &Client, table: &str) {
    let _ = client
        .simple_query(&format!("DROP TABLE IF EXISTS {table}"))
        .await;
}

#[tokio::test]
#[ignore = "requires a running orbit-server on 5432"]
async fn postgres_protocol_conformance() {
    let mut report = Report::default();

    let mut client = match connect().await {
        Ok(client) => {
            report.record(Area::Connection, "connect with SCRAM auth", Ok(()));
            client
        }
        Err(e) => {
            report.record(Area::Connection, "connect with SCRAM auth", Err(e.clone()));
            println!("{}", report.render());
            panic!("cannot reach the server under test: {e}");
        }
    };

    // ---------------------------------------------------------------- simple
    report.record(
        Area::SimpleQuery,
        "SELECT via simple query returns a row",
        client
            .simple_query("SELECT 1")
            .await
            .map_err(describe)
            .and_then(|messages| {
                let rows = messages
                    .iter()
                    .filter(|m| matches!(m, tokio_postgres::SimpleQueryMessage::Row(_)))
                    .count();
                (rows == 1).then_some(()).ok_or(format!("{rows} rows"))
            }),
    );

    report.record(
        Area::SimpleQuery,
        "multiple statements in one simple query",
        client
            .simple_query("SELECT 1; SELECT 2")
            .await
            .map(|_| ())
            .map_err(describe),
    );

    report.record(
        Area::SimpleQuery,
        "empty query returns EmptyQueryResponse",
        client.simple_query("").await.map(|_| ()).map_err(describe),
    );

    // -------------------------------------------------------------- extended
    report.record(
        Area::ExtendedQuery,
        "prepare() succeeds (Describe answers)",
        client.prepare("SELECT 1").await.map(|_| ()).map_err(describe),
    );

    report.record(
        Area::ExtendedQuery,
        "prepared statement reports its column types",
        client
            .prepare("SELECT 1")
            .await
            .map_err(describe)
            .and_then(|stmt| {
                (!stmt.columns().is_empty())
                    .then_some(())
                    .ok_or_else(|| "no columns described".to_string())
            }),
    );

    drop_table(&client, "conf_basic").await;
    let setup = client
        .simple_query("CREATE TABLE conf_basic (id INTEGER, name TEXT)")
        .await
        .map(|_| ())
        .map_err(describe);
    report.record(Area::SimpleQuery, "CREATE TABLE", setup.clone());

    if let Err(e) = &setup {
        // A check that silently disappears reads as a pass. Anything that
        // cannot run because its setup failed is recorded as a failure.
        for name in [
            "INSERT reports affected rows",
            "UPDATE reports affected rows",
            "DELETE reports affected rows",
        ] {
            report.record(
                Area::SimpleQuery,
                name,
                Err(format!("not run: table setup failed: {e}")),
            );
        }
        for name in [
            "text value round-trips with case intact",
            "integer column decodes as int4",
        ] {
            report.record(Area::Types, name, Err("not run: table setup failed".into()));
        }
        report.record(
            Area::ExtendedQuery,
            "parameterised query filters rows",
            Err("not run: table setup failed".into()),
        );
    }

    if setup.is_ok() {
        report.record(
            Area::SimpleQuery,
            "INSERT reports affected rows",
            client
                .execute("INSERT INTO conf_basic (id, name) VALUES (1, 'one')", &[])
                .await
                .map_err(describe)
                .and_then(|n| (n == 1).then_some(()).ok_or(format!("affected {n}"))),
        );

        report.record(
            Area::ExtendedQuery,
            "parameterised query filters rows",
            client
                .query("SELECT name FROM conf_basic WHERE id = $1", &[&1i32])
                .await
                .map_err(describe)
                .and_then(|rows| {
                    (rows.len() == 1)
                        .then_some(())
                        .ok_or(format!("{} rows", rows.len()))
                }),
        );

        report.record(
            Area::Types,
            "text value round-trips with case intact",
            client
                .query("SELECT name FROM conf_basic WHERE id = 1", &[])
                .await
                .map_err(describe)
                .and_then(|rows| {
                    let value: Result<&str, _> = rows[0].try_get(0);
                    match value {
                        Ok("one") => Ok(()),
                        Ok(other) => Err(format!("got {other:?}")),
                        Err(e) => Err(e.to_string()),
                    }
                }),
        );

        report.record(
            Area::Types,
            "integer column decodes as int4",
            client
                .query("SELECT id FROM conf_basic WHERE id = 1", &[])
                .await
                .map_err(describe)
                .and_then(|rows| {
                    rows[0]
                        .try_get::<_, i32>(0)
                        .map(|_| ())
                        .map_err(describe)
                }),
        );

        report.record(
            Area::SimpleQuery,
            "UPDATE reports affected rows",
            client
                .execute("UPDATE conf_basic SET name = 'two' WHERE id = 1", &[])
                .await
                .map_err(describe)
                .and_then(|n| (n == 1).then_some(()).ok_or(format!("affected {n}"))),
        );

        report.record(
            Area::SimpleQuery,
            "DELETE reports affected rows",
            client
                .execute("DELETE FROM conf_basic WHERE id = 1", &[])
                .await
                .map_err(describe)
                .and_then(|n| (n == 1).then_some(()).ok_or(format!("affected {n}"))),
        );
    }

    // --------------------------------------------------------------- portals
    // A portal fetched in pages is how every driver implements a cursor with a
    // fetch size.
    drop_table(&client, "conf_portal").await;
    let portal_setup = async {
        client
            .simple_query("CREATE TABLE conf_portal (id INTEGER)")
            .await
            .map_err(describe)?;
        for i in 1..=5 {
            client
                .execute(&format!("INSERT INTO conf_portal (id) VALUES ({i})"), &[])
                .await
                .map_err(describe)?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(e) = &portal_setup {
        report.record(
            Area::Portals,
            "portal returns only the requested number of rows",
            Err(format!("not run: setup failed: {e}")),
        );
    }

    if portal_setup.is_ok() {
        report.record(
            Area::Portals,
            "portal returns only the requested number of rows",
            async {
                let transaction = client
                    .transaction()
                    .await
                    .map_err(|e| format!("begin: {}", describe(e)))?;
                let statement = transaction
                    .prepare("SELECT id FROM conf_portal")
                    .await
                    .map_err(|e| format!("prepare: {}", describe(e)))?;
                let portal = transaction
                    .bind(&statement, &[])
                    .await
                    .map_err(|e| format!("bind: {}", describe(e)))?;
                let first = transaction
                    .query_portal(&portal, 2)
                    .await
                    .map_err(|e| format!("query_portal: {}", describe(e)))?;
                if first.len() != 2 {
                    return Err(format!("asked for 2 rows, received {}", first.len()));
                }
                let second = transaction
                    .query_portal(&portal, 2)
                    .await
                    .map_err(|e| format!("second fetch: {}", describe(e)))?;
                if second.len() != 2 {
                    return Err(format!("second page returned {}", second.len()));
                }
                Ok(())
            }
            .await,
        );
    }

    // ---------------------------------------------------------- transactions
    report.record(
        Area::Transactions,
        "BEGIN / COMMIT round trip",
        client
            .simple_query("BEGIN; COMMIT")
            .await
            .map(|_| ())
            .map_err(describe),
    );

    report.record(
        Area::Transactions,
        "ROLLBACK discards an uncommitted write",
        async {
            drop_table(&client, "conf_tx").await;
            client
                .simple_query("CREATE TABLE conf_tx (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("BEGIN")
                .await
                .map_err(|e| format!("BEGIN: {}", describe(e)))?;
            client
                .simple_query("INSERT INTO conf_tx (id) VALUES (1)")
                .await
                .map_err(|e| format!("INSERT: {}", describe(e)))?;
            client
                .simple_query("ROLLBACK")
                .await
                .map_err(|e| format!("ROLLBACK: {}", describe(e)))?;
            let rows = client
                .query("SELECT id FROM conf_tx", &[])
                .await
                .map_err(describe)?;
            rows.is_empty()
                .then_some(())
                .ok_or_else(|| format!("{} row(s) survived rollback", rows.len()))
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "driver-managed transaction commits",
        async {
            drop_table(&client, "conf_tx2").await;
            client
                .simple_query("CREATE TABLE conf_tx2 (id INTEGER)")
                .await
                .map_err(describe)?;
            let transaction = client.transaction().await.map_err(describe)?;
            transaction
                .execute("INSERT INTO conf_tx2 (id) VALUES (1)", &[])
                .await
                .map_err(describe)?;
            transaction.commit().await.map_err(describe)?;
            let rows = client
                .query("SELECT id FROM conf_tx2", &[])
                .await
                .map_err(describe)?;
            (rows.len() == 1)
                .then_some(())
                .ok_or_else(|| format!("{} rows after commit", rows.len()))
        }
        .await,
    );

    // --------------------------------------------------------------- catalog
    for (name, sql) in [
        ("SELECT version()", "SELECT version()"),
        ("SELECT current_database()", "SELECT current_database()"),
        (
            "pg_catalog.pg_class is queryable",
            "SELECT relname FROM pg_catalog.pg_class LIMIT 1",
        ),
        (
            "information_schema.tables is queryable",
            "SELECT table_name FROM information_schema.tables LIMIT 1",
        ),
        (
            "pg_catalog.pg_type is queryable (driver type lookup)",
            "SELECT typname FROM pg_catalog.pg_type LIMIT 1",
        ),
    ] {
        report.record(
            Area::Catalog,
            name,
            client
                .simple_query(sql)
                .await
                .map(|_| ())
                .map_err(describe),
        );
    }

    // ------------------------------------------------------------------ COPY
    drop_table(&client, "conf_copy_in").await;
    let _ = client
        .simple_query("CREATE TABLE conf_copy_in (id INTEGER)")
        .await;

    // Accepting the statement proves nothing: an engine that treats COPY as an
    // unknown no-op also returns success. These drive the actual subprotocol.
    report.record(
        Area::Copy,
        "COPY TO STDOUT streams rows",
        async {
            let stream = client
                .copy_out("COPY conf_portal TO STDOUT")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(stream);
            let mut bytes = 0usize;
            while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
                bytes += chunk.map_err(describe)?.len();
            }
            (bytes > 0)
                .then_some(())
                .ok_or_else(|| "COPY TO produced no data".to_string())
        }
        .await,
    );

    report.record(
        Area::Copy,
        "COPY FROM STDIN ingests rows",
        async {
            let sink = client
                .copy_in("COPY conf_copy_in FROM STDIN")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(sink);
            use futures_util::SinkExt as _;
            sink.as_mut()
                .send(bytes::Bytes::from_static(b"9\n10\n"))
                .await
                .map_err(describe)?;
            let written = sink.finish().await.map_err(describe)?;
            (written == 2)
                .then_some(())
                .ok_or_else(|| format!("COPY FROM reported {written} rows, expected 2"))
        }
        .await,
    );

    // -------------------------------------------------------------- NOTIFY
    // A server that ignores LISTEN also answers it without error, so the check
    // is whether a notification actually arrives.
    report.record(
        Area::Notify,
        "a NOTIFY reaches a listening session",
        async {
            let (notify_client, mut notify_stream) =
                connect_with_notifications().await.map_err(|e| e)?;

            notify_client
                .simple_query("LISTEN conf_channel")
                .await
                .map_err(describe)?;
            notify_client
                .simple_query("NOTIFY conf_channel, 'hello'")
                .await
                .map_err(describe)?;

            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                notify_stream.recv(),
            )
            .await
            {
                Ok(Some(payload)) if payload == "hello" => Ok(()),
                Ok(Some(other)) => Err(format!("unexpected payload {other:?}")),
                Ok(None) => Err("notification channel closed".to_string()),
                Err(_) => Err("no notification delivered within 2s".to_string()),
            }
        }
        .await,
    );

    // ---------------------------------------------------------------- errors
    report.record(
        Area::Errors,
        "unknown table is an error, not a silent empty result",
        match client.query("SELECT * FROM definitely_not_a_table", &[]).await {
            Ok(_) => Err("query on a missing table succeeded".to_string()),
            Err(_) => Ok(()),
        },
    );

    report.record(
        Area::Errors,
        "error carries a SQLSTATE code",
        match client.query("SELECT * FROM definitely_not_a_table", &[]).await {
            Ok(_) => Err("expected an error".to_string()),
            Err(e) => e
                .code()
                .map(|_| ())
                .ok_or_else(|| "error has no SQLSTATE".to_string()),
        },
    );

    report.record(
        Area::Errors,
        "session is usable after a failed statement",
        client
            .simple_query("SELECT 1")
            .await
            .map(|_| ())
            .map_err(describe),
    );

    for table in [
        "conf_basic",
        "conf_portal",
        "conf_tx",
        "conf_tx2",
        "conf_copy_in",
    ] {
        drop_table(&client, table).await;
    }

    println!("{}", report.render());

    // The harness reports; it does not gate. Conformance is tracked as a number
    // that should move up, and failing the build on a known gap would only make
    // the number invisible.
    assert!(
        report.passed() > 0,
        "no conformance checks passed at all — the server is not usable"
    );
}
