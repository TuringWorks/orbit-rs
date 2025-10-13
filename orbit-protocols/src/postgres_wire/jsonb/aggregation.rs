//! JSON Aggregation Functions
//!
//! This module implements PostgreSQL-compatible JSON aggregation functions:
//!
//! - `json_agg()` - Aggregates values into a JSON array
//! - `jsonb_agg()` - Aggregates values into a JSONB array  
//! - `json_object_agg()` - Aggregates key-value pairs into a JSON object
//! - `jsonb_object_agg()` - Aggregates key-value pairs into a JSONB object
//! - `json_arrayagg()` - SQL standard array aggregation function
//! - `json_objectagg()` - SQL standard object aggregation function

use crate::postgres_wire::jsonb::{JsonbError, JsonbResult, JsonbValue};
use std::collections::HashMap;

/// JSON aggregation context for collecting values during aggregation
#[derive(Debug, Clone)]
pub struct JsonAggregator {
    values: Vec<JsonbValue>,
    #[allow(dead_code)]
    preserve_order: bool,
    ignore_nulls: bool,
}

impl JsonAggregator {
    /// Create a new JSON aggregator
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            preserve_order: true,
            ignore_nulls: false,
        }
    }

    /// Create a new JSON aggregator with options
    pub fn with_options(preserve_order: bool, ignore_nulls: bool) -> Self {
        Self {
            values: Vec::new(),
            preserve_order,
            ignore_nulls,
        }
    }

    /// Add a value to the aggregation
    pub fn add_value(&mut self, value: JsonbValue) {
        if self.ignore_nulls && value == JsonbValue::Null {
            return;
        }
        self.values.push(value);
    }

    /// Finalize the aggregation as a JSON array
    pub fn finalize_array(&self) -> JsonbValue {
        JsonbValue::Array(self.values.clone())
    }

    /// Get the number of aggregated values
    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// Check if the aggregator is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Clear all accumulated values
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl Default for JsonAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON object aggregation context for collecting key-value pairs
#[derive(Debug, Clone)]
pub struct JsonObjectAggregator {
    pairs: HashMap<String, JsonbValue>,
    preserve_order: bool,
    ignore_nulls: bool,
    ordered_keys: Vec<String>, // For order preservation
}

impl JsonObjectAggregator {
    /// Create a new JSON object aggregator
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
            preserve_order: true,
            ignore_nulls: false,
            ordered_keys: Vec::new(),
        }
    }

    /// Create a new JSON object aggregator with options
    pub fn with_options(preserve_order: bool, ignore_nulls: bool) -> Self {
        Self {
            pairs: HashMap::new(),
            preserve_order,
            ignore_nulls,
            ordered_keys: Vec::new(),
        }
    }

    /// Add a key-value pair to the aggregation
    pub fn add_pair(&mut self, key: String, value: JsonbValue) -> JsonbResult<()> {
        if self.ignore_nulls && value == JsonbValue::Null {
            return Ok(());
        }

        if key.is_empty() {
            return Err(JsonbError::InvalidOperation(
                "Object aggregation key cannot be empty".to_string(),
            ));
        }

        // If we're preserving order and this is a new key, add it to ordered_keys
        if self.preserve_order && !self.pairs.contains_key(&key) {
            self.ordered_keys.push(key.clone());
        }

        self.pairs.insert(key, value);
        Ok(())
    }

    /// Finalize the aggregation as a JSON object
    pub fn finalize_object(&self) -> JsonbValue {
        if self.preserve_order {
            // Create ordered object using ordered_keys
            let mut ordered_map = HashMap::new();
            for key in &self.ordered_keys {
                if let Some(value) = self.pairs.get(key) {
                    ordered_map.insert(key.clone(), value.clone());
                }
            }
            // Add any keys not in ordered_keys (shouldn't happen normally)
            for (key, value) in &self.pairs {
                if !ordered_map.contains_key(key) {
                    ordered_map.insert(key.clone(), value.clone());
                }
            }
            JsonbValue::Object(ordered_map)
        } else {
            JsonbValue::Object(self.pairs.clone())
        }
    }

    /// Get the number of aggregated pairs
    pub fn count(&self) -> usize {
        self.pairs.len()
    }

    /// Check if the aggregator is empty
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Clear all accumulated pairs
    pub fn clear(&mut self) {
        self.pairs.clear();
        self.ordered_keys.clear();
    }
}

