//! Shared painting + animation helpers used across the toolbar, bars, menus and canvas.

use super::*;

/// Eased 0..1 value that follows `on`. egui repaints only while it is moving,
/// so animations cost nothing once they settle.
pub(crate) fn ease(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, 0.22 * anim_k(), egui::emath::easing::cubic_out)
}

/// Per-element entrance for dialogs: element `i` starts a little after the one before
pub(crate) fn stagger(t: f32, i: usize) -> f32 {
    let x = (t * 1.7 - (i as f32 + 1.0) * 0.1).clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

/// Shortens long names for display: keeps `n` characters and adds an ellipsis
pub(crate) fn clip_name(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>().trim_end()) }
}

/// Alt held this frame (Alt+drag duplicates)
/// Semibold UI font, for small labels that must stay readable (size pills, measurements)
pub(crate) fn semibold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("sb".into()))
}

/// Pointer speed in screen points/s (f32 bits), so snapping can tell careful moves from quick ones
pub(crate) static PTR_SPEED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Floating canvas UI (style bar + its open dropdown) publishes its area here each frame, so
/// the canvas underneath doesn't start resizes/moves or show resize cursors through it
pub(crate) fn ui_block(ctx: &egui::Context, r: Rect) {
    let n = ctx.cumulative_pass_nr();
    let id = egui::Id::new("qd_ui_block");
    ctx.data_mut(|d| {
        // Several bars can publish in one pass; a new pass starts a fresh list
        let mut cur: (Vec<Rect>, u64) = d.get_temp(id).unwrap_or_default();
        if cur.1 != n {
            cur = (Vec::new(), n);
        }
        cur.0.push(r);
        d.insert_temp(id, cur);
    });
}

pub(crate) fn ui_blocks(ctx: &egui::Context, q: Pos2) -> bool {
    ctx.data(|d| d.get_temp::<(Vec<Rect>, u64)>(egui::Id::new("qd_ui_block")))
        .is_some_and(|(rs, n)| ctx.cumulative_pass_nr() <= n + 1 && rs.iter().any(|r| r.contains(q)))
}

pub(crate) static ALT_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Ctrl held: a canvas drag doesn't drop objects into (or out of) frames
pub(crate) static CTRL_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A board dialog is open or still animating closed (blocks the + button)
pub(crate) static PROMPT_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Animation strength (0 full, 1 subtle, 2 off), from settings
pub(crate) static ANIM: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// Duration multiplier: full 1.0, subtle 0.55, off 0 (jumps straight to the end)
pub(crate) fn anim_k() -> f32 {
    match ANIM.load(std::sync::atomic::Ordering::Relaxed) {
        0 => 1.0,
        1 => 0.55,
        _ => 0.0,
    }
}

/// Critically damped spring toward 0/1 that keeps its velocity, so flicking the pointer
/// in and out never snaps (an eased bool restarts its curve when reversed).
pub(crate) fn spring(ctx: &egui::Context, id: egui::Id, on: bool, omega: f32) -> f32 {
    let target = if on { 1.0 } else { 0.0 };
    let k = anim_k();
    if k == 0.0 {
        ctx.data_mut(|d| d.insert_temp(id, (target, 0.0_f32)));
        return target;
    }
    let omega = omega / k;
    let (mut x, mut v): (f32, f32) = ctx.data(|d| d.get_temp(id)).unwrap_or((target, 0.0));
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let steps = 4;
    let h = dt / steps as f32;
    for _ in 0..steps {
        let acc = omega * omega * (target - x) - 2.0 * omega * v;
        v += acc * h;
        x += v * h;
    }
    if (target - x).abs() < 0.001 && v.abs() < 0.01 {
        x = target;
        v = 0.0;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, (x, v)));
    x.clamp(0.0, 1.0)
}

