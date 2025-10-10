//! Multi-head attention mechanisms for transformers
//!
//! This module implements various attention mechanisms including:
//! - Scaled Dot-Product Attention
//! - Multi-Head Attention
//! - Sparse Attention
//! - Cross Attention

use ndarray::{s, Array2, Array3, Array4, Axis};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::neural_networks::layers::Linear;

/// Multi-head attention mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    pub hidden_size: usize,
    pub num_attention_heads: usize,
    pub attention_head_size: usize,
    pub all_head_size: usize,

    pub query_linear: Linear,
    pub key_linear: Linear,
    pub value_linear: Linear,
    pub output_linear: Linear,

    pub dropout_prob: f64,
    pub scale: f64,
}

impl MultiHeadAttention {
    pub fn new(hidden_size: usize, num_attention_heads: usize, dropout_prob: f64) -> Result<Self> {
        assert_eq!(
            hidden_size % num_attention_heads,
            0,
            "Hidden size must be divisible by number of attention heads"
        );

        let attention_head_size = hidden_size / num_attention_heads;
        let all_head_size = num_attention_heads * attention_head_size;
        let scale = 1.0 / (attention_head_size as f64).sqrt();

        Ok(Self {
            hidden_size,
            num_attention_heads,
            attention_head_size,
            all_head_size,
            query_linear: Linear::new(hidden_size, all_head_size)?,
            key_linear: Linear::new(hidden_size, all_head_size)?,
            value_linear: Linear::new(hidden_size, all_head_size)?,
            output_linear: Linear::new(all_head_size, hidden_size)?,
            dropout_prob,
            scale,
        })
    }

    pub fn forward(
        &self,
        query: &Array3<f64>,
        key: &Array3<f64>,
        value: &Array3<f64>,
        attention_mask: Option<&Array2<f64>>,
    ) -> Result<Array3<f64>> {
        let batch_size = query.shape()[0];
        let seq_len = query.shape()[1];

        // Linear projections for Q, K, V
        let q = self.query_linear.forward_3d(query)?;
        let k = self.key_linear.forward_3d(key)?;
        let v = self.value_linear.forward_3d(value)?;

        // Reshape to separate heads: [batch, seq_len, num_heads, head_size]
        let q_heads = self.reshape_for_attention(&q)?;
        let k_heads = self.reshape_for_attention(&k)?;
        let v_heads = self.reshape_for_attention(&v)?;

        // Compute scaled dot-product attention
        let attention_output =
            self.scaled_dot_product_attention(&q_heads, &k_heads, &v_heads, attention_mask)?;

        // Reshape back and apply output projection
        let context = self.reshape_from_attention(&attention_output)?;
        let output = self.output_linear.forward_3d(&context)?;

        Ok(output)
    }

    fn reshape_for_attention(&self, tensor: &Array3<f64>) -> Result<Array4<f64>> {
        let (batch_size, seq_len, _) = tensor.dim();

        // Reshape from [batch, seq, hidden] to [batch, seq, num_heads, head_size]
        let mut reshaped = Array4::<f64>::zeros((
            batch_size,
            seq_len,
            self.num_attention_heads,
            self.attention_head_size,
        ));

        for b in 0..batch_size {
            for s in 0..seq_len {
                for h in 0..self.num_attention_heads {
                    for d in 0..self.attention_head_size {
                        let hidden_idx = h * self.attention_head_size + d;
                        reshaped[[b, s, h, d]] = tensor[[b, s, hidden_idx]];
                    }
                }
            }
        }

        Ok(reshaped)
    }

    fn reshape_from_attention(&self, tensor: &Array4<f64>) -> Result<Array3<f64>> {
        let (batch_size, seq_len, _, _) = tensor.dim();

        // Reshape from [batch, seq, num_heads, head_size] to [batch, seq, hidden]
        let mut reshaped = Array3::<f64>::zeros((batch_size, seq_len, self.all_head_size));

        for b in 0..batch_size {
            for s in 0..seq_len {
                for h in 0..self.num_attention_heads {
                    for d in 0..self.attention_head_size {
                        let hidden_idx = h * self.attention_head_size + d;
                        reshaped[[b, s, hidden_idx]] = tensor[[b, s, h, d]];
                    }
                }
            }
        }

        Ok(reshaped)
    }