impl Default for JsonObjectAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregation function implementations
pub struct JsonAggregationFunctions;

impl JsonAggregationFunctions {
    /// `json_agg(expression)` - Aggregates values into a JSON array
    ///
    /// # Examples
    /// ```sql
    /// SELECT json_agg(name) FROM users;
    /// -- Result: ["Alice", "Bob", "Charlie"]
    /// ```
    pub fn json_agg(values: Vec<JsonbValue>) -> JsonbValue {
        let mut aggregator = JsonAggregator::new();
        for value in values {
            aggregator.add_value(value);
        }
        aggregator.finalize_array()
    }

    /// `jsonb_agg(expression)` - Aggregates values into a JSONB array
    ///
    /// Same as json_agg but returns JSONB (binary) format
    pub fn jsonb_agg(values: Vec<JsonbValue>) -> JsonbValue {
        Self::json_agg(values) // Same implementation, format handled at storage level
    }

    /// `json_object_agg(key_expression, value_expression)` - Aggregates key-value pairs into a JSON object
    ///
    /// # Examples
    /// ```sql
    /// SELECT json_object_agg(name, age) FROM users;
    /// -- Result: {"Alice": 30, "Bob": 25, "Charlie": 35}
    /// ```
    pub fn json_object_agg(
        keys: Vec<JsonbValue>,
        values: Vec<JsonbValue>,
    ) -> JsonbResult<JsonbValue> {
        if keys.len() != values.len() {
            return Err(JsonbError::InvalidOperation(
                "Key and value arrays must have the same length".to_string(),
            ));
        }

        let mut aggregator = JsonObjectAggregator::new();
        for (key_value, value) in keys.into_iter().zip(values) {
            let key_str = match key_value {
                JsonbValue::String(s) => s,
                JsonbValue::Null => continue, // Skip null keys
                other => other.to_text(),     // Convert other types to string
            };
            aggregator.add_pair(key_str, value)?;
        }

        Ok(aggregator.finalize_object())
    }

    /// `jsonb_object_agg(key_expression, value_expression)` - Aggregates key-value pairs into a JSONB object
    pub fn jsonb_object_agg(
        keys: Vec<JsonbValue>,
        values: Vec<JsonbValue>,
    ) -> JsonbResult<JsonbValue> {
        Self::json_object_agg(keys, values) // Same implementation
    }

    /// `json_arrayagg(expression ORDER BY ...)` - SQL standard array aggregation
    ///
    /// This is equivalent to json_agg but follows SQL standard naming
    pub fn json_arrayagg(values: Vec<JsonbValue>) -> JsonbValue {
        Self::json_agg(values)
    }

    /// `json_objectagg(key_expression, value_expression)` - SQL standard object aggregation
    pub fn json_objectagg(
        keys: Vec<JsonbValue>,
        values: Vec<JsonbValue>,
    ) -> JsonbResult<JsonbValue> {
        Self::json_object_agg(keys, values)
    }

    /// Advanced aggregation with filtering and ordering
    pub fn json_agg_filtered<F>(
        values: Vec<JsonbValue>,
        filter: F,
        ignore_nulls: bool,
    ) -> JsonbValue
    where
        F: Fn(&JsonbValue) -> bool,
    {
        let mut aggregator = JsonAggregator::with_options(true, ignore_nulls);
        for value in values {
            if filter(&value) {
                aggregator.add_value(value);
            }
        }
        aggregator.finalize_array()
    }

