//! Metal compute kernels for database operations
//! Optimized for columnar data processing with predicates

#include <metal_stdlib>
using namespace metal;

// ============================================================================
// Filter Operations - Int32
// ============================================================================

/// Filter by equality: output[i] = (data[i] == value) ? 1 : 0
kernel void filter_i32_eq(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] == value) ? 1 : 0;
}

/// Filter by greater than: output[i] = (data[i] > value) ? 1 : 0
kernel void filter_i32_gt(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] > value) ? 1 : 0;
}

/// Filter by greater than or equal: output[i] = (data[i] >= value) ? 1 : 0
kernel void filter_i32_ge(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] >= value) ? 1 : 0;
}

/// Filter by less than: output[i] = (data[i] < value) ? 1 : 0
kernel void filter_i32_lt(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] < value) ? 1 : 0;
}

/// Filter by less than or equal: output[i] = (data[i] <= value) ? 1 : 0
kernel void filter_i32_le(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] <= value) ? 1 : 0;
}

/// Filter by not equal: output[i] = (data[i] != value) ? 1 : 0
kernel void filter_i32_ne(
    device const int* data [[buffer(0)]],
    device const int& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] != value) ? 1 : 0;
}

// ============================================================================
// Filter Operations - Int64
// ============================================================================

kernel void filter_i64_eq(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] == value) ? 1 : 0;
}

kernel void filter_i64_gt(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] > value) ? 1 : 0;
}

kernel void filter_i64_ge(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] >= value) ? 1 : 0;
}

kernel void filter_i64_lt(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] < value) ? 1 : 0;
}

kernel void filter_i64_le(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] <= value) ? 1 : 0;
}

kernel void filter_i64_ne(
    device const long* data [[buffer(0)]],
    device const long& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] != value) ? 1 : 0;
}

// ============================================================================
// Filter Operations - Float64
// ============================================================================

kernel void filter_f64_eq(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (abs(data[id] - value) < 1e-10) ? 1 : 0;
}

kernel void filter_f64_gt(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] > value) ? 1 : 0;
}

kernel void filter_f64_ge(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] >= value) ? 1 : 0;
}

kernel void filter_f64_lt(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] < value) ? 1 : 0;
}

kernel void filter_f64_le(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (data[id] <= value) ? 1 : 0;
}

