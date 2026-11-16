//! Per-picture edits that never touch the file on disk: colour adjustments,
//! cut-out areas (from the lasso) and the 3D tilt. The adjusted pixels are
//! rebuilt on the CPU when the settings change and kept as one texture per picture.

use crate::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Longest side the edited copy is worked on at (keeps scrubbing smooth)
const EDIT_MAX: u32 = 1600;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageEdit {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    /// Degrees, -180..180
    pub hue: f32,
    pub warmth: f32,
    /// Cut-out loops, in original image pixels
    pub cuts: Vec<Vec<Pt>>,
    /// 3D: lean back / forward, turn left / right, spin flat (degrees), perspective depth
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub spin: f32,
    pub depth: f32,
}

impl ImageEdit {
    pub fn has_color(&self) -> bool {
        [self.brightness, self.contrast, self.saturation, self.hue, self.warmth].iter().any(|v| *v != 0.0)
    }
    pub fn has_pixels(&self) -> bool {
        self.has_color() || !self.cuts.is_empty()
    }
    pub fn has_tilt(&self) -> bool {
        self.tilt_x != 0.0 || self.tilt_y != 0.0 || self.spin != 0.0
    }
    pub fn is_empty(&self) -> bool {
        !self.has_pixels() && !self.has_tilt() && self.depth == 0.0
    }
    /// Changes whenever the pixels would come out different
    fn pixel_key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in [self.brightness, self.contrast, self.saturation, self.hue, self.warmth] {
            v.to_bits().hash(&mut h);
        }
        for c in &self.cuts {
            c.len().hash(&mut h);
            for p in c {
                (p.x.to_bits(), p.y.to_bits()).hash(&mut h);
            }
        }
        h.finish()
    }
    /// Field by menu index (see `FIELDS`)
    pub fn field(&mut self, k: u8) -> &mut f32 {
        match k {
            0 => &mut self.brightness,
            1 => &mut self.contrast,
            2 => &mut self.saturation,
            3 => &mut self.hue,
            4 => &mut self.warmth,
            5 => &mut self.tilt_x,
            6 => &mut self.tilt_y,
            7 => &mut self.spin,
            _ => &mut self.depth,
        }
    }
}

/// (menu index, label, low, high) for the colour rows
pub(crate) const ADJUST_ROWS: [(u8, &str, f32, f32); 5] = [
    (0, "Brightness", -100.0, 100.0),
    (1, "Contrast", -100.0, 100.0),
    (2, "Saturation", -100.0, 100.0),
    (3, "Hue", -180.0, 180.0),
    (4, "Warmth", -100.0, 100.0),
];
/// Same for the 3D rows
pub(crate) const TILT_ROWS: [(u8, &str, f32, f32); 4] = [
    (5, "Tilt", -80.0, 80.0),
    (6, "Turn", -80.0, 80.0),
    (7, "Spin", -180.0, 180.0),
    (8, "Depth", -100.0, 100.0),
];

