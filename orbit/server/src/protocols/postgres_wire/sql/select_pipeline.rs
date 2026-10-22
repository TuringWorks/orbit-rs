//! Row-level execution of a `SELECT` over rows already fetched from storage.
//!
//! The executor previously returned every row of the table and projected the
//! select list by column name, ignoring `WHERE`, `GROUP BY`, `HAVING`,
//! `DISTINCT`, `ORDER BY`, `LIMIT`/`OFFSET` and aggregate calls. Those clauses
//! were parsed and then dropped, so `SELECT ... LIMIT 2` returned every row and
//! `COUNT(*)` produced one empty column per row — wrong answers reported as
//! success.
//!
//! This applies them, in SQL's evaluation order:
//! `WHERE` → `GROUP BY`/aggregates → `HAVING` → `DISTINCT` → `ORDER BY` →
//! `OFFSET`/`LIMIT` → projection.

use std::collections::HashMap;

use super::ast::{
    DistinctClause, Expression, FunctionName, NullsOrder, SelectItem, SelectStatement,
    SortDirection, WindowFunctionType,
};
use super::expression_evaluator::{EvaluationContext, ExpressionEvaluator};
use super::types::SqlValue;
use crate::protocols::error::{ProtocolError, ProtocolResult};

/// A row as it moves through the pipeline.
pub type Row = HashMap<String, SqlValue>;

/// The result of running a select over `rows`.
pub struct SelectOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// Apply a `SELECT`'s clauses to rows already read from a table.
///
/// # Errors
/// Returns an error when an expression cannot be evaluated — an unknown
/// function, or a comparison between values that do not compare.
pub fn run_select(select: &SelectStatement, rows: Vec<Row>) -> ProtocolResult<SelectOutput> {
    let (columns, values) = run_select_values(select, rows)?;
    Ok(SelectOutput {
        columns,
        rows: values
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| match value {
                        SqlValue::Null => None,
                        other => Some(other.to_postgres_string()),
                    })
                    .collect()
            })
            .collect(),
    })
}

