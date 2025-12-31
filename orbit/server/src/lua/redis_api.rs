//! Redis API for Lua scripts
//!
//! Provides Redis-compatible API functions for Lua scripts:
//! - redis.call(command, ...) - Execute Redis command, throw error on failure
//! - redis.pcall(command, ...) - Protected call, returns error instead of throwing
//! - redis.register_function(name, callback, options) - Register a function
//! - redis.log(level, message) - Log messages
//! - redis.status_reply(message) - Return status reply
//! - redis.error_reply(message) - Return error reply

use super::types::LuaResult;

#[cfg(feature = "lua-mlua")]
use mlua::{Lua, Table, Value as MluaValue};

/// Redis API provider for Lua scripts
/// This struct will be embedded in the Lua global environment as the 'redis' table
pub struct RedisApi {
    // This will be populated with actual command handler when integrated
    _phantom: std::marker::PhantomData<()>,
}

impl RedisApi {
    /// Create a new Redis API instance
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Register the Redis API in a Lua context
    /// Creates a global 'redis' table with all Redis functions
    #[cfg(feature = "lua-mlua")]
    pub fn register_in_lua(&self, lua: &Lua) -> LuaResult<()> {
        let redis_table = lua.create_table()?;

        // redis.call(command, ...)
        let call_fn =
            lua.create_async_function(|lua, args: mlua::Variadic<MluaValue>| async move {
                Self::redis_call(lua, args, false).await
            })?;
        redis_table.set("call", call_fn)?;

        // redis.pcall(command, ...) - protected call
        let pcall_fn =
            lua.create_async_function(|lua, args: mlua::Variadic<MluaValue>| async move {
                Self::redis_call(lua, args, true).await
            })?;
        redis_table.set("pcall", pcall_fn)?;

        // redis.register_function(name, callback, options)
        let register_fn =
            lua.create_function(|lua, args: (String, mlua::Function, Option<Table>)| {
                Self::redis_register_function(lua, args)
            })?;
        redis_table.set("register_function", register_fn)?;

        // redis.log(level, message)
        let log_fn = lua.create_function(|_lua, (level, message): (i32, String)| {
            Self::redis_log(level, message)
        })?;
        redis_table.set("log", log_fn)?;

        // redis.status_reply(message)
        let status_reply_fn =
            lua.create_function(|lua, message: String| Self::redis_status_reply(lua, message))?;
        redis_table.set("status_reply", status_reply_fn)?;

        // redis.error_reply(message)
        let error_reply_fn =
            lua.create_function(|lua, message: String| Self::redis_error_reply(lua, message))?;
        redis_table.set("error_reply", error_reply_fn)?;

        // Log level constants
        redis_table.set("LOG_DEBUG", 0)?;
        redis_table.set("LOG_VERBOSE", 1)?;
        redis_table.set("LOG_NOTICE", 2)?;
        redis_table.set("LOG_WARNING", 3)?;

        // Set the global redis table
        lua.globals().set("redis", redis_table)?;

        Ok(())
    }

    #[cfg(not(feature = "lua-mlua"))]
    pub fn register_in_lua(&self, _lua: &Lua) -> LuaResult<()> {
        Err(LuaError::InternalError(
            "Lua support not enabled".to_string(),
        ))
    }

    // Implementation of redis functions

