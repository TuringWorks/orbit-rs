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
    storage::GraphRAGStorage,
    GraphRAGActor,
};
use orbit_client::OrbitClient;
use orbit_shared::{graphrag::ReasoningPath, Key};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
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
            rows: Vec::new(),
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
            rows: Vec::new(),
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
            rows: Vec::new(),
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
            rows: Vec::new(),
        })
    }

    /// Execute orbit.graphrag.findSimilar procedure
    /// CALL orbit.graphrag.findSimilar(kg_name, entity, config)
    async fn execute_find_similar(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.findSimilar requires 3 arguments: (kg_name, entity, config)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let entity_text = self.extract_string_arg(&args[1], "entity")?;
        let config = self.parse_config_arg(&args[2])?;

        let limit = config
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let similarity_threshold = config
            .get("similarity_threshold")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.7);

        // Get storage and find similar entities
        let storage = self.get_storage(&kg_name).await?;
        let nodes = storage.list_nodes().await?;

        // Find the target entity
        let target_node = nodes
            .iter()
            .find(|n| n.text == entity_text || n.id == entity_text);

        if target_node.is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Entity '{}' not found in knowledge graph",
                entity_text
            )));
        }

        let target = target_node.unwrap();
        let target_embedding = target
            .embeddings
            .values()
            .next()
            .cloned()
            .unwrap_or_default();

        let columns = vec![
            "entity_id".to_string(),
            "entity_text".to_string(),
            "entity_type".to_string(),
            "similarity".to_string(),
            "confidence".to_string(),
        ];

        let mut rows = Vec::new();

        if target_embedding.is_empty() {
            // Text similarity fallback
            let target_lower = target.text.to_lowercase();
            let target_words: std::collections::HashSet<&str> =
                target_lower.split_whitespace().collect();
            for node in nodes.iter().filter(|n| n.id != target.id).take(limit) {
                let node_lower = node.text.to_lowercase();
                let node_words: std::collections::HashSet<&str> =
                    node_lower.split_whitespace().collect();
                let intersection = target_words.intersection(&node_words).count();
                let union = target_words.union(&node_words).count();
                if union > 0 {
                    let similarity = intersection as f32 / union as f32;
                    if similarity >= similarity_threshold {
                        rows.push(vec![
                            Some(node.id.clone()),
                            Some(node.text.clone()),
                            Some(format!("{:?}", node.entity_type)),
                            Some(format!("{:.4}", similarity)),
                            Some(format!("{:.4}", node.confidence)),
                        ]);
                    }
                }
            }
        } else {
            // Embedding similarity
            let mut similarities: Vec<(f32, &crate::protocols::graphrag::storage::GraphRAGNode)> = nodes
                .iter()
                .filter(|n| n.id != target.id)
                .filter_map(|n| {
                    let node_embedding = n.embeddings.values().next()?;
                    if node_embedding.len() != target_embedding.len() {
                        return None;
                    }

                    let dot_product: f32 = target_embedding
                        .iter()
                        .zip(node_embedding.iter())
                        .map(|(a, b)| a * b)
                        .sum();
                    let target_norm: f32 = target_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let node_norm: f32 = node_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

                    if target_norm == 0.0 || node_norm == 0.0 {
                        return None;
                    }

                    let similarity = dot_product / (target_norm * node_norm);
                    if similarity >= similarity_threshold {
                        Some((similarity, n))
                    } else {
                        None
                    }
                })
                .collect();

            similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            for (similarity, node) in similarities.into_iter().take(limit) {
                rows.push(vec![
                    Some(node.id.clone()),
                    Some(node.text.clone()),
                    Some(format!("{:?}", node.entity_type)),
                    Some(format!("{:.4}", similarity)),
                    Some(format!("{:.4}", node.confidence)),
                ]);
            }
        }

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graphrag.semanticSearch procedure
    /// CALL orbit.graphrag.semanticSearch(kg_name, query_text, config)
    async fn execute_semantic_search(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.semanticSearch requires 3 arguments: (kg_name, query_text, config)"
                    .to_string(),
            ));
        }

        let orbit_client = self.orbit_client.as_ref().unwrap();
        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let query_text = self.extract_string_arg(&args[1], "query_text")?;
        let config = self.parse_config_arg(&args[2])?;

        let max_results = config
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(20);

        // Use RAG query to find relevant entities and context
        let rag_query = GraphRAGQuery {
            query_text: query_text.clone(),
            max_hops: config
                .get("max_hops")
                .and_then(|v| v.as_u64().map(|n| n as u32))
                .or_else(|| config.get("max_hops").and_then(|v| v.as_i64().map(|n| n as u32))),
            context_size: config
                .get("context_size")
                .and_then(|v| v.as_u64().map(|n| n as usize))
                .or_else(|| config.get("context_size").and_then(|v| v.as_i64().map(|n| n as usize))),
            llm_provider: config
                .get("llm_provider")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            search_strategy: None,
            include_explanation: false,
            max_results: Some(max_results),
        };

        // Get GraphRAG actor reference
        let actor_ref = orbit_client
            .actor_reference::<GraphRAGActor>(Key::StringKey {
                key: kg_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Actor error: {e}")))?;

        // Execute RAG query
        let rag_result: GraphRAGQueryResult = actor_ref
            .invoke("query_rag", vec![serde_json::to_value(rag_query).unwrap()])
            .await
            .map_err(|e| ProtocolError::CypherError(format!("Semantic search failed: {e}")))?;

        let columns = vec![
            "type".to_string(),
            "content".to_string(),
            "entity_id".to_string(),
            "score".to_string(),
        ];

        let mut rows = Vec::new();

        // Add entities
        for entity_id in rag_result.entities_involved.iter().take(max_results) {
            rows.push(vec![
                Some("entity".to_string()),
                Some(entity_id.clone()),
                Some(entity_id.clone()),
                Some("1.0".to_string()),
            ]);
        }

        // Add reasoning paths
        if let Some(ref paths) = rag_result.reasoning_paths {
            for path in paths.iter().take(max_results.saturating_sub(rows.len())) {
                rows.push(vec![
                    Some("path".to_string()),
                    Some(path.nodes.join(" -> ")),
                    None,
                    Some(format!("{:.4}", path.score)),
                ]);
            }
        }

        // Add response
        rows.push(vec![
            Some("response".to_string()),
            Some(rag_result.response.response.clone()),
            None,
            Some(format!("{:.4}", rag_result.response.confidence)),
        ]);

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
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
            rows: Vec::new(),
        })
    }

    /// Execute orbit.graphrag.listEntities procedure
    /// CALL orbit.graphrag.listEntities(kg_name, config)
    async fn execute_list_entities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.listEntities requires 2 arguments: (kg_name, config)".to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let config = self.parse_config_arg(&args[1])?;

        let limit = config
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let entity_type_filter = config
            .get("entity_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get storage and list entities
        let storage = self.get_storage(&kg_name).await?;
        let mut nodes = storage.list_nodes().await?;

        // Apply filters
        if let Some(ref filter_type) = entity_type_filter {
            nodes.retain(|n| format!("{:?}", n.entity_type) == *filter_type);
        }

        // Apply limit
        if let Some(limit_val) = limit {
            nodes.truncate(limit_val);
        }

        let columns = vec![
            "entity_id".to_string(),
            "entity_text".to_string(),
            "entity_type".to_string(),
            "confidence".to_string(),
            "labels".to_string(),
            "source_documents".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = nodes
            .into_iter()
            .map(|n| {
                vec![
                    Some(n.id),
                    Some(n.text),
                    Some(format!("{:?}", n.entity_type)),
                    Some(format!("{:.4}", n.confidence)),
                    Some(serde_json::to_string(&n.labels).unwrap_or_default()),
                    Some(serde_json::to_string(&n.source_documents).unwrap_or_default()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graphrag.analyzeTrends procedure
    /// CALL orbit.graphrag.analyzeTrends(kg_name, concept, config)
    async fn execute_analyze_trends(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 3 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.analyzeTrends requires 3 arguments: (kg_name, concept, config)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let concept = self.extract_string_arg(&args[1], "concept")?;
        let config = self.parse_config_arg(&args[2])?;

        let time_window_days = config
            .get("time_window_days")
            .and_then(|v| v.as_u64())
            .map(|n| n as i64)
            .unwrap_or(30);

        // Get storage and analyze trends
        let storage = self.get_storage(&kg_name).await?;
        let nodes = storage.list_nodes().await?;
        let relationships = storage.list_relationships().await?;

        let now = chrono::Utc::now().timestamp_millis();
        let window_start = now - (time_window_days * 24 * 60 * 60 * 1000);

        // Find entities related to the concept
        let concept_entities: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n.text.to_lowercase().contains(&concept.to_lowercase())
                    || n.labels.iter().any(|l| l.to_lowercase().contains(&concept.to_lowercase()))
            })
            .collect();

        // Analyze relationship trends over time
        let mut time_buckets: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        let bucket_size_ms = 24 * 60 * 60 * 1000; // 1 day buckets

        for rel in &relationships {
            if rel.created_at >= window_start {
                let bucket = (rel.created_at / bucket_size_ms) * bucket_size_ms;
                *time_buckets.entry(bucket).or_insert(0) += 1;
            }
        }

        let columns = vec![
            "timestamp".to_string(),
            "relationship_count".to_string(),
            "concept_entities_found".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = time_buckets
            .into_iter()
            .map(|(timestamp, count)| {
                vec![
                    Some(timestamp.to_string()),
                    Some(count.to_string()),
                    Some(concept_entities.len().to_string()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graphrag.detectCommunities procedure
    /// CALL orbit.graphrag.detectCommunities(kg_name, config)
    async fn execute_detect_communities(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graphrag.detectCommunities requires 2 arguments: (kg_name, config)"
                    .to_string(),
            ));
        }

        let kg_name = self.extract_string_arg(&args[0], "kg_name")?;
        let config = self.parse_config_arg(&args[1])?;

        let min_community_size = config
            .get("min_community_size")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(3);

        // Get storage
        let storage = self.get_storage(&kg_name).await?;
        let nodes = storage.list_nodes().await?;
        let relationships = storage.list_relationships().await?;

        // Build adjacency list
        let mut adjacency: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for rel in &relationships {
            adjacency
                .entry(rel.from_entity_id.clone())
                .or_insert_with(Vec::new)
                .push(rel.to_entity_id.clone());
            adjacency
                .entry(rel.to_entity_id.clone())
                .or_insert_with(Vec::new)
                .push(rel.from_entity_id.clone());
        }

        // Community detection using connected components
        let mut visited = std::collections::HashSet::new();
        let mut communities = Vec::new();

        for node in &nodes {
            if visited.contains(&node.id) {
                continue;
            }

            let mut community = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(node.id.clone());
            visited.insert(node.id.clone());

            while let Some(current_id) = queue.pop_front() {
                community.push(current_id.clone());

                if let Some(neighbors) = adjacency.get(&current_id) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }

            if community.len() >= min_community_size {
                communities.push(community);
            }
        }

        let columns = vec![
            "community_id".to_string(),
            "size".to_string(),
            "entity_ids".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = communities
            .into_iter()
            .enumerate()
            .map(|(idx, community)| {
                vec![
                    Some(idx.to_string()),
                    Some(community.len().to_string()),
                    Some(serde_json::to_string(&community).unwrap_or_default()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
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
            rows: Vec::new(),
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
            rows: Vec::new(),
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
            rows: Vec::new(),
        })
    }

    /// Helper to get GraphRAG storage instance
    async fn get_storage(&self, kg_name: &str) -> ProtocolResult<GraphRAGStorage> {
        // Create storage instance using standard data directory
        let data_dir = PathBuf::from("data/graphrag");
        let storage = GraphRAGStorage::new(data_dir, kg_name.to_string());
        storage.initialize().await?;
        Ok(storage)
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
