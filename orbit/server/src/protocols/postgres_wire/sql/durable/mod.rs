//! A single SQL engine whose committed state lives in durable storage.
//!
//! Every statement runs here and a committed transaction is one atomic, synced list of physical
//! changes ([`Op`]). The data set is held in memory (immutable, structure-sharing maps) and
//! rebuilt from storage when the engine opens, so it must fit in RAM.
//!
//! Concurrency: the committed state is an immutable value. A read takes a snapshot (an O(1)
//! clone) and never waits for anyone. A transaction's first write takes the writer gate and a
//! private copy of the committed state; commit makes that copy the new committed state, rollback
//! and savepoints just drop or swap the copy. Writers are serialized, so there are no write
//! conflicts or deadlocks. Sessions that ask for REPEATABLE READ or SERIALIZABLE keep the snapshot
//! of their first statement and fail with `40001` if another transaction committed before their
//! first write.
//!
//! The same [`apply_ops`] function advances state from a list of ops at recovery and on replicas,
//! so a replicated store only has to ship the ops.

mod ddl;
mod dml;
mod persist;
mod query;
pub(crate) mod value;

#[cfg(feature = "storage-rocksdb")]
pub use persist::RocksPersist;
pub use persist::{Loaded, NoPersist, Op, Persist};

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::query_engine::QueryResult;
use crate::protocols::postgres_wire::sql::ast::{Expression, IsolationLevel, Statement};
use crate::protocols::postgres_wire::sql::parser::SqlParser;
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

pub(crate) type Row = Vec<SqlValue>;

/// How many sequence values are reserved per durable write. After a crash a sequence resumes
/// above the reserved block, so ids may skip but are never reused.
const SEQUENCE_BLOCK: i64 = 32;

/// Error code used internally when a sequence has run out of reserved values. The statement is
/// restarted after the reservation has been committed.
const NEED_RESERVE: &str = "RSV00";

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
    pub map: imbl::HashMap<String, Vec<u64>>,
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

/// A table. Cloning is cheap: rows and index maps share structure with the original.
#[derive(Clone)]
pub(crate) struct Table {
    pub id: u32,
    pub name: String,
    pub cols: Vec<Col>,
    pub rows: imbl::OrdMap<u64, Row>,
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

/// The database as of one commit: an immutable value that is cheap to clone.
#[derive(Clone, Default)]
pub(crate) struct State {
    pub tables: imbl::HashMap<String, Table>,
    /// Table name by table id, for applying row changes.
    pub ids: imbl::HashMap<u32, String>,
    pub next_table_id: u32,
    /// Number of commits applied; lets a REPEATABLE READ session notice that it is out of date.
    pub version: u64,
}

impl State {
    pub fn put_table(&mut self, t: Table) {
        self.ids.insert(t.id, t.name.clone());
        self.tables.insert(t.name.clone(), t);
    }

    pub fn remove_table(&mut self, name: &str) -> Option<Table> {
        let t = self.tables.remove(name)?;
        self.ids.remove(&t.id);
        Some(t)
    }
}

struct SeqState {
    next: i64,
    reserved: i64,
}

/// Sequences are not transactional (a rolled back insert keeps its id), so they live outside
/// the snapshot.
#[derive(Default)]
pub(crate) struct Seqs {
    map: HashMap<String, SeqState>,
}

impl Seqs {
    /// Next value, or the internal "reserve more" error when the reserved block is used up.
    pub fn alloc(&mut self, name: &str) -> R<i64> {
        let s = self.map.entry(name.to_string()).or_insert(SeqState {
            next: 1,
            reserved: 0,
        });
        if s.next > s.reserved {
            return err(NEED_RESERVE, name);
        }
        let v = s.next;
        s.next += 1;
        Ok(v)
    }

    /// The high-water mark to reserve next for `name`.
    fn reserve_target(&mut self, name: &str) -> i64 {
        let s = self.map.entry(name.to_string()).or_insert(SeqState {
            next: 1,
            reserved: 0,
        });
        s.next + SEQUENCE_BLOCK - 1
    }

    fn set_reserved(&mut self, name: &str, value: i64) {
        if value == 0 {
            self.map.remove(name);
            return;
        }
        let s = self.map.entry(name.to_string()).or_insert(SeqState {
            next: 1,
            reserved: 0,
        });
        s.reserved = s.reserved.max(value);
    }

    /// Apply the sequence changes in committed `ops`.
    fn apply(&mut self, ops: &[Op]) {
        for op in ops {
            if let Op::SeqHighWater { name, value } = op {
                self.set_reserved(name, *value);
            }
        }
    }

