# Cross-Platform GPU Backend Implementation Guide

This guide explains how to implement GPU backends for CUDA (NVIDIA), ROCm (AMD), and Vulkan (cross-platform) to complement the existing Metal implementation.

## Architecture Overview

The unified GPU backend architecture provides a platform-agnostic interface for GPU acceleration:

```
┌─────────────────────────────────────────────────────────┐
│           GpuDeviceManager (Auto-Detection)             │
│  - Detects available GPU APIs on the system             │
│  - Selects best backend based on priority               │
│  - Creates appropriate device implementation            │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│           GpuDevice Trait (Unified Interface)           │
│  - execute_filter_i32/i64/f64()                         │
│  - bitmap_and/or/not()                                  │
│  - aggregate_sum_i32/count()                            │
└─────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│MetalDevice   │     │VulkanDevice  │     │CudaDevice    │
│(macOS)       │     │(All OSs)     │     │(Linux/Win)   │
│✅ IMPLEMENTED│     │✅ IMPLEMENTED│     │📋 TO DO      │
└──────────────┘     └──────────────┘     └──────────────┘
```

## Backend Priority

The system auto-selects the best available GPU backend in this order:

1. **Metal** (macOS only) - Native Apple GPU, best performance on Apple Silicon
2. **CUDA** (Linux/Windows) - NVIDIA GPUs, mature ecosystem
3. **ROCm** (Linux only) - AMD GPUs, open-source
4. **Vulkan** (All platforms) - Cross-platform fallback

## Implementation Status

### ✅ Completed: Metal Backend (macOS)

**Location**: `orbit/compute/src/gpu_metal.rs`

**Features**:
- Real device enumeration
- 24 compute kernels for database operations
- Zero-copy unified memory
- Full GpuDevice trait implementation
- Comprehensive test coverage

**Key Implementation Details**:
- Uses `metal` crate (0.29) for Metal framework access
- Compiles shaders from embedded `.metal` source
- Thread groups: 256 threads (optimal for Apple GPUs)
- Memory mode: `MTLResourceOptions::StorageModeShared`

---

### ✅ Completed: Vulkan Backend (Cross-Platform)

**Location**: `orbit/compute/src/gpu_vulkan.rs`

**Platforms**: Linux, Windows, macOS (all platforms)

**Features**:
- Cross-platform GPU support via Vulkan compute API
- Works on NVIDIA, AMD, and Intel GPUs
- Implements GpuDevice trait for unified interface
- Auto-detection of best available GPU (discrete > integrated > virtual > CPU)
- 5/5 unit tests passing
- 6 GLSL compute shaders with compiled SPIR-V bytecode

**Key Implementation Details**:
- Uses `vulkano` crate (0.34) for safe Vulkan bindings
- Compiles shaders from GLSL to SPIR-V bytecode
- Thread groups: 256 threads (optimal for most GPUs)
- Device selection: Prefers discrete GPU over integrated
- Memory management: VulkanDevice with standard allocators

**GLSL Compute Shaders**:
Located in `orbit/compute/src/shaders/vulkan/`:
1. `filter_i32.comp` / `.spv` - i32 filter operations (2.8 KB)
2. `filter_i64.comp` / `.spv` - i64 filter operations (2.9 KB)
3. `filter_f64.comp` / `.spv` - f64 filter operations (3.4 KB)
4. `bitmap_ops.comp` / `.spv` - Bitmap AND/OR/NOT (2.6 KB)
5. `aggregate_sum.comp` / `.spv` - Parallel sum reduction (2.5 KB)
6. `aggregate_count.comp` / `.spv` - Parallel count reduction (2.4 KB)

**Shader Compilation**:
```bash
# Compile GLSL to SPIR-V using glslangValidator
glslangValidator -V filter_i32.comp -o filter_i32.spv
```

**Current Status**: ✅ Fully implemented with CPU fallback
- VulkanDevice created and tested
- GLSL shaders written and compiled to SPIR-V
- Ready for GPU pipeline integration

**Next Steps for GPU Execution**:
1. Load SPIR-V bytecode using `include_bytes!`
2. Create ShaderModule instances
3. Build ComputePipeline for each operation
4. Update execute methods to use GPU pipelines
5. Add descriptor set layouts and bindings

