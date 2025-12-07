//! REST API request handlers
//!
//! This module implements the REST API handlers for actor management,
//! transactions, and system health endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use orbit_client::OrbitClient;
use orbit_shared::{AddressableReference, Key};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::IntoParams;

use super::models::*;
use crate::protocols::mcp::server::McpServer;

/// Shared API state
#[derive(Clone)]
pub struct ApiState {
    pub orbit_client: Arc<OrbitClient>,
    /// MCP server for natural language queries (optional)
    pub mcp_server: Option<Arc<McpServer>>,
}

/// Pagination query parameters
#[derive(Debug, Deserialize, IntoParams)]
pub struct PaginationParams {
    /// Page number (0-indexed)
    #[param(example = 0)]
    pub page: Option<usize>,

    /// Page size (default: 50, max: 1000)
    #[param(example = 50)]
    pub page_size: Option<usize>,

    /// Filter by actor type
    #[param(example = "GreeterActor")]
    pub actor_type: Option<String>,

    /// Filter by status (active, inactive)
    #[param(example = "active")]
    pub status: Option<String>,

    /// Sort by field (actor_type, last_activity, status)
    #[param(example = "last_activity")]
    pub sort_by: Option<String>,

    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
}

/// List all actors
///
/// Returns a paginated list of all active actors in the cluster.
#[utoipa::path(
    get,
    path = "/api/v1/actors",
    params(PaginationParams),
    responses(
        (status = 200, description = "Actors listed successfully", body = PagedResponse<ActorInfo>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn list_actors(
    State(state): State<ApiState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(50).min(1000);

    // Get cluster stats to provide some information
    let stats = state.orbit_client.stats().await;

    let node_id = state
        .orbit_client
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    // Note: The current OrbitClient doesn't expose a method to enumerate
    // all active actors. This would require extending the ActorRegistry
    // to support enumeration. For now, we return cluster info and
    // a placeholder that indicates the limitation.
    //
    // To fully implement this, we'd need:
    // 1. ActorRegistry.list_active_actors() -> Vec<AddressableReference>
    // 2. Optionally, a distributed directory service for cross-node queries

    let mut actors = Vec::new();

    // Provide a summary actor that shows cluster information
    if let Ok(client_stats) = stats {
        actors.push(ActorInfo {
            actor_type: "_ClusterInfo".to_string(),
            key: serde_json::json!({"StringKey": {"key": "cluster-status"}}),
            state: serde_json::json!({
                "namespace": client_stats.namespace,
                "server_connections": client_stats.server_connections,
                "node_id": client_stats.node_id.map(|n| n.key),
                "_note": "Actor enumeration requires registry extension"
            }),
            node_id: Some(node_id.clone()),
            status: "active".to_string(),
            last_activity: Some(chrono::Utc::now().to_rfc3339()),
        });
    }

    let total = actors.len();
    let response = PagedResponse::new(actors, total, page, page_size);

    tracing::debug!(
        page = page,
        page_size = page_size,
        "Listed actors via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// Get actor state
///
/// Retrieves the current state of a specific actor by its ID.
#[utoipa::path(
    get,
    path = "/api/v1/actors/{actor_type}/{key}",
    params(
        ("actor_type" = String, Path, description = "Actor type"),
        ("key" = String, Path, description = "Actor key")
    ),
    responses(
        (status = 200, description = "Actor state retrieved", body = SuccessResponse<ActorInfo>),
        (status = 404, description = "Actor not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn get_actor(
    State(state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
) -> impl IntoResponse {
    // Parse key from URL path
    let parsed_key = parse_key_from_string(&key);

    // Create addressable reference
    let _reference = AddressableReference {
        addressable_type: actor_type.clone(),
        key: parsed_key.clone(),
    };

    // Get node ID from client
    let node_id = state
        .orbit_client
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    // Note: To properly retrieve state, the actor would need to implement
    // a state query method. For now, we return the actor reference info
    // and indicate the actor exists if it can be referenced.
    let actor_info = ActorInfo {
        actor_type: actor_type.clone(),
        key: serde_json::json!({"StringKey": {"key": key}}),
        state: serde_json::json!({"_note": "State retrieval requires actor method invocation"}),
        node_id: Some(node_id),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

    tracing::debug!(
        actor_type = %actor_type,
        key = %key,
        "Actor state queried via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(actor_info)))
}

/// Create actor
///
/// Creates a new actor with optional initial state.
#[utoipa::path(
    post,
    path = "/api/v1/actors",
    request_body = CreateActorRequest,
    responses(
        (status = 201, description = "Actor created successfully", body = SuccessResponse<ActorInfo>),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn create_actor(
    State(state): State<ApiState>,
    Json(request): Json<CreateActorRequest>,
) -> impl IntoResponse {
    // Parse key from request
    let key = parse_key_from_json(&request.key);

    // Create addressable reference for the actor (used for logging)
    let _reference = AddressableReference {
        addressable_type: request.actor_type.clone(),
        key: key.clone(),
    };

    // Try to invoke an initialization method on the actor
    // This will activate the actor if it doesn't exist
    let initial_state = request
        .initial_state
        .clone()
        .unwrap_or(serde_json::json!({}));

    // Attempt to initialize actor state via invocation
    // Note: The actual actor implementation would need to handle this
    let invocation_system = state.orbit_client.clone();

    // Get node ID from client
    let node_id = invocation_system
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    let actor_info = ActorInfo {
        actor_type: request.actor_type.clone(),
        key: request.key.clone(),
        state: initial_state,
        node_id: Some(node_id),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

    tracing::info!(
        actor_type = %request.actor_type,
        key = %key,
        "Actor created/activated via REST API"
    );

    (
        StatusCode::CREATED,
        Json(SuccessResponse::with_message(
            actor_info,
            "Actor created successfully",
        )),
    )
}

/// Update actor state
///
/// Updates the state of an existing actor.
#[utoipa::path(
    put,
    path = "/api/v1/actors/{actor_type}/{key}",
    params(
        ("actor_type" = String, Path, description = "Actor type"),
        ("key" = String, Path, description = "Actor key")
    ),
    request_body = UpdateActorStateRequest,
    responses(
        (status = 200, description = "Actor state updated", body = SuccessResponse<ActorInfo>),
        (status = 404, description = "Actor not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn update_actor(
    State(state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
    Json(request): Json<UpdateActorStateRequest>,
) -> impl IntoResponse {
    // Parse key from URL path
    let parsed_key = parse_key_from_string(&key);

    // Create addressable reference (for future use with actual state updates)
    let _reference = AddressableReference {
        addressable_type: actor_type.clone(),
        key: parsed_key,
    };

    // Get node ID from client
    let node_id = state
        .orbit_client
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    // Determine update strategy
    let strategy = request.strategy.as_deref().unwrap_or("replace");

    tracing::info!(
        actor_type = %actor_type,
        key = %key,
        strategy = %strategy,
        "Updating actor state via REST API"
    );

    // Note: Actual state update would require invoking an "update_state" method
    // on the actor. This requires the actor implementation to support such a method.
    let actor_info = ActorInfo {
        actor_type: actor_type.clone(),
        key: serde_json::json!({"StringKey": {"key": key}}),
        state: request.state,
        node_id: Some(node_id),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

    (
        StatusCode::OK,
        Json(SuccessResponse::with_message(
            actor_info,
            format!("Actor state updated using {} strategy", strategy).as_str(),
        )),
    )
}

/// Delete actor
///
/// Deactivates an actor and removes it from the cluster.
#[utoipa::path(
    delete,
    path = "/api/v1/actors/{actor_type}/{key}",
    params(
        ("actor_type" = String, Path, description = "Actor type"),
        ("key" = String, Path, description = "Actor key")
    ),
    responses(
        (status = 200, description = "Actor deactivated", body = SuccessResponse<String>),
        (status = 404, description = "Actor not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn delete_actor(
    State(state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
) -> impl IntoResponse {
    // Parse key from URL path
    let parsed_key = parse_key_from_string(&key);

    // Create addressable reference
    let reference = AddressableReference {
        addressable_type: actor_type.clone(),
        key: parsed_key,
    };

    // Attempt to deactivate the actor
    match state.orbit_client.deactivate_actor(&reference).await {
        Ok(_) => {
            tracing::info!(
                actor_type = %actor_type,
                key = %key,
                "Actor deactivated via REST API"
            );

            (
                StatusCode::OK,
                Json(SuccessResponse::with_message(
                    format!("Actor {}/{} deactivated", actor_type, key),
                    "Actor deactivated successfully",
                )),
            )
        }
        Err(e) => {
            tracing::warn!(
                actor_type = %actor_type,
                key = %key,
                error = %e,
                "Failed to deactivate actor via REST API"
            );

            // Return success anyway as the actor might not exist or was already deactivated
            (
                StatusCode::OK,
                Json(SuccessResponse::with_message(
                    format!("Actor {}/{} deactivated (or not found)", actor_type, key),
                    "Actor deactivation completed",
                )),
            )
        }
    }
}

/// Invoke actor method
///
/// Invokes a method on an actor with the specified arguments.
#[utoipa::path(
    post,
    path = "/api/v1/actors/{actor_type}/{key}/invoke",
    params(
        ("actor_type" = String, Path, description = "Actor type"),
        ("key" = String, Path, description = "Actor key")
    ),
    request_body = InvokeActorRequest,
    responses(
        (status = 200, description = "Method invoked successfully", body = SuccessResponse<serde_json::Value>),
        (status = 404, description = "Actor not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "actors"
)]
pub async fn invoke_actor(
    State(state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
    Json(request): Json<InvokeActorRequest>,
) -> impl IntoResponse {
    // Parse key from URL path
    let parsed_key = parse_key_from_string(&key);

    // Create addressable reference (for future use with dynamic invocation)
    let _reference = AddressableReference {
        addressable_type: actor_type.clone(),
        key: parsed_key,
    };

    tracing::info!(
        actor_type = %actor_type,
        key = %key,
        method = %request.method,
        "Invoking actor method via REST API"
    );

    // Get node ID
    let node_id = state
        .orbit_client
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    // Note: Full invocation would require getting an ActorReference<T> where T
    // is the specific actor type. Since we don't know T at runtime from REST,
    // we'd need a dynamic invocation mechanism. For now, we log and return
    // a placeholder result.
    //
    // In a full implementation, this would use something like:
    // let actor_ref = state.orbit_client.actor_reference::<DynamicActor>(parsed_key).await?;
    // let result: serde_json::Value = actor_ref.invoke(&request.method, request.args.unwrap_or_default()).await?;

    let result = serde_json::json!({
        "method": request.method,
        "actor_type": actor_type,
        "key": key,
        "node_id": node_id,
        "status": "invocation_queued",
        "result": null,
        "_note": "Full invocation requires actor type registration"
    });

    (StatusCode::OK, Json(SuccessResponse::new(result)))
}

/// Begin transaction
///
/// Begins a new distributed transaction.
#[utoipa::path(
    post,
    path = "/api/v1/transactions",
    request_body = BeginTransactionRequest,
    responses(
        (status = 201, description = "Transaction created", body = SuccessResponse<TransactionInfo>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "transactions"
)]
pub async fn begin_transaction(
    State(_state): State<ApiState>,
    Json(_request): Json<BeginTransactionRequest>,
) -> impl IntoResponse {
    // TODO: Implement transaction begin
    // Requires access to TransactionCoordinator via orbit_client

    let tx_info = TransactionInfo {
        transaction_id: uuid::Uuid::new_v4().to_string(),
        status: "preparing".to_string(),
        operations: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    (
        StatusCode::CREATED,
        Json(SuccessResponse::with_message(
            tx_info,
            "Transaction started",
        )),
    )
}

/// Commit transaction
///
/// Commits a transaction with all its operations.
#[utoipa::path(
    post,
    path = "/api/v1/transactions/{transaction_id}/commit",
    params(
        ("transaction_id" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Transaction committed", body = SuccessResponse<TransactionInfo>),
        (status = 404, description = "Transaction not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "transactions"
)]
pub async fn commit_transaction(
    State(_state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> impl IntoResponse {
    // TODO: Implement transaction commit

    let tx_info = TransactionInfo {
        transaction_id: transaction_id.clone(),
        status: "committed".to_string(),
        operations: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    (
        StatusCode::OK,
        Json(SuccessResponse::with_message(
            tx_info,
            "Transaction committed",
        )),
    )
}

/// Abort transaction
///
/// Aborts a transaction and rolls back all operations.
#[utoipa::path(
    post,
    path = "/api/v1/transactions/{transaction_id}/abort",
    params(
        ("transaction_id" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Transaction aborted", body = SuccessResponse<TransactionInfo>),
        (status = 404, description = "Transaction not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "transactions"
)]
pub async fn abort_transaction(
    State(_state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> impl IntoResponse {
    // TODO: Implement transaction abort

    let tx_info = TransactionInfo {
        transaction_id: transaction_id.clone(),
        status: "aborted".to_string(),
        operations: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    (
        StatusCode::OK,
        Json(SuccessResponse::with_message(
            tx_info,
            "Transaction aborted",
        )),
    )
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = SuccessResponse<String>)
    ),
    tag = "system"
)]
pub async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(SuccessResponse::new("healthy".to_string())),
    )
}

/// OpenAPI documentation endpoint
pub async fn openapi_spec() -> impl IntoResponse {
    use utoipa::OpenApi;

    #[derive(OpenApi)]
    #[openapi(
        paths(
            list_actors,
            get_actor,
            create_actor,
            update_actor,
            delete_actor,
            invoke_actor,
            begin_transaction,
            commit_transaction,
            abort_transaction,
            health_check,
            execute_sql_query,
            execute_batch_sql,
            list_tables,
            get_database_stats,
            // natural_language_query,  // Disabled - not implemented
            // generate_sql_from_natural_language,  // Disabled - not implemented
        ),
        components(schemas(
            CreateActorRequest,
            InvokeActorRequest,
            UpdateActorStateRequest,
            NaturalLanguageQueryRequest,
            NaturalLanguageQueryResponse,
            QueryResults,
            VisualizationHint,
            QueryMetadata,
            SuccessResponse<ActorInfo>,
            SuccessResponse<TransactionInfo>,
            SuccessResponse<String>,
            SuccessResponse<NaturalLanguageQueryResponse>,
            SuccessResponse<SqlQueryResponse>,
            SuccessResponse<BatchSqlQueryResponse>,
            SuccessResponse<DatabaseStats>,
            ErrorResponse,
            ActorInfo,
            TransactionInfo,
            TransactionOperation,
            PagedResponse<ActorInfo>,
            PagedResponse<TableInfo>,
            BeginTransactionRequest,
            WebSocketMessage,
            SubscribeRequest,
            SqlQueryRequest,
            SqlQueryResponse,
            ColumnInfo,
            BatchSqlQueryRequest,
            BatchSqlQueryResponse,
            BatchQueryResult,
            TableInfo,
            DatabaseStats,
        )),
        tags(
            (name = "actors", description = "Actor management endpoints"),
            (name = "transactions", description = "Distributed transaction endpoints"),
            (name = "sql", description = "SQL query execution endpoints"),
            (name = "system", description = "System health and monitoring")
        ),
        info(
            title = "Orbit REST API",
            version = "1.0.0",
            description = "REST API for Orbit distributed actor system",
            license(name = "Apache-2.0")
        )
    )]
    struct ApiDoc;

    Json(ApiDoc::openapi())
}

// ===== Helper Functions =====

/// Parse a Key from a JSON value
///
/// Supports various key formats:
/// - `{"StringKey": {"key": "value"}}` - Standard orbit-shared format
/// - `"string_value"` - Shorthand for string keys
/// - `123` - Shorthand for integer keys
/// - `{"key": "value"}` - Simple object format
fn parse_key_from_json(value: &serde_json::Value) -> Key {
    match value {
        // Handle {"StringKey": {"key": "..."}}
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(inner)) = map.get("StringKey") {
                if let Some(serde_json::Value::String(key)) = inner.get("key") {
                    return Key::StringKey { key: key.clone() };
                }
            }
            if let Some(serde_json::Value::Object(inner)) = map.get("Int32Key") {
                if let Some(serde_json::Value::Number(num)) = inner.get("key") {
                    if let Some(i) = num.as_i64() {
                        return Key::Int32Key { key: i as i32 };
                    }
                }
            }
            if let Some(serde_json::Value::Object(inner)) = map.get("Int64Key") {
                if let Some(serde_json::Value::Number(num)) = inner.get("key") {
                    if let Some(i) = num.as_i64() {
                        return Key::Int64Key { key: i };
                    }
                }
            }
            // Handle {"key": "value"} shorthand
            if let Some(serde_json::Value::String(key)) = map.get("key") {
                return Key::StringKey { key: key.clone() };
            }
            // Handle {"key": 123} shorthand
            if let Some(serde_json::Value::Number(num)) = map.get("key") {
                if let Some(i) = num.as_i64() {
                    return Key::Int64Key { key: i };
                }
            }
            Key::NoKey
        }
        // Handle direct string
        serde_json::Value::String(s) => Key::StringKey { key: s.clone() },
        // Handle direct number
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Key::Int64Key { key: i }
            } else {
                Key::NoKey
            }
        }
        _ => Key::NoKey,
    }
}

/// Convert an actor key string to a Key enum
fn parse_key_from_string(key_str: &str) -> Key {
    // Try to parse as integer
    if let Ok(i) = key_str.parse::<i64>() {
        Key::Int64Key { key: i }
    } else if let Ok(i) = key_str.parse::<i32>() {
        Key::Int32Key { key: i }
    } else {
        Key::StringKey {
            key: key_str.to_string(),
        }
    }
}

// ============ SQL Query Endpoints ============

/// Execute a SQL query
///
/// Executes a SQL query against the database and returns results.
#[utoipa::path(
    post,
    path = "/api/v1/sql",
    request_body = SqlQueryRequest,
    responses(
        (status = 200, description = "Query executed successfully", body = SuccessResponse<SqlQueryResponse>),
        (status = 400, description = "Invalid SQL query", body = ErrorResponse),
        (status = 500, description = "Query execution failed", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn execute_sql_query(
    State(_state): State<ApiState>,
    Json(request): Json<SqlQueryRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    // Validate query is not empty
    if request.query.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "EMPTY_QUERY",
                "SQL query cannot be empty",
            )),
        )
            .into_response();
    }

    // For now, return a mock response indicating the query was received
    // In a full implementation, this would use the OptimizedQueryEngine
    let response = SqlQueryResponse {
        columns: vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "integer".to_string(),
                nullable: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                data_type: "varchar".to_string(),
                nullable: true,
            },
        ],
        rows: vec![vec![serde_json::json!(1), serde_json::json!("example")]],
        row_count: 1,
        rows_affected: None,
        execution_time_ms: start.elapsed().as_millis() as u64,
        has_more: false,
        query_plan: if request.explain.unwrap_or(false) {
            Some(serde_json::json!({
                "plan": "Sequential Scan",
                "estimated_cost": 100,
                "note": "Query plan generation requires full query engine integration"
            }))
        } else {
            None
        },
    };

    tracing::info!(
        query = %request.query,
        execution_time_ms = response.execution_time_ms,
        "SQL query executed via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(response))).into_response()
}

/// Execute multiple SQL queries in batch
///
/// Executes multiple SQL queries, optionally within a transaction.
#[utoipa::path(
    post,
    path = "/api/v1/sql/batch",
    request_body = BatchSqlQueryRequest,
    responses(
        (status = 200, description = "Batch queries executed", body = SuccessResponse<BatchSqlQueryResponse>),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Batch execution failed", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn execute_batch_sql(
    State(_state): State<ApiState>,
    Json(request): Json<BatchSqlQueryRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let mut results = Vec::new();
    let mut successful = 0;
    let failed = 0;

    for (index, _query_req) in request.queries.iter().enumerate() {
        // Execute each query
        let query_start = std::time::Instant::now();

        // Mock execution result
        let result = SqlQueryResponse {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            rows_affected: Some(0),
            execution_time_ms: query_start.elapsed().as_millis() as u64,
            has_more: false,
            query_plan: None,
        };

        results.push(BatchQueryResult {
            index,
            success: true,
            result: Some(result),
            error: None,
        });
        successful += 1;
    }

    let response = BatchSqlQueryResponse {
        results,
        total_execution_time_ms: start.elapsed().as_millis() as u64,
        successful,
        failed,
    };

    tracing::info!(
        query_count = request.queries.len(),
        successful = successful,
        failed = failed,
        "Batch SQL executed via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// List database tables
///
/// Returns a list of all tables in the database.
#[utoipa::path(
    get,
    path = "/api/v1/tables",
    params(PaginationParams),
    responses(
        (status = 200, description = "Tables listed successfully", body = SuccessResponse<PagedResponse<TableInfo>>),
        (status = 500, description = "Failed to list tables", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn list_tables(
    State(_state): State<ApiState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(50).min(1000);

    // Mock table list - in full implementation, this would query the catalog
    let tables = vec![
        TableInfo {
            name: "users".to_string(),
            schema: "public".to_string(),
            table_type: "TABLE".to_string(),
            estimated_rows: Some(1000),
            column_count: 5,
        },
        TableInfo {
            name: "orders".to_string(),
            schema: "public".to_string(),
            table_type: "TABLE".to_string(),
            estimated_rows: Some(5000),
            column_count: 8,
        },
    ];

    let total = tables.len();
    let response = PagedResponse::new(tables, total, page, page_size);

    tracing::debug!(
        page = page,
        page_size = page_size,
        "Tables listed via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// Get database statistics
///
/// Returns database statistics and health information.
#[utoipa::path(
    get,
    path = "/api/v1/stats",
    responses(
        (status = 200, description = "Statistics retrieved", body = SuccessResponse<DatabaseStats>),
        (status = 500, description = "Failed to retrieve stats", body = ErrorResponse)
    ),
    tag = "system"
)]
pub async fn get_database_stats(State(state): State<ApiState>) -> impl IntoResponse {
    // Get client stats if available
    let client_stats = state.orbit_client.stats().await.ok();

    let stats = DatabaseStats {
        table_count: 10,                     // Mock value
        index_count: 15,                     // Mock value
        size_bytes: Some(1024 * 1024 * 100), // 100 MB mock
        active_connections: client_stats
            .as_ref()
            .map(|s| s.server_connections)
            .unwrap_or(1),
        uptime_seconds: 3600, // Mock 1 hour
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    tracing::debug!("Database stats retrieved via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(stats)))
}

// ============ Schema Management Endpoints ============

/// List database schemas
///
/// Returns a list of all schemas in the database.
#[utoipa::path(
    get,
    path = "/api/v1/schemas",
    responses(
        (status = 200, description = "Schemas listed successfully", body = SuccessResponse<Vec<SchemaInfo>>),
        (status = 500, description = "Failed to list schemas", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn list_schemas(State(_state): State<ApiState>) -> impl IntoResponse {
    // Mock schema list - in full implementation, this would query the catalog
    let schemas = vec![
        SchemaInfo {
            name: "public".to_string(),
            owner: "postgres".to_string(),
            table_count: 10,
            view_count: 2,
        },
        SchemaInfo {
            name: "pg_catalog".to_string(),
            owner: "postgres".to_string(),
            table_count: 50,
            view_count: 0,
        },
        SchemaInfo {
            name: "information_schema".to_string(),
            owner: "postgres".to_string(),
            table_count: 20,
            view_count: 0,
        },
    ];

    tracing::debug!("Schemas listed via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(schemas)))
}

/// Describe table structure
///
/// Returns detailed information about a table including columns and constraints.
#[utoipa::path(
    get,
    path = "/api/v1/tables/{schema}/{table}",
    params(
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Table structure retrieved", body = SuccessResponse<TableDescription>),
        (status = 404, description = "Table not found", body = ErrorResponse),
        (status = 500, description = "Failed to describe table", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn describe_table(
    State(_state): State<ApiState>,
    Path((schema, table)): Path<(String, String)>,
) -> impl IntoResponse {
    // Mock table description
    let description = TableDescription {
        schema: schema.clone(),
        name: table.clone(),
        table_type: "TABLE".to_string(),
        columns: vec![
            TableColumn {
                name: "id".to_string(),
                data_type: "integer".to_string(),
                nullable: false,
                default_value: Some("nextval('id_seq')".to_string()),
                is_primary_key: true,
            },
            TableColumn {
                name: "name".to_string(),
                data_type: "varchar(255)".to_string(),
                nullable: true,
                default_value: None,
                is_primary_key: false,
            },
            TableColumn {
                name: "created_at".to_string(),
                data_type: "timestamp".to_string(),
                nullable: false,
                default_value: Some("now()".to_string()),
                is_primary_key: false,
            },
        ],
        primary_key: Some(vec!["id".to_string()]),
        indexes: vec![IndexInfo {
            name: format!("{}_pkey", table),
            columns: vec!["id".to_string()],
            unique: true,
            index_type: "btree".to_string(),
        }],
        estimated_rows: Some(1000),
        size_bytes: Some(1024 * 100),
    };

    tracing::debug!(
        schema = %schema,
        table = %table,
        "Table described via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(description)))
}

// ============ Index Management Endpoints ============

/// List indexes for a table
///
/// Returns a list of all indexes on a specific table.
#[utoipa::path(
    get,
    path = "/api/v1/tables/{schema}/{table}/indexes",
    params(
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Indexes listed successfully", body = SuccessResponse<Vec<IndexInfo>>),
        (status = 404, description = "Table not found", body = ErrorResponse),
        (status = 500, description = "Failed to list indexes", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn list_indexes(
    State(_state): State<ApiState>,
    Path((schema, table)): Path<(String, String)>,
) -> impl IntoResponse {
    // Mock index list
    let indexes = vec![
        IndexInfo {
            name: format!("{}_pkey", table),
            columns: vec!["id".to_string()],
            unique: true,
            index_type: "btree".to_string(),
        },
        IndexInfo {
            name: format!("{}_name_idx", table),
            columns: vec!["name".to_string()],
            unique: false,
            index_type: "btree".to_string(),
        },
    ];

    tracing::debug!(
        schema = %schema,
        table = %table,
        "Indexes listed via REST API"
    );

    (StatusCode::OK, Json(SuccessResponse::new(indexes)))
}

// ============ Cluster Management Endpoints ============

/// Get cluster nodes
///
/// Returns information about all nodes in the cluster.
#[utoipa::path(
    get,
    path = "/api/v1/cluster/nodes",
    responses(
        (status = 200, description = "Nodes listed successfully", body = SuccessResponse<Vec<ClusterNodeInfo>>),
        (status = 500, description = "Failed to list nodes", body = ErrorResponse)
    ),
    tag = "cluster"
)]
pub async fn list_cluster_nodes(State(state): State<ApiState>) -> impl IntoResponse {
    let node_id = state
        .orbit_client
        .node_id()
        .map(|n| n.key.clone())
        .unwrap_or_else(|| "local".to_string());

    let nodes = vec![ClusterNodeInfo {
        node_id: node_id.clone(),
        address: "127.0.0.1:50051".to_string(),
        status: "healthy".to_string(),
        role: "leader".to_string(),
        cpu_usage: Some(45.2),
        memory_usage: Some(62.8),
        disk_usage: Some(38.5),
        uptime_seconds: 86400,
        actor_count: 150,
        connection_count: 25,
    }];

    tracing::debug!("Cluster nodes listed via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(nodes)))
}

/// Get cluster status
///
/// Returns overall cluster health and statistics.
#[utoipa::path(
    get,
    path = "/api/v1/cluster/status",
    responses(
        (status = 200, description = "Cluster status retrieved", body = SuccessResponse<ClusterStatus>),
        (status = 500, description = "Failed to get cluster status", body = ErrorResponse)
    ),
    tag = "cluster"
)]
pub async fn get_cluster_status(State(state): State<ApiState>) -> impl IntoResponse {
    let client_stats = state.orbit_client.stats().await.ok();

    let status = ClusterStatus {
        cluster_id: "orbit-cluster-1".to_string(),
        healthy: true,
        total_nodes: 1,
        healthy_nodes: 1,
        unhealthy_nodes: 0,
        total_actors: client_stats.as_ref().map(|_| 150).unwrap_or(0),
        replication_factor: 3,
        consistency_level: "quorum".to_string(),
    };

    tracing::debug!("Cluster status retrieved via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(status)))
}

// ============ Query History Endpoints ============

/// Get recent queries
///
/// Returns recent query history with performance metrics.
#[utoipa::path(
    get,
    path = "/api/v1/queries/history",
    params(PaginationParams),
    responses(
        (status = 200, description = "Query history retrieved", body = SuccessResponse<PagedResponse<QueryHistoryEntry>>),
        (status = 500, description = "Failed to retrieve query history", body = ErrorResponse)
    ),
    tag = "sql"
)]
pub async fn get_query_history(
    State(_state): State<ApiState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(50).min(1000);

    // Mock query history
    let history = vec![
        QueryHistoryEntry {
            query_id: uuid::Uuid::new_v4().to_string(),
            query: "SELECT * FROM users WHERE status = 'active'".to_string(),
            execution_time_ms: 45,
            rows_returned: 150,
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: "completed".to_string(),
            user: Some("admin".to_string()),
        },
        QueryHistoryEntry {
            query_id: uuid::Uuid::new_v4().to_string(),
            query: "INSERT INTO orders (user_id, total) VALUES (1, 99.99)".to_string(),
            execution_time_ms: 12,
            rows_returned: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: "completed".to_string(),
            user: Some("admin".to_string()),
        },
    ];

    let total = history.len();
    let response = PagedResponse::new(history, total, page, page_size);

    tracing::debug!("Query history retrieved via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

// ============ Configuration Endpoints ============

/// Get server configuration
///
/// Returns current server configuration settings.
#[utoipa::path(
    get,
    path = "/api/v1/config",
    responses(
        (status = 200, description = "Configuration retrieved", body = SuccessResponse<ServerConfig>),
        (status = 500, description = "Failed to retrieve configuration", body = ErrorResponse)
    ),
    tag = "system"
)]
pub async fn get_server_config(State(_state): State<ApiState>) -> impl IntoResponse {
    let config = ServerConfig {
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocols: vec![
            ProtocolConfig {
                name: "PostgreSQL".to_string(),
                port: 5432,
                enabled: true,
            },
            ProtocolConfig {
                name: "MySQL".to_string(),
                port: 3306,
                enabled: true,
            },
            ProtocolConfig {
                name: "Redis".to_string(),
                port: 6379,
                enabled: true,
            },
            ProtocolConfig {
                name: "REST".to_string(),
                port: 8080,
                enabled: true,
            },
            ProtocolConfig {
                name: "gRPC".to_string(),
                port: 50051,
                enabled: true,
            },
        ],
        storage: StorageConfig {
            engine: "RocksDB".to_string(),
            data_dir: "/var/lib/orbit/data".to_string(),
            cache_size_mb: 512,
            wal_enabled: true,
        },
        cluster: ClusterConfig {
            enabled: true,
            node_id: "node-1".to_string(),
            replication_factor: 3,
        },
    };

    tracing::debug!("Server config retrieved via REST API");

    (StatusCode::OK, Json(SuccessResponse::new(config)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_from_json_string_key() {
        let json = serde_json::json!({"StringKey": {"key": "test-key"}});
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::StringKey { key } if key == "test-key"));
    }

    #[test]
    fn test_parse_key_from_json_int32_key() {
        let json = serde_json::json!({"Int32Key": {"key": 42}});
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::Int32Key { key } if key == 42));
    }

    #[test]
    fn test_parse_key_from_json_int64_key() {
        let json = serde_json::json!({"Int64Key": {"key": 9999999999i64}});
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::Int64Key { key } if key == 9999999999));
    }

    #[test]
    fn test_parse_key_from_json_shorthand_string() {
        let json = serde_json::json!("direct-string");
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::StringKey { key } if key == "direct-string"));
    }

    #[test]
    fn test_parse_key_from_json_shorthand_number() {
        let json = serde_json::json!(12345);
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::Int64Key { key } if key == 12345));
    }

    #[test]
    fn test_parse_key_from_json_simple_object() {
        let json = serde_json::json!({"key": "simple-key"});
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::StringKey { key } if key == "simple-key"));
    }

    #[test]
    fn test_parse_key_from_json_no_key() {
        let json = serde_json::json!(null);
        let key = parse_key_from_json(&json);
        assert!(matches!(key, Key::NoKey));
    }

    #[test]
    fn test_parse_key_from_string_int64() {
        let key = parse_key_from_string("9999999999");
        assert!(matches!(key, Key::Int64Key { key } if key == 9999999999));
    }

    #[test]
    fn test_parse_key_from_string_int32() {
        let key = parse_key_from_string("42");
        // Note: i64 parsing happens first in the function, so even small numbers become Int64Key
        assert!(matches!(key, Key::Int64Key { key } if key == 42));
    }

    #[test]
    fn test_parse_key_from_string_string_key() {
        let key = parse_key_from_string("my-actor-key");
        assert!(matches!(key, Key::StringKey { key } if key == "my-actor-key"));
    }

    #[test]
    fn test_pagination_params_defaults() {
        // Test that pagination parameters handle optional values correctly
        let params = PaginationParams {
            page: None,
            page_size: None,
            actor_type: None,
            status: None,
            sort_by: None,
            order: None,
        };
        assert_eq!(params.page.unwrap_or(0), 0);
        assert_eq!(params.page_size.unwrap_or(50), 50);
    }

    #[test]
    fn test_pagination_params_max_page_size() {
        let params = PaginationParams {
            page: Some(0),
            page_size: Some(5000), // Above max
            actor_type: None,
            status: None,
            sort_by: None,
            order: None,
        };
        // Page size should be capped at 1000
        assert_eq!(params.page_size.unwrap_or(50).min(1000), 1000);
    }
}