/// Apply a `SELECT`'s clauses, keeping the results as typed values.
///
/// A derived table or a set operation consumes the output of a select as rows
/// to compute over, not as display text; rendering to strings first would make
/// `WHERE t.id > 1` a string comparison.
///
/// # Errors
/// Returns an error when an expression cannot be evaluated.
pub fn run_select_values(
    select: &SelectStatement,
    rows: Vec<Row>,
) -> ProtocolResult<(Vec<String>, Vec<Vec<SqlValue>>)> {
    let mut evaluator = ExpressionEvaluator::new();

    // A column that is not there is an error, not NULL. The evaluator answers
    // NULL for an unknown name, so `SELECT no_such_column FROM t` returned a
    // column of NULLs and `WHERE no_such_column = 1` returned no rows — both
    // reported as success.
    if let Some(sample) = rows.first() {
        for item in &select.select_list {
            if let SelectItem::Expression { expr, .. } = item {
                check_columns_exist(expr, sample)?;
            }
        }
        if let Some(predicate) = &select.where_clause {
            check_columns_exist(predicate, sample)?;
        }
    }

    // A `LIMIT` can stop the filter early, but only when nothing downstream
    // needs the rows it would skip: an ORDER BY re-orders them, an aggregate
    // or GROUP BY folds them, DISTINCT drops duplicates, and a window function
    // spans the partition. Applying the limit only at the end meant
    // `... WHERE note LIKE '%x%' LIMIT 1` filtered every row of the table
    // before discarding all but one.
    let stop_after = select
        .limit
        .as_ref()
        .filter(|_| {
            select.order_by.is_none()
                && select.group_by.is_none()
                && select.distinct.is_none()
                && !has_aggregate(select)
                && !select_has_window(select)
        })
        .and_then(|limit| limit.count.as_ref())
        .and_then(|count| as_i64(&evaluate(&mut evaluator, count, &Row::new()).ok()?))
        .and_then(|count| usize::try_from(count).ok())
        .map(|count| count.saturating_add(select.offset.unwrap_or(0) as usize));

    // WHERE
    let filtered = match &select.where_clause {
        None => match stop_after {
            Some(enough) => rows.into_iter().take(enough).collect(),
            None => rows,
        },
        Some(predicate) => {
            let mut kept = Vec::new();
            for (index, row) in rows.into_iter().enumerate() {
                if stop_after.is_some_and(|enough| kept.len() >= enough) {
                    break;
                }
                // Filtering a large table is the other place a cancelled
                // query spends its time.
                if index.is_multiple_of(512) {
                    crate::protocols::postgres_wire::query_engine::check_cancelled()?;
                }
                if is_true(&evaluate(&mut evaluator, predicate, &row)?) {
                    kept.push(row);
                }
            }
            kept
        }
    };

    // Window functions are computed over the filtered rows, before grouping
    // and projection: each call's value is attached to its row and the call is
    // replaced by a reference to it, so the rest of the pipeline sees an
    // ordinary column.
    let (windowed_select, filtered) = apply_window_functions(&mut evaluator, select, filtered)?;
    let select = windowed_select.as_ref().unwrap_or(select);

    // GROUP BY / aggregates. A select list containing an aggregate with no
    // GROUP BY is one group over every row, which is what `COUNT(*)` means.
    let grouped = if select.group_by.is_some() || has_aggregate(select) {
        aggregate_rows(&mut evaluator, select, filtered)?
    } else {
        project_rows(&mut evaluator, select, filtered)?
    };

    // HAVING, evaluated over each group: it usually contains an aggregate, so
    // it cannot be evaluated against a single representative row.
    let after_having = match &select.having {
        None => grouped,
        Some(predicate) => {
            let mut kept = Vec::new();
            for (row, group, output) in grouped {
                let verdict = evaluate_over_group(&mut evaluator, predicate, &group, &row)?;
                if is_true(&verdict) {
                    kept.push((row, group, output));
                }
            }
            kept
        }
    };

    // DISTINCT, over the projected values so it means what the user sees.
    let mut deduplicated = match &select.distinct {
        None => after_having,
        Some(DistinctClause::Distinct) => {
            let mut seen = Vec::new();
            let mut kept = Vec::new();
            for (row, group, output) in after_having {
                if !seen.contains(&output) {
                    seen.push(output.clone());
                    kept.push((row, group, output));
                }
            }
            kept
        }
        // `DISTINCT ON (keys)` keeps the first row per key, not per output
        // row; treating it as plain DISTINCT kept every row whose projection
        // differed, which is a different answer.
        Some(DistinctClause::DistinctOn(keys)) => {
            let mut seen: Vec<Vec<SqlValue>> = Vec::new();
            let mut kept = Vec::new();
            for (row, group, output) in after_having {
                let mut key = Vec::with_capacity(keys.len());
                for expression in keys {
                    key.push(evaluate(&mut evaluator, expression, &row)?);
                }
                if !seen.contains(&key) {
                    seen.push(key);
                    kept.push((row, group, output));
                }
            }
            kept
        }
    };

    // ORDER BY, on the source row so a sort key need not be selected.
    if let Some(order_by) = &select.order_by {
        // `ORDER BY 1` and `ORDER BY <alias>` name an output column, not a
        // value to evaluate. Evaluating them gave the same constant for every
        // row (an ordinal) or an unknown-column error (an alias), so the
        // statement silently returned rows in storage order.
        let output_names = output_column_names(select);
        let sort_positions: Vec<Option<usize>> = order_by
            .iter()
            .map(|item| output_position(&item.expression, &output_names))
            .collect();

        // Keys are computed once per row rather than on each comparison, which
        // would otherwise re-evaluate the expression O(n log n) times.
        let mut keyed = Vec::with_capacity(deduplicated.len());
        for (row, group, output) in deduplicated {
            let mut keys = Vec::with_capacity(order_by.len());
            for (index, item) in order_by.iter().enumerate() {
                keys.push(match sort_positions[index].and_then(|at| output.get(at)) {
                    Some(value) => value.clone(),
                    None => evaluate(&mut evaluator, &item.expression, &row)?,
                });
            }
            keyed.push((keys, row, group, output));
        }

        keyed.sort_by(|a, b| {
            for (index, item) in order_by.iter().enumerate() {
                let (left, right) = (&a.0[index], &b.0[index]);
                let descending = matches!(item.direction, Some(SortDirection::Descending));

                // `NULLS FIRST`/`LAST` overrides where the comparison would
                // put NULL. PostgreSQL's default is last when ascending and
                // first when descending; parsing the clause and then ignoring
                // it left `ORDER BY x NULLS FIRST` sorted the other way.
                let nulls_first = match item.nulls {
                    Some(NullsOrder::First) => true,
                    Some(NullsOrder::Last) => false,
                    None => descending,
                };
                let ordering = match (
                    matches!(left, SqlValue::Null),
                    matches!(right, SqlValue::Null),
                ) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) if nulls_first => std::cmp::Ordering::Less,
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) if nulls_first => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    (false, false) => {
                        let ordering = compare(left, right);
                        if descending {
                            ordering.reverse()
                        } else {
                            ordering
                        }
                    }
                };
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
            std::cmp::Ordering::Equal
        });

        deduplicated = keyed
            .into_iter()
            .map(|(_, row, group, output)| (row, group, output))
            .collect();
    }

    // OFFSET then LIMIT.
    let offset = select.offset.unwrap_or(0) as usize;
    let mut windowed: Vec<_> = deduplicated.into_iter().skip(offset).collect();
    if let Some(limit) = &select.limit {
        if let Some(count) = &limit.count {
            let count = evaluate(&mut evaluator, count, &Row::new())?;
            if let Some(count) = as_i64(&count) {
                windowed.truncate(count.max(0) as usize);
            }
        }
    }

    Ok((
        output_column_names(select),
        windowed.into_iter().map(|(_, _, output)| output).collect(),
    ))
}

/// Whether any select-list item calls a window function.
fn select_has_window(select: &SelectStatement) -> bool {
    fn walk(expr: &Expression) -> bool {
        match expr {
            Expression::WindowFunction { .. } => true,
            Expression::Binary { left, right, .. } => walk(left) || walk(right),
            Expression::Unary { operand, .. } => walk(operand),
            Expression::Function(call) => call.args.iter().any(walk),
            _ => false,
        }
    }
    select.select_list.iter().any(|item| match item {
        SelectItem::Expression { expr, .. } => walk(expr),
        _ => false,
    })
}

/// Whether the select list or HAVING clause calls an aggregate.
fn has_aggregate(select: &SelectStatement) -> bool {
    select.select_list.iter().any(|item| match item {
        SelectItem::Expression { expr, .. } => expression_has_aggregate(expr),
        _ => false,
    }) || select.having.as_ref().is_some_and(expression_has_aggregate)
}

fn expression_has_aggregate(expr: &Expression) -> bool {
    match expr {
        Expression::Function(call) => {
            let name = match &call.name {
                FunctionName::Simple(name) => name,
                FunctionName::Qualified { name, .. } => name,
            };
            matches!(
                name.to_uppercase().as_str(),
                "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "ARRAY_AGG" | "STRING_AGG"
            ) || call.args.iter().any(expression_has_aggregate)
        }
        Expression::Binary { left, right, .. } => {
            expression_has_aggregate(left) || expression_has_aggregate(right)
        }
        _ => false,
    }
}

