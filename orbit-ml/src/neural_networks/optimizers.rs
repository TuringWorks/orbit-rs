//! Optimizer implementations for neural networks.

use async_trait::async_trait;
use ndarray::Array2;

use crate::error::Result;

/// Generic optimizer trait
#[async_trait]
pub trait Optimizer: Send + Sync {
    /// Update parameters using gradients
    async fn update(
        &mut self,
        parameters: &mut [Array2<f64>],
        gradients: &[Array2<f64>],
    ) -> Result<()>;

    /// Reset optimizer state
    fn reset(&mut self);

    /// Get learning rate
    fn learning_rate(&self) -> f64;

    /// Set learning rate
    fn set_learning_rate(&mut self, lr: f64);
}

/// Stochastic Gradient Descent optimizer
#[derive(Debug, Clone)]
pub struct SGDOptimizer {
    learning_rate: f64,
    momentum: f64,
    velocity: Vec<Array2<f64>>,
}

impl SGDOptimizer {
    /// Create a new SGD optimizer
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate for gradient descent (typically 0.01-0.1)
    /// * `momentum` - Momentum factor for acceleration (0.0 for no momentum, typically 0.9)
    ///
    /// # Returns
    /// A new SGD optimizer instance
    pub fn new(learning_rate: f64, momentum: f64) -> Self {
        Self {
            learning_rate,
            momentum,
            velocity: Vec::new(),
        }
    }

    fn ensure_velocity_size(&mut self, size: usize) {
        while self.velocity.len() < size {
            self.velocity.push(Array2::zeros((1, 1)));
        }
    }
}

#[async_trait]
impl Optimizer for SGDOptimizer {
    async fn update(
        &mut self,
        parameters: &mut [Array2<f64>],
        gradients: &[Array2<f64>],
    ) -> Result<()> {
        self.ensure_velocity_size(parameters.len());

        for (i, (param, grad)) in parameters.iter_mut().zip(gradients.iter()).enumerate() {
            // Resize velocity if needed
            if self.velocity[i].shape() != param.shape() {
                self.velocity[i] = Array2::zeros(param.raw_dim());
            }

            if self.momentum > 0.0 {
                // Momentum SGD: v = momentum * v - lr * grad
                self.velocity[i] = &self.velocity[i] * self.momentum - grad * self.learning_rate;
                *param += &self.velocity[i];
            } else {
                // Standard SGD: param = param - lr * grad
                *param -= &(grad * self.learning_rate);
            }
        }

        Ok(())
    }

    fn reset(&mut self) {
        for v in &mut self.velocity {
            v.fill(0.0);
        }
    }

    fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }
}

/// Adam optimizer
#[derive(Debug, Clone)]
pub struct AdamOptimizer {
    learning_rate: f64,
    beta1: f64,
    beta2: f64,
    epsilon: f64,
    t: usize,            // Time step
    m: Vec<Array2<f64>>, // First moment
    v: Vec<Array2<f64>>, // Second moment
}

impl AdamOptimizer {
    /// Create a new Adam optimizer with custom parameters
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate (typically 0.001)
    /// * `beta1` - Exponential decay rate for first moment (typically 0.9)
    /// * `beta2` - Exponential decay rate for second moment (typically 0.999)
    /// * `epsilon` - Small constant for numerical stability (typically 1e-8)
    ///
    /// # Returns
    /// A new Adam optimizer instance
    pub fn new(learning_rate: f64, beta1: f64, beta2: f64, epsilon: f64) -> Self {
        Self {
            learning_rate,
            beta1,
            beta2,
            epsilon,
            t: 0,
            m: Vec::new(),
            v: Vec::new(),
        }
    }

    /// Create an Adam optimizer with default parameters
    ///
    /// Uses standard Adam hyperparameters:
    /// - learning_rate = 0.001
    /// - beta1 = 0.9  
    /// - beta2 = 0.999
    /// - epsilon = 1e-8
    ///
    /// # Returns
    /// A new Adam optimizer with default configuration
    pub fn with_defaults() -> Self {
        Self::new(0.001, 0.9, 0.999, 1e-8)
    }

    fn ensure_moments_size(&mut self, size: usize) {
        while self.m.len() < size {
            self.m.push(Array2::zeros((1, 1)));
            self.v.push(Array2::zeros((1, 1)));
        }
    }
}

impl Default for AdamOptimizer {
    fn default() -> Self {
        Self::new(0.001, 0.9, 0.999, 1e-8)
    }
}

#[async_trait]
impl Optimizer for AdamOptimizer {
    async fn update(
        &mut self,
        parameters: &mut [Array2<f64>],
        gradients: &[Array2<f64>],
    ) -> Result<()> {
        self.ensure_moments_size(parameters.len());
        self.t += 1;

        for (i, (param, grad)) in parameters.iter_mut().zip(gradients.iter()).enumerate() {
            // Resize moments if needed
            if self.m[i].shape() != param.shape() {
                self.m[i] = Array2::zeros(param.raw_dim());
                self.v[i] = Array2::zeros(param.raw_dim());
            }

            // Update biased first moment estimate
            self.m[i] = &self.m[i] * self.beta1 + grad * (1.0 - self.beta1);

            // Update biased second raw moment estimate
            self.v[i] = &self.v[i] * self.beta2 + &grad.map(|x| x * x) * (1.0 - self.beta2);

            // Compute bias-corrected first moment estimate
            let m_hat = &self.m[i] / (1.0 - self.beta1.powi(self.t as i32));

            // Compute bias-corrected second raw moment estimate
            let v_hat = &self.v[i] / (1.0 - self.beta2.powi(self.t as i32));

            // Update parameters
            let update = &m_hat / &(v_hat.map(|x| x.sqrt()) + self.epsilon);
            *param -= &(&update * self.learning_rate);
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.t = 0;
        for m in &mut self.m {
            m.fill(0.0);
        }
        for v in &mut self.v {
            v.fill(0.0);
        }
    }

    fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }
}

