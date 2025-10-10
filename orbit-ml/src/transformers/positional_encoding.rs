//! Positional encoding implementations for transformers
//!
//! This module provides various positional encoding strategies:
//! - Sinusoidal positional encoding (original Transformer)
//! - Learned positional embeddings
//! - Rotary positional encoding (RoPE)
//! - Relative positional encoding

use ndarray::{Array2, Array3};
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Trait for different positional encoding strategies
pub trait PositionalEncoding: Send + Sync + std::fmt::Debug {
    fn encode(&self, sequence_length: usize, hidden_size: usize) -> Result<Array2<f64>>;
    fn encode_batch(
        &self,
        batch_size: usize,
        sequence_length: usize,
        hidden_size: usize,
    ) -> Result<Array3<f64>> {
        let pos_encoding = self.encode(sequence_length, hidden_size)?;
        let mut batch_encoding = Array3::<f64>::zeros((batch_size, sequence_length, hidden_size));

        for b in 0..batch_size {
            for i in 0..sequence_length {
                for j in 0..hidden_size {
                    batch_encoding[[b, i, j]] = pos_encoding[[i, j]];
                }
            }
        }

        Ok(batch_encoding)
    }
}

/// Sinusoidal positional encoding as described in "Attention is All You Need"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinusoidalPositionalEncoding {
    max_length: usize,
    hidden_size: usize,
    cached_encoding: Option<Array2<f64>>,
}

impl SinusoidalPositionalEncoding {
    pub fn new(hidden_size: usize, max_length: usize) -> Self {
        Self {
            max_length,
            hidden_size,
            cached_encoding: None,
        }
    }

    fn compute_encoding(sequence_length: usize, hidden_size: usize) -> Array2<f64> {
        let mut encoding = Array2::<f64>::zeros((sequence_length, hidden_size));

        for pos in 0..sequence_length {
            for i in (0..hidden_size).step_by(2) {
                let div_term = (i as f64 / hidden_size as f64 * 10000_f64.ln()).exp();
                let angle = pos as f64 / div_term;

                encoding[[pos, i]] = angle.sin();
                if i + 1 < hidden_size {
                    encoding[[pos, i + 1]] = angle.cos();
                }
            }
        }

        encoding
    }
}

impl PositionalEncoding for SinusoidalPositionalEncoding {
    fn encode(&self, sequence_length: usize, hidden_size: usize) -> Result<Array2<f64>> {
        // Use cached encoding if available and matches requirements
        if let Some(ref cached) = self.cached_encoding {
            if cached.shape()[0] >= sequence_length && cached.shape()[1] == hidden_size {
                let mut result = Array2::<f64>::zeros((sequence_length, hidden_size));
                for i in 0..sequence_length {
                    for j in 0..hidden_size {
                        result[[i, j]] = cached[[i, j]];
                    }
                }
                return Ok(result);
            }
        }

        // Compute new encoding
        Ok(Self::compute_encoding(sequence_length, hidden_size))
    }
}

/// Learned positional embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPositionalEmbedding {
    embeddings: Array2<f64>,
    max_length: usize,
    hidden_size: usize,
}

impl LearnedPositionalEmbedding {
    pub fn new(max_length: usize, hidden_size: usize) -> Self {
        // Initialize with small random values
        let mut embeddings = Array2::<f64>::zeros((max_length, hidden_size));

        // Xavier/Glorot initialization
        let std = (2.0 / (max_length + hidden_size) as f64).sqrt();
        for i in 0..max_length {
            for j in 0..hidden_size {
                embeddings[[i, j]] = rand::random::<f64>() * std - (std / 2.0);
            }
        }

        Self {
            embeddings,
            max_length,
            hidden_size,
        }
    }

    pub fn update_embedding(&mut self, position: usize, embedding: &Array1<f64>) -> Result<()> {
        if position >= self.max_length {
            return Err(crate::error::MLError::model(format!(
                "Position {} exceeds max length {}",
                position, self.max_length
            )));
        }

        if embedding.len() != self.hidden_size {
            return Err(crate::error::MLError::model(format!(
                "Embedding size {} doesn't match hidden size {}",
                embedding.len(),
                self.hidden_size
            )));
        }

        for j in 0..self.hidden_size {
            self.embeddings[[position, j]] = embedding[j];
        }

        Ok(())
    }
}

impl PositionalEncoding for LearnedPositionalEmbedding {
    fn encode(&self, sequence_length: usize, hidden_size: usize) -> Result<Array2<f64>> {
        if sequence_length > self.max_length {
            return Err(crate::error::MLError::model(format!(
                "Sequence length {} exceeds maximum {}",
                sequence_length, self.max_length
            )));
        }

        if hidden_size != self.hidden_size {
            return Err(crate::error::MLError::model(format!(
                "Hidden size {} doesn't match embedding dimension {}",
                hidden_size, self.hidden_size
            )));
        }

        let mut result = Array2::<f64>::zeros((sequence_length, hidden_size));
        for i in 0..sequence_length {
            for j in 0..hidden_size {
                result[[i, j]] = self.embeddings[[i, j]];
            }
        }
        Ok(result)
    }
}

/// Rotary Positional Encoding (RoPE) as used in RoFormer and modern models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotaryPositionalEncoding {
    hidden_size: usize,
    base: f64,
    max_length: usize,
}

impl RotaryPositionalEncoding {
    pub fn new(hidden_size: usize, max_length: usize, base: f64) -> Self {
        Self {
            hidden_size,
            base,
            max_length,
        }
    }

