use egui::Pos2;

use crate::{
    project::{Project, timeline::Timeline},
    ui::timeline::{LAYER_HEIGHT, SelectionRect, TimelineWindow},
};

impl TimelineWindow {
    // return true if it was selected
    pub(super) fn handle_select_clip(
        &mut self,
        timeline: &mut Timeline,
        local: Pos2,
        ctrl: bool,
    ) -> bool {
        let Some((_layer_idx, _clip_idxx, clip)) = self.clip_at(&timeline, &local) else {
            self.selected_clips.clear();
            return false;
        };

        if !ctrl {
            self.selected_clips.clear();
        }
        self.selected_clips.insert(clip.id);
        return true;
    }

    pub(super) fn handle_area_sel_start(&mut self, local: Pos2, ctrl: bool) {
        if !ctrl {
            self.selected_clips.clear();
        }
        self.selection_rect = Some(SelectionRect {
            start: local,
            current: local,
        });
    }

    pub(super) fn handle_area_sel_continue(&mut self, local: Pos2) {
        if let Some(sel) = &mut self.selection_rect {
            sel.current = local;
        }
    }

    pub(super) fn handle_area_sel_drop(&mut self, timeline: &mut Timeline) {
        let Some(sel) = &self.selection_rect else {
            return;
        };

        let sel_rect = egui::Rect::from_two_pos(sel.start, sel.current);

        for (layer_idx, layer) in timeline.layers.iter().enumerate() {
            for (_clip_idx, clip) in layer.clips.iter().enumerate() {
                let clip_x_start = self.frame_to_x(clip.position);
                let clip_x_end = self.frame_to_x(clip.position + clip.duration);
                let clip_y_start = self.layer_to_y(layer_idx);
                let clip_y_end = clip_y_start + LAYER_HEIGHT;

                let clip_rect = egui::Rect::from_min_max(
                    egui::pos2(clip_x_start, clip_y_start),
                    egui::pos2(clip_x_end, clip_y_end),
                );

                if sel_rect.intersects(clip_rect) {
                    self.selected_clips.insert(clip.id);
                }
            }
        }

        self.selection_rect = None;
    }
}
