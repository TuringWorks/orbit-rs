//! GraphRAG procedure support for Cypher/Bolt protocol
//!
//! This module provides Cypher stored procedures for GraphRAG operations,
//! allowing users to call GraphRAG functionality through Cypher procedure syntax.

use crate::protocols::cypher::graph_engine::QueryResult;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::graphrag::{
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

/// GraphRAG procedure handler for Bolt/Cypher protocol
pub struct BoltGraphRAGProcedures {
    orbit_client: Option<Arc<OrbitClient>>,
}

impl BoltGraphRAGProcedures {
    /// Create new Bolt GraphRAG procedures handler
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client: Some(Arc::new(orbit_client)),
        }
    }

    /// Create new Bolt GraphRAG procedures handler without OrbitClient (placeholder mode)
    pub fn new_placeholder() -> Self {
        Self { orbit_client: None }
    }

    /// Execute a GraphRAG procedure call
    pub async fn execute_procedure(
        &self,
        procedure_name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        if self.orbit_client.is_none() {
            return Err(ProtocolError::CypherError(
                "GraphRAG functionality requires OrbitClient integration".to_string(),
            ));
        }

        match procedure_name.to_lowercase().as_str() {
            "orbit.graphrag.buildknowledge" => self.execute_build_knowledge(args).await,
            "orbit.graphrag.extractentities" => self.execute_extract_entities(args).await,
            "orbit.graphrag.ragquery" => self.execute_rag_query(args).await,
            "orbit.graphrag.findpaths" => self.execute_find_paths(args).await,
            "orbit.graphrag.findsimilar" => self.execute_find_similar(args).await,
            "orbit.graphrag.semanticsearch" => self.execute_semantic_search(args).await,
            "orbit.graphrag.getstats" => self.execute_get_stats(args).await,
            "orbit.graphrag.listentities" => self.execute_list_entities(args).await,
            "orbit.graphrag.analyzetrends" => self.execute_analyze_trends(args).await,
            "orbit.graphrag.detectcommunities" => self.execute_detect_communities(args).await,
            "orbit.graphrag.augmentgraph" => self.execute_augment_graph(args).await,
            "orbit.graphrag.processdocuments" => self.execute_process_documents(args).await,
            "orbit.graphrag.streamentities" => self.execute_stream_entities(args).await,
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown GraphRAG procedure: {procedure_name}"
            ))),
        }
    }

    /// Execute orbit.graphrag.buildKnowledge procedure
    /// CALL orbit.graphrag.buildKnowledge(kg_name, document_id, text, metadata, config)
    async fn execute_build_knowledge(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 5 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.buildKnowledge requires 5 arguments: (kg_name, document_id, text, metadata, config)".to_string()
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let document_id = self.extract_string_arg(&args[1], "document_id")?;
        let text = self.extract_string_arg(&args[2], "text")?;
        let metadata = self.parse_metadata_arg(&args[3])?;
        let config = self.parse_config_arg(&args[4])?;

        // Extract configuration
        let extractors = config
            .get("extractors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

        let build_graph = config
            .get("build_graph")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let generate_embeddings = config
            .get("generate_embeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

        // Create document processing request
        let request = GraphRAGDocumentRequest {
            document_id: document_id.clone(),
            text,
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
            .map_err(|e| ProtocolError::CypherError(format!("Document processing failed: {e}")))?;

        // Format as Cypher result
        let columns = vec![
            "kg_name".to_string(),
            "document_id".to_string(),
            "entities_extracted".to_string(),
            "relationships_extracted".to_string(),
            "extractors_used".to_string(),
            "processing_time_ms".to_string(),
            "graph_nodes_created".to_string(),
        ];

        let _rows = [vec![
            Some(kg_name),
            Some(document_id),
            Some(result.entities.len().to_string()),
            Some(result.relationships.len().to_string()),
            Some(result.extractors_used.to_string()),
            Some(result.processing_time_ms.to_string()),
            Some(result.entities.len().to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.extractEntities procedure
    /// CALL orbit.graphrag.extractEntities(text, config)
    async fn execute_extract_entities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.extractEntities requires 2 arguments: (text, config)".to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let text = self.extract_string_arg(&args[0], "text")?;
        let config = self.parse_config_arg(&args[1])?;

        let kg_name = config
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
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

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
            .map_err(|e| ProtocolError::CypherError(format!("Entity extraction failed: {e}")))?;

        // Format as Cypher result with multiple rows (one per entity)
        let columns = vec![
            "entity_text".to_string(),
            "entity_type".to_string(),
            "confidence".to_string(),
            "start_pos".to_string(),
            "end_pos".to_string(),
        ];

        let _rows: Vec<Vec<Option<String>>> = result
            .entities
            .iter()
            .map(|entity| {
                vec![
                    Some(entity.text.clone()),
                    Some(entity.entity_type.to_str().to_string()),
                    Some(entity.confidence.to_string()),
                    Some(entity.start_pos.to_string()),
                    Some(entity.end_pos.to_string()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.ragQuery procedure
    /// CALL orbit.graphrag.ragQuery(kg_name, query_text, config)
    async fn execute_rag_query(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.ragQuery requires 3 arguments: (kg_name, query_text, config)"
                    .to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let query_text = self.extract_string_arg(&args[1], "query_text")?;
        let config = self.parse_config_arg(&args[2])?;

        // Parse config options
        let max_hops = config
            .get("max_hops")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let context_size = config
            .get("context_size")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let llm_provider = config
            .get("llm_provider")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let include_explanation = config
            .get("include_explanation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

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
            .map_err(|e| ProtocolError::CypherError(format!("RAG query failed: {e}")))?;

        // Format as Cypher result
        let columns = vec![
            "response".to_string(),
            "confidence".to_string(),
            "processing_time_ms".to_string(),
            "entities_involved".to_string(),
            "citations".to_string(),
        ];

        let entities_json = serde_json::to_string(&result.entities_involved).unwrap_or_default();
        let citations_json = serde_json::to_string(&result.response.citations).unwrap_or_default();

        let _rows = [vec![
            Some(result.response.response),
            Some(result.response.confidence.to_string()),
            Some(result.processing_times.total_ms.to_string()),
            Some(entities_json),
            Some(citations_json),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.findPaths procedure
    /// CALL orbit.graphrag.findPaths(kg_name, from_entity, to_entity, config)
    async fn execute_find_paths(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 4 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.findPaths requires 4 arguments: (kg_name, from_entity, to_entity, config)".to_string()
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let from_entity = self.extract_string_arg(&args[1], "from_entity")?;
        let to_entity = self.extract_string_arg(&args[2], "to_entity")?;
        let config = self.parse_config_arg(&args[3])?;

        // Parse config options
        let max_hops = config
            .get("max_hops")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let include_explanation = config
            .get("include_explanation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_results = config
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

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
            .map_err(|e| ProtocolError::CypherError(format!("Reasoning failed: {e}")))?;

        // Format as Cypher result - one row per path
        let columns = vec![
            "path_nodes".to_string(),
            "relationships".to_string(),
            "score".to_string(),
            "length".to_string(),
            "explanation".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = paths
            .iter()
            .map(|path| {
                let nodes_json = serde_json::to_string(&path.nodes).unwrap_or_default();
                let relationships_json =
                    serde_json::to_string(&path.relationships).unwrap_or_default();

                vec![
                    Some(nodes_json),
                    Some(relationships_json),
                    Some(path.score.to_string()),
                    Some(path.length.to_string()),
                    Some(if include_explanation {
                        path.explanation.clone()
                    } else {
                        "".to_string()
                    }),
                ]
            })
            .collect();

        // If no paths found, return empty result set with proper structure
        let _final_rows = if rows.is_empty() {
            vec![vec![
                Some("[]".to_string()),
                Some("[]".to_string()),
                Some("0.0".to_string()),
                Some("0".to_string()),
                Some("No paths found".to_string()),
            ]]
        } else {
            rows
        };

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.findSimilar procedure (placeholder)
    /// CALL orbit.graphrag.findSimilar(kg_name, entity, config)
    async fn execute_find_similar(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.findSimilar requires 3 arguments: (kg_name, entity, config)"
                    .to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let entity = self.extract_string_arg(&args[1], "entity")?;
        let _config = self.parse_config_arg(&args[2])?;

        // For now, return a placeholder result
        let columns = vec![
            "entity_name".to_string(),
            "similarity_score".to_string(),
            "shared_properties".to_string(),
        ];

        let _rows = [vec![
            Some(format!("Similar to {entity}")),
            Some("0.0".to_string()),
            Some("{}".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.semanticSearch procedure (placeholder)
    /// CALL orbit.graphrag.semanticSearch(kg_name, query_text, config)
    async fn execute_semantic_search(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.semanticSearch requires 3 arguments: (kg_name, query_text, config)"
                    .to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _query_text = self.extract_string_arg(&args[1], "query_text")?;
        let _config = self.parse_config_arg(&args[2])?;

        // For now, return a placeholder result
        let columns = vec![
            "entity_name".to_string(),
            "entity_type".to_string(),
            "relevance_score".to_string(),
            "context_snippet".to_string(),
        ];

        let _rows = [vec![
            Some("Sample Entity".to_string()),
            Some("Concept".to_string()),
            Some("0.0".to_string()),
            Some("Semantic search not yet implemented".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.getStats procedure
    /// CALL orbit.graphrag.getStats(kg_name)
    async fn execute_get_stats(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.getStats requires 1 argument: (kg_name)".to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();
        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

        // Get statistics
        let stats: GraphRAGStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Failed to get stats: {e}")))?;

        // Format as Cypher result
        let columns = vec![
            "documents_processed".to_string(),
            "entities_extracted".to_string(),
            "relationships_extracted".to_string(),
            "rag_queries_executed".to_string(),
            "reasoning_queries_executed".to_string(),
            "avg_document_processing_time_ms".to_string(),
            "avg_rag_query_time_ms".to_string(),
            "rag_success_rate".to_string(),
        ];

        let _rows = [vec![
            Some(stats.documents_processed.to_string()),
            Some(stats.entities_extracted.to_string()),
            Some(stats.relationships_extracted.to_string()),
            Some(stats.rag_queries_executed.to_string()),
            Some(stats.reasoning_queries_executed.to_string()),
            Some(format!("{:.2}", stats.avg_document_processing_time_ms)),
            Some(format!("{:.2}", stats.avg_rag_query_time_ms)),
            Some(format!("{:.3}", stats.rag_success_rate)),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.listEntities procedure (placeholder)
    /// CALL orbit.graphrag.listEntities(kg_name, config)
    async fn execute_list_entities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.listEntities requires 2 arguments: (kg_name, config)".to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _config = self.parse_config_arg(&args[1])?;

        // For now, return a placeholder result
        let columns = vec![
            "entity_text".to_string(),
            "entity_type".to_string(),
            "confidence".to_string(),
            "properties".to_string(),
            "aliases".to_string(),
        ];

        let _rows = [vec![
            Some("Sample Entity".to_string()),
            Some("Concept".to_string()),
            Some("0.8".to_string()),
            Some("{}".to_string()),
            Some("[]".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.analyzeTrends procedure (placeholder)
    /// CALL orbit.graphrag.analyzeTrends(kg_name, concept, config)
    async fn execute_analyze_trends(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.analyzeTrends requires 3 arguments: (kg_name, concept, config)"
                    .to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _concept = self.extract_string_arg(&args[1], "concept")?;
        let _config = self.parse_config_arg(&args[2])?;

        // For now, return a placeholder result
        let columns = vec![
            "period".to_string(),
            "mention_frequency".to_string(),
            "sentiment".to_string(),
            "innovation_score".to_string(),
        ];

        let _rows = [vec![
            Some("2024".to_string()),
            Some("100".to_string()),
            Some("0.7".to_string()),
            Some("0.8".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.detectCommunities procedure (placeholder)
    /// CALL orbit.graphrag.detectCommunities(kg_name, config)
    async fn execute_detect_communities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.detectCommunities requires 2 arguments: (kg_name, config)"
                    .to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _config = self.parse_config_arg(&args[1])?;

        // For now, return a placeholder result
        let columns = vec![
            "community_id".to_string(),
            "entities".to_string(),
            "dominant_concepts".to_string(),
            "internal_density".to_string(),
        ];

        let _rows = [vec![
            Some("community_1".to_string()),
            Some("[]".to_string()),
            Some("[]".to_string()),
            Some("0.0".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.augmentGraph procedure (placeholder)
    /// CALL orbit.graphrag.augmentGraph(kg_name, config)
    async fn execute_augment_graph(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.augmentGraph requires 2 arguments: (kg_name, config)".to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _config = self.parse_config_arg(&args[1])?;

        // For now, return a placeholder result
        let columns = vec![
            "source_entity".to_string(),
            "target_entity".to_string(),
            "relationship_type".to_string(),
            "confidence".to_string(),
        ];

        let _rows = [vec![
            Some("Entity1".to_string()),
            Some("Entity2".to_string()),
            Some("RELATED_TO".to_string()),
            Some("0.8".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.processDocuments procedure (placeholder)
    /// CALL orbit.graphrag.processDocuments(documents, kg_name, config)
    async fn execute_process_documents(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.processDocuments requires 3 arguments: (documents, kg_name, config)".to_string()
            ));
        }

        let _documents = &args[0];
        let _kg_name = self.extract_string_arg(&args[1], "kg_name")?;
        let _config = self.parse_config_arg(&args[2])?;

        // For now, return a placeholder result
        let columns = vec![
            "document_id".to_string(),
            "success".to_string(),
            "entities_extracted".to_string(),
            "processing_time_ms".to_string(),
            "errors".to_string(),
        ];

        let _rows = [vec![
            Some("doc_1".to_string()),
            Some("true".to_string()),
            Some("0".to_string()),
            Some("0".to_string()),
            Some("[]".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Execute orbit.graphrag.streamEntities procedure (placeholder)
    /// CALL orbit.graphrag.streamEntities(kg_name, config)
    async fn execute_stream_entities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.streamEntities requires 2 arguments: (kg_name, config)".to_string(),
            ));
        }

        let _kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let _config = self.parse_config_arg(&args[1])?;

        // For now, return a placeholder result
        let columns = vec![
            "entity_text".to_string(),
            "entity_type".to_string(),
            "confidence".to_string(),
            "timestamp".to_string(),
            "source_document".to_string(),
        ];

        let _rows = [vec![
            Some("Streaming Entity".to_string()),
            Some("Event".to_string()),
            Some("0.9".to_string()),
            Some(chrono::Utc::now().to_rfc3339()),
            Some("stream_doc_1".to_string()),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
        })
    }

    /// Helper function to extract string argument
    fn extract_string_arg(&self, arg: &JsonValue, arg_name: &str) -> ProtocolResult<String> {
        match arg {
            JsonValue::String(s) => Ok(s.clone()),
            _ => Err(ProtocolError::CypherError(format!(
                "{arg_name} argument must be a string"
            ))),
        }
    }

    /// Helper function to parse metadata argument
    fn parse_metadata_arg(&self, arg: &JsonValue) -> ProtocolResult<HashMap<String, JsonValue>> {
        match arg {
            JsonValue::Object(obj) => Ok(obj.clone().into_iter().collect()),
            JsonValue::Null => Ok(HashMap::new()),
            _ => Err(ProtocolError::CypherError(
                "Metadata argument must be an object or null".to_string(),
            )),
        }
    }

    /// Helper function to parse config argument
    fn parse_config_arg(&self, arg: &JsonValue) -> ProtocolResult<HashMap<String, JsonValue>> {
        match arg {
            JsonValue::Object(obj) => Ok(obj.clone().into_iter().collect()),
            JsonValue::Null => Ok(HashMap::new()),
            _ => Err(ProtocolError::CypherError(
                "Config argument must be an object or null".to_string(),
            )),
        }
    }
}
