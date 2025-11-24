//! Metal compute kernels for database operations
//! Optimized for columnar data processing with predicates
//! Includes GPU-accelerated graph traversal algorithms

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

/// Min reduction for i32 using parallel reduction
/// Note: This uses a simple atomic min approach (requires initialization)
kernel void aggregate_i32_min(
    device const int* input [[buffer(0)]],
    device atomic_int* output [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    int value = input[id];
    int current = atomic_load_explicit(output, memory_order_relaxed);
    // Update if value is smaller
    if (value < current) {
        // Use atomic min (if available) or loop with compare-and-swap
        int expected = current;
        while (value < expected) {
            if (atomic_compare_exchange_weak_explicit(output, &expected, value, memory_order_relaxed, memory_order_relaxed)) {
                break;
            }
            current = expected;
        }
    }
}

/// Max reduction for i32 using parallel reduction
/// Note: This uses a simple atomic max approach (requires initialization)
kernel void aggregate_i32_max(
    device const int* input [[buffer(0)]],
    device atomic_int* output [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
    int value = input[id];
    int current = atomic_load_explicit(output, memory_order_relaxed);
    // Update if value is larger
    if (value > current) {
        // Use atomic max (if available) or loop with compare-and-swap
        int expected = current;
        while (value > expected) {
            if (atomic_compare_exchange_weak_explicit(output, &expected, value, memory_order_relaxed, memory_order_relaxed)) {
                break;
            }
            current = expected;
        }
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

// ============================================================================
// Graph Traversal Operations
// ============================================================================

/// Parallel BFS level expansion
/// Each thread processes one node's neighbors in the current level
/// Input: edge_array (flat array of all edges), edge_offset (offsets for each node)
///        current_level (nodes to expand), visited (visited nodes mask)
/// Output: next_level (nodes discovered in this expansion), level_size (number of new nodes)
kernel void bfs_level_expansion(
    device const uint* edge_array [[buffer(0)]],
    device const uint* edge_offset [[buffer(1)]],
    device const uint* current_level [[buffer(2)]],
    device atomic_uint* visited [[buffer(3)]],
    device uint* next_level [[buffer(4)]],
    device atomic_uint* next_level_size [[buffer(5)]],
    device const uint& current_level_size [[buffer(6)]],
    device const uint& max_nodes [[buffer(7)]],
    uint id [[thread_position_in_grid]]
) {
    if (id >= current_level_size) {
        return;
    }
    
    uint node_id = current_level[id];
    uint start_idx = edge_offset[node_id];
    uint end_idx = edge_offset[node_id + 1];
    
    // Process all neighbors of this node
    for (uint i = start_idx; i < end_idx; i++) {
        uint neighbor = edge_array[i];
        
        // Check if neighbor is already visited (atomic exchange)
        uint original = atomic_exchange_explicit(&visited[neighbor], 1, memory_order_relaxed);
        
        if (original == 0) {
            // Add neighbor to next level
            uint next_idx = atomic_fetch_add_explicit(next_level_size, 1, memory_order_relaxed);
            
            if (next_idx < max_nodes) {
                next_level[next_idx] = neighbor;
            }
        }
    }
}

/// Parallel community detection using connected components
/// Each thread processes one node to find its connected component
kernel void connected_components(
    device const uint* edge_array [[buffer(0)]],
    device const uint* edge_offset [[buffer(1)]],
    device uint* parent [[buffer(2)]],
    device uint* visited [[buffer(3)]],
    device const uint& node_count [[buffer(4)]],
    uint id [[thread_position_in_grid]]
) {
    if (id >= node_count) {
        return;
    }
    
    // Union-Find with path compression
    uint root = id;
    while (parent[root] != root) {
        root = parent[root];
    }
    
    // Path compression
    uint current = id;
    while (parent[current] != root) {
        uint next = parent[current];
        parent[current] = root;
        current = next;
    }
    
    // Mark as visited and process neighbors
    if (visited[id] == 0) {
        visited[id] = 1;
        
        uint start_idx = edge_offset[id];
        uint end_idx = edge_offset[id + 1];
        
        for (uint i = start_idx; i < end_idx; i++) {
            uint neighbor = edge_array[i];
            
            // Union: merge components
            uint neighbor_root = neighbor;
            while (parent[neighbor_root] != neighbor_root) {
                neighbor_root = parent[neighbor_root];
            }
            
            if (root < neighbor_root) {
                parent[neighbor_root] = root;
            } else if (neighbor_root < root) {
                parent[root] = neighbor_root;
                root = neighbor_root;
            }
        }
    }
}

// ============================================================================
// Vector Similarity Operations
// ============================================================================

/// Calculate cosine similarity between query vector and candidate vectors
/// Each thread processes one candidate vector
/// Output: scores[i] = cosine_similarity(query, candidates[i])
kernel void vector_cosine_similarity(
    device const float* query [[buffer(0)]],
    device const float* candidates [[buffer(1)]],
    device float* scores [[buffer(2)]],
    device const uint* params [[buffer(3)]], // [vector_count, dimension]
    uint id [[thread_position_in_grid]]
) {
    uint vector_count = params[0];
    uint dimension = params[1];
    
    if (id >= vector_count) {
        return;
    }
    
    // Calculate dot product and magnitudes
    float dot_product = 0.0;
    float query_magnitude_sq = 0.0;
    float candidate_magnitude_sq = 0.0;
    
    uint candidate_offset = id * dimension;
    
    for (uint i = 0; i < dimension; i++) {
        float q = query[i];
        float c = candidates[candidate_offset + i];
        
        dot_product += q * c;
        query_magnitude_sq += q * q;
        candidate_magnitude_sq += c * c;
    }
    
    // Calculate cosine similarity
    float query_magnitude = sqrt(query_magnitude_sq);
    float candidate_magnitude = sqrt(candidate_magnitude_sq);
    
    if (query_magnitude > 0.0 && candidate_magnitude > 0.0) {
        scores[id] = dot_product / (query_magnitude * candidate_magnitude);
    } else {
        scores[id] = 0.0;
    }
}

/// Calculate euclidean distance between query vector and candidate vectors
/// Each thread processes one candidate vector
/// Output: scores[i] = euclidean_distance(query, candidates[i])
kernel void vector_euclidean_distance(
    device const float* query [[buffer(0)]],
    device const float* candidates [[buffer(1)]],
    device float* scores [[buffer(2)]],
    device const uint* params [[buffer(3)]], // [vector_count, dimension]
    uint id [[thread_position_in_grid]]
) {
    uint vector_count = params[0];
    uint dimension = params[1];
    
    if (id >= vector_count) {
        return;
    }
    
    // Calculate squared euclidean distance
    float distance_sq = 0.0;
    uint candidate_offset = id * dimension;
    
    for (uint i = 0; i < dimension; i++) {
        float diff = query[i] - candidates[candidate_offset + i];
        distance_sq += diff * diff;
    }
    
    scores[id] = sqrt(distance_sq);
}

/// Calculate dot product between query vector and candidate vectors
/// Each thread processes one candidate vector
/// Output: scores[i] = dot_product(query, candidates[i])
kernel void vector_dot_product(
    device const float* query [[buffer(0)]],
    device const float* candidates [[buffer(1)]],
    device float* scores [[buffer(2)]],
    device const uint* params [[buffer(3)]], // [vector_count, dimension]
    uint id [[thread_position_in_grid]]
) {
    uint vector_count = params[0];
    uint dimension = params[1];
    
    if (id >= vector_count) {
        return;
    }
    
    // Calculate dot product
    float dot_product = 0.0;
    uint candidate_offset = id * dimension;
    
    for (uint i = 0; i < dimension; i++) {
        dot_product += query[i] * candidates[candidate_offset + i];
    }
    
    scores[id] = dot_product;
}

/// Calculate manhattan (L1) distance between query vector and candidate vectors
/// Each thread processes one candidate vector
/// Output: scores[i] = manhattan_distance(query, candidates[i])
kernel void vector_manhattan_distance(
    device const float* query [[buffer(0)]],
    device const float* candidates [[buffer(1)]],
    device float* scores [[buffer(2)]],
    device const uint* params [[buffer(3)]], // [vector_count, dimension]
    uint id [[thread_position_in_grid]]
) {
    uint vector_count = params[0];
    uint dimension = params[1];
    
    if (id >= vector_count) {
        return;
    }
    
    // Calculate manhattan distance
    float distance = 0.0;
    uint candidate_offset = id * dimension;
    
    for (uint i = 0; i < dimension; i++) {
        distance += abs(query[i] - candidates[candidate_offset + i]);
    }
    
    scores[id] = distance;
}

// ============================================================================
// Spatial Operations
// ============================================================================

/// Calculate 2D Euclidean distance between a query point and multiple candidate points
/// Each thread processes one candidate point
/// Output: distances[i] = sqrt((query_x - candidates_x[i])^2 + (query_y - candidates_y[i])^2)
kernel void spatial_distance(
    device const float& query_x [[buffer(0)]],
    device const float& query_y [[buffer(1)]],
    device const float* candidates_x [[buffer(2)]],
    device const float* candidates_y [[buffer(3)]],
    device float* distances [[buffer(4)]],
    device const uint& point_count [[buffer(5)]],
    uint id [[thread_position_in_grid]]
) {
    if (id >= point_count) {
        return;
    }
    
    float dx = candidates_x[id] - query_x;
    float dy = candidates_y[id] - query_y;
    distances[id] = sqrt(dx * dx + dy * dy);
}

/// Calculate great circle distance (Haversine) between a query point and multiple candidate points
/// Each thread processes one candidate point
/// Output: distances[i] = haversine_distance(query, candidates[i]) in meters
kernel void spatial_distance_sphere(
    device const float& query_lon [[buffer(0)]],
    device const float& query_lat [[buffer(1)]],
    device const float* candidates_lon [[buffer(2)]],
    device const float* candidates_lat [[buffer(3)]],
    device float* distances [[buffer(4)]],
    device const uint& point_count [[buffer(5)]],
    uint id [[thread_position_in_grid]]
) {
    if (id >= point_count) {
        return;
    }
    
    const float EARTH_RADIUS_KM = 6371.0;
    const float PI = 3.14159265358979323846;
    
    // Convert degrees to radians
    float lat1_rad = query_lat * PI / 180.0;
    float lat2_rad = candidates_lat[id] * PI / 180.0;
    float delta_lat = (candidates_lat[id] - query_lat) * PI / 180.0;
    float delta_lon = (candidates_lon[id] - query_lon) * PI / 180.0;
    
    // Haversine formula
    float a = sin(delta_lat / 2.0) * sin(delta_lat / 2.0) +
              cos(lat1_rad) * cos(lat2_rad) * sin(delta_lon / 2.0) * sin(delta_lon / 2.0);
    float c = 2.0 * atan2(sqrt(a), sqrt(1.0 - a));
    
    distances[id] = EARTH_RADIUS_KM * c * 1000.0; // Return distance in meters
}
