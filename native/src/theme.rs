//! Colours and canvas looks: theme/accent palette, surface tones, gradients, Aurora.

use super::*;

/// Canvas colour presets: (name, canvas rgb). All dark enough that white text,
/// the blue accent and every pen colour stay readable on them.
pub(crate) const THEMES: [(&str, [u8; 3]); 7] = [
    ("Graphite", [0x19, 0x19, 0x19]),
    ("Midnight", [0x12, 0x17, 0x24]),
    ("Forest", [0x13, 0x1d, 0x18]),
    ("Wine", [0x24, 0x13, 0x18]),
    ("Plum", [0x1c, 0x15, 0x26]),
    ("Espresso", [0x22, 0x1b, 0x16]),
    ("Aurora", [0x13, 0x14, 0x22]),
];
/// One background pattern mark centred on `c`
pub(crate) fn draw_pattern(p: &egui::Painter, c: Pos2, kind: u8, col: Color32, k: f32) {
    let ln = |a: egui::Vec2, b: egui::Vec2| p.line_segment([c + a * k, c + b * k], EStroke::new(1.0_f32, col));
    match kind {
        1 => {
            ln(vec2(-3.0, 0.0), vec2(3.0, 0.0));
            ln(vec2(0.0, -3.0), vec2(0.0, 3.0));
        }
        2 => {
            ln(vec2(-4.0, 0.0), vec2(4.0, 0.0));
            ln(vec2(0.0, -4.0), vec2(0.0, 4.0));
            ln(vec2(-1.6, -1.6), vec2(1.6, 1.6));
            ln(vec2(-1.6, 1.6), vec2(1.6, -1.6));
        }
        3 => {
            for (d, q) in [(vec2(1.0, 0.0), vec2(0.0, 1.0)), (vec2(0.0, 1.0), vec2(-1.0, 0.0)), (vec2(-1.0, 0.0), vec2(0.0, -1.0)), (vec2(0.0, -1.0), vec2(1.0, 0.0))] {
                ln(q * 1.5, d * 4.5);
                ln(-q * 0.5, d * 4.5);
            }
        }
        4 => {
            ln(vec2(-2.5, -2.5), vec2(2.5, 2.5));
            ln(vec2(-2.5, 2.5), vec2(2.5, -2.5));
        }
        _ => {
            p.rect_filled(Rect::from_center_size(c, vec2(1.5, 1.5) * k), 0.0, col);
        }
    }
}
/// Background pattern names, by settings.pattern
pub(crate) const PATTERNS: [&str; 5] = ["Dots", "Plus", "Stars", "Shuriken", "Cross"];
/// Index of the animated style in THEMES
pub(crate) const AURORA: usize = 6;
pub(crate) static THEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The canvas colour on screen (gliding to a new pick, theme_glide.rs)
pub(crate) fn theme_rgb() -> [u8; 3] {
    crate::theme_glide::shown_theme().unwrap_or_else(theme_target)
}
/// The picked theme's colour
pub(crate) fn theme_target() -> [u8; 3] {
    THEMES[THEME.load(std::sync::atomic::Ordering::Relaxed) % THEMES.len()].1
}
/// Three tones per theme, all from the theme's base colour: the tab bar is darkest, the
/// canvas a step up, floating bars (toolbar, outline, menus) a step above the canvas.
pub(crate) fn shade(base: [u8; 3], d: i16) -> Color32 {
    let f = |v: u8| (v as i16 + d).clamp(0, 255) as u8;
    Color32::from_rgb(f(base[0]), f(base[1]), f(base[2]))
}
/// The tab bar / window chrome behind the tabs
pub(crate) fn chrome() -> Color32 {
    tone(-10, -12)
}
/// The canvas (and the active tab, which is joined to it)
pub(crate) fn bg() -> Color32 {
    tone(-5, -5)
}
/// Floating bars: toolbar, outline, style bar
pub(crate) fn frame_bg() -> Color32 {
    tone(2, 14)
}
// ---- One palette for the whole UI ----
/// Accent presets: (name, rgb)
pub(crate) const ACCENTS: [(&str, u32); 7] = [
    ("Blue", 0x3b82f6),
    ("Violet", 0x8b5cf6),
    ("Pink", 0xec4899),
    ("Orange", 0xf97316),
    ("Green", 0x22c55e),
    ("Teal", 0x14b8a6),
    ("Silver", 0xa1a1aa),
];
pub(crate) static ACCENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3b82f6);
/// The accent: selection outline, active buttons, switches
/// Spacing handles (gap tool): a fixed magenta so it never blends with the accent
pub(crate) fn spacing_col() -> Color32 {
    Color32::from_rgb(232, 62, 170)
}

