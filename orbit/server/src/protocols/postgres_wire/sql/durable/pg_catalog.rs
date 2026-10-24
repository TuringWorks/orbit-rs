//! The `pg_catalog` views and functions that database tools read to list databases, schemas, tables,
//! columns, indexes and constraints.
//!
//! Every view is computed from the committed [`State`] on each use, so it always matches the tables.
//! Object ids are derived from table ids: a table is `16384 + id`, its indexes, constraints and
//! sequences get ids in separate ranges, and the built-in objects use PostgreSQL's own ids.

use super::query::{Rel, RelCol};
use super::{err, Col, Row, State, Table, R};
use crate::protocols::postgres_wire::sql::ast::{Expression, FunctionCall, FunctionName};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use std::sync::OnceLock;

const NS_CATALOG: i32 = 11;
const NS_PUBLIC: i32 = 2200;
const NS_INFORMATION_SCHEMA: i32 = 13000;
const OWNER: i32 = 10;
const DATABASE_OID: i32 = 5;
const TABLE_BASE: i32 = 16384;

/// The views, with the object id of the relation each one is listed as in `pg_class`.
const VIEWS: &[(&str, i32)] = &[
    ("pg_class", 1259),
    ("pg_attribute", 1249),
    ("pg_type", 1247),
    ("pg_proc", 1255),
    ("pg_namespace", 2615),
    ("pg_database", 1262),
    ("pg_index", 2610),
    ("pg_constraint", 2606),
    ("pg_attrdef", 2604),
    ("pg_description", 2609),
    ("pg_shdescription", 2396),
    ("pg_roles", 12000),
    ("pg_user", 12001),
    ("pg_auth_members", 1261),
    ("pg_tablespace", 1213),
    ("pg_tables", 12005),
    ("pg_indexes", 12006),
    ("pg_settings", 12002),
    ("pg_stat_activity", 12003),
    ("pg_stat_gssapi", 12004),
    ("pg_extension", 3079),
    ("pg_trigger", 2620),
    ("pg_inherits", 2611),
    ("pg_depend", 2608),
    ("pg_am", 2601),
];

/// The tables `information_schema` offers, listed in `pg_class` so tools recognise the schema.
const INFORMATION_SCHEMA_VIEWS: &[(&str, i32)] = &[
    ("schemata", 13001),
    ("tables", 13002),
    ("views", 13003),
    ("columns", 13004),
    ("table_constraints", 13005),
    ("key_column_usage", 13006),
    ("constraint_column_usage", 13007),
    ("constraint_table_usage", 13008),
    ("referential_constraints", 13009),
    ("check_constraints", 13010),
    ("sequences", 13011),
    ("table_privileges", 13012),
    ("role_table_grants", 13013),
    ("column_privileges", 13014),
    ("enabled_roles", 13015),
    ("applicable_roles", 13016),
    ("character_sets", 13017),
    ("routines", 13018),
    ("parameters", 13019),
    ("triggers", 13020),
    ("domains", 13021),
    ("collations", 13022),
    ("element_types", 13023),
];

/// The built-in types: oid, name, length, category.
const TYPES: &[(i32, &str, i32, &str)] = &[
    (16, "bool", 1, "B"),
    (17, "bytea", -1, "U"),
    (19, "name", 64, "S"),
    (20, "int8", 8, "N"),
    (21, "int2", 2, "N"),
    (23, "int4", 4, "N"),
    (25, "text", -1, "S"),
    (26, "oid", 4, "N"),
    (114, "json", -1, "U"),
    (700, "float4", 4, "N"),
    (701, "float8", 8, "N"),
    (1042, "bpchar", -1, "S"),
    (1043, "varchar", -1, "S"),
    (1082, "date", 4, "D"),
    (1083, "time", 8, "D"),
    (1114, "timestamp", 8, "D"),
    (1184, "timestamptz", 8, "D"),
    (1186, "interval", 16, "T"),
    (1700, "numeric", -1, "N"),
    (2950, "uuid", 16, "U"),
    (3802, "jsonb", -1, "U"),
];

/// The session-level settings listed in `pg_settings`.
const SETTINGS: &[(&str, &str)] = &[
    ("server_version", "14.0"),
    ("server_version_num", "140000"),
    ("max_connections", "100"),
    ("TimeZone", "UTC"),
    ("client_encoding", "UTF8"),
    ("server_encoding", "UTF8"),
    ("DateStyle", "ISO, MDY"),
    ("standard_conforming_strings", "on"),
    ("bytea_output", "hex"),
    ("integer_datetimes", "on"),
    ("search_path", "public"),
    ("default_transaction_isolation", "read committed"),
];

const FUNCTIONS: &[&str] = &[
    "pg_encoding_to_char",
    "has_database_privilege",
    "has_schema_privilege",
    "has_table_privilege",
    "has_column_privilege",
    "has_function_privilege",
    "has_sequence_privilege",
    "has_type_privilege",
    "pg_has_role",
    "pg_backend_pid",
    "pg_postmaster_start_time",
    "pg_size_pretty",
    "pg_database_size",
    "pg_total_relation_size",
    "pg_relation_size",
    "pg_table_size",
    "pg_indexes_size",
    "format_type",
    "pg_get_expr",
    "pg_get_indexdef",
    "pg_get_constraintdef",
    "pg_get_userbyid",
    "pg_get_serial_sequence",
    "obj_description",
    "col_description",
    "shobj_description",
    "pg_table_is_visible",
    "pg_type_is_visible",
    "pg_function_is_visible",
    "to_regclass",
];

