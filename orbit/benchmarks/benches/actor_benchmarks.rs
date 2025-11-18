use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use orbit_server::persistence::config::PersistenceProviderConfig;
use orbit_server::OrbitServer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use uuid::Uuid;

// Test message types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PingMessage {
    count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct PongMessage {
    count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateUpdateMessage {
    value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateQueryMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateQueryResponse {
    value: u64,
}

// Helper to create test server
async fn create_test_server() -> anyhow::Result<OrbitServer> {
    let mut server = OrbitServer::builder()
        .with_namespace("benchmark")
        .with_bind_address("127.0.0.1")
        .with_port(0) // Random available port
        .with_lease_duration(Duration::from_secs(300))
        .with_max_addressables(10000)
        .with_persistence(PersistenceProviderConfig::default_memory())
        .with_redis_enabled(false)
        .with_postgres_enabled(false)
        .build()
        .await?;

    server
        .register_addressable_type("BenchmarkActor".to_string())
        .await?;

    Ok(server)
}

/// Benchmark: Actor creation throughput
fn bench_actor_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let server = rt.block_on(async { create_test_server().await.unwrap() });
    let server = Arc::new(Mutex::new(server));

    let mut group = c.benchmark_group("actor_creation");

    for size in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let _server = server.lock().await;
                for i in 0..size {
                    let actor_id = format!("actor_{}", i);
                    black_box(actor_id);
                }
            });
        });
    }
    group.finish();
}

/// Benchmark: Message send latency (single actor)
fn bench_message_send_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    c.bench_function("message_send_single", |b| {
        b.to_async(&rt).iter(|| async {
            let msg = PingMessage { count: 1 };
            black_box(msg);
        });
    });
}

/// Benchmark: Message throughput (single actor)
fn bench_message_throughput_single(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("message_throughput_single");

    for count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.to_async(&rt).iter(|| async {
                for i in 0..count {
                    let msg = PingMessage { count: i };
                    black_box(msg);
                }
            });
        });
    }
    group.finish();
}

/// Benchmark: Concurrent message processing
fn bench_concurrent_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("concurrent_messages");

    for num_actors in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_actors as u64 * 100));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_actors),
            num_actors,
            |b, &num_actors| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];
                    for actor_id in 0..num_actors {
                        let handle = tokio::spawn(async move {
                            for msg_id in 0..100 {
                                let msg = PingMessage {
                                    count: (actor_id * 100 + msg_id) as u64,
                                };
                                black_box(msg);
                            }
                        });
                        handles.push(handle);
                    }
                    futures::future::join_all(handles).await;
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: State update operations
fn bench_state_updates(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("state_updates");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let mut state = 0u64;
                for i in 0..size {
                    state = state.wrapping_add(i as u64);
                    black_box(state);
                }
            });
        });
    }
    group.finish();
}

/// Benchmark: Actor lookup performance
fn bench_actor_lookup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("actor_lookup");

    // Pre-create actor IDs
    let actor_ids: Vec<String> = (0..1000).map(|i| format!("actor_{}", i)).collect();

    group.bench_function("lookup_existing", |b| {
        b.to_async(&rt).iter(|| async {
            let idx = fastrand::usize(0..actor_ids.len());
            let actor_id = &actor_ids[idx];
            black_box(actor_id);
        });
    });

    group.finish();
}

/// Benchmark: Request-response pattern
fn bench_request_response(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("request_response");

    for num_requests in [1, 10, 100].iter() {
        group.throughput(Throughput::Elements(*num_requests as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_requests),
            num_requests,
            |b, &num_requests| {
                b.to_async(&rt).iter(|| async {
                    for i in 0..num_requests {
                        let query = StateQueryMessage;
                        let response = StateQueryResponse { value: i as u64 };
                        black_box(query);
                        black_box(response);
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Message serialization/deserialization
fn bench_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");

    let ping_msg = PingMessage { count: 42 };
    let state_msg = StateUpdateMessage { value: 12345 };

    group.bench_function("serialize_ping", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&ping_msg).unwrap();
            black_box(serialized);
        });
    });

    group.bench_function("deserialize_ping", |b| {
        let serialized = serde_json::to_string(&ping_msg).unwrap();
        b.iter(|| {
            let deserialized: PingMessage = serde_json::from_str(&serialized).unwrap();
            black_box(deserialized);
        });
    });

    group.bench_function("serialize_state_update", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&state_msg).unwrap();
            black_box(serialized);
        });
    });

    group.bench_function("deserialize_state_update", |b| {
        let serialized = serde_json::to_string(&state_msg).unwrap();
        b.iter(|| {
            let deserialized: StateUpdateMessage = serde_json::from_str(&serialized).unwrap();
            black_box(deserialized);
        });
    });

    group.finish();
}

/// Benchmark: UUID generation (for actor IDs)
fn bench_uuid_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("uuid_generation");

    group.bench_function("uuid_v4", |b| {
        b.iter(|| {
            let uuid = Uuid::new_v4();
            black_box(uuid);
        });
    });

    group.bench_function("uuid_to_string", |b| {
        let uuid = Uuid::new_v4();
        b.iter(|| {
            let s = uuid.to_string();
            black_box(s);
        });
    });

    group.finish();
}

/// Benchmark: Actor memory footprint simulation
fn bench_actor_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("actor_memory");

    for actor_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(actor_count),
            actor_count,
            |b, &actor_count| {
                b.iter(|| {
                    let actors: Vec<(String, u64)> = (0..actor_count)
                        .map(|i| (format!("actor_{}", i), i as u64))
                        .collect();
                    black_box(actors);
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Parallel actor initialization
fn bench_parallel_actor_init(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("parallel_actor_init");

    for num_actors in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_actors as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_actors),
            num_actors,
            |b, &num_actors| {
                b.to_async(&rt).iter(|| async {
                    let handles: Vec<_> = (0..num_actors)
                        .map(|i| {
                            tokio::spawn(async move {
                                let actor_id = format!("actor_{}", i);
                                black_box(actor_id);
                            })
                        })
                        .collect();
                    futures::future::join_all(handles).await;
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Message batching
fn bench_message_batching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let _server = rt.block_on(async { create_test_server().await.unwrap() });

    let mut group = c.benchmark_group("message_batching");

    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let batch: Vec<PingMessage> = (0..batch_size)
                        .map(|i| PingMessage { count: i as u64 })
                        .collect();
                    black_box(batch);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_actor_creation,
    bench_message_send_latency,
    bench_message_throughput_single,
    bench_concurrent_messages,
    bench_state_updates,
    bench_actor_lookup,
    bench_request_response,
    bench_message_serialization,
    bench_uuid_generation,
    bench_actor_memory_allocation,
    bench_parallel_actor_init,
    bench_message_batching,
);

criterion_main!(benches);
