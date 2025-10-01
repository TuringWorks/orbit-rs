use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Persistent election state that survives node restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionState {
    /// Current Raft term
    pub current_term: u64,
    /// Node voted for in current term
    pub voted_for: Option<NodeId>,
    /// Last known leader
    pub last_known_leader: Option<NodeId>,
    /// Election history
    pub election_history: Vec<ElectionRecord>,
    /// Last update timestamp
    pub last_updated: i64,
}

/// Record of an election attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionRecord {
    pub term: u64,
    pub candidate: NodeId,
    pub timestamp: i64,
    pub successful: bool,
    pub vote_count: u32,
    pub total_nodes: u32,
}

impl Default for ElectionState {
    fn default() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            last_known_leader: None,
            election_history: Vec::new(),
            last_updated: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl ElectionState {
    /// Update the current term and reset vote
    pub fn update_term(&mut self, new_term: u64) {
        if new_term > self.current_term {
            self.current_term = new_term;
            self.voted_for = None; // Reset vote for new term
            self.last_updated = chrono::Utc::now().timestamp_millis();
        }
    }

    /// Cast vote for a candidate in current term
    pub fn vote_for(&mut self, candidate: NodeId, term: u64) -> Result<(), String> {
        if term != self.current_term {
            return Err(format!(
                "Term mismatch: expected {}, got {}",
                self.current_term, term
            ));
        }

        if let Some(existing_vote) = &self.voted_for {
            if existing_vote != &candidate {
                return Err(format!(
                    "Already voted for {} in term {}",
                    existing_vote, term
                ));
            }
        }

        self.voted_for = Some(candidate);
        self.last_updated = chrono::Utc::now().timestamp_millis();
        Ok(())
    }

    /// Record an election attempt
    pub fn record_election(&mut self, record: ElectionRecord) {
        self.election_history.push(record);
        self.last_updated = chrono::Utc::now().timestamp_millis();

        // Keep only last 100 election records
        if self.election_history.len() > 100 {
            self.election_history.remove(0);
        }
    }

    /// Update the last known leader
    pub fn update_leader(&mut self, leader: Option<NodeId>) {
        self.last_known_leader = leader;
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Check if we can vote for a candidate
    pub fn can_vote_for(&self, candidate: &NodeId, term: u64) -> bool {
        // Can't vote for old terms
        if term < self.current_term {
            return false;
        }

        // If it's a new term, we can vote
        if term > self.current_term {
            return true;
        }

        // For current term, check if we haven't voted or voted for same candidate
        match &self.voted_for {
            None => true,
            Some(voted_candidate) => voted_candidate == candidate,
        }
    }

    /// Get election statistics
    pub fn get_election_stats(&self) -> ElectionStats {
        let total_elections = self.election_history.len();
        let successful_elections = self
            .election_history
            .iter()
            .filter(|r| r.successful)
            .count();

        let recent_elections = self
            .election_history
            .iter()
            .rev()
            .take(10)
            .cloned()
            .collect();

        let avg_vote_percentage = if !self.election_history.is_empty() {
            let total_percentage: f64 = self
                .election_history
                .iter()
                .map(|r| r.vote_count as f64 / r.total_nodes as f64)
                .sum();
            total_percentage / self.election_history.len() as f64
        } else {
            0.0
        };

        ElectionStats {
            total_elections,
            successful_elections,
            success_rate: if total_elections > 0 {
                successful_elections as f64 / total_elections as f64
            } else {
                0.0
            },
            current_term: self.current_term,
            last_known_leader: self.last_known_leader.clone(),
            recent_elections,
            avg_vote_percentage,
        }
    }
}

/// Election statistics for monitoring
#[derive(Debug, Clone)]
pub struct ElectionStats {
    pub total_elections: usize,
    pub successful_elections: usize,
    pub success_rate: f64,
    pub current_term: u64,
    pub last_known_leader: Option<NodeId>,
    pub recent_elections: Vec<ElectionRecord>,
    pub avg_vote_percentage: f64,
}

/// Persistent storage manager for election state
pub struct ElectionStateManager {
    state_path: std::path::PathBuf,
    state: RwLock<ElectionState>,
    node_id: NodeId,
}

impl ElectionStateManager {
    /// Create a new election state manager
    pub fn new<P: AsRef<Path>>(state_path: P, node_id: NodeId) -> Self {
        Self {
            state_path: state_path.as_ref().to_path_buf(),
            state: RwLock::new(ElectionState::default()),
            node_id,
        }
    }

    /// Load election state from disk
    pub async fn load(&self) -> OrbitResult<()> {
        if !self.state_path.exists() {
            info!("No existing election state found, starting with defaults");
            return Ok(());
        }

        let content = fs::read_to_string(&self.state_path)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to read election state: {}", e)))?;

        let loaded_state: ElectionState = serde_json::from_str(&content)
            .map_err(|e| OrbitError::internal(format!("Failed to parse election state: {}", e)))?;

        {
            let mut state = self.state.write().await;
            *state = loaded_state;
        }

        info!(
            "Loaded election state from disk: term {}",
            self.get_current_term().await
        );
        Ok(())
    }

    /// Save election state to disk
    pub async fn save(&self) -> OrbitResult<()> {
        let state = self.state.read().await;
        let content = serde_json::to_string_pretty(&*state).map_err(|e| {
            OrbitError::internal(format!("Failed to serialize election state: {}", e))
        })?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OrbitError::internal(format!("Failed to create state directory: {}", e))
            })?;
        }

