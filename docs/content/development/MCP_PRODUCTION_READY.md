# MCP Server - Production Ready Guide

## Overview

The Orbit-RS MCP Server is now production-ready with comprehensive natural language query processing, SQL generation, result formatting, and Orbit-RS integration.

## Completed Features

### Core Components

1. **Natural Language Processing** (`nlp.rs`)
   - Intent classification (SELECT, INSERT, UPDATE, DELETE, ANALYZE)
   - Entity recognition (tables, columns, values)
   - Condition extraction
   - Confidence scoring

2. **SQL Generation** (`sql_generator.rs`)
   - Schema-aware query building
   - Parameter binding for security
   - Complexity estimation
   - Optimization hints

3. **Result Processing** (`result_processor.rs`)
   - Data summarization
   - Statistical analysis
   - Visualization hints
   - Data preview formatting

4. **Schema Management** (`schema.rs`, `schema_discovery.rs`)
   - Thread-safe caching with TTL
   - Real-time schema discovery
   - Schema change notifications
   - Cache statistics

5. **Orbit-RS Integration** (`integration.rs`)
   - Query execution via PostgreSQL wire protocol
   - Schema discovery from Orbit-RS
   - Result conversion
   - Type mapping

6. **ML Framework** (`ml_nlp.rs`)
   - ML model integration framework
   - Hybrid ML + rule-based processing
   - Model management
   - Confidence-based fallback

7. **MCP Tools** (`tools.rs`)
   - `query_data`: Natural language queries
   - `describe_schema`: Schema information
   - `analyze_data`: Statistical analysis
   - `list_tables`: Table listing

## Testing

### End-to-End Tests

Comprehensive test suite in `orbit/protocols/tests/mcp_integration_test.rs`:

- Natural language to SQL generation
- Intent classification
- Entity extraction
- Condition extraction
- Aggregation detection
- Limit extraction
- Result processing
- SQL parameter binding
- Complex query generation
- Error handling
- Query complexity estimation
- Optimization hints

### Running Tests

```bash
# Run all MCP integration tests
cargo test --package orbit-protocols --test mcp_integration_test

# Run specific test
cargo test --package orbit-protocols --test mcp_integration_test test_natural_language_to_sql_generation
```

## Production Deployment

### Kubernetes Deployment

Deployment configuration available:

**Features:**
- 3 replicas with rolling updates
- Horizontal Pod Autoscaling (3-20 pods)
- Pod Disruption Budget (min 2 available)
- Health and readiness probes
- Resource limits and requests
- TLS/SSL support via Ingress
- Prometheus metrics integration
- ConfigMap-based configuration
- Secret management for API keys

**Deployment Steps:**

1. **Create namespace:**
   ```bash
   kubectl create namespace orbit-production
   ```

2. **Create secrets:**
   ```bash
   kubectl create secret generic orbit-mcp-secrets \
     --from-literal=api_key=$(openssl rand -base64 32) \
     -n orbit-production
   ```

3. **Apply configuration:**
   ```bash

   ```

4. **Verify deployment:**
   ```bash
   kubectl get pods -n orbit-production -l app=orbit-mcp-server
   kubectl get svc -n orbit-production orbit-mcp-server
   ```

### Configuration

Key configuration options in `mcp_config.yaml`:

```yaml
server:
  port: 8080
  host: "0.0.0.0"

security:
  require_authentication: true
  api_key_required: true

limits:
  max_concurrent_requests: 100
  request_timeout_ms: 30000
  max_query_result_size: 10000

features:
  enable_sql_queries: true
  enable_vector_operations: true
  enable_ml_models: false  # Enable when models are available

nlp:
  use_ml_models: false
  confidence_threshold: 0.7

schema:
  discovery_interval_secs: 300
  enable_realtime_updates: true
```

## Monitoring

### Metrics

The MCP server exposes Prometheus metrics:

- `mcp_queries_total`: Total queries processed
- `mcp_query_duration_seconds`: Query processing time
- `mcp_nlp_processing_duration_seconds`: NLP processing time
- `mcp_sql_generation_duration_seconds`: SQL generation time
- `mcp_schema_cache_hits`: Schema cache hit rate
- `mcp_errors_total`: Error count by type

### Health Endpoints

- `GET /health`: Liveness probe
- `GET /ready`: Readiness probe
- `GET /metrics`: Prometheus metrics

## 🔧 Real-Time Schema Discovery

### Setup

