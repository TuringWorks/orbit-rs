//! Linux system monitoring implementation
//!
//! This module provides Linux-specific system monitoring using:
//! - /proc filesystem for system information and performance metrics
//! - /sys filesystem for hardware information and thermal data
//! - Linux-specific APIs for GPU monitoring and power management
//! - Hardware abstraction for different CPU and GPU vendors

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::errors::{ComputeError, LinuxError, PlatformError, SystemError};
use crate::scheduler::{PowerState, SystemConditions};

/// Linux system monitor implementation
#[derive(Debug)]
pub struct LinuxSystemMonitor {
    /// System information cache
    system_info: LinuxSystemInfo,

    /// Current monitoring state
    monitoring_active: Arc<RwLock<bool>>,

    /// Cached system conditions
    cached_conditions: Arc<RwLock<SystemConditions>>,

    /// CPU monitoring interface
    #[allow(dead_code)]
    cpu_monitor: LinuxCPUMonitor,

    /// Memory monitoring interface
    #[allow(dead_code)]
    memory_monitor: LinuxMemoryMonitor,

    /// GPU monitoring interface
    #[allow(dead_code)]
    gpu_monitor: Option<LinuxGPUMonitor>,

    /// Thermal monitoring interface
    #[allow(dead_code)]
    thermal_monitor: LinuxThermalMonitor,

    /// Power monitoring interface
    #[allow(dead_code)]
    power_monitor: LinuxPowerMonitor,
}

/// Linux system information
#[derive(Debug, Clone)]
pub struct LinuxSystemInfo {
    /// Distribution information
    pub distribution: LinuxDistribution,

    /// Kernel information
    pub kernel: KernelInfo,

    /// CPU information
    pub cpu_info: LinuxCPUInfo,

    /// Memory information
    pub memory_info: LinuxMemoryInfo,

    /// GPU information
    pub gpu_info: Vec<LinuxGPUInfo>,

    /// Hardware information
    pub hardware_info: LinuxHardwareInfo,

    /// Power management capabilities
    pub power_capabilities: LinuxPowerCapabilities,
}

/// Linux distribution information
#[derive(Debug, Clone)]
pub struct LinuxDistribution {
    /// Distribution name (Ubuntu, CentOS, etc.)
    pub name: String,

    /// Distribution version
    pub version: String,

    /// Distribution ID (ubuntu, centos, etc.)
    pub id: String,

    /// Version codename (if available)
    pub codename: Option<String>,

    /// Pretty name for display
    pub pretty_name: String,
}

/// Kernel information
#[derive(Debug, Clone)]
pub struct KernelInfo {
    /// Kernel version string
    pub version: String,

    /// Kernel release
    pub release: String,

    /// System architecture
    pub architecture: String,

    /// Hostname
    pub hostname: String,
}

/// Linux CPU information
#[derive(Debug, Clone)]
pub struct LinuxCPUInfo {
    /// CPU model name
    pub model_name: String,

    /// CPU vendor (GenuineIntel, AuthenticAMD, etc.)
    pub vendor: String,

    /// Number of physical sockets
    pub sockets: u8,

    /// Number of physical cores
    pub physical_cores: u8,

    /// Number of logical processors
    pub logical_processors: u8,

    /// CPU frequency (base) in MHz
    pub base_frequency_mhz: u32,

    /// CPU frequency (max) in MHz
    pub max_frequency_mhz: u32,

    /// Cache sizes
    pub cache_info: LinuxCacheInfo,

    /// CPU features/flags
    pub features: Vec<String>,

    /// CPU governors available
    pub governors: Vec<String>,

    /// Current CPU governor
    pub current_governor: String,
}

/// Linux cache information
#[derive(Debug, Clone)]
pub struct LinuxCacheInfo {
    /// L1 data cache size in KB
    pub l1d_size_kb: u32,

    /// L1 instruction cache size in KB
    pub l1i_size_kb: u32,

    /// L2 cache size in KB
    pub l2_size_kb: u32,

    /// L3 cache size in KB
    pub l3_size_kb: u32,
}

/// Linux memory information
#[derive(Debug, Clone)]
pub struct LinuxMemoryInfo {
    /// Total memory in GB
    pub total_gb: f32,

    /// Available memory in GB
    pub available_gb: f32,

    /// Free memory in GB
    pub free_gb: f32,

    /// Buffer memory in GB
    pub buffers_gb: f32,

    /// Cached memory in GB
    pub cached_gb: f32,

    /// Swap total in GB
    pub swap_total_gb: f32,

    /// Swap free in GB
    pub swap_free_gb: f32,

    /// Memory speed in MHz (if available)
    pub speed_mhz: Option<u32>,

    /// Memory type (if available)
    pub memory_type: Option<String>,
}

/// Linux GPU information
#[derive(Debug, Clone)]
pub struct LinuxGPUInfo {
    /// GPU device index
    pub device_index: u32,

    /// GPU name/model
    pub name: String,

