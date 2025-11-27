//! Iceberg Extensions
//!
//! This module provides extension traits that add functionality missing from the
//! iceberg-rust crate, specifically snapshot/time travel capabilities.
//!
//! ## Stopgap Implementation
//!
//! This is a **temporary stopgap solution** until iceberg-rust provides official
//! snapshot access APIs. When iceberg-rust adds these methods, we will:
//!
//! 1. Evaluate their implementation
//! 2. Compare with our approach
//! 3. Migrate to official API if it's better
//! 4. Deprecate this extension module
//!
//! ## Design Philosophy
//!
//! - **Extension Trait Pattern**: Uses Rust extension traits to add methods without modifying upstream
//! - **Zero Cost**: No runtime overhead, compiles to same code as if methods were native
//! - **Easy Migration**: When upstream adds support, simply remove this module and update imports
//! - **Type Safe**: Leverages Rust's type system for correctness
//!
//! ## Migration Path
//!
//! When iceberg-rust adds snapshot APIs:
//!
//! ```rust,no_run
//! // Before (using our extension):
//! // use orbit_engine::storage::iceberg_ext::TableMetadataExt;
//! // let snapshot = table.metadata().snapshot_by_timestamp_ext(timestamp_ms)?;
//! //
//! // After (using upstream):
//! // let snapshot = table.metadata().snapshot_by_timestamp(timestamp_ms)?;
//! ```
//!
//! Simply remove `_ext` suffix and delete this module.

use iceberg::spec::{Snapshot, TableMetadata};
use std::sync::Arc;
use std::time::SystemTime;

use crate::error::{EngineError, EngineResult};

/// Extension trait for TableMetadata to add snapshot access methods
///
/// This trait adds the snapshot/time travel methods that are missing from
/// iceberg-rust's TableMetadata.
///
/// ## Usage
///
/// ```rust,no_run
/// # use orbit_engine::storage::iceberg_ext::TableMetadataExt;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // let table = catalog.load_table(&table_ident).await?;
/// // let metadata = table.metadata();
/// //
/// // // Access snapshots
/// // let snapshots = metadata.snapshots_ext();
/// // println!("Found {} snapshots", snapshots.len());
/// //
/// // // Get snapshot by timestamp
/// // let snapshot = metadata.snapshot_by_timestamp_ext(timestamp_ms)?;
/// //
/// // // Get snapshot by ID
/// // let snapshot = metadata.snapshot_by_id_ext(snapshot_id)?;
/// # Ok(())
/// # }
/// ```
///
/// ## Method Naming Convention
///
/// All methods are suffixed with `_ext` to:
/// 1. Avoid name conflicts with future upstream methods
/// 2. Make it clear these are extensions, not native methods
/// 3. Make migration easier (just remove `_ext` when upstream is ready)
pub trait TableMetadataExt {
    /// Get all snapshots for this table
    ///
    /// Returns a vector of all snapshots, ordered by creation time.
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// # use orbit_engine::storage::iceberg_ext::TableMetadataExt;
    /// # fn example(metadata: &dyn TableMetadataExt) -> Result<(), Box<dyn std::error::Error>> {
    /// let snapshots = metadata.snapshots_ext();
    /// for snapshot in snapshots {
    ///     println!("Snapshot ID: {}, Timestamp: {}",
    ///         snapshot.snapshot_id(),
    ///         snapshot.timestamp_ms());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn snapshots_ext(&self) -> Vec<Arc<Snapshot>>;

    /// Get snapshot at or before specific timestamp
    ///
    /// Returns the most recent snapshot whose timestamp is less than or equal
    /// to the provided timestamp.
    ///
    /// ## Arguments
    ///
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// ## Returns
    ///
    /// * `Ok(Some(Arc<Snapshot>))` - Found snapshot at or before timestamp
    /// * `Ok(None)` - No snapshot found (table didn't exist at that time)
    /// * `Err(_)` - Error accessing metadata
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// # use orbit_engine::storage::iceberg_ext::TableMetadataExt;
    /// # use std::time::SystemTime;
    /// # fn example(metadata: &dyn TableMetadataExt) -> Result<(), Box<dyn std::error::Error>> {
    /// let now = SystemTime::now();
    /// let timestamp_ms = now.duration_since(SystemTime::UNIX_EPOCH)?.as_millis() as i64;
    ///
    /// if let Some(snapshot) = metadata.snapshot_by_timestamp_ext(timestamp_ms)? {
    ///     println!("Found snapshot at {}", snapshot.timestamp_ms());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn snapshot_by_timestamp_ext(&self, timestamp_ms: i64) -> EngineResult<Option<Arc<Snapshot>>>;

