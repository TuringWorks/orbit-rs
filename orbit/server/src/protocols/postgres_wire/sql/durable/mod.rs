//! A single SQL engine whose committed state lives in durable storage.
//!
//! Every statement runs here, reads see exactly what was committed, and a transaction becomes one
//! atomic synced write (`persist::Op`s). The working set is held in memory and rebuilt from
//! storage when the engine opens, so the data set must fit in RAM; paging is a later step.
//!
//! Concurrency in this first version is a single gate: one transaction at a time (a session in
//! `BEGIN` holds the gate until `COMMIT` or `ROLLBACK`). Snapshot isolation replaces it later.

mod ddl;
mod dml;
mod persist;
mod query;
pub(crate) mod value;

#[cfg(feature = "storage-rocksdb")]
pub use persist::RocksPersist;
pub use persist::{NoPersist, Op, Persist};

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::query_engine::QueryResult;
use crate::protocols::postgres_wire::sql::ast::{Expression, Statement};
use crate::protocols::postgres_wire::sql::parser::SqlParser;
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

pub(crate) type Row = Vec<SqlValue>;

/// How many sequence values are reserved per durable write. After a crash a sequence resumes
/// above the reserved block, so ids may skip but are never reused.
const SEQUENCE_BLOCK: i64 = 32;

/// An error with the SQLSTATE a PostgreSQL client expects.
#[derive(Debug)]
pub(crate) struct DbError {
    pub code: &'static str,
    pub message: String,
}

pub(crate) type R<T> = Result<T, DbError>;

pub(crate) fn err<T>(code: &'static str, message: impl Into<String>) -> R<T> {
    Err(DbError {
        code,
        message: message.into(),
    })
}

impl From<ProtocolError> for DbError {
    fn from(e: ProtocolError) -> Self {
        let message = match e {
            ProtocolError::PostgresError(m) | ProtocolError::ParseError(m) => m,
            other => other.to_string(),
        };
        let code = if message.contains("not found") && message.contains("olumn") {
            "42703"
        } else {
            "XX000"
        };
        DbError { code, message }
    }
}

impl From<DbError> for ProtocolError {
    fn from(e: DbError) -> Self {
        ProtocolError::PostgresError(format!("[{}] {}", e.code, e.message))
    }
}

#[derive(Clone)]
pub(crate) struct Col {
    pub name: String,
    pub ty: SqlType,
    pub not_null: bool,
    pub default: Option<Expression>,
    /// Name of the owned sequence for a `SERIAL` column.
    pub serial: Option<String>,
}

/// A hash index over one or more columns. Unique indexes hold at most one row id per key.
#[derive(Clone)]
pub(crate) struct Index {
    pub name: String,
    pub cols: Vec<usize>,
    pub unique: bool,
    /// Whether this is the table's primary key. Not enforced differently yet; `information_schema` will report it.
    #[allow(dead_code)]
    pub primary: bool,
    pub map: HashMap<String, Vec<u64>>,
}

impl Index {
    pub fn key(&self, row: &Row) -> Option<String> {
        let vals: Vec<SqlValue> = self.cols.iter().map(|&c| row[c].clone()).collect();
        if self.unique {
            value::unique_key(&vals)
        } else {
            Some(value::group_key(&vals))
        }
    }
}

#[derive(Clone)]
pub(crate) struct Table {
    pub id: u32,
    pub name: String,
    pub cols: Vec<Col>,
    pub rows: BTreeMap<u64, Row>,
    pub next_rowid: u64,
    pub indexes: Vec<Index>,
}

impl Table {
    pub fn col_index(&self, name: &str) -> Option<usize> {
        let n = name.to_lowercase();
        self.cols.iter().position(|c| c.name == n)
    }

    /// The name of the unique index a row would violate, if any, ignoring the row `except`.
    pub fn unique_conflict(&self, row: &Row, except: Option<u64>) -> Option<(&Index, u64)> {
        for idx in self.indexes.iter().filter(|i| i.unique) {
            if let Some(k) = idx.key(row) {
                if let Some(ids) = idx.map.get(&k) {
                    if let Some(&id) = ids.iter().find(|&&id| Some(id) != except) {
                        return Some((idx, id));
                    }
                }
            }
        }
        None
    }

    pub fn index_add(&mut self, rowid: u64, row: &Row) {
        for idx in &mut self.indexes {
            if let Some(k) = idx.key(row) {
                idx.map.entry(k).or_default().push(rowid);
            }
        }
    }

    pub fn index_remove(&mut self, rowid: u64, row: &Row) {
        for idx in &mut self.indexes {
            if let Some(k) = idx.key(row) {
                if let Some(ids) = idx.map.get_mut(&k) {
                    ids.retain(|&i| i != rowid);
                    if ids.is_empty() {
                        idx.map.remove(&k);
                    }
                }
            }
        }
    }

