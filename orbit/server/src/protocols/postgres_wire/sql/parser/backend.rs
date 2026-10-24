//! Selects which grammar decides whether a statement is valid SQL.
//!
//! The hand-rolled parser is the only one that produces Orbit's [`Statement`](super::Statement)
//! AST, so it always builds the tree. `libpg_query` (PostgreSQL's own grammar) is layered on top:
//!
//! | `ORBIT_SQL_PARSER` | behaviour |
//! |--------------------|-----------|
//! | `hand-rolled` (default) | the hand-rolled parser alone |
//! | `shadow` | both parse; disagreements are logged and counted, the hand-rolled result is used |
//! | `pg-query` | `libpg_query` gates: SQL PostgreSQL rejects is rejected here; accepted SQL is built by the hand-rolled parser |
//!
//! `shadow` and `pg-query` need the `pg-query-parser` cargo feature. Asking for them without it
//! falls back to `hand-rolled` with a warning rather than failing to start.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Which grammar is authoritative for accepting or rejecting a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParserBackend {
    HandRolled,
    Shadow,
    /// `libpg_query` parses and its tree is lowered to Orbit's AST; the hand-rolled parser is only
    /// the fallback for what is not lowered yet and for Orbit's own SQL extensions.
    PgQuery,
    /// As [`Self::PgQuery`], but a statement PostgreSQL's grammar rejects is rejected, extensions included.
    PgQueryStrict,
}

impl ParserBackend {
    /// Environment variable that selects the backend.
    pub const ENV: &'static str = "ORBIT_SQL_PARSER";

    /// The process-wide choice, read once from [`Self::ENV`].
    pub fn selected() -> Self {
        static SELECTED: OnceLock<ParserBackend> = OnceLock::new();
        *SELECTED.get_or_init(|| {
            let requested = std::env::var(Self::ENV)
                .map(|v| v.parse::<Self>().unwrap_or_else(|e: String| fallback(&e)))
                .unwrap_or(Self::HandRolled);
            if requested != Self::HandRolled && !pg_query_available() {
                tracing::warn!(
                    requested = ?requested,
                    "{} needs the `pg-query-parser` feature; using the hand-rolled parser",
                    Self::ENV
                );
                return Self::HandRolled;
            }
            requested
        })
    }

    /// In `shadow` mode, record whether the hand-rolled parser agreed with `libpg_query`.
    pub(super) fn compare(self, sql: &str, hand_rolled_accepted: bool) {
        if self != Self::Shadow {
            return;
        }
        let pg_accepted = pg_query_syntax(sql).is_ok();
        SHADOW_TOTAL.fetch_add(1, Ordering::Relaxed);
        if pg_accepted != hand_rolled_accepted {
            SHADOW_DISAGREEMENTS.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                pg_accepted,
                hand_rolled_accepted,
                sql,
                "SQL parser disagreement"
            );
        }
    }
}

impl std::str::FromStr for ParserBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hand-rolled" | "handrolled" => Ok(Self::HandRolled),
            "shadow" => Ok(Self::Shadow),
            "pg-query" | "pg_query" => Ok(Self::PgQuery),
            "pg-query-strict" | "pg_query_strict" => Ok(Self::PgQueryStrict),
            other => Err(format!("unknown {} value {other:?}", Self::ENV)),
        }
    }
}

fn fallback(reason: &str) -> ParserBackend {
    tracing::warn!("{reason}; using the hand-rolled parser");
    ParserBackend::HandRolled
}

static SHADOW_TOTAL: AtomicU64 = AtomicU64::new(0);
static SHADOW_DISAGREEMENTS: AtomicU64 = AtomicU64::new(0);

static LOWERED: AtomicU64 = AtomicU64::new(0);
static FELL_BACK: AtomicU64 = AtomicU64::new(0);

/// `(lowered from libpg_query, handled by the hand-rolled fallback)` since process start.
pub fn lowering_counts() -> (u64, u64) {
    (
        LOWERED.load(Ordering::Relaxed),
        FELL_BACK.load(Ordering::Relaxed),
    )
}

pub(super) fn count_lowered() {
    LOWERED.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn count_fallback() {
    FELL_BACK.fetch_add(1, Ordering::Relaxed);
}

/// `(statements compared, disagreements)` since process start, for `shadow` mode.
pub fn shadow_counts() -> (u64, u64) {
    (
        SHADOW_TOTAL.load(Ordering::Relaxed),
        SHADOW_DISAGREEMENTS.load(Ordering::Relaxed),
    )
}

/// Whether this build can ask `libpg_query` anything.
pub const fn pg_query_available() -> bool {
    cfg!(feature = "pg-query-parser")
}

/// Does PostgreSQL's own grammar accept `sql`? `None` when this build has no `libpg_query`.
pub fn pg_query_accepts(sql: &str) -> Option<bool> {
    pg_query_available().then(|| pg_query_syntax(sql).is_ok())
}

#[cfg(feature = "pg-query-parser")]
fn pg_query_syntax(sql: &str) -> Result<(), String> {
    pg_query::parse(sql, pg_query::ParserOptions::DEFAULT)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(feature = "pg-query-parser"))]
fn pg_query_syntax(_sql: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_documented_value() {
        for (text, want) in [
            ("hand-rolled", ParserBackend::HandRolled),
            ("shadow", ParserBackend::Shadow),
            ("pg-query", ParserBackend::PgQuery),
            ("PG_QUERY", ParserBackend::PgQuery),
        ] {
            assert_eq!(text.parse::<ParserBackend>(), Ok(want), "{text}");
        }
        assert!("antlr".parse::<ParserBackend>().is_err());
    }

    #[cfg(feature = "pg-query-parser")]
    #[test]
    fn libpg_query_rejects_what_postgres_rejects() {
        assert_eq!(pg_query_accepts("SELECT 1"), Some(true));
        assert_eq!(pg_query_accepts("SELEC 1"), Some(false));
    }

    /// Syntax that only PostgreSQL 18's grammar accepts; proves the linked `libpg_query` is 18.x.
    #[cfg(feature = "pg-query-parser")]
    #[test]
    fn linked_libpg_query_speaks_postgres_18() {
        for sql in [
            "INSERT INTO t VALUES (1) RETURNING OLD.a, NEW.a",
            "CREATE TABLE t (a int, b int GENERATED ALWAYS AS (a * 2) VIRTUAL)",
            "CREATE TABLE t (id int, valid daterange, PRIMARY KEY (id, valid WITHOUT OVERLAPS))",
        ] {
            assert_eq!(pg_query_accepts(sql), Some(true), "{sql}");
        }
    }
}
