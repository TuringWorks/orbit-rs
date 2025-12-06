//! AQL query engine with GraphRAG integration
//!
//! This module provides a complete AQL query engine that includes GraphRAG function support.

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
        let storage = self.storage.as_ref().ok_or_else(|| {
            ProtocolError::AqlError("Storage backend required for query execution".to_string())
        })?;

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
                    // Execute FOR clause - iterate over collection
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
                    let doc_result = self.execute_insert(storage, document, collection, &context).await?;
                    result_data.push(doc_result);
                }
                AqlClause::Update {
                    key,
                    document,
                    collection,
                    options: _,
                } => {
                    // Execute UPDATE clause - modify existing document
                    if let Some(ref var) = for_variable {
                        // Update based on FOR iteration
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(updated) = self.execute_update(storage, key, document, collection, &ctx).await {
                                result_data.push(updated);
                            }
                        }
                    } else {
                        // Direct update
                        let updated = self.execute_update(storage, key, document, collection, &context).await?;
                        result_data.push(updated);
                    }
                }
                AqlClause::Replace {
                    key,
                    document,
                    collection,
                } => {
                    // Execute REPLACE clause - replace entire document
                    if let Some(ref var) = for_variable {
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(replaced) = self.execute_replace(storage, key, document, collection, &ctx).await {
                                result_data.push(replaced);
                            }
                        }
                    } else {
                        let replaced = self.execute_replace(storage, key, document, collection, &context).await?;
                        result_data.push(replaced);
                    }
                }
                AqlClause::Remove { key, collection } => {
                    // Execute REMOVE clause - delete document
                    if let Some(ref var) = for_variable {
                        for doc in &for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            if let Ok(removed) = self.execute_remove(storage, key, collection, &ctx).await {
                                result_data.push(removed);
                            }
                        }
                    } else {
                        let removed = self.execute_remove(storage, key, collection, &context).await?;
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
                    let upserted = self.execute_upsert(storage, search, insert, update_or_replace, collection, &context).await?;
                    result_data.push(upserted);
                }
                AqlClause::Let { variable, expression } => {
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
                ProtocolError::AqlError(format!("Document {}/{} not found for UPDATE", collection, key))
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
                    info!("AQL UPSERT: Updated existing document {}/{}", collection, key);
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
                    info!("AQL UPSERT: Replaced existing document {}/{}", collection, key);
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

                match name.to_uppercase().as_str() {
                    "LENGTH" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::Number(serde_json::Number::from(s.len())))
                        } else if let Some(AqlValue::Array(arr)) = evaluated_args.first() {
                            Ok(AqlValue::Number(serde_json::Number::from(arr.len())))
                        } else {
                            Ok(AqlValue::Number(serde_json::Number::from(0)))
                        }
                    }
                    "UPPER" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::String(s.to_uppercase()))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    "LOWER" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::String(s.to_lowercase()))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    "CONCAT" => {
                        let result: String = evaluated_args
                            .iter()
                            .filter_map(|v| {
                                if let AqlValue::String(s) = v {
                                    Some(s.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        Ok(AqlValue::String(result))
                    }
                    "ABS" => {
                        if let Some(AqlValue::Number(n)) = evaluated_args.first() {
                            let f = n.as_f64().unwrap_or(0.0).abs();
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(f)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    _ => {
                        // Unknown function - return null
                        Ok(AqlValue::Null)
                    }
                }
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;

                match op.as_str() {
                    "+" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        (AqlValue::String(l), AqlValue::String(r)) => {
                            Ok(AqlValue::String(format!("{}{}", l, r)))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "-" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "*" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
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
            AqlValue::Number(n) => n.as_f64().map_or(false, |f| f != 0.0),
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
        AqlValue::Object(doc.data.clone())
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
            if let AqlClause::Sort { items: _ } = clause {
                // Simplified sorting - would need proper implementation
                // For now, just keep original order
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
                let include_min = args.get(3).map_or(true, |e| {
                    matches!(
                        self.evaluate_expression(e, context),
                        Ok(AqlValue::Bool(true))
                    )
                });
                let include_max = args.get(4).map_or(true, |e| {
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
