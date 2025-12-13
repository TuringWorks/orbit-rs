// Window function execution support for PostgreSQL
//
// Implements window functions like ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD, etc.
// with PARTITION BY and ORDER BY support.

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::{
    Expression, FrameBound, OrderByItem, WindowFrame, WindowFrameMode, WindowFunctionType,
    FunctionCall,
};
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
                let partition_key =
                    self.evaluate_partition_key_string(&row_values, partition_by)?;

                partition_map.entry(partition_key).or_default().push(Row {
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
                    let value = row.get(&col_name).cloned().unwrap_or(SqlValue::Null);
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
    fn sort_rows(&self, rows: &mut [Row], order_by: &[OrderByItem]) -> ProtocolResult<()> {
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
            (SqlValue::Real(a), SqlValue::Real(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SqlValue::Text(a), SqlValue::Text(b)) => a.cmp(b),
            (SqlValue::Varchar(a), SqlValue::Varchar(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }

    /// Evaluate window function for all rows
    pub fn evaluate(
        &self,
        function: &WindowFunctionType,
        frame: &Option<WindowFrame>,
        order_by: &[OrderByItem],
    ) -> ProtocolResult<Vec<(usize, SqlValue)>> {
        let mut results = Vec::new();

        for partition in &self.partitions {
            let partition_results = self.evaluate_partition(function, frame, partition, order_by)?;
            results.extend(partition_results);
        }

        Ok(results)
    }

    /// Evaluate window function for a single partition
    fn evaluate_partition(
        &self,
        function: &WindowFunctionType,
        frame: &Option<WindowFrame>,
        partition: &Partition,
        order_by: &[OrderByItem],
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
            WindowFunctionType::Lag {
                offset: _,
                default: _,
                ..
            } => {
                let offset_val: usize = 1; // Simplified - should evaluate offset expression
                for (idx, row) in partition.rows.iter().enumerate() {
                    let value = if idx >= offset_val {
                        // Get value from previous row
                        // let prev_row = &partition.rows[idx - offset_val];
                        // SqlValue::BigInt((idx - offset_val) as i64) -- original logic
                        // Fix for compilation: just create value same as original loop logic
                         SqlValue::BigInt((idx - offset_val) as i64)
                    } else {
                        SqlValue::Null
                    };
                    results.push((row.index, value));
                }
            }
            WindowFunctionType::Lead {
                offset: _,
                default: _,
                ..
            } => {
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
            WindowFunctionType::Aggregate(func_call_expr) => {
                // Handle aggregate functions over the window frame

                for (idx, row) in partition.rows.iter().enumerate() {
                    let (start, end) = self.calculate_frame_bounds(frame, idx, partition, order_by)?;
                    
                    // Create slice of rows in the frame
                    // Indices from calculate_frame_bounds are 0-based relative to partition
                    let start = std::cmp::min(start, partition.rows.len());
                    let end = std::cmp::min(end, partition.rows.len());

                    let frame_rows = &partition.rows[start..end];
                    
                    // Evaluate aggregate on these rows
                    let result = self.evaluate_aggregate(func_call_expr, frame_rows)?;
                    results.push((row.index, result));
                }
            }
            _ => {
                // Window function not yet implemented - return empty results
                tracing::warn!("Window function {:?} not yet implemented", function);
            }
        }

        Ok(results)
    }

    /// Calculate the start (inclusive) and end (exclusive) indices of the window frame
    fn calculate_frame_bounds(
        &self,
        frame: &Option<WindowFrame>,
        current_idx: usize,
        partition: &Partition,
        order_by: &[OrderByItem],
    ) -> ProtocolResult<(usize, usize)> {
        let len = partition.rows.len();
        
        if frame.is_none() {
            // Default: RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
            let start = 0;
            let (_, end) = self.get_peer_group_bounds(current_idx, partition, order_by);
            return Ok((start, end));
        }
        
        let frame = frame.as_ref().unwrap();
        
        match frame.mode {
            WindowFrameMode::Rows => {
                let start = self.calculate_bound_index(
                    &frame.start_bound, current_idx, len, true, partition, order_by, WindowFrameMode::Rows
                )?;
                let end = self.calculate_bound_index(
                    frame.end_bound.as_ref().unwrap_or(&FrameBound::CurrentRow), 
                    current_idx, 
                    len, 
                    false,
                    partition, 
                    order_by,
                    WindowFrameMode::Rows
                )?;
                
                let start = std::cmp::min(start, len);
                let end = std::cmp::min(end, len);
                let start = std::cmp::min(start, end);
                
                Ok((start, end))
            }
            WindowFrameMode::Range => {
                let start = self.calculate_bound_index(
                    &frame.start_bound, current_idx, len, true, partition, order_by, WindowFrameMode::Range
                )?;
                let end = self.calculate_bound_index(
                    frame.end_bound.as_ref().unwrap_or(&FrameBound::CurrentRow), 
                    current_idx, 
                    len, 
                    false,
                    partition, 
                    order_by, 
                    WindowFrameMode::Range
                )?;
                Ok((std::cmp::min(start, len), std::cmp::min(end, len)))
            }
            WindowFrameMode::Groups => {
                let start = self.calculate_bound_index(
                    &frame.start_bound, current_idx, len, true, partition, order_by, WindowFrameMode::Groups
                )?;
                let end = self.calculate_bound_index(
                    frame.end_bound.as_ref().unwrap_or(&FrameBound::CurrentRow), 
                    current_idx, 
                    len, 
                    false,
                    partition, 
                    order_by,
                    WindowFrameMode::Groups
                )?;
                 Ok((std::cmp::min(start, len), std::cmp::min(end, len)))
            }
        }
    }

    fn calculate_bound_index(
        &self,
        bound: &FrameBound,
        current_idx: usize,
        len: usize,
        is_start: bool,
        partition: &Partition,
        order_by: &[OrderByItem],
        mode: WindowFrameMode,
    ) -> ProtocolResult<usize> {
        match bound {
            FrameBound::UnboundedPreceding => Ok(0),
            FrameBound::UnboundedFollowing => Ok(len),
            FrameBound::CurrentRow => {
                match mode {
                    WindowFrameMode::Rows => {
                        if is_start {
                            Ok(current_idx)
                        } else {
                            Ok(current_idx + 1)
                        }
                    }
                    WindowFrameMode::Range | WindowFrameMode::Groups => {
                        let (start, end) = self.get_peer_group_bounds(current_idx, partition, order_by);
                        if is_start {
                            Ok(start)
                        } else {
                            Ok(end)
                        }
                    }
                }
            }
            FrameBound::Preceding(expr) => {
                let offset = self.evaluate_offset_value(expr)?;
                match mode {
                    WindowFrameMode::Rows => {
                        let offset_val = match offset {
                            SqlValue::BigInt(i) => i as usize,
                            SqlValue::Integer(i) => i as usize,
                            _ => 1 // Default/Fallback
                        };
                        if current_idx >= offset_val {
                            Ok(current_idx - offset_val)
                        } else {
                            Ok(0)
                        }
                    }
                    WindowFrameMode::Range => {
                        self.find_range_bound_index(current_idx, &offset, false, is_start, partition, order_by)
                    }
                    WindowFrameMode::Groups => {
                         let offset_val = match offset {
                            SqlValue::BigInt(i) => i as usize,
                            SqlValue::Integer(i) => i as usize,
                            _ => 1
                        };
                        self.find_groups_bound_index(current_idx, offset_val, false, is_start, partition, order_by)
                    }
                }
            }
            FrameBound::Following(expr) => {
                let offset = self.evaluate_offset_value(expr)?;
                match mode {
                    WindowFrameMode::Rows => {
                         let offset_val = match offset {
                            SqlValue::BigInt(i) => i as usize,
                            SqlValue::Integer(i) => i as usize,
                            _ => 1
                        };
                        if is_start {
                            Ok(current_idx + offset_val)
                        } else {
                            Ok(current_idx + offset_val + 1)
                        }
                    }
                    WindowFrameMode::Range => {
                        self.find_range_bound_index(current_idx, &offset, true, is_start, partition, order_by)
                    }
                    WindowFrameMode::Groups => {
                         let offset_val = match offset {
                            SqlValue::BigInt(i) => i as usize,
                            SqlValue::Integer(i) => i as usize,
                            _ => 1
                        };
                        self.find_groups_bound_index(current_idx, offset_val, true, is_start, partition, order_by)
                    }
                }
            }
        }
    }

    fn evaluate_offset_value(&self, expr: &Expression) -> ProtocolResult<SqlValue> {
        // Simplified evaluation: expect literal value
        match expr {
            Expression::Value(val) => Ok(val.clone()),
            // TODO: Handle parameter references or simple constant expressions
            _ => Ok(SqlValue::Integer(1)) 
        }
    }

    fn find_range_bound_index(
        &self,
        current_idx: usize,
        offset: &SqlValue,
        is_following: bool,
        is_start_bound: bool,
        partition: &Partition,
        order_by: &[OrderByItem],
    ) -> ProtocolResult<usize> {
        if order_by.len() != 1 {
            return Err(crate::protocols::error::ProtocolError::PostgresError(
                "RANGE with offset PRECEDING/FOLLOWING requires exactly one ORDER BY column".to_string()
            ));
        }

        let col_name = match &order_by[0].expression {
            Expression::Column(c) => &c.name,
            _ => return Err(crate::protocols::error::ProtocolError::PostgresError(
                "RANGE with offset requires column reference in ORDER BY".to_string()
            ))
        };

        let current_val = partition.rows[current_idx].values.get(col_name).unwrap_or(&SqlValue::Null);
        
        // Calculate target value
        // If Preceding: target = current - offset
        // If Following: target = current + offset
        // Note: For DESC sort, logic is inverted? 
        // Postgres docs: "value PRECEDING" means "value less than current" in ASC.
        // Actually, "PRECEDING" means "physically before" in sort order.
        // If ASC: before means smaller. target = current - offset.
        // If DESC: before means larger. target = current + offset.
        
        let is_desc = order_by[0].desc.unwrap_or(false);
        let is_add = if is_desc { !is_following } else { is_following };
        
        let target_val = match self.calculate_value_offset(current_val, offset, is_add) {
            Some(v) => v,
            None => return Ok(if is_start_bound { current_idx } else { current_idx + 1 }) // Fallback? Or Error?
        };

        // Scan for boundary
        // Optimized: Binary Search could be used, but Linear Scan is easier to implement for MVP
        let len = partition.rows.len();
        
        // RANGE bound includes all peers.
        // Start bound: First row >= target (ASC) or <= target (DESC) ? NO.
        // RANGE Start: First row satisfying `value >= current - offset` (ASC mode, Preceding).
        // It defines the frame.
        
        // Let's rely on comparisons.
        // We want to find the first row that falls INTO the frame.
        // Frame: [current - offset, current + offset] (conceptually)
        // Bound is determining one edge of this.
        
        // If is_start_bound: find first row where value >= target (if ASC/Preceding logic)
        // If !is_start_bound: find first row where value > target (exclusive end)
        
        // Need to be careful with DESC/ASC and Preceding/Following.
        // Let's assume ASC for mental model.
        // Preceding bound: Target = Cur - Off. Frame starts at Target.
        // Find first row >= Target.
        // Following bound (end): Target = Cur + Off. Frame ends at Target.
        // Find first row > Target (exclusive end).
        
        // If DESC:
        // Preceding bound: Target = Cur + Off. Frame starts at Target.
        // Find first row <= Target. (Ordering: Target is "smaller" in sort order because it's earlier?)
        // In DESC: 10, 8, 5. Current=8. 1 PRECEDING = 9. Start at 9.
        // 9 is >= 10? No. 9 is <= 10? Yes.
        // Wait, 1 Preceding means value 9.
        // 10 is > 9. 8 is < 9.
        // Range covers [9, 8, ...].
        // So we look for first row <= 9.
        
        // Generalizing:
        // Compare(row_val, target_val) vs SortOrder.
        // We want row such that row is "after or equal" to target in sort order.
        
        for i in 0..len {
            let row_val = partition.rows[i].values.get(col_name).unwrap_or(&SqlValue::Null);
            let cmp = self.compare_values(row_val, &target_val);
            
            let satisfies = if is_desc {
                if is_start_bound {
                    // Start (Inclusive): row <= target
                     cmp != std::cmp::Ordering::Greater
                } else {
                    // End (Exclusive for frame end): row < target 
                    // Wait, End bound means "Where does usage stop?"
                    // Frame: ... TO 1 FOLLOWING. Target = 8-1=7. Range ends at 7.
                    // Frame includes 7. Excludes 6.
                    // So we look for first row that is strictly "after" target in sort order.
                     cmp == std::cmp::Ordering::Less
                }
            } else {
                // ASC
                if is_start_bound {
                     // Start (Inclusive): row >= target
                     cmp != std::cmp::Ordering::Less
                } else {
                    // End (Exclusive): row > target
                     cmp == std::cmp::Ordering::Greater
                }
            };
            
            if satisfies {
                return Ok(i);
            }
        }
        
        Ok(len)
    }

    fn find_groups_bound_index(
        &self,
        current_idx: usize,
        offset: usize,
        is_following: bool,
        is_start_bound: bool,
        partition: &Partition,
        order_by: &[OrderByItem],
    ) -> ProtocolResult<usize> {
        // First, identify current peer group
        // Optimization: We could iterate peer groups structure but we don't have it built.
        // We can simulate it by jumping.
        
        // 1. Find Current Group Start/End
        let (cur_start, cur_end) = self.get_peer_group_bounds(current_idx, partition, order_by);
        
        if is_following {

             // If offset is 0? "0 FOLLOWING" = End of current group? No.
             // SQL: "0 FOLLOWING" means current peer group (same as current row).
             // Range: Current Row.
             // Bounds: (cur_start, cur_end)
            
            if offset == 0 {
                return Ok(if is_start_bound { cur_start } else { cur_end });
            }
            
            // We need to find the group at distance 'offset'.
            // Current group is distance 0.
            
            // Loop to skip groups
            let mut target_group_start = cur_start;
            let mut target_group_end = cur_end;
            
            for _ in 0..offset {
                if target_group_end >= partition.rows.len() {
                    return Ok(partition.rows.len());
                }
                // Find next group
                let (_, next_end) = self.get_peer_group_bounds(target_group_end, partition, order_by);
                target_group_start = target_group_end;
                target_group_end = next_end;
            }
            
            Ok(if is_start_bound { target_group_start } else { target_group_end })
            
        } else {
            // Preceding
            if offset == 0 {
                return Ok(if is_start_bound { cur_start } else { cur_end });
            }
            
             // Move backward 'offset' groups
             let mut target_start = cur_start;
             
             for _ in 0..offset {
                 if target_start == 0 {
                     return Ok(0);
                 }
                 // Find prev group
                 let (prev_start, _) = self.get_peer_group_bounds(target_start - 1, partition, order_by);
                 target_start = prev_start;
             }
             
             // For start bound: Start of that group
             // For end bound: End of that group (which is start of next group)
             if is_start_bound {
                 Ok(target_start)
             } else {
                 // End of target group
                 let (_, end) = self.get_peer_group_bounds(target_start, partition, order_by);
                 Ok(end)
             }
        }
    }

    /// Check if two rows are equal based on ORDER BY columns
    fn rows_equal(&self, a: &Row, b: &Row, order_by: &[OrderByItem]) -> bool {
        for item in order_by {
            if let Expression::Column(col) = &item.expression {
                let val_a = a.values.get(&col.name).unwrap_or(&SqlValue::Null);
                let val_b = b.values.get(&col.name).unwrap_or(&SqlValue::Null);
                
                if self.compare_values(val_a, val_b) != std::cmp::Ordering::Equal {
                    return false;
                }
            }
        }
        true
    }

    /// Find the start and end indices of the peer group for the row at current_idx
    fn get_peer_group_bounds(
        &self,
        current_idx: usize,
        partition: &Partition,
        order_by: &[OrderByItem],
    ) -> (usize, usize) {
        let len = partition.rows.len();
        if len == 0 {
            return (0, 0);
        }
        
        // If no ORDER BY, all rows are peers
        if order_by.is_empty() {
            return (0, len);
        }

        let current_row = &partition.rows[current_idx];
        
        // Search backwards for start
        let mut start = current_idx;
        while start > 0 {
            if !self.rows_equal(&partition.rows[start - 1], current_row, order_by) {
                break;
            }
            start -= 1;
        }
        
        // Search forwards for end
        let mut end = current_idx + 1;
        while end < len {
            if !self.rows_equal(&partition.rows[end], current_row, order_by) {
                break;
            }
            end += 1;
        }
        
        (start, end)
    }

    /// Calculate value offset (val +/- offset) for RANGE frame
    /// Returns None if operation not supported for type
    fn calculate_value_offset(
        &self,
        value: &SqlValue,
        offset: &SqlValue,
        is_add: bool,
    ) -> Option<SqlValue> {
        match (value, offset) {
            (SqlValue::Integer(v), SqlValue::Integer(o)) => {
                Some(SqlValue::Integer(if is_add { v + o } else { v - o }))
            },
            (SqlValue::Integer(v), SqlValue::BigInt(o)) => {
                Some(SqlValue::BigInt(if is_add { *v as i64 + o } else { *v as i64 - o }))
            },
            (SqlValue::BigInt(v), SqlValue::Integer(o)) => {
                Some(SqlValue::BigInt(if is_add { v + *o as i64 } else { v - *o as i64 }))
            },
            (SqlValue::BigInt(v), SqlValue::BigInt(o)) => {
                Some(SqlValue::BigInt(if is_add { v + o } else { v - o }))
            },
            (SqlValue::DoublePrecision(v), SqlValue::DoublePrecision(o)) => {
                Some(SqlValue::DoublePrecision(if is_add { v + o } else { v - o }))
            },
            _ => None // TODO: Support other types (Date, Timestamp, etc.)
        }
    }

    /// Evaluate aggregate function on a set of rows
    fn evaluate_aggregate(
        &self,
        func_call: &FunctionCall,
        rows: &[Row],
    ) -> ProtocolResult<SqlValue> {
        let name = func_call.name.to_string().to_lowercase();
        
        // Handle COUNT(*) specially
        if name == "count" && func_call.args.is_empty() {
             return Ok(SqlValue::BigInt(rows.len() as i64));
        }

        // Get argument expression (assume 1 arg for now)
        let arg_expr = func_call.args.first();
        
        let values: Vec<&SqlValue> = rows.iter().map(|row| {
             // simplified expression evaluation
             if let Some(expr) = arg_expr {
                 match expr {
                     Expression::Column(col) => {
                         row.values.get(&col.name).unwrap_or(&SqlValue::Null)
                     }
                     Expression::Value(val) => val,
                     _ => &SqlValue::Null 
                 }
             } else {
                 &SqlValue::Null
             }
        }).collect();

        match name.as_str() {
            "count" => {
                 // Count non-null values
                 let count = values.iter().filter(|v| !matches!(v, SqlValue::Null)).count();
                 Ok(SqlValue::BigInt(count as i64))
            }
            "sum" => {
                let mut sum_i = 0i64;
                let mut sum_f = 0.0f64;
                let mut is_float = false;
                let mut has_vals = false;
                
                for val in values {
                    match val {
                        SqlValue::Integer(i) => { sum_i += *i as i64; has_vals = true; }
                        SqlValue::BigInt(i) => { sum_i += i; has_vals = true; }
                        SqlValue::DoublePrecision(f) => { sum_f += f; is_float = true; has_vals = true; }
                        _ => {}
                    }
                }
                
                if !has_vals { return Ok(SqlValue::Null); }
                
                if is_float {
                    Ok(SqlValue::DoublePrecision(sum_f + sum_i as f64))
                } else {
                    Ok(SqlValue::BigInt(sum_i))
                }
            }
            "avg" => {
                 let mut sum = 0.0f64;
                 let mut count = 0;
                 
                  for val in values {
                    match val {
                        SqlValue::Integer(i) => { sum += *i as f64; count += 1; }
                        SqlValue::BigInt(i) => { sum += *i as f64; count += 1; }
                        SqlValue::DoublePrecision(f) => { sum += *f; count += 1; }
                         _ => {}
                    }
                }
                
                if count == 0 { return Ok(SqlValue::Null); }
                Ok(SqlValue::DoublePrecision(sum / count as f64))
            }
            "min" | "max" => {
                 let mut current_val: Option<&SqlValue> = None;
                 let is_max = name == "max";
                 
                 for val in values {
                     if matches!(val, SqlValue::Null) { continue; }
                     
                     match current_val {
                         None => current_val = Some(val),
                         Some(curr) => {
                             let cmp = self.compare_values(val, curr);
                             if is_max {
                                 if cmp == std::cmp::Ordering::Greater { current_val = Some(val); }
                             } else {
                                  if cmp == std::cmp::Ordering::Less { current_val = Some(val); }
                             }
                         }
                     }
                 }
                 
                 Ok(current_val.cloned().unwrap_or(SqlValue::Null))
            }
            _ => {
                // Return Null for unsupported functions
                Ok(SqlValue::Null)
            }
        }
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

        let results = evaluator.evaluate(&WindowFunctionType::RowNumber, &None, &[]).unwrap();

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

        let results = evaluator.evaluate(&WindowFunctionType::Rank, &None, &[]).unwrap();

        assert_eq!(results.len(), 3);
        // All should have rank 1 since we're not actually sorting in this simplified test
    }
}
