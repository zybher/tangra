//! Gradient fills (linear, radial, angular, diamond) for shapes, pen paths and frames.
//! Each gradient's colours are baked once into a small texture (cached by its stops); the
//! filled shape's own mesh then gets texture coordinates from where each corner sits relative
//! to the gradient's handles. The mapping is a straight-line function of position, so it stays
//! exact across every triangle, and the shape keeps its smooth edge.

use crate::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// Kinds in menu order (`Gradient::kind`); "Solid" is no gradient at all
pub(crate) const KINDS: [&str; 4] = ["Linear", "Radial", "Angular", "Diamond"];

static TEX: Mutex<Option<HashMap<u64, egui::TextureHandle>>> = Mutex::new(None);

/// The gradient's colour at `t` (0..1), straight RGBA 0..1
pub(crate) fn color_at(g: &Gradient, t: f32) -> [f32; 4] {
    let mut st: Vec<&Stop> = g.stops.iter().collect();
    if st.is_empty() {
        return [1.0; 4];
    }
    st.sort_by(|a, b| a.at.total_cmp(&b.at));
    let c = |s: &Stop| {
        let [r, gg, b] = hex_rgb(&s.color);
        [r as f32 / 255.0, gg as f32 / 255.0, b as f32 / 255.0, s.alpha.clamp(0.0, 1.0)]
    };
    if t <= st[0].at {
        return c(st[0]);
    }
    for w in st.windows(2) {
        if t <= w[1].at {
            let k = ((t - w[0].at) / (w[1].at - w[0].at).max(1e-6)).clamp(0.0, 1.0);
            // smoothing eases in and out of each stop (smootherstep), so no edge shows
            let e = k * k * k * (k * (k * 6.0 - 15.0) + 10.0);
            let k = k + (e - k) * g.smooth.clamp(0.0, 1.0);
            let (a, b) = (c(w[0]), c(w[1]));
            return std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k);
        }
    }
    c(st[st.len() - 1])
}

