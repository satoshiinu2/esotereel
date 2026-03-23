use std::sync::Arc;

use egui_wgpu::wgpu;
use winit::{
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

pub struct WGpuUtil {
    pub window: Arc<Window>,
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub renderer: egui_wgpu::Renderer,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

impl WGpuUtil {
    pub fn new(elwt: &ActiveEventLoop, attrs: WindowAttributes) -> Self {
        let window = Arc::new(elwt.create_window(attrs).unwrap());

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Could not get an adapter (GPU).");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .expect("Failed to get device");

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&device, &config);

        // egui renderer
        let renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1, false);

        let egui_ctx = egui::Context::default();

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            elwt,
            None,
            None,
            None,
        );

        Self {
            egui_ctx,
            egui_state,
            window,
            renderer,
            device,
            queue,
            surface,
            config,
        }
    }
}
