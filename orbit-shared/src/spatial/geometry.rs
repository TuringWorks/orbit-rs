//! Core spatial geometry types following OGC Simple Features specification.
//!
//! This module provides the fundamental building blocks for spatial data in Orbit-RS,
//! including points, lines, polygons, and their multi-geometry counterparts.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Universal spatial geometry type supporting all OGC Simple Features.
///
/// This enum represents any kind of spatial geometry and provides a unified
/// interface for spatial operations across different geometry types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpatialGeometry {
    Point(Point),
    LineString(LineString),
    Polygon(Polygon),
    MultiPoint(MultiPoint),
    MultiLineString(MultiLineString),
    MultiPolygon(MultiPolygon),
    GeometryCollection(GeometryCollection),
}

/// A spatial point with optional elevation and measure values.
///
/// Points support 2D, 3D (with elevation), and measured coordinates,
/// along with a spatial reference system identifier (SRID).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// X coordinate (longitude in geographic systems)
    pub x: f64,
    /// Y coordinate (latitude in geographic systems)  
    pub y: f64,
    /// Optional Z coordinate (elevation/height)
    pub z: Option<f64>,
    /// Optional M coordinate (measure value)
    pub m: Option<f64>,
    /// Spatial Reference System Identifier (EPSG code)
    pub srid: Option<i32>,
}

/// A linear geometry composed of two or more points.
///
/// LineStrings represent paths, roads, boundaries, and other linear features.
/// They must contain at least 2 points to be valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    /// Ordered sequence of points forming the line
    pub points: Vec<Point>,
    /// Spatial Reference System Identifier
    pub srid: Option<i32>,
}

/// A polygon geometry with exterior boundary and optional holes.
///
/// Polygons represent areas like countries, lakes, buildings, or zones.
/// They consist of an exterior ring and zero or more interior rings (holes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    /// Exterior boundary ring (must be closed)
    pub exterior_ring: LinearRing,
    /// Interior rings representing holes (each must be closed)
    pub interior_rings: Vec<LinearRing>,
    /// Spatial Reference System Identifier
    pub srid: Option<i32>,
}

/// A closed linear ring used in polygon construction.
///
/// Linear rings are special LineStrings where the first and last points
/// must be identical, creating a closed shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearRing {
    /// Points forming the ring (first and last must be identical)
    pub points: Vec<Point>,
}

/// A collection of multiple Point geometries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiPoint {
    pub points: Vec<Point>,
    pub srid: Option<i32>,
}

/// A collection of multiple LineString geometries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiLineString {
    pub linestrings: Vec<LineString>,
    pub srid: Option<i32>,
}

/// A collection of multiple Polygon geometries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiPolygon {
    pub polygons: Vec<Polygon>,
    pub srid: Option<i32>,
}

/// A heterogeneous collection of any geometry types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryCollection {
    pub geometries: Vec<SpatialGeometry>,
    pub srid: Option<i32>,
}

/// A bounding box representing the spatial extent of geometries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub min_z: Option<f64>,
    pub max_z: Option<f64>,
    pub srid: Option<i32>,
}

/// Spatial error types for geometry operations.
#[derive(Debug, thiserror::Error)]
pub enum SpatialError {
    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("Coordinate reference system error: {0}")]
    CRSError(String),

    #[error("Spatial index error: {0}")]
    IndexError(String),

    #[error("Spatial operation error: {0}")]
    OperationError(String),

    #[error("GPU acceleration error: {0}")]
    GPUError(String),
}

impl Point {
    /// Create a new 2D point.
    pub fn new(x: f64, y: f64, srid: Option<i32>) -> Self {
        Self {
            x,
            y,
            z: None,
            m: None,
            srid,
        }
    }

    /// Create a new 3D point with elevation.
    pub fn with_elevation(x: f64, y: f64, z: f64, srid: Option<i32>) -> Self {
        Self {
            x,
            y,
            z: Some(z),
            m: None,
            srid,
        }
    }

