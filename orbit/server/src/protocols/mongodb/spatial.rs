//! MongoDB Geospatial Operations
//!
//! This module provides MongoDB-compatible geospatial operations using the shared
//! `orbit_shared::spatial` module. It supports:
//!
//! - `$geoNear` aggregation pipeline stage
//! - `$geoWithin` query operator (with $box, $polygon, $center, $centerSphere)
//! - `$near` and `$nearSphere` query operators
//! - GeoJSON and legacy coordinate pair formats
//!
//! ## Architecture
//!
//! ```text
//! MongoDB Query                    Shared Spatial Module
//! ┌─────────────────┐             ┌─────────────────────┐
//! │ $geoNear        │────────────▶│ SpatialIndex        │
//! │ $geoWithin      │             │ - nearest_neighbors │
//! │ $near           │             │ - query_bbox        │
//! │ $nearSphere     │             ├─────────────────────┤
//! └─────────────────┘             │ SpatialOperations   │
//!         │                       │ - point_in_polygon  │
//!         ▼                       │ - distance          │
//! ┌─────────────────┐             │ - within            │
//! │ GeoJSON Parser  │             └─────────────────────┘
//! │ - Point         │                      │
//! │ - Polygon       │                      ▼
//! │ - LineString    │             ┌─────────────────────┐
//! └─────────────────┘             │ crs::utils          │
//!                                 │ - haversine_distance│
//!                                 └─────────────────────┘
//! ```

use orbit_shared::spatial::{
    crs::utils::haversine_distance, BoundingBox, LinearRing, Point, Polygon, SpatialFunctions,
    SpatialGeometry, SpatialOperations, WGS84_SRID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// MongoDB geospatial engine using shared spatial module
pub struct MongoSpatialEngine {
    spatial_functions: SpatialFunctions,
}

/// Result from $geoNear aggregation stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoNearResult {
    /// Document ID
    pub doc_id: String,
    /// Distance from the query point (in meters for sphere, units for flat)
    pub distance: f64,
    /// The location field value
    pub location: GeoJsonPoint,
    /// Original document fields
    pub document: HashMap<String, JsonValue>,
}

/// GeoJSON Point representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoJsonPoint {
    #[serde(rename = "type")]
    pub geo_type: String,
    pub coordinates: Vec<f64>,
}

/// GeoJSON Polygon representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoJsonPolygon {
    #[serde(rename = "type")]
    pub geo_type: String,
    /// Coordinates: [exterior_ring, ...interior_rings]
    /// Each ring: [[lon, lat], [lon, lat], ...]
    pub coordinates: Vec<Vec<Vec<f64>>>,
}

/// $geoNear aggregation stage configuration
#[derive(Debug, Clone)]
pub struct GeoNearConfig {
    /// The point for which to find the closest documents
    pub near: Point,
    /// The output field that contains the calculated distance
    pub distance_field: String,
    /// If true, calculate spherical distance (meters)
    pub spherical: bool,
    /// Maximum distance in meters (for spherical) or units (for flat)
    pub max_distance: Option<f64>,
    /// Minimum distance
    pub min_distance: Option<f64>,
    /// Additional query to filter documents
    pub query: Option<JsonValue>,
    /// Field that contains the location data
    pub key: Option<String>,
    /// Include the matched location in the output
    pub include_locs: Option<String>,
    /// Multiply all distances by this factor
    pub distance_multiplier: Option<f64>,
    /// Maximum number of documents to return
    pub limit: Option<usize>,
}

/// $geoWithin query operator types
#[derive(Debug, Clone)]
pub enum GeoWithinShape {
    /// $box: [[bottom-left], [top-right]]
    Box(BoundingBox),
    /// $polygon: [[p1], [p2], [p3], ...]
    Polygon(Polygon),
    /// $center: [[x, y], radius]
    Center { center: Point, radius: f64 },
    /// $centerSphere: [[lon, lat], radius_in_radians]
    CenterSphere { center: Point, radius_radians: f64 },
    /// GeoJSON geometry
    Geometry(SpatialGeometry),
}

/// $near query operator configuration
#[derive(Debug, Clone)]
pub struct NearConfig {
    /// Query point
    pub point: Point,
    /// Use spherical calculations
    pub spherical: bool,
    /// Maximum distance (meters for sphere, units for flat)
    pub max_distance: Option<f64>,
    /// Minimum distance
    pub min_distance: Option<f64>,
}

