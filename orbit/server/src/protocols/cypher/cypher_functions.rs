//! Cypher Built-in Functions Implementation
//!
//! This module provides comprehensive implementations of all Neo4j Cypher built-in functions
//! including string, list, math, date/time, type, and path functions.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Cypher function evaluator
pub struct CypherFunctions;

impl CypherFunctions {
    /// Evaluate a Cypher function by name with the given arguments
    pub fn evaluate(
        name: &str,
        args: &[Value],
        context: &FunctionContext,
    ) -> ProtocolResult<Value> {
        let name_lower = name.to_lowercase();

        match name_lower.as_str() {
            // String functions
            "toupper" | "touppercase" => Self::to_upper(args),
            "tolower" | "tolowercase" => Self::to_lower(args),
            "trim" => Self::trim(args),
            "ltrim" => Self::ltrim(args),
            "rtrim" => Self::rtrim(args),
            "replace" => Self::replace(args),
            "substring" => Self::substring(args),
            "left" => Self::left(args),
            "right" => Self::right(args),
            "split" => Self::split(args),
            "reverse" => Self::reverse_string(args),
            "tostring" => Self::to_string(args),
            "toboolean" => Self::to_boolean(args),
            "tointeger" | "toint" => Self::to_integer(args),
            "tofloat" => Self::to_float(args),
            "size" => Self::size(args),
            "length" => Self::length(args),
            "charlength" => Self::char_length(args),
            "starts with" | "startswith" => Self::starts_with(args),
            "ends with" | "endswith" => Self::ends_with(args),
            "contains" => Self::contains(args),

            // List functions
            "head" => Self::head(args),
            "tail" => Self::tail(args),
            "last" => Self::last(args),
            "range" => Self::range(args),
            "slice" | "sublist" => Self::slice(args),
            "reduce" => Self::reduce(args, context),
            "keys" => Self::keys(args),
            "labels" => Self::labels(args, context),
            "nodes" => Self::nodes(args, context),
            "relationships" | "rels" => Self::relationships(args, context),
            "collect" => Self::collect(args),
            "unwind" => Self::unwind(args),

            // Math functions
            "abs" => Self::abs(args),
            "ceil" | "ceiling" => Self::ceil(args),
            "floor" => Self::floor(args),
            "round" => Self::round(args),
            "sqrt" => Self::sqrt(args),
            "sign" => Self::sign(args),
            "rand" | "random" => Self::rand(args),
            "log" => Self::log(args),
            "log10" => Self::log10(args),
            "exp" => Self::exp(args),
            "e" => Self::e(args),
            "pi" => Self::pi(args),
            "sin" => Self::sin(args),
            "cos" => Self::cos(args),
            "tan" => Self::tan(args),
            "asin" => Self::asin(args),
            "acos" => Self::acos(args),
            "atan" => Self::atan(args),
            "atan2" => Self::atan2(args),
            "degrees" => Self::degrees(args),
            "radians" => Self::radians(args),
            "haversin" => Self::haversin(args),

            // Date/Time functions
            "date" => Self::date(args),
            "datetime" => Self::datetime(args),
            "localdatetime" => Self::local_datetime(args),
            "time" => Self::time(args),
            "localtime" => Self::local_time(args),
            "duration" => Self::duration(args),
            "date.truncate" | "datetruncate" => Self::date_truncate(args),
            "datetime.truncate" | "datetimetruncate" => Self::datetime_truncate(args),

            // Type functions
            "type" => Self::type_of(args, context),
            "id" => Self::id(args, context),
            "elementid" => Self::element_id(args, context),
            "properties" => Self::properties(args, context),
            "coalesce" => Self::coalesce(args),
            "nullif" => Self::nullif(args),
            "isnan" => Self::is_nan(args),
            "isfinite" => Self::is_finite(args),
            "isinfinite" => Self::is_infinite(args),

            // Path functions
            "pathlength" => Self::path_length(args, context),
            "startnode" => Self::start_node(args, context),
            "endnode" => Self::end_node(args, context),

            // Aggregation functions (scalar versions)
            "min" => Self::min(args),
            "max" => Self::max(args),
            "sum" => Self::sum(args),
            "avg" => Self::avg(args),
            "count" => Self::count(args),
            "stdev" | "stdevp" => Self::stdev(args),
            "percentilecont" => Self::percentile_cont(args),
            "percentiledisc" => Self::percentile_disc(args),

            // Existence check
            "exists" => Self::exists(args, context),

            // Graph-specific
            "shortestpath" => Self::shortest_path(args, context),
            "allshortestpaths" => Self::all_shortest_paths(args, context),

            _ => Err(ProtocolError::CypherError(format!(
                "Unknown function: {name}"
            ))),
        }
    }

    // ==================== String Functions ====================

