//! Value helpers for the durable engine: ordering, equality keys, coercion to a column type and
//! the PostgreSQL type OID reported for a column.

use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::str::FromStr;

/// `true` only for a boolean `TRUE`; NULL and everything else is not true.
pub fn truthy(v: &SqlValue) -> bool {
    matches!(v, SqlValue::Boolean(true))
}

fn as_decimal(v: &SqlValue) -> Option<Decimal> {
    match v {
        SqlValue::SmallInt(i) => Some(Decimal::from(*i)),
        SqlValue::Integer(i) => Some(Decimal::from(*i)),
        SqlValue::BigInt(i) => Some(Decimal::from(*i)),
        SqlValue::Decimal(d) => Some(*d),
        SqlValue::Real(f) => Decimal::from_f32(*f),
        SqlValue::DoublePrecision(f) => Decimal::from_f64(*f),
        _ => None,
    }
}

pub fn as_f64(v: &SqlValue) -> Option<f64> {
    match v {
        SqlValue::Real(f) => Some(f64::from(*f)),
        SqlValue::DoublePrecision(f) => Some(*f),
        other => as_decimal(other).and_then(|d| d.to_f64()),
    }
}

pub fn as_i64(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::SmallInt(i) => Some(i64::from(*i)),
        SqlValue::Integer(i) => Some(i64::from(*i)),
        SqlValue::BigInt(i) => Some(*i),
        SqlValue::Decimal(d) => d.to_i64(),
        SqlValue::Real(f) => Some(*f as i64),
        SqlValue::DoublePrecision(f) => Some(*f as i64),
        _ => None,
    }
}

fn is_float(v: &SqlValue) -> bool {
    matches!(v, SqlValue::Real(_) | SqlValue::DoublePrecision(_))
}

fn as_str(v: &SqlValue) -> Option<&str> {
    match v {
        SqlValue::Char(s) | SqlValue::Varchar(s) | SqlValue::Text(s) | SqlValue::Name(s) => Some(s),
        _ => None,
    }
}

