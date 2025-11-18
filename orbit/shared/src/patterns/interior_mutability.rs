//! Interior mutability patterns for safe mutation through shared references
//!
//! Demonstrates Cell, RefCell, and other interior mutability patterns for
//! scenarios where mutation is needed through shared references.

use crate::error::{OrbitError, OrbitResult};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

// ===== Lazy Initialization Pattern =====

/// Lazy initialization using interior mutability
pub struct Lazy<T, F = fn() -> T> {
    cell: RefCell<Option<T>>,
    init: Cell<Option<F>>,
}

impl<T, F> Lazy<T, F>
where
    F: FnOnce() -> T,
{
    pub const fn new(init: F) -> Self {
        Self {
            cell: RefCell::new(None),
            init: Cell::new(Some(init)),
        }
    }

    pub fn get(&self) -> std::cell::Ref<'_, T> {
        if self.cell.borrow().is_none() {
            let init = self.init.take().expect("Lazy already initialized");
            *self.cell.borrow_mut() = Some(init());
        }

        std::cell::Ref::map(self.cell.borrow(), |opt| {
            opt.as_ref().expect("Just initialized")
        })
    }

    pub fn get_mut(&mut self) -> &mut T {
        if self.cell.borrow().is_none() {
            let init = self.init.take().expect("Lazy already initialized");
            *self.cell.borrow_mut() = Some(init());
        }

        self.cell.get_mut().as_mut().expect("Just initialized")
    }

    pub fn is_initialized(&self) -> bool {
        self.cell.borrow().is_some()
    }
}

// ===== Copy-on-Write Cache =====

/// Cache with interior mutability for transparent updates
pub struct Cache<K, V> {
    data: RefCell<HashMap<K, V>>,
    hits: Cell<usize>,
    misses: Cell<usize>,
}

