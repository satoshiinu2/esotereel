use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use egui::Vec2;

use super::WindowBehavior;
use crate::{
    project::{Project, clip::ClipDragState, timeline::Timeline},
    ui::event::SubWindowEventQueue,
};

pub const LAYER_HEIGHT: f32 = 32.0;
pub const RULER_HEIGHT: f32 = 24.0;
pub const LABEL_WIDTH: f32 = 80.0;
pub const SCROLLBAR_SIZE: f32 = 12.0;

pub const DEFAULT_FRAME_COUNT: i64 = 300;
pub const DEFAULT_LAYER_LEN: i64 = 1;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimelineType {
    MAIN = 0,
    TEMP = 1,
}

pub struct TimelineWindow {
    pub timeline_type: TimelineType,
    pub zoom: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    event_queue: SubWindowEventQueue,
}

impl Default for TimelineWindow {
    fn default() -> Self {
        Self {
            timeline_type: TimelineType::MAIN,
            zoom: 4.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl TimelineWindow {
    pub fn set_timeline_type(mut self, ttype: TimelineType) -> Self {
        self.timeline_type = ttype;
        return self;
    }
}

impl WindowBehavior for TimelineWindow {
    fn id(&self) -> egui::ViewportId {
        return match self.timeline_type {
            TimelineType::MAIN => egui::ViewportId::from_hash_of("timeline"),
            TimelineType::TEMP => egui::ViewportId::from_hash_of("timeline temp"),
        };
    }

    fn title(&self) -> String {
        return match self.timeline_type {
            TimelineType::MAIN => "Timeline".to_string(),
            TimelineType::TEMP => "Temp Timeline".to_string(),
        };
    }

    fn size(&self) -> [f32; 2] {
        [800.0, 300.0]
    }

    fn update(
        &mut self,
        project: Arc<RwLock<Option<Project>>>,
        drag_state: Arc<RwLock<Option<ClipDragState>>>,
        ctx: &egui::Context,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Timeline");

            let mut project = project.write().unwrap();
            let mut drag = drag_state.write().unwrap();

            let available = ui.available_size();

            let timeline_size =
                egui::vec2(available.x - SCROLLBAR_SIZE, available.y - SCROLLBAR_SIZE);

            let (response, painter) =
                ui.allocate_painter(timeline_size, egui::Sense::click_and_drag());
            let rect = response.rect;

            self.draw(
                project.as_mut(),
                &mut drag,
                timeline_size,
                &response,
                &painter,
                rect,
            );
        });
    }
}

impl TimelineWindow {
    pub fn frame_to_x(&self, frame: i64) -> f32 {
        frame as f32 * self.zoom - self.scroll_x + LABEL_WIDTH
    }

    pub fn x_to_frame(&self, x: f32) -> i64 {
        ((x - LABEL_WIDTH + self.scroll_x) / self.zoom) as i64
    }

    pub fn layer_to_y(&self, layer_idx: usize) -> f32 {
        layer_idx as f32 * LAYER_HEIGHT + RULER_HEIGHT - self.scroll_y
    }

    pub fn draw(
        &mut self,
        project: Option<&mut Project>,
        drag_state: &mut Option<ClipDragState>,
        timeline_size: Vec2,
        response: &egui::Response,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        // 背景
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

        // ルーラー
        self.draw_ruler(&painter, rect);

        if let Some(project) = project {
            let playhead_frame = project.playhead;
            let timeline = project.get_timeline_by(self.timeline_type);
            // レイヤーラベルと区切り線
            self.draw_layers(timeline, drag_state.as_ref(), &painter, rect);

            // ゴースト
            self.draw_ghost(timeline, drag_state.as_ref(), &painter, rect);

            // 再生ヘッド
            self.draw_playhead(playhead_frame, &painter, rect);

            //スクロールバー
            self.draw_scrollbar(timeline_size, Some(timeline), &painter, &response, rect);

            // ドラッグ処理
            self.handle_drag(project, drag_state, &response, rect);
        } else {
            // 再生ヘッド
            self.draw_playhead(0, &painter, rect);

            //スクロールバー
            self.draw_scrollbar(timeline_size, None, &painter, &response, rect);
        }

        // ホイールでスクロール
        self.wheel_scroll(response);
    }

