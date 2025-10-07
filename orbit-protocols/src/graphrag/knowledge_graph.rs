//! Knowledge Graph Construction Module
//!
//! This module provides functionality for building and maintaining knowledge graphs
//! from extracted entities and relationships, integrating with the existing graph
//! database and vector store infrastructure.

use orbit_client::OrbitClient;
use orbit_shared::graphrag::{EntityType, ExtractedEntity, ExtractedRelationship};
use orbit_shared::{Addressable, Key, OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Knowledge graph builder that orchestrates entity/relationship storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphBuilder {
    /// Knowledge graph name/identifier
    pub kg_name: String,

    /// Configuration settings
    pub config: KnowledgeGraphConfig,

    /// Statistics and metrics
    pub stats: KnowledgeGraphStats,

    /// Entity deduplication settings
    pub deduplication_config: EntityDeduplicationConfig,

    /// Entity mapping cache (text -> node_id)
    pub entity_cache: HashMap<String, String>,

    /// Relationship cache for duplicate detection
    pub relationship_cache: HashSet<String>,

    /// Creation timestamp
    pub created_at: i64,

    /// Last activity timestamp
    pub updated_at: i64,
}

/// Configuration for knowledge graph construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphConfig {
    /// Maximum entities per document to process
    pub max_entities_per_document: usize,

    /// Maximum relationships per document
    pub max_relationships_per_document: usize,

    /// Minimum confidence threshold for entities
    pub entity_confidence_threshold: f32,

    /// Minimum confidence threshold for relationships  
    pub relationship_confidence_threshold: f32,

    /// Enable automatic relationship inference
    pub enable_relationship_inference: bool,

    /// Enable entity property enrichment
    pub enable_property_enrichment: bool,

    /// Maximum graph size (nodes + relationships)
    pub max_graph_size: usize,

    /// Graph database backend name
    pub graph_backend: String,

    /// Vector store backend name
    pub vector_backend: String,

    /// Default embedding model for entities
    pub default_embedding_model: String,
}

impl Default for KnowledgeGraphConfig {
    fn default() -> Self {
        Self {
            max_entities_per_document: 100,
            max_relationships_per_document: 200,
            entity_confidence_threshold: 0.7,
            relationship_confidence_threshold: 0.6,
            enable_relationship_inference: true,
            enable_property_enrichment: true,
            max_graph_size: 1_000_000,
            graph_backend: "default".to_string(),
            vector_backend: "default".to_string(),
            default_embedding_model: "sentence-transformer".to_string(),
        }
    }
}

/// Statistics for knowledge graph construction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeGraphStats {
    /// Total documents processed
    pub documents_processed: u64,

    /// Total entities added to graph
    pub entities_created: u64,

    /// Total relationships added to graph
    pub relationships_created: u64,

    /// Entities deduplicated/merged
    pub entities_merged: u64,

    /// Relationships deduplicated
    pub relationships_deduplicated: u64,

    /// Average processing time per document (ms)
    pub avg_processing_time_ms: f64,

    /// Current graph size (nodes)
    pub current_node_count: u64,

    /// Current graph size (relationships)
    pub current_relationship_count: u64,

    /// Entities by type distribution
    pub entities_by_type: HashMap<EntityType, u64>,

    /// Relationships by type distribution
    pub relationships_by_type: HashMap<String, u64>,

    /// Processing errors count
    pub error_count: u64,

    /// Last statistics update
    pub last_updated: i64,
}

/// Entity deduplication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDeduplicationConfig {
    /// Enable exact text matching
    pub exact_text_matching: bool,

    /// Enable fuzzy text matching
    pub fuzzy_text_matching: bool,

    /// Fuzzy matching threshold (0.0 to 1.0)
    pub fuzzy_threshold: f32,

    /// Enable semantic similarity matching
    pub semantic_similarity_matching: bool,

    /// Semantic similarity threshold
    pub semantic_threshold: f32,

    /// Consider entity types in deduplication
    pub consider_entity_types: bool,

    /// Alias-based matching
    pub alias_matching: bool,
}

