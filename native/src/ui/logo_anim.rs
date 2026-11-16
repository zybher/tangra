#![allow(dead_code)]
//! logo_anim: idle + easter-egg animation for the Tangra logo.
//!
//! Renderer-agnostic, no dependencies. Call `update(dt)` each frame while
//! `is_animating()` is true, then draw the three triangles from `triangle_points()`.
//! When nothing is playing it costs zero: ask `seconds_until_next_event()` and
//! schedule a single wake-up timer instead of redrawing every frame.
//!
//! Logo space is 0..120 x 0..120 (same as the SVG). Scale it to your icon size.
//!
//! Behaviour:
//!   - Calm by default: the logo sits still.
//!   - Every 7..20 s a random easter egg plays (never the same one twice in a row).
//!     Every egg starts and ends at rest, so nothing ever jitters.
//!       Hop, Flip (favourites, picked most often), Peek, Nod, Shake, Spin, Jelly,
//!       Duck, Breathe, Lean, Heartbeat, Wink, and two shape morphs:
//!       Gem (the peak turns into a diamond) and Melt (it slumps and reforms).
//!   - Hover: the pieces spread apart slightly on a spring.
//!   - Click: plays a Hop (or brings the next egg sooner if one is running).
//!   - reduce_motion: eggs are disabled; hover becomes a tiny, slow spread.

use std::f32::consts::PI;

pub const COLORS: [[u8; 3]; 3] = [
    [0xDC, 0x26, 0x26], // left
    [0xF8, 0x71, 0x71], // right
    [0x7F, 0x1D, 0x1D], // inner
];

/// Home shape of each triangle, in draw order (back to front).
pub const TRIS: [[(f32, f32); 3]; 3] = [
    [(20.0, 100.0), (60.0, 20.0), (60.0, 100.0)],
    [(60.0, 20.0), (100.0, 100.0), (60.0, 100.0)],
    [(60.0, 60.0), (80.0, 100.0), (40.0, 100.0)],
];

const CENTER: (f32, f32) = (60.0, 60.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Egg {
    Hop,
    Flip,
    Peek,
    Nod,
    Shake,
    Spin,
    Jelly,
    Duck,
    Breathe,
    Lean,
    Heartbeat,
    Wink,
    MorphGem,
    MorphMelt,
}

impl Egg {
    pub const ALL: [Egg; 14] = [
        Egg::Hop, Egg::Flip, Egg::Peek, Egg::Nod, Egg::Shake, Egg::Spin, Egg::Jelly,
        Egg::Duck, Egg::Breathe, Egg::Lean, Egg::Heartbeat, Egg::Wink, Egg::MorphGem, Egg::MorphMelt,
    ];
    fn duration(self) -> f32 {
        match self {
            Egg::Hop => 0.9,
            Egg::Flip => 1.0,
            Egg::Peek => 1.0,
            Egg::Nod => 0.9,
            Egg::Shake => 0.8,
            Egg::Spin => 1.4,
            Egg::Jelly => 0.9,
            Egg::Duck => 1.3,
            Egg::Breathe => 2.2,
            Egg::Lean => 1.6,
            Egg::Heartbeat => 1.0,
            Egg::Wink => 1.0,
            Egg::MorphGem => 1.9,
            Egg::MorphMelt => 1.9,
        }
    }
    fn weight(self) -> u32 {
        match self {
            Egg::Hop | Egg::Flip => 16,
            Egg::MorphGem | Egg::MorphMelt => 6,
            _ => 8,
        }
    }
}

/// Offset applied to one piece (around its own centroid) or to the whole logo
/// (around the logo center).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub dx: f32,
    pub dy: f32,
    pub rot_deg: f32,
    pub sx: f32,
    pub sy: f32,
}

impl Pose {
    pub const IDENTITY: Pose = Pose { dx: 0.0, dy: 0.0, rot_deg: 0.0, sx: 1.0, sy: 1.0 };
}

