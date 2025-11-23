//! JSON Indexing Strategies
//!
//! This module implements various indexing strategies for JSON data:
//! - GIN (Generalized Inverted Index) for containment queries
//! - Expression indexes for specific JSON paths
//! - Hash indexes for exact key matches
//! - B-tree indexes for ordered JSON path values

use crate::protocols::postgres_wire::jsonb::{JsonbPath, JsonbResult, JsonbValue};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Index type for JSON data
#[derive(Debug, Clone)]
pub enum JsonIndexType {
    /// GIN index for containment and existence queries (@>, ?, ?&, ?|)
    Gin,
    /// B-tree index for ordering and range queries on JSON paths
    BTree,
    /// Hash index for exact equality matches on JSON paths
    Hash,
    /// Expression index for specific JSON path expressions
    Expression(JsonbPath),
}

/// GIN (Generalized Inverted Index) for JSON containment queries
#[derive(Debug, Clone)]
pub struct GinIndex {
    /// Maps keys to row IDs that contain them
    key_index: HashMap<String, HashSet<u64>>,
    /// Maps value hashes to row IDs (for @> containment)
    value_index: HashMap<u64, HashSet<u64>>,
    /// Maps path-value pairs to row IDs
    path_value_index: HashMap<String, HashMap<u64, HashSet<u64>>>,
}

impl GinIndex {
    pub fn new() -> Self {
        Self {
            key_index: HashMap::new(),
            value_index: HashMap::new(),
            path_value_index: HashMap::new(),
        }
    }

    /// Add a JSONB value to the index with the given row ID
    pub fn insert(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        self.index_value(row_id, value, "")?;
        Ok(())
    }

    /// Remove a JSONB value from the index
    pub fn remove(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        self.deindex_value(row_id, value, "")?;
        Ok(())
    }

    /// Find rows that contain the given key (? operator)
    pub fn contains_key(&self, key: &str) -> HashSet<u64> {
        self.key_index.get(key).cloned().unwrap_or_default()
    }

    /// Find rows that contain any of the given keys (?| operator)
    pub fn contains_any_key(&self, keys: &[&str]) -> HashSet<u64> {
        let mut result = HashSet::new();
        for key in keys {
            if let Some(row_ids) = self.key_index.get(*key) {
                result.extend(row_ids);
            }
        }
        result
    }

    /// Find rows that contain all of the given keys (?& operator)
    pub fn contains_all_keys(&self, keys: &[&str]) -> HashSet<u64> {
        if keys.is_empty() {
            return HashSet::new();
        }

        let mut result = self.key_index.get(keys[0]).cloned().unwrap_or_default();

        for key in keys.iter().skip(1) {
            if let Some(key_rows) = self.key_index.get(*key) {
                result = result.intersection(key_rows).cloned().collect();
            } else {
                return HashSet::new(); // If any key is missing, no rows can contain all
            }
        }

        result
    }

    /// Find rows that contain the given JSON value (@> operator)
    pub fn contains_value(&self, value: &JsonbValue) -> HashSet<u64> {
        let value_hash = self.hash_value(value);
        self.value_index
            .get(&value_hash)
            .cloned()
            .unwrap_or_default()
    }

