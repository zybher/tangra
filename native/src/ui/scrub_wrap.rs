//! Scrubbing a number never runs out of screen: when the pointer reaches the left or right edge
//! of the desktop mid-scrub it jumps to the other edge and the scrub carries on (item 4).
//! `dx` is the sideways distance dragged since the press, wraps included; every number scrub
//! (props, place bar, style bar, adjust / 3D sliders) reads it instead of pointer - origin.
//! The jump only counts once egui actually sees the pointer land on the far side, so the value
//! never flicks for a frame.
use crate::*;

#[derive(Clone, Copy)]
struct St {
    origin: Pos2,
    /// points added by wraps that egui has already seen
    off: f32,
    /// a wrap egui hasn't reported yet (points)
    pending: f32,
    last_x: f32,
    pass: u64,
    out: f32,
}

fn sid() -> egui::Id {
    egui::Id::new("qd_scrub_wrap")
}

/// Sideways drag since the press (points), with edge wraps added in
pub(crate) fn dx(ctx: &egui::Context) -> f32 {
    let Some((o, q)) = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos())) else {
        ctx.data_mut(|d| d.remove::<St>(sid()));
        return 0.0;
    };
    let pass = ctx.cumulative_pass_nr();
    let mut st = ctx
        .data(|d| d.get_temp::<St>(sid()))
        .filter(|s| s.origin == o)
        .unwrap_or(St { origin: o, off: 0.0, pending: 0.0, last_x: q.x, pass: u64::MAX, out: 0.0 });
    if st.pass == pass {
        return st.out;
    }
    // The jump has arrived: count it
    if st.pending != 0.0 {
        let moved = st.last_x - q.x;
        if moved * st.pending > 0.0 && moved.abs() > st.pending.abs() / 2.0 {
            st.off += st.pending;
            st.pending = 0.0;
        }
    }
    if st.pending == 0.0 {
        if let (Some((x, y)), (left, width)) = (crate::platform::cursor_screen(), crate::platform::desktop_x()) {
            let right = left + width - 1;
            let to = if x >= right - 1 { Some(left + 2) } else if x <= left { Some(right - 3) } else { None };
            if let Some(nx) = to.filter(|_| width > 16) {
                crate::platform::set_cursor_screen(nx, y);
                st.pending = (x - nx) as f32 / ctx.pixels_per_point();
            }
        }
    }
    st.last_x = q.x;
    st.pass = pass;
    st.out = q.x - o.x + st.off;
    ctx.data_mut(|d| d.insert_temp(sid(), st));
    st.out
}
