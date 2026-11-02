//! Safe box and item mall windows, translated from `CPythonSafeBox` and the
//! `SafeboxWindow`/`MallWindow` game UI callbacks.

use bevy::picking::{hover::Hovered, pointer::PointerButton};
use bevy::prelude::*;
use m2_assets::paths;
use m2_game::player::{ItemCatalog, PlayerAction, PlayerState, StorageState};

use crate::{
    core::{self, Click, Consumed, Control, FloatingWindow, Selected, Surface, UiSet},
    hud::GameUiRoot,
    inventory::{self, InventoryUi, Slot},
};

const PAGE_SIZE: usize = 45;
const ICON: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pane {
    Safebox,
    Mall,
}

#[derive(Component, Clone, Copy)]
struct StorageSlot {
    pane: Pane,
    cell: u16,
}

#[derive(Component, Clone, Copy)]
struct StoragePage(Pane, i8);

#[derive(Component)]
struct StorageIcon(Entity);

#[derive(Component)]
struct StorageCount(Entity);

#[derive(Component)]
struct MoneyText;

#[derive(Component)]
struct StorageRoot;

#[derive(Resource, Default)]
pub struct StorageUi {
    safebox_page: usize,
    mall_page: usize,
    selected: Option<(Pane, u16)>,
    safebox_serial: u64,
    mall_serial: u64,
    safebox_dismissed: bool,
    mall_dismissed: bool,
    safebox_was_open: bool,
    mall_was_open: bool,
}

impl StorageUi {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct StoragePlugin;
impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StorageUi>().add_systems(
            Update,
            (
                actions.in_set(UiSet::Actions).after(inventory::InventoryActions),
                (sync_windows, refresh).in_set(UiSet::Draw),
            ),
        );
    }
}

pub fn build(commands: &mut Commands, assets: &AssetServer) {
    build_pane(commands, assets, Pane::Safebox, Vec2::new(390., 40.));
    build_pane(commands, assets, Pane::Mall, Vec2::new(598., 40.));
}

fn build_pane(commands: &mut Commands, assets: &AssetServer, pane: Pane, margin: Vec2) {
    let (id, title) = match pane {
        Pane::Safebox => ("safebox", "Storage"),
        Pane::Mall => ("item_mall", "Item Mall Storage"),
    };
    let root = core::window(
        commands,
        assets,
        id,
        title,
        Vec2::new(198., 408.),
        margin,
        core::Anchor::BottomRight(margin),
        false,
    );
    commands.entity(root).insert((GameUiRoot, StorageRoot));

    // Safe box and mall both use CPythonSafeBox's 5 by 9 cell grid.
    for cell in 0..PAGE_SIZE {
        let x = 10. + (cell % 5) as f32 * ICON;
        let y = 39. + (cell / 5) as f32 * ICON;
        storage_slot(commands, assets, root, pane, cell as u16, core::rect(x, y, ICON, ICON));
    }
    let previous = core::button(commands, assets, root, core::rect(18., 336., 62., 22.), "d:/ymir work/ui/public/small_button", "‹ Prev");
    commands.entity(previous).insert(StoragePage(pane, -1));
    let next = core::button(commands, assets, root, core::rect(116., 336., 62., 22.), "d:/ymir work/ui/public/small_button", "Next ›");
    commands.entity(next).insert(StoragePage(pane, 1));
    let page = core::text(commands, root, "1", core::rect(80., 339., 34., 18.));
    commands.entity(page).insert((StoragePageLabel(pane), TextLayout::justify(Justify::Center)));
    if pane == Pane::Safebox {
        let money = core::text(commands, root, "Yang: 0", core::rect(12., 367., 174., 20.));
        commands.entity(money).insert((MoneyText, TextLayout::justify(Justify::Center)));
        let message = core::text(commands, root, "", core::rect(12., 387., 174., 16.));
        commands.entity(message).insert((StorageNotice, TextColor(Color::srgb(1., 0.62, 0.48)), TextLayout::justify(Justify::Center)));
    }
}

#[derive(Component)]
struct StoragePageLabel(Pane);
#[derive(Component)]
struct StorageNotice;

