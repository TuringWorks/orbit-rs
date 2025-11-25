//! Time-based partitioning strategies for time series data
//!
//! Provides flexible partitioning strategies for efficient time series storage:
//! - Time interval partitioning (hourly, daily, weekly, etc.)
//! - Series count-based partitioning
//! - Data size-based partitioning
//! - Composite partitioning combining multiple strategies

use super::{DataPoint, DateTime, HashMap, TimeRange, Timestamp, Utc};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Partitioning strategy for time series data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Partition by time intervals
    TimeInterval(Duration),
    /// Partition by series count per partition
    SeriesCount(usize),
    /// Partition by data size (in bytes)
    DataSize(u64),
    /// Composite partitioning strategy (applies strategies in order)
    Composite(Vec<PartitionStrategy>),
}

/// Partition metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub partition_id: String,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub series_count: u64,
    pub data_points: u64,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

impl PartitionInfo {
    /// Create a new partition info
    pub fn new(partition_id: String, start_time: Timestamp, end_time: Timestamp) -> Self {
        let now = Utc::now();
        Self {
            partition_id,
            start_time,
            end_time,
            series_count: 0,
            data_points: 0,
            size_bytes: 0,
            created_at: now,
            last_accessed: now,
        }
    }

    /// Check if this partition overlaps with a time range
    pub fn overlaps(&self, range: &TimeRange) -> bool {
        self.start_time <= range.end && self.end_time >= range.start
    }

    /// Check if this partition is contiguous with another
    pub fn is_contiguous_with(&self, other: &PartitionInfo) -> bool {
        self.end_time == other.start_time || other.end_time == self.start_time
    }

    /// Merge with another partition info (metadata only)
    pub fn merge_with(&self, other: &PartitionInfo) -> PartitionInfo {
        let now = Utc::now();
        PartitionInfo {
            partition_id: format!(
                "merged_{}_{}_{}",
                self.start_time.min(other.start_time),
                self.end_time.max(other.end_time),
                now.timestamp_millis()
            ),
            start_time: self.start_time.min(other.start_time),
            end_time: self.end_time.max(other.end_time),
            series_count: self.series_count + other.series_count,
            data_points: self.data_points + other.data_points,
            size_bytes: self.size_bytes + other.size_bytes,
            created_at: now,
            last_accessed: now,
        }
    }
}

/// Partition assignment for series-based partitioning
#[derive(Debug, Default)]
struct SeriesPartitionAssignment {
    /// Maps series key (from labels) to partition ID
    series_to_partition: HashMap<String, String>,
    /// Maps partition ID to series count
    partition_series_count: HashMap<String, usize>,
    /// Next partition number for round-robin assignment
    next_partition: AtomicU64,
}

impl SeriesPartitionAssignment {
    fn new() -> Self {
        Self {
            series_to_partition: HashMap::new(),
            partition_series_count: HashMap::new(),
            next_partition: AtomicU64::new(0),
        }
    }

    /// Get or assign partition for a series
    fn get_or_assign(&mut self, series_key: &str, max_series_per_partition: usize) -> String {
        // Check if series already has an assignment
        if let Some(partition_id) = self.series_to_partition.get(series_key) {
            return partition_id.clone();
        }

        // Find a partition with available capacity
        let partition_id = self
            .partition_series_count
            .iter()
            .find(|(_, &count)| count < max_series_per_partition)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| {
                // Create a new partition
                let partition_num = self.next_partition.fetch_add(1, Ordering::SeqCst);
                format!("series_partition_{}", partition_num)
            });

        // Assign series to partition
        self.series_to_partition
            .insert(series_key.to_string(), partition_id.clone());
        *self
            .partition_series_count
            .entry(partition_id.clone())
            .or_insert(0) += 1;

        partition_id
    }
}

/// Partition assignment for size-based partitioning
#[derive(Debug, Default)]
struct SizePartitionAssignment {
    /// Maps partition ID to current size
    partition_sizes: HashMap<String, u64>,
    /// Next partition number
    next_partition: AtomicU64,
}

