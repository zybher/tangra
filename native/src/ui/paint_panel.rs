//! The paint panel: what a Fill or a Stroke row of the properties panel opens (click its colour
//! chip). It floats left of the properties card, like Figma's. Along the top, Solid / Gradient /
//! Pattern (a pill slides to the picked one; Pattern is a placeholder for now, and strokes are
//! solid only for now). Solid shows the colour picker. Gradient shows its type (with swap ends and
//! a quarter turn), the bar with its stops as dots, and every stop as a row: where it sits, its
//! colour square (click: a second panel further left with the colour picker) and its hex (click
//! to type it in place), its opacity, and a minus (lined up under the Stops plus). "Stops +" adds one in the widest gap. The panel grows and
//! shrinks with the stops (its height glides). At the bottom, the board's document colours: the
//! last 20 used, newest first, saved with the board; click one to use it.

use super::props::{field, PAct, FH, GAP};
use crate::*;

/// Inner width (the colour picker's) and the panel's margin
const W: f32 = 232.0;
const M: f32 = 12.0;
/// The most stops a gradient takes from the panel
pub(crate) const MAX_STOPS: usize = 8;

fn key() -> egui::Id {
    egui::Id::new("qd_paint_open")
}

/// Is the paint panel `id` the open one
pub(super) fn is_open(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<Option<egui::Id>>(key())).flatten() == Some(id)
}

/// Open `id` (closing any other) or close it if it's open
pub(super) fn toggle(ctx: &egui::Context, id: egui::Id) {
    let on = is_open(ctx, id);
    ctx.data_mut(|d| d.insert_temp(key(), if on { None } else { Some(id) }));
    super::panel_drop::close_all(ctx);
}

pub(super) fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp::<Option<egui::Id>>(key(), None));
}

/// What the panel edits
pub(super) struct Paint<'a> {
    pub color: &'a str,
    pub grad: Option<&'a Gradient>,
    /// this one can take a gradient (closed shapes and frames' fills)
    pub can_grad: bool,
    /// the gradient's picked stop
    pub pick: Option<usize>,
    pub recent: &'a [String],
    /// a stroke's panel (its colour changes the outline)
    pub stroke: bool,
}

fn hex_of(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// A small icon button (a row's minus, the Stops plus)
pub(super) fn icon_btn(ui: &mut egui::Ui, id: egui::Id, r: Rect, icon: &'static str, tip: &str, a: f32) -> bool {
    let ctx = ui.ctx().clone();
    let resp = ui.interact(r, id, Sense::click()).on_hover_text(tip);
    let (hh, pp) = hover_fx(&ctx, &resp, true);
    let p = ui.painter();
    p.rect_filled(r, 6.0, hover_fill(hh, pp).gamma_multiply(a));
    super::svg_icons::draw(p, icon, r.center(), 14.0, mix(txt_dim(), hi(), hh).gamma_multiply(a));
    resp.clicked()
}

/// The document colours: a label and rows of ten chips; returns the one clicked
fn doc_colours(ui: &mut egui::Ui, id: egui::Id, recent: &[String], a: f32) -> Option<String> {
    let ctx = ui.ctx().clone();
    ui.add_space(GAP * 2.0);
    let (r, _) = ui.allocate_exact_size(vec2(W, 18.0), Sense::hover());
    ui.painter().text(r.left_center(), Align2::LEFT_CENTER, "Document colours", FontId::proportional(11.0), txt_dim().gamma_multiply(a));
    ui.add_space(4.0);
    if recent.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(W, 18.0), Sense::hover());
        ui.painter().text(r.left_center(), Align2::LEFT_CENTER, "Colours you use show up here", FontId::proportional(10.5), txt_dim().gamma_multiply(a * 0.6));
        return None;
    }
    let (n, g) = (10usize, 6.0);
    let cs = (W - g * (n - 1) as f32) / n as f32;
    let mut out = None;
    for (ri, line) in recent.chunks(n).enumerate() {
        if ri > 0 {
            ui.add_space(g);
        }
        let (r, _) = ui.allocate_exact_size(vec2(W, cs), Sense::hover());
        for (k, c) in line.iter().enumerate() {
            let cr = Rect::from_min_size(pos2(r.min.x + k as f32 * (cs + g), r.min.y), vec2(cs, cs));
            let resp = ui.interact(cr, id.with(("doc", ri * n + k)), Sense::click()).on_hover_text(c.trim_start_matches('#').to_uppercase());
            let (hh, _) = hover_fx(&ctx, &resp, true);
            let p = ui.painter();
            p.rect_filled(cr, 4.0, hex(c).gamma_multiply(a));
            p.rect_stroke(cr, 4.0, EStroke::new(1.0_f32, mix(wa(60), hi(), hh).gamma_multiply(a)));
            if resp.clicked() {
                out = Some(c.clone());
            }
        }
    }
    out
}

