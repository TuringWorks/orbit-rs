use orbit_shared::OrbitResult;
use std::collections::HashMap;
use tokio_test;

// Mock types for GraphML testing
#[derive(Debug, Clone, PartialEq)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct GraphHandle(pub String);

#[derive(Debug, Clone)]
pub struct Vector(pub Vec<f64>);

#[derive(Debug, Clone)]
pub struct EmbeddingModel {
    pub model_type: EmbeddingType,
    pub dimensions: u32,
    pub embeddings: HashMap<NodeId, Vector>,
    pub parameters: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingType {
    Node2Vec { p: f64, q: f64, walk_length: u32, num_walks: u32 },
    GraphSAGE { aggregator: AggregatorType, batch_size: u32, epochs: u32 },
    FastRP { embedding_dim: u32, normalization_strength: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregatorType {
    Mean,
    LSTM,
    Pool,
}

#[derive(Debug, Clone)]
pub struct Node2VecParams {
    pub p: f64,
    pub q: f64,
    pub walk_length: u32,
    pub num_walks: u32,
    pub dimensions: u32,
    pub window_size: u32,
    pub min_count: u32,
    pub learning_rate: f64,
}

#[derive(Debug, Clone)]
pub struct GraphSAGEParams {
    pub aggregator: AggregatorType,
    pub batch_size: u32,
    pub epochs: u32,
    pub learning_rate: f64,
    pub hidden_dim: u32,
    pub num_layers: u32,
}

#[derive(Debug, Clone)]
pub struct FastRPParams {
    pub embedding_dim: u32,
    pub normalization_strength: f64,
    pub iteration_weights: Vec<f64>,
    pub property_ratio: f64,
    pub feature_properties: Vec<String>,
}

// Mock GraphMLActor trait for testing
#[async_trait::async_trait]
pub trait GraphMLActor {
    async fn train_node2vec(&self, graph: GraphHandle, params: Node2VecParams) -> OrbitResult<EmbeddingModel>;
    async fn train_graphsage(&self, graph: GraphHandle, params: GraphSAGEParams) -> OrbitResult<EmbeddingModel>;
    async fn train_fastRP(&self, graph: GraphHandle, params: FastRPParams) -> OrbitResult<EmbeddingModel>;
    async fn get_node_embedding(&self, model: &EmbeddingModel, node_id: NodeId) -> OrbitResult<Option<Vector>>;
    async fn get_similar_nodes(&self, model: &EmbeddingModel, node_id: NodeId, k: u32) -> OrbitResult<Vec<(NodeId, f64)>>;
}

pub struct MockGraphMLActor;

#[async_trait::async_trait]
impl GraphMLActor for MockGraphMLActor {
    async fn train_node2vec(&self, _graph: GraphHandle, params: Node2VecParams) -> OrbitResult<EmbeddingModel> {
        let mut embeddings = HashMap::new();
        
        // Generate mock embeddings
        for i in 1..=10 {
            let node_id = NodeId(format!("node-{}", i));
            let embedding = Vector((0..params.dimensions).map(|_| rand::random::<f64>()).collect());
            embeddings.insert(node_id, embedding);
        }
        
        let mut model_params = HashMap::new();
        model_params.insert("p".to_string(), params.p);
        model_params.insert("q".to_string(), params.q);
        model_params.insert("walk_length".to_string(), params.walk_length as f64);
        model_params.insert("num_walks".to_string(), params.num_walks as f64);
        
        Ok(EmbeddingModel {
            model_type: EmbeddingType::Node2Vec {
                p: params.p,
                q: params.q,
                walk_length: params.walk_length,
                num_walks: params.num_walks,
            },
            dimensions: params.dimensions,
            embeddings,
            parameters: model_params,
        })
    }
    
    async fn train_graphsage(&self, _graph: GraphHandle, params: GraphSAGEParams) -> OrbitResult<EmbeddingModel> {
        let mut embeddings = HashMap::new();
        
        // Generate mock embeddings
        for i in 1..=10 {
            let node_id = NodeId(format!("node-{}", i));
            let embedding = Vector((0..params.hidden_dim).map(|_| rand::random::<f64>()).collect());
            embeddings.insert(node_id, embedding);
        }
        
        let mut model_params = HashMap::new();
        model_params.insert("batch_size".to_string(), params.batch_size as f64);
        model_params.insert("epochs".to_string(), params.epochs as f64);
        model_params.insert("learning_rate".to_string(), params.learning_rate);
        
        Ok(EmbeddingModel {
            model_type: EmbeddingType::GraphSAGE {
                aggregator: params.aggregator,
                batch_size: params.batch_size,
                epochs: params.epochs,
            },
            dimensions: params.hidden_dim,
            embeddings,
            parameters: model_params,
        })
    }
    
    async fn train_fastRP(&self, _graph: GraphHandle, params: FastRPParams) -> OrbitResult<EmbeddingModel> {
        let mut embeddings = HashMap::new();
        
        // Generate mock embeddings
        for i in 1..=10 {
            let node_id = NodeId(format!("node-{}", i));
            let embedding = Vector((0..params.embedding_dim).map(|_| rand::random::<f64>()).collect());
            embeddings.insert(node_id, embedding);
        }
        
        let mut model_params = HashMap::new();
        model_params.insert("embedding_dim".to_string(), params.embedding_dim as f64);
        model_params.insert("normalization_strength".to_string(), params.normalization_strength);
        
        Ok(EmbeddingModel {
            model_type: EmbeddingType::FastRP {
                embedding_dim: params.embedding_dim,
                normalization_strength: params.normalization_strength,
            },
            dimensions: params.embedding_dim,
            embeddings,
            parameters: model_params,
        })
    }
    
    async fn get_node_embedding(&self, model: &EmbeddingModel, node_id: NodeId) -> OrbitResult<Option<Vector>> {
        Ok(model.embeddings.get(&node_id).cloned())
    }
    
    async fn get_similar_nodes(&self, model: &EmbeddingModel, node_id: NodeId, k: u32) -> OrbitResult<Vec<(NodeId, f64)>> {
        if let Some(target_embedding) = model.embeddings.get(&node_id) {
            let mut similarities = Vec::new();
            
            for (other_node_id, other_embedding) in &model.embeddings {
                if *other_node_id != node_id {
                    let similarity = cosine_similarity(&target_embedding.0, &other_embedding.0);
                    similarities.push((other_node_id.clone(), similarity));
                }
            }
            
            // Sort by similarity (descending) and take top k
            similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            similarities.truncate(k as usize);
            
            Ok(similarities)
        } else {
            Ok(Vec::new())
        }
    }
}

// Helper function for cosine similarity
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_node2vec_training_success() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = Node2VecParams {
            p: 1.0,
            q: 1.0,
            walk_length: 80,
            num_walks: 10,
            dimensions: 128,
            window_size: 10,
            min_count: 1,
            learning_rate: 0.01,
        };
        
        // When
        let result = actor.train_node2vec(graph, params.clone()).await;
        
        // Then
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.dimensions, params.dimensions);
        assert!(model.embeddings.len() > 0);
        
        if let EmbeddingType::Node2Vec { p, q, walk_length, num_walks } = model.model_type {
            assert_eq!(p, params.p);
            assert_eq!(q, params.q);
            assert_eq!(walk_length, params.walk_length);
            assert_eq!(num_walks, params.num_walks);
        } else {
            panic!("Expected Node2Vec model type");
        }
    }
    
