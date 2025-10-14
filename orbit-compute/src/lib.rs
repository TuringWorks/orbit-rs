//! # Orbit Compute - Heterogeneous Acceleration Engine
//!
//! This crate provides comprehensive heterogeneous computing acceleration for Orbit-RS,
//! including SIMD CPU optimizations, GPU acceleration, and Neural Engine support across
//! different hardware platforms.
//!
//! ## Supported Platforms
//!
//! ### CPU Acceleration
//! - **x86-64**: AVX2, AVX-512 SIMD instructions
//! - **ARM64**: NEON SIMD instructions, SVE (Scalable Vector Extensions)
//!
//! ### GPU Acceleration
//! - **Apple Silicon**: Metal compute shaders with unified memory
//! - **NVIDIA**: CUDA with cuDNN integration
//! - **AMD**: ROCm/HIP with ROCblas
//! - **Cross-platform**: OpenCL and Vulkan compute
//! - **Snapdragon**: Adreno GPU with OpenCL
//!
//! ### Neural Engine Acceleration
//! - **Apple Neural Engine**: Core ML integration with ANE optimization
//! - **Snapdragon AI Engine**: Hexagon DSP with tensor acceleration
//! - **Intel Neural Compute**: OpenVINO integration
//!
//! ## Architecture
//!
//! The acceleration engine uses a three-tier approach:
//! 1. **Runtime Detection**: Automatically detect available hardware capabilities
//! 2. **Workload Analysis**: Analyze queries for optimal acceleration strategy
//! 3. **Adaptive Execution**: Route operations to the best available compute unit
//!
//! ## Usage
//!
//! ```rust,no_run
//! use orbit_compute::{HeterogeneousEngine, create_acceleration_engine};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create acceleration engine with automatic capability detection
//!     let engine = create_acceleration_engine().await?;
//!     
//!     // Get engine status to see what capabilities are available
//!     let status = engine.get_engine_status().await;
//!     println!("Available compute units: {}", status.available_compute_units);
//!     
//!     Ok(())
//! }
//! ```

#![allow(missing_docs)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]

// Re-export key types for convenient access
pub use capabilities::{
    CPUArchitecture, CPUCapabilities, GPUCapabilities, GPUDevice, NPUCapabilities,
    NeuralEngineCapabilities, UniversalComputeCapabilities,
};
pub use engine::{EngineConfig, EngineStatus, HeterogeneousEngine};
pub use errors::{ComputeError, ComputeResult};
pub use monitoring::{MockSystemMonitorState, SystemInfo, SystemMonitor};
pub use query::{AccelerationStrategy, QueryAnalysis};
pub use scheduler::{AdaptiveWorkloadScheduler, ComputeUnit, WorkloadType};

#[cfg(feature = "benchmarks")]
pub use benchmarks::{
    BenchmarkConfig, BenchmarkReport, BenchmarkResult, BenchmarkSuite, BenchmarkSummary,
};

/// Hardware capability detection and runtime feature discovery
pub mod capabilities;

/// Error types for heterogeneous computing operations
pub mod errors;

/// Query analysis and workload characterization
pub mod query;

/// Workload scheduling and optimal hardware selection
pub mod scheduler;

/// System monitoring and resource tracking
pub mod monitoring;

/// Main heterogeneous acceleration engine
pub mod engine;

/// CPU SIMD acceleration implementations
#[cfg(feature = "cpu-simd")]
pub mod cpu;

/// GPU acceleration implementations
#[cfg(feature = "gpu-acceleration")]
pub mod gpu;

/// Neural engine acceleration implementations  
#[cfg(feature = "neural-acceleration")]
pub mod neural;

/// Memory management optimizations for accelerated computing
pub mod memory;

/// Benchmarking utilities for performance validation
#[cfg(any(feature = "criterion", feature = "benchmarks"))]
pub mod benchmarks;

// TODO: Testing utilities and mock implementations
// This would contain testing utilities
// #[cfg(test)]
// pub mod testing;

// Platform-specific modules
#[cfg(target_os = "macos")]
pub mod apple;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

// Architecture-specific optimizations
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

/// Initialize the heterogeneous compute engine with runtime detection
///
/// This performs comprehensive hardware detection and initializes all available
/// acceleration backends. It's recommended to call this once at application startup.
///
/// # Example
///
/// ```rust,no_run
/// use orbit_compute::init_heterogeneous_compute;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let capabilities = init_heterogeneous_compute().await?;
///     println!("Available acceleration: {:?}", capabilities);
///     Ok(())
/// }
/// ```
pub async fn init_heterogeneous_compute() -> Result<UniversalComputeCapabilities, ComputeError> {
    tracing::info!("Initializing heterogeneous compute acceleration");

    let capabilities = capabilities::detect_all_capabilities().await?;

    tracing::info!(
        "Detected capabilities: CPU={:?}, GPU={:?}, Neural={:?}",
        capabilities.cpu.architecture,
        capabilities.gpu.available_devices.len(),
        capabilities.neural
    );

    Ok(capabilities)
}

/// Convenience function to create a new heterogeneous acceleration engine
///
/// This function combines capability detection and engine initialization into a
/// single call for ease of use.
pub async fn create_acceleration_engine() -> Result<HeterogeneousEngine, ComputeError> {
    let capabilities = init_heterogeneous_compute().await?;
    HeterogeneousEngine::new_with_capabilities(capabilities).await
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_capability_detection() {
        let capabilities = init_heterogeneous_compute().await.unwrap();

        // Should always have some CPU capabilities
        assert!(matches!(
            capabilities.cpu.architecture,
            CPUArchitecture::X86_64 { .. } | CPUArchitecture::AArch64 { .. }
        ));
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = create_acceleration_engine().await;
        assert!(engine.is_ok(), "Engine creation should succeed");
    }
}
