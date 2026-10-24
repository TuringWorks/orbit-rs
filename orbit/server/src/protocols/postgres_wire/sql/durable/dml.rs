//! `INSERT`, `UPDATE` and `DELETE`.

use super::ddl::table_key;
use super::fk::{self, Refs};
use super::query::{ctx_for, rel_to_result, Rel, RelCol, Runner};
use super::value::coerce;
use super::{err, DbError, Op, QueryResult, Row, Seqs, State, Table, R};
use crate::protocols::postgres_wire::sql::ast::{
    Assignment, AssignmentTarget, ConflictAction, ConflictTarget, DeleteStatement, Expression,
    FromClause, InsertSource, InsertStatement, SelectItem, SelectStatement, UpdateStatement,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;
use std::sync::Mutex;

fn rel_cols(t: &Table, qual: &str) -> Vec<RelCol> {
    t.cols
        .iter()
        .map(|c| RelCol {
            qual: Some(qual.to_string()),
            name: c.name.clone(),
            ty: Some(c.ty.clone()),
        })
        .collect()
}

fn lookup<'a>(st: &'a State, key: &str) -> R<&'a Table> {
    match st.tables.get(key) {
        Some(t) => Ok(t),
        None => err("42P01", format!("relation \"{key}\" does not exist")),
    }
}

