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
use crate::protocols::common::graph_algorithms as graph_algo;
use crate::protocols::common::fts::{UnifiedFtsEngine, SharedFtsEngine, FtsQuery};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_client::OrbitClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use async_recursion::async_recursion;

/// Convert AqlValue to serde_json::Value for graph properties
fn aql_value_to_json(value: &AqlValue) -> serde_json::Value {
    match value {
        AqlValue::Null => serde_json::Value::Null,
        AqlValue::Bool(b) => serde_json::Value::Bool(*b),
        AqlValue::Number(n) => serde_json::Value::Number(n.clone()),
        AqlValue::String(s) => serde_json::Value::String(s.clone()),
        AqlValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(aql_value_to_json).collect())
        }
        AqlValue::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), aql_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        AqlValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
    }
}

/// Convert serde_json::Value to AqlValue for graph properties
fn json_to_aql_value(value: &serde_json::Value) -> AqlValue {
    match value {
        serde_json::Value::Null => AqlValue::Null,
        serde_json::Value::Bool(b) => AqlValue::Bool(*b),
        serde_json::Value::Number(n) => AqlValue::Number(n.clone()),
        serde_json::Value::String(s) => AqlValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            AqlValue::Array(arr.iter().map(json_to_aql_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: HashMap<String, AqlValue> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_aql_value(v)))
                .collect();
            AqlValue::Object(map)
        }
    }
}

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
    /// Full-text search engine
    fts_engine: Arc<dyn UnifiedFtsEngine>,
    /// Enable query profiling
    enable_profiling: bool,
}

/// Helper function to convert serde_json::Value to AqlValue
fn json_value_to_aql_value(json: &serde_json::Value) -> AqlValue {
    match json {
        serde_json::Value::Null => AqlValue::Null,
        serde_json::Value::Bool(b) => AqlValue::Bool(*b),
        serde_json::Value::Number(n) => AqlValue::Number(n.clone()),
        serde_json::Value::String(s) => AqlValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            AqlValue::Array(arr.iter().map(json_value_to_aql_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_value_to_aql_value(v));
            }
            AqlValue::Object(map)
        }
    }
}