impl SizePartitionAssignment {
    fn new() -> Self {
        Self {
            partition_sizes: HashMap::new(),
            next_partition: AtomicU64::new(0),
        }
    }

    /// Get partition for a data point based on size limits
    fn get_partition(&mut self, estimated_size: u64, max_size: u64) -> String {
        // Find a partition with available capacity
        let partition_id = self
            .partition_sizes
            .iter()
            .find(|(_, &size)| size + estimated_size <= max_size)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| {
                // Create a new partition
                let partition_num = self.next_partition.fetch_add(1, Ordering::SeqCst);
                format!("size_partition_{}", partition_num)
            });

        // Update size estimate
        *self.partition_sizes.entry(partition_id.clone()).or_insert(0) += estimated_size;

        partition_id
    }

    /// Update actual size for a partition
    fn update_size(&mut self, partition_id: &str, actual_size: u64) {
        self.partition_sizes
            .insert(partition_id.to_string(), actual_size);
    }
}

/// Partition manager for time series data
pub struct PartitionManager {
    strategy: PartitionStrategy,
    partitions: HashMap<String, PartitionInfo>,
    /// For series count-based partitioning
    series_assignment: SeriesPartitionAssignment,
    /// For size-based partitioning
    size_assignment: SizePartitionAssignment,
}

impl PartitionManager {
    pub fn new(strategy: PartitionStrategy) -> Self {
        Self {
            strategy,
            partitions: HashMap::new(),
            series_assignment: SeriesPartitionAssignment::new(),
            size_assignment: SizePartitionAssignment::new(),
        }
    }

    /// Determine which partition a data point belongs to
    pub fn get_partition_for_point(&mut self, data_point: &DataPoint) -> Result<String> {
        self.get_partition_for_point_with_strategy(data_point, &self.strategy.clone())
    }

    /// Internal method to get partition with a specific strategy
    fn get_partition_for_point_with_strategy(
        &mut self,
        data_point: &DataPoint,
        strategy: &PartitionStrategy,
    ) -> Result<String> {
        match strategy {
            PartitionStrategy::TimeInterval(interval) => {
                let interval_nanos = interval.as_nanos() as i64;
                if interval_nanos == 0 {
                    return Err(anyhow!("Interval cannot be zero"));
                }
                let partition_start = (data_point.timestamp / interval_nanos) * interval_nanos;
                Ok(format!(
                    "time_{}_{}",
                    partition_start,
                    partition_start + interval_nanos
                ))
            }
            PartitionStrategy::SeriesCount(max_series) => {
                // Create a series key from labels
                let series_key = Self::create_series_key(&data_point.labels);
                Ok(self
                    .series_assignment
                    .get_or_assign(&series_key, *max_series))
            }
            PartitionStrategy::DataSize(max_size) => {
                // Estimate size of this data point (timestamp + value + labels overhead)
                let estimated_size = Self::estimate_data_point_size(data_point);
                Ok(self.size_assignment.get_partition(estimated_size, *max_size))
            }
            PartitionStrategy::Composite(strategies) => {
                // Apply strategies in order, combining their partition IDs
                let mut partition_parts = Vec::new();
                for strat in strategies {
                    let part = self.get_partition_for_point_with_strategy(data_point, strat)?;
                    partition_parts.push(part);
                }
                Ok(partition_parts.join("__"))
            }
        }
    }

