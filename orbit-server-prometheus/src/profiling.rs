//! Performance profiling and analysis tools
//!
//! This module provides tools for profiling query performance, identifying bottlenecks,
//! and generating optimization recommendations.

use crate::{PrometheusError, PrometheusResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Performance profiler for query and system analysis
pub struct PerformanceProfiler {
    /// Query execution profiles
    query_profiles: Arc<RwLock<VecDeque<QueryProfile>>>,
    /// System resource profiles
    resource_profiles: Arc<RwLock<VecDeque<ResourceProfile>>>,
    /// Profiling configuration
    config: ProfilerConfig,
}

/// Profiler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    /// Maximum profiles to retain
    pub max_profiles: usize,
    /// Enable detailed profiling
    pub detailed_profiling: bool,
    /// Sample rate (0.0 to 1.0)
    pub sample_rate: f64,
    /// Minimum duration to profile (in milliseconds)
    pub min_duration_ms: u64,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            max_profiles: 1000,
            detailed_profiling: false,
            sample_rate: 1.0,
            min_duration_ms: 10,
        }
    }
}

/// Query execution profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryProfile {
    /// Query identifier
    pub query_id: String,
    /// Query text
    pub query: String,
    /// Execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Total execution time
    pub execution_time: Duration,
    /// Execution stages
    pub stages: Vec<ExecutionStage>,
    /// Resource usage
    pub resource_usage: ResourceUsage,
    /// Optimization suggestions
    pub suggestions: Vec<OptimizationSuggestion>,
}

/// Execution stage timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStage {
    /// Stage name
    pub name: String,
    /// Stage duration
    pub duration: Duration,
    /// Percentage of total time
    pub percentage: f64,
    /// Operations performed
    pub operations: u64,
    /// Details
    pub details: HashMap<String, String>,
}

/// Resource usage during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Memory allocated in bytes
    pub memory_bytes: u64,
    /// Disk reads
    pub disk_reads: u64,
    /// Disk writes
    pub disk_writes: u64,
    /// Network bytes sent
    pub network_sent_bytes: u64,
    /// Network bytes received
    pub network_received_bytes: u64,
}

/// System resource profile snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceProfile {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Available memory in bytes
    pub available_memory_bytes: u64,
    /// Disk I/O rate
    pub disk_io_rate: DiskIORate,
    /// Network I/O rate
    pub network_io_rate: NetworkIORate,
    /// Active connections
    pub active_connections: u64,
}

/// Disk I/O rate metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIORate {
    /// Reads per second
    pub reads_per_sec: f64,
    /// Writes per second
    pub writes_per_sec: f64,
    /// Read bytes per second
    pub read_bytes_per_sec: f64,
    /// Write bytes per second
    pub write_bytes_per_sec: f64,
}

/// Network I/O rate metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIORate {
    /// Packets received per second
    pub packets_in_per_sec: f64,
    /// Packets sent per second
    pub packets_out_per_sec: f64,
    /// Bytes received per second
    pub bytes_in_per_sec: f64,
    /// Bytes sent per second
    pub bytes_out_per_sec: f64,
}

/// Optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Suggestion type
    pub suggestion_type: SuggestionType,
    /// Severity
    pub severity: SuggestionSeverity,
    /// Description
    pub description: String,
    /// Estimated impact
    pub estimated_impact: String,
    /// Recommendation
    pub recommendation: String,
}

/// Types of optimization suggestions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuggestionType {
    /// Add an index
    AddIndex,
    /// Rewrite query
    RewriteQuery,
    /// Partition data
    PartitionData,
    /// Add caching
    AddCaching,
    /// Optimize schema
    OptimizeSchema,
    /// Increase resources
    IncreaseResources,
    /// Use connection pooling
    UseConnectionPooling,
    /// General optimization
    General,
}

/// Severity of optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum SuggestionSeverity {
    /// Low impact
    Low,
    /// Medium impact
    Medium,
    /// High impact
    High,
    /// Critical issue
    Critical,
}

