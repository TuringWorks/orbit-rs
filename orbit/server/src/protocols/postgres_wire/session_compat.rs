//! Session-level statements issued by PostgreSQL drivers and GUI clients
//! (pgAdmin, psycopg, JDBC) while connecting.
//!
//! These never touch table data, so they are answered here, before the SQL
//! parser, from a fixed table of server settings. Everything is pure: input
//! text in, optional result out.

use super::query_engine::QueryResult;

/// Server settings reported to clients, as `(name, value)`.
const SETTINGS: &[(&str, &str)] = &[
    ("server_version", "14.0"),
    ("server_version_num", "140000"),
    ("server_encoding", "UTF8"),
    ("client_encoding", "UTF8"),
    ("datestyle", "ISO, MDY"),
    ("intervalstyle", "postgres"),
    ("timezone", "UTC"),
    ("standard_conforming_strings", "on"),
    ("integer_datetimes", "on"),
    ("bytea_output", "hex"),
    ("max_identifier_length", "63"),
    ("transaction_isolation", "read committed"),
    ("is_superuser", "on"),
    ("application_name", ""),
    ("search_path", "\"$user\", public"),
];

/// Client-tunable settings whose `SET`/`RESET` is accepted and ignored.
const SETTABLE: &[&str] = &[
    "datestyle",
    "intervalstyle",
    "timezone",
    "time zone",
    "client_encoding",
    "client_min_messages",
    "extra_float_digits",
    "application_name",
    "statement_timeout",
    "lock_timeout",
    "idle_in_transaction_session_timeout",
    "bytea_output",
    "search_path",
    "standard_conforming_strings",
];

/// Stable synthetic OIDs for the current database and role.
const DATABASE_OID: &str = "5";
const ROLE_OID: &str = "10";

/// Value reported by `SELECT version()`.
pub const VERSION_STRING: &str = "PostgreSQL 14.0 (Orbit-RS) on orbit, compiled by rustc, 64-bit";

/// Split `sql` on top-level `;`, ignoring semicolons inside quotes.
/// Empty fragments are dropped.
pub fn split_statements(sql: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut start, mut quote) = (0, None::<char>);
    for (i, c) in sql.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '\'' | '"') => quote = Some(c),
            (None, ';') => {
                parts.push(&sql[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&sql[start..]);
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

fn setting(name: &str) -> Option<&'static str> {
    SETTINGS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| *v)
}

fn one_value(column: &str, value: &str) -> QueryResult {
    QueryResult::Select {
        columns: vec![column.to_string()],
        rows: vec![vec![Some(value.to_string())]],
    }
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches(|c| c == '\'' || c == '"')
}

/// Answer `stmt` (a single statement, no trailing `;`) if it is a session
/// statement. `database` and `user` fill `current_database()` / `current_user`.
#[must_use]
pub fn try_handle(stmt: &str, database: &str, user: &str) -> Option<QueryResult> {
    let norm = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = norm.to_ascii_lowercase();

    if let Some(rest) = lower
        .strip_prefix("set ")
        .or_else(|| lower.strip_prefix("reset "))
    {
        let rest = rest
            .strip_prefix("session ")
            .or_else(|| rest.strip_prefix("local "))
            .unwrap_or(rest);
        let name = rest.split(['=', ' ']).next().unwrap_or_default();
        let is_time_zone = rest.starts_with("time zone");
        if SETTABLE.contains(&name) || is_time_zone {
            let value = rest.split_once(['=', ' ']).map_or("", |(_, v)| v);
            return Some(QueryResult::Set {
                variable: name.to_string(),
                value: unquote(value.trim_start_matches("to ")).to_string(),
            });
        }
        return None;
    }

    if let Some(name) = lower.strip_prefix("show ") {
        let name = unquote(name);
        return setting(name).map(|v| one_value(name, v));
    }

    if let Some(result) = dashboard_probe(&lower) {
        return Some(result);
    }
    if let Some(result) = catalog_probe(&lower, database, user) {
        return Some(result);
    }

    // pgAdmin's replication probe: standalone server, so neither BDR/PGD nor slots.
    if lower.contains("pg_replication_slots") && lower.contains("pg_extension") {
        return Some(QueryResult::Select {
            columns: vec!["type".to_string()],
            rows: vec![vec![None]],
        });
    }

    let select = lower.strip_prefix("select ")?;
    match select {
        "version()" | "pg_catalog.version()" => Some(one_value("version", VERSION_STRING)),
        "current_database()" => Some(one_value("current_database", database)),
        "current_schema()" | "current_schema" => Some(one_value("current_schema", "public")),
        "current_user" | "session_user" | "user" => Some(one_value("current_user", user)),
        _ => select_setting_function(select, &norm[7..]),
    }
}

