use crate::error::{MLError, Result};
use crate::neural_networks::optimizers::create_optimizer;
use crate::neural_networks::{ActivationType, NetworkType, NeuralNetwork, NeuralNetworkBuilder};
use crate::neural_networks::{OptimizerConfig, OptimizerType};
use ndarray::Array2;
/// SQL function registry for ML operations
///
/// Provides SQL-callable functions for machine learning operations
/// including model inference, data transformations, and statistical functions.
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

/// Supported ML SQL functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MLFunctionType {
    Predict,
    CreateModel,
    TrainModel,
    SaveModel,
    LoadModel,
}

impl MLFunctionType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ML_PREDICT" => Some(Self::Predict),
            "ML_CREATE_MODEL" => Some(Self::CreateModel),
            "ML_TRAIN" => Some(Self::TrainModel),
            "ML_SAVE_MODEL" => Some(Self::SaveModel),
            "ML_LOAD_MODEL" => Some(Self::LoadModel),
            _ => None,
        }
    }
}

/// Global registry for ML models
/// In a real system, this would be persistent and distributed.
/// For this MVP, it's an in-memory singleton-like structure.
pub struct ModelRegistry {
    models: RwLock<HashMap<String, Arc<RwLock<Box<dyn NeuralNetwork>>>>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: String, model: Box<dyn NeuralNetwork>) {
        let mut models = self.models.write().unwrap();
        models.insert(name, Arc::new(RwLock::new(model)));
    }

    pub fn get(&self, name: &str) -> Option<Arc<RwLock<Box<dyn NeuralNetwork>>>> {
        let models = self.models.read().unwrap();
        models.get(name).cloned()
    }

    // Helper to get read access for prediction
    pub async fn predict(&self, name: &str, input: &Array2<f64>) -> Result<Array2<f64>> {
        let model_arc = self
            .get(name)
            .ok_or_else(|| MLError::not_found(format!("Model '{}' not found", name)))?;
        let model = model_arc
            .read()
            .map_err(|_| MLError::inference("Failed to acquire model lock"))?;
        model.forward(input).await
    }
}

/// SQL function handler
pub struct SqlFunctionHandler {
    registry: Arc<ModelRegistry>,
}

impl SqlFunctionHandler {
    pub fn new(registry: Arc<ModelRegistry>) -> Self {
        Self { registry }
    }

    /// Handle ML_CREATE_MODEL
    /// Syntax: ML_CREATE_MODEL('model_name', 'type', 'input_size', 'output_size', 'hidden_layers')
    /// Example: ML_CREATE_MODEL('my_model', 'feedforward', 10, 2, '64,32')
    pub async fn handle_create_model(&self, args: &[String]) -> Result<String> {
        if args.len() < 4 {
            return Err(MLError::invalid_input(
                "ML_CREATE_MODEL requires at least 4 arguments",
            ));
        }

        let name = args[0].clone();
        let model_type = args[1].to_lowercase();
        let input_size: usize = args[2]
            .parse()
            .map_err(|_| MLError::invalid_input("Invalid input size"))?;
        let output_size: usize = args[3]
            .parse()
            .map_err(|_| MLError::invalid_input("Invalid output size"))?;

        // Parse hidden layers if present
        let hidden_layers: Vec<usize> = if args.len() > 4 {
            args[4]
                .split(',')
                .map(|s| s.trim().parse().unwrap_or(0))
                .filter(|&x| x > 0)
                .collect()
        } else {
            vec![]
        };

        let builder = NeuralNetworkBuilder::new()
            .input_shape(vec![input_size])
            .output_shape(vec![output_size]);

        let builder = match model_type.as_str() {
            "feedforward" | "fnn" => {
                let mut b = builder.network_type(NetworkType::Feedforward);
                for units in hidden_layers {
                    b = b.dense(units, ActivationType::ReLU);
                }
                // Output layer
                b.dense(output_size, ActivationType::Linear)
            }
            "gru" => {
                let mut b = builder.network_type(NetworkType::GRU);
                for units in hidden_layers {
                    b = b.gru(units, ActivationType::Tanh);
                }
                b.dense(output_size, ActivationType::Linear)
            }
            "lstm" => {
                let mut b = builder.network_type(NetworkType::LSTM);
                for units in hidden_layers {
                    b = b.lstm(units, ActivationType::Tanh);
                }
                b.dense(output_size, ActivationType::Linear)
            }
            "rnn" => {
                let mut b = builder.network_type(NetworkType::Recurrent);
                for units in hidden_layers {
                    // Assuming we have a basic RNN layer type or use Dense for simple RNN
                    // The builder has .dense, .gru, .lstm. It doesn't have .rnn() explicit helper yet?
                    // Checking builder... it doesn't have .rnn() helper but has LayerType::RNN.
                    // We should add .rnn() helper or manually add layer.
                    // For now, let's skip or assume generic layer addition.
                    b = b.add_layer(crate::neural_networks::LayerConfig {
                        layer_type: crate::neural_networks::LayerType::RNN,
                        units,
                        activation: ActivationType::Tanh,
                        dropout: None,
                        l1_regularization: None,
                        l2_regularization: None,
                        parameters: HashMap::new(),
                    });
                }
                b.dense(output_size, ActivationType::Linear)
            }
            _ => {
                return Err(MLError::invalid_input(format!(
                    "Unknown model type: {}",
                    model_type
                )))
            }
        };

        let model = builder.build().await?;
        self.registry.register(name.clone(), model);

        Ok(format!("Model '{}' created successfully", name))
    }

