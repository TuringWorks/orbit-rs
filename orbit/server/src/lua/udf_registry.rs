//! User-Defined Function (UDF) Registry
//!
//! This module provides a centralized registry for user-defined functions
//! written in Lua or JavaScript, callable from SQL queries across all protocols.
//!
//! ## Features
//! - Register Lua/JS functions with metadata
//! - Call functions from SQL with automatic type conversion
//! - Function versioning and overloading
//! - SQL ↔ Lua/JS type conversions
//! - Thread-safe concurrent access

use super::types::{LuaError, LuaResult, LuaValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "lua-mlua")]
use super::mlua_runtime::MluaRuntime;

/// Runtime type for UDFs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UdfRuntime {
    /// Lua function (mlua)
    Lua,
    /// JavaScript function (QuickJS)
    JavaScript,
}

/// Function parameter metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdfParameter {
    /// Parameter name
    pub name: String,
    /// SQL type hint (e.g., "INTEGER", "TEXT", "NUMERIC[]")
    pub sql_type: String,
    /// Whether the parameter is optional
    pub optional: bool,
    /// Default value (SQL expression)
    pub default: Option<String>,
}

/// User-defined function metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdfMetadata {
    /// Unique function name
    pub name: String,
    /// Function source code
    pub source: String,
    /// Runtime (Lua or JavaScript)
    pub runtime: UdfRuntime,
    /// Function parameters
    pub parameters: Vec<UdfParameter>,
    /// Return type (SQL type)
    pub return_type: String,
    /// Whether function is volatile (has side effects)
    pub is_volatile: bool,
    /// Whether function is deterministic
    pub is_deterministic: bool,
    /// Function description
    pub description: Option<String>,
    /// Schema/namespace
    pub schema: String,
    /// Creation timestamp
    pub created_at: i64,
    /// Last modified timestamp
    pub modified_at: i64,
}

impl UdfMetadata {
    /// Create a new UDF metadata
    pub fn new(name: impl Into<String>, source: impl Into<String>, runtime: UdfRuntime) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            name: name.into(),
            source: source.into(),
            runtime,
            parameters: Vec::new(),
            return_type: "TEXT".to_string(),
            is_volatile: true,
            is_deterministic: false,
            description: None,
            schema: "public".to_string(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Add a parameter
    pub fn with_parameter(mut self, name: impl Into<String>, sql_type: impl Into<String>) -> Self {
        self.parameters.push(UdfParameter {
            name: name.into(),
            sql_type: sql_type.into(),
            optional: false,
            default: None,
        });
        self
    }

    /// Set return type
    pub fn with_return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = return_type.into();
        self
    }

    /// Set volatility
    pub fn with_volatility(mut self, is_volatile: bool) -> Self {
        self.is_volatile = is_volatile;
        self
    }

    /// Set determinism
    pub fn with_determinism(mut self, is_deterministic: bool) -> Self {
        self.is_deterministic = is_deterministic;
        self
    }

    /// Set schema
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    /// Get fully qualified name (schema.name)
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

/// UDF Registry for managing user-defined functions
pub struct UdfRegistry {
    /// Lua runtime for executing Lua functions
    #[cfg(feature = "lua-mlua")]
    lua_runtime: Arc<MluaRuntime>,

    /// JavaScript runtime (placeholder for future QuickJS integration)
    #[cfg(feature = "js-quickjs")]
    js_runtime: Arc<dyn std::any::Any + Send + Sync>,

    /// Registered functions (key: qualified name)
    functions: Arc<RwLock<HashMap<String, UdfMetadata>>>,

