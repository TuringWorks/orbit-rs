//! OrbitQL Spatial Functions and Operations
//!
//! This module provides spatial functionality for OrbitQL, including function
//! registry, spatial operations, and integration with the spatial subsystem.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Spatial function registry for OrbitQL
#[derive(Debug, Clone)]
pub struct SpatialFunctionRegistry {
    functions: HashMap<String, SpatialFunctionInfo>,
}

/// Information about a spatial function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialFunctionInfo {
    pub name: String,
    pub category: SpatialFunctionCategory,
    pub description: String,
    pub parameters: Vec<SpatialParameter>,
    pub return_type: SpatialReturnType,
    pub examples: Vec<String>,
}

/// Categories of spatial functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialFunctionCategory {
    Constructor,
    Accessor,
    Measurement,
    Relationship,
    Processing,
    Conversion,
    Validation,
    Analysis,
    Clustering,
    Index,
}

/// Spatial function parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialParameter {
    pub name: String,
    pub param_type: SpatialParameterType,
    pub required: bool,
    pub description: String,
}

/// Types for spatial function parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialParameterType {
    Geometry,
    Point,
    LineString,
    Polygon,
    Number,
    Integer,
    String,
    Boolean,
    Array(Box<SpatialParameterType>),
}

/// Return types for spatial functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialReturnType {
    Geometry,
    Point,
    LineString,
    Polygon,
    Number,
    Integer,
    Boolean,
    String,
    Array(Box<SpatialReturnType>),
    Void,
}

