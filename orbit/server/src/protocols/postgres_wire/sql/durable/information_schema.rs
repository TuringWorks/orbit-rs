//! `information_schema`: the standard views that describe the tables, columns, constraints, sequences
//! and privileges of the database. Each view is computed from the committed [`State`] on each use.

use super::pg_catalog;
use super::query::{Rel, RelCol};
use super::{err, Row, State, Table, R};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};

const CATALOG: &str = "orbit";
const OWNER: &str = "orbit";

const T: SqlType = SqlType::Text;
const I: SqlType = SqlType::Integer;

fn text(s: &str) -> SqlValue {
    SqlValue::Text(s.to_string())
}

fn int(i: i32) -> SqlValue {
    SqlValue::Integer(i)
}

fn yes(b: bool) -> SqlValue {
    text(if b { "YES" } else { "NO" })
}

fn opt_int(v: Option<i32>) -> SqlValue {
    v.map_or(SqlValue::Null, int)
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

fn sorted(st: &State) -> Vec<&Table> {
    let mut v: Vec<&Table> = st.tables.values().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// The name `columns.data_type` gives a column type.
fn data_type_name(t: &SqlType) -> &'static str {
    match t {
        SqlType::SmallInt => "smallint",
        SqlType::Integer => "integer",
        SqlType::BigInt => "bigint",
        SqlType::Decimal { .. } | SqlType::Numeric { .. } => "numeric",
        SqlType::Real => "real",
        SqlType::DoublePrecision => "double precision",
        SqlType::Boolean => "boolean",
        SqlType::Char(_) => "character",
        SqlType::Varchar(_) => "character varying",
        SqlType::Text => "text",
        SqlType::Bytea => "bytea",
        SqlType::Date => "date",
        SqlType::Time {
            with_timezone: true,
        } => "time with time zone",
        SqlType::Time { .. } => "time without time zone",
        SqlType::Timestamp {
            with_timezone: true,
        } => "timestamp with time zone",
        SqlType::Timestamp { .. } => "timestamp without time zone",
        SqlType::Interval => "interval",
        SqlType::Json => "json",
        SqlType::Jsonb => "jsonb",
        SqlType::Uuid => "uuid",
        _ => "USER-DEFINED",
    }
}

/// The type's name in `pg_type`.
fn udt_name(t: &SqlType) -> &'static str {
    match t {
        SqlType::SmallInt => "int2",
        SqlType::Integer => "int4",
        SqlType::BigInt => "int8",
        SqlType::Decimal { .. } | SqlType::Numeric { .. } => "numeric",
        SqlType::Real => "float4",
        SqlType::DoublePrecision => "float8",
        SqlType::Boolean => "bool",
        SqlType::Char(_) => "bpchar",
        SqlType::Varchar(_) => "varchar",
        SqlType::Text => "text",
        SqlType::Bytea => "bytea",
        SqlType::Date => "date",
        SqlType::Time {
            with_timezone: true,
        } => "timetz",
        SqlType::Time { .. } => "time",
        SqlType::Timestamp {
            with_timezone: true,
        } => "timestamptz",
        SqlType::Timestamp { .. } => "timestamp",
        SqlType::Interval => "interval",
        SqlType::Json => "json",
        SqlType::Jsonb => "jsonb",
        SqlType::Uuid => "uuid",
        _ => "unknown",
    }
}

/// `(precision, radix, scale)` for numeric types.
fn numeric_parts(t: &SqlType) -> (Option<i32>, Option<i32>, Option<i32>) {
    match t {
        SqlType::SmallInt => (Some(16), Some(2), Some(0)),
        SqlType::Integer => (Some(32), Some(2), Some(0)),
        SqlType::BigInt => (Some(64), Some(2), Some(0)),
        SqlType::Real => (Some(24), Some(2), None),
        SqlType::DoublePrecision => (Some(53), Some(2), None),
        SqlType::Decimal { precision, scale } | SqlType::Numeric { precision, scale } => (
            precision.map(i32::from),
            Some(10),
            precision.map(|_| scale.map_or(0, i32::from)),
        ),
        _ => (None, None, None),
    }
}

