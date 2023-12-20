use crate::render::{rgba_to_u32, Instance};
use opencl3 as cl;
use opencl3::{kernel, types};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window;

pub mod render;
pub mod wgpu_utils;

pub const PARTICLE_COUNT: usize = 2;
pub const MAX_PARTICLES_PER_CELL: usize = 4;
pub const PARTICLE_RADIUS: f32 = 0.5;

const PROGRAM_SOURCE: &str = include_str!("sorting.ocl");

struct OpenClState {
    particles: Vec<Instance>,
    particle_buffer: cl::memory::Buffer<Instance>,
    count_per_cell: Vec<u32>,
    count_buffer: cl::memory::Buffer<u32>,
    cell_ids: Vec<i32>,
    id_buffer: cl::memory::Buffer<i32>,
    n_per_cell: u32,
    n_cells: u32,

    device: cl::device::Device,
    context: cl::context::Context,
    queue: cl::command_queue::CommandQueue,
    sort_kernel: kernel::Kernel,
    collide_kernel: kernel::Kernel,
    active_events: Vec<cl::event::Event>,
}

impl OpenClState {
    pub fn new() -> cl::Result<Self> {
        use cl::{
            command_queue, context, device, kernel, memory, program,
            types::{self, cl_float, cl_int, cl_uint},
        };
        use std::ptr;

        let device_id = device::get_all_devices(device::CL_DEVICE_TYPE_GPU)
            .expect("no device found")
            .into_iter()
            .nth(0)
            .unwrap();

        let device = device::Device::new(device_id);
        println!("Device: {:?}", device.name());

        let context = context::Context::from_device(&device)?;

        let queue = command_queue::CommandQueue::create_default_with_properties(
            &context,
            command_queue::CL_QUEUE_PROFILING_ENABLE,
            device.queue_on_device_preferred_size()? as cl_uint,
        )?;

        let program =
            program::Program::create_and_build_from_source(&context, PROGRAM_SOURCE, "").unwrap();

        let sort_kernel = kernel::Kernel::create(&program, "sort_particles")?;
        let collide_kernel = kernel::Kernel::create(&program, "collide_particles")?;

        let n_per_cell = MAX_PARTICLES_PER_CELL as cl_uint;
        let grid_size: cl_float = PARTICLE_RADIUS * 2.0;

        let mut n_cells: usize = (1.0 / grid_size).floor() as usize;

        let mut count_per_cell = vec![0 as cl_uint; n_cells * n_cells];
        let mut cell_ids = vec![-1; n_cells * n_cells * MAX_PARTICLES_PER_CELL];

        //let mut particles = vec![Instance::default(); PARTICLE_COUNT];
        //for i in 0..PARTICLE_COUNT {
        //    let pos_x = rand_float((i + 1) as u32);
        //    let pos_y = rand_float(hash((i + 1) as u32));
        //    particles[i] = Instance {
        //        pos: [pos_x, pos_y],
        //        vel: [0.0, 0.0],
        //    };
        //}

        let mut particles = vec![
            Instance {
                pos: [0.5, 0.5],
                vel: [0.0, 0.0],
            },
            Instance {
                pos: [0.2, 0.5],
                vel: [0.0, 0.0],
            },
        ];

        let mut count_buffer = unsafe {
            memory::Buffer::<cl_uint>::create(
                &context,
                memory::CL_MEM_WRITE_ONLY,
                n_cells * n_cells,
                ptr::null_mut(),
            )?
        };

        let mut particle_buffer = unsafe {
            memory::Buffer::<Instance>::create(
                &context,
                memory::CL_MEM_READ_WRITE,
                PARTICLE_COUNT,
                ptr::null_mut(),
            )?
        };

        let mut id_buffer = unsafe {
            memory::Buffer::<cl_int>::create(
                &context,
                memory::CL_MEM_WRITE_ONLY,
                cell_ids.len(),
                ptr::null_mut(),
            )?
        };

        Ok(Self {
            particles,
            particle_buffer,
            count_per_cell,
            count_buffer,
            cell_ids,
            id_buffer,
            n_per_cell,
            n_cells: n_cells as u32,
            active_events: vec![],
            device,
            queue,
            context,
            sort_kernel,
            collide_kernel,
        })
    }

    pub fn event_wait_list(&mut self) -> Vec<types::cl_event> {
        self.active_events.iter().map(|e| e.get()).collect()
    }