pub(crate) fn blue() -> Color32 {
    let [r, g, b] = crate::theme_glide::shown_accent().unwrap_or_else(accent_target);
    Color32::from_rgb(r, g, b)
}
/// The picked accent
pub(crate) fn accent_target() -> [u8; 3] {
    let v = ACCENT.load(std::sync::atomic::Ordering::Relaxed);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}
/// Floating surfaces: menus, dialogs, pills, toasts.
pub(crate) fn txt() -> Color32 {
    gray_ui(236)
}
pub(crate) fn txt_dim() -> Color32 {
    gray_ui(165)
}
pub(crate) fn txt_faint() -> Color32 {
    gray_ui(115)
}
/// The strongest ink (hovered / active text and icons): white in dark mode, near-black in light
pub(crate) fn hi() -> Color32 {
    gray_ui(255)
}

// ---- Light mode (Settings > Extras) ----
// One switch: the theme's surfaces swap to a light version of the same tint, and every
// neutral grey / white wash in the UI turns over (light text becomes dark, white hover
// washes become dark ones). Both cross-fade with the app's usual ease. The accent, pen
// colours and what's on the board stay as they are.
pub(crate) static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LIGHT_K: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// How light the UI is right now (0 dark .. 1 light, eased)
pub(crate) fn lk() -> f32 {
    f32::from_bits(LIGHT_K.load(std::sync::atomic::Ordering::Relaxed))
}

/// Run every frame: eases toward the picked mode and re-tints egui's own widgets meanwhile
pub(crate) fn tick_light(ctx: &egui::Context) {
    let k = ease(ctx, egui::Id::new("qd_light_k"), LIGHT.load(std::sync::atomic::Ordering::Relaxed));
    if (k - lk()).abs() > 1e-4 {
        LIGHT_K.store(k.to_bits(), std::sync::atomic::Ordering::Relaxed);
        retint(ctx);
    }
}

/// The theme's tint as a light surface: a near-white that leans the same way
fn light_base([r, g, b]: [u8; 3]) -> [u8; 3] {
    let avg = (r as f32 + g as f32 + b as f32) / 3.0;
    let f = |v: u8| (238.0 + (v as f32 - avg) * 1.2).round().clamp(0.0, 255.0) as u8;
    [f(r), f(g), f(b)]
}

/// A surface tone: `dark` / `light` are steps from the theme's dark / light base
fn tone(dark: i16, light: i16) -> Color32 {
    let base = theme_rgb();
    let (d, l) = (shade(base, dark), shade(light_base(base), light));
    let k = lk();
    if k <= 0.0 { d } else if k >= 1.0 { l } else { mix(d, l, k) }
}

/// A neutral grey of the dark UI, turned over in light mode (236 text -> 19)
pub(crate) fn gray_ui(v: u8) -> Color32 {
    let k = lk();
    Color32::from_gray((v as f32 + (255.0 - 2.0 * v as f32) * k).round().clamp(0.0, 255.0) as u8)
}

/// A white wash of the dark UI (hover fills, tracks); a dark wash in light mode
pub(crate) fn wa(a: u8) -> Color32 {
    let k = lk();
    if k <= 0.0 { Color32::from_white_alpha(a) } else { mix(Color32::from_white_alpha(a), Color32::from_black_alpha(a), k) }
}
pub(crate) const DANGER: Color32 = Color32::from_rgb(232, 17, 35);

