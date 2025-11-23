//! REST API request handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use orbit_client::OrbitClient;
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
    State(_state): State<ApiState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(50).min(1000);

    // TODO: Implement actual actor listing via OrbitClient
    // This would require adding a directory query API to orbit-client
    let actors = vec![ActorInfo {
        actor_type: "GreeterActor".to_string(),
        key: serde_json::json!({"StringKey": {"key": "greeter-1"}}),
        state: serde_json::json!({"greetings": 42}),
        node_id: Some("node-1".to_string()),
        status: "active".to_string(),
        last_activity: Some("2024-01-15T10:30:00Z".to_string()),
    }];

    let total = actors.len();
    let response = PagedResponse::new(actors, total, page, page_size);

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
    State(_state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
) -> impl IntoResponse {
    // TODO: Implement actor state retrieval
    // 1. Parse key to appropriate Key type
    // 2. Get actor reference via orbit_client.actor_reference()
    // 3. Retrieve state via custom state query method

    let actor_info = ActorInfo {
        actor_type: actor_type.clone(),
        key: serde_json::json!({"StringKey": {"key": key}}),
        state: serde_json::json!({}),
        node_id: Some("node-1".to_string()),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

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
    State(_state): State<ApiState>,
    Json(request): Json<CreateActorRequest>,
) -> impl IntoResponse {
    // TODO: Implement actor creation
    // 1. Parse key from request.key
    // 2. Create actor reference
    // 3. If initial_state provided, set it
    // 4. Trigger activation via a method call

    let actor_info = ActorInfo {
        actor_type: request.actor_type.clone(),
        key: request.key.clone(),
        state: request.initial_state.unwrap_or(serde_json::json!({})),
        node_id: Some("node-1".to_string()),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

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
    State(_state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
    Json(request): Json<UpdateActorStateRequest>,
) -> impl IntoResponse {
    // TODO: Implement state update
    // 1. Get actor reference
    // 2. Call state update method based on strategy (replace/merge)

    let actor_info = ActorInfo {
        actor_type: actor_type.clone(),
        key: serde_json::json!({"StringKey": {"key": key}}),
        state: request.state,
        node_id: Some("node-1".to_string()),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };

    (
        StatusCode::OK,
        Json(SuccessResponse::with_message(
            actor_info,
            "Actor state updated",
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
    State(_state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
) -> impl IntoResponse {
    // TODO: Implement actor deactivation
    // 1. Get actor reference
    // 2. Call deactivate method or send shutdown signal

    (
        StatusCode::OK,
        Json(SuccessResponse::with_message(
            format!("Actor {}/{} deactivated", actor_type, key),
            "Actor deactivated successfully",
        )),
    )
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
    State(_state): State<ApiState>,
    Path((_actor_type, _key)): Path<(String, String)>,
    Json(request): Json<InvokeActorRequest>,
) -> impl IntoResponse {
    // TODO: Implement method invocation
    // 1. Get actor reference
    // 2. Serialize arguments
    // 3. Invoke method via orbit_client
    // 4. Deserialize and return result

    let result = serde_json::json!({
        "method": request.method,
        "result": "Method executed successfully"
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
