SELECT * FROM GRAPHRAG_BUILD('business_kg','doc_b1','Apple Inc. acquired NeXT. Steve Jobs returned to Apple after the acquisition.','{"source":"news","category":"business"}'::json);
SELECT * FROM GRAPHRAG_BUILD('business_kg','doc_b2','Apple produces the iPhone and iPad.','{"source":"catalog","category":"products"}'::json);
SELECT * FROM GRAPHRAG_BUILD('research_kg','doc_r1','Transformers rely on attention mechanisms.','{"source":"paper","category":"ml"}'::json);
SELECT * FROM GRAPHRAG_BUILD('research_kg','doc_r2','Graph Neural Networks use message passing between nodes.','{"source":"wiki","category":"ml"}'::json);
SELECT * FROM GRAPHRAG_QUERY('business_kg','Summarize Apple product relationships.',2,2048,'ollama',true);
SELECT * FROM GRAPHRAG_QUERY('research_kg','Explain attention vs message passing.',3,4096,'ollama',true);
SELECT * FROM GRAPHRAG_REASON('business_kg','Apple Inc.','iPhone',3);
SELECT * FROM GRAPHRAG_REASON('research_kg','Transformers','Graph Neural Networks',4);
