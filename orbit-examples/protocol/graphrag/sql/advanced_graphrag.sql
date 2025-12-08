-- Build knowledge graph from multiple documents with metadata
SELECT * FROM GRAPHRAG_BUILD('research_kg','doc_ml1','Machine Learning explores algorithms that learn from data. Deep Learning is a subset focusing on neural networks.','{"source":"wiki","category":"ml"}'::json);
SELECT * FROM GRAPHRAG_BUILD('research_kg','doc_dl1','Deep Learning architectures include CNNs and Transformers. Transformers enable attention-based sequence modeling.','{"source":"paper","category":"dl"}'::json);
SELECT * FROM GRAPHRAG_BUILD('research_kg','doc_quantum1','Quantum Computing leverages qubits and superposition to perform computations.','{"source":"news","category":"qc"}'::json);

-- Extract entities from raw text using specific extractors
SELECT * FROM GRAPHRAG_EXTRACT('research_kg','extract_001','Graph theory underpins many ML methods including GNNs.', ARRAY['named_entity','keyword']);

-- RAG query with larger context and explanation using specific LLM provider
SELECT * FROM GRAPHRAG_QUERY('research_kg','Summarize the relationship between Transformers and attention mechanisms.',3,4096,'ollama',true);

-- Multi-hop reasoning between concepts
SELECT * FROM GRAPHRAG_REASON('research_kg','Transformers','Graph Neural Networks',4);

-- Knowledge graph statistics
SELECT * FROM GRAPHRAG_STATS('research_kg');

-- List entities involved in the last query (if supported by backend)
-- Fallback: run a second query and capture entities_involved
SELECT * FROM GRAPHRAG_QUERY('research_kg','List entities related to Graph Neural Networks.',2,1024,'ollama',true);
