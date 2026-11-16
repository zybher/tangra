//! Board-level operations: delete, z-order, editing, switch board, zoom-fit, hide/quit.

use crate::*;

impl App {
    pub(crate) fn delete_selected(&mut self) {
        // Deleted things fade out first and are removed when that ends (canvas/presence.rs)
        if let Some((_, label)) = self.sel_label.take() {
            self.snapshot();
            self.kill_or_remove(vec![label]);
            return;
        }
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let to_delete = std::mem::take(&mut self.selected);
        self.sel = None;
        self.kill_or_remove(to_delete.into_iter().map(|s| s.1).collect());
    }

    /// Fades these items out (removed at the end), or removes them now with animations off
    pub(crate) fn kill_or_remove(&mut self, ids: Vec<String>) {
        if anim_k() == 0.0 {
            self.remove_ids(&ids.into_iter().collect());
        } else {
            crate::canvas::presence::kill(ids, self.now);
        }
    }

    pub(crate) fn bring_to_front(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        // within their own frame, in the order the outline and canvas share (canvas/z_order.rs)
        let ids: HashSet<String> = self.selected.iter().map(|s| s.1.clone()).collect();
        self.restack(&ids, true);
        // the moved items dip and come back so the restack reads (audit #37)
        crate::canvas::presence::enter(self.selected.iter().map(|s| s.1.clone()), self.now, 0.3);
    }

    /// Ctrl+[ : the selection goes beneath everything else in its frame
    pub(crate) fn send_to_back(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        // within their own frame, in the order the outline and canvas share (canvas/z_order.rs)
        let ids: HashSet<String> = self.selected.iter().map(|s| s.1.clone()).collect();
        self.restack(&ids, false);
        // the moved items dip and come back so the restack reads (audit #37)
        crate::canvas::presence::enter(self.selected.iter().map(|s| s.1.clone()), self.now, 0.3);
    }

    /// Double-click / "edit": notes and frames open an editor, links open in the browser.
    pub(crate) fn activate(&mut self, sel: Sel) {
        let (k, id) = sel;
        match k {
            Kind::Stroke => {}
            // Sticky notes were removed; an old board's notes still show but aren't edited
            Kind::Note => {}
            Kind::Group => {
                if let Some(g) = self.board.group_cards.iter().find(|g| g.id == id) {
                    self.edit = Some(Edit {
                        target: EditTarget::Group(id),
                        world: Pt { x: g.x, y: g.y },
                        text: g.title.clone(),
                        fresh: 2,
                    });
                }
            }
            Kind::Link => {
                if let Some(l) = self.board.link_cards.iter().find(|l| l.id == id) {
                    platform::open_url(&l.url);
                }
            }
            Kind::Text => {
                if let Some(t) = self.board.world_texts.iter().find(|t| t.id == id) {
                    self.edit = Some(Edit {
                        target: EditTarget::WorldText(id),
                        world: Pt { x: t.x, y: t.y },
                        text: t.text.clone(),
                        fresh: 2,
                    });
                }
            }
            Kind::Image => {}
        }
    }

    pub(crate) fn begin_label(&mut self, world: Pt) {
        let image = self.image_at(world).map(|i| i.id.clone());
        self.edit = Some(Edit { target: EditTarget::NewLabel { image }, world, text: String::new(), fresh: 2 });
    }

