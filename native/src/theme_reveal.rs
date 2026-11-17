//! Immersive dark <-> light theme transition.
//!
//! Renderer-agnostic: call [`ThemeToggle::update`] every frame, then
//! [`ThemeToggle::frame`] and paint what it describes:
//!
//! 1. Paint the whole UI with `frame.base`.
//! 2. If `frame.reveal` is `Some`, paint the UI again with `reveal.theme`,
//!    clipped to the circle (`center`, `radius`). The rim glow in the overlay
//!    hides the hard clip edge, so a plain circle clip is enough.
//! 3. Paint `frame.overlay` on top, in order (sun rays, bloom, stars, rim, sparkles, ripples).
//! 4. Paint the toggle button at `frame.button_scale` with `frame.icon_svg`
//!    (inner elements, 24x24, stroke 1.75, same format as the icon pack) in `frame.icon_color`.
//!
//! If you can't render the UI twice, ignore `reveal` and paint with
//! `frame.blended` (all tokens mixed in OKLab) - you still get the overlay.
//! Std-only, no dependencies.
//!
//! Tangra: used as-is by ui/theme_toggle.rs (which freezes the old look with a screenshot
//! instead of painting the UI twice); `poke` and `start` were added for that.
#![allow(dead_code)]

use std::f32::consts::{PI, TAU};
use std::fmt::Write;

// ---------------------------------------------------------------- math

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn dist(self, o: Vec2) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
    fn mul(self, k: f32) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
    fn polar(a: f32, r: f32) -> Vec2 {
        Vec2::new(a.cos() * r, a.sin() * r)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
/// Maps `t` from `[a, b]` to `[0, 1]`, clamped.
fn remap(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}
fn smoothstep(a: f32, b: f32, t: f32) -> f32 {
    let x = remap(t, a, b);
    x * x * (3.0 - 2.0 * x)
}
fn ease_in_out_cubic(x: f32) -> f32 {
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
    }
}
fn ease_out_cubic(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}
fn ease_out_back(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}
/// Deterministic hash -> [0, 1).
fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

// ---------------------------------------------------------------- color

/// sRGB color, components 0..1, straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn hex(h: u32) -> Self {
        Self {
            r: ((h >> 16) & 255) as f32 / 255.0,
            g: ((h >> 8) & 255) as f32 / 255.0,
            b: (h & 255) as f32 / 255.0,
            a: 1.0,
        }
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a: a.clamp(0.0, 1.0), ..self }
    }
    pub fn to_css(self) -> String {
        format!(
            "rgba({},{},{},{:.3})",
            (self.r.clamp(0.0, 1.0) * 255.0).round(),
            (self.g.clamp(0.0, 1.0) * 255.0).round(),
            (self.b.clamp(0.0, 1.0) * 255.0).round(),
            self.a
        )
    }
    /// Perceptual mix in OKLab: no muddy grey midpoint between dark and light.
    pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
        let (la, lb) = (to_oklab(a), to_oklab(b));
        let m = [lerp(la[0], lb[0], t), lerp(la[1], lb[1], t), lerp(la[2], lb[2], t)];
        let mut c = from_oklab(m);
        c.a = lerp(a.a, b.a, t);
        c
    }
}

fn to_lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}
fn to_oklab(c: Rgba) -> [f32; 3] {
    let (r, g, b) = (to_lin(c.r), to_lin(c.g), to_lin(c.b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}
fn from_oklab(c: [f32; 3]) -> Rgba {
    let l = (c[0] + 0.396_337_78 * c[1] + 0.215_803_76 * c[2]).powi(3);
    let m = (c[0] - 0.105_561_346 * c[1] - 0.063_854_17 * c[2]).powi(3);
    let s = (c[0] - 0.089_484_18 * c[1] - 1.291_485_5 * c[2]).powi(3);
    Rgba {
        r: to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        g: to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        b: to_srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        a: 1.0,
    }
}

// ---------------------------------------------------------------- theme

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

impl Mode {
    pub fn other(self) -> Mode {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
        }
    }
}

/// Swap these for your own tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub canvas: Rgba,
    pub grid: Rgba,
    pub surface: Rgba,
    pub surface_alt: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
}

