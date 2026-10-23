//! Schema statements: tables, indexes and column changes.
//!
//! Each statement records its [`Op`]s with the DDL text first, so a replica applying the ops in
//! order reshapes the table before it receives the rows that changed with it.

use super::value::coerce;
use super::{err, Col, DbError, Index, Op, QueryResult, Seqs, State, Table, R};
use crate::protocols::postgres_wire::sql::ast::{
    AlterTableAction, ColumnConstraint, ColumnDefinition, CreateIndexStatement,
    CreateTableStatement, Expression, IndexType, Statement, TableConstraint, TableName,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::{
    EvaluationContext, ExpressionEvaluator,
};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::sync::Mutex;

fn tag(t: &str) -> R<QueryResult> {
    Ok(QueryResult::Command { tag: t.to_string() })
}

/// Table names are matched case-insensitively and without a schema qualifier.
pub(crate) fn table_key(n: &TableName) -> String {
    n.name.to_lowercase()
}

/// Run a schema statement. `None` means the statement is not a schema statement.
/// `ops` is absent while replaying (recovery, replicas), when nothing is recorded; `seqs` is
/// absent there too, because sequences follow their own ops.
pub(crate) fn apply(
    st: &mut State,
    mut ops: Option<&mut Vec<Op>>,
    seqs: Option<&Mutex<Seqs>>,
    stmt: &Statement,
    text: &str,
    replay: bool,
) -> Option<R<QueryResult>> {
    let r = match stmt {
        Statement::CreateTable(ct) => create_table(st, &mut ops, ct, text),
        Statement::DropTable(d) => drop_tables(st, &mut ops, &d.names, d.if_exists, text),
        Statement::CreateIndex(ci) => create_index(st, &mut ops, ci, text),
        Statement::DropIndex(d) => drop_indexes(st, &mut ops, &d.names, d.if_exists, text),
        Statement::AlterTable(a) => {
            alter_table(st, &mut ops, seqs, &a.name, &a.actions, text, replay)
        }
        Statement::Truncate(t) => truncate(st, &mut ops, &t.tables),
        _ => return None,
    };
    Some(r)
}

fn record(ops: &mut Option<&mut Vec<Op>>, text: &str, extra: Vec<Op>) {
    if let Some(o) = ops {
        o.push(Op::Ddl(text.to_string()));
        o.extend(extra);
    }
}

fn eval_const(expr: &Expression) -> R<SqlValue> {
    let mut ev = ExpressionEvaluator::new();
    Ok(ev.evaluate(expr, &EvaluationContext::empty())?)
}

fn build_col(table: &str, def: &ColumnDefinition) -> (Col, bool, bool) {
    let mut col = Col {
        name: def.name.to_lowercase(),
        ty: def.data_type.clone(),
        not_null: false,
        default: None,
        serial: None,
    };
    let (mut primary, mut unique) = (false, false);
    for c in &def.constraints {
        match c {
            ColumnConstraint::NotNull => col.not_null = true,
            ColumnConstraint::Serial => col.serial = Some(format!("{table}_{}_seq", col.name)),
            ColumnConstraint::Default(e) => col.default = Some(e.clone()),
            ColumnConstraint::PrimaryKey => {
                primary = true;
                col.not_null = true;
            }
            ColumnConstraint::Unique => unique = true,
            ColumnConstraint::Generated { expression, .. } => {
                col.default = Some(expression.clone())
            }
            _ => {}
        }
    }
    (col, primary, unique)
}

fn new_index(name: String, cols: Vec<usize>, unique: bool, primary: bool) -> Index {
    Index {
        name,
        cols,
        unique,
        primary,
        map: imbl::HashMap::new(),
    }
}

fn create_table(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    ct: &CreateTableStatement,
    text: &str,
) -> R<QueryResult> {
    let name = table_key(&ct.name);
    if st.tables.contains_key(&name) {
        if ct.if_not_exists {
            return tag("CREATE TABLE");
        }
        return err("42P07", format!("relation \"{name}\" already exists"));
    }
    let mut cols = Vec::new();
    let mut indexes: Vec<Index> = Vec::new();
    for (i, def) in ct.columns.iter().enumerate() {
        let (col, primary, unique) = build_col(&name, def);
        if cols.iter().any(|c: &Col| c.name == col.name) {
            return err(
                "42701",
                format!("column \"{}\" specified more than once", col.name),
            );
        }
        if primary {
            indexes.push(new_index(format!("{name}_pkey"), vec![i], true, true));
        } else if unique {
            indexes.push(new_index(
                format!("{name}_{}_key", col.name),
                vec![i],
                true,
                false,
            ));
        }
        cols.push(col);
    }
    for c in &ct.constraints {
        match c {
            TableConstraint::PrimaryKey {
                name: n, columns, ..
            }
            | TableConstraint::Unique {
                name: n, columns, ..
            } => {
                let primary = matches!(c, TableConstraint::PrimaryKey { .. });
                let idx: Vec<usize> = columns
                    .iter()
                    .map(|cn| {
                        cols.iter()
                            .position(|c| c.name == cn.to_lowercase())
                            .ok_or(cn)
                    })
                    .collect::<Result<_, _>>()
                    .or_else(|cn| {
                        err(
                            "42703",
                            format!("column \"{cn}\" named in key does not exist"),
                        )
                    })?;
                if primary {
                    for &i in &idx {
                        cols[i].not_null = true;
                    }
                }
                let iname = n.clone().unwrap_or_else(|| {
                    if primary {
                        format!("{name}_pkey")
                    } else {
                        format!(
                            "{name}_{}_key",
                            idx.iter()
                                .map(|&i| cols[i].name.clone())
                                .collect::<Vec<_>>()
                                .join("_")
                        )
                    }
                });
                indexes.push(new_index(iname, idx, true, primary));
            }
            // Foreign keys and checks are not enforced yet.
            _ => {}
        }
    }
    let id = st.next_table_id;
    st.next_table_id += 1;
    st.put_table(Table {
        id,
        name,
        cols,
        rows: imbl::OrdMap::new(),
        next_rowid: 1,
        indexes,
    });
    record(ops, text, vec![]);
    tag("CREATE TABLE")
}

fn drop_tables(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    names: &[TableName],
    if_exists: bool,
    text: &str,
) -> R<QueryResult> {
    let mut extra = Vec::new();
    for n in names {
        let key = table_key(n);
        match st.remove_table(&key) {
            Some(t) => {
                extra.push(Op::DropRows { table: t.id });
                for c in &t.cols {
                    if let Some(seq) = &c.serial {
                        extra.push(Op::SeqHighWater {
                            name: seq.clone(),
                            value: 0,
                        });
                    }
                }
            }
            None if if_exists => {}
            None => return err("42P01", format!("table \"{key}\" does not exist")),
        }
    }
    record(ops, text, extra);
    tag("DROP TABLE")
}

fn create_index(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    ci: &CreateIndexStatement,
    text: &str,
) -> R<QueryResult> {
    let key = table_key(&ci.table);
    let Some(table) = st.tables.get_mut(&key) else {
        return err("42P01", format!("relation \"{key}\" does not exist"));
    };
    let name = ci
        .name
        .clone()
        .map(|n| n.to_lowercase())
        .unwrap_or_else(|| {
            format!(
                "{key}_{}_idx",
                ci.columns
                    .iter()
                    .map(|c| c.name.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("_")
            )
        });
    if table.indexes.iter().any(|i| i.name == name) {
        if ci.if_not_exists {
            return tag("CREATE INDEX");
        }
        return err("42P07", format!("relation \"{name}\" already exists"));
    }
    // Only plain column indexes are built. A partial index or one on an expression is accepted and
    // ignored, because enforcing it as a full index would reject rows PostgreSQL allows.
    let cols: Option<Vec<usize>> = ci
        .columns
        .iter()
        .map(|c| table.col_index(&c.name))
        .collect();
    let plain =
        ci.where_clause.is_none() && matches!(ci.index_type, IndexType::BTree | IndexType::Hash);
    let (Some(cols), true) = (cols, plain) else {
        return tag("CREATE INDEX");
    };
    let mut idx = new_index(name, cols, ci.unique, false);
    for (id, row) in &table.rows {
        if let Some(k) = idx.key(row) {
            let slot = idx.map.entry(k).or_default();
            if idx.unique && !slot.is_empty() {
                return err(
                    "23505",
                    format!(
                        "could not create unique index \"{}\": key is duplicated",
                        idx.name
                    ),
                );
            }
            slot.push(*id);
        }
    }
    table.indexes.push(idx);
    record(ops, text, vec![]);
    tag("CREATE INDEX")
}

fn drop_indexes(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    names: &[String],
    if_exists: bool,
    text: &str,
) -> R<QueryResult> {
    for n in names {
        let n = n.to_lowercase();
        let found = st
            .tables
            .iter()
            .find(|(_, t)| t.indexes.iter().any(|i| i.name == n))
            .map(|(k, _)| k.clone());
        match found {
            Some(key) => {
                let t = st.tables.get_mut(&key).expect("table");
                t.indexes.retain(|i| i.name != n);
            }
            None if if_exists => {}
            None => return err("42704", format!("index \"{n}\" does not exist")),
        }
    }
    record(ops, text, vec![]);
    tag("DROP INDEX")
}

fn alter_table(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    seqs: Option<&Mutex<Seqs>>,
    name: &TableName,
    actions: &[AlterTableAction],
    text: &str,
    replay: bool,
) -> R<QueryResult> {
    let key = table_key(name);
    let Some(mut table) = st.tables.get(&key).cloned() else {
        return err("42P01", format!("relation \"{key}\" does not exist"));
    };
    let mut rewrite = false;
    let mut renamed_to: Option<String> = None;
    for action in actions {
        match action {
            AlterTableAction::AddColumn(def) => {
                let (col, primary, unique) = build_col(&key, def);
                if table.cols.iter().any(|c| c.name == col.name) {
                    return err(
                        "42701",
                        format!(
                            "column \"{}\" of relation \"{key}\" already exists",
                            col.name
                        ),
                    );
                }
                let fill = match (&col.serial, &col.default) {
                    (None, Some(e)) => Some(eval_const(e)?),
                    _ => None,
                };
                if !replay
                    && col.not_null
                    && col.serial.is_none()
                    && fill.is_none()
                    && !table.rows.is_empty()
                {
                    return err(
                        "23502",
                        format!(
                            "column \"{}\" of relation \"{key}\" contains null values",
                            col.name
                        ),
                    );
                }
                let pos = table.cols.len();
                let ids: Vec<u64> = table.rows.keys().copied().collect();
                for id in ids {
                    // While replaying, the rewritten rows follow as ops, so the fill is a placeholder.
                    let v = match (&col.serial, &fill, seqs) {
                        (Some(seq), _, Some(s)) if !replay => coerce(
                            SqlValue::BigInt(s.lock().expect("sequences").alloc(seq)?),
                            &col.ty,
                        )
                        .map_err(|m| DbError {
                            code: "22003",
                            message: m,
                        })?,
                        (None, Some(v), _) => coerce(v.clone(), &col.ty).map_err(|m| DbError {
                            code: "22P02",
                            message: m,
                        })?,
                        _ => SqlValue::Null,
                    };
                    table.rows.get_mut(&id).expect("row").push(v);
                }
                table.cols.push(col.clone());
                if primary || unique {
                    table.indexes.push(new_index(
                        if primary {
                            format!("{key}_pkey")
                        } else {
                            format!("{key}_{}_key", col.name)
                        },
                        vec![pos],
                        true,
                        primary,
                    ));
                }
                rewrite = true;
            }
            AlterTableAction::DropColumn {
                name, if_exists, ..
            } => {
                let Some(i) = table.col_index(name) else {
                    if *if_exists {
                        continue;
                    }
                    return err(
                        "42703",
                        format!("column \"{name}\" of relation \"{key}\" does not exist"),
                    );
                };
                table.indexes.retain(|ix| !ix.cols.contains(&i));
                for ix in &mut table.indexes {
                    for c in &mut ix.cols {
                        if *c > i {
                            *c -= 1;
                        }
                    }
                }
                table.cols.remove(i);
                let ids: Vec<u64> = table.rows.keys().copied().collect();
                for id in ids {
                    table.rows.get_mut(&id).expect("row").remove(i);
                }
                rewrite = true;
            }
            AlterTableAction::RenameColumn { old_name, new_name } => {
                let Some(i) = table.col_index(old_name) else {
                    return err("42703", format!("column \"{old_name}\" does not exist"));
                };
                table.cols[i].name = new_name.to_lowercase();
            }
            AlterTableAction::RenameTable(n) => renamed_to = Some(n.to_lowercase()),
            AlterTableAction::AddConstraint(c) => match c {
                TableConstraint::PrimaryKey {
                    name: n, columns, ..
                }
                | TableConstraint::Unique {
                    name: n, columns, ..
                } => {
                    let primary = matches!(c, TableConstraint::PrimaryKey { .. });
                    let cols: Option<Vec<usize>> =
                        columns.iter().map(|c| table.col_index(c)).collect();
                    let Some(cols) = cols else {
                        return err("42703", "column named in key does not exist");
                    };
                    let iname = n.clone().unwrap_or_else(|| {
                        if primary {
                            format!("{key}_pkey")
                        } else {
                            format!("{key}_{}_key", columns.join("_").to_lowercase())
                        }
                    });
                    let mut idx = new_index(iname, cols, true, primary);
                    for (id, row) in &table.rows {
                        if let Some(k) = idx.key(row) {
                            let slot = idx.map.entry(k).or_default();
                            if !slot.is_empty() {
                                return err(
                                    "23505",
                                    format!(
                                        "could not create unique index \"{}\": key is duplicated",
                                        idx.name
                                    ),
                                );
                            }
                            slot.push(*id);
                        }
                    }
                    table.indexes.push(idx);
                }
                // Foreign keys and checks are not enforced yet.
                _ => {}
            },
            AlterTableAction::DropConstraint { name, .. } => {
                table.indexes.retain(|i| i.name != name.to_lowercase())
            }
            AlterTableAction::Owner(_) | AlterTableAction::SetSchema(_) => {}
            _ => return err("0A000", "this ALTER TABLE action is not supported"),
        }
    }
    if let Some(new) = &renamed_to {
        if st.tables.contains_key(new) {
            return err("42P07", format!("relation \"{new}\" already exists"));
        }
        table.name = new.clone();
    }
    // Indexes were built before rows were reshaped; rebuild so they match the final layout.
    table.rebuild_indexes();
    let mut extra = Vec::new();
    if rewrite {
        for (id, row) in &table.rows {
            extra.push(Op::PutRow {
                table: table.id,
                rowid: *id,
                row: row.clone(),
            });
        }
    }
    st.remove_table(&key);
    st.put_table(table);
    record(ops, text, extra);
    tag("ALTER TABLE")
}

fn truncate(
    st: &mut State,
    ops: &mut Option<&mut Vec<Op>>,
    tables: &[TableName],
) -> R<QueryResult> {
    for n in tables {
        let key = table_key(n);
        let Some(t) = st.tables.get_mut(&key) else {
            return err("42P01", format!("relation \"{key}\" does not exist"));
        };
        if let Some(o) = ops {
            o.push(Op::DropRows { table: t.id });
        }
        t.rows.clear();
        for i in &mut t.indexes {
            i.map.clear();
        }
    }
    tag("TRUNCATE TABLE")
}
