//! GraphRAG Integration Tests
//!
//! Comprehensive tests for the GraphRAG module including:
//! - RAGPipeline configuration and creation
//! - MultiHopReasoningEngine with various strategies
//! - EntityExtractionActor with document processing
//! - KnowledgeGraphBuilder operations
//! - GraphRAGActor orchestration

#![cfg(feature = "rest")]

use orbit_protocols::graphrag::{
    entity_extraction::{
        DeduplicationStrategy, DocumentProcessingRequest, EntityExtractionActor, ExtractorConfig,
        RelationshipRule,
    },
    graph_rag_actor::{
        GraphRAGActor, GraphRAGConfig, GraphRAGDocumentRequest, GraphRAGQuery, GraphRAGStats,
        ProcessingTimes,
    },
    knowledge_graph::{
        EntityDeduplicationConfig, KnowledgeGraphBuilder, KnowledgeGraphConfig, KnowledgeGraphStats,
    },
    multi_hop_reasoning::{
        MultiHopReasoningEngine, PathScoringStrategy, PruningStrategy, ReasoningConfig,
        ReasoningQuery, ReasoningStats,
    },
    rag_pipeline::{ContextFusion, HybridSearchConfig, RAGPipeline, RAGPipelineConfig, RAGQuery},
};
use orbit_shared::graphrag::{EntityType, SearchStrategy};
use std::collections::HashMap;

// =============================================================================
// RAGPipeline Tests
// =============================================================================

mod rag_pipeline_tests {
    use super::*;

    #[test]
    fn test_rag_pipeline_config_default() {
        let config = RAGPipelineConfig::default();
        assert!(matches!(config.search_strategy, SearchStrategy::Balanced));
        assert_eq!(config.max_context_size, 2048);
        assert_eq!(config.llm_provider, "default");
        assert_eq!(config.similarity_threshold, 0.7);
    }

    #[test]
    fn test_rag_pipeline_config_custom() {
        let config = RAGPipelineConfig {
            search_strategy: SearchStrategy::GraphFirst,
            max_context_size: 4096,
            llm_provider: "openai".to_string(),
            similarity_threshold: 0.85,
        };
        assert!(matches!(config.search_strategy, SearchStrategy::GraphFirst));
        assert_eq!(config.max_context_size, 4096);
        assert_eq!(config.llm_provider, "openai");
        assert_eq!(config.similarity_threshold, 0.85);
    }

    #[test]
    fn test_rag_pipeline_creation() {
        let config = RAGPipelineConfig::default();
        let pipeline = RAGPipeline::new(config.clone());
        assert_eq!(pipeline.config.max_context_size, config.max_context_size);
        assert!(pipeline.created_at > 0);
    }

    #[test]
    fn test_rag_query_structure() {
        let query = RAGQuery {
            query_text: "What is the relationship between entities?".to_string(),
            max_results: Some(10),
            filters: {
                let mut filters = HashMap::new();
                filters.insert("type".to_string(), serde_json::json!("Person"));
                filters
            },
        };
        assert_eq!(
            query.query_text,
            "What is the relationship between entities?"
        );
        assert_eq!(query.max_results, Some(10));
        assert!(query.filters.contains_key("type"));
    }

    #[test]
    fn test_hybrid_search_config_default() {
        let config = HybridSearchConfig::default();
        assert_eq!(config.graph_weight, 0.4);
        assert_eq!(config.vector_weight, 0.4);
        assert_eq!(config.text_weight, 0.2);
        // Weights should sum to 1.0
        let total = config.graph_weight + config.vector_weight + config.text_weight;
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_hybrid_search_config_custom_weights() {
        let config = HybridSearchConfig {
            graph_weight: 0.6,
            vector_weight: 0.3,
            text_weight: 0.1,
        };
        assert_eq!(config.graph_weight, 0.6);
        assert_eq!(config.vector_weight, 0.3);
        assert_eq!(config.text_weight, 0.1);
    }

    #[test]
    fn test_context_fusion_variants() {
        let concatenate = ContextFusion::Concatenate;
        assert!(matches!(concatenate, ContextFusion::Concatenate));

        let ranked = ContextFusion::RankedFusion;
        assert!(matches!(ranked, ContextFusion::RankedFusion));

        let weighted = ContextFusion::WeightedFusion {
            weights: {
                let mut weights = HashMap::new();
                weights.insert("graph".to_string(), 0.5);
                weights.insert("vector".to_string(), 0.5);
                weights
            },
        };
        if let ContextFusion::WeightedFusion { weights } = weighted {
            assert_eq!(weights.len(), 2);
            assert_eq!(weights.get("graph"), Some(&0.5));
        } else {
            panic!("Expected WeightedFusion");
        }
    }

    #[test]
    fn test_context_fusion_default() {
        let default = ContextFusion::default();
        assert!(matches!(default, ContextFusion::RankedFusion));
    }

    #[test]
    fn test_rag_pipeline_config_serialization() {
        let config = RAGPipelineConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RAGPipelineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_context_size, deserialized.max_context_size);
        assert_eq!(config.similarity_threshold, deserialized.similarity_threshold);
    }
}

