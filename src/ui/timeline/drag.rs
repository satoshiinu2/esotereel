use std::collections::HashSet;

use egui::Pos2;

use crate::{
    project::{clip::ClipDragState, timeline::Timeline},
    ui::timeline::{LAYER_HEIGHT, RULER_HEIGHT, TimelineWindow},
};

impl TimelineWindow {
    pub(super) fn handle_drag_grab(
        &mut self,
        timeline: &mut Timeline,
        local: Pos2,
        ctrl: bool,
    ) -> bool {
        let frame = self.x_to_frame(local.x);

        let Some((layer_idx, clip_idx, clip)) = self.clip_at(&timeline, &local) else {
            return false;
        };

        if !self.selected_clips.contains(&clip.id) {
            if !ctrl {
                self.selected_clips.clear();
            }
            self.selected_clips.insert(clip.id);
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
        return true;
    }

    pub(super) fn handle_drag_continue(&mut self, timeline: &mut Timeline, local: Pos2) {
        let frame = self.x_to_frame(local.x);

        let max_layer = timeline.layers.len().saturating_sub(1);

        let temp_layer_idx = ((local.y - RULER_HEIGHT + self.scroll_y) / LAYER_HEIGHT) as usize;

        let Some(drag) = &mut self.drag_state else {
            return;
        };

        drag.current_frame = (frame - drag.offset_frames).max(0);
        drag.current_layer_idx = temp_layer_idx.min(max_layer);
        drag.ghost_pos = local;

        // ovetlap check
        self.is_wrong = false;

        let frame_moved = drag.current_frame - drag.src_frame + drag.offset_frames;
        let layer_moved = drag.current_layer_idx as isize - drag.src_layer_idx as isize;

        for clip_id in &self.selected_clips {
            let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                continue;
            };
            let target_layer_idx = (layer_idx as isize + layer_moved) as usize;
            let clip = &timeline.layers[layer_idx].clips[clip_idx];
            let new_position = clip.position + frame_moved;
            if TimelineWindow::would_overlap(
                timeline,
                target_layer_idx,
                new_position,
                clip.duration,
                &self.selected_clips,
            ) {
                self.is_wrong = true;
                return;
            }
        }
    }

    pub(super) fn handle_drag_drop(&mut self, timeline: &mut Timeline, _local: Pos2) {
        let Some(drag) = &mut self.drag_state else {
            return;
        };

        let frame_moved = drag.current_frame - drag.src_frame + drag.offset_frames;
        let layer_moved = drag.current_layer_idx as isize - drag.src_layer_idx as isize;
        // range and overrap check
        for clip_id in &self.selected_clips {
            let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                self.drag_state = None;
                return;
            };
            let target_layer_idx = (layer_idx as isize + layer_moved) as usize;
            let clip = &timeline.layers[layer_idx].clips[clip_idx];
            let new_position = clip.position + frame_moved;

            if TimelineWindow::would_overlap(
                timeline,
                target_layer_idx,
                new_position,
                clip.duration,
                &self.selected_clips,
            ) {
                self.drag_state = None;
                return;
            }
        }

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
            self.drag_state = None;
            return;
        }

        // apply
        for clip_id in &self.selected_clips {
            let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) else {
                continue;
            };
            // delete old
            let mut clip = timeline.layers[layer_idx].clips.swap_remove(clip_idx);

            // update pos
            clip.position += frame_moved;

            let target_layer_idx = layer_idx as isize + layer_moved;

            let target_layer_idx = target_layer_idx as usize;

            // insert new
            timeline.layers[target_layer_idx].clips.push(clip);
        }

        self.drag_state = None;
    }

    fn would_overlap(
        timeline: &Timeline,
        layer_idx: usize,
        position: i64,
        duration: i64,
        exclude_ids: &HashSet<usize>,
    ) -> bool {
        for clip in &timeline.layers[layer_idx].clips {
            if exclude_ids.contains(&clip.id) {
                continue;
            }
            if position < clip.position + clip.duration && position + duration > clip.position {
                return true;
            }
        }
        false
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
            let color = if self.is_wrong {
                egui::Color32::from_rgba_unmultiplied(180, 70, 70, 180)
            } else {
                egui::Color32::from_rgba_unmultiplied(70, 130, 180, 180)
            };
            let stroke_color = if self.is_wrong {
                egui::Color32::from_rgb(255, 120, 120)
            } else {
                egui::Color32::from_rgb(150, 200, 255)
            };

            painter.rect_filled(ghost_rect, 3.0, color);
            painter.rect_stroke(ghost_rect, 3.0, egui::Stroke::new(2.0, stroke_color));
        }
    }
}
