//! Foreign keys: declaration, checking on the child side, and the referential actions on the
//! parent side (`CASCADE`, `SET NULL`, `SET DEFAULT`, `RESTRICT` / `NO ACTION`).
//!
//! Every key gets a hidden hash index over its child columns, so finding the children of a parent
//! row, which every delete and key change needs, does not scan the child table.

use super::dml;
use super::value::{canonical, coerce, group_key};
use super::{err, DbError, Index, Op, Row, State, Table, R};
use crate::protocols::postgres_wire::sql::ast::{
    ColumnConstraint, ColumnDefinition, ReferentialAction, TableConstraint, TableName,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// How deep a chain of cascading actions may go before the statement is stopped.
const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl Action {
    fn from_ast(a: &Option<ReferentialAction>) -> Self {
        match a {
            None | Some(ReferentialAction::NoAction) => Action::NoAction,
            Some(ReferentialAction::Restrict) => Action::Restrict,
            Some(ReferentialAction::Cascade) => Action::Cascade,
            Some(ReferentialAction::SetNull) => Action::SetNull,
            Some(ReferentialAction::SetDefault) => Action::SetDefault,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Fk {
    pub name: String,
    /// Child columns, as positions in the owning table.
    pub cols: Vec<usize>,
    pub ref_table: String,
    /// Referenced columns, by name so a column rename in the parent can follow.
    pub ref_cols: Vec<String>,
    pub on_delete: Action,
    pub on_update: Action,
}

/// A foreign key as declared, before its tables are looked up.
pub(crate) struct Spec {
    pub name: Option<String>,
    pub cols: Vec<String>,
    pub ref_table: String,
    pub ref_cols: Vec<String>,
    pub on_delete: Action,
    pub on_update: Action,
}

fn table_name(n: &TableName) -> String {
    n.name.to_lowercase()
}

pub(crate) fn spec_from_constraint(c: &TableConstraint) -> Option<Spec> {
    match c {
        TableConstraint::ForeignKey {
            name,
            columns,
            references_table,
            references_columns,
            on_delete,
            on_update,
            ..
        } => Some(Spec {
            name: name.clone().map(|n| n.to_lowercase()),
            cols: columns.iter().map(|c| c.to_lowercase()).collect(),
            ref_table: table_name(references_table),
            ref_cols: references_columns
                .iter()
                .map(|c| c.to_lowercase())
                .collect(),
            on_delete: Action::from_ast(on_delete),
            on_update: Action::from_ast(on_update),
        }),
        _ => None,
    }
}

/// The foreign keys declared inline on a column (`col INTEGER REFERENCES t (id) ON DELETE CASCADE`).
pub(crate) fn specs_from_column(def: &ColumnDefinition) -> Vec<Spec> {
    def.constraints
        .iter()
        .filter_map(|c| match c {
            ColumnConstraint::References {
                table,
                columns,
                on_delete,
                on_update,
            } => Some(Spec {
                name: None,
                cols: vec![def.name.to_lowercase()],
                ref_table: table_name(table),
                ref_cols: columns
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|c| c.to_lowercase())
                    .collect(),
                on_delete: Action::from_ast(on_delete),
                on_update: Action::from_ast(on_update),
            }),
            _ => None,
        })
        .collect()
}

fn values_at(row: &Row, cols: &[usize]) -> Vec<SqlValue> {
    cols.iter().map(|&c| row[c].clone()).collect()
}

fn show(vals: &[SqlValue]) -> String {
    vals.iter()
        .map(SqlValue::to_postgres_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn ref_positions(parent: &Table, fk: &Fk) -> Option<Vec<usize>> {
    fk.ref_cols.iter().map(|n| parent.col_index(n)).collect()
}

/// Declare `spec` on table `child_key`, which must already be in `st`. Existing rows are checked.
pub(crate) fn add(st: &mut State, child_key: &str, spec: &Spec) -> R<()> {
    let Some(child) = st.tables.get(child_key) else {
        return err("42P01", format!("relation \"{child_key}\" does not exist"));
    };
    let cols: Vec<usize> = spec
        .cols
        .iter()
        .map(|n| {
            child.col_index(n).ok_or_else(|| DbError {
                code: "42703",
                message: format!(
                    "column \"{n}\" referenced in foreign key constraint does not exist"
                ),
            })
        })
        .collect::<R<_>>()?;
    let Some(parent) = st.tables.get(&spec.ref_table) else {
        return err(
            "42P01",
            format!("relation \"{}\" does not exist", spec.ref_table),
        );
    };
    let ref_cols: Vec<String> = if spec.ref_cols.is_empty() {
        match parent.indexes.iter().find(|i| i.primary) {
            Some(pk) => pk
                .cols
                .iter()
                .map(|&c| parent.cols[c].name.clone())
                .collect(),
            None => {
                return err(
                    "42830",
                    format!(
                        "there is no primary key for referenced table \"{}\"",
                        parent.name
                    ),
                )
            }
        }
    } else {
        spec.ref_cols.clone()
    };
    if ref_cols.len() != cols.len() {
        return err(
            "42830",
            "number of referencing and referenced columns for foreign key disagree",
        );
    }
    let pcols: Vec<usize> = ref_cols
        .iter()
        .map(|n| {
            parent.col_index(n).ok_or_else(|| DbError {
                code: "42703",
                message: format!(
                    "column \"{n}\" referenced in foreign key constraint does not exist"
                ),
            })
        })
        .collect::<R<_>>()?;
    if !parent.indexes.iter().any(|i| i.unique && i.cols == pcols) {
        return err(
            "42830",
            format!(
                "there is no unique constraint matching given keys for referenced table \"{}\"",
                parent.name
            ),
        );
    }
    let name = spec
        .name
        .clone()
        .unwrap_or_else(|| format!("{child_key}_{}_fkey", spec.cols.join("_")));
    if child.fks.iter().any(|f| f.name == name) {
        return err(
            "42710",
            format!("constraint \"{name}\" for relation \"{child_key}\" already exists"),
        );
    }
    let fk = Fk {
        name,
        cols,
        ref_table: spec.ref_table.clone(),
        ref_cols,
        on_delete: spec.on_delete,
        on_update: spec.on_update,
    };
    // the keys already in the table must satisfy the new constraint
    for row in child.rows.values() {
        check_row(st, child_key, &fk, row)?;
    }
    let child = st.tables.get_mut(child_key).expect("table");
    if !child.indexes.iter().any(|i| i.cols == fk.cols) {
        let mut idx = Index {
            name: format!("{}_fkidx", fk.name),
            cols: fk.cols.clone(),
            unique: false,
            primary: false,
            map: imbl::HashMap::new(),
        };
        for (id, row) in &child.rows {
            if let Some(k) = idx.key(row) {
                idx.map.entry(k).or_default().push(*id);
            }
        }
        child.indexes.push(idx);
    }
    child.fks.push(fk);
    Ok(())
}

/// Whether the parent table has a row whose referenced columns equal `vals`.
fn parent_has(parent: &Table, pcols: &[usize], vals: &[SqlValue]) -> bool {
    if let Some(idx) = parent.indexes.iter().find(|i| i.unique && i.cols == pcols) {
        return idx
            .key(&{
                // a row-shaped probe: only the indexed positions matter
                let mut probe = vec![SqlValue::Null; parent.cols.len()];
                for (&c, v) in pcols.iter().zip(vals) {
                    probe[c] = v.clone();
                }
                probe
            })
            .map_or(false, |k| idx.map.contains_key(&k));
    }
    let want = group_key(vals);
    parent
        .rows
        .values()
        .any(|r| group_key(&values_at(r, pcols)) == want)
}

fn check_row(st: &State, child_key: &str, fk: &Fk, row: &Row) -> R<()> {
    let vals = values_at(row, &fk.cols);
    if vals.iter().any(SqlValue::is_null) {
        return Ok(()); // MATCH SIMPLE: a NULL in any column satisfies the key
    }
    let Some(parent) = st.tables.get(&fk.ref_table) else {
        return Ok(());
    };
    let Some(pcols) = ref_positions(parent, fk) else {
        return Ok(());
    };
    if parent_has(parent, &pcols, &vals) {
        return Ok(());
    }
    err(
        "23503",
        format!(
            "insert or update on table \"{child_key}\" violates foreign key constraint \"{}\" (Key ({})=({}) is not present in table \"{}\".)",
            fk.name,
            fk.ref_cols.join(", "),
            show(&vals),
            fk.ref_table
        ),
    )
}

/// Check the keys of a row that was just inserted or changed.
pub(crate) fn check_child(st: &State, child_key: &str, row: &Row) -> R<()> {
    let Some(t) = st.tables.get(child_key) else {
        return Ok(());
    };
    for fk in &t.fks {
        check_row(st, child_key, fk, row)?;
    }
    Ok(())
}

/// Like [`check_child`] but only for keys whose columns differ between `old` and `new`.
pub(crate) fn check_child_changed(st: &State, child_key: &str, old: &Row, new: &Row) -> R<()> {
    let Some(t) = st.tables.get(child_key) else {
        return Ok(());
    };
    for fk in &t.fks {
        if values_at(old, &fk.cols)
            .iter()
            .zip(values_at(new, &fk.cols).iter())
            .any(|(a, b)| canonical(a) != canonical(b))
        {
            check_row(st, child_key, fk, new)?;
        }
    }
    Ok(())
}

/// Keys declared on other tables that point at `parent`, with the table that owns each.
#[derive(Default)]
pub(crate) struct Refs {
    cache: HashMap<String, Vec<(String, Fk)>>,
}

impl Refs {
    fn to(&mut self, st: &State, parent: &str) -> Vec<(String, Fk)> {
        self.cache
            .entry(parent.to_string())
            .or_insert_with(|| {
                st.tables
                    .iter()
                    .flat_map(|(name, t)| {
                        t.fks
                            .iter()
                            .filter(|f| f.ref_table == parent)
                            .map(|f| (name.clone(), f.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .clone()
    }
}

/// Child rows (other than `skip`) whose key columns equal `vals`.
fn children(child: &Table, fk: &Fk, vals: &[SqlValue], skip: Option<u64>) -> Vec<u64> {
    let mut probe = vec![SqlValue::Null; child.cols.len()];
    for (&c, v) in fk.cols.iter().zip(vals) {
        probe[c] = v.clone();
    }
    let ids: Vec<u64> = match child.indexes.iter().find(|i| i.cols == fk.cols) {
        Some(idx) => idx
            .key(&probe)
            .and_then(|k| idx.map.get(&k).cloned())
            .unwrap_or_default(),
        None => {
            let want = group_key(vals);
            child
                .rows
                .iter()
                .filter(|(_, r)| group_key(&values_at(r, &fk.cols)) == want)
                .map(|(id, _)| *id)
                .collect()
        }
    };
    ids.into_iter().filter(|id| Some(*id) != skip).collect()
}

fn violation(parent: &str, fk: &Fk, child: &str) -> DbError {
    DbError {
        code: "23503",
        message: format!(
            "update or delete on table \"{parent}\" violates foreign key constraint \"{}\" on table \"{child}\"",
            fk.name
        ),
    }
}

/// Run the referential actions for a parent row that is about to be deleted.
pub(crate) fn before_delete(
    st: &mut State,
    ops: &mut Vec<Op>,
    refs: &mut Refs,
    parent: &str,
    rowid: u64,
    old: &Row,
    depth: usize,
) -> R<()> {
    if depth > MAX_DEPTH {
        return err(
            "54001",
            "cascading foreign key actions are nested too deeply",
        );
    }
    for (child_key, fk) in refs.to(st, parent) {
        let Some(p) = st.tables.get(parent) else {
            return Ok(());
        };
        let Some(pcols) = ref_positions(p, &fk) else {
            continue;
        };
        let key = values_at(old, &pcols);
        if key.iter().any(SqlValue::is_null) {
            continue;
        }
        let Some(child) = st.tables.get(&child_key) else {
            continue;
        };
        let skip = (child_key == parent).then_some(rowid);
        for id in children(child, &fk, &key, skip) {
            let Some(row) = st
                .tables
                .get(&child_key)
                .and_then(|t| t.rows.get(&id))
                .cloned()
            else {
                continue; // already removed by an earlier cascade
            };
            match fk.on_delete {
                Action::Cascade => dml::delete_row(st, ops, refs, &child_key, id, depth + 1)?,
                Action::SetNull | Action::SetDefault => {
                    let new = reset_key(
                        st,
                        &child_key,
                        &fk,
                        &row,
                        fk.on_delete == Action::SetDefault,
                    )?;
                    dml::update_row(st, ops, refs, &child_key, id, new, depth + 1)?;
                }
                Action::NoAction | Action::Restrict => {
                    return Err(violation(parent, &fk, &child_key))
                }
            }
        }
    }
    Ok(())
}

/// Run the referential actions for a parent row whose key columns changed from `old` to `new`.
pub(crate) fn after_key_change(
    st: &mut State,
    ops: &mut Vec<Op>,
    refs: &mut Refs,
    parent: &str,
    rowid: u64,
    old: &Row,
    new: &Row,
    depth: usize,
) -> R<()> {
    if depth > MAX_DEPTH {
        return err(
            "54001",
            "cascading foreign key actions are nested too deeply",
        );
    }
    for (child_key, fk) in refs.to(st, parent) {
        let Some(p) = st.tables.get(parent) else {
            return Ok(());
        };
        let Some(pcols) = ref_positions(p, &fk) else {
            continue;
        };
        let (old_key, new_key) = (values_at(old, &pcols), values_at(new, &pcols));
        if old_key
            .iter()
            .zip(&new_key)
            .all(|(a, b)| canonical(a) == canonical(b))
            || old_key.iter().any(SqlValue::is_null)
        {
            continue;
        }
        let Some(child) = st.tables.get(&child_key) else {
            continue;
        };
        let skip = (child_key == parent).then_some(rowid);
        for id in children(child, &fk, &old_key, skip) {
            let Some(row) = st
                .tables
                .get(&child_key)
                .and_then(|t| t.rows.get(&id))
                .cloned()
            else {
                continue;
            };
            match fk.on_update {
                Action::Cascade => {
                    let mut changed = row.clone();
                    for (&c, v) in fk.cols.iter().zip(&new_key) {
                        changed[c] = v.clone();
                    }
                    dml::update_row(st, ops, refs, &child_key, id, changed, depth + 1)?;
                }
                Action::SetNull | Action::SetDefault => {
                    let changed = reset_key(
                        st,
                        &child_key,
                        &fk,
                        &row,
                        fk.on_update == Action::SetDefault,
                    )?;
                    dml::update_row(st, ops, refs, &child_key, id, changed, depth + 1)?;
                }
                Action::NoAction | Action::Restrict => {
                    return Err(violation(parent, &fk, &child_key))
                }
            }
        }
    }
    Ok(())
}

/// `row` with the key columns of `fk` set to NULL, or to their column defaults.
fn reset_key(st: &State, child_key: &str, fk: &Fk, row: &Row, default: bool) -> R<Row> {
    let t = st.tables.get(child_key).expect("table");
    let mut out = row.clone();
    for &c in &fk.cols {
        out[c] = SqlValue::Null;
        if default {
            if let Some(d) = &t.cols[c].default {
                let v = super::query::Runner::new(st).eval(d, &EvaluationContext::empty())?;
                out[c] = coerce(v, &t.cols[c].ty).map_err(|m| DbError {
                    code: "22P02",
                    message: m,
                })?;
            }
        }
    }
    Ok(out)
}

/// Keys on other tables that reference `table`.
pub(crate) fn referenced_by_others(st: &State, table: &str) -> Vec<(String, String)> {
    st.tables
        .iter()
        .filter(|(name, _)| name.as_str() != table)
        .flat_map(|(name, t)| {
            t.fks
                .iter()
                .filter(|f| f.ref_table == table)
                .map(|f| (name.clone(), f.name.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Remove the keys of other tables that point at `table` (for `DROP TABLE ... CASCADE`).
pub(crate) fn drop_references_to(st: &mut State, table: &str) {
    let owners: Vec<String> = st
        .tables
        .iter()
        .filter(|(_, t)| t.fks.iter().any(|f| f.ref_table == table))
        .map(|(n, _)| n.clone())
        .collect();
    for n in owners {
        let t = st.tables.get_mut(&n).expect("table");
        t.fks.retain(|f| f.ref_table != table);
    }
}