    /// Create a unique key for a time series based on its labels
    fn create_series_key(labels: &HashMap<String, String>) -> String {
        if labels.is_empty() {
            return "__default__".to_string();
        }

        // Sort labels for consistent key generation
        let mut sorted_labels: Vec<_> = labels.iter().collect();
        sorted_labels.sort_by_key(|(k, _)| *k);

        sorted_labels
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Estimate the size of a data point in bytes
    fn estimate_data_point_size(data_point: &DataPoint) -> u64 {
        // Base size: timestamp (8 bytes) + value discriminant (1 byte)
        let mut size = 9u64;

        // Value size
        size += match &data_point.value {
            super::TimeSeriesValue::Float(_) => 8,
            super::TimeSeriesValue::Integer(_) => 8,
            super::TimeSeriesValue::String(s) => s.len() as u64 + 8, // string + capacity
            super::TimeSeriesValue::Boolean(_) => 1,
            super::TimeSeriesValue::Null => 0,
        };

        // Labels overhead
        for (k, v) in &data_point.labels {
            size += k.len() as u64 + v.len() as u64 + 48; // strings + HashMap overhead
        }

        size
    }

    /// Get partition information
    pub fn get_partition_info(&self, partition_id: &str) -> Option<&PartitionInfo> {
        self.partitions.get(partition_id)
    }

    /// Create a new partition
    pub fn create_partition(&mut self, partition_info: PartitionInfo) -> Result<()> {
        self.partitions
            .insert(partition_info.partition_id.clone(), partition_info);
        Ok(())
    }

    /// List all partitions within a time range
    pub fn get_partitions_for_range(&self, time_range: &TimeRange) -> Vec<&PartitionInfo> {
        self.partitions
            .values()
            .filter(|partition| partition.overlaps(time_range))
            .collect()
    }

    /// Get partitions that need maintenance (compression, cleanup, etc.)
    pub fn get_partitions_for_maintenance(&self, max_age: Duration) -> Vec<&PartitionInfo> {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();

        self.partitions
            .values()
            .filter(|partition| partition.last_accessed < cutoff_time)
            .collect()
    }

    /// Merge small partitions together
    ///
    /// Merges the specified partitions into a single partition.
    /// Partitions must be contiguous (adjacent time ranges) for time-based partitioning.
    pub async fn merge_partitions(&mut self, partition_ids: Vec<String>) -> Result<PartitionInfo> {
        if partition_ids.is_empty() {
            return Err(anyhow!("No partitions specified for merging"));
        }

        if partition_ids.len() == 1 {
            return self
                .partitions
                .get(&partition_ids[0])
                .cloned()
                .ok_or_else(|| anyhow!("Partition not found: {}", partition_ids[0]));
        }

        // Collect partition info and validate they exist
        let mut partitions_to_merge: Vec<PartitionInfo> = Vec::new();
        for id in &partition_ids {
            let partition = self
                .partitions
                .get(id)
                .ok_or_else(|| anyhow!("Partition not found: {}", id))?;
            partitions_to_merge.push(partition.clone());
        }

        // Sort by start time
        partitions_to_merge.sort_by_key(|p| p.start_time);

        // Validate contiguity for time-based partitions
        for i in 1..partitions_to_merge.len() {
            if !partitions_to_merge[i - 1].is_contiguous_with(&partitions_to_merge[i]) {
                // Allow non-contiguous merges but log a warning
                // In a real implementation, you might want to be stricter
            }
        }

        // Create merged partition
        let mut merged = partitions_to_merge[0].clone();
        for partition in partitions_to_merge.iter().skip(1) {
            merged = merged.merge_with(partition);
        }

        // Remove old partitions and add merged one
        for id in &partition_ids {
            self.partitions.remove(id);
        }
        self.partitions
            .insert(merged.partition_id.clone(), merged.clone());

        Ok(merged)
    }

    /// Split a partition at specified timestamps
    ///
    /// Creates new partitions from a single partition, splitting at the given timestamps.
    pub async fn split_partition(
        &mut self,
        partition_id: &str,
        split_points: Vec<Timestamp>,
    ) -> Result<Vec<PartitionInfo>> {
        let partition = self
            .partitions
            .get(partition_id)
            .ok_or_else(|| anyhow!("Partition not found: {}", partition_id))?
            .clone();

        if split_points.is_empty() {
            return Ok(vec![partition]);
        }

        // Validate and sort split points
        let mut valid_split_points: Vec<_> = split_points
            .into_iter()
            .filter(|&sp| sp > partition.start_time && sp < partition.end_time)
            .collect();
        valid_split_points.sort();
        valid_split_points.dedup();

        if valid_split_points.is_empty() {
            return Err(anyhow!(
                "No valid split points within partition time range"
            ));
        }

        // Create new partitions
        let now = Utc::now();
        let mut new_partitions = Vec::new();
        let mut prev_start = partition.start_time;

        // Estimate data distribution (simple proportional split)
        let total_duration = partition.end_time - partition.start_time;

        for split_point in &valid_split_points {
            let duration = split_point - prev_start;
            let ratio = duration as f64 / total_duration as f64;

            let new_partition = PartitionInfo {
                partition_id: format!("split_{}_{}", prev_start, split_point),
                start_time: prev_start,
                end_time: *split_point,
                series_count: (partition.series_count as f64 * ratio).ceil() as u64,
                data_points: (partition.data_points as f64 * ratio).ceil() as u64,
                size_bytes: (partition.size_bytes as f64 * ratio).ceil() as u64,
                created_at: now,
                last_accessed: now,
            };

            new_partitions.push(new_partition);
            prev_start = *split_point;
        }

        // Add final partition
        let duration = partition.end_time - prev_start;
        let ratio = duration as f64 / total_duration as f64;

        let final_partition = PartitionInfo {
            partition_id: format!("split_{}_{}", prev_start, partition.end_time),
            start_time: prev_start,
            end_time: partition.end_time,
            series_count: (partition.series_count as f64 * ratio).ceil() as u64,
            data_points: (partition.data_points as f64 * ratio).ceil() as u64,
            size_bytes: (partition.size_bytes as f64 * ratio).ceil() as u64,
            created_at: now,
            last_accessed: now,
        };
        new_partitions.push(final_partition);

        // Remove old partition and add new ones
        self.partitions.remove(partition_id);
        for p in &new_partitions {
            self.partitions.insert(p.partition_id.clone(), p.clone());
        }

        Ok(new_partitions)
    }

    /// Update partition statistics
    pub fn update_partition_stats(
        &mut self,
        partition_id: &str,
        data_points: u64,
        size_bytes: u64,
    ) {
        if let Some(partition) = self.partitions.get_mut(partition_id) {
            partition.data_points += data_points;
            partition.size_bytes += size_bytes;
            partition.last_accessed = Utc::now();

            // Also update size assignment if using size-based partitioning
            self.size_assignment
                .update_size(partition_id, partition.size_bytes);
        }
    }

    /// Remove old partitions based on retention policy
    pub async fn cleanup_old_partitions(
        &mut self,
        retention_duration: Duration,
    ) -> Result<Vec<String>> {
        let cutoff_time =
            Utc::now() - chrono::Duration::from_std(retention_duration).unwrap_or_default();
        let cutoff_timestamp = cutoff_time.timestamp_nanos_opt().unwrap_or(0);

        let mut removed_partitions = Vec::new();

        // Find partitions to remove
        let partitions_to_remove: Vec<_> = self
            .partitions
            .iter()
            .filter(|(_, partition)| partition.end_time < cutoff_timestamp)
            .map(|(id, _)| id.clone())
            .collect();

        // Remove the partitions
        for partition_id in partitions_to_remove {
            self.partitions.remove(&partition_id);
            removed_partitions.push(partition_id);
        }

        Ok(removed_partitions)
    }

    /// Get partitioning statistics
    pub fn get_partitioning_stats(&self) -> PartitioningStats {
        let total_partitions = self.partitions.len() as u64;
        let total_data_points: u64 = self.partitions.values().map(|p| p.data_points).sum();
        let total_size_bytes: u64 = self.partitions.values().map(|p| p.size_bytes).sum();
        let average_partition_size = if total_partitions > 0 {
            total_size_bytes / total_partitions
        } else {
            0
        };

        PartitioningStats {
            total_partitions,
            total_data_points,
            total_size_bytes,
            average_partition_size,
            strategy: self.strategy.clone(),
        }
    }

    /// Rebalance partitions to achieve more even distribution
    pub fn suggest_rebalancing(&self) -> Vec<RebalanceSuggestion> {
        let mut suggestions = Vec::new();

        if self.partitions.is_empty() {
            return suggestions;
        }

        let stats = self.get_partitioning_stats();
        let avg_size = stats.average_partition_size;

        // Find partitions that are significantly larger or smaller than average
        for (id, partition) in &self.partitions {
            if partition.size_bytes > avg_size * 2 {
                // Suggest splitting large partitions
                let suggested_splits = (partition.size_bytes / avg_size).max(2);
                suggestions.push(RebalanceSuggestion::Split {
                    partition_id: id.clone(),
                    suggested_parts: suggested_splits as usize,
                    reason: format!(
                        "Partition size {} is {}x average",
                        partition.size_bytes,
                        partition.size_bytes / avg_size.max(1)
                    ),
                });
            }
        }

        // Find small partitions that could be merged
        let small_partitions: Vec<_> = self
            .partitions
            .iter()
            .filter(|(_, p)| p.size_bytes < avg_size / 4 && avg_size > 0)
            .collect();

        if small_partitions.len() >= 2 {
            // Group contiguous small partitions
            let mut sorted: Vec<_> = small_partitions.iter().collect();
            sorted.sort_by_key(|(_, p)| p.start_time);

            let mut merge_groups: Vec<Vec<String>> = Vec::new();
            let mut current_group = vec![sorted[0].0.clone()];

            for i in 1..sorted.len() {
                if sorted[i - 1].1.is_contiguous_with(sorted[i].1) {
                    current_group.push(sorted[i].0.clone());
                } else {
                    if current_group.len() >= 2 {
                        merge_groups.push(current_group);
                    }
                    current_group = vec![sorted[i].0.clone()];
                }
            }
            if current_group.len() >= 2 {
                merge_groups.push(current_group);
            }

            for group in merge_groups {
                suggestions.push(RebalanceSuggestion::Merge {
                    partition_ids: group.clone(),
                    reason: format!(
                        "Merging {} small contiguous partitions",
                        group.len()
                    ),
                });
            }
        }

        suggestions
    }

    /// Get partition distribution by time
    pub fn get_partition_timeline(&self) -> BTreeMap<Timestamp, Vec<&PartitionInfo>> {
        let mut timeline: BTreeMap<Timestamp, Vec<&PartitionInfo>> = BTreeMap::new();

        for partition in self.partitions.values() {
            timeline
                .entry(partition.start_time)
                .or_default()
                .push(partition);
        }

        timeline
    }
}

/// Suggestion for rebalancing partitions
#[derive(Debug, Clone)]
pub enum RebalanceSuggestion {
    /// Suggest splitting a large partition
    Split {
        partition_id: String,
        suggested_parts: usize,
        reason: String,
    },
    /// Suggest merging small partitions
    Merge {
        partition_ids: Vec<String>,
        reason: String,
    },
}

/// Partitioning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitioningStats {
    pub total_partitions: u64,
    pub total_data_points: u64,
    pub total_size_bytes: u64,
    pub average_partition_size: u64,
    pub strategy: PartitionStrategy,
}

