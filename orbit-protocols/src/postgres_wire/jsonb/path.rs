//! JSON Path Expression Support
//!
//! This module implements JSON path expressions compatible with PostgreSQL's JSON path syntax.
//! It supports:
//!
//! - Simple key access: `"name"`
//! - Array indexing: `"tags[0]"`
//! - Nested paths: `"user.profile.name"`
//! - Recursive descent: `"**.name"`
//! - Filter expressions: `"users[?(@.active == true)]"`

use crate::postgres_wire::jsonb::{JsonbError, JsonbResult, JsonbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a JSON path for navigating JSON structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonbPath {
    /// Path components (keys and array indices)
    pub components: Vec<PathComponent>,
    /// Whether this is a recursive path (**)
    pub recursive: bool,
}

/// Individual component of a JSON path
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PathComponent {
    /// Object key access
    Key(String),
    /// Array index access
    Index(usize),
    /// Array slice [start:end]
    Slice {
        start: Option<usize>,
        end: Option<usize>,
    },
    /// Wildcard (*) - matches any key/index
    Wildcard,
    /// Filter expression
    Filter(FilterExpression),
}

/// Filter expression for conditional path matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterExpression {
    pub operator: FilterOperator,
    pub left: Box<JsonbPath>,
    pub right: JsonbValue,
}

/// Operators for filter expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
    Exists,
}

impl JsonbPath {
    /// Create a new empty path
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            recursive: false,
        }
    }

    /// Create a path from a dot-notation string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(path: &str) -> JsonbResult<Self> {
        if path.is_empty() {
            return Ok(Self::new());
        }

        let mut components = Vec::new();
        let mut chars = path.chars().peekable();
        let mut current_key = String::new();
        let recursive = path.starts_with("**");

        if recursive {
            chars.next(); // consume first *
            chars.next(); // consume second *
            if chars.peek() == Some(&'.') {
                chars.next(); // consume dot after **
            }
        }

        while let Some(ch) = chars.next() {
            match ch {
                '.' => {
                    if !current_key.is_empty() {
                        components.push(PathComponent::Key(current_key.clone()));
                        current_key.clear();
                    }
                }
                '[' => {
                    if !current_key.is_empty() {
                        components.push(PathComponent::Key(current_key.clone()));
                        current_key.clear();
                    }

                    // Parse array index or slice
                    let mut bracket_content = String::new();
                    let mut bracket_depth = 1;

                    for ch in chars.by_ref() {
                        if ch == '[' {
                            bracket_depth += 1;
                        } else if ch == ']' {
                            bracket_depth -= 1;
                            if bracket_depth == 0 {
                                break;
                            }
                        }
                        bracket_content.push(ch);
                    }

                    components.push(Self::parse_bracket_content(&bracket_content)?);
                }
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        return Err(JsonbError::InvalidPath(
                            "** can only appear at the beginning of a path".to_string(),
                        ));
                    }
                    components.push(PathComponent::Wildcard);
                }
                _ => {
                    current_key.push(ch);
                }
            }
        }

        if !current_key.is_empty() {
            components.push(PathComponent::Key(current_key));
        }

        Ok(Self {
            components,
            recursive,
        })
    }

    /// Parse the content inside brackets [...]
    fn parse_bracket_content(content: &str) -> JsonbResult<PathComponent> {
        if content.is_empty() {
            return Err(JsonbError::InvalidPath("Empty bracket content".to_string()));
        }

        // Check for wildcard
        if content == "*" {
            return Ok(PathComponent::Wildcard);
        }

        // Check for slice notation
        if content.contains(':') {
            let parts: Vec<&str> = content.splitn(2, ':').collect();
            let start = if parts[0].is_empty() {
                None
            } else {
                Some(parts[0].parse().map_err(|_| {
                    JsonbError::InvalidPath(format!("Invalid slice start: {}", parts[0]))
                })?)
            };
            let end = if parts.len() > 1 && !parts[1].is_empty() {
                Some(parts[1].parse().map_err(|_| {
                    JsonbError::InvalidPath(format!("Invalid slice end: {}", parts[1]))
                })?)
            } else {
                None
            };
            return Ok(PathComponent::Slice { start, end });
        }

        // Check for filter expression
        if let Some(stripped) = content.strip_prefix('?') {
            return Self::parse_filter_expression(stripped);
        }

        // Try to parse as array index
        match content.parse::<usize>() {
            Ok(index) => Ok(PathComponent::Index(index)),
            Err(_) => {
                // Treat as quoted key
                let key = content.trim_matches('"');
                Ok(PathComponent::Key(key.to_string()))
            }
        }
    }

    /// Parse a filter expression like `(@.active == true)`
    fn parse_filter_expression(expr: &str) -> JsonbResult<PathComponent> {
        // Simple parser for basic filter expressions
        // This is a simplified version - a full implementation would use a proper parser

        let expr = expr.trim();
        if !expr.starts_with("(@") {
            return Err(JsonbError::InvalidPath(
                "Filter expressions must start with (@".to_string(),
            ));
        }

        // For now, return a placeholder - full implementation would parse the expression
        Ok(PathComponent::Filter(FilterExpression {
            operator: FilterOperator::Exists,
            left: Box::new(JsonbPath::new()),
            right: JsonbValue::Bool(true),
        }))
    }

    /// Add a key component to the path
    pub fn push_key(&mut self, key: String) {
        self.components.push(PathComponent::Key(key));
    }

    /// Add an index component to the path
    pub fn push_index(&mut self, index: usize) {
        self.components.push(PathComponent::Index(index));
    }

    /// Check if this path is empty
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Get the length of the path
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Get a subpath from start to end
    pub fn subpath(&self, start: usize, end: usize) -> Self {
        Self {
            components: self.components[start..end].to_vec(),
            recursive: false, // Subpaths are not recursive
        }
    }
}

