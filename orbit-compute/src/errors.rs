//! Error handling for the orbit-compute crate
//!
//! This module provides comprehensive error types and handling for all compute
//! operations including hardware detection, scheduling, and execution.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Main error type for compute operations
#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    /// Hardware capability detection errors
    #[error("Capability detection failed")]
    CapabilityDetection {
        source: CapabilityDetectionError,
        context: String,
    },

    /// Scheduling and workload management errors
    #[error("Scheduling error")]
    Scheduling {
        source: SchedulingError,
        workload_type: Option<String>,
    },

    /// Execution engine errors
    #[error("Execution error")]
    Execution {
        source: ExecutionError,
        compute_unit: Option<String>,
    },

    /// System monitoring and resource errors
    #[error("System error")]
    System {
        source: SystemError,
        resource: Option<String>,
    },

    /// Performance database and persistence errors
    #[error("Database error")]
    Database {
        source: DatabaseError,
        operation: Option<String>,
    },

    /// GPU-specific errors (Metal, CUDA, ROCm, etc.)
    #[error("GPU error")]
    GPU {
        source: GPUError,
        api: Option<String>,
        device_id: Option<usize>,
    },

    /// Neural Engine and AI accelerator errors
    #[error("Neural Engine error")]
    NeuralEngine {
        source: NeuralEngineError,
        engine_type: Option<String>,
    },

    /// Memory and resource allocation errors
    #[error("Memory error")]
    Memory {
        source: MemoryError,
        requested_size: Option<usize>,
    },

    /// Configuration and validation errors
    #[error("Configuration error")]
    Configuration {
        source: ConfigurationError,
        parameter: Option<String>,
    },

    /// Platform-specific errors
    #[error("Platform error")]
    Platform {
        source: PlatformError,
        platform: String,
    },

    /// Generic I/O errors
    #[error("I/O error: {source}")]
    IO {
        source: std::io::Error,
        context: String,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {message}")]
    Serialization { message: String },

    /// Timeout and deadline errors
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String, timeout_ms: u64 },

    /// Resource exhaustion errors
    #[error("Resource exhausted: {resource_type}")]
    ResourceExhausted {
        resource_type: String,
        details: String,
    },

    /// Unsupported operation or feature
    #[error("Unsupported operation: {operation}")]
    Unsupported { operation: String, reason: String },

    /// Critical system errors requiring immediate attention
    #[error("Critical error: {message}")]
    Critical {
        error_type: CriticalErrorType,
        message: String,
    },
}

/// Hardware capability detection specific errors
#[derive(Debug, Clone)]
pub enum CapabilityDetectionError {
    /// Failed to detect CPU features
    CPUDetectionFailed { reason: String },

    /// Failed to detect GPU capabilities
    GPUDetectionFailed { vendor: Option<String> },

    /// Failed to detect neural engine capabilities
    NeuralEngineDetectionFailed { platform: String },

    /// System information access denied
    AccessDenied { resource: String },

    /// Incompatible hardware architecture
    IncompatibleArchitecture { detected: String, expected: String },

    /// Missing required system APIs
    MissingSystemAPI { api_name: String },

    /// Hardware driver issues
    DriverError { component: String, error: String },

    /// Insufficient permissions for hardware access
    InsufficientPermissions { required: String },
}

/// Scheduling system errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedulingError {
    /// No compute units available for the requested workload
    NoAvailableComputeUnits,

    /// Workload type not supported
    UnsupportedWorkloadType { workload_type: String },

    /// Quality of Service requirements cannot be met
    QoSRequirementsNotMet { constraint: String },

    /// Deadline cannot be satisfied
    DeadlineMissed { deadline_ms: u64, estimated_ms: u64 },

    /// Conflicting scheduling constraints
    ConflictingConstraints {
        constraint1: String,
        constraint2: String,
    },

    /// Insufficient performance data for decision making
    InsufficientPerformanceData { workload_type: String },

    /// Resource allocation failed
    ResourceAllocationFailed { resource: String },

    /// Scheduler overloaded
    SchedulerOverloaded {
        queue_depth: usize,
        max_depth: usize,
    },

    /// Invalid scheduling request
    InvalidRequest { reason: String },

    /// Performance regression detected
    PerformanceRegression { expected: f64, actual: f64 },
}

