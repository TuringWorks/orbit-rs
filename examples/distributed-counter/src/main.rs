use async_trait::async_trait;
use orbit_client::OrbitClient;
use orbit_server::OrbitServer;
use orbit_shared::{
    addressable::{Addressable, AddressableReference, Key},
    exception::OrbitResult,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::time::Duration;
use tracing::{error, info};

/// A distributed counter actor that can coordinate with other counters
#[derive(Debug, Clone)]
pub struct CounterActor {
    pub name: String,
    pub value: i64,
    pub operation_count: u64,
}

impl CounterActor {
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: 0,
            operation_count: 0,
        }
    }
}

#[async_trait]
impl Addressable for CounterActor {
    fn addressable_type() -> &'static str {
        "CounterActor"
    }
}

/// Messages for counter operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementMessage {
    pub amount: i64,
    pub requester: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecrementMessage {
    pub amount: i64,
    pub requester: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMessage {
    pub from_counter: String,
    pub sync_value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterState {
    pub name: String,
    pub value: i64,
    pub operation_count: u64,
}

/// Methods available on counter actors
pub trait CounterMethods {
    async fn increment(&mut self, message: IncrementMessage) -> OrbitResult<i64>;
    async fn decrement(&mut self, message: DecrementMessage) -> OrbitResult<i64>;
    async fn get_value(&self) -> OrbitResult<i64>;
    async fn get_state(&self) -> OrbitResult<CounterState>;
    async fn sync_with_counter(&mut self, message: SyncMessage) -> OrbitResult<()>;
    async fn reset(&mut self) -> OrbitResult<()>;
}

impl CounterMethods for CounterActor {
    async fn increment(&mut self, message: IncrementMessage) -> OrbitResult<i64> {
        self.value += message.amount;
        self.operation_count += 1;

        info!(
            "Counter '{}' incremented by {} (from '{}'), new value: {}",
            self.name, message.amount, message.requester, self.value
        );

        Ok(self.value)
    }

    async fn decrement(&mut self, message: DecrementMessage) -> OrbitResult<i64> {
        self.value -= message.amount;
        self.operation_count += 1;

        info!(
            "Counter '{}' decremented by {} (from '{}'), new value: {}",
            self.name, message.amount, message.requester, self.value
        );

        Ok(self.value)
    }

    async fn get_value(&self) -> OrbitResult<i64> {
        Ok(self.value)
    }

    async fn get_state(&self) -> OrbitResult<CounterState> {
        Ok(CounterState {
            name: self.name.clone(),
            value: self.value,
            operation_count: self.operation_count,
        })
    }

    async fn sync_with_counter(&mut self, message: SyncMessage) -> OrbitResult<()> {
        info!(
            "Counter '{}' syncing with '{}': adjusting value from {} to {}",
            self.name, message.from_counter, self.value, message.sync_value
        );

        self.value = message.sync_value;
        self.operation_count += 1;

        Ok(())
    }

    async fn reset(&mut self) -> OrbitResult<()> {
        info!("Counter '{}' reset from value {}", self.name, self.value);
        self.value = 0;
        self.operation_count += 1;
        Ok(())
    }
}

/// A coordinator actor that manages multiple counter actors
#[derive(Debug, Clone)]
pub struct CounterCoordinatorActor {
    pub name: String,
    pub managed_counters: Vec<String>,
    pub total_operations: u64,
}

impl CounterCoordinatorActor {
    pub fn new(name: String) -> Self {
        Self {
            name,
            managed_counters: Vec::new(),
            total_operations: 0,
        }
    }
}

#[async_trait]
impl Addressable for CounterCoordinatorActor {
    fn addressable_type() -> &'static str {
        "CounterCoordinatorActor"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCounterMessage {
    pub counter_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastIncrementMessage {
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorStats {
    pub name: String,
    pub managed_counters: Vec<String>,
    pub total_operations: u64,
}

pub trait CoordinatorMethods {
    async fn register_counter(&mut self, message: RegisterCounterMessage) -> OrbitResult<()>;
    async fn broadcast_increment(
        &mut self,
        message: BroadcastIncrementMessage,
    ) -> OrbitResult<Vec<i64>>;
    async fn get_all_counter_states(&self) -> OrbitResult<Vec<CounterState>>;
    async fn sync_all_counters(&mut self) -> OrbitResult<()>;
    async fn get_stats(&self) -> OrbitResult<CoordinatorStats>;
}

impl CoordinatorMethods for CounterCoordinatorActor {
    async fn register_counter(&mut self, message: RegisterCounterMessage) -> OrbitResult<()> {
        if !self.managed_counters.contains(&message.counter_name) {
            self.managed_counters.push(message.counter_name.clone());
            info!(
                "Coordinator '{}' registered counter '{}'",
                self.name, message.counter_name
            );
        }
        Ok(())
    }

    async fn broadcast_increment(
        &mut self,
        message: BroadcastIncrementMessage,
    ) -> OrbitResult<Vec<i64>> {
        self.total_operations += 1;
        info!(
            "Coordinator '{}' broadcasting increment of {} to {} counters",
            self.name,
            message.amount,
            self.managed_counters.len()
        );

        // In a real implementation, we would send messages to all managed counters
        // For this example, we'll simulate the results
        let results: Vec<i64> = self
            .managed_counters
            .iter()
            .enumerate()
            .map(|(i, _)| (i as i64 + 1) * message.amount)
            .collect();

        Ok(results)
    }

    async fn get_all_counter_states(&self) -> OrbitResult<Vec<CounterState>> {
        // In a real implementation, we would query all managed counters
        // For this example, we'll return mock data
        let states: Vec<CounterState> = self
            .managed_counters
            .iter()
            .enumerate()
            .map(|(i, name)| CounterState {
                name: name.clone(),
                value: i as i64 * 10,
                operation_count: i as u64 * 5,
            })
            .collect();

        Ok(states)
    }

    async fn sync_all_counters(&mut self) -> OrbitResult<()> {
        self.total_operations += 1;
        info!(
            "Coordinator '{}' syncing all {} counters",
            self.name,
            self.managed_counters.len()
        );

        // In a real implementation, we would send sync messages to all counters

        Ok(())
    }

    async fn get_stats(&self) -> OrbitResult<CoordinatorStats> {
        Ok(CoordinatorStats {
            name: self.name.clone(),
            managed_counters: self.managed_counters.clone(),
            total_operations: self.total_operations,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting Orbit Distributed Counter example");

    // Start Orbit server
    info!("Starting Orbit server...");
    let mut server = OrbitServer::builder().with_port(8080).build().await?;

    // Register actor types
    server
        .register_addressable_type("CounterActor".to_string())
        .await?;
    server
        .register_addressable_type("CounterCoordinatorActor".to_string())
        .await?;

    let _server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            error!("Server error: {}", e);
        }
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Connect client
    info!("Connecting Orbit client...");
    let client = OrbitClient::builder()
        .with_server_urls(vec!["http://localhost:8080".to_string()])
        .build()
        .await?;

    // Create counter actors
    let counter1_ref = AddressableReference {
        addressable_type: "CounterActor".to_string(),
        key: Key::StringKey {
            key: "counter-1".to_string(),
        },
    };
    let counter2_ref = AddressableReference {
        addressable_type: "CounterActor".to_string(),
        key: Key::StringKey {
            key: "counter-2".to_string(),
        },
    };
    let counter3_ref = AddressableReference {
        addressable_type: "CounterActor".to_string(),
        key: Key::StringKey {
            key: "counter-3".to_string(),
        },
    };

    // Create coordinator actor
    let coordinator_ref = AddressableReference {
        addressable_type: "CounterCoordinatorActor".to_string(),
        key: Key::StringKey {
            key: "main-coordinator".to_string(),
        },
    };

    let counter1 = client
        .actor_reference::<CounterActor>(counter1_ref.key.clone())
        .await?;
    let counter2 = client
        .actor_reference::<CounterActor>(counter2_ref.key.clone())
        .await?;
    let counter3 = client
        .actor_reference::<CounterActor>(counter3_ref.key.clone())
        .await?;
    let coordinator = client
        .actor_reference::<CounterCoordinatorActor>(coordinator_ref.key.clone())
        .await?;

    info!("Setting up distributed counter system...");

    // Register counters with coordinator
    coordinator
        .invoke::<()>(
            "register_counter",
            vec![serde_json::to_value(RegisterCounterMessage {
                counter_name: "counter-1".to_string(),
            })?],
        )
        .await?;
    coordinator
        .invoke::<()>(
            "register_counter",
            vec![serde_json::to_value(RegisterCounterMessage {
                counter_name: "counter-2".to_string(),
            })?],
        )
        .await?;
    coordinator
        .invoke::<()>(
            "register_counter",
            vec![serde_json::to_value(RegisterCounterMessage {
                counter_name: "counter-3".to_string(),
            })?],
        )
        .await?;

    // Perform individual counter operations
    info!("Performing individual counter operations...");

    let val1: i64 = counter1
        .invoke(
            "increment",
            vec![serde_json::to_value(IncrementMessage {
                amount: 5,
                requester: "main".to_string(),
            })?],
        )
        .await?;
    info!("Counter 1 value after increment: {}", val1);

    let val2: i64 = counter2
        .invoke(
            "increment",
            vec![serde_json::to_value(IncrementMessage {
                amount: 10,
                requester: "main".to_string(),
            })?],
        )
        .await?;
    info!("Counter 2 value after increment: {}", val2);

    let val3: i64 = counter3
        .invoke(
            "increment",
            vec![serde_json::to_value(IncrementMessage {
                amount: 15,
                requester: "main".to_string(),
            })?],
        )
        .await?;
    info!("Counter 3 value after increment: {}", val3);

    // Perform decrement operations
    let _: i64 = counter1
        .invoke(
            "decrement",
            vec![serde_json::to_value(DecrementMessage {
                amount: 2,
                requester: "main".to_string(),
            })?],
        )
        .await?;

    // Get all counter states
    info!("Getting individual counter states...");
    let state1: CounterState = counter1.invoke("get_state", vec![]).await?;
    let state2: CounterState = counter2.invoke("get_state", vec![]).await?;
    let state3: CounterState = counter3.invoke("get_state", vec![]).await?;

    info!("Counter states: {:?}, {:?}, {:?}", state1, state2, state3);

    // Use coordinator for broadcast operations
    info!("Using coordinator for broadcast operations...");
    let broadcast_results: Vec<i64> = coordinator
        .invoke(
            "broadcast_increment",
            vec![serde_json::to_value(BroadcastIncrementMessage {
                amount: 3,
            })?],
        )
        .await?;
    info!("Broadcast increment results: {:?}", broadcast_results);

    // Get coordinator stats
    let coord_stats: CoordinatorStats = coordinator.invoke("get_stats", vec![]).await?;
    info!("Coordinator stats: {:?}", coord_stats);

    // Demonstrate counter synchronization
    info!("Demonstrating counter synchronization...");
    let _: () = counter1
        .invoke(
            "sync_with_counter",
            vec![serde_json::to_value(SyncMessage {
                from_counter: "coordinator".to_string(),
                sync_value: 100,
            })?],
        )
        .await?;

    let synced_value: i64 = counter1.invoke("get_value", vec![]).await?;
    info!("Counter 1 value after sync: {}", synced_value);

    // Reset all counters
    info!("Resetting all counters...");
    let _: () = counter1.invoke("reset", vec![]).await?;
    let _: () = counter2.invoke("reset", vec![]).await?;
    let _: () = counter3.invoke("reset", vec![]).await?;

    // Final state check
    let final_val1: i64 = counter1.invoke("get_value", vec![]).await?;
    let final_val2: i64 = counter2.invoke("get_value", vec![]).await?;
    let final_val3: i64 = counter3.invoke("get_value", vec![]).await?;
    info!(
        "Final counter values: {}, {}, {}",
        final_val1, final_val2, final_val3
    );

    // Cleanup
    info!("Deactivating actors...");
    client.deactivate_actor(&counter1_ref).await?;
    client.deactivate_actor(&counter2_ref).await?;
    client.deactivate_actor(&counter3_ref).await?;
    client.deactivate_actor(&coordinator_ref).await?;

    client.shutdown().await?;
    info!("Distributed Counter example completed");

    Ok(())
}
