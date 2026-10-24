//! Set-returning functions used as a `FROM` item: `generate_series`, `unnest`, the `json[b]_each`
//! and `json[b]_array_elements` families, and `json[b]_object_keys`.

use super::{err, Row, R};
use crate::protocols::postgres_wire::sql::types::{PostgresInterval, SqlType, SqlValue};
use chrono::NaiveDateTime;
use rust_decimal::Decimal;

/// Most rows a single set-returning call may produce; a runaway `generate_series(1, 1e12)` is an
/// error, not an out-of-memory.
const MAX_ROWS: usize = 50_000_000;

/// What a table function returned: column names (and types where known) and the rows.
pub(super) struct Produced {
    pub cols: Vec<(String, Option<SqlType>)>,
    pub rows: Vec<Row>,
}

impl Produced {
    fn one(name: &str, ty: Option<SqlType>, values: impl IntoIterator<Item = SqlValue>) -> Self {
        Self {
            cols: vec![(name.to_string(), ty)],
            rows: values.into_iter().map(|v| vec![v]).collect(),
        }
    }
}

/// Run the named set-returning function, or `None` when `name` is not one.
pub(super) fn call(name: &str, args: &[SqlValue]) -> R<Option<Produced>> {
    Ok(Some(match name {
        "generate_series" => generate_series(args)?,
        "unnest" => unnest(args),
        "json_each" | "jsonb_each" => each(name, args, false),
        "json_each_text" | "jsonb_each_text" => each(name, args, true),
        "json_array_elements" | "jsonb_array_elements" => elements(name, args, false),
        "json_array_elements_text" | "jsonb_array_elements_text" => elements(name, args, true),
        "json_object_keys" | "jsonb_object_keys" => Produced::one(
            name,
            Some(SqlType::Text),
            json_arg(args)
                .and_then(|j| j.as_object().cloned())
                .into_iter()
                .flat_map(|o| o.into_iter().map(|(k, _)| SqlValue::Text(k))),
        ),
        _ => return Ok(None),
    }))
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::SmallInt(n) => Some(i64::from(*n)),
        SqlValue::Integer(n) => Some(i64::from(*n)),
        SqlValue::BigInt(n) => Some(*n),
        _ => None,
    }
}

fn generate_series(args: &[SqlValue]) -> R<Produced> {
    let [start, stop, rest @ ..] = args else {
        return err(
            "42883",
            "function generate_series requires 2 or 3 arguments",
        );
    };
    if args.iter().any(SqlValue::is_null) {
        return Ok(Produced::one("generate_series", None, []));
    }
    let step = rest.first();
    if let (Some(a), Some(b)) = (as_int(start), as_int(stop)) {
        let by = step.map_or(Some(1), as_int);
        let Some(by) = by else {
            return err("42883", "generate_series step must be an integer here");
        };
        if by == 0 {
            return err("22023", "step size cannot equal zero");
        }
        let narrow = args
            .iter()
            .all(|v| matches!(v, SqlValue::Integer(_) | SqlValue::SmallInt(_)));
        let count = if (by > 0 && a > b) || (by < 0 && a < b) {
            0
        } else {
            ((b - a) / by) as usize + 1
        };
        if count > MAX_ROWS {
            return err("54000", "generate_series would produce too many rows");
        }
        let (ty, make): (SqlType, fn(i64) -> SqlValue) = if narrow {
            (SqlType::Integer, |n| SqlValue::Integer(n as i32))
        } else {
            (SqlType::BigInt, SqlValue::BigInt)
        };
        return Ok(Produced::one(
            "generate_series",
            Some(ty),
            (0..count as i64).map(|i| make(a + i * by)),
        ));
    }
    if let (Some(a), Some(b)) = (decimal(start), decimal(stop)) {
        let by = step.and_then(decimal).unwrap_or(Decimal::ONE);
        if by.is_zero() {
            return err("22023", "step size cannot equal zero");
        }
        let mut out = vec![];
        let mut cur = a;
        while (by > Decimal::ZERO && cur <= b) || (by < Decimal::ZERO && cur >= b) {
            if out.len() >= MAX_ROWS {
                return err("54000", "generate_series would produce too many rows");
            }
            out.push(SqlValue::Decimal(cur));
            cur += by;
        }
        return Ok(Produced::one("generate_series", None, out));
    }
    temporal_series(start, stop, step)
}

fn decimal(v: &SqlValue) -> Option<Decimal> {
    match v {
        SqlValue::Decimal(d) => Some(*d),
        other => as_int(other).map(Decimal::from),
    }
}

fn naive(v: &SqlValue) -> Option<NaiveDateTime> {
    match v {
        SqlValue::Timestamp(t) => Some(*t),
        SqlValue::TimestampWithTimezone(t) => Some(t.naive_utc()),
        SqlValue::Date(d) => d.and_hms_opt(0, 0, 0),
        _ => None,
    }
}

