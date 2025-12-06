CALL orbit.graphrag.findSimilar('research_kg','Transformers',{limit:10, similarity_threshold:0.8}) YIELD entity_text, entity_type, similarity, confidence
RETURN entity_text, entity_type, similarity, confidence;
CALL orbit.graphrag.analyzeTrends('research_kg','Transformers',{time_window_days:60}) YIELD timestamp, relationship_count, concept_entities_found
RETURN timestamp, relationship_count, concept_entities_found;
CALL orbit.graphrag.listEntities('research_kg',{limit:50}) YIELD id, text, entity_type, confidence, labels, source_documents
RETURN id, text, entity_type, confidence, labels, source_documents;