    #[tokio::test]
    async fn test_graphsage_training_success() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = GraphSAGEParams {
            aggregator: AggregatorType::Mean,
            batch_size: 256,
            epochs: 20,
            learning_rate: 0.01,
            hidden_dim: 128,
            num_layers: 2,
        };
        
        // When
        let result = actor.train_graphsage(graph, params.clone()).await;
        
        // Then
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.dimensions, params.hidden_dim);
        assert!(model.embeddings.len() > 0);
        
        if let EmbeddingType::GraphSAGE { aggregator, batch_size, epochs } = model.model_type {
            assert_eq!(aggregator, params.aggregator);
            assert_eq!(batch_size, params.batch_size);
            assert_eq!(epochs, params.epochs);
        } else {
            panic!("Expected GraphSAGE model type");
        }
    }
    
    #[tokio::test]
    async fn test_fastrp_training_success() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = FastRPParams {
            embedding_dim: 128,
            normalization_strength: 0.75,
            iteration_weights: vec![0.0, 1.0, 1.0, 1.0],
            property_ratio: 0.5,
            feature_properties: vec!["age".to_string(), "category".to_string()],
        };
        
        // When
        let result = actor.train_fastRP(graph, params.clone()).await;
        
        // Then
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.dimensions, params.embedding_dim);
        assert!(model.embeddings.len() > 0);
        
        if let EmbeddingType::FastRP { embedding_dim, normalization_strength } = model.model_type {
            assert_eq!(embedding_dim, params.embedding_dim);
            assert_eq!(normalization_strength, params.normalization_strength);
        } else {
            panic!("Expected FastRP model type");
        }
    }
    
    #[tokio::test]
    async fn test_get_node_embedding_existing() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = Node2VecParams {
            p: 1.0,
            q: 1.0,
            walk_length: 80,
            num_walks: 10,
            dimensions: 128,
            window_size: 10,
            min_count: 1,
            learning_rate: 0.01,
        };
        
        let model = actor.train_node2vec(graph, params).await.unwrap();
        let node_id = NodeId("node-1".to_string());
        
        // When
        let result = actor.get_node_embedding(&model, node_id.clone()).await;
        
        // Then
        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert!(embedding.is_some());
        let embedding = embedding.unwrap();
        assert_eq!(embedding.0.len(), model.dimensions as usize);
    }
    
    #[tokio::test]
    async fn test_get_node_embedding_non_existing() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = Node2VecParams {
            p: 1.0,
            q: 1.0,
            walk_length: 80,
            num_walks: 10,
            dimensions: 128,
            window_size: 10,
            min_count: 1,
            learning_rate: 0.01,
        };
        
        let model = actor.train_node2vec(graph, params).await.unwrap();
        let non_existing_node = NodeId("non-existing-node".to_string());
        
        // When
        let result = actor.get_node_embedding(&model, non_existing_node).await;
        
        // Then
        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert!(embedding.is_none());
    }
    
    #[tokio::test]
    async fn test_get_similar_nodes() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        let params = Node2VecParams {
            p: 1.0,
            q: 1.0,
            walk_length: 80,
            num_walks: 10,
            dimensions: 128,
            window_size: 10,
            min_count: 1,
            learning_rate: 0.01,
        };
        
        let model = actor.train_node2vec(graph, params).await.unwrap();
        let node_id = NodeId("node-1".to_string());
        let k = 3;
        
        // When
        let result = actor.get_similar_nodes(&model, node_id, k).await;
        
        // Then
        assert!(result.is_ok());
        let similar_nodes = result.unwrap();
        assert!(similar_nodes.len() <= k as usize);
        
        // Check that similarities are in descending order
        for i in 1..similar_nodes.len() {
            assert!(similar_nodes[i-1].1 >= similar_nodes[i].1);
        }
        
        // Check that all similarity scores are between -1 and 1
        for (_, similarity) in similar_nodes {
            assert!(similarity >= -1.0 && similarity <= 1.0);
        }
    }
    
    #[tokio::test]
    async fn test_embedding_dimensions_consistency() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        
        // Test different embedding dimensions
        for &dim in &[64, 128, 256] {
            let params = Node2VecParams {
                p: 1.0,
                q: 1.0,
                walk_length: 80,
                num_walks: 10,
                dimensions: dim,
                window_size: 10,
                min_count: 1,
                learning_rate: 0.01,
            };
            
            // When
            let model = actor.train_node2vec(graph.clone(), params).await.unwrap();
            
            // Then
            assert_eq!(model.dimensions, dim);
            for (_, embedding) in model.embeddings {
                assert_eq!(embedding.0.len(), dim as usize);
            }
        }
    }
}

