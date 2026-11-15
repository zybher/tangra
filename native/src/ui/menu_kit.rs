//! Building blocks for menus: open/close animation, rows, radios, checks, swatches, dividers.

use crate::*;

/// Right-click menu reveal: (alpha, y offset) for a row at `y`; rows cascade in top-down
/// for a moment after the menu opens. Outside the canvas menu it is a no-op.
pub(crate) fn cm_anim(ui: &egui::Ui, y: f32) -> (f32, f32) {
    let ctx = ui.ctx();
    let Some((t0, last)) = ctx.data(|d| d.get_temp::<(f64, u64)>(egui::Id::new("qd_cm_t"))) else {
        return (1.0, 0.0);
    };
    if last != ctx.cumulative_pass_nr() {
        return (1.0, 0.0);
    }
    let top: f32 = ctx.data(|d| d.get_temp(egui::Id::new("qd_cm_top"))).unwrap_or(y);
    let k = ((ctx.input(|i| i.time) - t0) / 0.5) as f32;
    let e = (k * 1.4 - (((y - top) / 26.0).max(0.0) * 0.045).min(0.4)).clamp(0.0, 1.0);
    if e < 1.0 {
        ctx.request_repaint();
    }
    let e = 1.0 - (1.0 - e).powi(4);
    // Rows start bunched up at the cursor and unfold downward into place
    (e, -(1.0 - e) * ((y - top).max(0.0) * 0.22 + 6.0))
}

/// Wave magnify for a canvas-menu row centred at `cy`
pub(crate) fn cm_wave(ui: &egui::Ui, cy: f32) -> f32 {
    let ctx = ui.ctx();
    match ctx.data(|d| d.get_temp::<(f32, f32, u64)>(egui::Id::new("qd_cm_focus"))) {
        Some((fy, wa, fr)) if fr == ctx.cumulative_pass_nr() => {
            let d = (cy - fy) / 26.0;
            0.8 * wa * (-d * d).exp()
        }
        _ => 0.0,
    }
}

/// Menu row of small choice buttons (S / M / L …); the one matching `current` is lit.
pub(crate) fn choice_row(ui: &mut egui::Ui, choices: &[(&str, f32)], current: f32, tol: f32) -> Option<f32> {
    // One segmented control, inset like every other menu row
    let mut picked = None;
    let ws: Vec<f32> = choices.iter().map(|c| ui.painter().layout_no_wrap(c.0.to_string(), FontId::proportional(11.5), TEXT).size().x + 16.0).map(|w| w.max(30.0)).collect();
    let total: f32 = ws.iter().sum::<f32>() + 4.0;
    let (row, _) = ui.allocate_exact_size(vec2(total + MENU_INSET * 2.0, 26.0), Sense::hover());
    let (a, dy) = cm_anim(ui, row.min.y);
    let row_d = row.translate(vec2(0.0, dy));
    let track = Rect::from_min_size(pos2(row_d.min.x + MENU_INSET, row_d.min.y + 1.0), vec2(total, 24.0));
    let p = ui.painter().clone().with_clip_rect(ui.ctx().screen_rect());
    p.rect_filled(track, 6.0, Color32::from_gray(26).gamma_multiply(a));
    let mut sx = track.min.x + 2.0;
    for (i, &(label, val)) in choices.iter().enumerate() {
        let seg = Rect::from_min_size(pos2(sx, track.min.y), vec2(ws[i], track.height())).shrink(2.0);
        sx += ws[i];
        let resp = ui.interact(seg, ui.id().with(("seg", row.min.y as i32, i)), Sense::click());
        let (h, pr) = hover_fx(ui.ctx(), &resp, true);
        let on = ease(ui.ctx(), ui.id().with(("seg_on", row.min.y as i32, i)), (current - val).abs() < tol);
        let sg = seg.expand(1.5 * h - pr);
        p.rect_filled(sg, 4.0, mix(mix(Color32::from_gray(26), Color32::from_gray(62), h), blue(), on).gamma_multiply(a));
        let col = mix(mix(TEXT_DIM, TEXT, h), Color32::WHITE, on).gamma_multiply(a);
        p.text(sg.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.5 * (1.0 + 0.08 * h)), col);
        if resp.clicked() {
            picked = Some(val);
        }
    }
    picked
}

