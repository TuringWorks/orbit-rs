//! GraphRAG shared types and utilities
//!
//! This module provides shared data structures and utilities for GraphRAG
//! (Graph-enhanced Retrieval-Augmented Generation) functionality in Orbit-RS.

use crate::graph::{GraphNode, GraphRelationship};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enhanced graph node with vector embeddings and GraphRAG metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedGraphNode {
    /// Base graph node
    #[serde(flatten)]
    pub base: GraphNode,

    /// Multiple embeddings per model (model_name -> embedding)
    pub embeddings: HashMap<String, Vec<f32>>,

    /// Entity type classification
    pub entity_type: Option<EntityType>,

    /// Confidence score for entity extraction (0.0 to 1.0)
    pub confidence_score: f32,

    /// Source documents that contributed to this entity
    pub source_documents: Vec<String>,

    /// Last time this node was updated
    pub last_updated: i64,

    /// Semantic importance score based on graph structure
    pub importance_score: f32,

    /// Frequency of this entity across documents
    pub document_frequency: u32,

    /// Alternative names/aliases for this entity
    pub aliases: Vec<String>,
}

impl EnhancedGraphNode {
    /// Create a new enhanced graph node
    pub fn new(base: GraphNode, entity_type: Option<EntityType>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            base,
            embeddings: HashMap::new(),
            entity_type,
            confidence_score: 1.0,
            source_documents: Vec::new(),
            last_updated: now,
            importance_score: 0.0,
            document_frequency: 0,
            aliases: Vec::new(),
        }
    }

    /// Add an embedding for a specific model
    pub fn add_embedding(&mut self, model_name: String, embedding: Vec<f32>) {
        self.embeddings.insert(model_name, embedding);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Get embedding for a specific model
    pub fn get_embedding(&self, model_name: &str) -> Option<&Vec<f32>> {
        self.embeddings.get(model_name)
    }

    /// Add a source document
    pub fn add_source_document(&mut self, document_id: String) {
        if !self.source_documents.contains(&document_id) {
            self.source_documents.push(document_id);
            self.document_frequency += 1;
            self.last_updated = chrono::Utc::now().timestamp_millis();
        }
    }

    /// Add an alias
    pub fn add_alias(&mut self, alias: String) {
        if !self.aliases.contains(&alias) {
            self.aliases.push(alias);
            self.last_updated = chrono::Utc::now().timestamp_millis();
        }
    }

    /// Update confidence score
    pub fn update_confidence(&mut self, new_confidence: f32) {
        self.confidence_score = new_confidence.clamp(0.0, 1.0);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Update importance score
    pub fn update_importance(&mut self, new_importance: f32) {
        self.importance_score = new_importance.clamp(0.0, 1.0);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Check if node has embedding for model
    pub fn has_embedding(&self, model_name: &str) -> bool {
        self.embeddings.contains_key(model_name)
    }

    /// Get all available embedding models
    pub fn available_models(&self) -> Vec<&String> {
        self.embeddings.keys().collect()
    }
}

/// Enhanced graph relationship with vector embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedGraphRelationship {
    /// Base graph relationship
    #[serde(flatten)]
    pub base: GraphRelationship,

    /// Embedding of the relationship text/context
    pub embeddings: HashMap<String, Vec<f32>>,

    /// Confidence score for relationship extraction (0.0 to 1.0)
    pub confidence_score: f32,

    /// Source documents that mention this relationship
    pub source_documents: Vec<String>,

    /// Last time this relationship was updated
    pub last_updated: i64,

    /// Strength/weight of this relationship
    pub strength: f32,

    /// Directional relationship (for weight calculation)
    pub is_bidirectional: bool,
}

impl EnhancedGraphRelationship {
    /// Create a new enhanced graph relationship
    pub fn new(base: GraphRelationship) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            base,
            embeddings: HashMap::new(),
            confidence_score: 1.0,
            source_documents: Vec::new(),
            last_updated: now,
            strength: 1.0,
            is_bidirectional: false,
        }
    }

    /// Add an embedding for a specific model
    pub fn add_embedding(&mut self, model_name: String, embedding: Vec<f32>) {
        self.embeddings.insert(model_name, embedding);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Update relationship strength
    pub fn update_strength(&mut self, new_strength: f32) {
        self.strength = new_strength.clamp(0.0, 1.0);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }
}