// Property-based tests for embedding algorithms
#[cfg(test)]
#[cfg(feature = "property-tests")]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_node2vec_parameters_validation(
            p in 0.1f64..10.0,
            q in 0.1f64..10.0,
            walk_length in 10u32..200,
            num_walks in 1u32..50,
            dimensions in 16u32..512
        ) {
            tokio_test::block_on(async {
                let actor = MockGraphMLActor;
                let graph = GraphHandle("test-graph".to_string());
                let params = Node2VecParams {
                    p,
                    q,
                    walk_length,
                    num_walks,
                    dimensions,
                    window_size: 10,
                    min_count: 1,
                    learning_rate: 0.01,
                };
                
                let result = actor.train_node2vec(graph, params).await;
                assert!(result.is_ok());
                
                let model = result.unwrap();
                assert_eq!(model.dimensions, dimensions);
            });
        }
        
        #[test]
        fn test_cosine_similarity_properties(
            vec_a in prop::collection::vec(-1.0f64..1.0, 10..100),
            vec_b in prop::collection::vec(-1.0f64..1.0, 10..100)
        ) {
            if vec_a.len() == vec_b.len() {
                let similarity = cosine_similarity(&vec_a, &vec_b);
                
                // Cosine similarity should be between -1 and 1
                prop_assert!(similarity >= -1.0 && similarity <= 1.0);
                
                // Self-similarity should be 1 (for non-zero vectors)
                let norm_a: f64 = vec_a.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm_a > 1e-10 {
                    let self_similarity = cosine_similarity(&vec_a, &vec_a);
                    prop_assert!((self_similarity - 1.0).abs() < 1e-10);
                }
            }
        }
    }
}

