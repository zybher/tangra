//! Draws the board: background, items, annotations, selection, overlays.

use crate::*;
use std::collections::HashMap;

impl App {
    /// Every object in stacking order; `only`: just these ids (copy as picture, canvas/capture.rs)
    pub(crate) fn paint_items(&self, p: &egui::Painter, rect: Rect, v: View, only: Option<&std::collections::HashSet<String>>) {
        let z = v.cam.zoom;
        let b = &self.board;
        let item_rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_max(v.s(x, y), v.s(x + w, y + h));

        // Inside a rounded clipping frame: the object's own shape cut to the frame's rounded shape
        let round_cut = |id: &str, r: Rect, rd: f32| -> Option<(Vec<Pos2>, bool)> {
            let rc = self.round_clips(id);
            if rc.is_empty() {
                return None;
            }
            let mut poly = super::round_clip::rounded_poly(r, rd);
            // Fully clear of every rounded corner: nothing to cut beyond the usual rect clip
            let mut safe = true;
            for (q, rad) in &rc {
                let cr = item_rect(q[0], q[1], q[2], q[3]);
                let crd = rad * z;
                safe &= cr.shrink2(vec2(crd, 0.0)).contains_rect(r.intersect(cr)) || cr.shrink2(vec2(0.0, crd)).contains_rect(r.intersect(cr));
                poly = super::round_clip::clip_convex(&poly, &super::round_clip::rounded_poly(cr, crd));
            }
            Some((poly, safe))
        };
        let hid = self.hidden_ids();
        // Objects inside a clipping frame are cropped to it
        let clips = self.clip_boxes_eased(p.ctx());
        // Faded objects (and everything inside a faded frame)
        let fades = self.fades();
        // How present each item is: entering / leaving / hidden fade, opacity glides (canvas/presence.rs)
        let pctx = p.ctx().clone();
        let pres = |id: &str| -> Option<(f32, f32)> {
            let (vk, op) = self.presence_k(&pctx, id, hid.contains(id), fades.get(id).copied().unwrap_or(1.0));
            (vk > 0.001).then_some((vk, op))
        };
        // Frames
        // The title lights with the hover outline (one source of truth, set below last frame)
        let hovered_now: Option<Sel> = p.ctx().data(|d| d.get_temp::<Option<Sel>>(egui::Id::new("qd_hover_now"))).flatten();
        // Frame strokes go on after the objects, so content never covers a frame's edge
        // (kept per frame and put on once everything inside it is painted)
        let late_strokes: std::cell::RefCell<HashMap<String, Vec<(egui::Painter, egui::Shape)>>> = Default::default();
        // Parents paint before the frames inside them, so a nested frame is never hidden
        let paint_frame = |c: &Group| {
            let Some((vk, op)) = pres(&c.id) else { return };
            // The title is not faded with the frame (it labels the frame, it isn't part of it),
            // only with it coming and going
            let mut title_p = match clips.get(&c.id) {
                Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                None => p.clone(),
            };
            title_p.multiply_opacity(vk);
            let clipped_p = {
                let mut q = title_p.clone();
                q.multiply_opacity(op);
                q
            };
            let p = &clipped_p;
            let r = item_rect(c.x, c.y, c.width, c.height);
            if !r.intersects(rect) {
                return;
            }
            // A solid panel with its title above it; the old default slate counts as "no colour"
            let panel = Color32::from_rgb(36, 36, 36);
            let custom = !c.color.is_empty() && !c.color.eq_ignore_ascii_case("#1e293b");
            let (fill_color, stroke_color, title_color) = if c.color.eq_ignore_ascii_case("none") {
                // No fill: only the outline (and title) show (the panel's grey gone clear, so it fades)
                (Color32::from_rgba_unmultiplied(36, 36, 36, 0), wa(48), FRAME_TITLE_COL)
            } else if custom {
                let [cr, cg, cb] = hex_rgb(&c.color);
                // The frame is the colour picked; the title keeps the neutral grey
                let c = rgba(cr, cg, cb, 255);
                (c, mix(c, Color32::BLACK, 0.25), FRAME_TITLE_COL)
            } else {
                (panel, wa(48), FRAME_TITLE_COL)
            };
            // Looks glide to new values (canvas/presence.rs, audit #18, #22-24)
            use super::presence::{gcol, gf};
            let fid = egui::Id::new(("qd_fg", c.id.as_str()));
            let un = |c: Color32| if c == Color32::from_rgba_unmultiplied(36, 36, 36, 0) { [36, 36, 36, 0] } else { c.to_srgba_unmultiplied() };
            // its own border colour, and the Fill % / Stroke % on top
            let stroke_color = match &c.stroke_color {
                Some(h) => {
                    let [r, g, b] = hex_rgb(h);
                    rgba(r, g, b, 255)
                }
                None => stroke_color,
            };
            let fo = c.fill_opacity.unwrap_or(1.0).clamp(0.0, 1.0);
            let so = c.stroke_opacity.unwrap_or(1.0).clamp(0.0, 1.0);
            let mut fu = un(fill_color);
            fu[3] = (fu[3] as f32 * fo) as u8;
            let mut su = stroke_color.to_srgba_unmultiplied();
            su[3] = (su[3] as f32 * so) as u8;
            let fill_color = gcol(&pctx, fid, fu);
            let stroke_color = gcol(&pctx, fid.with("s"), su);
            let fr = gf(&pctx, fid.with("r"), group_radius(c)) * z;
            let cut = round_cut(&c.id, r, fr).filter(|(_, safe)| !safe);
            match &cut {
                // a gradient fill replaces the colour (canvas/gradient.rs)
                Some((poly, _)) if poly.len() >= 3 => match &c.gradient {
                    Some(g) => super::gradient::fill(p, egui::Shape::convex_polygon(poly.clone(), Color32::WHITE, EStroke::NONE), g, r, fo),
                    None => {
                        p.add(egui::Shape::convex_polygon(poly.clone(), fill_color, EStroke::NONE));
                    }
                },
                Some(_) => {}
                None => match &c.gradient {
                    Some(g) => super::gradient::fill(p, egui::Shape::rect_filled(r, fr, Color32::WHITE), g, r, fo),
                    None => {
                        p.rect_filled(r, fr, fill_color);
                    }
                },
            }
            // Stroke is in world units: it scales with the zoom like the frame itself
            let bw = gf(&pctx, fid.with("w"), c.border.unwrap_or(1.0)) * z;
            // Stroke position: inside keeps the frame's size, outside grows past it (slides between)
            let spk = gf(&pctx, fid.with("sp"), match c.stroke_pos { 1 => -1.0, 2 => 1.0, _ => 0.0 });
            let (sr, sfr) = (r.expand(spk * bw / 2.0), (fr + spk * bw / 2.0).max(0.0));
            if bw > 0.001 {
                match round_cut(&c.id, sr, sfr).filter(|(_, safe)| !safe) {
                    Some((poly, _)) if poly.len() >= 3 => {
                        late_strokes.borrow_mut().entry(c.id.clone()).or_default().push((p.clone(), egui::Shape::closed_line(poly, EStroke::new(bw, stroke_color))));
                    }
                    Some(_) => {}
                    None => {
                        // rect_stroke paints outward from the rect: hand it the inner edge
                        late_strokes.borrow_mut().entry(c.id.clone()).or_default().push((p.clone(), egui::Shape::rect_stroke(sr.shrink(bw / 2.0), (sfr - bw / 2.0).max(0.0), EStroke::new(bw, stroke_color))));
                    }
                }
            }
            let lit = ease(p.ctx(), egui::Id::new(("qd_ftitle", c.id.as_str())), self.selected.contains(&(Kind::Group, c.id.clone())) || hovered_now.as_ref().is_some_and(|(k, id)| *k == Kind::Group && *id == c.id));
            let title_color = mix(title_color, blue(), lit);
            // Titles keep one size on screen at every zoom, cut to the frame's width
            let editing = matches!(&self.edit, Some(Edit { target: EditTarget::Group(id), .. }) if id == &c.id);
            if !editing && r.width() > 12.0 {
                let title = if c.title.is_empty() { "Frame" } else { &c.title };
                let tp = r.left_top() + vec2(1.0, -5.0 - bw / 2.0);
                // Too long for the frame: cut with an ellipsis (hidden when even "F…" won't fit)
                let maxw = r.width() - 2.0;
                let f = semibold(FRAME_TITLE_PX);
                let mut g = p.layout_no_wrap(title.to_string(), f.clone(), title_color);
                if g.size().x > maxw {
                    let mut ch: Vec<char> = title.chars().collect();
                    while ch.len() > 1 {
                        ch.pop();
                        g = p.layout_no_wrap(ch.iter().collect::<String>().trim_end().to_string() + "…", f.clone(), title_color);
                        if g.size().x <= maxw {
                            break;
                        }
                    }
                }
                if g.size().x <= maxw {
                    title_p.galley(tp - vec2(0.0, g.size().y), g, title_color);
                }
            }
        };

        // Images + their annotations
        let paint_img = |img: &ImageNode| {
            let Some((vk, op)) = pres(&img.id) else { return };
            let clipped_p = {
                let mut q = match clips.get(&img.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                q.multiply_opacity(vk * op);
                q
            };
            let p = &clipped_p;
            let r = item_rect(img.x, img.y, img.width, img.height);
            if !r.intersects(rect) {
                return;
            }
            // Rounded corners (screen px), never more than half the short side
            let rd = (super::presence::gf(&pctx, egui::Id::new(("qd_ic", img.id.as_str())), img.corner) * z).clamp(0.0, r.width().min(r.height()) / 2.0);
            // Cut to a rounded frame's corners (None: away from them, drawn as usual)
            let cut = round_cut(&img.id, r, rd).filter(|(_, safe)| !safe).map(|(poly, _)| poly);
            let tilt = img.edit.as_ref().filter(|e| e.has_tilt()).map(|e| super::tilt3d::Tilt::new(e, img.x, img.y, img.width, img.height));
            // An adjusted / cut picture shows its edited copy
            let tex = self.edited.get(&img.id).map(|t| &t.1).or(self.textures.get(&img.asset_file_name).map(|t| &t.0));
            // One picture texture drawn with a tint (the crossfade below draws two)
            let draw_tex = |tex: &egui::TextureHandle, tint: Color32| {
                if let Some(t) = &tilt {
                    // rounded corners turn with the picture (rd is screen px, the mesh wants world)
                    p.add(egui::Shape::mesh(t.mesh_rounded([img.x, img.y, img.width, img.height], rd / z.max(1e-6), 1.0 / p.ctx().pixels_per_point(), |x, y| v.s(x, y), tex.id(), tint)));
                } else if let Some(poly) = &cut {
                    p.add(egui::Shape::mesh(super::round_clip::textured_poly(poly, r, tex.id(), tint)));
                } else if rd > 0.0 {
                    let mut rs = egui::epaint::RectShape::filled(r, rd, tint);
                    rs.fill_texture_id = tex.id();
                    rs.uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    p.add(rs);
                } else {
                    p.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                }
            };
            // A new cut / restore / adjustment crossfades from the old look (canvas/tex_fade.rs)
            let ncuts = img.edit.as_ref().map_or(0, |e| e.cuts.len());
            // rubbed-out spots / lasso cuts are cut out of the picture as it draws (canvas/holes.rs)
            let holes = super::holes::screen(&img.erase, &img.erase_loops, [img.x, img.y, img.width, img.height], |q| v.s(q.x, q.y));
            let draw_pic = |tex: &egui::TextureHandle| super::holes::with_holes(p, &holes, || match super::tex_fade::fade(&img.id, tex, ncuts, self.now) {
                Some((old, w, old_under)) => {
                    p.ctx().request_repaint();
                    if old_under {
                        draw_tex(&old, Color32::WHITE);
                        draw_tex(tex, Color32::WHITE.gamma_multiply(w));
                    } else {
                        draw_tex(tex, Color32::WHITE);
                        draw_tex(&old, Color32::WHITE.gamma_multiply(1.0 - w));
                    }
                }
                None => draw_tex(tex, Color32::WHITE),
            });
            if let (Some(t), Some(tex)) = (&tilt, tex) {
                // the same shadow the flat picture casts, following its tilted (rounded) outline
                if cut.is_none() && self.parent_of(&img.id).is_none() && !img.edit.as_ref().is_some_and(|e| !e.cuts.is_empty()) && img.erase.is_empty() && img.erase_loops.is_empty() {
                    let pts: Vec<Pos2> = t.shown_outline([img.x, img.y, img.width, img.height], rd / z.max(1e-6)).into_iter().map(|(x, y)| v.s(x, y)).collect();
                    super::tilt3d::soft_shadow(p, &pts, vec2(0.0, 4.0 * z), (14.0 * z).min(40.0), 90.0 / 255.0);
                }
                draw_pic(tex);
            } else {
            // Only pictures straight on the board, and not ones with cut-out holes (it shows grey through)
            if cut.is_none() && self.parent_of(&img.id).is_none() && !img.edit.as_ref().is_some_and(|e| !e.cuts.is_empty()) && img.erase.is_empty() && img.erase_loops.is_empty() {
            p.add(
                egui::epaint::Shadow {
                    offset: vec2(0.0, 4.0 * z),
                    blur: (14.0 * z).min(40.0),
                    spread: 0.0,
                    color: Color32::from_black_alpha(90),
                }
                .as_shape(r, Rounding::same(rd)),
            );
            }
            if let Some(tex) = tex {
                draw_pic(tex);
            } else if let Some(poly) = cut.as_ref().filter(|q| q.len() >= 3) {
                p.add(egui::Shape::convex_polygon(poly.clone(), rgba(30, 41, 59, 255), EStroke::NONE));
            } else {
                p.rect_filled(r, rd, rgba(30, 41, 59, 255));
                let msg = if self.missing.contains(&img.asset_file_name) { "Image file missing" } else { "Loading…" };
                p.text(r.center(), Align2::CENTER_CENTER, msg, FontId::proportional(12.0), txt_faint());
            }
            }
            let (sx, sy) = img.scale();
            // Strokes follow the tilt; their clip is the tilted outline's box
            let clip = match &tilt {
                Some(t) => Rect::from_points(&t.corners([img.x, img.y, img.width, img.height]).map(|(x, y)| v.s(x, y))),
                None => r,
            };
            let ip = p.with_clip_rect(clip.intersect(p.clip_rect()));
            let to_s = |x: f32, y: f32| match &tilt {
                Some(t) => {
                    let (x, y) = t.fwd(x, y);
                    v.s(x, y)
                }
                None => v.s(x, y),
            };
            // each mark fades in / out on its own (erased, cleared, undone)
            let mark_p = |id: &str| {
                let mut q = ip.clone();
                q.multiply_opacity(super::presence::k(id, self.now));
                q
            };
            for s in img.annotations.strokes.iter().filter(|s| s.pixelate) {
                self.paint_pixelate(&mark_p(&s.id), img, s, v);
            }
            for s in img.annotations.strokes.iter().filter(|s| !s.pixelate) {
                paint_stroke(&mark_p(&s.id), s, |q| to_s(img.x + q.x * sx, img.y + q.y * sy), s.width * z);
            }
            for t in &img.annotations.texts {
                let ip = mark_p(&t.id);
                let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
                let editing = matches!(&self.edit, Some(Edit { target: EditTarget::ImageLabel { label, .. }, .. }) if label == &t.id);
                let picked = matches!(&self.sel_label, Some((_, l)) if l == &t.id);
                if let (false, Some(f)) = (t.text.is_empty() || editing, font(fs * sx * z)) {
                    let pos = v.s(img.x + t.x * sx, img.y + t.y * sy);
                    let galley = label_galley(&ip, &t.text, f, hex(&t.color), t.style);
                    let pad = fs * sx * z * 0.25;
                    let bg = Rect::from_min_size(pos, galley.size()).expand2(vec2(pad, pad * 0.5));
                    // bitten by the eraser / lasso (canvas/holes.rs)
                    let holes = super::holes::screen(&t.erase, &t.erase_loops, super::holes::label_box(t), |q| v.s(img.x + q.x * sx, img.y + q.y * sy));
                    super::holes::with_holes(&ip, &holes, || {
                        ip.rect_filled(bg, 3.0, Color32::from_black_alpha(180));
                    });
                    // Picked ring fades in / out
                    let pk = ease(ip.ctx(), egui::Id::new(("qd_lbl_pick", t.id.as_str())), picked);
                    if pk > 0.01 {
                        ip.rect_stroke(bg.expand(2.0), 3.0, EStroke::new(1.5_f32, blue().gamma_multiply(pk)));
                    }
                    super::holes::with_holes(&ip, &holes, || draw_label(&ip, pos, galley, hex(&t.color), t.style, fs * sx * z));
                }
            }
        };

        // Link cards
        let paint_link = |l: &LinkCard| {
            let Some((vk, op)) = pres(&l.id) else { return };
            let clipped_p = {
                let mut q = match clips.get(&l.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                q.multiply_opacity(vk * op);
                q
            };
            let p = &clipped_p;
            let r = item_rect(l.x, l.y, l.width, l.height);
            if !r.intersects(rect) {
                return;
            }
            p.rect_filled(r, 10.0 * z, Color32::from_rgb(38, 38, 38));
            p.rect_stroke(r, 10.0 * z, EStroke::new(1.0_f32, rgba(255, 255, 255, 22)));
            let cp = p.with_clip_rect(r.shrink(10.0 * z).intersect(rect));
            if let Some(f) = font(11.0 * z) {
                let domain = if l.domain.is_empty() { "LINK" } else { &l.domain };
                cp.text(v.s(l.x + 14.0, l.y + 14.0), Align2::LEFT_TOP, domain, f.clone(), rgba(56, 189, 248, 255));
                cp.text(v.s(l.x + 14.0, l.y + 62.0), Align2::LEFT_TOP, &l.url, f.clone(), txt_faint());
                if !l.notes.is_empty() {
                    cp.text(v.s(l.x + 14.0, l.y + 84.0), Align2::LEFT_TOP, &l.notes, f, rgba(148, 163, 184, 255));
                }
            }
            if let Some(f) = font(14.0 * z) {
                let title = if l.title.is_empty() { &l.url } else { &l.title };
                cp.text(v.s(l.x + 14.0, l.y + 36.0), Align2::LEFT_TOP, title, f, rgba(241, 245, 249, 255));
            }
        };

        // Sticky notes
        let paint_note = |n: &Note| {
            let Some((vk, op)) = pres(&n.id) else { return };
            let clipped_p = {
                let mut q = match clips.get(&n.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                q.multiply_opacity(vk * op);
                q
            };
            let p = &clipped_p;
            let r = item_rect(n.x, n.y, n.width, n.height);
            if !r.intersects(rect) {
                return;
            }
            let op = n.opacity.clamp(0.0, 1.0);
            let col_hex = if n.color.is_empty() || n.color == "#1e293b" { "#303030" } else { &n.color };
            let [cr, cg, cb] = hex_rgb(col_hex);
            let fill_color = Color32::from_rgba_unmultiplied(cr, cg, cb, (op * 255.0) as u8);
            let stroke_alpha = ((op * 200.0) as u8).max(35);
            let stroke_color = Color32::from_rgba_unmultiplied(255, 255, 255, stroke_alpha);

            p.rect_filled(r, 10.0 * z, fill_color);
            p.rect_stroke(r, 10.0 * z, EStroke::new(1.0_f32, stroke_color));

            let fs = if n.font_size > 0.0 { n.font_size } else { 13.5 };
            if let Some(f) = font(fs * z) {
                let cp = p.with_clip_rect(r.shrink(8.0 * z).intersect(rect));
                let (text, col) = if n.text.is_empty() {
                    ("Double-click to write…".to_string(), rgba(148, 163, 184, 140))
                } else {
                    (n.text.clone(), rgba(248, 250, 252, 255))
                };
                let _ = f;
                smooth_text(&cp, v.s(n.x + 12.0, n.y + 12.0), &text, egui::FontFamily::Proportional, fs, z, Some(n.width - 24.0), col);
            }
        };

        // Free-floating strokes and text
        let paint_stroke_item = |s: &Stroke| {
            let Some((vk, op)) = pres(&s.id) else { return };
            let clipped_p = {
                let mut q = match clips.get(&s.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                q.multiply_opacity(vk * op);
                q
            };
            let p = &clipped_p;
            paint_stroke(p, s, |q| v.s(q.x, q.y), s.width * z);
        };
        let paint_text = |t: &Label| {
            let Some((vk, op)) = pres(&t.id) else { return };
            let clipped_p = {
                let mut q = match clips.get(&t.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                q.multiply_opacity(vk * op);
                q
            };
            let p = &clipped_p;
            let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
            // colour and size glide (audit #18, #25)
            let tid = egui::Id::new(("qd_tg", t.id.as_str()));
            let fs = super::presence::gf(&pctx, tid, fs);
            let tcol = super::presence::gcol(&pctx, tid.with("c"), hex(&t.color).to_srgba_unmultiplied());
            // Hidden while it is being edited in place
            let editing = matches!(&self.edit, Some(Edit { target: EditTarget::WorldText(id), .. }) if id == &t.id);
            if editing || t.text.is_empty() {
                return;
            }
            if font(fs * z).is_some() {
                let fam = styled_font(FontId::proportional(fs), t.style).family;
                // bitten by the eraser / lasso (canvas/holes.rs)
                let holes = super::holes::screen(&t.erase, &t.erase_loops, super::holes::label_box(t), |q| v.s(q.x, q.y));
                super::holes::with_holes(p, &holes, || smooth_text(p, v.s(t.x, t.y), &t.text, fam, fs, z, None, tcol));
            } else {
                // Too small to read: a bar in the text's colour keeps it findable
                let (w, h) = text_wh(&t.text, fs);
                let at = v.s(t.x, t.y + h * 0.3);
                let bar = Rect::from_min_size(at, vec2((w * z).max(3.0), (h * z * 0.4).max(1.5)));
                p.rect_filled(bar, 1.0, tcol.gamma_multiply(0.6));
            }
        };

        // Everything in the one stacking order the outline shows (canvas/z_order.rs)
        {
            use super::z_order::Step;
            let groups: HashMap<&str, &Group> = b.group_cards.iter().map(|x| (x.id.as_str(), x)).collect();
            let images: HashMap<&str, &ImageNode> = b.images.iter().map(|x| (x.id.as_str(), x)).collect();
            let links: HashMap<&str, &LinkCard> = b.link_cards.iter().map(|x| (x.id.as_str(), x)).collect();
            let notes: HashMap<&str, &Note> = b.sticky_notes.iter().map(|x| (x.id.as_str(), x)).collect();
            let strokes: HashMap<&str, &Stroke> = b.world_strokes.iter().map(|x| (x.id.as_str(), x)).collect();
            let texts: HashMap<&str, &Label> = b.world_texts.iter().map(|x| (x.id.as_str(), x)).collect();
            // Blend modes (canvas/layers.rs): the object goes on its own layer, then mixes with
            // what's under it; a frame's layer holds everything inside it too
            let blend = |id: &str| b.blend.get(id).copied().filter(|m| *m > 0 && super::layers::available());
            for st in self.paint_steps() {
                match st {
                    Step::Item((k, id)) => {
                        if only.is_some_and(|o| !o.contains(&id)) {
                            continue;
                        }
                        let id = id.as_str();
                        let bl = blend(id);
                        if bl.is_some() {
                            super::layers::begin_blend(p);
                        }
                        match k {
                            Kind::Group => groups.get(id).map(|x| paint_frame(x)),
                            Kind::Image => images.get(id).map(|x| paint_img(x)),
                            Kind::Link => links.get(id).map(|x| paint_link(x)),
                            Kind::Note => notes.get(id).map(|x| paint_note(x)),
                            Kind::Stroke => strokes.get(id).map(|x| paint_stroke_item(x)),
                            Kind::Text => texts.get(id).map(|x| paint_text(x)),
                        };
                        // a frame's layer ends with its contents (Step::End)
                        if let (Some(m), false) = (bl, k == Kind::Group) {
                            super::layers::end_blend(p, m);
                        }
                    }
                    // A frame's outline goes over its own contents, under whatever is in front of it
                    Step::End(id) => {
                        if only.is_some_and(|o| !o.contains(&id)) {
                            continue;
                        }
                        let ls = late_strokes.borrow_mut().remove(&id);
                        for (lp, s) in ls.into_iter().flatten() {
                            lp.add(s);
                        }
                        if let Some(m) = blend(&id) {
                            super::layers::end_blend(p, m);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn paint(&self, p: &egui::Painter, rect: Rect, v: View, hover: Option<Pos2>) {
        let r_top = Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 };
        p.rect_filled(rect, r_top, bg());
        // The gradient fades in and out (audit #92)
        let gk = ease(p.ctx(), egui::Id::new("qd_grad_k"), gradient_on());
        if gk > 0.001 {
            fill_rounded_top(p, rect, 10.0, |q| gradient_at(q, rect).gamma_multiply(gk));
        }

        if aurora() {
            p.ctx().data_mut(|d| d.insert_temp(egui::Id::new("qd_canvas_rect"), rect));
            paint_aura(&p.with_clip_rect(rect), rect, self.now);
        }
        if aurora() || gradient_on() {
            if let Some(n) = &self.noise {
                // Tiled 1:1 in screen pixels; adds 0-3 levels of brightness at random
                let ppp = p.ctx().pixels_per_point();
                let tile = 128.0 / ppp;
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(rect.width() / tile, rect.height() / tile));
                p.image(n.id(), rect, uv, Color32::WHITE);
            }
        }

        // Dot grid (canvas/grid_fade.rs: zoom levels and pattern changes crossfade)
        super::grid_fade::paint_grid(p, rect, v, self.settings.pattern, self.settings.grid_dots, self.now);

        // Every object, back to front (also drawn on its own for copy as picture)
        self.paint_items(p, rect, v, None);
        let item_rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_max(v.s(x, y), v.s(x + w, y + h));
        let z = v.cam.zoom;
        let b = &self.board;

        // Stroke being drawn right now
        if let Some((target, s)) = &self.active {
            match target.as_ref().and_then(|id| b.images.iter().find(|i| &i.id == id)) {
                Some(img) if s.pixelate => {
                    let r = Rect::from_min_max(v.s(img.x, img.y), v.s(img.x + img.width, img.y + img.height));
                    self.paint_pixelate(&p.with_clip_rect(r.intersect(p.clip_rect())), img, s, v);
                }
                Some(img) => {
                    let (sx, sy) = img.scale();
                    paint_stroke(p, s, |q| v.s(img.x + q.x * sx, img.y + q.y * sy), s.width * z);
                }
                None => paint_stroke(p, s, |q| v.s(q.x, q.y), s.width * z),
            }
            // While it's being drawn a shape shows the selection outline, an arrow a handle at
            // each end (where it starts and where it will point); a pen line stays plain
            let img = target.as_ref().and_then(|id| b.images.iter().find(|i| &i.id == id));
            let map = |q: &Pt| match img {
                Some(i) => {
                    let (sx, sy) = i.scale();
                    v.s(i.x + q.x * sx, i.y + q.y * sy)
                }
                None => v.s(q.x, q.y),
            };
            let a = ease(p.ctx(), egui::Id::new("qd_draw_hl"), true);
            let kind = s.tool_kind();
            if kind == "arrow" {
                for q in [&s.points[0], &s.points[1]] {
                    let c = map(q);
                    p.circle_filled(c, 5.0 * a, blue());
                    p.circle_stroke(c, 5.0 * a, EStroke::new(1.5_f32 * a, Color32::WHITE));
                }
            } else if kind == "rectangle" || kind == "ellipse" {
                let pts: Vec<Pos2> = s.points.iter().map(map).collect();
                let r = Rect::from_points(&pts).shrink(0.5);
                let round = super::geometry::outline_round(s).map_or(2.0, |r| r * z);
                p.rect_stroke(r, round, EStroke::new(2.0_f32, blue().gamma_multiply(a)));
            }
        } else {
            // so the next drawing fades its highlight in again
            ease(p.ctx(), egui::Id::new("qd_draw_hl"), false);
        }

        // Hover outline (Select tool, idle): thin, no handles; fades between objects
        {
            let ctx = p.ctx().clone();
            let idle = self.tool == Tool::Select && matches!(self.drag, Drag::None) && self.edit.is_none() && !ctx.input(|i| i.pointer.any_down() || i.key_down(Key::Space));
            let hov = ctx
                .pointer_hover_pos()
                .filter(|q| p.clip_rect().contains(*q) && idle && !ctx.is_pointer_over_area() && !ui_blocks(&ctx, *q))
                .and_then(|q| self.hit_test(v.w(q)).map(|h| self.pick(h, false)))
                .or_else(|| crate::ui::outline::outline_hover(&ctx))
                .filter(|h| !self.selected.contains(h));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_hover_now"), hov.clone()));
            let hid = egui::Id::new("qd_hover_outline");
            let mut last: Option<Sel> = ctx.data(|d| d.get_temp::<Option<Sel>>(hid)).flatten();
            if hov.is_some() {
                last = hov.clone();
            }
            ctx.data_mut(|d| d.insert_temp(hid, last.clone()));
            let a = ctx.animate_bool_with_time(hid.with("a"), hov.is_some(), 0.12 * anim_k());
            if let (Some((k, id)), true) = (last, a > 0.01) {
                // A drawing lights up along its own line (Figma), not as a box
                let free = if k == Kind::Stroke { self.find_stroke(&id).filter(|s| super::geometry::is_freehand(s)).cloned() } else { None };
                if let Some(s) = free {
                    let pts: Vec<Pos2> = s.points.iter().map(|q| v.s(q.x, q.y)).collect();
                    let line = super::stroke_mesh::smooth_line(&pts, s.width * z);
                    p.add(super::stroke_mesh::stroke_mesh(&line, 1.5, blue().gamma_multiply(a)));
                } else if let Some([x, y, w, h]) = self.rect_of(k, &id) {
                    let r = item_rect(x, y, w, h).shrink(0.5);
                    let round = match k {
                        Kind::Image => self.board.images.iter().find(|i| i.id == id).map_or(0.0, |i| i.corner * z),
                        Kind::Group => self.board.group_cards.iter().find(|g| g.id == id).map_or(0.0, |g| group_radius(g) * z),
                        Kind::Stroke => self.find_stroke(&id).and_then(super::geometry::outline_round).map_or(2.0, |r| r * z),
                        Kind::Text => 2.0,
                        _ => 10.0 * z,
                    };
                    p.rect_stroke(r, round, EStroke::new(2.0_f32, blue().gamma_multiply(a)));
                }
            }
        }

        // Canvas drag: the frame the cursor is in lights up ("let go to drop it here")
        // (fades in and out, and keeps the last frame while it fades)
        {
            let ctx = p.ctx().clone();
            let did = egui::Id::new("qd_drop_glow");
            let now = self.drop_target();
            if let Some(t) = &now {
                ctx.data_mut(|d| d.insert_temp(did, t.clone()));
            }
            let a = ctx.animate_bool_with_time(did.with("a"), now.is_some(), 0.12 * anim_k());
            let last: Option<String> = ctx.data(|d| d.get_temp(did));
            if let Some(g) = last.filter(|_| a > 0.01).and_then(|t| self.board.group_cards.iter().find(|g| g.id == t)) {
                let r = item_rect(g.x, g.y, g.width, g.height).expand(1.0);
                p.rect_stroke(r, group_radius(g) * z, EStroke::new(2.0_f32, blue().gamma_multiply(a)));
            }
        }

        // Selection outlines
        for (k, id) in &self.selected {
            if self.path_edit.as_ref().is_some_and(|e| &e.id == id) {
                continue;
            }
            // while the gradient's knobs show on it, its outline, corner dot and size pills fade away
            let gk = 1.0 - self.grad_knobs_k(&p.ctx().clone(), id);
            if gk < 0.01 {
                continue;
            }
            let mut gp = p.clone();
            gp.multiply_opacity(gk);
            let p = &gp;
            if let Some([x, y, w, h]) = self.rect_of(*k, id) {
                // Centred half a pixel in: covers the object's own edge, so no dark sliver shows
                let r = item_rect(x, y, w, h).shrink(0.5);
                let round = match k {
                    Kind::Image => self.board.images.iter().find(|i| &i.id == id).map_or(0.0, |i| i.corner * z),
                    Kind::Group => self.board.group_cards.iter().find(|g| &g.id == id).map_or(0.0, |g| group_radius(g) * z),
                    Kind::Stroke => self.find_stroke(id).and_then(super::geometry::outline_round).map_or(2.0, |r| r * z),
                    Kind::Text => 2.0,
                    _ => 10.0 * z,
                };
                p.rect_stroke(r, round, EStroke::new(2.0_f32, blue()));
                if self.selected.len() == 1 && *k != Kind::Text {
                    let ctx = p.ctx().clone();
                    let corners = [r.right_bottom(), r.left_bottom(), r.right_top(), r.left_top()];
                    let hid = egui::Id::new("qd_handle_corner");
                    let mut ci = ctx.data(|d| d.get_temp::<usize>(hid)).unwrap_or(0);
                    if let (Drag::Resize { left, top, .. }, _) = (&self.drag, ()) {
                        ci = (*left as usize) + 2 * (*top as usize);
                    } else if let Some(q) = ctx.pointer_hover_pos().filter(|q| r.expand(60.0).contains(*q) && !ctx.is_pointer_over_area() && !ui_blocks(&ctx, *q)) {
                        // Only switch corners once the pointer is clearly nearer another one
                        let d = |i: usize| corners[i].distance(q);
                        let best = (0..4).min_by(|a, b| d(*a).total_cmp(&d(*b))).unwrap_or(0);
                        if d(best) < d(ci) - 4.0 {
                            ci = best;
                        }
                        if d(ci) < 12.0 && matches!(self.drag, Drag::None) {
                            ctx.set_cursor_icon(if ci == 0 || ci == 3 { CursorIcon::ResizeNwSe } else { CursorIcon::ResizeNeSw });
                        }
                    }
                    ctx.data_mut(|d| d.insert_temp(hid, ci));
                    // Edge hover: one-way resize cursor
                    if let (Some(q), Drag::None) = (ctx.pointer_hover_pos().filter(|q| !ui_blocks(&ctx, *q)), &self.drag) {
                        let near = |a: f32, b: f32| (a - b).abs() < 8.0;
                        let (in_x, in_y) = (q.x > r.min.x && q.x < r.max.x, q.y > r.min.y && q.y < r.max.y);
                        if corners[ci].distance(q) >= 12.0 {
                            if in_y && (near(q.x, r.min.x) || near(q.x, r.max.x)) {
                                ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                            } else if in_x && (near(q.y, r.min.y) || near(q.y, r.max.y)) {
                                ctx.set_cursor_icon(CursorIcon::ResizeVertical);
                            }
                        }
                    }
                    let sid = egui::Id::new(("qd_handle_show", id.as_str()));
                    let (mut shown, mut sc) = ctx.data(|d| d.get_temp::<(usize, f32)>(sid)).unwrap_or((ci, 1.0));
                    let dt = ctx.input(|i| i.stable_dt).min(0.05);
                    if shown != ci {
                        sc -= dt / 0.09;
                        if sc <= 0.0 {
                            sc = 0.0;
                            shown = ci;
                        }
                    } else {
                        sc = (sc + dt / 0.14).min(1.0);
                    }
                    if sc < 1.0 {
                        ctx.request_repaint();
                    }
                    ctx.data_mut(|d| d.insert_temp(sid, (shown, sc)));
                    // overshoot a touch on the way back in
                    let e = if shown == ci { 1.0 + 0.25 * (sc * std::f32::consts::PI).sin() * (1.0 - sc).min(0.5) * 2.0 } else { 1.0 };
                    let rad = 5.0 * sc * e;
                    if rad > 0.3 {
                        p.circle_filled(corners[shown], rad, blue());
                        p.circle_stroke(corners[shown], rad, EStroke::new(1.5_f32 * sc.min(1.0), Color32::WHITE));
                    }
                    self.size_pills(&ctx, p, rect, r, id, w, h);
                }
            }
        }

        // Multi-selection: one bounding box with the same size pills as a single object; it fades in
        // (multi_edit.rs scales its corner dots in with the same ease)
        let multi_a = p.ctx().animate_bool_with_time(egui::Id::new("qd_multi_a"), self.selected.len() > 1, 0.12 * anim_k());
        if self.selected.len() > 1 {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (k, id) in &self.selected {
                if let Some([x, y, w, h]) = self.rect_of(*k, id) {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
                }
            }
            if x1 > x0 {
                let r = item_rect(x0, y0, x1 - x0, y1 - y0).expand(0.5);
                p.rect_stroke(r, 0.0, EStroke::new(1.5_f32, blue().gamma_multiply(multi_a)));
                // Keyed by the selection itself, so adding/removing an object glides the pills too
                let key = {
                    use std::hash::{Hash, Hasher};
                    let mut ids: Vec<&String> = self.selected.iter().map(|(_, i)| i).collect();
                    ids.sort();
                    let mut hs = std::collections::hash_map::DefaultHasher::new();
                    ids.hash(&mut hs);
                    format!("qd_multi{}", hs.finish())
                };
                self.size_pills(&p.ctx().clone(), &p, rect, r, &key, x1 - x0, y1 - y0);
            }
        }

        self.paint_path_edit(p, v);
        self.paint_gradient_handles(p, v);
        self.paint_pen(p, v, hover);
        self.paint_snap(p, v, hover);
        self.paint_gap_handles(p, v, hover);
        self.paint_multi_handles(p, v, hover);

        // Marquee selection box
        // (fades out where it was let go instead of vanishing)
        {
            let ctx = p.ctx().clone();
            let mid = egui::Id::new("qd_marquee");
            let on = if let Drag::BoxSelect { start, current, .. } = self.drag {
                ctx.data_mut(|d| d.insert_temp(mid, [start.x, start.y, current.x, current.y]));
                true
            } else {
                false
            };
            // Appears at once (it follows the drag), only the release fades
            let a = if on {
                ctx.animate_bool_with_time(mid.with("a"), true, 0.0);
                1.0
            } else {
                ctx.animate_bool_with_time(mid.with("a"), false, 0.12 * anim_k())
            };
            if let Some(m) = ctx.data(|d| d.get_temp::<[f32; 4]>(mid)).filter(|_| a > 0.01) {
                let marquee = Rect::from_two_pos(v.s(m[0], m[1]), v.s(m[2], m[3]));
                p.rect_filled(marquee, 0.0, blue().gamma_multiply(0.18 * a));
                p.rect_stroke(marquee, 0.0, EStroke::new(1.0_f32, blue().gamma_multiply(a)));
            }
        }

        self.paint_lasso(p, v);

        // Empty-board hint
        // Fades out with the first object and back in when the board is cleared
        let ea = ease(p.ctx(), egui::Id::new("qd_empty_hint"), b.is_empty() && self.edit.is_none());
        if ea > 0.01 {
            p.text(
                rect.center() - vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                "Right-click anywhere",
                FontId::proportional(18.0),
                rgba(148, 163, 184, 150).gamma_multiply(ea),
            );
            p.text(
                rect.center() + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                "Ctrl+V to paste  ·  scroll to pan  ·  Ctrl+scroll to zoom",
                FontId::proportional(12.5),
                rgba(100, 116, 139, 150).gamma_multiply(ea),
            );
        }

        // Active tool pill (top-center)
        let tool = match self.tool {
            Tool::Select => None,
            Tool::Pen => Some("Pen  ·  click points, drag to curve, Enter to finish"),
            Tool::Pencil => Some("Pencil"),
            Tool::Highlighter => Some("Highlighter"),
            Tool::Pixelate => Some("Pixelate  ·  swipe over an image"),
            Tool::Text => Some("Text  ·  click to place"),
            Tool::Eraser => Some("Eraser  ·  drag over strokes"),
            Tool::Rect => Some("Rectangle  ·  drag to draw"),
            Tool::Ellipse => Some("Ellipse  ·  drag to draw"),
            Tool::Arrow => Some("Arrow  ·  drag to draw"),
            Tool::Frame => Some("Frame  ·  click to place, drag to size"),
            Tool::Lasso => Some("Lasso  ·  loop part of a picture, then Delete"),
        };
        // Indicator pill retired: the shortcut list and toolbar already show the tool
        let tool: Option<&str> = tool.filter(|_| false);
        // One pill at a time: a new tool's pill grows in place, the previous one
        // shrinks, rises off the top edge and fades, so they never stack
        {
            let ctx = p.ctx().clone();
            let pid = egui::Id::new("qd_tool_pill2");
            type Pill = (String, bool, Color32);
            let cur_now: Option<Pill> = tool.map(|n| (n.to_string(), self.tool != Tool::Eraser, hex(&self.pen_color)));
            let (mut cur, mut cur_t0, mut old, mut old_t0) =
                ctx.data(|d| d.get_temp::<(Option<Pill>, f64, Option<Pill>, f64)>(pid)).unwrap_or((None, 0.0, None, 0.0));
            if cur.as_ref().map(|c| &c.0) != cur_now.as_ref().map(|c| &c.0) {
                if cur.is_some() {
                    old = cur.take();
                    old_t0 = self.now;
                }
                cur = cur_now.clone();
                cur_t0 = self.now;
            } else if let (Some(c), Some(n)) = (cur.as_mut(), cur_now.as_ref()) {
                c.2 = n.2;
            }
            ctx.data_mut(|d| d.insert_temp(pid, (cur.clone(), cur_t0, old.clone(), old_t0)));
            // Drawn at full size on its own layer and scaled by the layer transform,
            // so the text is never re-rasterised mid-animation (no stepping)
            let draw = |which: &str, pill: &Pill, scale: f32, sx: f32, sy: f32, dy: f32, a: f32| {
                let (name, dot, dot_col) = pill;
                let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new(("qd_pill_layer", which)));
                let lp = ctx.layer_painter(layer).with_clip_rect(rect);
                let galley = lp.layout_no_wrap(format!("{name}   ·   Esc to finish"), FontId::proportional(12.0), txt().gamma_multiply(a));
                let w = galley.size().x + if *dot { 44.0 } else { 28.0 };
                let c = pos2(rect.center().x, rect.min.y + 26.0);
                let r = Rect::from_center_size(c, vec2(w * sx, 28.0 * sy));
                glass(&lp, r, 14.0, a);
                lp.rect_stroke(r, 14.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
                let mut tx = r.min.x + 14.0;
                if *dot {
                    lp.circle_filled(pos2(tx + 4.0, r.center().y), 4.5, dot_col.gamma_multiply(a));
                    tx += 16.0;
                }
                lp.galley(pos2(tx, r.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
                let t = egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale) + vec2(0.0, dy), scale);
                ctx.set_transform_layer(layer, t);
            };
            let smooth = |x: f32| {
                let x = x.clamp(0.0, 1.0);
                x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
            };
            let mut busy = false;
            if let Some(o) = &old {
                let e = ((self.now - old_t0) / (0.34 * anim_k().max(1e-3) as f64)) as f32;
                if e < 1.0 {
                    busy = true;
                    let e = e.max(0.0);
                    let up = smooth(((e - 0.15) / 0.85).max(0.0));
                    let sq = (e / 0.15).min(1.0) * (1.0 - up);
                    let st = (up * std::f32::consts::PI).sin();
                    draw("old", o, 1.0 - 0.35 * up, 1.0 + 0.08 * sq - 0.12 * st, 1.0 - 0.1 * sq + 0.22 * st, 3.0 * sq - up * up * 46.0, (1.0 - up).powf(1.2));
                }
            }
            if let Some(c) = &cur {
                let x = (((self.now - cur_t0 - if old.is_some() { 0.08 } else { 0.0 }) / (0.55 * anim_k().max(1e-3) as f64)) as f32).clamp(0.0, 1.0);
                busy |= x < 1.0;
                if x > 0.0 {
                    let f = 1.0 - (-6.5 * x).exp() * (11.0 * x).cos();
                    let w = (-5.5 * x).exp() * (16.0 * x).sin() * 0.16;
                    draw("cur", c, 0.55 + 0.45 * f, 1.0 + w, 1.0 - w, (1.0 - f) * 16.0, (x / 0.18).min(1.0));
                }
            }
            if busy {
                ctx.request_repaint();
            }
        }

        // Hotkey list, bottom-left above the status bar; the active tool is lit
        {
            let keys: [(&str, &str, Option<Tool>); 13] = [
                ("V", "Select", Some(Tool::Select)),
                ("F", "Frame", Some(Tool::Frame)),
                ("R", "Rectangle", Some(Tool::Rect)),
                ("O", "Ellipse", Some(Tool::Ellipse)),
                ("A", "Arrow", Some(Tool::Arrow)),
                ("P", "Pen", Some(Tool::Pen)),
                ("⇧P", "Pencil", Some(Tool::Pencil)),
                ("H", "Highlighter", Some(Tool::Highlighter)),
                ("B", "Blur", Some(Tool::Pixelate)),
                ("T", "Text", Some(Tool::Text)),
                ("E", "Eraser", Some(Tool::Eraser)),
                ("K", "Lasso cut", Some(Tool::Lasso)),
                ("F3", "Stats", None),
            ];
            let ctx = p.ctx().clone();
            let rect = rect.translate(vec2(crate::ui::outline::hud_shift(&ctx), 0.0));
            let space = ctx.input(|i| i.key_down(Key::Space));
            let top = rect.max.y - 56.0 - keys.len() as f32 * 16.0;
            // A soft pill glides to the active tool, like the menus sliding between spots
            if let Some(k) = keys.iter().position(|k| k.2 == Some(self.tool)) {
                let gid = egui::Id::new("qd_keys_glide");
                let target = top + k as f32 * 16.0;
                let cur: f32 = ctx.data(|d| d.get_temp(gid)).unwrap_or(target);
                let dt = ctx.input(|i| i.stable_dt).min(0.05);
                let v = cur + (target - cur) * (1.0 - (-dt * 16.0).exp());
                let v = if (v - target).abs() < 0.2 { target } else { ctx.request_repaint(); v };
                ctx.data_mut(|d| d.insert_temp(gid, v));
                let pill = Rect::from_min_size(pos2(rect.min.x + 9.0, v), vec2(100.0, 16.0));
                p.rect_filled(pill, 5.0, blue().gamma_multiply(0.14));
            }
            let mut y = top;
            for (i, (key, name, tool)) in keys.into_iter().enumerate() {
                let lit = match (tool, name) {
                    (Some(t), _) => t == self.tool,
                    (None, "Stats") => self.stats_on,
                    (None, "Pan (hold)") => space,
                    _ => false,
                };
                // Colours ease on/off instead of switching instantly
                let on = ease(&ctx, egui::Id::new(("qd_keys_on", i)), lit);
                let (kc, nc) = (mix(txt_dim(), blue(), on), mix(txt_faint(), txt(), on));
                p.text(pos2(rect.min.x + 16.0 + 2.0 * on, y + 8.0), Align2::LEFT_CENTER, key, FontId::proportional(11.0), kc);
                p.text(pos2(rect.min.x + 40.0 + 2.0 * on, y + 8.0), Align2::LEFT_CENTER, name, FontId::proportional(11.5), nc);
                y += 16.0;
            }
        }

        // F3: performance read-out, top-left, in the same quiet style as the hotkey list
        let st = ease(p.ctx(), egui::Id::new("qd_stats_in"), self.stats_on);
        if st > 0.001 {
            let rect = rect.translate(vec2(crate::ui::outline::top_shift(p.ctx()), 0.0));
            let n = self.frames.len();
            let worst = self.frames.iter().zip(self.frames.iter().skip(1)).map(|(a, b)| b - a).fold(0.0_f64, f64::max);
            let idle = n < 5;
            let tex: usize = self.textures.values().map(|t| t.0.size()[0] * t.0.size()[1] * 4).sum();
            let rows: [(&str, String, bool); 8] = [
                ("State", if idle { "Idle".into() } else { "Drawing".into() }, !idle),
                ("FPS", if idle { "-".into() } else { format!("{n}") }, false),
                ("Slowest", if idle { "-".into() } else { format!("{:.0} ms", worst * 1000.0) }, false),
                ("Work", format!("{:.1} ms", self.ui_ms), false),
                ("CPU", format!("{:.1}%", self.cpu_pct), false),
                ("RAM", format!("{:.0} MB", self.mem_mb), false),
                ("Images", format!("{}", self.textures.len()), false),
                ("GPU", format!("~{:.0} MB", tex as f32 * 1.33 / 1_048_576.0), false),
            ];
            let mut y = rect.min.y + 16.0;
            for (i, (label, value, lit)) in rows.into_iter().enumerate() {
                let a = egui::emath::easing::cubic_out((st * 1.7 - i as f32 * 0.09).clamp(0.0, 1.0));
                let dx = -(1.0 - a) * 14.0;
                p.text(pos2(rect.min.x + 16.0 + dx, y + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), txt_faint().gamma_multiply(a));
                p.text(pos2(rect.min.x + 72.0 + dx, y + 8.0), Align2::LEFT_CENTER, value, FontId::proportional(11.5), if lit { blue() } else { txt_dim() }.gamma_multiply(a));
                y += 16.0;
            }
        }

        // Toasts (bottom-center), one at a time like the tool pill: the new one grows in
        // place, the old one shrinks, rises and fades
        {
            let ctx = p.ctx().clone();
            let draw = |which: &str, msg: &str, scale: f32, sx: f32, sy: f32, dy: f32, a: f32| {
                let msg = &clip_name(msg, 40);
                let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new(("qd_toast_layer", which)));
                let lp = ctx.layer_painter(layer).with_clip_rect(rect);
                let galley = lp.layout_no_wrap(msg.to_string(), FontId::proportional(12.5), txt().gamma_multiply(a));
                let size = galley.size() + vec2(28.0, 14.0);
                let c = pos2(rect.center().x, rect.max.y - 84.0);
                let r = Rect::from_center_size(c, size * vec2(sx, sy));
                glass(&lp, r, 10.0, a);
                lp.rect_stroke(r, 10.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
                lp.galley(r.min + vec2(14.0, 7.0), galley, Color32::WHITE);
                ctx.set_transform_layer(layer, egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale) + vec2(0.0, dy), scale));
            };
            let smooth = |x: f32| {
                let x = x.clamp(0.0, 1.0);
                x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
            };
            let n = self.toasts.len();
            let k = anim_k().max(1e-3);
            let mut busy = false;
            let exit = |which: &str, msg: &str, t0: f64, busy: &mut bool| {
                let e = ((self.now - t0) as f32 / (0.34 * k)).clamp(0.0, 1.0);
                if e < 1.0 {
                    *busy = true;
                    // squash down a touch, then stretch tall and shoot up
                    let up = smooth(((e - 0.15) / 0.85).max(0.0));
                    let sq = (e / 0.15).min(1.0) * (1.0 - up);
                    let st = (up * std::f32::consts::PI).sin();
                    draw(which, msg, 1.0 - 0.35 * up, 1.0 + 0.08 * sq - 0.12 * st, 1.0 - 0.1 * sq + 0.22 * st, 3.0 * sq - up * up * 46.0, (1.0 - up).powf(1.2));
                }
            };
            if n >= 2 {
                let (pm, pb) = &self.toasts[n - 2];
                let t0 = (pb + 2.6).min(self.toasts[n - 1].1);
                if self.now >= t0 {
                    exit("old", pm, t0, &mut busy);
                }
            }
            if let Some((msg, born)) = self.toasts.last() {
                let age = (self.now - born) as f32;
                if age < 2.6 {
                    let delay = if n >= 2 { 0.08 } else { 0.0 };
                    let x = ((age - delay) / (0.55 * k)).clamp(0.0, 1.0);
                    busy |= x < 1.0;
                    if x > 0.0 {
                        // springy pop with a jelly wobble that settles
                        let f = 1.0 - (-6.5 * x).exp() * (11.0 * x).cos();
                        let w = (-5.5 * x).exp() * (16.0 * x).sin() * 0.16;
                        let a = (x / 0.18).min(1.0);
                        draw("cur", msg, 0.55 + 0.45 * f, 1.0 + w, 1.0 - w, (1.0 - f) * 16.0, a);
                    }
                } else {
                    exit("cur", msg, born + 2.6, &mut busy);
                }
            }
            if busy {
                ctx.request_repaint();
            }
        }
    }

    // ---------- right-click menu ----------
}
