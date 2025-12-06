SELECT * FROM GRAPHRAG_ENTITIES('research_kg');
SELECT * FROM GRAPHRAG_SIMILAR('research_kg','Transformers',10,0.8);
SELECT * FROM GRAPHRAG_QUERY('research_kg','Find entities related to Graph Neural Networks',2,2048,'ollama',true);
