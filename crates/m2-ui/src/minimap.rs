//! The minimap (`uiMiniMap.MiniMap`, `uiscript/minimap.py`, drawn by
//! `CPythonMiniMap`): a 128-pixel disc of the terrains' `minimap.dds` around
//! the player in the `minimap.sub` frame, the player mark turned to the
//! character's heading, the camera's view cone, coloured marks for the other
//! characters, the zoom buttons (`ScaleUp`/`ScaleDown`, 0.5×–4×, 2× at the
//! start), and the close/show buttons. The atlas is not drawn yet.

use std::collections::HashMap;

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use m2_assets::paths;
use m2_game::ingame::{MiniMap, MiniMapMark};

use crate::core::{self, Art, Click, UiSet};
use crate::hud::GameUiRoot;
use crate::login::{LoginScreen, Status};

const ROOT: &str = "d:/ymir work/ui/minimap/";
/// `MiniMapWindow`: 128×128 at (4, 5) in the 136×137 frame.
const DISC: f32 = 128.0;
/// `TERRAIN_XSIZE · CELLSCALE`: one terrain's `minimap.dds` covers this much.
const TERRAIN_UNITS: f32 = 25600.0;
/// `CTerrainImpl::CELLSCALE`: one minimap pixel at scale 1.
const CELL: f32 = 100.0;
/// `whitemark.sub` and `playermark.sub` sizes.
const MARK: f32 = 4.0;
const PLAYER_MARK: Vec2 = Vec2::new(10.0, 11.0);
/// Marks drawn at most (the rest of the pool stays hidden).
const MARK_POOL: usize = 96;

#[derive(Resource)]
struct MiniMapView {
    /// `m_fScale`.
    scale: f32,
    open: bool,
    /// The loaded terrain textures, by `(map, x, y)`.
    tiles: HashMap<(String, i32, i32), Handle<Image>>,
}

impl Default for MiniMapView {
    fn default() -> Self {
        MiniMapView { scale: 2.0, open: true, tiles: HashMap::new() }
    }
}

#[derive(Clone, Copy, Debug, ShaderType)]
struct MiniMapUniform {
    center: Vec2,
    origin: Vec2,
    units_per_px: f32,
    radius: f32,
    _pad: Vec2,
    loaded_a: Vec4,
    loaded_b: Vec4,
    loaded_c: Vec4,
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct MiniMapMaterial {
    #[uniform(0)]
    uniform: MiniMapUniform,
    #[texture(1)]
    #[sampler(2)]
    t0: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    t1: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    t2: Handle<Image>,
    #[texture(7)]
    #[sampler(8)]
    t3: Handle<Image>,
    #[texture(9)]
    #[sampler(10)]
    t4: Handle<Image>,
    #[texture(11)]
    #[sampler(12)]
    t5: Handle<Image>,
    #[texture(13)]
    #[sampler(14)]
    t6: Handle<Image>,
    #[texture(15)]
    #[sampler(16)]
    t7: Handle<Image>,
    #[texture(17)]
    #[sampler(18)]
    t8: Handle<Image>,
}

impl UiMaterial for MiniMapMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://m2_ui/minimap.wesl".into()
    }
}

#[derive(Component)]
struct MiniMapOpen;
#[derive(Component)]
struct MiniMapClosed;
#[derive(Component)]
struct MiniMapDisc;
#[derive(Component)]
struct MiniMapMarkSlot;
#[derive(Component)]
struct MiniMapPlayer;
#[derive(Component)]
struct MiniMapCamera;
#[derive(Component)]
struct MiniMapPosition;
#[derive(Component, Clone, Copy)]
enum MiniMapButton {
    ScaleUp,
    ScaleDown,
    Hide,
    Show,
}

pub struct MiniMapPlugin;

impl Plugin for MiniMapPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "minimap.wesl");
        app.add_plugins(UiMaterialPlugin::<MiniMapMaterial>::default())
            .init_resource::<MiniMapView>()
            .add_systems(Update, (spawn.in_set(UiSet::State), press.in_set(UiSet::Actions), draw.in_set(UiSet::Draw)));
    }
}

fn art_button(commands: &mut Commands, assets: &AssetServer, parent: Entity, node: Node, name: &str) -> Entity {
    let art = ["default", "over", "down"].map(|state| Art::load(assets, &format!("{ROOT}{name}_{state}.sub")));
    core::button_with(commands, parent, node, art, "")
}

