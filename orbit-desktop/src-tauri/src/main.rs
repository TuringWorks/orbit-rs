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

use connections::{ConnectionManager, Connection, ConnectionInfo, ConnectionStatus, ConnectionType};
use queries::{QueryExecutor, QueryResult, QueryRequest};
use models::{ModelManager, ModelInfo, MLFunctionInfo};

/// Application state
#[derive(Default)]
struct AppState {
    connections: RwLock<ConnectionManager>,
    query_executor: RwLock<QueryExecutor>,
    model_manager: RwLock<ModelManager>,
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
    match manager.create_connection(connection_info).await {
        Ok(connection_id) => Ok(ApiResponse::success(connection_id)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
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
    let manager = state.connections.read().await;
    let mut connections = manager.list_connections().await;
    
    // Add some sample connections for demonstration
    if connections.is_empty() {
        connections = vec![
            Connection {
                id: "local-postgres".to_string(),
                info: ConnectionInfo {
                    name: "Local PostgreSQL".to_string(),
                    connection_type: ConnectionType::PostgreSQL,
                    host: "localhost".to_string(),
                    port: 5432,
                    database: Some("orbit_demo".to_string()),
                    username: Some("postgres".to_string()),
                    password: None,
                    ssl_mode: None,
                    connection_timeout: None,
                    additional_params: HashMap::new(),
                },
                status: ConnectionStatus::Disconnected,
                created_at: chrono::Utc::now(),
                last_used: None,
                query_count: 0,
            },
            Connection {
                id: "local-orbitql".to_string(),
                info: ConnectionInfo {
                    name: "Local OrbitQL".to_string(),
                    connection_type: ConnectionType::OrbitQL,
                    host: "localhost".to_string(),
                    port: 8080,
                    database: None,
                    username: None,
                    password: None,
                    ssl_mode: None,
                    connection_timeout: None,
                    additional_params: HashMap::new(),
                },
                status: ConnectionStatus::Connected,
                created_at: chrono::Utc::now(),
                last_used: None,
                query_count: 0,
            },
            Connection {
                id: "local-redis".to_string(),
                info: ConnectionInfo {
                    name: "Local Redis".to_string(),
                    connection_type: ConnectionType::Redis,
                    host: "localhost".to_string(),
                    port: 6379,
                    database: None,
                    username: None,
                    password: None,
                    ssl_mode: None,
                    connection_timeout: None,
                    additional_params: HashMap::new(),
                },
                status: ConnectionStatus::Connected,
                created_at: chrono::Utc::now(),
                last_used: None,
                query_count: 0,
            },
        ];
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
        Ok(_) => Ok(ApiResponse::success(true)),
        Err(e) => Ok(ApiResponse::error(e.to_string())),
    }
}

/// Query Execution Commands

#[tauri::command]
async fn execute_query(
    request: QueryRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let executor = state.query_executor.read().await;
    match executor.execute_query(request).await {
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
    let history = executor.get_history(&connection_id, limit.unwrap_or(50)).await?;
    Ok(ApiResponse::success(history))
}

#[tauri::command]
async fn explain_query(
    request: QueryRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<QueryResult>, String> {
    let executor = state.query_executor.read().await;
    match executor.explain_query(request).await {
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
) -> Result<ApiResponse<bool>, String> {
    // In a real implementation, this would save to a config file
    tracing::info!("Saving settings: {:?}", settings);
    Ok(ApiResponse::success(true))
}

#[tauri::command]
async fn load_settings() -> Result<ApiResponse<HashMap<String, serde_json::Value>>, String> {
    // In a real implementation, this would load from a config file
    let mut settings = HashMap::new();
    settings.insert("theme".to_string(), serde_json::Value::String("dark".to_string()));
    settings.insert("auto_save".to_string(), serde_json::Value::Bool(true));
    settings.insert("query_timeout".to_string(), serde_json::Value::Number(30.into()));
    
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

    tauri::Builder::default()
        .manage(AppState::default())
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