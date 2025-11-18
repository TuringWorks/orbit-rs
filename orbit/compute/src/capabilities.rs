//! Hardware capability detection and runtime feature discovery
//!
//! This module provides comprehensive detection of available compute capabilities
//! across CPU, GPU, and Neural Engine hardware, enabling optimal workload placement.

use std::fmt;

use crate::errors::{CapabilityDetectionError, ComputeError};
use serde::{Deserialize, Serialize};

/// Universal compute capability detection across all supported hardware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalComputeCapabilities {
    /// CPU capabilities and SIMD instruction support
    pub cpu: CPUCapabilities,
    /// GPU capabilities across all available devices
    pub gpu: GPUCapabilities,
    /// Neural engine and AI accelerator capabilities
    pub neural: NeuralEngineCapabilities,
    /// ARM-specific specialized compute units
    pub arm_specialized: ARMSpecializedCapabilities,
    /// Memory architecture and unified memory support
    pub memory_architecture: MemoryArchitecture,
    /// Platform-specific optimizations available
    pub platform_optimizations: PlatformOptimizations,
}

/// Comprehensive CPU capability detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CPUCapabilities {
    /// CPU architecture (x86-64 or AArch64)
    pub architecture: CPUArchitecture,
    /// SIMD instruction set support
    pub simd: SIMDCapabilities,
    /// Core configuration and topology
    pub cores: CoreConfiguration,
    /// Cache hierarchy information
    pub cache_hierarchy: CacheHierarchy,
    /// CPU vendor-specific optimizations
    pub vendor_optimizations: VendorOptimizations,
}

/// CPU architecture enumeration with detailed capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CPUArchitecture {
    /// x86-64 architecture (Intel/AMD)
    X86_64 {
        /// CPU vendor (Intel, AMD)
        vendor: X86Vendor,
        /// Microarchitecture details
        microarchitecture: X86Microarch,
        /// Available CPU features
        features: X86Features,
    },
    /// AArch64 architecture (ARM64)
    AArch64 {
        /// ARM vendor (Apple, Qualcomm, etc.)
        vendor: ARMVendor,
        /// System-on-Chip details
        soc: ARMSoC,
        /// Available ARM features
        features: ARMFeatures,
    },
}

/// x86-64 CPU vendors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum X86Vendor {
    /// Intel processors
    Intel,
    /// AMD processors  
    AMD,
    /// Other x86-64 compatible processors
    Other(String),
}

/// x86-64 microarchitecture families
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum X86Microarch {
    // Intel microarchitectures
    /// Intel Raptor Lake (13th gen)
    RaptorLake,
    /// Intel Alder Lake (12th gen)
    AlderLake,
    /// Intel Tiger Lake (11th gen mobile)
    TigerLake,
    /// Intel Ice Lake
    IceLake,
    /// Intel Skylake family
    Skylake,
    /// Intel Haswell family
    Haswell,

    // AMD microarchitectures
    /// AMD Zen 4 (Ryzen 7000, EPYC 9004 "Genoa")
    Zen4 { epyc_model: Option<EPYCModel> },
    /// AMD Zen 4c (EPYC 9004 "Bergamo" - cloud optimized)
    Zen4c { epyc_model: Option<EPYCModel> },
    /// AMD Zen 3 (Ryzen 5000, EPYC 7003 "Milan")
    Zen3 { epyc_model: Option<EPYCModel> },
    /// AMD Zen 2 (Ryzen 3000, EPYC 7002 "Rome")
    Zen2 { epyc_model: Option<EPYCModel> },
    /// AMD Zen+ (Ryzen 2000)
    ZenPlus { epyc_model: Option<EPYCModel> },
    /// AMD Zen (Ryzen 1000, EPYC 7001 "Naples")
    Zen { epyc_model: Option<EPYCModel> },
    /// AMD Zen 5 (Future EPYC and Ryzen - 2024+)
    Zen5 { epyc_model: Option<EPYCModel> },

    /// Unknown microarchitecture
    Unknown(String),
}

/// ARM vendors and their SoCs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ARMVendor {
    /// Apple Silicon
    Apple(AppleChip),
    /// Qualcomm Snapdragon
    Qualcomm(Box<SnapdragonChip>),
    /// Samsung Exynos
    Samsung(ExynosChip),
    /// MediaTek Dimensity
    MediaTek(DimensityChip),
    /// NVIDIA Tegra
    NVIDIA(TegraChip),
    /// Amazon Graviton
    Amazon(GravitonChip),
    /// Other ARM processors
    Other(String),
}

/// Samsung Exynos chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExynosChip {
    /// Exynos 2400 (Galaxy S24)
    Exynos2400,
    /// Exynos 2200 (Galaxy S22)
    Exynos2200,
    /// Other Exynos variant
    Other(String),
}

/// MediaTek Dimensity chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DimensityChip {
    /// Dimensity 9300
    Dimensity9300,
    /// Dimensity 8300
    Dimensity8300,
    /// Other Dimensity variant
    Other(String),
}

/// NVIDIA Tegra chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TegraChip {
    /// Tegra Orin
    TegraOrin,
    /// Tegra Xavier
    TegraXavier,
    /// Other Tegra variant
    Other(String),
}

/// Amazon Graviton chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GravitonChip {
    /// Graviton 4
    Graviton4,
    /// Graviton 3
    Graviton3,
    /// Graviton 2
    Graviton2,
}

/// Apple Silicon chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppleChip {
    /// M1 series
    M1 {
        /// M1 chip variant (base, Pro, Max, Ultra)
        variant: M1Variant,
        /// CPU core configuration
        cores: CoreConfiguration,
    },
    /// M2 series  
    M2 {
        /// M2 chip variant (base, Pro, Max, Ultra)
        variant: M2Variant,
        /// CPU core configuration
        cores: CoreConfiguration,
    },
    /// M3 series
    M3 {
        /// M3 chip variant (base, Pro, Max)
        variant: M3Variant,
        /// CPU core configuration
        cores: CoreConfiguration,
    },
    /// M4 series (future)
    M4 {
        /// M4 chip variant (base, Pro, Max)
        variant: M4Variant,
        /// CPU core configuration
        cores: CoreConfiguration,
    },
    /// A-series (iOS devices)
    A17Pro,
    /// A16 Bionic chip
    A16Bionic,
    /// A15 Bionic chip
    A15Bionic,
}

/// M1 chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum M1Variant {
    /// Standard M1
    M1,
    /// M1 Pro
    M1Pro,
    /// M1 Max
    M1Max,
    /// M1 Ultra
    M1Ultra,
}

/// M2 chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum M2Variant {
    /// Standard M2
    M2,
    /// M2 Pro
    M2Pro,
    /// M2 Max
    M2Max,
    /// M2 Ultra
    M2Ultra,
}

/// M3 chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum M3Variant {
    /// Standard M3
    M3,
    /// M3 Pro
    M3Pro,
    /// M3 Max
    M3Max,
}

/// M4 chip variants (future)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum M4Variant {
    /// M4 base variant
    M4,
    /// M4 Pro variant
    M4Pro,
    /// M4 Max variant
    M4Max,
}

/// Qualcomm Snapdragon chip variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapdragonChip {
    /// Snapdragon 8 Gen 3 (flagship mobile)
    Snapdragon8Gen3 {
        /// CPU configuration
        cpu_config: SnapdragonCPUConfig,
        /// Adreno GPU
        adreno_gpu: AdrenoGPU,
        /// Hexagon DSP
        hexagon_dsp: HexagonDSP,
        /// Spectra ISP
        spectra_isp: SpectraISP,
        /// Sensing Hub
        sensing_hub: SensingHub,
    },
    /// Snapdragon 8 Gen 2
    Snapdragon8Gen2 {
        /// CPU configuration
        cpu_config: SnapdragonCPUConfig,
        /// Adreno GPU
        adreno_gpu: AdrenoGPU,
        /// Hexagon DSP
        hexagon_dsp: HexagonDSP,
        /// Spectra ISP
        spectra_isp: SpectraISP,
    },
    /// Snapdragon X Elite/Plus (ARM Windows laptops)
    SnapdragonX {
        /// CPU configuration
        cpu_config: SnapdragonXCPUConfig,
        /// Adreno GPU
        adreno_gpu: AdrenoGPU,
        /// Hexagon DSP
        hexagon_dsp: HexagonDSP,
        /// Oryon custom cores
        oryon_cores: OryonCoreConfig,
    },
    /// Other Snapdragon variants
    Other {
        /// Model name
        model: String,
        /// CPU configuration
        cpu_config: SnapdragonCPUConfig,
        /// Optional Adreno GPU
        adreno_gpu: Option<AdrenoGPU>,
    },
}

/// Snapdragon CPU configuration with heterogeneous cores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapdragonCPUConfig {
    /// Prime cores (highest performance)
    pub prime_cores: u8,
    /// Performance cores (mid-tier)
    pub performance_cores: u8,
    /// Efficiency cores (power saving)
    pub efficiency_cores: u8,
    /// Maximum frequency in GHz
    pub max_freq_ghz: f32,
    /// ARM NEON SIMD support
    pub neon_support: bool,
    /// Scalable Vector Extension support
    pub sve_support: bool,
    /// SME (Scalable Matrix Extension) support
    pub sme_support: bool,
}

/// Snapdragon X series CPU configuration (for ARM Windows)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapdragonXCPUConfig {
    /// Oryon performance cores
    pub oryon_cores: u8,
    /// Base frequency in GHz
    pub base_freq_ghz: f32,
    /// Boost frequency in GHz  
    pub boost_freq_ghz: f32,
    /// Advanced SIMD support
    pub advanced_simd: bool,
    /// Windows on ARM optimizations
    pub windows_arm_optimizations: bool,
}

/// Oryon custom core configuration (Snapdragon X series)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OryonCoreConfig {
    /// Number of Oryon cores
    pub core_count: u8,
    /// Custom instruction set extensions
    pub custom_extensions: Vec<String>,
    /// Performance characteristics
    pub performance_profile: OryonPerformanceProfile,
}

/// Oryon performance profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OryonPerformanceProfile {
    /// Single-threaded performance score (relative)
    pub single_thread_score: f32,
    /// Multi-threaded performance score (relative)
    pub multi_thread_score: f32,
    /// Power efficiency rating
    pub power_efficiency: f32,
}

/// SIMD instruction set capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SIMDCapabilities {
    /// x86-64 SIMD features
    pub x86_features: Option<X86SIMDFeatures>,
    /// ARM SIMD features
    pub arm_features: Option<ARMSIMDFeatures>,
    /// Optimal vector width in bytes
    pub optimal_vector_width: usize,
    /// Maximum vector width supported
    pub max_vector_width: usize,
}

/// x86-64 SIMD instruction sets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X86SIMDFeatures {
    /// SSE instruction sets
    pub sse: SSESupport,
    /// AVX instruction sets  
    pub avx: AVXSupport,
    /// FMA (Fused Multiply-Add) support
    pub fma: bool,
    /// BMI (Bit Manipulation Instructions) support
    pub bmi: BMISupport,
}

/// SSE support levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSESupport {
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
}

/// AVX support levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AVXSupport {
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub avx512bw: bool,
    pub avx512cd: bool,
    pub avx512dq: bool,
    pub avx512vl: bool,
    pub avx512_vnni: bool,
    pub avx512_bf16: bool,
    pub avx512_fp16: bool,
}