/// Total order used by `ORDER BY`, `DISTINCT` and `min`/`max`. NULL sorts after every value, as PostgreSQL does for ascending order.
pub fn cmp(a: &SqlValue, b: &SqlValue) -> Ordering {
    match (a.is_null(), b.is_null()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    if is_float(a) || is_float(b) {
        if let (Some(x), Some(y)) = (as_f64(a), as_f64(b)) {
            return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
        }
    }
    if let (Some(x), Some(y)) = (as_decimal(a), as_decimal(b)) {
        return x.cmp(&y);
    }
    if let (Some(x), Some(y)) = (as_str(a), as_str(b)) {
        return x.cmp(y);
    }
    match (a, b) {
        (SqlValue::Boolean(x), SqlValue::Boolean(y)) => x.cmp(y),
        (SqlValue::Date(x), SqlValue::Date(y)) => x.cmp(y),
        (SqlValue::Time(x), SqlValue::Time(y)) => x.cmp(y),
        (SqlValue::Timestamp(x), SqlValue::Timestamp(y)) => x.cmp(y),
        (SqlValue::TimestampWithTimezone(x), SqlValue::TimestampWithTimezone(y)) => x.cmp(y),
        (SqlValue::Uuid(x), SqlValue::Uuid(y)) => x.cmp(y),
        (SqlValue::Bytea(x), SqlValue::Bytea(y)) => x.cmp(y),
        _ => a.to_postgres_string().cmp(&b.to_postgres_string()),
    }
}

/// The value of a numeric literal as PostgreSQL types it: `int4` if it fits, else `int8`, else `numeric`;
/// a literal with a decimal point is `numeric` (so `10.50 - 0.5` keeps two decimals) and one with an
/// exponent is a float.
pub(crate) fn numeric_literal(n: &str) -> Option<SqlValue> {
    if n.contains('e') || n.contains('E') {
        return n.parse::<f64>().ok().map(SqlValue::DoublePrecision);
    }
    if !n.contains('.') {
        if let Ok(i) = n.parse::<i32>() {
            return Some(SqlValue::Integer(i));
        }
        if let Ok(i) = n.parse::<i64>() {
            return Some(SqlValue::BigInt(i));
        }
    }
    Decimal::from_str(n)
        .ok()
        .map(SqlValue::Decimal)
        .or_else(|| n.parse::<f64>().ok().map(SqlValue::DoublePrecision))
}

/// Weight (in base-10000 digits, as `numeric` stores it) and leading digit group of a non-zero number.
fn base_10000_weight(d: Decimal) -> Option<(i32, u32)> {
    if d.is_zero() {
        return None;
    }
    let text = d.abs().normalize().to_string();
    let (int, frac) = text.split_once('.').unwrap_or((text.as_str(), ""));
    if int != "0" {
        let weight = (int.len() as i32 - 1) / 4;
        let lead = int.len() - (weight as usize) * 4;
        return int[..lead].parse().ok().map(|g| (weight, g));
    }
    let zeros = frac.len() - frac.trim_start_matches('0').len();
    let start = zeros / 4 * 4;
    let group = format!("{:0<4}", &frac[start..frac.len().min(start + 4)]);
    group.parse().ok().map(|g| (-((zeros / 4) as i32) - 1, g))
}

/// The scale of `x / y` in PostgreSQL's `numeric` (`select_div_scale`): at least 16 significant
/// digits, and never fewer decimals than either operand. Capped at what `Decimal` can hold.
pub(crate) fn numeric_div_scale(x: Decimal, y: Decimal) -> u32 {
    let (w1, lead1) = base_10000_weight(x).unwrap_or((0, 0));
    let (w2, lead2) = base_10000_weight(y).unwrap_or((0, 0));
    let qweight = w1 - w2 - i32::from(lead1 <= lead2);
    let scale = (16 - qweight * 4)
        .max(x.scale() as i32)
        .max(y.scale() as i32)
        .max(0);
    scale.min(28) as u32
}

/// Arithmetic on two numbers, with PostgreSQL's promotion rules: integers stay integers (`int4` unless
/// either side is wider), anything with a numeric becomes numeric, anything with a float becomes a float.
/// `None` when either side is not a number. The error is `"[SQLSTATE] message"`.
pub(crate) fn numeric_arith(
    a: &SqlValue,
    b: &SqlValue,
    op: &str,
) -> Option<Result<SqlValue, String>> {
    let kind = |v: &SqlValue| match v {
        SqlValue::SmallInt(_) | SqlValue::Integer(_) | SqlValue::BigInt(_) => Some(1),
        SqlValue::Decimal(_) => Some(2),
        SqlValue::Real(_) | SqlValue::DoublePrecision(_) => Some(3),
        _ => None,
    };
    let (ka, kb) = (kind(a)?, kind(b)?);
    let div0 = || Err("[22012] division by zero".to_string());
    Some(match ka.max(kb) {
        3 => {
            let (x, y) = (as_f64(a)?, as_f64(b)?);
            let v = match op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        return Some(div0());
                    }
                    x / y
                }
                "%" => x % y,
                "^" => x.powf(y),
                _ => return None,
            };
            if matches!((a, b), (SqlValue::Real(_), SqlValue::Real(_))) && op != "^" {
                Ok(SqlValue::Real(v as f32))
            } else {
                Ok(SqlValue::DoublePrecision(v))
            }
        }
        2 => {
            let (x, y) = (as_decimal(a)?, as_decimal(b)?);
            let r = match op {
                "+" => x.checked_add(y),
                "-" => x.checked_sub(y),
                "*" => x.checked_mul(y),
                "/" => {
                    if y.is_zero() {
                        return Some(div0());
                    }
                    x.checked_div(y).map(|d| {
                        let scale = numeric_div_scale(x, y);
                        let mut d = d.round_dp(scale);
                        d.rescale(scale);
                        d
                    })
                }
                "%" => {
                    if y.is_zero() {
                        return Some(div0());
                    }
                    x.checked_rem(y)
                }
                "^" => return Some(Ok(SqlValue::DoublePrecision(x.to_f64()?.powf(y.to_f64()?)))),
                _ => return None,
            };
            r.map(SqlValue::Decimal)
                .ok_or_else(|| "[22003] numeric field overflow".to_string())
        }
        _ => {
            let (x, y) = (as_i64(a)?, as_i64(b)?);
            if op == "^" {
                return Some(Ok(SqlValue::DoublePrecision((x as f64).powf(y as f64))));
            }
            let r = match op {
                "+" => x.checked_add(y),
                "-" => x.checked_sub(y),
                "*" => x.checked_mul(y),
                "/" => {
                    if y == 0 {
                        return Some(div0());
                    }
                    x.checked_div(y)
                }
                "%" => {
                    if y == 0 {
                        return Some(div0());
                    }
                    x.checked_rem(y)
                }
                _ => return None,
            };
            let narrow = matches!(a, SqlValue::SmallInt(_) | SqlValue::Integer(_))
                && matches!(b, SqlValue::SmallInt(_) | SqlValue::Integer(_));
            match r {
                Some(v) if narrow => i32::try_from(v)
                    .map(SqlValue::Integer)
                    .map_err(|_| "[22003] integer out of range".to_string()),
                Some(v) => Ok(SqlValue::BigInt(v)),
                None => Err("[22003] bigint out of range".to_string()),
            }
        }
    })
}

