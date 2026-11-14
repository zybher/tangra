//! Outline: the layer tree of the frame you're working in, on the left. It slides in while a
//! frame (or anything inside one) is selected and pushes the bottom-left HUD aside. `L` closes
//! it for good (until `L` again). Nesting is geometric, like the rest of the app: an object
//! belongs to the smallest frame that fully contains it.

use crate::*;
use std::collections::HashSet;

const W: f32 = 240.0;
const GAP: f32 = 12.0;
const ROW_H: f32 = 24.0;

fn kid() -> egui::Id {
    egui::Id::new("qd_outline_k")
}

/// How far the bottom-left HUD slides right to make room for the outline (animated)
pub(crate) fn hud_shift(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(kid())).unwrap_or(0.0).clamp(0.0, 1.0) * (W + GAP)
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
                    glass(p, r, 10.0, 1.0);
                    p.rect_filled(r, 10.0, hover_fill(h, pr));
                    p.rect_stroke(r, 10.0, EStroke::new(1.0_f32, border()));
                    let c = r.center();
                    for (dy, w) in [(-4.0_f32, 10.0_f32), (0.0, 7.0), (4.0, 10.0)] {
                        p.line_segment([c + vec2(-5.0, dy), c + vec2(-5.0 + w, dy)], EStroke::new(1.4_f32, Color32::from_gray(200)));
                    }
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
        egui::Area::new(egui::Id::new("qd_outline"))
            .order(egui::Order::Middle)
            .fixed_pos(pos)
            .interactable(show)
            .show(ctx, |ui| {
                ui.set_opacity(k.clamp(0.0, 1.0));
                let shell = Rect::from_min_size(pos, vec2(W, h));
                ui.painter().rect_filled(shell.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
                egui::Frame::none().fill(popup()).stroke(EStroke::new(1.0_f32, border())).rounding(10.0).inner_margin(6.0).show(ui, |ui| {
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
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        // Toolbar-style wave: rows near the pointer lift into a tile that slides right
                        let pitch = ROW_H + 1.0;
                        let y0 = ui.cursor().min.y + ROW_H / 2.0;
                        let area = ui.clip_rect();
                        let (fy, wa) = list_wave(ctx, egui::Id::new("qd_outline_wave"), area, y0, y0 + (rows.len().max(1) - 1) as f32 * pitch);
                        for (ri_, &(i, d)) in rows.iter().enumerate() {
                            let (s, _) = &tree.items[i];
                            let (r0, resp) = ui.allocate_exact_size(vec2(W - 12.0, ROW_H), Sense::click());
                            // Cascade in after the shell, top row first
                            let e = (k * 1.7 - (ri_ as f32 * 0.04).min(0.6)).clamp(0.0, 1.0);
                            let e = 1.0 - (1.0 - e).powi(3);
                            let (h, _pr) = hover_fx(ctx, &resp, true);
                            let m = (wa * (-((r0.center().y - fy) / pitch).powi(2)).exp()).max(h);
                            let r = Rect::from_min_max(pos2(r0.min.x + 6.0 * m - (1.0 - e) * 10.0, r0.min.y), pos2(r0.max.x - 2.0 + 2.0 * m, r0.max.y));
                            let p = ui.painter().with_clip_rect(ui.clip_rect());
                            let selected = self.selected.contains(s);
                            let sk = ease(ctx, egui::Id::new(("qd_outline_selk", &s.1)), selected);
                            wave_tile(&p, r, m, mix(popup(), Color32::from_gray(90), 0.35), e);
                            if sk > 0.01 {
                                p.rect_filled(r, 8.0, blue().gamma_multiply(0.22 * sk * e));
                            }
                            let mut p = p.with_clip_rect(r.intersect(ui.clip_rect()));
                            p.multiply_opacity(e);
                            if selected && changed {
                                resp.scroll_to_me(None);
                            }
                            let x0 = r.min.x + 6.0 + d as f32 * 14.0;
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
                            let ic = pos2(x0 + 19.0, cy);
                            let icol = if selected { blue() } else { Color32::from_gray(140) };
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
                            p.text(pos2(x0 + 30.0, cy), Align2::LEFT_CENTER, name, FontId::proportional(12.0), tc);
                            if has && folded {
                                p.text(pos2(r.max.x - 8.0, cy), Align2::RIGHT_CENTER, tree.kids[i].len().to_string(), FontId::proportional(10.5), Color32::from_gray(110));
                            }
                            if resp.hovered() {
                                hov = Some(s.clone());
                            }
                            if resp.clicked() {
                                let on_caret = has && resp.interact_pointer_pos().is_some_and(|q| q.x < x0 + 12.0);
                                if on_caret {
                                    toggle = Some(s.1.clone());
                                } else {
                                    pick = Some(s.clone());
                                }
                            }
                        }
                    });
                });
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
        if let Some(s) = pick {
            self.select_only(s);
        }
        if k > 0.01 && k < 0.999 {
            ctx.request_repaint();
        }
    }
}
