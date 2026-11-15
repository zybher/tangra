//! Act dispatch: every menu/toolbar/shortcut action lands in `apply`.

use crate::*;

impl App {
    pub(crate) fn apply(&mut self, act: Act, ctx: &egui::Context, rect: Rect) {
        match act {
            Act::FrameAspect(ratio) => {
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                if !ids.is_empty() {
                    self.snapshot();
                    for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                        g.height = (g.width / ratio).round();
                    }
                }
            }
            // Arrange "None": stop auto-layout, leave everything where it is
            Act::FrameLayout(255) => {
                self.snapshot();
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                    g.layout = 0;
                }
                self.mark_dirty();
            }
            Act::FrameLayout(_) | Act::ArrangeAuto(_) => {
                // Rapid switching: land the previous glide first so sizes never compound
                if let Some((_, tw)) = ctx.data(|d| d.get_temp::<Tween>(egui::Id::new("qd_layout_tw"))) {
                    for (frame, id, _, b) in tw {
                        if frame {
                            if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                                (g.x, g.y, g.width, g.height) = (b[0], b[1], b[2], b[3]);
                            }
                        } else if let Some(im) = self.board.images.iter_mut().find(|i| i.id == id) {
                            (im.x, im.y, im.width, im.height) = (b[0], b[1], b[2], b[3]);
                        }
                    }
                    ctx.data_mut(|d| d.remove::<Tween>(egui::Id::new("qd_layout_tw")));
                }
                let jobs: Vec<(String, u8)> = match &act {
                    Act::FrameLayout(m) => {
                        self.snapshot();
                        let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                        for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                            g.layout = m + 1;
                        }
                        ids.into_iter().map(|i| (i, *m)).collect()
                    }
                    Act::ArrangeAuto(id) => self.board.group_cards.iter().filter(|g| &g.id == id && g.layout > 0).map(|g| (g.id.clone(), g.layout - 1)).collect(),
                    _ => Vec::new(),
                };
                // (is_frame, id, from, to): everything glides into place over a few frames
                let mut tw: Vec<(bool, String, [f32; 4], [f32; 4])> = Vec::new();
                for (gid, mode) in jobs {
                    let Some(gi) = self.board.group_cards.iter().position(|g| g.id == gid) else { continue };
                    let g = self.board.group_cards[gi].clone();
                    let mut idx: Vec<usize> = (0..self.board.images.len())
                        .filter(|&i| {
                            let im = &self.board.images[i];
                            let (cx, cy) = (im.x + im.width / 2.0, im.y + im.height / 2.0);
                            // Stored frame membership decides; centre position only for objects not stamped yet
                            self.board.parents.get(&im.id).map_or(cx > g.x && cx < g.x + g.width && cy > g.y && cy < g.y + g.height, |p| p == &g.id)
                        })
                        .collect();
                    if idx.is_empty() {
                        continue;
                    }
                    // Reading order: rows top to bottom, then left to right
                    idx.sort_by(|&a, &b| {
                        let (ia, ib) = (&self.board.images[a], &self.board.images[b]);
                        ((ia.y / 60.0) as i32, ia.x as i32).cmp(&((ib.y / 60.0) as i32, ib.x as i32))
                    });
                    let asp: Vec<f32> = idx.iter().map(|&i| self.board.images[i].width.max(1.0) / self.board.images[i].height.max(1.0)).collect();
                    let med = |mut v: Vec<f32>| -> f32 {
                        v.sort_by(|a, b| a.total_cmp(b));
                        v[v.len() / 2]
                    };
                    let (pad, gap) = (24.0_f32, 12.0_f32);
                    let n = idx.len();
                    // One size unit per frame, picked the first time it is arranged, so switching layouts keeps image size
                    let masp = med(asp.clone());
                    let unit = if g.layout_unit > 0.0 { g.layout_unit } else { med(idx.iter().map(|&i| self.board.images[i].height).collect()).clamp(80.0, 600.0) };
                    let mut out: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(n);
                    let (fw, fh);
                    match mode {
                        // Single row, shared height
                        0 => {
                            let h = unit;
                            let mut x = pad;
                            for a in &asp {
                                out.push((x, pad, h * a, h));
                                x += h * a + gap;
                            }
                            fw = x - gap + pad;
                            fh = h + pad * 2.0;
                        }
                        // Single column, shared width
                        1 => {
                            let w = (unit * masp).clamp(80.0, 800.0);
                            let mut y = pad;
                            for a in &asp {
                                out.push((pad, y, w, w / a));
                                y += w / a + gap;
                            }
                            fw = w + pad * 2.0;
                            fh = y - gap + pad;
                        }
                        // Square grid: 1:1 cells, columns grow so the grid stays square
                        2 => {
                            let cols = (n as f32).sqrt().ceil() as usize;
                            let rows = (n + cols - 1) / cols;
                            let c = (unit * masp.max(1.0)).clamp(80.0, 500.0);
                            for (k, a) in asp.iter().enumerate() {
                                let (w, h) = if *a >= 1.0 { (c, c / a) } else { (c * a, c) };
                                let (cx, cy) = (pad + (k % cols) as f32 * (c + gap), pad + (k / cols) as f32 * (c + gap));
                                out.push((cx + (c - w) / 2.0, cy + (c - h) / 2.0, w, h));
                            }
                            fw = pad * 2.0 + cols as f32 * c + (cols - 1) as f32 * gap;
                            fh = pad * 2.0 + rows as f32 * c + (rows - 1) as f32 * gap;
                        }
                        // Justified rows across the current frame width, like a photo gallery
                        _ => {
                            let h0 = unit;
                            let per_row = (n as f32).sqrt().ceil();
                            let inner = (g.width - pad * 2.0).max(h0 * masp * per_row + gap * (per_row - 1.0)).max(200.0);
                            let mut y = pad;
                            let mut k = 0;
                            while k < n {
                                let mut j = k;
                                let mut sum = 0.0;
                                while j < n {
                                    sum += asp[j];
                                    j += 1;
                                    if sum * h0 + (j - k - 1) as f32 * gap >= inner {
                                        break;
                                    }
                                }
                                let gaps = (j - k - 1) as f32 * gap;
                                let full = sum * h0 + gaps >= inner;
                                let h = if full { (inner - gaps) / sum } else { h0 };
                                let mut x = pad;
                                for a in &asp[k..j] {
                                    out.push((x, y, h * a, h));
                                    x += h * a + gap;
                                }
                                y += h + gap;
                                k = j;
                            }
                            fw = inner + pad * 2.0;
                            fh = y - gap + pad;
                        }
                    }
                    for (k, &i) in idx.iter().enumerate() {
                        let (x, y, w, h) = out[k];
                        let im = &self.board.images[i];
                        tw.push((false, im.id.clone(), [im.x, im.y, im.width, im.height], [g.x + x, g.y + y, w, h]));
                    }
                    tw.push((true, g.id.clone(), [g.x, g.y, g.width, g.height], [g.x, g.y, fw, fh]));
                    self.board.group_cards[gi].layout_unit = unit;
                }
                if !tw.is_empty() {
                    ctx.data_mut(|d| d.insert_temp::<Tween>(egui::Id::new("qd_layout_tw"), (self.now, tw)));
                    ctx.request_repaint();
                }
            }
            Act::FitFrame => self.fit_frames_to_content(),
            Act::ImageCorner(v, first) => {
                if first {
                    self.snapshot();
                }
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Image).map(|s| s.1.clone()).collect();
                for i in self.board.images.iter_mut().filter(|i| ids.contains(&i.id)) {
                    i.corner = v;
                }
            }
            Act::StrokePos(v) => {
                self.snapshot();
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                    g.stroke_pos = v;
                }
            }
            Act::ToggleClip => {
                self.snapshot();
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                    g.clip = !g.clip;
                }
            }
            Act::TextFontSize(sz) => {
                self.snapshot();
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Text).map(|s| s.1.clone()).collect();
                for t in self.board.world_texts.iter_mut().filter(|t| ids.contains(&t.id)) {
                    t.font_size = sz;
                }
                if let Some((l, sx)) = self.sel_label_mut() {
                    l.font_size = sz / sx.max(0.01);
                }
            }
            Act::ResetStretch => {
                if let Some((_, id)) = self.menu_hit.clone() {
                    self.snapshot();
                    if let Some(i) = self.board.images.iter_mut().find(|i| i.id == id) {
                        // keep the width, restore the original aspect around the same centre
                        // same overall size (geometric mean of the two scales), same centre
                        let (cx, cy) = (i.x + i.width / 2.0, i.y + i.height / 2.0);
                        if i.original_width > 0.0 && i.original_height > 0.0 {
                            let (sx, sy) = i.scale();
                            // back to the size it had before stretching (fallback: the smaller side's scale)
                            let k = i.base_scale.take().unwrap_or(sx.min(sy));
                            (i.width, i.height) = (i.original_width * k, i.original_height * k);
                        }
                        (i.x, i.y) = (cx - i.width / 2.0, cy - i.height / 2.0);
                    }
                    self.mark_dirty();
                }
            }
            Act::Corner(v) => {
                self.snapshot();
                for (k, id) in self.selected.clone() {
                    match k {
                        Kind::Group => {
                            if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                                g.corner = Some(v);
                            }
                        }
                        Kind::Stroke => {
                            if let Some(st) = self.board.world_strokes.iter_mut().find(|st| st.id == id) {
                                st.radius = Some(v);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Act::Border(v) => {
                self.snapshot();
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Group).map(|s| s.1.clone()).collect();
                for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                    g.border = Some(v);
                }
            }
            Act::StrokeWidth(w) => {
                self.snapshot();
                self.pen_width = w;
                let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Stroke).map(|s| s.1.clone()).collect();
                let b = &mut self.board;
                let strokes = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
                for s in strokes.filter(|s| ids.contains(&s.id)) {
                    s.width = w;
                }
            }
            Act::ToggleFill => self.toggle_style(true),
            Act::ToggleStroke => self.toggle_style(false),
            Act::CentreMenu => self.menu_world = self.view(rect).w(rect.center()),
            Act::CycleColor => {
                let next = PEN_COLORS
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(&self.pen_color))
                    .map_or(0, |i| (i + 1) % PEN_COLORS.len());
                self.pen_color = PEN_COLORS[next].to_string();
            }
            Act::Paste => self.paste_at(ctx, self.menu_world, false),
            Act::Tool(t) => {
                self.tool = t;
                if t != Tool::Select {
                    self.deselect_all();
                }
            }
            Act::Color(c) => {
                self.pen_color = c.clone();
                let ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| matches!(k, Kind::Text | Kind::Stroke))
                    .map(|(_, id)| id.clone())
                    .collect();
                if self.sel_label.is_some() {
                    self.snapshot();
                    if let Some((l, _)) = self.sel_label_mut() {
                        l.color = c.clone();
                    }
                } else if !ids.is_empty() {
                    // Recolouring a selection; stay on the current tool
                    self.snapshot();
                    let b = &mut self.board;
                    for t in b.world_texts.iter_mut().filter(|t| ids.contains(&t.id)) {
                        t.color = c.clone();
                    }
                    let strokes = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
                    for s in strokes.filter(|s| ids.contains(&s.id)) {
                        s.color = c.clone();
                    }
                } else if self.tool == Tool::Select || self.tool == Tool::Eraser {
                    self.tool = Tool::Pen;
                }
            }
            Act::Width(w) => self.pen_width = w,
            Act::ToolMenu(k, r) => {
                self.tool_menu = if self.tool_menu.map(|m| m.0) == Some(k) { None } else { Some((k, r)) };
                self.tool_sub = None;
            }
            Act::AddNote => {
                self.snapshot();
                let id = self.board.add_note("", self.menu_world);
                self.select_only((Kind::Note, id.clone()));
                self.activate((Kind::Note, id));
            }
            Act::AddGroup => {
                self.snapshot();
                let id = self.board.add_group(self.menu_world);
                let (_, fw, fh) = FRAME_SIZES[self.frame_size];
                if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                    g.x = self.menu_world.x - fw / 2.0;
                    g.y = self.menu_world.y - fh / 2.0;
                    g.width = fw;
                    g.height = fh;
                    g.square = self.frame_square;
                }
                self.select_only((Kind::Group, id));
            }
            Act::LabelHere => self.begin_label(self.menu_world),
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::ZoomReset => {
                let v = self.view(rect);
                let c = v.w(rect.center());
                let cam = &mut self.board.camera;
                cam.zoom = 1.0;
                cam.x = c.x - rect.width() / 2.0;
                cam.y = c.y - rect.height() / 2.0;
                self.mark_dirty();
            }
            Act::ZoomFit => self.zoom_fit(rect),
            Act::SwitchBoard(id) => self.switch_board(&id),
            Act::NewBoard if !PROMPT_BUSY.load(std::sync::atomic::Ordering::Relaxed) => self.prompt = Some(Prompt::NewBoard(String::new())),
            Act::NewBoard => {}
            Act::RenameBoard => self.prompt = Some(Prompt::Rename(self.board.name.clone())),
            Act::DeleteBoard => self.prompt = Some(Prompt::ConfirmDelete),
            Act::Settings => self.open_settings(),
            Act::Hide => self.begin_hide(ctx),
            Act::Quit => self.quit(ctx),
            Act::CopyBaked => self.copy_image(true),
            Act::CopyClean => self.copy_image(false),
            Act::ClearAnnotations => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Image)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(i) = self.board.images.iter_mut().find(|i| i.id == id) {
                            i.annotations = Annotations::default();
                        }
                    }
                }
            }
            Act::Delete => self.delete_selected(),
            Act::Activate => {
                if let Some(l) = self.sel_label.clone() {
                    self.edit_label(l);
                } else if let Some(s) = self.sel.clone() {
                    self.activate(s);
                }
            }
            Act::NoteColor(c) => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Note)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(n) = self.board.sticky_notes.iter_mut().find(|n| n.id == id) {
                            n.color = c.clone();
                        }
                    }
                }
            }
            Act::NoteFontSize(sz) => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Note)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(n) = self.board.sticky_notes.iter_mut().find(|n| n.id == id) {
                            n.font_size = sz;
                        }
                    }
                }
            }
            Act::NoteOpacity(op) => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Note)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(n) = self.board.sticky_notes.iter_mut().find(|n| n.id == id) {
                            n.opacity = op;
                        }
                    }
                }
            }
            Act::GroupColor(c) => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Group)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                            g.color = c.clone();
                        }
                    }
                }
            }
            Act::GroupFontSize(sz) => {
                let target_ids: Vec<String> = self.selected.iter()
                    .filter(|(k, _)| *k == Kind::Group)
                    .map(|(_, id)| id.clone())
                    .collect();
                if !target_ids.is_empty() {
                    self.snapshot();
                    for id in target_ids {
                        if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                            g.font_size = sz;
                        }
                    }
                }
            }
            Act::Front => self.bring_to_front(),
        }
    }
}
