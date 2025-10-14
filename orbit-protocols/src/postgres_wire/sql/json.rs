//! JSON/JSONB Processing Module
//!
//! This module implements enhanced JSON and JSONB functionality including:
//! - JSON path expressions and operators
//! - JSON manipulation functions (jsonb_set, jsonb_insert, jsonb_delete_path)
//! - JSON aggregation functions (json_agg, jsonb_agg, json_object_agg)
//! - JSON schema validation
//! - Binary JSON (JSONB) storage optimization

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::types::SqlValue;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;

/// JSON path expression component
#[derive(Debug, Clone, PartialEq)]
pub enum JsonPathElement {
    /// Object field access by key
    Key(String),
    /// Array element access by index
    Index(usize),
    /// Array slice [start:end]
    Slice { start: Option<usize>, end: Option<usize> },
    /// Recursive descent (..)
    RecursiveDescent,
    /// Wildcard (*) - all elements
    Wildcard,
}

/// JSON path - sequence of path elements
#[derive(Debug, Clone, PartialEq)]
pub struct JsonPath {
    pub elements: Vec<JsonPathElement>,
}

impl JsonPath {
    /// Create a new JSON path from a string array
    /// Example: ["key1", "0", "key2"] for $.key1[0].key2
    pub fn from_text_array(path: &[String]) -> ProtocolResult<Self> {
        let mut elements = Vec::new();
        
        for component in path {
            if let Ok(index) = component.parse::<usize>() {
                elements.push(JsonPathElement::Index(index));
            } else {
                elements.push(JsonPathElement::Key(component.clone()));
            }
        }
        
        Ok(JsonPath { elements })
    }

    /// Navigate through a JSON value using this path
    pub fn navigate<'a>(&self, value: &'a JsonValue) -> Option<&'a JsonValue> {
        let mut current = value;
        
        for element in &self.elements {
            match element {
                JsonPathElement::Key(key) => {
                    current = current.get(key)?;
                }
                JsonPathElement::Index(idx) => {
                    current = current.get(idx)?;
                }
                JsonPathElement::Slice { .. } => {
                    // Slicing returns array, not supported in simple navigation
                    return None;
                }
                JsonPathElement::RecursiveDescent | JsonPathElement::Wildcard => {
                    // Complex operations not supported in simple navigation
                    return None;
                }
            }
        }
        
        Some(current)
    }
}

/// JSON manipulation operations
pub struct JsonOperations;

