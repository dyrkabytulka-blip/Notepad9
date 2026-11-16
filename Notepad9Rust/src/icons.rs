//! Toolbar / menu icons embedded at build time (pixel-exact PNGs from tools/make_icons.py).

use eframe::egui::{self, ColorImage, Context, Pos2, Rect, TextureHandle, TextureId, TextureOptions, Vec2};
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/icons_gen.rs"));

const SIZES: [u32; 8] = [16, 20, 24, 28, 32, 40, 48, 64];

#[derive(Default)]
pub struct Icons {
    cache: HashMap<String, Option<TextureHandle>>,
}

impl Icons {
    /// Texture for `name` shown at `size` points: picks the bitmap made for the current scale.
    pub fn get(&mut self, ctx: &Context, name: &str, size: f32, enabled: bool) -> Option<(TextureId, f32)> {
        let want = size * ctx.pixels_per_point();
        let px = SIZES.iter().copied().find(|&s| s as f32 >= want - 0.5).unwrap_or(64);
        let key = format!("{name}_{px}{}", if enabled || name == "app" { "" } else { "_d" });
        let tex = self.cache.entry(key.clone()).or_insert_with(|| load(ctx, &key)).as_ref()?;
        Some((tex.id(), px as f32 / ctx.pixels_per_point()))
    }

    /// Paint an icon centered in `rect`, snapped to whole pixels so it stays crisp.
    pub fn paint(&mut self, ui: &egui::Ui, name: &str, rect: Rect, size: f32, enabled: bool) {
        if let Some((id, pts)) = self.get(ui.ctx(), name, size, enabled) {
            let ppp = ui.ctx().pixels_per_point();
            let c = rect.center();
            let min = Pos2::new(((c.x - pts / 2.0) * ppp).round() / ppp, ((c.y - pts / 2.0) * ppp).round() / ppp);
            let r = Rect::from_min_size(min, Vec2::splat(pts));
            ui.painter().image(id, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
        }
    }

    pub fn image(&mut self, ctx: &Context, name: &str, size: f32, enabled: bool) -> Option<egui::Image<'static>> {
        let (id, pts) = self.get(ctx, name, size, enabled)?;
        Some(egui::Image::new((id, Vec2::splat(pts.min(size + 0.01)))).fit_to_exact_size(Vec2::splat(size)))
    }
}

fn load(ctx: &Context, key: &str) -> Option<TextureHandle> {
    let bytes = icon_bytes(key)?;
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ctx.load_texture(key, ColorImage::from_rgba_unmultiplied(size, img.as_raw()), TextureOptions::NEAREST))
}

pub fn app_icon() -> Option<egui::IconData> {
    let bytes = icon_bytes("app_64").or_else(|| icon_bytes("app_48"))?;
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    Some(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() })
}