impl SpatialFunctionRegistry {
    /// Create a new spatial function registry with default functions
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };
        registry.register_default_functions();
        registry
    }

    /// Register a spatial function
    pub fn register(&mut self, info: SpatialFunctionInfo) {
        self.functions.insert(info.name.clone(), info);
    }

    /// Get function information
    pub fn get(&self, name: &str) -> Option<&SpatialFunctionInfo> {
        self.functions.get(name)
    }

    /// List all functions in a category
    pub fn list_category(&self, category: &SpatialFunctionCategory) -> Vec<&SpatialFunctionInfo> {
        self.functions
            .values()
            .filter(|f| &f.category == category)
            .collect()
    }

    /// Get all function names
    pub fn function_names(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    /// Register default spatial functions
    fn register_default_functions(&mut self) {
        // Constructor functions
        self.register(SpatialFunctionInfo {
            name: "ST_Point".to_string(),
            category: SpatialFunctionCategory::Constructor,
            description: "Create a point from X and Y coordinates".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "x".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "X coordinate".to_string(),
                },
                SpatialParameter {
                    name: "y".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "Y coordinate".to_string(),
                },
            ],
            return_type: SpatialReturnType::Point,
            examples: vec!["ST_Point(1.0, 2.0)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_MakePoint".to_string(),
            category: SpatialFunctionCategory::Constructor,
            description: "Create a point from X, Y, and optional Z, M coordinates".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "x".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "X coordinate".to_string(),
                },
                SpatialParameter {
                    name: "y".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "Y coordinate".to_string(),
                },
                SpatialParameter {
                    name: "z".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: false,
                    description: "Z coordinate".to_string(),
                },
            ],
            return_type: SpatialReturnType::Point,
            examples: vec![
                "ST_MakePoint(1.0, 2.0)".to_string(),
                "ST_MakePoint(1.0, 2.0, 3.0)".to_string(),
            ],
        });

        // Measurement functions
        self.register(SpatialFunctionInfo {
            name: "ST_Distance".to_string(),
            category: SpatialFunctionCategory::Measurement,
            description: "Calculate distance between two geometries".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geom1".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geom2".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Number,
            examples: vec!["ST_Distance(ST_Point(0, 0), ST_Point(1, 1))".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Area".to_string(),
            category: SpatialFunctionCategory::Measurement,
            description: "Calculate the area of a polygon".to_string(),
            parameters: vec![SpatialParameter {
                name: "geom".to_string(),
                param_type: SpatialParameterType::Polygon,
                required: true,
                description: "Polygon geometry".to_string(),
            }],
            return_type: SpatialReturnType::Number,
            examples: vec!["ST_Area(polygon)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Length".to_string(),
            category: SpatialFunctionCategory::Measurement,
            description: "Calculate the length of a linestring".to_string(),
            parameters: vec![SpatialParameter {
                name: "geom".to_string(),
                param_type: SpatialParameterType::LineString,
                required: true,
                description: "LineString geometry".to_string(),
            }],
            return_type: SpatialReturnType::Number,
            examples: vec!["ST_Length(linestring)".to_string()],
        });

        // Relationship functions
        self.register(SpatialFunctionInfo {
            name: "ST_Contains".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if geometry A contains geometry B".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Container geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Contained geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Contains(polygon, point)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Intersects".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries intersect".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Intersects(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Within".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if geometry A is within geometry B".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Geometry to test".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Container geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Within(point, polygon)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Overlaps".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries overlap".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Overlaps(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Touches".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries touch".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Touches(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Crosses".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries cross".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Crosses(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Disjoint".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries are disjoint (do not intersect)".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Disjoint(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Equals".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries are equal".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_Equals(geom1, geom2)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Envelope".to_string(),
            category: SpatialFunctionCategory::Processing,
            description: "Get the bounding box of a geometry as a polygon".to_string(),
            parameters: vec![SpatialParameter {
                name: "geom".to_string(),
                param_type: SpatialParameterType::Geometry,
                required: true,
                description: "Input geometry".to_string(),
            }],
            return_type: SpatialReturnType::Polygon,
            examples: vec!["ST_Envelope(geom)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_DWithin".to_string(),
            category: SpatialFunctionCategory::Relationship,
            description: "Test if two geometries are within a given distance".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geomA".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "First geometry".to_string(),
                },
                SpatialParameter {
                    name: "geomB".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Second geometry".to_string(),
                },
                SpatialParameter {
                    name: "distance".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "Distance threshold".to_string(),
                },
            ],
            return_type: SpatialReturnType::Boolean,
            examples: vec!["ST_DWithin(geom1, geom2, 100.0)".to_string()],
        });

        // Processing functions
        self.register(SpatialFunctionInfo {
            name: "ST_Buffer".to_string(),
            category: SpatialFunctionCategory::Processing,
            description: "Create a buffer around a geometry".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "geom".to_string(),
                    param_type: SpatialParameterType::Geometry,
                    required: true,
                    description: "Input geometry".to_string(),
                },
                SpatialParameter {
                    name: "distance".to_string(),
                    param_type: SpatialParameterType::Number,
                    required: true,
                    description: "Buffer distance".to_string(),
                },
            ],
            return_type: SpatialReturnType::Geometry,
            examples: vec!["ST_Buffer(geom, 10.0)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Centroid".to_string(),
            category: SpatialFunctionCategory::Processing,
            description: "Calculate the centroid of a geometry".to_string(),
            parameters: vec![SpatialParameter {
                name: "geom".to_string(),
                param_type: SpatialParameterType::Geometry,
                required: true,
                description: "Input geometry".to_string(),
            }],
            return_type: SpatialReturnType::Point,
            examples: vec!["ST_Centroid(polygon)".to_string()],
        });

        // Accessor functions
        self.register(SpatialFunctionInfo {
            name: "ST_X".to_string(),
            category: SpatialFunctionCategory::Accessor,
            description: "Get X coordinate of a point".to_string(),
            parameters: vec![SpatialParameter {
                name: "point".to_string(),
                param_type: SpatialParameterType::Point,
                required: true,
                description: "Point geometry".to_string(),
            }],
            return_type: SpatialReturnType::Number,
            examples: vec!["ST_X(point)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_Y".to_string(),
            category: SpatialFunctionCategory::Accessor,
            description: "Get Y coordinate of a point".to_string(),
            parameters: vec![SpatialParameter {
                name: "point".to_string(),
                param_type: SpatialParameterType::Point,
                required: true,
                description: "Point geometry".to_string(),
            }],
            return_type: SpatialReturnType::Number,
            examples: vec!["ST_Y(point)".to_string()],
        });

        // Analysis functions
        self.register(SpatialFunctionInfo {
            name: "ST_KMeans".to_string(),
            category: SpatialFunctionCategory::Clustering,
            description: "Perform K-means clustering on points".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "points".to_string(),
                    param_type: SpatialParameterType::Array(Box::new(SpatialParameterType::Point)),
                    required: true,
                    description: "Array of points to cluster".to_string(),
                },
                SpatialParameter {
                    name: "k".to_string(),
                    param_type: SpatialParameterType::Integer,
                    required: true,
                    description: "Number of clusters".to_string(),
                },
            ],
            return_type: SpatialReturnType::Array(Box::new(SpatialReturnType::Integer)),
            examples: vec!["ST_KMeans(points_array, 5)".to_string()],
        });

        // Conversion functions
        self.register(SpatialFunctionInfo {
            name: "ST_AsText".to_string(),
            category: SpatialFunctionCategory::Conversion,
            description: "Convert geometry to WKT text".to_string(),
            parameters: vec![SpatialParameter {
                name: "geom".to_string(),
                param_type: SpatialParameterType::Geometry,
                required: true,
                description: "Geometry to convert".to_string(),
            }],
            return_type: SpatialReturnType::String,
            examples: vec!["ST_AsText(geom)".to_string()],
        });

        self.register(SpatialFunctionInfo {
            name: "ST_GeomFromText".to_string(),
            category: SpatialFunctionCategory::Conversion,
            description: "Create geometry from WKT text".to_string(),
            parameters: vec![
                SpatialParameter {
                    name: "wkt".to_string(),
                    param_type: SpatialParameterType::String,
                    required: true,
                    description: "WKT text representation".to_string(),
                },
                SpatialParameter {
                    name: "srid".to_string(),
                    param_type: SpatialParameterType::Integer,
                    required: false,
                    description: "Spatial Reference System ID".to_string(),
                },
            ],
            return_type: SpatialReturnType::Geometry,
            examples: vec![
                "ST_GeomFromText('POINT(1 2)')".to_string(),
                "ST_GeomFromText('POINT(1 2)', 4326)".to_string(),
            ],
        });
    }
}

impl Default for SpatialFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Lazy static registry instance
use std::sync::LazyLock;
pub static SPATIAL_FUNCTIONS: LazyLock<SpatialFunctionRegistry> =
    LazyLock::new(SpatialFunctionRegistry::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_registry() {
        let registry = SpatialFunctionRegistry::new();

        // Test getting a function
        let point_func = registry.get("ST_Point").unwrap();
        assert_eq!(point_func.name, "ST_Point");
        assert_eq!(point_func.category, SpatialFunctionCategory::Constructor);

        // Test listing by category
        let constructors = registry.list_category(&SpatialFunctionCategory::Constructor);
        assert!(!constructors.is_empty());

        // Test function names
        let names = registry.function_names();
        assert!(names.contains(&"ST_Point".to_string()));
        assert!(names.contains(&"ST_Distance".to_string()));
    }

    #[test]
    fn test_function_categories() {
        let registry = SpatialFunctionRegistry::new();

        // Test each category has functions
        let categories = [
            SpatialFunctionCategory::Constructor,
            SpatialFunctionCategory::Measurement,
            SpatialFunctionCategory::Relationship,
            SpatialFunctionCategory::Processing,
            SpatialFunctionCategory::Accessor,
            SpatialFunctionCategory::Clustering,
            SpatialFunctionCategory::Conversion,
        ];

        for category in &categories {
            let functions = registry.list_category(category);
            assert!(
                !functions.is_empty(),
                "Category {:?} should have functions",
                category
            );
        }
    }
}
