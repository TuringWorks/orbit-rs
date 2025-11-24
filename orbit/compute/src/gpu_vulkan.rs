//! Vulkan GPU acceleration backend for cross-platform GPU compute
//!
//! This module provides GPU-accelerated database operations using Vulkan compute shaders.
//! Vulkan works on all major platforms (Linux, Windows, macOS) and supports NVIDIA, AMD, and Intel GPUs.

use crate::errors::{ComputeError, GPUError};
use crate::gpu_backend::{FilterOp, GpuBackendType, GpuDevice};
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        CopyBufferInfo,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, Queue,
        QueueCreateInfo, QueueFlags,
    },
    instance::{Instance, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        compute::ComputePipelineCreateInfo, layout::PipelineDescriptorSetLayoutCreateInfo,
        ComputePipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    shader::{spirv::bytes_to_words, ShaderModule},
    sync::{self, GpuFuture},
    VulkanLibrary,
};

/// Vulkan GPU device for cross-platform GPU acceleration
pub struct VulkanDevice {
    device: Arc<Device>,
    queue: Arc<Queue>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    device_name: String,
    // Cached compute pipelines for performance
    #[allow(dead_code)] // Will be used when implementing execution methods
    bfs_pipeline: Option<Arc<ComputePipeline>>,
    #[allow(dead_code)] // Will be used when implementing execution methods
    vector_similarity_pipeline: Option<Arc<ComputePipeline>>,
}