    fn scaled_dot_product_attention(
        &self,
        query: &Array4<f64>,
        key: &Array4<f64>,
        value: &Array4<f64>,
        attention_mask: Option<&Array2<f64>>,
    ) -> Result<Array4<f64>> {
        let (batch_size, seq_len, num_heads, head_size) = query.dim();
        let key_seq_len = key.shape()[1];

        // Compute attention scores: Q * K^T / sqrt(d_k)
        let mut attention_scores =
            Array4::<f64>::zeros((batch_size, seq_len, num_heads, key_seq_len));

        for b in 0..batch_size {
            for i in 0..seq_len {
                for h in 0..num_heads {
                    for j in 0..key_seq_len {
                        let mut score = 0.0;
                        for d in 0..head_size {
                            score += query[[b, i, h, d]] * key[[b, j, h, d]];
                        }
                        attention_scores[[b, i, h, j]] = score * self.scale;
                    }
                }
            }
        }

        // Apply attention mask if provided
        if let Some(mask) = attention_mask {
            for b in 0..batch_size {
                for i in 0..seq_len {
                    for h in 0..num_heads {
                        for j in 0..key_seq_len {
                            if mask[[i, j]] == 0.0 {
                                attention_scores[[b, i, h, j]] = f64::NEG_INFINITY;
                            }
                        }
                    }
                }
            }
        }

        // Apply softmax to get attention probabilities
        let attention_probs = self.softmax_4d(&attention_scores);

        // Apply dropout (simplified - in practice would be stochastic)
        let dropped_probs = attention_probs.mapv(|x| {
            if rand::random::<f64>() < self.dropout_prob {
                0.0
            } else {
                x
            }
        });

        // Apply attention to values
        let mut context = Array4::<f64>::zeros((batch_size, seq_len, num_heads, head_size));

        for b in 0..batch_size {
            for i in 0..seq_len {
                for h in 0..num_heads {
                    for d in 0..head_size {
                        let mut weighted_sum = 0.0;
                        for j in 0..key_seq_len {
                            weighted_sum += dropped_probs[[b, i, h, j]] * value[[b, j, h, d]];
                        }
                        context[[b, i, h, d]] = weighted_sum;
                    }
                }
            }
        }

        Ok(context)
    }

    fn softmax_4d(&self, input: &Array4<f64>) -> Array4<f64> {
        let mut output = input.clone();
        let (batch_size, seq_len, num_heads, key_seq_len) = input.dim();

        // Apply softmax along the last dimension (attention over keys)
        for b in 0..batch_size {
            for i in 0..seq_len {
                for h in 0..num_heads {
                    // Find max for numerical stability
                    let mut max_val = f64::NEG_INFINITY;
                    for j in 0..key_seq_len {
                        max_val = max_val.max(input[[b, i, h, j]]);
                    }

                    // Compute exp and sum
                    let mut sum = 0.0;
                    for j in 0..key_seq_len {
                        let exp_val = (input[[b, i, h, j]] - max_val).exp();
                        output[[b, i, h, j]] = exp_val;
                        sum += exp_val;
                    }

                    // Normalize
                    for j in 0..key_seq_len {
                        output[[b, i, h, j]] /= sum;
                    }
                }
            }
        }

        output
    }
}

/// Sparse attention mechanism for handling longer sequences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseAttention {
    pub base_attention: MultiHeadAttention,
    pub sparsity_pattern: SparsityPattern,
    pub block_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SparsityPattern {
    Local,   // Only attend to nearby tokens
    Strided, // Attend to tokens at regular intervals
    Random,  // Random sparse attention pattern
    Hybrid,  // Combination of patterns
}