// =============================================================================
// MultiHopReasoningEngine Tests
// =============================================================================

mod multi_hop_reasoning_tests {
    use super::*;

    #[test]
    fn test_reasoning_engine_creation() {
        let engine = MultiHopReasoningEngine::new(5);
        assert_eq!(engine.max_hops, 5);
        assert_eq!(engine.stats.queries_executed, 0);
        assert_eq!(engine.stats.paths_explored, 0);
        assert!(engine.created_at > 0);
    }

    #[test]
    fn test_reasoning_engine_default() {
        let engine = MultiHopReasoningEngine::default();
        assert_eq!(engine.max_hops, 3); // Default is 3 hops
    }

    #[test]
    fn test_reasoning_config_default() {
        let config = ReasoningConfig::default();
        assert_eq!(config.max_paths_per_hop, 100);
        assert_eq!(config.max_results, 50);
        assert_eq!(config.min_path_score, 0.1);
        assert!(config.allowed_relationship_types.is_empty());
        assert!(config.excluded_relationship_types.is_empty());
        assert!(config.bidirectional_search);
        assert_eq!(config.query_timeout_ms, 30_000);
        assert!(config.enable_caching);
    }

    #[test]
    fn test_reasoning_config_custom() {
        let config = ReasoningConfig {
            max_paths_per_hop: 50,
            max_results: 20,
            min_path_score: 0.3,
            allowed_relationship_types: vec!["KNOWS".to_string(), "WORKS_AT".to_string()],
            excluded_relationship_types: vec!["BLOCKED".to_string()],
            bidirectional_search: false,
            query_timeout_ms: 10_000,
            enable_caching: false,
        };
        assert_eq!(config.max_paths_per_hop, 50);
        assert_eq!(config.allowed_relationship_types.len(), 2);
        assert!(!config.bidirectional_search);
    }

    #[test]
    fn test_path_scoring_strategy_default() {
        let default = PathScoringStrategy::default();
        if let PathScoringStrategy::Combined {
            confidence_weight,
            length_weight,
            importance_weight,
        } = default
        {
            assert_eq!(confidence_weight, 0.4);
            assert_eq!(length_weight, 0.3);
            assert_eq!(importance_weight, 0.3);
        } else {
            panic!("Expected Combined strategy as default");
        }
    }

    #[test]
    fn test_path_scoring_strategy_variants() {
        let confidence = PathScoringStrategy::ConfidenceBased;
        assert!(matches!(confidence, PathScoringStrategy::ConfidenceBased));

        let length = PathScoringStrategy::LengthBased;
        assert!(matches!(length, PathScoringStrategy::LengthBased));

        let importance = PathScoringStrategy::ImportanceBased;
        assert!(matches!(importance, PathScoringStrategy::ImportanceBased));

        let custom = PathScoringStrategy::Custom {
            function_name: "my_scorer".to_string(),
        };
        if let PathScoringStrategy::Custom { function_name } = custom {
            assert_eq!(function_name, "my_scorer");
        } else {
            panic!("Expected Custom strategy");
        }
    }

    #[test]
    fn test_pruning_strategy_default() {
        let default = PruningStrategy::default();
        if let PruningStrategy::Combined {
            score_threshold,
            max_branches,
            avoid_cycles,
        } = default
        {
            assert_eq!(score_threshold, 0.2);
            assert_eq!(max_branches, 50);
            assert!(avoid_cycles);
        } else {
            panic!("Expected Combined strategy as default");
        }
    }

    #[test]
    fn test_pruning_strategy_variants() {
        let none = PruningStrategy::None;
        assert!(matches!(none, PruningStrategy::None));

        let score_based = PruningStrategy::ScoreBased { threshold: 0.5 };
        if let PruningStrategy::ScoreBased { threshold } = score_based {
            assert_eq!(threshold, 0.5);
        }

        let branching = PruningStrategy::BranchingFactor { max_branches: 25 };
        if let PruningStrategy::BranchingFactor { max_branches } = branching {
            assert_eq!(max_branches, 25);
        }

        let avoid_cycles = PruningStrategy::AvoidCycles;
        assert!(matches!(avoid_cycles, PruningStrategy::AvoidCycles));
    }

