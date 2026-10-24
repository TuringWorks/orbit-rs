//! Per-connection settings (`SET`, `SHOW`, `RESET`, `set_config`, `current_setting`) and the
//! connection's identity (`current_user`, `current_database()`).
//!
//! A connection's settings live in its [`SessionVars`]. Statements run on the engine's snapshot, so
//! while one executes the settings are made visible to it through a thread-local that is set only
//! around the synchronous part of the statement (never across an `await`).

use super::{err, QueryResult, R};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Settings the server knows, with their defaults. A name not listed (and without a `.`) is
/// `unrecognized configuration parameter`. `server_*`-style entries cannot be changed.
const DEFAULTS: &[(&str, &str)] = &[
    ("server_version", "18.0"),
    ("server_version_num", "180000"),
    ("server_encoding", "UTF8"),
    ("client_encoding", "UTF8"),
    ("datestyle", "ISO, MDY"),
    ("intervalstyle", "postgres"),
    ("timezone", "UTC"),
    ("standard_conforming_strings", "on"),
    ("integer_datetimes", "on"),
    ("bytea_output", "hex"),
    ("max_identifier_length", "63"),
    ("max_connections", "100"),
    ("is_superuser", "on"),
    ("application_name", ""),
    ("search_path", "\"$user\", public"),
    ("statement_timeout", "0"),
    ("lock_timeout", "0"),
    ("idle_in_transaction_session_timeout", "0"),
    ("client_min_messages", "notice"),
    ("extra_float_digits", "1"),
    ("default_transaction_isolation", "read committed"),
    ("default_transaction_read_only", "off"),
    ("transaction_isolation", "read committed"),
    ("transaction_read_only", "off"),
    ("work_mem", "4MB"),
    ("lc_messages", "en_US.UTF-8"),
    ("lc_monetary", "en_US.UTF-8"),
    ("lc_numeric", "en_US.UTF-8"),
    ("lc_time", "en_US.UTF-8"),
    ("row_security", "on"),
    ("session_replication_role", "origin"),
];

/// Settings a client may not change.
const READ_ONLY: &[&str] = &[
    "server_version",
    "server_version_num",
    "server_encoding",
    "integer_datetimes",
    "max_identifier_length",
    "max_connections",
    "is_superuser",
];

/// One connection's settings and identity.
#[derive(Clone, Debug)]
pub struct SessionVars {
    user: String,
    database: String,
    values: BTreeMap<String, String>,
    /// `SET LOCAL` values, dropped when the transaction ends.
    local: BTreeMap<String, String>,
    /// The values as the open transaction began, restored if it rolls back.
    at_begin: Option<BTreeMap<String, String>>,
    /// `SET TRANSACTION ISOLATION LEVEL` / `BEGIN ISOLATION LEVEL` for the open transaction.
    txn_isolation: Option<String>,
    txn_read_only: bool,
}

impl Default for SessionVars {
    fn default() -> Self {
        Self::new(DEFAULT_IDENTITY, DEFAULT_IDENTITY)
    }
}

/// The user and database name when nothing says otherwise (an engine used without a connection).
const DEFAULT_IDENTITY: &str = "orbit";

/// The running statement's user, or the default.
pub(crate) fn current_user() -> String {
    user().unwrap_or_else(|| DEFAULT_IDENTITY.to_string())
}

/// The running statement's database, or the default.
pub(crate) fn current_database() -> String {
    database().unwrap_or_else(|| DEFAULT_IDENTITY.to_string())
}

impl SessionVars {
    pub fn new(user: &str, database: &str) -> Self {
        Self {
            user: user.to_string(),
            database: database.to_string(),
            values: BTreeMap::new(),
            local: BTreeMap::new(),
            at_begin: None,
            txn_isolation: None,
            txn_read_only: false,
        }
    }