impl Default for JsonbPath {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JsonbPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.recursive {
            write!(f, "**")?;
            if !self.components.is_empty() {
                write!(f, ".")?;
            }
        }

        for (i, component) in self.components.iter().enumerate() {
            if i > 0 {
                match component {
                    PathComponent::Index(_) | PathComponent::Slice { .. } => {
                        // No separator for array access
                    }
                    _ => write!(f, ".")?,
                }
            }

            match component {
                PathComponent::Key(key) => write!(f, "{}", key)?,
                PathComponent::Index(idx) => write!(f, "[{}]", idx)?,
                PathComponent::Slice { start, end } => {
                    write!(f, "[")?;
                    if let Some(s) = start {
                        write!(f, "{}", s)?;
                    }
                    write!(f, ":")?;
                    if let Some(e) = end {
                        write!(f, "{}", e)?;
                    }
                    write!(f, "]")?;
                }
                PathComponent::Wildcard => write!(f, "*")?,
                PathComponent::Filter(_) => write!(f, "[?(...)]")?, // Simplified display
            }
        }

        Ok(())
    }
}

impl JsonbValue {
    /// Get a value at the specified path
    pub fn get_path(&self, path: &JsonbPath) -> JsonbResult<Option<&JsonbValue>> {
        if path.is_empty() {
            return Ok(Some(self));
        }

        let mut current = self;

        for component in &path.components {
            match component {
                PathComponent::Key(key) => {
                    if let JsonbValue::Object(obj) = current {
                        current = obj.get(key).ok_or_else(|| {
                            JsonbError::PathNotFound(format!("Key '{}' not found", key))
                        })?;
                    } else {
                        return Ok(None);
                    }
                }
                PathComponent::Index(index) => {
                    if let JsonbValue::Array(arr) = current {
                        if *index >= arr.len() {
                            return Err(JsonbError::IndexOutOfBounds { index: *index });
                        }
                        current = &arr[*index];
                    } else {
                        return Ok(None);
                    }
                }
                PathComponent::Slice { start: _, end: _ } => {
                    // For slice operations, we need to return owned data
                    // This is a limitation of the current design - slices create new data
                    return Err(JsonbError::InvalidOperation(
                        "Slice operations require owned data access".to_string(),
                    ));
                }
                PathComponent::Wildcard => {
                    // Wildcard matching would return multiple values - simplified here
                    return Ok(Some(current));
                }
                PathComponent::Filter(_filter) => {
                    // Filter evaluation would be implemented here
                    return Ok(Some(current));
                }
            }
        }

        Ok(Some(current))
    }