    /// Aggregate with custom transformation
    pub fn json_agg_transform<F>(values: Vec<JsonbValue>, transform: F) -> JsonbValue
    where
        F: Fn(JsonbValue) -> JsonbValue,
    {
        let mut aggregator = JsonAggregator::new();
        for value in values {
            let transformed = transform(value);
            aggregator.add_value(transformed);
        }
        aggregator.finalize_array()
    }

    /// Conditional aggregation - only aggregate values that meet a condition
    pub fn json_agg_conditional(
        values: Vec<JsonbValue>,
        conditions: Vec<bool>,
    ) -> JsonbResult<JsonbValue> {
        if values.len() != conditions.len() {
            return Err(JsonbError::InvalidOperation(
                "Values and conditions arrays must have the same length".to_string(),
            ));
        }

        let mut aggregator = JsonAggregator::new();
        for (value, condition) in values.into_iter().zip(conditions) {
            if condition {
                aggregator.add_value(value);
            }
        }

        Ok(aggregator.finalize_array())
    }

    /// Group aggregation - aggregate values by groups
    pub fn json_agg_grouped(
        values: Vec<JsonbValue>,
        groups: Vec<String>,
    ) -> JsonbResult<HashMap<String, JsonbValue>> {
        if values.len() != groups.len() {
            return Err(JsonbError::InvalidOperation(
                "Values and groups arrays must have the same length".to_string(),
            ));
        }

        let mut group_aggregators: HashMap<String, JsonAggregator> = HashMap::new();

        for (value, group) in values.into_iter().zip(groups) {
            let aggregator = group_aggregators.entry(group).or_default();
            aggregator.add_value(value);
        }

        let result = group_aggregators
            .into_iter()
            .map(|(group, aggregator)| (group, aggregator.finalize_array()))
            .collect();

        Ok(result)
    }

    /// Nested aggregation - aggregate nested structures
    pub fn json_agg_nested(values: Vec<Vec<JsonbValue>>) -> JsonbValue {
        let mut aggregator = JsonAggregator::new();
        for nested_values in values {
            let nested_array = Self::json_agg(nested_values);
            aggregator.add_value(nested_array);
        }
        aggregator.finalize_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_agg() {
        let values = vec![
            JsonbValue::String("Alice".to_string()),
            JsonbValue::String("Bob".to_string()),
            JsonbValue::Number(30.0),
            JsonbValue::Bool(true),
        ];

        let result = JsonAggregationFunctions::json_agg(values);

        if let JsonbValue::Array(arr) = result {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[0], JsonbValue::String("Alice".to_string()));
            assert_eq!(arr[1], JsonbValue::String("Bob".to_string()));
            assert_eq!(arr[2], JsonbValue::Number(30.0));
            assert_eq!(arr[3], JsonbValue::Bool(true));
        } else {
            panic!("Expected array result from json_agg");
        }
    }

    #[test]
    fn test_json_object_agg() {
        let keys = vec![
            JsonbValue::String("name".to_string()),
            JsonbValue::String("age".to_string()),
            JsonbValue::String("active".to_string()),
        ];
        let values = vec![
            JsonbValue::String("Alice".to_string()),
            JsonbValue::Number(30.0),
            JsonbValue::Bool(true),
        ];

        let result = JsonAggregationFunctions::json_object_agg(keys, values).unwrap();

        if let JsonbValue::Object(obj) = result {
            assert_eq!(
                obj.get("name").unwrap(),
                &JsonbValue::String("Alice".to_string())
            );
            assert_eq!(obj.get("age").unwrap(), &JsonbValue::Number(30.0));
            assert_eq!(obj.get("active").unwrap(), &JsonbValue::Bool(true));
        } else {
            panic!("Expected object result from json_object_agg");
        }
    }

