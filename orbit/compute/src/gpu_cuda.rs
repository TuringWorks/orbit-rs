//! NVIDIA CUDA GPU acceleration backend
//!
//! Provides GPU compute acceleration for database operations using NVIDIA CUDA.
//! This module is available on Linux and Windows platforms with NVIDIA GPUs.
//!
//! ## Features
//!
//! - Filter operations (i32, i64, f64) with comparison predicates
//! - Bitmap operations (AND, OR, NOT) for combining filter masks
//! - Aggregation operations (SUM, COUNT, MIN, MAX)
//! - Vector similarity (cosine, euclidean, dot product)
//! - Spatial distance calculations (2D Euclidean, Haversine)
//! - Graph traversal (BFS, Dijkstra)
//! - Matrix operations (GEMM with tiling)
//! - Time-series window aggregation
//! - Hash join operations

#![cfg(feature = "gpu-cuda")]

use crate::errors::ComputeError;
use crate::gpu_backend::{FilterOp, GpuBackendType, GpuDevice};
use std::sync::Arc;

#[cfg(feature = "gpu-cuda")]
use cudarc::driver::{CudaDevice as CudaRcDevice, CudaSlice, LaunchAsync, LaunchConfig};
#[cfg(feature = "gpu-cuda")]
use cudarc::nvrtc::Ptx;

/// Embedded CUDA kernel source
const CUDA_KERNEL_SOURCE: &str = include_str!("shaders/cuda/database_kernels.cu");

/// CUDA GPU compute device with full kernel support
pub struct CudaDevice {
    device_id: u32,
    device_name: String,
    #[cfg(feature = "gpu-cuda")]
    device: Arc<CudaRcDevice>,
    #[cfg(feature = "gpu-cuda")]
    module_loaded: bool,
}

impl std::fmt::Debug for CudaDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CudaDevice")
            .field("device_id", &self.device_id)
            .field("device_name", &self.device_name)
            .finish()
    }
}

impl CudaDevice {
    /// Create a new CUDA device with compiled kernels
    pub fn new() -> Result<Self, ComputeError> {
        Self::new_with_device_id(0)
    }

    /// Create a new CUDA device with a specific device ID
    pub fn new_with_device_id(device_id: u32) -> Result<Self, ComputeError> {
        use crate::errors::GPUError;

        // Check if CUDA is available
        if !Self::is_cuda_available() {
            return Err(ComputeError::gpu(GPUError::DeviceNotFound {
                device_id: device_id as usize,
            }));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            // Initialize CUDA device using cudarc
            let device = CudaRcDevice::new(device_id as usize).map_err(|e| {
                ComputeError::gpu(GPUError::APIInitializationFailed {
                    api: "CUDA".to_string(),
                    error: format!("Failed to initialize CUDA device {}: {}", device_id, e),
                })
            })?;

            let device_name = Self::get_device_name_from_driver(&device)?;

            tracing::info!(
                "CUDA device initialized: {} (device {})",
                device_name,
                device_id
            );

            let mut cuda_device = Self {
                device_id,
                device_name,
                device: Arc::new(device),
                module_loaded: false,
            };

            // Load and compile kernels
            cuda_device.load_kernels()?;

            Ok(cuda_device)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let device_name = Self::get_device_name(device_id)?;

            tracing::info!(
                "CUDA device initialized (no cudarc): {} (device {})",
                device_name,
                device_id
            );

            Ok(Self {
                device_id,
                device_name,
            })
        }
    }

    #[cfg(feature = "gpu-cuda")]
    fn get_device_name_from_driver(device: &CudaRcDevice) -> Result<String, ComputeError> {
        // Get device name from CUDA driver
        Ok(format!("NVIDIA GPU (device {})", device.ordinal()))
    }

