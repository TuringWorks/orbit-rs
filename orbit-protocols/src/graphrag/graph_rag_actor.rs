//! GraphRAG Orchestrator Actor
//!
//! This module provides the main GraphRAG actor that orchestrates knowledge graph
//! construction, entity extraction, multi-hop reasoning, and RAG capabilities.

use crate::graphrag::{
    entity_extraction::DocumentProcessingRequest, knowledge_graph::DocumentGraphRequest,
    multi_hop_reasoning::ReasoningQuery, EntityExtractionActor, KnowledgeGraphBuilder,
    MultiHopReasoningEngine,
};
use orbit_client::OrbitClient;
use orbit_shared::graphrag::{
    ContextItem, ContextSourceType, LLMProvider, RAGResponse, SearchStrategy,
};
use orbit_shared::{Addressable, OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Main GraphRAG orchestrator actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGActor {
    /// Knowledge graph name/identifier
    pub kg_name: String,

    /// Configuration settings
    pub config: GraphRAGConfig,

    /// Statistics and metrics
    pub stats: GraphRAGStats,

    /// Entity extraction component
    pub entity_extractor: Option<EntityExtractionActor>,

    /// Knowledge graph builder component
    pub graph_builder: Option<KnowledgeGraphBuilder>,

    /// Multi-hop reasoning engine
    pub reasoning_engine: Option<MultiHopReasoningEngine>,

    /// Available LLM providers
    pub llm_providers: HashMap<String, LLMProvider>,

    /// Default LLM provider name
    pub default_llm_provider: Option<String>,

    /// Creation timestamp
    pub created_at: i64,

    /// Last activity timestamp
    pub updated_at: i64,
}

/// Configuration for GraphRAG operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGConfig {
    /// Maximum entities per document
    pub max_entities_per_document: usize,

    /// Maximum relationships per document
    pub max_relationships_per_document: usize,

    /// Default embedding dimension
    pub embedding_dimension: usize,

    /// Similarity threshold for deduplication
    pub similarity_threshold: f32,

    /// Maximum hops for multi-hop reasoning
    pub max_hops: u32,

    /// RAG context size
    pub rag_context_size: usize,

    /// Enable entity deduplication
    pub enable_entity_deduplication: bool,

    /// Enable relationship inference
    pub enable_relationship_inference: bool,

    /// Default search strategy
    pub default_search_strategy: SearchStrategy,

    /// Query timeout in milliseconds
    pub query_timeout_ms: u64,
}

impl Default for GraphRAGConfig {
    fn default() -> Self {
        use crate::graphrag::defaults;
        Self {
            max_entities_per_document: defaults::DEFAULT_MAX_ENTITIES_PER_DOCUMENT,
            max_relationships_per_document: defaults::DEFAULT_MAX_RELATIONSHIPS_PER_DOCUMENT,
            embedding_dimension: defaults::DEFAULT_EMBEDDING_DIMENSION,
            similarity_threshold: defaults::DEFAULT_SIMILARITY_THRESHOLD,
            max_hops: defaults::DEFAULT_MAX_HOPS,
            rag_context_size: defaults::DEFAULT_RAG_CONTEXT_SIZE,
            enable_entity_deduplication: true,
            enable_relationship_inference: true,
            default_search_strategy: SearchStrategy::Balanced,
            query_timeout_ms: 30_000,
        }
    }
}

/// Statistics for GraphRAG operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphRAGStats {
    /// Total documents processed
    pub documents_processed: u64,

    /// Total RAG queries executed
    pub rag_queries_executed: u64,

    /// Total reasoning queries executed
    pub reasoning_queries_executed: u64,

    /// Total entities extracted
    pub entities_extracted: u64,

    /// Total relationships extracted
    pub relationships_extracted: u64,

    /// Average document processing time (ms)
    pub avg_document_processing_time_ms: f64,

    /// Average RAG query time (ms)
    pub avg_rag_query_time_ms: f64,

    /// Average reasoning query time (ms)
    pub avg_reasoning_query_time_ms: f64,

    /// Success rate for RAG queries
    pub rag_success_rate: f32,

    /// LLM provider usage statistics
    pub llm_usage_stats: HashMap<String, u64>,

    /// Last statistics update
    pub last_updated: i64,
}

