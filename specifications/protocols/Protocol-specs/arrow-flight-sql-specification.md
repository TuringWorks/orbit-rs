# Arrow Flight SQL Integration Specification

## Document Information

| Field | Value |
|-------|-------|
| **Version** | 1.0.0 |
| **Status** | Draft |
| **Created** | 2025-12-08 |
| **Authors** | Orbit-RS Team |
| **License** | BSD-3-Clause OR MIT |

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Protocol Details](#protocol-details)
4. [Message Types](#message-types)
5. [Authentication](#authentication)
6. [Query Execution](#query-execution)
7. [Data Type Mappings](#data-type-mappings)
8. [Streaming and LIVE Queries](#streaming-and-live-queries)
9. [Prepared Statements](#prepared-statements)
10. [Transactions](#transactions)
11. [Metadata and Catalogs](#metadata-and-catalogs)
12. [Error Handling](#error-handling)
13. [Implementation Guide](#implementation-guide)
14. [Client Compatibility](#client-compatibility)
15. [Performance Considerations](#performance-considerations)

---

## Overview

### Purpose

Arrow Flight SQL provides a high-performance, columnar wire protocol for OrbitQL, enabling efficient data transfer between Orbit-RS servers and clients. It leverages Apache Arrow's columnar format for zero-copy data access and gRPC for transport.

### Key Benefits

| Benefit | Description |
|---------|-------------|
| **High Performance** | Zero-copy data transfer with Arrow columnar format |
| **Streaming** | Native support for large result sets and real-time updates |
| **Cross-Platform** | JDBC, ODBC, Python, Go, Rust client support |
| **Type Safety** | Strong typing with Arrow schemas |
| **Compression** | Built-in LZ4, ZSTD compression support |
| **Interoperability** | Works with DuckDB, Dremio, InfluxDB, DataGrip, DBeaver |

### Protocol Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    Client Application                        │
├─────────────────────────────────────────────────────────────┤
│              Flight SQL Client (JDBC/ODBC/Native)            │
├─────────────────────────────────────────────────────────────┤
│                    Arrow Flight Protocol                     │
├─────────────────────────────────────────────────────────────┤
│                         gRPC/HTTP2                           │
├─────────────────────────────────────────────────────────────┤
│                        TLS (optional)                        │
├─────────────────────────────────────────────────────────────┤
│                         TCP/IP                               │
└─────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Component Overview

```
┌────────────────────────────────────────────────────────────────────┐
│                        Orbit-RS Server                              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   Flight SQL Service                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐   │  │
│  │  │  Handshake  │  │   Query     │  │  Prepared Statement │   │  │
│  │  │   Handler   │  │  Executor   │  │      Manager        │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘   │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐   │  │
│  │  │ Transaction │  │   Catalog   │  │   Schema Registry   │   │  │
│  │  │   Manager   │  │  Provider   │  │                     │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     OrbitQL Engine                            │  │
│  │  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │  │
│  │  │  Lexer  │  │  Parser  │  │ Optimizer│  │   Executor   │   │  │
│  │  └─────────┘  └──────────┘  └──────────┘  └──────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Storage Engine                             │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

### Default Ports

| Service | Port | Description |
|---------|------|-------------|
| Flight SQL | 32010 | Primary Arrow Flight SQL endpoint |
| Flight SQL (TLS) | 32011 | TLS-encrypted Flight SQL |

---

## Protocol Details

### Flight Service Methods

Arrow Flight SQL uses the standard Flight service with SQL-specific commands encoded in Protocol Buffers.

```protobuf
service FlightService {
  // Handshake for authentication
  rpc Handshake(stream HandshakeRequest) returns (stream HandshakeResponse);

  // Get schema and endpoints for a query
  rpc GetFlightInfo(FlightDescriptor) returns (FlightInfo);

  // Get schema only (no execution)
  rpc GetSchema(FlightDescriptor) returns (SchemaResult);

  // Execute and stream results
  rpc DoGet(Ticket) returns (stream FlightData);

  // Upload data (INSERT operations)
  rpc DoPut(stream FlightData) returns (stream PutResult);

  // Bidirectional streaming
  rpc DoExchange(stream FlightData) returns (stream FlightData);

  // Execute actions (transactions, prepared statements)
  rpc DoAction(Action) returns (stream Result);

  // List available actions
  rpc ListActions(Empty) returns (stream ActionType);

  // List available "flights" (tables, queries)
  rpc ListFlights(Criteria) returns (stream FlightInfo);
}
```

### Flight SQL Commands

Commands are encoded as Protocol Buffer `Any` types in the `FlightDescriptor.cmd` field.

| Command | Description |
|---------|-------------|
| `CommandStatementQuery` | Execute a SQL/OrbitQL query |
| `CommandStatementUpdate` | Execute DML (INSERT/UPDATE/DELETE) |
| `CommandPreparedStatementQuery` | Execute prepared statement |
| `CommandPreparedStatementUpdate` | Execute prepared DML |
| `CommandGetCatalogs` | List available catalogs |
| `CommandGetDbSchemas` | List schemas in catalog |
| `CommandGetTables` | List tables |
| `CommandGetTableTypes` | List table types |
| `CommandGetSqlInfo` | Get server capabilities |
| `CommandGetPrimaryKeys` | Get primary key info |
| `CommandGetExportedKeys` | Get foreign key info |
| `CommandGetImportedKeys` | Get referenced keys |
| `CommandGetCrossReference` | Get cross-reference info |
| `CommandGetXdbcTypeInfo` | Get type information |

---

## Message Types

### FlightDescriptor

```protobuf
message FlightDescriptor {
  enum DescriptorType {
    UNKNOWN = 0;
    PATH = 1;
    CMD = 2;
  }
  DescriptorType type = 1;
  bytes cmd = 2;           // Encoded Flight SQL command
  repeated string path = 3; // Path for path-based descriptors
}
```

### FlightInfo

```protobuf
message FlightInfo {
  bytes schema = 1;                    // Arrow schema (IPC format)
  FlightDescriptor flight_descriptor = 2;
  repeated FlightEndpoint endpoint = 3; // Where to fetch data
  int64 total_records = 4;             // -1 if unknown
  int64 total_bytes = 5;               // -1 if unknown
  bool ordered = 6;                    // Results are ordered
}
```

### FlightEndpoint

```protobuf
message FlightEndpoint {
  Ticket ticket = 1;           // Opaque ticket for DoGet
  repeated Location location = 2; // Server locations
  google.protobuf.Timestamp expiration_time = 3;
}
```

### FlightData

```protobuf
message FlightData {
  FlightDescriptor flight_descriptor = 1;
  bytes data_header = 2;    // Arrow IPC message header
  bytes app_metadata = 3;   // Application-specific metadata
  bytes data_body = 4;      // Arrow IPC message body
}
```

---

## Authentication

### Supported Methods

| Method | Description | Use Case |
|--------|-------------|----------|
| **No Auth** | No authentication | Development/testing |
| **Basic** | Username/password via Handshake | Simple deployments |
| **Bearer Token** | JWT/OAuth2 token | Production/SSO |
| **mTLS** | Mutual TLS certificates | High security |

### Basic Authentication Flow

```
Client                                    Server
   │                                         │
   │──── Handshake(username:password) ──────▶│
   │                                         │
   │◀─── HandshakeResponse(token) ───────────│
   │                                         │
   │──── GetFlightInfo(Authorization: Bearer token) ─▶│
   │                                         │
```

### Implementation

```rust
use arrow_flight::flight_service_server::FlightService;
use tonic::{Request, Response, Status, Streaming};

#[tonic::async_trait]
impl FlightService for OrbitFlightSqlServer {
    type HandshakeStream = BoxStream<'static, Result<HandshakeResponse, Status>>;

    async fn handshake(
        &self,
        request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        let mut stream = request.into_inner();

        // Get first handshake message
        let handshake = stream.next().await
            .ok_or_else(|| Status::invalid_argument("No handshake received"))??;

        // Parse Basic auth: "username:password"
        let payload = String::from_utf8(handshake.payload.to_vec())
            .map_err(|_| Status::invalid_argument("Invalid UTF-8"))?;

        let parts: Vec<&str> = payload.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Status::unauthenticated("Invalid credentials format"));
        }

        let (username, password) = (parts[0], parts[1]);

        // Authenticate against Orbit's auth system
        let token = self.auth_manager
            .authenticate(username, password)
            .await
            .map_err(|_| Status::unauthenticated("Authentication failed"))?;

        // Return bearer token
        let response = HandshakeResponse {
            protocol_version: 0,
            payload: token.into_bytes().into(),
        };

        let stream = futures::stream::once(async { Ok(response) });
        Ok(Response::new(Box::pin(stream)))
    }
}
```

### Token Validation Interceptor

```rust
use tonic::service::Interceptor;

#[derive(Clone)]
pub struct AuthInterceptor {
    auth_manager: Arc<AuthManager>,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        // Extract Authorization header
        let token = request.metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        match token {
            Some(token) => {
                // Validate token
                let claims = self.auth_manager
                    .validate_token(token)
                    .map_err(|_| Status::unauthenticated("Invalid token"))?;

                // Add claims to request extensions
                request.extensions_mut().insert(claims);
                Ok(request)
            }
            None => {
                // Allow unauthenticated for Handshake
                Ok(request)
            }
        }
    }
}
```

---

## Query Execution

### Simple Query Flow

```
Client                                           Server
   │                                                │
   │── GetFlightInfo(CommandStatementQuery) ───────▶│
   │        { query: "SELECT * FROM users" }        │
   │                                                │
   │◀── FlightInfo ─────────────────────────────────│
   │        { schema, endpoints: [ticket], rows }   │
   │                                                │
   │── DoGet(ticket) ──────────────────────────────▶│
   │                                                │
   │◀── FlightData (schema) ────────────────────────│
   │◀── FlightData (batch 1) ───────────────────────│
   │◀── FlightData (batch 2) ───────────────────────│
   │◀── FlightData (batch N) ───────────────────────│
   │                                                │
```

### Implementation

```rust
use arrow_flight::sql::{
    CommandStatementQuery, CommandStatementUpdate,
    ProstMessageExt, SqlInfo,
};
use arrow::record_batch::RecordBatch;
use arrow_ipc::writer::IpcWriteOptions;

#[tonic::async_trait]
impl FlightService for OrbitFlightSqlServer {
    type DoGetStream = BoxStream<'static, Result<FlightData, Status>>;

    async fn get_flight_info(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let descriptor = request.into_inner();

        // Decode Flight SQL command
        let message = Any::decode(&*descriptor.cmd)
            .map_err(|e| Status::invalid_argument(format!("Invalid command: {}", e)))?;

        if let Some(query_cmd) = message.unpack::<CommandStatementQuery>()
            .map_err(|e| Status::internal(e.to_string()))?
        {
            self.handle_statement_query(query_cmd, descriptor).await
        } else if let Some(update_cmd) = message.unpack::<CommandStatementUpdate>()
            .map_err(|e| Status::internal(e.to_string()))?
        {
            self.handle_statement_update(update_cmd, descriptor).await
        } else {
            Err(Status::unimplemented("Unsupported command type"))
        }
    }

    async fn do_get(
        &self,
        request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let ticket = request.into_inner();

        // Decode ticket (contains query or prepared statement handle)
        let query_ticket: QueryTicket = serde_json::from_slice(&ticket.ticket)
            .map_err(|e| Status::invalid_argument(format!("Invalid ticket: {}", e)))?;

        // Execute query
        let result_stream = self.engine
            .execute_streaming(&query_ticket.query, query_ticket.params)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Convert to FlightData stream
        let flight_stream = self.record_batches_to_flight_data(result_stream);

        Ok(Response::new(Box::pin(flight_stream)))
    }
}

impl OrbitFlightSqlServer {
    async fn handle_statement_query(
        &self,
        cmd: CommandStatementQuery,
        descriptor: FlightDescriptor,
    ) -> Result<Response<FlightInfo>, Status> {
        let query = &cmd.query;

        // Parse and analyze query to get schema
        let (schema, estimated_rows) = self.engine
            .analyze_query(query)
            .await
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Create ticket for DoGet
        let ticket = QueryTicket {
            query: query.clone(),
            params: vec![],
            transaction_id: cmd.transaction_id.clone(),
        };
        let ticket_bytes = serde_json::to_vec(&ticket)
            .map_err(|e| Status::internal(e.to_string()))?;

        // Build FlightInfo
        let info = FlightInfo::new()
            .try_with_schema(&schema)
            .map_err(|e| Status::internal(e.to_string()))?
            .with_descriptor(descriptor)
            .with_endpoint(FlightEndpoint::new()
                .with_ticket(Ticket::new(ticket_bytes)))
            .with_total_records(estimated_rows.unwrap_or(-1));

        Ok(Response::new(info))
    }

    fn record_batches_to_flight_data(
        &self,
        batches: impl Stream<Item = Result<RecordBatch, OrbitError>> + Send + 'static,
    ) -> impl Stream<Item = Result<FlightData, Status>> + Send + 'static {
        let options = IpcWriteOptions::default();
        let mut schema_sent = false;

        batches.map(move |batch_result| {
            match batch_result {
                Ok(batch) => {
                    let mut data_vec = vec![];

                    // Send schema with first batch
                    if !schema_sent {
                        let schema_data = FlightData::new()
                            .with_schema(batch.schema().as_ref(), &options);
                        data_vec.push(Ok(schema_data));
                        schema_sent = true;
                    }

                    // Send batch data
                    let batch_data = flight_data_from_arrow_batch(&batch, &options)
                        .map_err(|e| Status::internal(e.to_string()))?;
                    data_vec.push(Ok(batch_data));

                    Ok(futures::stream::iter(data_vec))
                }
                Err(e) => Err(Status::internal(e.to_string())),
            }
        })
        .try_flatten()
    }
}

#[derive(Serialize, Deserialize)]
struct QueryTicket {
    query: String,
    params: Vec<serde_json::Value>,
    transaction_id: Option<String>,
}
```

### Update Execution (DML)

```rust
impl OrbitFlightSqlServer {
    async fn handle_statement_update(
        &self,
        cmd: CommandStatementUpdate,
        descriptor: FlightDescriptor,
    ) -> Result<Response<FlightInfo>, Status> {
        let query = &cmd.query;

        // Execute DML statement
        let affected_rows = self.engine
            .execute_update(query, cmd.transaction_id.as_deref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Return update count in FlightInfo
        let schema = Schema::new(vec![
            Field::new("affected_rows", DataType::Int64, false),
        ]);

        let info = FlightInfo::new()
            .try_with_schema(&schema)
            .map_err(|e| Status::internal(e.to_string()))?
            .with_descriptor(descriptor)
            .with_total_records(affected_rows);

        Ok(Response::new(info))
    }
}
```

---

## Data Type Mappings

### OrbitQL to Arrow Type Mappings

| OrbitQL Type | Arrow Type | Notes |
|--------------|------------|-------|
| `BOOLEAN` | `Boolean` | |
| `INTEGER` / `INT` | `Int64` | 64-bit signed |
| `SMALLINT` | `Int16` | 16-bit signed |
| `BIGINT` | `Int64` | 64-bit signed |
| `FLOAT` | `Float64` | IEEE 754 double |
| `DECIMAL(p,s)` | `Decimal128(p,s)` | High precision |
| `STRING` / `TEXT` | `Utf8` | UTF-8 encoded |
| `VARCHAR(n)` | `Utf8` | With metadata |
| `BYTES` / `BLOB` | `Binary` | |
| `DATE` | `Date32` | Days since epoch |
| `TIME` | `Time64(Nanosecond)` | |
| `TIMESTAMP` | `Timestamp(Nanosecond, UTC)` | |
| `TIMESTAMPTZ` | `Timestamp(Nanosecond, Some(tz))` | |
| `DURATION` | `Duration(Nanosecond)` | |
| `INTERVAL` | `Interval(MonthDayNano)` | |
| `UUID` | `FixedSizeBinary(16)` | Or Utf8 string |
| `JSON` / `OBJECT` | `Utf8` | JSON string |
| `ARRAY<T>` | `List<T>` | Nested array |
| `VECTOR(n)` | `FixedSizeList<Float32>(n)` | Vector embeddings |
| `GEOMETRY` | `Binary` | WKB encoded |
| `POINT` | `Struct{x: Float64, y: Float64}` | |

### Type Conversion Implementation

```rust
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

pub fn orbitql_type_to_arrow(orbitql_type: &str) -> Result<DataType, ConversionError> {
    match orbitql_type.to_uppercase().as_str() {
        "BOOLEAN" | "BOOL" => Ok(DataType::Boolean),
        "INTEGER" | "INT" | "INT64" => Ok(DataType::Int64),
        "SMALLINT" | "INT16" => Ok(DataType::Int16),
        "BIGINT" => Ok(DataType::Int64),
        "FLOAT" | "FLOAT64" | "DOUBLE" => Ok(DataType::Float64),
        "FLOAT32" => Ok(DataType::Float32),
        "STRING" | "TEXT" => Ok(DataType::Utf8),
        "BYTES" | "BLOB" | "BINARY" => Ok(DataType::Binary),
        "DATE" => Ok(DataType::Date32),
        "TIME" => Ok(DataType::Time64(TimeUnit::Nanosecond)),
        "TIMESTAMP" => Ok(DataType::Timestamp(TimeUnit::Nanosecond, None)),
        "TIMESTAMPTZ" => Ok(DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into()))),
        "DURATION" => Ok(DataType::Duration(TimeUnit::Nanosecond)),
        "UUID" => Ok(DataType::FixedSizeBinary(16)),
        "JSON" | "OBJECT" => Ok(DataType::Utf8),
        "GEOMETRY" => Ok(DataType::Binary),
        s if s.starts_with("VARCHAR") => Ok(DataType::Utf8),
        s if s.starts_with("DECIMAL") => {
            // Parse DECIMAL(p,s)
            let (precision, scale) = parse_decimal_params(s)?;
            Ok(DataType::Decimal128(precision, scale))
        }
        s if s.starts_with("ARRAY<") => {
            let inner_type = parse_array_inner_type(s)?;
            let inner_arrow = orbitql_type_to_arrow(&inner_type)?;
            Ok(DataType::List(Arc::new(Field::new("item", inner_arrow, true))))
        }
        s if s.starts_with("VECTOR(") => {
            let dimensions = parse_vector_dimensions(s)?;
            Ok(DataType::FixedSizeList(
                Arc::new(Field::new("value", DataType::Float32, false)),
                dimensions as i32,
            ))
        }
        _ => Err(ConversionError::UnsupportedType(orbitql_type.to_string())),
    }
}

pub fn arrow_value_to_orbitql(value: &dyn Array, row: usize) -> Result<Value, ConversionError> {
    use arrow::array::*;

    match value.data_type() {
        DataType::Boolean => {
            let arr = value.as_any().downcast_ref::<BooleanArray>().unwrap();
            Ok(Value::Bool(arr.value(row)))
        }
        DataType::Int64 => {
            let arr = value.as_any().downcast_ref::<Int64Array>().unwrap();
            Ok(Value::Int(arr.value(row)))
        }
        DataType::Float64 => {
            let arr = value.as_any().downcast_ref::<Float64Array>().unwrap();
            Ok(Value::Float(arr.value(row)))
        }
        DataType::Utf8 => {
            let arr = value.as_any().downcast_ref::<StringArray>().unwrap();
            Ok(Value::String(arr.value(row).to_string()))
        }
        DataType::FixedSizeList(_, dim) => {
            // Vector type
            let arr = value.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
            let inner = arr.value(row);
            let float_arr = inner.as_any().downcast_ref::<Float32Array>().unwrap();
            let vec: Vec<f32> = (0..float_arr.len()).map(|i| float_arr.value(i)).collect();
            Ok(Value::Vector(vec))
        }
        // ... handle other types
        _ => Err(ConversionError::UnsupportedArrowType(value.data_type().clone())),
    }
}
```

---

## Streaming and LIVE Queries

### LIVE Query Support

Arrow Flight's `DoExchange` method enables bidirectional streaming, perfect for LIVE queries.

```
Client                                           Server
   │                                                │
   │── DoExchange ─────────────────────────────────▶│
   │     { LIVE SELECT * FROM orders WHERE ... }    │
   │                                                │
   │◀── FlightData (schema) ────────────────────────│
   │◀── FlightData (initial results) ───────────────│
   │                                                │
   │     ... time passes, data changes ...          │
   │                                                │
   │◀── FlightData (INSERT notification) ───────────│
   │◀── FlightData (UPDATE notification) ───────────│
   │◀── FlightData (DELETE notification) ───────────│
   │                                                │
   │── FlightData (KILL query_id) ─────────────────▶│
   │                                                │
   │◀── FlightData (subscription ended) ────────────│
   │                                                │
```

### Implementation

```rust
#[derive(Serialize, Deserialize)]
pub struct LiveQueryMetadata {
    pub query_id: String,
    pub change_type: ChangeType,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize)]
pub enum ChangeType {
    Initial,
    Insert,
    Update,
    Delete,
    End,
}

impl OrbitFlightSqlServer {
    async fn do_exchange(
        &self,
        request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        let mut input = request.into_inner();

        // Get first message with LIVE query
        let first = input.next().await
            .ok_or_else(|| Status::invalid_argument("No query received"))??;

        let query = self.decode_live_query(&first)?;

        // Subscribe to changes
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let query_id = uuid::Uuid::new_v4().to_string();

        // Start live query subscription
        let subscription = self.engine
            .subscribe_live_query(&query, query_id.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Spawn task to handle incoming messages (KILL, etc.)
        let query_id_clone = query_id.clone();
        let engine = self.engine.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = input.next().await {
                if let Ok(cmd) = Self::decode_command(&msg) {
                    match cmd {
                        LiveCommand::Kill => {
                            engine.unsubscribe_live_query(&query_id_clone).await;
                            break;
                        }
                    }
                }
            }
        });

        // Convert subscription to FlightData stream
        let output_stream = subscription.map(move |change| {
            let metadata = LiveQueryMetadata {
                query_id: query_id.clone(),
                change_type: change.change_type,
                timestamp: change.timestamp,
            };

            let mut flight_data = flight_data_from_arrow_batch(&change.data, &IpcWriteOptions::default())?;
            flight_data.app_metadata = serde_json::to_vec(&metadata)
                .map_err(|e| Status::internal(e.to_string()))?
                .into();

            Ok(flight_data)
        });

        Ok(Response::new(Box::pin(output_stream)))
    }
}
```

---

## Prepared Statements

### Prepared Statement Lifecycle

```
Client                                           Server
   │                                                │
   │── DoAction(CreatePreparedStatement) ──────────▶│
   │     { query: "SELECT * FROM users WHERE id = ?"}│
   │                                                │
   │◀── Result(PreparedStatementHandle) ────────────│
   │     { handle, parameter_schema, result_schema }│
   │                                                │
   │── GetFlightInfo(CommandPreparedStatementQuery)▶│
   │     { handle, parameters: [Arrow batch] }      │
   │                                                │
   │◀── FlightInfo ─────────────────────────────────│
   │                                                │
   │── DoGet(ticket) ──────────────────────────────▶│
   │◀── FlightData (results) ───────────────────────│
   │                                                │
   │── DoAction(ClosePreparedStatement) ───────────▶│
   │     { handle }                                 │
   │                                                │
   │◀── Result(success) ────────────────────────────│
   │                                                │
```

### Implementation

```rust
use arrow_flight::sql::{
    ActionCreatePreparedStatementRequest, ActionCreatePreparedStatementResult,
    ActionClosePreparedStatementRequest, CommandPreparedStatementQuery,
};
use std::collections::HashMap;
use parking_lot::RwLock;

pub struct PreparedStatementManager {
    statements: RwLock<HashMap<Vec<u8>, PreparedStatement>>,
}

pub struct PreparedStatement {
    pub query: String,
    pub parameter_schema: Schema,
    pub result_schema: Schema,
    pub created_at: Instant,
}

impl OrbitFlightSqlServer {
    async fn do_action(
        &self,
        request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        let action = request.into_inner();

        match action.r#type.as_str() {
            "CreatePreparedStatement" => {
                self.create_prepared_statement(&action.body).await
            }
            "ClosePreparedStatement" => {
                self.close_prepared_statement(&action.body).await
            }
            "BeginTransaction" => {
                self.begin_transaction().await
            }
            "CommitTransaction" => {
                self.commit_transaction(&action.body).await
            }
            "RollbackTransaction" => {
                self.rollback_transaction(&action.body).await
            }
            "CreateSavepoint" => {
                self.create_savepoint(&action.body).await
            }
            "ReleaseSavepoint" => {
                self.release_savepoint(&action.body).await
            }
            _ => Err(Status::unimplemented(format!("Unknown action: {}", action.r#type))),
        }
    }

    async fn create_prepared_statement(
        &self,
        body: &[u8],
    ) -> Result<Response<Self::DoActionStream>, Status> {
        let request = ActionCreatePreparedStatementRequest::decode(body)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Parse query to extract parameter info
        let (param_schema, result_schema) = self.engine
            .analyze_prepared_statement(&request.query)
            .await
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Generate handle
        let handle = uuid::Uuid::new_v4().as_bytes().to_vec();

        // Store prepared statement
        self.prepared_statements.write().insert(handle.clone(), PreparedStatement {
            query: request.query,
            parameter_schema: param_schema.clone(),
            result_schema: result_schema.clone(),
            created_at: Instant::now(),
        });

        // Build response
        let result = ActionCreatePreparedStatementResult {
            prepared_statement_handle: handle.into(),
            dataset_schema: serialize_schema(&result_schema)?,
            parameter_schema: serialize_schema(&param_schema)?,
        };

        let result_bytes = result.encode_to_vec();
        let stream = futures::stream::once(async move {
            Ok(arrow_flight::Result { body: result_bytes.into() })
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn execute_prepared_statement(
        &self,
        cmd: CommandPreparedStatementQuery,
        descriptor: FlightDescriptor,
    ) -> Result<Response<FlightInfo>, Status> {
        let handle = cmd.prepared_statement_handle.to_vec();

        // Get prepared statement
        let stmt = self.prepared_statements.read()
            .get(&handle)
            .cloned()
            .ok_or_else(|| Status::not_found("Prepared statement not found"))?;

        // Build ticket with handle and parameters
        let ticket = PreparedStatementTicket {
            handle,
            // Parameters will be provided via DoPut before DoGet
        };

        let info = FlightInfo::new()
            .try_with_schema(&stmt.result_schema)
            .map_err(|e| Status::internal(e.to_string()))?
            .with_descriptor(descriptor)
            .with_endpoint(FlightEndpoint::new()
                .with_ticket(Ticket::new(serde_json::to_vec(&ticket).unwrap())));

        Ok(Response::new(info))
    }
}
```

---

## Transactions

### Transaction Actions

| Action | Description |
|--------|-------------|
| `BeginTransaction` | Start a new transaction |
| `CommitTransaction` | Commit current transaction |
| `RollbackTransaction` | Rollback current transaction |
| `CreateSavepoint` | Create a savepoint |
| `ReleaseSavepoint` | Release a savepoint |
| `RollbackToSavepoint` | Rollback to savepoint |

### Implementation

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TransactionManager {
    transactions: RwLock<HashMap<String, TransactionState>>,
    next_id: AtomicU64,
}

pub struct TransactionState {
    pub id: String,
    pub started_at: Instant,
    pub isolation_level: IsolationLevel,
    pub savepoints: Vec<String>,
}

impl OrbitFlightSqlServer {
    async fn begin_transaction(&self) -> Result<Response<Self::DoActionStream>, Status> {
        let tx_id = format!("tx_{}", self.transaction_manager.next_id.fetch_add(1, Ordering::SeqCst));

        // Start transaction in engine
        self.engine.begin_transaction(&tx_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Store transaction state
        self.transaction_manager.transactions.write().insert(tx_id.clone(), TransactionState {
            id: tx_id.clone(),
            started_at: Instant::now(),
            isolation_level: IsolationLevel::ReadCommitted,
            savepoints: vec![],
        });

        // Return transaction ID
        let result = BeginTransactionResult { transaction_id: tx_id.into_bytes() };
        let stream = futures::stream::once(async move {
            Ok(arrow_flight::Result { body: result.encode_to_vec().into() })
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn commit_transaction(&self, body: &[u8]) -> Result<Response<Self::DoActionStream>, Status> {
        let request = CommitTransactionRequest::decode(body)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let tx_id = String::from_utf8(request.transaction_id.to_vec())
            .map_err(|_| Status::invalid_argument("Invalid transaction ID"))?;

        // Commit in engine
        self.engine.commit_transaction(&tx_id).await
            .map_err(|e| Status::aborted(e.to_string()))?;

        // Remove transaction state
        self.transaction_manager.transactions.write().remove(&tx_id);

        let stream = futures::stream::once(async {
            Ok(arrow_flight::Result { body: vec![].into() })
        });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn create_savepoint(&self, body: &[u8]) -> Result<Response<Self::DoActionStream>, Status> {
        let request = CreateSavepointRequest::decode(body)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let tx_id = String::from_utf8(request.transaction_id.to_vec())
            .map_err(|_| Status::invalid_argument("Invalid transaction ID"))?;

        let savepoint_name = &request.savepoint_name;

        // Create savepoint in engine
        self.engine.create_savepoint(&tx_id, savepoint_name).await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Update transaction state
        if let Some(tx) = self.transaction_manager.transactions.write().get_mut(&tx_id) {
            tx.savepoints.push(savepoint_name.clone());
        }

        let stream = futures::stream::once(async {
            Ok(arrow_flight::Result { body: vec![].into() })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}
```

---

## Metadata and Catalogs

### GetSqlInfo

Returns server capabilities and configuration.

```rust
impl OrbitFlightSqlServer {
    fn get_sql_info(&self, info_ids: &[u32]) -> Result<RecordBatch, Status> {
        let mut info_names = Vec::new();
        let mut info_values = Vec::new();

        for id in info_ids {
            let (name, value) = match SqlInfo::try_from(*id) {
                Ok(SqlInfo::FlightSqlServerName) => {
                    ("FLIGHT_SQL_SERVER_NAME", SqlInfoValue::String("Orbit-RS".into()))
                }
                Ok(SqlInfo::FlightSqlServerVersion) => {
                    ("FLIGHT_SQL_SERVER_VERSION", SqlInfoValue::String(env!("CARGO_PKG_VERSION").into()))
                }
                Ok(SqlInfo::FlightSqlServerArrowVersion) => {
                    ("FLIGHT_SQL_SERVER_ARROW_VERSION", SqlInfoValue::String("53.0.0".into()))
                }
                Ok(SqlInfo::SqlDdlCatalog) => {
                    ("SQL_DDL_CATALOG", SqlInfoValue::Bool(true))
                }
                Ok(SqlInfo::SqlDdlSchema) => {
                    ("SQL_DDL_SCHEMA", SqlInfoValue::Bool(true))
                }
                Ok(SqlInfo::SqlDdlTable) => {
                    ("SQL_DDL_TABLE", SqlInfoValue::Bool(true))
                }
                Ok(SqlInfo::SqlIdentifierCase) => {
                    ("SQL_IDENTIFIER_CASE", SqlInfoValue::Int32(1)) // Case insensitive
                }
                Ok(SqlInfo::SqlQuotedIdentifierCase) => {
                    ("SQL_QUOTED_IDENTIFIER_CASE", SqlInfoValue::Int32(0)) // Case sensitive
                }
                // OrbitQL-specific extensions
                Ok(SqlInfo::SqlSupportedTransactionsIsolationLevels) => {
                    ("SQL_TRANSACTIONS_ISOLATION_LEVELS", SqlInfoValue::Int32(0x0F)) // All levels
                }
                _ => continue,
            };
            info_names.push(name);
            info_values.push(value);
        }

        // Build RecordBatch with info
        self.build_sql_info_batch(info_names, info_values)
    }
}
```

### GetCatalogs, GetDbSchemas, GetTables

```rust
impl OrbitFlightSqlServer {
    async fn get_catalogs(&self) -> Result<RecordBatch, Status> {
        let catalogs = self.engine.list_namespaces().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let schema = Schema::new(vec![
            Field::new("catalog_name", DataType::Utf8, false),
        ]);

        let catalog_names: Vec<&str> = catalogs.iter().map(|s| s.as_str()).collect();

        RecordBatch::try_new(
            Arc::new(schema),
            vec![Arc::new(StringArray::from(catalog_names))],
        ).map_err(|e| Status::internal(e.to_string()))
    }

    async fn get_db_schemas(
        &self,
        catalog: Option<&str>,
        schema_filter: Option<&str>,
    ) -> Result<RecordBatch, Status> {
        let schemas = self.engine.list_databases(catalog, schema_filter).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let schema = Schema::new(vec![
            Field::new("catalog_name", DataType::Utf8, true),
            Field::new("db_schema_name", DataType::Utf8, false),
        ]);

        // Build batch from schemas...
        todo!()
    }

    async fn get_tables(
        &self,
        catalog: Option<&str>,
        db_schema: Option<&str>,
        table_filter: Option<&str>,
        table_types: &[String],
        include_schema: bool,
    ) -> Result<RecordBatch, Status> {
        let tables = self.engine.list_tables(catalog, db_schema, table_filter, table_types).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut fields = vec![
            Field::new("catalog_name", DataType::Utf8, true),
            Field::new("db_schema_name", DataType::Utf8, true),
            Field::new("table_name", DataType::Utf8, false),
            Field::new("table_type", DataType::Utf8, false),
        ];

        if include_schema {
            fields.push(Field::new("table_schema", DataType::Binary, true));
        }

        // Build batch from tables...
        todo!()
    }
}
```

---

## Error Handling

### Error Codes

| gRPC Code | OrbitQL Error | Description |
|-----------|---------------|-------------|
| `INVALID_ARGUMENT` | Syntax/Parse error | Invalid query syntax |
| `NOT_FOUND` | Table/Column not found | Missing object |
| `ALREADY_EXISTS` | Duplicate key | Constraint violation |
| `PERMISSION_DENIED` | Access denied | Authorization failure |
| `ABORTED` | Transaction conflict | Deadlock/conflict |
| `INTERNAL` | Engine error | Internal failure |
| `UNAVAILABLE` | Connection error | Server unavailable |
| `RESOURCE_EXHAUSTED` | Limit exceeded | Memory/time limit |

### Error Response Format

```rust
use tonic::Status;

pub fn orbitql_error_to_status(error: OrbitError) -> Status {
    match error {
        OrbitError::ParseError { message, line, column } => {
            Status::invalid_argument(format!(
                "Parse error at {}:{}: {}",
                line, column, message
            ))
        }
        OrbitError::TableNotFound { table } => {
            Status::not_found(format!("Table '{}' not found", table))
        }
        OrbitError::ColumnNotFound { column, table } => {
            Status::not_found(format!("Column '{}' not found in table '{}'", column, table))
        }
        OrbitError::DuplicateKey { table, key } => {
            Status::already_exists(format!(
                "Duplicate key '{}' in table '{}'",
                key, table
            ))
        }
        OrbitError::AccessDenied { resource, action } => {
            Status::permission_denied(format!(
                "Access denied: cannot {} on '{}'",
                action, resource
            ))
        }
        OrbitError::TransactionConflict { details } => {
            Status::aborted(format!("Transaction conflict: {}", details))
        }
        OrbitError::InternalError { message } => {
            Status::internal(message)
        }
        _ => Status::unknown("Unknown error"),
    }
}
```

---

## Implementation Guide

### Project Structure

```
orbit/server/src/protocols/flight_sql/
├── mod.rs                    # Module exports
├── server.rs                 # FlightService implementation
├── auth.rs                   # Authentication handlers
├── query.rs                  # Query execution
├── prepared.rs               # Prepared statement management
├── transaction.rs            # Transaction management
├── catalog.rs                # Catalog/metadata queries
├── types.rs                  # Type conversions
├── live.rs                   # LIVE query support
└── error.rs                  # Error handling
```

### Cargo Dependencies

```toml
[dependencies]
# Arrow ecosystem
arrow = { version = "53", features = ["ipc", "prettyprint"] }
arrow-array = "53"
arrow-buffer = "53"
arrow-cast = "53"
arrow-flight = { version = "53", features = ["flight-sql-experimental"] }
arrow-ipc = "53"
arrow-schema = "53"

# gRPC
tonic = { version = "0.12", features = ["tls", "gzip"] }
prost = "0.13"
prost-types = "0.13"

# Async
tokio = { version = "1.48", features = ["full"] }
tokio-stream = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# UUID
uuid = { version = "1.0", features = ["v4"] }
```

### Server Startup

```rust
use tonic::transport::Server;
use arrow_flight::flight_service_server::FlightServiceServer;

pub async fn start_flight_sql_server(
    engine: Arc<OrbitQLEngine>,
    addr: SocketAddr,
    tls_config: Option<ServerTlsConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    let service = OrbitFlightSqlServer::new(engine);

    let auth_interceptor = AuthInterceptor::new(service.auth_manager.clone());

    let mut builder = Server::builder();

    if let Some(tls) = tls_config {
        builder = builder.tls_config(tls)?;
    }

    tracing::info!("Starting Arrow Flight SQL server on {}", addr);

    builder
        .add_service(FlightServiceServer::with_interceptor(service, auth_interceptor))
        .serve(addr)
        .await?;

    Ok(())
}
```

---

## Client Compatibility

### Supported Clients

| Client | Status | Notes |
|--------|--------|-------|
| **arrow-flight (Rust)** | Full | Native support |
| **pyarrow.flight (Python)** | Full | pip install pyarrow |
| **Arrow Flight JDBC** | Full | Standard JDBC driver |
| **Arrow Flight ODBC** | Full | ODBC driver |
| **DBeaver** | Partial | Via JDBC driver |
| **DataGrip** | Partial | Via JDBC driver |
| **Tableau** | Planned | Via ODBC driver |
| **Power BI** | Planned | Via ODBC driver |

### Python Client Example

```python
from pyarrow import flight

# Connect to Orbit-RS
client = flight.connect("grpc://localhost:32010")

# Authenticate
token = client.authenticate_basic_token("user", "password")
options = flight.FlightCallOptions(headers=[(b"authorization", b"Bearer " + token[1])])

# Execute query
info = client.get_flight_info(
    flight.FlightDescriptor.for_command(
        b'{"query": "SELECT * FROM users WHERE active = true"}'
    ),
    options
)

# Get results
reader = client.do_get(info.endpoints[0].ticket, options)
table = reader.read_all()
print(table.to_pandas())
```

### Rust Client Example

```rust
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::sql::client::FlightSqlServiceClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect
    let channel = tonic::transport::Channel::from_static("http://localhost:32010")
        .connect()
        .await?;

    let mut client = FlightSqlServiceClient::new(channel);

    // Authenticate
    let token = client.handshake("user", "password").await?;

    // Execute query
    let flight_info = client
        .execute("SELECT * FROM users WHERE active = true".to_string(), None)
        .await?;

    // Get results
    for endpoint in flight_info.endpoint {
        let mut stream = client.do_get(endpoint.ticket.unwrap()).await?;
        while let Some(batch) = stream.next().await {
            println!("{:?}", batch?);
        }
    }

    Ok(())
}
```

---

## Performance Considerations

### Batch Size Configuration

```rust
pub struct FlightSqlConfig {
    /// Maximum rows per Arrow batch
    pub max_batch_rows: usize,

    /// Maximum bytes per Arrow batch
    pub max_batch_bytes: usize,

    /// Enable compression (LZ4, ZSTD)
    pub compression: Option<CompressionType>,

    /// Concurrent streams per query
    pub max_concurrent_streams: usize,
}

impl Default for FlightSqlConfig {
    fn default() -> Self {
        Self {
            max_batch_rows: 65536,
            max_batch_bytes: 16 * 1024 * 1024, // 16MB
            compression: Some(CompressionType::Lz4Frame),
            max_concurrent_streams: 4,
        }
    }
}
```

### Memory Management

```rust
impl OrbitFlightSqlServer {
    fn stream_with_backpressure(
        &self,
        batches: impl Stream<Item = Result<RecordBatch, OrbitError>>,
    ) -> impl Stream<Item = Result<FlightData, Status>> {
        // Use bounded channel for backpressure
        let (tx, rx) = tokio::sync::mpsc::channel(self.config.max_concurrent_streams);

        // Producer task
        tokio::spawn(async move {
            tokio::pin!(batches);
            while let Some(batch) = batches.next().await {
                if tx.send(batch).await.is_err() {
                    break; // Client disconnected
                }
            }
        });

        // Consumer stream
        tokio_stream::wrappers::ReceiverStream::new(rx).map(|result| {
            result
                .map(|batch| flight_data_from_arrow_batch(&batch, &IpcWriteOptions::default()))
                .map_err(|e| Status::internal(e.to_string()))?
        })
    }
}
```

---

## References

- [Arrow Flight SQL Specification](https://arrow.apache.org/docs/format/FlightSql.html)
- [Arrow Flight Protocol](https://arrow.apache.org/docs/format/Flight.html)
- [Apache Arrow Rust Implementation](https://github.com/apache/arrow-rs)
- [Flight SQL JDBC Driver](https://arrow.apache.org/docs/java/flight_sql_jdbc_driver.html)

---

*Arrow Flight SQL Specification v1.0.0 - December 2025*