    #[test]
    fn test_reasoning_stats_default() {
        let stats = ReasoningStats::default();
        assert_eq!(stats.queries_executed, 0);
        assert_eq!(stats.paths_explored, 0);
        assert_eq!(stats.paths_found, 0);
        assert_eq!(stats.avg_query_time_ms, 0.0);
        assert_eq!(stats.cache_hit_ratio, 0.0);
    }

    #[test]
    fn test_reasoning_query_structure() {
        let query = ReasoningQuery {
            from_entity: "Alice".to_string(),
            to_entity: "Bob".to_string(),
            max_hops: Some(3),
            relationship_types: Some(vec!["KNOWS".to_string(), "WORKS_WITH".to_string()]),
            include_explanation: true,
            max_results: Some(10),
        };
        assert_eq!(query.from_entity, "Alice");
        assert_eq!(query.to_entity, "Bob");
        assert_eq!(query.max_hops, Some(3));
        assert!(query.include_explanation);
    }

    #[test]
    fn test_reasoning_engine_with_config() {
        let scoring = PathScoringStrategy::ConfidenceBased;
        let pruning = PruningStrategy::AvoidCycles;
        let config = ReasoningConfig {
            max_paths_per_hop: 200,
            ..ReasoningConfig::default()
        };

        let engine = MultiHopReasoningEngine::with_config(4, scoring, pruning, config);
        assert_eq!(engine.max_hops, 4);
        assert!(matches!(
            engine.path_scoring,
            PathScoringStrategy::ConfidenceBased
        ));
        assert!(matches!(engine.pruning_strategy, PruningStrategy::AvoidCycles));
        assert_eq!(engine.config.max_paths_per_hop, 200);
    }

    #[test]
    fn test_reasoning_engine_update_config() {
        let mut engine = MultiHopReasoningEngine::new(3);
        let original_updated_at = engine.updated_at;

        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_config = ReasoningConfig {
            max_results: 100,
            ..ReasoningConfig::default()
        };
        engine.update_config(new_config);

        assert_eq!(engine.config.max_results, 100);
        assert!(engine.updated_at >= original_updated_at);
    }

    #[test]
    fn test_reasoning_engine_update_strategies() {
        let mut engine = MultiHopReasoningEngine::new(3);

        engine.update_scoring_strategy(PathScoringStrategy::LengthBased);
        assert!(matches!(
            engine.path_scoring,
            PathScoringStrategy::LengthBased
        ));

        engine.update_pruning_strategy(PruningStrategy::None);
        assert!(matches!(engine.pruning_strategy, PruningStrategy::None));
    }

    #[test]
    fn test_reasoning_engine_reset_stats() {
        let mut engine = MultiHopReasoningEngine::new(3);

        // Manually set some stats
        engine.stats.queries_executed = 10;
        engine.stats.paths_found = 50;

        engine.reset_stats();

        assert_eq!(engine.stats.queries_executed, 0);
        assert_eq!(engine.stats.paths_found, 0);
    }

    #[test]
    fn test_reasoning_engine_serialization() {
        let engine = MultiHopReasoningEngine::new(4);
        let json = serde_json::to_string(&engine).unwrap();
        let deserialized: MultiHopReasoningEngine = serde_json::from_str(&json).unwrap();
        assert_eq!(engine.max_hops, deserialized.max_hops);
    }
}

// =============================================================================
// EntityExtractionActor Tests
// =============================================================================

mod entity_extraction_tests {
    use super::*;

    #[test]
    fn test_entity_extraction_actor_creation() {
        let actor = EntityExtractionActor::new();
        assert_eq!(actor.confidence_threshold, 0.5);
        assert!(actor.extractors.is_empty());
        assert_eq!(actor.stats.documents_processed, 0);
    }

    #[test]
    fn test_entity_extraction_actor_default() {
        let actor = EntityExtractionActor::default();
        assert_eq!(actor.confidence_threshold, 0.5);
    }

    #[test]
    fn test_entity_extraction_actor_with_config() {
        let extractors = vec![ExtractorConfig::person_name_extractor()];
        let actor = EntityExtractionActor::with_config(
            extractors,
            0.8,
            DeduplicationStrategy::ExactMatch,
        );
        assert_eq!(actor.confidence_threshold, 0.8);
        assert_eq!(actor.extractors.len(), 1);
    }

