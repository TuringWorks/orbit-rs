CREATE (:Customer {name:'Alice', segment:'low'})
CREATE (:Customer {name:'Bob', segment:'medium'})
CREATE (:Account {type:'checking'})
CREATE (:Account {type:'credit'})
CREATE (:Merchant {name:'SuperMart', category:'grocery'})
CREATE (:Merchant {name:'TechWorld', category:'electronics'})
MATCH (c:Customer {name:'Alice'}),(a:Account {type:'checking'}) CREATE (c)-[:OWNS]->(a)
MATCH (c:Customer {name:'Alice'}),(a:Account {type:'credit'}) CREATE (c)-[:OWNS]->(a)
MATCH (m:Merchant {name:'TechWorld'}) SET m.embedding = ML_EMBED_TEXT(m.name,'sentence-transformers')
MATCH (m:Merchant {name:'SuperMart'}) SET m.embedding = ML_EMBED_TEXT(m.name,'sentence-transformers')
MATCH (m1:Merchant {name:'TechWorld'}),(m2:Merchant {name:'SuperMart'}) RETURN 1 - (m1.embedding <=> m2.embedding) AS merchant_similarity
CALL orbit.graphrag.buildKnowledge('banking_kg','doc_policy','Online electronics purchases over $500 require secondary verification.',{source:'policy',domain:'banking'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('banking_kg','Explain rules affecting high-value online purchases.',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
