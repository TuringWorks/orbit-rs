//! Database API for Lua scripts
//!
//! Provides SQL execution capabilities for Lua scripts:
//! - sql.execute(query, params) - Execute SQL commands (INSERT, UPDATE, DELETE)
//! - sql.query(query, params) - Query data (SELECT)
//! - sql.transaction(fn) - Execute in transaction
//! - db.call(function_name, args) - Call stored procedures/functions

use super::types::{LuaResult, LuaValue};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "lua-mlua")]
use mlua::{Lua, Table, Value as MluaValue};

use orbit_client::OrbitClient;

/// Database API provider for Lua scripts
pub struct DatabaseApi {
    orbit_client: Arc<OrbitClient>,
}

impl DatabaseApi {
    /// Create a new Database API instance
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        Self { orbit_client }
    }

    /// Register the Database API in a Lua context
    /// Creates global 'sql' and 'db' tables with database functions
    #[cfg(feature = "lua-mlua")]
    pub fn register_in_lua(&self, lua: &Lua) -> LuaResult<()> {
        let sql_table = lua.create_table()?;
        let db_table = lua.create_table()?;

        // sql.execute(query, params) - Execute SQL command
        let client = self.orbit_client.clone();
        let execute_fn =
            lua.create_async_function(move |lua, (query, params): (String, Option<Table>)| {
                let client = client.clone();
                async move { Self::sql_execute(lua, client, query, params).await }
            })?;
        sql_table.set("execute", execute_fn)?;

        // sql.query(query, params) - Execute SQL query
        let client = self.orbit_client.clone();
        let query_fn =
            lua.create_async_function(move |lua, (query, params): (String, Option<Table>)| {
                let client = client.clone();
                async move { Self::sql_query(lua, client, query, params).await }
            })?;
        sql_table.set("query", query_fn)?;

        // sql.query_one(query, params) - Execute SQL query expecting single row
        let client = self.orbit_client.clone();
        let query_one_fn =
            lua.create_async_function(move |lua, (query, params): (String, Option<Table>)| {
                let client = client.clone();
                async move { Self::sql_query_one(lua, client, query, params).await }
            })?;
        sql_table.set("query_one", query_one_fn)?;

        // db.call(function_name, args) - Call stored procedure/function
        let client = self.orbit_client.clone();
        let call_fn =
            lua.create_async_function(move |lua, (func_name, args): (String, Option<Table>)| {
                let client = client.clone();
                async move { Self::db_call(lua, client, func_name, args).await }
            })?;
        db_table.set("call", call_fn)?;

        // Set the global tables
        lua.globals().set("sql", sql_table)?;
        lua.globals().set("db", db_table)?;

        Ok(())
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub fn register_in_lua(&self, _lua: &Lua) -> LuaResult<()> {
        Err(LuaError::InternalError(
            "Lua support not enabled".to_string(),
        ))
    }

    // Implementation of database functions

    /// Execute a SQL command (INSERT, UPDATE, DELETE, etc.)
    #[cfg(feature = "lua-mlua")]
    async fn sql_execute<'lua>(
        lua: &'lua Lua,
        _client: Arc<OrbitClient>,
        query: String,
        params: Option<Table<'lua>>,
    ) -> mlua::Result<MluaValue<'lua>> {
        // Extract parameters from Lua table
        let _params_vec = if let Some(tbl) = params {
            Self::extract_params(tbl)?
        } else {
            Vec::new()
        };

        // TODO: Execute SQL via OrbitClient
        // For now, return a stub result
        tracing::info!(
            "sql.execute('{}', {:?}) - stub implementation",
            query,
            _params_vec
        );

        // Return affected rows count
        let result = lua.create_table()?;
        result.set("affected_rows", 1)?;
        Ok(MluaValue::Table(result))
    }

    /// Execute a SQL query (SELECT)
    #[cfg(feature = "lua-mlua")]
    async fn sql_query<'lua>(
        lua: &'lua Lua,
        _client: Arc<OrbitClient>,
        query: String,
        params: Option<Table<'lua>>,
    ) -> mlua::Result<MluaValue<'lua>> {
        // Extract parameters from Lua table
        let _params_vec = if let Some(tbl) = params {
            Self::extract_params(tbl)?
        } else {
            Vec::new()
        };

        // TODO: Execute SQL query via OrbitClient
        // For now, return a stub result
        tracing::info!(
            "sql.query('{}', {:?}) - stub implementation",
            query,
            _params_vec
        );

        // Return empty result set
        let result = lua.create_table()?;
        Ok(MluaValue::Table(result))
    }

    /// Execute a SQL query expecting a single row
    #[cfg(feature = "lua-mlua")]
    async fn sql_query_one<'lua>(
        _lua: &'lua Lua,
        _client: Arc<OrbitClient>,
        query: String,
        params: Option<Table<'lua>>,
    ) -> mlua::Result<MluaValue<'lua>> {
        // Extract parameters from Lua table
        let _params_vec = if let Some(tbl) = params {
            Self::extract_params(tbl)?
        } else {
            Vec::new()
        };

        // TODO: Execute SQL query via OrbitClient
        // For now, return nil
        tracing::info!(
            "sql.query_one('{}', {:?}) - stub implementation",
            query,
            _params_vec
        );

        Ok(MluaValue::Nil)
    }

    /// Call a stored procedure or function
    #[cfg(feature = "lua-mlua")]
    async fn db_call<'lua>(
        _lua: &'lua Lua,
        _client: Arc<OrbitClient>,
        func_name: String,
        args: Option<Table<'lua>>,
    ) -> mlua::Result<MluaValue<'lua>> {
        // Extract arguments from Lua table
        let _args_vec = if let Some(tbl) = args {
            Self::extract_params(tbl)?
        } else {
            Vec::new()
        };

        // TODO: Call function via OrbitClient
        // For now, return nil
        tracing::info!(
            "db.call('{}', {:?}) - stub implementation",
            func_name,
            _args_vec
        );

        Ok(MluaValue::Nil)
    }

    /// Extract parameters from Lua table (array or map)
    #[cfg(feature = "lua-mlua")]
    fn extract_params(table: Table) -> mlua::Result<Vec<LuaValue>> {
        let mut params = Vec::new();

        // Try to iterate as array first
        for pair in table.clone().pairs::<i32, MluaValue>() {
            let (_, value) = pair?;
            params.push(Self::mlua_to_lua_value(&value)?);
        }

        // If no array elements, try as map
        if params.is_empty() {
            let mut map = HashMap::new();
            for pair in table.pairs::<String, MluaValue>() {
                let (key, value) = pair?;
                map.insert(key, Self::mlua_to_lua_value(&value)?);
            }
            if !map.is_empty() {
                params.push(LuaValue::Table(map));
            }
        }

        Ok(params)
    }

    /// Convert mlua::Value to LuaValue
    #[cfg(feature = "lua-mlua")]
    fn mlua_to_lua_value(value: &MluaValue) -> mlua::Result<LuaValue> {
        match value {
            MluaValue::Nil => Ok(LuaValue::Nil),
            MluaValue::Boolean(b) => Ok(LuaValue::Boolean(*b)),
            MluaValue::Integer(i) => Ok(LuaValue::Integer(*i)),
            MluaValue::Number(n) => Ok(LuaValue::Number(*n)),
            MluaValue::String(s) => Ok(LuaValue::String(s.to_str()?.to_string())),
            MluaValue::Table(t) => {
                // Try to determine if it's an array or map
                let mut is_array = true;
                let mut max_index = 0i64;
                let mut count = 0;

                for pair in t.clone().pairs::<MluaValue, MluaValue>() {
                    let (key, _) = pair?;
                    count += 1;
                    if let MluaValue::Integer(i) = key {
                        if i > max_index {
                            max_index = i;
                        }
                    } else {
                        is_array = false;
                        break;
                    }
                }

                if is_array && count == max_index as usize {
                    // It's an array
                    let mut arr = Vec::new();
                    for i in 1..=max_index {
                        let val: MluaValue = t.get(i)?;
                        arr.push(Self::mlua_to_lua_value(&val)?);
                    }
                    Ok(LuaValue::Array(arr))
                } else {
                    // It's a map
                    let mut map = HashMap::new();
                    for pair in t.clone().pairs::<String, MluaValue>() {
                        let (k, v) = pair?;
                        map.insert(k, Self::mlua_to_lua_value(&v)?);
                    }
                    Ok(LuaValue::Table(map))
                }
            }
            _ => Err(mlua::Error::RuntimeError(
                "Unsupported Lua value type for SQL parameter".to_string(),
            )),
        }
    }

    /// Convert LuaValue to mlua::Value
    #[cfg(feature = "lua-mlua")]
    pub fn lua_value_to_mlua<'lua>(
        lua: &'lua Lua,
        value: &LuaValue,
    ) -> mlua::Result<MluaValue<'lua>> {
        match value {
            LuaValue::Nil => Ok(MluaValue::Nil),
            LuaValue::Boolean(b) => Ok(MluaValue::Boolean(*b)),
            LuaValue::Integer(i) => Ok(MluaValue::Integer(*i)),
            LuaValue::Number(n) => Ok(MluaValue::Number(*n)),
            LuaValue::String(s) => Ok(MluaValue::String(lua.create_string(s)?)),
            LuaValue::Binary(b) => Ok(MluaValue::String(lua.create_string(b)?)),
            LuaValue::Array(arr) => {
                let table = lua.create_table()?;
                for (i, val) in arr.iter().enumerate() {
                    table.set(i + 1, Self::lua_value_to_mlua(lua, val)?)?;
                }
                Ok(MluaValue::Table(table))
            }
            LuaValue::Table(map) => {
                let table = lua.create_table()?;
                for (k, v) in map.iter() {
                    table.set(k.clone(), Self::lua_value_to_mlua(lua, v)?)?;
                }
                Ok(MluaValue::Table(table))
            }
            LuaValue::Function(name) => {
                // Functions can't be directly converted, return string representation
                Ok(MluaValue::String(lua.create_string(name)?))
            }
        }
    }
}

