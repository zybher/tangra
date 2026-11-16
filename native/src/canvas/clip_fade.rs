//! Turning a frame's clipping on or off eases the crop in from (or back out to) the edge of
//! what's inside it instead of cutting at once (animation audit #21). Paint only: picking
//! and everything else keep the plain `clip_boxes`.
use crate::*;
use std::collections::HashMap;

fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
    let (x1, y1) = ((a[0] + a[2]).max(b[0] + b[2]), (a[1] + a[3]).max(b[1] + b[3]));
    [x0, y0, x1 - x0, y1 - y0]
}

impl App {
    pub(crate) fn clip_boxes_eased(&self, ctx: &egui::Context) -> HashMap<String, [f32; 4]> {
        // How clipped each frame is right now
        let ks: HashMap<&str, f32> = self
            .board
            .group_cards
            .iter()
            .map(|g| (g.id.as_str(), ease(ctx, egui::Id::new(("qd_clip", g.id.as_str())), g.clip)))
            .filter(|(_, k)| *k > 0.001)
            .collect();
        if ks.values().all(|k| *k >= 0.999) {
            return self.clip_boxes();
        }
        // Each easing frame's crop starts from everything inside it
        let rects: HashMap<String, [f32; 4]> = self.all_items().into_iter().filter_map(|(k, id)| self.rect_of(k, &id).map(|r| (id, r))).collect();
        let frames: HashMap<&str, &Group> = self.board.group_cards.iter().map(|g| (g.id.as_str(), g)).collect();
        let mut boxes: HashMap<&str, [f32; 4]> = ks.keys().filter_map(|id| frames.get(id).map(|g| (*id, self.crop_box(g)))).collect();
        let mut ext = boxes.clone();
        for (id, _) in &self.board.parents {
            let Some(r) = rects.get(id) else { continue };
            let mut cur = self.parent_of(id);
            for _ in 0..64 {
                let Some(pid) = cur else { break };
                if let Some(e) = ext.get_mut(pid) {
                    *e = union(*e, *r);
                }
                cur = self.parent_of(pid);
            }
        }
        for (id, b) in boxes.iter_mut() {
            let k = ks[id];
            if k < 0.999 {
                let e = ext[id];
                *b = [0, 1, 2, 3].map(|i| e[i] + (b[i] - e[i]) * k);
            }
        }
        let mut out = HashMap::new();
        for (id, _) in &self.board.parents {
            let mut c: Option<[f32; 4]> = None;
            let mut cur = self.parent_of(id);
            for _ in 0..64 {
                let Some(pid) = cur else { break };
                if let Some(b) = boxes.get(pid).copied() {
                    c = Some(match c {
                        None => b,
                        Some(a) => {
                            let (x0, y0) = (a[0].max(b[0]), a[1].max(b[1]));
                            let (x1, y1) = ((a[0] + a[2]).min(b[0] + b[2]), (a[1] + a[3]).min(b[1] + b[3]));
                            [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
                        }
                    });
                }
                cur = self.parent_of(pid);
            }
            if let Some(c) = c {
                out.insert(id.clone(), c);
            }
        }
        out
    }
}