    #[test]
    fn test_json_aggregator() {
        let mut aggregator = JsonAggregator::new();

        aggregator.add_value(JsonbValue::String("test1".to_string()));
        aggregator.add_value(JsonbValue::String("test2".to_string()));
        aggregator.add_value(JsonbValue::Number(42.0));

        assert_eq!(aggregator.count(), 3);
        assert!(!aggregator.is_empty());

        let result = aggregator.finalize_array();
        if let JsonbValue::Array(arr) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_json_object_aggregator() {
        let mut aggregator = JsonObjectAggregator::new();

        aggregator
            .add_pair("name".to_string(), JsonbValue::String("Alice".to_string()))
            .unwrap();
        aggregator
            .add_pair("age".to_string(), JsonbValue::Number(30.0))
            .unwrap();

        assert_eq!(aggregator.count(), 2);
        assert!(!aggregator.is_empty());

        let result = aggregator.finalize_object();
        if let JsonbValue::Object(obj) = result {
            assert_eq!(obj.len(), 2);
            assert!(obj.contains_key("name"));
            assert!(obj.contains_key("age"));
        } else {
            panic!("Expected object result");
        }
    }

    #[test]
    fn test_json_agg_filtered() {
        let values = vec![
            JsonbValue::Number(1.0),
            JsonbValue::Number(2.0),
            JsonbValue::Number(3.0),
            JsonbValue::Number(4.0),
            JsonbValue::Number(5.0),
        ];

        // Filter to only even numbers
        let result = JsonAggregationFunctions::json_agg_filtered(
            values,
            |v| {
                if let JsonbValue::Number(n) = v {
                    (*n as i32) % 2 == 0
                } else {
                    false
                }
            },
            false,
        );

        if let JsonbValue::Array(arr) = result {
            assert_eq!(arr.len(), 2); // 2 and 4
            assert_eq!(arr[0], JsonbValue::Number(2.0));
            assert_eq!(arr[1], JsonbValue::Number(4.0));
        } else {
            panic!("Expected array result from filtered aggregation");
        }
    }

    #[test]
    fn test_json_agg_conditional() {
        let values = vec![
            JsonbValue::String("Alice".to_string()),
            JsonbValue::String("Bob".to_string()),
            JsonbValue::String("Charlie".to_string()),
        ];
        let conditions = vec![true, false, true];

        let result = JsonAggregationFunctions::json_agg_conditional(values, conditions).unwrap();

        if let JsonbValue::Array(arr) = result {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], JsonbValue::String("Alice".to_string()));
            assert_eq!(arr[1], JsonbValue::String("Charlie".to_string()));
        } else {
            panic!("Expected array result from conditional aggregation");
        }
    }

    #[test]
    fn test_json_agg_grouped() {
        let values = vec![
            JsonbValue::String("Alice".to_string()),
            JsonbValue::String("Bob".to_string()),
            JsonbValue::String("Charlie".to_string()),
            JsonbValue::String("David".to_string()),
        ];
        let groups = vec![
            "group1".to_string(),
            "group1".to_string(),
            "group2".to_string(),
            "group2".to_string(),
        ];

        let result = JsonAggregationFunctions::json_agg_grouped(values, groups).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("group1"));
        assert!(result.contains_key("group2"));

        if let Some(JsonbValue::Array(group1_arr)) = result.get("group1") {
            assert_eq!(group1_arr.len(), 2);
        } else {
            panic!("Expected array for group1");
        }

        if let Some(JsonbValue::Array(group2_arr)) = result.get("group2") {
            assert_eq!(group2_arr.len(), 2);
        } else {
            panic!("Expected array for group2");
        }
    }

    #[test]
    fn test_aggregator_with_null_handling() {
        let mut aggregator = JsonAggregator::with_options(true, true); // ignore nulls

        aggregator.add_value(JsonbValue::String("test".to_string()));
        aggregator.add_value(JsonbValue::Null);
        aggregator.add_value(JsonbValue::Number(42.0));

        // Should only have 2 values (null ignored)
        assert_eq!(aggregator.count(), 2);

        let result = aggregator.finalize_array();
        if let JsonbValue::Array(arr) = result {
            assert_eq!(arr.len(), 2);
            assert_ne!(arr[0], JsonbValue::Null);
            assert_ne!(arr[1], JsonbValue::Null);
        } else {
            panic!("Expected array result");
        }
    }
}