/// Execution engine errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionError {
    /// Compute kernel compilation failed
    KernelCompilationFailed { kernel: String, error: String },

    /// Runtime execution error
    RuntimeError { stage: String, error: String },

    /// Data transfer error (CPU ↔ GPU, etc.)
    DataTransferError { source: String, destination: String },

    /// Invalid kernel parameters
    InvalidKernelParameters { parameter: String, value: String },

    /// Compute unit not responsive
    ComputeUnitUnresponsive { unit_type: String, timeout_ms: u64 },

    /// Memory allocation on compute unit failed
    MemoryAllocationFailed {
        unit_type: String,
        size_bytes: usize,
    },

    /// Synchronization error between compute units
    SynchronizationError { units: Vec<String> },

    /// Execution timed out
    ExecutionTimeout { expected_ms: u64, actual_ms: u64 },

    /// Result validation failed
    ValidationFailed { expected: String, actual: String },

    /// Precision or numerical accuracy error
    NumericalError { error_type: NumericalErrorType },
}

/// System monitoring and resource errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemError {
    MonitoringError {
        reason: String,
    },
    /// Failed to read system sensors
    SensorReadFailed {
        sensor_type: String,
    },

    /// Thermal monitoring error
    ThermalMonitoringFailed {
        reason: String,
    },

    /// Power management error
    PowerManagementError {
        operation: String,
    },

    /// System load monitoring failed
    LoadMonitoringFailed {
        component: String,
    },

    /// Resource monitoring access denied
    MonitoringAccessDenied {
        resource: String,
    },

    /// System API call failed
    SystemAPIFailed {
        api: String,
        error_code: i32,
    },

    /// Insufficient system resources
    InsufficientSystemResources {
        resource: String,
        available: String,
        required: String,
    },

    /// System in emergency thermal state
    ThermalEmergency {
        temperature_c: f32,
    },

    /// System power state incompatible with operation
    IncompatiblePowerState {
        current: String,
        required: String,
    },

    /// System security restrictions
    SecurityRestrictions {
        restriction: String,
    },
}

/// Performance database errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseError {
    /// Failed to load performance database
    LoadFailed { path: String, reason: String },

    /// Failed to save performance database
    SaveFailed { path: String, reason: String },

    /// Database corruption detected
    CorruptionDetected { details: String },

    /// Database version mismatch
    VersionMismatch { expected: String, found: String },

    /// Failed to migrate database schema
    MigrationFailed {
        from_version: String,
        to_version: String,
    },

    /// Database lock contention
    LockContentionError { timeout_ms: u64 },

    /// Invalid database query
    InvalidQuery { query: String },

    /// Database integrity check failed
    IntegrityCheckFailed { details: String },

    /// Performance sample validation failed
    SampleValidationFailed { reason: String },

    /// Learning algorithm error
    LearningError { algorithm: String, error: String },
}

/// GPU-specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GPUError {
    /// GPU device not found
    DeviceNotFound { device_id: usize },

    /// GPU driver error
    DriverError {
        driver_version: Option<String>,
        error: String,
    },

    /// GPU memory allocation failed
    MemoryAllocationFailed {
        requested_bytes: usize,
        available_bytes: usize,
    },

    /// GPU compute API initialization failed
    APIInitializationFailed { api: String, error: String },

    /// GPU kernel launch failed
    KernelLaunchFailed { kernel_name: String, error: String },

    /// GPU synchronization timeout
    SynchronizationTimeout { operation: String, timeout_ms: u64 },

    /// GPU thermal throttling detected
    ThermalThrottling { temperature_c: f32 },

    /// GPU compute capability insufficient
    InsufficientComputeCapability { required: String, available: String },

    /// GPU memory bandwidth insufficient
    InsufficientMemoryBandwidth {
        required_gbps: f32,
        available_gbps: f32,
    },

    /// GPU context creation failed
    ContextCreationFailed { api: String, reason: String },

    /// GPU shader compilation failed
    ShaderCompilationFailed { shader_type: String, error: String },

    /// GPU command buffer execution failed
    CommandBufferFailed { error: String },
}