    /// Create a new measured point.
    pub fn with_measure(x: f64, y: f64, m: f64, srid: Option<i32>) -> Self {
        Self {
            x,
            y,
            z: None,
            m: Some(m),
            srid,
        }
    }

    /// Create a new 3D measured point.
    pub fn with_elevation_and_measure(x: f64, y: f64, z: f64, m: f64, srid: Option<i32>) -> Self {
        Self {
            x,
            y,
            z: Some(z),
            m: Some(m),
            srid,
        }
    }

    /// Calculate the 2D distance to another point.
    pub fn distance_2d(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate the 3D distance to another point (if both have elevation).
    pub fn distance_3d(&self, other: &Point) -> Option<f64> {
        match (self.z, other.z) {
            (Some(z1), Some(z2)) => {
                let dx = self.x - other.x;
                let dy = self.y - other.y;
                let dz = z1 - z2;
                Some((dx * dx + dy * dy + dz * dz).sqrt())
            }
            _ => None,
        }
    }

    /// Check if this point is 3D (has elevation).
    pub fn is_3d(&self) -> bool {
        self.z.is_some()
    }

    /// Check if this point is measured.
    pub fn is_measured(&self) -> bool {
        self.m.is_some()
    }
}

impl LineString {
    /// Create a new LineString from a vector of points.
    pub fn new(points: Vec<Point>, srid: Option<i32>) -> Result<Self, SpatialError> {
        if points.len() < 2 {
            return Err(SpatialError::InvalidGeometry(
                "LineString must contain at least 2 points".to_string(),
            ));
        }

        Ok(Self { points, srid })
    }

    /// Calculate the total length of the LineString.
    pub fn length(&self) -> f64 {
        let mut total_length = 0.0;
        for i in 1..self.points.len() {
            total_length += self.points[i - 1].distance_2d(&self.points[i]);
        }
        total_length
    }

    /// Get the first point of the LineString.
    pub fn start_point(&self) -> Option<&Point> {
        self.points.first()
    }

    /// Get the last point of the LineString.
    pub fn end_point(&self) -> Option<&Point> {
        self.points.last()
    }

    /// Check if the LineString is closed (first and last points are the same).
    pub fn is_closed(&self) -> bool {
        if self.points.len() < 3 {
            return false;
        }
        self.points.first() == self.points.last()
    }

    /// Calculate the bounding box of the LineString.
    pub fn bounding_box(&self) -> BoundingBox {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut min_z = None;
        let mut max_z = None;

        for point in &self.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);

            if let Some(z) = point.z {
                min_z = Some(min_z.map_or(z, |min: f64| min.min(z)));
                max_z = Some(max_z.map_or(z, |max: f64| max.max(z)));
            }
        }

        BoundingBox {
            min_x,
            min_y,
            max_x,
            max_y,
            min_z,
            max_z,
            srid: self.srid,
        }
    }
}

impl LinearRing {
    /// Create a new LinearRing from a vector of points.
    pub fn new(mut points: Vec<Point>) -> Result<Self, SpatialError> {
        if points.len() < 4 {
            return Err(SpatialError::InvalidGeometry(
                "LinearRing must contain at least 4 points".to_string(),
            ));
        }

        // Ensure the ring is closed
        if points.first() != points.last() {
            points.push(points[0].clone());
        }

        Ok(Self { points })
    }

    /// Check if the ring is valid (closed and has at least 4 points).
    pub fn is_valid(&self) -> bool {
        self.points.len() >= 4 && self.points.first() == self.points.last()
    }

    /// Calculate the area enclosed by this ring using the shoelace formula.
    pub fn area(&self) -> f64 {
        if self.points.len() < 4 {
            return 0.0;
        }

        let mut area = 0.0;
        for i in 0..self.points.len() - 1 {
            area += (self.points[i].x * self.points[i + 1].y)
                - (self.points[i + 1].x * self.points[i].y);
        }
        area.abs() / 2.0
    }
}