    /// Record who connected and to which database (from the startup message).
    pub fn set_identity(&mut self, user: Option<&str>, database: Option<&str>) {
        if let Some(u) = user {
            self.user = u.to_string();
            // PostgreSQL connects to a database named after the user when none is given
            if database.is_none() {
                self.database = u.to_string();
            }
        }
        if let Some(d) = database {
            self.database = d.to_string();
        }
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    fn canonical(name: &str) -> String {
        match name.trim().to_ascii_lowercase().as_str() {
            "time zone" => "timezone".to_string(),
            other => other.to_string(),
        }
    }

    fn default_of(name: &str) -> Option<&'static str> {
        DEFAULTS.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }

    /// The value of a setting, `None` if it does not exist.
    pub fn get(&self, name: &str) -> Option<String> {
        let name = Self::canonical(name);
        match name.as_str() {
            "transaction_isolation" => {
                return Some(self.txn_isolation.clone().unwrap_or_else(|| {
                    self.get("default_transaction_isolation")
                        .unwrap_or_else(|| "read committed".to_string())
                }))
            }
            "transaction_read_only" if self.at_begin.is_some() => {
                return Some(if self.txn_read_only { "on" } else { "off" }.to_string())
            }
            _ => {}
        }
        self.local
            .get(&name)
            .or_else(|| self.values.get(&name))
            .cloned()
            .or_else(|| Self::default_of(&name).map(str::to_string))
    }

    /// Change a setting. `local` limits it to the open transaction.
    pub(crate) fn set(&mut self, name: &str, value: &str, local: bool) -> R<()> {
        let name = Self::canonical(name);
        if READ_ONLY.contains(&name.as_str()) {
            return err("55P02", format!("parameter \"{name}\" cannot be changed"));
        }
        if Self::default_of(&name).is_none() && !name.contains('.') {
            return err(
                "42704",
                format!("unrecognized configuration parameter \"{name}\""),
            );
        }
        let value = Self::normalise(&name, value);
        if local && self.at_begin.is_some() {
            self.local.insert(name, value);
        } else {
            self.values.insert(name, value);
        }
        Ok(())
    }

    /// `on`/`off` spellings of boolean settings, and the isolation level in lower case.
    fn normalise(name: &str, value: &str) -> String {
        let lower = value.trim().to_ascii_lowercase();
        match name {
            "standard_conforming_strings" | "default_transaction_read_only" | "row_security" => {
                match lower.as_str() {
                    "true" | "t" | "yes" | "y" | "1" | "on" => "on".to_string(),
                    "false" | "f" | "no" | "n" | "0" | "off" => "off".to_string(),
                    _ => lower,
                }
            }
            "default_transaction_isolation" | "transaction_isolation" => lower,
            _ => value.trim().to_string(),
        }
    }

    pub(crate) fn reset(&mut self, name: &str) -> R<()> {
        let name = Self::canonical(name);
        if Self::default_of(&name).is_none() && !name.contains('.') {
            return err(
                "42704",
                format!("unrecognized configuration parameter \"{name}\""),
            );
        }
        self.values.remove(&name);
        self.local.remove(&name);
        Ok(())
    }

    pub fn reset_all(&mut self) {
        self.values.clear();
        self.local.clear();
    }

    /// Every setting with its current value, sorted by name.
    pub fn all(&self) -> Vec<(String, String)> {
        let mut names: Vec<String> = DEFAULTS.iter().map(|(n, _)| n.to_string()).collect();
        names.extend(self.values.keys().chain(self.local.keys()).cloned());
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter_map(|n| self.get(&n).map(|v| (n, v)))
            .collect()
    }

    /// A transaction starts: remember the settings so a rollback can restore them.
    pub fn begin_txn(&mut self, isolation: Option<&str>, read_only: bool) {
        if self.at_begin.is_none() {
            self.at_begin = Some(self.values.clone());
        }
        self.txn_isolation = isolation.map(str::to_string);
        self.txn_read_only = read_only;
    }

    pub fn set_txn_isolation(&mut self, level: &str) {
        self.txn_isolation = Some(level.to_ascii_lowercase());
    }

    pub fn in_txn(&self) -> bool {
        self.at_begin.is_some()
    }

