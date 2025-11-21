//! Orbit Desktop - Desktop UI for Orbit-RS Database Management
//! 
//! This is a Tauri-based desktop application that provides a UI similar to RedisInsights
//! for managing Orbit-RS databases, running PostgreSQL queries, OrbitQL queries, and Redis commands.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;
use chrono;
mod connections;
mod queries;
mod models;
mod storage;
mod encryption;

use connections::{ConnectionManager, Connection, ConnectionInfo, ConnectionStatus, ConnectionType};
use queries::{QueryExecutor, QueryResult, QueryRequest};
use models::{ModelManager, ModelInfo, MLFunctionInfo};
use storage::{StorageManager, AppStorage};
use encryption::EncryptionManager;

/// Application state
struct AppState {
    connections: RwLock<ConnectionManager>,
    query_executor: RwLock<QueryExecutor>,
    model_manager: RwLock<ModelManager>,
    storage: StorageManager,
    encryption: EncryptionManager,
}

/// Response wrapper for all API calls
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

    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Connection Management Commands

#[tauri::command]
async fn create_connection(
    connection_info: ConnectionInfo,
    state: State<'_, AppState>,
) -> Result<ApiResponse<String>, String> {
    let mut manager = state.connections.write().await;
    let connection_id = match manager.create_connection(connection_info.clone()).await {
        Ok(id) => id,
        Err(e) => return Ok(ApiResponse::error(e.to_string())),
    };
    
    // Save to storage
    let mut storage = state.storage.load()
        .map_err(|e| format!("Failed to load storage: {}", e))?;
    
    if let Some(conn) = manager.get_connection(&connection_id).await {
        let stored_conn = conn.to_stored(&state.encryption)
            .map_err(|e| format!("Failed to encrypt connection: {}", e))?;
        storage.connections.push(stored_conn);
        
        state.storage.save(&storage)
            .map_err(|e| format!("Failed to save storage: {}", e))?;
    }
    
    Ok(ApiResponse::success(connection_id))
}