fn typed(columns: &[(&str, i32)], row: Vec<&str>) -> QueryResult {
    QueryResult::Typed {
        columns: columns.iter().map(|(n, t)| (n.to_string(), *t)).collect(),
        rows: vec![row.into_iter().map(|v| Some(v.to_string())).collect()],
    }
}

/// Dashboard charts as `(chart name, JSON keys)`, in the order pgAdmin unions them.
const DASHBOARD_CHARTS: &[(&str, &[&str])] = &[
    ("session_stats", &["Total", "Active", "Idle"]),
    ("tps_stats", &["Transactions", "Commits", "Rollbacks"]),
    ("ti_stats", &["Inserts", "Updates", "Deletes"]),
    ("to_stats", &["Fetched", "Returned"]),
    ("bio_stats", &["Reads", "Hits"]),
];

const ACTIVITY_COLUMNS: &[&str] = &[
    "pid",
    "datname",
    "usename",
    "application_name",
    "client_addr",
    "backend_start",
    "state",
    "wait_event",
    "blocking_pids",
    "query",
    "state_change",
    "query_start",
    "xact_start",
    "backend_type",
    "active_since",
];
const LOCKS_COLUMNS: &[&str] = &[
    "pid",
    "locktype",
    "datname",
    "relation",
    "page",
    "tuple",
    "transactionid",
    "classid",
    "objid",
    "objsubid",
    "virtualtransaction",
    "mode",
    "granted",
    "fastpath",
];
const PREPARED_COLUMNS: &[&str] = &["gid", "database", "owner", "transaction", "prepared"];

fn empty_text_result(columns: &[&str]) -> QueryResult {
    QueryResult::Typed {
        columns: columns
            .iter()
            .map(|name| (name.to_string(), super::messages::type_oids::TEXT))
            .collect(),
        rows: Vec::new(),
    }
}

/// pgAdmin's dashboard (`/*pga4dash*/` queries). The server keeps no activity or
/// lock statistics, so the charts report zero and the tables are empty.
fn dashboard_probe(lower: &str) -> Option<QueryResult> {
    use super::messages::type_oids::{BOOL, JSON, TEXT};
    if !lower.starts_with("/*pga4dash*/") {
        if lower.contains("pg_is_in_recovery()") && lower.contains("pg_catalog.pg_user") {
            return Some(typed(
                &[("inrecovery", BOOL), ("isreplaypaused", BOOL)],
                vec!["f", "f"],
            ));
        }
        return None;
    }
    let charts: Vec<_> = DASHBOARD_CHARTS
        .iter()
        .filter(|(name, _)| lower.contains(&format!("'{name}' as chart_name")))
        .collect();
    if !charts.is_empty() {
        let rows = charts
            .iter()
            .map(|(name, keys)| {
                let body = keys
                    .iter()
                    .map(|k| format!("\"{k}\": 0"))
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![Some(name.to_string()), Some(format!("{{{body}}}"))]
            })
            .collect();
        return Some(QueryResult::Typed {
            columns: vec![
                ("chart_name".to_string(), TEXT),
                ("chart_data".to_string(), JSON),
            ],
            rows,
        });
    }
    let columns = if lower.contains("pg_stat_activity") {
        ACTIVITY_COLUMNS
    } else if lower.contains("pg_locks") {
        LOCKS_COLUMNS
    } else if lower.contains("pg_prepared_xacts") {
        PREPARED_COLUMNS
    } else {
        return None;
    };
    Some(empty_text_result(columns))
}