/// Evaluate a `RETURNING` list against `rows` of table `t`.
fn returning(st: &State, t: &Table, items: &[SelectItem], rows: Vec<Row>) -> R<Rel> {
    let mut r = Runner::new(st);
    r.with_cte(
        &t.name,
        Rel {
            cols: rel_cols(t, &t.name),
            rows,
        },
    );
    let sel = SelectStatement {
        with: None,
        select_list: items.to_vec(),
        distinct: None,
        from_clause: Some(FromClause::Table {
            name: crate::protocols::postgres_wire::sql::ast::TableName::new(t.name.clone()),
            alias: None,
            time_travel: None,
        }),
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
    r.select(&sel, None)
}

fn result(tag: &str, count: usize, ret: Option<Rel>) -> QueryResult {
    match ret {
        Some(rel) => match rel_to_result(rel) {
            QueryResult::Typed { columns, rows } => QueryResult::Returning {
                tag: format!("{tag}{count}"),
                columns,
                rows,
            },
            other => other,
        },
        None => match tag {
            t if t.starts_with("INSERT") => QueryResult::Insert { count },
            t if t.starts_with("UPDATE") => QueryResult::Update { count },
            _ => QueryResult::Delete { count },
        },
    }
}

fn not_null_check(t: &Table, row: &Row) -> R<()> {
    for (c, v) in t.cols.iter().zip(row) {
        if c.not_null && v.is_null() {
            return err(
                "23502",
                format!(
                    "null value in column \"{}\" of relation \"{}\" violates not-null constraint",
                    c.name, t.name
                ),
            );
        }
    }
    Ok(())
}

fn unique_violation(t: &Table, idx: &str) -> DbError {
    DbError {
        code: "23505",
        message: format!(
            "duplicate key value violates unique constraint \"{idx}\" on table \"{}\"",
            t.name
        ),
    }
}

/// The columns a statement's `RETURNING` list produces, without running the statement.
pub(crate) fn describe_returning(
    st: &State,
    table: &crate::protocols::postgres_wire::sql::ast::TableName,
    items: &[SelectItem],
) -> R<Vec<(String, i32)>> {
    let t = lookup(st, &table_key(table))?;
    let rel = returning(st, t, items, Vec::new())?;
    Ok(super::query::rel_columns(&rel))
}

pub(crate) fn insert(
    st: &mut State,
    ops: &mut Vec<Op>,
    seqs: &Mutex<Seqs>,
    ins: &InsertStatement,
) -> R<QueryResult> {
    let key = table_key(&ins.table);
    let mut refs = Refs::default();
    let (cols, ncols, tname) = {
        let t = lookup(st, &key)?;
        (t.cols.clone(), t.cols.len(), t.name.clone())
    };
    let target: Vec<usize> = match (&ins.columns, &ins.source) {
        (_, InsertSource::DefaultValues) => vec![],
        (Some(names), _) => names
            .iter()
            .map(|n| {
                cols.iter()
                    .position(|c| c.name == n.to_lowercase())
                    .ok_or_else(|| DbError {
                        code: "42703",
                        message: format!("column \"{n}\" of relation \"{tname}\" does not exist"),
                    })
            })
            .collect::<R<_>>()?,
        (None, _) => (0..ncols).collect(),
    };

    // plan: the values supplied by the statement
    let supplied: Vec<Vec<SqlValue>> = {
        let mut r = Runner::new(st);
        let empty = EvaluationContext::empty();
        match &ins.source {
            InsertSource::Values(rows) => {
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    if row.len() != target.len() {
                        return err(
                            "42601",
                            if row.len() > target.len() {
                                "INSERT has more expressions than target columns"
                            } else {
                                "INSERT has more target columns than expressions"
                            },
                        );
                    }
                    let mut vals = Vec::with_capacity(row.len());
                    for e in row {
                        vals.push(r.eval(e, &empty)?);
                    }
                    out.push(vals);
                }
                out
            }
            InsertSource::Query(sel) => {
                let rel = r.select(sel, None)?;
                if rel.cols.len() != target.len() {
                    return err("42601", "INSERT has more target columns than expressions");
                }
                rel.rows
            }
            InsertSource::DefaultValues => vec![vec![]],
        }
    };

    let mut inserted: Vec<Row> = Vec::new();
    let mut affected = 0usize;
    for vals in supplied {
        let mut row: Row = vec![SqlValue::Null; ncols];
        let mut given = vec![false; ncols];
        for (&i, v) in target.iter().zip(vals) {
            row[i] = coerce(v, &cols[i].ty).map_err(|m| DbError {
                code: "22P02",
                message: m,
            })?;
            given[i] = true;
        }
        for i in 0..ncols {
            if given[i] {
                continue;
            }
            if let Some(seq) = &cols[i].serial {
                let n = seqs.lock().expect("sequences").alloc(seq)?;
                row[i] = coerce(SqlValue::BigInt(n), &cols[i].ty).map_err(|m| DbError {
                    code: "22003",
                    message: m,
                })?;
            } else if let Some(d) = &cols[i].default {
                let v = Runner::new(st).eval(d, &EvaluationContext::empty())?;
                row[i] = coerce(v, &cols[i].ty).map_err(|m| DbError {
                    code: "22P02",
                    message: m,
                })?;
            }
        }
        {
            let t = lookup(st, &key)?;
            not_null_check(t, &row)?;
        }
        // uniqueness
        let conflict = {
            let t = lookup(st, &key)?;
            t.unique_conflict(&row, None)
                .map(|(i, id)| (i.name.clone(), i.cols.clone(), id))
        };
        if let Some((iname, icols, existing)) = conflict {
            let Some(oc) = &ins.on_conflict else {
                let t = lookup(st, &key)?;
                return Err(unique_violation(t, &iname));
            };
            // a conflict on a different unique index than the one named is still an error
            let matches_target = match &oc.target {
                Some(ConflictTarget::Columns(c)) => {
                    let want: Vec<usize> = c
                        .iter()
                        .filter_map(|n| cols.iter().position(|x| x.name == n.to_lowercase()))
                        .collect();
                    want == icols
                }
                Some(ConflictTarget::Constraint(n)) => n.to_lowercase() == iname,
                None => true,
            };
            if !matches_target {
                let t = lookup(st, &key)?;
                return Err(unique_violation(t, &iname));
            }
            match &oc.action {
                ConflictAction::DoNothing => continue,
                ConflictAction::DoUpdate { set, where_clause } => {
                    let old = lookup(st, &key)?
                        .rows
                        .get(&existing)
                        .cloned()
                        .expect("existing row");
                    let new = {
                        let t = lookup(st, &key)?;
                        let mut map: HashMap<String, SqlValue> = HashMap::new();
                        for (c, v) in t.cols.iter().zip(&row) {
                            map.insert(format!("excluded.{}", c.name), v.clone());
                        }
                        for (c, v) in t.cols.iter().zip(&old) {
                            map.insert(format!("{}.{}", t.name, c.name), v.clone());
                            map.insert(c.name.clone(), v.clone());
                        }
                        let ctx = EvaluationContext::with_row(map);
                        let mut r = Runner::new(st);
                        if let Some(w) = where_clause {
                            if !super::value::truthy(&r.eval(w, &ctx)?) {
                                continue;
                            }
                        }
                        apply_assignments(&mut r, t, &old, set, &ctx)?
                    };
                    let _ = old;
                    update_row(st, ops, &mut refs, &key, existing, new.clone(), 0)?;
                    affected += 1;
                    inserted.push(new);
                    continue;
                }
            }
        }
        let t = st.tables.get_mut(&key).expect("table");
        let rowid = t.next_rowid;
        t.next_rowid += 1;
        t.index_add(rowid, &row);
        t.rows.insert(rowid, row.clone());
        ops.push(Op::PutRow {
            table: t.id,
            rowid,
            row: row.clone(),
        });
        fk::check_child(st, &key, &row)?;
        affected += 1;
        inserted.push(row);
    }
    let ret = match &ins.returning {
        Some(items) => Some(returning(st, lookup(st, &key)?, items, inserted)?),
        None => None,
    };
    Ok(result("INSERT 0 ", affected, ret))
}

