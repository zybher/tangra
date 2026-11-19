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
pub(super) const FH: f32 = 26.0;
pub(super) const GAP: f32 = 6.0;
/// Card padding and distance from the canvas edges
const PAD: f32 = 10.0;
/// Card width with something selected (slim, like Figma's)
const CARD_W: f32 = 228.0;
const EDGE: f32 = 12.0;
const HINT: &str = "Select something to see its properties";

pub(super) enum PAct {
    /// key, value, first change of this edit (takes the undo step)
    Num(&'static str, f32, bool),
    /// width and height together (aspect lock)
    Size(f32, f32, bool),
    Align(Align),
    Dist(bool),
    /// Fill / Stroke rows and their paint panels: colour, how solid (first change), on / off
    FillColor(String),
    StrokeColor(String),
    FillPct(f32, bool),
    StrokePct(f32, bool),
    FillOn(bool),
    StrokeOn(bool),
    /// Flip the selected shapes: horizontally (true) or vertically
    Flip(bool),
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

/// A Fill or Stroke row: its colour chip and hex (or the gradient and its type), how solid it
/// is, and a minus to take it off. Click the chip for the paint panel `pid`, double-click a
/// solid one to type its hex. True when this click opened the panel
#[allow(clippy::too_many_arguments)]
fn paint_row(ui: &mut egui::Ui, id: egui::Id, r: Rect, color: &str, grad: Option<&Gradient>, pct: f32, removable: bool, stroke: bool, pid: egui::Id, a: f32, out: &mut Vec<PAct>) -> bool {
    let ctx = ui.ctx().clone();
    let del_w = if removable { 26.0 } else { 0.0 };
    let pct_w = 64.0;
    let chip_r = Rect::from_min_max(r.min, pos2(r.max.x - pct_w - 4.0 - if removable { del_w + 4.0 } else { 0.0 }, r.max.y));
    let pct_r = Rect::from_min_size(pos2(chip_r.max.x + 4.0, r.min.y), vec2(pct_w, r.height()));
    let open = super::paint_panel::is_open(&ctx, pid);
    let open_k = ease(&ctx, id.with("open"), open);
    let chip = Rect::from_center_size(pos2(chip_r.min.x + 14.0, chip_r.center().y), vec2(14.0, 14.0));
    let mut opened = false;
    match grad {
        Some(g) => {
            let resp = ui.interact(chip_r, id.with("chip"), Sense::click());
            let (hh, pp) = hover_fx(&ctx, &resp, true);
            let p = ui.painter();
            p.rect_filled(chip_r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
            p.rect_filled(chip_r, 6.0, hover_fill(hh.max(open_k * 0.6), pp).gamma_multiply(a));
            // a small swatch of the gradient and its type
            // a fine grid whose corner points are pulled onto the stroke's 3 px corners (as the
            // colour square, color_picker::round_in), so nothing pokes past it
            let mut m = egui::Mesh::default();
            const N: u32 = 16;
            for j in 0..=N {
                for k in 0..=N {
                    let t = k as f32 / N as f32;
                    let c = crate::canvas::gradient::color32_at(g, t).gamma_multiply(a);
                    let q = pos2(chip.min.x + t * chip.width(), chip.min.y + j as f32 / N as f32 * chip.height());
                    m.colored_vertex(super::color_picker::round_in(q, chip, 3.0), c);
                }
            }
            for j in 0..N {
                for k in 0..N {
                    let q = j * (N + 1) + k;
                    m.add_triangle(q, q + 1, q + N + 1);
                    m.add_triangle(q + 1, q + N + 2, q + N + 1);
                }
            }
            p.add(egui::Shape::mesh(m));
            p.rect_stroke(chip, 3.0, EStroke::new(1.0_f32, wa((60.0 * a) as u8)));
            let name = crate::canvas::gradient::KINDS.get(g.kind as usize).copied().unwrap_or("Gradient");
            p.with_clip_rect(chip_r.shrink(1.0)).text(pos2(chip.max.x + 7.0, chip_r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.0), mix(gray_ui(200), txt(), hh.max(open_k)).gamma_multiply(a));
            if resp.clicked() {
                opened = !open;
                super::paint_panel::toggle(&ctx, pid);
            }
        }
        None => {
            let cell = super::hex_cell::show(ui, id.with("cell"), chip_r, color, open_k, 0.0, a);
            if cell.square {
                opened = !open;
                super::paint_panel::toggle(&ctx, pid);
            }
            if let Some(c) = cell.typed {
                out.push(if stroke { PAct::StrokeColor(c) } else { PAct::FillColor(c) });
            }
        }
    }
    if let Some((v, f1)) = field(ui, id.with("pct"), pct_r, "", pct, "%", 0.5, 0.0, 100.0, true, a) {
        out.push(if stroke { PAct::StrokePct(v, f1) } else { PAct::FillPct(v, f1) });
    }
    super::svg_icons::draw(ui.painter(), "opacity", pos2(pct_r.min.x + 14.0, pct_r.center().y), 14.0, txt_dim().gamma_multiply(a));
    // the minus in the last `del_w` is the section's own plus / minus (`plus_minus`)
    opened
}

/// Rows that grow in when `on` turns on and fold away when it turns off (a section's Fill /
/// Stroke rows on its plus / minus): drawn into the returned child, revealed from the top
/// down and faded on the shared `ease`, while the room under it opens / closes to match.
/// Pair with `fold_end`. None once fully folded away
pub(super) fn fold_begin(ui: &mut egui::Ui, id: egui::Id, on: bool) -> Option<(egui::Ui, f32)> {
    let ctx = ui.ctx().clone();
    let k = ease(&ctx, id.with("k"), on);
    if k <= 0.001 {
        return None;
    }
    let h: f32 = ctx.data(|d| d.get_temp(id.with("h"))).unwrap_or(0.0);
    let top = ui.cursor().min;
    let w = ui.available_width();
    let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(w, 1e5))).layout(egui::Layout::top_down(egui::Align::Min)));
    let cl = ui.clip_rect();
    let shown = if h > 0.0 { h * k } else { 1e5 };
    // a little slack: rows sit 2 px in, and strokes on their edges stay whole
    sub.set_clip_rect(Rect::from_min_max(pos2(cl.min.x, top.y - 2.0), pos2(cl.max.x, top.y + shown + 2.0)).intersect(cl));
    sub.multiply_opacity(k);
    Some((sub, k))
}