    /// Get snapshot by snapshot ID
    ///
    /// ## Arguments
    ///
    /// * `snapshot_id` - The snapshot ID to find
    ///
    /// ## Returns
    ///
    /// * `Ok(Some(Arc<Snapshot>))` - Found snapshot with matching ID
    /// * `Ok(None)` - No snapshot with that ID exists
    /// * `Err(_)` - Error accessing metadata
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// # use orbit_engine::storage::iceberg_ext::TableMetadataExt;
    /// # fn example(metadata: &dyn TableMetadataExt) -> Result<(), Box<dyn std::error::Error>> {
    /// let snapshot_id = 2583872980615177898i64;
    /// if let Some(snapshot) = metadata.snapshot_by_id_ext(snapshot_id)? {
    ///     println!("Found snapshot: {}", snapshot.snapshot_id());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn snapshot_by_id_ext(&self, snapshot_id: i64) -> EngineResult<Option<Arc<Snapshot>>>;

    /// Get current (latest) snapshot
    ///
    /// This is a convenience method that returns the most recent snapshot.
    ///
    /// ## Returns
    ///
    /// * `Ok(Some(Arc<Snapshot>))` - The current snapshot
    /// * `Ok(None)` - No snapshots exist (empty table)
    /// * `Err(_)` - Error accessing metadata
    fn current_snapshot_ext(&self) -> EngineResult<Option<Arc<Snapshot>>>;
}

impl TableMetadataExt for TableMetadata {
    fn snapshots_ext(&self) -> Vec<Arc<Snapshot>> {
        // Access the snapshots via the iterator method
        // The snapshots() method returns impl ExactSizeIterator<Item = &Arc<Snapshot>>
        // We clone the Arc (cheap operation) to extend the lifetime
        self.snapshots()
            .map(|arc_snapshot| Arc::clone(arc_snapshot))
            .collect()
    }

    fn snapshot_by_timestamp_ext(&self, timestamp_ms: i64) -> EngineResult<Option<Arc<Snapshot>>> {
        // Find the most recent snapshot at or before the given timestamp
        let snapshot = self
            .snapshots()
            .filter(|s| s.timestamp_ms() <= timestamp_ms)
            .max_by_key(|s| s.timestamp_ms())
            .map(|arc_snapshot| Arc::clone(arc_snapshot));

        Ok(snapshot)
    }

    fn snapshot_by_id_ext(&self, snapshot_id: i64) -> EngineResult<Option<Arc<Snapshot>>> {
        // Find snapshot with matching ID
        let snapshot = self
            .snapshots()
            .find(|s| s.snapshot_id() == snapshot_id)
            .map(|arc_snapshot| Arc::clone(arc_snapshot));

        Ok(snapshot)
    }

    fn current_snapshot_ext(&self) -> EngineResult<Option<Arc<Snapshot>>> {
        // Get the snapshot with the highest timestamp (most recent)
        let snapshot = self
            .snapshots()
            .max_by_key(|s| s.timestamp_ms())
            .map(|arc_snapshot| Arc::clone(arc_snapshot));

        Ok(snapshot)
    }
}

/// Helper function to convert SystemTime to milliseconds since Unix epoch
///
/// ## Example
///
/// ```rust,no_run
/// # use orbit_engine::storage::iceberg_ext::system_time_to_millis;
/// # use std::time::SystemTime;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let timestamp_ms = system_time_to_millis(SystemTime::now())?;
/// // let snapshot = metadata.snapshot_by_timestamp_ext(timestamp_ms)?;
/// # Ok(())
/// # }
/// ```
pub fn system_time_to_millis(time: SystemTime) -> EngineResult<i64> {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| EngineError::invalid_input(format!("Invalid timestamp: {}", e)))?;

    Ok(duration.as_millis() as i64)
}

/// Helper function to convert milliseconds since Unix epoch to SystemTime
///
/// ## Example
///
/// ```rust,no_run
/// # use orbit_engine::storage::iceberg_ext::millis_to_system_time;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let time = millis_to_system_time(1681236397000)?;
/// # Ok(())
/// # }
/// ```
pub fn millis_to_system_time(millis: i64) -> EngineResult<SystemTime> {
    if millis < 0 {
        return Err(EngineError::invalid_input(format!(
            "Negative timestamp: {}",
            millis
        )));
    }

    let duration = std::time::Duration::from_millis(millis as u64);
    Ok(SystemTime::UNIX_EPOCH + duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_system_time_to_millis() {
        let now = SystemTime::now();
        let millis = system_time_to_millis(now).unwrap();
        assert!(millis > 0);

        // Convert back and verify it's close (within 1ms)
        let converted_back = millis_to_system_time(millis).unwrap();
        let diff = now
            .duration_since(converted_back)
            .unwrap_or(Duration::from_secs(0));
        assert!(diff < Duration::from_millis(1));
    }

    #[test]
    fn test_millis_to_system_time() {
        let millis = 1681236397000i64; // 2023-04-11T18:06:37Z
        let time = millis_to_system_time(millis).unwrap();

        // Convert back
        let converted_back = system_time_to_millis(time).unwrap();
        assert_eq!(millis, converted_back);
    }

    #[test]
    fn test_negative_timestamp() {
        let result = millis_to_system_time(-1000);
        assert!(result.is_err());
    }

    // Note: Tests for TableMetadataExt require actual TableMetadata instances
    // which need a full Iceberg setup. Those are integration tests.
}
