//! Function command handlers for Redis RESP protocol
//!
//! This module implements Redis Functions for server-side scripting.
//!
//! ## References
//! - Redis Functions Spec: `specifications/protocols/redis-resp-protocol-specification.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/redis>

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolError;
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// A Redis function definition
#[derive(Debug, Clone)]
pub struct RedisFunction {
    /// Function name
    pub name: String,
    /// Library name this function belongs to
    pub library_name: String,
    /// Function description
    pub description: String,
    /// Function flags (e.g., "no-writes", "allow-stale")
    pub flags: Vec<String>,
}

/// A Redis function library
#[derive(Debug, Clone)]
pub struct FunctionLibrary {
    /// Library name
    pub name: String,
    /// Library code (Lua script)
    pub code: String,
    /// Engine name (e.g., "LUA")
    pub engine: String,
    /// Functions defined in this library
    pub functions: HashMap<String, RedisFunction>,
    /// Library description
    pub description: String,
}

impl FunctionLibrary {
    /// Parse a library from Lua code
    pub fn parse(name: &str, code: &str, engine: &str) -> Result<Self, String> {
        // Simple parsing - in a real implementation, we'd parse the Lua code
        // to extract function definitions
        let mut library = FunctionLibrary {
            name: name.to_string(),
            code: code.to_string(),
            engine: engine.to_uppercase(),
            functions: HashMap::new(),
            description: String::new(),
        };

        // Extract function definitions from the code
        // This is a simplified parser - real implementation would use a Lua parser
        for line in code.lines() {
            let line = line.trim();
            if line.starts_with("redis.register_function") {
                // Parse redis.register_function('name', function...)
                if let Some(start) = line.find('\'') {
                    if let Some(end) = line[start + 1..].find('\'') {
                        let func_name = &line[start + 1..start + 1 + end];
                        library.functions.insert(
                            func_name.to_string(),
                            RedisFunction {
                                name: func_name.to_string(),
                                library_name: name.to_string(),
                                description: String::new(),
                                flags: Vec::new(),
                            },
                        );
                    }
                }
            }
        }

        Ok(library)
    }
}

/// Function manager for storing and executing functions
pub struct FunctionManager {
    /// All function libraries
    libraries: RwLock<HashMap<String, FunctionLibrary>>,
    /// Function statistics
    running_script: RwLock<Option<String>>,
    /// Statistics
    calls: RwLock<u64>,
}

impl FunctionManager {
    pub fn new() -> Self {
        Self {
            libraries: RwLock::new(HashMap::new()),
            running_script: RwLock::new(None),
            calls: RwLock::new(0),
        }
    }

    /// Load a function library
    pub async fn load_library(&self, name: &str, code: &str, engine: &str, replace: bool) -> Result<(), String> {
        let library = FunctionLibrary::parse(name, code, engine)?;

        let mut libraries = self.libraries.write().await;
        if libraries.contains_key(name) && !replace {
            return Err(format!("Library {} already exists", name));
        }

        libraries.insert(name.to_string(), library);
        Ok(())
    }

    /// Delete a library
    pub async fn delete_library(&self, name: &str) -> bool {
        self.libraries.write().await.remove(name).is_some()
    }

    /// List all libraries
    pub async fn list_libraries(&self, library_filter: Option<&str>, with_code: bool) -> Vec<FunctionLibrary> {
        let libraries = self.libraries.read().await;

        libraries
            .values()
            .filter(|lib| {
                if let Some(filter) = library_filter {
                    lib.name.contains(filter)
                } else {
                    true
                }
            })
            .map(|lib| {
                if with_code {
                    lib.clone()
                } else {
                    FunctionLibrary {
                        name: lib.name.clone(),
                        code: String::new(),
                        engine: lib.engine.clone(),
                        functions: lib.functions.clone(),
                        description: lib.description.clone(),
                    }
                }
            })
            .collect()
    }

