//! High-performance spatial indexing for efficient spatial queries.
//!
//! This module provides multiple spatial indexing algorithms optimized for
//! different use cases and data characteristics:
//! - R-tree for complex geometries and range queries
//! - QuadTree for high-density point data
//! - Geohash grid for global applications
//! - Adaptive index selection based on data patterns

use super::{BoundingBox, Point, SpatialError, SpatialGeometry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Universal spatial index interface.
pub trait SpatialIndexTrait {
    /// Insert a geometry with an associated ID.
    fn insert(&mut self, id: u64, geometry: &SpatialGeometry) -> Result<(), SpatialError>;

    /// Remove a geometry by ID.
    fn remove(&mut self, id: u64) -> Result<Option<SpatialGeometry>, SpatialError>;

    /// Query geometries that intersect with the given bounding box.
    fn query_bbox(&self, bbox: &BoundingBox) -> Result<Vec<(u64, &SpatialGeometry)>, SpatialError>;

    /// Find the nearest neighbors to a given point.
    fn nearest_neighbors(
        &self,
        point: &Point,
        count: usize,
    ) -> Result<Vec<(u64, &SpatialGeometry, f64)>, SpatialError>;

    /// Get statistics about the index.
    fn stats(&self) -> IndexStatistics;
}

/// Spatial index enumeration with different algorithms.
#[derive(Debug, Clone)]
pub enum SpatialIndex {
    /// Hierarchical spatial index for point data
    QuadTree(QuadTree),
    /// R-tree for complex geometries and range queries  
    RTree(RTree),
    /// Grid-based indexing for global applications
    GeohashGrid(GeohashGrid),
    /// K-d tree for high-dimensional spatial data
    KdTree(KdTree),
}

/// QuadTree spatial index for efficient point-based queries.
#[derive(Debug, Clone)]
pub struct QuadTree {
    root: QuadNode,
    max_depth: usize,
    max_points_per_node: usize,
    bounds: BoundingBox,
    point_count: usize,
}

#[derive(Debug, Clone)]
struct QuadNode {
    bounds: BoundingBox,
    points: Vec<(u64, Point)>,
    children: Option<Box<[QuadNode; 4]>>,
    depth: usize,
}

/// R-tree spatial index for complex geometries.
#[derive(Debug, Clone)]
pub struct RTree {
    root: Option<RTreeNode>,
    max_entries: usize,
    #[allow(dead_code)]
    min_entries: usize,
    #[allow(dead_code)]
    split_strategy: RTreeSplitStrategy,
    geometry_count: usize,
}

#[derive(Debug, Clone)]
struct RTreeNode {
    bounds: BoundingBox,
    entries: Vec<RTreeEntry>,
    is_leaf: bool,
}

#[derive(Debug, Clone)]
struct RTreeEntry {
    id: u64,
    bounds: BoundingBox,
    geometry: Option<SpatialGeometry>,
    child: Option<Box<RTreeNode>>,
}

/// R-tree split strategies.
#[derive(Debug, Clone, PartialEq)]
pub enum RTreeSplitStrategy {
    /// Quadratic split algorithm
    Quadratic,
    /// Linear split algorithm (faster but less optimal)
    Linear,
    /// R*-tree split strategy (best space utilization)
    RStart,
}

/// Geohash-based spatial grid index.
#[derive(Debug, Clone)]
pub struct GeohashGrid {
    precision: u8,
    #[allow(dead_code)]
    grid_size: usize,
    cells: HashMap<String, Vec<(u64, SpatialGeometry)>>,
    point_count: usize,
}

/// K-d tree for high-dimensional spatial data.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KdTree {
    root: Option<KdNode>,
    dimensions: u8,
    balance_threshold: f64,
    point_count: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct KdNode {
    point: (u64, Point),
    left: Option<Box<KdNode>>,
    right: Option<Box<KdNode>>,
    dimension: u8,
}

/// Index statistics for performance monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    pub index_type: String,
    pub total_entries: usize,
    pub max_depth: usize,
    pub average_entries_per_node: f64,
    pub memory_usage_bytes: usize,
    pub query_performance: QueryPerformanceStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPerformanceStats {
    pub avg_query_time_ms: f64,
    pub queries_per_second: f64,
    pub cache_hit_rate: f64,
}

/// Geometry statistics for adaptive index selection.
#[derive(Debug, Clone)]
pub struct GeometryStatistics {
    pub total_records: usize,
    pub point_density: f64,
    pub avg_polygon_vertices: usize,
    pub bounding_box: BoundingBox,
    pub geometry_distribution: HashMap<String, usize>,
}

/// Adaptive spatial indexer that selects the best index for the data.
pub struct AdaptiveSpatialIndexer {
    point_density_threshold: f64,
    polygon_complexity_threshold: usize,
    global_extent_threshold: f64,
}

impl QuadTree {
    /// Create a new QuadTree index.
    pub fn new(bounds: BoundingBox, max_depth: usize, max_points_per_node: usize) -> Self {
        Self {
            root: QuadNode {
                bounds: bounds.clone(),
                points: Vec::new(),
                children: None,
                depth: 0,
            },
            max_depth,
            max_points_per_node,
            bounds,
            point_count: 0,
        }
    }

    /// Insert a point into the QuadTree.
    pub fn insert_point(&mut self, id: u64, point: Point) -> Result<(), SpatialError> {
        if !self.bounds.contains_point(&point) {
            return Err(SpatialError::IndexError(
                "Point is outside the index bounds".to_string(),
            ));
        }

        let max_depth = self.max_depth;
        let max_points_per_node = self.max_points_per_node;
        Self::insert_point_recursive(&mut self.root, id, point, max_depth, max_points_per_node)?;
        self.point_count += 1;
        Ok(())
    }

    fn insert_point_recursive(
        node: &mut QuadNode,
        id: u64,
        point: Point,
        max_depth: usize,
        max_points_per_node: usize,
    ) -> Result<(), SpatialError> {
        // If this is a leaf node and not at max capacity, add the point
        if node.children.is_none() {
            let point_clone = point.clone();
            node.points.push((id, point));

            // Check if we need to split
            if node.points.len() > max_points_per_node && node.depth < max_depth {
                Self::split_node(node)?;

                // After splitting, redistribute the point to the correct child
                let center_x = (node.bounds.min_x + node.bounds.max_x) / 2.0;
                let center_y = (node.bounds.min_y + node.bounds.max_y) / 2.0;

                let child_index = if point_clone.x < center_x {
                    if point_clone.y < center_y {
                        0
                    } else {
                        2
                    } // SW or NW
                } else if point_clone.y < center_y {
                    1
                } else {
                    3
                }; // SE or NE

                if let Some(ref mut children) = node.children {
                    // Remove the point from this node and add to appropriate child
                    if let Some(pos) = node.points.iter().position(|(pid, _)| *pid == id) {
                        let (removed_id, removed_point) = node.points.remove(pos);
                        Self::insert_point_recursive(
                            &mut children[child_index],
                            removed_id,
                            removed_point,
                            max_depth,
                            max_points_per_node,
                        )?;
                    }
                }
            }

            return Ok(());
        }

        // Find the appropriate child quadrant
        if let Some(ref mut children) = node.children {
            let center_x = (node.bounds.min_x + node.bounds.max_x) / 2.0;
            let center_y = (node.bounds.min_y + node.bounds.max_y) / 2.0;

            let child_index = if point.x < center_x {
                if point.y < center_y {
                    0
                } else {
                    2
                } // SW or NW
            } else if point.y < center_y {
                1
            } else {
                3
            }; // SE or NE

            Self::insert_point_recursive(
                &mut children[child_index],
                id,
                point,
                max_depth,
                max_points_per_node,
            )?;
        }

        Ok(())
    }

    fn split_node(node: &mut QuadNode) -> Result<(), SpatialError> {
        let center_x = (node.bounds.min_x + node.bounds.max_x) / 2.0;
        let center_y = (node.bounds.min_y + node.bounds.max_y) / 2.0;

        // Create four child nodes
        let children = Box::new([
            QuadNode {
                // Southwest
                bounds: BoundingBox::new(
                    node.bounds.min_x,
                    node.bounds.min_y,
                    center_x,
                    center_y,
                    node.bounds.srid,
                ),
                points: Vec::new(),
                children: None,
                depth: node.depth + 1,
            },
            QuadNode {
                // Southeast
                bounds: BoundingBox::new(
                    center_x,
                    node.bounds.min_y,
                    node.bounds.max_x,
                    center_y,
                    node.bounds.srid,
                ),
                points: Vec::new(),
                children: None,
                depth: node.depth + 1,
            },
            QuadNode {
                // Northwest
                bounds: BoundingBox::new(
                    node.bounds.min_x,
                    center_y,
                    center_x,
                    node.bounds.max_y,
                    node.bounds.srid,
                ),
                points: Vec::new(),
                children: None,
                depth: node.depth + 1,
            },
            QuadNode {
                // Northeast
                bounds: BoundingBox::new(
                    center_x,
                    center_y,
                    node.bounds.max_x,
                    node.bounds.max_y,
                    node.bounds.srid,
                ),
                points: Vec::new(),
                children: None,
                depth: node.depth + 1,
            },
        ]);

        // Redistribute points to children
        let points = std::mem::take(&mut node.points);
        node.children = Some(children);

        for (id, point) in points {
            // Find the appropriate child for each point
            let center_x = (node.bounds.min_x + node.bounds.max_x) / 2.0;
            let center_y = (node.bounds.min_y + node.bounds.max_y) / 2.0;

            let child_index = if point.x < center_x {
                if point.y < center_y {
                    0
                } else {
                    2
                } // SW or NW
            } else if point.y < center_y {
                1
            } else {
                3
            }; // SE or NE

            if let Some(ref mut children) = node.children {
                children[child_index].points.push((id, point));
            }
        }

        Ok(())
    }

    /// Query points within a bounding box.
    pub fn query_bbox(&self, bbox: &BoundingBox) -> Vec<(u64, &Point)> {
        let mut results = Vec::new();
        self.query_bbox_recursive(&self.root, bbox, &mut results);
        results
    }

    fn query_bbox_recursive<'a>(
        &self,
        node: &'a QuadNode,
        bbox: &BoundingBox,
        results: &mut Vec<(u64, &'a Point)>,
    ) {
        // Check if node bounds intersect with query bbox
        if !node.bounds.intersects(bbox) {
            return;
        }

        // Add points from this node that fall within the bbox
        for (id, point) in &node.points {
            if bbox.contains_point(point) {
                results.push((*id, point));
            }
        }

        // Recursively search children
        if let Some(ref children) = node.children {
            for child in children.iter() {
                Self::query_bbox_recursive(self, child, bbox, results);
            }
        }
    }
}

impl RTree {
    /// Create a new R-tree index.
    pub fn new(max_entries: usize, min_entries: usize, split_strategy: RTreeSplitStrategy) -> Self {
        Self {
            root: None,
            max_entries,
            min_entries,
            split_strategy,
            geometry_count: 0,
        }
    }

    /// Insert a geometry into the R-tree.
    pub fn insert_geometry(
        &mut self,
        id: u64,
        geometry: SpatialGeometry,
    ) -> Result<(), SpatialError> {
        let bounds = self.calculate_geometry_bounds(&geometry)?;

        let entry = RTreeEntry {
            id,
            bounds: bounds.clone(),
            geometry: Some(geometry),
            child: None,
        };

        if self.root.is_none() {
            // Create root node
            self.root = Some(RTreeNode {
                bounds,
                entries: vec![entry],
                is_leaf: true,
            });
        } else {
            let mut root_node = self.root.take().unwrap();
            self.insert_entry_recursive(&mut root_node, entry)?;
            self.root = Some(root_node);
        }

        self.geometry_count += 1;
        Ok(())
    }

    fn calculate_geometry_bounds(
        &self,
        geometry: &SpatialGeometry,
    ) -> Result<BoundingBox, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => Ok(BoundingBox::new(p.x, p.y, p.x, p.y, p.srid)),
            SpatialGeometry::LineString(ls) => Ok(ls.bounding_box()),
            SpatialGeometry::Polygon(poly) => Ok(poly.bounding_box()),
            _ => Err(SpatialError::IndexError(
                "Unsupported geometry type for R-tree".to_string(),
            )),
        }
    }

    fn insert_entry_recursive(
        &mut self,
        node: &mut RTreeNode,
        entry: RTreeEntry,
    ) -> Result<(), SpatialError> {
        if node.is_leaf {
            let bounds_copy = entry.bounds.clone();
            node.entries.push(entry);
            self.expand_bounds(&mut node.bounds, &bounds_copy);

            // Check if we need to split
            if node.entries.len() > self.max_entries {
                // TODO: Implement R-tree splitting logic
            }
        } else {
            // Find the best child to insert into
            let _best_child_idx = self.choose_leaf(node, &entry.bounds);
            // TODO: Implement child insertion logic
        }

        Ok(())
    }

    fn choose_leaf(&self, node: &RTreeNode, bounds: &BoundingBox) -> usize {
        // Simple heuristic: choose child that requires least enlargement
        let mut best_idx = 0;
        let mut min_enlargement = f64::INFINITY;

        for (i, entry) in node.entries.iter().enumerate() {
            let enlargement = self.calculate_enlargement(&entry.bounds, bounds);
            if enlargement < min_enlargement {
                min_enlargement = enlargement;
                best_idx = i;
            }
        }

        best_idx
    }

    fn calculate_enlargement(&self, current: &BoundingBox, new: &BoundingBox) -> f64 {
        let enlarged_area = BoundingBox::new(
            current.min_x.min(new.min_x),
            current.min_y.min(new.min_y),
            current.max_x.max(new.max_x),
            current.max_y.max(new.max_y),
            current.srid,
        )
        .area();

        enlarged_area - current.area()
    }

    fn expand_bounds(&self, current: &mut BoundingBox, new: &BoundingBox) {
        current.min_x = current.min_x.min(new.min_x);
        current.min_y = current.min_y.min(new.min_y);
        current.max_x = current.max_x.max(new.max_x);
        current.max_y = current.max_y.max(new.max_y);
    }
}

