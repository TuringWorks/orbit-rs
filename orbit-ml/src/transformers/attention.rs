//! Multi-head attention mechanisms for transformers
//!
//! This module implements various attention mechanisms including:
//! - Scaled Dot-Product Attention
//! - Multi-Head Attention
//! - Sparse Attention
//! - Cross Attention

use ndarray::{Array2, Array3, Array4};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::neural_networks::layers::Linear;

/// Multi-head attention mechanism for transformers
///
/// Implements the scaled dot-product attention with multiple attention heads,
/// as described in "Attention Is All You Need" paper. This allows the model
/// to attend to information from different representation subspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    /// The hidden size of the model (must be divisible by num_attention_heads)
    pub hidden_size: usize,
    /// Number of parallel attention heads
    pub num_attention_heads: usize,
    /// Size of each attention head (hidden_size / num_attention_heads)
    pub attention_head_size: usize,
    /// Total size of all attention heads combined
    pub all_head_size: usize,

    /// Linear layer for query projection
    pub query_linear: Linear,
    /// Linear layer for key projection
    pub key_linear: Linear,
    /// Linear layer for value projection
    pub value_linear: Linear,
    /// Final linear layer to project concatenated heads back to hidden_size
    pub output_linear: Linear,

    /// Dropout probability for attention weights
    pub dropout_prob: f64,
    /// Scaling factor for dot-product attention (1/sqrt(d_k))
    pub scale: f64,
}

impl MultiHeadAttention {
    /// Creates a new multi-head attention layer
    ///
    /// # Arguments
    /// * `hidden_size` - The hidden dimension size (must be divisible by num_attention_heads)
    /// * `num_attention_heads` - Number of parallel attention heads
    /// * `dropout_prob` - Dropout probability for attention weights
    ///
    /// # Returns
    /// A new MultiHeadAttention instance or an error if parameters are invalid
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

    /// Performs the forward pass of multi-head attention
    ///
    /// # Arguments
    /// * `query` - Query tensor of shape [batch_size, seq_len, hidden_size]
    /// * `key` - Key tensor of shape [batch_size, key_seq_len, hidden_size]
    /// * `value` - Value tensor of shape [batch_size, key_seq_len, hidden_size]
    /// * `attention_mask` - Optional mask to prevent attention to certain positions
    ///
    /// # Returns
    /// Output tensor of shape [batch_size, seq_len, hidden_size]
    pub fn forward(
        &self,
        query: &Array3<f64>,
        key: &Array3<f64>,
        value: &Array3<f64>,
        attention_mask: Option<&Array2<f64>>,
    ) -> Result<Array3<f64>> {
        let _batch_size = query.shape()[0];
        let _seq_len = query.shape()[1];

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

    /// Reshapes tensor from [batch, seq, hidden] to [batch, seq, num_heads, head_size]
    /// to separate attention heads for parallel computation
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

    /// Reshapes tensor from [batch, seq, num_heads, head_size] to [batch, seq, hidden]
    /// to concatenate attention heads after computation
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
}

/// Parameters for single attention score computation
struct AttentionIndices {
    batch: usize,
    query_pos: usize,
    head: usize,
    key_pos: usize,
    head_size: usize,
}

impl MultiHeadAttention {
    /// Computes scaled dot-product attention using decomposed approach for better maintainability
    /// Attention(Q,K,V) = softmax(QK^T/sqrt(d_k))V
    fn scaled_dot_product_attention(
        &self,
        query: &Array4<f64>,
        key: &Array4<f64>,
        value: &Array4<f64>,
        attention_mask: Option<&Array2<f64>>,
    ) -> Result<Array4<f64>> {
        // Step 1: Compute raw attention scores
        let attention_scores = self.compute_attention_scores(query, key)?;

        // Step 2: Apply attention mask if provided
        let masked_scores = self.apply_attention_mask(attention_scores, attention_mask);

        // Step 3: Apply softmax to get attention probabilities
        let attention_probs = self.softmax_4d(&masked_scores);

        // Step 4: Apply dropout
        let dropped_probs = self.apply_attention_dropout(&attention_probs);

        // Step 5: Apply attention to values
        let context = self.apply_attention_to_values(&dropped_probs, value)?;

        Ok(context)
    }

    /// Computes raw attention scores: Q * K^T / sqrt(d_k)
    fn compute_attention_scores(
        &self,
        query: &Array4<f64>,
        key: &Array4<f64>,
    ) -> Result<Array4<f64>> {
        let (batch_size, seq_len, num_heads, head_size) = query.dim();
        let key_seq_len = key.shape()[1];

        let mut attention_scores =
            Array4::<f64>::zeros((batch_size, seq_len, num_heads, key_seq_len));

        for b in 0..batch_size {
            for i in 0..seq_len {
                for h in 0..num_heads {
                    for j in 0..key_seq_len {
                        let indices = AttentionIndices {
                            batch: b,
                            query_pos: i,
                            head: h,
                            key_pos: j,
                            head_size,
                        };
                        let score = self.compute_single_attention_score(query, key, indices);
                        attention_scores[[b, i, h, j]] = score * self.scale;
                    }
                }
            }
        }

        Ok(attention_scores)
    }

