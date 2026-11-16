//! Outline drag & drop: where a dragged row would land, and applying it (list order is stored
//! per object; moving into / out of a frame changes which frame it belongs to).
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
/// `src`: tree indices being dragged (one row, or every selected one); `next_sib`: the sibling after a tree index in its list
pub(crate) fn drop_target(
    y: f32,
    rows: &[(Rect, usize, usize)],
    parent: &[Option<usize>],
    is_frame: impl Fn(usize) -> bool,
    src: &[usize],
    next_sib: impl Fn(usize) -> Option<usize>,
) -> Option<Drop> {
    let (ri, &(r, j, dep)) = rows.iter().enumerate().find(|(_, (r, _, _))| y >= r.min.y - 1.0 && y < r.max.y + 1.0)?;
    // Can't drop an object into itself or anything inside it
    let inside_src = |mut i: usize| loop {
        if src.contains(&i) {
            return true;
        }
        match parent[i] {
            Some(p) => i = p,
            None => return false,
        }
    };
    let f = (y - r.min.y) / r.height();
    // An open frame with its rows showing: its whole lower half drops in too (it lands on top
    // of its children), like Figma
    let open = rows.get(ri + 1).is_some_and(|&(_, k, kd)| kd > dep && parent[k] == Some(j));
    let d = if is_frame(j) && (0.2..if open { 1.0 } else { 0.8 }).contains(&f) {
        Drop { parent: Some(j), before: None, into: true, row: ri, after: false }
    } else if f < 0.5 {
        Drop { parent: parent[j], before: Some(j), into: false, row: ri, after: false }
    } else {
        Drop { parent: parent[j], before: next_sib(j), into: false, row: ri, after: true }
    };
    if d.parent.is_some_and(inside_src) || (d.into && src.contains(&j)) {
        return None;
    }
    Some(d)
}

/// Marker for a pending drop: a line between rows, or an outline around the frame row
pub(crate) fn paint_marker(p: &egui::Painter, d: &Drop, rows: &[(Rect, usize, usize)]) {
    let (r, _, depth) = rows[d.row];
    let col = blue();
    let ctx = p.ctx();
    let id = egui::Id::new("qd_ol_drop_marker");
    // Dropping into a frame: the line fades into an outline around the frame row
    let ik = ease(ctx, id.with("into"), d.into);
    if ik > 0.01 {
        p.rect_stroke(r.shrink(0.75), 8.0, EStroke::new(1.5_f32, col.gamma_multiply(ik)));
    }
    if ik > 0.99 {
        return;
    }
    // The line glides between row seams instead of jumping (ui/micro.rs)
    let y = if d.after { r.max.y + 1.0 } else { r.min.y - 1.0 };
    let x0 = r.min.x + 8.0 + depth as f32 * 18.0;
    let y = super::micro::glide(ctx, id.with("y"), y, false);
    let x0 = super::micro::glide(ctx, id.with("x"), x0, false);
    let lc = col.gamma_multiply(1.0 - ik);
    p.line_segment([pos2(x0, y), pos2(r.max.x - 4.0, y)], EStroke::new(2.0_f32, lc));
    p.circle_stroke(pos2(x0 - 3.0, y), 3.0, EStroke::new(1.5_f32, lc));
}