/// Error types for MongoDB geospatial operations
#[derive(Debug, thiserror::Error)]
pub enum MongoSpatialError {
    #[error("Invalid GeoJSON: {0}")]
    InvalidGeoJson(String),

    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),

    #[error("Spatial operation error: {0}")]
    SpatialError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type MongoSpatialResult<T> = Result<T, MongoSpatialError>;

impl MongoSpatialEngine {
    /// Create a new MongoDB spatial engine
    pub fn new() -> Self {
        Self {
            spatial_functions: SpatialFunctions::new(),
        }
    }

    // =========================================================================
    // GeoJSON Parsing
    // =========================================================================

    /// Parse GeoJSON point from JSON value
    pub fn parse_geojson_point(&self, value: &JsonValue) -> MongoSpatialResult<Point> {
        // Handle GeoJSON format: { type: "Point", coordinates: [lon, lat] }
        if let Some(geo_type) = value.get("type").and_then(|t| t.as_str()) {
            if geo_type == "Point" {
                if let Some(coords) = value.get("coordinates").and_then(|c| c.as_array()) {
                    if coords.len() >= 2 {
                        let lon = coords[0].as_f64().ok_or_else(|| {
                            MongoSpatialError::InvalidCoordinates(
                                "Invalid longitude in GeoJSON".to_string(),
                            )
                        })?;
                        let lat = coords[1].as_f64().ok_or_else(|| {
                            MongoSpatialError::InvalidCoordinates(
                                "Invalid latitude in GeoJSON".to_string(),
                            )
                        })?;

                        let z = coords.get(2).and_then(|v| v.as_f64());
                        return Ok(if let Some(elevation) = z {
                            Point::with_elevation(lon, lat, elevation, Some(WGS84_SRID))
                        } else {
                            Point::new(lon, lat, Some(WGS84_SRID))
                        });
                    }
                }
            }
        }

        // Handle legacy coordinate pair: [lon, lat] or { lon: x, lat: y }
        if let Some(arr) = value.as_array() {
            if arr.len() >= 2 {
                let lon = arr[0].as_f64().ok_or_else(|| {
                    MongoSpatialError::InvalidCoordinates("Invalid longitude".to_string())
                })?;
                let lat = arr[1].as_f64().ok_or_else(|| {
                    MongoSpatialError::InvalidCoordinates("Invalid latitude".to_string())
                })?;
                return Ok(Point::new(lon, lat, Some(WGS84_SRID)));
            }
        }

        // Handle object format: { lon/lng/x: value, lat/y: value }
        if let Some(obj) = value.as_object() {
            let lon = obj
                .get("lon")
                .or_else(|| obj.get("lng"))
                .or_else(|| obj.get("x"))
                .and_then(|v| v.as_f64());
            let lat = obj
                .get("lat")
                .or_else(|| obj.get("y"))
                .and_then(|v| v.as_f64());

            if let (Some(lon), Some(lat)) = (lon, lat) {
                return Ok(Point::new(lon, lat, Some(WGS84_SRID)));
            }
        }

        Err(MongoSpatialError::InvalidGeoJson(
            "Unable to parse point from value".to_string(),
        ))
    }

    /// Parse GeoJSON polygon from JSON value
    pub fn parse_geojson_polygon(&self, value: &JsonValue) -> MongoSpatialResult<Polygon> {
        // Handle GeoJSON format
        if let Some(geo_type) = value.get("type").and_then(|t| t.as_str()) {
            if geo_type == "Polygon" {
                if let Some(coords) = value.get("coordinates").and_then(|c| c.as_array()) {
                    return self.parse_polygon_coordinates(coords);
                }
            }
        }

        // Handle raw coordinate array
        if let Some(coords) = value.as_array() {
            return self.parse_polygon_coordinates(coords);
        }

        Err(MongoSpatialError::InvalidGeoJson(
            "Unable to parse polygon from value".to_string(),
        ))
    }