/// BMI (Bit Manipulation Instructions) support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BMISupport {
    pub bmi1: bool,
    pub bmi2: bool,
}

/// ARM SIMD instruction sets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ARMSIMDFeatures {
    /// ARM NEON support
    pub neon: bool,
    /// Advanced SIMD support
    pub advanced_simd: bool,
    /// SVE (Scalable Vector Extension) support
    pub sve: Option<SVESupport>,
    /// SME (Scalable Matrix Extension) support  
    pub sme: Option<SMESupport>,
    /// Half-precision floating point support
    pub fp16: bool,
    /// BFloat16 support
    pub bf16: bool,
}

/// SVE support details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SVESupport {
    /// SVE vector length in bits (128-2048)
    pub vector_length: u32,
    /// SVE2 support
    pub sve2: bool,
}

/// SME support details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SMESupport {
    /// SME matrix tile support
    pub matrix_tiles: bool,
    /// SME2 support
    pub sme2: bool,
}

/// Core configuration and topology
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoreConfiguration {
    /// Physical CPU cores
    pub physical_cores: u8,
    /// Logical CPU cores (including hyperthreading)
    pub logical_cores: u8,
    /// Performance cores (P-cores)
    pub performance_cores: u8,
    /// Efficiency cores (E-cores)
    pub efficiency_cores: u8,
    /// Base frequency in MHz
    pub base_frequency_mhz: u32,
    /// Maximum boost frequency in MHz
    pub max_frequency_mhz: u32,
    /// Whether hyperthreading/SMT is enabled
    pub hyperthreading: bool,
}

/// Cache hierarchy information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHierarchy {
    /// L1 data cache per core in KB
    pub l1d_cache_kb: u32,
    /// L1 instruction cache per core in KB
    pub l1i_cache_kb: u32,
    /// L2 cache per core in KB
    pub l2_cache_kb: u32,
    /// L3 cache total in KB
    pub l3_cache_kb: u32,
    /// Cache line size in bytes
    pub cache_line_size: u32,
}

/// CPU vendor-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorOptimizations {
    /// Intel-specific optimizations
    pub intel: Option<IntelOptimizations>,
    /// AMD-specific optimizations
    pub amd: Option<AMDOptimizations>,
    /// ARM vendor-specific optimizations
    pub arm: Option<ARMOptimizations>,
}

/// Intel-specific optimization features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelOptimizations {
    /// Intel Turbo Boost support
    pub turbo_boost: bool,
    /// Intel Thermal Velocity Boost
    pub thermal_velocity_boost: bool,
    /// Intel Thread Director (for hybrid architectures)
    pub thread_director: bool,
    /// Intel Deep Learning Boost
    pub dl_boost: bool,
}

/// AMD-specific optimization features with enhanced EPYC support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMDOptimizations {
    /// AMD Precision Boost
    pub precision_boost: bool,
    /// AMD Precision Boost Overdrive
    pub precision_boost_overdrive: bool,
    /// AMD Smart Access Memory
    pub smart_access_memory: bool,
    /// AMD 3D V-Cache
    pub three_d_v_cache: bool,
    /// EPYC-specific optimization features
    pub epyc_optimizations: Option<EPYCOptimizations>,
    /// AMD Infinity Fabric optimizations
    pub infinity_fabric_optimizations: InfinityFabricOptimizations,
    /// Cache coherency and NUMA optimizations
    pub numa_optimizations: NUMAOptimizations,
}

/// EPYC processor-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EPYCOptimizations {
    /// Platform Quality of Service (QoS) features
    pub platform_qos: bool,
    /// Memory bandwidth optimization
    pub memory_bandwidth_optimization: bool,
    /// Inter-socket communication optimization
    pub inter_socket_optimization: bool,
    /// Virtualization optimizations (SEV, SME)
    pub virtualization_optimizations: VirtualizationOptimizations,
    /// Cloud-specific optimizations (Bergamo)
    pub cloud_optimizations: Option<CloudOptimizations>,
    /// Enterprise security features
    pub enterprise_security: EnterpriseSecurityFeatures,
}

/// Infinity Fabric optimization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfinityFabricOptimizations {
    /// Automatic fabric frequency scaling
    pub auto_frequency_scaling: bool,
    /// Memory-to-fabric frequency ratio optimization
    pub memory_fabric_ratio_optimization: bool,
    /// Inter-die communication optimization
    pub inter_die_optimization: bool,
    /// Cross-socket bandwidth optimization
    pub cross_socket_optimization: bool,
}

/// NUMA (Non-Uniform Memory Access) optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NUMAOptimizations {
    /// Automatic NUMA balancing
    pub auto_numa_balancing: bool,
    /// Memory locality optimization
    pub memory_locality_optimization: bool,
    /// Thread affinity optimization
    pub thread_affinity_optimization: bool,
    /// Cache coherency optimizations
    pub cache_coherency_optimization: bool,
}

/// Virtualization-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualizationOptimizations {
    /// Secure Memory Encryption (SME)
    pub secure_memory_encryption: bool,
    /// Secure Encrypted Virtualization (SEV)
    pub secure_encrypted_virtualization: bool,
    /// SEV-SNP (Secure Nested Paging)
    pub sev_snp: bool,
    /// Hardware-assisted virtualization performance
    pub hardware_assisted_performance: bool,
}

/// Cloud-specific optimizations for Bergamo and similar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudOptimizations {
    /// Density-optimized core configuration
    pub density_optimization: bool,
    /// Container workload optimization
    pub container_optimization: bool,
    /// Microservice architecture optimization
    pub microservice_optimization: bool,
    /// Multi-tenant performance isolation
    pub multi_tenant_isolation: bool,
}

/// Enterprise security features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterpriseSecurityFeatures {
    /// Memory Guard (hardware memory encryption)
    pub memory_guard: bool,
    /// Silicon Root of Trust
    pub silicon_root_of_trust: bool,
    /// Secure Boot support
    pub secure_boot_support: bool,
    /// Hardware-based attestation
    pub hardware_attestation: bool,
}

/// ARM vendor-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ARMOptimizations {
    /// Apple-specific optimizations
    pub apple: Option<AppleOptimizations>,
    /// Qualcomm-specific optimizations
    pub qualcomm: Option<QualcommOptimizations>,
}

/// Apple Silicon optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleOptimizations {
    /// Unified memory architecture
    pub unified_memory: bool,
    /// Apple Neural Engine integration
    pub neural_engine: bool,
    /// Metal Performance Shaders
    pub metal_performance_shaders: bool,
}

/// Qualcomm Snapdragon optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualcommOptimizations {
    /// Hexagon DSP integration
    pub hexagon_dsp: bool,
    /// Adreno GPU integration
    pub adreno_gpu: bool,
    /// AI Engine acceleration
    pub ai_engine: bool,
}

/// Comprehensive GPU capabilities across vendors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUCapabilities {
    /// List of available GPU devices
    pub available_devices: Vec<GPUDevice>,
    /// Primary GPU for compute operations
    pub primary_gpu: Option<usize>,
    /// Integrated GPU availability
    pub integrated_gpu: Option<IntegratedGPU>,
    /// Unified memory support (Apple Silicon)
    pub unified_memory_support: bool,
}

/// Individual GPU device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUDevice {
    /// GPU vendor and model
    pub vendor: GPUVendor,
    /// Device name/model
    pub name: String,
    /// Compute capability version
    pub compute_capability: String,
    /// Memory size in GB
    pub memory_gb: f32,
    /// Memory bandwidth in GB/s
    pub memory_bandwidth_gbps: f32,
    /// Compute units (CUDA cores, Stream processors, etc.)
    pub compute_units: u32,
    /// Base clock frequency in MHz
    pub base_clock_mhz: u32,
    /// Boost clock frequency in MHz
    pub boost_clock_mhz: u32,
    /// Supported compute APIs
    pub compute_apis: ComputeAPISupport,
    /// Performance characteristics
    pub performance: GPUPerformance,
}

/// GPU vendor enumeration with detailed specifications
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GPUVendor {
    /// Apple Silicon GPU
    Apple(AppleGPU),
    /// NVIDIA GPU
    NVIDIA(Box<NvidiaGPU>),
    /// AMD GPU
    AMD(AmdGPU),
    /// Intel GPU
    Intel(IntelGPU),
    /// Qualcomm Adreno GPU
    Qualcomm(AdrenoGPU),
    /// ARM Mali GPU
    ARM(MaliGPU),
    /// Other GPU vendor
    Other(String),
}

/// Apple Silicon GPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleGPU {
    /// GPU core count
    pub gpu_cores: u32,
    /// Metal feature set
    pub metal_feature_set: String,
    /// Neural Engine TOPS
    pub neural_engine_tops: f32,
    /// Unified memory bandwidth
    pub unified_memory_bandwidth_gbps: f32,
}

/// NVIDIA GPU details with enhanced cloud and model support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaGPU {
    /// GPU architecture (Ampere, Ada Lovelace, Hopper, etc.)
    pub architecture: NvidiaArchitecture,
    /// Specific GPU model for cloud deployments
    pub model: NvidiaModel,
    /// CUDA compute capability
    pub cuda_capability: (u8, u8),
    /// RT cores (for ray tracing)
    pub rt_cores: Option<u32>,
    /// Tensor cores (for AI workloads)
    pub tensor_cores: Option<u32>,
    /// NVENC/NVDEC support
    pub encoder_decoder: bool,
    /// Multi-Instance GPU (MIG) support
    pub mig_support: bool,
    /// Cloud provider information
    pub cloud_provider: Option<CloudProvider>,
    /// Instance type for cloud deployments
    pub cloud_instance_type: Option<String>,
    /// Performance characteristics
    pub performance_profile: GPUPerformanceProfile,
}

/// NVIDIA GPU architectures with enhanced support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NvidiaArchitecture {
    /// Blackwell (B100, B200, GB200 series) - Next-gen AI/ML (2024+)
    Blackwell,
    /// Hopper (H100, H200 series) - Latest for AI/ML
    Hopper,
    /// Ada Lovelace (RTX 40 series, L4, L40S)
    AdaLovelace,
    /// Ampere (RTX 30 series, A100, A10G)
    Ampere,
    /// Turing (RTX 20 series, T4)
    Turing,
    /// Volta (Titan V, V100)
    Volta,
    /// Pascal (GTX 10 series)
    Pascal,
    /// Other architecture
    Other(String),
}