impl VulkanDevice {
    /// Create a new Vulkan device
    pub fn new() -> Result<Self, ComputeError> {
        // Load Vulkan library
        let library = VulkanLibrary::new().map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to load Vulkan library: {}", e),
            })
        })?;

        // Create Vulkan instance
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                ..Default::default()
            },
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create Vulkan instance: {}", e),
            })
        })?;

        // Select best physical device (prefer discrete GPU)
        let physical_device = instance
            .enumerate_physical_devices()
            .map_err(|_e| {
                ComputeError::gpu(GPUError::DeviceNotFound { device_id: 0 })
            })?
            .filter(|p| {
                // Must support compute queue
                p.queue_family_properties()
                    .iter()
                    .any(|q| q.queue_flags.intersects(QueueFlags::COMPUTE))
            })
            .min_by_key(|p| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .ok_or_else(|| {
                ComputeError::gpu(GPUError::DeviceNotFound { device_id: 0 })
            })?;

        let device_name = physical_device.properties().device_name.clone();

        // Find compute queue family
        let queue_family_index = physical_device
            .queue_family_properties()
            .iter()
            .position(|q| q.queue_flags.intersects(QueueFlags::COMPUTE))
            .ok_or_else(|| {
                ComputeError::gpu(GPUError::APIInitializationFailed {
                    api: "Vulkan".to_string(),
                    error: "No compute queue family found".to_string(),
                })
            })? as u32;

        // Create logical device and queue
        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                enabled_extensions: DeviceExtensions::empty(),
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create device: {}", e),
            })
        })?;

        let queue = queues.next().ok_or_else(|| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: "Failed to get queue".to_string(),
            })
        })?;

        // Create allocators
        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let command_buffer_allocator =
            Arc::new(StandardCommandBufferAllocator::new(device.clone(), Default::default()));
        let descriptor_set_allocator =
            Arc::new(StandardDescriptorSetAllocator::new(device.clone(), Default::default()));

        tracing::info!("Initialized Vulkan device: {}", device_name);

        // Load and compile shaders (defer pipeline creation until first use)
        // Note: SPIR-V shaders need to be compiled from GLSL using glslc
        // For now, we'll create pipelines lazily when needed
        let bfs_pipeline = None; // Will be created on first use
        let vector_similarity_pipeline = None; // Will be created on first use

        Ok(Self {
            device,
            queue,
            memory_allocator,
            command_buffer_allocator,
            descriptor_set_allocator,
            device_name,
            bfs_pipeline,
            vector_similarity_pipeline,
        })
    }

    /// Load a SPIR-V shader module from bytes
    /// Note: Shader must be pre-compiled to SPIR-V using glslc
    fn load_shader_module(&self, shader_code: &[u8]) -> Result<Arc<ShaderModule>, ComputeError> {
        let words = bytes_to_words(shader_code).map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to convert shader bytes to words: {}", e),
            })
        })?;
        
        unsafe {
            ShaderModule::new(self.device.clone(), words)
                .map_err(|e| {
                    ComputeError::gpu(GPUError::APIInitializationFailed {
                        api: "Vulkan".to_string(),
                        error: format!("Failed to load shader: {}", e),
                    })
                })
        }
    }

    /// Create or get cached BFS compute pipeline
    fn get_bfs_pipeline(&mut self) -> Result<Arc<ComputePipeline>, ComputeError> {
        if let Some(ref pipeline) = self.bfs_pipeline {
            return Ok(pipeline.clone());
        }

        // Try to load pre-compiled SPIR-V shader
        // Note: Shader must be compiled with: glslc graph_bfs.comp -o graph_bfs.spv
        let shader_code = include_bytes!("shaders/vulkan/graph_bfs.spv");
        let shader_module = self.load_shader_module(shader_code)?;
        
        // Get entry point
        let entry_point = shader_module.entry_point("main").ok_or_else(|| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: "BFS shader entry point 'main' not found".to_string(),
            })
        })?;
        
        // Create shader stage
        let stage = PipelineShaderStageCreateInfo::new(entry_point);
        
        // Create pipeline layout from shader reflection
        let descriptor_set_layouts = PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage]);
        let layout = PipelineLayout::new(
            self.device.clone(),
            descriptor_set_layouts,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create pipeline layout: {}", e),
            })
        })?;
        
        // Create compute pipeline using stage_layout
        let pipeline = ComputePipeline::new(
            self.device.clone(),
            ComputePipelineCreateInfo::stage_layout(stage, layout),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create BFS pipeline: {}", e),
            })
        })?;
        
        // Cache the pipeline
        self.bfs_pipeline = Some(pipeline.clone());
        Ok(pipeline)
    }

    /// Create or get cached vector similarity compute pipeline
    fn get_vector_similarity_pipeline(&mut self) -> Result<Arc<ComputePipeline>, ComputeError> {
        if let Some(ref pipeline) = self.vector_similarity_pipeline {
            return Ok(pipeline.clone());
        }

        // Try to load pre-compiled SPIR-V shader
        let shader_code = include_bytes!("shaders/vulkan/vector_similarity.spv");
        let shader_module = self.load_shader_module(shader_code)?;
        
        // Get entry point
        let entry_point = shader_module.entry_point("main").ok_or_else(|| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: "Vector similarity shader entry point 'main' not found".to_string(),
            })
        })?;
        
        // Create shader stage
        let stage = PipelineShaderStageCreateInfo::new(entry_point);
        
        // Create pipeline layout from shader reflection
        let descriptor_set_layouts = PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage]);
        let layout = PipelineLayout::new(
            self.device.clone(),
            descriptor_set_layouts,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create pipeline layout: {}", e),
            })
        })?;
        
        // Create compute pipeline using stage_layout
        let pipeline = ComputePipeline::new(
            self.device.clone(),
            ComputePipelineCreateInfo::stage_layout(stage, layout),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::APIInitializationFailed {
                api: "Vulkan".to_string(),
                error: format!("Failed to create vector similarity pipeline: {}", e),
            })
        })?;
        
        // Cache the pipeline
        self.vector_similarity_pipeline = Some(pipeline.clone());
        Ok(pipeline)
    }

    /// Execute Vulkan BFS level expansion
    pub fn execute_bfs_level_expansion(
        &mut self,
        edge_array: &[u32],
        edge_offset: &[u32],
        current_level: &[u32],
        visited: &mut [u32],
        next_level: &mut [u32],
        next_level_size: &mut u32,
        current_level_size: u32,
        max_nodes: u32,
    ) -> Result<(), ComputeError> {
        // Get or create BFS pipeline
        let pipeline = self.get_bfs_pipeline()?;

        // Create buffers
        let edge_array_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            edge_array.iter().copied(),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: edge_array.len() * std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let edge_offset_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            edge_offset.iter().copied(),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: edge_offset.len() * std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let current_level_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            current_level.iter().copied(),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: current_level.len() * std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let visited_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            visited.iter().copied(),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: visited.len() * std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let next_level_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            (0..max_nodes as usize).map(|_| 0u32),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: max_nodes as usize * std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let next_level_size_buffer = Buffer::from_data(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            *next_level_size,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: std::mem::size_of::<u32>(),
                available_bytes: 0,
            })
        })?;

        let params = [current_level_size, max_nodes];
        let params_buffer = Buffer::from_data(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            params,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: std::mem::size_of::<[u32; 2]>(),
                available_bytes: 0,
            })
        })?;

        // Create descriptor set
        let layout = &pipeline.layout().set_layouts()[0];
        let descriptor_set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            layout.clone(),
            [
                WriteDescriptorSet::buffer(0, edge_array_buffer.clone()),
                WriteDescriptorSet::buffer(1, edge_offset_buffer.clone()),
                WriteDescriptorSet::buffer(2, current_level_buffer.clone()),
                WriteDescriptorSet::buffer(3, visited_buffer.clone()),
                WriteDescriptorSet::buffer(4, next_level_buffer.clone()),
                WriteDescriptorSet::buffer(5, next_level_size_buffer.clone()),
                WriteDescriptorSet::buffer(6, params_buffer.clone()),
            ],
            [],
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::KernelLaunchFailed {
                kernel_name: "bfs_level_expansion".to_string(),
                error: format!("Failed to create descriptor set: {}", e),
            })
        })?;

        // Build command buffer
        let workgroup_count = ((current_level_size as u32) + 255) / 256;
        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to create command buffer: {}", e),
            })
        })?;

        builder
            .bind_pipeline_compute(pipeline.clone())
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to bind pipeline: {}", e),
                })
            })?
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                pipeline.layout().clone(),
                0,
                descriptor_set,
            )
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to bind descriptor sets: {}", e),
                })
            })?
            .dispatch([workgroup_count, 1, 1])
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to dispatch: {}", e),
                })
            })?;

        let command_buffer = builder.build().map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to build command buffer: {}", e),
            })
        })?;

        // Execute and wait
        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to execute: {}", e),
                })
            })?
            .then_signal_fence_and_flush()
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to flush: {}", e),
                })
            })?;

        future.wait(None).map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to wait: {}", e),
            })
        })?;

        // Read results back
        let visited_content = visited_buffer.read().map_err(|_e| {
            ComputeError::Execution {
                source: crate::errors::ExecutionError::DataTransferError {
                    source: "GPU".to_string(),
                    destination: "CPU".to_string(),
                },
                compute_unit: Some("Vulkan".to_string()),
            }
        })?;
        visited.copy_from_slice(&visited_content);

        let next_level_content = next_level_buffer.read().map_err(|_e| {
            ComputeError::Execution {
                source: crate::errors::ExecutionError::DataTransferError {
                    source: "GPU".to_string(),
                    destination: "CPU".to_string(),
                },
                compute_unit: Some("Vulkan".to_string()),
            }
        })?;
        let next_level_slice = &next_level_content[..next_level.len().min(max_nodes as usize)];
        next_level[..next_level_slice.len()].copy_from_slice(next_level_slice);

        let size_content = next_level_size_buffer.read().map_err(|_e| {
            ComputeError::Execution {
                source: crate::errors::ExecutionError::DataTransferError {
                    source: "GPU".to_string(),
                    destination: "CPU".to_string(),
                },
                compute_unit: Some("Vulkan".to_string()),
            }
        })?;
        *next_level_size = size_content;

        Ok(())
    }

    /// Execute a compute shader with input/output buffers
    fn execute_compute_i32(
        &self,
        pipeline: &Arc<ComputePipeline>,
        input_data: &[i32],
        value: i32,
        output_size: usize,
    ) -> Result<Vec<i32>, ComputeError> {
        // Create input buffer
        let input_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            input_data.iter().copied(),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: input_data.len() * std::mem::size_of::<i32>(),
                available_bytes: 0, // Vulkan doesn't provide this info directly
            })
        })?;

        // Create value buffer (single i32)
        let value_buffer = Buffer::from_data(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            value,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: std::mem::size_of::<i32>(),
                available_bytes: 0,
            })
        })?;

        // Create output buffer
        let output_buffer = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            (0..output_size).map(|_| 0i32),
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::MemoryAllocationFailed {
                requested_bytes: output_size * std::mem::size_of::<i32>(),
                available_bytes: 0,
            })
        })?;

        // Create descriptor set
        let layout = &pipeline.layout().set_layouts()[0];
        let descriptor_set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            layout.clone(),
            [
                WriteDescriptorSet::buffer(0, input_buffer.clone()),
                WriteDescriptorSet::buffer(1, value_buffer.clone()),
                WriteDescriptorSet::buffer(2, output_buffer.clone()),
            ],
            [],
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::KernelLaunchFailed {
                kernel_name: "compute".to_string(),
                error: format!("Failed to create descriptor set: {}", e),
            })
        })?;

        // Build command buffer
        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to create command buffer: {}", e),
            })
        })?;

        // Calculate dispatch size (256 threads per workgroup)
        let workgroup_count = ((output_size as u32) + 255) / 256;

        builder
            .bind_pipeline_compute(pipeline.clone())
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to bind pipeline: {}", e),
                })
            })?
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                pipeline.layout().clone(),
                0,
                descriptor_set,
            )
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to bind descriptor sets: {}", e),
                })
            })?
            .dispatch([workgroup_count, 1, 1])
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to dispatch: {}", e),
                })
            })?;

        let command_buffer = builder.build().map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to build command buffer: {}", e),
            })
        })?;

        // Execute and wait
        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to execute: {}", e),
                })
            })?
            .then_signal_fence_and_flush()
            .map_err(|e| {
                ComputeError::gpu(GPUError::CommandBufferFailed {
                    error: format!("Failed to flush: {}", e),
                })
            })?;

        future.wait(None).map_err(|e| {
            ComputeError::gpu(GPUError::CommandBufferFailed {
                error: format!("Failed to wait: {}", e),
            })
        })?;

        // Read result
        let output_content = output_buffer.read().map_err(|_e| {
            ComputeError::Execution {
                source: crate::errors::ExecutionError::DataTransferError {
                    source: "GPU".to_string(),
                    destination: "CPU".to_string(),
                },
                compute_unit: Some("Vulkan".to_string()),
            }
        })?;

        Ok(output_content.to_vec())
    }
}

