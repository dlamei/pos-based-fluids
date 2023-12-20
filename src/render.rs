use glam::{Mat4, Vec3};
use std::iter;
use std::mem::size_of;
use winit::event_loop::ControlFlow;
use winit::{event::*, event_loop::EventLoop, window};

use crate::wgpu_utils as utils;
use crate::{PARTICLE_COUNT, PARTICLE_RADIUS};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
}

impl utils::VertexDescription for Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Clone, Default, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
}

impl utils::VertexDescription for Instance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as _,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

pub const fn rgba_to_u32(r: u8, g: u8, b: u8, _a: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | (b as u32) << 2
}

const SQUARE_VERT: &[Vertex] = &[
    Vertex {
        pos: [-1f32, -1f32],
    },
    Vertex { pos: [1f32, -1f32] },
    Vertex { pos: [1f32, 1f32] },
    Vertex { pos: [-1f32, 1f32] },
];

const SQUARE_INDICES: &[u16] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    aspect: f32,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Camera {
    pub fn raw(&self) -> [f32; 16] {
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let ar = self.aspect;

        let bounds = if self.aspect >= 1.0 {
            [self.left * ar, self.right * ar, self.bottom, self.top]
        } else {
            [self.left, self.right, self.bottom / ar, self.top / ar]
        };

        let proj = Mat4::orthographic_rh(bounds[0], bounds[1], bounds[2], bounds[3], 0.0, 1.0);

        (proj * view).to_cols_array()
    }
}

pub struct RenderState<'a> {
    pub context: utils::WGPUContext<'a>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub instances: Vec<Instance>,
    pub camera: Camera,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: utils::BindGroup,

    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
}

impl<'a> RenderState<'a> {
    pub async fn new(window: &'a window::Window) -> RenderState<'a> {
        let context = utils::WGPUContext::from_window(&window).await;
        let device = &context.device;
        let config = &context.config;

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let vertex = utils::ShaderModule::from(&shader)
            .entry("vs_main")
            .vertex::<Vertex>()
            .instance::<Instance>();

        let fragment = utils::ShaderModule::from(&shader)
            .entry("fs_main")
            .fragment()
            .color_target(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            });

        let vertex_buffer = utils::BufferBuilder::vertex()
            .label("Vertex Buffer")
            .data(SQUARE_VERT)
            .build(device);

        let index_buffer = utils::BufferBuilder::index()
            .label("Index Buffer")
            .data(SQUARE_INDICES)
            .build(device);

        let instances = vec![Instance::default(); PARTICLE_COUNT];

        let instance_buffer = utils::BufferBuilder::vertex()
            .label("Instance Buffer")
            .usage(wgpu::BufferUsages::COPY_DST)
            .data(instances.as_slice())
            .build(&context.device);

        let camera = Camera {
            aspect: config.width as f32 / config.height as f32,
            left: 0.0,
            right: 1.0,
            bottom: 0.0,
            top: 1.0,
        };

        let camera_buffer =
            utils::BufferBuilder::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
                .label("camera_buffer")
                .data(&[camera.raw()])
                .build(device);

        let camera_bind_group = utils::BindGroupBuilder::default()
            .label("camera_bind_group")
            .uniform_buffer(&camera_buffer, wgpu::ShaderStages::VERTEX)
            .build(device);

        let render_pipeline = utils::RenderPipelineBuilder::default()
            .vertex_stage(&vertex)
            .fragment_stage(&fragment)
            .bind(&camera_bind_group)
            .build(device);

        Self {
            context,
            render_pipeline,
            instances,
            camera,
            camera_buffer,
            camera_bind_group,
            vertex_buffer,
            index_buffer,
            instance_buffer,
        }
    }

    pub fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {
        let width = self.context.config.width as f32;
        let height = self.context.config.height as f32;
        self.camera.aspect = width / height;
        self.context.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera.raw()]),
        );
    }

    pub fn update_instances(&mut self, instances: &[Instance]) {
        self.context
            .queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.context.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (40f32 / 255f32).powf(2.2).into(),
                            g: (44f32 / 255f32).powf(2.2).into(),
                            b: (52f32 / 255f32).powf(2.2).into(),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group.group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..SQUARE_INDICES.len() as u32, 0, 0..PARTICLE_COUNT as _);
        }

        self.context.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
