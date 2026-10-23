//! Schema statements: tables, indexes and column changes.

use super::{err, Col, Index, Op, Persist, QueryResult, State, Table, Txn, Undo, R};
use crate::protocols::postgres_wire::sql::ast::{
    AlterTableAction, ColumnConstraint, ColumnDefinition, CreateIndexStatement,
    CreateTableStatement, Expression, IndexType, Statement, TableConstraint, TableName,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::{
    EvaluationContext, ExpressionEvaluator,
};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::{BTreeMap, HashMap};

fn tag(t: &str) -> R<QueryResult> {
    Ok(QueryResult::Command { tag: t.to_string() })
}

/// Table names are matched case-insensitively and without a schema qualifier.
pub(crate) fn table_key(n: &TableName) -> String {
    n.name.to_lowercase()
}

/// Run a schema statement. `None` means the statement is not a schema statement.
/// `ctx` is absent while replaying the DDL log at start-up, when nothing is written or undone.
pub(crate) fn apply(
    st: &mut State,
    mut ctx: Option<(&mut Txn, &dyn Persist)>,
    stmt: &Statement,
    text: &str,
    replay: bool,
) -> Option<R<QueryResult>> {
    let r = match stmt {
        Statement::CreateTable(ct) => create_table(st, &mut ctx, ct, text, replay),
        Statement::DropTable(d) => drop_tables(st, &mut ctx, &d.names, d.if_exists, text),
        Statement::CreateIndex(ci) => create_index(st, &mut ctx, ci, text),
        Statement::DropIndex(d) => drop_indexes(st, &mut ctx, &d.names, d.if_exists, text),
        Statement::AlterTable(a) => alter_table(st, &mut ctx, &a.name, &a.actions, text, replay),
        Statement::Truncate(t) => truncate(st, &mut ctx, &t.tables, text),
        _ => return None,
    };
    Some(r)
}

fn record(ctx: &mut Option<(&mut Txn, &dyn Persist)>, text: &str, undo: Vec<Undo>, ops: Vec<Op>) {
    if let Some((txn, _)) = ctx {
        txn.undo.extend(undo);
        txn.ops.extend(ops);
        txn.ops.push(Op::Ddl(text.to_string()));
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

fn create_table(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
    ct: &CreateTableStatement,
    text: &str,
    replay: bool,
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
            indexes.push(Index {
                name: format!("{name}_pkey"),
                cols: vec![i],
                unique: true,
                primary: true,
                map: HashMap::new(),
            });
        } else if unique {
            indexes.push(Index {
                name: format!("{name}_{}_key", col.name),
                cols: vec![i],
                unique: true,
                primary: false,
                map: HashMap::new(),
            });
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
                indexes.push(Index {
                    name: iname,
                    cols: idx,
                    unique: true,
                    primary,
                    map: HashMap::new(),
                });
            }
            // Foreign keys and checks are not enforced yet.
            _ => {}
        }
    }
    let id = st.next_table_id;
    st.next_table_id += 1;
    let mut ops = Vec::new();
    for c in &cols {
        // While replaying, sequences already hold the values read from storage.
        if let (Some(seq), false) = (&c.serial, replay) {
            st.reset_seq(seq);
            ops.push(Op::SeqHighWater {
                name: seq.clone(),
                value: 0,
            });
        }
    }
    st.tables.insert(
        name.clone(),
        Table {
            id,
            name: name.clone(),
            cols,
            rows: BTreeMap::new(),
            next_rowid: 1,
            indexes,
        },
    );
    record(ctx, text, vec![Undo::Table { name, prev: None }], ops);
    tag("CREATE TABLE")
}

fn drop_tables(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
    names: &[TableName],
    if_exists: bool,
    text: &str,
) -> R<QueryResult> {
    let mut undo = Vec::new();
    let mut ops = Vec::new();
    for n in names {
        let key = table_key(n);
        match st.tables.remove(&key) {
            Some(t) => {
                ops.push(Op::DropRows { table: t.id });
                undo.push(Undo::Table {
                    name: key,
                    prev: Some(Box::new(t)),
                });
            }
            None if if_exists => {}
            None => {
                // restore anything already dropped by this statement before failing
                for u in undo.into_iter().rev() {
                    if let Undo::Table {
                        name,
                        prev: Some(t),
                    } = u
                    {
                        st.tables.insert(name, *t);
                    }
                }
                return err("42P01", format!("table \"{key}\" does not exist"));
            }
        }
    }
    record(ctx, text, undo, ops);
    tag("DROP TABLE")
}

fn create_index(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
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
    let before = table.clone();
    let mut idx = Index {
        name,
        cols,
        unique: ci.unique,
        primary: false,
        map: HashMap::new(),
    };
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
    record(
        ctx,
        text,
        vec![Undo::Table {
            name: key,
            prev: Some(Box::new(before)),
        }],
        vec![],
    );
    tag("CREATE INDEX")
}

