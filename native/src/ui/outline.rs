//! Outline: the layer tree of the frame you're working in, on the left. It slides in while a
//! frame (or anything inside one) is selected and pushes the bottom-left HUD aside. `L` closes
//! it for good (until `L` again). Nesting is geometric, like the rest of the app: an object
//! belongs to the smallest frame that fully contains it.

use crate::*;
use std::collections::HashSet;

/// Panel width, dragged from its right edge (shared so the HUD and camera focus follow it)
static WIDTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const W_MIN: f32 = 200.0;
const W_MAX: f32 = 480.0;
fn width() -> f32 {
    let b = WIDTH.load(std::sync::atomic::Ordering::Relaxed);
    if b == 0 { 240.0 } else { f32::from_bits(b) }
}
pub(crate) fn set_width(w: f32) {
    WIDTH.store(w.clamp(W_MIN, W_MAX).to_bits(), std::sync::atomic::Ordering::Relaxed);
}
const GAP: f32 = 12.0;
/// Width of the collapsed bar (logo, name, open button)
const TAB_W: f32 = 150.0;

/// Sidebar glyph: a small window with its left pane marked
fn sidebar_glyph(p: &egui::Painter, c: Pos2, col: Color32) {
    let g = Rect::from_center_size(p.round_pos_to_pixels(c), vec2(14.0, 11.0));
    p.rect_stroke(g, 2.5, EStroke::new(1.2_f32, col));
    p.line_segment([pos2(g.min.x + 4.5, g.min.y + 0.5), pos2(g.min.x + 4.5, g.max.y - 0.5)], EStroke::new(1.2_f32, col));
}

/// Light "semantic" match: every word must hit the name (substring or in-order letters) or
/// a word people use for that kind of object (photo -> image, sticky -> note, ...)
fn search_hit(q: &str, name: &str, k: Kind) -> bool {
    let name = name.to_lowercase();
    let syn: &[&str] = match k {
        Kind::Image => &["image", "img", "picture", "pic", "photo", "screenshot", "png", "jpg"],
        Kind::Note => &["note", "sticky", "postit", "memo", "shape", "rectangle"],
        Kind::Link => &["link", "url", "web", "bookmark", "site"],
        Kind::Group => &["frame", "board", "artboard", "group", "container"],
        Kind::Text => &["text", "label", "title", "words", "type"],
        Kind::Stroke => &["drawing", "stroke", "pen", "line", "arrow", "sketch", "ellipse", "circle"],
    };
    q.to_lowercase().split_whitespace().all(|t| {
        if name.contains(t) || syn.iter().any(|w| w.starts_with(t)) {
            return true;
        }
        // In-order letters ("tst5" finds "Test 5") once the word is long enough to mean something
        let mut it = name.chars();
        t.chars().count() >= 3 && t.chars().all(|c| it.any(|n| n == c))
    })
}
const ROW_H: f32 = 30.0;

fn kid() -> egui::Id {
    egui::Id::new("qd_outline_k")
}

/// How far the bottom-left HUD slides right to make room for the outline (animated)
pub(crate) fn hud_shift(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(kid())).unwrap_or(0.0).clamp(0.0, 1.0) * (width() + GAP)
}

/// How far top-left readouts (F3 stats) move aside: past the panel, or past the collapsed tab
pub(crate) fn top_shift(ctx: &egui::Context) -> f32 {
    let tk = ctx.data(|d| d.get_temp::<f32>(egui::Id::new("qd_outline_tabk"))).unwrap_or(0.0);
    hud_shift(ctx).max(tk.clamp(0.0, 1.0) * (TAB_W + GAP))
}

/// The object whose outline row is under the pointer, so the canvas can highlight it
pub(crate) fn outline_hover(ctx: &egui::Context) -> Option<Sel> {
    let n = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(Sel, u64)>(egui::Id::new("qd_outline_hov"))).filter(|(_, p)| n <= p + 1).map(|(s, _)| s)
}

/// `L`: close / reopen the outline
pub(crate) fn toggle_outline(ctx: &egui::Context) {
    let id = egui::Id::new("qd_outline_closed");
    ctx.data_mut(|d| {
        let c = d.get_temp::<bool>(id).unwrap_or(false);
        d.insert_temp(id, !c);
    });
}

/// The Tangra mark: three tangram triangles forming one peak, drawn in a `size` square
pub(crate) fn paint_logo(p: &egui::Painter, c: Pos2, size: f32, a: f32) {
    let f = size / 80.0;
    let q = |x: f32, y: f32| c + vec2((x - 60.0) * f, (y - 60.0) * f);
    for (pts, col) in [
        ([(20.0, 100.0), (60.0, 20.0), (60.0, 100.0)], Color32::from_rgb(0xDC, 0x26, 0x26)),
        ([(60.0, 20.0), (100.0, 100.0), (60.0, 100.0)], Color32::from_rgb(0xF8, 0x71, 0x71)),
        ([(60.0, 60.0), (80.0, 100.0), (40.0, 100.0)], Color32::from_rgb(0x7F, 0x1D, 0x1D)),
    ] {
        p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| q(x, y)).collect(), col.gamma_multiply(a), EStroke::NONE));
    }
}

struct Tree {
    items: Vec<(Sel, [f32; 4])>,
    parent: Vec<Option<usize>>,
    kids: Vec<Vec<usize>>,
    /// Top-level objects in list order
    roots: Vec<usize>,
}

