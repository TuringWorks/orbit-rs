//! Advanced Redis geospatial commands for Orbit-RS.
//!
//! This module extends the standard Redis GEO commands with advanced spatial capabilities
//! including complex geometries, real-time geofencing, spatial analytics, and clustering.

use orbit_shared::spatial::{
    crs::utils::haversine_distance, BoundingBox, ClusteringAlgorithm, GPUSpatialEngine, Point,
    SpatialError, SpatialGeometry, SpatialOperations, SpatialStreamProcessor, WGS84_SRID,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Redis spatial command executor with advanced geospatial capabilities.
pub struct RedisSpatialCommands {
    /// Spatial indexes for different data sets
    indexes: Arc<RwLock<HashMap<String, SpatialDataSet>>>,
    /// GPU acceleration engine
    gpu_engine: GPUSpatialEngine,
    /// Real-time geofencing engine
    geofence_engine: Arc<RwLock<GeofenceEngine>>,
    /// Streaming processor for real-time updates
    #[allow(dead_code)]
    stream_processor: SpatialStreamProcessor,
}

/// Spatial data set with indexed geometries
#[derive(Debug, Clone)]
pub struct SpatialDataSet {
    pub name: String,
    pub geometries: HashMap<String, SpatialGeometry>,
    pub metadata: HashMap<String, RedisValue>,
    pub index_type: String,
    pub bounds: Option<BoundingBox>,
}

/// Real-time geofencing engine
#[derive(Debug, Clone)]
pub struct GeofenceEngine {
    geofences: HashMap<String, GeofenceDefinition>,
    #[allow(dead_code)]
    active_subscriptions: HashMap<String, Vec<String>>, // channel -> fence_ids
}

/// Geofence definition with alerting capabilities
#[derive(Debug, Clone)]
pub struct GeofenceDefinition {
    pub id: String,
    pub name: String,
    pub geometry: SpatialGeometry,
    pub alert_on_enter: bool,
    pub alert_on_exit: bool,
    pub metadata: HashMap<String, RedisValue>,
}

/// Redis value types for spatial operations
#[derive(Debug, Clone)]
pub enum RedisValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<RedisValue>),
    Null,
}

impl RedisSpatialCommands {
    /// Create a new Redis spatial commands executor.
    pub fn new() -> Self {
        Self {
            indexes: Arc::new(RwLock::new(HashMap::new())),
            gpu_engine: GPUSpatialEngine::new(),
            geofence_engine: Arc::new(RwLock::new(GeofenceEngine::new())),
            stream_processor: SpatialStreamProcessor::new(
                1000,
                tokio::time::Duration::from_millis(100),
                10,
            ),
        }
    }

    /// Execute a Redis spatial command.
    pub async fn execute_command(
        &self,
        command: &str,
        args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        match command.to_uppercase().as_str() {
            // Standard Redis GEO commands (enhanced)
            "GEOADD" => self.geo_add(args).await,
            "GEOPOS" => self.geo_pos(args).await,
            "GEODIST" => self.geo_dist(args).await,
            "GEOHASH" => self.geo_hash(args).await,
            "GEORADIUS" => self.geo_radius(args).await,
            "GEORADIUSBYMEMBER" => self.geo_radius_by_member(args).await,

            // Extended spatial commands (Orbit-RS specific)
            "GEO.INDEX.CREATE" => self.geo_index_create(args).await,
            "GEO.INDEX.DROP" => self.geo_index_drop(args).await,
            "GEO.INDEX.INFO" => self.geo_index_info(args).await,

            // Complex geometry support
            "GEO.POLYGON.ADD" => self.geo_polygon_add(args).await,
            "GEO.LINESTRING.ADD" => self.geo_linestring_add(args).await,
            "GEO.GEOMETRY.GET" => self.geo_geometry_get(args).await,
            "GEO.GEOMETRY.DEL" => self.geo_geometry_del(args).await,

            // Spatial relationship queries
            "GEO.WITHIN" => self.geo_within(args).await,
            "GEO.INTERSECTS" => self.geo_intersects(args).await,
            "GEO.CONTAINS" => self.geo_contains(args).await,
            "GEO.OVERLAPS" => self.geo_overlaps(args).await,

            // Geofencing commands
            "GEO.FENCE.ADD" => self.geo_fence_add(args).await,
            "GEO.FENCE.DEL" => self.geo_fence_del(args).await,
            "GEO.FENCE.CHECK" => self.geo_fence_check(args).await,
            "GEO.FENCE.SUBSCRIBE" => self.geo_fence_subscribe(args).await,
            "GEO.FENCE.LIST" => self.geo_fence_list(args).await,

            // Spatial analytics
            "GEO.CLUSTER.KMEANS" => self.geo_cluster_kmeans(args).await,
            "GEO.CLUSTER.DBSCAN" => self.geo_cluster_dbscan(args).await,
            "GEO.DENSITY.HEATMAP" => self.geo_density_heatmap(args).await,
            "GEO.NEAREST.NEIGHBORS" => self.geo_nearest_neighbors(args).await,

            // Real-time streaming
            "GEO.STREAM.PROCESS" => self.geo_stream_process(args).await,
            "GEO.STREAM.SUBSCRIBE" => self.geo_stream_subscribe(args).await,
            "GEO.STREAM.AGGREGATE" => self.geo_stream_aggregate(args).await,

            // Time-series geospatial
            "GEO.TS.ADD" => self.geo_ts_add(args).await,
            "GEO.TS.RANGE" => self.geo_ts_range(args).await,
            "GEO.TS.TRAJECTORY" => self.geo_ts_trajectory(args).await,

            _ => Err(SpatialError::OperationError(format!(
                "Unknown Redis spatial command: {}",
                command
            ))),
        }
    }