fn fname(f: &FunctionCall) -> String {
    match &f.name {
        FunctionName::Simple(s) => s.to_lowercase(),
        FunctionName::Qualified { name, .. } => name.to_lowercase(),
    }
}

/// Whether `f` is one of the catalog functions answered here.
pub(super) fn is_function(f: &FunctionCall) -> bool {
    FUNCTIONS.contains(&fname(f).as_str())
}

/// The names of the `pg_catalog` views.
pub(super) fn view_names() -> impl Iterator<Item = &'static str> {
    VIEWS.iter().map(|(n, _)| *n)
}

/// The names of the `information_schema` views listed in `pg_class`.
pub(super) fn information_schema_names() -> impl Iterator<Item = &'static str> {
    INFORMATION_SCHEMA_VIEWS.iter().map(|(n, _)| *n)
}

/// Whether `name` is a `pg_catalog` view.
pub(super) fn is_view(name: &str) -> bool {
    VIEWS.iter().any(|(n, _)| *n == name)
}

/// The types a cast resolves through the catalog (names and numbers to object ids).
pub(super) fn is_reg_type(t: &SqlType) -> bool {
    match t {
        SqlType::Oid | SqlType::Regclass => true,
        SqlType::Custom { type_name } => matches!(
            type_name.to_lowercase().as_str(),
            "oid" | "regclass" | "regtype" | "regproc" | "regnamespace" | "regrole"
        ),
        _ => false,
    }
}

/// Words that name a session value when written without parentheses.
pub(super) fn session_keyword(name: &str) -> Option<String> {
    use super::session_vars::{current_database, current_user};
    match name.to_lowercase().as_str() {
        "current_user" | "session_user" | "user" => Some(current_user()),
        "current_schema" => Some("public".to_string()),
        "current_catalog" => Some(current_database()),
        _ => None,
    }
}

fn table_oid(t: &Table) -> i32 {
    TABLE_BASE + t.id as i32
}

fn index_oid(t: &Table, pos: usize) -> i32 {
    1_000_000 + t.id as i32 * 1000 + pos as i32
}

fn constraint_oid(t: &Table, slot: i32) -> i32 {
    2_000_000 + t.id as i32 * 1000 + slot
}

fn sequence_oid(t: &Table, col: usize) -> i32 {
    3_000_000 + t.id as i32 * 100 + col as i32
}

fn sorted(st: &State) -> Vec<&Table> {
    let mut v: Vec<&Table> = st.tables.values().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// The `atttypmod` of a column type.
fn typmod(t: &SqlType) -> i32 {
    match t {
        SqlType::Numeric {
            precision: Some(p),
            scale,
        }
        | SqlType::Decimal {
            precision: Some(p),
            scale,
        } => ((*p as i32) << 16 | scale.unwrap_or(0) as i32) + 4,
        SqlType::Varchar(Some(n)) | SqlType::Char(Some(n)) => *n as i32 + 4,
        _ => -1,
    }
}

fn type_oid(t: &SqlType) -> i32 {
    super::value::oid_of_type(t)
}

fn type_length(oid: i32) -> i32 {
    TYPES
        .iter()
        .find(|(o, ..)| *o == oid)
        .map_or(-1, |(_, _, l, _)| *l)
}

/// The text a `DEFAULT` expression is shown as.
fn default_text(c: &Col) -> Option<String> {
    if let Some(seq) = &c.serial {
        return Some(format!("nextval('{seq}'::regclass)"));
    }
    match c.default.as_ref()? {
        Expression::Literal(v) => Some(match v {
            SqlValue::Null => "NULL".to_string(),
            SqlValue::Boolean(b) => b.to_string(),
            SqlValue::SmallInt(i) => i.to_string(),
            SqlValue::Integer(i) => i.to_string(),
            SqlValue::BigInt(i) => i.to_string(),
            SqlValue::Decimal(d) => d.to_string(),
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                format!("'{}'::text", s.replace('\'', "''"))
            }
            other => format!("{other:?}"),
        }),
        Expression::Function(f) => Some(format!("{}()", fname(f))),
        _ => Some("NULL".to_string()),
    }
}

fn text(s: &str) -> SqlValue {
    SqlValue::Text(s.to_string())
}

fn int(i: i32) -> SqlValue {
    SqlValue::Integer(i)
}

fn boolean(b: bool) -> SqlValue {
    SqlValue::Boolean(b)
}

fn rel(qual: &str, cols: &[(&str, SqlType)], rows: Vec<Row>) -> Rel {
    Rel {
        cols: cols
            .iter()
            .map(|(n, t)| RelCol {
                qual: Some(qual.to_string()),
                name: n.to_string(),
                ty: Some(t.clone()),
            })
            .collect(),
        rows,
    }
}

const I: SqlType = SqlType::Integer;
const T: SqlType = SqlType::Text;
const B: SqlType = SqlType::Boolean;

fn process_start() -> chrono::NaiveDateTime {
    static START: OnceLock<chrono::NaiveDateTime> = OnceLock::new();
    *START.get_or_init(|| chrono::Utc::now().naive_utc())
}

