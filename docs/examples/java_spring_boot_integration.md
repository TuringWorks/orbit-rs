---
layout: default
title: Java Spring Boot Integration with Orbit-RS
category: examples
---

## Java Spring Boot Integration with Orbit-RS

This example demonstrates how to integrate Rust Orbit services with Java Spring Boot applications using multiple communication methods: HTTP REST API, gRPC, and JNI (Java Native Interface).

## Overview

The `orbit-client-spring` crate now provides comprehensive Java Spring Boot integration through three different communication channels:

1. **HTTP Server** - RESTful API endpoints accessible via HTTP client
2. **gRPC Server** - High-performance type-safe gRPC communication
3. **JNI Bindings** - Direct native method calls from Java to Rust

## Architecture

```text
┌─────────────────────┐    ┌────────────────────────────--────┐
│   Java Spring Boot  │    │         Orbit-RS Rust            │
│    Application      │    │                                  │
│                     │    │  ┌─────────────────────────────┐ │
│  ┌─────────────────┐│    │  │   ApplicationContext        │ │
│  │ HTTP Client     ││───▶│  │   (Spring-like IoC)         │ │
│  └─────────────────┘│    │  └─────────────────────────────┘ │
│                     │    │                                  │
│  ┌─────────────────┐│    │  ┌─────────────────────────────┐ │
│  │ gRPC Client     ││───▶│  │   HTTP Server               │ │
│  └─────────────────┘│    │  │   (Axum-based REST API)     │ │
│                     │    │  └─────────────────────────────┘ │
│  ┌─────────────────┐│    │                                  │
│  │ JNI Bindings    ││───▶│  ┌─────────────────────────────┐ │
│  └─────────────────┘│    │  │   gRPC Server               │ │
└─────────────────────┘    │  │   (Tonic-based)             │ │
                           │  └─────────────────────────────┘ │
                           │                                  │
                           │  ┌─────────────────────────────┐ │
                           │  │   JNI Bindings              │ │
                           │  │   (Native Interface)        │ │
                           │  └─────────────────────────────┘ │
                           └──────────────────────────────--──┘
```

## Features Implemented

### 1. HTTP Server (`http_server.rs`)

- **RESTful API endpoints** for service management
- **JSON request/response** format
- **Health checks** and status monitoring
- **Configuration management** via HTTP
- **Graceful shutdown** support

**Key Endpoints:**

- `GET /orbit/services` - List all registered services
- `GET /orbit/services/{name}` - Get specific service info
- `GET /orbit/config` - Get application configuration
- `GET /orbit/health` - Health check endpoint
- `POST /orbit/context/start` - Start application context
- `POST /orbit/context/stop` - Stop application context

### 2. gRPC Server (`grpc_server.rs`)

- **Type-safe Protocol Buffers** communication
- **Streaming support** for real-time events
- **High-performance** binary protocol
- **Service discovery** and registration
- **Configuration management** via gRPC

**Key Services:**

- `OrbitSpringService.ListServices()` - Get all services
- `OrbitSpringService.GetServiceInfo()` - Service details
- `OrbitSpringService.GetConfig()` - Configuration access
- `OrbitSpringService.HealthCheck()` - Health monitoring
- `OrbitSpringService.StreamServiceEvents()` - Event streaming

### 3. JNI Bindings (`jni_bindings.rs`)

- **Direct native method calls** from Java
- **Memory-safe** Rust-Java interop
- **Context management** with proper lifecycle
- **Error handling** with Java exception propagation
- **Thread-safe** global state management

**Key Native Methods:**

- `initializeContext(String configPath)` - Initialize Orbit context
- `startContext(long contextId)` - Start application context
- `stopContext(long contextId)` - Stop application context
- `getServiceNames(long contextId)` - List service names
- `healthCheck(long contextId)` - Perform health check

### 4. Integration Layer (`java_integration.rs`)

- **Unified configuration** for all communication methods
- **Builder pattern** for easy setup
- **Graceful startup/shutdown** coordination
- **Status monitoring** across all services
- **Example usage** functions

