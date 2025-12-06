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
    pub fn create_index(&mut self, keys: Document, name: Option<String>, unique: bool, sparse: bool) -> String {
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
                        if let (Some(Bson::String(s)), Bson::String(pattern)) = (doc_value, op_value) {
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
            "$push" => {
                if let Bson::Document(fields) = op_value {
                    for (key, push_value) in fields.iter() {
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
            "$pull" => {
                if let Bson::Document(fields) = op_value {
                    for (key, pull_value) in fields.iter() {
                        if let Some(Bson::Array(arr)) = doc.get_mut(key) {
                            let before_len = arr.len();
                            arr.retain(|v| v != pull_value);
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

    pub async fn insert_one(&self, db: &str, collection: &str, doc: Document) -> Result<Bson, String> {
        let mut dbs = self.databases.write().await;
        let database = dbs.entry(db.to_string()).or_insert_with(Database::new);
        let coll = database.get_or_create_collection(collection);
        coll.insert_one(doc)
    }

    pub async fn insert_many(&self, db: &str, collection: &str, docs: Vec<Document>) -> Result<Vec<Bson>, String> {
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

    pub async fn get_more(&self, cursor_id: i64, batch_size: Option<i32>) -> Option<(i64, Vec<Document>)> {
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

    pub async fn find_one(&self, db: &str, collection: &str, filter: &Document) -> Option<Document> {
        let dbs = self.databases.read().await;
        if let Some(database) = dbs.get(db) {
            if let Some(coll) = database.get_collection(collection) {
                return coll.find_one(filter);
            }
        }
        None
    }

    pub async fn update_one(&self, db: &str, collection: &str, filter: &Document, update: &Document) -> (i64, i64) {
        let mut dbs = self.databases.write().await;
        if let Some(database) = dbs.get_mut(db) {
            if let Some(coll) = database.get_collection_mut(collection) {
                return coll.update_one(filter, update);
            }
        }
        (0, 0)
    }

    pub async fn update_many(&self, db: &str, collection: &str, filter: &Document, update: &Document) -> (i64, i64) {
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

        coll.insert_one(doc! { "name": "Bob", "score": 100 }).unwrap();

        let (matched, modified) = coll.update_one(
            &doc! { "name": "Bob" },
            &doc! { "$inc": { "score": 50 } },
        );
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
            .update_one("testdb", "users", &doc! { "name": "Charlie" }, &doc! { "$set": { "age": 25 } })
            .await;
        assert_eq!(matched, 1);
        assert_eq!(modified, 1);

        // Delete
        let deleted = store.delete_one("testdb", "users", &doc! { "name": "Charlie" }).await;
        assert_eq!(deleted, 1);
    }
}
