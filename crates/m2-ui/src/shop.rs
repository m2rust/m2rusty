//! NPC shop, following `uiscript/shopdialog.py` and `root/uishop.py`.

use bevy::picking::{hover::Hovered, pointer::PointerButton};
use bevy::prelude::*;
use m2_assets::paths;
use m2_game::player::{ItemCatalog, PlayerAction, ShopState, ShopTab};

use crate::{
    core::{self, Click, Consumed, Control, FloatingWindow, Selected, Surface, Tooltip, UiSet, WindowManager},
    hud::GameUiRoot,
    inventory::InventoryActions,
};

const COLUMNS: usize = 5;
const CELLS: usize = 40;
const ICON: f32 = 32.0;

#[derive(Component)]
struct ShopRoot;
#[derive(Component)]
struct PurchaseDialog;
#[derive(Component)]
struct ShopCell(u8);
#[derive(Component)]
struct ShopIcon { slot: Entity, cell: u8 }
#[derive(Component)]
struct ShopCount(u8);
#[derive(Component, Clone, Copy)]
enum ShopButton { Buy, Sell, Tab(u8), Yes, No }
#[derive(Component)]
struct TabName(u8);
#[derive(Component)]
struct ShopNotice;
#[derive(Component)]
struct PurchaseText;

#[derive(Clone, Copy)]
struct Purchase {
    generation: u64,
    tab: u8,
    cell: u8,
    vnum: u32,
    count: u8,
    price: u32,
}

#[derive(Resource, Default)]
pub struct ShopUi {
    pub tab: u8,
    pub sell_mode: bool,
    generation: u64,
    dismissed: bool,
    was_open: bool,
    pending: Option<Purchase>,
}

impl ShopUi {
    pub fn reset(&mut self) { *self = Self::default(); }
}

pub struct ShopPlugin;
impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopUi>().add_systems(Update, (
            actions.after(InventoryActions).in_set(UiSet::Actions),
            (sync_window, refresh_icons, refresh_cells, refresh_controls, refresh_purchase).in_set(UiSet::Draw),
        ));
    }
}

pub fn build(commands: &mut Commands, assets: &AssetServer) {
    let root = core::window(
        commands, assets, "npc_shop", "Shop", Vec2::new(184., 328.),
        Vec2::new(24., 0.), core::Anchor::LeftCenter(0.), false,
    );
    commands.entity(root).insert((GameUiRoot, ShopRoot));
    for cell in 0..CELLS {
        let x = 12. + (cell % COLUMNS) as f32 * ICON;
        let y = 34. + (cell / COLUMNS) as f32 * ICON;
        shop_cell(commands, assets, root, cell as u8, core::rect(x, y, ICON, ICON));
    }
    for (x, label, action) in [(21., "Buy", ShopButton::Buy), (104., "Sell", ShopButton::Sell)] {
        let button = core::button(commands, assets, root, core::rect(x, 295., 61., 21.),
            "d:/ymir work/ui/public/middle_button", label);
        commands.entity(button).insert((action, Selected(false)));
    }
    for tab in 0..3u8 {
        let button = core::button(commands, assets, root, core::rect(21., 295., 43., 21.),
            "d:/ymir work/ui/public/small_button", "");
        commands.entity(button).insert((ShopButton::Tab(tab), Selected(false)));
        let label = core::text(commands, button, "", Node {
            width: Val::Percent(100.), height: Val::Percent(100.), display: Display::Flex,
            justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default()
        });
        commands.entity(label).insert(TabName(tab));
    }
    let notice = core::text(commands, root, "", core::rect(12., 280., 160., 14.));
    commands.entity(notice).insert((ShopNotice, TextFont { font_size: bevy::text::FontSize::Px(10.), ..default() }, TextColor(Color::srgb(1., 0.72, 0.5))));

    let dialog = core::window(commands, assets, "shop_purchase", "Buy item", Vec2::new(300., 140.),
        Vec2::new(250., 170.), core::Anchor::TopLeft, false);
    commands.entity(dialog).insert((GameUiRoot, PurchaseDialog));
    let prompt = core::text(commands, dialog, "", core::rect(18., 37., 264., 55.));
    commands.entity(prompt).insert((PurchaseText, TextLayout::justify(Justify::Center)));
    for (x, label, action) in [(54., "Yes", ShopButton::Yes), (170., "No", ShopButton::No)] {
        let button = core::button(commands, assets, dialog, core::rect(x, 101., 76., 22.),
            "d:/ymir work/ui/public/middle_button", label);
        commands.entity(button).insert(action);
    }
}