    /// Get a specific function
    pub async fn get_function(&self, name: &str) -> Option<(FunctionLibrary, RedisFunction)> {
        let libraries = self.libraries.read().await;
        for lib in libraries.values() {
            if let Some(func) = lib.functions.get(name) {
                return Some((lib.clone(), func.clone()));
            }
        }
        None
    }

    /// Flush all libraries
    pub async fn flush(&self, mode: &str) -> usize {
        let mut libraries = self.libraries.write().await;
        let count = libraries.len();
        match mode.to_uppercase().as_str() {
            "SYNC" | "ASYNC" | "" => {
                libraries.clear();
            }
            _ => {}
        }
        count
    }

    /// Get statistics
    pub async fn stats(&self) -> HashMap<String, String> {
        let libraries = self.libraries.read().await;
        let calls = *self.calls.read().await;
        let running = self.running_script.read().await;

        let mut stats = HashMap::new();
        stats.insert("running_script".to_string(), running.clone().unwrap_or_default());
        stats.insert("engines".to_string(), "LUA".to_string());
        stats.insert("libraries".to_string(), libraries.len().to_string());
        stats.insert(
            "functions".to_string(),
            libraries.values().map(|l| l.functions.len()).sum::<usize>().to_string(),
        );
        stats.insert("calls".to_string(), calls.to_string());

        stats
    }

    /// Call a function (simulated)
    pub async fn call(&self, func_name: &str, _keys: &[String], _args: &[String]) -> Result<RespValue, String> {
        // Check if function exists
        let _func = self.get_function(func_name).await.ok_or_else(|| {
            format!("Function not found: {}", func_name)
        })?;

        // Update statistics
        *self.calls.write().await += 1;

        // In a real implementation, we would execute the Lua script here
        // For now, return a simulated response
        Ok(RespValue::SimpleString(format!("(function {} executed)", func_name)))
    }
}

impl Default for FunctionManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FunctionCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
    function_manager: Arc<FunctionManager>,
}

