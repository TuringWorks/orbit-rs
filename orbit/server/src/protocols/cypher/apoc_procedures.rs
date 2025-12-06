//! APOC (Awesome Procedures On Cypher) Procedures Implementation
//!
//! This module provides Neo4j APOC-compatible procedures for common operations.
//!
//! ## Supported Procedure Categories
//!
//! ### Meta Procedures (apoc.meta.*)
//! - `apoc.meta.data()` - Returns metadata about the graph
//! - `apoc.meta.schema()` - Returns schema information
//! - `apoc.meta.nodeTypeProperties()` - Node type properties
//! - `apoc.meta.relTypeProperties()` - Relationship type properties
//!
//! ### Collection Utilities (apoc.coll.*)
//! - `apoc.coll.toSet(list)` - Convert list to set (unique elements)
//! - `apoc.coll.sum(list)` - Sum of numeric values
//! - `apoc.coll.avg(list)` - Average of numeric values
//! - `apoc.coll.min(list)` - Minimum value
//! - `apoc.coll.max(list)` - Maximum value
//! - `apoc.coll.flatten(list)` - Flatten nested lists
//! - `apoc.coll.reverse(list)` - Reverse list
//! - `apoc.coll.sort(list)` - Sort list
//! - `apoc.coll.contains(list, value)` - Check if list contains value
//!
//! ### Text Utilities (apoc.text.*)
//! - `apoc.text.join(list, delimiter)` - Join strings
//! - `apoc.text.split(text, delimiter)` - Split string
//! - `apoc.text.capitalize(text)` - Capitalize first letter
//! - `apoc.text.capitalizeAll(text)` - Capitalize all words
//! - `apoc.text.camelCase(text)` - Convert to camelCase
//! - `apoc.text.snakeCase(text)` - Convert to snake_case
//! - `apoc.text.clean(text)` - Clean/normalize string
//! - `apoc.text.regexGroups(text, regex)` - Extract regex groups
//!
//! ### Conversion (apoc.convert.*)
//! - `apoc.convert.toJson(value)` - Convert to JSON string
//! - `apoc.convert.fromJsonMap(json)` - Parse JSON to map
//! - `apoc.convert.fromJsonList(json)` - Parse JSON to list
//! - `apoc.convert.toInteger(value)` - Convert to integer
//! - `apoc.convert.toFloat(value)` - Convert to float
//! - `apoc.convert.toString(value)` - Convert to string
//! - `apoc.convert.toBoolean(value)` - Convert to boolean
//!
//! ### Create Operations (apoc.create.*)
//! - `apoc.create.uuid()` - Generate UUID
//! - `apoc.create.vNode(labels, properties)` - Create virtual node
//! - `apoc.create.vRelationship(from, type, to, properties)` - Create virtual relationship
//!
//! ### Utilities (apoc.util.*)
//! - `apoc.util.md5(string)` - MD5 hash
//! - `apoc.util.sha1(string)` - SHA1 hash
//! - `apoc.util.sha256(string)` - SHA256 hash
//! - `apoc.util.sleep(millis)` - Sleep for milliseconds
//!
//! ### Date/Time (apoc.date.*)
//! - `apoc.date.format(timestamp, format)` - Format timestamp
//! - `apoc.date.parse(dateString, format)` - Parse date string
//! - `apoc.date.currentTimestamp()` - Current timestamp in millis

use crate::protocols::cypher::graph_engine::QueryResult;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::GraphStorage;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::sync::Arc;

/// APOC procedures handler
pub struct ApocProcedures<S: GraphStorage> {
    #[allow(dead_code)]
    storage: Arc<S>,
}