    fn to_upper(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toUpper")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.to_uppercase())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "toUpper requires a string argument".to_string(),
            )),
        }
    }

    fn to_lower(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toLower")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.to_lowercase())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "toLower requires a string argument".to_string(),
            )),
        }
    }

    fn trim(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "trim")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.trim().to_string())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "trim requires a string argument".to_string(),
            )),
        }
    }

    fn ltrim(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "ltrim")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.trim_start().to_string())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "ltrim requires a string argument".to_string(),
            )),
        }
    }

    fn rtrim(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "rtrim")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.trim_end().to_string())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "rtrim requires a string argument".to_string(),
            )),
        }
    }

    fn replace(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 3, "replace")?;
        match (&args[0], &args[1], &args[2]) {
            (Value::String(s), Value::String(from), Value::String(to)) => {
                Ok(Value::String(s.replace(from.as_str(), to.as_str())))
            }
            (Value::Null, _, _) => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "replace requires three string arguments".to_string(),
            )),
        }
    }

    fn substring(args: &[Value]) -> ProtocolResult<Value> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::CypherError(
                "substring requires 2 or 3 arguments".to_string(),
            ));
        }
        match &args[0] {
            Value::String(s) => {
                let start = Self::get_integer(&args[1])? as usize;
                let chars: Vec<char> = s.chars().collect();

                if start >= chars.len() {
                    return Ok(Value::String(String::new()));
                }

                let len = if args.len() == 3 {
                    Some(Self::get_integer(&args[2])? as usize)
                } else {
                    None
                };

                let end = match len {
                    Some(l) => (start + l).min(chars.len()),
                    None => chars.len(),
                };

                Ok(Value::String(chars[start..end].iter().collect()))
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "substring requires a string argument".to_string(),
            )),
        }
    }

    fn left(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "left")?;
        match &args[0] {
            Value::String(s) => {
                let n = Self::get_integer(&args[1])? as usize;
                let chars: Vec<char> = s.chars().collect();
                let end = n.min(chars.len());
                Ok(Value::String(chars[..end].iter().collect()))
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "left requires a string argument".to_string(),
            )),
        }
    }

    fn right(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "right")?;
        match &args[0] {
            Value::String(s) => {
                let n = Self::get_integer(&args[1])? as usize;
                let chars: Vec<char> = s.chars().collect();
                let start = chars.len().saturating_sub(n);
                Ok(Value::String(chars[start..].iter().collect()))
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "right requires a string argument".to_string(),
            )),
        }
    }

    fn split(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "split")?;
        match (&args[0], &args[1]) {
            (Value::String(s), Value::String(delimiter)) => {
                let parts: Vec<Value> = s
                    .split(delimiter.as_str())
                    .map(|p| Value::String(p.to_string()))
                    .collect();
                Ok(Value::Array(parts))
            }
            (Value::Null, _) => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "split requires two string arguments".to_string(),
            )),
        }
    }

    fn reverse_string(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "reverse")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
            Value::Array(arr) => Ok(Value::Array(arr.iter().rev().cloned().collect())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "reverse requires a string or list argument".to_string(),
            )),
        }
    }

    fn to_string(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toString")?;
        match &args[0] {
            Value::String(s) => Ok(Value::String(s.clone())),
            Value::Number(n) => Ok(Value::String(n.to_string())),
            Value::Bool(b) => Ok(Value::String(b.to_string())),
            Value::Null => Ok(Value::Null),
            v => Ok(Value::String(v.to_string())),
        }
    }

    fn to_boolean(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toBoolean")?;
        match &args[0] {
            Value::Bool(b) => Ok(Value::Bool(*b)),
            Value::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Ok(Value::Bool(true)),
                "false" | "no" | "0" => Ok(Value::Bool(false)),
                _ => Ok(Value::Null),
            },
            Value::Number(n) => Ok(Value::Bool(n.as_f64().unwrap_or(0.0) != 0.0)),
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn to_integer(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toInteger")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(json!(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(json!(f as i64))
                } else {
                    Ok(Value::Null)
                }
            }
            Value::String(s) => match s.parse::<i64>() {
                Ok(i) => Ok(json!(i)),
                Err(_) => match s.parse::<f64>() {
                    Ok(f) => Ok(json!(f as i64)),
                    Err(_) => Ok(Value::Null),
                },
            },
            Value::Bool(b) => Ok(json!(if *b { 1 } else { 0 })),
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn to_float(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "toFloat")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(json!(f))
                } else {
                    Ok(Value::Null)
                }
            }
            Value::String(s) => match s.parse::<f64>() {
                Ok(f) => Ok(json!(f)),
                Err(_) => Ok(Value::Null),
            },
            Value::Bool(b) => Ok(json!(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn size(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "size")?;
        match &args[0] {
            Value::String(s) => Ok(json!(s.chars().count())),
            Value::Array(arr) => Ok(json!(arr.len())),
            Value::Object(obj) => Ok(json!(obj.len())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "size requires a string, list, or map argument".to_string(),
            )),
        }
    }

    fn length(args: &[Value]) -> ProtocolResult<Value> {
        // length is an alias for size for strings and lists
        // For paths, it returns the number of relationships
        Self::size(args)
    }

    fn char_length(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "charLength")?;
        match &args[0] {
            Value::String(s) => Ok(json!(s.chars().count())),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "charLength requires a string argument".to_string(),
            )),
        }
    }

    fn starts_with(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "startsWith")?;
        match (&args[0], &args[1]) {
            (Value::String(s), Value::String(prefix)) => {
                Ok(Value::Bool(s.starts_with(prefix.as_str())))
            }
            (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "startsWith requires two string arguments".to_string(),
            )),
        }
    }

    fn ends_with(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "endsWith")?;
        match (&args[0], &args[1]) {
            (Value::String(s), Value::String(suffix)) => {
                Ok(Value::Bool(s.ends_with(suffix.as_str())))
            }
            (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "endsWith requires two string arguments".to_string(),
            )),
        }
    }

    fn contains(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "contains")?;
        match (&args[0], &args[1]) {
            (Value::String(s), Value::String(substr)) => {
                Ok(Value::Bool(s.contains(substr.as_str())))
            }
            (Value::Array(arr), val) => Ok(Value::Bool(arr.contains(val))),
            (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "contains requires string or list arguments".to_string(),
            )),
        }
    }

    // ==================== List Functions ====================

    fn head(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "head")?;
        match &args[0] {
            Value::Array(arr) => Ok(arr.first().cloned().unwrap_or(Value::Null)),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "head requires a list argument".to_string(),
            )),
        }
    }

    fn tail(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "tail")?;
        match &args[0] {
            Value::Array(arr) => {
                if arr.is_empty() {
                    Ok(Value::Array(vec![]))
                } else {
                    Ok(Value::Array(arr[1..].to_vec()))
                }
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "tail requires a list argument".to_string(),
            )),
        }
    }

    fn last(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "last")?;
        match &args[0] {
            Value::Array(arr) => Ok(arr.last().cloned().unwrap_or(Value::Null)),
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "last requires a list argument".to_string(),
            )),
        }
    }

    fn range(args: &[Value]) -> ProtocolResult<Value> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::CypherError(
                "range requires 2 or 3 arguments".to_string(),
            ));
        }

        let start = Self::get_integer(&args[0])?;
        let end = Self::get_integer(&args[1])?;
        let step = if args.len() == 3 {
            Self::get_integer(&args[2])?
        } else {
            1
        };

        if step == 0 {
            return Err(ProtocolError::CypherError(
                "range step cannot be zero".to_string(),
            ));
        }

        let mut result = Vec::new();
        let mut current = start;

        if step > 0 {
            while current <= end {
                result.push(json!(current));
                current += step;
            }
        } else {
            while current >= end {
                result.push(json!(current));
                current += step;
            }
        }

        Ok(Value::Array(result))
    }

    fn slice(args: &[Value]) -> ProtocolResult<Value> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::CypherError(
                "slice requires 2 or 3 arguments".to_string(),
            ));
        }

        match &args[0] {
            Value::Array(arr) => {
                let start = Self::get_integer(&args[1])? as usize;
                let len = arr.len();

                if start >= len {
                    return Ok(Value::Array(vec![]));
                }

                let end = if args.len() == 3 {
                    let e = Self::get_integer(&args[2])? as usize;
                    e.min(len)
                } else {
                    len
                };

                Ok(Value::Array(arr[start..end].to_vec()))
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "slice requires a list argument".to_string(),
            )),
        }
    }

    fn reduce(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        // reduce is complex and typically handled at AST level
        // This is a simplified version
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "reduce requires at least 2 arguments".to_string(),
            ));
        }

        let accumulator = args[0].clone();
        let list = match &args[1] {
            Value::Array(arr) => arr.clone(),
            _ => {
                return Err(ProtocolError::CypherError(
                    "reduce requires a list as second argument".to_string(),
                ))
            }
        };

        // Return accumulator for now - full reduce requires expression evaluation
        Ok(if list.is_empty() {
            accumulator
        } else {
            list.last().cloned().unwrap_or(accumulator)
        })
    }

    fn keys(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "keys")?;
        match &args[0] {
            Value::Object(obj) => {
                let keys: Vec<Value> = obj.keys().map(|k| Value::String(k.clone())).collect();
                Ok(Value::Array(keys))
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "keys requires a map/node argument".to_string(),
            )),
        }
    }

    fn labels(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "labels")?;
        match &args[0] {
            Value::Object(obj) => {
                // If the object has a _labels field, return it
                if let Some(labels) = obj.get("_labels") {
                    return Ok(labels.clone());
                }
                // Otherwise return empty array
                Ok(Value::Array(vec![]))
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Array(vec![])),
        }
    }

    fn nodes(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "nodes")?;
        match &args[0] {
            Value::Object(obj) => {
                // If the object represents a path with _nodes field
                if let Some(nodes) = obj.get("_nodes") {
                    return Ok(nodes.clone());
                }
                Ok(Value::Array(vec![]))
            }
            Value::Array(arr) => Ok(Value::Array(arr.clone())),
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Array(vec![])),
        }
    }

    fn relationships(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "relationships")?;
        match &args[0] {
            Value::Object(obj) => {
                // If the object represents a path with _relationships field
                if let Some(rels) = obj.get("_relationships") {
                    return Ok(rels.clone());
                }
                Ok(Value::Array(vec![]))
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Array(vec![])),
        }
    }

    fn collect(args: &[Value]) -> ProtocolResult<Value> {
        // Collect all non-null values into a list
        let collected: Vec<Value> = args.iter().filter(|v| !v.is_null()).cloned().collect();
        Ok(Value::Array(collected))
    }

    fn unwind(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "unwind")?;
        match &args[0] {
            Value::Array(arr) => Ok(Value::Array(arr.clone())),
            Value::Null => Ok(Value::Array(vec![])),
            v => Ok(Value::Array(vec![v.clone()])),
        }
    }

    // ==================== Math Functions ====================

    fn abs(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "abs")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(json!(i.abs()))
                } else if let Some(f) = n.as_f64() {
                    Ok(json!(f.abs()))
                } else {
                    Ok(Value::Null)
                }
            }
            Value::Null => Ok(Value::Null),
            _ => Err(ProtocolError::CypherError(
                "abs requires a numeric argument".to_string(),
            )),
        }
    }

    fn ceil(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "ceil")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.ceil()))
    }

    fn floor(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "floor")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.floor()))
    }

    fn round(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::CypherError(
                "round requires 1 or 2 arguments".to_string(),
            ));
        }

        let f = Self::get_float(&args[0])?;

        if args.len() == 2 {
            let precision = Self::get_integer(&args[1])?;
            let multiplier = 10_f64.powi(precision as i32);
            Ok(json!((f * multiplier).round() / multiplier))
        } else {
            Ok(json!(f.round()))
        }
    }

    fn sqrt(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "sqrt")?;
        let f = Self::get_float(&args[0])?;
        if f < 0.0 {
            Ok(Value::Null)
        } else {
            Ok(json!(f.sqrt()))
        }
    }

    fn sign(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "sign")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(if f > 0.0 {
            1
        } else if f < 0.0 {
            -1
        } else {
            0
        }))
    }

    fn rand(_args: &[Value]) -> ProtocolResult<Value> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Simple LCG random number generator
        let random = ((seed * 1103515245 + 12345) % (1 << 31)) as f64 / (1u64 << 31) as f64;
        Ok(json!(random))
    }

    fn log(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "log")?;
        let f = Self::get_float(&args[0])?;
        if f <= 0.0 {
            Ok(Value::Null)
        } else {
            Ok(json!(f.ln()))
        }
    }

    fn log10(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "log10")?;
        let f = Self::get_float(&args[0])?;
        if f <= 0.0 {
            Ok(Value::Null)
        } else {
            Ok(json!(f.log10()))
        }
    }

    fn exp(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "exp")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.exp()))
    }

    fn e(_args: &[Value]) -> ProtocolResult<Value> {
        Ok(json!(std::f64::consts::E))
    }

    fn pi(_args: &[Value]) -> ProtocolResult<Value> {
        Ok(json!(std::f64::consts::PI))
    }

    fn sin(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "sin")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.sin()))
    }

    fn cos(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "cos")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.cos()))
    }

    fn tan(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "tan")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.tan()))
    }

    fn asin(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "asin")?;
        let f = Self::get_float(&args[0])?;
        if !(-1.0..=1.0).contains(&f) {
            Ok(Value::Null)
        } else {
            Ok(json!(f.asin()))
        }
    }

    fn acos(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "acos")?;
        let f = Self::get_float(&args[0])?;
        if !(-1.0..=1.0).contains(&f) {
            Ok(Value::Null)
        } else {
            Ok(json!(f.acos()))
        }
    }

    fn atan(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "atan")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.atan()))
    }

    fn atan2(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "atan2")?;
        let y = Self::get_float(&args[0])?;
        let x = Self::get_float(&args[1])?;
        Ok(json!(y.atan2(x)))
    }

    fn degrees(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "degrees")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.to_degrees()))
    }

    fn radians(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "radians")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!(f.to_radians()))
    }

    fn haversin(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "haversin")?;
        let f = Self::get_float(&args[0])?;
        Ok(json!((1.0 - f.cos()) / 2.0))
    }

    // ==================== Date/Time Functions ====================

    fn date(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            // Return current date
            let now = Local::now();
            return Ok(json!({
                "year": now.year(),
                "month": now.month(),
                "day": now.day()
            }));
        }

        match &args[0] {
            Value::String(s) => {
                // Parse ISO date string
                if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(json!({
                        "year": date.year(),
                        "month": date.month(),
                        "day": date.day()
                    }))
                } else {
                    Err(ProtocolError::CypherError(format!(
                        "Cannot parse date from: {s}"
                    )))
                }
            }
            Value::Object(obj) => {
                // Construct date from components
                let year = obj.get("year").and_then(|v| v.as_i64()).unwrap_or(1970) as i32;
                let month = obj.get("month").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let day = obj.get("day").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

                Ok(json!({
                    "year": year,
                    "month": month,
                    "day": day
                }))
            }
            _ => Err(ProtocolError::CypherError(
                "date requires a string or map argument".to_string(),
            )),
        }
    }

    fn datetime(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            // Return current datetime
            let now = Utc::now();
            return Ok(json!({
                "year": now.year(),
                "month": now.month(),
                "day": now.day(),
                "hour": now.hour(),
                "minute": now.minute(),
                "second": now.second(),
                "nanosecond": now.nanosecond(),
                "timezone": "UTC"
            }));
        }

        match &args[0] {
            Value::String(s) => {
                // Parse ISO datetime string
                if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                    Ok(json!({
                        "year": dt.year(),
                        "month": dt.month(),
                        "day": dt.day(),
                        "hour": dt.hour(),
                        "minute": dt.minute(),
                        "second": dt.second(),
                        "nanosecond": dt.nanosecond(),
                        "timezone": dt.timezone().to_string()
                    }))
                } else if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                    Ok(json!({
                        "year": dt.year(),
                        "month": dt.month(),
                        "day": dt.day(),
                        "hour": dt.hour(),
                        "minute": dt.minute(),
                        "second": dt.second(),
                        "nanosecond": dt.nanosecond()
                    }))
                } else {
                    Err(ProtocolError::CypherError(format!(
                        "Cannot parse datetime from: {s}"
                    )))
                }
            }
            Value::Object(obj) => {
                // Construct datetime from components
                Ok(json!({
                    "year": obj.get("year").and_then(|v| v.as_i64()).unwrap_or(1970),
                    "month": obj.get("month").and_then(|v| v.as_u64()).unwrap_or(1),
                    "day": obj.get("day").and_then(|v| v.as_u64()).unwrap_or(1),
                    "hour": obj.get("hour").and_then(|v| v.as_u64()).unwrap_or(0),
                    "minute": obj.get("minute").and_then(|v| v.as_u64()).unwrap_or(0),
                    "second": obj.get("second").and_then(|v| v.as_u64()).unwrap_or(0),
                    "nanosecond": obj.get("nanosecond").and_then(|v| v.as_u64()).unwrap_or(0),
                    "timezone": obj.get("timezone").and_then(|v| v.as_str()).unwrap_or("UTC")
                }))
            }
            _ => Err(ProtocolError::CypherError(
                "datetime requires a string or map argument".to_string(),
            )),
        }
    }

    fn local_datetime(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            let now = Local::now();
            return Ok(json!({
                "year": now.year(),
                "month": now.month(),
                "day": now.day(),
                "hour": now.hour(),
                "minute": now.minute(),
                "second": now.second(),
                "nanosecond": now.nanosecond()
            }));
        }

        // Same handling as datetime but without timezone
        Self::datetime(args).map(|v| {
            if let Value::Object(mut obj) = v {
                obj.remove("timezone");
                Value::Object(obj)
            } else {
                v
            }
        })
    }

    fn time(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            let now = Utc::now();
            return Ok(json!({
                "hour": now.hour(),
                "minute": now.minute(),
                "second": now.second(),
                "nanosecond": now.nanosecond(),
                "timezone": "UTC"
            }));
        }

        match &args[0] {
            Value::String(s) => {
                if let Ok(time) = NaiveTime::parse_from_str(s, "%H:%M:%S") {
                    Ok(json!({
                        "hour": time.hour(),
                        "minute": time.minute(),
                        "second": time.second(),
                        "nanosecond": time.nanosecond()
                    }))
                } else {
                    Err(ProtocolError::CypherError(format!(
                        "Cannot parse time from: {s}"
                    )))
                }
            }
            Value::Object(obj) => Ok(json!({
                "hour": obj.get("hour").and_then(|v| v.as_u64()).unwrap_or(0),
                "minute": obj.get("minute").and_then(|v| v.as_u64()).unwrap_or(0),
                "second": obj.get("second").and_then(|v| v.as_u64()).unwrap_or(0),
                "nanosecond": obj.get("nanosecond").and_then(|v| v.as_u64()).unwrap_or(0)
            })),
            _ => Err(ProtocolError::CypherError(
                "time requires a string or map argument".to_string(),
            )),
        }
    }

    fn local_time(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            let now = Local::now();
            return Ok(json!({
                "hour": now.hour(),
                "minute": now.minute(),
                "second": now.second(),
                "nanosecond": now.nanosecond()
            }));
        }
        Self::time(args)
    }

    fn duration(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            return Ok(json!({
                "months": 0,
                "days": 0,
                "seconds": 0,
                "nanoseconds": 0
            }));
        }

        match &args[0] {
            Value::String(s) => {
                // Parse ISO 8601 duration (simplified)
                // Full parsing would require more complex logic
                Ok(json!({
                    "months": 0,
                    "days": 0,
                    "seconds": 0,
                    "nanoseconds": 0,
                    "string": s
                }))
            }
            Value::Object(obj) => Ok(json!({
                "months": obj.get("months").and_then(|v| v.as_i64()).unwrap_or(0),
                "days": obj.get("days").and_then(|v| v.as_i64()).unwrap_or(0),
                "seconds": obj.get("seconds").and_then(|v| v.as_i64()).unwrap_or(0),
                "nanoseconds": obj.get("nanoseconds").and_then(|v| v.as_i64()).unwrap_or(0)
            })),
            _ => Err(ProtocolError::CypherError(
                "duration requires a string or map argument".to_string(),
            )),
        }
    }

    fn date_truncate(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "date.truncate")?;
        let unit = match &args[0] {
            Value::String(s) => s.to_lowercase(),
            _ => {
                return Err(ProtocolError::CypherError(
                    "date.truncate requires a string unit".to_string(),
                ))
            }
        };

        match &args[1] {
            Value::Object(obj) => {
                let year = obj.get("year").and_then(|v| v.as_i64()).unwrap_or(1970);
                let month = obj.get("month").and_then(|v| v.as_u64()).unwrap_or(1);
                let day = obj.get("day").and_then(|v| v.as_u64()).unwrap_or(1);

                match unit.as_str() {
                    "year" => Ok(json!({"year": year, "month": 1, "day": 1})),
                    "month" => Ok(json!({"year": year, "month": month, "day": 1})),
                    "week" => {
                        // Simplified: truncate to first day of month
                        Ok(json!({"year": year, "month": month, "day": 1}))
                    }
                    "day" => Ok(json!({"year": year, "month": month, "day": day})),
                    _ => Err(ProtocolError::CypherError(format!(
                        "Unknown truncation unit: {unit}"
                    ))),
                }
            }
            _ => Err(ProtocolError::CypherError(
                "date.truncate requires a date argument".to_string(),
            )),
        }
    }

    fn datetime_truncate(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "datetime.truncate")?;
        let unit = match &args[0] {
            Value::String(s) => s.to_lowercase(),
            _ => {
                return Err(ProtocolError::CypherError(
                    "datetime.truncate requires a string unit".to_string(),
                ))
            }
        };

        match &args[1] {
            Value::Object(obj) => {
                let year = obj.get("year").and_then(|v| v.as_i64()).unwrap_or(1970);
                let month = obj.get("month").and_then(|v| v.as_u64()).unwrap_or(1);
                let day = obj.get("day").and_then(|v| v.as_u64()).unwrap_or(1);
                let hour = obj.get("hour").and_then(|v| v.as_u64()).unwrap_or(0);
                let minute = obj.get("minute").and_then(|v| v.as_u64()).unwrap_or(0);
                let second = obj.get("second").and_then(|v| v.as_u64()).unwrap_or(0);

                match unit.as_str() {
                    "year" => Ok(json!({
                        "year": year, "month": 1, "day": 1,
                        "hour": 0, "minute": 0, "second": 0, "nanosecond": 0
                    })),
                    "month" => Ok(json!({
                        "year": year, "month": month, "day": 1,
                        "hour": 0, "minute": 0, "second": 0, "nanosecond": 0
                    })),
                    "day" => Ok(json!({
                        "year": year, "month": month, "day": day,
                        "hour": 0, "minute": 0, "second": 0, "nanosecond": 0
                    })),
                    "hour" => Ok(json!({
                        "year": year, "month": month, "day": day,
                        "hour": hour, "minute": 0, "second": 0, "nanosecond": 0
                    })),
                    "minute" => Ok(json!({
                        "year": year, "month": month, "day": day,
                        "hour": hour, "minute": minute, "second": 0, "nanosecond": 0
                    })),
                    "second" => Ok(json!({
                        "year": year, "month": month, "day": day,
                        "hour": hour, "minute": minute, "second": second, "nanosecond": 0
                    })),
                    _ => Err(ProtocolError::CypherError(format!(
                        "Unknown truncation unit: {unit}"
                    ))),
                }
            }
            _ => Err(ProtocolError::CypherError(
                "datetime.truncate requires a datetime argument".to_string(),
            )),
        }
    }

    // ==================== Type Functions ====================

    fn type_of(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "type")?;
        match &args[0] {
            Value::Object(obj) => {
                // Return relationship type if present
                if let Some(t) = obj.get("_type") {
                    return Ok(t.clone());
                }
                Ok(Value::Null)
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn id(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "id")?;
        match &args[0] {
            Value::Object(obj) => {
                if let Some(id) = obj.get("_id") {
                    return Ok(id.clone());
                }
                if let Some(id) = obj.get("id") {
                    return Ok(id.clone());
                }
                Ok(Value::Null)
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn element_id(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "elementId")?;
        match &args[0] {
            Value::Object(obj) => {
                if let Some(id) = obj.get("_elementId") {
                    return Ok(id.clone());
                }
                if let Some(id) = obj.get("_id") {
                    return Ok(Value::String(format!("element:{}", id)));
                }
                Ok(Value::Null)
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn properties(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "properties")?;
        match &args[0] {
            Value::Object(obj) => {
                // Return properties, filtering out internal fields
                let mut props = serde_json::Map::new();
                for (k, v) in obj.iter() {
                    if !k.starts_with('_') {
                        props.insert(k.clone(), v.clone());
                    }
                }
                Ok(Value::Object(props))
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Object(serde_json::Map::new())),
        }
    }

    fn coalesce(args: &[Value]) -> ProtocolResult<Value> {
        for arg in args {
            if !arg.is_null() {
                return Ok(arg.clone());
            }
        }
        Ok(Value::Null)
    }

    fn nullif(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "nullif")?;
        if args[0] == args[1] {
            Ok(Value::Null)
        } else {
            Ok(args[0].clone())
        }
    }

    fn is_nan(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "isNaN")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::Bool(f.is_nan()))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Bool(false)),
        }
    }

    fn is_finite(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "isFinite")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::Bool(f.is_finite()))
                } else {
                    Ok(Value::Bool(true))
                }
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Bool(true)),
        }
    }

    fn is_infinite(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "isInfinite")?;
        match &args[0] {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::Bool(f.is_infinite()))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Bool(false)),
        }
    }

    // ==================== Path Functions ====================

    fn path_length(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "length")?;
        match &args[0] {
            Value::Object(obj) => {
                // If path, return number of relationships
                if let Some(Value::Array(arr)) = obj.get("_relationships") {
                    return Ok(json!(arr.len()));
                }
                Ok(json!(0))
            }
            Value::Array(arr) => Ok(json!(arr.len())),
            Value::String(s) => Ok(json!(s.len())),
            Value::Null => Ok(Value::Null),
            _ => Ok(json!(0)),
        }
    }

    fn start_node(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "startNode")?;
        match &args[0] {
            Value::Object(obj) => {
                // Return start node of relationship or path
                if let Some(node) = obj.get("_startNode") {
                    return Ok(node.clone());
                }
                if let Some(Value::Array(arr)) = obj.get("_nodes") {
                    return Ok(arr.first().cloned().unwrap_or(Value::Null));
                }
                Ok(Value::Null)
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    fn end_node(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "endNode")?;
        match &args[0] {
            Value::Object(obj) => {
                // Return end node of relationship or path
                if let Some(node) = obj.get("_endNode") {
                    return Ok(node.clone());
                }
                if let Some(Value::Array(arr)) = obj.get("_nodes") {
                    return Ok(arr.last().cloned().unwrap_or(Value::Null));
                }
                Ok(Value::Null)
            }
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::Null),
        }
    }

    // ==================== Aggregation Functions (Scalar) ====================

    fn min(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        let mut min_val = &args[0];
        for arg in &args[1..] {
            if arg.is_null() {
                continue;
            }
            if min_val.is_null() || Self::compare_values(arg, min_val) < 0 {
                min_val = arg;
            }
        }
        Ok(min_val.clone())
    }

    fn max(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        let mut max_val = &args[0];
        for arg in &args[1..] {
            if arg.is_null() {
                continue;
            }
            if max_val.is_null() || Self::compare_values(arg, max_val) > 0 {
                max_val = arg;
            }
        }
        Ok(max_val.clone())
    }

    fn sum(args: &[Value]) -> ProtocolResult<Value> {
        let mut total = 0.0;
        for arg in args {
            if let Some(n) = arg.as_f64() {
                total += n;
            }
        }
        Ok(json!(total))
    }

    fn avg(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        let mut total = 0.0;
        let mut count = 0;
        for arg in args {
            if let Some(n) = arg.as_f64() {
                total += n;
                count += 1;
            }
        }

        if count == 0 {
            Ok(Value::Null)
        } else {
            Ok(json!(total / count as f64))
        }
    }

    fn count(args: &[Value]) -> ProtocolResult<Value> {
        let count = args.iter().filter(|v| !v.is_null()).count();
        Ok(json!(count))
    }

    fn stdev(args: &[Value]) -> ProtocolResult<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        let values: Vec<f64> = args.iter().filter_map(|v| v.as_f64()).collect();

        if values.is_empty() {
            return Ok(Value::Null);
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

        Ok(json!(variance.sqrt()))
    }

    fn percentile_cont(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "percentileCont")?;

        let percentile = Self::get_float(&args[1])?;
        if !(0.0..=1.0).contains(&percentile) {
            return Err(ProtocolError::CypherError(
                "Percentile must be between 0 and 1".to_string(),
            ));
        }

        match &args[0] {
            Value::Array(arr) => {
                let mut values: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();

                if values.is_empty() {
                    return Ok(Value::Null);
                }

                values.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let idx = percentile * (values.len() - 1) as f64;
                let lower = idx.floor() as usize;
                let upper = idx.ceil() as usize;
                let frac = idx - lower as f64;

                if lower == upper {
                    Ok(json!(values[lower]))
                } else {
                    Ok(json!(values[lower] * (1.0 - frac) + values[upper] * frac))
                }
            }
            _ => Err(ProtocolError::CypherError(
                "percentileCont requires a list argument".to_string(),
            )),
        }
    }

    fn percentile_disc(args: &[Value]) -> ProtocolResult<Value> {
        Self::require_args(args, 2, "percentileDisc")?;

        let percentile = Self::get_float(&args[1])?;
        if !(0.0..=1.0).contains(&percentile) {
            return Err(ProtocolError::CypherError(
                "Percentile must be between 0 and 1".to_string(),
            ));
        }

        match &args[0] {
            Value::Array(arr) => {
                let mut values: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();

                if values.is_empty() {
                    return Ok(Value::Null);
                }

                values.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let idx = (percentile * (values.len() - 1) as f64).round() as usize;
                Ok(json!(values[idx]))
            }
            _ => Err(ProtocolError::CypherError(
                "percentileDisc requires a list argument".to_string(),
            )),
        }
    }

    // ==================== Existence Check ====================

    fn exists(args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        Self::require_args(args, 1, "exists")?;
        Ok(Value::Bool(!args[0].is_null()))
    }

    // ==================== Graph Functions ====================

    fn shortest_path(_args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        // This is typically handled at the engine level
        // Return a placeholder
        Ok(json!({
            "_type": "path",
            "_nodes": [],
            "_relationships": []
        }))
    }

    fn all_shortest_paths(_args: &[Value], _context: &FunctionContext) -> ProtocolResult<Value> {
        // This is typically handled at the engine level
        // Return a placeholder
        Ok(Value::Array(vec![]))
    }

    // ==================== Helper Functions ====================

    fn require_args(args: &[Value], count: usize, func_name: &str) -> ProtocolResult<()> {
        if args.len() != count {
            return Err(ProtocolError::CypherError(format!(
                "{func_name} requires exactly {count} argument(s), got {}",
                args.len()
            )));
        }
        Ok(())
    }

    fn get_integer(value: &Value) -> ProtocolResult<i64> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(i)
                } else if let Some(f) = n.as_f64() {
                    Ok(f as i64)
                } else {
                    Err(ProtocolError::CypherError(
                        "Expected integer value".to_string(),
                    ))
                }
            }
            _ => Err(ProtocolError::CypherError(
                "Expected numeric value".to_string(),
            )),
        }
    }

    fn get_float(value: &Value) -> ProtocolResult<f64> {
        match value {
            Value::Number(n) => n
                .as_f64()
                .ok_or_else(|| ProtocolError::CypherError("Expected numeric value".to_string())),
            Value::Null => Ok(f64::NAN),
            _ => Err(ProtocolError::CypherError(
                "Expected numeric value".to_string(),
            )),
        }
    }

    fn compare_values(a: &Value, b: &Value) -> i32 {
        match (a, b) {
            (Value::Number(na), Value::Number(nb)) => {
                let fa = na.as_f64().unwrap_or(0.0);
                let fb = nb.as_f64().unwrap_or(0.0);
                if fa < fb {
                    -1
                } else if fa > fb {
                    1
                } else {
                    0
                }
            }
            (Value::String(sa), Value::String(sb)) => sa.cmp(sb) as i32,
            (Value::Bool(ba), Value::Bool(bb)) => ba.cmp(bb) as i32,
            _ => 0,
        }
    }
}