    /// Parse polygon coordinates from array
    fn parse_polygon_coordinates(&self, coords: &[JsonValue]) -> MongoSpatialResult<Polygon> {
        if coords.is_empty() {
            return Err(MongoSpatialError::InvalidGeoJson(
                "Polygon must have at least one ring".to_string(),
            ));
        }

        // Parse exterior ring
        let exterior_ring = self.parse_linear_ring(coords[0].as_array().ok_or_else(|| {
            MongoSpatialError::InvalidGeoJson("Invalid ring format".to_string())
        })?)?;

        // Parse interior rings (holes)
        let mut interior_rings = Vec::new();
        for ring_value in coords.iter().skip(1) {
            if let Some(ring_coords) = ring_value.as_array() {
                interior_rings.push(self.parse_linear_ring(ring_coords)?);
            }
        }

        Polygon::new(exterior_ring, interior_rings, Some(WGS84_SRID))
            .map_err(|e| MongoSpatialError::SpatialError(e.to_string()))
    }

    /// Parse a linear ring from coordinate array
    fn parse_linear_ring(&self, coords: &[JsonValue]) -> MongoSpatialResult<LinearRing> {
        let mut points = Vec::new();

        for coord in coords {
            if let Some(pair) = coord.as_array() {
                if pair.len() >= 2 {
                    let lon = pair[0].as_f64().ok_or_else(|| {
                        MongoSpatialError::InvalidCoordinates("Invalid lon".to_string())
                    })?;
                    let lat = pair[1].as_f64().ok_or_else(|| {
                        MongoSpatialError::InvalidCoordinates("Invalid lat".to_string())
                    })?;
                    points.push(Point::new(lon, lat, Some(WGS84_SRID)));
                }
            }
        }

        LinearRing::new(points).map_err(|e| MongoSpatialError::SpatialError(e.to_string()))
    }

    // =========================================================================
    // $geoNear Aggregation Stage
    // =========================================================================

