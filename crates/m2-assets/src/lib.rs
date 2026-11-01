//! Bevy asset source and loaders for Metin2 client data (Phase 2).
//!
//! # Setup
//!
//! ```no_run
//! use bevy::prelude::*;
//! use m2_assets::M2AssetsPlugin;
//!
//! App::new()
//!     // MUST come before AssetPlugin (DefaultPlugins): asset sources are
//!     // built when AssetPlugin is added.
//!     .add_plugins(M2AssetsPlugin::new("bin"))
//!     .add_plugins(DefaultPlugins)
//!     .run();
//! ```
//!
//! The plugin loads `pack/Index.dev` (folder packs, loose files first) through
//! [`m2_formats::pack`], registers the `m2://` asset source over the shared
//! [`PackManager`](m2_formats::pack::PackManager) and, once `AssetPlugin`
//! exists, the loaders below. The packs are also available as the
//! [`M2Packs`] resource.
//!
//! # Path convention ([`paths`])
//!
//! `m2://ymir work/pc/warrior/a.gr2` is the pack file `d:/ymir work/pc/warrior/a.gr2`;
//! every other name is used as is (`m2://locale/en/x.txt`). Lookup is
//! case-insensitive and accepts `\`. Asset paths are UTF-8; the ~31 files whose
//! names are CP949 bytes cannot be addressed (see
//! [`source::DirectoryIndex::non_utf8`]). Bevy compares paths case-sensitively:
//! build handles from names found in game data with
//! [`paths::canonical_asset_path`]. `read_directory` lists the virtual tree.
//!
//! # Loaders
//!
//! | extension | asset | module |
//! |-----------|-------|--------|
//! | `dds tga jpg jpeg bmp png` | [`Image`](bevy::image::Image), `Rgba8Unorm` (not sRGB) | [`texture`] |
//! | `gr2` | [`gr2::Gr2Model`]; `load::<AnimationClip>` gives a motion | [`gr2`] |
//! | `spt` | [`spt::SptTree`] | [`spt`] |
//! | `msm` | [`race::RaceAsset`] | [`race`] |
//! | `msa` | [`race::RaceMotionAsset`] | [`race`] |
//!
//! Set `AssetPlugin { meta_check: AssetMetaCheck::Never, .. }` to skip the
//! `.meta` lookups (packs have none; a missing meta is handled anyway).
//!
//! # Coordinates ([`coords`])
//!
//! Metin2 is Z-up left-handed, Bevy Y-up right-handed: `(x, y, z) -> (x, z, y)`
//! with reversed triangle winding and negated quaternion vector parts. All
//! loaders convert on load; nothing downstream converts again.

pub mod coords;
mod dds_raw;
pub mod gr2;
pub mod paths;
pub mod race;
pub mod source;
pub mod spt;
pub mod texture;

use std::path::PathBuf;
use std::sync::Arc;

use bevy::app::{App, Plugin};
use bevy::asset::io::AssetSourceBuilder;
use bevy::asset::{Asset, AssetApp, Assets};
use bevy::ecs::resource::Resource;
use bevy::log::{error, info, warn};
use m2_formats::pack::{PackManager, SearchMode, load_index};

pub use source::{DirectoryIndex, PackAssetReader, SharedPacks};

/// Name of the asset source: `m2://…`.
pub const SOURCE_ID: &str = "m2";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Decode(String),
}

/// The shared packs and the directory index of the `m2://` source.
#[derive(Resource, Clone)]
pub struct M2Packs {
    pub packs: SharedPacks,
    pub reader: PackAssetReader,
}

impl M2Packs {
    /// The `.msa` motion files under a race folder (see [`race::list_motion_files`]).
    pub fn motion_files(&self, race_dir: &str) -> Vec<String> {
        race::list_motion_files(self.reader.index(), race_dir)
    }
}