#[derive(Clone, Copy, Debug)]
pub struct LogoPose {
    pub global: Pose,
    pub pieces: [Pose; 3],
    /// Shape morph: each vertex blends from TRIS toward `target` by `amount` (0..1).
    pub morph: Option<([[(f32, f32); 3]; 3], f32)>,
}

pub struct LogoAnim {
    time: f32,
    rng: u64,
    next_egg_at: f32,
    playing: Option<(Egg, f32)>, // (egg, start time)
    last_egg: Option<Egg>,
    hover_target: f32,
    hover: f32,
    hover_vel: f32,
    pub reduce_motion: bool,
    pub eggs_enabled: bool,
}

impl LogoAnim {
    pub fn new(seed: u64) -> Self {
        let mut a = Self {
            time: 0.0,
            rng: seed | 1,
            next_egg_at: 4.0, // first surprise shortly after launch
            playing: None,
            last_egg: None,
            hover_target: 0.0,
            hover: 0.0,
            hover_vel: 0.0,
            reduce_motion: false,
            eggs_enabled: true,
        };
        a.rand(); // warm up
        a
    }

    // ---------- input ----------

    pub fn set_hover(&mut self, hovered: bool) {
        self.hover_target = if hovered { 1.0 } else { 0.0 };
    }

    pub fn click(&mut self) {
        if self.reduce_motion {
            return;
        }
        match self.playing {
            None => self.start(Egg::Hop),
            Some(_) => self.next_egg_at = self.time + 0.4, // chain another soon
        }
    }

    /// Force a specific egg (debug menu, or a secret shortcut).
    pub fn play(&mut self, egg: Egg) {
        if !self.reduce_motion {
            self.start(egg);
        }
    }

