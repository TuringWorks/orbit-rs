//! GraphRAG function support for PostgreSQL wire protocol
//!
//! This module provides PostgreSQL-compatible function calls for GraphRAG operations,
//! allowing users to call GraphRAG functionality through SQL function syntax.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::graphrag::{
    entity_extraction::DocumentProcessingResult,
    graph_rag_actor::{GraphRAGDocumentRequest, GraphRAGQuery, GraphRAGQueryResult, GraphRAGStats},
    multi_hop_reasoning::ReasoningQuery,
    GraphRAGActor,
};
use crate::protocols::postgres_wire::query_engine::QueryResult;
use orbit_client::OrbitClient;
use orbit_shared::{graphrag::ReasoningPath, Key};
use regex::Regex;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Type alias for GraphRAG query parsing result
type GraphRAGQueryParams = (
    String,
    String,
    Option<u32>,
    Option<usize>,
    Option<String>,
    bool,
);

/// GraphRAG query engine for PostgreSQL function calls
pub struct GraphRAGQueryEngine {
    orbit_client: Option<Arc<OrbitClient>>,
}

impl GraphRAGQueryEngine {
    /// Create new GraphRAG query engine
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client: Some(Arc::new(orbit_client)),
        }
    }

    /// Create new GraphRAG query engine without OrbitClient (placeholder mode)
    pub fn new_placeholder() -> Self {
        Self { orbit_client: None }
    }

    /// Execute a GraphRAG function query
    pub async fn execute_graphrag_query(&self, sql: &str) -> ProtocolResult<QueryResult> {
        if self.orbit_client.is_none() {
            return Err(ProtocolError::PostgresError(
                "GraphRAG functionality requires OrbitClient integration".to_string(),
            ));
        }

        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.contains("GRAPHRAG_BUILD(") {
            self.execute_graphrag_build(sql).await
        } else if sql_upper.contains("GRAPHRAG_QUERY(") {
            self.execute_graphrag_query_func(sql).await
        } else if sql_upper.contains("GRAPHRAG_EXTRACT(") {
            self.execute_graphrag_extract(sql).await
        } else if sql_upper.contains("GRAPHRAG_REASON(") {
            self.execute_graphrag_reason(sql).await
        } else if sql_upper.contains("GRAPHRAG_STATS(") {
            self.execute_graphrag_stats(sql).await
        } else if sql_upper.contains("GRAPHRAG_ENTITIES(") {
            self.execute_graphrag_entities(sql).await
        } else if sql_upper.contains("GRAPHRAG_SIMILAR(") {
            self.execute_graphrag_similar(sql).await
        } else {
            Err(ProtocolError::PostgresError(format!(
                "Unknown GraphRAG function in query: {sql}"
            )))
        }
    }

    /// Execute GRAPHRAG_BUILD function
    /// SQL: SELECT * FROM GRAPHRAG_BUILD('kg_name', 'doc_id', 'document text', '{"key": "value"}'::json)
    async fn execute_graphrag_build(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, document_id, text, metadata_json) = self.parse_graphrag_build(sql)?;

        // Parse metadata JSON if provided
        let mut metadata = HashMap::new();
        if let Some(json_str) = metadata_json {
            if let Ok(JsonValue::Object(map)) = serde_json::from_str::<JsonValue>(&json_str) {
                for (k, v) in map {
                    metadata.insert(k, v);
                }
            }
        }

        // Get GraphRAG actor reference
        let orbit_client = self.orbit_client.as_ref().unwrap();
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Actor error: {e}")))?;

        // Create document processing request
        let request = GraphRAGDocumentRequest {
            document_id: document_id.clone(),
            text: text.clone(),
            metadata,
            build_knowledge_graph: true,
            generate_embeddings: true,
            extractors: None,
        };

        // Process document
        let result: DocumentProcessingResult = actor_ref
            .invoke(
                "process_document",
                vec![serde_json::to_value(request).unwrap()],
            )
            .await
            .map_err(|e| {
                ProtocolError::PostgresError(format!("Document processing failed: {e}"))
            })?;

        // Format as PostgreSQL result
        let columns = vec![
            "kg_name".to_string(),
            "document_id".to_string(),
            "entities_extracted".to_string(),
            "relationships_extracted".to_string(),
            "extractors_used".to_string(),
            "processing_time_ms".to_string(),
        ];

        let rows = vec![vec![
            Some(kg_name),
            Some(document_id),
            Some(result.entities.len().to_string()),
            Some(result.relationships.len().to_string()),
            Some(result.extractors_used.to_string()),
            Some(result.processing_time_ms.to_string()),
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_QUERY function  
    /// SQL: SELECT * FROM GRAPHRAG_QUERY('kg_name', 'query text', 3, 2048, 'ollama', true)
    async fn execute_graphrag_query_func(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, query_text, max_hops, context_size, llm_provider, include_explanation) =
            self.parse_graphrag_query(sql)?;

        // Get GraphRAG actor reference
        let orbit_client = self.orbit_client.as_ref().unwrap();
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Actor error: {e}")))?;

        // Create GraphRAG query
        let query = GraphRAGQuery {
            query_text: query_text.clone(),
            max_hops,
            context_size,
            llm_provider,
            search_strategy: None,
            include_explanation,
            max_results: None,
        };

        // Execute query
        let result: GraphRAGQueryResult = actor_ref
            .invoke("query_rag", vec![serde_json::to_value(query).unwrap()])
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("RAG query failed: {e}")))?;

        // Format as PostgreSQL result
        let columns = vec![
            "kg_name".to_string(),
            "query_text".to_string(),
            "response".to_string(),
            "confidence".to_string(),
            "processing_time_ms".to_string(),
            "entities_involved".to_string(),
            "citations".to_string(),
        ];

        let entities_json = serde_json::to_string(&result.entities_involved).unwrap_or_default();
        let citations_json = serde_json::to_string(&result.response.citations).unwrap_or_default();

        let rows = vec![vec![
            Some(kg_name),
            Some(query_text),
            Some(result.response.response),
            Some(result.response.confidence.to_string()),
            Some(result.processing_times.total_ms.to_string()),
            Some(entities_json),
            Some(citations_json),
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_EXTRACT function
    /// SQL: SELECT * FROM GRAPHRAG_EXTRACT('kg_name', 'doc_id', 'text', ARRAY['extractor1', 'extractor2'])
    async fn execute_graphrag_extract(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, document_id, text, extractors) = self.parse_graphrag_extract(sql)?;

        // Get GraphRAG actor reference
        let orbit_client = self.orbit_client.as_ref().unwrap();
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Actor error: {e}")))?;

        // Create extraction request
        let request = GraphRAGDocumentRequest {
            document_id: document_id.clone(),
            text: text.clone(),
            metadata: HashMap::new(),
            build_knowledge_graph: false,
            generate_embeddings: false,
            extractors,
        };

        // Process document for extraction only
        let result: DocumentProcessingResult = actor_ref
            .invoke(
                "process_document",
                vec![serde_json::to_value(request).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Entity extraction failed: {e}")))?;

        // Format as PostgreSQL result
        let columns = vec![
            "kg_name".to_string(),
            "document_id".to_string(),
            "entities_extracted".to_string(),
            "relationships_extracted".to_string(),
            "processing_time_ms".to_string(),
        ];

        let rows = vec![vec![
            Some(kg_name),
            Some(document_id),
            Some(result.entities.len().to_string()),
            Some(result.relationships.len().to_string()),
            Some(result.processing_time_ms.to_string()),
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_REASON function
    /// SQL: SELECT * FROM GRAPHRAG_REASON('kg_name', 'entity1', 'entity2', 3, true)
    async fn execute_graphrag_reason(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, from_entity, to_entity, max_hops, include_explanation) =
            self.parse_graphrag_reason(sql)?;

        // Get GraphRAG actor reference
        let orbit_client = self.orbit_client.as_ref().unwrap();
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Actor error: {e}")))?;

        // Create reasoning query
        let reasoning_query = ReasoningQuery {
            from_entity: from_entity.clone(),
            to_entity: to_entity.clone(),
            max_hops,
            relationship_types: None,
            include_explanation,
            max_results: None,
        };

        // Execute reasoning query
        let paths: Vec<ReasoningPath> = actor_ref
            .invoke(
                "find_connection_paths",
                vec![serde_json::to_value(reasoning_query).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Reasoning failed: {e}")))?;

        // Format as PostgreSQL result - one row per path
        let columns = vec![
            "kg_name".to_string(),
            "from_entity".to_string(),
            "to_entity".to_string(),
            "path_nodes".to_string(),
            "path_relationships".to_string(),
            "score".to_string(),
            "length".to_string(),
            "explanation".to_string(),
        ];

        let mut rows = Vec::new();
        for path in paths {
            let nodes_json = serde_json::to_string(&path.nodes).unwrap_or_default();
            let relationships_json = serde_json::to_string(&path.relationships).unwrap_or_default();

            rows.push(vec![
                Some(kg_name.clone()),
                Some(from_entity.clone()),
                Some(to_entity.clone()),
                Some(nodes_json),
                Some(relationships_json),
                Some(path.score.to_string()),
                Some(path.length.to_string()),
                Some(if include_explanation {
                    path.explanation
                } else {
                    "".to_string()
                }),
            ]);
        }

        // If no paths found, return empty result set
        if rows.is_empty() {
            rows.push(vec![
                Some(kg_name),
                Some(from_entity),
                Some(to_entity),
                Some("[]".to_string()),
                Some("[]".to_string()),
                Some("0.0".to_string()),
                Some("0".to_string()),
                Some("No paths found".to_string()),
            ]);
        }

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_STATS function
    /// SQL: SELECT * FROM GRAPHRAG_STATS('kg_name')
    async fn execute_graphrag_stats(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let kg_name = self.parse_graphrag_stats(sql)?;

        // Get GraphRAG actor reference
        let orbit_client = self.orbit_client.as_ref().unwrap();
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Actor error: {e}")))?;

        // Get statistics
        let stats: GraphRAGStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Failed to get stats: {e}")))?;

        // Format as PostgreSQL result
        let columns = vec![
            "kg_name".to_string(),
            "documents_processed".to_string(),
            "rag_queries_executed".to_string(),
            "reasoning_queries_executed".to_string(),
            "entities_extracted".to_string(),
            "relationships_extracted".to_string(),
            "avg_document_processing_time_ms".to_string(),
            "avg_rag_query_time_ms".to_string(),
            "rag_success_rate".to_string(),
        ];

        let rows = vec![vec![
            Some(kg_name),
            Some(stats.documents_processed.to_string()),
            Some(stats.rag_queries_executed.to_string()),
            Some(stats.reasoning_queries_executed.to_string()),
            Some(stats.entities_extracted.to_string()),
            Some(stats.relationships_extracted.to_string()),
            Some(format!("{:.2}", stats.avg_document_processing_time_ms)),
            Some(format!("{:.2}", stats.avg_rag_query_time_ms)),
            Some(format!("{:.3}", stats.rag_success_rate)),
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_ENTITIES function (placeholder)
    /// SQL: SELECT * FROM GRAPHRAG_ENTITIES('kg_name', 'entity_type', 10)
    async fn execute_graphrag_entities(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, entity_type, limit) = self.parse_graphrag_entities(sql)?;

        // For now, return a placeholder result
        let columns = vec![
            "kg_name".to_string(),
            "entity_type".to_string(),
            "limit_requested".to_string(),
            "total_count".to_string(),
        ];

        let rows = vec![vec![
            Some(kg_name),
            entity_type,
            limit.map(|l| l.to_string()),
            Some("0".to_string()), // Placeholder
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    /// Execute GRAPHRAG_SIMILAR function (placeholder)
    /// SQL: SELECT * FROM GRAPHRAG_SIMILAR('kg_name', 'entity_name', 5, 0.8)
    async fn execute_graphrag_similar(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let (kg_name, entity_name, limit, threshold) = self.parse_graphrag_similar(sql)?;

        // For now, return a placeholder result
        let columns = vec![
            "kg_name".to_string(),
            "entity_name".to_string(),
            "limit_requested".to_string(),
            "threshold_used".to_string(),
            "similar_count".to_string(),
        ];

        let rows = vec![vec![
            Some(kg_name),
            Some(entity_name),
            limit.map(|l| l.to_string()),
            threshold.map(|t| t.to_string()),
            Some("0".to_string()), // Placeholder
        ]];

        Ok(QueryResult::Select { columns, rows })
    }

    // Parsing methods for different function signatures...

    fn parse_graphrag_build(
        &self,
        sql: &str,
    ) -> ProtocolResult<(String, String, String, Option<String>)> {
        // Simple regex-based parsing for GRAPHRAG_BUILD('kg_name', 'doc_id', 'text', metadata)
        let re = Regex::new(r"GRAPHRAG_BUILD\s*\(\s*'([^']+)'\s*,\s*'([^']+)'\s*,\s*'([^']*?)'\s*(?:,\s*'([^']*?)'[^)]*?)?\s*\)").unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let document_id = captures.get(2).unwrap().as_str().to_string();
            let text = captures.get(3).unwrap().as_str().to_string();
            let metadata = captures.get(4).map(|m| m.as_str().to_string());

            Ok((kg_name, document_id, text, metadata))
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_BUILD function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_query(&self, sql: &str) -> ProtocolResult<GraphRAGQueryParams> {
        // Simple parsing for GRAPHRAG_QUERY function
        let re = Regex::new(r"GRAPHRAG_QUERY\s*\(\s*'([^']+)'\s*,\s*'([^']*?)'\s*(?:,\s*(\d+))?\s*(?:,\s*(\d+))?\s*(?:,\s*'([^']*?)')?\s*(?:,\s*(true|false))?\s*\)").unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let query_text = captures.get(2).unwrap().as_str().to_string();
            let max_hops = captures.get(3).and_then(|m| m.as_str().parse::<u32>().ok());
            let context_size = captures
                .get(4)
                .and_then(|m| m.as_str().parse::<usize>().ok());
            let llm_provider = captures.get(5).map(|m| m.as_str().to_string());
            let include_explanation = captures
                .get(6)
                .map(|m| m.as_str() == "true")
                .unwrap_or(false);

            Ok((
                kg_name,
                query_text,
                max_hops,
                context_size,
                llm_provider,
                include_explanation,
            ))
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_QUERY function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_extract(
        &self,
        sql: &str,
    ) -> ProtocolResult<(String, String, String, Option<Vec<String>>)> {
        // Simple parsing for GRAPHRAG_EXTRACT function
        let re =
            Regex::new(r"GRAPHRAG_EXTRACT\s*\(\s*'([^']+)'\s*,\s*'([^']+)'\s*,\s*'([^']*?)'\s*\)")
                .unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let document_id = captures.get(2).unwrap().as_str().to_string();
            let text = captures.get(3).unwrap().as_str().to_string();

            Ok((kg_name, document_id, text, None)) // Extractors parsing TBD
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_EXTRACT function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_reason(
        &self,
        sql: &str,
    ) -> ProtocolResult<(String, String, String, Option<u32>, bool)> {
        let re = Regex::new(r"GRAPHRAG_REASON\s*\(\s*'([^']+)'\s*,\s*'([^']+)'\s*,\s*'([^']+)'\s*(?:,\s*(\d+))?\s*(?:,\s*(true|false))?\s*\)").unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let from_entity = captures.get(2).unwrap().as_str().to_string();
            let to_entity = captures.get(3).unwrap().as_str().to_string();
            let max_hops = captures.get(4).and_then(|m| m.as_str().parse::<u32>().ok());
            let include_explanation = captures
                .get(5)
                .map(|m| m.as_str() == "true")
                .unwrap_or(false);

            Ok((
                kg_name,
                from_entity,
                to_entity,
                max_hops,
                include_explanation,
            ))
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_REASON function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_stats(&self, sql: &str) -> ProtocolResult<String> {
        let re = Regex::new(r"GRAPHRAG_STATS\s*\(\s*'([^']+)'\s*\)").unwrap();

        if let Some(captures) = re.captures(sql) {
            Ok(captures.get(1).unwrap().as_str().to_string())
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_STATS function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_entities(
        &self,
        sql: &str,
    ) -> ProtocolResult<(String, Option<String>, Option<usize>)> {
        let re = Regex::new(
            r"GRAPHRAG_ENTITIES\s*\(\s*'([^']+)'\s*(?:,\s*'([^']*?)')?(?:,\s*(\d+))?\s*\)",
        )
        .unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let entity_type = captures.get(2).map(|m| m.as_str().to_string());
            let limit = captures
                .get(3)
                .and_then(|m| m.as_str().parse::<usize>().ok());

            Ok((kg_name, entity_type, limit))
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_ENTITIES function syntax".to_string(),
            ))
        }
    }

    fn parse_graphrag_similar(
        &self,
        sql: &str,
    ) -> ProtocolResult<(String, String, Option<usize>, Option<f32>)> {
        let re = Regex::new(r"GRAPHRAG_SIMILAR\s*\(\s*'([^']+)'\s*,\s*'([^']+)'\s*(?:,\s*(\d+))?(?:,\s*([0-9.]+))?\s*\)").unwrap();

        if let Some(captures) = re.captures(sql) {
            let kg_name = captures.get(1).unwrap().as_str().to_string();
            let entity_name = captures.get(2).unwrap().as_str().to_string();
            let limit = captures
                .get(3)
                .and_then(|m| m.as_str().parse::<usize>().ok());
            let threshold = captures.get(4).and_then(|m| m.as_str().parse::<f32>().ok());

            Ok((kg_name, entity_name, limit, threshold))
        } else {
            Err(ProtocolError::PostgresError(
                "Invalid GRAPHRAG_SIMILAR function syntax".to_string(),
            ))
        }
    }
}
