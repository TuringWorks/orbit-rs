use crate::{ActorKey, ActorLease, PersistenceProvider, PersistenceMetrics};
use std::time::Duration;
use uuid::Uuid;
use tokio::time::{sleep, Instant};

/// Realistic actor lease workload patterns for orbit-rs
pub struct WorkloadSimulator {
    pub num_actors: usize,
    pub lease_duration: Duration,
    pub renewal_ratio: f64,      // 0.8 = 80% renewals, 20% new leases
    pub query_ratio: f64,        // 0.1 = 10% of operations are range queries
    pub operations_per_second: u64,
}

#[derive(Debug, Clone)]
pub struct WorkloadResults {
    pub total_operations: u64,
    pub write_latencies: Vec<Duration>,
    pub read_latencies: Vec<Duration>,
    pub range_query_latencies: Vec<Duration>,
    pub memory_usage_over_time: Vec<(Instant, u64)>,
    pub throughput_over_time: Vec<(Instant, u64)>,
    pub total_duration: Duration,
}

impl WorkloadSimulator {
    /// Create a workload that simulates typical orbit-rs actor lease patterns
    pub fn new_actor_lease_workload() -> Self {
        Self {
            num_actors: 10_000,
            lease_duration: Duration::from_secs(300), // 5 minute leases
            renewal_ratio: 0.85, // 85% renewals
            query_ratio: 0.05,   // 5% range queries for cluster coordination
            operations_per_second: 1_000,
        }
    }
    
    /// Create a high-throughput workload
    pub fn new_high_throughput_workload() -> Self {
        Self {
            num_actors: 50_000,
            lease_duration: Duration::from_secs(60), // 1 minute leases (more frequent renewals)
            renewal_ratio: 0.9,  // 90% renewals
            query_ratio: 0.02,   // 2% range queries
            operations_per_second: 10_000,
        }
    }
    
    /// Create a range-query heavy workload
    pub fn new_query_heavy_workload() -> Self {
        Self {
            num_actors: 5_000,
            lease_duration: Duration::from_secs(600), // 10 minute leases
            renewal_ratio: 0.7,  // 70% renewals
            query_ratio: 0.2,    // 20% range queries (high for cluster coordination)
            operations_per_second: 500,
        }
    }
    
    /// Run the workload simulation against a persistence provider
    pub async fn run<P: PersistenceProvider>(
        &self,
        provider: &P,
        duration: Duration,
    ) -> Result<WorkloadResults, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // Pre-populate with some initial leases
        let mut active_leases = self.create_initial_leases(provider).await?;
        
        let mut results = WorkloadResults {
            total_operations: 0,
            write_latencies: Vec::new(),
            read_latencies: Vec::new(),
            range_query_latencies: Vec::new(),
            memory_usage_over_time: Vec::new(),
            throughput_over_time: Vec::new(),
            total_duration: Duration::default(),
        };
        
        let mut last_throughput_sample = start_time;
        let mut ops_since_last_sample = 0;
        
        // Main simulation loop
        while start_time.elapsed() < duration {
            let operation_start = Instant::now();
            
            let rand_val: f64 = fastrand::f64();
            
            if rand_val < self.query_ratio {
                // Range query operation
                let metrics = self.perform_range_query(provider).await?;
                results.range_query_latencies.push(metrics.latency);
            } else if rand_val < self.query_ratio + self.renewal_ratio * (1.0 - self.query_ratio) {
                // Lease renewal operation
                if let Some(lease_key) = self.select_random_lease(&active_leases) {
                    let metrics = self.perform_lease_renewal(provider, &lease_key, &mut active_leases).await?;
                    results.write_latencies.push(metrics.latency);
                }
            } else {
                // New lease creation
                let metrics = self.perform_new_lease(provider, &mut active_leases).await?;
                results.write_latencies.push(metrics.latency);
            }
            
            results.total_operations += 1;
            ops_since_last_sample += 1;
            
            // Sample throughput and memory usage every second
            if operation_start.duration_since(last_throughput_sample) >= Duration::from_secs(1) {
                let stats = provider.get_stats().await?;
                results.memory_usage_over_time.push((operation_start, stats.memory_usage_bytes));
                results.throughput_over_time.push((operation_start, ops_since_last_sample));
                
                ops_since_last_sample = 0;
                last_throughput_sample = operation_start;
            }
            
            // Rate limiting to maintain target ops/sec
            let target_duration = Duration::from_nanos(1_000_000_000 / self.operations_per_second);
            let actual_duration = operation_start.elapsed();
            if actual_duration < target_duration {
                sleep(target_duration - actual_duration).await;
            }
        }
        