/// The two catalog lookups pgAdmin makes right after connecting. Their result
/// shape is fixed, so booleans must go out as `bool` (pgAdmin tests truthiness,
/// and the string `'f'` is truthy).
fn catalog_probe(lower: &str, database: &str, user: &str) -> Option<QueryResult> {
    use super::messages::type_oids::{BOOL, INT4, TEXT};
    if lower.contains("pg_catalog.pg_database") && lower.contains("= current_database()") {
        return Some(typed(
            &[
                ("did", INT4),
                ("datname", TEXT),
                ("datallowconn", BOOL),
                ("serverencoding", TEXT),
                ("cancreate", BOOL),
                ("datistemplate", BOOL),
            ],
            vec![DATABASE_OID, database, "t", "UTF8", "t", "f"],
        ));
    }
    // pgAdmin closes its cursor when this errors and then skips the roles query.
    if lower.contains("pg_stat_gssapi") {
        return Some(typed(
            &[("gss_authenticated", BOOL), ("encrypted", BOOL)],
            vec!["f", "f"],
        ));
    }
    if lower.contains("pg_catalog.pg_roles") && lower.contains("rolname = current_user") {
        return Some(typed(
            &[
                ("id", INT4),
                ("name", TEXT),
                ("is_superuser", BOOL),
                ("can_create_role", BOOL),
                ("can_create_db", BOOL),
                ("can_signal_backend", BOOL),
            ],
            vec![ROLE_OID, user, "t", "t", "t", "t"],
        ));
    }
    None
}

