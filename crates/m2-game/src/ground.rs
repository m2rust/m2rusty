//! Items lying on the ground (`CPythonItem`'s ground item instances): the
//! server's `GC_ITEM_GROUND_ADD`/`_DEL`/`ITEM_OWNERSHIP`, their drop models
//! with the `dropitem.mse` glow, and picking them up (`SendClickItemPacket`).

use std::collections::HashMap;

use bevy::prelude::*;
use m2_assets::gr2::Gr2Model;
use m2_assets::{M2Packs, paths};
use m2_net::phase::NetEvent;
use m2_net::wire::GcPacket;

use crate::SceneCamera;
use crate::fx::CombatFx;
use crate::ingame::GameWorld;
use crate::net::{Net, NetMessage};
use crate::player::{ItemCatalog, PlayerState};

/// `CItemData::SetDefaultItemData`: the drop model of an item without one.
const ITEM_BAG_MODEL: &str = "d:/ymir work/item/etc/item_bag.gr2";
/// `m_dwDropItemEffectID` (`CPythonItem::Create`).
const EFFECT_DROP_ITEM: &str = "d:/ymir work/effect/etc/dropitem/dropitem.mse";
/// `CreateItem(bDrop)`: the model rests 10 units above the ground.
const DROP_HEIGHT: f32 = 10.0;
/// `NEW_IsClickableDistanceDestPixelPosition`: a clicked item within this
/// reach is picked at once, else the character walks to it first.
pub const CLICKABLE_DISTANCE: f32 = 150.0;
/// `MODE_CLICK_ITEM`: a reserved item is picked once this close.
pub const RESERVED_PICK_DISTANCE: f32 = 20.0;
/// `SendClickItemPacket` sends at most one pick-up every 500 ms.
const PICK_INTERVAL: f32 = 0.5;
/// Screen distance (px) within which a click picks an item.
const PICK_RADIUS_PX: f32 = 30.0;

/// One `TGroundItemInstance`.
pub struct GroundItem {
    pub vnum: u32,
    /// Global coordinates.
    pub x: i32,
    pub y: i32,
    /// `GC_ITEM_OWNERSHIP`: who may pick it up; empty for anyone.
    pub owner: String,
    model: Option<Handle<Gr2Model>>,
    entity: Option<Entity>,
}

/// The ground items the server announced, by item VID.
#[derive(Resource, Default)]
pub struct GroundItems {
    pub by_vid: HashMap<u32, GroundItem>,
    next_pick: f32,
}

impl GroundItem {
    /// The spawned drop model, once its `.gr2` loaded.
    pub fn entity(&self) -> Option<Entity> {
        self.entity
    }
}

impl GroundItems {
    /// Remove ground items from the previous map when its scene is cleared.
    pub(crate) fn clear_scene(world: &mut World) {
        let Some(mut items) = world.remove_resource::<GroundItems>() else { return };
        for item in items.by_vid.drain().map(|(_, item)| item) {
            if let Some(entity) = item.entity {
                world.despawn(entity);
            }
        }
        items.next_pick = 0.0;
        world.insert_resource(items);
    }

    /// `GetCloseItem`: the nearest item within `distance` of `local`
    /// (map-local coordinates).
    pub fn closest(&self, base: (u32, u32), local: Vec2, distance: f32) -> Option<u32> {
        self.by_vid
            .iter()
            .map(|(&vid, item)| (vid, (item_local(base, item) - local).length()))
            .filter(|(_, d)| *d <= distance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(vid, _)| vid)
    }

    /// The item's map-local position.
    pub fn position(&self, base: (u32, u32), vid: u32) -> Option<Vec2> {
        self.by_vid.get(&vid).map(|item| item_local(base, item))
    }
}

fn item_local(base: (u32, u32), item: &GroundItem) -> Vec2 {
    Vec2::new((item.x - base.0 as i32) as f32, (item.y - base.1 as i32) as f32)
}

/// `CPythonPlayer::SendClickItemPacket`: asks the server for the item unless
/// it belongs to someone else (party sharing is not checked yet) or the last
/// request was under 500 ms ago. Returns whether a packet went out.
pub fn send_click_item(world: &mut World, vid: u32, now: f32) -> bool {
    let owner = {
        let Some(items) = world.get_resource::<GroundItems>() else { return false };
        if now < items.next_pick {
            return false;
        }
        let Some(item) = items.by_vid.get(&vid) else { return false };
        item.owner.clone()
    };
    world.resource_mut::<GroundItems>().next_pick = now + PICK_INTERVAL;
    let me = world.get_resource::<PlayerState>().map(|player| player.name.clone()).unwrap_or_default();
    if !owner.is_empty() && owner != me {
        // `OnCannotPickItem`.
        if let Some(mut player) = world.get_resource_mut::<PlayerState>() {
            player.line(1, "You cannot pick up this item.".to_owned());
        }
        return false;
    }
    world.get_resource_mut::<Net>().is_some_and(|mut net| net.game.send_item_pickup(vid))
}

