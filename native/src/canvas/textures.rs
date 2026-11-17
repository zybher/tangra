//! Image textures: upload, mosaic/pixelate, lazy loading.

use crate::*;

impl App {
    pub(crate) fn put_texture(&mut self, ctx: &egui::Context, file: &str, w: u32, h: u32, rgba: Vec<u8>) {
        if w == 0 || h == 0 || rgba.len() != (w as usize) * (h as usize) * 4 {
            self.missing.insert(file.to_string());
            return;
        }
        let max = (ctx.input(|i| i.max_texture_side) as u32).clamp(1024, 8192);
        let (w, h, rgba) = if w > max || h > max {
            let Some(img) = image::RgbaImage::from_raw(w, h, rgba) else { return };
            let s = max as f32 / w.max(h) as f32;
            let r = image::imageops::resize(
                &img,
                ((w as f32 * s) as u32).max(1),
                ((h as f32 * s) as u32).max(1),
                image::imageops::FilterType::Triangle,
            );
            (r.width(), r.height(), r.into_raw())
        } else {
            (w, h, rgba)
        };
        let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let opts = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
        let tex = ctx.load_texture(file, ci, opts);
        self.textures.insert(file.to_string(), (tex, self.now));
        self.tex_lvl.insert(file.to_string(), 0);
    }

    pub(crate) fn want_mosaic(&mut self, file: &str) {
        let key = format!("m:{file}");
        if self.mosaics.contains_key(file) || self.pending.contains(&key) || self.missing.contains(file) {
            return;
        }
        if self.jobs.send((asset_path(&self.board.id, file), file.to_string(), MOSAIC)).is_ok() {
            self.pending.insert(key);
        }
    }

    /// A pixelate swipe: the image's coarse copy, shown only along the swipe.
    pub(crate) fn paint_pixelate(&self, p: &egui::Painter, img: &ImageNode, s: &Stroke, v: View) {
        let (sx, sy) = img.scale();
        let pts: Vec<Pos2> = s.points.iter().map(|q| v.s(img.x + q.x * sx, img.y + q.y * sy)).collect();
        let width = s.width * v.cam.zoom;
        let Some(tex) = self.mosaics.get(&img.asset_file_name) else {
            // Not ready yet (a moment after the first swipe): cover it plainly
            p.add(Shape::line(pts, EStroke::new(width, gray_ui(70))));
            return;
        };
        let r = Rect::from_min_max(v.s(img.x, img.y), v.s(img.x + img.width, img.y + img.height));
        let mut mesh = egui::Mesh::with_texture(tex.id());
        let hw = width / 2.0;
        // Mouse points sit almost on top of each other; a band built from every tiny
        // step twists into needles. Keep points a fair distance apart, then cover
        // each one with a disc and join neighbours with a straight band.
        let mut keep: Vec<Pos2> = Vec::new();
        for (i, q) in pts.iter().enumerate() {
            let far = keep.last().map_or(true, |l| (*q - *l).length() >= (hw * 0.5).max(1.0));
            if far || i + 1 == pts.len() {
                keep.push(*q);
            }
        }
        let mut vert = |mesh: &mut egui::Mesh, c: Pos2| {
            let uv = pos2((c.x - r.min.x) / r.width().max(1.0), (c.y - r.min.y) / r.height().max(1.0));
            mesh.vertices.push(egui::epaint::Vertex { pos: c, uv, color: Color32::WHITE });
        };
        const SIDES: u32 = 16;
        for (k, c) in keep.iter().enumerate() {
            let i = mesh.vertices.len() as u32;
            vert(&mut mesh, *c);
            for s in 0..SIDES {
                let a = s as f32 / SIDES as f32 * std::f32::consts::TAU;
                vert(&mut mesh, *c + vec2(a.cos(), a.sin()) * hw);
            }
            for s in 0..SIDES {
                mesh.indices.extend_from_slice(&[i, i + 1 + s, i + 1 + (s + 1) % SIDES]);
            }
            if let Some(next) = keep.get(k + 1) {
                let d = *next - *c;
                let len = d.length();
                if len > 0.01 {
                    let n = vec2(-d.y, d.x) / len * hw;
                    let i = mesh.vertices.len() as u32;
                    for q in [*c + n, *next + n, *next - n, *c - n] {
                        vert(&mut mesh, q);
                    }
                    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                }
            }
        }
        p.add(Shape::mesh(mesh));
    }

