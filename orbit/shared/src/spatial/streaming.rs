//! Real-time spatial streaming for GPS tracking and IoT applications.
//!
//! This module provides high-throughput spatial stream processing capabilities
//! for handling millions of spatial updates per second with sub-millisecond latency.

use super::{Point, SpatialError, SpatialGeometry, SpatialOperations};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

/// GPS data point for vehicle tracking.
#[derive(Debug, Clone)]
pub struct GpsDataPoint {
    pub vehicle_id: u64,
    pub timestamp: u64,
    pub location: Point,
    pub speed: f64,
    pub heading: f64,
}

/// Geofence definition for real-time monitoring.
#[derive(Debug, Clone)]
pub struct Geofence {
    pub id: String,
    pub name: String,
    pub geometry: SpatialGeometry,
    pub alert_on_enter: bool,
    pub alert_on_exit: bool,
    pub metadata: HashMap<String, String>,
}

/// Entity state tracking for geofencing.
#[derive(Debug, Clone)]
pub struct EntityState {
    pub entity_id: u64,
    pub current_location: Point,
    pub previous_location: Option<Point>,
    pub inside_geofences: Vec<String>, // Geofence IDs
    pub last_update: u64,
}

/// Spatial streaming processor for real-time updates.
pub struct SpatialStreamProcessor {
    _batch_size: usize,
    _processing_interval: Duration,
    alert_threshold_ms: u64,
    /// Geofences being monitored
    geofences: Arc<RwLock<HashMap<String, Geofence>>>,
    /// Entity state tracking
    entity_states: Arc<RwLock<HashMap<u64, EntityState>>>,
    /// Previous locations for movement detection
    previous_locations: Arc<RwLock<HashMap<u64, Point>>>,
}