impl SparseAttention {
    pub fn new(
        hidden_size: usize,
        num_attention_heads: usize,
        dropout_prob: f64,
        sparsity_pattern: SparsityPattern,
        block_size: usize,
    ) -> Result<Self> {
        Ok(Self {
            base_attention: MultiHeadAttention::new(
                hidden_size,
                num_attention_heads,
                dropout_prob,
            )?,
            sparsity_pattern,
            block_size,
        })
    }

    pub fn forward(
        &self,
        query: &Array3<f64>,
        key: &Array3<f64>,
        value: &Array3<f64>,
    ) -> Result<Array3<f64>> {
        // Create sparse attention mask based on pattern
        let attention_mask = self.create_sparse_mask(query.shape()[1], key.shape()[1]);

        self.base_attention
            .forward(query, key, value, Some(&attention_mask))
    }

    fn create_sparse_mask(&self, query_len: usize, key_len: usize) -> Array2<f64> {
        let mut mask = Array2::<f64>::zeros((query_len, key_len));

        match self.sparsity_pattern {
            SparsityPattern::Local => {
                // Local attention - attend to nearby tokens only
                let window_size = self.block_size;
                for i in 0..query_len {
                    let start = i.saturating_sub(window_size / 2);
                    let end = (i + window_size / 2 + 1).min(key_len);
                    for j in start..end {
                        mask[[i, j]] = 1.0;
                    }
                }
            }
            SparsityPattern::Strided => {
                // Strided attention - attend to tokens at regular intervals
                for i in 0..query_len {
                    for j in (0..key_len).step_by(self.block_size) {
                        mask[[i, j]] = 1.0;
                    }
                    // Also attend to local context
                    if i > 0 {
                        mask[[i, i - 1]] = 1.0;
                    }
                    mask[[i, i]] = 1.0;
                    if i + 1 < key_len {
                        mask[[i, i + 1]] = 1.0;
                    }
                }
            }
            SparsityPattern::Random => {
                // Random sparse attention
                let sparsity_ratio = 0.1; // 10% of connections
                let num_connections = (query_len as f64 * key_len as f64 * sparsity_ratio) as usize;

                for _ in 0..num_connections {
                    let i = rand::random::<usize>() % query_len;
                    let j = rand::random::<usize>() % key_len;
                    mask[[i, j]] = 1.0;
                }
            }
            SparsityPattern::Hybrid => {
                // Combination of local and strided patterns
                // Local connections
                for i in 0..query_len {
                    let start = i.saturating_sub(self.block_size / 4);
                    let end = (i + self.block_size / 4 + 1).min(key_len);
                    for j in start..end {
                        mask[[i, j]] = 1.0;
                    }
                }
                // Strided connections
                for i in 0..query_len {
                    for j in (0..key_len).step_by(self.block_size) {
                        mask[[i, j]] = 1.0;
                    }
                }
            }
        }

        mask
    }
}

/// Cross-attention mechanism for encoder-decoder architectures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAttention {
    pub attention: MultiHeadAttention,
}

impl CrossAttention {
    pub fn new(hidden_size: usize, num_attention_heads: usize, dropout_prob: f64) -> Result<Self> {
        Ok(Self {
            attention: MultiHeadAttention::new(hidden_size, num_attention_heads, dropout_prob)?,
        })
    }

    pub fn forward(
        &self,
        decoder_hidden_states: &Array3<f64>,
        encoder_hidden_states: &Array3<f64>,
        encoder_attention_mask: Option<&Array2<f64>>,
    ) -> Result<Array3<f64>> {
        // In cross-attention:
        // - Query comes from decoder
        // - Key and Value come from encoder
        self.attention.forward(
            decoder_hidden_states, // query
            encoder_hidden_states, // key
            encoder_hidden_states, // value
            encoder_attention_mask,
        )
    }
}

use rand;