impl Polygon {
    /// Create a new Polygon with an exterior ring and optional interior rings.
    pub fn new(
        exterior_ring: LinearRing,
        interior_rings: Vec<LinearRing>,
        srid: Option<i32>,
    ) -> Result<Self, SpatialError> {
        if !exterior_ring.is_valid() {
            return Err(SpatialError::InvalidGeometry(
                "Exterior ring is not valid".to_string(),
            ));
        }

        for ring in &interior_rings {
            if !ring.is_valid() {
                return Err(SpatialError::InvalidGeometry(
                    "One or more interior rings are not valid".to_string(),
                ));
            }
        }

        Ok(Self {
            exterior_ring,
            interior_rings,
            srid,
        })
    }

    /// Create a simple polygon from a vector of points (no holes).
    pub fn from_points(points: Vec<Point>, srid: Option<i32>) -> Result<Self, SpatialError> {
        let exterior_ring = LinearRing::new(points)?;
        Self::new(exterior_ring, vec![], srid)
    }

    /// Calculate the area of the polygon (exterior area minus interior holes).
    pub fn area(&self) -> f64 {
        let exterior_area = self.exterior_ring.area();
        let interior_area: f64 = self.interior_rings.iter().map(|ring| ring.area()).sum();
        exterior_area - interior_area
    }

    /// Calculate the bounding box of the polygon.
    pub fn bounding_box(&self) -> BoundingBox {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut min_z = None;
        let mut max_z = None;

        // Process exterior ring
        for point in &self.exterior_ring.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);

            if let Some(z) = point.z {
                min_z = Some(min_z.map_or(z, |min: f64| min.min(z)));
                max_z = Some(max_z.map_or(z, |max: f64| max.max(z)));
            }
        }

        // Process interior rings
        for ring in &self.interior_rings {
            for point in &ring.points {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);

                if let Some(z) = point.z {
                    min_z = Some(min_z.map_or(z, |min: f64| min.min(z)));
                    max_z = Some(max_z.map_or(z, |max: f64| max.max(z)));
                }
            }
        }

        BoundingBox {
            min_x,
            min_y,
            max_x,
            max_y,
            min_z,
            max_z,
            srid: self.srid,
        }
    }
}

impl SpatialGeometry {
    /// Get the SRID of the geometry.
    pub fn srid(&self) -> Option<i32> {
        match self {
            SpatialGeometry::Point(p) => p.srid,
            SpatialGeometry::LineString(l) => l.srid,
            SpatialGeometry::Polygon(p) => p.srid,
            SpatialGeometry::MultiPoint(mp) => mp.srid,
            SpatialGeometry::MultiLineString(ml) => ml.srid,
            SpatialGeometry::MultiPolygon(mp) => mp.srid,
            SpatialGeometry::GeometryCollection(gc) => gc.srid,
        }
    }

    /// Set the SRID of the geometry.
    pub fn set_srid(&mut self, srid: Option<i32>) {
        match self {
            SpatialGeometry::Point(p) => p.srid = srid,
            SpatialGeometry::LineString(l) => l.srid = srid,
            SpatialGeometry::Polygon(p) => p.srid = srid,
            SpatialGeometry::MultiPoint(mp) => mp.srid = srid,
            SpatialGeometry::MultiLineString(ml) => ml.srid = srid,
            SpatialGeometry::MultiPolygon(mp) => mp.srid = srid,
            SpatialGeometry::GeometryCollection(gc) => gc.srid = srid,
        }
    }

    /// Get the geometry type as a string.
    pub fn geometry_type(&self) -> &'static str {
        match self {
            SpatialGeometry::Point(_) => "POINT",
            SpatialGeometry::LineString(_) => "LINESTRING",
            SpatialGeometry::Polygon(_) => "POLYGON",
            SpatialGeometry::MultiPoint(_) => "MULTIPOINT",
            SpatialGeometry::MultiLineString(_) => "MULTILINESTRING",
            SpatialGeometry::MultiPolygon(_) => "MULTIPOLYGON",
            SpatialGeometry::GeometryCollection(_) => "GEOMETRYCOLLECTION",
        }
    }

    /// Check if the geometry is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            SpatialGeometry::Point(_) => false, // Points are never empty
            SpatialGeometry::LineString(l) => l.points.is_empty(),
            SpatialGeometry::Polygon(p) => p.exterior_ring.points.is_empty(),
            SpatialGeometry::MultiPoint(mp) => mp.points.is_empty(),
            SpatialGeometry::MultiLineString(ml) => ml.linestrings.is_empty(),
            SpatialGeometry::MultiPolygon(mp) => mp.polygons.is_empty(),
            SpatialGeometry::GeometryCollection(gc) => gc.geometries.is_empty(),
        }
    }
}