    #[test]
    fn test_add_extractor() {
        let mut actor = EntityExtractionActor::new();
        assert!(actor.extractors.is_empty());

        actor.add_extractor(ExtractorConfig::person_name_extractor());
        assert_eq!(actor.extractors.len(), 1);

        actor.add_extractor(ExtractorConfig::organization_extractor());
        assert_eq!(actor.extractors.len(), 2);
    }

    #[test]
    fn test_set_confidence_threshold() {
        let mut actor = EntityExtractionActor::new();
        actor.set_confidence_threshold(0.9);
        assert_eq!(actor.confidence_threshold, 0.9);

        // Test clamping
        actor.set_confidence_threshold(1.5);
        assert_eq!(actor.confidence_threshold, 1.0);

        actor.set_confidence_threshold(-0.5);
        assert_eq!(actor.confidence_threshold, 0.0);
    }

    #[test]
    fn test_deduplication_strategy_variants() {
        let none = DeduplicationStrategy::None;
        assert!(matches!(none, DeduplicationStrategy::None));

        let exact = DeduplicationStrategy::ExactMatch;
        assert!(matches!(exact, DeduplicationStrategy::ExactMatch));

        let fuzzy = DeduplicationStrategy::FuzzyMatch { threshold: 0.8 };
        if let DeduplicationStrategy::FuzzyMatch { threshold } = fuzzy {
            assert_eq!(threshold, 0.8);
        }

        let semantic = DeduplicationStrategy::SemanticSimilarity {
            threshold: 0.9,
            model: "text-embedding-ada-002".to_string(),
        };
        if let DeduplicationStrategy::SemanticSimilarity { threshold, model } = semantic {
            assert_eq!(threshold, 0.9);
            assert_eq!(model, "text-embedding-ada-002");
        }
    }

    #[test]
    fn test_person_name_extractor_config() {
        let extractor = ExtractorConfig::person_name_extractor();
        if let ExtractorConfig::RegexNER {
            name,
            patterns,
            case_sensitive,
        } = extractor
        {
            assert_eq!(name, "person_names");
            assert!(case_sensitive);
            assert!(patterns.contains_key(&EntityType::Person));
            assert!(!patterns.get(&EntityType::Person).unwrap().is_empty());
        } else {
            panic!("Expected RegexNER extractor");
        }
    }

    #[test]
    fn test_organization_extractor_config() {
        let extractor = ExtractorConfig::organization_extractor();
        if let ExtractorConfig::KeywordExtraction {
            name,
            keywords,
            fuzzy_matching,
        } = extractor
        {
            assert_eq!(name, "organizations");
            assert!(!fuzzy_matching);
            assert!(keywords.contains_key(&EntityType::Organization));
        } else {
            panic!("Expected KeywordExtraction extractor");
        }
    }

    #[test]
    fn test_basic_relationship_extractor_config() {
        let extractor = ExtractorConfig::basic_relationship_extractor();
        if let ExtractorConfig::RuleBasedRelations { name, rules } = extractor {
            assert_eq!(name, "basic_relations");
            assert!(!rules.is_empty());
            // Check for expected rules
            let rule_names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
            assert!(rule_names.contains(&"works_for"));
            assert!(rule_names.contains(&"located_in"));
        } else {
            panic!("Expected RuleBasedRelations extractor");
        }
    }

    #[test]
    fn test_relationship_rule_structure() {
        let rule = RelationshipRule {
            name: "test_rule".to_string(),
            pattern: r"\b(\w+)\s+owns\s+(\w+)\b".to_string(),
            relationship_type: "OWNS".to_string(),
            min_confidence: 0.75,
            context_window: 100,
        };
        assert_eq!(rule.name, "test_rule");
        assert_eq!(rule.relationship_type, "OWNS");
        assert_eq!(rule.min_confidence, 0.75);
        assert_eq!(rule.context_window, 100);
    }