impl Default for EntityDeduplicationConfig {
    fn default() -> Self {
        Self {
            exact_text_matching: true,
            fuzzy_text_matching: true,
            fuzzy_threshold: 0.85,
            semantic_similarity_matching: false, // Requires embeddings
            semantic_threshold: 0.9,
            consider_entity_types: true,
            alias_matching: true,
        }
    }
}

/// Document processing result for knowledge graph construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConstructionResult {
    /// Document ID
    pub document_id: String,

    /// Number of entities processed
    pub entities_processed: usize,

    /// Number of entities created (new)
    pub entities_created: usize,

    /// Number of entities merged with existing
    pub entities_merged: usize,

    /// Number of relationships processed
    pub relationships_processed: usize,

    /// Number of relationships created (new)
    pub relationships_created: usize,

    /// Number of relationships deduplicated
    pub relationships_deduplicated: usize,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Any warnings or errors
    pub warnings: Vec<String>,

    /// Created node IDs
    pub created_node_ids: Vec<String>,

    /// Created relationship IDs
    pub created_relationship_ids: Vec<String>,
}

/// Document processing request for knowledge graph construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentGraphRequest {
    /// Document ID
    pub document_id: String,

    /// Extracted entities to add to graph
    pub entities: Vec<ExtractedEntity>,

    /// Extracted relationships to add to graph
    pub relationships: Vec<ExtractedRelationship>,

    /// Document metadata
    pub metadata: HashMap<String, serde_json::Value>,

    /// Override configuration for this request
    pub config_override: Option<KnowledgeGraphConfig>,
}

