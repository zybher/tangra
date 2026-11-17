//! The canvas dot grid without snaps (animation audit #93, #95): zooming fades the next
//! spacing's in-between dots in instead of halving the grid at once, and a new pattern
//! crossfades with the old one.
use crate::*;

/// Paints the grid of pattern marks over the canvas
pub(crate) fn paint_grid(p: &egui::Painter, rect: Rect, v: View, pattern: u8, dots: f32, now: f64) {
    let z = v.cam.zoom;
    // white dots on a dark canvas, dark ones in light mode (theme.rs)
    // (dark marks read fainter on light than white ones on dark, so they get more alpha)
    let dot = wa((dots.clamp(0.0, 100.0) * (0.26 + 0.29 * lk())) as u8);
    if dot.a() == 0 {
        return;
    }
    // Spacing on screen: 32 world units, doubled while under 20 px when zoomed out
    let mut g = 32.0 * z;
    let doubled = g < 20.0;
    while g < 20.0 {
        g *= 2.0;
    }
    // Zoomed out, the grid doubles its spacing at 20 px (to 40). Just before that the
    // in-between dots of the finer grid are still there, fading out as g nears 28, so the
    // jump from 20 to 40 shows the same dots on both sides. At normal zoom nothing changes.
    let t = if doubled { ((g - 28.0) / 12.0).clamp(0.0, 1.0) } else { 0.0 };
    let half = t * t * (3.0 - 2.0 * t);
    // Pattern change: (old pattern, when it changed); crossfades over the fade time
    let ctx = p.ctx();
    let pid = egui::Id::new("qd_grid_pat");
    let (cur, prev, t0): (u8, u8, f64) = ctx.data(|d| d.get_temp(pid)).unwrap_or((pattern, pattern, 0.0));
    let (prev, t0) = if cur != pattern { (cur, now) } else { (prev, t0) };
    ctx.data_mut(|d| d.insert_temp(pid, (pattern, prev, t0)));
    let dur = 0.18 * anim_k() as f64;
    let pk = if dur <= 0.0 { 1.0 } else { ((now - t0) / dur).clamp(0.0, 1.0) as f32 };
    let pk = 1.0 - (1.0 - pk).powi(3);
    if pk < 1.0 {
        ctx.request_repaint();
    }
    let marks: Vec<(u8, f32)> = if pk < 1.0 && prev != pattern { vec![(prev, 1.0 - pk), (pattern, pk)] } else { vec![(pattern, 1.0)] };
    // Step half the spacing; the in-between dots (odd on either axis) take `half`
    let fine = half > 0.01;
    let step = if fine { g / 2.0 } else { g };
    let ox = -v.cam.x * z;
    let oy = -v.cam.y * z;
    let i0 = ((rect.min.x - ox) / step).floor() as i64;
    let j0 = ((rect.min.y - oy) / step).floor() as i64;
    let mut i = i0;
    loop {
        let x = ox + i as f32 * step;
        if x >= rect.max.x {
            break;
        }
        let mut j = j0;
        loop {
            let y = oy + j as f32 * step;
            if y >= rect.max.y {
                break;
            }
            let a = if !fine || (i % 2 == 0 && j % 2 == 0) { 1.0 } else { half };
            if a > 0.01 && x >= rect.min.x && y >= rect.min.y {
                for &(kind, k) in &marks {
                    draw_pattern(p, pos2(x + 0.75, y + 0.75), kind, dot.gamma_multiply(a * k), 1.0);
                }
            }
            j += 1;
        }
        i += 1;
    }
}
