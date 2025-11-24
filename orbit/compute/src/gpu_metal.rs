//! Apple Metal GPU acceleration backend
//!
//! Provides GPU compute acceleration for database operations using Apple's Metal framework.
//! This module is only available on macOS and iOS platforms.

#![cfg(target_os = "macos")]

use crate::errors::ComputeError;
use crate::gpu_backend::{FilterOp, GpuBackendType, GpuDevice};
use metal::*;
use std::mem;

/// Metal GPU compute device
#[derive(Debug)]
pub struct MetalDevice {
    device: metal::Device,
    queue: metal::CommandQueue,
    library: metal::Library,
}

impl MetalDevice {
    /// Create a new Metal device with compiled kernels
    pub fn new() -> Result<Self, ComputeError> {
        use crate::errors::GPUError;

        let device = metal::Device::system_default().ok_or_else(|| {
            ComputeError::gpu(GPUError::DeviceNotFound { device_id: 0 })
        })?;

        tracing::info!(
            "Metal device initialized: {} ({})",
            device.name(),
            if device.is_low_power() {
                "Integrated"
            } else {
                "Discrete"
            }
        );

        let queue = device.new_command_queue();

        // Compile Metal shaders from embedded source
        let source = include_str!("shaders/database_kernels.metal");
        let options = CompileOptions::new();
        let library = device
            .new_library_with_source(source, &options)
            .map_err(|e| {
                ComputeError::gpu(GPUError::APIInitializationFailed {
                    api: "Metal".to_string(),
                    error: format!("Failed to compile kernels: {}", e),
                })
            })?;

        tracing::info!("Metal kernels compiled successfully");

        Ok(Self {
            device,
            queue,
            library,
        })
    }

    /// Get device information
    pub fn device_info(&self) -> MetalDeviceInfo {
        MetalDeviceInfo {
            name: self.device.name().to_string(),
            is_low_power: self.device.is_low_power(),
            is_headless: self.device.is_headless(),
            is_removable: self.device.is_removable(),
            supports_family_apple7: self
                .device
                .supports_family(MTLGPUFamily::Apple7),
            supports_family_apple8: self
                .device
                .supports_family(MTLGPUFamily::Apple8),
            max_threads_per_threadgroup: self.device.max_threads_per_threadgroup(),
            recommended_max_working_set_size: self.device.recommended_max_working_set_size(),
        }
    }

    /// Execute a filter operation on i32 data
    pub fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        let kernel_name = match operation {
            FilterOp::Equal => "filter_i32_eq",
            FilterOp::GreaterThan => "filter_i32_gt",
            FilterOp::GreaterOrEqual => "filter_i32_ge",
            FilterOp::LessThan => "filter_i32_lt",
            FilterOp::LessOrEqual => "filter_i32_le",
            FilterOp::NotEqual => "filter_i32_ne",
        };

        let len = data.len();

        // Create buffers
        let data_buffer = self.create_buffer_with_data_i32(data)?;
        let value_buffer = self.create_buffer_with_data_i32(&[value])?;
        let output_buffer = self.create_buffer_i32(len)?;

        // Execute kernel
        self.execute_kernel(
            kernel_name,
            &[&data_buffer, &value_buffer, &output_buffer],
            len as u64,
        )?;