        // Write to temporary file first, then rename for atomicity
        let temp_path = self.state_path.with_extension("tmp");
        fs::write(&temp_path, content)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to write election state: {}", e)))?;

        fs::rename(&temp_path, &self.state_path)
            .await
            .map_err(|e| {
                OrbitError::internal(format!("Failed to update election state file: {}", e))
            })?;

        Ok(())
    }

    /// Get current term
    pub async fn get_current_term(&self) -> u64 {
        let state = self.state.read().await;
        state.current_term
    }

    /// Get who we voted for in current term
    pub async fn get_voted_for(&self) -> Option<NodeId> {
        let state = self.state.read().await;
        state.voted_for.clone()
    }

    /// Update term and save to disk
    pub async fn update_term(&self, new_term: u64) -> OrbitResult<()> {
        {
            let mut state = self.state.write().await;
            state.update_term(new_term);
        }

        self.save().await?;
        info!("Updated election term to: {}", new_term);
        Ok(())
    }

    /// Cast vote and save to disk
    pub async fn vote_for(&self, candidate: NodeId, term: u64) -> OrbitResult<()> {
        {
            let mut state = self.state.write().await;
            state
                .vote_for(candidate.clone(), term)
                .map_err(|e| OrbitError::configuration(e))?;
        }

        self.save().await?;
        info!("Voted for {} in term {}", candidate, term);
        Ok(())
    }

    /// Check if we can vote for a candidate
    pub async fn can_vote_for(&self, candidate: &NodeId, term: u64) -> bool {
        let state = self.state.read().await;
        state.can_vote_for(candidate, term)
    }

    /// Record election result
    pub async fn record_election(&self, record: ElectionRecord) -> OrbitResult<()> {
        {
            let mut state = self.state.write().await;
            state.record_election(record.clone());
        }

        self.save().await?;
        info!(
            "Recorded election: term {}, candidate {}, success: {}",
            record.term, record.candidate, record.successful
        );
        Ok(())
    }

    /// Update leader and save to disk
    pub async fn update_leader(&self, leader: Option<NodeId>) -> OrbitResult<()> {
        {
            let mut state = self.state.write().await;
            state.update_leader(leader.clone());
        }

        self.save().await?;

        match leader {
            Some(leader_id) => info!("Updated leader to: {}", leader_id),
            None => info!("Cleared leader"),
        }

        Ok(())
    }

    /// Get election statistics
    pub async fn get_stats(&self) -> ElectionStats {
        let state = self.state.read().await;
        state.get_election_stats()
    }

    /// Get last known leader
    pub async fn get_last_known_leader(&self) -> Option<NodeId> {
        let state = self.state.read().await;
        state.last_known_leader.clone()
    }

    /// Start periodic state saving
    pub async fn start_periodic_save(&self) -> OrbitResult<()> {
        let state_manager = Self {
            state_path: self.state_path.clone(),
            state: RwLock::new(ElectionState::default()),
            node_id: self.node_id.clone(),
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                if let Err(e) = state_manager.save().await {
                    error!("Failed to save election state: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Cleanup old election records
    pub async fn cleanup_old_records(&self, keep_count: usize) -> OrbitResult<()> {
        {
            let mut state = self.state.write().await;
            if state.election_history.len() > keep_count {
                let remove_count = state.election_history.len() - keep_count;
                state.election_history.drain(0..remove_count);
                state.last_updated = chrono::Utc::now().timestamp_millis();
            }
        }

        self.save().await?;
        Ok(())
    }

    /// Export election state for debugging
    pub async fn export_state(&self) -> OrbitResult<String> {
        let state = self.state.read().await;
        serde_json::to_string_pretty(&*state)
            .map_err(|e| OrbitError::internal(format!("Failed to export election state: {}", e)))
    }

    /// Import election state (for recovery)
    pub async fn import_state(&self, state_json: &str) -> OrbitResult<()> {
        let imported_state: ElectionState = serde_json::from_str(state_json)
            .map_err(|e| OrbitError::internal(format!("Failed to parse imported state: {}", e)))?;

        {
            let mut state = self.state.write().await;
            *state = imported_state;
        }

        self.save().await?;
        info!("Imported election state successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_election_state_persistence() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("election_state.json");
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());

        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());

        // Update term and vote
        state_manager.update_term(5).await.unwrap();
        state_manager.vote_for(node_id.clone(), 5).await.unwrap();

        // Verify persistence
        assert!(state_path.exists());

        // Create new manager and load
        let state_manager2 = ElectionStateManager::new(&state_path, node_id.clone());
        state_manager2.load().await.unwrap();

        assert_eq!(state_manager2.get_current_term().await, 5);
        assert_eq!(state_manager2.get_voted_for().await, Some(node_id));
    }

    #[tokio::test]
    async fn test_election_record_tracking() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("election_state.json");
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());

        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());

        // Record some elections
        let record1 = ElectionRecord {
            term: 1,
            candidate: node_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            successful: true,
            vote_count: 3,
            total_nodes: 5,
        };

        let record2 = ElectionRecord {
            term: 2,
            candidate: node_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            successful: false,
            vote_count: 2,
            total_nodes: 5,
        };

        state_manager.record_election(record1).await.unwrap();
        state_manager.record_election(record2).await.unwrap();

        // Check statistics
        let stats = state_manager.get_stats().await;
        assert_eq!(stats.total_elections, 2);
        assert_eq!(stats.successful_elections, 1);
        assert_eq!(stats.success_rate, 0.5);
    }

    #[test]
    fn test_vote_validation() {
        let mut state = ElectionState::default();
        let node_id = NodeId::new("test".to_string(), "default".to_string());

        // Should be able to vote in term 1
        state.update_term(1);
        assert!(state.vote_for(node_id.clone(), 1).is_ok());

        // Should not be able to vote for different candidate in same term
        let node_id2 = NodeId::new("test2".to_string(), "default".to_string());
        assert!(state.vote_for(node_id2, 1).is_err());

        // Should be able to vote for same candidate again
        assert!(state.vote_for(node_id, 1).is_ok());
    }
}