    /// A node that becomes the leader resumes every sequence above what any node may have used.
    fn resume_above_reserved(&mut self) {
        for s in self.map.values_mut() {
            s.next = s.next.max(s.reserved + 1);
        }
    }
}

/// A transaction. It holds no engine state until its first write.
pub(crate) struct Txn {
    /// The private copy of the database, once the transaction has written.
    work: Option<State>,
    /// The snapshot a REPEATABLE READ / SERIALIZABLE transaction keeps reading.
    base: Option<State>,
    pin: bool,
    ops: Vec<Op>,
    failed: bool,
    /// The writer gate, held from the first write until the transaction ends.
    guard: Option<OwnedMutexGuard<()>>,
    /// Open savepoints: name, the working copy and the op count when it was set.
    savepoints: Vec<(String, Option<State>, usize)>,
}

impl Txn {
    fn new(pin: bool) -> Self {
        Self {
            work: None,
            base: None,
            pin,
            ops: Vec::new(),
            failed: false,
            guard: None,
            savepoints: Vec::new(),
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
    /// Held by the one transaction that is writing.
    writer: Arc<AsyncMutex<()>>,
    committed: Mutex<State>,
    seqs: Mutex<Seqs>,
    persist: Box<dyn Persist>,
}

/// Advance `st` by committed `ops`. Used when opening the engine and on replicas; a leader's own
/// transactions arrive here as the copy they were run on.
pub(crate) fn apply_ops(st: &mut State, ops: &[Op]) -> Result<(), String> {
    for op in ops {
        match op {
            Op::Ddl(text) => {
                let stmts = SqlParser::new()
                    .parse_multiple(text)
                    .map_err(|e| format!("apply `{text}`: {e}"))?;
                for stmt in &stmts {
                    if let Some(Err(e)) = ddl::apply(st, None, None, stmt, text, true) {
                        return Err(format!("apply `{text}`: {}", e.message));
                    }
                }
            }
            Op::PutRow { table, rowid, row } => {
                let Some(name) = st.ids.get(table).cloned() else {
                    continue;
                };
                let t = st.tables.get_mut(&name).expect("table by id");
                let mut row = row.clone();
                row.resize(t.cols.len(), SqlValue::Null);
                if let Some(old) = t.rows.get(rowid).cloned() {
                    t.index_remove(*rowid, &old);
                }
                t.index_add(*rowid, &row);
                t.next_rowid = t.next_rowid.max(rowid + 1);
                t.rows.insert(*rowid, row);
            }
            Op::DelRow { table, rowid } => {
                let Some(name) = st.ids.get(table).cloned() else {
                    continue;
                };
                let t = st.tables.get_mut(&name).expect("table by id");
                if let Some(old) = t.rows.remove(rowid) {
                    t.index_remove(*rowid, &old);
                }
            }
            Op::DropRows { table } => {
                if let Some(name) = st.ids.get(table).cloned() {
                    let t = st.tables.get_mut(&name).expect("table by id");
                    t.rows.clear();
                    for i in &mut t.indexes {
                        i.map.clear();
                    }
                }
            }
            Op::SeqHighWater { .. } => {}
        }
    }
    st.version += 1;
    Ok(())
}

/// Statements that change data or schema, or lock rows, and so need the writer gate.
fn is_write(stmt: &Statement) -> bool {
    match stmt {
        Statement::Select(sel) => sel.for_clause.is_some(),
        _ => true,
    }
}

impl DurableEngine {
    /// An engine that keeps nothing across restarts.
    pub fn in_memory() -> Self {
        Self {
            writer: Arc::new(AsyncMutex::new(())),
            committed: Mutex::new(State::default()),
            seqs: Mutex::new(Seqs::default()),
            persist: Box::new(NoPersist),
        }
    }

    /// Open an engine over `persist`, rebuilding the working set from it.
    pub fn open(persist: Box<dyn Persist>) -> Result<Self, String> {
        let loaded = persist.load()?;
        let mut st = State::default();
        let mut seqs = Seqs::default();
        for (name, hwm) in loaded.seqs {
            seqs.set_reserved(&name, hwm);
        }
        seqs.resume_above_reserved();
        for text in &loaded.ddl {
            apply_ops(&mut st, &[Op::Ddl(text.clone())])?;
        }
        for (tid, rowid, mut row) in loaded.rows {
            let Some(name) = st.ids.get(&tid).cloned() else {
                continue;
            };
            let t = st.tables.get_mut(&name).expect("table by id");
            row.resize(t.cols.len(), SqlValue::Null);
            t.next_rowid = t.next_rowid.max(rowid + 1);
            t.rows.insert(rowid, row);
        }
        let names: Vec<String> = st.tables.keys().cloned().collect();
        for n in names {
            st.tables.get_mut(&n).expect("table").rebuild_indexes();
        }
        st.version = 0;
        Ok(Self {
            writer: Arc::new(AsyncMutex::new(())),
            committed: Mutex::new(st),
            seqs: Mutex::new(seqs),
            persist,
        })
    }

    fn snapshot(&self) -> State {
        self.committed.lock().expect("committed state").clone()
    }

    /// Apply ops that a replicated store has committed (on every node, including the proposer).
    /// Safe to call from the store's apply loop.
    pub fn apply_replicated(&self, ops: &[Op]) -> Result<(), String> {
        let mut c = self.committed.lock().expect("committed state");
        let mut next = c.clone();
        apply_ops(&mut next, ops)?;
        *c = next;
        drop(c);
        self.seqs.lock().expect("sequences").apply(ops);
        Ok(())
    }

    /// Call when this node becomes the leader of a replicated store.
    pub fn became_leader(&self) {
        self.seqs.lock().expect("sequences").resume_above_reserved();
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
        let st = self.snapshot();
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
        let tag = |t: &str| Ok(QueryResult::Command { tag: t.to_string() });
        match stmt {
            Statement::Begin(b) => {
                if session.txn.is_none() {
                    let pin = matches!(
                        b.isolation_level,
                        Some(IsolationLevel::RepeatableRead | IsolationLevel::Serializable)
                    );
                    session.txn = Some(Txn::new(pin));
                }
                tag("BEGIN")
            }
            Statement::Commit(_) => {
                let t = self.finish(session, true).await?;
                tag(&t)
            }
            Statement::Rollback(r) if r.to_savepoint.is_some() => {
                let name = r.to_savepoint.clone().unwrap_or_default();
                let txn = txn_block(session, "ROLLBACK TO SAVEPOINT")?;
                let Some(pos) = txn.savepoints.iter().rposition(|(n, _, _)| *n == name) else {
                    return Err(no_savepoint(&name).into());
                };
                let (_, saved, ops_len) = txn.savepoints[pos].clone();
                txn.savepoints.truncate(pos + 1);
                if saved.is_none() {
                    // the savepoint came before any write: give the writer gate back too
                    txn.guard = None;
                }
                txn.work = saved;
                txn.ops.truncate(ops_len);
                txn.failed = false;
                tag("ROLLBACK")
            }
            Statement::Rollback(_) => {
                let t = self.finish(session, false).await?;
                tag(&t)
            }
            Statement::Savepoint(sp) => {
                let txn = txn_block(session, "SAVEPOINT")?;
                if txn.failed {
                    return Err(aborted().into());
                }
                txn.savepoints
                    .push((sp.name.clone(), txn.work.clone(), txn.ops.len()));
                tag("SAVEPOINT")
            }
            Statement::ReleaseSavepoint(sp) => {
                let txn = txn_block(session, "RELEASE SAVEPOINT")?;
                let Some(pos) = txn.savepoints.iter().rposition(|(n, _, _)| *n == sp.name) else {
                    return Err(no_savepoint(&sp.name).into());
                };
                txn.savepoints.truncate(pos);
                tag("RELEASE")
            }
            _ => self
                .run_statement(session, stmt, text)
                .await
                .map_err(Into::into),
        }
    }

    async fn run_statement(
        &self,
        session: &mut Session,
        stmt: &Statement,
        text: &str,
    ) -> R<QueryResult> {
        let write = is_write(stmt);
        if let Some(txn) = session.txn.as_mut() {
            if txn.failed {
                return Err(aborted());
            }
            let r = self.run_in(txn, stmt, text, write).await;
            if r.is_err() {
                txn.failed = true;
            }
            return r;
        }
        let mut txn = Txn::new(false);
        let res = self.run_in(&mut txn, stmt, text, write).await?;
        if write {
            self.commit(&mut txn).await?;
        }
        Ok(res)
    }

    /// Run `stmt` inside `txn`. A statement that fails leaves the transaction's data as it was.
    async fn run_in(
        &self,
        txn: &mut Txn,
        stmt: &Statement,
        text: &str,
        write: bool,
    ) -> R<QueryResult> {
        if write {
            self.begin_write(txn).await?;
        } else if txn.pin && txn.work.is_none() && txn.base.is_none() {
            txn.base = Some(self.snapshot());
        }
        loop {
            let ops_before = txn.ops.len();
            let backup = txn.work.clone();
            let outcome = if write {
                let st = txn.work.as_mut().expect("a writing transaction has a copy");
                self.exec_write(st, &mut txn.ops, stmt, text)
            } else {
                let snap = match (&txn.work, &txn.base) {
                    (Some(w), _) => w.clone(),
                    (None, Some(b)) => b.clone(),
                    _ => self.snapshot(),
                };
                match stmt {
                    Statement::Select(sel) => query::run_select_statement(&snap, sel),
                    _ => err("0A000", "statement not supported"),
                }
            };
            match outcome {
                Ok(r) => return Ok(r),
                Err(e) => {
                    txn.work = backup;
                    txn.ops.truncate(ops_before);
                    if e.code == NEED_RESERVE {
                        self.reserve(&e.message).await?;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    fn exec_write(
        &self,
        st: &mut State,
        ops: &mut Vec<Op>,
        stmt: &Statement,
        text: &str,
    ) -> R<QueryResult> {
        match stmt {
            Statement::Select(sel) => query::run_select_statement(st, sel),
            Statement::Insert(ins) => dml::insert(st, ops, &self.seqs, ins),
            Statement::Update(upd) => dml::update(st, ops, upd),
            Statement::Delete(del) => dml::delete(st, ops, del),
            other => ddl::apply(st, Some(ops), Some(&self.seqs), other, text, false)
                .unwrap_or_else(|| err("0A000", "statement not supported")),
        }
    }

    /// Take the writer gate and a private copy of the committed state, once per transaction.
    async fn begin_write(&self, txn: &mut Txn) -> R<()> {
        if txn.guard.is_some() {
            return Ok(());
        }
        txn.guard = Some(Arc::clone(&self.writer).lock_owned().await);
        let committed = self.snapshot();
        if let Some(base) = &txn.base {
            if base.version != committed.version {
                txn.guard = None;
                return err(
                    "40001",
                    "could not serialize access due to concurrent update",
                );
            }
        }
        txn.work = Some(committed);
        Ok(())
    }

    /// Make a transaction's changes durable and visible.
    async fn commit(&self, txn: &mut Txn) -> R<()> {
        if txn.ops.is_empty() {
            txn.work = None;
            txn.guard = None;
            return Ok(());
        }
        let ops = std::mem::take(&mut txn.ops);
        self.persist.commit(&ops).await.map_err(|e| DbError {
            code: "58030",
            message: e,
        })?;
        if !self.persist.replicated() {
            // Writers are serialized, so the copy is exactly the committed state plus this transaction.
            if let Some(mut work) = txn.work.take() {
                let mut c = self.committed.lock().expect("committed state");
                work.version = c.version + 1;
                *c = work;
            }
            self.seqs.lock().expect("sequences").apply(&ops);
        }
        txn.work = None;
        txn.guard = None;
        txn.savepoints.clear();
        Ok(())
    }

    /// Reserve another block of values for a sequence, durably, before the statement is retried.
    async fn reserve(&self, name: &str) -> R<()> {
        let value = self.seqs.lock().expect("sequences").reserve_target(name);
        let op = Op::SeqHighWater {
            name: name.to_string(),
            value,
        };
        self.persist
            .commit(std::slice::from_ref(&op))
            .await
            .map_err(|e| DbError {
                code: "58030",
                message: e,
            })?;
        self.seqs
            .lock()
            .expect("sequences")
            .set_reserved(name, value);
        Ok(())
    }

    /// End the session's transaction. Returns the command tag.
    async fn finish(&self, session: &mut Session, commit: bool) -> R<String> {
        let Some(mut txn) = session.txn.take() else {
            return Ok(if commit { "COMMIT" } else { "ROLLBACK" }.into());
        };
        if commit && !txn.failed {
            self.commit(&mut txn).await?;
            return Ok("COMMIT".into());
        }
        Ok("ROLLBACK".into())
    }
}

fn txn_block<'a>(session: &'a mut Session, what: &str) -> R<&'a mut Txn> {
    match session.txn.as_mut() {
        Some(t) => Ok(t),
        None => err(
            "25P01",
            format!("{what} can only be used in transaction blocks"),
        ),
    }
}

fn no_savepoint(name: &str) -> DbError {
    DbError {
        code: "3B001",
        message: format!("savepoint \"{name}\" does not exist"),
    }
}

fn aborted() -> DbError {
    DbError {
        code: "25P02",
        message: "current transaction is aborted, commands ignored until end of transaction block"
            .into(),
    }
}

#[cfg(test)]
mod tests;
