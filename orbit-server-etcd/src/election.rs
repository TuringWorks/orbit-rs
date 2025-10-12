/// Leader election using etcd for distributed coordination
use crate::error::{EtcdError, EtcdResult};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Leader election configuration
#[derive(Debug, Clone)]
pub struct ElectionConfig {
    /// Unique session ID for this node
    pub session_id: String,
    /// Election key prefix in etcd
    pub election_prefix: String,
    /// TTL for the leader lease (seconds)
    pub lease_ttl: i64,
    /// Interval for lease renewal
    pub renewal_interval: Duration,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            session_id: format!("session-{}", uuid::Uuid::new_v4()),
            election_prefix: "/orbit/leader-election".to_string(),
            lease_ttl: 10,
            renewal_interval: Duration::from_secs(3),
        }
    }
}

/// Leader election state
#[derive(Debug, Clone, PartialEq)]
pub enum LeadershipState {
    /// Not participating in election
    Idle,
    /// Waiting to become leader
    Follower,
    /// Currently the leader
    Leader,
    /// Lost leadership
    LostLeadership,
}

/// Callback trait for leadership events
#[async_trait]
pub trait LeadershipObserver: Send + Sync {
    /// Called when this node becomes the leader
    async fn on_elected(&self);
    
    /// Called when this node loses leadership
    async fn on_lost_leadership(&self);
    
    /// Called periodically while leader
    async fn on_heartbeat(&self);
}

/// Leader election manager using etcd
pub struct LeaderElection {
    config: ElectionConfig,
    state: Arc<RwLock<LeadershipState>>,
    lease_id: Arc<RwLock<Option<i64>>>,
    observers: Arc<RwLock<Vec<Arc<dyn LeadershipObserver>>>>,
}

impl LeaderElection {
    /// Create a new leader election instance
    pub fn new(config: ElectionConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(LeadershipState::Idle)),
            lease_id: Arc::new(RwLock::new(None)),
            observers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an observer for leadership events
    pub async fn add_observer(&self, observer: Arc<dyn LeadershipObserver>) {
        let mut observers = self.observers.write().await;
        observers.push(observer);
    }

    /// Start participating in leader election
    pub async fn start_election(&self) -> EtcdResult<()> {
        info!("Starting leader election with session: {}", self.config.session_id);
        
        let mut state = self.state.write().await;
        *state = LeadershipState::Follower;
        drop(state);
        
        // In a real implementation, this would:
        // 1. Create an etcd lease
        // 2. Try to acquire leadership by writing to election key
        // 3. Start background task to maintain leadership
        // 4. Monitor for leadership changes
        
        self.run_election_loop().await
    }