pub(crate) const MENU_INSET: f32 = 10.0;
/// Menu row whose number is changed by dragging left / right (same as the placement bar's
/// scrubs). Returns (new value, true on the first frame of the drag) while dragging.
#[allow(dead_code)]
pub(crate) fn scrub_item(ui: &mut egui::Ui, label: &str, value: f32, per_px: f32, lo: f32, hi: f32, glyph: bool) -> Option<(f32, bool)> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(160.0), 26.0), Sense::drag());
    let (h, pr) = hover_fx(ui.ctx(), &resp, true);
    let (a, dy) = cm_anim(ui, rect.min.y);
    let m = cm_wave(ui, rect.center().y).max(h);
    let p = ui.painter().clone().with_clip_rect(ui.ctx().screen_rect());
    let r = rect.translate(vec2(0.0, dy));
    let tile = Rect::from_min_max(pos2(r.min.x + 6.0 * m, r.min.y + 1.0), pos2(r.max.x + 2.0 * m, r.max.y - 1.0));
    wave_tile(&p, tile, m, mix(popup(), Color32::from_gray(90), 0.35), a);
    if pr > 0.01 {
        p.rect_filled(tile, 8.0, Color32::from_black_alpha((40.0 * pr * a) as u8));
    }
    let col = mix(TEXT_DIM, TEXT, 0.6 + 0.4 * m).gamma_multiply(a);
    let lg = p.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), col);
    p.galley(pos2(tile.min.x + 10.0, tile.center().y - lg.size().y / 2.0), lg, col);
    // Corner glyph + value on the right, like the bars
    let vx = tile.max.x - 10.0;
    let vg = p.layout_no_wrap(format!("{}", value.round() as i32), FontId::proportional(12.0), col);
    let ic = pos2(vx - vg.size().x - 12.0, tile.center().y);
    if glyph {
        let pts = vec![ic + vec2(-5.0, 6.0), ic + vec2(-5.0, -1.0), ic + vec2(-4.0, -4.0), ic + vec2(-1.0, -5.0), ic + vec2(6.0, -5.0)];
        p.add(egui::Shape::line(pts, EStroke::new(1.3_f32, col)));
    }
    p.galley(pos2(vx - vg.size().x, tile.center().y - vg.size().y / 2.0), vg, col);
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    let id = ui.id().with(("scrub0", label));
    if resp.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(id, value));
    }
    if resp.dragged() {
        let v0: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(value);
        let dx = resp.interact_pointer_pos().zip(ui.ctx().input(|i| i.pointer.press_origin())).map_or(0.0, |(q, o)| q.x - o.x);
        return Some(((v0 + dx * per_px).round().clamp(lo, hi), resp.drag_started()));
    }
    None
}

/// Worker job level meaning "make the coarse pixelation copy".
pub(crate) const MOSAIC: u8 = 255;

/// Menu row with a round "current tool" marker.
pub(crate) fn radio_item(ui: &mut egui::Ui, label: &str, shortcut: &str, on: bool) -> bool {
    let (clicked, mk, a) = item_ex(ui, &format!("       {label}"), shortcut, true);
    let c = mk + vec2(MENU_INSET + 6.0, 13.0);
    if on {
        ui.painter().circle_filled(c, 3.5, blue().gamma_multiply(a));
    } else {
        ui.painter().circle_stroke(c, 3.0, EStroke::new(1.0_f32, Color32::from_gray(110).gamma_multiply(a)));
    }
    clicked
}

/// Menu row with a checkbox.
pub(crate) fn check_item(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let (clicked, mk, a) = item_ex(ui, &format!("       {label}"), "", true);
    let r = Rect::from_min_size(mk + vec2(10.0, 7.0), vec2(12.0, 12.0));
    let p = ui.painter().clone().with_clip_rect(ui.ctx().screen_rect());
    if on {
        p.rect_filled(r, 3.0, blue().gamma_multiply(a));
        let tick = vec![r.min + vec2(2.5, 6.2), r.min + vec2(5.0, 8.7), r.min + vec2(9.5, 3.5)];
        p.add(Shape::line(tick, EStroke::new(1.5_f32, Color32::WHITE.gamma_multiply(a))));
    } else {
        p.rect_stroke(r, 3.0, EStroke::new(1.2_f32, TEXT_DIM.gamma_multiply(a)));
    }
    clicked
}

