//! The light / dark toggle in the title bar and its full-screen reveal (theme_reveal.rs).
//! A click squashes the button and asks egui for a screenshot of the current (old) look.
//! When it arrives the app switches mode at once underneath, and the screenshot is laid over
//! everything with a growing round hole cut out of it at the button, so the new look spreads
//! from there. The crate's overlay (sun rays / stars, rim glow, sparkles, ripples) is painted
//! on top. With animations off it's the usual fade instead.
use crate::theme_reveal as tr;
use crate::*;
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use std::cell::RefCell;

struct St {
    tg: tr::ThemeToggle,
    /// the old look, while the reveal runs
    snap: Option<egui::TextureHandle>,
    /// clicked, waiting for the screenshot: (button centre, mode to switch to)
    pending: Option<(Pos2, bool)>,
}

thread_local! {
    static ST: RefCell<Option<St>> = const { RefCell::new(None) };
}

fn light_on() -> bool {
    LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

fn with<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert_with(|| St {
            tg: tr::ThemeToggle::new(if light_on() { tr::Mode::Light } else { tr::Mode::Dark }, cfg()),
            snap: None,
            pending: None,
        });
        f(st)
    })
}

/// No stars: the canvas already has its dot pattern
fn cfg() -> tr::Config {
    tr::Config { star_count: 0, ..tr::Config::default() }
}