    /// GPU vendor (NVIDIA, AMD, Intel)
    pub vendor: String,

    /// PCI device ID
    pub pci_id: String,

    /// Driver in use
    pub driver: String,

    /// Driver version
    pub driver_version: String,

    /// Memory size in MB
    pub memory_mb: u32,

    /// Current GPU frequency in MHz
    pub current_freq_mhz: Option<u32>,

    /// Maximum GPU frequency in MHz
    pub max_freq_mhz: Option<u32>,

    /// Power limit in watts
    pub power_limit_w: Option<u32>,

    /// CUDA support (for NVIDIA)
    pub cuda_supported: bool,

    /// OpenCL support
    pub opencl_supported: bool,

    /// Vulkan support
    pub vulkan_supported: bool,
}

/// Linux hardware information
#[derive(Debug, Clone)]
pub struct LinuxHardwareInfo {
    /// DMI system manufacturer
    pub manufacturer: Option<String>,

    /// DMI system product name
    pub product_name: Option<String>,

    /// DMI system version
    pub version: Option<String>,

    /// DMI system serial number
    pub serial_number: Option<String>,

    /// BIOS information
    pub bios_info: Option<BiosInfo>,

    /// Motherboard information
    pub board_info: Option<BoardInfo>,

    /// Available thermal zones
    pub thermal_zones: Vec<ThermalZone>,

    /// Available power supplies
    pub power_supplies: Vec<PowerSupply>,
}

/// BIOS information
#[derive(Debug, Clone)]
pub struct BiosInfo {
    /// BIOS vendor
    pub vendor: String,

    /// BIOS version
    pub version: String,

    /// BIOS date
    pub date: String,
}

/// Motherboard information
#[derive(Debug, Clone)]
pub struct BoardInfo {
    /// Board manufacturer
    pub manufacturer: String,

    /// Board product name
    pub product_name: String,

    /// Board version
    pub version: String,
}

/// Thermal zone information
#[derive(Debug, Clone)]
pub struct ThermalZone {
    /// Zone type (cpu, gpu, acpi, etc.)
    pub zone_type: String,

    /// Current temperature in millidegrees Celsius
    pub temp_millidegrees: Option<i32>,

    /// Temperature in Celsius
    pub temp_celsius: Option<f32>,

    /// Critical temperature
    pub crit_temp_celsius: Option<f32>,

    /// Passive temperature
    pub passive_temp_celsius: Option<f32>,
}

/// Power supply information
#[derive(Debug, Clone)]
pub struct PowerSupply {
    /// Power supply name
    pub name: String,

    /// Power supply type (Mains, Battery, etc.)
    pub supply_type: String,

    /// Online status
    pub online: bool,

    /// Present status
    pub present: bool,

    /// Health status
    pub health: Option<String>,

    /// Capacity percentage (for batteries)
    pub capacity: Option<u8>,
}

/// Linux power management capabilities
#[derive(Debug, Clone)]
pub struct LinuxPowerCapabilities {
    /// CPU frequency scaling available
    pub cpu_freq_scaling: bool,

    /// Available CPU governors
    pub cpu_governors: Vec<String>,

    /// GPU power management available
    pub gpu_power_management: bool,

    /// System suspend support
    pub suspend_support: bool,

    /// Hibernation support
    pub hibernation_support: bool,

    /// AC power connected
    pub ac_connected: bool,

    /// Battery present
    pub battery_present: bool,
}

/// Linux CPU monitor
#[derive(Debug)]
pub struct LinuxCPUMonitor {
    /// Number of CPUs
    pub cpu_count: u32,

    /// Previous CPU stats for utilization calculation
    pub prev_stats: Option<CpuStats>,
}

/// CPU statistics from /proc/stat
#[derive(Debug, Clone)]
pub struct CpuStats {
    /// User time
    pub user: u64,
    /// Nice time
    pub nice: u64,
    /// System time
    pub system: u64,
    /// Idle time
    pub idle: u64,
    /// I/O wait time
    pub iowait: u64,
    /// IRQ time
    pub irq: u64,
    /// Soft IRQ time
    pub softirq: u64,
    /// Steal time
    pub steal: u64,
}

/// Linux memory monitor
#[derive(Debug)]
pub struct LinuxMemoryMonitor;

/// Linux GPU monitor
#[derive(Debug)]
pub struct LinuxGPUMonitor {
    /// NVIDIA GPU devices
    pub nvidia_devices: Vec<NvidiaGpuDevice>,

    /// AMD GPU devices
    pub amd_devices: Vec<AmdGpuDevice>,

    /// Intel GPU devices
    pub intel_devices: Vec<IntelGpuDevice>,
}

/// NVIDIA GPU device
#[derive(Debug)]
pub struct NvidiaGpuDevice {
    /// Device index
    pub index: u32,
    /// Device name
    pub name: String,
    /// UUID
    pub uuid: String,
}

/// AMD GPU device
#[derive(Debug)]
pub struct AmdGpuDevice {
    /// Device index
    pub index: u32,
    /// Device name
    pub name: String,
}