    pub fn rebuild_indexes(&mut self) {
        for i in 0..self.indexes.len() {
            self.indexes[i].map.clear();
        }
        let rows: Vec<(u64, Row)> = self.rows.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (id, row) in rows {
            self.index_add(id, &row);
        }
    }
}

struct SeqState {
    next: i64,
    reserved: i64,
}

#[derive(Default)]
pub(crate) struct State {
    pub tables: HashMap<String, Table>,
    pub next_table_id: u32,
    seqs: HashMap<String, SeqState>,
}

/// Undo information for one change, applied in reverse on rollback.
pub(crate) enum Undo {
    Inserted {
        table: String,
        rowid: u64,
    },
    Deleted {
        table: String,
        rowid: u64,
        row: Row,
    },
    Updated {
        table: String,
        rowid: u64,
        old: Row,
    },
    /// The table as it was before a DDL change (`None` if it did not exist).
    Table {
        name: String,
        prev: Option<Box<Table>>,
    },
}

pub(crate) struct Txn {
    pub ops: Vec<Op>,
    pub undo: Vec<Undo>,
    failed: bool,
    guard: Option<OwnedMutexGuard<()>>,
    /// Open savepoints: name, undo entries and ops recorded when it was set.
    savepoints: Vec<(String, usize, usize)>,
    /// Lets a transaction that is dropped while open (a client that disconnects) undo its changes.
    state: Arc<Mutex<State>>,
}

impl Txn {
    fn new(guard: Option<OwnedMutexGuard<()>>, state: Arc<Mutex<State>>) -> Self {
        Self {
            ops: Vec::new(),
            undo: Vec::new(),
            failed: false,
            guard,
            savepoints: Vec::new(),
            state,
        }
    }

    /// The changes are durable: forget how to undo them.
    fn committed(&mut self) {
        self.undo.clear();
        self.ops.clear();
        self.savepoints.clear();
    }
}

impl Drop for Txn {
    fn drop(&mut self) {
        if self.undo.is_empty() {
            return;
        }
        // Dropped with uncommitted changes: they were never written, so remove them from memory too.
        let state = Arc::clone(&self.state);
        let locked = state.lock();
        if let Ok(mut st) = locked {
            undo_to(&mut st, self, 0);
        }
    }
}

/// Per-connection state: the open transaction, if any.
#[derive(Default)]
pub struct Session {
    txn: Option<Txn>,
}

impl Session {
    /// `Some(true)` inside a failed transaction, `Some(false)` inside a healthy one, `None` outside.
    pub fn txn_state(&self) -> Option<bool> {
        self.txn.as_ref().map(|t| t.failed)
    }
}

pub struct DurableEngine {
    gate: Arc<AsyncMutex<()>>,
    state: Arc<Mutex<State>>,
    persist: Box<dyn Persist>,
}

impl State {
    /// Next value of a sequence. Reserves a block durably when the previous block runs out.
    pub(crate) fn nextval(&mut self, name: &str, persist: &dyn Persist) -> R<i64> {
        let s = self.seqs.entry(name.to_string()).or_insert(SeqState {
            next: 1,
            reserved: 0,
        });
        if s.next > s.reserved {
            let reserved = s.next + SEQUENCE_BLOCK - 1;
            persist
                .commit(&[Op::SeqHighWater {
                    name: name.to_string(),
                    value: reserved,
                }])
                .map_err(|e| DbError {
                    code: "58030",
                    message: e,
                })?;
            s.reserved = reserved;
        }
        let v = s.next;
        s.next += 1;
        Ok(v)
    }

    pub(crate) fn reset_seq(&mut self, name: &str) {
        self.seqs.insert(
            name.to_string(),
            SeqState {
                next: 1,
                reserved: 0,
            },
        );
    }
}

impl DurableEngine {
    /// An engine that keeps nothing across restarts.
    pub fn in_memory() -> Self {
        Self {
            gate: Arc::new(AsyncMutex::new(())),
            state: Arc::new(Mutex::new(State::default())),
            persist: Box::new(NoPersist),
        }
    }

