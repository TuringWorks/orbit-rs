//! Interpreter for Stored Procedures

use super::ast::*;
use crate::error::{EngineError, EngineResult};
use crate::query::{Query, QueryExecutor};
use crate::storage::QueryResult;
use crate::storage::SqlValue;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use std::collections::HashMap;
use std::sync::Arc;

/// Value type for interpreter
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    /// Date value (year, month, day)
    Date(NaiveDate),
    /// Time value (hour, minute, second, nanosecond)
    Time(NaiveTime),
    /// Timestamp value (date + time)
    Timestamp(NaiveDateTime),
    /// Interval/Duration value
    Interval(Duration),
    /// Array of values
    Array(Vec<Value>),
}

impl From<Literal> for Value {
    fn from(lit: Literal) -> Self {
        match lit {
            Literal::Integer(i) => Value::Integer(i),
            Literal::Float(f) => Value::Float(f),
            Literal::String(s) => Value::String(s),
            Literal::Boolean(b) => Value::Boolean(b),
            Literal::Null => Value::Null,
        }
    }
}

/// Symbol Table for variable scope
#[derive(Debug, Default)]
pub struct SymbolTable {
    variables: HashMap<String, Value>,
    parent: Option<Box<SymbolTable>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.variables.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn set(&mut self, name: String, value: Value) {
        // If variable exists in current scope, update it
        if self.variables.contains_key(&name) {
            self.variables.insert(name, value);
        } else if let Some(parent) = &mut self.parent {
            // Try to update in parent scope
            // Note: This is a simplified implementation.
            // In a real implementation with Box<SymbolTable>, we'd need RefCell or similar for mutable parent access.
            // For now, we'll just set in current scope if not found in parent (shadowing/new var)
            // or we need a better scope design (e.g. vector of scopes).
            self.variables.insert(name, value);
        } else {
            self.variables.insert(name, value);
        }
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }
}

/// Procedure Interpreter
pub struct ProcedureInterpreter {
    query_executor: Arc<dyn QueryExecutor>,
}

impl ProcedureInterpreter {
    pub fn new(query_executor: Arc<dyn QueryExecutor>) -> Self {
        Self { query_executor }
    }

    pub async fn execute(&self, procedure: &ProcedureDef, args: Vec<Value>) -> EngineResult<Value> {
        self.execute_with_context(procedure, args, HashMap::new())
            .await
    }

    pub async fn execute_with_context(
        &self,
        procedure: &ProcedureDef,
        args: Vec<Value>,
        context: HashMap<String, Value>,
    ) -> EngineResult<Value> {
        let mut scope = SymbolTable::new();

        // Bind arguments
        if procedure.args.len() != args.len() {
            return Err(EngineError::ExecutionError(format!(
                "Argument count mismatch: expected {}, got {}",
                procedure.args.len(),
                args.len()
            )));
        }

        for (arg_def, arg_val) in procedure.args.iter().zip(args.into_iter()) {
            scope.define(arg_def.name.clone(), arg_val);
        }

        // Inject context variables
        for (name, val) in context {
            scope.define(name, val);
        }

        self.execute_block(&procedure.body, &mut scope).await
    }

    fn execute_block<'a>(
        &'a self,
        block: &'a Block,
        scope: &'a mut SymbolTable,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = EngineResult<Value>> + Send + 'a>> {
        Box::pin(async move {
            for stmt in &block.statements {
                if let Some(ret_val) = self.execute_statement(stmt, scope).await? {
                    return Ok(ret_val);
                }
            }
            Ok(Value::Null)
        })
    }

