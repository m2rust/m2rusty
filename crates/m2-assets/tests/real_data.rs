//! Integration tests against the real client data. They skip (with a message)
//! when it is absent: set `M2_CLIENT_DIR`, or have a `bin/` folder with
//! `pack/Index.dev` in the repository (or a parent of it, for worktrees).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy::animation::AnimationClip;
use bevy::asset::{AssetPlugin, AssetServer, Assets, LoadState, io::AssetSourceId};
use bevy::image::Image;
use bevy::math::{Mat4, Vec3};
use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::*;
use m2_assets::gr2::Gr2Model;
use m2_assets::race::{RaceAsset, RaceMotionAsset};
use m2_assets::spt::SptTree;
use m2_assets::{M2AssetsPlugin, M2Packs, SharedPacks};
use m2_formats::pack::{PackManager, SearchMode, load_index};

fn client_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("M2_CLIENT_DIR") {
        let d = PathBuf::from(d);
        return d.join("pack/Index.dev").exists().then_some(d);
    }
    let mut dir = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    while let Some(d) = dir {
        if d.join("bin/pack/Index.dev").exists() {
            return Some(d.join("bin"));
        }
        dir = d.parent().map(PathBuf::from);
    }
    None
}

macro_rules! client {
    () => {
        match client_dir() {
            Some(d) => d,
            None => {
                eprintln!("SKIP: client data not found (set M2_CLIENT_DIR)");
                return;
            }
        }
    };
}

fn packs(dir: &PathBuf) -> SharedPacks {
    let mut m = PackManager::new(dir);
    m.set_search_mode(SearchMode::FileFirst);
    load_index(&mut m, "pack", "pack/Index.dev").expect("index");
    Arc::new(m)
}

fn app(packs: SharedPacks) -> App {
    let mut app = App::new();
    app.add_plugins(M2AssetsPlugin::from_packs(packs))
        .add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.finish();
    app.cleanup();
    app
}

/// Updates until every handle is loaded or failed; returns the failures.
fn run_until_loaded(app: &mut App, handles: &[UntypedHandle]) -> Vec<(String, String)> {
    let start = Instant::now();
    loop {
        app.update();
        let server = app.world().resource::<AssetServer>();
        let pending = handles.iter().any(|h| matches!(server.load_state(h), LoadState::Loading | LoadState::NotLoaded));
        if !pending {
            return handles
                .iter()
                .filter_map(|h| match server.load_state(h) {
                    LoadState::Failed(e) => Some((format!("{:?}", server.get_path(h.id())), e.to_string())),
                    _ => None,
                })
                .collect();
        }
        assert!(start.elapsed() < Duration::from_secs(300), "load timeout");
    }
}

#[test]
fn directory_listing_and_non_utf8_count() {
    let dir = client!();
    let m = packs(&dir);
    let reader = m2_assets::PackAssetReader::new(m);
    let index = reader.index();
    eprintln!("files: {} addressable, {} with non-UTF-8 names", index.file_count(), index.non_utf8());
    assert!(index.file_count() > 50_000);
    // The folder packs in `bin/pack` store their dozen CP949 names as Latin-1
    // mojibake, which is valid UTF-8, so this is 0 there; zipped packs with raw
    // CP949 names would count here.
    assert!(index.non_utf8() < 200, "{}", index.non_utf8());
    let root: Vec<_> = index.list("").unwrap().collect();
    assert!(root.contains(&("ymir work", true)), "{root:?}");
    assert!(index.is_dir("ymir work/pc/assassin"));
    let motions = m2_assets::race::list_motion_files(index, "ymir work/pc/assassin");
    eprintln!("assassin motions: {}", motions.len());
    assert!(motions.iter().any(|m| m.ends_with("general/wait.msa") || m.ends_with("/wait.msa")), "{motions:?}");
}

#[test]
fn reader_serves_bytes_through_the_asset_server() {
    use bevy::asset::io::AssetReader;
    let dir = client!();
    let reader = m2_assets::PackAssetReader::new(packs(&dir));
    let read = |p: &str| {
        bevy::tasks::block_on(async {
            let mut r = reader.read(std::path::Path::new(p)).await?;
            let mut v = Vec::new();
            bevy::asset::io::Reader::read_to_end(&mut r, &mut v).await.map(|_| v).map_err(Into::into)
        })
    };
    let a: Result<Vec<u8>, bevy::asset::io::AssetReaderError> = read("ymir work/pc/assassin/assassin.gr2");
    let b = read("YMIR WORK\\PC\\Assassin\\ASSASSIN.GR2").unwrap();
    assert_eq!(a.unwrap(), b);
    assert!(read("ymir work/does/not/exist.gr2").is_err());
    let _ = AssetSourceId::Default;
}

