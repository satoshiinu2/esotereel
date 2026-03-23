pub(super) mod drag;
pub(super) mod draw;
pub(super) mod scroll;
pub(super) mod select;

use std::collections::{HashMap, HashSet};

use egui::{Pos2, Rect};

use super::WindowBehavior;
use crate::project::{
    Project,
    clip::{Clip, ClipDragState},
    timeline::Timeline,
};

pub const LAYER_HEIGHT: f32 = 32.0;
pub const RULER_HEIGHT: f32 = 24.0;
pub const LABEL_WIDTH: f32 = 80.0;
pub const SCROLLBAR_SIZE: f32 = 12.0;

pub const DEFAULT_FRAME_COUNT: i64 = 300;
pub const DEFAULT_LAYER_LEN: i64 = 1;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimelineType {
    Main,
    Temp,
}

pub struct TimelineWindow {
    pub timeline_type: TimelineType,
    pub zoom: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,

    pub selected_clips: HashSet<usize>, // (layer_idx, clip_idx)
    pub(super) selection_rect: Option<SelectionRect>,
    pub(super) drag_state: Option<ClipDragState>,
    pub is_wrong: bool,

    was_primary_down: bool,
}
pub(super) struct SelectionRect {
    pub start: Pos2,
    pub current: Pos2,
}

impl TimelineWindow {
    pub fn new(timeline_type: TimelineType) -> Self {
        Self {
            timeline_type,
            zoom: 4.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            drag_state: None,
            selected_clips: HashSet::new(),
            was_primary_down: false,
            is_wrong: false,
            selection_rect: None,
        }
    }
}

impl WindowBehavior for TimelineWindow {
    fn title(&self) -> String {
        return match self.timeline_type {
            TimelineType::Main => "Timeline".to_string(),
            TimelineType::Temp => "Temp Timeline".to_string(),
        };
    }

    fn size(&self) -> [f32; 2] {
        [800.0, 300.0]
    }

    fn update(&mut self, project: &mut Option<Project>, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Timeline");

            let available = ui.available_size();

            let timeline_size =
                egui::vec2(available.x - SCROLLBAR_SIZE, available.y - SCROLLBAR_SIZE);

            let (response, painter) =
                ui.allocate_painter(timeline_size, egui::Sense::click_and_drag());
            let rect = response.rect;

            self.draw(project, timeline_size, &response, &painter, rect);
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

    // return val: (layer_idx, clip_idx, clip)
    pub(super) fn clip_at<'a>(
        &self,
        timeline: &'a Timeline,
        local: &Pos2,
    ) -> Option<(usize, usize, &'a Clip)> {
        let frame = self.x_to_frame(local.x);
        let layer_idx = ((local.y - RULER_HEIGHT + self.scroll_y) / LAYER_HEIGHT) as usize;

        if layer_idx >= timeline.layers.len() {
            return None;
        }

        for (clip_idx, clip) in timeline.layers[layer_idx].clips.iter().enumerate() {
            if frame >= clip.position && frame < clip.position + clip.duration {
                return Some((layer_idx, clip_idx, clip));
            }
        }

        None
    }

    pub(super) fn handle_clip_ctrl(
        &mut self,
        timeline: &mut Timeline,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        let Some(pos) = response.ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };

        let primary_down = response.ctx.input(|i| i.pointer.primary_down());
        let delete = response.ctx.input(|i| i.key_pressed(egui::Key::Delete));
        let ctrl = response.ctx.input(|i| i.modifiers.ctrl);
        let local = (pos - rect.min).to_pos2();

        let is_primary_up = self.was_primary_down && !primary_down;
        self.was_primary_down = primary_down;

        if self.drag_state.is_some() {
            self.check_edge_scroll(local, rect);
        } else if primary_down {
            self.handle_ctrl_playhead(timeline, local);
        }

        if response.drag_started() {
            let result = self.handle_drag_grab(timeline, local, ctrl);
            if !result {
                self.handle_area_sel_start(local, ctrl);
            }
        }

        if response.dragged() {
            if self.selection_rect.is_none() {
                self.handle_drag_continue(timeline, local);
            } else {
                self.handle_area_sel_continue(local);
            }
        }

        if response.drag_stopped() || is_primary_up {
            if self.selection_rect.is_none() {
                self.handle_drag_drop(timeline, local);
            } else {
                self.handle_area_sel_drop(timeline);
            }
        }

        if response.clicked_by(egui::PointerButton::Primary) {
            self.handle_select_clip(timeline, local, ctrl);
        }

        if delete {
            self.handle_delete_selected(timeline);
        }
    }

    fn handle_delete_selected(&mut self, timeline: &mut Timeline) {
        // layer_idxごとにclip_idxを降順でまとめて削除（インデックスずれ防止）
        let mut by_layer: std::collections::HashMap<usize, Vec<usize>> = HashMap::new();
        for clip_id in &self.selected_clips {
            if let Some((layer_idx, clip_idx)) = timeline.find_clip_by_id(*clip_id) {
                by_layer.entry(layer_idx).or_default().push(clip_idx);
            }
        }
        for (layer_idx, mut clip_indices) in by_layer {
            clip_indices.sort_unstable_by(|a, b| b.cmp(a)); // 降順
            for clip_idx in clip_indices {
                if layer_idx < timeline.layers.len()
                    && clip_idx < timeline.layers[layer_idx].clips.len()
                {
                    timeline.layers[layer_idx].clips.remove(clip_idx);
                }
            }
        }

        self.selected_clips.clear();
    }

    fn handle_ctrl_playhead(&self, timeline: &mut Timeline, local: Pos2) {
        if local.y > RULER_HEIGHT {
            return;
        }

        let frame = self.x_to_frame(local.x);
        timeline.playhead = frame;
    }

    fn check_edge_scroll(&mut self, local: Pos2, rect: egui::Rect) {
        let edge_zone = 40.0; // エッジから何px以内でスクロールするか
        let max_speed = 8.0;

        if local.x < edge_zone {
            let speed = (edge_zone - local.x) / edge_zone * max_speed;
            self.scroll_x = (self.scroll_x - speed).max(0.0);
        }
        if local.x > rect.width() - edge_zone {
            let speed = (local.x - (rect.width() - edge_zone)) / edge_zone * max_speed;
            self.scroll_x += speed;
        }
    }
}
