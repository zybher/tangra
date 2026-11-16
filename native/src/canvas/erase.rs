//! The eraser rubs out only what it passes over, like a paint program's eraser. Nothing is
//! split or deleted: each object stays one object and remembers the rubbed spots
//! (`erase`, drawn as holes by canvas/holes.rs).
//! - Starting a swipe on a picture rubs that picture only.
//! - Anywhere else it rubs drawings and text (on the board and on pictures). Frames, notes
//!   and links are never touched.
use super::holes::{in_hole, label_box, stroke_box};
use crate::*;
use std::sync::Mutex;

/// Where the eraser was on the last call (a fast swipe erases the whole way between) and
/// the picture this swipe rubs, if it started on one
static SWIPE: Mutex<Option<(Pt, Option<String>)>> = Mutex::new(None);

/// A new swipe of the eraser starts here
pub(crate) fn reset() {
    *SWIPE.lock().unwrap() = None;
}

/// Points along a..b no further apart than `step`
fn along(a: Pt, b: Pt, step: f32) -> Vec<Pt> {
    let d = (b.x - a.x).hypot(b.y - a.y);
    let n = (d / step.max(1e-4)).ceil().max(1.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            Pt { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
        })
        .collect()
}

/// Adds spots of radius `r` along a..b wherever `touch` says the object is there (all in the
/// object's own units). True if any were added
fn rub(spots: &mut Vec<[f32; 4]>, loops: &[Vec<[f32; 2]>], bx: [f32; 4], a: Pt, b: Pt, r: f32, touch: impl Fn(Pt) -> bool) -> bool {
    let [x, y, w, h] = bx;
    if w < 1e-4 || h < 1e-4 {
        return false;
    }
    let mut hit = false;
    for q in along(a, b, r * 0.4) {
        if !touch(q) {
            continue;
        }
        // already rubbed here
        if in_hole(spots, loops, bx, q) && spots.last().is_some_and(|l| ((l[0] * w + x) - q.x).hypot((l[1] * h + y) - q.y) < r * 0.3) {
            continue;
        }
        spots.push([(q.x - x) / w, (q.y - y) / h, r / w, r / h]);
        hit = true;
    }
    hit
}

fn rub_strokes(list: &mut [Stroke], a: Pt, b: Pt, r: f32) -> bool {
    let mut changed = false;
    for s in list.iter_mut().filter(|s| !super::presence::is_dying(&s.id)) {
        let Some(bx) = stroke_box(s) else { continue };
        let probe = s.clone();
        let filled = probe.fill && probe.closed_len().is_some();
        let [x, y, w, h] = bx;
        changed |= rub(&mut s.erase, &probe.erase_loops, bx, a, b, r, |q| {
            stroke_hit(&probe, q, r + probe.width / 2.0) || (filled && inside(q, x - r, y - r, w + 2.0 * r, h + 2.0 * r))
        });
    }
    changed
}

fn rub_texts(list: &mut [Label], a: Pt, b: Pt, r: f32) -> bool {
    let mut changed = false;
    for l in list.iter_mut().filter(|l| !super::presence::is_dying(&l.id) && !l.text.is_empty()) {
        let bx = label_box(l);
        let [x, y, w, h] = bx;
        changed |= rub(&mut l.erase, &l.erase_loops, bx, a, b, r, |q| inside(q, x - r, y - r, w + 2.0 * r, h + 2.0 * r));
    }
    changed
}

impl App {
    /// Rubs out what's under the eraser between its last spot and `p`. True if anything changed
    pub(crate) fn erase_at(&mut self, p: Pt) -> bool {
        let r = 9.0 / self.board.camera.zoom;
        let (a, pic) = {
            let mut sw = SWIPE.lock().unwrap();
            let (a, pic) = match sw.take() {
                Some(v) => v,
                // first touch: a swipe that starts on a picture rubs just that picture
                None => (p, self.hit_ordered(p, false).filter(|s| s.0 == Kind::Image).map(|s| s.1)),
            };
            *sw = Some((p, pic.clone()));
            (a, pic)
        };
        let b = &mut self.board;
        let changed = if let Some(id) = pic {
            match b.images.iter_mut().find(|i| i.id == id) {
                Some(img) => {
                    let bx = [img.x, img.y, img.width, img.height];
                    rub(&mut img.erase, &img.erase_loops.clone(), bx, a, p, r, |q| inside(q, bx[0] - r, bx[1] - r, bx[2] + 2.0 * r, bx[3] + 2.0 * r))
                }
                None => false,
            }
        } else {
            let mut changed = rub_strokes(&mut b.world_strokes, a, p, r);
            changed |= rub_texts(&mut b.world_texts, a, p, r);
            // drawings and text on the picture under the eraser, in its own units
            if let Some(img) = b.images.iter_mut().rev().find(|i| inside(p, i.x, i.y, i.width, i.height)) {
                let (sx, sy) = img.scale();
                let (ox, oy) = (img.x, img.y);
                let loc = |q: Pt| Pt { x: (q.x - ox) / sx, y: (q.y - oy) / sy };
                changed |= rub_strokes(&mut img.annotations.strokes, loc(a), loc(p), r / sx);
                changed |= rub_texts(&mut img.annotations.texts, loc(a), loc(p), r / sx);
            }
            changed
        };
        if changed {
            self.mark_dirty();
        }
        changed
    }
}