pub(crate) fn item(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    item_ex(ui, label, shortcut, enabled).0
}

/// A menu row with the toolbar-style wave: rows near the pointer lift into a tile that
/// slides right. Returns (clicked, where the row's left edge is drawn, alpha).
pub(crate) fn item_ex(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> (bool, Pos2, f32) {
    let probe = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(13.0), TEXT);
    let sw = if shortcut.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(shortcut.to_owned(), FontId::proportional(11.5), TEXT_FAINT).size().x + 28.0 };
    let need = probe.size().x + sw + 30.0;
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(need), 26.0), sense);
    let (h, pr) = hover_fx(ui.ctx(), &resp, enabled);
    let (a, dy) = cm_anim(ui, rect.min.y);
    let m = if enabled { cm_wave(ui, rect.center().y).max(h) } else { 0.0 };
    let p = ui.painter().clone().with_clip_rect(ui.ctx().screen_rect());
    let r = rect.translate(vec2(0.0, dy));
    let tile = Rect::from_min_max(pos2(r.min.x + 6.0 * m, r.min.y + 1.0), pos2(r.max.x + 2.0 * m, r.max.y - 1.0));
    wave_tile(&p, tile, m, mix(popup(), Color32::from_gray(90), 0.35), a);
    if pr > 0.01 {
        p.rect_filled(tile, 8.0, Color32::from_black_alpha((40.0 * pr * a) as u8));
    }
    let col = if enabled { mix(TEXT_DIM, TEXT, 0.6 + 0.4 * m) } else { TEXT_FAINT }.gamma_multiply(a);
    let lg = p.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), col);
    p.galley(pos2(tile.min.x + 10.0, tile.center().y - lg.size().y / 2.0), lg, col);
    if !shortcut.is_empty() {
        let sc = mix(TEXT_FAINT, TEXT_DIM, m).gamma_multiply(a);
        p.text(pos2(tile.max.x - 10.0, tile.center().y), Align2::RIGHT_CENTER, shortcut, FontId::proportional(11.5), sc);
    }
    let clicked = resp.clicked();
    if clicked {
        ui.close_menu();
    }
    (clicked, pos2(tile.min.x, r.min.y), a)
}

/// Start of any right-click menu: restarts the cascade clock on open, updates the wave focus
pub(crate) fn cm_begin(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let fr = ctx.cumulative_pass_nr();
    let now = ctx.input(|i| i.time);
    let tid = egui::Id::new("qd_cm_t");
    let (t0, last): (f64, u64) = ctx.data(|d| d.get_temp(tid)).unwrap_or((now, 0));
    let new_open = last + 1 < fr;
    let mut t0 = if new_open { now } else { t0 };
    // Re-opening somewhere else right away: the menu slides from the old spot instead of popping
    let org = ui.cursor().min;
    let oid = egui::Id::new("qd_cm_org");
    let sid = egui::Id::new("qd_cm_slide");
    let prev_org: Option<Pos2> = ctx.data(|d| d.get_temp(oid));
    let seen_t = ctx.data(|d| d.get_temp::<(u64, f64, Rect)>(egui::Id::new("qd_cm_seen"))).map_or(-1.0, |x| x.1);
    if let Some(po) = prev_org {
        let moved = (po - org).length() > 2.0;
        if moved && (!new_open || now - seen_t < 0.2) {
            let cur_off = ctx.data(|d| d.get_temp::<(egui::Vec2, f64)>(sid)).map_or(egui::Vec2::ZERO, |(o, st)| o * (1.0 - slide_e(now - st)));
            ctx.data_mut(|d| d.insert_temp(sid, (po - org + cur_off, now)));
            // Rows stay visible while the menu slides, so it never looks empty
            t0 = now - 10.0;
        } else if new_open {
            ctx.data_mut(|d| d.remove::<(egui::Vec2, f64)>(sid));
        }
    }
    ctx.data_mut(|d| d.insert_temp(oid, org));
    ctx.data_mut(|d| {
        d.insert_temp(tid, (t0, fr));
        d.insert_temp(egui::Id::new("qd_cm_top"), ui.cursor().min.y);
    });
    // Whole menu (background included) grows out of the cursor corner
    let e = (((now - t0) / 0.28) as f32).clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - e).powi(4);
    let slide: Option<(egui::Vec2, f64)> = ctx.data(|d| d.get_temp(sid));
    let (tr, sc, busy) = match slide {
        Some((off, st)) if now - st < 0.32 => (off * (1.0 - slide_e(now - st)), 1.0, true),
        _ => {
            let sc = 0.82 + 0.18 * e;
            let o = ui.max_rect().min - vec2(8.0, 8.0);
            (o.to_vec2() * (1.0 - sc), sc, e < 1.0)
        }
    };
    // plus the canvas movement since it opened, so it rides along with its spot
    let follow: egui::Vec2 = ctx.data(|d| d.get_temp(egui::Id::new("qd_cm_follow"))).unwrap_or_default();
    ctx.set_transform_layer(ui.layer_id(), egui::emath::TSTransform::new(tr + follow, sc));
    if busy {
        ctx.request_repaint();
    }
    let prev: Rect = ctx.data(|d| d.get_temp(egui::Id::new("qd_cm_rect"))).unwrap_or(Rect::NOTHING);
    if prev.is_positive() && last + 1 >= fr {
        let (fy, wa) = list_wave(&ctx, egui::Id::new("qd_cm_wave"), prev, prev.min.y + 13.0, (prev.max.y - 13.0).max(prev.min.y + 13.0));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_cm_focus"), (fy, wa, fr)));
    }
}

