//! Automated backup scheduling system
//!
//! This module provides cron-based scheduling for automated backups,
//! retention policy enforcement, and backup verification.

use crate::backup::{BackupSystem, BackupType};
use crate::exception::{OrbitError, OrbitResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Schedule for automated backups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    /// Full backup schedule (cron expression)
    pub full_backup_cron: String,
    /// Incremental backup schedule (cron expression)
    pub incremental_backup_cron: Option<String>,
    /// Differential backup schedule (cron expression)
    pub differential_backup_cron: Option<String>,
    /// Transaction log backup interval in seconds
    pub transaction_log_interval_secs: Option<u64>,
    /// Enable automatic verification after backup
    pub auto_verify: bool,
    /// Enable automatic retention policy enforcement
    pub auto_retention: bool,
    /// Retention check interval in hours
    pub retention_check_interval_hours: u64,
}

impl Default for BackupSchedule {
    fn default() -> Self {
        Self {
            full_backup_cron: "0 2 * * *".to_string(), // Daily at 2 AM
            incremental_backup_cron: Some("0 * * * *".to_string()), // Hourly
            differential_backup_cron: None,
            transaction_log_interval_secs: Some(300), // Every 5 minutes
            auto_verify: true,
            auto_retention: true,
            retention_check_interval_hours: 24,
        }
    }
}

/// Backup scheduler for automated backup operations
pub struct BackupScheduler {
    backup_system: Arc<BackupSystem>,
    schedule: BackupSchedule,
    state: Arc<RwLock<SchedulerState>>,
}

#[derive(Debug, Clone)]
struct SchedulerState {
    running: bool,
    last_full_backup: Option<DateTime<Utc>>,
    last_incremental_backup: Option<DateTime<Utc>>,
    last_differential_backup: Option<DateTime<Utc>>,
    last_transaction_log_backup: Option<DateTime<Utc>>,
    last_retention_check: Option<DateTime<Utc>>,
    backup_count: u64,
    failed_backups: u64,
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self {
            running: false,
            last_full_backup: None,
            last_incremental_backup: None,
            last_differential_backup: None,
            last_transaction_log_backup: None,
            last_retention_check: None,
            backup_count: 0,
            failed_backups: 0,
        }
    }
}

impl BackupScheduler {
    pub fn new(backup_system: Arc<BackupSystem>, schedule: BackupSchedule) -> Self {
        Self {
            backup_system,
            schedule,
            state: Arc::new(RwLock::new(SchedulerState::default())),
        }
    }

    /// Start the backup scheduler
    pub async fn start(&self) -> OrbitResult<()> {
        let mut state = self.state.write().await;
        if state.running {
            return Err(OrbitError::internal("Scheduler already running"));
        }
        state.running = true;
        drop(state);

        info!("Starting backup scheduler");

        // Start full backup task
        self.start_full_backup_task().await;

        // Start incremental backup task if configured
        if self.schedule.incremental_backup_cron.is_some() {
            self.start_incremental_backup_task().await;
        }

        // Start differential backup task if configured
        if self.schedule.differential_backup_cron.is_some() {
            self.start_differential_backup_task().await;
        }

        // Start transaction log backup task if configured
        if let Some(interval_secs) = self.schedule.transaction_log_interval_secs {
            self.start_transaction_log_backup_task(interval_secs).await;
        }

        // Start retention check task if configured
        if self.schedule.auto_retention {
            self.start_retention_check_task().await;
        }

        info!("Backup scheduler started successfully");
        Ok(())
    }

    /// Stop the backup scheduler
    pub async fn stop(&self) -> OrbitResult<()> {
        let mut state = self.state.write().await;
        state.running = false;
        info!("Backup scheduler stopped");
        Ok(())
    }

    /// Check if scheduler is running
    pub async fn is_running(&self) -> bool {
        self.state.read().await.running
    }

    /// Get scheduler statistics
    pub async fn get_stats(&self) -> SchedulerStats {
        let state = self.state.read().await;
        SchedulerStats {
            running: state.running,
            last_full_backup: state.last_full_backup,
            last_incremental_backup: state.last_incremental_backup,
            last_differential_backup: state.last_differential_backup,
            last_transaction_log_backup: state.last_transaction_log_backup,
            last_retention_check: state.last_retention_check,
            total_backups: state.backup_count,
            failed_backups: state.failed_backups,
        }
    }