enum Origin {
    Dir { dir: PathBuf, pack_folder: String, index: String, mode: SearchMode },
    Packs(SharedPacks),
}

/// Registers the `m2://` source and the loaders. Add it **before**
/// `AssetPlugin`/`DefaultPlugins`.
pub struct M2AssetsPlugin {
    origin: Origin,
}

impl M2AssetsPlugin {
    /// Client folder (the one containing `pack/`), `pack/Index.dev`, loose files first.
    pub fn new(client_dir: impl Into<PathBuf>) -> Self {
        Self {
            origin: Origin::Dir {
                dir: client_dir.into(),
                pack_folder: "pack".into(),
                index: "pack/Index.dev".into(),
                mode: SearchMode::FileFirst,
            },
        }
    }

    /// Uses packs you built yourself.
    pub fn from_packs(packs: SharedPacks) -> Self {
        Self { origin: Origin::Packs(packs) }
    }

    /// Another index file (relative to the client folder), e.g. `pack/Index`.
    pub fn with_index(mut self, index_file: &str) -> Self {
        if let Origin::Dir { index, .. } = &mut self.origin {
            *index = index_file.to_owned();
        }
        self
    }

    pub fn with_search_mode(mut self, search_mode: SearchMode) -> Self {
        if let Origin::Dir { mode, .. } = &mut self.origin {
            *mode = search_mode;
        }
        self
    }

    fn open(&self) -> SharedPacks {
        match &self.origin {
            Origin::Packs(p) => Arc::clone(p),
            Origin::Dir { dir, pack_folder, index, mode } => {
                let mut manager = PackManager::new(dir);
                manager.set_search_mode(*mode);
                match load_index(&mut manager, pack_folder, index) {
                    Ok(report) => {
                        info!("m2://: {} packs from {}", report.registered.len(), dir.join(index).display());
                        for (name, e) in &report.failed {
                            warn!("m2://: pack {name} not registered: {e}");
                        }
                    }
                    Err(e) => error!("m2://: cannot load {}: {e}; the source is empty", dir.join(index).display()),
                }
                Arc::new(manager)
            }
        }
    }
}

fn ensure_asset<A: Asset>(app: &mut App) {
    if !app.world().contains_resource::<Assets<A>>() {
        app.init_asset::<A>();
    }
}

impl Plugin for M2AssetsPlugin {
    fn build(&self, app: &mut App) {
        let packs = self.open();
        let reader = PackAssetReader::new(Arc::clone(&packs));
        app.insert_resource(M2Packs { packs, reader: reader.clone() });
        app.register_asset_source(SOURCE_ID, AssetSourceBuilder::new(move || Box::new(reader.clone())));
    }

    /// Loaders need the `AssetServer`, which exists once `AssetPlugin` was added.
    fn finish(&self, app: &mut App) {
        use bevy::animation::AnimationClip;
        use bevy::image::Image;
        use bevy::mesh::Mesh;
        use bevy::mesh::skinning::SkinnedMeshInverseBindposes;

        if app.world().get_resource::<bevy::asset::AssetServer>().is_none() {
            error!("M2AssetsPlugin: AssetPlugin is missing; no loaders registered");
            return;
        }
        ensure_asset::<Image>(app);
        ensure_asset::<Mesh>(app);
        ensure_asset::<AnimationClip>(app);
        ensure_asset::<SkinnedMeshInverseBindposes>(app);
        app.init_asset::<gr2::Gr2Model>()
            .init_asset::<spt::SptTree>()
            .init_asset::<race::RaceAsset>()
            .init_asset::<race::RaceMotionAsset>()
            .register_asset_loader(texture::M2ImageLoader)
            .register_asset_loader(gr2::Gr2Loader)
            .register_asset_loader(gr2::Gr2MotionLoader)
            .register_asset_loader(spt::SptLoader)
            .register_asset_loader(race::RaceLoader)
            .register_asset_loader(race::RaceMotionLoader);
    }
}
