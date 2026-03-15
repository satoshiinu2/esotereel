use std::collections::HashMap;

pub struct ViewportState {
    pub title: String,
    pub size: [f32; 2],
}

pub struct App {
    pub viewports: HashMap<egui::ViewportId, ViewportState>,
}

impl Default for App {
    fn default() -> Self {
        let viewports = HashMap::new();

        Self { viewports }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 閉じられたウィンドウを収集
        let mut to_close = vec![];
        for (id, _) in &self.viewports {
            if ctx.input_for(*id, |i| i.viewport().close_requested()) {
                to_close.push(*id);
            }
        }
        for id in to_close {
            self.viewports.remove(&id);
        }

        // メインウィンドウ
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Timeline");

            if ui.button("新しいウィンドウを開く").clicked() {
                let id = egui::ViewportId::from_hash_of(format!("window_{}", self.viewports.len()));
                self.viewports.insert(
                    id,
                    ViewportState {
                        title: format!("Window {}", self.viewports.len()),
                        size: [400.0, 300.0],
                    },
                );
            }
        });

        // サブウィンドウ
        for (id, state) in &self.viewports {
            let id = *id;
            let title = state.title.clone();
            let size = state.size;

            ctx.show_viewport_immediate(
                id,
                egui::ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size(size),
                move |ctx, _| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label(&title);
                    });
                },
            );
        }
    }
}
