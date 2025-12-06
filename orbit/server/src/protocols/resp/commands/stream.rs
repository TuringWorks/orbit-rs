//! Stream command handlers for Redis RESP protocol
//!
//! This module implements Redis Streams commands (XADD, XREAD, XRANGE, etc.)
//! that operate on append-only log data structures.
//!
//! ## References
//! - Redis Streams Spec: `specifications/protocols/redis-resp-protocol-specification.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/redis>

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolError;
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tracing::debug;

pub struct StreamCommands {
    base: BaseCommandHandler,
}

impl StreamCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    // Helper methods for argument parsing
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
            .and_then(|v| {
                v.as_integer()
                    .or_else(|| v.as_string().and_then(|s| s.parse().ok()))
            })
            .ok_or_else(|| {
                ProtocolError::RespError(format!(
                    "ERR invalid integer argument for '{}' command",
                    command_name.to_lowercase()
                ))
            })
    }

    /// Convert StreamEntry to RESP format
    fn entry_to_resp(entry: &serde_json::Value) -> RespValue {
        if let Some(obj) = entry.as_object() {
            let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let fields = obj.get("fields").and_then(|v| v.as_object());

            let mut field_values = Vec::new();
            if let Some(fields_map) = fields {
                for (k, v) in fields_map {
                    field_values.push(RespValue::BulkString(Bytes::from(k.as_bytes().to_vec())));
                    if let Some(val_str) = v.as_str() {
                        field_values.push(RespValue::BulkString(Bytes::from(
                            val_str.as_bytes().to_vec(),
                        )));
                    }
                }
            }

            RespValue::Array(vec![
                RespValue::BulkString(Bytes::from(id.as_bytes().to_vec())),
                RespValue::Array(field_values),
            ])
        } else {
            RespValue::NullArray
        }
    }

    /// XADD key [NOMKSTREAM] [MAXLEN|MINID [=|~] threshold] *|id field value [field value ...]
    async fn cmd_xadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xadd' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XADD")?;

        // Parse options and find where field-value pairs start
        let mut idx = 1;
        let mut entry_id = "*".to_string();

        // Skip optional NOMKSTREAM, MAXLEN/MINID options
        while idx < args.len() {
            let arg = self.get_string_arg(args, idx, "XADD")?.to_uppercase();
            match arg.as_str() {
                "NOMKSTREAM" => {
                    idx += 1;
                }
                "MAXLEN" | "MINID" => {
                    idx += 1;
                    // Check for ~ or =
                    if idx < args.len() {
                        let next = self.get_string_arg(args, idx, "XADD")?;
                        if next == "~" || next == "=" {
                            idx += 1;
                        }
                    }
                    // Skip the threshold value
                    if idx < args.len() {
                        idx += 1;
                    }
                }
                "*" => {
                    entry_id = "*".to_string();
                    idx += 1;
                    break;
                }
                _ => {
                    // This could be an explicit ID or the start of field-value pairs
                    if arg.contains('-') || arg.chars().all(|c| c.is_ascii_digit()) {
                        entry_id = arg;
                        idx += 1;
                    }
                    break;
                }
            }
        }

        // Parse field-value pairs
        let mut fields = Vec::new();
        while idx + 1 < args.len() {
            let field = self.get_string_arg(args, idx, "XADD")?;
            let value = self.get_string_arg(args, idx + 1, "XADD")?;
            fields.push((field, value));
            idx += 2;
        }

        if fields.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xadd' command".to_string(),
            ));
        }

        let id_value = if entry_id == "*" {
            serde_json::Value::Null
        } else {
            serde_json::to_value(&entry_id).unwrap()
        };

        let result = self
            .base
            .local_registry
            .execute_stream(
                &key,
                "xadd",
                &[id_value, serde_json::to_value(&fields).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let entry_id: String = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        debug!("XADD {} -> {}", key, entry_id);
        Ok(RespValue::BulkString(Bytes::from(entry_id.into_bytes())))
    }

    /// XLEN key - Get the length of a stream
    async fn cmd_xlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xlen' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XLEN")?;

        let result = self
            .base
            .local_registry
            .execute_stream(&key, "xlen", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let length: usize = serde_json::from_value(result).unwrap_or(0);

        debug!("XLEN {} -> {}", key, length);
        Ok(RespValue::Integer(length as i64))
    }

    /// XRANGE key start end [COUNT count] - Get entries in a range
    async fn cmd_xrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xrange' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XRANGE")?;
        let start = self.get_string_arg(args, 1, "XRANGE")?;
        let end = self.get_string_arg(args, 2, "XRANGE")?;

        let count: Option<usize> = if args.len() >= 5 {
            let count_keyword = self.get_string_arg(args, 3, "XRANGE")?;
            if count_keyword.to_uppercase() == "COUNT" {
                Some(self.get_int_arg(args, 4, "XRANGE")? as usize)
            } else {
                None
            }
        } else {
            None
        };

        let mut rpc_args = vec![
            serde_json::to_value(&start).unwrap(),
            serde_json::to_value(&end).unwrap(),
        ];
        if let Some(c) = count {
            rpc_args.push(serde_json::to_value(c).unwrap());
        }

        let result = self
            .base
            .local_registry
            .execute_stream(&key, "xrange", &rpc_args)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let entries: Vec<serde_json::Value> = serde_json::from_value(result).unwrap_or_default();
        let resp_entries: Vec<RespValue> = entries.iter().map(Self::entry_to_resp).collect();

        debug!(
            "XRANGE {} {} {} -> {} entries",
            key,
            start,
            end,
            resp_entries.len()
        );
        Ok(RespValue::Array(resp_entries))
    }

    /// XREVRANGE key end start [COUNT count] - Get entries in reverse order
    async fn cmd_xrevrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xrevrange' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XREVRANGE")?;
        let end = self.get_string_arg(args, 1, "XREVRANGE")?;
        let start = self.get_string_arg(args, 2, "XREVRANGE")?;

        let count: Option<usize> = if args.len() >= 5 {
            let count_keyword = self.get_string_arg(args, 3, "XREVRANGE")?;
            if count_keyword.to_uppercase() == "COUNT" {
                Some(self.get_int_arg(args, 4, "XREVRANGE")? as usize)
            } else {
                None
            }
        } else {
            None
        };

        let mut rpc_args = vec![
            serde_json::to_value(&end).unwrap(),
            serde_json::to_value(&start).unwrap(),
        ];
        if let Some(c) = count {
            rpc_args.push(serde_json::to_value(c).unwrap());
        }

        let result = self
            .base
            .local_registry
            .execute_stream(&key, "xrevrange", &rpc_args)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let entries: Vec<serde_json::Value> = serde_json::from_value(result).unwrap_or_default();
        let resp_entries: Vec<RespValue> = entries.iter().map(Self::entry_to_resp).collect();

        debug!(
            "XREVRANGE {} {} {} -> {} entries",
            key,
            end,
            start,
            resp_entries.len()
        );
        Ok(RespValue::Array(resp_entries))
    }

    /// XREAD [COUNT count] [BLOCK milliseconds] STREAMS key [key ...] id [id ...]
    async fn cmd_xread(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xread' command".to_string(),
            ));
        }

        let mut idx = 0;
        let mut count: Option<usize> = None;
        let mut _block: Option<u64> = None;

        // Parse options
        while idx < args.len() {
            let arg = self.get_string_arg(args, idx, "XREAD")?.to_uppercase();
            match arg.as_str() {
                "COUNT" => {
                    idx += 1;
                    if idx < args.len() {
                        count = Some(self.get_int_arg(args, idx, "XREAD")? as usize);
                        idx += 1;
                    }
                }
                "BLOCK" => {
                    idx += 1;
                    if idx < args.len() {
                        _block = Some(self.get_int_arg(args, idx, "XREAD")? as u64);
                        idx += 1;
                    }
                }
                "STREAMS" => {
                    idx += 1;
                    break;
                }
                _ => {
                    idx += 1;
                }
            }
        }

        // Count remaining args to split between keys and IDs
        let remaining = args.len() - idx;
        if remaining < 2 || remaining % 2 != 0 {
            return Err(ProtocolError::RespError(
                "ERR Unbalanced XREAD list of streams: for each stream key an ID must be specified"
                    .to_string(),
            ));
        }

        let num_streams = remaining / 2;
        let keys_end = idx + num_streams;

        // Collect keys and IDs
        let keys: Vec<String> = (idx..keys_end)
            .map(|i| self.get_string_arg(args, i, "XREAD"))
            .collect::<Result<Vec<_>, _>>()?;

        let ids: Vec<String> = (keys_end..args.len())
            .map(|i| self.get_string_arg(args, i, "XREAD"))
            .collect::<Result<Vec<_>, _>>()?;

        // Process each stream
        let mut results = Vec::new();
        for (key, id) in keys.iter().zip(ids.iter()) {
            // Handle special $ ID (means "only new entries from now")
            let read_id = if id == "$" {
                "9999999999999-9999999999999"
            } else {
                id
            };

            let mut rpc_args = vec![serde_json::to_value(read_id).unwrap()];
            if let Some(c) = count {
                rpc_args.push(serde_json::to_value(c).unwrap());
            }

            let result = self
                .base
                .local_registry
                .execute_stream(key, "xread", &rpc_args)
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

            let entries: Vec<serde_json::Value> =
                serde_json::from_value(result).unwrap_or_default();

            if !entries.is_empty() {
                let resp_entries: Vec<RespValue> =
                    entries.iter().map(Self::entry_to_resp).collect();
                results.push(RespValue::Array(vec![
                    RespValue::BulkString(Bytes::from(key.as_bytes().to_vec())),
                    RespValue::Array(resp_entries),
                ]));
            }
        }

        debug!("XREAD -> {} streams with data", results.len());
        if results.is_empty() {
            Ok(RespValue::NullArray)
        } else {
            Ok(RespValue::Array(results))
        }
    }

    /// XTRIM key MAXLEN|MINID [=|~] threshold
    async fn cmd_xtrim(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xtrim' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XTRIM")?;
        let strategy = self.get_string_arg(args, 1, "XTRIM")?.to_uppercase();

        let (max_len, approximate) = match strategy.as_str() {
            "MAXLEN" => {
                let mut idx = 2;
                let mut approx = false;

                // Check for ~ or =
                let next = self.get_string_arg(args, idx, "XTRIM")?;
                if next == "~" {
                    approx = true;
                    idx += 1;
                } else if next == "=" {
                    idx += 1;
                }

                let threshold = self.get_int_arg(args, idx, "XTRIM")? as usize;
                (threshold, approx)
            }
            _ => {
                return Err(ProtocolError::RespError(
                    "ERR unsupported XTRIM strategy".to_string(),
                ));
            }
        };

        let result = self
            .base
            .local_registry
            .execute_stream(
                &key,
                "xtrim",
                &[
                    serde_json::to_value(max_len).unwrap(),
                    serde_json::to_value(approximate).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let removed: usize = serde_json::from_value(result).unwrap_or(0);

        debug!("XTRIM {} MAXLEN {} -> {} removed", key, max_len, removed);
        Ok(RespValue::Integer(removed as i64))
    }

    /// XDEL key id [id ...]
    async fn cmd_xdel(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xdel' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XDEL")?;
        let ids: Vec<String> = (1..args.len())
            .map(|i| self.get_string_arg(args, i, "XDEL"))
            .collect::<Result<Vec<_>, _>>()?;

        let result = self
            .base
            .local_registry
            .execute_stream(&key, "xdel", &[serde_json::to_value(&ids).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let deleted: usize = serde_json::from_value(result).unwrap_or(0);

        debug!("XDEL {} {:?} -> {}", key, ids, deleted);
        Ok(RespValue::Integer(deleted as i64))
    }

    /// XINFO STREAM key [FULL [COUNT count]]
    async fn cmd_xinfo(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xinfo' command".to_string(),
            ));
        }

        let subcommand = self.get_string_arg(args, 0, "XINFO")?.to_uppercase();
        let key = self.get_string_arg(args, 1, "XINFO")?;

        match subcommand.as_str() {
            "STREAM" => {
                let result = self
                    .base
                    .local_registry
                    .execute_stream(&key, "xinfo_stream", &[])
                    .await
                    .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

                let info: std::collections::HashMap<String, String> =
                    serde_json::from_value(result).unwrap_or_default();

                let mut resp_arr = Vec::new();
                for (k, v) in info {
                    resp_arr.push(RespValue::BulkString(Bytes::from(k.into_bytes())));
                    resp_arr.push(RespValue::BulkString(Bytes::from(v.into_bytes())));
                }

                debug!("XINFO STREAM {} -> {} fields", key, resp_arr.len() / 2);
                Ok(RespValue::Array(resp_arr))
            }
            "GROUPS" | "CONSUMERS" | "HELP" => Err(ProtocolError::RespError(format!(
                "ERR XINFO {} not yet implemented",
                subcommand
            ))),
            _ => Err(ProtocolError::RespError(format!(
                "ERR Unknown XINFO subcommand '{}'",
                subcommand
            ))),
        }
    }

    /// XGROUP CREATE key group id|$ [MKSTREAM] [ENTRIESREAD entries-read]
    async fn cmd_xgroup(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xgroup' command".to_string(),
            ));
        }

        let subcommand = self.get_string_arg(args, 0, "XGROUP")?.to_uppercase();

        match subcommand.as_str() {
            "CREATE" => {
                if args.len() < 4 {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'xgroup create' command".to_string(),
                    ));
                }

                let key = self.get_string_arg(args, 1, "XGROUP")?;
                let group_name = self.get_string_arg(args, 2, "XGROUP")?;
                let start_id = self.get_string_arg(args, 3, "XGROUP")?;

                let _result = self
                    .base
                    .local_registry
                    .execute_stream(
                        &key,
                        "xgroup_create",
                        &[
                            serde_json::to_value(&group_name).unwrap(),
                            serde_json::to_value(&start_id).unwrap(),
                        ],
                    )
                    .await
                    .map_err(|e| ProtocolError::RespError(format!("{}", e)))?;

                debug!("XGROUP CREATE {} {} {} -> OK", key, group_name, start_id);
                Ok(RespValue::SimpleString("OK".to_string()))
            }
            "DESTROY" => {
                if args.len() < 3 {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'xgroup destroy' command".to_string(),
                    ));
                }

                let key = self.get_string_arg(args, 1, "XGROUP")?;
                let group_name = self.get_string_arg(args, 2, "XGROUP")?;

                let result = self
                    .base
                    .local_registry
                    .execute_stream(
                        &key,
                        "xgroup_destroy",
                        &[serde_json::to_value(&group_name).unwrap()],
                    )
                    .await
                    .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

                let destroyed: i64 = serde_json::from_value(result).unwrap_or(0);

                debug!("XGROUP DESTROY {} {} -> {}", key, group_name, destroyed);
                Ok(RespValue::Integer(destroyed))
            }
            "SETID" | "CREATECONSUMER" | "DELCONSUMER" => Err(ProtocolError::RespError(format!(
                "ERR XGROUP {} not yet implemented",
                subcommand
            ))),
            _ => Err(ProtocolError::RespError(format!(
                "ERR Unknown XGROUP subcommand '{}'",
                subcommand
            ))),
        }
    }

    /// XREADGROUP GROUP group consumer [COUNT count] [BLOCK milliseconds] [NOACK] STREAMS key [key ...] id [id ...]
    async fn cmd_xreadgroup(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 5 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xreadgroup' command".to_string(),
            ));
        }

        let mut idx = 0;

        // Parse GROUP keyword
        let group_keyword = self.get_string_arg(args, idx, "XREADGROUP")?.to_uppercase();
        if group_keyword != "GROUP" {
            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
        }
        idx += 1;

        let group_name = self.get_string_arg(args, idx, "XREADGROUP")?;
        idx += 1;

        let consumer_name = self.get_string_arg(args, idx, "XREADGROUP")?;
        idx += 1;

        let mut count: Option<usize> = None;
        let mut _block: Option<u64> = None;
        let mut _noack = false;

        // Parse optional arguments
        while idx < args.len() {
            let arg = self.get_string_arg(args, idx, "XREADGROUP")?.to_uppercase();
            match arg.as_str() {
                "COUNT" => {
                    idx += 1;
                    if idx < args.len() {
                        count = Some(self.get_int_arg(args, idx, "XREADGROUP")? as usize);
                        idx += 1;
                    }
                }
                "BLOCK" => {
                    idx += 1;
                    if idx < args.len() {
                        _block = Some(self.get_int_arg(args, idx, "XREADGROUP")? as u64);
                        idx += 1;
                    }
                }
                "NOACK" => {
                    _noack = true;
                    idx += 1;
                }
                "STREAMS" => {
                    idx += 1;
                    break;
                }
                _ => {
                    idx += 1;
                }
            }
        }

        // Count remaining args to split between keys and IDs
        let remaining = args.len() - idx;
        if remaining < 2 || remaining % 2 != 0 {
            return Err(ProtocolError::RespError(
                "ERR Unbalanced XREADGROUP list of streams".to_string(),
            ));
        }

        let num_streams = remaining / 2;
        let keys_end = idx + num_streams;

        let keys: Vec<String> = (idx..keys_end)
            .map(|i| self.get_string_arg(args, i, "XREADGROUP"))
            .collect::<Result<Vec<_>, _>>()?;

        let ids: Vec<String> = (keys_end..args.len())
            .map(|i| self.get_string_arg(args, i, "XREADGROUP"))
            .collect::<Result<Vec<_>, _>>()?;

        // Process each stream
        let mut results = Vec::new();
        for (key, id) in keys.iter().zip(ids.iter()) {
            let mut rpc_args = vec![
                serde_json::to_value(&group_name).unwrap(),
                serde_json::to_value(&consumer_name).unwrap(),
                serde_json::to_value(id).unwrap(),
            ];
            if let Some(c) = count {
                rpc_args.push(serde_json::to_value(c).unwrap());
            }

            let result = self
                .base
                .local_registry
                .execute_stream(key, "xreadgroup", &rpc_args)
                .await;

            match result {
                Ok(value) => {
                    let entries: Vec<serde_json::Value> =
                        serde_json::from_value(value).unwrap_or_default();

                    if !entries.is_empty() {
                        let resp_entries: Vec<RespValue> =
                            entries.iter().map(Self::entry_to_resp).collect();
                        results.push(RespValue::Array(vec![
                            RespValue::BulkString(Bytes::from(key.as_bytes().to_vec())),
                            RespValue::Array(resp_entries),
                        ]));
                    }
                }
                Err(e) => {
                    return Err(ProtocolError::RespError(format!("{}", e)));
                }
            }
        }

        debug!(
            "XREADGROUP {} {} -> {} streams with data",
            group_name,
            consumer_name,
            results.len()
        );
        if results.is_empty() {
            Ok(RespValue::NullArray)
        } else {
            Ok(RespValue::Array(results))
        }
    }

    /// XACK key group id [id ...]
    async fn cmd_xack(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xack' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XACK")?;
        let group_name = self.get_string_arg(args, 1, "XACK")?;
        let ids: Vec<String> = (2..args.len())
            .map(|i| self.get_string_arg(args, i, "XACK"))
            .collect::<Result<Vec<_>, _>>()?;

        let result = self
            .base
            .local_registry
            .execute_stream(
                &key,
                "xack",
                &[
                    serde_json::to_value(&group_name).unwrap(),
                    serde_json::to_value(&ids).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("{}", e)))?;

        let acked: usize = serde_json::from_value(result).unwrap_or(0);

        debug!("XACK {} {} {:?} -> {}", key, group_name, ids, acked);
        Ok(RespValue::Integer(acked as i64))
    }

    /// XPENDING key group [[IDLE min-idle-time] start end count [consumer]]
    async fn cmd_xpending(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xpending' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XPENDING")?;
        let group_name = self.get_string_arg(args, 1, "XPENDING")?;

        let result = self
            .base
            .local_registry
            .execute_stream(
                &key,
                "xpending",
                &[serde_json::to_value(&group_name).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("{}", e)))?;

        // Result is (count, min_id, max_id, consumers)
        let (count, min_id, max_id, consumers): (
            usize,
            Option<String>,
            Option<String>,
            Vec<(String, usize)>,
        ) = serde_json::from_value(result).unwrap_or((0, None, None, vec![]));

        let mut resp_arr = Vec::new();
        resp_arr.push(RespValue::Integer(count as i64));
        resp_arr.push(
            min_id
                .map(|s| RespValue::BulkString(Bytes::from(s.into_bytes())))
                .unwrap_or(RespValue::NullBulkString),
        );
        resp_arr.push(
            max_id
                .map(|s| RespValue::BulkString(Bytes::from(s.into_bytes())))
                .unwrap_or(RespValue::NullBulkString),
        );

        if !consumers.is_empty() {
            let consumer_arr: Vec<RespValue> = consumers
                .iter()
                .map(|(name, pending_count)| {
                    RespValue::Array(vec![
                        RespValue::BulkString(Bytes::from(name.as_bytes().to_vec())),
                        RespValue::BulkString(Bytes::from(pending_count.to_string().into_bytes())),
                    ])
                })
                .collect();
            resp_arr.push(RespValue::Array(consumer_arr));
        } else {
            resp_arr.push(RespValue::NullArray);
        }

        debug!("XPENDING {} {} -> {} pending", key, group_name, count);
        Ok(RespValue::Array(resp_arr))
    }

    /// XSETID key last-id [ENTRIESADDED entries-added] [MAXDELETEDID max-deleted-id]
    async fn cmd_xsetid(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'xsetid' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "XSETID")?;
        let last_id = self.get_string_arg(args, 1, "XSETID")?;

        let _result = self
            .base
            .local_registry
            .execute_stream(&key, "xsetid", &[serde_json::to_value(&last_id).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("{}", e)))?;

        debug!("XSETID {} {} -> OK", key, last_id);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// XCLAIM - Stub for claiming pending entries
    async fn cmd_xclaim(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR XCLAIM not yet implemented".to_string(),
        ))
    }

    /// XAUTOCLAIM - Stub for auto-claiming pending entries
    async fn cmd_xautoclaim(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR XAUTOCLAIM not yet implemented".to_string(),
        ))
    }
}