// Opaque on purpose: translucent white is blended in linear light here, so a
// faint alpha comes out several times brighter than it looks on paper, and an
// edge that lands on one pixel row instead of two turns into a pale line.
pub(crate) fn border() -> Color32 {
    tone(0x12, -26)
}

/// Floating surfaces that open (menus, dialogs, toasts): the bar tone, a hair lighter
pub(crate) fn popup() -> Color32 {
    tone(5, 16)
}

/// Soft round glow: a bell-shaped falloff built from many rings, so no edges show
pub(crate) fn glow(p: &egui::Painter, c: Pos2, radius: f32, col: Color32) {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    const N: u32 = 48; // points per ring
    const RINGS: u32 = 12;
    let mut m = Mesh::default();
    m.vertices.push(Vertex { pos: c, uv: WHITE_UV, color: col });
    for k in 1..=RINGS {
        let f = k as f32 / RINGS as f32;
        let a = (1.0 - f * f).powi(3);
        for i in 0..N {
            let ang = i as f32 / N as f32 * std::f32::consts::TAU;
            m.vertices.push(Vertex { pos: c + vec2(ang.cos(), ang.sin()) * radius * f, uv: WHITE_UV, color: col.gamma_multiply(a) });
        }
    }
    for i in 0..N {
        let j = (i + 1) % N;
        m.add_triangle(0, 1 + i, 1 + j);
    }
    for k in 0..RINGS - 1 {
        let (a0, b0) = (1 + k * N, 1 + (k + 1) * N);
        for i in 0..N {
            let j = (i + 1) % N;
            m.add_triangle(a0 + i, a0 + j, b0 + i);
            m.add_triangle(a0 + j, b0 + j, b0 + i);
        }
    }
    p.add(m);
}