impl GpuDevice for VulkanDevice {
    fn backend_type(&self) -> GpuBackendType {
        GpuBackendType::Vulkan
    }

    fn device_name(&self) -> String {
        self.device_name.clone()
    }

    fn is_available(&self) -> bool {
        true
    }

    fn execute_filter_i32(
        &self,
        data: &[i32],
        value: i32,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        // For now, implement on CPU - will add compute shaders next
        let result: Vec<i32> = data
            .iter()
            .map(|&x| {
                let matches = match operation {
                    FilterOp::Equal => x == value,
                    FilterOp::GreaterThan => x > value,
                    FilterOp::GreaterOrEqual => x >= value,
                    FilterOp::LessThan => x < value,
                    FilterOp::LessOrEqual => x <= value,
                    FilterOp::NotEqual => x != value,
                };
                if matches {
                    1
                } else {
                    0
                }
            })
            .collect();

        Ok(result)
    }

    fn execute_filter_i64(
        &self,
        data: &[i64],
        value: i64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        // CPU implementation for now
        let result: Vec<i32> = data
            .iter()
            .map(|&x| {
                let matches = match operation {
                    FilterOp::Equal => x == value,
                    FilterOp::GreaterThan => x > value,
                    FilterOp::GreaterOrEqual => x >= value,
                    FilterOp::LessThan => x < value,
                    FilterOp::LessOrEqual => x <= value,
                    FilterOp::NotEqual => x != value,
                };
                if matches {
                    1
                } else {
                    0
                }
            })
            .collect();

        Ok(result)
    }

