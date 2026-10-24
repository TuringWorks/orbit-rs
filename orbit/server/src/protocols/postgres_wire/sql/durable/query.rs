//! `SELECT` execution over the committed tables.

use super::value::{canonical, cmp, group_key, oid_of_type, oid_of_value, truthy, unique_key};
use super::{err, Row, State, Table, R};
use crate::protocols::postgres_wire::query_engine::QueryResult;
use crate::protocols::postgres_wire::sql::ast::{
    BinaryOperator, CaseExpression, DistinctClause, Expression, FromClause, FunctionCall,
    FunctionName, InList, JoinCondition, JoinType, NullsOrder, OrderByItem, SelectItem,
    SelectStatement, SetOperator, SortDirection, WhenClause,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::{
    EvaluationContext, ExpressionEvaluator,
};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// One column of an intermediate relation.
#[derive(Clone, Debug)]
pub(crate) struct RelCol {
    pub qual: Option<String>,
    pub name: String,
    pub ty: Option<SqlType>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Rel {
    pub cols: Vec<RelCol>,
    pub rows: Vec<Row>,
}

pub(super) type Outer<'a> = Option<&'a HashMap<String, SqlValue>>;

/// Evaluation context for one row: every column under `name` and `qualifier.name`, then the outer query's columns.
pub(crate) fn ctx_for(cols: &[RelCol], row: &[SqlValue], outer: Outer) -> EvaluationContext {
    let mut map: HashMap<String, SqlValue> = HashMap::with_capacity(cols.len() * 2);
    for (c, v) in cols.iter().zip(row) {
        if let Some(q) = &c.qual {
            map.insert(format!("{q}.{}", c.name), v.clone());
        }
        map.entry(c.name.clone()).or_insert_with(|| v.clone());
    }
    if let Some(o) = outer {
        for (k, v) in o {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    EvaluationContext::with_row(map)
}

fn fname(f: &FunctionCall) -> String {
    match &f.name {
        FunctionName::Simple(s) => s.to_lowercase(),
        FunctionName::Qualified { name, .. } => name.to_lowercase(),
    }
}

fn is_agg_name(n: &str) -> bool {
    matches!(
        n,
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "string_agg"
            | "array_agg"
            | "bool_and"
            | "bool_or"
            | "every"
    )
}

/// Visit `e` and its sub-expressions (not entering subqueries).
fn walk(e: &Expression, f: &mut dyn FnMut(&Expression)) {
    f(e);
    match e {
        Expression::Binary { left, right, .. } => {
            walk(left, f);
            walk(right, f);
        }
        Expression::Unary { operand, .. } => walk(operand, f),
        Expression::Function(c) => {
            c.args.iter().for_each(|a| walk(a, f));
            if let Some(fl) = &c.filter {
                walk(fl, f);
            }
        }
        Expression::Case(c) => {
            if let Some(o) = &c.operand {
                walk(o, f);
            }
            for w in &c.when_clauses {
                walk(&w.condition, f);
                walk(&w.result, f);
            }
            if let Some(el) = &c.else_clause {
                walk(el, f);
            }
        }
        Expression::In { expr, list, .. } => {
            walk(expr, f);
            if let InList::Expressions(l) = list {
                l.iter().for_each(|x| walk(x, f));
            }
        }
        Expression::Between {
            expr, low, high, ..
        } => {
            walk(expr, f);
            walk(low, f);
            walk(high, f);
        }
        Expression::Like { expr, pattern, .. } => {
            walk(expr, f);
            walk(pattern, f);
        }
        Expression::IsNull { expr, .. } | Expression::Cast { expr, .. } => walk(expr, f),
        Expression::Array(v) | Expression::Row(v) => v.iter().for_each(|x| walk(x, f)),
        Expression::Any(x) | Expression::All(x) | Expression::Some(x) => walk(x, f),
        _ => {}
    }
}

/// Rebuild `e`, replacing any node for which `f` returns `Some`. `cx` is handed to `f` so the
/// callback can use the runner without borrowing it.
fn map_expr<C>(
    e: &Expression,
    cx: &mut C,
    f: &mut dyn FnMut(&mut C, &Expression) -> R<Option<Expression>>,
) -> R<Expression> {
    if let Some(r) = f(cx, e)? {
        return Ok(r);
    }
    fn bx<C>(
        x: &Expression,
        cx: &mut C,
        f: &mut dyn FnMut(&mut C, &Expression) -> R<Option<Expression>>,
    ) -> R<Box<Expression>> {
        map_expr(x, cx, f).map(Box::new)
    }
    Ok(match e {
        Expression::Binary {
            left,
            operator,
            right,
        } => Expression::Binary {
            left: bx(left, cx, f)?,
            operator: operator.clone(),
            right: bx(right, cx, f)?,
        },
        Expression::Unary { operator, operand } => Expression::Unary {
            operator: operator.clone(),
            operand: bx(operand, cx, f)?,
        },
        Expression::Function(c) => {
            let mut c2 = (**c).clone();
            c2.args = c
                .args
                .iter()
                .map(|a| map_expr(a, cx, f))
                .collect::<R<_>>()?;
            if let Some(fl) = &c.filter {
                c2.filter = Some(bx(fl, cx, f)?);
            }
            Expression::Function(Box::new(c2))
        }
        Expression::Case(c) => Expression::Case(CaseExpression {
            operand: c.operand.as_ref().map(|o| bx(o, cx, f)).transpose()?,
            when_clauses: c
                .when_clauses
                .iter()
                .map(|w| {
                    Ok(WhenClause {
                        condition: bx(&w.condition, cx, f)?,
                        result: bx(&w.result, cx, f)?,
                    })
                })
                .collect::<R<_>>()?,
            else_clause: c.else_clause.as_ref().map(|o| bx(o, cx, f)).transpose()?,
        }),
        Expression::In {
            expr,
            list,
            negated,
        } => Expression::In {
            expr: bx(expr, cx, f)?,
            list: match list {
                InList::Expressions(l) => {
                    InList::Expressions(l.iter().map(|x| map_expr(x, cx, f)).collect::<R<_>>()?)
                }
                other => other.clone(),
            },
            negated: *negated,
        },
        Expression::Between {
            expr,
            low,
            high,
            negated,
        } => Expression::Between {
            expr: bx(expr, cx, f)?,
            low: bx(low, cx, f)?,
            high: bx(high, cx, f)?,
            negated: *negated,
        },
        Expression::Like {
            expr,
            pattern,
            escape,
            case_insensitive,
            negated,
        } => Expression::Like {
            expr: bx(expr, cx, f)?,
            pattern: bx(pattern, cx, f)?,
            escape: escape.clone(),
            case_insensitive: *case_insensitive,
            negated: *negated,
        },
        Expression::IsNull { expr, negated } => Expression::IsNull {
            expr: bx(expr, cx, f)?,
            negated: *negated,
        },
        Expression::Cast { expr, target_type } => Expression::Cast {
            expr: bx(expr, cx, f)?,
            target_type: target_type.clone(),
        },
        Expression::Array(v) => {
            Expression::Array(v.iter().map(|x| map_expr(x, cx, f)).collect::<R<_>>()?)
        }
        Expression::Row(v) => {
            Expression::Row(v.iter().map(|x| map_expr(x, cx, f)).collect::<R<_>>()?)
        }
        other => other.clone(),
    })
}

fn contains_agg(e: &Expression) -> bool {
    let mut found = false;
    walk(e, &mut |x| {
        if let Expression::Function(c) = x {
            if is_agg_name(&fname(c)) {
                found = true;
            }
        }
    });
    found
}

/// Functions whose value is a property of the session or server, not of the data.
fn session_function(f: &FunctionCall) -> Option<SqlValue> {
    if !f.args.is_empty() {
        return None;
    }
    Some(SqlValue::Text(match fname(f).as_str() {
        "current_schema" => "public".into(),
        "current_database" => "orbit".into(),
        "current_user" | "session_user" | "user" => "orbit".into(),
        "version" => "PostgreSQL 18.0 (orbit-rs)".into(),
        _ => return None,
    }))
}

/// The window function calls inside `e`, each listed once.
fn collect_windows(e: &Expression, into: &mut Vec<Expression>) {
    walk(e, &mut |x| {
        if matches!(x, Expression::WindowFunction { .. }) && !into.contains(x) {
            into.push(x.clone());
        }
    });
}

/// `e` with each window function call replaced by its value for row `i`.
fn substitute_windows(
    e: &Expression,
    wins: &[Expression],
    vals: &[Vec<SqlValue>],
    i: usize,
) -> R<Expression> {
    map_expr(e, &mut (), &mut |_, x| {
        Ok(match x {
            Expression::WindowFunction { .. } => wins
                .iter()
                .position(|w| w == x)
                .map(|k| Expression::Literal(vals[k][i].clone())),
            _ => None,
        })
    })
}

fn has_subquery(e: &Expression) -> bool {
    let mut found = false;
    walk(e, &mut |x| {
        let session = matches!(x, Expression::Function(f) if session_function(f).is_some());
        if session
            || matches!(
                x,
                Expression::Subquery(_)
                    | Expression::Exists(_)
                    | Expression::In {
                        list: InList::Subquery(_),
                        ..
                    }
            )
        {
            found = true;
        }
    });
    found
}

/// The type an expression yields, when it can be known without running it.
pub(crate) fn expr_type(e: &Expression, cols: &[RelCol]) -> Option<SqlType> {
    use crate::protocols::postgres_wire::sql::ast::UnaryOperator;
    let numeric = |a: Option<SqlType>, b: Option<SqlType>| -> Option<SqlType> {
        let rank = |t: &SqlType| match t {
            SqlType::SmallInt => 1,
            SqlType::Integer => 2,
            SqlType::BigInt => 3,
            SqlType::Decimal { .. } | SqlType::Numeric { .. } => 4,
            SqlType::Real => 5,
            SqlType::DoublePrecision => 6,
            _ => 0,
        };
        let (a, b) = (a?, b?);
        if rank(&a) == 0 || rank(&b) == 0 {
            return None;
        }
        Some(if rank(&a) >= rank(&b) { a } else { b })
    };
    match e {
        Expression::Literal(v) => (!v.is_null()).then(|| v.sql_type()),
        Expression::Column(c) => {
            find_col(cols, c.table.as_deref(), &c.name).and_then(|i| cols[i].ty.clone())
        }
        Expression::Cast { target_type, .. } => Some(target_type.clone()),
        Expression::Binary {
            left,
            operator,
            right,
        } => match operator {
            BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => numeric(expr_type(left, cols), expr_type(right, cols)),
            BinaryOperator::Concat => Some(SqlType::Text),
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::LessThan
            | BinaryOperator::LessThanOrEqual
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterThanOrEqual
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::Like
            | BinaryOperator::ILike
            | BinaryOperator::NotLike
            | BinaryOperator::Is
            | BinaryOperator::IsNot
            | BinaryOperator::In
            | BinaryOperator::NotIn => Some(SqlType::Boolean),
            _ => None,
        },
        Expression::Unary { operator, operand } => match operator {
            UnaryOperator::Not
            | UnaryOperator::IsNull
            | UnaryOperator::IsNotNull
            | UnaryOperator::IsTrue
            | UnaryOperator::IsNotTrue
            | UnaryOperator::IsFalse => Some(SqlType::Boolean),
            UnaryOperator::Minus | UnaryOperator::Plus => expr_type(operand, cols),
            _ => None,
        },
        Expression::IsNull { .. }
        | Expression::Exists(_)
        | Expression::In { .. }
        | Expression::Between { .. }
        | Expression::Like { .. } => Some(SqlType::Boolean),
        Expression::Case(c) => c
            .when_clauses
            .first()
            .and_then(|w| expr_type(&w.result, cols)),
        Expression::WindowFunction { function, .. } => {
            use crate::protocols::postgres_wire::sql::ast::WindowFunctionType as W;
            match function {
                W::RowNumber | W::Rank | W::DenseRank => Some(SqlType::BigInt),
                W::Ntile(_) => Some(SqlType::Integer),
                W::PercentRank | W::CumeDist => Some(SqlType::DoublePrecision),
                W::Lag { expr, .. }
                | W::Lead { expr, .. }
                | W::FirstValue(expr)
                | W::LastValue(expr) => expr_type(expr, cols),
                W::NthValue { expr, .. } => expr_type(expr, cols),
                W::Aggregate(call) => expr_type(&Expression::Function(call.clone()), cols),
            }
        }
        Expression::CurrentTimestamp(_) | Expression::LocalTimestamp(_) => {
            Some(SqlType::Timestamp {
                with_timezone: true,
            })
        }
        Expression::CurrentDate => Some(SqlType::Date),
        Expression::Function(f) => {
            let first = f.args.first().and_then(|a| expr_type(a, cols));
            match fname(f).as_str() {
                "count" => Some(SqlType::BigInt),
                "sum" => match first {
                    Some(SqlType::SmallInt | SqlType::Integer) => Some(SqlType::BigInt),
                    Some(SqlType::BigInt | SqlType::Decimal { .. } | SqlType::Numeric { .. }) => {
                        Some(SqlType::Numeric {
                            precision: None,
                            scale: None,
                        })
                    }
                    other => other,
                },
                "avg" => match first {
                    Some(SqlType::Real | SqlType::DoublePrecision) => {
                        Some(SqlType::DoublePrecision)
                    }
                    _ => Some(SqlType::Numeric {
                        precision: None,
                        scale: None,
                    }),
                },
                "min" | "max" | "coalesce" | "nullif" | "greatest" | "least" | "abs" | "round"
                | "ceil" | "floor" => first,
                "bool_and" | "bool_or" | "every" => Some(SqlType::Boolean),
                "string_agg" | "lower" | "upper" | "trim" | "ltrim" | "rtrim" | "concat"
                | "concat_ws" | "substr" | "substring" | "replace" | "left" | "right"
                | "to_char" | "format" | "initcap" | "md5" => Some(SqlType::Text),
                "length" | "char_length" | "strpos" | "position" => Some(SqlType::Integer),
                "now" | "current_timestamp" => Some(SqlType::Timestamp {
                    with_timezone: true,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

enum OrderKey {
    Out(usize),
    Expr(Expression),
}

/// Runs queries against a fixed view of the committed state.
pub(crate) struct Runner<'a> {
    pub st: &'a State,
    ev: ExpressionEvaluator,
    ctes: HashMap<String, Rel>,
    /// Only the shape of the result is wanted: tables are read as empty.
    describe: bool,
}

pub(crate) fn run_select_statement(st: &State, sel: &SelectStatement) -> R<QueryResult> {
    let mut r = Runner::new(st);
    let rel = r.select(sel, None)?;
    Ok(rel_to_result(rel))
}

pub(crate) fn rel_columns(rel: &Rel) -> Vec<(String, i32)> {
    rel.cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let oid = match &c.ty {
                Some(t) => oid_of_type(t),
                None => rel
                    .rows
                    .iter()
                    .find_map(|r| (!r[i].is_null()).then(|| oid_of_value(&r[i])))
                    .unwrap_or(25),
            };
            (c.name.clone(), oid)
        })
        .collect()
}

/// The columns (name and type OID) a `SELECT` returns, without running it.
pub(crate) fn describe_select(st: &State, sel: &SelectStatement) -> R<Vec<(String, i32)>> {
    let mut r = Runner::describing(st);
    Ok(rel_columns(&r.select(sel, None)?))
}

pub(crate) fn rel_to_result(rel: Rel) -> QueryResult {
    let columns = rel_columns(&rel);
    let rows = rel
        .rows
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|v| {
                    if v.is_null() {
                        None
                    } else {
                        Some(v.to_postgres_string())
                    }
                })
                .collect()
        })
        .collect();
    QueryResult::Typed { columns, rows }
}

impl<'a> Runner<'a> {
    pub fn new(st: &'a State) -> Self {
        Self {
            st,
            ev: ExpressionEvaluator::new(),
            ctes: HashMap::new(),
            describe: false,
        }
    }

    /// A runner that reads every table as empty, for finding a statement's result columns.
    pub fn describing(st: &'a State) -> Self {
        let mut r = Self::new(st);
        r.describe = true;
        r
    }

    /// Make `rel` visible to queries under `name`, as a `WITH` clause would.
    pub fn with_cte(&mut self, name: &str, rel: Rel) {
        self.ctes.insert(name.to_lowercase(), rel);
    }

    /// Evaluate `expr`, running any subqueries it contains first.
    pub fn eval(&mut self, expr: &Expression, ctx: &EvaluationContext) -> R<SqlValue> {
        if has_subquery(expr) {
            let e = self.resolve_subqueries(expr, ctx)?;
            return Ok(self.ev.evaluate(&e, ctx)?);
        }
        Ok(self.ev.evaluate(expr, ctx)?)
    }

    fn resolve_subqueries(&mut self, expr: &Expression, ctx: &EvaluationContext) -> R<Expression> {
        let outer = ctx.current_row.clone();
        map_expr(expr, self, &mut |me: &mut Self, e: &Expression| {
            Ok(match e {
                Expression::Subquery(s) => {
                    let rel = me.select(s, Some(&outer))?;
                    if rel.rows.len() > 1 {
                        return err(
                            "21000",
                            "more than one row returned by a subquery used as an expression",
                        );
                    }
                    Some(Expression::Literal(
                        rel.rows
                            .first()
                            .and_then(|r| r.first().cloned())
                            .unwrap_or(SqlValue::Null),
                    ))
                }
                Expression::Exists(s) => {
                    let rel = me.select(s, Some(&outer))?;
                    Some(Expression::Literal(SqlValue::Boolean(!rel.rows.is_empty())))
                }
                Expression::Function(f) if session_function(f).is_some() => {
                    session_function(f).map(Expression::Literal)
                }
                Expression::In {
                    expr,
                    list: InList::Subquery(s),
                    negated,
                } => {
                    let rel = me.select(s, Some(&outer))?;
                    let items = rel
                        .rows
                        .into_iter()
                        .map(|r| {
                            Expression::Literal(r.into_iter().next().unwrap_or(SqlValue::Null))
                        })
                        .collect();
                    Some(Expression::In {
                        expr: expr.clone(),
                        list: InList::Expressions(items),
                        negated: *negated,
                    })
                }
                _ => None,
            })
        })
    }

    fn add_ctes(&mut self, sel: &SelectStatement, outer: Outer) -> R<Vec<String>> {
        let mut added = Vec::new();
        let Some(with) = &sel.with else {
            return Ok(added);
        };
        for cte in &with.ctes {
            let name = cte.name.to_lowercase();
            let rel = match (&cte.query.set_operation, with.recursive) {
                (Some(op), true)
                    if matches!(op.operator, SetOperator::Union | SetOperator::UnionAll) =>
                {
                    let mut base_q = (*cte.query).clone();
                    base_q.set_operation = None;
                    let mut acc = self.select(&base_q, outer)?;
                    rename_cols(&mut acc, cte.columns.as_deref(), &name);
                    let distinct = matches!(op.operator, SetOperator::Union);
                    let mut seen: HashSet<String> = acc.rows.iter().map(|r| group_key(r)).collect();
                    let mut frontier = acc.clone();
                    for _ in 0..10_000 {
                        self.ctes.insert(name.clone(), frontier.clone());
                        let mut next = self.select(&op.right, outer)?;
                        rename_cols(&mut next, cte.columns.as_deref(), &name);
                        next.rows.retain(|r| !distinct || seen.insert(group_key(r)));
                        if next.rows.is_empty() {
                            break;
                        }
                        acc.rows.extend(next.rows.iter().cloned());
                        frontier = next;
                    }
                    acc
                }
                _ => {
                    let mut rel = self.select(&cte.query, outer)?;
                    rename_cols(&mut rel, cte.columns.as_deref(), &name);
                    rel
                }
            };
            self.ctes.insert(name.clone(), rel);
            added.push(name);
        }
        Ok(added)
    }

    pub fn select(&mut self, sel: &SelectStatement, outer: Outer) -> R<Rel> {
        let added = self.add_ctes(sel, outer)?;
        let out = if let Some(op) = &sel.set_operation {
            self.set_operation(sel, op, outer)
        } else {
            self.select_core(sel, outer)
        };
        for n in added {
            self.ctes.remove(&n);
        }
        out
    }

    fn set_operation(
        &mut self,
        sel: &SelectStatement,
        op: &crate::protocols::postgres_wire::sql::ast::SetOperation,
        outer: Outer,
    ) -> R<Rel> {
        let mut left_q = sel.clone();
        left_q.set_operation = None;
        left_q.with = None;
        left_q.order_by = None;
        left_q.limit = None;
        left_q.offset = None;
        let left = self.select_core(&left_q, outer)?;
        let right = self.select(&op.right, outer)?;
        if left.cols.len() != right.cols.len() {
            return err(
                "42601",
                "each UNION query must have the same number of columns",
            );
        }
        let mut rows = left.rows.clone();
        let keys = |rows: &[Row]| rows.iter().map(|r| group_key(r)).collect::<HashSet<_>>();
        match op.operator {
            SetOperator::UnionAll => rows.extend(right.rows),
            SetOperator::Union => {
                rows.extend(right.rows);
                dedupe(&mut rows);
            }
            SetOperator::Intersect | SetOperator::IntersectAll => {
                let r = keys(&right.rows);
                rows.retain(|x| r.contains(&group_key(x)));
                if matches!(op.operator, SetOperator::Intersect) {
                    dedupe(&mut rows);
                }
            }
            SetOperator::Except | SetOperator::ExceptAll => {
                let r = keys(&right.rows);
                rows.retain(|x| !r.contains(&group_key(x)));
                if matches!(op.operator, SetOperator::Except) {
                    dedupe(&mut rows);
                }
            }
        }
        let mut rel = Rel {
            cols: left.cols,
            rows,
        };
        // ORDER BY / LIMIT of the first query apply to the combined result, by output column.
        if let Some(ob) = &sel.order_by {
            let keys: Vec<(OrderKey, &OrderByItem)> = ob
                .iter()
                .map(|o| Ok((resolve_order(&o.expression, &rel.cols, &[]), o)))
                .collect::<R<_>>()?;
            let mut keyed: Vec<(Vec<SqlValue>, Row)> = Vec::new();
            for r in rel.rows.drain(..) {
                let mut k = Vec::new();
                for (key, _) in &keys {
                    k.push(match key {
                        OrderKey::Out(i) => r[*i].clone(),
                        OrderKey::Expr(e) => {
                            let c = ctx_for(&rel.cols, &r, None);
                            self.eval(e, &c)?
                        }
                    });
                }
                keyed.push((k, r));
            }
            sort_keyed(
                &mut keyed,
                &keys.iter().map(|(_, o)| *o).collect::<Vec<_>>(),
            );
            rel.rows = keyed.into_iter().map(|(_, r)| r).collect();
        }
        self.apply_limit(&mut rel.rows, sel)?;
        Ok(rel)
    }

    fn apply_limit(&mut self, rows: &mut Vec<Row>, sel: &SelectStatement) -> R<()> {
        let empty = EvaluationContext::empty();
        if let Some(o) = sel.offset {
            let o = o as usize;
            if o >= rows.len() {
                rows.clear();
            } else {
                rows.drain(..o);
            }
        }
        if let Some(l) = &sel.limit {
            if let Some(e) = &l.count {
                let n = self.eval(e, &empty)?;
                let n = super::value::as_i64(&n).ok_or_else(|| super::DbError {
                    code: "22023",
                    message: "LIMIT must be a number".into(),
                })?;
                if n < 0 {
                    return err("2201W", "LIMIT must not be negative");
                }
                rows.truncate(n as usize);
            }
        }
        Ok(())
    }

    fn select_core(&mut self, sel: &SelectStatement, outer: Outer) -> R<Rel> {
        let src = match &sel.from_clause {
            Some(f) => match self.indexed_from(f, sel.where_clause.as_ref())? {
                Some(rel) => rel,
                None => self.from(f, outer)?,
            },
            None => Rel {
                cols: vec![],
                rows: vec![vec![]],
            },
        };
        let mut rows = Vec::with_capacity(src.rows.len());
        match &sel.where_clause {
            Some(w) => {
                for r in src.rows {
                    let c = ctx_for(&src.cols, &r, outer);
                    if truthy(&self.eval(w, &c)?) {
                        rows.push(r);
                    }
                }
            }
            None => rows = src.rows,
        }
        let cols = src.cols;

        // output column plan
        enum Out {
            Src(usize),
            Expr(Expression),
        }
        let mut plan: Vec<(String, Option<SqlType>, Out)> = Vec::new();
        for item in &sel.select_list {
            match item {
                SelectItem::Wildcard => {
                    for (i, c) in cols.iter().enumerate() {
                        plan.push((c.name.clone(), c.ty.clone(), Out::Src(i)));
                    }
                }
                SelectItem::QualifiedWildcard { qualifier } => {
                    let q = qualifier.to_lowercase();
                    for (i, c) in cols
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.qual.as_deref() == Some(q.as_str()))
                    {
                        plan.push((c.name.clone(), c.ty.clone(), Out::Src(i)));
                    }
                }
                SelectItem::Expression { expr, alias } => {
                    let ty = expr_type(expr, &cols);
                    let name = match (alias, expr) {
                        (Some(a), _) => a.clone(),
                        (None, Expression::Column(c)) => c.name.clone(),
                        (None, Expression::Function(f)) => fname(f),
                        (None, _) => "?column?".to_string(),
                    };
                    plan.push((name, ty, Out::Expr(expr.clone())));
                }
            }
        }
        let out_cols: Vec<RelCol> = plan
            .iter()
            .map(|(n, t, _)| RelCol {
                qual: None,
                name: n.clone(),
                ty: t.clone(),
            })
            .collect();
        let order: Vec<OrderByItem> = sel.order_by.clone().unwrap_or_default();
        let select_exprs: Vec<Option<&Expression>> = plan
            .iter()
            .map(|(_, _, o)| if let Out::Expr(e) = o { Some(e) } else { None })
            .collect();
        let keys: Vec<OrderKey> = order
            .iter()
            .map(|o| resolve_order_exprs(&o.expression, &out_cols, &select_exprs))
            .collect();

        let grouped = sel.group_by.is_some()
            || sel.having.is_some()
            || plan
                .iter()
                .any(|(_, _, o)| matches!(o, Out::Expr(e) if contains_agg(e)))
            || order.iter().any(|o| contains_agg(&o.expression));

        let mut keyed: Vec<(Vec<SqlValue>, Row)> = Vec::new();
        if grouped {
            let mut any = Vec::new();
            for (_, _, o) in &plan {
                if let Out::Expr(e) = o {
                    collect_windows(e, &mut any);
                }
            }
            if !any.is_empty() {
                return err(
                    "0A000",
                    "window functions over grouped results are not supported",
                );
            }
            // partition rows
            let group_exprs: Vec<Expression> = sel
                .group_by
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(
                    |g| match resolve_order_exprs(&g, &out_cols, &select_exprs) {
                        OrderKey::Out(i) => {
                            select_exprs.get(i).copied().flatten().cloned().unwrap_or(g)
                        }
                        OrderKey::Expr(e) => e,
                    },
                )
                .collect();
            let mut order_of: Vec<String> = Vec::new();
            let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
            for (i, r) in rows.iter().enumerate() {
                let c = ctx_for(&cols, r, outer);
                let mut k = Vec::new();
                for g in &group_exprs {
                    k.push(self.eval(g, &c)?);
                }
                let key = group_key(&k);
                if !groups.contains_key(&key) {
                    order_of.push(key.clone());
                }
                groups.entry(key).or_default().push(i);
            }
            if group_exprs.is_empty() && groups.is_empty() {
                order_of.push(String::new());
                groups.insert(String::new(), vec![]);
            }
            for key in order_of {
                let idxs = &groups[&key];
                let ctxs: Vec<EvaluationContext> = idxs
                    .iter()
                    .map(|&i| ctx_for(&cols, &rows[i], outer))
                    .collect();
                let rep = match ctxs.first() {
                    Some(c) => c.clone(),
                    None => ctx_for(&cols, &vec![SqlValue::Null; cols.len()], outer),
                };
                if let Some(h) = &sel.having {
                    let v = self.eval_grouped(h, &rep, &ctxs)?;
                    if !truthy(&v) {
                        continue;
                    }
                }
                let mut out = Vec::with_capacity(plan.len());
                for (_, _, o) in &plan {
                    out.push(match o {
                        Out::Src(i) => rep
                            .current_row
                            .get(&cols[*i].name)
                            .cloned()
                            .unwrap_or(SqlValue::Null),
                        Out::Expr(e) => self.eval_grouped(e, &rep, &ctxs)?,
                    });
                }
                let mut k = Vec::new();
                for key in &keys {
                    k.push(match key {
                        OrderKey::Out(i) => out[*i].clone(),
                        OrderKey::Expr(e) => self.eval_grouped(e, &rep, &ctxs)?,
                    });
                }
                keyed.push((k, out));
            }
        } else {
            // window functions are computed once over all the rows, then read per row
            let mut wins: Vec<Expression> = Vec::new();
            for (_, _, o) in &plan {
                if let Out::Expr(e) = o {
                    collect_windows(e, &mut wins);
                }
            }
            for key in &keys {
                if let OrderKey::Expr(e) = key {
                    collect_windows(e, &mut wins);
                }
            }
            let mut win_vals: Vec<Vec<SqlValue>> = Vec::new();
            for w in &wins {
                win_vals.push(self.compute_window(w, &cols, &rows, outer)?);
            }
            for (ri, r) in rows.iter().enumerate() {
                let c = ctx_for(&cols, r, outer);
                let mut out = Vec::with_capacity(plan.len());
                for (_, _, o) in &plan {
                    out.push(match o {
                        Out::Src(i) => r[*i].clone(),
                        Out::Expr(e) if wins.is_empty() => self.eval(e, &c)?,
                        Out::Expr(e) => {
                            let e2 = substitute_windows(e, &wins, &win_vals, ri)?;
                            self.eval(&e2, &c)?
                        }
                    });
                }
                let mut k = Vec::new();
                for key in &keys {
                    k.push(match key {
                        OrderKey::Out(i) => out[*i].clone(),
                        OrderKey::Expr(e) if wins.is_empty() => self.eval(e, &c)?,
                        OrderKey::Expr(e) => {
                            let e2 = substitute_windows(e, &wins, &win_vals, ri)?;
                            self.eval(&e2, &c)?
                        }
                    });
                }
                keyed.push((k, out));
            }
        }

        // DISTINCT before ordering (output rows), DISTINCT ON after ordering.
        if matches!(sel.distinct, Some(DistinctClause::Distinct)) {
            let mut seen = HashSet::new();
            keyed.retain(|(_, r)| seen.insert(group_key(r)));
        }
        if !order.is_empty() {
            sort_keyed(&mut keyed, &order.iter().collect::<Vec<_>>());
        }
        if let Some(DistinctClause::DistinctOn(exprs)) = &sel.distinct {
            let mut seen = HashSet::new();
            let mut kept = Vec::new();
            for (k, r) in keyed {
                let c = ctx_for(&out_cols, &r, outer);
                let mut dk = Vec::new();
                for e in exprs {
                    dk.push(self.eval(e, &c)?);
                }
                if seen.insert(group_key(&dk)) {
                    kept.push((k, r));
                }
            }
            keyed = kept;
        }
        let mut out_rows: Vec<Row> = keyed.into_iter().map(|(_, r)| r).collect();
        self.apply_limit(&mut out_rows, sel)?;
        Ok(Rel {
            cols: out_cols,
            rows: out_rows,
        })
    }

    /// Return a direct table relation narrowed by a simple indexed predicate.
    /// The full WHERE expression is still evaluated below, so this is only a
    /// candidate-row optimization and cannot change query semantics.
    fn indexed_from(&mut self, f: &FromClause, filter: Option<&Expression>) -> R<Option<Rel>> {
        let FromClause::Table { name, alias, .. } = f else {
            return Ok(None);
        };
        let key = name.name.to_lowercase();
        let qual = alias
            .as_ref()
            .map(|a| a.name.to_lowercase())
            .unwrap_or_else(|| key.clone());
        if self.ctes.contains_key(&key)
            || name.schema.as_deref().map(str::to_lowercase).as_deref()
                == Some("information_schema")
        {
            return Ok(None);
        }
        let Some(t) = self.st.tables.get(&key) else {
            return Ok(None);
        };
        let Some(filter) = filter else {
            return Ok(None);
        };
        let Some(ids) = self.indexed_ids(t, filter, &qual)? else {
            return Ok(None);
        };
        let cols = t
            .cols
            .iter()
            .map(|c| RelCol {
                qual: Some(qual.clone()),
                name: c.name.clone(),
                ty: Some(c.ty.clone()),
            })
            .collect::<Vec<_>>();
        let rows = if self.describe {
            Vec::new()
        } else {
            ids.into_iter()
                .filter_map(|id| t.rows.get(&id).cloned())
                .collect()
        };
        Ok(Some(Rel { cols, rows }))
    }

    /// Find candidates for the first usable equality/IN predicate in an AND.
    pub(crate) fn indexed_ids(
        &mut self,
        t: &Table,
        e: &Expression,
        qual: &str,
    ) -> R<Option<Vec<u64>>> {
        if let Expression::Binary {
            left,
            operator: BinaryOperator::And,
            right,
        } = e
        {
            if let Some(ids) = self.indexed_ids(t, left, qual)? {
                return Ok(Some(ids));
            }
            return self.indexed_ids(t, right, qual);
        }
        let (column, values) = match e {
            Expression::Binary {
                left,
                operator: BinaryOperator::Equal,
                right,
            } => {
                if let (Expression::Column(c), Ok(v)) = (&**left, self.literal(right)) {
                    (c, vec![v])
                } else if let (Expression::Column(c), Ok(v)) = (&**right, self.literal(left)) {
                    (c, vec![v])
                } else {
                    return Ok(None);
                }
            }
            Expression::In {
                expr,
                list: InList::Expressions(items),
                negated: false,
            } => {
                let Expression::Column(c) = &**expr else {
                    return Ok(None);
                };
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    let Ok(v) = self.literal(item) else {
                        return Ok(None);
                    };
                    values.push(v);
                }
                (c, values)
            }
            _ => return Ok(None),
        };
        if column
            .table
            .as_deref()
            .is_some_and(|q| q.to_lowercase() != qual)
        {
            return Ok(None);
        }
        let Some(ci) = t.col_index(&column.name) else {
            return Ok(None);
        };
        let Some(index) = t.indexes.iter().find(|i| i.cols == [ci]) else {
            return Ok(None);
        };
        let mut ids = Vec::new();
        for value in values {
            let key = if index.unique {
                unique_key(&[value])
            } else {
                Some(group_key(&[value]))
            };
            if let Some(key) = key {
                if let Some(rows) = index.map.get(&key) {
                    ids.extend(rows.iter().copied());
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(Some(ids))
    }

    fn literal(&mut self, e: &Expression) -> R<SqlValue> {
        let empty = EvaluationContext::empty();
        self.eval(e, &empty)
    }

    /// Evaluate an expression for a group: aggregates are computed over the group's rows and the rest over its first row.
    fn eval_grouped(
        &mut self,
        e: &Expression,
        rep: &EvaluationContext,
        ctxs: &[EvaluationContext],
    ) -> R<SqlValue> {
        if !contains_agg(e) {
            return self.eval(e, rep);
        }
        let replaced = map_expr(e, self, &mut |me: &mut Self, x: &Expression| {
            if let Expression::Function(c) = x {
                if is_agg_name(&fname(c)) {
                    return Ok(Some(Expression::Literal(me.aggregate(c, ctxs)?)));
                }
            }
            Ok(None)
        })?;
        self.eval(&replaced, rep)
    }

    pub(super) fn aggregate_public(
        &mut self,
        f: &FunctionCall,
        ctxs: &[EvaluationContext],
    ) -> R<SqlValue> {
        self.aggregate(f, ctxs)
    }

    fn aggregate(&mut self, f: &FunctionCall, ctxs: &[EvaluationContext]) -> R<SqlValue> {
        let name = fname(f);
        let mut idxs = Vec::new();
        for (i, c) in ctxs.iter().enumerate() {
            let keep = match &f.filter {
                Some(fl) => truthy(&self.eval(fl, c)?),
                None => true,
            };
            if keep {
                idxs.push(i);
            }
        }
        let star = f.args.is_empty()
            || matches!(f.args.first(), Some(Expression::Column(c)) if c.name == "*");
        if name == "count" && star {
            return Ok(SqlValue::BigInt(idxs.len() as i64));
        }
        let Some(arg) = f.args.first() else {
            return err("42883", format!("{name}() requires an argument"));
        };
        let mut vals: Vec<(SqlValue, Vec<SqlValue>)> = Vec::with_capacity(idxs.len());
        for &i in &idxs {
            let v = self.eval(arg, &ctxs[i])?;
            let mut ks = Vec::new();
            if let Some(ob) = &f.order_by {
                for o in ob {
                    ks.push(self.eval(&o.expression, &ctxs[i])?);
                }
            }
            vals.push((v, ks));
        }
        if let Some(ob) = &f.order_by {
            sort_keyed_pairs(&mut vals, &ob.iter().collect::<Vec<_>>());
        }
        let mut values: Vec<SqlValue> = vals.into_iter().map(|(v, _)| v).collect();
        if name == "array_agg" {
            return Ok(SqlValue::Array(values));
        }
        values.retain(|v| !v.is_null());
        if f.distinct {
            let mut seen = HashSet::new();
            values.retain(|v| seen.insert(canonical(v)));
        }
        Ok(match name.as_str() {
            "count" => SqlValue::BigInt(values.len() as i64),
            "min" => values.into_iter().min_by(cmp).unwrap_or(SqlValue::Null),
            "max" => values.into_iter().max_by(cmp).unwrap_or(SqlValue::Null),
            "bool_and" | "every" => {
                if values.is_empty() {
                    SqlValue::Null
                } else {
                    SqlValue::Boolean(values.iter().all(truthy))
                }
            }
            "bool_or" => {
                if values.is_empty() {
                    SqlValue::Null
                } else {
                    SqlValue::Boolean(values.iter().any(truthy))
                }
            }
            "string_agg" => {
                if values.is_empty() {
                    SqlValue::Null
                } else {
                    let sep = match f.args.get(1) {
                        Some(e) => self
                            .eval(e, &EvaluationContext::empty())?
                            .to_postgres_string(),
                        None => ",".into(),
                    };
                    SqlValue::Text(
                        values
                            .iter()
                            .map(SqlValue::to_postgres_string)
                            .collect::<Vec<_>>()
                            .join(&sep),
                    )
                }
            }
            "sum" | "avg" => {
                if values.is_empty() {
                    return Ok(SqlValue::Null);
                }
                let floats = values
                    .iter()
                    .any(|v| matches!(v, SqlValue::Real(_) | SqlValue::DoublePrecision(_)));
                if floats {
                    let s: f64 = values.iter().filter_map(super::value::as_f64).sum();
                    return Ok(SqlValue::DoublePrecision(if name == "avg" {
                        s / values.len() as f64
                    } else {
                        s
                    }));
                }
                let mut total = Decimal::ZERO;
                for v in &values {
                    total += match v {
                        SqlValue::Decimal(d) => *d,
                        other => Decimal::from(super::value::as_i64(other).ok_or_else(|| {
                            super::DbError {
                                code: "42883",
                                message: format!("{name}() needs numbers"),
                            }
                        })?),
                    };
                }
                if name == "avg" {
                    SqlValue::Decimal(
                        (total / Decimal::from(values.len() as i64))
                            .round_dp(16)
                            .normalize(),
                    )
                } else if values
                    .iter()
                    .all(|v| matches!(v, SqlValue::SmallInt(_) | SqlValue::Integer(_)))
                {
                    SqlValue::BigInt(
                        rust_decimal::prelude::ToPrimitive::to_i64(&total).unwrap_or(0),
                    )
                } else {
                    SqlValue::Decimal(total)
                }
            }
            other => return err("42883", format!("function {other}() is not supported")),
        })
    }

    /// A comma-separated list of FROM items as one relation: their cross product.
    pub fn join_all(&mut self, items: &[FromClause]) -> R<Rel> {
        let mut it = items.iter();
        let Some(first) = it.next() else {
            return Ok(Rel {
                cols: vec![],
                rows: vec![vec![]],
            });
        };
        let mut acc = self.from(first, None)?;
        for f in it {
            let next = self.from(f, None)?;
            acc = self.join(
                acc,
                next,
                &JoinType::Cross,
                &JoinCondition::Using(vec![]),
                None,
            )?;
        }
        Ok(acc)
    }

    /// The parts of `information_schema` that tools use to find out what tables and columns exist.
    fn information_schema(&self, view: &str, qual: &str) -> R<Rel> {
        let col = |n: &str, t: SqlType| RelCol {
            qual: Some(qual.to_string()),
            name: n.to_string(),
            ty: Some(t),
        };
        let text = |s: &str| SqlValue::Text(s.to_string());
        let mut names: Vec<&String> = self.st.tables.keys().collect();
        names.sort();
        match view {
            "columns" => {
                let cols = vec![
                    col("table_catalog", SqlType::Text),
                    col("table_schema", SqlType::Text),
                    col("table_name", SqlType::Text),
                    col("column_name", SqlType::Text),
                    col("ordinal_position", SqlType::Integer),
                    col("column_default", SqlType::Text),
                    col("is_nullable", SqlType::Text),
                    col("data_type", SqlType::Text),
                ];
                let mut rows = Vec::new();
                for n in names {
                    let t = &self.st.tables[n];
                    for (i, c) in t.cols.iter().enumerate() {
                        let default = match &c.serial {
                            Some(seq) => text(&format!("nextval('{seq}'::regclass)")),
                            None => SqlValue::Null,
                        };
                        rows.push(vec![
                            text("orbit"),
                            text("public"),
                            text(&t.name),
                            text(&c.name),
                            SqlValue::Integer(i as i32 + 1),
                            default,
                            text(if c.not_null { "NO" } else { "YES" }),
                            text(&data_type_name(&c.ty)),
                        ]);
                    }
                }
                Ok(Rel { cols, rows })
            }
            "tables" => {
                let cols = vec![
                    col("table_catalog", SqlType::Text),
                    col("table_schema", SqlType::Text),
                    col("table_name", SqlType::Text),
                    col("table_type", SqlType::Text),
                ];
                let rows = names
                    .iter()
                    .map(|n| vec![text("orbit"), text("public"), text(n), text("BASE TABLE")])
                    .collect();
                Ok(Rel { cols, rows })
            }
            other => err(
                "42P01",
                format!("relation \"information_schema.{other}\" does not exist"),
            ),
        }
    }

    // ---- FROM ----

    fn from(&mut self, f: &FromClause, outer: Outer) -> R<Rel> {
        match f {
            FromClause::Table { name, alias, .. } => {
                let key = name.name.to_lowercase();
                let qual = alias
                    .as_ref()
                    .map(|a| a.name.to_lowercase())
                    .unwrap_or_else(|| key.clone());
                if let Some(c) = self.ctes.get(&key) {
                    let mut rel = c.clone();
                    for col in &mut rel.cols {
                        col.qual = Some(qual.clone());
                    }
                    return Ok(rel);
                }
                if name.schema.as_deref().map(str::to_lowercase).as_deref()
                    == Some("information_schema")
                {
                    return self.information_schema(&key, &qual);
                }
                let Some(t) = self.st.tables.get(&key) else {
                    return err("42P01", format!("relation \"{key}\" does not exist"));
                };
                let mut cols: Vec<RelCol> = t
                    .cols
                    .iter()
                    .map(|c| RelCol {
                        qual: Some(qual.clone()),
                        name: c.name.clone(),
                        ty: Some(c.ty.clone()),
                    })
                    .collect();
                if let Some(a) = alias.as_ref().and_then(|a| a.columns.as_ref()) {
                    for (c, n) in cols.iter_mut().zip(a) {
                        c.name = n.to_lowercase();
                    }
                }
                Ok(Rel {
                    cols,
                    rows: if self.describe {
                        Vec::new()
                    } else {
                        t.rows.values().cloned().collect()
                    },
                })
            }
            FromClause::Subquery { query, alias, .. } => {
                let mut rel = self.select(query, outer)?;
                let q = alias.name.to_lowercase();
                rename_cols(&mut rel, alias.columns.as_deref(), &q);
                Ok(rel)
            }
            FromClause::Values { values, alias } => {
                let empty = EvaluationContext::empty();
                let mut rows = Vec::new();
                for r in values {
                    let mut row = Vec::new();
                    for e in r {
                        row.push(self.eval(e, &empty)?);
                    }
                    rows.push(row);
                }
                let width = rows.first().map_or(0, Vec::len);
                let q = alias.as_ref().map(|a| a.name.to_lowercase());
                let names: Vec<String> = (0..width)
                    .map(|i| {
                        alias
                            .as_ref()
                            .and_then(|a| a.columns.as_ref())
                            .and_then(|c| c.get(i))
                            .map(|s| s.to_lowercase())
                            .unwrap_or_else(|| format!("column{}", i + 1))
                    })
                    .collect();
                Ok(Rel {
                    cols: names
                        .into_iter()
                        .map(|name| RelCol {
                            qual: q.clone(),
                            name,
                            ty: None,
                        })
                        .collect(),
                    rows,
                })
            }
            FromClause::Join {
                left,
                join_type,
                right,
                condition,
            } => {
                let l = self.from(left, outer)?;
                let r = self.from(right, outer)?;
                self.join(l, r, join_type, condition, outer)
            }
            _ => err("0A000", "this FROM item is not supported"),
        }
    }

    fn join(
        &mut self,
        l: Rel,
        r: Rel,
        jt: &JoinType,
        cond: &JoinCondition,
        outer: Outer,
    ) -> R<Rel> {
        let cols: Vec<RelCol> = l.cols.iter().chain(r.cols.iter()).cloned().collect();
        let on: Option<Expression> = match cond {
            JoinCondition::On(e) => Some(e.clone()),
            JoinCondition::Using(names) => names
                .iter()
                .map(|n| {
                    let lq = l
                        .cols
                        .iter()
                        .find(|c| c.name == n.to_lowercase())
                        .and_then(|c| c.qual.clone());
                    let rq = r
                        .cols
                        .iter()
                        .find(|c| c.name == n.to_lowercase())
                        .and_then(|c| c.qual.clone());
                    Expression::Binary {
                        left: Box::new(Expression::Column(
                            crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                table: lq,
                                name: n.to_lowercase(),
                            },
                        )),
                        operator: BinaryOperator::Equal,
                        right: Box::new(Expression::Column(
                            crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                table: rq,
                                name: n.to_lowercase(),
                            },
                        )),
                    }
                })
                .reduce(|a, b| Expression::Binary {
                    left: Box::new(a),
                    operator: BinaryOperator::And,
                    right: Box::new(b),
                }),
            JoinCondition::Natural => return err("0A000", "NATURAL JOIN is not supported"),
        };
        let cross = matches!(jt, JoinType::Cross) || on.is_none();

        // hash the right side on a single equality between a left and a right column
        let equi = on.as_ref().and_then(|e| equi_keys(e, &l.cols, &r.cols));
        let mut rindex: HashMap<String, Vec<usize>> = HashMap::new();
        if let Some((_, ri)) = equi {
            for (i, row) in r.rows.iter().enumerate() {
                if !row[ri].is_null() {
                    rindex.entry(canonical(&row[ri])).or_default().push(i);
                }
            }
        }
        let mut out = Vec::new();
        let mut right_hit = vec![false; r.rows.len()];
        let null_r: Row = vec![SqlValue::Null; r.cols.len()];
        let null_l: Row = vec![SqlValue::Null; l.cols.len()];
        for lrow in &l.rows {
            let candidates: Vec<usize> = match (equi, cross) {
                (_, true) => (0..r.rows.len()).collect(),
                (Some((li, _)), _) => {
                    if lrow[li].is_null() {
                        vec![]
                    } else {
                        rindex
                            .get(&canonical(&lrow[li]))
                            .cloned()
                            .unwrap_or_default()
                    }
                }
                (None, _) => (0..r.rows.len()).collect(),
            };
            let mut hit = false;
            for ri in candidates {
                let mut row = lrow.clone();
                row.extend(r.rows[ri].iter().cloned());
                let ok = match (&on, cross) {
                    (Some(e), false) => {
                        let c = ctx_for(&cols, &row, outer);
                        truthy(&self.eval(e, &c)?)
                    }
                    _ => true,
                };
                if ok {
                    hit = true;
                    right_hit[ri] = true;
                    out.push(row);
                }
            }
            if !hit && matches!(jt, JoinType::LeftOuter | JoinType::FullOuter) {
                let mut row = lrow.clone();
                row.extend(null_r.iter().cloned());
                out.push(row);
            }
        }
        if matches!(jt, JoinType::RightOuter | JoinType::FullOuter) {
            for (ri, hit) in right_hit.iter().enumerate() {
                if !hit {
                    let mut row = null_l.clone();
                    row.extend(r.rows[ri].iter().cloned());
                    out.push(row);
                }
            }
        }
        Ok(Rel { cols, rows: out })
    }
}

fn find_col(cols: &[RelCol], qual: Option<&str>, name: &str) -> Option<usize> {
    let n = name.to_lowercase();
    let q = qual.map(str::to_lowercase);
    cols.iter()
        .position(|c| c.name == n && (q.is_none() || c.qual == q))
}

/// `(left index, right index)` when `on` has a top-level equality between a left and a right column.
fn equi_keys(on: &Expression, l: &[RelCol], r: &[RelCol]) -> Option<(usize, usize)> {
    let mut conj = Vec::new();
    fn split<'e>(e: &'e Expression, out: &mut Vec<&'e Expression>) {
        if let Expression::Binary {
            left,
            operator: BinaryOperator::And,
            right,
        } = e
        {
            split(left, out);
            split(right, out);
        } else {
            out.push(e);
        }
    }
    split(on, &mut conj);
    for c in conj {
        if let Expression::Binary {
            left,
            operator: BinaryOperator::Equal,
            right,
        } = c
        {
            if let (Expression::Column(a), Expression::Column(b)) = (&**left, &**right) {
                let la = find_col(l, a.table.as_deref(), &a.name);
                let rb = find_col(r, b.table.as_deref(), &b.name);
                if let (Some(la), Some(rb)) = (la, rb) {
                    return Some((la, rb));
                }
                let lb = find_col(l, b.table.as_deref(), &b.name);
                let ra = find_col(r, a.table.as_deref(), &a.name);
                if let (Some(lb), Some(ra)) = (lb, ra) {
                    return Some((lb, ra));
                }
            }
        }
    }
    None
}

fn rename_cols(rel: &mut Rel, names: Option<&[String]>, qual: &str) {
    for (i, c) in rel.cols.iter_mut().enumerate() {
        c.qual = Some(qual.to_string());
        if let Some(n) = names.and_then(|n| n.get(i)) {
            c.name = n.to_lowercase();
        }
    }
}

fn dedupe(rows: &mut Vec<Row>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(group_key(r)));
}

/// An ORDER BY term is an output column when it is an ordinal or an output name; otherwise an expression.
fn resolve_order_exprs(
    e: &Expression,
    out: &[RelCol],
    _select: &[Option<&Expression>],
) -> OrderKey {
    match e {
        Expression::Literal(v) => match super::value::as_i64(v) {
            Some(n) if n >= 1 && (n as usize) <= out.len() => OrderKey::Out(n as usize - 1),
            _ => OrderKey::Expr(e.clone()),
        },
        Expression::Column(c) if c.table.is_none() => {
            match out.iter().position(|o| o.name == c.name.to_lowercase()) {
                Some(i) => OrderKey::Out(i),
                None => OrderKey::Expr(e.clone()),
            }
        }
        _ => OrderKey::Expr(e.clone()),
    }
}

fn resolve_order(e: &Expression, out: &[RelCol], select: &[Option<&Expression>]) -> OrderKey {
    resolve_order_exprs(e, out, select)
}

pub(super) fn order_cmp(a: &[SqlValue], b: &[SqlValue], items: &[&OrderByItem]) -> Ordering {
    for (i, item) in items.iter().enumerate() {
        let (x, y) = (&a[i], &b[i]);
        let desc = matches!(item.direction, Some(SortDirection::Descending));
        // NULLs sort as the largest value unless NULLS FIRST / NULLS LAST says otherwise.
        let nulls_first = match item.nulls {
            Some(NullsOrder::First) => true,
            Some(NullsOrder::Last) => false,
            None => desc,
        };
        match (x.is_null(), y.is_null()) {
            (true, true) => continue,
            (true, false) => {
                return if nulls_first {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (false, true) => {
                return if nulls_first {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            _ => {
                let c = cmp(x, y);
                let c = if desc { c.reverse() } else { c };
                if c != Ordering::Equal {
                    return c;
                }
            }
        }
    }
    Ordering::Equal
}

fn sort_keyed(rows: &mut [(Vec<SqlValue>, Row)], items: &[&OrderByItem]) {
    rows.sort_by(|a, b| order_cmp(&a.0, &b.0, items));
}

fn sort_keyed_pairs(rows: &mut [(SqlValue, Vec<SqlValue>)], items: &[&OrderByItem]) {
    rows.sort_by(|a, b| order_cmp(&a.1, &b.1, items));
}

/// The name `information_schema.columns.data_type` gives a column type.
fn data_type_name(t: &SqlType) -> String {
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
        SqlType::Time { .. } => "time without time zone",
        SqlType::Timestamp {
            with_timezone: true,
        } => "timestamp with time zone",
        SqlType::Timestamp { .. } => "timestamp without time zone",
        SqlType::Json => "json",
        SqlType::Jsonb => "jsonb",
        SqlType::Uuid => "uuid",
        _ => "USER-DEFINED",
    }
    .to_string()
}