/// `current_setting('x')` and the `set_config(..) FROM pg_show_all_settings()`
/// probe that pgAdmin runs on connect.
fn select_setting_function(select_lower: &str, select_raw: &str) -> Option<QueryResult> {
    if let Some(arg) = select_lower
        .strip_prefix("current_setting(")
        .and_then(|a| a.strip_suffix(')'))
    {
        let name = unquote(arg);
        return setting(name).map(|v| one_value("current_setting", v));
    }
    if select_lower.starts_with("set_config(") {
        let args = select_raw
            .strip_prefix("set_config(")
            .and_then(|a| a.split(')').next())?;
        let mut args = args.split(',');
        let name = unquote(args.next()?);
        let value = unquote(args.next().unwrap_or_default());
        // With `FROM pg_show_all_settings() WHERE name = ..` the row exists only
        // for a known setting; a bare call always yields one row.
        let known = !select_lower.contains("pg_show_all_settings") || setting(name).is_some();
        return Some(if known {
            one_value("set_config", value)
        } else {
            QueryResult::Select {
                columns: vec!["set_config".to_string()],
                rows: Vec::new(),
            }
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(r: Option<QueryResult>) -> Option<String> {
        match r? {
            QueryResult::Select { rows, .. } => rows.first()?.first()?.clone(),
            _ => None,
        }
    }

    #[test]
    fn splits_on_top_level_semicolons_only() {
        let parts = split_statements("SET a=1; SELECT ';' ; ;SET b=2");
        assert_eq!(parts, vec!["SET a=1", "SELECT ';'", "SET b=2"]);
    }

    #[test]
    fn accepts_driver_set_statements() {
        for sql in [
            "SET DateStyle=ISO",
            "SET client_min_messages=notice",
            "SET client_encoding='UNICODE'",
            "SET TIME ZONE 'UTC'",
            "RESET statement_timeout",
        ] {
            assert!(
                matches!(try_handle(sql, "db", "u"), Some(QueryResult::Set { .. })),
                "{sql}"
            );
        }
        assert!(try_handle("SET orbit.custom = 1", "db", "u").is_none());
    }

    #[test]
    fn answers_show_and_functions() {
        assert_eq!(
            single(try_handle("SHOW server_version_num", "d", "u")).as_deref(),
            Some("140000")
        );
        assert_eq!(
            single(try_handle("select current_database()", "mydb", "u")).as_deref(),
            Some("mydb")
        );
        assert_eq!(
            single(try_handle(
                "SELECT current_setting('server_version_num')",
                "d",
                "u"
            ))
            .as_deref(),
            Some("140000")
        );
        assert_eq!(
            single(try_handle("SELECT version()", "d", "u")).as_deref(),
            Some(VERSION_STRING)
        );
        assert!(try_handle("SELECT * FROM actors", "d", "u").is_none());
    }

    #[test]
    fn pgadmin_replication_probe_is_null() {
        let q = "SELECT CASE\n WHEN (SELECT count(extname) FROM pg_catalog.pg_extension WHERE extname='bdr') > 0\n THEN 'pgd'\n WHEN (SELECT COUNT(*) FROM pg_catalog.pg_replication_slots) > 0\n THEN 'log'\n ELSE NULL\nEND as type";
        match try_handle(q, "d", "u") {
            Some(QueryResult::Select { rows, .. }) => assert_eq!(rows, vec![vec![None]]),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn pgadmin_catalog_probes_are_typed() {
        let db = "SELECT db.oid as did, db.datname FROM pg_catalog.pg_database db WHERE db.datname = current_database()";
        let role =
            "SELECT roles.oid as id FROM pg_catalog.pg_roles as roles WHERE rolname = current_user";
        let gss = "SELECT gss_authenticated, encrypted FROM pg_catalog.pg_stat_gssapi WHERE pid = pg_backend_pid()";
        for q in [db, role, gss] {
            match try_handle(q, "mydb", "me") {
                Some(QueryResult::Typed { columns, rows }) => {
                    assert_eq!(rows.len(), 1);
                    assert!(columns
                        .iter()
                        .any(|(_, t)| *t == super::super::messages::type_oids::BOOL));
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn pgadmin_dashboard_probes() {
        let charts = "/*pga4dash*/\nSELECT 'session_stats' AS chart_name, row_to_json(t) AS chart_data FROM (SELECT 1) t\nUNION ALL\nSELECT 'tps_stats' AS chart_name, row_to_json(t) AS chart_data FROM (SELECT 1) t";
        match try_handle(charts, "d", "u") {
            Some(QueryResult::Typed { rows, .. }) => {
                let names: Vec<_> = rows
                    .iter()
                    .map(|r| r[0].clone().unwrap_or_default())
                    .collect();
                assert_eq!(names, ["session_stats", "tps_stats"]);
                assert_eq!(
                    rows[0][1].as_deref(),
                    Some("{\"Total\": 0, \"Active\": 0, \"Idle\": 0}")
                );
            }
            other => panic!("unexpected {other:?}"),
        }
        let sessions =
            "/*pga4dash*/\nSELECT pid, datname FROM pg_catalog.pg_stat_activity ORDER BY pid";
        match try_handle(sessions, "d", "u") {
            Some(QueryResult::Typed { columns, rows }) => {
                assert_eq!(columns.len(), ACTIVITY_COLUMNS.len());
                assert!(rows.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
        let recovery = "SELECT CASE WHEN usesuper THEN pg_catalog.pg_is_in_recovery() ELSE FALSE END as inrecovery FROM pg_catalog.pg_user WHERE usename=current_user";
        assert!(matches!(
            try_handle(recovery, "d", "u"),
            Some(QueryResult::Typed { .. })
        ));
    }

    #[test]
    fn pgadmin_bytea_output_probe() {
        let q = "SELECT set_config('bytea_output','hex',false) FROM pg_show_all_settings() WHERE name = 'bytea_output'";
        assert_eq!(single(try_handle(q, "d", "u")).as_deref(), Some("hex"));
    }
}