/// Neural Engine specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NeuralEngineError {
    /// Neural Engine not available on this platform
    NotAvailable { platform: String },

    /// Model compilation failed
    ModelCompilationFailed { model_type: String, error: String },

    /// Inference execution failed
    InferenceFailed {
        model: String,
        input_shape: Vec<usize>,
    },

    /// Unsupported model architecture
    UnsupportedModelArchitecture { architecture: String },

    /// Unsupported precision for Neural Engine
    UnsupportedPrecision {
        precision: String,
        supported: Vec<String>,
    },

    /// Neural Engine thermal limiting
    ThermalLimiting { current_tops: f32, max_tops: f32 },

    /// Model quantization failed
    QuantizationFailed {
        from_precision: String,
        to_precision: String,
    },

    /// Neural Engine driver communication error
    DriverCommunicationError { error: String },

    /// Model size exceeds Neural Engine capacity
    ModelTooLarge {
        model_size_mb: f32,
        max_size_mb: f32,
    },

    /// Input data validation failed
    InputValidationFailed {
        expected_shape: Vec<usize>,
        actual_shape: Vec<usize>,
    },
}

/// Memory management errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryError {
    /// Out of memory
    OutOfMemory { requested: usize, available: usize },

    /// Memory alignment requirements not met
    AlignmentError {
        required_alignment: usize,
        actual_alignment: usize,
    },

    /// Invalid memory access
    InvalidAccess { address: usize, operation: String },

    /// Memory fragmentation prevents allocation
    Fragmentation {
        largest_free_block: usize,
        requested: usize,
    },

    /// Unified memory allocation failed (Apple Silicon)
    UnifiedMemoryAllocationFailed { requested_bytes: usize },

    /// Memory mapping failed
    MappingFailed { size: usize, reason: String },

    /// Memory protection error
    ProtectionError { operation: String },

    /// Memory bandwidth insufficient for operation
    BandwidthInsufficient {
        required_gbps: f32,
        available_gbps: f32,
    },

    /// Memory leak detected
    MemoryLeak {
        leaked_bytes: usize,
        allocation_count: usize,
    },

    /// Memory corruption detected
    CorruptionDetected { address_range: (usize, usize) },
}

/// Configuration and validation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationError {
    /// Invalid parameter value
    InvalidParameter {
        parameter: String,
        value: String,
        expected: String,
    },

    /// Missing required configuration
    MissingConfiguration { parameter: String },

    /// Configuration validation failed
    ValidationFailed { errors: Vec<String> },

    /// Conflicting configuration options
    ConflictingOptions { option1: String, option2: String },

    /// Configuration file parsing error
    ParseError {
        file: String,
        line: Option<usize>,
        error: String,
    },

    /// Environment variable error
    EnvironmentVariableError { variable: String, error: String },

    /// Feature flag configuration error
    FeatureFlagError { flag: String, state: String },

    /// Runtime configuration change not allowed
    RuntimeChangeNotAllowed { parameter: String },

    /// Configuration schema version unsupported
    UnsupportedSchemaVersion {
        version: String,
        supported: Vec<String>,
    },

    /// Configuration dependency not met
    DependencyNotMet {
        dependency: String,
        required_by: String,
    },
}