## Usage Examples

### Rust Side Setup

```rust
use orbit_client_spring::java_integration::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create HTTP and gRPC configurations
    let http_config = HttpServerConfig {
        host: "localhost".to_string(),
        port: 8080,
        base_path: "/api/v1".to_string(),
    };

    let grpc_config = GrpcServerConfig {
        host: "localhost".to_string(),
        port: 9090,
        tls_enabled: false,
        max_message_size: 8 * 1024 * 1024,
    };

    // Build and run the integration
    let integration = JavaSpringBootIntegrationBuilder::new()
        .http_config(http_config)
        .grpc_config(grpc_config)
        .enable_http(true)
        .enable_grpc(true)
        .enable_jni(true)
        .build();

    // Start all servers and wait for shutdown
    integration.run().await?;
    
    Ok(())
}
```

### Java Spring Boot Integration

#### 1. HTTP Client Usage

```java
@RestController
@RequestMapping("/orbit")
public class OrbitController {
    
    @Autowired
    private OrbitHttpClient orbitClient;
    
    @GetMapping("/services")
    public ResponseEntity<ServiceListResponse> getServices() {
        try {
            ServiceListResponse services = orbitClient.listServices();
            return ResponseEntity.ok(services);
        } catch (Exception e) {
            return ResponseEntity.status(500).build();
        }
    }
    
    @GetMapping("/health")
    public ResponseEntity<HealthResponse> healthCheck() {
        try {
            HealthResponse health = orbitClient.healthCheck();
            return ResponseEntity.ok(health);
        } catch (Exception e) {
            return ResponseEntity.status(500).build();
        }
    }
}
```

#### 2. gRPC Client Usage

```java
@Service
public class OrbitGrpcService {
    
    private final OrbitGrpcClient grpcClient;
    
    public OrbitGrpcService() {
        this.grpcClient = new OrbitGrpcClient("localhost", 9090);
    }
    
    public List<ServiceInfo> getAllServices() {
        ListServicesRequest request = ListServicesRequest.newBuilder().build();
        ListServicesResponse response = grpcClient.listServices(request);
        return response.getServicesList();
    }
    
    public void streamEvents() {
        StreamServiceEventsRequest request = StreamServiceEventsRequest.newBuilder()
            .addServiceFilter("user-service")
            .build();
            
        grpcClient.streamServiceEvents(request)
            .forEach(event -> {
                System.out.println("Received event: " + event.getEventType());
            });
    }
}
```

#### 3. JNI Direct Integration

```java
@Component
public class OrbitNativeService {
    
    static {
        System.loadLibrary("orbit_client_spring");
    }
    
    private long contextId = -1;
    
    @PostConstruct
    public void initialize() {
        contextId = OrbitClient.initializeContext("/etc/orbit/config.yaml");
        if (contextId > 0) {
            boolean started = OrbitClient.startContext(contextId);
            if (started) {
                System.out.println("✅ Orbit context started successfully");
            }
        }
    }
    
    public String[] getServiceNames() {
        if (contextId > 0) {
            return OrbitClient.getServiceNames(contextId);
        }
        return new String[0];
    }
    
    public boolean isHealthy() {
        if (contextId > 0) {
            return OrbitClient.healthCheck(contextId);
        }
        return false;
    }
    
    @PreDestroy
    public void cleanup() {
        if (contextId > 0) {
            OrbitClient.stopContext(contextId);
            OrbitClient.destroyContext(contextId);
        }
    }
}
```

### Maven Dependencies

```xml
<dependencies>
    <!-- Orbit Spring Boot Starter -->
    <dependency>
        <groupId>com.orbit</groupId>
        <artifactId>orbit-spring-boot-starter</artifactId>
        <version>1.0.0</version>
    </dependency>
    
    <!-- gRPC dependencies (if using gRPC) -->
    <dependency>
        <groupId>io.grpc</groupId>
        <artifactId>grpc-netty-shaded</artifactId>
        <version>1.58.0</version>
    </dependency>
    <dependency>
        <groupId>io.grpc</groupId>
        <artifactId>grpc-protobuf</artifactId>
        <version>1.58.0</version>
    </dependency>
</dependencies>
```