    #[test]
    fn test_document_processing_request_structure() {
        let request = DocumentProcessingRequest {
            document_id: "doc123".to_string(),
            text: "John Smith works at Microsoft.".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("source".to_string(), serde_json::json!("manual"));
                meta
            },
            extractors: Some(vec!["person_names".to_string()]),
            confidence_threshold: Some(0.7),
        };
        assert_eq!(request.document_id, "doc123");
        assert!(request.metadata.contains_key("source"));
        assert_eq!(request.extractors.as_ref().unwrap().len(), 1);
        assert_eq!(request.confidence_threshold, Some(0.7));
    }

    #[tokio::test]
    async fn test_process_document_with_person_extractor() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::person_name_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "John Smith and Mary Johnson are colleagues.".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let result = actor.process_document(request).await.unwrap();
        assert_eq!(result.document_id, "test_doc");
        assert!(!result.entities.is_empty());
        assert_eq!(result.extractors_used, 1);

        let person_names: Vec<&str> = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Person)
            .map(|e| e.text.as_str())
            .collect();
        assert!(person_names.contains(&"John Smith"));
        assert!(person_names.contains(&"Mary Johnson"));
    }

    #[tokio::test]
    async fn test_process_document_with_organization_extractor() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::organization_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "The University of California partners with Microsoft Corporation.".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let result = actor.process_document(request).await.unwrap();
        assert!(!result.entities.is_empty());

        let has_org_entity = result
            .entities
            .iter()
            .any(|e| e.entity_type == EntityType::Organization);
        assert!(has_org_entity);
    }

    #[tokio::test]
    async fn test_process_document_stats_update() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::person_name_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "Alice Bob and Charlie Davis met today.".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let _ = actor.process_document(request).await.unwrap();

        let stats = actor.get_stats();
        assert_eq!(stats.documents_processed, 1);
        assert!(stats.entities_extracted > 0);
    }

    #[test]
    fn test_reset_stats() {
        let mut actor = EntityExtractionActor::new();
        actor.stats.documents_processed = 10;
        actor.stats.entities_extracted = 100;

        actor.reset_stats();

        assert_eq!(actor.stats.documents_processed, 0);
        assert_eq!(actor.stats.entities_extracted, 0);
    }

    #[test]
    fn test_extractor_config_serialization() {
        let extractor = ExtractorConfig::person_name_extractor();
        let json = serde_json::to_string(&extractor).unwrap();
        let deserialized: ExtractorConfig = serde_json::from_str(&json).unwrap();

        if let ExtractorConfig::RegexNER { name, .. } = deserialized {
            assert_eq!(name, "person_names");
        } else {
            panic!("Deserialization failed");
        }
    }
}

// =============================================================================
// KnowledgeGraphBuilder Tests
// =============================================================================

mod knowledge_graph_builder_tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_builder_creation() {
        let builder = KnowledgeGraphBuilder::new("test_kg".to_string());
        assert_eq!(builder.kg_name, "test_kg");
        assert_eq!(builder.stats.documents_processed, 0);
    }

    #[test]
    fn test_knowledge_graph_config_default() {
        let config = KnowledgeGraphConfig::default();
        assert!(config.entity_confidence_threshold > 0.0);
        assert!(config.relationship_confidence_threshold > 0.0);
        assert!(config.enable_relationship_inference);
        assert!(config.max_entities_per_document > 0);
        assert!(config.max_relationships_per_document > 0);
    }

    #[test]
    fn test_knowledge_graph_config_custom() {
        let config = KnowledgeGraphConfig {
            max_entities_per_document: 50,
            max_relationships_per_document: 100,
            entity_confidence_threshold: 0.8,
            relationship_confidence_threshold: 0.7,
            enable_relationship_inference: false,
            enable_property_enrichment: false,
            max_graph_size: 500_000,
            graph_backend: "rocksdb".to_string(),
            vector_backend: "milvus".to_string(),
            default_embedding_model: "openai-ada-002".to_string(),
        };
        assert_eq!(config.max_entities_per_document, 50);
        assert!(!config.enable_relationship_inference);
    }

    #[test]
    fn test_knowledge_graph_stats_default() {
        let stats = KnowledgeGraphStats::default();
        assert_eq!(stats.documents_processed, 0);
        assert_eq!(stats.entities_created, 0);
        assert_eq!(stats.relationships_created, 0);
        assert_eq!(stats.entities_merged, 0);
        assert_eq!(stats.relationships_deduplicated, 0);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn test_entity_deduplication_config_default() {
        let config = EntityDeduplicationConfig::default();
        assert!(config.exact_text_matching);
        assert!(config.fuzzy_text_matching);
        assert!(config.fuzzy_threshold > 0.0);
        assert!(config.semantic_threshold > 0.0);
        assert!(config.consider_entity_types);
        assert!(config.alias_matching);
    }

    #[test]
    fn test_entity_deduplication_config_custom() {
        let config = EntityDeduplicationConfig {
            exact_text_matching: true,
            fuzzy_text_matching: false,
            fuzzy_threshold: 0.9,
            semantic_similarity_matching: true,
            semantic_threshold: 0.95,
            consider_entity_types: false,
            alias_matching: false,
        };
        assert!(!config.fuzzy_text_matching);
        assert!(config.semantic_similarity_matching);
    }

    #[test]
    fn test_knowledge_graph_builder_with_config() {
        let config = KnowledgeGraphConfig {
            entity_confidence_threshold: 0.9,
            enable_relationship_inference: false,
            ..KnowledgeGraphConfig::default()
        };
        let dedup_config = EntityDeduplicationConfig::default();

        let builder =
            KnowledgeGraphBuilder::with_config("custom_kg".to_string(), config, dedup_config);
        assert_eq!(builder.kg_name, "custom_kg");
        assert!(!builder.config.enable_relationship_inference);
    }

    #[test]
    fn test_knowledge_graph_builder_serialization() {
        let builder = KnowledgeGraphBuilder::new("test".to_string());
        let json = serde_json::to_string(&builder).unwrap();
        let deserialized: KnowledgeGraphBuilder = serde_json::from_str(&json).unwrap();
        assert_eq!(builder.kg_name, deserialized.kg_name);
    }
}

