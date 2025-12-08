SELECT * FROM GRAPHRAG_BUILD('business_kg','doc_apple','Apple Inc. was founded by Steve Jobs in 1976. Steve Jobs co-founded Apple with Steve Wozniak and Ronald Wayne.','{"source":"news"}'::json);
SELECT * FROM GRAPHRAG_BUILD('business_kg','doc_iphone','Apple develops the iPhone. The iPhone is a smartphone product by Apple Inc.','{"source":"catalog"}'::json);
SELECT * FROM GRAPHRAG_QUERY('business_kg','What is the relationship between Apple and Steve Jobs?',2,1024,'ollama',true);
SELECT * FROM GRAPHRAG_REASON('business_kg','Apple Inc.','iPhone',3);
SELECT * FROM GRAPHRAG_STATS('business_kg');
