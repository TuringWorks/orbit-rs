use crate::mesh::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

/// Comprehensive election metrics collector
#[derive(Debug, Clone)]
pub struct ElectionMetrics {
    /// Election timing metrics
    timing: Arc<RwLock<ElectionTimingMetrics>>,
    /// Network metrics during elections
    network: Arc<RwLock<ElectionNetworkMetrics>>,
    /// Election outcome metrics
    outcomes: Arc<RwLock<ElectionOutcomeMetrics>>,
    /// Performance metrics
    performance: Arc<RwLock<ElectionPerformanceMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct ElectionTimingMetrics {
    /// Average election duration
    pub avg_election_duration: Duration,
    /// Fastest election time
    pub fastest_election: Option<Duration>,
    /// Slowest election time  
    pub slowest_election: Option<Duration>,
    /// Recent election durations (last 50)
    pub recent_durations: Vec<Duration>,
    /// Average time between elections
    pub avg_time_between_elections: Duration,
    /// Last election timestamp
    pub last_election_time: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
pub struct ElectionNetworkMetrics {
    /// Network round-trip times during elections
    pub vote_request_latencies: HashMap<NodeId, Vec<Duration>>,
    /// Message loss rates per node
    pub message_loss_rates: HashMap<NodeId, f64>,
    /// Network partition events
    pub partition_events: Vec<PartitionEvent>,
    /// Total messages sent during elections
    pub total_messages_sent: u64,
    /// Total messages received during elections
    pub total_messages_received: u64,
}

#[derive(Debug, Clone)]
pub struct PartitionEvent {
    pub timestamp: Instant,
    pub affected_nodes: Vec<NodeId>,
    pub duration: Option<Duration>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ElectionOutcomeMetrics {
    /// Total elections participated in
    pub total_elections: u64,
    /// Elections won by this node
    pub elections_won: u64,
    /// Elections lost by this node  
    pub elections_lost: u64,
    /// Elections timed out
    pub elections_timeout: u64,
    /// Split votes (no winner)
    pub split_votes: u64,
    /// Vote percentages received (as leader candidate)
    pub vote_percentages: Vec<f64>,
    /// Leadership tenure durations
    pub leadership_durations: Vec<Duration>,
}

#[derive(Debug, Clone, Default)]
pub struct ElectionPerformanceMetrics {
    /// CPU usage during elections
    pub cpu_usage_during_election: Vec<f64>,
    /// Memory usage during elections
    pub memory_usage_during_election: Vec<u64>,
    /// Disk I/O during state persistence
    pub disk_io_bytes: u64,
    /// Lock contention metrics
    pub lock_contention_events: u64,
    /// Background task performance
    pub heartbeat_performance: HeartbeatPerformanceMetrics,
}

#[derive(Debug, Clone, Default)]
pub struct HeartbeatPerformanceMetrics {
    /// Average heartbeat interval
    pub avg_heartbeat_interval: Duration,
    /// Missed heartbeats
    pub missed_heartbeats: u64,
    /// Heartbeat success rate
    pub heartbeat_success_rate: f64,
    /// Network delays for heartbeats
    pub heartbeat_network_delays: Vec<Duration>,
}

impl ElectionMetrics {
    pub fn new() -> Self {
        Self {
            timing: Arc::new(RwLock::new(ElectionTimingMetrics::default())),
            network: Arc::new(RwLock::new(ElectionNetworkMetrics::default())),
            outcomes: Arc::new(RwLock::new(ElectionOutcomeMetrics::default())),
            performance: Arc::new(RwLock::new(ElectionPerformanceMetrics::default())),
        }
    }

    /// Start recording an election
    pub async fn start_election(&self, term: u64) -> ElectionTracker {
        let now = Instant::now();

        // Update timing metrics
        {
            let mut timing = self.timing.write().await;
            if let Some(last_time) = timing.last_election_time {
                let time_between = now.duration_since(last_time);

                // Update average time between elections
                let current_avg = timing.avg_time_between_elections;
                timing.avg_time_between_elections = if current_avg == Duration::ZERO {
                    time_between
                } else {
                    Duration::from_millis(
                        (current_avg.as_millis() as u64 + time_between.as_millis() as u64) / 2,
                    )
                };
            }

            timing.last_election_time = Some(now);
        }

        // Update outcome metrics
        {
            let mut outcomes = self.outcomes.write().await;
            outcomes.total_elections += 1;
        }

        ElectionTracker {
            start_time: now,
            term,
            metrics: self.clone(),
        }
    }

    /// Record vote request latency
    pub async fn record_vote_latency(&self, target_node: &NodeId, latency: Duration) {
        let mut network = self.network.write().await;
        network
            .vote_request_latencies
            .entry(target_node.clone())
            .or_insert_with(Vec::new)
            .push(latency);

        // Keep only last 100 measurements per node
        if let Some(latencies) = network.vote_request_latencies.get_mut(target_node) {
            if latencies.len() > 100 {
                latencies.remove(0);
            }
        }
    }

    /// Record message loss
    pub async fn record_message_loss(
        &self,
        target_node: &NodeId,
        total_sent: u64,
        total_received: u64,
    ) {
        let loss_rate = if total_sent > 0 {
            1.0 - (total_received as f64 / total_sent as f64)
        } else {
            0.0
        };

        let mut network = self.network.write().await;
        network
            .message_loss_rates
            .insert(target_node.clone(), loss_rate);
        network.total_messages_sent += total_sent;
        network.total_messages_received += total_received;
    }

    /// Record network partition
    pub async fn record_partition(&self, affected_nodes: Vec<NodeId>) {
        let mut network = self.network.write().await;
        network.partition_events.push(PartitionEvent {
            timestamp: Instant::now(),
            affected_nodes,
            duration: None,
            resolved: false,
        });
    }

    /// Record partition resolution
    pub async fn record_partition_resolved(&self, partition_index: usize) {
        let mut network = self.network.write().await;
        if let Some(partition) = network.partition_events.get_mut(partition_index) {
            if !partition.resolved {
                partition.duration = Some(partition.timestamp.elapsed());
                partition.resolved = true;
            }
        }
    }

    /// Record performance metrics during election
    pub async fn record_performance(&self, cpu_usage: f64, memory_usage: u64) {
        let mut performance = self.performance.write().await;
        performance.cpu_usage_during_election.push(cpu_usage);
        performance.memory_usage_during_election.push(memory_usage);

        // Keep only last 50 measurements
        if performance.cpu_usage_during_election.len() > 50 {
            performance.cpu_usage_during_election.remove(0);
        }
        if performance.memory_usage_during_election.len() > 50 {
            performance.memory_usage_during_election.remove(0);
        }
    }

    /// Record heartbeat performance
    pub async fn record_heartbeat(
        &self,
        interval: Duration,
        success: bool,
        network_delay: Option<Duration>,
    ) {
        let mut performance = self.performance.write().await;
        let heartbeat = &mut performance.heartbeat_performance;

        // Update average interval
        let current_avg = heartbeat.avg_heartbeat_interval;
        heartbeat.avg_heartbeat_interval = if current_avg == Duration::ZERO {
            interval
        } else {
            Duration::from_millis(
                (current_avg.as_millis() as u64 + interval.as_millis() as u64) / 2,
            )
        };

        // Update success rate
        if !success {
            heartbeat.missed_heartbeats += 1;
        }

        // Record network delay
        if let Some(delay) = network_delay {
            heartbeat.heartbeat_network_delays.push(delay);

            // Keep only last 100 measurements
            if heartbeat.heartbeat_network_delays.len() > 100 {
                heartbeat.heartbeat_network_delays.remove(0);
            }
        }
    }

    /// Get comprehensive metrics summary
    pub async fn get_summary(&self) -> ElectionMetricsSummary {
        let timing = self.timing.read().await;
        let network = self.network.read().await;
        let outcomes = self.outcomes.read().await;
        let performance = self.performance.read().await;

        ElectionMetricsSummary {
            // Timing metrics
            avg_election_duration: timing.avg_election_duration,
            fastest_election: timing.fastest_election,
            slowest_election: timing.slowest_election,
            avg_time_between_elections: timing.avg_time_between_elections,

            // Network metrics
            avg_vote_latency: calculate_avg_latency(&network.vote_request_latencies),
            avg_message_loss_rate: calculate_avg_loss_rate(&network.message_loss_rates),
            partition_count: network.partition_events.len(),
            unresolved_partitions: network
                .partition_events
                .iter()
                .filter(|p| !p.resolved)
                .count(),

            // Outcome metrics
            total_elections: outcomes.total_elections,
            win_rate: if outcomes.total_elections > 0 {
                outcomes.elections_won as f64 / outcomes.total_elections as f64
            } else {
                0.0
            },
            timeout_rate: if outcomes.total_elections > 0 {
                outcomes.elections_timeout as f64 / outcomes.total_elections as f64
            } else {
                0.0
            },
            avg_vote_percentage: if !outcomes.vote_percentages.is_empty() {
                outcomes.vote_percentages.iter().sum::<f64>()
                    / outcomes.vote_percentages.len() as f64
            } else {
                0.0
            },
            avg_leadership_duration: if !outcomes.leadership_durations.is_empty() {
                let total_millis: u64 = outcomes
                    .leadership_durations
                    .iter()
                    .map(|d| d.as_millis() as u64)
                    .sum();
                Duration::from_millis(total_millis / outcomes.leadership_durations.len() as u64)
            } else {
                Duration::ZERO
            },

            // Performance metrics
            avg_cpu_during_election: if !performance.cpu_usage_during_election.is_empty() {
                performance.cpu_usage_during_election.iter().sum::<f64>()
                    / performance.cpu_usage_during_election.len() as f64
            } else {
                0.0
            },
            avg_memory_during_election: if !performance.memory_usage_during_election.is_empty() {
                performance.memory_usage_during_election.iter().sum::<u64>()
                    / performance.memory_usage_during_election.len() as u64
            } else {
                0
            },
            heartbeat_success_rate: performance.heartbeat_performance.heartbeat_success_rate,
            missed_heartbeats: performance.heartbeat_performance.missed_heartbeats,
        }
    }

    /// Export metrics for external monitoring systems
    pub async fn export_prometheus(&self) -> String {
        let summary = self.get_summary().await;

        format!(
            r#"# HELP orbit_election_duration_seconds Average election duration
# TYPE orbit_election_duration_seconds gauge
orbit_election_duration_seconds {:.3}

# HELP orbit_election_win_rate Election win rate
# TYPE orbit_election_win_rate gauge  
orbit_election_win_rate {:.3}

# HELP orbit_election_timeout_rate Election timeout rate
# TYPE orbit_election_timeout_rate gauge
orbit_election_timeout_rate {:.3}

# HELP orbit_election_vote_latency_seconds Average vote request latency
# TYPE orbit_election_vote_latency_seconds gauge
orbit_election_vote_latency_seconds {:.6}

# HELP orbit_election_message_loss_rate Message loss rate during elections
# TYPE orbit_election_message_loss_rate gauge
orbit_election_message_loss_rate {:.3}

# HELP orbit_election_partition_count Number of network partitions detected
# TYPE orbit_election_partition_count counter
orbit_election_partition_count {}

# HELP orbit_election_cpu_usage CPU usage during elections
# TYPE orbit_election_cpu_usage gauge
orbit_election_cpu_usage {:.2}

# HELP orbit_election_memory_bytes Memory usage during elections
# TYPE orbit_election_memory_bytes gauge
orbit_election_memory_bytes {}

# HELP orbit_heartbeat_success_rate Heartbeat success rate
# TYPE orbit_heartbeat_success_rate gauge
orbit_heartbeat_success_rate {:.3}
"#,
            summary.avg_election_duration.as_secs_f64(),
            summary.win_rate,
            summary.timeout_rate,
            summary.avg_vote_latency.as_secs_f64(),
            summary.avg_message_loss_rate,
            summary.partition_count,
            summary.avg_cpu_during_election,
            summary.avg_memory_during_election,
            summary.heartbeat_success_rate,
        )
    }
}

/// Tracks a single election from start to finish
pub struct ElectionTracker {
    start_time: Instant,
    term: u64,
    metrics: ElectionMetrics,
}

impl ElectionTracker {
    /// Complete the election with outcome
    pub async fn complete(self, outcome: ElectionOutcome, vote_count: u32, total_nodes: u32) {
        let duration = self.start_time.elapsed();

        // Update timing metrics
        {
            let mut timing = self.metrics.timing.write().await;

            // Update average duration
            let current_avg = timing.avg_election_duration;
            timing.avg_election_duration = if current_avg == Duration::ZERO {
                duration
            } else {
                Duration::from_millis(
                    (current_avg.as_millis() as u64 + duration.as_millis() as u64) / 2,
                )
            };

            // Update fastest/slowest
            if timing.fastest_election.is_none() || duration < timing.fastest_election.unwrap() {
                timing.fastest_election = Some(duration);
            }
            if timing.slowest_election.is_none() || duration > timing.slowest_election.unwrap() {
                timing.slowest_election = Some(duration);
            }

            // Add to recent durations
            timing.recent_durations.push(duration);
            if timing.recent_durations.len() > 50 {
                timing.recent_durations.remove(0);
            }
        }

        // Update outcome metrics
        {
            let mut outcomes = self.metrics.outcomes.write().await;

            match outcome {
                ElectionOutcome::Won => {
                    outcomes.elections_won += 1;
                    let vote_percentage = vote_count as f64 / total_nodes as f64;
                    outcomes.vote_percentages.push(vote_percentage);
                }
                ElectionOutcome::Lost => {
                    outcomes.elections_lost += 1;
                }
                ElectionOutcome::Timeout => {
                    outcomes.elections_timeout += 1;
                }
                ElectionOutcome::SplitVote => {
                    outcomes.split_votes += 1;
                }
            }
        }

        info!(
            "Election completed: term={}, duration={:?}, outcome={:?}, votes={}/{}",
            self.term, duration, outcome, vote_count, total_nodes
        );
    }
}

#[derive(Debug, Clone)]
pub enum ElectionOutcome {
    Won,
    Lost,
    Timeout,
    SplitVote,
}

/// Summary of all election metrics
#[derive(Debug, Clone)]
pub struct ElectionMetricsSummary {
    // Timing
    pub avg_election_duration: Duration,
    pub fastest_election: Option<Duration>,
    pub slowest_election: Option<Duration>,
    pub avg_time_between_elections: Duration,

    // Network
    pub avg_vote_latency: Duration,
    pub avg_message_loss_rate: f64,
    pub partition_count: usize,
    pub unresolved_partitions: usize,

    // Outcomes
    pub total_elections: u64,
    pub win_rate: f64,
    pub timeout_rate: f64,
    pub avg_vote_percentage: f64,
    pub avg_leadership_duration: Duration,

    // Performance
    pub avg_cpu_during_election: f64,
    pub avg_memory_during_election: u64,
    pub heartbeat_success_rate: f64,
    pub missed_heartbeats: u64,
}

// Helper functions
fn calculate_avg_latency(latencies: &HashMap<NodeId, Vec<Duration>>) -> Duration {
    let all_latencies: Vec<Duration> = latencies.values().flat_map(|v| v.iter()).cloned().collect();

    if all_latencies.is_empty() {
        return Duration::ZERO;
    }

    let total_millis: u64 = all_latencies.iter().map(|d| d.as_millis() as u64).sum();

    Duration::from_millis(total_millis / all_latencies.len() as u64)
}

fn calculate_avg_loss_rate(loss_rates: &HashMap<NodeId, f64>) -> f64 {
    if loss_rates.is_empty() {
        return 0.0;
    }

    loss_rates.values().sum::<f64>() / loss_rates.len() as f64
}

impl Default for ElectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}