impl Theme {
    pub const DARK: Theme = Theme {
        canvas: Rgba::hex(0x121317),
        grid: Rgba::hex(0x24262C),
        surface: Rgba::hex(0x1C1D22),
        surface_alt: Rgba::hex(0x27282E),
        border: Rgba::hex(0x2F3138),
        text: Rgba::hex(0xECECEF),
        text_muted: Rgba::hex(0x8B8D98),
        accent: Rgba::hex(0x7C9CFF),
    };
    pub const LIGHT: Theme = Theme {
        canvas: Rgba::hex(0xF4F3EF),
        grid: Rgba::hex(0xDDDBD4),
        surface: Rgba::hex(0xFFFFFF),
        surface_alt: Rgba::hex(0xF0EEE9),
        border: Rgba::hex(0xE3E1DB),
        text: Rgba::hex(0x1D1D1F),
        text_muted: Rgba::hex(0x6E6E73),
        accent: Rgba::hex(0x3D63E8),
    };
    pub fn of(mode: Mode) -> Theme {
        match mode {
            Mode::Dark => Theme::DARK,
            Mode::Light => Theme::LIGHT,
        }
    }
    pub fn mix(a: &Theme, b: &Theme, t: f32) -> Theme {
        let m = |x, y| Rgba::mix(x, y, t);
        Theme {
            canvas: m(a.canvas, b.canvas),
            grid: m(a.grid, b.grid),
            surface: m(a.surface, b.surface),
            surface_alt: m(a.surface_alt, b.surface_alt),
            border: m(a.border, b.border),
            text: m(a.text, b.text),
            text_muted: m(a.text_muted, b.text_muted),
            accent: m(a.accent, b.accent),
        }
    }
}

// ---------------------------------------------------------------- output

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reveal {
    pub center: Vec2,
    pub radius: f32,
    pub theme: Theme,
}

/// Overlay primitives, in screen pixels. Paint filled unless noted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prim {
    Circle { c: Vec2, r: f32, color: Rgba },
    /// Stroked circle, no fill.
    Ring { c: Vec2, r: f32, width: f32, color: Rgba },
    /// Pie slice from `c`, angles in radians (screen space, y down).
    Wedge { c: Vec2, r: f32, a0: f32, a1: f32, color: Rgba },
    /// Soft 4-point star glint.
    Sparkle { c: Vec2, size: f32, rot: f32, color: Rgba },
}

impl Prim {
    /// Full SVG element, handy if your renderer eats SVG.
    pub fn to_svg(&self) -> String {
        match *self {
            Prim::Circle { c, r, color } => format!(
                r#"<circle cx="{}" cy="{}" r="{}" fill="{}"/>"#,
                f(c.x), f(c.y), f(r), color.to_css()
            ),
            Prim::Ring { c, r, width, color } => format!(
                r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                f(c.x), f(c.y), f(r), color.to_css(), f(width)
            ),
            Prim::Wedge { color, .. } | Prim::Sparkle { color, .. } => {
                format!(r#"<path d="{}" fill="{}"/>"#, self.path_d(), color.to_css())
            }
        }
    }

    /// Path data for Wedge / Sparkle (empty for circles).
    pub fn path_d(&self) -> String {
        match *self {
            Prim::Wedge { c, r, a0, a1, .. } => {
                let p0 = c.add(Vec2::polar(a0, r));
                let p1 = c.add(Vec2::polar(a1, r));
                format!(
                    "M{} {}L{} {}A{} {} 0 0 1 {} {}Z",
                    f(c.x), f(c.y), f(p0.x), f(p0.y), f(r), f(r), f(p1.x), f(p1.y)
                )
            }
            Prim::Sparkle { c, size, rot, .. } => {
                let mut d = String::new();
                for k in 0..4 {
                    let a = rot + k as f32 * PI / 2.0;
                    let tip = c.add(Vec2::polar(a, size));
                    let ctrl = c.add(Vec2::polar(a + PI / 4.0, size * 0.14));
                    let next = c.add(Vec2::polar(a + PI / 2.0, size));
                    if k == 0 {
                        let _ = write!(d, "M{} {}", f(tip.x), f(tip.y));
                    }
                    let _ = write!(d, "Q{} {} {} {}", f(ctrl.x), f(ctrl.y), f(next.x), f(next.y));
                }
                d.push('Z');
                d
            }
            _ => String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    /// Paint the whole UI with this.
    pub base: Theme,
    /// Then paint the UI again with `reveal.theme`, clipped to the circle.
    pub reveal: Option<Reveal>,
    /// Single-pass fallback (and the right colors for anything drawn once, like the cursor).
    pub blended: Theme,
    pub overlay: Vec<Prim>,
    pub button_scale: f32,
    /// Toggle icon, inner elements for the 24x24 stroke-1.75 pack format.
    pub icon_svg: String,
    pub icon_color: Rgba,
    /// 0 = moon, 1 = sun.
    pub sun_amount: f32,
}

// ---------------------------------------------------------------- animator

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Seconds for the full reveal.
    pub duration: f32,
    /// Width of the glowing rim at the reveal edge, px.
    pub feather: f32,
    pub star_count: u32,
    pub ray_count: u32,
    /// Respect the OS "reduce motion" setting: quick crossfade, no overlay.
    pub reduced_motion: bool,
    /// Seconds the sun rays and bloom keep fading out after the reveal ends.
    pub glow_tail: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self { duration: 1.0, feather: 26.0, star_count: 110, ray_count: 14, reduced_motion: false, glow_tail: 1.5 }
    }
}

#[derive(Clone, Copy, Debug)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn step(&mut self, dt: f32, target: f32, stiffness: f32, damping: f32) {
        let n = (dt / (1.0 / 240.0)).ceil().max(1.0) as u32;
        let h = dt / n as f32;
        for _ in 0..n {
            let a = -stiffness * (self.x - target) - damping * self.v;
            self.v += a * h;
            self.x += self.v * h;
        }
    }
}