/// Entity type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// Person names, people
    Person,

    /// Companies, organizations, institutions
    Organization,

    /// Geographic locations, places
    Location,

    /// Events, meetings, occurrences
    Event,

    /// Abstract concepts, ideas
    Concept,

    /// Documents, files, publications
    Document,

    /// Products, services, items
    Product,

    /// Dates, times, temporal references
    Temporal,

    /// Numbers, quantities, measurements
    Quantity,

    /// Custom entity type
    Custom(String),
}

impl std::str::FromStr for EntityType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "person" | "people" | "human" => EntityType::Person,
            "organization" | "org" | "company" => EntityType::Organization,
            "location" | "place" | "geo" => EntityType::Location,
            "event" | "meeting" | "occurrence" => EntityType::Event,
            "concept" | "idea" | "abstract" => EntityType::Concept,
            "document" | "file" | "publication" => EntityType::Document,
            "product" | "service" | "item" => EntityType::Product,
            "temporal" | "time" | "date" => EntityType::Temporal,
            "quantity" | "number" | "measurement" => EntityType::Quantity,
            _ => EntityType::Custom(s.to_string()),
        })
    }
}

impl EntityType {
    /// Convert to string representation
    pub fn to_str(&self) -> &str {
        match self {
            EntityType::Person => "person",
            EntityType::Organization => "organization",
            EntityType::Location => "location",
            EntityType::Event => "event",
            EntityType::Concept => "concept",
            EntityType::Document => "document",
            EntityType::Product => "product",
            EntityType::Temporal => "temporal",
            EntityType::Quantity => "quantity",
            EntityType::Custom(s) => s,
        }
    }
}

/// Extracted entity from text processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    /// Entity text/name
    pub text: String,

    /// Entity type
    pub entity_type: EntityType,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Start position in source text
    pub start_pos: usize,

    /// End position in source text
    pub end_pos: usize,

    /// Additional properties/metadata
    pub properties: HashMap<String, serde_json::Value>,

    /// Potential aliases/variations
    pub aliases: Vec<String>,

    /// Source document identifier
    pub source_document: String,
}

/// Extracted relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    /// Source entity
    pub from_entity: String,

    /// Target entity
    pub to_entity: String,

    /// Relationship type/label
    pub relationship_type: String,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Source text that describes this relationship
    pub source_text: String,

    /// Additional properties/metadata
    pub properties: HashMap<String, serde_json::Value>,

    /// Source document identifier
    pub source_document: String,
}

/// Multi-hop reasoning path between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningPath {
    /// Path nodes in order
    pub nodes: Vec<String>,

    /// Relationships connecting the nodes
    pub relationships: Vec<String>,

    /// Path score/confidence
    pub score: f32,

    /// Path length (number of hops)
    pub length: usize,

    /// Explanation of the path
    pub explanation: String,
}

/// Connection explanation between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionExplanation {
    /// Source entity
    pub from_entity: String,

    /// Target entity  
    pub to_entity: String,

    /// All discovered paths
    pub paths: Vec<ReasoningPath>,

    /// Best/strongest connection path
    pub best_path: Option<ReasoningPath>,

    /// Overall connection strength
    pub connection_strength: f32,

    /// Human-readable explanation
    pub explanation: String,
}

/// Context item for RAG queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    /// Content text
    pub content: String,

    /// Source type (graph, vector, text)
    pub source_type: ContextSourceType,

    /// Relevance score
    pub relevance_score: f32,

    /// Source entity/document ID
    pub source_id: String,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Type of context source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextSourceType {
    /// From graph traversal
    Graph,

    /// From vector similarity search
    Vector,

    /// From text search
    Text,

    /// Hybrid/combined source
    Hybrid,
}