impl<K, V> Cache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            data: RefCell::new(HashMap::new()),
            hits: Cell::new(0),
            misses: Cell::new(0),
        }
    }

    /// Get from cache, computing and storing if missing
    pub fn get_or_insert_with<F>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        // Try to get from cache first
        if let Some(value) = self.data.borrow().get(&key) {
            self.hits.set(self.hits.get() + 1);
            return value.clone();
        }

        // Cache miss - compute and store
        self.misses.set(self.misses.get() + 1);
        let value = f();
        self.data.borrow_mut().insert(key, value.clone());
        value
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let result = self.data.borrow().get(key).cloned();
        if result.is_some() {
            self.hits.set(self.hits.get() + 1);
        } else {
            self.misses.set(self.misses.get() + 1);
        }
        result
    }

    pub fn insert(&self, key: K, value: V) {
        self.data.borrow_mut().insert(key, value);
    }

    pub fn clear(&self) {
        self.data.borrow_mut().clear();
        self.hits.set(0);
        self.misses.set(0);
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.get(),
            misses: self.misses.get(),
            size: self.data.borrow().len(),
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits.get() + self.misses.get();
        if total == 0 {
            0.0
        } else {
            self.hits.get() as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub size: usize,
}

// ===== Atomic Metrics Collector =====

/// Lock-free metrics collection using atomics
#[derive(Default)]
pub struct AtomicMetrics {
    operations: AtomicU64,
    errors: AtomicU64,
    total_duration_ns: AtomicU64,
    active_operations: AtomicUsize,
    circuit_open: AtomicBool,
}

impl AtomicMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record operation start
    pub fn start_operation(&self) {
        self.active_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record operation completion
    pub fn complete_operation(&self, duration: std::time::Duration, success: bool) {
        self.active_operations.fetch_sub(1, Ordering::Relaxed);
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);

        if !success {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_operations: self.operations.load(Ordering::Relaxed),
            total_errors: self.errors.load(Ordering::Relaxed),
            average_duration_ns: {
                let ops = self.operations.load(Ordering::Relaxed);
                if ops == 0 {
                    0
                } else {
                    self.total_duration_ns.load(Ordering::Relaxed) / ops
                }
            },
            active_operations: self.active_operations.load(Ordering::Relaxed),
            circuit_open: self.circuit_open.load(Ordering::Relaxed),
        }
    }

    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        let ops = self.operations.load(Ordering::Relaxed);
        if ops == 0 {
            0.0
        } else {
            self.errors.load(Ordering::Relaxed) as f64 / ops as f64
        }
    }

    /// Open circuit breaker
    pub fn open_circuit(&self) {
        self.circuit_open.store(true, Ordering::Release);
    }

    /// Close circuit breaker
    pub fn close_circuit(&self) {
        self.circuit_open.store(false, Ordering::Release);
    }

    /// Check if circuit is open
    pub fn is_circuit_open(&self) -> bool {
        self.circuit_open.load(Ordering::Acquire)
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.operations.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.total_duration_ns.store(0, Ordering::Relaxed);
        self.active_operations.store(0, Ordering::Relaxed);
        self.circuit_open.store(false, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub total_operations: u64,
    pub total_errors: u64,
    pub average_duration_ns: u64,
    pub active_operations: usize,
    pub circuit_open: bool,
}

// ===== Shared Mutable State with Arc + Mutex =====

/// Configuration that can be updated at runtime through shared references
pub struct SharedConfig {
    inner: Arc<RwLock<ConfigData>>,
}

#[derive(Debug, Clone)]
struct ConfigData {
    max_connections: usize,
    timeout_ms: u64,
    retry_attempts: u32,
    enabled_features: Vec<String>,
}

impl SharedConfig {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ConfigData {
                max_connections: 10,
                timeout_ms: 5000,
                retry_attempts: 3,
                enabled_features: Vec::new(),
            })),
        }
    }

    /// Get configuration value (read lock)
    pub fn max_connections(&self) -> usize {
        self.inner.read().unwrap().max_connections
    }

    pub fn timeout_ms(&self) -> u64 {
        self.inner.read().unwrap().timeout_ms
    }

    pub fn retry_attempts(&self) -> u32 {
        self.inner.read().unwrap().retry_attempts
    }

    /// Update configuration (write lock)
    pub fn set_max_connections(&self, value: usize) {
        self.inner.write().unwrap().max_connections = value;
    }

    pub fn set_timeout_ms(&self, value: u64) {
        self.inner.write().unwrap().timeout_ms = value;
    }

    pub fn set_retry_attempts(&self, value: u32) {
        self.inner.write().unwrap().retry_attempts = value;
    }

    /// Check if feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.inner
            .read()
            .unwrap()
            .enabled_features
            .contains(&feature.to_string())
    }

    /// Enable feature
    pub fn enable_feature(&self, feature: String) {
        let mut config = self.inner.write().unwrap();
        if !config.enabled_features.contains(&feature) {
            config.enabled_features.push(feature);
        }
    }

    /// Disable feature
    pub fn disable_feature(&self, feature: &str) {
        let mut config = self.inner.write().unwrap();
        config.enabled_features.retain(|f| f != feature);
    }

    /// Clone for sharing across threads
    pub fn clone_shared(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// ===== Observer Pattern with Interior Mutability =====

pub trait Observer: Send + Sync {
    fn on_event(&self, event: &str);
}

pub struct Observable {
    observers: RwLock<Vec<Arc<dyn Observer>>>,
}

impl Observable {
    pub fn new() -> Self {
        Self {
            observers: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, observer: Arc<dyn Observer>) {
        self.observers.write().unwrap().push(observer);
    }

    pub fn notify(&self, event: &str) {
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.on_event(event);
        }
    }

    pub fn observer_count(&self) -> usize {
        self.observers.read().unwrap().len()
    }
}

// Simple observer implementation for testing
pub struct EventLogger {
    events: Mutex<Vec<String>>,
}

impl EventLogger {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl Observer for EventLogger {
    fn on_event(&self, event: &str) {
        self.events.lock().unwrap().push(event.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_initialization() {
        let mut call_count = 0;
        let lazy = Lazy::new(|| {
            call_count += 1;
            42
        });

        assert!(!lazy.is_initialized());
        assert_eq!(*lazy.get(), 42);
        assert!(lazy.is_initialized());
        assert_eq!(*lazy.get(), 42); // Should not call init again
    }

    #[test]
    fn test_cache() {
        let cache = Cache::new();

        // First access - miss
        let value = cache.get_or_insert_with("key1", || "value1".to_string());
        assert_eq!(value, "value1");

        // Second access - hit
        let value = cache.get_or_insert_with("key1", || "different".to_string());
        assert_eq!(value, "value1"); // Should return cached value

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.size, 1);
        assert_eq!(cache.hit_rate(), 0.5);
    }

    #[test]
    fn test_atomic_metrics() {
        let metrics = AtomicMetrics::new();

        metrics.start_operation();
        metrics.complete_operation(std::time::Duration::from_millis(10), true);

        metrics.start_operation();
        metrics.complete_operation(std::time::Duration::from_millis(20), false);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_operations, 2);
        assert_eq!(snapshot.total_errors, 1);
        assert_eq!(metrics.error_rate(), 0.5);
        assert!(!metrics.is_circuit_open());

        metrics.open_circuit();
        assert!(metrics.is_circuit_open());
    }

    #[test]
    fn test_shared_config() {
        let config = SharedConfig::new();

        assert_eq!(config.max_connections(), 10);
        config.set_max_connections(20);
        assert_eq!(config.max_connections(), 20);

        config.enable_feature("feature1".to_string());
        assert!(config.is_feature_enabled("feature1"));

        config.disable_feature("feature1");
        assert!(!config.is_feature_enabled("feature1"));
    }

    #[test]
    fn test_observable() {
        let observable = Observable::new();
        let logger = Arc::new(EventLogger::new());

        observable.subscribe(logger.clone());
        observable.notify("event1");
        observable.notify("event2");

        let events = logger.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "event1");
        assert_eq!(events[1], "event2");
    }
}
