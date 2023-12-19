use wgpu::util::DeviceExt;
use winit::window;
use winit::window::WindowId;

mod private {
    pub trait Sealed {}
}

pub trait VertexDescription {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

pub trait ShaderModuleState: private::Sealed {}

#[derive(Debug)]
pub struct UnInitShaderModule;
impl private::Sealed for UnInitShaderModule {}
impl ShaderModuleState for UnInitShaderModule {}

#[derive(Debug)]
pub struct VertexModule {
    buffers: Vec<wgpu::VertexBufferLayout<'static>>,
}
impl private::Sealed for VertexModule {}
impl<'a> ShaderModuleState for VertexModule {}

#[derive(Debug)]
pub struct FragmentModule {
    targets: Vec<Option<wgpu::ColorTargetState>>,
}
impl private::Sealed for FragmentModule {}
impl<'a> ShaderModuleState for FragmentModule {}

#[derive(Debug)]
pub struct ShaderModule<'a, S: ShaderModuleState> {
    module: &'a wgpu::ShaderModule,
    entry: &'a str,
    state: S,
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
    pub fn vertex<V: VertexDescription>(self) -> ShaderModule<'a, VertexModule> {
        ShaderModule {
            module: self.module,
            entry: self.entry,
            state: VertexModule {
                buffers: vec![V::desc()],
            },
        }
    }

    pub fn fragment(self) -> ShaderModule<'a, FragmentModule> {
        ShaderModule {
            module: self.module,
            entry: self.entry,
            state: FragmentModule { targets: vec![] },
        }
    }
}

impl<'a> ShaderModule<'a, VertexModule> {
    pub fn state(&'a self) -> wgpu::VertexState<'a> {
        wgpu::VertexState {
            module: self.module,
            entry_point: self.entry,
            buffers: self.state.buffers.as_slice(),
        }
    }

    pub fn instance<V: VertexDescription>(self) -> ShaderModule<'a, VertexModule> {
        self.append::<V>()
    }

    pub fn append<V: VertexDescription>(mut self) -> ShaderModule<'a, VertexModule> {
        self.state.buffers.push(V::desc());
        self
    }
}

impl<'a> ShaderModule<'a, FragmentModule> {
    pub fn format(mut self, format: wgpu::TextureFormat) -> Self {
        self.state.targets = vec![Some(wgpu::ColorTargetState::from(format))];
        self
    }

    pub fn color_target(mut self, target: wgpu::ColorTargetState) -> Self {
        self.state.targets.push(Some(target));
        self
    }

    pub fn state(&'a self) -> wgpu::FragmentState<'a> {
        wgpu::FragmentState {
            module: self.module,
            entry_point: self.entry,
            targets: self.state.targets.as_slice(),
        }
    }
}

#[derive(Debug, Default)]
pub struct RenderPipelineBuilder<'a> {
    label: Option<&'a str>,
    vertex_module: Option<&'a ShaderModule<'a, VertexModule>>,
    fragment_module: Option<&'a ShaderModule<'a, FragmentModule>>,
    bind_group_layouts: Vec<&'a wgpu::BindGroupLayout>,
}

impl<'a> RenderPipelineBuilder<'a> {
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn vertex_stage(mut self, module: &'a ShaderModule<'a, VertexModule>) -> Self {
        self.vertex_module = Some(module);
        self
    }

    pub fn fragment_stage(mut self, fragment: &'a ShaderModule<'a, FragmentModule>) -> Self {
        self.fragment_module = Some(fragment);
        self
    }

    pub fn bind(mut self, bind_group: &'a BindGroup) -> Self {
        self.bind_group_layouts.push(&bind_group.layout);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: self.label,
            bind_group_layouts: self.bind_group_layouts.as_slice(),
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: self.label,
            layout: Some(&layout),
            vertex: self.vertex_module.expect("vertex_module not set").state(),
            fragment: self
                .fragment_module
                .map(|f| Some(f.state()))
                .unwrap_or(None),
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

pub trait BufferState: private::Sealed {}

#[derive(Debug)]
pub struct UnInitBuffer;
impl private::Sealed for UnInitBuffer {}
impl BufferState for UnInitBuffer {}

#[derive(Debug)]
pub struct EmptyBuffer {
    size: wgpu::BufferAddress,
    mapped_at_creation: bool,
}
impl private::Sealed for EmptyBuffer {}
impl BufferState for EmptyBuffer {}

#[derive(Debug)]
pub struct InitBuffer<'a> {
    data: &'a [u8],
}
impl<'a> private::Sealed for InitBuffer<'a> {}
impl<'a> BufferState for InitBuffer<'a> {}

#[derive(Debug)]
pub struct BufferBuilder<'a, S> {
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

    pub fn new(usage: wgpu::BufferUsages) -> Self {
        Self {
            usage,
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
            state: InitBuffer {
                data: bytemuck::cast_slice(data),
            },
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

#[derive(Debug)]
pub struct BindGroup {
    pub layout: wgpu::BindGroupLayout,
    pub group: wgpu::BindGroup,
}

#[derive(Debug, Default)]
pub struct BindGroupBuilder<'a> {
    label: Option<&'a str>,
    layout_entries: Vec<wgpu::BindGroupLayoutEntry>,
    group_entries: Vec<wgpu::BindGroupEntry<'a>>,
    binding: u32,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn uniform_buffer(
        mut self,
        buffer: &'a wgpu::Buffer,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        debug_assert!(buffer.usage().contains(wgpu::BufferUsages::UNIFORM));

        self.layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        self.group_entries.push(wgpu::BindGroupEntry {
            binding: self.binding,
            resource: buffer.as_entire_binding(),
        });

        self.binding += 1;

        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> BindGroup {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: self.layout_entries.as_slice(),
            label: self.label,
        });

        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: self.group_entries.as_slice(),
            label: self.label,
        });

        BindGroup { layout, group }
    }
}

#[derive(Debug)]
pub struct WGPUContext<'guard> {
    pub window_id: WindowId,
    pub surface: wgpu::Surface,
    pub config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub marker: std::marker::PhantomData<&'guard ()>,
}

impl<'guard> WGPUContext<'guard> {
    pub async fn from_window(window: &'guard window::Window) -> WGPUContext<'guard> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let window_id = window.id();
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
                    features: wgpu::Features::default(),
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
            window_id,
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
