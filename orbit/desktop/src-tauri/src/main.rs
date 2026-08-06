//! Orbit Desktop — a database client for Orbit-RS.
//!
//! Provides connection management, statement execution against Orbit's wire
//! protocols, and lifecycle control for a local development cluster.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;

mod cluster;
mod connections;
mod encryption;
mod models;
mod queries;
mod storage;

use cluster::{ClusterManager, ClusterStatus};
use connections::{Connection, ConnectionInfo, ConnectionManager, ConnectionStatus, ConnectionType};
use encryption::EncryptionManager;
use models::{MLFunctionInfo, ModelInfo, ModelManager};
use queries::{QueryExecutor, QueryHistoryEntry, QueryRequest, QueryResult};
use storage::StorageManager;

/// Shared application state.
///
/// [`ConnectionManager`] locks internally, so it is held behind an `Arc` rather
/// than an outer lock: opening a session for one connection must not block
/// queries running against another.
struct AppState {
    connections: Arc<ConnectionManager>,
    query_executor: RwLock<QueryExecutor>,
    model_manager: RwLock<ModelManager>,
    storage: StorageManager,
    encryption: EncryptionManager,
    /// The Orbit-RS checkout whose cluster this app manages, once located.
    cluster_root: RwLock<Option<ClusterManager>>,
}

/// Uniform envelope for every command result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    fn error(message: impl std::fmt::Display) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Run a fallible body, turning its error into an [`ApiResponse::error`].
///
/// The outer `Result` is `Ok` for both outcomes: Tauri's `Err` channel loses
/// the envelope, so failures travel in the payload where the UI can show them.
macro_rules! respond {
    ($body:expr) => {
        match $body {
            Ok(value) => Ok(ApiResponse::success(value)),
            Err(e) => Ok(ApiResponse::error(e)),
        }
    };
}

// ============================ Connections ============================

/// Persist the current in-memory connection list.
async fn persist_connections(state: &AppState) -> Result<(), String> {
    let mut storage = state
        .storage
        .load()
        .map_err(|e| format!("Failed to load storage: {e}"))?;

    let connections = state.connections.list_connections().await;
    storage.connections = connections
        .iter()
        .map(|connection| connection.to_stored(&state.encryption))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to encrypt connections: {e}"))?;

    state
        .storage
        .save(&storage)
        .map_err(|e| format!("Failed to save storage: {e}"))
}

#[tauri::command]
async fn create_connection(
    connection_info: ConnectionInfo,
    state: State<'_, AppState>,
) -> Result<ApiResponse<String>, String> {
    let id = match state.connections.create_connection(connection_info).await {
        Ok(id) => id,
        Err(e) => return Ok(ApiResponse::error(e)),
    };

    if let Err(e) = persist_connections(&state).await {
        return Ok(ApiResponse::error(e));
    }

    Ok(ApiResponse::success(id))
}