pub struct ThemeToggle {
    pub config: Config,
    from: Mode,
    to: Mode,
    /// Linear progress 0..1 of from -> to.
    t: f32,
    /// +1 forward, -1 reversing (interrupted), 0 idle.
    dir: f32,
    origin: Vec2,
    press: Spring,
    clock: f32,
    /// Seconds since the reveal started, and (origin, to light) while the glow still shows
    age: f32,
    fx: Option<(Vec2, bool)>,
}

impl ThemeToggle {
    pub fn new(mode: Mode, config: Config) -> Self {
        Self {
            config,
            from: mode,
            to: mode,
            t: 0.0,
            dir: 0.0,
            origin: Vec2::default(),
            press: Spring { x: 1.0, v: 0.0 },
            clock: 0.0,
            age: 0.0,
            fx: None,
        }
    }

    /// The mode the UI is heading to (use this for persisting the setting).
    pub fn mode(&self) -> Mode {
        if self.dir > 0.0 { self.to } else { self.from }
    }

    pub fn is_animating(&self) -> bool {
        self.dir != 0.0 || self.fx.is_some() || (self.press.x - 1.0).abs() > 0.001 || self.press.v.abs() > 0.01
    }

    /// Call on click. `origin` = centre of the toggle button in screen px.
    /// Clicking again mid-animation smoothly reverses it.
    pub fn toggle(&mut self, origin: Vec2) {
        self.press.v -= 7.5; // squash, then the spring bounces it back
        if self.dir != 0.0 {
            self.dir = -self.dir;
            return;
        }
        self.to = self.from.other();
        self.t = 0.0;
        self.dir = 1.0;
        self.origin = origin;
    }

    /// Just the press squash (the reveal starts once the old look is captured)
    pub fn poke(&mut self) {
        self.press.v -= 7.5;
    }

    /// Starts the reveal toward the other mode without another squash
    pub fn start(&mut self, origin: Vec2) {
        if self.dir != 0.0 {
            return;
        }
        self.to = self.from.other();
        self.t = 0.0;
        self.dir = 1.0;
        self.origin = origin;
        self.age = 0.0;
        self.fx = Some((origin, self.to == Mode::Light));
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.min(0.1); // don't jump after a hitch
        self.clock += dt;
        self.press.step(dt, 1.0, 520.0, 17.0);
        if self.fx.is_some() {
            self.age += dt;
            if self.age > self.config.duration + self.config.glow_tail {
                self.fx = None;
            }
        }
        if self.dir == 0.0 {
            return;
        }
        let dur = if self.config.reduced_motion { 0.28 } else { self.config.duration };
        self.t += self.dir * dt / dur;
        if self.t >= 1.0 {
            self.from = self.to;
            self.t = 0.0;
            self.dir = 0.0;
        } else if self.t <= 0.0 {
            self.to = self.from;
            self.t = 0.0;
            self.dir = 0.0;
        }
    }

