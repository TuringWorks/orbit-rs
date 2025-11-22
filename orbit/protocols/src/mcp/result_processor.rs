//! Result Processing and Formatting
//!
//! This module processes query results for optimal LLM consumption,
//! including summarization, formatting, and visualization hints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result Processor for formatting query results
pub struct ResultProcessor {
    /// Result formatter
    formatter: ResultFormatter,
    /// Data summarizer
    summarizer: DataSummarizer,
    /// Visualization hint generator
    visualizer: VisualizationHintGenerator,
}

impl ResultProcessor {
    /// Create a new result processor
    pub fn new() -> Self {
        Self {
            formatter: ResultFormatter::new(),
            summarizer: DataSummarizer::new(),
            visualizer: VisualizationHintGenerator::new(),
        }
    }

    /// Process query results for LLM consumption
    pub fn process_results(
        &self,
        results: &QueryResult,
        max_preview_rows: usize,
    ) -> ProcessedResult {
        // Format data preview
        let data_preview = self
            .formatter
            .format_preview(results, max_preview_rows);

        // Generate summary
        let summary = self.summarizer.summarize(results);

        // Generate statistics
        let statistics = self.summarizer.generate_statistics(results);

        // Generate visualization hints
        let visualization_hints = self.visualizer.generate_hints(results);

        // Determine if full result is available
        let full_result_available = results.rows.len() <= max_preview_rows;

        ProcessedResult {
            summary,
            data_preview,
            statistics,
            visualization_hints,
            full_result_available,
            continuation_token: if full_result_available {
                None
            } else {
                Some(format!("offset_{}", max_preview_rows))
            },
        }
    }
}

impl Default for ResultProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Processed result for LLM consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedResult {
    /// Human-readable summary
    pub summary: String,
    /// Data preview (first N rows)
    pub data_preview: Vec<Row>,
    /// Statistical summary
    pub statistics: ResultStatistics,
    /// Visualization hints
    pub visualization_hints: Vec<VisualizationHint>,
    /// Whether full result is available in preview
    pub full_result_available: bool,
    /// Continuation token for pagination
    pub continuation_token: Option<String>,
}

/// Query result (simplified representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Column names
    pub columns: Vec<String>,
    /// Data rows
    pub rows: Vec<Row>,
    /// Row count
    pub row_count: usize,
}

/// Row data
pub type Row = HashMap<String, serde_json::Value>;

/// Result statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultStatistics {
    /// Total row count
    pub total_rows: usize,
    /// Column statistics
    pub column_stats: HashMap<String, ColumnStatistics>,
    /// Execution metadata
    pub metadata: ExecutionMetadata,
}

/// Column statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Null count
    pub null_count: usize,
    /// Distinct count (if available)
    pub distinct_count: Option<usize>,
    /// Numeric summary (if numeric column)
    pub numeric_summary: Option<NumericSummary>,
    /// Text summary (if text column)
    pub text_summary: Option<TextSummary>,
}

/// Numeric summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericSummary {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub quartiles: Quartiles,
}

/// Quartiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quartiles {
    pub q1: f64,
    pub q2: f64, // median
    pub q3: f64,
}

/// Text summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSummary {
    pub min_length: usize,
    pub max_length: usize,
    pub avg_length: f64,
    pub sample_values: Vec<String>,
}

/// Execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub execution_time_ms: u64,
    pub query_type: String,
    pub rows_affected: usize,
}

/// Visualization hint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationHint {
    /// Visualization type
    pub viz_type: VisualizationType,
    /// Recommended columns
    pub columns: Vec<String>,
    /// Description
    pub description: String,
}

/// Visualization type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualizationType {
    /// Bar chart
    BarChart,
    /// Line chart
    LineChart,
    /// Scatter plot
    ScatterPlot,
    /// Pie chart
    PieChart,
    /// Histogram
    Histogram,
    /// Table
    Table,
    /// Heatmap
    Heatmap,
}

/// Result Formatter
struct ResultFormatter;

impl ResultFormatter {
    pub fn new() -> Self {
        Self
    }

    /// Format data preview
    pub fn format_preview(&self, results: &QueryResult, max_rows: usize) -> Vec<Row> {
        results
            .rows
            .iter()
            .take(max_rows)
            .cloned()
            .collect()
    }
}

impl Default for ResultFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Data Summarizer
struct DataSummarizer;

impl DataSummarizer {
    pub fn new() -> Self {
        Self
    }

    /// Generate human-readable summary
    pub fn summarize(&self, results: &QueryResult) -> String {
        if results.rows.is_empty() {
            return "Query returned no results.".to_string();
        }

        let mut summary = format!(
            "Query returned {} row{}",
            results.row_count,
            if results.row_count == 1 { "" } else { "s" }
        );

        if !results.columns.is_empty() {
            summary.push_str(&format!(
                " with {} column{}: {}",
                results.columns.len(),
                if results.columns.len() == 1 { "" } else { "s" },
                results.columns.join(", ")
            ));
        }

        summary
    }

    /// Generate statistics
    pub fn generate_statistics(&self, results: &QueryResult) -> ResultStatistics {
        let mut column_stats = HashMap::new();

        // Analyze each column
        for (col_idx, col_name) in results.columns.iter().enumerate() {
            let stats = self.analyze_column(results, col_idx, col_name);
            column_stats.insert(col_name.clone(), stats);
        }

        ResultStatistics {
            total_rows: results.row_count,
            column_stats,
            metadata: ExecutionMetadata {
                execution_time_ms: 0, // Would be populated from actual execution
                query_type: "SELECT".to_string(),
                rows_affected: results.row_count,
            },
        }
    }

