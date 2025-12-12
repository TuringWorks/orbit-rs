//! Scripting command handlers for Redis RESP protocol
//!
//! This module implements Redis scripting commands:
//! - EVAL: Execute a Lua script
//! - EVALSHA: Execute a cached Lua script by SHA1
//! - SCRIPT LOAD: Cache a script
//! - SCRIPT EXISTS: Check if scripts exist in cache
//! - SCRIPT FLUSH: Clear script cache
//! - SCRIPT KILL: Kill currently running script
//! - SCRIPT DEBUG: Enable/disable debugging
//!
//! ## References
//! - Redis Scripting: https://redis.io/docs/manual/programmability/eval-intro/

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(feature = "lua-mlua")]
use crate::lua::{LuaValue, MluaRuntime};

/// Scripting commands handler
pub struct ScriptingCommands {
    _base: BaseCommandHandler,
    #[cfg(feature = "lua-mlua")]
    lua_runtime: Arc<MluaRuntime>,
}

impl ScriptingCommands {
    /// Create a new scripting commands handler
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            _base: BaseCommandHandler::new(orbit_client, local_registry),
            #[cfg(feature = "lua-mlua")]
            lua_runtime: Arc::new(MluaRuntime::new()),
        }
    }

    /// EVAL script numkeys [key ...] [arg ...]
    /// Execute a Lua script
    async fn cmd_eval(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Validate minimum arguments: script + numkeys
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'eval' command".to_string(),
            ));
        }

        // Extract script
        let script = self.get_string_arg(args, 0, "EVAL")?;

        // Extract numkeys
        let numkeys = self.get_int_arg(args, 1, "EVAL")? as usize;

        // Validate we have enough arguments for keys
        if args.len() < 2 + numkeys {
            return Err(ProtocolError::RespError(format!(
                "ERR wrong number of arguments for 'eval' command (expected at least {} keys)",
                numkeys
            )));
        }

        // Extract KEYS array
        let mut keys = Vec::new();
        for i in 0..numkeys {
            keys.push(self.get_string_arg(args, 2 + i, "EVAL")?);
        }

        // Extract ARGV array
        let mut argv = Vec::new();
        for i in (2 + numkeys)..args.len() {
            argv.push(self.resp_value_to_lua(&args[i]));
        }

        // Execute script
        self.execute_lua_script(&script, &keys, &argv).await
    }

    /// EVALSHA sha1 numkeys [key ...] [arg ...]
    /// Execute a cached Lua script by SHA1
    async fn cmd_evalsha(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Validate minimum arguments: sha1 + numkeys
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'evalsha' command".to_string(),
            ));
        }

        // Extract SHA1
        let sha1 = self.get_string_arg(args, 0, "EVALSHA")?;

        // Extract numkeys
        let numkeys = self.get_int_arg(args, 1, "EVALSHA")? as usize;

        // Validate we have enough arguments for keys
        if args.len() < 2 + numkeys {
            return Err(ProtocolError::RespError(format!(
                "ERR wrong number of arguments for 'evalsha' command (expected at least {} keys)",
                numkeys
            )));
        }

        // Extract KEYS array
        let mut keys = Vec::new();
        for i in 0..numkeys {
            keys.push(self.get_string_arg(args, 2 + i, "EVALSHA")?);
        }

        // Extract ARGV array
        let mut argv = Vec::new();
        for i in (2 + numkeys)..args.len() {
            argv.push(self.resp_value_to_lua(&args[i]));
        }

        // Execute cached script
        self.execute_cached_script(&sha1, &keys, &argv).await
    }

    /// SCRIPT LOAD script
    /// Cache a script and return its SHA1
    async fn cmd_script_load(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SCRIPT LOAD", args, 1)?;

        let script = self.get_string_arg(args, 0, "SCRIPT LOAD")?;

        #[cfg(feature = "lua-mlua")]
        {
            let sha = self.lua_runtime.cache_script(&script).await;
            Ok(RespValue::bulk_string(sha))
        }

        #[cfg(not(feature = "lua-mlua"))]
        {
            Err(ProtocolError::RespError(
                "ERR Lua scripting not enabled".to_string(),
            ))
        }
    }

    /// SCRIPT EXISTS sha1 [sha1 ...]
    /// Check if scripts exist in cache
    async fn cmd_script_exists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'script exists' command".to_string(),
            ));
        }

        #[cfg(feature = "lua-mlua")]
        {
            let mut results = Vec::new();
            for arg in args {
                let sha = self.get_string_arg(&[arg.clone()], 0, "SCRIPT EXISTS")?;
                let exists = self.lua_runtime.script_exists(&sha).await;
                results.push(RespValue::Integer(if exists { 1 } else { 0 }));
            }
            Ok(RespValue::Array(results))
        }

        #[cfg(not(feature = "lua-mlua"))]
        {
            Err(ProtocolError::RespError(
                "ERR Lua scripting not enabled".to_string(),
            ))
        }
    }

    /// SCRIPT FLUSH [ASYNC | SYNC]
    /// Clear the script cache
    async fn cmd_script_flush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Optional ASYNC/SYNC argument
        if !args.is_empty() {
            let mode = self.get_string_arg(args, 0, "SCRIPT FLUSH")?.to_uppercase();
            if mode != "ASYNC" && mode != "SYNC" {
                return Err(ProtocolError::RespError(
                    "ERR invalid mode for SCRIPT FLUSH".to_string(),
                ));
            }
        }

        #[cfg(feature = "lua-mlua")]
        {
            self.lua_runtime.flush_scripts().await;
            Ok(RespValue::simple_string("OK"))
        }

        #[cfg(not(feature = "lua-mlua"))]
        {
            Err(ProtocolError::RespError(
                "ERR Lua scripting not enabled".to_string(),
            ))
        }
    }

    /// SCRIPT KILL
    /// Kill the currently running script
    async fn cmd_script_kill(&self) -> ProtocolResult<RespValue> {
        // TODO: Implement script interruption
        // For now, return error indicating no script is running
        Err(ProtocolError::RespError(
            "NOTBUSY No scripts in execution right now.".to_string(),
        ))
    }

    /// SCRIPT DEBUG YES|SYNC|NO
    /// Enable/disable Lua debugging
    async fn cmd_script_debug(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SCRIPT DEBUG", args, 1)?;

        let mode = self.get_string_arg(args, 0, "SCRIPT DEBUG")?.to_uppercase();
        match mode.as_str() {
            "YES" | "SYNC" | "NO" => {
                // TODO: Implement debugging support
                Ok(RespValue::simple_string("OK"))
            }
            _ => Err(ProtocolError::RespError(
                "ERR invalid debug mode (use YES, SYNC, or NO)".to_string(),
            )),
        }
    }

    /// SCRIPT HELP
    /// Show help for SCRIPT command
    async fn cmd_script_help(&self) -> ProtocolResult<RespValue> {
        let help_text = vec![
            "SCRIPT <subcommand> [<arg> [value] [opt] ...]. Subcommands are:",
            "DEBUG (YES|SYNC|NO)",
            "  Set the debug mode for subsequent scripts executed.",
            "EXISTS <sha1> [<sha1> ...]",
            "  Return information about the existence of the scripts in the script cache.",
            "FLUSH [ASYNC|SYNC]",
            "  Flush the Lua scripts cache. Very dangerous on replicas.",
            "  When called without the optional mode argument, the behavior is determined by the",
            "  lazyfree-lazy-user-flush configuration directive. Valid modes are:",
            "  * ASYNC: Asynchronously flush the scripts cache.",
            "  * SYNC: Synchronously flush the scripts cache.",
            "KILL",
            "  Kill the currently executing Lua script.",
            "LOAD <script>",
            "  Load a script into the scripts cache without executing it.",
            "HELP",
            "  Print this help.",
        ];

        Ok(RespValue::Array(
            help_text
                .iter()
                .map(|s| RespValue::bulk_string(s.to_string()))
                .collect(),
        ))
    }

    // Helper methods

    /// Convert RespValue to LuaValue
    fn resp_value_to_lua(&self, value: &RespValue) -> LuaValue {
        LuaValue::from_resp(value)
    }

    /// Convert LuaValue to RespValue
    fn lua_value_to_resp(&self, value: &LuaValue) -> RespValue {
        value.to_resp()
    }

    /// Execute a Lua script
    #[cfg(feature = "lua-mlua")]
    async fn execute_lua_script(
        &self,
        script: &str,
        keys: &[String],
        argv: &[LuaValue],
    ) -> ProtocolResult<RespValue> {
        match self.lua_runtime.eval_with_keys_args(script, keys, argv).await {
            Ok(result) => Ok(self.lua_value_to_resp(&result)),
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    #[cfg(not(feature = "lua-mlua"))]
    async fn execute_lua_script(
        &self,
        _script: &str,
        _keys: &[String],
        _argv: &[LuaValue],
    ) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR Lua scripting not enabled".to_string(),
        ))
    }

    /// Execute a cached Lua script by SHA1
    #[cfg(feature = "lua-mlua")]
    async fn execute_cached_script(
        &self,
        sha1: &str,
        keys: &[String],
        argv: &[LuaValue],
    ) -> ProtocolResult<RespValue> {
        match self.lua_runtime.eval_sha(sha1, keys, argv).await {
            Ok(result) => Ok(self.lua_value_to_resp(&result)),
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    #[cfg(not(feature = "lua-mlua"))]
    async fn execute_cached_script(
        &self,
        _sha1: &str,
        _keys: &[String],
        _argv: &[LuaValue],
    ) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR Lua scripting not enabled".to_string(),
        ))
    }
}