/// The gradient's colour at `t` as a Color32 (for previews)
pub(crate) fn color32_at(g: &Gradient, t: f32) -> Color32 {
    let [r, gg, b, a] = color_at(g, t);
    Color32::from_rgba_unmultiplied((r * 255.0) as u8, (gg * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8)
}

/// Where along the gradient a texture spot (u, v in -1..1) is, for each kind
fn t_of(kind: u8, u: f32, v: f32) -> f32 {
    match kind {
        0 => (u + 1.0) / 2.0,
        1 => (u * u + v * v).sqrt(),
        2 => (v.atan2(u) / std::f32::consts::TAU).rem_euclid(1.0),
        _ => u.abs() + v.abs(),
    }
}

fn texture(ctx: &egui::Context, g: &Gradient) -> egui::TextureId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.kind.hash(&mut h);
    g.smooth.to_bits().hash(&mut h);
    for s in &g.stops {
        s.at.to_bits().hash(&mut h);
        s.color.hash(&mut h);
        s.alpha.to_bits().hash(&mut h);
    }
    let key = h.finish();
    let Ok(mut cache) = TEX.lock() else { return egui::TextureId::default() };
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(t) = cache.get(&key) {
        return t.id();
    }
    if cache.len() > 64 {
        cache.clear();
    }
    let (w, hgt) = match g.kind {
        0 => (512, 1),
        2 => (512, 512),
        _ => (256, 256),
    };
    let mut px = Vec::with_capacity(w * hgt * 4);
    for y in 0..hgt {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / hgt as f32 * 2.0 - 1.0;
            let [r, gg, b, a] = color_at(g, t_of(g.kind, u, v));
            px.extend([(r * 255.0) as u8, (gg * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8]);
        }
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([w, hgt], &px);
    let t = ctx.load_texture(format!("qd_grad_{key}"), img, egui::TextureOptions::LINEAR);
    let id = t.id();
    cache.insert(key, t);
    id
}

/// The gradient's two handles on screen for an object drawn in `bx`
pub(crate) fn handles(g: &Gradient, bx: Rect) -> (Pos2, Pos2) {
    let f = |q: [f32; 2]| pos2(bx.min.x + q[0] * bx.width(), bx.min.y + q[1] * bx.height());
    (f(g.from), f(g.to))
}

/// Where a screen point sits in the gradient's own space: linear (t, 0); the others (u, v)
/// with the end handle at distance 1 from the centre
pub(crate) fn coords(g: &Gradient, bx: Rect) -> impl Fn(Pos2) -> (f32, f32) {
    let (a, b) = handles(g, bx);
    let a1 = b - a;
    let n = [g.to[0] - g.from[0], g.to[1] - g.from[1]];
    let a2 = vec2(-n[1] * bx.width(), n[0] * bx.height());
    let det = a1.x * a2.y - a1.y * a2.x;
    let det = if det.abs() < 1e-6 { 1e-6 } else { det };
    let kind = g.kind;
    let len2 = a1.length_sq().max(1e-6);
    move |p: Pos2| {
        let d = p - a;
        if kind == 0 {
            (d.dot(a1) / len2, 0.0)
        } else {
            ((d.x * a2.y - d.y * a2.x) / det, (a1.x * d.y - a1.y * d.x) / det)
        }
    }
}

/// Texture coordinates for a screen point of an object drawn in `bx`
fn uv_map(g: &Gradient, bx: Rect) -> impl Fn(Pos2) -> Pos2 {
    let (a, b) = handles(g, bx);
    let a1 = b - a;
    // the second axis: square to the first in the object's own box, so a radial gradient
    // fills an ellipse that stretches with the object
    let n = [g.to[0] - g.from[0], g.to[1] - g.from[1]];
    let a2 = vec2(-n[1] * bx.width(), n[0] * bx.height());
    let det = a1.x * a2.y - a1.y * a2.x;
    let det = if det.abs() < 1e-6 { 1e-6 } else { det };
    let solve = move |p: Pos2| {
        let d = p - a;
        ((d.x * a2.y - d.y * a2.x) / det, (a1.x * d.y - a1.y * d.x) / det)
    };
    // angular: only the direction counts, so shrink until the whole box fits the texture
    let m = if g.kind == 2 {
        [bx.left_top(), bx.right_top(), bx.left_bottom(), bx.right_bottom()].iter().map(|c| {
            let (u, v) = solve(*c);
            u.abs().max(v.abs())
        }).fold(1e-3, f32::max)
    } else {
        1.0
    };
    let kind = g.kind;
    let len2 = a1.length_sq().max(1e-6);
    move |p: Pos2| {
        if kind == 0 {
            pos2((p - a).dot(a1) / len2, 0.5)
        } else {
            let (u, v) = solve(p);
            pos2(0.5 + 0.5 * u / m, 0.5 + 0.5 * v / m)
        }
    }
}

/// `shape` (drawn in white) painted with the gradient instead; `bx` is the object's box on
/// screen (the handles are fractions of it), `alpha` fades it as a whole
pub(crate) fn fill(p: &egui::Painter, shape: egui::Shape, g: &Gradient, bx: Rect, alpha: f32) {
    let ctx = p.ctx();
    let mut mesh = match shape {
        egui::Shape::Mesh(m) => m,
        shape => {
            let mut t = egui::epaint::Tessellator::new(ctx.pixels_per_point(), Default::default(), ctx.fonts(|f| f.font_image_size()), vec![]);
            let mut m = egui::Mesh::default();
            t.tessellate_shape(shape, &mut m);
            m
        }
    };
    // sharp and banding-free on the GPU when it can (canvas/gradient_gl.rs)
    if super::gradient_gl::available() {
        let at = coords(g, bx);
        let ppp = ctx.pixels_per_point();
        let mut data = Vec::with_capacity(mesh.vertices.len() * 5);
        for v in &mesh.vertices {
            let (u, w) = at(v.pos);
            data.extend([v.pos.x * ppp, v.pos.y * ppp, u, w, v.color.a() as f32 / 255.0]);
        }
        let ramp: Vec<u8> = (0..super::gradient_gl::RAMP).flat_map(|i| {
            let [r, gg, b, a] = color_at(g, i as f32 / (super::gradient_gl::RAMP - 1) as f32);
            [r, gg, b, a].map(|c| (c * 255.0).round() as u8)
        }).collect();
        super::gradient_gl::draw(p, data, mesh.indices, ramp, g.kind, alpha);
        return;
    }
    let map = uv_map(g, bx);
    for v in &mut mesh.vertices {
        let a = ((v.color.a() as f32 / 255.0) * alpha * 255.0).round() as u8;
        v.color = Color32::from_rgba_premultiplied(a, a, a, a);
        v.uv = map(v.pos);
    }
    mesh.texture_id = texture(ctx, g);
    p.add(egui::Shape::mesh(mesh));
}