/// Specific NVIDIA GPU models for cloud deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NvidiaModel {
    // Blackwell Architecture (Next-Gen 2024+)
    /// B100 SXM 192GB - Next-gen AI training flagship
    B100SXM192GB,
    /// B200 SXM 288GB - Ultra-large memory AI training
    B200SXM288GB,
    /// GB200 SuperChip - Grace + Blackwell superchip
    GB200SuperChip,
    /// B40 PCIe 48GB - Mid-range Blackwell for inference
    B40PCIe48GB,
    /// B100 PCIe 128GB - PCIe variant of B100
    B100PCIe128GB,

    // Hopper Architecture (Current Gen)
    /// H100 SXM 80GB - Data center flagship
    H100SXM80GB,
    /// H100 PCIe 80GB - Server deployment
    H100PCIe80GB,
    /// H100 NVL 94GB - Large memory variant
    H100NVL94GB,
    /// H200 SXM 141GB - Latest with HBM3e
    H200SXM141GB,

    // Ampere Architecture (Current Gen)
    /// A100 SXM 80GB - ML training powerhouse
    A100SXM80GB,
    /// A100 SXM 40GB - Standard ML training
    A100SXM40GB,
    /// A100 PCIe 80GB - Server variant
    A100PCIe80GB,
    /// A100 PCIe 40GB - Server variant
    A100PCIe40GB,
    /// A10G - Graphics and AI inference
    A10G,
    /// A10 - Professional graphics
    A10,

    // Turing Architecture (Inference)
    /// T4 - Cost-effective inference
    T4,
    /// T4G - T4 with enhanced memory
    T4G,

    // Volta Architecture (Legacy)
    /// V100 SXM 32GB - Legacy training
    V100SXM32GB,
    /// V100 PCIe 32GB - Legacy server
    V100PCIe32GB,
    /// V100 SXM 16GB - Smaller memory
    V100SXM16GB,

    // Ada Lovelace (Latest Consumer/Pro)
    /// L4 - Inference optimized
    L4,
    /// L40S - Professional workstation
    L40S,
    /// RTX 4090 - High-end consumer
    RTX4090,
    /// RTX 4080 - Mid-high consumer
    RTX4080,

    // Other models
    Other(String),
}

/// AMD GPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdGPU {
    /// GPU architecture
    pub architecture: AMDArchitecture,
    /// Compute units
    pub compute_units: u32,
    /// Stream processors
    pub stream_processors: u32,
    /// Memory type
    pub memory_type: AMDMemoryType,
    /// ROCm support version
    pub rocm_version: Option<String>,
    /// RDNA/GCN generation
    pub generation: u8,
}

/// AMD GPU architectures with enhanced data center support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AMDArchitecture {
    /// RDNA 4 (RX 8000 series - 2024+)
    RDNA4 { model: RDNA4Model },
    /// RDNA 3 (RX 7000 series)
    RDNA3 { model: RDNA3Model },
    /// RDNA 2 (RX 6000 series)
    RDNA2 { model: RDNA2Model },
    /// RDNA 1 (RX 5000 series)
    RDNA1 { model: RDNA1Model },
    /// GCN architecture
    GCN { generation: u8 },
    /// CDNA 3 (MI300 series - latest data center)
    CDNA3 { model: CDNA3Model },
    /// CDNA 2 (MI200 series)
    CDNA2 { model: CDNA2Model },
    /// CDNA 1 (MI100 series)
    CDNA1 { model: CDNA1Model },
}

/// RDNA 3 GPU models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RDNA3Model {
    RX7900XTX,
    RX7900XT,
    RX7800XT,
    RX7700XT,
    RX7600XT,
    RX7600,
    // Mobile variants
    RX7900M,
    RX7800M,
    RX7700S,
}

/// RDNA 2 GPU models  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RDNA2Model {
    RX6950XT,
    RX6900XT,
    RX6800XT,
    RX6800,
    RX6700XT,
    RX6600XT,
    RX6600,
    RX6500XT,
    RX6400,
}

/// RDNA 1 GPU models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RDNA1Model {
    RX5700XT,
    RX5700,
    RX5600XT,
    RX5500XT,
}

/// RDNA 4 GPU models (2024+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RDNA4Model {
    /// RX 8800 XT - High-end RDNA 4
    RX8800XT,
    /// RX 8700 XT - Mid-high RDNA 4
    RX8700XT,
    /// RX 8600 XT - Mid-range RDNA 4
    RX8600XT,
    /// RX 8500 XT - Entry-level RDNA 4
    RX8500XT,
    /// Future RDNA 4 models
    Future(String),
}

/// CDNA 3 data center GPU models (MI300 series)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CDNA3Model {
    /// MI300X - 192GB HBM3, optimized for AI inference
    MI300X,
    /// MI300A - APU with CPU + GPU, 128GB unified memory
    MI300A,
    /// MI300C - Compute-optimized variant
    MI300C,
}

/// CDNA 2 data center GPU models (MI200 series)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CDNA2Model {
    /// MI250X - 128GB HBM2e, flagship CDNA2
    MI250X,
    /// MI250 - Standard CDNA2 model
    MI250,
    /// MI210 - Entry-level CDNA2
    MI210,
}

/// CDNA 1 data center GPU models (MI100 series)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CDNA1Model {
    /// MI100 - First generation CDNA
    MI100,
}

/// AMD GPU memory types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AMDMemoryType {
    GDDR6,
    GDDR6X,
    HBM2,
    HBM2E,
    HBM3,
    HBM3E, // Latest high-bandwidth memory for MI300 series
}

/// Qualcomm Adreno GPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdrenoGPU {
    /// Adreno GPU model
    pub model: AdrenoModel,
    /// Compute units
    pub compute_units: u32,
    /// Memory bandwidth in GB/s
    pub memory_bandwidth_gbps: f32,
    /// FP32 compute performance in TFLOPS
    pub fp32_tflops: f32,
    /// FP16 compute performance in TFLOPS
    pub fp16_tflops: f32,
    /// INT8 performance in TOPS
    pub int8_tops: f32,
    /// Vulkan API version
    pub vulkan_version: String,
    /// OpenCL version
    pub opencl_version: String,
    /// DirectX support (Windows on ARM)
    pub directx_support: bool,
}

/// Adreno GPU model variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdrenoModel {
    /// Adreno 750 (Snapdragon 8 Gen 3)
    Adreno750,
    /// Adreno 740 (Snapdragon 8 Gen 2)
    Adreno740,
    /// Adreno 730 (Snapdragon 8 Gen 1)
    Adreno730,
    /// Adreno X1-85 (Snapdragon X Elite)
    AdrenoX1_85,
    /// Adreno X1-75 (Snapdragon X Plus)
    AdrenoX1_75,
    /// Other Adreno model
    Other(String),
}

/// Intel GPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelGPU {
    /// Intel GPU architecture
    pub architecture: IntelGPUArchitecture,
    /// Execution units
    pub execution_units: u32,
    /// Intel Xe architecture details
    pub xe_details: Option<IntelXeDetails>,
}

/// Intel GPU architectures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntelGPUArchitecture {
    /// Intel Arc (discrete)
    Arc,
    /// Intel Xe (integrated)
    Xe,
    /// Intel Iris
    Iris,
    /// Intel UHD
    UHD,
    /// Other Intel GPU
    Other(String),
}

/// Intel Xe architecture details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelXeDetails {
    /// Xe-LP (low power)
    pub xe_lp: bool,
    /// Xe-HP (high performance)
    pub xe_hp: bool,
    /// Xe-HPG (high performance gaming)
    pub xe_hpg: bool,
    /// XMX units for AI workloads
    pub xmx_units: Option<u32>,
}

/// ARM Mali GPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaliGPU {
    /// Mali GPU model
    pub model: String,
    /// Shader cores
    pub shader_cores: u32,
    /// GPU frequency in MHz
    pub frequency_mhz: u32,
}

/// Cloud provider types for GPU deployments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CloudProvider {
    /// Amazon Web Services
    AWS {
        /// AWS region
        region: String,
        /// Availability zone (optional)
        availability_zone: Option<String>,
    },
    /// Microsoft Azure
    Azure {
        /// Azure region
        region: String,
        /// Resource group
        resource_group: Option<String>,
    },
    /// Google Cloud Platform
    GCP {
        /// GCP region
        region: String,
        /// GCP zone
        zone: Option<String>,
        /// Project ID
        project_id: Option<String>,
    },
    /// Digital Ocean
    DigitalOcean {
        /// DO region
        region: String,
        /// VPC UUID (optional)
        vpc_uuid: Option<String>,
    },
    /// On-premises or bare metal
    OnPremises,
    /// Other cloud provider
    Other(String),
}

/// GPU performance profiles for different workload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPerformanceProfile {
    /// Vector operations performance (TFLOPS)
    pub vector_operations: GPUPerformanceMetric,
    /// Matrix operations performance (TFLOPS)
    pub matrix_operations: GPUPerformanceMetric,
    /// ML inference performance (TOPS)
    pub ml_inference: GPUPerformanceMetric,
    /// ML training performance (TFLOPS)
    pub ml_training: GPUPerformanceMetric,
    /// Memory bandwidth utilization efficiency (0.0-1.0)
    pub memory_efficiency: f32,
    /// Power consumption characteristics
    pub power_profile: GPUPowerProfile,
    /// Thermal characteristics
    pub thermal_profile: GPUThermalProfile,
    /// Workload-specific optimizations
    pub workload_optimizations: Vec<WorkloadOptimization>,
}

/// GPU performance metrics for different precision types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPerformanceMetric {
    /// FP32 performance
    pub fp32_performance: f32,
    /// FP16 performance  
    pub fp16_performance: f32,
    /// BF16 performance (brain float)
    pub bf16_performance: Option<f32>,
    /// FP8 performance (H100 and newer)
    pub fp8_performance: Option<f32>,
    /// INT8 performance
    pub int8_performance: Option<f32>,
    /// INT4 performance
    pub int4_performance: Option<f32>,
}

/// GPU power consumption profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPowerProfile {
    /// Thermal design power in watts
    pub tdp_watts: u32,
    /// Typical power consumption in watts
    pub typical_power_watts: u32,
    /// Minimum power consumption in watts
    pub minimum_power_watts: u32,
    /// Maximum power consumption in watts
    pub maximum_power_watts: u32,
    /// Power efficiency (TFLOPS per watt)
    pub power_efficiency: f32,
    /// Supports dynamic voltage/frequency scaling
    pub dvfs_support: bool,
}

/// GPU thermal characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUThermalProfile {
    /// Maximum operating temperature in Celsius
    pub max_temp_celsius: u8,
    /// Thermal throttling temperature in Celsius
    pub throttle_temp_celsius: u8,
    /// Idle temperature in Celsius
    pub idle_temp_celsius: u8,
    /// Cooling requirements
    pub cooling_requirements: CoolingRequirements,
}

/// GPU cooling requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoolingRequirements {
    /// Passive cooling (fanless)
    Passive,
    /// Active air cooling
    ActiveAir,
    /// Liquid cooling required
    LiquidCooling,
    /// Data center cooling
    DataCenter,
}

/// Workload-specific optimizations available on GPU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadOptimization {
    /// Workload type
    pub workload_type: WorkloadType,
    /// Performance multiplier for this workload
    pub performance_multiplier: f32,
    /// Special features enabled
    pub special_features: Vec<String>,
}

/// Types of workloads that can be optimized
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadType {
    /// Vector similarity search
    VectorSimilarity,
    /// Matrix multiplications
    MatrixMultiply,
    /// Deep learning inference
    DLInference,
    /// Deep learning training
    DLTraining,
    /// Computer vision
    ComputerVision,
    /// Natural language processing
    NLP,
    /// Time series analysis
    TimeSeriesAnalysis,
    /// Graph analytics
    GraphAnalytics,
    /// Cryptographic operations
    Cryptography,
    /// General compute
    GeneralCompute,
}

/// Supported compute APIs for GPU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeAPISupport {
    /// OpenCL support
    pub opencl: Option<String>,
    /// Vulkan compute support
    pub vulkan: Option<String>,
    /// CUDA support (NVIDIA only)
    pub cuda: Option<String>,
    /// Metal support (Apple only)
    pub metal: Option<String>,
    /// DirectCompute/DirectML (Windows)
    pub direct_compute: Option<String>,
    /// ROCm/HIP support (AMD only)
    pub rocm: Option<String>,
}

