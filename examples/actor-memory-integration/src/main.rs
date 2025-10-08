use anyhow::Result;
use async_trait::async_trait;
use orbit_server::memory::*;
use orbit_shared::addressable::{AddressableReference, Addressable, Key, ActorWithStringKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Example actor that manages social media user data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialMediaUserActor {
    user_id: u64,
    followers_count: u64,
    influence_score: f64,
}

impl SocialMediaUserActor {
    pub fn new(user_id: u64, followers_count: u64) -> Self {
        Self {
            user_id,
            followers_count,
            influence_score: 0.0,
        }
    }

    /// Calculate influence score - this will benefit from memory pinning
    pub async fn calculate_influence(&mut self) -> Result<f64> {
        // Simulate complex calculation that accesses follower network data
        println!("Calculating influence for user {} with {} followers", 
                 self.user_id, self.followers_count);
        
        // Simulate memory-intensive computation
        let base_influence = (self.followers_count as f64).log10();
        let network_multiplier = self.get_network_multiplier().await?;
        
        self.influence_score = base_influence * network_multiplier;
        Ok(self.influence_score)
    }

    async fn get_network_multiplier(&self) -> Result<f64> {
        // Simulate accessing pinned network data
        sleep(Duration::from_micros(100)).await; // 100μs with pinned memory
        Ok(1.5) // Simplified
    }

    /// Get memory profile based on user characteristics
    pub fn get_memory_profile(&self) -> ActorMemoryProfile {
        match self.followers_count {
            0..=1000 => ActorMemoryProfile::Cold,
            1001..=100000 => ActorMemoryProfile::Warm,
            _ => ActorMemoryProfile::Hot,
        }
    }

    pub fn estimate_memory_footprint(&self) -> MemoryFootprint {
        let base_size = 1024; // 1KB per user
        let network_size = (self.followers_count / 100) * 1024; // 1KB per 100 followers
        
        MemoryFootprint {
            persistent_state_mb: ((base_size + network_size) / (1024 * 1024)).max(1),
            working_set_mb: 4, // 4MB for calculations
            prefetch_candidates: vec![], // Would contain related user extents
        }
    }
}

impl Addressable for SocialMediaUserActor {
    fn addressable_type() -> &'static str {
        "SocialMediaUserActor"
    }
}

impl ActorWithStringKey for SocialMediaUserActor {}

/// Memory profile classification for actors
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActorMemoryProfile {
    Hot,    // <1ms tail latency requirement
    Warm,   // <10ms tail latency requirement  
    Cold,   // >100ms acceptable
}

/// Memory footprint estimate for an actor
#[derive(Debug, Clone)]
pub struct MemoryFootprint {
    pub persistent_state_mb: u64,
    pub working_set_mb: u64,
    pub prefetch_candidates: Vec<ExtentRef>,
}

/// Context provided during actor activation
pub struct ActorActivationContext {
    pub actor_ref: AddressableReference,
    pub memory_manager: Arc<dyn ActorMemoryManager>,
    pub extent_index: Arc<dyn ExtentIndex>,
    pub local_numa_node: u16,
    pub activation_reason: ActivationReason,
}

#[derive(Debug, Clone, Copy)]
pub enum ActivationReason {
    FirstInvocation,
    Migration,
    Reactivation,
    Prefetch,
}

pub struct ActorDeactivationContext {
    pub actor_ref: AddressableReference,
    pub memory_manager: Arc<dyn ActorMemoryManager>,
}

/// Enhanced actor lifecycle with memory management
#[async_trait]
pub trait ActorLifecycleWithMemory: Addressable {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()>;
    async fn on_deactivate(&mut self, context: &ActorDeactivationContext) -> Result<()>;
}

