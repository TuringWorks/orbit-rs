// End-to-end fashion example in Cypher with embeddings and GraphRAG
// 1) Create customers, products; relationships
// 2) Generate embeddings for product descriptions and compute similarity
// 3) Ingest merchandising policies and query via GraphRAG
CREATE (:Customer {name:'Lina', segment:'premium'})
CREATE (:Customer {name:'Tom', segment:'standard'})
CREATE (:Product {name:'Silk Dress', category:'apparel', description:'Elegant silk dress suitable for evening events'})
CREATE (:Product {name:'Sport Jacket', category:'apparel', description:'Breathable jacket designed for outdoor activity'})
MATCH (c:Customer {name:'Lina'}),(p:Product {name:'Silk Dress'}) CREATE (c)-[:VIEWED]->(p)
MATCH (p:Product {name:'Silk Dress'}) SET p.embedding = ML_EMBED_TEXT(p.description,'sentence-transformers')
MATCH (p:Product {name:'Sport Jacket'}) SET p.embedding = ML_EMBED_TEXT(p.description,'sentence-transformers')
MATCH (a:Product {name:'Silk Dress'}),(b:Product {name:'Sport Jacket'}) RETURN 1 - (a.embedding <=> b.embedding) AS product_similarity
CALL orbit.graphrag.buildKnowledge('fashion_kg','policy_1','Premium customers receive curated recommendations prioritized by similarity and availability.',{source:'policy',domain:'fashion'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('fashion_kg','policy_2','High-value orders require manual review during promotional periods.',{source:'policy',domain:'fashion'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('fashion_kg','What rules apply to premium customers with high-value orders?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