    /// Execute a Redis command
    /// If protected=true, returns errors as values instead of throwing
    #[cfg(feature = "lua-mlua")]
    async fn redis_call<'lua>(
        _lua: &'lua Lua,
        args: mlua::Variadic<MluaValue<'lua>>,
        protected: bool,
    ) -> mlua::Result<MluaValue<'lua>> {
        if args.is_empty() {
            return Err(mlua::Error::RuntimeError(
                "redis.call requires at least one argument".to_string(),
            ));
        }

        // Extract command name
        let command = match &args[0] {
            MluaValue::String(s) => s.to_str()?.to_string(),
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "First argument to redis.call must be a string".to_string(),
                ))
            }
        };

        // Extract command arguments
        let cmd_args: Vec<String> = args[1..]
            .iter()
            .map(|v| Self::mlua_value_to_string(v))
            .collect::<mlua::Result<Vec<_>>>()?;

        // TODO: Execute actual Redis command through command handler
        // For now, return a stub response
        // In production, this would:
        // 1. Get command handler from Lua app data
        // 2. Execute command via handler
        // 3. Convert result to Lua value
        //
        // Example integration:
        // let handler = lua.app_data_ref::<Arc<CommandHandler>>().unwrap();
        // let result = handler.handle_command(&command, &cmd_args).await?;

        // Stub implementation - return nil for now
        tracing::warn!(
            "redis.call('{}', {:?}) - stub implementation, returning nil",
            command,
            cmd_args
        );

        if protected {
            // For pcall, return errors as table { err = "message" }
            Ok(MluaValue::Nil)
        } else {
            // For call, throw errors
            Ok(MluaValue::Nil)
        }
    }

    /// Register a Lua function
    #[cfg(feature = "lua-mlua")]
    fn redis_register_function(
        lua: &Lua,
        (name, callback, options): (String, mlua::Function, Option<Table>),
    ) -> mlua::Result<()> {
        // Store function in registry for later execution
        // This would be called from FUNCTION LOAD command processing

        // Parse options if provided
        let mut flags = Vec::new();
        let mut _description = None;

        if let Some(opts) = options {
            if let Ok(flags_val) = opts.get::<_, Table>("flags") {
                for (_, flag) in flags_val.pairs::<i32, String>().flatten() {
                    flags.push(flag);
                }
            }

            if let Ok(desc) = opts.get::<_, String>("description") {
                _description = Some(desc);
            }
        }

        // TODO: Store function metadata in function registry
        // For now, just store in Lua registry
        let registry = lua.named_registry_value::<Table>("_orbit_functions")?;
        registry.set(name.clone(), callback)?;

        tracing::debug!("Registered Lua function '{}' with flags {:?}", name, flags);

        Ok(())
    }

    /// Log a message from Lua
    #[cfg(feature = "lua-mlua")]
    fn redis_log(level: i32, message: String) -> mlua::Result<()> {
        match level {
            0 => tracing::debug!(target: "lua", "{}", message),
            1 => tracing::trace!(target: "lua", "{}", message),
            2 => tracing::info!(target: "lua", "{}", message),
            3 => tracing::warn!(target: "lua", "{}", message),
            _ => tracing::info!(target: "lua", "{}", message),
        }
        Ok(())
    }

    /// Create a status reply
    #[cfg(feature = "lua-mlua")]
    fn redis_status_reply(lua: &Lua, message: String) -> mlua::Result<Table<'_>> {
        let table = lua.create_table()?;
        table.set("ok", message)?;
        Ok(table)
    }

    /// Create an error reply
    #[cfg(feature = "lua-mlua")]
    fn redis_error_reply(lua: &Lua, message: String) -> mlua::Result<Table<'_>> {
        let table = lua.create_table()?;
        table.set("err", message)?;
        Ok(table)
    }

    /// Convert mlua Value to String for command arguments
    #[cfg(feature = "lua-mlua")]
    fn mlua_value_to_string(value: &MluaValue<'_>) -> mlua::Result<String> {
        match value {
            MluaValue::Nil => Ok("".to_string()),
            MluaValue::Boolean(b) => Ok(b.to_string()),
            MluaValue::Integer(i) => Ok(i.to_string()),
            MluaValue::Number(n) => Ok(n.to_string()),
            MluaValue::String(s) => Ok(s.to_str()?.to_string()),
            MluaValue::Table(_) => Err(mlua::Error::RuntimeError(
                "Cannot convert table to string for Redis command".to_string(),
            )),
            _ => Err(mlua::Error::RuntimeError(
                "Unsupported value type for Redis command".to_string(),
            )),
        }
    }
}

impl Default for RedisApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a Redis API instance and register it in Lua
#[cfg(feature = "lua-mlua")]
pub fn setup_redis_api(lua: &Lua) -> LuaResult<()> {
    // Create function registry table if it doesn't exist
    let registry_table = lua.create_table()?;
    lua.set_named_registry_value("_orbit_functions", registry_table)?;

    // Register the redis API
    let api = RedisApi::new();
    api.register_in_lua(lua)?;

    Ok(())
}

#[cfg(not(feature = "lua-mlua"))]
pub fn setup_redis_api(_lua: &Lua) -> LuaResult<()> {
    Err(LuaError::InternalError(
        "Lua support not enabled".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_redis_api_registration() {
        let lua = Lua::new();
        setup_redis_api(&lua).unwrap();

        // Verify redis table exists
        let globals = lua.globals();
        let redis_table: Table = globals.get("redis").unwrap();

        // Verify functions exist
        assert!(redis_table.contains_key("call").unwrap());
        assert!(redis_table.contains_key("pcall").unwrap());
        assert!(redis_table.contains_key("register_function").unwrap());
        assert!(redis_table.contains_key("log").unwrap());
        assert!(redis_table.contains_key("status_reply").unwrap());
        assert!(redis_table.contains_key("error_reply").unwrap());

        // Verify constants
        assert_eq!(redis_table.get::<_, i32>("LOG_DEBUG").unwrap(), 0);
        assert_eq!(redis_table.get::<_, i32>("LOG_WARNING").unwrap(), 3);
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_redis_log() {
        let lua = Lua::new();
        setup_redis_api(&lua).unwrap();

        // Test logging
        let result: () = lua
            .load("redis.log(redis.LOG_NOTICE, 'Test message')")
            .eval()
            .unwrap();

        assert_eq!(result, ());
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_redis_status_reply() {
        let lua = Lua::new();
        setup_redis_api(&lua).unwrap();

        let result: Table = lua.load("return redis.status_reply('OK')").eval().unwrap();

        assert_eq!(result.get::<_, String>("ok").unwrap(), "OK");
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_redis_error_reply() {
        let lua = Lua::new();
        setup_redis_api(&lua).unwrap();

        let result: Table = lua
            .load("return redis.error_reply('Error message')")
            .eval()
            .unwrap();

        assert_eq!(result.get::<_, String>("err").unwrap(), "Error message");
    }
}
