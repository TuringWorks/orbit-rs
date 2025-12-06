//! MongoDB document storage implementation
//!
//! Provides in-memory document storage with collection support for MongoDB protocol.

use bson::{doc, oid::ObjectId, Bson, Document};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A MongoDB collection containing documents
#[derive(Debug, Default)]
pub struct Collection {
    /// Documents indexed by _id (stored as string representation)
    documents: HashMap<String, Document>,
    /// Indexes on the collection
    indexes: Vec<IndexDefinition>,
}

/// Index definition for a collection
#[derive(Debug, Clone)]
pub struct IndexDefinition {
    pub name: String,
    pub keys: Document,
    pub unique: bool,
    pub sparse: bool,
}

impl Collection {
    pub fn new() -> Self {
        // Create default _id index
        Self {
            documents: HashMap::new(),
            indexes: vec![IndexDefinition {
                name: "_id_".to_string(),
                keys: doc! { "_id": 1 },
                unique: true,
                sparse: false,
            }],
        }
    }

    /// Convert a Bson value to a string key for storage
    fn bson_to_key(bson: &Bson) -> String {
        match bson {
            Bson::ObjectId(oid) => oid.to_hex(),
            Bson::String(s) => s.clone(),
            Bson::Int32(i) => i.to_string(),
            Bson::Int64(i) => i.to_string(),
            _ => format!("{:?}", bson),
        }
    }

    /// Insert a document, auto-generating _id if not present
    pub fn insert_one(&mut self, mut doc: Document) -> Result<Bson, String> {
        let id = if let Some(id) = doc.get("_id") {
            let key = Self::bson_to_key(id);
            if self.documents.contains_key(&key) {
                return Err(format!("Duplicate key error: _id {:?} already exists", id));
            }
            id.clone()
        } else {
            let id = Bson::ObjectId(ObjectId::new());
            doc.insert("_id", id.clone());
            id
        };

        let key = Self::bson_to_key(&id);
        self.documents.insert(key, doc);
        Ok(id)
    }

    /// Insert multiple documents
    pub fn insert_many(&mut self, docs: Vec<Document>) -> Result<Vec<Bson>, String> {
        let mut ids = Vec::with_capacity(docs.len());
        for doc in docs {
            ids.push(self.insert_one(doc)?);
        }
        Ok(ids)
    }

    /// Find documents matching a filter
    pub fn find(&self, filter: &Document, limit: Option<i64>, skip: Option<i64>) -> Vec<Document> {
        let mut results: Vec<Document> = self
            .documents
            .values()
            .filter(|doc| matches_filter(doc, filter))
            .cloned()
            .collect();

        // Apply skip
        if let Some(s) = skip {
            if s > 0 {
                results = results.into_iter().skip(s as usize).collect();
            }
        }

        // Apply limit
        if let Some(l) = limit {
            if l > 0 {
                results.truncate(l as usize);
            }
        }

        results
    }

    /// Find a single document
    pub fn find_one(&self, filter: &Document) -> Option<Document> {
        self.documents
            .values()
            .find(|doc| matches_filter(doc, filter))
            .cloned()
    }

    /// Update documents matching filter
    pub fn update_many(&mut self, filter: &Document, update: &Document) -> (i64, i64) {
        let mut matched = 0i64;
        let mut modified = 0i64;

        let matching_keys: Vec<String> = self
            .documents
            .iter()
            .filter(|(_, doc)| matches_filter(doc, filter))
            .map(|(key, _)| key.clone())
            .collect();

        for key in matching_keys {
            matched += 1;
            if let Some(doc) = self.documents.get_mut(&key) {
                if apply_update(doc, update) {
                    modified += 1;
                }
            }
        }

        (matched, modified)
    }

    /// Update a single document
    pub fn update_one(&mut self, filter: &Document, update: &Document) -> (i64, i64) {
        for (_key, doc) in self.documents.iter_mut() {
            if matches_filter(doc, filter) {
                let modified = if apply_update(doc, update) { 1 } else { 0 };
                return (1, modified);
            }
        }
        (0, 0)
    }

    /// Delete documents matching filter
    pub fn delete_many(&mut self, filter: &Document) -> i64 {
        let matching_keys: Vec<String> = self
            .documents
            .iter()
            .filter(|(_, doc)| matches_filter(doc, filter))
            .map(|(key, _)| key.clone())
            .collect();

        let count = matching_keys.len() as i64;
        for key in matching_keys {
            self.documents.remove(&key);
        }
        count
    }

    /// Delete a single document
    pub fn delete_one(&mut self, filter: &Document) -> i64 {
        let key_to_remove = self
            .documents
            .iter()
            .find(|(_, doc)| matches_filter(doc, filter))
            .map(|(key, _)| key.clone());

        if let Some(key) = key_to_remove {
            self.documents.remove(&key);
            1
        } else {
            0
        }
    }

    /// Count documents matching filter
    pub fn count(&self, filter: &Document) -> i64 {
        self.documents
            .values()
            .filter(|doc| matches_filter(doc, filter))
            .count() as i64
    }

    /// Get collection statistics
    pub fn stats(&self) -> Document {
        doc! {
            "count": self.documents.len() as i64,
            "size": self.documents.len() as i64 * 1024, // Approximate
            "avgObjSize": 1024,
            "nindexes": self.indexes.len() as i32,
        }
    }

    /// Create an index
    pub fn create_index(
        &mut self,
        keys: Document,
        name: Option<String>,
        unique: bool,
        sparse: bool,
    ) -> String {
        let index_name = name.unwrap_or_else(|| {
            keys.iter()
                .map(|(k, v)| format!("{}_{}", k, v))
                .collect::<Vec<_>>()
                .join("_")
        });

        // Check if index already exists
        if !self.indexes.iter().any(|i| i.name == index_name) {
            self.indexes.push(IndexDefinition {
                name: index_name.clone(),
                keys,
                unique,
                sparse,
            });
        }

        index_name
    }

    /// List indexes
    pub fn list_indexes(&self) -> Vec<Document> {
        self.indexes
            .iter()
            .map(|idx| {
                doc! {
                    "name": &idx.name,
                    "key": idx.keys.clone(),
                    "unique": idx.unique,
                    "sparse": idx.sparse,
                }
            })
            .collect()
    }

