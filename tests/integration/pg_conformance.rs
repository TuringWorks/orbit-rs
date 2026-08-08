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
/// The SQLSTATE a failure carried, or `-` when it carried none.
///
/// This is the thing a driver branches on, so it is checked directly rather
/// than through the message.
fn sqlstate(error: &tokio_postgres::Error) -> String {
    error
        .code()
        .map_or_else(|| "-".to_string(), |code| code.code().to_string())
}

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
        client
            .prepare("SELECT 1")
            .await
            .map(|_| ())
            .map_err(describe),
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
            client.simple_query(sql).await.map(|_| ()).map_err(describe),
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
        "COPY ... WITH CSV round-trips, quoting included",
        async {
            drop_table(&client, "conf_csv").await;
            client
                .simple_query("CREATE TABLE conf_csv (id INTEGER, name TEXT)")
                .await
                .map_err(describe)?;

            // The CSV option was parsed by nothing: output came back
            // tab-separated and CSV input arrived as a single field.
            let sink = client
                .copy_in("COPY conf_csv FROM STDIN WITH CSV")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(sink);
            use futures_util::SinkExt as _;
            // A comma inside quotes, a doubled quote, and an empty field for
            // NULL — the three things separating CSV from "split on commas".
            sink.as_mut()
                .send(bytes::Bytes::from_static(
                    b"1,ada\n2,\"a,b\"\n3,\"say \"\"hi\"\"\"\n4,\n",
                ))
                .await
                .map_err(describe)?;
            let written = sink.finish().await.map_err(describe)?;
            if written != 4 {
                return Err(format!("COPY FROM CSV reported {written} rows, expected 4"));
            }
            let names = simple_column(&client, "SELECT name FROM conf_csv ORDER BY id").await?;
            if names != ["ada", "a,b", "say \"hi\"", "NULL"] {
                return Err(format!("stored {names:?}"));
            }

            // And back out, with the same three cases re-quoted.
            let stream = client
                .copy_out("COPY conf_csv TO STDOUT WITH CSV")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(stream);
            let mut text = String::new();
            while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
                text.push_str(&String::from_utf8_lossy(&chunk.map_err(describe)?));
            }
            let lines: Vec<&str> = text.lines().collect();
            let result = (lines == ["1,ada", "2,\"a,b\"", "3,\"say \"\"hi\"\"\"", "4,"])
                .then_some(())
                .ok_or(format!("COPY TO CSV wrote {lines:?}"));
            drop_table(&client, "conf_csv").await;
            result
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
            let (notify_client, mut notify_stream) = connect_with_notifications().await?;

            notify_client
                .simple_query("LISTEN conf_channel")
                .await
                .map_err(describe)?;
            notify_client
                .simple_query("NOTIFY conf_channel, 'hello'")
                .await
                .map_err(describe)?;

            match tokio::time::timeout(std::time::Duration::from_secs(2), notify_stream.recv())
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
        match client
            .query("SELECT * FROM definitely_not_a_table", &[])
            .await
        {
            Ok(_) => Err("query on a missing table succeeded".to_string()),
            Err(_) => Ok(()),
        },
    );

    report.record(
        Area::Errors,
        "error carries a SQLSTATE code",
        match client
            .query("SELECT * FROM definitely_not_a_table", &[])
            .await
        {
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
        (
            Area::Sql,
            "ORDER BY (checked separately)",
            "SELECT id FROM conf_sql",
            3usize,
        ),
        (Area::Sql, "LIMIT", "SELECT id FROM conf_sql LIMIT 2", 2),
        (
            Area::Sql,
            "COUNT(*) aggregate",
            "SELECT COUNT(*) FROM conf_sql",
            1,
        ),
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
        (
            Area::Sql,
            "IN list",
            "SELECT id FROM conf_sql WHERE id IN (1, 2)",
            2,
        ),
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
        (
            "SET then SHOW a runtime parameter",
            "SET application_name = 'conf'",
        ),
        ("EXPLAIN returns a plan", "EXPLAIN SELECT id FROM conf_sql"),
        (
            "DECLARE a cursor",
            "DECLARE c CURSOR FOR SELECT id FROM conf_sql",
        ),
        (
            "SAVEPOINT inside a transaction",
            "BEGIN; SAVEPOINT s1; ROLLBACK",
        ),
        ("CREATE INDEX", "CREATE INDEX conf_idx ON conf_sql (id)"),
        (
            "ALTER TABLE ADD COLUMN",
            "ALTER TABLE conf_sql ADD COLUMN note TEXT",
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            client.simple_query(sql).await.map(|_| ()).map_err(describe),
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
        (
            Area::Sql,
            "LIKE pattern match",
            "SELECT id FROM conf_two WHERE name LIKE 'al%'",
            1usize,
        ),
        (
            Area::Sql,
            "BETWEEN range",
            "SELECT id FROM conf_two WHERE amount BETWEEN 15 AND 25",
            1,
        ),
        (
            Area::Sql,
            "IS NULL",
            "SELECT id FROM conf_two WHERE name IS NOT NULL",
            3,
        ),
        (
            Area::Sql,
            "NOT with comparison",
            "SELECT id FROM conf_two WHERE NOT id = 1",
            2,
        ),
        (
            Area::Sql,
            "OR predicate",
            "SELECT id FROM conf_two WHERE id = 1 OR id = 3",
            2,
        ),
        (
            Area::Sql,
            "arithmetic in projection",
            "SELECT amount + 1 FROM conf_two WHERE id = 1",
            1,
        ),
        (
            Area::Sql,
            "ORDER BY two keys",
            "SELECT id FROM conf_two ORDER BY amount DESC, id ASC",
            3,
        ),
        (
            Area::Sql,
            "aggregate with WHERE",
            "SELECT COUNT(*) FROM conf_two WHERE amount > 15",
            1,
        ),
        (
            Area::Sql,
            "LEFT JOIN keeps unmatched rows",
            "SELECT a.id FROM conf_two a LEFT JOIN conf_two b ON a.id = b.id + 100",
            3,
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
            client.simple_query("BEGIN").await.map_err(describe)?;
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
    for (id, grp, amount, name) in [
        (1, "a", 10, "alpha"),
        (2, "a", 20, "beta"),
        (3, "b", 30, "gamma"),
    ] {
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
        (
            "BIGINT round-trips at the i64 limit",
            "big",
            "9223372036854775807",
        ),
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
                rows.is_empty().then_some(()).ok_or(format!(
                    "{} rows matched a name that does not exist",
                    rows.len()
                ))
            }),
    );

    report.record(
        Area::ExtendedQuery,
        "a parameterised UPDATE reports its row count",
        client
            .execute(
                "UPDATE conf_three SET amount = $1 WHERE id = $2",
                &[&11i32, &1i32],
            )
            .await
            .map_err(describe)
            .and_then(|affected| {
                (affected == 1)
                    .then_some(())
                    .ok_or(format!("reported {affected} rows, expected 1"))
            }),
    );

    drop_table(&client, "conf_three").await;

    // ------------------------------------------- fourth round of coverage
    // Round three ended at 93/93. A harness that stops finding gaps has
    // stopped measuring: these probe expressions, casts, constraints,
    // defaults, `INSERT ... SELECT`, upserts and FROM-less selects — the
    // surface an ORM and a migration tool both lean on.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::Sql, "reconnect for round four", Err(e));
            println!("{}", report.render());
            return;
        }
    };

    drop_table(&client, "conf_four").await;
    let _ = client
        .simple_query("CREATE TABLE conf_four (id INTEGER, name TEXT, amount INTEGER)")
        .await;
    for (id, name, amount) in [(1, "alpha", 10), (2, "beta", 20), (3, "gamma", 30)] {
        let _ = client
            .simple_query(&format!(
                "INSERT INTO conf_four (id, name, amount) VALUES ({id}, '{name}', {amount})"
            ))
            .await;
    }

    for (name, sql, want) in [
        // A select with no FROM. Every driver and migration tool issues one.
        ("SELECT without FROM", "SELECT 1", "1"),
        ("arithmetic without FROM", "SELECT 1 + 1", "2"),
        (
            "ILIKE is case-insensitive",
            "SELECT id FROM conf_four WHERE name ILIKE 'AL%'",
            "1",
        ),
        (
            "NOT LIKE",
            "SELECT id FROM conf_four WHERE name NOT LIKE 'a%' ORDER BY id",
            "2,3",
        ),
        (
            "NOT IN list",
            "SELECT id FROM conf_four WHERE id NOT IN (1, 2)",
            "3",
        ),
        (
            "CAST to text",
            "SELECT CAST(amount AS TEXT) FROM conf_four WHERE id = 1",
            "10",
        ),
        (
            "cast with ::",
            "SELECT amount::text FROM conf_four WHERE id = 1",
            "10",
        ),
        ("NULLIF of equal values is NULL", "SELECT NULLIF(1, 1)", "NULL"),
        ("GREATEST", "SELECT GREATEST(1, 5, 3)", "5"),
        ("LEAST", "SELECT LEAST(4, 2, 9)", "2"),
        ("ABS", "SELECT ABS(-5)", "5"),
        ("UPPER and LOWER compose", "SELECT LOWER(UPPER('MiXeD'))", "mixed"),
        ("TRIM", "SELECT TRIM('  x  ')", "x"),
        ("REPLACE", "SELECT REPLACE('abc', 'b', 'X')", "aXc"),
        ("CONCAT()", "SELECT CONCAT('a', 'b')", "ab"),
        (
            "SUBSTRING",
            "SELECT SUBSTRING(name, 1, 2) FROM conf_four WHERE id = 1",
            "al",
        ),
        (
            "ORDER BY an ordinal",
            "SELECT id FROM conf_four ORDER BY 1 DESC",
            "3,2,1",
        ),
        (
            "ORDER BY an alias",
            "SELECT amount AS a FROM conf_four ORDER BY a DESC",
            "30,20,10",
        ),
        (
            "GROUP BY two keys",
            "SELECT COUNT(*) FROM conf_four GROUP BY name, amount",
            "1,1,1",
        ),
        (
            "aggregate over no rows is zero, not empty",
            "SELECT COUNT(*) FROM conf_four WHERE id = 999",
            "0",
        ),
        (
            "SUM over no rows is NULL, not zero",
            "SELECT SUM(amount) FROM conf_four WHERE id = 999",
            "NULL",
        ),
        ("LIMIT 0 returns nothing", "SELECT id FROM conf_four LIMIT 0", ""),
        // A LIMIT lets the filter stop early, which is only sound when nothing
        // downstream needs the rows it would skip. Each of these would return a
        // wrong answer — not a slow one — if the early exit ignored its clause.
        (
            "LIMIT does not truncate an aggregate",
            "SELECT COUNT(*) FROM conf_four LIMIT 1",
            "3",
        ),
        (
            "LIMIT does not truncate a GROUP BY",
            "SELECT COUNT(*) FROM conf_four GROUP BY name LIMIT 1",
            "1",
        ),
        (
            "ORDER BY with LIMIT sorts before it limits",
            "SELECT id FROM conf_four ORDER BY id DESC LIMIT 1",
            "3",
        ),
        (
            "LIMIT with OFFSET and no ORDER BY returns the right count",
            "SELECT id FROM conf_four LIMIT 2 OFFSET 1",
            "2,3",
        ),
        (
            "DISTINCT dedupes across every row, not just the limited ones",
            "SELECT DISTINCT name FROM conf_four ORDER BY name LIMIT 1",
            "alpha",
        ),
        (
            "CASE with no ELSE yields NULL",
            "SELECT CASE WHEN amount > 100 THEN 'big' END FROM conf_four WHERE id = 1",
            "NULL",
        ),
        (
            "EXISTS subquery",
            "SELECT id FROM conf_four WHERE EXISTS (SELECT 1 FROM conf_four WHERE id = 1) AND id = 2",
            "2",
        ),
        (
            "UPDATE reads the column it writes",
            "UPDATE conf_four SET amount = amount + 1 WHERE id = 1 RETURNING amount",
            "11",
        ),
        (
            "mixed sort directions",
            "SELECT id FROM conf_four ORDER BY name DESC, id ASC",
            "3,2,1",
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
        "NULLs sort where the statement says",
        async {
            client
                .simple_query("INSERT INTO conf_four (id, name, amount) VALUES (4, NULL, 40)")
                .await
                .map_err(describe)?;
            let ids = simple_column(
                &client,
                "SELECT id FROM conf_four ORDER BY name NULLS FIRST",
            )
            .await?;
            (ids.first().map(String::as_str) == Some("4"))
                .then_some(())
                .ok_or(format!("got {ids:?}, expected the NULL name first"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "INSERT ... SELECT copies rows",
        async {
            drop_table(&client, "conf_copy_sel").await;
            client
                .simple_query("CREATE TABLE conf_copy_sel (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_copy_sel (id, amount) \
                     SELECT id, amount FROM conf_four WHERE id < 3",
                )
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_copy_sel ORDER BY id").await?;
            let result = (ids == ["1", "2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1, 2]"));
            drop_table(&client, "conf_copy_sel").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "correlated subquery",
        simple_column(
            &client,
            "SELECT id FROM conf_four a \
             WHERE amount = (SELECT MAX(amount) FROM conf_four b WHERE b.id = a.id)",
        )
        .await
        .and_then(|values| {
            (values.len() == 4)
                .then_some(())
                .ok_or(format!("got {values:?}, expected every row"))
        }),
    );

    // ------------------------------------------------------------ constraints
    report.record(
        Area::Sql,
        "NOT NULL is enforced",
        async {
            drop_table(&client, "conf_notnull").await;
            client
                .simple_query("CREATE TABLE conf_notnull (id INTEGER NOT NULL)")
                .await
                .map_err(describe)?;
            let outcome = client
                .simple_query("INSERT INTO conf_notnull (id) VALUES (NULL)")
                .await;
            let result = outcome
                .is_err()
                .then_some(())
                .ok_or("a NULL was accepted into a NOT NULL column".to_string());
            drop_table(&client, "conf_notnull").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "PRIMARY KEY rejects a duplicate",
        async {
            drop_table(&client, "conf_pk").await;
            client
                .simple_query("CREATE TABLE conf_pk (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_pk (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let duplicate = client
                .simple_query("INSERT INTO conf_pk (id) VALUES (1)")
                .await;
            let rows = simple_column(&client, "SELECT id FROM conf_pk").await?;
            let result = if duplicate.is_err() {
                Ok(())
            } else {
                Err(format!(
                    "the duplicate was accepted; the table holds {rows:?}"
                ))
            };
            drop_table(&client, "conf_pk").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "DEFAULT fills an omitted column",
        async {
            drop_table(&client, "conf_default").await;
            client
                .simple_query("CREATE TABLE conf_default (id INTEGER, note TEXT DEFAULT 'none')")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_default (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let notes = simple_column(&client, "SELECT note FROM conf_default").await?;
            let result = (notes == ["none"])
                .then_some(())
                .ok_or(format!("got {notes:?}, expected the default"));
            drop_table(&client, "conf_default").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ON CONFLICT DO NOTHING",
        async {
            drop_table(&client, "conf_upsert").await;
            client
                .simple_query("CREATE TABLE conf_upsert (id INTEGER PRIMARY KEY, note TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_upsert (id, note) VALUES (1, 'first')")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_upsert (id, note) VALUES (1, 'second') \
                     ON CONFLICT (id) DO NOTHING",
                )
                .await
                .map_err(describe)?;
            let notes = simple_column(&client, "SELECT note FROM conf_upsert").await?;
            let result = (notes == ["first"])
                .then_some(())
                .ok_or(format!("got {notes:?}, expected the original row kept"));
            drop_table(&client, "conf_upsert").await;
            result
        }
        .await,
    );

    // ------------------------------------------------------------ error paths
    report.record(
        Area::Errors,
        "division by zero is an error, not NULL",
        client
            .simple_query("SELECT 1 / 0")
            .await
            .err()
            .map(|_| ())
            .ok_or_else(|| "division by zero was answered".to_string()),
    );

    report.record(
        Area::Errors,
        "an unknown column is an error",
        client
            .simple_query("SELECT no_such_column FROM conf_four")
            .await
            .err()
            .map(|_| ())
            .ok_or_else(|| "an unknown column produced a result".to_string()),
    );

    report.record(
        Area::Types,
        "an empty string is not NULL",
        async {
            client
                .simple_query("INSERT INTO conf_four (id, name, amount) VALUES (5, '', 50)")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT name FROM conf_four WHERE id = 5", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("no row".to_string())?;
            let name: Option<&str> = row.try_get(0).map_err(describe)?;
            (name == Some(""))
                .then_some(())
                .ok_or(format!("got {name:?}, expected an empty string"))
        }
        .await,
    );

    report.record(
        Area::Types,
        "a negative integer round-trips",
        async {
            client
                .simple_query("INSERT INTO conf_four (id, name, amount) VALUES (6, 'neg', -7)")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT amount FROM conf_four WHERE id = 6", &[])
                .await
                .map_err(describe)?;
            let row = rows.first().ok_or("no row".to_string())?;
            let amount: i32 = row.try_get(0).map_err(describe)?;
            (amount == -7)
                .then_some(())
                .ok_or(format!("got {amount}, expected -7"))
        }
        .await,
    );

    drop_table(&client, "conf_four").await;
    // -------------------------------------------- fifth round of coverage
    // The areas the PRD listed as unmeasured: arrays, CHECK and foreign keys,
    // DDL beyond ADD COLUMN, quoted identifiers, session state, savepoint
    // semantics, and the numeric and temporal functions.
    let client = match connect().await {
        Ok(fresh) => fresh,
        Err(e) => {
            report.record(Area::Sql, "reconnect for round five", Err(e));
            println!("{}", report.render());
            return;
        }
    };

    drop_table(&client, "conf_five").await;
    let _ = client
        .simple_query("CREATE TABLE conf_five (id INTEGER, name TEXT, amount INTEGER)")
        .await;
    for (id, name, amount) in [(1, "alpha", 10), (2, "beta", 20), (3, "gamma", 30)] {
        let _ = client
            .simple_query(&format!(
                "INSERT INTO conf_five (id, name, amount) VALUES ({id}, '{name}', {amount})"
            ))
            .await;
    }

    for (name, sql, want) in [
        // NULL is not a value that equals itself.
        (
            "NULL never equals NULL",
            "SELECT id FROM conf_five WHERE NULL = NULL",
            "",
        ),
        (
            "concatenating NULL yields NULL",
            "SELECT 'a' || NULL",
            "NULL",
        ),
        ("LENGTH(NULL) is NULL", "SELECT LENGTH(NULL)", "NULL"),
        (
            "LIKE with a single-character wildcard",
            "SELECT id FROM conf_five WHERE name LIKE '_lpha'",
            "1",
        ),
        ("MOD", "SELECT MOD(7, 3)", "1"),
        ("CEIL", "SELECT CEIL(1.2)", "2"),
        ("FLOOR", "SELECT FLOOR(1.8)", "1"),
        ("POWER", "SELECT POWER(2, 3)", "8"),
        ("SQRT", "SELECT SQRT(9)", "3"),
        (
            "EXTRACT from a date",
            "SELECT EXTRACT(YEAR FROM DATE '2026-01-02')",
            "2026",
        ),
        (
            "STRING_AGG with a separator",
            "SELECT STRING_AGG(name, '-') FROM conf_five",
            "alpha-beta-gamma",
        ),
        (
            "COUNT of a column skips NULL",
            "SELECT COUNT(name) FROM conf_five",
            "3",
        ),
        (
            "a schema-qualified table name resolves",
            "SELECT id FROM public.conf_five ORDER BY id",
            "1,2,3",
        ),
        (
            "DISTINCT ON keeps one row per key",
            "SELECT DISTINCT ON (amount) id FROM conf_five ORDER BY amount, id",
            "1,2,3",
        ),
        (
            "boolean predicate without a comparison",
            "SELECT id FROM conf_five WHERE true ORDER BY id",
            "1,2,3",
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
        "SELECT * returns each column once, in declaration order",
        client
            .query("SELECT * FROM conf_five WHERE id = 1", &[])
            .await
            .map_err(describe)
            .and_then(|rows| {
                let row = rows.first().ok_or("no row".to_string())?;
                let names: Vec<&str> = row.columns().iter().map(|c| c.name()).collect();
                (names == ["id", "name", "amount"])
                    .then_some(())
                    .ok_or(format!("got {names:?}, expected [id, name, amount]"))
            }),
    );

    report.record(
        Area::Sql,
        "CREATE TABLE AS SELECT",
        async {
            drop_table(&client, "conf_ctas").await;
            client
                .simple_query("CREATE TABLE conf_ctas AS SELECT id FROM conf_five WHERE id < 3")
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_ctas ORDER BY id").await?;
            let result = (ids == ["1", "2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1, 2]"));
            drop_table(&client, "conf_ctas").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ALTER TABLE DROP COLUMN",
        async {
            drop_table(&client, "conf_alter").await;
            client
                .simple_query("CREATE TABLE conf_alter (id INTEGER, spare TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_alter (id, spare) VALUES (1, 'x')")
                .await
                .map_err(describe)?;
            client
                .simple_query("ALTER TABLE conf_alter DROP COLUMN spare")
                .await
                .map_err(describe)?;
            // The column is gone, so naming it is an error.
            let result = client
                .simple_query("SELECT spare FROM conf_alter")
                .await
                .err()
                .map(|_| ())
                .ok_or_else(|| "the dropped column is still readable".to_string());
            drop_table(&client, "conf_alter").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a quoted identifier keeps its case",
        async {
            let _ = client
                .simple_query("DROP TABLE IF EXISTS \"ConfCase\"")
                .await;
            client
                .simple_query("CREATE TABLE \"ConfCase\" (\"Id\" INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO \"ConfCase\" (\"Id\") VALUES (7)")
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT \"Id\" FROM \"ConfCase\"").await?;
            let result = (ids == ["7"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [7]"));
            let _ = client
                .simple_query("DROP TABLE IF EXISTS \"ConfCase\"")
                .await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "CHECK rejects a violating row",
        async {
            drop_table(&client, "conf_check").await;
            client
                .simple_query("CREATE TABLE conf_check (id INTEGER CHECK (id > 0))")
                .await
                .map_err(describe)?;
            let outcome = client
                .simple_query("INSERT INTO conf_check (id) VALUES (-1)")
                .await;
            let result = outcome
                .is_err()
                .then_some(())
                .ok_or("a row violating CHECK was accepted".to_string());
            drop_table(&client, "conf_check").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an array literal round-trips",
        simple_column(&client, "SELECT ARRAY[1, 2, 3]")
            .await
            .and_then(|values| {
                let got = values.join(",");
                (got == "{1,2,3}")
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected {{1,2,3}}"))
            }),
    );

    // ------------------------------------------------------------- types
    report.record(
        Area::Types,
        "SMALLINT and REAL round-trip",
        async {
            drop_table(&client, "conf_narrow").await;
            client
                .simple_query("CREATE TABLE conf_narrow (small SMALLINT, approx REAL)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_narrow (small, approx) VALUES (7, 1.5)")
                .await
                .map_err(describe)?;
            let values = simple_column(&client, "SELECT small FROM conf_narrow").await?;
            let approx = simple_column(&client, "SELECT approx FROM conf_narrow").await?;
            let result = (values == ["7"] && approx == ["1.5"])
                .then_some(())
                .ok_or(format!("got {values:?} and {approx:?}"));
            drop_table(&client, "conf_narrow").await;
            result
        }
        .await,
    );

    report.record(
        Area::Types,
        "JSON round-trips",
        async {
            drop_table(&client, "conf_json").await;
            client
                .simple_query("CREATE TABLE conf_json (doc JSON)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_json (doc) VALUES ('{\"a\": 1}')")
                .await
                .map_err(describe)?;
            let docs = simple_column(&client, "SELECT doc FROM conf_json").await?;
            let result = docs
                .first()
                .is_some_and(|doc| doc.contains("\"a\"") && doc.contains('1'))
                .then_some(())
                .ok_or(format!("got {docs:?}"));
            drop_table(&client, "conf_json").await;
            result
        }
        .await,
    );

    // ------------------------------------------------------- session state
    report.record(
        Area::SimpleQuery,
        "SHOW reports what SET stored",
        async {
            client
                .simple_query("SET application_name = 'conformance'")
                .await
                .map_err(describe)?;
            let values = simple_column(&client, "SHOW application_name").await?;
            (values == ["conformance"])
                .then_some(())
                .ok_or(format!("got {values:?}, expected [conformance]"))
        }
        .await,
    );

    // -------------------------------------------------------- transactions
    report.record(
        Area::Transactions,
        "ROLLBACK TO SAVEPOINT undoes only the later work",
        async {
            drop_table(&client, "conf_savepoint").await;
            client
                .simple_query("CREATE TABLE conf_savepoint (id INTEGER)")
                .await
                .map_err(describe)?;
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_savepoint (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client.simple_query("SAVEPOINT s").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_savepoint (id) VALUES (2)")
                .await
                .map_err(describe)?;
            client
                .simple_query("ROLLBACK TO SAVEPOINT s")
                .await
                .map_err(describe)?;
            client.simple_query("COMMIT").await.map_err(describe)?;

            let ids = simple_column(&client, "SELECT id FROM conf_savepoint ORDER BY id").await?;
            let result = (ids == ["1"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1]"));
            drop_table(&client, "conf_savepoint").await;
            result
        }
        .await,
    );

    report.record(
        Area::Errors,
        "a statement after a failure inside a transaction is rejected",
        async {
            client.simple_query("BEGIN").await.map_err(describe)?;
            let _ = client.simple_query("SELECT * FROM no_such_table").await;
            let after = client.simple_query("SELECT 1").await;
            let _ = client.simple_query("ROLLBACK").await;
            after
                .is_err()
                .then_some(())
                .ok_or("the aborted transaction accepted another statement".to_string())
        }
        .await,
    );

    report.record(
        Area::Errors,
        "statements after a failure in one message do not run",
        async {
            drop_table(&client, "conf_multi").await;
            client
                .simple_query("CREATE TABLE conf_multi (id INTEGER)")
                .await
                .map_err(describe)?;
            let _ = client
                .simple_query(
                    "INSERT INTO conf_multi (id) VALUES (1); \
                     SELECT * FROM no_such_table; \
                     INSERT INTO conf_multi (id) VALUES (2)",
                )
                .await;
            let ids = simple_column(&client, "SELECT id FROM conf_multi ORDER BY id").await?;
            let result = (!ids.contains(&"2".to_string()))
                .then_some(())
                .ok_or(format!("got {ids:?}; the statement after the failure ran"));
            drop_table(&client, "conf_multi").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "one session's ROLLBACK spares another session's committed write",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_isolation").await;
            client
                .simple_query("CREATE TABLE conf_isolation (id INTEGER)")
                .await
                .map_err(describe)?;

            // One session opens a block and writes; the other commits its own
            // row while that block is open.
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_isolation (id) VALUES (1)")
                .await
                .map_err(describe)?;
            other
                .simple_query("INSERT INTO conf_isolation (id) VALUES (2)")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            // Rolling back the first session must not take the second
            // session's committed row with it. A table-level undo snapshot
            // would restore the table to how it stood before the block and
            // destroy row 2 — data loss, not merely a visibility anomaly.
            let ids = simple_column(&other, "SELECT id FROM conf_isolation").await?;
            let result = (ids == ["2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected only the committed row [2]"));
            drop_table(&client, "conf_isolation").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "ROLLBACK of an INSERT ... SELECT spares another session's row",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_iso_src").await;
            drop_table(&client, "conf_iso_dst").await;
            client
                .simple_query("CREATE TABLE conf_iso_src (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_iso_dst (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_iso_src (id) VALUES (1)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_iso_dst (id) SELECT id FROM conf_iso_src")
                .await
                .map_err(describe)?;
            other
                .simple_query("INSERT INTO conf_iso_dst (id) VALUES (9)")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            let ids = simple_column(&other, "SELECT id FROM conf_iso_dst").await?;
            let result = (ids == ["9"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected only the committed row [9]"));
            drop_table(&client, "conf_iso_src").await;
            drop_table(&client, "conf_iso_dst").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "ROLLBACK of a TRUNCATE restores the rows it removed",
        async {
            drop_table(&client, "conf_iso_trunc").await;
            client
                .simple_query("CREATE TABLE conf_iso_trunc (id INTEGER)")
                .await
                .map_err(describe)?;
            for id in [1, 2] {
                client
                    .simple_query(&format!("INSERT INTO conf_iso_trunc (id) VALUES ({id})"))
                    .await
                    .map_err(describe)?;
            }

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("TRUNCATE TABLE conf_iso_trunc")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            let ids = simple_column(&client, "SELECT id FROM conf_iso_trunc ORDER BY id").await?;
            let result = (ids == ["1", "2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1, 2]"));
            drop_table(&client, "conf_iso_trunc").await;
            result
        }
        .await,
    );

    for (name, sql, want) in [
        (
            "WITH RECURSIVE counts up",
            "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 5) \
             SELECT x FROM n",
            "1,2,3,4,5",
        ),
        (
            "WITH RECURSIVE settles on a UNION",
            "WITH RECURSIVE n(x) AS (SELECT 1 UNION SELECT x + 1 FROM n WHERE x < 3) \
             SELECT x FROM n",
            "1,2,3",
        ),
        (
            "PERCENT_RANK",
            "SELECT PERCENT_RANK() OVER (ORDER BY amount) FROM conf_five ORDER BY amount",
            "0,0.5,1",
        ),
        (
            "CUME_DIST",
            "SELECT CUME_DIST() OVER (ORDER BY amount) FROM conf_five ORDER BY amount",
            "0.3333333333333333,0.6666666666666666,1",
        ),
        (
            "a running total honours its frame",
            "SELECT SUM(amount) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) \
             FROM conf_five ORDER BY id",
            "10,30,60",
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
        "LATERAL reads the row to its left",
        simple_column(
            &client,
            "SELECT t.doubled FROM conf_five a, \
             LATERAL (SELECT a.amount * 2 AS doubled) t ORDER BY t.doubled",
        )
        .await
        .and_then(|values| {
            let got = values.join(",");
            (got == "20,40,60")
                .then_some(())
                .ok_or(format!("got {got:?}, expected 20,40,60"))
        }),
    );

    report.record(
        Area::Sql,
        "a foreign key rejects an orphan row",
        async {
            drop_table(&client, "conf_child").await;
            drop_table(&client, "conf_parent").await;
            client
                .simple_query("CREATE TABLE conf_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_child (id INTEGER, parent INTEGER REFERENCES conf_parent(id))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;

            // The parent exists, so this is allowed.
            client
                .simple_query("INSERT INTO conf_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;
            // This parent does not exist.
            let orphan = client
                .simple_query("INSERT INTO conf_child (id, parent) VALUES (2, 99)")
                .await;

            let result = orphan
                .is_err()
                .then_some(())
                .ok_or("an orphan row was accepted".to_string());
            drop_table(&client, "conf_child").await;
            drop_table(&client, "conf_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ALTER TABLE RENAME COLUMN keeps the values",
        async {
            drop_table(&client, "conf_rename").await;
            client
                .simple_query("CREATE TABLE conf_rename (id INTEGER, before TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_rename (id, before) VALUES (1, 'kept')")
                .await
                .map_err(describe)?;
            client
                .simple_query("ALTER TABLE conf_rename RENAME COLUMN before TO after")
                .await
                .map_err(describe)?;

            let values = simple_column(&client, "SELECT after FROM conf_rename").await?;
            let result = (values == ["kept"])
                .then_some(())
                .ok_or(format!("got {values:?}, expected the value to move"));
            drop_table(&client, "conf_rename").await;
            result
        }
        .await,
    );

    report.record(
        Area::SimpleQuery,
        "SHOW ALL lists the parameters that were set",
        async {
            client
                .simple_query("SET statement_timeout = '42'")
                .await
                .map_err(describe)?;
            let names = simple_column(&client, "SHOW ALL").await?;
            names
                .iter()
                .any(|name| name == "statement_timeout")
                .then_some(())
                .ok_or_else(|| format!("statement_timeout is missing from {names:?}"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "NATURAL JOIN matches on the shared column",
        async {
            drop_table(&client, "conf_nat_a").await;
            drop_table(&client, "conf_nat_b").await;
            client
                .simple_query("CREATE TABLE conf_nat_a (id INTEGER, left_value TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_nat_b (id INTEGER, right_value TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_nat_a (id, left_value) VALUES (1, 'a')")
                .await
                .map_err(describe)?;
            for (id, value) in [(1, "match"), (2, "other")] {
                client
                    .simple_query(&format!(
                        "INSERT INTO conf_nat_b (id, right_value) VALUES ({id}, '{value}')"
                    ))
                    .await
                    .map_err(describe)?;
            }

            let values = simple_column(
                &client,
                "SELECT right_value FROM conf_nat_a NATURAL JOIN conf_nat_b",
            )
            .await?;
            let result = (values == ["match"])
                .then_some(())
                .ok_or(format!("got {values:?}, expected only the matching row"));
            drop_table(&client, "conf_nat_a").await;
            drop_table(&client, "conf_nat_b").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a RANGE frame covers the whole peer group",
        async {
            drop_table(&client, "conf_peers").await;
            client
                .simple_query("CREATE TABLE conf_peers (id INTEGER, grade INTEGER)")
                .await
                .map_err(describe)?;
            // Two rows tie on the sort key, so RANGE and ROWS differ.
            for (id, grade) in [(1, 10), (2, 10), (3, 20)] {
                client
                    .simple_query(&format!(
                        "INSERT INTO conf_peers (id, grade) VALUES ({id}, {grade})"
                    ))
                    .await
                    .map_err(describe)?;
            }

            let ranged = simple_column(
                &client,
                "SELECT COUNT(*) OVER (ORDER BY grade RANGE BETWEEN UNBOUNDED PRECEDING \
                 AND CURRENT ROW) FROM conf_peers ORDER BY grade, id",
            )
            .await?;
            // The tied rows both see both of themselves; ROWS would give 1,2,3.
            let result = (ranged == ["2", "2", "3"])
                .then_some(())
                .ok_or(format!("got {ranged:?}, expected [2, 2, 3]"));
            drop_table(&client, "conf_peers").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a table-level FOREIGN KEY is enforced",
        async {
            drop_table(&client, "conf_tl_child").await;
            drop_table(&client, "conf_tl_parent").await;
            client
                .simple_query("CREATE TABLE conf_tl_parent (id INTEGER, PRIMARY KEY (id))")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_tl_child (id INTEGER, parent INTEGER, \
                     CONSTRAINT fk FOREIGN KEY (parent) REFERENCES conf_tl_parent(id))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_tl_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_tl_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            let orphan = client
                .simple_query("INSERT INTO conf_tl_child (id, parent) VALUES (2, 99)")
                .await;
            let result = orphan
                .is_err()
                .then_some(())
                .ok_or("a table-level foreign key was not enforced".to_string());
            drop_table(&client, "conf_tl_child").await;
            drop_table(&client, "conf_tl_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a referenced row cannot be deleted",
        async {
            drop_table(&client, "conf_ref_child").await;
            drop_table(&client, "conf_ref_parent").await;
            client
                .simple_query("CREATE TABLE conf_ref_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_ref_child (id INTEGER, \
                     parent INTEGER REFERENCES conf_ref_parent(id))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ref_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ref_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            let orphaning = client
                .simple_query("DELETE FROM conf_ref_parent WHERE id = 1")
                .await;
            let survived = simple_column(&client, "SELECT id FROM conf_ref_parent").await?;
            let result = (orphaning.is_err() && survived == ["1"])
                .then_some(())
                .ok_or_else(|| format!("the parent was deleted; rows left: {survived:?}"));
            drop_table(&client, "conf_ref_child").await;
            drop_table(&client, "conf_ref_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a materialized view stores its rows and REFRESH recomputes them",
        async {
            drop_table(&client, "conf_mat_src").await;
            let _ = client.simple_query("DROP TABLE IF EXISTS conf_mat").await;
            client
                .simple_query("CREATE TABLE conf_mat_src (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_mat_src (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE MATERIALIZED VIEW conf_mat AS SELECT id FROM conf_mat_src")
                .await
                .map_err(describe)?;

            // A materialized view does not follow its source until refreshed.
            client
                .simple_query("INSERT INTO conf_mat_src (id) VALUES (2)")
                .await
                .map_err(describe)?;
            let stale = simple_column(&client, "SELECT id FROM conf_mat ORDER BY id").await?;
            client
                .simple_query("REFRESH MATERIALIZED VIEW conf_mat")
                .await
                .map_err(describe)?;
            let fresh = simple_column(&client, "SELECT id FROM conf_mat ORDER BY id").await?;

            let result = (stale == ["1"] && fresh == ["1", "2"])
                .then_some(())
                .ok_or(format!("stale {stale:?} then fresh {fresh:?}"));
            let _ = client.simple_query("DROP TABLE IF EXISTS conf_mat").await;
            drop_table(&client, "conf_mat_src").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a composite FOREIGN KEY is checked as a whole",
        async {
            drop_table(&client, "conf_ck_child").await;
            drop_table(&client, "conf_ck_parent").await;
            client
                .simple_query(
                    "CREATE TABLE conf_ck_parent (a INTEGER, b INTEGER, PRIMARY KEY (a, b))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_ck_child (a INTEGER, b INTEGER, \
                     FOREIGN KEY (a, b) REFERENCES conf_ck_parent(a, b))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ck_parent (a, b) VALUES (1, 2)")
                .await
                .map_err(describe)?;

            // The pair (1, 2) exists.
            client
                .simple_query("INSERT INTO conf_ck_child (a, b) VALUES (1, 2)")
                .await
                .map_err(describe)?;
            // Each value exists on its own, but the pair (1, 3) does not — a
            // per-column check would wrongly accept this.
            let mismatched = client
                .simple_query("INSERT INTO conf_ck_child (a, b) VALUES (1, 3)")
                .await;

            let result = mismatched
                .is_err()
                .then_some(())
                .ok_or("a pair that does not exist together was accepted".to_string());
            drop_table(&client, "conf_ck_child").await;
            drop_table(&client, "conf_ck_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ON DELETE CASCADE removes the referring rows",
        async {
            drop_table(&client, "conf_cas_child").await;
            drop_table(&client, "conf_cas_parent").await;
            client
                .simple_query("CREATE TABLE conf_cas_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_cas_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_cas_parent(id) ON DELETE CASCADE)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_cas_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_cas_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            client
                .simple_query("DELETE FROM conf_cas_parent WHERE id = 1")
                .await
                .map_err(describe)?;
            let remaining = simple_column(&client, "SELECT id FROM conf_cas_child").await?;
            let result = remaining
                .is_empty()
                .then_some(())
                .ok_or(format!("{remaining:?} survived the cascade"));
            drop_table(&client, "conf_cas_child").await;
            drop_table(&client, "conf_cas_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ON DELETE SET NULL clears the referring column",
        async {
            drop_table(&client, "conf_sn_child").await;
            drop_table(&client, "conf_sn_parent").await;
            client
                .simple_query("CREATE TABLE conf_sn_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_sn_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_sn_parent(id) ON DELETE SET NULL)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_sn_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_sn_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            client
                .simple_query("DELETE FROM conf_sn_parent WHERE id = 1")
                .await
                .map_err(describe)?;
            let rows = client
                .query("SELECT parent FROM conf_sn_child", &[])
                .await
                .map_err(describe)?;
            let row = rows
                .first()
                .ok_or("the child row was deleted".to_string())?;
            let parent: Option<i32> = row.try_get(0).map_err(describe)?;
            let result = parent
                .is_none()
                .then_some(())
                .ok_or(format!("parent is {parent:?}, expected NULL"));
            drop_table(&client, "conf_sn_child").await;
            drop_table(&client, "conf_sn_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a referenced key cannot be updated away",
        async {
            drop_table(&client, "conf_up_child").await;
            drop_table(&client, "conf_up_parent").await;
            client
                .simple_query("CREATE TABLE conf_up_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_up_child (id INTEGER, \
                     parent INTEGER REFERENCES conf_up_parent(id))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_up_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_up_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            let moved = client
                .simple_query("UPDATE conf_up_parent SET id = 2 WHERE id = 1")
                .await;
            let still = simple_column(&client, "SELECT id FROM conf_up_parent").await?;
            let result = (moved.is_err() && still == ["1"])
                .then_some(())
                .ok_or_else(|| format!("the key moved; parent now {still:?}"));
            drop_table(&client, "conf_up_child").await;
            drop_table(&client, "conf_up_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "MATCH FULL rejects a half-null key",
        async {
            drop_table(&client, "conf_mf_child").await;
            drop_table(&client, "conf_mf_parent").await;
            client
                .simple_query(
                    "CREATE TABLE conf_mf_parent (a INTEGER, b INTEGER, PRIMARY KEY (a, b))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_mf_child (id INTEGER, a INTEGER, b INTEGER, \
                     FOREIGN KEY (a, b) REFERENCES conf_mf_parent(a, b) MATCH FULL)",
                )
                .await
                .map_err(describe)?;

            // All-NULL is allowed; a mixture is not.
            client
                .simple_query("INSERT INTO conf_mf_child (id, a, b) VALUES (1, NULL, NULL)")
                .await
                .map_err(describe)?;
            let mixed = client
                .simple_query("INSERT INTO conf_mf_child (id, a, b) VALUES (2, 1, NULL)")
                .await;

            let result = mixed
                .is_err()
                .then_some(())
                .ok_or("MATCH FULL accepted a half-null key".to_string());
            drop_table(&client, "conf_mf_child").await;
            drop_table(&client, "conf_mf_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ON UPDATE CASCADE moves the children",
        async {
            drop_table(&client, "conf_ou_child").await;
            drop_table(&client, "conf_ou_parent").await;
            client
                .simple_query("CREATE TABLE conf_ou_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_ou_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_ou_parent(id) ON UPDATE CASCADE)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ou_parent (id) VALUES (1)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ou_child (id, parent) VALUES (1, 1)")
                .await
                .map_err(describe)?;

            client
                .simple_query("UPDATE conf_ou_parent SET id = 2 WHERE id = 1")
                .await
                .map_err(describe)?;
            let parents = simple_column(&client, "SELECT parent FROM conf_ou_child").await?;
            let result = (parents == ["2"])
                .then_some(())
                .ok_or(format!("child points at {parents:?}, expected [2]"));
            drop_table(&client, "conf_ou_child").await;
            drop_table(&client, "conf_ou_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a DEFERRABLE key is checked at COMMIT, not at INSERT",
        async {
            drop_table(&client, "conf_def_child").await;
            drop_table(&client, "conf_def_parent").await;
            client
                .simple_query("CREATE TABLE conf_def_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_def_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_def_parent(id) DEFERRABLE)",
                )
                .await
                .map_err(describe)?;

            // The child is written before its parent exists, which an
            // immediate check would refuse.
            client.simple_query("BEGIN").await.map_err(describe)?;
            let early = client
                .simple_query("INSERT INTO conf_def_child (id, parent) VALUES (1, 7)")
                .await;
            client
                .simple_query("INSERT INTO conf_def_parent (id) VALUES (7)")
                .await
                .map_err(describe)?;
            let committed = client.simple_query("COMMIT").await;

            let rows = simple_column(&client, "SELECT id FROM conf_def_child").await?;
            let result = (early.is_ok() && committed.is_ok() && rows == ["1"])
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "insert {:?}, commit {:?}, rows {rows:?}",
                        early.is_ok(),
                        committed.is_ok()
                    )
                });
            drop_table(&client, "conf_def_child").await;
            drop_table(&client, "conf_def_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a domain carries its type and constraints",
        async {
            let _ = client
                .simple_query("DROP DOMAIN IF EXISTS conf_positive")
                .await;
            drop_table(&client, "conf_domain").await;
            client
                .simple_query("CREATE DOMAIN conf_positive AS INTEGER NOT NULL")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_domain (id conf_positive)")
                .await
                .map_err(describe)?;

            // The base type accepts an integer...
            client
                .simple_query("INSERT INTO conf_domain (id) VALUES (5)")
                .await
                .map_err(describe)?;
            // ...and the domain's NOT NULL is inherited by the column.
            let null = client
                .simple_query("INSERT INTO conf_domain (id) VALUES (NULL)")
                .await;

            let values = simple_column(&client, "SELECT id FROM conf_domain").await?;
            let result = (null.is_err() && values == ["5"])
                .then_some(())
                .ok_or_else(|| format!("null accepted: {}, values {values:?}", null.is_ok()));
            drop_table(&client, "conf_domain").await;
            let _ = client
                .simple_query("DROP DOMAIN IF EXISTS conf_positive")
                .await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an AFTER INSERT trigger runs its statement",
        async {
            drop_table(&client, "conf_trig_src").await;
            drop_table(&client, "conf_trig_log").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_trig_src (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_trig_log (note TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_trig AFTER INSERT ON conf_trig_src \
                     FOR EACH ROW EXECUTE INSERT INTO conf_trig_log (note) VALUES ('fired')",
                )
                .await
                .map_err(describe)?;

            // Nothing is logged until the watched table is written.
            let before = simple_column(&client, "SELECT note FROM conf_trig_log").await?;
            client
                .simple_query("INSERT INTO conf_trig_src (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let after = simple_column(&client, "SELECT note FROM conf_trig_log").await?;

            let result = (before.is_empty() && after == ["fired"])
                .then_some(())
                .ok_or(format!("log was {before:?} then {after:?}"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_trig")
                .await;
            drop_table(&client, "conf_trig_src").await;
            drop_table(&client, "conf_trig_log").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a domain CHECK applies to the column",
        async {
            let _ = client.simple_query("DROP DOMAIN IF EXISTS conf_pos").await;
            drop_table(&client, "conf_domain_check").await;
            client
                .simple_query("CREATE DOMAIN conf_pos AS INTEGER CHECK (VALUE > 0)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_domain_check (amount conf_pos)")
                .await
                .map_err(describe)?;

            client
                .simple_query("INSERT INTO conf_domain_check (amount) VALUES (5)")
                .await
                .map_err(describe)?;
            let negative = client
                .simple_query("INSERT INTO conf_domain_check (amount) VALUES (-1)")
                .await;

            let result = negative
                .is_err()
                .then_some(())
                .ok_or("the domain's CHECK did not apply".to_string());
            drop_table(&client, "conf_domain_check").await;
            let _ = client.simple_query("DROP DOMAIN IF EXISTS conf_pos").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SET CONSTRAINTS ALL IMMEDIATE reports a deferred failure early",
        async {
            drop_table(&client, "conf_sc_child").await;
            drop_table(&client, "conf_sc_parent").await;
            client
                .simple_query("CREATE TABLE conf_sc_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_sc_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_sc_parent(id) DEFERRABLE)",
                )
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_sc_child (id, parent) VALUES (1, 404)")
                .await
                .map_err(describe)?;
            // The parent never arrives, so asking now must say so.
            let early = client.simple_query("SET CONSTRAINTS ALL IMMEDIATE").await;
            let _ = client.simple_query("ROLLBACK").await;

            let result = early
                .is_err()
                .then_some(())
                .ok_or("SET CONSTRAINTS ALL IMMEDIATE reported no problem".to_string());
            drop_table(&client, "conf_sc_child").await;
            drop_table(&client, "conf_sc_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Types,
        "a row whose columns are all NULL can be stored",
        async {
            drop_table(&client, "conf_allnull").await;
            client
                .simple_query("CREATE TABLE conf_allnull (a INTEGER, b TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_allnull (a, b) VALUES (NULL, NULL)")
                .await
                .map_err(describe)?;

            let rows = client
                .query("SELECT a FROM conf_allnull", &[])
                .await
                .map_err(describe)?;
            let result = (rows.len() == 1)
                .then_some(())
                .ok_or(format!("{} rows, expected the all-NULL row", rows.len()));
            drop_table(&client, "conf_allnull").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a FOR EACH ROW trigger reads NEW",
        async {
            drop_table(&client, "conf_new_src").await;
            drop_table(&client, "conf_new_log").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_new_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_new_src (id INTEGER, label TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_new_log (seen TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_new_trig AFTER INSERT ON conf_new_src \
                     FOR EACH ROW EXECUTE INSERT INTO conf_new_log (seen) VALUES (NEW.label)",
                )
                .await
                .map_err(describe)?;

            client
                .simple_query("INSERT INTO conf_new_src (id, label) VALUES (1, 'written')")
                .await
                .map_err(describe)?;
            let seen = simple_column(&client, "SELECT seen FROM conf_new_log").await?;

            let result = (seen == ["written"])
                .then_some(())
                .ok_or(format!("log holds {seen:?}, expected the NEW value"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_new_trig")
                .await;
            drop_table(&client, "conf_new_src").await;
            drop_table(&client, "conf_new_log").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a trigger WHEN clause gates the firing",
        async {
            drop_table(&client, "conf_when_src").await;
            drop_table(&client, "conf_when_log").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_when_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_when_src (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_when_log (amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_when_trig AFTER INSERT ON conf_when_src \
                     FOR EACH ROW WHEN (NEW.amount > 10) \
                     EXECUTE INSERT INTO conf_when_log (amount) VALUES (NEW.amount)",
                )
                .await
                .map_err(describe)?;

            // Below the threshold nothing is logged; above it, one row is.
            client
                .simple_query("INSERT INTO conf_when_src (id, amount) VALUES (1, 5)")
                .await
                .map_err(describe)?;
            let quiet = simple_column(&client, "SELECT amount FROM conf_when_log").await?;
            client
                .simple_query("INSERT INTO conf_when_src (id, amount) VALUES (2, 50)")
                .await
                .map_err(describe)?;
            let fired = simple_column(&client, "SELECT amount FROM conf_when_log").await?;

            let result = (quiet.is_empty() && fired == ["50"])
                .then_some(())
                .ok_or(format!("log was {quiet:?} then {fired:?}"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_when_trig")
                .await;
            drop_table(&client, "conf_when_src").await;
            drop_table(&client, "conf_when_log").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SET CONSTRAINTS IMMEDIATE then DEFERRED changes when checks run",
        async {
            drop_table(&client, "conf_mode_child").await;
            drop_table(&client, "conf_mode_parent").await;
            client
                .simple_query("CREATE TABLE conf_mode_parent (id INTEGER PRIMARY KEY)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_mode_child (id INTEGER, parent INTEGER \
                     REFERENCES conf_mode_parent(id) DEFERRABLE)",
                )
                .await
                .map_err(describe)?;

            // Under IMMEDIATE the bad row is refused at the statement.
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("SET CONSTRAINTS ALL IMMEDIATE")
                .await
                .map_err(describe)?;
            let refused_now = client
                .simple_query("INSERT INTO conf_mode_child (id, parent) VALUES (1, 404)")
                .await;
            let _ = client.simple_query("ROLLBACK").await;

            // Under DEFERRED the same row is accepted and only COMMIT objects.
            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("SET CONSTRAINTS ALL DEFERRED")
                .await
                .map_err(describe)?;
            let accepted_now = client
                .simple_query("INSERT INTO conf_mode_child (id, parent) VALUES (2, 404)")
                .await;
            let refused_at_commit = client.simple_query("COMMIT").await;
            let _ = client.simple_query("ROLLBACK").await;

            let result =
                (refused_now.is_err() && accepted_now.is_ok() && refused_at_commit.is_err())
                    .then_some(())
                    .ok_or_else(|| {
                        format!(
                            "immediate refused {}, deferred accepted {}, commit refused {}",
                            refused_now.is_err(),
                            accepted_now.is_ok(),
                            refused_at_commit.is_err()
                        )
                    });
            drop_table(&client, "conf_mode_child").await;
            drop_table(&client, "conf_mode_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an INSTEAD OF trigger makes a view writable",
        async {
            drop_table(&client, "conf_io_base").await;
            let _ = client
                .simple_query("DROP VIEW IF EXISTS conf_io_view")
                .await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_io_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_io_base (id INTEGER, note TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE VIEW conf_io_view AS SELECT id FROM conf_io_base")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_io_trig INSTEAD OF INSERT ON conf_io_view \
                     FOR EACH ROW EXECUTE INSERT INTO conf_io_base (id, note) \
                     VALUES (NEW.id, 'through the view')",
                )
                .await
                .map_err(describe)?;

            client
                .simple_query("INSERT INTO conf_io_view (id) VALUES (7)")
                .await
                .map_err(describe)?;
            let notes = simple_column(&client, "SELECT note FROM conf_io_base").await?;

            let result = (notes == ["through the view"])
                .then_some(())
                .ok_or(format!("base table holds {notes:?}"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_io_trig")
                .await;
            let _ = client
                .simple_query("DROP VIEW IF EXISTS conf_io_view")
                .await;
            drop_table(&client, "conf_io_base").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a BEFORE trigger rewrites the row being written",
        async {
            drop_table(&client, "conf_bt").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_bt_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_bt (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_bt_trig BEFORE INSERT ON conf_bt \
                     FOR EACH ROW EXECUTE SET NEW.amount = NEW.amount * 2",
                )
                .await
                .map_err(describe)?;

            client
                .simple_query("INSERT INTO conf_bt (id, amount) VALUES (1, 21)")
                .await
                .map_err(describe)?;
            let stored = simple_column(&client, "SELECT amount FROM conf_bt").await?;

            let result = (stored == ["42"])
                .then_some(())
                .ok_or(format!("stored {stored:?}, expected the doubled value"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_bt_trig")
                .await;
            drop_table(&client, "conf_bt").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "MATCH PARTIAL checks the parts that are present",
        async {
            drop_table(&client, "conf_mp_child").await;
            drop_table(&client, "conf_mp_parent").await;
            client
                .simple_query(
                    "CREATE TABLE conf_mp_parent (a INTEGER, b INTEGER, PRIMARY KEY (a, b))",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TABLE conf_mp_child (id INTEGER, a INTEGER, b INTEGER, \
                     FOREIGN KEY (a, b) REFERENCES conf_mp_parent(a, b) MATCH PARTIAL)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_mp_parent (a, b) VALUES (1, 2)")
                .await
                .map_err(describe)?;

            // The present half matches a row, so this is allowed...
            let matching = client
                .simple_query("INSERT INTO conf_mp_child (id, a, b) VALUES (1, 1, NULL)")
                .await;
            // ...and this one does not match any row, so it is not.
            let missing = client
                .simple_query("INSERT INTO conf_mp_child (id, a, b) VALUES (2, 99, NULL)")
                .await;

            let result = (matching.is_ok() && missing.is_err())
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "matching accepted {}, missing rejected {}",
                        matching.is_ok(),
                        missing.is_err()
                    )
                });
            drop_table(&client, "conf_mp_child").await;
            drop_table(&client, "conf_mp_parent").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ALTER DOMAIN reaches a table that already uses it",
        async {
            let _ = client
                .simple_query("DROP DOMAIN IF EXISTS conf_alt_dom")
                .await;
            drop_table(&client, "conf_alt_use").await;
            client
                .simple_query("CREATE DOMAIN conf_alt_dom AS INTEGER")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_alt_use (amount conf_alt_dom)")
                .await
                .map_err(describe)?;

            // Unconstrained to begin with.
            client
                .simple_query("INSERT INTO conf_alt_use (amount) VALUES (-1)")
                .await
                .map_err(describe)?;

            // The constraint is added after the table already exists.
            client
                .simple_query("ALTER DOMAIN conf_alt_dom ADD CHECK (VALUE > 0)")
                .await
                .map_err(describe)?;
            let refused = client
                .simple_query("INSERT INTO conf_alt_use (amount) VALUES (-2)")
                .await;
            let allowed = client
                .simple_query("INSERT INTO conf_alt_use (amount) VALUES (3)")
                .await;

            let result = (refused.is_err() && allowed.is_ok())
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "negative refused {}, positive allowed {}",
                        refused.is_err(),
                        allowed.is_ok()
                    )
                });
            drop_table(&client, "conf_alt_use").await;
            let _ = client
                .simple_query("DROP DOMAIN IF EXISTS conf_alt_dom")
                .await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a BEFORE UPDATE trigger rewrites the row",
        async {
            drop_table(&client, "conf_bu").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_bu_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_bu (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_bu (id, amount) VALUES (1, 5)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_bu_trig BEFORE UPDATE ON conf_bu \
                     FOR EACH ROW EXECUTE SET NEW.amount = 100",
                )
                .await
                .map_err(describe)?;

            client
                .simple_query("UPDATE conf_bu SET amount = 7 WHERE id = 1")
                .await
                .map_err(describe)?;
            let stored = simple_column(&client, "SELECT amount FROM conf_bu").await?;

            // The trigger's value wins over the statement's.
            let result = (stored == ["100"])
                .then_some(())
                .ok_or(format!("stored {stored:?}, expected the trigger's value"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_bu_trig")
                .await;
            drop_table(&client, "conf_bu").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a trigger body runs several statements",
        async {
            drop_table(&client, "conf_body_src").await;
            drop_table(&client, "conf_body_log").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_body_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_body_src (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_body_log (note TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_body_trig AFTER INSERT ON conf_body_src \
                     FOR EACH ROW EXECUTE $$BEGIN \
                     INSERT INTO conf_body_log (note) VALUES ('first'); \
                     INSERT INTO conf_body_log (note) VALUES ('second') END$$",
                )
                .await
                .map_err(describe)?;

            client
                .simple_query("INSERT INTO conf_body_src (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let notes =
                simple_column(&client, "SELECT note FROM conf_body_log ORDER BY note").await?;

            let result = (notes == ["first", "second"])
                .then_some(())
                .ok_or(format!("log holds {notes:?}, expected both statements"));
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_body_trig")
                .await;
            drop_table(&client, "conf_body_src").await;
            drop_table(&client, "conf_body_log").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "RAISE in a BEFORE trigger rejects the write",
        async {
            drop_table(&client, "conf_raise").await;
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_raise_trig")
                .await;
            client
                .simple_query("CREATE TABLE conf_raise (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE TRIGGER conf_raise_trig BEFORE INSERT ON conf_raise \
                     FOR EACH ROW WHEN (NEW.amount < 0) \
                     EXECUTE RAISE EXCEPTION 'amount must not be negative'",
                )
                .await
                .map_err(describe)?;

            let allowed = client
                .simple_query("INSERT INTO conf_raise (id, amount) VALUES (1, 5)")
                .await;
            let refused = client
                .simple_query("INSERT INTO conf_raise (id, amount) VALUES (2, -5)")
                .await;
            let stored = simple_column(&client, "SELECT id FROM conf_raise").await?;

            let result = (allowed.is_ok() && refused.is_err() && stored == ["1"])
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "allowed {}, refused {}, rows {stored:?}",
                        allowed.is_ok(),
                        refused.is_err()
                    )
                });
            let _ = client
                .simple_query("DROP TRIGGER IF EXISTS conf_raise_trig")
                .await;
            drop_table(&client, "conf_raise").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "an uncommitted write is invisible to another session",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_iso_read").await;
            client
                .simple_query("CREATE TABLE conf_iso_read (id INTEGER)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_iso_read (id) VALUES (1)")
                .await
                .map_err(describe)?;

            // The writer sees its own row; nobody else does yet.
            let own = simple_column(&client, "SELECT id FROM conf_iso_read").await?;
            let others = simple_column(&other, "SELECT id FROM conf_iso_read").await?;

            client.simple_query("COMMIT").await.map_err(describe)?;
            let after_commit = simple_column(&other, "SELECT id FROM conf_iso_read").await?;

            let result = (own == ["1"] && others.is_empty() && after_commit == ["1"])
                .then_some(())
                .ok_or_else(|| {
                    format!("writer saw {own:?}, other saw {others:?} then {after_commit:?}")
                });
            drop_table(&client, "conf_iso_read").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a rolled-back write is never visible to another session",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_iso_roll").await;
            client
                .simple_query("CREATE TABLE conf_iso_roll (id INTEGER)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_iso_roll (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let during = simple_column(&other, "SELECT id FROM conf_iso_roll").await?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;
            let after = simple_column(&other, "SELECT id FROM conf_iso_roll").await?;

            let result = (during.is_empty() && after.is_empty())
                .then_some(())
                .ok_or(format!("other saw {during:?} during and {after:?} after"));
            drop_table(&client, "conf_iso_roll").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "an uncommitted delete is invisible to another session",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_iso_del").await;
            client
                .simple_query("CREATE TABLE conf_iso_del (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_iso_del (id) VALUES (1)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("DELETE FROM conf_iso_del WHERE id = 1")
                .await
                .map_err(describe)?;

            // The deleting session no longer sees it; nobody else has lost it
            // yet, because the delete has not committed.
            let own = simple_column(&client, "SELECT id FROM conf_iso_del").await?;
            let others = simple_column(&other, "SELECT id FROM conf_iso_del").await?;
            client.simple_query("COMMIT").await.map_err(describe)?;
            let after = simple_column(&other, "SELECT id FROM conf_iso_del").await?;

            let result = (own.is_empty() && others == ["1"] && after.is_empty())
                .then_some(())
                .ok_or_else(|| format!("writer saw {own:?}, other saw {others:?} then {after:?}"));
            drop_table(&client, "conf_iso_del").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "REPEATABLE READ sees the same rows twice",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_rr").await;
            client
                .simple_query("CREATE TABLE conf_rr (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_rr (id) VALUES (1)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL REPEATABLE READ")
                .await
                .map_err(describe)?;
            let first = simple_column(&client, "SELECT id FROM conf_rr").await?;

            // Another session commits a row after the snapshot was taken.
            other
                .simple_query("INSERT INTO conf_rr (id) VALUES (2)")
                .await
                .map_err(describe)?;

            let second = simple_column(&client, "SELECT id FROM conf_rr").await?;
            client.simple_query("COMMIT").await.map_err(describe)?;
            let after = simple_column(&client, "SELECT id FROM conf_rr ORDER BY id").await?;

            // The repeated read is unchanged; the new row appears once the
            // block ends.
            let result = (first == ["1"] && second == ["1"] && after == ["1", "2"])
                .then_some(())
                .ok_or_else(|| format!("{first:?} then {second:?}, after commit {after:?}"));
            drop_table(&client, "conf_rr").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "READ COMMITTED sees a commit that lands mid-block",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_rc").await;
            client
                .simple_query("CREATE TABLE conf_rc (id INTEGER)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            let first = simple_column(&client, "SELECT id FROM conf_rc").await?;
            other
                .simple_query("INSERT INTO conf_rc (id) VALUES (5)")
                .await
                .map_err(describe)?;
            let second = simple_column(&client, "SELECT id FROM conf_rc").await?;
            client.simple_query("COMMIT").await.map_err(describe)?;

            // The default level is read-committed, so the second read differs.
            let result = (first.is_empty() && second == ["5"])
                .then_some(())
                .ok_or(format!("{first:?} then {second:?}"));
            drop_table(&client, "conf_rc").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "REPEATABLE READ sees a row as it stood, not as it was updated to",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_ver").await;
            client
                .simple_query("CREATE TABLE conf_ver (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ver (id, amount) VALUES (1, 10)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL REPEATABLE READ")
                .await
                .map_err(describe)?;
            let first = simple_column(&client, "SELECT amount FROM conf_ver").await?;

            // Another session changes the row and commits, after the snapshot.
            other.simple_query("BEGIN").await.map_err(describe)?;
            other
                .simple_query("UPDATE conf_ver SET amount = 99 WHERE id = 1")
                .await
                .map_err(describe)?;
            other.simple_query("COMMIT").await.map_err(describe)?;

            let second = simple_column(&client, "SELECT amount FROM conf_ver").await?;
            client.simple_query("COMMIT").await.map_err(describe)?;
            let after = simple_column(&client, "SELECT amount FROM conf_ver").await?;

            // The snapshot keeps the old value; the new one appears after.
            let result = (first == ["10"] && second == ["10"] && after == ["99"])
                .then_some(())
                .ok_or_else(|| format!("{first:?} then {second:?}, after commit {after:?}"));
            drop_table(&client, "conf_ver").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a rolled-back update leaves the original row",
        async {
            drop_table(&client, "conf_verroll").await;
            client
                .simple_query("CREATE TABLE conf_verroll (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_verroll (id, amount) VALUES (1, 10)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("UPDATE conf_verroll SET amount = 99 WHERE id = 1")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            // Exactly one row, holding the value it started with.
            let amounts = simple_column(&client, "SELECT amount FROM conf_verroll").await?;
            let result = (amounts == ["10"])
                .then_some(())
                .ok_or(format!("got {amounts:?}, expected a single row of 10"));
            drop_table(&client, "conf_verroll").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SERIALIZABLE fails a block whose read moved underneath it",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_ser").await;
            client
                .simple_query("CREATE TABLE conf_ser (id INTEGER, amount INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ser (id, amount) VALUES (1, 10)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL SERIALIZABLE")
                .await
                .map_err(describe)?;
            // Reading it is what puts the table under the block's protection.
            let _ = simple_column(&client, "SELECT amount FROM conf_ser").await?;

            // Another session changes what was read, and commits first.
            other
                .simple_query("UPDATE conf_ser SET amount = 99 WHERE id = 1")
                .await
                .map_err(describe)?;

            let committed = client.simple_query("COMMIT").await;
            let _ = client.simple_query("ROLLBACK").await;

            let result = committed
                .is_err()
                .then_some(())
                .ok_or("the serializable block committed over a concurrent write".to_string());
            drop_table(&client, "conf_ser").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SERIALIZABLE commits when nothing moved",
        async {
            drop_table(&client, "conf_ser_ok").await;
            client
                .simple_query("CREATE TABLE conf_ser_ok (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_ser_ok (id) VALUES (1)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL SERIALIZABLE")
                .await
                .map_err(describe)?;
            let read = simple_column(&client, "SELECT id FROM conf_ser_ok").await?;
            let committed = client.simple_query("COMMIT").await;

            // Nothing else touched the table, so the block must succeed —
            // a check that only ever fails proves nothing.
            let result = (read == ["1"] && committed.is_ok())
                .then_some(())
                .ok_or_else(|| format!("read {read:?}, commit ok {}", committed.is_ok()));
            drop_table(&client, "conf_ser_ok").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "VACUUM reclaims the rows a committed delete left behind",
        async {
            drop_table(&client, "conf_vac").await;
            client
                .simple_query("CREATE TABLE conf_vac (id INTEGER)")
                .await
                .map_err(describe)?;
            for id in [1, 2] {
                client
                    .simple_query(&format!("INSERT INTO conf_vac (id) VALUES ({id})"))
                    .await
                    .map_err(describe)?;
            }

            client.simple_query("BEGIN").await.map_err(describe)?;
            client
                .simple_query("DELETE FROM conf_vac WHERE id = 1")
                .await
                .map_err(describe)?;
            client.simple_query("COMMIT").await.map_err(describe)?;

            client
                .simple_query("VACUUM conf_vac")
                .await
                .map_err(describe)?;

            // The surviving row is untouched by the reclamation.
            let remaining = simple_column(&client, "SELECT id FROM conf_vac").await?;
            let result = (remaining == ["2"]).then_some(()).ok_or(format!(
                "got {remaining:?}, expected only the row that stayed"
            ));
            drop_table(&client, "conf_vac").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SERIALIZABLE tolerates a write to a row it did not read",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_ser_row").await;
            client
                .simple_query("CREATE TABLE conf_ser_row (id INTEGER PRIMARY KEY, amount INTEGER)")
                .await
                .map_err(describe)?;
            for id in [1, 2] {
                client
                    .simple_query(&format!(
                        "INSERT INTO conf_ser_row (id, amount) VALUES ({id}, 10)"
                    ))
                    .await
                    .map_err(describe)?;
            }

            client
                .simple_query("BEGIN ISOLATION LEVEL SERIALIZABLE")
                .await
                .map_err(describe)?;
            // Only row 1 is read.
            let read =
                simple_column(&client, "SELECT amount FROM conf_ser_row WHERE id = 1").await?;

            // Another session writes row 2, which this block never looked at.
            other
                .simple_query("UPDATE conf_ser_row SET amount = 99 WHERE id = 2")
                .await
                .map_err(describe)?;

            let committed = client.simple_query("COMMIT").await;
            let _ = client.simple_query("ROLLBACK").await;

            // A table-grained check would fail this; a row-grained one lets it
            // through, which is the point of recording the rows.
            let result = (read == ["10"] && committed.is_ok())
                .then_some(())
                .ok_or_else(|| format!("read {read:?}, commit ok {}", committed.is_ok()));
            drop_table(&client, "conf_ser_row").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "SERIALIZABLE detects a phantom inserted into a range it read",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_phantom").await;
            client
                .simple_query("CREATE TABLE conf_phantom (id INTEGER PRIMARY KEY, grp INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_phantom (id, grp) VALUES (1, 7)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL SERIALIZABLE")
                .await
                .map_err(describe)?;
            // The block reads a range, not a row.
            let seen = simple_column(&client, "SELECT id FROM conf_phantom WHERE grp = 7").await?;

            // Another session adds a row that would have been in that range.
            other
                .simple_query("INSERT INTO conf_phantom (id, grp) VALUES (2, 7)")
                .await
                .map_err(describe)?;

            let committed = client.simple_query("COMMIT").await;
            let _ = client.simple_query("ROLLBACK").await;

            // Watching only the rows returned cannot see this: the phantom was
            // not there to record.
            let result = (seen == ["1"] && committed.is_err())
                .then_some(())
                .ok_or_else(|| format!("read {seen:?}, commit ok {}", committed.is_ok()));
            drop_table(&client, "conf_phantom").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a phantom outside the range read is not a conflict",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_nophantom").await;
            client
                .simple_query("CREATE TABLE conf_nophantom (id INTEGER PRIMARY KEY, grp INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_nophantom (id, grp) VALUES (1, 7)")
                .await
                .map_err(describe)?;

            client
                .simple_query("BEGIN ISOLATION LEVEL SERIALIZABLE")
                .await
                .map_err(describe)?;
            let seen =
                simple_column(&client, "SELECT id FROM conf_nophantom WHERE grp = 7").await?;

            // A row in a different group: outside the predicate the block read.
            other
                .simple_query("INSERT INTO conf_nophantom (id, grp) VALUES (2, 8)")
                .await
                .map_err(describe)?;

            let committed = client.simple_query("COMMIT").await;
            let _ = client.simple_query("ROLLBACK").await;

            let result = (seen == ["1"] && committed.is_ok())
                .then_some(())
                .ok_or_else(|| format!("read {seen:?}, commit ok {}", committed.is_ok()));
            drop_table(&client, "conf_nophantom").await;
            result
        }
        .await,
    );

    report.record(
        Area::Copy,
        "binary COPY round-trips through the server",
        async {
            use futures_util::{SinkExt, TryStreamExt};

            drop_table(&client, "conf_bincopy").await;
            client
                .simple_query("CREATE TABLE conf_bincopy (id INTEGER, note TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_bincopy (id, note) VALUES (1, 'first')")
                .await
                .map_err(describe)?;

            // Read the table out in the binary format...
            let stream = client
                .copy_out("COPY conf_bincopy (id, note) TO STDOUT (FORMAT BINARY)")
                .await
                .map_err(describe)?;
            let chunks: Vec<bytes::Bytes> = stream.try_collect().await.map_err(describe)?;
            let payload: Vec<u8> = chunks.concat();
            if !payload.starts_with(b"PGCOPY\n\xff\r\n\0") {
                let _ = client
                    .simple_query("DROP TABLE IF EXISTS conf_bincopy")
                    .await;
                return Err(format!(
                    "stream did not start with the signature: {:?}",
                    &payload[..payload.len().min(16)]
                ));
            }

            // ...and load it straight back into a second table.
            drop_table(&client, "conf_bincopy2").await;
            client
                .simple_query("CREATE TABLE conf_bincopy2 (id INTEGER, note TEXT)")
                .await
                .map_err(describe)?;
            let sink = client
                .copy_in("COPY conf_bincopy2 (id, note) FROM STDIN (FORMAT BINARY)")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(sink);
            sink.send(bytes::Bytes::from(payload))
                .await
                .map_err(describe)?;
            sink.close().await.map_err(describe)?;

            let notes = simple_column(&client, "SELECT note FROM conf_bincopy2").await?;
            let result = (notes == ["first"]).then_some(()).ok_or(format!(
                "got {notes:?}, expected the row to survive the round trip"
            ));
            let _ = client
                .simple_query("DROP TABLE IF EXISTS conf_bincopy")
                .await;
            let _ = client
                .simple_query("DROP TABLE IF EXISTS conf_bincopy2")
                .await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "ROLLBACK of a COPY spares another session's row",
        async {
            let other = connect().await?;
            drop_table(&client, "conf_iso_copy").await;
            client
                .simple_query("CREATE TABLE conf_iso_copy (id INTEGER)")
                .await
                .map_err(describe)?;

            client.simple_query("BEGIN").await.map_err(describe)?;
            let sink = client
                .copy_in("COPY conf_iso_copy (id) FROM STDIN")
                .await
                .map_err(describe)?;
            futures_util::pin_mut!(sink);
            {
                use futures_util::SinkExt;
                sink.send(bytes::Bytes::from_static(b"1\n2\n"))
                    .await
                    .map_err(describe)?;
                sink.close().await.map_err(describe)?;
            }
            other
                .simple_query("INSERT INTO conf_iso_copy (id) VALUES (9)")
                .await
                .map_err(describe)?;
            client.simple_query("ROLLBACK").await.map_err(describe)?;

            let ids = simple_column(&other, "SELECT id FROM conf_iso_copy").await?;
            let result = (ids == ["9"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected only the committed row [9]"));
            drop_table(&client, "conf_iso_copy").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a table larger than one scan page is counted in full",
        async {
            drop_table(&client, "conf_many").await;
            client
                .simple_query("CREATE TABLE conf_many (id INTEGER)")
                .await
                .map_err(describe)?;

            // Written in batches so the check costs a second, not a minute.
            const ROWS: usize = 12_000;
            for base in (0..ROWS).step_by(500) {
                let values: Vec<String> = (base..base + 500).map(|id| format!("({id})")).collect();
                client
                    .simple_query(&format!(
                        "INSERT INTO conf_many (id) VALUES {}",
                        values.join(", ")
                    ))
                    .await
                    .map_err(describe)?;
            }

            // The storage layer capped an unlimited scan at its configured
            // page size and reported success, so this answered with the cap.
            let counted = simple_column(&client, "SELECT COUNT(*) FROM conf_many").await?;
            let highest = simple_column(&client, "SELECT MAX(id) FROM conf_many").await?;
            let result = (counted == [ROWS.to_string()] && highest == [(ROWS - 1).to_string()])
                .then_some(())
                .ok_or(format!(
                    "counted {counted:?} with a maximum of {highest:?}, expected {ROWS} rows"
                ));
            drop_table(&client, "conf_many").await;
            result
        }
        .await,
    );

    // ---- PL/pgSQL ------------------------------------------------------
    //
    // `DO` and `CREATE FUNCTION ... LANGUAGE plpgsql` both used to answer
    // "Command completed successfully" and run nothing, so every check here
    // would have passed its statement and failed its effect. Each one asserts
    // what the block *did*, never that it was accepted.

    drop_table(&client, "conf_pl").await;
    let _ = client
        .simple_query("CREATE TABLE conf_pl (id INTEGER)")
        .await;

    for (name, block, expect) in [
        (
            "a DO block runs its statement",
            "DO $$ BEGIN INSERT INTO conf_pl (id) VALUES (1); END $$",
            "1",
        ),
        (
            "a FOR loop runs its body once per value",
            "DO $$ BEGIN FOR i IN 1..4 LOOP INSERT INTO conf_pl (id) VALUES (i); END LOOP; END $$",
            "1,1,2,3,4",
        ),
        (
            "REVERSE counts down without changing the set",
            "DO $$ BEGIN FOR i IN REVERSE 5..6 LOOP INSERT INTO conf_pl (id) VALUES (i); END LOOP; END $$",
            "1,1,2,3,4,5,6",
        ),
        (
            "IF runs only the branch that holds",
            "DO $$ BEGIN IF 1 > 2 THEN INSERT INTO conf_pl (id) VALUES (99); ELSE INSERT INTO conf_pl (id) VALUES (7); END IF; END $$",
            "1,1,2,3,4,5,6,7",
        ),
        (
            "a declared variable is substituted",
            "DO $$ DECLARE n INTEGER := 8; BEGIN INSERT INTO conf_pl (id) VALUES (n); END $$",
            "1,1,2,3,4,5,6,7,8",
        ),
        (
            "a WHILE loop assigns and terminates",
            "DO $$ DECLARE n INTEGER := 8; BEGIN WHILE n < 10 LOOP n := n + 1; INSERT INTO conf_pl (id) VALUES (n); END LOOP; END $$",
            "1,1,2,3,4,5,6,7,8,9,10",
        ),
        (
            "EXIT WHEN leaves the loop early",
            "DO $$ BEGIN FOR i IN 20..99 LOOP EXIT WHEN i > 21; INSERT INTO conf_pl (id) VALUES (i); END LOOP; END $$",
            "1,1,2,3,4,5,6,7,8,9,10,20,21",
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            async {
                client.simple_query(block).await.map_err(describe)?;
                let ids = simple_column(&client, "SELECT id FROM conf_pl ORDER BY id").await?;
                let got = ids.join(",");
                (got == expect)
                    .then_some(())
                    .ok_or(format!("rows are {got}, expected {expect}"))
            }
            .await,
        );
    }

    report.record(
        Area::Sql,
        "RAISE EXCEPTION aborts the block and its writes",
        async {
            let before = simple_column(&client, "SELECT COUNT(*) FROM conf_pl").await?;
            let outcome = client
                .simple_query(
                    "DO $$ BEGIN INSERT INTO conf_pl (id) VALUES (555); RAISE EXCEPTION 'refused'; END $$",
                )
                .await;
            let message = match outcome {
                Ok(_) => return Err("the block was accepted; RAISE EXCEPTION did nothing".into()),
                Err(e) => describe(e),
            };
            if !message.contains("refused") {
                return Err(format!("aborted with {message}, expected the raised message"));
            }
            // The negative control: the statement before the RAISE must not
            // survive, or the abort is only cosmetic.
            let after = simple_column(&client, "SELECT COUNT(*) FROM conf_pl").await?;
            (after == before)
                .then_some(())
                .ok_or(format!("row count went {before:?} -> {after:?} despite the abort"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "SELECT ... INTO binds a variable",
        async {
            client
                .simple_query(
                    "DO $$ DECLARE c INTEGER; BEGIN SELECT COUNT(*) FROM conf_pl INTO c; INSERT INTO conf_pl (id) VALUES (1000 + c); END $$",
                )
                .await
                .map_err(describe)?;
            // 13 rows were present, so the block must have written 1013.
            let found = simple_column(&client, "SELECT id FROM conf_pl WHERE id > 1000").await?;
            (found == ["1013"])
                .then_some(())
                .ok_or(format!("got {found:?}, expected [1013] from a bound COUNT"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a plpgsql function returns a value to its caller",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_double").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_double(x INTEGER) RETURNS INTEGER AS $$ BEGIN RETURN x * 2; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let doubled = simple_column(&client, "SELECT conf_double(21)").await?;
            (doubled == ["42"])
                .then_some(())
                .ok_or(format!("got {doubled:?}, expected [42]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a function body with a branch is interpreted, not just run",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_sign").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_sign(x INTEGER) RETURNS TEXT AS $$ BEGIN IF x > 0 THEN RETURN 'positive'; ELSIF x < 0 THEN RETURN 'negative'; ELSE RETURN 'zero'; END IF; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let mut seen = Vec::new();
            for argument in ["5", "-5", "0"] {
                let answer =
                    simple_column(&client, &format!("SELECT conf_sign({argument})")).await?;
                seen.push(answer.join(""));
            }
            (seen == ["positive", "negative", "zero"])
                .then_some(())
                .ok_or(format!("got {seen:?}, expected each branch in turn"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a malformed block is refused rather than silently accepted",
        async {
            // The failure this guards against is the original one: a block
            // that cannot be run reporting success.
            match client
                .simple_query("DO $$ BEGIN IF 1 > 0 THEN NULL; END $$")
                .await
            {
                Ok(_) => Err("an IF with no END IF was accepted".into()),
                Err(_) => Ok(()),
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an expression in INSERT ... VALUES is evaluated, not stored as text",
        async {
            drop_table(&client, "conf_expr").await;
            client
                .simple_query("CREATE TABLE conf_expr (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_expr (id) VALUES (500 + 1)")
                .await
                .map_err(describe)?;
            let stored = simple_column(&client, "SELECT id FROM conf_expr").await?;
            if stored != ["501"] {
                drop_table(&client, "conf_expr").await;
                return Err(format!("stored {stored:?}, expected [501]"));
            }
            // Storing the text made every later comparison fail, which is how
            // it stayed invisible: the row was there, it just never matched.
            let matched =
                simple_column(&client, "SELECT COUNT(*) FROM conf_expr WHERE id > 500").await?;
            let result = (matched == ["1"])
                .then_some(())
                .ok_or(format!("comparison matched {matched:?}, expected [1]"));
            drop_table(&client, "conf_expr").await;
            result
        }
        .await,
    );

    // Everything above runs a block for its effect on one table. These check
    // the constructs that read or return rows, and the one property a handler
    // exists for: that what it catches left nothing behind.

    report.record(
        Area::Sql,
        "a TEXT argument reaches a function as a string",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_greet").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_greet(who TEXT) RETURNS TEXT AS $$ BEGIN RETURN 'hello ' || who; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // Substituting it unquoted made this `'hello ' || world`, which
            // failed with `column "world" does not exist`.
            let greeting = simple_column(&client, "SELECT conf_greet('world')").await?;
            (greeting == ["hello world"])
                .then_some(())
                .ok_or(format!("got {greeting:?}, expected [hello world]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "FOR ... IN SELECT reads each row's columns",
        async {
            drop_table(&client, "conf_src").await;
            drop_table(&client, "conf_dst").await;
            client
                .simple_query("CREATE TABLE conf_src (id INTEGER, name TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_dst (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_src (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "DO $$ BEGIN FOR r IN SELECT id, name FROM conf_src LOOP INSERT INTO conf_dst (id) VALUES (r.id * 10); END LOOP; END $$",
                )
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_dst ORDER BY id").await?;
            (ids == ["10", "20", "30"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [10, 20, 30]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "RETURN QUERY makes a function set-returning",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_big").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_big() RETURNS SETOF INTEGER AS $$ BEGIN RETURN QUERY SELECT id FROM conf_src WHERE id > 1; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT conf_big()").await?;
            (ids == ["2", "3"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected two rows [2, 3]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an EXCEPTION handler catches and the block carries on",
        async {
            client
                .simple_query(
                    "DO $$ BEGIN BEGIN RAISE EXCEPTION 'boom'; EXCEPTION WHEN OTHERS THEN INSERT INTO conf_dst (id) VALUES (777); END; END $$",
                )
                .await
                .map_err(describe)?;
            let found = simple_column(&client, "SELECT id FROM conf_dst WHERE id = 777").await?;
            (found == ["777"])
                .then_some(())
                .ok_or("the handler did not run".to_string())
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a caught exception undoes what the protected block wrote",
        async {
            client
                .simple_query(
                    "DO $$ BEGIN BEGIN INSERT INTO conf_dst (id) VALUES (888); RAISE EXCEPTION 'x'; EXCEPTION WHEN OTHERS THEN NULL; END; END $$",
                )
                .await
                .map_err(describe)?;
            // Catching without undoing would leave a half-finished write,
            // which is the thing a handler exists to prevent.
            let left = simple_column(&client, "SELECT COUNT(*) FROM conf_dst WHERE id = 888")
                .await?;
            (left == ["0"])
                .then_some(())
                .ok_or(format!("{left:?} rows survived a caught exception"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "SQLERRM carries the raised message",
        async {
            client
                .simple_query(
                    "DO $$ BEGIN BEGIN RAISE EXCEPTION 'the reason'; EXCEPTION WHEN OTHERS THEN INSERT INTO conf_src (id, name) VALUES (99, SQLERRM); END; END $$",
                )
                .await
                .map_err(describe)?;
            let message = simple_column(&client, "SELECT name FROM conf_src WHERE id = 99").await?;
            (message == ["the reason"])
                .then_some(())
                .ok_or(format!("got {message:?}, expected [the reason]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an unhandled exception still reaches the client",
        async {
            match client
                .simple_query("DO $$ BEGIN BEGIN RAISE EXCEPTION 'unhandled'; END; END $$")
                .await
            {
                Ok(_) => Err("a block with no handler swallowed its exception".into()),
                Err(e) => {
                    let message = describe(e);
                    message.contains("unhandled").then_some(()).ok_or(format!(
                        "failed with {message}, expected the raised message"
                    ))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "RETURN NEXT appends one value at a time",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_next").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_next() RETURNS SETOF INTEGER AS $$ BEGIN RETURN NEXT 10; RETURN NEXT 20; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let values = simple_column(&client, "SELECT conf_next()").await?;
            (values == ["10", "20"])
                .then_some(())
                .ok_or(format!("got {values:?}, expected [10, 20]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a cursor is opened, fetched from and closed",
        async {
            client
                .simple_query("DELETE FROM conf_dst")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "DO $$ DECLARE c CURSOR FOR SELECT id FROM conf_src ORDER BY id; v INTEGER; BEGIN OPEN c; FETCH c INTO v; INSERT INTO conf_dst (id) VALUES (v); FETCH c INTO v; INSERT INTO conf_dst (id) VALUES (v); CLOSE c; END $$",
                )
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_dst ORDER BY id").await?;
            (ids == ["1", "2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected the first two rows [1, 2]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "FOUND reports whether a FETCH returned a row",
        async {
            client
                .simple_query("DELETE FROM conf_dst")
                .await
                .map_err(describe)?;
            // The loop can only terminate if FOUND goes false at the end, and
            // it can only run at all if FOUND is usable in a condition —
            // spelled `t` rather than `TRUE` it failed with
            // `column "t" does not exist`.
            client
                .simple_query(
                    "DO $$ DECLARE c CURSOR FOR SELECT id FROM conf_src WHERE id < 10; v INTEGER; BEGIN OPEN c; LOOP FETCH c INTO v; EXIT WHEN NOT FOUND; INSERT INTO conf_dst (id) VALUES (v); END LOOP; CLOSE c; END $$",
                )
                .await
                .map_err(describe)?;
            let counted = simple_column(&client, "SELECT COUNT(*) FROM conf_dst").await?;
            (counted == ["3"])
                .then_some(())
                .ok_or(format!("got {counted:?} rows, expected all 3"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a cursor FOR loop walks every row",
        async {
            client
                .simple_query("DELETE FROM conf_dst")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "DO $$ DECLARE c CURSOR FOR SELECT id FROM conf_src WHERE id < 10; BEGIN FOR r IN c LOOP INSERT INTO conf_dst (id) VALUES (r.id); END LOOP; END $$",
                )
                .await
                .map_err(describe)?;
            let counted = simple_column(&client, "SELECT COUNT(*) FROM conf_dst").await?;
            (counted == ["3"])
                .then_some(())
                .ok_or(format!("got {counted:?} rows, expected 3"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a %TYPE variable takes the column's type",
        async {
            // `name` is TEXT, so the value must be quoted when substituted.
            // Taking it as untyped would paste it in bare and fail.
            client
                .simple_query(
                    "DO $$ DECLARE n conf_src.name%TYPE; BEGIN n := 'from a domain'; INSERT INTO conf_src (id, name) VALUES (50, n); END $$",
                )
                .await
                .map_err(describe)?;
            let found = simple_column(&client, "SELECT name FROM conf_src WHERE id = 50").await?;
            (found == ["from a domain"])
                .then_some(())
                .ok_or(format!("got {found:?}, expected [from a domain]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "two functions of one name are told apart by argument count",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_over").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_over(a INTEGER) RETURNS INTEGER AS $$ BEGIN RETURN a; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE FUNCTION conf_over(a INTEGER, b INTEGER) RETURNS INTEGER AS $$ BEGIN RETURN a + b; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let one = simple_column(&client, "SELECT conf_over(5)").await?;
            let two = simple_column(&client, "SELECT conf_over(5, 6)").await?;
            // Defining the second must not have replaced the first.
            (one == ["5"] && two == ["11"])
                .then_some(())
                .ok_or(format!("got {one:?} and {two:?}, expected [5] and [11]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "DROP FUNCTION removes every arity of the name",
        async {
            client
                .simple_query("DROP FUNCTION conf_over")
                .await
                .map_err(describe)?;
            match client.simple_query("SELECT conf_over(5)").await {
                Ok(_) => Err("the one-argument form survived the drop".into()),
                Err(_) => match client.simple_query("SELECT conf_over(5, 6)").await {
                    Ok(_) => Err("the two-argument form survived the drop".into()),
                    Err(_) => Ok(()),
                },
            }
        }
        .await,
    );

    // ---- SQLSTATE ------------------------------------------------------
    //
    // Every error used to leave as XX000, so a driver could not tell a
    // duplicate key from a crashed backend. These trigger each condition
    // through the real code path and assert the code a client receives —
    // which is also what keeps the classifier from drifting as messages are
    // reworded.

    drop_table(&client, "conf_state").await;
    drop_table(&client, "conf_state_child").await;
    let _ = client
        .simple_query(
            "CREATE TABLE conf_state (id INTEGER PRIMARY KEY, n TEXT NOT NULL, amt INTEGER CHECK (amt > 0))",
        )
        .await;
    let _ = client
        .simple_query(
            "CREATE TABLE conf_state_child (id INTEGER, pid INTEGER REFERENCES conf_state(id))",
        )
        .await;
    let _ = client
        .simple_query("INSERT INTO conf_state (id, n, amt) VALUES (1, 'a', 5)")
        .await;

    for (name, sql, want) in [
        (
            "undefined_table is 42P01",
            "SELECT * FROM conf_no_such_table",
            "42P01",
        ),
        (
            "undefined_column is 42703",
            "SELECT no_such_column FROM conf_state",
            "42703",
        ),
        (
            "duplicate_table is 42P07",
            "CREATE TABLE conf_state (id INTEGER)",
            "42P07",
        ),
        (
            "unique_violation is 23505",
            "INSERT INTO conf_state (id, n, amt) VALUES (1, 'b', 5)",
            "23505",
        ),
        (
            "not_null_violation is 23502",
            "INSERT INTO conf_state (id, amt) VALUES (2, 5)",
            "23502",
        ),
        (
            "check_violation is 23514",
            "INSERT INTO conf_state (id, n, amt) VALUES (3, 'c', -1)",
            "23514",
        ),
        (
            "foreign_key_violation is 23503",
            "INSERT INTO conf_state_child (id, pid) VALUES (1, 999)",
            "23503",
        ),
        ("division_by_zero is 22012", "SELECT 1/0", "22012"),
        (
            "undefined_function is 42883",
            "SELECT conf_no_such_function(1)",
            "42883",
        ),
        (
            "raise_exception is P0001",
            "DO $$ BEGIN RAISE EXCEPTION 'raised'; END $$",
            "P0001",
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            async {
                match client.simple_query(sql).await {
                    Ok(_) => Err(format!(
                        "{sql} was accepted; expected it to fail with {want}"
                    )),
                    Err(e) => {
                        let got = sqlstate(&e);
                        (got == want)
                            .then_some(())
                            .ok_or(format!("reported {got}, expected {want}"))
                    }
                }
            }
            .await,
        );
    }

    report.record(
        Area::Sql,
        "an error message does not carry the transport's name",
        async {
            match client
                .simple_query("SELECT * FROM conf_no_such_table")
                .await
            {
                Ok(_) => Err("the missing table was accepted".into()),
                Err(e) => {
                    let message = describe(e);
                    // `PostgreSQL protocol error: ...` is our plumbing showing
                    // through; a client sees the condition, not the pipe.
                    (!message.contains("protocol error"))
                        .then_some(())
                        .ok_or(format!("message leaked the transport: {message}"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a named EXCEPTION condition catches only its own failure",
        async {
                drop_table(&client, "conf_caught").await;
            client
                .simple_query("CREATE TABLE conf_caught (what TEXT)")
                .await
                .map_err(describe)?;

            // Catches: the condition names what actually happened.
            client
                .simple_query(
                    "DO $$ BEGIN BEGIN INSERT INTO conf_state (id, n, amt) VALUES (1, 'x', 5); EXCEPTION WHEN unique_violation THEN INSERT INTO conf_caught (what) VALUES ('caught'); END; END $$",
                )
                .await
                .map_err(describe)?;
            let caught = simple_column(&client, "SELECT what FROM conf_caught").await?;
            if caught != ["caught"] {
                return Err(format!("WHEN unique_violation did not catch: {caught:?}"));
            }

            // Does not catch: a different condition must let it through, or
            // the matching is just `OTHERS` wearing a name.
            match client
                .simple_query(
                    "DO $$ BEGIN BEGIN INSERT INTO conf_state (id, n, amt) VALUES (1, 'y', 5); EXCEPTION WHEN division_by_zero THEN NULL; END; END $$",
                )
                .await
            {
                Ok(_) => Err("WHEN division_by_zero swallowed a unique violation".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    (got == "23505")
                        .then_some(())
                        .ok_or(format!("escaped with {got}, expected 23505"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "OUT parameters are the function's result",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_out").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_out(a INTEGER, OUT dbl INTEGER, OUT trp INTEGER) AS $$ BEGIN dbl := a * 2; trp := a * 3; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // Two values, named after the parameters — not one anonymous
            // column, and not whatever RETURN would have said.
            let rows = client
                .simple_query("SELECT conf_out(4)")
                .await
                .map_err(describe)?;
            let row = rows
                .iter()
                .find_map(|m| match m {
                    tokio_postgres::SimpleQueryMessage::Row(r) => Some(r),
                    _ => None,
                })
                .ok_or("no row came back")?;
            let got: Vec<String> = (0..row.len())
                .map(|i| row.get(i).unwrap_or("NULL").to_string())
                .collect();
            (got == ["8", "12"])
                .then_some(())
                .ok_or(format!("got {got:?}, expected [8, 12]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "INOUT both takes and returns",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_bump").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_bump(INOUT n INTEGER) AS $$ BEGIN n := n + 1; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let bumped = simple_column(&client, "SELECT conf_bump(41)").await?;
            (bumped == ["42"])
                .then_some(())
                .ok_or(format!("got {bumped:?}, expected [42]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "two functions of one name and arity are told apart by type",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_kind").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_kind(a INTEGER) RETURNS TEXT AS $$ BEGIN RETURN 'number'; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE FUNCTION conf_kind(a TEXT) RETURNS TEXT AS $$ BEGIN RETURN 'text'; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // Same name, same arity: keyed by count alone the second would
            // have replaced the first.
            let number = simple_column(&client, "SELECT conf_kind(5)").await?;
            let text = simple_column(&client, "SELECT conf_kind('x')").await?;
            (number == ["number"] && text == ["text"])
                .then_some(())
                .ok_or(format!("got {number:?} and {text:?}"))
        }
        .await,
    );

    for (name, definitions, call, want) in [
        (
            "INTEGER and BIGINT overloads are told apart",
            &["a INTEGER", "a BIGINT"][..],
            &["SELECT conf_width(42)", "SELECT conf_width(5000000000)"][..],
            &["a INTEGER", "a BIGINT"][..],
        ),
        (
            "VARCHAR and TEXT overloads are told apart",
            &["a VARCHAR", "a TEXT"][..],
            &["SELECT conf_width('x')"][..],
            // An untyped literal fits both; PostgreSQL prefers text.
            &["a TEXT"][..],
        ),
        (
            "a call widens to the only candidate that fits",
            &["a SMALLINT", "a BIGINT"][..],
            // int4 cannot narrow to int2, so only the BIGINT form is viable.
            &["SELECT conf_width(42)"][..],
            &["a BIGINT"][..],
        ),
        (
            "an exact type match beats an implicit conversion",
            &["a NUMERIC", "a INTEGER"][..],
            &["SELECT conf_width(7)"][..],
            &["a INTEGER"][..],
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            async {
                let _ = client.simple_query("DROP FUNCTION conf_width").await;
                for declaration in definitions {
                    client
                        .simple_query(&format!(
                            "CREATE FUNCTION conf_width({declaration}) RETURNS TEXT AS $$ BEGIN RETURN '{declaration}'; END $$ LANGUAGE plpgsql"
                        ))
                        .await
                        .map_err(describe)?;
                }
                for (call, want) in call.iter().zip(want) {
                    let got = simple_column(&client, call).await?;
                    if got != [(*want).to_string()] {
                        return Err(format!("{call} chose {got:?}, expected [{want}]"));
                    }
                }
                Ok(())
            }
            .await,
        );
    }

    report.record(
        Area::Catalog,
        "pg_proc lists a stored function with its types",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_proc").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_proc(a INTEGER) RETURNS TEXT AS $$ BEGIN RETURN 'x'; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // 23 is int4 and 25 is text: the argument and return types a
            // client reads to know how to call it.
            let described = simple_column(
                &client,
                "SELECT pronargs FROM pg_proc WHERE proname = 'conf_proc'",
            )
            .await?;
            if described != ["1"] {
                return Err(format!("pronargs is {described:?}, expected [1]"));
            }
            let argument_types = simple_column(
                &client,
                "SELECT proargtypes FROM pg_proc WHERE proname = 'conf_proc'",
            )
            .await?;
            if argument_types != ["23"] {
                return Err(format!("proargtypes is {argument_types:?}, expected [23]"));
            }
            let return_type = simple_column(
                &client,
                "SELECT prorettype FROM pg_proc WHERE proname = 'conf_proc'",
            )
            .await?;
            (return_type == ["25"])
                .then_some(())
                .ok_or(format!("prorettype is {return_type:?}, expected [25]"))
        }
        .await,
    );

    report.record(
        Area::Catalog,
        "a function's OID is in the user range and does not move",
        async {
            let before =
                simple_column(&client, "SELECT oid FROM pg_proc WHERE proname = 'conf_proc'")
                    .await?;
            let oid: i64 = before
                .first()
                .and_then(|value| value.parse().ok())
                .ok_or("no OID reported")?;
            // PostgreSQL reserves everything below 16384 for built-in objects.
            if oid < 16_384 {
                return Err(format!("OID {oid} is in the reserved range"));
            }

            // Creating another function must not shift it: an OID that moved
            // would make pg_proc useless for the thing OIDs are for.
            let _ = client.simple_query("DROP FUNCTION conf_proc_other").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_proc_other() RETURNS INTEGER AS $$ BEGIN RETURN 1; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            let after =
                simple_column(&client, "SELECT oid FROM pg_proc WHERE proname = 'conf_proc'")
                    .await?;
            (after == before)
                .then_some(())
                .ok_or(format!("OID moved from {before:?} to {after:?}"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a cast names an argument's type, overriding its value",
        async {
            for name in ["conf_kindof", "conf_ret8"] {
                let _ = client.simple_query(&format!("DROP FUNCTION {name}")).await;
            }
            for (declaration, answer) in [
                ("a INTEGER", "int4"),
                ("a BIGINT", "int8"),
                ("a TEXT", "text"),
            ] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_kindof({declaration}) RETURNS TEXT AS $$ BEGIN RETURN '{answer}'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }

            // 42 fits an int4, so its value says int4; the cast says
            // otherwise and the cast is what a caller wrote down.
            let plain = simple_column(&client, "SELECT conf_kindof(42)").await?;
            let cast = simple_column(&client, "SELECT conf_kindof(42::BIGINT)").await?;
            let spelled = simple_column(&client, "SELECT conf_kindof(CAST(42 AS BIGINT))").await?;
            let textual = simple_column(&client, "SELECT conf_kindof(42::TEXT)").await?;
            (plain == ["int4"] && cast == ["int8"] && spelled == ["int8"] && textual == ["text"])
                .then_some(())
                .ok_or(format!(
                    "got {plain:?}, {cast:?}, {spelled:?}, {textual:?}; expected int4, int8, int8, text"
                ))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a nested call is typed from the catalogue, not its value",
        async {
            client
                .simple_query(
                    "CREATE FUNCTION conf_ret8() RETURNS BIGINT AS $$ BEGIN RETURN 1; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // The value 1 fits an int4; only the catalogue knows the function
            // was declared to return int8.
            let chosen = simple_column(&client, "SELECT conf_kindof(conf_ret8())").await?;
            (chosen == ["int8"])
                .then_some(())
                .ok_or(format!("got {chosen:?}, expected [int8]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a domain parameter is the type it is built on",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_dom").await;
            let _ = client
                .simple_query("CREATE DOMAIN conf_posint AS INTEGER CHECK (VALUE > 0)")
                .await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_dom(a conf_posint) RETURNS INTEGER AS $$ BEGIN RETURN a * 2; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // Bound under the domain's own name the value was quoted, and
            // `a * 2` failed as arithmetic on text.
            let doubled = simple_column(&client, "SELECT conf_dom(5)").await?;
            if doubled != ["10"] {
                return Err(format!("got {doubled:?}, expected [10]"));
            }
            // And the catalogue reports what a caller must pass, not `text`.
            let reported = simple_column(
                &client,
                "SELECT proargtypes FROM pg_proc WHERE proname = 'conf_dom'",
            )
            .await?;
            (reported == ["23"])
                .then_some(())
                .ok_or(format!("proargtypes is {reported:?}, expected [23]"))
        }
        .await,
    );

    for (name, sql, want) in [
        ("= ANY over an array", "SELECT 1 = ANY(ARRAY[1,2])", "t"),
        (
            "= ANY that does not match",
            "SELECT 5 = ANY(ARRAY[1,2])",
            "f",
        ),
        ("= ALL over an array", "SELECT 1 = ALL(ARRAY[1,1])", "t"),
        (
            "= ALL that does not hold",
            "SELECT 1 = ALL(ARRAY[1,2])",
            "f",
        ),
        // The `<`/`>` forms already worked; `=` is the one people write.
        ("> ANY still works", "SELECT 2 > ANY(ARRAY[1,5])", "t"),
    ] {
        report.record(
            Area::Sql,
            name,
            async {
                let got = simple_column(&client, sql).await?;
                (got == [want.to_string()])
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected [{want}]"))
            }
            .await,
        );
    }

    report.record(
        Area::Sql,
        "a quantified comparison filters rows",
        async {
            drop_table(&client, "conf_any").await;
            client
                .simple_query("CREATE TABLE conf_any (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_any (id) VALUES (1), (2), (3)")
                .await
                .map_err(describe)?;
            // Stored as the text `ANY(ARRAY[1,3])` this matched nothing —
            // a wrong answer rather than an error.
            let ids = simple_column(
                &client,
                "SELECT id FROM conf_any WHERE id = ANY(ARRAY[1,3]) ORDER BY id",
            )
            .await?;
            let result = (ids == ["1", "3"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [1, 3]"));
            drop_table(&client, "conf_any").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an expression on the right of a comparison is evaluated",
        async {
            drop_table(&client, "conf_rhs").await;
            client
                .simple_query("CREATE TABLE conf_rhs (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_rhs (id) VALUES (1), (2), (3)")
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_rhs WHERE id = 1 + 1").await?;
            let result = (ids == ["2"])
                .then_some(())
                .ok_or(format!("got {ids:?}, expected [2]"));
            drop_table(&client, "conf_rhs").await;
            result
        }
        .await,
    );

    for (name, sql, want) in [
        // These were parse errors: the types were reachable as column
        // declarations but missing from the cast-target list, so a type you
        // could declare was not a type you could cast to.
        ("cast to NUMERIC", "SELECT '1.5'::NUMERIC", "1.5"),
        (
            "a declared scale is rendered",
            "SELECT 1.5::NUMERIC(10,2)",
            "1.50",
        ),
        (
            "a declared scale rounds",
            "SELECT '1.567'::NUMERIC(10,2)",
            "1.57",
        ),
        (
            "an integer takes the scale",
            "SELECT 3::NUMERIC(10,2)",
            "3.00",
        ),
        (
            "cast to json then read a key",
            "SELECT ('{\"a\":1}'::json)->>'a'",
            "1",
        ),
        (
            "cast to jsonb then read a key",
            "SELECT '{\"a\":1}'::jsonb->>'a'",
            "1",
        ),
    ] {
        report.record(
            Area::Types,
            name,
            async {
                let got = simple_column(&client, sql).await?;
                (got == [want.to_string()])
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected [{want}]"))
            }
            .await,
        );
    }

    report.record(
        Area::Sql,
        "UPDATE with an expression applies, and RETURNING agrees with storage",
        async {
            drop_table(&client, "conf_upd").await;
            client
                .simple_query(
                    "CREATE TABLE conf_upd (id INTEGER, n INTEGER, amt NUMERIC(10,2), t TEXT)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_upd (id, n, amt, t) VALUES (1, 5, 10.00, 'a'), (2, 7, 20.00, 'b')",
                )
                .await
                .map_err(describe)?;

            // `SET n = n + 1` changed nothing while `RETURNING` reported the
            // new value: a client was told a write had happened that had not.
            client
                .simple_query("UPDATE conf_upd SET n = n + 1 WHERE id = 1")
                .await
                .map_err(describe)?;
            let counted = simple_column(&client, "SELECT n FROM conf_upd WHERE id = 1").await?;
            if counted != ["6"] {
                return Err(format!("n is {counted:?}, expected [6]"));
            }

            // The same for an exact decimal, which must stay exact.
            client
                .simple_query("UPDATE conf_upd SET amt = amt * 2 WHERE id = 2")
                .await
                .map_err(describe)?;
            let doubled = simple_column(&client, "SELECT amt FROM conf_upd WHERE id = 2").await?;
            if doubled != ["40.00"] {
                return Err(format!("amt is {doubled:?}, expected [40.00]"));
            }

            // And what RETURNING says must be what was stored.
            let returned =
                simple_column(&client, "UPDATE conf_upd SET n = n + 100 WHERE id = 1 RETURNING n")
                    .await?;
            let stored = simple_column(&client, "SELECT n FROM conf_upd WHERE id = 1").await?;
            (returned == stored)
                .then_some(())
                .ok_or(format!("RETURNING said {returned:?}, storage holds {stored:?}"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a quoted SET value stays text, however it is spelled",
        async {
            // The quotes are what say this is a literal. Stripped before the
            // update saw them, `'a + b'` was indistinguishable from an
            // expression — and `'it''s'` kept its doubled quote.
            client
                .simple_query("UPDATE conf_upd SET t = 'a + b' WHERE id = 1")
                .await
                .map_err(describe)?;
            let text = simple_column(&client, "SELECT t FROM conf_upd WHERE id = 1").await?;
            if text != ["a + b"] {
                return Err(format!("got {text:?}, expected [a + b]"));
            }
            client
                .simple_query("UPDATE conf_upd SET t = 'it''s' WHERE id = 2")
                .await
                .map_err(describe)?;
            let escaped = simple_column(&client, "SELECT t FROM conf_upd WHERE id = 2").await?;
            (escaped == ["it's"])
                .then_some(())
                .ok_or(format!("got {escaped:?}, expected [it's]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a failing UPDATE expression changes nothing",
        async {
            let before = simple_column(&client, "SELECT n FROM conf_upd ORDER BY id").await?;
            // Computed after the old rows were marked deleted, a failure left
            // the row marked and no new version written — it destroyed the row
            // rather than merely failing.
            if client
                .simple_query("UPDATE conf_upd SET n = conf_no_such_column + 1")
                .await
                .is_ok()
            {
                return Err("an unknown column in SET was accepted".into());
            }
            let after = simple_column(&client, "SELECT n FROM conf_upd ORDER BY id").await?;
            let result = (after == before).then_some(()).ok_or(format!(
                "rows went {before:?} -> {after:?} after a failed update"
            ));
            drop_table(&client, "conf_upd").await;
            result
        }
        .await,
    );

    // ---- Parameters, unspecified type ---------------------------------
    //
    // A driver may leave a parameter's type to the server. Every such
    // parameter was filled in as text, so `WHERE id = $1` compared an integer
    // column against `'2'` and matched nothing. The harness reaches this
    // through `query`, which uses Parse/Bind/Execute rather than a simple
    // query.

    report.record(
        Area::ExtendedQuery,
        "a parameter's type is inferred from what it is compared against",
        async {
            drop_table(&client, "conf_param").await;
            client
                .simple_query(
                    "CREATE TABLE conf_param (id INTEGER, name TEXT, amt NUMERIC(10,2), flag BOOLEAN)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_param (id, name, amt, flag) VALUES (1, 'ada', 10.50, TRUE), (2, 'grace', 20.00, FALSE)",
                )
                .await
                .map_err(describe)?;

            // Each of these is a different column type, and each was matching
            // nothing when the parameter defaulted to text.
            let by_int = client
                .query("SELECT name FROM conf_param WHERE id = $1", &[&2i32])
                .await
                .map_err(describe)?;
            if by_int.len() != 1 {
                return Err(format!("integer parameter matched {} rows", by_int.len()));
            }
            let by_text = client
                .query("SELECT id FROM conf_param WHERE name = $1", &[&"ada"])
                .await
                .map_err(describe)?;
            if by_text.len() != 1 {
                return Err(format!("text parameter matched {} rows", by_text.len()));
            }
            let by_bool = client
                .query("SELECT id FROM conf_param WHERE flag = $1", &[&true])
                .await
                .map_err(describe)?;
            (by_bool.len() == 1)
                .then_some(())
                .ok_or(format!("boolean parameter matched {} rows", by_bool.len()))
        }
        .await,
    );

    report.record(
        Area::ExtendedQuery,
        "two parameters in one statement each get their own type",
        async {
            // With both forced to text this failed outright:
            // `Cannot compare Integer(1) and Text("1")`.
            let rows = client
                .query(
                    "SELECT id FROM conf_param WHERE id = $1 AND name = $2",
                    &[&1i32, &"ada"],
                )
                .await
                .map_err(describe)?;
            (rows.len() == 1)
                .then_some(())
                .ok_or(format!("matched {} rows, expected 1", rows.len()))
        }
        .await,
    );

    report.record(
        Area::ExtendedQuery,
        "a parameter in LIMIT is a row count, not text",
        async {
            // Spliced in quoted, the clause was ignored and every row came
            // back.
            let rows = client
                .query("SELECT id FROM conf_param ORDER BY id LIMIT $1", &[&1i64])
                .await
                .map_err(describe)?;
            (rows.len() == 1)
                .then_some(())
                .ok_or(format!("LIMIT $1 returned {} rows, expected 1", rows.len()))
        }
        .await,
    );

    report.record(
        Area::ExtendedQuery,
        "a parameterised UPDATE applies",
        async {
            // This reported success and changed nothing, because the WHERE
            // matched no row.
            client
                .execute(
                    "UPDATE conf_param SET name = $1 WHERE id = $2",
                    &[&"ADA", &1i32],
                )
                .await
                .map_err(describe)?;
            let names = simple_column(&client, "SELECT name FROM conf_param ORDER BY id").await?;
            let result = (names == ["ADA", "grace"])
                .then_some(())
                .ok_or(format!("names are {names:?}, expected [ADA, grace]"));
            drop_table(&client, "conf_param").await;
            result
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a statement in a failed transaction reports in_failed_sql_transaction",
        async {
            let other = connect().await?;
            other.simple_query("BEGIN").await.map_err(describe)?;
            // Put the block into the failed state.
            let _ = other
                .simple_query("SELECT conf_no_such_column_at_all")
                .await;

            // `25P02` is how a driver knows it must roll back rather than
            // retry. Reported as `XX000` it was indistinguishable from the
            // backend falling over.
            match other.simple_query("SELECT 1").await {
                Ok(_) => Err("a statement in a failed block was accepted".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    let _ = other.simple_query("ROLLBACK").await;
                    (got == "25P02")
                        .then_some(())
                        .ok_or(format!("reported {got}, expected 25P02"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Transactions,
        "a transaction block sent as one message leaves no transaction open",
        async {
            drop_table(&client, "conf_block").await;
            client
                .simple_query("CREATE TABLE conf_block (id INTEGER)")
                .await
                .map_err(describe)?;

            // The session's state was read from the first word of the whole
            // message, so the trailing COMMIT went unnoticed and the
            // connection was left holding a transaction the client had
            // already ended.
            client
                .simple_query("BEGIN; INSERT INTO conf_block (id) VALUES (9); COMMIT")
                .await
                .map_err(describe)?;

            // If a transaction were still open, this would be inside it, and
            // the rollback would discard it.
            client
                .simple_query("INSERT INTO conf_block (id) VALUES (10)")
                .await
                .map_err(describe)?;
            let _ = client.simple_query("ROLLBACK").await;

            let ids = simple_column(&client, "SELECT id FROM conf_block ORDER BY id").await?;
            let result = (ids == ["9", "10"])
                .then_some(())
                .ok_or(format!("rows are {ids:?}, expected [9, 10]"));
            drop_table(&client, "conf_block").await;
            result
        }
        .await,
    );

    report.record(
        Area::ExtendedQuery,
        "an empty statement is answered, not rejected",
        async {
            // `EmptyQueryResponse` is how a client tells "nothing to run" from
            // "your statement was rejected". The simple-query path answered
            // it; the extended path reported a parse error.
            let rows = client.query("", &[]).await.map_err(describe)?;
            rows.is_empty()
                .then_some(())
                .ok_or_else(|| format!("an empty query returned {} rows", rows.len()))
        }
        .await,
    );

    report.record(
        Area::Portals,
        "a row-limited Execute suspends and resumes",
        async {
            drop_table(&client, "conf_portal").await;
            client
                .simple_query("CREATE TABLE conf_portal (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_portal (id) VALUES (1), (2), (3), (4)")
                .await
                .map_err(describe)?;

            // Its own connection: a transaction needs a mutable client and
            // this one is borrowed immutably here.
            let mut owned = connect().await?;
            let transaction = owned.transaction().await.map_err(describe)?;
            let statement = transaction
                .prepare("SELECT id FROM conf_portal ORDER BY id")
                .await
                .map_err(describe)?;
            let portal = transaction.bind(&statement, &[]).await.map_err(describe)?;

            // Two now, two later: the second page must continue rather than
            // restart, and must not report the set ended early.
            let first = transaction
                .query_portal(&portal, 2)
                .await
                .map_err(describe)?;
            let second = transaction
                .query_portal(&portal, 0)
                .await
                .map_err(describe)?;
            let seen: Vec<i32> = first
                .iter()
                .chain(second.iter())
                .map(|row| row.get::<_, i32>(0))
                .collect();
            transaction.rollback().await.map_err(describe)?;

            let result = (first.len() == 2 && seen == [1, 2, 3, 4])
                .then_some(())
                .ok_or(format!(
                    "first page {} rows, all rows {seen:?}",
                    first.len()
                ));
            drop_table(&client, "conf_portal").await;
            result
        }
        .await,
    );

    for (name, sql, want) in [
        // `position(sub IN str)` and `strpos(str, sub)` are the same function
        // with opposite argument orders. Sharing one implementation made
        // `strpos('abc','b')` search "abc" inside "b" and answer 0.
        (
            "STRPOS takes the string first",
            "SELECT STRPOS('abc', 'b')",
            "2",
        ),
        (
            "STRPOS reports absence as 0",
            "SELECT STRPOS('abc', 'z')",
            "0",
        ),
        (
            "POSITION takes the needle first",
            "SELECT POSITION('b', 'abc')",
            "2",
        ),
        // The standard spelling. Parsed at the usual level the comparison
        // rules took `sub IN str` first and built an IN expression, leaving
        // the call malformed.
        (
            "POSITION ... IN parses",
            "SELECT POSITION('b' IN 'abc')",
            "2",
        ),
        (
            "POSITION ... IN reports absence",
            "SELECT POSITION('z' IN 'abc')",
            "0",
        ),
        // Character positions, not byte offsets.
        (
            "POSITION ... IN counts characters",
            "SELECT POSITION('語' IN '日本語')",
            "3",
        ),
        // And `IN` keeps its meaning everywhere else.
        (
            "IN is still an operator",
            "SELECT 1 WHERE 2 IN (1, 2, 3)",
            "1",
        ),
        (
            "NOT IN is still an operator",
            "SELECT 1 WHERE 5 NOT IN (1, 2, 3)",
            "1",
        ),
        // An exact average is exact: through f64 the mean of 2, 3 and 5 came
        // back as 3.3333333333333335, whose last digit is a rounding artifact.
        (
            "AVG over integers does not go through a float",
            "SELECT AVG(n) FROM (SELECT 2 AS n UNION ALL SELECT 3 UNION ALL SELECT 5) t",
            "3.3333333333333333333333333333",
        ),
    ] {
        report.record(
            Area::Sql,
            name,
            async {
                let got = simple_column(&client, sql).await?;
                (got == [want.to_string()])
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected [{want}]"))
            }
            .await,
        );
    }

    report.record(
        Area::ExtendedQuery,
        "a failed extended statement leaves the session usable",
        async {
            drop_table(&client, "conf_pipe").await;
            client
                .simple_query("CREATE TABLE conf_pipe (id INTEGER)")
                .await
                .map_err(describe)?;

            // The extended path enters an error state that discards what the
            // client already pipelined, until it synchronises. The discarding
            // itself is checked against the raw protocol; what a driver can
            // show here is that the state is left correctly — the failure is
            // reported and the next statement works.
            if client
                .execute("INSERT INTO conf_pipe (id) VALUES ($1)", &[&"not a number"])
                .await
                .is_ok()
            {
                return Err("an invalid parameter was accepted".into());
            }
            client
                .execute("INSERT INTO conf_pipe (id) VALUES ($1)", &[&7i32])
                .await
                .map_err(describe)?;
            let ids = simple_column(&client, "SELECT id FROM conf_pipe").await?;
            let result = (ids == ["7"])
                .then_some(())
                .ok_or(format!("rows are {ids:?}, expected [7]"));
            drop_table(&client, "conf_pipe").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ALTER TABLE DROP COLUMN and RENAME COLUMN",
        async {
            drop_table(&client, "conf_alter").await;
            client
                .simple_query("CREATE TABLE conf_alter (id INTEGER, extra TEXT, keep TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_alter (id, extra, keep) VALUES (1, 'x', 'y')")
                .await
                .map_err(describe)?;

            client
                .simple_query("ALTER TABLE conf_alter DROP COLUMN extra")
                .await
                .map_err(describe)?;
            if client
                .simple_query("SELECT extra FROM conf_alter")
                .await
                .is_ok()
            {
                return Err("a dropped column was still readable".into());
            }
            // The columns either side of it must survive.
            let kept = simple_column(&client, "SELECT keep FROM conf_alter").await?;
            if kept != ["y"] {
                return Err(format!("keep is {kept:?} after dropping a neighbour"));
            }

            client
                .simple_query("ALTER TABLE conf_alter RENAME COLUMN keep TO kept")
                .await
                .map_err(describe)?;
            let renamed = simple_column(&client, "SELECT kept FROM conf_alter").await?;
            if renamed != ["y"] {
                return Err(format!("renamed column reads {renamed:?}, expected [y]"));
            }
            if client
                .simple_query("SELECT keep FROM conf_alter")
                .await
                .is_ok()
            {
                return Err("the old name still resolves after a rename".into());
            }

            // Dropping something that was never there is an error, not a
            // silent success.
            let result = match client
                .simple_query("ALTER TABLE conf_alter DROP COLUMN conf_no_such_column")
                .await
            {
                Ok(_) => Err("dropping an absent column reported success".to_string()),
                Err(_) => Ok(()),
            };
            drop_table(&client, "conf_alter").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "outer joins keep the rows that found no partner",
        async {
            drop_table(&client, "conf_j1").await;
            drop_table(&client, "conf_j2").await;
            client
                .simple_query("CREATE TABLE conf_j1 (id INTEGER, tag TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE TABLE conf_j2 (id INTEGER, val TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_j1 (id, tag) VALUES (1,'a'),(2,'b'),(3,'c')")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_j2 (id, val) VALUES (2,'two'),(3,'three'),(4,'four')")
                .await
                .map_err(describe)?;

            // Only unmatched *left* rows were kept, so RIGHT JOIN behaved as
            // an inner join and FULL OUTER lost both unmatched sides.
            for (sql, want) in [
                (
                    "SELECT j.id FROM conf_j1 j LEFT JOIN conf_j2 k ON j.id = k.id ORDER BY j.id",
                    vec!["1", "2", "3"],
                ),
                (
                    "SELECT k.id FROM conf_j1 j RIGHT JOIN conf_j2 k ON j.id = k.id ORDER BY k.id",
                    vec!["2", "3", "4"],
                ),
                (
                    "SELECT COUNT(*) FROM conf_j1 FULL OUTER JOIN conf_j2 ON conf_j1.id = conf_j2.id",
                    vec!["4"],
                ),
            ] {
                let got = simple_column(&client, sql).await?;
                if got != want {
                    drop_table(&client, "conf_j1").await;
                    drop_table(&client, "conf_j2").await;
                    return Err(format!("{sql} gave {got:?}, expected {want:?}"));
                }
            }

            // An unmatched outer row must carry the other side's columns as
            // NULL, not lack them: without that a bare column reference
            // failed with `column does not exist` instead of returning NULL.
            let bare = simple_column(
                &client,
                "SELECT val FROM conf_j1 LEFT JOIN conf_j2 ON conf_j1.id = conf_j2.id ORDER BY conf_j1.id",
            )
            .await?;
            let result = (bare == ["NULL", "two", "three"])
                .then_some(())
                .ok_or(format!("got {bare:?}, expected [NULL, two, three]"));
            drop_table(&client, "conf_j1").await;
            drop_table(&client, "conf_j2").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a UNIQUE INDEX is enforced, and dropping it releases the constraint",
        async {
            drop_table(&client, "conf_uniq").await;
            client
                .simple_query("CREATE TABLE conf_uniq (id INTEGER, n INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_uniq (id, n) VALUES (1, 5)")
                .await
                .map_err(describe)?;
            client
                .simple_query("CREATE UNIQUE INDEX conf_uniq_id ON conf_uniq (id)")
                .await
                .map_err(describe)?;

            // Accepted and not enforced, duplicates went in silently — an
            // integrity constraint the caller asked for by name.
            match client
                .simple_query("INSERT INTO conf_uniq (id, n) VALUES (1, 6)")
                .await
            {
                Ok(_) => return Err("a duplicate was accepted under a UNIQUE INDEX".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    if got != "23505" {
                        return Err(format!("reported {got}, expected 23505"));
                    }
                }
            }
            // A distinct value must still be accepted.
            client
                .simple_query("INSERT INTO conf_uniq (id, n) VALUES (2, 6)")
                .await
                .map_err(describe)?;

            // Dropping the index takes the constraint with it.
            client
                .simple_query("DROP INDEX conf_uniq_id")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_uniq (id, n) VALUES (1, 7)")
                .await
                .map_err(describe)?;
            let counted = simple_column(&client, "SELECT COUNT(*) FROM conf_uniq").await?;
            let result = (counted == ["3"])
                .then_some(())
                .ok_or(format!("count is {counted:?}, expected [3]"));
            drop_table(&client, "conf_uniq").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a UNIQUE INDEX over data that already violates it is refused",
        async {
            drop_table(&client, "conf_uniq2").await;
            client
                .simple_query("CREATE TABLE conf_uniq2 (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_uniq2 (id) VALUES (1), (1)")
                .await
                .map_err(describe)?;
            // Creating it anyway would have the index claim something about
            // the table that is not true.
            let refused = client
                .simple_query("CREATE UNIQUE INDEX conf_uniq2_id ON conf_uniq2 (id)")
                .await
                .is_err();
            drop_table(&client, "conf_uniq2").await;
            refused
                .then_some(())
                .ok_or_else(|| "a unique index was built over duplicate rows".to_string())
        }
        .await,
    );

    report.record(
        Area::Sql,
        "ALTER TABLE ADD COLUMN actually adds the column",
        async {
            drop_table(&client, "conf_add").await;
            client
                .simple_query("CREATE TABLE conf_add (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_add (id) VALUES (1)")
                .await
                .map_err(describe)?;

            // Nothing handled `ADD COLUMN`, so it fell through to a generic
            // "Command completed successfully" and the column was not there —
            // every later reference then failed, pointing at the query rather
            // than at the DDL that never happened.
            client
                .simple_query("ALTER TABLE conf_add ADD COLUMN label TEXT")
                .await
                .map_err(describe)?;
            client
                .simple_query("UPDATE conf_add SET label = 'x' WHERE id = 1")
                .await
                .map_err(describe)?;
            let labels = simple_column(&client, "SELECT label FROM conf_add").await?;
            if labels != ["x"] {
                return Err(format!("label is {labels:?}, expected [x]"));
            }

            // A default fills the rows that already exist, or the same table
            // answers two ways depending on when a row arrived.
            client
                .simple_query("ALTER TABLE conf_add ADD COLUMN n INTEGER DEFAULT 7")
                .await
                .map_err(describe)?;
            let backfilled = simple_column(&client, "SELECT n FROM conf_add").await?;
            if backfilled != ["7"] {
                return Err(format!("existing row has n = {backfilled:?}, expected [7]"));
            }

            // Adding the same column twice is an error, not a silent no-op.
            match client
                .simple_query("ALTER TABLE conf_add ADD COLUMN label TEXT")
                .await
            {
                Ok(_) => Err("adding a column twice was accepted".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    drop_table(&client, "conf_add").await;
                    (got == "42701")
                        .then_some(())
                        .ok_or(format!("reported {got}, expected 42701"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a scalar subquery works in the select list, not only in WHERE",
        async {
            drop_table(&client, "conf_sub").await;
            client
                .simple_query("CREATE TABLE conf_sub (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_sub (id) VALUES (1), (2), (3)")
                .await
                .map_err(describe)?;

            // Subqueries were resolved for `WHERE` and `HAVING` only, so the
            // same subquery that filtered correctly failed as unimplemented
            // when it appeared in the select list.
            let counted = simple_column(&client, "SELECT (SELECT COUNT(*) FROM conf_sub)").await?;
            if counted != ["3"] {
                return Err(format!(
                    "bare scalar subquery gave {counted:?}, expected [3]"
                ));
            }
            let with_from = simple_column(
                &client,
                "SELECT (SELECT MAX(id) FROM conf_sub) FROM conf_sub ORDER BY id",
            )
            .await?;
            if with_from != ["3", "3", "3"] {
                return Err(format!("per-row scalar subquery gave {with_from:?}"));
            }
            // A subquery matching nothing is NULL, not an error or a zero.
            let empty =
                simple_column(&client, "SELECT (SELECT id FROM conf_sub WHERE id = 99)").await?;
            let result = (empty == ["NULL"]).then_some(()).ok_or(format!(
                "an empty scalar subquery gave {empty:?}, expected NULL"
            ));
            drop_table(&client, "conf_sub").await;
            result
        }
        .await,
    );

    report.record(
        Area::Types,
        "a NUMERIC column renders at its declared scale",
        async {
            drop_table(&client, "conf_scale").await;
            client
                .simple_query(
                    "CREATE TABLE conf_scale (id INTEGER, amt NUMERIC(10,2), d DOUBLE PRECISION)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_scale (id, amt, d) VALUES (1, 10.5, 7.5), (2, 3, 1.25)",
                )
                .await
                .map_err(describe)?;

            // Every read path must agree: a clause-free select, a simple
            // WHERE and an ORDER BY go through different code.
            for sql in [
                "SELECT amt FROM conf_scale ORDER BY id",
                "SELECT amt FROM conf_scale WHERE id = 1",
            ] {
                let shown = simple_column(&client, sql).await?;
                let expected: Vec<String> = if sql.contains("WHERE") {
                    vec!["10.50".to_string()]
                } else {
                    vec!["10.50".to_string(), "3.00".to_string()]
                };
                if shown != expected {
                    drop_table(&client, "conf_scale").await;
                    return Err(format!("{sql} gave {shown:?}, expected {expected:?}"));
                }
            }
            // And a column without a declared scale must not gain one.
            let floats = simple_column(&client, "SELECT d FROM conf_scale ORDER BY id").await?;
            let result = (floats == ["7.5", "1.25"]).then_some(()).ok_or(format!(
                "a DOUBLE rendered as {floats:?}, expected [7.5, 1.25]"
            ));
            drop_table(&client, "conf_scale").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a comparison on a non-integer column filters, not returns nothing",
        async {
            drop_table(&client, "conf_cmp").await;
            client
                .simple_query(
                    "CREATE TABLE conf_cmp (id INTEGER, amt NUMERIC(10,2), d DOUBLE, t TEXT)",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_cmp (id, amt, d, t) VALUES (1, 10.50, 7.5, 'b'), (2, 3, 1.5, 'a')",
                )
                .await
                .map_err(describe)?;

            // The storage matcher compared only `BigInt` against `BigInt`, so
            // `>` on a numeric, float or text column matched no rows at all —
            // an empty result rather than an error.
            for (sql, want) in [
                ("SELECT id FROM conf_cmp WHERE amt > 5", "1"),
                ("SELECT id FROM conf_cmp WHERE d > 5", "1"),
                ("SELECT id FROM conf_cmp WHERE d < 5", "2"),
                ("SELECT id FROM conf_cmp WHERE t > 'a'", "1"),
                ("SELECT id FROM conf_cmp WHERE id > 1", "2"),
            ] {
                let got = simple_column(&client, sql).await?;
                if got != [want.to_string()] {
                    drop_table(&client, "conf_cmp").await;
                    return Err(format!("{sql} gave {got:?}, expected [{want}]"));
                }
            }
            drop_table(&client, "conf_cmp").await;
            Ok(())
        }
        .await,
    );

    for (name, sql, want) in [
        (
            "date plus interval is a timestamp",
            "SELECT DATE '2024-01-01' + INTERVAL '1 day'",
            "2024-01-02 00:00:00",
        ),
        (
            "date plus an integer is a date",
            "SELECT DATE '2024-01-01' + 1",
            "2024-01-02",
        ),
        (
            "date minus an integer crosses a leap day",
            "SELECT DATE '2024-03-01' - 1",
            "2024-02-29",
        ),
        (
            "date minus date is a count of days",
            "SELECT DATE '2024-03-01' - DATE '2024-02-01'",
            "29",
        ),
    ] {
        report.record(
            Area::Types,
            name,
            async {
                let got = simple_column(&client, sql).await?;
                (got == [want.to_string()])
                    .then_some(())
                    .ok_or(format!("got {got:?}, expected [{want}]"))
            }
            .await,
        );
    }

    report.record(
        Area::Types,
        "a malformed value is refused by the cast, not accepted",
        async {
            // The direction that matters: adding the cast must not make
            // anything castable.
            for sql in ["SELECT 'nope'::json", "SELECT 'abc'::NUMERIC"] {
                if client.simple_query(sql).await.is_ok() {
                    return Err(format!("{sql} was accepted"));
                }
            }
            Ok(())
        }
        .await,
    );

    report.record(
        Area::Connection,
        "SHOW agrees with what the startup advertised",
        async {
            // A driver reads `server_version` to decide what the server
            // supports. It arrived in ParameterStatus but `SHOW` answered an
            // empty string, so the two disagreed about the same setting.
            let version = simple_column(&client, "SHOW server_version").await?;
            let reported = version.first().cloned().unwrap_or_default();
            if reported.is_empty() {
                return Err("SHOW server_version is empty".to_string());
            }
            // libpq reads the leading digits, so a version must start with one.
            if !reported.starts_with(|c: char| c.is_ascii_digit()) {
                return Err(format!(
                    "server_version {reported:?} does not start with a digit"
                ));
            }
            let encoding = simple_column(&client, "SHOW client_encoding").await?;
            (encoding == ["UTF8"])
                .then_some(())
                .ok_or(format!("client_encoding is {encoding:?}, expected [UTF8]"))
        }
        .await,
    );

    report.record(
        Area::Connection,
        "GSSENCRequest is declined in the conforming way",
        async {
            // A server without Kerberos integration answers a single `N` and
            // the client continues in the clear. PostgreSQL built without
            // --with-gssapi does exactly this; the negotiation is a protocol
            // outcome, not a missing message.
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut socket = tokio::net::TcpStream::connect("127.0.0.1:5432")
                .await
                .map_err(|e| e.to_string())?;

            let mut request = Vec::new();
            request.extend_from_slice(&8i32.to_be_bytes());
            request.extend_from_slice(&80_877_104i32.to_be_bytes());
            socket
                .write_all(&request)
                .await
                .map_err(|e| e.to_string())?;

            let mut answer = [0u8; 1];
            socket
                .read_exact(&mut answer)
                .await
                .map_err(|e| e.to_string())?;
            if answer != *b"N" {
                return Err(format!("answered {:?}, expected N", answer[0] as char));
            }

            // And the same connection must still be usable: a decline is not
            // a disconnect.
            let mut startup = Vec::new();
            for (key, value) in [("user", "postgres"), ("database", "orbit")] {
                startup.extend_from_slice(key.as_bytes());
                startup.push(0);
                startup.extend_from_slice(value.as_bytes());
                startup.push(0);
            }
            startup.push(0);
            let mut framed = Vec::new();
            framed.extend_from_slice(&((startup.len() + 8) as i32).to_be_bytes());
            framed.extend_from_slice(&196_608i32.to_be_bytes());
            framed.extend_from_slice(&startup);
            socket.write_all(&framed).await.map_err(|e| e.to_string())?;

            let mut tag = [0u8; 1];
            socket
                .read_exact(&mut tag)
                .await
                .map_err(|e| e.to_string())?;
            (tag == *b"R")
                .then_some(())
                .ok_or(format!("startup answered {:?}, expected R", tag[0] as char))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an expression in WHERE filters instead of being dropped",
        async {
            drop_table(&client, "conf_expr_where").await;
            client
                .simple_query("CREATE TABLE conf_expr_where (id INTEGER, name TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "INSERT INTO conf_expr_where (id, name) VALUES (1, 'ada'), (2, 'grace'), (3, 'alan')",
                )
                .await
                .map_err(describe)?;

            // `id * 2 = 4` was read as column `id`, operator `*`, and the
            // storage matcher treats an operator it does not know as matching
            // every row — so this returned the whole table.
            let arithmetic =
                simple_column(&client, "SELECT name FROM conf_expr_where WHERE id * 2 = 4").await?;
            if arithmetic != ["grace"] {
                return Err(format!("arithmetic gave {arithmetic:?}, expected [grace]"));
            }
            // A function call in WHERE went the same way and matched nothing.
            let called = simple_column(
                &client,
                "SELECT name FROM conf_expr_where WHERE UPPER(name) = 'ADA'",
            )
            .await?;
            let result = (called == ["ada"])
                .then_some(())
                .ok_or(format!("a function gave {called:?}, expected [ada]"));
            drop_table(&client, "conf_expr_where").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a stored function can be called from inside a query",
        async {
            drop_table(&client, "conf_callable").await;
            client
                .simple_query("CREATE TABLE conf_callable (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_callable (id) VALUES (1), (2), (3)")
                .await
                .map_err(describe)?;
            let _ = client.simple_query("DROP FUNCTION conf_dbl").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_dbl(a INTEGER) RETURNS INTEGER AS $$ BEGIN RETURN a * 2; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;

            // Only `SELECT f(literal)` used to work; over a table this failed
            // and in a WHERE it quietly matched nothing.
            let projected =
                simple_column(&client, "SELECT conf_dbl(id) FROM conf_callable ORDER BY id").await?;
            if projected != ["2", "4", "6"] {
                return Err(format!("select list gave {projected:?}, expected [2, 4, 6]"));
            }
            let filtered = simple_column(
                &client,
                "SELECT id FROM conf_callable WHERE conf_dbl(id) = 4",
            )
            .await?;
            let result = (filtered == ["2"])
                .then_some(())
                .ok_or(format!("WHERE gave {filtered:?}, expected [2]"));
            drop_table(&client, "conf_callable").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a column's declared type chooses the overload inside a query",
        async {
            drop_table(&client, "conf_mix").await;
            client
                .simple_query("CREATE TABLE conf_mix (n INTEGER, s TEXT, b BIGINT)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_mix (n, s, b) VALUES (2, 'ada', 5000000000)")
                .await
                .map_err(describe)?;
            let _ = client.simple_query("DROP FUNCTION conf_kindof2").await;
            for (declaration, answer) in [
                ("a INTEGER", "int4"),
                ("a TEXT", "text"),
                ("a BIGINT", "int8"),
            ] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_kindof2({declaration}) RETURNS TEXT AS $$ BEGIN RETURN '{answer}'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }

            // Keyed by argument count alone, every one of these reached
            // whichever overload was defined last.
            let mut chosen = Vec::new();
            for column in ["n", "s", "b"] {
                chosen.push(
                    simple_column(&client, &format!("SELECT conf_kindof2({column}) FROM conf_mix"))
                        .await?
                        .join(""),
                );
            }
            let result = (chosen == ["int4", "text", "int8"])
                .then_some(())
                .ok_or(format!("chose {chosen:?}, expected [int4, text, int8]"));
            drop_table(&client, "conf_mix").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a function that runs SQL is refused inside a query, not mis-run",
        async {
            drop_table(&client, "conf_impure").await;
            client
                .simple_query("CREATE TABLE conf_impure (id INTEGER)")
                .await
                .map_err(describe)?;
            client
                .simple_query("INSERT INTO conf_impure (id) VALUES (1)")
                .await
                .map_err(describe)?;
            let _ = client.simple_query("DROP FUNCTION conf_writes").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_writes(a INTEGER) RETURNS INTEGER AS $$ BEGIN INSERT INTO conf_impure (id) VALUES (a); RETURN a; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;

            // The evaluator is synchronous; a body that runs SQL cannot be
            // called from it. Saying so beats running it in a way that could
            // deadlock, and beats answering the wrong thing.
            let refused = client
                .simple_query("SELECT conf_writes(id) FROM conf_impure")
                .await
                .is_err();
            if !refused {
                drop_table(&client, "conf_impure").await;
                return Err("a SQL-running function was called from inside a query".into());
            }
            // But it still works where it always did.
            let direct = simple_column(&client, "SELECT conf_writes(9)").await?;
            let result = (direct == ["9"])
                .then_some(())
                .ok_or(format!("a direct call gave {direct:?}, expected [9]"));
            drop_table(&client, "conf_impure").await;
            result
        }
        .await,
    );

    report.record(
        Area::Types,
        "a composite type is created and appears in pg_type",
        async {
            let _ = client.simple_query("DROP TYPE IF EXISTS conf_addr").await;
            client
                .simple_query("CREATE TYPE conf_addr AS (street TEXT, num INTEGER)")
                .await
                .map_err(describe)?;
            // `c` is what tells a composite from a base type; a client reads
            // it to know the type has fields.
            let kind = simple_column(
                &client,
                "SELECT typtype FROM pg_type WHERE typname = 'conf_addr'",
            )
            .await?;
            (kind == ["c"])
                .then_some(())
                .ok_or(format!("typtype is {kind:?}, expected [c]"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a composite variable's fields are assignable and readable",
        async {
            drop_table(&client, "conf_comp").await;
            client
                .simple_query("CREATE TABLE conf_comp (v TEXT)")
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "DO $$ DECLARE a conf_addr; BEGIN a.street := 'Main'; a.num := 7; INSERT INTO conf_comp (v) VALUES (a.street); END $$",
                )
                .await
                .map_err(describe)?;
            let stored = simple_column(&client, "SELECT v FROM conf_comp").await?;
            let result = (stored == ["Main"])
                .then_some(())
                .ok_or(format!("got {stored:?}, expected [Main]"));
            drop_table(&client, "conf_comp").await;
            result
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a composite is its own type when choosing an overload",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_comp_fn").await;
            for (declaration, answer) in [("a conf_addr", "composite"), ("a TEXT", "text")] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_comp_fn({declaration}) RETURNS TEXT AS $$ BEGIN RETURN '{answer}'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }
            // Reduced to `text` these would be one signature, and a text call
            // could reach the composite form.
            let textual = simple_column(&client, "SELECT conf_comp_fn('x')").await?;
            if textual != ["text"] {
                return Err(format!("a text call chose {textual:?}"));
            }
            // And the catalogue reports the composite's own OID, the same one
            // pg_type gives it.
            let declared = simple_column(
                &client,
                "SELECT proargtypes FROM pg_proc WHERE proname = 'conf_comp_fn'",
            )
            .await?;
            let composite = simple_column(
                &client,
                "SELECT oid FROM pg_type WHERE typname = 'conf_addr'",
            )
            .await?;
            (composite.first().is_some_and(|oid| declared.contains(oid)))
                .then_some(())
                .ok_or(format!("proargtypes {declared:?} do not include {composite:?}"))
        }
        .await,
    );

    report.record(
        Area::Types,
        "DROP TYPE refuses a type that was never there",
        async {
            // Reporting success for a type that does not exist is the silent
            // no-op this harness exists to catch.
            match client.simple_query("DROP TYPE conf_no_such_type").await {
                Ok(_) => return Err("dropping an absent type reported success".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    if got != "42704" {
                        return Err(format!("reported {got}, expected 42704"));
                    }
                }
            }
            // IF EXISTS is the form that may say nothing.
            client
                .simple_query("DROP TYPE IF EXISTS conf_no_such_type")
                .await
                .map_err(describe)?;
            client
                .simple_query("DROP TYPE conf_addr")
                .await
                .map_err(describe)?;
            let left = simple_column(
                &client,
                "SELECT typname FROM pg_type WHERE typname = 'conf_addr'",
            )
            .await?;
            left.is_empty()
                .then_some(())
                .ok_or(format!("{left:?} survived the drop"))
        }
        .await,
    );

    report.record(
        Area::Types,
        "a cast to a domain is a cast to what it is built on",
        async {
            let _ = client
                .simple_query("CREATE DOMAIN conf_pos AS INTEGER CHECK (VALUE > 0)")
                .await;
            let cast = simple_column(&client, "SELECT 42::conf_pos").await?;
            if cast != ["42"] {
                return Err(format!("got {cast:?}, expected [42]"));
            }
            // The direction that matters: a name that is not a domain must
            // still fail, or every typo'd type would silently succeed.
            match client.simple_query("SELECT 42::conf_no_such_type").await {
                Ok(_) => Err("a cast to an unknown type was accepted".into()),
                Err(_) => Ok(()),
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "two array overloads of one name coexist",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_arrs").await;
            for (declaration, answer) in [("a INTEGER[]", "ints"), ("a TEXT[]", "texts")] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_arrs({declaration}) RETURNS TEXT AS $$ BEGIN RETURN '{answer}'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }
            // 1007 is int4[] and 1009 is text[]. Collapsing every array to one
            // kind made these one signature, so the second replaced the first.
            let types = simple_column(
                &client,
                "SELECT proargtypes FROM pg_proc WHERE proname = 'conf_arrs'",
            )
            .await?;
            (types.contains(&"1007".to_string()) && types.contains(&"1009".to_string()))
                .then_some(())
                .ok_or(format!("proargtypes are {types:?}, expected 1007 and 1009"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an array parameter is its own type",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_arr").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_arr(a INTEGER[]) RETURNS TEXT AS $$ BEGIN RETURN 'array'; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            client
                .simple_query(
                    "CREATE FUNCTION conf_arr(a TEXT) RETURNS TEXT AS $$ BEGIN RETURN 'scalar'; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // 1007 is int4[]. Collapsing it to text would have the two
            // overloads collide and report the wrong type to a client.
            let types = simple_column(
                &client,
                "SELECT proargtypes FROM pg_proc WHERE proname = 'conf_arr'",
            )
            .await?;
            if !types.contains(&"1007".to_string()) {
                return Err(format!("proargtypes are {types:?}, expected one to be 1007"));
            }
            // An array must never swallow a scalar call.
            let scalar = simple_column(&client, "SELECT conf_arr('x')").await?;
            (scalar == ["scalar"])
                .then_some(())
                .ok_or(format!("a scalar call chose {scalar:?}"))
        }
        .await,
    );

    report.record(
        Area::Catalog,
        "two overloads of a name get different OIDs",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_two").await;
            for declaration in ["a INTEGER", "a TEXT"] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_two({declaration}) RETURNS TEXT AS $$ BEGIN RETURN 'x'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }
            let oids =
                simple_column(&client, "SELECT oid FROM pg_proc WHERE proname = 'conf_two'")
                    .await?;
            // One OID for two functions would make a fast-path call ambiguous.
            (oids.len() == 2 && oids[0] != oids[1])
                .then_some(())
                .ok_or(format!("got {oids:?}, expected two distinct OIDs"))
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an ambiguous call reports ambiguous_function, not undefined",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_amb").await;
            // Neither takes a string, so the untyped-literal rule cannot
            // choose and both are their category's preferred type.
            for declaration in ["a INTEGER", "a BOOLEAN"] {
                client
                    .simple_query(&format!(
                        "CREATE FUNCTION conf_amb({declaration}) RETURNS TEXT AS $$ BEGIN RETURN 'x'; END $$ LANGUAGE plpgsql"
                    ))
                    .await
                    .map_err(describe)?;
            }
            // Telling a caller the function is missing when it is the choice
            // between two of them that failed sends them looking in the wrong
            // place.
            match client.simple_query("SELECT conf_amb(NULL)").await {
                Ok(_) => Err("an ambiguous call was answered".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    (got == "42725")
                        .then_some(())
                        .ok_or(format!("reported {got}, expected 42725"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "an ambiguous call is refused rather than guessed",
        async {
            // NULL names no type, so it fits both overloads. Picking one
            // silently would be a coin toss the caller cannot see.
            match client.simple_query("SELECT conf_amb(NULL)").await {
                Ok(_) => Err("an ambiguous call was answered".into()),
                Err(e) => describe(e)
                    .contains("not unique")
                    .then_some(())
                    .ok_or_else(|| "refused, but not as an ambiguity".to_string()),
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a value that cannot be the declared type is 22P02",
        async {
            match client
                .simple_query("SELECT conf_bump('not a number')")
                .await
            {
                Ok(_) => Err("text was accepted for an INTEGER parameter".into()),
                Err(e) => {
                    let got = sqlstate(&e);
                    (got == "22P02")
                        .then_some(())
                        .ok_or(format!("reported {got}, expected 22P02"))
                }
            }
        }
        .await,
    );

    report.record(
        Area::Sql,
        "a parameter type containing a comma is one parameter",
        async {
            let _ = client.simple_query("DROP FUNCTION conf_money").await;
            client
                .simple_query(
                    "CREATE FUNCTION conf_money(a NUMERIC(10, 2)) RETURNS NUMERIC AS $$ BEGIN RETURN a; END $$ LANGUAGE plpgsql",
                )
                .await
                .map_err(describe)?;
            // Splitting the list on every comma made this two parameters, so
            // the call arrived with the wrong count.
            let value = simple_column(&client, "SELECT conf_money(3.14)").await?;
            (value == ["3.14"])
                .then_some(())
                .ok_or(format!("got {value:?}, expected [3.14]"))
        }
        .await,
    );

    drop_table(&client, "conf_caught").await;
    drop_table(&client, "conf_state_child").await;
    drop_table(&client, "conf_state").await;

    drop_table(&client, "conf_src").await;
    drop_table(&client, "conf_dst").await;
    drop_table(&client, "conf_pl").await;

    drop_table(&client, "conf_five").await;

    println!("{}", report.render());

    // The harness reports; it does not gate. Conformance is tracked as a number
    // that should move up, and failing the build on a known gap would only make
    // the number invisible.
    assert!(
        report.passed() > 0,
        "no conformance checks passed at all — the server is not usable"
    );
}