    /// Analyze a single column
    fn analyze_column(
        &self,
        results: &QueryResult,
        _col_idx: usize,
        col_name: &str,
    ) -> ColumnStatistics {
        let mut null_count = 0;
        let mut numeric_values = Vec::new();
        let mut text_values = Vec::new();

        for row in &results.rows {
            if let Some(value) = row.get(col_name) {
                if value.is_null() {
                    null_count += 1;
                } else if let Some(num) = value.as_f64() {
                    numeric_values.push(num);
                } else if let Some(str_val) = value.as_str() {
                    text_values.push(str_val.to_string());
                }
            } else {
                null_count += 1;
            }
        }

        let data_type = if !numeric_values.is_empty() {
            "numeric".to_string()
        } else if !text_values.is_empty() {
            "text".to_string()
        } else {
            "unknown".to_string()
        };

        let numeric_summary = if !numeric_values.is_empty() {
            Some(self.summarize_numeric(&numeric_values))
        } else {
            None
        };

        let text_summary = if !text_values.is_empty() {
            Some(self.summarize_text(&text_values))
        } else {
            None
        };

        ColumnStatistics {
            name: col_name.to_string(),
            data_type,
            null_count,
            distinct_count: None, // Would require full scan
            numeric_summary,
            text_summary,
        }
    }

    /// Summarize numeric data
    fn summarize_numeric(&self, values: &[f64]) -> NumericSummary {
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = sorted.len() as f64;
        let sum: f64 = sorted.iter().sum();
        let mean = sum / n;

        let variance = sorted
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / n;
        let std_dev = variance.sqrt();

        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        let q1_idx = sorted.len() / 4;
        let q3_idx = 3 * sorted.len() / 4;
        let q1 = sorted.get(q1_idx).copied().unwrap_or(median);
        let q3 = sorted.get(q3_idx).copied().unwrap_or(median);

        NumericSummary {
            min: sorted.first().copied().unwrap_or(0.0),
            max: sorted.last().copied().unwrap_or(0.0),
            mean,
            median,
            std_dev,
            quartiles: Quartiles { q1, q2: median, q3 },
        }
    }

    /// Summarize text data
    fn summarize_text(&self, values: &[String]) -> TextSummary {
        let lengths: Vec<usize> = values.iter().map(|v| v.len()).collect();
        let total_length: usize = lengths.iter().sum();
        let avg_length = if lengths.is_empty() {
            0.0
        } else {
            total_length as f64 / lengths.len() as f64
        };

        TextSummary {
            min_length: lengths.iter().min().copied().unwrap_or(0),
            max_length: lengths.iter().max().copied().unwrap_or(0),
            avg_length,
            sample_values: values.iter().take(5).cloned().collect(),
        }
    }
}

impl Default for DataSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Visualization Hint Generator
struct VisualizationHintGenerator;

impl VisualizationHintGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate visualization hints based on data
    pub fn generate_hints(&self, results: &QueryResult) -> Vec<VisualizationHint> {
        let mut hints = Vec::new();

        if results.rows.is_empty() {
            return hints;
        }

        // Analyze columns to suggest visualizations
        let numeric_cols: Vec<String> = results
            .columns
            .iter()
            .filter(|col| {
                // Check if column contains numeric data
                results
                    .rows
                    .iter()
                    .any(|row| row.get(*col).and_then(|v| v.as_f64()).is_some())
            })
            .cloned()
            .collect();

        let text_cols: Vec<String> = results
            .columns
            .iter()
            .filter(|col| {
                // Check if column contains text data
                results
                    .rows
                    .iter()
                    .any(|row| row.get(*col).and_then(|v| v.as_str()).is_some())
            })
            .cloned()
            .collect();

        // Suggest bar chart for categorical data
        if !text_cols.is_empty() && !numeric_cols.is_empty() {
            hints.push(VisualizationHint {
                viz_type: VisualizationType::BarChart,
                columns: vec![text_cols[0].clone(), numeric_cols[0].clone()],
                description: format!(
                    "Bar chart showing {} by {}",
                    numeric_cols[0], text_cols[0]
                ),
            });
        }

        // Suggest line chart for time series
        if numeric_cols.len() >= 2 {
            hints.push(VisualizationHint {
                viz_type: VisualizationType::LineChart,
                columns: numeric_cols[0..2.min(numeric_cols.len())].to_vec(),
                description: "Line chart showing trend over time".to_string(),
            });
        }

        // Suggest scatter plot for two numeric columns
        if numeric_cols.len() >= 2 {
            hints.push(VisualizationHint {
                viz_type: VisualizationType::ScatterPlot,
                columns: vec![numeric_cols[0].clone(), numeric_cols[1].clone()],
                description: format!(
                    "Scatter plot showing relationship between {} and {}",
                    numeric_cols[0], numeric_cols[1]
                ),
            });
        }

        // Always suggest table view
        hints.push(VisualizationHint {
            viz_type: VisualizationType::Table,
            columns: results.columns.clone(),
            description: "Table view of all data".to_string(),
        });

        hints
    }
}

impl Default for VisualizationHintGenerator {
    fn default() -> Self {
        Self::new()
    }
}