/// The Aurora canvas: three slow colour clouds, laid out over `canvas` and drawn
/// with whatever clip the painter has (the canvas, or the active tab joined to it)
/// Aurora colour combos: three cloud colours each
pub(crate) const AURA_PALETTES: [(&str, [u32; 3]); 7] = [
    ("Aurora", [0x2dd4bf, 0x3b82f6, 0xa855f7]),
    ("Sunset", [0xf97316, 0xec4899, 0x8b5cf6]),
    ("Ocean", [0x22d3ee, 0x2563eb, 0x312e81]),
    ("Forest", [0x84cc16, 0x10b981, 0x0d9488]),
    ("Ember", [0xef4444, 0xf97316, 0xfacc15]),
    ("Candy", [0xf472b6, 0xa78bfa, 0x38bdf8]),
    ("Mono", [0xe5e7eb, 0x9ca3af, 0x6b7280]),
];
/// Aurora motion presets: name, speed, sway, breathing
pub(crate) const AURA_MOTIONS: [(&str, f32, f32, f32); 5] = [("Drift", 1.0, 1.0, 0.0), ("Calm", 0.45, 0.6, 0.0), ("Lively", 2.4, 1.15, 0.0), ("Pulse", 0.8, 0.7, 0.18), ("Still", 0.0, 1.0, 0.0)];
pub(crate) static AURA_PAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) static AURA_MOT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) fn rgb32(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub(crate) fn aura_blobs(canvas: Rect, now: f64) -> [(Color32, Pos2, f32); 3] {
    let (_, cols) = AURA_PALETTES[AURA_PAL.load(std::sync::atomic::Ordering::Relaxed) % AURA_PALETTES.len()];
    let (_, speed, sway, breathe) = AURA_MOTIONS[AURA_MOT.load(std::sync::atomic::Ordering::Relaxed) % AURA_MOTIONS.len()];
    // Still freezes at a pleasant pose instead of t = 0
    let t = if speed == 0.0 { 40.0 } else { now as f32 * speed };
    let size = canvas.width().max(canvas.height());
    let spec = [(rgb32(cols[0]), 0.0_f32, 0.60_f32), (rgb32(cols[1]), 2.1, 0.55), (rgb32(cols[2]), 4.2, 0.50)];
    let mut out = [(Color32::BLACK, Pos2::ZERO, 1.0); 3];
    for (i, (col, ph, rad)) in spec.iter().enumerate() {
        let sp = 0.05 + i as f32 * 0.013;
        let c = pos2(
            canvas.center().x + canvas.width() * 0.34 * sway * (t * sp + ph).sin(),
            canvas.center().y + canvas.height() * 0.30 * sway * (t * sp * 1.3 + ph * 1.7).cos(),
        );
        let br = 1.0 + breathe * (now as f32 * 0.9 + i as f32 * 2.1).sin();
        out[i] = (*col, c, size * rad * br);
    }
    out
}

/// Popup background: always fully opaque
pub(crate) fn glass(p: &egui::Painter, r: Rect, rad: f32, a: f32) {
    p.rect_filled(r, rad, popup().gamma_multiply(a));
}

/// Draws a number odometer-style: each digit rolls up/down to its new value on its own.
/// `origin` is the text's top-left before rotation by `angle` (radians); the painter's clip
/// should be the pill so digits roll in and out of view.
pub(crate) fn roll_text(p: &egui::Painter, id: egui::Id, origin: Pos2, text: &str, font: &FontId, col: Color32, angle: f32, dur: f32) {
    let ctx = p.ctx();
    let (sn, cs) = angle.sin_cos();
    let at = |lx: f32, ly: f32| origin + vec2(lx * cs - ly * sn, lx * sn + ly * cs);
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut lx = 0.0;
    for (i, ch) in chars.iter().enumerate() {
        let g = p.layout_no_wrap(ch.to_string(), font.clone(), col);
        let (cw, lh) = (g.size().x, g.size().y);
        if let Some(d) = ch.to_digit(10) {
            // Keyed from the right so units stay units when the digit count changes
            let f = ctx.animate_value_with_time(id.with(n - 1 - i), d as f32, dur);
            let lo = f.floor();
            let fr = f - lo;
            for (dg, off, a) in [(lo as i32, -fr, 1.0 - fr), (lo as i32 + 1, 1.0 - fr, fr)] {
                if a < 0.01 {
                    continue;
                }
                let gd = p.layout_no_wrap(dg.rem_euclid(10).to_string(), font.clone(), col);
                let x = lx + (cw - gd.size().x) / 2.0;
                p.add(egui::epaint::TextShape::new(at(x, off * lh), gd, col).with_override_text_color(col.gamma_multiply(a.clamp(0.0, 1.0))).with_angle(angle));
            }
        } else {
            p.add(egui::epaint::TextShape::new(at(lx, 0.0), g, col).with_override_text_color(col).with_angle(angle));
        }
        lx += cw;
    }
}

pub(crate) fn paint_aura(p: &egui::Painter, canvas: Rect, now: f64) {
    for (col, c, r) in aura_blobs(canvas, now) {
        glow(p, c, r, col.gamma_multiply(0.13));
    }
}

pub(crate) static GRADIENTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(crate) static GRAD_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) const GRAD_DIRS: [&str; 6] = ["Diagonal right", "Diagonal left", "Top to bottom", "Bottom to top", "Left to right", "Glow from centre"];

/// Gradient is switched on for the current colour (each colour remembers its own)
pub(crate) fn gradient_on() -> bool {
    let i = THEME.load(std::sync::atomic::Ordering::Relaxed);
    i != AURORA && GRADIENTS.load(std::sync::atomic::Ordering::Relaxed) & (1 << i) != 0
}

/// Gradient canvas: the colour brought out at the top-left, deepened toward the bottom-right
pub(crate) fn gradient_at(q: Pos2, canvas: Rect) -> Color32 {
    gradient_dir_at(q, canvas, GRAD_DIR.load(std::sync::atomic::Ordering::Relaxed))
}

