//! Statement execution and history.
//!
//! The executor is deliberately thin: it resolves a connection to a live
//! session, applies the caller's timeout, times the call and records what
//! happened. Everything protocol-specific — how a statement is sent and how a
//! response becomes rows — lives with the session in [`crate::connections`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::connections::{ConnectionError, ConnectionManager};

/// Upper bound on retained history entries, so a long session cannot grow the
/// executor without limit.
const MAX_HISTORY_ENTRIES: usize = 500;

/// Timeout applied when the caller does not specify one.
const DEFAULT_STATEMENT_TIMEOUT: Duration = Duration::from_secs(30);

/// One statement to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub connection_id: String,
    pub query: String,
    /// Statement timeout in milliseconds. Falls back to 30s when absent.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl QueryRequest {
    fn timeout(&self) -> Duration {
        self.timeout_ms
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_STATEMENT_TIMEOUT)
    }
}

/// A result set column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    /// The server's own type name, not a guess derived from a Debug rendering.
    #[serde(rename = "type")]
    pub column_type: String,
}

impl ColumnInfo {
    pub fn new(name: impl Into<String>, column_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            column_type: column_type.into(),
        }
    }
}

/// What a statement did.
///
/// Kept distinct because "10 rows came back" and "10 rows were modified" are
/// different facts, and a single `rows_affected` field cannot tell them apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatementOutcome {
    /// A result set of `rows` rows was returned.
    Returned { rows: u64 },
    /// `rows` rows were inserted, updated or deleted.
    Affected { rows: u64 },
    /// Completed with no row count of either kind (DDL, `SET`, Redis replies).
    Completed,
}

/// Rows and columns produced by one statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPayload {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub outcome: StatementOutcome,
}

impl QueryPayload {
    /// A statement that returned a result set.
    pub fn returned(
        columns: Vec<ColumnInfo>,
        rows: Vec<HashMap<String, serde_json::Value>>,
    ) -> Self {
        let count = rows.len() as u64;
        Self {
            columns,
            rows,
            outcome: StatementOutcome::Returned { rows: count },
        }
    }

    /// A statement that modified rows without returning any.
    pub fn affected(rows: u64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            outcome: StatementOutcome::Affected { rows },
        }
    }

    /// A single scalar reply rendered as a one-cell grid.
    pub fn single_value(
        column: &str,
        column_type: &str,
        value: serde_json::Value,
    ) -> Self {
        let row = std::iter::once((column.to_string(), value)).collect();
        Self {
            columns: vec![ColumnInfo::new(column, column_type)],
            rows: vec![row],
            outcome: StatementOutcome::Completed,
        }
    }

    /// Shape a JSON response from an HTTP-backed protocol.
    ///
    /// Handles the two layouts Orbit's REST surface and the ArangoDB/Neo4j
    /// compatible endpoints use: `data.rows` as positional arrays alongside
    /// `data.columns`, or a plain array of objects under `data`/`result`.
    pub fn from_json(payload: &serde_json::Value) -> Self {
        let body = payload
            .get("data")
            .or_else(|| payload.get("result"))
            .unwrap_or(payload);

        if let Some(rows) = body.as_array() {
            return Self::from_object_array(rows);
        }

        let columns: Vec<ColumnInfo> = body
            .get("columns")
            .and_then(|c| c.as_array())
            .map(|columns| {
                columns
                    .iter()
                    .map(|column| {
                        let name = column
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or_default();
                        let ty = column
                            .get("type")
                            .or_else(|| column.get("data_type"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        ColumnInfo::new(name, ty)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let Some(rows) = body.get("rows").and_then(|r| r.as_array()) else {
            return Self {
                columns,
                rows: Vec::new(),
                outcome: StatementOutcome::Completed,
            };
        };

        // Positional rows need the column list to be named; objects carry their
        // own keys and are taken as-is.
        let shaped: Vec<HashMap<String, serde_json::Value>> = rows
            .iter()
            .map(|row| match row {
                serde_json::Value::Array(values) => values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let name = columns
                            .get(index)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| format!("column_{index}"));
                        (name, value.clone())
                    })
                    .collect(),
                serde_json::Value::Object(fields) => {
                    fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                }
                other => std::iter::once(("result".to_string(), other.clone())).collect(),
            })
            .collect();

        Self::returned(columns, shaped)
    }

    fn from_object_array(rows: &[serde_json::Value]) -> Self {
        // Column order follows first appearance across all rows, so a key that
        // only shows up in a later row still gets a column.
        let mut columns: Vec<ColumnInfo> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for row in rows {
            if let Some(fields) = row.as_object() {
                for key in fields.keys() {
                    if seen.insert(key.clone()) {
                        columns.push(ColumnInfo::new(key.clone(), "unknown"));
                    }
                }
            }
        }

        if columns.is_empty() {
            columns.push(ColumnInfo::new("result", "unknown"));
        }

        let shaped = rows
            .iter()
            .map(|row| match row.as_object() {
                Some(fields) => fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                None => std::iter::once(("result".to_string(), row.clone())).collect(),
            })
            .collect();

        Self::returned(columns, shaped)
    }
}

/// The outcome of one execution, as sent to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub success: bool,
    pub data: Option<QueryPayload>,
    pub error: Option<String>,
    /// Wall-clock time for the statement, in milliseconds.
    pub execution_time_ms: f64,
    /// Set when the result needs a caveat the grid alone cannot convey.
    pub notice: Option<String>,
}

impl QueryResult {
    fn failure(error: String, elapsed: Duration) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            execution_time_ms: elapsed.as_secs_f64() * 1000.0,
            notice: None,
        }
    }
}

