//! Image layer names: pasted / screenshotted pictures are "Image 1", "Image 2", ... (the number
//! never repeats while a higher one exists); a dropped file keeps its own name unless that is a
//! random id like "18e7d62acfb7057c-5".
use crate::model::Board;

/// True for names that are just an id: hex digits and separators, no real word
pub fn junk_name(stem: &str) -> bool {
    let core: String = stem.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    core.is_empty() || (core.len() >= 6 && core.chars().all(|c| c.is_ascii_hexdigit()) && core.chars().any(|c| c.is_ascii_digit()))
}

impl Board {
    fn next_image_number(&self) -> u32 {
        self.layer_names
            .values()
            .filter_map(|n| {
                let n = n.trim();
                n.get(..6).filter(|p| p.eq_ignore_ascii_case("image "))?;
                n[6..].trim().parse::<u32>().ok()
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Give a new picture its layer name: the file's own name if it has a real one, else Image N
    pub fn name_image(&mut self, id: &str, source: Option<&std::path::Path>) {
        let own = source
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().trim().to_string())
            .filter(|s| !junk_name(s));
        let name = own.unwrap_or_else(|| format!("Image {}", self.next_image_number()));
        self.layer_names.insert(id.to_string(), name);
    }

    /// Older boards: pictures that only had their random file name get Image N (oldest first)
    pub fn name_unnamed_images(&mut self) {
        let mut ids: Vec<(f64, String)> = self
            .images
            .iter()
            .filter(|i| self.layer_names.get(&i.id).map_or(true, |n| n.trim().is_empty()))
            .map(|i| (i.created_at, i.id.clone()))
            .collect();
        ids.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, id) in ids {
            self.name_image(&id, None);
        }
    }
}