    /// The transaction ended: `SET LOCAL` is forgotten, and on rollback so is every `SET` made inside.
    pub fn end_txn(&mut self, committed: bool) {
        self.local.clear();
        if let Some(saved) = self.at_begin.take() {
            if !committed {
                self.values = saved;
            }
        }
        self.txn_isolation = None;
        self.txn_read_only = false;
    }

    /// Whether the open transaction was started `READ ONLY`.
    pub fn txn_read_only(&self) -> bool {
        self.txn_read_only
    }
}

thread_local! {
    static CURRENT: RefCell<Option<SessionVars>> = const { RefCell::new(None) };
}

/// Run `f` with `vars` visible to [`current`] and friends. Settings changed during `f` (by
/// `set_config`) are written back. `f` must not await.
pub(super) fn scoped<T>(vars: &mut SessionVars, f: impl FnOnce() -> T) -> T {
    CURRENT.with(|c| *c.borrow_mut() = Some(std::mem::take(vars)));
    let out = f();
    CURRENT.with(|c| {
        if let Some(back) = c.borrow_mut().take() {
            *vars = back;
        }
    });
    out
}

/// A setting as the running statement sees it.
pub(crate) fn lookup(name: &str) -> Option<String> {
    CURRENT.with(|c| c.borrow().as_ref().and_then(|v| v.get(name)))
}

/// The connection's user, if a statement is running for one.
pub(crate) fn user() -> Option<String> {
    CURRENT.with(|c| c.borrow().as_ref().map(|v| v.user.clone()))
}

pub(crate) fn database() -> Option<String> {
    CURRENT.with(|c| c.borrow().as_ref().map(|v| v.database.clone()))
}

/// `set_config(name, value, is_local)` from inside a statement; returns the new value.
pub(crate) fn set_config(name: &str, value: &str, local: bool) -> R<String> {
    CURRENT.with(|c| match c.borrow_mut().as_mut() {
        Some(vars) => {
            vars.set(name, value, local)?;
            Ok(vars.get(name).unwrap_or_default())
        }
        None => err("0A000", "set_config needs a session"),
    })
}

fn unquote(word: &str) -> String {
    let w = word.trim().trim_end_matches(';').trim();
    if w.len() >= 2
        && ((w.starts_with('\'') && w.ends_with('\'')) || (w.starts_with('"') && w.ends_with('"')))
    {
        w[1..w.len() - 1].replace("''", "'")
    } else {
        w.to_string()
    }
}

/// The value part of `SET name TO a, b` / `SET name = 'x'`: comma-separated items joined with `, `.
fn setting_value(rest: &str) -> String {
    rest.split(',').map(unquote).collect::<Vec<_>>().join(", ")
}

fn one_row(column: &str, value: String) -> QueryResult {
    QueryResult::Select {
        columns: vec![column.to_string()],
        rows: vec![vec![Some(value)]],
    }
}

/// Handle `SET`, `RESET`, `SHOW`, `DISCARD ALL` and `SELECT set_config(..)` against `vars`.
/// `None` when `statement` is something else.
pub(crate) fn handle_command(vars: &mut SessionVars, statement: &str) -> Option<R<QueryResult>> {
    let text = statement.trim().trim_end_matches(';').trim();
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = collapsed.to_ascii_lowercase();
    let words: Vec<&str> = collapsed.split(' ').collect();
    match words.first().map(|w| w.to_ascii_lowercase()).as_deref() {
        Some("show") => Some(show(vars, collapsed[5..].trim())),
        Some("reset") => {
            let name = collapsed[6..].trim();
            Some(if name.eq_ignore_ascii_case("all") {
                vars.reset_all();
                Ok(set_ok("all", ""))
            } else {
                vars.reset(&unquote(name)).map(|()| set_ok(name, ""))
            })
        }
        Some("discard") if lower == "discard all" => {
            vars.reset_all();
            Some(Ok(QueryResult::Command {
                tag: "DISCARD ALL".into(),
            }))
        }
        Some("set") => set(vars, &collapsed, &lower),
        Some("select") => select_set_config(vars, &collapsed, &lower),
        _ => None,
    }
}