/// Spawns the minimap; the HUD despawns it with the other in-game roots.
fn build(commands: &mut Commands, assets: &AssetServer, materials: &mut Assets<MiniMapMaterial>) {
    let frame = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Px(136.0),
        height: Val::Px(137.0),
        ..default()
    };
    let open = commands.spawn((MiniMapOpen, GameUiRoot, frame.clone(), GlobalZIndex(90), Pickable::IGNORE)).id();
    core::picture(commands, assets, open, &format!("{ROOT}minimap.sub"), core::rect(0.0, 0.0, 136.0, 137.0));
    let blank = Handle::<Image>::default();
    let material = materials.add(MiniMapMaterial {
        uniform: MiniMapUniform {
            center: Vec2::ZERO,
            origin: Vec2::ZERO,
            units_per_px: CELL / 2.0,
            radius: DISC / 2.0,
            _pad: Vec2::ZERO,
            loaded_a: Vec4::ZERO,
            loaded_b: Vec4::ZERO,
            loaded_c: Vec4::ZERO,
        },
        t0: blank.clone(),
        t1: blank.clone(),
        t2: blank.clone(),
        t3: blank.clone(),
        t4: blank.clone(),
        t5: blank.clone(),
        t6: blank.clone(),
        t7: blank.clone(),
        t8: blank,
    });
    let disc = commands.spawn((MiniMapDisc, MaterialNode(material), core::rect(4.0, 5.0, DISC, DISC), Pickable::IGNORE)).id();
    commands.entity(open).add_child(disc);
    // `minimap_camera.dds`: the camera's view, under the marks.
    let camera = core::picture(commands, assets, disc, "d:/ymir work/ui/minimap_camera.dds", core::rect(0.0, 0.0, DISC, DISC));
    commands.entity(camera).insert(MiniMapCamera);
    for _ in 0..MARK_POOL {
        let mark = core::picture(commands, assets, disc, &format!("{ROOT}whitemark.sub"), core::rect(0.0, 0.0, MARK, MARK));
        commands.entity(mark).insert(MiniMapMarkSlot);
    }
    let player = core::picture(
        commands,
        assets,
        disc,
        &format!("{ROOT}playermark.sub"),
        core::rect((DISC - PLAYER_MARK.x) / 2.0, (DISC - PLAYER_MARK.y) / 2.0, PLAYER_MARK.x, PLAYER_MARK.y),
    );
    commands.entity(player).insert(MiniMapPlayer);
    for (button, x, y, w, h, name) in [
        (MiniMapButton::ScaleUp, 101.0, 116.0, 15.0, 15.0, "minimap_scaleup"),
        (MiniMapButton::ScaleDown, 115.0, 103.0, 15.0, 15.0, "minimap_scaledown"),
        (MiniMapButton::Hide, 111.0, 6.0, 19.0, 20.0, "minimap_close"),
    ] {
        let id = art_button(commands, assets, open, core::rect(x, y, w, h), name);
        commands.entity(id).insert(button);
    }
    // `PositionInfo`: centred under the frame.
    let position = core::text(commands, open, "", core::rect(-64.0, 140.0, 200.0, 14.0));
    commands.entity(position).insert((MiniMapPosition, TextLayout::justify(Justify::Center)));

    // `CloseWindow`: only the show button.
    let closed = commands
        .spawn((
            MiniMapClosed,
            GameUiRoot,
            Node { display: Display::None, width: Val::Px(132.0), height: Val::Px(48.0), ..frame },
            GlobalZIndex(90),
            Pickable::IGNORE,
        ))
        .id();
    let art = [0; 3].map(|_| Art::load(assets, &format!("{ROOT}minimap_open_default.sub")));
    let show = core::button_with(commands, closed, core::rect(100.0, 4.0, 32.0, 32.0), art, "");
    commands.entity(show).insert(MiniMapButton::Show);
}

/// `ScaleUp`/`ScaleDown` (×2 / ×½ within 0.5–4) and hide/show.
fn press(mut clicks: MessageReader<Click>, buttons: Query<&MiniMapButton>, mut view: ResMut<MiniMapView>) {
    for click in clicks.read() {
        let Ok(button) = buttons.get(click.entity) else { continue };
        match button {
            MiniMapButton::ScaleUp => view.scale = (view.scale * 2.0).min(4.0),
            MiniMapButton::ScaleDown => view.scale = (view.scale * 0.5).max(0.5),
            MiniMapButton::Hide => view.open = false,
            MiniMapButton::Show => view.open = true,
        }
    }
}

/// Turns a heading (degrees, the game's: 0 is +y, which the map draws down)
/// into a clockwise screen rotation.
fn screen_rotation(heading: f32) -> Rot2 {
    Rot2::degrees(180.0 - heading)
}

