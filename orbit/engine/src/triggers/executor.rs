//! Trigger Execution Service

use super::manager::{TriggerDef, TriggerEvent, TriggerExecutor, TriggerTiming};
use crate::error::{EngineError, EngineResult};
use crate::procedures::{ProcedureCatalog, ProcedureInterpreter, Value};
use crate::storage::Row;
use std::collections::HashMap;
use std::sync::Arc;

/// Default implementation of TriggerExecutor
pub struct TriggerExecutionService {
    catalog: Arc<ProcedureCatalog>,
    interpreter: Arc<ProcedureInterpreter>,
}

impl TriggerExecutionService {
    pub fn new(catalog: Arc<ProcedureCatalog>, interpreter: Arc<ProcedureInterpreter>) -> Self {
        Self {
            catalog,
            interpreter,
        }
    }

    fn convert_sql_value(val: &crate::storage::SqlValue) -> Value {
        use crate::storage::SqlValue;
        match val {
            SqlValue::Null => Value::Null,
            SqlValue::Boolean(b) => Value::Boolean(*b),
            SqlValue::Int16(i) => Value::Integer(*i as i64),
            SqlValue::Int32(i) => Value::Integer(*i as i64),
            SqlValue::Int64(i) => Value::Integer(*i),
            SqlValue::Float32(f) => Value::Float(*f as f64),
            SqlValue::Float64(f) => Value::Float(*f),
            SqlValue::String(s)
            | SqlValue::Varchar(s)
            | SqlValue::Char(s)
            | SqlValue::Decimal(s) => Value::String(s.clone()),
            SqlValue::Binary(_) => Value::Null, // Binary data not directly supported in procedures
            SqlValue::Timestamp(_) => Value::Null, // Timestamp conversion not yet supported
        }
    }
}

#[async_trait::async_trait]
impl TriggerExecutor for TriggerExecutionService {
    async fn execute_trigger(
        &self,
        trigger: &TriggerDef,
        old_row: Option<&Row>,
        new_row: Option<&Row>,
    ) -> EngineResult<()> {
        // 1. Get procedure definition
        let procedure = self.catalog.get_procedure(&trigger.procedure_name)?;

        // 2. Prepare context (OLD/NEW variables)
        let mut context = HashMap::new();

        // Note: In PL/pgSQL, OLD and NEW are records.
        // Our interpreter currently supports simple variables.
        // We might need to flatten the row into variables like OLD.col1, NEW.col1
        // or extend Value to support Record/Object types.
        // For this MVP, we'll flatten them with prefixes: "OLD_colname", "NEW_colname"

        if let Some(row) = old_row {
            for (col, val) in row {
                context.insert(format!("OLD_{}", col), Self::convert_sql_value(val));
            }
        }

        if let Some(row) = new_row {
            for (col, val) in row {
                context.insert(format!("NEW_{}", col), Self::convert_sql_value(val));
            }
        }

        // Add TG_OP variable
        let op = match trigger.event {
            TriggerEvent::Insert => "INSERT",
            TriggerEvent::Update => "UPDATE",
            TriggerEvent::Delete => "DELETE",
        };
        context.insert("TG_OP".to_string(), Value::String(op.to_string()));

        // Add TG_TABLE_NAME
        context.insert(
            "TG_TABLE_NAME".to_string(),
            Value::String(trigger.table.clone()),
        );

        // 3. Execute procedure
        // Triggers typically don't take arguments in the call, but use context
        self.interpreter
            .execute_with_context(&procedure, vec![], context)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procedures::{ArgDef, BinaryOperator, Block, Expression, Statement};

    use crate::query::{ExecutionPlan, Query, QueryExecutor, QueryMetrics, QueryResult};
    use async_trait::async_trait;

    struct MockQueryExecutor;

    #[async_trait]
    impl QueryExecutor for MockQueryExecutor {
        async fn execute(&self, _query: Query) -> EngineResult<QueryResult> {
            Ok(QueryResult::Scalar {
                value: crate::storage::SqlValue::Int32(42),
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
    async fn test_trigger_execution() {
        let catalog = Arc::new(ProcedureCatalog::new());
        let query_executor = Arc::new(MockQueryExecutor);
        let interpreter = Arc::new(ProcedureInterpreter::new(query_executor));
        let executor = TriggerExecutionService::new(catalog.clone(), interpreter.clone());

        // Register a procedure that checks OLD/NEW values
        // Procedure: CHECK_TRIGGER() {
        //   IF NEW_val > OLD_val THEN RETURN 1; ELSE RETURN 0; END IF;
        // }
        // Note: For this test, we just want to ensure it runs without error and variables are present.
        // Since execute_trigger returns (), we can't easily check the return value here without side effects.
        // But we can check if it fails due to missing variables.

        let proc = crate::procedures::ProcedureDef {
            name: "check_trigger".to_string(),
            args: vec![], // Triggers take no args
            return_type: "int".to_string(),
            body: Block {
                statements: vec![Statement::If {
                    condition: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("NEW_val".to_string())),
                        op: BinaryOperator::GreaterThan,
                        right: Box::new(Expression::Variable("OLD_val".to_string())),
                    },
                    then_block: Block {
                        statements: vec![Statement::Return {
                            value: Some(Expression::Literal(crate::procedures::Literal::Integer(
                                1,
                            ))),
                        }],
                    },
                    else_block: Some(Block {
                        statements: vec![Statement::Return {
                            value: Some(Expression::Literal(crate::procedures::Literal::Integer(
                                0,
                            ))),
                        }],
                    }),
                }],
            },
            language: "plpgsql".to_string(),
        };

        catalog.register_procedure(proc).unwrap();

        let trigger = TriggerDef {
            name: "test_trigger".to_string(),
            table: "test_table".to_string(),
            event: TriggerEvent::Update,
            timing: TriggerTiming::Before,
            procedure_name: "check_trigger".to_string(),
        };

        let mut old_row = HashMap::new();
        old_row.insert("val".to_string(), crate::storage::SqlValue::Int32(10));

        let mut new_row = HashMap::new();
        new_row.insert("val".to_string(), crate::storage::SqlValue::Int32(20));

        let result = executor
            .execute_trigger(&trigger, Some(&old_row), Some(&new_row))
            .await;
        assert!(result.is_ok());
    }
}
