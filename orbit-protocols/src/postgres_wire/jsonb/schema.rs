//! JSON Schema Validation and Constraints
//!
//! This module provides JSON schema validation capabilities compatible with
//! JSON Schema Draft 7 and PostgreSQL's CHECK constraints on JSON columns.

use crate::postgres_wire::jsonb::JsonbValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON Schema for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    /// Schema type
    pub schema_type: Option<SchemaType>,
    /// Properties for objects  
    pub properties: Option<HashMap<String, JsonSchema>>,
    /// Required properties
    pub required: Option<Vec<String>>,
    /// Additional properties allowed
    pub additional_properties: Option<bool>,
    /// Items schema for arrays
    pub items: Option<Box<JsonSchema>>,
    /// Minimum value
    pub minimum: Option<f64>,
    /// Maximum value
    pub maximum: Option<f64>,
    /// Minimum length
    pub min_length: Option<usize>,
    /// Maximum length
    pub max_length: Option<usize>,
    /// Pattern for string validation
    pub pattern: Option<String>,
    /// Enum values
    pub enum_values: Option<Vec<JsonbValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaType {
    Null,
    Boolean,
    Integer,
    Number,
    String,
    Array,
    Object,
}

/// Schema validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl JsonSchema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self {
            schema_type: None,
            properties: None,
            required: None,
            additional_properties: None,
            items: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
            enum_values: None,
        }
    }

    /// Validate a JSON value against this schema
    pub fn validate(&self, value: &JsonbValue) -> ValidationResult {
        let mut errors = Vec::new();
        self.validate_recursive(value, "", &mut errors);

        ValidationResult {
            valid: errors.is_empty(),
            errors,
        }
    }

    fn validate_recursive(
        &self,
        value: &JsonbValue,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Type validation
        if let Some(expected_type) = &self.schema_type {
            if !self.type_matches(value, expected_type) {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "Expected type {:?}, got {:?}",
                        expected_type,
                        value.json_type()
                    ),
                });
                return; // Don't continue validation if type doesn't match
            }
        }

        // Type-specific validation
        match value {
            JsonbValue::String(s) => self.validate_string(s, path, errors),
            JsonbValue::Number(n) => self.validate_number(*n, path, errors),
            JsonbValue::Array(arr) => self.validate_array(arr, path, errors),
            JsonbValue::Object(obj) => self.validate_object(obj, path, errors),
            _ => {}
        }

        // Enum validation
        if let Some(enum_values) = &self.enum_values {
            if !enum_values.contains(value) {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Value is not in allowed enum values".to_string(),
                });
            }
        }
    }

    fn type_matches(&self, value: &JsonbValue, expected: &SchemaType) -> bool {
        match (value, expected) {
            (JsonbValue::Null, SchemaType::Null) => true,
            (JsonbValue::Bool(_), SchemaType::Boolean) => true,
            (JsonbValue::Number(n), SchemaType::Integer) => n.fract() == 0.0,
            (JsonbValue::Number(_), SchemaType::Number) => true,
            (JsonbValue::String(_), SchemaType::String) => true,
            (JsonbValue::Array(_), SchemaType::Array) => true,
            (JsonbValue::Object(_), SchemaType::Object) => true,
            _ => false,
        }
    }

    fn validate_string(&self, s: &str, path: &str, errors: &mut Vec<ValidationError>) {
        if let Some(min_len) = self.min_length {
            if s.len() < min_len {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("String length {} is less than minimum {}", s.len(), min_len),
                });
            }
        }

        if let Some(max_len) = self.max_length {
            if s.len() > max_len {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("String length {} exceeds maximum {}", s.len(), max_len),
                });
            }
        }

        if let Some(pattern) = &self.pattern {
            // Simplified pattern matching - in a real implementation, use regex
            if !s.contains(pattern) {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("String does not match pattern: {}", pattern),
                });
            }
        }
    }

    fn validate_number(&self, n: f64, path: &str, errors: &mut Vec<ValidationError>) {
        if let Some(min) = self.minimum {
            if n < min {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Number {} is less than minimum {}", n, min),
                });
            }
        }

        if let Some(max) = self.maximum {
            if n > max {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Number {} exceeds maximum {}", n, max),
                });
            }
        }
    }

    fn validate_array(&self, arr: &[JsonbValue], path: &str, errors: &mut Vec<ValidationError>) {
        if let Some(items_schema) = &self.items {
            for (i, item) in arr.iter().enumerate() {
                let item_path = if path.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", path, i)
                };
                items_schema.validate_recursive(item, &item_path, errors);
            }
        }
    }

    fn validate_object(
        &self,
        obj: &HashMap<String, JsonbValue>,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Check required properties
        if let Some(required) = &self.required {
            for req_prop in required {
                if !obj.contains_key(req_prop) {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!("Required property '{}' is missing", req_prop),
                    });
                }
            }
        }

        // Validate properties
        if let Some(properties) = &self.properties {
            for (key, value) in obj {
                let prop_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                if let Some(prop_schema) = properties.get(key) {
                    prop_schema.validate_recursive(value, &prop_path, errors);
                } else if Some(false) == self.additional_properties {
                    errors.push(ValidationError {
                        path: prop_path,
                        message: "Additional property not allowed".to_string(),
                    });
                }
            }
        }
    }
}

