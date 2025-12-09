//! Advanced SQL Expression Evaluator
//!
//! This module provides comprehensive evaluation of SQL expressions including:
//! - Basic literals and column references
//! - Binary and unary operations with proper precedence
//! - Function calls and aggregates
//! - Window functions (ROW_NUMBER, RANK, LAG, LEAD, etc.)
//! - CASE expressions and conditional logic
//! - Subqueries and EXISTS/IN operations
//! - Vector similarity operations
//! - Type casting and conversions
//! - Sequence functions (nextval, currval, setval, lastval)

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::{
    ast::{
        BinaryOperator, CaseExpression, ColumnRef, Expression, FunctionCall, FunctionName, InList,
        OrderByItem, SelectStatement, UnaryOperator, VectorOperator, WindowFrame,
        WindowFunctionType,
    },
    types::{SqlType, SqlValue},
};
use chrono::Datelike;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Sequence accessor trait for sequence function evaluation
/// This allows the expression evaluator to access and modify sequences
/// without directly depending on the executor's implementation.
pub trait SequenceAccessor: Send + Sync {
    /// Get the next value from a sequence
    fn nextval(&self, sequence_name: &str) -> ProtocolResult<i64>;
    /// Get the current value of a sequence (must have been called with nextval first)
    fn currval(&self, sequence_name: &str) -> ProtocolResult<i64>;
    /// Set the value of a sequence
    fn setval(&self, sequence_name: &str, value: i64, is_called: bool) -> ProtocolResult<i64>;
    /// Get the last value returned by nextval in this session
    fn lastval(&self) -> ProtocolResult<i64>;
}

/// A simple sequence accessor implementation that wraps sequence metadata storage
pub struct SimpleSequenceAccessor {
    sequences: Arc<RwLock<HashMap<String, SequenceMetadataRef>>>,
    last_value: Arc<RwLock<Option<(String, i64)>>>,
}

/// Reference to sequence metadata for the accessor
#[derive(Debug, Clone)]
pub struct SequenceMetadataRef {
    pub name: String,
    pub current_value: i64,
    pub increment: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub cycle: bool,
    pub is_called: bool,
}

impl SimpleSequenceAccessor {
    pub fn new(
        sequences: Arc<RwLock<HashMap<String, SequenceMetadataRef>>>,
        last_value: Arc<RwLock<Option<(String, i64)>>>,
    ) -> Self {
        Self {
            sequences,
            last_value,
        }
    }
}

impl SequenceAccessor for SimpleSequenceAccessor {
    fn nextval(&self, sequence_name: &str) -> ProtocolResult<i64> {
        let mut sequences = self.sequences.write().map_err(|_| {
            ProtocolError::PostgresError("Failed to acquire sequence lock".to_string())
        })?;

        let seq = sequences
            .get_mut(sequence_name)
            .ok_or_else(|| ProtocolError::not_found("Sequence", sequence_name))?;

        let next_value = if seq.is_called {
            let next = seq.current_value + seq.increment;
            if seq.increment > 0 && next > seq.max_value {
                if seq.cycle {
                    seq.min_value
                } else {
                    return Err(ProtocolError::PostgresError(format!(
                        "nextval: reached maximum value of sequence \"{}\" ({})",
                        sequence_name, seq.max_value
                    )));
                }
            } else if seq.increment < 0 && next < seq.min_value {
                if seq.cycle {
                    seq.max_value
                } else {
                    return Err(ProtocolError::PostgresError(format!(
                        "nextval: reached minimum value of sequence \"{}\" ({})",
                        sequence_name, seq.min_value
                    )));
                }
            } else {
                next
            }
        } else {
            seq.is_called = true;
            seq.current_value
        };

        seq.current_value = next_value;

        // Update last_value for lastval()
        if let Ok(mut last) = self.last_value.write() {
            *last = Some((sequence_name.to_string(), next_value));
        }

        Ok(next_value)
    }

    fn currval(&self, sequence_name: &str) -> ProtocolResult<i64> {
        let sequences = self.sequences.read().map_err(|_| {
            ProtocolError::PostgresError("Failed to acquire sequence lock".to_string())
        })?;

        let seq = sequences
            .get(sequence_name)
            .ok_or_else(|| ProtocolError::not_found("Sequence", sequence_name))?;

        if !seq.is_called {
            return Err(ProtocolError::PostgresError(format!(
                "currval of sequence \"{}\" is not yet defined in this session",
                sequence_name
            )));
        }

        Ok(seq.current_value)
    }

    fn setval(&self, sequence_name: &str, value: i64, is_called: bool) -> ProtocolResult<i64> {
        let mut sequences = self.sequences.write().map_err(|_| {
            ProtocolError::PostgresError("Failed to acquire sequence lock".to_string())
        })?;

        let seq = sequences
            .get_mut(sequence_name)
            .ok_or_else(|| ProtocolError::not_found("Sequence", sequence_name))?;

        if value < seq.min_value || value > seq.max_value {
            return Err(ProtocolError::PostgresError(format!(
                "setval: value {} is out of bounds for sequence \"{}\" ({} to {})",
                value, sequence_name, seq.min_value, seq.max_value
            )));
        }

        seq.current_value = value;
        seq.is_called = is_called;

        // Update last_value for lastval()
        if is_called {
            if let Ok(mut last) = self.last_value.write() {
                *last = Some((sequence_name.to_string(), value));
            }
        }

        Ok(value)
    }

    fn lastval(&self) -> ProtocolResult<i64> {
        let last = self.last_value.read().map_err(|_| {
            ProtocolError::PostgresError("Failed to acquire last value lock".to_string())
        })?;

        match &*last {
            Some((_, value)) => Ok(*value),
            None => Err(ProtocolError::PostgresError(
                "lastval is not yet defined in this session".to_string(),
            )),
        }
    }
}

/// Expression evaluation context
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    pub current_row: HashMap<String, SqlValue>,
    pub table_data: HashMap<String, Vec<HashMap<String, SqlValue>>>,
    pub variables: HashMap<String, SqlValue>,
    pub current_table: Option<String>,
    pub window_frame: Option<WindowFrameContext>,
}

impl EvaluationContext {
    /// Create a new empty evaluation context
    pub fn empty() -> Self {
        Self {
            current_row: HashMap::new(),
            table_data: HashMap::new(),
            variables: HashMap::new(),
            current_table: None,
            window_frame: None,
        }
    }

    /// Create context with a specific row
    pub fn with_row(current_row: HashMap<String, SqlValue>) -> Self {
        Self {
            current_row,
            table_data: HashMap::new(),
            variables: HashMap::new(),
            current_table: None,
            window_frame: None,
        }
    }

    /// Create context with row and table name
    pub fn with_row_and_table(current_row: HashMap<String, SqlValue>, table_name: String) -> Self {
        Self {
            current_row,
            table_data: HashMap::new(),
            variables: HashMap::new(),
            current_table: Some(table_name),
            window_frame: None,
        }
    }
}

/// Window function evaluation context
#[derive(Debug, Clone)]
pub struct WindowFrameContext {
    pub all_rows: Vec<HashMap<String, SqlValue>>,
    pub current_row_index: usize,
    pub partition_rows: Vec<usize>, // Row indices in current partition
    pub ordered_rows: Vec<usize>,   // Row indices in current order
    /// ORDER BY values for each row in the partition (for RANGE mode)
    pub order_by_values: Vec<SqlValue>,
    /// Peer group boundaries - each entry is (start_idx, end_idx) in partition_rows
    pub peer_groups: Vec<(usize, usize)>,
}

/// Aggregate function state
#[derive(Debug, Clone)]
pub enum AggregateState {
    Count(i64),
    Sum(SqlValue),
    Min(SqlValue),
    Max(SqlValue),
    Avg {
        sum: SqlValue,
        count: i64,
    },
    // New aggregate function states
    ArrayAgg(Vec<SqlValue>),
    StringAgg {
        values: Vec<String>,
        delimiter: String,
    },
    BoolAnd(Option<bool>),
    BoolOr(Option<bool>),
}

/// Expression evaluator
pub struct ExpressionEvaluator {
    #[allow(dead_code)]
    aggregates: HashMap<String, AggregateState>,
    /// Optional sequence accessor for nextval/currval/setval/lastval functions
    sequence_accessor: Option<Arc<dyn SequenceAccessor>>,
}

impl ExpressionEvaluator {
    pub fn new() -> Self {
        Self {
            aggregates: HashMap::new(),
            sequence_accessor: None,
        }
    }

    /// Create an expression evaluator with a sequence accessor
    pub fn with_sequence_accessor(sequence_accessor: Arc<dyn SequenceAccessor>) -> Self {
        Self {
            aggregates: HashMap::new(),
            sequence_accessor: Some(sequence_accessor),
        }
    }

    /// Set the sequence accessor
    pub fn set_sequence_accessor(&mut self, accessor: Arc<dyn SequenceAccessor>) {
        self.sequence_accessor = Some(accessor);
    }

    /// Evaluate an SQL expression
    pub fn evaluate(
        &mut self,
        expr: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        match expr {
            Expression::Literal(value) => Ok(value.clone()),

            Expression::Column(column_ref) => self.evaluate_column(column_ref, context),

            Expression::Parameter(param_num) => {
                // TODO: Handle prepared statement parameters
                Ok(SqlValue::Text(format!("${param_num}")))
            }

            Expression::Binary {
                left,
                operator,
                right,
            } => self.evaluate_binary_op(left, operator, right, context),

            Expression::Unary { operator, operand } => {
                self.evaluate_unary_op(operator, operand, context)
            }

            Expression::Function(func_call) => self.evaluate_function_call(func_call, context),

            Expression::WindowFunction {
                function,
                partition_by,
                order_by,
                frame,
            } => self.evaluate_window_function(function, partition_by, order_by, frame, context),

            Expression::Case(case_expr) => self.evaluate_case_expression(case_expr, context),

            Expression::Subquery(select_stmt) => self.evaluate_subquery(select_stmt, context),

            Expression::Exists(select_stmt) => self.evaluate_exists(select_stmt, context),

            Expression::In {
                expr,
                list,
                negated,
            } => self.evaluate_in_expression(expr, list, *negated, context),

            Expression::Between {
                expr,
                low,
                high,
                negated,
            } => self.evaluate_between_expression(expr, low, high, *negated, context),

            Expression::Like {
                expr,
                pattern,
                escape,
                case_insensitive,
                negated,
            } => self.evaluate_like_expression(
                expr,
                pattern,
                escape.as_deref(),
                *case_insensitive,
                *negated,
                context,
            ),

            Expression::IsNull { expr, negated } => self.evaluate_is_null(expr, *negated, context),

            Expression::Cast { expr, target_type } => {
                self.evaluate_cast(expr, target_type, context)
            }

            Expression::Array(elements) => self.evaluate_array_constructor(elements, context),

            Expression::Row(elements) => self.evaluate_row_constructor(elements, context),

            Expression::ArrayIndex { array, index } => {
                self.evaluate_array_index(array, index, context)
            }

            Expression::ArraySlice { array, start, end } => {
                self.evaluate_array_slice(array, start.as_deref(), end.as_deref(), context)
            }

            Expression::VectorSimilarity {
                left,
                operator,
                right,
            } => self.evaluate_vector_similarity(left, operator, right, context),
        }
    }

    fn evaluate_column(
        &self,
        column_ref: &ColumnRef,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let column_name = match &column_ref.table {
            Some(table) => format!("{}.{}", table, column_ref.name),
            None => column_ref.name.clone(),
        };

        // First try exact column name
        if let Some(value) = context.current_row.get(&column_name) {
            return Ok(value.clone());
        }

        // Try just the column name without table prefix
        if let Some(value) = context.current_row.get(&column_ref.name) {
            return Ok(value.clone());
        }

        // Try variables
        if let Some(value) = context.variables.get(&column_ref.name) {
            return Ok(value.clone());
        }

        Err(ProtocolError::PostgresError(format!(
            "Column '{column_name}' not found"
        )))
    }

    fn evaluate_binary_op(
        &mut self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let left_val = self.evaluate(left, context)?;
        let right_val = self.evaluate(right, context)?;

        match operator {
            BinaryOperator::Plus => self.arithmetic_op(&left_val, &right_val, "+"),
            BinaryOperator::Minus => self.arithmetic_op(&left_val, &right_val, "-"),
            BinaryOperator::Multiply => self.arithmetic_op(&left_val, &right_val, "*"),
            BinaryOperator::Divide => self.arithmetic_op(&left_val, &right_val, "/"),
            BinaryOperator::Modulo => self.arithmetic_op(&left_val, &right_val, "%"),
            BinaryOperator::Power => self.arithmetic_op(&left_val, &right_val, "^"),

            BinaryOperator::Equal => Ok(SqlValue::Boolean(
                self.compare_values(&left_val, &right_val)? == Ordering::Equal,
            )),
            BinaryOperator::NotEqual => Ok(SqlValue::Boolean(
                self.compare_values(&left_val, &right_val)? != Ordering::Equal,
            )),
            BinaryOperator::LessThan => Ok(SqlValue::Boolean(
                self.compare_values(&left_val, &right_val)? == Ordering::Less,
            )),
            BinaryOperator::LessThanOrEqual => Ok(SqlValue::Boolean(matches!(
                self.compare_values(&left_val, &right_val)?,
                Ordering::Less | Ordering::Equal
            ))),
            BinaryOperator::GreaterThan => Ok(SqlValue::Boolean(
                self.compare_values(&left_val, &right_val)? == Ordering::Greater,
            )),
            BinaryOperator::GreaterThanOrEqual => Ok(SqlValue::Boolean(matches!(
                self.compare_values(&left_val, &right_val)?,
                Ordering::Greater | Ordering::Equal
            ))),

            BinaryOperator::And => self.logical_and(&left_val, &right_val),
            BinaryOperator::Or => self.logical_or(&left_val, &right_val),

            BinaryOperator::Concat => self.string_concat(&left_val, &right_val),
            BinaryOperator::Like => self.pattern_match(&left_val, &right_val, false, false),
            BinaryOperator::ILike => self.pattern_match(&left_val, &right_val, true, false),

            BinaryOperator::VectorDistance => {
                self.vector_distance(&left_val, &right_val, VectorOperator::L2Distance)
            }
            BinaryOperator::VectorInnerProduct => {
                self.vector_distance(&left_val, &right_val, VectorOperator::InnerProduct)
            }
            BinaryOperator::VectorCosineDistance => {
                self.vector_distance(&left_val, &right_val, VectorOperator::CosineDistance)
            }

            // JSON operators
            BinaryOperator::JsonExtract => self.json_extract(&left_val, &right_val),
            BinaryOperator::JsonExtractText => self.json_extract_text(&left_val, &right_val),
            BinaryOperator::JsonPathExtract => self.json_path_extract(&left_val, &right_val),
            BinaryOperator::JsonPathExtractText => {
                self.json_path_extract_text(&left_val, &right_val)
            }
            BinaryOperator::JsonContains => self.json_contains(&left_val, &right_val),
            BinaryOperator::JsonContainedBy => self.json_contained_by(&left_val, &right_val),
            BinaryOperator::JsonExists => self.json_exists(&left_val, &right_val),
            BinaryOperator::JsonExistsAny => self.json_exists_any(&left_val, &right_val),
            BinaryOperator::JsonExistsAll => self.json_exists_all(&left_val, &right_val),
            BinaryOperator::JsonConcat => self.json_concat(&left_val, &right_val),
            BinaryOperator::JsonDelete => self.json_delete(&left_val, &right_val),
            BinaryOperator::JsonDeletePath => self.json_delete_path(&left_val, &right_val),

            // Range operators (PostgreSQL range types)
            BinaryOperator::RangeContains => self.range_contains(&left_val, &right_val),
            BinaryOperator::RangeContainedBy => self.range_contained_by(&left_val, &right_val),
            BinaryOperator::RangeOverlaps => self.range_overlaps(&left_val, &right_val),
            BinaryOperator::RangeAdjacent => self.range_adjacent(&left_val, &right_val),
            BinaryOperator::RangeStrictlyLeft => self.range_strictly_left(&left_val, &right_val),
            BinaryOperator::RangeStrictlyRight => self.range_strictly_right(&left_val, &right_val),
            BinaryOperator::RangeNotExtendRight => {
                self.range_not_extend_right(&left_val, &right_val)
            }
            BinaryOperator::RangeNotExtendLeft => self.range_not_extend_left(&left_val, &right_val),

            // Text Search operators
            BinaryOperator::TextSearchMatch => self.text_search_match(&left_val, &right_val),
            BinaryOperator::TextSearchContains => self.text_search_contains(&left_val, &right_val),
            BinaryOperator::TextSearchContainedBy => {
                self.text_search_contained_by(&left_val, &right_val)
            }
            BinaryOperator::TextSearchConcat => self.text_search_concat(&left_val, &right_val),
            BinaryOperator::TextSearchAnd => self.text_search_and(&left_val, &right_val),
            BinaryOperator::TextSearchNot => self.text_search_not(&left_val, &right_val),
            BinaryOperator::TextSearchFollowedBy => {
                self.text_search_followed_by(&left_val, &right_val)
            }

            // Comparison operators
            BinaryOperator::IsDistinctFrom => self.is_distinct_from(&left_val, &right_val),
            BinaryOperator::IsNotDistinctFrom => self.is_not_distinct_from(&left_val, &right_val),
            
            // Regex operators
            BinaryOperator::RegexMatch => self.regex_match(&left_val, &right_val, false, false),
            BinaryOperator::RegexMatchCaseInsensitive => {
                self.regex_match(&left_val, &right_val, true, false)
            }
            BinaryOperator::RegexNotMatch => self.regex_match(&left_val, &right_val, false, true),
            BinaryOperator::RegexNotMatchCaseInsensitive => {
                self.regex_match(&left_val, &right_val, true, true)
            }

            // Bitwise operators
            BinaryOperator::BitwiseAnd => self.bitwise_and(&left_val, &right_val),
            BinaryOperator::BitwiseOr => self.bitwise_or(&left_val, &right_val),
            BinaryOperator::BitwiseXor => self.bitwise_xor(&left_val, &right_val),
            BinaryOperator::LeftShift => self.left_shift(&left_val, &right_val),
            BinaryOperator::RightShift => self.right_shift(&left_val, &right_val),

            _ => Err(ProtocolError::not_implemented(
                "Binary operator",
                &format!("{operator:?}"),
            )),
        }
    }

    fn evaluate_unary_op(
        &mut self,
        operator: &UnaryOperator,
        operand: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(operand, context)?;

        match operator {
            UnaryOperator::Plus => Ok(value),
            UnaryOperator::Minus => self.negate_value(&value),
            UnaryOperator::Not => self.logical_not(&value),
            UnaryOperator::BitwiseNot => self.bitwise_not(&value),
            UnaryOperator::SquareRoot => self.square_root(&value),
            UnaryOperator::CubeRoot => self.cube_root(&value),
            UnaryOperator::AbsoluteValue => self.absolute_value(&value),
            UnaryOperator::IsNull => Ok(SqlValue::Boolean(value.is_null())),
            UnaryOperator::IsNotNull => Ok(SqlValue::Boolean(!value.is_null())),
            UnaryOperator::IsTrue => {
                Ok(SqlValue::Boolean(matches!(value, SqlValue::Boolean(true))))
            }
            UnaryOperator::IsNotTrue => {
                Ok(SqlValue::Boolean(!matches!(value, SqlValue::Boolean(true))))
            }
            UnaryOperator::IsFalse => {
                Ok(SqlValue::Boolean(matches!(value, SqlValue::Boolean(false))))
            }
            UnaryOperator::IsNotFalse => Ok(SqlValue::Boolean(!matches!(
                value,
                SqlValue::Boolean(false)
            ))),
            _ => Err(ProtocolError::not_implemented(
                "Unary operator",
                &format!("{operator:?}"),
            )),
        }
    }