impl App {
    fn outline_tree(&self) -> Tree {
        let items: Vec<(Sel, [f32; 4])> = self.all_items().into_iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s, r))).collect();
        let pos: std::collections::HashMap<&str, usize> = items.iter().enumerate().map(|(i, (s, _))| (s.1.as_str(), i)).collect();
        let parent: Vec<Option<usize>> = items.iter().map(|(s, _)| self.parent_of(&s.1).and_then(|p| pos.get(p).copied())).collect();
        let mut kids = vec![Vec::new(); items.len()];
        for (i, p) in parent.iter().enumerate() {
            if let Some(p) = p {
                kids[*p].push(i);
            }
        }
        // Order dragged in the outline first; otherwise frames first, then the order they were
        // made in (not position, so moving or aligning an object never shuffles the list)
        let ord = &self.board.layer_order;
        let key = |i: usize| (ord.get(&items[i].0 .1).copied().unwrap_or(f32::MAX), (items[i].0 .0 != Kind::Group) as u8, i as f32, 0.0_f32);
        let cmp = |a: &usize, b: &usize| key(*a).partial_cmp(&key(*b)).unwrap_or(std::cmp::Ordering::Equal);
        for k in kids.iter_mut() {
            k.sort_by(cmp);
        }
        let mut roots: Vec<usize> = (0..items.len()).filter(|&i| parent[i].is_none()).collect();
        roots.sort_by(cmp);
        Tree { items, parent, kids, roots }
    }

    fn outline_name(&self, (k, id): &Sel) -> String {
        let b = &self.board;
        if *k != Kind::Group {
            if let Some(n) = b.layer_names.get(id).filter(|n| !n.trim().is_empty()) {
                return n.clone();
            }
        }
        let first = |s: &str| s.lines().next().unwrap_or("").trim().to_string();
        let n = match k {
            Kind::Group => b.group_cards.iter().find(|g| &g.id == id).map(|g| g.title.clone()),
            Kind::Image => b.images.iter().find(|i| &i.id == id).map(|i| i.asset_file_name.clone()),
            Kind::Note => b.sticky_notes.iter().find(|n| &n.id == id).map(|n| first(&n.text)),
            Kind::Link => b.link_cards.iter().find(|l| &l.id == id).map(|l| if l.title.is_empty() { l.url.clone() } else { l.title.clone() }),
            Kind::Text => b.world_texts.iter().find(|t| &t.id == id).map(|t| first(&t.text)),
            Kind::Stroke => self.find_stroke(id).map(|s| if s.closed_len().is_some() { "Shape".to_string() } else { "Drawing".to_string() }),
        };
        let n = n.unwrap_or_default();
        if n.is_empty() {
            match k {
                Kind::Group => "Frame",
                Kind::Image => "Image",
                Kind::Note => "Note",
                Kind::Link => "Link",
                Kind::Text => "Text",
                Kind::Stroke => "Drawing",
            }
            .to_string()
        } else {
            n
        }
    }

    /// Glide the camera to `r` (world) if it's off-screen or too small to see, keeping it
    /// clear of the outline panel
    fn outline_focus(&mut self, r: [f32; 4], canvas: Rect) {
        let cam = self.board.camera;
        let left = width() + GAP + 10.0;
        let (aw, ah) = ((canvas.width() - left - 40.0).max(80.0), (canvas.height() - 120.0).max(80.0));
        let z = cam.zoom;
        let sx = (r[0] - cam.x) * z;
        let sy = (r[1] - cam.y) * z;
        let in_view = sx >= left && sy >= 0.0 && sx + r[2] * z <= canvas.width() && sy + r[3] * z <= canvas.height();
        let tiny = r[2] * z < 48.0 && r[3] * z < 48.0;
        if in_view && !tiny {
            return;
        }
        let fit = (aw / r[2].max(1.0)).min(ah / r[3].max(1.0));
        let zoom = if tiny || fit < z { (fit * 0.7).min(1.0).max(z.min(fit * 0.7)) } else { z }.clamp(0.02, 1.5);
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        // Centre of the free area (right of the panel), in screen px from the canvas origin
        let (fx, fy) = (left + (canvas.width() - left) / 2.0, canvas.height() / 2.0);
        let to = Camera { x: cx - fx / zoom, y: cy - fy / zoom, zoom };
        self.cam_anim = Some((cam, to, self.now));
        self.mark_dirty();
    }

    /// After a duplicate: `copy` (new id) inherits `orig`'s name, and `orig` is renamed to
    /// "<base> N" with the next free number among names sharing that base (Frame 1, Frame 2, ...)
    pub(crate) fn number_duplicate(&mut self, k: Kind, orig: &str, copy: &str) {
        let cur = self.outline_name(&(k, orig.to_string()));
        if let Some(p) = self.board.parents.get(orig).cloned() {
            self.board.parents.insert(copy.to_string(), p);
        }
        if let Some(o) = self.board.opacity.get(orig).copied() {
            self.board.opacity.insert(copy.to_string(), o);
        }
        if k != Kind::Group {
            if let Some(n) = self.board.layer_names.get(orig).cloned() {
                self.board.layer_names.insert(copy.to_string(), n);
            }
        }
        let split = |n: &str| -> (String, Option<u32>) {
            match n.rsplit_once(' ') {
                Some((b, d)) if !b.is_empty() && d.chars().all(|c| c.is_ascii_digit()) && !d.is_empty() => (b.to_string(), d.parse().ok()),
                _ => (n.to_string(), None),
            }
        };
        let (base, _) = split(&cur);
        let mut top = 0;
        for s in self.all_items() {
            let (b, n) = split(&self.outline_name(&s));
            if b == base {
                top = top.max(n.unwrap_or(0));
            }
        }
        let name = format!("{base} {}", top + 1);
        if k == Kind::Group {
            if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == orig) {
                g.title = name;
            }
        } else {
            self.board.layer_names.insert(orig.to_string(), name);
        }
    }

    pub(crate) fn outline_ui(&mut self, ctx: &egui::Context, canvas: Rect) {
        let closed = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_outline_closed"))).unwrap_or(false);
        let board_scope = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_outline_board"))).unwrap_or(false);
        let tree = self.outline_tree();
        let idx = |s: &Sel| tree.items.iter().position(|(t, _)| t == s);
        let top = |mut i: usize| {
            while let Some(p) = tree.parent[i] {
                i = p;
            }
            i
        };
        // The frame being worked in: the outermost frame around the selection
        let mut root: Option<Sel> = self.sel.as_ref().and_then(idx).and_then(|i| {
            let t = top(i);
            (tree.items[t].0 .0 == Kind::Group).then(|| tree.items[t].0.clone())
        });
        let rid = egui::Id::new("qd_outline_root");
        if root.is_none() {
            root = ctx.data(|d| d.get_temp::<Sel>(rid)).filter(|r| idx(r).is_some());
        }
        // No frame to show yet: list the whole board
        let board_scope = board_scope || root.is_none();
        let show = !closed && self.edit.is_none();
        if let Some(r) = &root {
            ctx.data_mut(|d| d.insert_temp(rid, r.clone()));
        }
        // Collapsed: a small tab (logo placeholder) stays at the top-left; click or L to expand
        let tk = ease(ctx, egui::Id::new("qd_outline_tab"), closed && self.edit.is_none());
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_outline_tabk"), tk));
        if tk > 0.01 {
            egui::Area::new(egui::Id::new("qd_outline_tab_area"))
                .order(egui::Order::Middle)
                .fixed_pos(pos2(canvas.min.x + 10.0 - (1.0 - tk) * 14.0, canvas.min.y + 10.0))
                .interactable(tk > 0.5)
                .show(ctx, |ui| {
                    ui.set_opacity(tk.clamp(0.0, 1.0));
                    let (r, _) = ui.allocate_exact_size(vec2(TAB_W, 40.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(r.translate(vec2(0.0, 3.0)), 12.0, Color32::from_black_alpha(70));
                    p.rect_filled(r, 12.0, frame_bg());
                    p.rect_stroke(r, 12.0, EStroke::new(1.0_f32, border()));
                    paint_logo(p, pos2(r.min.x + 22.0, r.center().y), 18.0, 1.0);
                    p.text(pos2(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, "Tangra", FontId::proportional(13.0), Color32::from_gray(235));
                    let br = Rect::from_center_size(pos2(r.max.x - 22.0, r.center().y), vec2(26.0, 26.0));
                    let resp = ui.interact(br, egui::Id::new("qd_outline_open"), Sense::click()).on_hover_text("Show outline  ·  L");
                    let (h, pr) = hover_fx(ui.ctx(), &resp, true);
                    p.rect_filled(br.expand(1.0 * h - pr), 7.0, hover_fill(h, pr));
                    sidebar_glyph(p, br.center(), mix(TEXT_DIM, Color32::from_gray(235), h));
                    if resp.clicked() {
                        toggle_outline(ui.ctx());
                    }
                });
            if tk < 0.999 {
                ctx.request_repaint();
            }
        }
        let k = ease(ctx, egui::Id::new("qd_outline_k_ease"), show);
        ctx.data_mut(|d| d.insert_temp(kid(), k));
        if k <= 0.01 {
            return;
        }
        let ri = root.as_ref().and_then(idx);

        // Folded frames; selecting something unfolds the frames around it
        let fid = egui::Id::new("qd_outline_fold");
        let mut fold: HashSet<String> = ctx.data(|d| d.get_temp(fid)).unwrap_or_default();
        let sel_i = self.sel.as_ref().and_then(idx);
        let last_sel = egui::Id::new("qd_outline_lastsel");
        let changed = ctx.data(|d| d.get_temp::<Option<Sel>>(last_sel)).flatten() != self.sel;
        if changed {
            let mut p = sel_i.and_then(|i| tree.parent[i]);
            while let Some(i) = p {
                fold.remove(&tree.items[i].0 .1);
                p = tree.parent[i];
            }
            let s = self.sel.clone();
            ctx.data_mut(|d| d.insert_temp(last_sel, s));
        }

        // Flatten the visible rows
        let mut rows: Vec<(usize, usize)> = Vec::new();
        fn walk(t: &Tree, i: usize, d: usize, fold: &HashSet<String>, out: &mut Vec<(usize, usize)>) {
            out.push((i, d));
            if !fold.contains(&t.items[i].0 .1) {
                for &c in &t.kids[i] {
                    walk(t, c, d + 1, fold, out);
                }
            }
        }
        let qid = egui::Id::new("qd_outline_q");
        let mut query: String = ctx.data(|d| d.get_temp(qid)).unwrap_or_default();
        let q = query.trim().to_string();
        if board_scope || ri.is_none() {
            for &r in &tree.roots {
                walk(&tree, r, 0, &fold, &mut rows);
            }
        } else if let Some(ri) = ri {
            walk(&tree, ri, 0, &fold, &mut rows);
        }
        // Searching: keep matches plus the frames around them (unfolded), drop the rest
        if !q.is_empty() {
            fn mark(t: &Tree, i: usize, hit: &dyn Fn(usize) -> bool, keep: &mut HashSet<usize>) -> bool {
                let mut any = hit(i);
                for &c in &t.kids[i] {
                    any |= mark(t, c, hit, keep);
                }
                if any {
                    keep.insert(i);
                }
                any
            }
            let hit = |i: usize| search_hit(&q, &self.outline_name(&tree.items[i].0), tree.items[i].0 .0);
            let mut keep = HashSet::new();
            let tops: Vec<usize> = rows.iter().filter(|r| r.1 == 0).map(|r| r.0).collect();
            for t in &tops {
                mark(&tree, *t, &hit, &mut keep);
            }
            fn walk_all(t: &Tree, i: usize, d: usize, keep: &HashSet<usize>, out: &mut Vec<(usize, usize)>) {
                if !keep.contains(&i) {
                    return;
                }
                out.push((i, d));
                for &c in &t.kids[i] {
                    walk_all(t, c, d + 1, keep, out);
                }
            }
            rows.clear();
            for t in tops {
                walk_all(&tree, t, 0, &keep, &mut rows);
            }
        }

        // Same reveal as the canvas popup: a short slide + fade, rows cascading in after the shell
        let pos = pos2(canvas.min.x + 10.0 - (1.0 - k) * 16.0, canvas.min.y + 10.0);
        let h = (canvas.height() - 20.0).max(120.0);
        let mut pick: Option<Sel> = None;
        let mut toggle: Option<String> = None;
        let mut hov: Option<Sel> = None;
        let mut close = false;
        let mut scope: Option<bool> = None;
        let ren_id = egui::Id::new("qd_outline_ren");
        let mut ren: Option<(Sel, String)> = ctx.data(|d| d.get_temp(ren_id)).flatten();
        let mut ren_start = false;
        let mut ren_done: Option<Option<(Sel, String)>> = None;
        let mut vis: Option<(Sel, bool)> = None;
        let drag_id = egui::Id::new("qd_outline_drag");
        let mut drop_req: Option<(usize, super::outline_drag::Drop)> = None;
        // A rename whose row is gone (folded away, scope switched, panel hidden) is dropped
        if ren.as_ref().is_some_and(|(rs, _)| !show || !rows.iter().any(|&(i, _)| &tree.items[i].0 == rs)) {
            ren = None;
        }
        // Enter on a selected object starts renaming it (same as double-clicking its row)
        if ren.is_none() && show && self.prompt.is_none() && !self.shortcuts_open && !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none()) {
            if let Some(s) = self.sel.clone().filter(|s| rows.iter().any(|&(i, _)| &tree.items[i].0 == s)) {
                let name = self.outline_name(&s);
                ren = Some((s, name));
                ren_start = true;
            }
        }
        egui::Area::new(egui::Id::new("qd_outline"))
            .order(egui::Order::Middle)
            .fixed_pos(pos)
            .interactable(show)
            .show(ctx, |ui| {
                ui.set_opacity(k.clamp(0.0, 1.0));
                #[allow(non_snake_case)]
                let W = width();
                let shell = Rect::from_min_size(pos, vec2(W, h));
                ui.painter().rect_filled(shell.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
                egui::Frame::none().fill(frame_bg()).stroke(EStroke::new(1.0_f32, border())).rounding(10.0).inner_margin(6.0).show(ui, |ui| {
                    ui.set_width(W - 12.0);
                    ui.set_height(h - 12.0);
                    // Header: title, scope switch, close
                    let (hr, _) = ui.allocate_exact_size(vec2(W - 12.0, 28.0), Sense::hover());
                    let p = ui.painter().clone();
                    p.text(pos2(hr.min.x + 6.0, hr.center().y), Align2::LEFT_CENTER, "Outline", FontId::proportional(12.5), Color32::from_gray(230));
                    let mut x = hr.max.x - 24.0;
                    let xr = Rect::from_min_size(pos2(x, hr.min.y + 3.0), vec2(22.0, 22.0));
                    let xresp = ui.interact(xr, egui::Id::new("qd_outline_x"), Sense::click()).on_hover_text("Collapse  ·  L");
                    let (xh, xp) = hover_fx(ctx, &xresp, true);
                    p.rect_filled(xr.expand(1.5 * xh - xp), 6.0, hover_fill(xh, xp));
                    sidebar_glyph(&p, xr.center(), mix(TEXT_DIM, Color32::from_gray(235), xh));
                    close |= xresp.clicked();
                    let _ = x;
                    // Scope switch under the title: one shared track, a pill slides to the active half
                    ui.add_space(4.0);
                    let (tr, _) = ui.allocate_exact_size(vec2(W - 12.0, 26.0), Sense::hover());
                    let tr = tr.shrink2(vec2(4.0, 0.0));
                    p.rect_filled(tr, 7.0, Color32::from_black_alpha(60));
                    let bk = {
                        let gid = egui::Id::new("qd_outline_scope_k");
                        let t = if board_scope { 1.0 } else { 0.0 };
                        let c: f32 = ctx.data(|d| d.get_temp(gid)).unwrap_or(t);
                        let dt = ctx.input(|i| i.stable_dt).min(0.05);
                        let mut v = c + (t - c) * (1.0 - (-dt * 14.0).exp());
                        if (v - t).abs() < 0.002 {
                            v = t;
                        } else {
                            ctx.request_repaint();
                        }
                        ctx.data_mut(|d| d.insert_temp(gid, v));
                        v
                    };
                    let half = tr.width() / 2.0;
                    let pill = Rect::from_min_size(pos2(tr.min.x + 2.0 + bk * half, tr.min.y + 2.0), vec2(half - 4.0 + 0.0, tr.height() - 4.0));
                    p.rect_filled(pill, 5.5, Color32::from_white_alpha(22));
                    for (i, (label, on_board)) in [("This frame", false), ("Whole board", true)].into_iter().enumerate() {
                        let r = Rect::from_min_size(pos2(tr.min.x + i as f32 * half, tr.min.y), vec2(half, tr.height()));
                        let resp = ui.interact(r, egui::Id::new(("qd_outline_scope", label)), Sense::click());
                        let on = board_scope == on_board;
                        let (sh, sp) = hover_fx(ctx, &resp, !on);
                        if !on {
                            p.rect_filled(r.shrink(2.0), 5.5, hover_fill(sh * 0.6, sp));
                        }
                        let onk = if on_board { bk } else { 1.0 - bk };
                        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.0), mix(TEXT_DIM, Color32::from_gray(235), onk.max(sh * 0.5)));
                        if resp.clicked() {
                            scope = Some(on_board);
                        }
                    }
                    // Search
                    ui.add_space(6.0);
                    let (sr, _) = ui.allocate_exact_size(vec2(W - 12.0, 28.0), Sense::hover());
                    let sr = sr.shrink2(vec2(4.0, 0.0));
                    let te_id = egui::Id::new("qd_outline_search");
                    let focused = ctx.memory(|m| m.has_focus(te_id));
                    let fk = ease(ctx, te_id.with("f"), focused);
                    p.rect_filled(sr, 7.0, Color32::from_black_alpha(60));
                    if fk > 0.01 {
                        p.rect_stroke(sr.shrink(0.5), 7.0, EStroke::new(1.0_f32, blue().gamma_multiply(fk)));
                    }
                    // Magnifier
                    let mc = p.round_pos_to_pixels(pos2(sr.min.x + 14.0, sr.center().y - 0.5));
                    let ms = EStroke::new(1.2_f32, TEXT_DIM);
                    p.circle_stroke(mc, 4.0, ms);
                    p.line_segment([mc + vec2(2.9, 2.9), mc + vec2(5.6, 5.6)], ms);
                    let te = ui.put(
                        Rect::from_min_max(pos2(sr.min.x + 26.0, sr.min.y), pos2(sr.max.x - 24.0, sr.max.y)),
                        egui::TextEdit::singleline(&mut query).id(te_id).frame(false).hint_text("Search layers").font(FontId::proportional(11.5)).vertical_align(egui::Align::Center),
                    );
                    if te.has_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                        query.clear();
                        te.surrender_focus();
                    }
                    if !query.is_empty() {
                        let cr = Rect::from_center_size(pos2(sr.max.x - 12.0, sr.center().y), vec2(18.0, 18.0));
                        let cresp = ui.interact(cr, te_id.with("clear"), Sense::click());
                        let (ch, cp) = hover_fx(ctx, &cresp, true);
                        p.rect_filled(cr, 5.0, hover_fill(ch, cp));
                        let c = p.round_pos_to_pixels(cr.center());
                        let cs = EStroke::new(1.2_f32, mix(TEXT_DIM, Color32::from_gray(235), ch));
                        p.line_segment([c + vec2(-3.0, -3.0), c + vec2(3.0, 3.0)], cs);
                        p.line_segment([c + vec2(3.0, -3.0), c + vec2(-3.0, 3.0)], cs);
                        if cresp.clicked() {
                            query.clear();
                        }
                    }
                    ctx.data_mut(|d| d.insert_temp(qid, query.clone()));
                    // Divider edge to edge, then the layers
                    ui.add_space(8.0);
                    let dy = ui.cursor().min.y;
                    p.line_segment([pos2(shell.min.x, dy), pos2(shell.max.x, dy)], EStroke::new(1.0_f32, border()));
                    ui.add_space(6.0);
                    // Solid scrollbar: it takes its own lane instead of floating over the rows
                    {
                        let sc = &mut ui.spacing_mut().scroll;
                        *sc = egui::style::ScrollStyle::solid();
                        sc.bar_width = 5.0;
                        sc.bar_inner_margin = 3.0;
                        // Handle only, like Figma: no visible track
                        sc.dormant_background_opacity = 0.0;
                        sc.active_background_opacity = 0.0;
                        sc.interact_background_opacity = 0.0;
                        sc.dormant_handle_opacity = 0.55;
                        sc.active_handle_opacity = 0.8;
                        sc.interact_handle_opacity = 1.0;
                    }
                    ui.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
                    egui::ScrollArea::vertical().auto_shrink([false, false]).drag_to_scroll(false).max_height(ui.available_height() - 36.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        // Toolbar-style wave: rows near the pointer lift into a tile that slides right
                        let pitch = ROW_H + 2.0;
                        let y0 = ui.cursor().min.y + ROW_H / 2.0;
                        let area = ui.clip_rect().intersect(shell);
                        let _ = (area, y0);
                        // Pass 1: lay out every row (wave lift, cascade) and paint the hover tiles
                        let top0 = ui.cursor().min.y;
                        let mut laid: Vec<(Rect, f32, f32, egui::Response)> = Vec::with_capacity(rows.len());
                        for (ri_, &(i, _)) in rows.iter().enumerate() {
                            let (s, _) = &tree.items[i];
                            let (r0, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
                            // Cascade in after the shell, top row first
                            let e = (k * 1.7 - (ri_ as f32 * 0.04).min(0.6)).clamp(0.0, 1.0);
                            let e = 1.0 - (1.0 - e).powi(3);
                            let renaming = ren.as_ref().is_some_and(|(rs, _)| rs == s);
                            // Hover lift; a row being renamed stays lifted, and easing in/out of it means
                            // no snap when the field opens or closes (the field steals the row's hover)
                            let m = ease(ctx, resp.id.with("lift"), resp.hovered() || renaming);
                            let r = Rect::from_min_max(pos2(r0.min.x + 4.0 + 6.0 * m - (1.0 - e) * 10.0, r0.min.y), pos2(r0.max.x - 2.0 + 2.0 * m, r0.max.y));
                            // Selected rows never get the grey hover tile: selection colour wins
                            if !self.selected.contains(s) {
                                wave_tile(&ui.painter().with_clip_rect(ui.clip_rect()), r, m, mix(popup(), Color32::from_gray(90), 0.35), e);
                            }
                            laid.push((r, m, e, resp));
                        }
                        // Pass 2: the selection pill glides between rows (same rate as the hotkey list's
                        // pill), on top of the tiles and following each row's wave shift
                        {
                            let target = rows.iter().position(|&(i, _)| Some(&tree.items[i].0) == self.sel.as_ref());
                            let gid = egui::Id::new("qd_outline_glide");
                            let pa = ease(ctx, gid.with("a"), target.is_some());
                            let cur: Option<f32> = ctx.data(|d| d.get_temp(gid));
                            let v = match (target.map(|n| n as f32), cur) {
                                // Coming back from hidden: appear in place instead of flying across
                                (Some(t), Some(c)) if pa > 0.05 => {
                                    let dt = ctx.input(|i| i.stable_dt).min(0.05);
                                    let v = c + (t - c) * (1.0 - (-dt * 16.0).exp());
                                    if (v - t).abs() < 0.01 { t } else { ctx.request_repaint(); v }
                                }
                                (Some(t), _) => t,
                                (None, c) => c.unwrap_or(0.0),
                            };
                            ctx.data_mut(|d| d.insert_temp(gid, v));
                            if pa > 0.01 && !laid.is_empty() {
                                let lo = (v.floor().max(0.0) as usize).min(laid.len() - 1);
                                let hi = (lo + 1).min(laid.len() - 1);
                                let f = (v - lo as f32).clamp(0.0, 1.0);
                                let (a, b) = (laid[lo].0, laid[hi].0);
                                let x0 = a.min.x + (b.min.x - a.min.x) * f;
                                let x1 = a.max.x + (b.max.x - a.max.x) * f;
                                let pill = Rect::from_min_max(pos2(x0, top0 + v * pitch), pos2(x1, top0 + v * pitch + ROW_H));
                                ui.painter().with_clip_rect(ui.clip_rect()).rect_filled(pill, 8.0, blue().gamma_multiply(0.22 * pa * k));
                            }
                        }
                        // Pass 3: row contents on top
                        let hid_all = self.hidden_ids();
                        let lock_all = self.blocked_ids();
                        let mut drag_rows: Vec<(Rect, usize, usize)> = Vec::with_capacity(rows.len());
                        for (&(i, d), (r, m, e, resp)) in rows.iter().zip(laid) {
                            drag_rows.push((r, i, d));
                            if resp.drag_started() && ren.is_none() {
                                ctx.data_mut(|dd| dd.insert_temp(drag_id, Some(tree.items[i].0.clone())));
                            }
                            let (s, _) = &tree.items[i];
                            let p = ui.painter().with_clip_rect(ui.clip_rect());
                            let selected = self.selected.contains(s);
                            let sk = ease(ctx, egui::Id::new(("qd_outline_selk", &s.1)), selected);
                            // Extra selected rows (multi-select) just tint; the primary one is the pill
                            if sk > 0.01 && Some(s) != self.sel.as_ref() {
                                p.rect_filled(r, 8.0, blue().gamma_multiply(0.22 * sk * e));
                            }
                            let mut p = p.with_clip_rect(r.intersect(ui.clip_rect()));
                            p.multiply_opacity(e);
                            if selected && changed {
                                resp.scroll_to_me(None);
                            }
                            let x0 = r.min.x + 8.0 + d as f32 * 18.0;
                            let cy = r.center().y;
                            let has = !tree.kids[i].is_empty();
                            let folded = fold.contains(&s.1);
                            if has {
                                let c = pos2(x0 + 5.0, cy);
                                let st = EStroke::new(1.3_f32, TEXT_DIM);
                                if folded {
                                    p.line_segment([c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0)], st);
                                    p.line_segment([c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)], st);
                                } else {
                                    p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0)], st);
                                    p.line_segment([c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)], st);
                                }
                            }
                            // Kind icon
                            let ic = pos2(x0 + 24.0, cy);
                            let icol = mix(Color32::from_gray(140), blue(), sk);
                            super::icons::kind_icon(&p, s.0, ic, icol);
                            let name = self.outline_name(s);
                            // Hidden (or inside a hidden frame) rows fade
                            let dim = if hid_all.contains(&s.1) { 0.4 } else { 1.0 };
                            let tc = mix(mix(TEXT_DIM, TEXT, 0.6 + 0.4 * m), Color32::WHITE, sk).gamma_multiply(dim);
                            // Eye + lock on the right: shown on hover, or always while switched on
                            let own_h = self.board.hidden.contains(&s.1);
                            let own_l = self.board.locked.contains(&s.1);
                            let show_ic = resp.hovered() || own_h || own_l;
                            let icon_w = if show_ic { 44.0 } else { 0.0 };
                            let lock_c = pos2(r.max.x - 34.0, cy);
                            let eye_c = pos2(r.max.x - 14.0, cy);
                            let ptr = resp.hover_pos().or(resp.interact_pointer_pos());
                            let on_eye = show_ic && ptr.is_some_and(|q| (q.x - eye_c.x).abs() < 9.0);
                            let on_lock = show_ic && ptr.is_some_and(|q| (q.x - lock_c.x).abs() < 9.0);
                            if show_ic {
                                let ic = |on: bool, over: bool| Color32::from_gray(if over { 240 } else if on { 205 } else { 150 });
                                let locked_eff = own_l || (lock_all.contains(&s.1) && !hid_all.contains(&s.1));
                                super::icons::lock(&p, lock_c, 15.0, ic(locked_eff, on_lock), locked_eff);
                                super::icons::eye(&p, eye_c, 15.0, ic(own_h, on_eye), own_h);
                            }
                            // Double-click: rename in place (Enter / click away saves, Esc cancels)
                            let on_caret = has && resp.interact_pointer_pos().is_some_and(|q| q.x < x0 + 15.0);
                            if resp.double_clicked() && !on_caret && !on_eye && !on_lock {
                                ren = Some((s.clone(), name.clone()));
                                ren_start = true;
                            }
                            if let Some((rs, buf)) = ren.as_mut().filter(|(rs, _)| rs == s) {
                                // Figma-style: the whole row lights up, the field fills it inside a 2 px inset
                                let er = Rect::from_min_max(pos2(x0 + 33.0, r.min.y + 3.0), pos2(r.max.x - 3.0, r.max.y - 2.0));
                                p.rect_stroke(er.shrink(0.5), 6.0, EStroke::new(1.0_f32, blue()));
                                let te = ui.put(
                                    er.shrink2(vec2(4.0, 0.0)),
                                    egui::TextEdit::singleline(buf).frame(false).font(FontId::proportional(12.0)).text_color(Color32::WHITE).vertical_align(egui::Align::Center),
                                );
                                if ren_start {
                                    te.request_focus();
                                    if let Some(mut st) = egui::TextEdit::load_state(ctx, te.id) {
                                        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(buf.chars().count()))));
                                        st.store(ctx, te.id);
                                    }
                                }
                                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                                    ren_done = Some(None);
                                } else if te.lost_focus() && !ren_start {
                                    ren_done = Some(Some((rs.clone(), buf.clone())));
                                }
                            } else {
                                // Long names end in an ellipsis before the folded-count badge
                                let room = r.max.x - (x0 + 37.0) - icon_w - if has && folded { 26.0 } else { 8.0 };
                                let font = FontId::proportional(12.0);
                                let wid = |t: String| ui.fonts(|f| f.layout_no_wrap(t, font.clone(), tc).size().x);
                                let mut shown = name.clone();
                                if wid(shown.clone()) > room {
                                    let mut cs: Vec<char> = name.chars().collect();
                                    while !cs.is_empty() && wid(cs.iter().collect::<String>() + "…") > room {
                                        cs.pop();
                                    }
                                    shown = cs.into_iter().collect::<String>().trim_end().to_string() + "…";
                                }
                                p.text(pos2(x0 + 37.0, cy), Align2::LEFT_CENTER, shown, font.clone(), tc);
                            }
                            if has && folded {
                                p.text(pos2(r.max.x - 8.0 - icon_w, cy), Align2::RIGHT_CENTER, tree.kids[i].len().to_string(), FontId::proportional(10.5), Color32::from_gray(110));
                            }
                            if resp.hovered() {
                                hov = Some(s.clone());
                            }
                            if resp.clicked() {
                                if on_eye || on_lock {
                                    vis = Some((s.clone(), on_lock));
                                } else if on_caret {
                                    toggle = Some(s.1.clone());
                                } else {
                                    pick = Some(s.clone());
                                }
                            }
                        }
                        // Dragging a row: show where it would land; let go to drop
                        let dragging: Option<Sel> = ctx.data(|dd| dd.get_temp::<Option<Sel>>(drag_id)).flatten();
                        if let Some(src) = dragging.filter(|s| idx(s).is_some()) {
                            let si = idx(&src).unwrap();
                            let ptr = ctx.input(|i| i.pointer.interact_pos());
                            let next_sib = |j: usize| {
                                let list = match tree.parent[j] { Some(p) => &tree.kids[p], None => &tree.roots };
                                list.iter().position(|&x| x == j).and_then(|n| list.get(n + 1).copied())
                            };
                            let target = ptr.and_then(|q| {
                                super::outline_drag::drop_target(q.y, &drag_rows, &tree.parent, |j| tree.items[j].0 .0 == Kind::Group, si, next_sib)
                            });
                            if let Some(t) = &target {
                                super::outline_drag::paint_marker(&ui.painter().with_clip_rect(ui.clip_rect()), t, &drag_rows);
                            }
                            if let Some(q) = ptr {
                                super::outline_drag::paint_ghost(ctx, q, &self.outline_name(&src), src.0);
                            }
                            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                            // Near the top / bottom edge the list scrolls (faster the closer you get)
                            if let Some(q) = ptr {
                                let vr = ui.clip_rect();
                                let edge = 36.0;
                                let up = (vr.min.y + edge - q.y).clamp(0.0, edge * 2.0);
                                let down = (q.y - (vr.max.y - edge)).clamp(0.0, edge * 2.0);
                                if up > 0.0 || down > 0.0 {
                                    let dt = ctx.input(|i| i.stable_dt).min(0.05);
                                    ui.scroll_with_delta(vec2(0.0, (up - down) * 14.0 * dt));
                                }
                            }
                            if ctx.input(|i| i.pointer.any_released() || !i.pointer.any_down()) {
                                ctx.data_mut(|dd| dd.remove::<Option<Sel>>(drag_id));
                                drop_req = target.map(|t| (si, t));
                            }
                            ctx.request_repaint();
                        }
                    });
                });
                // Right edge: drag to resize
                let er = Rect::from_min_max(pos2(shell.max.x - 4.0, shell.min.y + 12.0), pos2(shell.max.x + 4.0, shell.max.y - 12.0));
                let eresp = ui.interact(er, egui::Id::new("qd_outline_resize"), Sense::drag());
                let ek = ease(ctx, egui::Id::new("qd_outline_resize_k"), eresp.hovered() || eresp.dragged());
                if eresp.hovered() || eresp.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if eresp.dragged() {
                    set_width(W + eresp.drag_delta().x);
                    ctx.request_repaint();
                }
                // Remembered across restarts (saved once, when the drag ends)
                if eresp.drag_stopped() {
                    self.settings.outline_width = width();
                    save_settings(&self.settings);
                }
                if ek > 0.01 {
                    ui.painter().line_segment([pos2(shell.max.x - 0.5, shell.min.y + 14.0), pos2(shell.max.x - 0.5, shell.max.y - 14.0)], EStroke::new(2.0_f32, blue().gamma_multiply(ek)));
                }
                // Footer: the Tangra mark, bottom-left
                let p = ui.painter();
                let fy = shell.max.y - 22.0;
                p.line_segment([pos2(shell.min.x + 10.0, fy - 16.0), pos2(shell.max.x - 10.0, fy - 16.0)], EStroke::new(1.0_f32, border()));
                paint_logo(p, pos2(shell.min.x + 22.0, fy), 18.0, 1.0);
                p.text(pos2(shell.min.x + 38.0, fy), Align2::LEFT_CENTER, "Tangra", FontId::proportional(12.0), TEXT_DIM);
            });

        if let Some(id) = toggle {
            if !fold.remove(&id) {
                fold.insert(id);
            }
        }
        ctx.data_mut(|d| d.insert_temp(fid, fold));
        if let Some(b) = scope {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_outline_board"), b));
        }
        if close {
            toggle_outline(ctx);
        }
        let n = ctx.cumulative_pass_nr();
        match hov {
            Some(s) => ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_outline_hov"), (s, n))),
            None => ctx.data_mut(|d| d.remove::<(Sel, u64)>(egui::Id::new("qd_outline_hov"))),
        }
        match ren_done {
            Some(done) => {
                ctx.data_mut(|d| d.remove::<Option<(Sel, String)>>(ren_id));
                if let Some(((k, id), name)) = done {
                    let name = name.trim().to_string();
                    if name != self.outline_name(&(k, id.clone())) {
                        self.snapshot();
                        if k == Kind::Group {
                            if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                                g.title = name;
                            }
                        } else if name.is_empty() {
                            self.board.layer_names.remove(&id);
                        } else {
                            self.board.layer_names.insert(id, name);
                        }
                        self.mark_dirty();
                    }
                }
            }
            None => ctx.data_mut(|d| d.insert_temp(ren_id, ren)),
        }
        if let Some(((k, id), lock)) = vis {
            self.toggle_vis(k, &id, lock);
        }
        if let Some((si, d)) = drop_req {
            let mut sub = Vec::new();
            let mut stack = vec![si];
            while let Some(i) = stack.pop() {
                sub.push(tree.items[i].0.clone());
                stack.extend(tree.kids[i].iter().copied());
            }
            let mut top_i = si;
            while let Some(p) = tree.parent[top_i] {
                top_i = p;
            }
            let siblings: Vec<String> = match d.parent { Some(p) => &tree.kids[p], None => &tree.roots }.iter().map(|&i| tree.items[i].0 .1.clone()).collect();
            self.apply_outline_drop(
                tree.items[si].0.clone(),
                tree.items[si].1,
                tree.parent[si],
                d,
                d.parent.map(|p| tree.items[p].1),
                tree.items[top_i].1,
                sub,
                siblings,
                d.before.map(|b| tree.items[b].0 .1.clone()),
                d.parent.map(|p| tree.items[p].0 .1.clone()),
            );
            pick = None;
        }
        if let Some(s) = pick {
            if let Some(i) = idx(&s) {
                self.outline_focus(tree.items[i].1, canvas);
            }
            self.select_only(s);
        }
        if k > 0.01 && k < 0.999 {
            ctx.request_repaint();
        }
    }
}