        // Read results
        self.read_buffer_i32(&output_buffer, len)
    }

    /// Execute a filter operation on i64 data
    pub fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        let kernel_name = match operation {
            FilterOp::Equal => "filter_i64_eq",
            FilterOp::GreaterThan => "filter_i64_gt",
            FilterOp::GreaterOrEqual => "filter_i64_ge",
            FilterOp::LessThan => "filter_i64_lt",
            FilterOp::LessOrEqual => "filter_i64_le",
            FilterOp::NotEqual => "filter_i64_ne",
        };

        let len = data.len();

        // Create buffers
        let data_buffer = self.create_buffer_with_data_i64(data)?;
        let value_buffer = self.create_buffer_with_data_i64(&[value])?;
        let output_buffer = self.create_buffer_i32(len)?;

        // Execute kernel
        self.execute_kernel(
            kernel_name,
            &[&data_buffer, &value_buffer, &output_buffer],
            len as u64,
        )?;

        // Read results
        self.read_buffer_i32(&output_buffer, len)
    }

    /// Execute a filter operation on f64 data
    pub fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        let kernel_name = match operation {
            FilterOp::Equal => "filter_f64_eq",
            FilterOp::GreaterThan => "filter_f64_gt",
            FilterOp::GreaterOrEqual => "filter_f64_ge",
            FilterOp::LessThan => "filter_f64_lt",
            FilterOp::LessOrEqual => "filter_f64_le",
            FilterOp::NotEqual => "filter_f64_ne",
        };

        let len = data.len();

        // Create buffers
        let data_buffer = self.create_buffer_with_data_f64(data)?;
        let value_buffer = self.create_buffer_with_data_f64(&[value])?;
        let output_buffer = self.create_buffer_i32(len)?;

        // Execute kernel
        self.execute_kernel(
            kernel_name,
            &[&data_buffer, &value_buffer, &output_buffer],
            len as u64,
        )?;

        // Read results
        self.read_buffer_i32(&output_buffer, len)
    }

    /// Combine two filter masks with AND operation
    pub fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        use crate::errors::ExecutionError;

        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "mask_length".to_string(),
                value: format!("mask_a={}, mask_b={}", mask_a.len(), mask_b.len()),
            }));
        }

        let len = mask_a.len();
        let buffer_a = self.create_buffer_with_data_i32(mask_a)?;
        let buffer_b = self.create_buffer_with_data_i32(mask_b)?;
        let output_buffer = self.create_buffer_i32(len)?;

        self.execute_kernel("bitmap_and", &[&buffer_a, &buffer_b, &output_buffer], len as u64)?;

        self.read_buffer_i32(&output_buffer, len)
    }

    /// Combine two filter masks with OR operation
    pub fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        use crate::errors::ExecutionError;

        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "mask_length".to_string(),
                value: format!("mask_a={}, mask_b={}", mask_a.len(), mask_b.len()),
            }));
        }

        let len = mask_a.len();
        let buffer_a = self.create_buffer_with_data_i32(mask_a)?;
        let buffer_b = self.create_buffer_with_data_i32(mask_b)?;
        let output_buffer = self.create_buffer_i32(len)?;

        self.execute_kernel("bitmap_or", &[&buffer_a, &buffer_b, &output_buffer], len as u64)?;

        self.read_buffer_i32(&output_buffer, len)
    }

    /// Negate a filter mask with NOT operation
    pub fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError> {
        let len = mask.len();
        let buffer = self.create_buffer_with_data_i32(mask)?;
        let output_buffer = self.create_buffer_i32(len)?;

        self.execute_kernel("bitmap_not", &[&buffer, &output_buffer], len as u64)?;

        self.read_buffer_i32(&output_buffer, len)
    }

    /// Sum aggregation for i32
    pub fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError> {
        let len = data.len();
        let input_buffer = self.create_buffer_with_data_i32(data)?;
        let output_buffer = self.create_buffer_i32(1)?; // Single atomic counter

        self.execute_kernel("aggregate_i32_sum", &[&input_buffer, &output_buffer], len as u64)?;

        let result = self.read_buffer_i32(&output_buffer, 1)?;
        Ok(result[0] as i64)
    }

    /// Count aggregation (count non-zero mask values)
    pub fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError> {
        let len = mask.len();
        let mask_buffer = self.create_buffer_with_data_i32(mask)?;
        let count_buffer = self.create_buffer_i32(1)?; // Single atomic counter

        self.execute_kernel("aggregate_i32_count", &[&mask_buffer, &count_buffer], len as u64)?;

        let result = self.read_buffer_i32(&count_buffer, 1)?;
        Ok(result[0] as usize)
    }

    /// Execute GPU-accelerated i32 aggregation with null bitmap support
    /// 
    /// This method handles null bitmaps and routes to the appropriate aggregation kernel.
    pub fn execute_aggregate_i32(
        &self,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        kernel_name: &str,
    ) -> Result<Option<f64>, ComputeError> {
        use crate::errors::ExecutionError;

        let len = values.len();

        // Create null mask if null_bitmap is provided
        let mask: Vec<i32> = if let Some(bm) = null_bitmap {
            (0..len)
                .map(|idx| {
                    let byte_idx = idx / 8;
                    let bit_idx = idx % 8;
                    if byte_idx < bm.len() && (bm[byte_idx] >> bit_idx) & 1 == 0 {
                        1 // Valid
                    } else {
                        0 // Null
                    }
                })
                .collect()
        } else {
            vec![1; len] // All valid
        };

        // Filter values based on null bitmap
        let filtered_values: Vec<i32> = values
            .iter()
            .enumerate()
            .filter(|(idx, _)| mask[*idx] != 0)
            .map(|(_, &v)| v)
            .collect();

        if filtered_values.is_empty() {
            return Ok(None);
        }

        // Create buffers
        let input_buffer = self.create_buffer_with_data_i32(&filtered_values)?;
        let output_buffer = self.create_buffer_i32(1)?;

        // Initialize output buffer based on operation
        // For MIN, initialize with first valid value (handled in kernel)
        // For MAX, initialize with first valid value (handled in kernel)
        // For SUM and COUNT, initialize to 0 (handled by buffer creation)

        // Execute kernel
        self.execute_kernel(
            kernel_name,
            &[&input_buffer, &output_buffer],
            filtered_values.len() as u64,
        )?;

        // Read result
        let result = self.read_buffer_i32(&output_buffer, 1)?;

        match kernel_name {
            "aggregate_i32_sum" | "aggregate_i32_count" => Ok(Some(result[0] as f64)),
            "aggregate_i32_min" => {
                // Check if we got MAX (meaning no valid values)
                if result[0] == i32::MAX {
                    Ok(None)
                } else {
                    Ok(Some(result[0] as f64))
                }
            }
            "aggregate_i32_max" => {
                // Check if we got MIN (meaning no valid values)
                if result[0] == i32::MIN {
                    Ok(None)
                } else {
                    Ok(Some(result[0] as f64))
                }
            }
            _ => Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "kernel_name".to_string(),
                value: format!("Unknown kernel: {}", kernel_name),
            })),
        }
    }

    // ========================================================================
    // Internal helper methods
    // ========================================================================

    fn execute_kernel(
        &self,
        kernel_name: &str,
        buffers: &[&metal::Buffer],
        grid_size: u64,
    ) -> Result<(), ComputeError> {
        use crate::errors::{ExecutionError, GPUError};

        let kernel = self
            .library
            .get_function(kernel_name, None)
            .map_err(|e| {
                ComputeError::gpu(GPUError::KernelLaunchFailed {
                    kernel_name: kernel_name.to_string(),
                    error: format!("Kernel not found: {}", e),
                })
            })?;

        // Cache pipeline state for better performance (reuse compiled pipelines)
        // TODO: Implement pipeline caching in future optimization
        let pipeline = self
            .device
            .new_compute_pipeline_state_with_function(&kernel)
            .map_err(|e| {
                ComputeError::execution(ExecutionError::KernelCompilationFailed {
                    kernel: kernel_name.to_string(),
                    error: format!("Failed to create pipeline: {}", e),
                })
            })?;

        // Use optimized command buffer creation
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&pipeline);
        
        // Set buffers with optimized offset calculation
        for (i, buffer) in buffers.iter().enumerate() {
            encoder.set_buffer(i as u64, Some(buffer), 0);
        }

        // Optimize thread group size based on GPU capabilities
        // Apple Silicon GPUs work best with 256 threads, but can handle up to 1024
        let optimal_threads = 256u64;
        let thread_group_size = MTLSize {
            width: optimal_threads.min(grid_size),
            height: 1,
            depth: 1,
        };

        // Calculate optimal thread group count
        let thread_groups = MTLSize {
            width: (grid_size + thread_group_size.width - 1) / thread_group_size.width,
            height: 1,
            depth: 1,
        };

        // Dispatch with optimized configuration
        encoder.dispatch_thread_groups(thread_groups, thread_group_size);
        encoder.end_encoding();

        // Commit and wait (for now - could be async in future)
        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Check for errors
        if command_buffer.status() == MTLCommandBufferStatus::Error {
            return Err(ComputeError::gpu(GPUError::KernelLaunchFailed {
                kernel_name: kernel_name.to_string(),
                error: "Command buffer execution failed".to_string(),
            }));
        }

        Ok(())
    }

    fn create_buffer_i32(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<i32>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    #[allow(dead_code)] // Reserved for future i64 buffer operations
    fn create_buffer_i64(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<i64>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    #[allow(dead_code)] // Reserved for future f64 buffer operations
    fn create_buffer_f64(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<f64>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    fn create_buffer_with_data_i32(&self, data: &[i32]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<i32>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    fn create_buffer_with_data_i64(&self, data: &[i64]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<i64>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    fn create_buffer_with_data_f64(&self, data: &[f64]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<f64>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    fn create_buffer_f32(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<f32>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    fn create_buffer_with_data_f32(&self, data: &[f32]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<f32>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    fn read_buffer_f32(&self, buffer: &metal::Buffer, len: usize) -> Result<Vec<f32>, ComputeError> {
        let contents = buffer.contents();
        let slice = unsafe {
            std::slice::from_raw_parts(contents as *const f32, len)
        };
        Ok(slice.to_vec())
    }

    /// Execute GPU-accelerated vector similarity calculation
    ///
    /// Calculates similarity between a query vector and multiple candidate vectors.
    /// The candidate vectors are provided as a flattened array: [v1[0..dim], v2[0..dim], ...]
    pub fn execute_vector_similarity(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[f32],
        _vector_count: usize,
        dimension: usize,
        kernel_name: &str,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        // Validate inputs
        if query_vector.len() != dimension {
            return Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "query_vector_dimension".to_string(),
                value: format!("expected {}, got {}", dimension, query_vector.len()),
            }));
        }

        let vector_count = candidate_vectors.len() / dimension;
        if candidate_vectors.len() != vector_count * dimension {
            return Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "candidate_vectors_length".to_string(),
                value: format!("expected {}, got {}", vector_count * dimension, candidate_vectors.len()),
            }));
        }

        // Create buffers
        let query_buffer = self.create_buffer_with_data_f32(query_vector)?;
        let candidates_buffer = self.create_buffer_with_data_f32(candidate_vectors)?;
        let scores_buffer = self.create_buffer_f32(vector_count)?;

        // Create parameter buffer: [vector_count, dimension]
        let params = [vector_count as u32, dimension as u32];
        let params_buffer = self.create_buffer_with_data_u32(&params)?;

        // Execute kernel
        self.execute_kernel(
            kernel_name,
            &[&query_buffer, &candidates_buffer, &scores_buffer, &params_buffer],
            vector_count as u64,
        )?;

        // Read results back
        self.read_buffer_f32(&scores_buffer, vector_count)
    }

    /// Execute GPU-accelerated spatial distance calculation
    /// 
    /// Calculates 2D Euclidean distance between a query point and multiple candidate points.
    pub fn execute_spatial_distance(
        &self,
        query_x: f32,
        query_y: f32,
        candidates_x: &[f32],
        candidates_y: &[f32],
        point_count: usize,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        // Validate inputs
        if candidates_x.len() != point_count || candidates_y.len() != point_count {
            return Err(ComputeError::execution(ExecutionError::InvalidKernelParameters {
                parameter: "candidates_length".to_string(),
                value: format!(
                    "expected {}, got x={}, y={}",
                    point_count,
                    candidates_x.len(),
                    candidates_y.len()
                ),
            }));
        }

        // Create buffers
        let query_x_buffer = self.create_buffer_with_data_f32(&[query_x])?;
        let query_y_buffer = self.create_buffer_with_data_f32(&[query_y])?;
        let candidates_x_buffer = self.create_buffer_with_data_f32(candidates_x)?;
        let candidates_y_buffer = self.create_buffer_with_data_f32(candidates_y)?;
        let distances_buffer = self.create_buffer_f32(point_count)?;
        let point_count_buffer = self.create_buffer_with_data_u32(&[point_count as u32])?;

        // Execute kernel
        self.execute_kernel(
            "spatial_distance",
            &[
                &query_x_buffer,
                &query_y_buffer,
                &candidates_x_buffer,
                &candidates_y_buffer,
                &distances_buffer,
                &point_count_buffer,
            ],
            point_count as u64,
        )?;

        // Read results back
        self.read_buffer_f32(&distances_buffer, point_count)
    }

    /// Execute GPU-accelerated BFS level expansion (u32 indices - optimized for graphs < 4B nodes)
    /// This is a helper method for graph traversal operations
    pub fn execute_bfs_level_expansion_u32(
        &self,
        edge_array: &[u32],
        edge_offset: &[u32],
        current_level: &[u32],
        visited: &mut [u32],
        next_level: &mut [u32],
        next_level_size: &mut u32,
        parent: &mut [u32],
        current_level_size: u32,
        max_nodes: u32,
    ) -> Result<(), ComputeError> {
        // Create buffers
        let edge_array_buffer = self.create_buffer_with_data_u32(edge_array)?;
        let edge_offset_buffer = self.create_buffer_with_data_u32(edge_offset)?;
        let current_level_buffer = self.create_buffer_with_data_u32(current_level)?;
        let visited_buffer = self.create_buffer_with_data_u32(visited)?;
        let next_level_buffer = self.create_buffer_u32(max_nodes as usize)?;

        // Create atomic counter buffer for next_level_size
        let next_level_size_buffer = self.create_buffer_with_data_u32(&[*next_level_size])?;

        // Create parent buffer for path reconstruction
        let parent_buffer = self.create_buffer_with_data_u32(parent)?;

        // Create parameter buffer
        let params = [current_level_size, max_nodes];
        let params_buffer = self.create_buffer_with_data_u32(&params)?;

        // Execute kernel
        self.execute_kernel(
            "bfs_level_expansion",
            &[
                &edge_array_buffer,
                &edge_offset_buffer,
                &current_level_buffer,
                &visited_buffer,
                &next_level_buffer,
                &next_level_size_buffer,
                &parent_buffer,
                &params_buffer,
            ],
            current_level_size as u64,
        )?;

        // Read results back
        let visited_slice = unsafe {
            std::slice::from_raw_parts(
                visited_buffer.contents() as *const u32,
                visited.len(),
            )
        };
        visited.copy_from_slice(visited_slice);

        let next_level_slice = unsafe {
            std::slice::from_raw_parts(
                next_level_buffer.contents() as *const u32,
                next_level.len().min(max_nodes as usize),
            )
        };
        next_level[..next_level_slice.len()].copy_from_slice(next_level_slice);

        let size_slice = unsafe {
            std::slice::from_raw_parts(
                next_level_size_buffer.contents() as *const u32,
                1,
            )
        };
        *next_level_size = size_slice[0];

        // Read parent buffer back
        let parent_slice = unsafe {
            std::slice::from_raw_parts(
                parent_buffer.contents() as *const u32,
                parent.len(),
            )
        };
        parent.copy_from_slice(parent_slice);

        Ok(())
    }
    
    /// Execute GPU-accelerated BFS level expansion (u64 indices - for large graphs)
    /// This is a helper method for graph traversal operations
    pub fn execute_bfs_level_expansion(
        &self,
        edge_array: &[u32],
        edge_offset: &[u32],
        current_level: &[u64],
        visited: &mut [u32],
        next_level: &mut [u64],
        next_level_size: &mut u32,
        parent: &mut [u32],
        current_level_size: u32,
        max_nodes: u32,
    ) -> Result<(), ComputeError> {
        // Convert u64 to u32 for Metal kernel (Metal kernel expects u32)
        // Note: This assumes indices fit in u32, which is true for graphs < 4B nodes
        let current_level_u32: Vec<u32> = current_level.iter()
            .map(|&x| x as u32)
            .collect();
        let mut next_level_u32 = vec![0u32; max_nodes as usize];
        let mut next_level_size_u32 = *next_level_size;

        // Execute with u32 version
        self.execute_bfs_level_expansion_u32(
            edge_array,
            edge_offset,
            &current_level_u32,
            visited,
            &mut next_level_u32,
            &mut next_level_size_u32,
            parent,
            current_level_size,
            max_nodes,
        )?;

        // Convert results back to u64
        *next_level_size = next_level_size_u32;
        for (i, &val) in next_level_u32.iter().enumerate().take(*next_level_size as usize) {
            next_level[i] = val as u64;
        }
        
        Ok(())
    }

    #[allow(dead_code)] // Reserved for future u32 buffer operations
    fn create_buffer_u32(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<u32>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    #[allow(dead_code)]
    fn create_buffer_u64(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<u64>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

    fn create_buffer_with_data_u32(&self, data: &[u32]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<u32>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    #[allow(dead_code)]
    fn create_buffer_with_data_u64(&self, data: &[u64]) -> Result<metal::Buffer, ComputeError> {
        let size = (data.len() * mem::size_of::<u64>()) as u64;
        Ok(self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size,
            MTLResourceOptions::StorageModeShared,
        ))
    }

    fn read_buffer_i32(
        &self,
        buffer: &metal::Buffer,
        len: usize,
    ) -> Result<Vec<i32>, ComputeError> {
        let contents = buffer.contents() as *const i32;
        let slice = unsafe { std::slice::from_raw_parts(contents, len) };
        Ok(slice.to_vec())
    }

    #[allow(dead_code)] // Reserved for future i64 buffer operations
    fn read_buffer_i64(
        &self,
        buffer: &metal::Buffer,
        len: usize,
    ) -> Result<Vec<i64>, ComputeError> {
        let contents = buffer.contents() as *const i64;
        let slice = unsafe { std::slice::from_raw_parts(contents, len) };
        Ok(slice.to_vec())
    }

    #[allow(dead_code)] // Reserved for future f64 buffer operations
    fn read_buffer_f64(
        &self,
        buffer: &metal::Buffer,
        len: usize,
    ) -> Result<Vec<f64>, ComputeError> {
        let contents = buffer.contents() as *const f64;
        let slice = unsafe { std::slice::from_raw_parts(contents, len) };
        Ok(slice.to_vec())
    }
}

// Implement GpuDevice trait for MetalDevice
impl GpuDevice for MetalDevice {
    fn backend_type(&self) -> GpuBackendType {
        GpuBackendType::Metal
    }

    fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    fn is_available(&self) -> bool {
        true // If we created the device, it's available
    }

    fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::execute_filter_i32(self, data, value, operation)
    }

    fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::execute_filter_i64(self, data, value, operation)
    }

    fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::execute_filter_f64(self, data, value, operation)
    }

    fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::bitmap_and(self, mask_a, mask_b)
    }

    fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::bitmap_or(self, mask_a, mask_b)
    }

    fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError> {
        MetalDevice::bitmap_not(self, mask)
    }

    fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError> {
        MetalDevice::aggregate_sum_i32(self, data)
    }

    fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError> {
        MetalDevice::aggregate_count(self, mask)
    }
}