fn mat_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// Colour-adjust `src` (w×h RGBA) and punch the cuts out of its alpha.
/// `ow, oh` are the original pixel size the cut points are measured in.
fn apply(e: &ImageEdit, w: u32, h: u32, src: &[u8], ow: f32, oh: f32) -> Vec<u8> {
    let mut out = src.to_vec();
    if e.has_color() {
        // Saturation and hue as one matrix (the usual luminance-preserving ones)
        let k = 1.0 + e.saturation / 100.0;
        let sat = [
            [0.213 + 0.787 * k, 0.715 - 0.715 * k, 0.072 - 0.072 * k],
            [0.213 - 0.213 * k, 0.715 + 0.285 * k, 0.072 - 0.072 * k],
            [0.213 - 0.213 * k, 0.715 - 0.715 * k, 0.072 + 0.928 * k],
        ];
        let (s, c) = e.hue.to_radians().sin_cos();
        let hue = [
            [0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928],
            [0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283],
            [0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072],
        ];
        let m = mat_mul(hue, sat);
        let cf = if e.contrast >= 0.0 { 1.0 + e.contrast / 100.0 * 1.5 } else { 1.0 + e.contrast / 100.0 };
        let br = e.brightness / 100.0 * 0.35;
        let wm = e.warmth / 100.0 * 0.12;
        let off = [wm, wm * 0.2, -wm];
        for px in out.chunks_exact_mut(4) {
            let v = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0];
            for i in 0..3 {
                let x = m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] + off[i];
                let x = (x - 0.5) * cf + 0.5 + br;
                px[i] = (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
    }
    let (fx, fy) = (w as f32 / ow.max(1.0), h as f32 / oh.max(1.0));
    for cut in e.cuts.iter().filter(|c| c.len() >= 3) {
        let poly: Vec<(f32, f32)> = cut.iter().map(|p| (p.x * fx, p.y * fy)).collect();
        cut_out(&mut out, w as usize, h as usize, &poly);
    }
    out
}

/// Clear the alpha inside `poly`, antialiased with 4 sub-scanlines and exact span ends.
fn cut_out(px: &mut [u8], w: usize, h: usize, poly: &[(f32, f32)]) {
    let y0 = poly.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let y1 = (poly.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(h);
    let mut row = vec![0.0_f32; w];
    let mut xs: Vec<f32> = Vec::new();
    for y in y0..y1 {
        row.iter_mut().for_each(|c| *c = 0.0);
        for sub in 0..4 {
            let sy = y as f32 + (sub as f32 + 0.5) / 4.0;
            xs.clear();
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                if (a.1 <= sy) != (b.1 <= sy) {
                    xs.push(a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for pair in xs.chunks_exact(2) {
                let (x0, x1) = (pair[0].clamp(0.0, w as f32), pair[1].clamp(0.0, w as f32));
                if x1 <= x0 {
                    continue;
                }
                let (i0, i1) = (x0 as usize, (x1.ceil() as usize).min(w));
                for (i, c) in row.iter_mut().enumerate().take(i1).skip(i0) {
                    let cov = (x1.min(i as f32 + 1.0) - x0.max(i as f32)).clamp(0.0, 1.0);
                    *c += 0.25 * cov;
                }
            }
        }
        for (x, c) in row.iter().enumerate() {
            if *c > 0.0 {
                let a = &mut px[(y * w + x) * 4 + 3];
                *a = (*a as f32 * (1.0 - c.min(1.0))).round() as u8;
            }
        }
    }
}

impl App {
    /// Rebuild the edited copy of every picture whose colour settings or cuts changed.
    pub(crate) fn ensure_edits(&mut self, ctx: &egui::Context) {
        let mut live: HashSet<String> = HashSet::new();
        let mut files: HashSet<String> = HashSet::new();
        for i in 0..self.board.images.len() {
            let img = &self.board.images[i];
            let Some(e) = img.edit.as_ref().filter(|e| e.has_pixels()) else { continue };
            live.insert(img.id.clone());
            files.insert(img.asset_file_name.clone());
            let key = e.pixel_key();
            if self.edited.get(&img.id).is_some_and(|t| t.0 == key) {
                continue;
            }
            let (file, id, e, ow, oh) = (img.asset_file_name.clone(), img.id.clone(), e.clone(), img.original_width, img.original_height);
            if !self.edit_base.contains_key(&file) {
                if self.missing.contains(&file) {
                    continue;
                }
                let Ok((w, h, px)) = platform::load_rgba(&asset_path(&self.board.id, &file)) else {
                    self.missing.insert(file);
                    continue;
                };
                let base = if w.max(h) > EDIT_MAX {
                    let s = EDIT_MAX as f32 / w.max(h) as f32;
                    let Some(im) = image::RgbaImage::from_raw(w, h, px) else { continue };
                    let r = image::imageops::resize(&im, ((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1), image::imageops::FilterType::Triangle);
                    (r.width(), r.height(), r.into_raw())
                } else {
                    (w, h, px)
                };
                self.edit_base.insert(file.clone(), base);
            }
            let (w, h, px) = &self.edit_base[&file];
            let out = apply(&e, *w, *h, px, ow, oh);
            let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], &out);
            let opts = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
            let tex = ctx.load_texture(format!("edit:{id}"), ci, opts);
            self.edited.insert(id, (key, tex));
        }
        self.edited.retain(|k, _| live.contains(k));
        self.edit_base.retain(|k, _| files.contains(k));
    }

    /// Pictures the bar's sliders apply to: the selected ones
    fn edit_targets(&self) -> Vec<String> {
        self.selected.iter().filter(|s| s.0 == Kind::Image).map(|s| s.1.clone()).collect()
    }

    pub(crate) fn edit_each(&mut self, f: impl Fn(&mut ImageEdit)) {
        let ids = self.edit_targets();
        for img in self.board.images.iter_mut().filter(|i| ids.contains(&i.id)) {
            let mut e = img.edit.take().unwrap_or_default();
            f(&mut e);
            img.edit = (!e.is_empty()).then_some(e);
        }
        self.mark_dirty();
    }

    /// One scrub row moved (first frame of the drag takes the undo step)
    pub(crate) fn set_image_edit(&mut self, k: u8, v: f32, first: bool) {
        if first {
            self.snapshot();
        }
        self.edit_each(|e| *e.field(k) = v);
    }

    pub(crate) fn reset_adjust(&mut self) {
        self.snapshot();
        self.edit_each(|e| (e.brightness, e.contrast, e.saturation, e.hue, e.warmth) = (0.0, 0.0, 0.0, 0.0, 0.0));
    }

    pub(crate) fn reset_tilt(&mut self) {
        self.snapshot();
        self.edit_each(|e| (e.tilt_x, e.tilt_y, e.spin, e.depth) = (0.0, 0.0, 0.0, 0.0));
    }

    pub(crate) fn restore_cuts(&mut self) {
        self.snapshot();
        self.edit_each(|e| e.cuts.clear());
        // and the spots rubbed out with the eraser (canvas/erase.rs)
        let ids: std::collections::HashSet<String> = self.selected.iter().filter(|s| s.0 == Kind::Image).map(|s| s.1.clone()).collect();
        for i in self.board.images.iter_mut().filter(|i| ids.contains(&i.id)) {
            i.erase.clear();
            i.erase_loops.clear();
        }
        self.mark_dirty();
    }
}