/// GPU performance characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPerformance {
    /// FP32 performance in TFLOPS
    pub fp32_tflops: f32,
    /// FP16 performance in TFLOPS  
    pub fp16_tflops: f32,
    /// INT8 performance in TOPS
    pub int8_tops: f32,
    /// Memory bandwidth utilization efficiency
    pub memory_efficiency: f32,
    /// Power consumption in watts
    pub tdp_watts: u32,
}

/// Integrated GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedGPU {
    /// Whether integrated GPU is available
    pub available: bool,
    /// Shared system memory in GB
    pub shared_memory_gb: f32,
    /// GPU vendor
    pub vendor: GPUVendor,
}

/// Neural engine and AI accelerator capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NeuralEngineCapabilities {
    /// Apple Neural Engine
    AppleNeuralEngine {
        /// Neural Engine version/generation
        version: AppleNeuralEngineVersion,
        /// Performance in TOPS
        tops: f32,
        /// Unified memory access
        unified_memory: bool,
        /// Core ML optimization support
        coreml_optimized: bool,
    },
    /// Qualcomm AI Engine (Hexagon DSP + AI accelerator)
    QualcommAI {
        /// Hexagon DSP details
        hexagon_dsp: HexagonDSP,
        /// Sensing Hub details
        sensing_hub: Option<SensingHub>,
        /// Total AI performance in TOPS
        ai_engine_tops: f32,
        /// FastCV computer vision support
        fastcv_support: bool,
    },
    /// Intel Neural Compute
    IntelNeuralCompute {
        /// Neural compute version
        version: IntelNeuralComputeVersion,
        /// Inference execution units
        execution_units: u32,
        /// AI performance in TOPS
        tops: f32,
        /// OpenVINO support
        openvino_support: bool,
    },
    /// AMD Neural accelerator
    AMDNeuralAccelerator {
        /// XDNA AI accelerator version
        xdna_version: Option<u8>,
        /// ROCm AI framework support
        rocm_ai_support: bool,
        /// Estimated AI performance in TOPS
        estimated_tops: f32,
    },
    /// No dedicated neural acceleration
    None,
}

/// Apple Neural Engine versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppleNeuralEngineVersion {
    /// M1 series Neural Engine (15.8 TOPS)
    M1,
    /// M1 Pro Neural Engine (15.8 TOPS)
    M1Pro,
    /// M1 Max Neural Engine (15.8 TOPS)
    M1Max,
    /// M2 series Neural Engine (15.8 TOPS)
    M2,
    /// M2 Pro Neural Engine (15.8 TOPS)
    M2Pro,
    /// M2 Max Neural Engine (15.8 TOPS)
    M2Max,
    /// M3 series Neural Engine (18 TOPS)
    M3,
    /// M3 Pro Neural Engine (18 TOPS)
    M3Pro,
    /// M3 Max Neural Engine (18 TOPS)
    M3Max,
    /// A17 Pro Neural Engine (35 TOPS)
    A17Pro,
    /// A16 Bionic Neural Engine (17 TOPS)
    A16Bionic,
}

/// Qualcomm Hexagon DSP details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexagonDSP {
    /// Hexagon DSP version (e.g., Hexagon 780)
    pub version: HexagonVersion,
    /// Vector processing units
    pub vector_units: u8,
    /// Tensor accelerator availability
    pub tensor_accelerator: bool,
    /// AI operations per second
    pub ai_ops_per_sec: u64,
    /// Power efficiency (TOPS per Watt)
    pub power_efficiency: f32,
    /// HVX (Hexagon Vector Extensions) support
    pub hvx_support: bool,
}

/// Hexagon DSP version enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HexagonVersion {
    /// Hexagon 780 (Snapdragon 8 Gen 3)
    Hexagon780,
    /// Hexagon 770 (Snapdragon 8 Gen 2)
    Hexagon770,
    /// Hexagon 760 (Snapdragon 8 Gen 1)
    Hexagon760,
    /// Other Hexagon version
    Other(String),
}

/// Qualcomm Sensing Hub details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensingHub {
    /// Always-on processing capability
    pub always_on: bool,
    /// Supported sensors
    pub supported_sensors: Vec<SensorType>,
    /// Power consumption in mW
    pub power_consumption_mw: f32,
}

/// Sensor types supported by Sensing Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorType {
    Accelerometer,
    Gyroscope,
    Magnetometer,
    AmbientLight,
    Proximity,
    Microphone,
    Camera,
    Other(String),
}

/// Qualcomm Spectra ISP (Image Signal Processor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectraISP {
    /// ISP version
    pub version: String,
    /// Maximum resolution supported
    pub max_resolution: (u32, u32),
    /// AI-enhanced image processing
    pub ai_enhancement: bool,
    /// Computational photography features
    pub computational_photography: Vec<String>,
}

/// Intel Neural Compute versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntelNeuralComputeVersion {
    /// Neural Processing Unit in Intel processors
    NPU,
    /// Gaussian & Neural Accelerator
    GNA,
    /// Other Intel neural compute
    Other(String),
}

/// ARM-specific specialized compute capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ARMSpecializedCapabilities {
    /// DSP (Digital Signal Processor) availability
    pub dsp_support: Option<DSPCapabilities>,
    /// ISP (Image Signal Processor) support
    pub isp_support: Option<ISPCapabilities>,
    /// NPU (Neural Processing Unit) support
    pub npu_support: Option<NPUCapabilities>,
    /// Custom acceleration units
    pub custom_accelerators: Vec<CustomAccelerator>,
}

/// DSP capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DSPCapabilities {
    /// DSP type/vendor
    pub dsp_type: String,
    /// Vector processing capability
    pub vector_processing: bool,
    /// Audio processing optimization
    pub audio_processing: bool,
    /// Signal processing TOPS
    pub signal_processing_tops: f32,
}

/// ISP capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISPCapabilities {
    /// ISP vendor/type
    pub isp_type: String,
    /// Maximum supported resolution
    pub max_resolution: (u32, u32),
    /// AI-enhanced processing
    pub ai_enhancement: bool,
    /// Real-time processing capability
    pub real_time_processing: bool,
}

/// NPU capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NPUCapabilities {
    /// NPU vendor/type
    pub npu_type: String,
    /// AI performance in TOPS
    pub ai_tops: f32,
    /// Supported AI frameworks
    pub supported_frameworks: Vec<String>,
    /// Quantization support
    pub quantization_support: Vec<String>,
}

/// Custom accelerator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAccelerator {
    /// Accelerator name
    pub name: String,
    /// Accelerator type/purpose
    pub accelerator_type: AcceleratorType,
    /// Performance characteristics
    pub performance_tops: f32,
    /// Supported operations
    pub supported_operations: Vec<String>,
}

/// Types of custom accelerators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AcceleratorType {
    /// Matrix/Tensor operations
    MatrixProcessor,
    /// Cryptographic acceleration
    CryptoProcessor,
    /// Video encoding/decoding
    VideoProcessor,
    /// Audio processing
    AudioProcessor,
    /// Network packet processing
    NetworkProcessor,
    /// Custom/proprietary accelerator
    Custom(String),
}

/// Memory architecture characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryArchitecture {
    /// Unified memory support (Apple Silicon)
    pub unified_memory: bool,
    /// Total system memory in GB
    pub total_memory_gb: f32,
    /// Memory bandwidth in GB/s
    pub memory_bandwidth_gbps: f32,
    /// Memory type
    pub memory_type: MemoryType,
    /// NUMA (Non-Uniform Memory Access) topology
    pub numa_topology: Option<NUMATopology>,
    /// Memory compression support
    pub memory_compression: bool,
}

/// Memory types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    /// DDR4 system memory
    DDR4,
    /// DDR5 system memory
    DDR5,
    /// LPDDR4X (mobile)
    LPDDR4X,
    /// LPDDR5 (mobile)
    LPDDR5,
    /// LPDDR5X (latest mobile)
    LPDDR5X,
    /// Unified memory (Apple Silicon)
    UnifiedMemory,
    /// Other memory type
    Other(String),
}

/// NUMA topology information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NUMATopology {
    /// Number of NUMA nodes
    pub numa_nodes: u8,
    /// Memory per NUMA node in GB
    pub memory_per_node_gb: f32,
    /// CPU cores per NUMA node
    pub cores_per_node: u8,
}

/// Platform-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOptimizations {
    /// macOS-specific optimizations
    pub macos: Option<MacOSOptimizations>,
    /// iOS-specific optimizations
    pub ios: Option<IOSOptimizations>,
    /// Windows-specific optimizations
    pub windows: Option<WindowsOptimizations>,
    /// Linux-specific optimizations
    pub linux: Option<LinuxOptimizations>,
    /// Android-specific optimizations
    pub android: Option<AndroidOptimizations>,
}

/// macOS platform optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOSOptimizations {
    /// Grand Central Dispatch optimization
    pub gcd_optimized: bool,
    /// Metal Performance Shaders
    pub metal_performance_shaders: bool,
    /// Core ML framework
    pub core_ml: bool,
    /// Accelerate framework
    pub accelerate_framework: bool,
}

/// iOS platform optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSOptimizations {
    /// Metal compute optimization
    pub metal_optimized: bool,
    /// Neural Engine optimization
    pub neural_engine_optimized: bool,
    /// Power efficiency modes
    pub power_efficiency: bool,
}

/// Windows platform optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsOptimizations {
    /// DirectML support
    pub directml: bool,
    /// Windows ML
    pub windows_ml: bool,
    /// Thread pool optimization
    pub thread_pool_optimized: bool,
    /// Windows on ARM optimizations
    pub windows_on_arm: bool,
}

/// Linux platform optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxOptimizations {
    /// OpenMP support
    pub openmp: bool,
    /// NUMA awareness
    pub numa_aware: bool,
    /// CPU isolation support
    pub cpu_isolation: bool,
    /// Real-time kernel support
    pub real_time_support: bool,
}

/// Android platform optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidOptimizations {
    /// NNAPI (Neural Networks API) support
    pub nnapi: bool,
    /// Vulkan compute support
    pub vulkan_compute: bool,
    /// Android GPU Inspector compatibility
    pub gpu_inspector: bool,
    /// Power-aware scheduling
    pub power_aware_scheduling: bool,
}

/// Core configuration details (used by Apple chips)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Performance cores
    pub performance_cores: u8,
    /// Efficiency cores  
    pub efficiency_cores: u8,
    /// GPU cores
    pub gpu_cores: u32,
    /// Neural Engine cores
    pub neural_cores: u16,
    /// Memory controllers
    pub memory_controllers: u8,
}

impl fmt::Display for UniversalComputeCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CPU: {:?}, GPU devices: {}, Neural: {:?}",
            self.cpu.architecture,
            self.gpu.available_devices.len(),
            self.neural
        )
    }
}

