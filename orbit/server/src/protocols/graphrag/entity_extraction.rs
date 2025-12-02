//! Entity extraction actor for GraphRAG
//!
//! This module provides natural language processing capabilities for extracting
//! entities and relationships from text documents for knowledge graph construction.

use orbit_shared::graphrag::{EntityType, ExtractedEntity, ExtractedRelationship};
use orbit_shared::{Addressable, OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Entity extraction actor for NLP pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractionActor {
    /// Configured extractors
    pub extractors: Vec<ExtractorConfig>,

    /// Minimum confidence threshold
    pub confidence_threshold: f32,

    /// Entity deduplication strategy
    pub deduplication_strategy: DeduplicationStrategy,

    /// Statistics and performance metrics
    pub stats: ExtractionStats,

    /// Actor creation timestamp
    pub created_at: i64,

    /// Last activity timestamp
    pub updated_at: i64,
}

/// Configuration for different extraction methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractorConfig {
    /// Named Entity Recognition using regex patterns
    RegexNER {
        name: String,
        patterns: HashMap<EntityType, Vec<String>>,
        case_sensitive: bool,
    },

    /// Simple keyword-based entity extraction
    KeywordExtraction {
        name: String,
        keywords: HashMap<EntityType, Vec<String>>,
        fuzzy_matching: bool,
    },

    /// Rule-based relationship extraction
    RuleBasedRelations {
        name: String,
        rules: Vec<RelationshipRule>,
    },

    /// LLM-based extraction (future implementation)
    LLMBased {
        name: String,
        provider: String, // LLMProvider reference
        prompt_template: String,
        entity_types: Vec<EntityType>,
    },

    /// Custom extraction function (future implementation)
    Custom {
        name: String,
        function_name: String,
        parameters: HashMap<String, serde_json::Value>,
    },
}

/// Strategy for deduplicating extracted entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeduplicationStrategy {
    /// No deduplication
    None,

    /// Exact text matching
    ExactMatch,

    /// Fuzzy string matching with threshold
    FuzzyMatch { threshold: f32 },

    /// Semantic similarity using embeddings
    SemanticSimilarity { threshold: f32, model: String },
}

/// Rule for extracting relationships between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipRule {
    /// Rule name/identifier
    pub name: String,

    /// Pattern to match (simple regex for now)
    pub pattern: String,

    /// Relationship type to assign
    pub relationship_type: String,

    /// Minimum confidence for this rule
    pub min_confidence: f32,

    /// Context window around the pattern
    pub context_window: usize,
}

/// Extraction statistics and metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionStats {
    /// Total documents processed
    pub documents_processed: u64,

    /// Total entities extracted
    pub entities_extracted: u64,

    /// Total relationships extracted
    pub relationships_extracted: u64,

    /// Average processing time per document (ms)
    pub avg_processing_time_ms: f64,

    /// Entities by type
    pub entities_by_type: HashMap<EntityType, u64>,

    /// Relationships by type
    pub relationships_by_type: HashMap<String, u64>,

    /// Last update timestamp
    pub last_updated: i64,
}

/// Document processing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProcessingRequest {
    /// Document ID
    pub document_id: String,

    /// Document text content
    pub text: String,

    /// Document metadata
    pub metadata: HashMap<String, serde_json::Value>,

    /// Specific extractors to use (optional)
    pub extractors: Option<Vec<String>>,

    /// Override confidence threshold
    pub confidence_threshold: Option<f32>,
}

/// Document processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProcessingResult {
    /// Document ID
    pub document_id: String,

    /// Extracted entities
    pub entities: Vec<ExtractedEntity>,

    /// Extracted relationships
    pub relationships: Vec<ExtractedRelationship>,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Number of extractors used
    pub extractors_used: usize,

    /// Any warnings or errors
    pub warnings: Vec<String>,
}

