//! Statistical ML Functions
//!
//! Provides fundamental statistical functions for machine learning operations
//! including correlation, regression, and descriptive statistics.

use crate::ml::{FeatureType, FunctionSignature, MLFunction, MLResult, MLValue, ParameterSpec};
use async_trait::async_trait;

/// Linear regression function
pub struct LinearRegressionFunction;

#[async_trait]
impl MLFunction for LinearRegressionFunction {
    async fn execute(&self, args: Vec<MLValue>) -> MLResult<MLValue> {
        if args.len() != 2 {
            return Err(orbit_shared::OrbitError::internal(
                "ML_LINEAR_REGRESSION requires exactly 2 arguments: features, target".to_string(),
            ));
        }

        let features = match &args[0] {
            MLValue::Array(arr) => {
                let mut feature_matrix = Vec::new();
                for row in arr {
                    if let MLValue::Vector(vec) = row {
                        feature_matrix.push(vec.clone());
                    } else {
                        return Err(orbit_shared::OrbitError::internal(
                            "Features must be an array of vectors".to_string(),
                        ));
                    }
                }
                feature_matrix
            }
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "Features must be an array of vectors".to_string(),
                ))
            }
        };

        let targets = match &args[1] {
            MLValue::Vector(vec) => vec.clone(),
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "Targets must be a vector".to_string(),
                ))
            }
        };

        // Perform linear regression using normal equation: β = (X^T X)^(-1) X^T y
        let coefficients = compute_linear_regression(&features, &targets)?;

        Ok(MLValue::Vector(coefficients))
    }

    fn signature(&self) -> FunctionSignature {
        FunctionSignature {
            name: "ML_LINEAR_REGRESSION".to_string(),
            parameters: vec![
                ParameterSpec {
                    name: "features".to_string(),
                    param_type: FeatureType::Vector(0), // Variable dimension
                    required: true,
                    default: None,
                },
                ParameterSpec {
                    name: "target".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
            ],
            return_type: FeatureType::Vector(0), // Coefficients vector
            description: "Performs linear regression and returns coefficients".to_string(),
        }
    }
}

/// Correlation function
pub struct CorrelationFunction;

#[async_trait]
impl MLFunction for CorrelationFunction {
    async fn execute(&self, args: Vec<MLValue>) -> MLResult<MLValue> {
        if args.len() != 2 {
            return Err(orbit_shared::OrbitError::internal(
                "ML_CORRELATION requires exactly 2 arguments: x, y".to_string(),
            ));
        }

        let x = match &args[0] {
            MLValue::Vector(vec) => vec.clone(),
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "First argument must be a vector".to_string(),
                ))
            }
        };

        let y = match &args[1] {
            MLValue::Vector(vec) => vec.clone(),
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "Second argument must be a vector".to_string(),
                ))
            }
        };

        if x.len() != y.len() {
            return Err(orbit_shared::OrbitError::internal(
                "Input vectors must have the same length".to_string(),
            ));
        }

        let correlation = compute_correlation(&x, &y)?;
        Ok(MLValue::Float(correlation))
    }

    fn signature(&self) -> FunctionSignature {
        FunctionSignature {
            name: "ML_CORRELATION".to_string(),
            parameters: vec![
                ParameterSpec {
                    name: "x".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
                ParameterSpec {
                    name: "y".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
            ],
            return_type: FeatureType::Float64,
            description: "Computes Pearson correlation coefficient between two vectors".to_string(),
        }
    }
}

/// Z-score normalization function
pub struct ZScoreFunction;

#[async_trait]
impl MLFunction for ZScoreFunction {
    async fn execute(&self, args: Vec<MLValue>) -> MLResult<MLValue> {
        if args.is_empty() || args.len() > 3 {
            return Err(orbit_shared::OrbitError::internal(
                "ML_ZSCORE requires 1-3 arguments: values, [mean], [std]".to_string(),
            ));
        }

        let values = match &args[0] {
            MLValue::Vector(vec) => vec.clone(),
            MLValue::Float(f) => vec![*f],
            MLValue::Integer(i) => vec![*i as f64],
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "First argument must be a number or vector".to_string(),
                ))
            }
        };

        let (mean, std) = if args.len() == 3 {
            // Use provided mean and std
            let mean = match &args[1] {
                MLValue::Float(f) => *f,
                MLValue::Integer(i) => *i as f64,
                _ => {
                    return Err(orbit_shared::OrbitError::internal(
                        "Mean must be a number".to_string(),
                    ))
                }
            };
            let std = match &args[2] {
                MLValue::Float(f) => *f,
                MLValue::Integer(i) => *i as f64,
                _ => {
                    return Err(orbit_shared::OrbitError::internal(
                        "Standard deviation must be a number".to_string(),
                    ))
                }
            };
            (mean, std)
        } else {
            // Calculate mean and std from values
            let (mean, std, _, _) = crate::ml::utils::calculate_stats(&values);
            (mean, std)
        };

        if std == 0.0 {
            return Err(orbit_shared::OrbitError::internal(
                "Standard deviation cannot be zero".to_string(),
            ));
        }

        let z_scores: Vec<f64> = values.iter().map(|x| (x - mean) / std).collect();

        if z_scores.len() == 1 {
            Ok(MLValue::Float(z_scores[0]))
        } else {
            Ok(MLValue::Vector(z_scores))
        }
    }

    fn signature(&self) -> FunctionSignature {
        FunctionSignature {
            name: "ML_ZSCORE".to_string(),
            parameters: vec![
                ParameterSpec {
                    name: "values".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
                ParameterSpec {
                    name: "mean".to_string(),
                    param_type: FeatureType::Float64,
                    required: false,
                    default: Some("calculated".to_string()),
                },
                ParameterSpec {
                    name: "std".to_string(),
                    param_type: FeatureType::Float64,
                    required: false,
                    default: Some("calculated".to_string()),
                },
            ],
            return_type: FeatureType::Vector(0),
            description: "Computes Z-score normalization for values".to_string(),
        }
    }
}

