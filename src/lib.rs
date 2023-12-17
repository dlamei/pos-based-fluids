use std::iter;
use std::rc::Rc;

use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::EventLoop,
    window,
};

mod private {
    pub trait Sealed {}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    color: u32,
}

trait VertexLayout {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

impl VertexLayout for Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

const fn rgba_to_u32(r: u8, g: u8, b: u8, _a: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | (b as u32) << 2
}

const VERTICES: &[Vertex] = &[
    Vertex {
        pos: [-0.0868241, 0.49240386],
        color: rgba_to_u32(0, 0, 255, 255),
    }, // A
    Vertex {
        pos: [-0.49513406, 0.06958647],
        color: rgba_to_u32(255, 0, 0, 255),
    }, // B
    Vertex {
        pos: [-0.21918549, -0.44939706],
        color: rgba_to_u32(255, 0, 0, 255),
    }, // C
    Vertex {
        pos: [0.35966998, -0.3473291],
        color: rgba_to_u32(255, 0, 0, 255),
    }, // D
    Vertex {
        pos: [0.44147372, 0.2347359],
        color: rgba_to_u32(255, 0, 0, 255),
    }, // E
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, /* padding */ 0];

pub trait Attribute {}

struct VertexLayoutBuilder {
    array_stride: wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode,
    attributes: &'static [usize],
}

trait ShaderModuleState: private::Sealed {}

#[derive(Debug)]
struct UnInitShaderModule;
impl private::Sealed for UnInitShaderModule {}
impl ShaderModuleState for UnInitShaderModule {}

#[derive(Debug)]
struct VertexModule {
    buffers: Rc<[wgpu::VertexBufferLayout<'static>]>,
}
impl private::Sealed for VertexModule {}
impl<'a> ShaderModuleState for VertexModule {}

#[derive(Debug)]
struct FragmentModule {
    targets: Rc<[Option<wgpu::ColorTargetState>]>,
}
impl private::Sealed for FragmentModule {}
impl<'a> ShaderModuleState for FragmentModule {}

#[derive(Debug)]
struct ShaderModule<'a, S: ShaderModuleState> {
    module: &'a wgpu::ShaderModule,
    entry: &'a str,
    state: S
}

impl<'a> From<&'a wgpu::ShaderModule> for ShaderModule<'a, UnInitShaderModule> {
    fn from(module: &'a wgpu::ShaderModule) -> Self {
        Self {
            module,
            entry: "main",
            state: UnInitShaderModule,
        }
    }
}

impl<'a, S: ShaderModuleState> ShaderModule<'a, S> {
    pub fn entry(mut self, entry: &'a str) -> Self {
        self.entry = entry;
        self
    }
}

impl<'a> ShaderModule<'a, UnInitShaderModule> {

    pub fn vertex<V: VertexLayout>(self) -> ShaderModule<'a, VertexModule> {
        ShaderModule {
            module: self.module,
            entry: self.entry,
            state: VertexModule {
                buffers: Rc::from([V::desc()]),
            }
        }
    }

    pub fn fragment(self) -> ShaderModule<'a, FragmentModule> {
        ShaderModule {
            module: self.module,
            entry: self.entry,
            state: FragmentModule {
                targets: Rc::new([]),
            }
        }
    }
}

impl<'a> ShaderModule<'a, VertexModule> {
    pub fn state(&'a self) -> wgpu::VertexState<'a> {
        wgpu::VertexState {
            module: self.module,
            entry_point: self.entry,
            buffers: self.state.buffers.as_ref()
        }
    }
}

impl<'a> ShaderModule<'a, FragmentModule> {

    pub fn format(mut self, format: wgpu::TextureFormat) -> Self {
        self.state.targets = Rc::new([Some(wgpu::ColorTargetState::from(format))]);
        self
    }

    pub fn state(&'a self) -> wgpu::FragmentState<'a> {
        wgpu::FragmentState {
            module: self.module,
            entry_point: self.entry,
            targets: self.state.targets.as_ref(),
        }
    }
}

#[derive(Debug, Default)]
struct RenderPipelineBuilder<'a> {
    label: Option<&'a str>,
    vertex_module: Option<&'a ShaderModule<'a, VertexModule>>,
    fragment_module: Option<&'a ShaderModule<'a, FragmentModule>>,
}

impl<'a> RenderPipelineBuilder<'a> {
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn vertex(mut self, module: &'a ShaderModule<'a, VertexModule>) -> Self {
        self.vertex_module = Some(module);
        self
    }