    fn index_value(&mut self, row_id: u64, value: &JsonbValue, path: &str) -> JsonbResult<()> {
        match value {
            JsonbValue::Object(obj) => {
                for (key, val) in obj {
                    // Index the key
                    self.key_index
                        .entry(key.clone())
                        .or_default()
                        .insert(row_id);

                    // Index the value
                    let value_hash = self.hash_value(val);
                    self.value_index
                        .entry(value_hash)
                        .or_default()
                        .insert(row_id);

                    // Index path-value pair
                    let full_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };

                    self.path_value_index
                        .entry(full_path.clone())
                        .or_default()
                        .entry(value_hash)
                        .or_default()
                        .insert(row_id);

                    // Recursively index nested values
                    self.index_value(row_id, val, &full_path)?;
                }
            }
            JsonbValue::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let full_path = if path.is_empty() {
                        i.to_string()
                    } else {
                        format!("{path}[{i}]")
                    };

                    let value_hash = self.hash_value(item);
                    self.value_index
                        .entry(value_hash)
                        .or_default()
                        .insert(row_id);

                    self.path_value_index
                        .entry(full_path.clone())
                        .or_default()
                        .entry(value_hash)
                        .or_default()
                        .insert(row_id);

                    self.index_value(row_id, item, &full_path)?;
                }
            }
            _ => {
                // For primitive values, just index the value itself
                let value_hash = self.hash_value(value);
                self.value_index
                    .entry(value_hash)
                    .or_default()
                    .insert(row_id);

                if !path.is_empty() {
                    self.path_value_index
                        .entry(path.to_string())
                        .or_default()
                        .entry(value_hash)
                        .or_default()
                        .insert(row_id);
                }
            }
        }
        Ok(())
    }

    fn deindex_value(&mut self, row_id: u64, value: &JsonbValue, path: &str) -> JsonbResult<()> {
        match value {
            JsonbValue::Object(obj) => self.deindex_object(row_id, obj, path),
            JsonbValue::Array(arr) => self.deindex_array(row_id, arr, path),
            _ => self.deindex_primitive(row_id, value, path),
        }
    }

    /// Remove object values from all relevant indexes
    fn deindex_object(
        &mut self,
        row_id: u64,
        obj: &std::collections::HashMap<String, JsonbValue>,
        path: &str,
    ) -> JsonbResult<()> {
        for (key, val) in obj {
            self.deindex_object_entry(row_id, key, val, path)?;
        }
        Ok(())
    }

    /// Remove a single object key-value pair from indexes
    fn deindex_object_entry(
        &mut self,
        row_id: u64,
        key: &str,
        val: &JsonbValue,
        path: &str,
    ) -> JsonbResult<()> {
        // Remove from key index
        self.remove_from_key_index(row_id, key);

        let value_hash = self.hash_value(val);
        let full_path = self.build_object_path(path, key);

        // Remove from value and path-value indexes
        self.remove_from_value_index(row_id, value_hash);
        self.remove_from_path_value_index(row_id, &full_path, value_hash);

        // Recursively deindex nested values
        self.deindex_value(row_id, val, &full_path)
    }

    /// Remove array values from all relevant indexes
    fn deindex_array(&mut self, row_id: u64, arr: &[JsonbValue], path: &str) -> JsonbResult<()> {
        for (i, item) in arr.iter().enumerate() {
            self.deindex_array_item(row_id, i, item, path)?;
        }
        Ok(())
    }

    /// Remove a single array item from indexes
    fn deindex_array_item(
        &mut self,
        row_id: u64,
        index: usize,
        item: &JsonbValue,
        path: &str,
    ) -> JsonbResult<()> {
        let value_hash = self.hash_value(item);
        let full_path = self.build_array_path(path, index);

        // Remove from value and path-value indexes
        self.remove_from_value_index(row_id, value_hash);
        self.remove_from_path_value_index(row_id, &full_path, value_hash);

        // Recursively deindex nested values
        self.deindex_value(row_id, item, &full_path)
    }

    /// Remove primitive values from indexes
    fn deindex_primitive(
        &mut self,
        row_id: u64,
        value: &JsonbValue,
        path: &str,
    ) -> JsonbResult<()> {
        let value_hash = self.hash_value(value);
        self.remove_from_value_index(row_id, value_hash);

        if !path.is_empty() {
            self.remove_from_path_value_index(row_id, path, value_hash);
        }
        Ok(())
    }

    /// Helper to remove entry from key index
    fn remove_from_key_index(&mut self, row_id: u64, key: &str) {
        if let Some(key_set) = self.key_index.get_mut(key) {
            key_set.remove(&row_id);
            if key_set.is_empty() {
                self.key_index.remove(key);
            }
        }
    }

    /// Helper to remove entry from value index
    fn remove_from_value_index(&mut self, row_id: u64, value_hash: u64) {
        if let Some(value_set) = self.value_index.get_mut(&value_hash) {
            value_set.remove(&row_id);
            if value_set.is_empty() {
                self.value_index.remove(&value_hash);
            }
        }
    }

    /// Helper to remove entry from path-value index
    fn remove_from_path_value_index(&mut self, row_id: u64, path: &str, value_hash: u64) {
        if let Some(path_map) = self.path_value_index.get_mut(path) {
            if let Some(value_set) = path_map.get_mut(&value_hash) {
                value_set.remove(&row_id);
                if value_set.is_empty() {
                    path_map.remove(&value_hash);
                }
            }
            if path_map.is_empty() {
                self.path_value_index.remove(path);
            }
        }
    }

    /// Build path for object property
    fn build_object_path(&self, path: &str, key: &str) -> String {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        }
    }

    /// Build path for array index
    fn build_array_path(&self, path: &str, index: usize) -> String {
        if path.is_empty() {
            index.to_string()
        } else {
            format!("{path}[{index}]")
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn hash_value(&self, value: &JsonbValue) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        match value {
            JsonbValue::Null => 0u8.hash(&mut hasher),
            JsonbValue::Bool(b) => b.hash(&mut hasher),
            JsonbValue::Number(n) => n.to_bits().hash(&mut hasher),
            JsonbValue::String(s) => s.hash(&mut hasher),
            JsonbValue::Array(arr) => {
                arr.len().hash(&mut hasher);
                for item in arr {
                    self.hash_value(item).hash(&mut hasher);
                }
            }
            JsonbValue::Object(obj) => {
                obj.len().hash(&mut hasher);
                let mut keys: Vec<_> = obj.keys().collect();
                keys.sort();
                for key in keys {
                    key.hash(&mut hasher);
                    if let Some(val) = obj.get(key) {
                        self.hash_value(val).hash(&mut hasher);
                    }
                }
            }
        }
        hasher.finish()
    }
}