/// The rows of `pg_catalog.<name>`, with columns qualified by `qual`.
pub(super) fn view(st: &State, name: &str, qual: &str) -> Option<Rel> {
    let tables = sorted(st);
    Some(match name {
        "pg_namespace" => rel(
            qual,
            &[("oid", I), ("nspname", T), ("nspowner", I), ("nspacl", T)],
            vec![
                vec![
                    int(NS_CATALOG),
                    text("pg_catalog"),
                    int(OWNER),
                    SqlValue::Null,
                ],
                vec![int(99), text("pg_toast"), int(OWNER), SqlValue::Null],
                vec![int(NS_PUBLIC), text("public"), int(OWNER), SqlValue::Null],
                vec![
                    int(NS_INFORMATION_SCHEMA),
                    text("information_schema"),
                    int(OWNER),
                    SqlValue::Null,
                ],
            ],
        ),
        "pg_database" => rel(
            qual,
            &[
                ("oid", I),
                ("datname", T),
                ("datdba", I),
                ("encoding", I),
                ("datcollate", T),
                ("datctype", T),
                ("datistemplate", B),
                ("datallowconn", B),
                ("datconnlimit", I),
                ("datlastsysoid", I),
                ("dattablespace", I),
                ("datacl", T),
            ],
            vec![vec![
                int(DATABASE_OID),
                text(&super::session_vars::current_database()),
                int(OWNER),
                int(6),
                text("en_US.UTF-8"),
                text("en_US.UTF-8"),
                boolean(false),
                boolean(true),
                int(-1),
                int(NS_INFORMATION_SCHEMA),
                int(1663),
                SqlValue::Null,
            ]],
        ),
        "pg_tablespace" => rel(
            qual,
            &[
                ("oid", I),
                ("spcname", T),
                ("spcowner", I),
                ("spcacl", T),
                ("spcoptions", T),
            ],
            vec![
                vec![
                    int(1663),
                    text("pg_default"),
                    int(OWNER),
                    SqlValue::Null,
                    SqlValue::Null,
                ],
                vec![
                    int(1664),
                    text("pg_global"),
                    int(OWNER),
                    SqlValue::Null,
                    SqlValue::Null,
                ],
            ],
        ),
        "pg_tables" => rel(
            qual,
            &[
                ("schemaname", T),
                ("tablename", T),
                ("tableowner", T),
                ("tablespace", T),
                ("hasindexes", B),
                ("hasrules", B),
                ("hastriggers", B),
                ("rowsecurity", B),
            ],
            tables
                .iter()
                .map(|t| {
                    vec![
                        text("public"),
                        text(&t.name),
                        text("postgres"),
                        SqlValue::Null,
                        boolean(t.indexes.iter().any(|i| !i.name.ends_with("_fkidx"))),
                        boolean(false),
                        boolean(false),
                        boolean(false),
                    ]
                })
                .collect(),
        ),
        "pg_indexes" => rel(
            qual,
            &[
                ("schemaname", T),
                ("tablename", T),
                ("indexname", T),
                ("tablespace", T),
                ("indexdef", T),
            ],
            tables
                .iter()
                .flat_map(|t| {
                    t.indexes
                        .iter()
                        .enumerate()
                        .filter(|(_, idx)| !idx.name.ends_with("_fkidx"))
                        .map(|(pos, idx)| {
                            let definition = index_definition(st, i64::from(index_oid(t, pos)))
                                .unwrap_or_default();
                            vec![
                                text("public"),
                                text(&t.name),
                                text(&idx.name),
                                SqlValue::Null,
                                text(&definition),
                            ]
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        ),
        "pg_roles" | "pg_user" => {
            let connected = super::session_vars::current_user();
            let mut roles = vec![(OWNER, "postgres".to_string())];
            if connected != "postgres" {
                roles.push((16385, connected));
            }
            if name == "pg_roles" {
                rel(
                    qual,
                    &[
                        ("oid", I),
                        ("rolname", T),
                        ("rolsuper", B),
                        ("rolinherit", B),
                        ("rolcreaterole", B),
                        ("rolcreatedb", B),
                        ("rolcanlogin", B),
                        ("rolreplication", B),
                        ("rolconnlimit", I),
                        ("rolpassword", T),
                        ("rolvaliduntil", T),
                        ("rolbypassrls", B),
                        ("rolconfig", T),
                    ],
                    roles
                        .iter()
                        .map(|(oid, n)| {
                            vec![
                                int(*oid),
                                text(n),
                                boolean(true),
                                boolean(true),
                                boolean(true),
                                boolean(true),
                                boolean(true),
                                boolean(false),
                                int(-1),
                                text("********"),
                                SqlValue::Null,
                                boolean(false),
                                SqlValue::Null,
                            ]
                        })
                        .collect(),
                )
            } else {
                rel(
                    qual,
                    &[
                        ("usename", T),
                        ("usesysid", I),
                        ("usecreatedb", B),
                        ("usesuper", B),
                        ("userepl", B),
                        ("usebypassrls", B),
                        ("passwd", T),
                    ],
                    roles
                        .iter()
                        .map(|(oid, n)| {
                            vec![
                                text(n),
                                int(*oid),
                                boolean(true),
                                boolean(true),
                                boolean(false),
                                boolean(false),
                                text("********"),
                            ]
                        })
                        .collect(),
                )
            }
        }
        "pg_auth_members" => rel(
            qual,
            &[
                ("roleid", I),
                ("member", I),
                ("grantor", I),
                ("admin_option", B),
            ],
            vec![],
        ),
        "pg_am" => rel(
            qual,
            &[("oid", I), ("amname", T), ("amtype", T)],
            vec![
                vec![int(403), text("btree"), text("i")],
                vec![int(405), text("hash"), text("i")],
            ],
        ),
        "pg_type" => rel(
            qual,
            &[
                ("oid", I),
                ("typname", T),
                ("typnamespace", I),
                ("typowner", I),
                ("typlen", I),
                ("typbyval", B),
                ("typtype", T),
                ("typcategory", T),
                ("typispreferred", B),
                ("typisdefined", B),
                ("typdelim", T),
                ("typrelid", I),
                ("typelem", I),
                ("typarray", I),
                ("typnotnull", B),
                ("typbasetype", I),
                ("typtypmod", I),
                ("typndims", I),
                ("typcollation", I),
                ("typdefault", T),
            ],
            TYPES
                .iter()
                .map(|(oid, n, len, cat)| {
                    vec![
                        int(*oid),
                        text(n),
                        int(NS_CATALOG),
                        int(OWNER),
                        int(*len),
                        boolean(*len > 0 && *len <= 8),
                        text("b"),
                        text(cat),
                        boolean(false),
                        boolean(true),
                        text(","),
                        int(0),
                        int(0),
                        int(0),
                        boolean(false),
                        int(0),
                        int(-1),
                        int(0),
                        int(0),
                        SqlValue::Null,
                    ]
                })
                .collect(),
        ),
        "pg_class" => {
            let mut rows: Vec<Row> = Vec::new();
            let class_row = |oid: i32, name: &str, ns: i32, kind: &str, atts: usize, idx: bool| {
                vec![
                    int(oid),
                    text(name),
                    int(ns),
                    int(0),
                    int(OWNER),
                    int(if idx { 403 } else { 0 }),
                    int(0),
                    int(0),
                    int(0),
                    int(0),
                    int(0),
                    boolean(false),
                    text("p"),
                    text(kind),
                    int(atts as i32),
                    int(0),
                    boolean(false),
                    boolean(idx),
                    boolean(false),
                    boolean(false),
                    boolean(false),
                    boolean(false),
                    boolean(false),
                    boolean(false),
                    text("d"),
                    boolean(false),
                    SqlValue::Null,
                    SqlValue::Null,
                ]
            };
            for (n, oid) in VIEWS {
                rows.push(class_row(*oid, n, NS_CATALOG, "r", 0, false));
            }
            for (n, oid) in INFORMATION_SCHEMA_VIEWS {
                rows.push(class_row(*oid, n, NS_INFORMATION_SCHEMA, "v", 0, false));
            }
            for t in &tables {
                let visible_indexes: Vec<_> = t
                    .indexes
                    .iter()
                    .enumerate()
                    .filter(|(_, idx)| !idx.name.ends_with("_fkidx"))
                    .collect();
                let mut r = class_row(table_oid(t), &t.name, NS_PUBLIC, "r", t.cols.len(), false);
                r[8] = int(t.rows.len() as i32);
                r[17] = boolean(!visible_indexes.is_empty());
                rows.push(r);
                for (pos, idx) in visible_indexes {
                    let mut r = class_row(
                        index_oid(t, pos),
                        &idx.name,
                        NS_PUBLIC,
                        "i",
                        idx.cols.len(),
                        true,
                    );
                    // relhasindex describes indexes on a relation; an index relation itself
                    // does not have an index in PostgreSQL.
                    r[17] = boolean(false);
                    rows.push(r);
                }
                for (i, c) in t.cols.iter().enumerate() {
                    if let Some(seq) = &c.serial {
                        rows.push(class_row(sequence_oid(t, i), seq, NS_PUBLIC, "S", 3, false));
                    }
                }
            }
            rel(
                qual,
                &[
                    ("oid", I),
                    ("relname", T),
                    ("relnamespace", I),
                    ("reltype", I),
                    ("relowner", I),
                    ("relam", I),
                    ("relfilenode", I),
                    ("reltablespace", I),
                    ("reltuples", I),
                    ("relpages", I),
                    ("relallvisible", I),
                    ("relisshared", B),
                    ("relpersistence", T),
                    ("relkind", T),
                    ("relnatts", I),
                    ("relchecks", I),
                    ("relhasrules", B),
                    ("relhasindex", B),
                    ("relhastriggers", B),
                    ("relhassubclass", B),
                    ("relrowsecurity", B),
                    ("relforcerowsecurity", B),
                    ("relispopulated", B),
                    ("relispartition", B),
                    ("relreplident", T),
                    ("relhasoids", B),
                    ("relacl", T),
                    ("reloptions", T),
                ],
                rows,
            )
        }
        "pg_attribute" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (i, c) in t.cols.iter().enumerate() {
                    let oid = type_oid(&c.ty);
                    rows.push(vec![
                        int(table_oid(t)),
                        text(&c.name),
                        int(oid),
                        int(-1),
                        int(type_length(oid)),
                        int(i as i32 + 1),
                        int(0),
                        int(typmod(&c.ty)),
                        boolean(false),
                        text("p"),
                        text("i"),
                        boolean(c.not_null),
                        boolean(default_text(c).is_some()),
                        text(""),
                        text(""),
                        boolean(false),
                        boolean(true),
                        int(0),
                        int(0),
                        SqlValue::Null,
                        SqlValue::Null,
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("attrelid", I),
                    ("attname", T),
                    ("atttypid", I),
                    ("attstattarget", I),
                    ("attlen", I),
                    ("attnum", I),
                    ("attndims", I),
                    ("atttypmod", I),
                    ("attbyval", B),
                    ("attstorage", T),
                    ("attalign", T),
                    ("attnotnull", B),
                    ("atthasdef", B),
                    ("attidentity", T),
                    ("attgenerated", T),
                    ("attisdropped", B),
                    ("attislocal", B),
                    ("attinhcount", I),
                    ("attcollation", I),
                    ("attacl", T),
                    ("attoptions", T),
                ],
                rows,
            )
        }
        "pg_attrdef" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (i, c) in t.cols.iter().enumerate() {
                    if let Some(d) = default_text(c) {
                        rows.push(vec![
                            int(4_000_000 + t.id as i32 * 100 + i as i32),
                            int(table_oid(t)),
                            int(i as i32 + 1),
                            text(&d),
                        ]);
                    }
                }
            }
            rel(
                qual,
                &[("oid", I), ("adrelid", I), ("adnum", I), ("adbin", T)],
                rows,
            )
        }
        "pg_index" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (pos, idx) in t.indexes.iter().enumerate() {
                    if idx.name.ends_with("_fkidx") {
                        continue;
                    }
                    let key = idx
                        .cols
                        .iter()
                        .map(|c| (c + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    rows.push(vec![
                        int(index_oid(t, pos)),
                        int(table_oid(t)),
                        int(idx.cols.len() as i32),
                        int(idx.cols.len() as i32),
                        boolean(idx.unique),
                        boolean(idx.primary),
                        boolean(false),
                        boolean(true),
                        boolean(false),
                        boolean(true),
                        boolean(false),
                        boolean(true),
                        boolean(true),
                        boolean(false),
                        text(&key),
                        SqlValue::Null,
                        SqlValue::Null,
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("indexrelid", I),
                    ("indrelid", I),
                    ("indnatts", I),
                    ("indnkeyatts", I),
                    ("indisunique", B),
                    ("indisprimary", B),
                    ("indisexclusion", B),
                    ("indimmediate", B),
                    ("indisclustered", B),
                    ("indisvalid", B),
                    ("indcheckxmin", B),
                    ("indisready", B),
                    ("indislive", B),
                    ("indisreplident", B),
                    ("indkey", T),
                    ("indexprs", T),
                    ("indpred", T),
                ],
                rows,
            )
        }
        "pg_constraint" => {
            let mut rows: Vec<Row> = Vec::new();
            let con =
                |oid: i32, name: &str, kind: &str, rel: i32, idx: i32, fk: i32, keys: &[usize]| {
                    vec![
                        int(oid),
                        text(name),
                        int(NS_PUBLIC),
                        text(kind),
                        boolean(false),
                        boolean(false),
                        boolean(true),
                        int(rel),
                        int(0),
                        int(idx),
                        int(0),
                        int(fk),
                        text("a"),
                        text("a"),
                        text("s"),
                        boolean(true),
                        int(0),
                        boolean(true),
                        SqlValue::Array(
                            keys.iter()
                                .map(|k| SqlValue::SmallInt(*k as i16 + 1))
                                .collect(),
                        ),
                    ]
                };
            for t in &tables {
                for (pos, idx) in t.indexes.iter().enumerate() {
                    if idx.primary || idx.unique {
                        rows.push(con(
                            constraint_oid(t, pos as i32),
                            &idx.name,
                            if idx.primary { "p" } else { "u" },
                            table_oid(t),
                            index_oid(t, pos),
                            0,
                            &idx.cols,
                        ));
                    }
                }
                for (i, fk) in t.fks.iter().enumerate() {
                    let target = st.tables.get(&fk.ref_table).map_or(0, table_oid);
                    rows.push(con(
                        constraint_oid(t, 500 + i as i32),
                        &fk.name,
                        "f",
                        table_oid(t),
                        0,
                        target,
                        &fk.cols,
                    ));
                }
                for (i, col) in t.cols.iter().enumerate().filter(|(_, c)| c.not_null) {
                    rows.push(con(
                        constraint_oid(t, 900 + i as i32),
                        &format!("{}_{}_not_null", t.name, col.name),
                        "n",
                        table_oid(t),
                        0,
                        0,
                        &[i],
                    ));
                }
                for (i, (name, _)) in t.checks.iter().enumerate() {
                    let n = name
                        .clone()
                        .unwrap_or_else(|| format!("{}_check{}", t.name, i + 1));
                    rows.push(con(
                        constraint_oid(t, 700 + i as i32),
                        &n,
                        "c",
                        table_oid(t),
                        0,
                        0,
                        &[],
                    ));
                }
            }
            rel(
                qual,
                &[
                    ("oid", I),
                    ("conname", T),
                    ("connamespace", I),
                    ("contype", T),
                    ("condeferrable", B),
                    ("condeferred", B),
                    ("convalidated", B),
                    ("conrelid", I),
                    ("contypid", I),
                    ("conindid", I),
                    ("conparentid", I),
                    ("confrelid", I),
                    ("confupdtype", T),
                    ("confdeltype", T),
                    ("confmatchtype", T),
                    ("conislocal", B),
                    ("coninhcount", I),
                    ("connoinherit", B),
                    (
                        "conkey",
                        SqlType::Array {
                            element_type: Box::new(SqlType::SmallInt),
                            dimensions: None,
                        },
                    ),
                ],
                rows,
            )
        }
        "pg_settings" => rel(
            qual,
            &[
                ("name", T),
                ("setting", T),
                ("unit", T),
                ("category", T),
                ("short_desc", T),
                ("context", T),
                ("vartype", T),
                ("source", T),
                ("boot_val", T),
                ("reset_val", T),
            ],
            SETTINGS
                .iter()
                .map(|(n, v)| {
                    vec![
                        text(n),
                        text(v),
                        SqlValue::Null,
                        text("Preset Options"),
                        text(n),
                        text("user"),
                        text("string"),
                        text("default"),
                        text(v),
                        text(v),
                    ]
                })
                .collect(),
        ),
        "pg_stat_activity" => rel(
            qual,
            &[
                ("datid", I),
                ("datname", T),
                ("pid", I),
                ("usesysid", I),
                ("usename", T),
                ("application_name", T),
                ("client_addr", T),
                ("client_hostname", T),
                ("client_port", I),
                (
                    "backend_start",
                    SqlType::Timestamp {
                        with_timezone: false,
                    },
                ),
                (
                    "xact_start",
                    SqlType::Timestamp {
                        with_timezone: false,
                    },
                ),
                (
                    "query_start",
                    SqlType::Timestamp {
                        with_timezone: false,
                    },
                ),
                (
                    "state_change",
                    SqlType::Timestamp {
                        with_timezone: false,
                    },
                ),
                ("wait_event_type", T),
                ("wait_event", T),
                ("state", T),
                ("backend_xid", I),
                ("backend_xmin", I),
                ("query", T),
                ("backend_type", T),
            ],
            vec![vec![
                int(DATABASE_OID),
                text(&super::session_vars::current_database()),
                int(1),
                int(16385),
                text(&super::session_vars::current_user()),
                text(""),
                SqlValue::Null,
                SqlValue::Null,
                int(-1),
                SqlValue::Timestamp(process_start()),
                SqlValue::Null,
                SqlValue::Null,
                SqlValue::Timestamp(process_start()),
                SqlValue::Null,
                SqlValue::Null,
                text("active"),
                SqlValue::Null,
                SqlValue::Null,
                text(""),
                text("client backend"),
            ]],
        ),
        "pg_stat_gssapi" => rel(
            qual,
            &[
                ("pid", I),
                ("gss_authenticated", B),
                ("principal", T),
                ("encrypted", B),
            ],
            vec![vec![int(1), boolean(false), SqlValue::Null, boolean(false)]],
        ),
        "pg_description" => rel(
            qual,
            &[
                ("objoid", I),
                ("classoid", I),
                ("objsubid", I),
                ("description", T),
            ],
            vec![],
        ),
        "pg_shdescription" => rel(
            qual,
            &[("objoid", I), ("classoid", I), ("description", T)],
            vec![],
        ),
        "pg_extension" => rel(
            qual,
            &[
                ("oid", I),
                ("extname", T),
                ("extowner", I),
                ("extnamespace", I),
                ("extrelocatable", B),
                ("extversion", T),
            ],
            vec![],
        ),
        "pg_trigger" => rel(
            qual,
            &[
                ("oid", I),
                ("tgrelid", I),
                ("tgname", T),
                ("tgfoid", I),
                ("tgtype", I),
                ("tgenabled", T),
                ("tgisinternal", B),
            ],
            vec![],
        ),
        "pg_inherits" => rel(
            qual,
            &[("inhrelid", I), ("inhparent", I), ("inhseqno", I)],
            vec![],
        ),
        "pg_depend" => rel(
            qual,
            &[
                ("classid", I),
                ("objid", I),
                ("objsubid", I),
                ("refclassid", I),
                ("refobjid", I),
                ("refobjsubid", I),
                ("deptype", T),
            ],
            vec![],
        ),
        "pg_proc" => rel(
            qual,
            &[
                ("oid", I),
                ("proname", T),
                ("pronamespace", I),
                ("proowner", I),
                ("prolang", I),
                ("prokind", T),
                ("prorettype", I),
            ],
            std::iter::once("lower")
                .chain(FUNCTIONS.iter().copied())
                .enumerate()
                .map(|(i, name)| {
                    vec![
                        int(4_500_000 + i as i32),
                        text(name),
                        int(NS_CATALOG),
                        int(OWNER),
                        int(12),
                        text("f"),
                        int(25),
                    ]
                })
                .collect(),
        ),
        _ => return None,
    })
}

/// The object id a name stands for as a `regclass`, or `None` when no relation has that name.
fn regclass(st: &State, name: &str) -> Option<i32> {
    let bare = name.trim().trim_matches('"');
    let (schema, rel) = match bare.rsplit_once('.') {
        Some((s, r)) => (
            Some(s.trim_matches('"').to_lowercase()),
            r.trim_matches('"'),
        ),
        None => (None, bare),
    };
    let rel = rel.to_lowercase();
    if matches!(schema.as_deref(), None | Some("pg_catalog")) {
        if let Some((_, oid)) = VIEWS.iter().find(|(n, _)| *n == rel) {
            return Some(*oid);
        }
    }
    if matches!(schema.as_deref(), Some("information_schema")) {
        if let Some((_, oid)) = INFORMATION_SCHEMA_VIEWS.iter().find(|(n, _)| *n == rel) {
            return Some(*oid);
        }
    }
    if matches!(schema.as_deref(), None | Some("public")) {
        for t in st.tables.values() {
            if t.name == rel {
                return Some(table_oid(t));
            }
            if let Some(pos) = t.indexes.iter().position(|i| i.name == rel) {
                return Some(index_oid(t, pos));
            }
            if let Some(i) = t
                .cols
                .iter()
                .position(|c| c.serial.as_deref() == Some(&rel))
            {
                return Some(sequence_oid(t, i));
            }
        }
    }
    None
}

pub(super) fn relation_name(st: &State, oid: u32) -> Option<&str> {
    st.tables
        .values()
        .find(|t| table_oid(t) as u32 == oid)
        .map(|t| t.name.as_str())
}

fn type_by_name(name: &str) -> Option<i32> {
    let n = name.trim().trim_matches('"').to_lowercase();
    let alias = match n.as_str() {
        "integer" | "int" => "int4",
        "bigint" => "int8",
        "smallint" => "int2",
        "boolean" => "bool",
        "real" => "float4",
        "double precision" => "float8",
        "character varying" => "varchar",
        "character" => "bpchar",
        "timestamp without time zone" => "timestamp",
        "timestamp with time zone" => "timestamptz",
        other => other,
    };
    TYPES
        .iter()
        .find(|(_, t, ..)| *t == alias)
        .map(|(o, ..)| *o)
}

/// A cast to an object-id type: names are looked up, numbers pass through.
pub(super) fn cast(st: &State, target: &SqlType, v: SqlValue) -> R<SqlValue> {
    let kind = match target {
        SqlType::Custom { type_name } => type_name.to_lowercase(),
        SqlType::Regclass => "regclass".to_string(),
        _ => "oid".to_string(),
    };
    Ok(match v {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) | SqlValue::Name(s) => {
            if let Ok(n) = s.trim().parse::<i32>() {
                return Ok(int(n));
            }
            let found = match kind.as_str() {
                "regclass" => regclass(st, &s),
                "regtype" => type_by_name(&s),
                "regnamespace" => match s.trim().trim_matches('"').to_lowercase().as_str() {
                    "pg_catalog" => Some(NS_CATALOG),
                    "public" => Some(NS_PUBLIC),
                    "information_schema" => Some(NS_INFORMATION_SCHEMA),
                    _ => None,
                },
                "regrole" => match s.trim().trim_matches('"') {
                    "postgres" => Some(OWNER),
                    other if other == super::session_vars::current_user() => Some(16385),
                    _ => None,
                },
                _ => Some(0),
            };
            match found {
                Some(oid) if kind == "regclass" => SqlValue::Regclass(oid as u32),
                Some(oid) => int(oid),
                None if kind == "regclass" => {
                    return err("42P01", format!("relation \"{s}\" does not exist"))
                }
                None => {
                    return err(
                        "22P02",
                        format!("invalid input syntax for type {kind}: \"{s}\""),
                    )
                }
            }
        }
        SqlValue::SmallInt(n) => int(n as i32),
        SqlValue::Integer(n) => int(n),
        SqlValue::BigInt(n) => int(n as i32),
        SqlValue::Oid(n) | SqlValue::Regclass(n) => int(n as i32),
        other => return err("42846", format!("cannot cast {other:?} to {kind}")),
    })
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::SmallInt(n) => Some(*n as i64),
        SqlValue::Integer(n) => Some(*n as i64),
        SqlValue::BigInt(n) => Some(*n),
        SqlValue::Oid(n) | SqlValue::Regclass(n) => Some(*n as i64),
        SqlValue::Text(s) | SqlValue::Varchar(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn as_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) | SqlValue::Name(s) => {
            Some(s.clone())
        }
        _ => None,
    }
}

fn type_name(oid: i32, typmod: i32) -> String {
    let base = match oid {
        16 => "boolean",
        17 => "bytea",
        19 => "name",
        20 => "bigint",
        21 => "smallint",
        23 => "integer",
        25 => "text",
        26 => "oid",
        114 => "json",
        700 => "real",
        701 => "double precision",
        1042 => "character",
        1043 => "character varying",
        1082 => "date",
        1083 => "time without time zone",
        1114 => "timestamp without time zone",
        1184 => "timestamp with time zone",
        1186 => "interval",
        1700 => "numeric",
        2950 => "uuid",
        3802 => "jsonb",
        _ => return "unknown".to_string(),
    };
    match (oid, typmod) {
        (1700, m) if m >= 4 => format!("numeric({},{})", (m - 4) >> 16 & 0xffff, (m - 4) & 0xffff),
        (1042 | 1043, m) if m >= 4 => format!("{base}({})", m - 4),
        _ => base.to_string(),
    }
}

fn pretty_size(n: i64) -> String {
    let (mut v, mut unit) = (n as f64, "bytes");
    for next in ["kB", "MB", "GB", "TB"] {
        if v.abs() < 10240.0 {
            break;
        }
        v /= 1024.0;
        unit = next;
    }
    if unit == "bytes" {
        format!("{n} bytes")
    } else {
        format!("{} {unit}", v.round() as i64)
    }
}

/// A table's size as an estimate: a page per table plus the rows at an assumed width.
fn table_size(t: &Table) -> i64 {
    8192 + t.rows.len() as i64 * 64
}

fn relation_size(st: &State, oid: i64) -> i64 {
    st.tables
        .values()
        .find(|t| table_oid(t) as i64 == oid)
        .map_or(0, table_size)
}

fn index_definition(st: &State, oid: i64) -> Option<String> {
    for t in st.tables.values() {
        for (pos, idx) in t.indexes.iter().enumerate() {
            if index_oid(t, pos) as i64 == oid {
                let cols: Vec<&str> = idx.cols.iter().map(|c| t.cols[*c].name.as_str()).collect();
                return Some(format!(
                    "CREATE {}INDEX {} ON public.{} USING btree ({})",
                    if idx.unique { "UNIQUE " } else { "" },
                    idx.name,
                    t.name,
                    cols.join(", ")
                ));
            }
        }
    }
    None
}

fn constraint_definition(st: &State, oid: i64) -> Option<String> {
    for t in st.tables.values() {
        for (pos, idx) in t.indexes.iter().enumerate() {
            if (idx.primary || idx.unique) && constraint_oid(t, pos as i32) as i64 == oid {
                let cols: Vec<&str> = idx.cols.iter().map(|c| t.cols[*c].name.as_str()).collect();
                return Some(format!(
                    "{} ({})",
                    if idx.primary { "PRIMARY KEY" } else { "UNIQUE" },
                    cols.join(", ")
                ));
            }
        }
        for (i, fk) in t.fks.iter().enumerate() {
            if constraint_oid(t, 500 + i as i32) as i64 == oid {
                let cols: Vec<&str> = fk.cols.iter().map(|c| t.cols[*c].name.as_str()).collect();
                let mut definition = format!(
                    "FOREIGN KEY ({}) REFERENCES {}({})",
                    cols.join(", "),
                    fk.ref_table,
                    fk.ref_cols.join(", ")
                );
                if !matches!(fk.on_delete, super::fk::Action::NoAction) {
                    definition.push_str(" ON DELETE ");
                    definition.push_str(match fk.on_delete {
                        super::fk::Action::Restrict => "RESTRICT",
                        super::fk::Action::Cascade => "CASCADE",
                        super::fk::Action::SetNull => "SET NULL",
                        super::fk::Action::SetDefault => "SET DEFAULT",
                        super::fk::Action::NoAction => unreachable!(),
                    });
                }
                if !matches!(fk.on_update, super::fk::Action::NoAction) {
                    definition.push_str(" ON UPDATE ");
                    definition.push_str(match fk.on_update {
                        super::fk::Action::Restrict => "RESTRICT",
                        super::fk::Action::Cascade => "CASCADE",
                        super::fk::Action::SetNull => "SET NULL",
                        super::fk::Action::SetDefault => "SET DEFAULT",
                        super::fk::Action::NoAction => unreachable!(),
                    });
                }
                return Some(definition);
            }
        }
        for (i, col) in t.cols.iter().enumerate().filter(|(_, c)| c.not_null) {
            if constraint_oid(t, 900 + i as i32) as i64 == oid {
                return Some(format!("NOT NULL {}", col.name));
            }
        }
        for i in 0..t.checks.len() {
            if constraint_oid(t, 700 + i as i32) as i64 == oid {
                return Some("CHECK (...)".to_string());
            }
        }
    }
    None
}

/// The value of catalog function `f` for already evaluated arguments.
pub(super) fn call(st: &State, f: &FunctionCall, args: &[SqlValue]) -> R<SqlValue> {
    let name = fname(f);
    let first = args.first();
    if matches!(first, Some(SqlValue::Null))
        && !matches!(name.as_str(), "pg_backend_pid" | "pg_postmaster_start_time")
    {
        return Ok(SqlValue::Null);
    }
    Ok(match name.as_str() {
        "pg_encoding_to_char" => text("UTF8"),
        n if n.starts_with("has_") || n == "pg_has_role" => boolean(true),
        "pg_table_is_visible" | "pg_type_is_visible" | "pg_function_is_visible" => boolean(true),
        "pg_backend_pid" => int(1),
        "pg_postmaster_start_time" => SqlValue::Timestamp(process_start()),
        "pg_size_pretty" => match first.and_then(as_int) {
            Some(n) => text(&pretty_size(n)),
            None => SqlValue::Null,
        },
        "pg_database_size" => {
            SqlValue::BigInt(8 * 1024 * 1024 + st.tables.values().map(table_size).sum::<i64>())
        }
        "pg_total_relation_size" | "pg_relation_size" | "pg_table_size" => {
            SqlValue::BigInt(first.and_then(as_int).map_or(0, |o| relation_size(st, o)))
        }
        "pg_indexes_size" => SqlValue::BigInt(0),
        "format_type" => {
            let oid = first.and_then(as_int).unwrap_or(0) as i32;
            let tm = args.get(1).and_then(as_int).unwrap_or(-1) as i32;
            text(&type_name(oid, tm))
        }
        "pg_get_expr" => first.cloned().unwrap_or(SqlValue::Null),
        "pg_get_indexdef" => match first.and_then(as_int).and_then(|o| index_definition(st, o)) {
            Some(d) => text(&d),
            None => SqlValue::Null,
        },
        "pg_get_constraintdef" => {
            match first
                .and_then(as_int)
                .and_then(|o| constraint_definition(st, o))
            {
                Some(d) => text(&d),
                None => SqlValue::Null,
            }
        }
        "pg_get_userbyid" => match first.and_then(as_int) {
            Some(16385) => text(&super::session_vars::current_user()),
            Some(_) => text("postgres"),
            None => SqlValue::Null,
        },
        "pg_get_serial_sequence" => {
            let table = first.and_then(as_text).unwrap_or_default().to_lowercase();
            let col = args
                .get(1)
                .and_then(as_text)
                .unwrap_or_default()
                .to_lowercase();
            let seq = st
                .tables
                .get(table.rsplit('.').next().unwrap_or(&table))
                .and_then(|t| t.cols.iter().find(|c| c.name == col))
                .and_then(|c| c.serial.clone());
            match seq {
                Some(s) => text(&format!("public.{s}")),
                None => SqlValue::Null,
            }
        }
        "obj_description" | "col_description" | "shobj_description" => SqlValue::Null,
        "to_regclass" => match first.and_then(as_text).and_then(|s| regclass(st, &s)) {
            Some(oid) => int(oid),
            None => SqlValue::Null,
        },
        _ => SqlValue::Null,
    })
}