/// The ground item under the cursor (`CPythonItem::GetPickedItemID`), by its
/// projected position.
pub fn pick_item(world: &mut World, cursor: Vec2) -> Option<u32> {
    let items: Vec<(u32, Vec3)> = {
        let items = world.get_resource::<GroundItems>()?;
        items
            .by_vid
            .iter()
            .filter_map(|(&vid, item)| Some((vid, world.get::<GlobalTransform>(item.entity?)?.translation())))
            .collect()
    };
    let mut query = world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
    let (camera, transform) = query.single(world).ok()?;
    items
        .into_iter()
        .filter_map(|(vid, at)| {
            let screen = camera.world_to_viewport(transform, at + Vec3::Y * 10.0).ok()?;
            let distance = screen.distance(cursor);
            (distance <= PICK_RADIUS_PX).then_some((vid, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(vid, _)| vid)
}

pub struct GroundItemPlugin;

impl Plugin for GroundItemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundItems>().add_systems(Update, (track_ground_items, draw_ground_items).chain());
    }
}

/// `RecvItemGroundAddPacket`, `RecvItemGroundDelPacket`, `RecvItemOwnership`.
fn track_ground_items(
    mut commands: Commands,
    mut messages: MessageReader<NetMessage>,
    mut items: ResMut<GroundItems>,
    catalog: Option<Res<ItemCatalog>>,
    assets: Res<AssetServer>,
) {
    for message in messages.read() {
        let NetMessage::Game(event) = message else { continue };
        match event {
            NetEvent::Packet(GcPacket::ItemGroundAdd(p)) => {
                if let Some(old) = items.by_vid.remove(&p.dwVID).and_then(|item| item.entity) {
                    commands.entity(old).despawn();
                }
                // `CItemData::GetDropModelThing`: the item list's model.
                let model = catalog
                    .as_ref()
                    .and_then(|catalog| catalog.get(p.dwVnum))
                    .map(|item| String::from_utf8_lossy(&item.model_file_name).into_owned())
                    .filter(|model| !model.is_empty())
                    .unwrap_or_else(|| ITEM_BAG_MODEL.to_owned());
                let model = assets.load(paths::asset_source_uri(&paths::canonical_asset_path(&model)));
                items.by_vid.insert(
                    p.dwVID,
                    GroundItem { vnum: p.dwVnum, x: p.lX, y: p.lY, owner: String::new(), model: Some(model), entity: None },
                );
            }
            NetEvent::Packet(GcPacket::ItemGroundDel(p)) => {
                if let Some(entity) = items.by_vid.remove(&p.vid).and_then(|item| item.entity) {
                    commands.entity(entity).despawn();
                }
            }
            NetEvent::Packet(GcPacket::ItemOwnership(p)) => {
                if let Some(item) = items.by_vid.get_mut(&p.dwVID) {
                    let end = p.szName.iter().position(|&b| b == 0).unwrap_or(p.szName.len());
                    item.owner = String::from_utf8_lossy(&p.szName[..end]).into_owned();
                }
            }
            _ => {}
        }
    }
}

/// Spawns each item's drop model once loaded, on the ground of the current
/// map, and keeps the `dropitem.mse` glow on it.
fn draw_ground_items(world: &mut World) {
    let (ready, glows) = {
        let Some(game) = world.get_resource::<GameWorld>() else { return };
        if !game.is_ready() {
            return;
        }
        let base = game.base;
        let items = world.resource::<GroundItems>();
        let models = world.resource::<Assets<Gr2Model>>();
        let ground = |item: &GroundItem| {
            let local = item_local(base, item);
            (local, game.terrain_height(local.x, local.y) + DROP_HEIGHT)
        };
        let ready: Vec<(u32, Gr2Model, Vec3, f32)> = items
            .by_vid
            .iter()
            .filter(|(_, item)| item.entity.is_none())
            .filter_map(|(&vid, item)| {
                let model = models.get(item.model.as_ref()?)?.clone();
                let (local, height) = ground(item);
                // A stable random heading per item.
                let yaw = (vid.wrapping_mul(2_654_435_761) % 360) as f32;
                Some((vid, model, Vec3::new(local.x, height, -local.y), yaw))
            })
            .collect();
        let glows: Vec<((u32, u32), &'static str, Vec3)> = items
            .by_vid
            .iter()
            .map(|(&vid, item)| {
                let (local, height) = ground(item);
                ((vid, 0), EFFECT_DROP_ITEM, Vec3::new(local.x, -local.y, height))
            })
            .collect();
        (ready, glows)
    };
    for (vid, model, at, yaw) in ready {
        let parent = world
            .spawn((
                Name::new(format!("ground item {vid}")),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw.to_radians())),
                Visibility::Visible,
            ))
            .id();
        crate::render::model::spawn_model(world, &model, Some(parent), &[], None, None);
        if let Some(item) = world.resource_mut::<GroundItems>().by_vid.get_mut(&vid) {
            item.entity = Some(parent);
            item.model = None;
        }
    }
    let packs = world.get_resource::<M2Packs>().cloned();
    world.resource_mut::<CombatFx>().sync_ground(packs.as_ref(), &glows);
}
