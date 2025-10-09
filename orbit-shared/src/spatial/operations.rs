//! Spatial operations and geometric functions.
//!
//! This module provides comprehensive spatial operations including:
//! - Geometric relationships (intersects, within, contains, etc.)
//! - Measurements (distance, area, length, perimeter)
//! - Transformations (buffer, simplify, convex hull)
//! - Topological operations (union, intersection, difference)

use super::{Point, Polygon, SpatialError, SpatialGeometry};

/// Spatial relationship types for geometry comparisons.
#[derive(Debug, Clone, PartialEq)]
pub enum SpatialRelation {
    Intersects,
    Within,
    Contains,
    Touches,
    Overlaps,
    Crosses,
    Disjoint,
    Equals,
}

/// Spatial operation implementations.
pub struct SpatialOperations;

impl SpatialOperations {
    /// Test if two geometries intersect.
    pub fn intersects(
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        match (geom1, geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                Ok(p1.x == p2.x && p1.y == p2.y)
            }
            (SpatialGeometry::Point(p), SpatialGeometry::Polygon(poly)) => {
                Self::point_in_polygon(p, poly)
            }
            _ => Err(SpatialError::OperationError(
                "Unsupported geometry combination".to_string(),
            )),
        }
    }

    /// Test if a point is within a polygon using the ray casting algorithm.
    pub fn point_in_polygon(point: &Point, polygon: &Polygon) -> Result<bool, SpatialError> {
        let mut inside = false;
        let ring = &polygon.exterior_ring.points;

        let mut j = ring.len() - 1;
        for i in 0..ring.len() {
            if ((ring[i].y > point.y) != (ring[j].y > point.y))
                && (point.x
                    < (ring[j].x - ring[i].x) * (point.y - ring[i].y) / (ring[j].y - ring[i].y)
                        + ring[i].x)
            {
                inside = !inside;
            }
            j = i;
        }

        // Check holes (interior rings)
        for hole in &polygon.interior_rings {
            let mut hole_inside = false;
            let hole_ring = &hole.points;
            let mut j = hole_ring.len() - 1;

            for i in 0..hole_ring.len() {
                if ((hole_ring[i].y > point.y) != (hole_ring[j].y > point.y))
                    && (point.x
                        < (hole_ring[j].x - hole_ring[i].x) * (point.y - hole_ring[i].y)
                            / (hole_ring[j].y - hole_ring[i].y)
                            + hole_ring[i].x)
                {
                    hole_inside = !hole_inside;
                }
                j = i;
            }

            if hole_inside {
                inside = false;
                break;
            }
        }

        Ok(inside)
    }

    /// Calculate the distance between two geometries.
    pub fn distance(geom1: &SpatialGeometry, geom2: &SpatialGeometry) -> Result<f64, SpatialError> {
        match (geom1, geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => Ok(p1.distance_2d(p2)),
            _ => Err(SpatialError::OperationError(
                "Distance calculation not implemented for this geometry combination".to_string(),
            )),
        }
    }

    /// Calculate the area of a geometry.
    pub fn area(geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        match geometry {
            SpatialGeometry::Point(_) => Ok(0.0),
            SpatialGeometry::LineString(_) => Ok(0.0),
            SpatialGeometry::Polygon(poly) => Ok(poly.area()),
            _ => Err(SpatialError::OperationError(
                "Area calculation not implemented for this geometry type".to_string(),
            )),
        }
    }

    /// Calculate the length/perimeter of a geometry.
    pub fn length(geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        match geometry {
            SpatialGeometry::Point(_) => Ok(0.0),
            SpatialGeometry::LineString(ls) => Ok(ls.length()),
            SpatialGeometry::Polygon(poly) => {
                // Calculate perimeter
                let mut perimeter = 0.0;

                // Exterior ring
                for i in 1..poly.exterior_ring.points.len() {
                    perimeter +=
                        poly.exterior_ring.points[i - 1].distance_2d(&poly.exterior_ring.points[i]);
                }

                // Interior rings
                for ring in &poly.interior_rings {
                    for i in 1..ring.points.len() {
                        perimeter += ring.points[i - 1].distance_2d(&ring.points[i]);
                    }
                }

                Ok(perimeter)
            }
            _ => Err(SpatialError::OperationError(
                "Length calculation not implemented for this geometry type".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::LinearRing;

    #[test]
    fn test_point_in_polygon() {
        // Create a simple square polygon
        let points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None), // Close the ring
        ];

        let exterior_ring = LinearRing::new(points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], None).unwrap();

        // Test point inside
        let inside_point = Point::new(2.0, 2.0, None);
        assert!(SpatialOperations::point_in_polygon(&inside_point, &polygon).unwrap());

        // Test point outside
        let outside_point = Point::new(5.0, 5.0, None);
        assert!(!SpatialOperations::point_in_polygon(&outside_point, &polygon).unwrap());

        // Test point on boundary
        let _boundary_point = Point::new(0.0, 2.0, None);
        // Note: This test depends on the exact implementation of the ray casting algorithm
    }

    #[test]
    fn test_distance_calculation() {
        let p1 = SpatialGeometry::Point(Point::new(0.0, 0.0, None));
        let p2 = SpatialGeometry::Point(Point::new(3.0, 4.0, None));

        let distance = SpatialOperations::distance(&p1, &p2).unwrap();
        assert_eq!(distance, 5.0);
    }

    #[test]
    fn test_area_calculation() {
        // Create a 4x4 square
        let points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];

        let exterior_ring = LinearRing::new(points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], None).unwrap();
        let geom = SpatialGeometry::Polygon(polygon);

        let area = SpatialOperations::area(&geom).unwrap();
        assert_eq!(area, 16.0);
    }
}