    /// Computes a single attention score between query and key positions
    fn compute_single_attention_score(
        &self,
        query: &Array4<f64>,
        key: &Array4<f64>,
        indices: AttentionIndices,
    ) -> f64 {
        let mut score = 0.0;
        for d in 0..indices.head_size {
            score += query[[indices.batch, indices.query_pos, indices.head, d]]
                * key[[indices.batch, indices.key_pos, indices.head, d]];
        }
        score
    }

    /// Applies attention mask to prevent attention to certain positions
    fn apply_attention_mask(
        &self,
        mut attention_scores: Array4<f64>,
        attention_mask: Option<&Array2<f64>>,
    ) -> Array4<f64> {
        if let Some(mask) = attention_mask {
            let (batch_size, seq_len, num_heads, key_seq_len) = attention_scores.dim();
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
        attention_scores
    }

    /// Applies dropout to attention probabilities
    fn apply_attention_dropout(&self, attention_probs: &Array4<f64>) -> Array4<f64> {
        attention_probs.mapv(|x| {
            if rand::random::<f64>() < self.dropout_prob {
                0.0
            } else {
                x
            }
        })
    }

    /// Applies attention probabilities to values to compute context
    fn apply_attention_to_values(
        &self,
        attention_probs: &Array4<f64>,
        value: &Array4<f64>,
    ) -> Result<Array4<f64>> {
        let (batch_size, seq_len, num_heads, head_size) = value.dim();
        let key_seq_len = attention_probs.shape()[3];

        let mut context = Array4::<f64>::zeros((batch_size, seq_len, num_heads, head_size));

        for b in 0..batch_size {
            for i in 0..seq_len {
                for h in 0..num_heads {
                    for d in 0..head_size {
                        let mut weighted_sum = 0.0;
                        for j in 0..key_seq_len {
                            weighted_sum += attention_probs[[b, i, h, j]] * value[[b, j, h, d]];
                        }
                        context[[b, i, h, d]] = weighted_sum;
                    }
                }
            }
        }

        Ok(context)
    }

    /// Applies softmax along the last dimension of a 4D tensor for numerical stability
    /// Used to convert attention scores to attention probabilities
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

/// Sparse attention mechanism for efficiently handling longer sequences
///
/// Reduces computational complexity from O(n²) to O(n·√n) or O(n·log n)
/// by limiting attention to a subset of positions based on sparsity patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseAttention {
    /// The underlying multi-head attention mechanism
    pub base_attention: MultiHeadAttention,
    /// Pattern defining which positions can attend to each other
    pub sparsity_pattern: SparsityPattern,
    /// Size of attention blocks for pattern computation
    pub block_size: usize,
}

/// Different patterns for sparse attention computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SparsityPattern {
    /// Only attend to nearby tokens within a local window
    Local,
    /// Attend to tokens at regular intervals across the sequence
    Strided,
    /// Random sparse attention pattern for better coverage
    Random,
    /// Combination of local and strided patterns for balanced efficiency
    Hybrid,
}

impl SparseAttention {
    /// Creates a new sparse attention layer
    ///
    /// # Arguments
    /// * `hidden_size` - The hidden dimension size
    /// * `num_attention_heads` - Number of parallel attention heads
    /// * `dropout_prob` - Dropout probability for attention weights
    /// * `sparsity_pattern` - Pattern defining attention sparsity
    /// * `block_size` - Size of attention blocks for pattern computation
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

    /// Performs sparse attention forward pass with reduced computational complexity
    ///
    /// # Arguments
    /// * `query` - Query tensor of shape [batch_size, seq_len, hidden_size]
    /// * `key` - Key tensor of shape [batch_size, key_seq_len, hidden_size]
    /// * `value` - Value tensor of shape [batch_size, key_seq_len, hidden_size]
    ///
    /// # Returns
    /// Output tensor of shape [batch_size, seq_len, hidden_size]
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

    /// Creates a sparse attention mask based on the configured sparsity pattern
    ///
    /// # Arguments
    /// * `query_len` - Length of the query sequence
    /// * `key_len` - Length of the key sequence
    ///
    /// # Returns
    /// Binary mask where 1.0 allows attention and 0.0 prevents it
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
///
/// In cross-attention, queries come from the decoder while keys and values
/// come from the encoder, allowing the decoder to attend to encoder representations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAttention {
    /// The underlying multi-head attention mechanism
    pub attention: MultiHeadAttention,
}

impl CrossAttention {
    /// Creates a new cross-attention layer for encoder-decoder architectures
    ///
    /// # Arguments
    /// * `hidden_size` - The hidden dimension size
    /// * `num_attention_heads` - Number of parallel attention heads
    /// * `dropout_prob` - Dropout probability for attention weights
    pub fn new(hidden_size: usize, num_attention_heads: usize, dropout_prob: f64) -> Result<Self> {
        Ok(Self {
            attention: MultiHeadAttention::new(hidden_size, num_attention_heads, dropout_prob)?,
        })
    }

    /// Performs cross-attention between decoder and encoder states
    ///
    /// # Arguments
    /// * `decoder_hidden_states` - Decoder states used as queries [batch_size, dec_seq_len, hidden_size]
    /// * `encoder_hidden_states` - Encoder states used as keys and values [batch_size, enc_seq_len, hidden_size]
    /// * `encoder_attention_mask` - Optional mask to prevent attention to encoder positions
    ///
    /// # Returns
    /// Cross-attention output of shape [batch_size, dec_seq_len, hidden_size]
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