fn set_ok(variable: &str, value: &str) -> QueryResult {
    QueryResult::Set {
        variable: variable.to_ascii_lowercase(),
        value: value.to_string(),
    }
}

fn show(vars: &SessionVars, name: &str) -> R<QueryResult> {
    let name = unquote(name);
    if name.eq_ignore_ascii_case("all") {
        let all = vars.all();
        return Ok(QueryResult::Select {
            columns: vec!["name".into(), "setting".into(), "description".into()],
            rows: all
                .into_iter()
                .map(|(n, v)| vec![Some(n), Some(v), Some(String::new())])
                .collect(),
        });
    }
    match vars.get(&name) {
        Some(v) => Ok(one_row(&name, v)),
        None => err(
            "42704",
            format!("unrecognized configuration parameter \"{name}\""),
        ),
    }
}

fn set(vars: &mut SessionVars, original: &str, lower: &str) -> Option<R<QueryResult>> {
    // SET TRANSACTION ISOLATION LEVEL x / SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL x
    if let Some(pos) = lower.find("isolation level ") {
        let level = lower[pos + "isolation level ".len()..].trim().to_string();
        if lower.starts_with("set session characteristics") {
            return Some(
                vars.set("default_transaction_isolation", &level, false)
                    .map(|()| set_ok("default_transaction_isolation", &level)),
            );
        }
        if lower.starts_with("set transaction") {
            vars.set_txn_isolation(&level);
            return Some(Ok(set_ok("transaction_isolation", &level)));
        }
    }
    let rest = original[3..].trim();
    let (local, rest) = if let Some(r) = strip_word(rest, "local") {
        (true, r)
    } else if let Some(r) = strip_word(rest, "session") {
        (false, r)
    } else {
        (false, rest)
    };
    if let Some(value) = strip_word(rest, "time zone") {
        let value = setting_value(value);
        return Some(
            vars.set("timezone", &value, local)
                .map(|()| set_ok("timezone", &value)),
        );
    }
    // role and authorization changes are accepted; the connection keeps its identity
    if strip_word(rest, "role").is_some()
        || strip_word(rest, "authorization").is_some()
        || rest.to_ascii_lowercase().starts_with("names ")
    {
        return Some(Ok(set_ok("role", "")));
    }
    let split = rest.find(|c: char| c == '=' || c.is_whitespace())?;
    let name = &rest[..split];
    let tail = rest[split..].trim_start();
    let tail = tail
        .strip_prefix('=')
        .or_else(|| strip_word(tail, "to"))
        .unwrap_or(tail)
        .trim();
    Some(if tail.eq_ignore_ascii_case("default") {
        vars.reset(name).map(|()| set_ok(name, ""))
    } else {
        let value = setting_value(tail);
        vars.set(name, &value, local).map(|()| set_ok(name, &value))
    })
}

/// `text` without a leading `word` (case-insensitive, followed by a space or the end).
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let head = text.get(..word.len())?;
    let rest = &text[word.len()..];
    (head.eq_ignore_ascii_case(word) && (rest.is_empty() || rest.starts_with(' ')))
        .then(|| rest.trim_start())
}

fn select_set_config(
    vars: &mut SessionVars,
    original: &str,
    lower: &str,
) -> Option<R<QueryResult>> {
    let body = lower.strip_prefix("select set_config(")?;
    let args_end = body.rfind(')')?;
    let start = "select set_config(".len();
    let inner = &original[start..start + args_end];
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let [name, value, is_local] = parts.as_slice() else {
        return Some(err("42883", "function set_config requires 3 arguments"));
    };
    let local = matches!(
        is_local.to_ascii_lowercase().as_str(),
        "true" | "t" | "'t'" | "'true'"
    );
    let (name, value) = (unquote(name), unquote(value));
    Some(
        vars.set(&name, &value, local)
            .map(|()| one_row("set_config", vars.get(&name).unwrap_or_default())),
    )
}

/// When this process started, for `pg_postmaster_start_time()`.
pub(crate) fn process_start() -> std::time::SystemTime {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::SystemTime> = OnceLock::new();
    *START.get_or_init(std::time::SystemTime::now)
}