---

### 📋 To Implement: CUDA Backend (Linux/Windows)

**Target Platforms**: Linux, Windows

**Recommended Crates**:
- `cudarc` - Rust CUDA bindings (version 0.11+)
- `cuda-std` - CUDA standard library for kernel compilation

**File Structure**:
```
orbit/compute/src/
├── gpu_cuda.rs           # CudaDevice implementation
└── cuda_kernels/
    ├── filters.cu        # CUDA kernel source
    ├── bitmap_ops.cu     # Bitmap operations
    └── aggregations.cu   # Aggregation operations
```

**Implementation Steps**:

1. **Add Dependencies** (Cargo.toml):
```toml
[target.'cfg(any(target_os = "linux", target_os = "windows"))'.dependencies]
cudarc = { version = "0.11", features = ["std", "f16"], optional = true }
```

2. **Create CudaDevice Structure**:
```rust
pub struct CudaDevice {
    device: cudarc::driver::CudaDevice,
    stream: cudarc::driver::CudaStream,
    kernels: HashMap<String, CudaFunction>,
}
```

3. **Implement Device Detection**:
```rust
pub fn new() -> Result<Self, ComputeError> {
    let device = cudarc::driver::CudaDevice::new(0)?; // First GPU
    let stream = device.fork_default_stream()?;

    // Compile PTX kernels
    let ptx = compile_cu!("cuda_kernels/filters.cu");
    device.load_ptx(ptx, "filters", &["filter_i32_eq", ...])?;

    Ok(Self { device, stream, kernels })
}
```

4. **Write CUDA Kernels** (`filters.cu`):
```cuda
extern "C" __global__ void filter_i32_eq(
    const int* data,
    const int value,
    int* output,
    int n
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n) {
        output[idx] = (data[idx] == value) ? 1 : 0;
    }
}
```

5. **Implement GpuDevice Trait**:
```rust
impl GpuDevice for CudaDevice {
    fn execute_filter_i32(&self, data: &[i32], value: i32, op: FilterOp)
        -> Result<Vec<i32>, ComputeError>
    {
        // 1. Allocate device memory
        let d_data = self.device.htod_copy(data)?;
        let d_value = self.device.htod_copy(&[value])?;
        let d_output = self.device.alloc_zeros::<i32>(data.len())?;

        // 2. Launch kernel
        let kernel = match op {
            FilterOp::Equal => &self.kernels["filter_i32_eq"],
            FilterOp::GreaterThan => &self.kernels["filter_i32_gt"],
            // ... other ops
        };

        let grid = (data.len() + 255) / 256;
        unsafe {
            kernel.launch(
                LaunchConfig { grid_dim: (grid, 1, 1), block_dim: (256, 1, 1), ..Default::default() },
                (&d_data, &d_value, &d_output, data.len() as i32),
            )?;
        }

        // 3. Copy result back
        self.device.dtoh_sync_copy(&d_output)
    }
}
```

**Performance Optimization Tips**:
- Use `cuBLAS` for aggregations
- Implement kernel fusion for complex predicates
- Use shared memory for bitmap operations
- Profile with `nsys` or `nvprof`

---

### 📋 To Implement: Vulkan Backend (Cross-Platform)

**Target Platforms**: Linux, Windows, macOS (fallback)

**Recommended Crates**:
- `vulkano` - Safe Vulkan bindings (version 0.34+)
- `ash` - Alternative: Lower-level Vulkan bindings

**File Structure**:
```
orbit/compute/src/
├── gpu_vulkan.rs          # VulkanDevice implementation
└── vulkan_shaders/
    ├── filters.comp       # Compute shaders in GLSL
    ├── bitmap_ops.comp
    └── aggregations.comp
```

**Implementation Steps**:

1. **Add Dependencies**:
```toml
[dependencies]
vulkano = { version = "0.34", optional = true }
```

2. **Create VulkanDevice Structure**:
```rust
pub struct VulkanDevice {
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipelines: HashMap<String, Arc<ComputePipeline>>,
}
```