// =============================================================================
// GraphRAGActor Tests
// =============================================================================

mod graph_rag_actor_tests {
    use super::*;

    #[test]
    fn test_graphrag_actor_creation() {
        let actor = GraphRAGActor::new("test_kg".to_string());
        assert_eq!(actor.kg_name, "test_kg");
        assert_eq!(actor.stats.documents_processed, 0);
        assert!(actor.entity_extractor.is_none());
        assert!(actor.graph_builder.is_none());
        assert!(actor.reasoning_engine.is_none());
        assert!(actor.llm_providers.is_empty());
    }

    #[test]
    fn test_graphrag_actor_default() {
        let actor = GraphRAGActor::default();
        assert_eq!(actor.kg_name, "default");
    }

    #[test]
    fn test_graphrag_config_default() {
        let config = GraphRAGConfig::default();
        assert!(config.max_entities_per_document > 0);
        assert!(config.max_relationships_per_document > 0);
        assert!(config.embedding_dimension > 0);
        assert!(config.similarity_threshold > 0.0);
        assert!(config.max_hops > 0);
        assert!(config.rag_context_size > 0);
        assert!(config.enable_entity_deduplication);
        assert!(config.enable_relationship_inference);
        assert!(matches!(
            config.default_search_strategy,
            SearchStrategy::Balanced
        ));
        assert_eq!(config.query_timeout_ms, 30_000);
    }

    #[test]
    fn test_graphrag_config_custom() {
        let config = GraphRAGConfig {
            max_entities_per_document: 50,
            max_relationships_per_document: 100,
            embedding_dimension: 768,
            similarity_threshold: 0.8,
            max_hops: 5,
            rag_context_size: 4096,
            enable_entity_deduplication: false,
            enable_relationship_inference: false,
            default_search_strategy: SearchStrategy::VectorFirst,
            query_timeout_ms: 60_000,
        };
        assert_eq!(config.max_entities_per_document, 50);
        assert_eq!(config.embedding_dimension, 768);
        assert!(!config.enable_entity_deduplication);
    }

    #[test]
    fn test_graphrag_actor_with_config() {
        let config = GraphRAGConfig {
            max_hops: 5,
            ..GraphRAGConfig::default()
        };
        let actor = GraphRAGActor::with_config("custom_kg".to_string(), config);
        assert_eq!(actor.kg_name, "custom_kg");
        assert_eq!(actor.config.max_hops, 5);
    }

    #[test]
    fn test_initialize_components() {
        let mut actor = GraphRAGActor::new("test".to_string());
        assert!(actor.entity_extractor.is_none());
        assert!(actor.graph_builder.is_none());
        assert!(actor.reasoning_engine.is_none());

        actor.initialize_components();

        assert!(actor.entity_extractor.is_some());
        assert!(actor.graph_builder.is_some());
        assert!(actor.reasoning_engine.is_some());
    }

    #[test]
    fn test_add_llm_provider() {
        use orbit_shared::graphrag::LLMProvider;

        let mut actor = GraphRAGActor::new("test".to_string());
        assert!(actor.llm_providers.is_empty());
        assert!(actor.default_llm_provider.is_none());

        let provider = LLMProvider::OpenAI {
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
        };

        actor.add_llm_provider("openai".to_string(), provider);

        assert_eq!(actor.llm_providers.len(), 1);
        assert_eq!(actor.default_llm_provider, Some("openai".to_string()));
    }

    #[test]
    fn test_add_multiple_llm_providers() {
        use orbit_shared::graphrag::LLMProvider;

        let mut actor = GraphRAGActor::new("test".to_string());

        let provider1 = LLMProvider::OpenAI {
            api_key: "test-key-1".to_string(),
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
        };

        let provider2 = LLMProvider::Anthropic {
            api_key: "test-key-2".to_string(),
            model: "claude-3".to_string(),
            temperature: None,
            max_tokens: None,
        };

        actor.add_llm_provider("openai".to_string(), provider1);
        actor.add_llm_provider("anthropic".to_string(), provider2);

        assert_eq!(actor.llm_providers.len(), 2);
        // First provider becomes default
        assert_eq!(actor.default_llm_provider, Some("openai".to_string()));
    }