/// `generate_series(timestamp, timestamp, interval)` and the date form (which yields timestamps).
fn temporal_series(start: &SqlValue, stop: &SqlValue, step: Option<&SqlValue>) -> R<Produced> {
    let (Some(a), Some(b), Some(SqlValue::Interval(by))) = (naive(start), naive(stop), step) else {
        return err(
            "42883",
            "function generate_series(unknown types) does not exist",
        );
    };
    let forward =
        by.months > 0 || by.days > 0 || (by.months == 0 && by.days == 0 && by.microseconds > 0);
    let backward =
        by.months < 0 || by.days < 0 || (by.months == 0 && by.days == 0 && by.microseconds < 0);
    if !forward && !backward {
        return err("22023", "step size cannot equal zero");
    }
    let mut out = vec![];
    let mut cur = a;
    while (forward && cur <= b) || (backward && cur >= b) {
        if out.len() >= MAX_ROWS {
            return err("54000", "generate_series would produce too many rows");
        }
        out.push(SqlValue::Timestamp(cur));
        let Some(next) = add_interval(cur, by) else {
            break;
        };
        cur = next;
    }
    Ok(Produced::one("generate_series", None, out))
}

/// `ts + interval`, months first, then days, then the time part (PostgreSQL's order).
pub(crate) fn add_interval(ts: NaiveDateTime, by: &PostgresInterval) -> Option<NaiveDateTime> {
    let months = |t: NaiveDateTime| match by.months {
        0 => Some(t),
        m if m > 0 => t.checked_add_months(chrono::Months::new(m.unsigned_abs())),
        m => t.checked_sub_months(chrono::Months::new(m.unsigned_abs())),
    };
    let days = |t: NaiveDateTime| match by.days {
        0 => Some(t),
        d if d > 0 => t.checked_add_days(chrono::Days::new(u64::from(d.unsigned_abs()))),
        d => t.checked_sub_days(chrono::Days::new(u64::from(d.unsigned_abs()))),
    };
    months(ts)
        .and_then(days)
        .and_then(|t| t.checked_add_signed(chrono::Duration::microseconds(by.microseconds)))
}

/// `unnest(a [, b ...])`: one column per array, shorter arrays padded with NULL.
fn unnest(args: &[SqlValue]) -> Produced {
    let arrays: Vec<&[SqlValue]> = args
        .iter()
        .map(|a| match a {
            SqlValue::Array(items) => items.as_slice(),
            _ => &[],
        })
        .collect();
    let height = arrays.iter().map(|a| a.len()).max().unwrap_or(0);
    Produced {
        cols: arrays
            .iter()
            .map(|_| ("unnest".to_string(), None))
            .collect(),
        rows: (0..height)
            .map(|i| {
                arrays
                    .iter()
                    .map(|a| a.get(i).cloned().unwrap_or(SqlValue::Null))
                    .collect()
            })
            .collect(),
    }
}

fn json_arg(args: &[SqlValue]) -> Option<serde_json::Value> {
    match args.first()? {
        SqlValue::Json(j) | SqlValue::Jsonb(j) => Some(j.clone()),
        SqlValue::Text(s) | SqlValue::Varchar(s) => serde_json::from_str(s).ok(),
        _ => None,
    }
}

fn json_value(binary: bool, v: serde_json::Value) -> SqlValue {
    if binary {
        SqlValue::Jsonb(v)
    } else {
        SqlValue::Json(v)
    }
}

/// Text form of a JSON value as `->>` returns it: strings unquoted, `null` as SQL NULL.
fn json_text(v: &serde_json::Value) -> SqlValue {
    match v {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

fn each(name: &str, args: &[SqlValue], text: bool) -> Produced {
    let binary = name.starts_with("jsonb");
    let value_ty = if text {
        SqlType::Text
    } else if binary {
        SqlType::Jsonb
    } else {
        SqlType::Json
    };
    let object = json_arg(args)
        .and_then(|j| j.as_object().cloned())
        .unwrap_or_default();
    Produced {
        cols: vec![
            ("key".to_string(), Some(SqlType::Text)),
            ("value".to_string(), Some(value_ty)),
        ],
        rows: object
            .into_iter()
            .map(|(k, v)| {
                let value = if text {
                    json_text(&v)
                } else {
                    json_value(binary, v)
                };
                vec![SqlValue::Text(k), value]
            })
            .collect(),
    }
}

fn elements(name: &str, args: &[SqlValue], text: bool) -> Produced {
    let binary = name.starts_with("jsonb");
    let ty = if text {
        SqlType::Text
    } else if binary {
        SqlType::Jsonb
    } else {
        SqlType::Json
    };
    let items = json_arg(args)
        .and_then(|j| j.as_array().cloned())
        .unwrap_or_default();
    Produced::one(
        "value",
        Some(ty),
        items.into_iter().map(|v| {
            if text {
                json_text(&v)
            } else {
                json_value(binary, v)
            }
        }),
    )
}
