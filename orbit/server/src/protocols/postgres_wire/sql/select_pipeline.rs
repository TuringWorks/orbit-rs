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
    DistinctClause, Expression, FunctionName, SelectItem, SelectStatement, SortDirection,
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
    let mut evaluator = ExpressionEvaluator::new();

    // WHERE
    let filtered = match &select.where_clause {
        None => rows,
        Some(predicate) => {
            let mut kept = Vec::new();
            for row in rows {
                if is_true(&evaluate(&mut evaluator, predicate, &row)?) {
                    kept.push(row);
                }
            }
            kept
        }
    };

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
    let mut deduplicated = match select.distinct {
        None => after_having,
        Some(DistinctClause::Distinct) | Some(DistinctClause::DistinctOn(_)) => {
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
    };

    // ORDER BY, on the source row so a sort key need not be selected.
    if let Some(order_by) = &select.order_by {
        // Keys are computed once per row rather than on each comparison, which
        // would otherwise re-evaluate the expression O(n log n) times.
        let mut keyed = Vec::with_capacity(deduplicated.len());
        for (row, group, output) in deduplicated {
            let mut keys = Vec::with_capacity(order_by.len());
            for item in order_by {
                keys.push(evaluate(&mut evaluator, &item.expression, &row)?);
            }
            keyed.push((keys, row, group, output));
        }

        keyed.sort_by(|a, b| {
            for (index, item) in order_by.iter().enumerate() {
                let ordering = compare(&a.0[index], &b.0[index]);
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

    Ok(SelectOutput {
        columns: output_column_names(select),
        rows: windowed
            .into_iter()
            .map(|(_, _, output)| {
                output
                    .into_iter()
                    .map(|value| match value {
                        SqlValue::Null => None,
                        other => Some(other.to_postgres_string()),
                    })
                    .collect()
            })
            .collect(),
    })
}

/// Whether the select list or HAVING clause calls an aggregate.
fn has_aggregate(select: &SelectStatement) -> bool {
    select
        .select_list
        .iter()
        .any(|item| match item {
            SelectItem::Expression { expr, .. } => expression_has_aggregate(expr),
            _ => false,
        })
        || select.having.as_ref().is_some_and(expression_has_aggregate)
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
            } else {
                SqlValue::DoublePrecision(inputs.iter().filter_map(as_f64).sum())
            }
        }
        "AVG" => {
            let numbers: Vec<f64> = inputs.iter().filter_map(as_f64).collect();
            if numbers.is_empty() {
                SqlValue::Null
            } else {
                SqlValue::DoublePrecision(numbers.iter().sum::<f64>() / numbers.len() as f64)
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
        "STRING_AGG" | "ARRAY_AGG" => SqlValue::Text(
            inputs
                .iter()
                .map(SqlValue::to_postgres_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
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
        Expression::Function(_) => Expression::Literal(evaluate_over_group(
            evaluator,
            expr,
            group,
            representative,
        )?),
        Expression::Binary {
            left,
            operator,
            right,
        } => Expression::Binary {
            left: Box::new(substitute_aggregates(evaluator, left, group, representative)?),
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
        other => as_i64(other).map(|n| n as f64),
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
        let output = run("SELECT id FROM t LIMIT 2", rows(&[(1, "a"), (2, "a"), (3, "b")]));
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