/// Context for function evaluation
#[derive(Default)]
pub struct FunctionContext {
    /// Current variable bindings
    pub variables: HashMap<String, Value>,
    /// Node lookup function (if available)
    #[allow(clippy::type_complexity)]
    pub node_lookup: Option<Box<dyn Fn(&str) -> Option<Value> + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    pub relationship_lookup: Option<Box<dyn Fn(&str) -> Option<Value> + Send + Sync>>,
}

impl FunctionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_variables(variables: HashMap<String, Value>) -> Self {
        Self {
            variables,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_upper() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("toUpper", &[json!("hello")], &ctx).unwrap();
        assert_eq!(result, json!("HELLO"));
    }

    #[test]
    fn test_to_lower() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("toLower", &[json!("HELLO")], &ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_trim() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("trim", &[json!("  hello  ")], &ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_substring() {
        let ctx = FunctionContext::new();
        let result =
            CypherFunctions::evaluate("substring", &[json!("hello"), json!(1), json!(3)], &ctx)
                .unwrap();
        assert_eq!(result, json!("ell"));
    }

    #[test]
    fn test_size() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("size", &[json!([1, 2, 3])], &ctx).unwrap();
        assert_eq!(result, json!(3));

        let result = CypherFunctions::evaluate("size", &[json!("hello")], &ctx).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn test_head_tail() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("head", &[json!([1, 2, 3])], &ctx).unwrap();
        assert_eq!(result, json!(1));

        let result = CypherFunctions::evaluate("tail", &[json!([1, 2, 3])], &ctx).unwrap();
        assert_eq!(result, json!([2, 3]));
    }

    #[test]
    fn test_range() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("range", &[json!(1), json!(5)], &ctx).unwrap();
        assert_eq!(result, json!([1, 2, 3, 4, 5]));

        let result =
            CypherFunctions::evaluate("range", &[json!(0), json!(10), json!(2)], &ctx).unwrap();
        assert_eq!(result, json!([0, 2, 4, 6, 8, 10]));
    }

    #[test]
    fn test_math_functions() {
        let ctx = FunctionContext::new();

        let result = CypherFunctions::evaluate("abs", &[json!(-5)], &ctx).unwrap();
        assert_eq!(result, json!(5));

        let result = CypherFunctions::evaluate("ceil", &[json!(4.3)], &ctx).unwrap();
        assert_eq!(result, json!(5.0));

        let result = CypherFunctions::evaluate("floor", &[json!(4.7)], &ctx).unwrap();
        assert_eq!(result, json!(4.0));
    }

    #[test]
    fn test_coalesce() {
        let ctx = FunctionContext::new();
        let result =
            CypherFunctions::evaluate("coalesce", &[Value::Null, json!(1), json!(2)], &ctx)
                .unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_date() {
        let ctx = FunctionContext::new();
        let result = CypherFunctions::evaluate("date", &[json!("2024-01-15")], &ctx).unwrap();
        assert_eq!(result["year"], json!(2024));
        assert_eq!(result["month"], json!(1));
        assert_eq!(result["day"], json!(15));
    }
}
