use std::sync::{Arc, Mutex};

use crate::render::{RenderState, start_render_thread};
use crate::ui::wgpuutil;
use crate::{project::Project, render::callback::WgpuRenderCallback, ui::WindowBehavior};

pub struct PreviewWindow {
    render_state: Option<Arc<RenderState>>,
    texture: Option<egui::TextureHandle>,
}

impl Default for PreviewWindow {
    fn default() -> Self {
        Self {
            render_state: None,
            texture: None,
        }
    }
}

impl WindowBehavior for PreviewWindow {
    fn id(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of("preview")
    }

    fn title(&self) -> String {
        "Preview".to_string()
    }

    fn size(&self) -> [f32; 2] {
        [800.0, 300.0]
    }

    fn update(
        &mut self,
        project: std::sync::Arc<std::sync::RwLock<Option<crate::project::Project>>>,
        _drag_state: std::sync::Arc<std::sync::RwLock<Option<crate::project::clip::ClipDragState>>>,
        ctx: &egui::Context,
    ) {
        let Some(render_state) = &self.render_state else {
            return;
        };

        let fb = render_state.frame_buffer.lock().unwrap();
        if let Some(fb) = fb.as_ref() {
            self.texture = Some(ctx.load_texture(
                "preview",
                egui::ColorImage::from_rgba_unmultiplied(
                    [fb.width as usize, fb.height as usize],
                    &fb.data,
                ),
                egui::TextureOptions::default(),
            ));
        }
        drop(fb);

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                ui.image(texture);
            } else {
                ui.label("No preview");
            }
        });
    }
}

impl PreviewWindow {
    fn draw(&mut self, project: Option<&mut Project>, ui: &mut egui::Ui) {
        if ui.button("open project").clicked() {
            // self.openproject();
        }
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            WgpuRenderCallback,
        ));
    }

    pub(crate) fn init_render_state(&mut self, wgpuutil: &super::WGpuUtil) {
        let render_state = Arc::new(RenderState {
            device: Arc::clone(&wgpuutil.device),
            queue: Arc::clone(&wgpuutil.queue),
            frame_buffer: Arc::new(Mutex::new(None)),
        });

        start_render_thread(Arc::clone(&render_state));
        self.render_state = Some(render_state);
    }
}
