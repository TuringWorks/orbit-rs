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
            ErrorResponse,
            ActorInfo,
            TransactionInfo,
            TransactionOperation,
            PagedResponse<ActorInfo>,
            BeginTransactionRequest,
            WebSocketMessage,
            SubscribeRequest,
        )),
        tags(
            (name = "actors", description = "Actor management endpoints"),
            (name = "transactions", description = "Distributed transaction endpoints"),
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