/// Project each row through the select list, keeping the source row alongside
/// so later clauses can still see columns that were not selected.
#[allow(clippy::type_complexity)]
fn project_rows(
    evaluator: &mut ExpressionEvaluator,
    select: &SelectStatement,
    rows: Vec<Row>,
) -> ProtocolResult<Vec<(Row, Vec<Row>, Vec<SqlValue>)>> {
    let mut projected = Vec::with_capacity(rows.len());
    for row in rows {
        let values = project_one(evaluator, select, &row, &row)?;
        projected.push((row.clone(), vec![row], values));
    }
    Ok(projected)
}

/// Group rows and evaluate aggregates over each group.
#[allow(clippy::type_complexity)]
fn aggregate_rows(
    evaluator: &mut ExpressionEvaluator,
    select: &SelectStatement,
    rows: Vec<Row>,
) -> ProtocolResult<Vec<(Row, Vec<Row>, Vec<SqlValue>)>> {
    // Group key preserves first-seen order, so results are stable without an
    // ORDER BY rather than following a hash map's iteration order.
    let mut keys: Vec<Vec<SqlValue>> = Vec::new();
    let mut groups: Vec<Vec<Row>> = Vec::new();

    for row in rows {
        let key = match &select.group_by {
            None => Vec::new(),
            Some(expressions) => {
                let mut key = Vec::with_capacity(expressions.len());
                for expression in expressions {
                    key.push(evaluate(evaluator, expression, &row)?);
                }
                key
            }
        };

        match keys.iter().position(|existing| *existing == key) {
            Some(index) => groups[index].push(row),
            None => {
                keys.push(key);
                groups.push(vec![row]);
            }
        }
    }

    // An aggregate over no rows still produces one row — `COUNT(*)` of an
    // empty table is 0, not "no result".
    if groups.is_empty() && select.group_by.is_none() {
        groups.push(Vec::new());
    }

    let mut output = Vec::with_capacity(groups.len());
    for group in groups {
        let representative = group.first().cloned().unwrap_or_default();
        let values = project_group(evaluator, select, &group, &representative)?;
        output.push((representative, group, values));
    }
    Ok(output)
}

/// Evaluate the select list for one group.
fn project_group(
    evaluator: &mut ExpressionEvaluator,
    select: &SelectStatement,
    group: &[Row],
    representative: &Row,
) -> ProtocolResult<Vec<SqlValue>> {
    let mut values = Vec::new();
    for item in &select.select_list {
        match item {
            SelectItem::Wildcard | SelectItem::QualifiedWildcard { .. } => {
                for (_, value) in sorted_pairs(representative) {
                    values.push(value);
                }
            }
            SelectItem::Expression { expr, .. } => {
                values.push(evaluate_over_group(evaluator, expr, group, representative)?);
            }
        }
    }
    Ok(values)
}

/// Evaluate an expression that may contain aggregates over `group`.
fn evaluate_over_group(
    evaluator: &mut ExpressionEvaluator,
    expr: &Expression,
    group: &[Row],
    representative: &Row,
) -> ProtocolResult<SqlValue> {
    if !expression_has_aggregate(expr) {
        // A plain column in a grouped select takes the group's value, which is
        // the same for every row in it when it is a grouping key.
        return evaluate(evaluator, expr, representative);
    }

    let Expression::Function(call) = expr else {
        // An aggregate inside a larger expression, such as `COUNT(*) > 1` in a
        // HAVING clause. Each aggregate is reduced to a literal first, then the
        // surrounding expression is evaluated normally. This handles any depth
        // of nesting without the evaluator needing to know about groups.
        let substituted = substitute_aggregates(evaluator, expr, group, representative)?;
        return evaluate(evaluator, &substituted, representative);
    };

    let name = match &call.name {
        FunctionName::Simple(name) => name.to_uppercase(),
        FunctionName::Qualified { name, .. } => name.to_uppercase(),
    };

    // `COUNT(*)` counts rows; every other aggregate skips NULL inputs, as in
    // PostgreSQL.
    // `COUNT(*)` arrives either with no argument or with a column literally
    // named `*`, depending on how the parser spelled it.
    let is_star = call.args.is_empty()
        || matches!(
            call.args.first(),
            Some(Expression::Column(column)) if column.name == "*"
        );

    let mut inputs = Vec::new();
    if !is_star {
        for row in group {
            let value = evaluate(evaluator, &call.args[0], row)?;
            if matches!(value, SqlValue::Null) {
                continue;
            }
            // `DISTINCT` inside an aggregate deduplicates its inputs, so
            // `COUNT(DISTINCT grp)` counts groups rather than rows. Ignoring it
            // returns the plain count, which is right only by coincidence.
            if call.distinct && inputs.contains(&value) {
                continue;
            }
            inputs.push(value);
        }
    }

    Ok(match name.as_str() {
        "COUNT" if is_star => SqlValue::BigInt(group.len() as i64),
        "COUNT" => SqlValue::BigInt(inputs.len() as i64),
        "SUM" => {
            if inputs.is_empty() {
                SqlValue::Null
            } else if inputs.iter().all(|v| as_i64(v).is_some()) {
                SqlValue::BigInt(inputs.iter().filter_map(as_i64).sum())
            } else if inputs.iter().any(|v| matches!(v, SqlValue::Decimal(_)))
                && inputs.iter().all(|v| as_decimal(v).is_some())
            {
                SqlValue::Decimal(inputs.iter().filter_map(as_decimal).sum())
            } else {
                SqlValue::DoublePrecision(inputs.iter().filter_map(as_f64).sum())
            }
        }
        "AVG" => {
            // PostgreSQL averages exact inputs exactly: `AVG` over integers is
            // `numeric`, not a float. Through `f64` the mean of 2, 3 and 5 came
            // back as `3.3333333333333335` — a value that is not the average of
            // anything, carrying a rounding artifact in its last digit.
            if !inputs.is_empty() && inputs.iter().all(|v| as_decimal(v).is_some()) {
                let total: rust_decimal::Decimal = inputs.iter().filter_map(as_decimal).sum();
                let count = rust_decimal::Decimal::from(inputs.len());
                total
                    .checked_div(count)
                    .map_or(SqlValue::Null, SqlValue::Decimal)
            } else {
                let numbers: Vec<f64> = inputs.iter().filter_map(as_f64).collect();
                if numbers.is_empty() {
                    SqlValue::Null
                } else {
                    SqlValue::DoublePrecision(numbers.iter().sum::<f64>() / numbers.len() as f64)
                }
            }
        }
        "MIN" => inputs
            .into_iter()
            .reduce(|a, b| if compare(&a, &b).is_le() { a } else { b })
            .unwrap_or(SqlValue::Null),
        "MAX" => inputs
            .into_iter()
            .reduce(|a, b| if compare(&a, &b).is_ge() { a } else { b })
            .unwrap_or(SqlValue::Null),
        // `STRING_AGG(x, sep)` takes its separator as the second argument;
        // joining on a comma regardless produced a plausible-looking wrong
        // answer. `ARRAY_AGG` has no separator and renders as an array.
        "STRING_AGG" => {
            let separator = call
                .args
                .get(1)
                .map(|expr| evaluate(evaluator, expr, representative))
                .transpose()?
                .map_or_else(|| ",".to_string(), |value| value.to_postgres_string());
            SqlValue::Text(
                inputs
                    .iter()
                    .map(SqlValue::to_postgres_string)
                    .collect::<Vec<_>>()
                    .join(&separator),
            )
        }
        "ARRAY_AGG" => SqlValue::Array(inputs),
        other => {
            return Err(ProtocolError::PostgresError(format!(
                "aggregate function '{other}' is not implemented"
            )))
        }
    })
}

