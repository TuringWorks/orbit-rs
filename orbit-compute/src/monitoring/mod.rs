//! System monitoring and resource tracking
//!
//! This module provides platform-specific system monitoring capabilities for
//! tracking CPU, GPU, memory, thermal, and power state across different operating systems.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

use std::sync::{Arc, RwLock};

use crate::errors::ComputeError;
use crate::scheduler::{PowerState, SystemConditions, ThermalStatus};

/// Universal system monitor that abstracts platform-specific implementations
#[derive(Debug)]
pub enum SystemMonitor {
    /// macOS system monitor
    #[cfg(target_os = "macos")]
    MacOS(macos::MacOSSystemMonitor),

    /// Windows system monitor
    #[cfg(target_os = "windows")]
    Windows(windows::WindowsSystemMonitor),

    /// Linux system monitor
    #[cfg(target_os = "linux")]
    Linux(Box<linux::LinuxSystemMonitor>),

    /// Android system monitor
    #[cfg(target_os = "android")]
    Android(android::AndroidSystemMonitor),

    /// iOS system monitor
    #[cfg(target_os = "ios")]
    IOS(ios::IOSSystemMonitor),

    /// Mock monitor for testing/unsupported platforms
    Mock(MockSystemMonitor),
}

/// Mock system monitor configuration
#[derive(Debug)]
pub struct MockSystemMonitorState {
    #[allow(dead_code)]
    cpu_load: Arc<RwLock<f32>>,
    #[allow(dead_code)]
    memory_usage: Arc<RwLock<f32>>,
    #[allow(dead_code)]
    thermal_status: Arc<RwLock<ThermalStatus>>,
    #[allow(dead_code)]
    power_state: Arc<RwLock<PowerState>>,
}

impl SystemMonitor {
    /// Create a new system monitor for the current platform
    pub async fn new() -> Result<Self, ComputeError> {
        #[cfg(target_os = "macos")]
        {
            let monitor = macos::MacOSSystemMonitor::new().await?;
            Ok(SystemMonitor::MacOS(monitor))
        }

        #[cfg(target_os = "windows")]
        {
            let monitor = windows::WindowsSystemMonitor::new().await?;
            Ok(SystemMonitor::Windows(monitor))
        }

        #[cfg(target_os = "linux")]
        {
            let monitor = linux::LinuxSystemMonitor::new().await?;
            Ok(SystemMonitor::Linux(Box::new(monitor)))
        }

        #[cfg(target_os = "android")]
        {
            let monitor = android::AndroidSystemMonitor::new().await?;
            Ok(SystemMonitor::Android(monitor))
        }

        #[cfg(target_os = "ios")]
        {
            let monitor = ios::IOSSystemMonitor::new().await?;
            Ok(SystemMonitor::IOS(monitor))
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "android",
            target_os = "ios"
        )))]
        {
            let monitor = MockSystemMonitor::new().await?;
            Ok(SystemMonitor::Mock(monitor))
        }
    }

    /// Start background monitoring
    pub async fn start_monitoring(&mut self) -> Result<(), ComputeError> {
        match self {
            #[cfg(target_os = "macos")]
            SystemMonitor::MacOS(monitor) => monitor.start_monitoring().await,

            #[cfg(target_os = "windows")]
            SystemMonitor::Windows(monitor) => monitor.start_monitoring().await,

            #[cfg(target_os = "linux")]
            SystemMonitor::Linux(monitor) => monitor.start_monitoring().await,

            #[cfg(target_os = "android")]
            SystemMonitor::Android(monitor) => monitor.start_monitoring().await,

            #[cfg(target_os = "ios")]
            SystemMonitor::IOS(monitor) => monitor.start_monitoring().await,

            SystemMonitor::Mock(monitor) => monitor.start_monitoring().await,
        }
    }

    /// Get current system conditions
    pub async fn get_current_conditions(&self) -> Result<SystemConditions, ComputeError> {
        match self {
            #[cfg(target_os = "macos")]
            SystemMonitor::MacOS(monitor) => monitor.get_current_conditions().await,

            #[cfg(target_os = "windows")]
            SystemMonitor::Windows(monitor) => monitor.get_current_conditions().await,

            #[cfg(target_os = "linux")]
            SystemMonitor::Linux(monitor) => monitor.get_current_conditions().await,

            #[cfg(target_os = "android")]
            SystemMonitor::Android(monitor) => monitor.get_current_conditions().await,

            #[cfg(target_os = "ios")]
            SystemMonitor::IOS(monitor) => monitor.get_current_conditions().await,

            SystemMonitor::Mock(monitor) => monitor.get_current_conditions().await,
        }
    }

    /// Get platform-specific system information
    pub fn get_system_info(&self) -> SystemInfo {
        match self {
            #[cfg(target_os = "macos")]
            SystemMonitor::MacOS(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::MacOS(Box::new(info))
            }

            #[cfg(target_os = "windows")]
            SystemMonitor::Windows(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::Windows(info)
            }

            #[cfg(target_os = "linux")]
            SystemMonitor::Linux(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::Linux(Box::new(info))
            }

            #[cfg(target_os = "android")]
            SystemMonitor::Android(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::Android(info)
            }

            #[cfg(target_os = "ios")]
            SystemMonitor::IOS(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::IOS(info)
            }

            SystemMonitor::Mock(monitor) => {
                let info = monitor.get_system_info();
                SystemInfo::Mock(info)
            }
        }
    }
}