/// Ends `fold_begin`: remembers how tall the rows are and takes that much room, times how open
pub(super) fn fold_end(ui: &mut egui::Ui, id: egui::Id, sub: egui::Ui, k: f32) {
    let h = sub.min_rect().height();
    ui.ctx().data_mut(|d| d.insert_temp(id.with("h"), h));
    let w = ui.available_width();
    ui.allocate_exact_size(vec2(w, h * k), Sense::hover());
}

/// A dark box split into icon buttons (the Position section's alignment): (code, tip) each;
/// codes 0-5 are the align icons, 6 / 7 even spacing across / down. Returns the clicked code
fn btn_box(ui: &mut egui::Ui, id: egui::Id, r: Rect, btns: &[(u8, &str)], a: f32) -> Option<u8> {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    let cw = r.width() / btns.len().max(1) as f32;
    let mut out = None;
    for (j, (code, tip)) in btns.iter().enumerate() {
        let br = Rect::from_min_size(pos2(r.min.x + j as f32 * cw, r.min.y), vec2(cw, r.height())).shrink(2.0);
        let resp = ui.interact(br, id.with(j), Sense::click()).on_hover_text(*tip);
        let (hh, pp) = hover_fx(&ctx, &resp, true);
        let p = ui.painter();
        p.rect_filled(br, 5.0, hover_fill(hh, pp).gamma_multiply(a));
        let col = mix(txt_dim(), hi(), hh).gamma_multiply(a);
        if *code < 6 {
            super::icons::align(p, br.center(), 14.0, col, *code);
        } else {
            super::icons::distribute(p, br.center(), 14.0, col, *code == 6);
        }
        if resp.clicked() {
            out = Some(*code);
        }
    }
    out
}

