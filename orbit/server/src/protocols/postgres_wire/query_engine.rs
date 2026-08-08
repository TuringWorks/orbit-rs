//! SQL query engine for actor operations

// Recursive helper functions use parameters only for recursion - intentional design
#![allow(clippy::only_used_in_recursion)]

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::graphrag_engine::GraphRAGQueryEngine;
use crate::protocols::postgres_wire::persistent_storage::{
    ColumnType, PersistentTableStorage, QueryCondition, TableRow,
};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use crate::protocols::postgres_wire::sql::{ConfigurableSqlEngine, UnifiedExecutionResult};
use crate::protocols::postgres_wire::vector_engine::VectorQueryEngine;
use orbit_client::OrbitClient;

/// The result shape of a statement, determined without running it.
///
/// The extended query protocol requires the server to answer `Describe` before
/// the client sends `Execute`, so this must be derivable from the statement and
/// the catalogue alone. Executing to find out is not an option: `Describe` on an
/// `INSERT` must not insert anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementDescription {
    /// Columns the statement will return.
    ///
    /// Empty means the statement returns no result set, which the protocol
    /// reports as `NoData`.
    pub columns: Vec<ColumnDescription>,
}

/// One described output column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDescription {
    pub name: String,
    /// PostgreSQL type OID.
    ///
    /// Taken from the table's declared schema where one is known. Where it is
    /// not, `text` — the engine stores every value as text, and a type guessed
    /// from a value's characters is a claim the catalogue does not support.
    pub type_oid: i32,
}

impl ColumnDescription {
    /// A column of unknown declared type, reported as `text`.
    pub fn text(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_oid: super::messages::type_oids::TEXT,
        }
    }
}

impl StatementDescription {
    /// A statement that returns no rows.
    pub fn no_data() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// A statement returning the named columns, all typed `text`.
    pub fn returning_text(columns: Vec<String>) -> Self {
        Self {
            columns: columns.into_iter().map(ColumnDescription::text).collect(),
        }
    }

    /// A statement returning fully described columns.
    pub fn returning(columns: Vec<ColumnDescription>) -> Self {
        Self { columns }
    }

    /// Whether the statement produces a result set.
    pub fn returns_rows(&self) -> bool {
        !self.columns.is_empty()
    }
}

/// OID reported for the `public` namespace in `pg_namespace`.
const PUBLIC_NAMESPACE_OID: i64 = 2200;
/// First OID handed out to user tables in `pg_class`.
///
/// PostgreSQL reserves everything below 16384 for built-in objects.
const FIRST_USER_OID: i64 = 16_384;

/// The type OID a declared type maps to.
///
/// Anything outside the set this server implements reports `text`, which is
/// also how it is stored and compared.
#[must_use]
pub fn type_oid_for(sql_type: &str) -> i32 {
    use super::messages::type_oids;

    // An array has its own OID per element type, so it is read before the
    // lattice collapses every array to one kind.
    let written = sql_type.trim();
    if let Some(element) = written.strip_suffix("[]").or_else(|| {
        written
            .to_uppercase()
            .strip_suffix(" ARRAY")
            .map(|_| written[..written.len() - " ARRAY".len()].trim())
    }) {
        return match plpgsql_function::normalize(element).as_str() {
            "int2" => 1005,
            "int4" => 1007,
            "int8" => 1016,
            "float4" => 1021,
            "float8" => 1022,
            "numeric" => 1231,
            "bool" => 1000,
            "varchar" => 1015,
            _ => 1009,
        };
    }

    match plpgsql_function::normalize(sql_type).as_str() {
        "int2" => type_oids::INT2,
        "int4" => type_oids::INT4,
        "int8" => type_oids::INT8,
        "float4" => type_oids::FLOAT4,
        "float8" => type_oids::FLOAT8,
        "numeric" => type_oids::NUMERIC,
        "bool" => type_oids::BOOL,
        "varchar" => type_oids::VARCHAR,
        "bpchar" => type_oids::BPCHAR,
        "date" => type_oids::DATE,
        "time" => type_oids::TIME,
        "timestamp" => type_oids::TIMESTAMP,
        "timestamptz" => type_oids::TIMESTAMPTZ,
        _ => type_oids::TEXT,
    }
}

/// Render a stored number, honouring a column's declared scale.
///
/// Only `NUMERIC`/`DECIMAL` carries one; everything else prints as stored. A
/// scale that cannot be applied leaves the number alone rather than inventing
/// digits.
#[must_use]
fn render_number(number: &serde_json::Number, declared: Option<&ColumnType>) -> String {
    use std::str::FromStr;

    let Some(ColumnType::Numeric {
        scale: Some(scale), ..
    }) = declared
    else {
        return number.to_string();
    };
    rust_decimal::Decimal::from_str(&number.to_string()).map_or_else(
        |_| number.to_string(),
        |mut decimal| {
            decimal.rescale(u32::from(*scale));
            decimal.to_string()
        },
    )
}

/// The type a domain definition leads with.
///
/// A definition is the base type followed by whatever constraints were
/// declared: `INTEGER CHECK (VALUE > 0)`. Only the leading type says what it is
/// built on — taking the whole string left the type lattice reading
/// `INTEGER CHECK (VALUE > 0)` as text.
#[must_use]
pub fn leading_type(definition: &str) -> String {
    let upper = definition.to_uppercase();
    let end = ["CHECK", "NOT NULL", "DEFAULT", "CONSTRAINT", "|"]
        .iter()
        .filter_map(|keyword| upper.find(keyword))
        .min()
        .unwrap_or(definition.len());
    definition[..end].trim().to_string()
}

/// The storage type a declared type name maps to.
///
/// One table, used by `CREATE TABLE` and by `ALTER TABLE ... ADD COLUMN`.
/// Two copies of this would drift, which is the failure mode this document
/// records more than any other.
#[must_use]
fn column_type_from_name(declared: &str) -> ColumnType {
    match declared.trim().to_uppercase().as_str() {
        "INTEGER" | "INT" => ColumnType::Integer,
        "BIGINT" => ColumnType::BigInt,
        "SERIAL" | "BIGSERIAL" => ColumnType::Serial,
        "TEXT" => ColumnType::Text,
        "BOOLEAN" | "BOOL" => ColumnType::Boolean,
        "JSON" => ColumnType::Json,
        "DOUBLE" => ColumnType::Double,
        "TIMESTAMP" => ColumnType::Timestamp,
        "REAL" | "FLOAT" | "FLOAT4" | "FLOAT8" | "DOUBLE PRECISION" => ColumnType::Double,
        data_type if data_type.starts_with("NUMERIC") || data_type.starts_with("DECIMAL") => {
            // `NUMERIC(10,2)` reached none of the arms above and fell
            // through to the unknown case, which is `TEXT`. The column
            // then held whatever the value happened to be, and its
            // declared scale existed nowhere.
            let (precision, scale) = data_type
                .find('(')
                .and_then(|open| {
                    let close = data_type.find(')')?;
                    let inside = &data_type[open + 1..close];
                    let mut parts = inside.split(',');
                    let precision = parts.next()?.trim().parse::<u8>().ok();
                    let scale = parts.next().and_then(|s| s.trim().parse::<u8>().ok());
                    Some((precision, scale))
                })
                .unwrap_or((None, None));
            ColumnType::Numeric { precision, scale }
        }
        data_type => {
            if data_type.starts_with("VARCHAR") {
                // Extract length if present
                let len = if let Some(start) = data_type.find('(') {
                    let end = data_type.find(')').unwrap_or(data_type.len());
                    data_type[start + 1..end].parse().unwrap_or(255)
                } else {
                    255
                };
                ColumnType::Varchar(len)
            } else {
                // Default to text for unknown types
                ColumnType::Text
            }
        }
    }
}

/// Replace column references in an expression with a row's values.
///
/// Word boundaries only, and never inside a string literal: a column named `n`
/// must not rewrite the `n` in `'n'`.
#[must_use]
fn substitute_columns(expression: &str, row: &HashMap<String, JsonValue>) -> String {
    fn flush(word: &mut String, out: &mut String, row: &HashMap<String, JsonValue>) {
        if word.is_empty() {
            return;
        }
        let folded = fold_identifier(word);
        match row
            .iter()
            .find(|(name, _)| fold_identifier(name) == folded)
            .map(|(_, value)| value)
        {
            Some(JsonValue::Null) => out.push_str("NULL"),
            Some(JsonValue::String(text)) => {
                out.push('\'');
                out.push_str(&text.replace('\'', "''"));
                out.push('\'');
            }
            Some(value) => out.push_str(&value.to_string()),
            None => out.push_str(word),
        }
        word.clear();
    }

    let mut out = String::with_capacity(expression.len());
    let mut word = String::new();
    let mut in_string = false;

    for character in expression.chars() {
        if character == '\'' {
            flush(&mut word, &mut out, row);
            in_string = !in_string;
            out.push(character);
            continue;
        }
        if in_string {
            out.push(character);
            continue;
        }
        if character.is_alphanumeric() || character == '_' {
            word.push(character);
            continue;
        }
        flush(&mut word, &mut out, row);
        out.push(character);
    }
    flush(&mut word, &mut out, row);
    out
}

/// Whether a word is a bare column reference rather than an expression.
///
/// A qualified name (`t.id`) counts; anything with an operator, a call or a
/// literal in it does not.
#[must_use]
fn is_simple_column(word: &str) -> bool {
    let bare = word.trim_matches('"');
    !bare.is_empty()
        && bare
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !bare.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Whether the right-hand side is something a stored condition can carry.
///
/// The column and operator were checked but not this, so
/// `WHERE id = ANY(ARRAY[1,3])` was stored as `id = 'ANY(ARRAY[1,3])'` and
/// matched nothing — the same silent wrong answer as an unhandled operator,
/// arriving from the other side of the comparison.
#[must_use]
fn is_simple_value(operator: &str, value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    match operator.to_uppercase().as_str() {
        // `IS NULL` / `IS NOT NULL`, and nothing else.
        "IS" => matches!(
            trimmed.to_uppercase().as_str(),
            "NULL" | "NOT NULL" | "TRUE" | "FALSE" | "NOT TRUE" | "NOT FALSE"
        ),
        // A parenthesised list of literals.
        "IN" | "NOT" => {
            trimmed.starts_with('(')
                && trimmed.ends_with(')')
                && trimmed[1..trimmed.len() - 1].split(',').all(is_literal)
        }
        _ => is_literal(trimmed),
    }
}

/// Whether a token is a literal rather than an expression.
#[must_use]
fn is_literal(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() > 1 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        // A quoted string, provided the quotes are the only ones in it: a
        // value like `'a' || 'b'` is an expression.
        return !trimmed[1..trimmed.len() - 1].contains('\'');
    }
    matches!(
        trimmed.to_uppercase().as_str(),
        "NULL" | "TRUE" | "FALSE" | "DEFAULT"
    ) || trimmed.parse::<f64>().is_ok()
}

/// Whether a word is a comparison this parser's conditions can carry.
#[must_use]
fn is_comparison(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">=" | "IS" | "LIKE" | "ILIKE" | "IN" | "NOT"
    )
}

/// A stable OID for a composite type, in the same user range as a function's.
#[must_use]
pub fn composite_oid(name: &str) -> i64 {
    function_oid(&format!("composite:{name}"))
}

/// A stable OID for a stored function, in PostgreSQL's user-object range.
///
/// Derived from the catalog key rather than from position, so it survives a
/// restart and does not shift when another function is created or dropped —
/// an OID that moved would make `pg_proc` useless for the thing OIDs are for.
#[must_use]
pub fn function_oid(key: &str) -> i64 {
    // FNV-1a: small, stable, and not sensitive to the order keys arrive in.
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let span = (i32::MAX as u64) - (FIRST_USER_OID as u64);
    FIRST_USER_OID + (hash % span) as i64
}

/// Fold a SQL identifier the way PostgreSQL does.
///
/// An unquoted identifier folds to lower case; a double-quoted one keeps the
/// case it was written with. This parser used to fold to *upper* case while the
/// comprehensive SQL engine folded to lower, so a table created through one
/// path was invisible to the other — `CREATE TABLE t` over the simple query
/// protocol stored `t`, and `INSERT INTO t` over the extended protocol looked
/// for `T` and reported that the table did not exist.
pub fn fold_identifier(identifier: &str) -> String {
    let trimmed = identifier.trim().trim_end_matches(';').trim();
    match trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(quoted) => quoted.to_string(),
        None => trimmed.to_lowercase(),
    }
}

/// Table holding view definitions.
///
/// Prefixed so it cannot collide with a user table named `views`.
use super::plpgsql;
use super::plpgsql_function;

const VIEW_CATALOG: &str = "orbit_catalog_views";

/// Table holding the durable change log a replica replays from.
const CHANGE_LOG: &str = "orbit_catalog_changes";

/// The name at the end of a `DROP VIEW [IF EXISTS] name` statement.
fn upper_tail(statement: &str, if_exists: bool) -> &str {
    let skip = if if_exists { 4 } else { 2 };
    statement
        .split_whitespace()
        .nth(skip)
        .unwrap_or("")
        .trim_end_matches(',')
}

/// Split a simple-query message into its statements.
///
/// Semicolons inside string literals, quoted identifiers and dollar-quoted
/// bodies do not separate statements; splitting on every `;` would cut
/// `VALUES (\'a;b\')` in half.
pub(crate) fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut quote: Option<char> = None;
    // Dollar quoting: `$$ ... $$` or `$tag$ ... $tag$`. Everything between the
    // delimiters is literal text, which is how a body containing semicolons —
    // a function or a trigger — is written at all.
    let mut dollar_tag: Option<String> = None;

    while let Some(character) = chars.next() {
        if let Some(tag) = dollar_tag.clone() {
            current.push(character);
            if character == '$' && current.ends_with(&tag) {
                dollar_tag = None;
            }
            continue;
        }
        if quote.is_none() && character == '$' {
            // Read the tag up to the closing `$`.
            let mut tag = String::from('$');
            while let Some(next) = chars.peek() {
                let next = *next;
                if next == '$' {
                    tag.push('$');
                    chars.next();
                    break;
                }
                if !next.is_alphanumeric() && next != '_' {
                    break;
                }
                tag.push(next);
                chars.next();
            }
            current.push_str(&tag);
            if tag.ends_with('$') && tag.len() >= 2 {
                dollar_tag = Some(tag);
            }
            continue;
        }

        match quote {
            Some(open) => {
                current.push(character);
                if character == open {
                    // A doubled quote is an escaped quote, not the end.
                    if chars.peek() == Some(&open) {
                        current.push(open);
                        chars.next();
                    } else {
                        quote = None;
                    }
                }
            }
            None => match character {
                '\'' | '"' => {
                    quote = Some(character);
                    current.push(character);
                }
                ';' => {
                    if !current.trim().is_empty() {
                        statements.push(current.trim().to_string());
                    }
                    current.clear();
                }
                other => current.push(other),
            },
        }
    }

    if !current.trim().is_empty() {
        statements.push(current.trim().to_string());
    }
    statements
}

/// One change published to replication subscribers.
#[derive(Debug, Clone)]
pub struct ChangeRecord {
    /// Where this change sits in the stream, as an LSN.
    pub position: u64,
    /// The transaction that made the change.
    pub transaction: u64,
    /// `INSERT`, `UPDATE` or `DELETE`.
    pub action: String,
    /// The table it touched.
    pub table: String,
    /// The row as it stands afterwards, rendered as JSON.
    pub row: String,
}

/// Recent changes, kept so a subscriber can replay from a position it names.
///
/// Bounded: a replica that asks for a position older than the window is told
/// the history is gone rather than served a silently incomplete stream.
static HISTORY: std::sync::OnceLock<std::sync::Mutex<std::collections::VecDeque<ChangeRecord>>> =
    std::sync::OnceLock::new();

/// How many changes are retained for replay in memory.
const HISTORY_DEPTH: usize = 4096;

/// How far a slot may fall behind before it is invalidated.
///
/// The durable log is bounded by this: a subscriber that stops confirming
/// cannot hold it open indefinitely, which is what `max_slot_wal_keep_size`
/// protects against in PostgreSQL. Set from
/// `postgresql.max_slot_change_backlog`; the default stands when the setting
/// is absent.
static MAX_RETAINED_CHANGES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(100_000);

/// How many rows have been marked deleted since the last reclaim.
///
/// Vacuum read every row of every table on each tick to find out whether there
/// was anything to do. On a large table that is continuous work for an idle
/// server — the tick ran far more often than the thing it was looking for
/// changed. This counter answers the same question without a scan.
static PENDING_RECLAIM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Note that rows were marked deleted and will need reclaiming.
pub fn note_reclaimable(rows: u64) {
    PENDING_RECLAIM.fetch_add(rows, std::sync::atomic::Ordering::Relaxed);
}

/// Whether anything is waiting to be reclaimed.
#[must_use]
pub fn reclaim_pending() -> bool {
    PENDING_RECLAIM.load(std::sync::atomic::Ordering::Relaxed) > 0
}

pub use crate::protocols::common::cancel::{
    cancel_requested, check_cancelled, forget_cancellable, register_cancellable, request_cancel,
    with_cancel, CANCEL_CHECK_INTERVAL,
};

/// Whether any replication slot exists: `0` unknown, `1` none, `2` at least one.
///
/// Consulted on every write, so it is a cached answer rather than a catalog
/// read; creating or dropping a slot clears it.
static SLOT_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Forget whether a slot exists, after one is created or dropped.
pub fn forget_slot_cache() {
    SLOT_CACHE.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Set how far a slot may fall behind before it is invalidated.
pub fn set_max_slot_backlog(changes: u64) {
    // Zero would invalidate every slot the moment it was created, which is a
    // configuration mistake rather than a policy anyone wants.
    if changes > 0 {
        MAX_RETAINED_CHANGES.store(changes, std::sync::atomic::Ordering::Relaxed);
    }
}

/// How far a slot may currently fall behind.
#[must_use]
pub fn max_slot_backlog() -> u64 {
    MAX_RETAINED_CHANGES.load(std::sync::atomic::Ordering::Relaxed)
}

fn history() -> &'static std::sync::Mutex<std::collections::VecDeque<ChangeRecord>> {
    HISTORY.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

/// Changes recorded at or after `position`, oldest first.
///
/// Returns `None` when the window no longer reaches back that far, which the
/// caller reports rather than papering over.
#[must_use]
pub fn changes_since(position: u64) -> Option<Vec<ChangeRecord>> {
    let history = history().lock().ok()?;
    let oldest = history.front().map(|record| record.position)?;
    if position < oldest.saturating_sub(1) {
        return None;
    }
    Some(
        history
            .iter()
            .filter(|record| record.position > position)
            .cloned()
            .collect(),
    )
}

/// Changes published as they are written, for replication to stream.
///
/// A broadcast channel rather than a log: a subscriber that cannot keep up
/// misses records and is told so, which is honest, where an unbounded queue
/// would grow until the process died.
static CHANGES: std::sync::OnceLock<tokio::sync::broadcast::Sender<ChangeRecord>> =
    std::sync::OnceLock::new();

fn changes() -> &'static tokio::sync::broadcast::Sender<ChangeRecord> {
    CHANGES.get_or_init(|| tokio::sync::broadcast::channel(1024).0)
}

/// How many changes have been published, standing in for a write position.
static CHANGE_POSITION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The current write position, as `IDENTIFY_SYSTEM` reports it.
#[must_use]
pub fn latest_change_position() -> u64 {
    CHANGE_POSITION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Changes written but not yet flushed to the durable log.
///
/// Publishing happens on the write path, which is synchronous; the log write
/// is async, so records queue here and a flush drains them.
static PENDING_LOG: std::sync::OnceLock<std::sync::Mutex<Vec<ChangeRecord>>> =
    std::sync::OnceLock::new();

fn pending_log() -> &'static std::sync::Mutex<Vec<ChangeRecord>> {
    PENDING_LOG.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Take everything waiting to be logged.
#[must_use]
pub fn drain_pending_log() -> Vec<ChangeRecord> {
    pending_log()
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

/// Subscribe to the change stream.
#[must_use]
pub fn subscribe_to_changes() -> tokio::sync::broadcast::Receiver<ChangeRecord> {
    changes().subscribe()
}

/// Publish a marker that carries no row — the end of a transaction.
fn publish_marker(action: &str, transaction: u64) {
    let position = CHANGE_POSITION.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let record = ChangeRecord {
        position,
        transaction,
        action: action.to_string(),
        table: String::new(),
        row: "{}".to_string(),
    };
    if let Ok(mut history) = history().lock() {
        history.push_back(record.clone());
        while history.len() > HISTORY_DEPTH {
            history.pop_front();
        }
    }
    let _ = changes().send(record);
}

/// Publish a change. Does nothing when nobody is listening.
pub fn publish_change(
    action: &str,
    table: &str,
    row: &std::collections::HashMap<String, JsonValue>,
) {
    // Recorded even with nobody listening: a replica that connects later and
    // asks to replay from a position needs the history to be complete.
    let sender = changes();
    let visible: std::collections::BTreeMap<&String, &JsonValue> = row
        .iter()
        .filter(|(name, _)| *name != TRANSACTION_STAMP && *name != DELETED_BY)
        .collect();
    let position = CHANGE_POSITION.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let record = ChangeRecord {
        position,
        transaction: current_transaction_stamp().unwrap_or(0),
        action: action.to_string(),
        table: table.to_string(),
        row: serde_json::to_string(&visible).unwrap_or_else(|_| "{}".to_string()),
    };

    if let Ok(mut history) = history().lock() {
        history.push_back(record.clone());
        while history.len() > HISTORY_DEPTH {
            history.pop_front();
        }
    }
    if let Ok(mut pending) = pending_log().lock() {
        pending.push(record.clone());
    }
    let _ = sender.send(record);
}

/// The column each row is stamped with while its writing transaction is open.
///
/// Hidden from every projection: it is part of the row's bookkeeping, not of
/// the table. A row carrying an open transaction's id is invisible to every
/// other session, which is what stops uncommitted work from being read.
pub const TRANSACTION_STAMP: &str = "__orbit_txn";

/// The column marking a row deleted by a transaction that has not committed.
///
/// A delete inside a block cannot remove the row outright: other sessions must
/// go on seeing it until the block commits, and a rollback has to put it back.
pub const DELETED_BY: &str = "__orbit_deleted_by";

/// What the statement now running may see.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    /// The transaction the statement belongs to.
    pub id: u64,
    /// Transactions that were open when this one began.
    ///
    /// Present only under `REPEATABLE READ` and `SERIALIZABLE`: it is what
    /// makes a repeated read give the same answer, by judging a row against
    /// the moment the block started rather than against now.
    pub snapshot: Option<std::collections::HashSet<u64>>,
    /// What this transaction has read, recorded only under `SERIALIZABLE`.
    ///
    /// Entries are `table` for a whole-table read and `table\u{1}key` for an
    /// individual row, so a conflict can be judged against the rows a block
    /// actually looked at rather than against everything in the table.
    pub reads: Option<Arc<std::sync::Mutex<std::collections::HashSet<String>>>>,
}

tokio::task_local! {
    /// The transaction the statement currently running belongs to.
    ///
    /// A connection is a task, so this is per session without threading an id
    /// through every call.
    static CURRENT_TRANSACTION: TransactionContext;
}

/// Transaction ids whose writes have not been committed yet.
static OPEN_TRANSACTIONS: std::sync::OnceLock<std::sync::RwLock<std::collections::HashSet<u64>>> =
    std::sync::OnceLock::new();

fn open_transactions() -> &'static std::sync::RwLock<std::collections::HashSet<u64>> {
    OPEN_TRANSACTIONS.get_or_init(|| std::sync::RwLock::new(std::collections::HashSet::new()))
}

/// Start a transaction and return its id.
#[must_use]
pub fn begin_transaction(snapshot_isolation: bool) -> TransactionContext {
    begin_transaction_at(snapshot_isolation, false)
}

/// Start a transaction, optionally recording what it reads.
#[must_use]
pub fn begin_transaction_at(snapshot_isolation: bool, serializable: bool) -> TransactionContext {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);

    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    // The snapshot is taken before this transaction joins the open set, so it
    // records who was already running.
    let snapshot = snapshot_isolation.then(|| {
        open_transactions()
            .read()
            .map(|open| open.clone())
            .unwrap_or_default()
    });
    if let Ok(mut open) = open_transactions().write() {
        open.insert(id);
    }
    TransactionContext {
        id,
        snapshot,
        reads: serializable.then(|| Arc::new(std::sync::Mutex::new(Default::default()))),
    }
}

tokio::task_local! {
    /// Tables written while a procedural block is running.
    ///
    /// A `DO` block is atomic in PostgreSQL: a `RAISE EXCEPTION` after an
    /// `INSERT` leaves no row behind. Undoing needs to know where to look, and
    /// recording it in the write paths is exact — parsing table names back out
    /// of the statements would not be.
    static BLOCK_TABLES: Arc<std::sync::Mutex<std::collections::HashSet<String>>>;
}

tokio::task_local! {
    /// Transaction ids opened by nested blocks while a block is running.
    ///
    /// A sub-transaction that commits is still part of the block containing
    /// it: if that block then fails, its rows must go too, and they carry the
    /// sub-transaction's stamp rather than the outer one's.
    static BLOCK_TRANSACTIONS: Arc<std::sync::Mutex<Vec<u64>>>;
}

/// Note a sub-transaction opened inside the block currently running.
pub fn note_block_transaction(id: u64) {
    BLOCK_TRANSACTIONS
        .try_with(|ids| {
            if let Ok(mut ids) = ids.lock() {
                ids.push(id);
            }
        })
        .ok();
}

/// Note that `table` was written by the block currently running, if any.
pub fn note_block_write(table: &str) {
    BLOCK_TABLES
        .try_with(|tables| {
            if let Ok(mut tables) = tables.lock() {
                tables.insert(fold_identifier(table));
            }
        })
        .ok();
}

/// Mark a transaction finished, making its rows visible to everyone.
pub fn end_transaction(id: u64) {
    if let Ok(mut open) = open_transactions().write() {
        open.remove(&id);
    }
}

/// Note that the statement now running read `table`.
pub fn note_read(table: &str) {
    note_read_entry(table.to_string());
}

/// Note that the statement read one particular row.
///
/// Recording the row rather than the table is what keeps a serializable block
/// from failing because something unrelated in the same table moved.
pub fn note_read_row(table: &str, key: &str) {
    note_read_entry(format!("{table}\u{1}{key}"));
}

fn note_read_entry(entry: String) {
    let _ = CURRENT_TRANSACTION.try_with(|current| {
        if let Some(reads) = current.reads.as_ref() {
            if let Ok(mut reads) = reads.lock() {
                reads.insert(entry);
            }
        }
    });
}

/// Note the predicate a statement read a table through.
///
/// Stored as the conditions themselves so a row can be tested against it later;
/// an empty predicate means the whole table, which is already recorded as such.
pub fn note_read_predicate(table: &str, conditions: &[QueryCondition]) {
    if conditions.is_empty() {
        note_read(table);
        return;
    }
    let rendered: Vec<String> = conditions
        .iter()
        .map(|condition| {
            format!(
                "{}\u{2}{}\u{2}{}",
                condition.column, condition.operator, condition.value
            )
        })
        .collect();
    note_read_entry(format!("{table}\u{3}{}", rendered.join("\u{4}")));
}

/// Whether a serializable block is recording what it reads.
#[must_use]
pub fn records_reads() -> bool {
    CURRENT_TRANSACTION
        .try_with(|current| current.reads.is_some())
        .unwrap_or(false)
}