    /// Execute $geoNear aggregation stage
    ///
    /// Returns documents sorted by distance from the specified point
    pub fn geo_near(
        &self,
        documents: &[(String, HashMap<String, JsonValue>)],
        config: &GeoNearConfig,
        location_field: &str,
    ) -> MongoSpatialResult<Vec<GeoNearResult>> {
        let mut results = Vec::new();

        for (doc_id, doc) in documents {
            // Get location from document
            let location = match doc.get(location_field) {
                Some(loc) => self.parse_geojson_point(loc)?,
                None => continue, // Skip documents without location
            };

            // Calculate distance
            let distance = if config.spherical {
                haversine_distance(&config.near, &location)
            } else {
                config.near.distance_2d(&location)
            };

            // Apply distance multiplier if set
            let final_distance = if let Some(multiplier) = config.distance_multiplier {
                distance * multiplier
            } else {
                distance
            };

            // Check distance constraints
            if let Some(max_dist) = config.max_distance {
                if final_distance > max_dist {
                    continue;
                }
            }

            if let Some(min_dist) = config.min_distance {
                if final_distance < min_dist {
                    continue;
                }
            }

            results.push(GeoNearResult {
                doc_id: doc_id.clone(),
                distance: final_distance,
                location: GeoJsonPoint {
                    geo_type: "Point".to_string(),
                    coordinates: vec![location.x, location.y],
                },
                document: doc.clone(),
            });
        }

        // Sort by distance
        results.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        if let Some(limit) = config.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    // =========================================================================
    // $geoWithin Query Operator
    // =========================================================================

    /// Execute $geoWithin query
    ///
    /// Returns true if the point is within the specified shape
    pub fn geo_within(&self, point: &Point, shape: &GeoWithinShape) -> MongoSpatialResult<bool> {
        match shape {
            GeoWithinShape::Box(bbox) => Ok(bbox.contains_point(point)),

            GeoWithinShape::Polygon(polygon) => SpatialOperations::point_in_polygon(point, polygon)
                .map_err(|e| MongoSpatialError::SpatialError(e.to_string())),

            GeoWithinShape::Center { center, radius } => {
                let distance = center.distance_2d(point);
                Ok(distance <= *radius)
            }

            GeoWithinShape::CenterSphere {
                center,
                radius_radians,
            } => {
                // Convert radius from radians to meters (Earth's mean radius)
                let radius_meters = radius_radians * orbit_shared::spatial::EARTH_RADIUS_METERS;
                let distance = haversine_distance(center, point);
                Ok(distance <= radius_meters)
            }

            GeoWithinShape::Geometry(geometry) => {
                let point_geom = SpatialGeometry::Point(point.clone());
                SpatialOperations::within(&point_geom, geometry)
                    .map_err(|e| MongoSpatialError::SpatialError(e.to_string()))
            }
        }
    }

    /// Filter documents by $geoWithin
    pub fn filter_geo_within(
        &self,
        documents: &[(String, HashMap<String, JsonValue>)],
        location_field: &str,
        shape: &GeoWithinShape,
    ) -> MongoSpatialResult<Vec<(String, HashMap<String, JsonValue>)>> {
        let mut results = Vec::new();

        for (doc_id, doc) in documents {
            if let Some(loc_value) = doc.get(location_field) {
                let point = self.parse_geojson_point(loc_value)?;
                if self.geo_within(&point, shape)? {
                    results.push((doc_id.clone(), doc.clone()));
                }
            }
        }

        Ok(results)
    }

    // =========================================================================
    // $near / $nearSphere Query Operators
    // =========================================================================

    /// Execute $near query
    ///
    /// Returns documents sorted by distance from query point
    #[allow(clippy::type_complexity)]
    pub fn near(
        &self,
        documents: &[(String, HashMap<String, JsonValue>)],
        location_field: &str,
        config: &NearConfig,
    ) -> MongoSpatialResult<Vec<(String, HashMap<String, JsonValue>, f64)>> {
        let mut results = Vec::new();

        for (doc_id, doc) in documents {
            if let Some(loc_value) = doc.get(location_field) {
                let point = self.parse_geojson_point(loc_value)?;

                let distance = if config.spherical {
                    haversine_distance(&config.point, &point)
                } else {
                    config.point.distance_2d(&point)
                };

                // Apply distance constraints
                if let Some(max_dist) = config.max_distance {
                    if distance > max_dist {
                        continue;
                    }
                }

                if let Some(min_dist) = config.min_distance {
                    if distance < min_dist {
                        continue;
                    }
                }

                results.push((doc_id.clone(), doc.clone(), distance));
            }
        }

        // Sort by distance
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    // =========================================================================
    // $geoIntersects Query Operator
    // =========================================================================

    /// Execute $geoIntersects query
    pub fn geo_intersects(
        &self,
        geometry1: &SpatialGeometry,
        geometry2: &SpatialGeometry,
    ) -> MongoSpatialResult<bool> {
        SpatialOperations::intersects(geometry1, geometry2)
            .map_err(|e| MongoSpatialError::SpatialError(e.to_string()))
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Parse $geoWithin shape from JSON value
    pub fn parse_geo_within_shape(&self, value: &JsonValue) -> MongoSpatialResult<GeoWithinShape> {
        // $box: { $box: [[x1, y1], [x2, y2]] }
        if let Some(box_coords) = value.get("$box").and_then(|v| v.as_array()) {
            if box_coords.len() >= 2 {
                let p1 = self.parse_geojson_point(&box_coords[0])?;
                let p2 = self.parse_geojson_point(&box_coords[1])?;

                return Ok(GeoWithinShape::Box(BoundingBox::new(
                    p1.x.min(p2.x),
                    p1.y.min(p2.y),
                    p1.x.max(p2.x),
                    p1.y.max(p2.y),
                    Some(WGS84_SRID),
                )));
            }
        }

        // $polygon: { $polygon: [[x1, y1], [x2, y2], ...] }
        if let Some(poly_coords) = value.get("$polygon").and_then(|v| v.as_array()) {
            let polygon = self.parse_polygon_from_legacy(poly_coords)?;
            return Ok(GeoWithinShape::Polygon(polygon));
        }

        // $center: { $center: [[x, y], radius] }
        if let Some(center_def) = value.get("$center").and_then(|v| v.as_array()) {
            if center_def.len() >= 2 {
                let center = self.parse_geojson_point(&center_def[0])?;
                let radius = center_def[1].as_f64().ok_or_else(|| {
                    MongoSpatialError::InvalidCoordinates("Invalid radius".to_string())
                })?;
                return Ok(GeoWithinShape::Center { center, radius });
            }
        }

        // $centerSphere: { $centerSphere: [[lon, lat], radius_in_radians] }
        if let Some(sphere_def) = value.get("$centerSphere").and_then(|v| v.as_array()) {
            if sphere_def.len() >= 2 {
                let center = self.parse_geojson_point(&sphere_def[0])?;
                let radius_radians = sphere_def[1].as_f64().ok_or_else(|| {
                    MongoSpatialError::InvalidCoordinates("Invalid radius".to_string())
                })?;
                return Ok(GeoWithinShape::CenterSphere {
                    center,
                    radius_radians,
                });
            }
        }

        // $geometry: { $geometry: { type: "Polygon", coordinates: [...] } }
        if let Some(geom) = value.get("$geometry") {
            let geometry = self.parse_geojson_geometry(geom)?;
            return Ok(GeoWithinShape::Geometry(geometry));
        }

        // Direct GeoJSON geometry
        if value.get("type").is_some() {
            let geometry = self.parse_geojson_geometry(value)?;
            return Ok(GeoWithinShape::Geometry(geometry));
        }

        Err(MongoSpatialError::InvalidGeoJson(
            "Unknown $geoWithin shape".to_string(),
        ))
    }

    /// Parse legacy polygon format [[x1, y1], [x2, y2], ...]
    fn parse_polygon_from_legacy(&self, coords: &[JsonValue]) -> MongoSpatialResult<Polygon> {
        let mut points = Vec::new();

        for coord in coords {
            let point = self.parse_geojson_point(coord)?;
            points.push(point);
        }

        // Ensure ring is closed
        if points.len() >= 3 && points.first() != points.last() {
            points.push(points[0].clone());
        }

        let ring =
            LinearRing::new(points).map_err(|e| MongoSpatialError::SpatialError(e.to_string()))?;

        Polygon::new(ring, vec![], Some(WGS84_SRID))
            .map_err(|e| MongoSpatialError::SpatialError(e.to_string()))
    }

    /// Parse GeoJSON geometry
    fn parse_geojson_geometry(&self, value: &JsonValue) -> MongoSpatialResult<SpatialGeometry> {
        let geo_type = value.get("type").and_then(|t| t.as_str()).ok_or_else(|| {
            MongoSpatialError::InvalidGeoJson("Missing geometry type".to_string())
        })?;

        match geo_type {
            "Point" => {
                let point = self.parse_geojson_point(value)?;
                Ok(SpatialGeometry::Point(point))
            }
            "Polygon" => {
                let polygon = self.parse_geojson_polygon(value)?;
                Ok(SpatialGeometry::Polygon(polygon))
            }
            _ => Err(MongoSpatialError::InvalidGeoJson(format!(
                "Unsupported geometry type: {}",
                geo_type
            ))),
        }
    }

    /// Convert Point to GeoJSON
    pub fn point_to_geojson(&self, point: &Point) -> GeoJsonPoint {
        GeoJsonPoint {
            geo_type: "Point".to_string(),
            coordinates: if let Some(z) = point.z {
                vec![point.x, point.y, z]
            } else {
                vec![point.x, point.y]
            },
        }
    }

    /// Get the underlying spatial functions
    pub fn spatial_functions(&self) -> &SpatialFunctions {
        &self.spatial_functions
    }
}

impl Default for MongoSpatialEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_geojson_point() {
        let engine = MongoSpatialEngine::new();

        // GeoJSON format
        let geojson = json!({
            "type": "Point",
            "coordinates": [-122.4194, 37.7749]
        });
        let point = engine.parse_geojson_point(&geojson).unwrap();
        assert_eq!(point.x, -122.4194);
        assert_eq!(point.y, 37.7749);

        // Legacy array format
        let legacy = json!([-122.4194, 37.7749]);
        let point = engine.parse_geojson_point(&legacy).unwrap();
        assert_eq!(point.x, -122.4194);
        assert_eq!(point.y, 37.7749);

        // Object format
        let obj = json!({"lon": -122.4194, "lat": 37.7749});
        let point = engine.parse_geojson_point(&obj).unwrap();
        assert_eq!(point.x, -122.4194);
        assert_eq!(point.y, 37.7749);
    }

    #[test]
    fn test_parse_geojson_polygon() {
        let engine = MongoSpatialEngine::new();

        let geojson = json!({
            "type": "Polygon",
            "coordinates": [
                [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]
            ]
        });

        let polygon = engine.parse_geojson_polygon(&geojson).unwrap();
        assert_eq!(polygon.exterior_ring.points.len(), 5);
    }

    #[test]
    fn test_geo_within_box() {
        let engine = MongoSpatialEngine::new();

        let bbox = BoundingBox::new(0.0, 0.0, 10.0, 10.0, None);
        let shape = GeoWithinShape::Box(bbox);

        let inside = Point::new(5.0, 5.0, None);
        let outside = Point::new(15.0, 15.0, None);

        assert!(engine.geo_within(&inside, &shape).unwrap());
        assert!(!engine.geo_within(&outside, &shape).unwrap());
    }

    #[test]
    fn test_geo_within_center() {
        let engine = MongoSpatialEngine::new();

        let shape = GeoWithinShape::Center {
            center: Point::new(0.0, 0.0, None),
            radius: 10.0,
        };

        let inside = Point::new(5.0, 5.0, None); // ~7.07 units from center
        let outside = Point::new(10.0, 10.0, None); // ~14.14 units from center

        assert!(engine.geo_within(&inside, &shape).unwrap());
        assert!(!engine.geo_within(&outside, &shape).unwrap());
    }

    #[test]
    fn test_geo_near() {
        let engine = MongoSpatialEngine::new();

        let documents = vec![
            ("doc1".to_string(), {
                let mut map = HashMap::new();
                map.insert(
                    "location".to_string(),
                    json!({"type": "Point", "coordinates": [0.0, 0.0]}),
                );
                map.insert("name".to_string(), json!("Origin"));
                map
            }),
            ("doc2".to_string(), {
                let mut map = HashMap::new();
                map.insert(
                    "location".to_string(),
                    json!({"type": "Point", "coordinates": [1.0, 1.0]}),
                );
                map.insert("name".to_string(), json!("Near"));
                map
            }),
            ("doc3".to_string(), {
                let mut map = HashMap::new();
                map.insert(
                    "location".to_string(),
                    json!({"type": "Point", "coordinates": [10.0, 10.0]}),
                );
                map.insert("name".to_string(), json!("Far"));
                map
            }),
        ];

        let config = GeoNearConfig {
            near: Point::new(0.0, 0.0, None),
            distance_field: "distance".to_string(),
            spherical: false,
            max_distance: Some(5.0),
            min_distance: None,
            query: None,
            key: None,
            include_locs: None,
            distance_multiplier: None,
            limit: None,
        };

        let results = engine.geo_near(&documents, &config, "location").unwrap();

        // Should return doc1 and doc2 (within max_distance of 5.0)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].doc_id, "doc1"); // Closest
        assert_eq!(results[1].doc_id, "doc2");
    }

    #[test]
    fn test_parse_geo_within_shape() {
        let engine = MongoSpatialEngine::new();

        // Test $box
        let box_value = json!({
            "$box": [[0.0, 0.0], [10.0, 10.0]]
        });
        let shape = engine.parse_geo_within_shape(&box_value).unwrap();
        assert!(matches!(shape, GeoWithinShape::Box(_)));

        // Test $center
        let center_value = json!({
            "$center": [[5.0, 5.0], 10.0]
        });
        let shape = engine.parse_geo_within_shape(&center_value).unwrap();
        assert!(matches!(shape, GeoWithinShape::Center { .. }));

        // Test $centerSphere
        let sphere_value = json!({
            "$centerSphere": [[-122.4194, 37.7749], 0.001]
        });
        let shape = engine.parse_geo_within_shape(&sphere_value).unwrap();
        assert!(matches!(shape, GeoWithinShape::CenterSphere { .. }));
    }

    #[test]
    fn test_near_query() {
        let engine = MongoSpatialEngine::new();

        let documents = vec![
            ("a".to_string(), {
                let mut map = HashMap::new();
                map.insert("loc".to_string(), json!([3.0, 4.0]));
                map
            }),
            ("b".to_string(), {
                let mut map = HashMap::new();
                map.insert("loc".to_string(), json!([0.0, 1.0]));
                map
            }),
        ];

        let config = NearConfig {
            point: Point::new(0.0, 0.0, None),
            spherical: false,
            max_distance: None,
            min_distance: None,
        };

        let results = engine.near(&documents, "loc", &config).unwrap();

        // b should be first (distance 1.0), a second (distance 5.0)
        assert_eq!(results[0].0, "b");
        assert!((results[0].2 - 1.0).abs() < 0.001);
        assert_eq!(results[1].0, "a");
        assert!((results[1].2 - 5.0).abs() < 0.001);
    }
}