fn apply_assignments(
    r: &mut Runner,
    t: &Table,
    old: &Row,
    set: &[Assignment],
    ctx: &EvaluationContext,
) -> R<Row> {
    let mut new = old.clone();
    for a in set {
        let names: Vec<&String> = match &a.target {
            AssignmentTarget::Column(n) => vec![n],
            AssignmentTarget::Columns(ns) => ns.iter().collect(),
        };
        // `SET a = x` or `SET (a, b) = (x, y)` / `SET (a, b) = (SELECT ...)`
        let values: Vec<SqlValue> = if names.len() == 1 {
            vec![r.eval(&a.value, ctx)?]
        } else {
            match &a.value {
                Expression::Row(items) => items.iter().map(|e| r.eval(e, ctx)).collect::<R<_>>()?,
                Expression::Subquery(sel) => {
                    let rel = r.select(sel, Some(&ctx.current_row))?;
                    rel.rows
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| vec![SqlValue::Null; names.len()])
                }
                _ => {
                    return err(
                        "42601",
                        "a multi-column SET needs a row or a subquery on the right",
                    )
                }
            }
        };
        if values.len() != names.len() {
            return err("42601", "number of columns and values in SET do not match");
        }
        for (name, v) in names.into_iter().zip(values) {
            let Some(i) = t.col_index(name) else {
                return err(
                    "42703",
                    format!(
                        "column \"{name}\" of relation \"{}\" does not exist",
                        t.name
                    ),
                );
            };
            new[i] = coerce(v, &t.cols[i].ty).map_err(|m| DbError {
                code: "22P02",
                message: m,
            })?;
        }
    }
    not_null_check(t, &new)?;
    Ok(new)
}

/// Replace row `rowid` with `new`, keeping indexes consistent and recording undo and the write.
fn write_update(
    st: &mut State,
    ops: &mut Vec<Op>,
    key: &str,
    rowid: u64,
    old: Row,
    new: Row,
) -> R<()> {
    let t = st.tables.get_mut(key).expect("table");
    t.index_remove(rowid, &old);
    if let Some((idx, _)) = t.unique_conflict(&new, Some(rowid)) {
        let name = idx.name.clone();
        t.index_add(rowid, &old);
        return Err(unique_violation(t, &name));
    }
    t.index_add(rowid, &new);
    t.rows.insert(rowid, new.clone());
    ops.push(Op::PutRow {
        table: t.id,
        rowid,
        row: new,
    });
    Ok(())
}

/// Replace row `rowid` of `key` with `new`: unique and foreign key checks, then the actions on rows that reference it.
pub(crate) fn update_row(
    st: &mut State,
    ops: &mut Vec<Op>,
    refs: &mut Refs,
    key: &str,
    rowid: u64,
    new: Row,
    depth: usize,
) -> R<()> {
    let Some(old) = st.tables.get(key).and_then(|t| t.rows.get(&rowid)).cloned() else {
        return Ok(());
    };
    write_update(st, ops, key, rowid, old.clone(), new.clone())?;
    fk::check_child_changed(st, key, &old, &new)?;
    fk::after_key_change(st, ops, refs, key, rowid, &old, &new, depth)
}

/// Delete row `rowid` of `key`, running the actions of foreign keys that reference it first.
pub(crate) fn delete_row(
    st: &mut State,
    ops: &mut Vec<Op>,
    refs: &mut Refs,
    key: &str,
    rowid: u64,
    depth: usize,
) -> R<()> {
    let Some(old) = st.tables.get(key).and_then(|t| t.rows.get(&rowid)).cloned() else {
        return Ok(());
    };
    fk::before_delete(st, ops, refs, key, rowid, &old, depth)?;
    let t = st.tables.get_mut(key).expect("table");
    if let Some(row) = t.rows.remove(&rowid) {
        t.index_remove(rowid, &row);
        ops.push(Op::DelRow { table: t.id, rowid });
    }
    Ok(())
}

