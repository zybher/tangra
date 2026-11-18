//! Properties: a floating card in the canvas's top-right corner (rows like Figma's). Shows
//! only what the selection can change, like the hover bars (which stay): position and size,
//! align, opacity and corners, fill, stroke, text size, and a picture's adjustments and 3D.
//! Number fields scrub sideways, or take a typed value on click. With nothing selected it's a
//! small card with a hint; picking something morphs it (same glide as the bars) to a slim,
//! fixed width and the height of what that object has. Numbers behave like the bars': drag
//! to scrub, double-click to type (num_edit.rs), and the shown value counts to its new one.
//! Size has an aspect lock; a picture's flat rotation sits under Position (out of the 3D rows).
//! Tabs at the top: Design (all of the above) and Animate (a placeholder for now).

use crate::image_edit::{ImageEdit, ADJUST_ROWS, SPIN_ROW, TILT_ROWS};
use crate::*;

/// Field height
const FH: f32 = 26.0;
const GAP: f32 = 6.0;
/// Card padding and distance from the canvas edges
const PAD: f32 = 10.0;
/// Card width with something selected (slim, like Figma's)
const CARD_W: f32 = 228.0;
const EDGE: f32 = 12.0;
const HINT: &str = "Select something to see its properties";

enum PAct {
    /// key, value, first change of this edit (takes the undo step)
    Num(&'static str, f32, bool),
    /// width and height together (aspect lock)
    Size(f32, f32, bool),
    Align(Align),
    Dist(bool),
    Color(String),
    StrokePos(u8),
    /// arrow head style / line ends (canvas/arrow_mesh.rs)
    Tip(u8),
    Cap(u8),
    Edit(u8, f32, bool),
    Reset(u8),
    /// the picked path point (canvas/path_edit.rs): "px" / "py" / "pr" (corner), value, first
    Pt(&'static str, f32, bool),
    Mirror(u8),
    /// blend mode for everything selected (canvas/layers.rs BLENDS)
    Blend(u8),
    /// gradient fill (canvas/gradient.rs): kind (None = solid), pick / move / add / remove a
    /// stop, the picked stop's opacity, flip
    GKind(Option<u8>),
    GPick(usize),
    GMove(usize, f32),
    GAdd(f32),
    GDel(usize),
    GAlpha(usize, f32, bool),
    GAt(usize, f32, bool),
    GRotate,
    GColor(usize, String),
    GSmooth(f32, bool),
    GFlip,
}

/// Icons for the blend modes, in `canvas::layers::BLENDS` order
const BLEND_ICONS: [&str; 12] = [
    "blend-normal", "blend-multiply", "blend-screen", "blend-overlay", "blend-darken", "blend-lighten", "blend-color-dodge", "blend-color-burn", "blend-hard-light", "blend-soft-light", "blend-difference", "blend-exclusion",
];

/// A colour chip and its hex inside `r` (a field-style row). The colour glides to a new one
/// (so switching gradient stops eases rather than snaps); "none" shows as an empty chip
fn color_chip(ctx: &egui::Context, p: &egui::Painter, id: egui::Id, r: Rect, color: &str, hl: f32, label: bool, a: f32) {
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
/// along, drag it down off the bar (or right-click it) to remove it, click the bar to add one
fn gradient_bar(ui: &mut egui::Ui, id: egui::Id, r: Rect, g: &Gradient, pick: Option<usize>, a: f32, out: &mut Vec<PAct>) {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    let bar = Rect::from_min_size(r.min, vec2(r.width(), 22.0));
    // the colours run between the dots' outermost spots; the ends carry the end colours on
    // (inset enough that an end dot, grown and ringed, stays inside the panel)
    let ramp = bar.shrink2(vec2(14.0, 0.0));
    let cp = p.with_clip_rect(bar.shrink(1.0).intersect(p.clip_rect()));
    let sq = 5.5;
    let mut x = bar.min.x;
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
    let resp = ui.interact(bar.expand2(vec2(0.0, 4.0)), id, Sense::click_and_drag()).on_hover_text("Click to add a stop · drag a dot down to remove it");
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
    let off = |q: Pos2| q.y > bar.max.y + 22.0 && g.stops.len() > 2;
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
            if near(q).is_none() {
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

/// Figma-style number field: label left, value right. Drag sideways to scrub, double-click
/// to type (num_edit.rs, like the bars); the shown number counts to a new value.
/// `liquid`: fills with the accent up to the value on hover, like the bars' opacity.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, val: f32, unit: &str, per_px: f32, lo: f32, hi: f32, liquid: bool, a: f32) -> Option<(f32, bool)> {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    let lx = r.min.x + 8.0;
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let (hh, pp) = hover_fx(&ctx, &resp, true);
    p.rect_filled(r, 6.0, hover_fill(hh, pp).gamma_multiply(a));
    let mut hl = hh;
    if liquid {
        let lq = ctx.animate_bool_with_time(id.with("liquid"), resp.hovered() || resp.dragged(), 0.5 * anim_k());
        let lq = lq * lq * (3.0 - 2.0 * lq);
        let lvl = ctx.animate_value_with_time(id.with("lvl"), ((val - lo) / (hi - lo)).clamp(0.0, 1.0), 0.12 * anim_k()) * lq;
        if lvl >= 0.999 {
            p.rect_filled(r, 6.0, blue().gamma_multiply(a));
        } else if lvl > 0.001 {
            liquid_fill(&p, r, 6.0, lvl, ctx.input(|i| i.time) as f32 + 2.0);
        }
        if lq > 0.001 {
            ctx.request_repaint();
        }
        hl = hl.max(lq);
    }
    if resp.hovered() || resp.dragged() {
        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    let mut out = None;
    // Double-click to type it; typed / nudged values morph in
    let ne = super::num_edit::NumEdit::run(&ctx, &resp, a > 0.5, val, lo, hi);
    if let Some(v) = ne.apply {
        out = Some((v, ne.first));
    }
    let cy = r.center().y;
    ne.draw(&p, Some(r), pos2(r.max.x - 8.0, cy), 12.0);
    let lc = if liquid { mix(txt_dim(), crate::theme::hi(), hl) } else { mix(txt_dim(), txt(), hl * 0.5) };
    p.text(pos2(lx, cy), Align2::LEFT_CENTER, label, FontId::proportional(11.0), lc.gamma_multiply(a));
    let vc = if liquid { mix(txt(), crate::theme::hi(), hl) } else { mix(gray_ui(200), txt(), hl.max(ne.e)) };
    // Counts to a new value (follows the pointer exactly while scrubbed)
    let shown = super::micro::glide(&ctx, id.with("num"), val, resp.dragged());
    if shown != val {
        ctx.request_repaint();
    }
    let txt = ne.text.clone().unwrap_or_else(|| format!("{}{}", shown.round() as i32, unit));
    p.with_clip_rect(r).text(pos2(r.max.x - 8.0, cy), Align2::RIGHT_CENTER, txt, FontId::proportional(12.0), vc.gamma_multiply(a));
    if resp.drag_started() {
        ctx.data_mut(|d| d.insert_temp(id.with("v0"), val));
    }
    if resp.dragged() {
        let v0: f32 = ctx.data(|d| d.get_temp(id.with("v0"))).unwrap_or(val);
        let dx = super::scrub_wrap::dx(&ctx);
        let nv = (v0 + dx * per_px).round().clamp(lo, hi);
        if nv != val || resp.drag_started() {
            out = Some((nv, resp.drag_started()));
        }
    }
    out
}

/// Two side-by-side halves of a row
fn halves(r: Rect) -> (Rect, Rect) {
    let m = r.center().x;
    (Rect::from_min_max(r.min, pos2(m - GAP / 2.0, r.max.y)), Rect::from_min_max(pos2(m + GAP / 2.0, r.min.y), r.max))
}

/// Small text button (section header links such as "Reset")
fn link(ui: &mut egui::Ui, id: egui::Id, right: Pos2, text: &str, a: f32) -> bool {
    let p = ui.painter().clone();
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(11.0), txt_dim());
    let r = Rect::from_min_size(pos2(right.x - g.size().x, right.y - g.size().y / 2.0), g.size());
    let resp = ui.interact(r.expand(3.0), id, Sense::click());
    let (hh, _) = hover_fx(ui.ctx(), &resp, true);
    p.text(pos2(r.max.x, right.y), Align2::RIGHT_CENTER, text, FontId::proportional(11.0), mix(txt_dim(), blue(), hh).gamma_multiply(a));
    resp.clicked()
}

impl App {
    /// The properties card, top right of the canvas
    pub(crate) fn props_ui(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>, canvas: Rect) {
        let bid = egui::Id::new("qd_props");
        let target: Option<Sel> = if self.sel_label.is_some() || self.selected.is_empty() {
            None
        } else {
            self.sel.clone().or_else(|| self.selected.iter().next().cloned())
        }
        .filter(|s| self.rect_of(s.0, &s.1).is_some());
        let has = target.is_some();
        let now = self.now;
        let k = anim_k().max(1e-3);
        let was: bool = ctx.data(|d| d.get_temp(bid.with("was"))).unwrap_or(false);
        let last: Option<Sel> = ctx.data(|d| d.get_temp(bid.with("last"))).flatten();
        ctx.data_mut(|d| {
            if has && !was {
                d.insert_temp(bid.with("t0"), now);
            }
            if !has && was {
                d.insert_temp(bid.with("t_gone"), now);
            }
            // Rows cascade in again when the selection changes
            if has && target != last {
                d.insert_temp(bid.with("t_sel"), now);
            }
            d.insert_temp(bid.with("was"), has);
            if has {
                d.insert_temp(bid.with("last"), target.clone());
            }
        });
        // Content fades out on deselect (the last object's rows stay while they go)
        let ca = ease(ctx, bid.with("ca"), has);
        let shown = if has { target } else { last }.filter(|s| ca > 0.01 && self.rect_of(s.0, &s.1).is_some());

        // Card size: the hint's when empty; else slim and the content's height (measured last pass)
        let max_h = (canvas.height() - 2.0 * EDGE - 70.0).max(120.0);
        let hint_w = ctx.fonts(|f| f.layout_no_wrap(HINT.to_string(), FontId::proportional(11.5), txt_dim()).size().x);
        let (tw, th) = if has {
            let ch: f32 = ctx.data(|d| d.get_temp(bid.with("ch"))).unwrap_or(240.0);
            (CARD_W.max(hint_w + 2.0 * PAD + 8.0), (ch + 2.0 * PAD).min(max_h))
        } else {
            (hint_w + 2.0 * PAD + 8.0, 44.0)
        };
        let w = super::micro::glide(ctx, bid.with("gw"), tw, false);
        let h = super::micro::glide(ctx, bid.with("gh"), th, false);
        if w != tw || h != th {
            ctx.request_repaint();
        }
        let card = Rect::from_min_size(pos2(canvas.max.x - EDGE - w, canvas.min.y + EDGE), vec2(w, h));
        // the hover bars keep clear of it (bar_morph::wall_hi)
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_props_card"), card));
        // Design / Animate tab (switching cascades the rows in again)
        let tab: u8 = ctx.data(|d| d.get_temp(bid.with("tab"))).unwrap_or(0);
        let t_sel: f64 = ctx.data(|d| d.get_temp(bid.with("t_sel"))).unwrap_or(now);
        let xs = ((now - t_sel) as f32 / (0.6 * k)).clamp(0.0, 1.0);
        if xs < 1.0 {
            ctx.request_repaint();
        }

        let paint_card = |ui: &egui::Ui| {
            let p = ui.painter();
            p.rect_filled(card.translate(vec2(0.0, 2.0)), 10.0, Color32::from_black_alpha(40));
            p.rect_filled(card, 10.0, frame_bg());
            p.rect_stroke(card, 10.0, EStroke::new(1.0_f32, border()));
            let ha = 1.0 - ca;
            if ha > 0.01 {
                p.with_clip_rect(card).text(card.center(), Align2::CENTER_CENTER, HINT, FontId::proportional(11.5), txt_dim().gamma_multiply(ha));
            }
        };
        let Some((kind, id)) = shown else {
            egui::Area::new(bid).order(egui::Order::Middle).fixed_pos(card.min).show(ctx, |ui| {
                ui.allocate_exact_size(card.size(), Sense::click());
                paint_card(ui);
            });
            return;
        };

        // What this selection can change
        let r0 = self.rect_of(kind, &id).unwrap_or([0.0; 4]);
        let single = self.selected.len() <= 1;
        let targets = self.place_targets();
        let locked = self.place_locked_by_layout(&targets);
        let align = !locked && self.share_frame(&targets);
        let opacity = (self.opacity_of(&id) * 100.0).round();
        let mut corner: Option<f32> = None;
        let mut fill: Option<String> = None;
        let mut stroke_w: Option<(&'static str, f32, f32, f32, f32)> = None;
        let mut spos: Option<u8> = None;
        let mut tsize: Option<f32> = None;
        let mut edit: Option<ImageEdit> = None;
        let mut arrow: Option<(u8, u8)> = None;
        let title = match kind {
            Kind::Group => {
                if let Some(g) = self.board.group_cards.iter().find(|g| g.id == id) {
                    corner = Some(group_radius(g));
                    fill = Some(g.color.clone());
                    stroke_w = Some(("border", g.border.unwrap_or(1.0), 0.0, 12.0, 0.06));
                    spos = Some(g.stroke_pos);
                }
                "Frame"
            }
            Kind::Stroke => {
                if let Some(st) = self.find_stroke(&id) {
                    fill = Some(st.color.clone());
                    if st.closed_len() == Some(4) {
                        corner = Some(st.radius.unwrap_or(0.0));
                    }
                    stroke_w = Some(("width", st.width, 0.5, 500.0, 0.2));
                    if st.tool_kind() == "arrow" {
                        arrow = Some((st.tip, st.cap));
                    }
                    if st.closed_len().is_some() {
                        spos = Some(st.stroke_pos);
                    }
                }
                "Shape"
            }
            Kind::Text => {
                if let Some(t) = self.board.world_texts.iter().find(|t| t.id == id) {
                    fill = Some(t.color.clone());
                    tsize = Some(if t.font_size > 0.0 { t.font_size } else { 16.0 });
                }
                "Text"
            }
            Kind::Image => {
                if let Some(i) = self.board.images.iter().find(|i| i.id == id) {
                    corner = Some(i.corner);
                    edit = Some(i.edit.clone().unwrap_or_default());
                }
                "Picture"
            }
            Kind::Note => "Note",
            Kind::Link => "Link",
        };
        let n_sel = self.selected.len();
        let is_group = kind == Kind::Group;
        // Gradient fill: frames and closed shapes / paths (canvas/gradient.rs)
        let can_grad = single
            && match kind {
                Kind::Group => true,
                Kind::Stroke => self.find_stroke(&id).is_some_and(|s| s.closed_len().is_some() || (s.closed && !s.knots.is_empty())),
                _ => false,
            };
        let grad = if can_grad { self.gradient_of(&id) } else { None };
        let gstop = self.grad_edit.as_ref().filter(|e| e.0 == id).map(|e| e.1).filter(|j| grad.as_ref().is_some_and(|g| *j < g.stops.len()));
        let blend_now = self.board.blend.get(&id).copied().unwrap_or(0);
        // Editing this path's points with one picked: its own rows
        let pick = match (&self.path_edit, kind) {
            (Some(e), Kind::Stroke) if e.id == id => self.picked_knot(),
            _ => None,
        };
        // the point's rows cascade in when it's picked
        let pk_now = pick.map(|_| self.path_edit.as_ref().and_then(|e| e.sel)).flatten();
        let pk_was: Option<usize> = ctx.data(|d| d.get_temp(bid.with("pk"))).flatten();
        if pk_now.is_some() && pk_now != pk_was {
            ctx.data_mut(|d| d.insert_temp(bid.with("t_sel"), now));
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("pk"), pk_now));

        let mut out: Vec<PAct> = Vec::new();
        egui::Area::new(bid).order(egui::Order::Middle).fixed_pos(card.min).show(ctx, |ui| {
            ui.allocate_exact_size(card.size(), Sense::click());
            paint_card(ui);
            ui.set_opacity(ca);
            let cr = card.shrink(PAD);
            #[allow(deprecated)]
            ui.allocate_ui_at_rect(cr, |ui| {
                ui.set_clip_rect(cr);
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(cr.height()).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let w = ui.available_width() - 4.0;
                    let y_top = ui.min_rect().min.y;
                    // Each row rises into place a beat after the one above (like the bars' buttons)
                    let mut ri = 0usize;
                    let mut row = |ui: &mut egui::Ui, h: f32| -> (Rect, f32) {
                        let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                        let gi = ((xs - 0.02 - ri as f32 * 0.035) / 0.3).clamp(0.0, 1.0);
                        ri += 1;
                        let ca = 1.0 - (1.0 - gi).powi(3);
                        // 2 px clear of the card's clip on both sides, so edges and strokes drawn
                        // right on a row's border are never shaved off
                        (r.translate(vec2(2.0, (1.0 - ca) * 8.0)), ca)
                    };
                    let head = |ui: &mut egui::Ui, row: &mut dyn FnMut(&mut egui::Ui, f32) -> (Rect, f32), name: &str, first: bool| -> (Rect, f32) {
                        if !first {
                            let (r, a) = row(ui, 13.0);
                            ui.painter().line_segment([pos2(r.min.x, r.center().y + 2.0), pos2(r.max.x, r.center().y + 2.0)], EStroke::new(1.0_f32, border().gamma_multiply(a)));
                        }
                        let (r, a) = row(ui, 24.0);
                        ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(11.5), gray_ui(225).gamma_multiply(a));
                        (r, a)
                    };
                    let gap = |ui: &mut egui::Ui| ui.add_space(GAP);

                    // Tabs: a pill slides to the picked one (like the stroke position switch)
                    {
                        let (r, a) = row(ui, FH);
                        let at = ctx.animate_value_with_time(bid.with("tab_k"), tab as f32, 0.2 * anim_k());
                        let p = ui.painter();
                        p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                        let half = r.width() / 2.0;
                        p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at * half, r.min.y + 2.0), vec2(half - 4.0, r.height() - 4.0)), 5.0, wa((22.0 * a) as u8));
                        for (j, name) in ["Design", "Animate"].into_iter().enumerate() {
                            let br = Rect::from_min_size(pos2(r.min.x + j as f32 * half, r.min.y), vec2(half, r.height()));
                            let resp = ui.interact(br, bid.with(("tab", j)), Sense::click());
                            let on = j as u8 == tab;
                            let (hh, pp) = hover_fx(ctx, &resp, !on);
                            if !on {
                                ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp).gamma_multiply(a));
                            }
                            let onk = (1.0 - (at - j as f32).abs()).clamp(0.0, 1.0);
                            ui.painter().text(br.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.5), mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(a));
                            if resp.clicked() && !on {
                                ctx.data_mut(|d| {
                                    d.insert_temp(bid.with("tab"), j as u8);
                                    d.insert_temp(bid.with("t_sel"), now);
                                });
                            }
                        }
                        gap(ui);
                    }

                    // Title: what's selected
                    let (r, a) = row(ui, 22.0);
                    let label = if n_sel > 1 { format!("{title}  ·  {n_sel} selected") } else { title.to_string() };
                    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), gray_ui(235).gamma_multiply(a));
                    let mut first = false;