    /// Open an engine over `persist`, rebuilding the working set from it.
    pub fn open(persist: Box<dyn Persist>) -> Result<Self, String> {
        let loaded = persist.load()?;
        let mut st = State::default();
        for (name, hwm) in loaded.seqs {
            st.seqs.insert(
                name,
                SeqState {
                    next: hwm + 1,
                    reserved: hwm,
                },
            );
        }
        for text in &loaded.ddl {
            let stmts = SqlParser::new()
                .parse_multiple(text)
                .map_err(|e| format!("replay `{text}`: {e}"))?;
            for stmt in &stmts {
                if let Some(Err(e)) = ddl::apply(&mut st, None, stmt, text, true) {
                    return Err(format!("replay `{text}`: {}", e.message));
                }
            }
        }
        let by_id: HashMap<u32, String> =
            st.tables.values().map(|t| (t.id, t.name.clone())).collect();
        for (tid, rowid, mut row) in loaded.rows {
            let Some(name) = by_id.get(&tid) else {
                continue;
            };
            let t = st.tables.get_mut(name).expect("table by id");
            row.resize(t.cols.len(), SqlValue::Null);
            t.next_rowid = t.next_rowid.max(rowid + 1);
            t.rows.insert(rowid, row);
        }
        for t in st.tables.values_mut() {
            t.rebuild_indexes();
        }
        Ok(Self {
            gate: Arc::new(AsyncMutex::new(())),
            state: Arc::new(Mutex::new(st)),
            persist,
        })
    }

    /// Whether the engine runs this kind of statement. Other statements stay with the caller.
    pub fn handles(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::CreateTable(_)
                | Statement::DropTable(_)
                | Statement::CreateIndex(_)
                | Statement::DropIndex(_)
                | Statement::AlterTable(_)
                | Statement::Truncate(_)
                | Statement::Select(_)
                | Statement::Insert(_)
                | Statement::Update(_)
                | Statement::Delete(_)
                | Statement::Begin(_)
                | Statement::Commit(_)
                | Statement::Rollback(_)
                | Statement::Savepoint(_)
                | Statement::ReleaseSavepoint(_)
        )
    }

    /// The result columns of a statement (name and type OID), without running it. Empty when the
    /// statement returns no rows; `None` when the engine does not run this statement.
    pub fn describe(&self, stmt: &Statement) -> Option<Vec<(String, i32)>> {
        if !Self::handles(stmt) {
            return None;
        }
        let st = self.state.lock().expect("engine state");
        let cols = match stmt {
            Statement::Select(sel) => query::describe_select(&st, sel).ok(),
            Statement::Insert(i) => i
                .returning
                .as_ref()
                .and_then(|r| dml::describe_returning(&st, &i.table, r).ok()),
            Statement::Update(u) => u
                .returning
                .as_ref()
                .and_then(|r| dml::describe_returning(&st, &u.table, r).ok()),
            Statement::Delete(d) => d
                .returning
                .as_ref()
                .and_then(|r| dml::describe_returning(&st, &d.table, r).ok()),
            _ => None,
        };
        Some(cols.unwrap_or_default())
    }

    /// Run one statement for a session. `text` is the statement's SQL, stored for DDL replay.
    pub async fn execute(
        &self,
        session: &mut Session,
        stmt: &Statement,
        text: &str,
    ) -> ProtocolResult<QueryResult> {
        match stmt {
            Statement::Begin(_) => {
                if session.txn.is_none() {
                    let guard = Arc::clone(&self.gate).lock_owned().await;
                    session.txn = Some(Txn::new(Some(guard), Arc::clone(&self.state)));
                }
                Ok(QueryResult::Command {
                    tag: "BEGIN".into(),
                })
            }
            Statement::Commit(_) => Ok(QueryResult::Command {
                tag: self.finish(session, true)?,
            }),
            Statement::Rollback(r) if r.to_savepoint.is_some() => {
                let name = r.to_savepoint.clone().unwrap_or_default();
                let Some(txn) = session.txn.as_mut() else {
                    return Err(DbError {
                        code: "25P01",
                        message: "ROLLBACK TO SAVEPOINT can only be used in transaction blocks"
                            .into(),
                    }
                    .into());
                };
                let mut st = self.state.lock().expect("engine state");
                let Some(pos) = txn.savepoints.iter().rposition(|(n, _, _)| *n == name) else {
                    return Err(DbError {
                        code: "3B001",
                        message: format!("savepoint \"{name}\" does not exist"),
                    }
                    .into());
                };
                let (_, undo_len, ops_len) = txn.savepoints[pos].clone();
                txn.savepoints.truncate(pos + 1);
                undo_to(&mut st, txn, undo_len);
                txn.ops.truncate(ops_len);
                txn.failed = false;
                Ok(QueryResult::Command {
                    tag: "ROLLBACK".into(),
                })
            }
            Statement::Rollback(_) => Ok(QueryResult::Command {
                tag: self.finish(session, false)?,
            }),
            Statement::Savepoint(sp) => {
                let Some(txn) = session.txn.as_mut() else {
                    return Err(DbError {
                        code: "25P01",
                        message: "SAVEPOINT can only be used in transaction blocks".into(),
                    }
                    .into());
                };
                if txn.failed {
                    return Err(DbError { code: "25P02", message: "current transaction is aborted, commands ignored until end of transaction block".into() }.into());
                }
                txn.savepoints
                    .push((sp.name.clone(), txn.undo.len(), txn.ops.len()));
                Ok(QueryResult::Command {
                    tag: "SAVEPOINT".into(),
                })
            }
            Statement::ReleaseSavepoint(sp) => {
                let Some(txn) = session.txn.as_mut() else {
                    return Err(DbError {
                        code: "25P01",
                        message: "RELEASE SAVEPOINT can only be used in transaction blocks".into(),
                    }
                    .into());
                };
                let Some(pos) = txn.savepoints.iter().rposition(|(n, _, _)| *n == sp.name) else {
                    return Err(DbError {
                        code: "3B001",
                        message: format!("savepoint \"{}\" does not exist", sp.name),
                    }
                    .into());
                };
                txn.savepoints.truncate(pos);
                Ok(QueryResult::Command {
                    tag: "RELEASE".into(),
                })
            }
            _ => {
                if let Some(txn) = session.txn.as_mut() {
                    if txn.failed {
                        return Err(DbError { code: "25P02", message: "current transaction is aborted, commands ignored until end of transaction block".into() }.into());
                    }
                    let mut st = self.state.lock().expect("engine state");
                    let r = self.run(&mut st, txn, stmt, text);
                    if r.is_err() {
                        txn.failed = true;
                    }
                    return r.map_err(Into::into);
                }
                let guard = Arc::clone(&self.gate).lock_owned().await;
                let mut txn = Txn::new(Some(guard), Arc::clone(&self.state));
                let mut st = self.state.lock().expect("engine state");
                match self.run(&mut st, &mut txn, stmt, text) {
                    Ok(res) => {
                        if let Err(e) = self.persist.commit(&txn.ops) {
                            rollback(&mut st, &mut txn);
                            return Err(DbError {
                                code: "58030",
                                message: e,
                            }
                            .into());
                        }
                        txn.committed();
                        Ok(res)
                    }
                    Err(e) => {
                        rollback(&mut st, &mut txn);
                        Err(e.into())
                    }
                }
            }
        }
    }

    /// End the session's transaction. Returns the command tag.
    fn finish(&self, session: &mut Session, commit: bool) -> R<String> {
        let Some(mut txn) = session.txn.take() else {
            return Ok(if commit { "COMMIT" } else { "ROLLBACK" }.into());
        };
        let mut st = self.state.lock().expect("engine state");
        if commit && !txn.failed {
            if let Err(e) = self.persist.commit(&txn.ops) {
                rollback(&mut st, &mut txn);
                return err("58030", e);
            }
            txn.committed();
            return Ok("COMMIT".into());
        }
        rollback(&mut st, &mut txn);
        Ok("ROLLBACK".into())
    }

    fn run(&self, st: &mut State, txn: &mut Txn, stmt: &Statement, text: &str) -> R<QueryResult> {
        match stmt {
            Statement::Select(sel) => query::run_select_statement(st, sel),
            Statement::Insert(ins) => dml::insert(st, txn, &*self.persist, ins),
            Statement::Update(upd) => dml::update(st, txn, upd),
            Statement::Delete(del) => dml::delete(st, txn, del),
            other => ddl::apply(st, Some((txn, &*self.persist)), other, text, false)
                .unwrap_or_else(|| err("0A000", "statement not supported")),
        }
    }
}