#[async_trait]
impl ActorLifecycleWithMemory for SocialMediaUserActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        println!("🚀 Activating social media user actor for user {}", self.user_id);
        
        // 1. Determine memory strategy based on user profile
        let profile = self.get_memory_profile();
        let footprint = self.estimate_memory_footprint();
        
        println!("   Memory profile: {:?}, footprint: {}MB persistent + {}MB working", 
                 profile, footprint.persistent_state_mb, footprint.working_set_mb);

        // 2. Pin persistent state based on profile
        let pin_priority = match profile {
            ActorMemoryProfile::Hot => {
                println!("   🔥 High-influence user - using TailLatencyCritical pinning");
                PinPriority::TailLatencyCritical
            },
            ActorMemoryProfile::Warm => {
                println!("   🟡 Medium-influence user - using QueryCritical pinning");
                PinPriority::QueryCritical
            },
            ActorMemoryProfile::Cold => {
                println!("   🟦 Low-influence user - using Background pinning");
                PinPriority::Background
            },
        };

        // 3. Pin the actor's data
        context.memory_manager
            .pin_actor_state(&context.actor_ref, pin_priority)
            .await?;

        // 4. Allocate working set for computations
        context.memory_manager
            .allocate_working_set(
                &context.actor_ref,
                footprint.working_set_mb,
                LifetimeClass::Session,
            )
            .await?;

        // 5. Send prefetch hints for related users (followers/following)
        let related_users = self.get_related_users();
        if !related_users.is_empty() {
            println!("   📡 Sending prefetch hints for {} related users", related_users.len());
            
            let hint = MemoryHint::WillAccessSoon {
                actor_refs: related_users,
                estimated_delay_ms: 1000, // Expect access within 1 second
                access_pattern: AccessPattern::GraphTraversal,
            };
            
            context.memory_manager.send_memory_hint(hint).await?;
        }

        println!("   ✅ Activation complete for user {}", self.user_id);
        Ok(())
    }

    async fn on_deactivate(&mut self, context: &ActorDeactivationContext) -> Result<()> {
        println!("🛑 Deactivating social media user actor for user {}", self.user_id);
        
        // Automatically unpin all memory associated with this actor
        context.memory_manager.unpin_actor_memory(&context.actor_ref).await;
        
        println!("   ✅ Deactivation complete for user {}", self.user_id);
        Ok(())
    }
}

impl SocialMediaUserActor {
    fn get_related_users(&self) -> Vec<AddressableReference> {
        // In real implementation, would return follower/following actor references
        // For demo, return empty list
        vec![]
    }
}

/// Example system that coordinates multiple actors
pub struct SocialMediaAnalyticsSystem {
    memory_manager: Arc<DefaultActorMemoryManager>,
    extent_index: Arc<DefaultExtentIndex>,
    _hint_processor: MemoryHintProcessor,
}

impl SocialMediaAnalyticsSystem {
    pub fn new() -> Self {
        let pin_manager = Arc::new(DefaultPinManager::new(2 * 1024 * 1024 * 1024)); // 2GB budget
        let extent_index = Arc::new(DefaultExtentIndex::new());
        let lifetime_manager = Arc::new(DefaultLifetimeManager::new());

        let (memory_manager, hint_rx) = DefaultActorMemoryManager::new(
            pin_manager,
            extent_index.clone(),
            lifetime_manager,
        );

        let memory_manager = Arc::new(memory_manager);
        let hint_processor = MemoryHintProcessor::new(memory_manager.clone(), hint_rx);

        // Start hint processor in background
        let mut hint_processor_clone = hint_processor;
        tokio::spawn(async move {
            hint_processor_clone.run().await;
        });

        Self {
            memory_manager,
            extent_index,
            _hint_processor: MemoryHintProcessor::new(memory_manager.clone(), hint_rx),
        }
    }