#[tauri::command]
async fn test_connection(
    connection_info: ConnectionInfo,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ConnectionStatus>, String> {
    let manager = state.connections.read().await;
    match manager.test_connection(&connection_info).await {
        Ok(status) => Ok(ApiResponse::success(status)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

#[tauri::command]
async fn get_connections(
    state: tauri::State<'_, AppState>
) -> Result<ApiResponse<Vec<Connection>>, String> {
    // Load connections from storage
    let storage = state.storage.load()
        .map_err(|e| format!("Failed to load storage: {}", e))?;
    
    let mut connections = Vec::new();
    for stored_conn in &storage.connections {
        match stored_conn.to_connection(&state.encryption) {
            Ok(conn) => connections.push(conn),
            Err(e) => {
                tracing::warn!("Failed to load connection {}: {}", stored_conn.id, e);
            }
        }
    }
    
    // Update connection manager with loaded connections
    {
        let mut manager = state.connections.write().await;
        for conn in &connections {
            // Store connection metadata (without active connection)
            // Active connections will be created on demand
        }
    }
    
    Ok(ApiResponse::success(connections))
}

#[tauri::command]
async fn disconnect(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let mut manager = state.connections.write().await;
    match manager.disconnect(&connection_id).await {
        Ok(_) => Ok(ApiResponse::success(true)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

#[tauri::command]
async fn delete_connection(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let mut manager = state.connections.write().await;
    match manager.delete_connection(&connection_id).await {
        Ok(_) => {},
        Err(e) => return Ok(ApiResponse::error(e.to_string())),
    }
    
    // Remove from storage
    let mut storage = state.storage.load()
        .map_err(|e| format!("Failed to load storage: {}", e))?;
    
    storage.connections.retain(|c| c.id != connection_id);
    
    state.storage.save(&storage)
        .map_err(|e| format!("Failed to save storage: {}", e))?;
    
    Ok(ApiResponse::success(true))
}

/// Query Execution Commands

#[tauri::command]
async fn execute_query(
    request: QueryRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let connection_manager = state.connections.read().await;
    let mut executor = state.query_executor.write().await;
    match executor.execute_query(request, &connection_manager).await {
        Ok(result) => Ok(ApiResponse::success(result)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

#[tauri::command]
async fn get_query_history(
    connection_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<QueryRequest>>, String> {
    let executor = state.query_executor.read().await;
    let history = executor.get_history(&connection_id, limit.unwrap_or(50)).await
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::success(history))
}

#[tauri::command]
async fn explain_query(
    request: QueryRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let connection_manager = state.connections.read().await;
    let mut executor = state.query_executor.write().await;
    match executor.explain_query(request, &connection_manager).await {
        Ok(result) => Ok(ApiResponse::success(result)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

/// ML Model Management Commands

#[tauri::command]
async fn list_ml_functions(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<MLFunctionInfo>>, String> {
    let manager = state.model_manager.read().await;
    let functions = manager.get_ml_functions().await?;
    Ok(ApiResponse::success(functions))
}

#[tauri::command]
async fn list_models(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<ModelInfo>>, String> {
    let manager = state.model_manager.read().await;
    match manager.get_models(&connection_id).await {
        Ok(models) => Ok(ApiResponse::success(models)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

#[tauri::command]
async fn get_model_info(
    connection_id: String,
    model_name: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ModelInfo>, String> {
    let manager = state.model_manager.read().await;
    match manager.get_model_info(&connection_id, &model_name).await {
        Ok(model_info) => Ok(ApiResponse::success(model_info)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

#[tauri::command]
async fn delete_model(
    connection_id: String,
    model_name: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let manager = state.model_manager.read().await;
    match manager.delete_model(&connection_id, &model_name).await {
        Ok(_) => Ok(ApiResponse::success(true)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

/// System Info Commands

#[tauri::command]
async fn get_system_info() -> Result<ApiResponse<HashMap<String, serde_json::Value>>, String> {
    let mut info = HashMap::new();
    
    info.insert("version".to_string(), serde_json::Value::String("0.1.0".to_string()));
    info.insert("os".to_string(), serde_json::Value::String(std::env::consts::OS.to_string()));
    info.insert("arch".to_string(), serde_json::Value::String(std::env::consts::ARCH.to_string()));
    info.insert("timestamp".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
    
    Ok(ApiResponse::success(info))
}

#[tauri::command]
async fn save_settings(
    settings: HashMap<String, serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<ApiResponse<bool>, String> {
    let mut storage = state.storage.load()
        .map_err(|e| format!("Failed to load storage: {}", e))?;
    
    // Update settings from provided values
    if let Some(theme) = settings.get("theme").and_then(|v| v.as_str()) {
        storage.settings.theme = theme.to_string();
    }
    if let Some(auto_save) = settings.get("auto_save").and_then(|v| v.as_bool()) {
        storage.settings.auto_save = auto_save;
    }
    if let Some(timeout) = settings.get("query_timeout").and_then(|v| v.as_u64()) {
        storage.settings.query_timeout = timeout;
    }
    if let Some(font_size) = settings.get("editor_font_size").and_then(|v| v.as_u64()) {
        storage.settings.editor_font_size = font_size as u16;
    }
    if let Some(editor_theme) = settings.get("editor_theme").and_then(|v| v.as_str()) {
        storage.settings.editor_theme = editor_theme.to_string();
    }
    if let Some(show_line_numbers) = settings.get("show_line_numbers").and_then(|v| v.as_bool()) {
        storage.settings.show_line_numbers = show_line_numbers;
    }
    if let Some(word_wrap) = settings.get("word_wrap").and_then(|v| v.as_bool()) {
        storage.settings.word_wrap = word_wrap;
    }
    
    state.storage.save(&storage)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    
    Ok(ApiResponse::success(true))
}

#[tauri::command]
async fn load_settings(
    state: State<'_, AppState>,
) -> Result<ApiResponse<HashMap<String, serde_json::Value>>, String> {
    let storage = state.storage.load()
        .map_err(|e| format!("Failed to load storage: {}", e))?;
    
    let mut settings = HashMap::new();
    settings.insert("theme".to_string(), serde_json::Value::String(storage.settings.theme));
    settings.insert("auto_save".to_string(), serde_json::Value::Bool(storage.settings.auto_save));
    settings.insert("query_timeout".to_string(), serde_json::Value::Number(storage.settings.query_timeout.into()));
    settings.insert("editor_font_size".to_string(), serde_json::Value::Number(storage.settings.editor_font_size.into()));
    settings.insert("editor_theme".to_string(), serde_json::Value::String(storage.settings.editor_theme));
    settings.insert("show_line_numbers".to_string(), serde_json::Value::Bool(storage.settings.show_line_numbers));
    settings.insert("word_wrap".to_string(), serde_json::Value::Bool(storage.settings.word_wrap));
    
    Ok(ApiResponse::success(settings))
}

/// Menu event handlers
#[tauri::command]
async fn show_about_dialog(app: tauri::AppHandle) {
    let window = app.get_window("main").unwrap();
    
    tauri::api::dialog::message(
        Some(&window),
        "About Orbit Desktop",
        "Orbit Desktop v0.1.0\n\nA powerful desktop interface for Orbit-RS database management with support for PostgreSQL, OrbitQL, and Redis commands.\n\nBuilt with ❤️ using Tauri and React."
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Orbit Desktop application");

    let config = tauri::generate_context!().config();
    
    // Initialize storage and encryption
    let storage = StorageManager::new(&config)
        .expect("Failed to initialize storage manager");
    let encryption = EncryptionManager::new(&config)
        .expect("Failed to initialize encryption manager");
    
    // Load connections from storage
    let storage_data = storage.load().unwrap_or_default();
    let mut connection_manager = ConnectionManager::new();
    
    // Load connections into manager
    for stored_conn in &storage_data.connections {
        if let Ok(conn) = stored_conn.to_connection(&encryption) {
            // Connection will be created on-demand when needed
            // For now, just store the metadata
        }
    }
    
    let app_state = AppState {
        connections: RwLock::new(connection_manager),
        query_executor: RwLock::new(QueryExecutor::new()),
        model_manager: RwLock::new(ModelManager::new()),
        storage,
        encryption,
    };
    
    tauri::Builder::default()
        .manage(app_state)
        .menu(create_menu())
        .on_menu_event(|event| {
            match event.menu_item_id() {
                "quit" => {
                    std::process::exit(0);
                }
                "about" => {
                    let app = event.window().app_handle();
                    tauri::async_runtime::spawn(async move {
                        show_about_dialog(app).await;
                    });
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Connection management
            create_connection,
            test_connection,
            get_connections,
            disconnect,
            delete_connection,
            
            // Query execution
            execute_query,
            get_query_history,
            explain_query,
            
            // ML model management
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
        .setup(|app| {
            tracing::info!("Application setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn create_menu() -> tauri::Menu {
    use tauri::{Menu, MenuItem, Submenu, CustomMenuItem};

    let quit = CustomMenuItem::new("quit", "Quit");
    let about = CustomMenuItem::new("about", "About");
    
    let app_menu = Submenu::new("Orbit Desktop", Menu::new()
        .add_item(about)
        .add_native_item(MenuItem::Separator)
        .add_item(quit));

    Menu::new().add_submenu(app_menu)
}

fn main() {
    run();
}