        results.total_duration = start_time.elapsed();
        Ok(results)
    }
    
    /// Create initial set of leases to simulate existing actor state
    async fn create_initial_leases<P: PersistenceProvider>(
        &self,
        provider: &P,
    ) -> Result<Vec<ActorKey>, Box<dyn std::error::Error>> {
        let mut lease_keys = Vec::new();
        
        // Create 50% of target actors initially
        let initial_count = self.num_actors / 2;
        
        for i in 0..initial_count {
            let actor_type = format!("actor_type_{}", i % 20); // 20 different actor types
            let lease = ActorLease::new(
                Uuid::new_v4(),
                actor_type,
                format!("node_{}", i % 5), // 5 different nodes
                self.lease_duration,
            );
            
            provider.store_lease(&lease).await?;
            lease_keys.push(lease.key.clone());
        }
        
        Ok(lease_keys)
    }
    
    /// Select a random lease for renewal
    fn select_random_lease(&self, active_leases: &[ActorKey]) -> Option<ActorKey> {
        if active_leases.is_empty() {
            return None;
        }
        let index = fastrand::usize(..active_leases.len());
        Some(active_leases[index].clone())
    }
    
    /// Perform a lease renewal operation
    async fn perform_lease_renewal<P: PersistenceProvider>(
        &self,
        provider: &P,
        key: &ActorKey,
        active_leases: &mut Vec<ActorKey>,
    ) -> Result<PersistenceMetrics, Box<dyn std::error::Error>> {
        // Get existing lease
        let (lease_opt, _) = provider.get_lease(key).await?;
        
        if let Some(mut lease) = lease_opt {
            // Renew the lease
            lease.renew(self.lease_duration);
            let metrics = provider.store_lease(&lease).await?;
            Ok(metrics)
        } else {
            // Lease not found, create new one
            self.perform_new_lease(provider, active_leases).await
        }
    }
    
    /// Perform a new lease creation
    async fn perform_new_lease<P: PersistenceProvider>(
        &self,
        provider: &P,
        active_leases: &mut Vec<ActorKey>,
    ) -> Result<PersistenceMetrics, Box<dyn std::error::Error>> {
        let actor_type = format!("actor_type_{}", fastrand::usize(0..20));
        let lease = ActorLease::new(
            Uuid::new_v4(),
            actor_type,
            format!("node_{}", fastrand::usize(0..5)),
            self.lease_duration,
        );
        
        let key = lease.key.clone();
        let metrics = provider.store_lease(&lease).await?;
        
        // Add to active leases if not already there
        if !active_leases.contains(&key) {
            active_leases.push(key);
            
            // Prevent unbounded growth
            if active_leases.len() > self.num_actors {
                let remove_index = fastrand::usize(..active_leases.len());
                active_leases.swap_remove(remove_index);
            }
        }
        
        Ok(metrics)
    }
    
    /// Perform a range query for cluster coordination
    async fn perform_range_query<P: PersistenceProvider>(
        &self,
        provider: &P,
    ) -> Result<PersistenceMetrics, Box<dyn std::error::Error>> {
        // Query a range of actor types (simulating cluster coordination)
        let start_type = fastrand::usize(0..15);
        let end_type = start_type + fastrand::usize(1..5);
        
        let start_key = ActorKey {
            actor_id: Uuid::nil(), // Minimum UUID
            actor_type: format!("actor_type_{}", start_type),
        };
        
        let end_key = ActorKey {
            actor_id: Uuid::max(), // Maximum UUID
            actor_type: format!("actor_type_{}", end_type),
        };
        
        let (_, metrics) = provider.range_query(&start_key, &end_key).await?;
        Ok(metrics)
    }
}

/// Crash and recovery simulation
pub struct CrashSimulator;

