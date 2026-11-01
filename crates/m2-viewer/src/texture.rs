//! Texture preview viewer (dds/tga/jpg/bmp/png).
//!
//! TEMPORARY STAND-IN for the `m2-assets` texture loader: it decodes with
//! `m2_formats::image` directly and builds the Bevy `Image` itself. When
//! m2-assets provides the real loader, replace [`decode_levels`] by it (or
//! register a new viewer for the same extensions: later registrations win).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Mesh3d;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::egui;
use m2_formats::image::{DxtcImage, decode_image};

use crate::viewer::{CameraMode, ContentViewer, MainCamera, OpenFile, ViewerEntity, ViewerInput};

pub struct Level {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4`, R G B A.
    pub rgba: Vec<u8>,
}

pub struct TextureData {
    pub format: String,
    pub levels: Vec<Level>,
}

fn argb_words_to_rgba(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]).collect()
}

/// Decodes a DDS (DXT1/3/5, every mip level the header describes) or any image
/// `decode_image` knows (JPEG, TGA, BMP, PNG) to RGBA8.
pub fn decode_levels(bytes: &[u8]) -> Result<TextureData, String> {
    if bytes.starts_with(b"DDS ") {
        let dds = DxtcImage::load_header(bytes).ok_or("unsupported or malformed DDS (only DXT1/3/5)")?;
        let mut levels = Vec::new();
        for level in 0..dds.mip_map_count {
            let (w, h) = ((dds.width.max(0) as u32) >> level, (dds.height.max(0) as u32) >> level);
            if w == 0 || h == 0 {
                break;
            }
            let Some(comp) = dds.copy(bytes, level as usize) else { break };
            let Some(words) = dds.decompress(level, comp) else { break };
            levels.push(Level { width: w, height: h, rgba: argb_words_to_rgba(&words) });
        }
        if levels.is_empty() {
            return Err("DDS has no decodable level".into());
        }
        return Ok(TextureData { format: format!("DDS {} ({} mips in header)", dds.format_name, dds.mip_map_count), levels });
    }
    let d = decode_image(bytes).ok_or("decode_image failed (unknown or corrupt image)")?;
    let mut rgba = d.bgra;
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let kind = if bytes.starts_with(b"\x89PNG") {
        "PNG"
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        "JPEG"
    } else if bytes.starts_with(b"BM") {
        "BMP"
    } else {
        "TGA"
    };
    Ok(TextureData { format: kind.into(), levels: vec![Level { width: d.width, height: d.height, rgba }] })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Channels {
    #[default]
    Rgba,
    Rgb,
    Alpha,
}

/// Applies the channel toggle to RGBA8 bytes.
pub fn apply_channels(rgba: &[u8], ch: Channels) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        match ch {
            Channels::Rgba => {}
            Channels::Rgb => px[3] = 255,
            Channels::Alpha => {
                let a = px[3];
                px[0] = a;
                px[1] = a;
                px[2] = a;
                px[3] = 255;
            }
        }
    }
    out
}

const FOV: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Default)]
pub struct TextureViewer {
    data: Option<TextureData>,
    error: Option<String>,
    level: usize,
    channels: Channels,
    dirty: bool,
    fit: bool,
    image: Handle<Image>,
    cam_xy: Vec2,
    dist: f32,
    view_h: f32,
}

impl TextureViewer {
    fn make_image(&self) -> Option<Image> {
        let l = self.data.as_ref()?.levels.get(self.level)?;
        // Uploaded as an sRGB view of the raw bytes so the preview shows the file's
        // values unmodified; the data itself is plain RGBA8.
        let mut img = Image::new(
            Extent3d { width: l.width, height: l.height, depth_or_array_layers: 1 },
            TextureDimension::D2,
            apply_channels(&l.rgba, self.channels),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        img.sampler = bevy::image::ImageSampler::nearest();
        Some(img)
    }

    fn fit_distance(w: f32, h: f32, view: Vec2) -> f32 {
        let t = (FOV / 2.0).tan();
        let aspect = (view.x / view.y.max(1.0)).max(0.1);
        (h / 2.0 / t).max(w / 2.0 / (t * aspect)) * 1.1
    }
}

impl ContentViewer for TextureViewer {
    fn id(&self) -> &'static str {
        "texture"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dds", "tga", "jpg", "jpeg", "bmp", "png"]
    }
    fn camera_mode(&self) -> CameraMode {
        CameraMode::Custom
    }

    fn open(&mut self, world: &mut World, file: &OpenFile) {
        *self = TextureViewer::default();
        match decode_levels(&file.bytes) {
            Ok(d) => self.data = Some(d),
            Err(e) => {
                self.error = Some(e);
                return;
            }
        }
        let (w, h) = {
            let l = &self.data.as_ref().unwrap().levels[0];
            (l.width as f32, l.height as f32)
        };
        let img = self.make_image().unwrap();
        self.image = world.resource_mut::<Assets<Image>>().add(img);

        // Checkerboard behind transparent pixels, one tile per 8 texels.
        let (cw, ch) = ((w as u32).div_ceil(8).max(1), (h as u32).div_ceil(8).max(1));
        let mut cdata = Vec::with_capacity((cw * ch * 4) as usize);
        for y in 0..ch {
            for x in 0..cw {
                let v = if (x + y) % 2 == 0 { 70 } else { 110 };
                cdata.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let mut checker = Image::new(
            Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
            TextureDimension::D2,
            cdata,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        checker.sampler = bevy::image::ImageSampler::nearest();
        let checker = world.resource_mut::<Assets<Image>>().add(checker);

        let quad = world.resource_mut::<Assets<Mesh>>().add(Rectangle::new(1.0, 1.0));
        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        let tex_mat = mats.add(StandardMaterial {
            base_color_texture: Some(self.image.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let bg_mat = mats.add(StandardMaterial { base_color_texture: Some(checker), unlit: true, ..default() });
        world.spawn((ViewerEntity, Mesh3d(quad.clone()), MeshMaterial3d(bg_mat), Transform::from_xyz(0.0, 0.0, -0.01).with_scale(Vec3::new(w, h, 1.0))));
        world.spawn((ViewerEntity, Mesh3d(quad), MeshMaterial3d(tex_mat), Transform::from_scale(Vec3::new(w, h, 1.0))));
        self.fit = true;
    }

    fn update(&mut self, world: &mut World) {
        let Some(data) = &self.data else { return };
        let input = *world.resource::<ViewerInput>();
        let view = world
            .query_filtered::<&Camera, With<MainCamera>>()
            .single(world)
            .ok()
            .and_then(|c| c.logical_viewport_size())
            .unwrap_or(Vec2::new(800.0, 600.0));
        self.view_h = view.y;
        if self.fit {
            let l0 = &data.levels[0];
            self.dist = Self::fit_distance(l0.width as f32, l0.height as f32, view);
            self.cam_xy = Vec2::ZERO;
            self.fit = false;
        }
        if input.scene_hovered {
            self.dist = (self.dist * 0.9f32.powf(input.scroll)).clamp(1.0, 100_000.0);
            if input.left || input.right || input.middle {
                let world_per_px = 2.0 * self.dist * (FOV / 2.0).tan() / view.y.max(1.0);
                self.cam_xy += Vec2::new(-input.cursor_delta.x, input.cursor_delta.y) * world_per_px;
            }
        }
        if let Ok(mut t) = world.query_filtered::<&mut Transform, With<MainCamera>>().single_mut(world) {
            *t = Transform::from_xyz(self.cam_xy.x, self.cam_xy.y, self.dist).looking_to(Vec3::NEG_Z, Vec3::Y);
        }
        if self.dirty {
            self.dirty = false;
            if let Some(img) = self.make_image() {
                world.resource_mut::<Assets<Image>>().insert(self.image.id(), img).ok();
            }
        }
    }

    fn ui(&mut self, _world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Texture");
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
            return;
        }
        let Some(data) = &self.data else { return };
        let l = &data.levels[self.level];
        ui.label(format!("Format: {}", data.format));
        ui.label(format!("Size: {}x{}  ({} level(s) decoded, RGBA8)", data.levels[0].width, data.levels[0].height, data.levels.len()));
        ui.label(format!("Showing level {} ({}x{})", self.level, l.width, l.height));
        if data.levels.len() > 1 {
            let mut lv = self.level;
            ui.add(egui::Slider::new(&mut lv, 0..=data.levels.len() - 1).text("mip"));
            if lv != self.level {
                self.level = lv;
                self.dirty = true;
            }
        }
        let mut ch = self.channels;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ch, Channels::Rgba, "RGBA");
            ui.selectable_value(&mut ch, Channels::Rgb, "RGB");
            ui.selectable_value(&mut ch, Channels::Alpha, "A");
        });
        if ch != self.channels {
            self.channels = ch;
            self.dirty = true;
        }
        let t = (FOV / 2.0).tan();
        ui.label(format!("Zoom: {:.0}%", self.view_h / (2.0 * self.dist.max(1.0) * t) * 100.0));
        ui.label("Wheel: zoom, drag: pan");
        if ui.button("Fit (F)").clicked() {
            self.fit = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels() {
        let px = [10, 20, 30, 40];
        assert_eq!(apply_channels(&px, Channels::Rgba), px);
        assert_eq!(apply_channels(&px, Channels::Rgb), [10, 20, 30, 255]);
        assert_eq!(apply_channels(&px, Channels::Alpha), [40, 40, 40, 255]);
    }

    #[test]
    fn dds_dxt1_decodes() {
        // 8x8 DXT1, 1 level, all-zero blocks (black).
        let mut f = vec![0u8; 4 + 124 + 4 * 8];
        f[..4].copy_from_slice(b"DDS ");
        let h = &mut f[4..];
        h[0..4].copy_from_slice(&124u32.to_le_bytes());
        h[4..8].copy_from_slice(&0x1007u32.to_le_bytes()); // CAPS|HEIGHT|WIDTH|PIXELFORMAT
        h[8..12].copy_from_slice(&8u32.to_le_bytes());
        h[12..16].copy_from_slice(&8u32.to_le_bytes());
        h[16..20].copy_from_slice(&32u32.to_le_bytes()); // dwLinearSize
        h[72..76].copy_from_slice(&32u32.to_le_bytes());
        h[76..80].copy_from_slice(&4u32.to_le_bytes());
        h[80..84].copy_from_slice(b"DXT1");
        let t = decode_levels(&f).unwrap();
        assert_eq!(t.levels[0].width, 8);
        assert_eq!(t.levels[0].rgba.len(), 8 * 8 * 4);
        assert!(decode_levels(b"garbage!").is_err());
    }
}
