use egui::Vec2;

use crate::{
    project::timeline::Timeline,
    ui::timeline::{
        DEFAULT_FRAME_COUNT, DEFAULT_LAYER_LEN, LAYER_HEIGHT, RULER_HEIGHT, SCROLLBAR_SIZE,
    },
};

use super::timeline::TimelineWindow;

impl TimelineWindow {
    pub(super) fn wheel_scroll(&mut self, response: &egui::Response) {
        if let Some(scroll) = response.ctx.input(|i| {
            if i.raw_scroll_delta.x != 0.0 {
                Some(i.raw_scroll_delta.x)
            } else {
                None
            }
        }) {
            self.scroll_x = (self.scroll_x - scroll).max(0.0);
        }
    }

    pub(crate) fn draw_scrollbar(
        &mut self,
        timeline_size: Vec2,
        timeline: Option<&Timeline>,
        painter: &egui::Painter,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        //　スクロールバー位置計算
        let total_frames = timeline
            .map(|t| {
                t.layers
                    .iter()
                    .flat_map(|l| l.clips.iter())
                    .map(|c| c.position + c.duration)
                    .max()
                    .unwrap_or(DEFAULT_FRAME_COUNT)
            })
            .unwrap_or(DEFAULT_FRAME_COUNT);
        let total_layers = timeline
            .map(|t| t.layers.len() as i64)
            .unwrap_or(DEFAULT_LAYER_LEN);

        let content_width = total_frames as f32 * self.zoom;
        let content_height = total_layers as f32 * LAYER_HEIGHT + RULER_HEIGHT;

        // 横スクロールバー
        let h_bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom()),
            egui::vec2(timeline_size.x, SCROLLBAR_SIZE),
        );
        self.draw_scrollbar_h(h_bar_rect, content_width, painter, response);

        // 縦スクロールバー
        let v_bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right(), rect.top()),
            egui::vec2(SCROLLBAR_SIZE, timeline_size.y),
        );
        self.draw_scrollbar_v(v_bar_rect, content_height, painter, response);
    }

    fn draw_scrollbar_h(
        &mut self,
        rect: egui::Rect,
        content_width: f32,
        painter: &egui::Painter,
        response: &egui::Response,
    ) {
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(50, 50, 50));

        let visible_width = rect.width();
        if content_width <= visible_width {
            return;
        }

        let thumb_ratio = visible_width / content_width;
        let thumb_width = (rect.width() * thumb_ratio).max(20.0);
        let scroll_ratio = self.scroll_x / (content_width - visible_width);
        let thumb_x = rect.left() + scroll_ratio * (rect.width() - thumb_width);

        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(thumb_x, rect.top() + 2.0),
            egui::vec2(thumb_width, rect.height() - 4.0),
        );
        painter.rect_filled(thumb_rect, 4.0, egui::Color32::from_rgb(120, 120, 120));

        // ドラッグ
        if response.dragged() {
            let delta = response.drag_delta().x;
            let scroll_per_pixel = (content_width - visible_width) / (rect.width() - thumb_width);
            self.scroll_x = (self.scroll_x + delta * scroll_per_pixel)
                .clamp(0.0, content_width - visible_width);
        }
    }

    fn draw_scrollbar_v(
        &mut self,
        rect: egui::Rect,
        content_height: f32,
        painter: &egui::Painter,
        response: &egui::Response,
    ) {
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(50, 50, 50));

        let visible_height = rect.height();
        if content_height <= visible_height {
            return;
        }

        let thumb_ratio = visible_height / content_height;
        let thumb_height = (rect.height() * thumb_ratio).max(20.0);
        let scroll_ratio = self.scroll_y / (content_height - visible_height);
        let thumb_y = rect.top() + scroll_ratio * (rect.height() - thumb_height);

        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 2.0, thumb_y),
            egui::vec2(rect.width() - 4.0, thumb_height),
        );
        painter.rect_filled(thumb_rect, 4.0, egui::Color32::from_rgb(120, 120, 120));

        // ドラッグ
        if response.dragged() {
            let delta = response.drag_delta().y;
            let scroll_per_pixel =
                (content_height - visible_height) / (rect.height() - thumb_height);
            self.scroll_y = (self.scroll_y + delta * scroll_per_pixel)
                .clamp(0.0, content_height - visible_height);
        }
    }
}