#[async_trait]
impl CommandHandler for ScriptingCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "EVAL" => self.cmd_eval(args).await,
            "EVALSHA" => self.cmd_evalsha(args).await,
            "SCRIPT" => {
                if args.is_empty() {
                    return self.cmd_script_help().await;
                }
                let subcommand = self.get_string_arg(args, 0, "SCRIPT")?.to_uppercase();
                match subcommand.as_str() {
                    "LOAD" => self.cmd_script_load(&args[1..]).await,
                    "EXISTS" => self.cmd_script_exists(&args[1..]).await,
                    "FLUSH" => self.cmd_script_flush(&args[1..]).await,
                    "KILL" => self.cmd_script_kill().await,
                    "DEBUG" => self.cmd_script_debug(&args[1..]).await,
                    "HELP" => self.cmd_script_help().await,
                    _ => Err(ProtocolError::RespError(format!(
                        "ERR Unknown SCRIPT subcommand '{}'",
                        subcommand
                    ))),
                }
            }
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown command '{}'",
                command_name
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["EVAL", "EVALSHA", "SCRIPT"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::resp::simple_local::SimpleLocalRegistry;
    use orbit_client::OrbitClient;

    async fn create_test_handler() -> ScriptingCommands {
        let orbit_config = orbit_client::OrbitClientConfig::default();
        // Use new_offline for tests to avoid network connection requirement
        let orbit_client = Arc::new(OrbitClient::new_offline(orbit_config).await.unwrap());
        let local_registry = Arc::new(SimpleLocalRegistry::new());
        ScriptingCommands::new(orbit_client, local_registry)
    }

    #[tokio::test]
    #[cfg(feature = "lua-mlua")]
    async fn test_eval_simple() {
        let handler = create_test_handler().await;

        // EVAL "return 1 + 2" 0
        let result = handler
            .cmd_eval(&[
                RespValue::bulk_string("return 1 + 2".to_string()),
                RespValue::Integer(0),
            ])
            .await
            .unwrap();

        assert_eq!(result, RespValue::Integer(3));
    }

    #[tokio::test]
    #[cfg(feature = "lua-mlua")]
    async fn test_eval_with_keys_and_argv() {
        let handler = create_test_handler().await;

        // EVAL "return {KEYS[1], ARGV[1]}" 1 key1 arg1
        let result = handler
            .cmd_eval(&[
                RespValue::bulk_string("return {KEYS[1], ARGV[1]}".to_string()),
                RespValue::Integer(1),
                RespValue::bulk_string("key1".to_string()),
                RespValue::bulk_string("arg1".to_string()),
            ])
            .await
            .unwrap();

        match result {
            RespValue::Array(arr) => {
                assert_eq!(arr.len(), 2);
            }
            _ => panic!("Expected array result"),
        }
    }

    #[tokio::test]
    #[cfg(feature = "lua-mlua")]
    async fn test_script_load_and_evalsha() {
        let handler = create_test_handler().await;

        // SCRIPT LOAD "return 42"
        let sha_result = handler
            .cmd_script_load(&[RespValue::bulk_string("return 42".to_string())])
            .await
            .unwrap();

        let sha = sha_result.as_string().unwrap();
        assert_eq!(sha.len(), 40); // SHA1 is 40 hex characters

        // EVALSHA <sha> 0
        let result = handler
            .cmd_evalsha(&[RespValue::bulk_string(sha), RespValue::Integer(0)])
            .await
            .unwrap();

        assert_eq!(result, RespValue::Integer(42));
    }

    #[tokio::test]
    #[cfg(feature = "lua-mlua")]
    async fn test_script_exists() {
        let handler = create_test_handler().await;

        // Load a script
        let sha_result = handler
            .cmd_script_load(&[RespValue::bulk_string("return 1".to_string())])
            .await
            .unwrap();
        let sha = sha_result.as_string().unwrap();

        // Check if it exists
        let result = handler
            .cmd_script_exists(&[RespValue::bulk_string(sha.clone())])
            .await
            .unwrap();

        match result {
            RespValue::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr[0], RespValue::Integer(1));
            }
            _ => panic!("Expected array result"),
        }

        // Check non-existent script
        let result = handler
            .cmd_script_exists(&[RespValue::bulk_string(
                "0000000000000000000000000000000000000000".to_string(),
            )])
            .await
            .unwrap();

        match result {
            RespValue::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr[0], RespValue::Integer(0));
            }
            _ => panic!("Expected array result"),
        }
    }

    #[tokio::test]
    #[cfg(feature = "lua-mlua")]
    async fn test_script_flush() {
        let handler = create_test_handler().await;

        // Load a script
        handler
            .cmd_script_load(&[RespValue::bulk_string("return 1".to_string())])
            .await
            .unwrap();

        // Flush
        let result = handler.cmd_script_flush(&[]).await.unwrap();
        assert_eq!(result, RespValue::simple_string("OK"));
    }
}