impl EntityExtractionActor {
    /// Create a new entity extraction actor
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            extractors: Vec::new(),
            confidence_threshold: 0.5,
            deduplication_strategy: DeduplicationStrategy::FuzzyMatch { threshold: 0.8 },
            stats: ExtractionStats::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create actor with specific configuration
    pub fn with_config(
        extractors: Vec<ExtractorConfig>,
        confidence_threshold: f32,
        deduplication_strategy: DeduplicationStrategy,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            extractors,
            confidence_threshold: confidence_threshold.clamp(0.0, 1.0),
            deduplication_strategy,
            stats: ExtractionStats::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add an extractor configuration
    pub fn add_extractor(&mut self, extractor: ExtractorConfig) {
        self.extractors.push(extractor);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Process a document and extract entities/relationships
    pub async fn process_document(
        &mut self,
        request: DocumentProcessingRequest,
    ) -> OrbitResult<DocumentProcessingResult> {
        let start_time = std::time::Instant::now();

        debug!(
            document_id = %request.document_id,
            text_length = request.text.len(),
            "Processing document for entity extraction"
        );

        let confidence_threshold = request
            .confidence_threshold
            .unwrap_or(self.confidence_threshold);

        // Extract entities
        let mut all_entities = Vec::new();
        let mut all_relationships = Vec::new();
        let mut extractors_used = 0;
        let mut warnings = Vec::new();

        // Determine which extractors to use
        let extractors_to_use = if let Some(extractor_names) = &request.extractors {
            self.extractors
                .iter()
                .filter(|e| extractor_names.contains(&self.get_extractor_name(e)))
                .collect::<Vec<_>>()
        } else {
            self.extractors.iter().collect()
        };

        for extractor in extractors_to_use {
            extractors_used += 1;

            match self.apply_extractor(extractor, &request).await {
                Ok((entities, relationships)) => {
                    all_entities.extend(entities);
                    all_relationships.extend(relationships);
                }
                Err(e) => {
                    warn!(
                        extractor = self.get_extractor_name(extractor),
                        error = %e,
                        "Extractor failed"
                    );
                    warnings.push(format!(
                        "Extractor '{}' failed: {}",
                        self.get_extractor_name(extractor),
                        e
                    ));
                }
            }
        }

        // Filter by confidence threshold
        all_entities.retain(|e| e.confidence >= confidence_threshold);
        all_relationships.retain(|r| r.confidence >= confidence_threshold);

        // Apply deduplication
        all_entities = self.deduplicate_entities(all_entities).await?;
        all_relationships = self.deduplicate_relationships(all_relationships).await?;

        let processing_time = start_time.elapsed();
        let processing_time_ms = processing_time.as_millis() as u64;

        // Update statistics
        self.update_stats(&all_entities, &all_relationships, processing_time_ms);

        info!(
            document_id = %request.document_id,
            entities_count = all_entities.len(),
            relationships_count = all_relationships.len(),
            processing_time_ms = processing_time_ms,
            "Document processing completed"
        );

        Ok(DocumentProcessingResult {
            document_id: request.document_id,
            entities: all_entities,
            relationships: all_relationships,
            processing_time_ms,
            extractors_used,
            warnings,
        })
    }

    /// Apply a specific extractor to the document
    async fn apply_extractor(
        &self,
        extractor: &ExtractorConfig,
        request: &DocumentProcessingRequest,
    ) -> OrbitResult<(Vec<ExtractedEntity>, Vec<ExtractedRelationship>)> {
        match extractor {
            ExtractorConfig::RegexNER {
                patterns,
                case_sensitive,
                ..
            } => {
                self.apply_regex_ner(patterns, *case_sensitive, request)
                    .await
            }
            ExtractorConfig::KeywordExtraction {
                keywords,
                fuzzy_matching,
                ..
            } => {
                self.apply_keyword_extraction(keywords, *fuzzy_matching, request)
                    .await
            }
            ExtractorConfig::RuleBasedRelations { rules, .. } => {
                self.apply_relationship_rules(rules, request).await
            }
            ExtractorConfig::LLMBased {
                name: _,
                provider,
                prompt_template,
                entity_types,
            } => {
                self.apply_llm_extraction(provider, prompt_template, entity_types, request)
                    .await
            }
            ExtractorConfig::Custom { .. } => {
                // TODO: Implement custom extraction functions
                warn!("Custom extraction functions not yet implemented");
                Ok((Vec::new(), Vec::new()))
            }
        }
    }

    /// Apply regex-based named entity recognition
    async fn apply_regex_ner(
        &self,
        patterns: &HashMap<EntityType, Vec<String>>,
        case_sensitive: bool,
        request: &DocumentProcessingRequest,
    ) -> OrbitResult<(Vec<ExtractedEntity>, Vec<ExtractedRelationship>)> {
        let mut entities = Vec::new();

        for (entity_type, pattern_list) in patterns {
            for pattern_str in pattern_list {
                let regex = if case_sensitive {
                    regex::Regex::new(pattern_str)
                } else {
                    regex::RegexBuilder::new(pattern_str)
                        .case_insensitive(true)
                        .build()
                };

                let regex = regex.map_err(|e| {
                    OrbitError::internal(format!("Invalid regex pattern '{pattern_str}': {e}"))
                })?;

                for mat in regex.find_iter(&request.text) {
                    let entity = ExtractedEntity {
                        text: mat.as_str().to_string(),
                        entity_type: entity_type.clone(),
                        confidence: 0.8, // Fixed confidence for regex matches
                        start_pos: mat.start(),
                        end_pos: mat.end(),
                        properties: HashMap::new(),
                        aliases: Vec::new(),
                        source_document: request.document_id.clone(),
                    };

                    entities.push(entity);
                }
            }
        }

        Ok((entities, Vec::new())) // Regex NER doesn't extract relationships
    }

    /// Apply keyword-based entity extraction
    async fn apply_keyword_extraction(
        &self,
        keywords: &HashMap<EntityType, Vec<String>>,
        fuzzy_matching: bool,
        request: &DocumentProcessingRequest,
    ) -> OrbitResult<(Vec<ExtractedEntity>, Vec<ExtractedRelationship>)> {
        let mut entities = Vec::new();
        let text_lower = request.text.to_lowercase();

        for (entity_type, keyword_list) in keywords {
            for keyword in keyword_list {
                let keyword_lower = keyword.to_lowercase();

                if fuzzy_matching {
                    // Fuzzy matching using Levenshtein distance
                    let matches = self.find_fuzzy_matches(&text_lower, &keyword_lower, 0.8);
                    for (pos, matched_text, similarity) in matches {
                        let text_len = matched_text.len();
                        let entity = ExtractedEntity {
                            text: matched_text,
                            entity_type: entity_type.clone(),
                            confidence: similarity,
                            start_pos: pos,
                            end_pos: pos + text_len,
                            properties: HashMap::new(),
                            aliases: Vec::new(),
                            source_document: request.document_id.clone(),
                        };

                        entities.push(entity);
                    }
                } else {
                    // Exact matching
                    if text_lower.contains(&keyword_lower) {
                        if let Some(pos) = text_lower.find(&keyword_lower) {
                            let entity = ExtractedEntity {
                                text: keyword.clone(),
                                entity_type: entity_type.clone(),
                                confidence: 0.9,
                                start_pos: pos,
                                end_pos: pos + keyword.len(),
                                properties: HashMap::new(),
                                aliases: Vec::new(),
                                source_document: request.document_id.clone(),
                            };

                            entities.push(entity);
                        }
                    }
                }
            }
        }

        Ok((entities, Vec::new())) // Keyword extraction doesn't extract relationships
    }

    /// Apply rule-based relationship extraction
    async fn apply_relationship_rules(
        &self,
        rules: &[RelationshipRule],
        request: &DocumentProcessingRequest,
    ) -> OrbitResult<(Vec<ExtractedEntity>, Vec<ExtractedRelationship>)> {
        let mut relationships = Vec::new();

        for rule in rules {
            let regex = regex::Regex::new(&rule.pattern).map_err(|e| {
                OrbitError::internal(format!(
                    "Invalid relationship rule pattern '{}': {}",
                    rule.pattern, e
                ))
            })?;

            for mat in regex.find_iter(&request.text) {
                // Simple relationship extraction - assumes pattern captures entities
                // TODO: Implement more sophisticated relationship extraction
                let relationship = ExtractedRelationship {
                    from_entity: "unknown".to_string(), // TODO: Extract actual entities
                    to_entity: "unknown".to_string(),   // TODO: Extract actual entities
                    relationship_type: rule.relationship_type.clone(),
                    confidence: rule.min_confidence,
                    source_text: mat.as_str().to_string(),
                    properties: HashMap::new(),
                    source_document: request.document_id.clone(),
                };

                relationships.push(relationship);
            }
        }

        Ok((Vec::new(), relationships)) // Rule-based only extracts relationships
    }

    /// Deduplicate extracted entities
    async fn deduplicate_entities(
        &self,
        entities: Vec<ExtractedEntity>,
    ) -> OrbitResult<Vec<ExtractedEntity>> {
        match &self.deduplication_strategy {
            DeduplicationStrategy::None => Ok(entities),
            DeduplicationStrategy::ExactMatch => {
                let mut deduped = Vec::new();
                let mut seen_texts = std::collections::HashSet::new();

                for entity in entities {
                    let key = (entity.text.clone(), entity.entity_type.clone());
                    if !seen_texts.contains(&key) {
                        seen_texts.insert(key);
                        deduped.push(entity);
                    }
                }

                Ok(deduped)
            }
            DeduplicationStrategy::FuzzyMatch { threshold } => {
                self.fuzzy_deduplicate(entities, *threshold).await
            }
            DeduplicationStrategy::SemanticSimilarity {
                threshold,
                model: _,
            } => {
                // Semantic similarity deduplication using embeddings
                // For now, fallback to fuzzy matching (embeddings would require vector store)
                warn!("Semantic similarity deduplication using fuzzy matching fallback");
                self.fuzzy_deduplicate(entities, *threshold).await
            }
        }
    }

    /// Deduplicate extracted relationships
    async fn deduplicate_relationships(
        &self,
        relationships: Vec<ExtractedRelationship>,
    ) -> OrbitResult<Vec<ExtractedRelationship>> {
        // Simple deduplication based on entity pair and relationship type
        let mut deduped = Vec::new();
        let mut seen_relationships = std::collections::HashSet::new();

        for relationship in relationships {
            let key = (
                relationship.from_entity.clone(),
                relationship.to_entity.clone(),
                relationship.relationship_type.clone(),
            );

            if !seen_relationships.contains(&key) {
                seen_relationships.insert(key);
                deduped.push(relationship);
            }
        }

        Ok(deduped)
    }

    /// Update extraction statistics
    fn update_stats(
        &mut self,
        entities: &[ExtractedEntity],
        relationships: &[ExtractedRelationship],
        processing_time_ms: u64,
    ) {
        self.stats.documents_processed += 1;
        self.stats.entities_extracted += entities.len() as u64;
        self.stats.relationships_extracted += relationships.len() as u64;

        // Update average processing time
        let total_time = (self.stats.avg_processing_time_ms
            * (self.stats.documents_processed - 1) as f64)
            + processing_time_ms as f64;
        self.stats.avg_processing_time_ms = total_time / self.stats.documents_processed as f64;

        // Update entity counts by type
        for entity in entities {
            *self
                .stats
                .entities_by_type
                .entry(entity.entity_type.clone())
                .or_insert(0) += 1;
        }

        // Update relationship counts by type
        for relationship in relationships {
            *self
                .stats
                .relationships_by_type
                .entry(relationship.relationship_type.clone())
                .or_insert(0) += 1;
        }

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
        self.updated_at = self.stats.last_updated;
    }

    /// Get extractor name for identification
    fn get_extractor_name(&self, extractor: &ExtractorConfig) -> String {
        match extractor {
            ExtractorConfig::RegexNER { name, .. } => name.clone(),
            ExtractorConfig::KeywordExtraction { name, .. } => name.clone(),
            ExtractorConfig::RuleBasedRelations { name, .. } => name.clone(),
            ExtractorConfig::LLMBased { name, .. } => name.clone(),
            ExtractorConfig::Custom { name, .. } => name.clone(),
        }
    }

    /// Get extraction statistics
    pub fn get_stats(&self) -> &ExtractionStats {
        &self.stats
    }

    /// Reset extraction statistics
    pub fn reset_stats(&mut self) {
        self.stats = ExtractionStats::default();
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Get configured extractors
    pub fn get_extractors(&self) -> &[ExtractorConfig] {
        &self.extractors
    }

    /// Update confidence threshold
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update deduplication strategy
    pub fn set_deduplication_strategy(&mut self, strategy: DeduplicationStrategy) {
        self.deduplication_strategy = strategy;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Apply LLM-based entity extraction
    async fn apply_llm_extraction(
        &self,
        provider_name: &str,
        prompt_template: &str,
        entity_types: &[EntityType],
        request: &DocumentProcessingRequest,
    ) -> OrbitResult<(Vec<ExtractedEntity>, Vec<ExtractedRelationship>)> {
        use crate::protocols::graphrag::llm_client::{create_llm_client, LLMGenerationRequest};
        use orbit_shared::graphrag::LLMProvider;
        use std::str::FromStr;

        // Get LLM provider from environment or configuration
        let llm_provider = if provider_name == "openai" {
            if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
                LLMProvider::OpenAI {
                    api_key,
                    model: "gpt-4".to_string(),
                    temperature: Some(0.3),
                    max_tokens: Some(2048),
                }
            } else {
                warn!("OpenAI API key not found, skipping LLM extraction");
                return Ok((Vec::new(), Vec::new()));
            }
        } else if provider_name == "ollama" {
            let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama2".to_string());
            LLMProvider::Ollama {
                model,
                temperature: Some(0.3),
            }
        } else {
            warn!(
                "Unknown LLM provider: {}, skipping LLM extraction",
                provider_name
            );
            return Ok((Vec::new(), Vec::new()));
        };

        // Build prompt with entity types
        let entity_types_str = entity_types
            .iter()
            .map(|et| format!("{:?}", et))
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = prompt_template
            .replace("{text}", &request.text)
            .replace("{entity_types}", &entity_types_str);

        let llm_client = create_llm_client(&llm_provider)
            .map_err(|e| OrbitError::internal(format!("Failed to create LLM client: {}", e)))?;

        let generation_request = LLMGenerationRequest {
            prompt,
            max_tokens: Some(2048),
            temperature: Some(0.3),
            system_message: Some(
                "You are an entity extraction system. Extract entities and relationships from the text. "
                .to_string() +
                "Return JSON in format: {\"entities\": [{\"text\": \"...\", \"type\": \"...\", \"confidence\": 0.9}], " +
                "\"relationships\": [{\"from\": \"...\", \"to\": \"...\", \"type\": \"...\"}]}"
            ),
        };

        let response = llm_client
            .generate(generation_request)
            .await
            .map_err(|e| OrbitError::internal(format!("LLM extraction failed: {}", e)))?;

        // Parse JSON response
        let json: serde_json::Value = serde_json::from_str(&response.text)
            .map_err(|e| OrbitError::internal(format!("Failed to parse LLM response: {}", e)))?;

        let mut entities = Vec::new();
        let mut relationships = Vec::new();

        // Parse entities
        if let Some(entities_array) = json.get("entities").and_then(|v| v.as_array()) {
            for entity_json in entities_array {
                if let (Some(text), Some(type_str)) = (
                    entity_json.get("text").and_then(|v| v.as_str()),
                    entity_json.get("type").and_then(|v| v.as_str()),
                ) {
                    let entity_type = EntityType::from_str(type_str)
                        .unwrap_or(EntityType::Custom(type_str.to_string()));

                    let confidence = entity_json
                        .get("confidence")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.8) as f32;

                    let start_pos = request.text.find(text).unwrap_or(0);
                    let end_pos = start_pos + text.len();

                    entities.push(ExtractedEntity {
                        text: text.to_string(),
                        entity_type,
                        confidence,
                        start_pos,
                        end_pos,
                        properties: HashMap::new(),
                        aliases: Vec::new(),
                        source_document: request.document_id.clone(),
                    });
                }
            }
        }

        // Parse relationships
        if let Some(rels_array) = json.get("relationships").and_then(|v| v.as_array()) {
            for rel_json in rels_array {
                if let (Some(from), Some(to), Some(rel_type)) = (
                    rel_json.get("from").and_then(|v| v.as_str()),
                    rel_json.get("to").and_then(|v| v.as_str()),
                    rel_json.get("type").and_then(|v| v.as_str()),
                ) {
                    let confidence = rel_json
                        .get("confidence")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.7) as f32;

                    relationships.push(ExtractedRelationship {
                        from_entity: from.to_string(),
                        to_entity: to.to_string(),
                        relationship_type: rel_type.to_string(),
                        confidence,
                        source_text: format!("{} {} {}", from, rel_type, to),
                        properties: HashMap::new(),
                        source_document: request.document_id.clone(),
                    });
                }
            }
        }

        Ok((entities, relationships))
    }

    /// Find fuzzy matches in text using sliding window
    fn find_fuzzy_matches(
        &self,
        text: &str,
        keyword: &str,
        min_similarity: f32,
    ) -> Vec<(usize, String, f32)> {
        let mut matches = Vec::new();
        let keyword_len = keyword.len();
        let window_size = keyword_len.max(3);

        for i in 0..=text.len().saturating_sub(window_size) {
            let end = (i + window_size).min(text.len());
            let window = &text[i..end];

            let similarity = self.calculate_string_similarity(keyword, window);
            if similarity >= min_similarity {
                matches.push((i, window.to_string(), similarity));
            }
        }

        matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let mut filtered: Vec<(usize, String, f32)> = Vec::new();
        for (pos, matched_text, sim) in matches {
            let text_len = matched_text.len();
            let matched_text_for_push = matched_text.clone();
            if !filtered.iter().any(|(p, t, _)| {
                let t_len = t.len();
                (pos >= *p && pos < *p + t_len) || (*p >= pos && *p < pos + text_len)
            }) {
                filtered.push((pos, matched_text_for_push, sim));
            }
        }

        filtered
    }

    /// Calculate string similarity using Levenshtein distance
    fn calculate_string_similarity(&self, s1: &str, s2: &str) -> f32 {
        let distance = self.levenshtein_distance(s1, s2);
        let max_len = s1.len().max(s2.len());
        if max_len == 0 {
            return 1.0;
        }
        1.0 - (distance as f32 / max_len as f32)
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let s1_len = s1_chars.len();
        let s2_len = s2_chars.len();

        if s1_len == 0 {
            return s2_len;
        }
        if s2_len == 0 {
            return s1_len;
        }

        let mut matrix = vec![vec![0; s2_len + 1]; s1_len + 1];

        for i in 0..=s1_len {
            matrix[i][0] = i;
        }
        for j in 0..=s2_len {
            matrix[0][j] = j;
        }

        for i in 1..=s1_len {
            for j in 1..=s2_len {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[s1_len][s2_len]
    }

    /// Fuzzy deduplicate entities based on similarity threshold
    async fn fuzzy_deduplicate(
        &self,
        entities: Vec<ExtractedEntity>,
        threshold: f32,
    ) -> OrbitResult<Vec<ExtractedEntity>> {
        let mut deduped: Vec<ExtractedEntity> = Vec::new();
        let mut processed: Vec<ExtractedEntity> = Vec::new();

        for entity in entities {
            let mut is_duplicate = false;

            for processed_entity in &processed {
                if entity.entity_type == processed_entity.entity_type {
                    let similarity = self.calculate_string_similarity(
                        &entity.text.to_lowercase(),
                        &processed_entity.text.to_lowercase(),
                    );

                    if similarity >= threshold {
                        is_duplicate = true;
                        if entity.confidence > processed_entity.confidence {
                            if let Some(pos) = deduped.iter().position(|e: &ExtractedEntity| {
                                e.text == processed_entity.text
                                    && e.entity_type == processed_entity.entity_type
                            }) {
                                deduped[pos] = entity.clone();
                                processed.push(entity.clone());
                            }
                        }
                        break;
                    }
                }
            }

            if !is_duplicate {
                deduped.push(entity.clone());
                processed.push(entity);
            }
        }

        Ok(deduped)
    }
}

impl Default for EntityExtractionActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for EntityExtractionActor {
    fn addressable_type() -> &'static str {
        "EntityExtractionActor"
    }
}

/// Helper functions for creating common extractor configurations
impl ExtractorConfig {
    /// Create a simple person name extractor using regex
    pub fn person_name_extractor() -> Self {
        let mut patterns = HashMap::new();
        patterns.insert(
            EntityType::Person,
            vec![
                r"\b[A-Z][a-z]+ [A-Z][a-z]+\b".to_string(), // First Last
                r"\b[A-Z][a-z]+ [A-Z]\. [A-Z][a-z]+\b".to_string(), // First M. Last
                r"\b[A-Z][a-z]+, [A-Z][a-z]+\b".to_string(), // Last, First
            ],
        );

        ExtractorConfig::RegexNER {
            name: "person_names".to_string(),
            patterns,
            case_sensitive: true,
        }
    }

    /// Create an organization name extractor
    pub fn organization_extractor() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert(
            EntityType::Organization,
            vec![
                "Company".to_string(),
                "Corporation".to_string(),
                "Inc".to_string(),
                "LLC".to_string(),
                "Ltd".to_string(),
                "University".to_string(),
                "Institute".to_string(),
                "Foundation".to_string(),
            ],
        );

        ExtractorConfig::KeywordExtraction {
            name: "organizations".to_string(),
            keywords,
            fuzzy_matching: false,
        }
    }

    /// Create a basic relationship extractor
    pub fn basic_relationship_extractor() -> Self {
        let rules = vec![
            RelationshipRule {
                name: "works_for".to_string(),
                pattern: r"\b(\w+)\s+works\s+for\s+(\w+)\b".to_string(),
                relationship_type: "WORKS_FOR".to_string(),
                min_confidence: 0.7,
                context_window: 50,
            },
            RelationshipRule {
                name: "located_in".to_string(),
                pattern: r"\b(\w+)\s+(?:is\s+)?(?:located\s+)?in\s+(\w+)\b".to_string(),
                relationship_type: "LOCATED_IN".to_string(),
                min_confidence: 0.6,
                context_window: 30,
            },
        ];

        ExtractorConfig::RuleBasedRelations {
            name: "basic_relations".to_string(),
            rules,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_entity_extraction_actor_creation() {
        let actor = EntityExtractionActor::new();
        assert_eq!(actor.confidence_threshold, 0.5);
        assert!(actor.extractors.is_empty());
        assert_eq!(actor.stats.documents_processed, 0);
    }

    #[tokio::test]
    async fn test_person_name_extraction() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::person_name_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "John Smith works at Google. Mary Johnson is the CEO.".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let result = actor.process_document(request).await.unwrap();

        assert_eq!(result.document_id, "test_doc");
        assert!(!result.entities.is_empty());

        // Should find "John Smith" and "Mary Johnson"
        let person_names: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Person)
            .map(|e| e.text.as_str())
            .collect();

        assert!(person_names.contains(&"John Smith"));
        assert!(person_names.contains(&"Mary Johnson"));
    }

    #[tokio::test]
    async fn test_organization_extraction() {
        let mut actor = EntityExtractionActor::new();
        actor.add_extractor(ExtractorConfig::organization_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "The University of California and Microsoft Corporation are partners."
                .to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: None,
        };

        let result = actor.process_document(request).await.unwrap();

        assert!(!result.entities.is_empty());

        let org_entities: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Organization)
            .collect();

        assert!(!org_entities.is_empty());
    }

    #[tokio::test]
    async fn test_confidence_threshold() {
        let mut actor = EntityExtractionActor::new();
        actor.set_confidence_threshold(0.9);
        actor.add_extractor(ExtractorConfig::person_name_extractor());

        let request = DocumentProcessingRequest {
            document_id: "test_doc".to_string(),
            text: "John Smith is here".to_string(),
            metadata: HashMap::new(),
            extractors: None,
            confidence_threshold: Some(0.9),
        };

        let result = actor.process_document(request).await.unwrap();

        // Should filter entities below 0.9 confidence
        assert!(result.entities.iter().all(|e| e.confidence >= 0.9));
    }
}