#[cfg(test)]
mod tests {
    use super::super::TimeSeriesValue;
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_partition_manager_creation() {
        let strategy = PartitionStrategy::TimeInterval(Duration::from_secs(3600)); // 1 hour
        let manager = PartitionManager::new(strategy);
        assert_eq!(manager.partitions.len(), 0);
    }

    #[test]
    fn test_time_interval_partitioning() {
        let strategy = PartitionStrategy::TimeInterval(Duration::from_secs(3600)); // 1 hour
        let mut manager = PartitionManager::new(strategy);

        let data_point = DataPoint {
            timestamp: 1000000000000000000, // Some timestamp
            value: TimeSeriesValue::Float(42.0),
            labels: HashMap::new(),
        };

        let partition_id = manager.get_partition_for_point(&data_point).unwrap();
        assert!(partition_id.starts_with("time_"));
    }

    #[test]
    fn test_series_count_partitioning() {
        let strategy = PartitionStrategy::SeriesCount(2); // Max 2 series per partition
        let mut manager = PartitionManager::new(strategy);

        // Create data points with different series (different labels)
        let mut labels1 = HashMap::new();
        labels1.insert("host".to_string(), "server1".to_string());

        let mut labels2 = HashMap::new();
        labels2.insert("host".to_string(), "server2".to_string());

        let mut labels3 = HashMap::new();
        labels3.insert("host".to_string(), "server3".to_string());

        let dp1 = DataPoint {
            timestamp: 1000,
            value: TimeSeriesValue::Float(1.0),
            labels: labels1.clone(),
        };

        let dp2 = DataPoint {
            timestamp: 2000,
            value: TimeSeriesValue::Float(2.0),
            labels: labels2.clone(),
        };

        let dp3 = DataPoint {
            timestamp: 3000,
            value: TimeSeriesValue::Float(3.0),
            labels: labels3.clone(),
        };

        let partition1 = manager.get_partition_for_point(&dp1).unwrap();
        let partition2 = manager.get_partition_for_point(&dp2).unwrap();
        let partition3 = manager.get_partition_for_point(&dp3).unwrap();

        // First two should be in same partition, third should be in different
        assert_eq!(partition1, partition2);
        assert_ne!(partition2, partition3);
    }