/// Platform-specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlatformError {
    /// macOS specific errors
    MacOS { error: MacOSError },

    /// iOS specific errors
    IOS { error: IOSError },

    /// Windows specific errors
    Windows { error: WindowsError },

    /// Linux specific errors
    Linux { error: LinuxError },

    /// Android specific errors
    Android { error: AndroidError },

    /// Unsupported platform
    UnsupportedPlatform { platform: String, operation: String },

    /// Platform API version incompatible
    IncompatibleAPIVersion {
        api: String,
        required: String,
        available: String,
    },

    /// Platform permissions insufficient
    InsufficientPermissions {
        permission: String,
        platform: String,
    },

    /// Platform-specific hardware not available
    HardwareNotAvailable { hardware: String, platform: String },

    /// Platform system call failed
    SystemCallFailed {
        call: String,
        error_code: i32,
        platform: String,
    },
}

/// macOS specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MacOSError {
    /// Core Foundation error
    CoreFoundation { function: String, result: i32 },

    /// IOKit error
    IOKit { service: String, result: i32 },

    /// Metal framework error
    Metal { operation: String, error: String },

    /// Core ML framework error
    CoreML { model: String, error: String },

    /// System configuration error
    SystemConfiguration { key: String, error: String },

    /// Thermal monitoring API error
    ThermalAPI { sensor: String, error: String },

    /// Power management API error
    PowerAPI { operation: String, result: i32 },

    /// Security framework restriction
    Security {
        operation: String,
        restriction: String,
    },

    /// Entitlements insufficient
    InsufficientEntitlements {
        entitlement: String,
        operation: String,
    },

    /// Sandbox restriction
    SandboxRestriction { resource: String, operation: String },
}

/// iOS specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IOSError {
    /// iOS API unavailable
    APIUnavailable { api: String, ios_version: String },

    /// App store restrictions
    AppStoreRestriction { feature: String },

    /// iOS system resource limit
    SystemResourceLimit { resource: String, limit: String },

    /// Neural Engine access restricted
    NeuralEngineRestricted { reason: String },

    /// Background execution limited
    BackgroundExecutionLimited { operation: String },
}

/// Windows specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowsError {
    /// Win32 API error
    Win32API { function: String, error_code: u32 },

    /// DirectX error
    DirectX { component: String, hresult: u32 },

    /// Windows ML error
    WindowsML { operation: String, error: String },

    /// WMI query error
    WMI { query: String, error: String },

    /// Registry access error
    Registry { key: String, error: String },

    /// Windows driver error
    Driver { driver: String, error: String },
}

/// Linux specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinuxError {
    /// sysfs access error
    SysFS { path: String, error: String },

    /// procfs access error
    ProcFS { path: String, error: String },

    /// Device tree access error
    DeviceTree { node: String, error: String },

    /// OpenCL platform error
    OpenCL { platform: String, error: String },

    /// ROCm error
    ROCm { component: String, error: String },

    /// CUDA driver error
    CUDA { version: String, error: String },

    /// Linux kernel module error
    KernelModule { module: String, error: String },
}

/// Android specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AndroidError {
    /// NNAPI error
    NNAPI { operation: String, error_code: i32 },

    /// Android system service error
    SystemService { service: String, error: String },

    /// Vulkan API error
    Vulkan { operation: String, result: i32 },

    /// Android NDK error
    NDK { function: String, error: String },

    /// Qualcomm Snapdragon specific error
    Snapdragon { component: String, error: String },

    /// Android permissions error
    Permissions { permission: String, error: String },
}

/// Critical error types requiring immediate attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CriticalErrorType {
    /// Thermal emergency shutdown imminent
    ThermalEmergency,

    /// Memory corruption detected
    MemoryCorruption,

    /// Hardware failure detected
    HardwareFailure,

    /// System integrity compromised
    SystemIntegrity,

    /// Security violation detected
    SecurityViolation,

    /// Data loss imminent
    DataLoss,

    /// Resource exhaustion critical
    ResourceExhaustion,

    /// Unrecoverable error state
    UnrecoverableState,
}

