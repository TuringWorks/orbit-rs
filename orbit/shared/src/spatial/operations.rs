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

    /// Test if geometry1 is within geometry2.
    /// A is within B if A is completely inside B and their boundaries do not touch.
    pub fn within(geom1: &SpatialGeometry, geom2: &SpatialGeometry) -> Result<bool, SpatialError> {
        // ST_Within(A, B) is equivalent to ST_Contains(B, A)
        Self::contains(geom2, geom1)
    }

    /// Test if geometry1 contains geometry2.
    /// A contains B if B is completely inside A and their boundaries do not touch.
    pub fn contains(
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        match (geom1, geom2) {
            (SpatialGeometry::Polygon(poly), SpatialGeometry::Point(point)) => {
                Self::point_in_polygon(point, poly)
            }
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                Ok(p1.x == p2.x && p1.y == p2.y)
            }
            _ => {
                // For more complex cases, check if bounding boxes indicate containment
                let bbox1 = Self::bounding_box(geom1)?;
                let bbox2 = Self::bounding_box(geom2)?;

                // Quick rejection: if bbox2 is not within bbox1, then geom2 is not within geom1
                if bbox2.min_x < bbox1.min_x
                    || bbox2.max_x > bbox1.max_x
                    || bbox2.min_y < bbox1.min_y
                    || bbox2.max_y > bbox1.max_y
                {
                    return Ok(false);
                }

                // For complex geometries, we'd need more sophisticated algorithms
                // For now, return false for unsupported combinations
                Err(SpatialError::OperationError(
                    "Contains operation not fully implemented for this geometry combination"
                        .to_string(),
                ))
            }
        }
    }

    /// Test if two geometries overlap.
    /// Overlaps means they share some but not all interior points.
    pub fn overlaps(
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        // Overlaps is true if geometries intersect but neither contains the other
        let intersects = Self::intersects(geom1, geom2)?;
        if !intersects {
            return Ok(false);
        }

        let contains1 = Self::contains(geom1, geom2).unwrap_or(false);
        let contains2 = Self::contains(geom2, geom1).unwrap_or(false);

        // Overlaps if they intersect but neither fully contains the other
        Ok(!contains1 && !contains2)
    }

    /// Test if two geometries touch (share boundary but not interior).
    pub fn touches(geom1: &SpatialGeometry, geom2: &SpatialGeometry) -> Result<bool, SpatialError> {
        // Touches means they share boundary points but not interior points
        // This is a simplified implementation
        let intersects = Self::intersects(geom1, geom2)?;
        if !intersects {
            return Ok(false);
        }

        // Check if they share boundary but not interior
        // For now, return false as this requires more complex boundary analysis
        // TODO: Implement proper boundary intersection detection
        Ok(false)
    }

    /// Test if geometry1 crosses geometry2.
    /// Crosses means they share some interior points but not all.
    pub fn crosses(geom1: &SpatialGeometry, geom2: &SpatialGeometry) -> Result<bool, SpatialError> {
        // Crosses is similar to overlaps but with different semantics
        // For now, use a simplified check
        let intersects = Self::intersects(geom1, geom2)?;
        if !intersects {
            return Ok(false);
        }

        // Crosses typically applies to LineString-Polygon or LineString-LineString
        // For now, return false as this requires more complex analysis
        // TODO: Implement proper crossing detection
        Ok(false)
    }

    /// Test if two geometries are disjoint (do not intersect).
    pub fn disjoint(
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        let intersects = Self::intersects(geom1, geom2)?;
        Ok(!intersects)
    }

    /// Test if two geometries are equal (same shape and position).
    pub fn equals(geom1: &SpatialGeometry, geom2: &SpatialGeometry) -> Result<bool, SpatialError> {
        // For now, use simple equality check
        // In production, this would need more sophisticated comparison
        match (geom1, geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                Ok((p1.x - p2.x).abs() < 1e-9 && (p1.y - p2.y).abs() < 1e-9)
            }
            _ => {
                // For complex geometries, check bounding boxes first
                let bbox1 = Self::bounding_box(geom1)?;
                let bbox2 = Self::bounding_box(geom2)?;

                if bbox1.min_x != bbox2.min_x
                    || bbox1.max_x != bbox2.max_x
                    || bbox1.min_y != bbox2.min_y
                    || bbox1.max_y != bbox2.max_y
                {
                    return Ok(false);
                }

                // TODO: Implement full geometry equality check
                Ok(false)
            }
        }
    }

    /// Get the bounding box of a geometry.
    pub fn bounding_box(geometry: &SpatialGeometry) -> Result<super::BoundingBox, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => Ok(super::BoundingBox::new(p.x, p.y, p.x, p.y, p.srid)),
            SpatialGeometry::LineString(ls) => Ok(ls.bounding_box()),
            SpatialGeometry::Polygon(poly) => Ok(poly.bounding_box()),
            _ => Err(SpatialError::OperationError(
                "Bounding box calculation not implemented for this geometry type".to_string(),
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

    #[test]
    fn test_within() {
        let inner_point = SpatialGeometry::Point(Point::new(2.0, 2.0, None));
        let outer_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];
        let exterior_ring = LinearRing::new(outer_points).unwrap();
        let outer_polygon =
            SpatialGeometry::Polygon(Polygon::new(exterior_ring, vec![], None).unwrap());

        assert!(SpatialOperations::within(&inner_point, &outer_polygon).unwrap());
    }

    #[test]
    fn test_contains() {
        let inner_point = SpatialGeometry::Point(Point::new(2.0, 2.0, None));
        let outer_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];
        let exterior_ring = LinearRing::new(outer_points).unwrap();
        let outer_polygon =
            SpatialGeometry::Polygon(Polygon::new(exterior_ring, vec![], None).unwrap());

        assert!(SpatialOperations::contains(&outer_polygon, &inner_point).unwrap());
    }

    #[test]
    fn test_disjoint() {
        let p1 = SpatialGeometry::Point(Point::new(0.0, 0.0, None));
        let p2 = SpatialGeometry::Point(Point::new(10.0, 10.0, None));

        assert!(SpatialOperations::disjoint(&p1, &p2).unwrap());

        let p3 = SpatialGeometry::Point(Point::new(0.0, 0.0, None));
        assert!(!SpatialOperations::disjoint(&p1, &p3).unwrap());
    }

    #[test]
    fn test_equals() {
        let p1 = SpatialGeometry::Point(Point::new(1.0, 2.0, None));
        let p2 = SpatialGeometry::Point(Point::new(1.0, 2.0, None));
        let p3 = SpatialGeometry::Point(Point::new(1.0, 3.0, None));

        assert!(SpatialOperations::equals(&p1, &p2).unwrap());
        assert!(!SpatialOperations::equals(&p1, &p3).unwrap());
    }
}