    fn evaluate_function_call(
        &mut self,
        func_call: &FunctionCall,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let func_name = match &func_call.name {
            FunctionName::Simple(name) => name.to_uppercase(),
            FunctionName::Qualified { schema: _, name } => name.to_uppercase(),
        };

        // Evaluate arguments
        let mut args = Vec::new();
        for arg_expr in &func_call.args {
            args.push(self.evaluate(arg_expr, context)?);
        }

        match func_name.as_str() {
            // Aggregate functions
            "COUNT" => self.evaluate_count(&args, func_call.distinct),
            "SUM" => self.evaluate_sum(&args),
            "AVG" => self.evaluate_avg(&args),
            "MIN" => self.evaluate_min(&args),
            "MAX" => self.evaluate_max(&args),
            "ARRAY_AGG" => self.evaluate_array_agg(&args),
            "STRING_AGG" => self.evaluate_string_agg(&args),
            "BOOL_AND" | "EVERY" => self.evaluate_bool_and(&args),
            "BOOL_OR" => self.evaluate_bool_or(&args),
            "BIT_AND" => self.evaluate_bit_and(&args),
            "BIT_OR" => self.evaluate_bit_or(&args),
            "BIT_XOR" => self.evaluate_bit_xor(&args),
            "VARIANCE" | "VAR_POP" => self.evaluate_variance(&args),
            "VAR_SAMP" => self.evaluate_var_samp(&args),
            "STDDEV" | "STDDEV_POP" => self.evaluate_stddev(&args),
            "STDDEV_SAMP" => self.evaluate_stddev_samp(&args),

            // String functions
            "LENGTH" | "CHAR_LENGTH" | "CHARACTER_LENGTH" => self.evaluate_length(&args),
            "UPPER" => self.evaluate_upper(&args),
            "LOWER" => self.evaluate_lower(&args),
            "SUBSTRING" | "SUBSTR" => self.evaluate_substring(&args),
            "REPLACE" => self.evaluate_replace(&args),
            "CONCAT" => self.evaluate_concat(&args),
            "LEFT" => self.evaluate_left(&args),
            "RIGHT" => self.evaluate_right(&args),
            "LPAD" => self.evaluate_lpad(&args),
            "RPAD" => self.evaluate_rpad(&args),
            "REVERSE" => self.evaluate_reverse(&args),
            "SPLIT_PART" => self.evaluate_split_part(&args),
            "TRIM" | "BTRIM" => self.evaluate_trim(&args),
            "LTRIM" => self.evaluate_ltrim(&args),
            "RTRIM" => self.evaluate_rtrim(&args),
            "POSITION" | "STRPOS" => self.evaluate_position(&args),
            "INITCAP" => self.evaluate_initcap(&args),
            "REPEAT" => self.evaluate_repeat(&args),
            "ASCII" => self.evaluate_ascii(&args),
            "CHR" => self.evaluate_chr(&args),
            "MD5" => self.evaluate_md5(&args),
            "ENCODE" => self.evaluate_encode(&args),
            "DECODE" => self.evaluate_decode(&args),
            "OCTET_LENGTH" => self.evaluate_octet_length(&args),
            "BIT_LENGTH" => self.evaluate_bit_length(&args),
            "OVERLAY" => self.evaluate_overlay(&args),
            "TRANSLATE" => self.evaluate_translate(&args),
            "QUOTE_LITERAL" => self.evaluate_quote_literal(&args),
            "QUOTE_IDENT" => self.evaluate_quote_ident(&args),
            "FORMAT" => self.evaluate_format(&args),

            // Math functions
            "ABS" => self.evaluate_abs(&args),
            "ROUND" => self.evaluate_round(&args),
            "CEILING" | "CEIL" => self.evaluate_ceiling(&args),
            "FLOOR" => self.evaluate_floor(&args),
            "SQRT" => self.evaluate_sqrt(&args),
            "CBRT" => self.evaluate_cbrt(&args),
            "POWER" | "POW" => self.evaluate_power(&args),
            "EXP" => self.evaluate_exp(&args),
            "LN" => self.evaluate_ln(&args),
            "LOG" | "LOG10" => self.evaluate_log(&args),
            "MOD" => self.evaluate_mod(&args),
            "DIV" => self.evaluate_div(&args),
            "FACTORIAL" => self.evaluate_factorial(&args),
            "GCD" => self.evaluate_gcd(&args),
            "LCM" => self.evaluate_lcm(&args),
            "PI" => self.evaluate_pi(&args),
            "RADIANS" => self.evaluate_radians(&args),
            "DEGREES" => self.evaluate_degrees(&args),
            "SIN" => self.evaluate_sin(&args),
            "COS" => self.evaluate_cos(&args),
            "TAN" => self.evaluate_tan(&args),
            "COT" => self.evaluate_cot(&args),
            "ASIN" => self.evaluate_asin(&args),
            "ACOS" => self.evaluate_acos(&args),
            "ATAN" => self.evaluate_atan(&args),
            "ATAN2" => self.evaluate_atan2(&args),
            // Hyperbolic functions (PostgreSQL 18)
            "SINH" => self.evaluate_sinh(&args),
            "COSH" => self.evaluate_cosh(&args),
            "TANH" => self.evaluate_tanh(&args),
            // Inverse hyperbolic functions (PostgreSQL 18)
            "ASINH" => self.evaluate_asinh(&args),
            "ACOSH" => self.evaluate_acosh(&args),
            "ATANH" => self.evaluate_atanh(&args),
            "SIGN" => self.evaluate_sign(&args),
            "TRUNC" | "TRUNCATE" => self.evaluate_trunc(&args),

            // Date functions
            "NOW" => self.evaluate_now(&args),
            "CURRENT_DATE" | "CURDATE" => self.evaluate_current_date(&args),
            "CURRENT_TIME" => self.evaluate_current_time(&args),
            "CURRENT_TIMESTAMP" => self.evaluate_current_timestamp(&args),
            "YEAR" => self.evaluate_year(&args),
            "MONTH" => self.evaluate_month(&args),
            "DAY" | "DAYOFMONTH" => self.evaluate_day(&args),
            "EXTRACT" | "DATE_PART" => self.evaluate_extract(&args),
            "DATE_TRUNC" => self.evaluate_date_trunc(&args),
            "HOUR" => self.evaluate_hour(&args),
            "MINUTE" => self.evaluate_minute(&args),
            "SECOND" => self.evaluate_second(&args),
            "WEEK" => self.evaluate_week(&args),
            "QUARTER" => self.evaluate_quarter(&args),
            "DAYOFWEEK" | "DOW" => self.evaluate_day_of_week(&args),
            "DAYOFYEAR" | "DOY" => self.evaluate_day_of_year(&args),

            // Vector functions
            "VECTOR_DIMS" => self.evaluate_vector_dims(&args),
            "VECTOR_NORM" => self.evaluate_vector_norm(&args),

            // Conditional functions
            "COALESCE" => self.evaluate_coalesce(&args),
            "NULLIF" => self.evaluate_nullif(&args),
            "GREATEST" => self.evaluate_greatest(&args),
            "LEAST" => self.evaluate_least(&args),

            // TimescaleDB functions
            "CREATE_HYPERTABLE" => Ok(SqlValue::Text("Hypertable created".to_string())),

            "TIME_BUCKET" => {
                // time_bucket(interval, timestamp) - bucket timestamps into intervals
                if args.len() != 2 {
                    return Err(ProtocolError::PostgresError(
                        "time_bucket requires 2 arguments: interval and timestamp".to_string(),
                    ));
                }

                let interval = &args[0];
                let timestamp = &args[1];

                // Extract interval duration in microseconds
                // Auto-cast string literals to intervals
                let interval_micros = match interval {
                    SqlValue::Interval(pg_interval) => {
                        // Convert PostgresInterval to total microseconds
                        // Note: This is a simplified conversion that doesn't handle months/days perfectly
                        // For proper handling, we'd need the reference timestamp
                        let days_micros = pg_interval.days as i64 * 24 * 3600 * 1_000_000;
                        let months_micros = pg_interval.months as i64 * 30 * 24 * 3600 * 1_000_000; // Approximate
                        pg_interval.microseconds + days_micros + months_micros
                    }
                    // Auto-cast string to interval
                    SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                        // Parse the string as an interval
                        match crate::protocols::postgres_wire::sql::types::SqlValue::parse_interval(s) {
                            Ok(SqlValue::Interval(pg_interval)) => {
                                let days_micros = pg_interval.days as i64 * 24 * 3600 * 1_000_000;
                                let months_micros = pg_interval.months as i64 * 30 * 24 * 3600 * 1_000_000;
                                pg_interval.microseconds + days_micros + months_micros
                            }
                            _ => return Err(ProtocolError::PostgresError(
                                format!("Invalid interval string: {}", s)
                            )),
                        }
                    }
                    _ => return Err(ProtocolError::PostgresError(
                        format!("time_bucket first argument must be an interval or interval string, got {:?}", interval)
                    )),
                };

                // Extract timestamp
                let ts = match timestamp {
                    SqlValue::Timestamp(dt) => dt,
                    SqlValue::TimestampWithTimezone(dt) => &dt.naive_utc(),
                    _ => {
                        return Err(ProtocolError::PostgresError(
                            "time_bucket second argument must be a timestamp".to_string(),
                        ))
                    }
                };

                // Calculate bucket start time
                // Convert timestamp to microseconds since epoch
                let ts_micros = ts.and_utc().timestamp_micros();

                // Calculate bucket start (floor division)
                let bucket_start_micros = (ts_micros / interval_micros) * interval_micros;

                // Convert back to timestamp
                use chrono::DateTime;
                let bucket_start = DateTime::from_timestamp_micros(bucket_start_micros)
                    .ok_or_else(|| ProtocolError::PostgresError("Invalid timestamp".to_string()))?
                    .naive_utc();

                Ok(SqlValue::Timestamp(bucket_start))
            }

            // OrbitQL OBJECT function - creates JSON object from key-value pairs
            "OBJECT" => {
                // OBJECT('key1', value1, 'key2', value2, ...)
                if args.len() % 2 != 0 {
                    return Err(ProtocolError::PostgresError(
                        "OBJECT function requires an even number of arguments (key-value pairs)"
                            .to_string(),
                    ));
                }

                let mut map = serde_json::Map::new();
                for i in (0..args.len()).step_by(2) {
                    // Key must be a string
                    let key = match &args[i] {
                        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
                        _ => {
                            return Err(ProtocolError::PostgresError(format!(
                                "OBJECT function keys must be strings, got {:?}",
                                args[i]
                            )))
                        }
                    };

                    // Convert value to JSON
                    let value = match &args[i + 1] {
                        SqlValue::Text(s) => serde_json::Value::String(s.clone()),
                        SqlValue::Integer(n) => serde_json::Value::Number((*n).into()),
                        SqlValue::BigInt(n) => serde_json::Value::Number((*n).into()),
                        SqlValue::DoublePrecision(f) => serde_json::Number::from_f64(*f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                        SqlValue::Boolean(b) => serde_json::Value::Bool(*b),
                        SqlValue::Null => serde_json::Value::Null,
                        SqlValue::Json(j) | SqlValue::Jsonb(j) => j.clone(),
                        _ => serde_json::Value::String(args[i + 1].to_postgres_string()),
                    };

                    map.insert(key, value);
                }

                Ok(SqlValue::Json(serde_json::Value::Object(map)))
            }

            // UUID functions (PostgreSQL 18 compatible)
            // UUIDv7 - timestamp-ordered UUID (PostgreSQL 18 feature)
            // Recommended for primary keys as they are sortable by creation time
            "UUID_GENERATE_V7" | "UUIDV7" => Ok(SqlValue::Uuid(Uuid::now_v7())),
            // UUIDv4 - random UUID (standard PostgreSQL function)
            // gen_random_uuid() is the standard PostgreSQL function name
            "GEN_RANDOM_UUID" | "UUID_GENERATE_V4" | "UUIDV4" => Ok(SqlValue::Uuid(Uuid::new_v4())),
            // UUID nil - all zeros (useful for comparisons)
            "UUID_NIL" => Ok(SqlValue::Uuid(Uuid::nil())),
            // UUID max - all ones
            "UUID_MAX" => Ok(SqlValue::Uuid(Uuid::max())),

            // Sequence functions
            "NEXTVAL" => self.evaluate_nextval(&args),
            "CURRVAL" => self.evaluate_currval(&args),
            "SETVAL" => self.evaluate_setval(&args),
            "LASTVAL" => self.evaluate_lastval(&args),

            // JSON functions
            "JSON_TABLE" => Ok(SqlValue::Text("JSON Table".to_string())),

            // Full-Text Search functions
            "TO_TSVECTOR" => self.evaluate_to_tsvector(&args),
            "TO_TSQUERY" => self.evaluate_to_tsquery(&args),
            "PLAINTO_TSQUERY" => self.evaluate_plainto_tsquery(&args),
            "PHRASETO_TSQUERY" => self.evaluate_phraseto_tsquery(&args),
            "WEBSEARCH_TO_TSQUERY" => self.evaluate_websearch_to_tsquery(&args),
            "SETWEIGHT" => self.evaluate_setweight(&args),
            "TS_RANK" => self.evaluate_ts_rank(&args),
            "TS_RANK_CD" => self.evaluate_ts_rank_cd(&args),
            "TS_HEADLINE" => self.evaluate_ts_headline(&args),
            "TSVECTOR_CONCAT" | "TSVECTOR_UPDATE_TRIGGER" => self.evaluate_tsvector_concat(&args),
            "NUMNODE" => self.evaluate_numnode(&args),
            "QUERYTREE" => self.evaluate_querytree(&args),
            "STRIP" => self.evaluate_strip(&args),
            "TS_LEXIZE" => self.evaluate_ts_lexize(&args),
            "TS_PARSE" => self.evaluate_ts_parse(&args),
            "TS_TOKEN_TYPE" => self.evaluate_ts_token_type(&args),
            "GET_CURRENT_TS_CONFIG" => self.evaluate_get_current_ts_config(&args),
            "ARRAY_TO_TSVECTOR" => self.evaluate_array_to_tsvector(&args),
            "TSVECTOR_TO_ARRAY" => self.evaluate_tsvector_to_array(&args),
            "TS_DELETE" => self.evaluate_ts_delete(&args),
            "TS_FILTER" => self.evaluate_ts_filter(&args),
            "TSQUERY_PHRASE" => self.evaluate_tsquery_phrase(&args),

            _ => Err(ProtocolError::not_implemented("Function", &func_name)),
        }
    }

    // ===== Sequence Functions =====

