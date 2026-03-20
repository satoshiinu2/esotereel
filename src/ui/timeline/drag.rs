use egui::Pos2;

use crate::{
    project::{Project, clip::ClipDragState, timeline::Timeline},
    ui::timeline::{LAYER_HEIGHT, RULER_HEIGHT, TimelineWindow},
};

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum ClipGrabResult {
    None,
    NotSelected,
    DragStarted,
}

impl TimelineWindow {
    pub(super) fn handle_drag_grab(
        &mut self,
        timeline: &mut Timeline,
        local: Pos2,
    ) -> ClipGrabResult {
        let frame = self.x_to_frame(local.x);

        let Some((layer_idx, clip_idx, clip)) = self.clip_at(&timeline, &local) else {
            return ClipGrabResult::None;
        };

        if !self.selected_clips.contains(&clip.id) {
            return ClipGrabResult::NotSelected;
        }

        self.drag_state = Some(ClipDragState {
            src_layer_idx: layer_idx,
            clip_idx,
            src_frame: frame,
            offset_frames: frame - clip.position,
            current_layer_idx: layer_idx,
            current_frame: frame,
            ghost_pos: local,
        });
        return ClipGrabResult::DragStarted;
    }

    pub(super) fn handle_drag_continue(&mut self, timeline: &mut Timeline, local: Pos2) {
        // 👇 先に計算（ここでは self を普通に使える）
        let frame = self.x_to_frame(local.x);

        let max_layer = timeline.layers.len().saturating_sub(1);

        let temp_layer_idx = ((local.y - RULER_HEIGHT + self.scroll_y) / LAYER_HEIGHT) as usize;

        // 👇 あとで mutable borrow
        let Some(drag) = &mut self.drag_state else {
            return;
        };

        drag.current_frame = (frame - drag.offset_frames).max(0);
        drag.current_layer_idx = temp_layer_idx.min(max_layer);
        drag.ghost_pos = local;
    }

    pub(super) fn handle_drag_drop(&mut self, timeline: &mut Timeline, _local: Pos2) {
        let Some(drag) = &mut self.drag_state else {
            return;
        };

        let frame_moved = drag.current_frame - drag.src_frame + drag.offset_frames;
        let layer_moved = drag.current_layer_idx as isize - drag.src_layer_idx as isize;
        if layer_moved == 0 {
            for clip_id in &self.selected_clips {
                let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                    continue;
                };
                // update pos
                if let Some(clip) = timeline.layers[layer_idx].clips.get_mut(clip_idx) {
                    clip.position += frame_moved;
                }
            }
        } else {
            for clip_id in &self.selected_clips {
                let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                    continue;
                };
                // delete old
                let mut clip = timeline.layers[layer_idx].clips.swap_remove(clip_idx);

                // update pos
                clip.position += frame_moved;

                let target_layer_idx = layer_idx as isize + layer_moved;

                // range check
                let target_layer_idx = target_layer_idx as usize;
                if target_layer_idx >= timeline.layers.len() {
                    return;
                }

                // insert new
                timeline.layers[target_layer_idx].clips.push(clip);
            }
        }

        self.drag_state = None;
    }

    pub(super) fn draw_ghost(
        &self,
        timeline: &Timeline,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        let Some(drag) = &self.drag_state else {
            return;
        };

        let frame_moved = drag.current_frame - drag.src_frame + drag.offset_frames;
        let layer_moved = drag.current_layer_idx as isize - drag.src_layer_idx as isize;

        for clip_id in &self.selected_clips {
            let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                continue;
            };

            let clip = &timeline.layers[layer_idx].clips[clip_idx];

            let target_layer_idx = layer_idx as isize + layer_moved;

            // range check
            let target_layer_idx = target_layer_idx as usize;
            if target_layer_idx >= timeline.layers.len() {
                continue;
            }

            let w = clip.duration as f32 * self.zoom;
            let x = rect.left() + self.frame_to_x(clip.position + frame_moved);
            let y = rect.top() + self.layer_to_y(target_layer_idx);

            let ghost_rect = egui::Rect::from_min_size(
                egui::pos2(x, y + 2.0),
                egui::vec2(w, LAYER_HEIGHT - 4.0),
            );

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
}