    // Standard Redis GEO commands (enhanced implementations)

    /// GEOADD key longitude latitude member [longitude latitude member ...]
    async fn geo_add(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 4 || (args.len() - 1) % 3 != 0 {
            return Err(SpatialError::OperationError(
                "GEOADD requires key and longitude latitude member triplets".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let mut added_count = 0;

        let mut indexes = self.indexes.write().await;
        let dataset = indexes
            .entry(key.clone())
            .or_insert_with(|| SpatialDataSet::new(key.clone()));

        // Process longitude/latitude/member triplets
        for chunk in args[1..].chunks(3) {
            if chunk.len() == 3 {
                let longitude = chunk[0].as_float()?;
                let latitude = chunk[1].as_float()?;
                let member = chunk[2].as_string()?;

                let point = Point::new(longitude, latitude, Some(WGS84_SRID));
                let geometry = SpatialGeometry::Point(point);

                dataset.geometries.insert(member, geometry);
                added_count += 1;
            }
        }

        // Update dataset bounds
        dataset.update_bounds();

        Ok(RedisValue::Integer(added_count))
    }

    /// GEOPOS key member [member ...]
    async fn geo_pos(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "GEOPOS requires key and at least one member".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let indexes = self.indexes.read().await;

        let mut results = Vec::new();

        if let Some(dataset) = indexes.get(&key) {
            for member_val in &args[1..] {
                let member = member_val.as_string()?;

                if let Some(SpatialGeometry::Point(point)) = dataset.geometries.get(&member) {
                    results.push(RedisValue::Array(vec![
                        RedisValue::Float(point.x),
                        RedisValue::Float(point.y),
                    ]));
                } else {
                    results.push(RedisValue::Null);
                }
            }
        } else {
            // Key doesn't exist, return null for all members
            for _ in &args[1..] {
                results.push(RedisValue::Null);
            }
        }

        Ok(RedisValue::Array(results))
    }

    /// GEODIST key member1 member2 [m|km|ft|mi]
    async fn geo_dist(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 3 || args.len() > 4 {
            return Err(SpatialError::OperationError(
                "GEODIST requires key, member1, member2, and optional unit".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let member1 = args[1].as_string()?;
        let member2 = args[2].as_string()?;
        let unit = if args.len() == 4 {
            args[3].as_string()?
        } else {
            "m".to_string()
        };

        let indexes = self.indexes.read().await;

        if let Some(dataset) = indexes.get(&key) {
            if let (Some(SpatialGeometry::Point(p1)), Some(SpatialGeometry::Point(p2))) = (
                dataset.geometries.get(&member1),
                dataset.geometries.get(&member2),
            ) {
                let distance_meters = haversine_distance(p1, p2);
                let distance = match unit.as_str() {
                    "m" => distance_meters,
                    "km" => distance_meters / 1000.0,
                    "ft" => distance_meters * 3.28084,
                    "mi" => distance_meters / 1609.344,
                    _ => {
                        return Err(SpatialError::OperationError(
                            "Invalid unit, use m|km|ft|mi".to_string(),
                        ))
                    }
                };

                Ok(RedisValue::Float(distance))
            } else {
                Ok(RedisValue::Null)
            }
        } else {
            Ok(RedisValue::Null)
        }
    }

    /// GEOHASH key member [member ...]
    async fn geo_hash(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "GEOHASH requires key and at least one member".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let indexes = self.indexes.read().await;

        let mut results = Vec::new();

        if let Some(dataset) = indexes.get(&key) {
            for member_val in &args[1..] {
                let member = member_val.as_string()?;

                if let Some(SpatialGeometry::Point(point)) = dataset.geometries.get(&member) {
                    // Simple geohash implementation (in production, use proper geohash library)
                    let geohash = self.encode_geohash(point.x, point.y, 12);
                    results.push(RedisValue::String(geohash));
                } else {
                    results.push(RedisValue::Null);
                }
            }
        } else {
            for _ in &args[1..] {
                results.push(RedisValue::Null);
            }
        }

        Ok(RedisValue::Array(results))
    }

    /// GEORADIUS key longitude latitude radius m|km|ft|mi [WITHCOORD] [WITHDIST] [COUNT count]
    async fn geo_radius(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 5 {
            return Err(SpatialError::OperationError(
                "GEORADIUS requires key, longitude, latitude, radius, and unit".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let longitude = args[1].as_float()?;
        let latitude = args[2].as_float()?;
        let radius = args[3].as_float()?;
        let unit = args[4].as_string()?;

        // Parse optional parameters
        let mut with_coord = false;
        let mut with_dist = false;
        let mut count_limit = None;

        for arg in &args[5..] {
            match arg.as_string()?.to_uppercase().as_str() {
                "WITHCOORD" => with_coord = true,
                "WITHDIST" => with_dist = true,
                "COUNT" => {} // Next arg will be the count value
                _ => {
                    if let Ok(count) = arg.as_int() {
                        count_limit = Some(count as usize);
                    }
                }
            }
        }

        let center_point = Point::new(longitude, latitude, Some(WGS84_SRID));
        let radius_meters = match unit.as_str() {
            "m" => radius,
            "km" => radius * 1000.0,
            "ft" => radius / 3.28084,
            "mi" => radius * 1609.344,
            _ => {
                return Err(SpatialError::OperationError(
                    "Invalid unit, use m|km|ft|mi".to_string(),
                ))
            }
        };

        let indexes = self.indexes.read().await;
        let mut results = Vec::new();

        if let Some(dataset) = indexes.get(&key) {
            let mut candidates: Vec<(String, f64, &Point)> = Vec::new();

            // Find all points within radius
            for (member, geometry) in &dataset.geometries {
                if let SpatialGeometry::Point(point) = geometry {
                    let distance = haversine_distance(&center_point, point);
                    if distance <= radius_meters {
                        candidates.push((member.clone(), distance, point));
                    }
                }
            }

            // Sort by distance
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // Apply count limit
            if let Some(limit) = count_limit {
                candidates.truncate(limit);
            }

            // Format results
            for (member, distance, point) in candidates {
                let mut result_parts = vec![RedisValue::String(member)];

                if with_dist {
                    let display_distance = match unit.as_str() {
                        "m" => distance,
                        "km" => distance / 1000.0,
                        "ft" => distance * 3.28084,
                        "mi" => distance / 1609.344,
                        _ => distance,
                    };
                    result_parts.push(RedisValue::Float(display_distance));
                }

                if with_coord {
                    result_parts.push(RedisValue::Array(vec![
                        RedisValue::Float(point.x),
                        RedisValue::Float(point.y),
                    ]));
                }

                if with_dist || with_coord {
                    results.push(RedisValue::Array(result_parts));
                } else {
                    results.push(result_parts[0].clone());
                }
            }
        }

        Ok(RedisValue::Array(results))
    }

    /// GEORADIUSBYMEMBER key member radius m|km|ft|mi [WITHCOORD] [WITHDIST] [COUNT count]
    async fn geo_radius_by_member(
        &self,
        args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        if args.len() < 4 {
            return Err(SpatialError::OperationError(
                "GEORADIUSBYMEMBER requires key, member, radius, and unit".to_string(),
            ));
        }

        let key = args[0].as_string()?;
        let member = args[1].as_string()?;

        // Get the member's coordinates
        let coordinates = {
            let indexes = self.indexes.read().await;
            if let Some(dataset) = indexes.get(&key) {
                if let Some(SpatialGeometry::Point(point)) = dataset.geometries.get(&member) {
                    Some((point.x, point.y))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((longitude, latitude)) = coordinates {
            // Create new args with coordinates instead of member
            let mut radius_args = vec![
                args[0].clone(),              // key
                RedisValue::Float(longitude), // longitude
                RedisValue::Float(latitude),  // latitude
            ];
            radius_args.extend_from_slice(&args[2..]); // radius, unit, and options

            self.geo_radius(radius_args).await
        } else {
            Ok(RedisValue::Array(vec![]))
        }
    }

    // Extended spatial commands

    /// GEO.INDEX.CREATE index_name index_type [options]
    async fn geo_index_create(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "GEO.INDEX.CREATE requires index name and type".to_string(),
            ));
        }

        let index_name = args[0].as_string()?;
        let index_type = args[1].as_string()?;

        let mut indexes = self.indexes.write().await;

        let dataset = SpatialDataSet {
            name: index_name.clone(),
            geometries: HashMap::new(),
            metadata: HashMap::new(),
            index_type,
            bounds: None,
        };

        indexes.insert(index_name, dataset);

        Ok(RedisValue::String("OK".to_string()))
    }

    /// GEO.FENCE.ADD fence_name geometry alert_types
    async fn geo_fence_add(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() < 3 {
            return Err(SpatialError::OperationError(
                "GEO.FENCE.ADD requires fence name, geometry, and alert types".to_string(),
            ));
        }

        let fence_name = args[0].as_string()?;
        let geometry_wkt = args[1].as_string()?;
        let alert_types = args[2].as_string()?;

        // Parse geometry (simplified WKT parsing)
        let geometry = self.parse_simple_wkt(&geometry_wkt)?;

        let alert_on_enter = alert_types.to_uppercase().contains("ENTER");
        let alert_on_exit = alert_types.to_uppercase().contains("EXIT");

        let fence_def = GeofenceDefinition {
            id: fence_name.clone(),
            name: fence_name.clone(),
            geometry,
            alert_on_enter,
            alert_on_exit,
            metadata: HashMap::new(),
        };

        let mut geofence_engine = self.geofence_engine.write().await;
        geofence_engine.geofences.insert(fence_name, fence_def);

        Ok(RedisValue::String("OK".to_string()))
    }

    /// GEO.FENCE.CHECK entity_id longitude latitude
    async fn geo_fence_check(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() != 3 {
            return Err(SpatialError::OperationError(
                "GEO.FENCE.CHECK requires entity_id, longitude, and latitude".to_string(),
            ));
        }

        let _entity_id = args[0].as_string()?;
        let longitude = args[1].as_float()?;
        let latitude = args[2].as_float()?;

        let point = Point::new(longitude, latitude, Some(WGS84_SRID));
        let point_geom = SpatialGeometry::Point(point);

        let geofence_engine = self.geofence_engine.read().await;
        let mut violations = Vec::new();

        for (fence_id, fence) in &geofence_engine.geofences {
            match SpatialOperations::intersects(&point_geom, &fence.geometry) {
                Ok(true) => {
                    violations.push(RedisValue::Array(vec![
                        RedisValue::String(fence_id.clone()),
                        RedisValue::String("INSIDE".to_string()),
                    ]));
                }
                Ok(false) => {}
                Err(_) => {}
            }
        }

        Ok(RedisValue::Array(violations))
    }

    /// GEO.CLUSTER.KMEANS index_name k_value
    async fn geo_cluster_kmeans(&self, args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO.CLUSTER.KMEANS requires index name and k value".to_string(),
            ));
        }

        let index_name = args[0].as_string()?;
        let k = args[1].as_int()? as usize;

        let indexes = self.indexes.read().await;

        if let Some(dataset) = indexes.get(&index_name) {
            // Extract points for clustering
            let points: Vec<Point> = dataset
                .geometries
                .values()
                .filter_map(|geom| match geom {
                    SpatialGeometry::Point(p) => Some(p.clone()),
                    _ => None,
                })
                .collect();

            if !points.is_empty() {
                // Use GPU-accelerated clustering
                let cluster_labels = self
                    .gpu_engine
                    .gpu_spatial_clustering(&points, ClusteringAlgorithm::KMeans { k })
                    .await?;

                // Format results
                let mut results = Vec::new();
                let members: Vec<&String> = dataset.geometries.keys().collect();

                for (i, &cluster_id) in cluster_labels.iter().enumerate() {
                    if i < members.len() {
                        results.push(RedisValue::Array(vec![
                            RedisValue::String(members[i].clone()),
                            RedisValue::Integer(cluster_id as i64),
                        ]));
                    }
                }

                Ok(RedisValue::Array(results))
            } else {
                Ok(RedisValue::Array(vec![]))
            }
        } else {
            Err(SpatialError::IndexError("Index not found".to_string()))
        }
    }

    // Placeholder implementations for remaining commands

    async fn geo_index_drop(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_index_info(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_polygon_add(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_linestring_add(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_geometry_get(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Null)
    }

    async fn geo_geometry_del(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Integer(0))
    }

    async fn geo_within(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_intersects(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_contains(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_overlaps(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_fence_del(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Integer(0))
    }

    async fn geo_fence_subscribe(
        &self,
        _args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_fence_list(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_cluster_dbscan(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_density_heatmap(
        &self,
        _args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_nearest_neighbors(
        &self,
        _args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_stream_process(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_stream_subscribe(
        &self,
        _args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::String("OK".to_string()))
    }

    async fn geo_stream_aggregate(
        &self,
        _args: Vec<RedisValue>,
    ) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_ts_add(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Integer(1))
    }

    async fn geo_ts_range(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    async fn geo_ts_trajectory(&self, _args: Vec<RedisValue>) -> Result<RedisValue, SpatialError> {
        Ok(RedisValue::Array(vec![]))
    }

    // Helper functions

    fn encode_geohash(&self, longitude: f64, latitude: f64, precision: u8) -> String {
        // Simplified geohash encoding
        let base32_chars = "0123456789bcdefghjkmnpqrstuvwxyz";
        let mut lat_range = (-90.0, 90.0);
        let mut lon_range = (-180.0, 180.0);
        let mut geohash = String::new();
        let mut is_even = true;
        let mut bit = 0;
        let mut ch = 0;

        while geohash.len() < precision as usize {
            if is_even {
                // Longitude
                let mid = (lon_range.0 + lon_range.1) / 2.0;
                if longitude >= mid {
                    ch |= 1 << (4 - bit);
                    lon_range.0 = mid;
                } else {
                    lon_range.1 = mid;
                }
            } else {
                // Latitude
                let mid = (lat_range.0 + lat_range.1) / 2.0;
                if latitude >= mid {
                    ch |= 1 << (4 - bit);
                    lat_range.0 = mid;
                } else {
                    lat_range.1 = mid;
                }
            }

            is_even = !is_even;
            bit += 1;

            if bit == 5 {
                geohash.push(base32_chars.chars().nth(ch).unwrap());
                bit = 0;
                ch = 0;
            }
        }

        geohash
    }

    fn parse_simple_wkt(&self, wkt: &str) -> Result<SpatialGeometry, SpatialError> {
        let trimmed = wkt.trim().to_uppercase();

        if trimmed.starts_with("POINT") {
            // Extract coordinates from POINT(x y)
            let coords_str = trimmed
                .strip_prefix("POINT")
                .unwrap()
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim();

            let coords: Vec<f64> = coords_str
                .split_whitespace()
                .map(|s| s.parse::<f64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    SpatialError::OperationError("Invalid coordinates in WKT".to_string())
                })?;

            if coords.len() >= 2 {
                let point = Point::new(coords[0], coords[1], Some(WGS84_SRID));
                return Ok(SpatialGeometry::Point(point));
            }
        }

        Err(SpatialError::OperationError(
            "WKT parsing not fully implemented".to_string(),
        ))
    }
}

impl Default for RedisSpatialCommands {
    fn default() -> Self {
        Self::new()
    }
}

impl SpatialDataSet {
    fn new(name: String) -> Self {
        Self {
            name,
            geometries: HashMap::new(),
            metadata: HashMap::new(),
            index_type: "QUADTREE".to_string(),
            bounds: None,
        }
    }

    fn update_bounds(&mut self) {
        if self.geometries.is_empty() {
            self.bounds = None;
            return;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for geometry in self.geometries.values() {
            if let SpatialGeometry::Point(point) = geometry {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
        }

        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            self.bounds = Some(BoundingBox::new(
                min_x,
                min_y,
                max_x,
                max_y,
                Some(WGS84_SRID),
            ));
        }
    }
}

impl GeofenceEngine {
    fn new() -> Self {
        Self {
            geofences: HashMap::new(),
            active_subscriptions: HashMap::new(),
        }
    }
}

impl RedisValue {
    fn as_string(&self) -> Result<String, SpatialError> {
        match self {
            RedisValue::String(s) => Ok(s.clone()),
            _ => Err(SpatialError::OperationError(
                "Value is not a string".to_string(),
            )),
        }
    }

    fn as_float(&self) -> Result<f64, SpatialError> {
        match self {
            RedisValue::Float(f) => Ok(*f),
            RedisValue::Integer(i) => Ok(*i as f64),
            _ => Err(SpatialError::OperationError(
                "Value is not a number".to_string(),
            )),
        }
    }

    fn as_int(&self) -> Result<i64, SpatialError> {
        match self {
            RedisValue::Integer(i) => Ok(*i),
            RedisValue::Float(f) => Ok(*f as i64),
            _ => Err(SpatialError::OperationError(
                "Value is not an integer".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_geo_add() {
        let commands = RedisSpatialCommands::new();

        let args = vec![
            RedisValue::String("cities".to_string()),
            RedisValue::Float(-122.4194),
            RedisValue::Float(37.7749),
            RedisValue::String("San Francisco".to_string()),
        ];

        let result = commands.geo_add(args).await.unwrap();
        match result {
            RedisValue::Integer(count) => assert_eq!(count, 1),
            _ => panic!("Expected integer count"),
        }
    }

    #[tokio::test]
    async fn test_geo_dist() {
        let commands = RedisSpatialCommands::new();

        // Add two cities
        let add_args1 = vec![
            RedisValue::String("cities".to_string()),
            RedisValue::Float(-122.4194),
            RedisValue::Float(37.7749),
            RedisValue::String("San Francisco".to_string()),
        ];
        commands.geo_add(add_args1).await.unwrap();

        let add_args2 = vec![
            RedisValue::String("cities".to_string()),
            RedisValue::Float(-74.0060),
            RedisValue::Float(40.7128),
            RedisValue::String("New York".to_string()),
        ];
        commands.geo_add(add_args2).await.unwrap();

        // Calculate distance
        let dist_args = vec![
            RedisValue::String("cities".to_string()),
            RedisValue::String("San Francisco".to_string()),
            RedisValue::String("New York".to_string()),
            RedisValue::String("km".to_string()),
        ];

        let result = commands.geo_dist(dist_args).await.unwrap();
        match result {
            RedisValue::Float(distance) => {
                // Should be approximately 4135 km
                assert!((distance - 4135.0).abs() < 100.0);
            }
            _ => panic!("Expected float distance"),
        }
    }

    #[tokio::test]
    async fn test_geo_cluster_kmeans() {
        let commands = RedisSpatialCommands::new();

        // Add some test points
        let add_args = vec![
            RedisValue::String("test_points".to_string()),
            RedisValue::Float(1.0),
            RedisValue::Float(1.0),
            RedisValue::String("p1".to_string()),
            RedisValue::Float(1.5),
            RedisValue::Float(1.5),
            RedisValue::String("p2".to_string()),
            RedisValue::Float(10.0),
            RedisValue::Float(10.0),
            RedisValue::String("p3".to_string()),
            RedisValue::Float(10.5),
            RedisValue::Float(10.5),
            RedisValue::String("p4".to_string()),
        ];
        commands.geo_add(add_args).await.unwrap();

        // Perform K-means clustering
        let cluster_args = vec![
            RedisValue::String("test_points".to_string()),
            RedisValue::Integer(2),
        ];

        let result = commands.geo_cluster_kmeans(cluster_args).await.unwrap();
        match result {
            RedisValue::Array(clusters) => {
                assert_eq!(clusters.len(), 4); // 4 points
                                               // Verify each result has member name and cluster id
                for cluster_result in clusters {
                    match cluster_result {
                        RedisValue::Array(parts) => {
                            assert_eq!(parts.len(), 2); // member name and cluster id
                        }
                        _ => panic!("Expected array result for each cluster"),
                    }
                }
            }
            _ => panic!("Expected array of cluster results"),
        }
    }
}