impl Default for JsonSchema {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema builder for easier schema construction
pub struct JsonSchemaBuilder {
    schema: JsonSchema,
}

impl JsonSchemaBuilder {
    pub fn new() -> Self {
        Self {
            schema: JsonSchema::new(),
        }
    }

    pub fn schema_type(mut self, schema_type: SchemaType) -> Self {
        self.schema.schema_type = Some(schema_type);
        self
    }

    pub fn required(mut self, required: Vec<String>) -> Self {
        self.schema.required = Some(required);
        self
    }

    pub fn property(mut self, name: String, property_schema: JsonSchema) -> Self {
        if self.schema.properties.is_none() {
            self.schema.properties = Some(HashMap::new());
        }
        self.schema
            .properties
            .as_mut()
            .unwrap()
            .insert(name, property_schema);
        self
    }

    pub fn additional_properties(mut self, allowed: bool) -> Self {
        self.schema.additional_properties = Some(allowed);
        self
    }

    pub fn minimum(mut self, min: f64) -> Self {
        self.schema.minimum = Some(min);
        self
    }

    pub fn maximum(mut self, max: f64) -> Self {
        self.schema.maximum = Some(max);
        self
    }

    pub fn min_length(mut self, min_len: usize) -> Self {
        self.schema.min_length = Some(min_len);
        self
    }

    pub fn max_length(mut self, max_len: usize) -> Self {
        self.schema.max_length = Some(max_len);
        self
    }

    pub fn build(self) -> JsonSchema {
        self.schema
    }
}

impl Default for JsonSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_type_validation() {
        let schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::String)
            .build();

        let valid_value = JsonbValue::String("test".to_string());
        let invalid_value = JsonbValue::Number(42.0);

        let result1 = schema.validate(&valid_value);
        assert!(result1.valid);

        let result2 = schema.validate(&invalid_value);
        assert!(!result2.valid);
        assert_eq!(result2.errors.len(), 1);
    }

    #[test]
    fn test_string_length_validation() {
        let schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::String)
            .min_length(3)
            .max_length(10)
            .build();

        let valid_value = JsonbValue::String("hello".to_string());
        let too_short = JsonbValue::String("hi".to_string());
        let too_long = JsonbValue::String("this is way too long".to_string());

        assert!(schema.validate(&valid_value).valid);
        assert!(!schema.validate(&too_short).valid);
        assert!(!schema.validate(&too_long).valid);
    }

    #[test]
    fn test_number_range_validation() {
        let schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::Number)
            .minimum(0.0)
            .maximum(100.0)
            .build();

        let valid_value = JsonbValue::Number(50.0);
        let too_small = JsonbValue::Number(-10.0);
        let too_large = JsonbValue::Number(150.0);

        assert!(schema.validate(&valid_value).valid);
        assert!(!schema.validate(&too_small).valid);
        assert!(!schema.validate(&too_large).valid);
    }

    #[test]
    fn test_object_validation() {
        let name_schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::String)
            .min_length(1)
            .build();

        let age_schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::Integer)
            .minimum(0.0)
            .maximum(150.0)
            .build();

        let schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::Object)
            .required(vec!["name".to_string()])
            .property("name".to_string(), name_schema)
            .property("age".to_string(), age_schema)
            .additional_properties(false)
            .build();

        let valid_obj = JsonbValue::Object(
            [
                ("name".to_string(), JsonbValue::String("Alice".to_string())),
                ("age".to_string(), JsonbValue::Number(30.0)),
            ]
            .iter()
            .cloned()
            .collect(),
        );

        let missing_required = JsonbValue::Object(
            [("age".to_string(), JsonbValue::Number(30.0))]
                .iter()
                .cloned()
                .collect(),
        );

        let extra_property = JsonbValue::Object(
            [
                ("name".to_string(), JsonbValue::String("Alice".to_string())),
                (
                    "extra".to_string(),
                    JsonbValue::String("not allowed".to_string()),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        );

        assert!(schema.validate(&valid_obj).valid);
        assert!(!schema.validate(&missing_required).valid);
        assert!(!schema.validate(&extra_property).valid);
    }

    #[test]
    fn test_array_validation() {
        let item_schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::String)
            .build();

        let schema = JsonSchemaBuilder::new()
            .schema_type(SchemaType::Array)
            .build();

        // Set items manually since we don't have a builder method for it
        let mut schema = schema;
        schema.items = Some(Box::new(item_schema));

        let valid_array = JsonbValue::Array(vec![
            JsonbValue::String("item1".to_string()),
            JsonbValue::String("item2".to_string()),
        ]);

        let invalid_array = JsonbValue::Array(vec![
            JsonbValue::String("item1".to_string()),
            JsonbValue::Number(42.0), // Wrong type
        ]);

        assert!(schema.validate(&valid_array).valid);
        assert!(!schema.validate(&invalid_array).valid);
    }
}
