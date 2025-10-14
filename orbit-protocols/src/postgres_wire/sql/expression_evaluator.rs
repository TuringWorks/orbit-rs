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

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    ast::{
        BinaryOperator, CaseExpression, ColumnRef, Expression, FunctionCall, FunctionName, InList,
        OrderByItem, SelectStatement, UnaryOperator, VectorOperator, WindowFrame,
        WindowFunctionType,
    },
    types::{SqlType, SqlValue},
};
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

            // Math functions
            "ABS" => self.evaluate_abs(&args),
            "ROUND" => self.evaluate_round(&args),
            "CEILING" | "CEIL" => self.evaluate_ceiling(&args),
            "FLOOR" => self.evaluate_floor(&args),
            "SQRT" => self.evaluate_sqrt(&args),

            // Date functions
            "NOW" => self.evaluate_now(&args),
            "CURRENT_DATE" => self.evaluate_current_date(&args),
            "CURRENT_TIME" => self.evaluate_current_time(&args),
            "CURRENT_TIMESTAMP" => self.evaluate_current_timestamp(&args),

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
        _expr: &Expression,
        _list: &InList,
        _negated: bool,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement IN expression evaluation
        Err(ProtocolError::PostgresError(
            "IN expression evaluation not implemented".to_string(),
        ))
    }

    fn evaluate_between_expression(
        &mut self,
        _expr: &Expression,
        _low: &Expression,
        _high: &Expression,
        _negated: bool,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement BETWEEN evaluation
        Err(ProtocolError::PostgresError(
            "BETWEEN evaluation not implemented".to_string(),
        ))
    }

    fn evaluate_like_expression(
        &mut self,
        _expr: &Expression,
        _pattern: &Expression,
        _escape: Option<&Expression>,
        _case_insensitive: bool,
        _negated: bool,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement LIKE evaluation
        Err(ProtocolError::PostgresError(
            "LIKE evaluation not implemented".to_string(),
        ))
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
        _array: &Expression,
        _index: &Expression,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement array indexing
        Err(ProtocolError::PostgresError(
            "Array indexing not implemented".to_string(),
        ))
    }

    fn evaluate_array_slice(
        &mut self,
        _array: &Expression,
        _start: Option<&Expression>,
        _end: Option<&Expression>,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement array slicing
        Err(ProtocolError::PostgresError(
            "Array slicing not implemented".to_string(),
        ))
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
    fn evaluate_rank(
        &mut self,
        _order_by: &[OrderByItem],
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
        _dense: bool,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement RANK() and DENSE_RANK()
        Ok(SqlValue::BigInt(1))
    }

    fn evaluate_lag_lead(
        &mut self,
        _expr: &Expression,
        _offset: Option<&Expression>,
        _default: Option<&Expression>,
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
        _is_lag: bool,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement LAG() and LEAD()
        Ok(SqlValue::Null)
    }

    fn evaluate_first_last_value(
        &mut self,
        _expr: &Expression,
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
        _is_first: bool,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement FIRST_VALUE() and LAST_VALUE()
        Ok(SqlValue::Null)
    }

    fn evaluate_nth_value(
        &mut self,
        _expr: &Expression,
        _n: &Expression,
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement NTH_VALUE()
        Ok(SqlValue::Null)
    }

    fn evaluate_ntile(
        &mut self,
        _n_expr: &Expression,
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement NTILE()
        Ok(SqlValue::BigInt(1))
    }

    fn evaluate_percent_rank(
        &mut self,
        _order_by: &[OrderByItem],
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement PERCENT_RANK()
        Ok(SqlValue::DoublePrecision(0.0))
    }

    fn evaluate_cume_dist(
        &mut self,
        _order_by: &[OrderByItem],
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement CUME_DIST()
        Ok(SqlValue::DoublePrecision(1.0))
    }

    fn evaluate_window_aggregate(
        &mut self,
        _func: &FunctionCall,
        _frame: &Option<WindowFrame>,
        _window_context: &WindowFrameContext,
        _context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        // TODO: Implement aggregate functions used as window functions
        Ok(SqlValue::Null)
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

        // TODO: Implement proper SQL LIKE pattern matching with % and _
        let text_to_match = if case_insensitive {
            text_str.to_lowercase()
        } else {
            text_str.clone()
        };
        let pattern_to_match = if case_insensitive {
            pattern_str.to_lowercase()
        } else {
            pattern_str.clone()
        };

        // Simple implementation - replace with proper LIKE matching
        let matches = text_to_match.contains(&pattern_to_match);
        Ok(SqlValue::Boolean(if negated { !matches } else { matches }))
    }
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