/// Intel GPU device
#[derive(Debug)]
pub struct IntelGpuDevice {
    /// Device index
    pub index: u32,
    /// Device name
    pub name: String,
}

/// Linux thermal monitor
#[derive(Debug)]
pub struct LinuxThermalMonitor {
    /// Available thermal zones
    pub zones: Vec<String>,
}

/// Linux power monitor
#[derive(Debug)]
pub struct LinuxPowerMonitor {
    /// Available power supplies
    pub power_supplies: Vec<String>,
    /// Current power state
    pub current_state: Arc<RwLock<PowerState>>,
}

impl LinuxSystemMonitor {
    /// Create a new Linux system monitor
    pub async fn new() -> Result<Self, ComputeError> {
        info!("Initializing Linux system monitor");

        // Gather system information
        let system_info = Self::gather_system_info().await?;

        // Initialize CPU monitoring
        let cpu_monitor = Self::initialize_cpu_monitor().await?;

        // Initialize memory monitoring
        let memory_monitor = LinuxMemoryMonitor;

        // Initialize GPU monitoring (optional)
        let gpu_monitor = Self::initialize_gpu_monitor().await.ok();

        // Initialize thermal monitoring
        let thermal_monitor = Self::initialize_thermal_monitor().await?;

        // Initialize power monitoring
        let power_monitor = Self::initialize_power_monitor().await?;

        // Create initial system conditions
        let initial_conditions = SystemConditions {
            cpu_temperature_c: None,
            gpu_temperature_c: None,
            cpu_utilization: 0.0,
            gpu_utilization: 0.0,
            memory_utilization: 0.0,
            power_state: PowerState::Balanced,
            thermal_throttling: false,
            concurrent_workloads: 0,
        };

        Ok(LinuxSystemMonitor {
            system_info,
            monitoring_active: Arc::new(RwLock::new(false)),
            cached_conditions: Arc::new(RwLock::new(initial_conditions)),
            cpu_monitor,
            memory_monitor,
            gpu_monitor,
            thermal_monitor,
            power_monitor,
        })
    }

    /// Start background monitoring
    pub async fn start_monitoring(&mut self) -> Result<(), ComputeError> {
        info!("Starting Linux system monitoring");

        {
            let mut active = self
                .monitoring_active
                .write()
                .map_err(|e| ComputeError::System {
                    source: SystemError::MonitoringError {
                        reason: format!("Failed to acquire monitoring lock: {}", e),
                    },
                    resource: Some("monitoring_active".to_string()),
                })?;

            if *active {
                warn!("Linux system monitoring is already active");
                return Ok(());
            }

            *active = true;
        }

        // Start monitoring tasks
        let cached_conditions = Arc::clone(&self.cached_conditions);
        let monitoring_active = Arc::clone(&self.monitoring_active);

        tokio::spawn(async move {
            Self::monitoring_loop(cached_conditions, monitoring_active).await;
        });

        info!("Linux system monitoring started successfully");
        Ok(())
    }

    /// Get current system conditions
    pub async fn get_current_conditions(&self) -> Result<SystemConditions, ComputeError> {
        let conditions = self
            .cached_conditions
            .read()
            .map_err(|e| ComputeError::System {
                source: SystemError::MonitoringError {
                    reason: format!("Failed to read cached conditions: {}", e),
                },
                resource: Some("cached_conditions".to_string()),
            })?;

        Ok(conditions.clone())
    }

    /// Get Linux system information
    pub fn get_system_info(&self) -> LinuxSystemInfo {
        self.system_info.clone()
    }

    /// Gather system information from various Linux sources
    async fn gather_system_info() -> Result<LinuxSystemInfo, ComputeError> {
        debug!("Gathering Linux system information");

        // Get distribution info
        let distribution = Self::get_distribution_info().await?;

        // Get kernel info
        let kernel = Self::get_kernel_info().await?;

        // Get CPU info
        let cpu_info = Self::get_cpu_info().await?;

        // Get memory info
        let memory_info = Self::get_memory_info().await?;

        // Get GPU info
        let gpu_info = Self::get_gpu_info().await.unwrap_or_default();

        // Get hardware info
        let hardware_info = Self::get_hardware_info().await?;

        // Get power capabilities
        let power_capabilities = Self::get_power_capabilities().await?;

        Ok(LinuxSystemInfo {
            distribution,
            kernel,
            cpu_info,
            memory_info,
            gpu_info,
            hardware_info,
            power_capabilities,
        })
    }

