use crate::{ApplicationContext, SpringError, SpringResult};
use jni::{
    objects::{JClass, JObject, JString, JValue},
    sys::{jboolean, jint, jlong, jobject, jstring},
    JNIEnv, JavaVM,
};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Global application context holder for JNI
static GLOBAL_CONTEXT: OnceLock<Arc<Mutex<Option<Arc<RwLock<ApplicationContext>>>>>> =
    OnceLock::new();

/// Initialize the global context
fn init_global_context() -> Arc<Mutex<Option<Arc<RwLock<ApplicationContext>>>>> {
    Arc::new(Mutex::new(None))
}

/// Get the global context
fn get_global_context() -> Arc<Mutex<Option<Arc<RwLock<ApplicationContext>>>>> {
    GLOBAL_CONTEXT.get_or_init(init_global_context).clone()
}

// ============================================================================
// JNI Exported Functions (C API)
// ============================================================================

/// Initialize the Orbit application context
/// Java signature: public static native long initializeContext(String configPath);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_initializeContext(
    env: JNIEnv,
    _class: JClass,
    config_path: JString,
) -> jlong {
    match initialize_context_impl(env, config_path) {
        Ok(context_id) => context_id as jlong,
        Err(e) => {
            error!("Failed to initialize context: {}", e);
            -1
        }
    }
}

/// Start the application context
/// Java signature: public static native boolean startContext(long contextId);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_startContext(
    _env: JNIEnv,
    _class: JClass,
    context_id: jlong,
) -> jboolean {
    match start_context_impl(context_id) {
        Ok(_) => 1,
        Err(e) => {
            error!("Failed to start context: {}", e);
            0
        }
    }
}

/// Stop the application context
/// Java signature: public static native boolean stopContext(long contextId);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_stopContext(
    _env: JNIEnv,
    _class: JClass,
    context_id: jlong,
) -> jboolean {
    match stop_context_impl(context_id) {
        Ok(_) => 1,
        Err(e) => {
            error!("Failed to stop context: {}", e);
            0
        }
    }
}

/// Get configuration value
/// Java signature: public static native String getConfigValue(long contextId, String key);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_getConfigValue(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
    key: JString,
) -> jstring {
    match get_config_value_impl(env, context_id, key) {
        Ok(value) => value,
        Err(e) => {
            error!("Failed to get config value: {}", e);
            ptr::null_mut()
        }
    }
}

/// Set configuration value
/// Java signature: public static native boolean setConfigValue(long contextId, String key, String value);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_setConfigValue(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
    key: JString,
    value: JString,
) -> jboolean {
    match set_config_value_impl(env, context_id, key, value) {
        Ok(_) => 1,
        Err(e) => {
            error!("Failed to set config value: {}", e);
            0
        }
    }
}

/// Register a service
/// Java signature: public static native boolean registerService(long contextId, String name, String serviceType);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_registerService(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
    name: JString,
    service_type: JString,
) -> jboolean {
    match register_service_impl(env, context_id, name, service_type) {
        Ok(_) => 1,
        Err(e) => {
            error!("Failed to register service: {}", e);
            0
        }
    }
}

/// Unregister a service
/// Java signature: public static native boolean unregisterService(long contextId, String name);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_unregisterService(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
    name: JString,
) -> jboolean {
    match unregister_service_impl(env, context_id, name) {
        Ok(_) => 1,
        Err(e) => {
            error!("Failed to unregister service: {}", e);
            0
        }
    }
}

/// Check if service is registered
/// Java signature: public static native boolean hasService(long contextId, String name);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_hasService(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
    name: JString,
) -> jboolean {
    match has_service_impl(env, context_id, name) {
        Ok(has_service) => {
            if has_service {
                1
            } else {
                0
            }
        }
        Err(e) => {
            error!("Failed to check service: {}", e);
            0
        }
    }
}

/// Get all service names
/// Java signature: public static native String[] getServiceNames(long contextId);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_getServiceNames(
    env: JNIEnv,
    _class: JClass,
    context_id: jlong,
) -> jobject {
    match get_service_names_impl(env, context_id) {
        Ok(array) => array.into_inner(),
        Err(e) => {
            error!("Failed to get service names: {}", e);
            ptr::null_mut()
        }
    }
}

/// Perform health check
/// Java signature: public static native boolean healthCheck(long contextId);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_healthCheck(
    _env: JNIEnv,
    _class: JClass,
    context_id: jlong,
) -> jboolean {
    match health_check_impl(context_id) {
        Ok(is_healthy) => {
            if is_healthy {
                1
            } else {
                0
            }
        }
        Err(e) => {
            error!("Failed to perform health check: {}", e);
            0
        }
    }
}