/// Detect all available compute capabilities on the current system
pub async fn detect_all_capabilities() -> Result<UniversalComputeCapabilities, ComputeError> {
    tracing::info!("Starting comprehensive hardware capability detection");

    let cpu = detect_cpu_capabilities()?;
    let gpu = detect_gpu_capabilities().await?;
    let neural = detect_neural_capabilities().await?;
    let arm_specialized = detect_arm_specialized_capabilities().await?;
    let memory_architecture = detect_memory_architecture()?;
    let platform_optimizations = detect_platform_optimizations()?;

    Ok(UniversalComputeCapabilities {
        cpu,
        gpu,
        neural,
        arm_specialized,
        memory_architecture,
        platform_optimizations,
    })
}

/// Detect CPU capabilities including SIMD instruction sets
fn detect_cpu_capabilities() -> Result<CPUCapabilities, ComputeError> {
    tracing::debug!("Detecting CPU capabilities");

    #[cfg(feature = "runtime-detection")]
    {
        // Use raw-cpuid for x86-64 detection
        #[cfg(target_arch = "x86_64")]
        {
            detect_x86_64_capabilities()
        }

        // Use system calls for AArch64 detection
        #[cfg(target_arch = "aarch64")]
        {
            detect_aarch64_capabilities()
        }
    }

    #[cfg(not(feature = "runtime-detection"))]
    {
        // Compile-time feature detection fallback
        detect_compile_time_capabilities()
    }
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_x86_64_capabilities() -> Result<CPUCapabilities, ComputeError> {
    use raw_cpuid::CpuId;

    let cpuid = CpuId::new();

    // Get vendor info
    let vendor = cpuid
        .get_vendor_info()
        .map(|vi| match vi.as_str() {
            "GenuineIntel" => X86Vendor::Intel,
            "AuthenticAMD" => X86Vendor::AMD,
            other => X86Vendor::Other(other.to_string()),
        })
        .unwrap_or(X86Vendor::Other("Unknown".to_string()));

    // Detect SIMD features
    let feature_info = cpuid.get_feature_info();
    let extended_features = cpuid.get_extended_feature_info();

    let simd = SIMDCapabilities {
        x86_features: Some(X86SIMDFeatures {
            sse: SSESupport {
                sse: feature_info.as_ref().is_some_and(|fi| fi.has_sse()),
                sse2: feature_info.as_ref().is_some_and(|fi| fi.has_sse2()),
                sse3: feature_info.as_ref().is_some_and(|fi| fi.has_sse3()),
                ssse3: feature_info.as_ref().is_some_and(|fi| fi.has_ssse3()),
                sse4_1: feature_info.as_ref().is_some_and(|fi| fi.has_sse41()),
                sse4_2: feature_info.as_ref().is_some_and(|fi| fi.has_sse42()),
            },
            avx: AVXSupport {
                avx: feature_info.as_ref().is_some_and(|fi| fi.has_avx()),
                avx2: extended_features.as_ref().is_some_and(|ef| ef.has_avx2()),
                avx512f: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512f()),
                avx512bw: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512bw()),
                avx512cd: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512cd()),
                avx512dq: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512dq()),
                avx512vl: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512vl()),
                avx512_vnni: extended_features
                    .as_ref()
                    .is_some_and(|ef| ef.has_avx512vnni()),
                avx512_bf16: false, // Would need more detailed detection
                avx512_fp16: false, // Would need more detailed detection
            },
            fma: feature_info.as_ref().is_some_and(|fi| fi.has_fma()),
            bmi: BMISupport {
                bmi1: extended_features.as_ref().is_some_and(|ef| ef.has_bmi1()),
                bmi2: extended_features.as_ref().is_some_and(|ef| ef.has_bmi2()),
            },
        }),
        arm_features: None,
        optimal_vector_width: if extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512f())
        {
            64 // 512 bits = 64 bytes
        } else if extended_features.as_ref().is_some_and(|ef| ef.has_avx2()) {
            32 // 256 bits = 32 bytes
        } else {
            16 // 128 bits = 16 bytes (SSE)
        },
        max_vector_width: if extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512f())
        {
            64
        } else if extended_features.as_ref().is_some_and(|ef| ef.has_avx2()) {
            32
        } else {
            16
        },
    };

    // Get core information
    let processor_frequency = cpuid.get_processor_frequency_info();
    let _cache_params = cpuid.get_cache_parameters();

    let cores = CoreConfiguration {
        physical_cores: num_cpus::get_physical() as u8,
        logical_cores: num_cpus::get() as u8,
        performance_cores: num_cpus::get_physical() as u8, // Simplified
        efficiency_cores: 0, // x86-64 typically doesn't have E-cores (except Intel 12th gen+)
        base_frequency_mhz: processor_frequency
            .as_ref()
            .map_or(0, |pf| pf.processor_base_frequency().into()),
        max_frequency_mhz: processor_frequency
            .as_ref()
            .map_or(0, |pf| pf.processor_max_frequency().into()),
        hyperthreading: num_cpus::get() > num_cpus::get_physical(),
    };

    // Cache hierarchy detection (simplified)
    let cache_hierarchy = CacheHierarchy {
        l1d_cache_kb: 32, // Typical values, would need more detailed detection
        l1i_cache_kb: 32,
        l2_cache_kb: 256,
        l3_cache_kb: 8192,
        cache_line_size: 64,
    };

    // Enhanced microarchitecture detection
    let (microarchitecture, x86_features) =
        detect_detailed_microarch_and_features(&vendor, &cpuid)?;

    let architecture = CPUArchitecture::X86_64 {
        vendor: vendor.clone(),
        microarchitecture: microarchitecture.clone(),
        features: x86_features,
    };

    // Enhanced vendor optimizations detection
    let vendor_optimizations = detect_vendor_optimizations(&vendor, &microarchitecture)?;

    Ok(CPUCapabilities {
        architecture,
        simd,
        cores,
        cache_hierarchy,
        vendor_optimizations,
    })
}

/// Detailed microarchitecture and feature detection
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_detailed_microarch_and_features<R: raw_cpuid::CpuIdReader>(
    vendor: &X86Vendor,
    cpuid: &raw_cpuid::CpuId<R>,
) -> Result<(X86Microarch, X86Features), ComputeError> {
    let feature_info = cpuid.get_feature_info();
    let extended_features = cpuid.get_extended_feature_info();
    let _extended_function_info = cpuid.get_extended_feature_info();

    match vendor {
        X86Vendor::AMD => {
            let (microarch, epyc_model) = detect_amd_microarch_and_model(cpuid)?;
            let amd_features = detect_amd_specific_features(cpuid, &microarch, &epyc_model)?;

            let x86_features = X86Features {
                avx_support: detect_avx_capabilities(&feature_info, &extended_features),
                aes: feature_info.as_ref().is_some_and(|fi| fi.has_aesni()),
                pclmulqdq: feature_info.as_ref().is_some_and(|fi| fi.has_pclmulqdq()),
                rdrand: feature_info.as_ref().is_some_and(|fi| fi.has_rdrand()),
                sha: extended_features.as_ref().is_some_and(|ef| ef.has_sha()),
                mpx: extended_features.as_ref().is_some_and(|ef| ef.has_mpx()),
                cet: false, // Would need more detailed detection
                mpk: extended_features.as_ref().is_some_and(|ef| ef.has_pku()),
                amd_features: Some(amd_features),
                intel_features: None,
            };

            Ok((microarch, x86_features))
        }
        X86Vendor::Intel => {
            let microarch = detect_intel_microarch(cpuid);
            let intel_features = detect_intel_specific_features(cpuid)?;

            let x86_features = X86Features {
                avx_support: detect_avx_capabilities(&feature_info, &extended_features),
                aes: feature_info.as_ref().is_some_and(|fi| fi.has_aesni()),
                pclmulqdq: feature_info.as_ref().is_some_and(|fi| fi.has_pclmulqdq()),
                rdrand: feature_info.as_ref().is_some_and(|fi| fi.has_rdrand()),
                sha: extended_features.as_ref().is_some_and(|ef| ef.has_sha()),
                mpx: extended_features.as_ref().is_some_and(|ef| ef.has_mpx()),
                cet: extended_features.as_ref().is_some_and(|ef| ef.has_cet_ss()),
                mpk: extended_features.as_ref().is_some_and(|ef| ef.has_pku()),
                amd_features: None,
                intel_features: Some(intel_features),
            };

            Ok((microarch, x86_features))
        }
        _ => {
            let microarch = X86Microarch::Unknown("Unknown".to_string());
            let x86_features = X86Features {
                avx_support: detect_avx_capabilities(&feature_info, &extended_features),
                aes: feature_info.as_ref().is_some_and(|fi| fi.has_aesni()),
                pclmulqdq: feature_info.as_ref().is_some_and(|fi| fi.has_pclmulqdq()),
                rdrand: feature_info.as_ref().is_some_and(|fi| fi.has_rdrand()),
                sha: extended_features.as_ref().is_some_and(|ef| ef.has_sha()),
                mpx: extended_features.as_ref().is_some_and(|ef| ef.has_mpx()),
                cet: false,
                mpk: extended_features.as_ref().is_some_and(|ef| ef.has_pku()),
                amd_features: None,
                intel_features: None,
            };

            Ok((microarch, x86_features))
        }
    }
}

/// Detect AMD microarchitecture and EPYC model
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_amd_microarch_and_model<R: raw_cpuid::CpuIdReader>(
    cpuid: &raw_cpuid::CpuId<R>,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    let processor_brand = cpuid
        .get_processor_brand_string()
        .map(|pbs| pbs.as_str().to_string())
        .unwrap_or_default();

    let feature_info = cpuid.get_feature_info();
    let family = feature_info.as_ref().map_or(0, |fi| fi.family_id());
    let model = feature_info.as_ref().map_or(0, |fi| fi.model_id());

    match family {
        0x19 => detect_zen_3_4_architecture(&processor_brand, model),
        0x17 => detect_zen_1_2_architecture(&processor_brand, model),
        _ => Ok((
            X86Microarch::Unknown(format!("AMD Family {:#x}", family)),
            None,
        )),
    }
}

/// Detect Zen 3/4 architecture (Family 19h)
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_3_4_architecture(
    processor_brand: &str,
    model: u32,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    if processor_brand.to_lowercase().contains("epyc") {
        return detect_zen_3_4_epyc_models(processor_brand);
    }

    Ok(detect_zen_3_4_consumer_models(model))
}

/// Detect Zen 1/2 architecture (Family 17h)
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_1_2_architecture(
    processor_brand: &str,
    model: u32,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    if processor_brand.to_lowercase().contains("epyc") {
        return detect_zen_1_2_epyc_models(processor_brand);
    }

    Ok(detect_zen_1_2_consumer_models(model))
}

/// Detect EPYC models for Zen 3/4
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_3_4_epyc_models(
    processor_brand: &str,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    let epyc_patterns = [
        ("9004", "Genoa", EPYCModelType::Genoa),
        ("9374F", "Bergamo", EPYCModelType::Bergamo),
        ("7003", "Milan", EPYCModelType::Milan),
        ("7V13", "Milan-X", EPYCModelType::MilanX),
    ];

    for (model_code, name, model_type) in epyc_patterns {
        if processor_brand.contains(model_code) || processor_brand.contains(name) {
            return create_epyc_result(processor_brand, model_type);
        }
    }

    // Default Zen 3 for unrecognized EPYC
    Ok((X86Microarch::Zen3 { epyc_model: None }, None))
}