/// Gradient colour for a given direction preset; t = 0 is the lit end
pub(crate) fn gradient_dir_at(q: Pos2, canvas: Rect, dir: usize) -> Color32 {
    let [r, g, b] = theme_rgb();
    let fx = ((q.x - canvas.min.x) / canvas.width().max(1.0)).clamp(0.0, 1.0);
    let fy = ((q.y - canvas.min.y) / canvas.height().max(1.0)).clamp(0.0, 1.0);
    let t = match dir {
        1 => (1.0 - fx) * 0.35 + fy * 0.65,
        2 => fy,
        3 => 1.0 - fy,
        4 => fx,
        5 => (((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt() / 0.7071).min(1.0),
        _ => fx * 0.35 + fy * 0.65,
    }
    .clamp(0.0, 1.0);
    let ch = |v: u8| {
        let v = v as f32;
        let top = (v * 1.9).min(255.0);
        let low = v * 0.7;
        (top + (low - top) * t) as u8
    };
    Color32::from_rgb(ch(r), ch(g), ch(b))
}

/// Fills a rect whose top corners are rounded, with a colour that varies by position
pub(crate) fn fill_rounded_top(p: &egui::Painter, rect: Rect, r: f32, colour: impl Fn(Pos2) -> Color32) {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    use std::f32::consts::PI;
    let mut pts = vec![rect.left_bottom()];
    for i in 0..=8 {
        let a = PI + PI * 0.5 * i as f32 / 8.0;
        pts.push(pos2(rect.min.x + r + r * a.cos(), rect.min.y + r + r * a.sin()));
    }
    for i in 0..=8 {
        let a = PI * 1.5 + PI * 0.5 * i as f32 / 8.0;
        pts.push(pos2(rect.max.x - r + r * a.cos(), rect.min.y + r + r * a.sin()));
    }
    pts.push(rect.right_bottom());
    let mut m = Mesh::default();
    let c = rect.center();
    m.vertices.push(Vertex { pos: c, uv: WHITE_UV, color: colour(c) });
    for q in &pts {
        m.vertices.push(Vertex { pos: *q, uv: WHITE_UV, color: colour(*q) });
    }
    let n = pts.len() as u32;
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(m);
}

pub(crate) fn aurora() -> bool {
    THEME.load(std::sync::atomic::Ordering::Relaxed) == AURORA
}

/// Re-applies the theme to egui's own widgets (menus, tooltips, text fields)
pub(crate) fn retint(ctx: &egui::Context) {
    let mut st = (*ctx.style()).clone();
    let v = &mut st.visuals;
    v.panel_fill = frame_bg();
    v.window_fill = popup();
    v.window_stroke = EStroke::new(1.0_f32, border());
    v.widgets.noninteractive.bg_stroke = EStroke::new(1.0_f32, border());
    let [r, g, b] = theme_rgb();
    v.extreme_bg_color = mix(Color32::from_rgb(r.saturating_sub(1), g.saturating_sub(1), b.saturating_sub(1)), bg(), lk());
    v.override_text_color = Some(txt());
    ctx.set_style(st);
}

pub(crate) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}


/// The canvas colour at a point, including the Aurora glow when that style is on
pub(crate) fn aura_colour_at(q: Pos2, canvas: Rect, now: f64) -> Color32 {
    if canvas.width() <= 0.0 {
        return bg();
    }
    if gradient_on() {
        return gradient_at(q, canvas);
    }
    let base = bg();
    if !aurora() {
        return base;
    }
    let mut c = [base.r() as f32, base.g() as f32, base.b() as f32];
    for (col, centre, radius) in aura_blobs(canvas, now) {
        let f = (q.distance(centre) / radius).min(1.0);
        let a = 0.13 * (1.0 - f * f).powi(3);
        let s = [col.r() as f32, col.g() as f32, col.b() as f32];
        for k in 0..3 {
            c[k] = c[k] * (1.0 - a) + s[k] * a;
        }
    }
    Color32::from_rgb(c[0] as u8, c[1] as u8, c[2] as u8)
}

pub(crate) fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

pub(crate) fn hex(s: &str) -> Color32 {
    let [r, g, b] = hex_rgb(s);
    Color32::from_rgb(r, g, b)
}
