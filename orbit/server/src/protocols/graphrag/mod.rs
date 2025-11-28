//! GraphRAG (Graph-enhanced Retrieval-Augmented Generation) module
//!
//! This module provides comprehensive GraphRAG capabilities for Orbit-RS,
//! combining graph databases, vector stores, and large language models
//! for advanced knowledge extraction and retrieval-augmented generation.

pub mod entity_extraction;
pub mod graph_rag_actor;
pub mod knowledge_graph;
pub mod llm_client;
pub mod multi_hop_reasoning;
pub mod rag_pipeline;
#[cfg(feature = "storage-rocksdb")]
pub mod storage;

// Re-export main types and actors
pub use entity_extraction::{
    DeduplicationStrategy, DocumentProcessingRequest, DocumentProcessingResult,
    EntityExtractionActor, ExtractorConfig, RelationshipRule,
};

pub use graph_rag_actor::{GraphRAGActor, GraphRAGConfig, GraphRAGStats};

pub use knowledge_graph::{KnowledgeGraphBuilder, KnowledgeGraphStats};
#[cfg(feature = "storage-rocksdb")]
pub use storage::{
    GraphRAGMetadata, GraphRAGNode, GraphRAGRelationship, GraphRAGStorage, RelationshipDirection,
};

pub use multi_hop_reasoning::{
    MultiHopReasoningEngine, PathScoringStrategy, PruningStrategy, ReasoningQuery,
};

// Re-export from orbit-shared for convenience
pub use orbit_shared::graphrag::{ConnectionExplanation, ReasoningPath};

pub use rag_pipeline::{
    ContextFusion, HybridSearchConfig, RAGPipeline, RAGPipelineConfig, RAGQuery, RAGQueryResult,
};

/// GraphRAG protocol version
pub const GRAPHRAG_VERSION: &str = "1.0.0";

/// Default configuration values
pub mod defaults {
    pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.7;
    pub const DEFAULT_MAX_ENTITIES_PER_DOCUMENT: usize = 100;
    pub const DEFAULT_MAX_RELATIONSHIPS_PER_DOCUMENT: usize = 200;
    pub const DEFAULT_EMBEDDING_DIMENSION: usize = 384;
    pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.8;
    pub const DEFAULT_MAX_HOPS: u32 = 3;
    pub const DEFAULT_RAG_CONTEXT_SIZE: usize = 2048;
    pub const DEFAULT_MAX_QUERY_RESULTS: usize = 50;
}

/// Error types specific to GraphRAG operations
#[derive(Debug, thiserror::Error)]
pub enum GraphRAGError {
    #[error("Entity extraction failed: {0}")]
    EntityExtractionError(String),

    #[error("Knowledge graph construction failed: {0}")]
    KnowledgeGraphError(String),

    #[error("Multi-hop reasoning failed: {0}")]
    ReasoningError(String),

    #[error("RAG pipeline error: {0}")]
    RAGPipelineError(String),

    #[error("Embedding generation failed: {0}")]
    EmbeddingError(String),

    #[error("LLM provider error: {0}")]
    LLMError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}