impl KnowledgeGraphBuilder {
    /// Create a new knowledge graph builder
    pub fn new(kg_name: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            kg_name,
            config: KnowledgeGraphConfig::default(),
            stats: KnowledgeGraphStats::default(),
            deduplication_config: EntityDeduplicationConfig::default(),
            entity_cache: HashMap::new(),
            relationship_cache: HashSet::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create knowledge graph builder with custom configuration
    pub fn with_config(
        kg_name: String,
        config: KnowledgeGraphConfig,
        dedup_config: EntityDeduplicationConfig,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            kg_name,
            config,
            stats: KnowledgeGraphStats::default(),
            deduplication_config: dedup_config,
            entity_cache: HashMap::new(),
            relationship_cache: HashSet::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Process a document and build knowledge graph
    pub async fn process_document(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        request: DocumentGraphRequest,
    ) -> OrbitResult<GraphConstructionResult> {
        let start_time = std::time::Instant::now();

        info!(
            kg_name = %self.kg_name,
            document_id = %request.document_id,
            entities_count = request.entities.len(),
            relationships_count = request.relationships.len(),
            "Processing document for knowledge graph construction"
        );

        // Use override config if provided
        let config = request.config_override.as_ref().unwrap_or(&self.config);

        // Filter entities and relationships by confidence
        let filtered_entities: Vec<_> = request
            .entities
            .into_iter()
            .filter(|e| e.confidence >= config.entity_confidence_threshold)
            .take(config.max_entities_per_document)
            .collect();

        let filtered_relationships: Vec<_> = request
            .relationships
            .into_iter()
            .filter(|r| r.confidence >= config.relationship_confidence_threshold)
            .take(config.max_relationships_per_document)
            .collect();

        debug!(
            filtered_entities = filtered_entities.len(),
            filtered_relationships = filtered_relationships.len(),
            "Filtered entities and relationships by confidence thresholds"
        );

        let mut result = GraphConstructionResult {
            document_id: request.document_id.clone(),
            entities_processed: filtered_entities.len(),
            entities_created: 0,
            entities_merged: 0,
            relationships_processed: filtered_relationships.len(),
            relationships_created: 0,
            relationships_deduplicated: 0,
            processing_time_ms: 0,
            warnings: Vec::new(),
            created_node_ids: Vec::new(),
            created_relationship_ids: Vec::new(),
        };

        // Process entities first
        for entity in filtered_entities {
            match self
                .process_entity(orbit_client.clone(), &entity, &request.metadata)
                .await
            {
                Ok(ProcessedEntity::Created(node_id)) => {
                    result.entities_created += 1;
                    result.created_node_ids.push(node_id);
                }
                Ok(ProcessedEntity::Merged(node_id)) => {
                    result.entities_merged += 1;
                    result.created_node_ids.push(node_id);
                }
                Err(e) => {
                    self.stats.error_count += 1;
                    result
                        .warnings
                        .push(format!("Failed to process entity '{}': {}", entity.text, e));
                    warn!(
                        entity_text = %entity.text,
                        error = %e,
                        "Failed to process entity"
                    );
                }
            }
        }

        // Process relationships
        for relationship in filtered_relationships {
            match self
                .process_relationship(orbit_client.clone(), &relationship)
                .await
            {
                Ok(ProcessedRelationship::Created(rel_id)) => {
                    result.relationships_created += 1;
                    result.created_relationship_ids.push(rel_id);
                }
                Ok(ProcessedRelationship::Deduplicated) => {
                    result.relationships_deduplicated += 1;
                }
                Err(e) => {
                    self.stats.error_count += 1;
                    result.warnings.push(format!(
                        "Failed to process relationship '{}->{}': {}",
                        relationship.from_entity, relationship.to_entity, e
                    ));
                    warn!(
                        from_entity = %relationship.from_entity,
                        to_entity = %relationship.to_entity,
                        rel_type = %relationship.relationship_type,
                        error = %e,
                        "Failed to process relationship"
                    );
                }
            }
        }

        let processing_time = start_time.elapsed();
        result.processing_time_ms = processing_time.as_millis() as u64;

        // Update statistics
        self.update_stats(&result);

        info!(
            document_id = %request.document_id,
            entities_created = result.entities_created,
            entities_merged = result.entities_merged,
            relationships_created = result.relationships_created,
            processing_time_ms = result.processing_time_ms,
            "Knowledge graph construction completed"
        );

        Ok(result)
    }

    /// Process a single entity and add to knowledge graph
    async fn process_entity(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        entity: &ExtractedEntity,
        document_metadata: &HashMap<String, serde_json::Value>,
    ) -> OrbitResult<ProcessedEntity> {
        // Check for existing entity (deduplication)
        if let Some(existing_node_id) = self.find_existing_entity(entity).await? {
            // Merge with existing entity
            self.merge_entity(orbit_client, &existing_node_id, entity)
                .await?;
            return Ok(ProcessedEntity::Merged(existing_node_id));
        }

        // Create new entity node
        let node_id = self
            .create_entity_node(orbit_client, entity, document_metadata)
            .await?;

        // Update entity cache
        self.entity_cache
            .insert(entity.text.clone(), node_id.clone());

        // Add aliases to cache
        for alias in &entity.aliases {
            self.entity_cache.insert(alias.clone(), node_id.clone());
        }

        Ok(ProcessedEntity::Created(node_id))
    }

    /// Process a single relationship and add to knowledge graph
    async fn process_relationship(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        relationship: &ExtractedRelationship,
    ) -> OrbitResult<ProcessedRelationship> {
        // Check for relationship deduplication
        let rel_key = format!(
            "{}->{}:{}",
            relationship.from_entity, relationship.to_entity, relationship.relationship_type
        );

        if self.relationship_cache.contains(&rel_key) {
            return Ok(ProcessedRelationship::Deduplicated);
        }

        // Find source and target nodes
        let source_node_id = self
            .find_or_create_entity_node(
                orbit_client.clone(),
                &relationship.from_entity,
                EntityType::Concept, // Default type for unknown entities
                &relationship.source_document,
            )
            .await?;

        let target_node_id = self
            .find_or_create_entity_node(
                orbit_client.clone(),
                &relationship.to_entity,
                EntityType::Concept,
                &relationship.source_document,
            )
            .await?;

        // Create relationship
        let rel_id = self
            .create_relationship(orbit_client, &source_node_id, &target_node_id, relationship)
            .await?;

        // Update relationship cache
        self.relationship_cache.insert(rel_key);

        Ok(ProcessedRelationship::Created(rel_id))
    }

    /// Find existing entity using deduplication strategies
    async fn find_existing_entity(&self, entity: &ExtractedEntity) -> OrbitResult<Option<String>> {
        // Check exact text match first
        if self.deduplication_config.exact_text_matching {
            if let Some(node_id) = self.entity_cache.get(&entity.text) {
                return Ok(Some(node_id.clone()));
            }
        }

        // Check alias matching
        if self.deduplication_config.alias_matching {
            for alias in &entity.aliases {
                if let Some(node_id) = self.entity_cache.get(alias) {
                    return Ok(Some(node_id.clone()));
                }
            }
        }

        // TODO: Implement fuzzy matching
        if self.deduplication_config.fuzzy_text_matching {
            // Would use string similarity algorithms like Levenshtein distance
        }

        // TODO: Implement semantic similarity matching
        if self.deduplication_config.semantic_similarity_matching {
            // Would use vector embeddings to find similar entities
        }

        Ok(None)
    }

    /// Create a new entity node in the graph database
    async fn create_entity_node(
        &self,
        orbit_client: Arc<OrbitClient>,
        entity: &ExtractedEntity,
        document_metadata: &HashMap<String, serde_json::Value>,
    ) -> OrbitResult<String> {
        // Create enhanced graph node
        let mut properties = entity.properties.clone();
        properties.insert("entity_text".to_string(), serde_json::json!(entity.text));
        properties.insert(
            "entity_type".to_string(),
            serde_json::json!(entity.entity_type.to_str()),
        );
        properties.insert(
            "confidence".to_string(),
            serde_json::json!(entity.confidence),
        );
        properties.insert(
            "source_document".to_string(),
            serde_json::json!(entity.source_document),
        );

        // Add document metadata
        for (key, value) in document_metadata {
            properties.insert(format!("doc_{}", key), value.clone());
        }

        // Generate unique node ID
        let node_id = format!("entity_{}", uuid::Uuid::new_v4());

        // Create Cypher query to create node
        let labels = vec![
            "Entity".to_string(),
            entity.entity_type.to_str().to_string(),
        ];

        let cypher_query = self.build_create_node_cypher(&node_id, &labels, &properties);

        debug!(
            node_id = %node_id,
            entity_text = %entity.text,
            entity_type = %entity.entity_type.to_str(),
            "Creating entity node in graph database"
        );

        // Execute through graph actor
        let graph_actor_ref = orbit_client
            .actor_reference::<crate::graph_database::GraphActor>(Key::StringKey {
                key: self.kg_name.clone(),
            })
            .await
            .map_err(|e| {
                OrbitError::internal(format!("Failed to get graph actor reference: {}", e))
            })?;

        graph_actor_ref
            .invoke::<serde_json::Value>("execute_query", vec![serde_json::json!(cypher_query)])
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create entity node: {}", e)))?;

        Ok(node_id)
    }

    /// Create a relationship between two nodes
    async fn create_relationship(
        &self,
        orbit_client: Arc<OrbitClient>,
        source_node_id: &str,
        target_node_id: &str,
        relationship: &ExtractedRelationship,
    ) -> OrbitResult<String> {
        let rel_id = format!("rel_{}", uuid::Uuid::new_v4());

        let mut properties = relationship.properties.clone();
        properties.insert(
            "confidence".to_string(),
            serde_json::json!(relationship.confidence),
        );
        properties.insert(
            "source_text".to_string(),
            serde_json::json!(relationship.source_text),
        );
        properties.insert(
            "source_document".to_string(),
            serde_json::json!(relationship.source_document),
        );

        let cypher_query = format!(
            "MATCH (a) WHERE a.id = '{}' MATCH (b) WHERE b.id = '{}' CREATE (a)-[r:{} {{{}}}]->(b) RETURN r",
            source_node_id,
            target_node_id,
            relationship.relationship_type,
            self.serialize_properties(&properties)
        );

        debug!(
            rel_id = %rel_id,
            source_node = %source_node_id,
            target_node = %target_node_id,
            rel_type = %relationship.relationship_type,
            "Creating relationship in graph database"
        );

        let graph_actor_ref = orbit_client
            .actor_reference::<crate::graph_database::GraphActor>(Key::StringKey {
                key: self.kg_name.clone(),
            })
            .await
            .map_err(|e| {
                OrbitError::internal(format!("Failed to get graph actor reference: {}", e))
            })?;

        graph_actor_ref
            .invoke::<serde_json::Value>("execute_query", vec![serde_json::json!(cypher_query)])
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create relationship: {}", e)))?;

        Ok(rel_id)
    }

    /// Find existing entity node or create a new one
    async fn find_or_create_entity_node(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        entity_text: &str,
        entity_type: EntityType,
        source_document: &str,
    ) -> OrbitResult<String> {
        // Check cache first
        if let Some(node_id) = self.entity_cache.get(entity_text) {
            return Ok(node_id.clone());
        }

        // Create minimal extracted entity
        let extracted_entity = ExtractedEntity {
            text: entity_text.to_string(),
            entity_type,
            confidence: 0.5, // Lower confidence for inferred entities
            start_pos: 0,
            end_pos: entity_text.len(),
            properties: HashMap::new(),
            aliases: Vec::new(),
            source_document: source_document.to_string(),
        };

        // Create the entity node
        let node_id = self
            .create_entity_node(orbit_client, &extracted_entity, &HashMap::new())
            .await?;

        // Update cache
        self.entity_cache
            .insert(entity_text.to_string(), node_id.clone());

        Ok(node_id)
    }

    /// Merge entity data with existing node
    async fn merge_entity(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        existing_node_id: &str,
        entity: &ExtractedEntity,
    ) -> OrbitResult<()> {
        // TODO: Implement entity merging logic
        // This would update the existing node with new information
        // For now, we'll just add the source document

        let cypher_query = format!(
            "MATCH (n) WHERE n.id = '{}' SET n.source_documents = COALESCE(n.source_documents, []) + ['{}']",
            existing_node_id,
            entity.source_document
        );

        let graph_actor_ref = orbit_client
            .actor_reference::<crate::graph_database::GraphActor>(Key::StringKey {
                key: self.kg_name.clone(),
            })
            .await
            .map_err(|e| {
                OrbitError::internal(format!("Failed to get graph actor reference: {}", e))
            })?;

        graph_actor_ref
            .invoke::<serde_json::Value>("execute_query", vec![serde_json::json!(cypher_query)])
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to merge entity: {}", e)))?;

        Ok(())
    }

    /// Build Cypher query for creating a node
    fn build_create_node_cypher(
        &self,
        node_id: &str,
        labels: &[String],
        properties: &HashMap<String, serde_json::Value>,
    ) -> String {
        let labels_str = labels.join(":");
        let mut props = properties.clone();
        props.insert("id".to_string(), serde_json::json!(node_id));

        format!(
            "CREATE (n:{} {{{}}}) RETURN n",
            labels_str,
            self.serialize_properties(&props)
        )
    }

    /// Serialize properties for Cypher query
    fn serialize_properties(&self, properties: &HashMap<String, serde_json::Value>) -> String {
        properties
            .iter()
            .map(|(key, value)| {
                let value_str = match value {
                    serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "\\'")),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => format!("'{}'", value.to_string().replace('\'', "\\'")),
                };
                format!("{}: {}", key, value_str)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Update statistics after processing
    fn update_stats(&mut self, result: &GraphConstructionResult) {
        self.stats.documents_processed += 1;
        self.stats.entities_created += result.entities_created as u64;
        self.stats.relationships_created += result.relationships_created as u64;
        self.stats.entities_merged += result.entities_merged as u64;
        self.stats.relationships_deduplicated += result.relationships_deduplicated as u64;

        // Update average processing time
        let total_time = (self.stats.avg_processing_time_ms
            * (self.stats.documents_processed - 1) as f64)
            + result.processing_time_ms as f64;
        self.stats.avg_processing_time_ms = total_time / self.stats.documents_processed as f64;

        self.stats.current_node_count += result.entities_created as u64;
        self.stats.current_relationship_count += result.relationships_created as u64;

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
        self.updated_at = self.stats.last_updated;
    }

    /// Get knowledge graph statistics
    pub fn get_stats(&self) -> &KnowledgeGraphStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = KnowledgeGraphStats::default();
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Clear entity and relationship caches
    pub fn clear_caches(&mut self) {
        self.entity_cache.clear();
        self.relationship_cache.clear();
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update configuration
    pub fn update_config(&mut self, config: KnowledgeGraphConfig) {
        self.config = config;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update deduplication configuration
    pub fn update_deduplication_config(&mut self, config: EntityDeduplicationConfig) {
        self.deduplication_config = config;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

impl Default for KnowledgeGraphBuilder {
    fn default() -> Self {
        Self::new("default".to_string())
    }
}

impl Addressable for KnowledgeGraphBuilder {
    fn addressable_type() -> &'static str {
        "KnowledgeGraphBuilder"
    }
}

/// Result of entity processing
#[derive(Debug)]
enum ProcessedEntity {
    /// New entity was created
    Created(String),
    /// Entity was merged with existing
    Merged(String),
}

/// Result of relationship processing
#[derive(Debug)]
enum ProcessedRelationship {
    /// New relationship was created
    Created(String),
    /// Relationship was deduplicated (already exists)
    Deduplicated,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_knowledge_graph_builder_creation() {
        let builder = KnowledgeGraphBuilder::new("test_kg".to_string());
        assert_eq!(builder.kg_name, "test_kg");
        assert_eq!(builder.stats.documents_processed, 0);
        assert!(builder.entity_cache.is_empty());
    }

    #[test]
    fn test_entity_deduplication_config() {
        let config = EntityDeduplicationConfig::default();
        assert!(config.exact_text_matching);
        assert!(config.fuzzy_text_matching);
        assert_eq!(config.fuzzy_threshold, 0.85);
    }

    #[test]
    fn test_cypher_query_building() {
        let builder = KnowledgeGraphBuilder::new("test".to_string());
        let labels = vec!["Entity".to_string(), "Person".to_string()];
        let mut properties = HashMap::new();
        properties.insert("name".to_string(), serde_json::json!("John Smith"));
        properties.insert("confidence".to_string(), serde_json::json!(0.95));

        let query = builder.build_create_node_cypher("node_123", &labels, &properties);

        assert!(query.contains("CREATE (n:Entity:Person"));
        assert!(query.contains("name: 'John Smith'"));
        assert!(query.contains("confidence: 0.95"));
        assert!(query.contains("id: 'node_123'"));
    }

    #[test]
    fn test_property_serialization() {
        let builder = KnowledgeGraphBuilder::new("test".to_string());
        let mut properties = HashMap::new();
        properties.insert("text".to_string(), serde_json::json!("Hello World"));
        properties.insert("score".to_string(), serde_json::json!(0.85));
        properties.insert("active".to_string(), serde_json::json!(true));

        let serialized = builder.serialize_properties(&properties);

        assert!(serialized.contains("text: 'Hello World'"));
        assert!(serialized.contains("score: 0.85"));
        assert!(serialized.contains("active: true"));
    }
}
