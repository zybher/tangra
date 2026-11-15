//! Picture dropdowns on the placement bar: Adjust (colour) and 3D (tilt). Figma-style rows,
//! a label on the left and a slider track with a white knob; a reset row at the bottom.
//! Same glass panel, entrance and row stagger as the style bar's dropdowns.

use crate::image_edit::{ImageEdit, ADJUST_ROWS, TILT_ROWS};
use crate::*;

const ROW_H: f32 = 30.0;
const PANEL_W: f32 = 250.0;

impl App {
    /// Draws the open dropdown (1 = Adjust, 2 = 3D) for picture `img`, under / over `bar`.
    /// Returns the panel's rect while one is open.
    pub(crate) fn image_pop(&mut self, ui: &mut egui::Ui, rect: Rect, bar: Rect, pop: u8, img: &str, bid: egui::Id) -> Rect {
        let ctx = ui.ctx().clone();
        let ed: ImageEdit = self.board.images.iter().find(|i| i.id == img).and_then(|i| i.edit.clone()).unwrap_or_default();
        let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        let mut open_rect = Rect::NOTHING;
        for kind in [1u8, 2] {
            let kk = ctx.animate_bool_with_time(bid.with(("ipop", kind)), pop == kind, 0.22 * anim_k());
            if kk <= 0.001 {
                continue;
            }
            let sliders: &[(u8, &str, f32, f32)] = if kind == 1 { &ADJUST_ROWS } else { &TILT_ROWS };
            // Reset rows: (label, enabled, which)
            let mut resets: Vec<(&str, bool, u8)> = Vec::new();
            if kind == 1 {
                resets.push(("Reset adjustments", ed.has_color(), 0));
                if !ed.cuts.is_empty() {
                    resets.push(("Restore cut areas", true, 1));
                }
            } else {
                resets.push(("Reset 3D", ed.has_tilt() || ed.depth != 0.0, 2));
            }
            let ph = sliders.len() as f32 * ROW_H + 9.0 + resets.len() as f32 * 26.0 + 10.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let slide = (1.0 - ez(kk)) * -8.0 * sg;
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide;
            let x0 = bar.center().x.clamp(rect.min.x + PANEL_W / 2.0 + 8.0, (rect.max.x - PANEL_W / 2.0 - 8.0).max(rect.min.x + PANEL_W / 2.0 + 8.0)) - PANEL_W / 2.0;
            let pr = Rect::from_min_size(pos2(x0, top), vec2(PANEL_W, ph));
            let live = pop == kind;
            if live {
                open_rect = pr;
                ui.interact(pr, bid.with(("ipanel", kind)), Sense::click());
            }
            let pk = ui.painter().clone().with_clip_rect(ctx.screen_rect());
            let mut p = pk.clone();
            p.multiply_opacity(ez(kk));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));

            let mut y = pr.min.y + 4.0;
            let mut n = 0usize;
            for &(k, label, lo, hi) in sliders {
                let rk = ez(kk * 1.5 - n as f32 * 0.1);
                n += 1;
                let row = Rect::from_min_size(pos2(pr.min.x, y + sg * (1.0 - rk) * 10.0), vec2(PANEL_W, ROW_H));
                y += ROW_H;
                let mut p = pk.clone();
                p.multiply_opacity(ez(kk) * rk);
                let mut e = ed.clone();
                let v = *e.field(k);
                let track = Rect::from_min_max(pos2(row.min.x + 104.0, row.center().y - 9.0), pos2(row.max.x - 14.0, row.center().y + 9.0));
                let resp = ui.interact(track.expand2(vec2(4.0, 5.0)), bid.with(("islider", k)), if live { Sense::click_and_drag() } else { Sense::hover() });
                let (hh, _) = hover_fx(&ctx, &resp, live);
                let down = live && resp.is_pointer_button_down_on();
                let col = mix(TEXT_DIM, TEXT, hh.max(down as u8 as f32));
                p.text(pos2(row.min.x + 14.0, row.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), col);
                if v != 0.0 {
                    p.text(pos2(track.min.x - 8.0, row.center().y), Align2::RIGHT_CENTER, format!("{}", v.round() as i32), FontId::proportional(11.0), TEXT_FAINT);
                }
                p.rect_filled(track, 9.0, Color32::from_white_alpha(8 + (6.0 * hh) as u8));
                p.rect_stroke(track, 9.0, EStroke::new(1.0_f32, border()));
                let (kx0, kx1) = (track.min.x + 9.0, track.max.x - 9.0);
                let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
                let knob = pos2(kx0 + t * (kx1 - kx0), track.center().y);
                p.circle_filled(knob + vec2(0.0, 1.0), 8.0, Color32::from_black_alpha(60));
                p.circle_filled(knob, 8.0 + 0.6 * hh, Color32::WHITE);
                if resp.hovered() || down {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                // A press anywhere on the track sets the value; one undo step per press
                let wid = bid.with(("islider_down", k));
                let was_down: bool = ctx.data(|d| d.get_temp(wid)).unwrap_or(false);
                ctx.data_mut(|d| d.insert_temp(wid, down));
                if resp.double_clicked() {
                    self.set_image_edit(k, 0.0, true);
                } else if down {
                    if let Some(q) = ctx.input(|i| i.pointer.interact_pos()) {
                        let nv = (lo + ((q.x - kx0) / (kx1 - kx0)).clamp(0.0, 1.0) * (hi - lo)).round();
                        if nv != v || !was_down {
                            self.set_image_edit(k, nv, !was_down);
                        }
                    }
                }
            }
            // Divider, then the reset rows
            let dy = y + 4.0;
            p.line_segment([pos2(pr.min.x + 10.0, dy), pos2(pr.max.x - 10.0, dy)], EStroke::new(1.0_f32, border()));
            y += 9.0;
            for (label, enabled, which) in resets {
                let rk = ez(kk * 1.5 - n as f32 * 0.1);
                n += 1;
                let rr = Rect::from_min_size(pos2(pr.min.x + 4.0, y + sg * (1.0 - rk) * 10.0), vec2(PANEL_W - 8.0, 26.0));
                y += 26.0;
                let mut p = pk.clone();
                p.multiply_opacity(ez(kk) * rk);
                let resp = ui.interact(rr, bid.with(("ireset", which)), if live { Sense::click() } else { Sense::hover() });
                let (hh, _) = hover_fx(&ctx, &resp, live && enabled);
                wave_tile(&p, rr, hh, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let col = if enabled { mix(TEXT_DIM, TEXT, hh) } else { TEXT_FAINT };
                p.text(pos2(rr.min.x + 10.0, rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), col);
                if live && enabled && resp.clicked() {
                    match which {
                        0 => self.reset_adjust(),
                        1 => self.restore_cuts(),
                        _ => self.reset_tilt(),
                    }
                }
            }
            if kk < 1.0 {
                ctx.request_repaint();
            }
        }
        open_rect
    }
}
