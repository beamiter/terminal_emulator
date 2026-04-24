use super::instance::{CellInstance, GridUniforms};
use wgpu::util::DeviceExt;

/// Holds the wgpu render pipeline, bind group layout, and per-frame buffers.
pub struct GridPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    background_uniform_buffer: wgpu::Buffer,
    foreground_uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    pub background_bind_group: wgpu::BindGroup,
    pub foreground_bind_group: wgpu::BindGroup,
}

impl GridPipeline {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        atlas_view: &wgpu::TextureView,
        atlas_sampler: &wgpu::Sampler,
    ) -> Self {
        let shader_src = include_str!("shader.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grid_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grid_bind_group_layout"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grid_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let fs_entry = if target_format.is_srgb() {
            eprintln!("[GPU] sRGB framebuffer {:?}, using fs_main_linear", target_format);
            "fs_main_linear"
        } else {
            "fs_main_gamma"
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[CellInstance::vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some(fs_entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let background_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("grid_background_uniforms"),
                contents: bytemuck::bytes_of(&GridUniforms::zeroed()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let foreground_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("grid_foreground_uniforms"),
                contents: bytemuck::bytes_of(&GridUniforms::zeroed()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Initial instance buffer capacity for ~200x50 grid
        let initial_capacity = 16384;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid_instances"),
            size: (initial_capacity * std::mem::size_of::<CellInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let background_bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            &background_uniform_buffer,
            atlas_view,
            atlas_sampler,
        );

        let foreground_bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            &foreground_uniform_buffer,
            atlas_view,
            atlas_sampler,
        );

        GridPipeline {
            pipeline,
            bind_group_layout,
            background_uniform_buffer,
            foreground_uniform_buffer,
            instance_buffer,
            instance_capacity: initial_capacity,
            background_bind_group,
            foreground_bind_group,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        atlas_view: &wgpu::TextureView,
        atlas_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        })
    }

    /// Rebuild the bind group when atlas texture is recreated (e.g. after resize).
    pub fn rebuild_bind_group(
        &mut self,
        device: &wgpu::Device,
        atlas_view: &wgpu::TextureView,
        atlas_sampler: &wgpu::Sampler,
    ) {
        self.background_bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &self.background_uniform_buffer,
            atlas_view,
            atlas_sampler,
        );
        self.foreground_bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &self.foreground_uniform_buffer,
            atlas_view,
            atlas_sampler,
        );
    }

    /// Upload uniform data.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &GridUniforms) {
        let uniform_buffer = if uniforms.render_phase < 0.5 {
            &self.background_uniform_buffer
        } else {
            &self.foreground_uniform_buffer
        };

        queue.write_buffer(uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Upload instance data, resizing the buffer if needed.
    pub fn update_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[CellInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        if instances.len() > self.instance_capacity {
            let new_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grid_instances"),
                size: (new_capacity * std::mem::size_of::<CellInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_capacity;
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }
}

use bytemuck::Zeroable;
