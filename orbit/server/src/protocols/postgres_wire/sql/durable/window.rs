//! Window functions: `f(...) OVER (PARTITION BY ... ORDER BY ... [frame])`.
//!
//! Each distinct window expression in a query is computed once over the rows that survived `WHERE`,
//! giving one value per row; the projection then reads those values.

use super::query::{ctx_for, order_cmp, Outer, RelCol, Runner};
use super::value::{as_i64, group_key};
use super::{err, Row, R};
use crate::protocols::postgres_wire::sql::ast::{
    Expression, FrameBound, OrderByItem, WindowFrameMode, WindowFunctionType,
};
use crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use rust_decimal::Decimal;
use std::collections::HashMap;

impl Runner<'_> {
    /// The value of window expression `node` for every row of `rows`, in the same order.
    pub(crate) fn compute_window(
        &mut self,
        node: &Expression,
        cols: &[RelCol],
        rows: &[Row],
        outer: Outer,
    ) -> R<Vec<SqlValue>> {
        let Expression::WindowFunction {
            function,
            partition_by,
            order_by,
            frame,
        } = node
        else {
            return err("XX000", "not a window function");
        };
        let ctxs: Vec<EvaluationContext> = rows.iter().map(|r| ctx_for(cols, r, outer)).collect();
        let mut order_of: Vec<String> = Vec::new();
        let mut parts: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, c) in ctxs.iter().enumerate() {
            let mut k = Vec::new();
            for e in partition_by {
                k.push(self.eval(e, c)?);
            }
            let key = group_key(&k);
            if !parts.contains_key(&key) {
                order_of.push(key.clone());
            }
            parts.entry(key).or_default().push(i);
        }
        let items: Vec<&OrderByItem> = order_by.iter().collect();
        let mut out = vec![SqlValue::Null; rows.len()];
        for key in order_of {
            let mut keyed: Vec<(usize, Vec<SqlValue>)> = Vec::new();
            for &i in &parts[&key] {
                let mut k = Vec::new();
                for o in order_by {
                    k.push(self.eval(&o.expression, &ctxs[i])?);
                }
                keyed.push((i, k));
            }
            keyed.sort_by(|a, b| order_cmp(&a.1, &b.1, &items));
            let m = keyed.len();
            // peers: rows that compare equal on the ORDER BY keys
            let (mut first, mut last, mut dense) =
                (vec![0usize; m], vec![0usize; m], vec![0usize; m]);
            let mut group_start = 0;
            let mut group_no = 0;
            for pos in 0..m {
                if pos > 0 && group_key(&keyed[pos].1) != group_key(&keyed[pos - 1].1) {
                    for p in group_start..pos {
                        last[p] = pos - 1;
                    }
                    group_start = pos;
                    group_no += 1;
                }
                first[pos] = group_start;
                dense[pos] = group_no;
            }
            for p in group_start..m {
                last[p] = m - 1;
            }
            let whole_partition_aggregate = match function {
                WindowFunctionType::Aggregate(call) if frame.is_none() && order_by.is_empty() => {
                    let fctxs: Vec<EvaluationContext> =
                        keyed.iter().map(|(i, _)| ctxs[*i].clone()).collect();
                    Some(self.aggregate_public(call, &fctxs)?)
                }
                _ => None,
            };
            for pos in 0..m {
                let row_i = keyed[pos].0;
                let (lo, hi) = self.frame_bounds(
                    frame.as_ref(),
                    order_by.is_empty(),
                    pos,
                    m,
                    &first,
                    &last,
                    &ctxs[row_i],
                )?;
                let at = |p: usize| keyed[p].0;
                out[row_i] = match function {
                    WindowFunctionType::RowNumber => SqlValue::BigInt(pos as i64 + 1),
                    WindowFunctionType::Rank => SqlValue::BigInt(first[pos] as i64 + 1),
                    WindowFunctionType::DenseRank => SqlValue::BigInt(dense[pos] as i64 + 1),
                    WindowFunctionType::PercentRank => SqlValue::DoublePrecision(if m <= 1 {
                        0.0
                    } else {
                        first[pos] as f64 / (m - 1) as f64
                    }),
                    WindowFunctionType::CumeDist => {
                        SqlValue::DoublePrecision((last[pos] + 1) as f64 / m as f64)
                    }
                    WindowFunctionType::Ntile(n) => {
                        let buckets =
                            as_i64(&self.eval(n, &ctxs[row_i])?).unwrap_or(1).max(1) as usize;
                        let (base, extra) = (m / buckets, m % buckets);
                        // the first `extra` buckets hold one more row
                        let big = extra * (base + 1);
                        let bucket = if pos < big {
                            pos / (base + 1)
                        } else {
                            extra + (pos - big) / base.max(1)
                        };
                        SqlValue::Integer(bucket as i32 + 1)
                    }
                    WindowFunctionType::Lag {
                        expr,
                        offset,
                        default,
                    }
                    | WindowFunctionType::Lead {
                        expr,
                        offset,
                        default,
                    } => {
                        let n = match offset {
                            Some(o) => as_i64(&self.eval(o, &ctxs[row_i])?).unwrap_or(1),
                            None => 1,
                        };
                        let lag = matches!(function, WindowFunctionType::Lag { .. });
                        let target = if lag { pos as i64 - n } else { pos as i64 + n };
                        if target >= 0 && (target as usize) < m {
                            self.eval(expr, &ctxs[at(target as usize)])?
                        } else if let Some(d) = default {
                            self.eval(d, &ctxs[row_i])?
                        } else {
                            SqlValue::Null
                        }
                    }
                    WindowFunctionType::FirstValue(e) => {
                        if lo <= hi {
                            self.eval(e, &ctxs[at(lo)])?
                        } else {
                            SqlValue::Null
                        }
                    }
                    WindowFunctionType::LastValue(e) => {
                        if lo <= hi {
                            self.eval(e, &ctxs[at(hi)])?
                        } else {
                            SqlValue::Null
                        }
                    }
                    WindowFunctionType::NthValue { expr, n } => {
                        let k = as_i64(&self.eval(n, &ctxs[row_i])?).unwrap_or(1).max(1) as usize;
                        if lo + k - 1 <= hi && lo <= hi {
                            self.eval(expr, &ctxs[at(lo + k - 1)])?
                        } else {
                            SqlValue::Null
                        }
                    }
                    WindowFunctionType::Aggregate(call) => match &whole_partition_aggregate {
                        Some(v) => v.clone(),
                        None if lo > hi => self.aggregate_public(call, &[])?,
                        None => {
                            let fctxs: Vec<EvaluationContext> =
                                (lo..=hi).map(|p| ctxs[at(p)].clone()).collect();
                            self.aggregate_public(call, &fctxs)?
                        }
                    },
                };
            }
            let _ = Decimal::ZERO;
        }
        Ok(out)
    }

    /// Positions `lo..=hi` of the frame for the row at `pos` (the frame is empty when `lo > hi`).
    #[allow(clippy::too_many_arguments)]
    fn frame_bounds(
        &mut self,
        frame: Option<&crate::protocols::postgres_wire::sql::ast::WindowFrame>,
        unordered: bool,
        pos: usize,
        m: usize,
        first: &[usize],
        last: &[usize],
        ctx: &EvaluationContext,
    ) -> R<(usize, usize)> {
        let Some(f) = frame else {
            // default: the whole partition without ORDER BY, else from the start through the last peer
            return Ok((0, if unordered { m - 1 } else { last[pos] }));
        };
        let rows_mode = matches!(f.mode, WindowFrameMode::Rows);
        let mut offset =
            |e: &Expression| -> R<i64> { Ok(as_i64(&self.eval(e, ctx)?).unwrap_or(0)) };
        let mut bound = |b: &FrameBound, is_start: bool| -> R<i64> {
            Ok(match b {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::UnboundedFollowing => m as i64 - 1,
                FrameBound::CurrentRow => {
                    if rows_mode {
                        pos as i64
                    } else if is_start {
                        first[pos] as i64
                    } else {
                        last[pos] as i64
                    }
                }
                FrameBound::Preceding(e) => pos as i64 - offset(e)?,
                FrameBound::Following(e) => pos as i64 + offset(e)?,
            })
        };
        let lo = bound(&f.start_bound, true)?.max(0);
        let hi = match &f.end_bound {
            Some(b) => bound(b, false)?,
            None => bound(&FrameBound::CurrentRow, false)?,
        }
        .min(m as i64 - 1);
        if hi < 0 || lo > hi {
            return Ok((1, 0));
        }
        Ok((lo as usize, hi as usize))
    }
}