/// RAG response with context and citations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGResponse {
    /// Generated response text
    pub response: String,

    /// Context items used for generation
    pub context: Vec<ContextItem>,

    /// Confidence in the response
    pub confidence: f32,

    /// Citations/sources
    pub citations: Vec<String>,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Search strategy for hybrid search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchStrategy {
    /// Graph traversal first, then vector/text
    GraphFirst,

    /// Vector similarity first, then graph expansion
    VectorFirst,

    /// Text search first, then graph/vector
    TextFirst,

    /// Balanced parallel search across all sources
    Balanced,

    /// Adaptive strategy based on query type
    Adaptive,
}

/// Embedding model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    /// Model name/identifier
    pub name: String,

    /// Model type
    pub model_type: EmbeddingModelType,

    /// Embedding dimension
    pub dimension: usize,

    /// Model configuration
    pub config: HashMap<String, serde_json::Value>,
}

/// Types of embedding models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbeddingModelType {
    /// Sentence transformer models
    SentenceTransformer { model_path: String },

    /// OpenAI embedding models
    OpenAI { model_name: String, api_key: String },

    /// Local embedding API
    Local { endpoint: String },

    /// Ollama embedding models
    Ollama { model_name: String },

    /// Custom embedding function
    Custom { function_name: String },
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMProvider {
    /// OpenAI GPT models
    OpenAI {
        api_key: String,
        model: String,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    },

    /// Anthropic Claude models
    Anthropic {
        api_key: String,
        model: String,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    },

    /// Local LLM API
    Local {
        endpoint: String,
        model: String,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    },

    /// Ollama local models
    Ollama {
        model: String,
        temperature: Option<f32>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphNode;
    use std::collections::HashMap;

    #[test]
    fn test_enhanced_graph_node_creation() {
        let mut properties = HashMap::new();
        properties.insert("name".to_string(), serde_json::json!("Alice"));

        let now = chrono::Utc::now();
        let base_node = GraphNode {
            id: crate::graph::NodeId::new("node_1".to_string()),
            labels: vec!["Person".to_string()],
            properties,
            created_at: now,
            updated_at: now,
        };

        let enhanced_node = EnhancedGraphNode::new(base_node, Some(EntityType::Person));

        assert_eq!(enhanced_node.base.id.as_str(), "node_1");
        assert_eq!(enhanced_node.entity_type, Some(EntityType::Person));
        assert_eq!(enhanced_node.confidence_score, 1.0);
        assert!(enhanced_node.embeddings.is_empty());
    }

    #[test]
    fn test_entity_type_parsing() {
        use std::str::FromStr;

        assert_eq!(EntityType::from_str("person").unwrap(), EntityType::Person);
        assert_eq!(
            EntityType::from_str("Organization").unwrap(),
            EntityType::Organization
        );
        assert_eq!(
            EntityType::from_str("LOCATION").unwrap(),
            EntityType::Location
        );

        if let EntityType::Custom(name) = EntityType::from_str("custom_type").unwrap() {
            assert_eq!(name, "custom_type");
        } else {
            panic!("Expected Custom entity type");
        }
    }

    #[test]
    fn test_enhanced_node_embeddings() {
        let now = chrono::Utc::now();
        let base_node = GraphNode {
            id: crate::graph::NodeId::new("test".to_string()),
            labels: vec!["Test".to_string()],
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        };

        let mut enhanced_node = EnhancedGraphNode::new(base_node, None);

        // Add embedding
        enhanced_node.add_embedding("model1".to_string(), vec![1.0, 2.0, 3.0]);

        assert!(enhanced_node.has_embedding("model1"));
        assert!(!enhanced_node.has_embedding("model2"));

        let embedding = enhanced_node.get_embedding("model1").unwrap();
        assert_eq!(embedding, &vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_extracted_entity() {
        let entity = ExtractedEntity {
            text: "John Smith".to_string(),
            entity_type: EntityType::Person,
            confidence: 0.95,
            start_pos: 0,
            end_pos: 10,
            properties: HashMap::new(),
            aliases: vec!["John".to_string(), "J. Smith".to_string()],
            source_document: "doc1".to_string(),
        };

        assert_eq!(entity.text, "John Smith");
        assert_eq!(entity.entity_type, EntityType::Person);
        assert_eq!(entity.confidence, 0.95);
        assert_eq!(entity.aliases.len(), 2);
    }
}