impl JsonOperations {
    /// Extract JSON field using -> operator
    pub fn json_extract(json: &JsonValue, key: &str) -> ProtocolResult<SqlValue> {
        match json.get(key) {
            Some(value) => Ok(SqlValue::Json(value.clone())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Extract JSON field as text using ->> operator
    pub fn json_extract_text(json: &JsonValue, key: &str) -> ProtocolResult<SqlValue> {
        match json.get(key) {
            Some(value) => Ok(SqlValue::Text(value.to_string())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Extract JSON sub-object at path using #> operator
    pub fn json_path_extract(json: &JsonValue, path: &JsonPath) -> ProtocolResult<SqlValue> {
        match path.navigate(json) {
            Some(value) => Ok(SqlValue::Json(value.clone())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Extract JSON sub-object at path as text using #>> operator
    pub fn json_path_extract_text(json: &JsonValue, path: &JsonPath) -> ProtocolResult<SqlValue> {
        match path.navigate(json) {
            Some(value) => Ok(SqlValue::Text(value.to_string())),
            None => Ok(SqlValue::Null),
        }
    }

    /// Check if left JSON contains right JSON using @> operator
    pub fn json_contains(left: &JsonValue, right: &JsonValue) -> ProtocolResult<SqlValue> {
        let result = contains_json(left, right);
        Ok(SqlValue::Boolean(result))
    }

    /// Check if left JSON is contained by right JSON using <@ operator
    pub fn json_contained_by(left: &JsonValue, right: &JsonValue) -> ProtocolResult<SqlValue> {
        let result = contains_json(right, left);
        Ok(SqlValue::Boolean(result))
    }

    /// Check if string exists as top-level key using ? operator
    pub fn json_exists(json: &JsonValue, key: &str) -> ProtocolResult<SqlValue> {
        let result = match json {
            JsonValue::Object(map) => map.contains_key(key),
            _ => false,
        };
        Ok(SqlValue::Boolean(result))
    }

    /// Check if any strings exist as top-level keys using ?| operator
    pub fn json_exists_any(json: &JsonValue, keys: &[String]) -> ProtocolResult<SqlValue> {
        let result = match json {
            JsonValue::Object(map) => keys.iter().any(|key| map.contains_key(key)),
            _ => false,
        };
        Ok(SqlValue::Boolean(result))
    }

    /// Check if all strings exist as top-level keys using ?& operator
    pub fn json_exists_all(json: &JsonValue, keys: &[String]) -> ProtocolResult<SqlValue> {
        let result = match json {
            JsonValue::Object(map) => keys.iter().all(|key| map.contains_key(key)),
            _ => false,
        };
        Ok(SqlValue::Boolean(result))
    }

    /// Concatenate two JSON values using || operator
    pub fn json_concat(left: &JsonValue, right: &JsonValue) -> ProtocolResult<SqlValue> {
        let result = match (left, right) {
            (JsonValue::Object(left_map), JsonValue::Object(right_map)) => {
                let mut merged = left_map.clone();
                merged.extend(right_map.clone());
                JsonValue::Object(merged)
            }
            (JsonValue::Array(left_arr), JsonValue::Array(right_arr)) => {
                let mut merged = left_arr.clone();
                merged.extend(right_arr.clone());
                JsonValue::Array(merged)
            }
            _ => return Err(ProtocolError::PostgresError(
                "Cannot concatenate non-compatible JSON types".to_string()
            )),
        };
        Ok(SqlValue::Jsonb(result))
    }

    /// Set value in JSONB using jsonb_set function
    pub fn jsonb_set(
        json: &JsonValue,
        path: &JsonPath,
        new_value: &JsonValue,
        create_missing: bool,
    ) -> ProtocolResult<SqlValue> {
        let mut result = json.clone();
        set_json_value(&mut result, &path.elements, new_value, create_missing)?;
        Ok(SqlValue::Jsonb(result))
    }

    /// Insert value into JSONB using jsonb_insert function
    pub fn jsonb_insert(
        json: &JsonValue,
        path: &JsonPath,
        new_value: &JsonValue,
        insert_after: bool,
    ) -> ProtocolResult<SqlValue> {
        let mut result = json.clone();
        insert_json_value(&mut result, &path.elements, new_value, insert_after)?;
        Ok(SqlValue::Jsonb(result))
    }

    /// Delete path from JSONB using jsonb_delete_path function
    pub fn jsonb_delete_path(json: &JsonValue, path: &JsonPath) -> ProtocolResult<SqlValue> {
        let mut result = json.clone();
        delete_json_path(&mut result, &path.elements)?;
        Ok(SqlValue::Jsonb(result))
    }

    /// Build JSON array from SQL values (json_agg)
    pub fn json_agg(values: Vec<SqlValue>) -> ProtocolResult<SqlValue> {
        let json_values: Vec<JsonValue> = values
            .into_iter()
            .map(sql_value_to_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SqlValue::Json(JsonValue::Array(json_values)))
    }

    /// Build JSONB array from SQL values (jsonb_agg)
    pub fn jsonb_agg(values: Vec<SqlValue>) -> ProtocolResult<SqlValue> {
        let json_values: Vec<JsonValue> = values
            .into_iter()
            .map(sql_value_to_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SqlValue::Jsonb(JsonValue::Array(json_values)))
    }

    /// Build JSON object from key-value pairs (json_object_agg)
    pub fn json_object_agg(
        keys: Vec<SqlValue>,
        values: Vec<SqlValue>,
    ) -> ProtocolResult<SqlValue> {
        if keys.len() != values.len() {
            return Err(ProtocolError::PostgresError(
                "Key and value arrays must have the same length".to_string(),
            ));
        }

        let mut map = Map::new();
        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            let key_str = match key {
                SqlValue::Text(s) => s,
                SqlValue::Varchar(s) => s,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "Keys must be text values".to_string(),
                    ))
                }
            };
            map.insert(key_str, sql_value_to_json(value)?);
        }
        Ok(SqlValue::Json(JsonValue::Object(map)))
    }

    /// Build JSONB object from key-value pairs (jsonb_object_agg)
    pub fn jsonb_object_agg(
        keys: Vec<SqlValue>,
        values: Vec<SqlValue>,
    ) -> ProtocolResult<SqlValue> {
        if keys.len() != values.len() {
            return Err(ProtocolError::PostgresError(
                "Key and value arrays must have the same length".to_string(),
            ));
        }

        let mut map = Map::new();
        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            let key_str = match key {
                SqlValue::Text(s) => s,
                SqlValue::Varchar(s) => s,
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "Keys must be text values".to_string(),
                    ))
                }
            };
            map.insert(key_str, sql_value_to_json(value)?);
        }
        Ok(SqlValue::Jsonb(JsonValue::Object(map)))
    }
}

/// Helper function to check if left JSON contains right JSON
fn contains_json(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(left_map), JsonValue::Object(right_map)) => {
            right_map.iter().all(|(key, right_val)| {
                left_map
                    .get(key)
                    .map(|left_val| contains_json(left_val, right_val))
                    .unwrap_or(false)
            })
        }
        (JsonValue::Array(left_arr), JsonValue::Array(right_arr)) => {
            right_arr.iter().all(|right_val| {
                left_arr.iter().any(|left_val| contains_json(left_val, right_val))
            })
        }
        (left, right) => left == right,
    }
}