    /// Function aliases (key: alias, value: qualified name)
    aliases: Arc<RwLock<HashMap<String, String>>>,
}

impl UdfRegistry {
    /// Create a new UDF registry
    #[cfg(feature = "lua-mlua")]
    pub fn new(lua_runtime: Arc<MluaRuntime>) -> Self {
        Self {
            lua_runtime,
            #[cfg(feature = "js-quickjs")]
            js_runtime: Arc::new(()),
            functions: Arc::new(RwLock::new(HashMap::new())),
            aliases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "js-quickjs")]
            js_runtime: Arc::new(()),
            functions: Arc::new(RwLock::new(HashMap::new())),
            aliases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new UDF
    pub async fn register(&self, metadata: UdfMetadata) -> LuaResult<()> {
        let qualified_name = metadata.qualified_name();

        // For Lua functions, register in the Lua runtime
        #[cfg(feature = "lua-mlua")]
        if metadata.runtime == UdfRuntime::Lua {
            // Convert to LuaFunction and register
            let lua_func =
                super::types::LuaFunction::new(metadata.name.clone(), metadata.source.clone())
                    .with_volatility(metadata.is_volatile)
                    .with_determinism(metadata.is_deterministic);

            self.lua_runtime.register_function(lua_func).await?;
        }

        // Store metadata
        let mut functions = self.functions.write().await;
        functions.insert(qualified_name.clone(), metadata.clone());

        // Add alias for simple name (if not in public schema, use qualified)
        let mut aliases = self.aliases.write().await;
        if metadata.schema == "public" {
            aliases.insert(metadata.name.clone(), qualified_name);
        }

        Ok(())
    }

    /// Unregister a UDF
    pub async fn unregister(&self, qualified_name: &str) -> LuaResult<()> {
        let mut functions = self.functions.write().await;
        let metadata = functions.remove(qualified_name).ok_or_else(|| {
            LuaError::FunctionNotFound(format!("Function {} not found", qualified_name))
        })?;

        // Remove aliases
        let mut aliases = self.aliases.write().await;
        aliases.retain(|_, v| v != qualified_name);

        // For Lua functions, unregister from runtime
        #[cfg(feature = "lua-mlua")]
        if metadata.runtime == UdfRuntime::Lua {
            // TODO: self.lua_runtime.unregister_function(&metadata.name).await?;
        }

        Ok(())
    }

    /// Get UDF metadata
    pub async fn get_metadata(&self, name: &str) -> Option<UdfMetadata> {
        let functions = self.functions.read().await;

        // Try direct lookup first
        if let Some(metadata) = functions.get(name) {
            return Some(metadata.clone());
        }

        // Try alias lookup
        let aliases = self.aliases.read().await;
        if let Some(qualified_name) = aliases.get(name) {
            return functions.get(qualified_name).cloned();
        }

        None
    }

    /// Check if a UDF exists
    pub async fn exists(&self, name: &str) -> bool {
        self.get_metadata(name).await.is_some()
    }

    /// List all UDFs
    pub async fn list_all(&self) -> Vec<UdfMetadata> {
        let functions = self.functions.read().await;
        functions.values().cloned().collect()
    }

    /// List UDFs in a schema
    pub async fn list_schema(&self, schema: &str) -> Vec<UdfMetadata> {
        let functions = self.functions.read().await;
        functions
            .values()
            .filter(|f| f.schema == schema)
            .cloned()
            .collect()
    }

    /// Call a UDF with SQL values
    pub async fn call_udf(&self, name: &str, args: Vec<SqlValue>) -> LuaResult<SqlValue> {
        // Get metadata
        let metadata = self
            .get_metadata(name)
            .await
            .ok_or_else(|| LuaError::FunctionNotFound(name.to_string()))?;

        // Validate argument count
        let required_params = metadata.parameters.iter().filter(|p| !p.optional).count();
        if args.len() < required_params || args.len() > metadata.parameters.len() {
            return Err(LuaError::InvalidArgument(format!(
                "Function {} expects {} arguments, got {}",
                name,
                metadata.parameters.len(),
                args.len()
            )));
        }

        // Execute based on runtime
        match metadata.runtime {
            UdfRuntime::Lua => self.call_lua_udf(&metadata, args).await,
            UdfRuntime::JavaScript => self.call_js_udf(&metadata, args).await,
        }
    }