    /// Find and modify a single document atomically
    pub fn find_and_modify(
        &mut self,
        query: &Document,
        sort: Option<&Document>,
        update: Option<&Document>,
        remove: bool,
        new_doc: bool,
        upsert: bool,
    ) -> Option<Document> {
        // Find matching documents
        let mut matching: Vec<(String, Document)> = self
            .documents
            .iter()
            .filter(|(_, doc)| matches_filter(doc, query))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Apply sort if specified
        if let Some(sort_doc) = sort {
            matching.sort_by(|(_, a), (_, b)| {
                for (key, order) in sort_doc {
                    let order_val = match order {
                        Bson::Int32(n) => *n,
                        Bson::Int64(n) => *n as i32,
                        _ => 1,
                    };

                    let a_val = a.get(key);
                    let b_val = b.get(key);

                    let cmp = compare_bson_values_direct(a_val, b_val);
                    if cmp != std::cmp::Ordering::Equal {
                        return if order_val < 0 { cmp.reverse() } else { cmp };
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        // Get the first matching document
        if let Some((key, mut doc)) = matching.first().cloned() {
            let original_doc = doc.clone();

            if remove {
                // Remove the document
                self.documents.remove(&key);
                return Some(original_doc);
            } else if let Some(update_spec) = update {
                // Apply update
                apply_update(&mut doc, update_spec);
                self.documents.insert(key, doc.clone());
                
                if new_doc {
                    return Some(doc);
                } else {
                    return Some(original_doc);
                }
            }

            return Some(original_doc);
        } else if upsert && !remove {
            // No match and upsert is true - insert new document
            if let Some(update_spec) = update {
                let mut inserted_doc = query.clone();
                
                // Apply update operators to create the new document
                apply_update(&mut inserted_doc, update_spec);
                
                // Generate _id if not present
                if !inserted_doc.contains_key("_id") {
                    inserted_doc.insert("_id", Bson::ObjectId(ObjectId::new()));
                }
                
                let id = inserted_doc.get("_id").cloned().unwrap();
                let key = Self::bson_to_key(&id);
                self.documents.insert(key, inserted_doc.clone());
                
                if new_doc {
                    return Some(inserted_doc);
                } else {
                    return None; // Original was null since we inserted
                }
            }
        }

        None
    }

    /// Find distinct values for a field
    pub fn distinct(&self, key: &str, query: Option<&Document>) -> Vec<Bson> {
        use std::collections::HashSet;
        
        // Get documents matching the query (or all if no query)
        let docs: Vec<&Document> = if let Some(filter) = query {
            self.documents
                .values()
                .filter(|doc| matches_filter(doc, filter))
                .collect()
        } else {
            self.documents.values().collect()
        };

        // Collect unique values
        let mut seen = HashSet::new();
        let mut values = Vec::new();

        for doc in docs {
            if let Some(value) = get_nested_value(doc, key) {
                // If the value is an array, add each element
                if let Bson::Array(arr) = value {
                    for item in arr {
                        let key_str = format!("{:?}", item);
                        if seen.insert(key_str) {
                            values.push(item);
                        }
                    }
                } else {
                    let key_str = format!("{:?}", value);
                    if seen.insert(key_str) {
                        values.push(value);
                    }
                }
            }
        }

        values
    }
}

/// Check if a document matches a MongoDB filter
pub fn matches_filter(doc: &Document, filter: &Document) -> bool {
    if filter.is_empty() {
        return true;
    }

    for (key, filter_value) in filter.iter() {
        match key.as_str() {
            // Logical operators
            "$and" => {
                if let Bson::Array(conditions) = filter_value {
                    for cond in conditions {
                        if let Bson::Document(cond_doc) = cond {
                            if !matches_filter(doc, cond_doc) {
                                return false;
                            }
                        }
                    }
                }
            }
            "$or" => {
                if let Bson::Array(conditions) = filter_value {
                    let mut any_match = false;
                    for cond in conditions {
                        if let Bson::Document(cond_doc) = cond {
                            if matches_filter(doc, cond_doc) {
                                any_match = true;
                                break;
                            }
                        }
                    }
                    if !any_match {
                        return false;
                    }
                }
            }
            "$nor" => {
                if let Bson::Array(conditions) = filter_value {
                    for cond in conditions {
                        if let Bson::Document(cond_doc) = cond {
                            if matches_filter(doc, cond_doc) {
                                return false;
                            }
                        }
                    }
                }
            }
            // Regular field match
            _ => {
                let doc_value = get_nested_value(doc, key);
                if !matches_value(&doc_value, filter_value) {
                    return false;
                }
            }
        }
    }

    true
}

/// Get a nested value from a document using dot notation
fn get_nested_value(doc: &Document, key: &str) -> Option<Bson> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: Option<&Bson> = None;

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            current = doc.get(*part);
        } else if let Some(Bson::Document(d)) = current {
            current = d.get(*part);
        } else if let Some(Bson::Array(arr)) = current {
            // Array index access
            if let Ok(idx) = part.parse::<usize>() {
                current = arr.get(idx);
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    current.cloned()
}

/// Check if a document value matches a filter value
fn matches_value(doc_value: &Option<Bson>, filter_value: &Bson) -> bool {
    match filter_value {
        Bson::Document(filter_doc) => {
            // Check for operators
            for (op, op_value) in filter_doc.iter() {
                match op.as_str() {
                    "$eq" => {
                        if doc_value.as_ref() != Some(op_value) {
                            return false;
                        }
                    }
                    "$ne" => {
                        if doc_value.as_ref() == Some(op_value) {
                            return false;
                        }
                    }
                    "$gt" => {
                        if !compare_bson(doc_value, op_value, |a, b| a > b) {
                            return false;
                        }
                    }
                    "$gte" => {
                        if !compare_bson(doc_value, op_value, |a, b| a >= b) {
                            return false;
                        }
                    }
                    "$lt" => {
                        if !compare_bson(doc_value, op_value, |a, b| a < b) {
                            return false;
                        }
                    }
                    "$lte" => {
                        if !compare_bson(doc_value, op_value, |a, b| a <= b) {
                            return false;
                        }
                    }
                    "$in" => {
                        if let Bson::Array(arr) = op_value {
                            if !arr.iter().any(|v| doc_value.as_ref() == Some(v)) {
                                return false;
                            }
                        }
                    }
                    "$nin" => {
                        if let Bson::Array(arr) = op_value {
                            if arr.iter().any(|v| doc_value.as_ref() == Some(v)) {
                                return false;
                            }
                        }
                    }
                    "$exists" => {
                        let should_exist = matches!(op_value, Bson::Boolean(true));
                        let exists = doc_value.is_some();
                        if should_exist != exists {
                            return false;
                        }
                    }
                    "$type" => {
                        // Simplified type checking
                        if doc_value.is_none() {
                            return false;
                        }
                    }
                    "$regex" => {
                        if let (Some(Bson::String(s)), Bson::String(pattern)) =
                            (doc_value, op_value)
                        {
                            if let Ok(re) = regex::Regex::new(pattern) {
                                if !re.is_match(s) {
                                    return false;
                                }
                            }
                        } else {
                            return false;
                        }
                    }
                    "$size" => {
                        if let Some(Bson::Array(arr)) = doc_value {
                            if let Some(size) = op_value.as_i64() {
                                if arr.len() as i64 != size {
                                    return false;
                                }
                            }
                        } else {
                            return false;
                        }
                    }
                    "$elemMatch" => {
                        if let Some(Bson::Array(arr)) = doc_value {
                            if let Bson::Document(elem_filter) = op_value {
                                let found = arr.iter().any(|elem| {
                                    if let Bson::Document(elem_doc) = elem {
                                        matches_filter(elem_doc, elem_filter)
                                    } else {
                                        false
                                    }
                                });
                                if !found {
                                    return false;
                                }
                            }
                        } else {
                            return false;
                        }
                    }
                    "$not" => {
                        // $not negates the result of the nested operator
                        if let Bson::Document(not_doc) = op_value {
                            if matches_value(doc_value, &Bson::Document(not_doc.clone())) {
                                return false;
                            }
                        }
                    }
                    "$all" => {
                        // $all matches arrays containing all specified elements
                        if let Some(Bson::Array(doc_arr)) = doc_value {
                            if let Bson::Array(filter_arr) = op_value {
                                for required in filter_arr {
                                    if !doc_arr.contains(required) {
                                        return false;
                                    }
                                }
                            }
                        } else {
                            return false;
                        }
                    }
                    "$mod" => {
                        // $mod performs modulo operation: { field: { $mod: [divisor, remainder] } }
                        if let Bson::Array(mod_arr) = op_value {
                            if mod_arr.len() == 2 {
                                let divisor = match &mod_arr[0] {
                                    Bson::Int32(n) => *n as i64,
                                    Bson::Int64(n) => *n,
                                    Bson::Double(n) => *n as i64,
                                    _ => return false,
                                };
                                let remainder = match &mod_arr[1] {
                                    Bson::Int32(n) => *n as i64,
                                    Bson::Int64(n) => *n,
                                    Bson::Double(n) => *n as i64,
                                    _ => return false,
                                };
                                let doc_num = match doc_value {
                                    Some(Bson::Int32(n)) => *n as i64,
                                    Some(Bson::Int64(n)) => *n,
                                    Some(Bson::Double(n)) => *n as i64,
                                    _ => return false,
                                };
                                if divisor != 0 && doc_num % divisor != remainder {
                                    return false;
                                }
                            }
                        }
                    }
                    "$expr" => {
                        // $expr allows aggregation expressions in queries - simplified support
                        // Full support would require expression evaluation context
                        // For now, just check if the expression exists
                        if doc_value.is_none() {
                            return false;
                        }
                    }
                    "$options" => {
                        // $options is used with $regex, handled separately
                        // Just continue processing
                    }
                    "$bitsAllSet" => {
                        // Check if all specified bits are set
                        if let (Some(doc_val), Bson::Int64(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n as i64,
                                Bson::Int64(n) => *n,
                                _ => return false,
                            };
                            if num & mask != *mask {
                                return false;
                            }
                        } else if let (Some(doc_val), Bson::Int32(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n,
                                Bson::Int64(n) => *n as i32,
                                _ => return false,
                            };
                            if num & mask != *mask {
                                return false;
                            }
                        }
                    }
                    "$bitsAnyClear" => {
                        // Check if any specified bits are clear
                        if let (Some(doc_val), Bson::Int64(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n as i64,
                                Bson::Int64(n) => *n,
                                _ => return false,
                            };
                            if num & mask == *mask {
                                return false;
                            }
                        } else if let (Some(doc_val), Bson::Int32(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n,
                                Bson::Int64(n) => *n as i32,
                                _ => return false,
                            };
                            if num & mask == *mask {
                                return false;
                            }
                        }
                    }
                    "$bitsAllClear" => {
                        // Check if all specified bits are clear
                        if let (Some(doc_val), Bson::Int64(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n as i64,
                                Bson::Int64(n) => *n,
                                _ => return false,
                            };
                            if num & mask != 0 {
                                return false;
                            }
                        } else if let (Some(doc_val), Bson::Int32(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n,
                                Bson::Int64(n) => *n as i32,
                                _ => return false,
                            };
                            if num & mask != 0 {
                                return false;
                            }
                        }
                    }
                    "$bitsAnySet" => {
                        // Check if any specified bits are set
                        if let (Some(doc_val), Bson::Int64(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n as i64,
                                Bson::Int64(n) => *n,
                                _ => return false,
                            };
                            if num & mask == 0 {
                                return false;
                            }
                        } else if let (Some(doc_val), Bson::Int32(mask)) = (doc_value, op_value) {
                            let num = match doc_val {
                                Bson::Int32(n) => *n,
                                Bson::Int64(n) => *n as i32,
                                _ => return false,
                            };
                            if num & mask == 0 {
                                return false;
                            }
                        }
                    }
                    _ => {
                        // Unknown operator, treat as nested document match
                        if doc_value.as_ref() != Some(filter_value) {
                            return false;
                        }
                    }
                }
            }
            true
        }
        // Direct equality match
        _ => doc_value.as_ref() == Some(filter_value),
    }
}

/// Compare two BSON values directly for sorting
fn compare_bson_values_direct(a: Option<&Bson>, b: Option<&Bson>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(Bson::Null), Some(Bson::Null)) => std::cmp::Ordering::Equal,
        (Some(Bson::Null), _) => std::cmp::Ordering::Less,
        (_, Some(Bson::Null)) => std::cmp::Ordering::Greater,
        (Some(Bson::Int32(x)), Some(Bson::Int32(y))) => x.cmp(y),
        (Some(Bson::Int64(x)), Some(Bson::Int64(y))) => x.cmp(y),
        (Some(Bson::Int32(x)), Some(Bson::Int64(y))) => (*x as i64).cmp(y),
        (Some(Bson::Int64(x)), Some(Bson::Int32(y))) => x.cmp(&(*y as i64)),
        (Some(Bson::Double(x)), Some(Bson::Double(y))) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Some(Bson::String(x)), Some(Bson::String(y))) => x.cmp(y),
        (Some(Bson::Boolean(x)), Some(Bson::Boolean(y))) => x.cmp(y),
        (Some(Bson::DateTime(x)), Some(Bson::DateTime(y))) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Compare BSON values with a comparison function
fn compare_bson<F>(doc_value: &Option<Bson>, filter_value: &Bson, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (doc_value, filter_value) {
        (Some(Bson::Int32(a)), Bson::Int32(b)) => cmp(*a as f64, *b as f64),
        (Some(Bson::Int64(a)), Bson::Int64(b)) => cmp(*a as f64, *b as f64),
        (Some(Bson::Double(a)), Bson::Double(b)) => cmp(*a, *b),
        (Some(Bson::Int32(a)), Bson::Int64(b)) => cmp(*a as f64, *b as f64),
        (Some(Bson::Int64(a)), Bson::Int32(b)) => cmp(*a as f64, *b as f64),
        (Some(Bson::Int32(a)), Bson::Double(b)) => cmp(*a as f64, *b),
        (Some(Bson::Double(a)), Bson::Int32(b)) => cmp(*a, *b as f64),
        (Some(Bson::Int64(a)), Bson::Double(b)) => cmp(*a as f64, *b),
        (Some(Bson::Double(a)), Bson::Int64(b)) => cmp(*a, *b as f64),
        (Some(Bson::String(a)), Bson::String(b)) => cmp(a.len() as f64, b.len() as f64), // String comparison by length for gt/lt
        (Some(Bson::DateTime(a)), Bson::DateTime(b)) => {
            cmp(a.timestamp_millis() as f64, b.timestamp_millis() as f64)
        }
        _ => false,
    }
}

/// Apply an update to a document
fn apply_update(doc: &mut Document, update: &Document) -> bool {
    let mut modified = false;

    for (op, op_value) in update.iter() {
        match op.as_str() {
            "$set" => {
                if let Bson::Document(fields) = op_value {
                    for (key, value) in fields.iter() {
                        set_nested_value(doc, key, value.clone());
                        modified = true;
                    }
                }
            }
            "$unset" => {
                if let Bson::Document(fields) = op_value {
                    for (key, _) in fields.iter() {
                        if doc.remove(key).is_some() {
                            modified = true;
                        }
                    }
                }
            }
            "$inc" => {
                if let Bson::Document(fields) = op_value {
                    for (key, inc_value) in fields.iter() {
                        if let Some(current) = doc.get_mut(key) {
                            match (current, inc_value) {
                                (Bson::Int32(c), Bson::Int32(i)) => {
                                    *c += i;
                                    modified = true;
                                }
                                (Bson::Int64(c), Bson::Int64(i)) => {
                                    *c += i;
                                    modified = true;
                                }
                                (Bson::Double(c), Bson::Double(i)) => {
                                    *c += i;
                                    modified = true;
                                }
                                (Bson::Int32(c), Bson::Int64(i)) => {
                                    *c += *i as i32;
                                    modified = true;
                                }
                                (Bson::Int64(c), Bson::Int32(i)) => {
                                    *c += *i as i64;
                                    modified = true;
                                }
                                _ => {}
                            }
                        } else {
                            doc.insert(key.clone(), inc_value.clone());
                            modified = true;
                        }
                    }
                }
            }
            "$mul" => {
                // $mul multiplies the value of a field by a number
                if let Bson::Document(fields) = op_value {
                    for (key, mul_value) in fields.iter() {
                        if let Some(current) = doc.get_mut(key) {
                            match (current, mul_value) {
                                (Bson::Int32(c), Bson::Int32(m)) => {
                                    *c *= m;
                                    modified = true;
                                }
                                (Bson::Int64(c), Bson::Int64(m)) => {
                                    *c *= m;
                                    modified = true;
                                }
                                (Bson::Double(c), Bson::Double(m)) => {
                                    *c *= m;
                                    modified = true;
                                }
                                (Bson::Int32(c), Bson::Double(m)) => {
                                    *c = (*c as f64 * m) as i32;
                                    modified = true;
                                }
                                (Bson::Double(c), Bson::Int32(m)) => {
                                    *c *= *m as f64;
                                    modified = true;
                                }
                                (Bson::Int64(c), Bson::Double(m)) => {
                                    *c = (*c as f64 * m) as i64;
                                    modified = true;
                                }
                                (Bson::Double(c), Bson::Int64(m)) => {
                                    *c *= *m as f64;
                                    modified = true;
                                }
                                _ => {}
                            }
                        } else {
                            // If field doesn't exist, set to 0
                            doc.insert(key.clone(), Bson::Int32(0));
                            modified = true;
                        }
                    }
                }
            }
            "$push" => {
                if let Bson::Document(fields) = op_value {
                    for (key, push_value) in fields.iter() {
                        // Check if push_value is a document with $each modifier
                        if let Bson::Document(push_doc) = push_value {
                            if push_doc.contains_key("$each") {
                                // Handle $each with optional modifiers ($slice, $sort, $position)
                                let each_values = push_doc.get_array("$each").ok();
                                let slice = push_doc.get_i32("$slice").ok().or_else(|| push_doc.get_i64("$slice").ok().map(|v| v as i32));
                                let position = push_doc.get_i32("$position").ok().or_else(|| push_doc.get_i64("$position").ok().map(|v| v as i32));
                                let sort = push_doc.get_document("$sort").ok();

                                if let Some(values) = each_values {
                                    let arr = doc.entry(key.clone()).or_insert_with(|| Bson::Array(Vec::new()));
                                    if let Bson::Array(arr) = arr {
                                        // Insert at position or append
                                        let insert_pos = position.map(|p| {
                                            if p >= 0 { p as usize } else { (arr.len() as i32 + p).max(0) as usize }
                                        }).unwrap_or(arr.len());

                                        for (i, v) in values.iter().enumerate() {
                                            arr.insert(insert_pos + i, v.clone());
                                        }

                                        // Apply $sort if specified
                                        if let Some(sort_doc) = sort {
                                            if let Some((sort_field, sort_order)) = sort_doc.iter().next() {
                                                let order = match sort_order {
                                                    Bson::Int32(n) => *n,
                                                    Bson::Int64(n) => *n as i32,
                                                    _ => 1,
                                                };
                                                arr.sort_by(|a, b| {
                                                    let a_val = if let Bson::Document(d) = a { d.get(sort_field) } else { None };
                                                    let b_val = if let Bson::Document(d) = b { d.get(sort_field) } else { None };
                                                    let cmp = compare_bson_values_direct(a_val, b_val);
                                                    if order < 0 { cmp.reverse() } else { cmp }
                                                });
                                            }
                                        }

                                        // Apply $slice if specified
                                        if let Some(s) = slice {
                                            if s >= 0 {
                                                arr.truncate(s as usize);
                                            } else {
                                                let start = (arr.len() as i32 + s).max(0) as usize;
                                                *arr = arr[start..].to_vec();
                                            }
                                        }

                                        modified = true;
                                    }
                                }
                                continue;
                            }
                        }

                        // Simple push
                        if let Some(Bson::Array(arr)) = doc.get_mut(key) {
                            arr.push(push_value.clone());
                            modified = true;
                        } else {
                            doc.insert(key.clone(), Bson::Array(vec![push_value.clone()]));
                            modified = true;
                        }
                    }
                }
            }
            "$pop" => {
                // $pop removes the first or last element of an array
                if let Bson::Document(fields) = op_value {
                    for (key, pop_value) in fields.iter() {
                        if let Some(Bson::Array(arr)) = doc.get_mut(key) {
                            if !arr.is_empty() {
                                let direction = match pop_value {
                                    Bson::Int32(n) => *n,
                                    Bson::Int64(n) => *n as i32,
                                    _ => 1,
                                };
                                if direction >= 0 {
                                    arr.pop(); // Remove last
                                } else {
                                    arr.remove(0); // Remove first
                                }
                                modified = true;
                            }
                        }
                    }
                }
            }
            "$pull" => {
                if let Bson::Document(fields) = op_value {
                    for (key, pull_value) in fields.iter() {
                        if let Some(Bson::Array(arr)) = doc.get_mut(key) {
                            let before_len = arr.len();
                            // Check if pull_value is a document with query operators
                            if let Bson::Document(filter_doc) = pull_value {
                                arr.retain(|v| {
                                    if let Bson::Document(elem_doc) = v {
                                        !matches_filter(elem_doc, filter_doc)
                                    } else {
                                        true
                                    }
                                });
                            } else {
                                arr.retain(|v| v != pull_value);
                            }
                            if arr.len() != before_len {
                                modified = true;
                            }
                        }
                    }
                }
            }
            "$pullAll" => {
                // $pullAll removes all matching values from an array
                if let Bson::Document(fields) = op_value {
                    for (key, values_to_remove) in fields.iter() {
                        if let (Some(Bson::Array(arr)), Bson::Array(remove_arr)) = (doc.get_mut(key), values_to_remove) {
                            let before_len = arr.len();
                            arr.retain(|v| !remove_arr.contains(v));
                            if arr.len() != before_len {
                                modified = true;
                            }
                        }
                    }
                }
            }
            "$addToSet" => {
                if let Bson::Document(fields) = op_value {
                    for (key, add_value) in fields.iter() {
                        // Check for $each modifier
                        if let Bson::Document(add_doc) = add_value {
                            if let Ok(each_values) = add_doc.get_array("$each") {
                                let arr = doc.entry(key.clone()).or_insert_with(|| Bson::Array(Vec::new()));
                                if let Bson::Array(arr) = arr {
                                    for v in each_values {
                                        if !arr.contains(v) {
                                            arr.push(v.clone());
                                            modified = true;
                                        }
                                    }
                                }
                                continue;
                            }
                        }

                        // Simple addToSet
                        if let Some(Bson::Array(arr)) = doc.get_mut(key) {
                            if !arr.contains(add_value) {
                                arr.push(add_value.clone());
                                modified = true;
                            }
                        } else {
                            doc.insert(key.clone(), Bson::Array(vec![add_value.clone()]));
                            modified = true;
                        }
                    }
                }
            }
            "$bit" => {
                // $bit performs bitwise operations: and, or, xor
                if let Bson::Document(fields) = op_value {
                    for (key, bit_ops) in fields.iter() {
                        if let Bson::Document(ops) = bit_ops {
                            if let Some(current) = doc.get_mut(key) {
                                for (bit_op, bit_value) in ops.iter() {
                                    let current_num = match current {
                                        Bson::Int32(n) => *n as i64,
                                        Bson::Int64(n) => *n,
                                        _ => continue,
                                    };
                                    let operand = match bit_value {
                                        Bson::Int32(n) => *n as i64,
                                        Bson::Int64(n) => *n,
                                        _ => continue,
                                    };
                                    let result = match bit_op.as_str() {
                                        "and" => current_num & operand,
                                        "or" => current_num | operand,
                                        "xor" => current_num ^ operand,
                                        _ => continue,
                                    };
                                    *current = Bson::Int64(result);
                                    modified = true;
                                }
                            }
                        }
                    }
                }
            }
            "$setOnInsert" => {
                // $setOnInsert only sets values during upsert when creating a new document
                // In regular updates, this is a no-op
                // The actual implementation would be in the upsert logic
            }
            "$rename" => {
                if let Bson::Document(fields) = op_value {
                    for (old_key, new_key) in fields.iter() {
                        if let Bson::String(new_key_str) = new_key {
                            if let Some(value) = doc.remove(old_key) {
                                doc.insert(new_key_str.clone(), value);
                                modified = true;
                            }
                        }
                    }
                }
            }
            "$min" => {
                if let Bson::Document(fields) = op_value {
                    for (key, min_value) in fields.iter() {
                        if let Some(current) = doc.get(key) {
                            if compare_bson(&Some(min_value.clone()), current, |a, b| a < b) {
                                doc.insert(key.clone(), min_value.clone());
                                modified = true;
                            }
                        } else {
                            doc.insert(key.clone(), min_value.clone());
                            modified = true;
                        }
                    }
                }
            }
            "$max" => {
                if let Bson::Document(fields) = op_value {
                    for (key, max_value) in fields.iter() {
                        if let Some(current) = doc.get(key) {
                            if compare_bson(&Some(max_value.clone()), current, |a, b| a > b) {
                                doc.insert(key.clone(), max_value.clone());
                                modified = true;
                            }
                        } else {
                            doc.insert(key.clone(), max_value.clone());
                            modified = true;
                        }
                    }
                }
            }
            "$currentDate" => {
                if let Bson::Document(fields) = op_value {
                    for (key, type_spec) in fields.iter() {
                        let value = match type_spec {
                            Bson::Boolean(true) => Bson::DateTime(bson::DateTime::now()),
                            Bson::Document(d) if d.get_str("$type").ok() == Some("timestamp") => {
                                Bson::Timestamp(bson::Timestamp {
                                    time: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs() as u32,
                                    increment: 0,
                                })
                            }
                            _ => Bson::DateTime(bson::DateTime::now()),
                        };
                        doc.insert(key.clone(), value);
                        modified = true;
                    }
                }
            }
            // If no operator prefix, treat as replacement (except for _id)
            _ if !op.starts_with('$') => {
                // This is a replacement update, replace entire document except _id
                let id = doc.get("_id").cloned();
                doc.clear();
                if let Some(id) = id {
                    doc.insert("_id", id);
                }
                for (key, value) in update.iter() {
                    if key != "_id" {
                        doc.insert(key.clone(), value.clone());
                    }
                }
                return true;
            }
            _ => {}
        }
    }

    modified
}

/// Set a nested value in a document using dot notation
fn set_nested_value(doc: &mut Document, key: &str, value: Bson) {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() == 1 {
        doc.insert(key.to_string(), value);
    } else {
        // Handle nested documents
        let first = parts[0];
        let rest = parts[1..].join(".");

        if !doc.contains_key(first) {
            doc.insert(first.to_string(), Bson::Document(Document::new()));
        }

        if let Some(Bson::Document(nested)) = doc.get_mut(first) {
            set_nested_value(nested, &rest, value);
        }
    }
}

/// MongoDB database containing collections
#[derive(Default)]
pub struct Database {
    collections: HashMap<String, Collection>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            collections: HashMap::new(),
        }
    }

    pub fn get_or_create_collection(&mut self, name: &str) -> &mut Collection {
        self.collections
            .entry(name.to_string())
            .or_insert_with(Collection::new)
    }

    pub fn get_collection(&self, name: &str) -> Option<&Collection> {
        self.collections.get(name)
    }

    pub fn get_collection_mut(&mut self, name: &str) -> Option<&mut Collection> {
        self.collections.get_mut(name)
    }

    pub fn list_collections(&self) -> Vec<String> {
        self.collections.keys().cloned().collect()
    }

    pub fn drop_collection(&mut self, name: &str) -> bool {
        self.collections.remove(name).is_some()
    }
}

/// MongoDB document store - manages multiple databases
pub struct DocumentStore {
    databases: Arc<RwLock<HashMap<String, Database>>>,
    next_cursor_id: AtomicU64,
    cursors: Arc<RwLock<HashMap<i64, CursorState>>>,
}

/// State for an open cursor
pub struct CursorState {
    pub db: String,
    pub collection: String,
    pub documents: Vec<Document>,
    pub position: usize,
    pub batch_size: usize,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            databases: Arc::new(RwLock::new(HashMap::new())),
            next_cursor_id: AtomicU64::new(1),
            cursors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_or_create_database(&self, name: &str) -> &Self {
        let mut dbs = self.databases.write().await;
        dbs.entry(name.to_string()).or_insert_with(Database::new);
        self
    }

    pub async fn insert_one(
        &self,
        db: &str,
        collection: &str,
        doc: Document,
    ) -> Result<Bson, String> {
        let mut dbs = self.databases.write().await;
        let database = dbs.entry(db.to_string()).or_insert_with(Database::new);
        let coll = database.get_or_create_collection(collection);
        coll.insert_one(doc)
    }

    pub async fn insert_many(
        &self,
        db: &str,
        collection: &str,
        docs: Vec<Document>,
    ) -> Result<Vec<Bson>, String> {
        let mut dbs = self.databases.write().await;
        let database = dbs.entry(db.to_string()).or_insert_with(Database::new);
        let coll = database.get_or_create_collection(collection);
        coll.insert_many(docs)
    }

    pub async fn find(
        &self,
        db: &str,
        collection: &str,
        filter: &Document,
        limit: Option<i64>,
        skip: Option<i64>,
        batch_size: Option<i32>,
    ) -> (i64, Vec<Document>) {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                let all_docs = coll.find(filter, None, skip);
                let total = all_docs.len();

                // If batch_size specified and we have more docs, create a cursor
                let batch = batch_size.unwrap_or(101) as usize;
                let limit_val = limit.unwrap_or(i64::MAX) as usize;
                let actual_limit = batch.min(limit_val).min(total);

                if total > actual_limit {
                    // Create cursor for remaining docs
                    let cursor_id = self.next_cursor_id.fetch_add(1, Ordering::SeqCst) as i64;
                    let mut cursors = self.cursors.write().await;
                    cursors.insert(
                        cursor_id,
                        CursorState {
                            db: db.to_string(),
                            collection: collection.to_string(),
                            documents: all_docs[actual_limit..].to_vec(),
                            position: 0,
                            batch_size: batch,
                        },
                    );
                    return (cursor_id, all_docs[..actual_limit].to_vec());
                }

                let result_docs: Vec<Document> = all_docs.into_iter().take(actual_limit).collect();
                return (0, result_docs);
            }
        }
        (0, vec![])
    }