/// Ordering for SQL comparison operators. A string is converted to the other operand's type, as
/// PostgreSQL does for a literal (`created >= '2026-01-01'`, `id = '5'`). `None` means the values
/// are not comparable (or one is NULL).
pub(crate) fn compare_loose(a: &SqlValue, b: &SqlValue) -> Option<Ordering> {
    if a.is_null() || b.is_null() {
        return None;
    }
    let (sa, sb) = (as_str(a).is_some(), as_str(b).is_some());
    if sa && sb {
        return Some(cmp(a, b));
    }
    if sa != sb {
        let (s, other, flip) = if sa { (a, b, false) } else { (b, a, true) };
        let parsed = coerce(s.clone(), &other.sql_type()).ok()?;
        let o = if family(&parsed) == family(other) {
            cmp(&parsed, other)
        } else {
            return None;
        };
        return Some(if flip { o.reverse() } else { o });
    }
    if family(a) == family(b) {
        Some(cmp(a, b))
    } else {
        None
    }
}

fn family(v: &SqlValue) -> u8 {
    match v {
        SqlValue::SmallInt(_)
        | SqlValue::Integer(_)
        | SqlValue::BigInt(_)
        | SqlValue::Decimal(_)
        | SqlValue::Real(_)
        | SqlValue::DoublePrecision(_) => 1,
        SqlValue::Boolean(_) => 2,
        SqlValue::Date(_)
        | SqlValue::Time(_)
        | SqlValue::Timestamp(_)
        | SqlValue::TimestampWithTimezone(_)
        | SqlValue::TimeWithTimezone(_) => 3,
        SqlValue::Uuid(_) => 4,
        SqlValue::Bytea(_) => 5,
        SqlValue::Json(_) | SqlValue::Jsonb(_) => 6,
        _ => 0,
    }
}

/// A string that is equal for values that compare equal, used for unique indexes, grouping and `DISTINCT`.
/// A NULL is returned as `None` by [`unique_key`] because NULLs never collide in a unique index.
pub fn canonical(v: &SqlValue) -> String {
    if v.is_null() {
        return "n".to_string();
    }
    if is_float(v) {
        if let Some(f) = as_f64(v) {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                return format!("d:{}", f as i64);
            }
            return format!("f:{f}");
        }
    }
    if let Some(d) = as_decimal(v) {
        return format!("d:{}", d.normalize());
    }
    if let Some(s) = as_str(v) {
        return format!("s:{s}");
    }
    format!("o:{}", v.to_postgres_string())
}

/// Key for a unique index over `values`, or `None` if any value is NULL.
pub fn unique_key(values: &[SqlValue]) -> Option<String> {
    if values.iter().any(SqlValue::is_null) {
        return None;
    }
    Some(group_key(values))
}

