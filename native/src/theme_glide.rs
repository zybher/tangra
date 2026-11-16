//! A new theme or accent colour glides in instead of snapping (animation audit #90, #91):
//! `theme_rgb()` / `blue()` read the colours shown here, which `tick` moves toward the
//! pick once a frame (same rate as every other glide, ui/micro.rs).
use crate::*;
use std::sync::Mutex;

/// (theme rgb, accent rgb) on screen right now
static SHOWN: Mutex<Option<([f32; 3], [f32; 3])>> = Mutex::new(None);

fn f3(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| v as f32)
}

fn u3(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| v.round().clamp(0.0, 255.0) as u8)
}

pub(crate) fn shown_theme() -> Option<[u8; 3]> {
    SHOWN.lock().unwrap().map(|s| u3(s.0))
}

pub(crate) fn shown_accent() -> Option<[u8; 3]> {
    SHOWN.lock().unwrap().map(|s| u3(s.1))
}

/// Run once a frame
pub(crate) fn tick(ctx: &egui::Context) {
    let (tt, ta) = (f3(theme_target()), f3(accent_target()));
    let moving = {
        let mut g = SHOWN.lock().unwrap();
        let s = g.get_or_insert((tt, ta));
        let moving = s.0 != tt || s.1 != ta;
        if moving {
            let ak = anim_k();
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            let f = if ak > 0.0 { 1.0 - (-dt * 16.0 / ak).exp() } else { 1.0 };
            for (cur, to) in [(&mut s.0, tt), (&mut s.1, ta)] {
                for i in 0..3 {
                    cur[i] += (to[i] - cur[i]) * f;
                    if (to[i] - cur[i]).abs() < 0.5 {
                        cur[i] = to[i];
                    }
                }
            }
        }
        moving
    };
    if moving {
        // egui's own widgets (menus, fields) follow along
        retint(ctx);
        ctx.request_repaint();
    }
}