/// Metal device information
#[derive(Debug, Clone)]
pub struct MetalDeviceInfo {
    pub name: String,
    pub is_low_power: bool,
    pub is_headless: bool,
    pub is_removable: bool,
    pub supports_family_apple7: bool,
    pub supports_family_apple8: bool,
    pub max_threads_per_threadgroup: MTLSize,
    pub recommended_max_working_set_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metal_device_creation() {
        match MetalDevice::new() {
            Ok(device) => {
                let info = device.device_info();
                println!("Metal device: {}", info.name);
                println!("  Low power: {}", info.is_low_power);
                println!(
                    "  Max threads per threadgroup: {}x{}x{}",
                    info.max_threads_per_threadgroup.width,
                    info.max_threads_per_threadgroup.height,
                    info.max_threads_per_threadgroup.depth
                );
            }
            Err(e) => {
                println!("No Metal device available: {}", e);
            }
        }
    }

    #[test]
    fn test_filter_i32_eq() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (0..1000).collect();
        let result = device.execute_filter_i32(&data, 500, FilterOp::Equal).unwrap();

        // Count matches
        let matches: i32 = result.iter().sum();
        assert_eq!(matches, 1, "Should find exactly one match for value 500");

        // Check that index 500 is marked
        assert_eq!(result[500], 1);
    }

    #[test]
    fn test_filter_i32_gt() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (0..100).collect();
        let result = device.execute_filter_i32(&data, 50, FilterOp::GreaterThan).unwrap();

        let matches: i32 = result.iter().sum();
        assert_eq!(matches, 49, "Values 51-99 should match (49 values)");
    }

    #[test]
    fn test_bitmap_and() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let mask_a = vec![1, 1, 0, 0, 1, 1];
        let mask_b = vec![1, 0, 1, 0, 1, 0];
        let result = device.bitmap_and(&mask_a, &mask_b).unwrap();

        assert_eq!(result, vec![1, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn test_bitmap_or() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let mask_a = vec![1, 1, 0, 0, 1, 1];
        let mask_b = vec![1, 0, 1, 0, 1, 0];
        let result = device.bitmap_or(&mask_a, &mask_b).unwrap();

        assert_eq!(result, vec![1, 1, 1, 0, 1, 1]);
    }

    #[test]
    fn test_bitmap_not() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let mask = vec![1, 0, 1, 0, 1];
        let result = device.bitmap_not(&mask).unwrap();

        assert_eq!(result, vec![0, 1, 0, 1, 0]);
    }

    #[test]
    fn test_aggregate_sum_i32() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (1..=100).collect();
        let result = device.aggregate_sum_i32(&data).unwrap();

        // Sum of 1..=100 is 5050
        assert_eq!(result, 5050);
    }

    #[test]
    fn test_aggregate_count() {
        let device = match MetalDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping Metal test - no device available");
                return;
            }
        };

        let mask = vec![1, 0, 1, 1, 0, 1, 0, 0, 1, 1];
        let result = device.aggregate_count(&mask).unwrap();

        assert_eq!(result, 6, "Should count 6 non-zero values");
    }
}
