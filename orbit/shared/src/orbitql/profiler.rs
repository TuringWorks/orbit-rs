//! Query performance profiler and EXPLAIN ANALYZE for OrbitQL
//!
//! This module provides detailed query execution analysis, timing breakdowns,
//! resource usage tracking, and optimization suggestions to help developers
//! understand and optimize their OrbitQL queries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Comprehensive query execution profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryProfile {
    /// Unique profile ID
    pub profile_id: Uuid,
    /// Original query text
    pub query: String,
    /// Query parameters used
    pub parameters: HashMap<String, String>, // Serialized query values
    /// Execution phases breakdown
    pub phases: Vec<ExecutionPhase>,
    /// Overall execution statistics
    pub overall_stats: ExecutionStats,
    /// Resource usage during execution
    pub resource_usage: ResourceUsage,
    /// Optimization information
    pub optimization_info: OptimizationInfo,
    /// Performance bottlenecks identified
    pub bottlenecks: Vec<PerformanceBottleneck>,
    /// Optimization suggestions
    pub suggestions: Vec<OptimizationSuggestion>,
    /// Distributed execution details
    pub distributed_info: Option<DistributedExecutionInfo>,
}

/// Individual execution phase timing and details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    /// Phase name (e.g., "Parsing", "Planning", "Execution")
    pub name: String,
    /// Phase description
    pub description: String,
    /// Phase start time relative to query start
    pub start_offset: Duration,
    /// Phase duration
    pub duration: Duration,
    /// Percentage of total execution time
    pub percentage: f64,
    /// Phase-specific details
    pub details: PhaseDetails,
    /// Sub-phases for nested operations
    pub sub_phases: Vec<ExecutionPhase>,
}

/// Phase-specific execution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseDetails {
    Parsing {
        tokens_processed: usize,
        ast_nodes: usize,
        syntax_errors: usize,
    },
    Planning {
        plan_alternatives: usize,
        selected_plan_cost: f64,
        optimization_rules_applied: usize,
    },
    Optimization {
        initial_cost: f64,
        final_cost: f64,
        rules_applied: Vec<String>,
        transformations: usize,
    },
    Execution {
        plan_node_type: String,
        rows_processed: u64,
        bytes_processed: u64,
        network_calls: u32,
        cache_hits: u32,
        cache_misses: u32,
    },
    DataTransfer {
        source_actor: String,
        target_actor: String,
        bytes_transferred: u64,
        compression_ratio: f64,
        network_latency: Duration,
    },
    Aggregation {
        input_rows: u64,
        output_rows: u64,
        group_count: usize,
        hash_table_size: usize,
        spill_to_disk: bool,
    },
}

/// Overall execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total execution time
    pub total_duration: Duration,
    /// Total rows returned
    pub rows_returned: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Peak memory usage
    pub peak_memory_mb: f64,
    /// CPU time used
    pub cpu_time: Duration,
    /// I/O operations performed
    pub io_operations: u64,
    /// Network round trips
    pub network_round_trips: u32,
    /// Cache hit ratio
    pub cache_hit_ratio: f64,
}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Memory usage over time
    pub memory_timeline: Vec<MemorySnapshot>,
    /// CPU usage per core
    pub cpu_usage: Vec<f64>,
    /// Disk I/O statistics
    pub disk_io: DiskIOStats,
    /// Network I/O statistics
    pub network_io: NetworkIOStats,
    /// Actor resource usage (for distributed queries)
    pub actor_usage: HashMap<String, ActorResourceUsage>,
}

/// Memory usage snapshot at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Time offset from query start
    pub timestamp: Duration,
    /// Memory used in MB
    pub memory_mb: f64,
    /// Memory category (heap, stack, cache, etc.)
    pub category: String,
}

/// Disk I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIOStats {
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Number of read operations
    pub read_operations: u64,
    /// Number of write operations
    pub write_operations: u64,
    /// Average read latency
    pub avg_read_latency: Duration,
    /// Average write latency
    pub avg_write_latency: Duration,
}

/// Network I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIOStats {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of connections opened
    pub connections_opened: u32,
    /// Number of requests sent
    pub requests_sent: u32,
    /// Average network latency
    pub avg_latency: Duration,
}

/// Actor-specific resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorResourceUsage {
    /// Actor identifier
    pub actor_id: String,
    /// CPU time used by this actor
    pub cpu_time: Duration,
    /// Memory used by this actor
    pub memory_mb: f64,
    /// Network bytes sent/received
    pub network_bytes: u64,
    /// Number of operations performed
    pub operations: u64,
}

