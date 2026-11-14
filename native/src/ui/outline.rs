//! Outline: the layer tree of the frame you're working in, on the left. It slides in while a
//! frame (or anything inside one) is selected and pushes the bottom-left HUD aside. `L` closes
//! it for good (until `L` again). Nesting is geometric, like the rest of the app: an object
//! belongs to the smallest frame that fully contains it.

use crate::*;
use std::collections::HashSet;

const W: f32 = 240.0;
const GAP: f32 = 12.0;
const ROW_H: f32 = 30.0;

fn kid() -> egui::Id {
    egui::Id::new("qd_outline_k")
}

/// How far the bottom-left HUD slides right to make room for the outline (animated)
pub(crate) fn hud_shift(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(kid())).unwrap_or(0.0).clamp(0.0, 1.0) * (W + GAP)
}

/// How far top-left readouts (F3 stats) move aside: past the panel, or past the collapsed tab
pub(crate) fn top_shift(ctx: &egui::Context) -> f32 {
    let tk = ctx.data(|d| d.get_temp::<f32>(egui::Id::new("qd_outline_tabk"))).unwrap_or(0.0);
    hud_shift(ctx).max(tk.clamp(0.0, 1.0) * (34.0 + GAP))
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
}

impl App {
    fn outline_tree(&self) -> Tree {
        let items: Vec<(Sel, [f32; 4])> = self.all_items().into_iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s, r))).collect();
        let area = |r: &[f32; 4]| r[2] * r[3];
        let inside = |a: &[f32; 4], b: &[f32; 4]| a[0] >= b[0] && a[1] >= b[1] && a[0] + a[2] <= b[0] + b[2] && a[1] + a[3] <= b[1] + b[3];
        let mut parent = vec![None; items.len()];
        for (i, (_, r)) in items.iter().enumerate() {
            let mut best: Option<usize> = None;
            for (j, ((k, _), q)) in items.iter().enumerate() {
                // identical rects: the earlier one is the parent, so there's never a cycle
                if i == j || *k != Kind::Group || !inside(r, q) || (area(q) <= area(r) && !(area(q) == area(r) && j < i)) {
                    continue;
                }
                if best.map_or(true, |b| area(q) < area(&items[b].1)) {
                    best = Some(j);
                }
            }
            parent[i] = best;
        }
        let mut kids = vec![Vec::new(); items.len()];
        for (i, p) in parent.iter().enumerate() {
            if let Some(p) = p {
                kids[*p].push(i);
            }
        }
        // Frames first, then reading order (top to bottom, left to right)
        let key = |i: usize| ((items[i].0 .0 != Kind::Group) as u8, items[i].1[1], items[i].1[0]);
        for k in kids.iter_mut() {
            k.sort_by(|a, b| key(*a).partial_cmp(&key(*b)).unwrap_or(std::cmp::Ordering::Equal));
        }
        Tree { items, parent, kids }
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
        let left = W + GAP + 10.0;
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
        let root: Option<Sel> = self.sel.as_ref().and_then(idx).and_then(|i| {
            let t = top(i);
            (tree.items[t].0 .0 == Kind::Group).then(|| tree.items[t].0.clone())
        });
        let show = !closed && root.is_some() && self.edit.is_none();
        let rid = egui::Id::new("qd_outline_root");
        if let Some(r) = &root {
            ctx.data_mut(|d| d.insert_temp(rid, r.clone()));
        }
        // Collapsed: a small tab (logo placeholder) stays at the top-left; click or L to expand
        let tk = ease(ctx, egui::Id::new("qd_outline_tab"), closed && root.is_some() && self.edit.is_none());
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_outline_tabk"), tk));
        if tk > 0.01 {
            egui::Area::new(egui::Id::new("qd_outline_tab_area"))
                .order(egui::Order::Middle)
                .fixed_pos(pos2(canvas.min.x + 10.0 - (1.0 - tk) * 14.0, canvas.min.y + 10.0))
                .interactable(tk > 0.5)
                .show(ctx, |ui| {
                    ui.set_opacity(tk.clamp(0.0, 1.0));
                    let (r, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                    let resp = resp.on_hover_text("Outline  ·  L");
                    let p = ui.painter();
                    let (h, pr) = hover_fx(ui.ctx(), &resp, true);
                    let r = r.expand(1.5 * h - pr);
                    p.rect_filled(r.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
                    p.rect_filled(r, 10.0, frame_bg());
                    p.rect_filled(r, 10.0, hover_fill(h, pr));
                    p.rect_stroke(r, 10.0, EStroke::new(1.0_f32, border()));
                    let c = r.center();
                    paint_logo(p, c, 18.0 + 2.0 * h, 1.0);
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
        let Some(root) = root.or_else(|| ctx.data(|d| d.get_temp::<Sel>(rid))) else { return };
        let Some(ri) = idx(&root) else { return };

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
        if board_scope {
            let mut roots: Vec<usize> = (0..tree.items.len()).filter(|&i| tree.parent[i].is_none()).collect();
            roots.sort_by(|a, b| {
                let ka = ((tree.items[*a].0 .0 != Kind::Group) as u8, tree.items[*a].1[1], tree.items[*a].1[0]);
                let kb = ((tree.items[*b].0 .0 != Kind::Group) as u8, tree.items[*b].1[1], tree.items[*b].1[0]);
                ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
            });
            for r in roots {
                walk(&tree, r, 0, &fold, &mut rows);
            }
        } else {
            walk(&tree, ri, 0, &fold, &mut rows);
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
                    let c = xr.center();
                    p.line_segment([c + vec2(2.0, -4.5), c + vec2(-2.5, 0.0)], EStroke::new(1.4_f32, TEXT_DIM));
                    p.line_segment([c + vec2(-2.5, 0.0), c + vec2(2.0, 4.5)], EStroke::new(1.4_f32, TEXT_DIM));
                    close |= xresp.clicked();
                    for (label, on_board) in [("Board", true), ("Frame", false)] {
                        let tw = ui.fonts(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(11.0), Color32::WHITE).size().x) + 14.0;
                        x -= tw + 4.0;
                        let r = Rect::from_min_size(pos2(x, hr.min.y + 4.0), vec2(tw, 20.0));
                        let resp = ui.interact(r, egui::Id::new(("qd_outline_scope", label)), Sense::click());
                        let on = board_scope == on_board;
                        let (sh, sp) = hover_fx(ctx, &resp, !on);
                        let onk = ease(ctx, egui::Id::new(("qd_outline_scope_on", label)), on);
                        p.rect_filled(r.expand(1.0 * sh - sp), 6.0, mix(hover_fill(sh, sp), Color32::from_white_alpha(20), onk));
                        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.0), if on { Color32::from_gray(235) } else { TEXT_DIM });
                        if resp.clicked() {
                            scope = Some(on_board);
                        }
                    }
                    ui.add_space(2.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 36.0).show(ui, |ui| {
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
                            let (r0, resp) = ui.allocate_exact_size(vec2(W - 12.0, ROW_H), Sense::click());
                            // Cascade in after the shell, top row first
                            let e = (k * 1.7 - (ri_ as f32 * 0.04).min(0.6)).clamp(0.0, 1.0);
                            let e = 1.0 - (1.0 - e).powi(3);
                            let renaming = ren.as_ref().is_some_and(|(rs, _)| rs == s);
                            // Hover lift; a row being renamed stays lifted, and easing in/out of it means
                            // no snap when the field opens or closes (the field steals the row's hover)
                            let m = ease(ctx, resp.id.with("lift"), resp.hovered() || renaming);
                            let r = Rect::from_min_max(pos2(r0.min.x + 6.0 * m - (1.0 - e) * 10.0, r0.min.y), pos2(r0.max.x - 2.0 + 2.0 * m, r0.max.y));
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
                        for (&(i, d), (r, m, e, resp)) in rows.iter().zip(laid) {
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
                            match s.0 {
                                Kind::Group => {
                                    for t in [-2.0_f32, 2.0] {
                                        p.line_segment([ic + vec2(-5.0, t), ic + vec2(5.0, t)], EStroke::new(1.1_f32, icol));
                                        p.line_segment([ic + vec2(t, -5.0), ic + vec2(t, 5.0)], EStroke::new(1.1_f32, icol));
                                    }
                                }
                                Kind::Image => {
                                    let b = Rect::from_center_size(ic, vec2(10.0, 9.0));
                                    p.rect_stroke(b, 1.5, EStroke::new(1.1_f32, icol));
                                    p.circle_filled(b.min + vec2(3.0, 3.0), 1.2, icol);
                                }
                                Kind::Text => {
                                    p.text(ic, Align2::CENTER_CENTER, "T", FontId::proportional(11.0), icol);
                                }
                                Kind::Note => {
                                    p.rect_filled(Rect::from_center_size(ic, vec2(9.0, 9.0)), 2.0, icol.gamma_multiply(0.7));
                                }
                                Kind::Link => {
                                    p.line_segment([ic + vec2(-4.0, 4.0), ic + vec2(4.0, -4.0)], EStroke::new(1.2_f32, icol));
                                    p.line_segment([ic + vec2(0.0, -4.0), ic + vec2(4.0, -4.0)], EStroke::new(1.2_f32, icol));
                                    p.line_segment([ic + vec2(4.0, -4.0), ic + vec2(4.0, 0.0)], EStroke::new(1.2_f32, icol));
                                }
                                Kind::Stroke => {
                                    p.circle_stroke(ic, 4.5, EStroke::new(1.1_f32, icol));
                                }
                            }
                            let name = self.outline_name(s);
                            let tc = mix(mix(TEXT_DIM, TEXT, 0.6 + 0.4 * m), Color32::WHITE, sk);
                            // Double-click: rename in place (Enter / click away saves, Esc cancels)
                            let on_caret = has && resp.interact_pointer_pos().is_some_and(|q| q.x < x0 + 15.0);
                            if resp.double_clicked() && !on_caret {
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
                                let room = r.max.x - (x0 + 37.0) - if has && folded { 26.0 } else { 8.0 };
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
                                p.text(pos2(r.max.x - 8.0, cy), Align2::RIGHT_CENTER, tree.kids[i].len().to_string(), FontId::proportional(10.5), Color32::from_gray(110));
                            }
                            if resp.hovered() {
                                hov = Some(s.clone());
                            }
                            if resp.clicked() {
                                if on_caret {
                                    toggle = Some(s.1.clone());
                                } else {
                                    pick = Some(s.clone());
                                }
                            }
                        }
                    });
                });
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