    pub fn frame(&self, viewport: Vec2) -> Frame {
        let (from_th, to_th) = (Theme::of(self.from), Theme::of(self.to));
        let animating = self.dir != 0.0;
        let t = self.t;
        let to_light = self.to == Mode::Light;

        let blended = Theme::mix(&from_th, &to_th, ease_in_out_cubic(t));
        let sun = if !animating {
            if self.from == Mode::Light { 1.0 } else { 0.0 }
        } else {
            let s = ease_in_out_cubic(remap(t, 0.0, 0.62));
            if to_light { s } else { 1.0 - s }
        };

        let mut frame = Frame {
            base: from_th,
            reveal: None,
            blended,
            overlay: Vec::new(),
            button_scale: self.press.x,
            icon_svg: toggle_icon(sun),
            icon_color: blended.text,
            sun_amount: sun,
        };
        if !animating || self.config.reduced_motion {
            frame.base = blended;
            self.lingering(&mut frame.overlay, viewport);
            return frame;
        }

        let o = self.origin;
        let feather = self.config.feather;
        let corners = [Vec2::new(0.0, 0.0), Vec2::new(viewport.x, 0.0), Vec2::new(0.0, viewport.y), viewport];
        let max_r = corners.iter().map(|c| c.dist(o)).fold(0.0, f32::max) + feather;
        let radius = max_r * ease_in_out_cubic(remap(t, 0.03, 1.0));
        frame.reveal = Some(Reveal { center: o, radius, theme: to_th });

        // Overlay fades in fast and out at the end, so nothing pops.
        let env = smoothstep(0.0, 0.08, t) * (1.0 - smoothstep(0.8, 1.0, t));
        let rim = if to_light { Rgba::hex(0xFFD493) } else { Rgba::hex(0x8DAEFF) };
        let glint = if to_light { Rgba::hex(0xFFF4DC) } else { Rgba::hex(0xE4ECFF) };
        let ov = &mut frame.overlay;

        // 1-2. Sun rays and bloom (they outlive the reveal, see `lingering`)
        self.lingering(ov, viewport);

        // 3. Stars live in the dark half and are swept away / switched on by the edge.
        let mut glints = Vec::new();
        for i in 0..self.config.star_count {
            let p = Vec2::new(hash(i, 1) * viewport.x, hash(i, 2) * viewport.y);
            let size = 0.6 + 1.5 * hash(i, 3).powi(2);
            let tw = 0.55 + 0.45 * (self.clock * (1.3 + 2.2 * hash(i, 5)) + hash(i, 6) * TAU).sin();
            let e = radius - p.dist(o); // > 0 inside the reveal
            let in_dark = if to_light {
                smoothstep(0.0, feather, -e)
            } else {
                smoothstep(0.0, 160.0, e) // appear a beat behind the front
            };
            let a = 0.85 * tw * in_dark * env;
            if a > 0.01 {
                ov.push(Prim::Circle { c: p, r: size, color: Rgba::hex(0xE8EEFF).with_alpha(a) });
            }
            if hash(i, 4) > 0.5 {
                let k = (-(e / (feather * 0.9)).powi(2)).exp() * env;
                if k > 0.03 {
                    glints.push(Prim::Sparkle {
                        c: p,
                        size: (4.0 + 4.0 * size) * k,
                        rot: hash(i, 10) * PI + t * 2.5,
                        color: glint.with_alpha(0.95 * k),
                    });
                }
            }
        }

        // 4. Rim: layered rings fake a soft glowing edge over the hard clip.
        for (w, a) in [(1.7, 0.10), (0.85, 0.20), (0.32, 0.55)] {
            ov.push(Prim::Ring { c: o, r: radius, width: feather * w, color: rim.with_alpha(a * env) });
        }
        ov.extend(glints);

        // 5. Two quick ripples off the button so the click feels instant.
        for (delay, strength) in [(0.0, 0.7), (0.07, 0.4)] {
            let tr = remap(t, delay, delay + 0.38);
            if tr > 0.0 && tr < 1.0 {
                ov.push(Prim::Ring {
                    c: o,
                    r: 16.0 + 80.0 * ease_out_cubic(tr),
                    width: 0.6 + 2.2 * (1.0 - tr),
                    color: rim.with_alpha(strength * (1.0 - tr)),
                });
            }
        }
        frame
    }
}