/// Document processing request for GraphRAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGDocumentRequest {
    /// Document ID
    pub document_id: String,

    /// Document text content
    pub text: String,

    /// Document metadata
    pub metadata: HashMap<String, serde_json::Value>,

    /// Whether to build knowledge graph
    pub build_knowledge_graph: bool,

    /// Whether to generate embeddings
    pub generate_embeddings: bool,

    /// Specific extractors to use
    pub extractors: Option<Vec<String>>,
}

/// RAG query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGQuery {
    /// Query text
    pub query_text: String,

    /// Maximum hops for graph traversal
    pub max_hops: Option<u32>,

    /// Context size for RAG
    pub context_size: Option<usize>,

    /// LLM provider to use
    pub llm_provider: Option<String>,

    /// Search strategy
    pub search_strategy: Option<SearchStrategy>,

    /// Include reasoning explanation
    pub include_explanation: bool,

    /// Maximum results to return
    pub max_results: Option<usize>,
}

/// GraphRAG query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGQueryResult {
    /// Generated response
    pub response: RAGResponse,

    /// Reasoning paths used (if requested)
    pub reasoning_paths: Option<Vec<orbit_shared::graphrag::ReasoningPath>>,

    /// Graph entities involved
    pub entities_involved: Vec<String>,

    /// Processing time breakdown
    pub processing_times: ProcessingTimes,
}

/// Processing time breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingTimes {
    /// Entity extraction time (ms)
    pub entity_extraction_ms: u64,

    /// Graph traversal time (ms)
    pub graph_traversal_ms: u64,

    /// Vector search time (ms)
    pub vector_search_ms: u64,

    /// LLM generation time (ms)
    pub llm_generation_ms: u64,

    /// Total time (ms)
    pub total_ms: u64,
}

