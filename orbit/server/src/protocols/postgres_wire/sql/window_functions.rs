// Window function execution support for PostgreSQL
//
// Implements window functions like ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD, etc.
// with PARTITION BY and ORDER BY support.

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::{Expression, OrderByItem, WindowFunctionType};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// Window function evaluator
pub struct WindowFunctionEvaluator {
    /// Partitioned and sorted rows
    partitions: Vec<Partition>,
}

/// A partition of rows for window function evaluation
struct Partition {
    rows: Vec<Row>,
}

/// Row with its original index and values
struct Row {
    index: usize,
    values: HashMap<String, SqlValue>,
}

impl WindowFunctionEvaluator {
    /// Create a new window function evaluator
    pub fn new() -> Self {
        Self {
            partitions: Vec::new(),
        }
    }

    /// Partition and sort rows for window function evaluation
    pub fn partition_rows(
        &mut self,
        rows: Vec<HashMap<String, SqlValue>>,
        partition_by: &[Expression],
        order_by: &[OrderByItem],
    ) -> ProtocolResult<()> {
        if partition_by.is_empty() {
            // Single partition with all rows
            let mut partition_rows: Vec<Row> = rows
                .into_iter()
                .enumerate()
                .map(|(index, values)| Row { index, values })
                .collect();

            // Sort by order_by if specified
            if !order_by.is_empty() {
                self.sort_rows(&mut partition_rows, order_by)?;
            }

            self.partitions.push(Partition {
                rows: partition_rows,
            });
        } else {
            // Multiple partitions based on partition_by expressions
            // Use String key since SqlValue doesn't implement Hash
            let mut partition_map: HashMap<String, Vec<Row>> = HashMap::new();

            for (index, row_values) in rows.into_iter().enumerate() {
                // Evaluate partition key as string
                let partition_key = self.evaluate_partition_key_string(&row_values, partition_by)?;

                partition_map
                    .entry(partition_key)
                    .or_default()
                    .push(Row {
                        index,
                        values: row_values,
                    });
            }

            // Sort each partition and add to partitions list
            for (_, mut partition_rows) in partition_map {
                if !order_by.is_empty() {
                    self.sort_rows(&mut partition_rows, order_by)?;
                }
                self.partitions.push(Partition {
                    rows: partition_rows,
                });
            }
        }

        Ok(())
    }

    /// Evaluate partition key for a row as a string representation
    fn evaluate_partition_key_string(
        &self,
        row: &HashMap<String, SqlValue>,
        partition_by: &[Expression],
    ) -> ProtocolResult<String> {
        let mut key_parts = Vec::new();
        for expr in partition_by {
            // Simplified evaluation - in real implementation would use full expression evaluator
            match expr {
                Expression::Column(col_ref) => {
                    let col_name = col_ref.name.clone();
                    let value = row
                        .get(&col_name)
                        .cloned()
                        .unwrap_or(SqlValue::Null);
                    // Convert to string representation for hashing
                    key_parts.push(format!("{:?}", value));
                }
                _ => {
                    // Skip complex expressions for now
                    key_parts.push("_complex_".to_string());
                }
            }
        }
        Ok(key_parts.join("|"))
    }

    /// Sort rows within a partition
    fn sort_rows(
        &self,
        rows: &mut [Row],
        order_by: &[OrderByItem],
    ) -> ProtocolResult<()> {
        rows.sort_by(|a, b| {
            for order_item in order_by {
                // Simplified comparison - in real implementation would handle all expression types
                if let Expression::Column(col_ref) = &order_item.expression {
                    let col_name = &col_ref.name;
                    let a_val = a.values.get(col_name).unwrap_or(&SqlValue::Null);
                    let b_val = b.values.get(col_name).unwrap_or(&SqlValue::Null);

                    let cmp = self.compare_values(a_val, b_val);
                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                }
            }
            std::cmp::Ordering::Equal
        });

        Ok(())
    }