    /// Apply rotary encoding to query and key tensors
    pub fn apply_rotary_encoding(
        &self,
        tensor: &Array3<f64>,
        positions: &[usize],
    ) -> Result<Array3<f64>> {
        let (batch_size, seq_len, hidden_size) = tensor.dim();
        let mut encoded = tensor.clone();

        // RoPE is applied to pairs of dimensions
        let head_dim = hidden_size;

        for (pos_idx, &pos) in positions.iter().enumerate() {
            if pos_idx >= seq_len {
                break;
            }

            for batch in 0..batch_size {
                for dim in (0..head_dim).step_by(2) {
                    if dim + 1 >= head_dim {
                        break;
                    }

                    // Compute rotation angle
                    let inv_freq = 1.0 / self.base.powf(dim as f64 / head_dim as f64);
                    let angle = pos as f64 * inv_freq;

                    let cos_val = angle.cos();
                    let sin_val = angle.sin();

                    // Apply rotation matrix
                    let x = tensor[[batch, pos_idx, dim]];
                    let y = tensor[[batch, pos_idx, dim + 1]];

                    encoded[[batch, pos_idx, dim]] = x * cos_val - y * sin_val;
                    encoded[[batch, pos_idx, dim + 1]] = x * sin_val + y * cos_val;
                }
            }
        }

        Ok(encoded)
    }
}

impl PositionalEncoding for RotaryPositionalEncoding {
    fn encode(&self, sequence_length: usize, hidden_size: usize) -> Result<Array2<f64>> {
        // RoPE doesn't produce additive encodings, but we can return identity
        // The actual encoding is applied via apply_rotary_encoding
        Ok(Array2::<f64>::eye(sequence_length.min(hidden_size)))
    }
}

/// Relative positional encoding for handling relative positions between tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativePositionalEncoding {
    max_relative_distance: usize,
    hidden_size: usize,
    relative_embeddings: Array2<f64>,
}

impl RelativePositionalEncoding {
    pub fn new(max_relative_distance: usize, hidden_size: usize) -> Self {
        let vocab_size = 2 * max_relative_distance + 1; // -max_dist to +max_dist
        let mut relative_embeddings = Array2::<f64>::zeros((vocab_size, hidden_size));

        // Initialize with small random values
        let std = (2.0 / (vocab_size + hidden_size) as f64).sqrt();
        for i in 0..vocab_size {
            for j in 0..hidden_size {
                relative_embeddings[[i, j]] = rand::random::<f64>() * std - (std / 2.0);
            }
        }

        Self {
            max_relative_distance,
            hidden_size,
            relative_embeddings,
        }
    }

    /// Compute relative position bias for attention
    pub fn compute_relative_bias(
        &self,
        query_length: usize,
        key_length: usize,
    ) -> Result<Array2<f64>> {
        let mut bias = Array2::<f64>::zeros((query_length, key_length));

        for i in 0..query_length {
            for j in 0..key_length {
                let relative_distance = (j as i32 - i as i32).clamp(
                    -(self.max_relative_distance as i32),
                    self.max_relative_distance as i32,
                );
                let bucket_idx = (relative_distance + self.max_relative_distance as i32) as usize;

                // For simplicity, use first dimension of embedding as bias
                // In practice, you'd want a more sophisticated combination
                bias[[i, j]] = self.relative_embeddings[[bucket_idx, 0]];
            }
        }

        Ok(bias)
    }
}

impl PositionalEncoding for RelativePositionalEncoding {
    fn encode(&self, sequence_length: usize, hidden_size: usize) -> Result<Array2<f64>> {
        // Relative encoding doesn't produce absolute position encodings
        // Return zero array since relative positions are handled separately
        Ok(Array2::<f64>::zeros((sequence_length, hidden_size)))
    }
}

/// Alibi (Attention with Linear Biases) positional encoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlibiPositionalEncoding {
    num_heads: usize,
    slopes: Vec<f64>,
}

impl AlibiPositionalEncoding {
    pub fn new(num_heads: usize) -> Self {
        // Compute slopes for each attention head
        let mut slopes = Vec::new();
        let ratio = 2.0_f64.powf(-8.0 / num_heads as f64);

        for i in 0..num_heads {
            let slope = ratio.powi(i as i32);
            slopes.push(slope);
        }

        Self { num_heads, slopes }
    }

    /// Compute Alibi bias matrix for attention
    pub fn compute_alibi_bias(
        &self,
        sequence_length: usize,
        head_idx: usize,
    ) -> Result<Array2<f64>> {
        if head_idx >= self.num_heads {
            return Err(crate::error::MLError::model(format!(
                "Head index {} exceeds number of heads {}",
                head_idx, self.num_heads
            )));
        }

        let slope = self.slopes[head_idx];
        let mut bias = Array2::<f64>::zeros((sequence_length, sequence_length));

        for i in 0..sequence_length {
            for j in 0..sequence_length {
                let distance = (j as i32 - i as i32).abs() as f64;
                bias[[i, j]] = -slope * distance;
            }
        }

        Ok(bias)
    }
}

impl PositionalEncoding for AlibiPositionalEncoding {
    fn encode(&self, sequence_length: usize, _hidden_size: usize) -> Result<Array2<f64>> {
        // Alibi doesn't add to embeddings, it modifies attention directly
        // Return zero array since biases are computed separately
        Ok(Array2::<f64>::zeros((sequence_length, self.num_heads)))
    }
}

use ndarray::Array1;
use rand;