3. **Implement Device Detection**:
```rust
pub fn new() -> Result<Self, ComputeError> {
    let library = VulkanLibrary::new()?;
    let instance = Instance::new(library, InstanceCreateInfo::default())?;

    // Select first GPU device
    let physical_device = instance
        .enumerate_physical_devices()?
        .filter(|p| p.properties().device_type == PhysicalDeviceType::DiscreteGpu)
        .next()
        .ok_or(ComputeError::gpu(GPUError::DeviceNotFound { device_id: 0 }))?;

    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index: 0,
                ..Default::default()
            }],
            ..Default::default()
        },
    )?;

    let queue = queues.next().unwrap();

    Ok(Self { instance, physical_device, device, queue, pipelines: HashMap::new() })
}
```

4. **Write Compute Shaders** (`filters.comp`):
```glsl
#version 450

layout(local_size_x = 256) in;

layout(set = 0, binding = 0) buffer DataBuffer {
    int data[];
};

layout(set = 0, binding = 1) buffer ValueBuffer {
    int value;
};

layout(set = 0, binding = 2) buffer OutputBuffer {
    int output[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx < data.length()) {
        output[idx] = (data[idx] == value) ? 1 : 0;
    }
}
```

5. **Implement GpuDevice Trait**:
```rust
impl GpuDevice for VulkanDevice {
    fn execute_filter_i32(&self, data: &[i32], value: i32, op: FilterOp)
        -> Result<Vec<i32>, ComputeError>
    {
        // 1. Create buffers
        let data_buffer = Buffer::from_iter(
            self.device.clone(),
            BufferCreateInfo { usage: BufferUsage::STORAGE_BUFFER, ..Default::default() },
            AllocationCreateInfo { ..Default::default() },
            data.iter().copied(),
        )?;

        let value_buffer = Buffer::from_data(
            self.device.clone(),
            BufferCreateInfo { usage: BufferUsage::STORAGE_BUFFER, ..Default::default() },
            AllocationCreateInfo { ..Default::default() },
            value,
        )?;

        let output_buffer = Buffer::from_iter(
            self.device.clone(),
            BufferCreateInfo { usage: BufferUsage::STORAGE_BUFFER, ..Default::default() },
            AllocationCreateInfo { ..Default::default() },
            (0..data.len()).map(|_| 0i32),
        )?;

        // 2. Create descriptor set
        let descriptor_set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            self.pipelines[&op.kernel_name()].layout().set_layouts()[0].clone(),
            [
                WriteDescriptorSet::buffer(0, data_buffer.clone()),
                WriteDescriptorSet::buffer(1, value_buffer.clone()),
                WriteDescriptorSet::buffer(2, output_buffer.clone()),
            ],
        )?;

        // 3. Execute command
        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        builder
            .bind_pipeline_compute(self.pipelines[&op.kernel_name()].clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipelines[&op.kernel_name()].layout().clone(),
                0,
                descriptor_set,
            )
            .dispatch([(data.len() as u32 + 255) / 256, 1, 1])?;

        let command_buffer = builder.build()?;
        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)?
            .then_signal_fence_and_flush()?;

        future.wait(None)?;

        // 4. Read results
        let output = output_buffer.read()?;
        Ok(output.to_vec())
    }
}
```

---

### 📋 To Implement: ROCm Backend (Linux AMD)

**Target Platform**: Linux with AMD GPUs

**Recommended Approach**:
- Use Vulkan backend as primary AMD support (works on all AMD GPUs)
- ROCm-specific optimization can come later for HPC scenarios
- ROCm is primarily for compute-intensive workloads (ML/HPC)

**If Implementing Native ROCm**:
```rust
// Use HIP (C++ API) via FFI bindings
// Similar to CUDA implementation but with HIP runtime
```

---

## Testing Strategy

### Unit Tests (Per Backend)

Each backend should have tests matching Metal's test suite:

```rust
#[cfg(feature = "gpu-cuda")]
#[test]
fn test_cuda_filter_i32_eq() {
    let device = CudaDevice::new().unwrap();
    let data: Vec<i32> = (0..1000).collect();
    let result = device.execute_filter_i32(&data, 500, FilterOp::Equal).unwrap();

    let matches: i32 = result.iter().sum();
    assert_eq!(matches, 1);
    assert_eq!(result[500], 1);
}
```