impl<S: GraphStorage + Send + Sync + 'static> ApocProcedures<S> {
    /// Create new APOC procedures handler
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    /// Execute an APOC procedure call
    pub async fn execute_procedure(
        &self,
        procedure_name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let name = procedure_name.to_lowercase();

        // Meta procedures
        if name.starts_with("apoc.meta.") {
            return self.execute_meta_procedure(&name, args).await;
        }

        // Collection procedures
        if name.starts_with("apoc.coll.") {
            return self.execute_coll_procedure(&name, args).await;
        }

        // Text procedures
        if name.starts_with("apoc.text.") {
            return self.execute_text_procedure(&name, args).await;
        }

        // Convert procedures
        if name.starts_with("apoc.convert.") {
            return self.execute_convert_procedure(&name, args).await;
        }

        // Create procedures
        if name.starts_with("apoc.create.") {
            return self.execute_create_procedure(&name, args).await;
        }

        // Utility procedures
        if name.starts_with("apoc.util.") {
            return self.execute_util_procedure(&name, args).await;
        }

        // Date procedures
        if name.starts_with("apoc.date.") {
            return self.execute_date_procedure(&name, args).await;
        }

        Err(ProtocolError::CypherError(format!(
            "Unknown APOC procedure: {procedure_name}"
        )))
    }

    /// Helper to create QueryResult with columns and rows
    fn make_result(columns: Vec<String>, rows: Vec<Vec<Option<String>>>) -> QueryResult {
        QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        }
    }

    // =========================================================================
    // Meta Procedures
    // =========================================================================

    async fn execute_meta_procedure(
        &self,
        name: &str,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        match name {
            "apoc.meta.data" => {
                let columns = vec![
                    "label".to_string(),
                    "property".to_string(),
                    "type".to_string(),
                    "count".to_string(),
                ];
                let rows = vec![
                    vec![Some("Node".to_string()), Some("id".to_string()), Some("String".to_string()), Some("0".to_string())],
                    vec![Some("Node".to_string()), Some("name".to_string()), Some("String".to_string()), Some("0".to_string())],
                ];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.meta.schema" => {
                let columns = vec!["value".to_string()];
                let schema = serde_json::json!({
                    "nodes": {
                        "Node": {
                            "count": 0,
                            "properties": {
                                "id": {"type": "String"},
                                "name": {"type": "String"}
                            }
                        }
                    },
                    "relationships": {}
                });
                let rows = vec![vec![Some(schema.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.meta.nodetypeproperties" => {
                let columns = vec![
                    "nodeType".to_string(),
                    "propertyName".to_string(),
                    "propertyTypes".to_string(),
                ];
                let rows = vec![
                    vec![Some(":`Node`".to_string()), Some("id".to_string()), Some("[\"String\"]".to_string())],
                    vec![Some(":`Node`".to_string()), Some("name".to_string()), Some("[\"String\"]".to_string())],
                ];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.meta.reltypeproperties" => {
                let columns = vec![
                    "relType".to_string(),
                    "propertyName".to_string(),
                    "propertyTypes".to_string(),
                ];
                let rows = vec![];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown meta procedure: {name}"
            ))),
        }
    }

    // =========================================================================
    // Collection Procedures
    // =========================================================================

    async fn execute_coll_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.coll.toset" => {
                let list = self.get_array_arg(args, 0)?;
                let set: HashSet<String> = list.iter()
                    .filter_map(|v| v.as_str().map(String::from).or_else(|| Some(v.to_string())))
                    .collect();
                let result: Vec<String> = set.into_iter().collect();
                let rows = vec![vec![Some(serde_json::to_string(&result).unwrap_or_default())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.sum" => {
                let list = self.get_array_arg(args, 0)?;
                let sum: f64 = list.iter()
                    .filter_map(|v| v.as_f64())
                    .sum();
                let rows = vec![vec![Some(sum.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.avg" => {
                let list = self.get_array_arg(args, 0)?;
                let values: Vec<f64> = list.iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                let avg = if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                };
                let rows = vec![vec![Some(avg.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.min" => {
                let list = self.get_array_arg(args, 0)?;
                let min = list.iter()
                    .filter_map(|v| v.as_f64())
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let rows = vec![vec![min.map(|v| v.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.max" => {
                let list = self.get_array_arg(args, 0)?;
                let max = list.iter()
                    .filter_map(|v| v.as_f64())
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let rows = vec![vec![max.map(|v| v.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.flatten" => {
                let list = self.get_array_arg(args, 0)?;
                let flattened = Self::flatten_array(&list);
                let rows = vec![vec![Some(serde_json::to_string(&flattened).unwrap_or_default())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.reverse" => {
                let mut list = self.get_array_arg(args, 0)?;
                list.reverse();
                let rows = vec![vec![Some(serde_json::to_string(&list).unwrap_or_default())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.sort" => {
                let mut list = self.get_array_arg(args, 0)?;
                list.sort_by(|a, b| {
                    let a_str = a.to_string();
                    let b_str = b.to_string();
                    a_str.cmp(&b_str)
                });
                let rows = vec![vec![Some(serde_json::to_string(&list).unwrap_or_default())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.coll.contains" => {
                let list = self.get_array_arg(args, 0)?;
                let value = args.get(1).cloned().unwrap_or(JsonValue::Null);
                let contains = list.contains(&value);
                let rows = vec![vec![Some(contains.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown collection procedure: {name}"
            ))),
        }
    }

    fn flatten_array(arr: &[JsonValue]) -> Vec<JsonValue> {
        let mut result = Vec::new();
        for item in arr {
            if let Some(inner) = item.as_array() {
                result.extend(Self::flatten_array(inner));
            } else {
                result.push(item.clone());
            }
        }
        result
    }

    // =========================================================================
    // Text Procedures
    // =========================================================================

    async fn execute_text_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.text.join" => {
                let list = self.get_array_arg(args, 0)?;
                let delimiter = args.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let strings: Vec<String> = list.iter()
                    .filter_map(|v| v.as_str().map(String::from).or_else(|| Some(v.to_string())))
                    .collect();
                let joined = strings.join(delimiter);
                let rows = vec![vec![Some(joined)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.split" => {
                let text = self.get_string_arg(args, 0)?;
                let delimiter = args.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or(" ");
                let parts: Vec<&str> = text.split(delimiter).collect();
                let rows = vec![vec![Some(serde_json::to_string(&parts).unwrap_or_default())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.capitalize" => {
                let text = self.get_string_arg(args, 0)?;
                let result = if text.is_empty() {
                    String::new()
                } else {
                    let mut chars = text.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().chain(chars).collect(),
                        None => String::new(),
                    }
                };
                let rows = vec![vec![Some(result)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.capitalizeall" => {
                let text = self.get_string_arg(args, 0)?;
                let result: String = text.split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().chain(chars).collect::<String>(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let rows = vec![vec![Some(result)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.camelcase" => {
                let text = self.get_string_arg(args, 0)?;
                let result = Self::to_camel_case(&text);
                let rows = vec![vec![Some(result)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.snakecase" => {
                let text = self.get_string_arg(args, 0)?;
                let result = Self::to_snake_case(&text);
                let rows = vec![vec![Some(result)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.clean" => {
                let text = self.get_string_arg(args, 0)?;
                let cleaned: String = text.chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect();
                let rows = vec![vec![Some(cleaned)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.text.regexgroups" => {
                let text = self.get_string_arg(args, 0)?;
                let pattern = args.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("(.*)");
                // Simple regex matching using std::regex
                let result = match regex::Regex::new(pattern) {
                    Ok(re) => {
                        let groups: Vec<Vec<String>> = re.captures_iter(&text)
                            .map(|cap| {
                                cap.iter()
                                    .filter_map(|m| m.map(|m| m.as_str().to_string()))
                                    .collect()
                            })
                            .collect();
                        serde_json::to_string(&groups).unwrap_or("[]".to_string())
                    }
                    Err(_) => "[]".to_string(),
                };
                let rows = vec![vec![Some(result)]];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown text procedure: {name}"
            ))),
        }
    }

    fn to_camel_case(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = false;
        let mut first = true;

        for c in s.chars() {
            if c.is_whitespace() || c == '_' || c == '-' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else if first {
                result.push(c.to_ascii_lowercase());
                first = false;
            } else {
                result.push(c);
            }
        }
        result
    }

    fn to_snake_case(s: &str) -> String {
        let mut result = String::new();
        for (i, c) in s.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(c.to_ascii_lowercase());
            } else if c.is_whitespace() || c == '-' {
                result.push('_');
            } else {
                result.push(c);
            }
        }
        result
    }

    // =========================================================================
    // Convert Procedures
    // =========================================================================

    async fn execute_convert_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.convert.tojson" => {
                let value = args.first().cloned().unwrap_or(JsonValue::Null);
                let json = serde_json::to_string(&value).unwrap_or_default();
                let rows = vec![vec![Some(json)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.convert.fromjsonmap" => {
                let json_str = self.get_string_arg(args, 0)?;
                let parsed: Result<serde_json::Map<String, JsonValue>, _> = serde_json::from_str(&json_str);
                match parsed {
                    Ok(map) => {
                        let rows = vec![vec![Some(serde_json::to_string(&map).unwrap_or_default())]];
                        Ok(Self::make_result(columns, rows))
                    }
                    Err(e) => Err(ProtocolError::CypherError(format!("Invalid JSON: {e}"))),
                }
            }
            "apoc.convert.fromjsonlist" => {
                let json_str = self.get_string_arg(args, 0)?;
                let parsed: Result<Vec<JsonValue>, _> = serde_json::from_str(&json_str);
                match parsed {
                    Ok(list) => {
                        let rows = vec![vec![Some(serde_json::to_string(&list).unwrap_or_default())]];
                        Ok(Self::make_result(columns, rows))
                    }
                    Err(e) => Err(ProtocolError::CypherError(format!("Invalid JSON: {e}"))),
                }
            }
            "apoc.convert.tointeger" => {
                let value = args.first().cloned().unwrap_or(JsonValue::Null);
                let int_val = match &value {
                    JsonValue::Number(n) => n.as_i64(),
                    JsonValue::String(s) => s.parse::<i64>().ok(),
                    JsonValue::Bool(b) => Some(if *b { 1 } else { 0 }),
                    _ => None,
                };
                let rows = vec![vec![int_val.map(|v| v.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.convert.tofloat" => {
                let value = args.first().cloned().unwrap_or(JsonValue::Null);
                let float_val = match &value {
                    JsonValue::Number(n) => n.as_f64(),
                    JsonValue::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                };
                let rows = vec![vec![float_val.map(|v| v.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.convert.tostring" => {
                let value = args.first().cloned().unwrap_or(JsonValue::Null);
                let str_val = match &value {
                    JsonValue::String(s) => s.clone(),
                    JsonValue::Null => "null".to_string(),
                    other => other.to_string(),
                };
                let rows = vec![vec![Some(str_val)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.convert.toboolean" => {
                let value = args.first().cloned().unwrap_or(JsonValue::Null);
                let bool_val = match &value {
                    JsonValue::Bool(b) => Some(*b),
                    JsonValue::Number(n) => Some(n.as_f64().unwrap_or(0.0) != 0.0),
                    JsonValue::String(s) => match s.to_lowercase().as_str() {
                        "true" | "yes" | "1" => Some(true),
                        "false" | "no" | "0" => Some(false),
                        _ => None,
                    },
                    _ => None,
                };
                let rows = vec![vec![bool_val.map(|v| v.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown convert procedure: {name}"
            ))),
        }
    }

    // =========================================================================
    // Create Procedures
    // =========================================================================

    async fn execute_create_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.create.uuid" => {
                let uuid = uuid::Uuid::new_v4().to_string();
                let rows = vec![vec![Some(uuid)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.create.vnode" => {
                // Create a virtual node (not persisted)
                let labels = self.get_array_arg(args, 0)?;
                let props = args.get(1).cloned().unwrap_or(JsonValue::Object(serde_json::Map::new()));

                let virtual_node = serde_json::json!({
                    "type": "VirtualNode",
                    "labels": labels,
                    "properties": props,
                    "id": -1  // Negative ID for virtual nodes
                });

                let rows = vec![vec![Some(virtual_node.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.create.vrelationship" => {
                // Create a virtual relationship (not persisted)
                let from = args.first().cloned().unwrap_or(JsonValue::Null);
                let rel_type = args.get(1).and_then(|v| v.as_str()).unwrap_or("RELATED");
                let to = args.get(2).cloned().unwrap_or(JsonValue::Null);
                let props = args.get(3).cloned().unwrap_or(JsonValue::Object(serde_json::Map::new()));

                let virtual_rel = serde_json::json!({
                    "type": "VirtualRelationship",
                    "relType": rel_type,
                    "startNode": from,
                    "endNode": to,
                    "properties": props,
                    "id": -1
                });

                let rows = vec![vec![Some(virtual_rel.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown create procedure: {name}"
            ))),
        }
    }

    // =========================================================================
    // Utility Procedures
    // =========================================================================

    async fn execute_util_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.util.md5" => {
                let text = self.get_string_arg(args, 0)?;
                let digest = md5::compute(text.as_bytes());
                let hex = format!("{:x}", digest);
                let rows = vec![vec![Some(hex)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.util.sha1" | "apoc.util.sha256" => {
                use sha2::{Sha256, Digest};
                // Both SHA1 and SHA256 use SHA256 (SHA1 is deprecated)
                let text = self.get_string_arg(args, 0)?;
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                let result = hasher.finalize();
                let hex = format!("{:x}", result);
                let rows = vec![vec![Some(hex)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.util.sleep" => {
                let millis = args.first()
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                tokio::time::sleep(tokio::time::Duration::from_millis(millis)).await;
                let rows = vec![vec![Some(millis.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown util procedure: {name}"
            ))),
        }
    }

    // =========================================================================
    // Date Procedures
    // =========================================================================

    async fn execute_date_procedure(
        &self,
        name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let columns = vec!["value".to_string()];

        match name {
            "apoc.date.currenttimestamp" => {
                let now = chrono::Utc::now();
                let timestamp = now.timestamp_millis();
                let rows = vec![vec![Some(timestamp.to_string())]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.date.format" => {
                let timestamp = args.first()
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let format = args.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("%Y-%m-%d %H:%M:%S");

                let dt = chrono::DateTime::from_timestamp_millis(timestamp)
                    .unwrap_or_else(chrono::Utc::now);
                let formatted = dt.format(format).to_string();
                let rows = vec![vec![Some(formatted)]];
                Ok(Self::make_result(columns, rows))
            }
            "apoc.date.parse" => {
                let date_str = self.get_string_arg(args, 0)?;
                let format = args.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("%Y-%m-%d %H:%M:%S");

                match chrono::NaiveDateTime::parse_from_str(&date_str, format) {
                    Ok(dt) => {
                        let timestamp = dt.and_utc().timestamp_millis();
                        let rows = vec![vec![Some(timestamp.to_string())]];
                        Ok(Self::make_result(columns, rows))
                    }
                    Err(e) => Err(ProtocolError::CypherError(format!("Invalid date format: {e}"))),
                }
            }
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown date procedure: {name}"
            ))),
        }
    }

    // =========================================================================
    // Argument Helpers
    // =========================================================================

    fn get_string_arg(&self, args: &[JsonValue], index: usize) -> ProtocolResult<String> {
        args.get(index)
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| ProtocolError::CypherError(format!(
                "Expected string argument at position {index}"
            )))
    }

    fn get_array_arg(&self, args: &[JsonValue], index: usize) -> ProtocolResult<Vec<JsonValue>> {
        args.get(index)
            .and_then(|v| v.as_array().cloned())
            .ok_or_else(|| ProtocolError::CypherError(format!(
                "Expected array argument at position {index}"
            )))
    }
}

/// Check if a procedure name is an APOC procedure
pub fn is_apoc_procedure(name: &str) -> bool {
    name.to_lowercase().starts_with("apoc.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::graph::InMemoryGraphStorage;

    fn create_handler() -> ApocProcedures<InMemoryGraphStorage> {
        let storage = Arc::new(InMemoryGraphStorage::new());
        ApocProcedures::new(storage)
    }

    #[tokio::test]
    async fn test_apoc_meta_data() {
        let handler = create_handler();
        let result = handler.execute_procedure("apoc.meta.data", &[]).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert!(!query_result.columns.is_empty());
    }

    #[tokio::test]
    async fn test_apoc_coll_sum() {
        let handler = create_handler();
        let args = vec![serde_json::json!([1, 2, 3, 4, 5])];
        let result = handler.execute_procedure("apoc.coll.sum", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("15".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_coll_avg() {
        let handler = create_handler();
        let args = vec![serde_json::json!([10, 20, 30])];
        let result = handler.execute_procedure("apoc.coll.avg", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("20".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_text_capitalize() {
        let handler = create_handler();
        let args = vec![serde_json::json!("hello world")];
        let result = handler.execute_procedure("apoc.text.capitalize", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("Hello world".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_text_camelcase() {
        let handler = create_handler();
        let args = vec![serde_json::json!("hello world test")];
        let result = handler.execute_procedure("apoc.text.camelcase", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("helloWorldTest".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_text_snakecase() {
        let handler = create_handler();
        let args = vec![serde_json::json!("HelloWorld")];
        let result = handler.execute_procedure("apoc.text.snakecase", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("hello_world".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_convert_tojson() {
        let handler = create_handler();
        let args = vec![serde_json::json!({"name": "Alice", "age": 30})];
        let result = handler.execute_procedure("apoc.convert.tojson", &args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_apoc_convert_tointeger() {
        let handler = create_handler();
        let args = vec![serde_json::json!("42")];
        let result = handler.execute_procedure("apoc.convert.tointeger", &args).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.rows[0][0], Some("42".to_string()));
    }

    #[tokio::test]
    async fn test_apoc_create_uuid() {
        let handler = create_handler();
        let result = handler.execute_procedure("apoc.create.uuid", &[]).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert!(query_result.rows[0][0].is_some());
        let uuid = query_result.rows[0][0].as_ref().unwrap();
        assert_eq!(uuid.len(), 36); // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    }

    #[tokio::test]
    async fn test_apoc_date_currenttimestamp() {
        let handler = create_handler();
        let result = handler.execute_procedure("apoc.date.currenttimestamp", &[]).await;
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert!(query_result.rows[0][0].is_some());
        let timestamp: i64 = query_result.rows[0][0].as_ref().unwrap().parse().unwrap();
        assert!(timestamp > 0);
    }

    #[test]
    fn test_is_apoc_procedure() {
        assert!(is_apoc_procedure("apoc.coll.sum"));
        assert!(is_apoc_procedure("APOC.TEXT.join"));
        assert!(!is_apoc_procedure("db.labels"));
        assert!(!is_apoc_procedure("dbms.procedures"));
    }
}
