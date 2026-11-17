//! Copy as picture: the selection is drawn once more, on its own, onto a clear offscreen layer
//! (canvas/layers.rs) with the same painting code as the board, so the copy looks exactly like
//! the canvas: 3D tilt, adjustments, cuts, rubbed-out spots, opacity, corners, gradients,
//! blend modes, a frame's clipping. A frame takes everything inside it along. It renders at
//! 2 pixels per board unit (less if it wouldn't fit the window), with a see-through
//! background. The picture goes on the clipboard a frame later; pasting it back into Tangra
//! brings back the objects themselves (`IMG_CLIP`).

use crate::*;
use std::collections::HashSet;

pub(crate) struct Capture {
    ids: HashSet<String>,
    /// board box x, y, w, h
    world: [f32; 4],
    clip: ObjClip,
    note: String,
    /// frames since it was drawn (None: not drawn yet)
    sent: Option<u8>,
}

impl App {
    /// Ask for the selection `sel` as a picture; `clip` is what pasting it back brings back
    pub(crate) fn request_capture(&mut self, sel: &HashSet<Sel>, clip: ObjClip, note: impl Into<String>) {
        let roots: HashSet<&str> = sel.iter().map(|s| s.1.as_str()).collect();
        // everything inside a picked frame (any depth) comes along
        let mut ids: HashSet<String> = HashSet::new();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (k, id) in self.all_items() {
            let mut at: Option<&str> = Some(id.as_str());
            let mut inside = false;
            while let Some(a) = at {
                if roots.contains(a) {
                    inside = true;
                    break;
                }
                at = self.parent_of(a);
            }
            if !inside {
                continue;
            }
            // a frame's contents are cropped to it when it clips; otherwise they widen the box
            if let Some([x, y, w, h]) = self.rect_of(k, &id) {
                let pad = if k == Kind::Stroke { self.find_stroke(&id).map_or(0.0, |s| s.width) } else { 0.0 };
                (x0, y0, x1, y1) = (x0.min(x - pad), y0.min(y - pad), x1.max(x + w + pad), y1.max(y + h + pad));
            }
            ids.insert(id);
        }
        if ids.is_empty() || x0 >= x1 || y0 >= y1 {
            return;
        }
        self.capture = Some(Capture { ids, world: [x0, y0, x1 - x0, y1 - y0], clip, note: note.into(), sent: None });
    }

    /// Draw a waiting capture (once), then hand the picture to the clipboard when it's back
    pub(crate) fn run_capture(&mut self, ctx: &egui::Context) {
        let Some(c) = &mut self.capture else { return };
        ctx.request_repaint();
        match c.sent {
            None => {
                let ppp = ctx.pixels_per_point();
                let screen = ctx.screen_rect().size();
                let [wx, wy, ww, wh] = c.world;
                // 2 pixels per board unit, shrunk to fit the window (the layer is window-sized)
                let zoom = (2.0 / ppp).min(screen.x / ww).min(screen.y / wh);
                let size = vec2((ww * zoom).floor().max(1.0), (wh * zoom).floor().max(1.0));
                let v = View { origin: pos2(0.0, 0.0), cam: Camera { x: wx, y: wy, zoom } };
                let area = Rect::from_min_size(pos2(0.0, 0.0), size);
                let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("qd_capture"))).with_clip_rect(area);
                super::layers::begin_capture(&p);
                let ids = std::mem::take(&mut c.ids);
                self.paint_items(&p, area, v, Some(&ids));
                super::layers::end_capture(&p, size);
                if let Some(c) = &mut self.capture {
                    c.sent = Some(0);
                }
            }
            Some(n) => {
                if let Some((w, h, rgba)) = super::layers::take_capture() {
                    let Some(c) = self.capture.take() else { return };
                    match platform::copy_image(w, h, &rgba) {
                        Ok(()) => {
                            if let Ok(mut g) = IMG_CLIP.lock() {
                                *g = Some((w as u32, h as u32, c.clip));
                            }
                            self.toast(c.note);
                        }
                        Err(e) => self.toast(format!("Copy failed: {e}")),
                    }
                } else if n > 10 || !super::layers::available() {
                    // this GPU can't draw offscreen: the objects are still copied for pasting here
                    self.capture = None;
                    self.toast("Copied (as objects only)");
                } else {
                    c.sent = Some(n + 1);
                }
            }
        }
    }
}