/// One past execution, retained for the history panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    pub connection_id: String,
    pub query: String,
    pub executed_at: DateTime<Utc>,
    pub execution_time_ms: f64,
    pub success: bool,
    pub error: Option<String>,
    pub outcome: Option<StatementOutcome>,
}

/// Runs statements and remembers what was run.
#[derive(Default)]
pub struct QueryExecutor {
    history: VecDeque<QueryHistoryEntry>,
}

impl QueryExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute one statement against the request's connection.
    ///
    /// A failing statement is reported as an unsuccessful [`QueryResult`], not
    /// an `Err`: the UI needs the timing and the message either way. `Err` is
    /// reserved for not being able to reach the connection at all.
    ///
    /// # Errors
    /// Returns the connection failure when no session could be opened.
    pub async fn execute(
        &mut self,
        request: QueryRequest,
        connections: &ConnectionManager,
    ) -> Result<QueryResult, ConnectionError> {
        let session = connections.session(&request.connection_id).await?;
        let connection_type = session.lock().await.connection_type();
        let timeout = request.timeout();

        let started = Instant::now();
        let outcome = {
            let mut guard = session.lock().await;
            tokio::time::timeout(timeout, guard.execute(&request.query)).await
        };
        let elapsed = started.elapsed();

        connections.record_use(&request.connection_id).await;

        let result = match outcome {
            Ok(Ok(payload)) => QueryResult {
                success: true,
                execution_time_ms: elapsed.as_secs_f64() * 1000.0,
                // Results that came from a server endpoint known to answer with
                // canned rows are labelled, so example data is never mistaken
                // for the contents of the database.
                notice: (!connection_type.is_native_wire_protocol()).then(|| {
                    format!(
                        "{connection_type} runs over the REST API, whose SQL and catalog \
                         handlers in orbit-server still return fixed example rows. Treat these \
                         results as a protocol check, not as data."
                    )
                }),
                data: Some(payload),
                error: None,
            },
            Ok(Err(e)) => QueryResult::failure(e.to_string(), elapsed),
            Err(_) => QueryResult::failure(
                format!("Statement timed out after {timeout:?}"),
                elapsed,
            ),
        };

        self.record(&request, &result);
        Ok(result)
    }

    /// Ask the server for a plan without running the statement.
    ///
    /// Plain `EXPLAIN` — not `EXPLAIN ANALYZE`, which would execute the
    /// statement and so could delete rows the user only wanted to inspect.
    /// `analyze` opts into that behaviour explicitly.
    ///
    /// # Errors
    /// Returns the connection failure when no session could be opened.
    pub async fn explain(
        &mut self,
        request: QueryRequest,
        analyze: bool,
        connections: &ConnectionManager,
    ) -> Result<QueryResult, ConnectionError> {
        use crate::connections::ConnectionType;

        let session = connections.session(&request.connection_id).await?;
        let connection_type = session.lock().await.connection_type();

        if !matches!(
            connection_type,
            ConnectionType::PostgreSQL | ConnectionType::MySQL
        ) {
            return Ok(QueryResult::failure(
                format!("EXPLAIN is not supported for {connection_type} connections"),
                Duration::ZERO,
            ));
        }

        let prefix = if analyze { "EXPLAIN ANALYZE" } else { "EXPLAIN" };
        self.execute(
            QueryRequest {
                query: format!("{prefix} {}", request.query),
                ..request
            },
            connections,
        )
        .await
    }

    /// Most recent entries for a connection, newest first.
    pub fn history(&self, connection_id: &str, limit: usize) -> Vec<QueryHistoryEntry> {
        self.history
            .iter()
            .rev()
            .filter(|entry| entry.connection_id == connection_id)
            .take(limit)
            .cloned()
            .collect()
    }

    fn record(&mut self, request: &QueryRequest, result: &QueryResult) {
        if self.history.len() >= MAX_HISTORY_ENTRIES {
            self.history.pop_front();
        }

        self.history.push_back(QueryHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            connection_id: request.connection_id.clone(),
            query: request.query.clone(),
            executed_at: Utc::now(),
            execution_time_ms: result.execution_time_ms,
            success: result.success,
            error: result.error.clone(),
            outcome: result.data.as_ref().map(|d| d.outcome.clone()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(connection_id: &str, query: &str) -> QueryRequest {
        QueryRequest {
            connection_id: connection_id.to_string(),
            query: query.to_string(),
            timeout_ms: None,
        }
    }

    fn ok_result() -> QueryResult {
        QueryResult {
            success: true,
            data: Some(QueryPayload::affected(3)),
            error: None,
            execution_time_ms: 1.0,
            notice: None,
        }
    }

    #[test]
    fn timeout_falls_back_to_the_default_and_ignores_zero() {
        assert_eq!(request("c", "SELECT 1").timeout(), DEFAULT_STATEMENT_TIMEOUT);

        let mut req = request("c", "SELECT 1");
        req.timeout_ms = Some(0);
        assert_eq!(req.timeout(), DEFAULT_STATEMENT_TIMEOUT);

        req.timeout_ms = Some(1500);
        assert_eq!(req.timeout(), Duration::from_millis(1500));
    }

    #[test]
    fn returned_and_affected_row_counts_stay_distinguishable() {
        let returned = QueryPayload::returned(
            vec![ColumnInfo::new("id", "int4")],
            vec![std::iter::once(("id".to_string(), serde_json::json!(1))).collect()],
        );
        assert_eq!(returned.outcome, StatementOutcome::Returned { rows: 1 });
        assert_eq!(
            QueryPayload::affected(7).outcome,
            StatementOutcome::Affected { rows: 7 }
        );
    }

    #[test]
    fn history_is_bounded_and_filtered_by_connection() {
        let mut executor = QueryExecutor::new();
        for i in 0..(MAX_HISTORY_ENTRIES + 25) {
            executor.record(&request("a", &format!("SELECT {i}")), &ok_result());
        }
        executor.record(&request("b", "SELECT 'other'"), &ok_result());

        assert_eq!(executor.history.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(executor.history("b", 10).len(), 1);

        let newest = executor.history("a", 3);
        assert_eq!(newest.len(), 3);
        // Newest first: the last statement recorded for "a" leads.
        assert_eq!(
            newest[0].query,
            format!("SELECT {}", MAX_HISTORY_ENTRIES + 24)
        );
    }

    #[test]
    fn positional_json_rows_are_named_from_the_column_list() {
        let payload = QueryPayload::from_json(&serde_json::json!({
            "data": {
                "columns": [
                    { "name": "id", "data_type": "integer" },
                    { "name": "label", "type": "varchar" }
                ],
                "rows": [[1, "one"], [2, "two"]]
            }
        }));

        assert_eq!(payload.columns.len(), 2);
        assert_eq!(payload.columns[0].column_type, "integer");
        assert_eq!(payload.outcome, StatementOutcome::Returned { rows: 2 });
        assert_eq!(payload.rows[1].get("label"), Some(&serde_json::json!("two")));
    }

    #[test]
    fn arrays_of_objects_keep_every_key_as_a_column() {
        let payload = QueryPayload::from_json(&serde_json::json!({
            "result": [ { "a": 1 }, { "a": 2, "b": 3 } ]
        }));

        let names: Vec<&str> = payload.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(payload.outcome, StatementOutcome::Returned { rows: 2 });
    }

    #[test]
    fn a_response_with_no_rows_reports_completion_not_an_empty_result_set() {
        let payload = QueryPayload::from_json(&serde_json::json!({ "data": { "ok": true } }));
        assert_eq!(payload.outcome, StatementOutcome::Completed);
        assert!(payload.rows.is_empty());
    }
}