    fn execute_filter_f64(
        &self,
        data: &[f64],
        value: f64,
        operation: FilterOp,
    ) -> Result<Vec<i32>, ComputeError> {
        // CPU implementation for now
        let result: Vec<i32> = data
            .iter()
            .map(|&x| {
                let matches = match operation {
                    FilterOp::Equal => (x - value).abs() < f64::EPSILON,
                    FilterOp::GreaterThan => x > value,
                    FilterOp::GreaterOrEqual => x >= value,
                    FilterOp::LessThan => x < value,
                    FilterOp::LessOrEqual => x <= value,
                    FilterOp::NotEqual => (x - value).abs() >= f64::EPSILON,
                };
                if matches {
                    1
                } else {
                    0
                }
            })
            .collect();

        Ok(result)
    }

    fn bitmap_and(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::Execution {
                source: crate::errors::ExecutionError::InvalidKernelParameters {
                    parameter: "mask lengths".to_string(),
                    value: format!("{} vs {}", mask_a.len(), mask_b.len()),
                },
                compute_unit: Some("Vulkan".to_string()),
            });
        }

        Ok(mask_a
            .iter()
            .zip(mask_b.iter())
            .map(|(&a, &b)| if a != 0 && b != 0 { 1 } else { 0 })
            .collect())
    }

    fn bitmap_or(&self, mask_a: &[i32], mask_b: &[i32]) -> Result<Vec<i32>, ComputeError> {
        if mask_a.len() != mask_b.len() {
            return Err(ComputeError::Execution {
                source: crate::errors::ExecutionError::InvalidKernelParameters {
                    parameter: "mask lengths".to_string(),
                    value: format!("{} vs {}", mask_a.len(), mask_b.len()),
                },
                compute_unit: Some("Vulkan".to_string()),
            });
        }

        Ok(mask_a
            .iter()
            .zip(mask_b.iter())
            .map(|(&a, &b)| if a != 0 || b != 0 { 1 } else { 0 })
            .collect())
    }

    fn bitmap_not(&self, mask: &[i32]) -> Result<Vec<i32>, ComputeError> {
        Ok(mask.iter().map(|&x| if x == 0 { 1 } else { 0 }).collect())
    }

    fn aggregate_sum_i32(&self, data: &[i32]) -> Result<i64, ComputeError> {
        Ok(data.iter().map(|&x| x as i64).sum())
    }

    fn aggregate_count(&self, mask: &[i32]) -> Result<usize, ComputeError> {
        Ok(mask.iter().filter(|&&x| x != 0).count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulkan_device_creation() {
        match VulkanDevice::new() {
            Ok(device) => {
                println!("Created Vulkan device: {}", device.device_name());
                assert_eq!(device.backend_type(), GpuBackendType::Vulkan);
                assert!(device.is_available());
            }
            Err(e) => {
                println!("Vulkan not available (this is ok): {}", e);
            }
        }
    }

    #[test]
    fn test_vulkan_filter_i32_eq() {
        let device = match VulkanDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping test - Vulkan not available");
                return;
            }
        };

        let data: Vec<i32> = (0..1000).collect();
        let result = device
            .execute_filter_i32(&data, 500, FilterOp::Equal)
            .unwrap();

        assert_eq!(result.len(), 1000);
        assert_eq!(result[500], 1);
        assert_eq!(result[499], 0);
        assert_eq!(result[501], 0);
    }

    #[test]
    fn test_vulkan_filter_i32_gt() {
        let device = match VulkanDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping test - Vulkan not available");
                return;
            }
        };

        let data: Vec<i32> = (0..100).collect();
        let result = device
            .execute_filter_i32(&data, 50, FilterOp::GreaterThan)
            .unwrap();

        let count: i32 = result.iter().sum();
        assert_eq!(count, 49); // 51, 52, ..., 99 = 49 values
    }

    #[test]
    fn test_vulkan_bitmap_and() {
        let device = match VulkanDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping test - Vulkan not available");
                return;
            }
        };

        let mask_a = vec![1, 1, 0, 0, 1];
        let mask_b = vec![1, 0, 1, 0, 1];
        let result = device.bitmap_and(&mask_a, &mask_b).unwrap();

        assert_eq!(result, vec![1, 0, 0, 0, 1]);
    }

    #[test]
    fn test_vulkan_aggregate_sum() {
        let device = match VulkanDevice::new() {
            Ok(d) => d,
            Err(_) => {
                println!("Skipping test - Vulkan not available");
                return;
            }
        };

        let data: Vec<i32> = (1..=100).collect();
        let sum = device.aggregate_sum_i32(&data).unwrap();

        assert_eq!(sum, 5050); // Sum of 1 to 100
    }
}