/// Optimization information and decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationInfo {
    /// Original query cost estimate
    pub original_cost: f64,
    /// Final optimized cost estimate
    pub optimized_cost: f64,
    /// Cost improvement percentage
    pub cost_improvement: f64,
    /// Optimization rules that were applied
    pub applied_rules: Vec<String>,
    /// Optimization rules that were considered but not applied
    pub considered_rules: Vec<String>,
    /// Reasons rules were not applied
    pub rule_skip_reasons: HashMap<String, String>,
    /// Index usage analysis
    pub index_usage: Vec<IndexUsageInfo>,
    /// Join strategy decisions
    pub join_strategies: Vec<JoinStrategyInfo>,
}

/// Index usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUsageInfo {
    /// Index name
    pub index_name: String,
    /// Table/collection name
    pub table_name: String,
    /// Whether the index was used
    pub used: bool,
    /// Reason for not using (if applicable)
    pub skip_reason: Option<String>,
    /// Selectivity of the index for this query
    pub selectivity: f64,
    /// Estimated cost of using this index
    pub estimated_cost: f64,
}

/// Join strategy decision information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinStrategyInfo {
    /// Left table/collection
    pub left_relation: String,
    /// Right table/collection
    pub right_relation: String,
    /// Chosen join strategy
    pub chosen_strategy: String,
    /// Alternative strategies considered
    pub alternatives: Vec<String>,
    /// Reason for choosing this strategy
    pub reason: String,
    /// Estimated cost of chosen strategy
    pub estimated_cost: f64,
}

/// Performance bottleneck identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBottleneck {
    /// Bottleneck type
    pub bottleneck_type: BottleneckType,
    /// Severity (1-10, 10 being most severe)
    pub severity: u8,
    /// Description of the bottleneck
    pub description: String,
    /// Which part of the query is affected
    pub affected_operation: String,
    /// Time spent on this bottleneck
    pub time_impact: Duration,
    /// Percentage of total execution time
    pub percentage_impact: f64,
}

/// Types of performance bottlenecks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    /// CPU-bound operation
    CPU,
    /// Memory exhaustion or inefficient usage
    Memory,
    /// Disk I/O bottleneck
    DiskIO,
    /// Network latency or bandwidth
    Network,
    /// Lock contention or synchronization
    Synchronization,
    /// Inefficient query plan
    QueryPlan,
    /// Missing or suboptimal indexes
    IndexUsage,
    /// Data skew in distributed operations
    DataSkew,
    /// Cross-model join inefficiency
    CrossModelJoin,
}

/// Optimization suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Suggestion category
    pub category: SuggestionCategory,
    /// Priority (1-10, 10 being highest)
    pub priority: u8,
    /// Short suggestion title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Estimated performance improvement
    pub estimated_improvement: String,
    /// Specific actions to take
    pub actions: Vec<String>,
    /// SQL/OrbitQL code examples if applicable
    pub code_examples: Vec<String>,
}

/// Categories of optimization suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionCategory {
    /// Query rewriting suggestions
    QueryRewrite,
    /// Index recommendations
    IndexOptimization,
    /// Schema design improvements
    SchemaDesign,
    /// Configuration tuning
    Configuration,
    /// Hardware recommendations
    Hardware,
    /// Multi-model specific optimizations
    MultiModel,
    /// Distributed execution optimizations
    Distributed,
}

/// Distributed execution profiling information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedExecutionInfo {
    /// Participating actor nodes
    pub participating_actors: Vec<String>,
    /// Data locality score (0.0 to 1.0)
    pub data_locality_score: f64,
    /// Network data transfer breakdown
    pub data_transfers: Vec<DataTransferInfo>,
    /// Parallel execution efficiency
    pub parallelism_efficiency: f64,
    /// Load balancing effectiveness
    pub load_balance_score: f64,
    /// Bottleneck actors
    pub bottleneck_actors: Vec<String>,
}

/// Data transfer information for distributed queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransferInfo {
    /// Source actor
    pub source: String,
    /// Destination actor
    pub destination: String,
    /// Bytes transferred
    pub bytes: u64,
    /// Transfer duration
    pub duration: Duration,
    /// Compression used
    pub compressed: bool,
    /// Transfer efficiency (actual vs theoretical max)
    pub efficiency: f64,
}