/// Rows of `t` matching `where_clause`, as `(row id, row)`, evaluated against the committed state.
fn matching(
    st: &State,
    t: &Table,
    qual: &str,
    where_clause: &Option<Expression>,
) -> R<Vec<(u64, Row)>> {
    let cols = rel_cols(t, qual);
    let mut r = Runner::new(st);
    let mut out = Vec::new();
    let ids: Vec<u64> = match where_clause {
        Some(w) => r
            .indexed_ids(t, w, qual)?
            .unwrap_or_else(|| t.rows.keys().copied().collect()),
        None => t.rows.keys().copied().collect(),
    };
    for id in ids {
        let Some(row) = t.rows.get(&id) else {
            continue;
        };
        let keep = match where_clause {
            Some(w) => super::value::truthy(&r.eval(w, &ctx_for(&cols, row, None))?),
            None => true,
        };
        if keep {
            out.push((id, row.clone()));
        }
    }
    Ok(out)
}

pub(crate) fn update(st: &mut State, ops: &mut Vec<Op>, upd: &UpdateStatement) -> R<QueryResult> {
    let key = table_key(&upd.table);
    let qual = upd
        .alias
        .clone()
        .unwrap_or_else(|| key.clone())
        .to_lowercase();
    let plan: Vec<(u64, Row, Row)> = {
        let t = lookup(st, &key)?;
        let cols = rel_cols(t, &qual);
        let mut r = Runner::new(st);
        let mut out = Vec::new();
        match &upd.from {
            None => {
                let hits = matching(st, t, &qual, &upd.where_clause)?;
                out.reserve(hits.len());
                for (id, old) in hits {
                    let ctx = ctx_for(&cols, &old, None);
                    let new = apply_assignments(&mut r, t, &old, &upd.set, &ctx)?;
                    out.push((id, old, new));
                }
            }
            Some(items) => {
                // UPDATE ... FROM: each target row is changed using the first joined row that satisfies WHERE
                let from = r.from_items(items)?;
                let mut all = cols.clone();
                all.extend(from.cols.iter().cloned());
                for (id, old) in &t.rows {
                    for frow in &from.rows {
                        let mut row = old.clone();
                        row.extend(frow.iter().cloned());
                        let ctx = ctx_for(&all, &row, None);
                        let keep = match &upd.where_clause {
                            Some(w) => super::value::truthy(&r.eval(w, &ctx)?),
                            None => true,
                        };
                        if keep {
                            let new = apply_assignments(&mut r, t, old, &upd.set, &ctx)?;
                            out.push((*id, old.clone(), new));
                            break;
                        }
                    }
                }
            }
        }
        out
    };
    let count = plan.len();
    let mut after = Vec::with_capacity(count);
    let mut refs = Refs::default();
    for (id, _old, new) in plan {
        update_row(st, ops, &mut refs, &key, id, new.clone(), 0)?;
        after.push(new);
    }
    let ret = match &upd.returning {
        Some(items) => Some(returning(st, lookup(st, &key)?, items, after)?),
        None => None,
    };
    Ok(result("UPDATE ", count, ret))
}

pub(crate) fn delete(st: &mut State, ops: &mut Vec<Op>, del: &DeleteStatement) -> R<QueryResult> {
    let key = table_key(&del.table);
    let qual = del
        .alias
        .clone()
        .unwrap_or_else(|| key.clone())
        .to_lowercase();
    let hits = {
        let t = lookup(st, &key)?;
        match &del.using {
            None => matching(st, t, &qual, &del.where_clause)?,
            Some(items) => {
                // DELETE ... USING: a row goes when some joined row satisfies WHERE
                let mut r = Runner::new(st);
                let from = r.from_items(items)?;
                let mut all = rel_cols(t, &qual);
                all.extend(from.cols.iter().cloned());
                let mut out = Vec::new();
                for (id, old) in &t.rows {
                    for frow in &from.rows {
                        let mut row = old.clone();
                        row.extend(frow.iter().cloned());
                        let ctx = ctx_for(&all, &row, None);
                        let keep = match &del.where_clause {
                            Some(w) => super::value::truthy(&r.eval(w, &ctx)?),
                            None => true,
                        };
                        if keep {
                            out.push((*id, old.clone()));
                            break;
                        }
                    }
                }
                out
            }
        }
    };
    let count = hits.len();
    let mut gone = Vec::with_capacity(count);
    let mut refs = Refs::default();
    for (id, row) in hits {
        delete_row(st, ops, &mut refs, &key, id, 0)?;
        gone.push(row);
    }
    let ret = match &del.returning {
        Some(items) => Some(returning(st, lookup(st, &key)?, items, gone)?),
        None => None,
    };
    Ok(result("DELETE ", count, ret))
}
