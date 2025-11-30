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
}

/// Aggregate function state
#[derive(Debug, Clone)]
pub enum AggregateState {
    Count(i64),
    Sum(SqlValue),
    Min(SqlValue),
    Max(SqlValue),
    Avg { sum: SqlValue, count: i64 },
}

/// Expression evaluator
pub struct ExpressionEvaluator {
    #[allow(dead_code)]
    aggregates: HashMap<String, AggregateState>,
}

impl ExpressionEvaluator {
    pub fn new() -> Self {
        Self {
            aggregates: HashMap::new(),
        }
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

            // String functions
            "LENGTH" | "CHAR_LENGTH" => self.evaluate_length(&args),
            "UPPER" => self.evaluate_upper(&args),
            "LOWER" => self.evaluate_lower(&args),
            "SUBSTRING" => self.evaluate_substring(&args),
            "REPLACE" => self.evaluate_replace(&args),
            "CONCAT" => self.evaluate_concat(&args),

            // Math functions
            "ABS" => self.evaluate_abs(&args),
            "ROUND" => self.evaluate_round(&args),
            "CEILING" | "CEIL" => self.evaluate_ceiling(&args),
            "FLOOR" => self.evaluate_floor(&args),
            "SQRT" => self.evaluate_sqrt(&args),

            // Date functions
            "NOW" => self.evaluate_now(&args),
            "CURRENT_DATE" | "CURDATE" => self.evaluate_current_date(&args),
            "CURRENT_TIME" => self.evaluate_current_time(&args),
            "CURRENT_TIMESTAMP" => self.evaluate_current_timestamp(&args),
            "YEAR" => self.evaluate_year(&args),
            "MONTH" => self.evaluate_month(&args),
            "DAY" | "DAYOFMONTH" => self.evaluate_day(&args),

            // Vector functions
            "VECTOR_DIMS" => self.evaluate_vector_dims(&args),
            "VECTOR_NORM" => self.evaluate_vector_norm(&args),

            // Conditional functions
            "COALESCE" => self.evaluate_coalesce(&args),
            "NULLIF" => self.evaluate_nullif(&args),
            "GREATEST" => self.evaluate_greatest(&args),
            "LEAST" => self.evaluate_least(&args),

            _ => Err(ProtocolError::not_implemented("Function", &func_name)),
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

    fn evaluate_year(&self, args: &[SqlValue]) -> ProtocolResult<SqlValue> {
        if args.len() != 1 {
            return Err(ProtocolError::PostgresError(
                "YEAR requires exactly one argument".to_string(),
            ));
        }

        match &args[0] {
            SqlValue::Date(date) => Ok(SqlValue::Integer(date.year() as i32)),
            SqlValue::TimestampWithTimezone(ts) => {
                let date = ts.date_naive();
                Ok(SqlValue::Integer(date.year() as i32))
            }
            SqlValue::Text(s) => {
                // Try to parse as date
                if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(SqlValue::Integer(date.year() as i32))
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
        let mut chars = pattern.chars().peekable();
        let mut escaped = false;

        while let Some(c) = chars.next() {
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
            &mut text_chars.collect::<Vec<_>>(),
            &mut pattern_chars.collect::<Vec<_>>(),
            0,
            0,
        )
    }

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
                match self.compare_values(rv, cv)? {
                    Ordering::Greater => {
                        is_le = false;
                        break;
                    }
                    _ => {}
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

        // Determine frame bounds
        let (start_offset, end_offset) = if let Some(window_frame) = frame {
            self.get_frame_bounds(window_frame, partition_rows.len(), current_idx)?
        } else {
            // Default frame: RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
            // For simplicity, we'll use the entire partition up to current row
            let pos = partition_rows
                .iter()
                .position(|&idx| idx == current_idx)
                .unwrap_or(0);
            (0, pos)
        };

        // Collect values from frame rows
        let mut frame_values = Vec::new();
        for i in start_offset..=end_offset {
            if i < partition_rows.len() {
                let row_idx = partition_rows[i];
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
    fn get_frame_bounds(
        &mut self,
        frame: &WindowFrame,
        partition_size: usize,
        current_pos: usize,
    ) -> ProtocolResult<(usize, usize)> {
        #[allow(unused_imports)]
        use crate::protocols::postgres_wire::sql::ast::FrameBound;

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
                } else {
                    if is_start {
                        0
                    } else {
                        pos
                    }
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
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