    #[cfg(feature = "gpu-cuda")]
    fn load_kernels(&mut self) -> Result<(), ComputeError> {
        use crate::errors::GPUError;

        // Compile CUDA source to PTX using nvrtc
        let ptx = cudarc::nvrtc::compile_ptx(CUDA_KERNEL_SOURCE).map_err(|e| {
            ComputeError::gpu(GPUError::KernelLaunchFailed {
                kernel_name: "all".to_string(),
                error: format!("Failed to compile CUDA kernels: {}", e),
            })
        })?;

        // Load the PTX module into the device
        self.device
            .load_ptx(
                ptx,
                "database_kernels",
                &[
                    // Filter operations - i32
                    "filter_i32_eq",
                    "filter_i32_gt",
                    "filter_i32_ge",
                    "filter_i32_lt",
                    "filter_i32_le",
                    "filter_i32_ne",
                    // Filter operations - i64
                    "filter_i64_eq",
                    "filter_i64_gt",
                    "filter_i64_ge",
                    "filter_i64_lt",
                    "filter_i64_le",
                    "filter_i64_ne",
                    // Filter operations - f64
                    "filter_f64_eq",
                    "filter_f64_gt",
                    "filter_f64_ge",
                    "filter_f64_lt",
                    "filter_f64_le",
                    "filter_f64_ne",
                    // Bitmap operations
                    "bitmap_and",
                    "bitmap_or",
                    "bitmap_not",
                    // Aggregation operations
                    "aggregate_i32_sum",
                    "aggregate_i32_count",
                    "aggregate_i32_min",
                    "aggregate_i32_max",
                    // Vector similarity
                    "vector_cosine_similarity",
                    "vector_euclidean_distance",
                    "vector_dot_product",
                    // Spatial operations
                    "spatial_distance",
                    "spatial_distance_sphere",
                    // Graph traversal
                    "bfs_level_expansion",
                    "dijkstra_relax",
                    // Matrix operations
                    "matrix_multiply_tiled_f32",
                    // Time-series
                    "timeseries_window_aggregate",
                    "timeseries_finalize_avg",
                    // Hash join
                    "hash_join_build",
                    "hash_join_probe",
                ],
            )
            .map_err(|e| {
                ComputeError::gpu(GPUError::KernelLaunchFailed {
                    kernel_name: "load_ptx".to_string(),
                    error: format!("Failed to load CUDA PTX module: {}", e),
                })
            })?;

        self.module_loaded = true;
        tracing::info!("CUDA kernels compiled and loaded successfully");

        Ok(())
    }

    /// Check if CUDA is available on the system
    pub(crate) fn is_cuda_available() -> bool {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            // Linux/Unix CUDA detection
            return std::path::Path::new("/usr/local/cuda").exists()
                || std::env::var("CUDA_PATH").is_ok()
                || which::which("nvcc").is_ok()
                || std::path::Path::new("/usr/bin/nvcc").exists();
        }

        #[cfg(target_os = "windows")]
        {
            // Windows CUDA detection
            if std::env::var("CUDA_PATH").is_ok() {
                return true;
            }

            if which::which("nvcc.exe").is_ok() {
                return true;
            }

            // Check common Windows CUDA installation paths
            let program_files =
                std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
            let cuda_paths = vec![
                format!("{}\\NVIDIA GPU Computing Toolkit\\CUDA", program_files),
                "C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA".to_string(),
                "C:\\CUDA".to_string(),
            ];

            for path in cuda_paths {
                if std::path::Path::new(&path).exists() {
                    return true;
                }
            }

            false
        }

        #[cfg(target_os = "macos")]
        {
            // CUDA is not supported on modern macOS
            false
        }