    pub fn step(&mut self) -> cl::Result<()> {
        self.cell_ids.iter_mut().for_each(|id| *id = -1);
        self.count_per_cell.iter_mut().for_each(|id| *id = 0);

        let _ = unsafe {
            self.queue.enqueue_write_buffer(
                &mut self.count_buffer,
                types::CL_NON_BLOCKING,
                0,
                self.count_per_cell.as_mut_slice(),
                &[],
            )?
        };

        let _ = unsafe {
            self.queue.enqueue_write_buffer(
                &mut self.id_buffer,
                types::CL_NON_BLOCKING,
                0,
                self.cell_ids.as_mut_slice(),
                &[],
            )?
        };

        let e = unsafe {
            self.queue.enqueue_write_buffer(
                &mut self.particle_buffer,
                types::CL_NON_BLOCKING,
                0,
                &self.particles,
                &[],
            )?
        };
        self.active_events.push(e);

        let mut wait_list = self.event_wait_list();

        let sorting = unsafe {
            kernel::ExecuteKernel::new(&self.sort_kernel)
                .set_arg(&self.count_buffer)
                .set_arg(&self.id_buffer)
                .set_arg(&self.particle_buffer)
                .set_arg(&self.n_per_cell)
                .set_arg(&self.n_cells)
                .set_global_work_size(self.particles.len())
                .set_event_wait_list(wait_list.as_mut_slice())
                .enqueue_nd_range(&self.queue)?
        };

        let colliding = unsafe {
            kernel::ExecuteKernel::new(&self.collide_kernel)
                .set_arg(&self.count_buffer)
                .set_arg(&self.id_buffer)
                .set_arg(&self.particle_buffer)
                .set_arg(&self.n_per_cell)
                .set_arg(&self.n_cells)
                .set_arg(&PARTICLE_RADIUS)
                .set_global_work_size(self.particles.len())
                .set_wait_event(&sorting)
                .enqueue_nd_range(&self.queue)?
        };

        self.active_events = vec![colliding];
        Ok(())
    }

    pub fn read(&mut self) -> cl::Result<()> {
        let mut event = self.event_wait_list();

        unsafe {
            self.queue.enqueue_read_buffer(
                &self.count_buffer,
                types::CL_NON_BLOCKING,
                0,
                &mut self.count_per_cell,
                event.as_mut_slice(),
            )?
        }.wait()?;

        unsafe {
            self.queue.enqueue_read_buffer(
                &self.id_buffer,
                types::CL_NON_BLOCKING,
                0,
                &mut self.cell_ids,
                event.as_mut_slice(),
            )?
        }.wait()?;

        unsafe {
            self.queue.enqueue_read_buffer(
                &self.particle_buffer,
                types::CL_NON_BLOCKING,
                0,
                &mut self.particles,
                event.as_mut_slice(),
            )?
        }.wait()?;

        self.active_events.clear();
        Ok(())
    }

    pub fn color_particles(&mut self) {
    }
}

fn hash(x: u32) -> u32 {
    let mut x = std::num::Wrapping(x);
    x += x.0.wrapping_shl(10u32);
    x ^= x.0.wrapping_shr(6u32);
    x += x.0.wrapping_shl(3u32);
    x ^= x.0.wrapping_shr(11u32);
    x += x.0.wrapping_shl(15u32);
    return x.0;
}

// random float in range [0..1]
fn rand_float(x: u32) -> f32 {
    let mut m = hash(x);
    const IEEE_MANTISSA: u32 = 0x007FFFFFu32;
    const IEEE_ONE: u32 = 0x3F800000u32;
    m &= IEEE_MANTISSA;
    m |= IEEE_ONE;
    let f: f32 = unsafe { std::mem::transmute(m) };
    return f - 1.0;
}

pub async fn run() {
    let event_loop = EventLoop::new().expect("could not create event loop");
    let window = window::WindowBuilder::new().build(&event_loop).unwrap();

    let mut cl_state = OpenClState::new().unwrap_or_else(|err| panic!("{err}"));
    cl_state.step().unwrap_or_else(|err| panic!("{err}"));
    cl_state.read().unwrap();
    cl_state.color_particles();

    let mut state = render::RenderState::new(&window).await;
    state.update_instances(cl_state.particles.as_slice());

    event_loop
        .run(|event, elwt| match event {
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
        })
        .unwrap();
}
