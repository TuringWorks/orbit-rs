//! PostgreSQL-compatible JSON operators
//!
//! This module implements all PostgreSQL JSON/JSONB operators:
//!
//! - `->` : Get JSON object field by key or array element by index (returns JSON)
//! - `->>` : Get JSON object field by key or array element by index (returns text)
//! - `#>` : Get JSON object at specified path (returns JSON)
//! - `#>>` : Get JSON object at specified path (returns text)
//! - `@>` : Does the left JSON value contain the right JSON path/value at the top level?
//! - `<@` : Is the left JSON value contained at the top level within the right JSON value?
//! - `?` : Does the string exist as a top-level key within the JSON value?
//! - `?|` : Do any of these array strings exist as top-level keys?
//! - `?&` : Do all of these array strings exist as top-level keys?
//! - `||` : Concatenate two JSON values
//! - `-` : Delete key/value pair or string element from left operand
//! - `#-` : Delete the field or array element at the specified path

use crate::postgres_wire::jsonb::{JsonbError, JsonbPath, JsonbResult, JsonbValue, PathComponent};

impl JsonbValue {
    /// `->` operator: Get JSON object field by key or array element by index (returns JSON)
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "tags": ["dev", "rust"]}"#)?;
    /// assert_eq!(json.arrow_json(&["name"])?, Some(JsonbValue::String("Alice".into())));
    /// assert_eq!(json.arrow_json(&["tags", "0"])?, Some(JsonbValue::String("dev".into())));
    /// # Ok(())
    /// # }
    /// ```
    pub fn arrow_json(&self, path: &[&str]) -> JsonbResult<Option<JsonbValue>> {
        if path.is_empty() {
            return Ok(Some(self.clone()));
        }

        let mut current = self.clone();

        for &key in path {
            match current {
                JsonbValue::Object(obj) => {
                    current = obj.get(key).cloned().unwrap_or(JsonbValue::Null);
                }
                JsonbValue::Array(arr) => {
                    if let Ok(index) = key.parse::<usize>() {
                        current = arr.get(index).cloned().unwrap_or(JsonbValue::Null);
                    } else {
                        return Ok(Some(JsonbValue::Null));
                    }
                }
                _ => return Ok(Some(JsonbValue::Null)),
            }
        }

        if current == JsonbValue::Null {
            Ok(None)
        } else {
            Ok(Some(current))
        }
    }

    /// `->>` operator: Get JSON object field by key or array element by index (returns text)
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#)?;
    /// assert_eq!(json.arrow_text(&["name"])?, Some("Alice".to_string()));
    /// assert_eq!(json.arrow_text(&["age"])?, Some("30".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn arrow_text(&self, path: &[&str]) -> JsonbResult<Option<String>> {
        match self.arrow_json(path)? {
            Some(value) => Ok(Some(value.to_text())),
            None => Ok(None),
        }
    }

    /// `#>` operator: Get JSON object at specified path (returns JSON)
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::{JsonbValue, JsonbPath};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"user": {"profile": {"name": "Alice"}}}"#)?;
    /// let path = JsonbPath::from_str("user.profile.name")?;
    /// assert_eq!(json.path_json(&path)?, Some(JsonbValue::String("Alice".into())));
    /// # Ok(())
    /// # }
    /// ```
    pub fn path_json(&self, path: &JsonbPath) -> JsonbResult<Option<JsonbValue>> {
        match self.get_path(path)? {
            Some(value) => Ok(Some(value.clone())),
            None => Ok(None),
        }
    }

    /// `#>>` operator: Get JSON object at specified path (returns text)
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::{JsonbValue, JsonbPath};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"user": {"age": 30}}"#)?;
    /// let path = JsonbPath::from_str("user.age")?;
    /// assert_eq!(json.path_text(&path)?, Some("30".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn path_text(&self, path: &JsonbPath) -> JsonbResult<Option<String>> {
        match self.path_json(path)? {
            Some(value) => Ok(Some(value.to_text())),
            None => Ok(None),
        }
    }

    /// `@>` operator: Does the left JSON value contain the right JSON path/value?
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json1 = JsonbValue::from_json_str(r#"{"name": "Alice", "tags": ["dev", "rust"]}"#)?;
    /// let json2 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#)?;
    /// assert!(json1.contains(&json2)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn contains(&self, other: &JsonbValue) -> JsonbResult<bool> {
        match (self, other) {
            // Handle null containment
            (JsonbValue::Null, JsonbValue::Null) => Ok(true),
            (_, JsonbValue::Null) | (JsonbValue::Null, _) => Ok(false),

            // Handle primitive type containment
            (JsonbValue::Bool(a), JsonbValue::Bool(b)) => Ok(a == b),
            (JsonbValue::Number(a), JsonbValue::Number(b)) => Ok(self.numbers_equal(*a, *b)),
            (JsonbValue::String(a), JsonbValue::String(b)) => Ok(a == b),

            // Handle complex type containment
            (JsonbValue::Array(self_arr), JsonbValue::Array(other_arr)) => {
                self.array_contains_array(self_arr, other_arr)
            }
            (JsonbValue::Object(self_obj), JsonbValue::Object(other_obj)) => {
                self.object_contains_object(self_obj, other_obj)
            }
            (JsonbValue::Array(arr), value) => self.array_contains_value(arr, value),

            // Other combinations are false
            _ => Ok(false),
        }
    }

    /// Check if two numbers are equal within floating point precision
    fn numbers_equal(&self, a: f64, b: f64) -> bool {
        (a - b).abs() < f64::EPSILON
    }

    /// Check if an array contains all elements of another array
    fn array_contains_array(
        &self,
        self_arr: &[JsonbValue],
        other_arr: &[JsonbValue],
    ) -> JsonbResult<bool> {
        for other_item in other_arr {
            if !self.array_contains_element(self_arr, other_item)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check if an array contains a specific element
    fn array_contains_element(
        &self,
        arr: &[JsonbValue],
        element: &JsonbValue,
    ) -> JsonbResult<bool> {
        for item in arr {
            if item.contains(element)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if an object contains all key-value pairs of another object
    fn object_contains_object(
        &self,
        self_obj: &std::collections::HashMap<String, JsonbValue>,
        other_obj: &std::collections::HashMap<String, JsonbValue>,
    ) -> JsonbResult<bool> {
        for (key, other_value) in other_obj {
            match self_obj.get(key) {
                Some(self_value) => {
                    if !self_value.contains(other_value)? {
                        return Ok(false);
                    }
                }
                None => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Check if an array contains a specific value
    fn array_contains_value(&self, arr: &[JsonbValue], value: &JsonbValue) -> JsonbResult<bool> {
        self.array_contains_element(arr, value)
    }

    /// `<@` operator: Is the left JSON value contained within the right JSON value?
    pub fn contained_by(&self, other: &JsonbValue) -> JsonbResult<bool> {
        other.contains(self)
    }

    /// `?` operator: Does the string exist as a top-level key within the JSON value?
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#)?;
    /// assert!(json.has_key("name")?);
    /// assert!(!json.has_key("email")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_key(&self, key: &str) -> JsonbResult<bool> {
        match self {
            JsonbValue::Object(obj) => Ok(obj.contains_key(key)),
            _ => Ok(false),
        }
    }

    /// `?|` operator: Do any of these array strings exist as top-level keys?
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#)?;
    /// assert!(json.has_any_key(&["name", "email"])?);
    /// assert!(!json.has_any_key(&["email", "phone"])?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_any_key(&self, keys: &[&str]) -> JsonbResult<bool> {
        for key in keys {
            if self.has_key(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `?&` operator: Do all of these array strings exist as top-level keys?
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#)?;
    /// assert!(json.has_all_keys(&["name", "age"])?);
    /// assert!(!json.has_all_keys(&["name", "age", "email"])?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_all_keys(&self, keys: &[&str]) -> JsonbResult<bool> {
        for key in keys {
            if !self.has_key(key)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `||` operator: Concatenate two JSON values
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json1 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#)?;
    /// let json2 = JsonbValue::from_json_str(r#"{"age": 30}"#)?;
    /// let result = json1.concat(&json2)?;
    /// // result: {"name": "Alice", "age": 30}
    /// # Ok(())
    /// # }
    /// ```
    pub fn concat(&self, other: &JsonbValue) -> JsonbResult<JsonbValue> {
        match (self, other) {
            // Object concatenation: merge objects, with `other` values taking precedence
            (JsonbValue::Object(self_obj), JsonbValue::Object(other_obj)) => {
                let mut result = self_obj.clone();
                for (key, value) in other_obj {
                    result.insert(key.clone(), value.clone());
                }
                Ok(JsonbValue::Object(result))
            }

            // Array concatenation: concatenate arrays
            (JsonbValue::Array(self_arr), JsonbValue::Array(other_arr)) => {
                let mut result = self_arr.clone();
                result.extend(other_arr.clone());
                Ok(JsonbValue::Array(result))
            }

            // Object + Array: treat array as values to add to object
            (JsonbValue::Object(obj), JsonbValue::Array(arr)) => {
                let mut result = obj.clone();
                for (i, value) in arr.iter().enumerate() {
                    result.insert(i.to_string(), value.clone());
                }
                Ok(JsonbValue::Object(result))
            }

            // For other combinations, return the right operand
            (_, other) => Ok(other.clone()),
        }
    }

    /// `-` operator: Delete key/value pair or string element from left operand
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::JsonbValue;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#)?;
    /// let result = json.delete_key("age")?;
    /// // result: {"name": "Alice"}
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_key(&self, key: &str) -> JsonbResult<JsonbValue> {
        match self {
            JsonbValue::Object(obj) => {
                let mut result = obj.clone();
                result.remove(key);
                Ok(JsonbValue::Object(result))
            }
            JsonbValue::Array(arr) => {
                if let Ok(index) = key.parse::<usize>() {
                    let mut result = arr.clone();
                    if index < result.len() {
                        result.remove(index);
                    }
                    Ok(JsonbValue::Array(result))
                } else {
                    // For arrays, non-numeric keys are ignored
                    Ok(self.clone())
                }
            }
            _ => Ok(self.clone()),
        }
    }

    /// `#-` operator: Delete the field or array element at the specified path
    ///
    /// # Examples
    /// ```
    /// use orbit_protocols::postgres_wire::jsonb::{JsonbValue, JsonbPath};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = JsonbValue::from_json_str(r#"{"user": {"name": "Alice", "age": 30}}"#)?;
    /// let path = JsonbPath::from_str("user.age")?;
    /// let result = json.delete_path(&path)?;
    /// // result: {"user": {"name": "Alice"}}
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_path(&self, path: &JsonbPath) -> JsonbResult<JsonbValue> {
        if path.is_empty() {
            return Ok(JsonbValue::Null);
        }

        let mut result = self.clone();

        // For single-component paths, use direct deletion
        if path.components.len() == 1 {
            match &path.components[0] {
                PathComponent::Key(key) => return result.delete_key(key),
                PathComponent::Index(index) => return result.delete_key(&index.to_string()),
                _ => {
                    return Err(JsonbError::InvalidOperation(
                        "Unsupported path component for deletion".to_string(),
                    ))
                }
            }
        }

        // For multi-component paths, navigate to parent and delete the last component
        let parent_path = JsonbPath {
            components: path.components[..path.components.len() - 1].to_vec(),
            recursive: false,
        };

        if let Some(parent) = result.get_path_mut(&parent_path)? {
            match &path.components[path.components.len() - 1] {
                PathComponent::Key(key) => {
                    if let JsonbValue::Object(obj) = parent {
                        obj.remove(key);
                    }
                }
                PathComponent::Index(index) => {
                    if let JsonbValue::Array(arr) = parent {
                        if *index < arr.len() {
                            arr.remove(*index);
                        }
                    }
                }
                _ => {
                    return Err(JsonbError::InvalidOperation(
                        "Unsupported path component for deletion".to_string(),
                    ))
                }
            }
        }

        Ok(result)
    }

    /// Convert JSON value to text representation (used by ->> and #>> operators)
    pub fn to_text(&self) -> String {
        match self {
            JsonbValue::Null => "".to_string(),
            JsonbValue::Bool(b) => b.to_string(),
            JsonbValue::Number(n) => {
                // Format number without unnecessary decimal places
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            JsonbValue::String(s) => s.clone(),
            JsonbValue::Array(_) | JsonbValue::Object(_) => self.to_json_compact(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow_json_operator() {
        let json =
            JsonbValue::from_json_str(r#"{"name": "Alice", "tags": ["dev", "rust"]}"#).unwrap();

        let name = json.arrow_json(&["name"]).unwrap().unwrap();
        assert_eq!(name, JsonbValue::String("Alice".to_string()));

        let tag = json.arrow_json(&["tags", "0"]).unwrap().unwrap();
        assert_eq!(tag, JsonbValue::String("dev".to_string()));

        let missing = json.arrow_json(&["missing"]).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_arrow_text_operator() {
        let json =
            JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30, "active": true}"#).unwrap();

        assert_eq!(json.arrow_text(&["name"]).unwrap().unwrap(), "Alice");
        assert_eq!(json.arrow_text(&["age"]).unwrap().unwrap(), "30");
        assert_eq!(json.arrow_text(&["active"]).unwrap().unwrap(), "true");

        let missing = json.arrow_text(&["missing"]).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_path_operators() {
        let json =
            JsonbValue::from_json_str(r#"{"user": {"profile": {"name": "Alice", "age": 30}}}"#)
                .unwrap();

        let path = JsonbPath::from_str("user.profile.name").unwrap();
        let result = json.path_json(&path).unwrap().unwrap();
        assert_eq!(result, JsonbValue::String("Alice".to_string()));

        let text_result = json.path_text(&path).unwrap().unwrap();
        assert_eq!(text_result, "Alice");
    }

    #[test]
    fn test_contains_operator() {
        let json1 =
            JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30, "tags": ["dev", "rust"]}"#)
                .unwrap();
        let json2 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#).unwrap();
        let json3 = JsonbValue::from_json_str(r#"{"name": "Bob"}"#).unwrap();

        assert!(json1.contains(&json2).unwrap());
        assert!(!json1.contains(&json3).unwrap());

        let arr1 = JsonbValue::from_json_str(r#"[1, 2, 3, 4]"#).unwrap();
        let arr2 = JsonbValue::from_json_str(r#"[2, 3]"#).unwrap();
        let arr3 = JsonbValue::from_json_str(r#"[5, 6]"#).unwrap();

        assert!(arr1.contains(&arr2).unwrap());
        assert!(!arr1.contains(&arr3).unwrap());
    }

    #[test]
    fn test_key_existence_operators() {
        let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#).unwrap();

        assert!(json.has_key("name").unwrap());
        assert!(!json.has_key("email").unwrap());

        assert!(json.has_any_key(&["name", "email"]).unwrap());
        assert!(!json.has_any_key(&["email", "phone"]).unwrap());

        assert!(json.has_all_keys(&["name", "age"]).unwrap());
        assert!(!json.has_all_keys(&["name", "age", "email"]).unwrap());
    }

    #[test]
    fn test_concat_operator() {
        let json1 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#).unwrap();
        let json2 = JsonbValue::from_json_str(r#"{"age": 30}"#).unwrap();

        let result = json1.concat(&json2).unwrap();
        if let JsonbValue::Object(obj) = result {
            assert_eq!(
                obj.get("name").unwrap(),
                &JsonbValue::String("Alice".to_string())
            );
            assert_eq!(obj.get("age").unwrap(), &JsonbValue::Number(30.0));
        } else {
            panic!("Expected object result from concat");
        }

        let arr1 = JsonbValue::from_json_str(r#"[1, 2]"#).unwrap();
        let arr2 = JsonbValue::from_json_str(r#"[3, 4]"#).unwrap();
        let arr_result = arr1.concat(&arr2).unwrap();

        if let JsonbValue::Array(arr) = arr_result {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[0], JsonbValue::Number(1.0));
            assert_eq!(arr[3], JsonbValue::Number(4.0));
        } else {
            panic!("Expected array result from concat");
        }
    }

    #[test]
    fn test_delete_operators() {
        let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
        let result = json.delete_key("age").unwrap();

        if let JsonbValue::Object(obj) = result {
            assert!(obj.contains_key("name"));
            assert!(!obj.contains_key("age"));
        } else {
            panic!("Expected object result from delete");
        }

        let arr = JsonbValue::from_json_str(r#"["a", "b", "c"]"#).unwrap();
        let arr_result = arr.delete_key("1").unwrap();

        if let JsonbValue::Array(arr) = arr_result {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], JsonbValue::String("a".to_string()));
            assert_eq!(arr[1], JsonbValue::String("c".to_string()));
        } else {
            panic!("Expected array result from delete");
        }
    }

    #[test]
    fn test_to_text_conversion() {
        assert_eq!(JsonbValue::Null.to_text(), "");
        assert_eq!(JsonbValue::Bool(true).to_text(), "true");
        assert_eq!(JsonbValue::Number(42.0).to_text(), "42");
        assert_eq!(
            JsonbValue::Number(std::f64::consts::PI).to_text(),
            "3.141592653589793"
        );
        assert_eq!(JsonbValue::String("hello".to_string()).to_text(), "hello");

        let arr = JsonbValue::from_json_str(r#"[1, 2, 3]"#).unwrap();
        // JSON numbers are parsed as f64, so they will be formatted with decimals unless they're whole numbers
        // Our to_text() method uses JSON serialization for arrays, which formats floats as "1.0"
        assert_eq!(arr.to_text(), "[1.0,2.0,3.0]");
    }
}
