---
layout: default
title: Kotlin/JVM to Rust Dependency Mapping
category: development
---

# Kotlin/JVM to Rust Dependency Mapping

## Core Runtime Dependencies

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| kotlin-stdlib-jdk8 | 1.3.72 | std library | - | Built into Rust |
| kotlinx-coroutines-core | 1.3.5 | tokio | 1.0+ | Async runtime |
| kotlinx-coroutines-jdk8 | 1.3.5 | tokio | 1.0+ | JDK8 integration not needed |
| kotlinx-coroutines-guava | 1.3.5 | futures | 0.3+ | Future utilities |

## Serialization

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| jackson-databind | 2.10.2 | serde_json | 1.0+ | JSON serialization |
| jackson-module-kotlin | 2.10.2 | serde | 1.0+ | Kotlin reflection → Rust derive macros |

## gRPC and Protocol Buffers

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| grpc-netty-shaded | 1.29.0 | tonic | 0.12+ | gRPC implementation |
| grpc-kotlin-stub | 0.1.4 | tonic | 0.12+ | Integrated in tonic |
| protobuf-java | 3.11.1 | prost | 0.13+ | Protocol Buffer implementation |
| protobuf-kotlin | 3.11.1 | prost | 0.13+ | Built-in with prost |

## Logging

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| kotlin-logging | 1.7.8 | tracing | 0.1+ | Structured logging |
| slf4j-api | 1.7.30 | tracing | 0.1+ | Logging abstraction |
| slf4j-simple | 1.7.30 | tracing-subscriber | 0.3+ | Console logging |

## Metrics

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| micrometer-core | 1.3.5 | metrics | 0.23+ | Metrics collection |
| - | - | metrics-exporter-prometheus | 0.15+ | Prometheus integration |

## Utilities

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| kotlin-reflect | Built-in | - | - | Using trait objects and generics instead |
| classgraph | 4.8.80 | - | - | Not needed, using compile-time registration |

## Collections and Data Structures

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| ConcurrentHashMap | JDK | DashMap | 6.0+ | Lock-free concurrent HashMap |
| AtomicReference | JDK | std::sync::atomic | - | Built into Rust std |
| ReentrantLock | JDK | tokio::sync::Mutex | - | Async-aware mutex |

## Testing

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| kotlintest-runner-junit5 | 3.4.2 | - | - | Built-in Rust testing |
| mockito-core | 3.3.3 | mockall | 0.13+ | Mock object framework |
| mockito-kotlin | 2.2.0 | mockall | 0.13+ | Integrated in mockall |

## Time and UUID

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| java.time | JDK | chrono | 0.4+ | Date/time handling |
| java.util.UUID | JDK | uuid | 1.0+ | UUID generation |

## Error Handling

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| Kotlin Result/Either | Built-in | Result<T, E> | - | Built into Rust |
| - | - | anyhow | 1.0+ | Error context and chaining |
| - | - | thiserror | 1.0+ | Custom error types |

## External Integrations

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| etcd-java | - | etcd-rs | 2.0+ | etcd client |
| Spring Framework | - | axum/warp | Latest | HTTP server framework |

## Build and Benchmarking

| Kotlin/JVM | Version | Rust Equivalent | Version | Notes |
|------------|---------|-----------------|---------|-------|
| Gradle | 6.x | Cargo | Built-in | Build system |
| JMH | - | criterion | 0.5+ | Benchmarking framework |

## Architectural Changes

### From JVM Dynamic Proxies to Rust Traits

The original Kotlin implementation uses Java's dynamic proxy mechanism for actor invocations. In Rust, this is replaced with:

- **Trait objects**: `Box<dyn ActorTrait>` for dynamic dispatch
- **Macros**: Code generation for actor proxy creation
- **Generic programming**: Compile-time polymorphism where possible

### From Kotlin Coroutines to Rust Async/Await

- **Suspend functions** → **async fn**
- **Deferred<T>** → **Future<Output = T>**
- **GlobalScope.launch** → **tokio::spawn**
- **runBlocking** → **tokio::runtime::Runtime::block_on**

### From Reflection to Code Generation

Kotlin's runtime reflection is replaced with:
- **Procedural macros** for actor trait generation
- **build.rs** scripts for protocol buffer compilation
- **Compile-time trait resolution** instead of runtime class scanning

### Memory Management

- **Garbage Collection** → **RAII and ownership**
- **Reference counting** when needed via `Arc<T>` and `Rc<T>`
- **Weak references** via `Weak<T>` for cycle breaking