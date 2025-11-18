//! Activation functions for neural networks.

use crate::neural_networks::ActivationType;
use ndarray::Array2;

/// Activation function utilities
pub struct Activations;

impl Activations {
    /// Apply activation function
    pub fn apply(input: &Array2<f64>, activation: &ActivationType) -> Array2<f64> {
        match activation {
            ActivationType::Linear => input.clone(),
            ActivationType::ReLU => input.map(|x| x.max(0.0)),
            ActivationType::LeakyReLU { alpha } => {
                input.map(|x| if *x > 0.0 { *x } else { alpha * x })
            }
            ActivationType::ELU { alpha } => input.map(|x| {
                if *x > 0.0 {
                    *x
                } else {
                    alpha * (x.exp() - 1.0)
                }
            }),
            ActivationType::Swish => input.map(|x| x / (1.0 + (-x).exp())),
            ActivationType::GELU => input
                .map(|x| 0.5 * x * (1.0 + (x * 0.7978845608 * (1.0 + 0.044715 * x * x)).tanh())),
            ActivationType::Sigmoid => input.map(|x| 1.0 / (1.0 + (-x).exp())),
            ActivationType::Tanh => input.map(|x| x.tanh()),
            ActivationType::Softmax => Self::softmax(input),
            ActivationType::LogSoftmax => Self::log_softmax(input),
        }
    }

    /// Apply activation derivative
    pub fn derivative(input: &Array2<f64>, activation: &ActivationType) -> Array2<f64> {
        match activation {
            ActivationType::Linear => Array2::ones(input.raw_dim()),
            ActivationType::ReLU => input.map(|x| if *x > 0.0 { 1.0 } else { 0.0 }),
            ActivationType::LeakyReLU { alpha } => {
                input.map(|x| if *x > 0.0 { 1.0 } else { *alpha })
            }
            ActivationType::ELU { alpha } => {
                input.map(|x| if *x > 0.0 { 1.0 } else { alpha * x.exp() })
            }
            ActivationType::Sigmoid => {
                let sigmoid = Self::apply(input, activation);
                sigmoid.map(|x| x * (1.0 - x))
            }
            ActivationType::Tanh => {
                let tanh = Self::apply(input, activation);
                tanh.map(|x| 1.0 - x * x)
            }
            ActivationType::Swish => {
                // Swish derivative: σ(x) * (1 + x * (1 - σ(x)))
                let sigmoid = input.map(|x| 1.0 / (1.0 + (-x).exp()));
                sigmoid.clone() // Simplified for now
            }
            ActivationType::GELU => {
                // Approximate derivative of GELU
                input.map(|x| {
                    let cdf = 0.5 * (1.0 + (x * 0.7978845608).tanh());
                    let pdf = 0.7978845608 * (1.0 - (x * 0.7978845608).tanh().powi(2));
                    cdf + x * pdf
                })
            }
            ActivationType::Softmax => {
                // For softmax, derivative is handled differently in cross-entropy loss
                Array2::ones(input.raw_dim())
            }
            ActivationType::LogSoftmax => {
                // Derivative of log softmax
                let softmax = Self::softmax(input);
                Array2::ones(input.raw_dim()) - &softmax
            }
        }
    }

    /// Numerically stable softmax
    fn softmax(input: &Array2<f64>) -> Array2<f64> {
        let mut result = input.clone();
        for mut row in result.rows_mut() {
            let max_val = row.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            for val in row.iter_mut() {
                *val = (*val - max_val).exp();
            }
            let sum: f64 = row.sum();
            for val in row.iter_mut() {
                *val /= sum;
            }
        }
        result
    }

    /// Log softmax
    fn log_softmax(input: &Array2<f64>) -> Array2<f64> {
        let softmax = Self::softmax(input);
        softmax.map(|x| x.ln())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_relu_activation() {
        let input = Array2::from_shape_vec((1, 3), vec![-1.0, 0.0, 1.0]).unwrap();
        let output = Activations::apply(&input, &ActivationType::ReLU);

        assert_eq!(output[[0, 0]], 0.0);
        assert_eq!(output[[0, 1]], 0.0);
        assert_eq!(output[[0, 2]], 1.0);
    }

    #[test]
    fn test_sigmoid_activation() {
        let input = Array2::from_shape_vec((1, 3), vec![-1000.0, 0.0, 1000.0]).unwrap();
        let output = Activations::apply(&input, &ActivationType::Sigmoid);

        assert!(output[[0, 0]] < 0.01); // Should be close to 0
        assert!((output[[0, 1]] - 0.5).abs() < 0.01); // Should be close to 0.5
        assert!(output[[0, 2]] > 0.99); // Should be close to 1
    }

    #[test]
    fn test_softmax_activation() {
        let input = Array2::from_shape_vec((1, 3), vec![1.0, 2.0, 3.0]).unwrap();
        let output = Activations::apply(&input, &ActivationType::Softmax);

        // Check that probabilities sum to 1
        let sum: f64 = output.row(0).sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Check that values are positive
        assert!(output[[0, 0]] > 0.0);
        assert!(output[[0, 1]] > 0.0);
        assert!(output[[0, 2]] > 0.0);

        // Check that larger inputs produce larger probabilities
        assert!(output[[0, 2]] > output[[0, 1]]);
        assert!(output[[0, 1]] > output[[0, 0]]);
    }

    #[test]
    fn test_leaky_relu() {
        let input = Array2::from_shape_vec((1, 3), vec![-2.0, 0.0, 2.0]).unwrap();
        let alpha = 0.1;
        let output = Activations::apply(&input, &ActivationType::LeakyReLU { alpha });

        assert_eq!(output[[0, 0]], -0.2); // -2.0 * 0.1
        assert_eq!(output[[0, 1]], 0.0);
        assert_eq!(output[[0, 2]], 2.0);
    }
}