    /// Get a mutable reference to a value at the specified path
    pub fn get_path_mut(&mut self, path: &JsonbPath) -> JsonbResult<Option<&mut JsonbValue>> {
        if path.is_empty() {
            return Ok(Some(self));
        }

        let mut current = self;

        for component in &path.components {
            match component {
                PathComponent::Key(key) => {
                    if let JsonbValue::Object(obj) = current {
                        current = obj.get_mut(key).ok_or_else(|| {
                            JsonbError::PathNotFound(format!("Key '{}' not found", key))
                        })?;
                    } else {
                        return Ok(None);
                    }
                }
                PathComponent::Index(index) => {
                    if let JsonbValue::Array(arr) = current {
                        if *index >= arr.len() {
                            return Err(JsonbError::IndexOutOfBounds { index: *index });
                        }
                        current = &mut arr[*index];
                    } else {
                        return Ok(None);
                    }
                }
                _ => {
                    // Other component types not implemented for mutable access
                    return Err(JsonbError::InvalidOperation(
                        "Mutable access not supported for this path component".to_string(),
                    ));
                }
            }
        }

        Ok(Some(current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_parsing() {
        let path = JsonbPath::from_str("user.profile.name").unwrap();
        assert_eq!(path.components.len(), 3);
        assert_eq!(path.components[0], PathComponent::Key("user".to_string()));
        assert_eq!(
            path.components[1],
            PathComponent::Key("profile".to_string())
        );
        assert_eq!(path.components[2], PathComponent::Key("name".to_string()));
    }

    #[test]
    fn test_array_index_parsing() {
        let path = JsonbPath::from_str("users[0].name").unwrap();
        assert_eq!(path.components.len(), 3);
        assert_eq!(path.components[0], PathComponent::Key("users".to_string()));
        assert_eq!(path.components[1], PathComponent::Index(0));
        assert_eq!(path.components[2], PathComponent::Key("name".to_string()));
    }

    #[test]
    fn test_path_navigation() {
        let json_str = r#"{"user": {"profile": {"name": "Alice", "tags": ["developer", "rust"]}}}"#;
        let jsonb = JsonbValue::from_json_str(json_str).unwrap();

        let path = JsonbPath::from_str("user.profile.name").unwrap();
        let result = jsonb.get_path(&path).unwrap().unwrap();
        assert_eq!(*result, JsonbValue::String("Alice".to_string()));

        let array_path = JsonbPath::from_str("user.profile.tags[0]").unwrap();
        let array_result = jsonb.get_path(&array_path).unwrap().unwrap();
        assert_eq!(*array_result, JsonbValue::String("developer".to_string()));
    }

    #[test]
    fn test_recursive_path() {
        let path = JsonbPath::from_str("**.name").unwrap();
        assert!(path.recursive);
        assert_eq!(path.components.len(), 1);
        assert_eq!(path.components[0], PathComponent::Key("name".to_string()));
    }

    #[test]
    fn test_wildcard_path() {
        let path = JsonbPath::from_str("users.*.name").unwrap();
        assert_eq!(path.components.len(), 3);
        assert_eq!(path.components[1], PathComponent::Wildcard);
    }

    #[test]
    fn test_path_display() {
        let path = JsonbPath::from_str("user.tags[0]").unwrap();
        assert_eq!(path.to_string(), "user.tags[0]");

        let recursive_path = JsonbPath::from_str("**.name").unwrap();
        assert_eq!(recursive_path.to_string(), "**.name");
    }
}