impl SpatialStreamProcessor {
    /// Create a new spatial stream processor.
    pub fn new(batch_size: usize, processing_interval: Duration, alert_threshold_ms: u64) -> Self {
        Self {
            _batch_size: batch_size,
            _processing_interval: processing_interval,
            alert_threshold_ms,
            geofences: Arc::new(RwLock::new(HashMap::new())),
            entity_states: Arc::new(RwLock::new(HashMap::new())),
            previous_locations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a geofence for monitoring.
    pub async fn add_geofence(&self, geofence: Geofence) -> Result<(), SpatialError> {
        let mut geofences = self.geofences.write().await;
        geofences.insert(geofence.id.clone(), geofence);
        Ok(())
    }

    /// Remove a geofence.
    pub async fn remove_geofence(&self, geofence_id: &str) -> Result<(), SpatialError> {
        let mut geofences = self.geofences.write().await;
        geofences.remove(geofence_id);
        Ok(())
    }

    /// Process a batch of GPS updates with geofencing and analytics.
    pub async fn process_gps_batch(
        &self,
        updates: Vec<GpsDataPoint>,
    ) -> Result<Vec<SpatialAlert>, SpatialError> {
        let start_time = Instant::now();
        let mut alerts = Vec::new();

        // Get current geofences and entity states
        let geofences = self.geofences.read().await;
        let mut entity_states = self.entity_states.write().await;
        let mut previous_locations = self.previous_locations.write().await;

        // Process each GPS point
        for update in updates {
            let point_geom = SpatialGeometry::Point(update.location.clone());
            let previous_location = previous_locations.get(&update.vehicle_id).cloned();

            // Get or create entity state
            let entity_state =
                entity_states
                    .entry(update.vehicle_id)
                    .or_insert_with(|| EntityState {
                        entity_id: update.vehicle_id,
                        current_location: update.location.clone(),
                        previous_location: None,
                        inside_geofences: Vec::new(),
                        last_update: update.timestamp,
                    });

            // Update entity state
            entity_state.previous_location = previous_location.clone();
            entity_state.current_location = update.location.clone();
            entity_state.last_update = update.timestamp;

            // Check geofence violations
            let mut currently_inside = Vec::new();
            for (fence_id, geofence) in geofences.iter() {
                match SpatialOperations::intersects(&point_geom, &geofence.geometry) {
                    Ok(true) => {
                        currently_inside.push(fence_id.clone());

                        // Check if entity entered this geofence
                        let was_inside = entity_state.inside_geofences.contains(fence_id);
                        if !was_inside && geofence.alert_on_enter {
                            alerts.push(SpatialAlert {
                                alert_type: AlertType::GeofenceEntered,
                                entity_id: update.vehicle_id,
                                location: update.location.clone(),
                                timestamp: update.timestamp,
                                message: format!(
                                    "Entity {} entered geofence {}",
                                    update.vehicle_id, geofence.name
                                ),
                                geofence_id: Some(fence_id.clone()),
                            });
                        }
                    }
                    Ok(false) => {
                        // Check if entity exited this geofence
                        let was_inside = entity_state.inside_geofences.contains(fence_id);
                        if was_inside && geofence.alert_on_exit {
                            alerts.push(SpatialAlert {
                                alert_type: AlertType::GeofenceExited,
                                entity_id: update.vehicle_id,
                                location: update.location.clone(),
                                timestamp: update.timestamp,
                                message: format!(
                                    "Entity {} exited geofence {}",
                                    update.vehicle_id, geofence.name
                                ),
                                geofence_id: Some(fence_id.clone()),
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Error checking geofence intersection: {}", e);
                    }
                }
            }

            // Update entity's geofence membership
            entity_state.inside_geofences = currently_inside;

            // Check for speed violations
            if update.speed > 100.0 {
                alerts.push(SpatialAlert {
                    alert_type: AlertType::SpeedViolation,
                    entity_id: update.vehicle_id,
                    location: update.location.clone(),
                    timestamp: update.timestamp,
                    message: format!("Speed violation: {:.1} km/h", update.speed),
                    geofence_id: None,
                });
            }

            // Check for significant movement
            if let Some(prev_loc) = previous_location {
                let distance = update.location.distance_2d(&prev_loc);
                if distance > 1000.0 {
                    // 1km threshold
                    alerts.push(SpatialAlert {
                        alert_type: AlertType::SignificantMovement,
                        entity_id: update.vehicle_id,
                        location: update.location.clone(),
                        timestamp: update.timestamp,
                        message: format!("Significant movement: {:.1} meters", distance),
                        geofence_id: None,
                    });
                }
            }

            // Update previous location
            previous_locations.insert(update.vehicle_id, update.location);
        }

        // Check processing time
        let processing_time = start_time.elapsed();
        if processing_time.as_millis() > self.alert_threshold_ms as u128 {
            tracing::warn!(
                "Spatial processing took {} ms, exceeds threshold of {} ms",
                processing_time.as_millis(),
                self.alert_threshold_ms
            );
        }

        Ok(alerts)
    }

    /// Get current entity state.
    pub async fn get_entity_state(&self, entity_id: u64) -> Option<EntityState> {
        let entity_states = self.entity_states.read().await;
        entity_states.get(&entity_id).cloned()
    }

    /// Get all entities currently inside a geofence.
    pub async fn get_entities_in_geofence(&self, geofence_id: &str) -> Vec<u64> {
        let entity_states = self.entity_states.read().await;
        entity_states
            .values()
            .filter(|state| state.inside_geofences.contains(&geofence_id.to_string()))
            .map(|state| state.entity_id)
            .collect()
    }

    /// Calculate real-time analytics for a batch of updates.
    pub async fn calculate_analytics(
        &self,
        updates: &[GpsDataPoint],
    ) -> Result<SpatialAnalytics, SpatialError> {
        if updates.is_empty() {
            return Ok(SpatialAnalytics::default());
        }

        let mut total_distance = 0.0;
        let mut total_speed = 0.0;
        let mut max_speed = 0.0;

        let mut entity_ids = std::collections::HashSet::new();
        for update in updates {
            entity_ids.insert(update.vehicle_id);
            total_speed += update.speed;
            if update.speed > max_speed {
                max_speed = update.speed;
            }

            // Calculate distance from previous location if available
            let previous_locations = self.previous_locations.read().await;
            if let Some(prev_loc) = previous_locations.get(&update.vehicle_id) {
                total_distance += update.location.distance_2d(prev_loc);
            }
        }

        let entity_count = entity_ids.len() as u64;
        let avg_speed = if !updates.is_empty() {
            total_speed / updates.len() as f64
        } else {
            0.0
        };

        Ok(SpatialAnalytics {
            total_updates: updates.len(),
            unique_entities: entity_count,
            average_speed: avg_speed,
            max_speed,
            total_distance,
            processing_timestamp: updates.last().map(|u| u.timestamp).unwrap_or(0),
        })
    }
}

/// Spatial alert types.
#[derive(Debug, Clone)]
pub enum AlertType {
    GeofenceEntered,
    GeofenceExited,
    SpeedViolation,
    SignificantMovement,
    ProximityAlert,
}

/// Spatial alert notification.
#[derive(Debug, Clone)]
pub struct SpatialAlert {
    pub alert_type: AlertType,
    pub entity_id: u64,
    pub location: Point,
    pub timestamp: u64,
    pub message: String,
    pub geofence_id: Option<String>,
}

/// Real-time spatial analytics.
#[derive(Debug, Clone, Default)]
pub struct SpatialAnalytics {
    pub total_updates: usize,
    pub unique_entities: u64,
    pub average_speed: f64,
    pub max_speed: f64,
    pub total_distance: f64,
    pub processing_timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::{LinearRing, Polygon};

    #[tokio::test]
    async fn test_spatial_stream_processor_creation() {
        let processor = SpatialStreamProcessor::new(100, Duration::from_millis(100), 10);
        assert_eq!(processor._batch_size, 100);
    }

    #[tokio::test]
    async fn test_geofence_add_remove() {
        let processor = SpatialStreamProcessor::new(100, Duration::from_millis(100), 10);

        let polygon_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];
        let exterior_ring = LinearRing::new(polygon_points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], None).unwrap();

        let geofence = Geofence {
            id: "test_fence".to_string(),
            name: "Test Fence".to_string(),
            geometry: SpatialGeometry::Polygon(polygon),
            alert_on_enter: true,
            alert_on_exit: true,
            metadata: HashMap::new(),
        };

        assert!(processor.add_geofence(geofence).await.is_ok());
        assert!(processor.remove_geofence("test_fence").await.is_ok());
    }

    #[tokio::test]
    async fn test_geofence_enter_exit_detection() {
        let processor = SpatialStreamProcessor::new(100, Duration::from_millis(100), 10);

        // Create a geofence (square from 0,0 to 4,4)
        let polygon_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];
        let exterior_ring = LinearRing::new(polygon_points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], None).unwrap();

        let geofence = Geofence {
            id: "test_zone".to_string(),
            name: "Test Zone".to_string(),
            geometry: SpatialGeometry::Polygon(polygon),
            alert_on_enter: true,
            alert_on_exit: true,
            metadata: HashMap::new(),
        };

        processor.add_geofence(geofence).await.unwrap();

        // First update: entity outside geofence
        let updates1 = vec![GpsDataPoint {
            vehicle_id: 1,
            timestamp: 1000,
            location: Point::new(10.0, 10.0, None),
            speed: 50.0,
            heading: 0.0,
        }];

        let alerts1 = processor.process_gps_batch(updates1).await.unwrap();
        assert_eq!(alerts1.len(), 0); // No alerts when outside

        // Second update: entity enters geofence
        let updates2 = vec![GpsDataPoint {
            vehicle_id: 1,
            timestamp: 2000,
            location: Point::new(2.0, 2.0, None), // Inside the geofence
            speed: 30.0,
            heading: 0.0,
        }];

        let alerts2 = processor.process_gps_batch(updates2).await.unwrap();
        assert!(alerts2
            .iter()
            .any(|a| matches!(a.alert_type, AlertType::GeofenceEntered)));

        // Third update: entity exits geofence
        let updates3 = vec![GpsDataPoint {
            vehicle_id: 1,
            timestamp: 3000,
            location: Point::new(10.0, 10.0, None), // Outside again
            speed: 40.0,
            heading: 0.0,
        }];

        let alerts3 = processor.process_gps_batch(updates3).await.unwrap();
        assert!(alerts3
            .iter()
            .any(|a| matches!(a.alert_type, AlertType::GeofenceExited)));
    }

    #[tokio::test]
    async fn test_speed_violation_detection() {
        let processor = SpatialStreamProcessor::new(100, Duration::from_millis(100), 10);

        let updates = vec![GpsDataPoint {
            vehicle_id: 1,
            timestamp: 1000,
            location: Point::new(1.0, 1.0, None),
            speed: 120.0, // Over 100 km/h threshold
            heading: 0.0,
        }];

        let alerts = processor.process_gps_batch(updates).await.unwrap();
        assert!(alerts
            .iter()
            .any(|a| matches!(a.alert_type, AlertType::SpeedViolation)));
    }

    #[tokio::test]
    async fn test_analytics_calculation() {
        let processor = SpatialStreamProcessor::new(100, Duration::from_millis(100), 10);

        let updates = vec![
            GpsDataPoint {
                vehicle_id: 1,
                timestamp: 1000,
                location: Point::new(0.0, 0.0, None),
                speed: 50.0,
                heading: 0.0,
            },
            GpsDataPoint {
                vehicle_id: 2,
                timestamp: 1000,
                location: Point::new(1.0, 1.0, None),
                speed: 60.0,
                heading: 45.0,
            },
        ];

        let analytics = processor.calculate_analytics(&updates).await.unwrap();
        assert_eq!(analytics.total_updates, 2);
        assert_eq!(analytics.unique_entities, 2);
        assert!(analytics.average_speed > 0.0);
    }
}
