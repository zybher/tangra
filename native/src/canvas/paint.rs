//! Draws the board: background, items, annotations, selection, overlays.

use crate::*;

impl App {
    pub(crate) fn paint(&self, p: &egui::Painter, rect: Rect, v: View, hover: Option<Pos2>) {
        let z = v.cam.zoom;
        let b = &self.board;
        let r_top = Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 };
        p.rect_filled(rect, r_top, bg());
        if gradient_on() {
            fill_rounded_top(p, rect, 10.0, |q| gradient_at(q, rect));
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

        // Dot grid
        let mut g = 32.0 * z;
        while g < 20.0 {
            g *= 2.0;
        }
        let dot = rgba(255, 255, 255, (self.settings.grid_dots.clamp(0.0, 100.0) * 0.26) as u8);
        if dot.a() > 0 {
        let mut x = rect.min.x + (-v.cam.x * z).rem_euclid(g);
        while x < rect.max.x {
            let mut y = rect.min.y + (-v.cam.y * z).rem_euclid(g);
            while y < rect.max.y {
                draw_pattern(&p, pos2(x + 0.75, y + 0.75), self.settings.pattern, dot, 1.0);
                y += g;
            }
            x += g;
        }
        }

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
        let clips = self.clip_boxes();
        // Faded objects (and everything inside a faded frame)
        let fades = self.fades();
        // Frames
        // The title lights with the hover outline (one source of truth, set below last frame)
        let hovered_now: Option<Sel> = p.ctx().data(|d| d.get_temp::<Option<Sel>>(egui::Id::new("qd_hover_now"))).flatten();
        // Frame strokes go on after the objects, so content never covers a frame's edge
        let mut late_strokes: Vec<(egui::Painter, egui::Shape)> = Vec::new();
        // Parents paint before the frames inside them, so a nested frame is never hidden
        for c in self.frames_paint_order() {
            if hid.contains(&c.id) {
                continue;
            }
            // The title is not faded with the frame (it labels the frame, it isn't part of it)
            let title_p = match clips.get(&c.id) {
                Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                None => p.clone(),
            };
            let clipped_p = {
                let mut q = title_p.clone();
                if let Some(o) = fades.get(&c.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            let r = item_rect(c.x, c.y, c.width, c.height);
            if !r.intersects(rect) {
                continue;
            }
            // A solid panel with its title above it; the old default slate counts as "no colour"
            let panel = Color32::from_rgb(36, 36, 36);
            let custom = !c.color.is_empty() && !c.color.eq_ignore_ascii_case("#1e293b");
            let (fill_color, stroke_color, title_color) = if c.color.eq_ignore_ascii_case("none") {
                // No fill: only the outline (and title) show
                (Color32::TRANSPARENT, Color32::from_white_alpha(48), FRAME_TITLE_COL)
            } else if custom {
                let [cr, cg, cb] = hex_rgb(&c.color);
                // The frame is the colour picked; the title keeps the neutral grey
                let c = rgba(cr, cg, cb, 255);
                (c, mix(c, Color32::BLACK, 0.25), FRAME_TITLE_COL)
            } else {
                (panel, Color32::from_white_alpha(48), FRAME_TITLE_COL)
            };
            let fr = group_radius(c) * z;
            let cut = round_cut(&c.id, r, fr).filter(|(_, safe)| !safe);
            match &cut {
                Some((poly, _)) if poly.len() >= 3 => {
                    p.add(egui::Shape::convex_polygon(poly.clone(), fill_color, EStroke::NONE));
                }
                Some(_) => {}
                None => {
                    p.rect_filled(r, fr, fill_color);
                }
            }
            // Stroke is in world units: it scales with the zoom like the frame itself
            let bw = c.border.unwrap_or(1.0) * z;
            // Stroke position: inside keeps the frame's size, outside grows past it
            let (sr, sfr) = match c.stroke_pos {
                1 => (r.shrink(bw / 2.0), (fr - bw / 2.0).max(0.0)),
                2 => (r.expand(bw / 2.0), fr + bw / 2.0),
                _ => (r, fr),
            };
            if bw > 0.0 {
                match round_cut(&c.id, sr, sfr).filter(|(_, safe)| !safe) {
                    Some((poly, _)) if poly.len() >= 3 => {
                        late_strokes.push((p.clone(), egui::Shape::closed_line(poly, EStroke::new(bw, stroke_color))));
                    }
                    Some(_) => {}
                    None => {
                        // rect_stroke paints outward from the rect: hand it the inner edge
                        late_strokes.push((p.clone(), egui::Shape::rect_stroke(sr.shrink(bw / 2.0), (sfr - bw / 2.0).max(0.0), EStroke::new(bw, stroke_color))));
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
        }

        // Images + their annotations
        for img in &b.images {
            if hid.contains(&img.id) {
                continue;
            }
            let clipped_p = {
                let mut q = match clips.get(&img.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                if let Some(o) = fades.get(&img.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            let r = item_rect(img.x, img.y, img.width, img.height);
            if !r.intersects(rect) {
                continue;
            }
            // Rounded corners (screen px), never more than half the short side
            let rd = (img.corner * z).clamp(0.0, r.width().min(r.height()) / 2.0);
            // Cut to a rounded frame's corners (None: away from them, drawn as usual)
            let cut = round_cut(&img.id, r, rd).filter(|(_, safe)| !safe).map(|(poly, _)| poly);
            if cut.is_none() {
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
            if let Some((tex, _)) = self.textures.get(&img.asset_file_name) {
                if let Some(poly) = &cut {
                    p.add(egui::Shape::mesh(super::round_clip::textured_poly(poly, r, tex.id(), Color32::WHITE)));
                } else if rd > 0.0 {
                    let mut rs = egui::epaint::RectShape::filled(r, rd, Color32::WHITE);
                    rs.fill_texture_id = tex.id();
                    rs.uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    p.add(rs);
                } else {
                    p.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            } else if let Some(poly) = cut.as_ref().filter(|q| q.len() >= 3) {
                p.add(egui::Shape::convex_polygon(poly.clone(), rgba(30, 41, 59, 255), EStroke::NONE));
            } else {
                p.rect_filled(r, rd, rgba(30, 41, 59, 255));
                let msg = if self.missing.contains(&img.asset_file_name) { "Image file missing" } else { "Loading…" };
                p.text(r.center(), Align2::CENTER_CENTER, msg, FontId::proportional(12.0), TEXT_FAINT);
            }
            let (sx, sy) = img.scale();
            let ip = p.with_clip_rect(r.intersect(p.clip_rect()));
            for s in img.annotations.strokes.iter().filter(|s| s.pixelate) {
                self.paint_pixelate(&ip, img, s, v);
            }
            for s in img.annotations.strokes.iter().filter(|s| !s.pixelate) {
                paint_stroke(&ip, s, |q| v.s(img.x + q.x * sx, img.y + q.y * sy), s.width * z);
            }
            for t in &img.annotations.texts {
                let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
                let editing = matches!(&self.edit, Some(Edit { target: EditTarget::ImageLabel { label, .. }, .. }) if label == &t.id);
                let picked = matches!(&self.sel_label, Some((_, l)) if l == &t.id);
                if let (false, Some(f)) = (t.text.is_empty() || editing, font(fs * sx * z)) {
                    let pos = v.s(img.x + t.x * sx, img.y + t.y * sy);
                    let galley = label_galley(&ip, &t.text, f, hex(&t.color), t.style);
                    let pad = fs * sx * z * 0.25;
                    let bg = Rect::from_min_size(pos, galley.size()).expand2(vec2(pad, pad * 0.5));
                    ip.rect_filled(bg, 3.0, Color32::from_black_alpha(180));
                    if picked {
                        ip.rect_stroke(bg.expand(2.0), 3.0, EStroke::new(1.5_f32, blue()));
                    }
                    draw_label(&ip, pos, galley, hex(&t.color), t.style, fs * sx * z);
                }
            }
        }

        // Link cards
        for l in &b.link_cards {
            if hid.contains(&l.id) {
                continue;
            }
            let clipped_p = {
                let mut q = match clips.get(&l.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                if let Some(o) = fades.get(&l.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            let r = item_rect(l.x, l.y, l.width, l.height);
            if !r.intersects(rect) {
                continue;
            }
            p.rect_filled(r, 10.0 * z, Color32::from_rgb(38, 38, 38));
            p.rect_stroke(r, 10.0 * z, EStroke::new(1.0_f32, rgba(255, 255, 255, 22)));
            let cp = p.with_clip_rect(r.shrink(10.0 * z).intersect(rect));
            if let Some(f) = font(11.0 * z) {
                let domain = if l.domain.is_empty() { "LINK" } else { &l.domain };
                cp.text(v.s(l.x + 14.0, l.y + 14.0), Align2::LEFT_TOP, domain, f.clone(), rgba(56, 189, 248, 255));
                cp.text(v.s(l.x + 14.0, l.y + 62.0), Align2::LEFT_TOP, &l.url, f.clone(), TEXT_FAINT);
                if !l.notes.is_empty() {
                    cp.text(v.s(l.x + 14.0, l.y + 84.0), Align2::LEFT_TOP, &l.notes, f, rgba(148, 163, 184, 255));
                }
            }
            if let Some(f) = font(14.0 * z) {
                let title = if l.title.is_empty() { &l.url } else { &l.title };
                cp.text(v.s(l.x + 14.0, l.y + 36.0), Align2::LEFT_TOP, title, f, rgba(241, 245, 249, 255));
            }
        }

        // Sticky notes
        for n in &b.sticky_notes {
            if hid.contains(&n.id) {
                continue;
            }
            let clipped_p = {
                let mut q = match clips.get(&n.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                if let Some(o) = fades.get(&n.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            let r = item_rect(n.x, n.y, n.width, n.height);
            if !r.intersects(rect) {
                continue;
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
        }

        // Free-floating strokes and text
        for s in &b.world_strokes {
            if hid.contains(&s.id) {
                continue;
            }
            let clipped_p = {
                let mut q = match clips.get(&s.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                if let Some(o) = fades.get(&s.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            paint_stroke(p, s, |q| v.s(q.x, q.y), s.width * z);
        }
        for t in &b.world_texts {
            if hid.contains(&t.id) {
                continue;
            }
            let clipped_p = {
                let mut q = match clips.get(&t.id) {
                    Some(q) => p.with_clip_rect(item_rect(q[0], q[1], q[2], q[3]).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                if let Some(o) = fades.get(&t.id) {
                    q.multiply_opacity(*o);
                }
                q
            };
            let p = &clipped_p;
            let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
            // Hidden while it is being edited in place
            let editing = matches!(&self.edit, Some(Edit { target: EditTarget::WorldText(id), .. }) if id == &t.id);
            if editing || t.text.is_empty() {
                continue;
            }
            if font(fs * z).is_some() {
                let fam = styled_font(FontId::proportional(fs), t.style).family;
                smooth_text(p, v.s(t.x, t.y), &t.text, fam, fs, z, None, hex(&t.color));
            } else {
                // Too small to read: a bar in the text's colour keeps it findable
                let (w, h) = text_wh(&t.text, fs);
                let at = v.s(t.x, t.y + h * 0.3);
                let bar = Rect::from_min_size(at, vec2((w * z).max(3.0), (h * z * 0.4).max(1.5)));
                p.rect_filled(bar, 1.0, hex(&t.color).gamma_multiply(0.6));
            }
        }

        for (lp, s) in late_strokes {
            lp.add(s);
        }
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
                if let Some([x, y, w, h]) = self.rect_of(k, &id) {
                    let r = item_rect(x, y, w, h).shrink(0.5);
                    let round = match k {
                        Kind::Image => self.board.images.iter().find(|i| i.id == id).map_or(0.0, |i| i.corner * z),
                        Kind::Group => self.board.group_cards.iter().find(|g| g.id == id).map_or(0.0, |g| group_radius(g) * z),
                        Kind::Stroke => self.find_stroke(&id).and_then(|s| s.radius).map_or(2.0, |r| r * z),
                        Kind::Text => 2.0,
                        _ => 10.0 * z,
                    };
                    p.rect_stroke(r, round, EStroke::new(2.0_f32, blue().gamma_multiply(a)));
                }
            }
        }

        // Canvas drag: the frame the cursor is in lights up ("let go to drop it here")
        if let Some(g) = self.drop_target().and_then(|t| self.board.group_cards.iter().find(|g| g.id == t)) {
            let r = item_rect(g.x, g.y, g.width, g.height).expand(1.0);
            p.rect_stroke(r, group_radius(g) * z, EStroke::new(2.0_f32, blue()));
        }

        // Selection outlines
        for (k, id) in &self.selected {
            if let Some([x, y, w, h]) = self.rect_of(*k, id) {
                // Centred half a pixel in: covers the object's own edge, so no dark sliver shows
                let r = item_rect(x, y, w, h).shrink(0.5);
                let round = match k {
                    Kind::Image => self.board.images.iter().find(|i| &i.id == id).map_or(0.0, |i| i.corner * z),
                    Kind::Group => self.board.group_cards.iter().find(|g| &g.id == id).map_or(0.0, |g| group_radius(g) * z),
                    Kind::Stroke => self.find_stroke(id).and_then(|s| s.radius).map_or(2.0, |r| r * z),
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

        // Multi-selection: one bounding box with the same size pills as a single object
        if self.selected.len() > 1 {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (k, id) in &self.selected {
                if let Some([x, y, w, h]) = self.rect_of(*k, id) {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
                }
            }
            if x1 > x0 {
                let r = item_rect(x0, y0, x1 - x0, y1 - y0).expand(0.5);
                p.rect_stroke(r, 0.0, EStroke::new(1.5_f32, blue()));
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

        self.paint_snap(p, v, hover);
        self.paint_gap_handles(p, v, hover);
        self.paint_multi_handles(p, v, hover);

        // Marquee selection box
        if let Drag::BoxSelect { start, current, .. } = self.drag {
            let p1 = v.s(start.x, start.y);
            let p2 = v.s(current.x, current.y);
            let marquee = Rect::from_two_pos(p1, p2);
            p.rect_filled(marquee, 0.0, blue().gamma_multiply(0.18));
            p.rect_stroke(marquee, 0.0, EStroke::new(1.0_f32, blue()));
        }

        // Eraser cursor
        if let (Tool::Eraser, Some(h)) = (self.tool, hover) {
            let panning = p.ctx().input(|i| i.key_down(Key::Space) || i.pointer.middle_down()) && self.edit.is_none();
            if rect.contains(h) && !panning {
                // Drawn on the top layer so it stays above the toolbar, like a real cursor
                p.ctx()
                    .layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("qd_eraser_cursor")))
                    .circle_stroke(h, 9.0, EStroke::new(1.2_f32, rgba(255, 255, 255, 190)));
            }
        }

        // Empty-board hint
        if b.is_empty() && self.edit.is_none() {
            p.text(
                rect.center() - vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                "Right-click anywhere",
                FontId::proportional(18.0),
                rgba(148, 163, 184, 150),
            );
            p.text(
                rect.center() + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                "Ctrl+V to paste  ·  scroll to pan  ·  Ctrl+scroll to zoom",
                FontId::proportional(12.5),
                rgba(100, 116, 139, 150),
            );
        }

        // Active tool pill (top-center)
        let tool = match self.tool {
            Tool::Select => None,
            Tool::Pen => Some("Pen"),
            Tool::Highlighter => Some("Highlighter"),
            Tool::Pixelate => Some("Pixelate  ·  swipe over an image"),
            Tool::Text => Some("Text  ·  click to place"),
            Tool::Eraser => Some("Eraser  ·  drag over strokes"),
            Tool::Rect => Some("Rectangle  ·  drag to draw"),
            Tool::Ellipse => Some("Ellipse  ·  drag to draw"),
            Tool::Arrow => Some("Arrow  ·  drag to draw"),
            Tool::Frame => Some("Frame  ·  click to place, drag to size"),
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
                let galley = lp.layout_no_wrap(format!("{name}   ·   Esc to finish"), FontId::proportional(12.0), TEXT.gamma_multiply(a));
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
            let keys: [(&str, &str, Option<Tool>); 11] = [
                ("V", "Select", Some(Tool::Select)),
                ("F", "Frame", Some(Tool::Frame)),
                ("R", "Rectangle", Some(Tool::Rect)),
                ("O", "Ellipse", Some(Tool::Ellipse)),
                ("A", "Arrow", Some(Tool::Arrow)),
                ("P", "Pen", Some(Tool::Pen)),
                ("H", "Highlighter", Some(Tool::Highlighter)),
                ("B", "Blur", Some(Tool::Pixelate)),
                ("T", "Text", Some(Tool::Text)),
                ("E", "Eraser", Some(Tool::Eraser)),
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
                let (kc, nc) = (mix(TEXT_DIM, blue(), on), mix(TEXT_FAINT, TEXT, on));
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
                p.text(pos2(rect.min.x + 16.0 + dx, y + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), TEXT_FAINT.gamma_multiply(a));
                p.text(pos2(rect.min.x + 72.0 + dx, y + 8.0), Align2::LEFT_CENTER, value, FontId::proportional(11.5), if lit { blue() } else { TEXT_DIM }.gamma_multiply(a));
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
                let galley = lp.layout_no_wrap(msg.to_string(), FontId::proportional(12.5), TEXT.gamma_multiply(a));
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