fn shop_cell(commands: &mut Commands, assets: &AssetServer, parent: Entity, cell: u8, node: Node) {
    let entity = commands.spawn((ShopCell(cell), Control, Surface, Hovered::default(), Tooltip(String::new()), node, ZIndex(0))).id();
    commands.entity(parent).add_child(entity);
    core::picture(commands, assets, entity, "d:/ymir work/ui/public/slot_base.sub", core::rect(0., 0., ICON, ICON));
    let icon = commands.spawn((ShopIcon { slot: entity, cell },
        Node { display: Display::None, ..core::rect(0., 0., ICON, ICON) }, ImageNode::default(), ZIndex(3), Pickable::IGNORE)).id();
    commands.entity(entity).add_child(icon);
    let count = core::text(commands, entity, "", core::rect(0., 17., 30., 15.));
    commands.entity(count).insert((ShopCount(cell), ZIndex(4), TextLayout::justify(Justify::Right)));
}

fn item_height(catalog: &ItemCatalog, vnum: u32) -> usize {
    usize::from(catalog.get(vnum).map_or(1, |data| data.table.size.clamp(1, 3)))
}

/// Shop packet entries are anchored at their grid position, with taller items
/// covering the cells below that position in the same column.
fn anchor_for_cell(tab: &ShopTab, catalog: &ItemCatalog, cell: usize) -> Option<usize> {
    if cell >= CELLS { return None; }
    tab.items.iter().take(CELLS).enumerate().find_map(|(anchor, item)| {
        (item.vnum != 0 && cell >= anchor && cell % COLUMNS == anchor % COLUMNS
            && (cell - anchor) / COLUMNS < item_height(catalog, item.vnum)).then_some(anchor)
    })
}

fn cancel_purchase(ui: &mut ShopUi, manager: &mut WindowManager, dialog: Entity, window: &mut FloatingWindow) {
    ui.pending = None;
    window.open = false;
    if manager.modal == Some(dialog) { manager.modal = None; }
}