// Integration tests with multiple embedding algorithms
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_compare_embedding_algorithms() {
        // Given
        let actor = MockGraphMLActor;
        let graph = GraphHandle("test-graph".to_string());
        
        let node2vec_params = Node2VecParams {
            p: 1.0,
            q: 1.0,
            walk_length: 80,
            num_walks: 10,
            dimensions: 128,
            window_size: 10,
            min_count: 1,
            learning_rate: 0.01,
        };
        
        let graphsage_params = GraphSAGEParams {
            aggregator: AggregatorType::Mean,
            batch_size: 256,
            epochs: 20,
            learning_rate: 0.01,
            hidden_dim: 128,
            num_layers: 2,
        };
        
        let fastrp_params = FastRPParams {
            embedding_dim: 128,
            normalization_strength: 0.75,
            iteration_weights: vec![0.0, 1.0, 1.0, 1.0],
            property_ratio: 0.5,
            feature_properties: vec!["age".to_string()],
        };
        
        // When
        let node2vec_model = actor.train_node2vec(graph.clone(), node2vec_params).await.unwrap();
        let graphsage_model = actor.train_graphsage(graph.clone(), graphsage_params).await.unwrap();
        let fastrp_model = actor.train_fastRP(graph.clone(), fastrp_params).await.unwrap();
        
        // Then
        assert_eq!(node2vec_model.dimensions, 128);
        assert_eq!(graphsage_model.dimensions, 128);
        assert_eq!(fastrp_model.dimensions, 128);
        
        // All models should have embeddings for the same nodes
        let node_id = NodeId("node-1".to_string());
        
        let node2vec_embedding = actor.get_node_embedding(&node2vec_model, node_id.clone()).await.unwrap();
        let graphsage_embedding = actor.get_node_embedding(&graphsage_model, node_id.clone()).await.unwrap();
        let fastrp_embedding = actor.get_node_embedding(&fastrp_model, node_id.clone()).await.unwrap();
        
        assert!(node2vec_embedding.is_some());
        assert!(graphsage_embedding.is_some());
        assert!(fastrp_embedding.is_some());
    }
}