    fn draw_layers(
        &mut self,
        timeline: &Timeline,
        drag_state: Option<&ClipDragState>,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        for (i, layer) in timeline.layers.iter().enumerate() {
            let y = rect.top() + self.layer_to_y(i);
            let layer_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), y),
                egui::vec2(rect.width(), LAYER_HEIGHT),
            );

            // 背景色
            let is_drop_target = drag_state
                .as_ref()
                .map_or(false, |d| d.current_layer_idx == i && d.src_layer_idx != i);

            let bg = if is_drop_target {
                egui::Color32::from_rgb(60, 80, 60) // ハイライト
            } else if i % 2 == 0 {
                egui::Color32::from_rgb(40, 40, 40)
            } else {
                egui::Color32::from_rgb(45, 45, 45)
            };
            painter.rect_filled(layer_rect, 0.0, bg);

            // レイヤーラベル
            painter.text(
                egui::pos2(rect.left() + 4.0, y + LAYER_HEIGHT / 2.0),
                egui::Align2::LEFT_CENTER,
                &layer.name,
                egui::FontId::default(),
                egui::Color32::WHITE,
            );

            // クリップ描画
            for (clip_idx, clip) in layer.clips.iter().enumerate() {
                let is_dragging =
                    drag_state.map_or(false, |d| d.src_layer_idx == i && d.clip_idx == clip_idx);

                // ドラッグ中は元の位置に半透明で残す
                let color = if is_dragging {
                    egui::Color32::from_rgba_premultiplied(70, 130, 180, 80)
                } else {
                    egui::Color32::from_rgb(70, 130, 180)
                };

                let border_color = if is_dragging {
                    egui::Color32::from_rgba_premultiplied(100, 160, 210, 80)
                } else {
                    egui::Color32::from_rgb(100, 160, 210)
                };

                let x = rect.left() + self.frame_to_x(clip.position);
                let w = clip.duration as f32 * self.zoom;
                let clip_rect = egui::Rect::from_min_size(
                    egui::pos2(x, y + 2.0),
                    egui::vec2(w, LAYER_HEIGHT - 4.0),
                );
                painter.rect_filled(clip_rect, 3.0, color);
                painter.rect_stroke(clip_rect, 3.0, egui::Stroke::new(1.0, border_color));
            }
        }
    }

    fn draw_playhead(&self, playhead_frame: i64, painter: &egui::Painter, rect: egui::Rect) {
        let ph_x = rect.left() + self.frame_to_x(playhead_frame);
        painter.line_segment(
            [
                egui::pos2(ph_x, rect.top()),
                egui::pos2(ph_x, rect.bottom()),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 80, 80)),
        );
    }

    fn draw_ruler(&self, painter: &egui::Painter, rect: egui::Rect) {
        let ruler_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + LABEL_WIDTH, rect.top()),
            egui::vec2(rect.width() - LABEL_WIDTH, RULER_HEIGHT),
        );
        painter.rect_filled(ruler_rect, 0.0, egui::Color32::from_rgb(50, 50, 50));

        // 目盛り（10フレームごと）
        let start_frame = (self.scroll_x / self.zoom) as i64;
        let end_frame = start_frame + (rect.width() / self.zoom) as i64 + 10;

        for frame in (start_frame..end_frame).step_by(10) {
            let x = rect.left() + self.frame_to_x(frame);
            if x < rect.left() + LABEL_WIDTH {
                continue;
            }

            painter.line_segment(
                [
                    egui::pos2(x, rect.top()),
                    egui::pos2(x, rect.top() + RULER_HEIGHT),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 100)),
            );
            painter.text(
                egui::pos2(x + 2.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                format!("{}", frame),
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(180, 180, 180),
            );
        }
    }
}