pub(crate) fn slide_e(dt: f64) -> f32 {
    let k = ((dt / 0.32) as f32).clamp(0.0, 1.0);
    1.0 - (1.0 - k).powi(4)
}

/// End of any right-click menu: remember its rect for next frame's wave
pub(crate) fn cm_end(ui: &mut egui::Ui) {
    let r = ui.min_rect();
    let (pass, now) = (ui.ctx().cumulative_pass_nr(), ui.ctx().input(|i| i.time));
    ui.ctx().data_mut(|d| {
        d.insert_temp(egui::Id::new("qd_cm_rect"), r);
        d.insert_temp(egui::Id::new("qd_cm_seen"), (pass, now, r));
    });
    let shapes: Vec<egui::epaint::ClippedShape> = ui.ctx().graphics(|g| g.get(ui.layer_id()).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("qd_cm_shapes"), std::sync::Arc::new(shapes)));
}

/// After a right-click menu closes, a ghost of its panel shrinks back into the cursor corner and fades
pub(crate) fn cm_ghost(ctx: &egui::Context) {
    let Some((pass, t, r)) = ctx.data(|d| d.get_temp::<(u64, f64, Rect)>(egui::Id::new("qd_cm_seen"))) else { return };
    // Wait one extra frame: a menu re-opened elsewhere slides instead, so no ghost should flash
    if ctx.data(|d| d.get_temp::<(egui::Vec2, f64)>(egui::Id::new("qd_cm_slide"))).map_or(false, |(_, st)| st >= t) {
        return;
    }
    // While waiting to see if the menu re-opens, hold the ghost at full opacity (no blank frame)
    let cur = ctx.cumulative_pass_nr();
    if pass >= cur {
        return; // menu is open this frame
    }
    let waiting = pass + 2 >= cur;
    let now = ctx.input(|i| i.time);
    let gid = egui::Id::new("qd_cm_ghost_t0");
    let k = if waiting {
        ctx.request_repaint();
        0.0
    } else {
        // Time the fade from its own first frame, so a late start never skips or jumps part of it
        let start = match ctx.data(|d| d.get_temp::<(u64, f64)>(gid)) {
            Some((p, st)) if p == pass => st,
            _ => {
                ctx.data_mut(|d| d.insert_temp(gid, (pass, now)));
                now
            }
        };
        ((now - start) / 0.16) as f32
    };
    if !(0.0..1.0).contains(&k) {
        return;
    }
    ctx.request_repaint();
    let e = k * k;
    let sc = 1.0 - 0.12 * e;
    let full = r.expand(6.0);
    let o = full.min;
    let g = full;
    let v = ctx.style().visuals.clone();
    let lid = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("qd_cm_ghost"));
    ctx.set_transform_layer(lid, egui::emath::TSTransform::new(o.to_vec2() * (1.0 - sc), sc));
    let mut p = ctx.layer_painter(lid);
    let a = 1.0 - e;
    p.rect_filled(g, v.menu_rounding, v.window_fill.gamma_multiply(a));
    p.rect_stroke(g, v.menu_rounding, EStroke::new(v.window_stroke.width, v.window_stroke.color.gamma_multiply(a)));
    // The menu's own rows fade out with it instead of vanishing first
    if let Some(shapes) = ctx.data(|d| d.get_temp::<std::sync::Arc<Vec<egui::epaint::ClippedShape>>>(egui::Id::new("qd_cm_shapes"))) {
        p.set_opacity(a);
        for c in shapes.iter() {
            p.add(c.shape.clone());
        }
    }
}