/// Detect EPYC models for Zen 1/2
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_1_2_epyc_models(
    processor_brand: &str,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    let epyc_patterns = [
        ("7002", "Rome", EPYCModelType::Rome),
        ("7001", "Naples", EPYCModelType::Naples),
    ];

    for (model_code, name, model_type) in epyc_patterns {
        if processor_brand.contains(model_code) || processor_brand.contains(name) {
            return create_epyc_result(processor_brand, model_type);
        }
    }

    // Default Zen for unrecognized EPYC
    Ok((X86Microarch::Zen { epyc_model: None }, None))
}

/// Detect consumer Zen 3/4 models
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_3_4_consumer_models(model: u32) -> (X86Microarch, Option<EPYCModel>) {
    match model {
        0x10..=0x1F => (X86Microarch::Zen3 { epyc_model: None }, None),
        0x20..=0x2F => (X86Microarch::Zen4 { epyc_model: None }, None),
        _ => (X86Microarch::Zen3 { epyc_model: None }, None),
    }
}

/// Detect consumer Zen 1/2 models
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_zen_1_2_consumer_models(model: u32) -> (X86Microarch, Option<EPYCModel>) {
    match model {
        0x30..=0x3F => (X86Microarch::Zen2 { epyc_model: None }, None),
        _ => (X86Microarch::Zen { epyc_model: None }, None),
    }
}

/// EPYC model type enum for pattern matching
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
enum EPYCModelType {
    Genoa,
    Bergamo,
    Milan,
    MilanX,
    Rome,
    Naples,
}

/// Create EPYC result based on model type
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn create_epyc_result(
    processor_brand: &str,
    model_type: EPYCModelType,
) -> Result<(X86Microarch, Option<EPYCModel>), ComputeError> {
    let epyc_model = match model_type {
        EPYCModelType::Genoa => parse_genoa_model(processor_brand),
        EPYCModelType::Bergamo => parse_bergamo_model(processor_brand),
        EPYCModelType::Milan => parse_milan_model(processor_brand),
        EPYCModelType::MilanX => parse_milan_x_model(processor_brand),
        EPYCModelType::Rome => parse_rome_model(processor_brand),
        EPYCModelType::Naples => parse_naples_model(processor_brand),
    };

    let microarch = match model_type {
        EPYCModelType::Genoa => X86Microarch::Zen4 {
            epyc_model: epyc_model.clone(),
        },
        EPYCModelType::Bergamo => X86Microarch::Zen4c {
            epyc_model: epyc_model.clone(),
        },
        EPYCModelType::Milan | EPYCModelType::MilanX => X86Microarch::Zen3 {
            epyc_model: epyc_model.clone(),
        },
        EPYCModelType::Rome => X86Microarch::Zen2 {
            epyc_model: epyc_model.clone(),
        },
        EPYCModelType::Naples => X86Microarch::Zen {
            epyc_model: epyc_model.clone(),
        },
    };

    Ok((microarch, epyc_model))
}