/// The flip buttons in a dark box (dimmed when nothing selected can flip yet)
fn flip_box(ui: &mut egui::Ui, id: egui::Id, r: Rect, btns: &[(u8, &str)], en: bool, a: f32) -> Option<u8> {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    let cw = r.width() / btns.len().max(1) as f32;
    let en_k = ease(&ctx, id.with("en"), en);
    let mut out = None;
    for (j, (code, tip)) in btns.iter().enumerate() {
        let br = Rect::from_min_size(pos2(r.min.x + j as f32 * cw, r.min.y), vec2(cw, r.height())).shrink(2.0);
        let resp = ui.interact(br, id.with(j), Sense::click()).on_hover_text(*tip);
        let (hh, pp) = hover_fx(&ctx, &resp, en);
        let p = ui.painter();
        p.rect_filled(br, 5.0, hover_fill(hh, pp).gamma_multiply(a));
        let col = mix(txt_dim(), hi(), hh).gamma_multiply(a * (0.35 + 0.65 * en_k));
        super::svg_icons::draw(p, if *code == 0 { "flip-horizontal" } else { "flip-vertical" }, br.center(), 15.0, col);
        if en && resp.clicked() {
            out = Some(*code);
        }
    }
    out
}

/// A section's add / remove button: a plus on its heading while it has none, a minus at the
/// end of its row once it has one. It is one button: adding glides it from the heading down
/// to the row while the plus's upright stroke folds away into the minus (and back on removal).
/// `at`: where it rests now (None: no button, it fades out). True when clicked
fn plus_minus(ui: &mut egui::Ui, id: egui::Id, at: Option<Rect>, on: bool, tip: &str, a: f32) -> bool {
    let ctx = ui.ctx().clone();
    let last: Option<Rect> = ctx.data(|d| d.get_temp(id.with("at")));
    let tgt = at.or(last).unwrap_or(Rect::NOTHING);
    if !tgt.is_positive() {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(id.with("at"), tgt));
    let vis = ease(&ctx, id.with("vis"), at.is_some());
    if vis <= 0.001 {
        return false;
    }
    let x = super::micro::glide(&ctx, id.with("x"), tgt.center().x, false);
    let y = super::micro::glide(&ctx, id.with("y"), tgt.center().y, false);
    let r = Rect::from_center_size(pos2(x, y), tgt.size());
    let resp = ui.interact(r, id, Sense::click()).on_hover_text(tip);
    let (hh, pp) = hover_fx(&ctx, &resp, at.is_some());
    let k = ease(&ctx, id.with("minus"), on);
    let p = ui.painter();
    let a = a * vis;
    p.rect_filled(r, 6.0, hover_fill(hh, pp).gamma_multiply(a));
    let col = mix(txt_dim(), hi(), hh).gamma_multiply(a);
    let c = r.center();
    let h = 5.5;
    let st = EStroke::new(1.4_f32, col);
    p.line_segment([c - vec2(h, 0.0), c + vec2(h, 0.0)], st);
    // the upright folds into the crossbar (turning a quarter as it goes)
    let up = 1.0 - k;
    if up > 0.01 {
        let ang = k * std::f32::consts::FRAC_PI_2;
        let d = vec2(ang.sin(), ang.cos()) * h * up;
        p.line_segment([c - d, c + d], st);
    }
    at.is_some() && resp.clicked()
}