/// Replace every aggregate call in `expr` with the value it takes over `group`.
fn substitute_aggregates(
    evaluator: &mut ExpressionEvaluator,
    expr: &Expression,
    group: &[Row],
    representative: &Row,
) -> ProtocolResult<Expression> {
    if !expression_has_aggregate(expr) {
        return Ok(expr.clone());
    }

    Ok(match expr {
        Expression::Function(_) => {
            Expression::Literal(evaluate_over_group(evaluator, expr, group, representative)?)
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => Expression::Binary {
            left: Box::new(substitute_aggregates(
                evaluator,
                left,
                group,
                representative,
            )?),
            operator: operator.clone(),
            right: Box::new(substitute_aggregates(
                evaluator,
                right,
                group,
                representative,
            )?),
        },
        other => other.clone(),
    })
}

/// Evaluate the select list for one ungrouped row.
fn project_one(
    evaluator: &mut ExpressionEvaluator,
    select: &SelectStatement,
    row: &Row,
    source: &Row,
) -> ProtocolResult<Vec<SqlValue>> {
    let mut values = Vec::new();
    for item in &select.select_list {
        match item {
            SelectItem::Wildcard | SelectItem::QualifiedWildcard { .. } => {
                for (_, value) in sorted_pairs(source) {
                    values.push(value);
                }
            }
            SelectItem::Expression { expr, .. } => {
                values.push(evaluate(evaluator, expr, row)?);
            }
        }
    }
    Ok(values)
}

/// Column names in the order the projection produces them.
pub fn output_column_names(select: &SelectStatement) -> Vec<String> {
    let mut names = Vec::new();
    for item in &select.select_list {
        match item {
            SelectItem::Wildcard | SelectItem::QualifiedWildcard { .. } => {
                names.push("*".to_string());
            }
            SelectItem::Expression { expr, alias } => names.push(match (alias, expr) {
                (Some(alias), _) => alias.clone(),
                (None, Expression::Column(column)) => column.name.clone(),
                (None, Expression::Function(call)) => match &call.name {
                    FunctionName::Simple(name) => name.to_lowercase(),
                    FunctionName::Qualified { name, .. } => name.to_lowercase(),
                },
                (None, _) => "expr".to_string(),
            }),
        }
    }
    names
}

/// Row entries in a stable order, so `SELECT *` does not vary run to run.
fn sorted_pairs(row: &Row) -> Vec<(String, SqlValue)> {
    let mut pairs: Vec<(String, SqlValue)> =
        row.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

fn evaluate(
    evaluator: &mut ExpressionEvaluator,
    expr: &Expression,
    row: &Row,
) -> ProtocolResult<SqlValue> {
    let context = EvaluationContext::with_row(row.clone());
    evaluator.evaluate(expr, &context)
}

fn is_true(value: &SqlValue) -> bool {
    matches!(value, SqlValue::Boolean(true))
}

fn as_i64(value: &SqlValue) -> Option<i64> {
    match value {
        SqlValue::SmallInt(n) => Some(i64::from(*n)),
        SqlValue::Integer(n) => Some(i64::from(*n)),
        SqlValue::BigInt(n) => Some(*n),
        _ => None,
    }
}

fn as_f64(value: &SqlValue) -> Option<f64> {
    match value {
        SqlValue::Real(n) => Some(f64::from(*n)),
        SqlValue::DoublePrecision(n) => Some(*n),
        // An exact decimal is a number: without this an aggregate over a
        // `NUMERIC` column summed nothing at all.
        SqlValue::Decimal(n) => n.to_string().parse().ok(),
        other => as_i64(other).map(|n| n as f64),
    }
}

/// Sum values exactly when every one of them is exact.
///
/// `SUM` over a `NUMERIC` column must not go through binary floating point:
/// that is the reason the column was declared `NUMERIC`.
fn as_decimal(value: &SqlValue) -> Option<rust_decimal::Decimal> {
    use std::str::FromStr;
    match value {
        SqlValue::Decimal(n) => Some(*n),
        SqlValue::SmallInt(n) => Some(rust_decimal::Decimal::from(*n)),
        SqlValue::Integer(n) => Some(rust_decimal::Decimal::from(*n)),
        SqlValue::BigInt(n) => Some(rust_decimal::Decimal::from(*n)),
        SqlValue::Real(n) => rust_decimal::Decimal::from_str(&n.to_string()).ok(),
        SqlValue::DoublePrecision(n) => rust_decimal::Decimal::from_str(&n.to_string()).ok(),
        _ => None,
    }
}

/// Order two values, with NULL sorting last as PostgreSQL does by default.
fn compare(a: &SqlValue, b: &SqlValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (a, b) {
        (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
        (SqlValue::Null, _) => Ordering::Greater,
        (_, SqlValue::Null) => Ordering::Less,
        _ => match (as_f64(a), as_f64(b)) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            _ => a.to_postgres_string().cmp(&b.to_postgres_string()),
        },
    }
}

/// Fail if `expr` names a column the row does not have.
///
/// Only bare and qualified column references are checked; a function's own
/// argument names, a literal, or a subquery are not columns of this row.
fn check_columns_exist(expr: &Expression, row: &Row) -> ProtocolResult<()> {
    match expr {
        Expression::Column(column) => {
            if column.name == "*" {
                return Ok(());
            }
            let qualified = column
                .table
                .as_ref()
                .map(|table| format!("{table}.{}", column.name));
            let known = row.contains_key(&column.name)
                || qualified.is_some_and(|key| row.contains_key(&key))
                || row.keys().any(|key| key.eq_ignore_ascii_case(&column.name));
            known.then_some(()).ok_or_else(|| {
                ProtocolError::PostgresError(format!("column \"{}\" does not exist", column.name))
            })
        }
        Expression::Binary { left, right, .. } => {
            check_columns_exist(left, row)?;
            check_columns_exist(right, row)
        }
        Expression::Unary { operand, .. } => check_columns_exist(operand, row),
        Expression::Function(call) => call
            .args
            .iter()
            .try_for_each(|arg| check_columns_exist(arg, row)),
        _ => Ok(()),
    }
}

/// The output column an `ORDER BY` item names, if it names one.
///
/// A positive integer literal is a 1-based position into the select list; a
/// bare identifier that matches an output name — usually an alias — is that
/// column. Anything else is an expression to evaluate against the source row.
fn output_position(expr: &Expression, output_names: &[String]) -> Option<usize> {
    match expr {
        Expression::Literal(value) => {
            let ordinal = as_i64(value)?;
            let index = usize::try_from(ordinal - 1).ok()?;
            (index < output_names.len()).then_some(index)
        }
        Expression::Column(column) if column.table.is_none() => output_names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&column.name)),
        _ => None,
    }
}

