//! BDD steps for PostgreSQL transaction and savepoint behavior.
//!
//! Drives the durable SQL engine directly with named sessions, so a scenario can interleave
//! several connections against one engine. Kept apart from `tests/bdd/mod.rs`, which owns the
//! general database and vector steps.

use cucumber::{gherkin::Step, given, then, when, World};
use orbit_server::protocols::postgres_wire::query_engine::QueryResult;
use orbit_server::protocols::postgres_wire::sql::durable::{DurableEngine, Session};
use orbit_server::protocols::postgres_wire::sql::SqlParser;
use std::collections::HashMap;

type StepResult = Result<(), Box<dyn std::error::Error>>;

#[derive(World)]
#[world(init = Self::new)]
pub struct TxnWorld {
    engine: DurableEngine,
    sessions: HashMap<String, Session>,
    /// Outcome of the most recent `executes` step: `Ok` or the error text.
    last: Option<Result<(), String>>,
}

impl std::fmt::Debug for TxnWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxnWorld")
            .field("sessions", &self.sessions.keys().collect::<Vec<_>>())
            .field("last", &self.last)
            .finish()
    }
}

impl TxnWorld {
    fn new() -> Self {
        Self {
            engine: DurableEngine::in_memory(),
            sessions: HashMap::new(),
            last: None,
        }
    }

    /// Run `sql` (one or more statements) on the named session, creating it on first use.
    async fn run(
        &mut self,
        session: &str,
        sql: &str,
    ) -> Result<QueryResult, Box<dyn std::error::Error>> {
        let sess = self.sessions.entry(session.to_string()).or_default();
        let mut last = QueryResult::Command {
            tag: "EMPTY".into(),
        };
        for stmt in SqlParser::new().parse_multiple(sql)? {
            last = self
                .engine
                .execute(sess, &stmt, sql)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(last)
    }

    fn state(&self, session: &str) -> Option<bool> {
        self.sessions.get(session).and_then(Session::txn_state)
    }
}

fn rows_of(result: QueryResult) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    match result.untyped() {
        QueryResult::Select { rows, .. } => Ok(rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|cell| cell.unwrap_or_else(|| "NULL".into()))
                    .collect()
            })
            .collect()),
        other => Err(format!("expected rows, got {other:?}").into()),
    }
}

#[given(expr = "a fresh SQL engine with:")]
async fn fresh_engine(world: &mut TxnWorld, step: &Step) -> StepResult {
    *world = TxnWorld::new();
    let script = step.docstring.as_ref().ok_or("missing setup script")?;
    world.run("setup", script).await?;
    world.sessions.remove("setup");
    Ok(())
}

#[when(expr = "session {string} executes {string}")]
async fn executes(world: &mut TxnWorld, session: String, sql: String) -> StepResult {
    world.last = Some(
        world
            .run(&session, &sql)
            .await
            .map(drop)
            .map_err(|e| e.to_string()),
    );
    Ok(())
}

#[when(expr = "session {string} disconnects")]
async fn disconnects(world: &mut TxnWorld, session: String) -> StepResult {
    // Dropping the session without COMMIT or ROLLBACK is what a client disconnect does.
    world
        .sessions
        .remove(&session)
        .map(drop)
        .ok_or_else(|| format!("no such session {session}").into())
}

#[then(expr = "the statement should succeed")]
async fn statement_succeeds(world: &mut TxnWorld) -> StepResult {
    match world.last.take() {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(format!("expected success, got error: {e}").into()),
        None => Err("no statement has been executed".into()),
    }
}

#[then(expr = "the statement should fail")]
async fn statement_fails(world: &mut TxnWorld) -> StepResult {
    match world.last.take() {
        Some(Err(_)) => Ok(()),
        Some(Ok(())) => Err("expected an error but the statement succeeded".into()),
        None => Err("no statement has been executed".into()),
    }
}

#[then(expr = "the statement should fail with SQLSTATE {string}")]
async fn statement_fails_with(world: &mut TxnWorld, code: String) -> StepResult {
    match world.last.take() {
        Some(Err(e)) if e.contains(&format!("[{code}]")) => Ok(()),
        Some(Err(e)) => Err(format!("expected SQLSTATE {code}, got: {e}").into()),
        Some(Ok(())) => Err(format!("expected SQLSTATE {code} but the statement succeeded").into()),
        None => Err("no statement has been executed".into()),
    }
}

#[then(expr = "session {string} should be outside a transaction")]
async fn outside_txn(world: &mut TxnWorld, session: String) -> StepResult {
    match world.state(&session) {
        None => Ok(()),
        Some(failed) => {
            Err(format!("session {session} is in a transaction (failed={failed})").into())
        }
    }
}

#[then(expr = "session {string} should be in an open transaction")]
async fn in_open_txn(world: &mut TxnWorld, session: String) -> StepResult {
    match world.state(&session) {
        Some(false) => Ok(()),
        other => Err(format!("expected a healthy open transaction, state is {other:?}").into()),
    }
}

#[then(expr = "session {string} should be in a failed transaction")]
async fn in_failed_txn(world: &mut TxnWorld, session: String) -> StepResult {
    match world.state(&session) {
        Some(true) => Ok(()),
        other => Err(format!("expected a failed transaction, state is {other:?}").into()),
    }
}

#[then(expr = "session {string} querying {string} should return the value {string}")]
async fn returns_value(
    world: &mut TxnWorld,
    session: String,
    sql: String,
    expected: String,
) -> StepResult {
    let rows = rows_of(world.run(&session, &sql).await?)?;
    match rows.as_slice() {
        [row] if row.as_slice() == [expected.clone()] => Ok(()),
        other => Err(format!("expected a single value {expected:?}, got {other:?}").into()),
    }
}

#[then(expr = "session {string} querying {string} should return:")]
async fn returns_rows(
    world: &mut TxnWorld,
    session: String,
    sql: String,
    step: &Step,
) -> StepResult {
    let expected = step
        .table
        .as_ref()
        .ok_or("step is missing a data table")?
        .rows
        .clone();
    let actual = rows_of(world.run(&session, &sql).await?)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected rows {expected:?}, got {actual:?}").into())
    }
}

#[tokio::main]
async fn main() {
    TxnWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/bdd_transactions/features"
        ))
        .await;
}