    /// Simulate activating an actor and running computations
    pub async fn process_user(&self, user_id: u64, followers_count: u64) -> Result<f64> {
        let mut actor = SocialMediaUserActor::new(user_id, followers_count);
        
        let actor_ref = AddressableReference {
            addressable_type: SocialMediaUserActor::addressable_type().to_string(),
            key: Key::StringKey { key: user_id.to_string() },
        };

        // 1. Activate actor with memory management
        let activation_context = ActorActivationContext {
            actor_ref: actor_ref.clone(),
            memory_manager: self.memory_manager.clone(),
            extent_index: self.extent_index.clone(),
            local_numa_node: 0, // Assume NUMA node 0
            activation_reason: ActivationReason::FirstInvocation,
        };

        actor.on_activate(&activation_context).await?;

        // 2. Perform computation (benefits from pinned memory)
        let start = SystemTime::now();
        let influence_score = actor.calculate_influence().await?;
        let compute_time = start.elapsed().unwrap();
        
        println!("   💪 Computed influence score: {:.2} in {:?}", 
                 influence_score, compute_time);

        // 3. Get memory statistics
        if let Ok(stats) = self.memory_manager.get_actor_memory_stats(&actor_ref).await {
            println!("   📊 Memory stats: {}MB persistent, {}MB working, {} pinned extents",
                     stats.persistent_state_bytes / (1024 * 1024),
                     stats.working_set_bytes / (1024 * 1024),
                     stats.pinned_extents.len());
        }

        // 4. Simulate some delay before deactivation
        sleep(Duration::from_millis(100)).await;

        // 5. Deactivate actor
        let deactivation_context = ActorDeactivationContext {
            actor_ref: actor_ref.clone(),
            memory_manager: self.memory_manager.clone(),
        };

        actor.on_deactivate(&deactivation_context).await?;

        Ok(influence_score)
    }

    /// Demonstrate memory management statistics
    pub async fn show_memory_stats(&self) {
        let stats = self.memory_manager.pin_manager.stats();
        
        println!("\n📈 Overall Memory Management Statistics:");
        println!("   Total pinned: {:.1}MB ({} slices)", 
                 stats.total_pinned_bytes as f64 / (1024.0 * 1024.0),
                 stats.total_pinned_count);
        println!("   Budget used: {:.1}MB / {:.1}MB ({:.1}%)",
                 stats.budget_used_bytes as f64 / (1024.0 * 1024.0),
                 stats.budget_total_bytes as f64 / (1024.0 * 1024.0),
                 (stats.budget_used_bytes as f64 / stats.budget_total_bytes as f64) * 100.0);
        println!("   Huge pages: {:.1}MB", 
                 stats.huge_page_bytes as f64 / (1024.0 * 1024.0));

        for (priority, priority_stats) in &stats.by_priority {
            println!("   {:?}: {} slices, {:.1}MB, {} evictions",
                     priority,
                     priority_stats.pinned_count,
                     priority_stats.pinned_bytes as f64 / (1024.0 * 1024.0),
                     priority_stats.eviction_count);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎭 Orbit-RS Actor Memory Integration Demo\n");

    let system = SocialMediaAnalyticsSystem::new();

    // Simulate processing users with different influence levels
    let users = vec![
        (1, 50000),      // Medium influence user  
        (2, 5000000),    // High influence user (celebrity)
        (3, 500),        // Low influence user
        (4, 2000000),    // High influence user (influencer)
        (5, 10000),      // Medium influence user
    ];

    println!("Processing {} users with different influence levels:\n", users.len());

    for (user_id, followers) in users {
        let influence = system.process_user(user_id, followers).await?;
        println!("User {} ({}K followers): influence = {:.2}\n", 
                 user_id, followers / 1000, influence);
        
        // Small delay between users
        sleep(Duration::from_millis(200)).await;
    }

    // Show final memory statistics
    system.show_memory_stats().await;

    println!("\n🎉 Demo completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_social_media_actor() {
        let mut actor = SocialMediaUserActor::new(123, 50000);
        assert_eq!(actor.user_id, 123);
        assert_eq!(actor.followers_count, 50000);
        assert_eq!(actor.get_memory_profile(), ActorMemoryProfile::Warm);

        let influence = actor.calculate_influence().await.unwrap();
        assert!(influence > 0.0);
    }

    #[tokio::test] 
    async fn test_memory_profiles() {
        let low_influence = SocialMediaUserActor::new(1, 500);
        let medium_influence = SocialMediaUserActor::new(2, 50000);
        let high_influence = SocialMediaUserActor::new(3, 5000000);

        assert_eq!(low_influence.get_memory_profile(), ActorMemoryProfile::Cold);
        assert_eq!(medium_influence.get_memory_profile(), ActorMemoryProfile::Warm);
        assert_eq!(high_influence.get_memory_profile(), ActorMemoryProfile::Hot);
    }

    #[tokio::test]
    async fn test_analytics_system() {
        let system = SocialMediaAnalyticsSystem::new();
        let influence = system.process_user(123, 10000).await.unwrap();
        assert!(influence > 0.0);
    }
}