/// Main query profiler
pub struct QueryProfiler {
    /// Whether profiling is enabled
    enabled: bool,
    /// Profiling configuration
    #[allow(dead_code)]
    config: ProfilerConfig,
    /// Active profiling sessions
    active_sessions: HashMap<Uuid, ProfileSession>,
}

/// Profiler configuration
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// Enable memory tracking
    pub track_memory: bool,
    /// Enable CPU tracking
    pub track_cpu: bool,
    /// Enable network tracking
    pub track_network: bool,
    /// Memory sampling interval
    pub memory_sample_interval: Duration,
    /// Maximum profile retention time
    pub max_profile_age: Duration,
    /// Detailed timing for each operation
    pub detailed_timing: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            track_memory: true,
            track_cpu: true,
            track_network: true,
            memory_sample_interval: Duration::from_millis(100),
            max_profile_age: Duration::from_secs(86400), // 24 hours
            detailed_timing: true,
        }
    }
}

/// Active profiling session
struct ProfileSession {
    #[allow(dead_code)]
    profile_id: Uuid,
    query: String,
    start_time: Instant,
    phases: Vec<ExecutionPhase>,
    current_phase: Option<String>,
    current_phase_start: Option<Instant>,
    current_phase_description: Option<String>,
    resource_tracker: ResourceTracker,
    bottleneck_detector: BottleneckDetector,
}

/// Resource tracking during execution
struct ResourceTracker {
    memory_samples: Vec<MemorySnapshot>,
    #[allow(dead_code)]
    start_memory: f64,
    peak_memory: f64,
    io_stats: DiskIOStats,
    network_stats: NetworkIOStats,
}

/// Bottleneck detection during execution
struct BottleneckDetector {
    cpu_usage_samples: Vec<f64>,
    memory_pressure_events: u32,
    io_wait_time: Duration,
    #[allow(dead_code)]
    network_timeouts: u32,
}