impl Default for GinIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// B-tree index for ordered JSON path queries
#[derive(Debug, Clone)]
pub struct BTreeIndex {
    /// Maps JSON path values to sorted row IDs
    index: BTreeMap<String, Vec<u64>>,
    /// The JSON path being indexed
    path: JsonbPath,
}

impl BTreeIndex {
    pub fn new(path: JsonbPath) -> Self {
        Self {
            index: BTreeMap::new(),
            path,
        }
    }

    pub fn insert(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        if let Some(path_value) = value.get_path(&self.path)? {
            let key = path_value.to_text();
            self.index.entry(key).or_default().push(row_id);
        }
        Ok(())
    }

    pub fn remove(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        if let Some(path_value) = value.get_path(&self.path)? {
            let key = path_value.to_text();
            if let Some(row_ids) = self.index.get_mut(&key) {
                row_ids.retain(|&id| id != row_id);
                if row_ids.is_empty() {
                    self.index.remove(&key);
                }
            }
        }
        Ok(())
    }

    /// Find rows with exact value match
    pub fn find_equal(&self, value: &str) -> Vec<u64> {
        self.index.get(value).cloned().unwrap_or_default()
    }

    /// Find rows with values in the given range
    pub fn find_range(&self, min: &str, max: &str) -> Vec<u64> {
        let mut result = Vec::new();
        for (_key, row_ids) in self.index.range(min.to_string()..=max.to_string()) {
            result.extend(row_ids);
        }
        result
    }

    /// Find rows with values less than the given value
    pub fn find_less_than(&self, value: &str) -> Vec<u64> {
        let mut result = Vec::new();
        for (_, row_ids) in self.index.range(..value.to_string()) {
            result.extend(row_ids);
        }
        result
    }

    /// Find rows with values greater than the given value
    pub fn find_greater_than(&self, value: &str) -> Vec<u64> {
        let mut result = Vec::new();
        for (_, row_ids) in self.index.range((
            std::ops::Bound::Excluded(value.to_string()),
            std::ops::Bound::Unbounded,
        )) {
            result.extend(row_ids);
        }
        result
    }
}

/// Hash index for exact JSON path equality matches
#[derive(Debug, Clone)]
pub struct HashIndex {
    /// Maps JSON path values to row IDs
    index: HashMap<String, HashSet<u64>>,
    /// The JSON path being indexed
    path: JsonbPath,
}

impl HashIndex {
    pub fn new(path: JsonbPath) -> Self {
        Self {
            index: HashMap::new(),
            path,
        }
    }

    pub fn insert(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        if let Some(path_value) = value.get_path(&self.path)? {
            let key = path_value.to_text();
            self.index.entry(key).or_default().insert(row_id);
        }
        Ok(())
    }

    pub fn remove(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        if let Some(path_value) = value.get_path(&self.path)? {
            let key = path_value.to_text();
            if let Some(row_ids) = self.index.get_mut(&key) {
                row_ids.remove(&row_id);
                if row_ids.is_empty() {
                    self.index.remove(&key);
                }
            }
        }
        Ok(())
    }

    /// Find rows with exact value match
    pub fn find(&self, value: &str) -> HashSet<u64> {
        self.index.get(value).cloned().unwrap_or_default()
    }
}

/// Expression index for complex JSON expressions
#[derive(Debug, Clone)]
pub struct ExpressionIndex {
    /// The expression being indexed (stored as path for simplicity)
    #[allow(dead_code)]
    expression: JsonbPath,
    /// Hash index for the expression results
    hash_index: HashIndex,
}

impl ExpressionIndex {
    pub fn new(expression: JsonbPath) -> Self {
        let hash_index = HashIndex::new(expression.clone());
        Self {
            expression,
            hash_index,
        }
    }

    pub fn insert(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        self.hash_index.insert(row_id, value)
    }

