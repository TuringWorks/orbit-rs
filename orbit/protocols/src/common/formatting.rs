//! Result formatting utilities for protocol adapters
//!
//! This module provides formatted table output for query results,
//! with support for:
//! - Pretty-printed tables with borders
//! - Column alignment based on data type
//! - Color-coded output for different data types
//! - Multiple output formats (table, JSON, CSV)

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use owo_colors::OwoColorize;
use std::collections::HashMap;

use crate::postgres_wire::sql::types::SqlValue;

/// Output format for query results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Pretty table with borders (default)
    Table,
    /// JSON array of objects
    Json,
    /// CSV format
    Csv,
    /// Plain text (tab-separated)
    Plain,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Table
    }
}

/// Format query results as a pretty table
pub fn format_table(
    columns: &[String],
    rows: &[HashMap<String, SqlValue>],
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Table => format_as_table(columns, rows),
        OutputFormat::Json => format_as_json(columns, rows),
        OutputFormat::Csv => format_as_csv(columns, rows),
        OutputFormat::Plain => format_as_plain(columns, rows),
    }
}

/// Format as a pretty table with UTF-8 borders
fn format_as_table(columns: &[String], rows: &[HashMap<String, SqlValue>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

    // Add header row with colors
    let header_cells: Vec<Cell> = columns
        .iter()
        .map(|col| {
            Cell::new(col)
                .fg(Color::Cyan)
                .set_alignment(comfy_table::CellAlignment::Center)
        })
        .collect();
    table.set_header(header_cells);

    // Add data rows
    for row in rows {
        let cells: Vec<Cell> = columns
            .iter()
            .map(|col| {
                let value = row.get(col).unwrap_or(&SqlValue::Null);
                format_cell_value(value)
            })
            .collect();
        table.add_row(cells);
    }

    // Add summary footer
    let summary = if rows.is_empty() {
        "0 rows".to_string()
    } else {
        format!(
            "{} row{}",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        )
    };

    format!("{}\n\n{}", table, summary.dimmed())
}

/// Format a cell value with appropriate color and alignment
fn format_cell_value(value: &SqlValue) -> Cell {
    match value {
        SqlValue::Null => Cell::new("NULL")
            .fg(Color::Grey)
            .set_alignment(comfy_table::CellAlignment::Center),
        SqlValue::Boolean(b) => {
            let text = if *b { "true" } else { "false" };
            Cell::new(text)
                .fg(Color::Yellow)
                .set_alignment(comfy_table::CellAlignment::Center)
        }
        SqlValue::SmallInt(n) => Cell::new(n.to_string())
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::Integer(n) => Cell::new(n.to_string())
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::BigInt(n) => Cell::new(n.to_string())
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::Real(f) => Cell::new(format!("{:.2}", f))
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::DoublePrecision(f) => Cell::new(format!("{:.2}", f))
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::Decimal(d) => Cell::new(d.to_string())
            .fg(Color::Green)
            .set_alignment(comfy_table::CellAlignment::Right),
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Cell::new(s)
            .fg(Color::White)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Bytea(bytes) => {
            let hex = hex::encode(bytes);
            let display = if hex.len() > 32 {
                format!("{}...", &hex[..32])
            } else {
                hex
            };
            Cell::new(format!("\\x{}", display))
                .fg(Color::Magenta)
                .set_alignment(comfy_table::CellAlignment::Left)
        }
        SqlValue::Timestamp(ts) => Cell::new(ts.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::TimestampWithTimezone(ts) => Cell::new(ts.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Date(d) => Cell::new(d.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Time(t) => Cell::new(t.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::TimeWithTimezone(t) => Cell::new(t.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Interval(i) => {
            let interval_str = format!("{} months {} days {}μs", i.months, i.days, i.microseconds);
            Cell::new(interval_str)
                .fg(Color::Blue)
                .set_alignment(comfy_table::CellAlignment::Left)
        }
        SqlValue::Uuid(u) => Cell::new(u.to_string())
            .fg(Color::Cyan)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Json(j) | SqlValue::Jsonb(j) => {
            let json_str = serde_json::to_string(j).unwrap_or_else(|_| "{}".to_string());
            let display = if json_str.len() > 50 {
                format!("{}...", &json_str[..50])
            } else {
                json_str
            };
            Cell::new(display)
                .fg(Color::Yellow)
                .set_alignment(comfy_table::CellAlignment::Left)
        }
        SqlValue::Array(items) => {
            let display = format!("[{} items]", items.len());
            Cell::new(display)
                .fg(Color::Magenta)
                .set_alignment(comfy_table::CellAlignment::Left)
        }
        SqlValue::Point(x, y) => Cell::new(format!("({}, {})", x, y))
            .fg(Color::Cyan)
            .set_alignment(comfy_table::CellAlignment::Left),
        SqlValue::Inet(s) => Cell::new(s.to_string())
            .fg(Color::Blue)
            .set_alignment(comfy_table::CellAlignment::Left),
        // Handle remaining types with a generic approach
        _ => Cell::new(value.to_postgres_string())
            .fg(Color::White)
            .set_alignment(comfy_table::CellAlignment::Left),
    }
}

/// Format as JSON array
fn format_as_json(columns: &[String], rows: &[HashMap<String, SqlValue>]) -> String {
    let json_rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for col in columns {
                if let Some(value) = row.get(col) {
                    obj.insert(col.clone(), sql_value_to_json(value));
                }
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    serde_json::to_string_pretty(&json_rows).unwrap_or_else(|_| "[]".to_string())
}

/// Convert SqlValue to JSON value
fn sql_value_to_json(value: &SqlValue) -> serde_json::Value {
    match value {
        SqlValue::Null => serde_json::Value::Null,
        SqlValue::Boolean(b) => serde_json::Value::Bool(*b),
        SqlValue::SmallInt(n) => serde_json::Value::Number((*n).into()),
        SqlValue::Integer(n) => serde_json::Value::Number((*n).into()),
        SqlValue::BigInt(n) => serde_json::Value::Number((*n).into()),
        SqlValue::Real(f) => serde_json::Number::from_f64(*f as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        SqlValue::DoublePrecision(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        SqlValue::Decimal(d) => serde_json::Value::String(d.to_string()),
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
            serde_json::Value::String(s.clone())
        }
        SqlValue::Json(j) | SqlValue::Jsonb(j) => j.clone(),
        SqlValue::Uuid(u) => serde_json::Value::String(u.to_string()),
        _ => serde_json::Value::String(value.to_postgres_string()),
    }
}

/// Format as CSV
fn format_as_csv(columns: &[String], rows: &[HashMap<String, SqlValue>]) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&columns.join(","));
    output.push('\n');

    // Rows
    for row in rows {
        let values: Vec<String> = columns
            .iter()
            .map(|col| {
                row.get(col)
                    .map(|v| escape_csv_value(&v.to_postgres_string()))
                    .unwrap_or_else(|| String::new())
            })
            .collect();
        output.push_str(&values.join(","));
        output.push('\n');
    }

    output
}

/// Escape CSV value if needed
fn escape_csv_value(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Format as plain text (tab-separated)
fn format_as_plain(columns: &[String], rows: &[HashMap<String, SqlValue>]) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&columns.join("\t"));
    output.push('\n');

    // Rows
    for row in rows {
        let values: Vec<String> = columns
            .iter()
            .map(|col| {
                row.get(col)
                    .map(|v| v.to_postgres_string())
                    .unwrap_or_else(|| "NULL".to_string())
            })
            .collect();
        output.push_str(&values.join("\t"));
        output.push('\n');
    }

    output
}

/// Format query timing information
pub fn format_timing(elapsed_ms: u64) -> String {
    if elapsed_ms < 1000 {
        format!("Time: {} ms", elapsed_ms).dimmed().to_string()
    } else {
        format!("Time: {:.2} s", elapsed_ms as f64 / 1000.0)
            .dimmed()
            .to_string()
    }
}

/// Format error message with color
pub fn format_error(error: &str) -> String {
    format!("ERROR: {}", error).red().to_string()
}

/// Format success message with color
pub fn format_success(message: &str) -> String {
    message.green().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table() {
        let columns = vec!["id".to_string(), "name".to_string(), "age".to_string()];
        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), SqlValue::Integer(1));
        row1.insert("name".to_string(), SqlValue::Text("Alice".to_string()));
        row1.insert("age".to_string(), SqlValue::Integer(30));

        let rows = vec![row1];
        let output = format_table(&columns, &rows, OutputFormat::Table);

        assert!(output.contains("id"));
        assert!(output.contains("Alice"));
        assert!(output.contains("1 row"));
    }

    #[test]
    fn test_format_json() {
        let columns = vec!["id".to_string(), "value".to_string()];
        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), SqlValue::Integer(1));
        row1.insert("value".to_string(), SqlValue::Text("test".to_string()));

        let rows = vec![row1];
        let output = format_table(&columns, &rows, OutputFormat::Json);

        assert!(output.contains("\"id\""));
        assert!(output.contains("\"value\""));
        assert!(output.contains("test"));
    }
}