impl ThemeToggle {
    /// Sun rays (going light) and the bloom round the button. They come in with the reveal
    /// and fade out smoothly over `glow_tail` after it, instead of ending with it.
    fn lingering(&self, ov: &mut Vec<Prim>, viewport: Vec2) {
        let Some((o, to_light)) = self.fx else { return };
        if self.config.reduced_motion {
            return;
        }
        let dur = self.config.duration;
        let s = (self.age / (dur + self.config.glow_tail)).clamp(0.0, 1.0);
        let env = smoothstep(0.0, 0.06, s) * (1.0 - smoothstep(0.25, 1.0, s));
        if env <= 0.001 {
            return;
        }
        let corners = [Vec2::new(0.0, 0.0), Vec2::new(viewport.x, 0.0), Vec2::new(0.0, viewport.y), viewport];
        let max_r = corners.iter().map(|c| c.dist(o)).fold(0.0, f32::max) + self.config.feather;
        // follows the reveal's front while it grows, then holds at full size
        let tr = (self.age / dur).min(1.0);
        let radius = max_r * ease_in_out_cubic(remap(tr, 0.03, 1.0));
        let rim = if to_light { Rgba::hex(0xFFD493) } else { Rgba::hex(0x8DAEFF) };
        if to_light {
            let n = self.config.ray_count;
            for i in 0..n {
                let a = i as f32 * TAU / n as f32 + hash(i, 7) * 0.3 + self.age / dur * 0.5;
                let w = 0.018 + 0.022 * hash(i, 8);
                let r = radius * (1.12 + 0.45 * hash(i, 9));
                for (wm, alpha) in [(1.9, 0.045), (0.7, 0.085)] {
                    ov.push(Prim::Wedge { c: o, r, a0: a - w * wm, a1: a + w * wm, color: Rgba::hex(0xFFE9C2).with_alpha(alpha * env) });
                }
            }
        }
        for k in 0..3 {
            let kf = k as f32;
            ov.push(Prim::Circle { c: o, r: 18.0 + radius * (0.06 + 0.07 * kf), color: rim.with_alpha(0.11 * env / (1.0 + kf)) });
        }
    }
}

// ---------------------------------------------------------------- toggle icon