#[tauri::command]
async fn test_connection(
    connection_info: ConnectionInfo,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ConnectionStatus>, String> {
    Ok(ApiResponse::success(
        state.connections.test_connection(&connection_info).await,
    ))
}

#[tauri::command]
async fn get_connections(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<Connection>>, String> {
    Ok(ApiResponse::success(
        state.connections.list_connections().await,
    ))
}

/// Open a session now rather than on first query, so the UI can report whether
/// a saved connection is actually usable.
#[tauri::command]
async fn connect(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ConnectionStatus>, String> {
    match state.connections.session(&connection_id).await {
        Ok(_) => Ok(ApiResponse::success(ConnectionStatus::Connected)),
        Err(e) => Ok(ApiResponse::error(e)),
    }
}

#[tauri::command]
async fn disconnect(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    state.connections.disconnect(&connection_id).await;
    Ok(ApiResponse::success(true))
}

#[tauri::command]
async fn delete_connection(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    state.connections.delete_connection(&connection_id).await;
    persist_connections(&state).await?;
    Ok(ApiResponse::success(true))
}

/// The connection types the UI can offer, with their default ports.
#[tauri::command]
async fn list_connection_types() -> Result<ApiResponse<Vec<ConnectionTypeInfo>>, String> {
    Ok(ApiResponse::success(
        ConnectionType::ALL
            .into_iter()
            .map(|connection_type| ConnectionTypeInfo {
                id: connection_type.as_str().to_string(),
                default_port: connection_type.default_port(),
                native_wire_protocol: connection_type.is_native_wire_protocol(),
            })
            .collect(),
    ))
}

/// A connection type as offered in the connection dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectionTypeInfo {
    id: String,
    default_port: u16,
    /// False for the HTTP-backed protocols, whose server handlers still return
    /// canned rows; the dialog warns instead of implying they return data.
    native_wire_protocol: bool,
}

// ============================ Queries ============================

#[tauri::command]
async fn execute_query(
    request: QueryRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let mut executor = state.query_executor.write().await;
    respond!(executor.execute(request, &state.connections).await)
}

#[tauri::command]
async fn explain_query(
    request: QueryRequest,
    analyze: Option<bool>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let mut executor = state.query_executor.write().await;
    respond!(
        executor
            .explain(request, analyze.unwrap_or(false), &state.connections)
            .await
    )
}

#[tauri::command]
async fn get_query_history(
    connection_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<QueryHistoryEntry>>, String> {
    let executor = state.query_executor.read().await;
    Ok(ApiResponse::success(
        executor.history(&connection_id, limit.unwrap_or(50)),
    ))
}

// ============================ Cluster ============================

/// Resolve the cluster manager, or explain why there is none.
async fn with_cluster<T, F, Fut>(state: &AppState, action: F) -> Result<ApiResponse<T>, String>
where
    F: FnOnce(&ClusterManager) -> Fut,
    Fut: std::future::Future<Output = Result<T, cluster::ClusterError>>,
{
    let guard = state.cluster_root.read().await;
    let Some(manager) = guard.as_ref() else {
        return Ok(ApiResponse::error(cluster::ClusterError::RootNotSet));
    };
    respond!(action(manager).await)
}

#[tauri::command]
async fn get_cluster_status(state: State<'_, AppState>) -> Result<ApiResponse<ClusterStatus>, String> {
    with_cluster(&state, |manager| async move { manager.status().await }).await
}

/// Point the cluster panel at an Orbit-RS checkout.
#[tauri::command]
async fn set_cluster_root(
    path: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ClusterStatus>, String> {
    let manager = match ClusterManager::new(&path) {
        Ok(manager) => manager,
        Err(e) => return Ok(ApiResponse::error(e)),
    };

    let status = match manager.status().await {
        Ok(status) => status,
        Err(e) => return Ok(ApiResponse::error(e)),
    };

    *state.cluster_root.write().await = Some(manager);
    Ok(ApiResponse::success(status))
}

#[tauri::command]
async fn start_cluster(
    size: u8,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    with_cluster(&state, |manager| async move {
        manager.start(size).await.map(|()| true)
    })
    .await
}

#[tauri::command]
async fn stop_cluster(state: State<'_, AppState>) -> Result<ApiResponse<bool>, String> {
    with_cluster(&state, |manager| async move {
        manager.stop().await.map(|()| true)
    })
    .await
}

#[tauri::command]
async fn get_cluster_log(
    node_id: Option<String>,
    lines: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<String>, String> {
    let lines = lines.unwrap_or(200);
    with_cluster(&state, |manager| async move {
        match node_id {
            Some(node_id) => manager.node_log(&node_id, lines),
            None => manager.control_log(lines),
        }
    })
    .await
}

// ============================ ML models ============================

#[tauri::command]
async fn list_ml_functions(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<MLFunctionInfo>>, String> {
    let manager = state.model_manager.read().await;
    respond!(manager.get_ml_functions().await)
}

#[tauri::command]
async fn list_models(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<ModelInfo>>, String> {
    let manager = state.model_manager.read().await;
    respond!(manager.get_models(&connection_id).await)
}

#[tauri::command]
async fn get_model_info(
    connection_id: String,
    model_name: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ModelInfo>, String> {
    let manager = state.model_manager.read().await;
    respond!(manager.get_model_info(&connection_id, &model_name).await)
}

#[tauri::command]
async fn delete_model(
    connection_id: String,
    model_name: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let manager = state.model_manager.read().await;
    respond!(manager
        .delete_model(&connection_id, &model_name)
        .await
        .map(|()| true))
}

// ============================ System ============================

#[tauri::command]
async fn get_system_info() -> Result<ApiResponse<HashMap<String, serde_json::Value>>, String> {
    let info = [
        ("version", env!("CARGO_PKG_VERSION").to_string()),
        ("os", std::env::consts::OS.to_string()),
        ("arch", std::env::consts::ARCH.to_string()),
        ("timestamp", chrono::Utc::now().to_rfc3339()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), serde_json::Value::String(value)))
    .collect();

    Ok(ApiResponse::success(info))
}

#[tauri::command]
async fn save_settings(
    settings: HashMap<String, serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let mut storage = state
        .storage
        .load()
        .map_err(|e| format!("Failed to load storage: {e}"))?;

    let current = &mut storage.settings;
    if let Some(value) = settings.get("theme").and_then(|v| v.as_str()) {
        current.theme = value.to_string();
    }
    if let Some(value) = settings.get("auto_save").and_then(serde_json::Value::as_bool) {
        current.auto_save = value;
    }
    if let Some(value) = settings.get("query_timeout").and_then(serde_json::Value::as_u64) {
        current.query_timeout = value;
    }
    if let Some(value) = settings
        .get("editor_font_size")
        .and_then(serde_json::Value::as_u64)
    {
        current.editor_font_size = value as u16;
    }
    if let Some(value) = settings.get("editor_theme").and_then(|v| v.as_str()) {
        current.editor_theme = value.to_string();
    }
    if let Some(value) = settings
        .get("show_line_numbers")
        .and_then(serde_json::Value::as_bool)
    {
        current.show_line_numbers = value;
    }
    if let Some(value) = settings.get("word_wrap").and_then(serde_json::Value::as_bool) {
        current.word_wrap = value;
    }
    if let Some(value) = settings.get("cluster_root").and_then(|v| v.as_str()) {
        current.cluster_root = Some(value.to_string());
    }

    state
        .storage
        .save(&storage)
        .map_err(|e| format!("Failed to save settings: {e}"))?;

    Ok(ApiResponse::success(true))
}

#[tauri::command]
async fn load_settings(
    state: State<'_, AppState>,
) -> Result<ApiResponse<HashMap<String, serde_json::Value>>, String> {
    let storage = state
        .storage
        .load()
        .map_err(|e| format!("Failed to load storage: {e}"))?;

    let settings = storage.settings;
    let map = [
        ("theme", serde_json::Value::String(settings.theme)),
        ("auto_save", serde_json::Value::Bool(settings.auto_save)),
        (
            "query_timeout",
            serde_json::Value::Number(settings.query_timeout.into()),
        ),
        (
            "editor_font_size",
            serde_json::Value::Number(settings.editor_font_size.into()),
        ),
        (
            "editor_theme",
            serde_json::Value::String(settings.editor_theme),
        ),
        (
            "show_line_numbers",
            serde_json::Value::Bool(settings.show_line_numbers),
        ),
        ("word_wrap", serde_json::Value::Bool(settings.word_wrap)),
        (
            "cluster_root",
            settings
                .cluster_root
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();

    Ok(ApiResponse::success(map))
}

#[tauri::command]
async fn show_about_dialog(app: tauri::AppHandle) {
    let Some(window) = app.get_window("main") else {
        tracing::warn!("about dialog requested but the main window is gone");
        return;
    };

    tauri::api::dialog::message(
        Some(&window),
        "About Orbit Desktop",
        format!(
            "Orbit Desktop v{}\n\nA desktop client for Orbit-RS: connection management, \
             SQL and Redis execution, and local cluster lifecycle control.",
            env!("CARGO_PKG_VERSION")
        ),
    );
}

// ============================ Startup ============================

/// Restore saved connections so they are queryable straight after launch.
///
/// Descriptions are registered without contacting any server; the session opens
/// on first use. A connection whose stored form cannot be decoded is reported
/// and skipped rather than aborting startup.
async fn restore_connections(
    storage: &StorageManager,
    encryption: &EncryptionManager,
    connections: &ConnectionManager,
) {
    let stored = match storage.load() {
        Ok(storage) => storage.connections,
        Err(e) => {
            tracing::error!("could not load saved connections: {e}");
            return;
        }
    };

    let mut restored = 0usize;
    for entry in &stored {
        match entry.to_connection(encryption) {
            Ok(connection) => {
                connections.register(connection).await;
                restored += 1;
            }
            Err(e) => tracing::warn!("skipping saved connection {}: {e}", entry.id),
        }
    }

    tracing::info!("restored {restored} of {} saved connections", stored.len());
}

/// Locate the Orbit-RS checkout to manage: the saved path if it still verifies,
/// otherwise a search upward from the working directory.
fn locate_cluster_root(saved: Option<&str>) -> Option<ClusterManager> {
    if let Some(path) = saved {
        match ClusterManager::new(path) {
            Ok(manager) => return Some(manager),
            Err(e) => tracing::warn!("saved cluster root is unusable: {e}"),
        }
    }

    std::env::current_dir()
        .ok()
        .and_then(|cwd| ClusterManager::discover(&cwd))
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("starting Orbit Desktop");

    let context = tauri::generate_context!();

    let storage = match StorageManager::new(context.config()) {
        Ok(storage) => storage,
        Err(e) => {
            tracing::error!("cannot initialise storage: {e}");
            std::process::exit(1);
        }
    };
    let encryption = match EncryptionManager::new(context.config()) {
        Ok(encryption) => encryption,
        Err(e) => {
            tracing::error!("cannot initialise encryption: {e}");
            std::process::exit(1);
        }
    };

    let saved_root = storage
        .load()
        .ok()
        .and_then(|storage| storage.settings.cluster_root);
    let cluster_root = locate_cluster_root(saved_root.as_deref());
    match &cluster_root {
        Some(manager) => tracing::info!("managing cluster at {}", manager.root().display()),
        None => tracing::info!("no Orbit-RS checkout found; set one in the cluster panel"),
    }

    let connections = Arc::new(ConnectionManager::new());

    let app_state = AppState {
        connections: Arc::clone(&connections),
        query_executor: RwLock::new(QueryExecutor::new()),
        model_manager: RwLock::new(ModelManager::new()),
        storage,
        encryption,
        cluster_root: RwLock::new(cluster_root),
    };

    tauri::Builder::default()
        .manage(app_state)
        .menu(create_menu())
        .on_menu_event(|event| match event.menu_item_id() {
            "quit" => std::process::exit(0),
            "about" => {
                let app = event.window().app_handle();
                tauri::async_runtime::spawn(show_about_dialog(app));
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            // Connections
            create_connection,
            test_connection,
            get_connections,
            connect,
            disconnect,
            delete_connection,
            list_connection_types,
            // Queries
            execute_query,
            explain_query,
            get_query_history,
            // Cluster
            get_cluster_status,
            set_cluster_root,
            start_cluster,
            stop_cluster,
            get_cluster_log,
            // ML models
            list_ml_functions,
            list_models,
            get_model_info,
            delete_model,
            // System
            get_system_info,
            save_settings,
            load_settings,
            show_about_dialog,
        ])
        .setup(move |app| {
            let state = app.state::<AppState>();
            let storage = state.storage.clone();
            let encryption = state.encryption.clone();
            let connections = Arc::clone(&connections);

            tauri::async_runtime::spawn(async move {
                restore_connections(&storage, &encryption, &connections).await;
            });

            Ok(())
        })
        .run(context)
        .expect("Tauri failed to start");
}

fn create_menu() -> tauri::Menu {
    use tauri::{CustomMenuItem, Menu, MenuItem, Submenu};

    let app_menu = Submenu::new(
        "Orbit Desktop",
        Menu::new()
            .add_item(CustomMenuItem::new("about", "About"))
            .add_native_item(MenuItem::Separator)
            .add_item(CustomMenuItem::new("quit", "Quit")),
    );

    Menu::new().add_submenu(app_menu)
}