impl CrashSimulator {
    /// Simulate a crash and measure recovery characteristics
    pub async fn simulate_crash_recovery<P: PersistenceProvider>(
        provider: &P,
        pre_crash_operations: u64,
    ) -> Result<CrashRecoveryResults, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // Populate some data before crash
        let _workload = WorkloadSimulator::new_actor_lease_workload();
        let mut active_leases = Vec::new();
        
        for i in 0..pre_crash_operations {
            let actor_type = format!("actor_type_{}", i % 10);
            let lease = ActorLease::new(
                Uuid::new_v4(),
                actor_type,
                format!("node_{}", i % 3),
                Duration::from_secs(300),
            );
            
            provider.store_lease(&lease).await?;
            active_leases.push(lease.key.clone());
        }
        
        let data_load_time = start_time.elapsed();
        
        // Force a snapshot
        let (snapshot_id, snapshot_metrics) = provider.create_snapshot().await?;
        
        // Simulate crash and recovery
        let recovery_start = Instant::now();
        let _recovery_metrics = provider.simulate_crash_recovery().await?;
        let recovery_time = recovery_start.elapsed();
        
        // Verify data integrity post-recovery
        let verification_start = Instant::now();
        let mut recovered_count = 0;
        
        for lease_key in &active_leases {
            if let (Some(_), _) = provider.get_lease(lease_key).await? {
                recovered_count += 1;
            }
        }
        
        let verification_time = verification_start.elapsed();
        
        Ok(CrashRecoveryResults {
            pre_crash_operations,
            data_load_time,
            snapshot_creation_time: snapshot_metrics.latency,
            recovery_time,
            verification_time,
            data_integrity_ratio: recovered_count as f64 / active_leases.len() as f64,
            snapshot_id,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrashRecoveryResults {
    pub pre_crash_operations: u64,
    pub data_load_time: Duration,
    pub snapshot_creation_time: Duration,
    pub recovery_time: Duration,
    pub verification_time: Duration,
    pub data_integrity_ratio: f64, // 1.0 = perfect recovery
    pub snapshot_id: String,
}

impl WorkloadResults {
    /// Calculate statistics from the workload results
    pub fn calculate_stats(&self) -> WorkloadStats {
        WorkloadStats {
            total_operations: self.total_operations,
            duration: self.total_duration,
            throughput: self.total_operations as f64 / self.total_duration.as_secs_f64(),
            write_latency_p50: Self::percentile(&self.write_latencies, 0.5),
            write_latency_p95: Self::percentile(&self.write_latencies, 0.95),
            write_latency_p99: Self::percentile(&self.write_latencies, 0.99),
            read_latency_p50: Self::percentile(&self.read_latencies, 0.5),
            read_latency_p95: Self::percentile(&self.read_latencies, 0.95),
            read_latency_p99: Self::percentile(&self.read_latencies, 0.99),
            range_query_latency_p50: Self::percentile(&self.range_query_latencies, 0.5),
            range_query_latency_p95: Self::percentile(&self.range_query_latencies, 0.95),
            range_query_latency_p99: Self::percentile(&self.range_query_latencies, 0.99),
            peak_memory_usage: self.memory_usage_over_time.iter()
                .map(|(_, mem)| *mem)
                .max()
                .unwrap_or(0),
            average_throughput: self.throughput_over_time.iter()
                .map(|(_, tps)| *tps)
                .sum::<u64>() as f64 / self.throughput_over_time.len().max(1) as f64,
        }
    }
    
    fn percentile(values: &[Duration], p: f64) -> Duration {
        if values.is_empty() {
            return Duration::default();
        }
        
        let mut sorted: Vec<_> = values.iter().copied().collect();
        sorted.sort();
        
        let index = ((sorted.len() as f64 - 1.0) * p) as usize;
        sorted[index.min(sorted.len() - 1)]
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkloadStats {
    pub total_operations: u64,
    pub duration: Duration,
    pub throughput: f64,
    pub write_latency_p50: Duration,
    pub write_latency_p95: Duration,
    pub write_latency_p99: Duration,
    pub read_latency_p50: Duration,
    pub read_latency_p95: Duration,
    pub read_latency_p99: Duration,
    pub range_query_latency_p50: Duration,
    pub range_query_latency_p95: Duration,
    pub range_query_latency_p99: Duration,
    pub peak_memory_usage: u64,
    pub average_throughput: f64,
}