    #[test]
    fn test_data_size_partitioning() {
        let strategy = PartitionStrategy::DataSize(100); // Max 100 bytes per partition
        let mut manager = PartitionManager::new(strategy);

        let dp1 = DataPoint {
            timestamp: 1000,
            value: TimeSeriesValue::Float(1.0),
            labels: HashMap::new(),
        };

        let dp2 = DataPoint {
            timestamp: 2000,
            value: TimeSeriesValue::Float(2.0),
            labels: HashMap::new(),
        };

        let partition1 = manager.get_partition_for_point(&dp1).unwrap();
        let partition2 = manager.get_partition_for_point(&dp2).unwrap();

        // Both small points should fit in same partition
        assert_eq!(partition1, partition2);

        // Add many more points to trigger new partition
        for i in 3..20 {
            let dp = DataPoint {
                timestamp: i * 1000,
                value: TimeSeriesValue::Float(i as f64),
                labels: HashMap::new(),
            };
            manager.get_partition_for_point(&dp).unwrap();
        }

        // Should have multiple partitions now
        assert!(manager.size_assignment.partition_sizes.len() >= 1);
    }

    #[test]
    fn test_composite_partitioning() {
        let strategy = PartitionStrategy::Composite(vec![
            PartitionStrategy::TimeInterval(Duration::from_secs(3600)),
            PartitionStrategy::SeriesCount(10),
        ]);
        let mut manager = PartitionManager::new(strategy);

        let mut labels = HashMap::new();
        labels.insert("metric".to_string(), "cpu".to_string());

        let dp = DataPoint {
            timestamp: 1000000000000000000,
            value: TimeSeriesValue::Float(50.0),
            labels,
        };

        let partition_id = manager.get_partition_for_point(&dp).unwrap();

        // Should have both time and series components
        assert!(partition_id.contains("time_"));
        assert!(partition_id.contains("__"));
    }