/// Clean up and destroy the context
/// Java signature: public static native void destroyContext(long contextId);
#[no_mangle]
pub extern "C" fn Java_com_orbit_spring_OrbitClient_destroyContext(
    _env: JNIEnv,
    _class: JClass,
    context_id: jlong,
) {
    if let Err(e) = destroy_context_impl(context_id) {
        error!("Failed to destroy context: {}", e);
    }
}

// ============================================================================
// Implementation Functions
// ============================================================================

fn initialize_context_impl(
    env: JNIEnv,
    config_path: JString,
) -> Result<u64, Box<dyn std::error::Error>> {
    let config_path_str: String = env.get_string(&config_path)?.into();

    info!(
        "Initializing Orbit context with config: {}",
        config_path_str
    );

    // In a real implementation, load configuration from the path
    let context = ApplicationContext::new();
    let context_arc = Arc::new(RwLock::new(context));

    let global_context = get_global_context();
    let mut guard = global_context
        .lock()
        .map_err(|_| "Failed to lock global context")?;
    *guard = Some(context_arc);

    // Return a simple context ID (in production, use proper ID management)
    Ok(1)
}

fn start_context_impl(context_id: jlong) -> SpringResult<()> {
    info!("Starting context with ID: {}", context_id);

    let global_context = get_global_context();
    let guard = global_context
        .lock()
        .map_err(|_| SpringError::context("Failed to lock global context"))?;

    if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| SpringError::context(format!("Failed to create tokio runtime: {}", e)))?;

        rt.block_on(async {
            let context_guard = context.read().await;
            context_guard.start().await
        })
    } else {
        Err(SpringError::context("Context not found"))
    }
}

fn stop_context_impl(context_id: jlong) -> SpringResult<()> {
    info!("Stopping context with ID: {}", context_id);

    let global_context = get_global_context();
    let guard = global_context
        .lock()
        .map_err(|_| SpringError::context("Failed to lock global context"))?;

    if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| SpringError::context(format!("Failed to create tokio runtime: {}", e)))?;

        rt.block_on(async {
            let context_guard = context.read().await;
            context_guard.stop().await
        })
    } else {
        Err(SpringError::context("Context not found"))
    }
}

fn get_config_value_impl(
    env: JNIEnv,
    context_id: jlong,
    key: JString,
) -> Result<jstring, Box<dyn std::error::Error>> {
    let key_str: String = env.get_string(&key)?.into();

    let global_context = get_global_context();
    let guard = global_context.lock()?;

    if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()?;

        let value = rt.block_on(async {
            let context_guard = context.read().await;
            let config = context_guard.config();

            match key_str.as_str() {
                "application.name" => Some(config.application.name.clone()),
                "application.version" => Some(config.application.version.clone()),
                "server.port" => Some(config.server.port.to_string()),
                _ => None,
            }
        });

        match value {
            Some(v) => {
                let result = env.new_string(v)?;
                Ok(result.into_raw())
            }
            None => Ok(ptr::null_mut()),
        }
    } else {
        Ok(ptr::null_mut())
    }
}

fn set_config_value_impl(
    env: JNIEnv,
    context_id: jlong,
    key: JString,
    value: JString,
) -> SpringResult<()> {
    let key_str: String = env
        .get_string(&key)
        .map_err(|e| SpringError::context(format!("JNI error: {}", e)))?
        .into();
    let value_str: String = env
        .get_string(&value)
        .map_err(|e| SpringError::context(format!("JNI error: {}", e)))?
        .into();

    info!("Setting config via JNI: {} = {}", key_str, value_str);

    // In a real implementation, we would update the configuration
    Ok(())
}

fn register_service_impl(
    env: JNIEnv,
    context_id: jlong,
    name: JString,
    service_type: JString,
) -> SpringResult<()> {
    let name_str: String = env
        .get_string(&name)
        .map_err(|e| SpringError::context(format!("JNI error: {}", e)))?
        .into();
    let type_str: String = env
        .get_string(&service_type)
        .map_err(|e| SpringError::context(format!("JNI error: {}", e)))?
        .into();

    info!(
        "Registering service via JNI: {} (type: {})",
        name_str, type_str
    );

    // In a real implementation, we would register the service with the context
    Ok(())
}

fn unregister_service_impl(env: JNIEnv, context_id: jlong, name: JString) -> SpringResult<()> {
    let name_str: String = env
        .get_string(&name)
        .map_err(|e| SpringError::context(format!("JNI error: {}", e)))?
        .into();

    info!("Unregistering service via JNI: {}", name_str);

    // In a real implementation, we would unregister the service from the context
    Ok(())
}