/// Loads texture, skinned model, motion, tree and race scripts through the asset server.
#[test]
fn headless_app_loads_everything() {
    let dir = client!();
    let mut app = app(packs(&dir));
    let server = app.world().resource::<AssetServer>().clone();

    let tex: Handle<Image> = server.load("m2://ymir work/pc/assassin/assassin_1-1_tanma_a.dds");
    let model: Handle<Gr2Model> = server.load("m2://ymir work/pc/assassin/assassin.gr2");
    let clip: Handle<AnimationClip> = server.load("m2://ymir work/pc/assassin/general/wait.gr2");
    let tree: Handle<SptTree> = server.load("m2://ymir work/tree/b1_baobab_rt.spt");
    let msa: Handle<RaceMotionAsset> = server.load("m2://ymir work/pc/assassin/general/wait.msa");
    let msm: Handle<RaceAsset> = server.load("m2://assassin_m.msm");

    let failed = run_until_loaded(
        &mut app,
        &[tex.clone().untyped(), model.clone().untyped(), clip.clone().untyped(), tree.clone().untyped(), msa.clone().untyped(), msm.clone().untyped()],
    );
    assert!(failed.is_empty(), "{failed:?}");

    // Texture: DXT decoded to RGBA8 with mips.
    let images = app.world().resource::<Assets<Image>>();
    let img = images.get(&tex).unwrap();
    assert_eq!(img.texture_descriptor.format, bevy::render::render_resource::TextureFormat::Rgba8Unorm);
    eprintln!("texture {}x{} mips {}", img.width(), img.height(), img.texture_descriptor.mip_level_count);
    assert!(img.texture_descriptor.mip_level_count > 1);

    // Model: skinned, standing upright (taller than wide), joints in range.
    let models = app.world().resource::<Assets<Gr2Model>>();
    let m = models.get(&model).unwrap();
    let skel = m.skeleton.as_ref().unwrap();
    eprintln!("model {:?}: {} bones, {} meshes, bounds {:?}", m.name, skel.bones.len(), m.meshes.len(), m.bounds);
    assert!(skel.bones.len() > 30 && !m.meshes.is_empty());
    let size = m.bounds.1 - m.bounds.0;
    assert!(size.y > size.x && size.y > size.z, "not upright: {size:?}");
    let meshes = app.world().resource::<Assets<Mesh>>();
    let mut parts = 0;
    for part in m.meshes.iter().flat_map(|m| &m.parts) {
        let mesh = meshes.get(&part.mesh).unwrap();
        let Some(VertexAttributeValues::Uint16x4(j)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) else {
            panic!("no joint indices")
        };
        assert!(j.iter().flatten().all(|&i| usize::from(i) < skel.bones.len()));
        let Some(VertexAttributeValues::Float32x4(w)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) else { panic!() };
        assert!(w.iter().all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-3));
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some() && mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(part.material.diffuse.as_deref().is_some_and(|d| d.starts_with("ymir work/")), "{:?}", part.material);
        parts += 1;
    }
    eprintln!("{parts} parts, textures {:?}", m.texture_paths());
    assert!(parts > 0);

    // Skeleton conventions: world(bind pose from local transforms) * inverse_world == identity.
    let mut world: Vec<Mat4> = Vec::new();
    for b in &skel.bones {
        let local = b.local.to_matrix();
        let w = b.parent.map_or(local, |p| world[p] * local);
        world.push(w);
    }
    // The data has one exception: the root `assasin_low` of the object meshes
    // stores inverse_world = (InitialPlacement * local)^-1, so every bone under
    // it has the residual InitialPlacement^-1. The C++ client never applies
    // InitialPlacement either (`GrannyCompat.h`: composite = InverseWorld4x4 *
    // world), so that is expected. What must hold: within one root's subtree
    // the residual is the same matrix, and the body skeleton (`Bip01`) is exact.
    let max_diff = |a: Mat4, b: Mat4| (a - b).to_cols_array().iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let root_of = |mut i: usize| {
        while let Some(p) = skel.bones[i].parent {
            i = p;
        }
        i
    };
    let residual = |i: usize| world[i] * skel.bones[i].inverse_world;
    let mut bad = Vec::new();
    for i in 0..skel.bones.len() {
        let root = root_of(i);
        let err = max_diff(residual(i), residual(root));
        if err > 5e-3 {
            bad.push((skel.bones[i].name.clone(), err));
        }
    }
    eprintln!("bind pose: {} of {} bones inconsistent with their root: {bad:?}", bad.len(), skel.bones.len());
    assert!(bad.is_empty(), "bind pose mismatch");
    let bip = skel.bones.iter().position(|b| b.name == "Bip01").expect("Bip01");
    assert!(max_diff(residual(bip), Mat4::IDENTITY) < 5e-3, "Bip01 bind pose: {:?}", residual(bip));

    // Motion has curves for the skeleton's bones.
    let clips = app.world().resource::<Assets<AnimationClip>>();
    let c = clips.get(&clip).unwrap();
    let hit = skel.bones.iter().filter(|b| c.curves().contains_key(&m2_assets::gr2::bone_target_id(&b.name))).count();
    eprintln!("clip: {:.3}s, {} targets, {hit}/{} bones animated", c.duration(), c.curves().len(), skel.bones.len());
    assert!(c.duration() > 0.1 && hit > 30);

    // Tree.
    let trees = app.world().resource::<Assets<SptTree>>();
    let t = trees.get(&tree).unwrap();
    eprintln!("tree: {:?} leaves {} bounds {:?} baked {}", t.textures, t.leaves.len(), t.bounding_box, t.baked);
    assert!(t.branches.is_some() && !t.leaves.is_empty());
    assert!(t.bounding_box.1.y > t.bounding_box.1.x);
    let tmesh = meshes.get(t.leaf_mesh.as_ref().unwrap()).unwrap();
    assert_eq!(tmesh.count_vertices(), t.leaves.len() * 4);
    let mut tm = tmesh.clone();
    t.update_leaf_billboards(&mut tm, Vec3::ZERO, Vec3::new(0.0, 5.0, 30.0));

    // Race scripts.
    let motions = app.world().resource::<Assets<RaceMotionAsset>>();
    let ma = motions.get(&msa).unwrap();
    eprintln!("msa: {ma:?}");
    assert!(ma.motion_file.ends_with(".gr2") && ma.duration > 0.0);
    let races = app.world().resource::<Assets<RaceAsset>>();
    let r = races.get(&msm).unwrap();
    eprintln!("msm: base {} shapes {} hairs {}", r.base_model, r.shapes.len(), r.hairs.len());
    assert!(r.base_model.starts_with("ymir work/"));
    let _ = app.world().resource::<M2Packs>();
}