        #[cfg(not(any(unix, target_os = "windows", target_os = "macos")))]
        {
            false
        }
    }

    /// Get device name for a given device ID (fallback when cudarc not available)
    #[cfg(not(feature = "gpu-cuda"))]
    fn get_device_name(device_id: u32) -> Result<String, ComputeError> {
        Ok(format!("NVIDIA GPU (device {})", device_id))
    }

    /// Calculate optimal launch configuration for a kernel
    #[cfg(feature = "gpu-cuda")]
    fn launch_config(&self, count: usize) -> LaunchConfig {
        let block_size = 256u32;
        let grid_size = ((count as u32) + block_size - 1) / block_size;
        LaunchConfig {
            grid_dim: (grid_size, 1, 1),
            block_dim: (block_size, 1, 1),
            shared_mem_bytes: 0,
        }
    }

    /// Calculate 2D launch configuration for matrix operations
    #[cfg(feature = "gpu-cuda")]
    fn launch_config_2d(&self, rows: usize, cols: usize) -> LaunchConfig {
        let tile_size = 16u32;
        let grid_x = ((cols as u32) + tile_size - 1) / tile_size;
        let grid_y = ((rows as u32) + tile_size - 1) / tile_size;
        LaunchConfig {
            grid_dim: (grid_x, grid_y, 1),
            block_dim: (tile_size, tile_size, 1),
            shared_mem_bytes: 0,
        }
    }

    // ========================================================================
    // Filter Operations
    // ========================================================================

    /// Execute a filter operation on i32 data
    pub fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let kernel_name = match operation {
                FilterOp::Equal => "filter_i32_eq",
                FilterOp::GreaterThan => "filter_i32_gt",
                FilterOp::GreaterOrEqual => "filter_i32_ge",
                FilterOp::LessThan => "filter_i32_lt",
                FilterOp::LessOrEqual => "filter_i32_le",
                FilterOp::NotEqual => "filter_i32_ne",
            };

            let len = data.len();
            let config = self.launch_config(len);

            // Allocate device memory
            let d_data = self.device.htod_sync_copy(data).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            // Get kernel function
            let func = self
                .device
                .get_func("database_kernels", kernel_name)
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: kernel_name.to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            // Launch kernel
            unsafe {
                func.launch(config, (&d_data, value, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: kernel_name.to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            // Copy results back
            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            // CPU fallback
            let result: Vec<i32> = data
                .iter()
                .map(|&x| match operation {
                    FilterOp::Equal => {
                        if x == value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterThan => {
                        if x > value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterOrEqual => {
                        if x >= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessThan => {
                        if x < value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessOrEqual => {
                        if x <= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::NotEqual => {
                        if x != value {
                            1
                        } else {
                            0
                        }
                    }
                })
                .collect();
            Ok(result)
        }
    }

    /// Execute a filter operation on i64 data
    pub fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let kernel_name = match operation {
                FilterOp::Equal => "filter_i64_eq",
                FilterOp::GreaterThan => "filter_i64_gt",
                FilterOp::GreaterOrEqual => "filter_i64_ge",
                FilterOp::LessThan => "filter_i64_lt",
                FilterOp::LessOrEqual => "filter_i64_le",
                FilterOp::NotEqual => "filter_i64_ne",
            };

            let len = data.len();
            let config = self.launch_config(len);

            let d_data = self.device.htod_sync_copy(data).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i64>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", kernel_name)
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: kernel_name.to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_data, value, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: kernel_name.to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<i32> = data
                .iter()
                .map(|&x| match operation {
                    FilterOp::Equal => {
                        if x == value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterThan => {
                        if x > value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterOrEqual => {
                        if x >= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessThan => {
                        if x < value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessOrEqual => {
                        if x <= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::NotEqual => {
                        if x != value {
                            1
                        } else {
                            0
                        }
                    }
                })
                .collect();
            Ok(result)
        }
    }

    /// Execute a filter operation on f64 data
    pub fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let kernel_name = match operation {
                FilterOp::Equal => "filter_f64_eq",
                FilterOp::GreaterThan => "filter_f64_gt",
                FilterOp::GreaterOrEqual => "filter_f64_ge",
                FilterOp::LessThan => "filter_f64_lt",
                FilterOp::LessOrEqual => "filter_f64_le",
                FilterOp::NotEqual => "filter_f64_ne",
            };

            let len = data.len();
            let config = self.launch_config(len);

            let d_data = self.device.htod_sync_copy(data).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<f64>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", kernel_name)
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: kernel_name.to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_data, value, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: kernel_name.to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<i32> = data
                .iter()
                .map(|&x| match operation {
                    FilterOp::Equal => {
                        if (x - value).abs() < 1e-10 {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterThan => {
                        if x > value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::GreaterOrEqual => {
                        if x >= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessThan => {
                        if x < value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::LessOrEqual => {
                        if x <= value {
                            1
                        } else {
                            0
                        }
                    }
                    FilterOp::NotEqual => {
                        if (x - value).abs() >= 1e-10 {
                            1
                        } else {
                            0
                        }
                    }
                })
                .collect();
            Ok(result)
        }
    }

    // ========================================================================
    // Bitmap Operations
    // ========================================================================

    /// Combine two filter masks with AND operation
    pub fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        use crate::errors::ExecutionError;

        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "mask_length".to_string(),
                    value: format!("mask_a={}, mask_b={}", mask_a.len(), mask_b.len()),
                },
            ));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let len = mask_a.len();
            let config = self.launch_config(len);

            let d_mask_a = self.device.htod_sync_copy(mask_a).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_mask_b = self.device.htod_sync_copy(mask_b).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "bitmap_and")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "bitmap_and".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_mask_a, &d_mask_b, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "bitmap_and".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<i32> = mask_a
                .iter()
                .zip(mask_b.iter())
                .map(|(&a, &b)| if a != 0 && b != 0 { 1 } else { 0 })
                .collect();
            Ok(result)
        }
    }

    /// Combine two filter masks with OR operation
    pub fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        use crate::errors::ExecutionError;

        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "mask_length".to_string(),
                    value: format!("mask_a={}, mask_b={}", mask_a.len(), mask_b.len()),
                },
            ));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let len = mask_a.len();
            let config = self.launch_config(len);

            let d_mask_a = self.device.htod_sync_copy(mask_a).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_mask_b = self.device.htod_sync_copy(mask_b).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "bitmap_or")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "bitmap_or".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_mask_a, &d_mask_b, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "bitmap_or".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<i32> = mask_a
                .iter()
                .zip(mask_b.iter())
                .map(|(&a, &b)| if a != 0 || b != 0 { 1 } else { 0 })
                .collect();
            Ok(result)
        }
    }

    /// Negate a filter mask with NOT operation
    pub fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let len = mask.len();
            let config = self.launch_config(len);

            let d_mask = self.device.htod_sync_copy(mask).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i32> = self.device.alloc_zeros(len).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "bitmap_not")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "bitmap_not".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_mask, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "bitmap_not".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<i32> = mask.iter().map(|&x| if x == 0 { 1 } else { 0 }).collect();
            Ok(result)
        }
    }

    // ========================================================================
    // Aggregation Operations
    // ========================================================================

    /// Sum aggregation for i32
    pub fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let len = data.len();
            let config = self.launch_config(len);

            let d_data = self.device.htod_sync_copy(data).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<i64> = self.device.alloc_zeros(1).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: std::mem::size_of::<i64>() as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "aggregate_i32_sum")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "aggregate_i32_sum".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_data, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_sum".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result[0])
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let sum: i64 = data.iter().map(|&x| x as i64).sum();
            Ok(sum)
        }
    }

    /// Count aggregation (count non-zero mask values)
    pub fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError> {
        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let len = mask.len();
            let config = self.launch_config(len);

            let d_mask = self.device.htod_sync_copy(mask).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (len * std::mem::size_of::<i32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_output: CudaSlice<u32> = self.device.alloc_zeros(1).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: std::mem::size_of::<u32>() as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "aggregate_i32_count")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "aggregate_i32_count".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_mask, &d_output, len as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_count".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_output).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result[0] as usize)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let count = mask.iter().filter(|&&x| x != 0).count();
            Ok(count)
        }
    }

    // ========================================================================
    // Vector Similarity Operations
    // ========================================================================

    /// Execute GPU-accelerated vector similarity calculation
    pub fn execute_vector_similarity(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[f32],
        _vector_count: usize,
        dimension: usize,
        kernel_name: &str,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        if query_vector.len() != dimension {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "query_vector_dimension".to_string(),
                    value: format!("expected {}, got {}", dimension, query_vector.len()),
                },
            ));
        }

        let vector_count = candidate_vectors.len() / dimension;

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let config = self.launch_config(vector_count);

            let d_query = self.device.htod_sync_copy(query_vector).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (dimension * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_candidates = self.device.htod_sync_copy(candidate_vectors).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (candidate_vectors.len() * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_scores: CudaSlice<f32> = self.device.alloc_zeros(vector_count).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (vector_count * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", kernel_name)
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: kernel_name.to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(
                    config,
                    (
                        &d_query,
                        &d_candidates,
                        &d_scores,
                        vector_count as u32,
                        dimension as u32,
                    ),
                )
                .map_err(|e| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: kernel_name.to_string(),
                        error: e.to_string(),
                    })
                })?;
            }

            let result = self.device.dtoh_sync_copy(&d_scores).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            // CPU fallback
            let mut scores = vec![0.0f32; vector_count];
            for i in 0..vector_count {
                let offset = i * dimension;
                let candidate = &candidate_vectors[offset..offset + dimension];

                match kernel_name {
                    "vector_cosine_similarity" => {
                        let dot: f32 = query_vector
                            .iter()
                            .zip(candidate.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                        let mag_q: f32 = query_vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                        let mag_c: f32 = candidate.iter().map(|x| x * x).sum::<f32>().sqrt();
                        scores[i] = if mag_q > 0.0 && mag_c > 0.0 {
                            dot / (mag_q * mag_c)
                        } else {
                            0.0
                        };
                    }
                    "vector_euclidean_distance" => {
                        let dist_sq: f32 = query_vector
                            .iter()
                            .zip(candidate.iter())
                            .map(|(a, b)| (a - b).powi(2))
                            .sum();
                        scores[i] = dist_sq.sqrt();
                    }
                    "vector_dot_product" => {
                        scores[i] = query_vector
                            .iter()
                            .zip(candidate.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                    }
                    _ => {}
                }
            }
            Ok(scores)
        }
    }

    // ========================================================================
    // Spatial Operations
    // ========================================================================

    /// Execute GPU-accelerated spatial distance calculation
    pub fn execute_spatial_distance(
        &self,
        query_x: f32,
        query_y: f32,
        candidates_x: &[f32],
        candidates_y: &[f32],
        point_count: usize,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        if candidates_x.len() != point_count || candidates_y.len() != point_count {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "candidates_length".to_string(),
                    value: format!(
                        "expected {}, got x={}, y={}",
                        point_count,
                        candidates_x.len(),
                        candidates_y.len()
                    ),
                },
            ));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let config = self.launch_config(point_count);

            let d_candidates_x = self.device.htod_sync_copy(candidates_x).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_candidates_y = self.device.htod_sync_copy(candidates_y).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_distances: CudaSlice<f32> =
                self.device.alloc_zeros(point_count).map_err(|e| {
                    ComputeError::gpu(GPUError::MemoryAllocationFailed {
                        size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                        error: e.to_string(),
                    })
                })?;

            let func = self
                .device
                .get_func("database_kernels", "spatial_distance")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "spatial_distance".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(
                    config,
                    (
                        query_x,
                        query_y,
                        &d_candidates_x,
                        &d_candidates_y,
                        &d_distances,
                        point_count as u32,
                    ),
                )
                .map_err(|e| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "spatial_distance".to_string(),
                        error: e.to_string(),
                    })
                })?;
            }

            let result = self.device.dtoh_sync_copy(&d_distances).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            let result: Vec<f32> = (0..point_count)
                .map(|i| {
                    let dx = candidates_x[i] - query_x;
                    let dy = candidates_y[i] - query_y;
                    (dx * dx + dy * dy).sqrt()
                })
                .collect();
            Ok(result)
        }
    }

    /// Execute GPU-accelerated Haversine distance calculation
    pub fn execute_spatial_distance_sphere(
        &self,
        query_lon: f32,
        query_lat: f32,
        candidates_lon: &[f32],
        candidates_lat: &[f32],
        point_count: usize,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        if candidates_lon.len() != point_count || candidates_lat.len() != point_count {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "candidates_length".to_string(),
                    value: format!(
                        "expected {}, got lon={}, lat={}",
                        point_count,
                        candidates_lon.len(),
                        candidates_lat.len()
                    ),
                },
            ));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let config = self.launch_config(point_count);

            let d_candidates_lon = self.device.htod_sync_copy(candidates_lon).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_candidates_lat = self.device.htod_sync_copy(candidates_lat).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_distances: CudaSlice<f32> =
                self.device.alloc_zeros(point_count).map_err(|e| {
                    ComputeError::gpu(GPUError::MemoryAllocationFailed {
                        size_bytes: (point_count * std::mem::size_of::<f32>()) as u64,
                        error: e.to_string(),
                    })
                })?;

            let func = self
                .device
                .get_func("database_kernels", "spatial_distance_sphere")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "spatial_distance_sphere".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(
                    config,
                    (
                        query_lon,
                        query_lat,
                        &d_candidates_lon,
                        &d_candidates_lat,
                        &d_distances,
                        point_count as u32,
                    ),
                )
                .map_err(|e| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "spatial_distance_sphere".to_string(),
                        error: e.to_string(),
                    })
                })?;
            }

            let result = self.device.dtoh_sync_copy(&d_distances).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            const EARTH_RADIUS_KM: f32 = 6371.0;
            let result: Vec<f32> = (0..point_count)
                .map(|i| {
                    let lat1_rad = query_lat.to_radians();
                    let lat2_rad = candidates_lat[i].to_radians();
                    let delta_lat = (candidates_lat[i] - query_lat).to_radians();
                    let delta_lon = (candidates_lon[i] - query_lon).to_radians();

                    let a = (delta_lat / 2.0).sin().powi(2)
                        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
                    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

                    EARTH_RADIUS_KM * c * 1000.0 // meters
                })
                .collect();
            Ok(result)
        }
    }

    // ========================================================================
    // Matrix Operations
    // ========================================================================

    /// Execute GPU-accelerated matrix multiplication (GEMM): C = A * B
    pub fn execute_matrix_multiply(
        &self,
        matrix_a: &[f32],
        matrix_b: &[f32],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<f32>, ComputeError> {
        use crate::errors::ExecutionError;

        if matrix_a.len() != m * k {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "matrix_a_size".to_string(),
                    value: format!("expected {}, got {}", m * k, matrix_a.len()),
                },
            ));
        }

        if matrix_b.len() != k * n {
            return Err(ComputeError::execution(
                ExecutionError::InvalidKernelParameters {
                    parameter: "matrix_b_size".to_string(),
                    value: format!("expected {}, got {}", k * n, matrix_b.len()),
                },
            ));
        }

        #[cfg(feature = "gpu-cuda")]
        {
            use crate::errors::GPUError;

            let config = self.launch_config_2d(m, n);

            let d_a = self.device.htod_sync_copy(matrix_a).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (matrix_a.len() * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_b = self.device.htod_sync_copy(matrix_b).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (matrix_b.len() * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let d_c: CudaSlice<f32> = self.device.alloc_zeros(m * n).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryAllocationFailed {
                    size_bytes: (m * n * std::mem::size_of::<f32>()) as u64,
                    error: e.to_string(),
                })
            })?;

            let func = self
                .device
                .get_func("database_kernels", "matrix_multiply_tiled_f32")
                .ok_or_else(|| {
                    ComputeError::gpu(GPUError::KernelLaunchFailed {
                        kernel_name: "matrix_multiply_tiled_f32".to_string(),
                        error: "Kernel function not found".to_string(),
                    })
                })?;

            unsafe {
                func.launch(config, (&d_a, &d_b, &d_c, m as u32, n as u32, k as u32))
                    .map_err(|e| {
                        ComputeError::gpu(GPUError::KernelLaunchFailed {
                            kernel_name: "matrix_multiply_tiled_f32".to_string(),
                            error: e.to_string(),
                        })
                    })?;
            }

            let result = self.device.dtoh_sync_copy(&d_c).map_err(|e| {
                ComputeError::gpu(GPUError::MemoryTransferFailed {
                    direction: "device to host".to_string(),
                    error: e.to_string(),
                })
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "gpu-cuda"))]
        {
            // CPU fallback using naive O(n^3) algorithm
            let mut result = vec![0.0f32; m * n];
            for i in 0..m {
                for j in 0..n {
                    let mut sum = 0.0f32;
                    for kk in 0..k {
                        sum += matrix_a[i * k + kk] * matrix_b[kk * n + j];
                    }
                    result[i * n + j] = sum;
                }
            }
            Ok(result)
        }
    }
}

