//! Every dropdown that hangs off the edit bar (style_bar.rs) is opened through `open`, so they
//! all behave the same whatever the object is (picture, shape, frame, text, on the canvas or
//! in a frame): where it opens (`drop_anim::drop_fit`: off the object, in 1-3 columns, else the
//! bar moves to the object's other side), centred over its button, the shared panel that
//! fades / slides and glides between dropdowns (`DropAnim`), the glass panel itself, the wave
//! magnify of its rows and their cascade from the bar edge. Closing (click outside, cursor
//! wandering off) is the bar's, also shared.
//!
//! A new dropdown on the bar is: a button that sets the bar's `pop` to a new number, then
//!
//! ```ignore
//! if da.k(N) > 0.001 {
//!     let dd = super::bar_drop::open(ui, &da, bid, pop, N, bar, rect, btn_cx[N], rows, width, bar_drop::MENU);
//!     pop_rect = pop_rect.union(dd.zone());
//!     for i in 0..rows {
//!         let (rr, wm, p) = dd.row(&da, i);
//!         // draw row i in `rr` with painter `p` (`wm` = how magnified it is), handle its click
//!     }
//! }
//! ```
use super::drop_anim::{self, DropAnim, PITCH};
use crate::*;

/// How the rows magnify under the pointer
pub(crate) struct Wave<'a> {
    /// per row 0..1: rows held raised (a slider being dragged)
    pub pinned: &'a [f32],
    /// wave spread (1 = the menus' wave)
    pub soft: f32,
    /// hold the wave still (while a row is dragged)
    pub lock: bool,
}

/// The menus' wave: pointer-follow, nothing pinned
pub(crate) const MENU: Wave<'static> = Wave { pinned: &[], soft: 1.0, lock: false };

/// One open (or closing) dropdown, placed and with its panel drawn
pub(crate) struct BarDrop {
    /// How shown it is (0..1), and whether it's the open one (takes clicks)
    pub k: f32,
    pub live: bool,
    /// Opens upward (above the bar)
    pub up: bool,
    /// The rows' area and the drawn panel around it (gliding between dropdowns)
    pub pr: Rect,
    pub bgr: Rect,
    /// Painter for things beside the panel (full opacity, unclipped)
    pub base: egui::Painter,
    rows: Vec<Rect>,
    mags: Vec<f32>,
    gx: f32,
    dy: f32,
    sg: f32,
}

/// Places dropdown `kind` (the bar's `pop` value for it) of `n` rows `w` wide under or over
/// `bar`, centred over its button at `btn_cx` (NAN once the button is gone: keeps the last
/// spot while it fades), and draws its panel. Call while `da.k(kind) > 0.001`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn open(ui: &mut egui::Ui, da: &DropAnim, bar_id: egui::Id, pop: u8, kind: u8, bar: Rect, rect: Rect, btn_cx: f32, n: usize, w: f32, wave: Wave) -> BarDrop {
    let ctx = ui.ctx().clone();
    let k = da.k(kind);
    let (cols, ph, up) = drop_anim::drop_fit(&ctx, bar_id, bar, n, rect, w);
    let xid = bar_id.with(("dcx", kind));
    let cx = if btn_cx.is_finite() { btn_cx } else { ctx.data(|d| d.get_temp(xid)).unwrap_or(bar.center().x) };
    ctx.data_mut(|d| d.insert_temp(xid, cx));
    let left = drop_anim::left_for(cx, rect, grid_w(w, cols));
    let live = pop == kind;
    // 14 px off the bar
    place(ui, da, bar_id.with(("wave", kind)), bar_id.with(("pop", kind)), k, live, bar, 14.0, left, ph, w, n, cols, up, wave)
}

/// A dropdown like the bar's hanging off a box in a panel (the properties card's stroke
/// position): left-aligned under `anchor` (over it when the window ends first), `gap` px off
/// it, one column (`right`: lined up on its right edge instead). `da` is the box's own
/// `DropAnim` (its dropdown is kind 1)
#[allow(clippy::too_many_arguments)]
pub(crate) fn open_under(ui: &mut egui::Ui, da: &DropAnim, id: egui::Id, live: bool, anchor: Rect, gap: f32, n: usize, w: f32, right: bool, wave: Wave) -> BarDrop {
    let ctx = ui.ctx().clone();
    let ph = n as f32 * PITCH + 8.0;
    let screen = ctx.screen_rect();
    let up = anchor.max.y + gap + ph > screen.max.y - 8.0 && anchor.min.y - gap - ph >= screen.min.y + 8.0;
    let left = if right { anchor.max.x - w } else { anchor.min.x }.clamp(screen.min.x + 8.0, (screen.max.x - 8.0 - w).max(screen.min.x + 8.0));
    place(ui, da, id.with("wave"), id.with("pop"), da.k(1), live, anchor, gap, left, ph, w, n, 1, up, wave)
}