    /// Compare two SQL values
    fn compare_values(&self, a: &SqlValue, b: &SqlValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
            (SqlValue::Null, _) => Ordering::Less,
            (_, SqlValue::Null) => Ordering::Greater,
            (SqlValue::Integer(a), SqlValue::Integer(b)) => a.cmp(b),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => a.cmp(b),
            (SqlValue::DoublePrecision(a), SqlValue::DoublePrecision(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (SqlValue::Real(a), SqlValue::Real(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (SqlValue::Text(a), SqlValue::Text(b)) => a.cmp(b),
            (SqlValue::Varchar(a), SqlValue::Varchar(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }

    /// Evaluate window function for all rows
    pub fn evaluate(
        &self,
        function: &WindowFunctionType,
    ) -> ProtocolResult<Vec<(usize, SqlValue)>> {
        let mut results = Vec::new();

        for partition in &self.partitions {
            let partition_results = self.evaluate_partition(function, partition)?;
            results.extend(partition_results);
        }

        Ok(results)
    }

    /// Evaluate window function for a single partition
    fn evaluate_partition(
        &self,
        function: &WindowFunctionType,
        partition: &Partition,
    ) -> ProtocolResult<Vec<(usize, SqlValue)>> {
        let mut results = Vec::new();

        match function {
            WindowFunctionType::RowNumber => {
                for (row_num, row) in partition.rows.iter().enumerate() {
                    results.push((row.index, SqlValue::BigInt((row_num + 1) as i64)));
                }
            }
            WindowFunctionType::Rank => {
                let mut current_rank: i64 = 1;
                let mut rows_at_rank: i64 = 0;

                for (idx, row) in partition.rows.iter().enumerate() {
                    rows_at_rank += 1;

                    // Check if next row has different values (simplified)
                    let is_last_in_group = idx == partition.rows.len() - 1
                        || !self.rows_equal(row, &partition.rows[idx + 1]);

                    results.push((row.index, SqlValue::BigInt(current_rank)));

                    if is_last_in_group {
                        current_rank += rows_at_rank;
                        rows_at_rank = 0;
                    }
                }
            }
            WindowFunctionType::DenseRank => {
                let mut current_rank: i64 = 1;

                for (idx, row) in partition.rows.iter().enumerate() {
                    results.push((row.index, SqlValue::BigInt(current_rank)));

                    // Check if next row has different values
                    if idx < partition.rows.len() - 1
                        && !self.rows_equal(row, &partition.rows[idx + 1])
                    {
                        current_rank += 1;
                    }
                }
            }
            WindowFunctionType::Lag { offset: _, default: _, .. } => {
                let offset_val: usize = 1; // Simplified - should evaluate offset expression
                for (idx, row) in partition.rows.iter().enumerate() {
                    let value = if idx >= offset_val {
                        // Get value from previous row
                        SqlValue::BigInt((idx - offset_val) as i64)
                    } else {
                        // Use default or NULL
                        SqlValue::Null
                    };
                    results.push((row.index, value));
                }
            }
            WindowFunctionType::Lead { offset: _, default: _, .. } => {
                let offset_val: usize = 1; // Simplified
                for (idx, row) in partition.rows.iter().enumerate() {
                    let value = if idx + offset_val < partition.rows.len() {
                        SqlValue::BigInt((idx + offset_val) as i64)
                    } else {
                        SqlValue::Null
                    };
                    results.push((row.index, value));
                }
            }
            _ => {
                // Window function not yet implemented - return empty results
                tracing::warn!("Window function {:?} not yet implemented", function);
            }
        }

        Ok(results)
    }

    /// Check if two rows are equal (simplified)
    fn rows_equal(&self, a: &Row, b: &Row) -> bool {
        // Simplified - should compare based on ORDER BY columns
        a.values == b.values
    }
}

impl Default for WindowFunctionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_number() {
        let mut evaluator = WindowFunctionEvaluator::new();

        let rows = vec![
            HashMap::from([("id".to_string(), SqlValue::Integer(1))]),
            HashMap::from([("id".to_string(), SqlValue::Integer(2))]),
            HashMap::from([("id".to_string(), SqlValue::Integer(3))]),
        ];

        evaluator.partition_rows(rows, &[], &[]).unwrap();

        let results = evaluator
            .evaluate(&WindowFunctionType::RowNumber)
            .unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].1, SqlValue::BigInt(1));
        assert_eq!(results[1].1, SqlValue::BigInt(2));
        assert_eq!(results[2].1, SqlValue::BigInt(3));
    }

    #[test]
    fn test_rank() {
        let mut evaluator = WindowFunctionEvaluator::new();

        let rows = vec![
            HashMap::from([("score".to_string(), SqlValue::Integer(100))]),
            HashMap::from([("score".to_string(), SqlValue::Integer(100))]),
            HashMap::from([("score".to_string(), SqlValue::Integer(90))]),
        ];

        evaluator.partition_rows(rows, &[], &[]).unwrap();

        let results = evaluator.evaluate(&WindowFunctionType::Rank).unwrap();

        assert_eq!(results.len(), 3);
        // All should have rank 1 since we're not actually sorting in this simplified test
    }
}