impl PerformanceProfiler {
    /// Create a new performance profiler
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            query_profiles: Arc::new(RwLock::new(VecDeque::new())),
            resource_profiles: Arc::new(RwLock::new(VecDeque::new())),
            config,
        }
    }

    /// Record a query profile
    pub async fn record_query_profile(&self, profile: QueryProfile) -> PrometheusResult<()> {
        let mut profiles = self.query_profiles.write().await;
        
        // Check if we should sample this query
        if profile.execution_time.as_millis() as u64 >= self.config.min_duration_ms {
            profiles.push_back(profile);
            
            // Trim to max size
            while profiles.len() > self.config.max_profiles {
                profiles.pop_front();
            }
        }

        Ok(())
    }

    /// Record a resource profile snapshot
    pub async fn record_resource_profile(&self, profile: ResourceProfile) -> PrometheusResult<()> {
        let mut profiles = self.resource_profiles.write().await;
        
        profiles.push_back(profile);
        
        // Trim to max size
        while profiles.len() > self.config.max_profiles {
            profiles.pop_front();
        }

        Ok(())
    }

    /// Get query profiles
    pub async fn get_query_profiles(
        &self,
        limit: Option<usize>,
    ) -> PrometheusResult<Vec<QueryProfile>> {
        let profiles = self.query_profiles.read().await;
        
        let result: Vec<_> = if let Some(limit) = limit {
            profiles.iter().rev().take(limit).cloned().collect()
        } else {
            profiles.iter().rev().cloned().collect()
        };

        Ok(result)
    }

    /// Get slowest queries
    pub async fn get_slowest_queries(&self, limit: usize) -> PrometheusResult<Vec<QueryProfile>> {
        let profiles = self.query_profiles.read().await;
        
        let mut sorted: Vec<_> = profiles.iter().cloned().collect();
        sorted.sort_by(|a, b| b.execution_time.cmp(&a.execution_time));
        
        Ok(sorted.into_iter().take(limit).collect())
    }

    /// Get resource profiles
    pub async fn get_resource_profiles(
        &self,
        limit: Option<usize>,
    ) -> PrometheusResult<Vec<ResourceProfile>> {
        let profiles = self.resource_profiles.read().await;
        
        let result: Vec<_> = if let Some(limit) = limit {
            profiles.iter().rev().take(limit).cloned().collect()
        } else {
            profiles.iter().rev().cloned().collect()
        };

        Ok(result)
    }

    /// Analyze query patterns and generate report
    pub async fn analyze_query_patterns(&self) -> PrometheusResult<QueryAnalysisReport> {
        let profiles = self.query_profiles.read().await;
        
        if profiles.is_empty() {
            return Ok(QueryAnalysisReport {
                total_queries: 0,
                avg_execution_time: Duration::from_secs(0),
                median_execution_time: Duration::from_secs(0),
                p95_execution_time: Duration::from_secs(0),
                p99_execution_time: Duration::from_secs(0),
                slowest_queries: Vec::new(),
                common_patterns: Vec::new(),
                optimization_recommendations: Vec::new(),
            });
        }

        let total_queries = profiles.len();
        
        // Calculate average
        let total_time: Duration = profiles.iter().map(|p| p.execution_time).sum();
        let avg_execution_time = total_time / total_queries as u32;

        // Calculate percentiles
        let mut sorted_durations: Vec<_> = profiles.iter().map(|p| p.execution_time).collect();
        sorted_durations.sort();
        
        let median_execution_time = sorted_durations[sorted_durations.len() / 2];
        let p95_execution_time = sorted_durations[(sorted_durations.len() as f64 * 0.95) as usize];
        let p99_execution_time = sorted_durations[(sorted_durations.len() as f64 * 0.99) as usize];

        // Get slowest queries
        let mut slowest: Vec<_> = profiles.iter().cloned().collect();
        slowest.sort_by(|a, b| b.execution_time.cmp(&a.execution_time));
        let slowest_queries: Vec<_> = slowest.into_iter().take(10).collect();

        // Generate recommendations
        let optimization_recommendations = self.generate_recommendations(&profiles);

        Ok(QueryAnalysisReport {
            total_queries,
            avg_execution_time,
            median_execution_time,
            p95_execution_time,
            p99_execution_time,
            slowest_queries,
            common_patterns: Vec::new(), // Would analyze patterns in production
            optimization_recommendations,
        })
    }

    /// Generate optimization recommendations
    fn generate_recommendations(&self, profiles: &VecDeque<QueryProfile>) -> Vec<OptimizationSuggestion> {
        let mut recommendations = Vec::new();

        // Check for slow queries
        let slow_query_count = profiles.iter()
            .filter(|p| p.execution_time.as_millis() > 1000)
            .count();

        if slow_query_count > profiles.len() / 10 {
            recommendations.push(OptimizationSuggestion {
                suggestion_type: SuggestionType::AddIndex,
                severity: SuggestionSeverity::High,
                description: "High number of slow queries detected".to_string(),
                estimated_impact: "Could reduce query time by 50-90%".to_string(),
                recommendation: "Consider adding indexes on frequently queried columns".to_string(),
            });
        }

        // Check for high resource usage
        let high_memory_count = profiles.iter()
            .filter(|p| p.resource_usage.memory_bytes > 100 * 1024 * 1024) // > 100MB
            .count();

        if high_memory_count > 0 {
            recommendations.push(OptimizationSuggestion {
                suggestion_type: SuggestionType::OptimizeSchema,
                severity: SuggestionSeverity::Medium,
                description: "Queries consuming excessive memory".to_string(),
                estimated_impact: "Could reduce memory usage by 30-50%".to_string(),
                recommendation: "Review query patterns and consider result set limiting".to_string(),
            });
        }

        recommendations
    }

    /// Clear all profiles
    pub async fn clear_profiles(&self) -> PrometheusResult<()> {
        let mut query_profiles = self.query_profiles.write().await;
        let mut resource_profiles = self.resource_profiles.write().await;
        
        query_profiles.clear();
        resource_profiles.clear();
        
        Ok(())
    }
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new(ProfilerConfig::default())
    }
}