/// Figma-style number field: label left, value right. Drag sideways to scrub, double-click
/// to type (num_edit.rs, like the bars); the shown number counts to a new value.
/// `liquid`: fills with the accent up to the value on hover, like the bars' opacity.
#[allow(clippy::too_many_arguments)]
pub(super) fn field(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, val: f32, unit: &str, per_px: f32, lo: f32, hi: f32, liquid: bool, a: f32) -> Option<(f32, bool)> {
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
        // Paint: the Fill and Stroke rows (colour, how solid, on or off, can it come off)
        let mut fill_on = true;
        let mut fill_pct = 100.0;
        let mut fill_rm = false;
        let mut stroke_paint: Option<(String, f32, bool, bool)> = None;
        match kind {
            Kind::Group => {
                if let Some(g) = self.board.group_cards.iter().find(|g| g.id == id) {
                    let plain = g.color.is_empty() || g.color.eq_ignore_ascii_case("#1e293b");
                    fill_on = !g.color.eq_ignore_ascii_case("none");
                    // the default panel grey, as it's drawn
                    if plain {
                        fill = Some("#242424".into());
                    }
                    fill_pct = (g.fill_opacity.unwrap_or(1.0) * 100.0).round();
                    fill_rm = true;
                    // no border colour of its own: the shade it's drawn in (canvas/paint.rs)
                    let sc = g.stroke_color.clone().unwrap_or_else(|| {
                        let c = if fill_on && !plain {
                            let [r, gg, b] = hex_rgb(&g.color);
                            mix(Color32::from_rgb(r, gg, b), Color32::BLACK, 0.25)
                        } else {
                            let [r, gg, b, _] = wa(48).to_srgba_unmultiplied();
                            Color32::from_rgb(r, gg, b)
                        };
                        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
                    });
                    stroke_paint = Some((sc, (g.stroke_opacity.unwrap_or(1.0) * 100.0).round(), g.border.unwrap_or(1.0) > 0.0, true));
                }
            }
            Kind::Stroke => {
                if let Some(st) = self.find_stroke(&id) {
                    let closed = st.takes_fill();
                    if closed {
                        fill_on = st.fill;
                        fill_pct = (st.fill_alpha() * 100.0).round();
                        fill_rm = true;
                    } else {
                        fill = None;
                    }
                    stroke_paint = Some((st.stroke_col().to_string(), (st.stroke_alpha() * 100.0).round(), !closed || !st.no_stroke, closed));
                }
            }
            Kind::Text => fill_pct = opacity,
            _ => {}
        }
        let recent = self.board.recent_colors.clone();
        // a paint panel follows the selection only while it stays the same
        let last: Option<String> = ctx.data(|d| d.get_temp(bid.with("paint_for")));
        if last.as_deref() != Some(id.as_str()) {
            super::paint_panel::close(ctx);
            ctx.data_mut(|d| d.insert_temp(bid.with("paint_for"), id.clone()));
        }
        let n_sel = self.selected.len();
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
                // no scrollbar: the card grows to fit its rows (it only flashed in for a frame while the
                // card was still growing after a + ); a card taller than the window still scrolls by wheel
                egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).auto_shrink([false, false]).max_height(cr.height()).show(ui, |ui| {
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
                    let (r, a) = row(ui, FH);
                    let label = if n_sel > 1 { format!("{title}  ·  {n_sel} selected") } else { title.to_string() };
                    let blend_on = tab == 0 && crate::canvas::layers::available();
                    let tclip = if blend_on { Rect::from_min_max(r.min, pos2(r.max.x - 32.0, r.max.y)) } else { r };
                    ui.painter().with_clip_rect(tclip.intersect(ui.clip_rect())).text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), gray_ui(235).gamma_multiply(a));
                    // Blend mode, on the title's right: just its icon (the names are in the list)
                    if blend_on {
                        let br = Rect::from_min_max(pos2(r.max.x - FH, r.min.y), r.max);
                        if let Some(j) = super::panel_drop::dropdown_bar_ex(ui, bid.with("blend_dd"), br, &crate::canvas::layers::BLENDS, &BLEND_ICONS, blend_now as usize, "Blend mode", true, a) {
                            out.push(PAct::Blend(j as u8));
                        }
                    }
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

                    // Position: lining up in the frame, where it is, its turn and flips; then its size
                    if design && ((single && !locked) || align) {
                        head(ui, &mut row, "Position", first);
                        first = false;
                        // Alignment: two dark boxes, across (+ even spacing) and down (+ even spacing)
                        if align {
                            let (r, a) = row(ui, FH);
                            let (l, rr) = halves(r);
                            let three = targets.len() >= 3;
                            let mut across: Vec<(u8, &str)> = vec![(0, "Align left"), (1, "Align centre"), (2, "Align right")];
                            let mut down: Vec<(u8, &str)> = vec![(3, "Align top"), (4, "Align middle"), (5, "Align bottom")];
                            if three {
                                across.push((6, "Space across"));
                                down.push((7, "Space down"));
                            }
                            for (bx, btns) in [(l, across), (rr, down)] {
                                if let Some(code) = btn_box(ui, bid.with(("al_box", bx.min.x as i32)), bx, &btns, a) {
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
                        if single && !locked {
                            if align {
                                gap(ui);
                            }
                            let (r, a) = row(ui, FH);
                            let (l, rr) = halves(r);
                            if let Some((v, f1)) = field(ui, bid.with("x"), l, "X", r0[0], "", 1.0, -1e6, 1e6, false, a) {
                                out.push(PAct::Num("x", v, f1));
                            }
                            if let Some((v, f1)) = field(ui, bid.with("y"), rr, "Y", r0[1], "", 1.0, -1e6, 1e6, false, a) {
                                out.push(PAct::Num("y", v, f1));
                            }
                            // Rotation (pictures turn for now; the rest later) and the flips
                            gap(ui);
                            let (r, a) = row(ui, FH);
                            let (l, rr) = halves(r);
                            match &edit {
                                Some(e) => {
                                    let (fk, _, lo, hi) = SPIN_ROW;
                                    if let Some((nv, f1)) = field(ui, bid.with("spin"), l, "", e.spin, "°", 0.5, lo, hi, false, a) {
                                        out.push(PAct::Edit(fk, nv, f1));
                                    }
                                    super::svg_icons::draw(ui.painter(), "rotate-cw", pos2(l.min.x + 15.0, l.center().y), 14.0, txt_dim().gamma_multiply(a));
                                }
                                None => {
                                    ui.interact(l, bid.with("rot_soon"), Sense::hover()).on_hover_text("Rotation: pictures for now, shapes soon");
                                    let p = ui.painter();
                                    p.rect_filled(l, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
                                    super::svg_icons::draw(p, "rotate-cw", pos2(l.min.x + 15.0, l.center().y), 14.0, txt_dim().gamma_multiply(a * 0.45));
                                    p.text(pos2(l.max.x - 8.0, l.center().y), Align2::RIGHT_CENTER, "0°", FontId::proportional(12.0), gray_ui(225).gamma_multiply(a * 0.4));
                                }
                            }
                            let can = self.can_flip();
                            let flips: [(u8, &str); 2] = if can { [(0, "Flip horizontal"), (1, "Flip vertical")] } else { [(0, "Flip horizontal (shapes only for now)"), (1, "Flip vertical (shapes only for now)")] };
                            if let Some(c) = flip_box(ui, bid.with("flip_box"), rr, &flips, can, a) {
                                out.push(PAct::Flip(c == 0));
                            }
                        }
                        if single && !locked && kind != Kind::Text {
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

                    if design {
                    // Corners (and a picture's / note's opacity): under the size
                    let show_op = matches!(kind, Kind::Image | Kind::Note | Kind::Link);
                    if corner.is_some() || show_op {
                        if single && !locked && kind != Kind::Text {
                            gap(ui);
                        } else {
                            head(ui, &mut row, if corner.is_some() { "Corners" } else { "Opacity" }, first);
                            first = false;
                        }
                        let (r, a) = row(ui, FH);
                        let (l, rr) = if corner.is_some() && show_op { halves(r) } else { (r, r) };
                        if show_op {
                            if let Some((v, f1)) = field(ui, bid.with("op"), l, "Opacity", opacity, "%", 0.5, 0.0, 100.0, true, a) {
                                out.push(PAct::Num("opacity", v, f1));
                            }
                        }
                        if let Some(c) = corner {
                            if let Some((v, f1)) = field(ui, bid.with("cr"), rr, "Corners", c, "", 0.25, 0.0, if kind == Kind::Image { 500.0 } else { 60.0 }, false, a) {
                                out.push(PAct::Num("corner", v, f1));
                            }
                        }
                    }

                    // Fill: its colour or gradient (click: the paint panel), how solid, a minus.
                    // Its row grows in when added and folds away when removed (`fold_begin`)
                    if let Some(col) = &fill {
                        let (hr, a) = head(ui, &mut row, "Fill", first);
                        first = false;
                        let mut pm_at = (!fill_on).then(|| Rect::from_center_size(pos2(hr.max.x - 13.0, hr.center().y), vec2(26.0, 22.0)));
                        let fid = bid.with("fill_fold");
                        // while it folds away it keeps showing what it had
                        let g_now = if can_grad { grad.clone() } else { None };
                        if fill_on {
                            ctx.data_mut(|d| d.insert_temp(fid.with("last"), (col.clone(), g_now.clone())));
                        }
                        let (fcol, fgrad): (String, Option<Gradient>) = if fill_on { (col.clone(), g_now) } else { ctx.data(|d| d.get_temp(fid.with("last"))).unwrap_or((col.clone(), None)) };
                        if let Some((mut sub, k)) = fold_begin(ui, fid, fill_on) {
                            let mut sink = Vec::new();
                            let o: &mut Vec<PAct> = if fill_on { &mut out } else { &mut sink };
                            let (r, a) = row(&mut sub, FH);
                            if fill_on {
                                pm_at = fill_rm.then(|| Rect::from_min_max(pos2(r.max.x - 26.0, r.min.y), r.max));
                            }
                            let pid = bid.with("paint_fill");
                            // the canvas shows the gradient's knobs only while this panel is open
                            crate::canvas::note_grad_panel(sub.ctx(), fill_on && super::paint_panel::is_open(sub.ctx(), pid));
                            let g = fgrad.as_ref();
                            if paint_row(&mut sub, bid.with("fill_row"), r, &fcol, g, fill_pct, fill_rm, false, pid, a, o) {
                                if let Some(g) = g {
                                    let j = (0..g.stops.len()).min_by(|x, y| g.stops[*x].at.total_cmp(&g.stops[*y].at)).unwrap_or(0);
                                    o.push(PAct::GPick(gstop.unwrap_or(j)));
                                }
                            }
                            if fill_on {
                                let paint = super::paint_panel::Paint { color: &fcol, grad: g, can_grad, pick: gstop, recent: &recent, stroke: false };
                                super::paint_panel::show(ctx, pid, bid.with("gbar"), r, card, &paint, o);
                            }
                            fold_end(ui, fid, sub, k);
                        }
                        if plus_minus(ui, bid.with("fill_pm"), pm_at, fill_on, if fill_on { "Remove the fill" } else { "Add a fill" }, a) {
                            if fill_on {
                                super::paint_panel::close(ctx);
                            }
                            out.push(PAct::FillOn(!fill_on));
                        }
                    }
                    // Stroke: its colour (click: the paint panel), how solid, a minus; then where
                    // it sits on the edge and how thick. Grows in / folds away like the fill
                    if let Some((scol, spct, son, srm)) = &stroke_paint {
                        let (hr, a) = head(ui, &mut row, "Stroke", first);
                        first = false;
                        let son = *son;
                        let mut pm_at = (!son).then(|| Rect::from_center_size(pos2(hr.max.x - 13.0, hr.center().y), vec2(26.0, 22.0)));
                        let sid = bid.with("stroke_fold");
                        if let Some((mut sub, k)) = fold_begin(ui, sid, son) {
                            let mut sink = Vec::new();
                            let o: &mut Vec<PAct> = if son { &mut out } else { &mut sink };
                            let (r, a) = row(&mut sub, FH);
                            if son && *srm {
                                pm_at = Some(Rect::from_min_max(pos2(r.max.x - 26.0, r.min.y), r.max));
                            }
                            let pid = bid.with("paint_stroke");
                            paint_row(&mut sub, bid.with("stroke_row"), r, scol, None, *spct, *srm, true, pid, a, o);
                            if son {
                                let paint = super::paint_panel::Paint { color: scol, grad: None, can_grad: false, pick: None, recent: &recent, stroke: true };
                                super::paint_panel::show(ctx, pid, bid.with("gbar_s"), r, card, &paint, o);
                            }
                            if let Some((key, v, lo, hi, per)) = stroke_w {
                                gap(&mut sub);
                                let (r, a) = row(&mut sub, FH);
                                // lined up with the row above: position under the colour, weight under its %, and
                                // the spot under the minus kept free (stroke styles, later)
                                let wx = r.max.x - 64.0 - if *srm { 30.0 } else { 0.0 };
                                let wr = Rect::from_min_max(pos2(if spos.is_some() { wx } else { r.min.x }, r.min.y), pos2(wx + 64.0, r.max.y));
                                let l = if spos.is_some() { Rect::from_min_max(r.min, pos2(wx - 4.0, r.max.y)) } else { Rect::NOTHING };
                                if let Some(sp) = spos {
                                    const VALS: [u8; 3] = [1, 0, 2];
                                    let cur = VALS.iter().position(|v| *v == sp).unwrap_or(0);
                                    if let Some(j) = super::panel_drop::dropdown_bar(&mut sub, bid.with("spos_dd"), l, &["Inside", "Center", "Outside"], &["stroke-inside", "stroke-center", "stroke-outside"], cur, "Stroke position", a) {
                                        o.push(PAct::StrokePos(VALS[j]));
                                    }
                                }
                                if let Some((nv, f1)) = field(&mut sub, bid.with("sw_w"), wr, "", v, "", per, lo, hi, false, a) {
                                    o.push(PAct::Num(key, nv, f1));
                                }
                                super::svg_icons::draw(sub.painter(), "stroke-weight", pos2(wr.min.x + 15.0, wr.center().y), 15.0, txt_dim().gamma_multiply(a));
                            }
                            fold_end(ui, sid, sub, k);
                        }
                        if plus_minus(ui, bid.with("stroke_pm"), pm_at, son, if son { "Remove the stroke" } else { "Add a stroke" }, a) {
                            if son {
                                super::paint_panel::close(ctx);
                            }
                            out.push(PAct::StrokeOn(!son));
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
                PAct::Flip(h) => self.flip_selection(h),
                PAct::FillColor(c) => {
                    self.snap_once(ctx);
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_recent_pending"), c.clone()));
                    match kind {
                        Kind::Text => acts.push(Act::Color(c)),
                        _ => self.paint_edit(
                            kind,
                            &id,
                            |s| {
                                // the outline keeps the colour it had
                                if s.takes_fill() && s.stroke_color.is_none() {
                                    s.stroke_color = Some(s.color.clone());
                                }
                                s.color = c.clone();
                            },
                            |g| g.color = c.clone(),
                        ),
                    }
                    dirty = true;
                }
                PAct::StrokeColor(c) => {
                    self.snap_once(ctx);
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_recent_pending"), c.clone()));
                    self.paint_edit(
                        kind,
                        &id,
                        |s| {
                            if s.takes_fill() {
                                s.stroke_color = Some(c.clone());
                            } else {
                                s.color = c.clone();
                            }
                        },
                        |g| g.stroke_color = Some(c.clone()),
                    );
                    dirty = true;
                }
                PAct::FillPct(v, first) => {
                    if first {
                        self.snapshot();
                    }
                    let v = (v / 100.0).clamp(0.0, 1.0);
                    if kind == Kind::Text {
                        self.set_selection_opacity(v);
                    } else {
                        self.paint_edit(kind, &id, |s| s.fill_opacity = Some(v), |g| g.fill_opacity = Some(v));
                    }
                    dirty = true;
                }
                PAct::StrokePct(v, first) => {
                    if first {
                        self.snapshot();
                    }
                    let v = (v / 100.0).clamp(0.0, 1.0);
                    self.paint_edit(
                        kind,
                        &id,
                        |s| {
                            // the inside keeps how solid it was (old shapes tied it to this)
                            if s.takes_fill() && s.fill_opacity.is_none() {
                                s.fill_opacity = Some(s.fill_alpha());
                            }
                            s.opacity = Some(v);
                        },
                        |g| g.stroke_opacity = Some(v),
                    );
                    dirty = true;
                }
                PAct::FillOn(on) => {
                    self.snapshot();
                    self.paint_edit(
                        kind,
                        &id,
                        |s| {
                            if !s.takes_fill() {
                                return;
                            }
                            if on {
                                // a light grey inside (like Figma's), the outline keeping its colour
                                if s.stroke_color.is_none() && !s.no_stroke {
                                    s.stroke_color = Some(s.color.clone());
                                    s.color = "#d9d9d9".into();
                                }
                                if s.fill_opacity.is_none() {
                                    s.fill_opacity = Some(1.0);
                                }
                            }
                            s.fill = on;
                        },
                        |g| {
                            g.color = if on { "#1e293b".into() } else { "none".into() };
                            if !on {
                                g.gradient = None;
                            }
                        },
                    );
                    if !on {
                        self.grad_edit = None;
                    }
                    dirty = true;
                }
                PAct::StrokeOn(on) => {
                    self.snapshot();
                    self.paint_edit(kind, &id, |s| s.no_stroke = !on, |g| g.border = Some(if on { 1.0 } else { 0.0 }));
                    dirty = true;
                }                PAct::StrokePos(v) => {
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
                    // switching type keeps the picked stop (a new gradient starts on its first)
                    let keep = self.grad_edit.as_ref().filter(|(g, _)| g == &id).map(|(_, j)| *j);
                    self.grad_edit = k.map(|_| (id.clone(), keep.unwrap_or(0)));
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
                PAct::GAdd(t) if self.gradient_of(&id).is_some_and(|g| g.stops.len() >= super::paint_panel::MAX_STOPS) => {
                    let _ = t;
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
        // a colour lands in the document colours once it's let go (not every step of a drag)
        if !ctx.input(|i| i.pointer.any_down()) {
            ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("qd_props_snap1")));
            if let Some(c) = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new("qd_recent_pending"))) {
                let c = c.to_lowercase();
                if c.starts_with('#') {
                    let rc = &mut self.board.recent_colors;
                    rc.retain(|x| *x != c);
                    rc.insert(0, c);
                    rc.truncate(20);
                    dirty = true;
                }
            }
        }
        if dirty {
            self.mark_dirty();
        }
    }

    /// One undo step for a whole colour drag (the paint panel's picker sends every step)
    fn snap_once(&mut self, ctx: &egui::Context) {
        let k = egui::Id::new("qd_props_snap1");
        if !ctx.data(|d| d.get_temp::<bool>(k)).unwrap_or(false) {
            self.snapshot();
            ctx.data_mut(|d| d.insert_temp(k, true));
        }
    }

    /// Change the Fill / Stroke of everything selected of this kind (shapes: `fs`, frames: `fg`)
    fn paint_edit(&mut self, kind: Kind, id: &str, mut fs: impl FnMut(&mut Stroke), mut fg: impl FnMut(&mut Group)) {
        let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == kind).map(|s| s.1.clone()).chain(std::iter::once(id.to_string())).collect();
        let b = &mut self.board;
        match kind {
            Kind::Group => b.group_cards.iter_mut().filter(|g| ids.contains(&g.id)).for_each(&mut fg),
            Kind::Stroke => b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut())).filter(|s| ids.contains(&s.id)).for_each(&mut fs),
            _ => {}
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
