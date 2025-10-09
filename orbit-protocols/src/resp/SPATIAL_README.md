# Orbit-RS Redis Spatial Extensions

This document describes the advanced geospatial capabilities available through the Redis protocol in Orbit-RS. These extensions go beyond standard Redis GEO commands to provide complex geometry support, real-time geofencing, spatial analytics, and GPU-accelerated clustering.

## Standard Redis GEO Commands (Enhanced)

Orbit-RS implements all standard Redis geospatial commands with performance optimizations and enhanced functionality:

### GEOADD
```redis
GEOADD key longitude latitude member [longitude latitude member ...]
```
Adds one or more geospatial items to the specified key.

**Example:**
```redis
GEOADD cities -122.4194 37.7749 "San Francisco" -74.0060 40.7128 "New York"
```

### GEOPOS
```redis
GEOPOS key member [member ...]
```
Returns longitude/latitude coordinates for given members.

**Example:**
```redis
GEOPOS cities "San Francisco"
1) 1) "-122.41940000000002"
   2) "37.77489999999999"
```

### GEODIST
```redis
GEODIST key member1 member2 [m|km|ft|mi]
```
Calculates the distance between two members.

**Example:**
```redis
GEODIST cities "San Francisco" "New York" km
"4134.972"
```

### GEOHASH
```redis
GEOHASH key member [member ...]
```
Returns geohash strings for the given members.

### GEORADIUS
```redis
GEORADIUS key longitude latitude radius m|km|ft|mi [WITHCOORD] [WITHDIST] [COUNT count]
```
Query points within a radius from given coordinates.

**Example:**
```redis
GEORADIUS cities -122.4 37.8 50 km WITHCOORD WITHDIST COUNT 10
```

### GEORADIUSBYMEMBER
```redis
GEORADIUSBYMEMBER key member radius m|km|ft|mi [WITHCOORD] [WITHDIST] [COUNT count]
```
Query points within a radius from an existing member.

## Extended Spatial Commands (Orbit-RS Specific)

### Index Management

#### GEO.INDEX.CREATE
```redis
GEO.INDEX.CREATE index_name index_type [options]
```
Creates a spatial index with the specified type.

**Index Types:**
- `QUADTREE` - Quad-tree spatial index (default)
- `RTREE` - R-tree spatial index
- `GRID` - Grid-based spatial index
- `GPU` - GPU-accelerated spatial index

**Example:**
```redis
GEO.INDEX.CREATE my_locations RTREE
```

#### GEO.INDEX.DROP
```redis
GEO.INDEX.DROP index_name
```
Removes a spatial index.

#### GEO.INDEX.INFO
```redis
GEO.INDEX.INFO index_name
```
Returns information about a spatial index.

### Complex Geometry Support

#### GEO.POLYGON.ADD
```redis
GEO.POLYGON.ADD key member "POLYGON((x1 y1, x2 y2, x3 y3, x1 y1))"
```
Adds a polygon geometry to the spatial index.

**Example:**
```redis
GEO.POLYGON.ADD zones downtown "POLYGON((-122.42 37.77, -122.41 37.77, -122.41 37.78, -122.42 37.78, -122.42 37.77))"
```

#### GEO.LINESTRING.ADD
```redis
GEO.LINESTRING.ADD key member "LINESTRING(x1 y1, x2 y2, x3 y3)"
```
Adds a linestring geometry.

#### GEO.GEOMETRY.GET
```redis
GEO.GEOMETRY.GET key member
```
Retrieves the geometry for a given member.

#### GEO.GEOMETRY.DEL
```redis
GEO.GEOMETRY.DEL key member [member ...]
```
Removes geometries from the spatial index.

### Spatial Relationship Queries

#### GEO.WITHIN
```redis
GEO.WITHIN key "POLYGON((x1 y1, x2 y2, x3 y3, x1 y1))"
```
Finds all geometries within the given polygon.