/// `CPythonMiniMap::Update` + `Render`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    minimap: Option<Res<MiniMap>>,
    mut view: ResMut<MiniMapView>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut materials: ResMut<Assets<MiniMapMaterial>>,
    disc: Query<&MaterialNode<MiniMapMaterial>, With<MiniMapDisc>>,
    mut open: Query<&mut Node, (With<MiniMapOpen>, Without<MiniMapClosed>, Without<MiniMapMarkSlot>)>,
    mut closed: Query<&mut Node, (With<MiniMapClosed>, Without<MiniMapOpen>, Without<MiniMapMarkSlot>)>,
    mut marks: Query<(&mut Node, &mut ImageNode), (With<MiniMapMarkSlot>, Without<MiniMapOpen>, Without<MiniMapClosed>)>,
    mut player: Query<&mut UiTransform, (With<MiniMapPlayer>, Without<MiniMapCamera>)>,
    mut camera: Query<&mut UiTransform, (With<MiniMapCamera>, Without<MiniMapPlayer>)>,
    mut position: Query<&mut Text, With<MiniMapPosition>>,
) {
    let Some(minimap) = minimap else { return };
    let Some(map) = minimap.map.clone() else { return };
    for mut node in &mut open {
        let display = if view.open { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    for mut node in &mut closed {
        let display = if view.open { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
    if !view.open {
        return;
    }
    // The 3×3 terrains around the player (`%s\\%06u\\minimap.dds`, id x·1000+y).
    let tile = (minimap.center / TERRAIN_UNITS).floor();
    let (tx, ty) = (tile.x as i32, tile.y as i32);
    let mut textures: [Handle<Image>; 9] = Default::default();
    let mut loaded = [0.0f32; 12];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (x, y) = (tx + dx, ty + dy);
            let index = ((dy + 1) * 3 + dx + 1) as usize;
            if x < 0 || y < 0 {
                continue;
            }
            let handle = view
                .tiles
                .entry((map.clone(), x, y))
                .or_insert_with(|| {
                    let path = format!("{map}/{:06}/minimap.dds", x * 1000 + y);
                    assets.load(paths::asset_source_uri(&path))
                })
                .clone();
            loaded[index] = if images.get(&handle).is_some() { 1.0 } else { 0.0 };
            textures[index] = handle;
        }
    }
    let units_per_px = CELL / view.scale;
    if let Ok(node) = disc.single()
        && let Some(mut material) = materials.get_mut(&node.0)
    {
        material.uniform = MiniMapUniform {
            center: minimap.center,
            origin: (tile - Vec2::ONE) * TERRAIN_UNITS,
            units_per_px,
            radius: DISC / 2.0,
            _pad: Vec2::ZERO,
            loaded_a: Vec4::from_slice(&loaded[0..4]),
            loaded_b: Vec4::from_slice(&loaded[4..8]),
            loaded_c: Vec4::from_slice(&loaded[8..12]),
        };
        let [t0, t1, t2, t3, t4, t5, t6, t7, t8] = textures;
        (material.t0, material.t1, material.t2, material.t3, material.t4) = (t0, t1, t2, t3, t4);
        (material.t5, material.t6, material.t7, material.t8) = (t5, t6, t7, t8);
    }
    // Marks within the disc (`m_fMiniMapRadius`), in the name colours.
    let radius = DISC / 2.0 - MARK;
    let near = minimap.marks.iter().filter_map(|(at, kind)| {
        let px = (*at - minimap.center) / units_per_px;
        (px.length() < radius).then_some((px, *kind))
    });
    let mut slots = marks.iter_mut();
    for ((px, kind), (mut node, mut image)) in near.zip(&mut slots) {
        let [r, g, b] = match kind {
            MiniMapMark::Pc => [255, 215, 76],
            MiniMapMark::Npc => [122, 231, 93],
            MiniMapMark::Monster => [235, 22, 9],
            MiniMapMark::Warp => [136, 218, 241],
        };
        node.display = Display::Flex;
        node.left = Val::Px((DISC / 2.0 + px.x - MARK / 2.0).round());
        node.top = Val::Px((DISC / 2.0 + px.y - MARK / 2.0).round());
        image.color = Color::srgb_u8(r, g, b);
    }
    for (mut node, _) in slots {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
    if let Ok(mut transform) = player.single_mut() {
        transform.rotation = screen_rotation(minimap.facing);
    }
    if let Ok(mut transform) = camera.single_mut() {
        transform.rotation = screen_rotation(minimap.camera);
    }
    if let Ok(mut text) = position.single_mut() {
        // `MINIMAP_POSITIONINFO`: global coordinates in metres.
        let value = format!("{} ({}, {})", map, minimap.global.0 / 100, minimap.global.1 / 100);
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Builds the minimap once the game screen is up.
fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    screen: Res<LoginScreen>,
    existing: Query<(), With<MiniMapOpen>>,
    mut materials: ResMut<Assets<MiniMapMaterial>>,
) {
    if matches!(screen.status, Status::InGame { .. }) && existing.is_empty() {
        build(&mut commands, &assets, &mut materials);
    }
}