                    // Animate: a placeholder (prototype-style interactions come later)
                    if tab == 1 {
                        head(ui, &mut row, "Interaction", first);
                        for (k, v) in [("Trigger", "On click"), ("Animation", "Smart animate"), ("Curve", "Ease out"), ("Duration", "300 ms")] {
                            let (r, a) = row(ui, FH);
                            let p = ui.painter();
                            p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                            p.text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, k, FontId::proportional(11.0), txt_dim().gamma_multiply(a * 0.7));
                            p.text(pos2(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, v, FontId::proportional(12.0), gray_ui(200).gamma_multiply(a * 0.6));
                            gap(ui);
                        }
                        let (r, a) = row(ui, 22.0);
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "Coming soon", FontId::proportional(11.0), txt_dim().gamma_multiply(a));
                    }
                    let design = tab == 0;

                    // The picked point: where it is, how its handles follow, corner rounding
                    if let (true, Some(k)) = (design, pick) {
                        head(ui, &mut row, "Point", first);
                        first = false;
                        let (r, a) = row(ui, FH);
                        let (l, rr) = halves(r);
                        if let Some((v, f1)) = field(ui, bid.with("px"), l, "X", k.p.x, "", 1.0, -1e6, 1e6, false, a) {
                            out.push(PAct::Pt("px", v, f1));
                        }
                        if let Some((v, f1)) = field(ui, bid.with("py"), rr, "Y", k.p.y, "", 1.0, -1e6, 1e6, false, a) {
                            out.push(PAct::Pt("py", v, f1));
                        }
                        gap(ui);
                        if k.sharp() {
                            // a sharp point can be rounded off
                            let (r, a) = row(ui, FH);
                            if let Some((v, f1)) = field(ui, bid.with("pr"), r, "Corner", k.r, "", 0.25, 0.0, 1000.0, false, a) {
                                out.push(PAct::Pt("pr", v, f1));
                            }
                        } else {
                            // Mirroring: a pill slides to the picked third (as the stroke position)
                            let (r, a) = row(ui, FH);
                            let opts: [(&str, u8); 3] = [("None", 1), ("Angle", 0), ("Angle+Len", 2)];
                            let cur = opts.iter().position(|o| o.1 == k.m).unwrap_or(1) as f32;
                            let at = ctx.animate_value_with_time(bid.with("pm_k"), cur, 0.2 * anim_k());
                            let p = ui.painter();
                            p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                            let third = r.width() / 3.0;
                            p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at * third, r.min.y + 2.0), vec2(third - 4.0, r.height() - 4.0)), 5.0, wa((22.0 * a) as u8));
                            for (j, (name, val)) in opts.into_iter().enumerate() {
                                let br = Rect::from_min_size(pos2(r.min.x + j as f32 * third, r.min.y), vec2(third, r.height()));
                                let resp = ui.interact(br, bid.with(("pm", j)), Sense::click()).on_hover_text(match val {
                                    1 => "Handles move on their own",
                                    0 => "Handles stay in line",
                                    _ => "Handles stay in line and the same length",
                                });
                                let on = val == k.m;
                                let (hh, pp) = hover_fx(ctx, &resp, !on);
                                if !on {
                                    ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp).gamma_multiply(a));
                                }
                                let onk = (1.0 - (at - j as f32).abs()).clamp(0.0, 1.0);
                                ui.painter().text(br.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.0), mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(a));
                                if resp.clicked() && !on {
                                    out.push(PAct::Mirror(val));
                                }
                            }
                        }
                    }

                    // Position and size (one object, not one the frame arranges)
                    if design && single && !locked {
                        head(ui, &mut row, "Position", first);
                        first = false;
                        let (r, a) = row(ui, FH);
                        let (l, rr) = halves(r);
                        if let Some((v, f1)) = field(ui, bid.with("x"), l, "X", r0[0], "", 1.0, -1e6, 1e6, false, a) {
                            out.push(PAct::Num("x", v, f1));
                        }
                        if let Some((v, f1)) = field(ui, bid.with("y"), rr, "Y", r0[1], "", 1.0, -1e6, 1e6, false, a) {
                            out.push(PAct::Num("y", v, f1));
                        }
                        // A picture's flat rotation (its own control, out of the 3D rows)
                        if let Some(e) = &edit {
                            gap(ui);
                            let (r, a) = row(ui, FH);
                            let (fk, name, lo, hi) = SPIN_ROW;
                            if let Some((nv, f1)) = field(ui, bid.with("spin"), r, name, e.spin, "°", 0.5, lo, hi, false, a) {
                                out.push(PAct::Edit(fk, nv, f1));
                            }
                        }
                        if kind != Kind::Text {
                            head(ui, &mut row, "Size", false);
                            let (r, a) = row(ui, FH);
                            // W, the aspect lock, H
                            let lw = 24.0;
                            let fw = (r.width() - lw - GAP) / 2.0;
                            let l = Rect::from_min_size(r.min, vec2(fw, r.height()));
                            let lr = Rect::from_min_size(pos2(l.max.x + GAP / 2.0, r.min.y), vec2(lw, r.height()));
                            let rr = Rect::from_min_max(pos2(lr.max.x + GAP / 2.0, r.min.y), r.max);
                            let lock_id = egui::Id::new("qd_props_ratio_lock");
                            let lock: bool = ctx.data(|d| d.get_temp(lock_id)).unwrap_or(false);
                            let resp = ui.interact(lr, bid.with("lock"), Sense::click()).on_hover_text(if lock { "Unlock aspect ratio" } else { "Lock aspect ratio" });
                            let (hh, pp) = hover_fx(ctx, &resp, true);
                            let lk = ease(ctx, bid.with("lock_k"), lock);
                            let p = ui.painter();
                            p.rect_filled(lr, 6.0, hover_fill(hh, pp).gamma_multiply(a));
                            super::svg_icons::draw(p, "link", lr.center(), 15.0, mix(mix(txt_dim(), txt(), hh * 0.5), blue(), lk).gamma_multiply(a));
                            if resp.clicked() {
                                ctx.data_mut(|d| d.insert_temp(lock_id, !lock));
                            }
                            let ratio = r0[3] / r0[2].max(1e-3);
                            if let Some((v, f1)) = field(ui, bid.with("w"), l, "W", r0[2], "", 1.0, 1.0, 1e6, false, a) {
                                out.push(if lock { PAct::Size(v, (v * ratio).max(1.0), f1) } else { PAct::Num("w", v, f1) });
                            }
                            if let Some((v, f1)) = field(ui, bid.with("h"), rr, "H", r0[3], "", 1.0, 1.0, 1e6, false, a) {
                                out.push(if lock { PAct::Size((v / ratio.max(1e-3)).max(1.0), v, f1) } else { PAct::Num("h", v, f1) });
                            }
                        }
                    }

                    // Align inside the frame
                    if design && align {
                        head(ui, &mut row, "Align", first);
                        first = false;
                        let mut btns: Vec<(u8, &str)> = vec![(0, "Left"), (1, "Centre"), (2, "Right"), (3, "Top"), (4, "Middle"), (5, "Bottom")];
                        if targets.len() >= 3 {
                            btns.push((6, "Space across"));
                            btns.push((7, "Space down"));
                        }
                        let (r, a) = row(ui, 28.0);
                        let cw = r.width() / btns.len() as f32;
                        for (j, (code, tip)) in btns.into_iter().enumerate() {
                            let br = Rect::from_min_size(pos2(r.min.x + j as f32 * cw, r.min.y), vec2(cw, r.height())).shrink2(vec2(1.5, 0.0));
                            let resp = ui.interact(br, bid.with(("al", code)), Sense::click()).on_hover_text(tip);
                            let (hh, pp) = hover_fx(ctx, &resp, true);
                            let p = ui.painter();
                            p.rect_filled(br, 6.0, hover_fill(hh, pp).gamma_multiply(a));
                            let col = mix(txt_dim(), txt(), hh).gamma_multiply(a);
                            if code < 6 {
                                super::icons::align(p, br.center(), 15.0, col, code);
                            } else {
                                super::icons::distribute(p, br.center(), 15.0, col, code == 6);
                            }
                            if resp.clicked() {
                                out.push(match code {
                                    0 => PAct::Align(Align::Left),
                                    1 => PAct::Align(Align::HCenter),
                                    2 => PAct::Align(Align::Right),
                                    3 => PAct::Align(Align::Top),
                                    4 => PAct::Align(Align::VCenter),
                                    5 => PAct::Align(Align::Bottom),
                                    c => PAct::Dist(c == 6),
                                });
                            }
                        }
                    }

                    // Layer: opacity (+ corners)
                    if design {
                    head(ui, &mut row, "Layer", first);
                    first = false;
                    let (r, a) = row(ui, FH);
                    let (l, rr) = if corner.is_some() { halves(r) } else { (r, Rect::NOTHING) };
                    if let Some((v, f1)) = field(ui, bid.with("op"), l, "Opacity", opacity, "%", 0.5, 0.0, 100.0, true, a) {
                        out.push(PAct::Num("opacity", v, f1));
                    }
                    if let Some(c) = corner {
                        if let Some((v, f1)) = field(ui, bid.with("cr"), rr, "Corners", c, "", 0.25, 0.0, if kind == Kind::Image { 500.0 } else { 60.0 }, false, a) {
                            out.push(PAct::Num("corner", v, f1));
                        }
                    }
                    // Blend mode: a dropdown that floats over the panel (ui/panel_drop.rs)
                    if crate::canvas::layers::available() {
                        gap(ui);
                        let (r, a) = row(ui, FH);
                        if let Some(j) = super::panel_drop::dropdown_icons(ui, bid.with("blend_dd"), r, "Blend", &crate::canvas::layers::BLENDS, &BLEND_ICONS, blend_now as usize, a) {
                            out.push(PAct::Blend(j as u8));
                        }
                    }

                    // Fill / colour swatches
                    if let Some(col) = &fill {
                        head(ui, &mut row, if is_group { "Fill" } else { "Colour" }, first);
                        // Solid or a gradient (canvas/gradient.rs; its handles show on the canvas
                        // while a stop is picked)
                        if can_grad {
                            // type: a floating dropdown; while it's hovered, flip and a quarter
                            // turn show on its right
                            let (r, a) = row(ui, FH);
                            let mut names = vec!["Solid"];
                            names.extend(crate::canvas::gradient::KINDS);
                            let cur = grad.as_ref().map_or(0, |g| g.kind as usize + 1);
                            let extras: &[(&'static str, &str)] = if grad.is_some() { &[("swap-ends", "Swap the gradient's ends"), ("rotate-cw", "Turn it a quarter")] } else { &[] };
                            let (pick, extra) = super::panel_drop::dropdown_ex(ui, bid.with("gkind_dd"), r, "", &names, &["fill-solid", "gradient-linear", "gradient-radial", "gradient-angular", "gradient-diamond"], cur, extras, a);
                            if let Some(j) = pick {
                                out.push(PAct::GKind(j.checked_sub(1).map(|k| k as u8)));
                            }
                            match extra {
                                Some(0) => out.push(PAct::GFlip),
                                Some(_) => out.push(PAct::GRotate),
                                None => {}
                            }
                            if let Some(g) = &grad {
                                gap(ui);
                                let (r, a) = row(ui, 22.0);
                                gradient_bar(ui, bid.with("gbar"), r, g, gstop, a, &mut out);
                                gap(ui);
                                // the picked stop only (pick another on the bar): where it sits,
                                // its colour (click: colour panel), its opacity; remove shows on hover
                                let j = gstop.unwrap_or_else(|| (0..g.stops.len()).min_by(|x, y| g.stops[*x].at.total_cmp(&g.stops[*y].at)).unwrap_or(0));
                                let st = &g.stops[j];
                                let (r, a) = row(ui, FH);
                                let can = g.stops.len() > 2;
                                let row_hov = ctx.pointer_hover_pos().is_some_and(|q| r.contains(q));
                                let dk = ease(ctx, bid.with("gst_del_k"), row_hov && can);
                                let del_w = 26.0 * dk;
                                let rest = r.width() - FH - 8.0 - del_w - 4.0 * dk;
                                let pos_r = Rect::from_min_size(r.min, vec2(rest / 2.0, FH));
                                let col_r = Rect::from_min_size(pos2(pos_r.max.x + 4.0, r.min.y), vec2(FH, FH));
                                let al_r = Rect::from_min_size(pos2(col_r.max.x + 4.0, r.min.y), vec2(rest / 2.0, FH));
                                if let Some((v, f1)) = field(ui, bid.with("gst_at"), pos_r, "", (st.at * 100.0).round(), "%", 0.5, 0.0, 100.0, false, a) {
                                    out.push(PAct::GAt(j, v / 100.0, f1));
                                }
                                // small icons on the left say what each number is
                                crate::ui::svg_icons::draw(ui.painter(), "stop-add", pos2(pos_r.min.x + 15.0, pos_r.center().y), 15.0, txt_dim().gamma_multiply(a));
                                // the stop's colour as a square: click for the colour panel,
                                // double-click to type its hex in a bubble under it
                                let cpid = bid.with("gstop_color");
                                let hbid = bid.with("gstop_hex");
                                let resp = ui.interact(col_r, bid.with("gst_col"), Sense::click()).on_hover_text("Click: colour panel · double-click: type the hex");
                                let open_k = ease(ctx, bid.with("gst_col_open"), super::panel_drop::is_open(ctx, cpid) || super::panel_drop::is_open(ctx, hbid));
                                let (hh, pp) = hover_fx(ctx, &resp, true);
                                let p = ui.painter();
                                p.rect_filled(col_r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                                p.rect_filled(col_r, 6.0, hover_fill(hh.max(open_k * 0.6), pp).gamma_multiply(a));
                                color_chip(ctx, p, bid.with("gst_cc"), col_r, &st.color, hh.max(open_k), false, a);
                                match super::hex_bubble::chip_click(ctx, bid.with("gst_col"), &resp) {
                                    Some(false) => {
                                        out.push(PAct::GPick(j));
                                        super::panel_drop::toggle(ctx, cpid);
                                    }
                                    Some(true) => {
                                        out.push(PAct::GPick(j));
                                        super::hex_bubble::open(ctx, hbid);
                                    }
                                    None => {}
                                }
                                if let Some(c) = super::hex_bubble::show(ctx, hbid, col_r, &st.color) {
                                    out.push(PAct::GColor(j, c));
                                }
                                if let Some((v, f1)) = field(ui, bid.with("gst_al"), al_r, "", (st.alpha * 100.0).round(), "%", 0.5, 0.0, 100.0, true, a) {
                                    out.push(PAct::GAlpha(j, v / 100.0, f1));
                                }
                                crate::ui::svg_icons::draw(ui.painter(), "opacity", pos2(al_r.min.x + 15.0, al_r.center().y), 15.0, txt_dim().gamma_multiply(a));
                                if dk > 0.01 {
                                    let del_r = Rect::from_min_max(pos2(r.max.x - del_w, r.min.y), r.max);
                                    let resp = ui.interact(del_r, bid.with("gst_del"), Sense::click()).on_hover_text("Remove this stop");
                                    let (hh, pp) = hover_fx(ctx, &resp, true);
                                    let p = ui.painter();
                                    p.rect_filled(del_r, 6.0, hover_fill(hh, pp).gamma_multiply(a * dk));
                                    crate::ui::svg_icons::draw(p, "minus", del_r.center(), 14.0 * dk, mix(txt_dim(), hi(), hh).gamma_multiply(a * dk));
                                    if resp.clicked() {
                                        out.push(PAct::GDel(j));
                                    }
                                }
                                if let Some(c) = super::panel_drop::color_pop(ctx, cpid, r, card, &st.color) {
                                    out.push(PAct::GColor(j, c));
                                }
                                gap(ui);
                                // how softly the colours ease into each other (no hard edge where it ends)
                                let (r, a) = row(ui, FH);
                                if let Some((v, f1)) = field(ui, bid.with("gsm"), r, "Smooth", (g.smooth * 100.0).round(), "%", 0.5, 0.0, 100.0, true, a) {
                                    out.push(PAct::GSmooth(v / 100.0, f1));
                                }
                            }
                            gap(ui);
                        }
                        if grad.is_none() {
                            let (r, a) = row(ui, FH);
                            let none_ok = is_group && col != "none";
                            let row_hov = ctx.pointer_hover_pos().is_some_and(|q| r.contains(q));
                            let dk = ease(ctx, bid.with("fill_none_k"), row_hov && none_ok);
                            let del_w = 26.0 * dk;
                            let col_r = Rect::from_min_max(r.min, pos2(r.max.x - del_w - 4.0 * dk, r.max.y));
                            let cpid = bid.with("fill_color");
                            let resp = ui.interact(col_r, bid.with("fill_col"), Sense::click()).on_hover_text("Click: colour panel · double-click: type the hex");
                            let open_k = ease(ctx, bid.with("fill_col_open"), super::panel_drop::is_open(ctx, cpid));
                            let (hh, pp) = hover_fx(ctx, &resp, true);
                            let p = ui.painter();
                            p.rect_filled(col_r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                            p.rect_filled(col_r, 6.0, hover_fill(hh.max(open_k * 0.6), pp).gamma_multiply(a));
                            color_chip(ctx, p, bid.with("fill_cc"), col_r, col, hh.max(open_k), true, a);
                            let hbid = bid.with("fill_hex");
                            match super::hex_bubble::chip_click(ctx, bid.with("fill_col"), &resp) {
                                Some(false) => super::panel_drop::toggle(ctx, cpid),
                                Some(true) => super::hex_bubble::open(ctx, hbid),
                                None => {}
                            }
                            let chip_at = Rect::from_center_size(pos2(col_r.min.x + 14.0, col_r.center().y), vec2(14.0, 14.0));
                            if let Some(c) = super::hex_bubble::show(ctx, hbid, chip_at.expand(4.0), if col == "none" { "#ffffff" } else { col.as_str() }) {
                                out.push(PAct::Color(c));
                            }
                            if dk > 0.01 {
                                let del_r = Rect::from_min_max(pos2(r.max.x - del_w, r.min.y), r.max);
                                let resp = ui.interact(del_r, bid.with("fill_none"), Sense::click()).on_hover_text("No fill");
                                let (hh, pp) = hover_fx(ctx, &resp, true);
                                let p = ui.painter();
                                p.rect_filled(del_r, 6.0, hover_fill(hh, pp).gamma_multiply(a * dk));
                                crate::ui::svg_icons::draw(p, "eye-off", del_r.center(), 14.0 * dk, mix(txt_dim(), hi(), hh).gamma_multiply(a * dk));
                                if resp.clicked() {
                                    out.push(PAct::Color("none".into()));
                                }
                            }
                            let shown = if col == "none" { "#ffffff" } else { col.as_str() };
                            if let Some(c) = super::panel_drop::color_pop(ctx, cpid, r, card, shown) {
                                out.push(PAct::Color(c));
                            }
                        }
                    }

                    // Stroke
                    if let Some((key, v, lo, hi, per)) = stroke_w {
                        head(ui, &mut row, if is_group { "Stroke" } else { "Line" }, first);
                        let (r, a) = row(ui, FH);
                        if let Some((nv, f1)) = field(ui, bid.with("sw_w"), r, "Width", v, "", per, lo, hi, false, a) {
                            out.push(PAct::Num(key, nv, f1));
                        }
                        if let Some(sp) = spos {
                            gap(ui);
                            let (r, a) = row(ui, FH);
                            // Inside / centre / outside: a pill slides to the picked third
                            let opts: [(&str, u8); 3] = [("Inside", 1), ("Center", 0), ("Outside", 2)];
                            let cur = opts.iter().position(|o| o.1 == sp).unwrap_or(1) as f32;
                            let at = ctx.animate_value_with_time(bid.with("spos_k"), cur, 0.2 * anim_k());
                            let p = ui.painter();
                            p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                            let third = r.width() / 3.0;
                            p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at * third, r.min.y + 2.0), vec2(third - 4.0, r.height() - 4.0)), 5.0, wa((22.0 * a) as u8));
                            for (j, (name, val)) in opts.into_iter().enumerate() {
                                let br = Rect::from_min_size(pos2(r.min.x + j as f32 * third, r.min.y), vec2(third, r.height()));
                                let resp = ui.interact(br, bid.with(("spos", j)), Sense::click());
                                let on = val == sp;
                                let (hh, pp) = hover_fx(ctx, &resp, !on);
                                if !on {
                                    ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp).gamma_multiply(a));
                                }
                                let onk = (1.0 - (at - j as f32).abs()).clamp(0.0, 1.0);
                                ui.painter().text(br.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.0), mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(a));
                                if resp.clicked() && !on {
                                    out.push(PAct::StrokePos(val));
                                }
                            }
                        }
                    }

                    // Arrow: head style and line ends
                    if let (true, Some((tip, cap))) = (design, arrow) {
                        head(ui, &mut row, "Arrow", first);
                        let (r, a) = row(ui, 30.0);
                        let names = crate::canvas::arrow_mesh::TIPS;
                        let cw = r.width() / names.len() as f32;
                        for (j, name) in names.into_iter().enumerate() {
                            let br = Rect::from_min_size(pos2(r.min.x + j as f32 * cw, r.min.y), vec2(cw, r.height())).shrink2(vec2(1.5, 0.0));
                            let resp = ui.interact(br, bid.with(("tip", j)), Sense::click()).on_hover_text(name);
                            let on = j as u8 == tip;
                            let (hh, pp) = hover_fx(ctx, &resp, !on);
                            let onk = ease(ctx, bid.with(("tip_on", j)), on);
                            let p = ui.painter();
                            p.rect_filled(br, 6.0, hover_fill(hh, pp).gamma_multiply(a));
                            p.rect_filled(br, 6.0, wa((22.0 * onk * a) as u8));
                            // the head itself, drawn small
                            let c = br.center();
                            let (t0, t1) = (c - vec2(9.0, 0.0), c + vec2(9.0, 0.0));
                            let gl = [t0, t1, t1 + vec2(-5.5, -3.6), t1, t1 + vec2(-5.5, 3.6)];
                            let col = mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(a);
                            p.add(crate::canvas::arrow_mesh::arrow(&gl, 1.6, j as u8, cap, col, ctx.pixels_per_point()));
                            if resp.clicked() && !on {
                                out.push(PAct::Tip(j as u8));
                            }
                        }
                        gap(ui);
                        // Round / square ends: a pill slides to the picked one
                        let (r, a) = row(ui, FH);
                        let at = ctx.animate_value_with_time(bid.with("cap_k"), cap as f32, 0.2 * anim_k());
                        let p = ui.painter();
                        p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                        let half = r.width() / 2.0;
                        p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at * half, r.min.y + 2.0), vec2(half - 4.0, r.height() - 4.0)), 5.0, wa((22.0 * a) as u8));
                        for (j, name) in ["Round ends", "Square ends"].into_iter().enumerate() {
                            let br = Rect::from_min_size(pos2(r.min.x + j as f32 * half, r.min.y), vec2(half, r.height()));
                            let resp = ui.interact(br, bid.with(("cap", j)), Sense::click());
                            let on = j as u8 == cap;
                            let (hh, pp) = hover_fx(ctx, &resp, !on);
                            if !on {
                                ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp).gamma_multiply(a));
                            }
                            let onk = (1.0 - (at - j as f32).abs()).clamp(0.0, 1.0);
                            ui.painter().text(br.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.0), mix(txt_dim(), gray_ui(235), onk.max(hh * 0.5)).gamma_multiply(a));
                            if resp.clicked() && !on {
                                out.push(PAct::Cap(j as u8));
                            }
                        }
                    }

                    // Text size
                    if let Some(ts) = tsize {
                        head(ui, &mut row, "Text", first);
                        let (r, a) = row(ui, FH);
                        if let Some((v, f1)) = field(ui, bid.with("ts"), r, "Size", ts, "", 0.2, 8.0, 96.0, false, a) {
                            out.push(PAct::Num("tsize", v, f1));
                        }
                    }

                    // Picture: adjust and 3D
                    if let Some(e) = &edit {
                        for (sec, rows, per, reset) in [("Adjust", &ADJUST_ROWS[..], 0.5, 0u8), ("3D", &TILT_ROWS[..], 0.3, 2u8)] {
                            let (hr, a) = head(ui, &mut row, sec, first);
                            let dirty = if reset == 0 { e.has_color() } else { e.tilt_x != 0.0 || e.tilt_y != 0.0 || e.depth != 0.0 };
                            if dirty && link(ui, bid.with(("rst", reset)), pos2(hr.max.x, hr.center().y), "Reset", a) {
                                out.push(PAct::Reset(reset));
                            }
                            for pair in rows.chunks(2) {
                                let (r, a) = row(ui, FH);
                                let (l, rr) = if pair.len() == 2 { halves(r) } else { (r, Rect::NOTHING) };
                                for (&(fk, name, lo, hi), cell) in pair.iter().zip([l, rr]) {
                                    let mut ec = e.clone();
                                    let v = *ec.field(fk);
                                    if let Some((nv, f1)) = field(ui, bid.with(("ed", fk)), cell, name, v, "", per, lo, hi, false, a) {
                                        out.push(PAct::Edit(fk, nv, f1));
                                    }
                                }
                                gap(ui);
                            }
                            if reset == 0 && !e.cuts.is_empty() {
                                let (r, a) = row(ui, 18.0);
                                if link(ui, bid.with("restore"), pos2(r.max.x, r.center().y), "Restore cut areas", a) {
                                    out.push(PAct::Reset(1));
                                }
                            }
                        }
                    }
                    }
                    let _ = first;
                    // measured for next pass's card height (only for what's selected now)
                    if has {
                        let ch = ui.min_rect().max.y - y_top;
                        ctx.data_mut(|d| d.insert_temp(bid.with("ch"), ch));
                    }
                });
            });
        });

        // Apply
        let mut dirty = false;
        for act in out {
            match act {
                PAct::Num(key, v, first) => {
                    if first {
                        self.snapshot();
                    }
                    self.props_set(kind, &id, key, v);
                    dirty = true;
                }
                PAct::Size(w, h, first) => {
                    if first {
                        self.snapshot();
                    }
                    if let Some(r) = self.rect_of(kind, &id) {
                        self.set_rect(kind, &id, [r[0], r[1], w, h]);
                    }
                    dirty = true;
                }
                PAct::Tip(v) | PAct::Cap(v) => {
                    self.snapshot();
                    let tip = matches!(act, PAct::Tip(_));
                    let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Stroke).map(|s| s.1.clone()).chain(std::iter::once(id.clone())).collect();
                    let b = &mut self.board;
                    let strokes = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
                    for st in strokes.filter(|st| ids.contains(&st.id) && st.tool_kind() == "arrow") {
                        if tip {
                            st.tip = v;
                        } else {
                            st.cap = v;
                        }
                    }
                    dirty = true;
                }
                PAct::Align(a) => self.align_selection(a),
                PAct::Dist(h) => self.distribute_selection(h),
                PAct::Color(c) if grad.is_some() && c != "none" => {
                    self.snapshot();
                    let j = gstop.unwrap_or(0);
                    self.edit_gradient(&id, |g, _| {
                        if let Some(s) = g.as_mut().and_then(|g| g.stops.get_mut(j)) {
                            s.color = c;
                        }
                    });
                    self.grad_edit = Some((id.clone(), j));
                }
                PAct::Color(c) => acts.push(if is_group { Act::GroupColor(c) } else { Act::Color(c) }),
                PAct::StrokePos(v) => {
                    acts.push(Act::StrokePos(v));
                    dirty = true;
                }
                PAct::Edit(fk, v, first) => self.set_image_edit(fk, v, first),
                PAct::Reset(0) => self.reset_adjust(),
                PAct::Reset(1) => self.restore_cuts(),
                PAct::Reset(_) => self.reset_tilt(),
                PAct::Mirror(m) => self.path_set_mirror(m),
                PAct::Blend(m) => {
                    self.snapshot();
                    for (_, sid) in self.selected.clone() {
                        if m == 0 {
                            self.board.blend.remove(&sid);
                        } else {
                            self.board.blend.insert(sid, m);
                        }
                    }
                    dirty = true;
                }
                PAct::GKind(k) => {
                    self.snapshot();
                    self.edit_gradient(&id, |g, base| match (k, g.as_mut()) {
                        (None, _) => *g = None,
                        (Some(k), Some(g)) => g.set_kind(k),
                        (Some(k), None) => *g = Some(Gradient::new(k, base)),
                    });
                    self.grad_edit = k.map(|_| (id.clone(), 0));
                }
                PAct::GPick(j) => self.grad_edit = Some((id.clone(), j)),
                PAct::GMove(j, t) => {
                    // one undo step for the whole drag
                    let mid = egui::Id::new("qd_props").with("gbar").with("gmv");
                    if !ctx.data(|d| d.get_temp::<bool>(mid)).unwrap_or(false) {
                        self.snapshot();
                        ctx.data_mut(|d| d.insert_temp(mid, true));
                    }
                    self.edit_gradient(&id, |g, _| {
                        if let Some(s) = g.as_mut().and_then(|g| g.stops.get_mut(j)) {
                            s.at = t;
                        }
                    });
                }
                PAct::GAdd(t) => {
                    self.snapshot();
                    let mut at = 0;
                    self.edit_gradient(&id, |g, _| {
                        if let Some(g) = g.as_mut() {
                            let [r, gg, b, a] = crate::canvas::gradient::color_at(g, t);
                            let hx = format!("#{:02x}{:02x}{:02x}", (r * 255.0) as u8, (gg * 255.0) as u8, (b * 255.0) as u8);
                            g.stops.push(Stop { at: t, color: hx, alpha: a });
                            at = g.stops.len() - 1;
                        }
                    });
                    self.grad_edit = Some((id.clone(), at));
                }
                PAct::GDel(j) => {
                    self.snapshot();
                    self.edit_gradient(&id, |g, _| {
                        if let Some(g) = g.as_mut().filter(|g| g.stops.len() > 2) {
                            g.stops.remove(j);
                        }
                    });
                    self.grad_edit = Some((id.clone(), 0));
                }
                PAct::GAt(j, t, first) => {
                    if first {
                        self.snapshot();
                    }
                    self.edit_gradient(&id, |g, _| {
                        if let Some(s) = g.as_mut().and_then(|g| g.stops.get_mut(j)) {
                            s.at = t.clamp(0.0, 1.0);
                        }
                    });
                    self.grad_edit = Some((id.clone(), j));
                }
                PAct::GColor(j, c) => {
                    self.snapshot();
                    self.edit_gradient(&id, |g, _| {
                        if let Some(s) = g.as_mut().and_then(|g| g.stops.get_mut(j)) {
                            s.color = c;
                        }
                    });
                    self.grad_edit = Some((id.clone(), j));
                }
                PAct::GRotate => {
                    // a quarter turn clockwise: linear about its middle, the others about the centre
                    self.snapshot();
                    self.edit_gradient(&id, |g, _| {
                        if let Some(g) = g.as_mut() {
                            let c = if g.kind == 0 { [(g.from[0] + g.to[0]) / 2.0, (g.from[1] + g.to[1]) / 2.0] } else { g.from };
                            let turn = |q: [f32; 2]| [c[0] - (q[1] - c[1]), c[1] + (q[0] - c[0])];
                            g.from = turn(g.from);
                            g.to = turn(g.to);
                        }
                    });
                }
                PAct::GAlpha(j, v, first) => {
                    if first {
                        self.snapshot();
                    }
                    self.edit_gradient(&id, |g, _| {
                        if let Some(s) = g.as_mut().and_then(|g| g.stops.get_mut(j)) {
                            s.alpha = v.clamp(0.0, 1.0);
                        }
                    });
                    self.grad_edit = Some((id.clone(), j));
                }
                PAct::GSmooth(v, first) => {
                    if first {
                        self.snapshot();
                    }
                    self.edit_gradient(&id, |g, _| {
                        if let Some(g) = g.as_mut() {
                            g.smooth = v.clamp(0.0, 1.0);
                        }
                    });
                }
                PAct::GFlip => {
                    self.snapshot();
                    self.edit_gradient(&id, |g, _| {
                        if let Some(g) = g.as_mut() {
                            for s in &mut g.stops {
                                s.at = 1.0 - s.at;
                            }
                        }
                    });
                }
                PAct::Pt(key, v, first) => {
                    if first {
                        self.snapshot();
                    }
                    self.path_change_knot(|s, j| {
                        let k = &mut s.knots[j];
                        if key == "pr" {
                            k.r = v.max(0.0);
                            return;
                        }
                        let (dx, dy) = if key == "px" { (v - k.p.x, 0.0) } else { (0.0, v - k.p.y) };
                        for q in [&mut k.p, &mut k.i, &mut k.o] {
                            q.x += dx;
                            q.y += dy;
                        }
                    });
                }
            }
        }
        if dirty {
            self.mark_dirty();
        }
    }

    /// One number from the panel onto the selection
    fn props_set(&mut self, kind: Kind, id: &str, key: &str, v: f32) {
        let Some(r) = self.rect_of(kind, id) else { return };
        match key {
            "x" | "y" => {
                let (dx, dy) = if key == "x" { (v - r[0], 0.0) } else { (0.0, v - r[1]) };
                // Frames bring their contents along (as dragging or the arrow keys do)
                let items = self.selected_rects();
                let follow = self.frame_contents(&items);
                for (s, q) in items.iter().map(|(s, q)| (s, *q)).chain(follow.iter().map(|(s, q, _)| (s, *q))) {
                    self.set_rect(s.0, &s.1, [q[0] + dx, q[1] + dy, q[2], q[3]]);
                }
            }
            "w" => self.set_rect(kind, id, [r[0], r[1], v.max(1.0), r[3]]),
            "h" => self.set_rect(kind, id, [r[0], r[1], r[2], v.max(1.0)]),
            "opacity" => self.set_selection_opacity(v / 100.0),
            "corner" | "border" | "width" | "tsize" => {
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == kind).map(|s| s.1.clone()).collect();
                let ids = if ids.is_empty() { vec![id.to_string()] } else { ids };
                let b = &mut self.board;
                match kind {
                    Kind::Group => {
                        for g in b.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                            if key == "corner" {
                                g.corner = Some(v);
                            } else {
                                g.border = Some(v);
                            }
                        }
                    }
                    Kind::Stroke => {
                        let strokes = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
                        for s in strokes.filter(|s| ids.contains(&s.id)) {
                            if key == "corner" {
                                s.radius = Some(v);
                            } else {
                                s.width = v;
                            }
                        }
                    }
                    Kind::Text => {
                        for t in b.world_texts.iter_mut().filter(|t| ids.contains(&t.id)) {
                            t.font_size = v;
                        }
                    }
                    Kind::Image => {
                        for i in b.images.iter_mut().filter(|i| ids.contains(&i.id)) {
                            i.corner = v;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
