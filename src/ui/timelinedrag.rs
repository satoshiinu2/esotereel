use egui::Pos2;

use crate::{
    project::{Project, clip::ClipDragState, timeline::Timeline},
    ui::timeline::{LAYER_HEIGHT, RULER_HEIGHT},
};

use super::timeline::TimelineWindow;

impl TimelineWindow {
    pub(crate) fn handle_drag(
        &mut self,
        project: &mut Project,
        drag_state: &mut Option<ClipDragState>,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.handle_drag_grab(project, drag_state, rect, pos);
            }
        }

        let mouse_down = response.ctx.input(|i| i.pointer.primary_down());
        if !mouse_down {
            self.handle_drag_drop(project, drag_state);
        }

        if let Some(pos) = response.ctx.input(|i| i.pointer.hover_pos()) {
            if rect.contains(pos) {
                self.handle_drag_drag(project, drag_state, rect, pos);
            }
        }
    }

    fn handle_drag_grab(
        &mut self,
        project: &mut Project,
        drag_state: &mut Option<ClipDragState>,
        rect: egui::Rect,
        pos: Pos2,
    ) {
        let timeline = project.get_timeline_by(self.timeline_type);
        let local = pos - rect.min;
        let frame = self.x_to_frame(local.x);
        let layer_idx = ((local.y - RULER_HEIGHT + self.scroll_y) / LAYER_HEIGHT) as usize;

        if layer_idx < timeline.layers.len() {
            for (clip_idx, clip) in timeline.layers[layer_idx].clips.iter().enumerate() {
                if frame >= clip.position && frame < clip.position + clip.duration {
                    *drag_state = Some(ClipDragState {
                        src_timeline_type: self.timeline_type,
                        src_layer_idx: layer_idx,
                        clip_idx,
                        offset_frames: frame - clip.position,
                        current_layer_idx: layer_idx,
                        current_frame: frame,
                        ghost_pos: pos,
                    });
                    break;
                }
            }
        }
    }

    fn handle_drag_drag(
        &mut self,
        project: &mut Project,
        drag_state: &mut Option<ClipDragState>,
        rect: egui::Rect,
        pos: Pos2,
    ) {
        if let Some(drag) = drag_state.as_mut() {
            let timeline = project.get_timeline_by(self.timeline_type);

            let local = pos - rect.min;
            drag.current_frame = (self.x_to_frame(local.x) - drag.offset_frames).max(0);
            let temp_layer_idx = ((local.y - RULER_HEIGHT + self.scroll_y) / LAYER_HEIGHT) as usize;
            let max_layer = timeline.layers.len().saturating_sub(1);
            drag.current_layer_idx = temp_layer_idx.min(max_layer);
            drag.ghost_pos = pos;
        }
    }
    fn handle_drag_drop(&mut self, project: &mut Project, drag_state: &mut Option<ClipDragState>) {
        if let Some(drag) = drag_state.take() {
            let mut clip = project.get_timeline_by(drag.src_timeline_type).layers
                [drag.src_layer_idx]
                .clips
                .remove(drag.clip_idx);

            clip.position = drag.current_frame;

            project.get_timeline_by(self.timeline_type).layers[drag.current_layer_idx]
                .clips
                .push(clip);
        }
    }

    pub(crate) fn draw_ghost(
        &self,
        timeline: &Timeline,
        drag_state: Option<&ClipDragState>,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        let Some(drag) = drag_state else {
            return;
        };

        // index checks
        if drag.src_layer_idx >= timeline.layers.len() {
            return;
        }
        if drag.clip_idx >= timeline.layers[drag.src_layer_idx].clips.len() {
            return;
        }

        let clip = &timeline.layers[drag.src_layer_idx].clips[drag.clip_idx];

        let w = clip.duration as f32 * self.zoom;
        let x = rect.left() + self.frame_to_x(drag.current_frame);
        let y = rect.top() + self.layer_to_y(drag.current_layer_idx);

        let ghost_rect =
            egui::Rect::from_min_size(egui::pos2(x, y + 2.0), egui::vec2(w, LAYER_HEIGHT - 4.0));

        painter.rect_filled(
            ghost_rect,
            3.0,
            egui::Color32::from_rgba_premultiplied(70, 130, 180, 180),
        );
        painter.rect_stroke(
            ghost_rect,
            3.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(150, 200, 255)),
        );
    }
}
