//! CQL Geospatial User-Defined Functions (UDFs)
//!
//! This module provides geospatial UDFs for CQL (Cassandra Query Language) using
//! the shared `orbit_shared::spatial` module. These UDFs can be used in CQL queries
//! for spatial operations.
//!
//! ## Supported UDFs
//!
//! | Function | Description |
//! |----------|-------------|
//! | `geo_distance(p1, p2)` | Calculate distance between two points |
//! | `geo_distance_sphere(p1, p2)` | Calculate spherical (great-circle) distance |
//! | `geo_within(point, polygon)` | Test if point is within polygon |
//! | `geo_contains(polygon, point)` | Test if polygon contains point |
//! | `geo_near(point, center, radius)` | Test if point is within radius of center |
//! | `geo_bbox(point, minX, minY, maxX, maxY)` | Test if point is within bounding box |
//! | `geo_intersects(geom1, geom2)` | Test if two geometries intersect |
//!
//! ## CQL Usage Examples
//!
//! ```sql
//! -- Find locations within 10km of a point
//! SELECT * FROM locations
//! WHERE geo_distance_sphere(location, geo_point(-122.4194, 37.7749)) < 10000;
//!
//! -- Find locations within a polygon
//! SELECT * FROM locations
//! WHERE geo_within(location, geo_polygon('POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))'));
//!
//! -- Find locations within a bounding box
//! SELECT * FROM locations
//! WHERE geo_bbox(location, -123.0, 37.0, -122.0, 38.0);
//! ```
//!
//! ## Architecture
//!
//! ```text
//! CQL Query                        Shared Spatial Module
//! ┌─────────────────────┐         ┌─────────────────────┐
//! │ geo_distance()      │────────▶│ SpatialOperations   │
//! │ geo_within()        │         │ - distance()        │
//! │ geo_contains()      │         │ - point_in_polygon()│
//! │ geo_near()          │         │ - contains()        │
//! │ geo_bbox()          │         │ - within()          │
//! └─────────────────────┘         └─────────────────────┘
//!         │                                │
//!         ▼                                ▼
//! ┌─────────────────────┐         ┌─────────────────────┐
//! │ CqlSpatialUdfs      │         │ SpatialFunctions    │
//! │ - parse_point()     │         │ - st_geomfromtext() │
//! │ - parse_polygon()   │         │ - st_distance()     │
//! │ - parse_wkt()       │         │ - st_within()       │
//! └─────────────────────┘         └─────────────────────┘
//!                                          │
//!                                          ▼
//!                                 ┌─────────────────────┐
//!                                 │ crs::utils          │
//!                                 │ - haversine_distance│
//!                                 └─────────────────────┘
//! ```

use orbit_shared::spatial::{
    crs::utils::haversine_distance, BoundingBox, LinearRing, Point, Polygon, SpatialFunctions,
    SpatialGeometry, SpatialOperations, WGS84_SRID,
};
use serde::{Deserialize, Serialize};

/// CQL Geospatial UDF engine using shared spatial module
pub struct CqlSpatialUdfs {
    spatial_functions: SpatialFunctions,
}

/// CQL Point type (stored as tuple or frozen<point>)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CqlPoint {
    pub longitude: f64,
    pub latitude: f64,
}

/// CQL Polygon type (stored as frozen<polygon> or text WKT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CqlPolygon {
    /// WKT representation or coordinates
    pub wkt: String,
}

/// Spatial UDF result value
#[derive(Debug, Clone)]
pub enum SpatialValue {
    Float(f64),
    Boolean(bool),
    Point(CqlPoint),
    Polygon(CqlPolygon),
    Null,
}

/// Error types for CQL spatial operations
#[derive(Debug, thiserror::Error)]
pub enum CqlSpatialError {
    #[error("Invalid point format: {0}")]
    InvalidPoint(String),

    #[error("Invalid polygon format: {0}")]
    InvalidPolygon(String),

    #[error("Invalid WKT: {0}")]
    InvalidWkt(String),