    // ---------- time ----------

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1); // survive hitches/backgrounding
        self.time += dt;

        // Hover spring (critically-ish damped, slightly bouncy).
        let (k, c) = if self.reduce_motion { (60.0, 16.0) } else { (220.0, 18.0) };
        let acc = k * (self.hover_target - self.hover) - c * self.hover_vel;
        self.hover_vel += acc * dt;
        self.hover += self.hover_vel * dt;

        if let Some((egg, start)) = self.playing {
            if self.time - start >= egg.duration() {
                self.playing = None;
                self.schedule_next();
            }
        } else if self.eggs_enabled && !self.reduce_motion && self.time >= self.next_egg_at {
            let egg = self.pick_egg();
            self.start(egg);
        }
    }

    /// True while something is moving: redraw every frame only then.
    pub fn is_animating(&self) -> bool {
        self.playing.is_some()
            || (self.hover - self.hover_target).abs() > 1e-3
            || self.hover_vel.abs() > 1e-3
    }

    /// When idle, how long until the next scheduled egg (for a wake-up timer).
    pub fn seconds_until_next_event(&self) -> Option<f32> {
        if self.is_animating() {
            return Some(0.0);
        }
        if !self.eggs_enabled || self.reduce_motion {
            return None;
        }
        Some((self.next_egg_at - self.time).max(0.0))
    }

    // ---------- output ----------

    pub fn pose(&self) -> LogoPose {
        let mut out = LogoPose { global: Pose::IDENTITY, pieces: [Pose::IDENTITY; 3], morph: None };

        // Hover: open the pieces slightly.
        let h = if self.reduce_motion { self.hover * 0.4 } else { self.hover };
        out.pieces[0].dx -= 3.0 * h;
        out.pieces[0].rot_deg -= 3.0 * h;
        out.pieces[1].dx += 3.0 * h;
        out.pieces[1].rot_deg += 3.0 * h;
        out.pieces[2].dy += 2.0 * h;
        let s = 1.0 + 0.06 * h;
        out.pieces[2].sx *= s;
        out.pieces[2].sy *= s;

        if let Some((egg, start)) = self.playing {
            let p = ((self.time - start) / egg.duration()).clamp(0.0, 1.0);
            apply_egg(egg, p, &mut out);
        }
        out
    }

    /// The three triangles' final points in logo space (0..120), back to front.
    pub fn triangle_points(&self) -> [[(f32, f32); 3]; 3] {
        let pose = self.pose();
        let mut out = [[(0.0, 0.0); 3]; 3];
        for (i, tri) in TRIS.iter().enumerate() {
            let c = centroid(tri);
            for (j, &home) in tri.iter().enumerate() {
                let pt = match pose.morph {
                    Some((target, m)) => lerp2(home, target[i][j], m),
                    None => home,
                };
                let local = transform(pt, c, &pose.pieces[i]);
                out[i][j] = transform(local, CENTER, &pose.global);
            }
        }
        out
    }

    // ---------- internals ----------

    fn start(&mut self, egg: Egg) {
        self.playing = Some((egg, self.time));
        self.last_egg = Some(egg);
    }

    fn schedule_next(&mut self) {
        self.next_egg_at = self.time + 7.0 + self.rand_f32() * 13.0;
    }

    fn pick_egg(&mut self) -> Egg {
        let options: Vec<Egg> = Egg::ALL.iter().copied().filter(|e| Some(*e) != self.last_egg).collect();
        let total: u32 = options.iter().map(|e| e.weight()).sum();
        let mut roll = (self.rand() % total as u64) as u32;
        for e in &options {
            if roll < e.weight() {
                return *e;
            }
            roll -= e.weight();
        }
        options[0]
    }

    fn rand(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn rand_f32(&mut self) -> f32 {
        (self.rand() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Gem: the peak becomes a faceted diamond.
const GEM: [[(f32, f32); 3]; 3] = [
    [(20.0, 60.0), (60.0, 20.0), (60.0, 100.0)],
    [(60.0, 20.0), (100.0, 60.0), (60.0, 100.0)],
    [(60.0, 50.0), (70.0, 68.0), (50.0, 68.0)],
];
/// Melt: the peak slumps and spreads like a drip.
const MELT: [[(f32, f32); 3]; 3] = [
    [(12.0, 100.0), (60.0, 74.0), (60.0, 100.0)],
    [(60.0, 74.0), (108.0, 100.0), (60.0, 100.0)],
    [(60.0, 84.0), (78.0, 100.0), (42.0, 100.0)],
];

/// A smooth hump between a and b (0 outside), starting and ending at rest.
fn bump(p: f32, a: f32, b: f32) -> f32 {
    if p <= a || p >= b { 0.0 } else { (PI * (p - a) / (b - a)).sin().powi(2) }
}
/// Ease in, hold, ease out (for morphs and holds).
fn hold(p: f32, rise: f32, fall_start: f32) -> f32 {
    smooth((p / rise).min(1.0)) * (1.0 - smooth(((p - fall_start) / (1.0 - fall_start)).clamp(0.0, 1.0)))
}
/// Keep the base of the logo planted while scaling vertically.
fn plant_base(g: &mut Pose) {
    g.dy += 40.0 * (1.0 - g.sy);
}
/// Rotate around the bottom-center of the logo (like leaning on its base).
fn lean_on_base(g: &mut Pose, deg: f32) {
    let a = deg.to_radians();
    g.rot_deg += deg;
    g.dx += 40.0 * a.sin();
    g.dy += 40.0 - 40.0 * a.cos();
}

fn apply_egg(egg: Egg, p: f32, out: &mut LogoPose) {
    // 0 -> 1 -> 0, starting and ending at rest (sin^2), so nothing kicks off abruptly.
    let env = (PI * p).sin().powi(2);
    let g = &mut out.global;
    match egg {
        Egg::Hop => {
            // Every phase starts exactly where the previous one ended (no jumps).
            if p < 0.18 {
                let q = smooth(p / 0.18);
                g.sy *= 1.0 - 0.16 * q;
                g.sx *= 1.0 + 0.10 * q;
                g.dy += 6.0 * q;
            } else if p < 0.68 {
                let q = (p - 0.18) / 0.5;
                let release = 1.0 - smooth((q / 0.35).min(1.0));
                let air = (PI * smooth(q)).sin();
                g.sy *= 1.0 - 0.16 * release + 0.08 * air;
                g.sx *= 1.0 + 0.10 * release - 0.05 * air;
                g.dy += 6.0 * release - 14.0 * air;
                g.rot_deg += 6.0 * air;
            } else {
                let q = (p - 0.68) / 0.32;
                let w = 2.0 * (PI * q).sin().powi(2) * (1.0 - q);
                g.sy *= 1.0 - 0.10 * w;
                g.sx *= 1.0 + 0.07 * w;
                g.dy += 4.0 * w;
            }
        }
        Egg::Flip => {
            let a = 2.0 * PI * ease_in_out_cubic(p);
            g.sx *= a.cos();
            g.dy -= 4.0 * env;
            g.sy *= 1.0 + 0.04 * env;
        }
        Egg::Peek => {
            let up = if p < 0.38 {
                ease_out_back(smooth(p / 0.38))
            } else {
                1.0 - ease_out_back(smooth((p - 0.38) / 0.62))
            };
            out.pieces[2].dy -= 22.0 * up;
            out.pieces[2].rot_deg += 8.0 * (2.0 * PI * p).sin() * env;
            out.pieces[0].sy *= 1.0 - 0.04 * env;
            out.pieces[1].sy *= 1.0 - 0.04 * env;
        }
        Egg::Nod => {
            // Two little "yes" nods.
            let v = (2.0 * PI * p).sin().powi(2);
            g.sy *= 1.0 - 0.09 * v;
            plant_base(g);
            g.rot_deg += 3.0 * v;
            out.pieces[2].dy += 2.0 * v;
        }
        Egg::Shake => {
            // A playful "no".
            g.rot_deg += 11.0 * (6.0 * PI * p).sin() * (PI * p).sin().powi(2);
        }
        Egg::Spin => {
            // Wind back, spin once, overshoot a touch, settle.
            g.rot_deg += 360.0 * ease_in_out_cubic(p) - 28.0 * env * (PI * p).cos();
            let s = 1.0 - 0.12 * env;
            g.sx *= s;
            g.sy *= s;
        }
        Egg::Jelly => {
            let w = (6.0 * PI * p).sin() * (PI * p).sin().powi(2) * (1.0 - 0.5 * p);
            g.sx *= 1.0 + 0.14 * w;
            g.sy *= 1.0 - 0.14 * w;
            plant_base(g);
        }
        Egg::Duck => {
            // Hides by squashing flat, peeks back up.
            let d = hold(p, 0.3, 0.62);
            g.sy *= 1.0 - 0.62 * d;
            g.sx *= 1.0 + 0.14 * d;
            plant_base(g);
            g.dy -= 3.0 * bump(p, 0.62, 1.0); // tiny pop on the way back
        }
        Egg::Breathe => {
            let s = 1.0 + 0.07 * env;
            g.sx *= s;
            g.sy *= s;
            out.pieces[0].dx -= 2.5 * env;
            out.pieces[1].dx += 2.5 * env;
            out.pieces[2].dy += 1.5 * env;
        }
        Egg::Lean => {
            // Looks right, then left, standing on its base.
            let deg = 13.0 * (2.0 * PI * p).sin() * (PI * p).sin();
            lean_on_base(g, deg);
        }
        Egg::Heartbeat => {
            let s = 1.0 + 0.13 * bump(p, 0.08, 0.36) + 0.09 * bump(p, 0.40, 0.66);
            g.sx *= s;
            g.sy *= s;
        }
        Egg::Wink => {
            let w = bump(p, 0.1, 0.42) + bump(p, 0.52, 0.86);
            out.pieces[2].sy *= 1.0 - 0.8 * w;
            out.pieces[2].sx *= 1.0 + 0.1 * w;
        }
        Egg::MorphGem => {
            let m = hold(p, 0.35, 0.68);
            out.morph = Some((GEM, m));
            g.rot_deg += 4.0 * bump(p, 0.2, 0.8);
        }
        Egg::MorphMelt => {
            let m = hold(p, 0.4, 0.62);
            out.morph = Some((MELT, m));
            g.sx *= 1.0 + 0.04 * bump(p, 0.6, 1.0);
        }
    }
}

fn lerp2(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

// ---------- geometry ----------

fn centroid(t: &[(f32, f32); 3]) -> (f32, f32) {
    ((t[0].0 + t[1].0 + t[2].0) / 3.0, (t[0].1 + t[1].1 + t[2].1) / 3.0)
}

/// Scale, then rotate, around `pivot`, then translate.
fn transform(p: (f32, f32), pivot: (f32, f32), pose: &Pose) -> (f32, f32) {
    let x = (p.0 - pivot.0) * pose.sx;
    let y = (p.1 - pivot.1) * pose.sy;
    let (s, c) = pose.rot_deg.to_radians().sin_cos();
    (pivot.0 + x * c - y * s + pose.dx, pivot.1 + x * s + y * c + pose.dy)
}

// ---------- easing ----------

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_is_exact_home_shape() {
        let a = LogoAnim::new(42);
        assert_eq!(a.triangle_points(), TRIS);
        assert!(!a.is_animating());
    }

    #[test]
    fn every_egg_returns_home() {
        for egg in Egg::ALL {
            let mut a = LogoAnim::new(7);
            a.play(egg);
            for _ in 0..400 {
                a.update(1.0 / 120.0);
            }
            let pts = a.triangle_points();
            for i in 0..3 {
                for j in 0..3 {
                    assert!((pts[i][j].0 - TRIS[i][j].0).abs() < 0.05, "{egg:?}");
                    assert!((pts[i][j].1 - TRIS[i][j].1).abs() < 0.05, "{egg:?}");
                }
            }
            assert!(!a.is_animating(), "{egg:?} still animating");
        }
    }

    #[test]
    fn eggs_fire_on_their_own_and_dont_repeat() {
        let mut a = LogoAnim::new(99);
        let mut seen = Vec::new();
        for _ in 0..(120 * 600) {
            a.update(1.0 / 120.0);
            if let Some((e, start)) = a.playing {
                if seen.last() != Some(&(e, start)) {
                    seen.push((e, start));
                }
            }
        }
        assert!(seen.len() > 10);
        for w in seen.windows(2) {
            assert_ne!(w[0].0, w[1].0);
        }
    }

    /// Worst "jerk spike" of an egg: the third difference of every point's
    /// position between frames. Smooth motion (even fast spins) keeps it tiny;
    /// a jump, or a phase that starts with the wrong position or speed, spikes.
    fn worst_jerk(egg: Egg) -> f32 {
        let mut a = LogoAnim::new(3);
        a.play(egg);
        let mut h = vec![a.triangle_points()];
        for _ in 0..300 {
            a.update(1.0 / 120.0);
            h.push(a.triangle_points());
        }
        let mut worst = 0.0f32;
        for f in 3..h.len() {
            for i in 0..3 {
                for j in 0..3 {
                    let d = |k: usize| h[f - k][i][j];
                    let jx = d(0).0 - 3.0 * d(1).0 + 3.0 * d(2).0 - d(3).0;
                    let jy = d(0).1 - 3.0 * d(1).1 + 3.0 * d(2).1 - d(3).1;
                    worst = worst.max((jx * jx + jy * jy).sqrt());
                }
            }
        }
        worst
    }

    #[test]
    fn no_jitter() {
        for egg in Egg::ALL {
            let w = worst_jerk(egg);
            assert!(w < 0.3, "{egg:?} jerk spike {w:.3}");
        }
    }

    #[test]
    fn reduce_motion_blocks_eggs() {
        let mut a = LogoAnim::new(1);
        a.reduce_motion = true;
        for _ in 0..(120 * 60) {
            a.update(1.0 / 120.0);
        }
        assert!(a.playing.is_none());
        assert_eq!(a.seconds_until_next_event(), None);
    }
}