/// Numerical computation error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NumericalErrorType {
    /// Overflow in computation
    Overflow { operation: String },

    /// Underflow in computation
    Underflow { operation: String },

    /// Division by zero
    DivisionByZero { operation: String },

    /// Invalid floating point result (NaN, Inf)
    InvalidFloatingPoint { value: String, operation: String },

    /// Precision loss detected
    PrecisionLoss {
        expected_precision: String,
        actual_precision: String,
    },

    /// Matrix operation error
    MatrixError {
        operation: String,
        dimensions: String,
    },

    /// Numerical instability
    NumericalInstability { condition_number: f64 },

    /// Convergence failure
    ConvergenceFailure {
        algorithm: String,
        iterations: usize,
    },
}

// Conversion from std::io::Error
impl From<std::io::Error> for ComputeError {
    fn from(err: std::io::Error) -> Self {
        ComputeError::IO {
            source: err,
            context: "Unknown".to_string(),
        }
    }
}

/// Conversion from serde errors
impl From<serde_json::Error> for ComputeError {
    fn from(err: serde_json::Error) -> Self {
        ComputeError::Serialization {
            message: err.to_string(),
        }
    }
}

// Implement Display for sub-error types
macro_rules! impl_display {
    ($($type:ty),+ $(,)?) => {
        $(
            impl fmt::Display for $type {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{:?}", self)
                }
            }

            impl std::error::Error for $type {}
        )+
    };
}

impl_display!(
    CapabilityDetectionError,
    SchedulingError,
    ExecutionError,
    SystemError,
    DatabaseError,
    GPUError,
    NeuralEngineError,
    MemoryError,
    ConfigurationError,
    PlatformError,
    MacOSError,
    IOSError,
    WindowsError,
    LinuxError,
    AndroidError,
    CriticalErrorType,
    NumericalErrorType,
);

// ComputeError Display is handled by thiserror

// Helper functions for common error creation patterns
impl ComputeError {
    /// Create a capability detection error
    pub fn capability_detection(source: CapabilityDetectionError, context: &str) -> Self {
        ComputeError::CapabilityDetection {
            source,
            context: context.to_string(),
        }
    }

    /// Create a scheduling error
    pub fn scheduling(source: SchedulingError) -> Self {
        ComputeError::Scheduling {
            source,
            workload_type: None,
        }
    }

    /// Create a scheduling error with workload type context
    pub fn scheduling_with_workload(source: SchedulingError, workload_type: &str) -> Self {
        ComputeError::Scheduling {
            source,
            workload_type: Some(workload_type.to_string()),
        }
    }

    /// Create an execution error
    pub fn execution(source: ExecutionError) -> Self {
        ComputeError::Execution {
            source,
            compute_unit: None,
        }
    }

    /// Create an execution error with compute unit context
    pub fn execution_with_unit(source: ExecutionError, compute_unit: &str) -> Self {
        ComputeError::Execution {
            source,
            compute_unit: Some(compute_unit.to_string()),
        }
    }

    /// Create a GPU error
    pub fn gpu(source: GPUError) -> Self {
        ComputeError::GPU {
            source,
            api: None,
            device_id: None,
        }
    }

    /// Create a GPU error with context
    pub fn gpu_with_context(source: GPUError, api: &str, device_id: usize) -> Self {
        ComputeError::GPU {
            source,
            api: Some(api.to_string()),
            device_id: Some(device_id),
        }
    }

    /// Create a neural engine error
    pub fn neural_engine(source: NeuralEngineError, engine_type: &str) -> Self {
        ComputeError::NeuralEngine {
            source,
            engine_type: Some(engine_type.to_string()),
        }
    }

    /// Create a memory error
    pub fn memory(source: MemoryError, requested_size: Option<usize>) -> Self {
        ComputeError::Memory {
            source,
            requested_size,
        }
    }

    /// Create a critical error
    pub fn critical(error_type: CriticalErrorType, message: &str) -> Self {
        ComputeError::Critical {
            error_type,
            message: message.to_string(),
        }
    }

    /// Create an I/O error with context
    pub fn io(source: std::io::Error, context: &str) -> Self {
        ComputeError::IO {
            source,
            context: context.to_string(),
        }
    }