    pub async fn get_more(
        &self,
        cursor_id: i64,
        batch_size: Option<i32>,
    ) -> Option<(i64, Vec<Document>)> {
        let mut cursors = self.cursors.write().await;
        if let Some(cursor) = cursors.get_mut(&cursor_id) {
            let batch = batch_size.unwrap_or(cursor.batch_size as i32) as usize;
            let remaining = cursor.documents.len() - cursor.position;

            if remaining == 0 {
                cursors.remove(&cursor_id);
                return Some((0, vec![]));
            }

            let end = (cursor.position + batch).min(cursor.documents.len());
            let docs = cursor.documents[cursor.position..end].to_vec();
            cursor.position = end;

            let new_cursor_id = if cursor.position >= cursor.documents.len() {
                cursors.remove(&cursor_id);
                0
            } else {
                cursor_id
            };

            return Some((new_cursor_id, docs));
        }
        None
    }

    pub async fn find_one(
        &self,
        db: &str,
        collection: &str,
        filter: &Document,
    ) -> Option<Document> {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                return coll.find_one(filter);
            }
        }
        None
    }

    pub async fn update_one(
        &self,
        db: &str,
        collection: &str,
        filter: &Document,
        update: &Document,
    ) -> (i64, i64) {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.update_one(filter, update);
            }
        }
        (0, 0)
    }

    pub async fn update_many(
        &self,
        db: &str,
        collection: &str,
        filter: &Document,
        update: &Document,
    ) -> (i64, i64) {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.update_many(filter, update);
            }
        }
        (0, 0)
    }

    pub async fn delete_one(&self, db: &str, collection: &str, filter: &Document) -> i64 {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.delete_one(filter);
            }
        }
        0
    }

    pub async fn delete_many(&self, db: &str, collection: &str, filter: &Document) -> i64 {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.delete_many(filter);
            }
        }
        0
    }

    pub async fn count(&self, db: &str, collection: &str, filter: &Document) -> i64 {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                return coll.count(filter);
            }
        }
        0
    }

    pub async fn list_databases(&self) -> Vec<Document> {
        let dbs = self.databases.read().await;
        dbs.keys()
            .map(|name| {
                doc! {
                    "name": name,
                    "sizeOnDisk": 1024i64,
                    "empty": false,
                }
            })
            .collect()
    }

    pub async fn list_collections(&self, db: &str) -> Vec<Document> {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            return database
                .list_collections()
                .into_iter()
                .map(|name| {
                    doc! {
                        "name": name,
                        "type": "collection",
                    }
                })
                .collect();
        }
        vec![]
    }

    pub async fn create_index(
        &self,
        db: &str,
        collection: &str,
        keys: Document,
        name: Option<String>,
        unique: bool,
        sparse: bool,
    ) -> String {
        let mut dbs = self.databases.write().await;
        let database = dbs.entry(db.to_string()).or_insert_with(Database::new);
        let coll = database.get_or_create_collection(collection);
        coll.create_index(keys, name, unique, sparse)
    }

    pub async fn list_indexes(&self, db: &str, collection: &str) -> Vec<Document> {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                return coll.list_indexes();
            }
        }
        vec![]
    }

    pub async fn drop_collection(&self, db: &str, collection: &str) -> bool {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            return database.drop_collection(collection);
        }
        false
    }

    pub async fn drop_database(&self, db: &str) -> bool {
        let mut dbs = self.databases.write().await;
        dbs.remove(db).is_some()
    }

    pub async fn find_and_modify(
        &self,
        db: &str,
        collection: &str,
        query: &Document,
        sort: Option<&Document>,
        update: Option<&Document>,
        remove: bool,
        new_doc: bool,
        upsert: bool,
    ) -> Option<Document> {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.find_and_modify(query, sort, update, remove, new_doc, upsert);
            }
        }
        None
    }

    pub async fn distinct(
        &self,
        db: &str,
        collection: &str,
        key: &str,
        query: Option<&Document>,
    ) -> Vec<Bson> {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                return coll.distinct(key, query);
            }
        }
        vec![]
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_find() {
        let mut coll = Collection::new();

        let doc = doc! {
            "name": "Alice",
            "age": 30,
            "city": "NYC"
        };

        let id = coll.insert_one(doc).unwrap();
        assert!(matches!(id, Bson::ObjectId(_)));

        let found = coll.find(&doc! { "name": "Alice" }, None, None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].get_str("name").unwrap(), "Alice");
    }

    #[test]
    fn test_update_operators() {
        let mut coll = Collection::new();

        coll.insert_one(doc! { "name": "Bob", "score": 100 })
            .unwrap();

        let (matched, modified) =
            coll.update_one(&doc! { "name": "Bob" }, &doc! { "$inc": { "score": 50 } });
        assert_eq!(matched, 1);
        assert_eq!(modified, 1);

        let found = coll.find_one(&doc! { "name": "Bob" }).unwrap();
        assert_eq!(found.get_i32("score").unwrap(), 150);
    }

    #[test]
    fn test_delete() {
        let mut coll = Collection::new();

        coll.insert_one(doc! { "x": 1 }).unwrap();
        coll.insert_one(doc! { "x": 2 }).unwrap();
        coll.insert_one(doc! { "x": 3 }).unwrap();

        let deleted = coll.delete_one(&doc! { "x": 2 });
        assert_eq!(deleted, 1);
        assert_eq!(coll.count(&doc! {}), 2);

        let deleted = coll.delete_many(&doc! {});
        assert_eq!(deleted, 2);
        assert_eq!(coll.count(&doc! {}), 0);
    }

    #[test]
    fn test_filter_operators() {
        let mut coll = Collection::new();

        coll.insert_one(doc! { "value": 10 }).unwrap();
        coll.insert_one(doc! { "value": 20 }).unwrap();
        coll.insert_one(doc! { "value": 30 }).unwrap();

        // $gt
        let found = coll.find(&doc! { "value": { "$gt": 15 } }, None, None);
        assert_eq!(found.len(), 2);

        // $lte
        let found = coll.find(&doc! { "value": { "$lte": 20 } }, None, None);
        assert_eq!(found.len(), 2);

        // $in
        let found = coll.find(&doc! { "value": { "$in": [10, 30] } }, None, None);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_logical_operators() {
        let mut coll = Collection::new();

        coll.insert_one(doc! { "a": 1, "b": 2 }).unwrap();
        coll.insert_one(doc! { "a": 3, "b": 4 }).unwrap();

        // $and
        let found = coll.find(&doc! { "$and": [{ "a": 1 }, { "b": 2 }] }, None, None);
        assert_eq!(found.len(), 1);

        // $or
        let found = coll.find(&doc! { "$or": [{ "a": 1 }, { "a": 3 }] }, None, None);
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn test_document_store() {
        let store = DocumentStore::new();

        // Insert
        let id = store
            .insert_one("testdb", "users", doc! { "name": "Charlie" })
            .await
            .unwrap();
        assert!(matches!(id, Bson::ObjectId(_)));

        // Find
        let (cursor_id, docs) = store
            .find("testdb", "users", &doc! {}, None, None, None)
            .await;
        assert_eq!(cursor_id, 0);
        assert_eq!(docs.len(), 1);

        // Count
        let count = store.count("testdb", "users", &doc! {}).await;
        assert_eq!(count, 1);

        // Update
        let (matched, modified) = store
            .update_one(
                "testdb",
                "users",
                &doc! { "name": "Charlie" },
                &doc! { "$set": { "age": 25 } },
            )
            .await;
        assert_eq!(matched, 1);
        assert_eq!(modified, 1);

        // Delete
        let deleted = store
            .delete_one("testdb", "users", &doc! { "name": "Charlie" })
            .await;
        assert_eq!(deleted, 1);
    }

    // ============================================================================
    // EXPRESSION OPERATOR TESTS
    // ============================================================================

    // Helper function to evaluate expressions in aggregation context
    use crate::protocols::mongodb::server::evaluate_expression;

    // Helper to convert doc! macro result to Bson for evaluate_expression
    fn eval_expr(expr: Document, doc: &Document) -> Bson {
        evaluate_expression(&Bson::Document(expr), doc)
    }

    // Trigonometric Operators Tests (20 tests)
    
    #[test]
    fn test_trig_sin() {
        let doc = doc! { "angle": 0.0 };
        let result = eval_expr(doc! { "$sin": "$angle" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
        
        let doc2 = doc! { "angle": std::f64::consts::PI / 2.0 };
        let result2 = eval_expr(doc! { "$sin": "$angle" }, &doc2);
        assert!((bson_to_f64(&result2).unwrap() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_trig_cos() {
        let doc = doc! { "angle": 0.0 };
        let result = eval_expr(doc! { "$cos": "$angle" }, &doc);
        assert_eq!(result, Bson::Double(1.0));
        
        let doc2 = doc! { "angle": std::f64::consts::PI };
        let result2 = eval_expr(doc! { "$cos": "$angle" }, &doc2);
        assert!((bson_to_f64(&result2).unwrap() + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_trig_tan() {
        let doc = doc! { "angle": 0.0 };
        let result = eval_expr(doc! { "$tan": "$angle" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
        
        let doc2 = doc! { "angle": std::f64::consts::PI / 4.0 };
        let result2 = eval_expr(doc! { "$tan": "$angle" }, &doc2);
        assert!((bson_to_f64(&result2).unwrap() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_trig_asin() {
        let doc = doc! { "value": 0.5 };
        let result = eval_expr(doc! { "$asin": "$value" }, &doc);
        assert!((bson_to_f64(&result).unwrap() - (std::f64::consts::PI / 6.0)).abs() < 0.0001);
        
        // Test domain error
        let doc2 = doc! { "value": 2.0 };
        let result2 = eval_expr(doc! { "$asin": "$value" }, &doc2);
        assert_eq!(result2, Bson::Null);
    }

    #[test]
    fn test_trig_acos() {
        let doc = doc! { "value": 0.5 };
        let result = eval_expr(doc! { "$acos": "$value" }, &doc);
        assert!((bson_to_f64(&result).unwrap() - (std::f64::consts::PI / 3.0)).abs() < 0.0001);
        
        // Test domain error
        let doc2 = doc! { "value": -2.0 };
        let result2 = eval_expr(doc! { "$acos": "$value" }, &doc2);
        assert_eq!(result2, Bson::Null);
    }

    #[test]
    fn test_trig_atan() {
        let doc = doc! { "value": 1.0 };
        let result = eval_expr(doc! { "$atan": "$value" }, &doc);
        assert!((bson_to_f64(&result).unwrap() - (std::f64::consts::PI / 4.0)).abs() < 0.0001);
    }

    #[test]
    fn test_trig_atan2() {
        let doc = doc! { "y": 1.0, "x": 1.0 };
        let result = eval_expr(doc! { "$atan2": ["$y", "$x"] }, &doc);
        assert!((bson_to_f64(&result).unwrap() - (std::f64::consts::PI / 4.0)).abs() < 0.0001);
    }

    #[test]
    fn test_trig_sinh() {
        let doc = doc! { "value": 0.0 };
        let result = eval_expr(doc! { "$sinh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
    }

    #[test]
    fn test_trig_cosh() {
        let doc = doc! { "value": 0.0 };
        let result = eval_expr(doc! { "$cosh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(1.0));
    }

    #[test]
    fn test_trig_tanh() {
        let doc = doc! { "value": 0.0 };
        let result = eval_expr(doc! { "$tanh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
    }

    #[test]
    fn test_trig_asinh() {
        let doc = doc! { "value": 0.0 };
        let result = eval_expr(doc! { "$asinh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
    }

    #[test]
    fn test_trig_acosh() {
        let doc = doc! { "value": 1.0 };
        let result = eval_expr(doc! { "$acosh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
        
        // Test domain error
        let doc2 = doc! { "value": 0.5 };
        let result2 = eval_expr(doc! { "$acosh": "$value" }, &doc2);
        assert_eq!(result2, Bson::Null);
    }

    #[test]
    fn test_trig_atanh() {
        let doc = doc! { "value": 0.0 };
        let result = eval_expr(doc! { "$atanh": "$value" }, &doc);
        assert_eq!(result, Bson::Double(0.0));
        
        // Test domain error
        let doc2 = doc! { "value": 1.5 };
        let result2 = eval_expr(doc! { "$atanh": "$value" }, &doc2);
        assert_eq!(result2, Bson::Null);
    }

    #[test]
    fn test_trig_degrees_to_radians() {
        let doc = doc! { "degrees": 180.0 };
        let result = eval_expr(doc! { "$degreesToRadians": "$degrees" }, &doc);
        assert!((bson_to_f64(&result).unwrap() - std::f64::consts::PI).abs() < 0.0001);
    }

    #[test]
    fn test_trig_radians_to_degrees() {
        let doc = doc! { "radians": std::f64::consts::PI };
        let result = eval_expr(doc! { "$radiansToDegrees": "$radians" }, &doc);
        assert!((bson_to_f64(&result).unwrap() - 180.0).abs() < 0.0001);
    }

    #[test]
    fn test_trig_null_handling() {
        let doc = doc! { "value": Bson::Null };
        let result = eval_expr(doc! { "$sin": "$value" }, &doc);
        assert_eq!(result, Bson::Null);
    }

    #[test]
    fn test_trig_combined() {
        // Test sin^2 + cos^2 = 1
        let doc = doc! { "angle": 0.5 };
        let sin_result = eval_expr(doc! { "$sin": "$angle" }, &doc);
        let cos_result = eval_expr(doc! { "$cos": "$angle" }, &doc);
        
        let sin_val = bson_to_f64(&sin_result).unwrap();
        let cos_val = bson_to_f64(&cos_result).unwrap();
        assert!((sin_val * sin_val + cos_val * cos_val - 1.0).abs() < 0.0001);
    }

    // Array Operators Tests (15 tests)

    #[test]
    fn test_index_of_array_basic() {
        let doc = doc! { "arr": ["a", "b", "c", "d"] };
        let result = eval_expr(doc! { "$indexOfArray": ["$arr", "c"] }, &doc);
        assert_eq!(result, Bson::Int32(2));
    }

    #[test]
    fn test_index_of_array_not_found() {
        let doc = doc! { "arr": ["a", "b", "c"] };
        let result = eval_expr(doc! { "$indexOfArray": ["$arr", "z"] }, &doc);
        assert_eq!(result, Bson::Int32(-1));
    }

    #[test]
    fn test_index_of_array_with_range() {
        let doc = doc! { "arr": ["a", "b", "c", "b", "d"] };
        let result = eval_expr(doc! { "$indexOfArray": ["$arr", "b", 2] }, &doc);
        assert_eq!(result, Bson::Int32(3));
    }

    #[test]
    fn test_index_of_array_with_end() {
        let doc = doc! { "arr": ["a", "b", "c", "b", "d"] };
        let result = eval_expr(doc! { "$indexOfArray": ["$arr", "b", 0, 2] }, &doc);
        assert_eq!(result, Bson::Int32(1));
    }

    #[test]
    fn test_index_of_array_empty() {
        let doc = doc! { "arr": [] };
        let result = eval_expr(doc! { "$indexOfArray": ["$arr", "a"] }, &doc);
        assert_eq!(result, Bson::Int32(-1));
    }

    #[test]
    fn test_zip_basic() {
        let doc = doc! { "arr1": [1, 2, 3], "arr2": ["a", "b", "c"] };
        let result = eval_expr(doc! { "$zip": { "inputs": ["$arr1", "$arr2"] } },
            &doc
        );
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Bson::Array(vec![Bson::Int32(1), Bson::String("a".to_string())]));
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_zip_different_lengths() {
        let doc = doc! { "arr1": [1, 2], "arr2": ["a", "b", "c"] };
        let result = eval_expr(doc! { "$zip": { "inputs": ["$arr1", "$arr2"] } },
            &doc
        );
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 2); // Shortest length
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_zip_use_longest() {
        let doc = doc! { "arr1": [1, 2], "arr2": ["a", "b", "c"] };
        let result = eval_expr(doc! { "$zip": { "inputs": ["$arr1", "$arr2"], "useLongestLength": true } },
            &doc
        );
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 3); // Longest length
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_zip_with_defaults() {
        let doc = doc! { "arr1": [1, 2], "arr2": ["a", "b", "c"] };
        let result = eval_expr(doc! { 
                "$zip": { 
                    "inputs": ["$arr1", "$arr2"], 
                    "useLongestLength": true,
                    "defaults": [0, "x"]
                } 
            },
            &doc
        );
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            if let Bson::Array(last) = &arr[2] {
                assert_eq!(last[0], Bson::Int32(0)); // Default for arr1
            }
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_range_basic() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$range": [0, 5] }, &doc);
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 5);
            assert_eq!(arr, vec![
                Bson::Int32(0), Bson::Int32(1), Bson::Int32(2), 
                Bson::Int32(3), Bson::Int32(4)
            ]);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_range_with_step() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$range": [0, 10, 2] }, &doc);
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr, vec![
                Bson::Int32(0), Bson::Int32(2), Bson::Int32(4), 
                Bson::Int32(6), Bson::Int32(8)
            ]);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_range_negative_step() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$range": [10, 0, -2] }, &doc);
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr, vec![
                Bson::Int32(10), Bson::Int32(8), Bson::Int32(6), 
                Bson::Int32(4), Bson::Int32(2)
            ]);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_range_zero_step() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$range": [0, 5, 0] }, &doc);
        assert_eq!(result, Bson::Null);
    }

    #[test]
    fn test_range_empty() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$range": [5, 5] }, &doc);
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 0);
        } else {
            panic!("Expected array result");
        }
    }

    // String Operators Tests (15 tests)

    #[test]
    fn test_index_of_bytes_basic() {
        let doc = doc! { "str": "hello world" };
        let result = eval_expr(doc! { "$indexOfBytes": ["$str", "world"] }, &doc);
        assert_eq!(result, Bson::Int32(6));
    }

    #[test]
    fn test_index_of_bytes_not_found() {
        let doc = doc! { "str": "hello" };
        let result = eval_expr(doc! { "$indexOfBytes": ["$str", "xyz"] }, &doc);
        assert_eq!(result, Bson::Int32(-1));
    }

    #[test]
    fn test_index_of_bytes_with_start() {
        let doc = doc! { "str": "hello hello" };
        let result = eval_expr(doc! { "$indexOfBytes": ["$str", "hello", 1] }, &doc);
        assert_eq!(result, Bson::Int32(6));
    }

    #[test]
    fn test_index_of_cp_basic() {
        let doc = doc! { "str": "hello world" };
        let result = eval_expr(doc! { "$indexOfCP": ["$str", "world"] }, &doc);
        assert_eq!(result, Bson::Int32(6));
    }

    #[test]
    fn test_index_of_cp_unicode() {
        let doc = doc! { "str": "café" };
        let result = eval_expr(doc! { "$indexOfCP": ["$str", "é"] }, &doc);
        assert_eq!(result, Bson::Int32(3));
    }

    #[test]
    fn test_index_of_cp_not_found() {
        let doc = doc! { "str": "hello" };
        let result = eval_expr(doc! { "$indexOfCP": ["$str", "xyz"] }, &doc);
        assert_eq!(result, Bson::Int32(-1));
    }

    #[test]
    fn test_strcasecmp_equal() {
        let doc = doc! { "str1": "Hello", "str2": "hello" };
        let result = eval_expr(doc! { "$strcasecmp": ["$str1", "$str2"] }, &doc);
        assert_eq!(result, Bson::Int32(0));
    }

    #[test]
    fn test_strcasecmp_less() {
        let doc = doc! { "str1": "apple", "str2": "BANANA" };
        let result = eval_expr(doc! { "$strcasecmp": ["$str1", "$str2"] }, &doc);
        assert_eq!(result, Bson::Int32(-1));
    }

    #[test]
    fn test_strcasecmp_greater() {
        let doc = doc! { "str1": "zebra", "str2": "APPLE" };
        let result = eval_expr(doc! { "$strcasecmp": ["$str1", "$str2"] }, &doc);
        assert_eq!(result, Bson::Int32(1));
    }

    #[test]
    fn test_substr_cp_basic() {
        let doc = doc! { "str": "hello world" };
        let result = eval_expr(doc! { "$substrCP": ["$str", 0, 5] }, &doc);
        assert_eq!(result, Bson::String("hello".to_string()));
    }

    #[test]
    fn test_substr_cp_middle() {
        let doc = doc! { "str": "hello world" };
        let result = eval_expr(doc! { "$substrCP": ["$str", 6, 5] }, &doc);
        assert_eq!(result, Bson::String("world".to_string()));
    }

    #[test]
    fn test_substr_cp_unicode() {
        let doc = doc! { "str": "café" };
        let result = eval_expr(doc! { "$substrCP": ["$str", 0, 3] }, &doc);
        assert_eq!(result, Bson::String("caf".to_string()));
    }

    #[test]
    fn test_substr_cp_out_of_bounds() {
        let doc = doc! { "str": "hello" };
        let result = eval_expr(doc! { "$substrCP": ["$str", 10, 5] }, &doc);
        assert_eq!(result, Bson::String("".to_string()));
    }

    #[test]
    fn test_substr_cp_zero_length() {
        let doc = doc! { "str": "hello" };
        let result = eval_expr(doc! { "$substrCP": ["$str", 0, 0] }, &doc);
        assert_eq!(result, Bson::String("".to_string()));
    }

    #[test]
    fn test_string_operators_null_handling() {
        let doc = doc! { "str": Bson::Null };
        let result = eval_expr(doc! { "$indexOfBytes": ["$str", "test"] }, &doc);
        assert_eq!(result, Bson::Null);
    }

    // Date Operators Tests (30 tests) - Simplified for brevity, covering key scenarios

    #[test]
    fn test_date_from_parts_basic() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$dateFromParts": { "year": 2024, "month": 1, "day": 15 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_from_parts_with_time() {
        let doc = doc! {};
        let result = eval_expr(doc! { 
                "$dateFromParts": { 
                    "year": 2024, "month": 1, "day": 15,
                    "hour": 10, "minute": 30, "second": 45
                } 
            },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_from_string_iso() {
        let doc = doc! {};
        let result = eval_expr(doc! { "$dateFromString": { "dateString": "2024-01-15T10:30:00Z" } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_to_parts() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateToParts": { "date": "$date" } }, &doc);
        
        if let Bson::Document(parts) = result {
            assert!(parts.contains_key("year"));
            assert!(parts.contains_key("month"));
            assert!(parts.contains_key("day"));
        } else {
            panic!("Expected document result");
        }
    }

    #[test]
    fn test_date_add_days() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateAdd": { "startDate": "$date", "unit": "day", "amount": 7 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_add_months() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateAdd": { "startDate": "$date", "unit": "month", "amount": 3 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_subtract_days() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateSubtract": { "startDate": "$date", "unit": "day", "amount": 7 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_diff_days() {
        use bson::DateTime;
        let dt1 = DateTime::now();
        let dt2 = DateTime::now();
        let doc = doc! { "date1": dt1, "date2": dt2 };
        let result = eval_expr(doc! { "$dateDiff": { "startDate": "$date1", "endDate": "$date2", "unit": "day" } },
            &doc
        );
        assert!(matches!(result, Bson::Int64(_)));
    }

    #[test]
    fn test_date_trunc_day() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateTrunc": { "date": "$date", "unit": "day" } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_iso_week() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$isoWeek": "$date" }, &doc);
        
        if let Bson::Int32(week) = result {
            assert!(week >= 1 && week <= 53);
        } else {
            panic!("Expected Int32 result");
        }
    }

    #[test]
    fn test_iso_week_year() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$isoWeekYear": "$date" }, &doc);
        assert!(matches!(result, Bson::Int32(_)));
    }

    #[test]
    fn test_iso_day_of_week() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$isoDayOfWeek": "$date" }, &doc);
        
        if let Bson::Int32(day) = result {
            assert!(day >= 1 && day <= 7);
        } else {
            panic!("Expected Int32 result");
        }
    }

    #[test]
    fn test_millisecond() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$millisecond": "$date" }, &doc);
        
        if let Bson::Int32(ms) = result {
            assert!(ms >= 0 && ms < 1000);
        } else {
            panic!("Expected Int32 result");
        }
    }

    #[test]
    fn test_week() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$week": "$date" }, &doc);
        
        if let Bson::Int32(week) = result {
            assert!(week >= 0 && week <= 53);
        } else {
            panic!("Expected Int32 result");
        }
    }

    // Additional date tests for comprehensive coverage
    #[test]
    fn test_date_add_hours() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateAdd": { "startDate": "$date", "unit": "hour", "amount": 24 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_add_minutes() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateAdd": { "startDate": "$date", "unit": "minute", "amount": 60 } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_diff_hours() {
        use bson::DateTime;
        let dt1 = DateTime::now();
        let dt2 = DateTime::now();
        let doc = doc! { "date1": dt1, "date2": dt2 };
        let result = eval_expr(doc! { "$dateDiff": { "startDate": "$date1", "endDate": "$date2", "unit": "hour" } },
            &doc
        );
        assert!(matches!(result, Bson::Int64(_)));
    }

    #[test]
    fn test_date_trunc_month() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateTrunc": { "date": "$date", "unit": "month" } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_trunc_year() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        let result = eval_expr(doc! { "$dateTrunc": { "date": "$date", "unit": "year" } },
            &doc
        );
        assert!(matches!(result, Bson::DateTime(_)));
    }

    #[test]
    fn test_date_operators_null_handling() {
        let doc = doc! { "date": Bson::Null };
        let result = eval_expr(doc! { "$isoWeek": "$date" }, &doc);
        assert_eq!(result, Bson::Null);
    }

    // findAndModify Tests (15 tests)

    #[test]
    fn test_find_and_modify_update_return_original() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Alice", "score": 100 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Alice" },
            None,
            Some(&doc! { "$inc": { "score": 50 } }),
            false,
            false,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_i32("score").unwrap(), 100); // Original value
        
        // Verify update was applied
        let updated = coll.find_one(&doc! { "name": "Alice" }).unwrap();
        assert_eq!(updated.get_i32("score").unwrap(), 150);
    }

    #[test]
    fn test_find_and_modify_update_return_new() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Bob", "score": 200 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Bob" },
            None,
            Some(&doc! { "$inc": { "score": 50 } }),
            false,
            true,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_i32("score").unwrap(), 250); // New value
    }

    #[test]
    fn test_find_and_modify_remove() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Charlie", "score": 300 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Charlie" },
            None,
            None,
            true,
            false,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "Charlie");
        
        // Verify document was removed
        assert_eq!(coll.count(&doc! { "name": "Charlie" }), 0);
    }

    #[test]
    fn test_find_and_modify_upsert_no_match() {
        let mut coll = Collection::new();
        
        let result = coll.find_and_modify(
            &doc! { "name": "David" },
            None,
            Some(&doc! { "$set": { "score": 400 } }),
            false,
            true,
            true,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "David");
        assert_eq!(doc.get_i32("score").unwrap(), 400);
        
        // Verify document was inserted
        assert_eq!(coll.count(&doc! { "name": "David" }), 1);
    }

    #[test]
    fn test_find_and_modify_with_sort() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "type": "test", "value": 10 }).unwrap();
        coll.insert_one(doc! { "type": "test", "value": 20 }).unwrap();
        coll.insert_one(doc! { "type": "test", "value": 15 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "type": "test" },
            Some(&doc! { "value": -1 }), // Sort descending
            Some(&doc! { "$set": { "modified": true } }),
            false,
            false,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_i32("value").unwrap(), 20); // Highest value
    }

    #[test]
    fn test_find_and_modify_no_match() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Eve" }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Frank" },
            None,
            Some(&doc! { "$set": { "score": 500 } }),
            false,
            false,
            false,
        );
        
        assert!(result.is_none());
    }

    #[test]
    fn test_find_and_modify_upsert_return_original() {
        let mut coll = Collection::new();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Grace" },
            None,
            Some(&doc! { "$set": { "score": 600 } }),
            false,
            false,
            true,
        );
        
        assert!(result.is_none()); // Original was null since we inserted
        assert_eq!(coll.count(&doc! { "name": "Grace" }), 1);
    }

    #[test]
    fn test_find_and_modify_multiple_updates() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Henry", "x": 1, "y": 2 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Henry" },
            None,
            Some(&doc! { "$inc": { "x": 10 }, "$set": { "y": 20 } }),
            false,
            true,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_i32("x").unwrap(), 11);
        assert_eq!(doc.get_i32("y").unwrap(), 20);
    }

    #[test]
    fn test_find_and_modify_empty_collection() {
        let mut coll = Collection::new();
        
        let result = coll.find_and_modify(
            &doc! { "name": "Ivy" },
            None,
            Some(&doc! { "$set": { "score": 700 } }),
            false,
            false,
            false,
        );
        
        assert!(result.is_none());
    }

    #[test]
    fn test_find_and_modify_complex_query() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "status": "active", "priority": 1 }).unwrap();
        coll.insert_one(doc! { "status": "active", "priority": 2 }).unwrap();
        
        let result = coll.find_and_modify(
            &doc! { "status": "active", "priority": { "$gt": 1 } },
            None,
            Some(&doc! { "$set": { "processed": true } }),
            false,
            true,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_bool("processed").unwrap(), true);
    }

    // distinct Tests (10 tests)

    #[test]
    fn test_distinct_simple() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "city": "NYC" }).unwrap();
        coll.insert_one(doc! { "city": "LA" }).unwrap();
        coll.insert_one(doc! { "city": "NYC" }).unwrap();
        coll.insert_one(doc! { "city": "Chicago" }).unwrap();
        
        let values = coll.distinct("city", None);
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_distinct_with_query() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "city": "NYC", "age": 25 }).unwrap();
        coll.insert_one(doc! { "city": "LA", "age": 30 }).unwrap();
        coll.insert_one(doc! { "city": "NYC", "age": 35 }).unwrap();
        coll.insert_one(doc! { "city": "Chicago", "age": 20 }).unwrap();
        
        let values = coll.distinct("city", Some(&doc! { "age": { "$gte": 30 } }));
        assert_eq!(values.len(), 2); // LA and NYC
    }

    #[test]
    fn test_distinct_nested_field() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "address": { "city": "NYC" } }).unwrap();
        coll.insert_one(doc! { "address": { "city": "LA" } }).unwrap();
        coll.insert_one(doc! { "address": { "city": "NYC" } }).unwrap();
        
        let values = coll.distinct("address.city", None);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_distinct_array_values() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "tags": ["a", "b", "c"] }).unwrap();
        coll.insert_one(doc! { "tags": ["b", "c", "d"] }).unwrap();
        coll.insert_one(doc! { "tags": ["a", "d"] }).unwrap();
        
        let values = coll.distinct("tags", None);
        assert_eq!(values.len(), 4); // a, b, c, d
    }

    #[test]
    fn test_distinct_empty_collection() {
        let coll = Collection::new();
        let values = coll.distinct("field", None);
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_distinct_no_matches() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "city": "NYC", "age": 25 }).unwrap();
        
        let values = coll.distinct("city", Some(&doc! { "age": { "$gt": 100 } }));
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_distinct_missing_field() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Alice" }).unwrap();
        coll.insert_one(doc! { "name": "Bob" }).unwrap();
        
        let values = coll.distinct("city", None);
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_distinct_mixed_types() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "value": 1 }).unwrap();
        coll.insert_one(doc! { "value": "1" }).unwrap();
        coll.insert_one(doc! { "value": 1 }).unwrap();
        
        let values = coll.distinct("value", None);
        assert_eq!(values.len(), 2); // Number 1 and string "1"
    }

    #[test]
    fn test_distinct_null_values() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "value": Bson::Null }).unwrap();
        coll.insert_one(doc! { "value": "test" }).unwrap();
        coll.insert_one(doc! { "value": Bson::Null }).unwrap();
        
        let values = coll.distinct("value", None);
        assert_eq!(values.len(), 2); // Null and "test"
    }

    #[test]
    fn test_distinct_single_value() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "city": "NYC" }).unwrap();
        coll.insert_one(doc! { "city": "NYC" }).unwrap();
        coll.insert_one(doc! { "city": "NYC" }).unwrap();
        
        let values = coll.distinct("city", None);
        assert_eq!(values.len(), 1);
    }

    // Integration Tests (10 tests)

    #[test]
    fn test_integration_trig_in_aggregation() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "angle": 0.0 }).unwrap();
        coll.insert_one(doc! { "angle": std::f64::consts::PI / 2.0 }).unwrap();
        
        // This would be tested via aggregation pipeline in real usage
        let doc = coll.find_one(&doc! {}).unwrap();
        let sin_result = eval_expr(doc! { "$sin": "$angle" }, &doc);
        assert!(matches!(sin_result, Bson::Double(_)));
    }

    #[test]
    fn test_integration_array_and_string_ops() {
        let doc = doc! { 
            "items": ["apple", "banana", "cherry"],
            "search": "banana"
        };
        
        let idx = eval_expr(doc! { "$indexOfArray": ["$items", "$search"] }, &doc);
        assert_eq!(idx, Bson::Int32(1));
        
        let substr = eval_expr(doc! { "$substrCP": ["$search", 0, 3] }, &doc);
        assert_eq!(substr, Bson::String("ban".to_string()));
    }

    #[test]
    fn test_integration_date_calculations() {
        use bson::DateTime;
        let dt = DateTime::now();
        let doc = doc! { "date": dt };
        
        // Add 7 days then get day of week
        let future = eval_expr(doc! { "$dateAdd": { "startDate": "$date", "unit": "day", "amount": 7 } },
            &doc
        );
        
        let future_doc = doc! { "date": future };
        let day_of_week = eval_expr(doc! { "$isoDayOfWeek": "$date" }, &future_doc);
        assert!(matches!(day_of_week, Bson::Int32(_)));
    }

    #[test]
    fn test_integration_find_and_modify_with_expressions() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "name": "Test", "score": 100, "bonus": 10 }).unwrap();
        
        // Update using multiple operators
        let result = coll.find_and_modify(
            &doc! { "name": "Test" },
            None,
            Some(&doc! { 
                "$inc": { "score": 50 },
                "$mul": { "bonus": 2 }
            }),
            false,
            true,
            false,
        );
        
        assert!(result.is_some());
        let doc = result.unwrap();
        assert_eq!(doc.get_i32("score").unwrap(), 150);
        assert_eq!(doc.get_i32("bonus").unwrap(), 20);
    }

    #[test]
    fn test_integration_distinct_with_complex_query() {
        let mut coll = Collection::new();
        coll.insert_one(doc! { "category": "A", "value": 10, "active": true }).unwrap();
        coll.insert_one(doc! { "category": "B", "value": 20, "active": true }).unwrap();
        coll.insert_one(doc! { "category": "A", "value": 30, "active": false }).unwrap();
        coll.insert_one(doc! { "category": "C", "value": 40, "active": true }).unwrap();
        
        let values = coll.distinct(
            "category",
            Some(&doc! { "active": true, "value": { "$gte": 15 } })
        );
        assert_eq!(values.len(), 2); // B and C
    }

    #[test]
    fn test_integration_range_with_array_ops() {
        let doc = doc! {};
        let range = eval_expr(doc! { "$range": [0, 5] }, &doc);
        
        let doc_with_range = doc! { "arr": range };
        let idx = eval_expr(doc! { "$indexOfArray": ["$arr", 3] }, &doc_with_range);
        assert_eq!(idx, Bson::Int32(3));
    }

    #[test]
    fn test_integration_zip_with_range() {
        let doc = doc! {};
        let result = eval_expr(doc! { 
                "$zip": { 
                    "inputs": [
                        { "$range": [0, 3] },
                        ["a", "b", "c"]
                    ]
                } 
            },
            &doc
        );
        
        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_integration_string_comparison_and_search() {
        let doc = doc! { "text": "Hello World", "search": "World" };
        
        let cmp = eval_expr(doc! { "$strcasecmp": ["$text", "HELLO WORLD"] }, &doc);
        assert_eq!(cmp, Bson::Int32(0));
        
        let idx = eval_expr(doc! { "$indexOfCP": ["$text", "$search"] }, &doc);
        assert_eq!(idx, Bson::Int32(6));
    }

    #[test]
    fn test_integration_multiple_trig_operations() {
        let doc = doc! { "x": 1.0, "y": 1.0 };
        
        // Calculate angle using atan2
        let angle = eval_expr(doc! { "$atan2": ["$y", "$x"] }, &doc);
        
        // Convert to degrees
        let angle_doc = doc! { "radians": angle };
        let degrees = eval_expr(doc! { "$radiansToDegrees": "$radians" }, &angle_doc);
        
        if let Bson::Double(deg) = degrees {
            assert!((deg - 45.0).abs() < 0.0001);
        } else {
            panic!("Expected double result");
        }
    }

    #[test]
    fn test_integration_comprehensive_document_operations() {
        let mut coll = Collection::new();
        
        // Insert with generated ID
        let id = coll.insert_one(doc! { 
            "name": "Integration Test",
            "tags": ["test", "integration", "mongodb"],
            "created": bson::DateTime::now(),
            "score": 0
        }).unwrap();
        
        // Find and modify
        let updated = coll.find_and_modify(
            &doc! { "_id": id.clone() },
            None,
            Some(&doc! { "$inc": { "score": 100 } }),
            false,
            true,
            false,
        );
        
        assert!(updated.is_some());
        
        // Get distinct tags
        let tags = coll.distinct("tags", None);
        assert_eq!(tags.len(), 3);
        
        // Remove document
        let removed = coll.find_and_modify(
            &doc! { "_id": id },
            None,
            None,
            true,
            false,
            false,
        );
        
        assert!(removed.is_some());
        assert_eq!(coll.count(&doc! {}), 0);
    }

    // Helper function for tests
    fn bson_to_f64(bson: &Bson) -> Option<f64> {
        match bson {
            Bson::Double(d) => Some(*d),
            Bson::Int32(i) => Some(*i as f64),
            Bson::Int64(i) => Some(*i as f64),
            _ => None,
        }
    }
}