/// Key identifying a group (`GROUP BY`, `DISTINCT`); NULLs group together.
pub fn group_key(values: &[SqlValue]) -> String {
    let mut out = String::new();
    for v in values {
        let c = canonical(v);
        out.push_str(&c.len().to_string());
        out.push(':');
        out.push_str(&c);
    }
    out
}

/// The PostgreSQL type OID a result column of this type is reported with.
pub fn oid_of_type(ty: &SqlType) -> i32 {
    match ty {
        SqlType::Decimal { .. } => 1700,
        other => {
            let oid = other.postgres_oid();
            if oid == 0 {
                25
            } else {
                oid as i32
            }
        }
    }
}

/// The OID reported for a value computed by an expression whose column type is not known statically.
pub fn oid_of_value(v: &SqlValue) -> i32 {
    match v {
        SqlValue::Null => 25,
        SqlValue::Decimal(_) => 1700,
        other => {
            let oid = other.sql_type().postgres_oid();
            if oid == 0 {
                25
            } else {
                oid as i32
            }
        }
    }
}

/// Convert `v` to the storage representation for a column of type `ty`.
pub fn coerce(v: SqlValue, ty: &SqlType) -> Result<SqlValue, String> {
    if v.is_null() {
        return Ok(SqlValue::Null);
    }
    let text = |v: &SqlValue| as_str(v).map(str::to_string);
    let bad = |v: &SqlValue| {
        format!(
            "invalid input syntax for type {}: \"{}\"",
            type_name(ty),
            v.to_postgres_string()
        )
    };
    match ty {
        SqlType::SmallInt | SqlType::Integer | SqlType::BigInt => {
            let n = match &v {
                SqlValue::Boolean(b) => i64::from(*b),
                SqlValue::Decimal(d) => d.round().to_i64().ok_or_else(|| bad(&v))?,
                SqlValue::Real(_) | SqlValue::DoublePrecision(_) => as_f64(&v)
                    .map(|f| f.round() as i64)
                    .ok_or_else(|| bad(&v))?,
                other => match as_i64(other) {
                    Some(n) => n,
                    None => {
                        let s = text(other).ok_or_else(|| bad(&v))?;
                        let t = s.trim();
                        t.parse::<i64>()
                            .or_else(|_| {
                                Decimal::from_str(t)
                                    .ok()
                                    .and_then(|d| d.round().to_i64())
                                    .ok_or(())
                            })
                            .map_err(|()| bad(&v))?
                    }
                },
            };
            Ok(match ty {
                SqlType::SmallInt => SqlValue::SmallInt(
                    i16::try_from(n).map_err(|_| "smallint out of range".to_string())?,
                ),
                SqlType::Integer => SqlValue::Integer(
                    i32::try_from(n).map_err(|_| "integer out of range".to_string())?,
                ),
                _ => SqlValue::BigInt(n),
            })
        }
        SqlType::Decimal { precision, scale } | SqlType::Numeric { precision, scale } => {
            let mut d = match as_decimal(&v) {
                Some(d) => d,
                None => Decimal::from_str(text(&v).ok_or_else(|| bad(&v))?.trim())
                    .map_err(|_| bad(&v))?,
            };
            if let Some(s) = scale {
                // NUMERIC(p, s) rounds half away from zero and always shows s decimals
                d = d.round_dp_with_strategy(
                    u32::from(*s),
                    rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                );
                d.rescale(u32::from(*s));
                if let Some(p) = precision {
                    let whole = d.trunc().abs().to_string();
                    let digits = if whole == "0" { 0 } else { whole.len() as u32 };
                    if digits + u32::from(*s) > u32::from(*p) {
                        return Err(format!(
                            "numeric field overflow: a field with precision {p}, scale {s} must round to an absolute value less than 10^{}",
                            u32::from(*p) - u32::from(*s)
                        ));
                    }
                }
            }
            Ok(SqlValue::Decimal(d))
        }
        SqlType::Real => as_f64(&v)
            .or_else(|| text(&v).and_then(|s| s.trim().parse().ok()))
            .map(|f| SqlValue::Real(f as f32))
            .ok_or_else(|| bad(&v)),
        SqlType::DoublePrecision => as_f64(&v)
            .or_else(|| text(&v).and_then(|s| s.trim().parse().ok()))
            .map(SqlValue::DoublePrecision)
            .ok_or_else(|| bad(&v)),
        SqlType::Boolean => match &v {
            SqlValue::Boolean(_) => Ok(v),
            other => match text(other).map(|s| s.trim().to_lowercase()).as_deref() {
                Some("t" | "true" | "y" | "yes" | "on" | "1") => Ok(SqlValue::Boolean(true)),
                Some("f" | "false" | "n" | "no" | "off" | "0") => Ok(SqlValue::Boolean(false)),
                _ => match as_i64(other) {
                    Some(n) => Ok(SqlValue::Boolean(n != 0)),
                    None => Err(bad(&v)),
                },
            },
        },
        SqlType::Char(n) | SqlType::Varchar(n) => {
            let s = match text(&v) {
                Some(s) => s,
                None => v.to_postgres_string(),
            };
            if let Some(limit) = n {
                if s.chars().count() > *limit as usize {
                    return Err(format!(
                        "value too long for type {}({limit})",
                        if matches!(ty, SqlType::Char(_)) {
                            "character"
                        } else {
                            "character varying"
                        }
                    ));
                }
            }
            Ok(if matches!(ty, SqlType::Char(_)) {
                SqlValue::Char(s)
            } else {
                SqlValue::Varchar(s)
            })
        }
        SqlType::Text => Ok(SqlValue::Text(
            text(&v).unwrap_or_else(|| v.to_postgres_string()),
        )),
        SqlType::Json | SqlType::Jsonb => match &v {
            SqlValue::Json(j) | SqlValue::Jsonb(j) => Ok(if matches!(ty, SqlType::Json) {
                SqlValue::Json(j.clone())
            } else {
                SqlValue::Jsonb(j.clone())
            }),
            other => {
                let s = text(other).ok_or_else(|| bad(&v))?;
                let j: serde_json::Value = serde_json::from_str(&s).map_err(|_| bad(&v))?;
                Ok(if matches!(ty, SqlType::Json) {
                    SqlValue::Json(j)
                } else {
                    SqlValue::Jsonb(j)
                })
            }
        },
        SqlType::Date
        | SqlType::Timestamp { .. }
        | SqlType::Time { .. }
        | SqlType::Uuid
        | SqlType::Bytea
        | SqlType::Interval => coerce_temporal_or_binary(&v, ty).ok_or_else(|| bad(&v)),
        SqlType::Array { element_type, .. } => match &v {
            SqlValue::Array(items) => items
                .iter()
                .map(|item| match item {
                    SqlValue::Array(_) => coerce(item.clone(), ty),
                    other => coerce(other.clone(), element_type),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(SqlValue::Array),
            other => match text(other) {
                Some(s) => parse_array_literal(&s, element_type).ok_or_else(|| bad(&v)),
                None => Err(bad(&v)),
            },
        },
        _ => match text(&v) {
            Some(s) => SqlValue::parse_string(&s, ty).or(Ok(v)),
            None => Ok(v),
        },
    }
}

/// PostgreSQL array text (`{1,2,3}`, `{{1,2},{3,4}}`, `{a,"b c",NULL}`) as an array of `element_type`.
fn parse_array_literal(text: &str, element_type: &SqlType) -> Option<SqlValue> {
    fn group(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        element_type: &SqlType,
    ) -> Option<Vec<SqlValue>> {
        let mut out = Vec::new();
        loop {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            match chars.peek()? {
                '}' => {
                    chars.next();
                    return Some(out);
                }
                ',' => {
                    chars.next();
                }
                '{' => {
                    chars.next();
                    out.push(SqlValue::Array(group(chars, element_type)?));
                }
                '"' => {
                    chars.next();
                    let mut item = String::new();
                    loop {
                        match chars.next()? {
                            '\\' => item.push(chars.next()?),
                            '"' => break,
                            c => item.push(c),
                        }
                    }
                    out.push(coerce(SqlValue::Text(item), element_type).ok()?);
                }
                _ => {
                    let mut item = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == ',' || c == '}' {
                            break;
                        }
                        item.push(c);
                        chars.next();
                    }
                    let item = item.trim();
                    out.push(if item.eq_ignore_ascii_case("null") {
                        SqlValue::Null
                    } else {
                        coerce(SqlValue::Text(item.to_string()), element_type).ok()?
                    });
                }
            }
        }
    }
    let mut chars = text.trim().chars().peekable();
    if chars.next()? != '{' {
        return None;
    }
    let items = group(&mut chars, element_type)?;
    chars.next().is_none().then_some(SqlValue::Array(items))
}

fn parse_naive_timestamp(s: &str) -> Option<chrono::NaiveDateTime> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};
    let s = s.trim();
    for f in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
    ] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, f) {
            return Some(d);
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0);
    }
    // with an offset: keep the UTC wall time
    DateTime::parse_from_rfc3339(&s.replacen(' ', "T", 1))
        .ok()
        .map(|d| d.naive_utc())
        .or_else(|| {
            ["%Y-%m-%d %H:%M:%S%.f%#z", "%Y-%m-%d %H:%M:%S%.f%z"]
                .iter()
                .find_map(|f| DateTime::parse_from_str(s, f).ok().map(|d| d.naive_utc()))
        })
}

