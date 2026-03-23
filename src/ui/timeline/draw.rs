use egui::Vec2;

use crate::{
    project::timeline::Timeline,
    ui::{
        Project,
        timeline::{LABEL_WIDTH, LAYER_HEIGHT, RULER_HEIGHT, TimelineWindow},
    },
};

impl TimelineWindow {
    pub(super) fn draw(
        &mut self,
        project: &mut Option<Project>,
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
            let timeline = project.get_timeline_by(self.timeline_type);
            // レイヤーラベルと区切り線
            self.draw_layers(timeline, &painter, rect);

            // 選択エリア
            self.draw_selection_rect(painter, rect);

            // ゴースト
            self.draw_ghost(timeline, &painter, rect);

            // 再生ヘッド
            self.draw_playhead(timeline.playhead, &painter, rect);

            //スクロールバー
            self.draw_scrollbar(timeline_size, Some(timeline), &painter, &response, rect);

            // ドラッグ処理
            self.handle_clip_ctrl(timeline, &response, rect);
        } else {
            // 選択エリア
            self.draw_selection_rect(painter, rect);

            // 再生ヘッド
            self.draw_playhead(0, &painter, rect);

            //スクロールバー
            self.draw_scrollbar(timeline_size, None, &painter, &response, rect);
        }

        // ホイールでスクロール
        self.wheel_scroll(response);
    }

    pub(super) fn draw_layers(
        &mut self,
        timeline: &Timeline,
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
            let is_drop_target = self
                .drag_state
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
                self.render_clip(i, clip_idx, clip, rect, y, painter);
            }
        }
    }

    pub(super) fn render_clip(
        &mut self,
        layer_idx: usize,
        clip_idx: usize,
        clip: &crate::project::clip::Clip,
        rect: egui::Rect,
        y: f32,
        painter: &egui::Painter,
    ) {
        let is_selected = self.selected_clips.contains(&clip.id);
        let is_dragging = self.drag_state.as_mut().map_or(false, |d| {
            d.src_layer_idx == layer_idx && d.clip_idx == clip_idx
        });

        // ドラッグ中は元の位置に半透明で残す
        let color = if is_dragging {
            egui::Color32::from_rgba_premultiplied(70, 130, 180, 50)
        } else if is_selected {
            egui::Color32::from_rgb(100, 150, 200)
        } else {
            egui::Color32::from_rgb(70, 130, 180)
        };

        let stroke_color = if is_dragging {
            egui::Color32::from_rgba_premultiplied(100, 160, 210, 50)
        } else if is_selected {
            egui::Color32::from_rgb(150, 200, 255)
        } else {
            egui::Color32::from_rgb(100, 160, 210)
        };

        let x = rect.left() + self.frame_to_x(clip.position);
        let w = clip.duration as f32 * self.zoom;
        let clip_rect =
            egui::Rect::from_min_size(egui::pos2(x, y + 2.0), egui::vec2(w, LAYER_HEIGHT - 4.0));
        painter.rect_filled(clip_rect, 3.0, color);
        painter.rect_stroke(clip_rect, 3.0, egui::Stroke::new(1.0, stroke_color));
    }

    pub(super) fn draw_playhead(
        &self,
        playhead_frame: i64,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        let ph_x = rect.left() + self.frame_to_x(playhead_frame);
        painter.line_segment(
            [
                egui::pos2(ph_x, rect.top()),
                egui::pos2(ph_x, rect.bottom()),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 80, 80)),
        );
    }

    pub(super) fn draw_ruler(&self, painter: &egui::Painter, rect: egui::Rect) {
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
    pub(super) fn draw_selection_rect(&self, painter: &egui::Painter, rect: egui::Rect) {
        let Some(sel) = &self.selection_rect else {
            return;
        };

        let sel_rect = egui::Rect::from_two_pos(
            rect.min + sel.start.to_vec2(),
            rect.min + sel.current.to_vec2(),
        );

        painter.rect_filled(
            sel_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(100, 150, 255, 64),
        );
        painter.rect_stroke(
            sel_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 150, 255)),
        );
    }
}
