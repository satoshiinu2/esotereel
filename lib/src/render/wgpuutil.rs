use std::{collections::HashMap, sync::Arc};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::ExperimentalFeatures;

use crate::render::{
    pipeline::WgpuRenderResources,
    surfacetarget::{self, SurfaceTarget},
};

pub struct WGpuUtil {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
    pub resources: WgpuRenderResources,
    pub textures: HashMap<u32, (wgpu::Texture, wgpu::BindGroup)>,
}

impl WGpuUtil {
    pub fn new(surface_target: SurfaceTarget, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::default();

        let surface_target = Arc::new(surface_target);
        let surface = instance
            .create_surface(surface_target.clone())
            .expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Could not get an adapter (GPU).");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: Default::default(),
            experimental_features: ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .expect("Failed to create device");

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

        let surface = Some(surface);

        let resources = WgpuRenderResources::new(&device, &queue, config.format);

        Self {
            instance,
            device,
            queue,
            surface,
            config,
            resources,
            textures: HashMap::new(),
        }
    }

    pub fn get_surface(
        instance: &wgpu::Instance,
        target: impl HasWindowHandle + HasDisplayHandle + Clone + Send + Sync + 'static,
    ) -> wgpu::Surface<'static> {
        instance
            .create_surface(target)
            .expect("Failed to create surface")
    }

    pub fn detach_surface(&mut self) {
        self.surface = None; // Dropが走ってGPU側surfaceだけ解放。device等は無傷
    }

    pub fn attach_surface(&mut self, target: SurfaceTarget) {
        let target_arc = std::sync::Arc::new(target);
        let surface = self
            .instance
            .create_surface(target_arc)
            .expect("Failed to create surface");

        surface.configure(&self.device, &self.config);

        self.surface = Some(surface);
    }
}