fn col(c: tr::Rgba) -> Color32 {
    // Premultiplied in gamma space: egui's from_rgba_unmultiplied does it in linear light,
    // which makes faint glows several times brighter and keeps them showing until alpha is 0
    let a = c.a.clamp(0.0, 1.0);
    let b = |v: f32| (v.clamp(0.0, 1.0) * a * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(b(c.r), b(c.g), b(c.b), (a * 255.0).round() as u8)
}

/// The toggle button. Returns the new mode (true = light) on the click that picks it.
pub(crate) fn button(ui: &mut egui::Ui, ctx: &egui::Context, r: Rect) -> Option<bool> {
    let resp = ui.allocate_rect(r, Sense::click());
    let (h, pr) = hover_fx(ctx, &resp, true);
    let p = ui.painter();
    p.rect_filled(r.shrink(1.5 * (1.0 - h) + pr), 6.0, hover_fill(h, pr));
    let (svg, scale) = with(|s| {
        let fr = s.tg.frame(tr::Vec2::new(1.0, 1.0));
        (fr.icon_svg, fr.button_scale)
    });
    super::svg_icons::draw_svg(p, &svg, r.center(), 17.0 * scale, mix(txt_dim(), hi(), h));
    if !resp.clicked() {
        return None;
    }
    with(|s| {
        if s.snap.is_some() || s.pending.is_some() {
            return None; // one reveal at a time
        }
        let to = !light_on();
        s.tg.poke();
        if anim_k() <= 0.0 {
            // animations off: straight switch, no overlay
            s.tg = tr::ThemeToggle::new(if to { tr::Mode::Light } else { tr::Mode::Dark }, cfg());
            LIGHT.store(to, std::sync::atomic::Ordering::Relaxed);
        } else {
            s.pending = Some((r.center(), to));
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        ctx.request_repaint();
        Some(to)
    })
}

/// Run every frame before anything reads the theme: picks up the screenshot, runs the reveal
pub(crate) fn tick(ctx: &egui::Context) {
    let shot = ctx.input(|i| {
        i.raw.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    let screen = ctx.screen_rect();
    let dt = ctx.input(|i| i.stable_dt);
    with(|s| {
        if let (Some(img), Some((at, to))) = (shot, s.pending) {
            s.pending = None;
            s.snap = Some(ctx.load_texture("qd_theme_snap", (*img).clone(), egui::TextureOptions::LINEAR));
            set_light_now(ctx, to);
            s.tg.start(tr::Vec2::new(at.x - screen.min.x, at.y - screen.min.y));
        }
        s.tg.update(dt);
        let fr = s.tg.frame(tr::Vec2::new(screen.width(), screen.height()));
        if s.tg.is_animating() || s.pending.is_some() {
            ctx.request_repaint();
        }
        if fr.reveal.is_none() {
            s.snap = None;
        }
        if fr.overlay.is_empty() && s.snap.is_none() {
            return;
        }
        // Under the drawn pointer (Debug), over everything else
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("qd_theme_reveal")));
        let o = screen.min;
        let at = |v: tr::Vec2| o + vec2(v.x, v.y);
        if let (Some(rv), Some(tex)) = (fr.reveal, &s.snap) {
            // The old look, with the new one showing through a growing hole
            let c = at(rv.center);
            let far = screen.size().length() * 2.0 + rv.radius;
            let uv = |q: Pos2| pos2((q.x - o.x) / screen.width().max(1.0), (q.y - o.y) / screen.height().max(1.0));
            let mut m = Mesh::with_texture(tex.id());
            const N: u32 = 128;
            for i in 0..N {
                let a = i as f32 / N as f32 * std::f32::consts::TAU;
                let d = vec2(a.cos(), a.sin());
                for q in [c + d * rv.radius, c + d * far] {
                    m.vertices.push(Vertex { pos: q, uv: uv(q), color: Color32::WHITE });
                }
            }
            for i in 0..N {
                let (a, b) = (2 * i, 2 * ((i + 1) % N));
                m.add_triangle(a, a + 1, b);
                m.add_triangle(b, a + 1, b + 1);
            }
            p.add(m);
        }
        // The crate's overlay, in its order
        for pr in &fr.overlay {
            match *pr {
                // Big circles (the bloom) as soft glows, not flat discs
                tr::Prim::Circle { c, r, color } if r > 12.0 => {
                    glow(&p, at(c), r * 1.5, col(color));
                }
                tr::Prim::Circle { c, r, color } => {
                    p.circle_filled(at(c), r, col(color));
                }
                // Wide rings (the rim) as a soft band, clear at both edges
                tr::Prim::Ring { c, r, width, color } if width > 4.0 => {
                    let (c, cl) = (at(c), col(color));
                    let mut m = Mesh::default();
                    const N: u32 = 160;
                    for i in 0..N {
                        let a = i as f32 / N as f32 * std::f32::consts::TAU;
                        let d = vec2(a.cos(), a.sin());
                        for (rr, cc) in [((r - width / 2.0).max(0.0), Color32::TRANSPARENT), (r, cl), (r + width / 2.0, Color32::TRANSPARENT)] {
                            m.vertices.push(Vertex { pos: c + d * rr, uv: WHITE_UV, color: cc });
                        }
                    }
                    for i in 0..N {
                        let (a, b) = (3 * i, 3 * ((i + 1) % N));
                        for s in 0..2 {
                            m.add_triangle(a + s, a + s + 1, b + s);
                            m.add_triangle(a + s + 1, b + s + 1, b + s);
                        }
                    }
                    p.add(m);
                }
                tr::Prim::Ring { c, r, width, color } => {
                    p.circle_stroke(at(c), r, EStroke::new(width, col(color)));
                }
                tr::Prim::Wedge { c, r, a0, a1, color } => {
                    // A soft beam instead of a hard slice: brightest down its middle, clear at
                    // both sides and fading in from the button and out toward its end, at a
                    // fraction of the crate's strength so the beams melt into one glow
                    let c = at(c);
                    let (am, hw) = ((a0 + a1) / 2.0, (a1 - a0) / 2.0);
                    let mut m = Mesh::default();
                    const L: u32 = 10;
                    for j in 0..=L {
                        let f = j as f32 / L as f32;
                        let k = (std::f32::consts::PI * f.powf(0.7)).sin().powi(2);
                        let d = r * f;
                        for (side, a) in [(-hw, 0.0), (0.0, k), (hw, 0.0)] {
                            let ang = am + side;
                            m.vertices.push(Vertex { pos: c + vec2(ang.cos(), ang.sin()) * d, uv: WHITE_UV, color: col(color.with_alpha(color.a * a)) });
                        }
                    }
                    for j in 0..L {
                        let (a, b) = (3 * j, 3 * (j + 1));
                        for s in 0..2 {
                            m.add_triangle(a + s, a + s + 1, b + s);
                            m.add_triangle(a + s + 1, b + s + 1, b + s);
                        }
                    }
                    p.add(m);
                }
                tr::Prim::Sparkle { c, size, rot, color } => {
                    // four tips joined by curves that dip toward the centre (same as path_d)
                    let (c, cl) = (at(c), col(color));
                    let pol = |a: f32, r: f32| c + vec2(a.cos(), a.sin()) * r;
                    let mut m = Mesh::default();
                    m.vertices.push(Vertex { pos: c, uv: WHITE_UV, color: cl });
                    const S: u32 = 6;
                    for k in 0..4 {
                        let a = rot + k as f32 * std::f32::consts::FRAC_PI_2;
                        let (t0, ct, t1) = (pol(a, size), pol(a + std::f32::consts::FRAC_PI_4, size * 0.14), pol(a + std::f32::consts::FRAC_PI_2, size));
                        for i in 0..S {
                            let t = i as f32 / S as f32;
                            let q = t0.to_vec2() * (1.0 - t) * (1.0 - t) + ct.to_vec2() * 2.0 * t * (1.0 - t) + t1.to_vec2() * t * t;
                            m.vertices.push(Vertex { pos: q.to_pos2(), uv: WHITE_UV, color: cl });
                        }
                    }
                    let n = 4 * S;
                    for i in 0..n {
                        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
                    }
                    p.add(m);
                }
            }
        }
    });
}