    #[error("Spatial operation error: {0}")]
    SpatialError(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

pub type CqlSpatialResult<T> = Result<T, CqlSpatialError>;

impl CqlSpatialUdfs {
    /// Create a new CQL spatial UDF engine
    pub fn new() -> Self {
        Self {
            spatial_functions: SpatialFunctions::new(),
        }
    }

    // =========================================================================
    // Point/Geometry Parsing
    // =========================================================================

    /// Parse CQL point from tuple (longitude, latitude)
    pub fn parse_point(&self, lon: f64, lat: f64) -> Point {
        Point::new(lon, lat, Some(WGS84_SRID))
    }

    /// Parse CQL point from CqlPoint struct
    pub fn parse_cql_point(&self, cql_point: &CqlPoint) -> Point {
        Point::new(cql_point.longitude, cql_point.latitude, Some(WGS84_SRID))
    }

    /// Parse polygon from WKT string
    pub fn parse_polygon_wkt(&self, wkt: &str) -> CqlSpatialResult<Polygon> {
        let geometry = self
            .spatial_functions
            .st_geomfromtext(wkt, Some(WGS84_SRID))
            .map_err(|e| CqlSpatialError::InvalidWkt(e.to_string()))?;

        match geometry {
            SpatialGeometry::Polygon(p) => Ok(p),
            _ => Err(CqlSpatialError::InvalidPolygon(
                "WKT does not represent a polygon".to_string(),
            )),
        }
    }

    /// Parse geometry from WKT string
    pub fn parse_geometry_wkt(&self, wkt: &str) -> CqlSpatialResult<SpatialGeometry> {
        self.spatial_functions
            .st_geomfromtext(wkt, Some(WGS84_SRID))
            .map_err(|e| CqlSpatialError::InvalidWkt(e.to_string()))
    }

    // =========================================================================
    // UDF: geo_point(longitude, latitude)
    // =========================================================================

    /// Create a point from longitude and latitude
    ///
    /// ```sql
    /// SELECT geo_point(-122.4194, 37.7749) AS point FROM ...;
    /// ```
    pub fn geo_point(&self, longitude: f64, latitude: f64) -> CqlPoint {
        CqlPoint {
            longitude,
            latitude,
        }
    }

    // =========================================================================
    // UDF: geo_polygon(wkt)
    // =========================================================================

    /// Create a polygon from WKT string
    ///
    /// ```sql
    /// SELECT geo_polygon('POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))') AS polygon FROM ...;
    /// ```
    pub fn geo_polygon(&self, wkt: &str) -> CqlSpatialResult<CqlPolygon> {
        // Validate by parsing
        let _ = self.parse_polygon_wkt(wkt)?;
        Ok(CqlPolygon {
            wkt: wkt.to_string(),
        })
    }

    // =========================================================================
    // UDF: geo_distance(p1, p2)
    // =========================================================================

    /// Calculate Euclidean distance between two points (in coordinate units)
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_distance(location, geo_point(0, 0)) < 10;
    /// ```
    pub fn geo_distance(&self, p1: &CqlPoint, p2: &CqlPoint) -> f64 {
        let point1 = self.parse_cql_point(p1);
        let point2 = self.parse_cql_point(p2);
        point1.distance_2d(&point2)
    }

    /// Calculate distance between two Points (internal)
    pub fn geo_distance_points(&self, p1: &Point, p2: &Point) -> f64 {
        p1.distance_2d(p2)
    }

    // =========================================================================
    // UDF: geo_distance_sphere(p1, p2)
    // =========================================================================

    /// Calculate great-circle (spherical) distance between two points in meters
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_distance_sphere(location, geo_point(-122.4194, 37.7749)) < 10000;
    /// ```
    pub fn geo_distance_sphere(&self, p1: &CqlPoint, p2: &CqlPoint) -> f64 {
        let point1 = self.parse_cql_point(p1);
        let point2 = self.parse_cql_point(p2);
        haversine_distance(&point1, &point2)
    }

    /// Calculate spherical distance between two Points (internal)
    pub fn geo_distance_sphere_points(&self, p1: &Point, p2: &Point) -> f64 {
        haversine_distance(p1, p2)
    }

    // =========================================================================
    // UDF: geo_within(point, polygon)
    // =========================================================================

    /// Test if a point is within a polygon
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_within(location, geo_polygon('POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))'));
    /// ```
    pub fn geo_within(&self, point: &CqlPoint, polygon_wkt: &str) -> CqlSpatialResult<bool> {
        let p = self.parse_cql_point(point);
        let polygon = self.parse_polygon_wkt(polygon_wkt)?;

        SpatialOperations::point_in_polygon(&p, &polygon)
            .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))
    }

