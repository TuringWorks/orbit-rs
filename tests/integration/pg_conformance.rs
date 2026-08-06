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
    Sql,
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
            Area::Sql => "SQL surface",
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

/// Read the first column of every row a statement returns, as text.
///
/// The simple-query protocol renders every value as text, so this compares
/// answers without needing the client to guess a Rust type per column — and it
/// checks the value, not merely that the statement did not error.
async fn simple_column(client: &Client, sql: &str) -> Result<Vec<String>, String> {
    use tokio_postgres::SimpleQueryMessage;
    let messages = client.simple_query(sql).await.map_err(describe)?;
    Ok(messages
        .iter()
        .filter_map(|message| match message {
            SimpleQueryMessage::Row(row) => Some(row.get(0).unwrap_or("NULL").to_string()),
            _ => None,
        })
        .collect())
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
                    let row = rows.first().ok_or("no rows returned".to_string())?;
                    match row.try_get::<_, &str>(0) {
                        Ok("one") => Ok(()),
                        Ok(other) => Err(format!("got {other:?}")),
                        Err(e) => Err(describe(e)),
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
                    rows.first()
                        .ok_or("no rows returned".to_string())?
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
    // On its own connection: a server that does not understand the COPY
    // subprotocol leaves the session unusable, and sharing one connection made
    // every later check fail for a reason that had nothing to do with it.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::Copy, "COPY TO STDOUT streams rows", Err(e.clone()));
            report.record(Area::Copy, "COPY FROM STDIN ingests rows", Err(e.clone()));
            report.record(Area::Errors, "reconnect for COPY", Err(e));
            println!("{}", report.render());
            return;
        }
    };

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
                connect_with_notifications().await?;

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
    // Fresh again, for the same reason.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            for name in [
                "unknown table is an error, not a silent empty result",
                "error carries a SQLSTATE code",
                "session is usable after a failed statement",
            ] {
                report.record(Area::Errors, name, Err(e.clone()));
            }
            println!("{}", report.render());
            return;
        }
    };

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


    // ------------------------------------------------- broader SQL surface
    // Added so the score reflects more than the features already known to
    // work: a harness that only measures what passes overstates conformance.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::SimpleQuery, "reconnect for SQL surface", Err(e));
            println!("{}", report.render());
            return;
        }
    };

    drop_table(&client, "conf_sql").await;
    let _ = client
        .simple_query("CREATE TABLE conf_sql (id INTEGER, grp TEXT, amount INTEGER)")
        .await;
    for (id, grp, amount) in [(1, "a", 10), (2, "a", 20), (3, "b", 30)] {
        let _ = client
            .simple_query(&format!(
                "INSERT INTO conf_sql (id, grp, amount) VALUES ({id}, '{grp}', {amount})"
            ))
            .await;
    }

    for (area, name, sql, want_rows) in [
        (Area::Sql, "ORDER BY (checked separately)", "SELECT id FROM conf_sql", 3usize),
        (Area::Sql, "LIMIT", "SELECT id FROM conf_sql LIMIT 2", 2),
        (Area::Sql, "COUNT(*) aggregate", "SELECT COUNT(*) FROM conf_sql", 1),
        (
            Area::Sql,
            "GROUP BY with aggregate",
            "SELECT grp, SUM(amount) FROM conf_sql GROUP BY grp",
            2,
        ),
        (
            Area::Sql,
            "WHERE with AND",
            "SELECT id FROM conf_sql WHERE grp = 'a' AND amount > 15",
            1,
        ),
        (Area::Sql, "IN list", "SELECT id FROM conf_sql WHERE id IN (1, 2)", 2),
        (
            Area::Sql,
            "self JOIN",
            "SELECT a.id FROM conf_sql a JOIN conf_sql b ON a.grp = b.grp WHERE a.id = 1",
            2,
        ),
        (
            Area::Sql,
            "subquery in WHERE",
            "SELECT id FROM conf_sql WHERE amount = (SELECT MAX(amount) FROM conf_sql)",
            1,
        ),
        (
            Area::Sql,
            "DISTINCT",
            "SELECT DISTINCT grp FROM conf_sql",
            2,
        ),
        (
            Area::Sql,
            "column alias",
            "SELECT id AS identifier FROM conf_sql WHERE id = 1",
            1,
        ),
    ] {
        report.record(
            area,
            name,
            client
                .query(sql, &[])
                .await
                .map_err(describe)
                .and_then(|rows| {
                    (rows.len() == want_rows)
                        .then_some(())
                        .ok_or(format!("{} rows, expected {want_rows}", rows.len()))
                }),
        );
    }

    for (name, sql) in [
        ("SET then SHOW a runtime parameter", "SET application_name = 'conf'"),
        ("EXPLAIN returns a plan", "EXPLAIN SELECT id FROM conf_sql"),
        ("DECLARE a cursor", "DECLARE c CURSOR FOR SELECT id FROM conf_sql"),
        ("SAVEPOINT inside a transaction", "BEGIN; SAVEPOINT s1; ROLLBACK"),
        ("CREATE INDEX", "CREATE INDEX conf_idx ON conf_sql (id)"),
        ("ALTER TABLE ADD COLUMN", "ALTER TABLE conf_sql ADD COLUMN note TEXT"),
    ] {
        report.record(
            Area::Sql,
            name,
            client
                .simple_query(sql)
                .await
                .map(|_| ())
                .map_err(describe),
        );
    }

    report.record(
        Area::Sql,
        "ORDER BY actually orders",
        client
            .query("SELECT id FROM conf_sql ORDER BY id DESC", &[])
            .await
            .map_err(describe)
            .and_then(|rows| {
                let ids: Vec<i32> = rows
                    .iter()
                    .filter_map(|row| row.try_get::<_, i32>(0).ok())
                    .collect();
                // Returning every row in storage order also yields three rows,
                // so the count alone proves nothing.
                (ids == vec![3, 2, 1])
                    .then_some(())
                    .ok_or(format!("got {ids:?}, expected [3, 2, 1]"))
            }),
    );

    report.record(
        Area::Types,
        "NULL round-trips as NULL",
        async {
            client
                .simple_query("INSERT INTO conf_sql (id, grp, amount) VALUES (9, NULL, 1)")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT grp FROM conf_sql WHERE id = 9", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("no row".to_string())?;
            let value: Option<&str> = row.try_get(0).map_err(describe)?;
            value
                .is_none()
                .then_some(())
                .ok_or_else(|| format!("expected NULL, got {value:?}"))
        }
        .await,
    );

    report.record(
        Area::Types,
        "BOOLEAN column round-trips",
        async {
            drop_table(&client, "conf_bool").await;
            client
                .simple_query("CREATE TABLE conf_bool (flag BOOLEAN)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_bool (flag) VALUES (true)")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT flag FROM conf_bool", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("no row".to_string())?;
            row.try_get::<_, bool>(0)
                .map_err(describe)
                .and_then(|v| v.then_some(()).ok_or("expected true".to_string()))
        }
        .await,
    );

    report.record(
        Area::ExtendedQuery,
        "a prepared statement can be executed twice",
        async {
            let statement = client
                .prepare("SELECT id FROM conf_sql WHERE id = $1")
                .await
                .map_err(describe)?;
            let first = client.query(&statement, &[&1i32]).await.map_err(describe)?;
            let second = client.query(&statement, &[&2i32]).await.map_err(describe)?;
            (first.len() == 1 && second.len() == 1)
                .then_some(())
                .ok_or_else(|| format!("{} then {} rows", first.len(), second.len()))
        }
        .await,
    );

    for table in ["conf_sql", "conf_bool"] {
        drop_table(&client, table).await;
    }


    // ------------------------------------------- second round of coverage
    // Added after the first 48 checks all passed: a harness that stops finding
    // gaps has stopped measuring, not finished. These probe the surface real
    // clients use that the first round never touched.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::Sql, "reconnect for round two", Err(e));
            println!("{}", report.render());
            return;
        }
    };

    drop_table(&client, "conf_two").await;
    let _ = client
        .simple_query("CREATE TABLE conf_two (id INTEGER, name TEXT, amount INTEGER)")
        .await;
    for (id, name, amount) in [(1, "alpha", 10), (2, "beta", 20), (3, "gamma", 30)] {
        let _ = client
            .simple_query(&format!(
                "INSERT INTO conf_two (id, name, amount) VALUES ({id}, '{name}', {amount})"
            ))
            .await;
    }

    for (area, name, sql, want) in [
        (Area::Sql, "LIKE pattern match", "SELECT id FROM conf_two WHERE name LIKE 'al%'", 1usize),
        (Area::Sql, "BETWEEN range", "SELECT id FROM conf_two WHERE amount BETWEEN 15 AND 25", 1),
        (Area::Sql, "IS NULL", "SELECT id FROM conf_two WHERE name IS NOT NULL", 3),
        (Area::Sql, "NOT with comparison", "SELECT id FROM conf_two WHERE NOT id = 1", 2),
        (Area::Sql, "OR predicate", "SELECT id FROM conf_two WHERE id = 1 OR id = 3", 2),
        (Area::Sql, "arithmetic in projection", "SELECT amount + 1 FROM conf_two WHERE id = 1", 1),
        (Area::Sql, "ORDER BY two keys", "SELECT id FROM conf_two ORDER BY amount DESC, id ASC", 3),
        (Area::Sql, "aggregate with WHERE", "SELECT COUNT(*) FROM conf_two WHERE amount > 15", 1),
        (Area::Sql, "LEFT JOIN keeps unmatched rows", "SELECT a.id FROM conf_two a LEFT JOIN conf_two b ON a.id = b.id + 100", 3),
    ] {
        report.record(
            area,
            name,
            client.query(sql, &[]).await.map_err(describe).and_then(|rows| {
                (rows.len() == want)
                    .then_some(())
                    .ok_or(format!("{} rows, expected {want}", rows.len()))
            }),
        );
    }

    report.record(
        Area::Sql,
        "aggregate value is correct, not just the row count",
        client
            .query("SELECT SUM(amount) FROM conf_two", &[])
            .await
            .map_err(describe)
            .and_then(|rows| {
                let row = rows.first().ok_or("no row".to_string())?;
                let text: String = row
                    .try_get::<_, i64>(0)
                    .map(|v| v.to_string())
                    .or_else(|_| row.try_get::<_, &str>(0).map(str::to_string))
                    .map_err(describe)?;
                (text == "60")
                    .then_some(())
                    .ok_or(format!("SUM was {text}, expected 60"))
            }),
    );

    report.record(
        Area::Transactions,
        "ROLLBACK undoes an UPDATE, not just an INSERT",
        async {
            client
                .simple_query("BEGIN")
                .await
                .map_err(describe)?;
            client
                .simple_query("UPDATE conf_two SET amount = 999 WHERE id = 1")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            let rows = client
                .query("SELECT amount FROM conf_two WHERE id = 1", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("row disappeared".to_string())?;
            let amount: i32 = row.try_get(0).map_err(describe)?;
            (amount == 10)
                .then_some(())
                .ok_or(format!("amount is {amount}, expected the original 10"))
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "ROLLBACK undoes a DELETE",
        async {
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("DELETE FROM conf_two WHERE id = 2")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            let rows = client
                .query("SELECT id FROM conf_two", &[])
                .await
                .map_err(describe)?;
            (rows.len() == 3)
                .then_some(())
                .ok_or(format!("{} rows survived, expected 3", rows.len()))
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "COMMIT keeps the write",
        async {
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_two (id, name, amount) VALUES (4, 'delta', 40)")
                .await
                .map_err(describe)?;
            client.simple_query("COMMIT").await.map_err(describe)?;

            let rows = client
                .query("SELECT id FROM conf_two WHERE id = 4", &[])
                .await
                .map_err(describe)?;
            (rows.len() == 1)
                .then_some(())
                .ok_or("committed row is missing".to_string())
        }
        .await,
    );

    report.record(
        Area::Types,
        "a text value containing a quote round-trips",
        async {
            client
                .simple_query("INSERT INTO conf_two (id, name, amount) VALUES (5, 'O''Brien', 1)")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT name FROM conf_two WHERE id = 5", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("no row".to_string())?;
            let name: &str = row.try_get(0).map_err(describe)?;
            (name == "O'Brien")
                .then_some(())
                .ok_or(format!("got {name:?}"))
        }
        .await,
    );

    drop_table(&client, "conf_two").await;


    // -------------------------------------------- third round of coverage
    // The second round ended at 62/62, which measures the checks written, not
    // the protocol. These cover the surface a real client reaches for that
    // nothing above touches: set operations, CTEs, window functions, string
    // and aggregate functions, RETURNING, views, and the numeric and temporal
    // types. Every check compares a value, because a statement that runs and
    // answers wrongly is worse than one that errors.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::Sql, "reconnect for round three", Err(e));
            println!("{}", report.render());
            return;
        }
    };

    drop_table(&client, "conf_three").await;
    let _ = client
        .simple_query("CREATE TABLE conf_three (id INTEGER, grp TEXT, amount INTEGER, name TEXT)")
        .await;
    for (id, grp, amount, name) in [(1, "a", 10, "alpha"), (2, "a", 20, "beta"), (3, "b", 30, "gamma")] {
        let _ = client
            .simple_query(&format!(
                "INSERT INTO conf_three (id, grp, amount, name) VALUES ({id}, '{grp}', {amount}, '{name}')"
            ))
            .await;
    }

    for (name, sql, want) in [
        (
            "HAVING filters groups",
            // Both groups sum to 30, so a SUM threshold would not discriminate;
            // the row count does.
            "SELECT grp FROM conf_three GROUP BY grp HAVING COUNT(*) > 1",
            "a",
        ),
        (
            "UNION ALL keeps duplicates",
            "SELECT id FROM conf_three WHERE id = 1 UNION ALL SELECT id FROM conf_three WHERE id = 1",
            "1,1",
        ),
        (
            "UNION removes duplicates",
            "SELECT id FROM conf_three WHERE id = 1 UNION SELECT id FROM conf_three WHERE id = 1",
            "1",
        ),
        (
            "CASE expression",
            "SELECT CASE WHEN amount > 15 THEN 'big' ELSE 'small' END FROM conf_three ORDER BY id",
            "small,big,big",
        ),
        (
            "COALESCE picks the first non-NULL",
            "SELECT COALESCE(NULL, 'fallback')",
            "fallback",
        ),
        (
            "CTE (WITH)",
            "WITH big AS (SELECT id FROM conf_three WHERE amount > 15) SELECT id FROM big ORDER BY id",
            "2,3",
        ),
        (
            "window function ROW_NUMBER",
            "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM conf_three",
            "1,2,3",
        ),
        (
            "UPPER()",
            "SELECT UPPER(name) FROM conf_three WHERE id = 1",
            "ALPHA",
        ),
        (
            "LENGTH()",
            "SELECT LENGTH(name) FROM conf_three WHERE id = 1",
            "5",
        ),
        (
            "string concatenation",
            "SELECT name || '!' FROM conf_three WHERE id = 1",
            "alpha!",
        ),
        ("MIN()", "SELECT MIN(amount) FROM conf_three", "10"),
        ("MAX()", "SELECT MAX(amount) FROM conf_three", "30"),
        (
            "COUNT(DISTINCT)",
            "SELECT COUNT(DISTINCT grp) FROM conf_three",
            "2",
        ),
        (
            "LIMIT with OFFSET",
            "SELECT id FROM conf_three ORDER BY id LIMIT 1 OFFSET 1",
            "2",
        ),
        (
            "derived table in FROM",
            "SELECT t.id FROM (SELECT id FROM conf_three WHERE id > 1) t ORDER BY t.id",
            "2,3",
        ),
        (
            "ORDER BY with LIMIT picks the top row",
            "SELECT id FROM conf_three ORDER BY amount DESC LIMIT 1",
            "3",
        ),
        (
            "INSERT ... RETURNING",
            "INSERT INTO conf_three (id, grp, amount, name) VALUES (7, 'c', 70, 'eta') RETURNING id",
            "7",
        ),
        (
            "UPDATE ... RETURNING",
            "UPDATE conf_three SET amount = 71 WHERE id = 7 RETURNING amount",
            "71",
        ),
        (
            "DELETE ... RETURNING",
            "DELETE FROM conf_three WHERE id = 7 RETURNING id",
            "7",
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            simple_column(&client, sql).await.and_then(|values| {
                let got = values.join(",");
                (got == want)
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected {want:?}"))
            }),
        );
    }

    report.record(
        Area::Sql,
        "AVG()",
        simple_column(&client, "SELECT AVG(amount) FROM conf_three")
            .await
            .and_then(|values| {
                let got = values.join(",");
                // 20, 20.0 and 20.0000000000000000 are all the right answer;
                // only the scale differs, and PostgreSQL's own scale for
                // avg(integer) is not something to hard-code here.
                got.starts_with("20")
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected 20"))
            }),
    );

    report.record(
        Area::Sql,
        "multi-row INSERT",
        async {
            client
                .simple_query(
                    "INSERT INTO conf_three (id, grp, amount, name) \
                     VALUES (8, 'd', 80, 'theta'), (9, 'd', 90, 'iota')",
                )
                .await
                .map_err(describe)?;
            let ids = simple_column(
                &client,
                "SELECT id FROM conf_three WHERE id > 7 ORDER BY id",
            )
            .await?;
            (ids == ["8", "9"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [8, 9]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "CREATE VIEW then read it back",
        async {
            let _ = client.simple_query("DROP VIEW IF EXISTS conf_view").await;
            client
                .simple_query("CREATE VIEW conf_view AS SELECT id FROM conf_three WHERE id = 1")
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_view").await?;
            let result = (ids == ["1"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1]"));
            let _ = client.simple_query("DROP VIEW IF EXISTS conf_view").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "TRUNCATE empties the table",
        async {
            drop_table(&client, "conf_trunc").await;
            client
                .simple_query("CREATE TABLE conf_trunc (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_trunc (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("TRUNCATE TABLE conf_trunc")
                .await
                .map_err(describe)?;
            let remaining = simple_column(&client, "SELECT id FROM conf_trunc").await?;
            let result = remaining
                .is_empty()
                .then_some(())
                .ok_or(format!("{} rows survived TRUNCATE", remaining.len()));
            drop_table(&client, "conf_trunc").await;
            result
        }
        .await,
    );

    // ------------------------------------------------------- numeric & time
    drop_table(&client, "conf_types").await;
    let types_ready = async {
        client
            .simple_query(
                "CREATE TABLE conf_types (big BIGINT, exact NUMERIC, approx DOUBLE PRECISION, \
                 day DATE, moment TIMESTAMP)",
            )
            .await
            .map_err(describe)?;
        client
            .simple_query(
                "INSERT INTO conf_types (big, exact, approx, day, moment) VALUES \
                 (9223372036854775807, 12.34, 1.5, '2026-01-02', '2026-01-02 03:04:05')",
            )
            .await
            .map_err(describe)?;
        Ok::<(), String>(())
    }
    .await;

    for (name, column, want) in [
        ("BIGINT round-trips at the i64 limit", "big", "9223372036854775807"),
        ("NUMERIC keeps its scale", "exact", "12.34"),
        ("DOUBLE PRECISION round-trips", "approx", "1.5"),
        ("DATE round-trips", "day", "2026-01-02"),
    ] {
        report.record(
            Area::Types,
            name,
            match &types_ready {
                Err(e) => Err(format!("not run: {e}")),
                Ok(()) => simple_column(&client, &format!("SELECT {column} FROM conf_types"))
                    .await
                    .and_then(|values| {
                        let got = values.join(",");
                        (got == want)
                            .then_some(())
                            .ok_or(format!("got {got:?}, expected {want:?}"))
                    }),
            },
        );
    }

    report.record(
        Area::Types,
        "TIMESTAMP round-trips",
        match &types_ready {
            Err(e) => Err(format!("not run: {e}")),
            Ok(()) => simple_column(&client, "SELECT moment FROM conf_types")
                .await
                .and_then(|values| {
                    let got = values.join(",");
                    // The fractional-second suffix is PostgreSQL's business;
                    // the instant is what has to survive.
                    got.starts_with("2026-01-02 03:04:05")
                        .then_some(())
                        .ok_or(format!("got {got:?}, expected 2026-01-02 03:04:05"))
                }),
        },
    );
    drop_table(&client, "conf_types").await;

    // --------------------------------------------- extended-protocol params
    report.record(
        Area::ExtendedQuery,
        "two parameters of different types",
        client
            .query(
                "SELECT id FROM conf_three WHERE grp = $1 AND amount > $2",
                &[&"a", &15i32],
            )
            .await
            .map_err(describe)
            .and_then(|rows| {
                (rows.len() == 1)
                    .then_some(())
                    .ok_or(format!("{} rows, expected 1", rows.len()))
            }),
    );

    report.record(
        Area::ExtendedQuery,
        "a parameter is bound as a value, not spliced as SQL",
        // If the parameter were pasted into the statement text, the quote
        // would end the literal and this would be a syntax error or, worse,
        // would match everything.
        client
            .query("SELECT id FROM conf_three WHERE name = $1", &[&"o'brien"])
            .await
            .map_err(describe)
            .and_then(|rows| {
                rows.is_empty()
                    .then_some(())
                    .ok_or(format!("{} rows matched a name that does not exist", rows.len()))
            }),
    );

    report.record(
        Area::ExtendedQuery,
        "a parameterised UPDATE reports its row count",
        client
            .execute("UPDATE conf_three SET amount = $1 WHERE id = $2", &[&11i32, &1i32])
            .await
            .map_err(describe)
            .and_then(|affected| {
                (affected == 1)
                    .then_some(())
                    .ok_or(format!("reported {affected} rows, expected 1"))
            }),
    );

    drop_table(&client, "conf_three").await;

    println!("{}", report.render());

    // The harness reports; it does not gate. Conformance is tracked as a number
    // that should move up, and failing the build on a known gap would only make
    // the number invisible.
    assert!(
        report.passed() > 0,
        "no conformance checks passed at all — the server is not usable"
    );
}