    /// Handle ML_PREDICT
    /// Syntax: ML_PREDICT('model_name', 'features_json')
    /// Example: ML_PREDICT('my_model', '[0.1, 0.5, 0.9]')
    pub async fn handle_predict(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(MLError::invalid_input(
                "ML_PREDICT requires at least 2 arguments",
            ));
        }

        let name = args[0].clone();
        let features_str = args[1].clone();

        // Parse features from JSON string
        let features: Vec<f64> = serde_json::from_str(&features_str)
            .map_err(|e| MLError::invalid_input(format!("Invalid features JSON: {}", e)))?;

        // Convert to Array2 (batch size 1)
        let input_len = features.len();
        let input = Array2::from_shape_vec((1, input_len), features).map_err(|e| {
            MLError::invalid_input(format!("Failed to create input tensor: {}", e))
        })?;

        // Run inference
        let output = self.registry.predict(&name, &input).await?;

        // Format output as JSON
        let output_vec: Vec<f64> = output.iter().cloned().collect();
        serde_json::to_string(&output_vec)
            .map_err(|e| MLError::internal(format!("Failed to serialize output: {}", e)))
    }

    /// Handle ML_TRAIN
    /// Syntax: ML_TRAIN('model_name', 'input_json', 'target_json', 'epochs', 'learning_rate')
    /// Example: ML_TRAIN('my_model', '[[0.1, 0.2]]', '[[0.3]]', '10', '0.01')
    pub async fn handle_train_model(&self, args: &[String]) -> Result<String> {
        if args.len() < 3 {
            return Err(MLError::invalid_input(
                "ML_TRAIN requires at least 3 arguments: model_name, input_data, target_data",
            ));
        }

        let name = args[0].clone();
        let input_str = args[1].clone();
        let target_str = args[2].clone();
        let epochs: usize = if args.len() > 3 {
            args[3].parse().unwrap_or(1)
        } else {
            1
        };
        let epochs: usize = if args.len() > 3 {
            args[3].parse().unwrap_or(1)
        } else {
            1
        };

        // Parse optimizer config or learning rate
        let optimizer_config = if args.len() > 4 {
            let arg = &args[4];
            // Try to parse as learning rate (float) first for backward compatibility
            if let Ok(lr) = arg.parse::<f64>() {
                OptimizerConfig {
                    optimizer_type: OptimizerType::SGD,
                    learning_rate: lr,
                    parameters: HashMap::new(),
                }
            } else {
                // Try to parse as JSON config
                serde_json::from_str::<OptimizerConfig>(arg).map_err(|e| {
                    MLError::invalid_input(format!("Invalid optimizer config: {}", e))
                })?
            }
        } else {
            // Default to SGD with lr=0.01
            OptimizerConfig {
                optimizer_type: OptimizerType::SGD,
                learning_rate: 0.01,
                parameters: HashMap::new(),
            }
        };

        // Parse input and target data
        let input_vec: Vec<Vec<f64>> = serde_json::from_str(&input_str)
            .map_err(|e| MLError::invalid_input(format!("Invalid input JSON: {}", e)))?;
        let target_vec: Vec<Vec<f64>> = serde_json::from_str(&target_str)
            .map_err(|e| MLError::invalid_input(format!("Invalid target JSON: {}", e)))?;

        // Convert to Array2
        let rows = input_vec.len();
        let cols = input_vec[0].len();
        let input_flat: Vec<f64> = input_vec.into_iter().flatten().collect();
        let input = Array2::from_shape_vec((rows, cols), input_flat).map_err(|e| {
            MLError::invalid_input(format!("Failed to create input tensor: {}", e))
        })?;

        let t_rows = target_vec.len();
        let t_cols = target_vec[0].len();
        let target_flat: Vec<f64> = target_vec.into_iter().flatten().collect();
        let target = Array2::from_shape_vec((t_rows, t_cols), target_flat).map_err(|e| {
            MLError::invalid_input(format!("Failed to create target tensor: {}", e))
        })?;

        if rows != t_rows {
            return Err(MLError::invalid_input(format!(
                "Input rows ({}) do not match target rows ({})",
                rows, t_rows
            )));
        }

        // Get model and train
        // We need write access to the model registry? No, just the model.
        // Since we use Arc<RwLock> inside the models for mutable state, we can use the shared Arc.
        // But wait, update_weights and backward take &mut self.
        // If ModelRegistry holds Arc<dyn NeuralNetwork>, we can't get &mut self easily if there are other references.
        // However, the NeuralNetwork trait methods backward and update_weights take &mut self.
        // This implies we need exclusive access to the NeuralNetwork struct.
        // But our implementation of GRUNetwork uses Arc<RwLock> for internal state (cache, gradients).
        // Does it? Let's check GRUNetwork definition.
        // struct GRUNetwork { ... cache: Arc<RwLock<GRUCache>>, gradients: Arc<RwLock<Vec<GRULayerGradients>>> ... }
        // So forward takes &self and uses read/write lock on cache.
        // backward takes &mut self.
        // If we have Arc<dyn NeuralNetwork>, we can't call backward(&mut self) unless we have the only Arc or use interior mutability for EVERYTHING.
        // The current design of NeuralNetwork trait requires &mut self for backward/update_weights.
        // This conflicts with Arc<dyn NeuralNetwork> in ModelRegistry if we want to train it.
        // We might need to change NeuralNetwork trait to take &self for backward/update_weights and rely on interior mutability,
        // OR ModelRegistry needs to give us a way to lock the model.
        // Since we are in a single-node MVP, maybe we can rely on the fact that we are the only user during training?
        // But Arc doesn't let us call &mut self methods.
        // We need to fix this.
        // Option 1: Change NeuralNetwork trait to use &self for all methods and use interior mutability (RwLock) for all mutable fields.
        // Option 2: Wrap the whole model in RwLock in the registry: Arc<RwLock<Box<dyn NeuralNetwork>>>.

        // Let's go with Option 2 for now as it's safer for the trait definition.
        // So ModelRegistry should hold Arc<RwLock<dyn NeuralNetwork>>? Or just RwLock<Box<dyn NeuralNetwork>>?
        // The registry itself is RwLock<HashMap...>.
        // If we want to return a reference that can be trained, we need to return something that allows &mut access.
        // If we use Arc<RwLock<dyn NeuralNetwork>>, then we can write() to get &mut.

        // Let's modify ModelRegistry to use Arc<RwLock<dyn NeuralNetwork>>.

        // Wait, I already modified ModelRegistry in the previous chunk to use Arc<dyn NeuralNetwork>.
        // I should probably update that chunk or this one.
        // Since I can't update the previous chunk in this tool call (it's one call), I have to assume the previous chunk is applied.
        // But I can't easily fix it here if the previous chunk makes it Arc<dyn NN>.
        // Actually, I can use get_mut on the registry if I hold the write lock on the registry.
        // But get_mut on HashMap gives &mut Arc<dyn NN>, which doesn't give &mut dyn NN.
        // Arc::get_mut works only if there are no other clones.

        // Okay, the best approach for this MVP where we control the code is to change NeuralNetwork trait to use &self for backward/update_weights
        // AND ensure all implementations use interior mutability.
        // I saw GRUNetwork uses Arc<RwLock> for cache and gradients.
        // But what about weights?
        // GRUNetwork weights are `layers: Vec<GRULayerWeights>`. This is NOT wrapped in RwLock.
        // So `update_weights` needs `&mut self` to modify weights.

        // So we MUST use Arc<RwLock<dyn NeuralNetwork>> in the registry.
        // Let's adjust the ModelRegistry definition in the previous chunk.
        // Oh, I can't edit the previous chunk in the same tool call if I already defined it.
        // I will cancel this tool call and start a new one with the correct ModelRegistry definition.

        // Wait, I can just edit the file again.
        // But better to get it right in one go.
        // I will abort this tool call and restart the thought process to ensure correctness.
        // Actually, I can just proceed and fix it in a subsequent step, but that's messy.
        // I'll correct the replacement chunks in this call.

        // Revised Plan:
        // 1. ModelRegistry: models: RwLock<HashMap<String, Arc<RwLock<dyn NeuralNetwork>>>>
        // 2. register: takes Box<dyn NN>, wraps in Arc::new(RwLock::new(model))
        // 3. get: returns Option<Arc<RwLock<dyn NeuralNetwork>>>
        // 4. predict: gets read lock on model.
        // 5. train: gets write lock on model.

        // This seems robust.

        // Let's update the chunks.

        // Chunk 3 (ModelRegistry):
        // models: RwLock<HashMap<String, Arc<RwLock<dyn NeuralNetwork>>>>
        // register: models.insert(name, Arc::new(RwLock::new(model)));
        // get: returns Option<Arc<RwLock<dyn NeuralNetwork>>>
        // predict: model.read().unwrap().forward(...)

        // Chunk 4 (handle_train_model):
        // let model_arc = self.registry.get(&name)...
        // let mut model = model_arc.write().unwrap();
        // model.forward... model.backward...

        // This looks correct.

        // Let's rewrite the chunks.

        let mut registry_lock = self.registry.models.write().unwrap();
        let model = registry_lock
            .get_mut(&name)
            .ok_or_else(|| MLError::not_found(&name))?;

        let mut optimizer = create_optimizer(&optimizer_config);
        let mut final_loss = 0.0;

        // Get the model from registry
        let model_arc = self
            .registry
            .get(&name)
            .ok_or_else(|| MLError::not_found(&name))?;

        for _ in 0..epochs {
            // Acquire write lock for the whole training step (forward + backward + update)
            // Or maybe just for update?
            // Forward needs read access (if we had read/write split in trait, but we don't).
            // Forward in our impl uses interior mutability for cache, so &self is fine.
            // But backward needs &mut self.
            // So we need write lock.
            let mut model = model_arc
                .write()
                .map_err(|_| MLError::internal("Failed to acquire model lock"))?;

            // Forward pass
            let output = model.forward(&input).await?;

            // Compute Loss (MSE)
            let diff = &output - &target;
            let mse = diff.mapv(|x| x * x).sum() / (rows as f64);
            final_loss = mse;

            let loss_gradient = diff.mapv(|x| 2.0 * x / (rows as f64));

            // Backward pass
            model.backward(&loss_gradient).await?;

            // Update weights
            model.update_weights(optimizer.as_ref()).await?;
        }

        Ok(format!("Training completed. Final Loss: {:.6}", final_loss))
    }

    /// Handle ML_SAVE_MODEL
    /// Syntax: ML_SAVE_MODEL('model_name', 'file_path')
    pub async fn handle_save_model(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(MLError::invalid_input(
                "ML_SAVE_MODEL requires 2 arguments: model_name, file_path",
            ));
        }
        let name = args[0].clone();
        let path = args[1].clone();

        let model_arc = self
            .registry
            .get(&name)
            .ok_or_else(|| MLError::not_found(&name))?;

        let model = model_arc
            .read()
            .map_err(|_| MLError::internal("Failed to acquire model lock"))?;
        let weights = model.save_weights().await?;

        let mut file = File::create(&path)
            .map_err(|e| MLError::internal(format!("Failed to create file: {}", e)))?;
        file.write_all(&weights)
            .map_err(|e| MLError::internal(format!("Failed to write weights: {}", e)))?;

        Ok(format!("Model '{}' saved to '{}'", name, path))
    }

    /// Handle ML_LOAD_MODEL
    /// Syntax: ML_LOAD_MODEL('model_name', 'file_path')
    pub async fn handle_load_model(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(MLError::invalid_input(
                "ML_LOAD_MODEL requires 2 arguments: model_name, file_path",
            ));
        }
        let name = args[0].clone();
        let path = args[1].clone();

        let mut file = File::open(&path)
            .map_err(|e| MLError::internal(format!("Failed to open file: {}", e)))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| MLError::internal(format!("Failed to read file: {}", e)))?;

        let model_arc = self
            .registry
            .get(&name)
            .ok_or_else(|| MLError::not_found(&name))?;

        let mut model = model_arc
            .write()
            .map_err(|_| MLError::internal("Failed to acquire model lock"))?;
        model.load_weights(&buffer).await?;

        Ok(format!("Model '{}' loaded from '{}'", name, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_predict_feedforward() {
        let registry = Arc::new(ModelRegistry::new());
        let handler = SqlFunctionHandler::new(registry.clone());

        // 1. Create Model
        // ML_CREATE_MODEL('test_fnn', 'feedforward', '10', '2', '64,32')
        let create_args = vec![
            "test_fnn".to_string(),
            "feedforward".to_string(),
            "10".to_string(),
            "2".to_string(),
            "64,32".to_string(),
        ];
        let create_result = handler.handle_create_model(&create_args).await;
        assert!(create_result.is_ok());

        // 2. Predict
        // ML_PREDICT('test_fnn', '[0.1, 0.2, ...]')
        let features = vec![0.5; 10]; // 10 features
        let features_json = serde_json::to_string(&features).unwrap();

        let predict_args = vec!["test_fnn".to_string(), features_json];
        let predict_result = handler.handle_predict(&predict_args).await;
        assert!(predict_result.is_ok());

        let output_json = predict_result.unwrap();
        let output: Vec<f64> = serde_json::from_str(&output_json).unwrap();
        assert_eq!(output.len(), 2); // Output size 2
    }

    #[tokio::test]
    async fn test_create_and_predict_gru() {
        let registry = Arc::new(ModelRegistry::new());
        let handler = SqlFunctionHandler::new(registry.clone());

        // 1. Create GRU Model
        // ML_CREATE_MODEL('test_gru', 'gru', '10', '2', '32')
        let create_args = vec![
            "test_gru".to_string(),
            "gru".to_string(),
            "10".to_string(),
            "2".to_string(),
            "32".to_string(),
        ];
        let create_result = handler.handle_create_model(&create_args).await;
        assert!(create_result.is_ok());

        // 2. Predict
        // Note: GRU expects sequence input, but our current ML_PREDICT handles 1D array (batch=1, features=N).
        // Our GRU implementation treats input as [seq_len, features].
        // So if we pass 10 features, it will be treated as seq_len=1, features=10?
        // Or seq_len=10, features=1?
        // Let's check GRU forward: `let seq_len = input.nrows();`
        // If input is [1, 10], seq_len is 1.
        // So it treats it as a sequence of length 1 with 10 features.

        let features = vec![0.5; 10];
        let features_json = serde_json::to_string(&features).unwrap();

        let predict_args = vec!["test_gru".to_string(), features_json];
        let predict_result = handler.handle_predict(&predict_args).await;
        assert!(predict_result.is_ok());
    }

    #[tokio::test]
    async fn test_train_model() {
        let registry = Arc::new(ModelRegistry::new());
        let handler = SqlFunctionHandler::new(registry.clone());

        // 1. Create Model
        let create_args = vec![
            "train_test".to_string(),
            "feedforward".to_string(),
            "2".to_string(),
            "1".to_string(),
            "4".to_string(),
        ];
        handler.handle_create_model(&create_args).await.unwrap();

        // 2. Train Model (XOR-like data)
        // Input: [[0,0], [0,1], [1,0], [1,1]]
        // Target: [[0], [1], [1], [0]]
        let input_json = "[[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]]";
        let target_json = "[[0.0], [1.0], [1.0], [0.0]]";

        let train_args = vec![
            "train_test".to_string(),
            input_json.to_string(),
            target_json.to_string(),
            "10".to_string(),  // epochs
            "0.1".to_string(), // lr
        ];

        let result = handler.handle_train_model(&train_args).await;
        assert!(result.is_ok());
        println!("Train result: {}", result.unwrap());
    }

    #[tokio::test]
    async fn test_train_model_with_optimizer_config() {
        let registry = Arc::new(ModelRegistry::new());
        let handler = SqlFunctionHandler::new(registry.clone());

        // 1. Create Model
        let create_args = vec![
            "adam_test".to_string(),
            "feedforward".to_string(),
            "2".to_string(),
            "1".to_string(),
            "4".to_string(),
        ];
        handler.handle_create_model(&create_args).await.unwrap();

        // 2. Train Model with Adam
        let input_json = "[[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]]";
        let target_json = "[[0.0], [1.0], [1.0], [0.0]]";
        let optimizer_config =
            r#"{"optimizer_type": "adam", "learning_rate": 0.01, "parameters": {"beta1": 0.9}}"#;

        let train_args = vec![
            "adam_test".to_string(),
            input_json.to_string(),
            target_json.to_string(),
            "10".to_string(),
            optimizer_config.to_string(),
        ];

        let result = handler.handle_train_model(&train_args).await;
        assert!(result.is_ok());
        println!("Adam Train result: {}", result.unwrap());
    }

    #[tokio::test]
    async fn test_save_and_load_model() {
        let registry = Arc::new(ModelRegistry::new());
        let handler = SqlFunctionHandler::new(registry.clone());

        // 1. Create Model
        let create_args = vec![
            "save_test".to_string(),
            "feedforward".to_string(),
            "2".to_string(),
            "1".to_string(),
            "4".to_string(),
        ];
        handler.handle_create_model(&create_args).await.unwrap();

        // 2. Save Model
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("save_test.model");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let save_args = vec!["save_test".to_string(), file_path_str.clone()];
        let save_result = handler.handle_save_model(&save_args).await;
        assert!(save_result.is_ok());

        // 3. Create another model (to load into)
        let create_args2 = vec![
            "load_test".to_string(),
            "feedforward".to_string(),
            "2".to_string(),
            "1".to_string(),
            "4".to_string(),
        ];
        handler.handle_create_model(&create_args2).await.unwrap();

        // 4. Load Model
        let load_args = vec!["load_test".to_string(), file_path_str.clone()];
        let load_result = handler.handle_load_model(&load_args).await;
        assert!(load_result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(file_path);
    }
}