/// Query analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysisReport {
    /// Total number of queries analyzed
    pub total_queries: usize,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Median execution time
    pub median_execution_time: Duration,
    /// 95th percentile execution time
    pub p95_execution_time: Duration,
    /// 99th percentile execution time
    pub p99_execution_time: Duration,
    /// Slowest queries
    pub slowest_queries: Vec<QueryProfile>,
    /// Common query patterns
    pub common_patterns: Vec<String>,
    /// Optimization recommendations
    pub optimization_recommendations: Vec<OptimizationSuggestion>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_profiler_creation() {
        let profiler = PerformanceProfiler::default();
        
        let profiles = profiler.get_query_profiles(None).await.unwrap();
        assert_eq!(profiles.len(), 0);
    }

    #[tokio::test]
    async fn test_record_query_profile() {
        let profiler = PerformanceProfiler::default();
        
        let profile = QueryProfile {
            query_id: "test-1".to_string(),
            query: "SELECT * FROM users".to_string(),
            timestamp: Utc::now(),
            execution_time: Duration::from_millis(50),
            stages: vec![],
            resource_usage: ResourceUsage {
                cpu_time_ms: 20,
                memory_bytes: 1024,
                disk_reads: 0,
                disk_writes: 0,
                network_sent_bytes: 0,
                network_received_bytes: 0,
            },
            suggestions: vec![],
        };

        profiler.record_query_profile(profile).await.unwrap();
        
        let profiles = profiler.get_query_profiles(None).await.unwrap();
        assert_eq!(profiles.len(), 1);
    }

    #[tokio::test]
    async fn test_slowest_queries() {
        let profiler = PerformanceProfiler::default();
        
        // Add multiple queries with different durations
        for i in 1..=5 {
            let profile = QueryProfile {
                query_id: format!("test-{}", i),
                query: format!("SELECT * FROM table{}", i),
                timestamp: Utc::now(),
                execution_time: Duration::from_millis(i * 10),
                stages: vec![],
                resource_usage: ResourceUsage {
                    cpu_time_ms: i * 10,
                    memory_bytes: 1024,
                    disk_reads: 0,
                    disk_writes: 0,
                    network_sent_bytes: 0,
                    network_received_bytes: 0,
                },
                suggestions: vec![],
            };
            profiler.record_query_profile(profile).await.unwrap();
        }

        let slowest = profiler.get_slowest_queries(3).await.unwrap();
        assert_eq!(slowest.len(), 3);
        assert_eq!(slowest[0].query_id, "test-5");
    }
}