    fn execute_statement<'a>(
        &'a self,
        stmt: &'a Statement,
        scope: &'a mut SymbolTable,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = EngineResult<Option<Value>>> + Send + 'a>> {
        Box::pin(async move {
        match stmt {
            Statement::Declaration {
                name,
                type_name: _,
                default_value,
            } => {
                let value = if let Some(expr) = default_value {
                    self.evaluate_expression(expr, scope).await?
                } else {
                    Value::Null
                };
                scope.define(name.clone(), value);
                Ok(None)
            }
            Statement::Assignment { name, value } => {
                let val = self.evaluate_expression(value, scope).await?;
                scope.set(name.clone(), val);
                Ok(None)
            }
            Statement::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond_val = self.evaluate_expression(condition, scope).await?;
                let is_true = match cond_val {
                    Value::Boolean(b) => b,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "Condition must be boolean".to_string(),
                        ))
                    }
                };

                if is_true {
                    self.execute_block(then_block, scope).await.map(Some)
                } else if let Some(else_blk) = else_block {
                    self.execute_block(else_blk, scope).await.map(Some)
                } else {
                    Ok(None)
                }
            }
            Statement::Return { value } => {
                let ret_val = if let Some(expr) = value {
                    self.evaluate_expression(expr, scope).await?
                } else {
                    Value::Null
                };
                Ok(Some(ret_val))
            }
            Statement::Loop { body } => {
                // Infinite loop - must be terminated by a RETURN statement
                // In PL/pgSQL, loops can also be terminated by EXIT, but we don't support that yet
                const MAX_ITERATIONS: usize = 100_000;
                let mut iteration = 0;
                loop {
                    iteration += 1;
                    if iteration > MAX_ITERATIONS {
                        return Err(EngineError::ExecutionError(
                            "Loop exceeded maximum iterations (possible infinite loop)".to_string(),
                        ));
                    }

                    let result = self.execute_block(body, scope).await?;
                    // If block returns a value (via RETURN), propagate it
                    if result != Value::Null {
                        return Ok(Some(result));
                    }
                }
            }
            Statement::While { condition, body } => {
                const MAX_ITERATIONS: usize = 100_000;
                let mut iteration = 0;
                loop {
                    iteration += 1;
                    if iteration > MAX_ITERATIONS {
                        return Err(EngineError::ExecutionError(
                            "While loop exceeded maximum iterations (possible infinite loop)"
                                .to_string(),
                        ));
                    }

                    // Evaluate condition
                    let cond_val = self.evaluate_expression(condition, scope).await?;
                    let is_true = match cond_val {
                        Value::Boolean(b) => b,
                        Value::Null => false, // NULL is falsy in loop conditions
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "While condition must be boolean".to_string(),
                            ))
                        }
                    };

                    if !is_true {
                        break;
                    }

                    // Execute body
                    let result = self.execute_block(body, scope).await?;
                    // If block returns a value (via RETURN), propagate it
                    if result != Value::Null {
                        return Ok(Some(result));
                    }
                }
                Ok(None)
            }
            Statement::Raise { level, message } => {
                // Handle RAISE statements
                match level.to_uppercase().as_str() {
                    "NOTICE" | "INFO" | "DEBUG" | "LOG" | "WARNING" => {
                        // In a real implementation, we'd log this appropriately
                        // For now, we just continue execution
                        tracing::info!("RAISE {}: {}", level, message);
                        Ok(None)
                    }
                    "EXCEPTION" | "ERROR" => {
                        // RAISE EXCEPTION terminates execution with an error
                        Err(EngineError::ExecutionError(format!(
                            "RAISE {}: {}",
                            level, message
                        )))
                    }
                    _ => {
                        tracing::warn!("Unknown RAISE level {}: {}", level, message);
                        Ok(None)
                    }
                }
            }
            Statement::Sql { query, params: _ } => {
                // Execute SQL via query engine
                // Note: We currently ignore params in this MVP integration
                // In a real implementation, we'd bind params to the query

                // Parse query string into Query struct (simplified)
                // For MVP, we assume the query string IS the table name for a simple scan
                // or we need a proper SQL parser here.
                // Since we don't have a SQL parser exposed here, we'll assume the query string
                // is a simple "SELECT * FROM table" and extract the table name.

                let table_name = if query.to_uppercase().starts_with("SELECT * FROM ") {
                    query[14..].trim().to_string()
                } else {
                    query.clone()
                };

                let query_obj = Query {
                    table: table_name,
                    projection: None,
                    filter: None,
                    limit: None,
                    offset: None,
                };

                let result = self.query_executor.execute(query_obj).await?;

                // Convert QueryResult to Value
                match result {
                    QueryResult::Aggregate(value) => Ok(Some(Self::sql_value_to_value(value))),
                    QueryResult::Rows(rows) => {
                        // Return first value of first row for now (scalar context)
                        if let Some(first_row) = rows.first() {
                            if let Some((_, first_val)) = first_row.iter().next() {
                                Ok(Some(Self::sql_value_to_value(first_val.clone())))
                            } else {
                                Ok(Some(Value::Null))
                            }
                        } else {
                            Ok(Some(Value::Null))
                        }
                    }
                    QueryResult::ColumnBatch(_) => Ok(Some(Value::Null)), // TODO: Handle column batch
                    QueryResult::Empty => Ok(Some(Value::Null)),
                }
            }
        }
        })
    }

    fn evaluate_expression<'a>(
        &'a self,
        expr: &'a Expression,
        scope: &'a SymbolTable,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = EngineResult<Value>> + Send + 'a>> {
        Box::pin(async move {
            match expr {
                Expression::Literal(lit) => Ok(Value::from(lit.clone())),
                Expression::Variable(name) => scope.get(name).ok_or_else(|| {
                    EngineError::ExecutionError(format!("Variable not found: {}", name))
                }),
                Expression::BinaryOp { left, op, right } => {
                    let l = self.evaluate_expression(left, scope).await?;
                    let r = self.evaluate_expression(right, scope).await?;
                    self.evaluate_binary_op(l, op, r)
                }
                Expression::UnaryOp { op, operand } => {
                    let val = self.evaluate_expression(operand, scope).await?;
                    self.evaluate_unary_op(op, val)
                }
                Expression::FunctionCall { name, args } => {
                    // Evaluate all arguments
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for arg in args {
                        evaluated_args.push(self.evaluate_expression(arg, scope).await?);
                    }
                    self.evaluate_builtin_function(name, evaluated_args)
                }
            }
        })
    }

    fn evaluate_unary_op(&self, op: &UnaryOperator, val: Value) -> EngineResult<Value> {
        match (op, val) {
            (UnaryOperator::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
            (UnaryOperator::Not, Value::Null) => Ok(Value::Null),
            (UnaryOperator::Negate, Value::Integer(i)) => Ok(Value::Integer(-i)),
            (UnaryOperator::Negate, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOperator::Negate, Value::Null) => Ok(Value::Null),
            (op, val) => Err(EngineError::ExecutionError(format!(
                "Unsupported unary operation: {:?} on {:?}",
                op, val
            ))),
        }
    }

    fn evaluate_builtin_function(&self, name: &str, args: Vec<Value>) -> EngineResult<Value> {
        match name.to_uppercase().as_str() {
            // Math functions
            "ABS" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "ABS requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(i.abs())),
                    Value::Float(f) => Ok(Value::Float(f.abs())),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ABS requires numeric argument".to_string(),
                    )),
                }
            }
            "CEIL" | "CEILING" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "CEIL requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(*i)),
                    Value::Float(f) => Ok(Value::Integer(f.ceil() as i64)),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "CEIL requires numeric argument".to_string(),
                    )),
                }
            }
            "FLOOR" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "FLOOR requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(*i)),
                    Value::Float(f) => Ok(Value::Integer(f.floor() as i64)),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "FLOOR requires numeric argument".to_string(),
                    )),
                }
            }
            "ROUND" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(EngineError::ExecutionError(
                        "ROUND requires 1 or 2 arguments".to_string(),
                    ));
                }
                let precision = if args.len() == 2 {
                    match &args[1] {
                        Value::Integer(p) => *p as i32,
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "ROUND precision must be integer".to_string(),
                            ))
                        }
                    }
                } else {
                    0
                };
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(*i)),
                    Value::Float(f) => {
                        let multiplier = 10_f64.powi(precision);
                        Ok(Value::Float((f * multiplier).round() / multiplier))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ROUND requires numeric argument".to_string(),
                    )),
                }
            }
            "MOD" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "MOD requires 2 arguments".to_string(),
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Integer(a), Value::Integer(b)) => {
                        if *b == 0 {
                            Err(EngineError::ExecutionError(
                                "Division by zero in MOD".to_string(),
                            ))
                        } else {
                            Ok(Value::Integer(a % b))
                        }
                    }
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "MOD requires integer arguments".to_string(),
                    )),
                }
            }
            "POWER" | "POW" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "POWER requires 2 arguments".to_string(),
                    ));
                }
                let base = match &args[0] {
                    Value::Integer(i) => *i as f64,
                    Value::Float(f) => *f,
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "POWER requires numeric arguments".to_string(),
                        ))
                    }
                };
                let exp = match &args[1] {
                    Value::Integer(i) => *i as f64,
                    Value::Float(f) => *f,
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "POWER requires numeric arguments".to_string(),
                        ))
                    }
                };
                Ok(Value::Float(base.powf(exp)))
            }
            "SQRT" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "SQRT requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Float((*i as f64).sqrt())),
                    Value::Float(f) => Ok(Value::Float(f.sqrt())),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "SQRT requires numeric argument".to_string(),
                    )),
                }
            }

            // String functions
            "LENGTH" | "CHAR_LENGTH" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "LENGTH requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::Integer(s.len() as i64)),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "LENGTH requires string argument".to_string(),
                    )),
                }
            }
            "LOWER" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "LOWER requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::String(s.to_lowercase())),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "LOWER requires string argument".to_string(),
                    )),
                }
            }
            "UPPER" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "UPPER requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::String(s.to_uppercase())),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "UPPER requires string argument".to_string(),
                    )),
                }
            }
            "TRIM" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "TRIM requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::String(s.trim().to_string())),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "TRIM requires string argument".to_string(),
                    )),
                }
            }
            "CONCAT" => {
                let mut result = String::new();
                for arg in &args {
                    match arg {
                        Value::String(s) => result.push_str(s),
                        Value::Integer(i) => result.push_str(&i.to_string()),
                        Value::Float(f) => result.push_str(&f.to_string()),
                        Value::Boolean(b) => result.push_str(&b.to_string()),
                        Value::Date(d) => result.push_str(&d.to_string()),
                        Value::Time(t) => result.push_str(&t.to_string()),
                        Value::Timestamp(ts) => result.push_str(&ts.to_string()),
                        Value::Interval(i) => result.push_str(&format!("{:?}", i)),
                        Value::Array(arr) => result.push_str(&format!("{:?}", arr)),
                        Value::Null => {} // NULL is ignored in CONCAT
                    }
                }
                Ok(Value::String(result))
            }
            "SUBSTRING" | "SUBSTR" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(EngineError::ExecutionError(
                        "SUBSTRING requires 2 or 3 arguments".to_string(),
                    ));
                }
                let s = match &args[0] {
                    Value::String(s) => s.clone(),
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "SUBSTRING requires string as first argument".to_string(),
                        ))
                    }
                };
                let start = match &args[1] {
                    Value::Integer(i) => (*i as usize).saturating_sub(1), // SQL is 1-indexed
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "SUBSTRING start must be integer".to_string(),
                        ))
                    }
                };
                let len = if args.len() == 3 {
                    match &args[2] {
                        Value::Integer(i) => Some(*i as usize),
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "SUBSTRING length must be integer".to_string(),
                            ))
                        }
                    }
                } else {
                    None
                };

                let chars: Vec<char> = s.chars().collect();
                let result: String = if let Some(l) = len {
                    chars.iter().skip(start).take(l).collect()
                } else {
                    chars.iter().skip(start).collect()
                };
                Ok(Value::String(result))
            }

            // Type conversion
            "COALESCE" => {
                for arg in &args {
                    if *arg != Value::Null {
                        return Ok(arg.clone());
                    }
                }
                Ok(Value::Null)
            }
            "NULLIF" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "NULLIF requires 2 arguments".to_string(),
                    ));
                }
                if args[0] == args[1] {
                    Ok(Value::Null)
                } else {
                    Ok(args[0].clone())
                }
            }

            // Numeric type casting
            "TO_INTEGER" | "TO_INT" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "TO_INTEGER requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(*i)),
                    Value::Float(f) => Ok(Value::Integer(*f as i64)),
                    Value::String(s) => s.parse::<i64>().map(Value::Integer).map_err(|_| {
                        EngineError::ExecutionError(format!("Cannot convert '{}' to integer", s))
                    }),
                    Value::Boolean(b) => Ok(Value::Integer(if *b { 1 } else { 0 })),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "TO_INTEGER cannot convert this type".to_string(),
                    )),
                }
            }
            "TO_FLOAT" | "TO_REAL" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "TO_FLOAT requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Float(*i as f64)),
                    Value::Float(f) => Ok(Value::Float(*f)),
                    Value::String(s) => s.parse::<f64>().map(Value::Float).map_err(|_| {
                        EngineError::ExecutionError(format!("Cannot convert '{}' to float", s))
                    }),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "TO_FLOAT cannot convert this type".to_string(),
                    )),
                }
            }
            "TO_STRING" | "TO_TEXT" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "TO_STRING requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::String(s.clone())),
                    Value::Integer(i) => Ok(Value::String(i.to_string())),
                    Value::Float(f) => Ok(Value::String(f.to_string())),
                    Value::Boolean(b) => Ok(Value::String(b.to_string())),
                    Value::Date(d) => Ok(Value::String(d.format("%Y-%m-%d").to_string())),
                    Value::Time(t) => Ok(Value::String(t.format("%H:%M:%S").to_string())),
                    Value::Timestamp(ts) => {
                        Ok(Value::String(ts.format("%Y-%m-%d %H:%M:%S").to_string()))
                    }
                    Value::Interval(i) => Ok(Value::String(format!("{} seconds", i.num_seconds()))),
                    Value::Array(arr) => {
                        let items: Vec<String> = arr.iter().map(|v| format!("{:?}", v)).collect();
                        Ok(Value::String(format!("[{}]", items.join(", "))))
                    }
                    Value::Null => Ok(Value::Null),
                }
            }

            // Date/Time functions
            "CURRENT_DATE" | "NOW_DATE" => Ok(Value::Date(chrono::Local::now().date_naive())),
            "CURRENT_TIME" | "NOW_TIME" => Ok(Value::Time(chrono::Local::now().time())),
            "CURRENT_TIMESTAMP" | "NOW" => Ok(Value::Timestamp(chrono::Local::now().naive_local())),
            "MAKE_DATE" => {
                if args.len() != 3 {
                    return Err(EngineError::ExecutionError(
                        "MAKE_DATE requires 3 arguments (year, month, day)".to_string(),
                    ));
                }
                let year = match &args[0] {
                    Value::Integer(y) => *y as i32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_DATE year must be integer".to_string(),
                        ))
                    }
                };
                let month = match &args[1] {
                    Value::Integer(m) => *m as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_DATE month must be integer".to_string(),
                        ))
                    }
                };
                let day = match &args[2] {
                    Value::Integer(d) => *d as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_DATE day must be integer".to_string(),
                        ))
                    }
                };
                NaiveDate::from_ymd_opt(year, month, day)
                    .map(Value::Date)
                    .ok_or_else(|| {
                        EngineError::ExecutionError(format!(
                            "Invalid date: {}-{}-{}",
                            year, month, day
                        ))
                    })
            }
            "MAKE_TIME" => {
                if args.len() < 3 || args.len() > 4 {
                    return Err(EngineError::ExecutionError(
                        "MAKE_TIME requires 3-4 arguments (hour, minute, second, [microsecond])"
                            .to_string(),
                    ));
                }
                let hour = match &args[0] {
                    Value::Integer(h) => *h as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_TIME hour must be integer".to_string(),
                        ))
                    }
                };
                let minute = match &args[1] {
                    Value::Integer(m) => *m as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_TIME minute must be integer".to_string(),
                        ))
                    }
                };
                let second = match &args[2] {
                    Value::Integer(s) => *s as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "MAKE_TIME second must be integer".to_string(),
                        ))
                    }
                };
                let micro = if args.len() == 4 {
                    match &args[3] {
                        Value::Integer(m) => *m as u32,
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "MAKE_TIME microsecond must be integer".to_string(),
                            ))
                        }
                    }
                } else {
                    0
                };
                NaiveTime::from_hms_micro_opt(hour, minute, second, micro)
                    .map(Value::Time)
                    .ok_or_else(|| {
                        EngineError::ExecutionError(format!(
                            "Invalid time: {}:{}:{}.{}",
                            hour, minute, second, micro
                        ))
                    })
            }
            "MAKE_TIMESTAMP" => {
                if args.len() != 6 {
                    return Err(EngineError::ExecutionError("MAKE_TIMESTAMP requires 6 arguments (year, month, day, hour, minute, second)".to_string()));
                }
                let year = match &args[0] {
                    Value::Integer(y) => *y as i32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "year must be integer".to_string(),
                        ))
                    }
                };
                let month = match &args[1] {
                    Value::Integer(m) => *m as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "month must be integer".to_string(),
                        ))
                    }
                };
                let day = match &args[2] {
                    Value::Integer(d) => *d as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "day must be integer".to_string(),
                        ))
                    }
                };
                let hour = match &args[3] {
                    Value::Integer(h) => *h as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "hour must be integer".to_string(),
                        ))
                    }
                };
                let minute = match &args[4] {
                    Value::Integer(m) => *m as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "minute must be integer".to_string(),
                        ))
                    }
                };
                let second = match &args[5] {
                    Value::Integer(s) => *s as u32,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "second must be integer".to_string(),
                        ))
                    }
                };
                let date = NaiveDate::from_ymd_opt(year, month, day)
                    .ok_or_else(|| EngineError::ExecutionError("Invalid date".to_string()))?;
                let time = NaiveTime::from_hms_opt(hour, minute, second)
                    .ok_or_else(|| EngineError::ExecutionError("Invalid time".to_string()))?;
                Ok(Value::Timestamp(NaiveDateTime::new(date, time)))
            }
            "EXTRACT" | "DATE_PART" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "EXTRACT requires 2 arguments (field, datetime)".to_string(),
                    ));
                }
                let field = match &args[0] {
                    Value::String(s) => s.to_uppercase(),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "EXTRACT field must be string".to_string(),
                        ))
                    }
                };
                match &args[1] {
                    Value::Date(d) => {
                        let result = match field.as_str() {
                            "YEAR" => d.year() as i64,
                            "MONTH" => d.month() as i64,
                            "DAY" => d.day() as i64,
                            "DOW" | "DAYOFWEEK" => d.weekday().num_days_from_sunday() as i64,
                            "DOY" | "DAYOFYEAR" => d.ordinal() as i64,
                            "WEEK" => d.iso_week().week() as i64,
                            "QUARTER" => ((d.month() - 1) / 3 + 1) as i64,
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown date field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Integer(result))
                    }
                    Value::Time(t) => {
                        let result = match field.as_str() {
                            "HOUR" => t.hour() as i64,
                            "MINUTE" => t.minute() as i64,
                            "SECOND" => t.second() as i64,
                            "MILLISECOND" => (t.nanosecond() / 1_000_000) as i64,
                            "MICROSECOND" => (t.nanosecond() / 1_000) as i64,
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown time field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Integer(result))
                    }
                    Value::Timestamp(ts) => {
                        let result = match field.as_str() {
                            "YEAR" => ts.date().year() as i64,
                            "MONTH" => ts.date().month() as i64,
                            "DAY" => ts.date().day() as i64,
                            "HOUR" => ts.time().hour() as i64,
                            "MINUTE" => ts.time().minute() as i64,
                            "SECOND" => ts.time().second() as i64,
                            "DOW" | "DAYOFWEEK" => {
                                ts.date().weekday().num_days_from_sunday() as i64
                            }
                            "DOY" | "DAYOFYEAR" => ts.date().ordinal() as i64,
                            "WEEK" => ts.date().iso_week().week() as i64,
                            "QUARTER" => ((ts.date().month() - 1) / 3 + 1) as i64,
                            "EPOCH" => ts.and_utc().timestamp(),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown timestamp field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Integer(result))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "EXTRACT requires date/time/timestamp".to_string(),
                    )),
                }
            }
            "DATE_ADD" | "DATEADD" => {
                if args.len() != 3 {
                    return Err(EngineError::ExecutionError(
                        "DATE_ADD requires 3 arguments (field, amount, datetime)".to_string(),
                    ));
                }
                let field = match &args[0] {
                    Value::String(s) => s.to_uppercase(),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "DATE_ADD field must be string".to_string(),
                        ))
                    }
                };
                let amount = match &args[1] {
                    Value::Integer(n) => *n,
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "DATE_ADD amount must be integer".to_string(),
                        ))
                    }
                };
                match &args[2] {
                    Value::Date(d) => {
                        let new_date = match field.as_str() {
                            "DAY" | "DAYS" => *d + Duration::days(amount),
                            "WEEK" | "WEEKS" => *d + Duration::weeks(amount),
                            "MONTH" | "MONTHS" => {
                                // Adding months is complex, simplified implementation
                                let new_month = d.month() as i64 + amount;
                                let years_to_add = (new_month - 1) / 12;
                                let final_month = ((new_month - 1) % 12 + 1) as u32;
                                let final_year = d.year() + years_to_add as i32;
                                NaiveDate::from_ymd_opt(final_year, final_month, d.day().min(28))
                                    .unwrap_or(*d)
                            }
                            "YEAR" | "YEARS" => NaiveDate::from_ymd_opt(
                                d.year() + amount as i32,
                                d.month(),
                                d.day(),
                            )
                            .unwrap_or(*d),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown date field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Date(new_date))
                    }
                    Value::Timestamp(ts) => {
                        let new_ts = match field.as_str() {
                            "SECOND" | "SECONDS" => *ts + Duration::seconds(amount),
                            "MINUTE" | "MINUTES" => *ts + Duration::minutes(amount),
                            "HOUR" | "HOURS" => *ts + Duration::hours(amount),
                            "DAY" | "DAYS" => *ts + Duration::days(amount),
                            "WEEK" | "WEEKS" => *ts + Duration::weeks(amount),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown timestamp field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Timestamp(new_ts))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "DATE_ADD requires date or timestamp".to_string(),
                    )),
                }
            }
            "DATE_DIFF" | "DATEDIFF" => {
                if args.len() != 3 {
                    return Err(EngineError::ExecutionError(
                        "DATE_DIFF requires 3 arguments (field, date1, date2)".to_string(),
                    ));
                }
                let field = match &args[0] {
                    Value::String(s) => s.to_uppercase(),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "DATE_DIFF field must be string".to_string(),
                        ))
                    }
                };
                match (&args[1], &args[2]) {
                    (Value::Date(d1), Value::Date(d2)) => {
                        let diff = *d2 - *d1;
                        let result = match field.as_str() {
                            "DAY" | "DAYS" => diff.num_days(),
                            "WEEK" | "WEEKS" => diff.num_weeks(),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown date diff field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Integer(result))
                    }
                    (Value::Timestamp(t1), Value::Timestamp(t2)) => {
                        let diff = *t2 - *t1;
                        let result = match field.as_str() {
                            "SECOND" | "SECONDS" => diff.num_seconds(),
                            "MINUTE" | "MINUTES" => diff.num_minutes(),
                            "HOUR" | "HOURS" => diff.num_hours(),
                            "DAY" | "DAYS" => diff.num_days(),
                            "WEEK" | "WEEKS" => diff.num_weeks(),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown timestamp diff field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Integer(result))
                    }
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "DATE_DIFF requires dates or timestamps".to_string(),
                    )),
                }
            }
            "TO_DATE" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(EngineError::ExecutionError(
                        "TO_DATE requires 1-2 arguments (string, [format])".to_string(),
                    ));
                }
                let s = match &args[0] {
                    Value::String(s) => s.clone(),
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "TO_DATE requires string".to_string(),
                        ))
                    }
                };
                let format = if args.len() == 2 {
                    match &args[1] {
                        Value::String(f) => f.clone(),
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "TO_DATE format must be string".to_string(),
                            ))
                        }
                    }
                } else {
                    "%Y-%m-%d".to_string()
                };
                NaiveDate::parse_from_str(&s, &format)
                    .map(Value::Date)
                    .map_err(|e| {
                        EngineError::ExecutionError(format!("Cannot parse date '{}': {}", s, e))
                    })
            }
            "TO_TIMESTAMP" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(EngineError::ExecutionError(
                        "TO_TIMESTAMP requires 1-2 arguments (string, [format])".to_string(),
                    ));
                }
                let s = match &args[0] {
                    Value::String(s) => s.clone(),
                    Value::Integer(epoch) => {
                        // Interpret as Unix epoch
                        return Ok(Value::Timestamp(
                            chrono::DateTime::from_timestamp(*epoch, 0)
                                .map(|dt| dt.naive_utc())
                                .unwrap_or_else(|| NaiveDateTime::default()),
                        ));
                    }
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "TO_TIMESTAMP requires string or integer".to_string(),
                        ))
                    }
                };
                let format = if args.len() == 2 {
                    match &args[1] {
                        Value::String(f) => f.clone(),
                        _ => {
                            return Err(EngineError::ExecutionError(
                                "TO_TIMESTAMP format must be string".to_string(),
                            ))
                        }
                    }
                } else {
                    "%Y-%m-%d %H:%M:%S".to_string()
                };
                NaiveDateTime::parse_from_str(&s, &format)
                    .map(Value::Timestamp)
                    .map_err(|e| {
                        EngineError::ExecutionError(format!(
                            "Cannot parse timestamp '{}': {}",
                            s, e
                        ))
                    })
            }
            "DATE_TRUNC" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "DATE_TRUNC requires 2 arguments (field, datetime)".to_string(),
                    ));
                }
                let field = match &args[0] {
                    Value::String(s) => s.to_uppercase(),
                    _ => {
                        return Err(EngineError::ExecutionError(
                            "DATE_TRUNC field must be string".to_string(),
                        ))
                    }
                };
                match &args[1] {
                    Value::Date(d) => {
                        let truncated = match field.as_str() {
                            "YEAR" => NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap_or(*d),
                            "MONTH" => {
                                NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(*d)
                            }
                            "WEEK" => {
                                let days_from_monday = d.weekday().num_days_from_monday();
                                *d - Duration::days(days_from_monday as i64)
                            }
                            "DAY" => *d,
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown truncation field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Date(truncated))
                    }
                    Value::Timestamp(ts) => {
                        let truncated = match field.as_str() {
                            "YEAR" => NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(ts.date().year(), 1, 1)
                                    .unwrap_or(ts.date()),
                                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                            ),
                            "MONTH" => NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(ts.date().year(), ts.date().month(), 1)
                                    .unwrap_or(ts.date()),
                                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                            ),
                            "DAY" => NaiveDateTime::new(
                                ts.date(),
                                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                            ),
                            "HOUR" => NaiveDateTime::new(
                                ts.date(),
                                NaiveTime::from_hms_opt(ts.time().hour(), 0, 0).unwrap(),
                            ),
                            "MINUTE" => NaiveDateTime::new(
                                ts.date(),
                                NaiveTime::from_hms_opt(ts.time().hour(), ts.time().minute(), 0)
                                    .unwrap(),
                            ),
                            _ => {
                                return Err(EngineError::ExecutionError(format!(
                                    "Unknown truncation field: {}",
                                    field
                                )))
                            }
                        };
                        Ok(Value::Timestamp(truncated))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "DATE_TRUNC requires date or timestamp".to_string(),
                    )),
                }
            }

            // Array functions
            "ARRAY_LENGTH" | "CARDINALITY" => {
                if args.len() != 1 {
                    return Err(EngineError::ExecutionError(
                        "ARRAY_LENGTH requires 1 argument".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Array(arr) => Ok(Value::Integer(arr.len() as i64)),
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ARRAY_LENGTH requires array".to_string(),
                    )),
                }
            }
            "ARRAY_APPEND" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "ARRAY_APPEND requires 2 arguments (array, element)".to_string(),
                    ));
                }
                match &args[0] {
                    Value::Array(arr) => {
                        let mut new_arr = arr.clone();
                        new_arr.push(args[1].clone());
                        Ok(Value::Array(new_arr))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ARRAY_APPEND requires array as first argument".to_string(),
                    )),
                }
            }
            "ARRAY_PREPEND" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "ARRAY_PREPEND requires 2 arguments (element, array)".to_string(),
                    ));
                }
                match &args[1] {
                    Value::Array(arr) => {
                        let mut new_arr = vec![args[0].clone()];
                        new_arr.extend(arr.clone());
                        Ok(Value::Array(new_arr))
                    }
                    Value::Null => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ARRAY_PREPEND requires array as second argument".to_string(),
                    )),
                }
            }
            "ARRAY_CAT" | "ARRAY_CONCAT" => {
                if args.len() != 2 {
                    return Err(EngineError::ExecutionError(
                        "ARRAY_CAT requires 2 arguments".to_string(),
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Array(a1), Value::Array(a2)) => {
                        let mut new_arr = a1.clone();
                        new_arr.extend(a2.clone());
                        Ok(Value::Array(new_arr))
                    }
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => Err(EngineError::ExecutionError(
                        "ARRAY_CAT requires two arrays".to_string(),
                    )),
                }
            }

            _ => Err(EngineError::ExecutionError(format!(
                "Unknown function: {}",
                name
            ))),
        }
    }

    fn evaluate_binary_op(
        &self,
        left: Value,
        op: &BinaryOperator,
        right: Value,
    ) -> EngineResult<Value> {
        match (left.clone(), op, right.clone()) {
            // Integer arithmetic
            (Value::Integer(l), BinaryOperator::Add, Value::Integer(r)) => {
                Ok(Value::Integer(l + r))
            }
            (Value::Integer(l), BinaryOperator::Subtract, Value::Integer(r)) => {
                Ok(Value::Integer(l - r))
            }
            (Value::Integer(l), BinaryOperator::Multiply, Value::Integer(r)) => {
                Ok(Value::Integer(l * r))
            }
            (Value::Integer(l), BinaryOperator::Divide, Value::Integer(r)) => {
                if r == 0 {
                    Err(EngineError::ExecutionError("Division by zero".to_string()))
                } else {
                    Ok(Value::Integer(l / r))
                }
            }

            // Float arithmetic
            (Value::Float(l), BinaryOperator::Add, Value::Float(r)) => Ok(Value::Float(l + r)),
            (Value::Float(l), BinaryOperator::Subtract, Value::Float(r)) => Ok(Value::Float(l - r)),
            (Value::Float(l), BinaryOperator::Multiply, Value::Float(r)) => Ok(Value::Float(l * r)),
            (Value::Float(l), BinaryOperator::Divide, Value::Float(r)) => {
                if r == 0.0 {
                    Err(EngineError::ExecutionError("Division by zero".to_string()))
                } else {
                    Ok(Value::Float(l / r))
                }
            }

            // Mixed Integer/Float arithmetic (promote to Float)
            (Value::Integer(l), BinaryOperator::Add, Value::Float(r)) => {
                Ok(Value::Float(l as f64 + r))
            }
            (Value::Float(l), BinaryOperator::Add, Value::Integer(r)) => {
                Ok(Value::Float(l + r as f64))
            }
            (Value::Integer(l), BinaryOperator::Subtract, Value::Float(r)) => {
                Ok(Value::Float(l as f64 - r))
            }
            (Value::Float(l), BinaryOperator::Subtract, Value::Integer(r)) => {
                Ok(Value::Float(l - r as f64))
            }
            (Value::Integer(l), BinaryOperator::Multiply, Value::Float(r)) => {
                Ok(Value::Float(l as f64 * r))
            }
            (Value::Float(l), BinaryOperator::Multiply, Value::Integer(r)) => {
                Ok(Value::Float(l * r as f64))
            }
            (Value::Integer(l), BinaryOperator::Divide, Value::Float(r)) => {
                if r == 0.0 {
                    Err(EngineError::ExecutionError("Division by zero".to_string()))
                } else {
                    Ok(Value::Float(l as f64 / r))
                }
            }
            (Value::Float(l), BinaryOperator::Divide, Value::Integer(r)) => {
                if r == 0 {
                    Err(EngineError::ExecutionError("Division by zero".to_string()))
                } else {
                    Ok(Value::Float(l / r as f64))
                }
            }

            // String concatenation
            (Value::String(l), BinaryOperator::Add, Value::String(r)) => Ok(Value::String(l + &r)),

            // Integer comparisons
            (Value::Integer(l), BinaryOperator::Equal, Value::Integer(r)) => {
                Ok(Value::Boolean(l == r))
            }
            (Value::Integer(l), BinaryOperator::NotEqual, Value::Integer(r)) => {
                Ok(Value::Boolean(l != r))
            }
            (Value::Integer(l), BinaryOperator::GreaterThan, Value::Integer(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::Integer(l), BinaryOperator::LessThan, Value::Integer(r)) => {
                Ok(Value::Boolean(l < r))
            }
            (Value::Integer(l), BinaryOperator::GreaterThanOrEqual, Value::Integer(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::Integer(l), BinaryOperator::LessThanOrEqual, Value::Integer(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Float comparisons
            (Value::Float(l), BinaryOperator::Equal, Value::Float(r)) => {
                Ok(Value::Boolean((l - r).abs() < f64::EPSILON))
            }
            (Value::Float(l), BinaryOperator::NotEqual, Value::Float(r)) => {
                Ok(Value::Boolean((l - r).abs() >= f64::EPSILON))
            }
            (Value::Float(l), BinaryOperator::GreaterThan, Value::Float(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::Float(l), BinaryOperator::LessThan, Value::Float(r)) => {
                Ok(Value::Boolean(l < r))
            }
            (Value::Float(l), BinaryOperator::GreaterThanOrEqual, Value::Float(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::Float(l), BinaryOperator::LessThanOrEqual, Value::Float(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Mixed Integer/Float comparisons
            (Value::Integer(l), BinaryOperator::Equal, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64 - r).abs() < f64::EPSILON))
            }
            (Value::Float(l), BinaryOperator::Equal, Value::Integer(r)) => {
                Ok(Value::Boolean((l - r as f64).abs() < f64::EPSILON))
            }
            (Value::Integer(l), BinaryOperator::NotEqual, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64 - r).abs() >= f64::EPSILON))
            }
            (Value::Float(l), BinaryOperator::NotEqual, Value::Integer(r)) => {
                Ok(Value::Boolean((l - r as f64).abs() >= f64::EPSILON))
            }
            (Value::Integer(l), BinaryOperator::GreaterThan, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64) > r))
            }
            (Value::Float(l), BinaryOperator::GreaterThan, Value::Integer(r)) => {
                Ok(Value::Boolean(l > r as f64))
            }
            (Value::Integer(l), BinaryOperator::LessThan, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64) < r))
            }
            (Value::Float(l), BinaryOperator::LessThan, Value::Integer(r)) => {
                Ok(Value::Boolean(l < r as f64))
            }
            (Value::Integer(l), BinaryOperator::GreaterThanOrEqual, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64) >= r))
            }
            (Value::Float(l), BinaryOperator::GreaterThanOrEqual, Value::Integer(r)) => {
                Ok(Value::Boolean(l >= r as f64))
            }
            (Value::Integer(l), BinaryOperator::LessThanOrEqual, Value::Float(r)) => {
                Ok(Value::Boolean((l as f64) <= r))
            }
            (Value::Float(l), BinaryOperator::LessThanOrEqual, Value::Integer(r)) => {
                Ok(Value::Boolean(l <= r as f64))
            }

            // String comparisons
            (Value::String(l), BinaryOperator::Equal, Value::String(r)) => {
                Ok(Value::Boolean(l == r))
            }
            (Value::String(l), BinaryOperator::NotEqual, Value::String(r)) => {
                Ok(Value::Boolean(l != r))
            }
            (Value::String(l), BinaryOperator::GreaterThan, Value::String(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::String(l), BinaryOperator::LessThan, Value::String(r)) => {
                Ok(Value::Boolean(l < r))
            }
            (Value::String(l), BinaryOperator::GreaterThanOrEqual, Value::String(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::String(l), BinaryOperator::LessThanOrEqual, Value::String(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Boolean comparisons
            (Value::Boolean(l), BinaryOperator::Equal, Value::Boolean(r)) => {
                Ok(Value::Boolean(l == r))
            }
            (Value::Boolean(l), BinaryOperator::NotEqual, Value::Boolean(r)) => {
                Ok(Value::Boolean(l != r))
            }

            // Logical operators
            (Value::Boolean(l), BinaryOperator::And, Value::Boolean(r)) => {
                Ok(Value::Boolean(l && r))
            }
            (Value::Boolean(l), BinaryOperator::Or, Value::Boolean(r)) => {
                Ok(Value::Boolean(l || r))
            }

            // Date comparisons
            (Value::Date(l), BinaryOperator::Equal, Value::Date(r)) => Ok(Value::Boolean(l == r)),
            (Value::Date(l), BinaryOperator::NotEqual, Value::Date(r)) => {
                Ok(Value::Boolean(l != r))
            }
            (Value::Date(l), BinaryOperator::GreaterThan, Value::Date(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::Date(l), BinaryOperator::LessThan, Value::Date(r)) => Ok(Value::Boolean(l < r)),
            (Value::Date(l), BinaryOperator::GreaterThanOrEqual, Value::Date(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::Date(l), BinaryOperator::LessThanOrEqual, Value::Date(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Time comparisons
            (Value::Time(l), BinaryOperator::Equal, Value::Time(r)) => Ok(Value::Boolean(l == r)),
            (Value::Time(l), BinaryOperator::NotEqual, Value::Time(r)) => {
                Ok(Value::Boolean(l != r))
            }
            (Value::Time(l), BinaryOperator::GreaterThan, Value::Time(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::Time(l), BinaryOperator::LessThan, Value::Time(r)) => Ok(Value::Boolean(l < r)),
            (Value::Time(l), BinaryOperator::GreaterThanOrEqual, Value::Time(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::Time(l), BinaryOperator::LessThanOrEqual, Value::Time(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Timestamp comparisons
            (Value::Timestamp(l), BinaryOperator::Equal, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l == r))
            }
            (Value::Timestamp(l), BinaryOperator::NotEqual, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l != r))
            }
            (Value::Timestamp(l), BinaryOperator::GreaterThan, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l > r))
            }
            (Value::Timestamp(l), BinaryOperator::LessThan, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l < r))
            }
            (Value::Timestamp(l), BinaryOperator::GreaterThanOrEqual, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l >= r))
            }
            (Value::Timestamp(l), BinaryOperator::LessThanOrEqual, Value::Timestamp(r)) => {
                Ok(Value::Boolean(l <= r))
            }

            // Date arithmetic with intervals
            (Value::Date(d), BinaryOperator::Add, Value::Interval(i)) => Ok(Value::Date(d + i)),
            (Value::Date(d), BinaryOperator::Subtract, Value::Interval(i)) => {
                Ok(Value::Date(d - i))
            }
            (Value::Timestamp(t), BinaryOperator::Add, Value::Interval(i)) => {
                Ok(Value::Timestamp(t + i))
            }
            (Value::Timestamp(t), BinaryOperator::Subtract, Value::Interval(i)) => {
                Ok(Value::Timestamp(t - i))
            }

            // Date subtraction yields interval (in days)
            (Value::Date(d1), BinaryOperator::Subtract, Value::Date(d2)) => {
                Ok(Value::Interval(d1 - d2))
            }
            (Value::Timestamp(t1), BinaryOperator::Subtract, Value::Timestamp(t2)) => {
                Ok(Value::Interval(t1 - t2))
            }

            // NULL handling: any comparison with NULL yields NULL (SQL semantics)
            (Value::Null, _, _) | (_, _, Value::Null) => Ok(Value::Null),

            _ => Err(EngineError::ExecutionError(format!(
                "Unsupported binary operation: {:?} {:?} {:?}",
                left, op, right
            ))),
        }
    }

    fn sql_value_to_value(val: SqlValue) -> Value {
        match val {
            SqlValue::Null => Value::Null,
            SqlValue::Boolean(b) => Value::Boolean(b),
            SqlValue::Int16(i) => Value::Integer(i as i64),
            SqlValue::Int32(i) => Value::Integer(i as i64),
            SqlValue::Int64(i) => Value::Integer(i),
            SqlValue::Float32(f) => Value::Float(f as f64),
            SqlValue::Float64(f) => Value::Float(f),
            SqlValue::String(s)
            | SqlValue::Varchar(s)
            | SqlValue::Char(s)
            | SqlValue::Decimal(s) => Value::String(s),
            SqlValue::Binary(_) => Value::Null, // Binary not supported in procedures yet
            SqlValue::Timestamp(_) => Value::Null, // Timestamp conversion not yet supported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{ExecutionPlan, QueryMetrics};
    use async_trait::async_trait;

    struct MockQueryExecutor;

    #[async_trait]
    impl QueryExecutor for MockQueryExecutor {
        async fn execute(&self, _query: Query) -> EngineResult<QueryResult> {
            Ok(QueryResult::Scalar {
                value: SqlValue::Int32(42),
            })
        }
        async fn explain(&self, _query: Query) -> EngineResult<ExecutionPlan> {
            unimplemented!()
        }
        async fn metrics(&self) -> QueryMetrics {
            QueryMetrics::default()
        }
    }

    #[tokio::test]
    async fn test_interpreter_basic() {
        let executor = Arc::new(MockQueryExecutor);
        let interpreter = ProcedureInterpreter::new(executor);

        // Procedure: ADD(a, b) { RETURN a + b; }
        let proc = ProcedureDef {
            name: "add".to_string(),
            args: vec![
                ArgDef {
                    name: "a".to_string(),
                    type_name: "int".to_string(),
                },
                ArgDef {
                    name: "b".to_string(),
                    type_name: "int".to_string(),
                },
            ],
            return_type: "int".to_string(),
            body: Block {
                statements: vec![Statement::Return {
                    value: Some(Expression::BinaryOp {
                        left: Box::new(Expression::Variable("a".to_string())),
                        op: BinaryOperator::Add,
                        right: Box::new(Expression::Variable("b".to_string())),
                    }),
                }],
            },
            language: "plpgsql".to_string(),
        };

        let result = interpreter
            .execute(&proc, vec![Value::Integer(10), Value::Integer(20)])
            .await
            .unwrap();
        assert_eq!(result, Value::Integer(30));
    }

    #[tokio::test]
    async fn test_interpreter_if_else() {
        let executor = Arc::new(MockQueryExecutor);
        let interpreter = ProcedureInterpreter::new(executor);

        // Procedure: MAX(a, b) { IF a > b THEN RETURN a; ELSE RETURN b; END IF; }
        let proc = ProcedureDef {
            name: "max".to_string(),
            args: vec![
                ArgDef {
                    name: "a".to_string(),
                    type_name: "int".to_string(),
                },
                ArgDef {
                    name: "b".to_string(),
                    type_name: "int".to_string(),
                },
            ],
            return_type: "int".to_string(),
            body: Block {
                statements: vec![Statement::If {
                    condition: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("a".to_string())),
                        op: BinaryOperator::GreaterThan,
                        right: Box::new(Expression::Variable("b".to_string())),
                    },
                    then_block: Block {
                        statements: vec![Statement::Return {
                            value: Some(Expression::Variable("a".to_string())),
                        }],
                    },
                    else_block: Some(Block {
                        statements: vec![Statement::Return {
                            value: Some(Expression::Variable("b".to_string())),
                        }],
                    }),
                }],
            },
            language: "plpgsql".to_string(),
        };

        let result = interpreter
            .execute(&proc, vec![Value::Integer(10), Value::Integer(20)])
            .await
            .unwrap();
        assert_eq!(result, Value::Integer(20));

        let result = interpreter
            .execute(&proc, vec![Value::Integer(30), Value::Integer(20)])
            .await
            .unwrap();
        assert_eq!(result, Value::Integer(30));
    }
}