#### GEO.INTERSECTS
```redis
GEO.INTERSECTS key geometry_wkt
```
Finds all geometries that intersect with the given geometry.

#### GEO.CONTAINS
```redis
GEO.CONTAINS key geometry_wkt
```
Finds all geometries that contain the given geometry.

#### GEO.OVERLAPS
```redis
GEO.OVERLAPS key geometry_wkt
```
Finds all geometries that overlap with the given geometry.

## Real-Time Geofencing

### GEO.FENCE.ADD
```redis
GEO.FENCE.ADD fence_name geometry_wkt alert_types
```
Creates a geofence with alerting capabilities.

**Alert Types:**
- `ENTER` - Alert when entities enter the fence
- `EXIT` - Alert when entities exit the fence
- `ENTER,EXIT` - Alert on both enter and exit

**Example:**
```redis
GEO.FENCE.ADD downtown_zone "POLYGON((-122.42 37.77, -122.41 37.77, -122.41 37.78, -122.42 37.78, -122.42 37.77))" ENTER,EXIT
```

### GEO.FENCE.DEL
```redis
GEO.FENCE.DEL fence_name [fence_name ...]
```
Removes one or more geofences.

### GEO.FENCE.CHECK
```redis
GEO.FENCE.CHECK entity_id longitude latitude
```
Checks if a location violates any active geofences.

**Example:**
```redis
GEO.FENCE.CHECK user123 -122.415 37.775
1) 1) "downtown_zone"
   2) "INSIDE"
```

### GEO.FENCE.SUBSCRIBE
```redis
GEO.FENCE.SUBSCRIBE channel fence_name [fence_name ...]
```
Subscribes to geofence violations on a channel.

### GEO.FENCE.LIST
```redis
GEO.FENCE.LIST [PATTERN pattern]
```
Lists all active geofences, optionally filtered by pattern.

## Spatial Analytics

### Clustering

#### GEO.CLUSTER.KMEANS
```redis
GEO.CLUSTER.KMEANS index_name k_value
```
Performs K-means clustering on spatial points.

**Example:**
```redis
GEO.CLUSTER.KMEANS user_locations 3
1) 1) "user1"
   2) (integer) 0
2) 1) "user2"  
   2) (integer) 0
3) 1) "user3"
   2) (integer) 1
```

#### GEO.CLUSTER.DBSCAN
```redis
GEO.CLUSTER.DBSCAN index_name epsilon min_points
```
Performs DBSCAN clustering for density-based clustering.

### Density Analysis

#### GEO.DENSITY.HEATMAP
```redis
GEO.DENSITY.HEATMAP index_name grid_size [bounds]
```
Generates a density heatmap of spatial points.

### Nearest Neighbors

#### GEO.NEAREST.NEIGHBORS
```redis
GEO.NEAREST.NEIGHBORS index_name longitude latitude k [algorithm]
```
Finds k nearest neighbors to a given point.

**Algorithms:**
- `BRUTE_FORCE` - Brute force search
- `KD_TREE` - KD-tree based search  
- `BALL_TREE` - Ball tree based search
- `GPU` - GPU-accelerated search

## Real-Time Spatial Streaming

### GEO.STREAM.PROCESS
```redis
GEO.STREAM.PROCESS stream_key processor_config
```
Sets up real-time spatial stream processing.

### GEO.STREAM.SUBSCRIBE
```redis
GEO.STREAM.SUBSCRIBE channel pattern
```
Subscribes to spatial stream events.

### GEO.STREAM.AGGREGATE
```redis
GEO.STREAM.AGGREGATE stream_key time_window aggregation_function
```
Performs time-windowed spatial aggregations.

## Time-Series Geospatial Data

### GEO.TS.ADD
```redis
GEO.TS.ADD key timestamp longitude latitude [metadata]
```
Adds a time-stamped spatial point.

**Example:**
```redis
GEO.TS.ADD vehicle_track 1640995200 -122.4194 37.7749 speed=65
```

