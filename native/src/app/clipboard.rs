//! Copy/paste of canvas objects and images, file drops, duplicate.

use crate::*;

impl App {
    /// Ctrl+C on frames/shapes/notes: keep a copy in memory and put a marker on the system
    /// clipboard, so a newer screenshot still wins on paste
    pub(crate) fn copy_objects(&mut self, ctx: &egui::Context) {
        let mut sel = self.selected.clone();
        for (k, id) in &self.selected {
            if *k != Kind::Group {
                continue;
            }
            let Some([gx, gy, gw, gh]) = self.rect_of(Kind::Group, id) else { continue };
            let inside = |x: f32, y: f32| x > gx && x < gx + gw && y > gy && y < gy + gh;
            let b = &self.board;
            b.images.iter().filter(|i| inside(i.x + i.width / 2.0, i.y + i.height / 2.0)).for_each(|i| { sel.insert((Kind::Image, i.id.clone())); });
            b.sticky_notes.iter().filter(|i| inside(i.x + i.width / 2.0, i.y + i.height / 2.0)).for_each(|i| { sel.insert((Kind::Note, i.id.clone())); });
            b.link_cards.iter().filter(|i| inside(i.x + i.width / 2.0, i.y + i.height / 2.0)).for_each(|i| { sel.insert((Kind::Link, i.id.clone())); });
            b.world_texts.iter().filter(|i| inside(i.x, i.y)).for_each(|i| { sel.insert((Kind::Text, i.id.clone())); });
            b.world_strokes.iter().filter(|s| pts_bbox(&s.points).is_some_and(|[x, y, w, h]| inside(x + w / 2.0, y + h / 2.0))).for_each(|s| { sel.insert((Kind::Stroke, s.id.clone())); });
        }
        let has = |k: Kind, id: &String| sel.contains(&(k, id.clone()));
        let b = &self.board;
        let mut c = ObjClip {
            nonce: new_id(),
            images: b.images.iter().filter(|i| has(Kind::Image, &i.id)).cloned().collect(),
            notes: b.sticky_notes.iter().filter(|i| has(Kind::Note, &i.id)).cloned().collect(),
            links: b.link_cards.iter().filter(|i| has(Kind::Link, &i.id)).cloned().collect(),
            groups: b.group_cards.iter().filter(|i| has(Kind::Group, &i.id)).cloned().collect(),
            texts: b.world_texts.iter().filter(|i| has(Kind::Text, &i.id)).cloned().collect(),
            strokes: b.world_strokes.iter().filter(|i| has(Kind::Stroke, &i.id)).cloned().collect(),
            center: Pt { x: 0.0, y: 0.0 },
        };
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for s in &sel {
            if let Some([x, y, w, h]) = self.rect_of(s.0, &s.1) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
            }
        }
        if x0 > x1 {
            return;
        }
        c.center = Pt { x: (x0 + x1) / 2.0, y: (y0 + y1) / 2.0 };
        let marker = format!("Tangra objects {}", c.nonce);
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(marker);
        }
        let n = sel.len();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_objclip"), c));
        self.toast(format!("Copied {n} item{}", if n == 1 { "" } else { "s" }));
    }

    /// Leaves a copy of each item where it is, just beneath the original (Alt+drag)
    pub(crate) fn duplicate_in_place(&mut self, items: &HashSet<(Kind, String)>) {
        fn dup<T: Clone>(v: &mut Vec<T>, ids: &[&String], id_of: impl Fn(&T) -> &String, set_id: impl Fn(&mut T, String), pairs: &mut Vec<(String, String)>) {
            let mut i = 0;
            while i < v.len() {
                if ids.contains(&id_of(&v[i])) {
                    let mut c = v[i].clone();
                    let n = new_id();
                    pairs.push((id_of(&v[i]).clone(), n.clone()));
                    set_id(&mut c, n);
                    v.insert(i, c);
                    i += 1;
                }
                i += 1;
            }
        }
        let ids = |k: Kind| items.iter().filter(|s| s.0 == k).map(|s| &s.1).collect::<Vec<_>>();
        let b = &mut self.board;
        let mut pairs: Vec<(Kind, Vec<(String, String)>)> = Vec::new();
        for k in [Kind::Image, Kind::Note, Kind::Link, Kind::Group, Kind::Text, Kind::Stroke] {
            let mut p = Vec::new();
            match k {
                Kind::Image => dup(&mut b.images, &ids(k), |x| &x.id, |x, n| {
                    x.id = n;
                    x.annotations.fresh_ids();
                }, &mut p),
                Kind::Note => dup(&mut b.sticky_notes, &ids(k), |x| &x.id, |x, n| x.id = n, &mut p),
                Kind::Link => dup(&mut b.link_cards, &ids(k), |x| &x.id, |x, n| x.id = n, &mut p),
                Kind::Group => dup(&mut b.group_cards, &ids(k), |x| &x.id, |x, n| x.id = n, &mut p),
                Kind::Text => dup(&mut b.world_texts, &ids(k), |x| &x.id, |x, n| x.id = n, &mut p),
                Kind::Stroke => dup(&mut b.world_strokes, &ids(k), |x| &x.id, |x, n| x.id = n, &mut p),
            }
            pairs.push((k, p));
        }
        // The copy left behind keeps the name; the one you're holding gets the next number
        for (k, p) in pairs {
            for (orig, copy) in p {
                self.number_duplicate(k, &orig, &copy);
            }
        }
        self.mark_dirty();
    }

    pub(crate) fn paste_objects(&mut self, c: ObjClip, world: Pt) {
        self.snapshot();
        let (dx, dy) = (world.x - c.center.x, world.y - c.center.y);
        let mut new_sel = HashSet::new();
        for mut i in c.images {
            (i.id, i.x, i.y) = (new_id(), i.x + dx, i.y + dy);
            i.annotations.fresh_ids();
            new_sel.insert((Kind::Image, i.id.clone()));
            self.board.images.push(i);
        }
        for mut i in c.notes {
            (i.id, i.x, i.y) = (new_id(), i.x + dx, i.y + dy);
            new_sel.insert((Kind::Note, i.id.clone()));
            self.board.sticky_notes.push(i);
        }
        for mut i in c.links {
            (i.id, i.x, i.y) = (new_id(), i.x + dx, i.y + dy);
            new_sel.insert((Kind::Link, i.id.clone()));
            self.board.link_cards.push(i);
        }
        for mut i in c.groups {
            (i.id, i.x, i.y) = (new_id(), i.x + dx, i.y + dy);
            new_sel.insert((Kind::Group, i.id.clone()));
            self.board.group_cards.push(i);
        }
        for mut i in c.texts {
            (i.id, i.x, i.y) = (new_id(), i.x + dx, i.y + dy);
            new_sel.insert((Kind::Text, i.id.clone()));
            self.board.world_texts.push(i);
        }
        for mut i in c.strokes {
            i.id = new_id();
            for q in &mut i.points {
                (q.x, q.y) = (q.x + dx, q.y + dy);
            }
            new_sel.insert((Kind::Stroke, i.id.clone()));
            self.board.world_strokes.push(i);
        }
        self.sel = new_sel.iter().find(|s| s.0 == Kind::Group).or(new_sel.iter().next()).cloned();
        self.selected = new_sel;
        self.tool = Tool::Select;
    }

    pub(crate) fn paste_at(&mut self, ctx: &egui::Context, world: Pt, quiet: bool) {
        let clip = platform::read_clipboard();
        if let Clip::Text(t) = &clip {
            if let Some(c) = ctx.data(|d| d.get_temp::<ObjClip>(egui::Id::new("qd_objclip"))) {
                if t.trim() == format!("Tangra objects {}", c.nonce) {
                    self.paste_objects(c, world);
                    return;
                }
            }
        }
        if let Clip::Image { w, h, .. } = &clip {
            let mine = IMG_CLIP.lock().ok().and_then(|g| g.as_ref().filter(|c| c.0 == *w && c.1 == *h).map(|c| c.2.clone()));
            if let Some(c) = mine {
                self.paste_objects(c, world);
                return;
            }
        }
        match clip {
            Clip::Image { w, h, rgba } => {
                let file = format!("{}.png", new_id());
                if let Err(e) = platform::save_png(&asset_path(&self.board.id, &file), &rgba, w, h) {
                    self.toast(format!("Paste failed: {e}"));
                    return;
                }
                self.put_texture(ctx, &file, w, h, rgba);
                self.snapshot();
                // The frame under the cursor receives the paste (selected or not); empty canvas
                // under the cursor means it lands right there, even if a frame is selected
                let frame = (!quiet)
                    .then(|| self.board.group_cards.iter().rev()
                        .find(|g| world.x >= g.x && world.x <= g.x + g.width && world.y >= g.y && world.y <= g.y + g.height)
                        .map(|g| g.id.clone()))
                    .flatten();
                let id = self.board.add_image(file, w, h, world);
                match frame {
                    Some(g) => {
                        self.place_in_frame(&g, &id);
                        self.select_only((Kind::Group, g));
                    }
                    None => self.select_only((Kind::Image, id)),
                }
                if !quiet {
                    self.tool = Tool::Select;
                }
            }
            Clip::Url(u) => {
                self.snapshot();
                let id = self.board.add_link(&u, world);
                self.select_only((Kind::Link, id));
            }
            Clip::Text(t) => {
                self.snapshot();
                let id = self.board.add_note(&t, world);
                self.select_only((Kind::Note, id));
            }
            Clip::Empty => {
                self.toast("Nothing to paste");
                return;
            }
        }
        if quiet {
            self.toast("Quick-dump captured");
        }
    }

    pub(crate) fn drop_files(&mut self, ctx: &egui::Context, world: Pt, paths: &[std::path::PathBuf]) {
        let mut ids = Vec::new();
        let mut skipped = 0;
        let mut at = world;
        for path in paths {
            let Ok((w, h, rgba)) = platform::load_rgba(path) else {
                skipped += 1;
                continue;
            };
            let file = format!("{}.png", new_id());
            if platform::save_png(&asset_path(&self.board.id, &file), &rgba, w, h).is_err() {
                skipped += 1;
                continue;
            }
            if ids.is_empty() {
                self.snapshot();
            }
            self.put_texture(ctx, &file, w, h, rgba);
            let id = self.board.add_image(file, w, h, at);
            // A file with a real name keeps it; a random-id name stays Image N
            if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()).filter(|s| !crate::image_names::junk_name(s)) {
                self.board.layer_names.insert(id.clone(), stem);
            }
            ids.push(id);
            at = Pt { x: at.x + 32.0, y: at.y + 32.0 };
        }
        // Dropped onto a frame: file them into it. Otherwise they stay where they were dropped,
        // whatever happens to be selected.
        let frame = self.board.group_cards.iter().rev().find(|g| world.x >= g.x && world.y >= g.y && world.x <= g.x + g.width && world.y <= g.y + g.height).map(|g| g.id.clone());
        if let Some(g) = &frame {
            for id in &ids {
                self.place_in_frame(g, id);
            }
        }
        if !ids.is_empty() {
            self.selected.clear();
            for id in &ids {
                self.selected.insert((Kind::Image, id.clone()));
            }
            self.sel = Some((Kind::Image, ids[ids.len() - 1].clone()));
            self.tool = Tool::Select;
        }
        if skipped > 0 {
            self.toast(if ids.is_empty() { "Not an image file".to_string() } else { format!("Skipped {skipped} non-image file(s)") });
        }
    }

    pub(crate) fn copy_image(&mut self, baked: bool) {
        let Some((Kind::Image, id)) = self.sel.clone() else { return };
        let Some(img) = self.board.images.iter().find(|i| i.id == id).cloned() else { return };
        match platform::load_rgba(&asset_path(&self.board.id, &img.asset_file_name)) {
            Ok((mut w, mut h, mut rgba)) => {
                // Bake at 2x for normal-sized images so ink stays as crisp as it looks on canvas
                if baked && w.max(h) <= 2000 {
                    if let Some(im) = image::RgbaImage::from_raw(w, h, rgba.clone()) {
                        rgba = image::imageops::resize(&im, w * 2, h * 2, image::imageops::FilterType::CatmullRom).into_raw();
                        (w, h) = (w * 2, h * 2);
                    }
                }
                if baked {
                    platform::bake(&img, &mut rgba, w as usize, h as usize);
                }
                match platform::copy_image(w as usize, h as usize, &rgba) {
                    Ok(()) => {
                        let mut one = img.clone();
                        if !baked {
                            one.annotations = Default::default();
                        }
                        let center = Pt { x: one.x + one.width / 2.0, y: one.y + one.height / 2.0 };
                        let c = ObjClip { nonce: new_id(), images: vec![one], notes: vec![], links: vec![], groups: vec![], texts: vec![], strokes: vec![], center };
                        if let Ok(mut g) = IMG_CLIP.lock() {
                            *g = Some((w, h, c));
                        }
                        self.toast(if baked { "Copied with annotations" } else { "Copied original image" });
                    }
                    Err(e) => self.toast(format!("Copy failed: {e}")),
                }
            }
            Err(e) => self.toast(format!("Copy failed: {e}")),
        }
    }
}