/// Name under which a window function's value is stored on each row.
fn window_column(index: usize) -> String {
    format!("__window_{index}")
}

/// Replace each window function in the select list with its per-row value.
///
/// Returns `None` for the statement when there are no window functions, so the
/// common case does not pay for a clone of the select list.
///
/// # Errors
/// Returns an error when a window function's arguments cannot be evaluated.
#[allow(clippy::type_complexity)]
fn apply_window_functions(
    evaluator: &mut ExpressionEvaluator,
    select: &SelectStatement,
    rows: Vec<Row>,
) -> ProtocolResult<(Option<SelectStatement>, Vec<Row>)> {
    let mut calls = Vec::new();
    let mut rewritten = select.clone();
    for item in &mut rewritten.select_list {
        if let SelectItem::Expression { expr, .. } = item {
            extract_windows(expr, &mut calls);
        }
    }

    if calls.is_empty() {
        return Ok((None, rows));
    }

    let mut rows = rows;
    for (index, call) in calls.iter().enumerate() {
        let values = window_values(evaluator, call, &rows)?;
        let name = window_column(index);
        for (row, value) in rows.iter_mut().zip(values) {
            row.insert(name.clone(), value);
        }
    }

    Ok((Some(rewritten), rows))
}

/// Replace every window call in `expr` with a reference to its computed column,
/// collecting the calls in evaluation order.
fn extract_windows(expr: &mut Expression, calls: &mut Vec<Expression>) {
    match expr {
        Expression::WindowFunction { .. } => {
            let name = window_column(calls.len());
            calls.push(expr.clone());
            *expr = Expression::Column(super::ast::ColumnRef { table: None, name });
        }
        Expression::Binary { left, right, .. } => {
            extract_windows(left, calls);
            extract_windows(right, calls);
        }
        Expression::Unary { operand, .. } => extract_windows(operand, calls),
        Expression::Function(call) => {
            for arg in &mut call.args {
                extract_windows(arg, calls);
            }
        }
        _ => {}
    }
}