/// Helper function to set a value in JSON at the given path
fn set_json_value(
    json: &mut JsonValue,
    path: &[JsonPathElement],
    new_value: &JsonValue,
    create_missing: bool,
) -> ProtocolResult<()> {
    if path.is_empty() {
        return Err(ProtocolError::PostgresError("Path cannot be empty".to_string()));
    }

    if path.len() == 1 {
        match &path[0] {
            JsonPathElement::Key(key) => {
                if let JsonValue::Object(map) = json {
                    if create_missing || map.contains_key(key) {
                        map.insert(key.clone(), new_value.clone());
                    }
                }
            }
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    if *idx < arr.len() {
                        arr[*idx] = new_value.clone();
                    }
                }
            }
            _ => {}
        }
    } else {
        let first = &path[0];
        let rest = &path[1..];

        match first {
            JsonPathElement::Key(key) => {
                if let JsonValue::Object(map) = json {
                    if let Some(child) = map.get_mut(key) {
                        set_json_value(child, rest, new_value, create_missing)?;
                    } else if create_missing {
                        let mut new_child = JsonValue::Object(Map::new());
                        set_json_value(&mut new_child, rest, new_value, create_missing)?;
                        map.insert(key.clone(), new_child);
                    }
                }
            }
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    if *idx < arr.len() {
                        set_json_value(&mut arr[*idx], rest, new_value, create_missing)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Helper function to insert a value in JSON at the given path
fn insert_json_value(
    json: &mut JsonValue,
    path: &[JsonPathElement],
    new_value: &JsonValue,
    insert_after: bool,
) -> ProtocolResult<()> {
    if path.is_empty() {
        return Err(ProtocolError::PostgresError("Path cannot be empty".to_string()));
    }

    if path.len() == 1 {
        match &path[0] {
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    let insert_pos = if insert_after {
                        (*idx + 1).min(arr.len())
                    } else {
                        (*idx).min(arr.len())
                    };
                    arr.insert(insert_pos, new_value.clone());
                }
            }
            _ => {
                return Err(ProtocolError::PostgresError(
                    "Insert only works with array indices".to_string(),
                ));
            }
        }
    } else {
        let first = &path[0];
        let rest = &path[1..];

        match first {
            JsonPathElement::Key(key) => {
                if let JsonValue::Object(map) = json {
                    if let Some(child) = map.get_mut(key) {
                        insert_json_value(child, rest, new_value, insert_after)?;
                    }
                }
            }
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    if *idx < arr.len() {
                        insert_json_value(&mut arr[*idx], rest, new_value, insert_after)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Helper function to delete a path from JSON
fn delete_json_path(json: &mut JsonValue, path: &[JsonPathElement]) -> ProtocolResult<()> {
    if path.is_empty() {
        return Err(ProtocolError::PostgresError("Path cannot be empty".to_string()));
    }

    if path.len() == 1 {
        match &path[0] {
            JsonPathElement::Key(key) => {
                if let JsonValue::Object(map) = json {
                    map.remove(key);
                }
            }
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    if *idx < arr.len() {
                        arr.remove(*idx);
                    }
                }
            }
            _ => {}
        }
    } else {
        let first = &path[0];
        let rest = &path[1..];

        match first {
            JsonPathElement::Key(key) => {
                if let JsonValue::Object(map) = json {
                    if let Some(child) = map.get_mut(key) {
                        delete_json_path(child, rest)?;
                    }
                }
            }
            JsonPathElement::Index(idx) => {
                if let JsonValue::Array(arr) = json {
                    if *idx < arr.len() {
                        delete_json_path(&mut arr[*idx], rest)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Convert SqlValue to JSON Value
fn sql_value_to_json(value: SqlValue) -> ProtocolResult<JsonValue> {
    match value {
        SqlValue::Null => Ok(JsonValue::Null),
        SqlValue::Boolean(b) => Ok(JsonValue::Bool(b)),
        SqlValue::SmallInt(i) => Ok(json!(i)),
        SqlValue::Integer(i) => Ok(json!(i)),
        SqlValue::BigInt(i) => Ok(json!(i)),
        SqlValue::Real(f) => Ok(json!(f)),
        SqlValue::DoublePrecision(f) => Ok(json!(f)),
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Ok(JsonValue::String(s)),
        SqlValue::Json(j) | SqlValue::Jsonb(j) => Ok(j),
        SqlValue::Array(arr) => {
            let json_arr: Vec<JsonValue> = arr
                .into_iter()
                .map(sql_value_to_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(JsonValue::Array(json_arr))
        }
        SqlValue::Composite(map) => {
            let mut json_map = Map::new();
            for (key, val) in map {
                json_map.insert(key, sql_value_to_json(val)?);
            }
            Ok(JsonValue::Object(json_map))
        }
        _ => Err(ProtocolError::PostgresError(format!(
            "Cannot convert {:?} to JSON",
            value
        ))),
    }
}

/// JSON Schema validation structures
#[derive(Debug, Clone)]
pub struct JsonSchema {
    pub schema_type: JsonSchemaType,
    pub properties: Option<HashMap<String, Box<JsonSchema>>>,
    pub required: Option<Vec<String>>,
    pub items: Option<Box<JsonSchema>>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub enum_values: Option<Vec<JsonValue>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonSchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

impl JsonSchema {
    /// Validate a JSON value against this schema
    pub fn validate(&self, value: &JsonValue) -> ProtocolResult<()> {
        match (&self.schema_type, value) {
            (JsonSchemaType::String, JsonValue::String(s)) => {
                if let Some(min_len) = self.min_length {
                    if s.len() < min_len {
                        return Err(ProtocolError::PostgresError(format!(
                            "String length {} is less than minimum {}",
                            s.len(),
                            min_len
                        )));
                    }
                }
                if let Some(max_len) = self.max_length {
                    if s.len() > max_len {
                        return Err(ProtocolError::PostgresError(format!(
                            "String length {} exceeds maximum {}",
                            s.len(),
                            max_len
                        )));
                    }
                }
                Ok(())
            }
            (JsonSchemaType::Number, JsonValue::Number(n)) => {
                let num = n.as_f64().unwrap_or(0.0);
                if let Some(min) = self.minimum {
                    if num < min {
                        return Err(ProtocolError::PostgresError(format!(
                            "Number {} is less than minimum {}",
                            num, min
                        )));
                    }
                }
                if let Some(max) = self.maximum {
                    if num > max {
                        return Err(ProtocolError::PostgresError(format!(
                            "Number {} exceeds maximum {}",
                            num, max
                        )));
                    }
                }
                Ok(())
            }
            (JsonSchemaType::Integer, JsonValue::Number(n)) if n.is_i64() => Ok(()),
            (JsonSchemaType::Boolean, JsonValue::Bool(_)) => Ok(()),
            (JsonSchemaType::Null, JsonValue::Null) => Ok(()),
            (JsonSchemaType::Array, JsonValue::Array(arr)) => {
                if let Some(items_schema) = &self.items {
                    for item in arr {
                        items_schema.validate(item)?;
                    }
                }
                Ok(())
            }
            (JsonSchemaType::Object, JsonValue::Object(map)) => {
                if let Some(required) = &self.required {
                    for req_key in required {
                        if !map.contains_key(req_key) {
                            return Err(ProtocolError::PostgresError(format!(
                                "Required property '{}' is missing",
                                req_key
                            )));
                        }
                    }
                }
                if let Some(properties) = &self.properties {
                    for (key, prop_schema) in properties {
                        if let Some(value) = map.get(key) {
                            prop_schema.validate(value)?;
                        }
                    }
                }
                Ok(())
            }
            _ => Err(ProtocolError::PostgresError(format!(
                "Type mismatch: expected {:?}, got {:?}",
                self.schema_type, value
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_extract() {
        let json = json!({"name": "Alice", "age": 30});
        let result = JsonOperations::json_extract(&json, "name").unwrap();
        match result {
            SqlValue::Json(v) => assert_eq!(v, json!("Alice")),
            _ => panic!("Expected JSON value"),
        }
    }

    #[test]
    fn test_json_extract_text() {
        let json = json!({"name": "Alice", "age": 30});
        let result = JsonOperations::json_extract_text(&json, "name").unwrap();
        match result {
            SqlValue::Text(s) => assert_eq!(s, "\"Alice\""),
            _ => panic!("Expected text value"),
        }
    }

    #[test]
    fn test_json_path_navigation() {
        let json = json!({"user": {"name": "Alice", "scores": [10, 20, 30]}});
        let path = JsonPath::from_text_array(&["user".to_string(), "name".to_string()]).unwrap();
        let result = path.navigate(&json);
        assert_eq!(result, Some(&json!("Alice")));
    }

    #[test]
    fn test_json_contains() {
        let left = json!({"a": 1, "b": 2, "c": 3});
        let right = json!({"a": 1, "b": 2});
        let result = JsonOperations::json_contains(&left, &right).unwrap();
        assert_eq!(result, SqlValue::Boolean(true));

        let right2 = json!({"a": 1, "d": 4});
        let result2 = JsonOperations::json_contains(&left, &right2).unwrap();
        assert_eq!(result2, SqlValue::Boolean(false));
    }

    #[test]
    fn test_json_agg() {
        let values = vec![
            SqlValue::Integer(1),
            SqlValue::Integer(2),
            SqlValue::Integer(3),
        ];
        let result = JsonOperations::json_agg(values).unwrap();
        match result {
            SqlValue::Json(v) => assert_eq!(v, json!([1, 2, 3])),
            _ => panic!("Expected JSON array"),
        }
    }

    #[test]
    fn test_jsonb_set() {
        let json = json!({"a": {"b": 1}});
        let path = JsonPath::from_text_array(&["a".to_string(), "b".to_string()]).unwrap();
        let new_value = json!(42);
        let result = JsonOperations::jsonb_set(&json, &path, &new_value, false).unwrap();
        match result {
            SqlValue::Jsonb(v) => assert_eq!(v, json!({"a": {"b": 42}})),
            _ => panic!("Expected JSONB value"),
        }
    }

    #[test]
    fn test_json_schema_validation() {
        let schema = JsonSchema {
            schema_type: JsonSchemaType::Object,
            properties: Some(HashMap::from([
                (
                    "name".to_string(),
                    Box::new(JsonSchema {
                        schema_type: JsonSchemaType::String,
                        properties: None,
                        required: None,
                        items: None,
                        minimum: None,
                        maximum: None,
                        min_length: Some(1),
                        max_length: None,
                        pattern: None,
                        enum_values: None,
                    }),
                ),
                (
                    "age".to_string(),
                    Box::new(JsonSchema {
                        schema_type: JsonSchemaType::Integer,
                        properties: None,
                        required: None,
                        items: None,
                        minimum: Some(0.0),
                        maximum: Some(150.0),
                        min_length: None,
                        max_length: None,
                        pattern: None,
                        enum_values: None,
                    }),
                ),
            ])),
            required: Some(vec!["name".to_string()]),
            items: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
            enum_values: None,
        };

        let valid_json = json!({"name": "Alice", "age": 30});
        assert!(schema.validate(&valid_json).is_ok());

        let invalid_json = json!({"age": 30}); // Missing required "name"
        assert!(schema.validate(&invalid_json).is_err());
    }
}