/// What a drag of several rows is called on its ghost: "3 Texts selected", "2 Rectangles
/// selected", mixed kinds "5 layers selected"
pub(crate) fn many_label(kinds: &[(Kind, Option<&'static str>)]) -> String {
    let n = kinds.len();
    let same_kind = kinds.iter().all(|k| k.0 == kinds[0].0);
    let same_tool = same_kind && kinds.iter().all(|k| k.1 == kinds[0].1);
    let noun = match kinds[0] {
        _ if !same_kind => "layers",
        (Kind::Stroke, t) if same_tool => match t.unwrap_or("") {
            "rectangle" => "Rectangles",
            "ellipse" => "Ellipses",
            "arrow" => "Arrows",
            "highlighter" => "Highlights",
            _ => "Drawings",
        },
        (Kind::Stroke, _) => "Shapes",
        (Kind::Group, _) => "Frames",
        (Kind::Image, _) => "Images",
        (Kind::Note, _) => "Notes",
        (Kind::Link, _) => "Links",
        (Kind::Text, _) => "Texts",
    };
    format!("{n} {noun} selected")
}

/// The dragged row's name (or how many are going) following the pointer
pub(crate) fn paint_ghost(ctx: &egui::Context, at: egui::Pos2, name: &str, k: Kind, tool: Option<&'static str>) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("qd_outline_ghost")));
    let font = FontId::proportional(12.0);
    let w = ctx.fonts(|f| f.layout_no_wrap(name.to_string(), font.clone(), Color32::WHITE).size().x) + 40.0;
    let r = Rect::from_min_size(at + vec2(12.0, 4.0), vec2(w, 24.0));
    p.rect_filled(r.translate(vec2(0.0, 2.0)), 7.0, Color32::from_black_alpha(80));
    p.rect_filled(r, 7.0, mix(frame_bg(), blue(), 0.35));
    // What's being moved: the same kind glyph as its row
    super::icons::kind_icon_named(&p, k, tool, r.left_center() + vec2(15.0, 0.0), Color32::WHITE);
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

    /// Apply a drop of one or more rows (each with everything under it), kept together in their
    /// list order. `siblings`: the new parent's children in list order
    pub(crate) fn apply_outline_drop(&mut self, movers: Vec<Mover>, d: Drop, new_parent_rect: Option<[f32; 4]>, siblings: Vec<String>, before_id: Option<String>, new_parent_id: Option<String>) {
        if movers.is_empty() {
            return;
        }
        let moving = |s: &String| movers.iter().any(|m| m.sel.1 == *s);
        // List order among the new siblings: the movers go together before `before_id` (or the
        // first sibling after it that isn't moving)
        let mut list: Vec<String> = siblings.iter().filter(|s| !moving(s)).cloned().collect();
        let at = if d.into {
            0
        } else {
            before_id
                .and_then(|b| siblings.iter().position(|s| *s == b))
                .and_then(|i| siblings[i..].iter().find(|s| !moving(s)))
                .and_then(|b| list.iter().position(|s| s == b))
                .unwrap_or(list.len())
        };
        for (n, m) in movers.iter().enumerate() {
            list.insert((at + n).min(list.len()), m.sel.1.clone());
        }
        // Dropped back where it was: nothing to do (and no undo step)
        if movers.iter().all(|m| m.old_parent == d.parent) && list == siblings {
            return;
        }
        self.snapshot();
        // Reparent: the objects move into (or out of) the frame together, as one block
        let re: Vec<&Mover> = movers.iter().filter(|m| m.old_parent != d.parent).collect();
        if !re.is_empty() {
            let u = re.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, m| {
                let r = m.rect;
                [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[0] + r[2]), a[3].max(r[1] + r[3])]
            });
            let r = [u[0], u[1], u[2] - u[0], u[3] - u[1]];
            let to = match new_parent_rect {
                // Keep them where they are if they already sit over that frame, else centre them
                // (bigger ones are simply clipped by the frame)
                Some(f) => {
                    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                    let over = cx >= f[0] && cx <= f[0] + f[2] && cy >= f[1] && cy <= f[1] + f[3];
                    if over { [r[0], r[1]] } else { [f[0] + (f[2] - r[2]) / 2.0, f[1] + (f[3] - r[3]) / 2.0] }
                }
                // Out to the board: just right of the frame they came from
                None => [re[0].top_rect[0] + re[0].top_rect[2] + 40.0, re[0].top_rect[1]],
            };
            let (dx, dy) = (to[0] - r[0], to[1] - r[1]);
            // A block wider / taller than the frame it's going into closes up (gaps only, sizes
            // stay) so every piece's centre lands inside the frame; otherwise the pieces that
            // ended up outside it would be sent back to the board
            let s = match new_parent_rect {
                Some(f) if r[2] > f[2] || r[3] > f[3] => (f[2] / r[2].max(1e-3)).min(f[3] / r[3].max(1e-3)).min(1.0),
                _ => 1.0,
            };
            // closing up works around the block's centre, so a too-big block centres on the frame
            let (dx, dy) = match new_parent_rect {
                Some(f) if s < 1.0 => (f[0] + f[2] / 2.0 - (r[0] + r[2] / 2.0), f[1] + f[3] / 2.0 - (r[1] + r[3] / 2.0)),
                _ => (dx, dy),
            };
            let (bcx, bcy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let jobs: Vec<(Vec<(Kind, String)>, [f32; 2], String)> = re.iter().map(|m| (m.subtree.clone(), [m.rect[0], m.rect[1]], m.sel.1.clone())).collect();
            for (m, (sub, at, id)) in re.iter().zip(jobs) {
                let (mcx, mcy) = (m.rect[0] + m.rect[2] / 2.0, m.rect[1] + m.rect[3] / 2.0);
                let (ox, oy) = ((bcx + (mcx - bcx) * s) - mcx, (bcy + (mcy - bcy) * s) - mcy);
                self.shift_tree(&sub, at, [at[0] + dx + ox, at[1] + dy + oy], 1.0);
                self.board.parents.insert(id, new_parent_id.clone().unwrap_or_default());
            }
        }
        for (n, id) in list.into_iter().enumerate() {
            self.board.layer_order.insert(id, n as f32);
        }
        self.mark_dirty();
    }
}

/// One dragged row: the object, its box, its old parent (tree index), its outermost frame's box
/// and itself plus everything under it
pub(crate) struct Mover {
    pub sel: (Kind, String),
    pub rect: [f32; 4],
    pub old_parent: Option<usize>,
    pub top_rect: [f32; 4],
    pub subtree: Vec<(Kind, String)>,
}