/// Helper function to setup database API
#[cfg(feature = "lua-mlua")]
pub fn setup_database_api(lua: &Lua, orbit_client: Arc<OrbitClient>) -> LuaResult<()> {
    let api = DatabaseApi::new(orbit_client);
    api.register_in_lua(lua)?;
    Ok(())
}

#[cfg(not(feature = "lua-mlua"))]
pub fn setup_database_api(_lua: &Lua, _orbit_client: Arc<OrbitClient>) -> LuaResult<()> {
    Err(LuaError::InternalError(
        "Lua support not enabled".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_database_api_registration() {
        let lua = Lua::new();
        let client = OrbitClient::new("localhost:50051").await.unwrap();
        setup_database_api(&lua, Arc::new(client)).unwrap();

        // Verify sql table exists
        let globals = lua.globals();
        let sql_table: Table = globals.get("sql").unwrap();

        // Verify functions exist
        assert!(sql_table.contains_key("execute").unwrap());
        assert!(sql_table.contains_key("query").unwrap());
        assert!(sql_table.contains_key("query_one").unwrap());

        // Verify db table exists
        let db_table: Table = globals.get("db").unwrap();
        assert!(db_table.contains_key("call").unwrap());
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_lua_value_conversion() {
        let lua = Lua::new();

        // Test simple values
        let nil = LuaValue::Nil;
        let mlua_nil = DatabaseApi::lua_value_to_mlua(&lua, &nil).unwrap();
        assert!(matches!(mlua_nil, MluaValue::Nil));

        let bool_val = LuaValue::Boolean(true);
        let mlua_bool = DatabaseApi::lua_value_to_mlua(&lua, &bool_val).unwrap();
        assert!(matches!(mlua_bool, MluaValue::Boolean(true)));

        let int_val = LuaValue::Integer(42);
        let mlua_int = DatabaseApi::lua_value_to_mlua(&lua, &int_val).unwrap();
        assert!(matches!(mlua_int, MluaValue::Integer(42)));

        // Test array
        let arr = LuaValue::Array(vec![LuaValue::Integer(1), LuaValue::Integer(2)]);
        let mlua_arr = DatabaseApi::lua_value_to_mlua(&lua, &arr).unwrap();
        assert!(matches!(mlua_arr, MluaValue::Table(_)));
    }
}