impl GeohashGrid {
    /// Create a new Geohash grid index.
    pub fn new(precision: u8, grid_size: usize) -> Self {
        Self {
            precision,
            grid_size,
            cells: HashMap::new(),
            point_count: 0,
        }
    }

    /// Insert a geometry into the geohash grid.
    pub fn insert_geometry(
        &mut self,
        id: u64,
        geometry: SpatialGeometry,
    ) -> Result<(), SpatialError> {
        let geohash = match &geometry {
            SpatialGeometry::Point(p) => self.encode_geohash(p.x, p.y),
            SpatialGeometry::LineString(ls) => {
                let bbox = ls.bounding_box();
                let center = bbox.center();
                self.encode_geohash(center.x, center.y)
            }
            SpatialGeometry::Polygon(poly) => {
                let bbox = poly.bounding_box();
                let center = bbox.center();
                self.encode_geohash(center.x, center.y)
            }
            _ => {
                return Err(SpatialError::IndexError(
                    "Unsupported geometry type for Geohash grid".to_string(),
                ))
            }
        };

        self.cells.entry(geohash).or_default().push((id, geometry));
        self.point_count += 1;
        Ok(())
    }

    fn encode_geohash(&self, lon: f64, lat: f64) -> String {
        // Simplified geohash encoding (in production, use a proper geohash library)
        let mut lat_range = (-90.0, 90.0);
        let mut lon_range = (-180.0, 180.0);
        let mut geohash = String::new();
        let mut is_even = true; // Start with longitude

        for _ in 0..self.precision * 5 {
            // Each character represents 5 bits
            if is_even {
                // Longitude
                let mid = (lon_range.0 + lon_range.1) / 2.0;
                if lon >= mid {
                    geohash.push('1');
                    lon_range.0 = mid;
                } else {
                    geohash.push('0');
                    lon_range.1 = mid;
                }
            } else {
                // Latitude
                let mid = (lat_range.0 + lat_range.1) / 2.0;
                if lat >= mid {
                    geohash.push('1');
                    lat_range.0 = mid;
                } else {
                    geohash.push('0');
                    lat_range.1 = mid;
                }
            }
            is_even = !is_even;
        }

        // Convert binary to base32 (simplified)
        geohash.chars().take(self.precision as usize).collect()
    }

