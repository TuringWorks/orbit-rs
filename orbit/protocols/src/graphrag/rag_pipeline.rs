//! RAG Pipeline for GraphRAG
//!
//! This module provides the RAG (Retrieval-Augmented Generation) pipeline
//! that combines graph traversal, vector search, and LLM generation.

use orbit_shared::graphrag::{ContextItem, SearchStrategy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RAG pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGPipelineConfig {
    /// Search strategy to use
    pub search_strategy: SearchStrategy,

    /// Maximum context size
    pub max_context_size: usize,

    /// LLM provider configuration
    pub llm_provider: String,

    /// Vector similarity threshold
    pub similarity_threshold: f32,
}

impl Default for RAGPipelineConfig {
    fn default() -> Self {
        Self {
            search_strategy: SearchStrategy::Balanced,
            max_context_size: 2048,
            llm_provider: "default".to_string(),
            similarity_threshold: 0.7,
        }
    }
}

/// RAG pipeline actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGPipeline {
    /// Pipeline configuration
    pub config: RAGPipelineConfig,

    /// Creation timestamp
    pub created_at: i64,
}

impl RAGPipeline {
    /// Create new RAG pipeline
    pub fn new(config: RAGPipelineConfig) -> Self {
        Self {
            config,
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// RAG query structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGQuery {
    /// Query text
    pub query_text: String,

    /// Maximum results
    pub max_results: Option<usize>,

    /// Additional filters
    pub filters: HashMap<String, serde_json::Value>,
}

/// RAG query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGQueryResult {
    /// Query text
    pub query_text: String,

    /// Retrieved context items
    pub context: Vec<ContextItem>,

    /// Generated response
    pub response: String,

    /// Confidence score
    pub confidence: f32,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Hybrid search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    /// Graph weight
    pub graph_weight: f32,

    /// Vector weight  
    pub vector_weight: f32,

    /// Text weight
    pub text_weight: f32,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            graph_weight: 0.4,
            vector_weight: 0.4,
            text_weight: 0.2,
        }
    }
}

/// Context fusion strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ContextFusion {
    /// Simple concatenation
    Concatenate,

    /// Ranked by relevance
    #[default]
    RankedFusion,

    /// Weighted combination
    WeightedFusion { weights: HashMap<String, f32> },
}