    pub(crate) fn commit_edit(&mut self) {
        let Some(e) = self.edit.take() else { return };
        match e.target {
            EditTarget::NewLabel { image } => {
                let text = e.text.trim().to_string();
                if text.is_empty() {
                    return;
                }
                self.snapshot();
                let color = self.pen_color.clone();
                let target = image.and_then(|id| self.board.images.iter_mut().find(|i| i.id == id));
                if let Some(img) = target {
                    let (sx, sy) = img.scale();
                    img.annotations.texts.push(Label {
                        id: new_id(),
                        x: (e.world.x - img.x) / sx,
                        y: (e.world.y - img.y) / sy,
                        text,
                        color,
                        font_size: self.text_size / sx,
                        style: self.text_style,
                        ..Default::default()
                    });
                } else {
                    let id = new_id();
                    self.board.world_texts.push(Label {
                        id: id.clone(),
                        x: e.world.x,
                        y: e.world.y,
                        text,
                        color,
                        font_size: self.text_size,
                        style: self.text_style,
                        ..Default::default()
                    });
                    self.select_only((Kind::Text, id));
                }
            }
            EditTarget::ImageLabel { image, label } => {
                let text = e.text.trim().to_string();
                self.snapshot();
                if let Some(img) = self.board.images.iter_mut().find(|i| i.id == image) {
                    if text.is_empty() {
                        img.annotations.texts.retain(|l| l.id != label);
                        self.sel_label = None;
                    } else if let Some(l) = img.annotations.texts.iter_mut().find(|l| l.id == label) {
                        l.text = text;
                    }
                }
            }
            EditTarget::WorldText(id) => {
                let text = e.text.trim().to_string();
                self.snapshot();
                if text.is_empty() {
                    self.board.world_texts.retain(|t| t.id != id);
                    self.deselect_all();
                } else if let Some(t) = self.board.world_texts.iter_mut().find(|t| t.id == id) {
                    t.text = text;
                }
            }
            EditTarget::Note(id) => {
                self.snapshot();
                if let Some(n) = self.board.sticky_notes.iter_mut().find(|n| n.id == id) {
                    n.text = e.text;
                }
            }
            EditTarget::Group(id) => {
                self.snapshot();
                if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                    g.title = e.text.trim().to_string();
                }
            }
        }
    }

    /// Deletes the files of images that were removed from this board during this
    /// session. Only called once undo can no longer bring them back.
    pub(crate) fn drop_deleted_assets(&mut self) {
        let live: HashSet<&str> = self.board.images.iter().map(|i| i.asset_file_name.as_str()).collect();
        let mut gone: HashSet<String> = HashSet::new();
        for b in self.undo.iter().chain(self.redo.iter()) {
            for i in &b.images {
                if !live.contains(i.asset_file_name.as_str()) {
                    gone.insert(i.asset_file_name.clone());
                }
            }
        }
        for f in gone {
            let _ = std::fs::remove_file(asset_path(&self.board.id, &f));
        }
    }

    pub(crate) fn switch_board(&mut self, id: &str) {
        self.finish_dying();
        self.land_glides();
        self.save_now();
        self.drop_deleted_assets();
        self.tex_lvl.clear();
        self.mosaics.clear();
        self.textures.clear();
        self.missing.clear();
        self.undo.clear();
        self.redo.clear();
        self.deselect_all();
        self.edit = None;
        self.board = load_board(id);
        // the new board's items fade in rather than pop (audit #33)
        crate::canvas::presence::enter(crate::canvas::presence::board_ids(&self.board), self.now, 0.0);
        self.settings.active_board_id = id.to_string();
        save_settings(&self.settings);
        self.toast(format!("Board: {}", self.board.name));
    }

    pub(crate) fn zoom_fit(&mut self, rect: Rect) {
        let b = &self.board;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut add = |x: f32, y: f32, w: f32, h: f32| {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + w);
            y1 = y1.max(y + h);
        };
        b.images.iter().for_each(|i| add(i.x, i.y, i.width, i.height));
        b.sticky_notes.iter().for_each(|i| add(i.x, i.y, i.width, i.height));
        b.link_cards.iter().for_each(|i| add(i.x, i.y, i.width, i.height));
        b.group_cards.iter().for_each(|i| add(i.x, i.y, i.width, i.height));
        b.world_texts.iter().for_each(|t| add(t.x, t.y, 100.0, 20.0));
        b.world_strokes.iter().flat_map(|s| s.points.iter()).for_each(|p| add(p.x, p.y, 0.0, 0.0));
        if x0 > x1 {
            self.board.camera = Camera::default();
        } else {
            let (bw, bh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
            let zoom = (rect.width() / bw).min(rect.height() / bh) * 0.9;
            let zoom = zoom.clamp(0.02, 1.5);
            self.board.camera = Camera {
                x: (x0 + x1) / 2.0 - rect.width() / zoom / 2.0,
                y: (y0 + y1) / 2.0 - rect.height() / zoom / 2.0,
                zoom,
            };
        }
        self.mark_dirty();
    }

    pub(crate) fn begin_hide(&mut self, ctx: &egui::Context) {
        self.commit_edit();
        self.save_now();
        super::saver::flush();
        // Drop every texture, then let one more frame run so the GPU memory is really freed
        self.textures.clear();
        self.hide_in = 2;
        ctx.request_repaint();
    }

    pub(crate) fn quit(&mut self, ctx: &egui::Context) {
        self.commit_edit();
        self.save_now();
        super::saver::flush();
        self.drop_deleted_assets();
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub(crate) fn open_settings(&mut self) {
        self.panel_open = !self.panel_open;
    }

    // ---------- input ----------
}