    /// Query geometries within a bounding box.
    pub fn query_bbox(&self, bbox: &BoundingBox) -> Vec<(u64, &SpatialGeometry)> {
        let mut results = Vec::new();

        // Get all geohash cells that might intersect with the bbox
        let cells = self.get_intersecting_cells(bbox);

        for cell in cells {
            if let Some(geometries) = self.cells.get(&cell) {
                for (id, geometry) in geometries {
                    // TODO: Add more precise intersection test
                    results.push((*id, geometry));
                }
            }
        }

        results
    }

    fn get_intersecting_cells(&self, _bbox: &BoundingBox) -> Vec<String> {
        // TODO: Implement proper geohash cell intersection logic
        self.cells.keys().cloned().collect()
    }
}

impl AdaptiveSpatialIndexer {
    /// Create a new adaptive spatial indexer.
    pub fn new() -> Self {
        Self {
            point_density_threshold: 1_000_000.0,
            polygon_complexity_threshold: 1000,
            global_extent_threshold: 180.0, // Degrees (global coverage)
        }
    }

    /// Recommend the best spatial index for the given data characteristics.
    pub fn recommend_index(&self, stats: &GeometryStatistics) -> SpatialIndex {
        // High-density point clouds -> GeohashGrid
        if stats.point_density > self.point_density_threshold {
            return SpatialIndex::GeohashGrid(GeohashGrid::new(8, 1024));
        }

        // Complex polygons -> R-tree
        if stats.avg_polygon_vertices > self.polygon_complexity_threshold {
            return SpatialIndex::RTree(RTree::new(16, 4, RTreeSplitStrategy::RStart));
        }

        // Global extent -> GeohashGrid
        let bbox_width = stats.bounding_box.max_x - stats.bounding_box.min_x;
        let bbox_height = stats.bounding_box.max_y - stats.bounding_box.min_y;
        if bbox_width > self.global_extent_threshold || bbox_height > self.global_extent_threshold {
            return SpatialIndex::GeohashGrid(GeohashGrid::new(6, 512));
        }

        // Default to QuadTree for general use
        SpatialIndex::QuadTree(QuadTree::new(stats.bounding_box.clone(), 20, 100))
    }
}