fn has_service_impl(
    env: JNIEnv,
    context_id: jlong,
    name: JString,
) -> Result<bool, Box<dyn std::error::Error>> {
    let name_str: String = env.get_string(&name)?.into();

    let global_context = get_global_context();
    let guard = global_context.lock()?;

    if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()?;

        let has_service = rt.block_on(async {
            let context_guard = context.read().await;
            context_guard.contains_service(&name_str)
        });

        Ok(has_service)
    } else {
        Ok(false)
    }
}

fn get_service_names_impl(
    mut env: JNIEnv,
    _context_id: jlong,
) -> Result<JObject, Box<dyn std::error::Error>> {
    let global_context = get_global_context();
    let guard = global_context.lock()?;

    let service_names = if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let context_guard = context.read().await;
            context_guard.get_service_names()
        })
    } else {
        vec![]
    };
    drop(guard);

    let string_class = env.find_class("java/lang/String")?;
    let array = env.new_object_array(service_names.len() as i32, string_class, JObject::null())?;

    for (i, name) in service_names.iter().enumerate() {
        let java_string = env.new_string(name)?;
        env.set_object_array_element(&array, i as i32, java_string)?;
    }

    Ok(JObject::from(array))
}

fn health_check_impl(_context_id: jlong) -> Result<bool, Box<dyn std::error::Error>> {
    let global_context = get_global_context();
    let guard = global_context.lock()?;

    let result = if let Some(context) = guard.as_ref() {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let context_guard = context.read().await;
            context_guard.health_check().await.healthy
        })
    } else {
        false
    };
    drop(guard);

    Ok(result)
}

fn destroy_context_impl(context_id: jlong) -> SpringResult<()> {
    info!("Destroying context with ID: {}", context_id);

    let global_context = get_global_context();
    let mut guard = global_context
        .lock()
        .map_err(|_| SpringError::context("Failed to lock global context"))?;

    if guard.is_some() {
        *guard = None;
        info!("Context destroyed successfully");
    } else {
        warn!("Context was already destroyed or not found");
    }

    Ok(())
}

// ============================================================================
// Helper Functions for Java Integration
// ============================================================================

/// Convert a Rust Result to Java boolean (for methods that return success/failure)
pub fn result_to_jboolean<T, E>(result: Result<T, E>) -> jboolean {
    match result {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Convert a Rust String to Java String
pub fn string_to_jstring(env: &JNIEnv, s: String) -> Result<jstring, jni::errors::Error> {
    let java_string = env.new_string(s)?;
    Ok(java_string.into_raw())
}

/// Convert a Java String to Rust String
pub fn jstring_to_string(env: &JNIEnv, s: JString) -> Result<String, jni::errors::Error> {
    let java_str = env.get_string(&s)?;
    Ok(java_str.into())
}

/// Create a Java HashMap from a Rust HashMap
pub fn create_java_hashmap<'a>(
    env: &'a JNIEnv<'a>,
    map: HashMap<String, String>,
) -> Result<JObject<'a>, jni::errors::Error> {
    let hashmap_class = env.find_class("java/util/HashMap")?;
    let hashmap = env.new_object(hashmap_class, "()V", &[])?;

    for (key, value) in map {
        let java_key = env.new_string(key)?;
        let java_value = env.new_string(value)?;

        env.call_method(
            hashmap,
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[
                JValue::Object(&java_key.into()),
                JValue::Object(&java_value.into()),
            ],
        )?;
    }

    Ok(hashmap)
}

// ============================================================================
// JNI Lifecycle Management
// ============================================================================

/// Called when the library is loaded by the JVM
#[no_mangle]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut std::ffi::c_void) -> jint {
    info!("🚀 Orbit Spring JNI library loaded");
    jni::sys::JNI_VERSION_1_8
}

/// Called when the library is unloaded by the JVM
#[no_mangle]
pub extern "C" fn JNI_OnUnload(_vm: *mut jni::sys::JavaVM, _reserved: *mut std::ffi::c_void) {
    info!("🛑 Orbit Spring JNI library unloaded");
}

// ============================================================================
// Error Conversion Utilities
// ============================================================================

impl From<jni::errors::Error> for SpringError {
    fn from(error: jni::errors::Error) -> Self {
        SpringError::context(format!("JNI error: {}", error))
    }
}

impl<T> From<std::sync::PoisonError<T>> for SpringError {
    fn from(_error: std::sync::PoisonError<T>) -> Self {
        SpringError::context("Mutex poisoned")
    }
}

// Note: In a production setup, you would also need:
// 1. Proper context ID management (not just using 1)
// 2. Thread-safe context storage with proper cleanup
// 3. Error propagation to Java exceptions
// 4. Memory management for JNI objects
// 5. Proper logging integration
// 6. Configuration validation
// 7. Service lifecycle management
// 8. Health check implementations