/// Platform-specific system information
#[derive(Debug, Clone)]
pub enum SystemInfo {
    /// macOS system information
    #[cfg(target_os = "macos")]
    MacOS(Box<macos::MacOSSystemInfo>),

    /// Windows system information
    #[cfg(target_os = "windows")]
    Windows(windows::WindowsSystemInfo),

    /// Linux system information
    #[cfg(target_os = "linux")]
    Linux(Box<linux::LinuxSystemInfo>),

    /// Android system information
    #[cfg(target_os = "android")]
    Android(android::AndroidSystemInfo),

    /// iOS system information
    #[cfg(target_os = "ios")]
    IOS(ios::IOSSystemInfo),

    /// Mock system information
    Mock(MockSystemInfo),
}

/// Mock system information for testing
#[derive(Debug, Clone)]
pub struct MockSystemInfo {
    pub platform: String,
    pub cpu_cores: u8,
    pub memory_gb: f32,
}

/// Mock system monitor for testing
#[derive(Debug, Clone)]
pub struct MockSystemMonitor;

impl MockSystemMonitor {
    /// Create a new mock system monitor
    pub async fn new() -> Result<Self, ComputeError> {
        Ok(MockSystemMonitor)
    }

    /// Start mock monitoring
    pub async fn start_monitoring(&mut self) -> Result<(), ComputeError> {
        tracing::info!("Mock system monitoring started");
        Ok(())
    }

    /// Get mock system conditions
    pub async fn get_current_conditions(&self) -> Result<SystemConditions, ComputeError> {
        use crate::scheduler::PowerState;

        Ok(SystemConditions {
            cpu_temperature_c: Some(55.0),
            gpu_temperature_c: Some(60.0),
            cpu_utilization: 25.0,
            gpu_utilization: 15.0,
            memory_utilization: 60.0,
            power_state: PowerState::Balanced,
            thermal_throttling: false,
            concurrent_workloads: 2,
        })
    }

    /// Get mock system information
    pub fn get_system_info(&self) -> MockSystemInfo {
        MockSystemInfo {
            platform: "Mock".to_string(),
            cpu_cores: 8,
            memory_gb: 16.0,
        }
    }
}

// Re-export commonly used types from scheduler
pub use crate::scheduler::ThermalState;

#[cfg(target_os = "macos")]
pub use macos::{CoreInfo, GPUInfo, MacOSSystemInfo, MacOSSystemMonitor, ThermalSensor};

#[cfg(target_os = "windows")]
pub use windows::{
    CacheInfo, MemoryInfo, MotherboardInfo, PowerCapabilities, ProcessorFeatures, ProcessorInfo,
    ThermalZone, WindowsGPUInfo, WindowsHardwareInfo, WindowsSystemInfo, WindowsSystemMonitor,
    WindowsVersion,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitor_creation() {
        let monitor = SystemMonitor::new().await;
        assert!(monitor.is_ok());
    }

    #[tokio::test]
    async fn test_mock_monitor() {
        let monitor = MockSystemMonitor::new().await.unwrap();
        let conditions = monitor.get_current_conditions().await;
        assert!(conditions.is_ok());

        let cond = conditions.unwrap();
        assert!(cond.cpu_utilization >= 0.0);
        assert!(cond.memory_utilization >= 0.0);
    }
}
