//! Time series query language and operations

use super::{AggregationType, HashMap, QueryResult, SeriesId, TimeRange, Timestamp, Utc};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Time series query builder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesQuery {
    pub series_selector: SeriesSelector,
    pub time_range: TimeRange,
    pub aggregation: Option<AggregationQuery>,
    pub filters: Vec<Filter>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Series selector for querying
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeriesSelector {
    ById(SeriesId),
    ByName(String),
    ByLabels(HashMap<String, String>),
    All,
}

/// Aggregation query specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationQuery {
    pub aggregation_type: AggregationType,
    pub window_size: Duration,
    pub step: Option<Duration>,
}

/// Value filter for time series data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    ValueGreaterThan(f64),
    ValueLessThan(f64),
    ValueEquals(f64),
    ValueBetween(f64, f64),
    LabelEquals(String, String),
    LabelExists(String),
}

impl Default for TimeSeriesQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeSeriesQuery {
    pub fn new() -> Self {
        Self {
            series_selector: SeriesSelector::All,
            time_range: TimeRange {
                start: 0,
                end: Utc::now().timestamp_nanos_opt().unwrap_or(0),
            },
            aggregation: None,
            filters: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    pub fn select_series(mut self, selector: SeriesSelector) -> Self {
        self.series_selector = selector;
        self
    }

    pub fn time_range(mut self, start: Timestamp, end: Timestamp) -> Self {
        self.time_range = TimeRange { start, end };
        self
    }

    pub fn aggregate(mut self, agg_type: AggregationType, window_size: Duration) -> Self {
        self.aggregation = Some(AggregationQuery {
            aggregation_type: agg_type,
            window_size,
            step: None,
        });
        self
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Time series query executor
pub struct QueryExecutor {
    // TODO: Add reference to time series engine
}

impl Default for QueryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryExecutor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn execute(&self, _query: TimeSeriesQuery) -> Result<Vec<QueryResult>> {
        // TODO: Implement query execution
        Err(anyhow::anyhow!("Query execution not yet implemented"))
    }

    pub async fn explain(&self, _query: TimeSeriesQuery) -> Result<QueryPlan> {
        // TODO: Implement query planning and explanation
        Err(anyhow::anyhow!("Query explanation not yet implemented"))
    }
}

/// Query execution plan
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub estimated_cost: f64,
    pub estimated_rows: usize,
    pub execution_steps: Vec<ExecutionStep>,
}

/// Query execution step
#[derive(Debug, Clone)]
pub enum ExecutionStep {
    ScanSeries(SeriesSelector),
    FilterTime(TimeRange),
    FilterValues(Vec<Filter>),
    Aggregate(AggregationQuery),
    Limit(usize),
    Sort,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_query_builder() {
        let query = TimeSeriesQuery::new()
            .select_series(SeriesSelector::ByName("cpu_usage".to_string()))
            .time_range(1000, 2000)
            .aggregate(AggregationType::Average, Duration::from_secs(60))
            .filter(Filter::ValueGreaterThan(50.0))
            .limit(100);

        match query.series_selector {
            SeriesSelector::ByName(name) => assert_eq!(name, "cpu_usage"),
            _ => panic!("Wrong series selector"),
        }

        assert_eq!(query.time_range.start, 1000);
        assert_eq!(query.time_range.end, 2000);
        assert!(query.aggregation.is_some());
        assert_eq!(query.filters.len(), 1);
        assert_eq!(query.limit, Some(100));
    }
}