#[async_trait]
impl CommandHandler for StreamCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "XADD" => self.cmd_xadd(args).await,
            "XLEN" => self.cmd_xlen(args).await,
            "XRANGE" => self.cmd_xrange(args).await,
            "XREVRANGE" => self.cmd_xrevrange(args).await,
            "XREAD" => self.cmd_xread(args).await,
            "XTRIM" => self.cmd_xtrim(args).await,
            "XDEL" => self.cmd_xdel(args).await,
            "XINFO" => self.cmd_xinfo(args).await,
            "XGROUP" => self.cmd_xgroup(args).await,
            "XREADGROUP" => self.cmd_xreadgroup(args).await,
            "XACK" => self.cmd_xack(args).await,
            "XPENDING" => self.cmd_xpending(args).await,
            "XSETID" => self.cmd_xsetid(args).await,
            "XCLAIM" => self.cmd_xclaim(args).await,
            "XAUTOCLAIM" => self.cmd_xautoclaim(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown stream command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "XADD",
            "XLEN",
            "XRANGE",
            "XREVRANGE",
            "XREAD",
            "XTRIM",
            "XDEL",
            "XINFO",
            "XGROUP",
            "XREADGROUP",
            "XACK",
            "XPENDING",
            "XSETID",
            "XCLAIM",
            "XAUTOCLAIM",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::traits::BaseCommandHandler;
    use super::*;

    #[tokio::test]
    async fn test_supported_commands() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry =
            Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = StreamCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
        };

        let commands = handler.supported_commands();
        assert!(commands.contains(&"XADD"));
        assert!(commands.contains(&"XREAD"));
        assert!(commands.contains(&"XRANGE"));
        assert!(commands.contains(&"XGROUP"));
        assert!(commands.contains(&"XREADGROUP"));
        assert!(commands.contains(&"XACK"));
        assert!(commands.contains(&"XPENDING"));
    }

    #[tokio::test]
    async fn test_xadd_and_xlen() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry =
            Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = StreamCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
        };

        // Test XADD
        let args = vec![
            RespValue::BulkString(Bytes::from("mystream")),
            RespValue::BulkString(Bytes::from("*")),
            RespValue::BulkString(Bytes::from("field1")),
            RespValue::BulkString(Bytes::from("value1")),
        ];
        let result = handler.handle("XADD", &args).await;
        assert!(result.is_ok());
        let id = result.unwrap();
        assert!(matches!(id, RespValue::BulkString(_)));

        // Test XLEN
        let args = vec![RespValue::BulkString(Bytes::from("mystream"))];
        let result = handler.handle("XLEN", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Integer(len) => assert_eq!(len, 1),
            _ => panic!("Expected integer"),
        }
    }

    #[tokio::test]
    async fn test_xrange() {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = orbit_client::OrbitClient::new_offline(client_config)
            .await
            .unwrap();
        let local_registry =
            Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        let handler = StreamCommands {
            base: BaseCommandHandler::new(Arc::new(orbit_client), local_registry),
        };

        // Add some entries
        for i in 0..3 {
            let args = vec![
                RespValue::BulkString(Bytes::from("rangestream")),
                RespValue::BulkString(Bytes::from("*")),
                RespValue::BulkString(Bytes::from("index")),
                RespValue::BulkString(Bytes::from(format!("{}", i))),
            ];
            handler.handle("XADD", &args).await.unwrap();
        }

        // Test XRANGE with - and +
        let args = vec![
            RespValue::BulkString(Bytes::from("rangestream")),
            RespValue::BulkString(Bytes::from("-")),
            RespValue::BulkString(Bytes::from("+")),
        ];
        let result = handler.handle("XRANGE", &args).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RespValue::Array(entries) => assert_eq!(entries.len(), 3),
            _ => panic!("Expected array"),
        }
    }
}