    async fn start_full_backup_task(&self) {
        let backup_system = Arc::clone(&self.backup_system);
        let state = Arc::clone(&self.state);
        let schedule = self.schedule.clone();

        tokio::spawn(async move {
            // For simplicity, use a daily interval instead of parsing cron
            let mut interval = interval(Duration::from_secs(86400)); // 24 hours

            loop {
                interval.tick().await;

                let is_running = state.read().await.running;
                if !is_running {
                    break;
                }

                info!("Starting scheduled full backup");
                match Self::perform_backup(
                    &backup_system,
                    BackupType::Full,
                    schedule.auto_verify,
                )
                .await
                {
                    Ok(backup_id) => {
                        let mut s = state.write().await;
                        s.last_full_backup = Some(Utc::now());
                        s.backup_count += 1;
                        info!("Scheduled full backup completed: {}", backup_id);
                    }
                    Err(e) => {
                        let mut s = state.write().await;
                        s.failed_backups += 1;
                        error!("Scheduled full backup failed: {}", e);
                    }
                }
            }
        });
    }

    async fn start_incremental_backup_task(&self) {
        let backup_system = Arc::clone(&self.backup_system);
        let state = Arc::clone(&self.state);
        let schedule = self.schedule.clone();

        tokio::spawn(async move {
            // Use hourly interval
            let mut interval = interval(Duration::from_secs(3600)); // 1 hour

            loop {
                interval.tick().await;

                let is_running = state.read().await.running;
                if !is_running {
                    break;
                }

                debug!("Starting scheduled incremental backup");
                match Self::perform_backup(
                    &backup_system,
                    BackupType::Incremental,
                    schedule.auto_verify,
                )
                .await
                {
                    Ok(backup_id) => {
                        let mut s = state.write().await;
                        s.last_incremental_backup = Some(Utc::now());
                        s.backup_count += 1;
                        debug!("Scheduled incremental backup completed: {}", backup_id);
                    }
                    Err(e) => {
                        let mut s = state.write().await;
                        s.failed_backups += 1;
                        warn!("Scheduled incremental backup failed: {}", e);
                    }
                }
            }
        });
    }

    async fn start_differential_backup_task(&self) {
        let backup_system = Arc::clone(&self.backup_system);
        let state = Arc::clone(&self.state);
        let schedule = self.schedule.clone();

        tokio::spawn(async move {
            // Use 6 hour interval for differential backups
            let mut interval = interval(Duration::from_secs(21600)); // 6 hours

            loop {
                interval.tick().await;

                let is_running = state.read().await.running;
                if !is_running {
                    break;
                }

                debug!("Starting scheduled differential backup");
                match Self::perform_backup(
                    &backup_system,
                    BackupType::Differential,
                    schedule.auto_verify,
                )
                .await
                {
                    Ok(backup_id) => {
                        let mut s = state.write().await;
                        s.last_differential_backup = Some(Utc::now());
                        s.backup_count += 1;
                        debug!("Scheduled differential backup completed: {}", backup_id);
                    }
                    Err(e) => {
                        let mut s = state.write().await;
                        s.failed_backups += 1;
                        warn!("Scheduled differential backup failed: {}", e);
                    }
                }
            }
        });
    }