### GEO.TS.RANGE
```redis
GEO.TS.RANGE key from_timestamp to_timestamp [WITHLOCATION] [WITHMETADATA]
```
Queries spatial data within a time range.

### GEO.TS.TRAJECTORY
```redis
GEO.TS.TRAJECTORY key from_timestamp to_timestamp [SIMPLIFY tolerance]
```
Reconstructs movement trajectory from time-series spatial data.

## Performance Features

### GPU Acceleration
Many spatial operations can leverage GPU acceleration when available:
- Spatial clustering (K-means, DBSCAN)
- Nearest neighbor search
- Complex spatial queries
- Density calculations

### Parallel Processing
Orbit-RS automatically parallelizes spatial operations across available CPU cores for:
- Large-scale spatial joins
- Bulk geometry operations
- Multi-polygon intersections

### Adaptive Indexing
Spatial indexes automatically optimize themselves based on:
- Data distribution patterns
- Query patterns
- Available memory
- Hardware capabilities

## Configuration

Spatial functionality can be configured through environment variables or configuration files:

```toml
[spatial]
# Enable GPU acceleration (requires CUDA/OpenCL)
gpu_enabled = true

# Default spatial reference system
default_srid = 4326  # WGS84

# Index optimization settings
auto_optimize_indexes = true
optimization_interval = "1h"

# Memory limits for spatial operations
max_memory_per_operation = "1GB"

# Parallel processing settings
max_parallel_threads = 0  # 0 = auto-detect

[geofencing]
# Maximum number of active geofences
max_geofences = 10000

# Real-time processing buffer size
buffer_size = 1000

# Alert delivery timeout
alert_timeout = "5s"
```

## Error Handling

Spatial commands return standard Redis error responses:

- `(error) ERR Invalid geometry` - Malformed WKT or coordinates
- `(error) ERR Index not found` - Referenced spatial index doesn't exist
- `(error) ERR Unsupported operation` - Operation not supported for geometry type
- `(error) ERR Out of memory` - Insufficient memory for spatial operation
- `(error) ERR GPU not available` - GPU acceleration requested but not available

## Examples

### Setting up a real-time location tracking system:

```redis
# Create a spatial index for vehicles
GEO.INDEX.CREATE vehicle_locations RTREE

# Add some vehicles
GEOADD vehicle_locations -122.4194 37.7749 vehicle1 -122.4094 37.7849 vehicle2

# Create a geofence for downtown area
GEO.FENCE.ADD downtown "POLYGON((-122.42 37.77, -122.40 37.77, -122.40 37.79, -122.42 37.79, -122.42 37.77))" ENTER,EXIT

# Subscribe to geofence violations
GEO.FENCE.SUBSCRIBE alerts downtown

# Check for vehicles in downtown area
GEO.WITHIN vehicle_locations "POLYGON((-122.42 37.77, -122.40 37.77, -122.40 37.79, -122.42 37.79, -122.42 37.77))"

# Find clusters of vehicles
GEO.CLUSTER.KMEANS vehicle_locations 3

# Track vehicle movement over time
GEO.TS.ADD vehicle1_track 1640995200 -122.4194 37.7749
GEO.TS.ADD vehicle1_track 1640995260 -122.4184 37.7759
GEO.TS.TRAJECTORY vehicle1_track 1640995200 1640999999
```

### Spatial analytics on customer locations:

```redis
# Create customer location index  
GEO.INDEX.CREATE customers QUADTREE

# Add customer locations
GEOADD customers -74.0060 40.7128 customer1 -73.9352 40.7306 customer2

# Find density hotspots
GEO.DENSITY.HEATMAP customers 0.01

# Cluster customers by location
GEO.CLUSTER.DBSCAN customers 0.005 5

# Find customers within delivery zones
GEO.WITHIN customers "POLYGON((-74.01 40.71, -74.00 40.71, -74.00 40.72, -74.01 40.72, -74.01 40.71))"
```

This spatial extension makes Orbit-RS a powerful platform for building location-aware applications with Redis-compatible tooling.