/// A floating glass panel at `at` whose height glides to what it held last frame; `body` fills it,
/// its insides cascading in like a bar dropdown's rows at progress `t` (ui/cascade.rs)
fn glass_panel(ctx: &egui::Context, id: egui::Id, at: Pos2, k: f32, t: f32, open: bool, body: impl FnOnce(&mut egui::Ui)) -> Rect {
    let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
    let want: f32 = ctx.data(|d| d.get_temp(id.with("h"))).unwrap_or(320.0);
    let h = super::micro::glide(ctx, id.with("gh"), want, k < 0.05);
    if (h - want).abs() > 0.5 {
        ctx.request_repaint();
    }
    let screen = ctx.screen_rect();
    let y = at.y.min(screen.max.y - h - 8.0).max(screen.min.y + 8.0);
    let at = pos2(at.x + (1.0 - ez(k)) * 8.0, y);
    let bg = Rect::from_min_size(at, vec2(W + 2.0 * M, h));
    let mut used = 0.0;
    // (not while a dropdown list covers it: the list takes the click)
    let live = open && !super::panel_drop::covering(ctx);
    egui::Area::new(id.with("area")).order(egui::Order::Foreground).fixed_pos(at).interactable(live).show(ctx, |ui| {
        ui.set_opacity(ez(k));
        let bp = ui.painter().clone().with_clip_rect(screen);
        bp.rect_filled(bg.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
        glass(&bp, bg, 10.0, 1.0);
        bp.rect_stroke(bg, 10.0, EStroke::new(1.0_f32, border()));
        ui.set_clip_rect(bg.shrink(1.0).intersect(screen));
        let layer = ui.layer_id();
        let from = super::cascade::mark(ctx, layer);
        #[allow(deprecated)]
        let r = ui.allocate_ui_at_rect(Rect::from_min_size(at + vec2(M, M), vec2(W, 4000.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_width(W);
            body(ui);
        });
        used = r.response.rect.height();
        super::cascade::apply(ctx, layer, from, at.y + M, used, t);
    });
    ctx.data_mut(|d| d.insert_temp(id.with("h"), used + 2.0 * M));
    if open {
        ui_block(ctx, bg);
    }
    bg
}

/// The paint panel `id`, left of `card`, level with `anchor` (its row) while it's open
#[allow(clippy::too_many_arguments)]
pub(super) fn show(ctx: &egui::Context, id: egui::Id, bar_id: egui::Id, anchor: Rect, card: Rect, pt: &Paint, out: &mut Vec<PAct>) {
    let open = is_open(ctx, id);
    let kc = ease(ctx, id.with("k"), open);
    let ct = super::cascade::progress(ctx, id, open);
    let sub_id = id.with("sub");
    let st = super::cascade::progress(ctx, sub_id, open && ctx.data(|d| d.get_temp::<usize>(sub_id)).is_some());
    if !open {
        ctx.data_mut(|d| d.remove::<usize>(sub_id));
    }
    if kc <= 0.001 {
        return;
    }
    let color_act = |c: String| if pt.stroke { PAct::StrokeColor(c) } else { PAct::FillColor(c) };
    let n_stops = pt.grad.map_or(0, |g| g.stops.len());
    // a stop's knob pressed on the canvas opens its colour panel, like a click on its square
    if let Some(j) = crate::canvas::take_knob_pick(ctx).filter(|_| open) {
        ctx.data_mut(|d| d.insert_temp(sub_id, j));
    }
    let sub: Option<usize> = ctx.data(|d| d.get_temp(sub_id)).filter(|j| *j < n_stops);
    let mut sub_row: Option<Rect> = None;
    let mut acts: Vec<PAct> = Vec::new();
    let at = pos2(card.min.x - 10.0 - (W + 2.0 * M), anchor.min.y);
    let bg = glass_panel(ctx, id, at, kc, ct, open, |ui| {
        let a = 1.0;
        let row = |ui: &mut egui::Ui, h: f32| ui.allocate_exact_size(vec2(W, h), Sense::hover()).0;
        // Solid / Gradient / Pattern
        let mode = if pt.grad.is_some() { 1usize } else { 0 };
        let r = row(ui, FH);
        let at_k = ctx.animate_value_with_time(id.with("mode_k"), mode as f32, 0.2 * anim_k());
        let p = ui.painter();
        p.rect_filled(r, 6.0, Color32::from_black_alpha(60));
        let third = r.width() / 3.0;
        p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at_k * third, r.min.y + 2.0), vec2(third - 4.0, r.height() - 4.0)), 5.0, wa(22));
        let grad_tip = if pt.stroke { "Gradient (strokes are solid for now)" } else if pt.can_grad { "Gradient" } else { "Gradient (not on this)" };
        for (j, (icon, tip)) in [("fill-solid", "Solid"), ("gradient-linear", grad_tip), ("image", "Pattern (coming soon)")].into_iter().enumerate() {
            let br = Rect::from_min_size(pos2(r.min.x + j as f32 * third, r.min.y), vec2(third, r.height()));
            let on = j == mode;
            let ok = j == 0 || (j == 1 && pt.can_grad && !pt.stroke);
            let resp = ui.interact(br, id.with(("mode", j)), Sense::click()).on_hover_text(tip);
            let (hh, pp) = hover_fx(ctx, &resp, !on && ok);
            if !on && ok {
                ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp));
            }
            let onk = (1.0 - (at_k - j as f32).abs()).clamp(0.0, 1.0);
            let col = mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(if ok { 1.0 } else { 0.35 });
            super::svg_icons::draw(ui.painter(), icon, br.center(), 15.0, col);
            if resp.clicked() && ok && !on {
                acts.push(PAct::GKind(if j == 0 { None } else { Some(0) }));
            }
        }
        ui.add_space(GAP * 1.5);
        match pt.grad {
            None => {
                let mut c = if pt.color.is_empty() || pt.color == "none" { Color32::WHITE } else { hex(pt.color) };
                let (ch, _) = super::color_picker::color_panel_ex(ui, id.with("hsv"), &mut c, 1.0, false);
                ui.set_opacity(1.0);
                if ch && open {
                    acts.push(color_act(hex_of(c)));
                }
                if let Some(c) = doc_colours(ui, id, pt.recent, a) {
                    acts.push(color_act(c));
                }
            }
            Some(g) => {
                // type, with swap ends and a quarter turn beside its chevron while hovered
                let r = row(ui, FH);
                let (pk, extra) = super::panel_drop::dropdown_bar_extras(
                    ui,
                    id.with("kind_dd"),
                    r,
                    &crate::canvas::gradient::KINDS,
                    &["gradient-linear", "gradient-radial", "gradient-angular", "gradient-diamond"],
                    g.kind as usize,
                    "Gradient type",
                    &[("swap-ends", "Swap the gradient's ends"), ("rotate-cw", "Turn it a quarter")],
                    a,
                );
                if let Some(j) = pk {
                    acts.push(PAct::GKind(Some(j as u8)));
                }
                match extra {
                    Some(0) => acts.push(PAct::GFlip),
                    Some(_) => acts.push(PAct::GRotate),
                    None => {}
                }
                ui.add_space(GAP);
                let r = row(ui, 22.0);
                gradient_bar(ui, bar_id, r, g, pt.pick, a, &mut acts);
                ui.add_space(GAP * 1.5);
                // Stops +
                let r = row(ui, 22.0);
                ui.painter().text(r.left_center(), Align2::LEFT_CENTER, "Stops", FontId::proportional(11.5), gray_ui(225));
                let pr = Rect::from_center_size(pos2(r.max.x - 13.0, r.center().y), vec2(26.0, 22.0));
                // a soft cap: at MAX_STOPS the plus dims and adding stops
                let room = g.stops.len() < MAX_STOPS;
                let add_a = a * ease(ctx, id.with("st_room"), room).max(0.35);
                if icon_btn(ui, id.with("st_add"), pr, "plus", if room { "Add a stop" } else { "Up to 8 stops" }, add_a) && room {
                    // in the middle of the widest gap between stops
                    let mut ts: Vec<f32> = g.stops.iter().map(|s| s.at).collect();
                    ts.sort_by(|x, y| x.total_cmp(y));
                    ts.insert(0, 0.0);
                    ts.push(1.0);
                    let t = ts.windows(2).max_by(|x, y| (x[1] - x[0]).total_cmp(&(y[1] - y[0]))).map_or(0.5, |w| (w[0] + w[1]) / 2.0);
                    acts.push(PAct::GAdd(t));
                }
                // one row per stop, in order along the bar. Each stop keeps a key while it lives,
                // so its row grows in when added and folds away when removed (props `fold_begin`)
                let kid = id.with("st_keys");
                let sigs: Vec<(f32, String, f32)> = g.stops.iter().map(|s| (s.at, s.color.clone(), s.alpha)).collect();
                type Keys = (Vec<u64>, Vec<(f32, String, f32)>, u64, Vec<(u64, crate::model::Stop)>);
                let (mut keys, old, mut next, mut ghosts): Keys = ctx.data(|d| d.get_temp(kid)).unwrap_or_default();
                let n = sigs.len();
                if keys.len() != n || old.len() != n {
                    if keys.len() + 1 == n && old.len() + 1 == n {
                        // added: the first place the list changed
                        let at = (0..keys.len()).find(|&i| old[i] != sigs[i]).unwrap_or(keys.len());
                        keys.insert(at, next);
                        // seen closed first, so its row grows in from nothing
                        ease(ctx, id.with(("st_row", next)).with("k"), false);
                        next += 1;
                    } else if keys.len() == n + 1 && old.len() == n + 1 {
                        // removed: its row stays a moment as it folds away
                        let at = (0..n).find(|&i| old[i] != sigs[i]).unwrap_or(n);
                        let k = keys.remove(at);
                        let (t, c, al) = old[at].clone();
                        ghosts.push((k, crate::model::Stop { at: t, color: c, alpha: al }));
                    } else {
                        // a different gradient: its rows just show
                        keys = (0..n as u64).map(|i| next + i).collect();
                        next += n as u64;
                        ghosts.clear();
                    }
                }
                let mut list: Vec<(u64, Option<usize>, crate::model::Stop)> = (0..n).map(|j| (keys[j], Some(j), g.stops[j].clone())).collect();
                list.extend(ghosts.iter().map(|(k, st)| (*k, None, st.clone())));
                list.sort_by(|x, y| x.2.at.total_cmp(&y.2.at));
                let can_del = n > 2;
                // the picked stop's highlight: the panel's full width, gliding from row to row like
                // the outline's selection (drawn under the rows)
                let hl = ui.painter().add(egui::Shape::Noop);
                let top0 = ui.cursor().min.y;
                let mut hl_r: Option<Rect> = None;
                let mut gone: Vec<u64> = Vec::new();
                for (key, j, st) in list {
                    let fid = id.with(("st_row", key));
                    let Some((mut fu, fk)) = super::props::fold_begin(ui, fid, j.is_some()) else {
                        gone.push(key);
                        continue;
                    };
                    let mut ghost_acts = Vec::new();
                    let out: &mut Vec<PAct> = if j.is_some() { &mut acts } else { &mut ghost_acts };
                    let jj = j.unwrap_or(usize::MAX);
                    fu.add_space(4.0);
                    let r = row(&mut fu, FH);
                    // a press anywhere on the row (its fields, its hex, the space either side) picks it
                    let band = Rect::from_min_max(pos2(r.min.x - M, r.min.y - 2.0), pos2(r.max.x + M, r.max.y + 2.0));
                    // (not while one of its dropdown lists is open over it)
                    if j.is_some() && pt.pick != j && !super::panel_drop::covering(ctx) {
                        let layer = fu.layer_id();
                        let clip = fu.clip_rect();
                        let hit = ctx.input(|i| i.pointer.primary_pressed().then(|| i.pointer.press_origin()).flatten());
                        if hit.is_some_and(|q| band.contains(q) && clip.contains(q) && ctx.layer_id_at(q) == Some(layer)) {
                            out.push(PAct::GPick(jj));
                        }
                    }
                    if j.is_some() && pt.pick == j {
                        hl_r = Some(r);
                    }
                    // where it sits (just wide enough for 100%), its colour + hex, its opacity, a minus
                    let del_w = if can_del { 26.0 } else { 0.0 };
                    let (pw, aw) = (46.0, 66.0);
                    let pos_r = Rect::from_min_size(r.min, vec2(pw, FH));
                    let col_max = r.max.x - del_w - if can_del { 4.0 } else { 0.0 } - aw - 4.0;
                    let col_r = Rect::from_min_max(pos2(pos_r.max.x + 4.0, r.min.y), pos2(col_max, r.max.y));
                    let al_r = Rect::from_min_size(pos2(col_max + 4.0, r.min.y), vec2(aw, FH));
                    if let Some((v, f1)) = field(&mut fu, id.with(("st_at", key)), pos_r, "", (st.at * 100.0).round(), "%", 0.5, 0.0, 100.0, false, a) {
                        out.push(PAct::GAt(jj, v / 100.0, f1));
                    }
                    // its colour: the square opens the colour panel on the left, the hex types in place
                    let open_k = ease(ctx, id.with(("st_open", key)), j.is_some() && sub == j);
                    let cell = super::hex_cell::show(&mut fu, id.with(("st_cell", key)), col_r, &st.color, open_k, open_k, a);
                    if cell.typing {
                        out.push(PAct::GPick(jj));
                        if j.is_some() {
                            ctx.data_mut(|d| d.remove::<usize>(sub_id));
                        }
                    }
                    if let Some(c) = cell.typed {
                        out.push(PAct::GColor(jj, c));
                    }
                    if cell.square && j.is_some() {
                        out.push(PAct::GPick(jj));
                        let now = if sub == j { None } else { j };
                        ctx.data_mut(|d| match now {
                            Some(j) => d.insert_temp(sub_id, j),
                            None => d.remove::<usize>(sub_id),
                        });
                    }
                    if let Some((v, f1)) = field(&mut fu, id.with(("st_al", key)), al_r, "", (st.alpha * 100.0).round(), "%", 0.5, 0.0, 100.0, true, a) {
                        out.push(PAct::GAlpha(jj, v / 100.0, f1));
                    }
                    super::svg_icons::draw(fu.painter(), "opacity", pos2(al_r.min.x + 14.0, al_r.center().y), 14.0, txt_dim());
                    if can_del || j.is_none() {
                        let del_r = Rect::from_min_max(pos2(r.max.x - 26.0, r.min.y), r.max);
                        if icon_btn(&mut fu, id.with(("st_del", key)), del_r, "minus", "Remove this stop", a) {
                            out.push(PAct::GDel(jj));
                        }
                    }
                    if j.is_some() && sub == j {
                        sub_row = Some(r);
                    }
                    super::props::fold_end(ui, fid, fu, fk);
                }
                ghosts.retain(|(k, _)| !gone.contains(k));
                ctx.data_mut(|d| d.insert_temp::<Keys>(kid, (keys, sigs, next, ghosts)));
                let gid = id.with("st_hl");
                let pa = ease(ctx, gid.with("a"), hl_r.is_some());
                if let Some(t) = hl_r {
                    ctx.data_mut(|d| d.insert_temp(gid.with("r"), t));
                }
                let last: Option<Rect> = ctx.data(|d| d.get_temp(gid.with("r")));
                if let Some(t) = hl_r.or(last).filter(|_| pa > 0.01) {
                    // y from the first row's top, so the panel moving doesn't make it lag; coming
                    // back from hidden it appears in place instead of flying across
                    let y = super::micro::glide(ctx, gid, t.min.y - top0, pa <= 0.05);
                    let band = Rect::from_min_max(pos2(t.min.x - M, top0 + y - 2.0), pos2(t.max.x + M, top0 + y + t.height() + 2.0));
                    ui.painter().set(hl, egui::Shape::rect_filled(band, 0.0, blue().gamma_multiply(0.22 * pa * a)));
                }
                ui.add_space(GAP * 1.5);
                // how softly the colours ease into each other
                let r = row(ui, FH);
                if let Some((v, f1)) = field(ui, id.with("smooth"), r, "Smooth", (g.smooth * 100.0).round(), "%", 0.5, 0.0, 100.0, true, a) {
                    acts.push(PAct::GSmooth(v / 100.0, f1));
                }
                // a document colour goes on the picked stop
                if let Some(c) = doc_colours(ui, id, pt.recent, a) {
                    let j = pt.pick.unwrap_or(0);
                    acts.push(PAct::GColor(j, c));
                }
            }
        }
    });

    // The picked stop's colour, in a second panel further left
    let sk = ease(ctx, sub_id.with("k"), open && sub.is_some());
    let mut sub_bg = Rect::NOTHING;
    if let (Some(g), true) = (pt.grad, sk > 0.001) {
        if let Some(j) = sub {
            ctx.data_mut(|d| d.insert_temp(sub_id.with("j"), j));
        }
        let j: usize = ctx.data(|d| d.get_temp(sub_id.with("j"))).unwrap_or(0).min(g.stops.len().saturating_sub(1));
        if let Some(r) = sub_row {
            ctx.data_mut(|d| d.insert_temp(sub_id.with("y"), r.min.y));
        }
        let y: f32 = ctx.data(|d| d.get_temp(sub_id.with("y"))).unwrap_or(bg.min.y);
        // it glides to the picked stop's row like the row highlight (in place when it opens)
        let y = super::micro::glide(ctx, sub_id.with("gy"), y, sk <= 0.05);
        let color = g.stops[j].color.clone();
        sub_bg = glass_panel(ctx, sub_id, pos2(bg.min.x - 10.0 - (W + 2.0 * M), y), sk, st, sub.is_some(), |ui| {
            let mut c = hex(&color);
            let (ch, _) = super::color_picker::color_panel_ex(ui, sub_id.with("hsv"), &mut c, 1.0, false);
            ui.set_opacity(1.0);
            if ch && sub.is_some() {
                acts.push(PAct::GColor(j, hex_of(c)));
            }
            if let Some(c) = doc_colours(ui, sub_id, pt.recent, 1.0) {
                acts.push(PAct::GColor(j, c));
            }
        });
    }

    // Esc or a click away closes it (the sub panel first); its own dropdown / hex bubble have theirs
    if open {
        let (pressed, esc, q) = ctx.input(|i| (i.pointer.any_pressed(), i.key_pressed(egui::Key::Escape), i.pointer.interact_pos()));
        let popup = super::panel_drop::any_open(ctx);
        if esc && !popup {
            if sub.is_some() {
                ctx.data_mut(|d| d.remove::<usize>(sub_id));
            } else {
                close(ctx);
            }
        }
        if pressed && !popup && !super::color_picker::picking(ctx) {
            if let Some(q) = q {
                let inside = bg.contains(q) || card.contains(q) || (sub.is_some() && sub_bg.contains(q)) || crate::canvas::on_grad_knob(ctx, q);
                if !inside {
                    close(ctx);
                }
            }
        }
    }
    if kc < 1.0 || sk > 0.0 && sk < 1.0 {
        ctx.request_repaint();
    }
    out.extend(acts);
}