// EPYC model parsing functions
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_genoa_model(brand_string: &str) -> Option<EPYCModel> {
    // Example: "AMD EPYC 9654 96-Core Processor"
    let cores = extract_core_count(brand_string).unwrap_or(96);
    let model_name = brand_string.to_string();

    Some(EPYCModel::Genoa {
        model_name,
        cores,
        threads: cores * 2,                               // SMT enabled
        base_freq_ghz: 2.4,                               // Typical base frequency
        boost_freq_ghz: 3.7,                              // Typical boost frequency
        l3_cache_mb: if cores >= 64 { 384 } else { 256 }, // Typical L3 cache
        tdp_watts: if cores >= 64 { 360 } else { 280 },
        memory_channels: 12, // DDR5 support
        pcie_lanes: 160,     // PCIe 5.0
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_bergamo_model(brand_string: &str) -> Option<EPYCModel> {
    let cores = extract_core_count(brand_string).unwrap_or(128);
    let model_name = brand_string.to_string();

    Some(EPYCModel::Bergamo {
        model_name,
        cores,
        threads: cores * 2,
        base_freq_ghz: 2.0, // Lower base frequency for density
        boost_freq_ghz: 3.0,
        l3_cache_mb: 256, // Optimized for cloud workloads
        tdp_watts: 360,
        optimized_for_cloud: true,
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_milan_model(brand_string: &str) -> Option<EPYCModel> {
    let cores = extract_core_count(brand_string).unwrap_or(64);
    let model_name = brand_string.to_string();

    Some(EPYCModel::Milan {
        model_name,
        cores,
        threads: cores * 2,
        base_freq_ghz: 2.2,
        boost_freq_ghz: 3.4,
        l3_cache_mb: if cores >= 32 { 256 } else { 128 },
        tdp_watts: if cores >= 32 { 280 } else { 225 },
        memory_support: MemorySupport {
            channels_per_socket: 8,
            max_capacity_gb: 4096, // 4TB max
            supported_speeds_mhz: vec![3200, 2933, 2666, 2400],
            ddr4_support: true,
            ddr5_support: false,
            ecc_support: true,
            memory_interleaving: true,
        },
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_milan_x_model(brand_string: &str) -> Option<EPYCModel> {
    let cores = extract_core_count(brand_string).unwrap_or(64);
    let model_name = brand_string.to_string();

    Some(EPYCModel::MilanX {
        model_name,
        cores,
        threads: cores * 2,
        base_freq_ghz: 2.2,
        boost_freq_ghz: 3.4,
        l3_cache_mb: if cores >= 32 { 256 } else { 128 },
        v_cache_mb: if cores >= 32 { 768 } else { 384 }, // 3D V-Cache
        tdp_watts: if cores >= 32 { 280 } else { 225 },
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_rome_model(brand_string: &str) -> Option<EPYCModel> {
    let cores = extract_core_count(brand_string).unwrap_or(64);
    let model_name = brand_string.to_string();

    Some(EPYCModel::Rome {
        model_name,
        cores,
        threads: cores * 2,
        base_freq_ghz: 2.0,
        boost_freq_ghz: 3.2,
        l3_cache_mb: if cores >= 32 { 256 } else { 128 },
        tdp_watts: if cores >= 32 { 280 } else { 200 },
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn parse_naples_model(brand_string: &str) -> Option<EPYCModel> {
    let cores = extract_core_count(brand_string).unwrap_or(32);
    let model_name = brand_string.to_string();

    Some(EPYCModel::Naples {
        model_name,
        cores,
        threads: cores * 2,
        base_freq_ghz: 2.0,
        boost_freq_ghz: 3.0,
        l3_cache_mb: if cores >= 16 { 64 } else { 32 },
        tdp_watts: if cores >= 16 { 180 } else { 155 },
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn extract_core_count(brand_string: &str) -> Option<u8> {
    // Look for patterns like "64-Core" or "96-Core"
    for part in brand_string.split_whitespace() {
        if part.ends_with("-Core") {
            let core_part = part.strip_suffix("-Core")?;
            if let Ok(cores) = core_part.parse::<u8>() {
                return Some(cores);
            }
        }
    }
    None
}

// AMD-specific feature detection
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_amd_specific_features<R: raw_cpuid::CpuIdReader>(
    cpuid: &raw_cpuid::CpuId<R>,
    microarch: &X86Microarch,
    epyc_model: &Option<EPYCModel>,
) -> Result<AMDSpecificFeatures, ComputeError> {
    let _extended_features = cpuid.get_extended_feature_info();
    let _amd_features = cpuid.get_extended_feature_info();

    let has_3d_v_cache = matches!(
        epyc_model,
        Some(EPYCModel::MilanX { .. }) | Some(EPYCModel::GenoaX { .. })
    );
    let v_cache_size = if has_3d_v_cache {
        match epyc_model {
            Some(EPYCModel::MilanX { v_cache_mb, .. }) => Some(*v_cache_mb),
            Some(EPYCModel::GenoaX { v_cache_mb, .. }) => Some(*v_cache_mb),
            _ => None,
        }
    } else {
        None
    };

    let epyc_features = epyc_model.as_ref().map(|_| EPYCFeatures {
        ccd_count: match microarch {
            X86Microarch::Zen4 { .. } | X86Microarch::Zen4c { .. } => 12, // Up to 12 CCDs
            X86Microarch::Zen3 { .. } => 8,                               // Up to 8 CCDs
            _ => 4,
        },
        cores_per_ccd: 8,        // 8 cores per CCD
        l3_cache_per_ccd_mb: 32, // 32MB L3 per CCD
        dual_socket_support: true,
        quad_socket_support: matches!(
            microarch,
            X86Microarch::Zen3 { .. } | X86Microarch::Zen4 { .. }
        ),
        memory_controllers: 12, // DDR5 support in newer generations
        secure_memory_encryption: true,
        secure_encrypted_virtualization: true,
        platform_qos: true,
    });

    Ok(AMDSpecificFeatures {
        three_d_v_cache: has_3d_v_cache,
        v_cache_size_mb: v_cache_size,
        precision_boost: true, // Available on all modern AMD CPUs
        precision_boost_overdrive: true,
        smart_access_memory: matches!(
            microarch,
            X86Microarch::Zen3 { .. } | X86Microarch::Zen4 { .. } | X86Microarch::Zen4c { .. }
        ),
        smt_support: true,                    // All EPYC processors support SMT
        infinity_fabric_freq_mhz: Some(1800), // Typical IF frequency
        memory_channels: match microarch {
            X86Microarch::Zen4 { .. } | X86Microarch::Zen4c { .. } => 12, // DDR5 support
            _ => 8,                                                       // DDR4 support
        },
        max_memory_speed_mhz: match microarch {
            X86Microarch::Zen4 { .. } | X86Microarch::Zen4c { .. } => 5600, // DDR5
            _ => 3200,                                                      // DDR4
        },
        pcie_lanes: match microarch {
            X86Microarch::Zen4 { .. } | X86Microarch::Zen4c { .. } => 160, // PCIe 5.0
            X86Microarch::Zen3 { .. } => 128,                              // PCIe 4.0
            _ => 128,
        },
        epyc_features,
    })
}

// Helper functions for detection
#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_avx_capabilities(
    _feature_info: &Option<raw_cpuid::FeatureInfo>,
    extended_features: &Option<raw_cpuid::ExtendedFeatures>,
) -> AVXCapabilities {
    AVXCapabilities {
        avx256: extended_features.as_ref().is_some_and(|ef| ef.has_avx2()),
        avx512: extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512f()),
        avx512_vnni: extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512vnni()),
        avx512_bf16: false, // Would need more detailed detection
        avx512_vbmi: extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512vbmi()),
    }
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_intel_microarch<R: raw_cpuid::CpuIdReader>(cpuid: &raw_cpuid::CpuId<R>) -> X86Microarch {
    let feature_info = cpuid.get_feature_info();
    let family = feature_info.as_ref().map_or(0, |fi| fi.family_id());
    let model = feature_info.as_ref().map_or(0, |fi| fi.model_id());

    // Intel family 6 detection (most modern Intel CPUs)
    if family == 0x6 {
        match model {
            0xB7 | 0xBA | 0xBF => X86Microarch::RaptorLake,
            0x97 | 0x9A | 0x9C => X86Microarch::AlderLake,
            0x8C | 0x8D => X86Microarch::TigerLake,
            0x7D | 0x7E => X86Microarch::IceLake,
            0x4E | 0x5E | 0x8E | 0x9E => X86Microarch::Skylake,
            0x3C | 0x3F | 0x45 | 0x46 => X86Microarch::Haswell,
            _ => X86Microarch::Unknown(format!("Intel Model {:#x}", model)),
        }
    } else {
        X86Microarch::Unknown(format!("Intel Family {:#x}", family))
    }
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_intel_specific_features<R: raw_cpuid::CpuIdReader>(
    cpuid: &raw_cpuid::CpuId<R>,
) -> Result<IntelSpecificFeatures, ComputeError> {
    let extended_features = cpuid.get_extended_feature_info();

    Ok(IntelSpecificFeatures {
        turbo_boost: true,             // Available on most modern Intel CPUs
        thermal_velocity_boost: false, // Would need more detailed detection
        thread_director: false,        // Available on 12th gen+
        dl_boost: extended_features
            .as_ref()
            .is_some_and(|ef| ef.has_avx512vnni()),
        optane_support: false, // Would need platform-specific detection
        speed_select_technology: false, // Would need more detailed detection
    })
}

#[cfg(all(target_arch = "x86_64", feature = "runtime-detection"))]
fn detect_vendor_optimizations(
    vendor: &X86Vendor,
    microarch: &X86Microarch,
) -> Result<VendorOptimizations, ComputeError> {
    match vendor {
        X86Vendor::AMD => {
            let amd_opts = AMDOptimizations {
                precision_boost: true,
                precision_boost_overdrive: true,
                smart_access_memory: matches!(
                    microarch,
                    X86Microarch::Zen3 { .. }
                        | X86Microarch::Zen4 { .. }
                        | X86Microarch::Zen4c { .. }
                ),
                three_d_v_cache: false, // Would be detected per-model
                epyc_optimizations: Some(EPYCOptimizations {
                    platform_qos: true,
                    memory_bandwidth_optimization: true,
                    inter_socket_optimization: true,
                    virtualization_optimizations: VirtualizationOptimizations {
                        secure_memory_encryption: true,
                        secure_encrypted_virtualization: true,
                        sev_snp: matches!(
                            microarch,
                            X86Microarch::Zen3 { .. } | X86Microarch::Zen4 { .. }
                        ),
                        hardware_assisted_performance: true,
                    },
                    cloud_optimizations: if matches!(microarch, X86Microarch::Zen4c { .. }) {
                        Some(CloudOptimizations {
                            density_optimization: true,
                            container_optimization: true,
                            microservice_optimization: true,
                            multi_tenant_isolation: true,
                        })
                    } else {
                        None
                    },
                    enterprise_security: EnterpriseSecurityFeatures {
                        memory_guard: true,
                        silicon_root_of_trust: true,
                        secure_boot_support: true,
                        hardware_attestation: true,
                    },
                }),
                infinity_fabric_optimizations: InfinityFabricOptimizations {
                    auto_frequency_scaling: true,
                    memory_fabric_ratio_optimization: true,
                    inter_die_optimization: true,
                    cross_socket_optimization: true,
                },
                numa_optimizations: NUMAOptimizations {
                    auto_numa_balancing: true,
                    memory_locality_optimization: true,
                    thread_affinity_optimization: true,
                    cache_coherency_optimization: true,
                },
            };

            Ok(VendorOptimizations {
                intel: None,
                amd: Some(amd_opts),
                arm: None,
            })
        }
        X86Vendor::Intel => {
            let intel_opts = IntelOptimizations {
                turbo_boost: true,
                thermal_velocity_boost: matches!(
                    microarch,
                    X86Microarch::RaptorLake | X86Microarch::AlderLake
                ),
                thread_director: matches!(
                    microarch,
                    X86Microarch::RaptorLake | X86Microarch::AlderLake
                ),
                dl_boost: true,
            };

            Ok(VendorOptimizations {
                intel: Some(intel_opts),
                amd: None,
                arm: None,
            })
        }
        _ => Ok(VendorOptimizations {
            intel: None,
            amd: None,
            arm: None,
        }),
    }
}

#[cfg(all(target_arch = "aarch64", feature = "runtime-detection"))]
fn detect_aarch64_capabilities() -> Result<CPUCapabilities, ComputeError> {
    // ARM64 capability detection
    // This would use system calls or /proc/cpuinfo parsing on Linux
    // or system APIs on macOS/iOS

    let simd = SIMDCapabilities {
        x86_features: None,
        arm_features: Some(ARMSIMDFeatures {
            neon: std::arch::is_aarch64_feature_detected!("neon"),
            advanced_simd: std::arch::is_aarch64_feature_detected!("asimd"),
            sve: None, // Would require more detailed detection
            sme: None, // Would require more detailed detection
            fp16: std::arch::is_aarch64_feature_detected!("fp16"),
            bf16: false, // Would require detection
        }),
        optimal_vector_width: 16, // 128-bit NEON vectors
        max_vector_width: 32,     // Could be higher with SVE
    };

    // Detect Apple Silicon or other ARM vendors
    let (vendor, soc) = detect_arm_vendor_and_soc()?;

    let cores = CoreConfiguration {
        physical_cores: num_cpus::get_physical() as u8,
        logical_cores: num_cpus::get() as u8,
        performance_cores: 4, // Typical for ARM big.LITTLE
        efficiency_cores: 4,
        base_frequency_mhz: 2400, // Would need system-specific detection
        max_frequency_mhz: 3200,
        hyperthreading: false, // ARM typically doesn't use hyperthreading
    };

    let cache_hierarchy = CacheHierarchy {
        l1d_cache_kb: 64,
        l1i_cache_kb: 64,
        l2_cache_kb: 512,
        l3_cache_kb: 4096,
        cache_line_size: 64,
    };

    let architecture = CPUArchitecture::AArch64 {
        vendor,
        soc,
        features: ARMFeatures {
            // Would populate with ARM-specific features
        },
    };

    Ok(CPUCapabilities {
        architecture,
        simd,
        cores,
        cache_hierarchy,
        vendor_optimizations: VendorOptimizations {
            intel: None,
            amd: None,
            arm: Some(ARMOptimizations {
                apple: None, // Would populate based on detection
                qualcomm: None,
            }),
        },
    })
}

/// Detect ARM vendor and SoC information
#[cfg(target_arch = "aarch64")]
fn detect_arm_vendor_and_soc() -> Result<(ARMVendor, ARMSoC), ComputeError> {
    #[cfg(target_os = "macos")]
    {
        // Use system calls to detect Apple Silicon
        detect_apple_silicon()
    }

    #[cfg(target_os = "linux")]
    {
        // Parse /proc/cpuinfo or device tree
        detect_linux_arm_soc()
    }

    #[cfg(target_os = "android")]
    {
        // Use Android system properties
        detect_android_soc()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
    {
        Ok((ARMVendor::Other("Unknown".to_string()), ARMSoC::Unknown))
    }
}

// Platform-specific detection functions would be implemented here
// For brevity, I'll provide stub implementations

/// Detect Apple Silicon chip and capabilities
#[cfg(target_os = "macos")]
fn detect_apple_silicon() -> Result<(ARMVendor, ARMSoC), ComputeError> {
    let cpu_brand = get_macos_cpu_brand_string()?;
    let chip = parse_apple_chip(&cpu_brand);
    Ok((ARMVendor::Apple(chip), ARMSoC::Unknown))
}

/// Get CPU brand string from macOS system info
#[cfg(target_os = "macos")]
fn get_macos_cpu_brand_string() -> Result<String, ComputeError> {
    use std::process::Command;

    let output = Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.brand_string")
        .output()
        .map_err(|e| {
            ComputeError::capability_detection(
                CapabilityDetectionError::CPUDetectionFailed {
                    reason: format!("Failed to execute sysctl: {}", e),
                },
                "Apple Silicon detection",
            )
        })?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parse Apple chip information from brand string
#[cfg(target_os = "macos")]
fn parse_apple_chip(cpu_brand: &str) -> AppleChip {
    let chip_generations = [
        ("M4", ChipGeneration::M4),
        ("M3", ChipGeneration::M3),
        ("M2", ChipGeneration::M2),
        ("M1", ChipGeneration::M1),
    ];

    for (chip_name, generation) in chip_generations {
        if cpu_brand.contains(chip_name) {
            return create_apple_chip_variant(generation, cpu_brand);
        }
    }

    // Fallback to M1 if detection fails
    AppleChip::M1 {
        variant: M1Variant::M1,
        cores: CoreConfiguration::default(),
    }
}

/// Apple chip generation enum for parsing
#[cfg(target_os = "macos")]
enum ChipGeneration {
    M4,
    M3,
    M2,
    M1,
}

/// Create Apple chip variant based on generation and brand string
#[cfg(target_os = "macos")]
fn create_apple_chip_variant(generation: ChipGeneration, cpu_brand: &str) -> AppleChip {
    let cores = CoreConfiguration::default();

    match generation {
        ChipGeneration::M4 => AppleChip::M4 {
            variant: determine_m4_variant(cpu_brand),
            cores,
        },
        ChipGeneration::M3 => AppleChip::M3 {
            variant: determine_m3_variant(cpu_brand),
            cores,
        },
        ChipGeneration::M2 => AppleChip::M2 {
            variant: determine_m2_variant(cpu_brand),
            cores,
        },
        ChipGeneration::M1 => AppleChip::M1 {
            variant: determine_m1_variant(cpu_brand),
            cores,
        },
    }
}

/// Determine M4 variant from brand string
#[cfg(target_os = "macos")]
fn determine_m4_variant(cpu_brand: &str) -> M4Variant {
    if cpu_brand.contains("Max") {
        M4Variant::M4Max
    } else if cpu_brand.contains("Pro") {
        M4Variant::M4Pro
    } else {
        M4Variant::M4
    }
}

/// Determine M3 variant from brand string
#[cfg(target_os = "macos")]
fn determine_m3_variant(cpu_brand: &str) -> M3Variant {
    if cpu_brand.contains("Max") {
        M3Variant::M3Max
    } else if cpu_brand.contains("Pro") {
        M3Variant::M3Pro
    } else {
        M3Variant::M3
    }
}

/// Determine M2 variant from brand string
#[cfg(target_os = "macos")]
fn determine_m2_variant(cpu_brand: &str) -> M2Variant {
    if cpu_brand.contains("Max") {
        M2Variant::M2Max
    } else if cpu_brand.contains("Pro") {
        M2Variant::M2Pro
    } else {
        M2Variant::M2
    }
}

/// Determine M1 variant from brand string
#[cfg(target_os = "macos")]
fn determine_m1_variant(cpu_brand: &str) -> M1Variant {
    if cpu_brand.contains("Max") {
        M1Variant::M1Max
    } else if cpu_brand.contains("Pro") {
        M1Variant::M1Pro
    } else {
        M1Variant::M1
    }
}

/// Fallback detection for non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn detect_apple_silicon() -> Result<(ARMVendor, ARMSoC), ComputeError> {
    Err(ComputeError::capability_detection(
        CapabilityDetectionError::NeuralEngineDetectionFailed {
            platform: "non-macOS".to_string(),
        },
        "Apple Silicon detection",
    ))
}

#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn detect_linux_arm_soc() -> Result<(ARMVendor, ARMSoC), ComputeError> {
    // Stub implementation for Linux ARM detection
    Ok((
        ARMVendor::Other("Unknown Linux ARM".to_string()),
        ARMSoC::Unknown,
    ))
}

#[cfg(target_os = "android")]
#[allow(dead_code)]
fn detect_android_soc() -> Result<(ARMVendor, ARMSoC), ComputeError> {
    // Stub implementation for Android detection
    Ok((
        ARMVendor::Other("Unknown Android SoC".to_string()),
        ARMSoC::Unknown,
    ))
}

async fn detect_gpu_capabilities() -> Result<GPUCapabilities, ComputeError> {
    // GPU detection logic would go here
    Ok(GPUCapabilities {
        available_devices: Vec::new(),
        primary_gpu: None,
        integrated_gpu: None,
        unified_memory_support: false,
    })
}

async fn detect_neural_capabilities() -> Result<NeuralEngineCapabilities, ComputeError> {
    // Neural engine detection logic would go here
    Ok(NeuralEngineCapabilities::None)
}

async fn detect_arm_specialized_capabilities() -> Result<ARMSpecializedCapabilities, ComputeError> {
    // ARM specialized compute detection would go here
    Ok(ARMSpecializedCapabilities {
        dsp_support: None,
        isp_support: None,
        npu_support: None,
        custom_accelerators: Vec::new(),
    })
}

fn detect_memory_architecture() -> Result<MemoryArchitecture, ComputeError> {
    // Memory architecture detection would go here
    Ok(MemoryArchitecture {
        unified_memory: false,
        total_memory_gb: 16.0,       // Would detect actual value
        memory_bandwidth_gbps: 51.2, // Would detect actual value
        memory_type: MemoryType::DDR4,
        numa_topology: None,
        memory_compression: false,
    })
}

fn detect_platform_optimizations() -> Result<PlatformOptimizations, ComputeError> {
    // Platform optimization detection would go here
    Ok(PlatformOptimizations {
        macos: None,
        ios: None,
        windows: None,
        linux: None,
        android: None,
    })
}

/// x86-64 specific CPU features and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X86Features {
    /// Advanced Vector Extensions (AVX) support
    pub avx_support: AVXCapabilities,
    /// Advanced Encryption Standard (AES) instructions
    pub aes: bool,
    /// Carry-less Multiplication (PCLMULQDQ)
    pub pclmulqdq: bool,
    /// Random Number Generator (RDRAND)
    pub rdrand: bool,
    /// Secure Hash Algorithm (SHA) instructions
    pub sha: bool,
    /// Memory Protection Extensions (MPX)
    pub mpx: bool,
    /// Control-flow Enforcement Technology (CET)
    pub cet: bool,
    /// Intel Memory Protection Keys (MPK)
    pub mpk: bool,
    /// AMD-specific features
    pub amd_features: Option<AMDSpecificFeatures>,
    /// Intel-specific features
    pub intel_features: Option<IntelSpecificFeatures>,
}

/// AMD-specific CPU features for EPYC and Ryzen processors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMDSpecificFeatures {
    /// AMD 3D V-Cache technology
    pub three_d_v_cache: bool,
    /// 3D V-Cache size in MB
    pub v_cache_size_mb: Option<u32>,
    /// AMD Precision Boost technology
    pub precision_boost: bool,
    /// AMD Precision Boost Overdrive
    pub precision_boost_overdrive: bool,
    /// AMD Smart Access Memory (SAM)
    pub smart_access_memory: bool,
    /// AMD Simultaneous Multithreading (SMT)
    pub smt_support: bool,
    /// AMD Infinity Fabric frequency (MHz)
    pub infinity_fabric_freq_mhz: Option<u32>,
    /// Memory channels supported
    pub memory_channels: u8,
    /// Maximum memory speed (DDR4/DDR5) in MHz
    pub max_memory_speed_mhz: u32,
    /// PCIe lanes supported
    pub pcie_lanes: u32,
    /// EPYC-specific features
    pub epyc_features: Option<EPYCFeatures>,
}

/// EPYC processor-specific capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EPYCFeatures {
    /// Number of CPU Complex Dies (CCDs)
    pub ccd_count: u8,
    /// Cores per CCD
    pub cores_per_ccd: u8,
    /// L3 cache per CCD in MB
    pub l3_cache_per_ccd_mb: u32,
    /// Support for 2-socket configurations
    pub dual_socket_support: bool,
    /// Support for 4-socket configurations (Milan and later)
    pub quad_socket_support: bool,
    /// Number of memory controllers
    pub memory_controllers: u8,
    /// Secure Memory Encryption (SME)
    pub secure_memory_encryption: bool,
    /// Secure Encrypted Virtualization (SEV)
    pub secure_encrypted_virtualization: bool,
    /// Platform Quality of Service (QoS)
    pub platform_qos: bool,
}

/// Intel-specific CPU features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelSpecificFeatures {
    /// Intel Turbo Boost Technology
    pub turbo_boost: bool,
    /// Intel Thermal Velocity Boost
    pub thermal_velocity_boost: bool,
    /// Intel Thread Director (12th gen+)
    pub thread_director: bool,
    /// Intel Deep Learning Boost (DL Boost)
    pub dl_boost: bool,
    /// Intel Optane DC Persistent Memory support
    pub optane_support: bool,
    /// Intel Speed Select Technology (SST)
    pub speed_select_technology: bool,
}

/// AVX capabilities with detailed support levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AVXCapabilities {
    /// AVX-256 support
    pub avx256: bool,
    /// AVX-512 support
    pub avx512: bool,
    /// AVX-512 Vector Neural Network Instructions (VNNI)
    pub avx512_vnni: bool,
    /// AVX-512 BFloat16 support
    pub avx512_bf16: bool,
    /// AVX-512 Vector Byte Manipulation Instructions (VBMI)
    pub avx512_vbmi: bool,
}

/// AMD EPYC processor models with detailed specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EPYCModel {
    // Zen 5 Architecture (2024+)
    /// EPYC 9005 "Turin" series (Zen 5)
    Turin {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
    },

    // Zen 4 Architecture (Current Gen)
    /// EPYC 9004 "Genoa" series (Zen 4)
    Genoa {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
        memory_channels: u8,
        pcie_lanes: u32,
    },
    /// EPYC 9004 "Bergamo" series (Zen 4c - cloud optimized)
    Bergamo {
        model_name: String,
        cores: u8,   // Up to 128 cores
        threads: u8, // Up to 256 threads
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
        optimized_for_cloud: bool,
    },

    // Zen 3 Architecture
    /// EPYC 7004 "Genoa-X" with 3D V-Cache
    GenoaX {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        v_cache_mb: u32, // Additional 3D V-Cache
        tdp_watts: u32,
    },
    /// EPYC 7003 "Milan" series (Zen 3)
    Milan {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
        memory_support: MemorySupport,
    },
    /// EPYC 7003 "Milan-X" with 3D V-Cache
    MilanX {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        v_cache_mb: u32, // Additional 3D V-Cache
        tdp_watts: u32,
    },

    // Zen 2 Architecture
    /// EPYC 7002 "Rome" series (Zen 2)
    Rome {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
    },

    // Zen Architecture
    /// EPYC 7001 "Naples" series (Zen)
    Naples {
        model_name: String,
        cores: u8,
        threads: u8,
        base_freq_ghz: f32,
        boost_freq_ghz: f32,
        l3_cache_mb: u32,
        tdp_watts: u32,
    },

    /// Other EPYC model
    Other(String),
}

/// Memory support characteristics for EPYC processors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySupport {
    /// Memory channels per socket
    pub channels_per_socket: u8,
    /// Maximum memory capacity per socket (GB)
    pub max_capacity_gb: u32,
    /// Supported memory speeds (MHz)
    pub supported_speeds_mhz: Vec<u32>,
    /// DDR4 support
    pub ddr4_support: bool,
    /// DDR5 support
    pub ddr5_support: bool,
    /// Error Correcting Code (ECC) support
    pub ecc_support: bool,
    /// Memory interleaving capabilities
    pub memory_interleaving: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ARMFeatures {
    // ARM-specific features would be defined here
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ARMSoC {
    Unknown,
    // Other ARM SoC variants would be defined here
}

// Compile-time fallback detection
#[cfg(not(feature = "runtime-detection"))]
fn detect_compile_time_capabilities() -> Result<CPUCapabilities, ComputeError> {
    // Simplified compile-time detection based on target features
    let simd = SIMDCapabilities {
        x86_features: Some(X86SIMDFeatures {
            sse: SSESupport {
                sse: cfg!(target_feature = "sse"),
                sse2: cfg!(target_feature = "sse2"),
                sse3: cfg!(target_feature = "sse3"),
                ssse3: cfg!(target_feature = "ssse3"),
                sse4_1: cfg!(target_feature = "sse4.1"),
                sse4_2: cfg!(target_feature = "sse4.2"),
            },
            avx: AVXSupport {
                avx: cfg!(target_feature = "avx"),
                avx2: cfg!(target_feature = "avx2"),
                avx512f: cfg!(target_feature = "avx512f"),
                avx512bw: cfg!(target_feature = "avx512bw"),
                avx512cd: cfg!(target_feature = "avx512cd"),
                avx512dq: cfg!(target_feature = "avx512dq"),
                avx512vl: cfg!(target_feature = "avx512vl"),
                avx512_vnni: cfg!(target_feature = "avx512vnni"),
                avx512_bf16: cfg!(target_feature = "avx512bf16"),
                avx512_fp16: cfg!(target_feature = "avx512fp16"),
            },
            fma: cfg!(target_feature = "fma"),
            bmi: BMISupport {
                bmi1: cfg!(target_feature = "bmi1"),
                bmi2: cfg!(target_feature = "bmi2"),
            },
        }),
        arm_features: None,
        optimal_vector_width: if cfg!(target_feature = "avx512f") {
            64
        } else if cfg!(target_feature = "avx2") {
            32
        } else {
            16
        },
        max_vector_width: 64,
    };

    let cores = CoreConfiguration {
        physical_cores: num_cpus::get_physical() as u8,
        logical_cores: num_cpus::get() as u8,
        performance_cores: num_cpus::get_physical() as u8,
        efficiency_cores: 0,
        base_frequency_mhz: 2400,
        max_frequency_mhz: 3600,
        hyperthreading: num_cpus::get() > num_cpus::get_physical(),
    };

    let cache_hierarchy = CacheHierarchy {
        l1d_cache_kb: 32,
        l1i_cache_kb: 32,
        l2_cache_kb: 256,
        l3_cache_kb: 8192,
        cache_line_size: 64,
    };

    let architecture = CPUArchitecture::X86_64 {
        vendor: X86Vendor::Other("CompileTime".to_string()),
        microarchitecture: X86Microarch::Unknown("CompileTime".to_string()),
        features: X86Features {},
    };

    Ok(CPUCapabilities {
        architecture,
        simd,
        cores,
        cache_hierarchy,
        vendor_optimizations: VendorOptimizations {
            intel: None,
            amd: None,
            arm: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capability_detection() {
        let capabilities = detect_all_capabilities().await;
        assert!(capabilities.is_ok());
    }

    #[test]
    fn test_cpu_detection() {
        let cpu_caps = detect_cpu_capabilities();
        assert!(cpu_caps.is_ok());

        let caps = cpu_caps.unwrap();
        assert!(matches!(
            caps.architecture,
            CPUArchitecture::X86_64 { .. } | CPUArchitecture::AArch64 { .. }
        ));
    }
}