    /// Main election loop
    async fn run_election_loop(&self) -> EtcdResult<()> {
        let election = self.clone();
        
        tokio::spawn(async move {
            loop {
                let current_state = *election.state.read().await;
                
                match current_state {
                    LeadershipState::Idle => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    LeadershipState::Follower => {
                        if election.try_acquire_leadership().await.unwrap_or(false) {
                            info!("Acquired leadership");
                            *election.state.write().await = LeadershipState::Leader;
                            election.notify_elected().await;
                        } else {
                            debug!("Failed to acquire leadership, remaining as follower");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                    LeadershipState::Leader => {
                        if !election.maintain_leadership().await.unwrap_or(false) {
                            warn!("Lost leadership");
                            *election.state.write().await = LeadershipState::LostLeadership;
                            election.notify_lost_leadership().await;
                        } else {
                            election.notify_heartbeat().await;
                            tokio::time::sleep(election.config.renewal_interval).await;
                        }
                    }
                    LeadershipState::LostLeadership => {
                        info!("Attempting to regain leadership");
                        *election.state.write().await = LeadershipState::Follower;
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Try to acquire leadership
    async fn try_acquire_leadership(&self) -> EtcdResult<bool> {
        // In a real implementation with etcd client:
        // 1. Create a lease with TTL
        // 2. Try to create election key with lease
        // 3. If key already exists, become follower
        // 4. If creation succeeds, become leader
        
        // Simulate election with 20% success rate
        let acquired = fastrand::f32() < 0.2;
        
        if acquired {
            let mut lease_id = self.lease_id.write().await;
            *lease_id = Some(fastrand::i64(1..1000000));
        }
        
        Ok(acquired)
    }

    /// Maintain leadership by renewing lease
    async fn maintain_leadership(&self) -> EtcdResult<bool> {
        let lease_id = self.lease_id.read().await;
        
        if lease_id.is_none() {
            return Ok(false);
        }
        
        // In a real implementation:
        // 1. Keep the lease alive via etcd keep-alive
        // 2. Monitor for lease expiration
        // 3. Detect if someone else acquired leadership
        
        // Simulate 98% success rate for maintaining leadership
        Ok(fastrand::f32() < 0.98)
    }

    /// Resign from leadership
    pub async fn resign(&self) -> EtcdResult<()> {
        info!("Resigning from leadership");
        
        let mut state = self.state.write().await;
        if *state == LeadershipState::Leader {
            *state = LeadershipState::Follower;
            drop(state);
            
            // Revoke lease in real implementation
            let mut lease_id = self.lease_id.write().await;
            *lease_id = None;
            
            self.notify_lost_leadership().await;
        }
        
        Ok(())
    }

    /// Check if this node is currently the leader
    pub async fn is_leader(&self) -> bool {
        *self.state.read().await == LeadershipState::Leader
    }

    /// Get current leadership state
    pub async fn get_state(&self) -> LeadershipState {
        *self.state.read().await
    }

    /// Notify observers of election
    async fn notify_elected(&self) {
        let observers = self.observers.read().await;
        for observer in observers.iter() {
            observer.on_elected().await;
        }
    }

    /// Notify observers of lost leadership
    async fn notify_lost_leadership(&self) {
        let observers = self.observers.read().await;
        for observer in observers.iter() {
            observer.on_lost_leadership().await;
        }
    }

    /// Notify observers of heartbeat
    async fn notify_heartbeat(&self) {
        let observers = self.observers.read().await;
        for observer in observers.iter() {
            observer.on_heartbeat().await;
        }
    }
}

impl Clone for LeaderElection {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            lease_id: Arc::clone(&self.lease_id),
            observers: Arc::clone(&self.observers),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObserver {
        elected_count: Arc<RwLock<usize>>,
    }

    #[async_trait]
    impl LeadershipObserver for TestObserver {
        async fn on_elected(&self) {
            let mut count = self.elected_count.write().await;
            *count += 1;
        }

        async fn on_lost_leadership(&self) {}
        async fn on_heartbeat(&self) {}
    }

    #[tokio::test]
    async fn test_election_creation() {
        let config = ElectionConfig::default();
        let election = LeaderElection::new(config);
        
        assert_eq!(election.get_state().await, LeadershipState::Idle);
        assert!(!election.is_leader().await);
    }

    #[tokio::test]
    async fn test_observer_registration() {
        let election = LeaderElection::new(ElectionConfig::default());
        let observer = Arc::new(TestObserver {
            elected_count: Arc::new(RwLock::new(0)),
        });
        
        election.add_observer(observer.clone()).await;
        
        let observers = election.observers.read().await;
        assert_eq!(observers.len(), 1);
    }

    #[tokio::test]
    async fn test_leadership_state_transitions() {
        let election = LeaderElection::new(ElectionConfig::default());
        
        // Start as idle
        assert_eq!(election.get_state().await, LeadershipState::Idle);
        
        // Manually set to follower
        *election.state.write().await = LeadershipState::Follower;
        assert_eq!(election.get_state().await, LeadershipState::Follower);
        
        // Manually become leader
        *election.state.write().await = LeadershipState::Leader;
        assert!(election.is_leader().await);
    }
}