impl GraphRAGActor {
    /// Create a new GraphRAG actor
    pub fn new(kg_name: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            kg_name,
            config: GraphRAGConfig::default(),
            stats: GraphRAGStats::default(),
            entity_extractor: None,
            graph_builder: None,
            reasoning_engine: None,
            llm_providers: HashMap::new(),
            default_llm_provider: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create GraphRAG actor with configuration
    pub fn with_config(kg_name: String, config: GraphRAGConfig) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            kg_name,
            config,
            stats: GraphRAGStats::default(),
            entity_extractor: None,
            graph_builder: None,
            reasoning_engine: None,
            llm_providers: HashMap::new(),
            default_llm_provider: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Initialize GraphRAG components
    pub fn initialize_components(&mut self) {
        info!(kg_name = %self.kg_name, "Initializing GraphRAG components");

        // Initialize entity extractor
        self.entity_extractor = Some(EntityExtractionActor::new());

        // Initialize knowledge graph builder
        self.graph_builder = Some(KnowledgeGraphBuilder::new(self.kg_name.clone()));

        // Initialize reasoning engine
        self.reasoning_engine = Some(MultiHopReasoningEngine::new(self.config.max_hops));

        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Add LLM provider
    pub fn add_llm_provider(&mut self, name: String, provider: LLMProvider) {
        self.llm_providers.insert(name.clone(), provider);

        // Set as default if it's the first one
        if self.default_llm_provider.is_none() {
            self.default_llm_provider = Some(name);
        }

        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Process document and build knowledge graph
    pub async fn process_document(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        request: GraphRAGDocumentRequest,
    ) -> OrbitResult<DocumentProcessingResult> {
        let start_time = std::time::Instant::now();

        info!(
            kg_name = %self.kg_name,
            document_id = %request.document_id,
            text_length = request.text.len(),
            "Processing document for GraphRAG"
        );

        // Ensure components are initialized
        if self.entity_extractor.is_none() || self.graph_builder.is_none() {
            self.initialize_components();
        }

        let mut result = DocumentProcessingResult {
            document_id: request.document_id.clone(),
            entities_extracted: 0,
            relationships_extracted: 0,
            graph_nodes_created: 0,
            graph_relationships_created: 0,
            processing_time_ms: 0,
            warnings: Vec::new(),
        };

        // Step 1: Extract entities and relationships
        if let Some(ref mut extractor) = self.entity_extractor {
            let extraction_request = DocumentProcessingRequest {
                document_id: request.document_id.clone(),
                text: request.text.clone(),
                metadata: request.metadata.clone(),
                extractors: request.extractors,
                confidence_threshold: None,
            };

            match extractor.process_document(extraction_request).await {
                Ok(extraction_result) => {
                    result.entities_extracted = extraction_result.entities.len();
                    result.relationships_extracted = extraction_result.relationships.len();

                    // Step 2: Build knowledge graph if requested
                    if request.build_knowledge_graph {
                        if let Some(ref mut builder) = self.graph_builder {
                            let graph_request = DocumentGraphRequest {
                                document_id: request.document_id.clone(),
                                entities: extraction_result.entities,
                                relationships: extraction_result.relationships,
                                metadata: request.metadata,
                                config_override: None,
                            };

                            match builder.process_document(orbit_client, graph_request).await {
                                Ok(graph_result) => {
                                    result.graph_nodes_created = graph_result.entities_created;
                                    result.graph_relationships_created =
                                        graph_result.relationships_created;
                                    result.warnings.extend(graph_result.warnings);
                                }
                                Err(e) => {
                                    result
                                        .warnings
                                        .push(format!("Knowledge graph construction failed: {e}"));
                                    warn!(error = %e, "Knowledge graph construction failed");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    result
                        .warnings
                        .push(format!("Entity extraction failed: {e}"));
                    warn!(error = %e, "Entity extraction failed");
                }
            }
        }

        let processing_time = start_time.elapsed();
        result.processing_time_ms = processing_time.as_millis() as u64;

        // Update statistics
        self.update_document_stats(&result);

        info!(
            document_id = %request.document_id,
            entities_extracted = result.entities_extracted,
            graph_nodes_created = result.graph_nodes_created,
            processing_time_ms = result.processing_time_ms,
            "Document processing completed"
        );

        Ok(result)
    }

    /// Execute GraphRAG query
    pub async fn query_rag(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        query: GraphRAGQuery,
    ) -> OrbitResult<GraphRAGQueryResult> {
        let start_time = std::time::Instant::now();

        info!(
            kg_name = %self.kg_name,
            query_text = %query.query_text,
            "Executing GraphRAG query"
        );

        // Ensure components are initialized
        if self.reasoning_engine.is_none() {
            self.initialize_components();
        }

        let mut processing_times = ProcessingTimes {
            entity_extraction_ms: 0,
            graph_traversal_ms: 0,
            vector_search_ms: 0,
            llm_generation_ms: 0,
            total_ms: 0,
        };

        // Step 1: Extract entities from query (for graph traversal starting points)
        let query_start_time = std::time::Instant::now();
        let query_entities = self.extract_query_entities(&query.query_text).await?;
        processing_times.entity_extraction_ms = query_start_time.elapsed().as_millis() as u64;

        // Step 2: Perform graph traversal and reasoning
        let graph_start_time = std::time::Instant::now();
        let reasoning_paths = if query_entities.len() >= 2 {
            // Multi-entity reasoning
            self.perform_multi_entity_reasoning(orbit_client.clone(), &query_entities, &query)
                .await?
        } else {
            // Single entity expansion
            self.perform_single_entity_expansion(orbit_client.clone(), &query_entities, &query)
                .await?
        };
        processing_times.graph_traversal_ms = graph_start_time.elapsed().as_millis() as u64;

        // Step 3: Collect context from various sources
        let context_start_time = std::time::Instant::now();
        let context_items = self.collect_context(&query, &reasoning_paths).await?;
        processing_times.vector_search_ms = context_start_time.elapsed().as_millis() as u64;

        // Step 4: Generate response using LLM
        let llm_start_time = std::time::Instant::now();
        let rag_response = self.generate_rag_response(&query, &context_items).await?;
        processing_times.llm_generation_ms = llm_start_time.elapsed().as_millis() as u64;

        let total_time = start_time.elapsed();
        processing_times.total_ms = total_time.as_millis() as u64;

        // Collect entities involved in the query
        let entities_involved: Vec<String> = reasoning_paths
            .iter()
            .flat_map(|path| path.nodes.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let total_time_ms = processing_times.total_ms;

        let result = GraphRAGQueryResult {
            response: rag_response,
            reasoning_paths: if query.include_explanation {
                Some(reasoning_paths)
            } else {
                None
            },
            entities_involved,
            processing_times,
        };

        // Update statistics
        self.update_query_stats(total_time.as_millis() as u64, true);

        info!(
            query_text = %query.query_text,
            entities_involved = result.entities_involved.len(),
            reasoning_paths = result.reasoning_paths.as_ref().map(|p| p.len()).unwrap_or(0),
            total_time_ms = total_time_ms,
            "GraphRAG query completed"
        );

        Ok(result)
    }

    /// Extract entities from query text
    async fn extract_query_entities(&self, query_text: &str) -> OrbitResult<Vec<String>> {
        // Simplified entity extraction from query
        // In practice, this would use NLP libraries or the entity extractor
        let words: Vec<String> = query_text
            .split_whitespace()
            .filter(|word| word.len() > 3 && word.chars().next().unwrap().is_uppercase())
            .map(|word| word.to_string())
            .take(5) // Limit to 5 potential entities
            .collect();

        Ok(words)
    }

    /// Perform multi-entity reasoning between extracted entities
    async fn perform_multi_entity_reasoning(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        entities: &[String],
        query: &GraphRAGQuery,
    ) -> OrbitResult<Vec<orbit_shared::graphrag::ReasoningPath>> {
        let mut all_paths = Vec::new();

        if let Some(ref mut reasoning_engine) = self.reasoning_engine {
            // Find paths between all pairs of entities
            for i in 0..entities.len() {
                for j in i + 1..entities.len() {
                    let reasoning_query = ReasoningQuery {
                        from_entity: entities[i].clone(),
                        to_entity: entities[j].clone(),
                        max_hops: query.max_hops,
                        relationship_types: None,
                        include_explanation: query.include_explanation,
                        max_results: query.max_results,
                    };

                    match reasoning_engine
                        .find_paths(orbit_client.clone(), &self.kg_name, reasoning_query)
                        .await
                    {
                        Ok(paths) => all_paths.extend(paths),
                        Err(e) => {
                            warn!(
                                from_entity = %entities[i],
                                to_entity = %entities[j],
                                error = %e,
                                "Failed to find reasoning path between entities"
                            );
                        }
                    }
                }
            }
        }

        Ok(all_paths)
    }

    /// Perform single entity expansion (find related entities)
    async fn perform_single_entity_expansion(
        &self,
        _orbit_client: Arc<OrbitClient>,
        _entities: &[String],
        _query: &GraphRAGQuery,
    ) -> OrbitResult<Vec<orbit_shared::graphrag::ReasoningPath>> {
        // Simplified implementation - would expand around single entity
        // to find related entities and create reasoning paths
        Ok(Vec::new())
    }

    /// Collect context from various sources (graph, vector, text)
    async fn collect_context(
        &self,
        query: &GraphRAGQuery,
        reasoning_paths: &[orbit_shared::graphrag::ReasoningPath],
    ) -> OrbitResult<Vec<ContextItem>> {
        let mut context_items = Vec::new();

        // Add context from reasoning paths
        for path in reasoning_paths {
            let context_item = ContextItem {
                content: format!("Connection found: {}", path.explanation),
                source_type: ContextSourceType::Graph,
                relevance_score: path.score,
                source_id: format!("path_{}", path.nodes.join("_")),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("path_length".to_string(), serde_json::json!(path.length));
                    meta.insert("nodes".to_string(), serde_json::json!(path.nodes));
                    meta
                },
            };
            context_items.push(context_item);
        }

        // TODO: Add vector similarity search context
        // TODO: Add full-text search context

        // Sort by relevance and limit context size
        context_items.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        context_items.truncate(query.context_size.unwrap_or(self.config.rag_context_size));

        Ok(context_items)
    }

    /// Generate RAG response using LLM
    async fn generate_rag_response(
        &self,
        query: &GraphRAGQuery,
        context_items: &[ContextItem],
    ) -> OrbitResult<RAGResponse> {
        let llm_provider_name = query
            .llm_provider
            .as_ref()
            .or(self.default_llm_provider.as_ref())
            .ok_or_else(|| OrbitError::internal("No LLM provider configured"))?;

        let _llm_provider = self.llm_providers.get(llm_provider_name).ok_or_else(|| {
            OrbitError::internal(format!("LLM provider '{llm_provider_name}' not found"))
        })?;

        // Build context text
        let context_text = context_items
            .iter()
            .map(|item| format!("- {}", item.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Create simplified RAG response (would integrate with actual LLM in practice)
        let response_text = if context_items.is_empty() {
            format!(
                "I don't have specific information about '{}' in the knowledge graph.",
                query.query_text
            )
        } else {
            format!(
                "Based on the knowledge graph, here's what I found about '{}':\n\n{}",
                query.query_text, context_text
            )
        };

        let citations = context_items
            .iter()
            .map(|item| item.source_id.clone())
            .collect();

        Ok(RAGResponse {
            response: response_text,
            context: context_items.to_vec(),
            confidence: if context_items.is_empty() { 0.1 } else { 0.8 },
            citations,
            processing_time_ms: 100, // Mock processing time
            metadata: HashMap::new(),
        })
    }

    /// Update document processing statistics
    fn update_document_stats(&mut self, result: &DocumentProcessingResult) {
        self.stats.documents_processed += 1;
        self.stats.entities_extracted += result.entities_extracted as u64;
        self.stats.relationships_extracted += result.relationships_extracted as u64;

        // Update average processing time
        let total_time = (self.stats.avg_document_processing_time_ms
            * (self.stats.documents_processed - 1) as f64)
            + result.processing_time_ms as f64;
        self.stats.avg_document_processing_time_ms =
            total_time / self.stats.documents_processed as f64;

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
        self.updated_at = self.stats.last_updated;
    }

    /// Update query statistics
    fn update_query_stats(&mut self, query_time_ms: u64, success: bool) {
        if success {
            self.stats.rag_queries_executed += 1;

            // Update average query time
            let total_time = (self.stats.avg_rag_query_time_ms
                * (self.stats.rag_queries_executed - 1) as f64)
                + query_time_ms as f64;
            self.stats.avg_rag_query_time_ms = total_time / self.stats.rag_queries_executed as f64;

            // Update success rate
            self.stats.rag_success_rate = self.stats.rag_queries_executed as f32
                / (self.stats.rag_queries_executed + 1) as f32;
        }

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
        self.updated_at = self.stats.last_updated;
    }

    /// Get GraphRAG statistics
    pub fn get_stats(&self) -> &GraphRAGStats {
        &self.stats
    }

    /// Update configuration
    pub fn update_config(&mut self, config: GraphRAGConfig) {
        self.config = config;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

impl Default for GraphRAGActor {
    fn default() -> Self {
        Self::new("default".to_string())
    }
}

impl Addressable for GraphRAGActor {
    fn addressable_type() -> &'static str {
        "GraphRAGActor"
    }
}

/// Document processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProcessingResult {
    /// Document ID
    pub document_id: String,

    /// Number of entities extracted
    pub entities_extracted: usize,

    /// Number of relationships extracted
    pub relationships_extracted: usize,

    /// Number of graph nodes created
    pub graph_nodes_created: usize,

    /// Number of graph relationships created
    pub graph_relationships_created: usize,

    /// Total processing time (ms)
    pub processing_time_ms: u64,

    /// Any warnings or errors
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphrag_actor_creation() {
        let actor = GraphRAGActor::new("test_kg".to_string());
        assert_eq!(actor.kg_name, "test_kg");
        assert_eq!(actor.stats.documents_processed, 0);
        assert!(actor.entity_extractor.is_none());
    }

    #[test]
    fn test_graphrag_config() {
        let config = GraphRAGConfig::default();
        assert_eq!(
            config.max_entities_per_document,
            crate::graphrag::defaults::DEFAULT_MAX_ENTITIES_PER_DOCUMENT
        );
        assert_eq!(config.max_hops, crate::graphrag::defaults::DEFAULT_MAX_HOPS);
        assert!(config.enable_entity_deduplication);
    }

    #[test]
    fn test_component_initialization() {
        let mut actor = GraphRAGActor::new("test".to_string());
        assert!(actor.entity_extractor.is_none());

        actor.initialize_components();

        assert!(actor.entity_extractor.is_some());
        assert!(actor.graph_builder.is_some());
        assert!(actor.reasoning_engine.is_some());
    }

    #[tokio::test]
    async fn test_query_entity_extraction() {
        let actor = GraphRAGActor::new("test".to_string());
        let entities = actor
            .extract_query_entities("What is the relationship between Apple and Microsoft?")
            .await
            .unwrap();

        // Should extract capitalized words as potential entities
        assert!(!entities.is_empty());
        assert!(entities
            .iter()
            .any(|e| e.contains("Apple") || e.contains("Microsoft")));
    }
}