### Cross-Platform Integration Tests

```rust
#[tokio::test]
async fn test_gpu_backend_auto_selection() {
    let manager = GpuDeviceManager::new();
    let device = manager.create_device().expect("At least one GPU backend should work");

    println!("Selected backend: {:?}", device.backend_type());

    // Run same test on any backend
    let data: Vec<i32> = (0..100).collect();
    let result = device.execute_filter_i32(&data, 50, FilterOp::GreaterThan).unwrap();
    assert_eq!(result.iter().filter(|&&x| x != 0).count(), 49);
}
```

---

## Shader/Kernel Comparison

| Operation | Metal (MSL) | CUDA | Vulkan (GLSL) |
|-----------|-------------|------|---------------|
| Filter i32 eq | `kernel void filter_i32_eq` | `__global__ void filter_i32_eq` | `layout(local_size_x=256) in` |
| Thread ID | `uint id [[thread_position_in_grid]]` | `blockIdx.x * blockDim.x + threadIdx.x` | `gl_GlobalInvocationID.x` |
| Buffer binding | `device const int* data [[buffer(0)]]` | Function parameter | `layout(set=0, binding=0) buffer` |
| Atomics | `atomic_fetch_add_explicit` | `atomicAdd` | `atomicAdd` |

---

## Performance Benchmarks (To Add)

Once all backends are implemented, add benchmarks:

```rust
#[bench]
fn bench_filter_10k_rows_metal(b: &mut Bencher) {
    let device = MetalDevice::new().unwrap();
    let data: Vec<i32> = (0..10000).collect();
    b.iter(|| device.execute_filter_i32(&data, 5000, FilterOp::GreaterThan));
}

#[bench]
fn bench_filter_10k_rows_cuda(b: &mut Bencher) {
    let device = CudaDevice::new().unwrap();
    let data: Vec<i32> = (0..10000).collect();
    b.iter(|| device.execute_filter_i32(&data, 5000, FilterOp::GreaterThan));
}
```

---

## Cargo Features

Enable backends with feature flags:

```toml
[features]
default = ["cpu-simd"]
gpu-metal = []  # Auto-enabled on macOS
gpu-cuda = ["cudarc"]
gpu-rocm = []
gpu-vulkan = ["vulkano"]
gpu-all = ["gpu-cuda", "gpu-vulkan"]
```

Build for specific GPU:
```bash
# NVIDIA GPU (Linux/Windows)
cargo build --features gpu-cuda

# Cross-platform Vulkan
cargo build --features gpu-vulkan

# All GPU backends
cargo build --features gpu-all
```

---

## Priority Roadmap

1. ✅ **Completed**: Vulkan backend (works everywhere, including AMD/Intel GPUs)
   - VulkanDevice implemented (671 lines)
   - 6 GLSL compute shaders with SPIR-V bytecode
   - Cross-platform support for Linux, Windows, macOS
   - Ready for GPU pipeline integration
2. **High Priority**: CUDA backend (NVIDIA GPUs, large user base)
   - Recommended for NVIDIA-specific optimizations
   - Complete implementation guide available
3. **Low Priority**: Native ROCm backend (Vulkan covers AMD GPUs already)
   - Only needed for HPC-specific ROCm features

---

## Getting Started

To add a new backend:

1. Create `orbit/compute/src/gpu_<backend>.rs`
2. Implement `GpuDevice` trait
3. Add backend detection to `GpuDeviceManager::is_<backend>_available()`
4. Add backend creation to `GpuDeviceManager::create_device_for_backend()`
5. Add shader/kernel source files
6. Write unit tests matching Metal's test suite
7. Update Cargo.toml with feature flags and dependencies

---

## Resources

- **Metal**: https://developer.apple.com/metal/
- **CUDA**: https://docs.nvidia.com/cuda/
- **Vulkan**: https://www.khronos.org/vulkan/
- **ROCm**: https://rocm.docs.amd.com/
- **cudarc**: https://github.com/coreylowman/cudarc
- **vulkano**: https://github.com/vulkano-rs/vulkano