    /// Check if error is critical and requires immediate attention
    pub fn is_critical(&self) -> bool {
        matches!(self, ComputeError::Critical { .. })
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            ComputeError::Critical { .. } => false,
            ComputeError::System { source, .. } => !matches!(
                source,
                SystemError::ThermalEmergency { .. }
                    | SystemError::InsufficientSystemResources { .. }
            ),
            ComputeError::Memory { source, .. } => !matches!(
                source,
                MemoryError::OutOfMemory { .. } | MemoryError::CorruptionDetected { .. }
            ),
            _ => true,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ComputeError::Critical { .. } => ErrorSeverity::Critical,
            ComputeError::System { source, .. } => match source {
                SystemError::ThermalEmergency { .. } => ErrorSeverity::Critical,
                SystemError::InsufficientSystemResources { .. } => ErrorSeverity::High,
                _ => ErrorSeverity::Medium,
            },
            ComputeError::Memory { source, .. } => match source {
                MemoryError::OutOfMemory { .. } => ErrorSeverity::High,
                MemoryError::CorruptionDetected { .. } => ErrorSeverity::Critical,
                _ => ErrorSeverity::Medium,
            },
            ComputeError::GPU { .. } => ErrorSeverity::Medium,
            ComputeError::NeuralEngine { .. } => ErrorSeverity::Medium,
            ComputeError::Execution { .. } => ErrorSeverity::Medium,
            ComputeError::Scheduling { .. } => ErrorSeverity::Low,
            ComputeError::Configuration { .. } => ErrorSeverity::Low,
            _ => ErrorSeverity::Low,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Low severity - operation can continue with degraded performance
    Low,
    /// Medium severity - operation should be retried or alternative used
    Medium,
    /// High severity - operation should be aborted but system can continue
    High,
    /// Critical severity - immediate attention required, system stability at risk
    Critical,
}

/// Result type alias for compute operations
pub type ComputeResult<T> = Result<T, ComputeError>;

/// Trait for error context enhancement
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context(self, context: &str) -> ComputeResult<T>;

    /// Add context using a closure (lazy evaluation)
    fn with_context_fn<F>(self, f: F) -> ComputeResult<T>
    where
        F: FnOnce() -> String;
}

impl<T> ErrorContext<T> for ComputeResult<T> {
    fn with_context(self, context: &str) -> ComputeResult<T> {
        self.map_err(|mut err| {
            // Add context based on error type
            match &mut err {
                ComputeError::IO {
                    context: ref mut ctx,
                    ..
                } => {
                    *ctx = context.to_string();
                }
                ComputeError::CapabilityDetection {
                    context: ref mut ctx,
                    ..
                } => {
                    *ctx = context.to_string();
                }
                _ => {
                    // For other error types, we could wrap in a generic context error
                }
            }
            err
        })
    }

    fn with_context_fn<F>(self, f: F) -> ComputeResult<T>
    where
        F: FnOnce() -> String,
    {
        self.with_context(&f())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = ComputeError::scheduling(SchedulingError::NoAvailableComputeUnits);
        assert!(!err.is_critical());
        assert_eq!(err.severity(), ErrorSeverity::Low);
    }

    #[test]
    fn test_critical_error() {
        let err = ComputeError::critical(CriticalErrorType::ThermalEmergency, "System overheating");
        assert!(err.is_critical());
        assert!(!err.is_recoverable());
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_display() {
        let err = ComputeError::GPU {
            source: GPUError::DeviceNotFound { device_id: 0 },
            api: Some("Metal".to_string()),
            device_id: Some(0),
        };
        let display = format!("{}", err);
        assert!(display.contains("GPU error"));
    }

    #[test]
    fn test_error_context() {
        let result: ComputeResult<()> = Err(ComputeError::scheduling(
            SchedulingError::NoAvailableComputeUnits,
        ));

        let with_context = result.with_context("matrix multiplication");
        assert!(with_context.is_err());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let compute_error: ComputeError = io_error.into();

        match compute_error {
            ComputeError::IO { .. } => {}
            _ => panic!("Should convert to IO error"),
        }
    }
}