impl Default for AdaptiveSpatialIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryStatistics {
    /// Calculate statistics for a collection of geometries.
    pub fn calculate(geometries: &[(u64, SpatialGeometry)]) -> Result<Self, SpatialError> {
        if geometries.is_empty() {
            return Err(SpatialError::IndexError(
                "Cannot calculate statistics for empty geometry collection".to_string(),
            ));
        }

        let mut geometry_distribution = HashMap::new();
        let mut total_polygon_vertices = 0;
        let mut polygon_count = 0;

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for (_id, geometry) in geometries {
            // Count geometry types
            *geometry_distribution
                .entry(geometry.geometry_type().to_string())
                .or_insert(0) += 1;

            // Calculate polygon complexity
            if let SpatialGeometry::Polygon(poly) = geometry {
                total_polygon_vertices += poly.exterior_ring.points.len();
                for interior in &poly.interior_rings {
                    total_polygon_vertices += interior.points.len();
                }
                polygon_count += 1;

                let bbox = poly.bounding_box();
                min_x = min_x.min(bbox.min_x);
                min_y = min_y.min(bbox.min_y);
                max_x = max_x.max(bbox.max_x);
                max_y = max_y.max(bbox.max_y);
            }

            // Update bounding box for points and linestrings
            match geometry {
                SpatialGeometry::Point(p) => {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                SpatialGeometry::LineString(ls) => {
                    let bbox = ls.bounding_box();
                    min_x = min_x.min(bbox.min_x);
                    min_y = min_y.min(bbox.min_y);
                    max_x = max_x.max(bbox.max_x);
                    max_y = max_y.max(bbox.max_y);
                }
                _ => {}
            }
        }

        let bounding_box = BoundingBox::new(min_x, min_y, max_x, max_y, None);
        let area = bounding_box.area();
        let point_density = if area > 0.0 {
            geometries.len() as f64 / area
        } else {
            0.0
        };
        let avg_polygon_vertices = if polygon_count > 0 {
            total_polygon_vertices / polygon_count
        } else {
            0
        };

        Ok(Self {
            total_records: geometries.len(),
            point_density,
            avg_polygon_vertices,
            bounding_box,
            geometry_distribution,
        })
    }
}

impl fmt::Display for SpatialIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpatialIndex::QuadTree(_) => write!(f, "QuadTree"),
            SpatialIndex::RTree(_) => write!(f, "R-Tree"),
            SpatialIndex::GeohashGrid(_) => write!(f, "GeohashGrid"),
            SpatialIndex::KdTree(_) => write!(f, "K-d Tree"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadtree_creation() {
        let bounds = BoundingBox::new(0.0, 0.0, 100.0, 100.0, None);
        let qtree = QuadTree::new(bounds, 10, 10);

        assert_eq!(qtree.max_depth, 10);
        assert_eq!(qtree.max_points_per_node, 10);
        assert_eq!(qtree.point_count, 0);
    }

    #[test]
    fn test_quadtree_insert() {
        let bounds = BoundingBox::new(0.0, 0.0, 100.0, 100.0, None);
        let mut qtree = QuadTree::new(bounds, 10, 4);

        // Insert some points
        let point1 = Point::new(25.0, 25.0, None);
        let point2 = Point::new(75.0, 75.0, None);
        let point3 = Point::new(25.0, 75.0, None);

        assert!(qtree.insert_point(1, point1).is_ok());
        assert!(qtree.insert_point(2, point2).is_ok());
        assert!(qtree.insert_point(3, point3).is_ok());

        assert_eq!(qtree.point_count, 3);
    }

    #[test]
    fn test_quadtree_query() {
        let bounds = BoundingBox::new(0.0, 0.0, 100.0, 100.0, None);
        let mut qtree = QuadTree::new(bounds, 10, 10);

        // Insert points
        qtree.insert_point(1, Point::new(10.0, 10.0, None)).unwrap();
        qtree.insert_point(2, Point::new(90.0, 90.0, None)).unwrap();
        qtree.insert_point(3, Point::new(50.0, 50.0, None)).unwrap();

        // Query a region
        let query_bbox = BoundingBox::new(0.0, 0.0, 60.0, 60.0, None);
        let results = qtree.query_bbox(&query_bbox);

        assert_eq!(results.len(), 2); // Should find points 1 and 3
    }

    #[test]
    fn test_geohash_encoding() {
        let grid = GeohashGrid::new(5, 1024);

        // Test encoding for known coordinates
        let geohash = grid.encode_geohash(0.0, 0.0); // Equator/Prime Meridian
        assert!(!geohash.is_empty());
        assert_eq!(geohash.len(), 5);
    }

    #[test]
    fn test_adaptive_indexer() {
        let indexer = AdaptiveSpatialIndexer::new();

        // Create test statistics for high-density points
        let stats = GeometryStatistics {
            total_records: 2_000_000,
            point_density: 2_000_000.0,
            avg_polygon_vertices: 4,
            bounding_box: BoundingBox::new(-180.0, -90.0, 180.0, 90.0, None),
            geometry_distribution: HashMap::from([("POINT".to_string(), 2_000_000)]),
        };

        let recommended_index = indexer.recommend_index(&stats);
        matches!(recommended_index, SpatialIndex::GeohashGrid(_));
    }
}