/// Loads a sample of every kind of file and reports the failures.
#[test]
fn bulk_sample_loads() {
    let dir = client!();
    let packs = packs(&dir);
    let reader = m2_assets::PackAssetReader::new(packs.clone());
    let all: Vec<String> = {
        let mut v: Vec<String> = packs.file_names().filter_map(|n| m2_assets::paths::asset_path(n)).collect();
        v.sort();
        v
    };
    let _ = reader;
    let pick = |ext: &str, prefix: &str, step: usize| -> Vec<String> {
        all.iter().filter(|p| p.starts_with(prefix) && p.ends_with(ext)).step_by(step).cloned().collect()
    };

    let mut app = app(packs);
    let server = app.world().resource::<AssetServer>().clone();
    let mut handles: Vec<UntypedHandle> = Vec::new();
    let mut counts: Vec<(&str, usize)> = Vec::new();

    let mut add = |name: &'static str, files: Vec<String>, handles: &mut Vec<UntypedHandle>| {
        counts.push((name, files.len()));
        for f in files {
            let p = format!("m2://{f}");
            let h: UntypedHandle = match name {
                "tex" => server.load::<Image>(p).untyped(),
                "model" => server.load::<Gr2Model>(p).untyped(),
                "motion" => server.load::<AnimationClip>(p).untyped(),
                "spt" => server.load::<SptTree>(p).untyped(),
                "msm" => server.load::<RaceAsset>(p).untyped(),
                _ => server.load::<RaceMotionAsset>(p).untyped(),
            };
            handles.push(h);
        }
    };
    add("tex", pick(".dds", "ymir work/", 40), &mut handles);
    add("tex", pick(".tga", "ymir work/", 20), &mut handles);
    add("tex", pick(".jpg", "", 5), &mut handles);
    add("tex", pick(".bmp", "", 5), &mut handles);
    add("tex", pick(".png", "", 5), &mut handles);
    add("model", pick(".gr2", "ymir work/pc/", 14).into_iter().chain(pick("/00.gr2", "ymir work/monster", 3)).chain(pick(".gr2", "ymir work/npc", 25)).collect(), &mut handles);
    add("motion", pick("/wait.gr2", "ymir work/", 1).into_iter().chain(pick("/run.gr2", "ymir work/", 3)).collect(), &mut handles);
    add("spt", pick(".spt", "", 1), &mut handles);
    add("msm", pick(".msm", "", 4), &mut handles);
    add("msa", pick(".msa", "ymir work/", 20), &mut handles);

    eprintln!("loading {} assets: {counts:?}", handles.len());
    let mut failed = run_until_loaded(&mut app, &handles);
    // The model sample also catches motion-only `.gr2` files: not a failure.
    failed.retain(|(_, e)| !e.contains("file has no model"));
    for (p, e) in failed.iter().take(40) {
        eprintln!("FAILED {p}: {e}");
    }
    eprintln!("{} of {} failed", failed.len(), handles.len());
    // A model file may legitimately lack a model (motion-only files in 00.gr2 style names): allow a few.
    assert!(failed.len() * 50 < handles.len(), "too many failures: {}/{}", failed.len(), handles.len());
}