    pub fn remove(&mut self, row_id: u64, value: &JsonbValue) -> JsonbResult<()> {
        self.hash_index.remove(row_id, value)
    }

    pub fn find(&self, value: &str) -> HashSet<u64> {
        self.hash_index.find(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gin_index_key_operations() {
        let mut gin = GinIndex::new();
        let json =
            JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30, "tags": ["dev", "rust"]}"#)
                .unwrap();

        gin.insert(1, &json).unwrap();

        // Test ? operator
        assert!(gin.contains_key("name").contains(&1));
        assert!(gin.contains_key("age").contains(&1));
        assert!(!gin.contains_key("email").contains(&1));

        // Test ?& operator
        assert!(gin.contains_all_keys(&["name", "age"]).contains(&1));
        assert!(gin.contains_all_keys(&["name", "age", "email"]).is_empty());

        // Test ?| operator
        assert!(gin.contains_any_key(&["name", "email"]).contains(&1));
        assert!(gin.contains_any_key(&["email", "phone"]).is_empty());
    }

    #[test]
    fn test_gin_index_multiple_rows() {
        let mut gin = GinIndex::new();

        let json1 = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
        let json2 = JsonbValue::from_json_str(r#"{"name": "Bob", "city": "NYC"}"#).unwrap();

        gin.insert(1, &json1).unwrap();
        gin.insert(2, &json2).unwrap();

        let name_rows = gin.contains_key("name");
        assert!(name_rows.contains(&1));
        assert!(name_rows.contains(&2));

        let age_rows = gin.contains_key("age");
        assert!(age_rows.contains(&1));
        assert!(!age_rows.contains(&2));
    }

    #[test]
    fn test_btree_index() {
        let path = JsonbPath::from_str("age").unwrap();
        let mut btree = BTreeIndex::new(path);

        let json1 = JsonbValue::from_json_str(r#"{"age": 25}"#).unwrap();
        let json2 = JsonbValue::from_json_str(r#"{"age": 35}"#).unwrap();
        let json3 = JsonbValue::from_json_str(r#"{"age": 45}"#).unwrap();

        btree.insert(1, &json1).unwrap();
        btree.insert(2, &json2).unwrap();
        btree.insert(3, &json3).unwrap();

        // Test exact match
        assert!(btree.find_equal("35").contains(&2));

        // Test range query
        let range_results = btree.find_range("30", "40");
        assert!(range_results.contains(&2));
        assert!(!range_results.contains(&1));
        assert!(!range_results.contains(&3));

        // Test less than
        let less_than_results = btree.find_less_than("30");
        assert!(less_than_results.contains(&1));
        assert!(!less_than_results.contains(&2));
    }

    #[test]
    fn test_hash_index() {
        let path = JsonbPath::from_str("name").unwrap();
        let mut hash_index = HashIndex::new(path);

        let json1 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#).unwrap();
        let json2 = JsonbValue::from_json_str(r#"{"name": "Bob"}"#).unwrap();
        let json3 = JsonbValue::from_json_str(r#"{"name": "Alice"}"#).unwrap();

        hash_index.insert(1, &json1).unwrap();
        hash_index.insert(2, &json2).unwrap();
        hash_index.insert(3, &json3).unwrap();

        let alice_rows = hash_index.find("Alice");
        assert!(alice_rows.contains(&1));
        assert!(alice_rows.contains(&3));
        assert!(!alice_rows.contains(&2));

        let bob_rows = hash_index.find("Bob");
        assert!(bob_rows.contains(&2));
        assert!(!bob_rows.contains(&1));
    }

    #[test]
    fn test_gin_index_removal() {
        let mut gin = GinIndex::new();
        let json = JsonbValue::from_json_str(r#"{"name": "Alice", "age": 30}"#).unwrap();

        gin.insert(1, &json).unwrap();
        assert!(gin.contains_key("name").contains(&1));

        gin.remove(1, &json).unwrap();
        assert!(!gin.contains_key("name").contains(&1));
        assert!(gin.contains_key("name").is_empty());
    }

    #[test]
    fn test_nested_json_indexing() {
        let mut gin = GinIndex::new();
        let json = JsonbValue::from_json_str(
            r#"{"user": {"profile": {"name": "Alice"}}, "tags": ["dev", "rust"]}"#,
        )
        .unwrap();

        gin.insert(1, &json).unwrap();

        // Should be able to find top-level keys
        assert!(gin.contains_key("user").contains(&1));
        assert!(gin.contains_key("tags").contains(&1));

        // Should also index nested keys (depending on implementation)
        // This would require path-aware indexing
    }
}
