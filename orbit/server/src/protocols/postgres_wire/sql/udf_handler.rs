//! UDF (User-Defined Function) Handler
//!
//! Handles CREATE FUNCTION, DROP FUNCTION, and ALTER FUNCTION statements
//! for Lua and JavaScript user-defined functions.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::SqlValue;

#[cfg(feature = "lua-mlua")]
use crate::lua::udf_registry::{UdfMetadata, UdfRegistry, UdfRuntime};
#[cfg(feature = "lua-mlua")]
use crate::lua::MluaRuntime;

use orbit_shared::orbitql::ast::{
    CreateDefinition, CreateStatement, DataType, DropStatement, FunctionLanguage,
    FunctionVolatility,
};
use std::sync::Arc;

/// UDF Handler for managing user-defined functions
pub struct UdfHandler {
    #[cfg(feature = "lua-mlua")]
    udf_registry: Arc<UdfRegistry>,
}

impl UdfHandler {
    /// Create a new UDF handler
    #[cfg(feature = "lua-mlua")]
    pub fn new(lua_runtime: Arc<MluaRuntime>) -> Self {
        Self {
            udf_registry: Arc::new(UdfRegistry::new(lua_runtime)),
        }
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub fn new() -> Self {
        Self {}
    }

    /// Get a reference to the UDF registry
    #[cfg(feature = "lua-mlua")]
    pub fn registry(&self) -> Arc<UdfRegistry> {
        self.udf_registry.clone()
    }

    /// Handle CREATE FUNCTION statement
    pub async fn handle_create_function(&self, stmt: &CreateStatement) -> ProtocolResult<SqlValue> {
        #[cfg(feature = "lua-mlua")]
        {
            match &stmt.definition {
                CreateDefinition::Function {
                    or_replace,
                    parameters,
                    return_type,
                    language,
                    volatility,
                    body,
                } => {
                    // Check if function already exists
                    let exists = self.udf_registry.exists(&stmt.name).await;
                    if exists && !or_replace {
                        return Err(ProtocolError::PostgresError(format!(
                            "Function '{}' already exists. Use CREATE OR REPLACE FUNCTION to replace it.",
                            stmt.name
                        )));
                    }

                    // Determine runtime from language
                    let runtime = match language {
                        Some(FunctionLanguage::Lua) => UdfRuntime::Lua,
                        Some(FunctionLanguage::JavaScript) => UdfRuntime::JavaScript,
                        Some(lang) => {
                            return Err(ProtocolError::PostgresError(format!(
                                "Unsupported language: {:?}. Supported languages: Lua, JavaScript",
                                lang
                            )))
                        }
                        None => {
                            // Default to Lua if not specified
                            UdfRuntime::Lua
                        }
                    };

                    // Build UDF metadata
                    let mut metadata = UdfMetadata::new(&stmt.name, body.clone(), runtime)
                        .with_return_type(data_type_to_sql_type(return_type));

                    // Add parameters
                    for param in parameters {
                        metadata = metadata
                            .with_parameter(&param.name, data_type_to_sql_type(&param.data_type));
                    }

                    // Set volatility
                    match volatility {
                        Some(FunctionVolatility::Immutable) => {
                            metadata = metadata.with_volatility(false).with_determinism(true);
                        }
                        Some(FunctionVolatility::Stable) => {
                            metadata = metadata.with_volatility(false).with_determinism(false);
                        }
                        Some(FunctionVolatility::Volatile) | None => {
                            metadata = metadata.with_volatility(true).with_determinism(false);
                        }
                    }

                    // Register the function
                    self.udf_registry.register(metadata).await.map_err(|e| {
                        ProtocolError::PostgresError(format!("Failed to register function: {}", e))
                    })?;

                    Ok(SqlValue::Text(format!("CREATE FUNCTION {}", stmt.name)))
                }
                _ => Err(ProtocolError::PostgresError(
                    "Invalid CREATE FUNCTION definition".to_string(),
                )),
            }
        }

        #[cfg(not(feature = "lua-mlua"))]
        {
            let _ = stmt;
            Err(ProtocolError::PostgresError(
                "Lua support not enabled. Compile with --features lua-mlua".to_string(),
            ))
        }
    }

    /// Handle DROP FUNCTION statement
    pub async fn handle_drop_function(&self, stmt: &DropStatement) -> ProtocolResult<SqlValue> {
        #[cfg(feature = "lua-mlua")]
        {
            // Check if function exists
            let exists = self.udf_registry.exists(&stmt.name).await;

            if !exists {
                if stmt.if_exists {
                    return Ok(SqlValue::Text(format!(
                        "NOTICE: function {} does not exist, skipping",
                        stmt.name
                    )));
                } else {
                    return Err(ProtocolError::PostgresError(format!(
                        "Function '{}' does not exist",
                        stmt.name
                    )));
                }
            }

            // Unregister the function
            let qualified_name = format!("public.{}", stmt.name);
            self.udf_registry
                .unregister(&qualified_name)
                .await
                .map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to drop function: {}", e))
                })?;

            Ok(SqlValue::Text(format!("DROP FUNCTION {}", stmt.name)))
        }