    /// Call a Lua UDF
    #[cfg(feature = "lua-mlua")]
    async fn call_lua_udf(
        &self,
        metadata: &UdfMetadata,
        args: Vec<SqlValue>,
    ) -> LuaResult<SqlValue> {
        // Convert SQL values to Lua values
        let lua_args: Vec<LuaValue> = args.into_iter().map(sql_to_lua).collect();

        // Build a wrapper script that assigns parameters
        let mut script = String::new();
        for (i, param) in metadata.parameters.iter().enumerate() {
            script.push_str(&format!("local {} = ARGV[{}]\n", param.name, i + 1));
        }
        script.push_str(&metadata.source);

        // Execute the wrapped script
        let result = self
            .lua_runtime
            .eval_with_keys_args(&script, &[], &lua_args)
            .await?;

        // Convert result back to SQL
        Ok(lua_to_sql(&result))
    }

    #[cfg(not(feature = "lua-mlua"))]
    async fn call_lua_udf(
        &self,
        metadata: &UdfMetadata,
        args: Vec<SqlValue>,
    ) -> LuaResult<SqlValue> {
        // Convert SQL values to Lua values
        let lua_args: Vec<LuaValue> = args.into_iter().map(sql_to_lua).collect();

        // Build a wrapper script that assigns parameters
        let mut script = String::new();
        for (i, param) in metadata.parameters.iter().enumerate() {
            script.push_str(&format!("local {} = ARGV[{}]\n", param.name, i + 1));
        }
        script.push_str(&metadata.source);

        // Execute the wrapped script
        let result = self
            .lua_runtime
            .eval_with_keys_args(&script, &[], &lua_args)
            .await?;

        // Convert result back to SQL
        Ok(lua_to_sql(&result))
    }

    /// Call a JavaScript UDF (stub for now)
    async fn call_js_udf(
        &self,
        _metadata: &UdfMetadata,
        _args: Vec<SqlValue>,
    ) -> LuaResult<SqlValue> {
        Err(LuaError::InternalError(
            "JavaScript UDFs not yet implemented".to_string(),
        ))
    }
}

// SQL ↔ Lua type conversions

/// Placeholder for SqlValue until we import the real type
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Boolean(bool),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Real(f32),
    Double(f64),
    Numeric(String),
    Text(String),
    Bytea(Vec<u8>),
    Timestamp(i64),
    Date(i32),
    Time(i64),
    Interval(i64),
    Array(Vec<SqlValue>),
    Json(String),
    Jsonb(Vec<u8>),
    Uuid(uuid::Uuid),
}

/// Convert SQL value to Lua value
pub fn sql_to_lua(sql: SqlValue) -> LuaValue {
    match sql {
        SqlValue::Null => LuaValue::Nil,
        SqlValue::Boolean(b) => LuaValue::Boolean(b),
        SqlValue::SmallInt(i) => LuaValue::Integer(i as i64),
        SqlValue::Integer(i) => LuaValue::Integer(i as i64),
        SqlValue::BigInt(i) => LuaValue::Integer(i),
        SqlValue::Real(f) => LuaValue::Number(f as f64),
        SqlValue::Double(f) => LuaValue::Number(f),
        SqlValue::Numeric(s) => {
            // Try to parse as number, fallback to string
            s.parse::<f64>()
                .map(LuaValue::Number)
                .unwrap_or_else(|_| LuaValue::String(s))
        }
        SqlValue::Text(s) => LuaValue::String(s),
        SqlValue::Bytea(b) => LuaValue::Binary(b),
        SqlValue::Timestamp(ts) => LuaValue::Integer(ts),
        SqlValue::Date(d) => LuaValue::Integer(d as i64),
        SqlValue::Time(t) => LuaValue::Integer(t),
        SqlValue::Interval(i) => LuaValue::Integer(i),
        SqlValue::Array(arr) => LuaValue::Array(arr.into_iter().map(sql_to_lua).collect()),
        SqlValue::Json(s) => {
            // Parse JSON and convert to Lua table
            // For now, just return as string
            LuaValue::String(s)
        }
        SqlValue::Jsonb(bytes) => {
            // For JSONB, convert to string first
            let s = String::from_utf8_lossy(&bytes).to_string();
            LuaValue::String(s)
        }
        SqlValue::Uuid(u) => LuaValue::String(u.to_string()),
    }
}