/// Compute a window function's value for each row, in the rows' own order.
fn window_values(
    evaluator: &mut ExpressionEvaluator,
    call: &Expression,
    rows: &[Row],
) -> ProtocolResult<Vec<SqlValue>> {
    let Expression::WindowFunction {
        function,
        partition_by,
        order_by,
        frame,
    } = call
    else {
        return Ok(vec![SqlValue::Null; rows.len()]);
    };

    // Partition, preserving first-seen order so results are stable.
    let mut keys: Vec<Vec<SqlValue>> = Vec::new();
    let mut partitions: Vec<Vec<usize>> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let mut key = Vec::with_capacity(partition_by.len());
        for expression in partition_by {
            key.push(evaluate(evaluator, expression, row)?);
        }
        match keys.iter().position(|existing| *existing == key) {
            Some(at) => partitions[at].push(index),
            None => {
                keys.push(key);
                partitions.push(vec![index]);
            }
        }
    }

    let mut out = vec![SqlValue::Null; rows.len()];
    for partition in &partitions {
        // Order within the partition. The sort is stable, so rows with equal
        // keys keep their input order — which is what PostgreSQL leaves
        // unspecified but every implementation has to pick something for.
        let mut ordered = partition.clone();
        if !order_by.is_empty() {
            let mut sort_keys: HashMap<usize, Vec<SqlValue>> = HashMap::new();
            for &index in partition {
                let mut key = Vec::with_capacity(order_by.len());
                for item in order_by {
                    key.push(evaluate(evaluator, &item.expression, &rows[index])?);
                }
                sort_keys.insert(index, key);
            }
            ordered.sort_by(|a, b| {
                let (left, right) = (&sort_keys[a], &sort_keys[b]);
                for (position, item) in order_by.iter().enumerate() {
                    let ordering = compare(&left[position], &right[position]);
                    let ordering = match item.direction {
                        Some(SortDirection::Descending) => ordering.reverse(),
                        _ => ordering,
                    };
                    if ordering != std::cmp::Ordering::Equal {
                        return ordering;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        let values = window_partition_values(evaluator, function, order_by, frame, &ordered, rows)?;
        for (&index, value) in ordered.iter().zip(values) {
            out[index] = value;
        }
    }

    Ok(out)
}

/// Values for one ordered partition, in that partition's order.
fn window_partition_values(
    evaluator: &mut ExpressionEvaluator,
    function: &WindowFunctionType,
    order_by: &[super::ast::OrderByItem],
    frame: &Option<super::ast::WindowFrame>,
    ordered: &[usize],
    rows: &[Row],
) -> ProtocolResult<Vec<SqlValue>> {
    let size = ordered.len();
    let offset_or = |expr: &Option<Box<Expression>>, evaluator: &mut ExpressionEvaluator| -> i64 {
        expr.as_ref()
            .and_then(|e| evaluate(evaluator, e, &Row::new()).ok())
            .and_then(|v| as_i64(&v))
            .unwrap_or(1)
    };

    Ok(match function {
        WindowFunctionType::RowNumber => (1..=size as i64).map(SqlValue::BigInt).collect(),

        // RANK leaves gaps after ties; DENSE_RANK does not. With no ORDER BY
        // every row ties, so both are 1 throughout.
        WindowFunctionType::Rank | WindowFunctionType::DenseRank => {
            let dense = matches!(function, WindowFunctionType::DenseRank);
            let mut out = Vec::with_capacity(size);
            let mut rank: i64 = 1;
            for position in 0..size {
                if position > 0 {
                    let tied = order_keys_equal(
                        evaluator,
                        order_by,
                        &rows[ordered[position - 1]],
                        &rows[ordered[position]],
                    )?;
                    if !tied {
                        rank = if dense { rank + 1 } else { position as i64 + 1 };
                    }
                }
                out.push(SqlValue::BigInt(rank));
            }
            out
        }

        WindowFunctionType::Lag { expr, offset, .. }
        | WindowFunctionType::Lead { expr, offset, .. } => {
            let step = offset_or(offset, evaluator);
            let backwards = matches!(function, WindowFunctionType::Lag { .. });
            let mut out = Vec::with_capacity(size);
            for position in 0..size as i64 {
                let target = if backwards {
                    position - step
                } else {
                    position + step
                };
                out.push(
                    match usize::try_from(target).ok().and_then(|t| ordered.get(t)) {
                        Some(&index) => evaluate(evaluator, expr, &rows[index])?,
                        None => SqlValue::Null,
                    },
                );
            }
            out
        }

        WindowFunctionType::FirstValue(expr) | WindowFunctionType::LastValue(expr) => {
            let at = if matches!(function, WindowFunctionType::FirstValue(_)) {
                ordered.first()
            } else {
                ordered.last()
            };
            let value = match at {
                Some(&index) => evaluate(evaluator, expr, &rows[index])?,
                None => SqlValue::Null,
            };
            vec![value; size]
        }

        WindowFunctionType::NthValue { expr, n } => {
            let n = as_i64(&evaluate(evaluator, n, &Row::new())?).unwrap_or(1);
            let value = match usize::try_from(n - 1).ok().and_then(|at| ordered.get(at)) {
                Some(&index) => evaluate(evaluator, expr, &rows[index])?,
                None => SqlValue::Null,
            };
            vec![value; size]
        }

        // An aggregate covers the whole partition unless a frame narrows it;
        // `ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW` is what makes a
        // running total a running total rather than the partition's sum.
        WindowFunctionType::Aggregate(call) => {
            // Which rows tie with the one before them, so a RANGE or GROUPS
            // frame can count peer groups rather than rows.
            let mut peers = Vec::with_capacity(size);
            for position in 0..size {
                peers.push(
                    position > 0
                        && order_keys_equal(
                            evaluator,
                            order_by,
                            &rows[ordered[position - 1]],
                            &rows[ordered[position]],
                        )?,
                );
            }

            let mut out = Vec::with_capacity(size);
            for position in 0..size {
                let (from, to) = frame_bounds(evaluator, frame, position, size, &peers)?;
                let group: Vec<Row> = ordered[from..to]
                    .iter()
                    .map(|&index| rows[index].clone())
                    .collect();
                let representative = group.first().cloned().unwrap_or_default();
                out.push(evaluate_over_group(
                    evaluator,
                    &Expression::Function(call.clone()),
                    &group,
                    &representative,
                )?);
            }
            out
        }

        WindowFunctionType::Ntile(buckets) => {
            let buckets = as_i64(&evaluate(evaluator, buckets, &Row::new())?)
                .unwrap_or(1)
                .max(1);
            (0..size)
                .map(|position| {
                    let bucket = (position as i64 * buckets) / size.max(1) as i64;
                    SqlValue::BigInt(bucket + 1)
                })
                .collect()
        }

        // Both are defined in terms of a row's rank within its partition.
        // `PERCENT_RANK` is (rank - 1) / (rows - 1), and is 0 for a single-row
        // partition rather than a division by zero. `CUME_DIST` is the share
        // of rows at or before this one, so ties share the higher value.
        WindowFunctionType::PercentRank | WindowFunctionType::CumeDist => {
            let cumulative = matches!(function, WindowFunctionType::CumeDist);
            let mut ranks = Vec::with_capacity(size);
            let mut rank: usize = 1;
            for position in 0..size {
                if position > 0
                    && !order_keys_equal(
                        evaluator,
                        order_by,
                        &rows[ordered[position - 1]],
                        &rows[ordered[position]],
                    )?
                {
                    rank = position + 1;
                }
                ranks.push(rank);
            }

            (0..size)
                .map(|position| {
                    if cumulative {
                        // The number of rows in this row's peer group and all
                        // earlier ones.
                        let peers = ranks
                            .iter()
                            .filter(|other| **other <= ranks[position])
                            .count();
                        SqlValue::DoublePrecision(peers as f64 / size as f64)
                    } else if size <= 1 {
                        SqlValue::DoublePrecision(0.0)
                    } else {
                        SqlValue::DoublePrecision((ranks[position] - 1) as f64 / (size - 1) as f64)
                    }
                })
                .collect()
        }
    })
}

/// The half-open row range a frame covers for the row at `position`.
///
/// With no frame the range is the whole partition, which is what an unframed
/// window means. Only `ROWS` offsets are counted; `RANGE` and `GROUPS` need
/// peer-group arithmetic this does not do, and fall back to the partition
/// rather than quietly counting rows as if they were ranges.
fn frame_bounds(
    evaluator: &mut ExpressionEvaluator,
    frame: &Option<super::ast::WindowFrame>,
    position: usize,
    size: usize,
    peers: &[bool],
) -> ProtocolResult<(usize, usize)> {
    use super::ast::{FrameBound, WindowFrameMode};

    let Some(frame) = frame else {
        return Ok((0, size));
    };

    let offset = |bound: &FrameBound, evaluator: &mut ExpressionEvaluator| -> usize {
        match bound {
            FrameBound::Preceding(expr) | FrameBound::Following(expr) => {
                evaluate(evaluator, expr, &Row::new())
                    .ok()
                    .and_then(|value| as_i64(&value))
                    .and_then(|n| usize::try_from(n).ok())
                    .unwrap_or(0)
            }
            _ => 0,
        }
    };

    // `RANGE` and `GROUPS` count peer groups — runs of rows that tie under the
    // window's ORDER BY — rather than rows. `CURRENT ROW` in those modes means
    // the whole peer group, which is why a `RANGE` running total repeats the
    // same value across a tie where a `ROWS` one does not.
    let group_starts = match frame.mode {
        WindowFrameMode::Rows => Vec::new(),
        WindowFrameMode::Range | WindowFrameMode::Groups => peer_group_starts(peers),
    };
    let (position, size) = if group_starts.is_empty() {
        (position, size)
    } else {
        (
            group_starts
                .iter()
                .rposition(|start| *start <= position)
                .unwrap_or(0),
            group_starts.len(),
        )
    };

    let start = match &frame.start_bound {
        FrameBound::UnboundedPreceding => 0,
        FrameBound::Preceding(_) => position.saturating_sub(offset(&frame.start_bound, evaluator)),
        FrameBound::CurrentRow => position,
        FrameBound::Following(_) => (position + offset(&frame.start_bound, evaluator)).min(size),
        FrameBound::UnboundedFollowing => size,
    };

    // The end is exclusive here, so a bound that names a row includes it.
    let end = match frame.end_bound.as_ref() {
        None | Some(FrameBound::CurrentRow) => (position + 1).min(size),
        Some(FrameBound::UnboundedFollowing) => size,
        Some(bound @ FrameBound::Following(_)) => {
            (position + offset(bound, evaluator) + 1).min(size)
        }
        Some(bound @ FrameBound::Preceding(_)) => {
            position.saturating_sub(offset(bound, evaluator)) + 1
        }
        Some(FrameBound::UnboundedPreceding) => 0,
    };

    let (start, end) = (start.min(size), end.max(start).min(size));

    // Translate group indices back to row indices.
    if group_starts.is_empty() {
        return Ok((start, end));
    }
    let row_start = group_starts.get(start).copied().unwrap_or(peers.len());
    let row_end = group_starts.get(end).copied().unwrap_or(peers.len());
    Ok((row_start, row_end.max(row_start)))
}

/// The row index each peer group starts at.
///
/// `peers[i]` is true when row `i` ties with row `i - 1`; a false entry starts
/// a new group.
fn peer_group_starts(peers: &[bool]) -> Vec<usize> {
    (0..peers.len())
        .filter(|index| *index == 0 || !peers[*index])
        .collect()
}

/// Whether two rows tie under the window's ORDER BY.
fn order_keys_equal(
    evaluator: &mut ExpressionEvaluator,
    order_by: &[super::ast::OrderByItem],
    left: &Row,
    right: &Row,
) -> ProtocolResult<bool> {
    for item in order_by {
        let a = evaluate(evaluator, &item.expression, left)?;
        let b = evaluate(evaluator, &item.expression, right)?;
        if compare(&a, &b) != std::cmp::Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::parser::SqlParser;

    fn rows(values: &[(i32, &str)]) -> Vec<Row> {
        values
            .iter()
            .map(|(id, grp)| {
                let mut row = Row::new();
                row.insert("id".to_string(), SqlValue::Integer(*id));
                row.insert("grp".to_string(), SqlValue::Text((*grp).to_string()));
                row
            })
            .collect()
    }

    fn select(sql: &str) -> SelectStatement {
        let statement = SqlParser::new()
            .parse(sql)
            .unwrap_or_else(|e| panic!("parse {sql}: {e}"));
        match statement {
            super::super::ast::Statement::Select(select) => *select,
            other => panic!("expected a SELECT, got {other:?}"),
        }
    }

    fn run(sql: &str, data: Vec<Row>) -> SelectOutput {
        run_select(&select(sql), data).unwrap_or_else(|e| panic!("run {sql}: {e}"))
    }

    fn first_column(output: &SelectOutput) -> Vec<Option<String>> {
        output.rows.iter().map(|row| row[0].clone()).collect()
    }

    #[test]
    fn limit_truncates_the_result() {
        let output = run(
            "SELECT id FROM t LIMIT 2",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(output.rows.len(), 2, "LIMIT must bound the row count");
    }

    #[test]
    fn offset_skips_leading_rows() {
        let output = run(
            "SELECT id FROM t LIMIT 1 OFFSET 1",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(first_column(&output), vec![Some("2".to_string())]);
    }

    #[test]
    fn order_by_sorts_descending() {
        let output = run(
            "SELECT id FROM t ORDER BY id DESC",
            rows(&[(1, "a"), (3, "b"), (2, "a")]),
        );
        assert_eq!(
            first_column(&output),
            vec![
                Some("3".to_string()),
                Some("2".to_string()),
                Some("1".to_string())
            ]
        );
    }

    #[test]
    fn where_filters_rows() {
        let output = run(
            "SELECT id FROM t WHERE id > 1",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(output.rows.len(), 2);
    }

    #[test]
    fn count_star_counts_rows() {
        let output = run("SELECT COUNT(*) FROM t", rows(&[(1, "a"), (2, "a")]));
        assert_eq!(first_column(&output), vec![Some("2".to_string())]);
    }

    /// An aggregate over an empty input still produces one row.
    #[test]
    fn count_of_no_rows_is_zero_not_empty() {
        let output = run("SELECT COUNT(*) FROM t", Vec::new());
        assert_eq!(first_column(&output), vec![Some("0".to_string())]);
    }

    #[test]
    fn group_by_produces_one_row_per_group() {
        let output = run(
            "SELECT grp, COUNT(*) FROM t GROUP BY grp",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(output.rows.len(), 2);
        assert_eq!(output.rows[0][1], Some("2".to_string()));
        assert_eq!(output.rows[1][1], Some("1".to_string()));
    }

    #[test]
    fn distinct_removes_duplicate_output_rows() {
        let output = run(
            "SELECT DISTINCT grp FROM t",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(output.rows.len(), 2);
    }

    #[test]
    fn sum_and_avg_reduce_the_group() {
        let data = rows(&[(1, "a"), (3, "a")]);
        assert_eq!(
            first_column(&run("SELECT SUM(id) FROM t", data.clone())),
            vec![Some("4".to_string())]
        );
        assert_eq!(
            first_column(&run("SELECT MIN(id) FROM t", data.clone())),
            vec![Some("1".to_string())]
        );
        assert_eq!(
            first_column(&run("SELECT MAX(id) FROM t", data)),
            vec![Some("3".to_string())]
        );
    }

    #[test]
    fn nulls_first_puts_nulls_first() {
        let mut with_null = rows(&[(1, "a"), (2, "b")]);
        with_null.push(HashMap::from([
            ("id".to_string(), SqlValue::Integer(3)),
            ("grp".to_string(), SqlValue::Null),
        ]));
        let output = run("SELECT id FROM t ORDER BY grp NULLS FIRST", with_null);
        assert_eq!(first_column(&output)[0], Some("3".to_string()));
    }

    #[test]
    fn order_by_an_ordinal_sorts_by_that_output_column() {
        let output = run(
            "SELECT id FROM t ORDER BY 1 DESC",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(
            first_column(&output),
            vec![
                Some("3".to_string()),
                Some("2".to_string()),
                Some("1".to_string())
            ]
        );
    }

    /// An alias is not a column of the source row, so it has to resolve
    /// against the output.
    #[test]
    fn order_by_an_alias_sorts_by_that_output_column() {
        let output = run(
            "SELECT id AS ident FROM t ORDER BY ident DESC",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(
            first_column(&output),
            vec![
                Some("3".to_string()),
                Some("2".to_string()),
                Some("1".to_string())
            ]
        );
    }

    /// Two rows share a group, so the row count and the distinct count differ.
    #[test]
    fn count_distinct_counts_values_not_rows() {
        let output = run(
            "SELECT COUNT(DISTINCT grp) FROM t",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(first_column(&output), vec![Some("2".to_string())]);
    }

    #[test]
    fn having_filters_groups() {
        let output = run(
            "SELECT grp FROM t GROUP BY grp HAVING COUNT(*) > 1",
            rows(&[(1, "a"), (2, "a"), (3, "b")]),
        );
        assert_eq!(output.rows.len(), 1);
        assert_eq!(output.rows[0][0], Some("a".to_string()));
    }

    /// NULL sorts last ascending, matching PostgreSQL's default.
    #[test]
    fn nulls_sort_last_by_default() {
        let mut with_null = rows(&[(2, "a")]);
        let mut null_row = Row::new();
        null_row.insert("id".to_string(), SqlValue::Null);
        null_row.insert("grp".to_string(), SqlValue::Text("z".to_string()));
        with_null.push(null_row);

        let output = run("SELECT id FROM t ORDER BY id", with_null);
        assert_eq!(first_column(&output), vec![Some("2".to_string()), None]);
    }

    #[test]
    fn a_query_with_no_clauses_returns_every_row() {
        let output = run("SELECT id FROM t", rows(&[(1, "a"), (2, "b")]));
        assert_eq!(output.rows.len(), 2);
    }
}
