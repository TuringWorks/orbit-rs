//! CUDA compute kernels for database operations
//! Optimized for columnar data processing with predicates
//! Includes GPU-accelerated graph traversal algorithms

#include <cuda_runtime.h>
#include <cstdint>
#include <cfloat>

// ============================================================================
// Filter Operations - Int32
// ============================================================================

extern "C" __global__ void filter_i32_eq(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] == value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i32_gt(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] > value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i32_ge(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] >= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i32_lt(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] < value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i32_le(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] <= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i32_ne(
    const int* data,
    const int value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] != value) ? 1 : 0;
    }
}

// ============================================================================
// Filter Operations - Int64
// ============================================================================

extern "C" __global__ void filter_i64_eq(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] == value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i64_gt(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] > value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i64_ge(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] >= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i64_lt(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] < value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i64_le(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] <= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_i64_ne(
    const long long* data,
    const long long value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] != value) ? 1 : 0;
    }
}

// ============================================================================
// Filter Operations - Float64
// ============================================================================

extern "C" __global__ void filter_f64_eq(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (fabs(data[id] - value) < 1e-10) ? 1 : 0;
    }
}

extern "C" __global__ void filter_f64_gt(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] > value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_f64_ge(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] >= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_f64_lt(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] < value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_f64_le(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (data[id] <= value) ? 1 : 0;
    }
}

extern "C" __global__ void filter_f64_ne(
    const double* data,
    const double value,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        output[id] = (fabs(data[id] - value) >= 1e-10) ? 1 : 0;
    }
}

// ============================================================================
// Bitmap Operations
// ============================================================================

extern "C" __global__ void bitmap_and(
    const int* mask_a,
    const int* mask_b,
    int* result,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        result[id] = (mask_a[id] != 0 && mask_b[id] != 0) ? 1 : 0;
    }
}

extern "C" __global__ void bitmap_or(
    const int* mask_a,
    const int* mask_b,
    int* result,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        result[id] = (mask_a[id] != 0 || mask_b[id] != 0) ? 1 : 0;
    }
}

extern "C" __global__ void bitmap_not(
    const int* mask,
    int* result,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        result[id] = (mask[id] == 0) ? 1 : 0;
    }
}

// ============================================================================
// Aggregation Operations - Int32
// ============================================================================

extern "C" __global__ void aggregate_i32_sum(
    const int* input,
    long long* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        atomicAdd(output, (long long)input[id]);
    }
}

extern "C" __global__ void aggregate_i32_count(
    const int* mask,
    unsigned int* count_output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count && mask[id] != 0) {
        atomicAdd(count_output, 1u);
    }
}

extern "C" __global__ void aggregate_i32_min(
    const int* input,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        atomicMin(output, input[id]);
    }
}

extern "C" __global__ void aggregate_i32_max(
    const int* input,
    int* output,
    unsigned int count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id < count) {
        atomicMax(output, input[id]);
    }
}

// ============================================================================
// Vector Similarity Operations
// ============================================================================

extern "C" __global__ void vector_cosine_similarity(
    const float* query,
    const float* candidates,
    float* scores,
    unsigned int vector_count,
    unsigned int dimension
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= vector_count) return;

    float dot_product = 0.0f;
    float query_magnitude_sq = 0.0f;
    float candidate_magnitude_sq = 0.0f;

    unsigned int candidate_offset = id * dimension;

    for (unsigned int i = 0; i < dimension; i++) {
        float q = query[i];
        float c = candidates[candidate_offset + i];

        dot_product += q * c;
        query_magnitude_sq += q * q;
        candidate_magnitude_sq += c * c;
    }

    float query_magnitude = sqrtf(query_magnitude_sq);
    float candidate_magnitude = sqrtf(candidate_magnitude_sq);

    if (query_magnitude > 0.0f && candidate_magnitude > 0.0f) {
        scores[id] = dot_product / (query_magnitude * candidate_magnitude);
    } else {
        scores[id] = 0.0f;
    }
}

extern "C" __global__ void vector_euclidean_distance(
    const float* query,
    const float* candidates,
    float* scores,
    unsigned int vector_count,
    unsigned int dimension
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= vector_count) return;

    float distance_sq = 0.0f;
    unsigned int candidate_offset = id * dimension;

    for (unsigned int i = 0; i < dimension; i++) {
        float diff = query[i] - candidates[candidate_offset + i];
        distance_sq += diff * diff;
    }

    scores[id] = sqrtf(distance_sq);
}

extern "C" __global__ void vector_dot_product(
    const float* query,
    const float* candidates,
    float* scores,
    unsigned int vector_count,
    unsigned int dimension
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= vector_count) return;

    float dot_product = 0.0f;
    unsigned int candidate_offset = id * dimension;

    for (unsigned int i = 0; i < dimension; i++) {
        dot_product += query[i] * candidates[candidate_offset + i];
    }

    scores[id] = dot_product;
}