    #[test]
    fn test_graphrag_stats_default() {
        let stats = GraphRAGStats::default();
        assert_eq!(stats.documents_processed, 0);
        assert_eq!(stats.rag_queries_executed, 0);
        assert_eq!(stats.reasoning_queries_executed, 0);
        assert_eq!(stats.entities_extracted, 0);
        assert_eq!(stats.relationships_extracted, 0);
        assert_eq!(stats.avg_document_processing_time_ms, 0.0);
        assert_eq!(stats.avg_rag_query_time_ms, 0.0);
        assert_eq!(stats.rag_success_rate, 0.0);
        assert!(stats.llm_usage_stats.is_empty());
    }

    #[test]
    fn test_graphrag_document_request_structure() {
        let request = GraphRAGDocumentRequest {
            document_id: "doc1".to_string(),
            text: "This is a test document.".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("author".to_string(), serde_json::json!("Test Author"));
                meta
            },
            build_knowledge_graph: true,
            generate_embeddings: true,
            extractors: Some(vec!["person_names".to_string()]),
        };
        assert_eq!(request.document_id, "doc1");
        assert!(request.build_knowledge_graph);
        assert!(request.generate_embeddings);
    }

    #[test]
    fn test_graphrag_query_structure() {
        let query = GraphRAGQuery {
            query_text: "What is the relationship between A and B?".to_string(),
            max_hops: Some(4),
            context_size: Some(2048),
            llm_provider: Some("openai".to_string()),
            search_strategy: Some(SearchStrategy::GraphFirst),
            include_explanation: true,
            max_results: Some(20),
        };
        assert_eq!(query.max_hops, Some(4));
        assert!(query.include_explanation);
        assert_eq!(query.max_results, Some(20));
    }

    #[test]
    fn test_processing_times_structure() {
        let times = ProcessingTimes {
            entity_extraction_ms: 10,
            graph_traversal_ms: 50,
            vector_search_ms: 30,
            llm_generation_ms: 200,
            total_ms: 290,
        };
        assert_eq!(times.total_ms, 290);
        assert!(
            times.total_ms
                >= times.entity_extraction_ms
                    + times.graph_traversal_ms
                    + times.vector_search_ms
                    + times.llm_generation_ms
                    - 10
        ); // Allow small variance
    }

    #[test]
    fn test_update_config() {
        let mut actor = GraphRAGActor::new("test".to_string());
        let original_updated_at = actor.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_config = GraphRAGConfig {
            max_hops: 10,
            ..GraphRAGConfig::default()
        };
        actor.update_config(new_config);

        assert_eq!(actor.config.max_hops, 10);
        assert!(actor.updated_at >= original_updated_at);
    }

    #[test]
    fn test_graphrag_query_minimal() {
        // Test minimal query with just required fields
        let query = GraphRAGQuery {
            query_text: "Simple query".to_string(),
            max_hops: None,
            context_size: None,
            llm_provider: None,
            search_strategy: None,
            include_explanation: false,
            max_results: None,
        };
        assert_eq!(query.query_text, "Simple query");
        assert!(!query.include_explanation);
    }

    #[test]
    fn test_graphrag_actor_serialization() {
        let actor = GraphRAGActor::new("test_kg".to_string());
        let json = serde_json::to_string(&actor).unwrap();
        let deserialized: GraphRAGActor = serde_json::from_str(&json).unwrap();
        assert_eq!(actor.kg_name, deserialized.kg_name);
    }
}

// =============================================================================
// Search Strategy Tests
// =============================================================================

mod search_strategy_tests {
    use super::*;

    #[test]
    fn test_search_strategy_variants() {
        let balanced = SearchStrategy::Balanced;
        assert!(matches!(balanced, SearchStrategy::Balanced));

        let graph_first = SearchStrategy::GraphFirst;
        assert!(matches!(graph_first, SearchStrategy::GraphFirst));

        let vector_first = SearchStrategy::VectorFirst;
        assert!(matches!(vector_first, SearchStrategy::VectorFirst));
    }

    #[test]
    fn test_search_strategy_serialization() {
        let strategy = SearchStrategy::Balanced;
        let json = serde_json::to_string(&strategy).unwrap();
        let deserialized: SearchStrategy = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, SearchStrategy::Balanced));
    }
}

// =============================================================================
// Entity Type Tests
// =============================================================================