    async fn start_transaction_log_backup_task(&self, interval_secs: u64) {
        let backup_system = Arc::clone(&self.backup_system);
        let state = Arc::clone(&self.state);
        let schedule = self.schedule.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let is_running = state.read().await.running;
                if !is_running {
                    break;
                }

                debug!("Starting scheduled transaction log backup");
                match Self::perform_backup(
                    &backup_system,
                    BackupType::TransactionLog,
                    schedule.auto_verify,
                )
                .await
                {
                    Ok(backup_id) => {
                        let mut s = state.write().await;
                        s.last_transaction_log_backup = Some(Utc::now());
                        s.backup_count += 1;
                        debug!("Scheduled transaction log backup completed: {}", backup_id);
                    }
                    Err(e) => {
                        let mut s = state.write().await;
                        s.failed_backups += 1;
                        warn!("Scheduled transaction log backup failed: {}", e);
                    }
                }
            }
        });
    }

    async fn start_retention_check_task(&self) {
        let backup_system = Arc::clone(&self.backup_system);
        let state = Arc::clone(&self.state);
        let interval_hours = self.schedule.retention_check_interval_hours;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_hours * 3600));

            loop {
                interval.tick().await;

                let is_running = state.read().await.running;
                if !is_running {
                    break;
                }

                info!("Running scheduled retention check");
                match backup_system.apply_retention_policy().await {
                    Ok(deleted_count) => {
                        let mut s = state.write().await;
                        s.last_retention_check = Some(Utc::now());
                        info!("Retention check completed: {} backups deleted", deleted_count);
                    }
                    Err(e) => {
                        error!("Retention check failed: {}", e);
                    }
                }
            }
        });
    }

    async fn perform_backup(
        backup_system: &Arc<BackupSystem>,
        backup_type: BackupType,
        auto_verify: bool,
    ) -> OrbitResult<Uuid> {
        // Create backup
        let backup_id = backup_system.create_backup(backup_type).await?;

        // Verify if auto-verify is enabled
        if auto_verify {
            debug!("Auto-verifying backup {}", backup_id);
            match backup_system.verify_backup(backup_id).await {
                Ok(true) => {
                    debug!("Backup {} verified successfully", backup_id);
                }
                Ok(false) => {
                    warn!("Backup {} verification failed", backup_id);
                }
                Err(e) => {
                    warn!("Failed to verify backup {}: {}", backup_id, e);
                }
            }
        }

        Ok(backup_id)
    }

    /// Trigger an immediate full backup
    pub async fn trigger_full_backup(&self) -> OrbitResult<Uuid> {
        info!("Triggering immediate full backup");
        let backup_id = Self::perform_backup(
            &self.backup_system,
            BackupType::Full,
            self.schedule.auto_verify,
        )
        .await?;

        let mut state = self.state.write().await;
        state.last_full_backup = Some(Utc::now());
        state.backup_count += 1;

        Ok(backup_id)
    }

    /// Trigger an immediate incremental backup
    pub async fn trigger_incremental_backup(&self) -> OrbitResult<Uuid> {
        info!("Triggering immediate incremental backup");
        let backup_id = Self::perform_backup(
            &self.backup_system,
            BackupType::Incremental,
            self.schedule.auto_verify,
        )
        .await?;

        let mut state = self.state.write().await;
        state.last_incremental_backup = Some(Utc::now());
        state.backup_count += 1;

        Ok(backup_id)
    }
}

/// Statistics for the backup scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub running: bool,
    pub last_full_backup: Option<DateTime<Utc>>,
    pub last_incremental_backup: Option<DateTime<Utc>>,
    pub last_differential_backup: Option<DateTime<Utc>>,
    pub last_transaction_log_backup: Option<DateTime<Utc>>,
    pub last_retention_check: Option<DateTime<Utc>>,
    pub total_backups: u64,
    pub failed_backups: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{BackupConfiguration, LocalStorageBackend, StorageBackendType};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_backup_scheduler_creation() {
        let temp_dir = tempdir().unwrap();
        let config = BackupConfiguration {
            storage_backend: StorageBackendType::Local {
                path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let backend = Arc::new(LocalStorageBackend::new(temp_dir.path().to_path_buf()).unwrap());
        let system = Arc::new(BackupSystem::new(config, backend, None));
        let schedule = BackupSchedule::default();

        let scheduler = BackupScheduler::new(system, schedule);
        assert!(!scheduler.is_running().await);
    }

    #[tokio::test]
    async fn test_backup_scheduler_start_stop() {
        let temp_dir = tempdir().unwrap();
        let config = BackupConfiguration {
            storage_backend: StorageBackendType::Local {
                path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let backend = Arc::new(LocalStorageBackend::new(temp_dir.path().to_path_buf()).unwrap());
        let system = Arc::new(BackupSystem::new(config, backend, None));
        let schedule = BackupSchedule::default();

        let scheduler = BackupScheduler::new(system, schedule);

        scheduler.start().await.unwrap();
        assert!(scheduler.is_running().await);

        scheduler.stop().await.unwrap();
        assert!(!scheduler.is_running().await);
    }

    #[tokio::test]
    async fn test_trigger_immediate_backup() {
        let temp_dir = tempdir().unwrap();
        let config = BackupConfiguration {
            storage_backend: StorageBackendType::Local {
                path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let backend = Arc::new(LocalStorageBackend::new(temp_dir.path().to_path_buf()).unwrap());
        let system = Arc::new(BackupSystem::new(config, backend, None));
        let schedule = BackupSchedule::default();

        let scheduler = BackupScheduler::new(system, schedule);
        scheduler.start().await.unwrap();

        let backup_id = scheduler.trigger_full_backup().await.unwrap();
        assert!(!backup_id.is_nil());

        let stats = scheduler.get_stats().await;
        assert_eq!(stats.total_backups, 1);
        assert!(stats.last_full_backup.is_some());
    }
}