/// AdaGrad optimizer
#[derive(Debug, Clone)]
pub struct AdaGradOptimizer {
    learning_rate: f64,
    epsilon: f64,
    sum_of_squares: Vec<Array2<f64>>,
}

impl AdaGradOptimizer {
    /// Create a new AdaGrad optimizer
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate (typically 0.01)
    /// * `epsilon` - Small constant for numerical stability (typically 1e-8)
    ///
    /// # Returns
    /// A new AdaGrad optimizer instance
    pub fn new(learning_rate: f64, epsilon: f64) -> Self {
        Self {
            learning_rate,
            epsilon,
            sum_of_squares: Vec::new(),
        }
    }

    fn ensure_sum_of_squares_size(&mut self, size: usize) {
        while self.sum_of_squares.len() < size {
            self.sum_of_squares.push(Array2::zeros((1, 1)));
        }
    }
}

#[async_trait]
impl Optimizer for AdaGradOptimizer {
    async fn update(
        &mut self,
        parameters: &mut [Array2<f64>],
        gradients: &[Array2<f64>],
    ) -> Result<()> {
        self.ensure_sum_of_squares_size(parameters.len());

        for (i, (param, grad)) in parameters.iter_mut().zip(gradients.iter()).enumerate() {
            // Resize sum of squares if needed
            if self.sum_of_squares[i].shape() != param.shape() {
                self.sum_of_squares[i] = Array2::zeros(param.raw_dim());
            }

            // Accumulate squared gradients
            self.sum_of_squares[i] += &grad.map(|x| x * x);

            // Update parameters
            let denominator = self.sum_of_squares[i].map(|x| x.sqrt()) + self.epsilon;
            let update = grad / &denominator * self.learning_rate;
            *param -= &update;
        }

        Ok(())
    }

    fn reset(&mut self) {
        for sos in &mut self.sum_of_squares {
            sos.fill(0.0);
        }
    }

    fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }
}

/// RMSprop optimizer
#[derive(Debug, Clone)]
pub struct RMSpropOptimizer {
    learning_rate: f64,
    alpha: f64,
    epsilon: f64,
    squared_avg: Vec<Array2<f64>>,
}

impl RMSpropOptimizer {
    /// Create a new RMSprop optimizer
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate (typically 0.001)
    /// * `alpha` - Decay factor for moving average (typically 0.99)
    /// * `epsilon` - Small constant for numerical stability (typically 1e-8)
    ///
    /// # Returns
    /// A new RMSprop optimizer instance
    pub fn new(learning_rate: f64, alpha: f64, epsilon: f64) -> Self {
        Self {
            learning_rate,
            alpha,
            epsilon,
            squared_avg: Vec::new(),
        }
    }

    fn ensure_squared_avg_size(&mut self, size: usize) {
        while self.squared_avg.len() < size {
            self.squared_avg.push(Array2::zeros((1, 1)));
        }
    }
}

#[async_trait]
impl Optimizer for RMSpropOptimizer {
    async fn update(
        &mut self,
        parameters: &mut [Array2<f64>],
        gradients: &[Array2<f64>],
    ) -> Result<()> {
        self.ensure_squared_avg_size(parameters.len());

        for (i, (param, grad)) in parameters.iter_mut().zip(gradients.iter()).enumerate() {
            // Resize squared average if needed
            if self.squared_avg[i].shape() != param.shape() {
                self.squared_avg[i] = Array2::zeros(param.raw_dim());
            }

            // Update exponential moving average of squared gradients
            self.squared_avg[i] =
                &self.squared_avg[i] * self.alpha + &grad.map(|x| x * x) * (1.0 - self.alpha);

            // Update parameters
            let denominator = self.squared_avg[i].map(|x| x.sqrt()) + self.epsilon;
            let update = grad / &denominator * self.learning_rate;
            *param -= &update;
        }

        Ok(())
    }

    fn reset(&mut self) {
        for sa in &mut self.squared_avg {
            sa.fill(0.0);
        }
    }

    fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[tokio::test]
    async fn test_sgd_optimizer() {
        let mut optimizer = SGDOptimizer::new(0.1, 0.0);
        let mut params = vec![Array2::ones((2, 2))];
        let gradients = vec![Array2::ones((2, 2))];

        optimizer.update(&mut params, &gradients).await.unwrap();

        // Parameters should be updated: 1.0 - 0.1 * 1.0 = 0.9
        assert_eq!(params[0][[0, 0]], 0.9);
    }

    #[tokio::test]
    async fn test_adam_optimizer() {
        let mut optimizer = AdamOptimizer::default();
        let mut params = vec![Array2::ones((2, 2))];
        let gradients = vec![Array2::ones((2, 2))];

        optimizer.update(&mut params, &gradients).await.unwrap();

        // Parameters should be updated (exact value depends on Adam's internal calculations)
        assert_ne!(params[0][[0, 0]], 1.0);
        assert!(params[0][[0, 0]] < 1.0); // Should decrease with positive gradient
    }

    #[test]
    fn test_optimizer_learning_rate() {
        let mut optimizer = SGDOptimizer::new(0.01, 0.9);
        assert_eq!(optimizer.learning_rate(), 0.01);

        optimizer.set_learning_rate(0.02);
        assert_eq!(optimizer.learning_rate(), 0.02);
    }

    #[test]
    fn test_optimizer_reset() {
        let mut optimizer = AdamOptimizer::default();
        optimizer.t = 10;
        optimizer.reset();
        assert_eq!(optimizer.t, 0);
    }
}
