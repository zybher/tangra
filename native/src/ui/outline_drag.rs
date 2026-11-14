//! Outline drag & drop: where a dragged row would land, and applying it (list order is stored
//! per object; moving into / out of a frame moves the object, since nesting is geometric).
use crate::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Stroke as EStroke};

/// Where a drop lands: under `parent` (None = board), at `pos` among its children
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Drop {
    pub parent: Option<usize>,
    /// Insert before this sibling (None = at the end); `into` drops go first
    pub before: Option<usize>,
    pub into: bool,
    /// Row index (in the visible list) used to draw the marker
    pub row: usize,
    pub after: bool,
}

/// `rows`: visible rows as (rect, tree index, depth); `parent`: tree parents; `is_frame`;
/// `src`: tree index being dragged; `next_sib`: the sibling after a tree index in its list
pub(crate) fn drop_target(
    y: f32,
    rows: &[(Rect, usize, usize)],
    parent: &[Option<usize>],
    is_frame: impl Fn(usize) -> bool,
    src: usize,
    next_sib: impl Fn(usize) -> Option<usize>,
) -> Option<Drop> {
    let (ri, &(r, j, _)) = rows.iter().enumerate().find(|(_, (r, _, _))| y >= r.min.y - 1.0 && y < r.max.y + 1.0)?;
    // Can't drop an object into itself or anything inside it
    let inside_src = |mut i: usize| loop {
        if i == src {
            return true;
        }
        match parent[i] {
            Some(p) => i = p,
            None => return false,
        }
    };
    let f = (y - r.min.y) / r.height();
    let d = if is_frame(j) && (0.28..0.72).contains(&f) {
        Drop { parent: Some(j), before: None, into: true, row: ri, after: false }
    } else if f < 0.5 {
        Drop { parent: parent[j], before: Some(j), into: false, row: ri, after: false }
    } else {
        Drop { parent: parent[j], before: next_sib(j), into: false, row: ri, after: true }
    };
    if d.parent.is_some_and(inside_src) || (d.into && j == src) {
        return None;
    }
    Some(d)
}

/// Marker for a pending drop: a line between rows, or an outline around the frame row
pub(crate) fn paint_marker(p: &egui::Painter, d: &Drop, rows: &[(Rect, usize, usize)]) {
    let (r, _, depth) = rows[d.row];
    let col = blue();
    if d.into {
        p.rect_stroke(r.shrink(0.75), 8.0, EStroke::new(1.5_f32, col));
        return;
    }
    let y = if d.after { r.max.y + 1.0 } else { r.min.y - 1.0 };
    let x0 = r.min.x + 8.0 + depth as f32 * 18.0;
    p.line_segment([pos2(x0, y), pos2(r.max.x - 4.0, y)], EStroke::new(2.0_f32, col));
    p.circle_stroke(pos2(x0 - 3.0, y), 3.0, EStroke::new(1.5_f32, col));
}

/// The dragged row's name following the pointer
pub(crate) fn paint_ghost(ctx: &egui::Context, at: egui::Pos2, name: &str, k: Kind) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("qd_outline_ghost")));
    let font = FontId::proportional(12.0);
    let w = ctx.fonts(|f| f.layout_no_wrap(name.to_string(), font.clone(), Color32::WHITE).size().x) + 40.0;
    let r = Rect::from_min_size(at + vec2(12.0, 4.0), vec2(w, 24.0));
    p.rect_filled(r.translate(vec2(0.0, 2.0)), 7.0, Color32::from_black_alpha(80));
    p.rect_filled(r, 7.0, mix(frame_bg(), blue(), 0.35));
    // What's being moved: the same kind glyph as its row
    super::icons::kind_icon(&p, k, r.left_center() + vec2(15.0, 0.0), Color32::WHITE);
    p.text(r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, name, font, Color32::WHITE);
}

impl App {
    /// Move an object (and, for a frame, everything inside it) so its box's corner `from` lands
    /// on `to`, scaled by `k` around that corner
    fn shift_tree(&mut self, ids: &[(Kind, String)], from: [f32; 2], to: [f32; 2], k: f32) {
        for (kd, id) in ids {
            if let Some([x, y, w, h]) = self.rect_of(*kd, id) {
                self.set_rect(*kd, id, [to[0] + (x - from[0]) * k, to[1] + (y - from[1]) * k, w * k, h * k]);
            }
        }
    }

    /// Apply a drop. `items`/`rects`: the tree's objects; `subtree`: src plus everything under it;
    /// `siblings`: the new parent's children in list order; `top_of_src`: src's outermost frame
    pub(crate) fn apply_outline_drop(
        &mut self,
        src: (Kind, String),
        src_rect: [f32; 4],
        old_parent: Option<usize>,
        d: Drop,
        new_parent_rect: Option<[f32; 4]>,
        top_rect: [f32; 4],
        subtree: Vec<(Kind, String)>,
        siblings: Vec<String>,
        before_id: Option<String>,
        new_parent_id: Option<String>,
    ) {
        self.snapshot();
        // Reparent: nesting is geometric, so the object moves into (or out of) the frame
        if d.parent != old_parent {
            let r = src_rect;
            let to = match new_parent_rect {
                // Keep it where it is if it already sits over that frame, else centre it (a
                // bigger object is simply clipped by the frame)
                Some(f) => {
                    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                    let over = cx >= f[0] && cx <= f[0] + f[2] && cy >= f[1] && cy <= f[1] + f[3];
                    if over { [r[0], r[1]] } else { [f[0] + (f[2] - r[2]) / 2.0, f[1] + (f[3] - r[3]) / 2.0] }
                }
                // Out to the board: just right of the frame it came from
                None => [top_rect[0] + top_rect[2] + 40.0, top_rect[1]],
            };
            self.shift_tree(&subtree, [r[0], r[1]], to, 1.0);
            self.board.parents.insert(src.1.clone(), new_parent_id.unwrap_or_default());

        }
        // List order among the new siblings
        let mut list: Vec<String> = siblings.into_iter().filter(|s| *s != src.1).collect();
        let at = if d.into { 0 } else { before_id.and_then(|b| list.iter().position(|s| *s == b)).unwrap_or(list.len()) };
        list.insert(at.min(list.len()), src.1.clone());
        for (n, id) in list.into_iter().enumerate() {
            self.board.layer_order.insert(id, n as f32);
        }
        self.mark_dirty();
    }
}