/// Convert Lua value to SQL value
pub fn lua_to_sql(lua: &LuaValue) -> SqlValue {
    match lua {
        LuaValue::Nil => SqlValue::Null,
        LuaValue::Boolean(b) => SqlValue::Boolean(*b),
        LuaValue::Integer(i) => {
            // Try to fit in i32 range first
            if *i >= i32::MIN as i64 && *i <= i32::MAX as i64 {
                SqlValue::Integer(*i as i32)
            } else {
                SqlValue::BigInt(*i)
            }
        }
        LuaValue::Number(f) => SqlValue::Double(*f),
        LuaValue::String(s) => SqlValue::Text(s.clone()),
        LuaValue::Binary(b) => SqlValue::Bytea(b.clone()),
        LuaValue::Array(arr) => SqlValue::Array(arr.iter().map(lua_to_sql).collect()),
        LuaValue::Table(map) => {
            // Convert table to JSON
            let json = serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string());
            SqlValue::Json(json)
        }
        LuaValue::Function(name) => SqlValue::Text(format!("function:{}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udf_metadata_builder() {
        let udf = UdfMetadata::new("my_func", "return arg1 + arg2", UdfRuntime::Lua)
            .with_parameter("arg1", "INTEGER")
            .with_parameter("arg2", "INTEGER")
            .with_return_type("INTEGER")
            .with_determinism(true)
            .with_schema("public");

        assert_eq!(udf.name, "my_func");
        assert_eq!(udf.parameters.len(), 2);
        assert_eq!(udf.return_type, "INTEGER");
        assert!(udf.is_deterministic);
        assert_eq!(udf.qualified_name(), "public.my_func");
    }

    #[test]
    fn test_sql_lua_conversions() {
        // Null
        assert_eq!(sql_to_lua(SqlValue::Null), LuaValue::Nil);

        // Boolean
        assert_eq!(sql_to_lua(SqlValue::Boolean(true)), LuaValue::Boolean(true));

        // Integers
        assert_eq!(sql_to_lua(SqlValue::Integer(42)), LuaValue::Integer(42));

        // Float
        assert_eq!(sql_to_lua(SqlValue::Double(3.14)), LuaValue::Number(3.14));

        // String
        assert_eq!(
            sql_to_lua(SqlValue::Text("hello".to_string())),
            LuaValue::String("hello".to_string())
        );

        // Array
        let sql_arr = SqlValue::Array(vec![SqlValue::Integer(1), SqlValue::Integer(2)]);
        let lua_arr = sql_to_lua(sql_arr);
        assert_eq!(
            lua_arr,
            LuaValue::Array(vec![LuaValue::Integer(1), LuaValue::Integer(2)])
        );
    }

    #[test]
    fn test_lua_sql_conversions() {
        // Nil
        assert_eq!(lua_to_sql(&LuaValue::Nil), SqlValue::Null);

        // Boolean
        assert_eq!(
            lua_to_sql(&LuaValue::Boolean(true)),
            SqlValue::Boolean(true)
        );

        // Integer (small values fit in i32)
        assert_eq!(lua_to_sql(&LuaValue::Integer(42)), SqlValue::Integer(42));
        // BigInt (large values use i64)
        assert_eq!(
            lua_to_sql(&LuaValue::Integer(i64::MAX)),
            SqlValue::BigInt(i64::MAX)
        );

        // Number
        assert_eq!(lua_to_sql(&LuaValue::Number(3.14)), SqlValue::Double(3.14));

        // String
        assert_eq!(
            lua_to_sql(&LuaValue::String("hello".to_string())),
            SqlValue::Text("hello".to_string())
        );
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_udf_registry() {
        let runtime = Arc::new(MluaRuntime::new());
        let registry = UdfRegistry::new(runtime);

        // Register a function
        let udf = UdfMetadata::new("add", "return arg1 + arg2", UdfRuntime::Lua)
            .with_parameter("arg1", "INTEGER")
            .with_parameter("arg2", "INTEGER")
            .with_return_type("INTEGER");

        registry.register(udf.clone()).await.unwrap();

        // Check existence
        assert!(registry.exists("add").await);
        assert!(registry.exists("public.add").await);

        // Get metadata
        let retrieved = registry.get_metadata("add").await.unwrap();
        assert_eq!(retrieved.name, "add");

        // List functions
        let all_funcs = registry.list_all().await;
        assert_eq!(all_funcs.len(), 1);
    }
}