fn storage_slot(commands: &mut Commands, assets: &AssetServer, parent: Entity, pane: Pane, cell: u16, node: Node) -> Entity {
    let mut node = node;
    node.border = UiRect::all(Val::Px(1.));
    let entity = commands
        .spawn((
            StorageSlot { pane, cell },
            Control,
            Surface,
            Hovered::default(),
            Selected(false),
            node,
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.55, 0.48, 0.36, 0.6)),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    core::picture(commands, assets, entity, "d:/ymir work/ui/public/slot_base.sub", core::rect(0., 0., ICON, ICON));
    let icon = commands
        .spawn((StorageIcon(entity), Node { display: Display::None, ..core::rect(0., 0., ICON, ICON) }, ImageNode::default(), ZIndex(3), Pickable::IGNORE))
        .id();
    commands.entity(entity).add_child(icon);
    let count = core::text(commands, entity, "", core::rect(0., 17., 30., 15.));
    commands.entity(count).insert((StorageCount(entity), ZIndex(4), TextLayout::justify(Justify::Right)));
    entity
}

fn actions(
    mut clicks: MessageReader<Click>,
    slots: Query<&StorageSlot>,
    pages: Query<&StoragePage>,
    inventory_slots: Query<&Slot>,
    mut ui: ResMut<StorageUi>,
    mut inventory_ui: ResMut<InventoryUi>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    state: Res<StorageState>,
    mut actions: MessageWriter<PlayerAction>,
    mut consumed: ResMut<Consumed>,
) {
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        if let Ok(page) = pages.get(click.entity) {
            match page.0 {
                Pane::Safebox => {
                    let max = state.safebox.len().saturating_sub(1) / PAGE_SIZE;
                    ui.safebox_page = (ui.safebox_page as isize + isize::from(page.1)).clamp(0, max as isize) as usize;
                }
                Pane::Mall => {
                    let max = state.mall.len().saturating_sub(1) / PAGE_SIZE;
                    ui.mall_page = (ui.mall_page as isize + isize::from(page.1)).clamp(0, max as isize) as usize;
                }
            }
            consumed.pointer = true;
            continue;
        }
        if let Ok(slot) = slots.get(click.entity) {
            let index = match slot.pane {
                Pane::Safebox => ui.safebox_page * PAGE_SIZE + usize::from(slot.cell),
                Pane::Mall => ui.mall_page * PAGE_SIZE + usize::from(slot.cell),
            };
            let Some(index_u8) = u8::try_from(index).ok() else { continue };
            match slot.pane {
                Pane::Safebox => {
                    if let Some((position, _)) = inventory_ui.held_item() {
                        if state.safebox_open && position.window == 1 && position.cell < 90 {
                            actions.write(PlayerAction::SafeboxCheckin { inventory: position, safe_cell: index_u8 });
                            inventory_ui.clear_held();
                        }
                        ui.selected = None;
                    } else if let Some((Pane::Safebox, source)) = ui.selected {
                        if source == index as u16 {
                            ui.selected = None;
                        } else if let Some(item) = state.safebox.get(usize::from(source)).and_then(Option::as_ref) {
                            actions.write(PlayerAction::SafeboxMove { source: source as u8, destination: index_u8, count: item.count });
                            ui.selected = None;
                        }
                    } else if state.safebox.get(index).is_some_and(Option::is_some) {
                        ui.selected = Some((Pane::Safebox, index as u16));
                    }
                }
                Pane::Mall => {
                    if state.mall.get(index).is_some_and(Option::is_some) {
                        ui.selected = Some((Pane::Mall, index as u16));
                    } else {
                        ui.selected = None;
                    }
                }
            }
            consumed.pointer = true;
            continue;
        }
        let Ok(slot) = inventory_slots.get(click.entity) else { continue };
        let Some((pane, source)) = ui.selected else { continue };
        let Some(position) = inventory::position(*slot, &inventory_ui, &player, &catalog) else { continue };
        if position.window != 1 || position.cell >= 90 || player.items.contains_key(&position) {
            inventory_ui.clear_held();
            continue;
        }
        match pane {
            Pane::Safebox if state.safebox_open => {
                actions.write(PlayerAction::SafeboxCheckout { safe_cell: source as u8, inventory: position });
            }
            Pane::Mall if state.mall_open => {
                actions.write(PlayerAction::MallCheckout { mall_cell: source as u8, inventory: position });
            }
            _ => {}
        }
        ui.selected = None;
        inventory_ui.clear_held();
        consumed.pointer = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_windows(
    state: Res<StorageState>,
    mut ui: ResMut<StorageUi>,
    mut windows: Query<&mut FloatingWindow, With<StorageRoot>>,
) {
    for mut window in &mut windows {
        let pane = if window.id == "safebox" { Pane::Safebox } else { Pane::Mall };
        match pane {
            Pane::Safebox => {
                if ui.safebox_serial != state.safebox_serial {
                    ui.safebox_serial = state.safebox_serial;
                    ui.safebox_dismissed = false;
                    ui.safebox_page = 0;
                    ui.selected = None;
                }
                if ui.safebox_was_open && !window.open { ui.safebox_dismissed = true; }
                let open = state.safebox_open && !ui.safebox_dismissed;
                if window.open != open {
                    window.open = open;
                }
                if ui.safebox_was_open != open {
                    ui.safebox_was_open = open;
                }
            }
            Pane::Mall => {
                if ui.mall_serial != state.mall_serial {
                    ui.mall_serial = state.mall_serial;
                    ui.mall_dismissed = false;
                    ui.mall_page = 0;
                    ui.selected = None;
                }
                if ui.mall_was_open && !window.open { ui.mall_dismissed = true; }
                let open = state.mall_open && !ui.mall_dismissed;
                if window.open != open {
                    window.open = open;
                }
                if ui.mall_was_open != open {
                    ui.mall_was_open = open;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    assets: Res<AssetServer>,
    state: Res<StorageState>,
    catalog: Res<ItemCatalog>,
    ui: Res<StorageUi>,
    mut icons: Query<(&StorageIcon, &mut Node, &mut ImageNode)>,
    mut counts: Query<(&StorageCount, &mut Text), (Without<StoragePageLabel>, Without<MoneyText>, Without<StorageNotice>)>,
    slots: Query<&StorageSlot>,
    mut selection: Query<(&StorageSlot, &mut Selected, &mut BorderColor)>,
    mut pages: Query<(&StoragePageLabel, &mut Text), (Without<StorageCount>, Without<MoneyText>, Without<StorageNotice>)>,
    mut money: Query<&mut Text, (With<MoneyText>, Without<StorageCount>, Without<StoragePageLabel>, Without<StorageNotice>)>,
    mut notices: Query<&mut Text, (With<StorageNotice>, Without<StorageCount>, Without<StoragePageLabel>, Without<MoneyText>)>,
) {
    // The windows are built once and usually closed; only touch their widgets
    // when their data changed. Writing them every frame dirties the whole UI
    // layout for two hidden windows.
    if !state.is_changed() && !ui.is_changed() && !catalog.is_changed() {
        return;
    }
    for (icon, mut node, mut image) in &mut icons {
        let Ok(slot) = slots.get(icon.0) else { continue };
        let index = match slot.pane {
            Pane::Safebox => ui.safebox_page * PAGE_SIZE + usize::from(slot.cell),
            Pane::Mall => ui.mall_page * PAGE_SIZE + usize::from(slot.cell),
        };
        let item = match slot.pane {
            Pane::Safebox => state.safebox.get(index),
            Pane::Mall => state.mall.get(index),
        }.and_then(Option::as_ref);
        if let Some(item) = item {
            let path = catalog.get(item.vnum)
                .map(|data| String::from_utf8_lossy(&data.icon_file_name).into_owned())
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| format!("icon/item/{:05}.tga", item.vnum));
            image.image = assets.load(paths::asset_source_uri(&path));
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
    for (count, mut text) in &mut counts {
        let item = slots.get(count.0).ok().and_then(|slot| {
            let (items, page) = match slot.pane {
                Pane::Safebox => (&state.safebox, ui.safebox_page),
                Pane::Mall => (&state.mall, ui.mall_page),
            };
            items.get(page * PAGE_SIZE + usize::from(slot.cell)).and_then(Option::as_ref)
        });
        text.0 = item.filter(|item| item.count > 1).map_or_else(String::new, |item| item.count.to_string());
    }
    for (slot, mut selected, mut border) in &mut selection {
        let index = match slot.pane {
            Pane::Safebox => ui.safebox_page * PAGE_SIZE + usize::from(slot.cell),
            Pane::Mall => ui.mall_page * PAGE_SIZE + usize::from(slot.cell),
        } as u16;
        selected.set_if_neq(Selected(ui.selected == Some((slot.pane, index))));
        border.set_all(if selected.0 { Color::srgb(0.95, 0.71, 0.25) } else { Color::srgba(0.55, 0.48, 0.36, 0.6) });
    }
    for (label, mut text) in &mut pages {
        let (page, len) = match label.0 {
            Pane::Safebox => (ui.safebox_page, state.safebox.len()),
            Pane::Mall => (ui.mall_page, state.mall.len()),
        };
        let page_count = len.div_ceil(PAGE_SIZE).max(1);
        text.0 = format!("{} / {}", page + 1, page_count);
    }
    for mut text in &mut money {
        text.0 = format!("Yang: {}", state.money);
    }
    for mut text in &mut notices {
        text.0 = if state.password_error { "Wrong storage password" } else { "" }.into();
    }
}
