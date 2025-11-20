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

        let pipeline = self
            .device
            .new_compute_pipeline_state_with_function(&kernel)
            .map_err(|e| {
                ComputeError::execution(ExecutionError::KernelCompilationFailed {
                    kernel: kernel_name.to_string(),
                    error: format!("Failed to create pipeline: {}", e),
                })
            })?;

        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&pipeline);
        for (i, buffer) in buffers.iter().enumerate() {
            encoder.set_buffer(i as u64, Some(buffer), 0);
        }

        // Configure thread groups (256 threads per group is optimal for most Apple GPUs)
        let thread_group_size = MTLSize {
            width: 256.min(grid_size),
            height: 1,
            depth: 1,
        };

        let thread_groups = MTLSize {
            width: grid_size.div_ceil(thread_group_size.width),
            height: 1,
            depth: 1,
        };

        encoder.dispatch_thread_groups(thread_groups, thread_group_size);
        encoder.end_encoding();

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

    fn create_buffer_i64(&self, len: usize) -> Result<metal::Buffer, ComputeError> {
        let size = (len * mem::size_of::<i64>()) as u64;
        Ok(self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeShared))
    }

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

    fn read_buffer_i32(
        &self,
        buffer: &metal::Buffer,
        len: usize,
    ) -> Result<Vec<i32>, ComputeError> {
        let contents = buffer.contents() as *const i32;
        let slice = unsafe { std::slice::from_raw_parts(contents, len) };
        Ok(slice.to_vec())
    }

    fn read_buffer_i64(
        &self,
        buffer: &metal::Buffer,
        len: usize,
    ) -> Result<Vec<i64>, ComputeError> {
        let contents = buffer.contents() as *const i64;
        let slice = unsafe { std::slice::from_raw_parts(contents, len) };
        Ok(slice.to_vec())
    }

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
