//! Original `.sub` sprite regions, retaining the parent texture and pixel rect.
//! Path rules match `CGraphicSubImage::OnLoad` in `GrpSubImage.cpp`.
use std::collections::HashMap;
use bevy::{asset::{AssetLoader, LoadContext, io::Reader}, prelude::*};
use crate::{Error, SharedPacks, paths};

#[derive(Asset, TypePath, Debug, Clone)]
pub struct SubImage {
    pub image: Handle<Image>,
    pub rect: Rect,
}

#[derive(Debug, PartialEq)]
pub struct SubImageDescription {
    pub image: String,
    pub relative: bool,
    pub rect: Rect,
}
impl SubImageDescription {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let mut fields = HashMap::new();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let split = line.find(char::is_whitespace).ok_or("missing subimage field value")?;
            fields.insert(line[..split].to_ascii_lowercase(), line[split..].trim().trim_matches('"').to_owned());
        }
        if !fields.get("title").is_some_and(|s| s.eq_ignore_ascii_case("subimage")) {
            return Err("expected title subImage".into());
        }
        let number = |key: &str| -> Result<f32, String> {
            fields.get(key).ok_or_else(|| format!("missing {key}"))?.parse::<i32>()
                .map(|n| n as f32).map_err(|_| format!("invalid {key}"))
        };
        let rect = Rect::new(number("left")?, number("top")?, number("right")?, number("bottom")?);
        if rect.min.x < 0. || rect.min.y < 0. || rect.width() <= 0. || rect.height() <= 0. {
            return Err("invalid subimage rectangle".into());
        }
        let image = fields.remove("image").filter(|s| !s.is_empty()).ok_or("missing image")?;
        Ok(Self { image, relative: fields.get("version").is_some_and(|v| v == "2.0"), rect })
    }

    /// Version 1 searches the common UI directory first; version 2 is relative.
    pub fn resolve(&self, sub_path: &str, exists: impl Fn(&str) -> bool) -> String {
        let image = self.image.replace('\\', "/").to_ascii_lowercase();
        let shared = format!("ymir work/ui/{image}");
        if !self.relative && exists(&shared) { return shared; }
        paths::join(paths::parent(&paths::canonical_asset_path(sub_path)), &image)
    }
}

#[derive(TypePath)]
pub struct SubImageLoader { pub packs: SharedPacks }
impl AssetLoader for SubImageLoader {
    type Asset = SubImage;
    type Settings = ();
    type Error = Error;
    async fn load(&self, reader: &mut dyn Reader, _: &(), context: &mut LoadContext<'_>) -> Result<SubImage, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let description = SubImageDescription::parse(&bytes).map_err(Error::Decode)?;
        let name = description.resolve(&context.path().path().to_string_lossy(), |name| self.packs.exists(paths::virtual_name(name)));
        Ok(SubImage { image: context.load(paths::asset_source_uri(&name)), rect: description.rect })
    }
    fn extensions(&self) -> &[&str] { &["sub"] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_follow_cpp_search_rules() {
        let mut sub = SubImageDescription::parse(b"title subImage\nversion 1.0\nimage \"Atlas.dds\"\nleft 2\ntop 4\nright 34\nbottom 68\n").unwrap();
        assert_eq!(sub.rect.size(), Vec2::new(32.,64.));
        assert_eq!(sub.resolve("locale/en/ui/button.sub", |_| true), "ymir work/ui/atlas.dds");
        assert_eq!(sub.resolve("locale/en/ui/button.sub", |_| false), "locale/en/ui/atlas.dds");
        sub.relative = true;
        assert_eq!(sub.resolve("locale/en/ui/button.sub", |_| true), "locale/en/ui/atlas.dds");
    }
    #[test]
    fn corrupt_regions_fail_before_rendering() {
        assert!(SubImageDescription::parse(b"title subImage\nimage a.dds\nleft 0\ntop 0\nright 0\nbottom 5").is_err());
        assert!(SubImageDescription::parse(b"title anything\n").is_err());
    }
}