        #[cfg(not(feature = "lua-mlua"))]
        {
            let _ = stmt;
            Err(ProtocolError::PostgresError(
                "Lua support not enabled. Compile with --features lua-mlua".to_string(),
            ))
        }
    }

    /// List all user-defined functions
    #[cfg(feature = "lua-mlua")]
    pub async fn list_functions(&self) -> Vec<String> {
        let functions = self.udf_registry.list_all().await;
        functions.iter().map(|f| f.qualified_name()).collect()
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub async fn list_functions(&self) -> Vec<String> {
        Vec::new()
    }

    /// Handle PostgreSQL CREATE FUNCTION statement
    #[cfg(feature = "lua-mlua")]
    pub async fn handle_pg_create_function(
        &self,
        stmt: &crate::protocols::postgres_wire::sql::ast::CreateFunctionStatement,
    ) -> ProtocolResult<SqlValue> {
        // Check if function already exists
        let func_name = match &stmt.name {
            crate::protocols::postgres_wire::sql::ast::FunctionName::Simple(n) => n.to_uppercase(),
            crate::protocols::postgres_wire::sql::ast::FunctionName::Qualified {
                schema: _,
                name,
            } => name.to_uppercase(), // ignoring schema for now or assuming public
        };

        let exists = self.udf_registry.exists(&func_name).await;

        if exists && !stmt.or_replace {
            return Err(ProtocolError::PostgresError(format!(
                "Function '{}' already exists. Use CREATE OR REPLACE FUNCTION to replace it.",
                func_name
            )));
        }

        // Determine runtime from language
        let runtime = match &stmt.language {
            Some(crate::protocols::postgres_wire::sql::ast::FunctionLanguage::Lua) => {
                UdfRuntime::Lua
            }
            Some(crate::protocols::postgres_wire::sql::ast::FunctionLanguage::PlJavaScript) => {
                UdfRuntime::JavaScript
            }
            Some(crate::protocols::postgres_wire::sql::ast::FunctionLanguage::Other(l))
                if l.eq_ignore_ascii_case("lua") =>
            {
                UdfRuntime::Lua
            }
            Some(crate::protocols::postgres_wire::sql::ast::FunctionLanguage::Other(l))
                if l.eq_ignore_ascii_case("javascript") || l.eq_ignore_ascii_case("js") =>
            {
                UdfRuntime::JavaScript
            }
            Some(lang) => {
                return Err(ProtocolError::PostgresError(format!(
                    "Unsupported language: {:?}. Supported languages: Lua, JavaScript",
                    lang
                )))
            }
            None => UdfRuntime::Lua, // Default to Lua
        };

        // Build UDF metadata
        let mut metadata = UdfMetadata::new(&func_name, stmt.body.clone(), runtime);

        // Set return type if specified
        if let Some(ref return_type) = stmt.return_type {
            metadata = metadata.with_return_type(&pg_sql_type_to_string(return_type));
        }

        // Add parameters
        if let Some(ref args) = stmt.args {
            for param in args {
                let param_name = param.name.as_deref().unwrap_or("arg");
                metadata =
                    metadata.with_parameter(param_name, &pg_sql_type_to_string(&param.data_type));
            }
        }

        // Set volatility
        if let Some(ref volatility) = stmt.volatility {
            match volatility {
                crate::protocols::postgres_wire::sql::ast::FunctionVolatility::Immutable => {
                    metadata = metadata.with_volatility(false).with_determinism(true);
                }
                crate::protocols::postgres_wire::sql::ast::FunctionVolatility::Stable => {
                    metadata = metadata.with_volatility(false).with_determinism(false);
                }
                crate::protocols::postgres_wire::sql::ast::FunctionVolatility::Volatile => {
                    metadata = metadata.with_volatility(true).with_determinism(false);
                }
            }
        }

        // Register the function
        self.udf_registry.register(metadata).await.map_err(|e| {
            ProtocolError::PostgresError(format!("Failed to register function: {}", e))
        })?;

        Ok(SqlValue::Text(format!("CREATE FUNCTION {}", func_name)))
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub async fn handle_pg_create_function(
        &self,
        _stmt: &crate::protocols::postgres_wire::sql::ast::CreateFunctionStatement,
    ) -> ProtocolResult<SqlValue> {
        Err(ProtocolError::PostgresError(
            "Lua support not enabled. Compile with --features lua-mlua".to_string(),
        ))
    }

    /// Handle PostgreSQL DROP FUNCTION statement
    #[cfg(feature = "lua-mlua")]
    /// Handle PostgreSQL DROP FUNCTION statement
    #[cfg(feature = "lua-mlua")]
    pub async fn handle_pg_drop_function(
        &self,
        stmt: &crate::protocols::postgres_wire::sql::ast::DropFunctionStatement,
    ) -> ProtocolResult<SqlValue> {
        let mut dropped_funcs = Vec::new();

        for (table_name, _args) in &stmt.functions {
            let func_name = table_name.to_string().to_uppercase();

            // Check if function exists
            let exists = self.udf_registry.exists(&func_name).await;

            if !exists {
                if stmt.if_exists {
                    continue;
                } else {
                    return Err(ProtocolError::PostgresError(format!(
                        "Function '{}' does not exist",
                        func_name
                    )));
                }
            }

            // Unregister the function
            let qualified_name = format!("public.{}", func_name);
            self.udf_registry
                .unregister(&qualified_name)
                .await
                .map_err(|e| {
                    ProtocolError::PostgresError(format!("Failed to drop function: {}", e))
                })?;

            dropped_funcs.push(func_name);
        }

        Ok(SqlValue::Text(format!(
            "DROP FUNCTION {}",
            dropped_funcs.join(", ")
        )))
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub async fn handle_pg_drop_function(
        &self,
        _stmt: &crate::protocols::postgres_wire::sql::ast::DropFunctionStatement,
    ) -> ProtocolResult<SqlValue> {
        Err(ProtocolError::PostgresError(
            "Lua support not enabled. Compile with --features lua-mlua".to_string(),
        ))
    }
}

/// Convert OrbitQL DataType to SQL type string
fn data_type_to_sql_type(dt: &DataType) -> String {
    match dt {
        DataType::Boolean => "BOOLEAN".to_string(),
        DataType::Integer => "INTEGER".to_string(),
        DataType::Float => "DOUBLE PRECISION".to_string(),
        DataType::String { .. } => "TEXT".to_string(),
        DataType::DateTime => "TIMESTAMP".to_string(),
        DataType::Duration => "INTERVAL".to_string(),
        DataType::Uuid => "UUID".to_string(),
        DataType::Array(inner) => format!("{}[]", data_type_to_sql_type(inner)),
        DataType::Object | DataType::Json => "JSON".to_string(),
        DataType::Any => "TEXT".to_string(),
        _ => "TEXT".to_string(),
    }
}

/// Convert PostgreSQL SqlType to string representation
fn pg_sql_type_to_string(
    sql_type: &crate::protocols::postgres_wire::sql::types::SqlType,
) -> String {
    use crate::protocols::postgres_wire::sql::types::SqlType;

    match sql_type {
        SqlType::Boolean => "BOOLEAN".to_string(),
        SqlType::SmallInt => "SMALLINT".to_string(),
        SqlType::Integer => "INTEGER".to_string(),
        SqlType::BigInt => "BIGINT".to_string(),
        SqlType::Real => "REAL".to_string(),
        SqlType::DoublePrecision => "DOUBLE PRECISION".to_string(),
        SqlType::Numeric { .. } => "NUMERIC".to_string(),
        SqlType::Varchar(_) | SqlType::Char(_) | SqlType::Text => "TEXT".to_string(),
        SqlType::Bytea => "BYTEA".to_string(),
        SqlType::Timestamp { .. } => "TIMESTAMP".to_string(),
        SqlType::Date => "DATE".to_string(),
        SqlType::Time { .. } => "TIME".to_string(),
        SqlType::Interval => "INTERVAL".to_string(),
        SqlType::Uuid => "UUID".to_string(),
        SqlType::Json | SqlType::Jsonb => "JSON".to_string(),
        SqlType::Array { element_type, .. } => format!("{}[]", pg_sql_type_to_string(element_type)), // element_type is Box<SqlType>
        _ => "TEXT".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_conversion() {
        assert_eq!(data_type_to_sql_type(&DataType::Boolean), "BOOLEAN");
        assert_eq!(data_type_to_sql_type(&DataType::Integer), "INTEGER");
        assert_eq!(data_type_to_sql_type(&DataType::Float), "DOUBLE PRECISION");
        assert_eq!(
            data_type_to_sql_type(&DataType::String { max_length: None }),
            "TEXT"
        );
        assert_eq!(
            data_type_to_sql_type(&DataType::Array(Box::new(DataType::Integer))),
            "INTEGER[]"
        );
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_udf_handler_creation() {
        use crate::lua::MluaRuntime;
        let runtime = Arc::new(MluaRuntime::new());
        let handler = UdfHandler::new(runtime);
        let functions = handler.list_functions().await;
        assert_eq!(functions.len(), 0);
    }
}