// ============================================================================
// Spatial Operations
// ============================================================================

extern "C" __global__ void spatial_distance(
    const float query_x,
    const float query_y,
    const float* candidates_x,
    const float* candidates_y,
    float* distances,
    unsigned int point_count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= point_count) return;

    float dx = candidates_x[id] - query_x;
    float dy = candidates_y[id] - query_y;
    distances[id] = sqrtf(dx * dx + dy * dy);
}

extern "C" __global__ void spatial_distance_sphere(
    const float query_lon,
    const float query_lat,
    const float* candidates_lon,
    const float* candidates_lat,
    float* distances,
    unsigned int point_count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= point_count) return;

    const float EARTH_RADIUS_KM = 6371.0f;
    const float PI = 3.14159265358979323846f;

    float lat1_rad = query_lat * PI / 180.0f;
    float lat2_rad = candidates_lat[id] * PI / 180.0f;
    float delta_lat = (candidates_lat[id] - query_lat) * PI / 180.0f;
    float delta_lon = (candidates_lon[id] - query_lon) * PI / 180.0f;

    float a = sinf(delta_lat / 2.0f) * sinf(delta_lat / 2.0f) +
              cosf(lat1_rad) * cosf(lat2_rad) * sinf(delta_lon / 2.0f) * sinf(delta_lon / 2.0f);
    float c = 2.0f * atan2f(sqrtf(a), sqrtf(1.0f - a));

    distances[id] = EARTH_RADIUS_KM * c * 1000.0f; // Return distance in meters
}

// ============================================================================
// Graph Traversal Operations
// ============================================================================

extern "C" __global__ void bfs_level_expansion(
    const unsigned int* edge_array,
    const unsigned int* edge_offset,
    const unsigned int* current_level,
    unsigned int* visited,
    unsigned int* next_level,
    unsigned int* next_level_size,
    unsigned int* parent,
    unsigned int current_level_size,
    unsigned int max_nodes
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= current_level_size) return;

    unsigned int node_id = current_level[id];
    unsigned int start_idx = edge_offset[node_id];
    unsigned int end_idx = edge_offset[node_id + 1];

    for (unsigned int i = start_idx; i < end_idx; i++) {
        unsigned int neighbor = edge_array[i];

        unsigned int original = atomicExch(&visited[neighbor], 1);

        if (original == 0) {
            parent[neighbor] = node_id;
            unsigned int next_idx = atomicAdd(next_level_size, 1);

            if (next_idx < max_nodes) {
                next_level[next_idx] = neighbor;
            }
        }
    }
}

extern "C" __global__ void dijkstra_relax(
    const unsigned int* edge_array,
    const unsigned int* edge_offset,
    const float* edge_weights,
    float* distances,
    unsigned int* parent,
    const unsigned int* active_mask,
    unsigned int* changed,
    unsigned int node_count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= node_count) return;

    if (active_mask[id] == 0) return;

    float current_dist = distances[id];

    if (isinf(current_dist)) return;

    unsigned int start_idx = edge_offset[id];
    unsigned int end_idx = edge_offset[id + 1];

    for (unsigned int i = start_idx; i < end_idx; i++) {
        unsigned int neighbor = edge_array[i];
        float edge_weight = edge_weights[i];
        float new_dist = current_dist + edge_weight;

        float old_dist = atomicMin((int*)&distances[neighbor], __float_as_int(new_dist));
        // Note: This is a simplified version. CUDA doesn't have native atomic float min,
        // so in production, use atomicCAS loop for proper floating-point atomic min

        if (new_dist < __int_as_float(old_dist)) {
            parent[neighbor] = id;
            atomicAdd(changed, 1);
        }
    }
}

// ============================================================================
// Matrix Operations
// ============================================================================

extern "C" __global__ void matrix_multiply_tiled_f32(
    const float* matrix_a,
    const float* matrix_b,
    float* matrix_c,
    unsigned int M,
    unsigned int N,
    unsigned int K
) {
    const unsigned int TILE_SIZE = 16;

    __shared__ float tile_a[TILE_SIZE][TILE_SIZE];
    __shared__ float tile_b[TILE_SIZE][TILE_SIZE];

    unsigned int row = blockIdx.y * TILE_SIZE + threadIdx.y;
    unsigned int col = blockIdx.x * TILE_SIZE + threadIdx.x;

    float sum = 0.0f;

    unsigned int num_tiles = (K + TILE_SIZE - 1) / TILE_SIZE;

    for (unsigned int tile = 0; tile < num_tiles; tile++) {
        unsigned int a_col = tile * TILE_SIZE + threadIdx.x;
        if (row < M && a_col < K) {
            tile_a[threadIdx.y][threadIdx.x] = matrix_a[row * K + a_col];
        } else {
            tile_a[threadIdx.y][threadIdx.x] = 0.0f;
        }

        unsigned int b_row = tile * TILE_SIZE + threadIdx.y;
        if (b_row < K && col < N) {
            tile_b[threadIdx.y][threadIdx.x] = matrix_b[b_row * N + col];
        } else {
            tile_b[threadIdx.y][threadIdx.x] = 0.0f;
        }

        __syncthreads();

        for (unsigned int k = 0; k < TILE_SIZE; k++) {
            sum += tile_a[threadIdx.y][k] * tile_b[k][threadIdx.x];
        }

        __syncthreads();
    }

    if (row < M && col < N) {
        matrix_c[row * N + col] = sum;
    }
}