/// Dates, times, UUIDs, byte strings and intervals from text or from a neighbouring temporal type.
fn coerce_temporal_or_binary(v: &SqlValue, ty: &SqlType) -> Option<SqlValue> {
    use chrono::{DateTime, NaiveTime, Utc};
    match ty {
        SqlType::Timestamp { with_timezone } => {
            let naive = match v {
                SqlValue::Timestamp(t) => *t,
                SqlValue::TimestampWithTimezone(t) => t.naive_utc(),
                SqlValue::Date(d) => d.and_hms_opt(0, 0, 0)?,
                other => parse_naive_timestamp(as_str(other)?)?,
            };
            Some(if *with_timezone {
                SqlValue::TimestampWithTimezone(DateTime::<Utc>::from_naive_utc_and_offset(
                    naive, Utc,
                ))
            } else {
                SqlValue::Timestamp(naive)
            })
        }
        SqlType::Date => match v {
            SqlValue::Date(d) => Some(SqlValue::Date(*d)),
            SqlValue::Timestamp(t) => Some(SqlValue::Date(t.date())),
            SqlValue::TimestampWithTimezone(t) => Some(SqlValue::Date(t.date_naive())),
            other => parse_naive_timestamp(as_str(other)?).map(|t| SqlValue::Date(t.date())),
        },
        SqlType::Time { .. } => match v {
            SqlValue::Time(t) => Some(SqlValue::Time(*t)),
            other => {
                let s = as_str(other)?.trim();
                ["%H:%M:%S%.f", "%H:%M"]
                    .iter()
                    .find_map(|f| NaiveTime::parse_from_str(s, f).ok())
                    .map(SqlValue::Time)
            }
        },
        SqlType::Uuid => match v {
            SqlValue::Uuid(u) => Some(SqlValue::Uuid(*u)),
            other => uuid::Uuid::parse_str(as_str(other)?.trim())
                .ok()
                .map(SqlValue::Uuid),
        },
        SqlType::Bytea => match v {
            SqlValue::Bytea(b) => Some(SqlValue::Bytea(b.clone())),
            other => {
                let s = as_str(other)?;
                match s.strip_prefix("\\x") {
                    Some(hex_digits) => hex::decode(hex_digits).ok().map(SqlValue::Bytea),
                    None => Some(SqlValue::Bytea(s.as_bytes().to_vec())),
                }
            }
        },
        SqlType::Interval => match v {
            SqlValue::Interval(_) => Some(v.clone()),
            other => SqlValue::parse_interval(as_str(other)?).ok(),
        },
        _ => None,
    }
}

