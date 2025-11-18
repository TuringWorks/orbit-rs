//! ML Function Implementations
//!
//! This module contains implementations of various machine learning functions
//! that can be called from SQL queries.

pub mod statistical;

// Future modules to be implemented
// pub mod supervised;
// pub mod unsupervised;
// pub mod neural;
// pub mod nlp;
// pub mod timeseries;
// pub mod vectors;

// Re-export commonly used functions
pub use statistical::{
    CorrelationFunction, CovarianceFunction, LinearRegressionFunction, ZScoreFunction,
};