    /// Get distribution information from /etc/os-release
    async fn get_distribution_info() -> Result<LinuxDistribution, ComputeError> {
        let os_release =
            fs::read_to_string("/etc/os-release").map_err(|e| ComputeError::Platform {
                source: PlatformError::Linux {
                    error: LinuxError::SysFS {
                        path: "/etc/os-release".to_string(),
                        error: e.to_string(),
                    },
                },
                platform: "Linux".to_string(),
            })?;

        let mut name = "Unknown".to_string();
        let mut version = "Unknown".to_string();
        let mut id = "unknown".to_string();
        let mut codename = None;
        let mut pretty_name = "Linux".to_string();

        for line in os_release.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                match key {
                    "NAME" => name = value.to_string(),
                    "VERSION" => version = value.to_string(),
                    "ID" => id = value.to_string(),
                    "VERSION_CODENAME" => codename = Some(value.to_string()),
                    "PRETTY_NAME" => pretty_name = value.to_string(),
                    _ => {}
                }
            }
        }

        Ok(LinuxDistribution {
            name,
            version,
            id,
            codename,
            pretty_name,
        })
    }

    /// Get kernel information from uname
    async fn get_kernel_info() -> Result<KernelInfo, ComputeError> {
        let version = fs::read_to_string("/proc/version")
            .map_err(|e| ComputeError::Platform {
                source: PlatformError::Linux {
                    error: LinuxError::ProcFS {
                        path: "/proc/version".to_string(),
                        error: e.to_string(),
                    },
                },
                platform: "Linux".to_string(),
            })?
            .trim()
            .to_string();

        // Parse kernel release from version string
        let release = version
            .split_whitespace()
            .nth(2)
            .unwrap_or("unknown")
            .to_string();

        // Get architecture
        let architecture = std::env::consts::ARCH.to_string();

        // Get hostname
        let hostname = fs::read_to_string("/proc/sys/kernel/hostname")
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();

        Ok(KernelInfo {
            version,
            release,
            architecture,
            hostname,
        })
    }

    /// Get CPU information from /proc/cpuinfo
    async fn get_cpu_info() -> Result<LinuxCPUInfo, ComputeError> {
        let cpuinfo = fs::read_to_string("/proc/cpuinfo").map_err(|e| ComputeError::Platform {
            source: PlatformError::Linux {
                error: LinuxError::ProcFS {
                    path: "/proc/cpuinfo".to_string(),
                    error: e.to_string(),
                },
            },
            platform: "Linux".to_string(),
        })?;

        let mut model_name = "Unknown".to_string();
        let mut vendor = "Unknown".to_string();
        let mut features = Vec::new();
        let mut logical_processors = 0u8;

        // Parse /proc/cpuinfo
        for line in cpuinfo.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "model name" if model_name == "Unknown" => {
                        model_name = value.to_string();
                    }
                    "vendor_id" if vendor == "Unknown" => {
                        vendor = value.to_string();
                    }
                    "flags" | "Features" if features.is_empty() => {
                        features = value.split_whitespace().map(|s| s.to_string()).collect();
                    }
                    "processor" => {
                        logical_processors += 1;
                    }
                    _ => {}
                }
            }
        }

        // Get frequency information
        let base_frequency_mhz = Self::read_cpu_frequency("cpuinfo_min_freq")
            .await
            .unwrap_or(0)
            / 1000;
        let max_frequency_mhz = Self::read_cpu_frequency("cpuinfo_max_freq")
            .await
            .unwrap_or(0)
            / 1000;

        // Get cache information
        let cache_info = Self::get_cache_info().await.unwrap_or_default();

        // Get CPU governors
        let governors = Self::get_cpu_governors().await.unwrap_or_default();
        let current_governor = Self::get_current_governor()
            .await
            .unwrap_or("unknown".to_string());

        // Estimate physical cores (rough heuristic)
        let physical_cores = (logical_processors / 2).max(1);
        let sockets = 1; // Most systems have 1 socket

        Ok(LinuxCPUInfo {
            model_name,
            vendor,
            sockets,
            physical_cores,
            logical_processors,
            base_frequency_mhz,
            max_frequency_mhz,
            cache_info,
            features,
            governors,
            current_governor,
        })
    }

    /// Read CPU frequency from /sys/devices/system/cpu/cpu0/cpufreq/
    async fn read_cpu_frequency(file: &str) -> Result<u32, ComputeError> {
        let path = format!("/sys/devices/system/cpu/cpu0/cpufreq/{}", file);
        let content = fs::read_to_string(&path).map_err(|e| ComputeError::Platform {
            source: PlatformError::Linux {
                error: LinuxError::SysFS {
                    path,
                    error: e.to_string(),
                },
            },
            platform: "Linux".to_string(),
        })?;

        content.trim().parse().map_err(|e| ComputeError::Platform {
            source: PlatformError::Linux {
                error: LinuxError::SysFS {
                    path: file.to_string(),
                    error: format!("Parse error: {}", e),
                },
            },
            platform: "Linux".to_string(),
        })
    }

    /// Get CPU cache information
    async fn get_cache_info() -> Result<LinuxCacheInfo, ComputeError> {
        // Default cache info if we can't read from sysfs
        Ok(LinuxCacheInfo {
            l1d_size_kb: 32,
            l1i_size_kb: 32,
            l2_size_kb: 256,
            l3_size_kb: 8192,
        })
    }

    /// Get available CPU governors
    async fn get_cpu_governors() -> Result<Vec<String>, ComputeError> {
        let path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors";
        if let Ok(content) = fs::read_to_string(path) {
            Ok(content.split_whitespace().map(|s| s.to_string()).collect())
        } else {
            Ok(vec!["performance".to_string(), "powersave".to_string()])
        }
    }

    /// Get current CPU governor
    async fn get_current_governor() -> Result<String, ComputeError> {
        let path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
        if let Ok(content) = fs::read_to_string(path) {
            Ok(content.trim().to_string())
        } else {
            Ok("unknown".to_string())
        }
    }

    /// Get memory information from /proc/meminfo
    async fn get_memory_info() -> Result<LinuxMemoryInfo, ComputeError> {
        let meminfo = fs::read_to_string("/proc/meminfo").map_err(|e| ComputeError::Platform {
            source: PlatformError::Linux {
                error: LinuxError::ProcFS {
                    path: "/proc/meminfo".to_string(),
                    error: e.to_string(),
                },
            },
            platform: "Linux".to_string(),
        })?;

        let mut mem_data = HashMap::new();

        // Parse /proc/meminfo
        for line in meminfo.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                if let Some(num_str) = value.split_whitespace().next() {
                    if let Ok(num) = num_str.parse::<u64>() {
                        mem_data.insert(key.to_string(), num);
                    }
                }
            }
        }

        // Convert from kB to GB
        let total_gb = mem_data.get("MemTotal").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let available_gb =
            mem_data.get("MemAvailable").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let free_gb = mem_data.get("MemFree").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let buffers_gb = mem_data.get("Buffers").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let cached_gb = mem_data.get("Cached").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let swap_total_gb =
            mem_data.get("SwapTotal").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;
        let swap_free_gb = mem_data.get("SwapFree").copied().unwrap_or(0) as f32 / 1024.0 / 1024.0;

        Ok(LinuxMemoryInfo {
            total_gb,
            available_gb,
            free_gb,
            buffers_gb,
            cached_gb,
            swap_total_gb,
            swap_free_gb,
            speed_mhz: None,   // Not easily available on Linux
            memory_type: None, // Not easily available on Linux
        })
    }

    /// Get GPU information (basic stub - would need specific GPU vendor implementations)
    async fn get_gpu_info() -> Result<Vec<LinuxGPUInfo>, ComputeError> {
        let mut gpus = Vec::new();

        // Try to detect GPUs from /sys/class/drm
        if let Ok(entries) = fs::read_dir("/sys/class/drm") {
            let mut device_index = 0;
            for entry in entries.flatten() {
                if let Some(gpu_info) = Self::process_gpu_entry(&entry, device_index).await {
                    gpus.push(gpu_info);
                    device_index += 1;
                }
            }
        }

        Ok(gpus)
    }

    /// Process a single GPU entry from /sys/class/drm
    async fn process_gpu_entry(
        entry: &std::fs::DirEntry,
        device_index: u32,
    ) -> Option<LinuxGPUInfo> {
        let name = entry.file_name();
        let name_str = name.to_str()?;

        if !name_str.starts_with("card") || name_str.contains('-') {
            return None;
        }

        let (vendor_id, device_id) = Self::read_gpu_ids(name_str).await;
        let vendor = Self::parse_gpu_vendor(&vendor_id);
        let gpu_name = format!("{} GPU ({})", vendor, device_id);

        Some(LinuxGPUInfo {
            device_index,
            name: gpu_name,
            vendor,
            pci_id: format!("{}:{}", vendor_id, device_id),
            driver: "unknown".to_string(),
            driver_version: "unknown".to_string(),
            memory_mb: 0, // Would need vendor-specific querying
            current_freq_mhz: None,
            max_freq_mhz: None,
            power_limit_w: None,
            cuda_supported: vendor_id == "0x10de",
            opencl_supported: true, // Assume OpenCL support
            vulkan_supported: true, // Assume Vulkan support
        })
    }

    /// Read GPU vendor and device IDs
    async fn read_gpu_ids(name_str: &str) -> (String, String) {
        let vendor_path = format!("/sys/class/drm/{}/device/vendor", name_str);
        let device_path = format!("/sys/class/drm/{}/device/device", name_str);

        let vendor_id = fs::read_to_string(&vendor_path)
            .unwrap_or_default()
            .trim()
            .to_string();
        let device_id = fs::read_to_string(&device_path)
            .unwrap_or_default()
            .trim()
            .to_string();

        (vendor_id, device_id)
    }

    /// Parse GPU vendor from vendor ID
    fn parse_gpu_vendor(vendor_id: &str) -> String {
        match vendor_id {
            "0x10de" => "NVIDIA",
            "0x1002" => "AMD",
            "0x8086" => "Intel",
            _ => "Unknown",
        }
        .to_string()
    }

    /// Get hardware information from DMI
    async fn get_hardware_info() -> Result<LinuxHardwareInfo, ComputeError> {
        let manufacturer = Self::read_dmi_field("sys_vendor").await;
        let product_name = Self::read_dmi_field("product_name").await;
        let version = Self::read_dmi_field("product_version").await;
        let serial_number = Self::read_dmi_field("product_serial").await;

        // BIOS information
        let bios_info = if let (Ok(vendor), Ok(version), Ok(date)) = (
            Self::read_dmi_field("bios_vendor").await,
            Self::read_dmi_field("bios_version").await,
            Self::read_dmi_field("bios_date").await,
        ) {
            Some(BiosInfo {
                vendor,
                version,
                date,
            })
        } else {
            None
        };

        // Board information
        let board_info = if let (Ok(manufacturer), Ok(product_name), Ok(version)) = (
            Self::read_dmi_field("board_vendor").await,
            Self::read_dmi_field("board_name").await,
            Self::read_dmi_field("board_version").await,
        ) {
            Some(BoardInfo {
                manufacturer,
                product_name,
                version,
            })
        } else {
            None
        };

        // Thermal zones
        let thermal_zones = Self::get_thermal_zones().await.unwrap_or_default();

        // Power supplies
        let power_supplies = Self::get_power_supplies().await.unwrap_or_default();

        Ok(LinuxHardwareInfo {
            manufacturer: manufacturer.ok(),
            product_name: product_name.ok(),
            version: version.ok(),
            serial_number: serial_number.ok(),
            bios_info,
            board_info,
            thermal_zones,
            power_supplies,
        })
    }

    /// Read DMI field from /sys/class/dmi/id/
    async fn read_dmi_field(field: &str) -> Result<String, ComputeError> {
        let path = format!("/sys/class/dmi/id/{}", field);
        fs::read_to_string(&path)
            .map_err(|e| ComputeError::Platform {
                source: PlatformError::Linux {
                    error: LinuxError::SysFS {
                        path,
                        error: e.to_string(),
                    },
                },
                platform: "Linux".to_string(),
            })
            .map(|s| s.trim().to_string())
    }

    /// Get thermal zones from /sys/class/thermal
    async fn get_thermal_zones() -> Result<Vec<ThermalZone>, ComputeError> {
        let mut zones = Vec::new();

        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Some(name_str) = name.to_str() {
                    if name_str.starts_with("thermal_zone") {
                        let base_path = format!("/sys/class/thermal/{}", name_str);

                        // Read zone type
                        let zone_type = fs::read_to_string(format!("{}/type", base_path))
                            .unwrap_or_else(|_| "unknown".to_string())
                            .trim()
                            .to_string();

                        // Read temperature (in millidegrees)
                        let temp_millidegrees = fs::read_to_string(format!("{}/temp", base_path))
                            .ok()
                            .and_then(|s| s.trim().parse::<i32>().ok());

                        let temp_celsius = temp_millidegrees.map(|t| t as f32 / 1000.0);

                        // Try to read critical and passive temperatures
                        let crit_temp_celsius =
                            fs::read_to_string(format!("{}/trip_point_0_temp", base_path))
                                .ok()
                                .and_then(|s| s.trim().parse::<i32>().ok())
                                .map(|t| t as f32 / 1000.0);

                        zones.push(ThermalZone {
                            zone_type,
                            temp_millidegrees,
                            temp_celsius,
                            crit_temp_celsius,
                            passive_temp_celsius: None, // Could be implemented
                        });
                    }
                }
            }
        }

        Ok(zones)
    }

    /// Get power supplies from /sys/class/power_supply
    async fn get_power_supplies() -> Result<Vec<PowerSupply>, ComputeError> {
        let mut supplies = Vec::new();

        if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let base_path = format!("/sys/class/power_supply/{}", name);

                // Read power supply properties
                let supply_type = fs::read_to_string(format!("{}/type", base_path))
                    .unwrap_or_else(|_| "Unknown".to_string())
                    .trim()
                    .to_string();

                let online = fs::read_to_string(format!("{}/online", base_path))
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false);

                let present = fs::read_to_string(format!("{}/present", base_path))
                    .map(|s| s.trim() == "1")
                    .unwrap_or(true);

                let health = fs::read_to_string(format!("{}/health", base_path))
                    .ok()
                    .map(|s| s.trim().to_string());

                let capacity = fs::read_to_string(format!("{}/capacity", base_path))
                    .ok()
                    .and_then(|s| s.trim().parse::<u8>().ok());

                supplies.push(PowerSupply {
                    name,
                    supply_type,
                    online,
                    present,
                    health,
                    capacity,
                });
            }
        }

        Ok(supplies)
    }

    /// Get power management capabilities
    async fn get_power_capabilities() -> Result<LinuxPowerCapabilities, ComputeError> {
        let cpu_freq_scaling = Path::new("/sys/devices/system/cpu/cpu0/cpufreq").exists();
        let cpu_governors = Self::get_cpu_governors().await.unwrap_or_default();

        // Check for suspend/hibernate support
        let suspend_support = Path::new("/sys/power/state").exists();
        let hibernation_support = fs::read_to_string("/sys/power/state")
            .map(|s| s.contains("disk"))
            .unwrap_or(false);

        // Check power supply status
        let power_supplies = Self::get_power_supplies().await.unwrap_or_default();
        let ac_connected = power_supplies
            .iter()
            .any(|ps| ps.supply_type == "Mains" && ps.online);
        let battery_present = power_supplies
            .iter()
            .any(|ps| ps.supply_type == "Battery" && ps.present);

        Ok(LinuxPowerCapabilities {
            cpu_freq_scaling,
            cpu_governors,
            gpu_power_management: false, // Would need GPU-specific implementation
            suspend_support,
            hibernation_support,
            ac_connected,
            battery_present,
        })
    }

    /// Initialize CPU monitoring
    async fn initialize_cpu_monitor() -> Result<LinuxCPUMonitor, ComputeError> {
        let cpu_count = num_cpus::get() as u32;

        Ok(LinuxCPUMonitor {
            cpu_count,
            prev_stats: None,
        })
    }

    /// Initialize GPU monitoring
    async fn initialize_gpu_monitor() -> Result<LinuxGPUMonitor, ComputeError> {
        Ok(LinuxGPUMonitor {
            nvidia_devices: Vec::new(), // Would detect NVIDIA devices
            amd_devices: Vec::new(),    // Would detect AMD devices
            intel_devices: Vec::new(),  // Would detect Intel devices
        })
    }

    /// Initialize thermal monitoring
    async fn initialize_thermal_monitor() -> Result<LinuxThermalMonitor, ComputeError> {
        let zones = if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("thermal_zone") {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(LinuxThermalMonitor { zones })
    }

    /// Initialize power monitoring
    async fn initialize_power_monitor() -> Result<LinuxPowerMonitor, ComputeError> {
        let power_supplies = if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .collect()
        } else {
            Vec::new()
        };

        Ok(LinuxPowerMonitor {
            power_supplies,
            current_state: Arc::new(RwLock::new(PowerState::Balanced)),
        })
    }

    /// Main monitoring loop
    async fn monitoring_loop(
        cached_conditions: Arc<RwLock<SystemConditions>>,
        monitoring_active: Arc<RwLock<bool>>,
    ) {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Check if monitoring should continue
            {
                let active = monitoring_active.read().unwrap();
                if !*active {
                    info!("Linux system monitoring stopped");
                    break;
                }
            }

            // Sample system conditions
            match Self::sample_system_conditions().await {
                Ok(conditions) => {
                    if let Ok(mut cached) = cached_conditions.write() {
                        *cached = conditions;
                    }
                }
                Err(e) => {
                    error!("Failed to sample system conditions: {}", e);
                }
            }
        }
    }

    /// Sample current system conditions
    async fn sample_system_conditions() -> Result<SystemConditions, ComputeError> {
        // Get CPU utilization
        let cpu_utilization = Self::get_cpu_utilization().await.unwrap_or(0.0);

        // Get memory utilization
        let memory_utilization = Self::get_memory_utilization().await.unwrap_or(0.0);

        // Get CPU temperature
        let cpu_temperature_c = Self::get_cpu_temperature().await;

        // Get GPU temperature (if available)
        let gpu_temperature_c = Self::get_gpu_temperature().await;

        // Check thermal throttling
        let thermal_throttling = Self::is_thermal_throttling().await.unwrap_or(false);

        // Estimate concurrent workloads (from load average)
        let concurrent_workloads = Self::get_load_average().await.unwrap_or(1.0) as u32;

        // Determine power state (simplified)
        let power_state = Self::determine_power_state().await;

        Ok(SystemConditions {
            cpu_temperature_c,
            gpu_temperature_c,
            cpu_utilization,
            gpu_utilization: 0.0, // Would need GPU-specific implementation
            memory_utilization,
            power_state,
            thermal_throttling,
            concurrent_workloads,
        })
    }

    /// Get CPU utilization by reading /proc/stat
    async fn get_cpu_utilization() -> Result<f32, ComputeError> {
        let stat_content =
            fs::read_to_string("/proc/stat").map_err(|e| ComputeError::Platform {
                source: PlatformError::Linux {
                    error: LinuxError::ProcFS {
                        path: "/proc/stat".to_string(),
                        error: e.to_string(),
                    },
                },
                platform: "Linux".to_string(),
            })?;

        if let Some(line) = stat_content.lines().next() {
            if line.starts_with("cpu ") {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 8 {
                    let user: u64 = fields[1].parse().unwrap_or(0);
                    let nice: u64 = fields[2].parse().unwrap_or(0);
                    let system: u64 = fields[3].parse().unwrap_or(0);
                    let idle: u64 = fields[4].parse().unwrap_or(0);
                    let iowait: u64 = fields[5].parse().unwrap_or(0);
                    let irq: u64 = fields[6].parse().unwrap_or(0);
                    let softirq: u64 = fields[7].parse().unwrap_or(0);
                    let steal: u64 = if fields.len() > 8 {
                        fields[8].parse().unwrap_or(0)
                    } else {
                        0
                    };

                    let total = user + nice + system + idle + iowait + irq + softirq + steal;
                    let idle_total = idle + iowait;
                    let non_idle = total - idle_total;

                    if total > 0 {
                        return Ok((non_idle as f32 / total as f32) * 100.0);
                    }
                }
            }
        }

        Ok(0.0)
    }

    /// Get memory utilization from /proc/meminfo
    async fn get_memory_utilization() -> Result<f32, ComputeError> {
        let meminfo = Self::get_memory_info().await?;

        if meminfo.total_gb > 0.0 {
            let used_gb = meminfo.total_gb - meminfo.available_gb;
            Ok((used_gb / meminfo.total_gb) * 100.0)
        } else {
            Ok(0.0)
        }
    }

    /// Get CPU temperature from thermal zones
    async fn get_cpu_temperature() -> Option<f32> {
        if let Ok(zones) = Self::get_thermal_zones().await {
            for zone in zones {
                if zone.zone_type.to_lowercase().contains("cpu")
                    || zone.zone_type.to_lowercase().contains("x86_pkg_temp")
                    || zone.zone_type.to_lowercase().contains("coretemp")
                {
                    if let Some(temp) = zone.temp_celsius {
                        return Some(temp);
                    }
                }
            }
        }
        None
    }

    /// Get GPU temperature (placeholder)
    async fn get_gpu_temperature() -> Option<f32> {
        // Would need GPU vendor-specific implementation
        None
    }

    /// Check if system is thermal throttling
    async fn is_thermal_throttling() -> Result<bool, ComputeError> {
        // Check if any thermal zone is at critical temperature
        if let Ok(zones) = Self::get_thermal_zones().await {
            for zone in zones {
                if let (Some(temp), Some(crit_temp)) = (zone.temp_celsius, zone.crit_temp_celsius) {
                    if temp >= crit_temp * 0.95 {
                        // 95% of critical temperature
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Get load average from /proc/loadavg
    async fn get_load_average() -> Result<f32, ComputeError> {
        let loadavg = fs::read_to_string("/proc/loadavg").map_err(|e| ComputeError::Platform {
            source: PlatformError::Linux {
                error: LinuxError::ProcFS {
                    path: "/proc/loadavg".to_string(),
                    error: e.to_string(),
                },
            },
            platform: "Linux".to_string(),
        })?;

        if let Some(load_str) = loadavg.split_whitespace().next() {
            Ok(load_str.parse().unwrap_or(1.0))
        } else {
            Ok(1.0)
        }
    }

    /// Determine power state based on system conditions
    async fn determine_power_state() -> PowerState {
        // Check if we're on battery power
        if let Ok(supplies) = Self::get_power_supplies().await {
            let on_battery = supplies.iter().any(|ps| {
                ps.supply_type == "Battery"
                    && ps.present
                    && !supplies
                        .iter()
                        .any(|ac| ac.supply_type == "Mains" && ac.online)
            });

            if on_battery {
                return PowerState::PowerSaver;
            }
        }

        // Default to balanced
        PowerState::Balanced
    }
}

impl Default for LinuxCacheInfo {
    fn default() -> Self {
        LinuxCacheInfo {
            l1d_size_kb: 32,
            l1i_size_kb: 32,
            l2_size_kb: 256,
            l3_size_kb: 8192,
        }
    }
}

impl Drop for LinuxSystemMonitor {
    fn drop(&mut self) {
        // Stop monitoring when the monitor is dropped
        if let Ok(mut active) = self.monitoring_active.write() {
            *active = false;
        }

        info!("Linux system monitor dropped, monitoring stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_linux_monitor_creation() {
        let monitor = LinuxSystemMonitor::new().await;
        // Don't fail if we can't access system files in test environment
        if monitor.is_err() {
            return;
        }

        let monitor = monitor.unwrap();
        assert!(!monitor.system_info.distribution.name.is_empty());
    }

    #[tokio::test]
    async fn test_distribution_info() {
        // This test might fail in containers or non-standard environments
        if let Ok(dist) = LinuxSystemMonitor::get_distribution_info().await {
            assert!(!dist.name.is_empty());
            assert!(!dist.id.is_empty());
        }
    }

    #[test]
    fn test_cpu_stats_parsing() {
        let stats = CpuStats {
            user: 1000,
            nice: 100,
            system: 500,
            idle: 8000,
            iowait: 200,
            irq: 50,
            softirq: 50,
            steal: 100,
        };

        let total = stats.user
            + stats.nice
            + stats.system
            + stats.idle
            + stats.iowait
            + stats.irq
            + stats.softirq
            + stats.steal;
        let idle_total = stats.idle + stats.iowait;
        let utilization = ((total - idle_total) as f32 / total as f32) * 100.0;

        assert!(utilization > 0.0);
        assert!(utilization < 100.0);
    }
}