#[allow(clippy::too_many_arguments)]
fn actions(
    mut clicks: MessageReader<Click>, cells: Query<&ShopCell>, buttons: Query<&ShopButton>,
    shop: Res<ShopState>, catalog: Res<ItemCatalog>, keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<ShopUi>, mut manager: ResMut<WindowManager>, mut consumed: ResMut<Consumed>,
    mut dialogs: Query<(Entity, &mut FloatingWindow, &mut GlobalZIndex), With<PurchaseDialog>>,
    mut actions: MessageWriter<PlayerAction>,
) {
    let Ok((dialog, mut window, mut z)) = dialogs.single_mut() else { return };
    if ui.pending.is_some() && (!window.open || !shop.open || ui.dismissed || ui.generation != shop.generation) {
        cancel_purchase(&mut ui, &mut manager, dialog, &mut window);
    }
    if ui.pending.is_some() && keys.just_pressed(KeyCode::Escape) {
        cancel_purchase(&mut ui, &mut manager, dialog, &mut window);
        consumed.keyboard = true;
    }
    for click in clicks.read() {
        if click.button != PointerButton::Primary { continue; }
        if let Ok(button) = buttons.get(click.entity) {
            match *button {
                ShopButton::Yes if ui.pending.is_some() && manager.modal == Some(dialog) => {
                    let purchase = ui.pending.unwrap();
                    let current = shop.tabs.get(usize::from(purchase.tab))
                        .and_then(|tab| tab.items.get(usize::from(purchase.cell)));
                    if shop.open && shop.generation == purchase.generation
                        && current.is_some_and(|item| item.vnum == purchase.vnum && item.count == purchase.count && item.price == purchase.price)
                    {
                        actions.write(PlayerAction::ShopBuy(purchase.tab * CELLS as u8 + purchase.cell));
                    }
                    cancel_purchase(&mut ui, &mut manager, dialog, &mut window);
                }
                ShopButton::No if ui.pending.is_some() && manager.modal == Some(dialog) => {
                    cancel_purchase(&mut ui, &mut manager, dialog, &mut window);
                }
                ShopButton::Buy if manager.modal.is_none() => ui.sell_mode = false,
                ShopButton::Sell if manager.modal.is_none() => ui.sell_mode = true,
                ShopButton::Tab(tab) if manager.modal.is_none() && usize::from(tab) < shop.tabs.len() => {
                    ui.tab = tab;
                    ui.sell_mode = false;
                }
                _ => {}
            }
        } else if manager.modal.is_none() && shop.open && !ui.dismissed && !ui.sell_mode {
            let Ok(cell) = cells.get(click.entity) else { continue };
            let Some(tab) = shop.tabs.get(usize::from(ui.tab)) else { continue };
            let Some(anchor) = anchor_for_cell(tab, &catalog, usize::from(cell.0)) else { continue };
            let item = &tab.items[anchor];
            ui.pending = Some(Purchase { generation: shop.generation, tab: ui.tab, cell: anchor as u8,
                vnum: item.vnum, count: item.count, price: item.price });
            manager.serial += 1;
            z.0 = 200 + manager.serial;
            manager.modal = Some(dialog);
            window.open = true;
        }
    }
}

fn sync_window(shop: Res<ShopState>, mut ui: ResMut<ShopUi>, mut windows: Query<&mut FloatingWindow, With<ShopRoot>>, mut actions: MessageWriter<PlayerAction>) {
    let Ok(mut window) = windows.single_mut() else { return };
    if ui.generation != shop.generation {
        ui.generation = shop.generation; ui.tab = 0; ui.sell_mode = false; ui.dismissed = false;
    }
    if ui.was_open && !window.open && shop.open && !ui.dismissed {
        ui.dismissed = true;
        actions.write(PlayerAction::ShopEnd);
    }
    if !shop.open { ui.dismissed = false; ui.tab = 0; ui.sell_mode = false; }
    window.open = shop.open && !ui.dismissed;
    ui.was_open = window.open;
}

fn refresh_icons(assets: Res<AssetServer>, catalog: Res<ItemCatalog>, shop: Res<ShopState>, ui: Res<ShopUi>,
    mut icons: Query<(&ShopIcon, &mut Node, &mut ImageNode)>, mut slot_z: Query<&mut ZIndex, With<ShopCell>>) {
    let tab = shop.tabs.get(usize::from(ui.tab));
    for (icon, mut node, mut image) in &mut icons {
        let item = tab.and_then(|tab| tab.items.get(usize::from(icon.cell))).filter(|item| item.vnum != 0);
        if let Ok(mut z) = slot_z.get_mut(icon.slot) { z.0 = if item.is_some() { 5 } else { 0 }; }
        if let Some(item) = item {
            let path = catalog.get(item.vnum).map(|data| String::from_utf8_lossy(&data.icon_file_name).into_owned())
                .filter(|path| !path.is_empty()).unwrap_or_else(|| format!("icon/item/{:05}.tga", item.vnum));
            image.image = assets.load(paths::asset_source_uri(&path));
            node.display = Display::Flex;
            node.height = Val::Px(item_height(&catalog, item.vnum) as f32 * ICON);
        } else { node.display = Display::None; }
    }
}