impl BoundingBox {
    /// Create a new bounding box.
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64, srid: Option<i32>) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
            min_z: None,
            max_z: None,
            srid,
        }
    }

    /// Create a 3D bounding box.
    pub fn new_3d(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
        srid: Option<i32>,
    ) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
            min_z: Some(min_z),
            max_z: Some(max_z),
            srid,
        }
    }

    /// Check if this bounding box intersects with another.
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        !(self.max_x < other.min_x
            || self.min_x > other.max_x
            || self.max_y < other.min_y
            || self.min_y > other.max_y)
    }

    /// Check if this bounding box contains a point.
    pub fn contains_point(&self, point: &Point) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
    }

    /// Calculate the area of the bounding box.
    pub fn area(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y)
    }

    /// Calculate the center point of the bounding box.
    pub fn center(&self) -> Point {
        Point::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            self.srid,
        )
    }
}

impl fmt::Display for SpatialGeometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.geometry_type())
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.z, self.m) {
            (Some(z), Some(m)) => write!(f, "POINT ZM ({} {} {} {})", self.x, self.y, z, m),
            (Some(z), None) => write!(f, "POINT Z ({} {} {})", self.x, self.y, z),
            (None, Some(m)) => write!(f, "POINT M ({} {} {})", self.x, self.y, m),
            (None, None) => write!(f, "POINT ({} {})", self.x, self.y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let point = Point::new(-122.4194, 37.7749, Some(4326));
        assert_eq!(point.x, -122.4194);
        assert_eq!(point.y, 37.7749);
        assert_eq!(point.srid, Some(4326));
        assert!(!point.is_3d());
        assert!(!point.is_measured());
    }

    #[test]
    fn test_point_3d() {
        let point = Point::with_elevation(-122.4194, 37.7749, 100.0, Some(4326));
        assert!(point.is_3d());
        assert_eq!(point.z, Some(100.0));
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0.0, 0.0, None);
        let p2 = Point::new(3.0, 4.0, None);
        assert_eq!(p1.distance_2d(&p2), 5.0);
    }

    #[test]
    fn test_linestring_creation() {
        let points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(1.0, 1.0, None),
            Point::new(2.0, 0.0, None),
        ];

        let linestring = LineString::new(points, Some(4326)).unwrap();
        assert_eq!(linestring.points.len(), 3);
        assert_eq!(linestring.srid, Some(4326));
    }

    #[test]
    fn test_linestring_length() {
        let points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(0.0, 1.0, None),
            Point::new(1.0, 1.0, None),
        ];

        let linestring = LineString::new(points, None).unwrap();
        assert_eq!(linestring.length(), 2.0);
    }

    #[test]
    fn test_polygon_creation() {
        let exterior_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None), // Close the ring
        ];

        let exterior_ring = LinearRing::new(exterior_points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], Some(4326)).unwrap();

        assert_eq!(polygon.area(), 16.0);
        assert_eq!(polygon.srid, Some(4326));
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(2.0, 3.0, None),
            Point::new(-1.0, 1.0, None),
        ];

        let linestring = LineString::new(points, None).unwrap();
        let bbox = linestring.bounding_box();

        assert_eq!(bbox.min_x, -1.0);
        assert_eq!(bbox.min_y, 0.0);
        assert_eq!(bbox.max_x, 2.0);
        assert_eq!(bbox.max_y, 3.0);
    }
}