    /// Evaluate nextval('sequence_name') - advance sequence and return new value
    fn evaluate_nextval(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "nextval() requires exactly one argument".to_string(),
            ));
        }

        let sequence_name = match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "nextval() argument must be a text (sequence name)".to_string(),
                ))
            }
        };

        match &self.sequence_accessor {
            Some(accessor) => {
                let value = accessor.nextval(&sequence_name)?;
                Ok(SqlValue::BigInt(value))
            }
            None => Err(ProtocolError::PostgresError(
                "Sequence operations are not available in this context".to_string(),
            )),
        }
    }

    /// Evaluate currval('sequence_name') - return current value (must have called nextval first)
    fn evaluate_currval(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "currval() requires exactly one argument".to_string(),
            ));
        }

        let sequence_name = match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "currval() argument must be a text (sequence name)".to_string(),
                ))
            }
        };

        match &self.sequence_accessor {
            Some(accessor) => {
                let value = accessor.currval(&sequence_name)?;
                Ok(SqlValue::BigInt(value))
            }
            None => Err(ProtocolError::PostgresError(
                "Sequence operations are not available in this context".to_string(),
            )),
        }
    }

    /// Evaluate setval('sequence_name', value [, is_called]) - set sequence value
    fn evaluate_setval(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "setval() requires 2 or 3 arguments".to_string(),
            ));
        }

        let sequence_name = match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "setval() first argument must be a text (sequence name)".to_string(),
                ))
            }
        };

        let value = match &args[1] {
            SqlValue::Integer(n) => *n as i64,
            SqlValue::BigInt(n) => *n,
            _ => {
                return Err(ProtocolError::PostgresError(
                    "setval() second argument must be an integer".to_string(),
                ))
            }
        };

        let is_called = if args.len() == 3 {
            match &args[2] {
                SqlValue::Boolean(b) => *b,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "setval() third argument must be a boolean".to_string(),
                    ))
                }
            }
        } else {
            true // Default: is_called = true
        };

        match &self.sequence_accessor {
            Some(accessor) => {
                let result = accessor.setval(&sequence_name, value, is_called)?;
                Ok(SqlValue::BigInt(result))
            }
            None => Err(ProtocolError::PostgresError(
                "Sequence operations are not available in this context".to_string(),
            )),
        }
    }

    /// Evaluate lastval() - return last value from nextval in this session
    fn evaluate_lastval(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "lastval() takes no arguments".to_string(),
            ));
        }

        match &self.sequence_accessor {
            Some(accessor) => {
                let value = accessor.lastval()?;
                Ok(SqlValue::BigInt(value))
            }
            None => Err(ProtocolError::PostgresError(
                "Sequence operations are not available in this context".to_string(),
            )),
        }
    }

    fn evaluate_window_function(
        &mut self,
        function: &WindowFunctionType,
        _partition_by: &[Expression],
        order_by: &[OrderByItem],
        frame: &Option<WindowFrame>,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let window_context = context.window_frame.as_ref().ok_or_else(|| {
            ProtocolError::PostgresError("Window function used outside window context".to_string())
        })?;

        match function {
            WindowFunctionType::RowNumber => {
                // ROW_NUMBER() returns the row number within the partition
                let row_num = window_context.current_row_index + 1;
                Ok(SqlValue::BigInt(row_num as i64))
            }

            WindowFunctionType::Rank => {
                // RANK() returns the rank with gaps
                self.evaluate_rank(order_by, window_context, context, false)
            }

            WindowFunctionType::DenseRank => {
                // DENSE_RANK() returns the rank without gaps
                self.evaluate_rank(order_by, window_context, context, true)
            }

            WindowFunctionType::Lag {
                expr,
                offset,
                default,
            } => self.evaluate_lag_lead(
                expr,
                offset.as_deref(),
                default.as_deref(),
                window_context,
                context,
                true,
            ),

            WindowFunctionType::Lead {
                expr,
                offset,
                default,
            } => self.evaluate_lag_lead(
                expr,
                offset.as_deref(),
                default.as_deref(),
                window_context,
                context,
                false,
            ),

            WindowFunctionType::FirstValue(expr) => {
                self.evaluate_first_last_value(expr, window_context, context, true)
            }

            WindowFunctionType::LastValue(expr) => {
                self.evaluate_first_last_value(expr, window_context, context, false)
            }

            WindowFunctionType::NthValue { expr, n } => {
                self.evaluate_nth_value(expr, n, window_context, context)
            }

            WindowFunctionType::Ntile(n_expr) => {
                self.evaluate_ntile(n_expr, window_context, context)
            }

            WindowFunctionType::PercentRank => {
                self.evaluate_percent_rank(order_by, window_context, context)
            }

            WindowFunctionType::CumeDist => {
                self.evaluate_cume_dist(order_by, window_context, context)
            }

            WindowFunctionType::Aggregate(func) => {
                // Aggregate function used as window function
                self.evaluate_window_aggregate(func, frame, window_context, context)
            }
        }
    }

    fn evaluate_case_expression(
        &mut self,
        case_expr: &CaseExpression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // Handle simple vs searched case
        if let Some(operand) = &case_expr.operand {
            // Simple CASE: CASE operand WHEN value1 THEN result1 ...
            let case_value = self.evaluate(operand, context)?;

            for when_clause in &case_expr.when_clauses {
                let when_value = self.evaluate(&when_clause.condition, context)?;
                if self.compare_values(&case_value, &when_value)? == Ordering::Equal {
                    return self.evaluate(&when_clause.result, context);
                }
            }
        } else {
            // Searched CASE: CASE WHEN condition1 THEN result1 ...
            for when_clause in &case_expr.when_clauses {
                let condition_value = self.evaluate(&when_clause.condition, context)?;
                if self.is_true(&condition_value)? {
                    return self.evaluate(&when_clause.result, context);
                }
            }
        }

        // No conditions matched, use ELSE clause or NULL
        if let Some(else_clause) = &case_expr.else_clause {
            self.evaluate(else_clause, context)
        } else {
            Ok(SqlValue::Null)
        }
    }

    // Helper methods for arithmetic operations
    #[allow(clippy::only_used_in_recursion)]
    fn arithmetic_op(
        &self,
        left: &SqlValue,
        right: &SqlValue,
        op: &str,
    ) -> ProtocolResult<SqlValue> {
        if left.is_null() || right.is_null() {
            return Ok(SqlValue::Null);
        }

        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => match op {
                "+" => Ok(SqlValue::Integer(a + b)),
                "-" => Ok(SqlValue::Integer(a - b)),
                "*" => Ok(SqlValue::Integer(a * b)),
                "/" => {
                    if *b == 0 {
                        Err(ProtocolError::PostgresError("Division by zero".to_string()))
                    } else {
                        Ok(SqlValue::Integer(a / b))
                    }
                }
                "%" => {
                    if *b == 0 {
                        Err(ProtocolError::PostgresError("Division by zero".to_string()))
                    } else {
                        Ok(SqlValue::Integer(a % b))
                    }
                }
                "^" => Ok(SqlValue::DoublePrecision((*a as f64).powf(*b as f64))),
                _ => Err(ProtocolError::PostgresError(format!(
                    "Unknown arithmetic operator: {op}"
                ))),
            },
            (SqlValue::DoublePrecision(a), SqlValue::DoublePrecision(b)) => match op {
                "+" => Ok(SqlValue::DoublePrecision(a + b)),
                "-" => Ok(SqlValue::DoublePrecision(a - b)),
                "*" => Ok(SqlValue::DoublePrecision(a * b)),
                "/" => {
                    if *b == 0.0 {
                        Err(ProtocolError::PostgresError("Division by zero".to_string()))
                    } else {
                        Ok(SqlValue::DoublePrecision(a / b))
                    }
                }
                "%" => Ok(SqlValue::DoublePrecision(a % b)),
                "^" => Ok(SqlValue::DoublePrecision(a.powf(*b))),
                _ => Err(ProtocolError::PostgresError(format!(
                    "Unknown arithmetic operator: {op}"
                ))),
            },
            // Type coercion for mixed types
            (SqlValue::Integer(a), SqlValue::DoublePrecision(_b)) => {
                self.arithmetic_op(&SqlValue::DoublePrecision(*a as f64), right, op)
            }
            (SqlValue::DoublePrecision(_), SqlValue::Integer(b)) => {
                self.arithmetic_op(left, &SqlValue::DoublePrecision(*b as f64), op)
            }

            // Timestamp arithmetic with intervals
            (SqlValue::Timestamp(ts), SqlValue::Interval(interval)) => {
                match op {
                    "+" => {
                        // Add interval to timestamp
                        let mut result = *ts;

                        // Add months
                        if interval.months != 0 {
                            result = result
                                .checked_add_months(chrono::Months::new(
                                    interval.months.unsigned_abs(),
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp overflow".to_string())
                                })?;
                        }

                        // Add days
                        if interval.days != 0 {
                            result = result
                                .checked_add_days(chrono::Days::new(
                                    interval.days.unsigned_abs() as u64
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp overflow".to_string())
                                })?;
                        }

                        // Add microseconds
                        if interval.microseconds != 0 {
                            result = result
                                .checked_add_signed(chrono::Duration::microseconds(
                                    interval.microseconds,
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp overflow".to_string())
                                })?;
                        }

                        Ok(SqlValue::Timestamp(result))
                    }
                    "-" => {
                        // Subtract interval from timestamp
                        let mut result = *ts;

                        // Subtract months
                        if interval.months != 0 {
                            result = result
                                .checked_sub_months(chrono::Months::new(
                                    interval.months.unsigned_abs(),
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp underflow".to_string())
                                })?;
                        }

                        // Subtract days
                        if interval.days != 0 {
                            result = result
                                .checked_sub_days(chrono::Days::new(
                                    interval.days.unsigned_abs() as u64
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp underflow".to_string())
                                })?;
                        }

                        // Subtract microseconds
                        if interval.microseconds != 0 {
                            result = result
                                .checked_sub_signed(chrono::Duration::microseconds(
                                    interval.microseconds,
                                ))
                                .ok_or_else(|| {
                                    ProtocolError::PostgresError("Timestamp underflow".to_string())
                                })?;
                        }

                        Ok(SqlValue::Timestamp(result))
                    }
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Cannot perform operation {op} on timestamp and interval"
                    ))),
                }
            }

            // TimestampWithTimezone arithmetic with intervals
            (SqlValue::TimestampWithTimezone(ts), SqlValue::Interval(_interval)) => {
                match op {
                    "+" | "-" => {
                        // Convert to naive, perform operation, convert back
                        let naive_ts = ts.naive_utc();
                        let result_naive =
                            self.arithmetic_op(&SqlValue::Timestamp(naive_ts), right, op)?;

                        match result_naive {
                            SqlValue::Timestamp(dt) => {
                                Ok(SqlValue::TimestampWithTimezone(dt.and_utc()))
                            }
                            _ => Err(ProtocolError::PostgresError(
                                "Unexpected result type".to_string(),
                            )),
                        }
                    }
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Cannot perform operation {op} on timestamp with timezone and interval"
                    ))),
                }
            }

            _ => Err(ProtocolError::PostgresError(format!(
                "Cannot perform arithmetic operation {op} on {left:?} and {right:?}"
            ))),
        }
    }

    fn compare_values(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<Ordering> {
        match (left, right) {
            (SqlValue::Null, SqlValue::Null) => Ok(Ordering::Equal),
            (SqlValue::Null, _) => Ok(Ordering::Less),
            (_, SqlValue::Null) => Ok(Ordering::Greater),

            (SqlValue::Boolean(a), SqlValue::Boolean(b)) => Ok(a.cmp(b)),
            (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(a.cmp(b)),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => Ok(a.cmp(b)),
            (SqlValue::DoublePrecision(a), SqlValue::DoublePrecision(b)) => {
                a.partial_cmp(b).ok_or_else(|| {
                    ProtocolError::PostgresError("Cannot compare float values".to_string())
                })
            }
            (SqlValue::Text(a), SqlValue::Text(b)) => Ok(a.cmp(b)),
            (SqlValue::Varchar(a), SqlValue::Varchar(b)) => Ok(a.cmp(b)),

            // Type coercion for numeric types
            (SqlValue::Integer(a), SqlValue::DoublePrecision(b)) => (*a as f64)
                .partial_cmp(b)
                .ok_or_else(|| ProtocolError::PostgresError("Cannot compare values".to_string())),
            (SqlValue::DoublePrecision(a), SqlValue::Integer(b)) => a
                .partial_cmp(&(*b as f64))
                .ok_or_else(|| ProtocolError::PostgresError("Cannot compare values".to_string())),

            _ => Err(ProtocolError::PostgresError(format!(
                "Cannot compare {left:?} and {right:?}"
            ))),
        }
    }

    fn logical_and(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Boolean(false), _) | (_, SqlValue::Boolean(false)) => {
                Ok(SqlValue::Boolean(false))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Boolean(true), SqlValue::Boolean(true)) => Ok(SqlValue::Boolean(true)),
            _ => Err(ProtocolError::PostgresError(
                "AND operator requires boolean operands".to_string(),
            )),
        }
    }

    fn logical_or(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Boolean(true), _) | (_, SqlValue::Boolean(true)) => {
                Ok(SqlValue::Boolean(true))
            }
            (SqlValue::Boolean(false), SqlValue::Boolean(false)) => Ok(SqlValue::Boolean(false)),
            (SqlValue::Null, SqlValue::Boolean(false))
            | (SqlValue::Boolean(false), SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "OR operator requires boolean operands".to_string(),
            )),
        }
    }

    fn logical_not(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match value {
            SqlValue::Boolean(b) => Ok(SqlValue::Boolean(!b)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "NOT operator requires boolean operand".to_string(),
            )),
        }
    }

    fn string_concat(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        if left.is_null() || right.is_null() {
            return Ok(SqlValue::Null);
        }

        let left_str = left.to_postgres_string();
        let right_str = right.to_postgres_string();
        Ok(SqlValue::Text(format!("{left_str}{right_str}")))
    }

    fn negate_value(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match value {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(-i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(-i)),
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(-f)),
            SqlValue::Real(f) => Ok(SqlValue::Real(-f)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(format!(
                "Cannot negate {value:?}"
            ))),
        }
    }

    fn bitwise_not(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match value {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(!i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(!i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bitwise NOT requires integer operand".to_string(),
            )),
        }
    }

    fn square_root(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match Self::to_f64_static(value)? {
            Some(f) => {
                if f < 0.0 {
                    Err(ProtocolError::PostgresError(
                        "Square root of negative number".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision(f.sqrt()))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn cube_root(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match Self::to_f64_static(value)? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.cbrt())),
            None => Ok(SqlValue::Null),
        }
    }

    fn absolute_value(&self, value: &SqlValue) -> ProtocolResult<SqlValue> {
        match value {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(i.abs())),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(i.abs())),
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f.abs())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.abs())),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Absolute value requires numeric operand".to_string(),
            )),
        }
    }

    fn bitwise_and(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(SqlValue::Integer(a & b)),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => Ok(SqlValue::BigInt(a & b)),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bitwise AND requires integer operands".to_string(),
            )),
        }
    }

    fn bitwise_or(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(SqlValue::Integer(a | b)),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => Ok(SqlValue::BigInt(a | b)),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bitwise OR requires integer operands".to_string(),
            )),
        }
    }

    fn bitwise_xor(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(SqlValue::Integer(a ^ b)),
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => Ok(SqlValue::BigInt(a ^ b)),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bitwise XOR requires integer operands".to_string(),
            )),
        }
    }

    fn left_shift(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => {
                if *b < 0 || *b > 31 {
                    return Err(ProtocolError::PostgresError(
                        "Shift amount out of range".to_string(),
                    ));
                }
                Ok(SqlValue::Integer(a << b))
            }
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => {
                if *b < 0 || *b > 63 {
                    return Err(ProtocolError::PostgresError(
                        "Shift amount out of range".to_string(),
                    ));
                }
                Ok(SqlValue::BigInt(a << b))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bit shift requires integer operands".to_string(),
            )),
        }
    }

    fn right_shift(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => {
                if *b < 0 || *b > 31 {
                    return Err(ProtocolError::PostgresError(
                        "Shift amount out of range".to_string(),
                    ));
                }
                Ok(SqlValue::Integer(a >> b))
            }
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => {
                if *b < 0 || *b > 63 {
                    return Err(ProtocolError::PostgresError(
                        "Shift amount out of range".to_string(),
                    ));
                }
                Ok(SqlValue::BigInt(a >> b))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Bit shift requires integer operands".to_string(),
            )),
        }
    }

    /// IS DISTINCT FROM - null-safe not equal
    fn is_distinct_from(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, SqlValue::Null) => Ok(SqlValue::Boolean(false)),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Boolean(true)),
            _ => Ok(SqlValue::Boolean(
                self.compare_values(left, right)? != std::cmp::Ordering::Equal,
            )),
        }
    }

    /// IS NOT DISTINCT FROM - null-safe equal
    fn is_not_distinct_from(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, SqlValue::Null) => Ok(SqlValue::Boolean(true)),
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Boolean(false)),
            _ => Ok(SqlValue::Boolean(
                self.compare_values(left, right)? == std::cmp::Ordering::Equal,
            )),
        }
    }

    /// Regex match operator (~, ~*, !~, !~*)
    fn regex_match(
        &self,
        text: &SqlValue,
        pattern: &SqlValue,
        case_insensitive: bool,
        negated: bool,
    ) -> ProtocolResult<SqlValue> {
        let text_str = match text {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => return Err(ProtocolError::PostgresError(
                "Regex match requires text operand".to_string(),
            )),
        };

        let pattern_str = match pattern {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => return Err(ProtocolError::PostgresError(
                "Regex match requires text pattern".to_string(),
            )),
        };

        // Build regex with case-insensitive flag if needed
        let regex_pattern = if case_insensitive {
            format!("(?i){}", pattern_str)
        } else {
            pattern_str
        };

        match regex::Regex::new(&regex_pattern) {
            Ok(re) => {
                let matches = re.is_match(&text_str);
                Ok(SqlValue::Boolean(if negated { !matches } else { matches }))
            }
            Err(e) => Err(ProtocolError::PostgresError(format!(
                "Invalid regex pattern: {}",
                e
            ))),
        }
    }

    fn is_true(&self, value: &SqlValue) -> ProtocolResult<bool> {
        match value {
            SqlValue::Boolean(b) => Ok(*b),
            SqlValue::Null => Ok(false),
            _ => Err(ProtocolError::PostgresError(
                "Value is not a boolean".to_string(),
            )),
        }
    }

    // Aggregate function implementations
    fn evaluate_count(&self, args: &[SqlValue], _distinct: bool) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            // COUNT(*)
            Ok(SqlValue::BigInt(1)) // This would be accumulated by the query engine
        } else {
            // COUNT(expr) - only count non-null values
            let count = if args[0].is_null() { 0 } else { 1 };
            Ok(SqlValue::BigInt(count))
        }
    }

    fn evaluate_sum(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SUM requires exactly one argument".to_string(),
            ));
        }

        if args[0].is_null() {
            Ok(SqlValue::Null)
        } else {
            Ok(args[0].clone()) // This would be accumulated by the query engine
        }
    }

    fn evaluate_avg(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "AVG requires exactly one argument".to_string(),
            ));
        }

        Ok(args[0].clone()) // This would be computed by the query engine
    }

    fn evaluate_min(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "MIN requires exactly one argument".to_string(),
            ));
        }

        Ok(args[0].clone()) // This would be computed by the query engine
    }

    fn evaluate_max(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "MAX requires exactly one argument".to_string(),
            ));
        }

        Ok(args[0].clone()) // This would be computed by the query engine
    }

    fn evaluate_array_agg(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ARRAY_AGG requires exactly one argument".to_string(),
            ));
        }

        // In single-row evaluation, return the value in an array
        // The query engine accumulates all values during aggregation
        if args[0].is_null() {
            Ok(SqlValue::Array(vec![]))
        } else {
            Ok(SqlValue::Array(vec![args[0].clone()]))
        }
    }

    fn evaluate_string_agg(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 1 || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "STRING_AGG requires one or two arguments".to_string(),
            ));
        }

        // First arg is the value, second arg is the delimiter (default ',')
        let _delimiter = if args.len() == 2 {
            match &args[1] {
                SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
                SqlValue::Null => return Ok(SqlValue::Null),
                _ => ",".to_string(),
            }
        } else {
            ",".to_string()
        };

        // In single-row evaluation, return the value as-is
        // The query engine accumulates and joins all values during aggregation
        if args[0].is_null() {
            Ok(SqlValue::Null)
        } else {
            Ok(SqlValue::Text(args[0].to_postgres_string()))
        }
    }

    fn evaluate_bool_and(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BOOL_AND requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Boolean(b) => Ok(SqlValue::Boolean(*b)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BOOL_AND requires boolean argument".to_string(),
            )),
        }
    }

    fn evaluate_bool_or(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BOOL_OR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Boolean(b) => Ok(SqlValue::Boolean(*b)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BOOL_OR requires boolean argument".to_string(),
            )),
        }
    }

    fn evaluate_bit_and(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BIT_AND requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BIT_AND requires integer argument".to_string(),
            )),
        }
    }

    fn evaluate_bit_or(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BIT_OR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BIT_OR requires integer argument".to_string(),
            )),
        }
    }

    fn evaluate_bit_xor(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BIT_XOR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BIT_XOR requires integer argument".to_string(),
            )),
        }
    }

    fn evaluate_variance(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "VARIANCE requires exactly one argument".to_string(),
            ));
        }

        // For single value, variance is 0
        match &args[0] {
            SqlValue::Integer(_) | SqlValue::BigInt(_) | SqlValue::Real(_) | SqlValue::DoublePrecision(_) => {
                Ok(SqlValue::DoublePrecision(0.0))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "VARIANCE requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_var_samp(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "VAR_SAMP requires exactly one argument".to_string(),
            ));
        }

        // For single value, sample variance is NULL
        match &args[0] {
            SqlValue::Integer(_) | SqlValue::BigInt(_) | SqlValue::Real(_) | SqlValue::DoublePrecision(_) => {
                Ok(SqlValue::Null)
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "VAR_SAMP requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_stddev(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "STDDEV requires exactly one argument".to_string(),
            ));
        }

        // For single value, stddev is 0
        match &args[0] {
            SqlValue::Integer(_) | SqlValue::BigInt(_) | SqlValue::Real(_) | SqlValue::DoublePrecision(_) => {
                Ok(SqlValue::DoublePrecision(0.0))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "STDDEV requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_stddev_samp(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "STDDEV_SAMP requires exactly one argument".to_string(),
            ));
        }

        // For single value, sample stddev is NULL
        match &args[0] {
            SqlValue::Integer(_) | SqlValue::BigInt(_) | SqlValue::Real(_) | SqlValue::DoublePrecision(_) => {
                Ok(SqlValue::Null)
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "STDDEV_SAMP requires numeric argument".to_string(),
            )),
        }
    }

    // String function implementations
    fn evaluate_length(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "LENGTH requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(SqlValue::Integer(s.chars().count() as i32))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "LENGTH requires string argument".to_string(),
            )),
        }
    }

    fn evaluate_upper(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "UPPER requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(SqlValue::Text(s.to_uppercase()))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "UPPER requires string argument".to_string(),
            )),
        }
    }

    fn evaluate_lower(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "LOWER requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(SqlValue::Text(s.to_lowercase()))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "LOWER requires string argument".to_string(),
            )),
        }
    }

    fn evaluate_substring(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "SUBSTRING requires 2 or 3 arguments".to_string(),
            ));
        }

        let string = match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "SUBSTRING requires string argument".to_string(),
                ))
            }
        };

        let start = match &args[1] {
            SqlValue::Integer(i) => *i as usize,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "SUBSTRING start position must be integer".to_string(),
                ))
            }
        };

        let length = if args.len() == 3 {
            match &args[2] {
                SqlValue::Integer(i) => Some(*i as usize),
                SqlValue::Null => return Ok(SqlValue::Null),
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "SUBSTRING length must be integer".to_string(),
                    ))
                }
            }
        } else {
            None
        };

        let chars: Vec<char> = string.chars().collect();
        let start_idx = if start > 0 { start - 1 } else { 0 };

        if start_idx >= chars.len() {
            return Ok(SqlValue::Text("".to_string()));
        }

        let end_idx = if let Some(len) = length {
            std::cmp::min(start_idx + len, chars.len())
        } else {
            chars.len()
        };

        let result: String = chars[start_idx..end_idx].iter().collect();
        Ok(SqlValue::Text(result))
    }

    fn evaluate_replace(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 3 {
            return Err(ProtocolError::PostgresError(
                "REPLACE requires exactly 3 arguments".to_string(),
            ));
        }

        let string = match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "REPLACE requires string arguments".to_string(),
                ))
            }
        };

        let from = match &args[1] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "REPLACE requires string arguments".to_string(),
                ))
            }
        };

        let to = match &args[2] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "REPLACE requires string arguments".to_string(),
                ))
            }
        };

        Ok(SqlValue::Text(string.replace(from, to)))
    }

    // Math function implementations
    fn evaluate_abs(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ABS requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(i.abs())),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(i.abs())),
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f.abs())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.abs())),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "ABS requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_round(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "ROUND requires 1 or 2 arguments".to_string(),
            ));
        }

        let precision = if args.len() == 2 {
            match &args[1] {
                SqlValue::Integer(i) => *i,
                SqlValue::Null => return Ok(SqlValue::Null),
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "ROUND precision must be integer".to_string(),
                    ))
                }
            }
        } else {
            0
        };

        match &args[0] {
            SqlValue::DoublePrecision(f) => {
                let multiplier = 10_f64.powi(precision);
                Ok(SqlValue::DoublePrecision(
                    (f * multiplier).round() / multiplier,
                ))
            }
            SqlValue::Real(f) => {
                let multiplier = 10_f32.powi(precision);
                Ok(SqlValue::Real((f * multiplier).round() / multiplier))
            }
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "ROUND requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_ceiling(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "CEILING requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f.ceil())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.ceil())),
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "CEILING requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_floor(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "FLOOR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f.floor())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.floor())),
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "FLOOR requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_sqrt(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SQRT requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::DoublePrecision(f) => {
                if *f < 0.0 {
                    Err(ProtocolError::PostgresError(
                        "SQRT of negative number".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision(f.sqrt()))
                }
            }
            SqlValue::Real(f) => {
                if *f < 0.0 {
                    Err(ProtocolError::PostgresError(
                        "SQRT of negative number".to_string(),
                    ))
                } else {
                    Ok(SqlValue::Real(f.sqrt()))
                }
            }
            SqlValue::Integer(i) => {
                if *i < 0 {
                    Err(ProtocolError::PostgresError(
                        "SQRT of negative number".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision((*i as f64).sqrt()))
                }
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "SQRT requires numeric argument".to_string(),
            )),
        }
    }

    /// Evaluate cbrt(x) - cube root
    fn evaluate_cbrt(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "CBRT requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f.cbrt())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.cbrt())),
            SqlValue::Integer(i) => Ok(SqlValue::DoublePrecision((*i as f64).cbrt())),
            SqlValue::BigInt(i) => Ok(SqlValue::DoublePrecision((*i as f64).cbrt())),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "CBRT requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_power(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "POWER requires exactly two arguments".to_string(),
            ));
        }

        let base = Self::to_f64_static(&args[0])?;
        let exp = Self::to_f64_static(&args[1])?;

        if base.is_none() || exp.is_none() {
            return Ok(SqlValue::Null);
        }

        Ok(SqlValue::DoublePrecision(base.unwrap().powf(exp.unwrap())))
    }

    fn evaluate_exp(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "EXP requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.exp())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_ln(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "LN requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                if f <= 0.0 {
                    Err(ProtocolError::PostgresError(
                        "LN requires positive argument".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision(f.ln()))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_log(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "LOG requires one or two arguments".to_string(),
            ));
        }

        if args.len() == 1 {
            // LOG(x) = log10(x)
            match Self::to_f64_static(&args[0])? {
                Some(f) => {
                    if f <= 0.0 {
                        Err(ProtocolError::PostgresError(
                            "LOG requires positive argument".to_string(),
                        ))
                    } else {
                        Ok(SqlValue::DoublePrecision(f.log10()))
                    }
                }
                None => Ok(SqlValue::Null),
            }
        } else {
            // LOG(base, x) = log_base(x)
            let base = Self::to_f64_static(&args[0])?;
            let x = Self::to_f64_static(&args[1])?;

            if base.is_none() || x.is_none() {
                return Ok(SqlValue::Null);
            }

            let base = base.unwrap();
            let x = x.unwrap();

            if base <= 0.0 || base == 1.0 || x <= 0.0 {
                Err(ProtocolError::PostgresError(
                    "LOG requires positive arguments and base != 1".to_string(),
                ))
            } else {
                Ok(SqlValue::DoublePrecision(x.log(base)))
            }
        }
    }

    fn evaluate_mod(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "MOD requires exactly two arguments".to_string(),
            ));
        }

        match (&args[0], &args[1]) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => {
                if *b == 0 {
                    Err(ProtocolError::PostgresError("Division by zero".to_string()))
                } else {
                    Ok(SqlValue::Integer(a % b))
                }
            }
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => {
                if *b == 0 {
                    Err(ProtocolError::PostgresError("Division by zero".to_string()))
                } else {
                    Ok(SqlValue::BigInt(a % b))
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => {
                let a = Self::to_f64_static(&args[0])?;
                let b = Self::to_f64_static(&args[1])?;
                match (a, b) {
                    (Some(a), Some(b)) => {
                        if b == 0.0 {
                            Err(ProtocolError::PostgresError("Division by zero".to_string()))
                        } else {
                            Ok(SqlValue::DoublePrecision(a % b))
                        }
                    }
                    _ => Ok(SqlValue::Null),
                }
            }
        }
    }

    /// Evaluate div(a, b) - integer division (truncate towards zero)
    fn evaluate_div(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "DIV requires exactly two arguments".to_string(),
            ));
        }

        match (&args[0], &args[1]) {
            (SqlValue::Integer(a), SqlValue::Integer(b)) => {
                if *b == 0 {
                    Err(ProtocolError::PostgresError("Division by zero".to_string()))
                } else {
                    Ok(SqlValue::Integer(a / b))
                }
            }
            (SqlValue::BigInt(a), SqlValue::BigInt(b)) => {
                if *b == 0 {
                    Err(ProtocolError::PostgresError("Division by zero".to_string()))
                } else {
                    Ok(SqlValue::BigInt(a / b))
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => {
                let a = Self::to_f64_static(&args[0])?;
                let b = Self::to_f64_static(&args[1])?;
                match (a, b) {
                    (Some(a), Some(b)) => {
                        if b == 0.0 {
                            Err(ProtocolError::PostgresError("Division by zero".to_string()))
                        } else {
                            Ok(SqlValue::BigInt((a / b).trunc() as i64))
                        }
                    }
                    _ => Ok(SqlValue::Null),
                }
            }
        }
    }

    /// Evaluate factorial(n) - n!
    fn evaluate_factorial(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "FACTORIAL requires exactly one argument".to_string(),
            ));
        }

        let n = match &args[0] {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "FACTORIAL requires integer argument".to_string(),
                ))
            }
        };

        if n < 0 {
            return Err(ProtocolError::PostgresError(
                "FACTORIAL of negative number".to_string(),
            ));
        }

        if n > 20 {
            return Err(ProtocolError::PostgresError(
                "FACTORIAL argument too large (max 20)".to_string(),
            ));
        }

        let result: i64 = (1..=n).product();
        Ok(SqlValue::BigInt(result))
    }

    /// Evaluate gcd(a, b) - greatest common divisor
    fn evaluate_gcd(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "GCD requires exactly two arguments".to_string(),
            ));
        }

        let a = match &args[0] {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "GCD requires integer arguments".to_string(),
                ))
            }
        };

        let b = match &args[1] {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "GCD requires integer arguments".to_string(),
                ))
            }
        };

        fn gcd(mut a: i64, mut b: i64) -> i64 {
            a = a.abs();
            b = b.abs();
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a
        }

        Ok(SqlValue::BigInt(gcd(a, b)))
    }

    /// Evaluate lcm(a, b) - least common multiple
    fn evaluate_lcm(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "LCM requires exactly two arguments".to_string(),
            ));
        }

        let a = match &args[0] {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "LCM requires integer arguments".to_string(),
                ))
            }
        };

        let b = match &args[1] {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "LCM requires integer arguments".to_string(),
                ))
            }
        };

        fn gcd(mut a: i64, mut b: i64) -> i64 {
            a = a.abs();
            b = b.abs();
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a
        }

        if a == 0 || b == 0 {
            Ok(SqlValue::BigInt(0))
        } else {
            Ok(SqlValue::BigInt((a.abs() / gcd(a, b)) * b.abs()))
        }
    }


    fn evaluate_pi(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "PI requires no arguments".to_string(),
            ));
        }
        Ok(SqlValue::DoublePrecision(std::f64::consts::PI))
    }

    fn evaluate_radians(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "RADIANS requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.to_radians())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_degrees(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "DEGREES requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.to_degrees())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_sin(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SIN requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.sin())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_cos(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "COS requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.cos())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_tan(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "TAN requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.tan())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_asin(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ASIN requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                if f < -1.0 || f > 1.0 {
                    Err(ProtocolError::PostgresError(
                        "ASIN argument must be between -1 and 1".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision(f.asin()))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_acos(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ACOS requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                if f < -1.0 || f > 1.0 {
                    Err(ProtocolError::PostgresError(
                        "ACOS argument must be between -1 and 1".to_string(),
                    ))
                } else {
                    Ok(SqlValue::DoublePrecision(f.acos()))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_atan(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ATAN requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.atan())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_atan2(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ATAN2 requires exactly two arguments".to_string(),
            ));
        }

        let y = Self::to_f64_static(&args[0])?;
        let x = Self::to_f64_static(&args[1])?;

        match (y, x) {
            (Some(y), Some(x)) => Ok(SqlValue::DoublePrecision(y.atan2(x))),
            _ => Ok(SqlValue::Null),
        }
    }

    /// Evaluate cot(x) - cotangent (PostgreSQL 18)
    fn evaluate_cot(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "COT requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                let tan_val = f.tan();
                if tan_val.abs() < f64::EPSILON {
                    return Err(ProtocolError::PostgresError(
                        "COT division by zero".to_string(),
                    ));
                }
                Ok(SqlValue::DoublePrecision(1.0 / tan_val))
            }
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate sinh(x) - hyperbolic sine (PostgreSQL 18)
    fn evaluate_sinh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SINH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.sinh())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate cosh(x) - hyperbolic cosine (PostgreSQL 18)
    fn evaluate_cosh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "COSH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.cosh())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate tanh(x) - hyperbolic tangent (PostgreSQL 18)
    fn evaluate_tanh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "TANH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.tanh())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate asinh(x) - inverse hyperbolic sine (PostgreSQL 18)
    fn evaluate_asinh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ASINH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => Ok(SqlValue::DoublePrecision(f.asinh())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate acosh(x) - inverse hyperbolic cosine (PostgreSQL 18)
    fn evaluate_acosh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ACOSH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                if f < 1.0 {
                    return Err(ProtocolError::PostgresError(
                        "ACOSH input must be >= 1".to_string(),
                    ));
                }
                Ok(SqlValue::DoublePrecision(f.acosh()))
            }
            None => Ok(SqlValue::Null),
        }
    }

    /// Evaluate atanh(x) - inverse hyperbolic tangent (PostgreSQL 18)
    fn evaluate_atanh(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ATANH requires exactly one argument".to_string(),
            ));
        }

        match Self::to_f64_static(&args[0])? {
            Some(f) => {
                if f.abs() >= 1.0 {
                    return Err(ProtocolError::PostgresError(
                        "ATANH input must be in range (-1, 1)".to_string(),
                    ));
                }
                Ok(SqlValue::DoublePrecision(f.atanh()))
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_sign(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SIGN requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Integer(i) => Ok(SqlValue::Integer(i.signum())),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(i.signum())),
            SqlValue::DoublePrecision(f) => {
                if f.is_nan() {
                    Ok(SqlValue::DoublePrecision(f64::NAN))
                } else if *f > 0.0 {
                    Ok(SqlValue::DoublePrecision(1.0))
                } else if *f < 0.0 {
                    Ok(SqlValue::DoublePrecision(-1.0))
                } else {
                    Ok(SqlValue::DoublePrecision(0.0))
                }
            }
            SqlValue::Real(f) => {
                if f.is_nan() {
                    Ok(SqlValue::Real(f32::NAN))
                } else if *f > 0.0 {
                    Ok(SqlValue::Real(1.0))
                } else if *f < 0.0 {
                    Ok(SqlValue::Real(-1.0))
                } else {
                    Ok(SqlValue::Real(0.0))
                }
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "SIGN requires numeric argument".to_string(),
            )),
        }
    }

    fn evaluate_trunc(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "TRUNC requires 1 or 2 arguments".to_string(),
            ));
        }

        let scale = if args.len() == 2 {
            match &args[1] {
                SqlValue::Integer(i) => *i,
                SqlValue::BigInt(i) => *i as i32,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "TRUNC scale must be an integer".to_string(),
                    ))
                }
            }
        } else {
            0
        };

        match &args[0] {
            SqlValue::DoublePrecision(f) => {
                let factor = 10_f64.powi(scale);
                Ok(SqlValue::DoublePrecision((f * factor).trunc() / factor))
            }
            SqlValue::Real(f) => {
                let factor = 10_f32.powi(scale);
                Ok(SqlValue::Real((f * factor).trunc() / factor))
            }
            SqlValue::Integer(i) => Ok(SqlValue::Integer(*i)),
            SqlValue::BigInt(i) => Ok(SqlValue::BigInt(*i)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "TRUNC requires numeric argument".to_string(),
            )),
        }
    }

    /// Helper to convert SqlValue to f64
    fn to_f64_static(value: &SqlValue) -> ProtocolResult<Option<f64>> {
        match value {
            SqlValue::SmallInt(i) => Ok(Some(*i as f64)),
            SqlValue::Integer(i) => Ok(Some(*i as f64)),
            SqlValue::BigInt(i) => Ok(Some(*i as f64)),
            SqlValue::Real(f) => Ok(Some(*f as f64)),
            SqlValue::DoublePrecision(f) => Ok(Some(*f)),
            SqlValue::Decimal(d) => Ok(d.to_string().parse().ok()),
            SqlValue::Null => Ok(None),
            _ => Err(ProtocolError::PostgresError(
                "Expected numeric argument".to_string(),
            )),
        }
    }

    // Additional function implementations would continue here...
    // Date, vector, and other specialized functions

    fn evaluate_now(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "NOW requires no arguments".to_string(),
            ));
        }

        Ok(SqlValue::TimestampWithTimezone(chrono::Utc::now()))
    }

    fn evaluate_current_date(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "CURRENT_DATE requires no arguments".to_string(),
            ));
        }

        Ok(SqlValue::Date(chrono::Utc::now().date_naive()))
    }

    fn evaluate_current_time(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "CURRENT_TIME requires no arguments".to_string(),
            ));
        }

        Ok(SqlValue::TimeWithTimezone(chrono::Utc::now()))
    }

    fn evaluate_current_timestamp(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "CURRENT_TIMESTAMP requires no arguments".to_string(),
            ));
        }

        Ok(SqlValue::TimestampWithTimezone(chrono::Utc::now()))
    }

    fn evaluate_vector_dims(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "VECTOR_DIMS requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Vector(v) | SqlValue::HalfVec(v) => Ok(SqlValue::Integer(v.len() as i32)),
            SqlValue::SparseVec(v) => Ok(SqlValue::Integer(v.len() as i32)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "VECTOR_DIMS requires vector argument".to_string(),
            )),
        }
    }

    fn evaluate_vector_norm(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "VECTOR_NORM requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Vector(v) | SqlValue::HalfVec(v) => {
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                Ok(SqlValue::Real(norm))
            }
            SqlValue::SparseVec(v) => {
                let norm = v.iter().map(|(_, val)| val * val).sum::<f32>().sqrt();
                Ok(SqlValue::Real(norm))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "VECTOR_NORM requires vector argument".to_string(),
            )),
        }
    }

    fn evaluate_concat(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            return Ok(SqlValue::Text(String::new()));
        }

        let mut result = String::new();
        for arg in args {
            if !arg.is_null() {
                result.push_str(&arg.to_postgres_string());
            }
        }

        Ok(SqlValue::Text(result))
    }

    fn evaluate_left(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "LEFT requires exactly two arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let n = Self::get_int_arg(&args[1])?;

        match (s, n) {
            (Some(s), Some(n)) => {
                let chars: Vec<char> = s.chars().collect();
                let len = if n >= 0 { n as usize } else { 0 };
                let result: String = chars.into_iter().take(len).collect();
                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_right(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "RIGHT requires exactly two arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let n = Self::get_int_arg(&args[1])?;

        match (s, n) {
            (Some(s), Some(n)) => {
                let chars: Vec<char> = s.chars().collect();
                let len = if n >= 0 { n as usize } else { 0 };
                let start = chars.len().saturating_sub(len);
                let result: String = chars.into_iter().skip(start).collect();
                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_lpad(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "LPAD requires 2 or 3 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let len = Self::get_int_arg(&args[1])?;
        let fill = if args.len() == 3 {
            Self::get_string_arg(&args[2])?
        } else {
            Some(" ".to_string())
        };

        match (s, len, fill) {
            (Some(s), Some(len), Some(fill)) => {
                let target_len = if len >= 0 { len as usize } else { 0 };
                let current_len = s.chars().count();

                if current_len >= target_len {
                    let result: String = s.chars().take(target_len).collect();
                    Ok(SqlValue::Text(result))
                } else if fill.is_empty() {
                    Ok(SqlValue::Text(s))
                } else {
                    let padding_needed = target_len - current_len;
                    let fill_chars: Vec<char> = fill.chars().collect();
                    let mut padding = String::new();
                    let mut i = 0;
                    while padding.chars().count() < padding_needed {
                        padding.push(fill_chars[i % fill_chars.len()]);
                        i += 1;
                    }
                    Ok(SqlValue::Text(format!("{}{}", padding, s)))
                }
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_rpad(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "RPAD requires 2 or 3 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let len = Self::get_int_arg(&args[1])?;
        let fill = if args.len() == 3 {
            Self::get_string_arg(&args[2])?
        } else {
            Some(" ".to_string())
        };

        match (s, len, fill) {
            (Some(s), Some(len), Some(fill)) => {
                let target_len = if len >= 0 { len as usize } else { 0 };
                let current_len = s.chars().count();

                if current_len >= target_len {
                    let result: String = s.chars().take(target_len).collect();
                    Ok(SqlValue::Text(result))
                } else if fill.is_empty() {
                    Ok(SqlValue::Text(s))
                } else {
                    let padding_needed = target_len - current_len;
                    let fill_chars: Vec<char> = fill.chars().collect();
                    let mut padding = String::new();
                    let mut i = 0;
                    while padding.chars().count() < padding_needed {
                        padding.push(fill_chars[i % fill_chars.len()]);
                        i += 1;
                    }
                    Ok(SqlValue::Text(format!("{}{}", s, padding)))
                }
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_reverse(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "REVERSE requires exactly one argument".to_string(),
            ));
        }

        match Self::get_string_arg(&args[0])? {
            Some(s) => Ok(SqlValue::Text(s.chars().rev().collect())),
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_split_part(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 3 {
            return Err(ProtocolError::PostgresError(
                "SPLIT_PART requires exactly three arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let delimiter = Self::get_string_arg(&args[1])?;
        let field = Self::get_int_arg(&args[2])?;

        match (s, delimiter, field) {
            (Some(s), Some(delimiter), Some(field)) => {
                if field <= 0 {
                    return Err(ProtocolError::PostgresError(
                        "SPLIT_PART field position must be positive".to_string(),
                    ));
                }

                let parts: Vec<&str> = if delimiter.is_empty() {
                    vec![&s[..]]
                } else {
                    s.split(&delimiter).collect()
                };

                let idx = (field - 1) as usize;
                if idx < parts.len() {
                    Ok(SqlValue::Text(parts[idx].to_string()))
                } else {
                    Ok(SqlValue::Text(String::new()))
                }
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_trim(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "TRIM requires 1 or 2 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let chars_to_trim = if args.len() == 2 {
            Self::get_string_arg(&args[1])?
        } else {
            Some(" \t\n\r".to_string())
        };

        match (s, chars_to_trim) {
            (Some(s), Some(chars)) => {
                let char_set: Vec<char> = chars.chars().collect();
                let result: String = s.trim_matches(|c| char_set.contains(&c)).to_string();
                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_ltrim(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "LTRIM requires 1 or 2 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let chars_to_trim = if args.len() == 2 {
            Self::get_string_arg(&args[1])?
        } else {
            Some(" \t\n\r".to_string())
        };

        match (s, chars_to_trim) {
            (Some(s), Some(chars)) => {
                let char_set: Vec<char> = chars.chars().collect();
                let result: String = s.trim_start_matches(|c| char_set.contains(&c)).to_string();
                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_rtrim(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::PostgresError(
                "RTRIM requires 1 or 2 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let chars_to_trim = if args.len() == 2 {
            Self::get_string_arg(&args[1])?
        } else {
            Some(" \t\n\r".to_string())
        };

        match (s, chars_to_trim) {
            (Some(s), Some(chars)) => {
                let char_set: Vec<char> = chars.chars().collect();
                let result: String = s.trim_end_matches(|c| char_set.contains(&c)).to_string();
                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_position(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "POSITION requires exactly two arguments".to_string(),
            ));
        }

        let substring = Self::get_string_arg(&args[0])?;
        let s = Self::get_string_arg(&args[1])?;

        match (substring, s) {
            (Some(substring), Some(s)) => {
                // POSITION returns 1-based index, 0 if not found
                match s.find(&substring) {
                    Some(pos) => {
                        // Convert byte position to char position (1-based)
                        let char_pos = s[..pos].chars().count() + 1;
                        Ok(SqlValue::Integer(char_pos as i32))
                    }
                    None => Ok(SqlValue::Integer(0)),
                }
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_initcap(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "INITCAP requires exactly one argument".to_string(),
            ));
        }

        match Self::get_string_arg(&args[0])? {
            Some(s) => {
                let mut result = String::new();
                let mut capitalize_next = true;

                for c in s.chars() {
                    if c.is_whitespace() || !c.is_alphanumeric() {
                        result.push(c);
                        capitalize_next = true;
                    } else if capitalize_next {
                        result.extend(c.to_uppercase());
                        capitalize_next = false;
                    } else {
                        result.extend(c.to_lowercase());
                    }
                }

                Ok(SqlValue::Text(result))
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_repeat(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "REPEAT requires exactly two arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let n = Self::get_int_arg(&args[1])?;

        match (s, n) {
            (Some(s), Some(n)) => {
                let count = if n >= 0 { n as usize } else { 0 };
                Ok(SqlValue::Text(s.repeat(count)))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_ascii(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ASCII requires exactly one argument".to_string(),
            ));
        }

        match Self::get_string_arg(&args[0])? {
            Some(s) => {
                if s.is_empty() {
                    Ok(SqlValue::Integer(0))
                } else {
                    Ok(SqlValue::Integer(s.chars().next().unwrap() as i32))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_chr(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "CHR requires exactly one argument".to_string(),
            ));
        }

        match Self::get_int_arg(&args[0])? {
            Some(n) => {
                if n < 0 || n > 0x10FFFF {
                    Err(ProtocolError::PostgresError(
                        "CHR argument out of valid Unicode range".to_string(),
                    ))
                } else {
                    match char::from_u32(n as u32) {
                        Some(c) => Ok(SqlValue::Text(c.to_string())),
                        None => Err(ProtocolError::PostgresError(
                            "CHR argument is not a valid Unicode code point".to_string(),
                        )),
                    }
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_md5(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "MD5 requires exactly one argument".to_string(),
            ));
        }

        match Self::get_string_arg(&args[0])? {
            Some(s) => {
                let digest = md5::compute(s.as_bytes());
                Ok(SqlValue::Text(format!("{:x}", digest)))
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_encode(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ENCODE requires exactly two arguments".to_string(),
            ));
        }

        let data = Self::get_string_arg(&args[0])?;
        let format = Self::get_string_arg(&args[1])?;

        match (data, format) {
            (Some(data), Some(format)) => {
                match format.to_lowercase().as_str() {
                    "base64" => {
                        use base64::{engine::general_purpose, Engine as _};
                        Ok(SqlValue::Text(
                            general_purpose::STANDARD.encode(data.as_bytes()),
                        ))
                    }
                    "hex" => Ok(SqlValue::Text(hex::encode(data.as_bytes()))),
                    "escape" => {
                        // Simple escape encoding
                        Ok(SqlValue::Text(data.escape_default().to_string()))
                    }
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Unknown encoding format: {}",
                        format
                    ))),
                }
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_decode(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "DECODE requires exactly two arguments".to_string(),
            ));
        }

        let data = Self::get_string_arg(&args[0])?;
        let format = Self::get_string_arg(&args[1])?;

        match (data, format) {
            (Some(data), Some(format)) => match format.to_lowercase().as_str() {
                "base64" => {
                    use base64::{engine::general_purpose, Engine as _};
                    match general_purpose::STANDARD.decode(&data) {
                        Ok(bytes) => match String::from_utf8(bytes) {
                            Ok(s) => Ok(SqlValue::Text(s)),
                            Err(_) => Ok(SqlValue::Bytea(
                                general_purpose::STANDARD.decode(&data).unwrap(),
                            )),
                        },
                        Err(e) => Err(ProtocolError::PostgresError(format!(
                            "Invalid base64 data: {}",
                            e
                        ))),
                    }
                }
                "hex" => match hex::decode(&data) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(s) => Ok(SqlValue::Text(s)),
                        Err(_) => Ok(SqlValue::Bytea(hex::decode(&data).unwrap())),
                    },
                    Err(e) => Err(ProtocolError::PostgresError(format!(
                        "Invalid hex data: {}",
                        e
                    ))),
                },
                _ => Err(ProtocolError::PostgresError(format!(
                    "Unknown decoding format: {}",
                    format
                ))),
            },
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_octet_length(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "OCTET_LENGTH requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(SqlValue::Integer(s.len() as i32))
            }
            SqlValue::Bytea(b) => Ok(SqlValue::Integer(b.len() as i32)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "OCTET_LENGTH requires string or bytea argument".to_string(),
            )),
        }
    }

    fn evaluate_bit_length(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "BIT_LENGTH requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(SqlValue::Integer((s.len() * 8) as i32))
            }
            SqlValue::Bytea(b) => Ok(SqlValue::Integer((b.len() * 8) as i32)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "BIT_LENGTH requires string or bytea argument".to_string(),
            )),
        }
    }

    fn evaluate_overlay(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::PostgresError(
                "OVERLAY requires 3 or 4 arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let replacement = Self::get_string_arg(&args[1])?;
        let start = Self::get_int_arg(&args[2])?;
        let len = if args.len() == 4 {
            Self::get_int_arg(&args[3])?
        } else {
            replacement.as_ref().map(|r| r.chars().count() as i32)
        };

        match (s, replacement, start, len) {
            (Some(s), Some(replacement), Some(start), Some(len)) => {
                let chars: Vec<char> = s.chars().collect();
                let start_idx = if start > 0 { (start - 1) as usize } else { 0 };
                let end_idx = (start_idx + len as usize).min(chars.len());

                let mut result = String::new();
                result.extend(chars.iter().take(start_idx));
                result.push_str(&replacement);
                result.extend(chars.iter().skip(end_idx));

                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_translate(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 3 {
            return Err(ProtocolError::PostgresError(
                "TRANSLATE requires exactly three arguments".to_string(),
            ));
        }

        let s = Self::get_string_arg(&args[0])?;
        let from = Self::get_string_arg(&args[1])?;
        let to = Self::get_string_arg(&args[2])?;

        match (s, from, to) {
            (Some(s), Some(from), Some(to)) => {
                let from_chars: Vec<char> = from.chars().collect();
                let to_chars: Vec<char> = to.chars().collect();

                let result: String = s
                    .chars()
                    .filter_map(|c| {
                        if let Some(pos) = from_chars.iter().position(|&fc| fc == c) {
                            if pos < to_chars.len() {
                                Some(to_chars[pos])
                            } else {
                                None // Remove character if no corresponding replacement
                            }
                        } else {
                            Some(c)
                        }
                    })
                    .collect();

                Ok(SqlValue::Text(result))
            }
            _ => Ok(SqlValue::Null),
        }
    }

    fn evaluate_quote_literal(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "QUOTE_LITERAL requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Null => Ok(SqlValue::Text("NULL".to_string())),
            _ => {
                let s = args[0].to_postgres_string();
                // Escape single quotes by doubling them
                let escaped = s.replace('\'', "''");
                Ok(SqlValue::Text(format!("'{}'", escaped)))
            }
        }
    }

    fn evaluate_quote_ident(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "QUOTE_IDENT requires exactly one argument".to_string(),
            ));
        }

        match Self::get_string_arg(&args[0])? {
            Some(s) => {
                // Check if quoting is needed
                let needs_quoting = s.is_empty()
                    || s.chars().next().map(|c| c.is_numeric()).unwrap_or(false)
                    || s.chars().any(|c| !c.is_alphanumeric() && c != '_')
                    || s.to_lowercase() != s;

                if needs_quoting {
                    // Escape double quotes by doubling them
                    let escaped = s.replace('"', "\"\"");
                    Ok(SqlValue::Text(format!("\"{}\"", escaped)))
                } else {
                    Ok(SqlValue::Text(s))
                }
            }
            None => Ok(SqlValue::Null),
        }
    }

    fn evaluate_format(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "FORMAT requires at least one argument".to_string(),
            ));
        }

        let format_str = Self::get_string_arg(&args[0])?;

        match format_str {
            Some(format_str) => {
                // Simple format implementation supporting %s, %I, %L
                let mut result = format_str.clone();
                let mut arg_idx = 1;

                // Process format specifiers
                let mut i = 0;
                while i < result.len() {
                    if result[i..].starts_with('%') && i + 1 < result.len() {
                        let spec = result.chars().nth(i + 1).unwrap();
                        match spec {
                            's' => {
                                if arg_idx < args.len() {
                                    let val = args[arg_idx].to_postgres_string();
                                    result = format!("{}{}{}", &result[..i], val, &result[i + 2..]);
                                    i += val.len();
                                    arg_idx += 1;
                                } else {
                                    i += 2;
                                }
                            }
                            'I' => {
                                if arg_idx < args.len() {
                                    let s = args[arg_idx].to_postgres_string();
                                    let escaped = s.replace('"', "\"\"");
                                    let val = format!("\"{}\"", escaped);
                                    result = format!("{}{}{}", &result[..i], val, &result[i + 2..]);
                                    i += val.len();
                                    arg_idx += 1;
                                } else {
                                    i += 2;
                                }
                            }
                            'L' => {
                                if arg_idx < args.len() {
                                    let s = args[arg_idx].to_postgres_string();
                                    let escaped = s.replace('\'', "''");
                                    let val = format!("'{}'", escaped);
                                    result = format!("{}{}{}", &result[..i], val, &result[i + 2..]);
                                    i += val.len();
                                    arg_idx += 1;
                                } else {
                                    i += 2;
                                }
                            }
                            '%' => {
                                result = format!("{}{}", &result[..i], &result[i + 1..]);
                                i += 1;
                            }
                            _ => {
                                i += 2;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }

                Ok(SqlValue::Text(result))
            }
            None => Ok(SqlValue::Null),
        }
    }

    /// Helper to extract string from SqlValue
    fn get_string_arg(value: &SqlValue) -> ProtocolResult<Option<String>> {
        match value {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Ok(Some(s.clone())),
            SqlValue::Null => Ok(None),
            _ => Ok(Some(value.to_postgres_string())),
        }
    }

    /// Helper to extract integer from SqlValue
    fn get_int_arg(value: &SqlValue) -> ProtocolResult<Option<i32>> {
        match value {
            SqlValue::SmallInt(i) => Ok(Some(*i as i32)),
            SqlValue::Integer(i) => Ok(Some(*i)),
            SqlValue::BigInt(i) => Ok(Some(*i as i32)),
            SqlValue::Null => Ok(None),
            _ => Err(ProtocolError::PostgresError(
                "Expected integer argument".to_string(),
            )),
        }
    }

    fn evaluate_year(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "YEAR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.year())),
            SqlValue::TimestampWithTimezone(ts) => {
                let date = ts.date_naive();
                Ok(SqlValue::Integer(date.year()))
            }
            SqlValue::Text(s) => {
                // Try to parse as date
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(SqlValue::Integer(date.year()))
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "YEAR requires date argument, got: {}",
                        s
                    )))
                }
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "YEAR requires date argument".to_string(),
            )),
        }
    }

    fn evaluate_month(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "MONTH requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.month() as i32)),
            SqlValue::TimestampWithTimezone(ts) => {
                let date = ts.date_naive();
                Ok(SqlValue::Integer(date.month() as i32))
            }
            SqlValue::Text(s) => {
                // Try to parse as date
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(SqlValue::Integer(date.month() as i32))
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "MONTH requires date argument, got: {}",
                        s
                    )))
                }
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "MONTH requires date argument".to_string(),
            )),
        }
    }

    fn evaluate_day(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "DAY requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.day() as i32)),
            SqlValue::TimestampWithTimezone(ts) => {
                let date = ts.date_naive();
                Ok(SqlValue::Integer(date.day() as i32))
            }
            SqlValue::Text(s) => {
                // Try to parse as date
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(SqlValue::Integer(date.day() as i32))
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "DAY requires date argument, got: {}",
                        s
                    )))
                }
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "DAY requires date argument".to_string(),
            )),
        }
    }

    /// EXTRACT(field FROM source) or DATE_PART(field, source)
    /// Extracts a component (year, month, day, hour, etc.) from a timestamp/date/time
    fn evaluate_extract(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "EXTRACT/DATE_PART requires exactly two arguments: field and source".to_string(),
            ));
        }

        // First arg is the field name (as text)
        let field = match &args[0] {
            SqlValue::Text(s) => s.to_uppercase(),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "EXTRACT field must be a string (year, month, day, etc.)".to_string(),
                ))
            }
        };

        let source = &args[1];

        // Handle NULL source
        if source.is_null() {
            return Ok(SqlValue::Null);
        }

        self.extract_field_from_value(&field, source)
    }

    /// Internal helper to extract a field from a date/time value
    fn extract_field_from_value(&self, field: &str, value: &SqlValue) -> ProtocolResult<SqlValue> {
        use chrono::{Datelike, Timelike};

        match value {
            SqlValue::Date(date) => match field {
                "YEAR" => Ok(SqlValue::Integer(date.year())),
                "MONTH" => Ok(SqlValue::Integer(date.month() as i32)),
                "DAY" => Ok(SqlValue::Integer(date.day() as i32)),
                "DOW" | "DAYOFWEEK" => {
                    // PostgreSQL: Sunday=0 to Saturday=6
                    Ok(SqlValue::Integer(
                        date.weekday().num_days_from_sunday() as i32
                    ))
                }
                "DOY" | "DAYOFYEAR" => Ok(SqlValue::Integer(date.ordinal() as i32)),
                "WEEK" | "ISOWEEK" => Ok(SqlValue::Integer(date.iso_week().week() as i32)),
                "QUARTER" => Ok(SqlValue::Integer(((date.month() - 1) / 3 + 1) as i32)),
                "EPOCH" => {
                    // Seconds since 1970-01-01
                    let datetime = date.and_hms_opt(0, 0, 0).unwrap_or_default();
                    Ok(SqlValue::DoublePrecision(
                        datetime.and_utc().timestamp() as f64
                    ))
                }
                _ => Err(ProtocolError::PostgresError(format!(
                    "Cannot extract '{}' from DATE",
                    field
                ))),
            },
            SqlValue::Time(time) => match field {
                "HOUR" => Ok(SqlValue::Integer(time.hour() as i32)),
                "MINUTE" => Ok(SqlValue::Integer(time.minute() as i32)),
                "SECOND" => Ok(SqlValue::DoublePrecision(
                    time.second() as f64 + time.nanosecond() as f64 / 1_000_000_000.0,
                )),
                "MILLISECOND" | "MILLISECONDS" => Ok(SqlValue::DoublePrecision(
                    time.second() as f64 * 1000.0 + time.nanosecond() as f64 / 1_000_000.0,
                )),
                "MICROSECOND" | "MICROSECONDS" => Ok(SqlValue::DoublePrecision(
                    time.second() as f64 * 1_000_000.0 + time.nanosecond() as f64 / 1_000.0,
                )),
                _ => Err(ProtocolError::PostgresError(format!(
                    "Cannot extract '{}' from TIME",
                    field
                ))),
            },
            SqlValue::TimeWithTimezone(ts) => {
                let time = ts.time();
                match field {
                    "HOUR" => Ok(SqlValue::Integer(time.hour() as i32)),
                    "MINUTE" => Ok(SqlValue::Integer(time.minute() as i32)),
                    "SECOND" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 + time.nanosecond() as f64 / 1_000_000_000.0,
                    )),
                    "TIMEZONE" | "TIMEZONE_HOUR" | "TIMEZONE_MINUTE" => {
                        // UTC timezone = 0
                        Ok(SqlValue::Integer(0))
                    }
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Cannot extract '{}' from TIME WITH TIMEZONE",
                        field
                    ))),
                }
            }
            SqlValue::Timestamp(ts) => {
                let date = ts.date();
                let time = ts.time();
                match field {
                    "YEAR" => Ok(SqlValue::Integer(date.year())),
                    "MONTH" => Ok(SqlValue::Integer(date.month() as i32)),
                    "DAY" => Ok(SqlValue::Integer(date.day() as i32)),
                    "HOUR" => Ok(SqlValue::Integer(time.hour() as i32)),
                    "MINUTE" => Ok(SqlValue::Integer(time.minute() as i32)),
                    "SECOND" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 + time.nanosecond() as f64 / 1_000_000_000.0,
                    )),
                    "MILLISECOND" | "MILLISECONDS" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 * 1000.0 + time.nanosecond() as f64 / 1_000_000.0,
                    )),
                    "MICROSECOND" | "MICROSECONDS" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 * 1_000_000.0 + time.nanosecond() as f64 / 1_000.0,
                    )),
                    "DOW" | "DAYOFWEEK" => Ok(SqlValue::Integer(
                        date.weekday().num_days_from_sunday() as i32,
                    )),
                    "DOY" | "DAYOFYEAR" => Ok(SqlValue::Integer(date.ordinal() as i32)),
                    "WEEK" | "ISOWEEK" => Ok(SqlValue::Integer(date.iso_week().week() as i32)),
                    "QUARTER" => Ok(SqlValue::Integer(((date.month() - 1) / 3 + 1) as i32)),
                    "EPOCH" => Ok(SqlValue::DoublePrecision(ts.and_utc().timestamp() as f64)),
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Cannot extract '{}' from TIMESTAMP",
                        field
                    ))),
                }
            }
            SqlValue::TimestampWithTimezone(ts) => {
                let date = ts.date_naive();
                let time = ts.time();
                match field {
                    "YEAR" => Ok(SqlValue::Integer(date.year())),
                    "MONTH" => Ok(SqlValue::Integer(date.month() as i32)),
                    "DAY" => Ok(SqlValue::Integer(date.day() as i32)),
                    "HOUR" => Ok(SqlValue::Integer(time.hour() as i32)),
                    "MINUTE" => Ok(SqlValue::Integer(time.minute() as i32)),
                    "SECOND" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 + time.nanosecond() as f64 / 1_000_000_000.0,
                    )),
                    "MILLISECOND" | "MILLISECONDS" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 * 1000.0 + time.nanosecond() as f64 / 1_000_000.0,
                    )),
                    "MICROSECOND" | "MICROSECONDS" => Ok(SqlValue::DoublePrecision(
                        time.second() as f64 * 1_000_000.0 + time.nanosecond() as f64 / 1_000.0,
                    )),
                    "DOW" | "DAYOFWEEK" => Ok(SqlValue::Integer(
                        date.weekday().num_days_from_sunday() as i32,
                    )),
                    "DOY" | "DAYOFYEAR" => Ok(SqlValue::Integer(date.ordinal() as i32)),
                    "WEEK" | "ISOWEEK" => Ok(SqlValue::Integer(date.iso_week().week() as i32)),
                    "QUARTER" => Ok(SqlValue::Integer(((date.month() - 1) / 3 + 1) as i32)),
                    "EPOCH" => Ok(SqlValue::DoublePrecision(ts.timestamp() as f64)),
                    "TIMEZONE" => Ok(SqlValue::Integer(0)), // UTC
                    "TIMEZONE_HOUR" => Ok(SqlValue::Integer(0)),
                    "TIMEZONE_MINUTE" => Ok(SqlValue::Integer(0)),
                    _ => Err(ProtocolError::PostgresError(format!(
                        "Cannot extract '{}' from TIMESTAMP WITH TIMEZONE",
                        field
                    ))),
                }
            }
            SqlValue::Interval(interval) => match field {
                "YEAR" => Ok(SqlValue::Integer(interval.months / 12)),
                "MONTH" => Ok(SqlValue::Integer(interval.months % 12)),
                "DAY" => Ok(SqlValue::Integer(interval.days)),
                "HOUR" => Ok(SqlValue::Integer(
                    (interval.microseconds / 3_600_000_000) as i32,
                )),
                "MINUTE" => Ok(SqlValue::Integer(
                    ((interval.microseconds / 60_000_000) % 60) as i32,
                )),
                "SECOND" => Ok(SqlValue::DoublePrecision(
                    (interval.microseconds % 60_000_000) as f64 / 1_000_000.0,
                )),
                "EPOCH" => {
                    // Total seconds in the interval
                    let total_seconds = (interval.months as f64 * 30.0 * 24.0 * 3600.0)
                        + (interval.days as f64 * 24.0 * 3600.0)
                        + (interval.microseconds as f64 / 1_000_000.0);
                    Ok(SqlValue::DoublePrecision(total_seconds))
                }
                _ => Err(ProtocolError::PostgresError(format!(
                    "Cannot extract '{}' from INTERVAL",
                    field
                ))),
            },
            SqlValue::Text(s) => {
                // Try to parse as timestamp or date
                if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
                    return self.extract_field_from_value(field, &SqlValue::Timestamp(ts));
                }
                if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                    return self.extract_field_from_value(field, &SqlValue::Timestamp(ts));
                }
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    return self.extract_field_from_value(field, &SqlValue::Date(date));
                }
                Err(ProtocolError::PostgresError(format!(
                    "Cannot parse '{}' as date/time for EXTRACT",
                    s
                )))
            }
            _ => Err(ProtocolError::PostgresError(format!(
                "EXTRACT requires date/time value, got: {:?}",
                value
            ))),
        }
    }

    /// DATE_TRUNC(field, source) - Truncate to specified precision
    fn evaluate_date_trunc(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "DATE_TRUNC requires exactly two arguments: precision and source".to_string(),
            ));
        }

        // First arg is the precision (as text)
        let precision = match &args[0] {
            SqlValue::Text(s) => s.to_uppercase(),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "DATE_TRUNC precision must be a string".to_string(),
                ))
            }
        };

        let source = &args[1];

        // Handle NULL source
        if source.is_null() {
            return Ok(SqlValue::Null);
        }

        self.truncate_to_precision(&precision, source)
    }

    /// Internal helper to truncate a date/time to a specified precision
    fn truncate_to_precision(&self, precision: &str, value: &SqlValue) -> ProtocolResult<SqlValue> {
        use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

        match value {
            SqlValue::Date(date) => {
                let truncated = match precision {
                    "MILLENNIUM" => {
                        let millennium = (date.year() - 1) / 1000 * 1000 + 1;
                        NaiveDate::from_ymd_opt(millennium, 1, 1).unwrap_or(*date)
                    }
                    "CENTURY" => {
                        let century = (date.year() - 1) / 100 * 100 + 1;
                        NaiveDate::from_ymd_opt(century, 1, 1).unwrap_or(*date)
                    }
                    "DECADE" => {
                        let decade = date.year() / 10 * 10;
                        NaiveDate::from_ymd_opt(decade, 1, 1).unwrap_or(*date)
                    }
                    "YEAR" => NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap_or(*date),
                    "QUARTER" => {
                        let quarter_start = ((date.month() - 1) / 3) * 3 + 1;
                        NaiveDate::from_ymd_opt(date.year(), quarter_start, 1).unwrap_or(*date)
                    }
                    "MONTH" => {
                        NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(*date)
                    }
                    "WEEK" => {
                        let days_from_monday = date.weekday().num_days_from_monday();
                        *date - chrono::Duration::days(days_from_monday as i64)
                    }
                    "DAY" => *date,
                    _ => {
                        return Err(ProtocolError::PostgresError(format!(
                            "Invalid DATE_TRUNC precision '{}' for DATE",
                            precision
                        )))
                    }
                };
                Ok(SqlValue::Date(truncated))
            }
            SqlValue::Timestamp(ts) => {
                let date = ts.date();
                let time = ts.time();
                let truncated = match precision {
                    "MILLENNIUM" => {
                        let millennium = (date.year() - 1) / 1000 * 1000 + 1;
                        NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(millennium, 1, 1).unwrap_or(date),
                            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                        )
                    }
                    "CENTURY" => {
                        let century = (date.year() - 1) / 100 * 100 + 1;
                        NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(century, 1, 1).unwrap_or(date),
                            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                        )
                    }
                    "DECADE" => {
                        let decade = date.year() / 10 * 10;
                        NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(decade, 1, 1).unwrap_or(date),
                            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                        )
                    }
                    "YEAR" => NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap_or(date),
                        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    ),
                    "QUARTER" => {
                        let quarter_start = ((date.month() - 1) / 3) * 3 + 1;
                        NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(date.year(), quarter_start, 1).unwrap_or(date),
                            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                        )
                    }
                    "MONTH" => NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date),
                        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    ),
                    "WEEK" => {
                        let days_from_monday = date.weekday().num_days_from_monday();
                        let week_start = date - chrono::Duration::days(days_from_monday as i64);
                        NaiveDateTime::new(week_start, NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    }
                    "DAY" => NaiveDateTime::new(date, NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
                    "HOUR" => NaiveDateTime::new(
                        date,
                        NaiveTime::from_hms_opt(time.hour(), 0, 0).unwrap(),
                    ),
                    "MINUTE" => NaiveDateTime::new(
                        date,
                        NaiveTime::from_hms_opt(time.hour(), time.minute(), 0).unwrap(),
                    ),
                    "SECOND" => NaiveDateTime::new(
                        date,
                        NaiveTime::from_hms_opt(time.hour(), time.minute(), time.second()).unwrap(),
                    ),
                    "MILLISECOND" | "MILLISECONDS" => {
                        let ms = (time.nanosecond() / 1_000_000) * 1_000_000;
                        NaiveDateTime::new(
                            date,
                            NaiveTime::from_hms_nano_opt(
                                time.hour(),
                                time.minute(),
                                time.second(),
                                ms,
                            )
                            .unwrap_or(time),
                        )
                    }
                    "MICROSECOND" | "MICROSECONDS" => {
                        let us = (time.nanosecond() / 1_000) * 1_000;
                        NaiveDateTime::new(
                            date,
                            NaiveTime::from_hms_nano_opt(
                                time.hour(),
                                time.minute(),
                                time.second(),
                                us,
                            )
                            .unwrap_or(time),
                        )
                    }
                    _ => {
                        return Err(ProtocolError::PostgresError(format!(
                            "Invalid DATE_TRUNC precision '{}' for TIMESTAMP",
                            precision
                        )))
                    }
                };
                Ok(SqlValue::Timestamp(truncated))
            }
            SqlValue::TimestampWithTimezone(ts) => {
                let naive = ts.naive_utc();
                let truncated_naive =
                    self.truncate_to_precision(precision, &SqlValue::Timestamp(naive))?;
                if let SqlValue::Timestamp(t) = truncated_naive {
                    Ok(SqlValue::TimestampWithTimezone(
                        chrono::DateTime::from_naive_utc_and_offset(t, chrono::Utc),
                    ))
                } else {
                    Err(ProtocolError::PostgresError(
                        "Unexpected result from timestamp truncation".to_string(),
                    ))
                }
            }
            SqlValue::Text(s) => {
                // Try to parse as timestamp or date
                if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
                    return self.truncate_to_precision(precision, &SqlValue::Timestamp(ts));
                }
                if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                    return self.truncate_to_precision(precision, &SqlValue::Timestamp(ts));
                }
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    return self.truncate_to_precision(precision, &SqlValue::Date(date));
                }
                Err(ProtocolError::PostgresError(format!(
                    "Cannot parse '{}' as date/time for DATE_TRUNC",
                    s
                )))
            }
            _ => Err(ProtocolError::PostgresError(format!(
                "DATE_TRUNC requires date/time value, got: {:?}",
                value
            ))),
        }
    }

    fn evaluate_hour(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "HOUR requires exactly one argument".to_string(),
            ));
        }

        use chrono::Timelike;
        match &args[0] {
            SqlValue::Time(time) => Ok(SqlValue::Integer(time.hour() as i32)),
            SqlValue::TimeWithTimezone(ts) => Ok(SqlValue::Integer(ts.time().hour() as i32)),
            SqlValue::Timestamp(ts) => Ok(SqlValue::Integer(ts.time().hour() as i32)),
            SqlValue::TimestampWithTimezone(ts) => Ok(SqlValue::Integer(ts.time().hour() as i32)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "HOUR requires time/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_minute(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "MINUTE requires exactly one argument".to_string(),
            ));
        }

        use chrono::Timelike;
        match &args[0] {
            SqlValue::Time(time) => Ok(SqlValue::Integer(time.minute() as i32)),
            SqlValue::TimeWithTimezone(ts) => Ok(SqlValue::Integer(ts.time().minute() as i32)),
            SqlValue::Timestamp(ts) => Ok(SqlValue::Integer(ts.time().minute() as i32)),
            SqlValue::TimestampWithTimezone(ts) => Ok(SqlValue::Integer(ts.time().minute() as i32)),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "MINUTE requires time/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_second(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "SECOND requires exactly one argument".to_string(),
            ));
        }

        use chrono::Timelike;
        match &args[0] {
            SqlValue::Time(time) => Ok(SqlValue::DoublePrecision(
                time.second() as f64 + time.nanosecond() as f64 / 1_000_000_000.0,
            )),
            SqlValue::TimeWithTimezone(ts) => Ok(SqlValue::DoublePrecision(
                ts.time().second() as f64 + ts.time().nanosecond() as f64 / 1_000_000_000.0,
            )),
            SqlValue::Timestamp(ts) => Ok(SqlValue::DoublePrecision(
                ts.time().second() as f64 + ts.time().nanosecond() as f64 / 1_000_000_000.0,
            )),
            SqlValue::TimestampWithTimezone(ts) => Ok(SqlValue::DoublePrecision(
                ts.time().second() as f64 + ts.time().nanosecond() as f64 / 1_000_000_000.0,
            )),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "SECOND requires time/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_week(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "WEEK requires exactly one argument".to_string(),
            ));
        }

        use chrono::Datelike;
        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.iso_week().week() as i32)),
            SqlValue::Timestamp(ts) => Ok(SqlValue::Integer(ts.date().iso_week().week() as i32)),
            SqlValue::TimestampWithTimezone(ts) => {
                Ok(SqlValue::Integer(ts.date_naive().iso_week().week() as i32))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "WEEK requires date/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_quarter(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "QUARTER requires exactly one argument".to_string(),
            ));
        }

        use chrono::Datelike;
        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(((date.month() - 1) / 3 + 1) as i32)),
            SqlValue::Timestamp(ts) => {
                Ok(SqlValue::Integer(((ts.date().month() - 1) / 3 + 1) as i32))
            }
            SqlValue::TimestampWithTimezone(ts) => Ok(SqlValue::Integer(
                ((ts.date_naive().month() - 1) / 3 + 1) as i32,
            )),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "QUARTER requires date/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_day_of_week(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "DAYOFWEEK requires exactly one argument".to_string(),
            ));
        }

        use chrono::Datelike;
        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(
                date.weekday().num_days_from_sunday() as i32
            )),
            SqlValue::Timestamp(ts) => Ok(SqlValue::Integer(
                ts.date().weekday().num_days_from_sunday() as i32,
            )),
            SqlValue::TimestampWithTimezone(ts) => Ok(SqlValue::Integer(
                ts.date_naive().weekday().num_days_from_sunday() as i32,
            )),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "DAYOFWEEK requires date/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_day_of_year(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "DAYOFYEAR requires exactly one argument".to_string(),
            ));
        }

        use chrono::Datelike;
        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.ordinal() as i32)),
            SqlValue::Timestamp(ts) => Ok(SqlValue::Integer(ts.date().ordinal() as i32)),
            SqlValue::TimestampWithTimezone(ts) => {
                Ok(SqlValue::Integer(ts.date_naive().ordinal() as i32))
            }
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "DAYOFYEAR requires date/timestamp argument".to_string(),
            )),
        }
    }

    fn evaluate_coalesce(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "COALESCE requires at least one argument".to_string(),
            ));
        }

        for arg in args {
            if !arg.is_null() {
                return Ok(arg.clone());
            }
        }

        Ok(SqlValue::Null)
    }

    fn evaluate_nullif(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "NULLIF requires exactly two arguments".to_string(),
            ));
        }

        if self.compare_values(&args[0], &args[1])? == Ordering::Equal {
            Ok(SqlValue::Null)
        } else {
            Ok(args[0].clone())
        }
    }

    fn evaluate_greatest(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "GREATEST requires at least one argument".to_string(),
            ));
        }

        let mut result = &args[0];
        for arg in &args[1..] {
            if self.compare_values(arg, result)? == Ordering::Greater {
                result = arg;
            }
        }

        Ok(result.clone())
    }

    fn evaluate_least(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "LEAST requires at least one argument".to_string(),
            ));
        }

        let mut result = &args[0];
        for arg in &args[1..] {
            if self.compare_values(arg, result)? == Ordering::Less {
                result = arg;
            }
        }

        Ok(result.clone())
    }

    // Placeholder implementations for complex evaluations that need more context
    fn evaluate_subquery(
        &self,
        _stmt: &SelectStatement,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement subquery evaluation
        Err(ProtocolError::PostgresError(
            "Subquery evaluation not implemented".to_string(),
        ))
    }

    fn evaluate_exists(
        &self,
        _stmt: &SelectStatement,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement EXISTS evaluation
        Err(ProtocolError::PostgresError(
            "EXISTS evaluation not implemented".to_string(),
        ))
    }

    fn evaluate_in_expression(
        &mut self,
        expr: &Expression,
        list: &InList,
        negated: bool,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(expr, context)?;

        // NULL IN (...) always returns NULL
        if value.is_null() {
            return Ok(SqlValue::Null);
        }

        let result = match list {
            InList::Expressions(expressions) => {
                let mut found = false;
                let mut has_null = false;

                for list_expr in expressions {
                    let list_value = self.evaluate(list_expr, context)?;

                    if list_value.is_null() {
                        has_null = true;
                        continue;
                    }

                    if self.compare_values(&value, &list_value)? == Ordering::Equal {
                        found = true;
                        break;
                    }
                }

                if found {
                    SqlValue::Boolean(true)
                } else if has_null {
                    // If not found but there were NULLs, result is NULL (unknown)
                    SqlValue::Null
                } else {
                    SqlValue::Boolean(false)
                }
            }
            InList::Subquery(select_stmt) => {
                // For subquery IN, we need to execute the subquery
                // This is a simplified implementation that uses the subquery evaluation
                let subquery_result = self.evaluate_subquery(select_stmt, context)?;

                match subquery_result {
                    SqlValue::Array(values) => {
                        let mut found = false;
                        let mut has_null = false;

                        for list_value in &values {
                            if list_value.is_null() {
                                has_null = true;
                                continue;
                            }

                            if self.compare_values(&value, list_value)? == Ordering::Equal {
                                found = true;
                                break;
                            }
                        }

                        if found {
                            SqlValue::Boolean(true)
                        } else if has_null {
                            SqlValue::Null
                        } else {
                            SqlValue::Boolean(false)
                        }
                    }
                    _ => {
                        // Single value result
                        if subquery_result.is_null() {
                            SqlValue::Null
                        } else if self.compare_values(&value, &subquery_result)? == Ordering::Equal
                        {
                            SqlValue::Boolean(true)
                        } else {
                            SqlValue::Boolean(false)
                        }
                    }
                }
            }
        };

        // Apply negation if needed
        match result {
            SqlValue::Boolean(b) => Ok(SqlValue::Boolean(if negated { !b } else { b })),
            SqlValue::Null => Ok(SqlValue::Null),
            _ => Ok(result),
        }
    }

    fn evaluate_between_expression(
        &mut self,
        expr: &Expression,
        low: &Expression,
        high: &Expression,
        negated: bool,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(expr, context)?;
        let low_value = self.evaluate(low, context)?;
        let high_value = self.evaluate(high, context)?;

        // NULL handling: if any value is NULL, result is NULL
        if value.is_null() || low_value.is_null() || high_value.is_null() {
            return Ok(SqlValue::Null);
        }

        // BETWEEN is inclusive: value >= low AND value <= high
        let cmp_low = self.compare_values(&value, &low_value)?;
        let cmp_high = self.compare_values(&value, &high_value)?;

        let in_range = matches!(cmp_low, Ordering::Greater | Ordering::Equal)
            && matches!(cmp_high, Ordering::Less | Ordering::Equal);

        Ok(SqlValue::Boolean(if negated {
            !in_range
        } else {
            in_range
        }))
    }

    fn evaluate_like_expression(
        &mut self,
        expr: &Expression,
        pattern: &Expression,
        escape: Option<&Expression>,
        case_insensitive: bool,
        negated: bool,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(expr, context)?;
        let pattern_value = self.evaluate(pattern, context)?;

        // NULL handling
        if value.is_null() || pattern_value.is_null() {
            return Ok(SqlValue::Null);
        }

        let text = match &value {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            _ => value.to_postgres_string(),
        };

        let pattern_str = match &pattern_value {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s.clone(),
            _ => pattern_value.to_postgres_string(),
        };

        // Get escape character (default is backslash, but can be customized)
        let escape_char = if let Some(escape_expr) = escape {
            let escape_value = self.evaluate(escape_expr, context)?;
            match &escape_value {
                SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                    s.chars().next().unwrap_or('\\')
                }
                SqlValue::Null => return Ok(SqlValue::Null),
                _ => '\\',
            }
        } else {
            '\\'
        };

        // Convert SQL LIKE pattern to regex pattern
        let regex_pattern = self.like_pattern_to_regex(&pattern_str, escape_char, case_insensitive);

        // Perform the match
        let matches = self.match_like_pattern(&text, &regex_pattern, case_insensitive);

        Ok(SqlValue::Boolean(if negated { !matches } else { matches }))
    }

    /// Convert SQL LIKE pattern to a matching pattern
    /// % matches any sequence of characters (including empty)
    /// _ matches any single character
    fn like_pattern_to_regex(
        &self,
        pattern: &str,
        escape_char: char,
        _case_insensitive: bool,
    ) -> String {
        let mut result = String::new();
        let chars = pattern.chars().peekable();
        let mut escaped = false;

        for c in chars {
            if escaped {
                // Previous character was escape, so this character is literal
                result.push(c);
                escaped = false;
            } else if c == escape_char {
                // Escape character - next character is literal
                escaped = true;
            } else if c == '%' {
                // Match any sequence of characters
                result.push_str(".*");
            } else if c == '_' {
                // Match any single character
                result.push('.');
            } else if "\\^$.|?*+()[]{}".contains(c) {
                // Escape regex special characters
                result.push('\\');
                result.push(c);
            } else {
                result.push(c);
            }
        }

        // If the last character was an escape but nothing followed, add it literally
        if escaped {
            result.push(escape_char);
        }

        result
    }

    /// Perform LIKE pattern matching
    fn match_like_pattern(&self, text: &str, pattern: &str, case_insensitive: bool) -> bool {
        let text_to_match = if case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        let pattern_to_match = if case_insensitive {
            pattern.to_lowercase()
        } else {
            pattern.to_string()
        };

        // Build full regex pattern with anchors
        let full_pattern = format!("^{}$", pattern_to_match);

        // Use simple pattern matching for common cases to avoid regex overhead
        if !pattern_to_match.contains(".*") && !pattern_to_match.contains('.') {
            // No wildcards, exact match
            return text_to_match == pattern_to_match;
        }

        // Try regex matching
        match regex::Regex::new(&full_pattern) {
            Ok(re) => re.is_match(&text_to_match),
            Err(_) => {
                // Fallback to simple matching if regex fails
                self.simple_like_match(&text_to_match, &pattern_to_match)
            }
        }
    }

    /// Simple LIKE matching fallback without regex
    fn simple_like_match(&self, text: &str, pattern: &str) -> bool {
        let text_chars = text.chars().peekable();
        let pattern_chars = pattern.chars().peekable();

        self.like_match_recursive(
            &text_chars.collect::<Vec<_>>(),
            &pattern_chars.collect::<Vec<_>>(),
            0,
            0,
        )
    }

    #[allow(clippy::only_used_in_recursion)]
    fn like_match_recursive(&self, text: &[char], pattern: &[char], ti: usize, pi: usize) -> bool {
        // Base cases
        if pi >= pattern.len() {
            return ti >= text.len();
        }

        // Check for .* (% wildcard converted to regex)
        if pi + 1 < pattern.len() && pattern[pi] == '.' && pattern[pi + 1] == '*' {
            // Try matching zero or more characters
            for i in ti..=text.len() {
                if self.like_match_recursive(text, pattern, i, pi + 2) {
                    return true;
                }
            }
            return false;
        }

        // Check for . (single character wildcard)
        if pattern[pi] == '.' {
            if ti < text.len() {
                return self.like_match_recursive(text, pattern, ti + 1, pi + 1);
            }
            return false;
        }

        // Regular character match
        if ti < text.len()
            && (pattern[pi] == text[ti]
                || pattern[pi] == '\\' && pi + 1 < pattern.len() && pattern[pi + 1] == text[ti])
        {
            if pattern[pi] == '\\' {
                return self.like_match_recursive(text, pattern, ti + 1, pi + 2);
            }
            return self.like_match_recursive(text, pattern, ti + 1, pi + 1);
        }

        false
    }

    fn evaluate_is_null(
        &mut self,
        expr: &Expression,
        negated: bool,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(expr, context)?;
        let is_null = value.is_null();
        Ok(SqlValue::Boolean(if negated { !is_null } else { is_null }))
    }

    fn evaluate_cast(
        &mut self,
        expr: &Expression,
        target_type: &SqlType,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let value = self.evaluate(expr, context)?;
        value
            .cast_to(target_type)
            .map_err(ProtocolError::PostgresError)
    }

    fn evaluate_array_constructor(
        &mut self,
        elements: &[Expression],
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let mut array_values = Vec::new();
        for element in elements {
            array_values.push(self.evaluate(element, context)?);
        }
        Ok(SqlValue::Array(array_values))
    }

    fn evaluate_row_constructor(
        &mut self,
        elements: &[Expression],
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let mut row_values = Vec::new();
        for element in elements {
            row_values.push(self.evaluate(element, context)?);
        }
        Ok(SqlValue::Array(row_values)) // Row constructor returns array-like structure
    }

    fn evaluate_array_index(
        &mut self,
        array: &Expression,
        index: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let array_value = self.evaluate(array, context)?;
        let index_value = self.evaluate(index, context)?;

        // NULL handling
        if array_value.is_null() || index_value.is_null() {
            return Ok(SqlValue::Null);
        }

        let elements = match &array_value {
            SqlValue::Array(arr) => arr,
            _ => {
                return Err(ProtocolError::PostgresError(
                    "Cannot index non-array value".to_string(),
                ))
            }
        };

        // PostgreSQL arrays are 1-indexed
        let index = match &index_value {
            SqlValue::Integer(i) => *i as i64,
            SqlValue::BigInt(i) => *i,
            SqlValue::SmallInt(i) => *i as i64,
            _ => {
                return Err(ProtocolError::PostgresError(
                    "Array index must be an integer".to_string(),
                ))
            }
        };

        // Convert 1-based index to 0-based
        if index < 1 {
            return Ok(SqlValue::Null); // PostgreSQL returns NULL for out-of-bounds
        }

        let zero_based_index = (index - 1) as usize;
        if zero_based_index >= elements.len() {
            return Ok(SqlValue::Null); // PostgreSQL returns NULL for out-of-bounds
        }

        Ok(elements[zero_based_index].clone())
    }

    fn evaluate_array_slice(
        &mut self,
        array: &Expression,
        start: Option<&Expression>,
        end: Option<&Expression>,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let array_value = self.evaluate(array, context)?;

        // NULL array handling
        if array_value.is_null() {
            return Ok(SqlValue::Null);
        }

        let elements = match &array_value {
            SqlValue::Array(arr) => arr,
            _ => {
                return Err(ProtocolError::PostgresError(
                    "Cannot slice non-array value".to_string(),
                ))
            }
        };

        // Evaluate start and end indices (PostgreSQL arrays are 1-indexed)
        let start_idx = if let Some(start_expr) = start {
            let start_value = self.evaluate(start_expr, context)?;
            if start_value.is_null() {
                return Ok(SqlValue::Null);
            }
            match &start_value {
                SqlValue::Integer(i) => std::cmp::max(1, *i) as usize,
                SqlValue::BigInt(i) => std::cmp::max(1, *i) as usize,
                SqlValue::SmallInt(i) => std::cmp::max(1, *i as i32) as usize,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "Array slice index must be an integer".to_string(),
                    ))
                }
            }
        } else {
            1 // Default to first element
        };

        let end_idx = if let Some(end_expr) = end {
            let end_value = self.evaluate(end_expr, context)?;
            if end_value.is_null() {
                return Ok(SqlValue::Null);
            }
            match &end_value {
                SqlValue::Integer(i) => *i as usize,
                SqlValue::BigInt(i) => *i as usize,
                SqlValue::SmallInt(i) => *i as usize,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "Array slice index must be an integer".to_string(),
                    ))
                }
            }
        } else {
            elements.len() // Default to last element
        };

        // Convert from 1-based to 0-based and perform slice
        let zero_start = start_idx.saturating_sub(1);
        let zero_end = std::cmp::min(end_idx, elements.len());

        if zero_start >= elements.len() || zero_start >= zero_end {
            return Ok(SqlValue::Array(Vec::new())); // Empty array
        }

        let sliced: Vec<SqlValue> = elements[zero_start..zero_end].to_vec();
        Ok(SqlValue::Array(sliced))
    }

    fn evaluate_vector_similarity(
        &mut self,
        left: &Expression,
        operator: &VectorOperator,
        right: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let left_val = self.evaluate(left, context)?;
        let right_val = self.evaluate(right, context)?;
        self.vector_distance(&left_val, &right_val, operator.clone())
    }

    fn vector_distance(
        &self,
        left: &SqlValue,
        right: &SqlValue,
        operator: VectorOperator,
    ) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Vector(a), SqlValue::Vector(b)) => {
                if a.len() != b.len() {
                    return Err(ProtocolError::PostgresError(
                        "Vector dimensions do not match".to_string(),
                    ));
                }

                let distance = match operator {
                    VectorOperator::L2Distance => {
                        let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
                        sum.sqrt()
                    }
                    VectorOperator::InnerProduct => {
                        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
                    }
                    VectorOperator::CosineDistance => {
                        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
                        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

                        if norm_a == 0.0 || norm_b == 0.0 {
                            return Err(ProtocolError::PostgresError(
                                "Cannot compute cosine distance with zero vector".to_string(),
                            ));
                        }

                        1.0 - (dot_product / (norm_a * norm_b))
                    }
                    VectorOperator::L1Distance => {
                        a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
                    }
                    VectorOperator::HammingDistance => {
                        // Convert to binary and count differences
                        let count: f32 = a
                            .iter()
                            .zip(b.iter())
                            .map(|(x, y)| if (x > &0.5) != (y > &0.5) { 1.0 } else { 0.0 })
                            .sum();
                        count
                    }
                };

                Ok(SqlValue::Real(distance))
            }
            _ => Err(ProtocolError::PostgresError(
                "Vector operations require vector operands".to_string(),
            )),
        }
    }

    // Window function helper methods

    /// Evaluate RANK() or DENSE_RANK() window function
    /// RANK() returns rank with gaps (1, 1, 3 for ties)
    /// DENSE_RANK() returns rank without gaps (1, 1, 2 for ties)
    fn evaluate_rank(
        &mut self,
        order_by: &[OrderByItem],
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
        dense: bool,
    ) -> ProtocolResult<SqlValue> {
        let partition_rows = &window_context.partition_rows;
        let current_idx = window_context.current_row_index;

        if partition_rows.is_empty() {
            return Ok(SqlValue::BigInt(1));
        }

        // Find position of current row in partition
        let pos_in_partition = partition_rows
            .iter()
            .position(|&idx| idx == current_idx)
            .unwrap_or(0);

        if order_by.is_empty() {
            // No ORDER BY means all rows have rank 1
            return Ok(SqlValue::BigInt(1));
        }

        // Get current row's order values
        let _current_row = &window_context.all_rows[current_idx];

        // Count rows with smaller order values
        let mut rank = 1i64;
        let mut distinct_ranks = 1i64;
        let mut last_order_values: Option<Vec<SqlValue>> = None;

        for (i, &row_idx) in partition_rows.iter().enumerate() {
            if i >= pos_in_partition {
                break;
            }

            let row = &window_context.all_rows[row_idx];

            // Evaluate order expressions for this row
            let mut row_context = context.clone();
            row_context.current_row = row.clone();

            let mut order_values = Vec::new();
            for item in order_by {
                let val = self.evaluate(&item.expression, &row_context)?;
                order_values.push(val);
            }

            // Check if values changed from last
            let values_changed = match &last_order_values {
                None => true,
                Some(last) => order_values.iter().zip(last.iter()).any(|(a, b)| {
                    self.compare_values(a, b).unwrap_or(Ordering::Equal) != Ordering::Equal
                }),
            };

            if values_changed {
                distinct_ranks += 1;
            }
            rank = i as i64 + 2; // Position + 1 (1-indexed)
            last_order_values = Some(order_values);
        }

        // For dense rank, use distinct_ranks; for regular rank, use position
        if pos_in_partition == 0 {
            Ok(SqlValue::BigInt(1))
        } else if dense {
            // Check if current row ties with previous
            let prev_row = &window_context.all_rows[partition_rows[pos_in_partition - 1]];
            let mut prev_context = context.clone();
            prev_context.current_row = prev_row.clone();

            let mut current_order_values = Vec::new();
            let mut prev_order_values = Vec::new();

            for item in order_by {
                current_order_values.push(self.evaluate(&item.expression, context)?);
                prev_order_values.push(self.evaluate(&item.expression, &prev_context)?);
            }

            let ties = current_order_values
                .iter()
                .zip(prev_order_values.iter())
                .all(|(a, b)| {
                    self.compare_values(a, b).unwrap_or(Ordering::Equal) == Ordering::Equal
                });

            if ties {
                Ok(SqlValue::BigInt(distinct_ranks))
            } else {
                Ok(SqlValue::BigInt(distinct_ranks + 1))
            }
        } else {
            // Check if current row ties with previous
            let prev_row = &window_context.all_rows[partition_rows[pos_in_partition - 1]];
            let mut prev_context = context.clone();
            prev_context.current_row = prev_row.clone();

            let mut current_order_values = Vec::new();
            let mut prev_order_values = Vec::new();

            for item in order_by {
                current_order_values.push(self.evaluate(&item.expression, context)?);
                prev_order_values.push(self.evaluate(&item.expression, &prev_context)?);
            }

            let ties = current_order_values
                .iter()
                .zip(prev_order_values.iter())
                .all(|(a, b)| {
                    self.compare_values(a, b).unwrap_or(Ordering::Equal) == Ordering::Equal
                });

            if ties {
                // Find the rank of the tied group
                let mut tie_rank = pos_in_partition as i64 + 1;
                for i in (0..pos_in_partition).rev() {
                    let check_row = &window_context.all_rows[partition_rows[i]];
                    let mut check_context = context.clone();
                    check_context.current_row = check_row.clone();

                    let mut check_order_values = Vec::new();
                    for item in order_by {
                        check_order_values.push(self.evaluate(&item.expression, &check_context)?);
                    }

                    let still_ties = check_order_values
                        .iter()
                        .zip(current_order_values.iter())
                        .all(|(a, b)| {
                            self.compare_values(a, b).unwrap_or(Ordering::Equal) == Ordering::Equal
                        });

                    if still_ties {
                        tie_rank = i as i64 + 1;
                    } else {
                        break;
                    }
                }
                Ok(SqlValue::BigInt(tie_rank))
            } else {
                Ok(SqlValue::BigInt(rank))
            }
        }
    }

    /// Evaluate LAG() or LEAD() window function
    /// LAG(expr, offset, default) returns value from row at offset rows before current
    /// LEAD(expr, offset, default) returns value from row at offset rows after current
    fn evaluate_lag_lead(
        &mut self,
        expr: &Expression,
        offset: Option<&Expression>,
        default: Option<&Expression>,
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
        is_lag: bool,
    ) -> ProtocolResult<SqlValue> {
        // Evaluate offset (default is 1)
        let offset_value = if let Some(offset_expr) = offset {
            let val = self.evaluate(offset_expr, context)?;
            match val {
                SqlValue::Integer(i) => i as i64,
                SqlValue::BigInt(i) => i,
                SqlValue::SmallInt(i) => i as i64,
                SqlValue::Null => return Ok(SqlValue::Null),
                _ => 1,
            }
        } else {
            1
        };

        // Evaluate default value
        let default_value = if let Some(default_expr) = default {
            self.evaluate(default_expr, context)?
        } else {
            SqlValue::Null
        };

        let partition_rows = &window_context.partition_rows;
        let current_idx = window_context.current_row_index;

        // Find position in partition
        let pos_in_partition = partition_rows
            .iter()
            .position(|&idx| idx == current_idx)
            .unwrap_or(0);

        // Calculate target position
        let target_pos = if is_lag {
            pos_in_partition as i64 - offset_value
        } else {
            pos_in_partition as i64 + offset_value
        };

        // Check bounds
        if target_pos < 0 || target_pos >= partition_rows.len() as i64 {
            return Ok(default_value);
        }

        // Get the target row
        let target_row_idx = partition_rows[target_pos as usize];
        let target_row = &window_context.all_rows[target_row_idx];

        // Evaluate expression in context of target row
        let mut target_context = context.clone();
        target_context.current_row = target_row.clone();

        self.evaluate(expr, &target_context)
    }

    /// Evaluate FIRST_VALUE() or LAST_VALUE() window function
    fn evaluate_first_last_value(
        &mut self,
        expr: &Expression,
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
        is_first: bool,
    ) -> ProtocolResult<SqlValue> {
        let partition_rows = &window_context.partition_rows;

        if partition_rows.is_empty() {
            return Ok(SqlValue::Null);
        }

        // Get first or last row in partition
        let target_row_idx = if is_first {
            partition_rows[0]
        } else {
            partition_rows[partition_rows.len() - 1]
        };

        let target_row = &window_context.all_rows[target_row_idx];

        // Evaluate expression in context of target row
        let mut target_context = context.clone();
        target_context.current_row = target_row.clone();

        self.evaluate(expr, &target_context)
    }

    /// Evaluate NTH_VALUE() window function
    /// Returns the value of expr from the nth row in the window frame
    fn evaluate_nth_value(
        &mut self,
        expr: &Expression,
        n: &Expression,
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // Evaluate n (must be positive integer)
        let n_value = self.evaluate(n, context)?;
        let n_int = match n_value {
            SqlValue::Integer(i) => i as i64,
            SqlValue::BigInt(i) => i,
            SqlValue::SmallInt(i) => i as i64,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "NTH_VALUE requires integer argument".to_string(),
                ))
            }
        };

        if n_int < 1 {
            return Err(ProtocolError::PostgresError(
                "NTH_VALUE requires positive integer".to_string(),
            ));
        }

        let partition_rows = &window_context.partition_rows;

        // n is 1-indexed
        let index = (n_int - 1) as usize;
        if index >= partition_rows.len() {
            return Ok(SqlValue::Null);
        }

        let target_row_idx = partition_rows[index];
        let target_row = &window_context.all_rows[target_row_idx];

        // Evaluate expression in context of target row
        let mut target_context = context.clone();
        target_context.current_row = target_row.clone();

        self.evaluate(expr, &target_context)
    }

    /// Evaluate NTILE() window function
    /// Divides the partition into n groups and returns the group number
    fn evaluate_ntile(
        &mut self,
        n_expr: &Expression,
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // Evaluate n (number of buckets)
        let n_value = self.evaluate(n_expr, context)?;
        let n_buckets = match n_value {
            SqlValue::Integer(i) => i as i64,
            SqlValue::BigInt(i) => i,
            SqlValue::SmallInt(i) => i as i64,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "NTILE requires integer argument".to_string(),
                ))
            }
        };

        if n_buckets < 1 {
            return Err(ProtocolError::PostgresError(
                "NTILE requires positive integer".to_string(),
            ));
        }

        let partition_rows = &window_context.partition_rows;
        let current_idx = window_context.current_row_index;
        let partition_size = partition_rows.len() as i64;

        if partition_size == 0 {
            return Ok(SqlValue::BigInt(1));
        }

        // Find position in partition
        let pos_in_partition = partition_rows
            .iter()
            .position(|&idx| idx == current_idx)
            .unwrap_or(0) as i64;

        // Calculate bucket assignment
        // PostgreSQL distributes extra rows to earlier buckets
        let base_bucket_size = partition_size / n_buckets;
        let extra_rows = partition_size % n_buckets;

        // Rows 0..(extra_rows * (base_bucket_size + 1)) get bucket sizes of (base_bucket_size + 1)
        // Remaining rows get bucket size of base_bucket_size
        let bucket = if pos_in_partition < extra_rows * (base_bucket_size + 1) {
            pos_in_partition / (base_bucket_size + 1) + 1
        } else {
            let adjusted_pos = pos_in_partition - extra_rows * (base_bucket_size + 1);
            extra_rows + adjusted_pos / base_bucket_size + 1
        };

        Ok(SqlValue::BigInt(bucket))
    }

    /// Evaluate PERCENT_RANK() window function
    /// Returns (rank - 1) / (partition_size - 1)
    fn evaluate_percent_rank(
        &mut self,
        order_by: &[OrderByItem],
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let partition_size = window_context.partition_rows.len();

        if partition_size <= 1 {
            return Ok(SqlValue::DoublePrecision(0.0));
        }

        // Get the rank first
        let rank = self.evaluate_rank(order_by, window_context, context, false)?;
        let rank_value = match rank {
            SqlValue::BigInt(r) => r as f64,
            SqlValue::Integer(r) => r as f64,
            _ => 1.0,
        };

        let percent_rank = (rank_value - 1.0) / (partition_size as f64 - 1.0);
        Ok(SqlValue::DoublePrecision(percent_rank))
    }

    /// Evaluate CUME_DIST() window function
    /// Returns the cumulative distribution: number of rows <= current / total rows
    fn evaluate_cume_dist(
        &mut self,
        order_by: &[OrderByItem],
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let partition_rows = &window_context.partition_rows;
        let partition_size = partition_rows.len();
        let _current_idx = window_context.current_row_index;

        if partition_size == 0 {
            return Ok(SqlValue::DoublePrecision(1.0));
        }

        if order_by.is_empty() {
            // No ORDER BY means CUME_DIST is always 1.0
            return Ok(SqlValue::DoublePrecision(1.0));
        }

        // Get current row's order values
        let mut current_order_values = Vec::new();
        for item in order_by {
            current_order_values.push(self.evaluate(&item.expression, context)?);
        }

        // Count rows with values <= current row's values
        let mut count_le = 0i64;
        for &row_idx in partition_rows {
            let row = &window_context.all_rows[row_idx];
            let mut row_context = context.clone();
            row_context.current_row = row.clone();

            let mut row_order_values = Vec::new();
            for item in order_by {
                row_order_values.push(self.evaluate(&item.expression, &row_context)?);
            }

            // Check if row values <= current values
            let mut is_le = true;
            for (rv, cv) in row_order_values.iter().zip(current_order_values.iter()) {
                if self.compare_values(rv, cv)? == Ordering::Greater {
                    is_le = false;
                    break;
                }
            }

            if is_le {
                count_le += 1;
            }
        }

        let cume_dist = count_le as f64 / partition_size as f64;
        Ok(SqlValue::DoublePrecision(cume_dist))
    }

    /// Evaluate aggregate functions used as window functions
    fn evaluate_window_aggregate(
        &mut self,
        func: &FunctionCall,
        frame: &Option<WindowFrame>,
        window_context: &WindowFrameContext,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let func_name = match &func.name {
            FunctionName::Simple(name) => name.to_uppercase(),
            FunctionName::Qualified { schema: _, name } => name.to_uppercase(),
        };

        let partition_rows = &window_context.partition_rows;
        let current_idx = window_context.current_row_index;

        // Find current position within partition
        let current_partition_pos = partition_rows
            .iter()
            .position(|&idx| idx == current_idx)
            .unwrap_or(0);

        // Determine frame bounds based on mode (ROWS, RANGE, GROUPS)
        let (start_offset, end_offset) = if let Some(window_frame) = frame {
            self.get_frame_bounds_with_context(window_frame, window_context, current_partition_pos)?
        } else {
            // Default frame: RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
            (0, current_partition_pos)
        };

        // Collect values from frame rows, applying exclusion if specified
        let mut frame_values = Vec::new();
        let exclusion = frame.as_ref().and_then(|f| f.exclusion.as_ref());

        for i in start_offset..=end_offset {
            if i < partition_rows.len() {
                let row_idx = partition_rows[i];

                // Apply EXCLUDE clause
                if self.should_exclude_row(exclusion, i, current_partition_pos, window_context) {
                    continue;
                }

                let row = &window_context.all_rows[row_idx];

                let mut row_context = context.clone();
                row_context.current_row = row.clone();

                // Evaluate function arguments for this row
                for arg_expr in &func.args {
                    let val = self.evaluate(arg_expr, &row_context)?;
                    frame_values.push(val);
                }
            }
        }

        // Apply aggregate function
        match func_name.as_str() {
            "COUNT" => {
                let count = if func.args.is_empty() {
                    // COUNT(*)
                    (end_offset - start_offset + 1) as i64
                } else {
                    // COUNT(expr) - count non-null values
                    frame_values.iter().filter(|v| !v.is_null()).count() as i64
                };
                Ok(SqlValue::BigInt(count))
            }
            "SUM" => {
                let sum = self.sum_values(&frame_values)?;
                Ok(sum)
            }
            "AVG" => {
                let non_null: Vec<&SqlValue> =
                    frame_values.iter().filter(|v| !v.is_null()).collect();
                if non_null.is_empty() {
                    return Ok(SqlValue::Null);
                }
                let sum = self.sum_values(&frame_values)?;
                let count = non_null.len() as f64;
                match sum {
                    SqlValue::Integer(i) => Ok(SqlValue::DoublePrecision(i as f64 / count)),
                    SqlValue::BigInt(i) => Ok(SqlValue::DoublePrecision(i as f64 / count)),
                    SqlValue::DoublePrecision(f) => Ok(SqlValue::DoublePrecision(f / count)),
                    SqlValue::Real(f) => Ok(SqlValue::DoublePrecision(f as f64 / count)),
                    SqlValue::Null => Ok(SqlValue::Null),
                    _ => Ok(SqlValue::Null),
                }
            }
            "MIN" => {
                let min = frame_values.iter().filter(|v| !v.is_null()).try_fold(
                    None,
                    |acc: Option<&SqlValue>, v| -> ProtocolResult<Option<&SqlValue>> {
                        match acc {
                            None => Ok(Some(v)),
                            Some(current) => {
                                if self.compare_values(v, current)? == Ordering::Less {
                                    Ok(Some(v))
                                } else {
                                    Ok(Some(current))
                                }
                            }
                        }
                    },
                )?;
                Ok(min.cloned().unwrap_or(SqlValue::Null))
            }
            "MAX" => {
                let max = frame_values.iter().filter(|v| !v.is_null()).try_fold(
                    None,
                    |acc: Option<&SqlValue>, v| -> ProtocolResult<Option<&SqlValue>> {
                        match acc {
                            None => Ok(Some(v)),
                            Some(current) => {
                                if self.compare_values(v, current)? == Ordering::Greater {
                                    Ok(Some(v))
                                } else {
                                    Ok(Some(current))
                                }
                            }
                        }
                    },
                )?;
                Ok(max.cloned().unwrap_or(SqlValue::Null))
            }
            _ => Err(ProtocolError::not_implemented(
                "Window aggregate function",
                &func_name,
            )),
        }
    }

    /// Get frame bounds from WindowFrame specification
    /// Now properly handles ROWS, RANGE, and GROUPS modes
    fn get_frame_bounds(
        &mut self,
        frame: &WindowFrame,
        partition_size: usize,
        current_pos: usize,
    ) -> ProtocolResult<(usize, usize)> {
        let pos = current_pos.min(partition_size.saturating_sub(1));
        let empty_context = EvaluationContext::empty();

        let start = self.frame_bound_to_pos(
            &frame.start_bound,
            pos,
            partition_size,
            &empty_context,
            true,
        );
        let end = frame
            .end_bound
            .as_ref()
            .map(|b| self.frame_bound_to_pos(b, pos, partition_size, &empty_context, false))
            .unwrap_or(pos); // Default to CURRENT ROW

        Ok((start, end))
    }

    /// Get frame bounds with full context for RANGE and GROUPS modes
    fn get_frame_bounds_with_context(
        &mut self,
        frame: &WindowFrame,
        window_context: &WindowFrameContext,
        current_partition_pos: usize,
    ) -> ProtocolResult<(usize, usize)> {
        use crate::protocols::postgres_wire::sql::ast::WindowFrameMode;

        let partition_size = window_context.partition_rows.len();
        let pos = current_partition_pos.min(partition_size.saturating_sub(1));
        let empty_context = EvaluationContext::empty();

        match &frame.mode {
            WindowFrameMode::Rows => {
                // ROWS mode: use simple position-based calculation
                let start = self.frame_bound_to_pos(
                    &frame.start_bound,
                    pos,
                    partition_size,
                    &empty_context,
                    true,
                );
                let end = frame
                    .end_bound
                    .as_ref()
                    .map(|b| self.frame_bound_to_pos(b, pos, partition_size, &empty_context, false))
                    .unwrap_or(pos);
                Ok((start, end))
            }
            WindowFrameMode::Range => {
                // RANGE mode: use ORDER BY values for comparison
                self.get_range_frame_bounds(frame, window_context, pos)
            }
            WindowFrameMode::Groups => {
                // GROUPS mode: use peer groups
                self.get_groups_frame_bounds(frame, window_context, pos)
            }
        }
    }

    /// Get frame bounds for RANGE mode based on ORDER BY values
    fn get_range_frame_bounds(
        &mut self,
        frame: &WindowFrame,
        window_context: &WindowFrameContext,
        current_pos: usize,
    ) -> ProtocolResult<(usize, usize)> {
        use crate::protocols::postgres_wire::sql::ast::FrameBound;

        let partition_size = window_context.partition_rows.len();
        if partition_size == 0 {
            return Ok((0, 0));
        }

        // Get current ORDER BY value
        let current_value = if current_pos < window_context.order_by_values.len() {
            &window_context.order_by_values[current_pos]
        } else {
            return Ok((0, current_pos));
        };

        // For RANGE mode with UNBOUNDED bounds, same as ROWS
        let start = match &frame.start_bound {
            FrameBound::UnboundedPreceding => 0,
            FrameBound::CurrentRow => {
                // Find first row with same ORDER BY value (peer group start)
                self.find_range_peer_start(window_context, current_pos, current_value)
            }
            FrameBound::Preceding(expr) => {
                // Find rows where ORDER BY value >= current - offset
                let empty_ctx = EvaluationContext::empty();
                if let Ok(offset_val) = self.evaluate(expr, &empty_ctx) {
                    self.find_range_start_with_offset(window_context, current_value, &offset_val)
                } else {
                    0
                }
            }
            FrameBound::Following(_) => current_pos, // Invalid for start, use current
            FrameBound::UnboundedFollowing => current_pos, // Invalid for start
        };

        let end = match frame.end_bound.as_ref().unwrap_or(&FrameBound::CurrentRow) {
            FrameBound::UnboundedFollowing => partition_size.saturating_sub(1),
            FrameBound::CurrentRow => {
                // Find last row with same ORDER BY value (peer group end)
                self.find_range_peer_end(window_context, current_pos, current_value)
            }
            FrameBound::Following(expr) => {
                // Find rows where ORDER BY value <= current + offset
                let empty_ctx = EvaluationContext::empty();
                if let Ok(offset_val) = self.evaluate(expr, &empty_ctx) {
                    self.find_range_end_with_offset(window_context, current_value, &offset_val)
                } else {
                    partition_size.saturating_sub(1)
                }
            }
            FrameBound::Preceding(_) => current_pos, // Invalid for end, use current
            FrameBound::UnboundedPreceding => current_pos, // Invalid for end
        };

        Ok((
            start.min(partition_size.saturating_sub(1)),
            end.min(partition_size.saturating_sub(1)),
        ))
    }

    /// Find start of peer group (rows with same ORDER BY value)
    fn find_range_peer_start(
        &self,
        window_context: &WindowFrameContext,
        current_pos: usize,
        current_value: &SqlValue,
    ) -> usize {
        let mut start = current_pos;
        while start > 0 {
            if let Some(prev_val) = window_context.order_by_values.get(start - 1) {
                if self
                    .compare_values(prev_val, current_value)
                    .unwrap_or(Ordering::Less)
                    == Ordering::Equal
                {
                    start -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        start
    }

    /// Find end of peer group (rows with same ORDER BY value)
    fn find_range_peer_end(
        &self,
        window_context: &WindowFrameContext,
        current_pos: usize,
        current_value: &SqlValue,
    ) -> usize {
        let partition_size = window_context.partition_rows.len();
        let mut end = current_pos;
        while end < partition_size.saturating_sub(1) {
            if let Some(next_val) = window_context.order_by_values.get(end + 1) {
                if self
                    .compare_values(next_val, current_value)
                    .unwrap_or(Ordering::Less)
                    == Ordering::Equal
                {
                    end += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        end
    }

    /// Find start position for RANGE with PRECEDING offset
    fn find_range_start_with_offset(
        &self,
        window_context: &WindowFrameContext,
        current_value: &SqlValue,
        offset: &SqlValue,
    ) -> usize {
        // Calculate target value = current - offset
        let target_value = self.subtract_values(current_value, offset);

        // Find first row >= target value
        for (i, val) in window_context.order_by_values.iter().enumerate() {
            if let Ok(ordering) = self.compare_values(val, &target_value) {
                if ordering != Ordering::Less {
                    return i;
                }
            }
        }
        0
    }

    /// Find end position for RANGE with FOLLOWING offset
    fn find_range_end_with_offset(
        &self,
        window_context: &WindowFrameContext,
        current_value: &SqlValue,
        offset: &SqlValue,
    ) -> usize {
        // Calculate target value = current + offset
        let target_value = self.add_values(current_value, offset);

        let partition_size = window_context.partition_rows.len();

        // Find last row <= target value
        let mut last_valid = 0;
        for (i, val) in window_context.order_by_values.iter().enumerate() {
            if let Ok(ordering) = self.compare_values(val, &target_value) {
                if ordering != Ordering::Greater {
                    last_valid = i;
                }
            }
        }
        last_valid.min(partition_size.saturating_sub(1))
    }

    /// Get frame bounds for GROUPS mode based on peer groups
    fn get_groups_frame_bounds(
        &mut self,
        frame: &WindowFrame,
        window_context: &WindowFrameContext,
        current_pos: usize,
    ) -> ProtocolResult<(usize, usize)> {
        use crate::protocols::postgres_wire::sql::ast::FrameBound;

        let partition_size = window_context.partition_rows.len();
        if partition_size == 0 {
            return Ok((0, 0));
        }

        // Find current peer group index
        let current_group_idx = self.find_peer_group_index(window_context, current_pos);
        let num_groups = window_context.peer_groups.len();

        if num_groups == 0 {
            // No peer groups computed, fall back to ROWS behavior
            return self.get_frame_bounds(frame, partition_size, current_pos);
        }

        let empty_ctx = EvaluationContext::empty();

        // Calculate start group index
        let start_group_idx = match &frame.start_bound {
            FrameBound::UnboundedPreceding => 0,
            FrameBound::CurrentRow => current_group_idx,
            FrameBound::Preceding(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, &empty_ctx) {
                    current_group_idx.saturating_sub(n as usize)
                } else {
                    0
                }
            }
            FrameBound::Following(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, &empty_ctx) {
                    (current_group_idx + n as usize).min(num_groups.saturating_sub(1))
                } else {
                    current_group_idx
                }
            }
            FrameBound::UnboundedFollowing => current_group_idx, // Invalid for start
        };

        // Calculate end group index
        let end_group_idx = match frame.end_bound.as_ref().unwrap_or(&FrameBound::CurrentRow) {
            FrameBound::UnboundedFollowing => num_groups.saturating_sub(1),
            FrameBound::CurrentRow => current_group_idx,
            FrameBound::Following(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, &empty_ctx) {
                    (current_group_idx + n as usize).min(num_groups.saturating_sub(1))
                } else {
                    num_groups.saturating_sub(1)
                }
            }
            FrameBound::Preceding(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, &empty_ctx) {
                    current_group_idx.saturating_sub(n as usize)
                } else {
                    current_group_idx
                }
            }
            FrameBound::UnboundedPreceding => current_group_idx, // Invalid for end
        };

        // Convert group indices to row positions
        let start_pos = if start_group_idx < window_context.peer_groups.len() {
            window_context.peer_groups[start_group_idx].0
        } else {
            0
        };

        let end_pos = if end_group_idx < window_context.peer_groups.len() {
            window_context.peer_groups[end_group_idx].1
        } else {
            partition_size.saturating_sub(1)
        };

        Ok((start_pos, end_pos))
    }

    /// Find which peer group contains the given position
    fn find_peer_group_index(&self, window_context: &WindowFrameContext, pos: usize) -> usize {
        for (idx, (start, end)) in window_context.peer_groups.iter().enumerate() {
            if pos >= *start && pos <= *end {
                return idx;
            }
        }
        0
    }

    /// Add two SqlValues (for RANGE offset calculation)
    fn add_values(&self, a: &SqlValue, b: &SqlValue) -> SqlValue {
        match (a, b) {
            (SqlValue::Integer(x), SqlValue::Integer(y)) => SqlValue::Integer(x + y),
            (SqlValue::BigInt(x), SqlValue::Integer(y)) => SqlValue::BigInt(x + *y as i64),
            (SqlValue::Integer(x), SqlValue::BigInt(y)) => SqlValue::BigInt(*x as i64 + y),
            (SqlValue::BigInt(x), SqlValue::BigInt(y)) => SqlValue::BigInt(x + y),
            (SqlValue::DoublePrecision(x), SqlValue::DoublePrecision(y)) => {
                SqlValue::DoublePrecision(x + y)
            }
            (SqlValue::DoublePrecision(x), SqlValue::Integer(y)) => {
                SqlValue::DoublePrecision(x + *y as f64)
            }
            (SqlValue::Integer(x), SqlValue::DoublePrecision(y)) => {
                SqlValue::DoublePrecision(*x as f64 + y)
            }
            _ => a.clone(), // Fallback
        }
    }

    /// Subtract two SqlValues (for RANGE offset calculation)
    fn subtract_values(&self, a: &SqlValue, b: &SqlValue) -> SqlValue {
        match (a, b) {
            (SqlValue::Integer(x), SqlValue::Integer(y)) => SqlValue::Integer(x - y),
            (SqlValue::BigInt(x), SqlValue::Integer(y)) => SqlValue::BigInt(x - *y as i64),
            (SqlValue::Integer(x), SqlValue::BigInt(y)) => SqlValue::BigInt(*x as i64 - y),
            (SqlValue::BigInt(x), SqlValue::BigInt(y)) => SqlValue::BigInt(x - y),
            (SqlValue::DoublePrecision(x), SqlValue::DoublePrecision(y)) => {
                SqlValue::DoublePrecision(x - y)
            }
            (SqlValue::DoublePrecision(x), SqlValue::Integer(y)) => {
                SqlValue::DoublePrecision(x - *y as f64)
            }
            (SqlValue::Integer(x), SqlValue::DoublePrecision(y)) => {
                SqlValue::DoublePrecision(*x as f64 - y)
            }
            _ => a.clone(), // Fallback
        }
    }

    /// Check if a row should be excluded based on EXCLUDE clause
    fn should_exclude_row(
        &self,
        exclusion: Option<&crate::protocols::postgres_wire::sql::ast::WindowFrameExclusion>,
        row_pos: usize,
        current_pos: usize,
        window_context: &WindowFrameContext,
    ) -> bool {
        use crate::protocols::postgres_wire::sql::ast::WindowFrameExclusion;

        match exclusion {
            None | Some(WindowFrameExclusion::NoOthers) => false,
            Some(WindowFrameExclusion::CurrentRow) => row_pos == current_pos,
            Some(WindowFrameExclusion::Group) => {
                // Exclude all rows in the same peer group as current row
                self.is_in_same_peer_group(window_context, row_pos, current_pos)
            }
            Some(WindowFrameExclusion::Ties) => {
                // Exclude peers of the current row, but not the current row itself
                row_pos != current_pos
                    && self.is_in_same_peer_group(window_context, row_pos, current_pos)
            }
        }
    }

    /// Check if two positions are in the same peer group (same ORDER BY values)
    fn is_in_same_peer_group(
        &self,
        window_context: &WindowFrameContext,
        pos1: usize,
        pos2: usize,
    ) -> bool {
        // Check using peer_groups if available
        if !window_context.peer_groups.is_empty() {
            let group1 = self.find_peer_group_index(window_context, pos1);
            let group2 = self.find_peer_group_index(window_context, pos2);
            return group1 == group2;
        }

        // Otherwise compare ORDER BY values directly
        if let (Some(val1), Some(val2)) = (
            window_context.order_by_values.get(pos1),
            window_context.order_by_values.get(pos2),
        ) {
            self.compare_values(val1, val2).unwrap_or(Ordering::Less) == Ordering::Equal
        } else {
            false
        }
    }

    /// Convert a frame bound to a position
    fn frame_bound_to_pos(
        &mut self,
        bound: &crate::protocols::postgres_wire::sql::ast::FrameBound,
        pos: usize,
        partition_size: usize,
        context: &EvaluationContext,
        is_start: bool,
    ) -> usize {
        use crate::protocols::postgres_wire::sql::ast::FrameBound;
        match bound {
            FrameBound::UnboundedPreceding => 0,
            FrameBound::Preceding(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, context) {
                    pos.saturating_sub(n as usize)
                } else if is_start {
                    0
                } else {
                    pos
                }
            }
            FrameBound::CurrentRow => pos,
            FrameBound::Following(expr) => {
                if let Ok(SqlValue::Integer(n)) = self.evaluate(expr, context) {
                    std::cmp::min(pos + n as usize, partition_size.saturating_sub(1))
                } else {
                    pos
                }
            }
            FrameBound::UnboundedFollowing => partition_size.saturating_sub(1),
        }
    }

    /// Sum a slice of SqlValues
    fn sum_values(&self, values: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let mut sum_int: i64 = 0;
        let mut sum_float: f64 = 0.0;
        let mut has_float = false;
        let mut has_any = false;

        for v in values {
            match v {
                SqlValue::Integer(i) => {
                    has_any = true;
                    sum_int += *i as i64;
                }
                SqlValue::BigInt(i) => {
                    has_any = true;
                    sum_int += i;
                }
                SqlValue::SmallInt(i) => {
                    has_any = true;
                    sum_int += *i as i64;
                }
                SqlValue::DoublePrecision(f) => {
                    has_any = true;
                    has_float = true;
                    sum_float += f;
                }
                SqlValue::Real(f) => {
                    has_any = true;
                    has_float = true;
                    sum_float += *f as f64;
                }
                SqlValue::Null => {}
                _ => {}
            }
        }

        if !has_any {
            return Ok(SqlValue::Null);
        }

        if has_float {
            Ok(SqlValue::DoublePrecision(sum_float + sum_int as f64))
        } else {
            Ok(SqlValue::BigInt(sum_int))
        }
    }

    fn pattern_match(
        &self,
        text: &SqlValue,
        pattern: &SqlValue,
        case_insensitive: bool,
        negated: bool,
    ) -> ProtocolResult<SqlValue> {
        let text_str = match text {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "LIKE requires string operands".to_string(),
                ))
            }
        };

        let pattern_str = match pattern {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => s,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "LIKE requires string operands".to_string(),
                ))
            }
        };

        // Convert SQL LIKE pattern to regex pattern
        let regex_pattern = self.like_pattern_to_regex(pattern_str, '\\', case_insensitive);

        // Perform the match using the LIKE matching implementation
        let matches = self.match_like_pattern(text_str, &regex_pattern, case_insensitive);

        Ok(SqlValue::Boolean(if negated { !matches } else { matches }))
    }

    // ==================== JSON Operators ====================

    /// Extract JSON field using -> operator
    fn json_extract(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        match right {
            SqlValue::Text(key) | SqlValue::Varchar(key) => {
                super::json::JsonOperations::json_extract(&json, key)
            }
            SqlValue::Integer(idx) => {
                super::json::JsonOperations::json_extract_index(&json, *idx as i64)
            }
            SqlValue::BigInt(idx) => super::json::JsonOperations::json_extract_index(&json, *idx),
            _ => Err(ProtocolError::PostgresError(
                "JSON extract requires string key or integer index".to_string(),
            )),
        }
    }

    /// Extract JSON field as text using ->> operator
    fn json_extract_text(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        match right {
            SqlValue::Text(key) | SqlValue::Varchar(key) => {
                super::json::JsonOperations::json_extract_text(&json, key)
            }
            SqlValue::Integer(idx) => {
                super::json::JsonOperations::json_extract_text_index(&json, *idx as i64)
            }
            SqlValue::BigInt(idx) => {
                super::json::JsonOperations::json_extract_text_index(&json, *idx)
            }
            _ => Err(ProtocolError::PostgresError(
                "JSON extract text requires string key or integer index".to_string(),
            )),
        }
    }

    /// Extract JSON sub-object at path using #> operator
    fn json_path_extract(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let path = self.get_json_path(right)?;
        super::json::JsonOperations::json_path_extract(&json, &path)
    }

    /// Extract JSON sub-object at path as text using #>> operator
    fn json_path_extract_text(
        &self,
        left: &SqlValue,
        right: &SqlValue,
    ) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let path = self.get_json_path(right)?;
        super::json::JsonOperations::json_path_extract_text(&json, &path)
    }

    /// Check if left JSON contains right JSON using @> operator
    fn json_contains(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let left_json = self.get_json_value(left)?;
        let right_json = self.get_json_value(right)?;
        super::json::JsonOperations::json_contains(&left_json, &right_json)
    }

    /// Check if left JSON is contained by right JSON using <@ operator
    fn json_contained_by(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let left_json = self.get_json_value(left)?;
        let right_json = self.get_json_value(right)?;
        super::json::JsonOperations::json_contained_by(&left_json, &right_json)
    }

    /// Check if string exists as top-level key using ? operator
    fn json_exists(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let key = self.get_string_value(right)?;
        super::json::JsonOperations::json_exists(&json, &key)
    }

    /// Check if any strings exist as top-level keys using ?| operator
    fn json_exists_any(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let keys = self.get_string_array(right)?;
        super::json::JsonOperations::json_exists_any(&json, &keys)
    }

    /// Check if all strings exist as top-level keys using ?& operator
    fn json_exists_all(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let keys = self.get_string_array(right)?;
        super::json::JsonOperations::json_exists_all(&json, &keys)
    }

    /// Concatenate two JSON values using || operator
    fn json_concat(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let left_json = self.get_json_value(left)?;
        let right_json = self.get_json_value(right)?;
        super::json::JsonOperations::json_concat(&left_json, &right_json)
    }

    /// Delete key or array element using - operator
    fn json_delete(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let key = self.get_string_value(right)?;
        super::json::JsonOperations::json_delete(&json, &key)
    }

    /// Delete path from JSONB using #- operator
    fn json_delete_path(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        let json = self.get_json_value(left)?;
        let path = self.get_json_path(right)?;
        super::json::JsonOperations::jsonb_delete_path(&json, &path)
    }

    // JSON helper methods

    fn get_json_value(&self, value: &SqlValue) -> ProtocolResult<serde_json::Value> {
        match value {
            SqlValue::Json(j) | SqlValue::Jsonb(j) => Ok(j.clone()),
            SqlValue::Text(s) | SqlValue::Varchar(s) => serde_json::from_str(s)
                .map_err(|e| ProtocolError::PostgresError(format!("Invalid JSON: {}", e))),
            SqlValue::Null => Ok(serde_json::Value::Null),
            _ => Err(ProtocolError::PostgresError(format!(
                "Expected JSON value, got {:?}",
                value
            ))),
        }
    }

    fn get_json_path(&self, value: &SqlValue) -> ProtocolResult<super::json::JsonPath> {
        match value {
            SqlValue::Array(arr) => {
                let path_strs: Vec<String> = arr
                    .iter()
                    .map(|v| match v {
                        SqlValue::Text(s) | SqlValue::Varchar(s) => Ok(s.clone()),
                        SqlValue::Integer(i) => Ok(i.to_string()),
                        SqlValue::BigInt(i) => Ok(i.to_string()),
                        _ => Err(ProtocolError::PostgresError(
                            "JSON path elements must be strings or integers".to_string(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                super::json::JsonPath::from_text_array(&path_strs)
            }
            SqlValue::Text(s) | SqlValue::Varchar(s) => {
                // Parse text array notation like '{key1,0,key2}'
                let trimmed = s.trim_matches(|c| c == '{' || c == '}');
                let parts: Vec<String> = trimmed.split(',').map(|s| s.trim().to_string()).collect();
                super::json::JsonPath::from_text_array(&parts)
            }
            _ => Err(ProtocolError::PostgresError(
                "JSON path must be a text array".to_string(),
            )),
        }
    }

    fn get_string_value(&self, value: &SqlValue) -> ProtocolResult<String> {
        match value {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Ok(s.clone()),
            SqlValue::Integer(i) => Ok(i.to_string()),
            SqlValue::BigInt(i) => Ok(i.to_string()),
            _ => Err(ProtocolError::PostgresError(format!(
                "Expected string value, got {:?}",
                value
            ))),
        }
    }

    fn get_string_array(&self, value: &SqlValue) -> ProtocolResult<Vec<String>> {
        match value {
            SqlValue::Array(arr) => arr
                .iter()
                .map(|v| self.get_string_value(v))
                .collect::<Result<Vec<_>, _>>(),
            SqlValue::Text(s) | SqlValue::Varchar(s) => {
                // Parse text array notation like '{a,b,c}'
                let trimmed = s.trim_matches(|c| c == '{' || c == '}');
                Ok(trimmed.split(',').map(|s| s.trim().to_string()).collect())
            }
            _ => Err(ProtocolError::PostgresError(
                "Expected string array".to_string(),
            )),
        }
    }

    // Range operator implementations for PostgreSQL range types

    /// @> operator: range contains element/range
    fn range_contains(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(range), SqlValue::Range(other)) => {
                // Range contains another range if lower <= other.lower AND upper >= other.upper
                let lower_ok = match (&range.lower, &other.lower) {
                    (Some(r_lower), Some(o_lower)) => {
                        self.compare_values(r_lower, o_lower)? != Ordering::Greater
                    }
                    (None, _) => true,        // Unbounded lower contains any lower
                    (Some(_), None) => false, // Bounded lower doesn't contain unbounded
                };
                let upper_ok = match (&range.upper, &other.upper) {
                    (Some(r_upper), Some(o_upper)) => {
                        self.compare_values(r_upper, o_upper)? != Ordering::Less
                    }
                    (None, _) => true,        // Unbounded upper contains any upper
                    (Some(_), None) => false, // Bounded upper doesn't contain unbounded
                };
                Ok(SqlValue::Boolean(lower_ok && upper_ok))
            }
            (SqlValue::Range(range), elem) => {
                // Range contains element
                let lower_ok = match &range.lower {
                    Some(lower) => {
                        if range.lower_inclusive {
                            self.compare_values(lower, elem)? != Ordering::Greater
                        } else {
                            self.compare_values(lower, elem)? == Ordering::Less
                        }
                    }
                    None => true, // Unbounded lower
                };
                let upper_ok = match &range.upper {
                    Some(upper) => {
                        if range.upper_inclusive {
                            self.compare_values(upper, elem)? != Ordering::Less
                        } else {
                            self.compare_values(upper, elem)? == Ordering::Greater
                        }
                    }
                    None => true, // Unbounded upper
                };
                Ok(SqlValue::Boolean(lower_ok && upper_ok))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range contains operator requires range type".to_string(),
            )),
        }
    }

    /// <@ operator: element/range is contained by range
    fn range_contained_by(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        self.range_contains(right, left)
    }

    /// && operator: ranges overlap
    fn range_overlaps(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                let r1_lower_lt_r2_upper = match (&r1.lower, &r2.upper) {
                    (Some(l), Some(u)) => self.compare_values(l, u)? == Ordering::Less,
                    (None, _) | (_, None) => true, // Unbounded ranges always overlap
                };
                let r2_lower_lt_r1_upper = match (&r2.lower, &r1.upper) {
                    (Some(l), Some(u)) => self.compare_values(l, u)? == Ordering::Less,
                    (None, _) | (_, None) => true, // Unbounded ranges always overlap
                };
                Ok(SqlValue::Boolean(
                    r1_lower_lt_r2_upper && r2_lower_lt_r1_upper,
                ))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range overlaps operator requires range types".to_string(),
            )),
        }
    }

    /// -|- operator: ranges are adjacent
    fn range_adjacent(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                let r1_upper_eq_r2_lower = match (&r1.upper, &r2.lower) {
                    (Some(u), Some(l)) => {
                        self.compare_values(u, l)? == Ordering::Equal
                            && (r1.upper_inclusive != r2.lower_inclusive)
                    }
                    _ => false, // Unbounded ranges can't be adjacent
                };
                let r2_upper_eq_r1_lower = match (&r2.upper, &r1.lower) {
                    (Some(u), Some(l)) => {
                        self.compare_values(u, l)? == Ordering::Equal
                            && (r2.upper_inclusive != r1.lower_inclusive)
                    }
                    _ => false, // Unbounded ranges can't be adjacent
                };
                Ok(SqlValue::Boolean(
                    r1_upper_eq_r2_lower || r2_upper_eq_r1_lower,
                ))
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range adjacent operator requires range types".to_string(),
            )),
        }
    }

    /// << operator: range is strictly left of range
    fn range_strictly_left(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                match (&r1.upper, &r2.lower) {
                    (Some(u), Some(l)) => Ok(SqlValue::Boolean(
                        self.compare_values(u, l)? == Ordering::Less,
                    )),
                    _ => Ok(SqlValue::Boolean(false)), // Unbounded ranges can't be strictly left
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range strictly left operator requires range types".to_string(),
            )),
        }
    }

    /// >> operator: range is strictly right of range
    fn range_strictly_right(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                match (&r1.lower, &r2.upper) {
                    (Some(l), Some(u)) => Ok(SqlValue::Boolean(
                        self.compare_values(l, u)? == Ordering::Greater,
                    )),
                    _ => Ok(SqlValue::Boolean(false)), // Unbounded ranges can't be strictly right
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range strictly right operator requires range types".to_string(),
            )),
        }
    }

    /// &< operator: range does not extend right of range
    fn range_not_extend_right(
        &self,
        left: &SqlValue,
        right: &SqlValue,
    ) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                match (&r1.upper, &r2.upper) {
                    (Some(u1), Some(u2)) => Ok(SqlValue::Boolean(
                        self.compare_values(u1, u2)? != Ordering::Greater,
                    )),
                    (None, _) => Ok(SqlValue::Boolean(false)), // Unbounded upper extends right
                    (_, None) => Ok(SqlValue::Boolean(true)), // Any bounded doesn't extend past unbounded
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range not extend right operator requires range types".to_string(),
            )),
        }
    }

    /// &> operator: range does not extend left of range
    fn range_not_extend_left(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Range(r1), SqlValue::Range(r2)) => {
                match (&r1.lower, &r2.lower) {
                    (Some(l1), Some(l2)) => Ok(SqlValue::Boolean(
                        self.compare_values(l1, l2)? != Ordering::Less,
                    )),
                    (None, _) => Ok(SqlValue::Boolean(false)), // Unbounded lower extends left
                    (_, None) => Ok(SqlValue::Boolean(true)), // Any bounded doesn't extend past unbounded
                }
            }
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            _ => Err(ProtocolError::PostgresError(
                "Range not extend left operator requires range types".to_string(),
            )),
        }
    }

    // ===== Full-Text Search Functions =====

    /// to_tsvector([ config, ] document) - convert document to tsvector
    fn evaluate_to_tsvector(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let (config, document) = match args.len() {
            1 => ("english".to_string(), self.sqlvalue_to_string(&args[0])?),
            2 => (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            ),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "to_tsvector() requires 1 or 2 arguments".to_string(),
                ))
            }
        };

        // Tokenize the document into lexemes with positions
        let tokens = self.tokenize_text(&document, &config);
        let tsvector_str = tokens
            .iter()
            .map(|(lexeme, positions)| {
                let pos_str: Vec<String> = positions.iter().map(|p| p.to_string()).collect();
                format!("'{}':{}",lexeme, pos_str.join(","))
            })
            .collect::<Vec<_>>()
            .join(" ");

        Ok(SqlValue::Text(tsvector_str))
    }

    /// to_tsquery([ config, ] querytext) - convert query to tsquery
    fn evaluate_to_tsquery(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let (config, query) = match args.len() {
            1 => ("english".to_string(), self.sqlvalue_to_string(&args[0])?),
            2 => (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            ),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "to_tsquery() requires 1 or 2 arguments".to_string(),
                ))
            }
        };

        // Parse the query - supports & (AND), | (OR), ! (NOT) operators
        let normalized = self.normalize_tsquery(&query, &config);
        Ok(SqlValue::Text(normalized))
    }

    /// plainto_tsquery([ config, ] querytext) - convert plain text to tsquery (words joined with &)
    fn evaluate_plainto_tsquery(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let (config, query) = match args.len() {
            1 => ("english".to_string(), self.sqlvalue_to_string(&args[0])?),
            2 => (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            ),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "plainto_tsquery() requires 1 or 2 arguments".to_string(),
                ))
            }
        };

        // Plain text: split by whitespace and join with &
        let words: Vec<String> = query
            .split_whitespace()
            .map(|w| self.stem_word(w, &config))
            .filter(|w| !w.is_empty())
            .collect();

        let tsquery = if words.is_empty() {
            String::new()
        } else {
            words
                .iter()
                .map(|w| format!("'{}'", w))
                .collect::<Vec<_>>()
                .join(" & ")
        };

        Ok(SqlValue::Text(tsquery))
    }

    /// phraseto_tsquery([ config, ] querytext) - convert phrase to tsquery (with <-> proximity)
    fn evaluate_phraseto_tsquery(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let (config, query) = match args.len() {
            1 => ("english".to_string(), self.sqlvalue_to_string(&args[0])?),
            2 => (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            ),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "phraseto_tsquery() requires 1 or 2 arguments".to_string(),
                ))
            }
        };

        // Phrase: split by whitespace and join with <-> (FOLLOWED BY operator)
        let words: Vec<String> = query
            .split_whitespace()
            .map(|w| self.stem_word(w, &config))
            .filter(|w| !w.is_empty())
            .collect();

        let tsquery = if words.is_empty() {
            String::new()
        } else {
            words
                .iter()
                .map(|w| format!("'{}'", w))
                .collect::<Vec<_>>()
                .join(" <-> ")
        };

        Ok(SqlValue::Text(tsquery))
    }

    /// websearch_to_tsquery([ config, ] querytext) - convert web search syntax to tsquery
    fn evaluate_websearch_to_tsquery(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        let (config, query) = match args.len() {
            1 => ("english".to_string(), self.sqlvalue_to_string(&args[0])?),
            2 => (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            ),
            _ => {
                return Err(ProtocolError::PostgresError(
                    "websearch_to_tsquery() requires 1 or 2 arguments".to_string(),
                ))
            }
        };

        // Web search syntax:
        // - unquoted words are ANDed
        // - "quoted text" creates phrases
        // - -word excludes that word
        // - or between words creates OR

        let mut result_parts: Vec<String> = Vec::new();
        let mut chars = query.chars().peekable();
        let mut current_word = String::new();
        let mut in_quotes = false;
        let mut negate_next = false;

        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    if in_quotes {
                        // End of quoted phrase
                        if !current_word.is_empty() {
                            let phrase_words: Vec<String> = current_word
                                .split_whitespace()
                                .map(|w| self.stem_word(w, &config))
                                .filter(|w| !w.is_empty())
                                .collect();
                            if !phrase_words.is_empty() {
                                let phrase = phrase_words
                                    .iter()
                                    .map(|w| format!("'{}'", w))
                                    .collect::<Vec<_>>()
                                    .join(" <-> ");
                                let part = if negate_next {
                                    format!("!({})", phrase)
                                } else {
                                    format!("({})", phrase)
                                };
                                result_parts.push(part);
                                negate_next = false;
                            }
                            current_word.clear();
                        }
                        in_quotes = false;
                    } else {
                        in_quotes = true;
                    }
                }
                '-' if !in_quotes && current_word.is_empty() => {
                    negate_next = true;
                }
                ' ' if !in_quotes => {
                    if !current_word.is_empty() {
                        let lower = current_word.to_lowercase();
                        if lower == "or" {
                            // Replace last AND with OR if present
                            if !result_parts.is_empty() {
                                // Mark next item for OR
                                result_parts.push("|".to_string());
                            }
                        } else {
                            let stemmed = self.stem_word(&current_word, &config);
                            if !stemmed.is_empty() {
                                let part = if negate_next {
                                    format!("!'{}'", stemmed)
                                } else {
                                    format!("'{}'", stemmed)
                                };
                                result_parts.push(part);
                                negate_next = false;
                            }
                        }
                        current_word.clear();
                    }
                }
                _ => {
                    current_word.push(c);
                }
            }
        }

        // Handle trailing word
        if !current_word.is_empty() {
            let stemmed = self.stem_word(&current_word, &config);
            if !stemmed.is_empty() {
                let part = if negate_next {
                    format!("!'{}'", stemmed)
                } else {
                    format!("'{}'", stemmed)
                };
                result_parts.push(part);
            }
        }

        // Join parts with appropriate operators
        let mut final_parts: Vec<String> = Vec::new();
        let mut use_or = false;

        for part in result_parts {
            if part == "|" {
                use_or = true;
            } else {
                if !final_parts.is_empty() {
                    if use_or {
                        final_parts.push(" | ".to_string());
                        use_or = false;
                    } else {
                        final_parts.push(" & ".to_string());
                    }
                }
                final_parts.push(part);
            }
        }

        Ok(SqlValue::Text(final_parts.join("")))
    }

    /// setweight(tsvector, weight [, lexemes]) - assign weight to lexemes
    fn evaluate_setweight(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "setweight() requires 2 or 3 arguments".to_string(),
            ));
        }

        let tsvector = self.sqlvalue_to_string(&args[0])?;
        let weight = self.sqlvalue_to_string(&args[1])?;

        // Validate weight
        let weight_char = weight.chars().next().unwrap_or('D');
        if !['A', 'B', 'C', 'D'].contains(&weight_char.to_ascii_uppercase()) {
            return Err(ProtocolError::PostgresError(
                "weight must be A, B, C, or D".to_string(),
            ));
        }

        // Parse tsvector and add weight
        let weighted = self.add_weight_to_tsvector(&tsvector, weight_char.to_ascii_uppercase());

        Ok(SqlValue::Text(weighted))
    }

    /// ts_rank([weights,] vector, query [, normalization]) - rank document for query
    fn evaluate_ts_rank(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 4 {
            return Err(ProtocolError::PostgresError(
                "ts_rank() requires 2 to 4 arguments".to_string(),
            ));
        }

        let (vector, query) = if args.len() >= 2 {
            (
                self.sqlvalue_to_string(&args[args.len() - 2])?,
                self.sqlvalue_to_string(&args[args.len() - 1])?,
            )
        } else {
            return Err(ProtocolError::PostgresError(
                "ts_rank() requires at least vector and query arguments".to_string(),
            ));
        };

        // Calculate rank based on matching terms
        let rank = self.calculate_ts_rank(&vector, &query, false);

        Ok(SqlValue::DoublePrecision(rank))
    }

    /// ts_rank_cd([weights,] vector, query [, normalization]) - cover density rank
    fn evaluate_ts_rank_cd(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.is_empty() || args.len() > 4 {
            return Err(ProtocolError::PostgresError(
                "ts_rank_cd() requires 2 to 4 arguments".to_string(),
            ));
        }

        let (vector, query) = if args.len() >= 2 {
            (
                self.sqlvalue_to_string(&args[args.len() - 2])?,
                self.sqlvalue_to_string(&args[args.len() - 1])?,
            )
        } else {
            return Err(ProtocolError::PostgresError(
                "ts_rank_cd() requires at least vector and query arguments".to_string(),
            ));
        };

        // Calculate cover density rank
        let rank = self.calculate_ts_rank(&vector, &query, true);

        Ok(SqlValue::DoublePrecision(rank))
    }

    /// ts_headline([config,] document, query [, options]) - display search result with highlights
    fn evaluate_ts_headline(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 4 {
            return Err(ProtocolError::PostgresError(
                "ts_headline() requires 2 to 4 arguments".to_string(),
            ));
        }

        let (document, query) = if args.len() == 2 {
            (
                self.sqlvalue_to_string(&args[0])?,
                self.sqlvalue_to_string(&args[1])?,
            )
        } else {
            (
                self.sqlvalue_to_string(&args[1])?,
                self.sqlvalue_to_string(&args[2])?,
            )
        };

        // Extract query terms
        let query_terms = self.extract_query_terms(&query);

        // Highlight matching terms with <b>...</b>
        let highlighted = self.highlight_text(&document, &query_terms, "<b>", "</b>");

        Ok(SqlValue::Text(highlighted))
    }

    /// tsvector || tsvector - concatenate tsvectors
    fn evaluate_tsvector_concat(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "tsvector concatenation requires 2 arguments".to_string(),
            ));
        }

        let vec1 = self.sqlvalue_to_string(&args[0])?;
        let vec2 = self.sqlvalue_to_string(&args[1])?;

        // Parse and merge tsvectors
        let merged = self.merge_tsvectors(&vec1, &vec2);

        Ok(SqlValue::Text(merged))
    }

    /// numnode(tsquery) - number of nodes in tsquery
    fn evaluate_numnode(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "numnode() requires exactly 1 argument".to_string(),
            ));
        }

        let query = self.sqlvalue_to_string(&args[0])?;

        // Count nodes in query (terms and operators)
        let count = self.count_query_nodes(&query);

        Ok(SqlValue::Integer(count))
    }

    /// querytree(tsquery) - display query tree
    fn evaluate_querytree(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "querytree() requires exactly 1 argument".to_string(),
            ));
        }

        let query = self.sqlvalue_to_string(&args[0])?;

        // Return the normalized query tree representation
        Ok(SqlValue::Text(query))
    }

    /// strip(tsvector) - remove positions and weights
    fn evaluate_strip(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "strip() requires exactly 1 argument".to_string(),
            ));
        }

        let tsvector = self.sqlvalue_to_string(&args[0])?;

        // Remove positions and weights, keep only lexemes
        let stripped = self.strip_tsvector(&tsvector);

        Ok(SqlValue::Text(stripped))
    }

    /// ts_lexize(dict, token) - test dictionary on token
    fn evaluate_ts_lexize(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ts_lexize() requires exactly 2 arguments".to_string(),
            ));
        }

        let _dict = self.sqlvalue_to_string(&args[0])?;
        let token = self.sqlvalue_to_string(&args[1])?;

        // For now, return a simple array with the lowercased, stemmed token
        let stemmed = self.stem_word(&token, "english");
        let result = format!("{{{}}}", stemmed);

        Ok(SqlValue::Text(result))
    }

    /// ts_parse(parser, document) - test parser
    fn evaluate_ts_parse(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ts_parse() requires exactly 2 arguments".to_string(),
            ));
        }

        let _parser = self.sqlvalue_to_string(&args[0])?;
        let document = self.sqlvalue_to_string(&args[1])?;

        // Return tokens as a set of (tokid, token) pairs
        let tokens: Vec<String> = document
            .split_whitespace()
            .enumerate()
            .map(|(i, token)| format!("({},\"{}\")", i + 1, token))
            .collect();

        Ok(SqlValue::Text(format!("{{{}}}", tokens.join(","))))
    }

    /// ts_token_type(parser) - get token types for parser
    fn evaluate_ts_token_type(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "ts_token_type() requires exactly 1 argument".to_string(),
            ));
        }

        // Return common token types
        let token_types = vec![
            "(1,\"asciiword\",\"Word, all ASCII\")",
            "(2,\"word\",\"Word, all letters\")",
            "(3,\"numword\",\"Word, letters and digits\")",
            "(4,\"email\",\"Email address\")",
            "(5,\"url\",\"URL\")",
            "(6,\"host\",\"Host\")",
            "(7,\"sfloat\",\"Scientific notation\")",
            "(8,\"version\",\"Version number\")",
            "(9,\"hword_numpart\",\"Hyphenated word part, letters and digits\")",
            "(10,\"hword_part\",\"Hyphenated word part, all letters\")",
            "(11,\"hword_asciipart\",\"Hyphenated word part, all ASCII\")",
            "(12,\"blank\",\"Space symbols\")",
            "(13,\"tag\",\"XML tag\")",
            "(14,\"protocol\",\"Protocol head\")",
            "(15,\"numhword\",\"Hyphenated word, letters and digits\")",
            "(16,\"asciihword\",\"Hyphenated word, all ASCII\")",
            "(17,\"hword\",\"Hyphenated word, all letters\")",
            "(18,\"url_path\",\"URL path\")",
            "(19,\"file\",\"File or path name\")",
            "(20,\"float\",\"Decimal notation\")",
            "(21,\"int\",\"Signed integer\")",
            "(22,\"uint\",\"Unsigned integer\")",
            "(23,\"entity\",\"XML entity\")",
        ];

        Ok(SqlValue::Text(format!("{{{}}}", token_types.join(","))))
    }

    /// get_current_ts_config() - get default text search configuration
    fn evaluate_get_current_ts_config(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if !args.is_empty() {
            return Err(ProtocolError::PostgresError(
                "get_current_ts_config() takes no arguments".to_string(),
            ));
        }

        // Return the default text search configuration
        Ok(SqlValue::Text("english".to_string()))
    }

    /// array_to_tsvector(text[]) - convert array to tsvector
    fn evaluate_array_to_tsvector(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "array_to_tsvector() requires exactly 1 argument".to_string(),
            ));
        }

        let array_str = self.sqlvalue_to_string(&args[0])?;

        // Parse array {word1,word2,...} format
        let cleaned = array_str.trim_matches(|c| c == '{' || c == '}');
        let words: Vec<&str> = cleaned.split(',').map(|s| s.trim().trim_matches('"')).collect();

        // Create tsvector with positions
        let tsvector = words
            .iter()
            .enumerate()
            .map(|(i, word)| format!("'{}':{}",word.to_lowercase(), i + 1))
            .collect::<Vec<_>>()
            .join(" ");

        Ok(SqlValue::Text(tsvector))
    }

    /// tsvector_to_array(tsvector) - convert tsvector to array
    fn evaluate_tsvector_to_array(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "tsvector_to_array() requires exactly 1 argument".to_string(),
            ));
        }

        let tsvector = self.sqlvalue_to_string(&args[0])?;

        // Extract lexemes from tsvector
        let lexemes: Vec<String> = tsvector
            .split_whitespace()
            .filter_map(|part| {
                if let Some(pos) = part.find(':') {
                    Some(part[..pos].trim_matches('\'').to_string())
                } else {
                    Some(part.trim_matches('\'').to_string())
                }
            })
            .collect();

        let array_str = format!("{{{}}}", lexemes.join(","));
        Ok(SqlValue::Text(array_str))
    }

    /// ts_delete(tsvector, lexeme) / ts_delete(tsvector, lexeme[]) - remove lexemes
    fn evaluate_ts_delete(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ts_delete() requires exactly 2 arguments".to_string(),
            ));
        }

        let tsvector = self.sqlvalue_to_string(&args[0])?;
        let to_delete = self.sqlvalue_to_string(&args[1])?;

        // Parse lexemes to delete
        let delete_set: std::collections::HashSet<String> = if to_delete.starts_with('{') {
            to_delete
                .trim_matches(|c| c == '{' || c == '}')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_lowercase())
                .collect()
        } else {
            std::iter::once(to_delete.to_lowercase()).collect()
        };

        // Filter tsvector
        let filtered: Vec<&str> = tsvector
            .split_whitespace()
            .filter(|part| {
                let lexeme = if let Some(pos) = part.find(':') {
                    part[..pos].trim_matches('\'')
                } else {
                    part.trim_matches('\'')
                };
                !delete_set.contains(&lexeme.to_lowercase())
            })
            .collect();

        Ok(SqlValue::Text(filtered.join(" ")))
    }

    /// ts_filter(tsvector, weights) - filter tsvector by weights
    fn evaluate_ts_filter(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 2 {
            return Err(ProtocolError::PostgresError(
                "ts_filter() requires exactly 2 arguments".to_string(),
            ));
        }

        let tsvector = self.sqlvalue_to_string(&args[0])?;
        let weights = self.sqlvalue_to_string(&args[1])?;

        // Parse weight filter
        let allowed_weights: std::collections::HashSet<char> = weights
            .trim_matches(|c| c == '{' || c == '}')
            .chars()
            .filter(|c| ['A', 'B', 'C', 'D'].contains(&c.to_ascii_uppercase()))
            .map(|c| c.to_ascii_uppercase())
            .collect();

        // Filter tsvector by weights (if no weights specified, keep all D weight entries)
        let filtered: Vec<&str> = tsvector
            .split_whitespace()
            .filter(|part| {
                // Check if this entry has a matching weight
                if let Some(colon_pos) = part.find(':') {
                    let after_colon = &part[colon_pos + 1..];
                    // Look for weight letter at end of positions
                    for c in after_colon.chars() {
                        if ['A', 'B', 'C', 'D'].contains(&c.to_ascii_uppercase()) {
                            return allowed_weights.contains(&c.to_ascii_uppercase());
                        }
                    }
                    // No explicit weight means D
                    allowed_weights.contains(&'D')
                } else {
                    allowed_weights.contains(&'D')
                }
            })
            .collect();

        Ok(SqlValue::Text(filtered.join(" ")))
    }

    /// tsquery_phrase(query1, query2 [, distance]) - create phrase query
    fn evaluate_tsquery_phrase(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::PostgresError(
                "tsquery_phrase() requires 2 or 3 arguments".to_string(),
            ));
        }

        let query1 = self.sqlvalue_to_string(&args[0])?;
        let query2 = self.sqlvalue_to_string(&args[1])?;

        let distance = if args.len() == 3 {
            match &args[2] {
                SqlValue::Integer(n) => *n as usize,
                _ => 1,
            }
        } else {
            1
        };

        // Create phrase query with distance operator
        let phrase = if distance == 1 {
            format!("{} <-> {}", query1, query2)
        } else {
            format!("{} <{}> {}", query1, distance, query2)
        };

        Ok(SqlValue::Text(phrase))
    }

    // ===== FTS Helper Functions =====

    /// Tokenize text into lexemes with positions
    fn tokenize_text(&self, text: &str, config: &str) -> Vec<(String, Vec<u32>)> {
        let mut tokens: std::collections::HashMap<String, Vec<u32>> =
            std::collections::HashMap::new();

        for (pos, word) in text.split_whitespace().enumerate() {
            let lexeme = self.stem_word(word, config);
            if !lexeme.is_empty() {
                tokens
                    .entry(lexeme)
                    .or_insert_with(Vec::new)
                    .push((pos + 1) as u32);
            }
        }

        let mut result: Vec<(String, Vec<u32>)> = tokens.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Simple word stemming (lowercase + basic suffix removal)
    fn stem_word(&self, word: &str, _config: &str) -> String {
        let lower = word
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>();

        // Very basic Porter-style stemming for common suffixes
        let stemmed = if lower.ends_with("ies") && lower.len() > 4 {
            format!("{}y", &lower[..lower.len() - 3])
        } else if lower.ends_with("es") && lower.len() > 3 {
            lower[..lower.len() - 2].to_string()
        } else if lower.ends_with("s") && lower.len() > 2 && !lower.ends_with("ss") {
            lower[..lower.len() - 1].to_string()
        } else if lower.ends_with("ing") && lower.len() > 5 {
            lower[..lower.len() - 3].to_string()
        } else if lower.ends_with("ed") && lower.len() > 4 {
            lower[..lower.len() - 2].to_string()
        } else {
            lower
        };

        stemmed
    }

    /// Normalize tsquery string
    fn normalize_tsquery(&self, query: &str, config: &str) -> String {
        // Parse and normalize query operators
        let mut result = String::new();
        let mut in_word = false;
        let mut current_word = String::new();

        for c in query.chars() {
            match c {
                '&' | '|' | '!' | '(' | ')' | '<' | '>' => {
                    if !current_word.is_empty() {
                        let stemmed = self.stem_word(&current_word, config);
                        if !stemmed.is_empty() {
                            result.push_str(&format!("'{}'", stemmed));
                        }
                        current_word.clear();
                    }
                    result.push(' ');
                    result.push(c);
                    result.push(' ');
                    in_word = false;
                }
                ' ' | '\t' | '\n' => {
                    if !current_word.is_empty() {
                        let stemmed = self.stem_word(&current_word, config);
                        if !stemmed.is_empty() {
                            result.push_str(&format!("'{}'", stemmed));
                        }
                        current_word.clear();
                    }
                    in_word = false;
                }
                '\'' => {
                    // Skip quotes
                }
                _ => {
                    if !in_word && !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    current_word.push(c);
                    in_word = true;
                }
            }
        }

        if !current_word.is_empty() {
            let stemmed = self.stem_word(&current_word, config);
            if !stemmed.is_empty() {
                result.push_str(&format!("'{}'", stemmed));
            }
        }

        // Clean up extra spaces
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Add weight to tsvector lexemes
    fn add_weight_to_tsvector(&self, tsvector: &str, weight: char) -> String {
        tsvector
            .split_whitespace()
            .map(|part| {
                if let Some(colon_pos) = part.find(':') {
                    // Replace or add weight to positions
                    let lexeme = &part[..colon_pos];
                    let positions = &part[colon_pos + 1..];
                    let new_positions: Vec<String> = positions
                        .split(',')
                        .map(|p| {
                            let num: String = p.chars().take_while(|c| c.is_numeric()).collect();
                            format!("{}{}", num, weight)
                        })
                        .collect();
                    format!("{}:{}", lexeme, new_positions.join(","))
                } else {
                    format!("{}:{}", part, weight)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Calculate ts_rank score
    fn calculate_ts_rank(&self, vector: &str, query: &str, cover_density: bool) -> f64 {
        let query_terms = self.extract_query_terms(query);
        if query_terms.is_empty() {
            return 0.0;
        }

        // Parse tsvector
        let mut matches = 0;
        let mut total_positions = 0;
        let mut min_pos = u32::MAX;
        let mut max_pos = 0u32;

        for part in vector.split_whitespace() {
            if let Some(colon_pos) = part.find(':') {
                let lexeme = part[..colon_pos].trim_matches('\'').to_lowercase();
                let positions: Vec<u32> = part[colon_pos + 1..]
                    .split(',')
                    .filter_map(|p| p.chars().take_while(|c| c.is_numeric()).collect::<String>().parse().ok())
                    .collect();

                if query_terms.contains(&lexeme) {
                    matches += 1;
                    for pos in &positions {
                        total_positions += 1;
                        min_pos = min_pos.min(*pos);
                        max_pos = max_pos.max(*pos);
                    }
                }
            }
        }

        if matches == 0 {
            return 0.0;
        }

        if cover_density && max_pos > min_pos {
            // Cover density: favor documents where matching terms are close together
            let span = (max_pos - min_pos + 1) as f64;
            (matches as f64 * total_positions as f64) / span
        } else {
            // Standard rank: based on term frequency
            matches as f64 / query_terms.len() as f64
        }
    }

    /// Extract query terms from tsquery
    fn extract_query_terms(&self, query: &str) -> std::collections::HashSet<String> {
        query
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_matches('\'').to_lowercase())
            .filter(|s| !s.is_empty() && s != "and" && s != "or" && s != "not")
            .collect()
    }

    /// Highlight matching terms in text
    fn highlight_text(&self, text: &str, terms: &std::collections::HashSet<String>, start_tag: &str, end_tag: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let highlighted: Vec<String> = words
            .iter()
            .map(|word| {
                let clean = word.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                if terms.contains(&clean) {
                    format!("{}{}{}", start_tag, word, end_tag)
                } else {
                    word.to_string()
                }
            })
            .collect();
        highlighted.join(" ")
    }

    /// Merge two tsvectors
    fn merge_tsvectors(&self, vec1: &str, vec2: &str) -> String {
        let mut lexemes: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for part in vec1.split_whitespace().chain(vec2.split_whitespace()) {
            if let Some(colon_pos) = part.find(':') {
                let lexeme = part[..colon_pos].to_string();
                let positions = part[colon_pos + 1..].to_string();
                lexemes
                    .entry(lexeme)
                    .or_insert_with(Vec::new)
                    .push(positions);
            }
        }

        let mut result: Vec<String> = lexemes
            .into_iter()
            .map(|(lexeme, positions)| format!("{}:{}", lexeme, positions.join(",")))
            .collect();

        result.sort();
        result.join(" ")
    }

    /// Count nodes in tsquery
    fn count_query_nodes(&self, query: &str) -> i32 {
        let mut count = 0;
        let mut in_word = false;

        for c in query.chars() {
            match c {
                '&' | '|' | '!' => count += 1,
                '\'' => {
                    if !in_word {
                        count += 1;
                        in_word = true;
                    } else {
                        in_word = false;
                    }
                }
                _ => {}
            }
        }

        count.max(1)
    }

    /// Strip positions and weights from tsvector
    fn strip_tsvector(&self, tsvector: &str) -> String {
        tsvector
            .split_whitespace()
            .map(|part| {
                if let Some(colon_pos) = part.find(':') {
                    part[..colon_pos].to_string()
                } else {
                    part.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Convert SqlValue to String for FTS functions
    fn sqlvalue_to_string(&self, value: &SqlValue) -> ProtocolResult<String> {
        match value {
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Ok(s.clone()),
            SqlValue::Null => Ok(String::new()),
            other => Ok(format!("{:?}", other)),
        }
    }

    // ===== Text Search Operators =====

    /// @@ operator: tsvector matches tsquery
    fn text_search_match(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Tsvector(vec), SqlValue::Tsquery(query))
            | (SqlValue::Tsquery(query), SqlValue::Tsvector(vec)) => {
                // Check if tsvector matches tsquery
                let vec_lexemes: std::collections::HashSet<String> =
                    vec.iter().map(|e| e.lexeme.to_lowercase()).collect();

                // Parse query terms from tsquery string
                let query_terms = self.extract_query_terms(query);

                // Simple match: check if any query term is in vector
                let matches = query_terms.iter().any(|term| vec_lexemes.contains(term));
                Ok(SqlValue::Boolean(matches))
            }
            (SqlValue::Text(tsvec), SqlValue::Text(tsquery))
            | (SqlValue::Varchar(tsvec), SqlValue::Text(tsquery))
            | (SqlValue::Text(tsvec), SqlValue::Varchar(tsquery)) => {
                // String-based comparison
                let vec_terms = self.extract_tsvector_lexemes(tsvec);
                let query_terms = self.extract_query_terms(tsquery);

                let matches = query_terms.iter().any(|term| vec_terms.contains(term));
                Ok(SqlValue::Boolean(matches))
            }
            _ => Err(ProtocolError::PostgresError(
                "@@ operator requires tsvector and tsquery arguments".to_string(),
            )),
        }
    }

    /// @> operator: tsquery contains tsquery
    fn text_search_contains(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Tsquery(left_q), SqlValue::Tsquery(right_q)) => {
                let left_terms = self.extract_query_terms(left_q);
                let right_terms = self.extract_query_terms(right_q);

                // Left contains right if all right terms are in left
                let contains = right_terms.iter().all(|term| left_terms.contains(term));
                Ok(SqlValue::Boolean(contains))
            }
            (SqlValue::Text(left_s), SqlValue::Text(right_s)) => {
                let left_terms = self.extract_query_terms(left_s);
                let right_terms = self.extract_query_terms(right_s);

                let contains = right_terms.iter().all(|term| left_terms.contains(term));
                Ok(SqlValue::Boolean(contains))
            }
            _ => Err(ProtocolError::PostgresError(
                "@> text search operator requires tsquery arguments".to_string(),
            )),
        }
    }

    /// <@ operator: tsquery is contained by tsquery
    fn text_search_contained_by(
        &self,
        left: &SqlValue,
        right: &SqlValue,
    ) -> ProtocolResult<SqlValue> {
        // Reverse of contains
        self.text_search_contains(right, left)
    }

    /// || operator: concatenate tsvectors or tsqueries
    fn text_search_concat(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, other) | (other, SqlValue::Null) => Ok(other.clone()),
            (SqlValue::Tsvector(vec1), SqlValue::Tsvector(vec2)) => {
                // Merge tsvectors
                let mut merged = vec1.clone();
                merged.extend(vec2.clone());
                Ok(SqlValue::Tsvector(merged))
            }
            (SqlValue::Tsquery(q1), SqlValue::Tsquery(q2)) => {
                // OR the tsqueries together
                let combined = format!("{} | {}", q1, q2);
                Ok(SqlValue::Tsquery(combined))
            }
            (SqlValue::Text(t1), SqlValue::Text(t2)) => {
                // Concatenate as tsvector strings
                let merged = self.merge_tsvectors(t1, t2);
                Ok(SqlValue::Text(merged))
            }
            _ => Err(ProtocolError::PostgresError(
                "|| text search operator requires tsvector or tsquery arguments".to_string(),
            )),
        }
    }

    /// && operator: AND tsqueries
    fn text_search_and(&self, left: &SqlValue, right: &SqlValue) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Tsquery(q1), SqlValue::Tsquery(q2)) => {
                let combined = format!("{} & {}", q1, q2);
                Ok(SqlValue::Tsquery(combined))
            }
            (SqlValue::Text(t1), SqlValue::Text(t2)) => {
                let combined = format!("{} & {}", t1, t2);
                Ok(SqlValue::Text(combined))
            }
            _ => Err(ProtocolError::PostgresError(
                "&& text search operator requires tsquery arguments".to_string(),
            )),
        }
    }

    /// !! operator: negate tsquery
    fn text_search_not(&self, left: &SqlValue, _right: &SqlValue) -> ProtocolResult<SqlValue> {
        match left {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Tsquery(q) => {
                let negated = format!("!{}", q);
                Ok(SqlValue::Tsquery(negated))
            }
            SqlValue::Text(t) => {
                let negated = format!("!{}", t);
                Ok(SqlValue::Text(negated))
            }
            _ => Err(ProtocolError::PostgresError(
                "!! text search operator requires tsquery argument".to_string(),
            )),
        }
    }

    /// <-> operator: phrase search (followed by)
    fn text_search_followed_by(
        &self,
        left: &SqlValue,
        right: &SqlValue,
    ) -> ProtocolResult<SqlValue> {
        match (left, right) {
            (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
            (SqlValue::Tsquery(q1), SqlValue::Tsquery(q2)) => {
                let phrase = format!("{} <-> {}", q1, q2);
                Ok(SqlValue::Tsquery(phrase))
            }
            (SqlValue::Text(t1), SqlValue::Text(t2)) => {
                let phrase = format!("{} <-> {}", t1, t2);
                Ok(SqlValue::Text(phrase))
            }
            _ => Err(ProtocolError::PostgresError(
                "<-> text search operator requires tsquery arguments".to_string(),
            )),
        }
    }

    /// Extract lexemes from tsvector string
    fn extract_tsvector_lexemes(&self, tsvector: &str) -> std::collections::HashSet<String> {
        tsvector
            .split_whitespace()
            .map(|part| {
                if let Some(colon_pos) = part.find(':') {
                    part[..colon_pos].trim_matches('\'').to_lowercase()
                } else {
                    part.trim_matches('\'').to_lowercase()
                }
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
