use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ImageCache {
    base: PathBuf,
    textures: HashMap<String, TextureHandle>,
}

impl ImageCache {
    pub fn new(base: &Path) -> Self {
        ImageCache {
            base: base.to_path_buf(),
            textures: HashMap::new(),
        }
    }

    pub fn get(
        &mut self,
        key: &str,
        ctx: &egui::Context,
    ) -> Option<&TextureHandle> {
        if !self.textures.contains_key(key) {
            let path = self.base.join(format!("{key}.png"));
            if let Ok(img) = image::open(&path) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();
                let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
                let handle = ctx.load_texture(key, color_image, TextureOptions::LINEAR);
                self.textures.insert(key.to_string(), handle);
            }
        }
        self.textures.get(key)
    }

    pub fn get_size(&self, key: &str) -> Option<(u32, u32)> {
        let path = self.base.join(format!("{key}.png"));
        let img = image::open(&path).ok()?;
        Some((img.width(), img.height()))
    }
}