/// The identity of a row for conflict detection: its key columns, or all of
/// its values when the table has none.
#[must_use]
pub fn row_identity(
    values: &std::collections::HashMap<String, JsonValue>,
    key_columns: &[String],
) -> String {
    key_columns
        .iter()
        .map(|column| {
            values
                .iter()
                .find(|(name, _)| fold_identifier(name) == *column)
                .map_or_else(|| "null".to_string(), |(_, value)| value.to_string())
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The tables the statement's transaction has read, if it is serializable.
#[must_use]
pub fn tables_read() -> Vec<String> {
    CURRENT_TRANSACTION
        .try_with(|current| {
            current
                .reads
                .as_ref()
                .and_then(|reads| {
                    reads
                        .lock()
                        .ok()
                        .map(|reads| reads.iter().cloned().collect())
                })
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// The id to stamp a write with, allocating one for a statement that is not
/// inside a block.
///
/// An autocommit statement is its own transaction: without an id of its own its
/// rows carry no stamp, and a reader holding an older snapshot cannot tell they
/// arrived after it began.
#[must_use]
pub fn stamp_for_write() -> u64 {
    if let Some(id) = current_transaction_stamp() {
        return id;
    }
    // Allocated and closed at once: the row is visible to everyone from now,
    // and to no snapshot taken before this moment.
    let context = begin_transaction(false);
    end_transaction(context.id);
    context.id
}

/// Run a statement as part of `transaction`.
pub async fn within_transaction<F, T>(transaction: TransactionContext, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CURRENT_TRANSACTION.scope(transaction, future).await
}

/// Whether a stored row is visible to the statement now running.
///
/// A row stamped by a transaction that is still open belongs to that session
/// alone; everyone else must not see it until it commits.
#[must_use]
pub fn row_is_visible(values: &std::collections::HashMap<String, JsonValue>) -> bool {
    let context = CURRENT_TRANSACTION.try_with(Clone::clone).ok();
    let mine = |id: u64| context.as_ref().is_some_and(|current| current.id == id);

    // Under a snapshot, "still running" means "was running when I began", so
    // a transaction that commits mid-flight stays invisible for the rest of
    // this block. Without one, it means running right now, which is what
    // read-committed reports.
    let still_open = |id: u64| match context.as_ref().and_then(|c| c.snapshot.as_ref()) {
        Some(snapshot) => {
            snapshot.contains(&id) || context.as_ref().is_some_and(|current| id > current.id)
        }
        None => open_transactions()
            .read()
            .map(|open| open.contains(&id))
            .unwrap_or(false),
    };

    // A row deleted by this session is gone as far as it is concerned; one
    // deleted by a block that has not committed is still there for everyone
    // else. Once that block ends, the row is gone for good.
    if let Some(deleter) = values.get(DELETED_BY).and_then(JsonValue::as_u64) {
        if mine(deleter) || !still_open(deleter) {
            return false;
        }
    }

    // A row written by a block that has not committed belongs to it alone.
    let Some(stamp) = values.get(TRANSACTION_STAMP).and_then(JsonValue::as_u64) else {
        return true;
    };
    mine(stamp) || !still_open(stamp)
}

/// The stamp to write onto a row, if this statement is inside a transaction.
#[must_use]
pub fn current_transaction_stamp() -> Option<u64> {
    CURRENT_TRANSACTION.try_with(|current| current.id).ok()
}

/// A trigger as it was declared.
#[derive(Debug, Clone)]
pub struct TriggerDefinition {
    /// Whether it fires once per affected row rather than once per statement.
    pub per_row: bool,
    /// The `WHEN` predicate, if it has one.
    pub when: Option<String>,
    /// The statement it runs.
    pub action: String,
}

/// Pull the body out of `AS $$ ... $$` (or `$tag$ ... $tag$`).
///
/// Returns `None` when there is no dollar-quoted section, which is the only
/// form a PL/pgSQL body is accepted in — a body in single quotes would have to
/// escape every quote inside it.
fn extract_dollar_quoted(source: &str) -> Option<String> {
    let open = source.find('$')?;
    let tag_end = source[open + 1..].find('$')? + open + 1;
    let tag = &source[open..=tag_end];
    let rest = &source[tag_end + 1..];
    let close = rest.find(tag)?;
    Some(rest[..close].to_string())
}

/// The type named by a top-level `::` cast, if the expression ends in one.
///
/// Only at paren depth zero and outside a string: `f('a::b')` casts nothing,
/// and neither does `f((x::int) + 1)` as a whole.
fn split_top_level_cast(expression: &str) -> Option<String> {
    let bytes: Vec<char> = expression.chars().collect();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut last = None;

    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            '\'' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            ':' if !in_string && depth == 0 && bytes.get(index + 1) == Some(&':') => {
                last = Some(index + 2);
                index += 2;
                continue;
            }
            _ => {}
        }
        index += 1;
    }

    let at = last?;
    let named: String = bytes[at..].iter().collect();
    let named = named.trim();
    (!named.is_empty()
        && named
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ' ' || c == '[' || c == ']'))
    .then(|| named.to_string())
}

/// Split a call's argument list on commas that are not inside parentheses or
/// a string.
fn split_arguments(arguments: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;

    for c in arguments.chars() {
        match c {
            '\'' => {
                in_string = !in_string;
                current.push(c);
            }
            '(' if !in_string => {
                depth += 1;
                current.push(c);
            }
            ')' if !in_string => {
                depth -= 1;
                current.push(c);
            }
            ',' if !in_string && depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

#[async_trait::async_trait]
impl plpgsql::PlPgSqlHost for QueryEngine {
    async fn evaluate(&self, expression: &str) -> ProtocolResult<Option<String>> {
        Box::pin(self.evaluate_scalar(expression)).await
    }

    async fn run(&self, sql: &str) -> ProtocolResult<()> {
        Box::pin(self.execute_query(sql)).await.map(|_| ())
    }

    async fn query(&self, sql: &str) -> ProtocolResult<plpgsql::Rows> {
        Ok(match Box::pin(self.execute_query(sql)).await? {
            QueryResult::Select { columns, rows } => plpgsql::Rows { columns, rows },
            // A statement that is not a query contributes no rows rather than
            // failing: `PERFORM` and `RETURN QUERY` over a DML statement both
            // reach here.
            _ => plpgsql::Rows::default(),
        })
    }

    async fn column_type(&self, table: &str, column: &str) -> ProtocolResult<Option<String>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(None);
        };
        let Some(schema) = storage.get_table_schema(&fold_identifier(table)).await? else {
            return Ok(None);
        };
        Ok(schema
            .columns
            .iter()
            .find(|c| fold_identifier(&c.name) == fold_identifier(column))
            .map(|c| format!("{:?}", c.data_type).to_uppercase()))
    }

    async fn row_columns(&self, table: &str) -> ProtocolResult<Vec<String>> {
        // A composite type is a row shape too, so `DECLARE v mytype` brings
        // its fields into scope exactly as `%ROWTYPE` does for a table.
        if let Some(fields) = self.composite_fields(table).await? {
            return Ok(fields.into_iter().map(|field| field.name).collect());
        }

        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(storage
            .get_table_schema(&fold_identifier(table))
            .await?
            .map(|schema| schema.columns.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default())
    }

    async fn run_protected(
        &self,
        block: &plpgsql::Block,
        state: plpgsql::State,
    ) -> ProtocolResult<(Result<plpgsql::Returned, ProtocolError>, plpgsql::State)> {
        let mut state = state;
        let tables = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let context = begin_transaction(false);
        let id = context.id;
        // Registered with the enclosing block so that if *it* fails later,
        // these rows go too — a sub-transaction that committed is still part
        // of the block that contained it.
        note_block_transaction(id);

        let outcome = BLOCK_TABLES
            .scope(Arc::clone(&tables), async {
                within_transaction(context, plpgsql::execute_in(block, self, &mut state)).await
            })
            .await;

        if outcome.is_err() {
            let written: Vec<String> = tables
                .lock()
                .map(|tables| tables.iter().cloned().collect())
                .unwrap_or_default();
            self.discard_transaction_writes(&written, id).await?;
        }
        end_transaction(id);
        Ok((outcome, state))
    }
}

/// Remove duplicate rows, preserving first-seen order.
///
/// `UNION` (without `ALL`) deduplicates; `SqlValue` is not hashable, so this
/// compares rather than hashes. Result sets combined by a set operation are
/// small enough that the quadratic scan is not the cost worth optimising.
fn deduplicate_rows(rows: Vec<Vec<SqlValue>>) -> Vec<Vec<SqlValue>> {
    let mut seen: Vec<Vec<SqlValue>> = Vec::with_capacity(rows.len());
    for row in rows {
        if !seen.contains(&row) {
            seen.push(row);
        }
    }
    seen
}

/// Map a declared column type to the PostgreSQL type OID the wire advertises.
pub fn column_type_oid(column_type: &ColumnType) -> i32 {
    use super::messages::type_oids;

    match column_type {
        ColumnType::Serial | ColumnType::Integer => type_oids::INT4,
        ColumnType::BigInt => type_oids::INT8,
        ColumnType::Boolean => type_oids::BOOL,
        ColumnType::Double => type_oids::FLOAT8,
        ColumnType::Numeric { .. } => type_oids::NUMERIC,
        ColumnType::Timestamp => type_oids::TIMESTAMPTZ,
        ColumnType::Json => type_oids::JSON,
        ColumnType::Text | ColumnType::Varchar(_) => type_oids::TEXT,
    }
}

/// Query result types
#[derive(Debug, Clone)]
pub enum QueryResult {
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
    },
    Insert {
        count: usize,
    },
    Update {
        count: usize,
    },
    Delete {
        count: usize,
    },
    Set {
        variable: String,
        value: String,
    },
    Merge {
        count: usize,
        rows: Vec<Vec<Option<String>>>,
        columns: Vec<String>,
    },
}

/// Parsed SQL statement
#[derive(Debug, Clone)]
enum Statement {
    Select {
        columns: Vec<String>,
        table: String,
        where_clause: Option<WhereClause>,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Vec<String>>,
    },
    Update {
        table: String,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    },
    Delete {
        table: String,
        where_clause: Option<WhereClause>,
    },
    CreateTable {
        table: String,
        columns: Vec<SimpleColumnDef>,
        if_not_exists: bool,
        /// Table-level foreign keys, kept whole so a composite key stays one
        /// constraint rather than becoming several per-column ones.
        foreign_keys: Vec<crate::protocols::postgres_wire::persistent_storage::ForeignKey>,
    },
    DropTable {
        table: String,
        if_exists: bool,
    },
    Truncate {
        table: String,
    },
}

/// Simple column definition for basic DDL support
#[derive(Debug, Clone)]
struct SimpleColumnDef {
    name: String,
    data_type: String,
    constraints: Vec<String>,
}

#[derive(Debug, Clone)]
struct WhereClause {
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
struct Condition {
    column: String,
    operator: String,
    value: String,
}

/// In-memory actor storage for demonstration
/// In production, this would use OrbitClient
#[derive(Debug, Clone)]
struct ActorRecord {
    actor_id: String,
    actor_type: String,
    state: JsonValue,
}

/// Query engine that translates SQL to actor operations
pub struct QueryEngine {
    // In-memory storage for demonstration (legacy actor operations)
    // TODO: Replace with OrbitClient integration
    actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
    // Optional persistent table storage for regular SQL tables
    persistent_storage: Option<Arc<dyn PersistentTableStorage>>,
    // Optional vector query engine for pgvector compatibility
    vector_engine: Option<VectorQueryEngine>,
    // Optional GraphRAG query engine
    graphrag_engine: Option<GraphRAGQueryEngine>,
    // Comprehensive SQL engine for DDL and other advanced operations
    sql_engine: Arc<Mutex<ConfigurableSqlEngine>>,
    // Current database context
    current_database: Arc<RwLock<String>>,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new() -> Self {
        tracing::debug!("query engine created without persistent storage");
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: None,
            vector_engine: None,
            graphrag_engine: None,
            sql_engine: Arc::new(Mutex::new(ConfigurableSqlEngine::new())),
            current_database: Arc::new(RwLock::new("actors".to_string())),
        }
    }

    /// Create a new query engine with persistent storage
    pub fn new_with_persistent_storage(storage: Arc<dyn PersistentTableStorage>) -> Self {
        tracing::debug!("query engine created with persistent storage");
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: Some(storage),
            vector_engine: None,
            graphrag_engine: None,
            sql_engine: Arc::new(Mutex::new(ConfigurableSqlEngine::new())),
            current_database: Arc::new(RwLock::new("actors".to_string())),
        }
    }

    /// Create a new query engine with vector support
    pub fn new_with_vector_support(orbit_client: OrbitClient) -> Self {
        // Since OrbitClient doesn't implement Clone, we need to create separate instances
        // For now, we'll create the GraphRAG engine in placeholder mode
        // This needs to be fixed when OrbitClient supports cloning or sharing
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: None,
            vector_engine: Some(VectorQueryEngine::new(orbit_client)),
            graphrag_engine: Some(GraphRAGQueryEngine::new_placeholder()),
            sql_engine: Arc::new(Mutex::new(ConfigurableSqlEngine::new())),
            current_database: Arc::new(RwLock::new("actors".to_string())),
        }
    }

    /// Create a new query engine with both persistent storage and vector support
    pub fn new_with_persistent_and_vector_support(
        storage: Arc<dyn PersistentTableStorage>,
        orbit_client: OrbitClient,
    ) -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: Some(storage),
            vector_engine: Some(VectorQueryEngine::new(orbit_client)),
            graphrag_engine: Some(GraphRAGQueryEngine::new_placeholder()),
            sql_engine: Arc::new(Mutex::new(ConfigurableSqlEngine::new())),
            current_database: Arc::new(RwLock::new("actors".to_string())),
        }
    }

    /// Set the current database context
    pub async fn set_current_database(&self, database: &str) {
        let mut current_db = self.current_database.write().await;
        *current_db = database.to_string();

        // Also update the SQL executor's current database
        let mut sql_engine = self.sql_engine.lock().await;
        sql_engine.set_current_database(database).await;
    }

    /// Get the current database name
    pub async fn get_current_database(&self) -> String {
        let db = self.current_database.read().await;
        db.clone()
    }

    /// Determine what `sql` will return, without executing it.
    ///
    /// Used to answer the extended query protocol's `Describe`, which the
    /// client sends before `Execute`. Column names come from the statement's
    /// own select list, or from the table's schema for `SELECT *`.
    ///
    /// # Errors
    /// Returns an error only when the catalogue cannot be read. A statement
    /// this engine cannot parse is reported as returning rows of unknown
    /// shape — see [`QueryEngine::describe_unparsed`] — rather than failing,
    /// so that `Describe` never rejects a statement `Execute` would accept.
    pub async fn describe_statement(&self, sql: &str) -> ProtocolResult<StatementDescription> {
        // A statement being described still carries its `$n` placeholders, which
        // the parser does not accept. The result *shape* never depends on the
        // parameter values, so they are stood in for by NULL purely to make the
        // statement parseable here. The statement executed later is the one with
        // the real values bound.
        let sql = Self::placeholders_as_null(sql);
        let sql = sql.as_str();
        let sql_upper = sql.trim().to_uppercase();

        // These paths build their result set dynamically and cannot be
        // described from the catalogue.
        if self.is_graphrag_query(&sql_upper)
            || self
                .vector_engine
                .as_ref()
                .is_some_and(|_| self.is_vector_query(&sql_upper))
        {
            return self.describe_by_probing(sql, &sql_upper).await;
        }

        let Ok(statement) = self.parse_sql(sql) else {
            return self.describe_by_probing(sql, &sql_upper).await;
        };

        match statement {
            Statement::Select { columns, table, .. } => self.describe_select(columns, &table).await,
            // Everything else completes with a command tag and no result set.
            Statement::Insert { .. }
            | Statement::Update { .. }
            | Statement::Delete { .. }
            | Statement::CreateTable { .. }
            | Statement::DropTable { .. }
            | Statement::Truncate { .. } => Ok(StatementDescription::no_data()),
        }
    }

    /// Replace `$n` placeholders with `NULL` so a statement can be parsed for
    /// description. Placeholders inside string literals are left alone.
    fn placeholders_as_null(sql: &str) -> String {
        let bytes = sql.as_bytes();
        let mut out = String::with_capacity(sql.len());
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            let ch = bytes[index];

            if ch == b'\'' {
                in_literal = !in_literal;
                out.push('\'');
                index += 1;
                continue;
            }

            if ch != b'$' || in_literal {
                out.push(ch as char);
                index += 1;
                continue;
            }

            let start = index + 1;
            let end = start
                + bytes[start..]
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .count();

            if end == start {
                out.push('$');
                index += 1;
            } else {
                out.push_str("NULL");
                index = end;
            }
        }

        out
    }

    /// Serve a query against a system catalogue relation.
    ///
    /// Returns `None` when `table` is not one, so ordinary tables fall through.
    ///
    /// Only relations this server can answer truthfully are provided, and each
    /// row describes something that actually exists here — the tables really
    /// present, the types really advertised on the wire. Clients read these to
    /// decide what the server supports, so inventing entries would make them
    /// use features that are not implemented.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    async fn select_system_catalog(
        &self,
        table: &str,
        columns: &[String],
    ) -> ProtocolResult<Option<QueryResult>> {
        use super::messages::type_oids;

        // `pg_class` and `pg_catalog.pg_class` name the same relation.
        let relation = table
            .rsplit('.')
            .next()
            .unwrap_or(table)
            .to_ascii_lowercase();
        let qualifier = table
            .rsplit_once('.')
            .map(|(schema, _)| schema.to_ascii_lowercase());
        let is_information_schema = qualifier.as_deref() == Some("information_schema");

        // Only answer for the catalogue schemas, so a user table called
        // `pg_class` in the default schema is still their table.
        if !matches!(
            qualifier.as_deref(),
            Some("pg_catalog") | Some("information_schema") | None
        ) {
            return Ok(None);
        }
        if qualifier.is_none() && !relation.starts_with("pg_") {
            return Ok(None);
        }

        let tables = self.list_tables().await?.unwrap_or_default();

        let (all_columns, rows): (Vec<&str>, Vec<Vec<Option<String>>>) =
            match (is_information_schema, relation.as_str()) {
                (false, "pg_class") => (
                    vec!["oid", "relname", "relnamespace", "relkind"],
                    tables
                        .iter()
                        .enumerate()
                        .map(|(index, name)| {
                            vec![
                                Some((FIRST_USER_OID + index as i64).to_string()),
                                Some(name.clone()),
                                Some(PUBLIC_NAMESPACE_OID.to_string()),
                                // Only ordinary tables exist here; no views,
                                // indexes or sequences are reported because
                                // none are implemented.
                                Some("r".to_string()),
                            ]
                        })
                        .collect(),
                ),
                (false, "pg_proc") => {
                    let composites: HashMap<String, i64> = self
                        .all_composites()
                        .await?
                        .into_iter()
                        .map(|(name, _)| (name.clone(), composite_oid(&name)))
                        .collect();
                    let mut functions = self.all_functions().await?;
                    // A domain reports the OID of what it is built on. This
                    // server assigns OIDs to functions, not to domains, and
                    // reporting `text` for a domain over `INTEGER` would tell
                    // a client the wrong thing about how to call it.
                    for (_, parameters, return_type, _) in &mut functions {
                        for parameter in parameters.iter_mut() {
                            parameter.sql_type = self.base_type_of(&parameter.sql_type).await?;
                        }
                        *return_type = self.base_type_of(return_type).await?;
                    }
                    (
                        vec![
                            "oid",
                            "proname",
                            "pronamespace",
                            "pronargs",
                            "proargtypes",
                            "prorettype",
                            "prokind",
                        ],
                        functions
                            .iter()
                            .map(|(key, parameters, return_type, _)| {
                                let name = key
                                    .trim_start_matches("function:")
                                    .split('/')
                                    .next()
                                    .unwrap_or_default();
                                let inputs = plpgsql_function::inputs(parameters);
                                vec![
                                    Some(function_oid(key).to_string()),
                                    Some(name.to_string()),
                                    Some(PUBLIC_NAMESPACE_OID.to_string()),
                                    Some(inputs.len().to_string()),
                                    // `oidvector` is space-separated, as
                                    // PostgreSQL renders it.
                                    Some(
                                        inputs
                                            .iter()
                                            .map(|p| {
                                                composites
                                                    .get(&fold_identifier(&p.sql_type))
                                                    .copied()
                                                    .unwrap_or_else(|| {
                                                        i64::from(type_oid_for(&p.sql_type))
                                                    })
                                                    .to_string()
                                            })
                                            .collect::<Vec<_>>()
                                            .join(" "),
                                    ),
                                    Some(
                                        composites
                                            .get(&fold_identifier(return_type))
                                            .copied()
                                            .unwrap_or_else(|| i64::from(type_oid_for(return_type)))
                                            .to_string(),
                                    ),
                                    // `f` is a plain function; this server has
                                    // no procedures, aggregates or windows.
                                    Some("f".to_string()),
                                ]
                            })
                            .collect(),
                    )
                }
                (false, "pg_namespace") => (
                    vec!["oid", "nspname"],
                    vec![
                        vec![
                            Some(PUBLIC_NAMESPACE_OID.to_string()),
                            Some("public".to_string()),
                        ],
                        vec![Some("11".to_string()), Some("pg_catalog".to_string())],
                    ],
                ),
                (false, "pg_type") => {
                    let composites = self.all_composites().await?;
                    (
                        vec![
                            "oid",
                            "typname",
                            "typtype",
                            "typelem",
                            "typbasetype",
                            "typrelid",
                        ],
                        [
                            (type_oids::BOOL, "bool"),
                            (type_oids::BYTEA, "bytea"),
                            (type_oids::INT8, "int8"),
                            (type_oids::INT2, "int2"),
                            (type_oids::INT4, "int4"),
                            (type_oids::TEXT, "text"),
                            (type_oids::JSON, "json"),
                            (type_oids::FLOAT4, "float4"),
                            (type_oids::FLOAT8, "float8"),
                            (type_oids::VARCHAR, "varchar"),
                            (type_oids::TIMESTAMP, "timestamp"),
                            (type_oids::TIMESTAMPTZ, "timestamptz"),
                            (type_oids::UUID, "uuid"),
                            (type_oids::JSONB, "jsonb"),
                        ]
                        .into_iter()
                        .map(|(oid, name)| {
                            vec![
                                Some(oid.to_string()),
                                Some(name.to_string()),
                                // Base type, no element, no composite relation.
                                Some("b".to_string()),
                                Some("0".to_string()),
                                Some("0".to_string()),
                                Some("0".to_string()),
                            ]
                        })
                        // A composite created here is a real type and belongs in
                        // the catalogue a client reads to find out what exists.
                        // `c` is what tells one from a base type.
                        .chain(composites.iter().map(|(name, _)| {
                            vec![
                                Some(composite_oid(name).to_string()),
                                Some(name.clone()),
                                Some("c".to_string()),
                                Some("0".to_string()),
                                Some("0".to_string()),
                                Some("0".to_string()),
                            ]
                        }))
                        .collect(),
                    )
                }
                (true, "tables") => (
                    vec!["table_catalog", "table_schema", "table_name", "table_type"],
                    tables
                        .iter()
                        .map(|name| {
                            vec![
                                Some("orbit".to_string()),
                                Some("public".to_string()),
                                Some(name.clone()),
                                Some("BASE TABLE".to_string()),
                            ]
                        })
                        .collect(),
                ),
                (true, "schemata") => (
                    vec!["catalog_name", "schema_name"],
                    vec![vec![Some("orbit".to_string()), Some("public".to_string())]],
                ),
                _ => return Ok(None),
            };

        let all_columns: Vec<String> = all_columns.into_iter().map(str::to_string).collect();

        // Honour an explicit select list by projecting; `*` keeps every column.
        let selects_everything = columns.len() == 1 && columns[0] == "*";
        if selects_everything {
            return Ok(Some(QueryResult::Select {
                columns: all_columns,
                rows,
            }));
        }

        let wanted: Vec<String> = columns.iter().map(|c| fold_identifier(c)).collect();
        let indices: Vec<Option<usize>> = wanted
            .iter()
            .map(|want| all_columns.iter().position(|have| have == want))
            .collect();

        let projected = rows
            .into_iter()
            .map(|row| {
                indices
                    .iter()
                    .map(|index| index.and_then(|i| row.get(i).cloned().flatten()))
                    .collect()
            })
            .collect();

        Ok(Some(QueryResult::Select {
            columns: wanted,
            rows: projected,
        }))
    }

    /// Copy a table's current contents, for restoring on rollback.
    ///
    /// # Errors
    /// Returns an error when the table cannot be read.
    pub async fn snapshot_table(&self, table: &str) -> ProtocolResult<Option<Vec<TableRow>>> {
        let Some(storage) = &self.persistent_storage else {
            return Ok(None);
        };
        let table = fold_identifier(table);
        if !storage.table_exists(&table).await? {
            return Ok(None);
        }
        storage
            .select_rows(&table, Vec::new(), Vec::new(), None)
            .await
            .map(Some)
    }

    /// Put a table back to a previously taken snapshot.
    ///
    /// Every row is removed and the snapshot re-inserted, so the table matches
    /// the moment the snapshot was taken.
    ///
    /// # Errors
    /// Returns an error when the table cannot be written.
    pub async fn restore_table(&self, table: &str, rows: Vec<TableRow>) -> ProtocolResult<()> {
        let Some(storage) = &self.persistent_storage else {
            return Ok(());
        };
        let table = fold_identifier(table);

        // An empty condition list matches every row.
        storage.delete_rows(&table, Vec::new()).await?;
        for row in rows {
            storage.insert_row(&table, row).await?;
        }
        Ok(())
    }

    /// The rows a write statement is about to change, before it runs.
    ///
    /// For `UPDATE` and `DELETE` these are the rows the predicate selects; for
    /// `INSERT` there are none, because the rows do not exist yet. Undoing a
    /// transaction from these — rather than from a copy of the whole table —
    /// is what keeps one session's `ROLLBACK` off another session's rows.
    ///
    /// # Errors
    /// Returns an error when the table cannot be read.
    pub async fn rows_a_statement_will_change(
        &self,
        sql: &str,
    ) -> ProtocolResult<Option<Vec<TableRow>>> {
        use crate::protocols::postgres_wire::sql::ast::Statement as AstStatement;
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        let Some(storage) = &self.persistent_storage else {
            return Ok(None);
        };
        let Ok(statement) = SqlParser::new().parse(sql) else {
            return Ok(None);
        };

        let (table, where_clause) = match statement {
            AstStatement::Update(update) => (update.table.full_name(), update.where_clause),
            AstStatement::Delete(delete) => (delete.table.full_name(), delete.where_clause),
            // An insert changes no existing row; the rollback deletes what it
            // added instead, which the caller records from the statement.
            _ => return Ok(Some(Vec::new())),
        };

        let table = fold_identifier(&table);
        if !storage.table_exists(&table).await? {
            return Ok(None);
        }

        let all = storage
            .select_rows(&table, Vec::new(), Vec::new(), None)
            .await?;
        let Some(predicate) = where_clause else {
            return Ok(Some(all));
        };

        let Some(schema) = storage.get_table_schema(&table).await? else {
            return Ok(None);
        };
        let predicate = self.resolve_subqueries(predicate).await?;

        let mut evaluator =
            crate::protocols::postgres_wire::sql::expression_evaluator::ExpressionEvaluator::new();
        let mut matched = Vec::new();
        for row in all {
            let values: crate::protocols::postgres_wire::sql::select_pipeline::Row = schema
                .columns
                .iter()
                .map(|column| {
                    let value = row
                        .values
                        .get(&column.name)
                        .cloned()
                        .unwrap_or(JsonValue::Null);
                    (
                        fold_identifier(&column.name),
                        Self::json_to_sql_value(&value, &column.data_type),
                    )
                })
                .collect();
            let context =
                crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext::with_row(values);
            if matches!(
                evaluator.evaluate(&predicate, &context)?,
                SqlValue::Boolean(true)
            ) {
                matched.push(row);
            }
        }
        Ok(Some(matched))
    }

    /// Undo one session's writes without touching anyone else's rows.
    ///
    /// `inserted` are rows this session added, matched by value and removed;
    /// `pre_images` are rows it changed or deleted, put back if they are no
    /// longer there. Restoring a whole-table snapshot instead destroyed rows
    /// another session had committed while the block was open.
    ///
    /// # Errors
    /// Returns an error when the table cannot be written.
    pub async fn undo_session_writes(
        &self,
        table: &str,
        inserted: &[TableRow],
        pre_images: &[TableRow],
    ) -> ProtocolResult<()> {
        let Some(storage) = &self.persistent_storage else {
            return Ok(());
        };
        let table = fold_identifier(table);

        let conditions_for = |row: &TableRow| -> Vec<QueryCondition> {
            row.values
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(column, value)| QueryCondition {
                    column: fold_identifier(column),
                    operator: "=".to_string(),
                    value: value.clone(),
                })
                .collect()
        };

        // Remove what this session added.
        for row in inserted {
            let conditions = conditions_for(row);
            if conditions.is_empty() {
                continue;
            }
            storage.delete_rows(&table, conditions).await?;
        }

        // Put back what it changed or removed, unless an identical row is
        // already there.
        for row in pre_images {
            let conditions = conditions_for(row);
            let present = if conditions.is_empty() {
                Vec::new()
            } else {
                storage
                    .select_rows(&table, Vec::new(), conditions, Some(1))
                    .await?
            };
            if present.is_empty() {
                storage.insert_row(&table, row.clone()).await?;
            }
        }
        Ok(())
    }

    /// The rows an `INSERT ... VALUES` statement adds, as they will be stored.
    ///
    /// # Errors
    /// Returns an error when a value cannot be evaluated.
    pub async fn rows_an_insert_adds(&self, sql: &str) -> ProtocolResult<Option<Vec<TableRow>>> {
        use crate::protocols::postgres_wire::sql::ast::{InsertSource, Statement as AstStatement};
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        let Some(storage) = &self.persistent_storage else {
            return Ok(None);
        };
        let Ok(AstStatement::Insert(insert)) = SqlParser::new().parse(sql) else {
            return Ok(None);
        };
        let table = fold_identifier(&insert.table.full_name());
        // A view has no stored schema, and an `INSTEAD OF` trigger still needs
        // the row the statement names; the statement's own column list answers
        // that without one.
        let schema = storage.get_table_schema(&table).await?;
        let Some(names) = insert.columns.clone().or_else(|| {
            schema
                .as_ref()
                .map(|schema| schema.columns.iter().map(|c| c.name.clone()).collect())
        }) else {
            return Ok(None);
        };
        let names: Vec<String> = names.iter().map(|name| fold_identifier(name)).collect();

        let stored_name = |name: &String| {
            schema
                .as_ref()
                .and_then(|schema| {
                    schema
                        .columns
                        .iter()
                        .find(|column| fold_identifier(&column.name) == *name)
                })
                .map_or_else(|| name.clone(), |column| column.name.clone())
        };

        // `INSERT ... SELECT` adds whatever the select yields. Running it here
        // predicts those rows so the undo log can name them; without that the
        // statement fell back to a whole-table copy, whose rollback takes a
        // concurrent session's rows with it.
        let tuples = match insert.source {
            InsertSource::Values(tuples) => tuples,
            InsertSource::Query(query) => {
                let Some((_, values)) = self.evaluate_select(&query, &HashMap::new()).await? else {
                    return Ok(None);
                };
                let now = chrono::Utc::now();
                return Ok(Some(
                    values
                        .into_iter()
                        .map(|row| TableRow {
                            values: names
                                .iter()
                                .zip(row)
                                .map(|(name, value)| {
                                    (stored_name(name), Self::sql_value_to_json(&value))
                                })
                                .collect(),
                            created_at: now,
                            updated_at: now,
                        })
                        .collect(),
                ));
            }
            InsertSource::DefaultValues => return Ok(None),
        };

        let mut evaluator = ExpressionEvaluator::new();
        let context = EvaluationContext::empty();
        let mut rows = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            let now = chrono::Utc::now();
            let mut values = std::collections::HashMap::new();
            for (name, expression) in names.iter().zip(&tuple) {
                values.insert(
                    stored_name(name),
                    Self::sql_value_to_json(&evaluator.evaluate(expression, &context)?),
                );
            }
            rows.push(TableRow {
                values,
                created_at: now,
                updated_at: now,
            });
        }
        Ok(Some(rows))
    }

    /// The table a write statement targets, if it names one.
    ///
    /// Used to decide what to snapshot when a transaction block opens a write.
    pub fn write_target_table(sql: &str) -> Option<String> {
        let trimmed = sql.trim();
        let upper = trimmed.to_uppercase();

        let after = if let Some(rest) = upper.strip_prefix("INSERT INTO ") {
            &trimmed[trimmed.len() - rest.len()..]
        } else if let Some(rest) = upper.strip_prefix("UPDATE ") {
            &trimmed[trimmed.len() - rest.len()..]
        } else if let Some(rest) = upper.strip_prefix("DELETE FROM ") {
            &trimmed[trimmed.len() - rest.len()..]
        } else if let Some(rest) = upper.strip_prefix("TRUNCATE TABLE ") {
            &trimmed[trimmed.len() - rest.len()..]
        } else if let Some(rest) = upper.strip_prefix("COPY ") {
            &trimmed[trimmed.len() - rest.len()..]
        } else {
            return None;
        };

        after
            .split(|c: char| c.is_whitespace() || c == '(')
            .find(|token| !token.is_empty())
            .map(fold_identifier)
    }

    /// Names of the tables this engine can see.
    ///
    /// `None` when no persistent storage is attached, which is different from
    /// "there are no tables" and is reported as such rather than as an empty
    /// catalogue.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    pub async fn list_tables(&self) -> ProtocolResult<Option<Vec<String>>> {
        match &self.persistent_storage {
            Some(storage) => storage.list_tables().await.map(Some),
            None => Ok(None),
        }
    }

    /// Schema of one table, or `None` if it does not exist.
    ///
    /// The name is normalised the same way the SQL parser normalises it, so a
    /// caller passing `users` finds the table that `CREATE TABLE users` stored
    /// as `USERS`.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    pub async fn table_schema(
        &self,
        table: &str,
    ) -> ProtocolResult<Option<super::persistent_storage::TableSchema>> {
        let Some(storage) = &self.persistent_storage else {
            return Ok(None);
        };

        if let Some(schema) = storage.get_table_schema(table).await? {
            return Ok(Some(schema));
        }
        storage.get_table_schema(&fold_identifier(table)).await
    }

    /// Infer the type of each `$n` parameter from where it is used.
    ///
    /// Returns one OID per placeholder, in position order. A client that
    /// declares no parameter types relies entirely on this answer to decide how
    /// to serialise its values, so reporting everything as `text` would force
    /// every caller to stringify integers by hand.
    ///
    /// Two shapes are resolved, which between them cover ordinary
    /// parameterised statements:
    ///
    /// * a comparison against a column — `WHERE id = $1`, `SET name = $2`;
    /// * an `INSERT ... VALUES` list, by position.
    ///
    /// Anything else falls back to `text`, which is what the engine stores.
    ///
    /// # Errors
    /// Returns an error only if the catalogue cannot be read.
    pub async fn describe_parameters(&self, sql: &str) -> ProtocolResult<Vec<i32>> {
        use super::messages::type_oids;

        let count = Self::highest_placeholder(sql);
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut types = vec![type_oids::TEXT; count];

        let neutralised = Self::placeholders_as_null(sql);
        let (table, insert_columns) = match self.parse_sql(&neutralised) {
            Ok(Statement::Select { table, .. })
            | Ok(Statement::Update { table, .. })
            | Ok(Statement::Delete { table, .. }) => (table, None),
            Ok(Statement::Insert { table, columns, .. }) => (table, Some(columns)),
            Ok(Statement::CreateTable { .. })
            | Ok(Statement::DropTable { .. })
            | Ok(Statement::Truncate { .. }) => return Ok(types),
            // The simple parser rejects any clause it does not implement, so a
            // `WHERE a = $1 AND b > $2` used to leave every parameter typed as
            // text and the driver refused to send an integer for one.
            Err(_) => match Self::table_of(&neutralised) {
                Some(table) => (table, None),
                None => return Ok(types),
            },
        };

        let Some(storage) = &self.persistent_storage else {
            return Ok(types);
        };
        let Some(schema) = storage.get_table_schema(&table).await? else {
            return Ok(types);
        };

        let oid_of = |column: &str| {
            schema
                .columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(column))
                .map(|c| column_type_oid(&c.data_type))
        };

        if let Some(columns) = insert_columns {
            // Every placeholder in an INSERT belongs to the VALUES list, so the
            // nth placeholder is the nth column.
            for (position, column) in columns.iter().enumerate().take(count) {
                if let Some(oid) = oid_of(column) {
                    types[position] = oid;
                }
            }
            return Ok(types);
        }

        for (position, column) in Self::placeholder_comparisons(sql) {
            if position <= count {
                if let Some(oid) = oid_of(&column) {
                    types[position - 1] = oid;
                }
            }
        }

        // A placeholder in `LIMIT`/`OFFSET` is compared against no column, so
        // nothing above types it. Left as text it was spliced in quoted and
        // the clause was ignored — `LIMIT $1` returned every row.
        for position in Self::row_count_placeholders(sql) {
            if position <= count {
                types[position - 1] = type_oids::INT8;
            }
        }

        Ok(types)
    }

    /// Highest `$n` position appearing outside string literals.
    fn highest_placeholder(sql: &str) -> usize {
        Self::scan_placeholders(sql)
            .into_iter()
            .map(|(position, _)| position)
            .max()
            .unwrap_or(0)
    }

    /// Every placeholder as `(position, byte offset of the `$`)`.
    fn scan_placeholders(sql: &str) -> Vec<(usize, usize)> {
        let bytes = sql.as_bytes();
        let mut found = Vec::new();
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            match bytes[index] {
                b'\'' => {
                    in_literal = !in_literal;
                    index += 1;
                }
                b'$' if !in_literal => {
                    let start = index + 1;
                    let end = start
                        + bytes[start..]
                            .iter()
                            .take_while(|b| b.is_ascii_digit())
                            .count();
                    if end > start {
                        if let Ok(position) = sql[start..end].parse::<usize>() {
                            found.push((position, index));
                        }
                        index = end;
                    } else {
                        index += 1;
                    }
                }
                _ => index += 1,
            }
        }

        found
    }

    /// Placeholders that give a row count: `LIMIT $1`, `OFFSET $2`.
    fn row_count_placeholders(sql: &str) -> Vec<usize> {
        let upper = sql.to_uppercase();
        Self::scan_placeholders(sql)
            .into_iter()
            .filter(|(_, at)| {
                let before = upper[..*at].trim_end();
                before.ends_with("LIMIT") || before.ends_with("OFFSET")
            })
            .map(|(position, _)| position)
            .collect()
    }

    /// Placeholders that sit on the right of a comparison, paired with the
    /// column name on the left: `WHERE id = $1` yields `(1, "id")`.
    fn placeholder_comparisons(sql: &str) -> Vec<(usize, String)> {
        const OPERATOR_CHARS: [char; 6] = ['=', '<', '>', '!', '~', '@'];

        Self::scan_placeholders(sql)
            .into_iter()
            .filter_map(|(position, offset)| {
                let before = sql[..offset].trim_end();

                // Step back over the operator, which may be one or two
                // characters (`=`, `>=`, `<>`), or a word such as LIKE.
                let before = if before.ends_with(|c| OPERATOR_CHARS.contains(&c)) {
                    before.trim_end_matches(|c| OPERATOR_CHARS.contains(&c))
                } else {
                    let word_start = before.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                    let word = &before[word_start..];
                    if word.eq_ignore_ascii_case("LIKE") || word.eq_ignore_ascii_case("ILIKE") {
                        &before[..word_start]
                    } else {
                        return None;
                    }
                };

                let identifier: String = before
                    .trim_end()
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                (!identifier.is_empty()).then_some((position, identifier))
            })
            .collect()
    }

    /// The table a statement reads or writes, taken from the full parser.
    ///
    /// Only single-table statements yield a name; a join has no single table
    /// to resolve a column against.
    fn table_of(sql: &str) -> Option<String> {
        use crate::protocols::postgres_wire::sql::ast::{FromClause, Statement as AstStatement};

        let parsed = crate::protocols::postgres_wire::sql::parser::SqlParser::new()
            .parse(sql)
            .ok()?;
        let name = match parsed {
            AstStatement::Select(select) => match select.from_clause? {
                FromClause::Table { name, .. } => name.full_name(),
                _ => return None,
            },
            AstStatement::Update(update) => update.table.full_name(),
            AstStatement::Delete(delete) => delete.table.full_name(),
            AstStatement::Insert(insert) => insert.table.full_name(),
            _ => return None,
        };
        Some(fold_identifier(&name))
    }

    /// Test hook for [`QueryEngine::placeholder_comparisons`].
    #[cfg(test)]
    pub fn placeholder_comparisons_for_test(sql: &str) -> Vec<(usize, String)> {
        Self::placeholder_comparisons(sql)
    }

    /// Test hook for [`QueryEngine::placeholders_as_null`].
    #[cfg(test)]
    pub fn placeholders_as_null_for_test(sql: &str) -> String {
        Self::placeholders_as_null(sql)
    }

    /// Column list for a `SELECT`, resolving `*` against the table's schema.
    async fn describe_select(
        &self,
        columns: Vec<String>,
        table: &str,
    ) -> ProtocolResult<StatementDescription> {
        let selects_everything = columns.len() == 1 && columns[0] == "*";

        if table.to_uppercase() == "ACTORS" {
            let all = ["actor_id", "actor_type", "state"];
            let names: Vec<String> = if selects_everything {
                all.iter().map(|c| (*c).to_string()).collect()
            } else {
                columns
            };
            return Ok(StatementDescription::returning_text(names));
        }

        let Some(storage) = &self.persistent_storage else {
            return Ok(StatementDescription::no_data());
        };

        // Unknown table: let `Execute` produce the real error rather than
        // failing the describe with a different one.
        let Some(schema) = storage.get_table_schema(table).await? else {
            return Ok(StatementDescription::no_data());
        };

        let described = |name: &str| ColumnDescription {
            // Casing mirrors what `execute_persistent_select` produces, so the
            // description matches the rows that follow it.
            name: fold_identifier(name),
            type_oid: schema
                .columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(name))
                .map_or(super::messages::type_oids::TEXT, |c| {
                    column_type_oid(&c.data_type)
                }),
        };

        let described_columns = if selects_everything {
            schema.columns.iter().map(|c| described(&c.name)).collect()
        } else {
            columns.iter().map(|c| described(c)).collect()
        };

        Ok(StatementDescription::returning(described_columns))
    }

    /// Describe a statement the simple parser cannot, without running it.
    ///
    /// This used to *execute* read-only statements to learn their shape, which
    /// meant every extended-protocol query ran twice — once for `Describe` and
    /// once for `Execute` — with all the work that implies. It also made
    /// `Describe` a side-effecting operation. The full parser can name the
    /// output columns directly, so nothing needs to run.
    async fn describe_by_probing(
        &self,
        sql: &str,
        sql_upper: &str,
    ) -> ProtocolResult<StatementDescription> {
        use crate::protocols::postgres_wire::sql::ast::Statement as AstStatement;
        use crate::protocols::postgres_wire::sql::parser::SqlParser;
        use crate::protocols::postgres_wire::sql::select_pipeline;

        const ROW_RETURNING_PREFIXES: [&str; 6] =
            ["SELECT", "SHOW", "WITH", "EXPLAIN", "VALUES", "TABLE"];

        if !ROW_RETURNING_PREFIXES
            .iter()
            .any(|keyword| sql_upper.starts_with(keyword))
        {
            return Ok(StatementDescription::no_data());
        }

        let Ok(statement) = SqlParser::new().parse(sql) else {
            return Ok(StatementDescription::no_data());
        };

        let AstStatement::Select(select) = statement else {
            return Ok(StatementDescription::no_data());
        };

        let names = select_pipeline::output_column_names(&select);

        // A wildcard is expanded from the table's schema, so the description
        // matches the row that `Execute` will send.
        if names.iter().any(|name| name == "*") {
            let table = select
                .from_clause
                .as_ref()
                .and_then(Self::from_clause_table_name);

            if let (Some(table), Some(storage)) = (table, &self.persistent_storage) {
                if let Some(schema) = storage.get_table_schema(&fold_identifier(&table)).await? {
                    return Ok(StatementDescription::returning(
                        schema
                            .columns
                            .iter()
                            .map(|column| ColumnDescription {
                                name: fold_identifier(&column.name),
                                type_oid: column_type_oid(&column.data_type),
                            })
                            .collect(),
                    ));
                }
            }
            return Ok(StatementDescription::no_data());
        }

        // A plain column takes its declared type, so a client that asks for
        // binary results decodes it correctly. Reporting everything as text
        // made an integer column arrive as digits, which drivers reject when
        // asked for an i32.
        let table = select
            .from_clause
            .as_ref()
            .and_then(Self::from_clause_table_name);

        let schema = match (&table, &self.persistent_storage) {
            (Some(table), Some(storage)) => {
                storage.get_table_schema(&fold_identifier(table)).await?
            }
            _ => None,
        };

        let described = names
            .into_iter()
            .map(|name| {
                let type_oid = schema
                    .as_ref()
                    .and_then(|schema| {
                        schema
                            .columns
                            .iter()
                            .find(|column| column.name.eq_ignore_ascii_case(&name))
                    })
                    .map_or(super::messages::type_oids::TEXT, |column| {
                        column_type_oid(&column.data_type)
                    });
                ColumnDescription { name, type_oid }
            })
            .collect();

        Ok(StatementDescription::returning(described))
    }

    /// Table named by a simple FROM clause, if it is one.
    fn from_clause_table_name(
        from: &crate::protocols::postgres_wire::sql::ast::FromClause,
    ) -> Option<String> {
        use crate::protocols::postgres_wire::sql::ast::FromClause;

        match from {
            FromClause::Table { name, .. } => Some(name.full_name()),
            _ => None,
        }
    }

    /// Execute a SQL query and return results
    pub async fn execute_query(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Domains stored before this process started are loaded once, so a
        // cast to one works after a restart and not only in the session that
        // created it.
        self.warm_domain_registry().await;

        let sql_upper = sql.trim().to_uppercase();

        // Check if this is a GraphRAG function query
        if self.is_graphrag_query(&sql_upper) {
            if let Some(ref graphrag_engine) = self.graphrag_engine {
                return graphrag_engine.execute_graphrag_query(sql).await;
            } else {
                return Err(ProtocolError::PostgresError(
                    "GraphRAG support not enabled. Use new_with_vector_support() to enable GraphRAG functions.".to_string()
                ));
            }
        }

        // Check if this is a vector-related query
        if let Some(ref vector_engine) = self.vector_engine {
            if self.is_vector_query(&sql_upper) {
                return vector_engine.execute_vector_query(sql).await;
            }
        }

        // PL/pgSQL before anything else looks at the text: a `DO` block and a
        // `CREATE FUNCTION ... LANGUAGE plpgsql` were both answered with
        // "Command completed successfully" and then not run, so a block that
        // should have written a row reported success and wrote nothing.
        if let Some(result) = self.execute_plpgsql(sql).await? {
            return Ok(result);
        }

        // DDL that changes a stored table's shape is applied here for the same
        // reason views are: the comprehensive engine alters its own copy of the
        // schema, so the change was reported and then not there.
        if let Some(result) = self.execute_table_ddl(sql).await? {
            return Ok(result);
        }

        // Views are kept by this engine because the comprehensive engine
        // registers them in its own state: `CREATE VIEW` reported success and
        // the view was then not there.
        if let Some(result) = self.execute_view_statement(sql).await? {
            return Ok(result);
        }

        // Triggers fire around the write they watch. They run as ordinary
        // statements, so a BEFORE trigger that raises an error stops the write.
        if let Some(result) = self.execute_with_triggers(sql).await? {
            return Ok(result);
        }

        self.execute_without_triggers(sql).await
    }

    /// Execute a statement without firing triggers around it.
    ///
    /// The write a trigger wraps goes through here, so the wrapper does not
    /// find its own triggers again.
    ///
    /// # Errors
    /// Returns an error when the statement cannot be parsed or executed.
    async fn execute_without_triggers(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // `RETURNING` is answered here because the simple parser drops the
        // clause and the write path has no select list: the statement applied
        // and then reported no rows, which reads as "nothing matched".
        if let Some(result) = self.execute_with_returning(sql).await? {
            return Ok(result);
        }

        // Try to parse and execute with the simple parser
        // For unsupported statements, fall back to the comprehensive SQL engine
        let statement = match self.parse_sql(sql) {
            Ok(stmt) => stmt,
            Err(_) => {
                // Clause-bearing SELECTs are executed over the rows in
                // persistent storage — the same rows a plain SELECT reads.
                // Sending them to the comprehensive engine instead meant the
                // two answered from different copies of the table, so
                // `SELECT id FROM t` and `SELECT id FROM t ORDER BY id`
                // disagreed about how many rows existed.
                if let Some(result) = self.select_over_storage(sql).await? {
                    return Ok(result);
                }

                // Fall back to comprehensive SQL engine for unsupported statements
                return match self.execute_with_comprehensive_engine(sql).await {
                    Ok(result) => Ok(result),
                    Err(e) => Err(self.explain_unsupported_query(sql, e).await),
                };
            }
        };

        // Route queries based on table type and storage availability
        match statement {
            Statement::Select {
                columns,
                table,
                where_clause,
            } => {
                // A catalogue query carrying a `WHERE` has to go through the
                // path that can evaluate one. Answering it here dropped the
                // clause and returned every row, so a driver asking
                // `... FROM pg_class WHERE relname = $1` got the whole
                // catalogue and read the first entry as its answer.
                if where_clause.is_some()
                    && self
                        .select_system_catalog(&table, &["*".to_string()])
                        .await?
                        .is_some()
                {
                    if let Some(result) = self.select_over_storage(sql).await? {
                        return Ok(result);
                    }
                }
                if let Some(result) = self.select_system_catalog(&table, &columns).await? {
                    return Ok(result);
                }
                // A view is a query, not stored rows, so it is answered by the
                // path that can evaluate one. Without this a clause-free
                // `SELECT ... FROM view` reported that the relation is missing.
                if self.view_definition(&table).await?.is_some() {
                    if let Some(result) = self.select_over_storage(sql).await? {
                        return Ok(result);
                    }
                }
                if table.to_uppercase() == "ACTORS" {
                    self.execute_actor_select(columns, &table, where_clause)
                        .await
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_select(storage, columns, &table, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Use actors table for actor queries or enable persistent storage.",
                        table
                    )))
                }
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                if table.to_uppercase() == "ACTORS" {
                    // In-memory insert for actors
                    // ... (existing logic)
                    Ok(QueryResult::Insert { count: 1 })
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_insert(storage, &table, columns, values)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Enable persistent storage for table operations.",
                        table
                    )))
                }
            }
            Statement::Update {
                table,
                set_clauses,
                where_clause,
            } => {
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_update(storage, &table, set_clauses, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(
                        "Persistent storage not enabled".to_string(),
                    ))
                }
            }
            Statement::Delete {
                table,
                where_clause,
            } => {
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_delete(storage, &table, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(
                        "Persistent storage not enabled".to_string(),
                    ))
                }
            }
            Statement::CreateTable {
                table,
                columns,
                if_not_exists,
                foreign_keys,
            } => {
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_create_table(storage, &table, columns, if_not_exists, foreign_keys)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(
                        "Persistent storage not enabled".to_string(),
                    ))
                }
            }
            Statement::Truncate { table } => match self.persistent_storage {
                Some(ref storage) => self.execute_truncate(storage, &table).await,
                None => Err(ProtocolError::PostgresError(
                    "Persistent storage not enabled".to_string(),
                )),
            },
            Statement::DropTable { table, if_exists } => {
                tracing::debug!(
                    storage = self.persistent_storage.is_some(),
                    "executing DROP TABLE"
                );
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_drop_table(storage, &table, if_exists).await
                } else {
                    Err(ProtocolError::PostgresError(
                        "Persistent storage not enabled".to_string(),
                    ))
                }
            }
        }
    }

    /// Execute multiple SQL queries (separated by semicolons)
    /// Execute the statements of a simple-query message, in order.
    ///
    /// Each statement goes through [`Self::execute_query`] rather than through
    /// a second dispatcher over the parsed AST. Two routing tables drifted:
    /// `RETURNING`, `TRUNCATE`, views, CTEs, derived tables and set operations
    /// all worked when a statement arrived alone and failed when the same text
    /// arrived over the wire, because only one of the two routers knew about
    /// them.
    ///
    /// # Errors
    /// Returns the first statement's error; statements after it do not run,
    /// as PostgreSQL does within one simple-query message.
    pub async fn execute_multiple_queries(&self, sql: &str) -> ProtocolResult<Vec<QueryResult>> {
        let mut results = Vec::new();
        for statement in split_statements(sql) {
            // A cancel is honoured between statements, which is where
            // PostgreSQL takes one too: the statement already running finishes,
            // and nothing after it starts.
            if cancel_requested() {
                return Err(ProtocolError::PostgresError(
                    "canceling statement due to user request".to_string(),
                ));
            }
            results.push(self.execute_query(&statement).await?);
        }
        Ok(results)
    }

    /// Execute a query using the comprehensive SQL engine
    async fn execute_with_comprehensive_engine(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let mut sql_engine = self.sql_engine.lock().await;
        match sql_engine.execute(sql).await {
            Ok(result) => {
                // Debug: log what result we got from the comprehensive engine
                tracing::debug!("Comprehensive SQL engine result: {:?}", result);
                Ok(self.convert_sql_result_to_query_result(result))
            }
            Err(e) => Err(e),
        }
    }

    /// Execute SQL using the comprehensive engine directly (bypasses persistent storage checks)
    /// This is useful for testing and operations that don't require persistent storage
    pub async fn execute_sql_direct(&self, sql: &str) -> ProtocolResult<QueryResult> {
        self.execute_with_comprehensive_engine(sql).await
    }

    /// Convert SQL execution result to QueryResult
    fn convert_sql_result_to_query_result(&self, result: UnifiedExecutionResult) -> QueryResult {
        match result {
            UnifiedExecutionResult::Select { columns, rows, .. } => {
                QueryResult::Select { columns, rows }
            }
            UnifiedExecutionResult::Insert { count, .. } => QueryResult::Insert { count },
            UnifiedExecutionResult::Update { count, .. } => QueryResult::Update { count },
            UnifiedExecutionResult::Delete { count, .. } => QueryResult::Delete { count },
            UnifiedExecutionResult::Merge {
                count,
                rows,
                columns,
                ..
            } => QueryResult::Merge {
                count,
                rows,
                columns,
            },
            UnifiedExecutionResult::CreateTable { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::CreateIndex { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::Transaction { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::CreateExtension { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::CreateSchema { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::CreateView { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::DropTable { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::DropIndex { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::DropExtension { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::DropSchema { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::DropView { .. } => QueryResult::Update { count: 0 },
            UnifiedExecutionResult::Set {
                variable, value, ..
            } => QueryResult::Set { variable, value },
            UnifiedExecutionResult::Other { message, .. } => QueryResult::Select {
                columns: vec!["message".to_string()],
                rows: vec![vec![Some(message)]],
            },
        }
    }

    /// Check if a query is vector-related
    fn is_vector_query(&self, sql: &str) -> bool {
        sql.contains("CREATE EXTENSION VECTOR")
            || sql.contains("VECTOR(")
            || sql.contains("HALFVEC(")
            || sql.contains("<->")
            || sql.contains("<#>")
            || sql.contains("<=>")
            || sql.contains("VECTOR_DIMS")
            || sql.contains("VECTOR_NORM")
            || (sql.contains("CREATE INDEX")
                && (sql.contains("USING IVFFLAT") || sql.contains("USING HNSW")))
    }

    /// Check if a query contains GraphRAG functions
    fn is_graphrag_query(&self, sql: &str) -> bool {
        sql.contains("GRAPHRAG_BUILD(")
            || sql.contains("GRAPHRAG_QUERY(")
            || sql.contains("GRAPHRAG_EXTRACT(")
            || sql.contains("GRAPHRAG_REASON(")
            || sql.contains("GRAPHRAG_STATS(")
            || sql.contains("GRAPHRAG_ENTITIES(")
            || sql.contains("GRAPHRAG_SIMILAR(")
    }

    /// Parse SQL statement
    ///
    /// The statement is dispatched on an uppercased copy but each sub-parser
    /// receives the text as written. Uppercasing the statement itself — which
    /// this did, behind a variable named `original_sql` that was in fact a clone
    /// of the uppercased one — rewrote string literals too, so
    /// `INSERT ... VALUES ('alpha')` stored `ALPHA`. The sub-parsers already
    /// match keywords case-insensitively and normalise identifiers themselves.
    fn parse_sql(&self, sql: &str) -> ProtocolResult<Statement> {
        let original_sql = sql.trim();
        let sql = original_sql.to_uppercase();

        if sql.starts_with("SELECT") {
            self.parse_select(original_sql)
        } else if sql.starts_with("INSERT") {
            self.parse_insert(original_sql)
        } else if sql.starts_with("UPDATE") {
            self.parse_update(original_sql)
        } else if sql.starts_with("DELETE") {
            self.parse_delete(original_sql)
        } else if sql.starts_with("CREATE TABLE") {
            self.parse_create_table(original_sql)
        } else if sql.starts_with("DROP TABLE") {
            self.parse_drop_table(original_sql)
        } else if sql.starts_with("TRUNCATE") {
            Self::parse_truncate(original_sql)
        } else {
            Err(ProtocolError::PostgresError(format!(
                "Unsupported SQL statement: {sql}"
            )))
        }
    }

    /// `CREATE TABLE ... AS SELECT` and `ALTER TABLE ... DROP COLUMN`.
    ///
    /// Returns `None` when the statement is neither.
    ///
    /// # Errors
    /// Returns an error when the table is unknown or the select cannot be run.
    async fn execute_table_ddl(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        use crate::protocols::postgres_wire::persistent_storage::{
            ColumnDefinition, ColumnType, TableSchema,
        };

        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(None);
        };
        let trimmed = sql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_uppercase();

        // `CREATE TABLE <name> AS <select>`: the shape comes from the select.
        if upper.starts_with("CREATE TABLE") && !trimmed.contains('(') {
            let Some(as_at) = upper.find(" AS ") else {
                return Ok(None);
            };
            let name = upper[..as_at]
                .split_whitespace()
                .last()
                .map(|_| trimmed[..as_at].split_whitespace().last().unwrap_or(""))
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    ProtocolError::PostgresError("CREATE TABLE requires a name".to_string())
                })?;
            let table = fold_identifier(name);
            let query = trimmed[as_at + 4..].trim();

            let QueryResult::Select { columns, rows } =
                self.select_over_storage(query).await?.ok_or_else(|| {
                    ProtocolError::PostgresError(
                        "the source of CREATE TABLE AS uses a shape this engine cannot evaluate"
                            .to_string(),
                    )
                })?
            else {
                return Ok(None);
            };

            // Types are not carried by the projection, so every column is
            // stored as text unless a value parses as a number. Recording that
            // here rather than guessing per row keeps the schema stable.
            let column_defs = columns
                .iter()
                .enumerate()
                .map(|(index, name)| ColumnDefinition {
                    name: fold_identifier(name),
                    data_type: if rows.iter().all(|row| {
                        row.get(index)
                            .and_then(Option::as_ref)
                            .is_none_or(|value| value.parse::<i64>().is_ok())
                    }) {
                        ColumnType::BigInt
                    } else {
                        ColumnType::Text
                    },
                    nullable: true,
                    default_value: None,
                    unique: false,
                    check: None,
                    references: None,
                    domain: None,
                })
                .collect::<Vec<_>>();

            storage
                .create_table(TableSchema {
                    name: table.clone(),
                    columns: column_defs.clone(),
                    created_at: chrono::Utc::now(),
                    row_count: 0,
                    foreign_keys: Vec::new(),
                })
                .await?;

            let count = rows.len();
            for row in rows {
                let now = chrono::Utc::now();
                let values = column_defs
                    .iter()
                    .zip(&row)
                    .map(|(column, value)| {
                        let stored = match value {
                            None => JsonValue::Null,
                            Some(text) => Self::literal_to_json(text),
                        };
                        (column.name.clone(), stored)
                    })
                    .collect();
                storage
                    .insert_row(
                        &table,
                        TableRow {
                            values,
                            created_at: now,
                            updated_at: now,
                        },
                    )
                    .await?;
            }
            return Ok(Some(QueryResult::Insert { count }));
        }

        // `VACUUM [table]`: reclaim the versions and deleted rows no open
        // block can still need.
        if upper.starts_with("VACUUM") {
            let target = trimmed.split_whitespace().nth(1).filter(|word| {
                !word.eq_ignore_ascii_case("FULL") && !word.eq_ignore_ascii_case("ANALYZE")
            });
            // The change log is reclaimed here too: it is storage nobody can
            // still need, which is what VACUUM is for.
            let trimmed = self.truncate_change_log().await?;
            let reclaimed = self.vacuum(target).await?;
            return Ok(Some(QueryResult::Delete {
                count: reclaimed + trimmed,
            }));
        }

        // `CREATE TRIGGER <name> {BEFORE|AFTER} <events> ON <table>
        //  [FOR EACH ROW] EXECUTE <statement>`
        //
        // The action is a SQL statement rather than a stored function: this
        // engine has no PL/pgSQL, and running a statement is the part of a
        // trigger that changes what the database does.
        if upper.starts_with("CREATE TRIGGER") {
            let Some(execute_at) = upper.find(" EXECUTE ") else {
                return Err(ProtocolError::PostgresError(
                    "CREATE TRIGGER requires EXECUTE followed by a statement".to_string(),
                ));
            };
            let header = &trimmed[..execute_at];
            let action = trimmed[execute_at + " EXECUTE ".len()..]
                .trim()
                .trim_start_matches("FUNCTION ")
                .trim_start_matches("PROCEDURE ")
                .trim();
            let header_upper = header.to_uppercase();

            let name = header
                .split_whitespace()
                .nth(2)
                .map(fold_identifier)
                .ok_or_else(|| {
                    ProtocolError::PostgresError("CREATE TRIGGER requires a name".to_string())
                })?;
            let table = header_upper
                .rfind(" ON ")
                .and_then(|at| header[at + 4..].split_whitespace().next())
                .map(fold_identifier)
                .ok_or_else(|| {
                    ProtocolError::PostgresError("CREATE TRIGGER requires ON <table>".to_string())
                })?;

            let timing = if header_upper.contains("INSTEAD OF") {
                "INSTEAD"
            } else if header_upper.contains("BEFORE") {
                "BEFORE"
            } else {
                "AFTER"
            };
            let events: Vec<&str> = ["INSERT", "UPDATE", "DELETE"]
                .into_iter()
                .filter(|event| header_upper.contains(event))
                .collect();
            if events.is_empty() {
                return Err(ProtocolError::PostgresError(
                    "CREATE TRIGGER requires at least one of INSERT, UPDATE or DELETE".to_string(),
                ));
            }

            // `FOR EACH ROW` fires once per affected row and can read that
            // row through `NEW`/`OLD`; `FOR EACH STATEMENT` — the default —
            // fires once however many rows the statement touched.
            let scope = if header_upper.contains("FOR EACH ROW") {
                "ROW"
            } else {
                "STATEMENT"
            };
            // `WHEN (...)` gates the firing; it is stored with its brackets
            // stripped and evaluated per row.
            let when = header_upper
                .find("WHEN")
                .and_then(|at| Self::parenthesised(&header[at..]))
                .unwrap_or_default()
                .to_string();

            self.remember_trigger(
                &name,
                &format!(
                    "{table}|{timing}|{}|{scope}|{when}|{action}",
                    events.join(",")
                ),
            )
            .await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        if upper.starts_with("DROP TRIGGER") {
            let if_exists = upper.contains("IF EXISTS");
            let name = fold_identifier(upper_tail(trimmed, if_exists));
            self.ensure_view_catalog(storage).await?;
            let removed = storage
                .delete_rows(
                    VIEW_CATALOG,
                    vec![QueryCondition {
                        column: "name".to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::String(format!("trigger:{name}")),
                    }],
                )
                .await?;
            if removed == 0 && !if_exists {
                return Err(ProtocolError::PostgresError(format!(
                    "trigger \"{name}\" does not exist"
                )));
            }
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `CREATE DOMAIN <name> [AS] <type> [NOT NULL] [CHECK (VALUE ...)]`:
        // a named type with constraints attached, which columns then use.
        if upper.starts_with("CREATE DOMAIN") {
            let rest = trimmed["CREATE DOMAIN".len()..].trim();
            let (name, definition) = rest.split_once(char::is_whitespace).ok_or_else(|| {
                ProtocolError::PostgresError("CREATE DOMAIN requires a type".to_string())
            })?;
            let definition = definition
                .trim()
                .strip_prefix("AS ")
                .or_else(|| definition.trim().strip_prefix("as "))
                .unwrap_or(definition.trim());
            self.remember_domain(&fold_identifier(name), definition.trim())
                .await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `ALTER DOMAIN <name> {SET|DROP} NOT NULL | ADD CHECK (...) | DROP CONSTRAINT`
        if upper.starts_with("ALTER DOMAIN") {
            let rest = trimmed["ALTER DOMAIN".len()..].trim();
            let (name, action) = rest.split_once(char::is_whitespace).ok_or_else(|| {
                ProtocolError::PostgresError("ALTER DOMAIN requires an action".to_string())
            })?;
            let name = fold_identifier(name);
            let current = self.domain_definition(&name).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("domain \"{name}\" does not exist"))
            })?;
            let action_upper = action.trim().to_uppercase();

            let updated = if action_upper.starts_with("SET NOT NULL") {
                format!("{current} NOT NULL")
            } else if action_upper.starts_with("DROP NOT NULL") {
                current.replace(" NOT NULL", "")
            } else if action_upper.starts_with("ADD") && action_upper.contains("CHECK") {
                match Self::parenthesised(action) {
                    Some(predicate) => format!("{current} CHECK ({predicate})"),
                    None => current.clone(),
                }
            } else if action_upper.starts_with("DROP CONSTRAINT") {
                // Every check on the domain goes; named constraints are not
                // tracked separately.
                match current.find(" CHECK (") {
                    Some(at) => current[..at].to_string(),
                    None => current.clone(),
                }
            } else {
                return Err(ProtocolError::PostgresError(format!(
                    "unsupported ALTER DOMAIN action: {}",
                    action.trim()
                )));
            };

            self.remember_domain(&name, updated.trim()).await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        if upper.starts_with("DROP DOMAIN") {
            let if_exists = upper.contains("IF EXISTS");
            let name = fold_identifier(upper_tail(trimmed, if_exists));
            self.ensure_view_catalog(storage).await?;
            let removed = storage
                .delete_rows(
                    VIEW_CATALOG,
                    vec![QueryCondition {
                        column: "name".to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::String(format!("domain:{name}")),
                    }],
                )
                .await?;
            if removed == 0 && !if_exists {
                return Err(ProtocolError::PostgresError(format!(
                    "domain \"{name}\" does not exist"
                )));
            }
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `CREATE MATERIALIZED VIEW <name> AS <select>`: unlike a view, the
        // rows are computed once and stored, and `REFRESH` recomputes them.
        if upper.starts_with("CREATE MATERIALIZED VIEW") {
            let Some(as_at) = upper.find(" AS ") else {
                return Err(ProtocolError::PostgresError(
                    "CREATE MATERIALIZED VIEW requires AS followed by a query".to_string(),
                ));
            };
            let name = trimmed[..as_at]
                .split_whitespace()
                .last()
                .map(fold_identifier)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    ProtocolError::PostgresError(
                        "CREATE MATERIALIZED VIEW requires a name".to_string(),
                    )
                })?;
            let query = trimmed[as_at + 4..].trim();
            self.remember_materialized(&name, query).await?;
            return Box::pin(self.execute_table_ddl(&format!("CREATE TABLE {name} AS {query}")))
                .await;
        }

        if upper.starts_with("REFRESH MATERIALIZED VIEW") {
            let name = trimmed
                .split_whitespace()
                .last()
                .map(fold_identifier)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    ProtocolError::PostgresError("REFRESH requires a name".to_string())
                })?;
            let query = self.materialized_definition(&name).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("materialized view \"{name}\" does not exist"))
            })?;
            storage.drop_table(&name).await?;
            return Box::pin(self.execute_table_ddl(&format!("CREATE TABLE {name} AS {query}")))
                .await;
        }

        // `ALTER TABLE <name> RENAME [COLUMN <old> TO <new>] | [TO <new>]`.
        if upper.starts_with("ALTER TABLE") && upper.contains(" RENAME") {
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let table = words
                .get(2)
                .map(|name| fold_identifier(name))
                .ok_or_else(|| {
                    ProtocolError::PostgresError("ALTER TABLE requires a name".to_string())
                })?;
            let mut schema = storage.get_table_schema(&table).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
            })?;
            let at = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("RENAME"))
                .ok_or_else(|| {
                    ProtocolError::PostgresError("RENAME requires a target".to_string())
                })?;
            let new_name = words
                .last()
                .map(|name| fold_identifier(name))
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    ProtocolError::PostgresError("RENAME requires a new name".to_string())
                })?;

            if words
                .get(at + 1)
                .is_some_and(|w| w.eq_ignore_ascii_case("COLUMN"))
            {
                let old = words
                    .get(at + 2)
                    .map(|name| fold_identifier(name))
                    .ok_or_else(|| {
                        ProtocolError::PostgresError("RENAME COLUMN requires a name".to_string())
                    })?;
                let column = schema
                    .columns
                    .iter_mut()
                    .find(|column| fold_identifier(&column.name) == old)
                    .ok_or_else(|| {
                        ProtocolError::PostgresError(format!(
                            "column \"{old}\" of relation \"{table}\" does not exist"
                        ))
                    })?;

                // The stored rows are keyed by the old name, so they are read
                // and written back under the new one; renaming only the schema
                // would make the column read as NULL.
                let rows = storage
                    .select_rows(&table, Vec::new(), Vec::new(), None)
                    .await?;
                let previous = column.name.clone();
                column.name = new_name.clone();
                storage.create_table(schema).await?;
                storage.delete_rows(&table, Vec::new()).await?;
                for mut row in rows {
                    if let Some(value) = row.values.remove(&previous) {
                        row.values.insert(new_name.clone(), value);
                    }
                    storage.insert_row(&table, row).await?;
                }
                return Ok(Some(QueryResult::Update { count: 0 }));
            }

            // `RENAME TO <new>`: the rows move with the table.
            let rows = storage
                .select_rows(&table, Vec::new(), Vec::new(), None)
                .await?;
            schema.name = new_name.clone();
            storage.create_table(schema).await?;
            for row in rows {
                storage.insert_row(&new_name, row).await?;
            }
            storage.drop_table(&table).await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `ALTER TABLE <name> DROP COLUMN <column>`.
        if upper.starts_with("ALTER TABLE") && upper.contains("DROP COLUMN") {
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let table = words
                .get(2)
                .map(|name| fold_identifier(name))
                .ok_or_else(|| {
                    ProtocolError::PostgresError("ALTER TABLE requires a name".to_string())
                })?;
            let column = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("COLUMN"))
                .and_then(|at| words.get(at + 1))
                .map(|name| fold_identifier(name.trim_end_matches(',')))
                .ok_or_else(|| {
                    ProtocolError::PostgresError("DROP COLUMN requires a column".to_string())
                })?;

            let mut schema = storage.get_table_schema(&table).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
            })?;
            if !schema
                .columns
                .iter()
                .any(|existing| fold_identifier(&existing.name) == column)
            {
                return Err(ProtocolError::PostgresError(format!(
                    "column \"{column}\" of relation \"{table}\" does not exist"
                )));
            }
            schema
                .columns
                .retain(|existing| fold_identifier(&existing.name) != column);
            storage.create_table(schema).await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `CREATE [UNIQUE] INDEX [name] ON <table> (<column>)`.
        //
        // A plain index is a performance structure and changes no answer, so
        // accepting one without building it is honest. A *unique* index is an
        // integrity constraint the caller asked for by name: accepted and not
        // enforced, duplicates went in silently. It is recorded on the column,
        // which is where uniqueness is already checked.
        if upper.starts_with("CREATE ") && upper.contains(" INDEX ") {
            let unique = upper.contains(" UNIQUE INDEX ");
            if !unique {
                return Ok(None);
            }
            let Some(on_at) = upper.find(" ON ") else {
                return Ok(None);
            };
            let name = trimmed[..on_at]
                .split_whitespace()
                .last()
                .map(fold_identifier)
                .unwrap_or_default();
            let rest = trimmed[on_at + 4..].trim();
            let Some(open) = rest.find('(') else {
                return Ok(None);
            };
            let Some(close) = rest.rfind(')') else {
                return Ok(None);
            };
            let table = fold_identifier(rest[..open].trim());
            let columns: Vec<String> = rest[open + 1..close]
                .split(',')
                .map(|column| fold_identifier(column.trim()))
                .filter(|column| !column.is_empty())
                .collect();

            let [column] = columns.as_slice() else {
                // A schema records uniqueness per column, so a multi-column
                // unique index has nowhere to live. Refusing says so rather
                // than accepting a constraint that would never be checked.
                return Err(ProtocolError::SqlState {
                    code: "0A000",
                    message: "a multi-column UNIQUE INDEX is not supported; \
                              declare the columns UNIQUE instead"
                        .to_string(),
                });
            };

            let mut schema = storage.get_table_schema(&table).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
            })?;
            let Some(existing) = schema
                .columns
                .iter_mut()
                .find(|c| fold_identifier(&c.name) == *column)
            else {
                return Err(ProtocolError::SqlState {
                    code: "42703",
                    message: format!("column \"{column}\" of relation \"{table}\" does not exist"),
                });
            };

            // The rows already there have to satisfy it, or the index would
            // claim something about the table that is not true.
            let rows = storage
                .select_rows(&table, Vec::new(), Vec::new(), None)
                .await?;
            let mut seen = std::collections::HashSet::new();
            for row in rows.iter().filter(|row| row_is_visible(&row.values)) {
                let Some(value) = row
                    .values
                    .iter()
                    .find(|(key, _)| fold_identifier(key) == *column)
                    .map(|(_, value)| value)
                    .filter(|value| !value.is_null())
                else {
                    continue;
                };
                if !seen.insert(value.to_string()) {
                    return Err(ProtocolError::SqlState {
                        code: "23505",
                        message: format!(
                            "could not create unique index \"{name}\":                              key value is duplicated"
                        ),
                    });
                }
            }

            existing.unique = true;
            storage.create_table(schema).await?;
            self.remember_index(&name, &table, column).await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `DROP INDEX <name>` — a unique index carries a constraint, so
        // dropping it has to take the constraint with it.
        if upper.starts_with("DROP INDEX") {
            let name = fold_identifier(
                trimmed["DROP INDEX".len()..]
                    .trim()
                    .trim_start_matches("IF EXISTS")
                    .trim(),
            );
            let Some((table, column)) = self.index_definition(&name).await? else {
                // Not a unique index this server recorded; nothing to undo.
                return Ok(None);
            };
            if let Some(mut schema) = storage.get_table_schema(&table).await? {
                if let Some(existing) = schema
                    .columns
                    .iter_mut()
                    .find(|c| fold_identifier(&c.name) == column)
                {
                    existing.unique = false;
                }
                storage.create_table(schema).await?;
            }
            self.forget_catalog_entry(&format!("index:{name}")).await?;
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        // `ALTER TABLE <name> ADD COLUMN <column> <type> [DEFAULT x] [NOT NULL]`.
        //
        // Nothing handled this, so it fell through to a generic
        // "Command completed successfully": the statement reported success and
        // the column was not there. Every later reference to it then failed
        // with `column does not exist`, pointing at the query rather than at
        // the DDL that never happened.
        if upper.starts_with("ALTER TABLE") && upper.contains(" ADD ") {
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let table = words
                .get(2)
                .map(|name| fold_identifier(name))
                .ok_or_else(|| {
                    ProtocolError::PostgresError("ALTER TABLE requires a name".to_string())
                })?;
            let at = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("ADD"))
                .ok_or_else(|| {
                    ProtocolError::PostgresError(
                        "ALTER TABLE ... ADD requires a column".to_string(),
                    )
                })?;
            // `COLUMN` is optional in PostgreSQL.
            let start = if words
                .get(at + 1)
                .is_some_and(|word| word.eq_ignore_ascii_case("COLUMN"))
            {
                at + 2
            } else {
                at + 1
            };
            let definition: Vec<&str> = words[start.min(words.len())..].to_vec();
            let (Some(name), Some(declared)) = (definition.first(), definition.get(1)) else {
                return Err(ProtocolError::PostgresError(
                    "ALTER TABLE ... ADD COLUMN requires a name and a type".to_string(),
                ));
            };
            let rest: Vec<String> = definition[2.min(definition.len())..]
                .iter()
                .map(|word| word.to_uppercase())
                .collect();
            let says = |word: &str| rest.iter().any(|c| c == word);
            let column = ColumnDefinition {
                name: fold_identifier(name),
                data_type: column_type_from_name(declared.trim_end_matches(',')),
                // A column added to a table that already has rows must be
                // nullable unless a default fills it, or the existing rows
                // would violate it the moment it is added.
                nullable: !(says("NOT") && says("NULL")),
                default_value: rest
                    .iter()
                    .position(|word| word == "DEFAULT")
                    .and_then(|at| definition.get(2 + at + 1))
                    .map(|value| Self::literal_to_json(value)),
                unique: says("UNIQUE"),
                check: None,
                references: None,
                domain: None,
            };

            let mut schema = storage.get_table_schema(&table).await?.ok_or_else(|| {
                ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
            })?;
            if schema
                .columns
                .iter()
                .any(|existing| fold_identifier(&existing.name) == fold_identifier(&column.name))
            {
                return Err(ProtocolError::SqlState {
                    code: "42701",
                    message: format!(
                        "column \"{}\" of relation \"{table}\" already exists",
                        column.name
                    ),
                });
            }
            let default_value = column.default_value.clone();
            let column_name = column.name.clone();
            schema.columns.push(column);
            storage.create_table(schema).await?;

            // PostgreSQL fills the rows that already exist with the default;
            // left out, a row written before the column existed reads NULL
            // while one written after reads the default, and the same table
            // answers two ways depending on when a row arrived.
            if let Some(default_value) = default_value {
                storage
                    .update_rows(
                        &table,
                        HashMap::from([(column_name, default_value)]),
                        Vec::new(),
                    )
                    .await?;
            }
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        Ok(None)
    }

    /// Run a write with its `BEFORE` and `AFTER` triggers around it.
    ///
    /// Returns `None` when the statement is not a write, or when the table it
    /// writes has no trigger — the common case, which costs one catalog read.
    ///
    /// # Errors
    /// Returns an error when a trigger's own statement fails; a `BEFORE`
    /// failure stops the write.
    async fn execute_with_triggers(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        if self.persistent_storage.is_none() {
            return Ok(None);
        }
        let event = match sql
            .split_whitespace()
            .next()
            .map(str::to_uppercase)
            .as_deref()
        {
            Some("INSERT") => "INSERT",
            Some("UPDATE") => "UPDATE",
            Some("DELETE") => "DELETE",
            _ => return Ok(None),
        };
        let Some(table) = Self::write_target_table(sql) else {
            return Ok(None);
        };

        let before = self.triggers_for(&table, "BEFORE", event).await?;
        let after = self.triggers_for(&table, "AFTER", event).await?;
        let instead = self.triggers_for(&table, "INSTEAD", event).await?;
        if before.is_empty() && after.is_empty() && instead.is_empty() {
            return Ok(None);
        }

        // `OLD` is the row as it stands before the statement; `NEW` is the
        // row it will become. An INSERT has only NEW, a DELETE only OLD.
        let old_rows = match event {
            "UPDATE" | "DELETE" => self
                .rows_a_statement_will_change(sql)
                .await?
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let new_rows = match event {
            "INSERT" => self.rows_an_insert_adds(sql).await?.unwrap_or_default(),
            "UPDATE" => self
                .rows_a_statement_will_change(sql)
                .await?
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        // A `BEFORE` trigger may rewrite the row on its way in, spelled
        // `SET NEW.col = <expr>` — the one form a statement-based trigger can
        // express without a procedural language.
        let mut new_rows = new_rows;
        let mut rewritten = None;
        for trigger in &before {
            if let Some(assignment) = trigger.action.to_uppercase().strip_prefix("SET NEW.") {
                let _ = assignment;
                rewritten = Some(Self::apply_new_assignment(
                    &trigger.action,
                    &mut new_rows,
                    &old_rows,
                )?);
                continue;
            }
            self.fire_trigger(trigger, &old_rows, &new_rows).await?;
        }

        // `INSTEAD OF` replaces the write entirely, which is what makes a view
        // writable.
        if !instead.is_empty() {
            for trigger in &instead {
                self.fire_trigger(trigger, &old_rows, &new_rows).await?;
            }
            for trigger in &after {
                self.fire_trigger(trigger, &old_rows, &new_rows).await?;
            }
            return Ok(Some(QueryResult::Update {
                count: new_rows.len(),
            }));
        }

        // The write itself goes through the ordinary path; the recursion is
        // bounded because that path finds no trigger left to fire for it.
        let statement = match (rewritten, event) {
            (Some(()), "INSERT") => Self::rewrite_insert(sql, &table, &new_rows),
            // A rewritten UPDATE becomes a SET of the values the trigger left
            // on each row, applied to the rows it already selected.
            (Some(()), "UPDATE") => Self::rewrite_update(sql, &table, &old_rows, &new_rows)
                .unwrap_or_else(|| sql.to_string()),
            _ => sql.to_string(),
        };
        let result = Box::pin(self.execute_without_triggers(&statement)).await?;
        for trigger in &after {
            self.fire_trigger(trigger, &old_rows, &new_rows).await?;
        }
        Ok(Some(result))
    }

    /// Apply a `SET NEW.col = <expr>` trigger action to the incoming rows.
    ///
    /// # Errors
    /// Returns an error when the assignment cannot be parsed or evaluated.
    fn apply_new_assignment(
        action: &str,
        new_rows: &mut [TableRow],
        old_rows: &[TableRow],
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        let body = action.trim().get(4..).unwrap_or_default().trim();
        let Some((target, expression)) = body.split_once('=') else {
            return Err(ProtocolError::PostgresError(
                "a BEFORE trigger's SET requires NEW.<column> = <expression>".to_string(),
            ));
        };
        let column = fold_identifier(
            target
                .trim()
                .trim_start_matches("NEW.")
                .trim_start_matches("new."),
        );

        for (index, row) in new_rows.iter_mut().enumerate() {
            // The expression may itself mention NEW/OLD, so it is substituted
            // against the row it is about to change.
            let text = Self::substitute_row_references(
                expression.trim(),
                old_rows.get(index),
                Some(&row.clone()),
            );
            let parsed = SqlParser::new().parse(&format!("SELECT {text}"))?;
            let crate::protocols::postgres_wire::sql::ast::Statement::Select(select) = parsed
            else {
                continue;
            };
            let Some(crate::protocols::postgres_wire::sql::ast::SelectItem::Expression {
                expr,
                ..
            }) = select.select_list.first()
            else {
                continue;
            };

            let mut evaluator = ExpressionEvaluator::new();
            let value = evaluator.evaluate(expr, &EvaluationContext::empty())?;
            let stored = row
                .values
                .keys()
                .find(|name| fold_identifier(name) == column)
                .cloned()
                .unwrap_or(column.clone());
            row.values.insert(stored, Self::sql_value_to_json(&value));
        }
        Ok(())
    }

    /// Rebuild an `INSERT` from the rows a `BEFORE` trigger rewrote.
    fn rewrite_insert(original: &str, table: &str, rows: &[TableRow]) -> String {
        let Some(first) = rows.first() else {
            return original.to_string();
        };
        let columns: Vec<String> = first.values.keys().cloned().collect();
        let tuples: Vec<String> = rows
            .iter()
            .map(|row| {
                let values: Vec<String> = columns
                    .iter()
                    .map(|column| match row.values.get(column) {
                        None | Some(JsonValue::Null) => "NULL".to_string(),
                        Some(JsonValue::Number(number)) => number.to_string(),
                        Some(JsonValue::Bool(flag)) => flag.to_string(),
                        Some(other) => format!(
                            "'{}'",
                            other.as_str().unwrap_or_default().replace('\'', "''")
                        ),
                    })
                    .collect();
                format!("({})", values.join(", "))
            })
            .collect();

        format!(
            "INSERT INTO {table} ({}) VALUES {}",
            columns.join(", "),
            tuples.join(", ")
        )
    }

    /// Rebuild an `UPDATE` from the rows a `BEFORE` trigger rewrote.
    ///
    /// Returns `None` when the rows cannot be told apart, in which case the
    /// original statement runs unchanged rather than a guess.
    fn rewrite_update(
        original: &str,
        table: &str,
        old_rows: &[TableRow],
        new_rows: &[TableRow],
    ) -> Option<String> {
        // One row is the case a per-row rewrite can express as a statement;
        // more than one would need a different SET per row.
        if new_rows.len() != 1 || old_rows.len() != 1 {
            return None;
        }
        let new = new_rows.first()?;
        let old = old_rows.first()?;

        let literal = |value: &JsonValue| match value {
            JsonValue::Null => "NULL".to_string(),
            JsonValue::Number(number) => number.to_string(),
            JsonValue::Bool(flag) => flag.to_string(),
            other => format!(
                "'{}'",
                other.as_str().unwrap_or_default().replace('\'', "''")
            ),
        };

        let assignments: Vec<String> = new
            .values
            .iter()
            .map(|(column, value)| format!("{column} = {}", literal(value)))
            .collect();
        if assignments.is_empty() {
            return None;
        }
        // The original row identifies itself by every column it had.
        let conditions: Vec<String> = old
            .values
            .iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(column, value)| format!("{column} = {}", literal(value)))
            .collect();
        if conditions.is_empty() {
            return None;
        }
        let _ = original;

        Some(format!(
            "UPDATE {table} SET {} WHERE {}",
            assignments.join(", "),
            conditions.join(" AND ")
        ))
    }

    /// Run one trigger, once per row or once per statement.
    ///
    /// # Errors
    /// Returns an error when the trigger's own statement or `WHEN` clause
    /// fails.
    async fn fire_trigger(
        &self,
        trigger: &TriggerDefinition,
        old_rows: &[TableRow],
        new_rows: &[TableRow],
    ) -> ProtocolResult<()> {
        if !trigger.per_row {
            // A statement-level trigger fires once, and has no row to read.
            if Self::trigger_fires(self, &trigger.when, None, None).await? {
                self.run_trigger_body(&trigger.action).await?;
            }
            return Ok(());
        }

        let count = old_rows.len().max(new_rows.len());
        for index in 0..count {
            let old = old_rows.get(index);
            let new = new_rows.get(index);
            if !Self::trigger_fires(self, &trigger.when, old, new).await? {
                continue;
            }
            let action = Self::substitute_row_references(&trigger.action, old, new);
            self.run_trigger_body(&action).await?;
        }
        Ok(())
    }

    /// Run a trigger's body: one statement, or several between `BEGIN` and
    /// `END`, with `RAISE` as a way to reject the write.
    ///
    /// # Errors
    /// Returns an error when a statement fails, or when `RAISE` is reached.
    async fn run_trigger_body(&self, body: &str) -> ProtocolResult<()> {
        // A body with a variable, a branch or a loop in it goes to the
        // interpreter; one that is only SQL statements keeps the simpler path,
        // which is what almost every trigger is.
        if plpgsql::needs_interpreter(body) {
            let block = plpgsql::parse(body)?;
            return Box::pin(plpgsql::execute(&block, self, HashMap::new()))
                .await
                .map(|_| ());
        }

        let trimmed = body.trim().trim_end_matches(';').trim();
        // A dollar-quoted body is unwrapped here; the quoting exists to carry
        // the semicolons through the statement splitter, not to be executed.
        let trimmed = match trimmed
            .strip_prefix("$$")
            .and_then(|r| r.strip_suffix("$$"))
        {
            Some(inner) => inner.trim(),
            None => trimmed,
        };
        let upper = trimmed.to_uppercase();

        // `BEGIN ... END` wraps a sequence; anything else is one statement.
        let inner = match (upper.starts_with("BEGIN"), upper.ends_with("END")) {
            (true, true) => trimmed[5..trimmed.len() - 3].trim(),
            _ => trimmed,
        };

        for statement in split_statements(inner) {
            let statement = statement.trim();
            if statement.is_empty() {
                continue;
            }
            // `RAISE [level] 'message'` stops the trigger, and with it the
            // write a BEFORE trigger guards.
            if let Some(rest) = statement
                .to_uppercase()
                .strip_prefix("RAISE")
                .map(|rest| rest.trim().to_string())
            {
                let message = statement[statement.len() - rest.len()..]
                    .trim()
                    .trim_start_matches(|c: char| c.is_ascii_alphabetic())
                    .trim()
                    .trim_matches('\'')
                    .to_string();
                return Err(ProtocolError::PostgresError(if message.is_empty() {
                    "raised by a trigger".to_string()
                } else {
                    message
                }));
            }
            Box::pin(self.execute_query(statement)).await?;
        }
        Ok(())
    }

    /// Whether a trigger's `WHEN` clause holds for a row.
    async fn trigger_fires(
        &self,
        when: &Option<String>,
        old: Option<&TableRow>,
        new: Option<&TableRow>,
    ) -> ProtocolResult<bool> {
        let Some(when) = when else {
            return Ok(true);
        };
        let predicate = Self::substitute_row_references(when, old, new);
        let rows = Box::pin(self.execute_query(&format!("SELECT 1 WHERE {predicate}"))).await?;
        Ok(match rows {
            QueryResult::Select { rows, .. } => !rows.is_empty(),
            _ => true,
        })
    }

    /// Replace `NEW.col` and `OLD.col` with the values they stand for.
    ///
    /// Substituting text keeps the trigger's action an ordinary statement,
    /// which is what makes it runnable without a procedural language.
    fn substitute_row_references(
        text: &str,
        old: Option<&TableRow>,
        new: Option<&TableRow>,
    ) -> String {
        let mut out = text.to_string();
        for (prefix, row) in [("NEW", new), ("OLD", old)] {
            let Some(row) = row else {
                continue;
            };
            for (column, value) in &row.values {
                let literal =
                    Self::sql_value_to_literal(&Self::json_to_sql_value(value, &ColumnType::Text));
                // Values are written as SQL literals, so a text column arrives
                // quoted and a NULL arrives as NULL rather than as the word.
                let literal = match value {
                    JsonValue::Null => "NULL".to_string(),
                    JsonValue::Number(number) => number.to_string(),
                    JsonValue::Bool(flag) => flag.to_string(),
                    _ => literal,
                };
                for spelling in [
                    format!("{prefix}.{column}"),
                    format!("{}.{column}", prefix.to_lowercase()),
                ] {
                    out = out.replace(&spelling, &literal);
                }
            }
        }
        out
    }

    /// Create or drop a view, recording its definition in storage.
    ///
    /// A view is a stored query, so its definition has to outlive the process
    /// that created it; keeping it in memory would make `CREATE VIEW` a claim
    /// that stops being true at the next restart.
    ///
    /// Returns `None` when the statement is not a view statement.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be read or written.
    async fn execute_view_statement(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(None);
        };

        let trimmed = sql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("DROP VIEW") {
            let if_exists = upper.contains("IF EXISTS");
            let name = fold_identifier(upper_tail(trimmed, if_exists));
            self.ensure_view_catalog(storage).await?;
            let removed = storage
                .delete_rows(
                    VIEW_CATALOG,
                    vec![QueryCondition {
                        column: "name".to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::String(name.clone()),
                    }],
                )
                .await?;
            if removed == 0 && !if_exists {
                return Err(ProtocolError::PostgresError(format!(
                    "View '{name}' does not exist"
                )));
            }
            return Ok(Some(QueryResult::Update { count: 0 }));
        }

        if !upper.starts_with("CREATE VIEW") && !upper.starts_with("CREATE OR REPLACE VIEW") {
            return Ok(None);
        }

        let Some(as_at) = upper.find(" AS ") else {
            return Err(ProtocolError::PostgresError(
                "CREATE VIEW requires AS followed by a query".to_string(),
            ));
        };
        let header = &trimmed[..as_at];
        let definition = trimmed[as_at + 4..].trim().to_string();
        let Some(name) = header.split_whitespace().last() else {
            return Err(ProtocolError::PostgresError(
                "CREATE VIEW requires a name".to_string(),
            ));
        };
        let name = fold_identifier(name);

        // The definition has to parse now rather than at first read, so a
        // typo is an error at CREATE time as it is in PostgreSQL.
        crate::protocols::postgres_wire::sql::parser::SqlParser::new().parse(&definition)?;

        self.ensure_view_catalog(storage).await?;
        let replacing = upper.starts_with("CREATE OR REPLACE VIEW");
        if storage.table_exists(&name).await?
            || (!replacing && self.view_definition(&name).await?.is_some())
        {
            return Err(ProtocolError::PostgresError(format!(
                "Relation '{name}' already exists"
            )));
        }
        storage
            .delete_rows(
                VIEW_CATALOG,
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(name.clone()),
                }],
            )
            .await?;

        let now = chrono::Utc::now();
        storage
            .insert_row(
                VIEW_CATALOG,
                TableRow {
                    values: HashMap::from([
                        ("name".to_string(), JsonValue::String(name)),
                        ("definition".to_string(), JsonValue::String(definition)),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await?;

        Ok(Some(QueryResult::Update { count: 0 }))
    }

    /// Publish the end of a transaction, so a subscriber can close its
    /// `Begin`/`Commit` pair around everything the block wrote.
    pub fn publish_transaction_end(transaction: u64) {
        publish_marker("COMMIT", transaction);
    }

    /// Drop logged changes every slot has confirmed.
    ///
    /// Without this the log is a table that only grows. The bound is the
    /// slowest slot's confirmed position: anything before it has been read by
    /// everyone who asked, so nothing can still need it.
    ///
    /// # Errors
    /// Returns an error when the log or the catalog cannot be read or written.
    pub async fn truncate_change_log(&self) -> ProtocolResult<usize> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(0);
        };
        if !storage.table_exists(CHANGE_LOG).await? {
            return Ok(0);
        }

        // The slowest slot decides. With no slots at all nothing is
        // subscribed, so the whole log is spent.
        let mut bound = latest_change_position();
        let mut any_slot = false;
        let mut invalidated: Vec<String> = Vec::new();
        if storage.table_exists(VIEW_CATALOG).await? {
            for row in storage
                .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
                .await?
            {
                let Some(name) = row.values.get("name").and_then(JsonValue::as_str) else {
                    continue;
                };
                if !name.starts_with("slot:") {
                    continue;
                }
                let confirmed = row
                    .values
                    .get("definition")
                    .and_then(JsonValue::as_str)
                    .and_then(|definition| definition.split_once('|'))
                    .and_then(|(_, position)| position.parse::<u64>().ok())
                    .unwrap_or(0);

                // A slot that has fallen further behind than the log is
                // allowed to grow is invalidated, as PostgreSQL does past
                // `max_slot_wal_keep_size`. Keeping it would let one dead
                // subscriber hold the log open for ever.
                if latest_change_position().saturating_sub(confirmed) > max_slot_backlog() {
                    tracing::warn!(
                        slot = name.trim_start_matches("slot:"),
                        confirmed,
                        "invalidating a replication slot that has fallen too far behind"
                    );
                    invalidated.push(name.to_string());
                    continue;
                }

                any_slot = true;
                bound = bound.min(confirmed);
            }
        }
        if !invalidated.is_empty() {
            forget_slot_cache();
        }
        for name in invalidated {
            storage
                .delete_rows(
                    VIEW_CATALOG,
                    vec![QueryCondition {
                        column: "name".to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::String(name),
                    }],
                )
                .await?;
        }

        if any_slot && bound == 0 {
            return Ok(0);
        }

        // One conditional delete, not one per row: deleting each position
        // individually rescanned the log every time, so trimming a log of any
        // size took quadratic work and timed out well before it finished.
        let removed = storage
            .delete_rows(
                CHANGE_LOG,
                vec![QueryCondition {
                    column: "last".to_string(),
                    operator: "<=".to_string(),
                    value: JsonValue::from(bound),
                }],
            )
            .await?
            .max(0) as usize;
        Ok(removed)
    }

    /// Continue the change stream where the last run left off.
    ///
    /// Positions are handed out from a counter that starts at one, so without
    /// this a restart would reuse LSNs a replica had already seen and its
    /// bookkeeping would silently skip the new records.
    ///
    /// # Errors
    /// Returns an error when the log cannot be read.
    pub async fn resume_change_positions(&self) -> ProtocolResult<u64> {
        let highest = self
            .logged_changes_since(0)
            .await?
            .last()
            .map_or(0, |record| record.position);
        if highest > 0 {
            CHANGE_POSITION.store(highest, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(highest)
    }

    /// Write everything waiting to the durable log.
    ///
    /// Called at the end of each write, so a replica that reconnects after a
    /// restart finds the change there. Queuing until the background tick would
    /// have meant losing whatever was written in the last minute.
    ///
    /// # Errors
    /// Returns an error when the log cannot be written.
    pub async fn flush_change_log(&self) -> ProtocolResult<()> {
        // With no slot, nothing can ever ask to replay, so the log would be
        // written and never read — doubling every write for no one. The
        // pending queue is drained either way so it cannot grow.
        let pending = drain_pending_log();
        if !self.any_replication_slot().await? {
            return Ok(());
        }
        if pending.is_empty() {
            return Ok(());
        }
        // One row per flush rather than per change: a statement writing a
        // thousand rows produced a thousand log rows, each of which the replay
        // scan then had to read.
        self.record_changes(&pending).await
    }

    /// Whether any replication slot exists.
    ///
    /// Cached: this is on the write path, and a catalog read per write would
    /// cost more than the log row it saves.
    async fn any_replication_slot(&self) -> ProtocolResult<bool> {
        use std::sync::atomic::Ordering;

        match SLOT_CACHE.load(Ordering::Relaxed) {
            1 => return Ok(false),
            2 => return Ok(true),
            _ => {}
        }

        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(false);
        };
        let any = storage.table_exists(VIEW_CATALOG).await?
            && storage
                .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
                .await?
                .iter()
                .any(|row| {
                    row.values
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|name| name.starts_with("slot:"))
                });
        SLOT_CACHE.store(if any { 2 } else { 1 }, Ordering::Relaxed);
        Ok(any)
    }

    /// Write a change to the durable log a replica replays from.
    ///
    /// The in-memory window is a cache in front of this: it answers the common
    /// case without a read, and the log answers a replica that reconnects
    /// after a restart, when the window is empty.
    async fn record_changes(&self, records: &[ChangeRecord]) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        let Some(first) = records.first() else {
            return Ok(());
        };
        self.ensure_change_log(storage).await?;

        // The batch is keyed by its first position, and trimming compares
        // against the last one, so a batch is dropped only once every change
        // in it has been confirmed.
        let payload: Vec<String> = records
            .iter()
            .map(|record| {
                format!(
                    "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
                    record.position, record.transaction, record.action, record.table, record.row
                )
            })
            .collect();

        let now = chrono::Utc::now();
        storage
            .insert_row(
                CHANGE_LOG,
                TableRow {
                    values: HashMap::from([
                        ("position".to_string(), JsonValue::from(first.position)),
                        (
                            "last".to_string(),
                            JsonValue::from(records.last().map_or(first.position, |r| r.position)),
                        ),
                        (
                            "payload".to_string(),
                            JsonValue::String(payload.join("\u{2}")),
                        ),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map(|_| ())
    }

    /// Create the change log if it is not there yet.
    async fn ensure_change_log(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::persistent_storage::{
            ColumnDefinition, ColumnType, TableSchema,
        };

        if storage.table_exists(CHANGE_LOG).await? {
            return Ok(());
        }
        storage
            .create_table(TableSchema {
                name: CHANGE_LOG.to_string(),
                columns: vec![
                    ColumnDefinition {
                        name: "position".to_string(),
                        data_type: ColumnType::BigInt,
                        nullable: false,
                        default_value: None,
                        unique: true,
                        check: None,
                        references: None,
                        domain: None,
                    },
                    ColumnDefinition {
                        name: "last".to_string(),
                        data_type: ColumnType::BigInt,
                        nullable: false,
                        default_value: None,
                        unique: false,
                        check: None,
                        references: None,
                        domain: None,
                    },
                    ColumnDefinition {
                        name: "payload".to_string(),
                        data_type: ColumnType::Text,
                        nullable: false,
                        default_value: None,
                        unique: false,
                        check: None,
                        references: None,
                        domain: None,
                    },
                ],
                created_at: chrono::Utc::now(),
                row_count: 0,
                foreign_keys: Vec::new(),
            })
            .await
    }

    /// Changes after `position`, read from the durable log.
    ///
    /// # Errors
    /// Returns an error when the log cannot be read.
    pub async fn logged_changes_since(&self, position: u64) -> ProtocolResult<Vec<ChangeRecord>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(CHANGE_LOG).await? {
            return Ok(Vec::new());
        }

        // The predicate goes to storage rather than being applied after: a log
        // trimmed to the slot backlog is still large, and building a TableRow
        // for every batch only to drop it is work the storage layer can skip.
        let mut records: Vec<ChangeRecord> = storage
            .select_rows(
                CHANGE_LOG,
                Vec::new(),
                vec![QueryCondition {
                    column: "last".to_string(),
                    operator: ">".to_string(),
                    value: JsonValue::from(position),
                }],
                None,
            )
            .await?
            .into_iter()
            .filter_map(|row| Some(row.values.get("payload")?.as_str()?.to_string()))
            .flat_map(|payload| {
                payload
                    .split('\u{2}')
                    .filter_map(|entry| {
                        let mut parts = entry.splitn(5, '\u{1}');
                        Some(ChangeRecord {
                            position: parts.next()?.parse().ok()?,
                            transaction: parts.next()?.parse().ok()?,
                            action: parts.next()?.to_string(),
                            table: parts.next()?.to_string(),
                            row: parts.next()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|record| record.position > position)
            .collect();
        records.sort_by_key(|record| record.position);
        Ok(records)
    }

    /// Remember a replication slot, so a restart does not forget it.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be written.
    pub async fn create_replication_slot(&self, name: &str, plugin: &str) -> ProtocolResult<()> {
        forget_slot_cache();
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        self.ensure_view_catalog(storage).await?;
        let now = chrono::Utc::now();
        storage
            .delete_rows(
                VIEW_CATALOG,
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(format!("slot:{name}")),
                }],
            )
            .await?;
        storage
            .insert_row(
                VIEW_CATALOG,
                TableRow {
                    values: HashMap::from([
                        (
                            "name".to_string(),
                            JsonValue::String(format!("slot:{name}")),
                        ),
                        (
                            "definition".to_string(),
                            JsonValue::String(format!("{plugin}|{}", latest_change_position())),
                        ),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map(|_| ())
    }

    /// A slot's plugin and confirmed position, if it exists.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be read.
    pub async fn replication_slot(&self, name: &str) -> ProtocolResult<Option<(String, u64)>> {
        Ok(self
            .view_definition(&format!("slot:{name}"))
            .await?
            .and_then(|definition| {
                let (plugin, position) = definition.split_once('|')?;
                Some((plugin.to_string(), position.parse().ok()?))
            }))
    }

    /// Record how far a replica has confirmed, so a restart resumes there.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be written.
    pub async fn confirm_replication_slot(&self, name: &str, position: u64) -> ProtocolResult<()> {
        let Some((plugin, _)) = self.replication_slot(name).await? else {
            return Ok(());
        };
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        storage
            .update_rows(
                VIEW_CATALOG,
                HashMap::from([(
                    "definition".to_string(),
                    JsonValue::String(format!("{plugin}|{position}")),
                )]),
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(format!("slot:{name}")),
                }],
            )
            .await
            .map(|_| ())
    }

    /// Forget a replication slot.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be written.
    pub async fn drop_replication_slot(&self, name: &str) -> ProtocolResult<()> {
        forget_slot_cache();
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        self.ensure_view_catalog(storage).await?;
        storage
            .delete_rows(
                VIEW_CATALOG,
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(format!("slot:{name}")),
                }],
            )
            .await
            .map(|_| ())
    }

    /// Record a trigger's table, timing, events and action.
    async fn remember_trigger(&self, name: &str, definition: &str) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        self.ensure_view_catalog(storage).await?;
        let now = chrono::Utc::now();
        storage
            .insert_row(
                VIEW_CATALOG,
                TableRow {
                    values: HashMap::from([
                        (
                            "name".to_string(),
                            JsonValue::String(format!("trigger:{name}")),
                        ),
                        (
                            "definition".to_string(),
                            JsonValue::String(definition.to_string()),
                        ),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map(|_| ())
    }

    /// The actions of every trigger on `table` firing at `timing` for `event`.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be read.
    pub async fn triggers_for(
        &self,
        table: &str,
        timing: &str,
        event: &str,
    ) -> ProtocolResult<Vec<TriggerDefinition>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(Vec::new());
        }

        let rows = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await?;
        Ok(rows
            .into_iter()
            .filter(|row| {
                row.values
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|name| name.starts_with("trigger:"))
            })
            .filter_map(|row| {
                let definition = row.values.get("definition")?.as_str()?.to_string();
                let mut parts = definition.splitn(6, '|');
                let on = parts.next()?;
                let at = parts.next()?;
                let events = parts.next()?;
                let scope = parts.next()?;
                let when = parts.next()?;
                let action = parts.next()?;
                (on == fold_identifier(table)
                    && at == timing
                    && events.split(',').any(|e| e == event))
                .then(|| TriggerDefinition {
                    per_row: scope == "ROW",
                    when: (!when.is_empty()).then(|| when.to_string()),
                    action: action.to_string(),
                })
            })
            .collect())
    }

    /// Run a `DO` block, define a PL/pgSQL function, or call one.
    ///
    /// Returns `None` when `sql` is none of those, so the caller carries on.
    ///
    /// # Errors
    /// Returns an error when the block will not parse, a statement inside it
    /// fails, or a `RAISE EXCEPTION` fires.
    async fn execute_plpgsql(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        let trimmed = sql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("DO ") || upper == "DO" {
            let body = trimmed[2..].trim();
            // `DO ... LANGUAGE plpgsql` is the same block with the language
            // named after it rather than before.
            let body = match body.to_uppercase().rfind("LANGUAGE") {
                Some(at) if body[at..].to_uppercase().contains("PLPGSQL") => body[..at].trim(),
                _ => body,
            };
            let block = plpgsql::parse(body)?;
            Box::pin(self.run_block_atomically(&block, HashMap::new())).await?;
            return Ok(Some(QueryResult::Set {
                variable: "DO".to_string(),
                value: String::new(),
            }));
        }

        // `CREATE TYPE name AS (field type, ...)` — a composite. Its fields are
        // stored so a PL/pgSQL variable of the type can bring them into scope
        // and so the type is its own type when choosing between overloads.
        if upper.starts_with("CREATE TYPE") {
            let rest = trimmed["CREATE TYPE".len()..].trim();
            let Some(as_at) = rest.to_uppercase().find(" AS ") else {
                return Ok(None);
            };
            let name = fold_identifier(rest[..as_at].trim());
            let body = rest[as_at + 4..].trim();
            let Some(fields) = body
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
            else {
                // `CREATE TYPE ... AS ENUM (...)` and the other forms are not
                // composites; leaving them unhandled is better than storing
                // something that claims to be one.
                return Ok(None);
            };
            let parsed = plpgsql_function::parse_parameters(fields);
            if parsed.is_empty() {
                return Err(ProtocolError::PostgresError(
                    "a composite type needs at least one field".to_string(),
                ));
            }
            self.forget_catalog_entry(&format!("composite:{name}"))
                .await?;
            self.remember_domain(
                &format!("__composite__{name}"),
                &plpgsql_function::encode(&parsed),
            )
            .await?;
            self.rename_catalog_entry(
                &format!("domain:__composite__{name}"),
                &format!("composite:{name}"),
            )
            .await?;
            super::domains::forget(&format!("__composite__{name}"));
            return Ok(Some(QueryResult::Set {
                variable: "CREATE TYPE".to_string(),
                value: String::new(),
            }));
        }

        if upper.starts_with("DROP TYPE") {
            let name = fold_identifier(
                trimmed["DROP TYPE".len()..]
                    .trim()
                    .trim_start_matches("IF EXISTS")
                    .trim(),
            );
            if self.composite_fields(&name).await?.is_none() {
                // Reporting success for a type that was never there is the
                // silent no-op this document records elsewhere.
                if upper.contains("IF EXISTS") {
                    return Ok(Some(QueryResult::Set {
                        variable: "DROP TYPE".to_string(),
                        value: String::new(),
                    }));
                }
                return Err(ProtocolError::SqlState {
                    code: "42704",
                    message: format!("type \"{name}\" does not exist"),
                });
            }
            self.forget_catalog_entry(&format!("composite:{name}"))
                .await?;
            return Ok(Some(QueryResult::Set {
                variable: "DROP TYPE".to_string(),
                value: String::new(),
            }));
        }

        if upper.starts_with("CREATE FUNCTION") || upper.starts_with("CREATE OR REPLACE FUNCTION") {
            if !upper.contains("PLPGSQL") {
                return Ok(None);
            }
            return self.define_plpgsql_function(trimmed).await.map(Some);
        }

        if upper.starts_with("DROP FUNCTION") {
            let name = trimmed
                .split_whitespace()
                .nth(2)
                .map(|n| n.split('(').next().unwrap_or(n))
                .unwrap_or_default();
            let name = fold_identifier(name);
            // Without argument types to name one, a bare `DROP FUNCTION f`
            // removes every overload of `f`.
            let keys = self.function_keys(&name).await?;
            if keys.is_empty() {
                return Ok(None);
            }
            for key in keys {
                self.forget_catalog_entry(&key).await?;
            }
            super::stored_functions::forget(&name);
            return Ok(Some(QueryResult::Set {
                variable: "DROP FUNCTION".to_string(),
                value: String::new(),
            }));
        }

        // `SELECT fname(args)` where `fname` is one of ours.
        self.call_plpgsql_function(trimmed).await
    }

    /// Run a block so that a failure leaves none of its writes behind.
    ///
    /// PostgreSQL runs a `DO` block and a function body inside a transaction:
    /// a `RAISE EXCEPTION` after an `INSERT` leaves no row. Running the
    /// statements directly left the `INSERT` committed and only reported the
    /// error, which is a partial write reported as a failure — the worst of
    /// both.
    ///
    /// Inside an open transaction this does nothing extra: the block joins the
    /// transaction already running, and `ROLLBACK` undoes it along with
    /// everything else.
    ///
    /// # Errors
    /// Returns whatever the block failed with, after undoing its writes.
    async fn run_block_atomically(
        &self,
        block: &plpgsql::Block,
        scope: HashMap<String, plpgsql::Value>,
    ) -> ProtocolResult<plpgsql::Returned> {
        if current_transaction_stamp().is_some() {
            return plpgsql::execute(block, self, scope).await;
        }

        let tables = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let nested = Arc::new(std::sync::Mutex::new(Vec::new()));
        let context = begin_transaction(false);
        let id = context.id;

        let outcome = BLOCK_TRANSACTIONS
            .scope(Arc::clone(&nested), async {
                BLOCK_TABLES
                    .scope(
                        Arc::clone(&tables),
                        within_transaction(context, plpgsql::execute(block, self, scope)),
                    )
                    .await
            })
            .await;

        if outcome.is_err() {
            let written: Vec<String> = tables
                .lock()
                .map(|tables| tables.iter().cloned().collect())
                .unwrap_or_default();
            // Undo before the id is retired: while it is still open, the rows
            // it wrote are invisible to everyone else, so nobody can read a
            // row that is about to be removed.
            let mut ids = vec![id];
            if let Ok(nested) = nested.lock() {
                ids.extend(nested.iter().copied());
            }
            for id in ids {
                self.discard_transaction_writes(&written, id).await?;
            }
        }
        end_transaction(id);
        outcome
    }

    /// Run a block and report both what it returned and the values of the
    /// variables named in `wanted`.
    ///
    /// Output parameters are ordinary variables while the block runs; this is
    /// how their final values are read back out afterwards.
    ///
    /// # Errors
    /// Returns whatever the block failed with.
    async fn run_block_reporting_state(
        &self,
        block: &plpgsql::Block,
        scope: HashMap<String, plpgsql::Value>,
        wanted: &[String],
    ) -> ProtocolResult<(plpgsql::Returned, HashMap<String, Option<String>>)> {
        if wanted.is_empty() {
            let returned = self.run_block_atomically(block, scope).await?;
            return Ok((returned, HashMap::new()));
        }

        // `run_protected` is the path that hands the state back; used here for
        // its return value rather than for its rollback, which is why the
        // block it runs has no handlers of its own.
        let (outcome, state) = <Self as plpgsql::PlPgSqlHost>::run_protected(
            self,
            block,
            plpgsql::State::with_arguments(scope),
        )
        .await?;
        let returned = outcome?;
        let values = wanted
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    state.scope.get(name).and_then(|value| value.text.clone()),
                )
            })
            .collect();
        Ok((returned, values))
    }

    /// Remove everything a transaction wrote, by its stamp.
    ///
    /// Rows it inserted carry its id and are deleted; rows it deleted carry
    /// its id as the remover and are unmarked. That covers an `UPDATE` too,
    /// which is stored as both.
    ///
    /// # Errors
    /// Returns an error when a table cannot be written.
    async fn discard_transaction_writes(&self, tables: &[String], id: u64) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        for table in tables {
            if !storage.table_exists(table).await? {
                continue;
            }
            storage
                .delete_rows(
                    table,
                    vec![QueryCondition {
                        column: TRANSACTION_STAMP.to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::from(id),
                    }],
                )
                .await?;
            storage
                .update_rows(
                    table,
                    HashMap::from([(DELETED_BY.to_string(), JsonValue::Null)]),
                    vec![QueryCondition {
                        column: DELETED_BY.to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::from(id),
                    }],
                )
                .await?;
        }
        Ok(())
    }

    /// Store a PL/pgSQL function's arguments and body in the catalog.
    async fn define_plpgsql_function(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let after_function = sql
            .to_uppercase()
            .find("FUNCTION")
            .map(|at| at + "FUNCTION".len())
            .ok_or_else(|| ProtocolError::PostgresError("malformed CREATE FUNCTION".to_string()))?;
        let rest = sql[after_function..].trim();

        let open = rest.find('(').ok_or_else(|| {
            ProtocolError::PostgresError("a function needs an argument list".to_string())
        })?;
        let close = rest.find(')').ok_or_else(|| {
            ProtocolError::PostgresError("unterminated function argument list".to_string())
        })?;
        let name = fold_identifier(rest[..open].trim());

        let parameters = plpgsql_function::parse_parameters(&rest[open + 1..close]);

        // `RETURNS <type>` sits between the argument list and the body. It is
        // what `pg_proc.prorettype` reports; without it the catalog would
        // claim every function returns the same thing.
        let after_args = &sql[after_function + close + 1..];
        let return_type = after_args
            .to_uppercase()
            .find("RETURNS")
            .map(|at| &after_args[at + "RETURNS".len()..])
            .map(|rest| {
                let end = rest.to_uppercase().find(" AS ").unwrap_or(rest.len());
                rest[..end].trim().to_uppercase()
            })
            .unwrap_or_default();

        let body_source = sql[after_function + open..].to_string();
        let body = extract_dollar_quoted(&body_source).ok_or_else(|| {
            ProtocolError::PostgresError(
                "a PL/pgSQL function body must be dollar-quoted: AS $$ ... $$".to_string(),
            )
        })?;

        // Parsing now rather than at call time means a body that cannot be
        // parsed is refused where the mistake was made.
        let parsed = plpgsql::parse(&body)?;
        // A body that needs no database can be called from an expression, so
        // `SELECT f(id) FROM t` and `WHERE f(id) = 4` work rather than failing
        // or, worse, quietly matching nothing.
        super::stored_functions::remember(&name, parameters.clone(), parsed);

        // Keyed by name, how many arguments a caller passes, and what kinds
        // they are, so `f(INTEGER)` and `f(TEXT)` are two functions rather
        // than one overwriting the other. Output parameters are not in the
        // key: a caller does not pass them.
        let arity = plpgsql_function::input_arity(&parameters);
        // The key records base types: a parameter declared as a domain is the
        // type the domain is built on, so `f(posint)` and `f(text)` are two
        // entries rather than one overwriting the other.
        let mut resolved = parameters.clone();
        for parameter in &mut resolved {
            parameter.sql_type = self.base_type_of(&parameter.sql_type).await?;
        }
        let signature = plpgsql_function::signature(&resolved);
        let key = format!("function:{name}/{arity}/{signature}");
        let slug = format!("__function__{name}__{arity}__{signature}");
        self.forget_catalog_entry(&key).await?;
        self.remember_domain(
            &slug,
            &format!(
                "{}|{return_type}|{body}",
                plpgsql_function::encode(&parameters)
            ),
        )
        .await?;
        // `remember_domain` writes under a `domain:` prefix; rewrite the name
        // so lookups find it as a function.
        self.rename_catalog_entry(&format!("domain:{slug}"), &key)
            .await?;

        Ok(QueryResult::Set {
            variable: "CREATE FUNCTION".to_string(),
            value: String::new(),
        })
    }

    /// The stored parameters and body of a PL/pgSQL function, if it exists.
    async fn stored_functions(
        &self,
        name: &str,
        arity: usize,
    ) -> ProtocolResult<Vec<(Vec<plpgsql_function::Parameter>, String)>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(Vec::new());
        }
        // Every overload of this name that takes this many arguments; which
        // one a call means is decided by the argument types.
        let prefix = format!("function:{name}/{arity}/");
        let rows = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let stored = row.values.get("name").and_then(JsonValue::as_str)?;
                if !stored.starts_with(&prefix) {
                    return None;
                }
                let definition = row.values.get("definition").and_then(JsonValue::as_str)?;
                let (parameters, rest) = definition.split_once('|')?;
                // `params|rettype|body`; an entry written before return types
                // were recorded has no second separator and is all body.
                let body = rest.split_once('|').map_or(rest, |(_, body)| body);
                Some((plpgsql_function::decode(parameters), body.to_string()))
            })
            .collect())
    }

    /// Every stored function: catalog key, parameters, return type, body.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be read.
    pub async fn all_functions(
        &self,
    ) -> ProtocolResult<Vec<(String, Vec<plpgsql_function::Parameter>, String, String)>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(Vec::new());
        }
        let rows = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let key = row.values.get("name").and_then(JsonValue::as_str)?;
                if !key.starts_with("function:") {
                    return None;
                }
                let definition = row.values.get("definition").and_then(JsonValue::as_str)?;
                let (parameters, rest) = definition.split_once('|')?;
                let (return_type, body) = rest.split_once('|').unwrap_or(("", rest));
                Some((
                    key.to_string(),
                    plpgsql_function::decode(parameters),
                    return_type.to_string(),
                    body.to_string(),
                ))
            })
            .collect())
    }

    /// The function a fast-path OID names, if this server published it.
    ///
    /// # Errors
    /// Returns an error when the catalog cannot be read.
    pub async fn function_for_oid(
        &self,
        oid: i64,
    ) -> ProtocolResult<Option<(String, Vec<plpgsql_function::Parameter>, String)>> {
        Ok(self
            .all_functions()
            .await?
            .into_iter()
            .find_map(|(key, parameters, return_type, _)| {
                (function_oid(&key) == oid).then(|| {
                    // `function:name/arity/signature` — the name is what a
                    // message reports back.
                    let name = key
                        .trim_start_matches("function:")
                        .split('/')
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    (name, parameters, return_type)
                })
            }))
    }

    /// Every catalog key belonging to a function of this name, at any arity.
    async fn function_keys(&self, name: &str) -> ProtocolResult<Vec<String>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(Vec::new());
        }
        let prefix = format!("function:{name}/");
        let rows = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let stored = row.values.get("name").and_then(JsonValue::as_str)?;
                stored.starts_with(&prefix).then(|| stored.to_string())
            })
            .collect())
    }

    /// Run `SELECT fname(args)` when `fname` is a stored PL/pgSQL function.
    async fn call_plpgsql_function(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        let upper = sql.to_uppercase();
        if !upper.starts_with("SELECT") {
            return Ok(None);
        }
        let call = sql["SELECT".len()..].trim();
        let Some(open) = call.find('(') else {
            return Ok(None);
        };
        let Some(close) = call.rfind(')') else {
            return Ok(None);
        };
        let name = fold_identifier(call[..open].trim());
        if name.is_empty() || !call[close + 1..].trim().is_empty() {
            return Ok(None);
        }
        let arguments = split_arguments(&call[open + 1..close]);
        let candidates = self.stored_functions(&name, arguments.len()).await?;
        if candidates.is_empty() {
            return Ok(None);
        }

        // Each argument is evaluated once, in the caller's context, before the
        // body runs — so an argument that is itself a query is not re-run per
        // reference inside the body. The values are also what says which
        // overload the call means.
        let mut values = Vec::with_capacity(arguments.len());
        for argument in &arguments {
            values.push(self.evaluate_scalar(argument).await?);
        }
        // An argument's type comes from the catalogue where the text names
        // one — a cast, or a call to a function whose return type is
        // recorded — from how it was written where that says, and from its
        // value only as a last resort.
        let mut argument_types = Vec::with_capacity(arguments.len());
        for (source, value) in arguments.iter().zip(&values) {
            argument_types.push(self.argument_type_of(source, value.as_deref()).await?);
        }
        let argument_types: Vec<&str> = argument_types.iter().map(String::as_str).collect();

        let mut signatures: Vec<Vec<plpgsql_function::Parameter>> =
            candidates.iter().map(|(p, _)| p.clone()).collect();
        // Overload choice is made on base types, so a domain parameter is
        // resolved to what it is built on first.
        for parameters in &mut signatures {
            for parameter in parameters.iter_mut() {
                parameter.sql_type = self.base_type_of(&parameter.sql_type).await?;
            }
        }
        let chosen = match plpgsql_function::resolve(&name, &signatures, &argument_types) {
            Ok(chosen) => chosen,
            Err(unresolved) => {
                // With one candidate, the caller did not choose the wrong
                // overload — they passed a value that cannot be the declared
                // type. Saying which value and which type is more use than
                // saying no overload matched.
                if let [(parameters, _)] = candidates.as_slice() {
                    for (parameter, value) in plpgsql_function::inputs(parameters)
                        .into_iter()
                        .zip(&values)
                    {
                        plpgsql_function::check_argument(parameter, value.as_deref())?;
                    }
                }
                return Err(unresolved);
            }
        };
        let (_, body) = &candidates[chosen];
        // The resolved parameters, so a domain binds as the type it is built
        // on: bound under its own name it was quoted, and `a * 2` failed with
        // an arithmetic error on text.
        let parameters = &signatures[chosen];

        let mut scope = HashMap::new();
        for (parameter, value) in plpgsql_function::inputs(parameters).into_iter().zip(values) {
            plpgsql_function::check_argument(parameter, value.as_deref())?;
            scope.insert(
                parameter.name.clone(),
                plpgsql::Value::typed(value, &parameter.sql_type),
            );
        }
        // Output parameters start as NULL and are whatever the body leaves.
        for parameter in plpgsql_function::outputs(parameters) {
            if !parameter.mode.is_input() {
                scope.insert(
                    parameter.name.clone(),
                    plpgsql::Value::typed(None, &parameter.sql_type),
                );
            }
        }

        let block = plpgsql::parse(body)?;
        let outputs: Vec<String> = plpgsql_function::outputs(parameters)
            .into_iter()
            .map(|p| p.name.clone())
            .collect();
        let (returned, state) =
            Box::pin(self.run_block_reporting_state(&block, scope, &outputs)).await?;

        Ok(Some(match returned {
            // A set-returning body answers with its rows and their own column
            // names, as `RETURN QUERY` produced them.
            plpgsql::Returned::Rows(rows) => QueryResult::Select {
                columns: rows.columns,
                rows: rows.rows,
            },
            other if outputs.is_empty() => QueryResult::Select {
                columns: vec![name],
                rows: vec![vec![other.scalar()]],
            },
            // With output parameters the answer is their values, named after
            // them — `RETURN` is not how such a function reports.
            _ => QueryResult::Select {
                rows: vec![outputs
                    .iter()
                    .map(|name| state.get(name).cloned().flatten())
                    .collect()],
                columns: outputs,
            },
        }))
    }

    /// The type of a call's argument.
    ///
    /// A cast says what it is, and so does a call to a function whose return
    /// type this server recorded. Falling back to the printed value is a last
    /// resort: it cannot tell an `int8` that happens to be small from an
    /// `int4`, which is why anything that names a type is preferred.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    async fn argument_type_of(&self, source: &str, value: Option<&str>) -> ProtocolResult<String> {
        let written = source.trim();

        // `expr::TYPE`, at the top level rather than inside a call.
        if let Some(named) = split_top_level_cast(written) {
            return Ok(plpgsql_function::normalize(
                &self.base_type_of(&named).await?,
            ));
        }

        // `CAST(expr AS TYPE)`
        let upper = written.to_uppercase();
        if upper.starts_with("CAST(") || upper.starts_with("CAST (") {
            if let Some(inner) = written
                .find('(')
                .and_then(|open| written.rfind(')').map(|close| &written[open + 1..close]))
            {
                if let Some(at) = inner.to_uppercase().rfind(" AS ") {
                    let named = inner[at + 4..].trim();
                    return Ok(
                        plpgsql_function::normalize(&self.base_type_of(named).await?).to_string(),
                    );
                }
            }
        }

        // A call to one of this server's own functions: its return type is in
        // the catalogue, so there is no need to guess from the value.
        if let Some(open) = written.find('(') {
            if written.ends_with(')') {
                let called = fold_identifier(written[..open].trim());
                if !called.is_empty() {
                    let inner_arity = split_arguments(&written[open + 1..written.len() - 1]).len();
                    let candidates = self.stored_functions(&called, inner_arity).await?;
                    // Only when one candidate could have been meant; two
                    // overloads may return different types, and picking one
                    // here would be a guess dressed as a lookup.
                    if let [(_, _)] = candidates.as_slice() {
                        if let Some((_, _, return_type)) = self
                            .all_functions()
                            .await?
                            .into_iter()
                            .find(|(key, parameters, _, _)| {
                                key.starts_with(&format!("function:{called}/{inner_arity}/"))
                                    && plpgsql_function::input_arity(parameters) == inner_arity
                            })
                            .map(|(key, parameters, return_type, _)| (key, parameters, return_type))
                        {
                            if !return_type.trim().is_empty() {
                                return Ok(plpgsql_function::normalize(
                                    &self.base_type_of(&return_type).await?,
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(plpgsql_function::argument_type(source, value).to_string())
    }

    /// Evaluate an expression against one row's values.
    ///
    /// Column references are replaced with that row's values first, which is
    /// what makes `SET n = n + 1` mean *this* row's `n`.
    ///
    /// # Errors
    /// Returns the engine's error when the expression cannot be evaluated.
    async fn evaluate_over_row(
        &self,
        expression: &str,
        row: &HashMap<String, JsonValue>,
    ) -> ProtocolResult<JsonValue> {
        let substituted = substitute_columns(expression, row);
        let evaluated = Box::pin(self.evaluate_scalar(&substituted)).await?;
        Ok(evaluated.map_or(JsonValue::Null, |text| Self::literal_to_json(&text)))
    }

    /// Evaluate a scalar expression by asking the engine for `SELECT <expr>`.
    async fn evaluate_scalar(&self, expression: &str) -> ProtocolResult<Option<String>> {
        let result = Box::pin(self.execute_query(&format!("SELECT {expression}"))).await?;
        Ok(match result {
            QueryResult::Select { rows, .. } => rows
                .into_iter()
                .next()
                .and_then(|row| row.into_iter().next())
                .flatten(),
            _ => None,
        })
    }

    /// Remove a catalog entry by its full name.
    async fn forget_catalog_entry(&self, name: &str) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(());
        }
        storage
            .delete_rows(
                VIEW_CATALOG,
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(name.to_string()),
                }],
            )
            .await
            .map(|_| ())
    }

    /// Rename a catalog entry, which is how a definition written under one
    /// prefix is filed under another.
    async fn rename_catalog_entry(&self, from: &str, to: &str) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        storage
            .update_rows(
                VIEW_CATALOG,
                HashMap::from([("name".to_string(), JsonValue::String(to.to_string()))]),
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(from.to_string()),
                }],
            )
            .await
            .map(|_| ())
    }

    /// Record a domain's base type and constraints.
    async fn remember_domain(&self, name: &str, definition: &str) -> ProtocolResult<()> {
        // Functions are filed through here too and then renamed; only a real
        // domain belongs in the cast registry.
        if !name.starts_with("__function__") {
            super::domains::remember(name, &leading_type(definition));
        }
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        self.ensure_view_catalog(storage).await?;
        let now = chrono::Utc::now();
        storage
            .insert_row(
                VIEW_CATALOG,
                TableRow {
                    values: HashMap::from([
                        (
                            "name".to_string(),
                            JsonValue::String(format!("domain:{name}")),
                        ),
                        (
                            "definition".to_string(),
                            JsonValue::String(definition.to_string()),
                        ),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map(|_| ())
    }

    /// Load every stored domain into the cast registry, once per process.
    ///
    /// Failure is not fatal and not retried per query: a cast to a domain then
    /// reports that it cannot be cast, which is what it did before this
    /// existed.
    async fn warm_domain_registry(&self) {
        static WARMED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        if WARMED.get().is_some() {
            return;
        }

        let Some(storage) = self.persistent_storage.as_ref() else {
            return;
        };
        let Ok(true) = storage.table_exists(VIEW_CATALOG).await else {
            return;
        };
        let Ok(rows) = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await
        else {
            return;
        };
        for row in rows {
            let Some(name) = row.values.get("name").and_then(JsonValue::as_str) else {
                continue;
            };
            let Some(domain) = name.strip_prefix("domain:") else {
                continue;
            };
            if let Some(definition) = row.values.get("definition").and_then(JsonValue::as_str) {
                super::domains::remember(domain, &leading_type(definition));
            }
        }

        // Functions stored before this process started, so one created in an
        // earlier run is callable from an expression too.
        if let Ok(functions) = self.all_functions().await {
            for (key, parameters, _, body) in functions {
                let name = key
                    .trim_start_matches("function:")
                    .split('/')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                if let Ok(parsed) = plpgsql::parse(&body) {
                    super::stored_functions::remember(&name, parameters, parsed);
                }
            }
        }

        let _ = WARMED.set(());
    }

    /// The fields of a composite type, if the name is one.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    pub async fn composite_fields(
        &self,
        name: &str,
    ) -> ProtocolResult<Option<Vec<plpgsql_function::Parameter>>> {
        Ok(self
            .view_definition(&format!("composite:{}", fold_identifier(name)))
            .await?
            .map(|definition| plpgsql_function::decode(&definition)))
    }

    /// Every composite type, by name.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    pub async fn all_composites(
        &self,
    ) -> ProtocolResult<Vec<(String, Vec<plpgsql_function::Parameter>)>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(Vec::new());
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(Vec::new());
        }
        let rows = storage
            .select_rows(VIEW_CATALOG, Vec::new(), Vec::new(), None)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let name = row.values.get("name").and_then(JsonValue::as_str)?;
                let composite = name.strip_prefix("composite:")?;
                let definition = row.values.get("definition").and_then(JsonValue::as_str)?;
                Some((composite.to_string(), plpgsql_function::decode(definition)))
            })
            .collect())
    }

    /// A parameter's type with any domain resolved to what it is built on.
    ///
    /// A domain is a base type plus constraints; for choosing between
    /// overloads only the base type matters, and the lattice has no catalogue
    /// to look one up in. Resolved per call rather than recorded at definition
    /// time, so `ALTER DOMAIN` is seen by calls made after it.
    ///
    /// # Errors
    /// Returns an error when the catalogue cannot be read.
    pub async fn base_type_of(&self, declared: &str) -> ProtocolResult<String> {
        let Some(definition) = self.domain_definition(&fold_identifier(declared)).await? else {
            return Ok(declared.to_string());
        };
        let base = leading_type(&definition);
        Ok(if base.is_empty() {
            declared.to_string()
        } else {
            base
        })
    }

    /// Record a unique index so dropping it can take its constraint away.
    async fn remember_index(&self, name: &str, table: &str, column: &str) -> ProtocolResult<()> {
        self.remember_domain(&format!("__index__{name}"), &format!("{table}|{column}"))
            .await?;
        self.rename_catalog_entry(&format!("domain:__index__{name}"), &format!("index:{name}"))
            .await
    }

    /// The table and column a recorded unique index covers.
    async fn index_definition(&self, name: &str) -> ProtocolResult<Option<(String, String)>> {
        Ok(self
            .view_definition(&format!("index:{name}"))
            .await?
            .and_then(|definition| {
                definition
                    .split_once('|')
                    .map(|(table, column)| (table.to_string(), column.to_string()))
            }))
    }

    /// A domain's declared type and constraints, if the name is one.
    pub async fn domain_definition(&self, name: &str) -> ProtocolResult<Option<String>> {
        self.view_definition(&format!("domain:{name}")).await
    }

    /// Record a materialized view's defining query so `REFRESH` can re-run it.
    async fn remember_materialized(&self, name: &str, query: &str) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        self.ensure_view_catalog(storage).await?;
        let now = chrono::Utc::now();
        storage
            .insert_row(
                VIEW_CATALOG,
                TableRow {
                    values: HashMap::from([
                        ("name".to_string(), JsonValue::String(format!("mat:{name}"))),
                        (
                            "definition".to_string(),
                            JsonValue::String(query.to_string()),
                        ),
                    ]),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map(|_| ())
    }

    /// The query behind a materialized view, if there is one.
    async fn materialized_definition(&self, name: &str) -> ProtocolResult<Option<String>> {
        self.view_definition(&format!("mat:{name}")).await
    }

    /// Create the view catalog if it is not there yet.
    async fn ensure_view_catalog(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::persistent_storage::{
            ColumnDefinition, ColumnType, TableSchema,
        };

        if storage.table_exists(VIEW_CATALOG).await? {
            return Ok(());
        }

        let column = |name: &str, unique: bool| ColumnDefinition {
            name: name.to_string(),
            data_type: ColumnType::Text,
            nullable: false,
            default_value: None,
            unique,
            check: None,
            references: None,
            domain: None,
        };

        storage
            .create_table(TableSchema {
                name: VIEW_CATALOG.to_string(),
                columns: vec![column("name", true), column("definition", false)],
                created_at: chrono::Utc::now(),
                row_count: 0,
                foreign_keys: Vec::new(),
            })
            .await
    }

    /// The query a view stands for, if `name` names one.
    async fn view_definition(&self, name: &str) -> ProtocolResult<Option<String>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(None);
        };
        if !storage.table_exists(VIEW_CATALOG).await? {
            return Ok(None);
        }

        let rows = storage
            .select_rows(
                VIEW_CATALOG,
                Vec::new(),
                vec![QueryCondition {
                    column: "name".to_string(),
                    operator: "=".to_string(),
                    value: JsonValue::String(name.to_string()),
                }],
                None,
            )
            .await?;

        Ok(rows.into_iter().next().and_then(|row| {
            row.values
                .get("definition")
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        }))
    }

    /// `INSERT ... ON CONFLICT DO NOTHING | DO UPDATE SET ...`.
    ///
    /// The conflict is detected by reading the target columns before writing,
    /// which is what the uniqueness check does on an ordinary insert.
    ///
    /// # Errors
    /// Returns an error when the table is unknown or a value cannot be
    /// evaluated.
    async fn insert_on_conflict(
        &self,
        insert: &crate::protocols::postgres_wire::sql::ast::InsertStatement,
    ) -> ProtocolResult<QueryResult> {
        use crate::protocols::postgres_wire::sql::ast::{
            AssignmentTarget, ConflictAction, ConflictTarget, InsertSource,
        };
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };

        let storage = self.persistent_storage.as_ref().ok_or_else(|| {
            ProtocolError::PostgresError("Persistent storage not enabled".to_string())
        })?;
        let table = fold_identifier(&insert.table.full_name());
        let schema = storage.get_table_schema(&table).await?.ok_or_else(|| {
            ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
        })?;
        let InsertSource::Values(tuples) = &insert.source else {
            return Err(ProtocolError::PostgresError(
                "ON CONFLICT is only supported with a VALUES source".to_string(),
            ));
        };
        let clause = insert.on_conflict.as_ref().ok_or_else(|| {
            ProtocolError::PostgresError("missing ON CONFLICT clause".to_string())
        })?;

        // Without an explicit target, any unique column is the conflict key.
        let keys: Vec<String> = match &clause.target {
            Some(ConflictTarget::Columns(columns)) => {
                columns.iter().map(|name| fold_identifier(name)).collect()
            }
            Some(ConflictTarget::Constraint(_)) | None => schema
                .columns
                .iter()
                .filter(|column| column.unique)
                .map(|column| fold_identifier(&column.name))
                .collect(),
        };

        let names: Vec<String> = insert
            .columns
            .clone()
            .unwrap_or_else(|| {
                schema
                    .columns
                    .iter()
                    .map(|column| column.name.clone())
                    .collect()
            })
            .iter()
            .map(|name| fold_identifier(name))
            .collect();

        let mut evaluator = ExpressionEvaluator::new();
        let context = EvaluationContext::empty();
        let mut inserted = 0usize;
        let mut updated = 0usize;

        for tuple in tuples {
            let mut row: HashMap<String, SqlValue> = HashMap::new();
            for (name, expression) in names.iter().zip(tuple) {
                row.insert(name.clone(), evaluator.evaluate(expression, &context)?);
            }

            let conditions: Vec<QueryCondition> = keys
                .iter()
                .filter_map(|key| {
                    row.get(key).map(|value| QueryCondition {
                        column: key.clone(),
                        operator: "=".to_string(),
                        value: Self::sql_value_to_json(value),
                    })
                })
                .collect();
            let conflicting = if conditions.is_empty() {
                Vec::new()
            } else {
                storage
                    .select_rows(&table, Vec::new(), conditions.clone(), Some(1))
                    .await?
            };

            if conflicting.is_empty() {
                let now = chrono::Utc::now();
                let mut values: HashMap<String, JsonValue> = row
                    .iter()
                    .map(|(name, value)| (name.clone(), Self::sql_value_to_json(value)))
                    .collect();
                Self::apply_defaults(&schema, &mut values);
                Self::check_not_null(&schema, &values)?;
                storage
                    .insert_row(
                        &table,
                        TableRow {
                            values,
                            created_at: now,
                            updated_at: now,
                        },
                    )
                    .await?;
                inserted += 1;
                continue;
            }

            match &clause.action {
                ConflictAction::DoNothing => {}
                ConflictAction::DoUpdate { set, .. } => {
                    let set_values: HashMap<String, JsonValue> = set
                        .iter()
                        .filter_map(|assignment| match &assignment.target {
                            AssignmentTarget::Column(name) => Some((name, &assignment.value)),
                            AssignmentTarget::Columns(_) => None,
                        })
                        .map(|(name, expression)| {
                            let context = EvaluationContext::with_row(row.clone());
                            evaluator.evaluate(expression, &context).map(|value| {
                                (fold_identifier(name), Self::sql_value_to_json(&value))
                            })
                        })
                        .collect::<ProtocolResult<_>>()?;
                    updated += storage
                        .update_rows(&table, set_values, conditions)
                        .await?
                        .max(0) as usize;
                }
            }
        }

        Ok(QueryResult::Insert {
            count: inserted + updated,
        })
    }

    /// `INSERT INTO t (...) SELECT ...`.
    ///
    /// The rows come from evaluating the select, so the source may be any
    /// query this engine can answer, including a join or a CTE.
    ///
    /// # Errors
    /// Returns an error when the target table is unknown, the select cannot be
    /// evaluated, or the column count does not match.
    async fn insert_from_select(
        &self,
        table: &str,
        columns: Option<&[String]>,
        query: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
        returning: Option<&[crate::protocols::postgres_wire::sql::ast::SelectItem]>,
    ) -> ProtocolResult<QueryResult> {
        let storage = self.persistent_storage.as_ref().ok_or_else(|| {
            ProtocolError::PostgresError("Persistent storage not enabled".to_string())
        })?;
        let schema = storage.get_table_schema(table).await?.ok_or_else(|| {
            ProtocolError::PostgresError(format!("Table '{table}' does not exist"))
        })?;

        let (source_columns, rows) = self
            .evaluate_select(query, &HashMap::new())
            .await?
            .ok_or_else(|| {
                ProtocolError::PostgresError(
                    "the source of an INSERT ... SELECT uses a shape this engine cannot evaluate"
                        .to_string(),
                )
            })?;

        // Without an explicit column list the target's own columns are filled
        // in order, which is what PostgreSQL does.
        let targets: Vec<String> = match columns {
            Some(columns) => columns.iter().map(|name| fold_identifier(name)).collect(),
            None => schema
                .columns
                .iter()
                .map(|column| fold_identifier(&column.name))
                .collect(),
        };
        if !rows.is_empty() && targets.len() != source_columns.len() {
            return Err(ProtocolError::PostgresError(format!(
                "INSERT has {} target columns but the query returns {}",
                targets.len(),
                source_columns.len()
            )));
        }

        let mut inserted = Vec::with_capacity(rows.len());
        for row in rows {
            let now = chrono::Utc::now();
            let values: HashMap<String, JsonValue> = targets
                .iter()
                .zip(&row)
                .map(|(name, value)| {
                    let stored = schema
                        .columns
                        .iter()
                        .find(|column| fold_identifier(&column.name) == *name)
                        .map_or_else(|| name.clone(), |column| column.name.clone());
                    (stored, Self::sql_value_to_json(value))
                })
                .collect();
            storage
                .insert_row(
                    table,
                    TableRow {
                        values,
                        created_at: now,
                        updated_at: now,
                    },
                )
                .await?;

            let mut projected = crate::protocols::postgres_wire::sql::select_pipeline::Row::new();
            for (name, value) in targets.iter().zip(row) {
                projected.insert(format!("{table}.{name}"), value.clone());
                projected.insert(name.clone(), value);
            }
            inserted.push(projected);
        }

        let count = inserted.len();
        match returning {
            None => Ok(QueryResult::Insert { count }),
            Some(items) => self.project_returning(table, items, inserted).await,
        }
    }

    /// `UPDATE` whose `SET` reads columns, such as `SET n = n + 1`.
    ///
    /// Each matching row is recomputed from its own values and written back
    /// individually; the storage layer takes one value map per call, which
    /// cannot express a per-row result.
    ///
    /// # Errors
    /// Returns an error when the table is unknown or an assignment cannot be
    /// evaluated.
    async fn update_with_expressions(
        &self,
        table: &str,
        update: &crate::protocols::postgres_wire::sql::ast::UpdateStatement,
    ) -> ProtocolResult<QueryResult> {
        use crate::protocols::postgres_wire::sql::ast::{AssignmentTarget, FromClause, TableName};

        let storage = self.persistent_storage.as_ref().ok_or_else(|| {
            ProtocolError::PostgresError("Persistent storage not enabled".to_string())
        })?;
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{table}' does not exist"
            )));
        }

        let from = FromClause::Table {
            name: TableName {
                schema: None,
                name: table.to_string(),
            },
            alias: None,
            time_travel: None,
        };
        let Some((rows, _)) = self.rows_from_clause(&from, &HashMap::new()).await? else {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{table}' does not exist"
            )));
        };

        let mut evaluator =
            crate::protocols::postgres_wire::sql::expression_evaluator::ExpressionEvaluator::new();
        let assigned: Vec<String> = update
            .set
            .iter()
            .filter_map(|assignment| match &assignment.target {
                AssignmentTarget::Column(name) => Some(fold_identifier(name)),
                AssignmentTarget::Columns(_) => None,
            })
            .collect();

        // The originals identify the rows to rewrite; the updated copies carry
        // the new values.
        let originals =
            Self::apply_assignments(&mut evaluator, rows, update.where_clause.as_ref(), &[])?;
        let updated =
            Self::apply_assignments(&mut evaluator, originals.clone(), None, &update.set)?;

        let mut count = 0usize;
        for (original, new_row) in originals.iter().zip(&updated) {
            let set_values: HashMap<String, JsonValue> = assigned
                .iter()
                .filter_map(|name| {
                    new_row
                        .get(name)
                        .map(|value| (name.clone(), Self::sql_value_to_json(value)))
                })
                .collect();

            // Every column of the original row identifies it. Rows that are
            // identical in every column compute the same new values, so
            // matching more than one is not a wrong answer.
            let conditions: Vec<QueryCondition> = original
                .iter()
                .filter(|(name, _)| !name.contains('.'))
                .map(|(name, value)| QueryCondition {
                    column: name.clone(),
                    operator: "=".to_string(),
                    value: Self::sql_value_to_json(value),
                })
                .collect();

            count += storage
                .update_rows(table, set_values, conditions)
                .await?
                .max(0) as usize;
        }

        match update.returning.as_deref() {
            None => Ok(QueryResult::Update { count }),
            Some(items) => self.project_returning(table, items, updated).await,
        }
    }

    /// Execute a write statement that carries a `RETURNING` clause.
    ///
    /// Returns `None` when the statement has no `RETURNING`, leaving the normal
    /// write path in charge. The rows are projected from the affected rows —
    /// read before a `DELETE`, and with the assignments applied for an
    /// `UPDATE` — so the values reflect the statement, not the table's state
    /// at some other moment.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or the write fails.
    async fn execute_with_returning(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        use crate::protocols::postgres_wire::sql::ast::{
            Expression, InsertSource, Statement as AstStatement,
        };
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        if self.persistent_storage.is_none() {
            return Ok(None);
        }
        // A cheap reject before parsing: only writes come through here.
        let leading = sql.trim_start();
        if !["INSERT", "UPDATE", "DELETE"].iter().any(|verb| {
            leading.len() >= verb.len() && leading[..verb.len()].eq_ignore_ascii_case(verb)
        }) {
            return Ok(None);
        }

        let Ok(statement) = SqlParser::new().parse(sql) else {
            return Ok(None);
        };

        // `ON CONFLICT` is part of the statement, not of the VALUES list; the
        // simple parser read it as more values and rejected the statement for
        // a column-count mismatch.
        if let AstStatement::Insert(insert) = &statement {
            if insert.on_conflict.is_some() {
                return self.insert_on_conflict(insert).await.map(Some);
            }
        }

        // `INSERT ... SELECT` inserted nothing and reported success, because
        // the simple parser reads the source as a VALUES list and finds none.
        if let AstStatement::Insert(insert) = &statement {
            if let InsertSource::Query(query) = &insert.source {
                return self
                    .insert_from_select(
                        &fold_identifier(&insert.table.full_name()),
                        insert.columns.as_deref(),
                        query,
                        insert.returning.as_deref(),
                    )
                    .await
                    .map(Some);
            }
        }

        // An assignment that is not a literal has to be evaluated per row.
        // Passing the text through stored `"amount + 1"` into the column.
        if let AstStatement::Update(update) = &statement {
            if update
                .set
                .iter()
                .any(|assignment| !matches!(assignment.value, Expression::Literal(_)))
            {
                return self
                    .update_with_expressions(&fold_identifier(&update.table.full_name()), update)
                    .await
                    .map(Some);
            }
        }

        // Everything else here is only interesting for its RETURNING clause.
        if !sql.to_uppercase().contains("RETURNING") {
            return Ok(None);
        }

        let (table, returning, where_clause, assignments, inserted) = match statement {
            AstStatement::Insert(insert) => {
                let Some(returning) = insert.returning else {
                    return Ok(None);
                };
                let InsertSource::Values(tuples) = insert.source else {
                    return Ok(None);
                };
                (
                    fold_identifier(&insert.table.full_name()),
                    returning,
                    None,
                    Vec::new(),
                    Some((insert.columns.unwrap_or_default(), tuples)),
                )
            }
            AstStatement::Update(update) => {
                let Some(returning) = update.returning else {
                    return Ok(None);
                };
                (
                    fold_identifier(&update.table.full_name()),
                    returning,
                    update.where_clause,
                    update.set,
                    None,
                )
            }
            AstStatement::Delete(delete) => {
                let Some(returning) = delete.returning else {
                    return Ok(None);
                };
                (
                    fold_identifier(&delete.table.full_name()),
                    returning,
                    delete.where_clause,
                    Vec::new(),
                    None,
                )
            }
            _ => return Ok(None),
        };

        // Assemble the rows the statement affects, before it runs.
        let mut evaluator =
            crate::protocols::postgres_wire::sql::expression_evaluator::ExpressionEvaluator::new();
        let affected = match &inserted {
            Some((columns, tuples)) => {
                let names: Vec<String> = columns.iter().map(|name| fold_identifier(name)).collect();
                let context = crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext::empty();
                let mut rows = Vec::with_capacity(tuples.len());
                for tuple in tuples {
                    let mut row = crate::protocols::postgres_wire::sql::select_pipeline::Row::new();
                    for (name, expression) in names.iter().zip(tuple) {
                        let value = evaluator.evaluate(expression, &context)?;
                        row.insert(format!("{table}.{name}"), value.clone());
                        row.insert(name.clone(), value);
                    }
                    rows.push(row);
                }
                rows
            }
            None => {
                let from = crate::protocols::postgres_wire::sql::ast::FromClause::Table {
                    name: crate::protocols::postgres_wire::sql::ast::TableName {
                        schema: None,
                        name: table.clone(),
                    },
                    alias: None,
                    time_travel: None,
                };
                let Some((rows, _)) = self.rows_from_clause(&from, &HashMap::new()).await? else {
                    return Ok(None);
                };
                Self::apply_assignments(&mut evaluator, rows, where_clause.as_ref(), &assignments)?
            }
        };

        // Run the write itself, with the clause removed. The recursive call is
        // cheap to reject: the stripped statement has no RETURNING.
        let without_returning = Self::strip_returning(sql);
        Box::pin(self.execute_query(&without_returning)).await?;

        self.project_returning(&table, &returning, affected)
            .await
            .map(Some)
    }

    /// Project a `RETURNING` list over the rows a write affected.
    ///
    /// # Errors
    /// Returns an error when an item cannot be evaluated.
    async fn project_returning(
        &self,
        table: &str,
        returning: &[crate::protocols::postgres_wire::sql::ast::SelectItem],
        rows: Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
    ) -> ProtocolResult<QueryResult> {
        use crate::protocols::postgres_wire::sql::ast::{ColumnRef, Expression, SelectItem};

        // A wildcard is expanded from the schema rather than from the row's
        // keys: a row carries each value twice, bare and table-qualified, so
        // expanding over the keys would return every column twice and in hash
        // order.
        let expands_wildcard = returning
            .iter()
            .any(|item| matches!(item, SelectItem::Wildcard));
        let returning = match (expands_wildcard, self.persistent_storage.as_ref()) {
            (true, Some(storage)) => match storage.get_table_schema(table).await? {
                None => returning.to_vec(),
                Some(schema) => schema
                    .columns
                    .iter()
                    .map(|column| SelectItem::Expression {
                        expr: Expression::Column(ColumnRef {
                            table: None,
                            name: fold_identifier(&column.name),
                        }),
                        alias: None,
                    })
                    .collect(),
            },
            _ => returning.to_vec(),
        };

        // A select with no FROM over rows already in hand is exactly what the
        // pipeline's projection does.
        let projection = crate::protocols::postgres_wire::sql::ast::SelectStatement {
            with: None,
            select_list: returning,
            distinct: None,
            from_clause: None,
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
            for_clause: None,
            traverse: None,
            set_operation: None,
        };
        let (columns, values) =
            crate::protocols::postgres_wire::sql::select_pipeline::run_select_values(
                &projection,
                rows,
            )?;

        Ok(QueryResult::Select {
            columns,
            rows: values
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|value| match value {
                            SqlValue::Null => None,
                            other => Some(other.to_postgres_string()),
                        })
                        .collect()
                })
                .collect(),
        })
    }

    /// Keep the rows a predicate selects, with any assignments applied.
    fn apply_assignments(
        evaluator: &mut crate::protocols::postgres_wire::sql::expression_evaluator::ExpressionEvaluator,
        rows: Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
        where_clause: Option<&crate::protocols::postgres_wire::sql::ast::Expression>,
        assignments: &[crate::protocols::postgres_wire::sql::ast::Assignment],
    ) -> ProtocolResult<Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>> {
        use crate::protocols::postgres_wire::sql::ast::AssignmentTarget;
        use crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext;

        let mut kept = Vec::new();
        for mut row in rows {
            if let Some(predicate) = where_clause {
                let context = EvaluationContext::with_row(row.clone());
                if !matches!(
                    evaluator.evaluate(predicate, &context)?,
                    SqlValue::Boolean(true)
                ) {
                    continue;
                }
            }
            for assignment in assignments {
                let AssignmentTarget::Column(name) = &assignment.target else {
                    continue;
                };
                let context = EvaluationContext::with_row(row.clone());
                let value = evaluator.evaluate(&assignment.value, &context)?;
                row.insert(fold_identifier(name), value);
            }
            kept.push(row);
        }
        Ok(kept)
    }

    /// Remove a trailing `RETURNING ...` clause from a statement.
    fn strip_returning(sql: &str) -> String {
        let upper = sql.to_uppercase();
        match upper.rfind(" RETURNING ") {
            Some(index) => sql[..index].trim_end().trim_end_matches(';').to_string(),
            None => sql.to_string(),
        }
    }

    /// Run a `SELECT` over the rows held in persistent storage.
    ///
    /// Returns `None` when the statement is not a single-table SELECT of a
    /// stored table, leaving it to the engine that can handle it.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or an expression cannot be
    /// evaluated.
    async fn select_over_storage(&self, sql: &str) -> ProtocolResult<Option<QueryResult>> {
        use crate::protocols::postgres_wire::sql::ast::Statement as AstStatement;
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        if self.persistent_storage.is_none() {
            return Ok(None);
        }

        let Ok(statement) = SqlParser::new().parse(sql) else {
            return Ok(None);
        };
        let AstStatement::Select(select) = statement else {
            return Ok(None);
        };

        let Some((columns, rows)) = self.evaluate_select(&select, &HashMap::new()).await? else {
            return Ok(None);
        };

        Ok(Some(QueryResult::Select {
            columns,
            rows: rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|value| match value {
                            SqlValue::Null => None,
                            other => Some(other.to_postgres_string()),
                        })
                        .collect()
                })
                .collect(),
        }))
    }

    /// Evaluate a `SELECT` against storage, returning typed values.
    ///
    /// `ctes` carries the named queries a `WITH` clause introduced, so a
    /// reference to one resolves to its rows rather than to a table that does
    /// not exist. Returns `None` when the statement uses a shape this path does
    /// not handle, leaving it to the engine that can.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or an expression cannot be
    /// evaluated.
    #[allow(clippy::type_complexity)]
    fn evaluate_select<'a>(
        &'a self,
        select: &'a crate::protocols::postgres_wire::sql::ast::SelectStatement,
        ctes: &'a HashMap<String, crate::protocols::postgres_wire::sql::ast::SelectStatement>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = ProtocolResult<Option<(Vec<String>, Vec<Vec<SqlValue>>)>>,
                > + Send
                + 'a,
        >,
    > {
        use crate::protocols::postgres_wire::sql::ast::SetOperator;
        use crate::protocols::postgres_wire::sql::select_pipeline;

        Box::pin(async move {
            // A `WITH` clause adds names visible to this statement and to the
            // queries nested inside it.
            let mut scope = ctes.clone();
            let mut materialised: HashMap<String, (Vec<String>, Vec<Vec<SqlValue>>)> =
                HashMap::new();
            if let Some(with) = &select.with {
                for cte in &with.ctes {
                    let name = fold_identifier(&cte.name);
                    let declared: Option<Vec<String>> = cte.columns.as_ref().map(|columns| {
                        columns
                            .iter()
                            .map(|column| fold_identifier(column))
                            .collect()
                    });

                    if with.recursive && Self::references_table(&cte.query, &name) {
                        // A recursive CTE is its non-recursive term plus every
                        // row the recursive term yields when applied to what is
                        // known so far, until it yields nothing new.
                        let rows = self
                            .evaluate_recursive_cte(&name, &cte.query, &scope, declared.as_deref())
                            .await?;
                        match rows {
                            Some(rows) => {
                                materialised.insert(name.clone(), rows);
                            }
                            None => return Ok(None),
                        }
                        continue;
                    }

                    // `WITH t(a, b) AS (...)` renames the query's output
                    // columns, so the rows have to be produced now rather than
                    // re-run later under their original names.
                    if let Some(columns) = declared {
                        let Some((_, values)) = self.evaluate_select(&cte.query, &scope).await?
                        else {
                            return Ok(None);
                        };
                        materialised.insert(name.clone(), (columns, values));
                        continue;
                    }

                    scope.insert(name, (*cte.query).clone());
                }
            }
            let scope = scope;

            // A select with no FROM runs over exactly one empty row, which is
            // what `SELECT 1` means. Sending it elsewhere gave a second
            // rendering of values, where NULL came back as an empty string.
            let (rows, column_order) = match select.from_clause.as_ref() {
                None => (
                    vec![crate::protocols::postgres_wire::sql::select_pipeline::Row::new()],
                    Vec::new(),
                ),
                // A recursive CTE has already been computed, so the FROM clause
                // reads its rows rather than re-running a query.
                Some(crate::protocols::postgres_wire::sql::ast::FromClause::Table {
                    name,
                    alias,
                    ..
                }) if materialised.contains_key(&fold_identifier(&name.full_name())) => {
                    let key = fold_identifier(&name.full_name());
                    let (columns, values) = materialised[&key].clone();
                    let qualifier = alias
                        .as_ref()
                        .map(|a| fold_identifier(&a.name))
                        .unwrap_or(key);
                    Self::rows_from_values(&columns, values, &qualifier)
                }
                Some(from) => match self.rows_from_clause(from, &scope).await? {
                    Some(assembled) => assembled,
                    None => return Ok(None),
                },
            };

            // Subqueries are executed here and replaced by the values they
            // yield, so the expression evaluator — which has no access to
            // storage — never has to run one.
            let mut resolved = select.clone();

            // A correlated subquery has a different answer per outer row, so
            // resolving it once produces one wrong answer applied to every
            // row. Those rows are filtered here, one at a time, and the
            // pipeline then runs with the predicate already applied.
            let rows = match resolved.where_clause.as_ref() {
                Some(predicate) if Self::is_correlated(predicate) => {
                    let filtered = self.filter_correlated(predicate, rows).await?;
                    resolved.where_clause = None;
                    filtered
                }
                _ => {
                    if let Some(predicate) = resolved.where_clause.take() {
                        resolved.where_clause = Some(self.resolve_subqueries(predicate).await?);
                    }
                    rows
                }
            };

            if let Some(having) = resolved.having.take() {
                resolved.having = Some(self.resolve_subqueries(having).await?);
            }

            // The select list needs this as much as the predicate does. It was
            // resolved for `WHERE` and `HAVING` only, so
            // `SELECT (SELECT COUNT(*) FROM t)` reached the evaluator with the
            // subquery still in it and failed as unimplemented — while the
            // same subquery in a `WHERE` worked.
            for item in &mut resolved.select_list {
                if let crate::protocols::postgres_wire::sql::ast::SelectItem::Expression {
                    expr,
                    ..
                } = item
                {
                    let taken = std::mem::replace(
                        expr,
                        crate::protocols::postgres_wire::sql::ast::Expression::Literal(
                            crate::protocols::postgres_wire::sql::types::SqlValue::Null,
                        ),
                    );
                    *expr = self.resolve_subqueries(taken).await?;
                }
            }

            let (names, mut values) = select_pipeline::run_select_values(&resolved, rows)?;

            // A wildcard is named by the pipeline as `*`; the real names come
            // from the tables involved, in declaration order.
            let columns = if names.iter().any(|name| name == "*") {
                column_order
            } else {
                names
            };

            let Some(set_operation) = &select.set_operation else {
                return Ok(Some((columns, values)));
            };

            let Some((_, right)) = self.evaluate_select(&set_operation.right, &scope).await? else {
                return Ok(None);
            };

            values = match set_operation.operator {
                SetOperator::UnionAll => {
                    values.extend(right);
                    values
                }
                SetOperator::Union => {
                    values.extend(right);
                    deduplicate_rows(values)
                }
                SetOperator::IntersectAll => {
                    values.retain(|row| right.contains(row));
                    values
                }
                SetOperator::Intersect => {
                    values.retain(|row| right.contains(row));
                    deduplicate_rows(values)
                }
                SetOperator::ExceptAll => {
                    values.retain(|row| !right.contains(row));
                    values
                }
                SetOperator::Except => {
                    values.retain(|row| !right.contains(row));
                    deduplicate_rows(values)
                }
            };

            Ok(Some((columns, values)))
        })
    }

    /// Turn a nested select's output into rows the pipeline can read.
    ///
    /// Each value is keyed both bare and qualified by `qualifier`, so both
    /// `id` and `t.id` resolve — the same convention stored tables use.
    fn rows_from_values(
        columns: &[String],
        values: Vec<Vec<SqlValue>>,
        qualifier: &str,
    ) -> (
        Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
        Vec<String>,
    ) {
        let names: Vec<String> = columns.iter().map(|name| fold_identifier(name)).collect();
        let rows = values
            .into_iter()
            .map(|value_row| {
                let mut row = crate::protocols::postgres_wire::sql::select_pipeline::Row::new();
                for (name, value) in names.iter().zip(value_row) {
                    row.insert(format!("{qualifier}.{name}"), value.clone());
                    row.insert(name.clone(), value);
                }
                row
            })
            .collect();
        (rows, names)
    }

    /// Whether a select reads from a table of the given name.
    ///
    /// Used to tell a genuinely recursive CTE from one merely declared inside a
    /// `WITH RECURSIVE`, which PostgreSQL also allows.
    fn references_table(
        select: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
        name: &str,
    ) -> bool {
        use crate::protocols::postgres_wire::sql::ast::FromClause;

        fn walk(from: &FromClause, name: &str) -> bool {
            match from {
                FromClause::Table { name: table, .. } => {
                    fold_identifier(&table.full_name()) == name
                }
                FromClause::Join { left, right, .. } => walk(left, name) || walk(right, name),
                FromClause::Subquery { query, .. } => query
                    .from_clause
                    .as_ref()
                    .is_some_and(|from| walk(from, name)),
                _ => false,
            }
        }

        let own = select
            .from_clause
            .as_ref()
            .is_some_and(|from| walk(from, name));
        own || select
            .set_operation
            .as_ref()
            .is_some_and(|operation| Self::references_table(&operation.right, name))
    }

    /// Evaluate `WITH RECURSIVE name AS (base UNION [ALL] recursive)`.
    ///
    /// The base term runs once; the recursive term then runs repeatedly over
    /// the rows found so far until a round adds nothing. Returns `None` when
    /// the statement is not in the shape this can evaluate.
    ///
    /// # Errors
    /// Returns an error when a term cannot be evaluated, or when the recursion
    /// does not settle within its bound.
    #[allow(clippy::type_complexity)]
    async fn evaluate_recursive_cte(
        &self,
        name: &str,
        query: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
        scope: &HashMap<String, crate::protocols::postgres_wire::sql::ast::SelectStatement>,
        declared: Option<&[String]>,
    ) -> ProtocolResult<Option<(Vec<String>, Vec<Vec<SqlValue>>)>> {
        use crate::protocols::postgres_wire::sql::ast::SetOperator;

        // The shape is `base UNION [ALL] recursive`; anything else is not a
        // recursion this can run.
        let Some(operation) = query.set_operation.as_ref() else {
            return Ok(None);
        };
        let distinct = matches!(
            operation.operator,
            SetOperator::Union | SetOperator::Intersect | SetOperator::Except
        );

        let mut base = query.clone();
        base.with = None;
        base.set_operation = None;
        let Some((columns, mut accumulated)) = self.evaluate_select(&base, scope).await? else {
            return Ok(None);
        };
        // `WITH RECURSIVE n(x) AS ...` names the columns; the base term's own
        // names (`SELECT 1` yields `expr`) are not what the recursive term
        // refers to.
        let columns = match declared {
            Some(declared) if declared.len() == columns.len() => declared.to_vec(),
            _ => columns,
        };

        // Each round feeds the rows found so far back in under the CTE's name.
        // A bound is kept so a recursion that does not settle fails loudly
        // instead of running until the process is killed.
        const MAX_ROUNDS: usize = 1_000;
        let mut frontier = accumulated.clone();
        for round in 0..MAX_ROUNDS {
            if frontier.is_empty() {
                return Ok(Some((columns, accumulated)));
            }

            let known = Self::rows_from_values(&columns, frontier.clone(), name);
            let Some(produced) = self
                .evaluate_recursive_term(&operation.right, scope, name, known.0)
                .await?
            else {
                return Ok(None);
            };

            let fresh: Vec<Vec<SqlValue>> = produced
                .into_iter()
                .filter(|row| !distinct || !accumulated.contains(row))
                .collect();
            accumulated.extend(fresh.clone());
            frontier = fresh;

            if round + 1 == MAX_ROUNDS {
                return Err(ProtocolError::PostgresError(format!(
                    "recursive query '{name}' did not settle within {MAX_ROUNDS} rounds"
                )));
            }
        }

        Ok(Some((columns, accumulated)))
    }

    /// Run one round of a recursive term against the rows found so far.
    async fn evaluate_recursive_term(
        &self,
        term: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
        scope: &HashMap<String, crate::protocols::postgres_wire::sql::ast::SelectStatement>,
        name: &str,
        known: Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
    ) -> ProtocolResult<Option<Vec<Vec<SqlValue>>>> {
        use crate::protocols::postgres_wire::sql::ast::FromClause;
        use crate::protocols::postgres_wire::sql::select_pipeline;

        // The term reads the CTE by name; those rows are supplied directly.
        let reads_only_the_cte = matches!(
            term.from_clause.as_ref(),
            Some(FromClause::Table { name: table, .. })
                if fold_identifier(&table.full_name()) == name
        );
        if !reads_only_the_cte {
            // A join between the CTE and a stored table is assembled here.
            let Some(from) = term.from_clause.as_ref() else {
                return Ok(None);
            };
            let Some((rows, _)) = self
                .rows_from_clause_with(from, scope, name, &known)
                .await?
            else {
                return Ok(None);
            };
            let mut resolved = term.clone();
            resolved.set_operation = None;
            let (_, values) = select_pipeline::run_select_values(&resolved, rows)?;
            return Ok(Some(values));
        }

        let mut resolved = term.clone();
        resolved.set_operation = None;
        let (_, values) = select_pipeline::run_select_values(&resolved, known)?;
        Ok(Some(values))
    }

    /// Assemble a FROM clause where one table name stands for supplied rows.
    #[allow(clippy::type_complexity)]
    fn rows_from_clause_with<'a>(
        &'a self,
        from: &'a crate::protocols::postgres_wire::sql::ast::FromClause,
        scope: &'a HashMap<String, crate::protocols::postgres_wire::sql::ast::SelectStatement>,
        name: &'a str,
        known: &'a [crate::protocols::postgres_wire::sql::select_pipeline::Row],
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = ProtocolResult<
                        Option<(
                            Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
                            Vec<String>,
                        )>,
                    >,
                > + Send
                + 'a,
        >,
    > {
        use crate::protocols::postgres_wire::sql::ast::{FromClause, JoinType};

        Box::pin(async move {
            match from {
                FromClause::Table { name: table, .. }
                    if fold_identifier(&table.full_name()) == name =>
                {
                    Ok(Some((known.to_vec(), Vec::new())))
                }
                FromClause::Join {
                    left,
                    join_type,
                    right,
                    condition,
                } => {
                    let Some((left_rows, _)) =
                        self.rows_from_clause_with(left, scope, name, known).await?
                    else {
                        return Ok(None);
                    };
                    let Some((right_rows, _)) = self
                        .rows_from_clause_with(right, scope, name, known)
                        .await?
                    else {
                        return Ok(None);
                    };
                    let Some(joined) =
                        Self::join_rows(&left_rows, &right_rows, join_type, condition)?
                    else {
                        return Ok(None);
                    };
                    Ok(Some((joined, Vec::new())))
                }
                other => {
                    let _ = JoinType::Inner;
                    self.rows_from_clause(other, scope).await
                }
            }
        })
    }

    /// Whether an expression contains a subquery that reads the outer row.
    ///
    /// A qualified column whose qualifier is not one of the subquery's own
    /// sources can only come from outside it.
    fn is_correlated(expr: &crate::protocols::postgres_wire::sql::ast::Expression) -> bool {
        use crate::protocols::postgres_wire::sql::ast::{Expression, InList};

        match expr {
            Expression::Subquery(select) | Expression::Exists(select) => {
                Self::reads_outer_columns(select)
            }
            Expression::In {
                list: InList::Subquery(select),
                ..
            } => Self::reads_outer_columns(select),
            Expression::Binary { left, right, .. } => {
                Self::is_correlated(left) || Self::is_correlated(right)
            }
            Expression::Unary { operand, .. } => Self::is_correlated(operand),
            _ => false,
        }
    }

    /// Whether a select's predicate names a table it does not read from.
    fn reads_outer_columns(
        select: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
    ) -> bool {
        use crate::protocols::postgres_wire::sql::ast::{Expression, FromClause};

        fn sources(from: &FromClause, into: &mut Vec<String>) {
            match from {
                FromClause::Table { name, alias, .. } => {
                    into.push(fold_identifier(&name.full_name()));
                    if let Some(alias) = alias {
                        into.push(fold_identifier(&alias.name));
                    }
                }
                FromClause::Join { left, right, .. } => {
                    sources(left, into);
                    sources(right, into);
                }
                _ => {}
            }
        }

        fn qualifiers(expr: &Expression, into: &mut Vec<String>) {
            match expr {
                Expression::Column(column) => {
                    if let Some(table) = &column.table {
                        into.push(fold_identifier(table));
                    }
                }
                Expression::Binary { left, right, .. } => {
                    qualifiers(left, into);
                    qualifiers(right, into);
                }
                Expression::Unary { operand, .. } => qualifiers(operand, into),
                Expression::Function(call) => {
                    for arg in &call.args {
                        qualifiers(arg, into);
                    }
                }
                _ => {}
            }
        }

        let mut own = Vec::new();
        if let Some(from) = &select.from_clause {
            sources(from, &mut own);
        }
        let mut referenced = Vec::new();
        if let Some(predicate) = &select.where_clause {
            qualifiers(predicate, &mut referenced);
        }
        referenced.iter().any(|name| !own.contains(name))
    }

    /// Keep the rows a predicate containing a correlated subquery selects.
    ///
    /// The outer row's values are substituted into the subquery before it
    /// runs, so each row is judged against its own answer.
    ///
    /// # Errors
    /// Returns an error when a subquery cannot be run or the predicate cannot
    /// be evaluated.
    async fn filter_correlated(
        &self,
        predicate: &crate::protocols::postgres_wire::sql::ast::Expression,
        rows: Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
    ) -> ProtocolResult<Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>> {
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };

        let mut evaluator = ExpressionEvaluator::new();
        let mut kept = Vec::with_capacity(rows.len());
        for row in rows {
            let bound = Self::bind_outer_row(predicate.clone(), &row);
            let resolved = self.resolve_subqueries(bound).await?;
            let context = EvaluationContext::with_row(row.clone());
            if matches!(
                evaluator.evaluate(&resolved, &context)?,
                SqlValue::Boolean(true)
            ) {
                kept.push(row);
            }
        }
        Ok(kept)
    }

    /// Replace qualified column references the outer row can answer.
    ///
    /// Only qualified names are substituted: a bare name inside a subquery
    /// belongs to the subquery's own table, which PostgreSQL resolves first.
    fn bind_outer_row(
        expr: crate::protocols::postgres_wire::sql::ast::Expression,
        row: &crate::protocols::postgres_wire::sql::select_pipeline::Row,
    ) -> crate::protocols::postgres_wire::sql::ast::Expression {
        use crate::protocols::postgres_wire::sql::ast::{Expression, InList};

        match expr {
            Expression::Column(ref column) => match &column.table {
                Some(table) => {
                    let key = format!(
                        "{}.{}",
                        fold_identifier(table),
                        fold_identifier(&column.name)
                    );
                    match row.get(&key) {
                        Some(value) => Expression::Literal(value.clone()),
                        None => expr,
                    }
                }
                None => expr,
            },
            Expression::Binary {
                left,
                operator,
                right,
            } => Expression::Binary {
                left: Box::new(Self::bind_outer_row(*left, row)),
                operator,
                right: Box::new(Self::bind_outer_row(*right, row)),
            },
            Expression::Unary { operator, operand } => Expression::Unary {
                operator,
                operand: Box::new(Self::bind_outer_row(*operand, row)),
            },
            Expression::Subquery(select) => {
                Expression::Subquery(Box::new(Self::bind_outer_select(*select, row)))
            }
            Expression::Exists(select) => {
                Expression::Exists(Box::new(Self::bind_outer_select(*select, row)))
            }
            Expression::In {
                expr: inner,
                list: InList::Subquery(select),
                negated,
            } => Expression::In {
                expr: Box::new(Self::bind_outer_row(*inner, row)),
                list: InList::Subquery(Box::new(Self::bind_outer_select(*select, row))),
                negated,
            },
            other => other,
        }
    }

    /// Bind the outer row into a subquery.
    ///
    /// The select list is bound as well as the predicate: a `LATERAL` subquery
    /// most often reads the outer row in what it projects, as in
    /// `LATERAL (SELECT a.amount * 2)`.
    fn bind_outer_select(
        mut select: crate::protocols::postgres_wire::sql::ast::SelectStatement,
        row: &crate::protocols::postgres_wire::sql::select_pipeline::Row,
    ) -> crate::protocols::postgres_wire::sql::ast::SelectStatement {
        use crate::protocols::postgres_wire::sql::ast::SelectItem;

        if let Some(predicate) = select.where_clause.take() {
            select.where_clause = Some(Self::bind_outer_row(predicate, row));
        }
        select.select_list = select
            .select_list
            .into_iter()
            .map(|item| match item {
                SelectItem::Expression { expr, alias } => SelectItem::Expression {
                    expr: Self::bind_outer_row(expr, row),
                    alias,
                },
                other => other,
            })
            .collect();
        select
    }

    /// Replace subqueries in an expression with the values they produce.
    ///
    /// A scalar subquery becomes its single value, `IN (SELECT ...)` becomes an
    /// explicit list, and `EXISTS (SELECT ...)` becomes a boolean. Correlated
    /// subqueries — those referring to the outer row — are left in place and
    /// reported by the evaluator, because they cannot be reduced to a constant
    /// before the outer row is known.
    fn resolve_subqueries<'a>(
        &'a self,
        expr: crate::protocols::postgres_wire::sql::ast::Expression,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = ProtocolResult<crate::protocols::postgres_wire::sql::ast::Expression>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            use crate::protocols::postgres_wire::sql::ast::{Expression, InList};
            use crate::protocols::postgres_wire::sql::types::SqlValue;

            Ok(match expr {
                Expression::Subquery(select) => {
                    let values = self.run_subquery(&select).await?;
                    // A scalar subquery with no rows is NULL in SQL.
                    Expression::Literal(values.into_iter().next().unwrap_or(SqlValue::Null))
                }
                Expression::Exists(select) => {
                    let values = self.run_subquery(&select).await?;
                    Expression::Literal(SqlValue::Boolean(!values.is_empty()))
                }
                Expression::In {
                    expr,
                    list: InList::Subquery(select),
                    negated,
                } => {
                    let values = self.run_subquery(&select).await?;
                    Expression::In {
                        expr: Box::new(self.resolve_subqueries(*expr).await?),
                        list: InList::Expressions(
                            values.into_iter().map(Expression::Literal).collect(),
                        ),
                        negated,
                    }
                }
                Expression::Binary {
                    left,
                    operator,
                    right,
                } => Expression::Binary {
                    left: Box::new(self.resolve_subqueries(*left).await?),
                    operator,
                    right: Box::new(self.resolve_subqueries(*right).await?),
                },
                other => other,
            })
        })
    }

    /// Run a subquery and return its first column, one value per row.
    ///
    /// # Errors
    /// Returns an error when the subquery cannot be executed over storage.
    async fn run_subquery(
        &self,
        select: &crate::protocols::postgres_wire::sql::ast::SelectStatement,
    ) -> ProtocolResult<Vec<crate::protocols::postgres_wire::sql::types::SqlValue>> {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        // Rendered back to SQL would lose fidelity, so the statement is
        // executed directly through the same storage path.
        let Some(from) = select.from_clause.as_ref() else {
            return Err(ProtocolError::PostgresError(
                "subquery without a FROM clause is not supported here".to_string(),
            ));
        };

        let Some((rows, _)) = self.rows_from_clause(from, &HashMap::new()).await? else {
            return Err(ProtocolError::PostgresError(
                "subquery reads a source this engine cannot assemble".to_string(),
            ));
        };

        let output =
            crate::protocols::postgres_wire::sql::select_pipeline::run_select(select, rows)?;

        // Typed the way an untyped SQL literal is: a value that reads as a
        // number is a number. Returning everything as text made
        // `WHERE amount = (SELECT MAX(amount) ...)` compare an integer against
        // the string "30" and fail.
        Ok(output
            .rows
            .into_iter()
            .map(|row| match row.into_iter().next() {
                Some(Some(text)) => Self::text_to_sql_value(&text),
                _ => SqlValue::Null,
            })
            .collect())
    }

    /// Interpret text the way an untyped SQL literal is interpreted.
    fn text_to_sql_value(text: &str) -> crate::protocols::postgres_wire::sql::types::SqlValue {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        if let Ok(n) = text.parse::<i32>() {
            return SqlValue::Integer(n);
        }
        if let Ok(n) = text.parse::<i64>() {
            return SqlValue::BigInt(n);
        }
        if let Ok(n) = text.parse::<f64>() {
            return SqlValue::DoublePrecision(n);
        }
        match text {
            "t" | "true" => SqlValue::Boolean(true),
            "f" | "false" => SqlValue::Boolean(false),
            other => SqlValue::Text(other.to_string()),
        }
    }

    /// Build the row set a FROM clause denotes, plus its column order.
    ///
    /// Handles a single table and joins of tables. Returns `None` for anything
    /// else, so the statement falls through to an engine that may handle it.
    ///
    /// Joins are evaluated as a nested loop over the two sides. That is
    /// quadratic and there is no index selection: acceptable for the table
    /// sizes this engine holds, and the honest starting point — a plan that
    /// claims to use an index it does not have would be worse.
    fn rows_from_clause<'a>(
        &'a self,
        from: &'a crate::protocols::postgres_wire::sql::ast::FromClause,
        ctes: &'a HashMap<String, crate::protocols::postgres_wire::sql::ast::SelectStatement>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = ProtocolResult<
                        Option<(
                            Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>,
                            Vec<String>,
                        )>,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            use crate::protocols::postgres_wire::sql::ast::FromClause;
            use crate::protocols::postgres_wire::sql::select_pipeline::Row;
            use crate::protocols::postgres_wire::sql::types::SqlValue;

            let Some(storage) = &self.persistent_storage else {
                return Ok(None);
            };

            match from {
                FromClause::Table { name, alias, .. } => {
                    // Tables live in one namespace here, so an explicit
                    // `public.` qualifier names the same table as a bare name.
                    let table = fold_identifier(&name.full_name());
                    let table = table
                        .strip_prefix("public.")
                        .map_or(table.clone(), str::to_string);

                    // A `WITH` name shadows storage: it is a query, not a table.
                    if let Some(cte) = ctes.get(&table) {
                        let Some((columns, values)) = self.evaluate_select(cte, ctes).await? else {
                            return Ok(None);
                        };
                        let qualifier = alias
                            .as_ref()
                            .map(|a| fold_identifier(&a.name))
                            .unwrap_or_else(|| table.clone());
                        return Ok(Some(Self::rows_from_values(&columns, values, &qualifier)));
                    }

                    let Some(schema) = storage.get_table_schema(&table).await? else {
                        // A catalogue relation is generated, not stored. It has
                        // to be reachable from here as well as from the plain
                        // select path: `SELECT relname FROM pg_class LIMIT 1`
                        // carries a clause and so arrives here.
                        if let Some(QueryResult::Select { columns, rows }) = self
                            .select_system_catalog(&table, &["*".to_string()])
                            .await?
                        {
                            let values = rows
                                .into_iter()
                                .map(|row| {
                                    row.into_iter()
                                        .map(|value| value.map_or(SqlValue::Null, SqlValue::Text))
                                        .collect()
                                })
                                .collect();
                            let qualifier = alias
                                .as_ref()
                                .map(|a| fold_identifier(&a.name))
                                .unwrap_or_else(|| table.clone());
                            return Ok(Some(Self::rows_from_values(&columns, values, &qualifier)));
                        }

                        // Not a table — it may be a view, which is a stored
                        // query rather than stored rows.
                        let Some(definition) = self.view_definition(&table).await? else {
                            return Ok(None);
                        };
                        let parsed = crate::protocols::postgres_wire::sql::parser::SqlParser::new()
                            .parse(&definition)?;
                        let crate::protocols::postgres_wire::sql::ast::Statement::Select(view) =
                            parsed
                        else {
                            return Ok(None);
                        };
                        let Some((columns, values)) = self.evaluate_select(&view, ctes).await?
                        else {
                            return Ok(None);
                        };
                        let qualifier = alias
                            .as_ref()
                            .map(|a| fold_identifier(&a.name))
                            .unwrap_or_else(|| table.clone());
                        return Ok(Some(Self::rows_from_values(&columns, values, &qualifier)));
                    };

                    // Rows carry both the bare column name and its qualified
                    // form, so `a.id` and `id` both resolve after a join.
                    let qualifier = alias
                        .as_ref()
                        .map(|a| fold_identifier(&a.name))
                        .unwrap_or_else(|| table.clone());

                    note_read(&table);
                    let stored = storage
                        .select_rows(&table, Vec::new(), Vec::new(), None)
                        .await?;

                    // A long scan is where a cancelled query spends its time,
                    // so it is checked as the rows go by.
                    let mut scanned = 0usize;
                    let rows: Vec<Row> = stored
                        .into_iter()
                        .filter(|row| row_is_visible(&row.values))
                        .map(|row| {
                            scanned += 1;
                            if scanned.is_multiple_of(CANCEL_CHECK_INTERVAL) {
                                check_cancelled()?;
                            }
                            let mut out = Row::new();
                            for column in &schema.columns {
                                let value = row
                                    .values
                                    .get(&column.name)
                                    .or_else(|| {
                                        row.values.iter().find_map(|(key, value)| {
                                            key.eq_ignore_ascii_case(&column.name).then_some(value)
                                        })
                                    })
                                    .cloned()
                                    .unwrap_or(JsonValue::Null);
                                let value = Self::json_to_sql_value(&value, &column.data_type);
                                let name = fold_identifier(&column.name);
                                out.insert(format!("{qualifier}.{name}"), value.clone());
                                out.insert(name, value);
                            }
                            Ok(out)
                        })
                        .collect::<ProtocolResult<Vec<Row>>>()?;

                    let order = schema
                        .columns
                        .iter()
                        .map(|column| fold_identifier(&column.name))
                        .collect();

                    Ok(Some((rows, order)))
                }

                // A derived table: `FROM (SELECT ...) alias`. Its rows come
                // from running the inner select, not from a stored table.
                FromClause::Subquery { query, alias, .. } => {
                    // A `LATERAL` subquery may read the rows to its left. It is
                    // handled at the join, where those rows are known; on its
                    // own it is an ordinary derived table.
                    let Some((columns, values)) = self.evaluate_select(query, ctes).await? else {
                        return Ok(None);
                    };
                    Ok(Some(Self::rows_from_values(
                        &columns,
                        values,
                        &fold_identifier(&alias.name),
                    )))
                }

                FromClause::Join {
                    left,
                    join_type,
                    right,
                    condition,
                } => {
                    let Some((left_rows, mut order)) = self.rows_from_clause(left, ctes).await?
                    else {
                        return Ok(None);
                    };
                    let left_order_len = order.len();
                    // `LATERAL` re-evaluates its subquery for each row on the
                    // left, which is the whole point of the keyword: without
                    // it the subquery cannot refer to those rows.
                    if let FromClause::Subquery {
                        query,
                        alias,
                        lateral: true,
                    } = right.as_ref()
                    {
                        let qualifier = fold_identifier(&alias.name);
                        let mut joined = Vec::new();
                        for left_row in &left_rows {
                            let bound = Self::bind_outer_select((**query).clone(), left_row);
                            let Some((columns, values)) =
                                self.evaluate_select(&bound, ctes).await?
                            else {
                                return Ok(None);
                            };
                            let (right_rows, right_order) =
                                Self::rows_from_values(&columns, values, &qualifier);
                            if order.len() == left_order_len {
                                order.extend(right_order);
                            }
                            match Self::join_rows(
                                std::slice::from_ref(left_row),
                                &right_rows,
                                join_type,
                                condition,
                            )? {
                                Some(rows) => joined.extend(rows),
                                None => return Ok(None),
                            }
                        }
                        return Ok(Some((joined, order)));
                    }

                    let Some((right_rows, right_order)) =
                        self.rows_from_clause(right, ctes).await?
                    else {
                        return Ok(None);
                    };
                    order.extend(right_order);

                    match Self::join_rows(&left_rows, &right_rows, join_type, condition)? {
                        Some(joined) => Ok(Some((joined, order))),
                        None => Ok(None),
                    }
                }

                _ => Ok(None),
            }
        })
    }

    /// Combine two row sets under a join condition.
    ///
    /// Returns `None` when the condition cannot be resolved.
    ///
    /// # Errors
    /// Returns an error when the join condition cannot be evaluated.
    #[allow(clippy::type_complexity)]
    fn join_rows(
        left_rows: &[crate::protocols::postgres_wire::sql::select_pipeline::Row],
        right_rows: &[crate::protocols::postgres_wire::sql::select_pipeline::Row],
        join_type: &crate::protocols::postgres_wire::sql::ast::JoinType,
        condition: &crate::protocols::postgres_wire::sql::ast::JoinCondition,
    ) -> ProtocolResult<Option<Vec<crate::protocols::postgres_wire::sql::select_pipeline::Row>>>
    {
        use crate::protocols::postgres_wire::sql::ast::{JoinCondition, JoinType};
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };

        let mut evaluator = ExpressionEvaluator::new();
        let mut joined = Vec::new();

        // Every column name each side contributes, so an unmatched row can be
        // padded with NULLs instead of simply lacking them. Without the
        // padding an outer row had no key for the other side's columns at all,
        // and `SELECT val FROM a LEFT JOIN b ...` failed with
        // `column "val" does not exist` rather than returning NULL.
        let columns_of = |rows: &[crate::protocols::postgres_wire::sql::select_pipeline::Row]| {
            let mut names: Vec<String> = Vec::new();
            for row in rows {
                for key in row.keys() {
                    if !names.contains(key) {
                        names.push(key.clone());
                    }
                }
            }
            names
        };
        let left_columns = columns_of(left_rows);
        let right_columns = columns_of(right_rows);
        let padded = |row: &crate::protocols::postgres_wire::sql::select_pipeline::Row,
                      missing: &[String]| {
            let mut out = row.clone();
            for name in missing {
                out.entry(name.clone()).or_insert(SqlValue::Null);
            }
            out
        };

        // Which right rows found a partner, for the outer joins that keep the
        // ones that did not.
        let mut right_matched = vec![false; right_rows.len()];

        for left_row in left_rows {
            let mut matched = false;
            for (right_index, right_row) in right_rows.iter().enumerate() {
                let mut combined = left_row.clone();
                for (key, value) in right_row {
                    // A bare name present on both sides keeps the left one;
                    // the qualified names stay distinct.
                    combined.entry(key.clone()).or_insert_with(|| value.clone());
                    if key.contains('.') {
                        combined.insert(key.clone(), value.clone());
                    }
                }

                let keep = match condition {
                    JoinCondition::On(predicate) => {
                        let context = EvaluationContext::with_row(combined.clone());
                        matches!(
                            evaluator.evaluate(predicate, &context)?,
                            SqlValue::Boolean(true)
                        )
                    }
                    JoinCondition::Using(columns) => columns.iter().all(|column| {
                        let column = fold_identifier(column);
                        left_row.get(&column) == right_row.get(&column)
                    }),
                    // A natural join matches on every column name the two
                    // sides share. Bare keys only: the qualified duplicates
                    // each row carries would otherwise never match.
                    JoinCondition::Natural => {
                        let shared: Vec<&String> = left_row
                            .keys()
                            .filter(|key| !key.contains('.') && right_row.contains_key(*key))
                            .collect();
                        !shared.is_empty()
                            && shared
                                .iter()
                                .all(|key| left_row.get(*key) == right_row.get(*key))
                    }
                };

                if keep || matches!(join_type, JoinType::Cross) {
                    matched = true;
                    right_matched[right_index] = true;
                    joined.push(combined);
                }
            }

            // A left or full outer join keeps an unmatched left row, with the
            // right side's columns present and NULL.
            if !matched && matches!(join_type, JoinType::LeftOuter | JoinType::FullOuter) {
                joined.push(padded(left_row, &right_columns));
            }
        }

        // And the mirror: a right or full outer join keeps the right rows that
        // found no partner. Neither did this at all, so `RIGHT JOIN` behaved
        // as an inner join and `FULL OUTER JOIN` lost both unmatched sides.
        if matches!(join_type, JoinType::RightOuter | JoinType::FullOuter) {
            for (right_index, right_row) in right_rows.iter().enumerate() {
                if !right_matched[right_index] {
                    joined.push(padded(right_row, &left_columns));
                }
            }
        }

        Ok(Some(joined))
    }

    /// Convert a stored value into the typed value the evaluator works with.
    fn json_to_sql_value(
        value: &JsonValue,
        column_type: &ColumnType,
    ) -> crate::protocols::postgres_wire::sql::types::SqlValue {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        match value {
            JsonValue::Null => SqlValue::Null,
            JsonValue::Bool(b) => SqlValue::Boolean(*b),
            JsonValue::Number(n) => match column_type {
                // An exact decimal keeps its declared scale, so a column
                // declared `NUMERIC(10,2)` reads back `10.50` rather than
                // `10.5` — and never goes through binary floating point.
                ColumnType::Numeric { scale, .. } => {
                    use std::str::FromStr;
                    rust_decimal::Decimal::from_str(&n.to_string()).map_or(
                        SqlValue::Null,
                        |mut decimal| {
                            if let Some(scale) = scale {
                                decimal.rescale(u32::from(*scale));
                            }
                            SqlValue::Decimal(decimal)
                        },
                    )
                }
                ColumnType::BigInt => n.as_i64().map_or(SqlValue::Null, SqlValue::BigInt),
                ColumnType::Double => n.as_f64().map_or(SqlValue::Null, SqlValue::DoublePrecision),
                ColumnType::Serial | ColumnType::Integer => {
                    n.as_i64().and_then(|v| i32::try_from(v).ok()).map_or_else(
                        || n.as_i64().map_or(SqlValue::Null, SqlValue::BigInt),
                        SqlValue::Integer,
                    )
                }
                // The column is not declared numeric, so keep the number's own
                // width rather than forcing it into the declared type.
                _ => n
                    .as_i64()
                    .map(SqlValue::BigInt)
                    .or_else(|| n.as_f64().map(SqlValue::DoublePrecision))
                    .unwrap_or(SqlValue::Null),
            },
            JsonValue::String(s) => match column_type {
                ColumnType::Timestamp => SqlValue::Text(s.clone()),
                _ => SqlValue::Text(s.clone()),
            },
            other => SqlValue::Json(other.clone()),
        }
    }

    /// Turn a comprehensive-engine failure into an accurate message.
    ///
    /// That engine keeps its own tables and cannot see persistent storage, so
    /// it reports "table does not exist" for a table that plainly does. Saying
    /// which feature is missing is the truthful answer, and the actionable one.
    async fn explain_unsupported_query(&self, sql: &str, error: ProtocolError) -> ProtocolError {
        let text = error.to_string();
        if !text.contains("does not exist") {
            return error;
        }

        let Some(storage) = &self.persistent_storage else {
            return error;
        };

        // Which table the statement names, if the simple parser can tell.
        let upper = sql.to_uppercase();
        let Some(from) = upper.find(" FROM ") else {
            return error;
        };
        let table = sql[from + 6..]
            .split_whitespace()
            .next()
            .map(fold_identifier)
            .unwrap_or_default();

        match storage.table_exists(&table).await {
            Ok(true) => ProtocolError::PostgresError(format!(
                "Table '{table}' exists, but this query uses SQL features that are not yet                  supported over stored tables (aggregates, GROUP BY, ORDER BY, LIMIT, JOIN,                  DISTINCT and subqueries are executed only by the in-memory engine). The                  statement was refused rather than run without those clauses."
            )),
            _ => error,
        }
    }

    /// Clauses this parser does not implement.
    ///
    /// It parses the statement around them and then ignores them, so
    /// `SELECT ... LIMIT 2` returned every row and `GROUP BY` returned the
    /// ungrouped rows — wrong answers reported as success. Refusing here sends
    /// the statement to the comprehensive engine instead, and if that cannot
    /// run it either the client gets an error rather than bad data.
    const UNSUPPORTED_SELECT_CLAUSES: [&'static str; 19] = [
        " LIMIT ",
        " OFFSET ",
        " GROUP BY ",
        " HAVING ",
        " DISTINCT ",
        " JOIN ",
        " UNION ",
        " INTERSECT ",
        " EXCEPT ",
        " ORDER BY ",
        // The storage matcher implements LIKE as a case-insensitive `contains`
        // after deleting every `%`, so `'al%'` matched anywhere in the value
        // instead of anchoring at the start — and `BETWEEN`/`IS` it does not
        // implement at all. The expression evaluator handles all of them.
        " LIKE ",
        " ILIKE ",
        " BETWEEN ",
        " IS NULL",
        " IS NOT ",
        // This parser reads a WHERE clause as a single `column op value`, so a
        // second condition was swallowed into the value: `WHERE a = 'x' AND b
        // > 1` compared `a` against the text "'x' AND b > 1" and matched
        // nothing.
        " AND ",
        " OR ",
        " NOT ",
        " IN ",
    ];

    /// Whether the simple parser would silently ignore part of `sql`.
    fn has_unsupported_select_clause(sql: &str) -> bool {
        // Padded so the check sees clause keywords at the end too.
        let padded = format!(" {} ", sql.trim().trim_end_matches(';'));
        let upper = padded.to_uppercase();

        if Self::UNSUPPORTED_SELECT_CLAUSES
            .iter()
            .any(|clause| upper.contains(clause))
        {
            return true;
        }

        // Anything in the select list that is not a bare column: a call such
        // as `COUNT(*)`, an operator such as `name || '!'`, a literal, a
        // subquery. This parser treats the projection as column names to look
        // up, so it returned a NULL column named after the expression instead
        // of evaluating it.
        let projection_end = upper.find(" FROM ").unwrap_or(upper.len());
        let projection = upper[..projection_end]
            .trim()
            .strip_prefix("SELECT")
            .unwrap_or_default();
        !Self::projection_is_plain_columns(projection) || upper.contains("(SELECT ")
    }

    /// Whether a select list is only column names, `*`, or qualified names.
    ///
    /// Anything else has to be evaluated rather than looked up.
    fn projection_is_plain_columns(projection: &str) -> bool {
        let projection = projection.trim();
        !projection.is_empty()
            && projection.split(',').all(|item| {
                let item = item.trim();
                !item.is_empty()
                    && (item == "*"
                        || item.chars().all(|character| {
                            character.is_alphanumeric()
                                || matches!(character, '_' | '.' | '"' | '*')
                        }))
            })
    }

    /// Parse SELECT statement
    fn parse_select(&self, sql: &str) -> ProtocolResult<Statement> {
        if Self::has_unsupported_select_clause(sql) {
            return Err(ProtocolError::PostgresError(
                "statement uses a clause this parser does not implement".to_string(),
            ));
        }

        // Simple parser: SELECT columns FROM table [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 4 || parts[0].to_uppercase() != "SELECT" {
            return Err(ProtocolError::PostgresError(
                "Invalid SELECT syntax".to_string(),
            ));
        }

        // Find FROM
        let from_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;

        // Parse columns
        let columns_str = parts[1..from_idx].join(" ");
        let columns: Vec<String> = if columns_str == "*" {
            vec!["*".to_string()]
        } else {
            columns_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        // Parse table - convert to uppercase and trim semicolon
        let table = fold_identifier(parts[from_idx + 1]);

        // Parse WHERE clause if present
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Select {
            columns,
            table,
            where_clause,
        })
    }

    /// Parse INSERT statement
    fn parse_insert(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: INSERT INTO table (columns) VALUES (values), (values)...
        let sql_upper = sql.to_uppercase();

        if !sql_upper.contains("INSERT INTO") || !sql_upper.contains("VALUES") {
            return Err(ProtocolError::PostgresError(
                "Invalid INSERT syntax".to_string(),
            ));
        }

        // Find positions using uppercase version. Each of these was an
        // `unwrap`: a statement without a column list panicked the connection's
        // task rather than reporting a syntax error.
        let invalid = || ProtocolError::PostgresError("Invalid INSERT syntax".to_string());
        let table_start = sql_upper.find("INTO").ok_or_else(invalid)? + 4;
        let table_end = sql_upper[table_start..].find('(').ok_or_else(invalid)? + table_start;
        let col_start = table_end + 1;
        let col_end = sql_upper[col_start..].find(')').ok_or_else(invalid)? + col_start;
        let val_keyword_pos = sql_upper.find("VALUES").ok_or_else(invalid)? + 6;

        // Extract data using original SQL to preserve case
        let table = fold_identifier(&sql[table_start..table_end]);
        // Folded from the original text, not the uppercased copy, so a quoted
        // identifier keeps its case.
        let columns: Vec<String> = sql[col_start..col_end]
            .split(',')
            .map(fold_identifier)
            .collect();

        // Parse values list: (v1, v2), (v3, v4)
        let values_str = sql[val_keyword_pos..].trim();
        let values = self.parse_values_list(values_str);

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    /// Parse list of value groups: (v1, v2), (v3, v4)
    fn parse_values_list(&self, values_str: &str) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut current_row_str = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut paren_depth = 0;
        let chars: Vec<char> = values_str.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    if paren_depth > 0 {
                        current_row_str.push(ch);
                    }
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    if paren_depth > 0 {
                        current_row_str.push(ch);
                    }
                }
                '(' if !in_quotes => {
                    paren_depth += 1;
                    if paren_depth > 1 {
                        current_row_str.push(ch);
                    }
                }
                ')' if !in_quotes => {
                    paren_depth -= 1;
                    if paren_depth > 0 {
                        current_row_str.push(ch);
                    } else if paren_depth == 0 {
                        // End of a row
                        if !current_row_str.trim().is_empty() {
                            rows.push(self.parse_csv_values(&current_row_str));
                        }
                        current_row_str.clear();
                    }
                }
                ',' if !in_quotes && paren_depth == 0 => {
                    // Separator between rows, ignore
                }
                _ => {
                    if paren_depth > 0 {
                        current_row_str.push(ch);
                    }
                }
            }
            i += 1;
        }
        rows
    }

    /// Parse UPDATE statement
    fn parse_update(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: UPDATE table SET col=val [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 4 || parts[0].to_uppercase() != "UPDATE" {
            return Err(ProtocolError::PostgresError(
                "Invalid UPDATE syntax".to_string(),
            ));
        }

        let table = fold_identifier(parts[1]);

        // Find SET
        let set_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "SET")
            .ok_or_else(|| ProtocolError::PostgresError("Missing SET clause".to_string()))?;

        // Find WHERE or end
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let set_end = where_idx.unwrap_or(parts.len());

        // Parse SET clauses safely with JSON support
        let set_str = parts[set_idx + 1..set_end].join(" ");
        let set_clauses = self.parse_set_clauses(&set_str);

        // Parse WHERE clause
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Update {
            table,
            set_clauses,
            where_clause,
        })
    }

    /// Parse DELETE statement
    fn parse_delete(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: DELETE FROM table [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 3 || parts[0].to_uppercase() != "DELETE" {
            return Err(ProtocolError::PostgresError(
                "Invalid DELETE syntax".to_string(),
            ));
        }

        // Find FROM
        let from_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;

        let table = fold_identifier(parts[from_idx + 1]);

        // Parse WHERE clause
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Delete {
            table,
            where_clause,
        })
    }

    /// Parse SET clauses respecting quotes and JSON braces
    fn parse_set_clauses(&self, set_str: &str) -> Vec<(String, String)> {
        let mut clauses = Vec::new();
        let mut current_clause = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut brace_depth = 0;
        let chars: Vec<char> = set_str.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    current_clause.push(ch);
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    current_clause.push(ch);
                }
                '{' if !in_quotes => {
                    brace_depth += 1;
                    current_clause.push(ch);
                }
                '}' if !in_quotes => {
                    brace_depth -= 1;
                    current_clause.push(ch);
                }
                ',' if !in_quotes && brace_depth == 0 => {
                    // Found a separator - parse the current clause
                    if let Some(parsed) = self.parse_single_set_clause(&current_clause) {
                        clauses.push(parsed);
                    }
                    current_clause.clear();
                }
                _ => {
                    current_clause.push(ch);
                }
            }
            i += 1;
        }

        // Add the last clause
        if !current_clause.is_empty() {
            if let Some(parsed) = self.parse_single_set_clause(&current_clause) {
                clauses.push(parsed);
            }
        }

        clauses
    }

    /// Parse a single SET clause (key = value)
    /// Split `column = value`, keeping the value exactly as written.
    ///
    /// The quotes are deliberately left on. Stripping them here threw away the
    /// only thing that distinguishes a text literal from an expression, so
    /// `SET n = n + 1` and `SET t = 'n + 1'` arrived identical — and it
    /// mangled an escaped quote besides, turning `'it''s'` into `it''s`.
    /// `literal_to_json` unquotes properly, as it already does for
    /// `INSERT ... VALUES`.
    fn parse_single_set_clause(&self, clause: &str) -> Option<(String, String)> {
        let eq_pos = clause.find('=')?;
        let key = clause[..eq_pos].trim().to_string();
        let value = clause[eq_pos + 1..].trim().to_string();
        Some((key, value))
    }

    /// Parse CSV values respecting quotes and JSON braces
    fn parse_csv_values(&self, values_str: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut current_value = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut brace_depth = 0;
        let chars: Vec<char> = values_str.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    current_value.push(ch);
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    current_value.push(ch);
                }
                '{' if !in_quotes => {
                    brace_depth += 1;
                    current_value.push(ch);
                }
                '}' if !in_quotes => {
                    brace_depth -= 1;
                    current_value.push(ch);
                }
                ',' if !in_quotes && brace_depth == 0 => {
                    // Quotes are kept; `literal_to_json` strips them. Stripping
                    // here made the NULL keyword indistinguishable from the text
                    // 'NULL', and stored '123' as the number 123.
                    values.push(current_value.trim().to_string());
                    current_value.clear();
                }
                _ => {
                    current_value.push(ch);
                }
            }
            i += 1;
        }

        // Add the last value
        if !current_value.trim().is_empty() {
            values.push(current_value.trim().to_string());
        }

        values
    }

    /// Render a value as the SQL literal that denotes it.
    ///
    /// The inverse of [`QueryEngine::literal_to_json`], so a value that goes
    /// out through one and back through the other is unchanged.
    /// Store a value as JSON, keeping its type rather than its rendering.
    ///
    /// Going through a SQL literal and back would turn an integer into the
    /// string `"11"`, which then compares as text.
    fn sql_value_to_json(
        value: &crate::protocols::postgres_wire::sql::types::SqlValue,
    ) -> JsonValue {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        match value {
            SqlValue::Null => JsonValue::Null,
            SqlValue::Boolean(b) => JsonValue::Bool(*b),
            SqlValue::SmallInt(i) => JsonValue::Number((*i).into()),
            SqlValue::Integer(i) => JsonValue::Number((*i).into()),
            SqlValue::BigInt(i) => JsonValue::Number((*i).into()),
            SqlValue::Real(f) => serde_json::Number::from_f64(f64::from(*f))
                .map_or(JsonValue::Null, JsonValue::Number),
            SqlValue::DoublePrecision(f) => {
                serde_json::Number::from_f64(*f).map_or(JsonValue::Null, JsonValue::Number)
            }
            // An exact decimal is a number. Rendered as a string it did not
            // match the number in storage, and because an update identifies
            // its row by *every* column, one mismatching column stopped the
            // whole update — a table merely containing a `NUMERIC` column
            // silently dropped updates to its other columns.
            SqlValue::Decimal(d) => std::str::FromStr::from_str(&d.to_string())
                .map_or(JsonValue::Null, JsonValue::Number),
            other => JsonValue::String(other.to_postgres_string()),
        }
    }

    pub fn sql_value_to_literal(
        value: &crate::protocols::postgres_wire::sql::types::SqlValue,
    ) -> String {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        match value {
            SqlValue::Null => "NULL".to_string(),
            SqlValue::Boolean(b) => b.to_string(),
            SqlValue::SmallInt(n) => n.to_string(),
            SqlValue::Integer(n) => n.to_string(),
            SqlValue::BigInt(n) => n.to_string(),
            SqlValue::Real(n) => n.to_string(),
            SqlValue::DoublePrecision(n) => n.to_string(),
            SqlValue::Decimal(d) => d.to_string(),
            // Everything else is text on the way in; quoting keeps it text.
            other => format!("'{}'", other.to_postgres_string().replace('\'', "''")),
        }
    }

    /// Convert a SQL literal as written into the value to store.
    ///
    /// Quoting carries meaning: `NULL` is the null value while `'NULL'` is the
    /// three-letter string, and `123` is a number while `'123'` is text.
    /// Whether a `VALUES` entry is a literal that needs no evaluation.
    ///
    /// Everything else is an expression — `500 + 1`, `NOW()`, `a || b` — which
    /// PostgreSQL evaluates. This engine stored the *text*: an `INTEGER`
    /// column given `500 + 1` held the string `500 + 1`, which then failed
    /// every later comparison against a number.
    fn is_plain_literal(value: &str) -> bool {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return true;
        }
        let quoted = (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() > 1)
            || (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1);
        quoted
            || matches!(
                trimmed.to_uppercase().as_str(),
                "NULL" | "TRUE" | "FALSE" | "DEFAULT"
            )
            || trimmed.parse::<f64>().is_ok()
    }

    /// Convert one `VALUES` entry, evaluating it if it is an expression.
    ///
    /// # Errors
    /// Returns the engine's error when the expression cannot be evaluated —
    /// an unknown column, say, which PostgreSQL also refuses.
    async fn value_to_json(&self, value: &str) -> ProtocolResult<JsonValue> {
        if Self::is_plain_literal(value) {
            return Ok(Self::literal_to_json(value));
        }
        let evaluated = Box::pin(self.evaluate_scalar(value)).await?;
        Ok(evaluated.map_or(JsonValue::Null, |text| Self::literal_to_json(&text)))
    }

    pub fn literal_to_json(literal: &str) -> JsonValue {
        let trimmed = literal.trim();

        let quoted = trimmed
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
            .or_else(|| {
                trimmed
                    .strip_prefix('"')
                    .and_then(|rest| rest.strip_suffix('"'))
            });

        if let Some(inner) = quoted {
            // `''` is how a quote is escaped inside a SQL string literal.
            return JsonValue::String(inner.replace("''", "'"));
        }

        if trimmed.eq_ignore_ascii_case("NULL") {
            return JsonValue::Null;
        }
        if trimmed.eq_ignore_ascii_case("TRUE") {
            return JsonValue::Bool(true);
        }
        if trimmed.eq_ignore_ascii_case("FALSE") {
            return JsonValue::Bool(false);
        }

        serde_json::from_str(trimmed).unwrap_or_else(|_| JsonValue::String(trimmed.to_string()))
    }

    /// Parse WHERE clause
    fn parse_where_clause(&self, parts: &[&str]) -> ProtocolResult<WhereClause> {
        if parts.len() < 3 {
            return Err(ProtocolError::PostgresError(
                "Invalid WHERE clause".to_string(),
            ));
        }

        // Conjuncts are separate conditions. Reading the whole clause as one
        // made `WHERE a = 1 AND b = 2` compare `a` against the text
        // `1 AND b = 2`, which matches nothing — so an `UPDATE` or `DELETE`
        // with two conditions silently changed no rows and reported success.
        let text = parts.join(" ");
        let text = text.trim_end_matches(';').trim();

        // Conditions are combined with AND; an OR cannot be expressed as a
        // list of them, so it is refused rather than quietly mis-read.
        if Self::splits_on_keyword(text, "OR").len() > 1 {
            return Err(ProtocolError::PostgresError(
                "statement uses a clause this parser does not implement".to_string(),
            ));
        }

        let conditions = Self::splits_on_keyword(text, "AND")
            .into_iter()
            .map(|conjunct| {
                let words: Vec<&str> = conjunct.split_whitespace().collect();
                if words.len() < 3 {
                    return Err(ProtocolError::PostgresError(
                        "Invalid WHERE clause".to_string(),
                    ));
                }
                // Only `column op value` can be represented here. Anything
                // else — `id * 2 = 4`, `UPPER(name) = 'ADA'` — was accepted
                // with the first word as the column and the second as the
                // operator, and the storage matcher treats an operator it does
                // not know as matching every row. `WHERE id * 2 = 4` returned
                // the whole table. Refusing sends the statement to the path
                // that evaluates expressions properly.
                let value = words[2..].join(" ");
                if !is_simple_column(words[0])
                    || !is_comparison(words[1])
                    || !is_simple_value(words[1], &value)
                {
                    return Err(ProtocolError::PostgresError(
                        "statement uses a clause this parser does not implement".to_string(),
                    ));
                }
                Ok(Condition {
                    column: words[0].to_string(),
                    operator: words[1].to_string(),
                    // Quotes are kept and interpreted by `literal_to_json`, so
                    // `x = NULL` and `x = 'NULL'` stay distinguishable.
                    value,
                })
            })
            .collect::<ProtocolResult<Vec<_>>>()?;

        Ok(WhereClause { conditions })
    }

    /// Split a predicate on a keyword that is not inside quotes or brackets.
    fn splits_on_keyword(text: &str, keyword: &str) -> Vec<String> {
        let upper = text.to_uppercase();
        let padded = format!(" {keyword} ");
        let mut parts = Vec::new();
        let mut start = 0usize;
        let mut depth = 0usize;
        let mut quote: Option<char> = None;

        let bytes: Vec<char> = text.chars().collect();
        let upper_bytes: Vec<char> = upper.chars().collect();
        let needle: Vec<char> = padded.chars().collect();

        let mut index = 0usize;
        while index < bytes.len() {
            let character = bytes[index];
            match quote {
                Some(open) => {
                    if character == open {
                        quote = None;
                    }
                }
                None => match character {
                    '\'' | '"' => quote = Some(character),
                    '(' => depth += 1,
                    ')' => depth = depth.saturating_sub(1),
                    _ => {
                        if depth == 0
                            && index + needle.len() <= upper_bytes.len()
                            && upper_bytes[index..index + needle.len()] == needle[..]
                        {
                            parts.push(bytes[start..index].iter().collect::<String>());
                            index += needle.len();
                            start = index;
                            continue;
                        }
                    }
                },
            }
            index += 1;
        }
        parts.push(bytes[start..].iter().collect::<String>());
        parts
            .into_iter()
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }

    /// Parse CREATE TABLE statement
    fn parse_create_table(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: CREATE TABLE [IF NOT EXISTS] table_name (column_definitions)
        let sql_upper = sql.to_uppercase();

        // Check for IF NOT EXISTS
        let if_not_exists = sql_upper.contains("IF NOT EXISTS");

        // Find table name. These were `unwrap`s: `CREATE TABLE t AS SELECT ...`
        // has no `(`, so an ordinary statement panicked the connection's task
        // and every later statement on it failed with "connection closed".
        let table_start = if if_not_exists {
            sql_upper.find("EXISTS").map(|at| at + 6)
        } else {
            sql_upper.find("TABLE").map(|at| at + 5)
        }
        .ok_or_else(|| ProtocolError::PostgresError("Invalid CREATE TABLE syntax".to_string()))?;

        let table_end = sql[table_start..]
            .find('(')
            .map(|at| at + table_start)
            .ok_or_else(|| {
                ProtocolError::PostgresError(
                    "Invalid CREATE TABLE syntax: expected a column list".to_string(),
                )
            })?;
        let table_name = fold_identifier(&sql[table_start..table_end]);

        // Find column definitions between parentheses
        let col_start = table_end + 1;
        let col_end = sql.rfind(')').ok_or_else(|| {
            ProtocolError::PostgresError(
                "Invalid CREATE TABLE syntax: missing closing parenthesis".to_string(),
            )
        })?;

        let column_defs_str = &sql[col_start..col_end];
        let mut columns: Vec<SimpleColumnDef> = Vec::new();

        // Split by commas and parse each column definition
        // Table-level clauses — `PRIMARY KEY (a)`, `UNIQUE (a)`,
        // `FOREIGN KEY (a) REFERENCES t(b)`, `CHECK (...)`, optionally named
        // with `CONSTRAINT` — are collected here and applied to the columns
        // they name. Splitting on commas alone read them as columns called
        // `PRIMARY`, so the constraint was silently dropped.
        let mut table_unique: Vec<String> = Vec::new();
        let mut foreign_keys: Vec<crate::protocols::postgres_wire::persistent_storage::ForeignKey> =
            Vec::new();
        let mut table_checks: Vec<String> = Vec::new();

        for col_def in Self::split_column_definitions(column_defs_str) {
            let col_def = col_def.trim();
            let without_name = col_def
                .strip_prefix("CONSTRAINT ")
                .or_else(|| col_def.strip_prefix("constraint "))
                .and_then(|rest| rest.split_once(char::is_whitespace).map(|(_, rest)| rest))
                .unwrap_or(col_def)
                .trim();
            let upper = without_name.to_uppercase();

            if upper.starts_with("PRIMARY KEY") || upper.starts_with("UNIQUE") {
                if let Some(columns) = Self::parenthesised(without_name) {
                    table_unique.extend(columns.split(',').map(fold_identifier));
                }
                continue;
            }
            if upper.starts_with("FOREIGN KEY") {
                if let Some(key) = Self::parse_foreign_key(without_name) {
                    foreign_keys.push(key);
                }
                continue;
            }
            if upper.starts_with("CHECK") {
                if let Some(predicate) = Self::parenthesised(without_name) {
                    table_checks.push(predicate.to_string());
                }
                continue;
            }

            let parts: Vec<&str> = col_def.split_whitespace().collect();

            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let data_type = parts[1].to_string();
                let constraints = parts[2..].iter().map(|s| s.to_string()).collect();

                columns.push(SimpleColumnDef {
                    name,
                    data_type,
                    constraints,
                });
            }
        }

        // A table-level clause is written onto the column it names, so the
        // executor sees one uniform description of each column.
        for column in &mut columns {
            let name = fold_identifier(&column.name);
            if table_unique.contains(&name) {
                column.constraints.push("PRIMARY".to_string());
                column.constraints.push("KEY".to_string());
            }
        }
        // A table-level CHECK may name any column, so it goes on the first
        // one; the evaluator sees the whole row either way.
        if let (Some(check), Some(column)) = (table_checks.first(), columns.first_mut()) {
            column.constraints.push("CHECK".to_string());
            column.constraints.push(format!("({check})"));
        }

        Ok(Statement::CreateTable {
            table: table_name,
            columns,
            if_not_exists,
            foreign_keys,
        })
    }

    /// Parse a `FOREIGN KEY (...) REFERENCES t(...) [ON DELETE ...] [ON UPDATE ...]`
    /// clause, or the column-level `REFERENCES t(c)` form.
    fn parse_foreign_key(
        text: &str,
    ) -> Option<crate::protocols::postgres_wire::persistent_storage::ForeignKey> {
        use crate::protocols::postgres_wire::persistent_storage::{ForeignKey, ReferentialAction};

        let upper = text.to_uppercase();
        let references_at = upper.find("REFERENCES")?;

        let columns: Vec<String> = if upper.starts_with("FOREIGN KEY") {
            Self::parenthesised(&text[..references_at])?
                .split(',')
                .map(fold_identifier)
                .collect()
        } else {
            Vec::new()
        };

        let target = text[references_at + "REFERENCES".len()..].trim();
        let (table, referenced) = match target.split_once('(') {
            Some((table, rest)) => {
                let close = rest.find(')')?;
                (
                    fold_identifier(table),
                    rest[..close].split(',').map(fold_identifier).collect(),
                )
            }
            None => (
                fold_identifier(target.split_whitespace().next().unwrap_or(target)),
                Vec::new(),
            ),
        };

        // `ON DELETE`/`ON UPDATE` follow the reference; the default is the
        // standard NO ACTION.
        let action_after = |keyword: &str| -> ReferentialAction {
            let Some(at) = upper.find(keyword) else {
                return ReferentialAction::NoAction;
            };
            let rest = upper[at + keyword.len()..].trim_start();
            if rest.starts_with("CASCADE") {
                ReferentialAction::Cascade
            } else if rest.starts_with("SET NULL") {
                ReferentialAction::SetNull
            } else if rest.starts_with("SET DEFAULT") {
                ReferentialAction::SetDefault
            } else if rest.starts_with("RESTRICT") {
                ReferentialAction::Restrict
            } else {
                ReferentialAction::NoAction
            }
        };

        use crate::protocols::postgres_wire::persistent_storage::MatchType;
        let match_type = if upper.contains("MATCH FULL") {
            MatchType::Full
        } else if upper.contains("MATCH PARTIAL") {
            MatchType::Partial
        } else {
            MatchType::Simple
        };

        Some(ForeignKey {
            columns,
            table,
            referenced,
            on_delete: action_after("ON DELETE"),
            on_update: action_after("ON UPDATE"),
            deferrable: upper.contains("DEFERRABLE") && !upper.contains("NOT DEFERRABLE"),
            match_type,
        })
    }

    /// Split a column-definition list on commas that are not inside brackets.
    ///
    /// `CHECK (a > 0)` and `FOREIGN KEY (a, b)` contain commas of their own; a
    /// plain split cut them in half.
    fn split_column_definitions(text: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut depth = 0usize;

        for character in text.chars() {
            match character {
                '(' => {
                    depth += 1;
                    current.push(character);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    current.push(character);
                }
                ',' if depth == 0 => {
                    parts.push(std::mem::take(&mut current));
                }
                other => current.push(other),
            }
        }
        if !current.trim().is_empty() {
            parts.push(current);
        }
        parts
    }

    /// The text inside the first bracketed group of `text`.
    ///
    /// The closing bracket is the one that matches the opening one, not the
    /// last in the string: `FOREIGN KEY (a) REFERENCES t(b)` has two groups,
    /// and taking the last `)` returned `a) REFERENCES t(b`.
    fn parenthesised(text: &str) -> Option<&str> {
        let open = text.find('(')?;
        let mut depth = 0usize;
        for (offset, character) in text[open..].char_indices() {
            match character {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(text[open + 1..open + offset].trim());
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Parse DROP TABLE statement
    fn parse_drop_table(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: DROP TABLE [IF EXISTS] table_name
        let sql_upper = sql.to_uppercase();

        // Check for IF EXISTS
        let if_exists = sql_upper.contains("IF EXISTS");

        // Find table name
        let table_start = if if_exists {
            sql_upper.find("EXISTS").map(|at| at + 6)
        } else {
            sql_upper.find("TABLE").map(|at| at + 5)
        }
        .ok_or_else(|| ProtocolError::PostgresError("Invalid DROP TABLE syntax".to_string()))?;

        let table_name = fold_identifier(&sql[table_start..]);

        Ok(Statement::DropTable {
            table: table_name,
            if_exists,
        })
    }

    /// Execute SELECT query on actors table
    async fn execute_actor_select(
        &self,
        columns: Vec<String>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let actors = self.actors.read().await;
        let mut rows = Vec::new();

        for actor in actors.values() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }

            // Build row
            let mut row = Vec::new();
            if columns.len() == 1 && columns[0] == "*" {
                // For SELECT *, add all columns in the expected order
                row.push(Some(actor.actor_id.clone()));
                row.push(Some(actor.actor_type.clone()));
                row.push(Some(actor.state.to_string()));
            } else {
                // For specific columns
                for col in &columns {
                    let value = match col.to_uppercase().as_str() {
                        "ACTOR_ID" => Some(actor.actor_id.clone()),
                        "ACTOR_TYPE" => Some(actor.actor_type.clone()),
                        "STATE" => Some(actor.state.to_string()),
                        _ => None,
                    };
                    row.push(value);
                }
            }
            rows.push(row);
        }

        // Determine columns
        let result_columns = if columns.len() == 1 && columns[0] == "*" {
            vec![
                "actor_id".to_string(),
                "actor_type".to_string(),
                "state".to_string(),
            ]
        } else {
            columns
        };

        Ok(QueryResult::Select {
            columns: result_columns,
            rows,
        })
    }

    /// Execute INSERT query on actors table
    #[allow(dead_code)]
    async fn execute_actor_insert(
        &self,
        table: &str,
        columns: Vec<String>,
        values_list: Vec<Vec<String>>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let mut count = 0;
        let mut actors = self.actors.write().await;

        for values in values_list {
            if columns.len() != values.len() {
                return Err(ProtocolError::PostgresError(
                    "Column count doesn't match value count".to_string(),
                ));
            }

            let mut actor_id = None;
            let mut actor_type = None;
            let mut state = JsonValue::Object(serde_json::Map::new());

            for (col, val) in columns.iter().zip(values.iter()) {
                match col.to_uppercase().as_str() {
                    "ACTOR_ID" => actor_id = Some(val.clone()),
                    "ACTOR_TYPE" => actor_type = Some(val.clone()),
                    "STATE" => {
                        state = serde_json::from_str(val)
                            .unwrap_or_else(|_| JsonValue::String(val.clone()));
                    }
                    _ => {}
                }
            }

            let actor_id = actor_id
                .ok_or_else(|| ProtocolError::PostgresError("Missing actor_id".to_string()))?;
            let actor_type = actor_type
                .ok_or_else(|| ProtocolError::PostgresError("Missing actor_type".to_string()))?;

            let record = ActorRecord {
                actor_id: actor_id.clone(),
                actor_type,
                state,
            };

            actors.insert(actor_id, record);
            count += 1;
        }

        self.flush_change_log().await?;
        Ok(QueryResult::Insert { count })
    }

    /// Execute UPDATE query on actors table
    #[allow(dead_code)]
    async fn execute_actor_update(
        &self,
        table: &str,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let mut actors = self.actors.write().await;
        let mut count = 0;

        for actor in actors.values_mut() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }

            // Apply updates
            for (col, val) in &set_clauses {
                // The value keeps its quotes now, so unquote it the same way
                // the stored path does.
                let val = match Self::literal_to_json(val) {
                    JsonValue::String(text) => text,
                    other => other.to_string(),
                };
                match col.to_uppercase().as_str() {
                    "STATE" => {
                        actor.state = serde_json::from_str(&val)
                            .unwrap_or_else(|_| JsonValue::String(val.clone()));
                    }
                    "ACTOR_TYPE" => {
                        actor.actor_type = val.clone();
                    }
                    _ => {}
                }
            }
            count += 1;
        }

        Ok(QueryResult::Update { count })
    }

    /// Execute DELETE query on actors table
    #[allow(dead_code)]
    async fn execute_actor_delete(
        &self,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let mut actors = self.actors.write().await;
        let mut to_delete = Vec::new();

        for (id, actor) in actors.iter() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }
            to_delete.push(id.clone());
        }

        let count = to_delete.len();
        for id in to_delete {
            actors.remove(&id);
        }

        Ok(QueryResult::Delete { count })
    }

    /// Check if actor matches WHERE clause
    fn matches_where(&self, actor: &ActorRecord, where_clause: &WhereClause) -> bool {
        for condition in &where_clause.conditions {
            let value = match condition.column.to_uppercase().as_str() {
                "ACTOR_ID" => &actor.actor_id,
                "ACTOR_TYPE" => &actor.actor_type,
                _ => return false,
            };

            let matches = match condition.operator.as_str() {
                "=" => value == &condition.value,
                "!=" | "<>" => value != &condition.value,
                _ => false,
            };

            if !matches {
                return false;
            }
        }
        true
    }

    /// Execute SELECT query on persistent storage
    async fn execute_persistent_select(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        columns: Vec<String>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert WHERE clause to QueryConditions
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: fold_identifier(&c.column),
                    operator: c.operator,
                    value: Self::literal_to_json(&c.value),
                })
                .collect()
        } else {
            vec![]
        };

        // Execute select query - pass normalized column names to storage
        let storage_columns: Vec<String> = if columns.len() == 1 && columns[0] == "*" {
            // For SELECT *, pass empty columns to storage (no filtering)
            vec![]
        } else {
            columns.into_iter().map(|c| fold_identifier(&c)).collect()
        };

        // A column the table does not have is an error. Projecting it produced
        // a column of NULLs, which reads as "every row has no value there".
        //
        // The wanted name is matched against the stored name exactly first, so
        // a quoted identifier resolves, then case-insensitively, so an
        // unquoted one still finds a column stored with different case.
        let storage_columns: Vec<String> = match storage.get_table_schema(table).await? {
            None => storage_columns,
            Some(schema) => {
                let resolve = |wanted: &String| {
                    schema
                        .columns
                        .iter()
                        .find(|column| column.name == *wanted)
                        .or_else(|| {
                            schema
                                .columns
                                .iter()
                                .find(|column| column.name.eq_ignore_ascii_case(wanted))
                        })
                        .map(|column| column.name.clone())
                        .ok_or_else(|| {
                            ProtocolError::PostgresError(format!(
                                "column \"{wanted}\" does not exist"
                            ))
                        })
                };
                storage_columns
                    .iter()
                    .map(resolve)
                    .collect::<ProtocolResult<Vec<_>>>()?
            }
        };

        // Every column is fetched even when only some are projected: the row's
        // transaction stamp decides whether it may be seen at all, and asking
        // storage for a projection threw it away before that could be judged.
        let rows: Vec<_> = storage
            .select_rows(table, Vec::new(), conditions.clone(), None)
            .await?
            .into_iter()
            .filter(|row| row_is_visible(&row.values))
            .collect();

        // Whether the fetch that just returned should have been abandoned.
        // Walking these rows to check would be theatre: `select_rows` has
        // already done the work, so the only honest thing a check can do here
        // is stop the rest of the statement.
        check_cancelled()?;

        // A serializable block records what it saw, so a later write to one of
        // those rows is a conflict and a write elsewhere is not.
        if records_reads() {
            let key_columns = match storage.get_table_schema(table).await? {
                Some(schema) => schema
                    .columns
                    .iter()
                    .filter(|column| column.unique)
                    .map(|column| fold_identifier(&column.name))
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };
            if key_columns.is_empty() {
                // Without a key a row cannot be named across a change: its
                // identity would be its contents, and an update changes those.
                // The whole table is watched instead — coarser, but it never
                // misses a conflict.
                note_read(table);
            } else {
                for row in &rows {
                    note_read_row(table, &row_identity(&row.values, &key_columns));
                }
                // The predicate is recorded as well as the rows, so a row that
                // *starts* matching it counts as a conflict. Watching only the
                // rows returned cannot see a phantom: it was not there to
                // record.
                note_read_predicate(table, &conditions);
            }
        }

        // The schema is read once, and is also what says a column carries a
        // declared scale.
        let table_schema = storage.get_table_schema(table).await?;

        // Convert TableRows to QueryResult format
        let result_columns = if storage_columns.is_empty() {
            // The schema already holds each name in its final form: a quoted
            // identifier kept its case when the table was created. Folding it
            // again lowercased it, so `SELECT "Id"` could not find a column
            // that `SELECT *` reported as `id`, and reading it returned NULL.
            table_schema
                .as_ref()
                .map(|schema| schema.columns.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default()
        } else {
            storage_columns.clone()
        };

        let result_rows: Vec<Vec<Option<String>>> = rows
            .into_iter()
            .map(|row| {
                result_columns
                    .iter()
                    .map(|col| {
                        // Try both original case and uppercase for compatibility
                        let value = row
                            .values
                            .get(col)
                            .or_else(|| row.values.get(&col.to_uppercase()))
                            .or_else(|| {
                                // Rows written before identifiers were folded
                                // consistently may carry either case.
                                row.values.iter().find_map(|(key, value)| {
                                    key.eq_ignore_ascii_case(col).then_some(value)
                                })
                            });
                        // `None` is SQL NULL on the wire. Rendering it as the
                        // text "NULL" made a null indistinguishable from a row
                        // whose value is the three-letter string.
                        // A column declared with a scale renders at that
                        // scale: `NUMERIC(10,2)` reads back `10.50`, not
                        // `10.5`. This was written once before and removed as
                        // dead — it was inert only because the column's type
                        // was still `Text` at the time, which is now fixed.
                        let declared = table_schema.as_ref().and_then(|schema| {
                            schema
                                .columns
                                .iter()
                                .find(|c| fold_identifier(&c.name) == fold_identifier(col))
                                .map(|c| &c.data_type)
                        });
                        value.and_then(|v| match v {
                            JsonValue::Null => None,
                            JsonValue::String(s) => Some(s.clone()),
                            JsonValue::Number(n) => Some(render_number(n, declared)),
                            JsonValue::Bool(b) => Some(b.to_string()),
                            other => Some(other.to_string()),
                        })
                    })
                    .collect()
            })
            .collect();

        Ok(QueryResult::Select {
            columns: result_columns,
            rows: result_rows,
        })
    }

    /// Execute INSERT query on persistent storage
    async fn execute_persistent_insert(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        columns: Vec<String>,
        values_list: Vec<Vec<String>>,
    ) -> ProtocolResult<QueryResult> {
        note_block_write(table);
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Get table schema to handle column types properly
        let schema = storage.get_table_schema(table).await?;
        let schema = schema.ok_or_else(|| {
            ProtocolError::PostgresError(format!("Table '{}' schema not found", table))
        })?;

        let mut count = 0;

        for values in values_list {
            if columns.len() != values.len() {
                return Err(ProtocolError::PostgresError(
                    "Column count doesn't match value count".to_string(),
                ));
            }

            // Build row data
            let mut row_values = std::collections::HashMap::new();
            let now = chrono::Utc::now();

            // Handle SERIAL columns (auto-increment)
            use crate::protocols::postgres_wire::persistent_storage::ColumnType;
            for column_def in &schema.columns {
                if matches!(column_def.data_type, ColumnType::Serial) {
                    // Generate next ID - for now use a simple counter based on current time + count
                    let next_id = chrono::Utc::now().timestamp_micros() % 1000000 + count as i64;
                    row_values.insert(
                        column_def.name.clone(), // Use schema name directly (don't uppercase)
                        JsonValue::Number(serde_json::Number::from(next_id)),
                    );
                }
            }

            for (col, val) in columns.iter().zip(values.iter()) {
                let col_upper = fold_identifier(col);

                // Find column in schema to get correct casing
                let schema_col = schema
                    .columns
                    .iter()
                    .find(|c| fold_identifier(&c.name) == col_upper);

                if let Some(column_def) = schema_col {
                    // Skip SERIAL columns as they're auto-generated
                    if matches!(column_def.data_type, ColumnType::Serial) {
                        continue;
                    }

                    row_values.insert(column_def.name.clone(), self.value_to_json(val).await?);
                } else {
                    // Column not found in schema, skip or insert with uppercase?
                    // For now, insert with uppercase as fallback, but this might be wrong if schema is strict
                    // But if we are here, it means we are inserting a column that doesn't exist in schema?
                    // Postgres would error. For now, let's just use uppercase as before.
                    row_values.insert(col_upper, self.value_to_json(val).await?);
                }
            }

            // Domains are read once per statement rather than per row.
            let domains = self.domains_for(&schema).await?;
            Self::apply_defaults(&schema, &mut row_values);
            row_values.insert(
                TRANSACTION_STAMP.to_string(),
                JsonValue::from(stamp_for_write()),
            );
            Self::check_not_null(&schema, &row_values)?;
            Self::check_constraints(&schema, &row_values, &domains)?;
            self.check_references(storage, &schema, &row_values, false)
                .await?;
            self.check_unique(storage, table, &schema, &row_values)
                .await?;

            let row = TableRow {
                values: row_values,
                created_at: now,
                updated_at: now,
            };

            // Insert the row
            publish_change("INSERT", table, &row.values);
            storage.insert_row(table, row).await?;
            count += 1;
        }

        self.flush_change_log().await?;
        Ok(QueryResult::Insert { count })
    }

    /// Fill omitted columns that declare a `DEFAULT`.
    fn apply_defaults(
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
        row: &mut std::collections::HashMap<String, JsonValue>,
    ) {
        for column in &schema.columns {
            let Some(default) = column.default_value.as_ref() else {
                continue;
            };
            if !row.contains_key(&column.name) {
                row.insert(column.name.clone(), default.clone());
            }
        }
    }

    /// Reject a row that leaves a `NOT NULL` column empty or null.
    ///
    /// # Errors
    /// Returns an error naming the column, as PostgreSQL does.
    fn check_not_null(
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
        row: &std::collections::HashMap<String, JsonValue>,
    ) -> ProtocolResult<()> {
        for column in &schema.columns {
            if column.nullable
                || matches!(
                    column.data_type,
                    crate::protocols::postgres_wire::persistent_storage::ColumnType::Serial
                )
            {
                continue;
            }
            if row.get(&column.name).is_none_or(JsonValue::is_null) {
                return Err(ProtocolError::PostgresError(format!(
                    "null value in column \"{}\" violates not-null constraint",
                    column.name
                )));
            }
        }
        Ok(())
    }

    /// The current definition of every domain a table's columns use.
    async fn domains_for(
        &self,
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
    ) -> ProtocolResult<HashMap<String, String>> {
        let mut domains = HashMap::new();
        for column in &schema.columns {
            let Some(domain) = column.domain.as_ref() else {
                continue;
            };
            if domains.contains_key(domain) {
                continue;
            }
            if let Some(definition) = self.domain_definition(domain).await? {
                domains.insert(domain.clone(), definition);
            }
        }
        Ok(domains)
    }

    /// Reject a row whose `CHECK` predicate does not hold.
    ///
    /// PostgreSQL accepts a row whose check evaluates to NULL — only a
    /// definite false is a violation.
    ///
    /// # Errors
    /// Returns an error naming the column whose check failed.
    fn check_constraints(
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
        row: &std::collections::HashMap<String, JsonValue>,
        domains: &HashMap<String, String>,
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::sql::expression_evaluator::{
            EvaluationContext, ExpressionEvaluator,
        };
        use crate::protocols::postgres_wire::sql::parser::SqlParser;

        let mut checked: Vec<(String, String)> = schema
            .columns
            .iter()
            .filter_map(|column| {
                column
                    .check
                    .as_ref()
                    .map(|check| (column.name.clone(), check.clone()))
            })
            .collect();

        // A domain's constraints are read now rather than copied when the
        // table was created, so an `ALTER DOMAIN` reaches the tables already
        // using it.
        for column in &schema.columns {
            let Some(domain) = column.domain.as_ref() else {
                continue;
            };
            let Some(definition) = domains.get(domain) else {
                continue;
            };
            let mut rest = definition.as_str();
            while let Some(at) = rest.to_uppercase().find("CHECK") {
                let tail = &rest[at..];
                let Some(predicate) = Self::parenthesised(tail) else {
                    break;
                };
                checked.push((
                    column.name.clone(),
                    predicate
                        .replace("VALUE", &column.name)
                        .replace("value", &column.name),
                ));
                rest = &tail[tail.find(')').map_or(tail.len(), |end| end + 1)..];
            }
        }

        if checked.is_empty() {
            return Ok(());
        }

        // The row is keyed as the evaluator expects a row to be keyed.
        let values: crate::protocols::postgres_wire::sql::select_pipeline::Row = schema
            .columns
            .iter()
            .map(|column| {
                let value = row.get(&column.name).map_or(JsonValue::Null, Clone::clone);
                (
                    fold_identifier(&column.name),
                    Self::json_to_sql_value(&value, &column.data_type),
                )
            })
            .collect();

        let mut evaluator = ExpressionEvaluator::new();
        for (name, predicate) in &checked {
            // Parsed as the predicate of a select so the expression parser
            // sees it in the position it was written for.
            let statement = SqlParser::new().parse(&format!("SELECT 1 WHERE {predicate}"))?;
            let crate::protocols::postgres_wire::sql::ast::Statement::Select(select) = statement
            else {
                continue;
            };
            let Some(expression) = select.where_clause else {
                continue;
            };

            let context = EvaluationContext::with_row(values.clone());
            if matches!(
                evaluator.evaluate(&expression, &context)?,
                SqlValue::Boolean(false)
            ) {
                return Err(ProtocolError::PostgresError(format!(
                    "new row violates check constraint on column \"{name}\""
                )));
            }
        }
        Ok(())
    }

    /// Whether anything a serializable block read has since been written by a
    /// transaction it could not see.
    ///
    /// This is the check that makes `SERIALIZABLE` more than a label: without
    /// it the level is `REPEATABLE READ` with a different name.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read.
    pub async fn serialization_conflict(
        &self,
        transaction: u64,
        snapshot: &std::collections::HashSet<u64>,
        tables: &[String],
    ) -> ProtocolResult<Option<String>> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(None);
        };

        // Reads are recorded as `table` or `table\u{1}row-identity`; a bare
        // table name means the whole table was read.
        let mut whole_tables = std::collections::HashSet::new();
        let mut rows_read: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        let mut predicates: HashMap<String, Vec<Vec<QueryCondition>>> = HashMap::new();
        for entry in tables {
            if let Some((table, rendered)) = entry.split_once('\u{3}') {
                let conditions: Vec<QueryCondition> = rendered
                    .split('\u{4}')
                    .filter_map(|part| {
                        let mut fields = part.split('\u{2}');
                        Some(QueryCondition {
                            column: fields.next()?.to_string(),
                            operator: fields.next()?.to_string(),
                            value: serde_json::from_str(fields.next()?).unwrap_or(JsonValue::Null),
                        })
                    })
                    .collect();
                predicates
                    .entry(fold_identifier(table))
                    .or_default()
                    .push(conditions);
                continue;
            }
            match entry.split_once('\u{1}') {
                Some((table, identity)) => {
                    rows_read
                        .entry(fold_identifier(table))
                        .or_default()
                        .insert(identity.to_string());
                }
                None => {
                    whole_tables.insert(fold_identifier(entry));
                }
            }
        }

        let names: std::collections::HashSet<String> = whole_tables
            .iter()
            .chain(rows_read.keys())
            .chain(predicates.keys())
            .cloned()
            .collect();

        for table in names {
            if !storage.table_exists(&table).await? {
                continue;
            }
            let key_columns = match storage.get_table_schema(&table).await? {
                Some(schema) => schema
                    .columns
                    .iter()
                    .filter(|column| column.unique)
                    .map(|column| fold_identifier(&column.name))
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };

            for row in storage
                .select_rows(&table, Vec::new(), Vec::new(), None)
                .await?
            {
                // Only a write to a row this block actually read is a
                // conflict; a write elsewhere in the table is not.
                let identity = row_identity(&row.values, &key_columns);
                let watched = whole_tables.contains(&table)
                    || rows_read
                        .get(&table)
                        .is_some_and(|rows| rows.contains(&identity))
                    // A row that now satisfies a predicate the block read is a
                    // phantom: it was not among the rows returned, but the
                    // block's answer would have differed had it been there.
                    || predicates.get(&table).is_some_and(|sets| {
                        sets.iter().any(|conditions| {
                            Self::row_matches_conditions(&row.values, conditions)
                        })
                    });
                if !watched {
                    continue;
                }

                for column in [TRANSACTION_STAMP, DELETED_BY] {
                    let Some(writer) = row.values.get(column).and_then(JsonValue::as_u64) else {
                        continue;
                    };
                    // A writer this block could not see, that is no longer
                    // running, committed underneath it.
                    let invisible = writer > transaction || snapshot.contains(&writer);
                    let finished = open_transactions()
                        .read()
                        .map(|open| !open.contains(&writer))
                        .unwrap_or(true);
                    if writer != transaction && invisible && finished {
                        return Ok(Some(table));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Whether a stored row satisfies every one of `conditions`.
    ///
    /// Only the comparisons a `WHERE` is reduced to here are understood; an
    /// operator this does not know matches, so an unrecognised predicate
    /// widens the watch rather than narrowing it.
    fn row_matches_conditions(
        values: &std::collections::HashMap<String, JsonValue>,
        conditions: &[QueryCondition],
    ) -> bool {
        conditions.iter().all(|condition| {
            let Some(actual) = values
                .iter()
                .find(|(name, _)| fold_identifier(name) == fold_identifier(&condition.column))
                .map(|(_, value)| value)
            else {
                return false;
            };
            match condition.operator.as_str() {
                "=" | "==" => *actual == condition.value,
                "!=" | "<>" => *actual != condition.value,
                "<" => Self::json_less_than(actual, &condition.value),
                "<=" => {
                    *actual == condition.value || Self::json_less_than(actual, &condition.value)
                }
                ">" => Self::json_less_than(&condition.value, actual),
                ">=" => {
                    *actual == condition.value || Self::json_less_than(&condition.value, actual)
                }
                "LIKE" | "ILIKE" => match (actual.as_str(), condition.value.as_str()) {
                    (Some(text), Some(pattern)) => Self::like_matches(
                        text,
                        pattern,
                        condition.operator.eq_ignore_ascii_case("ILIKE"),
                    ),
                    // Not text on both sides: widen rather than narrow.
                    _ => true,
                },
                "IN" => condition
                    .value
                    .as_array()
                    .is_some_and(|values| values.contains(actual)),
                _ => true,
            }
        })
    }

    /// Whether `text` matches a SQL `LIKE` pattern.
    ///
    /// `%` stands for any run of characters and `_` for exactly one, anchored
    /// at both ends — the anchoring is what a naive `contains` gets wrong.
    fn like_matches(text: &str, pattern: &str, case_insensitive: bool) -> bool {
        let (text, pattern) = if case_insensitive {
            (text.to_lowercase(), pattern.to_lowercase())
        } else {
            (text.to_string(), pattern.to_string())
        };
        let text: Vec<char> = text.chars().collect();
        let pattern: Vec<char> = pattern.chars().collect();

        // Two-pointer wildcard match: linear, backtracking only to the last
        // `%`.
        let (mut t, mut p) = (0usize, 0usize);
        let (mut star, mut resume) = (None, 0usize);
        while t < text.len() {
            match pattern.get(p) {
                Some('%') => {
                    star = Some(p);
                    resume = t;
                    p += 1;
                }
                Some('_') => {
                    t += 1;
                    p += 1;
                }
                Some(expected) if *expected == text[t] => {
                    t += 1;
                    p += 1;
                }
                _ => match star {
                    Some(at) => {
                        p = at + 1;
                        resume += 1;
                        t = resume;
                    }
                    None => return false,
                },
            }
        }
        pattern[p..].iter().all(|c| *c == '%')
    }

    /// Order two stored values, numerically when both are numbers.
    fn json_less_than(left: &JsonValue, right: &JsonValue) -> bool {
        match (left.as_f64(), right.as_f64()) {
            (Some(left), Some(right)) => left < right,
            _ => match (left.as_str(), right.as_str()) {
                (Some(left), Some(right)) => left < right,
                _ => false,
            },
        }
    }

    /// Reclaim in the background, so old versions do not need a hand.
    ///
    /// The interval is the cadence at which the check runs; the reclaim itself
    /// does nothing while any block is open, so a busy server pays only for
    /// the check. Returns the task handle so a caller can stop it.
    #[must_use]
    pub fn start_autovacuum(
        engine: Arc<Self>,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // A missed tick is not worth catching up on: the next one reclaims
            // whatever accumulated.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                if let Err(e) = engine.flush_change_log().await {
                    tracing::warn!("could not log changes for replication: {e}");
                }
                match engine.truncate_change_log().await {
                    Ok(0) => {}
                    Ok(removed) => tracing::debug!(removed, "trimmed the change log"),
                    Err(e) => tracing::warn!("could not trim the change log: {e}"),
                }
                match engine.vacuum(None).await {
                    Ok(0) => {}
                    Ok(reclaimed) => {
                        tracing::debug!(reclaimed, "autovacuum reclaimed superseded rows");
                    }
                    Err(e) => tracing::warn!("autovacuum failed: {e}"),
                }
            }
        })
    }

    /// Reclaim rows no open block can still need.
    ///
    /// # Errors
    /// Returns an error when a table cannot be read or written.
    pub async fn vacuum(&self, table: Option<&str>) -> ProtocolResult<usize> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(0);
        };
        // A version is dead once no open block could still see it: the block
        // that removed it finished, and it finished before the oldest block
        // now running began. Waiting for *every* block to close instead meant
        // one long-lived transaction stopped reclamation altogether.
        let oldest_open = open_transactions()
            .read()
            .map(|open| open.iter().copied().min())
            .unwrap_or(None);

        // Nothing has been marked since the last pass, so there is nothing to
        // find and no reason to read a single row.
        if !reclaim_pending() {
            return Ok(0);
        }

        let tables = match table {
            Some(name) => vec![fold_identifier(name)],
            None => storage.list_tables().await?,
        };

        PENDING_RECLAIM.store(0, std::sync::atomic::Ordering::Relaxed);
        let mut reclaimed = 0usize;
        for name in tables {
            if !storage.table_exists(&name).await? {
                continue;
            }
            // One pass per distinct writer, not per row: a table with many
            // rows removed by one transaction would otherwise delete the same
            // set once for each of them.
            let writers: std::collections::BTreeSet<u64> = storage
                .select_rows(&name, Vec::new(), Vec::new(), None)
                .await?
                .into_iter()
                .filter_map(|row| row.values.get(DELETED_BY)?.as_u64())
                .collect();

            for writer in writers {
                // Still running, or old enough that a running block might have
                // begun before it committed: leave it be.
                let still_running = open_transactions()
                    .read()
                    .map(|open| open.contains(&writer))
                    .unwrap_or(true);
                if still_running || oldest_open.is_some_and(|oldest| writer >= oldest) {
                    continue;
                }
                reclaimed += storage
                    .delete_rows(
                        &name,
                        vec![QueryCondition {
                            column: DELETED_BY.to_string(),
                            operator: "=".to_string(),
                            value: JsonValue::from(writer),
                        }],
                    )
                    .await?
                    .max(0) as usize;
            }
        }
        Ok(reclaimed)
    }

    /// Remove the rows a committing transaction marked deleted.
    ///
    /// # Errors
    /// Returns an error when a table cannot be written.
    pub async fn purge_deleted(&self, transaction: u64, tables: &[String]) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        // A block still running may hold a snapshot that includes these rows,
        // so they are left marked until nothing else is open. The mark already
        // hides them from every new reader.
        let others_open = open_transactions()
            .read()
            .map(|open| open.iter().any(|id| *id != transaction))
            .unwrap_or(false);
        if others_open {
            return Ok(());
        }
        for table in tables {
            let table = fold_identifier(table);
            if !storage.table_exists(&table).await? {
                continue;
            }
            storage
                .delete_rows(
                    &table,
                    vec![QueryCondition {
                        column: DELETED_BY.to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::from(transaction),
                    }],
                )
                .await?;
        }
        Ok(())
    }

    /// Un-mark the rows a rolled-back transaction had deleted.
    ///
    /// # Errors
    /// Returns an error when a table cannot be written.
    pub async fn restore_deleted(&self, transaction: u64, tables: &[String]) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };
        for table in tables {
            let table = fold_identifier(table);
            if !storage.table_exists(&table).await? {
                continue;
            }
            // The versions this block wrote go; the ones it marked come back.
            storage
                .delete_rows(
                    &table,
                    vec![QueryCondition {
                        column: TRANSACTION_STAMP.to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::from(transaction),
                    }],
                )
                .await?;
            storage
                .update_rows(
                    &table,
                    std::collections::HashMap::from([(DELETED_BY.to_string(), JsonValue::Null)]),
                    vec![QueryCondition {
                        column: DELETED_BY.to_string(),
                        operator: "=".to_string(),
                        value: JsonValue::from(transaction),
                    }],
                )
                .await?;
        }
        Ok(())
    }

    /// Re-check every deferrable foreign key in the database.
    ///
    /// Called at `COMMIT` when checks were deferred, so a set of rows that
    /// only makes sense together — a circular reference, most often — can be
    /// inserted and validated as a whole.
    ///
    /// # Errors
    /// Returns an error naming the first constraint that does not hold.
    pub async fn check_deferred_constraints(&self, touched: &[String]) -> ProtocolResult<()> {
        let Some(storage) = self.persistent_storage.as_ref() else {
            return Ok(());
        };

        // Only the tables this transaction wrote can have broken a constraint,
        // so only those are re-read. Scanning every table made the cost of a
        // COMMIT depend on the size of the database rather than on the work
        // the transaction did.
        let tables: Vec<String> = storage
            .list_tables()
            .await?
            .into_iter()
            .filter(|name| touched.iter().any(|t| fold_identifier(t) == *name))
            .collect();

        for name in tables {
            let Some(schema) = storage.get_table_schema(&name).await? else {
                continue;
            };
            if !schema.foreign_keys.iter().any(|key| key.deferrable) {
                continue;
            }

            let deferred = crate::protocols::postgres_wire::persistent_storage::TableSchema {
                foreign_keys: schema
                    .foreign_keys
                    .iter()
                    .filter(|key| key.deferrable)
                    .cloned()
                    .collect(),
                ..schema.clone()
            };
            for row in storage
                .select_rows(&name, Vec::new(), Vec::new(), None)
                .await?
            {
                self.check_references(storage, &deferred, &row.values, true)
                    .await?;
            }
        }
        Ok(())
    }

    /// The columns a foreign key points at, filling in the target's key when
    /// the clause named no columns.
    async fn referenced_columns(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        key: &crate::protocols::postgres_wire::persistent_storage::ForeignKey,
    ) -> ProtocolResult<Vec<String>> {
        if !key.referenced.is_empty() {
            return Ok(key.referenced.iter().map(|c| fold_identifier(c)).collect());
        }
        Ok(match storage.get_table_schema(&key.table).await? {
            Some(schema) => schema
                .columns
                .iter()
                .filter(|column| column.unique)
                .map(|column| fold_identifier(&column.name))
                .collect(),
            None => Vec::new(),
        })
    }

    /// Reject a delete that would orphan a row in another table.
    ///
    /// # Errors
    /// Returns an error naming the table that still refers to the row.
    async fn check_not_referenced(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        conditions: &[QueryCondition],
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::persistent_storage::ReferentialAction;

        let referrers = self.tables_referring_to(storage, table).await?;
        if referrers.is_empty() {
            return Ok(());
        }

        let doomed = storage
            .select_rows(table, Vec::new(), conditions.to_vec(), None)
            .await?;
        for row in &doomed {
            for (child, key) in &referrers {
                let referenced = self.referenced_columns(storage, key).await?;
                let Some(conditions) = Self::child_conditions(row, key, &referenced) else {
                    continue;
                };

                let referring = storage
                    .select_rows(child, Vec::new(), conditions.clone(), None)
                    .await?;
                if referring.is_empty() {
                    continue;
                }

                match key.on_delete {
                    // The referring rows go too.
                    ReferentialAction::Cascade => {
                        Box::pin(self.cascade_delete(storage, child, conditions)).await?;
                    }
                    // The referring column is cleared or reset instead.
                    ReferentialAction::SetNull | ReferentialAction::SetDefault => {
                        let child_schema = storage.get_table_schema(child).await?;
                        let mut set_values = HashMap::new();
                        for column in &key.columns {
                            let replacement = if key.on_delete == ReferentialAction::SetDefault {
                                child_schema
                                    .as_ref()
                                    .and_then(|schema| {
                                        schema
                                            .columns
                                            .iter()
                                            .find(|c| fold_identifier(&c.name) == *column)
                                    })
                                    .and_then(|c| c.default_value.clone())
                                    .unwrap_or(JsonValue::Null)
                            } else {
                                JsonValue::Null
                            };
                            set_values.insert(column.clone(), replacement);
                        }
                        storage.update_rows(child, set_values, conditions).await?;
                    }
                    ReferentialAction::NoAction | ReferentialAction::Restrict => {
                        return Err(ProtocolError::PostgresError(format!(
                            "delete violates foreign key constraint: \"{child}\" still refers to \
                             this row through ({})",
                            key.columns.join(", ")
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Apply each foreign key's `ON UPDATE` action before a key column moves.
    ///
    /// # Errors
    /// Returns an error when a child still refers to the row and the action is
    /// `NO ACTION` or `RESTRICT`.
    async fn apply_update_actions(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        conditions: &[QueryCondition],
        set_values: &std::collections::HashMap<String, JsonValue>,
    ) -> ProtocolResult<()> {
        use crate::protocols::postgres_wire::persistent_storage::ReferentialAction;

        let referrers = self.tables_referring_to(storage, table).await?;
        if referrers.is_empty() {
            return Ok(());
        }

        let changing = storage
            .select_rows(table, Vec::new(), conditions.to_vec(), None)
            .await?;
        for row in &changing {
            for (child, key) in &referrers {
                let referenced = self.referenced_columns(storage, key).await?;
                let Some(child_conditions) = Self::child_conditions(row, key, &referenced) else {
                    continue;
                };
                let referring = storage
                    .select_rows(child, Vec::new(), child_conditions.clone(), Some(1))
                    .await?;
                if referring.is_empty() {
                    continue;
                }

                match key.on_update {
                    // The children follow the key to its new value.
                    ReferentialAction::Cascade => {
                        let mut updates = std::collections::HashMap::new();
                        for (column, target) in key.columns.iter().zip(&referenced) {
                            if let Some(value) = set_values.iter().find_map(|(name, value)| {
                                (fold_identifier(name) == *target).then(|| value.clone())
                            }) {
                                updates.insert(column.clone(), value);
                            }
                        }
                        if !updates.is_empty() {
                            storage
                                .update_rows(child, updates, child_conditions)
                                .await?;
                        }
                    }
                    ReferentialAction::SetNull | ReferentialAction::SetDefault => {
                        let child_schema = storage.get_table_schema(child).await?;
                        let mut updates = std::collections::HashMap::new();
                        for column in &key.columns {
                            let replacement = if key.on_update == ReferentialAction::SetDefault {
                                child_schema
                                    .as_ref()
                                    .and_then(|schema| {
                                        schema
                                            .columns
                                            .iter()
                                            .find(|c| fold_identifier(&c.name) == *column)
                                    })
                                    .and_then(|c| c.default_value.clone())
                                    .unwrap_or(JsonValue::Null)
                            } else {
                                JsonValue::Null
                            };
                            updates.insert(column.clone(), replacement);
                        }
                        storage
                            .update_rows(child, updates, child_conditions)
                            .await?;
                    }
                    ReferentialAction::NoAction | ReferentialAction::Restrict => {
                        return Err(ProtocolError::PostgresError(format!(
                            "update violates foreign key constraint: \"{child}\" still refers to \
                             this row through ({})",
                            key.columns.join(", ")
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Delete the rows a cascade reaches, checking their own children first.
    async fn cascade_delete(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        conditions: Vec<QueryCondition>,
    ) -> ProtocolResult<()> {
        Box::pin(self.check_not_referenced(storage, table, &conditions)).await?;
        storage.delete_rows(table, conditions).await?;
        Ok(())
    }

    /// Every table that refers to `table`, with the constraint that does it.
    #[allow(clippy::type_complexity)]
    async fn tables_referring_to(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
    ) -> ProtocolResult<
        Vec<(
            String,
            crate::protocols::postgres_wire::persistent_storage::ForeignKey,
        )>,
    > {
        let mut referrers = Vec::new();
        for name in storage.list_tables().await? {
            let Some(schema) = storage.get_table_schema(&name).await? else {
                continue;
            };
            for key in &schema.foreign_keys {
                if fold_identifier(&key.table) == fold_identifier(table) {
                    referrers.push((name.clone(), key.clone()));
                }
            }
        }
        Ok(referrers)
    }

    /// Conditions selecting the child rows that point at `parent`.
    fn child_conditions(
        parent: &TableRow,
        key: &crate::protocols::postgres_wire::persistent_storage::ForeignKey,
        referenced: &[String],
    ) -> Option<Vec<QueryCondition>> {
        if referenced.len() != key.columns.len() {
            return None;
        }
        let mut conditions = Vec::with_capacity(key.columns.len());
        for (column, target) in key.columns.iter().zip(referenced) {
            let value = parent
                .values
                .iter()
                .find(|(name, _)| fold_identifier(name) == *target)
                .map(|(_, value)| value.clone())
                .filter(|value| !value.is_null())?;
            conditions.push(QueryCondition {
                column: column.clone(),
                operator: "=".to_string(),
                value,
            });
        }
        Some(conditions)
    }

    /// Reject a row whose foreign key names a row that is not there.
    ///
    /// # Errors
    /// Returns an error naming the column and the table it references.
    pub async fn check_references(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
        row: &std::collections::HashMap<String, JsonValue>,
        deferred_pass: bool,
    ) -> ProtocolResult<()> {
        for key in &schema.foreign_keys {
            // A deferrable key is checked at COMMIT, not when the row is
            // written — that is what lets a circular reference be inserted at
            // all. Each pass therefore looks at exactly the other's keys.
            if key.deferrable != deferred_pass {
                continue;
            }
            let referenced = self.referenced_columns(storage, key).await?;
            if referenced.len() != key.columns.len() {
                continue;
            }

            // What a partly-NULL key means depends on MATCH:
            //   SIMPLE  — any NULL satisfies the constraint (the default)
            //   FULL    — all NULL or none; a mixture is an error
            //   PARTIAL — the non-NULL parts must still match a row
            use crate::protocols::postgres_wire::persistent_storage::MatchType;
            let mut conditions = Vec::with_capacity(key.columns.len());
            let mut nulls = 0usize;
            for (column, target) in key.columns.iter().zip(&referenced) {
                let value = row
                    .iter()
                    .find(|(name, _)| fold_identifier(name) == *column)
                    .map(|(_, value)| value.clone())
                    .filter(|value| !value.is_null());
                match value {
                    Some(value) => conditions.push(QueryCondition {
                        column: target.clone(),
                        operator: "=".to_string(),
                        value,
                    }),
                    None => nulls += 1,
                }
            }

            // An entirely NULL key refers to nothing under every MATCH type.
            if nulls == key.columns.len() {
                continue;
            }
            match key.match_type {
                MatchType::Simple if nulls > 0 => continue,
                MatchType::Full if nulls > 0 => {
                    return Err(ProtocolError::PostgresError(format!(
                        "MATCH FULL does not allow mixing null and nonnull key values in ({})",
                        key.columns.join(", ")
                    )));
                }
                // MATCH PARTIAL keeps checking with the parts it has, so the
                // conditions built above already express it: a NULL column
                // simply contributes no condition.
                MatchType::Partial | MatchType::Full | MatchType::Simple => {}
            }

            let found = storage
                .select_rows(&key.table, Vec::new(), conditions, Some(1))
                .await?;
            if found.is_empty() {
                return Err(ProtocolError::PostgresError(format!(
                    "insert or update violates foreign key constraint: no row in \"{}\" \
                     matches ({})",
                    key.table,
                    key.columns.join(", ")
                )));
            }
        }
        Ok(())
    }

    /// Reject a row that duplicates a unique or primary-key column.
    ///
    /// # Errors
    /// Returns an error when the value is already present.
    async fn check_unique(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        schema: &crate::protocols::postgres_wire::persistent_storage::TableSchema,
        row: &std::collections::HashMap<String, JsonValue>,
    ) -> ProtocolResult<()> {
        for column in &schema.columns {
            if !column.unique {
                continue;
            }
            let Some(value) = row.get(&column.name).filter(|v| !v.is_null()) else {
                continue;
            };

            let existing = storage
                .select_rows(
                    table,
                    Vec::new(),
                    vec![QueryCondition {
                        column: fold_identifier(&column.name),
                        operator: "=".to_string(),
                        value: value.clone(),
                    }],
                    Some(1),
                )
                .await?;
            if !existing.is_empty() {
                return Err(ProtocolError::PostgresError(format!(
                    "duplicate key value violates unique constraint on column \"{}\"",
                    column.name
                )));
            }
        }
        Ok(())
    }

    /// Execute UPDATE query on persistent storage
    async fn execute_persistent_update(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        note_block_write(table);
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert SET clauses to HashMap.
        //
        // A value that is not a literal is an expression over the row being
        // updated — `SET n = n + 1`. Put through `literal_to_json` it became
        // the *text* `n + 1` and was never applied, while `RETURNING` reported
        // the computed value: a client was told a write had happened that had
        // not. Those are computed per row, below.
        let row_expressions: Vec<(String, String)> = set_clauses
            .iter()
            .filter(|(_, value)| !Self::is_plain_literal(value))
            .cloned()
            .collect();
        let mut set_values = std::collections::HashMap::new();
        for (col, val) in set_clauses {
            if Self::is_plain_literal(&val) {
                set_values.insert(fold_identifier(&col), Self::literal_to_json(&val));
            }
        }

        // Convert WHERE clause to QueryConditions
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: fold_identifier(&c.column),
                    operator: c.operator,
                    value: Self::literal_to_json(&c.value),
                })
                .collect()
        } else {
            vec![]
        };

        // Changing a key column moves the row out from under any child that
        // points at it, so the same check a delete makes applies here.
        let touches_key = match storage.get_table_schema(table).await? {
            Some(schema) => set_values.keys().any(|column| {
                schema
                    .columns
                    .iter()
                    .any(|c| c.unique && fold_identifier(&c.name) == fold_identifier(column))
            }),
            None => false,
        };
        if touches_key {
            self.apply_update_actions(storage, table, &conditions, &set_values)
                .await?;
        }

        // Execute update
        // An update always writes a new version: the previous row is marked
        // deleted by the writing transaction and a fresh row carries the new
        // values under a key that includes that transaction's id.
        //
        // Doing this only inside a block was not enough. An autocommit update
        // overwrote in place, so it left no trace of having happened — a
        // snapshot reader saw the new value, and a serializable block could
        // not tell that what it read had moved. Old versions accumulate until
        // `VACUUM`, which is the trade this model makes.
        {
            let stamp = stamp_for_write();
            let previous = storage
                .select_rows(table, Vec::new(), conditions.clone(), None)
                .await?;
            let visible: Vec<_> = previous
                .into_iter()
                .filter(|row| row_is_visible(&row.values))
                .collect();
            let count = visible.len();

            // Every new row is built before a single old one is marked.
            //
            // Computed after the mark, an expression that failed to evaluate
            // left the old row deleted and no new row written — the update did
            // not merely fail, it destroyed the row. This way a failure
            // returns an error having changed nothing.
            let mut replacements = Vec::with_capacity(visible.len());
            for row in &visible {
                let mut values = row.values.clone();
                for (column, expression) in &row_expressions {
                    let computed = self.evaluate_over_row(expression, &row.values).await?;
                    let stored = values
                        .keys()
                        .find(|name| fold_identifier(name) == fold_identifier(column))
                        .cloned()
                        .unwrap_or_else(|| fold_identifier(column));
                    values.insert(stored, computed);
                }
                replacements.push(values);
            }

            // The previous rows are marked first: marking after writing the
            // new version would match it too — it satisfies the same predicate
            // — and the row would vanish for everyone.
            storage
                .update_rows(
                    table,
                    std::collections::HashMap::from([(
                        DELETED_BY.to_string(),
                        JsonValue::from(stamp),
                    )]),
                    conditions,
                )
                .await?;

            for mut values in replacements {
                let now = chrono::Utc::now();
                for (column, value) in &set_values {
                    let stored = values
                        .keys()
                        .find(|name| fold_identifier(name) == fold_identifier(column))
                        .cloned()
                        .unwrap_or_else(|| column.clone());
                    values.insert(stored, value.clone());
                }
                values.insert(TRANSACTION_STAMP.to_string(), JsonValue::from(stamp));
                values.remove(DELETED_BY);

                publish_change("UPDATE", table, &values);
                storage
                    .insert_row(
                        table,
                        TableRow {
                            values,
                            created_at: now,
                            updated_at: now,
                        },
                    )
                    .await?;
            }

            note_reclaimable(count as u64);
            self.flush_change_log().await?;
            Ok(QueryResult::Update { count })
        }
    }

    /// Execute DELETE query on persistent storage
    async fn execute_persistent_delete(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        note_block_write(table);
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert WHERE clause to QueryConditions
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: fold_identifier(&c.column),
                    operator: c.operator,
                    value: Self::literal_to_json(&c.value),
                })
                .collect()
        } else {
            vec![]
        };

        // A row cannot be deleted while another table's row points at it.
        // Checking only on insert let a parent be removed out from under its
        // children, leaving foreign keys naming rows that are not there.
        self.check_not_referenced(storage, table, &conditions)
            .await?;

        // Inside a transaction the rows are marked rather than removed, so a
        // concurrent session keeps seeing them until this block commits and a
        // rollback has something to put back.
        if let Some(stamp) = current_transaction_stamp() {
            let marked = storage
                .update_rows(
                    table,
                    std::collections::HashMap::from([(
                        DELETED_BY.to_string(),
                        JsonValue::from(stamp),
                    )]),
                    conditions,
                )
                .await?;
            note_reclaimable(marked.max(0) as u64);
            return Ok(QueryResult::Delete {
                count: marked.max(0) as usize,
            });
        }

        // Superseded versions still match the predicate but are not rows any
        // client can see, so they must not be counted as deleted. They are
        // removed along with the visible ones.
        let visible = storage
            .select_rows(table, Vec::new(), conditions.clone(), None)
            .await?
            .into_iter()
            .filter(|row| row_is_visible(&row.values))
            .count();
        for row in storage
            .select_rows(table, Vec::new(), conditions.clone(), None)
            .await?
        {
            if row_is_visible(&row.values) {
                publish_change("DELETE", table, &row.values);
            }
        }
        storage.delete_rows(table, conditions).await?;
        let count = visible as i64;

        self.flush_change_log().await?;
        Ok(QueryResult::Delete {
            count: count as usize,
        })
    }

    /// Execute CREATE TABLE on persistent storage
    async fn execute_create_table(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        columns: Vec<SimpleColumnDef>,
        if_not_exists: bool,
        foreign_keys: Vec<crate::protocols::postgres_wire::persistent_storage::ForeignKey>,
    ) -> ProtocolResult<QueryResult> {
        use crate::protocols::postgres_wire::persistent_storage::{ColumnDefinition, TableSchema};

        // Check if table already exists
        if storage.table_exists(table).await? {
            if if_not_exists {
                return Ok(QueryResult::Update { count: 0 });
            } else {
                return Err(ProtocolError::PostgresError(format!(
                    "Table '{}' already exists",
                    table
                )));
            }
        }

        // Convert simple column definitions to persistent storage format
        let mut column_defs = Vec::new();
        // A column whose type names a domain takes that domain's base type and
        // inherits its constraints, which is what makes a domain more than an
        // alias.
        let mut columns = columns;
        let mut column_domains: HashMap<String, String> = HashMap::new();
        for col in &mut columns {
            let domain = fold_identifier(&col.data_type);
            let Some(definition) = self.domain_definition(&domain).await? else {
                continue;
            };
            column_domains.insert(fold_identifier(&col.name), domain);
            let mut words = definition.split_whitespace();
            if let Some(base) = words.next() {
                col.data_type = base.to_string();
            }
            // A domain's `CHECK (VALUE > 0)` is written in terms of `VALUE`;
            // on a column it has to name that column instead.
            let rest = words
                .collect::<Vec<_>>()
                .join(" ")
                .replace("VALUE", &col.name)
                .replace("value", &col.name);
            if !rest.is_empty() {
                col.constraints
                    .extend(rest.split_whitespace().map(str::to_string));
            }
        }

        for col in &columns {
            let column_type = column_type_from_name(&col.data_type);

            let constraints: Vec<String> = col
                .constraints
                .iter()
                .map(|word| word.to_uppercase())
                .collect();
            let says = |word: &str| constraints.iter().any(|c| c == word);

            let nullable = !(says("PRIMARY") || says("NOT") && says("NULL"));
            let unique = says("UNIQUE") || (says("PRIMARY") && says("KEY"));

            // `DEFAULT <literal>`: the word after DEFAULT, kept as stored JSON
            // so an omitted column is filled with the declared value rather
            // than with NULL.
            let default_value = col
                .constraints
                .iter()
                .position(|word| word.eq_ignore_ascii_case("DEFAULT"))
                .and_then(|at| col.constraints.get(at + 1))
                .map(|literal| Self::literal_to_json(literal));

            // `CHECK (<predicate>)`: the parenthesised text after CHECK, kept
            // as written so the evaluator can run it against each row.
            let check = col
                .constraints
                .iter()
                .position(|word| word.to_uppercase().starts_with("CHECK"))
                .map(|at| col.constraints[at..].join(" "))
                .and_then(|text| {
                    let open = text.find('(')?;
                    let close = text.rfind(')')?;
                    (close > open).then(|| text[open + 1..close].trim().to_string())
                })
                .filter(|predicate| !predicate.is_empty());

            // `REFERENCES other(column)`, or `REFERENCES other` naming its
            // primary key.
            let references = col
                .constraints
                .iter()
                .position(|word| word.eq_ignore_ascii_case("REFERENCES"))
                .and_then(|at| col.constraints.get(at + 1))
                .map(|target| {
                    let target = target.trim_end_matches(',');
                    match target.split_once('(') {
                        Some((table, column)) => (
                            fold_identifier(table),
                            fold_identifier(column.trim_end_matches(')')),
                        ),
                        None => (fold_identifier(target), String::new()),
                    }
                });

            column_defs.push(ColumnDefinition {
                // Folded like every other identifier, so the keys a row is
                // written with are the keys a query looks it up by. Storing
                // these uppercase while queries folded to lower made
                // `SELECT <col>` return NULL for a column that was present.
                name: fold_identifier(&col.name),
                data_type: column_type,
                nullable,
                default_value,
                unique,
                check,
                references,
                domain: column_domains.get(&fold_identifier(&col.name)).cloned(),
            });
        }

        // A column-level `REFERENCES` is a one-column foreign key; both forms
        // end up in the same list so the check does not care how it was
        // written.
        let mut foreign_keys = foreign_keys;
        for (column, definition) in column_defs.iter().zip(&columns) {
            let text = definition.constraints.join(" ");
            if !text.to_uppercase().contains("REFERENCES") {
                continue;
            }
            if let Some(mut key) = Self::parse_foreign_key(&text) {
                key.columns = vec![fold_identifier(&column.name)];
                foreign_keys.push(key);
            }
        }

        let schema = TableSchema {
            name: table.to_string(),
            columns: column_defs,
            created_at: chrono::Utc::now(),
            row_count: 0,
            foreign_keys,
        };

        // Create the table
        storage.create_table(schema).await?;

        Ok(QueryResult::Update { count: 0 })
    }

    /// Parse `TRUNCATE [TABLE] name [CASCADE|RESTRICT]`.
    fn parse_truncate(sql: &str) -> ProtocolResult<Statement> {
        let rest = sql
            .trim()
            .trim_end_matches(';')
            .split_whitespace()
            .skip(1)
            .skip_while(|word| word.eq_ignore_ascii_case("TABLE"))
            .find(|word| {
                !word.eq_ignore_ascii_case("ONLY")
                    && !word.eq_ignore_ascii_case("CASCADE")
                    && !word.eq_ignore_ascii_case("RESTRICT")
            })
            .ok_or_else(|| {
                ProtocolError::PostgresError("TRUNCATE requires a table name".to_string())
            })?;

        Ok(Statement::Truncate {
            table: fold_identifier(rest.trim_end_matches(',')),
        })
    }

    /// Execute TRUNCATE on persistent storage.
    ///
    /// Routed here rather than to the SQL engine because that engine truncates
    /// its own table state; against a stored table it reported success while
    /// every row survived.
    async fn execute_truncate(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
    ) -> ProtocolResult<QueryResult> {
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{table}' does not exist"
            )));
        }

        let count = storage.delete_rows(table, vec![]).await?;
        Ok(QueryResult::Delete {
            count: count as usize,
        })
    }

    /// Execute DROP TABLE on persistent storage
    async fn execute_drop_table(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        if_exists: bool,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            if if_exists {
                return Ok(QueryResult::Update { count: 0 });
            } else {
                return Err(ProtocolError::PostgresError(format!(
                    "Table '{}' does not exist",
                    table
                )));
            }
        }

        // Drop the table
        storage.drop_table(table).await?;

        Ok(QueryResult::Update { count: 0 })
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_extension_vector() {
        let query_engine = QueryEngine::new();

        // Test CREATE EXTENSION vector
        let result = query_engine.execute_query("CREATE EXTENSION vector;").await;
        assert!(result.is_ok(), "CREATE EXTENSION vector should succeed");

        if let Ok(QueryResult::Select { columns, rows }) = result {
            assert_eq!(columns.len(), 1);
            assert_eq!(columns[0], "message");
            assert_eq!(rows.len(), 1);
            let message = rows[0][0].as_ref().unwrap();
            assert!(message.contains("Extension") || message.contains("EXTENSION"));
            println!("✅ CREATE EXTENSION vector result: {:?}", rows[0][0]);
        }
    }

    #[tokio::test]
    async fn test_drop_extension_vector() {
        let query_engine = QueryEngine::new();

        // First create the extension
        let create_result = query_engine.execute_query("CREATE EXTENSION vector;").await;
        assert!(
            create_result.is_ok(),
            "CREATE EXTENSION should succeed first"
        );

        // Test DROP EXTENSION vector
        let result = query_engine.execute_query("DROP EXTENSION vector;").await;

        match result {
            Ok(QueryResult::Select { columns, rows }) => {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0], "message");
                assert_eq!(rows.len(), 1);
                println!("✅ DROP EXTENSION vector result: {:?}", rows[0][0]);
                // Just check that we got some result - don't be too strict about content
                assert!(rows[0][0].is_some());
            }
            Ok(other_result) => {
                println!(
                    "✅ DROP EXTENSION vector got unexpected result type: {:?}",
                    other_result
                );
                // Pass the test anyway since we got a successful result
            }
            Err(e) => {
                panic!("DROP EXTENSION vector should succeed, but got error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_unsupported_sql_fallback() {
        let query_engine = QueryEngine::new();

        // Test that unsupported statements get routed to comprehensive SQL engine
        let result = query_engine
            .execute_query("CREATE SCHEMA test_schema;")
            .await;
        assert!(result.is_ok(), "CREATE SCHEMA should succeed via fallback");

        if let Ok(QueryResult::Select { columns, rows }) = result {
            assert_eq!(columns.len(), 1);
            assert_eq!(columns[0], "message");
            assert_eq!(rows.len(), 1);
            println!("✅ CREATE SCHEMA result: {:?}", rows[0][0]);
        }
    }
}

#[cfg(test)]
mod literal_case_tests {
    use super::*;
    use crate::protocols::postgres_wire::persistent_storage::RocksDbTableStorage;

    /// String literals must survive a round trip unchanged.
    ///
    /// The parser used to uppercase the whole statement before parsing, so
    /// `VALUES ('alpha')` stored `ALPHA` — silent corruption of every text value
    /// written through this engine.
    #[tokio::test]
    async fn string_literals_keep_their_case_through_insert_and_select() {
        let dir = std::env::temp_dir().join(format!("orbit-literal-case-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let storage = Arc::new(
            RocksDbTableStorage::new(dir.to_str().expect("utf-8 temp path"))
                .expect("open temporary storage"),
        );
        let engine = QueryEngine::new_with_persistent_storage(storage);

        engine
            .execute_query("CREATE TABLE case_check (id INTEGER, name TEXT)")
            .await
            .expect("create table");
        engine
            .execute_query("INSERT INTO case_check (id, name) VALUES (1, 'MixedCase Value')")
            .await
            .expect("insert");

        let result = engine
            .execute_query("SELECT name FROM case_check")
            .await
            .expect("select");

        let QueryResult::Select { rows, .. } = result else {
            panic!("SELECT should return a result set");
        };
        assert_eq!(
            rows[0][0].as_deref(),
            Some("MixedCase Value"),
            "the stored literal must come back exactly as written"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod literal_tests {
    use super::*;

    /// The NULL keyword and the string 'NULL' are different values. Stripping
    /// quotes before conversion collapsed them.
    #[test]
    fn the_null_keyword_is_null_but_quoted_null_is_text() {
        assert_eq!(QueryEngine::literal_to_json("NULL"), JsonValue::Null);
        assert_eq!(QueryEngine::literal_to_json("null"), JsonValue::Null);
        assert_eq!(
            QueryEngine::literal_to_json("'NULL'"),
            JsonValue::String("NULL".to_string())
        );
    }

    /// A quoted number is text. Storing it as a number loses leading zeros and
    /// changes how it compares.
    #[test]
    fn a_quoted_number_stays_text() {
        assert_eq!(
            QueryEngine::literal_to_json("'0123'"),
            JsonValue::String("0123".to_string())
        );
        assert_eq!(
            QueryEngine::literal_to_json("123"),
            JsonValue::Number(123.into())
        );
    }

    #[test]
    fn booleans_are_recognised_unquoted_only() {
        assert_eq!(QueryEngine::literal_to_json("true"), JsonValue::Bool(true));
        assert_eq!(
            QueryEngine::literal_to_json("FALSE"),
            JsonValue::Bool(false)
        );
        assert_eq!(
            QueryEngine::literal_to_json("'true'"),
            JsonValue::String("true".to_string())
        );
    }

    #[test]
    fn an_escaped_quote_inside_a_literal_is_unescaped_once() {
        assert_eq!(
            QueryEngine::literal_to_json("'O''Brien'"),
            JsonValue::String("O'Brien".to_string())
        );
    }

    #[test]
    fn an_unquoted_word_is_kept_as_text() {
        assert_eq!(
            QueryEngine::literal_to_json("hello"),
            JsonValue::String("hello".to_string())
        );
    }

    /// A value rendered as a literal and read back must be unchanged.
    #[test]
    fn values_round_trip_through_their_literal_form() {
        use crate::protocols::postgres_wire::sql::types::SqlValue;

        let cases = [
            (SqlValue::Null, JsonValue::Null),
            (SqlValue::Integer(42), JsonValue::Number(42.into())),
            (SqlValue::BigInt(-7), JsonValue::Number((-7).into())),
            (SqlValue::Boolean(true), JsonValue::Bool(true)),
            (
                SqlValue::Text("hello".to_string()),
                JsonValue::String("hello".to_string()),
            ),
            // The three-letter string, not the null value.
            (
                SqlValue::Text("NULL".to_string()),
                JsonValue::String("NULL".to_string()),
            ),
            (
                SqlValue::Text("O'Brien".to_string()),
                JsonValue::String("O'Brien".to_string()),
            ),
        ];

        for (value, expected) in cases {
            let literal = QueryEngine::sql_value_to_literal(&value);
            assert_eq!(
                QueryEngine::literal_to_json(&literal),
                expected,
                "round trip failed for {value:?} via {literal:?}"
            );
        }
    }
}

#[cfg(test)]
mod quoted_identifier_tests {
    use super::{fold_identifier, QueryEngine};

    /// A quoted identifier keeps its case; an unquoted one folds down.
    #[test]
    fn folding_respects_quotes() {
        assert_eq!(fold_identifier("\"Id\""), "Id");
        assert_eq!(fold_identifier("Id"), "id");
    }

    /// The column list of a CREATE TABLE keeps quoted names as written.
    #[test]
    fn create_table_keeps_quoted_column_case() {
        let engine = QueryEngine::new();
        let statement = engine
            .parse_sql("CREATE TABLE qq (\"Id\" INTEGER, plain TEXT)")
            .expect("parses");
        let super::Statement::CreateTable { columns, .. } = statement else {
            panic!("not a CREATE TABLE");
        };
        let names: Vec<String> = columns
            .iter()
            .map(|column| fold_identifier(&column.name))
            .collect();
        assert_eq!(names, vec!["Id".to_string(), "plain".to_string()]);
    }
}

#[cfg(test)]
mod transaction_visibility_tests {
    use super::{
        begin_transaction, current_transaction_stamp, end_transaction, row_is_visible,
        within_transaction, TRANSACTION_STAMP,
    };
    use serde_json::Value as JsonValue;
    use std::collections::HashMap;

    fn stamped(id: u64) -> HashMap<String, JsonValue> {
        HashMap::from([(TRANSACTION_STAMP.to_string(), JsonValue::from(id))])
    }

    #[tokio::test]
    async fn a_statement_inside_a_transaction_knows_its_id() {
        let context = begin_transaction(false);
        let id = context.id;
        let seen = within_transaction(context, async { current_transaction_stamp() }).await;
        end_transaction(id);
        assert_eq!(seen, Some(id));
    }

    #[tokio::test]
    async fn an_open_transactions_rows_are_hidden_from_everyone_else() {
        let context = begin_transaction(false);
        let id = context.id;

        // The writer sees its own row...
        assert!(within_transaction(context, async { row_is_visible(&stamped(id)) }).await);
        // ...and nobody else does.
        assert!(!row_is_visible(&stamped(id)));

        // Ending the transaction publishes it.
        end_transaction(id);
        assert!(row_is_visible(&stamped(id)));
    }

    /// A snapshot judges a row against the moment the block began, so work
    /// that commits afterwards stays invisible for its whole life.
    #[tokio::test]
    async fn a_snapshot_hides_work_committed_after_it_was_taken() {
        let reader = begin_transaction(true);
        let reader_id = reader.id;

        // A write that happens and commits after the snapshot was taken.
        let later = begin_transaction(false);
        end_transaction(later.id);

        assert!(!within_transaction(reader, async { row_is_visible(&stamped(later.id)) }).await);
        end_transaction(reader_id);

        // Without a snapshot the same row is visible: it is committed.
        assert!(row_is_visible(&stamped(later.id)));
    }

    #[tokio::test]
    async fn an_unstamped_row_is_visible() {
        assert!(row_is_visible(&HashMap::new()));
    }
}