kernel void filter_f64_ne(
    device const double* data [[buffer(0)]],
    device const double& value [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    output[id] = (abs(data[id] - value) >= 1e-10) ? 1 : 0;
}

// ============================================================================
// Aggregation Operations - Int32
// ============================================================================

/// Sum reduction for i32 using parallel reduction
kernel void aggregate_i32_sum(
    device const int* input [[buffer(0)]],
    device atomic_int* output [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    atomic_fetch_add_explicit(output, input[id], memory_order_relaxed);
}

/// Count reduction
kernel void aggregate_i32_count(
    device const int* mask [[buffer(0)]],
    device atomic_int* count [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    if (mask[id] != 0) {
        atomic_fetch_add_explicit(count, 1, memory_order_relaxed);
    }
}

// ============================================================================
// Aggregation Operations - Int64
// ============================================================================

kernel void aggregate_i64_sum(
    device const long* input [[buffer(0)]],
    device atomic_long* output [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    atomic_fetch_add_explicit(output, input[id], memory_order_relaxed);
}

kernel void aggregate_i64_count(
    device const int* mask [[buffer(0)]],
    device atomic_long* count [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    if (mask[id] != 0) {
        atomic_fetch_add_explicit(count, 1, memory_order_relaxed);
    }
}

// ============================================================================
// Aggregation Operations - Float64
// ============================================================================

// Note: Metal doesn't have atomic operations for double/float types
// We'll need to use a different approach for floating point aggregations
// For now, we'll use integer atomics on the bit representation

kernel void aggregate_f64_sum_partial(
    device const double* input [[buffer(0)]],
    device double* partial_sums [[buffer(1)]],
    device const uint& num_partitions [[buffer(2)]],
    uint id [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]]
) {
    // Each threadgroup computes a partial sum
    threadgroup double shared[256];
    shared[tid] = input[id];
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Parallel reduction within threadgroup
    for (uint stride = 128; stride > 0; stride >>= 1) {
        if (tid < stride) {
            shared[tid] += shared[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // First thread writes result
    if (tid == 0) {
        partial_sums[gid] = shared[0];
    }
}

// ============================================================================
// Bitmap AND Operation (for combining filters)
// ============================================================================

kernel void bitmap_and(
    device const int* mask_a [[buffer(0)]],
    device const int* mask_b [[buffer(1)]],
    device int* result [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    result[id] = (mask_a[id] != 0 && mask_b[id] != 0) ? 1 : 0;
}

// ============================================================================
// Bitmap OR Operation (for combining filters)
// ============================================================================

kernel void bitmap_or(
    device const int* mask_a [[buffer(0)]],
    device const int* mask_b [[buffer(1)]],
    device int* result [[buffer(2)]],
    uint id [[thread_position_in_grid]]
) {
    result[id] = (mask_a[id] != 0 || mask_b[id] != 0) ? 1 : 0;
}

// ============================================================================
// Bitmap NOT Operation (for negating filters)
// ============================================================================

kernel void bitmap_not(
    device const int* mask [[buffer(0)]],
    device int* result [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    result[id] = (mask[id] == 0) ? 1 : 0;
}

// ============================================================================
// Copy filtered rows (compaction)
// ============================================================================

kernel void compact_i32(
    device const int* input [[buffer(0)]],
    device const int* mask [[buffer(1)]],
    device const uint* output_indices [[buffer(2)]],
    device int* output [[buffer(3)]],
    uint id [[thread_position_in_grid]]
) {
    if (mask[id] != 0) {
        uint out_idx = output_indices[id];
        output[out_idx] = input[id];
    }
}

kernel void compact_i64(
    device const long* input [[buffer(0)]],
    device const int* mask [[buffer(1)]],
    device const uint* output_indices [[buffer(2)]],
    device long* output [[buffer(3)]],
    uint id [[thread_position_in_grid]]
) {
    if (mask[id] != 0) {
        uint out_idx = output_indices[id];
        output[out_idx] = input[id];
    }
}

kernel void compact_f64(
    device const double* input [[buffer(0)]],
    device const int* mask [[buffer(1)]],
    device const uint* output_indices [[buffer(2)]],
    device double* output [[buffer(3)]],
    uint id [[thread_position_in_grid]]
) {
    if (mask[id] != 0) {
        uint out_idx = output_indices[id];
        output[out_idx] = input[id];
    }
}

// ============================================================================
// Prefix sum for compaction (exclusive scan)
// ============================================================================

kernel void prefix_sum(
    device const int* input [[buffer(0)]],
    device uint* output [[buffer(1)]],
    uint id [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]]
) {
    threadgroup uint shared[512];

    // Load input
    shared[tid] = (input[id] != 0) ? 1 : 0;
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Up-sweep phase
    uint offset = 1;
    for (uint d = 256; d > 0; d >>= 1) {
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (tid < d) {
            uint ai = offset * (2 * tid + 1) - 1;
            uint bi = offset * (2 * tid + 2) - 1;
            shared[bi] += shared[ai];
        }
        offset *= 2;
    }

    // Clear last element
    if (tid == 0) {
        shared[511] = 0;
    }

    // Down-sweep phase
    for (uint d = 1; d < 512; d *= 2) {
        offset >>= 1;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (tid < d) {
            uint ai = offset * (2 * tid + 1) - 1;
            uint bi = offset * (2 * tid + 2) - 1;
            uint temp = shared[ai];
            shared[ai] = shared[bi];
            shared[bi] += temp;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);
    output[id] = shared[tid];
}