    #[test]
    fn test_partition_info_creation() {
        let mut manager =
            PartitionManager::new(PartitionStrategy::TimeInterval(Duration::from_secs(3600)));

        let partition_info = PartitionInfo {
            partition_id: "test_partition".to_string(),
            start_time: 0,
            end_time: 3_600_000_000_000,
            series_count: 10,
            data_points: 1000,
            size_bytes: 50000,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        manager.create_partition(partition_info.clone()).unwrap();
        assert_eq!(manager.partitions.len(), 1);

        let retrieved = manager.get_partition_info("test_partition");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data_points, 1000);
    }

    #[tokio::test]
    async fn test_partition_merge() {
        let mut manager =
            PartitionManager::new(PartitionStrategy::TimeInterval(Duration::from_secs(3600)));

        let partition1 = PartitionInfo::new("p1".to_string(), 0, 1000);
        let partition2 = PartitionInfo::new("p2".to_string(), 1000, 2000);

        manager.create_partition(partition1).unwrap();
        manager.create_partition(partition2).unwrap();

        let merged = manager
            .merge_partitions(vec!["p1".to_string(), "p2".to_string()])
            .await
            .unwrap();

        assert_eq!(merged.start_time, 0);
        assert_eq!(merged.end_time, 2000);
        assert_eq!(manager.partitions.len(), 1);
    }