fn type_name(ty: &SqlType) -> &'static str {
    match ty {
        SqlType::SmallInt => "smallint",
        SqlType::Integer => "integer",
        SqlType::BigInt => "bigint",
        SqlType::Decimal { .. } | SqlType::Numeric { .. } => "numeric",
        SqlType::Real => "real",
        SqlType::DoublePrecision => "double precision",
        SqlType::Boolean => "boolean",
        SqlType::Date => "date",
        SqlType::Timestamp { .. } => "timestamp",
        SqlType::Uuid => "uuid",
        SqlType::Bytea => "bytea",
        SqlType::Json => "json",
        SqlType::Jsonb => "jsonb",
        _ => "value",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_zero_stays_text_and_numbers_compare_across_types() {
        assert_eq!(
            coerce(SqlValue::Text("0".into()), &SqlType::Varchar(None)).unwrap(),
            SqlValue::Varchar("0".into())
        );
        assert_eq!(
            coerce(SqlValue::Text("42".into()), &SqlType::Integer).unwrap(),
            SqlValue::Integer(42)
        );
        assert_eq!(
            cmp(&SqlValue::Integer(2), &SqlValue::BigInt(10)),
            Ordering::Less
        );
        assert_eq!(
            cmp(&SqlValue::Null, &SqlValue::Integer(1)),
            Ordering::Greater
        );
    }

    #[test]
    fn equal_numbers_share_a_key() {
        assert_eq!(
            unique_key(&[SqlValue::Integer(5)]),
            unique_key(&[SqlValue::BigInt(5)])
        );
        assert_ne!(
            unique_key(&[SqlValue::Text("5".into())]),
            unique_key(&[SqlValue::Integer(5)])
        );
        assert_eq!(unique_key(&[SqlValue::Null, SqlValue::Integer(1)]), None);
    }

    #[test]
    fn timestamps_dates_and_uuids_parse_from_text() {
        let ts = coerce(
            SqlValue::Text("2026-10-23 20:57:17".into()),
            &SqlType::Timestamp {
                with_timezone: false,
            },
        )
        .unwrap();
        assert_eq!(ts.to_postgres_string(), "2026-10-23 20:57:17");
        let d = coerce(SqlValue::Text("2026-10-23".into()), &SqlType::Date).unwrap();
        assert_eq!(d.to_postgres_string(), "2026-10-23");
        assert!(coerce(SqlValue::Text("not a date".into()), &SqlType::Date).is_err());
        assert!(coerce(
            SqlValue::Text("6f1c2d34-0000-4000-8000-000000000001".into()),
            &SqlType::Uuid
        )
        .is_ok());
        let tz = coerce(
            SqlValue::Text("2026-10-23T20:57:17+02:00".into()),
            &SqlType::Timestamp {
                with_timezone: false,
            },
        )
        .unwrap();
        assert_eq!(tz.to_postgres_string(), "2026-10-23 18:57:17");
    }

    #[test]
    fn varchar_limit_and_bad_integers_are_errors() {
        assert!(coerce(SqlValue::Text("abcd".into()), &SqlType::Varchar(Some(3))).is_err());
        assert!(coerce(SqlValue::Text("x".into()), &SqlType::Integer).is_err());
    }

    /// Every expected string was produced by PostgreSQL 18 (`SELECT a / b` on numerics).
    #[test]
    fn numeric_division_scale_matches_postgresql() {
        let d = |t: &str| SqlValue::Decimal(Decimal::from_str(t).unwrap());
        for (a, b, want) in [
            ("7.0", "2", "3.5000000000000000"),
            ("1.0", "3", "0.33333333333333333333"),
            ("10000", "3.0", "3333.3333333333333333"),
            ("100000000.0", "3", "33333333.333333333333"),
            ("0.0001", "3", "0.000033333333333333333333"),
            ("1", "0.0007", "1428.5714285714285714"),
            ("7", "10.50", "0.66666666666666666667"),
        ] {
            let got = numeric_arith(&d(a), &d(b), "/").unwrap().unwrap();
            assert_eq!(got.to_postgres_string(), want, "{a} / {b}");
        }
    }
}
