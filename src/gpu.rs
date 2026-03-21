//! GPU-accelerated 4D soft search using wgpu compute shaders.
//!
//! Uploads only (x, y, z, zoom) = 16 bytes per block to GPU.
//! Dispatches a single compute pass over all blocks, reads back f32 distances,
//! then does top-k selection on CPU.

#[cfg(feature = "gpu")]
use wgpu::{Device, Queue, Buffer, BufferUsages};
#[cfg(feature = "gpu")]
use std::sync::Arc;

#[cfg(feature = "gpu")]
use crate::MicroscopeReader;

/// Query uniform: 5 floats = 20 bytes, padded to 32 for alignment
#[cfg(feature = "gpu")]
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuQuery {
    x: f32,
    y: f32,
    z: f32,
    qz: f32,
    zw: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

#[cfg(feature = "gpu")]
pub struct GpuAccelerator {
    device: Arc<Device>,
    queue: Arc<Queue>,
    compute_pipeline: wgpu::ComputePipeline,
    positions_buffer: Buffer,
    query_buffer: Buffer,
    distances_buffer: Buffer,    // STORAGE | COPY_SRC
    staging_buffer: Buffer,      // MAP_READ | COPY_DST
    block_count: usize,
}

#[cfg(feature = "gpu")]
impl GpuAccelerator {
    /// Initialize GPU accelerator, uploading block positions from the reader.
    pub fn new(reader: &MicroscopeReader) -> Result<Self, Box<dyn std::error::Error>> {
        pollster::block_on(Self::new_async(reader))
    }

    async fn new_async(reader: &MicroscopeReader) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No GPU adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Microscope GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);
        let block_count = reader.block_count;

        // Extract positions (x, y, z, zoom) from reader
        let mut positions: Vec<[f32; 4]> = Vec::with_capacity(block_count);
        for i in 0..block_count {
            let h = reader.header(i);
            positions.push([h.x, h.y, h.z, h.zoom]);
        }

        let pos_bytes: &[u8] = bytemuck::cast_slice(&positions);

        // Create buffers
        let positions_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Positions"),
            size: pos_bytes.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&positions_buffer, 0, pos_bytes);

        let query_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query"),
            size: std::mem::size_of::<GpuQuery>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dist_size = (block_count * std::mem::size_of::<f32>()) as u64;
        let distances_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Distances"),
            size: dist_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging"),
            size: dist_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shader + pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Microscope Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute BGL"),
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
                        ty: wgpu::BufferBindingType::Uniform,
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
            label: Some("Compute PL"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("L2 4D Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "l2_4d",
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            positions_buffer,
            query_buffer,
            distances_buffer,
            staging_buffer,
            block_count,
        })
    }

    /// GPU-accelerated 4D L2 search. Returns top-k (distance², index) pairs.
    pub fn l2_search_4d(&self, x: f32, y: f32, z: f32, zoom: u8, zw: f32, k: usize) -> Vec<(f32, usize)> {
        pollster::block_on(self.l2_search_4d_async(x, y, z, zoom, zw, k))
    }

    async fn l2_search_4d_async(&self, x: f32, y: f32, z: f32, zoom: u8, zw: f32, k: usize) -> Vec<(f32, usize)> {
        let qz = zoom as f32 / 8.0;
        let query = GpuQuery { x, y, z, qz, zw, _pad0: 0.0, _pad1: 0.0, _pad2: 0.0 };
        self.queue.write_buffer(&self.query_buffer, 0, bytemuck::bytes_of(&query));

        // Build bind group
        let bind_group_layout = self.compute_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute BG"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.positions_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.query_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.distances_buffer.as_entire_binding() },
            ],
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("L2 4D Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = ((self.block_count + 63) / 64) as u32;
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy distances → staging
        let byte_size = (self.block_count * std::mem::size_of::<f32>()) as u64;
        encoder.copy_buffer_to_buffer(&self.distances_buffer, 0, &self.staging_buffer, 0, byte_size);

        self.queue.submit(Some(encoder.finish()));

        // Map staging buffer and read back
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.await.unwrap().unwrap();

        let mapped = buffer_slice.get_mapped_range();
        let distances: &[f32] = bytemuck::cast_slice(&mapped);

        // Top-k on CPU
        let mut results: Vec<(f32, usize)> = distances.iter().copied().enumerate()
            .map(|(i, d)| (d, i))
            .collect();

        drop(mapped);
        self.staging_buffer.unmap();

        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
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
                (i, dx * dx + dy * dy + dz * dz)
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
        assert_eq!(results[0].0, 0); // Closest block
    }
}