mod entity_type_tests {
    use super::*;

    #[test]
    fn test_entity_type_person() {
        let entity_type = EntityType::Person;
        assert!(matches!(entity_type, EntityType::Person));
    }

    #[test]
    fn test_entity_type_organization() {
        let entity_type = EntityType::Organization;
        assert!(matches!(entity_type, EntityType::Organization));
    }

    #[test]
    fn test_entity_type_location() {
        let entity_type = EntityType::Location;
        assert!(matches!(entity_type, EntityType::Location));
    }

    #[test]
    fn test_entity_type_serialization() {
        let entity_type = EntityType::Person;
        let json = serde_json::to_string(&entity_type).unwrap();
        let deserialized: EntityType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, EntityType::Person));
    }

    #[test]
    fn test_entity_type_custom() {
        let entity_type = EntityType::Custom("Product".to_string());
        if let EntityType::Custom(name) = entity_type {
            assert_eq!(name, "Product");
        } else {
            panic!("Expected Custom entity type");
        }
    }
}

// =============================================================================
// Integration Scenario Tests
// =============================================================================

mod integration_scenarios {
    use super::*;

    #[tokio::test]
    async fn test_full_entity_extraction_pipeline() {
        // Setup actor with multiple extractors
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::person_name_extractor());
        actor.add_extractor(ExtractorConfig::organization_extractor());

        // Process a document with both person and organization mentions
        let request = DocumentProcessingRequest {
            document_id: "integration_test".to_string(),
            text: "John Smith joined Microsoft Corporation in 2020. Mary Johnson works at Google Inc."
                .to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let result = actor.process_document(request).await.unwrap();

        // Verify multiple entity types were extracted
        let person_count = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Person)
            .count();
        let org_count = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Organization)
            .count();

        assert!(person_count >= 2, "Should find at least 2 persons");
        assert!(org_count >= 1, "Should find at least 1 organization");
        assert_eq!(result.extractors_used, 2);
    }

    #[test]
    fn test_graphrag_actor_full_initialization() {
        use orbit_shared::graphrag::LLMProvider;

        let mut actor = GraphRAGActor::with_config(
            "full_test".to_string(),
            GraphRAGConfig {
                max_hops: 4,
                rag_context_size: 4096,
                ..GraphRAGConfig::default()
            },
        );

        // Add LLM provider
        let provider = LLMProvider::OpenAI {
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            max_tokens: Some(1000),
            temperature: Some(0.5),
        };
        actor.add_llm_provider("test_llm".to_string(), provider);

        // Initialize components
        actor.initialize_components();

        // Verify full initialization
        assert!(actor.entity_extractor.is_some());
        assert!(actor.graph_builder.is_some());
        assert!(actor.reasoning_engine.is_some());
        assert_eq!(actor.llm_providers.len(), 1);
        assert_eq!(actor.config.max_hops, 4);
    }

    #[test]
    fn test_reasoning_engine_configuration_chain() {
        let mut engine = MultiHopReasoningEngine::new(3);

        // Configure scoring
        engine.update_scoring_strategy(PathScoringStrategy::Combined {
            confidence_weight: 0.5,
            length_weight: 0.3,
            importance_weight: 0.2,
        });

        // Configure pruning
        engine.update_pruning_strategy(PruningStrategy::Combined {
            score_threshold: 0.3,
            max_branches: 30,
            avoid_cycles: true,
        });

        // Configure general settings
        engine.update_config(ReasoningConfig {
            max_paths_per_hop: 75,
            max_results: 25,
            bidirectional_search: true,
            ..ReasoningConfig::default()
        });

        // Verify all configurations
        if let PathScoringStrategy::Combined {
            confidence_weight, ..
        } = engine.path_scoring
        {
            assert_eq!(confidence_weight, 0.5);
        }

        if let PruningStrategy::Combined {
            score_threshold, ..
        } = engine.pruning_strategy
        {
            assert_eq!(score_threshold, 0.3);
        }

        assert_eq!(engine.config.max_paths_per_hop, 75);
    }

    #[tokio::test]
    async fn test_document_processing_with_custom_threshold() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::person_name_extractor());

        // Process with high confidence threshold
        let request = DocumentProcessingRequest {
            document_id: "threshold_test".to_string(),
            text: "John Smith met Jane Doe.".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: Some(0.95), // Very high threshold
        };

        let result = actor.process_document(request).await.unwrap();

        // All entities should have confidence >= 0.95
        for entity in &result.entities {
            assert!(
                entity.confidence >= 0.95,
                "Entity confidence {} should be >= 0.95",
                entity.confidence
            );
        }
    }
}
