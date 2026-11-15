//! Lasso cut tool (K): drag a loop over pictures, then Delete cuts that part out
//! (kept as an edit on each picture, so "Restore cut areas" brings it back).

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
        let picked: Vec<&(String, Vec<Pt>)> = hits.iter().filter(|h| self.selected.contains(&(Kind::Image, h.0.clone()))).collect();
        let hits: Vec<(String, Vec<Pt>)> = if picked.is_empty() { hits.clone() } else { picked.into_iter().cloned().collect() };
        if hits.is_empty() {
            self.toast("The loop doesn't touch a picture".to_string());
            return true;
        }
        self.snapshot();
        for (id, pts) in &hits {
            if let Some(img) = self.board.images.iter_mut().find(|i| &i.id == id) {
                img.edit.get_or_insert_with(Default::default).cuts.push(pts.clone());
            }
        }
        let n = hits.len();
        self.toast(format!("Cut from {n} picture{}  ·  right-click → Restore cut areas", if n == 1 { "" } else { "s" }));
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
