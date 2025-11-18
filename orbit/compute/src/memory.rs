//! Memory management optimizations for accelerated computing
//!
//! This module provides memory management utilities with graceful degradation
//! across different platforms and architectures.

use crate::errors::{ComputeError, ComputeResult, MemoryError};

/// Memory allocator with platform-specific optimizations
#[derive(Debug)]
pub struct AcceleratedMemoryAllocator {
    /// Whether unified memory is available (Apple Silicon)
    unified_memory_available: bool,
    /// Memory alignment requirements
    alignment_bytes: usize,
    /// Platform-specific optimizations
    #[allow(dead_code)]
    optimizations: MemoryOptimizations,
}

/// Platform-specific memory optimizations
#[derive(Debug, Clone)]
pub struct MemoryOptimizations {
    /// Use memory mapping when available
    pub use_memory_mapping: bool,
    /// Enable NUMA awareness on supported platforms
    pub numa_aware: bool,
    /// Use large pages when available
    pub use_large_pages: bool,
    /// Enable memory prefetching
    pub enable_prefetch: bool,
}

impl Default for MemoryOptimizations {
    fn default() -> Self {
        Self {
            use_memory_mapping: true,
            numa_aware: cfg!(target_os = "linux"),
            use_large_pages: cfg!(any(target_os = "linux", target_os = "windows")),
            enable_prefetch: true,
        }
    }
}

impl AcceleratedMemoryAllocator {
    /// Create a new memory allocator with platform detection
    pub fn new() -> ComputeResult<Self> {
        let unified_memory_available = Self::detect_unified_memory();
        let alignment_bytes = Self::detect_optimal_alignment();
        let optimizations = MemoryOptimizations::default();

        tracing::info!(
            "Memory allocator initialized: unified={}, alignment={}",
            unified_memory_available,
            alignment_bytes
        );

        Ok(Self {
            unified_memory_available,
            alignment_bytes,
            optimizations,
        })
    }

    /// Detect if unified memory is available (Apple Silicon)
    fn detect_unified_memory() -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, check if we're running on Apple Silicon
            // This is a simplified check - real implementation would use syscalls
            std::env::consts::ARCH == "aarch64"
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Detect optimal memory alignment for the platform
    fn detect_optimal_alignment() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            // AVX-512 requires 64-byte alignment
            64
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM NEON typically uses 16-byte alignment
            16
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // Safe fallback
            8
        }
    }

    /// Allocate aligned memory with graceful fallback
    pub fn allocate_aligned(&self, size: usize) -> ComputeResult<AlignedMemory> {
        if size == 0 {
            return Err(ComputeError::memory(
                MemoryError::OutOfMemory {
                    requested: 0,
                    available: 0,
                },
                Some(size),
            ));
        }

        // Try optimal allocation first
        match self.try_optimal_allocation(size) {
            Ok(memory) => Ok(memory),
            Err(e) => {
                tracing::warn!("Optimal allocation failed, falling back: {}", e);
                self.fallback_allocation(size)
            }
        }
    }

    /// Try optimal allocation with platform-specific features
    fn try_optimal_allocation(&self, size: usize) -> ComputeResult<AlignedMemory> {
        // Platform-specific allocation strategies
        #[cfg(target_os = "macos")]
        {
            if self.unified_memory_available {
                return self.allocate_unified_memory(size);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if self.optimizations.use_large_pages {
                if let Ok(memory) = self.try_large_page_allocation(size) {
                    return Ok(memory);
                }
            }
        }

        // Standard aligned allocation
        self.allocate_standard_aligned(size)
    }

    /// Fallback allocation with minimal requirements
    fn fallback_allocation(&self, size: usize) -> ComputeResult<AlignedMemory> {
        tracing::info!("Using fallback memory allocation for {} bytes", size);

        // Simple aligned allocation without platform optimizations
        let layout = std::alloc::Layout::from_size_align(size, 8).map_err(|_| {
            ComputeError::memory(
                MemoryError::AlignmentError {
                    required_alignment: 8,
                    actual_alignment: 0,
                },
                Some(size),
            )
        })?;

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(ComputeError::memory(
                MemoryError::OutOfMemory {
                    requested: size,
                    available: 0,
                },
                Some(size),
            ));
        }

        Ok(AlignedMemory {
            ptr,
            size,
            layout,
            is_unified: false,
            is_large_page: false,
        })
    }

    /// Allocate unified memory (Apple Silicon)
    #[cfg(target_os = "macos")]
    fn allocate_unified_memory(&self, size: usize) -> ComputeResult<AlignedMemory> {
        // This would use Apple's unified memory APIs
        // For now, fall back to standard allocation
        self.allocate_standard_aligned(size)
    }

    /// Try large page allocation (Linux/Windows)
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn try_large_page_allocation(&self, size: usize) -> ComputeResult<AlignedMemory> {
        // This would use platform-specific large page APIs
        // For now, fall back to standard allocation
        self.allocate_standard_aligned(size)
    }

    /// Standard aligned allocation
    fn allocate_standard_aligned(&self, size: usize) -> ComputeResult<AlignedMemory> {
        let alignment = self.alignment_bytes.max(8);
        let layout = std::alloc::Layout::from_size_align(size, alignment).map_err(|_| {
            ComputeError::memory(
                MemoryError::AlignmentError {
                    required_alignment: alignment,
                    actual_alignment: 0,
                },
                Some(size),
            )
        })?;

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(ComputeError::memory(
                MemoryError::OutOfMemory {
                    requested: size,
                    available: 0,
                },
                Some(size),
            ));
        }

        Ok(AlignedMemory {
            ptr,
            size,
            layout,
            is_unified: self.unified_memory_available,
            is_large_page: false,
        })
    }
}

/// Aligned memory allocation
#[derive(Debug)]
pub struct AlignedMemory {
    /// Memory pointer
    ptr: *mut u8,
    /// Allocation size
    size: usize,
    /// Memory layout
    layout: std::alloc::Layout,
    /// Whether this is unified memory
    is_unified: bool,
    /// Whether large pages were used
    is_large_page: bool,
}

impl AlignedMemory {
    /// Get the memory pointer
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Get the allocation size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if unified memory is used
    pub fn is_unified(&self) -> bool {
        self.is_unified
    }

    /// Check if large pages are used
    pub fn is_large_page(&self) -> bool {
        self.is_large_page
    }
}

impl Drop for AlignedMemory {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.ptr, self.layout);
        }
    }
}

// Safe wrappers for different architectures
unsafe impl Send for AlignedMemory {}
unsafe impl Sync for AlignedMemory {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocator_creation() {
        let allocator = AcceleratedMemoryAllocator::new();
        assert!(allocator.is_ok());
    }

    #[test]
    fn test_memory_allocation() {
        let allocator = AcceleratedMemoryAllocator::new().unwrap();
        let memory = allocator.allocate_aligned(1024);
        assert!(memory.is_ok());

        let mem = memory.unwrap();
        assert_eq!(mem.size(), 1024);
        assert!(!mem.as_ptr().is_null());
    }

    #[test]
    fn test_zero_allocation() {
        let allocator = AcceleratedMemoryAllocator::new().unwrap();
        let memory = allocator.allocate_aligned(0);
        assert!(memory.is_err());
    }
}
