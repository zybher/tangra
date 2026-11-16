//! "Reset adjustments" / "Reset 3D" morph the values back to zero instead of snapping: the
//! same tween a typed number uses (num_edit.rs, cubic out). One undo step for the whole reset.
use crate::*;

#[derive(Clone)]
struct Morph {
    img: String,
    /// (field, start value)
    from: Vec<(u8, f32)>,
    t0: f64,
}

fn mid() -> egui::Id {
    egui::Id::new("qd_reset_morph")
}

impl App {
    /// Starts easing `fields` of picture `img` (currently `vals`) back to 0
    pub(crate) fn reset_morph_start(&mut self, ctx: &egui::Context, img: &str, fields: &[u8], ed: &crate::image_edit::ImageEdit) {
        let mut e = ed.clone();
        let from: Vec<(u8, f32)> = fields.iter().map(|&f| (f, *e.field(f))).filter(|(_, v)| *v != 0.0).collect();
        if from.is_empty() {
            return;
        }
        self.snapshot();
        let t0 = ctx.input(|i| i.time);
        ctx.data_mut(|d| d.insert_temp(mid(), Morph { img: img.to_owned(), from, t0 }));
        ctx.request_repaint();
    }

    /// Advances a running reset; `img` is the picture the bar is on, `stop` cancels (a drag took over)
    pub(crate) fn reset_morph_step(&mut self, ctx: &egui::Context, img: &str, stop: bool) {
        let Some(m) = ctx.data(|d| d.get_temp::<Morph>(mid())) else {
            return;
        };
        if stop || m.img != img {
            ctx.data_mut(|d| d.remove::<Morph>(mid()));
            return;
        }
        let now = ctx.input(|i| i.time);
        let x = (((now - m.t0) / (super::num_edit::TWEEN * anim_k().max(1e-3) as f64)) as f32).clamp(0.0, 1.0);
        let k = 1.0 - (1.0 - x).powi(3);
        let from = m.from.clone();
        self.edit_each(|e| {
            for &(f, v0) in &from {
                *e.field(f) = if x >= 1.0 { 0.0 } else { v0 * (1.0 - k) };
            }
        });
        if x >= 1.0 {
            ctx.data_mut(|d| d.remove::<Morph>(mid()));
        } else {
            ctx.request_repaint();
        }
    }
}
