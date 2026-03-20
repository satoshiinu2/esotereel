use egui_wgpu::wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowAttributes;

use crate::project::clip::ClipDragState;
use crate::ui::preview::PreviewWindow;
use crate::ui::timeline::TimelineWindow;
use crate::ui::{WindowBehavior, WindowState, wgpuutil};
use crate::{project::Project, ui::wgpuutil::WGpuUtil};
use std::rc::Rc;
use std::sync::Mutex;

pub struct App {
    pub project: Option<Project>,

    pub(super) windows: Vec<WindowState>,
}

impl App {
    pub fn new() -> Self {
        Self {
            windows: vec![],
            project: None,
        }
    }
    fn openproject(&mut self) {
        self.project = Some(Project::new());
    }

    fn get_attr_by<T: WindowBehavior>(behavior: &T) -> WindowAttributes {
        let wsize = behavior.size();
        let wtitle = behavior.title();

        WindowAttributes::default()
            .with_title(wtitle)
            .with_inner_size(winit::dpi::LogicalSize::new(wsize[0], wsize[1]))
    }
    fn get_wgpuutil_by<T: WindowBehavior>(event_loop: &ActiveEventLoop, behavior: &T) -> WGpuUtil {
        WGpuUtil::new(event_loop, App::get_attr_by(behavior))
    }
    fn get_win_state_by<T: WindowBehavior + 'static>(
        event_loop: &ActiveEventLoop,
        mut behavior: T,
    ) -> WindowState {
        let wgpuutil = App::get_wgpuutil_by(event_loop, &behavior);
        behavior.init_special_renderer(&wgpuutil);
        WindowState::new(wgpuutil, Box::new(behavior))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.openproject();

        let behavior = PreviewWindow::default();
        let win_state = App::get_win_state_by(event_loop, behavior);
        self.windows.push(win_state);

        let behavior: TimelineWindow = TimelineWindow::new(crate::ui::timeline::TimelineType::Main);
        let win_state = App::get_win_state_by(event_loop, behavior);
        self.windows.push(win_state);

        let behavior: TimelineWindow = TimelineWindow::new(crate::ui::timeline::TimelineType::Temp);
        let win_state = App::get_win_state_by(event_loop, behavior);
        self.windows.push(win_state);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(win_state) = self
            .windows
            .iter_mut()
            .find(|w| w.wgpuutil.window.id() == window_id)
        else {
            return;
        };

        if !win_state.visible {
            return;
        }

        if win_state
            .wgpuutil
            .egui_state
            .on_window_event(&win_state.wgpuutil.window, &event)
            .consumed
        {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                win_state.visible = false;
            }
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }

                win_state.wgpuutil.config.width = size.width;
                win_state.wgpuutil.config.height = size.height;
                win_state.need_reconfigure = true;
            }
            WindowEvent::RedrawRequested => {
                let wgpuutil = &mut win_state.wgpuutil;
                let win = &wgpuutil.window;
                let raw_input = wgpuutil.egui_state.take_egui_input(&win);

                if win_state.need_reconfigure {
                    wgpuutil
                        .surface
                        .configure(&wgpuutil.device, &wgpuutil.config);
                    win_state.need_reconfigure = false;

                    wgpuutil.window.request_redraw();
                }

                let full_output = wgpuutil.egui_ctx.run(raw_input, |ctx| {
                    win_state.behavior.update(&mut self.project, &ctx);
                });

                wgpuutil
                    .egui_state
                    .handle_platform_output(win, full_output.platform_output);

                // 描画準備
                let paint_jobs = wgpuutil
                    .egui_ctx
                    .tessellate(full_output.shapes, wgpuutil.window.scale_factor() as f32);

                let frame = wgpuutil.surface.get_current_texture().unwrap();

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = wgpuutil
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                // egui描画
                let screen_desc = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [wgpuutil.config.width, wgpuutil.config.height],
                    pixels_per_point: win.scale_factor() as f32,
                };

                for (id, image_delta) in &full_output.textures_delta.set {
                    wgpuutil.renderer.update_texture(
                        &wgpuutil.device,
                        &wgpuutil.queue,
                        *id,
                        image_delta,
                    );
                }

                wgpuutil.renderer.update_buffers(
                    &wgpuutil.device,
                    &wgpuutil.queue,
                    &mut encoder,
                    &paint_jobs,
                    &egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [wgpuutil.config.width, wgpuutil.config.height],
                        pixels_per_point: win.scale_factor() as f32,
                    },
                );

                {
                    let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });

                    // 'staticに昇格
                    let mut rpass = unsafe { to_static_rpass(rpass) };

                    wgpuutil
                        .renderer
                        .render(&mut rpass, &paint_jobs, &screen_desc);

                    win_state
                        .behavior
                        .render_special(&mut self.project, &mut rpass, &wgpuutil);
                }

                for id in &full_output.textures_delta.free {
                    wgpuutil.renderer.free_texture(id);
                }

                wgpuutil.queue.submit(Some(encoder.finish()));
                frame.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.windows.retain(|w| w.visible);
        if self.windows.is_empty() {
            event_loop.exit();
            return;
        }

        self.windows
            .iter()
            .filter(|w| w.need_redraw && w.visible)
            .for_each(|w| w.wgpuutil.window.request_redraw());
    }
}

unsafe fn to_static_rpass(rpass: wgpu::RenderPass<'_>) -> wgpu::RenderPass<'static> {
    unsafe { std::mem::transmute(rpass) }
}
