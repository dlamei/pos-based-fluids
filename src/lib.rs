pub mod wgpu_utils;

use glam::{Mat4, Vec3};
use std::iter;
use winit::{event::*, event_loop::EventLoop, window};
use winit::event_loop::ControlFlow;

use crate::utils::BindGroupBuilder;
use wgpu_utils as utils;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
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
#[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    pos: [f32; 2],
    scale: f32,
    color: u32,
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
                    offset: mem::size_of::<[f32; 2]>() as _,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as _,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

const fn rgba_to_u32(r: u8, g: u8, b: u8, _a: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | (b as u32) << 2
}

const SQUARE_VERT: &[Vertex] = &[
    Vertex { pos: [-1f32, -1f32] },
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

struct State<'a> {
    context: utils::WGPUContext<'a>,
    render_pipeline: wgpu::RenderPipeline,
    instances: Vec<Instance>,
    camera: Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: utils::BindGroup,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
}

impl<'a> State<'a> {
    async fn new(window: &'a window::Window) -> State<'a> {
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

        let instances = vec![
            Instance {
                pos: [0.0, 0.0],
                scale: 1.0,
                color: rgba_to_u32(255, 0, 0, 255),
            },
            Instance {
                pos: [0.0, 0.3],
                scale: 0.5,
                color: rgba_to_u32(0, 0, 255, 255),
            }
        ];

        let instance_buffer = utils::BufferBuilder::vertex()
            .label("Instance Buffer")
            .data(&instances)
            .build(&context.device);

        let camera = Camera {
            aspect: config.width as f32 / config.height as f32,
            left: -1.0,
            right: 1.0,
            bottom: 0.0,
            top: 2.0,
        };

        let camera_buffer =
            utils::BufferBuilder::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
                .label("camera_buffer")
                .data(&[camera.raw()])
                .build(device);

        let camera_bind_group = BindGroupBuilder::default()
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

    fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        let width = self.context.config.width as f32;
        let height = self.context.config.height as f32;
        self.camera.aspect = width / height;
        self.context.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera.raw()]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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

            render_pass.draw_indexed(
                0..SQUARE_INDICES.len() as u32,
                0,
                0..self.instances.len() as _,
            );
        }

        self.context.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    let event_loop = EventLoop::new().expect("could not create event loop");
    let window = window::WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(&window).await;

    let mut prev_time = std::time::Instant::now();
    let mut time_sum = std::time::Duration::new(0, 0);
    let mut frame_count = 1u64;

    event_loop
        .run(|event, elwt| {
            let elapsed = prev_time.elapsed();
            prev_time = std::time::Instant::now();
            time_sum += elapsed;
            let time_step = 100;
            if (frame_count % time_step == 0) {
                println!("frame_time: {} ms", time_sum.as_millis() as f64 / (time_step as f64));
                time_sum = std::time::Duration::new(0, 0);
            }
            frame_count += 1;

            match event {
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::WindowEvent { event, window_id } if window_id == state.context.window_id => {
                    if state.input(&event) {
                        return;
                    }

                    match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WindowEvent::Resized(physical_size) => {
                            state.context.resize(physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            let mut new_size = winit::dpi::PhysicalSize::default();
                            new_size.width = (state.context.config.width as f64 * scale_factor) as u32;
                            new_size.height =
                                (state.context.config.height as f64 * scale_factor) as u32;
                            state.context.resize(new_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(()) => {}
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    state.context.resize(state.context.size())
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        })
        .unwrap();
}
