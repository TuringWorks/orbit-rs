//! Data loading infrastructure for ML training

use ndarray::{Array2, Array4};
use crate::error::Result;

/// Generic data loader trait
pub trait DataLoader: Send + Sync {
    type Item;
    
    /// Load next batch of data
    fn next_batch(&mut self) -> Result<Option<Vec<Self::Item>>>;
    
    /// Get total number of samples
    fn len(&self) -> usize;
    
    /// Check if loader is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Reset loader to beginning
    fn reset(&mut self);
}

/// Image data loader with preprocessing
pub struct ImageDataLoader {
    image_paths: Vec<String>,
    labels: Vec<usize>,
    batch_size: usize,
    current_idx: usize,
    shuffle: bool,
    augment: bool,
}

impl ImageDataLoader {
    pub fn new(
        image_paths: Vec<String>,
        labels: Vec<usize>,
        batch_size: usize,
        shuffle: bool,
        augment: bool,
    ) -> Self {
        Self {
            image_paths,
            labels,
            batch_size,
            current_idx: 0,
            shuffle,
            augment,
        }
    }

    /// Load and preprocess a single image
    fn load_image(&self, _path: &str) -> Result<Array4<f32>> {
        // In production, use image crate to load actual images
        // For now, return dummy data
        Ok(Array4::zeros((1, 3, 224, 224)))
    }

    /// Apply data augmentation
    fn augment_image(&self, image: Array4<f32>) -> Array4<f32> {
        // Random horizontal flip, rotation, color jitter, etc.
        image
    }
}

impl DataLoader for ImageDataLoader {
    type Item = (Array4<f32>, usize);

    fn next_batch(&mut self) -> Result<Option<Vec<Self::Item>>> {
        if self.current_idx >= self.image_paths.len() {
            return Ok(None);
        }

        let end_idx = (self.current_idx + self.batch_size).min(self.image_paths.len());
        let mut batch = Vec::new();

        for i in self.current_idx..end_idx {
            let mut image = self.load_image(&self.image_paths[i])?;
            if self.augment {
                image = self.augment_image(image);
            }
            batch.push((image, self.labels[i]));
        }

        self.current_idx = end_idx;
        Ok(Some(batch))
    }

    fn len(&self) -> usize {
        self.image_paths.len()
    }

    fn reset(&mut self) {
        self.current_idx = 0;
        if self.shuffle {
            // Shuffle indices
        }
    }
}

/// Time-series data loader
pub struct TimeSeriesDataLoader {
    data: Array2<f32>,
    sequence_length: usize,
    batch_size: usize,
    current_idx: usize,
}

impl TimeSeriesDataLoader {
    pub fn new(data: Array2<f32>, sequence_length: usize, batch_size: usize) -> Self {
        Self {
            data,
            sequence_length,
            batch_size,
            current_idx: 0,
        }
    }
}

impl DataLoader for TimeSeriesDataLoader {
    type Item = (Array2<f32>, Array2<f32>);

    fn next_batch(&mut self) -> Result<Option<Vec<Self::Item>>> {
        let max_start = self.data.nrows().saturating_sub(self.sequence_length + 1);
        if self.current_idx >= max_start {
            return Ok(None);
        }

        let end_idx = (self.current_idx + self.batch_size).min(max_start);
        let mut batch = Vec::new();

        for i in self.current_idx..end_idx {
            // Extract sequences manually to avoid unsafe s! macro
            let ncols = self.data.ncols();
            let mut x_data = Vec::with_capacity(self.sequence_length * ncols);
            let mut y_data = Vec::with_capacity(self.sequence_length * ncols);
            
            for row_idx in 0..self.sequence_length {
                for col_idx in 0..ncols {
                    x_data.push(self.data[[i + row_idx, col_idx]]);
                    y_data.push(self.data[[i + row_idx + 1, col_idx]]);
                }
            }
            
            let x = Array2::from_shape_vec((self.sequence_length, ncols), x_data)
                .map_err(|e| crate::error::MLError::InvalidInput(e.to_string()))?;
            let y = Array2::from_shape_vec((self.sequence_length, ncols), y_data)
                .map_err(|e| crate::error::MLError::InvalidInput(e.to_string()))?;
            
            batch.push((x, y));
        }

        self.current_idx = end_idx;
        Ok(Some(batch))
    }

    fn len(&self) -> usize {
        self.data.nrows().saturating_sub(self.sequence_length + 1)
    }

    fn reset(&mut self) {
        self.current_idx = 0;
    }
}

/// Graph data loader for GNN training
pub struct GraphDataLoader {
    graphs: Vec<GraphData>,
    batch_size: usize,
    current_idx: usize,
}

#[derive(Clone)]
pub struct GraphData {
    pub node_features: Array2<f32>,
    pub edge_index: Vec<(usize, usize)>,
    pub labels: Vec<usize>,
}

impl GraphDataLoader {
    pub fn new(graphs: Vec<GraphData>, batch_size: usize) -> Self {
        Self {
            graphs,
            batch_size,
            current_idx: 0,
        }
    }
}

impl DataLoader for GraphDataLoader {
    type Item = GraphData;

    fn next_batch(&mut self) -> Result<Option<Vec<Self::Item>>> {
        if self.current_idx >= self.graphs.len() {
            return Ok(None);
        }

        let end_idx = (self.current_idx + self.batch_size).min(self.graphs.len());
        let batch: Vec<GraphData> = self.graphs[self.current_idx..end_idx].to_vec();

        self.current_idx = end_idx;
        Ok(Some(batch))
    }

    fn len(&self) -> usize {
        self.graphs.len()
    }

    fn reset(&mut self) {
        self.current_idx = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array;

    #[test]
    fn test_image_data_loader() {
        let paths = vec!["img1.jpg".to_string(), "img2.jpg".to_string()];
        let labels = vec![0, 1];
        let mut loader = ImageDataLoader::new(paths, labels, 1, false, false);
        
        assert_eq!(loader.len(), 2);
        assert!(!loader.is_empty());
    }

    #[test]
    fn test_time_series_data_loader() {
        let data = Array::from_shape_fn((100, 10), |(i, j)| (i + j) as f32);
        let mut loader = TimeSeriesDataLoader::new(data, 10, 5);
        
        assert!(loader.len() > 0);
    }
}
