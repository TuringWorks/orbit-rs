//! Unified GPU backend abstraction for cross-platform GPU acceleration
//!
//! This module provides a unified interface for GPU operations that works across
//! multiple GPU APIs: Metal (macOS), CUDA (NVIDIA), ROCm (AMD), and Vulkan (cross-platform).

use crate::errors::ComputeError;

/// Filter comparison operations supported by all GPU backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Equal,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    NotEqual,
}

/// GPU backend types available on different platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackendType {
    /// Apple Metal (macOS, iOS)
    Metal,
    /// NVIDIA CUDA (Linux, Windows)
    Cuda,
    /// AMD ROCm (Linux)
    Rocm,
    /// Cross-platform Vulkan compute
    Vulkan,
}

/// Unified GPU device trait that all backends must implement
pub trait GpuDevice: Send + Sync {
    /// Get the backend type
    fn backend_type(&self) -> GpuBackendType;

    /// Get device name
    fn device_name(&self) -> String;

    /// Check if device is available
    fn is_available(&self) -> bool;

    /// Execute filter operation on i32 data
    fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError>;

    /// Execute filter operation on i64 data
    fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError>;

    /// Execute filter operation on f64 data
    fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError>;

    /// Combine two filter masks with AND operation
    fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError>;

    /// Combine two filter masks with OR operation
    fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError>;

    /// Negate a filter mask with NOT operation
    fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError>;

    /// Sum aggregation for i32
    fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError>;

    /// Count aggregation (count non-zero mask values)
    fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError>;
}

/// GPU device manager that selects the best available backend
pub struct GpuDeviceManager {
    available_backends: Vec<GpuBackendType>,
}

impl GpuDeviceManager {
    /// Create a new GPU device manager and detect available backends
    pub fn new() -> Self {
        let mut available_backends = Vec::new();

        #[cfg(target_os = "macos")]
        {
            available_backends.push(GpuBackendType::Metal);
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            // Try CUDA (NVIDIA)
            if Self::is_cuda_available() {
                available_backends.push(GpuBackendType::Cuda);
            }

            // Try ROCm (AMD)
            if Self::is_rocm_available() {
                available_backends.push(GpuBackendType::Rocm);
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Try CUDA (NVIDIA)
            if Self::is_cuda_available() {
                available_backends.push(GpuBackendType::Cuda);
            }
        }

        // Vulkan is available on all platforms as fallback
        if Self::is_vulkan_available() {
            available_backends.push(GpuBackendType::Vulkan);
        }

        tracing::info!("Available GPU backends: {:?}", available_backends);

        Self { available_backends }
    }

    /// Get list of available backends
    pub fn available_backends(&self) -> &[GpuBackendType] {
        &self.available_backends
    }

    /// Create a GPU device using the best available backend
    pub fn create_device(&self) -> Result<Box<dyn GpuDevice>, ComputeError> {
        for backend in &self.available_backends {
            match self.create_device_for_backend(*backend) {
                Ok(device) => {
                    tracing::info!("Selected GPU backend: {:?}", backend);
                    return Ok(device);
                }
                Err(e) => {
                    tracing::warn!("Failed to create {:?} device: {}", backend, e);
                    continue;
                }
            }
        }

        Err(ComputeError::gpu(crate::errors::GPUError::DeviceNotFound {
            device_id: 0,
        }))
    }

    /// Create a device for a specific backend
    pub fn create_device_for_backend(
        &self,
        backend: GpuBackendType,
    ) -> Result<Box<dyn GpuDevice>, ComputeError> {
        match backend {
            #[cfg(target_os = "macos")]
            GpuBackendType::Metal => {
                use crate::gpu_metal::MetalDevice;
                Ok(Box::new(MetalDevice::new()?))
            }

            #[cfg(not(target_os = "macos"))]
            GpuBackendType::Metal => Err(ComputeError::gpu(
                crate::errors::GPUError::APIInitializationFailed {
                    api: "Metal".to_string(),
                    error: "Metal is only available on macOS".to_string(),
                },
            )),

            GpuBackendType::Cuda => {
                #[cfg(feature = "gpu-cuda")]
                {
                    use crate::gpu_cuda::CudaDevice;
                    Ok(Box::new(CudaDevice::new()?))
                }
                #[cfg(not(feature = "gpu-cuda"))]
                Err(ComputeError::gpu(
                    crate::errors::GPUError::APIInitializationFailed {
                        api: "CUDA".to_string(),
                        error: "CUDA support not compiled in. Enable 'gpu-cuda' feature"
                            .to_string(),
                    },
                ))
            }

            GpuBackendType::Rocm => {
                #[cfg(feature = "gpu-rocm")]
                {
                    use crate::gpu_rocm::RocmDevice;
                    Ok(Box::new(RocmDevice::new()?))
                }
                #[cfg(not(feature = "gpu-rocm"))]
                Err(ComputeError::gpu(
                    crate::errors::GPUError::APIInitializationFailed {
                        api: "ROCm".to_string(),
                        error: "ROCm support not compiled in. Enable 'gpu-rocm' feature"
                            .to_string(),
                    },
                ))
            }

            GpuBackendType::Vulkan => {
                #[cfg(feature = "gpu-vulkan")]
                {
                    use crate::gpu_vulkan::VulkanDevice;
                    Ok(Box::new(VulkanDevice::new()?))
                }
                #[cfg(not(feature = "gpu-vulkan"))]
                Err(ComputeError::gpu(
                    crate::errors::GPUError::APIInitializationFailed {
                        api: "Vulkan".to_string(),
                        error: "Vulkan support not compiled in. Enable 'gpu-vulkan' feature"
                            .to_string(),
                    },
                ))
            }
        }
    }

    // Platform-specific availability checks

    #[cfg(all(unix, not(target_os = "macos")))]
    fn is_cuda_available() -> bool {
        // Check for CUDA runtime library
        std::path::Path::new("/usr/local/cuda").exists()
            || std::env::var("CUDA_PATH").is_ok()
            || which::which("nvcc").is_ok()
    }

    #[cfg(target_os = "windows")]
    fn is_cuda_available() -> bool {
        // Check for CUDA on Windows
        std::env::var("CUDA_PATH").is_ok() || which::which("nvcc.exe").is_ok()
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    fn is_cuda_available() -> bool {
        false
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn is_rocm_available() -> bool {
        // Check for ROCm installation
        std::path::Path::new("/opt/rocm").exists() || which::which("rocm-smi").is_ok()
    }

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    #[allow(dead_code)] // Reserved for future ROCm support
    fn is_rocm_available() -> bool {
        false
    }

    fn is_vulkan_available() -> bool {
        // Vulkan is generally available on most modern systems
        // We'll do a runtime check when creating the device
        true
    }
}

impl Default for GpuDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_manager_creation() {
        let manager = GpuDeviceManager::new();
        println!("Available backends: {:?}", manager.available_backends());

        // Should have at least one backend available
        assert!(
            !manager.available_backends().is_empty(),
            "At least one GPU backend should be available"
        );
    }

    #[test]
    fn test_device_creation() {
        let manager = GpuDeviceManager::new();

        match manager.create_device() {
            Ok(device) => {
                println!("Created GPU device: {}", device.device_name());
                println!("Backend: {:?}", device.backend_type());
                assert!(device.is_available());
            }
            Err(e) => {
                println!("No GPU device available: {}", e);
                // This is ok - not all systems have GPUs
            }
        }
    }
}