    pub fn fragment(mut self, fragment: &'a ShaderModule<'a, FragmentModule>) -> Self {
        self.fragment_module = Some(fragment);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: self.label,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: self.label,
            layout: Some(&layout),
            vertex: self.vertex_module.expect("vertex_module not set").state(),
            fragment: self.fragment_module.map(|f| Some(f.state())).unwrap_or(None),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // others require Features::POLYGON_MODE_LINE
                // or Features::POLYGON_MODE_POINT
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
        })
    }
}

trait BufferState: private::Sealed {}

#[derive(Debug)]
struct UnInitBuffer;
impl private::Sealed for UnInitBuffer {}
impl BufferState for UnInitBuffer {}

#[derive(Debug)]
struct EmptyBuffer {
    size: wgpu::BufferAddress,
    mapped_at_creation: bool,
}
impl private::Sealed for EmptyBuffer {}
impl BufferState for EmptyBuffer {}

#[derive(Debug)]
struct InitBuffer<'a> {
    data: &'a [u8],
}
impl<'a> private::Sealed for InitBuffer<'a> {}
impl<'a> BufferState for InitBuffer<'a> {}

#[derive(Debug)]
struct BufferBuilder<'a, S> {
    usage: wgpu::BufferUsages,
    label: Option<&'a str>,
    state: S,
}

impl<'a> BufferBuilder<'a, UnInitBuffer> {
    pub fn vertex() -> Self {
        Self {
            usage: wgpu::BufferUsages::VERTEX,
            label: None,
            state: UnInitBuffer,
        }
    }

    pub fn index() -> Self {
        Self {
            usage: wgpu::BufferUsages::INDEX,
            label: None,
            state: UnInitBuffer,
        }
    }

}

impl<'a, S: BufferState> BufferBuilder<'a, S> {
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn data<T: bytemuck::Pod>(self, data: &[T]) -> BufferBuilder<'a, InitBuffer> {
        BufferBuilder {
            usage: self.usage,
            label: self.label,
            state: InitBuffer { data: bytemuck::cast_slice(data) },
        }
    }

    pub fn size(self, size: wgpu::BufferAddress) -> BufferBuilder<'a, EmptyBuffer> {
        BufferBuilder {
            usage: self.usage,
            label: self.label,
            state: EmptyBuffer {
                size,
                mapped_at_creation: false,
            },
        }
    }
}

impl<'a> BufferBuilder<'a, InitBuffer<'a>> {
    pub fn build(self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: self.label,
            usage: self.usage,
            contents: self.state.data,
        })
    }
}

impl<'a> BufferBuilder<'a, EmptyBuffer> {
    pub fn build(self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label,
            usage: self.usage,
            size: self.state.size,
            mapped_at_creation: self.state.mapped_at_creation,
        })
    }
}

struct WGPUContext<'guard> {
    surface: wgpu::Surface,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    marker: std::marker::PhantomData<&'guard ()>
}

impl<'guard> WGPUContext<'guard> {
    async fn from_window(window: &'guard window::Window) -> WGPUContext<'guard> {

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // The surface needs to live as long as the window that created it.
        // thats why we need the guard
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    features: wgpu::Features::all_webgpu_mask(),
                    limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            surface,
            config,
            device,
            queue,
            marker: Default::default(),
        }
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        winit::dpi::PhysicalSize {
            width: self.config.width,
            height: self.config.height,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}


struct State<'a> {
    context: WGPUContext<'a>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl<'a> State<'a> {
    async fn new(window: &'a window::Window) -> State<'a> {
        let context = WGPUContext::from_window(&window).await;

        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let vertex = ShaderModule::from(&shader)
            .entry("vs_main")
            .vertex::<Vertex>();

        let fragment = ShaderModule::from(&shader)
            .entry("fs_main")
            .fragment()
            .format(context.config.format);

        let render_pipeline = RenderPipelineBuilder::default()
            .vertex(&vertex)
            .fragment(&fragment)
            .build(&context.device);

        let vertex_buffer = BufferBuilder::vertex()
            .label("Vertex Buffer")
            .data(VERTICES)
            .build(&context.device);

        let index_buffer = BufferBuilder::index()
            .label("Index Buffer")
            .data(INDICES)
            .build(&context.device);

        Self {
            context,
            render_pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.context.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.context
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
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
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
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
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

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    state.context.resize(physical_size);
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    let mut new_size = winit::dpi::PhysicalSize::default();
                    new_size.width = (state.context.config.width as f64 * scale_factor) as u32;
                    new_size.height = (state.context.config.height as f64 * scale_factor) as u32;
                    state.context.resize(new_size);
                }
                WindowEvent::RedrawRequested => match state.render() {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.context.resize(state.context.size())
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                },
                _ => (),
            },
            _ => (),
        })
        .unwrap();
}