fn refresh_cells(catalog: Res<ItemCatalog>, shop: Res<ShopState>, ui: Res<ShopUi>,
    mut cells: Query<(&ShopCell, &mut Tooltip)>, mut counts: Query<(&ShopCount, &mut Text, &mut Node)>) {
    let tab = shop.tabs.get(usize::from(ui.tab));
    for (cell, mut tooltip) in &mut cells {
        tooltip.0 = tab.and_then(|tab| anchor_for_cell(tab, &catalog, usize::from(cell.0)).map(|anchor| (&tab.items[anchor], tab.coin_type)))
            .map_or_else(String::new, |(item, coin)| format!("{}\n{} {}", item_name(&catalog, item.vnum), item.price, if coin == 1 { "Won" } else { "Yang" }));
    }
    for (count, mut text, mut node) in &mut counts {
        let item = tab.and_then(|tab| tab.items.get(usize::from(count.0))).filter(|item| item.vnum != 0);
        text.0 = item.filter(|item| item.count > 1).map_or_else(String::new, |item| item.count.to_string());
        node.top = Val::Px(item.map_or(17., |item| item_height(&catalog, item.vnum) as f32 * ICON - 15.));
    }
}

fn refresh_controls(shop: Res<ShopState>, ui: Res<ShopUi>,
    mut buttons: Query<(&ShopButton, &mut Node, &mut Selected)>,
    mut names: Query<(&TabName, &mut Text), Without<ShopNotice>>,
    mut notices: Query<&mut Text, (With<ShopNotice>, Without<TabName>)>) {
    for (button, mut node, mut selected) in &mut buttons {
        match *button {
            ShopButton::Buy | ShopButton::Sell => {
                node.display = if shop.tabs.len() <= 1 { Display::Flex } else { Display::None };
                selected.0 = matches!(*button, ShopButton::Sell) == ui.sell_mode;
            }
            ShopButton::Tab(tab) => {
                let count = shop.tabs.len();
                node.display = if count > 1 && usize::from(tab) < count { Display::Flex } else { Display::None };
                let (x, width) = if count == 2 { (21. + f32::from(tab) * 83., 61.) } else { (21. + f32::from(tab) * 50., 43.) };
                node.left = Val::Px(x); node.width = Val::Px(width);
                selected.0 = ui.tab == tab;
            }
            _ => {}
        }
    }
    for (name, mut text) in &mut names {
        text.0 = shop.tabs.get(usize::from(name.0)).map_or_else(String::new, |tab| tab.name.clone());
    }
    for mut text in &mut notices { text.0 = shop.message.clone().unwrap_or_default(); }
}

fn item_name(catalog: &ItemCatalog, vnum: u32) -> String {
    catalog.get(vnum).map(|data| data.table.locale_name_utf8(1252).into_owned()).filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Item {vnum}"))
}