impl AqlQueryEngine {
    /// Create new AQL query engine
    pub fn new() -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: None,
            fts_engine: Arc::new(SharedFtsEngine::default()),
            enable_profiling: false,
        }
    }

    /// Create new AQL query engine with storage
    pub fn with_storage(storage: Arc<AqlStorage>) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: Some(storage),
            fts_engine: Arc::new(SharedFtsEngine::default()),
            enable_profiling: false,
        }
    }

    /// Create new AQL query engine with GraphRAG support
    pub fn new_with_graphrag(orbit_client: OrbitClient) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: Some(AqlGraphRAGEngine::new(orbit_client)),
            storage: None,
            fts_engine: Arc::new(SharedFtsEngine::default()),
            enable_profiling: false,
        }
    }

    /// Enable or disable query profiling
    pub fn set_profiling(&mut self, enable: bool) {
        self.enable_profiling = enable;
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
        // Start timing for profiling
        let start_time = std::time::Instant::now();
        let profile_data = HashMap::new();

        // Check if query requires storage (has FOR, INSERT, UPDATE, REMOVE, REPLACE, UPSERT, or graph traversal)
        let needs_storage = query.clauses.iter().any(|clause| {
            matches!(
                clause,
                AqlClause::For { .. }
                    | AqlClause::Insert { .. }
                    | AqlClause::Update { .. }
                    | AqlClause::Replace { .. }
                    | AqlClause::Remove { .. }
                    | AqlClause::Upsert { .. }
                    | AqlClause::ForTraversal { .. }
                    | AqlClause::ForShortestPath { .. }
                    | AqlClause::ForKShortestPaths { .. }
                    | AqlClause::ForAllShortestPaths { .. }
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
                        let mut filtered_docs = Vec::new();
                        for doc in for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(&doc));
                            if self.evaluate_condition(condition, &ctx).await? {
                                filtered_docs.push(doc);
                            }
                        }
                        for_documents = filtered_docs;
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
                            let value = self.evaluate_expression(expression, &context).await?;
                            result_data.push(value);
                        }
                    } else {
                        // No FOR clause - evaluate expression directly
                        let value = self.evaluate_expression(expression, &context).await?;
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
                        let mut filtered_docs = Vec::new();
                        for doc in for_documents {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(&doc));
                            if self.evaluate_search_expression(expression, &ctx, analyzer_name).await? {
                                filtered_docs.push(doc);
                            }
                        }
                        for_documents = filtered_docs;
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
                    let value = self.evaluate_expression(expression, &context).await?;
                    context.insert(variable.clone(), value);
                }
                AqlClause::ForTraversal {
                    vertex_var,
                    edge_var,
                    path_var,
                    min_depth,
                    max_depth,
                    direction,
                    start_vertex,
                    graph_name,
                    options,
                    prune,
                } => {
                    // Execute graph traversal
                    let storage = storage_opt.ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Storage backend required for graph traversal".to_string(),
                        )
                    })?;

                    let traversal_results = self
                        .execute_for_traversal(
                            storage,
                            vertex_var,
                            edge_var,
                            path_var,
                            *min_depth,
                            *max_depth,
                            direction,
                            start_vertex,
                            graph_name,
                            options,
                            prune,
                            &context,
                        )
                        .await?;

                    // Store traversal results for subsequent clauses
                    // Each result is a binding (vertex_var, edge_var, path_var)
                    for_documents = traversal_results
                        .iter()
                        .map(|binding| {
                            // Convert binding to AqlDocument (approximation)
                            let mut data = HashMap::new();
                            for (k, v) in binding {
                                data.insert(k.clone(), v.clone());
                            }
                            AqlDocument::new("_traversal", uuid::Uuid::new_v4().to_string(), data)
                        })
                        .collect();

                    // Also add traversal bindings to context for RETURN to access
                    if let Some(first_binding) = traversal_results.first() {
                        for (k, v) in first_binding {
                            context.insert(k.clone(), v.clone());
                        }
                    }
                    for_variable = Some(vertex_var.clone());
                }
                AqlClause::ForShortestPath { path_var, query } => {
                    // Execute shortest path query
                    let storage = storage_opt.ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Storage backend required for shortest path query".to_string(),
                        )
                    })?;

                    let path_results = self
                        .execute_for_shortest_path(storage, path_var, query, &context)
                        .await?;

                    // Store path results
                    for_documents = path_results
                        .iter()
                        .map(|binding| {
                            let mut data = HashMap::new();
                            for (k, v) in binding {
                                data.insert(k.clone(), v.clone());
                            }
                            AqlDocument::new("_path", uuid::Uuid::new_v4().to_string(), data)
                        })
                        .collect();

                    if let Some(first_binding) = path_results.first() {
                        for (k, v) in first_binding {
                            context.insert(k.clone(), v.clone());
                        }
                    }
                    for_variable = Some(path_var.clone());
                }
                AqlClause::ForKShortestPaths { path_var, query } => {
                    // Execute K shortest paths query
                    let storage = storage_opt.ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Storage backend required for K shortest paths query".to_string(),
                        )
                    })?;

                    let path_results = self
                        .execute_for_k_shortest_paths(storage, path_var, query, &context)
                        .await?;

                    // Store path results
                    for_documents = path_results
                        .iter()
                        .map(|binding| {
                            let mut data = HashMap::new();
                            for (k, v) in binding {
                                data.insert(k.clone(), v.clone());
                            }
                            AqlDocument::new("_kpaths", uuid::Uuid::new_v4().to_string(), data)
                        })
                        .collect();

                    if let Some(first_binding) = path_results.first() {
                        for (k, v) in first_binding {
                            context.insert(k.clone(), v.clone());
                        }
                    }
                    for_variable = Some(path_var.clone());
                }
                AqlClause::ForAllShortestPaths {
                    path_var,
                    start_vertex,
                    target_vertex,
                    direction,
                    graph_source,
                } => {
                    // Execute all shortest paths query
                    let storage = storage_opt.ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Storage backend required for all shortest paths query".to_string(),
                        )
                    })?;

                    let path_results = self
                        .execute_for_all_shortest_paths(
                            storage,
                            path_var,
                            start_vertex,
                            target_vertex,
                            direction,
                            graph_source,
                            &context,
                        )
                        .await?;

                    // Store path results
                    for_documents = path_results
                        .iter()
                        .map(|binding| {
                            let mut data = HashMap::new();
                            for (k, v) in binding {
                                data.insert(k.clone(), v.clone());
                            }
                            AqlDocument::new("_allpaths", uuid::Uuid::new_v4().to_string(), data)
                        })
                        .collect();

                    if let Some(first_binding) = path_results.first() {
                        for (k, v) in first_binding {
                            context.insert(k.clone(), v.clone());
                        }
                    }
                    for_variable = Some(path_var.clone());
                }
                AqlClause::Collect {
                    groups,
                    into,
                    keep,
                    aggregates,
                    count_into,
                } => {
                    // Execute COLLECT clause - group and aggregate
                    let collected_results = self.execute_collect(
                        &for_documents,
                        &for_variable,
                        groups,
                        into,
                        keep,
                        aggregates,
                        count_into,
                        &context,
                    ).await?;

                    // COLLECT transforms the document stream
                    for_documents = collected_results;
                }
                _ => {
                    // Other clauses not yet implemented (WINDOW, etc.)
                    warn!("Unsupported clause type in AQL query execution");
                }
            }
        }

        // Apply SORT and LIMIT if present
        result_data = self.apply_sort_and_limit(&query.clauses, result_data).await?;

        // Calculate execution time
        let execution_duration = start_time.elapsed();

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "rows_returned".to_string(),
            AqlValue::Number(serde_json::Number::from(result_data.len())),
        );
        metadata.insert(
            "execution_time_ms".to_string(),
            AqlValue::Number(serde_json::Number::from(
                execution_duration.as_millis() as i64
            )),
        );

        // Add profiling data if enabled
        if self.enable_profiling {
            metadata.insert("profile".to_string(), AqlValue::Object(profile_data));
            metadata.insert(
                "query_clauses_count".to_string(),
                AqlValue::Number(serde_json::Number::from(query.clauses.len())),
            );
        }

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
        let doc_value = self.evaluate_expression(document_expr, context).await?;

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

        // Index document for FTS
        let index_name = collection;
        let indexes = self.fts_engine.list_indexes().await;
        if !indexes.iter().any(|i| i == index_name) {
            let _ = self.fts_engine.create_index(index_name, &[]).await;
        }

        let mut fields = HashMap::new();
        for (k, v) in &doc.data {
            if let AqlValue::String(s) = v {
                fields.insert(k.clone(), s.clone());
            } else {
                fields.insert(k.clone(), format!("{:?}", v));
            }
        }
        let doc_id = format!("{}/{}", collection, key);
        let _ = self.fts_engine.index_document(index_name, &doc_id, fields).await;

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
        let key = self.extract_document_key(key_expr, context).await?;

        // Evaluate the update expression to get the update data
        let update_value = self.evaluate_expression(document_expr, context).await?;

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

        // Update FTS index
        let index_name = collection;
        let indexes = self.fts_engine.list_indexes().await;
        if !indexes.iter().any(|i| i == index_name) {
             let _ = self.fts_engine.create_index(index_name, &[]).await;
        }

        let mut fields = HashMap::new();
        for (k, v) in &updated_doc.data {
            if let AqlValue::String(s) = v {
                 fields.insert(k.clone(), s.clone());
            } else {
                 fields.insert(k.clone(), format!("{:?}", v));
            }
        }
        let doc_id = format!("{}/{}", collection, key);
        let _ = self.fts_engine.index_document(index_name, &doc_id, fields).await;
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
        let key = self.extract_document_key(key_expr, context).await?;

        // Evaluate the replacement document expression
        let replace_value = self.evaluate_expression(document_expr, context).await?;

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

        // Update FTS index
        let index_name = collection;
        let indexes = self.fts_engine.list_indexes().await;
        if !indexes.iter().any(|i| i == index_name) {
             let _ = self.fts_engine.create_index(index_name, &[]).await;
        }

        let mut fields = HashMap::new();
        for (k, v) in &doc.data {
            if let AqlValue::String(s) = v {
                 fields.insert(k.clone(), s.clone());
            } else {
                 fields.insert(k.clone(), format!("{:?}", v));
            }
        }
        let doc_id = format!("{}/{}", collection, key);
        let _ = self.fts_engine.index_document(index_name, &doc_id, fields).await;
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
        let key = self.extract_document_key(key_expr, context).await?;

        // Get the document before deletion to return it
        let doc = storage.get_document(collection, &key).await?;

        // Delete the document
        let deleted = storage.delete_document(collection, &key).await?;

        if deleted {
            info!("AQL REMOVE: Deleted document {}/{}", collection, key);
            
            // Remove from FTS index
            let index_name = collection;
            let doc_id = format!("{}/{}", collection, key);
            let _ = self.fts_engine.remove_document(index_name, &doc_id).await;

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
        let search_value = self.evaluate_expression(search_expr, context).await?;

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
                    let update_value = self.evaluate_expression(update_expr, context).await?;
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
                    let replace_value = self.evaluate_expression(replace_expr, context).await?;
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
            let insert_value = self.evaluate_expression(insert_expr, context).await?;
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

    /// Execute ForTraversal clause - graph traversal
    #[allow(clippy::too_many_arguments)]
    async fn execute_for_traversal(
        &self,
        storage: &AqlStorage,
        vertex_var: &str,
        edge_var: &Option<String>,
        path_var: &Option<String>,
        min_depth: Option<u32>,
        max_depth: Option<u32>,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
        start_vertex: &str,
        graph_name: &Option<String>,
        options: &Option<crate::protocols::aql::aql_parser::TraversalOptions>,
        prune: &Option<crate::protocols::aql::aql_parser::PruneClause>,
        _context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<Vec<HashMap<String, AqlValue>>> {
        use crate::protocols::aql::aql_parser::TraversalOrder;

        // Build graph from storage
        let graph = self.build_graph_from_storage(storage, graph_name).await?;

        // Determine traversal order and uniqueness constraints
        let (use_bfs, unique_vertices, unique_edges) = if let Some(opts) = options {
            (
                opts.order == TraversalOrder::Bfs,
                opts.unique_vertices.clone(),
                opts.unique_edges.clone(),
            )
        } else {
            (
                true, // Default to BFS
                crate::protocols::aql::aql_parser::UniquenessLevel::None,
                crate::protocols::aql::aql_parser::UniquenessLevel::None,
            )
        };

        // Perform custom traversal with PRUNE and uniqueness support
        let max_depth_usize = max_depth.map(|d| d as usize);
        let min_depth_usize = min_depth.unwrap_or(0) as usize;

        let traversal_result = self.custom_traversal(
            &graph,
            start_vertex,
            direction,
            min_depth_usize,
            max_depth_usize,
            use_bfs,
            &unique_vertices,
            &unique_edges,
            prune,
        ).await?;

        // Build result: for each visited vertex, create a binding with vertex, edge, and path
        let mut results = Vec::new();
        for (vertex_id, parent_id, _depth) in &traversal_result {
            let mut binding = HashMap::new();

            // Add vertex data
            if let Some(node) = graph.nodes.get(vertex_id) {
                let mut vertex_obj = HashMap::new();
                vertex_obj.insert("_key".to_string(), AqlValue::String(vertex_id.clone()));
                vertex_obj.insert("_id".to_string(), AqlValue::String(vertex_id.clone()));
                for (k, v) in &node.properties {
                    vertex_obj.insert(k.clone(), json_to_aql_value(v));
                }
                binding.insert(vertex_var.to_string(), AqlValue::Object(vertex_obj));
            }

            // Add edge data if requested
            if let Some(edge_var_name) = edge_var {
                if let Some(parent) = parent_id {
                    let edge_obj = self.find_edge(&graph, parent, vertex_id, direction);
                    binding.insert(edge_var_name.clone(), edge_obj);
                } else {
                    binding.insert(edge_var_name.clone(), AqlValue::Null);
                }
            }

            // Add path data if requested
            if let Some(path_var_name) = path_var {
                // Build path from current position back to start
                let path =
                    self.build_path_from_traversal(&traversal_result, vertex_id, &graph, direction);
                binding.insert(path_var_name.clone(), path);
            }

            results.push(binding);
        }

        Ok(results)
    }

    /// Execute ForShortestPath clause - shortest path between two vertices
    async fn execute_for_shortest_path(
        &self,
        storage: &AqlStorage,
        path_var: &str,
        query: &crate::protocols::aql::aql_parser::ShortestPathQuery,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<Vec<HashMap<String, AqlValue>>> {
        // Build graph from storage
        let graph_name = match &query.graph_source {
            crate::protocols::aql::aql_parser::GraphSource::Graph(name) => Some(name.as_str()),
            _ => None,
        };
        let graph = self
            .build_graph_from_storage(storage, &graph_name.map(|s| s.to_string()))
            .await?;

        // Evaluate start and target vertex expressions
        let start_vertex = self.evaluate_expression(&query.start_vertex, context).await?;
        let target_vertex = self.evaluate_expression(&query.target_vertex, context).await?;

        let start_id = self.extract_vertex_id(&start_vertex)?;
        let target_id = self.extract_vertex_id(&target_vertex)?;

        // Find shortest path using Dijkstra's algorithm
        let path_result = graph_algo::dijkstra(&graph, &start_id, &target_id);

        // If no path found, return empty
        if !path_result.found {
            return Ok(vec![]);
        }

        // Build path object
        let mut path_obj = HashMap::new();

        // Add vertices in path
        let vertices: Vec<AqlValue> = path_result
            .path
            .iter()
            .map(|id| {
                let mut vertex = HashMap::new();
                vertex.insert("_key".to_string(), AqlValue::String(id.clone()));
                vertex.insert("_id".to_string(), AqlValue::String(id.clone()));
                if let Some(node) = graph.nodes.get(id) {
                    for (k, v) in &node.properties {
                        vertex.insert(k.clone(), json_to_aql_value(v));
                    }
                }
                AqlValue::Object(vertex)
            })
            .collect();

        path_obj.insert("vertices".to_string(), AqlValue::Array(vertices));

        // Add edges in path
        let mut edges = Vec::new();
        for i in 0..path_result.path.len().saturating_sub(1) {
            let from = &path_result.path[i];
            let to = &path_result.path[i + 1];
            let edge_obj = self.find_edge(&graph, from, to, &query.direction);
            edges.push(edge_obj);
        }
        path_obj.insert("edges".to_string(), AqlValue::Array(edges));

        // Add metadata
        path_obj.insert(
            "distance".to_string(),
            AqlValue::Number(
                serde_json::Number::from_f64(path_result.cost)
                    .unwrap_or(serde_json::Number::from(0)),
            ),
        );
        path_obj.insert(
            "length".to_string(),
            AqlValue::Number(serde_json::Number::from(path_result.length)),
        );

        let mut binding = HashMap::new();
        binding.insert(path_var.to_string(), AqlValue::Object(path_obj));

        Ok(vec![binding])
    }

    /// Execute ForKShortestPaths clause - K shortest paths
    async fn execute_for_k_shortest_paths(
        &self,
        storage: &AqlStorage,
        path_var: &str,
        query: &crate::protocols::aql::aql_parser::KShortestPathsQuery,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<Vec<HashMap<String, AqlValue>>> {
        // Build graph from storage
        let graph_name = match &query.graph_source {
            crate::protocols::aql::aql_parser::GraphSource::Graph(name) => Some(name.as_str()),
            _ => None,
        };
        let graph = self
            .build_graph_from_storage(storage, &graph_name.map(|s| s.to_string()))
            .await?;

        // Evaluate start and target vertex expressions
        let start_vertex = self.evaluate_expression(&query.start_vertex, context).await?;
        let target_vertex = self.evaluate_expression(&query.target_vertex, context).await?;

        let start_id = self.extract_vertex_id(&start_vertex)?;
        let target_id = self.extract_vertex_id(&target_vertex)?;

        // Find K shortest paths
        let k_paths_result =
            graph_algo::k_shortest_paths(&graph, &start_id, &target_id, query.k as usize);

        // Build result for each path
        let mut results = Vec::new();
        for path_result in k_paths_result.paths {
            if !path_result.found {
                continue;
            }

            let mut path_obj = HashMap::new();

            // Add vertices
            let vertices: Vec<AqlValue> = path_result
                .path
                .iter()
                .map(|id| {
                    let mut vertex = HashMap::new();
                    vertex.insert("_key".to_string(), AqlValue::String(id.clone()));
                    vertex.insert("_id".to_string(), AqlValue::String(id.clone()));
                    if let Some(node) = graph.nodes.get(id) {
                        for (k, v) in &node.properties {
                            vertex.insert(k.clone(), json_to_aql_value(v));
                        }
                    }
                    AqlValue::Object(vertex)
                })
                .collect();

            path_obj.insert("vertices".to_string(), AqlValue::Array(vertices));

            // Add edges
            let mut edges = Vec::new();
            for i in 0..path_result.path.len().saturating_sub(1) {
                let from = &path_result.path[i];
                let to = &path_result.path[i + 1];
                let edge_obj = self.find_edge(&graph, from, to, &query.direction);
                edges.push(edge_obj);
            }
            path_obj.insert("edges".to_string(), AqlValue::Array(edges));

            // Add metadata
            path_obj.insert(
                "distance".to_string(),
                AqlValue::Number(
                    serde_json::Number::from_f64(path_result.cost)
                        .unwrap_or(serde_json::Number::from(0)),
                ),
            );
            path_obj.insert(
                "length".to_string(),
                AqlValue::Number(serde_json::Number::from(path_result.length)),
            );

            let mut binding = HashMap::new();
            binding.insert(path_var.to_string(), AqlValue::Object(path_obj));
            results.push(binding);
        }

        Ok(results)
    }

    /// Execute ForAllShortestPaths clause - all shortest paths
    #[allow(clippy::too_many_arguments)]
    async fn execute_for_all_shortest_paths(
        &self,
        storage: &AqlStorage,
        path_var: &str,
        start_vertex: &AqlExpression,
        target_vertex: &AqlExpression,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
        graph_source: &crate::protocols::aql::aql_parser::GraphSource,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<Vec<HashMap<String, AqlValue>>> {
        // Build graph from storage
        let graph_name = match graph_source {
            crate::protocols::aql::aql_parser::GraphSource::Graph(name) => Some(name.as_str()),
            _ => None,
        };
        let graph = self
            .build_graph_from_storage(storage, &graph_name.map(|s| s.to_string()))
            .await?;

        // Evaluate start and target vertex expressions
        let start = self.evaluate_expression(start_vertex, context).await?;
        let target = self.evaluate_expression(target_vertex, context).await?;

        let start_id = self.extract_vertex_id(&start)?;
        let target_id = self.extract_vertex_id(&target)?;

        // Find all shortest paths
        let all_paths_result = graph_algo::all_shortest_paths(&graph, &start_id, &target_id);

        // Build result for each path
        let mut results = Vec::new();
        for path_vec in all_paths_result.paths {
            let mut path_obj = HashMap::new();

            // Add vertices
            let vertices: Vec<AqlValue> = path_vec
                .iter()
                .map(|id| {
                    let mut vertex = HashMap::new();
                    vertex.insert("_key".to_string(), AqlValue::String(id.clone()));
                    vertex.insert("_id".to_string(), AqlValue::String(id.clone()));
                    if let Some(node) = graph.nodes.get(id) {
                        for (k, v) in &node.properties {
                            vertex.insert(k.clone(), json_to_aql_value(v));
                        }
                    }
                    AqlValue::Object(vertex)
                })
                .collect();

            path_obj.insert("vertices".to_string(), AqlValue::Array(vertices));

            // Add edges
            let mut edges = Vec::new();
            for i in 0..path_vec.len().saturating_sub(1) {
                let from = &path_vec[i];
                let to = &path_vec[i + 1];
                let edge_obj = self.find_edge(&graph, from, to, direction);
                edges.push(edge_obj);
            }
            path_obj.insert("edges".to_string(), AqlValue::Array(edges));

            // Add length
            path_obj.insert(
                "length".to_string(),
                AqlValue::Number(serde_json::Number::from(path_vec.len().saturating_sub(1))),
            );

            let mut binding = HashMap::new();
            binding.insert(path_var.to_string(), AqlValue::Object(path_obj));
            results.push(binding);
        }

        Ok(results)
    }

    /// Build graph from storage by loading edge and vertex collections
    async fn build_graph_from_storage(
        &self,
        storage: &AqlStorage,
        _graph_name: &Option<String>,
    ) -> ProtocolResult<graph_algo::Graph> {
        let mut graph = graph_algo::Graph::new();

        // Get all collections
        let collections = storage.list_collections().await?;

        // Identify edge collections (collections whose documents have _from and _to)
        for collection_name in &collections {
            let documents = storage.get_collection_documents(collection_name).await?;

            for doc in documents {
                // Check if this is an edge document
                if doc.data.contains_key("_from") && doc.data.contains_key("_to") {
                    // This is an edge collection
                    let from = if let Some(AqlValue::String(f)) = doc.data.get("_from") {
                        f.clone()
                    } else {
                        continue;
                    };

                    let to = if let Some(AqlValue::String(t)) = doc.data.get("_to") {
                        t.clone()
                    } else {
                        continue;
                    };

                    let weight = doc
                        .data
                        .get("weight")
                        .or(doc.data.get("cost"))
                        .and_then(|v| {
                            if let AqlValue::Number(n) = v {
                                n.as_f64()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(1.0);

                    let edge_type = doc
                        .data
                        .get("_type")
                        .or(doc.data.get("type"))
                        .and_then(|v| {
                            if let AqlValue::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        });

                    graph.add_edge(from, to, weight, edge_type);
                } else {
                    // This is a vertex document
                    let id = doc.id.clone();
                    let mut properties = HashMap::new();
                    for (k, v) in &doc.data {
                        if !k.starts_with('_') {
                            properties.insert(k.clone(), aql_value_to_json(v));
                        }
                    }
                    graph.add_node(id, properties);
                }
            }
        }

        Ok(graph)
    }

    /// Find edge between two vertices
    fn find_edge(
        &self,
        graph: &graph_algo::Graph,
        from: &str,
        to: &str,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
    ) -> AqlValue {
        use crate::protocols::aql::aql_parser::TraversalDirection;

        let edge_info = match direction {
            TraversalDirection::Outbound => graph
                .adjacency
                .get(from)
                .and_then(|neighbors| neighbors.iter().find(|(neighbor, _, _)| neighbor == to)),
            TraversalDirection::Inbound => graph
                .reverse_adjacency
                .get(to)
                .and_then(|neighbors| neighbors.iter().find(|(neighbor, _, _)| neighbor == from)),
            TraversalDirection::Any => graph
                .adjacency
                .get(from)
                .and_then(|neighbors| neighbors.iter().find(|(neighbor, _, _)| neighbor == to))
                .or_else(|| {
                    graph.reverse_adjacency.get(to).and_then(|neighbors| {
                        neighbors.iter().find(|(neighbor, _, _)| neighbor == from)
                    })
                }),
        };

        if let Some((_, weight, edge_type)) = edge_info {
            let mut edge_obj = HashMap::new();
            edge_obj.insert("_from".to_string(), AqlValue::String(from.to_string()));
            edge_obj.insert("_to".to_string(), AqlValue::String(to.to_string()));
            edge_obj.insert(
                "weight".to_string(),
                AqlValue::Number(
                    serde_json::Number::from_f64(*weight).unwrap_or(serde_json::Number::from(1)),
                ),
            );
            if let Some(edge_type_str) = edge_type {
                edge_obj.insert("type".to_string(), AqlValue::String(edge_type_str.clone()));
            }
            AqlValue::Object(edge_obj)
        } else {
            AqlValue::Null
        }
    }

    /// Reconstruct full path from traversal result (kept for compatibility)
    #[allow(dead_code)]
    fn reconstruct_path(
        &self,
        traversal: &graph_algo::TraversalResult,
        vertex_id: &str,
        graph: &graph_algo::Graph,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
    ) -> AqlValue {
        // Build path from start to this vertex
        let mut path_vertices = Vec::new();
        let mut current = vertex_id;

        // Backtrack to find full path
        path_vertices.push(current.to_string());
        while let Some(parent) = traversal.parents.get(current) {
            path_vertices.push(parent.clone());
            current = parent;
        }
        path_vertices.reverse();

        // Build path object
        let mut path_obj = HashMap::new();

        // Add vertices
        let vertices: Vec<AqlValue> = path_vertices
            .iter()
            .map(|id| {
                let mut vertex = HashMap::new();
                vertex.insert("_key".to_string(), AqlValue::String(id.clone()));
                vertex.insert("_id".to_string(), AqlValue::String(id.clone()));
                if let Some(node) = graph.nodes.get(id) {
                    for (k, v) in &node.properties {
                        vertex.insert(k.clone(), json_to_aql_value(v));
                    }
                }
                AqlValue::Object(vertex)
            })
            .collect();

        path_obj.insert("vertices".to_string(), AqlValue::Array(vertices));

        // Add edges
        let mut edges = Vec::new();
        for i in 0..path_vertices.len().saturating_sub(1) {
            let from = &path_vertices[i];
            let to = &path_vertices[i + 1];
            let edge_obj = self.find_edge(graph, from, to, direction);
            edges.push(edge_obj);
        }
        path_obj.insert("edges".to_string(), AqlValue::Array(edges));

        AqlValue::Object(path_obj)
    }

    /// Custom graph traversal with PRUNE and uniqueness support
    /// Returns: Vec<(vertex_id, parent_id, depth)>
    #[allow(clippy::too_many_arguments)]
    async fn custom_traversal(
        &self,
        graph: &graph_algo::Graph,
        start_vertex: &str,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
        min_depth: usize,
        max_depth: Option<usize>,
        use_bfs: bool,
        unique_vertices: &crate::protocols::aql::aql_parser::UniquenessLevel,
        unique_edges: &crate::protocols::aql::aql_parser::UniquenessLevel,
        prune: &Option<crate::protocols::aql::aql_parser::PruneClause>,
    ) -> ProtocolResult<Vec<(String, Option<String>, usize)>> {
        use crate::protocols::aql::aql_parser::{TraversalDirection, UniquenessLevel};
        use std::collections::{HashSet, VecDeque};

        let mut results = Vec::new();
        let mut global_visited_vertices: HashSet<String> = HashSet::new();
        let mut global_visited_edges: HashSet<(String, String)> = HashSet::new();

        // Queue/Stack: (vertex_id, parent_id, depth, path_vertices, path_edges)
        let mut queue: VecDeque<(
            String,
            Option<String>,
            usize,
            Vec<String>,
            Vec<(String, String)>,
        )> = VecDeque::new();
        queue.push_back((
            start_vertex.to_string(),
            None,
            0,
            vec![start_vertex.to_string()],
            vec![],
        ));

        while let Some((current_id, parent_id, depth, path_vertices, path_edges)) = if use_bfs {
            queue.pop_front()
        } else {
            queue.pop_back()
        } {
            // Check max depth
            if let Some(max_d) = max_depth {
                if depth > max_d {
                    continue;
                }
            }

            // Add to results if within min_depth
            if depth >= min_depth {
                results.push((current_id.clone(), parent_id.clone(), depth));
            }

            // Check PRUNE condition
            if let Some(prune_clause) = prune {
                // Create context for condition evaluation
                let mut context = HashMap::new();

                // Add current vertex to context
                if let Some(node) = graph.nodes.get(&current_id) {
                    let mut vertex_obj = HashMap::new();
                    vertex_obj.insert("_key".to_string(), AqlValue::String(current_id.clone()));
                    vertex_obj.insert("_id".to_string(), AqlValue::String(current_id.clone()));
                    for (k, v) in &node.properties {
                        vertex_obj.insert(k.clone(), json_to_aql_value(v));
                    }

                    if let Some(ref var) = prune_clause.prune_var {
                        context.insert(var.clone(), AqlValue::Object(vertex_obj.clone()));
                    } else {
                        context.insert("vertex".to_string(), AqlValue::Object(vertex_obj));
                    }
                }

                // Evaluate PRUNE condition
                if self
                    .evaluate_condition(&prune_clause.condition, &context).await?
                {
                    // PRUNE: don't expand this vertex further
                    continue;
                }
            }

            // Don't expand beyond max_depth
            if let Some(max_d) = max_depth {
                if depth >= max_d {
                    continue;
                }
            }

            // Get neighbors based on direction
            let neighbors = match direction {
                TraversalDirection::Outbound => graph.get_outgoing_neighbors(&current_id),
                TraversalDirection::Inbound => graph.get_incoming_neighbors(&current_id),
                TraversalDirection::Any => graph.get_all_neighbors(&current_id),
            };

            // Expand to neighbors
            for (neighbor_id, _weight, _edge_type) in neighbors {
                // Check uniqueness constraints for vertices
                let should_visit_vertex = match unique_vertices {
                    UniquenessLevel::None => true,
                    UniquenessLevel::Path => !path_vertices.contains(&neighbor_id),
                    UniquenessLevel::Global => !global_visited_vertices.contains(&neighbor_id),
                };

                if !should_visit_vertex {
                    continue;
                }

                // Check uniqueness constraints for edges
                let edge = (current_id.clone(), neighbor_id.clone());
                let should_visit_edge = match unique_edges {
                    UniquenessLevel::None => true,
                    UniquenessLevel::Path => !path_edges.contains(&edge),
                    UniquenessLevel::Global => !global_visited_edges.contains(&edge),
                };

                if !should_visit_edge {
                    continue;
                }

                // Build new path
                let mut new_path_vertices = path_vertices.clone();
                new_path_vertices.push(neighbor_id.clone());

                let mut new_path_edges = path_edges.clone();
                new_path_edges.push(edge.clone());

                // Mark as visited if using global uniqueness
                if matches!(unique_vertices, UniquenessLevel::Global) {
                    global_visited_vertices.insert(neighbor_id.clone());
                }
                if matches!(unique_edges, UniquenessLevel::Global) {
                    global_visited_edges.insert(edge);
                }

                // Add to queue
                queue.push_back((
                    neighbor_id,
                    Some(current_id.clone()),
                    depth + 1,
                    new_path_vertices,
                    new_path_edges,
                ));
            }
        }

        Ok(results)
    }

    /// Build path from custom traversal result
    fn build_path_from_traversal(
        &self,
        traversal_result: &[(String, Option<String>, usize)],
        target_vertex: &str,
        graph: &graph_algo::Graph,
        direction: &crate::protocols::aql::aql_parser::TraversalDirection,
    ) -> AqlValue {
        // Find the path to target_vertex by backtracking through parents
        let mut path_vertices = vec![target_vertex.to_string()];
        let mut current = target_vertex;

        // Build parent map from traversal result
        let mut parent_map: HashMap<String, String> = HashMap::new();
        for (vertex_id, parent_id, _depth) in traversal_result {
            if let Some(parent) = parent_id {
                parent_map.insert(vertex_id.clone(), parent.clone());
            }
        }

        // Backtrack to find full path
        while let Some(parent) = parent_map.get(current) {
            path_vertices.push(parent.clone());
            current = parent;
        }
        path_vertices.reverse();

        // Build path object
        let mut path_obj = HashMap::new();

        // Add vertices
        let vertices: Vec<AqlValue> = path_vertices
            .iter()
            .map(|id| {
                let mut vertex = HashMap::new();
                vertex.insert("_key".to_string(), AqlValue::String(id.clone()));
                vertex.insert("_id".to_string(), AqlValue::String(id.clone()));
                if let Some(node) = graph.nodes.get(id) {
                    for (k, v) in &node.properties {
                        vertex.insert(k.clone(), json_to_aql_value(v));
                    }
                }
                AqlValue::Object(vertex)
            })
            .collect();

        path_obj.insert("vertices".to_string(), AqlValue::Array(vertices));

        // Add edges
        let mut edges = Vec::new();
        for i in 0..path_vertices.len().saturating_sub(1) {
            let from = &path_vertices[i];
            let to = &path_vertices[i + 1];
            let edge_obj = self.find_edge(graph, from, to, direction);
            edges.push(edge_obj);
        }
        path_obj.insert("edges".to_string(), AqlValue::Array(edges));

        AqlValue::Object(path_obj)
    }

    /// Extract vertex ID from a vertex expression result
    fn extract_vertex_id(&self, vertex: &AqlValue) -> ProtocolResult<String> {
        match vertex {
            AqlValue::String(id) => Ok(id.clone()),
            AqlValue::Object(obj) => {
                // Try _key first, then _id
                if let Some(AqlValue::String(id)) = obj.get("_key").or(obj.get("_id")) {
                    Ok(id.clone())
                } else {
                    Err(ProtocolError::AqlError(
                        "Vertex object must have _key or _id field".to_string(),
                    ))
                }
            }
            _ => Err(ProtocolError::AqlError(
                "Vertex must be a string ID or an object with _key/_id".to_string(),
            )),
        }
    }

    /// Execute COLLECT clause - grouping and aggregation
    #[allow(clippy::too_many_arguments)]
    async fn execute_collect(
        &self,
        documents: &[AqlDocument],
        for_variable: &Option<String>,
        groups: &[crate::protocols::aql::aql_parser::CollectGroup],
        into: &Option<String>,
        _keep: &Option<Vec<String>>,
        aggregates: &Option<Vec<crate::protocols::aql::aql_parser::CollectAggregate>>,
        count_into: &Option<String>,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<Vec<AqlDocument>> {
        use std::collections::BTreeMap;

        // Group documents by group expressions
        let mut grouped: BTreeMap<Vec<String>, Vec<AqlDocument>> = BTreeMap::new();

        for doc in documents {
            // Build context for this document
            let mut doc_context = context.clone();
            if let Some(ref var) = for_variable {
                doc_context.insert(var.clone(), self.document_to_value(doc));
            }

            // Evaluate group keys
            let mut group_key = Vec::new();
            for group in groups {
                let value = if let Some(ref expr) = group.expression {
                    self.evaluate_expression(expr, &doc_context).await?
                } else {
                    // If no expression, use the variable value directly
                    doc_context
                        .get(&group.variable)
                        .cloned()
                        .unwrap_or(AqlValue::Null)
                };

                // Convert to string for grouping
                group_key.push(format!("{:?}", value));
            }

            grouped.entry(group_key).or_default().push(doc.clone());
        }

        // Build result documents from groups
        let mut result_docs = Vec::new();

        for (group_keys, group_docs) in grouped {
            let mut result_data = HashMap::new();

            // Add group variables
            for (i, group) in groups.iter().enumerate() {
                if let Some(key_str) = group_keys.get(i) {
                    // Parse the debug string back (simplified - in production would preserve original values)
                    result_data.insert(group.variable.clone(), AqlValue::String(key_str.clone()));
                }
            }

            // Add INTO variable if specified (array of grouped documents)
            if let Some(into_var) = into {
                let group_array: Vec<AqlValue> = group_docs
                    .iter()
                    .map(|doc| {
                        let mut obj = HashMap::new();
                        for (k, v) in &doc.data {
                            obj.insert(k.clone(), v.clone());
                        }
                        AqlValue::Object(obj)
                    })
                    .collect();
                result_data.insert(into_var.clone(), AqlValue::Array(group_array));
            }

            // Add COUNT variable if specified
            if let Some(count_var) = count_into {
                result_data.insert(
                    count_var.clone(),
                    AqlValue::Number(serde_json::Number::from(group_docs.len())),
                );
            }

            // Compute aggregates if specified
            if let Some(agg_list) = aggregates {
                for agg in agg_list {
                    let agg_value = self.compute_aggregate(
                        &agg.function,
                        &agg.expression,
                        &group_docs,
                        for_variable,
                        context,
                    ).await?;
                    result_data.insert(agg.variable.clone(), agg_value);
                }
            }

            // Create result document
            let doc = AqlDocument::new("_collect", uuid::Uuid::new_v4().to_string(), result_data);
            result_docs.push(doc);
        }

        Ok(result_docs)
    }

    /// Compute aggregate function over a group of documents
    async fn compute_aggregate(
        &self,
        function: &crate::protocols::aql::aql_parser::AggregateFunction,
        expression: &AqlExpression,
        documents: &[AqlDocument],
        for_variable: &Option<String>,
        base_context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        use crate::protocols::aql::aql_parser::AggregateFunction;

        let mut values: Vec<AqlValue> = Vec::new();
        for doc in documents {
            let mut context = base_context.clone();
            if let Some(ref var) = for_variable {
                context.insert(var.clone(), self.document_to_value(doc));
            }
            values.push(self.evaluate_expression(expression, &context).await?);
        }

        match function {
            AggregateFunction::Count => {
                Ok(AqlValue::Number(serde_json::Number::from(values.len())))
            }
            AggregateFunction::Sum => {
                let sum: f64 = values
                    .iter()
                    .filter_map(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_f64()
                        } else {
                            None
                        }
                    })
                    .sum();
                Ok(AqlValue::Number(
                    serde_json::Number::from_f64(sum).unwrap_or(serde_json::Number::from(0)),
                ))
            }
            AggregateFunction::Avg => {
                let numbers: Vec<f64> = values
                    .iter()
                    .filter_map(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_f64()
                        } else {
                            None
                        }
                    })
                    .collect();
                if numbers.is_empty() {
                    Ok(AqlValue::Null)
                } else {
                    let avg = numbers.iter().sum::<f64>() / numbers.len() as f64;
                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(avg).unwrap_or(serde_json::Number::from(0)),
                    ))
                }
            }
            AggregateFunction::Min => values
                .iter()
                .min_by(|a, b| self.compare_aql_values(a, b))
                .cloned()
                .ok_or_else(|| ProtocolError::AqlError("MIN on empty group".to_string())),
            AggregateFunction::Max => values
                .iter()
                .max_by(|a, b| self.compare_aql_values(a, b))
                .cloned()
                .ok_or_else(|| ProtocolError::AqlError("MAX on empty group".to_string())),
            AggregateFunction::CountDistinct => {
                use std::collections::HashSet;
                let unique: HashSet<String> = values.iter().map(|v| format!("{:?}", v)).collect();
                Ok(AqlValue::Number(serde_json::Number::from(unique.len())))
            }
            AggregateFunction::CollectArray => Ok(AqlValue::Array(values)),
            AggregateFunction::CollectUnique => {
                use std::collections::HashSet;
                let mut seen = HashSet::new();
                let unique: Vec<AqlValue> = values
                    .into_iter()
                    .filter(|v| seen.insert(format!("{:?}", v)))
                    .collect();
                Ok(AqlValue::Array(unique))
            }
            _ => {
                // Stddev, Variance, etc. - not implemented yet
                Ok(AqlValue::Null)
            }
        }
    }

    /// Extract document key from key expression
    async fn extract_document_key(
        &self,
        key_expr: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<String> {
        let key_value = self.evaluate_expression(key_expr, context).await?;

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
    #[async_recursion]
    async fn evaluate_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        match expression {
            AqlExpression::Literal(val) => Ok(val.clone()),
            AqlExpression::Variable(name) => {
                if let Some(val) = context.get(name) {
                    Ok(val.clone())
                } else if name == "CURRENT" {
                    // Special case for CURRENT variable (used in array expansion)
                     // If it's not in context, it might be handled by the caller, but for now return Null
                    Ok(AqlValue::Null)
                } else {
                    // Check if it's a collection name or known identifier
                    // For now, treat as Null if not found
                    Ok(AqlValue::Null)
                }
            }
            AqlExpression::PropertyAccess { object, property } => {
                // If resolving from a variable, try to find it in context
                if let Some(AqlValue::Object(map)) = context.get(object) {
                     if let Some(val) = map.get(property) {
                         Ok(val.clone())
                     } else {
                         Ok(AqlValue::Null)
                     }
                } else if object == "doc" || object == "_" {
                     // Special variables "doc" or "_" often refer to the current document in context
                     // But usually context maps "doc" -> AqlValue.
                     if let Some(AqlValue::Object(map)) = context.get(object) {
                         map.get(property).cloned().ok_or(ProtocolError::AqlError(format!("Property not found: {}", property))).or(Ok(AqlValue::Null))
                     } else {
                         // Fallback: try to find "doc" in context if object name is "doc"
                         Ok(AqlValue::Null)
                     }
                } else {
                    // Try to resolve variable first
                    let var_val = self.evaluate_expression(&AqlExpression::Variable(object.clone()), context).await?;
                    if let AqlValue::Object(map) = var_val {
                        Ok(map.get(property).cloned().unwrap_or(AqlValue::Null))
                    } else {
                        Ok(AqlValue::Null)
                    }
                }
            }
            AqlExpression::Object(entries) => {
                let mut map = HashMap::new();
                for (k, v) in entries {
                    let val = self.evaluate_expression(v, context).await?;
                    map.insert(k.clone(), val);
                }
                Ok(AqlValue::Object(map))
            }
            AqlExpression::Array(items) => {
                let mut list = Vec::new();
                for item in items {
                    let val = self.evaluate_expression(item, context).await?;
                    list.push(val);
                }
                Ok(AqlValue::Array(list))
            }
            AqlExpression::FunctionCall { name, args } => {
                // Evaluate arguments first? No, evaluate_builtin_function might handle args differently (e.g. lazy)
                // But generally AQL functions take evaluated args.
                // evaluate_builtin_function implementation (Step 2819 refactor) takes `&[AqlExpression]`.
                // So we pass expressions directly!
                let mut evaluated_args = Vec::with_capacity(args.len());
                for arg in args {
                    evaluated_args.push(self.evaluate_expression(arg, context).await?);
                }
                self.evaluate_builtin_function(name, &evaluated_args, context).await
            },

            AqlExpression::UnaryOp { op, expr } => {
                let val = self.evaluate_expression(expr, context).await?;
                match op.as_str() {
                    "NOT" => match val {
                        AqlValue::Bool(b) => Ok(AqlValue::Bool(!b)),
                        AqlValue::Null => Ok(AqlValue::Bool(true)), // NOT null is true
                        _ => Ok(AqlValue::Bool(false)), // Any other value is "truthy", so NOT is false
                    },
                    "-" => match val {
                        AqlValue::Number(n) => {
                             if let Some(i) = n.as_i64() {
                                 Ok(AqlValue::Number(serde_json::Number::from(-i)))
                             } else if let Some(f) = n.as_f64() {
                                 Ok(AqlValue::Number(serde_json::Number::from_f64(-f).unwrap_or(serde_json::Number::from(0))))
                             } else {
                                 Ok(AqlValue::Null)
                             }
                        },
                        _ => Ok(AqlValue::Number(serde_json::Number::from(0))), // Should produce null/0
                    },
                    "+" => match val {
                         AqlValue::Number(n) => Ok(AqlValue::Number(n)),
                         _ => Ok(AqlValue::Number(serde_json::Number::from(0))),
                    }
                    _ => Err(ProtocolError::AqlError(format!("Unknown unary operator: {}", op))),
                }
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_val = self.evaluate_expression(left, context).await?;
                let right_val = self.evaluate_expression(right, context).await?;

                match op.as_str() {
                    "AND" => {
                        if self.is_truthy(&left_val) && self.is_truthy(&right_val) {
                            Ok(AqlValue::Bool(true))
                        } else {
                            Ok(AqlValue::Bool(false))
                        }
                    }
                    "OR" => {
                        if self.is_truthy(&left_val) || self.is_truthy(&right_val) {
                            Ok(AqlValue::Bool(true))
                        } else {
                            Ok(AqlValue::Bool(false))
                        }
                    }
                    "==" => Ok(AqlValue::Bool(left_val == right_val)),
                    "!=" => Ok(AqlValue::Bool(left_val != right_val)),
                    "<" => Ok(AqlValue::Bool(self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Less)),
                    "<=" => Ok(AqlValue::Bool(self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Greater)),
                    ">" => Ok(AqlValue::Bool(self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Greater)),
                    ">=" => Ok(AqlValue::Bool(self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Less)),
                    "+" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
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
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(l_f64 % r_f64)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    _ => Ok(AqlValue::Null),
                }
            }
        }
    }

    /// Evaluate an AQL condition
    #[async_recursion]
    async fn evaluate_condition(
        &self,
        condition: &AqlCondition,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        match condition {
            AqlCondition::Comparison {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate_expression(left, context).await?;
                let right_val = self.evaluate_expression(right, context).await?;

                Ok(match operator {
                    ComparisonOperator::Equals => left_val == right_val,
                    ComparisonOperator::NotEquals => left_val != right_val,
                    ComparisonOperator::Less => self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Less,
                    ComparisonOperator::LessOrEqual => self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Greater,
                    ComparisonOperator::Greater => self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Greater,
                    ComparisonOperator::GreaterOrEqual => self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Less,
                })
            }
            AqlCondition::Expression(expr) => self.evaluate_expression_as_bool(expr, context).await,
        }
    }
    /// Evaluate an expression and convert the result to a boolean
    #[async_recursion]
    async fn evaluate_expression_as_bool(
        &self,
        expr: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::aql_parser::AqlExpression;

        match expr {
            AqlExpression::BinaryOp { op, left, right } => {
                match op.as_str() {
                    "AND" | "&&" => {
                        let left_bool = self.evaluate_expression_as_bool(left, context).await?;
                        if !left_bool {
                            return Ok(false); // Short-circuit
                        }
                        self.evaluate_expression_as_bool(right, context).await
                    }
                    "OR" | "||" => {
                        let left_bool = self.evaluate_expression_as_bool(left, context).await?;
                        if left_bool {
                            return Ok(true); // Short-circuit
                        }
                        self.evaluate_expression_as_bool(right, context).await
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        // Comparison operators
                        let left_val = self.evaluate_expression(left, context).await?;
                        let right_val = self.evaluate_expression(right, context).await?;
                        let cmp_op = match op.as_str() {
                            "==" => ComparisonOperator::Equals,
                            "!=" => ComparisonOperator::NotEquals,
                            "<" => ComparisonOperator::Less,
                            "<=" => ComparisonOperator::LessOrEqual,
                            ">" => ComparisonOperator::Greater,
                            ">=" => ComparisonOperator::GreaterOrEqual,
                            _ => unreachable!(),
                        };
                        // Use compare_aql_values logic instead of compare_values if compare_values is missing
                        // Or just evaluate comparison directly
                        Ok(match cmp_op {
                            ComparisonOperator::Equals => left_val == right_val,
                            ComparisonOperator::NotEquals => left_val != right_val,
                            ComparisonOperator::Less => self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Less,
                            ComparisonOperator::LessOrEqual => self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Greater,
                            ComparisonOperator::Greater => self.compare_aql_values(&left_val, &right_val) == std::cmp::Ordering::Greater,
                            ComparisonOperator::GreaterOrEqual => self.compare_aql_values(&left_val, &right_val) != std::cmp::Ordering::Less,
                        })
                    }
                    _ => {
                        // Other binary ops - evaluate and check if truthy
                        let val = self.evaluate_expression(expr, context).await?;
                        Ok(self.is_truthy(&val))
                    }
                }
            }
            AqlExpression::UnaryOp { op, expr: inner } if op == "NOT" || op == "!" => {
                let inner_bool = self.evaluate_expression_as_bool(inner, context).await?;
                Ok(!inner_bool)
            }
            _ => {
                // For other expressions, evaluate and check truthiness
                let val = self.evaluate_expression(expr, context).await?;
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
    #[allow(dead_code)]
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
    async fn apply_sort_and_limit(
        &self,
        clauses: &[AqlClause],
        mut results: Vec<AqlValue>,
    ) -> ProtocolResult<Vec<AqlValue>> {
        use crate::protocols::aql::aql_parser::AqlClause;

        // Apply SORT if present
        for clause in clauses {
            if let AqlClause::Sort { items } = clause {
                if !items.is_empty() {
                    // Pre-calculate sort keys asynchronously because sort_by expects a synchronous closure
                    let mut results_with_keys = Vec::with_capacity(results.len());
                    for result in results {
                         let mut keys = Vec::with_capacity(items.len());
                         for item in items {
                              keys.push(self.extract_sort_value(&result, &item.expression).await);
                         }
                         results_with_keys.push((result, keys));
                    }

                    // Sort the results synchronously
                    results_with_keys.sort_by(|(_, keys_a), (_, keys_b)| {
                        for (i, item) in items.iter().enumerate() {
                            let val_a = &keys_a[i];
                            let val_b = &keys_b[i];

                            let cmp = self.compare_aql_values(val_a, val_b);
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
                    
                    // Extract results back
                    results = results_with_keys.into_iter().map(|(res, _)| res).collect();
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
    /// Evaluate a search expression for SEARCH clause
    #[async_recursion]
    async fn evaluate_search_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
        _analyzer: &str,
    ) -> ProtocolResult<bool> {
        match expression {
            AqlExpression::FunctionCall { name, args } => {
                self.evaluate_search_function(name, args, context).await
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_result = self.evaluate_search_expression(left, context, _analyzer).await?;
                let right_result = self.evaluate_search_expression(right, context, _analyzer).await?;

                match op.as_str() {
                    "AND" | "&&" => Ok(left_result && right_result),
                    "OR" | "||" => Ok(left_result || right_result),
                    _ => Ok(false),
                }
            }
            AqlExpression::UnaryOp { op, expr } if op == "NOT" || op == "!" => {
                let result = self.evaluate_search_expression(expr, context, _analyzer).await?;
                Ok(!result)
            }
            AqlExpression::Literal(value) => match value {
                AqlValue::Bool(b) => Ok(*b),
                _ => Ok(false),
            },
            // For conditions like doc.field == "value", evaluate as equality check
             _ => {
                 let val = self.evaluate_expression(expression, context).await?;
                 Ok(self.is_truthy(&val))
             }
        }
    }

    /// Evaluate a search function call
    async fn evaluate_search_function(
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
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let search_phrase = self.evaluate_expression(&args[1], context).await?;

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
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let prefix = self.evaluate_expression(&args[1], context).await?;

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
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let pattern = self.evaluate_expression(&args[1], context).await?;

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
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let term = self.evaluate_expression(&args[1], context).await?;
                let max_distance: usize = if args.len() >= 3 {
                    if let Ok(AqlValue::Number(n)) = self.evaluate_expression(&args[2], context).await {
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
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let min_val = self.evaluate_expression(&args[1], context).await?;
                let max_val = self.evaluate_expression(&args[2], context).await?;
                
                // Helper to evaluate bool expressions
                let include_min = if let Some(e) = args.get(3) { 
                    matches!(self.evaluate_expression(e, context).await, Ok(AqlValue::Bool(true)))
                } else { true };
                let include_max = if let Some(e) = args.get(4) { 
                    matches!(self.evaluate_expression(e, context).await, Ok(AqlValue::Bool(true)))
                } else { true };

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
                let value = self.evaluate_expression(&args[0], context).await;
                Ok(value.is_ok() && !matches!(value.unwrap(), AqlValue::Null))
            }
            "ANALYZER" => {
                // ANALYZER(expression, "analyzer_name") - apply analyzer and evaluate
                if args.is_empty() {
                    return Ok(false);
                }
                // For now, just evaluate the inner expression with default analyzer
                Box::pin(self.evaluate_search_expression(&args[0], context, "text_en")).await
            }
            "BOOST" => {
                // BOOST(expression, factor) - boost relevance (just evaluate expression for now)
                if args.is_empty() {
                    return Ok(false);
                }
                // Just evaluate the expression
                Box::pin(self.evaluate_search_expression(&args[0], context, "text_en")).await
            }
            "TOKENS" | "NGRAM_MATCH" | "NGRAM_SIMILARITY" => {
                // Token-based and n-gram functions - simplified implementation
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context).await?;
                let search_value = self.evaluate_expression(&args[1], context).await?;

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
    /// Public wrapper for evaluate_expression (for testing)
    #[cfg(test)]
    pub async fn evaluate_expression_public(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        self.evaluate_expression(expression, context).await
    }

    /// Public wrapper for levenshtein_distance (for testing)
    #[cfg(test)]
    pub fn levenshtein_distance_public(&self, a: &str, b: &str) -> usize {
        self.levenshtein_distance(a, b)
    }

    /// Extract a value from a result for sorting based on the expression
    /// Extract a value from a result for sorting based on the expression
    async fn extract_sort_value(&self, value: &AqlValue, expression: &AqlExpression) -> AqlValue {
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
                    .await
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

    /// Build a graph from context data (vertices and edges arrays)
    /// This extracts graph data from the query context for use with shared graph algorithms
    fn build_graph_from_context(
        &self,
        context: &HashMap<String, AqlValue>,
        graph_name: Option<&str>,
    ) -> graph_algo::Graph {
        let mut graph = graph_algo::Graph::new();

        // Look for vertices in context - check common variable names
        let vertex_keys = if let Some(name) = graph_name {
            vec![
                format!("{}_vertices", name),
                format!("{}Vertices", name),
                "vertices".to_string(),
                "nodes".to_string(),
            ]
        } else {
            vec!["vertices".to_string(), "nodes".to_string()]
        };

        for key in &vertex_keys {
            if let Some(AqlValue::Array(vertices)) = context.get(key) {
                for vertex in vertices {
                    if let AqlValue::Object(props) = vertex {
                        if let Some(AqlValue::String(id)) = props.get("_key").or(props.get("_id")) {
                            let mut properties = HashMap::new();
                            for (k, v) in props {
                                if k != "_key" && k != "_id" {
                                    properties.insert(k.clone(), aql_value_to_json(v));
                                }
                            }
                            graph.add_node(id.clone(), properties);
                        }
                    }
                }
                break;
            }
        }

        // Look for edges in context
        let edge_keys = if let Some(name) = graph_name {
            vec![
                format!("{}_edges", name),
                format!("{}Edges", name),
                "edges".to_string(),
            ]
        } else {
            vec!["edges".to_string()]
        };

        for key in &edge_keys {
            if let Some(AqlValue::Array(edges)) = context.get(key) {
                for edge in edges {
                    if let AqlValue::Object(props) = edge {
                        let from = props.get("_from").and_then(|v| {
                            if let AqlValue::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        });
                        let to = props.get("_to").and_then(|v| {
                            if let AqlValue::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        });

                        if let (Some(from_id), Some(to_id)) = (from, to) {
                            let weight = props
                                .get("weight")
                                .or(props.get("cost"))
                                .and_then(|v| {
                                    if let AqlValue::Number(n) = v {
                                        n.as_f64()
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(1.0);

                            let edge_type =
                                props.get("_type").or(props.get("type")).and_then(|v| {
                                    if let AqlValue::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                });

                            graph.add_edge(from_id, to_id, weight, edge_type);
                        }
                    }
                }
                break;
            }
        }

        graph
    }

    /// Evaluate a built-in function
    async fn evaluate_builtin_function(
        &self,
        name: &str,
        args: &[AqlValue],
        context: &HashMap<String, AqlValue>, // Added context for nested evaluations if needed
    ) -> ProtocolResult<AqlValue> {
        match name.to_uppercase().as_str() {
            // --- String Functions ---
            "LENGTH" | "CHAR_LENGTH" => {
                if let Some(AqlValue::String(s)) = args.first() {
                    Ok(AqlValue::Number(serde_json::Number::from(s.chars().count())))
                } else if let Some(AqlValue::Array(arr)) = args.first() {
                     Ok(AqlValue::Number(serde_json::Number::from(arr.len())))
                } else if let Some(AqlValue::Object(obj)) = args.first() {
                     Ok(AqlValue::Number(serde_json::Number::from(obj.len())))
                }
                 else {
                    Ok(AqlValue::Null)
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
                    Ok(AqlValue::String(
                        urlencoding::decode(s)
                            .map(|cow| cow.into_owned())
                            .unwrap_or_else(|_| s.clone()),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "FIND_LAST" => {
                if let (Some(AqlValue::String(haystack)), Some(AqlValue::String(needle))) =
                    (args.first(), args.get(1))
                {
                    let _pos = args.get(2).and_then(|v| {
                         if let AqlValue::Number(n) = v { n.as_i64() } else { None }
                    });
                     // Note: AQL FIND_LAST(str, search, start, end) behavior is strictly finding last occurrence
                     // Simple impl for now:
                     match haystack.rfind(needle) {
                         Some(p) => Ok(AqlValue::Number(serde_json::Number::from(p))),
                         None => Ok(AqlValue::Number(serde_json::Number::from(-1))),
                     }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "SUBSTITUTE" => {
                if args.len() < 3 {
                     return Ok(args.first().cloned().unwrap_or(AqlValue::Null));
                }
                if let (
                    Some(AqlValue::String(val)), 
                    Some(AqlValue::String(search)), 
                    Some(AqlValue::String(replace))
                ) = (args.first(), args.get(1), args.get(2)) {
                    let limit = args.get(3).and_then(|v| {
                        if let AqlValue::Number(n) = v { n.as_i64() } else { None }
                    });
                    
                    if let Some(l) = limit {
                         if l > 0 {
                             Ok(AqlValue::String(val.replacen(search, replace, l as usize)))
                         } else {
                             Ok(AqlValue::String(val.replace(search, replace)))
                         }
                    } else {
                        Ok(AqlValue::String(val.replace(search, replace)))
                    }
                } else {
                     Ok(AqlValue::Null)
                }
            }
            "SOUNDEX" => {
                 if let Some(AqlValue::String(s)) = args.first() {
                    let s_upper = s.to_uppercase();
                    let mut chars = s_upper.chars().filter(|c| c.is_ascii_alphabetic());
                    if let Some(first) = chars.next() {
                         let mut code = String::with_capacity(4);
                         code.push(first);
                         let mut last_digit = match first {
                            'B' | 'F' | 'P' | 'V' => '1',
                            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
                            'D' | 'T' => '3',
                            'L' => '4',
                            'M' | 'N' => '5',
                            'R' => '6',
                            _ => '0',
                         };
                         for c in chars {
                             let digit = match c {
                                'B' | 'F' | 'P' | 'V' => '1',
                                'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
                                'D' | 'T' => '3',
                                'L' => '4',
                                'M' | 'N' => '5',
                                'R' => '6',
                                _ => '0',
                             };
                             if digit != '0' && digit != last_digit {
                                 code.push(digit);
                                 last_digit = digit;
                             }
                             if code.len() == 4 { break; }
                         }
                         while code.len() < 4 { code.push('0'); }
                         Ok(AqlValue::String(code))
                    } else {
                         Ok(AqlValue::String(String::new()))
                    }
                 } else {
                     Ok(AqlValue::Null)
                 }
            }
            "UUID" => Ok(AqlValue::String(uuid::Uuid::new_v4().to_string())),

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
            "MEDIAN" | "PERCENTILE" => {
                if args.is_empty() { return Ok(AqlValue::Null); }
                let mut numbers = Vec::new();
                let percentile = if name.to_uppercase() == "PERCENTILE" {
                     args.get(1).and_then(|v| if let AqlValue::Number(n) = v { n.as_f64() } else { None }).unwrap_or(50.0)
                } else {
                     50.0
                };
                
                let source = if name.to_uppercase() == "PERCENTILE" { args.first() } else { Some(&AqlValue::Array(args.to_vec())) };
                
                if let Some(AqlValue::Array(arr)) = source {
                    for item in arr {
                        if let AqlValue::Number(n) = item {
                            numbers.push(n.as_f64().unwrap_or(0.0));
                        }
                    }
                } else if name.to_uppercase() == "MEDIAN" {
                     for arg in args {
                         if let AqlValue::Number(n) = arg {
                             numbers.push(n.as_f64().unwrap_or(0.0));
                         }
                     }
                }

                if numbers.is_empty() { return Ok(AqlValue::Null); }
                numbers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                
                // Simple percentile calc
                let k = (percentile / 100.0) * (numbers.len() - 1) as f64;
                let f = k.floor();
                let c = k.ceil();
                let idx_f = f as usize;
                let idx_c = c as usize;
                
                let result = if idx_f == idx_c {
                    numbers[idx_f]
                } else {
                    numbers[idx_f] * (c - k) + numbers[idx_c] * (k - f)
                };
                Ok(AqlValue::Number(serde_json::Number::from_f64(result).unwrap_or(serde_json::Number::from(0))))
            }
            "VARIANCE_SAMPLE" | "VARIANCE_POPULATION" | "STDDEV_SAMPLE" | "STDDEV_POPULATION" => {
                 let mut numbers = Vec::new();
                 let input_args = if args.len() == 1 {
                     if let AqlValue::Array(arr) = &args[0] {
                         arr.iter().collect::<Vec<_>>()
                     } else {
                         args.iter().collect()
                     }
                 } else {
                     args.iter().collect()
                 };

                 for arg in input_args {
                     if let AqlValue::Number(n) = arg {
                         numbers.push(n.as_f64().unwrap_or(0.0));
                     }
                 }
                 
                 if numbers.is_empty() { return Ok(AqlValue::Null); }
                 let n = numbers.len() as f64;
                 let mean = numbers.iter().sum::<f64>() / n;
                 let sum_sq_diff: f64 = numbers.iter().map(|x| (x - mean).powi(2)).sum();
                 
                 let is_population = name.to_uppercase().contains("POPULATION");
                 let divisor = if is_population { n } else { n - 1.0 };
                 
                 if divisor <= 0.0 { return Ok(AqlValue::Null); }
                 
                 let variance = sum_sq_diff / divisor;
                 
                 if name.to_uppercase().contains("STDDEV") {
                      Ok(AqlValue::Number(serde_json::Number::from_f64(variance.sqrt()).unwrap_or(serde_json::Number::from(0))))
                 } else {
                      Ok(AqlValue::Number(serde_json::Number::from_f64(variance).unwrap_or(serde_json::Number::from(0))))
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
            "OUTERSECTION" => {
                 let mut value_map = HashMap::new();
                 let mut counts = HashMap::new();
                 
                  for arg in args {
                     if let AqlValue::Array(arr) = arg {
                         for item in arr {
                             let key = format!("{:?}", item);
                             value_map.entry(key.clone()).or_insert_with(|| item.clone());
                             *counts.entry(key).or_insert(0) += 1;
                         }
                     }
                 }
                 
                 let result: Vec<AqlValue> = counts.into_iter()
                     .filter(|(_, count)| *count == 1)
                     .filter_map(|(key, _)| value_map.get(&key).cloned())
                     .collect();
                 Ok(AqlValue::Array(result))
            }
            "JACCARD" => {
                 if let (Some(AqlValue::Array(arr1)), Some(AqlValue::Array(arr2))) = (args.first(), args.get(1)) {
                     let set1: std::collections::HashSet<String> = arr1.iter().map(|v| format!("{:?}", v)).collect();
                     let set2: std::collections::HashSet<String> = arr2.iter().map(|v| format!("{:?}", v)).collect();
                     
                     let intersection_count = set1.intersection(&set2).count();
                     let union_count = set1.union(&set2).count();
                     
                     if union_count == 0 {
                         Ok(AqlValue::Number(serde_json::Number::from(0)))
                     } else {
                         Ok(AqlValue::Number(serde_json::Number::from_f64(intersection_count as f64 / union_count as f64).unwrap()))
                     }
                 } else {
                      Ok(AqlValue::Null)
                 }
            }
            "INTERLEAVE" => {
                 if args.is_empty() { return Ok(AqlValue::Array(vec![])); }
                 
                 let arrays: Vec<&Vec<AqlValue>> = args.iter().filter_map(|a| if let AqlValue::Array(arr) = a { Some(arr) } else { None }).collect();
                 if arrays.is_empty() { return Ok(AqlValue::Array(vec![])); }
                 
                 let max_len = arrays.iter().map(|a| a.len()).max().unwrap_or(0);
                 let mut result = Vec::new();
                 
                 for i in 0..max_len {
                     for arr in &arrays {
                         if let Some(val) = arr.get(i) {
                             result.push(val.clone());
                         }
                     }
                 }
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
            "MATCHES" => {
                 if let (Some(AqlValue::Object(doc)), Some(AqlValue::Object(example))) = (args.first(), args.get(1)) {
                     let matches = example.iter().all(|(k, v)| {
                         doc.get(k).map_or(false, |doc_v| doc_v == v)
                     });
                     Ok(AqlValue::Bool(matches))
                 } else if let (Some(AqlValue::Array(arr)), Some(AqlValue::Object(example))) = (args.first(), args.get(1)) {
                      let matches: Vec<AqlValue> = arr.iter().filter(|item| {
                           if let AqlValue::Object(doc) = item {
                               example.iter().all(|(k, v)|  doc.get(k).map_or(false, |doc_v| doc_v == v))
                           } else {
                               false
                           }
                      }).cloned().collect();
                      Ok(AqlValue::Array(matches))
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
            "IS_DATETIME" => Ok(AqlValue::Bool(matches!(
                args.first(),
                Some(AqlValue::DateTime(_))
            ))),
            "IS_KEY" => {
                 if let Some(AqlValue::String(s)) = args.first() {
                     let is_valid = s.chars().all(|c| c.is_ascii_alphanumeric() || "_-:.@()+,=;$!*'%".contains(c));
                     Ok(AqlValue::Bool(is_valid))
                 } else {
                     Ok(AqlValue::Bool(false))
                 }
            }
            "IS_ID" => {
                 if let Some(AqlValue::String(s)) = args.first() {
                     let parts: Vec<&str> = s.split('/').collect();
                     Ok(AqlValue::Bool(parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty()))
                 } else {
                     Ok(AqlValue::Bool(false))
                 }
            }
            "TO_INT" => {
                let result = match args.first() {
                     Some(AqlValue::Number(n)) => {
                         let f = n.as_f64().unwrap_or(0.0);
                         serde_json::Number::from(f as i64)
                     },
                     Some(AqlValue::String(s)) => {
                         let f = s.parse::<f64>().unwrap_or(0.0);
                         serde_json::Number::from(f as i64)
                     },
                     Some(AqlValue::Bool(true)) => serde_json::Number::from(1),
                     _ => serde_json::Number::from(0),
                };
                Ok(AqlValue::Number(result))
            }
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
            "DATE_FORMAT" => {
                 if let (Some(val), Some(AqlValue::String(fmt))) = (args.first(), args.get(1)) {
                      let dt = match val {
                          AqlValue::String(s) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).ok(),
                          AqlValue::Number(n) => chrono::DateTime::from_timestamp_millis(n.as_i64().unwrap_or(0)),
                          _ => None
                      };
                      if let Some(d) = dt {
                          Ok(AqlValue::String(d.format(fmt).to_string()))
                      } else {
                          Ok(AqlValue::Null)
                      }
                 } else {
                      Ok(AqlValue::Null)
                 }
            }
            "DATE_LEAPYEAR" => {
                 use chrono::Datelike;
                 let year = match args.first() {
                     Some(AqlValue::Number(n)) => Some(n.as_i64().unwrap_or(0) as i32),
                     Some(AqlValue::String(s)) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.year()).ok(),
                     _ => None
                 };
                 if let Some(y) = year {
                     let is_leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
                     Ok(AqlValue::Bool(is_leap))
                 } else {
                     Ok(AqlValue::Null)
                 }
            }
            "DATE_QUARTER" => {
                 use chrono::Datelike;
                 let dt = match args.first() {
                     Some(AqlValue::Number(n)) => chrono::DateTime::from_timestamp_millis(n.as_i64().unwrap_or(0)),
                     Some(AqlValue::String(s)) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).ok(),
                     _ => None
                 };
                 if let Some(d) = dt {
                     let q = (d.month() - 1) / 3 + 1;
                     Ok(AqlValue::Number(serde_json::Number::from(q)))
                 } else {
                     Ok(AqlValue::Null)
                 }
            }
            "DATE_DAYS_IN_MONTH" => {
                 use chrono::Datelike;
                 let dt = match args.first() {
                     Some(AqlValue::Number(n)) => chrono::DateTime::from_timestamp_millis(n.as_i64().unwrap_or(0)),
                     Some(AqlValue::String(s)) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).ok(),
                     _ => None
                 };
                 if let Some(d) = dt {
                     let year = d.year();
                     let month = d.month();
                     let days = match month {
                         1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                         4 | 6 | 9 | 11 => 30,
                         2 => if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) { 29 } else { 28 },
                         _ => 0,
                     };
                     Ok(AqlValue::Number(serde_json::Number::from(days)))
                 } else {
                     Ok(AqlValue::Null)
                 }
            }
            "DATE_TRUNC" => {
                // Truncate to precision: YEAR, MONTH, DAY, HOUR, MINUTE, SECOND
                Ok(args.first().cloned().unwrap_or(AqlValue::Null)) // Placeholder for now, date truncation is complex
            }
            "DATE_COMPARE" => {
                 if let (Some(_d1_val), Some(_d2_val), Some(AqlValue::String(_unit))) = (args.first(), args.get(1), args.get(2)) {
                      // Compare dates with unit
                      Ok(AqlValue::Bool(false)) // Placeholder
                 } else {
                      Ok(AqlValue::Bool(false))
                 }
            }
            "DATE_ISOWEEK" => {
                 use chrono::Datelike;
                 let dt = match args.first() {
                     Some(AqlValue::Number(n)) => chrono::DateTime::from_timestamp_millis(n.as_i64().unwrap_or(0)),
                     Some(AqlValue::String(s)) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).ok(),
                     _ => None
                 };
                 if let Some(d) = dt {
                     Ok(AqlValue::Number(serde_json::Number::from(d.iso_week().week())))
                 } else {
                     Ok(AqlValue::Null)
                 }
            }

            // ============ Graph Functions ============


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

            // ============ Geo Functions ============
            "GEO_POINT" => {
                // GEO_POINT(longitude, latitude) - Create a GeoJSON point
                if let (Some(AqlValue::Number(lon)), Some(AqlValue::Number(lat))) =
                    (args.first(), args.get(1))
                {
                    let lon_f = lon.as_f64().unwrap_or(0.0);
                    let lat_f = lat.as_f64().unwrap_or(0.0);
                    let mut point = HashMap::new();
                    point.insert("type".to_string(), AqlValue::String("Point".to_string()));
                    point.insert(
                        "coordinates".to_string(),
                        AqlValue::Array(vec![
                            AqlValue::Number(
                                serde_json::Number::from_f64(lon_f)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ),
                            AqlValue::Number(
                                serde_json::Number::from_f64(lat_f)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ),
                        ]),
                    );
                    Ok(AqlValue::Object(point))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "GEO_POLYGON" => {
                // GEO_POLYGON(points) - Create a GeoJSON polygon
                // points is array of [lon, lat] arrays or array of GeoJSON points
                if let Some(AqlValue::Array(points)) = args.first() {
                    let mut ring: Vec<AqlValue> = Vec::new();
                    for point in points {
                        match point {
                            AqlValue::Array(coords) if coords.len() >= 2 => {
                                ring.push(AqlValue::Array(coords.clone()));
                            }
                            AqlValue::Object(obj) => {
                                if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                    ring.push(AqlValue::Array(coords.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                    // Close the ring if not already closed
                    if !ring.is_empty() && ring.first() != ring.last() {
                        if let Some(first) = ring.first().cloned() {
                            ring.push(first);
                        }
                    }
                    let mut polygon = HashMap::new();
                    polygon.insert("type".to_string(), AqlValue::String("Polygon".to_string()));
                    polygon.insert(
                        "coordinates".to_string(),
                        AqlValue::Array(vec![AqlValue::Array(ring)]),
                    );
                    Ok(AqlValue::Object(polygon))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "GEO_LINESTRING" => {
                // GEO_LINESTRING(points) - Create a GeoJSON LineString
                if let Some(AqlValue::Array(points)) = args.first() {
                    let mut coords: Vec<AqlValue> = Vec::new();
                    for point in points {
                        match point {
                            AqlValue::Array(c) if c.len() >= 2 => {
                                coords.push(AqlValue::Array(c.clone()));
                            }
                            AqlValue::Object(obj) => {
                                if let Some(AqlValue::Array(c)) = obj.get("coordinates") {
                                    coords.push(AqlValue::Array(c.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut linestring = HashMap::new();
                    linestring.insert(
                        "type".to_string(),
                        AqlValue::String("LineString".to_string()),
                    );
                    linestring.insert("coordinates".to_string(), AqlValue::Array(coords));
                    Ok(AqlValue::Object(linestring))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "GEO_MULTIPOINT" => {
                // GEO_MULTIPOINT(points) - Create a GeoJSON MultiPoint
                if let Some(AqlValue::Array(points)) = args.first() {
                    let mut coords: Vec<AqlValue> = Vec::new();
                    for point in points {
                        match point {
                            AqlValue::Array(c) if c.len() >= 2 => {
                                coords.push(AqlValue::Array(c.clone()));
                            }
                            AqlValue::Object(obj) => {
                                if let Some(AqlValue::Array(c)) = obj.get("coordinates") {
                                    coords.push(AqlValue::Array(c.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut multipoint = HashMap::new();
                    multipoint.insert(
                        "type".to_string(),
                        AqlValue::String("MultiPoint".to_string()),
                    );
                    multipoint.insert("coordinates".to_string(), AqlValue::Array(coords));
                    Ok(AqlValue::Object(multipoint))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "DISTANCE" => {
                // DISTANCE(lat1, lon1, lat2, lon2) - Haversine distance in meters
                if args.len() >= 4 {
                    let lat1 = match &args[0] {
                        AqlValue::Number(n) => n.as_f64().unwrap_or(0.0),
                        _ => return Ok(AqlValue::Null),
                    };
                    let lon1 = match &args[1] {
                        AqlValue::Number(n) => n.as_f64().unwrap_or(0.0),
                        _ => return Ok(AqlValue::Null),
                    };
                    let lat2 = match &args[2] {
                        AqlValue::Number(n) => n.as_f64().unwrap_or(0.0),
                        _ => return Ok(AqlValue::Null),
                    };
                    let lon2 = match &args[3] {
                        AqlValue::Number(n) => n.as_f64().unwrap_or(0.0),
                        _ => return Ok(AqlValue::Null),
                    };

                    // Haversine formula
                    let r = 6371000.0; // Earth radius in meters
                    let lat1_rad = lat1.to_radians();
                    let lat2_rad = lat2.to_radians();
                    let delta_lat = (lat2 - lat1).to_radians();
                    let delta_lon = (lon2 - lon1).to_radians();

                    let a = (delta_lat / 2.0).sin().powi(2)
                        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
                    let c = 2.0 * a.sqrt().asin();
                    let distance = r * c;

                    Ok(AqlValue::Number(
                        serde_json::Number::from_f64(distance)
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "GEO_DISTANCE" => {
                // GEO_DISTANCE(geo1, geo2) - Distance between two GeoJSON objects in meters
                fn extract_coords(geo: &AqlValue) -> Option<(f64, f64)> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                // For Point: [lon, lat]
                                if coords.len() >= 2 {
                                    let lon = match &coords[0] {
                                        AqlValue::Number(n) => n.as_f64()?,
                                        _ => return None,
                                    };
                                    let lat = match &coords[1] {
                                        AqlValue::Number(n) => n.as_f64()?,
                                        _ => return None,
                                    };
                                    return Some((lat, lon));
                                }
                            }
                            None
                        }
                        AqlValue::Array(coords) if coords.len() >= 2 => {
                            let lon = match &coords[0] {
                                AqlValue::Number(n) => n.as_f64()?,
                                _ => return None,
                            };
                            let lat = match &coords[1] {
                                AqlValue::Number(n) => n.as_f64()?,
                                _ => return None,
                            };
                            Some((lat, lon))
                        }
                        _ => None,
                    }
                }

                if let (Some(geo1), Some(geo2)) = (args.first(), args.get(1)) {
                    if let (Some((lat1, lon1)), Some((lat2, lon2))) =
                        (extract_coords(geo1), extract_coords(geo2))
                    {
                        // Haversine formula
                        let r = 6371000.0;
                        let lat1_rad = lat1.to_radians();
                        let lat2_rad = lat2.to_radians();
                        let delta_lat = (lat2 - lat1).to_radians();
                        let delta_lon = (lon2 - lon1).to_radians();

                        let a = (delta_lat / 2.0).sin().powi(2)
                            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
                        let c = 2.0 * a.sqrt().asin();
                        let distance = r * c;

                        return Ok(AqlValue::Number(
                            serde_json::Number::from_f64(distance)
                                .unwrap_or(serde_json::Number::from(0)),
                        ));
                    }
                }
                Ok(AqlValue::Null)
            }
            "GEO_AREA" => {
                // GEO_AREA(geoJson) - Calculate area in square meters using spherical excess formula
                fn extract_polygon_ring(geo: &AqlValue) -> Option<Vec<(f64, f64)>> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                // Polygon: [[[lon, lat], ...]]
                                if let Some(AqlValue::Array(ring)) = coords.first() {
                                    let mut points = Vec::new();
                                    for point in ring {
                                        if let AqlValue::Array(p) = point {
                                            if p.len() >= 2 {
                                                let lon = match &p[0] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                let lat = match &p[1] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                points.push((lon, lat));
                                            }
                                        }
                                    }
                                    return Some(points);
                                }
                            }
                            None
                        }
                        _ => None,
                    }
                }

                if let Some(geo) = args.first() {
                    if let Some(ring) = extract_polygon_ring(geo) {
                        if ring.len() >= 3 {
                            // Shoelace formula for area (simplified, works for small areas)
                            // For more accuracy, use spherical excess formula
                            let r = 6371000.0; // Earth radius in meters
                            let mut area = 0.0;
                            let n = ring.len();
                            for i in 0..n {
                                let j = (i + 1) % n;
                                let (lon1, lat1) = ring[i];
                                let (lon2, lat2) = ring[j];
                                area += lon1.to_radians() * lat2.to_radians();
                                area -= lon2.to_radians() * lat1.to_radians();
                            }
                            area = area.abs() * r * r / 2.0;
                            return Ok(AqlValue::Number(
                                serde_json::Number::from_f64(area)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ));
                        }
                    }
                }
                Ok(AqlValue::Number(serde_json::Number::from(0)))
            }
            "GEO_CONTAINS" => {
                // GEO_CONTAINS(geoJsonA, geoJsonB) - Check if A contains B
                // Simplified: checks if point B is inside polygon A
                fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
                    let (px, py) = point;
                    let mut inside = false;
                    let n = polygon.len();
                    let mut j = n - 1;
                    for i in 0..n {
                        let (xi, yi) = polygon[i];
                        let (xj, yj) = polygon[j];
                        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi)
                        {
                            inside = !inside;
                        }
                        j = i;
                    }
                    inside
                }

                fn extract_point(geo: &AqlValue) -> Option<(f64, f64)> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                if coords.len() >= 2 {
                                    let lon = match &coords[0] {
                                        AqlValue::Number(n) => n.as_f64()?,
                                        _ => return None,
                                    };
                                    let lat = match &coords[1] {
                                        AqlValue::Number(n) => n.as_f64()?,
                                        _ => return None,
                                    };
                                    return Some((lon, lat));
                                }
                            }
                            None
                        }
                        _ => None,
                    }
                }

                fn extract_polygon(geo: &AqlValue) -> Option<Vec<(f64, f64)>> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                if let Some(AqlValue::Array(ring)) = coords.first() {
                                    let mut points = Vec::new();
                                    for point in ring {
                                        if let AqlValue::Array(p) = point {
                                            if p.len() >= 2 {
                                                let lon = match &p[0] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                let lat = match &p[1] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                points.push((lon, lat));
                                            }
                                        }
                                    }
                                    return Some(points);
                                }
                            }
                            None
                        }
                        _ => None,
                    }
                }

                if let (Some(geo_a), Some(geo_b)) = (args.first(), args.get(1)) {
                    // Check if polygon A contains point B
                    if let (Some(polygon), Some(point)) =
                        (extract_polygon(geo_a), extract_point(geo_b))
                    {
                        return Ok(AqlValue::Bool(point_in_polygon(point, &polygon)));
                    }
                }
                Ok(AqlValue::Bool(false))
            }
            "GEO_EQUALS" => {
                // GEO_EQUALS(geo1, geo2) - Check if two geo objects are equal
                Ok(AqlValue::Bool(args.first() == args.get(1)))
            }
            "GEO_INTERSECTS" => {
                // GEO_INTERSECTS(geo1, geo2) - Check if two geo objects intersect
                // Simplified: for point-polygon, checks containment; for polygon-polygon, checks any point overlap
                // Reuse point_in_polygon logic from GEO_CONTAINS
                fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
                    let (px, py) = point;
                    let mut inside = false;
                    let n = polygon.len();
                    let mut j = n - 1;
                    for i in 0..n {
                        let (xi, yi) = polygon[i];
                        let (xj, yj) = polygon[j];
                        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi)
                        {
                            inside = !inside;
                        }
                        j = i;
                    }
                    inside
                }

                fn extract_point(geo: &AqlValue) -> Option<(f64, f64)> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if obj.get("type") == Some(&AqlValue::String("Point".to_string())) {
                                if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                    if coords.len() >= 2 {
                                        let lon = match &coords[0] {
                                            AqlValue::Number(n) => n.as_f64()?,
                                            _ => return None,
                                        };
                                        let lat = match &coords[1] {
                                            AqlValue::Number(n) => n.as_f64()?,
                                            _ => return None,
                                        };
                                        return Some((lon, lat));
                                    }
                                }
                            }
                            None
                        }
                        _ => None,
                    }
                }

                fn extract_polygon(geo: &AqlValue) -> Option<Vec<(f64, f64)>> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if obj.get("type") == Some(&AqlValue::String("Polygon".to_string())) {
                                if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                    if let Some(AqlValue::Array(ring)) = coords.first() {
                                        let mut points = Vec::new();
                                        for point in ring {
                                            if let AqlValue::Array(p) = point {
                                                if p.len() >= 2 {
                                                    let lon = match &p[0] {
                                                        AqlValue::Number(n) => n.as_f64()?,
                                                        _ => return None,
                                                    };
                                                    let lat = match &p[1] {
                                                        AqlValue::Number(n) => n.as_f64()?,
                                                        _ => return None,
                                                    };
                                                    points.push((lon, lat));
                                                }
                                            }
                                        }
                                        return Some(points);
                                    }
                                }
                            }
                            None
                        }
                        _ => None,
                    }
                }

                if let (Some(geo1), Some(geo2)) = (args.first(), args.get(1)) {
                    // Point in polygon
                    if let (Some(point), Some(polygon)) =
                        (extract_point(geo1), extract_polygon(geo2))
                    {
                        return Ok(AqlValue::Bool(point_in_polygon(point, &polygon)));
                    }
                    if let (Some(polygon), Some(point)) =
                        (extract_polygon(geo1), extract_point(geo2))
                    {
                        return Ok(AqlValue::Bool(point_in_polygon(point, &polygon)));
                    }
                    // Polygon-polygon: check if any vertex of one is in the other
                    if let (Some(poly1), Some(poly2)) =
                        (extract_polygon(geo1), extract_polygon(geo2))
                    {
                        for p in &poly1 {
                            if point_in_polygon(*p, &poly2) {
                                return Ok(AqlValue::Bool(true));
                            }
                        }
                        for p in &poly2 {
                            if point_in_polygon(*p, &poly1) {
                                return Ok(AqlValue::Bool(true));
                            }
                        }
                    }
                }
                Ok(AqlValue::Bool(false))
            }
            "IS_IN_POLYGON" => {
                // IS_IN_POLYGON(polygon, latitude, longitude) or IS_IN_POLYGON(polygon, [lon, lat])
                fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
                    let (px, py) = point;
                    let mut inside = false;
                    let n = polygon.len();
                    let mut j = n - 1;
                    for i in 0..n {
                        let (xi, yi) = polygon[i];
                        let (xj, yj) = polygon[j];
                        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi)
                        {
                            inside = !inside;
                        }
                        j = i;
                    }
                    inside
                }

                fn extract_polygon_points(geo: &AqlValue) -> Option<Vec<(f64, f64)>> {
                    match geo {
                        AqlValue::Object(obj) => {
                            if let Some(AqlValue::Array(coords)) = obj.get("coordinates") {
                                if let Some(AqlValue::Array(ring)) = coords.first() {
                                    let mut points = Vec::new();
                                    for point in ring {
                                        if let AqlValue::Array(p) = point {
                                            if p.len() >= 2 {
                                                let lon = match &p[0] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                let lat = match &p[1] {
                                                    AqlValue::Number(n) => n.as_f64()?,
                                                    _ => return None,
                                                };
                                                points.push((lon, lat));
                                            }
                                        }
                                    }
                                    return Some(points);
                                }
                            }
                            None
                        }
                        AqlValue::Array(arr) => {
                            // Direct array of points
                            let mut points = Vec::new();
                            for point in arr {
                                if let AqlValue::Array(p) = point {
                                    if p.len() >= 2 {
                                        let lon = match &p[0] {
                                            AqlValue::Number(n) => n.as_f64()?,
                                            _ => return None,
                                        };
                                        let lat = match &p[1] {
                                            AqlValue::Number(n) => n.as_f64()?,
                                            _ => return None,
                                        };
                                        points.push((lon, lat));
                                    }
                                }
                            }
                            if points.is_empty() {
                                None
                            } else {
                                Some(points)
                            }
                        }
                        _ => None,
                    }
                }

                if let Some(polygon) = args.first() {
                    if let Some(poly_points) = extract_polygon_points(polygon) {
                        // Check if second arg is array [lon, lat] or separate lat, lon args
                        if args.len() >= 3 {
                            // IS_IN_POLYGON(polygon, lat, lon)
                            if let (Some(AqlValue::Number(lat)), Some(AqlValue::Number(lon))) =
                                (args.get(1), args.get(2))
                            {
                                let lat_f = lat.as_f64().unwrap_or(0.0);
                                let lon_f = lon.as_f64().unwrap_or(0.0);
                                return Ok(AqlValue::Bool(point_in_polygon(
                                    (lon_f, lat_f),
                                    &poly_points,
                                )));
                            }
                        } else if args.len() >= 2 {
                            // IS_IN_POLYGON(polygon, [lon, lat])
                            if let Some(AqlValue::Array(coords)) = args.get(1) {
                                if coords.len() >= 2 {
                                    if let (
                                        Some(AqlValue::Number(lon)),
                                        Some(AqlValue::Number(lat)),
                                    ) = (coords.first(), coords.get(1))
                                    {
                                        let lon_f = lon.as_f64().unwrap_or(0.0);
                                        let lat_f = lat.as_f64().unwrap_or(0.0);
                                        return Ok(AqlValue::Bool(point_in_polygon(
                                            (lon_f, lat_f),
                                            &poly_points,
                                        )));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(AqlValue::Bool(false))
            }


            "FULLTEXT" => {
                // FULLTEXT(collection, attribute, query) - Full-text search
                if args.len() < 3 {
                    return Err(ProtocolError::AqlError(
                        "FULLTEXT expects at least 3 arguments: collection, attribute, query"
                            .to_string(),
                    ));
                }

                if let (
                    Some(AqlValue::String(collection_name)),
                    Some(AqlValue::String(attribute)),
                    Some(AqlValue::String(query_text)),
                ) = (args.get(0), args.get(1), args.get(2))
                {
                    // Check if collection exists
                    if let Some(storage) = &self.storage {
                        if storage.get_collection(collection_name).await?.is_none() {
                            return Err(ProtocolError::AqlError(format!(
                                "Collection '{}' not found",
                                collection_name
                            )));
                        }
                    }

                    // Create FTS query
                    let mut query = FtsQuery::default();
                    query.must_terms = query_text.split_whitespace().map(|s| s.to_string()).collect();
                    query.fields = Some(vec![attribute.clone()]);
                    let index_name = collection_name;
                    
                    match self.fts_engine.search(index_name, &query).await {
                        Ok(results) => {
                            let mut docs = Vec::new();
                            if let Some(storage) = &self.storage {
                                for result in results {
                                    let parts: Vec<&str> = result.doc_id.split('/').collect();
                                    if parts.len() == 2 {
                                        if let Some(doc) = storage.get_document(parts[0], parts[1]).await? {
                                            docs.push(AqlValue::Object(doc.data));
                                        }
                                    }
                                }
                            }
                            Ok(AqlValue::Array(docs))
                        }
                        Err(_) => Ok(AqlValue::Array(vec![]))
                    }
                } else {
                     Ok(AqlValue::Array(vec![]))
                }
            }
            "TOKENS" => {
                // TOKENS(input, analyzer) - Tokenize text using analyzer
                if let Some(AqlValue::String(text)) = args.first() {
                    // Simple whitespace tokenization
                    let tokens: Vec<AqlValue> = text
                        .split_whitespace()
                        .map(|s| AqlValue::String(s.to_lowercase()))
                        .collect();
                    Ok(AqlValue::Array(tokens))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "PHRASE" => {
                // PHRASE(tokens, text, analyzer) - Build phrase for search
                if let Some(AqlValue::String(text)) = args.get(1).or(args.first()) {
                    Ok(AqlValue::String(text.clone()))
                } else {
                    Ok(AqlValue::String(String::new()))
                }
            }
            "ANALYZER" => {
                // ANALYZER(expr, analyzer) - Set analyzer for expression
                // Just return the expression as-is
                Ok(args.first().cloned().unwrap_or(AqlValue::Null))
            }
            "BOOST" => {
                // BOOST(expr, factor) - Boost relevance of expression
                // Just return the expression as-is (boosting affects scoring)
                Ok(args.first().cloned().unwrap_or(AqlValue::Null))
            }
            "BM25" | "TFIDF" => {
                // BM25(doc) / TFIDF(doc) - Get relevance score
                // Return 0 as default score
                Ok(AqlValue::Number(serde_json::Number::from(0)))
            }

            // ============ Graph Functions ============
            // These functions use the shared graph_algorithms module from protocols/common
            // Graph data is extracted from context (vertices, edges arrays)
            "SHORTEST_PATH" => {
                // SHORTEST_PATH(startVertex, targetVertex, options) - Find shortest path
                // Uses shared Dijkstra implementation from protocols/common/graph_algorithms
                let graph = self.build_graph_from_context(context, None);

                let start = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    let path_result = graph_algo::dijkstra(&graph, start_id, target_id);

                    let mut result = HashMap::new();
                    result.insert(
                        "vertices".to_string(),
                        AqlValue::Array(
                            path_result
                                .path
                                .iter()
                                .map(|id| AqlValue::String(id.clone()))
                                .collect(),
                        ),
                    );
                    result.insert("edges".to_string(), AqlValue::Array(vec![]));
                    result.insert(
                        "distance".to_string(),
                        AqlValue::Number(
                            serde_json::Number::from_f64(path_result.cost)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ),
                    );
                    result.insert("found".to_string(), AqlValue::Bool(path_result.found));
                    Ok(AqlValue::Object(result))
                } else {
                    let mut result = HashMap::new();
                    result.insert("vertices".to_string(), AqlValue::Array(vec![]));
                    result.insert("edges".to_string(), AqlValue::Array(vec![]));
                    result.insert(
                        "distance".to_string(),
                        AqlValue::Number(serde_json::Number::from(0)),
                    );
                    result.insert("found".to_string(), AqlValue::Bool(false));
                    Ok(AqlValue::Object(result))
                }
            }
            "K_SHORTEST_PATHS" => {
                // K_SHORTEST_PATHS(startVertex, targetVertex, k, options) - Find k shortest paths
                let graph = self.build_graph_from_context(context, None);

                let start = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let k = args
                    .get(2)
                    .and_then(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_u64().map(|n| n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1);

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    let k_result = graph_algo::k_shortest_paths(&graph, start_id, target_id, k);

                    let paths: Vec<AqlValue> = k_result
                        .paths
                        .iter()
                        .map(|path_result| {
                            let mut obj = HashMap::new();
                            obj.insert(
                                "vertices".to_string(),
                                AqlValue::Array(
                                    path_result
                                        .path
                                        .iter()
                                        .map(|id| AqlValue::String(id.clone()))
                                        .collect(),
                                ),
                            );
                            obj.insert(
                                "cost".to_string(),
                                AqlValue::Number(
                                    serde_json::Number::from_f64(path_result.cost)
                                        .unwrap_or_else(|| serde_json::Number::from(0)),
                                ),
                            );
                            AqlValue::Object(obj)
                        })
                        .collect();

                    Ok(AqlValue::Array(paths))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "K_PATHS" => {
                // K_PATHS(startVertex, targetVertex, k, options) - Alias for K_SHORTEST_PATHS
                let graph = self.build_graph_from_context(context, None);

                let start = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let k = args
                    .get(2)
                    .and_then(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_u64().map(|n| n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1);

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    let k_result = graph_algo::k_shortest_paths(&graph, start_id, target_id, k);
                    let paths: Vec<AqlValue> = k_result
                        .paths
                        .iter()
                        .map(|path_result| {
                            let mut obj = HashMap::new();
                            obj.insert(
                                "vertices".to_string(),
                                AqlValue::Array(
                                    path_result
                                        .path
                                        .iter()
                                        .map(|id| AqlValue::String(id.clone()))
                                        .collect(),
                                ),
                            );
                            obj.insert(
                                "cost".to_string(),
                                AqlValue::Number(
                                    serde_json::Number::from_f64(path_result.cost)
                                        .unwrap_or_else(|| serde_json::Number::from(0)),
                                ),
                            );
                            AqlValue::Object(obj)
                        })
                        .collect();
                    Ok(AqlValue::Array(paths))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "ALL_SHORTEST_PATHS" => {
                // ALL_SHORTEST_PATHS(startVertex, targetVertex, options) - Find all shortest paths
                let graph = self.build_graph_from_context(context, None);

                let start = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    let all_result = graph_algo::all_shortest_paths(&graph, start_id, target_id);

                    let paths: Vec<AqlValue> = all_result
                        .paths
                        .iter()
                        .map(|path| {
                            AqlValue::Array(
                                path.iter().map(|id| AqlValue::String(id.clone())).collect(),
                            )
                        })
                        .collect();

                    let mut result = HashMap::new();
                    result.insert("paths".to_string(), AqlValue::Array(paths));
                    result.insert(
                        "count".to_string(),
                        AqlValue::Number(serde_json::Number::from(all_result.paths.len())),
                    );
                    Ok(AqlValue::Object(result))
                } else {
                    let mut result = HashMap::new();
                    result.insert("paths".to_string(), AqlValue::Array(vec![]));
                    result.insert(
                        "count".to_string(),
                        AqlValue::Number(serde_json::Number::from(0)),
                    );
                    Ok(AqlValue::Object(result))
                }
            }
            "GRAPH_VERTICES" => {
                // GRAPH_VERTICES(graphName, startVertex, options) - Get vertices from BFS traversal
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let graph = self.build_graph_from_context(context, graph_name);

                let start = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let max_depth = args.get(2).and_then(|v| {
                    if let AqlValue::Object(opts) = v {
                        opts.get("maxDepth").and_then(|d| {
                            if let AqlValue::Number(n) = d {
                                n.as_u64().map(|n| n as usize)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                });

                if let Some(start_id) = start {
                    let traversal = graph_algo::bfs_traversal(&graph, start_id, max_depth);
                    let vertices: Vec<AqlValue> = traversal
                        .visited
                        .iter()
                        .map(|id| {
                            if let Some(node) = graph.nodes.get(id) {
                                let mut obj = HashMap::new();
                                obj.insert("_key".to_string(), AqlValue::String(id.clone()));
                                for (k, v) in &node.properties {
                                    obj.insert(k.clone(), json_to_aql_value(v));
                                }
                                AqlValue::Object(obj)
                            } else {
                                AqlValue::String(id.clone())
                            }
                        })
                        .collect();
                    Ok(AqlValue::Array(vertices))
                } else {
                    // Return all vertices if no start vertex specified
                    let vertices: Vec<AqlValue> = graph
                        .nodes
                        .iter()
                        .map(|(id, node)| {
                            let mut obj = HashMap::new();
                            obj.insert("_key".to_string(), AqlValue::String(id.clone()));
                            for (k, v) in &node.properties {
                                obj.insert(k.clone(), json_to_aql_value(v));
                            }
                            AqlValue::Object(obj)
                        })
                        .collect();
                    Ok(AqlValue::Array(vertices))
                }
            }
            "GRAPH_EDGES" => {
                // GRAPH_EDGES(graphName, startVertex, options) - Get edges from traversal
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let graph = self.build_graph_from_context(context, graph_name);

                let start = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let Some(start_id) = start {
                    // Get neighbors (edges) from start vertex
                    let neighbor_result = graph_algo::get_neighbors(
                        &graph,
                        start_id,
                        graph_algo::NeighborDirection::Outgoing,
                    );
                    let edges: Vec<AqlValue> = neighbor_result
                        .edges
                        .iter()
                        .map(|edge| {
                            let mut obj = HashMap::new();
                            obj.insert("_from".to_string(), AqlValue::String(edge.from.clone()));
                            obj.insert("_to".to_string(), AqlValue::String(edge.to.clone()));
                            obj.insert(
                                "weight".to_string(),
                                AqlValue::Number(
                                    serde_json::Number::from_f64(edge.weight)
                                        .unwrap_or_else(|| serde_json::Number::from(1)),
                                ),
                            );
                            if let Some(ref et) = edge.edge_type {
                                obj.insert("_type".to_string(), AqlValue::String(et.clone()));
                            }
                            AqlValue::Object(obj)
                        })
                        .collect();
                    Ok(AqlValue::Array(edges))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "GRAPH_NEIGHBORS" => {
                // GRAPH_NEIGHBORS(graphName, startVertex, options) - Get neighbors of vertex
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                
                // Fallback: build from context (for compatibility with existing tests that might use context variables?)
                // But primarily we want storage.
                // Let's try storage first.
                let mut neighbor_ids = Vec::new();
                if let Some(name) = graph_name {
                    if let Some(storage) = &self.storage {
                        if let Ok(edges) = storage.get_collection_documents(name).await {
                         // Graph name is treated as edge collection name
                         let start_node = args.get(1).and_then(|v| if let AqlValue::String(s) = v { Some(s.clone()) } else { None }).unwrap_or_default();
                         
                         // Parse direction
                         let direction = args.get(2).and_then(|v| {
                             if let AqlValue::Object(opts) = v {
                                 opts.get("direction").and_then(|d| if let AqlValue::String(s) = d { Some(s.clone()) } else { None })
                             } else {
                                 None
                             }
                         }).unwrap_or("ANY".to_string());
                         
                         let dir_upper = direction.to_uppercase();
                         
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             
                             if dir_upper == "OUTBOUND" {
                                 if from == start_node {
                                     neighbor_ids.push(AqlValue::String(to.to_string()));
                                 }
                             } else if dir_upper == "INBOUND" {
                                 if to == start_node {
                                     neighbor_ids.push(AqlValue::String(from.to_string()));
                                 }
                             } else { // ANY
                                 if from == start_node {
                                     neighbor_ids.push(AqlValue::String(to.to_string()));
                                 } else if to == start_node {
                                     neighbor_ids.push(AqlValue::String(from.to_string()));
                                 }
                             }
                         }
                    }
                }
            }
                
                // If storage yielded nothing, maybe try context? 
                // But for now let's just return what we found.
                // If we found neighbors, return them.
                // Note: Duplicate removal might be needed.
                return Ok(AqlValue::Array(neighbor_ids));


            }
            "GRAPH_COMMON_NEIGHBORS" => {
                // GRAPH_COMMON_NEIGHBORS(graphName, vertex1, vertex2, options)
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }

                let vertex1 = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let vertex2 = args.get(2).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(v1), Some(v2)) = (vertex1, vertex2) {
                    let common = graph_algo::common_neighbors(&graph, v1, v2);
                    let neighbors: Vec<AqlValue> = common
                        .iter()
                        .map(|id| {
                            if let Some(node) = graph.nodes.get(id) {
                                let mut obj = HashMap::new();
                                obj.insert("_key".to_string(), AqlValue::String(id.clone()));
                                for (k, v) in &node.properties {
                                    obj.insert(k.clone(), json_to_aql_value(v));
                                }
                                AqlValue::Object(obj)
                            } else {
                                AqlValue::String(id.clone())
                            }
                        })
                        .collect();
                    Ok(AqlValue::Array(neighbors))
                } else {
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "GRAPH_COMMON_PROPERTIES" => {
                // GRAPH_COMMON_PROPERTIES(graphName, vertex1, vertex2, options)
                // Returns properties that both vertices share with the same value
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let graph = self.build_graph_from_context(context, graph_name);

                let vertex1 = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let vertex2 = args.get(2).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(v1), Some(v2)) = (vertex1, vertex2) {
                    if let (Some(node1), Some(node2)) = (graph.nodes.get(v1), graph.nodes.get(v2)) {
                        let mut common_props = HashMap::new();
                        for (k, v) in &node1.properties {
                            if let Some(other_v) = node2.properties.get(k) {
                                if v == other_v {
                                    common_props.insert(k.clone(), json_to_aql_value(v));
                                }
                            }
                        }
                        Ok(AqlValue::Object(common_props))
                    } else {
                        Ok(AqlValue::Object(HashMap::new()))
                    }
                } else {
                    Ok(AqlValue::Object(HashMap::new()))
                }
            }
            "GRAPH_PATHS" => {
                // GRAPH_PATHS(graphName, options) - Get all paths via DFS traversal
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }

                // Get start vertex and max depth from options
                let (start, max_depth) = if let Some(AqlValue::Object(opts)) = args.get(1) {
                    let start = opts.get("startVertex").and_then(|v| {
                        if let AqlValue::String(s) = v {
                            Some(s.as_str())
                        } else {
                            None
                        }
                    });
                    let max_depth = opts.get("maxDepth").and_then(|v| {
                        if let AqlValue::Number(n) = v {
                            n.as_u64().map(|n| n as usize)
                        } else {
                            None
                        }
                    });
                    (start, max_depth)
                } else {
                    (None, None)
                };

                if let Some(start_id) = start {
                    let traversal = graph_algo::dfs_traversal(&graph, start_id, max_depth);
                    // Return the traversal path (visited nodes in DFS order)
                    let path: Vec<AqlValue> = traversal
                        .visited
                        .iter()
                        .map(|id| AqlValue::String(id.clone()))
                        .collect();
                    Ok(AqlValue::Array(vec![AqlValue::Array(path)]))
                } else {
                    // No start vertex - return empty
                    Ok(AqlValue::Array(vec![]))
                }
            }
            "GRAPH_SHORTEST_PATH" => {
                // GRAPH_SHORTEST_PATH(graphName, startVertex, targetVertex, options)
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }

                let start = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(2).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    let path_result = graph_algo::dijkstra(&graph, start_id, target_id);

                    let mut result = HashMap::new();
                    result.insert(
                        "vertices".to_string(),
                        AqlValue::Array(
                            path_result
                                .path
                                .iter()
                                .map(|id| AqlValue::String(id.clone()))
                                .collect(),
                        ),
                    );
                    result.insert("edges".to_string(), AqlValue::Array(vec![]));
                    result.insert(
                        "distance".to_string(),
                        AqlValue::Number(
                            serde_json::Number::from_f64(path_result.cost)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ),
                    );
                    result.insert("found".to_string(), AqlValue::Bool(path_result.found));
                    Ok(AqlValue::Object(result))
                } else {
                    let mut result = HashMap::new();
                    result.insert("vertices".to_string(), AqlValue::Array(vec![]));
                    result.insert("edges".to_string(), AqlValue::Array(vec![]));
                    result.insert(
                        "distance".to_string(),
                        AqlValue::Number(serde_json::Number::from(0)),
                    );
                    result.insert("found".to_string(), AqlValue::Bool(false));
                    Ok(AqlValue::Object(result))
                }
            }
            "GRAPH_DISTANCE_TO" => {
                // GRAPH_DISTANCE_TO(graphName, startVertex, targetVertex, options)
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }

                let start = args.get(1).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                let target = args.get(2).and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                if let (Some(start_id), Some(target_id)) = (start, target) {
                    if let Some(distance) = graph_algo::graph_distance(&graph, start_id, target_id)
                    {
                        Ok(AqlValue::Number(serde_json::Number::from(distance)))
                    } else {
                        // No path found
                        Ok(AqlValue::Number(serde_json::Number::from(-1)))
                    }
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(-1)))
                }
            }
            "GRAPH_ECCENTRICITY" => {
                // GRAPH_ECCENTRICITY(graphName, [vertex], [options])
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }

                // Check for vertex argument
                let vertex = args.get(1).and_then(|v| {
                     if let AqlValue::String(s) = v {
                         Some(s.as_str())
                     } else {
                         None
                     }
                });

                if let Some(v_id) = vertex {
                    // Single vertex eccentricity
                    if let Some(ecc) = graph_algo::eccentricity(&graph, v_id) {
                         Ok(AqlValue::Number(serde_json::Number::from(ecc)))
                    } else {
                         Ok(AqlValue::Number(serde_json::Number::from(-1)))
                    }
                } else {
                    // All vertices
                    let results = graph_algo::all_eccentricities(&graph);
                    let mut map = HashMap::new();
                    for (k, v) in results {
                         map.insert(k, AqlValue::Number(serde_json::Number::from(v)));
                    }
                    Ok(AqlValue::Object(map))
                }
            }
            "GRAPH_RADIUS" => {
                 // GRAPH_RADIUS(graphName, options)
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }
                
                if let Some(rad) = graph_algo::radius(&graph) {
                    Ok(AqlValue::Number(serde_json::Number::from(rad)))
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(-1)))
                }
            }
            "GRAPH_DIAMETER" => {
                 // GRAPH_DIAMETER(graphName, options)
                let graph_name = args.first().and_then(|v| {
                    if let AqlValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
                
                let mut graph = graph_algo::Graph::new();
                if let Some(name) = graph_name {
                     if let Some(storage) = &self.storage {
                         if let Ok(edges) = storage.get_collection_documents(name).await {
                         for edge in edges {
                             let from = edge.data.get("_from").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             let to = edge.data.get("_to").and_then(|v| if let AqlValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
                             if !from.is_empty() && !to.is_empty() {
                                 graph.add_node(from.to_string(), HashMap::new());
                                 graph.add_node(to.to_string(), HashMap::new());
                                 graph.add_edge(from.to_string(), to.to_string(), 1.0, None);
                             }
                         }
                     }
                }
            }
                
                if let Some(dia) = graph_algo::diameter(&graph) {
                    Ok(AqlValue::Number(serde_json::Number::from(dia)))
                } else {
                    Ok(AqlValue::Number(serde_json::Number::from(-1)))
                }
            }
            "PREGEL_RESULT" => {
                // PREGEL_RESULT(id) - Get Pregel algorithm result
                // Pregel is a distributed graph processing framework
                // This requires a separate Pregel engine which is not yet implemented
                Ok(AqlValue::Array(vec![]))
            }

            // ============ JSON Functions ============
            "JSON_PARSE" => {
                // JSON_PARSE(json_string) - Parse a JSON string into an AQL value
                if let Some(AqlValue::String(json_str)) = args.first() {
                    match serde_json::from_str::<serde_json::Value>(json_str) {
                        Ok(json_val) => {
                            // Convert serde_json::Value to AqlValue
                            Ok(json_value_to_aql_value(&json_val))
                        }
                        Err(_) => {
                            // Invalid JSON string returns null
                            Ok(AqlValue::Null)
                        }
                    }
                } else {
                    Ok(AqlValue::Null)
                }
            }
            "JSON_STRINGIFY" => {
                // JSON_STRINGIFY(value) - Convert an AQL value to a JSON string
                let value = args.first().unwrap_or(&AqlValue::Null);
                match serde_json::to_string(value) {
                    Ok(json_str) => Ok(AqlValue::String(json_str)),
                    Err(_) => {
                        // Fallback to simple string conversion
                        Ok(AqlValue::String(format!("{:?}", value)))
                    }
                }
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
