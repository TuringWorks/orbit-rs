//! GraphRAG function support for AQL (ArangoDB Query Language)
//!
//! This module provides AQL-compatible function calls for GraphRAG operations,
//! allowing users to call GraphRAG functionality through AQL function syntax.

use crate::aql::data_model::AqlValue;
use crate::error::{ProtocolError, ProtocolResult};
use crate::graphrag::{
    entity_extraction::DocumentProcessingResult,
    graph_rag_actor::{GraphRAGDocumentRequest, GraphRAGQuery, GraphRAGQueryResult, GraphRAGStats},
    multi_hop_reasoning::ReasoningQuery,
    GraphRAGActor,
};
use orbit_client::OrbitClient;
use orbit_shared::{graphrag::ReasoningPath, Key};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// GraphRAG query engine for AQL function calls
pub struct AqlGraphRAGEngine {
    orbit_client: Option<Arc<OrbitClient>>,
}

impl AqlGraphRAGEngine {
    /// Create new AQL GraphRAG engine
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client: Some(Arc::new(orbit_client)),
        }
    }

    /// Create new AQL GraphRAG engine without OrbitClient (placeholder mode)
    pub fn new_placeholder() -> Self {
        Self { orbit_client: None }
    }

    /// Execute an AQL GraphRAG function call
    pub async fn execute_graphrag_function(
        &self,
        function_name: &str,
        args: &[AqlValue],
    ) -> ProtocolResult<AqlValue> {
        if self.orbit_client.is_none() {
            return Err(ProtocolError::AqlError(
                "GraphRAG functionality requires OrbitClient integration".to_string(),
            ));
        }

        match function_name.to_uppercase().as_str() {
            "GRAPHRAG_BUILD_KNOWLEDGE" => self.execute_build_knowledge(args).await,
            "GRAPHRAG_EXTRACT_ENTITIES" => self.execute_extract_entities(args).await,
            "GRAPHRAG_QUERY" => self.execute_rag_query(args).await,
            "GRAPHRAG_FIND_PATHS" => self.execute_find_paths(args).await,
            "GRAPHRAG_FIND_SIMILAR" => self.execute_find_similar(args).await,
            "GRAPHRAG_SEMANTIC_SEARCH" => self.execute_semantic_search(args).await,
            "GRAPHRAG_GET_STATS" => self.execute_get_stats(args).await,
            "GRAPHRAG_LIST_ENTITIES" => self.execute_list_entities(args).await,
            "GRAPHRAG_ANALYZE_TRENDS" => self.execute_analyze_trends(args).await,
            "GRAPHRAG_DETECT_COMMUNITIES" => self.execute_detect_communities(args).await,
            _ => Err(ProtocolError::AqlError(format!(
                "Unknown GraphRAG function: {}",
                function_name
            ))),
        }
    }

    /// Execute GRAPHRAG_BUILD_KNOWLEDGE function
    /// GRAPHRAG_BUILD_KNOWLEDGE(document_or_collection, options)
    async fn execute_build_knowledge(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 2 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_BUILD_KNOWLEDGE requires at least 2 arguments: (document_or_collection, options)".to_string()
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        // Parse arguments
        let (document_text, document_id, metadata) = self.parse_document_arg(&args[0])?;
        let options = self.parse_options_arg(&args[1])?;

        // Extract configuration from options
        let kg_name = options
            .get("knowledge_graph")
            .and_then(|v| v.as_str())
            .unwrap_or("default_kg")
            .to_string();

        let extractors = options
            .get("extractors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

        let build_graph = options
            .get("build_graph")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let generate_embeddings = options
            .get("generate_embeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Actor error: {}", e)))?;

        // Create document processing request
        let request = GraphRAGDocumentRequest {
            document_id,
            text: document_text,
            metadata,
            build_knowledge_graph: build_graph,
            generate_embeddings,
            extractors,
        };

        // Process document
        let result: DocumentProcessingResult = actor_ref
            .invoke(
                "process_document",
                vec![serde_json::to_value(request).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Document processing failed: {}", e)))?;

        // Format as AQL result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("kg_name".to_string(), AqlValue::String(kg_name.clone()));
        result_obj.insert(
            "document_id".to_string(),
            AqlValue::String(result.document_id),
        );
        result_obj.insert(
            "entities_extracted".to_string(),
            AqlValue::Number(serde_json::Number::from(result.entities.len())),
        );
        result_obj.insert(
            "relationships_extracted".to_string(),
            AqlValue::Number(serde_json::Number::from(result.relationships.len())),
        );
        result_obj.insert(
            "processing_time_ms".to_string(),
            AqlValue::Number(serde_json::Number::from(result.processing_time_ms)),
        );
        result_obj.insert(
            "extractors_used".to_string(),
            AqlValue::Number(serde_json::Number::from(result.extractors_used)),
        );

        // Add metadata
        let mut metadata_obj = HashMap::new();
        metadata_obj.insert(
            "timestamp".to_string(),
            AqlValue::String(chrono::Utc::now().to_rfc3339()),
        );
        metadata_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("metadata".to_string(), AqlValue::Object(metadata_obj));

        if !result.warnings.is_empty() {
            let warnings: Vec<AqlValue> = result
                .warnings
                .iter()
                .map(|w| AqlValue::String(w.clone()))
                .collect();
            result_obj.insert("warnings".to_string(), AqlValue::Array(warnings));
        }

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_EXTRACT_ENTITIES function
    /// GRAPHRAG_EXTRACT_ENTITIES(text, options)
    async fn execute_extract_entities(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 2 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_EXTRACT_ENTITIES requires 2 arguments: (text, options)".to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let text = self.extract_string_arg(&args[0], "text")?;
        let options = self.parse_options_arg(&args[1])?;

        let kg_name = options
            .get("knowledge_graph")
            .and_then(|v| v.as_str())
            .unwrap_or("default_kg")
            .to_string();

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Actor error: {}", e)))?;

        // Create extraction request
        let request = GraphRAGDocumentRequest {
            document_id: format!("extract_{}", chrono::Utc::now().timestamp()),
            text,
            metadata: HashMap::new(),
            build_knowledge_graph: false,
            generate_embeddings: false,
            extractors: None,
        };

        // Process document for extraction only
        let result: DocumentProcessingResult = actor_ref
            .invoke(
                "process_document",
                vec![serde_json::to_value(request).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Entity extraction failed: {}", e)))?;

        // Convert entities to AQL format
        let entities: Vec<AqlValue> = result
            .entities
            .iter()
            .map(|entity| {
                let mut entity_obj = HashMap::new();
                entity_obj.insert("text".to_string(), AqlValue::String(entity.text.clone()));
                entity_obj.insert(
                    "entity_type".to_string(),
                    AqlValue::String(entity.entity_type.to_str().to_string()),
                );
                entity_obj.insert(
                    "confidence".to_string(),
                    AqlValue::Number(
                        serde_json::Number::from_f64(entity.confidence as f64).unwrap(),
                    ),
                );
                entity_obj.insert(
                    "start_pos".to_string(),
                    AqlValue::Number(serde_json::Number::from(entity.start_pos)),
                );
                entity_obj.insert(
                    "end_pos".to_string(),
                    AqlValue::Number(serde_json::Number::from(entity.end_pos)),
                );

                // Add properties if they exist
                if !entity.properties.is_empty() {
                    let properties: HashMap<String, AqlValue> = entity
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), json_value_to_aql_value(v)))
                        .collect();
                    entity_obj.insert("properties".to_string(), AqlValue::Object(properties));
                }

                AqlValue::Object(entity_obj)
            })
            .collect();

        Ok(AqlValue::Array(entities))
    }

    /// Execute GRAPHRAG_QUERY function
    /// GRAPHRAG_QUERY(knowledge_graph, query_text, options)
    async fn execute_rag_query(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 3 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_QUERY requires 3 arguments: (knowledge_graph, query_text, options)"
                    .to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let query_text = self.extract_string_arg(&args[1], "query_text")?;
        let options = self.parse_options_arg(&args[2])?;

        // Parse options
        let max_hops = options
            .get("max_hops")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let context_size = options
            .get("context_size")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let llm_provider = options
            .get("llm_provider")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let include_explanation = options
            .get("include_explanation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Actor error: {}", e)))?;

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
            .map_err(|e| ProtocolError::AqlError(format!("RAG query failed: {}", e)))?;

        // Format as AQL result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("query_text".to_string(), AqlValue::String(query_text));
        result_obj.insert(
            "response".to_string(),
            AqlValue::String(result.response.response),
        );
        result_obj.insert(
            "confidence".to_string(),
            AqlValue::Number(
                serde_json::Number::from_f64(result.response.confidence as f64).unwrap(),
            ),
        );
        result_obj.insert(
            "processing_time_ms".to_string(),
            AqlValue::Number(serde_json::Number::from(result.processing_times.total_ms)),
        );

        // Add entities involved
        let entities_involved: Vec<AqlValue> = result
            .entities_involved
            .iter()
            .map(|e| AqlValue::String(e.clone()))
            .collect();
        result_obj.insert(
            "entities_involved".to_string(),
            AqlValue::Array(entities_involved),
        );

        // Add citations
        let citations: Vec<AqlValue> = result
            .response
            .citations
            .iter()
            .map(|c| AqlValue::String(c.clone()))
            .collect();
        result_obj.insert("citations".to_string(), AqlValue::Array(citations));

        // Add reasoning paths if requested
        if include_explanation {
            if let Some(paths) = result.reasoning_paths {
                let reasoning_paths: Vec<AqlValue> = paths
                    .iter()
                    .map(|path| {
                        let mut path_obj = HashMap::new();
                        path_obj.insert(
                            "nodes".to_string(),
                            AqlValue::Array(
                                path.nodes
                                    .iter()
                                    .map(|n| AqlValue::String(n.clone()))
                                    .collect(),
                            ),
                        );
                        path_obj.insert(
                            "relationships".to_string(),
                            AqlValue::Array(
                                path.relationships
                                    .iter()
                                    .map(|r| AqlValue::String(r.clone()))
                                    .collect(),
                            ),
                        );
                        path_obj.insert(
                            "score".to_string(),
                            AqlValue::Number(
                                serde_json::Number::from_f64(path.score as f64).unwrap(),
                            ),
                        );
                        path_obj.insert(
                            "length".to_string(),
                            AqlValue::Number(serde_json::Number::from(path.length)),
                        );
                        path_obj.insert(
                            "explanation".to_string(),
                            AqlValue::String(path.explanation.clone()),
                        );
                        AqlValue::Object(path_obj)
                    })
                    .collect();
                result_obj.insert(
                    "reasoning_paths".to_string(),
                    AqlValue::Array(reasoning_paths),
                );
            }
        }

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_FIND_PATHS function
    /// GRAPHRAG_FIND_PATHS(knowledge_graph, from_entity, to_entity, options)
    async fn execute_find_paths(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 4 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_FIND_PATHS requires 4 arguments: (knowledge_graph, from_entity, to_entity, options)".to_string()
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let from_entity = self.extract_string_arg(&args[1], "from_entity")?;
        let to_entity = self.extract_string_arg(&args[2], "to_entity")?;
        let options = self.parse_options_arg(&args[3])?;

        // Parse options
        let max_hops = options
            .get("max_hops")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let include_explanation = options
            .get("include_explanation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_results = options
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Actor error: {}", e)))?;

        // Create reasoning query
        let reasoning_query = ReasoningQuery {
            from_entity: from_entity.clone(),
            to_entity: to_entity.clone(),
            max_hops,
            relationship_types: None,
            include_explanation,
            max_results,
        };

        // Execute reasoning query
        let paths: Vec<ReasoningPath> = actor_ref
            .invoke(
                "find_connection_paths",
                vec![serde_json::to_value(reasoning_query).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Reasoning failed: {}", e)))?;

        // Convert paths to AQL format
        let aql_paths: Vec<AqlValue> = paths
            .iter()
            .map(|path| {
                let mut path_obj = HashMap::new();
                path_obj.insert(
                    "path_nodes".to_string(),
                    AqlValue::Array(
                        path.nodes
                            .iter()
                            .map(|n| AqlValue::String(n.clone()))
                            .collect(),
                    ),
                );
                path_obj.insert(
                    "relationships".to_string(),
                    AqlValue::Array(
                        path.relationships
                            .iter()
                            .map(|r| AqlValue::String(r.clone()))
                            .collect(),
                    ),
                );
                path_obj.insert(
                    "score".to_string(),
                    AqlValue::Number(serde_json::Number::from_f64(path.score as f64).unwrap()),
                );
                path_obj.insert(
                    "length".to_string(),
                    AqlValue::Number(serde_json::Number::from(path.length)),
                );
                if include_explanation {
                    path_obj.insert(
                        "explanation".to_string(),
                        AqlValue::String(path.explanation.clone()),
                    );
                }
                AqlValue::Object(path_obj)
            })
            .collect();

        Ok(AqlValue::Array(aql_paths))
    }

    /// Execute GRAPHRAG_FIND_SIMILAR function
    /// GRAPHRAG_FIND_SIMILAR(knowledge_graph, entity, options)
    async fn execute_find_similar(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 3 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_FIND_SIMILAR requires 3 arguments: (knowledge_graph, entity, options)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let entity = self.extract_string_arg(&args[1], "entity")?;
        let _options = self.parse_options_arg(&args[2])?;

        // For now, return a placeholder result since we don't have similarity search implemented
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("entity".to_string(), AqlValue::String(entity));
        result_obj.insert("similar_entities".to_string(), AqlValue::Array(Vec::new()));
        result_obj.insert(
            "message".to_string(),
            AqlValue::String("Similarity search not yet implemented".to_string()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_SEMANTIC_SEARCH function
    /// GRAPHRAG_SEMANTIC_SEARCH(knowledge_graph, query_text, options)
    async fn execute_semantic_search(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 3 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_SEMANTIC_SEARCH requires 3 arguments: (knowledge_graph, query_text, options)".to_string()
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let query_text = self.extract_string_arg(&args[1], "query_text")?;
        let _options = self.parse_options_arg(&args[2])?;

        // For now, return a placeholder result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("query_text".to_string(), AqlValue::String(query_text));
        result_obj.insert("results".to_string(), AqlValue::Array(Vec::new()));
        result_obj.insert(
            "message".to_string(),
            AqlValue::String("Semantic search not yet implemented".to_string()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_GET_STATS function
    /// GRAPHRAG_GET_STATS(knowledge_graph)
    async fn execute_get_stats(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.is_empty() {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_GET_STATS requires 1 argument: (knowledge_graph)".to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();
        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Actor error: {}", e)))?;

        // Get statistics
        let stats: GraphRAGStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::AqlError(format!("Failed to get stats: {}", e)))?;

        // Format as AQL result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert(
            "documents_processed".to_string(),
            AqlValue::Number(serde_json::Number::from(stats.documents_processed)),
        );
        result_obj.insert(
            "rag_queries_executed".to_string(),
            AqlValue::Number(serde_json::Number::from(stats.rag_queries_executed)),
        );
        result_obj.insert(
            "reasoning_queries_executed".to_string(),
            AqlValue::Number(serde_json::Number::from(stats.reasoning_queries_executed)),
        );
        result_obj.insert(
            "entities_extracted".to_string(),
            AqlValue::Number(serde_json::Number::from(stats.entities_extracted)),
        );
        result_obj.insert(
            "relationships_extracted".to_string(),
            AqlValue::Number(serde_json::Number::from(stats.relationships_extracted)),
        );
        result_obj.insert(
            "avg_document_processing_time_ms".to_string(),
            AqlValue::Number(
                serde_json::Number::from_f64(stats.avg_document_processing_time_ms).unwrap(),
            ),
        );
        result_obj.insert(
            "avg_rag_query_time_ms".to_string(),
            AqlValue::Number(serde_json::Number::from_f64(stats.avg_rag_query_time_ms).unwrap()),
        );
        result_obj.insert(
            "rag_success_rate".to_string(),
            AqlValue::Number(serde_json::Number::from_f64(stats.rag_success_rate as f64).unwrap()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_LIST_ENTITIES function
    /// GRAPHRAG_LIST_ENTITIES(knowledge_graph, options)
    async fn execute_list_entities(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 2 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_LIST_ENTITIES requires 2 arguments: (knowledge_graph, options)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let _options = self.parse_options_arg(&args[1])?;

        // For now, return a placeholder result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("entities".to_string(), AqlValue::Array(Vec::new()));
        result_obj.insert(
            "message".to_string(),
            AqlValue::String("Entity listing not yet implemented".to_string()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_ANALYZE_TRENDS function
    /// GRAPHRAG_ANALYZE_TRENDS(knowledge_graph, concept, options)
    async fn execute_analyze_trends(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 3 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_ANALYZE_TRENDS requires 3 arguments: (knowledge_graph, concept, options)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let concept = self.extract_string_arg(&args[1], "concept")?;
        let _options = self.parse_options_arg(&args[2])?;

        // For now, return a placeholder result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("concept".to_string(), AqlValue::String(concept));
        result_obj.insert("trends".to_string(), AqlValue::Array(Vec::new()));
        result_obj.insert(
            "message".to_string(),
            AqlValue::String("Trend analysis not yet implemented".to_string()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Execute GRAPHRAG_DETECT_COMMUNITIES function
    /// GRAPHRAG_DETECT_COMMUNITIES(knowledge_graph, options)
    async fn execute_detect_communities(&self, args: &[AqlValue]) -> ProtocolResult<AqlValue> {
        if args.len() < 2 {
            return Err(ProtocolError::AqlError(
                "GRAPHRAG_DETECT_COMMUNITIES requires 2 arguments: (knowledge_graph, options)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "knowledge_graph")?;
        let _options = self.parse_options_arg(&args[1])?;

        // For now, return a placeholder result
        let mut result_obj = HashMap::new();
        result_obj.insert("success".to_string(), AqlValue::Bool(true));
        result_obj.insert("knowledge_graph".to_string(), AqlValue::String(kg_name));
        result_obj.insert("communities".to_string(), AqlValue::Array(Vec::new()));
        result_obj.insert(
            "message".to_string(),
            AqlValue::String("Community detection not yet implemented".to_string()),
        );

        Ok(AqlValue::Object(result_obj))
    }

    /// Helper function to parse document argument (can be a string or object)
    fn parse_document_arg(
        &self,
        arg: &AqlValue,
    ) -> ProtocolResult<(String, String, HashMap<String, JsonValue>)> {
        match arg {
            AqlValue::String(text) => {
                // Simple text document
                Ok((
                    text.clone(),
                    format!("doc_{}", chrono::Utc::now().timestamp()),
                    HashMap::new(),
                ))
            }
            AqlValue::Object(obj) => {
                // Document object with metadata
                let text = obj
                    .get("content")
                    .or_else(|| obj.get("text"))
                    .and_then(|v| match v {
                        AqlValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        ProtocolError::AqlError(
                            "Document object must have 'content' or 'text' field".to_string(),
                        )
                    })?;

                let document_id = obj
                    .get("_key")
                    .or_else(|| obj.get("id"))
                    .and_then(|v| match v {
                        AqlValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("doc_{}", chrono::Utc::now().timestamp()));

                // Extract metadata
                let mut metadata = HashMap::new();
                for (key, value) in obj {
                    if key != "content" && key != "text" && key != "_key" && key != "id" {
                        metadata.insert(key.clone(), aql_value_to_json_value(value));
                    }
                }

                Ok((text, document_id, metadata))
            }
            _ => Err(ProtocolError::AqlError(
                "Document argument must be string or object".to_string(),
            )),
        }
    }

    /// Helper function to parse options argument
    fn parse_options_arg(&self, arg: &AqlValue) -> ProtocolResult<HashMap<String, JsonValue>> {
        match arg {
            AqlValue::Object(obj) => {
                let mut options = HashMap::new();
                for (key, value) in obj {
                    options.insert(key.clone(), aql_value_to_json_value(value));
                }
                Ok(options)
            }
            _ => Err(ProtocolError::AqlError(
                "Options argument must be an object".to_string(),
            )),
        }
    }

    /// Helper function to extract string argument
    fn extract_string_arg(&self, arg: &AqlValue, arg_name: &str) -> ProtocolResult<String> {
        match arg {
            AqlValue::String(s) => Ok(s.clone()),
            _ => Err(ProtocolError::AqlError(format!(
                "{} argument must be a string",
                arg_name
            ))),
        }
    }
}

/// Helper function to convert AqlValue to JsonValue
fn aql_value_to_json_value(value: &AqlValue) -> JsonValue {
    match value {
        AqlValue::Null => JsonValue::Null,
        AqlValue::Bool(b) => JsonValue::Bool(*b),
        AqlValue::Number(n) => JsonValue::Number(n.clone()),
        AqlValue::String(s) => JsonValue::String(s.clone()),
        AqlValue::Array(arr) => {
            let json_arr: Vec<JsonValue> = arr.iter().map(aql_value_to_json_value).collect();
            JsonValue::Array(json_arr)
        }
        AqlValue::Object(obj) => {
            let mut json_obj = serde_json::Map::new();
            for (key, val) in obj {
                json_obj.insert(key.clone(), aql_value_to_json_value(val));
            }
            JsonValue::Object(json_obj)
        }
        AqlValue::DateTime(dt) => JsonValue::String(dt.to_rfc3339()),
    }
}

/// Helper function to convert JsonValue to AqlValue
fn json_value_to_aql_value(value: &JsonValue) -> AqlValue {
    match value {
        JsonValue::Null => AqlValue::Null,
        JsonValue::Bool(b) => AqlValue::Bool(*b),
        JsonValue::Number(n) => AqlValue::Number(n.clone()),
        JsonValue::String(s) => AqlValue::String(s.clone()),
        JsonValue::Array(arr) => {
            let aql_arr: Vec<AqlValue> = arr.iter().map(json_value_to_aql_value).collect();
            AqlValue::Array(aql_arr)
        }
        JsonValue::Object(obj) => {
            let mut aql_obj = HashMap::new();
            for (key, val) in obj {
                aql_obj.insert(key.clone(), json_value_to_aql_value(val));
            }
            AqlValue::Object(aql_obj)
        }
    }
}