fn f(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Moon (0) -> sun (1) icon as inner SVG elements in the pack format
/// (24x24, centre 12,12, meant for stroke 1.75 round caps/joins).
/// Only `<path>` with M L A Z, so it goes through the same parser as the pack.
pub fn toggle_icon(sun: f32) -> String {
    let s = sun.clamp(0.0, 1.0);
    let c = Vec2::new(12.0, 12.0);
    let r1 = lerp(7.5, 4.25, s);
    let r2 = 6.5;
    // The "bite" slides out to the top-right, opening the crescent into a full disc.
    let c2 = c.add(Vec2::new(0.707_1, -0.707_1).mul(lerp(6.5, 13.5, s)));
    let mut out = format!(r#"<path d="{}"/>"#, crescent(c, r1, c2, r2));

    // Rays grow in one after another with a little overshoot while the sun turns.
    let rot = -(1.0 - s) * PI / 3.0;
    let mut d = String::new();
    for i in 0..8 {
        let start = 0.3 + i as f32 * 0.05;
        let g = ease_out_back(remap(s, start, start + 0.35));
        if g <= 0.01 {
            continue;
        }
        let a = rot + i as f32 * TAU / 8.0 - PI / 2.0;
        let p0 = c.add(Vec2::polar(a, 7.0));
        let p1 = c.add(Vec2::polar(a, 7.0 + 2.5 * g));
        let _ = write!(d, "M{} {}L{} {}", f(p0.x), f(p0.y), f(p1.x), f(p1.y));
    }
    // Two tiny stars beside the moon, fading out as it turns into the sun.
    let m = 1.0 - smoothstep(0.0, 0.35, s);
    if m > 0.02 {
        for (x, y, k) in [(18.25, 5.75, 1.6), (20.5, 10.25, 1.0)] {
            let r = k * m;
            let _ = write!(d, "M{} {}L{} {}M{} {}L{} {}", f(x), f(y - r), f(x), f(y + r), f(x - r), f(y), f(x + r), f(y));
        }
    }
    if !d.is_empty() {
        let _ = write!(out, r#"<path d="{}"/>"#, d);
    }
    out
}

/// Outline of disc (c1, r1) minus disc (c2, r2).
fn crescent(c1: Vec2, r1: f32, c2: Vec2, r2: f32) -> String {
    let d = c1.dist(c2);
    if d >= r1 + r2 - 1e-3 {
        return format!(
            "M{} {}A{r} {r} 0 1 0 {} {}A{r} {r} 0 1 0 {} {}Z",
            f(c1.x - r1), f(c1.y), f(c1.x + r1), f(c1.y), f(c1.x - r1), f(c1.y),
            r = f(r1)
        );
    }
    let a = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let u = c2.sub(c1).mul(1.0 / d);
    let perp = Vec2::new(-u.y, u.x);
    let base = c1.add(u.mul(a));
    let p1 = base.add(perp.mul(h));
    let p2 = base.sub(perp.mul(h));
    let (l1, s1) = arc_flags(c1, r1, p1, p2, |m| m.dist(c2) > r2);
    let (l2, s2) = arc_flags(c2, r2, p2, p1, |m| m.dist(c1) < r1);
    format!(
        "M{} {}A{} {} 0 {} {} {} {}A{} {} 0 {} {} {} {}Z",
        f(p1.x), f(p1.y), f(r1), f(r1), l1, s1, f(p2.x), f(p2.y),
        f(r2), f(r2), l2, s2, f(p1.x), f(p1.y)
    )
}

/// Picks SVG arc flags so the arc from `a` to `b` goes through the side where `want` holds.
fn arc_flags(c: Vec2, r: f32, a: Vec2, b: Vec2, want: impl Fn(Vec2) -> bool) -> (u8, u8) {
    let a0 = (a.y - c.y).atan2(a.x - c.x);
    let a1 = (b.y - c.y).atan2(b.x - c.x);
    let span = (a1 - a0).rem_euclid(TAU);
    let mid = c.add(Vec2::polar(a0 + span / 2.0, r));
    if want(mid) {
        ((span > PI) as u8, 1)
    } else {
        ((TAU - span > PI) as u8, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_cycle_and_reverse() {
        let mut tg = ThemeToggle::new(Mode::Dark, Config::default());
        tg.toggle(Vec2::new(10.0, 10.0));
        for _ in 0..30 {
            tg.update(1.0 / 60.0);
        }
        tg.toggle(Vec2::new(10.0, 10.0)); // interrupt -> reverse
        assert_eq!(tg.mode(), Mode::Dark);
        for _ in 0..120 {
            tg.update(1.0 / 60.0);
        }
        assert_eq!(tg.mode(), Mode::Dark);
        assert!(tg.frame(Vec2::new(800.0, 600.0)).reveal.is_none());
    }

    #[test]
    fn oklab_roundtrip() {
        let c = Rgba::hex(0x3D63E8);
        let m = Rgba::mix(c, c, 0.5);
        assert!((m.r - c.r).abs() < 1e-3 && (m.g - c.g).abs() < 1e-3 && (m.b - c.b).abs() < 1e-3);
    }

    #[test]
    fn icon_only_uses_allowed_commands() {
        for i in 0..=20 {
            let s = toggle_icon(i as f32 / 20.0);
            let d: String = s.split("d=\"").skip(1).map(|p| p.split('"').next().unwrap()).collect();
            assert!(d.chars().all(|ch| "MLAZ0123456789.- ".contains(ch)), "{d}");
        }
    }
}