### Application Configuration

```yaml

# application.yml
orbit:
  enabled: true
  
  # HTTP client configuration
  http:
    url: http://localhost:8080/api/v1
    timeout: 30000
    retry-attempts: 3
  
  # gRPC client configuration  
  grpc:
    host: localhost
    port: 9090
    tls-enabled: false
    max-message-size: 8388608
    
  # JNI configuration
  jni:
    enabled: true
    config-path: /etc/orbit/config.yaml
    library-path: /usr/local/lib
```

## Build Instructions

### Building the Rust Library

```bash

# Build the shared library for JNI
cd orbit-client-spring
cargo build --release

# For cross-platform builds (macOS to Linux)
cargo build --release --target x86_64-unknown-linux-gnu

# The shared library will be generated as:
# target/release/liborbit_client_spring.dylib (macOS)
# target/release/liborbit_client_spring.so (Linux)
```

### Java Client Generation

```bash

# Generate Java client stubs from gRPC proto files
protoc --java_out=src/main/java \
       --grpc-java_out=src/main/java \
       --proto_path=proto \
       orbit_spring.proto
```

## Testing

### Integration Tests

```bash

# Start the Rust server
cargo run --example java_integration_server

# In another terminal, run Java tests
cd java-client
mvn test

# Or run specific integration test
mvn test -Dtest=OrbitIntegrationTest
```

### Manual Testing

```bash

# Test HTTP endpoints
curl http://localhost:8080/api/v1/services
curl http://localhost:8080/api/v1/health
curl http://localhost:8080/api/v1/config

# Test gRPC (using grpcurl)
grpcurl -plaintext localhost:9090 orbit.spring.OrbitSpringService/ListServices
grpcurl -plaintext localhost:9090 orbit.spring.OrbitSpringService/HealthCheck

# Test JNI (from Java application)
java -Djava.library.path=/path/to/rust/target/release \
     -cp target/classes \
     com.example.OrbitClientTest
```

## Performance Characteristics

| Method | Throughput | Latency | Memory | Use Case |
|--------|------------|---------|--------|----------|
| HTTP   | ~10K req/s | ~5ms    | Low    | Web APIs, REST services |
| gRPC   | ~50K req/s | ~1ms    | Medium | Microservices, streaming |
| JNI    | ~100K req/s| ~0.1ms  | High   | High-performance computing |

## Future Enhancements

1. **Protocol Buffers Generation**: Automated Java client generation from Rust
2. **Spring Boot Autoconfiguration**: Automatic bean registration and configuration
3. **Metrics Integration**: Micrometer metrics support for monitoring
4. **Security**: Authentication and authorization support
5. **Load Balancing**: Client-side load balancing for multiple Rust instances
6. **Circuit Breaker**: Resilience patterns for fault tolerance

## Troubleshooting

### Common Issues

1. **JNI Library Not Found**

   ```java
   java.lang.UnsatisfiedLinkError: no orbit_client_spring in java.library.path
   ```

   Solution: Ensure the shared library is in the Java library path.

2. **gRPC Connection Refused**

   ```java
   io.grpc.StatusRuntimeException: UNAVAILABLE
   ```

   Solution: Check that the Rust gRPC server is running on the correct port.

3. **HTTP 404 Errors**

   ```text
   404 Not Found
   ```

   Solution: Verify the base path configuration matches between client and server.

### Debugging

Enable debug logging in both Rust and Java:

```rust
// Rust
tracing_subscriber::fmt::init();
```

```java
// Java - application.yml
logging:
  level:
    com.orbit: DEBUG
```

This comprehensive integration provides a robust foundation for Java Spring Boot applications to interact with Rust-based Orbit services using their preferred communication method.