/// Undo everything a transaction changed in memory and release its hold on the engine.
fn rollback(st: &mut State, txn: &mut Txn) {
    undo_to(st, txn, 0);
    txn.ops.clear();
    txn.savepoints.clear();
    txn.guard = None;
}

/// Undo changes until only `keep` undo entries remain.
fn undo_to(st: &mut State, txn: &mut Txn, keep: usize) {
    while txn.undo.len() > keep {
        let Some(u) = txn.undo.pop() else { break };
        match u {
            Undo::Inserted { table, rowid } => {
                if let Some(t) = st.tables.get_mut(&table) {
                    if let Some(row) = t.rows.remove(&rowid) {
                        t.index_remove(rowid, &row);
                    }
                }
            }
            Undo::Deleted { table, rowid, row } => {
                if let Some(t) = st.tables.get_mut(&table) {
                    t.index_add(rowid, &row);
                    t.rows.insert(rowid, row);
                }
            }
            Undo::Updated { table, rowid, old } => {
                if let Some(t) = st.tables.get_mut(&table) {
                    if let Some(cur) = t.rows.get(&rowid).cloned() {
                        t.index_remove(rowid, &cur);
                    }
                    t.index_add(rowid, &old);
                    t.rows.insert(rowid, old);
                }
            }
            Undo::Table { name, prev } => match prev {
                Some(t) => {
                    st.tables.insert(name, *t);
                }
                None => {
                    st.tables.remove(&name);
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