/// 2D wave focus over an area: smoothed pointer position plus a springy presence.
pub(crate) fn grid_wave(ctx: &egui::Context, id: egui::Id, area: Rect) -> (Pos2, f32) {
    let inside = ctx.pointer_hover_pos().filter(|q| area.contains(*q));
    let pres = spring(ctx, id.with("p"), inside.is_some(), 20.0);
    let prev: Pos2 = ctx.data(|d| d.get_temp(id)).unwrap_or(inside.unwrap_or(area.center()));
    let f = match inside {
        Some(q) => {
            let q = if pres < 0.05 { q } else { q };
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            let start = if pres < 0.05 { q } else { prev };
            if (q - start).length() > 0.3 {
                ctx.request_repaint();
            }
            start + (q - start) * (1.0 - (-14.0 * dt).exp())
        }
        None => prev,
    };
    ctx.data_mut(|d| d.insert_temp(id, f));
    (f, pres)
}

/// Eased (hover, press) amounts for a custom-painted control.
pub(crate) fn hover_fx(ctx: &egui::Context, resp: &egui::Response, enabled: bool) -> (f32, f32) {
    (
        ease(ctx, resp.id.with("hov"), enabled && resp.hovered()),
        ease(ctx, resp.id.with("prs"), enabled && resp.is_pointer_button_down_on()),
    )
}