fn refresh_purchase(catalog: Res<ItemCatalog>, shop: Res<ShopState>, ui: Res<ShopUi>, mut prompts: Query<&mut Text, With<PurchaseText>>) {
    let text = ui.pending.map_or_else(String::new, |purchase| {
        let coin = shop.tabs.get(usize::from(purchase.tab)).map_or("Yang", |tab| if tab.coin_type == 1 { "Won" } else { "Yang" });
        format!("Buy {} x{}?\n{} {}", item_name(&catalog, purchase.vnum), purchase.count, purchase.price, coin)
    });
    for mut prompt in &mut prompts { prompt.0.clone_from(&text); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use m2_formats::proto::{ItemData, ItemTable};
    use m2_net::wire::TShopItemData;

    fn shop_fixture() -> (ShopTab, ItemCatalog) {
        let mut catalog = ItemCatalog::default();
        for (vnum, size) in [(10, 2), (20, 3), (30, 1)] {
            catalog.0.items.insert(vnum, ItemData { table: ItemTable { vnum, size, ..default() }, ..default() });
        }
        let mut items = vec![TShopItemData::default(); CELLS];
        items[0] = TShopItemData { vnum: 10, count: 1, price: 100, ..default() };
        items[1] = TShopItemData { vnum: 20, count: 1, price: 200, ..default() };
        items[2] = TShopItemData { vnum: 30, count: 200, price: 300, ..default() };
        (ShopTab { name: "Shop".into(), coin_type: 0, items }, catalog)
    }

    #[test]
    fn tall_items_are_clickable_across_their_cells() {
        let (tab, catalog) = shop_fixture();
        assert_eq!(anchor_for_cell(&tab, &catalog, 0), Some(0));
        assert_eq!(anchor_for_cell(&tab, &catalog, 5), Some(0));
        assert_eq!(anchor_for_cell(&tab, &catalog, 10), None);
        assert_eq!(anchor_for_cell(&tab, &catalog, 1), Some(1));
        assert_eq!(anchor_for_cell(&tab, &catalog, 6), Some(1));
        assert_eq!(anchor_for_cell(&tab, &catalog, 11), Some(1));
        assert_eq!(anchor_for_cell(&tab, &catalog, 16), None);
        assert_eq!(anchor_for_cell(&tab, &catalog, CELLS), None);
    }

    #[test]
    fn buying_requires_confirmation_and_uses_the_anchor_slot() {
        let (tab, catalog) = shop_fixture();
        let mut app = App::new();
        app.add_message::<Click>().add_message::<PlayerAction>()
            .insert_resource(ShopState { open: true, generation: 1, tabs: vec![tab], ..default() })
            .insert_resource(ShopUi { generation: 1, ..default() })
            .insert_resource(catalog).init_resource::<WindowManager>().init_resource::<Consumed>()
            .init_resource::<ButtonInput<KeyCode>>().add_systems(Update, (actions, refresh_purchase).chain());
        app.world_mut().spawn((PurchaseDialog, FloatingWindow { id: "shop_purchase", open: false,
            size: Vec2::ZERO, position: Vec2::ZERO, moved: false, anchor: core::Anchor::TopLeft }, GlobalZIndex(200)));
        let lower_cell = app.world_mut().spawn(ShopCell(11)).id();
        let potion = app.world_mut().spawn(ShopCell(2)).id();
        let yes = app.world_mut().spawn(ShopButton::Yes).id();
        let no = app.world_mut().spawn(ShopButton::No).id();
        let prompt = app.world_mut().spawn((PurchaseText, Text::new(""))).id();
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();

        app.world_mut().write_message(Click { entity: lower_cell, button: PointerButton::Primary, count: 1 });
        app.update();
        assert_eq!(app.world().resource::<ShopUi>().pending.unwrap().cell, 1);
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);
        app.world_mut().write_message(Click { entity: no, button: PointerButton::Primary, count: 1 });
        app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);

        app.world_mut().write_message(Click { entity: lower_cell, button: PointerButton::Primary, count: 1 });
        app.update();
        app.world_mut().write_message(Click { entity: yes, button: PointerButton::Primary, count: 1 });
        app.update();
        let sent: Vec<_> = cursor.read(app.world().resource::<Messages<PlayerAction>>()).collect();
        assert!(matches!(sent.as_slice(), [PlayerAction::ShopBuy(1)]));

        app.world_mut().write_message(Click { entity: potion, button: PointerButton::Primary, count: 1 });
        app.update();
        assert!(app.world().get::<Text>(prompt).unwrap().0.contains("x200"));
        app.world_mut().resource_mut::<ShopState>().tabs[0].items[2].price = 400;
        app.world_mut().write_message(Click { entity: yes, button: PointerButton::Primary, count: 1 });
        app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);
    }
}