// ============================================================================
// Time-Series Window Aggregation
// ============================================================================

extern "C" __global__ void timeseries_window_aggregate(
    const float* values,
    const unsigned long long* timestamps,
    unsigned int point_count,
    unsigned long long window_size_ms,
    unsigned int aggregation_type, // 0=Sum, 1=Min, 2=Max, 3=Avg, 4=Count
    unsigned int* window_counts,
    float* window_results,
    unsigned int* window_ids
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= point_count) return;

    unsigned long long timestamp = timestamps[id];
    unsigned int window_id = (unsigned int)(timestamp / window_size_ms);
    window_ids[id] = window_id;

    float value = values[id];

    atomicAdd(&window_counts[window_id], 1);

    if (aggregation_type == 0 || aggregation_type == 3) {
        // Sum or Avg (accumulate sum)
        atomicAdd(&window_results[window_id], value);
    } else if (aggregation_type == 1) {
        // Min - use atomicMin with int reinterpretation
        int* result_int = (int*)&window_results[window_id];
        int value_int = __float_as_int(value);
        atomicMin(result_int, value_int);
    } else if (aggregation_type == 2) {
        // Max - use atomicMax with int reinterpretation
        int* result_int = (int*)&window_results[window_id];
        int value_int = __float_as_int(value);
        atomicMax(result_int, value_int);
    }
    // Count is handled by window_counts increment above
}

extern "C" __global__ void timeseries_finalize_avg(
    float* window_results,
    const unsigned int* window_counts,
    float* final_results,
    unsigned int window_count
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= window_count) return;

    unsigned int count = window_counts[id];
    if (count == 0) {
        final_results[id] = 0.0f;
        return;
    }

    final_results[id] = window_results[id] / (float)count;
}

// ============================================================================
// Hash Join Operations
// ============================================================================

extern "C" __global__ void hash_join_build(
    const unsigned int* build_keys,
    const unsigned int* build_values,
    unsigned int build_count,
    unsigned int table_size,
    unsigned int* hash_table // [key0, value0, key1, value1, ...]
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= build_count) return;

    unsigned int key = build_keys[id];
    unsigned int value = build_values[id];

    unsigned int hash = key % table_size;

    // Linear probing
    for (unsigned int i = 0; i < table_size; i++) {
        unsigned int probe = (hash + i) % table_size;
        unsigned int slot_idx = probe * 2;

        unsigned int expected = UINT_MAX;
        unsigned int old = atomicCAS(&hash_table[slot_idx], expected, key);

        if (old == UINT_MAX) {
            // Successfully inserted key
            hash_table[slot_idx + 1] = value;
            return;
        }
    }
}

extern "C" __global__ void hash_join_probe(
    const unsigned int* probe_keys,
    const unsigned int* probe_values,
    unsigned int probe_count,
    unsigned int table_size,
    const unsigned int* hash_table,
    unsigned int* output_count,
    unsigned int* output_build_ids,
    unsigned int* output_probe_ids,
    unsigned int max_output
) {
    unsigned int id = blockIdx.x * blockDim.x + threadIdx.x;
    if (id >= probe_count) return;

    unsigned int key = probe_keys[id];
    unsigned int probe_value = probe_values[id];

    unsigned int hash = key % table_size;

    for (unsigned int i = 0; i < table_size; i++) {
        unsigned int probe_slot = (hash + i) % table_size;
        unsigned int slot_idx = probe_slot * 2;
        unsigned int stored_key = hash_table[slot_idx];

        if (stored_key == UINT_MAX) {
            // Empty slot, key not found
            return;
        }

        if (stored_key == key) {
            // Found matching key
            unsigned int build_value = hash_table[slot_idx + 1];

            unsigned int output_idx = atomicAdd(output_count, 1);

            if (output_idx < max_output) {
                output_build_ids[output_idx] = build_value;
                output_probe_ids[output_idx] = probe_value;
            }

            return;
        }
    }
}