fn char_length(t: &SqlType) -> Option<i32> {
    match t {
        SqlType::Char(Some(n)) | SqlType::Varchar(Some(n)) => Some(*n as i32),
        _ => None,
    }
}

fn datetime_precision(t: &SqlType) -> Option<i32> {
    match t {
        SqlType::Date => Some(0),
        SqlType::Time { .. } | SqlType::Timestamp { .. } | SqlType::Interval => Some(6),
        _ => None,
    }
}

/// The rows of `information_schema.<view>`, with columns qualified by `qual`.
pub(super) fn view(st: &State, view: &str, qual: &str) -> R<Rel> {
    let tables = sorted(st);
    Ok(match view {
        "schemata" => rel(
            qual,
            &[
                ("catalog_name", T),
                ("schema_name", T),
                ("schema_owner", T),
                ("default_character_set_catalog", T),
                ("default_character_set_schema", T),
                ("default_character_set_name", T),
                ("sql_path", T),
            ],
            ["public", "pg_catalog", "information_schema", "pg_toast"]
                .iter()
                .map(|s| {
                    vec![
                        text(CATALOG),
                        text(s),
                        text(OWNER),
                        SqlValue::Null,
                        SqlValue::Null,
                        SqlValue::Null,
                        SqlValue::Null,
                    ]
                })
                .collect(),
        ),
        "tables" => {
            let mut rows: Vec<Row> = Vec::new();
            let row = |schema: &str, name: &str, kind: &str| {
                vec![
                    text(CATALOG),
                    text(schema),
                    text(name),
                    text(kind),
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                    yes(kind == "BASE TABLE" && schema == "public"),
                    yes(false),
                    SqlValue::Null,
                ]
            };
            for t in &tables {
                rows.push(row("public", &t.name, "BASE TABLE"));
            }
            for name in pg_catalog::view_names() {
                rows.push(row("pg_catalog", name, "BASE TABLE"));
            }
            for name in pg_catalog::information_schema_names() {
                rows.push(row("information_schema", name, "VIEW"));
            }
            rel(
                qual,
                &[
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("table_type", T),
                    ("self_referencing_column_name", T),
                    ("reference_generation", T),
                    ("user_defined_type_catalog", T),
                    ("user_defined_type_schema", T),
                    ("user_defined_type_name", T),
                    ("is_insertable_into", T),
                    ("is_typed", T),
                    ("commit_action", T),
                ],
                rows,
            )
        }
        "views" => rel(
            qual,
            &[
                ("table_catalog", T),
                ("table_schema", T),
                ("table_name", T),
                ("view_definition", T),
                ("check_option", T),
                ("is_updatable", T),
                ("is_insertable_into", T),
            ],
            vec![],
        ),
        "columns" => {
            let mut rows: Vec<Row> = Vec::new();
            let column = |schema: &str,
                          table: &str,
                          name: &str,
                          pos: usize,
                          default: SqlValue,
                          nullable: bool,
                          ty: &SqlType,
                          updatable: bool| {
                let (precision, radix, scale) = numeric_parts(ty);
                let length = char_length(ty);
                vec![
                    text(CATALOG),
                    text(schema),
                    text(table),
                    text(name),
                    int(pos as i32 + 1),
                    default,
                    yes(nullable),
                    text(data_type_name(ty)),
                    opt_int(length),
                    opt_int(length.map(|n| n * 4)),
                    opt_int(precision),
                    opt_int(radix),
                    opt_int(scale),
                    opt_int(datetime_precision(ty)),
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                    text(CATALOG),
                    text("pg_catalog"),
                    text(udt_name(ty)),
                    text(&(pos + 1).to_string()),
                    yes(false),
                    text("NO"),
                    SqlValue::Null,
                    text("NEVER"),
                    SqlValue::Null,
                    yes(updatable),
                ]
            };
            for t in &tables {
                for (i, c) in t.cols.iter().enumerate() {
                    let default = match &c.serial {
                        Some(seq) => text(&format!("nextval('{seq}'::regclass)")),
                        None => SqlValue::Null,
                    };
                    rows.push(column(
                        "public",
                        &t.name,
                        &c.name,
                        i,
                        default,
                        !c.not_null,
                        &c.ty,
                        true,
                    ));
                }
            }
            for name in pg_catalog::view_names() {
                if let Some(v) = pg_catalog::view(st, name, name) {
                    for (i, c) in v.cols.iter().enumerate() {
                        let ty = c.ty.clone().unwrap_or(SqlType::Text);
                        rows.push(column(
                            "pg_catalog",
                            name,
                            &c.name,
                            i,
                            SqlValue::Null,
                            true,
                            &ty,
                            false,
                        ));
                    }
                }
            }
            rel(
                qual,
                &[
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("column_name", T),
                    ("ordinal_position", I),
                    ("column_default", T),
                    ("is_nullable", T),
                    ("data_type", T),
                    ("character_maximum_length", I),
                    ("character_octet_length", I),
                    ("numeric_precision", I),
                    ("numeric_precision_radix", I),
                    ("numeric_scale", I),
                    ("datetime_precision", I),
                    ("interval_type", T),
                    ("character_set_name", T),
                    ("collation_name", T),
                    ("udt_catalog", T),
                    ("udt_schema", T),
                    ("udt_name", T),
                    ("dtd_identifier", T),
                    ("is_self_referencing", T),
                    ("is_identity", T),
                    ("identity_generation", T),
                    ("is_generated", T),
                    ("generation_expression", T),
                    ("is_updatable", T),
                ],
                rows,
            )
        }
        "table_constraints" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (name, kind) in constraints(t) {
                    rows.push(vec![
                        text(CATALOG),
                        text("public"),
                        text(&name),
                        text(CATALOG),
                        text("public"),
                        text(&t.name),
                        text(kind),
                        yes(false),
                        yes(false),
                        text("YES"),
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("constraint_type", T),
                    ("is_deferrable", T),
                    ("initially_deferred", T),
                    ("enforced", T),
                ],
                rows,
            )
        }
        "key_column_usage" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                let mut push = |name: &str, cols: &[usize], refs: Option<(&Table, &[String])>| {
                    for (n, c) in cols.iter().enumerate() {
                        let position = refs.map(|(rt, rc)| {
                            rc.get(n)
                                .and_then(|name| rt.col_index(name))
                                .map_or(0, |p| p as i32 + 1)
                        });
                        rows.push(vec![
                            text(CATALOG),
                            text("public"),
                            text(name),
                            text(CATALOG),
                            text("public"),
                            text(&t.name),
                            text(&t.cols[*c].name),
                            int(n as i32 + 1),
                            opt_int(position),
                        ]);
                    }
                };
                for idx in t.indexes.iter().filter(|i| i.primary) {
                    push(&idx.name, &idx.cols, None);
                }
                for fk in &t.fks {
                    let target = st.tables.get(&fk.ref_table);
                    push(
                        &fk.name,
                        &fk.cols,
                        target.map(|rt| (rt, fk.ref_cols.as_slice())),
                    );
                }
            }
            rel(
                qual,
                &[
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("column_name", T),
                    ("ordinal_position", I),
                    ("position_in_unique_constraint", I),
                ],
                rows,
            )
        }
        "constraint_column_usage" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for idx in t.indexes.iter().filter(|i| i.primary) {
                    for c in &idx.cols {
                        rows.push(usage_row(&t.name, &t.cols[*c].name, &idx.name));
                    }
                }
                for fk in &t.fks {
                    for c in &fk.ref_cols {
                        rows.push(usage_row(&fk.ref_table, c, &fk.name));
                    }
                }
            }
            rel(
                qual,
                &[
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("column_name", T),
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                ],
                rows,
            )
        }
        "constraint_table_usage" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (name, _) in constraints(t) {
                    rows.push(vec![
                        text(CATALOG),
                        text("public"),
                        text(&t.name),
                        text(CATALOG),
                        text("public"),
                        text(&name),
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                ],
                rows,
            )
        }
        "referential_constraints" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for fk in &t.fks {
                    let unique = st.tables.get(&fk.ref_table).and_then(|rt| {
                        rt.indexes
                            .iter()
                            .find(|i| i.primary)
                            .map(|i| i.name.clone())
                    });
                    rows.push(vec![
                        text(CATALOG),
                        text("public"),
                        text(&fk.name),
                        unique.as_ref().map_or(SqlValue::Null, |_| text(CATALOG)),
                        unique.as_ref().map_or(SqlValue::Null, |_| text("public")),
                        unique.as_deref().map_or(SqlValue::Null, text),
                        text("NONE"),
                        text(
                            &format!("{:?}", fk.on_update)
                                .to_uppercase()
                                .replace("SET", "SET "),
                        ),
                        text(
                            &format!("{:?}", fk.on_delete)
                                .to_uppercase()
                                .replace("SET", "SET "),
                        ),
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                    ("unique_constraint_catalog", T),
                    ("unique_constraint_schema", T),
                    ("unique_constraint_name", T),
                    ("match_option", T),
                    ("update_rule", T),
                    ("delete_rule", T),
                ],
                rows,
            )
        }
        "check_constraints" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for (name, kind) in constraints(t) {
                    if kind == "CHECK" {
                        rows.push(vec![
                            text(CATALOG),
                            text("public"),
                            text(&name),
                            text("(...)"),
                        ]);
                    }
                }
            }
            rel(
                qual,
                &[
                    ("constraint_catalog", T),
                    ("constraint_schema", T),
                    ("constraint_name", T),
                    ("check_clause", T),
                ],
                rows,
            )
        }
        "sequences" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for c in &t.cols {
                    if let Some(seq) = &c.serial {
                        let (data_type, precision) = match c.ty {
                            SqlType::BigInt => ("bigint", 64),
                            SqlType::SmallInt => ("smallint", 16),
                            _ => ("integer", 32),
                        };
                        rows.push(vec![
                            text(CATALOG),
                            text("public"),
                            text(seq),
                            text(data_type),
                            int(precision),
                            int(2),
                            int(0),
                            text("1"),
                            text("1"),
                            text(if precision == 64 {
                                "9223372036854775807"
                            } else if precision == 16 {
                                "32767"
                            } else {
                                "2147483647"
                            }),
                            text("1"),
                            yes(false),
                        ]);
                    }
                }
            }
            rel(
                qual,
                &[
                    ("sequence_catalog", T),
                    ("sequence_schema", T),
                    ("sequence_name", T),
                    ("data_type", T),
                    ("numeric_precision", I),
                    ("numeric_precision_radix", I),
                    ("numeric_scale", I),
                    ("start_value", T),
                    ("minimum_value", T),
                    ("maximum_value", T),
                    ("increment", T),
                    ("cycle_option", T),
                ],
                rows,
            )
        }
        "table_privileges" | "role_table_grants" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for privilege in [
                    "INSERT",
                    "SELECT",
                    "UPDATE",
                    "DELETE",
                    "TRUNCATE",
                    "REFERENCES",
                    "TRIGGER",
                ] {
                    rows.push(vec![
                        text(OWNER),
                        text(OWNER),
                        text(CATALOG),
                        text("public"),
                        text(&t.name),
                        text(privilege),
                        yes(true),
                        yes(privilege == "SELECT"),
                    ]);
                }
            }
            rel(
                qual,
                &[
                    ("grantor", T),
                    ("grantee", T),
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("privilege_type", T),
                    ("is_grantable", T),
                    ("with_hierarchy", T),
                ],
                rows,
            )
        }
        "column_privileges" => {
            let mut rows: Vec<Row> = Vec::new();
            for t in &tables {
                for c in &t.cols {
                    for privilege in ["INSERT", "SELECT", "UPDATE", "REFERENCES"] {
                        rows.push(vec![
                            text(OWNER),
                            text(OWNER),
                            text(CATALOG),
                            text("public"),
                            text(&t.name),
                            text(&c.name),
                            text(privilege),
                            yes(true),
                        ]);
                    }
                }
            }
            rel(
                qual,
                &[
                    ("grantor", T),
                    ("grantee", T),
                    ("table_catalog", T),
                    ("table_schema", T),
                    ("table_name", T),
                    ("column_name", T),
                    ("privilege_type", T),
                    ("is_grantable", T),
                ],
                rows,
            )
        }
        "enabled_roles" | "applicable_roles" => {
            if view == "enabled_roles" {
                rel(qual, &[("role_name", T)], vec![vec![text(OWNER)]])
            } else {
                rel(
                    qual,
                    &[("grantee", T), ("role_name", T), ("is_grantable", T)],
                    vec![],
                )
            }
        }
        "character_sets" => rel(
            qual,
            &[
                ("character_set_catalog", T),
                ("character_set_schema", T),
                ("character_set_name", T),
                ("character_repertoire", T),
                ("form_of_use", T),
                ("default_collate_catalog", T),
                ("default_collate_schema", T),
                ("default_collate_name", T),
            ],
            vec![vec![
                SqlValue::Null,
                SqlValue::Null,
                text("UTF8"),
                text("UCS"),
                text("UTF8"),
                text(CATALOG),
                text("pg_catalog"),
                text("en_US.UTF-8"),
            ]],
        ),
        "routines" => rel(
            qual,
            &[
                ("specific_catalog", T),
                ("specific_schema", T),
                ("specific_name", T),
                ("routine_catalog", T),
                ("routine_schema", T),
                ("routine_name", T),
                ("routine_type", T),
                ("data_type", T),
                ("routine_definition", T),
                ("external_language", T),
            ],
            vec![],
        ),
        "parameters" => rel(
            qual,
            &[
                ("specific_catalog", T),
                ("specific_schema", T),
                ("specific_name", T),
                ("ordinal_position", I),
                ("parameter_mode", T),
                ("parameter_name", T),
                ("data_type", T),
            ],
            vec![],
        ),
        "triggers" => rel(
            qual,
            &[
                ("trigger_catalog", T),
                ("trigger_schema", T),
                ("trigger_name", T),
                ("event_manipulation", T),
                ("event_object_catalog", T),
                ("event_object_schema", T),
                ("event_object_table", T),
                ("action_order", I),
                ("action_condition", T),
                ("action_statement", T),
                ("action_orientation", T),
                ("action_timing", T),
            ],
            vec![],
        ),
        "domains" => rel(
            qual,
            &[
                ("domain_catalog", T),
                ("domain_schema", T),
                ("domain_name", T),
                ("data_type", T),
                ("domain_default", T),
            ],
            vec![],
        ),
        "collations" => rel(
            qual,
            &[
                ("collation_catalog", T),
                ("collation_schema", T),
                ("collation_name", T),
                ("pad_attribute", T),
            ],
            vec![],
        ),
        "element_types" => rel(
            qual,
            &[
                ("object_catalog", T),
                ("object_schema", T),
                ("object_name", T),
                ("object_type", T),
                ("collection_type_identifier", T),
                ("data_type", T),
            ],
            vec![],
        ),
        other => {
            return err(
                "42P01",
                format!("relation \"information_schema.{other}\" does not exist"),
            )
        }
    })
}

/// A table's constraints as `(name, type)`: the primary key, foreign keys and checks.
fn constraints(t: &Table) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for idx in t.indexes.iter().filter(|i| i.primary) {
        out.push((idx.name.clone(), "PRIMARY KEY"));
    }
    for fk in &t.fks {
        out.push((fk.name.clone(), "FOREIGN KEY"));
    }
    for (i, (name, _)) in t.checks.iter().enumerate() {
        out.push((
            name.clone()
                .unwrap_or_else(|| format!("{}_check{}", t.name, i + 1)),
            "CHECK",
        ));
    }
    out
}

fn usage_row(table: &str, column: &str, constraint: &str) -> Row {
    vec![
        text(CATALOG),
        text("public"),
        text(table),
        text(column),
        text(CATALOG),
        text("public"),
        text(constraint),
    ]
}