// Implement GpuDevice trait for CudaDevice
impl GpuDevice for CudaDevice {
    fn backend_type(&self) -> GpuBackendType {
        GpuBackendType::Cuda
    }

    fn device_name(&self) -> String {
        self.device_name.clone()
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "gpu-cuda")]
        {
            self.module_loaded
        }
        #[cfg(not(feature = "gpu-cuda"))]
        {
            true
        }
    }

    fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::execute_filter_i32(self, data, value, operation)
    }

    fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::execute_filter_i64(self, data, value, operation)
    }

    fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::execute_filter_f64(self, data, value, operation)
    }

    fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::bitmap_and(self, mask_a, mask_b)
    }

    fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::bitmap_or(self, mask_a, mask_b)
    }

    fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError> {
        CudaDevice::bitmap_not(self, mask)
    }

    fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError> {
        CudaDevice::aggregate_sum_i32(self, data)
    }

    fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError> {
        CudaDevice::aggregate_count(self, mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cuda_device_creation() {
        match CudaDevice::new() {
            Ok(device) => {
                println!("CUDA device: {}", device.device_name());
                assert!(device.is_available());
            }
            Err(e) => {
                println!("No CUDA device available: {}", e);
                // This is ok - not all systems have CUDA GPUs
            }
        }
    }

    #[test]
    fn test_cuda_availability_check() {
        let is_available = CudaDevice::is_cuda_available();
        println!("CUDA availability check: {}", is_available);

        #[cfg(target_os = "linux")]
        {
            println!("Platform: Linux");
        }

        #[cfg(target_os = "windows")]
        {
            println!("Platform: Windows");
        }

        #[cfg(target_os = "macos")]
        {
            println!("Platform: macOS (CUDA not supported)");
            assert!(!is_available, "CUDA should not be available on macOS");
        }
    }

    #[test]
    fn test_filter_i32_eq() {
        let device = match CudaDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping CUDA test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (0..1000).collect();
        let result = device
            .execute_filter_i32(&data, 500, FilterOp::Equal)
            .unwrap();

        let matches: i32 = result.iter().sum();
        assert_eq!(matches, 1, "Should find exactly one match for value 500");
    }

    #[test]
    fn test_filter_i32_gt() {
        let device = match CudaDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping CUDA test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (0..100).collect();
        let result = device
            .execute_filter_i32(&data, 50, FilterOp::GreaterThan)
            .unwrap();

        let matches: i32 = result.iter().sum();
        assert_eq!(matches, 49, "Values 51-99 should match (49 values)");
    }

    #[test]
    fn test_bitmap_and() {
        let device = match CudaDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping CUDA test - no device available");
                return;
            }
        };

        let mask_a = vec![1, 1, 0, 0, 1, 1];
        let mask_b = vec![1, 0, 1, 0, 1, 0];
        let result = device.bitmap_and(&mask_a, &mask_b).unwrap();

        assert_eq!(result, vec![1, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn test_aggregate_sum_i32() {
        let device = match CudaDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping CUDA test - no device available");
                return;
            }
        };

        let data: Vec<i32> = (1..=100).collect();
        let result = device.aggregate_sum_i32(&data).unwrap();

        assert_eq!(result, 5050);
    }

    #[test]
    fn test_aggregate_count() {
        let device = match CudaDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping CUDA test - no device available");
                return;
            }
        };

        let mask = vec![1, 0, 1, 1, 0, 1, 0, 0, 1, 1];
        let result = device.aggregate_count(&mask).unwrap();

        assert_eq!(result, 6, "Should count 6 non-zero values");
    }
}
