//! Properties: the lower part of the outline panel (split like Blender's right side, rows like
//! Figma's). Shows only what the selection can change, like the hover bars (which stay):
//! position and size, align, opacity and corners, fill, stroke, text size, and a picture's
//! adjustments and 3D. Number fields scrub sideways, or take a typed value on click. The
//! divider above it drags to share the height with the layers. With nothing selected it folds
//! away, and springs back up with the bars' jelly once something is picked.

use crate::image_edit::{ImageEdit, ADJUST_ROWS, TILT_ROWS};
use crate::*;

/// Field height
const FH: f32 = 26.0;
const GAP: f32 = 6.0;
const PH_DEFAULT: f32 = 300.0;
const PH_MIN: f32 = 120.0;

/// Height the properties take from the bottom of the outline panel (one pass late)
pub(crate) fn reserved(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp(egui::Id::new("qd_props_h"))).unwrap_or(0.0)
}

enum PAct {
    /// key, value, first change of this edit (takes the undo step)
    Num(&'static str, f32, bool),
    Align(Align),
    Dist(bool),
    Color(String),
    StrokePos(u8),
    Edit(u8, f32, bool),
    Reset(u8),
}

/// Figma-style number field: label left, value right. Drag sideways to scrub, click to type.
/// `liquid`: fills with the accent up to the value on hover, like the bars' opacity.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, val: f32, unit: &str, per_px: f32, lo: f32, hi: f32, liquid: bool, a: f32) -> Option<(f32, bool)> {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    let tid = id.with("txt");
    let typing: Option<String> = ctx.data(|d| d.get_temp(tid));
    p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    let lx = r.min.x + 8.0;
    if let Some(mut s) = typing {
        p.rect_stroke(r.shrink(0.5), 6.0, EStroke::new(1.0_f32, blue().gamma_multiply(a)));
        let lr = p.text(pos2(lx, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(11.0), TEXT_DIM.gamma_multiply(a));
        let te_id = id.with("te");
        let resp = ui.put(
            Rect::from_min_max(pos2(lr.max.x + 6.0, r.min.y), pos2(r.max.x - 6.0, r.max.y)),
            egui::TextEdit::singleline(&mut s).id(te_id).frame(false).font(FontId::proportional(12.0)).vertical_align(egui::Align::Center),
        );
        let started: bool = ctx.data(|d| d.get_temp(tid.with("st"))).unwrap_or(false);
        if !started {
            resp.request_focus();
            ctx.data_mut(|d| d.insert_temp(tid.with("st"), true));
            ctx.data_mut(|d| d.insert_temp(tid, s));
            return None;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.data_mut(|d| d.remove::<String>(tid));
            return None;
        }
        if resp.lost_focus() || !resp.has_focus() {
            ctx.data_mut(|d| d.remove::<String>(tid));
            let t = s.trim().trim_end_matches(unit).trim().replace(',', ".");
            return t.parse::<f32>().ok().map(|v| (v.clamp(lo, hi), true));
        }
        ctx.data_mut(|d| d.insert_temp(tid, s));
        return None;
    }
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
    let lc = if liquid { mix(TEXT_DIM, Color32::WHITE, hl) } else { mix(TEXT_DIM, TEXT, hl * 0.5) };
    p.text(pos2(lx, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(11.0), lc.gamma_multiply(a));
    let vc = if liquid { mix(TEXT, Color32::WHITE, hl) } else { mix(Color32::from_gray(200), TEXT, hl) };
    p.text(pos2(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, format!("{}{}", val.round() as i32, unit), FontId::proportional(12.0), vc.gamma_multiply(a));
    if resp.clicked() {
        ctx.data_mut(|d| {
            d.insert_temp(tid, format!("{}", val.round() as i32));
            d.insert_temp(tid.with("st"), false);
        });
    }
    if resp.drag_started() {
        ctx.data_mut(|d| d.insert_temp(id.with("v0"), val));
    }
    if resp.dragged() {
        let v0: f32 = ctx.data(|d| d.get_temp(id.with("v0"))).unwrap_or(val);
        let dx = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()).map_or(0.0, |(a, b)| b.x - a.x));
        let nv = (v0 + dx * per_px).round().clamp(lo, hi);
        if nv != val || resp.drag_started() {
            return Some((nv, resp.drag_started()));
        }
    }
    None
}

/// Two side-by-side halves of a row
fn halves(r: Rect) -> (Rect, Rect) {
    let m = r.center().x;
    (Rect::from_min_max(r.min, pos2(m - GAP / 2.0, r.max.y)), Rect::from_min_max(pos2(m + GAP / 2.0, r.min.y), r.max))
}

/// Small text button (section header links such as "Reset")
fn link(ui: &mut egui::Ui, id: egui::Id, right: Pos2, text: &str, a: f32) -> bool {
    let p = ui.painter().clone();
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(11.0), TEXT_DIM);
    let r = Rect::from_min_size(pos2(right.x - g.size().x, right.y - g.size().y / 2.0), g.size());
    let resp = ui.interact(r.expand(3.0), id, Sense::click());
    let (hh, _) = hover_fx(ui.ctx(), &resp, true);
    p.text(pos2(r.max.x, right.y), Align2::RIGHT_CENTER, text, FontId::proportional(11.0), mix(TEXT_DIM, blue(), hh).gamma_multiply(a));
    resp.clicked()
}