/// A colour chip and its hex inside `r` (a field-style row). The colour glides to a new one
/// (so switching gradient stops eases rather than snaps); "none" shows as an empty chip
pub(super) fn color_chip(ctx: &egui::Context, p: &egui::Painter, id: egui::Id, r: Rect, color: &str, hl: f32, label: bool, a: f32) {
    let none = color == "none";
    let c = hex(if none { "#000000" } else { color });
    let ch = |k: &str, v: u8| super::micro::glide(ctx, id.with(k), v as f32, false).round().clamp(0.0, 255.0) as u8;
    let g = Color32::from_rgb(ch("r", c.r()), ch("g", c.g()), ch("b", c.b()));
    if g != c {
        ctx.request_repaint();
    }
    let chip = if label { Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0)) } else { Rect::from_center_size(r.center(), vec2(16.0, 16.0)) };
    if none {
        p.line_segment([chip.left_bottom() + vec2(2.0, -2.0), chip.right_top() + vec2(-2.0, 2.0)], EStroke::new(1.5_f32, hex("#ef4444").gamma_multiply(a)));
    } else {
        p.rect_filled(chip, 3.0, g.gamma_multiply(a));
    }
    p.rect_stroke(chip, 3.0, EStroke::new(1.0_f32, wa((60.0 * a) as u8)));
    if !label {
        return;
    }
    let label = if none { "None".to_string() } else { format!("{:02X}{:02X}{:02X}", g.r(), g.g(), g.b()) };
    p.with_clip_rect(r.shrink(1.0)).text(pos2(chip.max.x + 7.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), mix(gray_ui(200), txt(), hl).gamma_multiply(a));
}