    /// Test if a Point is within a Polygon (internal)
    pub fn geo_within_points(&self, point: &Point, polygon: &Polygon) -> CqlSpatialResult<bool> {
        SpatialOperations::point_in_polygon(point, polygon)
            .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))
    }

    // =========================================================================
    // UDF: geo_contains(polygon, point)
    // =========================================================================

    /// Test if a polygon contains a point (reverse of geo_within)
    ///
    /// ```sql
    /// SELECT * FROM regions
    /// WHERE geo_contains(boundary, geo_point(-122.4194, 37.7749));
    /// ```
    pub fn geo_contains(&self, polygon_wkt: &str, point: &CqlPoint) -> CqlSpatialResult<bool> {
        self.geo_within(point, polygon_wkt)
    }

    // =========================================================================
    // UDF: geo_near(point, center, radius)
    // =========================================================================

    /// Test if a point is within a given radius (in coordinate units) of a center point
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_near(location, geo_point(0, 0), 10.0);
    /// ```
    pub fn geo_near(&self, point: &CqlPoint, center: &CqlPoint, radius: f64) -> bool {
        let distance = self.geo_distance(point, center);
        distance <= radius
    }

    /// Test proximity with points (internal)
    pub fn geo_near_points(&self, point: &Point, center: &Point, radius: f64) -> bool {
        point.distance_2d(center) <= radius
    }

    // =========================================================================
    // UDF: geo_near_sphere(point, center, radius_meters)
    // =========================================================================

    /// Test if a point is within a given radius (in meters) of a center point using
    /// spherical (great-circle) distance
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_near_sphere(location, geo_point(-122.4194, 37.7749), 10000);
    /// ```
    pub fn geo_near_sphere(&self, point: &CqlPoint, center: &CqlPoint, radius_meters: f64) -> bool {
        let distance = self.geo_distance_sphere(point, center);
        distance <= radius_meters
    }

    /// Test spherical proximity with points (internal)
    pub fn geo_near_sphere_points(&self, point: &Point, center: &Point, radius_meters: f64) -> bool {
        haversine_distance(point, center) <= radius_meters
    }

    // =========================================================================
    // UDF: geo_bbox(point, minX, minY, maxX, maxY)
    // =========================================================================

    /// Test if a point is within a bounding box
    ///
    /// ```sql
    /// SELECT * FROM locations
    /// WHERE geo_bbox(location, -123.0, 37.0, -122.0, 38.0);
    /// ```
    pub fn geo_bbox(
        &self,
        point: &CqlPoint,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    ) -> bool {
        point.longitude >= min_x
            && point.longitude <= max_x
            && point.latitude >= min_y
            && point.latitude <= max_y
    }

    /// Test bounding box containment with Point (internal)
    pub fn geo_bbox_point(
        &self,
        point: &Point,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    ) -> bool {
        let bbox = BoundingBox::new(min_x, min_y, max_x, max_y, None);
        bbox.contains_point(point)
    }

    // =========================================================================
    // UDF: geo_intersects(geom1, geom2)
    // =========================================================================

    /// Test if two geometries (as WKT) intersect
    ///
    /// ```sql
    /// SELECT * FROM shapes
    /// WHERE geo_intersects(geometry, 'POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))');
    /// ```
    pub fn geo_intersects(&self, wkt1: &str, wkt2: &str) -> CqlSpatialResult<bool> {
        let geom1 = self.parse_geometry_wkt(wkt1)?;
        let geom2 = self.parse_geometry_wkt(wkt2)?;

        SpatialOperations::intersects(&geom1, &geom2)
            .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))
    }

    // =========================================================================
    // UDF: geo_area(polygon)
    // =========================================================================

    /// Calculate the area of a polygon (in square coordinate units)
    ///
    /// ```sql
    /// SELECT geo_area(boundary) AS area FROM regions;
    /// ```
    pub fn geo_area(&self, polygon_wkt: &str) -> CqlSpatialResult<f64> {
        let polygon = self.parse_polygon_wkt(polygon_wkt)?;
        Ok(polygon.area())
    }

    // =========================================================================
    // UDF: geo_length(linestring)
    // =========================================================================

    /// Calculate the length of a linestring (in coordinate units)
    ///
    /// ```sql
    /// SELECT geo_length(route) AS length FROM paths;
    /// ```
    pub fn geo_length(&self, wkt: &str) -> CqlSpatialResult<f64> {
        let geometry = self.parse_geometry_wkt(wkt)?;

        self.spatial_functions
            .st_length(&geometry)
            .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))
    }

    // =========================================================================
    // UDF: geo_envelope(geometry)
    // =========================================================================

    /// Get the bounding box of a geometry as WKT
    ///
    /// ```sql
    /// SELECT geo_envelope(geometry) AS bbox FROM shapes;
    /// ```
    pub fn geo_envelope(&self, wkt: &str) -> CqlSpatialResult<String> {
        let geometry = self.parse_geometry_wkt(wkt)?;
        let bbox = self
            .spatial_functions
            .st_envelope(&geometry)
            .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))?;

        // Convert bbox to WKT POLYGON
        Ok(format!(
            "POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))",
            bbox.min_x,
            bbox.min_y,
            bbox.max_x,
            bbox.min_y,
            bbox.max_x,
            bbox.max_y,
            bbox.min_x,
            bbox.max_y,
            bbox.min_x,
            bbox.min_y
        ))
    }

    // =========================================================================
    // Batch Operations (for efficient filtering)
    // =========================================================================

    /// Filter points by distance threshold
    pub fn filter_by_distance(
        &self,
        points: &[(String, Point)],
        center: &Point,
        max_distance: f64,
        spherical: bool,
    ) -> Vec<(String, f64)> {
        points
            .iter()
            .filter_map(|(id, point)| {
                let distance = if spherical {
                    haversine_distance(center, point)
                } else {
                    center.distance_2d(point)
                };

                if distance <= max_distance {
                    Some((id.clone(), distance))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Filter points by polygon containment
    pub fn filter_by_polygon(
        &self,
        points: &[(String, Point)],
        polygon: &Polygon,
    ) -> CqlSpatialResult<Vec<String>> {
        let mut results = Vec::new();

        for (id, point) in points {
            if SpatialOperations::point_in_polygon(point, polygon)
                .map_err(|e| CqlSpatialError::SpatialError(e.to_string()))?
            {
                results.push(id.clone());
            }
        }

        Ok(results)
    }

    /// Filter points by bounding box
    pub fn filter_by_bbox(&self, points: &[(String, Point)], bbox: &BoundingBox) -> Vec<String> {
        points
            .iter()
            .filter_map(|(id, point)| {
                if bbox.contains_point(point) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the underlying spatial functions
    pub fn spatial_functions(&self) -> &SpatialFunctions {
        &self.spatial_functions
    }
}

impl Default for CqlSpatialUdfs {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating CQL spatial types
pub mod constructors {
    use super::*;

    /// Create a CqlPoint from coordinates
    pub fn point(longitude: f64, latitude: f64) -> CqlPoint {
        CqlPoint {
            longitude,
            latitude,
        }
    }

    /// Create a CqlPolygon from WKT
    pub fn polygon(wkt: &str) -> CqlPolygon {
        CqlPolygon {
            wkt: wkt.to_string(),
        }
    }

    /// Create a Point from coordinates
    pub fn spatial_point(longitude: f64, latitude: f64) -> Point {
        Point::new(longitude, latitude, Some(WGS84_SRID))
    }

    /// Create a Polygon from coordinate vectors
    pub fn spatial_polygon(
        exterior: Vec<(f64, f64)>,
        interiors: Vec<Vec<(f64, f64)>>,
    ) -> CqlSpatialResult<Polygon> {
        // Convert exterior coordinates to Points
        let mut exterior_points: Vec<Point> = exterior
            .iter()
            .map(|(lon, lat)| Point::new(*lon, *lat, Some(WGS84_SRID)))
            .collect();

        // Ensure ring is closed
        if exterior_points.len() >= 3 {
            if exterior_points.first() != exterior_points.last() {
                exterior_points.push(exterior_points[0].clone());
            }
        }

        let exterior_ring = LinearRing::new(exterior_points)
            .map_err(|e| CqlSpatialError::InvalidPolygon(e.to_string()))?;

        // Convert interior rings
        let mut interior_rings = Vec::new();
        for interior in interiors {
            let mut interior_points: Vec<Point> = interior
                .iter()
                .map(|(lon, lat)| Point::new(*lon, *lat, Some(WGS84_SRID)))
                .collect();

            if interior_points.len() >= 3 {
                if interior_points.first() != interior_points.last() {
                    interior_points.push(interior_points[0].clone());
                }
            }

            interior_rings.push(
                LinearRing::new(interior_points)
                    .map_err(|e| CqlSpatialError::InvalidPolygon(e.to_string()))?,
            );
        }

        Polygon::new(exterior_ring, interior_rings, Some(WGS84_SRID))
            .map_err(|e| CqlSpatialError::InvalidPolygon(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geo_point() {
        let udfs = CqlSpatialUdfs::new();
        let point = udfs.geo_point(-122.4194, 37.7749);
        assert_eq!(point.longitude, -122.4194);
        assert_eq!(point.latitude, 37.7749);
    }

    #[test]
    fn test_geo_distance() {
        let udfs = CqlSpatialUdfs::new();
        let p1 = udfs.geo_point(0.0, 0.0);
        let p2 = udfs.geo_point(3.0, 4.0);

        let distance = udfs.geo_distance(&p1, &p2);
        assert!((distance - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_geo_distance_sphere() {
        let udfs = CqlSpatialUdfs::new();

        // San Francisco to Los Angeles (approximately 559 km)
        let sf = udfs.geo_point(-122.4194, 37.7749);
        let la = udfs.geo_point(-118.2437, 34.0522);

        let distance = udfs.geo_distance_sphere(&sf, &la);
        // Should be approximately 559,000 meters
        assert!(distance > 500_000.0);
        assert!(distance < 600_000.0);
    }

    #[test]
    fn test_geo_within() {
        let udfs = CqlSpatialUdfs::new();

        let inside = udfs.geo_point(2.0, 2.0);
        let outside = udfs.geo_point(10.0, 10.0);
        let polygon_wkt = "POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))";

        assert!(udfs.geo_within(&inside, polygon_wkt).unwrap());
        assert!(!udfs.geo_within(&outside, polygon_wkt).unwrap());
    }

    #[test]
    fn test_geo_contains() {
        let udfs = CqlSpatialUdfs::new();

        let inside = udfs.geo_point(2.0, 2.0);
        let polygon_wkt = "POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))";

        assert!(udfs.geo_contains(polygon_wkt, &inside).unwrap());
    }

    #[test]
    fn test_geo_near() {
        let udfs = CqlSpatialUdfs::new();

        let center = udfs.geo_point(0.0, 0.0);
        let near_point = udfs.geo_point(1.0, 1.0); // ~1.41 units away
        let far_point = udfs.geo_point(10.0, 10.0); // ~14.14 units away

        assert!(udfs.geo_near(&near_point, &center, 5.0));
        assert!(!udfs.geo_near(&far_point, &center, 5.0));
    }

    #[test]
    fn test_geo_near_sphere() {
        let udfs = CqlSpatialUdfs::new();

        // Points near San Francisco
        let sf = udfs.geo_point(-122.4194, 37.7749);
        let oakland = udfs.geo_point(-122.2711, 37.8044); // ~13 km away
        let la = udfs.geo_point(-118.2437, 34.0522); // ~559 km away

        assert!(udfs.geo_near_sphere(&oakland, &sf, 20_000.0)); // Within 20km
        assert!(!udfs.geo_near_sphere(&la, &sf, 20_000.0)); // Not within 20km
    }

    #[test]
    fn test_geo_bbox() {
        let udfs = CqlSpatialUdfs::new();

        let inside = udfs.geo_point(5.0, 5.0);
        let outside = udfs.geo_point(15.0, 15.0);

        assert!(udfs.geo_bbox(&inside, 0.0, 0.0, 10.0, 10.0));
        assert!(!udfs.geo_bbox(&outside, 0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn test_geo_area() {
        let udfs = CqlSpatialUdfs::new();

        // 4x4 square = 16 square units
        let polygon_wkt = "POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))";
        let area = udfs.geo_area(polygon_wkt).unwrap();
        assert!((area - 16.0).abs() < 0.001);
    }

    #[test]
    fn test_filter_by_distance() {
        let udfs = CqlSpatialUdfs::new();

        let points = vec![
            ("a".to_string(), Point::new(1.0, 1.0, None)),
            ("b".to_string(), Point::new(5.0, 5.0, None)),
            ("c".to_string(), Point::new(10.0, 10.0, None)),
        ];
        let center = Point::new(0.0, 0.0, None);

        let results = udfs.filter_by_distance(&points, &center, 8.0, false);

        // a and b should be within distance 8
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(id, _)| id == "a"));
        assert!(results.iter().any(|(id, _)| id == "b"));
    }

    #[test]
    fn test_filter_by_bbox() {
        let udfs = CqlSpatialUdfs::new();

        let points = vec![
            ("a".to_string(), Point::new(1.0, 1.0, None)),
            ("b".to_string(), Point::new(5.0, 5.0, None)),
            ("c".to_string(), Point::new(15.0, 15.0, None)),
        ];
        let bbox = BoundingBox::new(0.0, 0.0, 10.0, 10.0, None);

        let results = udfs.filter_by_bbox(&points, &bbox);

        // a and b should be within bbox
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"a".to_string()));
        assert!(results.contains(&"b".to_string()));
    }

    #[test]
    fn test_constructors() {
        use constructors::*;

        let p = point(-122.4194, 37.7749);
        assert_eq!(p.longitude, -122.4194);

        let poly = polygon("POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))");
        assert!(!poly.wkt.is_empty());

        let sp = spatial_point(-122.4194, 37.7749);
        assert_eq!(sp.x, -122.4194);

        let spatial_poly = spatial_polygon(
            vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
            vec![],
        )
        .unwrap();
        assert!((spatial_poly.area() - 16.0).abs() < 0.001);
    }
}