/// Blue rising like liquid in a rounded tile: a wavy surface (calm when full or empty)
pub(crate) fn liquid_fill(p: &egui::Painter, r: Rect, rad: f32, level: f32, time: f32) {
    let n = 28;
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let amp = 4.5 * (level * std::f32::consts::PI).sin().powf(0.6);
    let base = r.max.y - r.height() * level;
    let edge = |x: f32| {
        // vertical inset of the rounded corners at this x
        let d = (x - r.min.x).min(r.max.x - x);
        if d >= rad { 0.0 } else { rad - (rad * rad - (rad - d) * (rad - d)).max(0.0).sqrt() }
    };
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=n {
        let x = r.min.x + r.width() * i as f32 / n as f32;
        let u = i as f32 / n as f32;
        let wave = amp * ((u * 5.0 + time * 11.0).sin() * 0.55 + (u * 9.5 - time * 7.5).sin() * 0.3 + (u * 15.0 + time * 14.0).sin() * 0.15);
        let inset = edge(x);
        let bot = r.max.y - inset;
        let top = (base + wave).max(r.min.y + inset).min(bot);
        mesh.colored_vertex(pos2(x, top), blue());
        mesh.colored_vertex(pos2(x, bot), blue());
        if i > 0 {
            let j = (i as u32) * 2;
            mesh.add_triangle(j - 2, j - 1, j);
            mesh.add_triangle(j - 1, j + 1, j);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

/// A frame's corner radius in world units (old frames: rounded unless set to square)
pub(crate) fn group_radius(g: &Group) -> f32 {
    g.corner.unwrap_or(if g.square { 0.0 } else { 6.0 })
}

/// Springy hover/press for the bottom bar buttons
pub(crate) fn bar_fx(ctx: &egui::Context, resp: &egui::Response, enabled: bool) -> (f32, f32) {
    (spring(ctx, resp.id.with("bh"), enabled && resp.hovered(), 16.0), spring(ctx, resp.id.with("bp"), enabled && resp.is_pointer_button_down_on(), 26.0))
}

/// Lays out a text label with its style (0 normal, 1 bold, 2 italic, 3 bold italic)
pub(crate) fn label_galley(p: &egui::Painter, text: &str, f: FontId, col: Color32, style: u8) -> std::sync::Arc<egui::Galley> {
    p.layout_no_wrap(text.to_string(), styled_font(f, style), col)
}

/// The font face for a label style (0 regular, 1 bold, 2 italic, 3 bold italic)
pub(crate) fn styled_font(f: FontId, style: u8) -> FontId {
    let fam = match style {
        1 => egui::FontFamily::Name("b".into()),
        2 => egui::FontFamily::Name("i".into()),
        3 => egui::FontFamily::Name("bi".into()),
        _ => return f,
    };
    FontId::new(f.size, fam)
}

/// Zoom-stable text: lays the text out at a size that only changes at power-of-two zoom steps,
/// then scales the glyph mesh continuously, so size and line breaks never jitter while zooming
pub(crate) fn smooth_text(p: &egui::Painter, pos: Pos2, text: &str, family: egui::FontFamily, world_fs: f32, z: f32, wrap_world: Option<f32>, col: Color32) {
    let mut tier = 2f32.powf(z.max(1e-3).log2().ceil());
    while world_fs * tier > 200.0 && tier > 1e-3 {
        tier /= 2.0;
    }
    let f = FontId::new(world_fs * tier, family);
    let g = match wrap_world {
        Some(w) => p.layout(text.to_owned(), f, col, (w * tier).max(10.0)),
        None => p.layout_no_wrap(text.to_owned(), f, col),
    };
    let sc = z / tier;
    let [aw, ah] = p.ctx().fonts(|fo| fo.font_image_size());
    let mut mesh = egui::epaint::Mesh::with_texture(egui::TextureId::default());
    for row in &g.rows {
        let base = mesh.vertices.len() as u32;
        mesh.indices.extend(row.visuals.mesh.indices.iter().map(|i| i + base));
        for vx in &row.visuals.mesh.vertices {
            let color = if vx.color == Color32::PLACEHOLDER { col } else { vx.color };
            mesh.vertices.push(egui::epaint::Vertex { pos: pos + vx.pos.to_vec2() * sc, uv: pos2(vx.uv.x / aw as f32, vx.uv.y / ah as f32), color });
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

pub(crate) fn draw_label(p: &egui::Painter, pos: Pos2, g: std::sync::Arc<egui::Galley>, col: Color32, _style: u8, _size: f32) {
    p.galley(pos, g, col);
}

/// The toolbar's "wave" for a vertical list: a smoothed pointer focus inside `area`,
/// clamped between the first and last row centres. Returns (focus y, presence 0..1).
pub(crate) fn list_wave(ctx: &egui::Context, id: egui::Id, area: Rect, first: f32, last: f32) -> (f32, f32) {
    let ptr = ctx.pointer_hover_pos();
    let inside = ptr.map_or(false, |q| area.contains(q));
    let a = spring(ctx, id.with("a"), inside, 20.0);
    let mut f = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(first);
    if let (true, Some(q)) = (inside, ptr) {
        let target = q.y.clamp(first, last);
        if a < 0.05 {
            f = target;
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        f += (target - f) * (1.0 - (-dt * 14.0).exp());
        if (target - f).abs() > 0.05 {
            ctx.request_repaint();
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, f));
    (f, a)
}

/// Lays out a vertical list the way the toolbar magnifies: rows near the pointer (and
/// the chosen one) grow taller and poke out to the right; the left edge stays put and
/// the panel grows upward from its bottom. Returns (row rects, magnify per row, panel).
pub(crate) fn wave_rows(ctx: &egui::Context, id: egui::Id, left: f32, bottom: f32, w: f32, pitch: f32, n: usize, pinned: &[f32]) -> (Vec<Rect>, Vec<f32>, Rect) {
    wave_rows_k(ctx, id, left, bottom, w, pitch, n, pinned, 1.0, false)
}

/// `wave_rows` with the wave's spread scaled by `k` (1 = the menus' wave).
/// `lock`: only the pinned rows rise (the others stay flat, e.g. while it's being dragged).
#[allow(clippy::too_many_arguments)]
pub(crate) fn wave_rows_k(ctx: &egui::Context, id: egui::Id, left: f32, bottom: f32, w: f32, pitch: f32, n: usize, pinned: &[f32], k: f32, lock: bool) -> (Vec<Rect>, Vec<f32>, Rect) {
    if n == 0 {
        let r = Rect::from_min_max(pos2(left, bottom - 8.0), pos2(left + w, bottom));
        return (Vec::new(), Vec::new(), r);
    }
    let base_h = n as f32 * pitch + 8.0;
    let base = Rect::from_min_max(pos2(left, bottom - base_h), pos2(left + w, bottom));
    let first = base.min.y + 4.0 + pitch / 2.0;
    let (fy, wa) = list_wave(ctx, id, base.expand2(vec2(14.0, 6.0)), first, first + (n as f32 - 1.0) * pitch);
    // The other rows settle / come back with the same easing both ways (no snap on let-go)
    let lk = ease(ctx, id.with("lock"), lock);
    // While locked the held row (the most pinned one) stays fully up and the rest stay flat; the
    // lock blends between that and the wave, so pressing / letting go with the pointer still on
    // the held row doesn't dip it.
    let hid = id.with("lock_row");
    let mut held: Option<usize> = ctx.data(|d| d.get_temp::<Option<usize>>(hid)).flatten();
    if lock {
        if let Some((i, _)) = pinned.iter().enumerate().filter(|(_, v)| **v > 0.0).max_by(|a, b| a.1.total_cmp(b.1)) {
            held = Some(i);
        }
    } else if lk < 0.001 {
        held = None;
    }
    ctx.data_mut(|d| d.insert_temp(hid, held));
    // `k` narrows the wave: the row under the pointer still rises fully, its neighbours less
    let m: Vec<f32> = (0..n)
        .map(|i| {
            let c = first + i as f32 * pitch;
            let wv = wa * (-((c - fy) / (pitch * k)).powi(2)).exp();
            let hold = if held == Some(i) { 1.0 } else { 0.0 };
            let wv = wv + (hold - wv) * lk;
            if lk > 0.0 { wv } else { wv.max(pinned.get(i).copied().unwrap_or(0.0)) }
        })
        .collect();
    let hs: Vec<f32> = m.iter().map(|k| pitch * (1.0 + 0.22 * k)).collect();
    let panel_h = hs.iter().sum::<f32>() + 8.0;
    let panel = Rect::from_min_max(pos2(left, bottom - panel_h), pos2(left + w, bottom));
    let mut y = panel.min.y + 4.0;
    let rows = (0..n)
        .map(|i| {
            let r = Rect::from_min_size(pos2(left + 4.0 + 10.0 * m[i], y), vec2(w - 8.0 + 4.0 * m[i], hs[i]));
            y += hs[i];
            r
        })
        .collect();
    (rows, m, panel)
}

/// The raised tile behind a magnified row
pub(crate) fn wave_tile(p: &egui::Painter, r: Rect, m: f32, fill: Color32, a: f32) {
    if m <= 0.01 {
        return;
    }
    p.rect_filled(r.translate(vec2(0.0, 3.0 * m)), 8.0, Color32::from_black_alpha((60.0 * m * a) as u8));
    p.rect_filled(r, 8.0, fill.gamma_multiply(m * a));
}

pub(crate) fn hover_fill(h: f32, p: f32) -> Color32 {
    Color32::from_white_alpha((h * 18.0 + p * 20.0) as u8)
}
/// Frame size presets: label, width, height
pub(crate) const FRAME_SIZES: [(&str, f32, f32); 6] = [("Free", 380.0, 280.0), ("16:9", 480.0, 270.0), ("9:16", 270.0, 480.0), ("1:1", 360.0, 360.0), ("4:3", 400.0, 300.0), ("3:4", 300.0, 400.0)];
pub(crate) const PEN_COLORS: [&str; 7] = ["#ef4444", "#f97316", "#facc15", "#22c55e", "#06b6d4", "#3b82f6", "#ffffff"];
pub(crate) const NOTE_COLORS: [&str; 6] = ["#303030", "#3f2a0a", "#0f3324", "#2a2a3e", "#3b0f4a", "#4a1220"];

/// The active tab as a single shape: rounded top corners, flared feet that curve
/// into the canvas, and a flat bottom exactly on the canvas edge. One mesh means
/// no seams between pieces; a thin outline on top gives the curves soft edges.
/// Returned as shapes so the title bar can slot it in under every tab's title while it slides.
pub(crate) fn active_tab_shapes(top: f32, x: f32, w: f32, bot: f32, canvas: Rect, now: f64) -> Vec<egui::Shape> {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    let (r_top, r_foot) = (8.0_f32, 7.0_f32);
    let mut pts: Vec<Pos2> = Vec::new();
    let arc = |pts: &mut Vec<Pos2>, c: Pos2, r: f32, a0: f32, a1: f32| {
        for i in 0..=12 {
            let a = a0 + (a1 - a0) * i as f32 / 12.0;
            pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    };
    use std::f32::consts::PI;
    // left foot: from the canvas edge, curving up into the tab's left side
    arc(&mut pts, pos2(x - r_foot, bot - r_foot), r_foot, PI * 0.5, 0.0);
    // top-left, top-right
    arc(&mut pts, pos2(x + r_top, top + r_top), r_top, PI, PI * 1.5);
    arc(&mut pts, pos2(x + w - r_top, top + r_top), r_top, PI * 1.5, PI * 2.0);
    // right foot
    arc(&mut pts, pos2(x + w + r_foot, bot - r_foot), r_foot, PI, PI * 0.5);

    let colour = |q: Pos2| aura_colour_at(q, canvas, now);
    let mut m = Mesh::default();
    let hub = pos2(x + w * 0.5, bot);
    m.vertices.push(Vertex { pos: hub, uv: WHITE_UV, color: colour(hub) });
    for q in &pts {
        m.vertices.push(Vertex { pos: *q, uv: WHITE_UV, color: colour(*q) });
    }
    // The outline is star-shaped from the middle of its bottom edge, so a fan works
    for i in 1..pts.len() as u32 {
        m.add_triangle(0, i, i + 1);
    }
    let mut out = vec![egui::Shape::mesh(m)];
    // Soft edge on the curved sides only (the bottom must stay flush with the canvas)
    for pair in pts.windows(2) {
        let c = colour(pair[0]);
        out.push(egui::Shape::line_segment([pair[0], pair[1]], EStroke::new(1.0_f32, c)));
    }
    out
}

/// World-scaled font, or None when it would be too small to read.
pub(crate) fn font(size: f32) -> Option<FontId> {
    // Half-pixel steps: whole-pixel rounding made text visibly jump while zooming
    let s = (size * 2.0).round() / 2.0;
    (s >= 5.0).then(|| FontId::proportional(s.min(200.0)))
}

/// Text on a magnified bar segment, kept sharp: drawn at the swollen size on its own layer and
/// scaled down to the segment's current size (scaling small glyphs up is what blurred them).
/// `pl` is the segment's scaled painter (layer transform `tv`, `sc`); `pos` in its coordinates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zoom_text(pl: &egui::Painter, tv: egui::Vec2, sc: f32, pos: Pos2, align: Align2, text: impl ToString, size: f32, col: Color32) {
    const S: f32 = 1.25;
    let ctx = pl.ctx();
    let lid = pl.layer_id();
    let tid = egui::LayerId::new(lid.order, lid.id.with("ztxt"));
    // egui scales text as a bitmap, so any scale but 1 resamples it (soft). While the size holds
    // still, draw real glyphs at the swollen size instead; only the grow / shrink is resampled.
    let pass = ctx.cumulative_pass_nr();
    let sid = tid.id.with("still");
    let (last, seen, mut still): (f32, u64, bool) = ctx.data(|d| d.get_temp(sid)).unwrap_or((sc, 0, true));
    if seen != pass {
        still = (sc - last).abs() < 0.0015;
        ctx.data_mut(|d| d.insert_temp(sid, (sc, pass, still)));
    }
    if still {
        let c = pl.clip_rect();
        let tf = |q: Pos2| pos2(q.x * sc + tv.x, q.y * sc + tv.y);
        let cid = egui::LayerId::new(lid.order, lid.id.with("ztxt_crisp"));
        ctx.set_transform_layer(cid, egui::emath::TSTransform::IDENTITY);
        let p = pl.clone().with_layer_id(cid).with_clip_rect(Rect::from_min_max(tf(c.min), tf(c.max)));
        // Quarter-pixel sizes: sharp, and only a handful of sizes ever reach the glyph atlas
        let fs = (size * sc * 4.0).round() / 4.0;
        p.text(tf(pos), align, text.to_string(), FontId::proportional(fs), col);
        return;
    }
    ctx.request_repaint();
    ctx.set_transform_layer(tid, egui::emath::TSTransform::new(tv, sc / S));
    let c = pl.clip_rect();
    let p = pl.clone().with_layer_id(tid).with_clip_rect(Rect::from_min_max(pos2(c.min.x * S, c.min.y * S), pos2(c.max.x * S, c.max.y * S)));
    p.text(pos2(pos.x * S, pos.y * S), align, text.to_string(), FontId::proportional(size * S), col);
}