/// The panel `gap` px off `bar`, sliding 8 px in from it, its rows on the wave, panel drawn
#[allow(clippy::too_many_arguments)]
fn place(ui: &mut egui::Ui, da: &DropAnim, wave_id: egui::Id, hit_id: egui::Id, k: f32, live: bool, bar: Rect, gap: f32, left: f32, ph: f32, w: f32, n: usize, cols: usize, up: bool, wave: Wave) -> BarDrop {
    let ctx = ui.ctx().clone();
    let sg = if up { -1.0 } else { 1.0 };
    let top = if up { bar.min.y - gap - ph } else { bar.max.y + gap } + (1.0 - k) * -8.0 * sg;
    let (rows, mags, pr) = wave_grid_k(&ctx, wave_id, left, top + ph, w, PITCH, n, cols, wave.pinned, wave.soft, wave.lock);
    // Anchored at the side next to the bar, so a hover swell grows away from it
    let dy = if up { top + ph - pr.max.y } else { top - pr.min.y };
    let pr = pr.translate(vec2(0.0, dy));
    let (bgr, gx) = da.panel(&ctx, pr, up);
    if live {
        ui.interact(pr, hit_id, Sense::click());
    }
    let base = ui.painter().clone().with_clip_rect(ctx.screen_rect());
    let mut p = base.clone();
    p.multiply_opacity(k);
    p.rect_filled(bgr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
    glass(&p, bgr, 10.0, 1.0);
    p.rect_stroke(bgr, 10.0, EStroke::new(1.0_f32, border()));
    BarDrop { k, live, up, pr, bgr, base, rows, mags, gx, dy, sg }
}

impl BarDrop {
    /// Row `i`: its rect (cascading in from the bar edge), how magnified it is (0..1), and a
    /// painter at its fade
    pub(crate) fn row(&self, da: &DropAnim, i: usize) -> (Rect, f32, egui::Painter) {
        let rk = da.row(i, self.rows.len(), self.up);
        let rr = self.rows[i].translate(vec2(self.gx, self.dy - self.sg * (1.0 - rk) * 16.0));
        let mut p = self.base.clone();
        p.multiply_opacity(self.k * rk);
        (rr, self.mags[i], p)
    }

    /// A scrolled list's item sitting `f` slots down (fractional while it scrolls): between the
    /// slots' rects and magnify, cascading with the slot it's in, its painter cut off at the
    /// panel's edge (no fade)
    pub(crate) fn row_at(&self, da: &DropAnim, f: f32) -> (Rect, f32, egui::Painter) {
        let n = self.rows.len();
        let slot = |j: i32| -> (Rect, f32) {
            if j < 0 {
                (self.rows[0].translate(vec2(0.0, j as f32 * PITCH)), 0.0)
            } else if j as usize >= n {
                (self.rows[n - 1].translate(vec2(0.0, (j as usize + 1 - n) as f32 * PITCH)), 0.0)
            } else {
                (self.rows[j as usize], self.mags[j as usize])
            }
        };
        let lo = f.floor();
        let t = f - lo;
        let ((a, ma), (b, mb)) = (slot(lo as i32), slot(lo as i32 + 1));
        let r = Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t));
        let rk = da.row((f.round().max(0.0) as usize).min(n - 1), n, self.up);
        let rr = r.translate(vec2(self.gx, self.dy - self.sg * (1.0 - rk) * 16.0));
        let clip = self.bgr.shrink(1.0).intersect(self.base.clip_rect());
        let mut p = self.base.clone().with_clip_rect(clip);
        p.multiply_opacity(self.k * rk);
        (rr, ma + (mb - ma) * t, p)
    }

    /// The panel as drawn (gliding), for a scrollbar inside it
    pub(crate) fn panel_rect(&self) -> Rect {
        self.bgr
    }

    /// Slot `i`'s rect where it rests sideways (wave swell included), for lining things up
    pub(crate) fn slot(&self, i: usize) -> Rect {
        self.rows[i].translate(vec2(self.gx, self.dy))
    }

    /// The rows where they rest (no cascade), for things riding them (the picture reset button)
    pub(crate) fn rest_rows(&self) -> impl Iterator<Item = Rect> + '_ {
        self.rows.iter().map(move |r| r.translate(vec2(self.gx, 0.0)))
    }

    /// While open: the area that keeps it open (pointer / clicks there don't close it)
    pub(crate) fn zone(&self) -> Rect {
        if self.live {
            self.pr.expand2(vec2(14.0, 0.0))
        } else {
            Rect::NOTHING
        }
    }
}