impl FunctionCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            function_manager: Arc::new(FunctionManager::new()),
        }
    }

    fn get_string_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<String> {
        args.get(index).and_then(|v| v.as_string()).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "ERR invalid argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }

    fn get_int_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<i64> {
        args.get(index)
            .and_then(|v| v.as_integer().or_else(|| v.as_string().and_then(|s| s.parse().ok())))
            .ok_or_else(|| {
                ProtocolError::RespError(format!(
                    "ERR invalid integer argument for '{}' command",
                    command_name.to_lowercase()
                ))
            })
    }

    /// FCALL function numkeys key [key ...] arg [arg ...]
    async fn cmd_fcall(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'fcall' command".to_string(),
            ));
        }

        let func_name = self.get_string_arg(args, 0, "FCALL")?;
        let numkeys = self.get_int_arg(args, 1, "FCALL")? as usize;

        let keys: Vec<String> = (0..numkeys)
            .filter_map(|i| self.get_string_arg(args, 2 + i, "FCALL").ok())
            .collect();

        let args_start = 2 + numkeys;
        let func_args: Vec<String> = (args_start..args.len())
            .filter_map(|i| self.get_string_arg(args, i, "FCALL").ok())
            .collect();

        self.function_manager
            .call(&func_name, &keys, &func_args)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))
    }

    /// FCALL_RO - Read-only version of FCALL
    async fn cmd_fcall_ro(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Same as FCALL but with read-only semantics
        self.cmd_fcall(args).await
    }

    /// FUNCTION LOAD [REPLACE] function-code
    async fn cmd_function_load(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'function load' command".to_string(),
            ));
        }

        let mut idx = 0;
        let mut replace = false;

        // Check for REPLACE option
        if let Ok(arg) = self.get_string_arg(args, idx, "FUNCTION LOAD") {
            if arg.to_uppercase() == "REPLACE" {
                replace = true;
                idx += 1;
            }
        }

        let code = self.get_string_arg(args, idx, "FUNCTION LOAD")?;

        // Extract library name from code (simplified)
        let library_name = if code.contains("#!lua name=") {
            code.lines()
                .find(|l| l.contains("#!lua name="))
                .and_then(|l| l.split("name=").nth(1))
                .map(|n| n.trim().to_string())
                .unwrap_or_else(|| format!("lib_{}", uuid::Uuid::new_v4().simple()))
        } else {
            format!("lib_{}", uuid::Uuid::new_v4().simple())
        };

        self.function_manager
            .load_library(&library_name, &code, "LUA", replace)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        debug!("FUNCTION LOAD {} -> OK", library_name);
        Ok(RespValue::BulkString(Bytes::from(library_name.into_bytes())))
    }

    /// FUNCTION LIST [LIBRARYNAME library-name-pattern] [WITHCODE]
    async fn cmd_function_list(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let mut library_filter: Option<String> = None;
        let mut with_code = false;
        let mut idx = 0;

        while idx < args.len() {
            let arg = self.get_string_arg(args, idx, "FUNCTION LIST")?.to_uppercase();
            match arg.as_str() {
                "LIBRARYNAME" => {
                    idx += 1;
                    library_filter = Some(self.get_string_arg(args, idx, "FUNCTION LIST")?);
                }
                "WITHCODE" => {
                    with_code = true;
                }
                _ => {}
            }
            idx += 1;
        }

        let libraries = self.function_manager.list_libraries(library_filter.as_deref(), with_code).await;

        let result: Vec<RespValue> = libraries
            .iter()
            .map(|lib| {
                let mut info = vec![
                    RespValue::BulkString(Bytes::from("library_name")),
                    RespValue::BulkString(Bytes::from(lib.name.as_bytes().to_vec())),
                    RespValue::BulkString(Bytes::from("engine")),
                    RespValue::BulkString(Bytes::from(lib.engine.as_bytes().to_vec())),
                    RespValue::BulkString(Bytes::from("functions")),
                ];

                let funcs: Vec<RespValue> = lib
                    .functions
                    .values()
                    .map(|f| {
                        RespValue::Array(vec![
                            RespValue::BulkString(Bytes::from("name")),
                            RespValue::BulkString(Bytes::from(f.name.as_bytes().to_vec())),
                            RespValue::BulkString(Bytes::from("description")),
                            RespValue::BulkString(Bytes::from(f.description.as_bytes().to_vec())),
                            RespValue::BulkString(Bytes::from("flags")),
                            RespValue::Array(
                                f.flags
                                    .iter()
                                    .map(|fl| RespValue::BulkString(Bytes::from(fl.as_bytes().to_vec())))
                                    .collect(),
                            ),
                        ])
                    })
                    .collect();

                info.push(RespValue::Array(funcs));

                if with_code {
                    info.push(RespValue::BulkString(Bytes::from("library_code")));
                    info.push(RespValue::BulkString(Bytes::from(lib.code.as_bytes().to_vec())));
                }

                RespValue::Array(info)
            })
            .collect();

        debug!("FUNCTION LIST -> {} libraries", result.len());
        Ok(RespValue::Array(result))
    }

    /// FUNCTION DELETE library-name
    async fn cmd_function_delete(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'function delete' command".to_string(),
            ));
        }

        let library_name = self.get_string_arg(args, 0, "FUNCTION DELETE")?;

        if self.function_manager.delete_library(&library_name).await {
            debug!("FUNCTION DELETE {} -> OK", library_name);
            Ok(RespValue::SimpleString("OK".to_string()))
        } else {
            Err(ProtocolError::RespError(format!(
                "ERR Library '{}' not found",
                library_name
            )))
        }
    }

    /// FUNCTION FLUSH [ASYNC | SYNC]
    async fn cmd_function_flush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let mode = if args.is_empty() {
            "SYNC".to_string()
        } else {
            self.get_string_arg(args, 0, "FUNCTION FLUSH")?
        };

        let count = self.function_manager.flush(&mode).await;
        debug!("FUNCTION FLUSH {} -> {} libraries removed", mode, count);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// FUNCTION STATS
    async fn cmd_function_stats(&self) -> ProtocolResult<RespValue> {
        let stats = self.function_manager.stats().await;

        let mut result = Vec::new();
        result.push(RespValue::BulkString(Bytes::from("running_script")));
        result.push(if stats.get("running_script").map(|s| s.is_empty()).unwrap_or(true) {
            RespValue::NullBulkString
        } else {
            RespValue::BulkString(Bytes::from(stats.get("running_script").unwrap().as_bytes().to_vec()))
        });

        result.push(RespValue::BulkString(Bytes::from("engines")));
        let engines = RespValue::Array(vec![
            RespValue::Array(vec![
                RespValue::BulkString(Bytes::from("name")),
                RespValue::BulkString(Bytes::from("LUA")),
                RespValue::BulkString(Bytes::from("libraries_count")),
                RespValue::Integer(stats.get("libraries").and_then(|s| s.parse().ok()).unwrap_or(0)),
                RespValue::BulkString(Bytes::from("functions_count")),
                RespValue::Integer(stats.get("functions").and_then(|s| s.parse().ok()).unwrap_or(0)),
            ]),
        ]);
        result.push(engines);

        debug!("FUNCTION STATS -> OK");
        Ok(RespValue::Array(result))
    }

    /// FUNCTION KILL - Kill the currently running function
    async fn cmd_function_kill(&self) -> ProtocolResult<RespValue> {
        // In a real implementation, this would interrupt the running script
        debug!("FUNCTION KILL -> NOTBUSY (no script running)");
        Err(ProtocolError::RespError("NOTBUSY No scripts in execution right now.".to_string()))
    }

    /// FUNCTION DUMP - Dump all functions (serialized)
    async fn cmd_function_dump(&self) -> ProtocolResult<RespValue> {
        let libraries = self.function_manager.list_libraries(None, true).await;

        // Serialize to a simple format
        let mut dump = String::new();
        for lib in &libraries {
            dump.push_str(&format!("LIB:{}:{}\n{}\nENDLIB\n", lib.name, lib.engine, lib.code));
        }

        debug!("FUNCTION DUMP -> {} bytes", dump.len());
        Ok(RespValue::BulkString(Bytes::from(dump.into_bytes())))
    }

    /// FUNCTION RESTORE serialized-value [FLUSH | APPEND | REPLACE]
    async fn cmd_function_restore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'function restore' command".to_string(),
            ));
        }

        let data = self.get_string_arg(args, 0, "FUNCTION RESTORE")?;
        let mode = if args.len() > 1 {
            self.get_string_arg(args, 1, "FUNCTION RESTORE")?.to_uppercase()
        } else {
            "APPEND".to_string()
        };

        // Flush if requested
        if mode == "FLUSH" {
            self.function_manager.flush("SYNC").await;
        }

        let replace = mode == "REPLACE" || mode == "FLUSH";

        // Parse and restore libraries from dump format
        let mut current_lib: Option<(String, String)> = None;
        let mut current_code = String::new();

        for line in data.lines() {
            if line.starts_with("LIB:") {
                let parts: Vec<&str> = line.splitn(3, ':').collect();
                if parts.len() >= 3 {
                    current_lib = Some((parts[1].to_string(), parts[2].to_string()));
                    current_code.clear();
                }
            } else if line == "ENDLIB" {
                if let Some((name, engine)) = current_lib.take() {
                    let _ = self.function_manager.load_library(&name, &current_code, &engine, replace).await;
                }
            } else if current_lib.is_some() {
                current_code.push_str(line);
                current_code.push('\n');
            }
        }

        debug!("FUNCTION RESTORE {} -> OK", mode);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// FUNCTION HELP
    async fn cmd_function_help(&self) -> ProtocolResult<RespValue> {
        let help = vec![
            "FUNCTION <subcommand> [<arg> [value] [opt] ...]. Subcommands are:",
            "DELETE <library-name>",
            "    Delete a function library.",
            "DUMP",
            "    Return a serialized payload of all loaded libraries.",
            "FLUSH [ASYNC|SYNC]",
            "    Delete all function libraries.",
            "KILL",
            "    Kill the current function running in Lua.",
            "LIST [LIBRARYNAME library-name-pattern] [WITHCODE]",
            "    Return information about all libraries.",
            "LOAD [REPLACE] <function-code>",
            "    Load a library containing functions.",
            "RESTORE <serialized-value> [FLUSH|APPEND|REPLACE]",
            "    Restore libraries from the serialized payload.",
            "STATS",
            "    Return information about the function currently running.",
            "HELP",
            "    Prints this help.",
        ];

        let resp_help: Vec<RespValue> = help
            .iter()
            .map(|h| RespValue::BulkString(Bytes::from(h.as_bytes().to_vec())))
            .collect();

        Ok(RespValue::Array(resp_help))
    }
}