impl QueryProfiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            enabled: true,
            config,
            active_sessions: HashMap::new(),
        }
    }

    /// Start profiling a query
    pub fn start_profiling(&mut self, query: String) -> Uuid {
        let profile_id = Uuid::new_v4();
        let session = ProfileSession {
            profile_id,
            query: query.clone(),
            start_time: Instant::now(),
            phases: Vec::new(),
            current_phase: None,
            current_phase_start: None,
            current_phase_description: None,
            resource_tracker: ResourceTracker {
                memory_samples: Vec::new(),
                start_memory: self.get_current_memory_usage(),
                peak_memory: 0.0,
                io_stats: DiskIOStats {
                    bytes_read: 0,
                    bytes_written: 0,
                    read_operations: 0,
                    write_operations: 0,
                    avg_read_latency: Duration::ZERO,
                    avg_write_latency: Duration::ZERO,
                },
                network_stats: NetworkIOStats {
                    bytes_sent: 0,
                    bytes_received: 0,
                    connections_opened: 0,
                    requests_sent: 0,
                    avg_latency: Duration::ZERO,
                },
            },
            bottleneck_detector: BottleneckDetector {
                cpu_usage_samples: Vec::new(),
                memory_pressure_events: 0,
                io_wait_time: Duration::ZERO,
                network_timeouts: 0,
            },
        };

        self.active_sessions.insert(profile_id, session);
        profile_id
    }

    /// Mark the start of a new execution phase
    pub fn start_phase(&mut self, profile_id: Uuid, phase_name: &str, description: &str) {
        if let Some(session) = self.active_sessions.get_mut(&profile_id) {
            let now = Instant::now();

            session.current_phase = Some(phase_name.to_string());
            session.current_phase_start = Some(now);
            session.current_phase_description = Some(description.to_string());
        }
    }

    /// Mark the end of the current execution phase
    pub fn end_phase(&mut self, profile_id: Uuid) {
        if let Some(session) = self.active_sessions.get_mut(&profile_id) {
            if let (Some(phase_name), Some(phase_start), Some(description)) = (
                session.current_phase.take(),
                session.current_phase_start.take(),
                session.current_phase_description.take(),
            ) {
                let now = Instant::now();
                let duration = now.duration_since(phase_start);
                let start_offset = phase_start.duration_since(session.start_time);

                // Create the execution phase
                let phase = ExecutionPhase {
                    name: phase_name.clone(),
                    description,
                    start_offset,
                    duration,
                    percentage: 0.0, // Will be calculated at finish
                    details: match phase_name.as_str() {
                        "Lexing" | "Parsing" => PhaseDetails::Parsing {
                            tokens_processed: 0,
                            ast_nodes: 0,
                            syntax_errors: 0,
                        },
                        "Planning" => PhaseDetails::Planning {
                            plan_alternatives: 1,
                            selected_plan_cost: 1.0,
                            optimization_rules_applied: 0,
                        },
                        "Optimization" => PhaseDetails::Optimization {
                            initial_cost: 1.0,
                            final_cost: 1.0,
                            rules_applied: Vec::new(),
                            transformations: 0,
                        },
                        _ => PhaseDetails::Execution {
                            plan_node_type: "TableScan".to_string(),
                            rows_processed: 0,
                            bytes_processed: 0,
                            network_calls: 0,
                            cache_hits: 0,
                            cache_misses: 0,
                        },
                    },
                    sub_phases: Vec::new(),
                };

                session.phases.push(phase);
            }
        }
    }

    /// Record execution statistics for a plan node
    pub fn record_plan_node_execution(
        &mut self,
        profile_id: Uuid,
        _node_type: &str,
        _rows_processed: u64,
        _duration: Duration,
    ) {
        if let Some(_session) = self.active_sessions.get_mut(&profile_id) {
            // TODO: Record detailed plan node execution info
        }
    }

    /// Finish profiling and generate comprehensive profile
    pub fn finish_profiling(&mut self, profile_id: Uuid) -> Option<QueryProfile> {
        if let Some(session) = self.active_sessions.remove(&profile_id) {
            let total_duration = session.start_time.elapsed();

            // Generate suggestions first to avoid borrowing issues
            let suggestions = self.generate_suggestions(&session);

            // Generate comprehensive profile
            let profile = QueryProfile {
                profile_id,
                query: session.query.clone(),
                parameters: HashMap::new(), // TODO: Capture actual parameters
                phases: session.phases,
                overall_stats: ExecutionStats {
                    total_duration,
                    rows_returned: 0, // TODO: Track actual rows
                    bytes_processed: 0,
                    peak_memory_mb: session.resource_tracker.peak_memory,
                    cpu_time: Duration::ZERO, // TODO: Calculate actual CPU time
                    io_operations: session.resource_tracker.io_stats.read_operations
                        + session.resource_tracker.io_stats.write_operations,
                    network_round_trips: session.resource_tracker.network_stats.requests_sent,
                    cache_hit_ratio: 0.0, // TODO: Calculate cache hit ratio
                },
                resource_usage: self.build_resource_usage(&session.resource_tracker),
                optimization_info: OptimizationInfo {
                    original_cost: 0.0,
                    optimized_cost: 0.0,
                    cost_improvement: 0.0,
                    applied_rules: Vec::new(),
                    considered_rules: Vec::new(),
                    rule_skip_reasons: HashMap::new(),
                    index_usage: Vec::new(),
                    join_strategies: Vec::new(),
                },
                bottlenecks: self.detect_bottlenecks(&session.bottleneck_detector, total_duration),
                suggestions,
                distributed_info: None, // TODO: Add distributed execution info if applicable
            };

            Some(profile)
        } else {
            None
        }
    }

    /// Get current memory usage in MB
    fn get_current_memory_usage(&self) -> f64 {
        // TODO: Implement actual memory usage detection
        // This would use system APIs to get process memory usage
        100.0 // Placeholder
    }

    /// Build resource usage summary from tracker
    fn build_resource_usage(&self, tracker: &ResourceTracker) -> ResourceUsage {
        ResourceUsage {
            memory_timeline: tracker.memory_samples.clone(),
            cpu_usage: vec![50.0, 60.0, 45.0, 70.0], // TODO: Actual CPU tracking
            disk_io: tracker.io_stats.clone(),
            network_io: tracker.network_stats.clone(),
            actor_usage: HashMap::new(), // TODO: Track actor-specific usage
        }
    }

    /// Detect performance bottlenecks
    fn detect_bottlenecks(
        &self,
        detector: &BottleneckDetector,
        total_duration: Duration,
    ) -> Vec<PerformanceBottleneck> {
        let mut bottlenecks = Vec::new();

        // Check for CPU bottlenecks
        let avg_cpu = detector.cpu_usage_samples.iter().sum::<f64>()
            / detector.cpu_usage_samples.len() as f64;
        if avg_cpu > 80.0 {
            bottlenecks.push(PerformanceBottleneck {
                bottleneck_type: BottleneckType::CPU,
                severity: 8,
                description: format!("High CPU usage detected (avg: {avg_cpu:.1}%)"),
                affected_operation: "Query execution".to_string(),
                time_impact: Duration::from_millis(
                    (total_duration.as_millis() as f64 * 0.6) as u64,
                ),
                percentage_impact: 60.0,
            });
        }

        // Check for memory pressure
        if detector.memory_pressure_events > 0 {
            bottlenecks.push(PerformanceBottleneck {
                bottleneck_type: BottleneckType::Memory,
                severity: 7,
                description: format!(
                    "{} memory pressure events detected",
                    detector.memory_pressure_events
                ),
                affected_operation: "Memory allocation".to_string(),
                time_impact: Duration::from_millis(100),
                percentage_impact: 10.0,
            });
        }

        // Check for I/O wait
        if detector.io_wait_time > Duration::from_millis(100) {
            let percentage = (detector.io_wait_time.as_millis() as f64
                / total_duration.as_millis() as f64)
                * 100.0;
            bottlenecks.push(PerformanceBottleneck {
                bottleneck_type: BottleneckType::DiskIO,
                severity: 6,
                description: format!("I/O wait time: {:.2}s", detector.io_wait_time.as_secs_f64()),
                affected_operation: "Data access".to_string(),
                time_impact: detector.io_wait_time,
                percentage_impact: percentage,
            });
        }

        bottlenecks
    }

    /// Generate optimization suggestions
    fn generate_suggestions(&self, session: &ProfileSession) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        // Basic query optimization suggestion
        suggestions.push(OptimizationSuggestion {
            category: SuggestionCategory::QueryRewrite,
            priority: 7,
            title: "Consider adding WHERE clause filters early".to_string(),
            description: "Applying filters early in the query can reduce the amount of data processed by subsequent operations.".to_string(),
            estimated_improvement: "20-50% reduction in execution time".to_string(),
            actions: vec![
                "Move WHERE conditions closer to table scans".to_string(),
                "Use selective predicates to filter data early".to_string(),
            ],
            code_examples: vec![
                "-- Instead of:\nSELECT * FROM large_table ORDER BY date\n-- Use:\nSELECT * FROM large_table WHERE date > '2024-01-01' ORDER BY date".to_string(),
            ],
        });

        // Index suggestion if high I/O detected
        if session.resource_tracker.io_stats.read_operations > 1000 {
            suggestions.push(OptimizationSuggestion {
                category: SuggestionCategory::IndexOptimization,
                priority: 9,
                title: "Consider adding indexes to reduce I/O operations".to_string(),
                description: "High number of read operations detected. Adding appropriate indexes could significantly improve performance.".to_string(),
                estimated_improvement: "50-90% reduction in I/O operations".to_string(),
                actions: vec![
                    "Analyze frequently filtered columns".to_string(),
                    "Create composite indexes for multi-column filters".to_string(),
                    "Consider partial indexes for selective conditions".to_string(),
                ],
                code_examples: vec![
                    "CREATE INDEX idx_user_active_date ON users (active, created_date) WHERE active = true;".to_string(),
                ],
            });
        }

        suggestions
    }

    /// Generate an EXPLAIN ANALYZE report
    pub fn explain_analyze(&self, profile: &QueryProfile) -> String {
        let mut report = String::new();

        report.push_str(&format!("Query Profile ID: {}\n", profile.profile_id));
        report.push_str(&format!("Query: {}\n", profile.query));
        report.push_str(&format!(
            "Total execution time: {:.2}s\n",
            profile.overall_stats.total_duration.as_secs_f64()
        ));
        report.push_str(&format!(
            "Rows returned: {}\n",
            profile.overall_stats.rows_returned
        ));
        report.push_str(&format!(
            "Peak memory usage: {:.1} MB\n",
            profile.overall_stats.peak_memory_mb
        ));
        report.push_str(&format!(
            "Cache hit ratio: {:.1}%\n",
            profile.overall_stats.cache_hit_ratio * 100.0
        ));
        report.push('\n');

        // Execution phases breakdown
        report.push_str("Execution Phases:\n");
        for phase in &profile.phases {
            report.push_str(&format!(
                "  {}: {:.2}s ({:.1}%)\n",
                phase.name,
                phase.duration.as_secs_f64(),
                phase.percentage
            ));
        }
        report.push('\n');

        // Bottlenecks
        if !profile.bottlenecks.is_empty() {
            report.push_str("Performance Bottlenecks:\n");
            for bottleneck in &profile.bottlenecks {
                report.push_str(&format!(
                    "  {:?} (Severity {}): {}\n",
                    bottleneck.bottleneck_type, bottleneck.severity, bottleneck.description
                ));
            }
            report.push('\n');
        }

        // Optimization suggestions
        if !profile.suggestions.is_empty() {
            report.push_str("Optimization Suggestions:\n");
            for suggestion in &profile.suggestions {
                report.push_str(&format!(
                    "  {} (Priority {}): {}\n",
                    suggestion.title, suggestion.priority, suggestion.estimated_improvement
                ));
            }
        }

        report
    }

    /// Enable or disable profiling
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_creation() {
        let config = ProfilerConfig::default();
        let profiler = QueryProfiler::new(config);
        assert!(profiler.is_enabled());
    }

    #[test]
    fn test_profile_session_lifecycle() {
        let config = ProfilerConfig::default();
        let mut profiler = QueryProfiler::new(config);

        let profile_id = profiler.start_profiling("SELECT * FROM test".to_string());
        assert!(profiler.active_sessions.contains_key(&profile_id));

        profiler.start_phase(profile_id, "Planning", "Query planning phase");
        profiler.end_phase(profile_id);

        let profile = profiler.finish_profiling(profile_id);
        assert!(profile.is_some());
        assert!(!profiler.active_sessions.contains_key(&profile_id));
    }

    #[test]
    fn test_bottleneck_detection() {
        let detector = BottleneckDetector {
            cpu_usage_samples: vec![85.0, 90.0, 88.0, 92.0],
            memory_pressure_events: 2,
            io_wait_time: Duration::from_millis(500),
            network_timeouts: 0,
        };

        let config = ProfilerConfig::default();
        let profiler = QueryProfiler::new(config);
        let bottlenecks = profiler.detect_bottlenecks(&detector, Duration::from_secs(5));

        assert!(!bottlenecks.is_empty());
        assert!(bottlenecks
            .iter()
            .any(|b| matches!(b.bottleneck_type, BottleneckType::CPU)));
    }

    #[test]
    fn test_explain_analyze_output() {
        let profile = QueryProfile {
            profile_id: Uuid::new_v4(),
            query: "SELECT * FROM users WHERE active = true".to_string(),
            parameters: HashMap::new(),
            phases: vec![ExecutionPhase {
                name: "Planning".to_string(),
                description: "Query planning".to_string(),
                start_offset: Duration::from_millis(0),
                duration: Duration::from_millis(50),
                percentage: 5.0,
                details: PhaseDetails::Planning {
                    plan_alternatives: 3,
                    selected_plan_cost: 100.0,
                    optimization_rules_applied: 5,
                },
                sub_phases: Vec::new(),
            }],
            overall_stats: ExecutionStats {
                total_duration: Duration::from_secs(1),
                rows_returned: 1000,
                bytes_processed: 50000,
                peak_memory_mb: 128.0,
                cpu_time: Duration::from_millis(800),
                io_operations: 50,
                network_round_trips: 5,
                cache_hit_ratio: 0.85,
            },
            resource_usage: ResourceUsage {
                memory_timeline: Vec::new(),
                cpu_usage: vec![50.0, 60.0],
                disk_io: DiskIOStats {
                    bytes_read: 1000,
                    bytes_written: 0,
                    read_operations: 10,
                    write_operations: 0,
                    avg_read_latency: Duration::from_millis(5),
                    avg_write_latency: Duration::ZERO,
                },
                network_io: NetworkIOStats {
                    bytes_sent: 500,
                    bytes_received: 2000,
                    connections_opened: 2,
                    requests_sent: 5,
                    avg_latency: Duration::from_millis(20),
                },
                actor_usage: HashMap::new(),
            },
            optimization_info: OptimizationInfo {
                original_cost: 200.0,
                optimized_cost: 100.0,
                cost_improvement: 50.0,
                applied_rules: vec!["PredicatePushdown".to_string()],
                considered_rules: vec!["JoinReordering".to_string()],
                rule_skip_reasons: HashMap::new(),
                index_usage: Vec::new(),
                join_strategies: Vec::new(),
            },
            bottlenecks: Vec::new(),
            suggestions: Vec::new(),
            distributed_info: None,
        };

        let config = ProfilerConfig::default();
        let profiler = QueryProfiler::new(config);
        let report = profiler.explain_analyze(&profile);

        assert!(report.contains("Query Profile ID"));
        assert!(report.contains("Total execution time"));
        assert!(report.contains("Execution Phases"));
    }
}