/// Covariance function
pub struct CovarianceFunction;

#[async_trait]
impl MLFunction for CovarianceFunction {
    async fn execute(&self, args: Vec<MLValue>) -> MLResult<MLValue> {
        if args.len() != 2 {
            return Err(orbit_shared::OrbitError::internal(
                "ML_COVARIANCE requires exactly 2 arguments: x, y".to_string(),
            ));
        }

        let x = match &args[0] {
            MLValue::Vector(vec) => vec.clone(),
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "First argument must be a vector".to_string(),
                ))
            }
        };

        let y = match &args[1] {
            MLValue::Vector(vec) => vec.clone(),
            _ => {
                return Err(orbit_shared::OrbitError::internal(
                    "Second argument must be a vector".to_string(),
                ))
            }
        };

        if x.len() != y.len() {
            return Err(orbit_shared::OrbitError::internal(
                "Input vectors must have the same length".to_string(),
            ));
        }

        let covariance = compute_covariance(&x, &y)?;
        Ok(MLValue::Float(covariance))
    }

    fn signature(&self) -> FunctionSignature {
        FunctionSignature {
            name: "ML_COVARIANCE".to_string(),
            parameters: vec![
                ParameterSpec {
                    name: "x".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
                ParameterSpec {
                    name: "y".to_string(),
                    param_type: FeatureType::Vector(0),
                    required: true,
                    default: None,
                },
            ],
            return_type: FeatureType::Float64,
            description: "Computes covariance between two vectors".to_string(),
        }
    }
}

// Helper functions for statistical computations

/// Compute linear regression coefficients using normal equation
fn compute_linear_regression(features: &[Vec<f64>], targets: &[f64]) -> MLResult<Vec<f64>> {
    if features.is_empty() || targets.is_empty() {
        return Err(orbit_shared::OrbitError::internal(
            "Features and targets cannot be empty".to_string(),
        ));
    }

    if features.len() != targets.len() {
        return Err(orbit_shared::OrbitError::internal(
            "Number of feature vectors must equal number of targets".to_string(),
        ));
    }

    let n = features.len();
    let p = features[0].len() + 1; // +1 for intercept

    // Create design matrix X with intercept column
    let mut x_matrix = vec![vec![0.0; p]; n];
    for i in 0..n {
        x_matrix[i][0] = 1.0; // Intercept term
        for j in 0..features[i].len() {
            x_matrix[i][j + 1] = features[i][j];
        }
    }

    // Compute X^T X
    let mut xtx = vec![vec![0.0; p]; p];
    for (i, xtx_row) in xtx.iter_mut().enumerate().take(p) {
        for (j, xtx_val) in xtx_row.iter_mut().enumerate().take(p) {
            for x_row in x_matrix.iter().take(n) {
                *xtx_val += x_row[i] * x_row[j];
            }
        }
    }

    // Compute X^T y
    let mut xty = vec![0.0; p];
    for (i, xty_val) in xty.iter_mut().enumerate().take(p) {
        for (x_row, &target) in x_matrix.iter().zip(targets.iter()).take(n) {
            *xty_val += x_row[i] * target;
        }
    }

    // Solve (X^T X) β = X^T y using simple Gaussian elimination
    let coefficients = solve_linear_system(xtx, xty)?;
    Ok(coefficients)
}