/// A gradient's colours as a bar the full width of the panel (over a checkerboard, so
/// see-through stops show) with its stops as dots on it: click a dot to pick it, drag it
/// along (it stays on the bar), right-click it to remove it, click the bar to add one
pub(super) fn gradient_bar(ui: &mut egui::Ui, id: egui::Id, r: Rect, g: &Gradient, pick: Option<usize>, a: f32, out: &mut Vec<PAct>) {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    let bar = Rect::from_min_size(r.min, vec2(r.width(), 22.0));
    // the colours run between the dots' outermost spots; the ends carry the end colours on
    // (inset enough that an end dot, grown and ringed, stays inside the panel)
    let ramp = bar.shrink2(vec2(14.0, 0.0));
    let cp = p.with_clip_rect(bar.shrink(1.0).intersect(p.clip_rect()));
    let sq = 5.5;
    // the checkerboard only where it can show (a see-through stop), so it never peeks through
    // an opaque gradient as the panel fades
    let see_through = g.stops.iter().any(|s| s.alpha < 0.999);
    let mut x = if see_through { bar.min.x } else { bar.max.x };
    let mut i = 0;
    while x < bar.max.x {
        let mut y = bar.min.y;
        let mut j = 0;
        while y < bar.max.y {
            let c = if (i + j) % 2 == 0 { gray_ui(70) } else { gray_ui(110) };
            cp.rect_filled(Rect::from_min_size(pos2(x, y), vec2(sq, sq)), 0.0, c.gamma_multiply(a));
            y += sq;
            j += 1;
        }
        x += sq;
        i += 1;
    }
    let c0 = crate::canvas::gradient::color32_at(g, 0.0).gamma_multiply(a);
    let c1 = crate::canvas::gradient::color32_at(g, 1.0).gamma_multiply(a);
    p.rect_filled(Rect::from_min_max(bar.min, pos2(ramp.min.x + 1.0, bar.max.y)), Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 }, c0);
    p.rect_filled(Rect::from_min_max(pos2(ramp.max.x - 1.0, bar.min.y), bar.max), Rounding { nw: 0.0, sw: 0.0, ne: 6.0, se: 6.0 }, c1);
    let mut m = egui::Mesh::default();
    const N: usize = 64;
    for k in 0..=N {
        let t = k as f32 / N as f32;
        let c = crate::canvas::gradient::color32_at(g, t).gamma_multiply(a);
        let x = ramp.min.x + t * ramp.width();
        m.colored_vertex(pos2(x, bar.min.y), c);
        m.colored_vertex(pos2(x, bar.max.y), c);
        if k > 0 {
            let q = (k * 2) as u32;
            m.add_triangle(q - 2, q - 1, q);
            m.add_triangle(q - 1, q + 1, q);
        }
    }
    p.add(egui::Shape::mesh(m));
    p.rect_stroke(bar.shrink(0.5), 5.5, EStroke::new(1.0_f32, wa((40.0 * a) as u8)));

    let x_of = |t: f32| ramp.min.x + t.clamp(0.0, 1.0) * ramp.width();
    let t_of = |x: f32| ((x - ramp.min.x) / ramp.width()).clamp(0.0, 1.0);
    let resp = ui.interact(bar.expand2(vec2(0.0, 4.0)), id, Sense::click_and_drag());
    let hov = resp.hover_pos();
    let near = |q: Pos2| g.stops.iter().enumerate().map(|(j, s)| (j, (x_of(s.at) - q.x).abs())).filter(|(_, d)| *d < 11.0).min_by(|x, y| x.1.total_cmp(&y.1)).map(|(j, _)| j);
    let held_id = id.with("held");
    let held_prev: Option<(usize, f32)> = ctx.data(|d| d.get_temp(held_id));
    let down = resp.is_pointer_button_down_on();
    let q_now = ctx.input(|i| i.pointer.latest_pos());
    // pressed on a dot: it's picked and held at once, remembering how far off its middle the
    // pointer was, so it moves with the pointer from the first pixel and never jumps
    let mut held = held_prev;
    if down && held.is_none() && ctx.input(|i| i.pointer.primary_pressed()) {
        if let Some(q) = q_now {
            if let Some(j) = near(q) {
                out.push(PAct::GPick(j));
                held = Some((j, x_of(g.stops[j].at) - q.x));
                ctx.data_mut(|d| d.insert_temp(held_id, held.unwrap()));
            }
        }
    }
    // dragged down off the bar: it's going to be removed (needs two left)
    // a dot stays on the bar however far the pointer wanders (remove with its row's minus or a
    // right-click)
    let off = |_: Pos2| false;
    if down {
        if let (Some((j, dx)), Some(q)) = (held, q_now) {
            let t = t_of(q.x + dx);
            if !off(q) && (t - g.stops[j].at).abs() > 1e-4 {
                out.push(PAct::GMove(j, t));
            }
        }
    } else if let Some((j, _)) = held {
        // let go
        if q_now.is_some_and(off) {
            out.push(PAct::GDel(j));
        }
        ctx.data_mut(|d| d.remove::<(usize, f32)>(held_id));
        ctx.data_mut(|d| d.remove::<bool>(id.with("gmv")));
    }
    if resp.clicked() && held_prev.is_none() && held.is_none() {
        if let Some(q) = resp.interact_pointer_pos() {
            if near(q).is_none() && g.stops.len() < MAX_STOPS {
                out.push(PAct::GAdd(t_of(q.x)));
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(j) = resp.interact_pointer_pos().and_then(near).filter(|_| g.stops.len() > 2) {
            out.push(PAct::GDel(j));
        }
    }
    let held = if down { held.map(|h| h.0) } else { None };
    let going = held.is_some() && q_now.is_some_and(off);
    for (j, st) in g.stops.iter().enumerate() {
        let leaving = ease(&ctx, id.with(("off", j)), going && held == Some(j));
        let mut c = pos2(x_of(st.at), bar.center().y);
        if leaving > 0.001 {
            if let Some(q) = q_now {
                c = c.lerp(q, leaving);
            }
        }
        let hh = ease(&ctx, id.with(("hv", j)), hov.is_some_and(|q| (q.x - c.x).abs() < 11.0));
        let on = ease(&ctx, id.with(("on", j)), pick == Some(j));
        let rad = 6.0 + 1.0 * hh + 1.0 * on;
        let fa = a * (1.0 - 0.6 * leaving);
        p.circle_filled(c + vec2(0.0, 1.0), rad + 1.5, Color32::from_black_alpha((110.0 * fa) as u8));
        p.circle_filled(c, rad + 1.5, mix(Color32::WHITE, blue(), on).gamma_multiply(fa));
        p.circle_filled(c, rad - 0.5, hex(&st.color).gamma_multiply(fa));
    }
    // the hand only while a dot is being dragged
    if held.is_some() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }
}