/// Menu divider that draws itself left to right as the menu cascades in
pub(crate) fn divider(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    let (a, dy) = cm_anim(ui, r.min.y);
    let y = r.center().y + dy;
    ui.painter().with_clip_rect(ui.ctx().screen_rect()).line_segment([pos2(r.min.x, y), pos2(r.min.x + r.width() * a, y)], EStroke::new(1.0_f32, border().gamma_multiply(a)));
}

pub(crate) fn header(ui: &mut egui::Ui, text: &str) {
    // Section label, on the same left edge as the rows under it
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let (a, dy) = cm_anim(ui, rect.min.y);
    ui.painter().with_clip_rect(ui.ctx().screen_rect()).text(
        pos2(rect.min.x + MENU_INSET, rect.max.y - 4.0 + dy),
        Align2::LEFT_BOTTOM,
        text.trim_start(),
        FontId::proportional(10.0),
        TEXT_FAINT.gamma_multiply(a),
    );
}

pub(crate) fn swatches(ui: &mut egui::Ui, colors: &[&str], current: &str) -> Option<String> {
    let mut picked = None;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width().max(150.0), 30.0), Sense::hover());
    let (a, dy) = cm_anim(ui, row.min.y);
    let n = colors.len().max(2) as f32;
    // Dots are spread across the row so both ends line up with the other rows
    let (first, last) = (row.min.x + MENU_INSET + 9.0, row.max.x - MENU_INSET - 9.0);
    let step = ((last - first) / (n - 1.0)).min(26.0);
    let (gf, gp) = grid_wave(ui.ctx(), ui.id().with(("swwave", row.min.y as i32)), row);
    let p = ui.painter().clone().with_clip_rect(ui.ctx().screen_rect());
    for (i, c) in colors.iter().enumerate() {
        let base = pos2(first + step * i as f32, row.center().y);
        let r = Rect::from_center_size(base, vec2(22.0, 24.0));
        let resp = ui.interact(r, ui.id().with(("sw", row.min.y as i32, i)), Sense::click());
        let (h, pr) = hover_fx(ui.ctx(), &resp, true);
        let on = ease(ui.ctx(), ui.id().with(("sw_on", row.min.y as i32, i)), c.eq_ignore_ascii_case(current));
        let d = (base.x - gf.x) / 20.0;
        let m = (0.6 * gp * (-d * d).exp()).max(h).max(on);
        let centre = base + vec2(0.0, dy - 2.0 * m);
        let rad = 7.0 + 2.0 * m - pr * 1.2;
        let sq = Rect::from_center_size(centre, vec2(rad * 2.0, rad * 2.0));
        let round = rad * (1.0 - 0.6 * m * m);
        p.rect_filled(sq, round, hex(c).gamma_multiply(a));
        p.rect_stroke(sq, round, EStroke::new(1.0_f32, Color32::from_white_alpha((35.0 + 40.0 * m) as u8).gamma_multiply(a)));
        if on > 0.01 {
            p.rect_stroke(sq.expand(2.5), round + 2.5, EStroke::new(1.5_f32, Color32::WHITE.gamma_multiply(a * on)));
        }
        if h > 0.01 {
            p.rect_stroke(sq.expand(2.5 + 2.0 * on), round + 3.0, EStroke::new(2.0_f32, blue().gamma_multiply(a * h)));
        }
        if resp.clicked() {
            picked = Some(c.to_string());
        }
    }
    picked
}