fn drop_indexes(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
    names: &[String],
    if_exists: bool,
    text: &str,
) -> R<QueryResult> {
    let mut undo = Vec::new();
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
                undo.push(Undo::Table {
                    name: key,
                    prev: Some(Box::new(t.clone())),
                });
                t.indexes.retain(|i| i.name != n);
            }
            None if if_exists => {}
            None => return err("42704", format!("index \"{n}\" does not exist")),
        }
    }
    record(ctx, text, undo, vec![]);
    tag("DROP INDEX")
}

fn alter_table(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
    name: &TableName,
    actions: &[AlterTableAction],
    text: &str,
    replay: bool,
) -> R<QueryResult> {
    let key = table_key(name);
    let Some(before) = st.tables.get(&key).cloned() else {
        return err("42P01", format!("relation \"{key}\" does not exist"));
    };
    let persist = ctx.as_ref().map(|(_, p)| *p);
    let mut table = before.clone();
    let mut rewrite = false;
    let mut renamed_to: Option<String> = None;
    let mut seq_ops = Vec::new();
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
                if col.not_null && col.serial.is_none() && fill.is_none() && !table.rows.is_empty()
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
                if let (Some(seq), false) = (&col.serial, replay) {
                    st.reset_seq(seq);
                    seq_ops.push(Op::SeqHighWater {
                        name: seq.clone(),
                        value: 0,
                    });
                }
                let ids: Vec<u64> = table.rows.keys().copied().collect();
                for id in ids {
                    let v = match (&col.serial, &fill) {
                        (Some(seq), _) => {
                            let p = persist.ok_or_else(|| super::DbError {
                                code: "XX000",
                                message: "no storage while replaying".into(),
                            })?;
                            SqlValue::BigInt(st.nextval(seq, p)?)
                        }
                        (None, Some(v)) => {
                            super::value::coerce(v.clone(), &col.ty).map_err(|m| {
                                super::DbError {
                                    code: "22P02",
                                    message: m,
                                }
                            })?
                        }
                        _ => SqlValue::Null,
                    };
                    let v = if col.serial.is_some() {
                        super::value::coerce(v, &col.ty).map_err(|m| super::DbError {
                            code: "22003",
                            message: m,
                        })?
                    } else {
                        v
                    };
                    table.rows.get_mut(&id).expect("row").push(v);
                }
                table.cols.push(col.clone());
                if primary || unique {
                    table.indexes.push(Index {
                        name: if primary {
                            format!("{key}_pkey")
                        } else {
                            format!("{key}_{}_key", col.name)
                        },
                        cols: vec![pos],
                        unique: true,
                        primary,
                        map: HashMap::new(),
                    });
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
                for row in table.rows.values_mut() {
                    row.remove(i);
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
            AlterTableAction::AddConstraint(c) => {
                match c {
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
                        let mut idx = Index {
                            name: iname,
                            cols,
                            unique: true,
                            primary,
                            map: HashMap::new(),
                        };
                        for (id, row) in &table.rows {
                            if let Some(k) = idx.key(row) {
                                let slot = idx.map.entry(k).or_default();
                                if !slot.is_empty() {
                                    return err("23505", format!("could not create unique index \"{}\": key is duplicated", idx.name));
                                }
                                slot.push(*id);
                            }
                        }
                        table.indexes.push(idx);
                    }
                    // Foreign keys and checks are not enforced yet.
                    _ => {}
                }
            }
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
    let mut undo = vec![Undo::Table {
        name: key.clone(),
        prev: Some(Box::new(before)),
    }];
    let mut ops = seq_ops;
    st.tables.remove(&key);
    let final_key = renamed_to.clone().unwrap_or_else(|| key.clone());
    if rewrite {
        for (id, row) in &table.rows {
            ops.push(Op::PutRow {
                table: table.id,
                rowid: *id,
                row: row.clone(),
            });
        }
    }
    if renamed_to.is_some() {
        undo.push(Undo::Table {
            name: final_key.clone(),
            prev: None,
        });
    }
    // Indexes were built before rows were reshaped; rebuild so they match the final layout.
    table.rebuild_indexes();
    st.tables.insert(final_key, table);
    record(ctx, text, undo, ops);
    tag("ALTER TABLE")
}

fn truncate(
    st: &mut State,
    ctx: &mut Option<(&mut Txn, &dyn Persist)>,
    tables: &[TableName],
    text: &str,
) -> R<QueryResult> {
    let mut undo = Vec::new();
    let mut ops = Vec::new();
    for n in tables {
        let key = table_key(n);
        let Some(t) = st.tables.get_mut(&key) else {
            return err("42P01", format!("relation \"{key}\" does not exist"));
        };
        undo.push(Undo::Table {
            name: key,
            prev: Some(Box::new(t.clone())),
        });
        ops.push(Op::DropRows { table: t.id });
        t.rows.clear();
        for i in &mut t.indexes {
            i.map.clear();
        }
    }
    // TRUNCATE changes data, not schema, so it is not replayed from the DDL log.
    if let Some((txn, _)) = ctx {
        txn.undo.extend(undo);
        txn.ops.extend(ops);
    }
    let _ = text;
    tag("TRUNCATE TABLE")
}