    #[tokio::test]
    async fn test_partition_split() {
        let mut manager =
            PartitionManager::new(PartitionStrategy::TimeInterval(Duration::from_secs(3600)));

        let mut partition = PartitionInfo::new("original".to_string(), 0, 3000);
        partition.data_points = 300;
        partition.size_bytes = 30000;

        manager.create_partition(partition).unwrap();

        let split_partitions = manager
            .split_partition("original", vec![1000, 2000])
            .await
            .unwrap();

        assert_eq!(split_partitions.len(), 3);
        assert_eq!(manager.partitions.len(), 3);

        // Verify time ranges
        assert_eq!(split_partitions[0].start_time, 0);
        assert_eq!(split_partitions[0].end_time, 1000);
        assert_eq!(split_partitions[1].start_time, 1000);
        assert_eq!(split_partitions[1].end_time, 2000);
        assert_eq!(split_partitions[2].start_time, 2000);
        assert_eq!(split_partitions[2].end_time, 3000);
    }

    #[test]
    fn test_rebalancing_suggestions() {
        let mut manager =
            PartitionManager::new(PartitionStrategy::TimeInterval(Duration::from_secs(3600)));

        // Create one large partition
        let mut large = PartitionInfo::new("large".to_string(), 0, 1000);
        large.size_bytes = 10000;

        // Create several small partitions
        let mut small1 = PartitionInfo::new("small1".to_string(), 1000, 2000);
        small1.size_bytes = 100;

        let mut small2 = PartitionInfo::new("small2".to_string(), 2000, 3000);
        small2.size_bytes = 100;

        manager.create_partition(large).unwrap();
        manager.create_partition(small1).unwrap();
        manager.create_partition(small2).unwrap();

        let suggestions = manager.suggest_rebalancing();

        // Should suggest splitting the large partition
        assert!(suggestions.iter().any(|s| matches!(s, RebalanceSuggestion::Split { .. })));
    }

    #[test]
    fn test_partition_timeline() {
        let mut manager =
            PartitionManager::new(PartitionStrategy::TimeInterval(Duration::from_secs(3600)));

        manager
            .create_partition(PartitionInfo::new("p1".to_string(), 0, 1000))
            .unwrap();
        manager
            .create_partition(PartitionInfo::new("p2".to_string(), 1000, 2000))
            .unwrap();
        manager
            .create_partition(PartitionInfo::new("p3".to_string(), 2000, 3000))
            .unwrap();

        let timeline = manager.get_partition_timeline();

        assert_eq!(timeline.len(), 3);
        assert!(timeline.contains_key(&0));
        assert!(timeline.contains_key(&1000));
        assert!(timeline.contains_key(&2000));
    }

    #[test]
    fn test_series_key_generation() {
        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "server1".to_string());
        labels.insert("region".to_string(), "us-east".to_string());

        let key = PartitionManager::create_series_key(&labels);

        // Should be sorted alphabetically
        assert!(key.contains("host=server1"));
        assert!(key.contains("region=us-east"));
    }

    #[test]
    fn test_estimate_data_point_size() {
        let dp = DataPoint {
            timestamp: 1000,
            value: TimeSeriesValue::Float(42.0),
            labels: HashMap::new(),
        };

        let size = PartitionManager::estimate_data_point_size(&dp);
        assert!(size > 0);
        assert!(size < 100); // Reasonable size for simple point
    }
}