#[async_trait]
impl CommandHandler for FunctionCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "FCALL" => self.cmd_fcall(args).await,
            "FCALL_RO" => self.cmd_fcall_ro(args).await,
            "FUNCTION" => {
                if args.is_empty() {
                    return self.cmd_function_help().await;
                }

                let subcommand = self.get_string_arg(args, 0, "FUNCTION")?.to_uppercase();
                let sub_args = &args[1..];

                match subcommand.as_str() {
                    "DELETE" => self.cmd_function_delete(sub_args).await,
                    "DUMP" => self.cmd_function_dump().await,
                    "FLUSH" => self.cmd_function_flush(sub_args).await,
                    "HELP" => self.cmd_function_help().await,
                    "KILL" => self.cmd_function_kill().await,
                    "LIST" => self.cmd_function_list(sub_args).await,
                    "LOAD" => self.cmd_function_load(sub_args).await,
                    "RESTORE" => self.cmd_function_restore(sub_args).await,
                    "STATS" => self.cmd_function_stats().await,
                    _ => Err(ProtocolError::RespError(format!(
                        "ERR Unknown FUNCTION subcommand '{}'",
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
        &["FCALL", "FCALL_RO", "FUNCTION"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::traits::BaseCommandHandler;

    #[tokio::test]
    async fn test_function_load_and_list() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = FunctionCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            function_manager: Arc::new(FunctionManager::new()),
        };

        // Load a function
        let code = "#!lua name=mylib\nredis.register_function('myfunc', function(keys, args) return 'Hello' end)";
        let args = vec![
            RespValue::BulkString(Bytes::from("LOAD")),
            RespValue::BulkString(Bytes::from(code)),
        ];
        let result = handler.handle("FUNCTION", &args).await;
        assert!(result.is_ok());

        // List functions
        let args = vec![RespValue::BulkString(Bytes::from("LIST"))];
        let result = handler.handle("FUNCTION", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(libs) => assert_eq!(libs.len(), 1),
            _ => panic!("Expected array"),
        }
    }

    #[tokio::test]
    async fn test_function_stats() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = FunctionCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            function_manager: Arc::new(FunctionManager::new()),
        };

        let args = vec![RespValue::BulkString(Bytes::from("STATS"))];
        let result = handler.handle("FUNCTION", &args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_function_flush() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = FunctionCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
            function_manager: Arc::new(FunctionManager::new()),
        };

        // Load a function
        let code = "#!lua name=testlib\nredis.register_function('testfunc', function() end)";
        let args = vec![
            RespValue::BulkString(Bytes::from("LOAD")),
            RespValue::BulkString(Bytes::from(code)),
        ];
        handler.handle("FUNCTION", &args).await.unwrap();

        // Flush
        let args = vec![RespValue::BulkString(Bytes::from("FLUSH"))];
        let result = handler.handle("FUNCTION", &args).await;
        assert!(result.is_ok());

        // List should be empty
        let args = vec![RespValue::BulkString(Bytes::from("LIST"))];
        let result = handler.handle("FUNCTION", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(libs) => assert_eq!(libs.len(), 0),
            _ => panic!("Expected array"),
        }
    }
}
