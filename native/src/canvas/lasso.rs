//! Lasso cut tool (K): drag a loop over pictures, drawings or text, then Delete cuts that
//! part out (on pictures kept as an edit, so "Restore cut areas" brings it back; on
//! drawings and text kept as a hole, canvas/holes.rs).

use super::tilt3d::Tilt;
use crate::*;

impl App {
    pub(crate) fn begin_lasso(&mut self, w: Pt) {
        self.lasso = Some(vec![w]);
        self.drag = Drag::Lasso;
    }

    pub(crate) fn extend_lasso(&mut self, w: Pt) {
        let min = 2.0 / self.board.camera.zoom;
        if let Some(l) = self.lasso.as_mut() {
            if l.last().map_or(true, |q| (q.x - w.x).hypot(q.y - w.y) > min) {
                l.push(w);
            }
        }
    }

    /// A loop too small to mean anything is dropped when the drag ends
    pub(crate) fn end_lasso(&mut self) {
        let z = self.board.camera.zoom;
        let keep = self.lasso.as_ref().is_some_and(|l| {
            let (x0, x1) = l.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.x), a.1.max(p.x)));
            let (y0, y1) = l.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.y), a.1.max(p.y)));
            l.len() >= 3 && (x1 - x0).max(y1 - y0) * z > 6.0
        });
        if !keep {
            self.lasso = None;
        }
    }

    /// Delete with a loop drawn: cut it out of the pictures it touches (the selected
    /// ones if any of those are under it). False when there was no loop.
    pub(crate) fn lasso_cut(&mut self) -> bool {
        let Some(l) = self.lasso.take() else { return false };
        // The loop in each picture's own pixels, if it overlaps the picture
        let local = |img: &ImageNode| -> Option<Vec<Pt>> {
            let (sx, sy) = img.scale();
            let tilt = img.edit.as_ref().filter(|e| e.has_tilt()).map(|e| Tilt::new(e, img.x, img.y, img.width, img.height));
            let pts: Vec<Pt> = l
                .iter()
                .filter_map(|p| match &tilt {
                    Some(t) => t.back(p.x, p.y).map(|(x, y)| Pt { x, y }),
                    None => Some(*p),
                })
                .map(|p| Pt { x: (p.x - img.x) / sx, y: (p.y - img.y) / sy })
                .collect();
            let (ow, oh) = (img.width / sx, img.height / sy);
            let hit = pts.len() >= 3
                && pts.iter().map(|p| p.x).fold(f32::MIN, f32::max) > 0.0
                && pts.iter().map(|p| p.x).fold(f32::MAX, f32::min) < ow
                && pts.iter().map(|p| p.y).fold(f32::MIN, f32::max) > 0.0
                && pts.iter().map(|p| p.y).fold(f32::MAX, f32::min) < oh;
            hit.then_some(pts)
        };
        let hits: Vec<(String, Vec<Pt>)> = self.board.images.iter().filter_map(|i| local(i).map(|p| (i.id.clone(), p))).collect();
        // Drawings and text the loop overlaps get it as a hole, in their box's fractions
        // (canvas/holes.rs)
        use super::holes::{label_box, stroke_box};
        let lb = pts_bbox(&l).unwrap_or([0.0; 4]);
        let over = |b: &[f32; 4]| b[2] > 0.0 && b[3] > 0.0 && lb[0] < b[0] + b[2] && b[0] < lb[0] + lb[2] && lb[1] < b[1] + b[3] && b[1] < lb[1] + lb[3];
        let b = &self.board;
        let mut marks: Vec<Sel> = b.world_strokes.iter().filter(|s| !s.pixelate && stroke_box(s).is_some_and(|bx| over(&bx))).map(|s| (Kind::Stroke, s.id.clone())).collect();
        marks.extend(b.world_texts.iter().filter(|t| over(&label_box(t))).map(|t| (Kind::Text, t.id.clone())));
        // only the selected ones, if any of those are under it
        let sel = |s: &Sel| self.selected.contains(s);
        let any_sel = hits.iter().any(|h| sel(&(Kind::Image, h.0.clone()))) || marks.iter().any(sel);
        let hits: Vec<(String, Vec<Pt>)> = hits.into_iter().filter(|h| !any_sel || sel(&(Kind::Image, h.0.clone()))).collect();
        let marks: Vec<Sel> = marks.into_iter().filter(|m| !any_sel || sel(m)).collect();
        if hits.is_empty() && marks.is_empty() {
            self.toast("The loop doesn't touch anything it can cut".to_string());
            return true;
        }
        self.snapshot();
        for (id, pts) in &hits {
            if let Some(img) = self.board.images.iter_mut().find(|i| &i.id == id) {
                img.edit.get_or_insert_with(Default::default).cuts.push(pts.clone());
            }
        }
        let norm = |bx: [f32; 4]| -> Vec<[f32; 2]> { l.iter().map(|q| [(q.x - bx[0]) / bx[2], (q.y - bx[1]) / bx[3]]).collect() };
        for (k, id) in &marks {
            let b = &mut self.board;
            match k {
                Kind::Stroke => {
                    if let Some(s) = b.world_strokes.iter_mut().find(|s| &s.id == id) {
                        if let Some(bx) = stroke_box(s) {
                            s.erase_loops.push(norm(bx));
                        }
                    }
                }
                _ => {
                    if let Some(t) = b.world_texts.iter_mut().find(|t| &t.id == id) {
                        let bx = label_box(t);
                        t.erase_loops.push(norm(bx));
                    }
                }
            }
        }
        self.mark_dirty();
        true
    }

    /// Marching-ants outline of the loop
    pub(crate) fn paint_lasso(&self, p: &egui::Painter, v: View) {
        let Some(l) = self.lasso.as_ref().filter(|l| l.len() >= 2) else { return };
        let mut pts: Vec<Pos2> = l.iter().map(|q| v.s(q.x, q.y)).collect();
        let drawing = matches!(self.drag, Drag::Lasso);
        if !drawing {
            pts.push(pts[0]);
        }
        p.add(egui::Shape::line(pts.clone(), EStroke::new(1.5_f32, Color32::from_black_alpha(170))));
        let off = (self.now * 24.0) as f32 % 10.0;
        p.extend(egui::Shape::dashed_line_with_offset(&pts, EStroke::new(1.5_f32, Color32::WHITE), &[5.0], &[5.0], off));
        p.ctx().request_repaint();
    }
}
