# Vulkan GLSL Compute Shaders

This directory contains GLSL compute shaders for GPU-accelerated database operations using Vulkan.

## Shader Files

### Filter Operations

1. **`filter_i32.comp`** - Filter operations for 32-bit integers
   - Operations: Equal, Greater Than, Greater or Equal, Less Than, Less or Equal, Not Equal
   - Workgroup size: 256 threads
   - Input: i32 data array, filter value, operation type
   - Output: Binary mask (1 = matches, 0 = doesn't match)

2. **`filter_i64.comp`** - Filter operations for 64-bit integers
   - Requires: `GL_ARB_gpu_shader_int64` extension
   - Same operations as i32
   - Uses int64_t for data and filter value

3. **`filter_f64.comp`** - Filter operations for 64-bit floating point
   - Requires: `GL_ARB_gpu_shader_fp64` extension
   - Uses epsilon-based comparison for equality (1e-10)
   - Operations support floating point tolerance

### Bitmap Operations

4. **`bitmap_ops.comp`** - Bitmap logical operations
   - Operations: AND, OR, NOT
   - Used for combining filter predicates
   - Efficient parallel execution

### Aggregation Operations

5. **`aggregate_sum.comp`** - Parallel sum reduction
   - Uses shared memory for efficient reduction
   - Two-stage reduction: workgroup-local + global
   - Output: Partial sums from each workgroup (needs CPU final reduction)

6. **`aggregate_count.comp`** - Parallel count reduction
   - Counts non-zero elements in mask
   - Uses shared memory reduction
   - Output: Partial counts from each workgroup

## Compilation

These shaders must be compiled to SPIR-V bytecode before use:

```bash
# Install glslc (part of shaderc)
# macOS: brew install shaderc
# Linux: sudo apt-get install shaderc

# Compile shaders
glslc filter_i32.comp -o filter_i32.spv
glslc filter_i64.comp -o filter_i64.spv
glslc filter_f64.comp -o filter_f64.spv
glslc bitmap_ops.comp -o bitmap_ops.spv
glslc aggregate_sum.comp -o aggregate_sum.spv
glslc aggregate_count.comp -o aggregate_count.spv
```

## Performance Characteristics

### Workgroup Size
- All shaders use 256 threads per workgroup
- Optimal for most modern GPUs (NVIDIA, AMD, Intel)
- Provides good occupancy and resource utilization

### Memory Access Patterns
- **Coalesced reads**: All shaders read input data in sequential order
- **Coalesced writes**: Output writes are also sequential
- **Shared memory**: Aggregation shaders use shared memory for reduction

### Reduction Strategy
Aggregation shaders use a two-stage reduction:
1. **Stage 1**: Parallel reduction within each workgroup using shared memory
2. **Stage 2**: CPU reduces partial results from all workgroups

This hybrid approach balances GPU and CPU work optimally for most dataset sizes.

## Extension Requirements

### Required Extensions
- **Filter i64**: `GL_ARB_gpu_shader_int64` - Most modern GPUs support this
- **Filter f64**: `GL_ARB_gpu_shader_fp64` - Most modern GPUs support this

### Fallback Strategy
If extensions are not available:
- i64 filters: Use CPU fallback or emulate with two i32 values
- f64 filters: Use f32 (single precision) or CPU fallback

## Integration with VulkanDevice

The `VulkanDevice` struct loads these shaders and creates compute pipelines:

```rust
// Load shader
let shader_code = include_bytes!("shaders/vulkan/filter_i32.spv");
let shader_module = unsafe { ShaderModule::new(device, shader_code)? };

// Create compute pipeline
let pipeline = ComputePipeline::new(device, shader_module, ...)?;

// Execute
device.execute_compute(pipeline, data, value, operation)?;
```

## Performance Expectations

Typical performance gains vs CPU (SIMD):
- **Filters**: 2-5x speedup on datasets > 10K rows
- **Bitmap ops**: 3-7x speedup on large masks
- **Aggregations**: 5-10x speedup on datasets > 100K rows

Performance varies by:
- GPU model (discrete >> integrated)
- Dataset size (larger = better GPU utilization)
- Memory bandwidth
- PCIe transfer overhead

## Testing

Each shader has corresponding unit tests in `gpu_vulkan::tests`:
- `test_vulkan_filter_i32_*` - Filter operation tests
- `test_vulkan_bitmap_*` - Bitmap operation tests
- `test_vulkan_aggregate_*` - Aggregation tests

Run tests with:
```bash
cargo test -p orbit-compute --features gpu-vulkan gpu_vulkan
```
