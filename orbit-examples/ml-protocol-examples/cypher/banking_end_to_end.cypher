// End-to-end banking example in Cypher with embeddings and GraphRAG
// 1) Create customers, accounts, merchants; ownership relationships
// 2) Generate vector embeddings for merchants and compute similarity
// 3) Ingest policy documents and query via GraphRAG
CREATE (:Customer {name:'Alice', segment:'low'})
CREATE (:Customer {name:'Bob', segment:'medium'})
CREATE (:Account {type:'checking'})
CREATE (:Account {type:'credit'})
CREATE (:Merchant {name:'SuperMart', category:'grocery'})
CREATE (:Merchant {name:'TechWorld', category:'electronics'})
// Ownership edges
MATCH (c:Customer {name:'Alice'}),(a:Account {type:'checking'}) CREATE (c)-[:OWNS]->(a)
MATCH (c:Customer {name:'Alice'}),(a:Account {type:'credit'}) CREATE (c)-[:OWNS]->(a)
// Merchant embeddings and similarity
MATCH (m:Merchant {name:'TechWorld'}) SET m.embedding = ML_EMBED_TEXT(m.name,'sentence-transformers')
MATCH (m:Merchant {name:'SuperMart'}) SET m.embedding = ML_EMBED_TEXT(m.name,'sentence-transformers')
MATCH (m1:Merchant {name:'TechWorld'}),(m2:Merchant {name:'SuperMart'}) RETURN 1 - (m1.embedding <=> m2.embedding) AS merchant_similarity
// GraphRAG knowledge ingestion
CALL orbit.graphrag.buildKnowledge('banking_kg','doc_policy','Online electronics purchases over $500 require secondary verification.',{source:'policy',domain:'banking'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
// GraphRAG query with path reasoning
CALL orbit.graphrag.ragQuery('banking_kg','Explain rules affecting high-value online purchases.',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