impl App {
    /// Drawn after the outline panel, inside its lower part
    pub(crate) fn props_ui(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let bid = egui::Id::new("qd_props");
        let hid = egui::Id::new("qd_props_h");
        let nr = ctx.cumulative_pass_nr();
        let shell: Option<(Rect, f32, u64)> = ctx.data(|d| d.get_temp(egui::Id::new("qd_outline_shell")));
        let Some((shell, ok)) = shell.filter(|s| s.2 + 1 >= nr && s.1 > 0.01).map(|s| (s.0, s.1)) else {
            ctx.data_mut(|d| d.insert_temp(hid, 0.0f32));
            return;
        };
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
        // Springs up with the bars' jelly; folds away on deselect
        let f = if has {
            let t0: f64 = ctx.data(|d| d.get_temp(bid.with("t0"))).unwrap_or(now);
            let x = ((now - t0) as f32 / (0.6 * k)).clamp(0.0, 1.0);
            if x < 1.0 {
                ctx.request_repaint();
            }
            1.0 - (-4.5 * x).exp() * (8.0 * x).cos()
        } else {
            let tg: f64 = ctx.data(|d| d.get_temp(bid.with("t_gone"))).unwrap_or(now);
            let e = ((now - tg) as f32 / (0.3 * k)).clamp(0.0, 1.0);
            if e < 1.0 {
                ctx.request_repaint();
            }
            1.0 - egui::emath::easing::cubic_in(e)
        };
        let shown = if has { target } else { last };
        let Some((kind, id)) = shown.filter(|s| f > 0.001 && self.rect_of(s.0, &s.1).is_some()) else {
            ctx.data_mut(|d| d.insert_temp(hid, 0.0f32));
            return;
        };
        let max_ph = (shell.height() - 230.0).max(PH_MIN);
        let ph_t = if self.settings.props_height > 0.0 { self.settings.props_height } else { PH_DEFAULT }.clamp(PH_MIN, max_ph);
        let ph = ph_t * f.max(0.0);
        ctx.data_mut(|d| d.insert_temp(hid, ph));
        let bottom = shell.max.y - 40.0;
        let top = bottom - ph;
        let t_sel: f64 = ctx.data(|d| d.get_temp(bid.with("t_sel"))).unwrap_or(now);
        let xs = ((now - t_sel) as f32 / (0.6 * k)).clamp(0.0, 1.0);
        if xs < 1.0 {
            ctx.request_repaint();
        }

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

        let mut out: Vec<PAct> = Vec::new();
        let area = Rect::from_min_max(pos2(shell.min.x, top), pos2(shell.max.x, bottom));
        // Stay above the outline (a click there brings its layer to the front)
        ctx.memory_mut(|m| m.areas_mut().move_to_top(egui::LayerId::new(egui::Order::Middle, bid)));
        egui::Area::new(bid).order(egui::Order::Middle).fixed_pos(pos2(shell.min.x, top - 4.0)).interactable(has).show(ctx, |ui| {
            ui.set_opacity(ok.clamp(0.0, 1.0));
            ui.set_clip_rect(area.expand2(vec2(0.0, 4.0)));
            // Divider: drag to share the height with the layers
            let dr = Rect::from_min_max(pos2(shell.min.x + 1.0, top - 4.0), pos2(shell.max.x - 1.0, top + 4.0));
            let dresp = ui.interact(dr, bid.with("div"), Sense::drag());
            let dk = ease(ctx, bid.with("dk"), has && (dresp.hovered() || dresp.dragged()));
            if has && (dresp.hovered() || dresp.dragged()) {
                ctx.set_cursor_icon(CursorIcon::ResizeVertical);
            }
            if has && dresp.dragged() {
                self.settings.props_height = (ph_t - dresp.drag_delta().y).clamp(PH_MIN, max_ph);
                ctx.request_repaint();
            }
            if dresp.drag_stopped() {
                save_settings(&self.settings);
            }
            let p = ui.painter().clone();
            p.rect_filled(Rect::from_min_max(pos2(shell.min.x + 1.0, top), pos2(shell.max.x - 1.0, bottom)), 0.0, frame_bg());
            p.line_segment([pos2(shell.min.x, top), pos2(shell.max.x, top)], EStroke::new(1.0_f32, border()));
            if dk > 0.01 {
                p.line_segment([pos2(shell.min.x + 14.0, top), pos2(shell.max.x - 14.0, top)], EStroke::new(2.0_f32, blue().gamma_multiply(dk)));
            }
            p.rect_filled(Rect::from_center_size(pos2(shell.center().x, top), vec2(28.0, 4.0)), 2.0, mix(border(), Color32::from_gray(200), dk));

            let cr = Rect::from_min_max(pos2(shell.min.x + 12.0, top + 8.0), pos2(shell.max.x - 12.0, bottom));
            #[allow(deprecated)]
            ui.allocate_ui_at_rect(cr, |ui| {
                ui.set_clip_rect(cr);
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(cr.height()).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let w = ui.available_width() - 4.0;
                    // Each row rises into place a beat after the one above (like the bars' buttons)
                    let mut ri = 0usize;
                    let mut row = |ui: &mut egui::Ui, h: f32| -> (Rect, f32) {
                        let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                        let gi = ((xs - 0.02 - ri as f32 * 0.035) / 0.3).clamp(0.0, 1.0);
                        ri += 1;
                        let ca = 1.0 - (1.0 - gi).powi(3);
                        (r.translate(vec2(0.0, (1.0 - ca) * 8.0)), ca)
                    };
                    let head = |ui: &mut egui::Ui, row: &mut dyn FnMut(&mut egui::Ui, f32) -> (Rect, f32), name: &str, first: bool| -> (Rect, f32) {
                        if !first {
                            let (r, a) = row(ui, 13.0);
                            ui.painter().line_segment([pos2(r.min.x, r.center().y + 2.0), pos2(r.max.x, r.center().y + 2.0)], EStroke::new(1.0_f32, border().gamma_multiply(a)));
                        }
                        let (r, a) = row(ui, 24.0);
                        ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(11.5), Color32::from_gray(225).gamma_multiply(a));
                        (r, a)
                    };
                    let gap = |ui: &mut egui::Ui| ui.add_space(GAP);

                    // Title: what's selected
                    let (r, a) = row(ui, 22.0);
                    let label = if n_sel > 1 { format!("{title}  ·  {n_sel} selected") } else { title.to_string() };
                    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), Color32::from_gray(235).gamma_multiply(a));
                    let mut first = false;

                    // Position and size (one object, not one the frame arranges)
                    if single && !locked {
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
                        if kind != Kind::Text {
                            gap(ui);
                            let (r, a) = row(ui, FH);
                            let (l, rr) = halves(r);
                            if let Some((v, f1)) = field(ui, bid.with("w"), l, "W", r0[2], "", 1.0, 1.0, 1e6, false, a) {
                                out.push(PAct::Num("w", v, f1));
                            }
                            if let Some((v, f1)) = field(ui, bid.with("h"), rr, "H", r0[3], "", 1.0, 1.0, 1e6, false, a) {
                                out.push(PAct::Num("h", v, f1));
                            }
                        }
                    }

                    // Align inside the frame
                    if align {
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
                            let col = mix(TEXT_DIM, TEXT, hh).gamma_multiply(a);
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

                    // Fill / colour swatches
                    if let Some(col) = &fill {
                        head(ui, &mut row, if is_group { "Fill" } else { "Colour" }, first);
                        let (r, a) = row(ui, 28.0);
                        let mut sw: Vec<&str> = PEN_COLORS.to_vec();
                        if is_group {
                            sw.push("none");
                        }
                        let cw = (r.width() / sw.len() as f32).min(30.0);
                        for (j, c) in sw.into_iter().enumerate() {
                            let cc = pos2(r.min.x + cw * (j as f32 + 0.5), r.center().y);
                            let br = Rect::from_center_size(cc, vec2(cw, 28.0));
                            let resp = ui.interact(br, bid.with(("sw", j)), Sense::click());
                            let hk = spring(ctx, bid.with(("swk", j)), resp.hovered(), 18.0);
                            let rad = 7.5 * (1.0 + 0.18 * hk);
                            let p = ui.painter();
                            if c == "none" {
                                p.circle_stroke(cc, rad, EStroke::new(1.0_f32, Color32::from_white_alpha((90.0 * a) as u8)));
                                p.line_segment([cc + vec2(-rad * 0.7, rad * 0.7), cc + vec2(rad * 0.7, -rad * 0.7)], EStroke::new(1.5_f32, hex("#ef4444").gamma_multiply(a)));
                            } else {
                                p.circle_filled(cc, rad, hex(c).gamma_multiply(a));
                                p.circle_stroke(cc, rad, EStroke::new(1.0_f32, Color32::from_white_alpha((50.0 * a) as u8)));
                            }
                            if col.eq_ignore_ascii_case(c) {
                                p.circle_stroke(cc, rad + 3.0, EStroke::new(1.3_f32, Color32::WHITE.gamma_multiply(a)));
                            }
                            if resp.clicked() {
                                out.push(PAct::Color(c.to_string()));
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
                            p.rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0 + at * third, r.min.y + 2.0), vec2(third - 4.0, r.height() - 4.0)), 5.0, Color32::from_white_alpha((22.0 * a) as u8));
                            for (j, (name, val)) in opts.into_iter().enumerate() {
                                let br = Rect::from_min_size(pos2(r.min.x + j as f32 * third, r.min.y), vec2(third, r.height()));
                                let resp = ui.interact(br, bid.with(("spos", j)), Sense::click());
                                let on = val == sp;
                                let (hh, pp) = hover_fx(ctx, &resp, !on);
                                if !on {
                                    ui.painter().rect_filled(br.shrink(2.0), 5.0, hover_fill(hh * 0.6, pp).gamma_multiply(a));
                                }
                                let onk = (1.0 - (at - j as f32).abs()).clamp(0.0, 1.0);
                                ui.painter().text(br.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.0), mix(TEXT_DIM, Color32::from_gray(235), onk.max(hh * 0.5)).gamma_multiply(a));
                                if resp.clicked() && !on {
                                    out.push(PAct::StrokePos(val));
                                }
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
                            let dirty = if reset == 0 { e.has_color() } else { e.has_tilt() || e.depth != 0.0 };
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
                    let _ = first;
                    ui.add_space(10.0);
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
                PAct::Align(a) => self.align_selection(a),
                PAct::Dist(h) => self.distribute_selection(h),
                PAct::Color(c) => acts.push(if is_group { Act::GroupColor(c) } else { Act::Color(c) }),
                PAct::StrokePos(v) => {
                    acts.push(Act::StrokePos(v));
                    dirty = true;
                }
                PAct::Edit(fk, v, first) => self.set_image_edit(fk, v, first),
                PAct::Reset(0) => self.reset_adjust(),
                PAct::Reset(1) => self.restore_cuts(),
                PAct::Reset(_) => self.reset_tilt(),
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
