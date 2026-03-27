use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::render::pipeline::WgpuRenderResources;

pub struct WGpuUtil {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub resources: WgpuRenderResources,
}

impl WGpuUtil {
    pub fn new(target: impl HasWindowHandle + HasDisplayHandle, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::default();
        let surface = WGpuUtil::get_surface(&instance, target);

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Could not get an adapter (GPU).");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("Failed to get device");

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

        let resources = WgpuRenderResources::new(&device, config.format);

        Self {
            instance,
            device,
            queue,
            surface,
            config,
            resources,
        }
    }

    pub fn get_surface(
        instance: &wgpu::Instance,
        target: impl HasWindowHandle + HasDisplayHandle,
    ) -> wgpu::Surface<'static> {
        unsafe {
            let surface = instance
                .create_surface_unsafe(
                    wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target)
                        .expect("Failed to create SurfaceTargetUnsafe from handles"),
                )
                .expect("Failed to create surface");

            // ライフタイムを強制的に 'static にキャスト
            std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface)
        }
    }

    pub fn update_surface(&mut self, target: impl HasWindowHandle + HasDisplayHandle) {
        self.surface = WGpuUtil::get_surface(&self.instance, target)
    }
}
