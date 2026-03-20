// GPU acceleration module using wgpu
// Enables massively parallel search operations

#[cfg(feature = "gpu")]
use wgpu::{Device, Queue, Buffer, BufferUsages, ShaderModule};
use std::sync::Arc;

#[cfg(feature = "gpu")]
pub struct GpuAccelerator {
    device: Arc<Device>,
    queue: Arc<Queue>,
    compute_pipeline: wgpu::ComputePipeline,
    block_buffer: Buffer,
    query_buffer: Buffer,
    result_buffer: Buffer,
    block_count: usize,
}

#[cfg(feature = "gpu")]
impl GpuAccelerator {
    /// Initialize GPU accelerator
    pub async fn new(block_count: usize) -> Result<Self, Box<dyn std::error::Error>> {
        // Get GPU adapter
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find GPU adapter")?;

        // Get device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Microscope GPU"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Create shader module
        let shader = Self::create_shader(&device);

        // Create compute pipeline
        let compute_pipeline = Self::create_pipeline(&device, &shader);

        // Create buffers
        let block_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Block Buffer"),
            size: (block_count * 32) as u64,  // 32 bytes per block header
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let query_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Buffer"),
            size: 512 * 4,  // 512 floats for embedding
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Result Buffer"),
            size: (block_count * 8) as u64,  // index + distance per block
            usage: BufferUsages::STORAGE | BufferUsages::MAP_READ | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            block_buffer,
            query_buffer,
            result_buffer,
            block_count,
        })
    }

    /// Create compute shader
    fn create_shader(device: &Device) -> ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Microscope Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl")),
        })
    }

    /// Create compute pipeline
    fn create_pipeline(device: &Device, shader: &ShaderModule) -> wgpu::ComputePipeline {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Microscope Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: shader,
            entry_point: "main",
        })
    }

    /// Perform GPU-accelerated L2 search
    pub async fn l2_search(&self, query: &[f32; 3], k: usize) -> Vec<(usize, f32)> {
        // Write query to buffer
        self.queue.write_buffer(
            &self.query_buffer,
            0,
            bytemuck::cast_slice(query),
        );

        // Create bind group
        let bind_group_layout = self.compute_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.block_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.result_buffer.as_entire_binding(),
                },
            ],
        });

        // Create command encoder
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("L2 Search Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch with workgroups
            let workgroup_size = 64;
            let workgroups = (self.block_count + workgroup_size - 1) / workgroup_size;
            compute_pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        // Submit commands
        self.queue.submit(Some(encoder.finish()));

        // Read results
        let buffer_slice = self.result_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        rx.await.unwrap().unwrap();

        // Parse results
        let data = buffer_slice.get_mapped_range();
        let results: Vec<(usize, f32)> = bytemuck::cast_slice(&data)
            .chunks(2)
            .map(|chunk| (chunk[0] as usize, chunk[1]))
            .collect();

        // Unmap buffer
        drop(data);
        self.result_buffer.unmap();

        // Sort and return top k
        let mut sorted = results;
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        sorted.truncate(k);
        sorted
    }

    /// Perform GPU-accelerated cosine similarity
    pub async fn cosine_similarity_batch(&self, query_embedding: &[f32], embeddings: &[Vec<f32>]) -> Vec<f32> {
        // This would compute cosine similarity for all embeddings in parallel
        // Implementation would follow similar pattern to l2_search
        vec![0.0; embeddings.len()]  // Placeholder
    }
}

/// CPU fallback for systems without GPU
pub struct CpuFallback;

impl CpuFallback {
    pub fn l2_search(blocks: &[(f32, f32, f32)], query: &[f32; 3], k: usize) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = blocks
            .iter()
            .enumerate()
            .map(|(i, (x, y, z))| {
                let dx = x - query[0];
                let dy = y - query[1];
                let dz = z - query[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                (i, dist)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(k);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_fallback() {
        let blocks = vec![
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
        ];

        let query = [0.5, 0.5, 0.5];
        let results = CpuFallback::l2_search(&blocks, &query, 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);  // Closest block
    }
}