```rust
use orbit_protocols::mcp::{
    schema::SchemaAnalyzer,
    schema_discovery::SchemaDiscoveryManager,
    integration::OrbitMcpIntegration,
};

// Create components
let schema_analyzer = Arc::new(SchemaAnalyzer::new());
let integration = Arc::new(OrbitMcpIntegration::with_storage(
    query_engine,
    storage,
));

// Create discovery manager
let discovery_manager = SchemaDiscoveryManager::new(
    schema_analyzer,
    integration,
    300, // Refresh every 5 minutes
);

// Start background discovery
discovery_manager.start_discovery().await?;
```

### Schema Change Notifications

```rust
use orbit_protocols::mcp::schema_discovery::{SchemaChangeListener, SchemaChange};

struct MyListener;

impl SchemaChangeListener for MyListener {
    fn on_schema_change(&self, change: SchemaChange) {
        match change {
            SchemaChange::TableCreated { table_name, .. } => {
                println!("New table created: {}", table_name);
            }
            SchemaChange::TableModified { table_name, .. } => {
                println!("Table modified: {}", table_name);
            }
            _ => {}
        }
    }
}

// Register listener
let notifier = SchemaChangeNotifier::new();
notifier.register_listener(Arc::new(MyListener)).await;
```

## ML Model Integration

### Framework Ready

The ML NLP framework is in place for future model integration:

```rust
use orbit_protocols::mcp::ml_nlp::{MlNlpProcessor, MlModelManager, MlModelConfig};

// Create ML processor
let ml_processor = MlNlpProcessor::new();

// Load models (when available)
let mut model_manager = MlModelManager::new();
model_manager.load_intent_model(
    "bert-intent".to_string(),
    MlModelConfig {
        model_path: Some("/models/intent.onnx".to_string()),
        confidence_threshold: 0.7,
        enable_gpu: true,
        ..Default::default()
    },
).await?;
```

### Future ML Integration

To integrate actual ML models:

1. **Add ML dependencies** to `Cargo.toml`:
   ```toml
   # Option 1: ONNX Runtime
   ort = "2.0"
   
   # Option 2: Candle (Rust-native)
   candle-core = "0.4"
   candle-transformers = "0.4"
   
   # Option 3: PyTorch bindings
   tch = "0.13"
   ```

2. **Implement model loading** in `ml_nlp.rs`
3. **Implement inference** methods
4. **Enable in configuration**: `enable_ml_models: true`

## Security

### Authentication

- API key authentication (required in production)
- Origin-based access control
- Rate limiting (100 requests/minute per IP)

### SQL Injection Protection

- All queries use parameterized statements
- Input validation and sanitization
- Query complexity limits

### Network Security

- TLS/SSL via Ingress
- Network policies (Kubernetes)
- Service mesh integration ready

## Performance

### Optimization

- Schema caching (5-minute TTL)
- Query result preview (first 100 rows)
- Connection pooling
- Async processing throughout

### Scaling

- Horizontal scaling (3-20 pods)
- Load balancing
- Session affinity for stateful operations

## 🐛 Troubleshooting

### Common Issues

1. **Schema not found:**
   - Check schema discovery is running
   - Verify Orbit-RS connection
   - Check cache TTL

2. **Query execution fails:**
   - Verify Orbit-RS query engine is accessible
   - Check query syntax in logs
   - Verify table exists

3. **ML models not loading:**
   - Check model file paths
   - Verify model format compatibility
   - Check GPU availability (if enabled)

### Debugging

Enable debug logging:
```bash
export RUST_LOG=debug,mcp=debug
```

Check logs:
```bash
kubectl logs -f deployment/orbit-mcp-server -n orbit-production
```

## API Documentation

### MCP Protocol Endpoints

- `POST /mcp/initialize`: Initialize MCP session
- `GET /mcp/tools`: List available tools
- `POST /mcp/tools/:name`: Execute tool
- `GET /mcp/resources`: List resources
- `GET /mcp/resources/:uri`: Read resource
- `GET /mcp/prompts`: List prompts
- `POST /mcp/prompts/:name`: Get prompt

### Example Tool Call

```json
POST /mcp/tools/query_data
{
  "query": "Show me all users from California",
  "limit": 100
}
```

Response:
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"summary\": \"Query returned 42 rows with 3 columns: id, name, state\", \"data_preview\": [...], \"statistics\": {...}}"
    }
  ],
  "is_error": false
}
```

## Next Steps

1. **Load Testing**: Run performance tests with realistic workloads
2. **ML Model Training**: Train and deploy intent classification models
3. **Enhanced NLP**: Improve entity recognition accuracy
4. **Query Optimization**: Add query plan caching
5. **Monitoring Dashboard**: Create Grafana dashboards
6. **Documentation**: Add API documentation and examples

## 📞 Support

For issues or questions:
- Check logs: `kubectl logs -f deployment/orbit-mcp-server`
- Review metrics: Prometheus dashboard
- Check health: `curl https://mcp.orbit.example.com/health`