/// Solve linear system Ax = b using Gaussian elimination
#[allow(clippy::needless_range_loop)]
fn solve_linear_system(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> MLResult<Vec<f64>> {
    let n = a.len();

    // Forward elimination
    for i in 0..n {
        // Find pivot
        let mut max_row = i;
        for k in (i + 1)..n {
            if a[k][i].abs() > a[max_row][i].abs() {
                max_row = k;
            }
        }

        // Swap rows
        a.swap(i, max_row);
        b.swap(i, max_row);

        // Check for singular matrix
        if a[i][i].abs() < 1e-12 {
            return Err(orbit_shared::OrbitError::internal(
                "Matrix is singular or nearly singular".to_string(),
            ));
        }

        // Eliminate column
        for k in (i + 1)..n {
            let factor = a[k][i] / a[i][i];
            for j in i..n {
                a[k][j] -= factor * a[i][j];
            }
            b[k] -= factor * b[i];
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = b[i];
        for j in (i + 1)..n {
            x[i] -= a[i][j] * x[j];
        }
        x[i] /= a[i][i];
    }

    Ok(x)
}

/// Compute Pearson correlation coefficient
fn compute_correlation(x: &[f64], y: &[f64]) -> MLResult<f64> {
    if x.len() < 2 {
        return Err(orbit_shared::OrbitError::internal(
            "Need at least 2 data points for correlation".to_string(),
        ));
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        num += dx * dy;
        den_x += dx * dx;
        den_y += dy * dy;
    }

    if den_x == 0.0 || den_y == 0.0 {
        return Ok(0.0); // No correlation if either variable is constant
    }

    Ok(num / (den_x * den_y).sqrt())
}

/// Compute covariance between two vectors
fn compute_covariance(x: &[f64], y: &[f64]) -> MLResult<f64> {
    if x.len() < 2 {
        return Err(orbit_shared::OrbitError::internal(
            "Need at least 2 data points for covariance".to_string(),
        ));
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let covariance = x
        .iter()
        .zip(y.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>()
        / (n - 1.0); // Sample covariance

    Ok(covariance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_function() {
        let func = CorrelationFunction;
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];

        let args = vec![MLValue::Vector(x), MLValue::Vector(y)];

        let result = func.execute(args).await.unwrap();
        match result {
            MLValue::Float(corr) => {
                assert!((corr - 1.0).abs() < 1e-10); // Perfect positive correlation
            }
            _ => panic!("Expected float result"),
        }
    }

    #[tokio::test]
    async fn test_zscore_function() {
        let func = ZScoreFunction;
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let args = vec![MLValue::Vector(values)];

        let result = func.execute(args).await.unwrap();
        match result {
            MLValue::Vector(z_scores) => {
                assert_eq!(z_scores.len(), 5);
                // Mean should be approximately 0
                let mean_z = z_scores.iter().sum::<f64>() / z_scores.len() as f64;
                assert!(mean_z.abs() < 1e-10);
            }
            _ => panic!("Expected vector result"),
        }
    }

    #[test]
    fn test_linear_regression_simple() {
        // Simple case: y = 2x + 1
        let features = vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0], vec![5.0]];
        let targets = vec![3.0, 5.0, 7.0, 9.0, 11.0];

        let coeffs = compute_linear_regression(&features, &targets).unwrap();
        assert_eq!(coeffs.len(), 2); // Intercept + 1 feature

        // Should be approximately [1.0, 2.0] (intercept, slope)
        assert!((coeffs[0] - 1.0).abs() < 1e-10);
        assert!((coeffs[1] - 2.0).abs() < 1e-10);
    }
}