    /// Loads textures for images on screen (one per frame, so big boards never freeze the UI).
    pub(crate) fn ensure_textures(&mut self, ctx: &egui::Context, v: View, rect: Rect) {
        // Each image is kept at roughly the resolution it is shown at: a board of
        // 50 screenshots seen from afar costs a few MB each instead of ~10 MB.
        let ppp = ctx.pixels_per_point();
        let mut want: HashMap<String, u8> = HashMap::new();
        for i in &self.board.images {
            if !Rect::from_min_max(v.s(i.x, i.y), v.s(i.x + i.width, i.y + i.height)).intersects(rect) {
                continue;
            }
            let (sx, sy) = i.scale();
            // image pixels per screen pixel
            let ratio = 1.0 / (sx.min(sy) * v.cam.zoom * ppp).max(1e-4);
            let l: u8 = if ratio < 1.5 { 0 } else if ratio < 3.0 { 1 } else if ratio < 6.0 { 2 } else { 3 };
            let e = want.entry(i.asset_file_name.clone()).or_insert(l);
            *e = (*e).min(l);
        }
        // Re-sharpening waits until zooming / dragging stops, so it never stutters
        let idle = ctx.input(|i| i.raw_scroll_delta == egui::Vec2::ZERO && !i.pointer.any_down());
        // Decoded images coming back from the worker thread (uploads are cheap)
        for _ in 0..2 {
            let Ok((f, l, out)) = self.done.try_recv() else { break };
            if l == MOSAIC {
                self.pending.remove(&format!("m:{f}"));
                if let Some((w, h, rgba)) = out {
                    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    let tex = ctx.load_texture(format!("m:{f}"), ci, egui::TextureOptions::NEAREST);
                    self.mosaics.insert(f, tex);
                }
                ctx.request_repaint();
                continue;
            }
            self.pending.remove(&f);
            match out {
                Some((w, h, rgba)) => {
                    self.put_texture(ctx, &f, w, h, rgba);
                    self.tex_lvl.insert(f, l);
                }
                None => {
                    self.missing.insert(f);
                }
            }
            ctx.request_repaint();
        }
        let need: Vec<String> = self
            .board
            .images
            .iter()
            .filter(|i| i.annotations.strokes.iter().any(|s| s.pixelate) && !self.mosaics.contains_key(&i.asset_file_name))
            .map(|i| i.asset_file_name.clone())
            .collect();
        for f in need {
            self.want_mosaic(&f);
        }
        for (f, l) in want {
            let have = match self.textures.get_mut(&f) {
                Some(t) => {
                    t.1 = self.now;
                    Some(self.tex_lvl.get(&f).copied().unwrap_or(0))
                }
                None => None,
            };
            // Reload when sharper is needed, or when it is far more detailed than needed
            let reload = have.map_or(true, |h| l < h || l >= h + 2);
            if !reload || self.missing.contains(&f) || self.pending.contains(&f) {
                continue;
            }
            if have.is_some() && !idle {
                ctx.request_repaint_after(Duration::from_millis(150));
                continue;
            }
            // A short queue, so panning past images does not decode ones already gone
            if self.pending.len() >= 3 {
                continue;
            }
            if self.jobs.send((asset_path(&self.board.id, &f), f.clone(), l)).is_ok() {
                self.pending.insert(f);
            }
        }
    }

    // ---------- actions ----------
}
