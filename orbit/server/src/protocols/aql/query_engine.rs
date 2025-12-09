//! AQL query engine with GraphRAG integration
//!
//! This module provides a complete AQL query engine that includes GraphRAG function support.

// Matrix operations use indexed loops for clarity
#![allow(clippy::needless_range_loop)]

use crate::protocols::aql::aql_parser::{
    AqlClause, AqlCondition, AqlExpression, ComparisonOperator, UpsertAction,
};
use crate::protocols::aql::{
    AqlDocument, AqlGraphRAGEngine, AqlParser, AqlQuery, AqlStorage, AqlValue,
};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_client::OrbitClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// AQL query execution result
#[derive(Debug, Clone)]
pub struct AqlQueryResult {
    /// Result data as AQL values
    pub data: Vec<AqlValue>,
    /// Query execution metadata
    pub metadata: HashMap<String, AqlValue>,
}

/// AQL query engine with GraphRAG function support
pub struct AqlQueryEngine {
    /// AQL parser for standard queries
    parser: AqlParser,
    /// GraphRAG function engine
    graphrag_engine: Option<AqlGraphRAGEngine>,
    /// Storage backend for document operations
    storage: Option<Arc<AqlStorage>>,
}

impl AqlQueryEngine {
    /// Create new AQL query engine
    pub fn new() -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: None,
        }
    }

    /// Create new AQL query engine with storage
    pub fn with_storage(storage: Arc<AqlStorage>) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: Some(storage),
        }
    }

    /// Create new AQL query engine with GraphRAG support
    pub fn new_with_graphrag(orbit_client: OrbitClient) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: Some(AqlGraphRAGEngine::new(orbit_client)),
            storage: None,
        }
    }

    /// Execute an AQL query
    pub async fn execute_query(&self, aql: &str) -> ProtocolResult<AqlQueryResult> {
        // Check if this is a GraphRAG function call
        if self.is_graphrag_function_call(aql) {
            return self.execute_graphrag_function_call(aql).await;
        }

        // Parse and execute regular AQL query
        let parsed_query = self.parser.parse(aql)?;
        self.execute_parsed_query(parsed_query).await
    }

    /// Check if query contains GraphRAG function calls
    fn is_graphrag_function_call(&self, aql: &str) -> bool {
        let aql_upper = aql.to_uppercase();
        aql_upper.contains("GRAPHRAG_BUILD_KNOWLEDGE(")
            || aql_upper.contains("GRAPHRAG_EXTRACT_ENTITIES(")
            || aql_upper.contains("GRAPHRAG_QUERY(")
            || aql_upper.contains("GRAPHRAG_FIND_PATHS(")
            || aql_upper.contains("GRAPHRAG_FIND_SIMILAR(")
            || aql_upper.contains("GRAPHRAG_SEMANTIC_SEARCH(")
            || aql_upper.contains("GRAPHRAG_GET_STATS(")
            || aql_upper.contains("GRAPHRAG_LIST_ENTITIES(")
            || aql_upper.contains("GRAPHRAG_ANALYZE_TRENDS(")
            || aql_upper.contains("GRAPHRAG_DETECT_COMMUNITIES(")
    }

    /// Execute GraphRAG function call
    async fn execute_graphrag_function_call(&self, aql: &str) -> ProtocolResult<AqlQueryResult> {
        let graphrag_engine = self.graphrag_engine.as_ref().ok_or_else(|| {
            ProtocolError::AqlError(
                "GraphRAG functions require GraphRAG engine integration".to_string(),
            )
        })?;

        // Simple parsing to extract function name and arguments
        // In a real implementation, this would use the full AQL parser
        let (function_name, args) = self.parse_graphrag_function_call(aql)?;

        let result = graphrag_engine
            .execute_graphrag_function(&function_name, &args)
            .await?;

        // Wrap result in query result format
        let mut metadata = HashMap::new();
        metadata.insert(
            "function_called".to_string(),
            AqlValue::String(function_name),
        );
        metadata.insert(
            "execution_time".to_string(),
            AqlValue::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(AqlQueryResult {
            data: vec![result],
            metadata,
        })
    }

    /// Parse GraphRAG function call (simplified implementation)
    fn parse_graphrag_function_call(&self, aql: &str) -> ProtocolResult<(String, Vec<AqlValue>)> {
        // This is a simplified implementation
        // In practice, this would use a proper AQL parser to extract function calls

        let aql = aql.trim();
        if let Some(start) = aql.find('(') {
            let function_name = aql[..start]
                .trim()
                .strip_prefix("FOR result IN ")
                .unwrap_or(&aql[..start])
                .trim()
                .to_string();

            // For now, return empty args - a full implementation would parse the arguments
            Ok((function_name, vec![]))
        } else {
            Err(ProtocolError::AqlError(
                "Invalid function call syntax".to_string(),
            ))
        }
    }

    /// Execute parsed AQL query
    async fn execute_parsed_query(&self, query: AqlQuery) -> ProtocolResult<AqlQueryResult> {
        // Check if query requires storage (has FOR, INSERT, UPDATE, REMOVE, REPLACE, UPSERT)
        let needs_storage = query.clauses.iter().any(|clause| {
            matches!(
                clause,
                AqlClause::For { .. }
                    | AqlClause::Insert { .. }
                    | AqlClause::Update { .. }
                    | AqlClause::Replace { .. }
                    | AqlClause::Remove { .. }
                    | AqlClause::Upsert { .. }
            )
        });

        // Get storage reference if available, or error if query requires it
        let storage_opt = self.storage.as_ref();
        if needs_storage && storage_opt.is_none() {
            return Err(ProtocolError::AqlError(
                "Storage backend required for query execution".to_string(),
            ));
        }

        // Execution context for variables
        let mut context: HashMap<String, AqlValue> = HashMap::new();
        let mut result_data = Vec::new();

        // Process clauses in order
        let mut for_variable: Option<String> = None;
        let mut for_documents: Vec<AqlDocument> = Vec::new();

        for clause in &query.clauses {
            match clause {
                AqlClause::For {
                    variable,
                    data_source,
                } => {
                    // Execute FOR clause - iterate over collection (storage is guaranteed present)
                    let storage = storage_opt.ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Storage backend required for FOR clause".to_string(),
                        )
                    })?;
                    for_documents = self.execute_for_clause(storage, data_source).await?;
                    for_variable = Some(variable.clone());
                }
                AqlClause::Filter { condition } => {
                    // Apply filter to documents from FOR clause
                    if let Some(ref var) = for_variable {
                        for_documents.retain(|doc| {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            self.evaluate_condition(condition, &ctx).unwrap_or(false)
                        });
                    }
                }
                AqlClause::Return {
                    distinct,
                    expression,
                } => {
                    // Process RETURN clause
                    if let Some(ref var) = for_variable {
                        // Return documents from FOR clause
                        for doc in &for_documents {
                            context.insert(var.clone(), self.document_to_value(doc));
                            let value = self.evaluate_expression(expression, &context)?;
                            result_data.push(value);
                        }
                    } else {
                        // No FOR clause - evaluate expression directly
                        let value = self.evaluate_expression(expression, &context)?;
                        result_data.push(value);
                    }

                    if *distinct {
                        result_data = self.deduplicate_results(result_data);
                    }
                    break; // RETURN is typically the last clause
                }
                AqlClause::Sort { items: _ } => {
                    // Sorting is deferred until after all rows are collected
                    continue;
                }
                AqlClause::Limit {
                    offset: _,
                    count: _,
                } => {
                    // Limiting is deferred until after all rows are collected
                    continue;
                }
                AqlClause::Search {
                    expression,
                    analyzer,
                } => {
                    // Execute SEARCH clause - full-text search on documents
                    if let Some(ref var) = for_variable {
                        let analyzer_name = analyzer.as_deref().unwrap_or("text_en");
                        for_documents.retain(|doc| {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            self.evaluate_search_expression(expression, &ctx, analyzer_name)
                                .unwrap_or(false)
                        });
                    }
                }
                AqlClause::Insert {
                    document,
                    collection,
                    options: _,
                } => {
                    // Execute INSERT clause - create new document
                    // Storage is guaranteed to exist (checked at start of function)
                    let storage = storage_opt.unwrap();
                    let doc_result = self
                        .execute_insert(storage, document, collection, &context)
                        .await?;
                    result_data.push(doc_result);
                }
                AqlClause::Update {
                    key,
                    document,
                    collection,
                    options: _,
                } => {
                    // Execute UPDATE clause - modify existing document
                    // Storage is guaranteed to exist (checked at start of function)
                    let storage = storage_opt.unwrap();
                    if let Some(ref var) = for_variable {
                        // Update based on FOR iteration
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(updated) = self
                                .execute_update(storage, key, document, collection, &ctx)
                                .await
                            {
                                result_data.push(updated);
                            }
                        }
                    } else {
                        // Direct update
                        let updated = self
                            .execute_update(storage, key, document, collection, &context)
                            .await?;
                        result_data.push(updated);
                    }
                }
                AqlClause::Replace {
                    key,
                    document,
                    collection,
                } => {
                    // Execute REPLACE clause - replace entire document
                    // Storage is guaranteed to exist (checked at start of function)
                    let storage = storage_opt.unwrap();
                    if let Some(ref var) = for_variable {
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(replaced) = self
                                .execute_replace(storage, key, document, collection, &ctx)
                                .await
                            {
                                result_data.push(replaced);
                            }
                        }
                    } else {
                        let replaced = self
                            .execute_replace(storage, key, document, collection, &context)
                            .await?;
                        result_data.push(replaced);
                    }
                }
                AqlClause::Remove { key, collection } => {
                    // Execute REMOVE clause - delete document
                    // Storage is guaranteed to exist (checked at start of function)
                    let storage = storage_opt.unwrap();
                    if let Some(ref var) = for_variable {
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(removed) =
                                self.execute_remove(storage, key, collection, &ctx).await
                            {
                                result_data.push(removed);
                            }
                        }
                    } else {
                        let removed = self
                            .execute_remove(storage, key, collection, &context)
                            .await?;
                        result_data.push(removed);
                    }
                }
                AqlClause::Upsert {
                    search,
                    insert,
                    update_or_replace,
                    collection,
                } => {
                    // Execute UPSERT clause - insert or update/replace
                    // Storage is guaranteed to exist (checked at start of function)
                    let storage = storage_opt.unwrap();
                    let upserted = self
                        .execute_upsert(
                            storage,
                            search,
                            insert,
                            update_or_replace,
                            collection,
                            &context,
                        )
                        .await?;
                    result_data.push(upserted);
                }
                AqlClause::Let {
                    variable,
                    expression,
                } => {
                    // Execute LET clause - bind variable to expression result
                    let value = self.evaluate_expression(expression, &context)?;
                    context.insert(variable.clone(), value);
                }
                _ => {
                    // Other clauses not yet implemented
                    warn!("Unsupported clause type in AQL query execution");
                }
            }
        }

        // Apply SORT and LIMIT if present
        result_data = self.apply_sort_and_limit(&query.clauses, result_data)?;

        let mut metadata = HashMap::new();
        metadata.insert(
            "rows_returned".to_string(),
            AqlValue::Number(serde_json::Number::from(result_data.len())),
        );
        metadata.insert(
            "execution_time".to_string(),
            AqlValue::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(AqlQueryResult {
            data: result_data,
            metadata,
        })
    }

    /// Execute FOR clause - get documents from collection
    async fn execute_for_clause(
        &self,
        storage: &AqlStorage,
        collection_name: &str,
    ) -> ProtocolResult<Vec<AqlDocument>> {
        // Get all documents from the collection
        storage.get_collection_documents(collection_name).await
    }

    /// Execute INSERT clause - create a new document
    async fn execute_insert(
        &self,
        storage: &AqlStorage,
        document_expr: &AqlExpression,
        collection: &str,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        // Evaluate the document expression to get the document data
        let doc_value = self.evaluate_expression(document_expr, context)?;

        // Convert AqlValue to document data
        let doc_data = match doc_value {
            AqlValue::Object(obj) => obj,
            _ => {
                return Err(ProtocolError::AqlError(
                    "INSERT expects an object expression".to_string(),
                ))
            }
        };

        // Generate a key if not provided
        let key = if let Some(AqlValue::String(k)) = doc_data.get("_key") {
            k.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        };

        // Create the document (filter out system fields from user data)
        let mut data = HashMap::new();
        for (k, v) in doc_data {
            if !k.starts_with('_') {
                data.insert(k, v);
            }
        }

        let doc = AqlDocument::new(collection, key.clone(), data);

        // Store the document
        storage.store_document(doc.clone()).await?;
        info!("AQL INSERT: Created document {}/{}", collection, key);

        // Return the created document as result
        Ok(self.document_to_value(&doc))
    }

    /// Execute UPDATE clause - modify an existing document
    async fn execute_update(
        &self,
        storage: &AqlStorage,
        key_expr: &AqlExpression,
        document_expr: &AqlExpression,
        collection: &str,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        // Evaluate the key expression
        let key = self.extract_document_key(key_expr, context)?;

        // Evaluate the update expression to get the update data
        let update_value = self.evaluate_expression(document_expr, context)?;

        let updates = match update_value {
            AqlValue::Object(obj) => obj,
            _ => {
                return Err(ProtocolError::AqlError(
                    "UPDATE expects an object expression".to_string(),
                ))
            }
        };

        // Perform the update
        let updated_doc = storage
            .update_document(collection, &key, updates)
            .await?
            .ok_or_else(|| {
                ProtocolError::AqlError(format!(
                    "Document {}/{} not found for UPDATE",
                    collection, key
                ))
            })?;

        info!("AQL UPDATE: Updated document {}/{}", collection, key);
        Ok(self.document_to_value(&updated_doc))
    }

    /// Execute REPLACE clause - replace an entire document
    async fn execute_replace(
        &self,
        storage: &AqlStorage,
        key_expr: &AqlExpression,
        document_expr: &AqlExpression,
        collection: &str,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        // Evaluate the key expression
        let key = self.extract_document_key(key_expr, context)?;

        // Evaluate the replacement document expression
        let replace_value = self.evaluate_expression(document_expr, context)?;

        let new_data = match replace_value {
            AqlValue::Object(obj) => obj,
            _ => {
                return Err(ProtocolError::AqlError(
                    "REPLACE expects an object expression".to_string(),
                ))
            }
        };

        // Delete the old document and create a new one with the same key
        storage.delete_document(collection, &key).await?;

        // Create new document with the replacement data
        let mut data = HashMap::new();
        for (k, v) in new_data {
            if !k.starts_with('_') {
                data.insert(k, v);
            }
        }

        let doc = AqlDocument::new(collection, key.clone(), data);
        storage.store_document(doc.clone()).await?;

        info!("AQL REPLACE: Replaced document {}/{}", collection, key);
        Ok(self.document_to_value(&doc))
    }

    /// Execute REMOVE clause - delete a document
    async fn execute_remove(
        &self,
        storage: &AqlStorage,
        key_expr: &AqlExpression,
        collection: &str,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        // Evaluate the key expression
        let key = self.extract_document_key(key_expr, context)?;

        // Get the document before deletion to return it
        let doc = storage.get_document(collection, &key).await?;

        // Delete the document
        let deleted = storage.delete_document(collection, &key).await?;

        if deleted {
            info!("AQL REMOVE: Deleted document {}/{}", collection, key);
            if let Some(d) = doc {
                Ok(self.document_to_value(&d))
            } else {
                Ok(AqlValue::Object({
                    let mut m = HashMap::new();
                    m.insert("_key".to_string(), AqlValue::String(key));
                    m.insert("_removed".to_string(), AqlValue::Bool(true));
                    m
                }))
            }
        } else {
            Err(ProtocolError::AqlError(format!(
                "Document {}/{} not found for REMOVE",
                collection, key
            )))
        }
    }

    /// Execute UPSERT clause - insert or update/replace
    async fn execute_upsert(
        &self,
        storage: &AqlStorage,
        search_expr: &AqlExpression,
        insert_expr: &AqlExpression,
        update_or_replace: &UpsertAction,
        collection: &str,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        // Evaluate the search expression to find matching document
        let search_value = self.evaluate_expression(search_expr, context)?;

        // Try to find the document by _key if present in search
        let existing_key = if let AqlValue::Object(ref obj) = search_value {
            obj.get("_key").and_then(|v| {
                if let AqlValue::String(k) = v {
                    Some(k.clone())
                } else {
                    None
                }
            })
        } else {
            None
        };

        // Check if document exists
        let doc_exists = if let Some(ref key) = existing_key {
            storage.document_exists(collection, key).await
        } else {
            false
        };

        if doc_exists {
            // Document exists - perform UPDATE or REPLACE
            let key = existing_key.unwrap();
            match update_or_replace {
                UpsertAction::Update(update_expr) => {
                    let update_value = self.evaluate_expression(update_expr, context)?;
                    let updates = match update_value {
                        AqlValue::Object(obj) => obj,
                        _ => {
                            return Err(ProtocolError::AqlError(
                                "UPSERT UPDATE expects an object expression".to_string(),
                            ))
                        }
                    };
                    let updated_doc = storage
                        .update_document(collection, &key, updates)
                        .await?
                        .ok_or_else(|| {
                            ProtocolError::AqlError(format!(
                                "Document {}/{} not found for UPSERT UPDATE",
                                collection, key
                            ))
                        })?;
                    info!(
                        "AQL UPSERT: Updated existing document {}/{}",
                        collection, key
                    );
                    Ok(self.document_to_value(&updated_doc))
                }
                UpsertAction::Replace(replace_expr) => {
                    let replace_value = self.evaluate_expression(replace_expr, context)?;
                    let new_data = match replace_value {
                        AqlValue::Object(obj) => obj,
                        _ => {
                            return Err(ProtocolError::AqlError(
                                "UPSERT REPLACE expects an object expression".to_string(),
                            ))
                        }
                    };

                    storage.delete_document(collection, &key).await?;
                    let mut data = HashMap::new();
                    for (k, v) in new_data {
                        if !k.starts_with('_') {
                            data.insert(k, v);
                        }
                    }
                    let doc = AqlDocument::new(collection, key.clone(), data);
                    storage.store_document(doc.clone()).await?;
                    info!(
                        "AQL UPSERT: Replaced existing document {}/{}",
                        collection, key
                    );
                    Ok(self.document_to_value(&doc))
                }
            }
        } else {
            // Document doesn't exist - perform INSERT
            let insert_value = self.evaluate_expression(insert_expr, context)?;
            let doc_data = match insert_value {
                AqlValue::Object(obj) => obj,
                _ => {
                    return Err(ProtocolError::AqlError(
                        "UPSERT INSERT expects an object expression".to_string(),
                    ))
                }
            };

            let key = if let Some(AqlValue::String(k)) = doc_data.get("_key") {
                k.clone()
            } else if let Some(k) = existing_key {
                k
            } else {
                uuid::Uuid::new_v4().to_string()
            };

            let mut data = HashMap::new();
            for (k, v) in doc_data {
                if !k.starts_with('_') {
                    data.insert(k, v);
                }
            }

            let doc = AqlDocument::new(collection, key.clone(), data);
            storage.store_document(doc.clone()).await?;
            info!("AQL UPSERT: Inserted new document {}/{}", collection, key);
            Ok(self.document_to_value(&doc))
        }
    }

    /// Extract document key from key expression
    fn extract_document_key(
        &self,
        key_expr: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<String> {
        let key_value = self.evaluate_expression(key_expr, context)?;

        match key_value {
            AqlValue::String(k) => Ok(k),
            AqlValue::Object(obj) => {
                // If it's an object, try to get the _key field
                if let Some(AqlValue::String(k)) = obj.get("_key") {
                    Ok(k.clone())
                } else {
                    Err(ProtocolError::AqlError(
                        "Could not extract _key from object".to_string(),
                    ))
                }
            }
            _ => Err(ProtocolError::AqlError(format!(
                "Invalid key type: expected string or object with _key, got {:?}",
                key_value
            ))),
        }
    }

    /// Evaluate an AQL expression
    #[allow(clippy::only_used_in_recursion)]
    fn evaluate_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        use crate::protocols::aql::aql_parser::AqlExpression;

        match expression {
            AqlExpression::Variable(name) => context
                .get(name)
                .cloned()
                .ok_or_else(|| ProtocolError::AqlError(format!("Variable '{}' not found", name))),
            AqlExpression::Literal(value) => Ok(value.clone()),
            AqlExpression::PropertyAccess { object, property } => {
                let obj_value = context.get(object).ok_or_else(|| {
                    ProtocolError::AqlError(format!("Object '{}' not found", object))
                })?;

                if let AqlValue::Object(obj_map) = obj_value {
                    obj_map.get(property).cloned().ok_or_else(|| {
                        ProtocolError::AqlError(format!("Property '{}' not found", property))
                    })
                } else {
                    Err(ProtocolError::AqlError(
                        "Property access on non-object".to_string(),
                    ))
                }
            }
            AqlExpression::Object(fields) => {
                let mut result = HashMap::new();
                for (key, expr) in fields {
                    result.insert(key.clone(), self.evaluate_expression(expr, context)?);
                }
                Ok(AqlValue::Object(result))
            }
            AqlExpression::Array(elements) => {
                let mut result = Vec::new();
                for expr in elements {
                    result.push(self.evaluate_expression(expr, context)?);
                }
                Ok(AqlValue::Array(result))
            }
            AqlExpression::FunctionCall { name, args } => {
                // Evaluate built-in functions
                let evaluated_args: Vec<AqlValue> = args
                    .iter()
                    .map(|arg| self.evaluate_expression(arg, context))
                    .collect::<ProtocolResult<Vec<_>>>()?;

                self.evaluate_builtin_function(name, &evaluated_args)
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;

                match op.as_str() {
                    "+" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            // Preserve integer type if both operands are integers
                            if let (Some(l_i64), Some(r_i64)) = (l.as_i64(), r.as_i64()) {
                                Ok(AqlValue::Number(serde_json::Number::from(l_i64 + r_i64)))
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0);
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        (AqlValue::String(l), AqlValue::String(r)) => {
                            Ok(AqlValue::String(format!("{}{}", l, r)))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "-" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            if let (Some(l_i64), Some(r_i64)) = (l.as_i64(), r.as_i64()) {
                                Ok(AqlValue::Number(serde_json::Number::from(l_i64 - r_i64)))
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0);
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "*" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            if let (Some(l_i64), Some(r_i64)) = (l.as_i64(), r.as_i64()) {
                                Ok(AqlValue::Number(serde_json::Number::from(l_i64 * r_i64)))
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0);
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "/" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let r_f64 = r.as_f64().unwrap_or(0.0);
                            if r_f64.abs() < f64::EPSILON {
                                Ok(AqlValue::Null) // Division by zero
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) / r_f64;
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "%" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let r_f64 = r.as_f64().unwrap_or(0.0);
                            if r_f64.abs() < f64::EPSILON {
                                Ok(AqlValue::Null)
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) % r_f64;
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "==" | "=" => Ok(AqlValue::Bool(left_val == right_val)),
                    "!=" | "<>" => Ok(AqlValue::Bool(left_val != right_val)),
                    "AND" | "&&" => match (&left_val, &right_val) {
                        (AqlValue::Bool(l), AqlValue::Bool(r)) => Ok(AqlValue::Bool(*l && *r)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    "OR" | "||" => match (&left_val, &right_val) {
                        (AqlValue::Bool(l), AqlValue::Bool(r)) => Ok(AqlValue::Bool(*l || *r)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    _ => Ok(AqlValue::Null),
                }
            }
            AqlExpression::UnaryOp { op, expr } => {
                let val = self.evaluate_expression(expr, context)?;

                match op.as_str() {
                    "-" => match val {
                        AqlValue::Number(n) => {
                            let result = -n.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "NOT" | "!" => match val {
                        AqlValue::Bool(b) => Ok(AqlValue::Bool(!b)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    _ => Ok(AqlValue::Null),
                }
            }
        }
    }

    /// Evaluate an AQL condition
    fn evaluate_condition(
        &self,
        condition: &AqlCondition,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::aql_parser::AqlCondition;

        match condition {
            AqlCondition::Comparison {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;
                self.compare_values(&left_val, operator, &right_val)
            }
            AqlCondition::Expression(expr) => self.evaluate_expression_as_bool(expr, context),
        }
    }

    /// Evaluate an expression and convert the result to a boolean
    fn evaluate_expression_as_bool(
        &self,
        expr: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::aql_parser::AqlExpression;

        match expr {
            AqlExpression::BinaryOp { op, left, right } => {
                match op.as_str() {
                    "AND" => {
                        let left_bool = self.evaluate_expression_as_bool(left, context)?;
                        if !left_bool {
                            return Ok(false); // Short-circuit
                        }
                        self.evaluate_expression_as_bool(right, context)
                    }
                    "OR" => {
                        let left_bool = self.evaluate_expression_as_bool(left, context)?;
                        if left_bool {
                            return Ok(true); // Short-circuit
                        }
                        self.evaluate_expression_as_bool(right, context)
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        // Comparison operators
                        let left_val = self.evaluate_expression(left, context)?;
                        let right_val = self.evaluate_expression(right, context)?;
                        let cmp_op = match op.as_str() {
                            "==" => ComparisonOperator::Equals,
                            "!=" => ComparisonOperator::NotEquals,
                            "<" => ComparisonOperator::Less,
                            "<=" => ComparisonOperator::LessOrEqual,
                            ">" => ComparisonOperator::Greater,
                            ">=" => ComparisonOperator::GreaterOrEqual,
                            _ => unreachable!(),
                        };
                        self.compare_values(&left_val, &cmp_op, &right_val)
                    }
                    _ => {
                        // Other binary ops - evaluate and check if truthy
                        let val = self.evaluate_expression(expr, context)?;
                        Ok(self.is_truthy(&val))
                    }
                }
            }
            AqlExpression::UnaryOp { op, expr: inner } if op == "NOT" => {
                let inner_bool = self.evaluate_expression_as_bool(inner, context)?;
                Ok(!inner_bool)
            }
            _ => {
                // For other expressions, evaluate and check truthiness
                let val = self.evaluate_expression(expr, context)?;
                Ok(self.is_truthy(&val))
            }
        }
    }

    /// Check if an AqlValue is truthy
    fn is_truthy(&self, val: &AqlValue) -> bool {
        match val {
            AqlValue::Bool(b) => *b,
            AqlValue::Null => false,
            AqlValue::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
            AqlValue::String(s) => !s.is_empty(),
            AqlValue::Array(arr) => !arr.is_empty(),
            AqlValue::Object(obj) => !obj.is_empty(),
            AqlValue::DateTime(_) => true, // DateTime is always truthy if present
        }
    }

    /// Compare two AQL values
    fn compare_values(
        &self,
        left: &AqlValue,
        operator: &ComparisonOperator,
        right: &AqlValue,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::data_model::AqlValue;

        match (left, right) {
            (AqlValue::Number(l), AqlValue::Number(r)) => {
                let l_f64 = l.as_f64().unwrap_or(0.0);
                let r_f64 = r.as_f64().unwrap_or(0.0);
                Ok(match operator {
                    ComparisonOperator::Equals => (l_f64 - r_f64).abs() < f64::EPSILON,
                    ComparisonOperator::NotEquals => (l_f64 - r_f64).abs() >= f64::EPSILON,
                    ComparisonOperator::Greater => l_f64 > r_f64,
                    ComparisonOperator::Less => l_f64 < r_f64,
                    ComparisonOperator::GreaterOrEqual => l_f64 >= r_f64,
                    ComparisonOperator::LessOrEqual => l_f64 <= r_f64,
                })
            }
            (AqlValue::String(l), AqlValue::String(r)) => Ok(match operator {
                ComparisonOperator::Equals => l == r,
                ComparisonOperator::NotEquals => l != r,
                ComparisonOperator::Greater => l > r,
                ComparisonOperator::Less => l < r,
                ComparisonOperator::GreaterOrEqual => l >= r,
                ComparisonOperator::LessOrEqual => l <= r,
            }),
            _ => Ok(false),
        }
    }

    /// Convert AQL document to AQL value
    fn document_to_value(&self, doc: &AqlDocument) -> AqlValue {
        let mut obj = doc.data.clone();
        // Include system fields
        obj.insert("_key".to_string(), AqlValue::String(doc.key.clone()));
        obj.insert("_id".to_string(), AqlValue::String(doc.id.clone()));
        obj.insert("_rev".to_string(), AqlValue::String(doc.revision.clone()));
        AqlValue::Object(obj)
    }

    /// Apply SORT and LIMIT clauses
    fn apply_sort_and_limit(
        &self,
        clauses: &[AqlClause],
        mut results: Vec<AqlValue>,
    ) -> ProtocolResult<Vec<AqlValue>> {
        use crate::protocols::aql::aql_parser::AqlClause;

        // Apply SORT if present
        for clause in clauses {
            if let AqlClause::Sort { items } = clause {
                if !items.is_empty() {
                    // Sort the results based on sort items
                    results.sort_by(|a, b| {
                        for item in items {
                            // Extract value for comparison from each result
                            let val_a = self.extract_sort_value(a, &item.expression);
                            let val_b = self.extract_sort_value(b, &item.expression);

                            let cmp = self.compare_aql_values(&val_a, &val_b);
                            if cmp != std::cmp::Ordering::Equal {
                                return match item.direction {
                                    crate::protocols::aql::aql_parser::SortDirection::Asc => cmp,
                                    crate::protocols::aql::aql_parser::SortDirection::Desc => {
                                        cmp.reverse()
                                    }
                                };
                            }
                        }
                        std::cmp::Ordering::Equal
                    });
                }
                break;
            }
        }

        // Apply LIMIT if present
        for clause in clauses {
            if let AqlClause::Limit { offset, count } = clause {
                let offset = offset.unwrap_or(0) as usize;
                let count = *count as usize;
                results = results.into_iter().skip(offset).take(count).collect();
                break;
            }
        }

        Ok(results)
    }

    /// Remove duplicate results (for DISTINCT)
    fn deduplicate_results(&self, results: Vec<AqlValue>) -> Vec<AqlValue> {
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();

        for result in results {
            // Simple deduplication based on string representation
            let key = format!("{:?}", result);
            if seen.insert(key) {
                unique.push(result);
            }
        }

        unique
    }

    /// Evaluate a search expression for full-text search
    ///
    /// Supports ArangoSearch-compatible functions:
    /// - PHRASE(doc.field, "search text") - exact phrase match
    /// - ANALYZER(expression, "analyzer_name") - apply analyzer
    /// - STARTS_WITH(doc.field, "prefix") - prefix matching
    /// - LIKE(doc.field, "pattern%") - pattern matching
    /// - LEVENSHTEIN_MATCH(doc.field, "term", distance) - fuzzy matching
    /// - IN_RANGE(doc.field, min, max, include_min, include_max) - range search
    /// - BOOST(expression, factor) - relevance boost
    /// - EXISTS(doc.field) - field existence check
    /// - Boolean operators: AND, OR, NOT
    #[allow(dead_code)]
    fn evaluate_search_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
        _analyzer: &str,
    ) -> ProtocolResult<bool> {
        match expression {
            AqlExpression::FunctionCall { name, args } => {
                self.evaluate_search_function(name, args, context)
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_result = self.evaluate_search_expression(left, context, _analyzer)?;
                let right_result = self.evaluate_search_expression(right, context, _analyzer)?;

                match op.as_str() {
                    "AND" | "&&" => Ok(left_result && right_result),
                    "OR" | "||" => Ok(left_result || right_result),
                    _ => Ok(false),
                }
            }
            AqlExpression::UnaryOp { op, expr } if op == "NOT" || op == "!" => {
                let result = self.evaluate_search_expression(expr, context, _analyzer)?;
                Ok(!result)
            }
            AqlExpression::Literal(value) => match value {
                AqlValue::Bool(b) => Ok(*b),
                _ => Ok(false),
            },
            // For conditions like doc.field == "value", evaluate as equality check
            _ => Ok(true),
        }
    }

    /// Evaluate a search function call
    fn evaluate_search_function(
        &self,
        name: &str,
        args: &[AqlExpression],
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        match name.to_uppercase().as_str() {
            "PHRASE" => {
                // PHRASE(doc.field, "search phrase") - exact phrase matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let search_phrase = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(phrase)) =
                    (&field_value, &search_phrase)
                {
                    Ok(text.to_lowercase().contains(&phrase.to_lowercase()))
                } else {
                    Ok(false)
                }
            }
            "STARTS_WITH" => {
                // STARTS_WITH(doc.field, "prefix") - prefix matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let prefix = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(pref)) = (&field_value, &prefix) {
                    Ok(text.to_lowercase().starts_with(&pref.to_lowercase()))
                } else {
                    Ok(false)
                }
            }
            "LIKE" => {
                // LIKE(doc.field, "pattern%") - SQL-like pattern matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let pattern = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(pat)) = (&field_value, &pattern) {
                    // Simple pattern matching: % = any chars, _ = single char
                    let regex_pattern = pat.replace('%', ".*").replace('_', ".");
                    if let Ok(re) = regex::Regex::new(&format!("(?i)^{}$", regex_pattern)) {
                        Ok(re.is_match(text))
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            "LEVENSHTEIN_MATCH" => {
                // LEVENSHTEIN_MATCH(doc.field, "term", max_distance) - fuzzy matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let term = self.evaluate_expression(&args[1], context)?;
                let max_distance: usize = if args.len() >= 3 {
                    if let Ok(AqlValue::Number(n)) = self.evaluate_expression(&args[2], context) {
                        n.as_u64().unwrap_or(2) as usize
                    } else {
                        2
                    }
                } else {
                    2
                };

                if let (AqlValue::String(text), AqlValue::String(search_term)) =
                    (&field_value, &term)
                {
                    let distance = self
                        .levenshtein_distance(&text.to_lowercase(), &search_term.to_lowercase());
                    Ok(distance <= max_distance)
                } else {
                    Ok(false)
                }
            }
            "IN_RANGE" => {
                // IN_RANGE(doc.field, min, max, include_min, include_max)
                if args.len() < 3 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let min_val = self.evaluate_expression(&args[1], context)?;
                let max_val = self.evaluate_expression(&args[2], context)?;
                let include_min = args.get(3).is_none_or(|e| {
                    matches!(
                        self.evaluate_expression(e, context),
                        Ok(AqlValue::Bool(true))
                    )
                });
                let include_max = args.get(4).is_none_or(|e| {
                    matches!(
                        self.evaluate_expression(e, context),
                        Ok(AqlValue::Bool(true))
                    )
                });

                if let (AqlValue::Number(val), AqlValue::Number(min), AqlValue::Number(max)) =
                    (&field_value, &min_val, &max_val)
                {
                    let v = val.as_f64().unwrap_or(0.0);
                    let min_f = min.as_f64().unwrap_or(0.0);
                    let max_f = max.as_f64().unwrap_or(0.0);

                    let above_min = if include_min { v >= min_f } else { v > min_f };
                    let below_max = if include_max { v <= max_f } else { v < max_f };
                    Ok(above_min && below_max)
                } else {
                    Ok(false)
                }
            }
            "EXISTS" => {
                // EXISTS(doc.field) - check if field exists and is not null
                if args.is_empty() {
                    return Ok(false);
                }
                let value = self.evaluate_expression(&args[0], context);
                Ok(value.is_ok() && !matches!(value.unwrap(), AqlValue::Null))
            }
            "ANALYZER" => {
                // ANALYZER(expression, "analyzer_name") - apply analyzer and evaluate
                if args.is_empty() {
                    return Ok(false);
                }
                // For now, just evaluate the inner expression with default analyzer
                self.evaluate_search_expression(&args[0], context, "text_en")
            }
            "BOOST" => {
                // BOOST(expression, factor) - boost relevance (just evaluate expression for now)
                if args.is_empty() {
                    return Ok(false);
                }
                self.evaluate_search_expression(&args[0], context, "text_en")
            }
            "TOKENS" | "NGRAM_MATCH" | "NGRAM_SIMILARITY" => {
                // Token-based and n-gram functions - simplified implementation
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let search_value = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(search)) =
                    (&field_value, &search_value)
                {
                    // Simple token overlap check
                    let text_lower = text.to_lowercase();
                    let search_lower = search.to_lowercase();
                    let text_tokens: std::collections::HashSet<&str> =
                        text_lower.split_whitespace().collect();
                    let search_tokens: std::collections::HashSet<&str> =
                        search_lower.split_whitespace().collect();
                    let overlap = text_tokens.intersection(&search_tokens).count();
                    Ok(overlap > 0)
                } else {
                    Ok(false)
                }
            }
            _ => {
                // Unknown function - log and return false
                warn!("Unknown search function: {}", name);
                Ok(false)
            }
        }
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                    matrix[i - 1][j - 1] + cost,
                );
            }
        }

        matrix[a_len][b_len]
    }

    /// Public wrapper for evaluate_expression (for testing)
    #[cfg(test)]
    pub fn evaluate_expression_public(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        self.evaluate_expression(expression, context)
    }

    /// Public wrapper for levenshtein_distance (for testing)
    #[cfg(test)]
    pub fn levenshtein_distance_public(&self, a: &str, b: &str) -> usize {
        self.levenshtein_distance(a, b)
    }

    /// Extract a value from a result for sorting based on the expression
    fn extract_sort_value(&self, value: &AqlValue, expression: &AqlExpression) -> AqlValue {
        use crate::protocols::aql::aql_parser::AqlExpression;

        match expression {
            AqlExpression::Variable(name) => {
                // If the value is an object, try to get the field
                if let AqlValue::Object(map) = value {
                    map.get(name).cloned().unwrap_or(AqlValue::Null)
                } else if name == "doc" || name == "_" {
                    value.clone()
                } else {
                    AqlValue::Null
                }
            }
            AqlExpression::PropertyAccess { object, property } => {
                // First resolve the object variable from the value
                let obj_val = if let AqlValue::Object(map) = value {
                    map.get(object).cloned().unwrap_or(AqlValue::Null)
                } else if object == "doc" || object == "_" {
                    value.clone()
                } else {
                    AqlValue::Null
                };
                // Then get the property from that object
                if let AqlValue::Object(map) = obj_val {
                    map.get(property).cloned().unwrap_or(AqlValue::Null)
                } else {
                    AqlValue::Null
                }
            }
            AqlExpression::Literal(lit_val) => lit_val.clone(),
            _ => {
                // For complex expressions, try evaluating with the value as context
                let mut context = HashMap::new();
                if let AqlValue::Object(map) = value {
                    for (k, v) in map {
                        context.insert(k.clone(), v.clone());
                    }
                }
                context.insert("doc".to_string(), value.clone());
                self.evaluate_expression(expression, &context)
                    .unwrap_or(AqlValue::Null)
            }
        }
    }

    /// Compare two AQL values for sorting
    fn compare_aql_values(&self, a: &AqlValue, b: &AqlValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            (AqlValue::Null, AqlValue::Null) => Ordering::Equal,
            (AqlValue::Null, _) => Ordering::Less,
            (_, AqlValue::Null) => Ordering::Greater,
            (AqlValue::Bool(a), AqlValue::Bool(b)) => a.cmp(b),
            (AqlValue::Number(a), AqlValue::Number(b)) => {
                let a_f = a.as_f64().unwrap_or(0.0);
                let b_f = b.as_f64().unwrap_or(0.0);
                a_f.partial_cmp(&b_f).unwrap_or(Ordering::Equal)
            }
            (AqlValue::String(a), AqlValue::String(b)) => a.cmp(b),
            (AqlValue::Array(a), AqlValue::Array(b)) => a.len().cmp(&b.len()),
            (AqlValue::Object(_), AqlValue::Object(_)) => Ordering::Equal,
            // Cross-type comparisons
            (AqlValue::Number(_), AqlValue::String(_)) => Ordering::Less,
            (AqlValue::String(_), AqlValue::Number(_)) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }

    /// Evaluate built-in AQL functions
    fn evaluate_builtin_function(&self, name: &str, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        match name.to_uppercase().as_str() {
            // ============ String Functions ============
            "LENGTH" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::Number(serde_json::Number::from(s.len())))
                } else if let Some(AqlValue::Array(arr)) = args.first() {
                    Ok(AqlValue::Number(serde_json::Number::from(arr.len())))
                } else if let Some(AqlValue::Object(obj)) = args.first() {
                    Ok(AqlValue::Number(serde_json::Number::from(obj.len())))
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(0)))
                }
            }
            "UPPER" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.to_uppercase()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LOWER" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.to_lowercase()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "CONCAT" => {
                let result: String = args
                    .iter()
                    .filter_map(|v| match v {
                        AqlValue::String(s) => Some(s.clone()),
                        AqlValue::Number(n) => Some(n.to_string()),
                        AqlValue::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                    .collect();
                Ok(AqlValue::String(result))
            }
            "CONCAT_SEPARATOR" => {
                if args.len() < 2 {
                    return Ok(AqlValue::String(String::new()));
                }
                let sep = match &args[0] {
                    AqlValue::String(s) => s.clone(),
                    _ => String::new(),
                };
                let parts: Vec<String> = args[1..]
                    .iter()
                    .filter_map(|v| match v {
                        AqlValue::String(s) => Some(s.clone()),
                        AqlValue::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect();
                Ok(AqlValue::String(parts.join(&sep)))
            }
            "SUBSTRING" => {
                if args.len() < 2 {
                    return Ok(AqlValue::Null);
                }
                if let (Some(AqlValue::String(s)), Some(AqlValue::Number(offset))) =
                    (args.first(), args.get(1))
                {
                    let offset = offset.as_i64().unwrap_or(0) as usize;
                    let len = args
                        .get(2)
                        .and_then(|v| {
                            if let AqlValue::Number(n) = v {
                                n.as_i64()
                            } else {
                                None
                            }
                        })
                        .map(|l| l as usize);

                    if offset >= s.len() {
                        return Ok(AqlValue::String(String::new()));
                    }
                    let result = match len {
                        Some(l) => s.chars().skip(offset).take(l).collect(),
                        None => s.chars().skip(offset).collect(),
                    };
                    Ok(AqlValue::String(result))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LEFT" => {
                if let (Some(AqlValue::String(s)), Some(AqlValue::Number(n))) =
                    (args.first(), args.get(1))
                {
                    let n = n.as_i64().unwrap_or(0).max(0) as usize;
                    Ok(AqlValue::String(s.chars().take(n).collect()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "RIGHT" => {
                if let (Some(AqlValue::String(s)), Some(AqlValue::Number(n))) =
                    (args.first(), args.get(1))
                {
                    let n = n.as_i64().unwrap_or(0).max(0) as usize;
                    let len = s.chars().count();
                    let skip = len.saturating_sub(n);
                    Ok(AqlValue::String(s.chars().skip(skip).collect()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "TRIM" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.trim().to_string()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LTRIM" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.trim_start().to_string()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "RTRIM" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.trim_end().to_string()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SPLIT" => {
                if args.is_empty() {
                    return Ok(AqlValue::Array(vec![]));
                }
                if let Some(AqlValue::String(s)) = args.first() {
                    let sep = args
                        .get(1)
                        .and_then(|v| {
                            if let AqlValue::String(sep) = v {
                                Some(sep.as_str())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(",");
                    let parts: Vec<AqlValue> = s
                        .split(sep)
                        .map(|p| AqlValue::String(p.to_string()))
                        .collect();
                    Ok(AqlValue::Array(parts))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "REVERSE" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(s.chars().rev().collect()))
                } else if let Some(AqlValue::Array(arr)) = args.first() {
                    let mut reversed = arr.clone();
                    reversed.reverse();
                    Ok(AqlValue::Array(reversed))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "CONTAINS" => {
                if let (Some(AqlValue::String(haystack)), Some(AqlValue::String(needle))) =
                    (args.first(), args.get(1))
                {
                    let case_insensitive = args
                        .get(2)
                        .map(|v| matches!(v, AqlValue::Bool(true)))
                        .unwrap_or(false);
                    let result = if case_insensitive {
                        haystack.to_lowercase().contains(&needle.to_lowercase())
                    } else {
                        haystack.contains(needle.as_str())
                    };
                    Ok(AqlValue::Bool(result))
                } else {
                    Ok(AqlValue::Bool(false))
                }
            }
            "LIKE" => {
                if let (Some(AqlValue::String(text)), Some(AqlValue::String(pattern))) =
                    (args.first(), args.get(1))
                {
                    // Simple LIKE pattern matching (% = any, _ = single char)
                    let regex_pattern = pattern.replace('%', ".*").replace('_', ".");
                    if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                        Ok(AqlValue::Bool(re.is_match(text)))
                    } else {
                        Ok(AqlValue::Bool(false))
                    }
                } else {
                    Ok(AqlValue::Bool(false))
                }
            }
            "REGEX_TEST" => {
                if let (Some(AqlValue::String(text)), Some(AqlValue::String(pattern))) =
                    (args.first(), args.get(1))
                {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        Ok(AqlValue::Bool(re.is_match(text)))
                    } else {
                        Ok(AqlValue::Bool(false))
                    }
                } else {
                    Ok(AqlValue::Bool(false))
                }
            }
            "REGEX_REPLACE" => {
                if let (
                    Some(AqlValue::String(text)),
                    Some(AqlValue::String(pattern)),
                    Some(AqlValue::String(replacement)),
                ) = (args.first(), args.get(1), args.get(2))
                {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        Ok(AqlValue::String(
                            re.replace_all(text, replacement.as_str()).to_string(),
                        ))
                    } else {
                        Ok(AqlValue::String(text.clone()))
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "MD5" | "SHA1" | "SHA256" | "SHA512" => {
                // Hash functions - return placeholder for now
                if let Some(AqlValue::String(s)) = args.first() {
                    // Simple hash placeholder - would need actual crypto lib
                    Ok(AqlValue::String(format!(
                        "{}_{}",
                        name.to_lowercase(),
                        s.len()
                    )))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "ENCODE_URI_COMPONENT" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::String(urlencoding::encode(s).to_string()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "DECODE_URI_COMPONENT" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    match urlencoding::decode(s) {
                        Ok(decoded) => Ok(AqlValue::String(decoded.to_string())),
                        Err(_) => Ok(AqlValue::String(s.clone())),
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }

            // ============ Numeric Functions ============
            "ABS" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0).abs();
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "CEIL" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0).ceil();
                    Ok(AqlValue::Number(serde_json::Number::from(f as i64)))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "FLOOR" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0).floor();
                    Ok(AqlValue::Number(serde_json::Number::from(f as i64)))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "ROUND" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let precision = args
                        .get(1)
                        .and_then(|v| {
                            if let AqlValue::Number(p) = v {
                                p.as_i64()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let f = n.as_f64().unwrap_or(0.0);
                    let factor = 10_f64.powi(precision as i32);
                    let rounded = (f * factor).round() / factor;
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(rounded)
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SQRT" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    if f >= 0.0 {
                        Ok(AqlValue::Number(
                            serde_json::Number::from_f64(f.sqrt())
                                .unwrap_or(serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(AqlValue::Null)
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "POW" => {
                if let (Some(AqlValue::Number(base)), Some(AqlValue::Number(exp))) =
                    (args.first(), args.get(1))
                {
                    let b = base.as_f64().unwrap_or(0.0);
                    let e = exp.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(b.powf(e))
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LOG" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    if f > 0.0 {
                        let base = args
                            .get(1)
                            .and_then(|v| {
                                if let AqlValue::Number(b) = v {
                                    b.as_f64()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(std::f64::consts::E);
                        Ok(AqlValue::Number(
                            serde_json::Number::from_f64(f.log(base))
                                .unwrap_or(serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(AqlValue::Null)
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LOG2" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    if f > 0.0 {
                        Ok(AqlValue::Number(
                            serde_json::Number::from_f64(f.log2())
                                .unwrap_or(serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(AqlValue::Null)
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LOG10" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    if f > 0.0 {
                        Ok(AqlValue::Number(
                            serde_json::Number::from_f64(f.log10())
                                .unwrap_or(serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(AqlValue::Null)
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "EXP" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(f.exp())
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "EXP2" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(f.exp2())
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SIN" | "COS" | "TAN" | "ASIN" | "ACOS" | "ATAN" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    let result = match name.to_uppercase().as_str() {
                        "SIN" => f.sin(),
                        "COS" => f.cos(),
                        "TAN" => f.tan(),
                        "ASIN" => f.asin(),
                        "ACOS" => f.acos(),
                        "ATAN" => f.atan(),
                        _ => 0.0,
                    };
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(result).unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "ATAN2" => {
                if let (Some(AqlValue::Number(y)), Some(AqlValue::Number(x))) =
                    (args.first(), args.get(1))
                {
                    let y_f = y.as_f64().unwrap_or(0.0);
                    let x_f = x.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(y_f.atan2(x_f))
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "PI" => Ok(AqlValue::Number(
                serde_json::Number::from_f64(std::f64::consts::PI)
                    .unwrap_or(serde_json::Number::from(0)),
            )),
            "DEGREES" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(f.to_degrees())
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "RADIANS" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let f = n.as_f64().unwrap_or(0.0);
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(f.to_radians())
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "MIN" => {
                if args.is_empty() {
                    return Ok(AqlValue::Null);
                }
                let mut min_val: Option<f64> = None;
                for arg in args {
                    if let AqlValue::Number(n) = arg {
                        let f = n.as_f64().unwrap_or(f64::MAX);
                        min_val = Some(min_val.map_or(f, |m| m.min(f)));
                    } else if let AqlValue::Array(arr) = arg {
                        for item in arr {
                            if let AqlValue::Number(n) = item {
                                let f = n.as_f64().unwrap_or(f64::MAX);
                                min_val = Some(min_val.map_or(f, |m| m.min(f)));
                            }
                        }
                    }
                }
                match min_val {
                    Some(v) => Ok(AqlValue::Number(
                        serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0)),
                    )),
                    None => Ok(AqlValue::Null),
                }
            }
            "MAX" => {
                if args.is_empty() {
                    return Ok(AqlValue::Null);
                }
                let mut max_val: Option<f64> = None;
                for arg in args {
                    if let AqlValue::Number(n) = arg {
                        let f = n.as_f64().unwrap_or(f64::MIN);
                        max_val = Some(max_val.map_or(f, |m| m.max(f)));
                    } else if let AqlValue::Array(arr) = arg {
                        for item in arr {
                            if let AqlValue::Number(n) = item {
                                let f = n.as_f64().unwrap_or(f64::MIN);
                                max_val = Some(max_val.map_or(f, |m| m.max(f)));
                            }
                        }
                    }
                }
                match max_val {
                    Some(v) => Ok(AqlValue::Number(
                        serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0)),
                    )),
                    None => Ok(AqlValue::Null),
                }
            }
            "SUM" => {
                let mut sum = 0.0;
                for arg in args {
                    if let AqlValue::Number(n) = arg {
                        sum += n.as_f64().unwrap_or(0.0);
                    } else if let AqlValue::Array(arr) = arg {
                        for item in arr {
                            if let AqlValue::Number(n) = item {
                                sum += n.as_f64().unwrap_or(0.0);
                            }
                        }
                    }
                }
                Ok(AqlValue::Number(
                    serde_json::Number::from_f64(sum).unwrap_or(serde_json::Number::from(0)),
                ))
            }
            "AVERAGE" | "AVG" => {
                let mut sum = 0.0;
                let mut count = 0;
                for arg in args {
                    if let AqlValue::Number(n) = arg {
                        sum += n.as_f64().unwrap_or(0.0);
                        count += 1;
                    } else if let AqlValue::Array(arr) = arg {
                        for item in arr {
                            if let AqlValue::Number(n) = item {
                                sum += n.as_f64().unwrap_or(0.0);
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(sum / count as f64)
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "RAND" => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                Ok(AqlValue::Number(
                    serde_json::Number::from_f64(rng.gen::<f64>())
                        .unwrap_or(serde_json::Number::from(0)),
                ))
            }
            "RANDOM_TOKEN" => {
                use rand::Rng;
                let length = args
                    .first()
                    .and_then(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_i64()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(16) as usize;
                let token: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(length)
                    .map(char::from)
                    .collect();
                Ok(AqlValue::String(token))
            }

            // ============ Array Functions ============
            "FIRST" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    Ok(arr.first().cloned().unwrap_or(AqlValue::Null))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "LAST" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    Ok(arr.last().cloned().unwrap_or(AqlValue::Null))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "NTH" => {
                if let (Some(AqlValue::Array(arr)), Some(AqlValue::Number(n))) =
                    (args.first(), args.get(1))
                {
                    let idx = n.as_i64().unwrap_or(0) as usize;
                    Ok(arr.get(idx).cloned().unwrap_or(AqlValue::Null))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "PUSH" => {
                if let (Some(AqlValue::Array(arr)), Some(value)) = (args.first(), args.get(1)) {
                    let mut new_arr = arr.clone();
                    new_arr.push(value.clone());
                    Ok(AqlValue::Array(new_arr))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "APPEND" => {
                if let (Some(AqlValue::Array(arr1)), Some(AqlValue::Array(arr2))) =
                    (args.first(), args.get(1))
                {
                    let mut new_arr = arr1.clone();
                    new_arr.extend(arr2.clone());
                    Ok(AqlValue::Array(new_arr))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "POP" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    let mut new_arr = arr.clone();
                    new_arr.pop();
                    Ok(AqlValue::Array(new_arr))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SHIFT" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    if arr.is_empty() {
                        Ok(AqlValue::Array(vec![]))
                    } else {
                        Ok(AqlValue::Array(arr[1..].to_vec()))
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "UNSHIFT" => {
                if let (Some(AqlValue::Array(arr)), Some(value)) = (args.first(), args.get(1)) {
                    let mut new_arr = vec![value.clone()];
                    new_arr.extend(arr.clone());
                    Ok(AqlValue::Array(new_arr))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "UNIQUE" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    let mut seen = std::collections::HashSet::new();
                    let unique: Vec<AqlValue> = arr
                        .iter()
                        .filter(|v| {
                            let key = format!("{:?}", v);
                            seen.insert(key)
                        })
                        .cloned()
                        .collect();
                    Ok(AqlValue::Array(unique))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "FLATTEN" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    let depth = args
                        .get(1)
                        .and_then(|v| {
                            if let AqlValue::Number(n) = v {
                                n.as_i64()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(1) as usize;
                    fn flatten_recursive(arr: &[AqlValue], depth: usize) -> Vec<AqlValue> {
                        let mut result = Vec::new();
                        for item in arr {
                            if let AqlValue::Array(inner) = item {
                                if depth > 0 {
                                    result.extend(flatten_recursive(inner, depth - 1));
                                } else {
                                    result.push(item.clone());
                                }
                            } else {
                                result.push(item.clone());
                            }
                        }
                        result
                    }
                    Ok(AqlValue::Array(flatten_recursive(arr, depth)))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SLICE" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    let start = args
                        .get(1)
                        .and_then(|v| {
                            if let AqlValue::Number(n) = v {
                                n.as_i64()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0) as usize;
                    let length = args.get(2).and_then(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_i64().map(|l| l as usize)
                        } else {
                            None
                        }
                    });
                    let end = length.map_or(arr.len(), |l| (start + l).min(arr.len()));
                    Ok(AqlValue::Array(arr[start.min(arr.len())..end].to_vec()))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "POSITION" | "FIND_FIRST" => {
                if let (Some(AqlValue::Array(arr)), Some(needle)) = (args.first(), args.get(1)) {
                    for (i, item) in arr.iter().enumerate() {
                        if item == needle {
                            return Ok(AqlValue::Number(serde_json::Number::from(i)));
                        }
                    }
                    Ok(AqlValue::Number(serde_json::Number::from(-1)))
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(-1)))
                }
            }
            "COUNT" | "COUNT_DISTINCT" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    if name.to_uppercase() == "COUNT_DISTINCT" {
                        let mut seen = std::collections::HashSet::new();
                        for v in arr {
                            seen.insert(format!("{:?}", v));
                        }
                        Ok(AqlValue::Number(serde_json::Number::from(seen.len())))
                    } else {
                        Ok(AqlValue::Number(serde_json::Number::from(arr.len())))
                    }
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(0)))
                }
            }
            "SORTED" | "SORTED_UNIQUE" => {
                if let Some(AqlValue::Array(arr)) = args.first() {
                    let mut sorted = arr.clone();
                    sorted.sort_by(|a, b| match (a, b) {
                        (AqlValue::Number(n1), AqlValue::Number(n2)) => {
                            let f1 = n1.as_f64().unwrap_or(0.0);
                            let f2 = n2.as_f64().unwrap_or(0.0);
                            f1.partial_cmp(&f2).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (AqlValue::String(s1), AqlValue::String(s2)) => s1.cmp(s2),
                        _ => std::cmp::Ordering::Equal,
                    });
                    if name.to_uppercase() == "SORTED_UNIQUE" {
                        let mut seen = std::collections::HashSet::new();
                        sorted.retain(|v| {
                            let key = format!("{:?}", v);
                            seen.insert(key)
                        });
                    }
                    Ok(AqlValue::Array(sorted))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "UNION" | "UNION_DISTINCT" => {
                let mut result = Vec::new();
                for arg in args {
                    if let AqlValue::Array(arr) = arg {
                        result.extend(arr.clone());
                    }
                }
                if name.to_uppercase() == "UNION_DISTINCT" {
                    let mut seen = std::collections::HashSet::new();
                    result.retain(|v| {
                        let key = format!("{:?}", v);
                        seen.insert(key)
                    });
                }
                Ok(AqlValue::Array(result))
            }
            "INTERSECTION" => {
                if args.is_empty() {
                    return Ok(AqlValue::Array(vec![]));
                }
                let first = match args.first() {
                    Some(AqlValue::Array(arr)) => arr.clone(),
                    _ => return Ok(AqlValue::Array(vec![])),
                };
                let result: Vec<AqlValue> = first
                    .into_iter()
                    .filter(|item| {
                        args[1..].iter().all(|arg| {
                            if let AqlValue::Array(arr) = arg {
                                arr.contains(item)
                            } else {
                                false
                            }
                        })
                    })
                    .collect();
                Ok(AqlValue::Array(result))
            }
            "MINUS" => {
                if args.len() < 2 {
                    return Ok(args.first().cloned().unwrap_or(AqlValue::Null));
                }
                let first = match args.first() {
                    Some(AqlValue::Array(arr)) => arr.clone(),
                    _ => return Ok(AqlValue::Array(vec![])),
                };
                let result: Vec<AqlValue> = first
                    .into_iter()
                    .filter(|item| {
                        !args[1..].iter().any(|arg| {
                            if let AqlValue::Array(arr) = arg {
                                arr.contains(item)
                            } else {
                                false
                            }
                        })
                    })
                    .collect();
                Ok(AqlValue::Array(result))
            }

            // ============ Object/Document Functions ============
            "KEYS" | "ATTRIBUTES" => {
                if let Some(AqlValue::Object(obj)) = args.first() {
                    let keys: Vec<AqlValue> =
                        obj.keys().map(|k| AqlValue::String(k.clone())).collect();
                    Ok(AqlValue::Array(keys))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "VALUES" => {
                if let Some(AqlValue::Object(obj)) = args.first() {
                    let values: Vec<AqlValue> = obj.values().cloned().collect();
                    Ok(AqlValue::Array(values))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "MERGE" => {
                let mut result = HashMap::new();
                for arg in args {
                    if let AqlValue::Object(obj) = arg {
                        for (k, v) in obj {
                            result.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(AqlValue::Object(result))
            }
            "MERGE_RECURSIVE" => {
                fn merge_recursive(
                    base: HashMap<String, AqlValue>,
                    overlay: &HashMap<String, AqlValue>,
                ) -> HashMap<String, AqlValue> {
                    let mut result = base;
                    for (k, v) in overlay {
                        if let (Some(AqlValue::Object(base_obj)), AqlValue::Object(overlay_obj)) =
                            (result.get(k), v)
                        {
                            result.insert(
                                k.clone(),
                                AqlValue::Object(merge_recursive(base_obj.clone(), overlay_obj)),
                            );
                        } else {
                            result.insert(k.clone(), v.clone());
                        }
                    }
                    result
                }
                let mut result = HashMap::new();
                for arg in args {
                    if let AqlValue::Object(obj) = arg {
                        result = merge_recursive(result, obj);
                    }
                }
                Ok(AqlValue::Object(result))
            }
            "HAS" => {
                if let (Some(AqlValue::Object(obj)), Some(AqlValue::String(key))) =
                    (args.first(), args.get(1))
                {
                    Ok(AqlValue::Bool(obj.contains_key(key)))
                } else {
                    Ok(AqlValue::Bool(false))
                }
            }
            "UNSET" => {
                if args.len() < 2 {
                    return Ok(args.first().cloned().unwrap_or(AqlValue::Null));
                }
                if let Some(AqlValue::Object(obj)) = args.first() {
                    let mut result = obj.clone();
                    for arg in &args[1..] {
                        if let AqlValue::String(key) = arg {
                            result.remove(key);
                        }
                    }
                    Ok(AqlValue::Object(result))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "KEEP" => {
                if args.len() < 2 {
                    return Ok(args.first().cloned().unwrap_or(AqlValue::Null));
                }
                if let Some(AqlValue::Object(obj)) = args.first() {
                    let keys_to_keep: std::collections::HashSet<String> = args[1..]
                        .iter()
                        .filter_map(|a| {
                            if let AqlValue::String(k) = a {
                                Some(k.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let result: HashMap<String, AqlValue> = obj
                        .iter()
                        .filter(|(k, _)| keys_to_keep.contains(*k))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    Ok(AqlValue::Object(result))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "ZIP" => {
                if let (Some(AqlValue::Array(keys)), Some(AqlValue::Array(values))) =
                    (args.first(), args.get(1))
                {
                    let mut result = HashMap::new();
                    for (k, v) in keys.iter().zip(values.iter()) {
                        if let AqlValue::String(key) = k {
                            result.insert(key.clone(), v.clone());
                        }
                    }
                    Ok(AqlValue::Object(result))
                } else {
                    Ok(AqlValue::Null)
                }
            }

            // ============ Type Functions ============
            "IS_NULL" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::Null) | None
            ))),
            "IS_BOOL" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::Bool(_))
            ))),
            "IS_NUMBER" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::Number(_))
            ))),
            "IS_STRING" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::String(_))
            ))),
            "IS_ARRAY" | "IS_LIST" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::Array(_))
            ))),
            "IS_OBJECT" | "IS_DOCUMENT" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::Object(_))
            ))),
            "TYPENAME" => {
                let type_name = match args.first() {
                    Some(AqlValue::Null) => "null",
                    Some(AqlValue::Bool(_)) => "bool",
                    Some(AqlValue::Number(_)) => "number",
                    Some(AqlValue::String(_)) => "string",
                    Some(AqlValue::Array(_)) => "array",
                    Some(AqlValue::Object(_)) => "object",
                    Some(AqlValue::DateTime(_)) => "datetime",
                    None => "null",
                };
                Ok(AqlValue::String(type_name.to_string()))
            }
            "TO_BOOL" => {
                let result = match args.first() {
                    Some(AqlValue::Bool(b)) => *b,
                    Some(AqlValue::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
                    Some(AqlValue::String(s)) => !s.is_empty(),
                    Some(AqlValue::Array(arr)) => !arr.is_empty(),
                    Some(AqlValue::Object(obj)) => !obj.is_empty(),
                    _ => false,
                };
                Ok(AqlValue::Bool(result))
            }
            "TO_NUMBER" => {
                let result = match args.first() {
                    Some(AqlValue::Number(n)) => n.clone(),
                    Some(AqlValue::String(s)) => s
                        .parse::<f64>()
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .unwrap_or(serde_json::Number::from(0)),
                    Some(AqlValue::Bool(true)) => serde_json::Number::from(1),
                    Some(AqlValue::Bool(false)) => serde_json::Number::from(0),
                    _ => serde_json::Number::from(0),
                };
                Ok(AqlValue::Number(result))
            }
            "TO_STRING" => {
                let result = match args.first() {
                    Some(AqlValue::String(s)) => s.clone(),
                    Some(AqlValue::Number(n)) => n.to_string(),
                    Some(AqlValue::Bool(b)) => b.to_string(),
                    Some(AqlValue::Null) => "null".to_string(),
                    Some(AqlValue::Array(arr)) => {
                        format!("{:?}", arr)
                    }
                    Some(AqlValue::Object(obj)) => {
                        format!("{:?}", obj)
                    }
                    Some(AqlValue::DateTime(dt)) => dt.to_rfc3339(),
                    None => "null".to_string(),
                };
                Ok(AqlValue::String(result))
            }
            "TO_ARRAY" | "TO_LIST" => match args.first() {
                Some(AqlValue::Array(arr)) => Ok(AqlValue::Array(arr.clone())),
                Some(val) => Ok(AqlValue::Array(vec![val.clone()])),
                None => Ok(AqlValue::Array(vec![])),
            },

            // ============ Date/Time Functions ============
            "DATE_NOW" => {
                let now = chrono::Utc::now().timestamp_millis();
                Ok(AqlValue::Number(serde_json::Number::from(now)))
            }
            "DATE_ISO8601" => {
                if let Some(AqlValue::Number(n)) = args.first() {
                    let ts = n.as_i64().unwrap_or(0);
                    let dt = chrono::DateTime::from_timestamp_millis(ts)
                        .unwrap_or_else(chrono::Utc::now);
                    Ok(AqlValue::String(dt.to_rfc3339()))
                } else {
                    let now = chrono::Utc::now();
                    Ok(AqlValue::String(now.to_rfc3339()))
                }
            }
            "DATE_TIMESTAMP" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                        Ok(AqlValue::Number(serde_json::Number::from(
                            dt.timestamp_millis(),
                        )))
                    } else {
                        Ok(AqlValue::Null)
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "DATE_YEAR" | "DATE_MONTH" | "DATE_DAY" | "DATE_HOUR" | "DATE_MINUTE"
            | "DATE_SECOND" | "DATE_DAYOFWEEK" | "DATE_DAYOFYEAR" => {
                use chrono::{Datelike, Timelike};
                let dt = match args.first() {
                    Some(AqlValue::Number(n)) => {
                        let ts = n.as_i64().unwrap_or(0);
                        chrono::DateTime::from_timestamp_millis(ts).unwrap_or_else(chrono::Utc::now)
                    }
                    Some(AqlValue::String(s)) => chrono::DateTime::parse_from_rfc3339(s)
                        .map(|d| d.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    _ => chrono::Utc::now(),
                };
                let value = match name.to_uppercase().as_str() {
                    "DATE_YEAR" => dt.year() as i64,
                    "DATE_MONTH" => dt.month() as i64,
                    "DATE_DAY" => dt.day() as i64,
                    "DATE_HOUR" => dt.hour() as i64,
                    "DATE_MINUTE" => dt.minute() as i64,
                    "DATE_SECOND" => dt.second() as i64,
                    "DATE_DAYOFWEEK" => dt.weekday().num_days_from_sunday() as i64,
                    "DATE_DAYOFYEAR" => dt.ordinal() as i64,
                    _ => 0,
                };
                Ok(AqlValue::Number(serde_json::Number::from(value)))
            }
            "DATE_ADD" | "DATE_SUBTRACT" => {
                use chrono::Duration;
                if let (
                    Some(AqlValue::Number(ts)),
                    Some(AqlValue::Number(amount)),
                    Some(AqlValue::String(unit)),
                ) = (args.first(), args.get(1), args.get(2))
                {
                    let timestamp = ts.as_i64().unwrap_or(0);
                    let amt = amount.as_i64().unwrap_or(0);
                    let duration = match unit.to_lowercase().as_str() {
                        "milliseconds" | "ms" => Duration::milliseconds(amt),
                        "seconds" | "s" => Duration::seconds(amt),
                        "minutes" | "m" | "i" => Duration::minutes(amt),
                        "hours" | "h" => Duration::hours(amt),
                        "days" | "d" => Duration::days(amt),
                        "weeks" | "w" => Duration::weeks(amt),
                        _ => Duration::milliseconds(0),
                    };
                    let new_ts = if name.to_uppercase() == "DATE_ADD" {
                        timestamp + duration.num_milliseconds()
                    } else {
                        timestamp - duration.num_milliseconds()
                    };
                    Ok(AqlValue::Number(serde_json::Number::from(new_ts)))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "DATE_DIFF" => {
                if let (
                    Some(AqlValue::Number(ts1)),
                    Some(AqlValue::Number(ts2)),
                    Some(AqlValue::String(unit)),
                ) = (args.first(), args.get(1), args.get(2))
                {
                    let t1 = ts1.as_i64().unwrap_or(0);
                    let t2 = ts2.as_i64().unwrap_or(0);
                    let diff_ms = t2 - t1;
                    let result = match unit.to_lowercase().as_str() {
                        "milliseconds" | "ms" => diff_ms,
                        "seconds" | "s" => diff_ms / 1000,
                        "minutes" | "m" | "i" => diff_ms / 60000,
                        "hours" | "h" => diff_ms / 3600000,
                        "days" | "d" => diff_ms / 86400000,
                        "weeks" | "w" => diff_ms / 604800000,
                        _ => diff_ms,
                    };
                    Ok(AqlValue::Number(serde_json::Number::from(result)))
                } else {
                    Ok(AqlValue::Null)
                }
            }

            // ============ Misc Functions ============
            "NOT_NULL" | "FIRST_LIST" | "FIRST_DOCUMENT" => {
                for arg in args {
                    if !matches!(arg, AqlValue::Null) {
                        match name.to_uppercase().as_str() {
                            "FIRST_LIST" if matches!(arg, AqlValue::Array(_)) => {
                                return Ok(arg.clone())
                            }
                            "FIRST_DOCUMENT" if matches!(arg, AqlValue::Object(_)) => {
                                return Ok(arg.clone())
                            }
                            "NOT_NULL" => return Ok(arg.clone()),
                            _ => continue,
                        }
                    }
                }
                Ok(AqlValue::Null)
            }
            "RANGE" => {
                if let (Some(AqlValue::Number(start)), Some(AqlValue::Number(end))) =
                    (args.first(), args.get(1))
                {
                    let s = start.as_i64().unwrap_or(0);
                    let e = end.as_i64().unwrap_or(0);
                    let step = args
                        .get(2)
                        .and_then(|v| {
                            if let AqlValue::Number(n) = v {
                                n.as_i64()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(1)
                        .max(1);
                    let range: Vec<AqlValue> = (s..=e)
                        .step_by(step as usize)
                        .map(|i| AqlValue::Number(serde_json::Number::from(i)))
                        .collect();
                    Ok(AqlValue::Array(range))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "DOCUMENT" => {
                // Return the document as-is (used for document lookup)
                Ok(args.first().cloned().unwrap_or(AqlValue::Null))
            }
            "V8" | "CALL" | "APPLY" => {
                // JavaScript function calls - not supported, return null
                Ok(AqlValue::Null)
            }
            "ASSERT" | "WARN" => {
                // Assertion/warning functions - just return the condition result
                Ok(args.first().cloned().unwrap_or(AqlValue::Null))
            }
            "PASSTHRU" | "NOOPT" | "SLEEP" => {
                // Pass-through functions
                Ok(args.first().cloned().unwrap_or(AqlValue::Null))
            }
            "UUID" => Ok(AqlValue::String(uuid::Uuid::new_v4().to_string())),
            "HASH" => {
                // Simple hash - return a numeric hash
                let input = format!("{:?}", args);
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                input.hash(&mut hasher);
                Ok(AqlValue::Number(serde_json::Number::from(
                    hasher.finish() as i64
                )))
            }

            // ============ Default Case ============
            _ => {
                // Unknown function - return null
                Ok(AqlValue::Null)
            }
        }
    }
}

impl Default for AqlQueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphrag_function_detection() {
        let engine = AqlQueryEngine::new();

        assert!(engine.is_graphrag_function_call(
            "FOR result IN GRAPHRAG_QUERY('kg', 'query', {}) RETURN result"
        ));
        assert!(
            engine.is_graphrag_function_call("FOR stats IN GRAPHRAG_GET_STATS('kg') RETURN stats")
        );
        assert!(!engine.is_graphrag_function_call("FOR doc IN documents RETURN doc"));
    }

    #[tokio::test]
    async fn test_regular_aql_query() {
        let engine = AqlQueryEngine::new();
        let result = engine
            .execute_query("FOR doc IN documents RETURN doc")
            .await;

        // Query should fail without storage backend
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Storage backend required"));
    }

    #[tokio::test]
    async fn test_graphrag_function_without_engine() {
        let engine = AqlQueryEngine::new();
        let result = engine
            .execute_query("FOR result IN GRAPHRAG_QUERY('kg', 'query', {}) RETURN result")
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("GraphRAG functions require"));
    }
}
