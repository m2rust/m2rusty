//! Two-sided exchange: server-owned offers, item dragging, Yang, accept/cancel.
use bevy::{picking::{events::PointerDragDrop, hover::Hovered, pointer::PointerButton}, prelude::*, text::EditableText};
use m2_game::{player::{ItemCatalog, ItemPosition, PlayerAction, PlayerState}, social::SocialState};
use crate::{core::{self, Click, Consumed, Control, FloatingWindow, Surface, Tooltip, UiSet}, hud::GameUiRoot, inventory::{InventoryActions, InventoryUi}};
#[derive(Component)] struct ExchangeRoot;
#[derive(Component, Clone, Copy)] struct Cell { mine: bool, slot: u8 }
#[derive(Component)] struct Icon(Cell);
#[derive(Component)] struct Count(Cell);
#[derive(Component)] struct Offer(bool);
#[derive(Component)] struct Gold;
#[derive(Component, Clone, Copy)] enum Action { Accept, Cancel, Gold }
#[derive(Resource, Default)] struct ExchangeUi { was_trading: bool, was_open: bool, dismissed: bool }
pub struct ExchangePlugin;
impl Plugin for ExchangePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExchangeUi>().add_systems(Update, actions.after(InventoryActions).in_set(UiSet::Actions))
            .add_systems(Update, refresh.in_set(UiSet::Draw)).add_observer(drop_on_cell);
    }
}
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    commands.insert_resource(ExchangeUi::default());
    let root = core::window(commands, assets, "exchange", "Exchange", Vec2::new(380., 310.), Vec2::new(230., 130.), core::Anchor::TopLeft, false); commands.entity(root).insert((GameUiRoot, ExchangeRoot));
    for mine in [true, false] {
        let x = if mine {18.} else {210.};
        let text = core::text(commands, root, "", core::rect(x, 35., 152., 43.)); commands.entity(text).insert((Offer(mine), TextLayout::justify(Justify::Left)));
        for slot in 0..12u8 {
            let cell = Cell { mine, slot };
            let id = commands.spawn((cell, Control, Surface, Hovered::default(), Tooltip(String::new()), core::rect(x + f32::from(slot % 4) * 32., 87. + f32::from(slot / 4) * 32., 32., 32.))).id(); commands.entity(root).add_child(id);
            core::picture(commands, assets, id, "d:/ymir work/ui/public/slot_base.sub", core::rect(0., 0., 32., 32.));
            let icon = commands.spawn((Icon(cell), core::rect(0., 0., 32., 32.), ImageNode::default(), Pickable::IGNORE, ZIndex(2))).id(); commands.entity(id).add_child(icon);
            let count = core::text(commands, id, "", core::rect(0., 17., 32., 15.)); commands.entity(count).insert((Count(cell), ZIndex(3), TextLayout::justify(Justify::Right)));
        }
    }
    core::text(commands, root, "Yang", core::rect(18., 220., 44., 20.));
    let gold = crate::social_ui::input(commands, root, core::rect(62., 220., 115., 21.), 10); commands.entity(gold).insert(Gold);
    for (x, y, label, action) in [(190., 219., "Offer Yang", Action::Gold), (52., 268., "Accept", Action::Accept), (244., 268., "Cancel", Action::Cancel)] {
        let id = core::button(commands, assets, root, core::rect(x, y, 90., 23.), "d:/ymir work/ui/public/middle_button", label); commands.entity(id).insert(action);
    }
}
/// `uiexchange.SelectEmptySlot`: the item attached from the bag goes to the
/// chosen display slot, unless it is `ANTIFLAG_GIVE`; the attached item is
/// let go either way.
fn offer(cell: Cell, inventory: &mut InventoryUi, player: &PlayerState, catalog: &ItemCatalog, social: &SocialState, requests: &mut MessageWriter<PlayerAction>) -> bool {
    let Some((source, vnum)) = inventory.held_item() else { return false };
    if !social.exchange.trading || !cell.mine { return false; }
    let in_bag = source.window == 1 && source.cell < 90 && player.items.get(&source).is_some_and(|i| i.vnum == vnum);
    let givable = catalog.get(vnum).is_none_or(|data| data.table.anti_flags & ANTIFLAG_GIVE == 0);
    if in_bag && givable && social.exchange.me.items[usize::from(cell.slot)].is_none() {
        requests.write(PlayerAction::Exchange { subheader: 1, arg: 0, slot: cell.slot, position: source });
    }
    inventory.clear_held();
    true
}
/// `ITEM_ANTIFLAG_GIVE` (`ItemData.h`).
const ANTIFLAG_GIVE: u32 = 1 << 13;
/// An item dragged from the bag and released over an own display slot (the
/// inventory's own drop handler only knows inventory slots).
fn drop_on_cell(mut event: On<PointerDragDrop>, cells: Query<&Cell>, mut inventory: ResMut<InventoryUi>, player: Res<PlayerState>, catalog: Res<ItemCatalog>, social: Res<SocialState>, mut requests: MessageWriter<PlayerAction>, mut consumed: ResMut<Consumed>) {
    if event.button != PointerButton::Primary { return; }
    let Ok(cell) = cells.get(event.entity) else { return };
    if offer(*cell, &mut inventory, &player, &catalog, &social, &mut requests) {
        consumed.pointer = true; consumed.drag_dropped = true; event.propagate(false);
    }
}
#[allow(clippy::too_many_arguments)]
fn actions(mut clicks: MessageReader<Click>, cells: Query<&Cell>, buttons: Query<&Action>, mut inventory: ResMut<InventoryUi>, player: Res<PlayerState>, catalog: Res<ItemCatalog>, social: Res<SocialState>, fields: Query<&EditableText, With<Gold>>, mut requests: MessageWriter<PlayerAction>) {
    if !social.exchange.trading { return; }
    for click in clicks.read() {
        if let Ok(cell) = cells.get(click.entity) {
            if click.button == PointerButton::Primary { offer(*cell, &mut inventory, &player, &catalog, &social, &mut requests); }
            continue;
        }
        if click.button != PointerButton::Primary { continue; }
        if let Ok(action) = buttons.get(click.entity) {
            let (subheader, arg) = match action {
                Action::Accept => (4, 0), Action::Cancel => (5, 0), Action::Gold if social.exchange.me.elk == 0 => {
                    let Some(value) = fields.iter().next().and_then(|e| e.value().to_string().parse::<u32>().ok()).filter(|v| i64::from(*v) <= i64::from(player.points[11])) else { continue }; (3, value)
                }
                Action::Gold => continue,
            }; requests.write(PlayerAction::Exchange { subheader, arg, slot: 0, position: ItemPosition::inventory(0) });
        }
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(mut ui: ResMut<ExchangeUi>, social: Res<SocialState>, catalog: Res<ItemCatalog>, assets: Res<AssetServer>, mut windows: Query<&mut FloatingWindow, With<ExchangeRoot>>, mut offers: Query<(&Offer, &mut Text), Without<Count>>, mut counts: Query<(&Count, &mut Text), Without<Offer>>, mut icons: Query<(&Icon, &mut ImageNode, &mut Node)>, mut cells: Query<(&Cell, &mut Tooltip)>, mut requests: MessageWriter<PlayerAction>, (locale, player): (Res<crate::item_tooltip::LocaleGame>, Res<PlayerState>)) {
    let Ok(mut window) = windows.single_mut() else { return };
    let exchange = &social.exchange;
    if exchange.trading && !ui.was_trading { window.open = true; ui.dismissed = false; }
    if ui.was_open && !window.open && exchange.trading && !ui.dismissed {
        requests.write(PlayerAction::Exchange { subheader: 5, arg: 0, slot: 0, position: ItemPosition::inventory(0) }); ui.dismissed = true;
    }
    if !exchange.trading { window.open = false; }
    ui.was_trading = exchange.trading; ui.was_open = window.open;
    if !social.is_changed() && !catalog.is_changed() && !locale.is_changed() { return; }
    let side = |mine| if mine { &exchange.me } else { &exchange.target };
    for (offer, mut text) in &mut offers { let s = side(offer.0); text.0 = format!("{}{}\n{} Yang", s.name, if s.accept {" [Accepted]"} else {""}, s.elk); }
    for (count, mut text) in &mut counts { text.0 = side(count.0.mine).items[usize::from(count.0.slot)].as_ref().filter(|i| i.count > 1).map_or(String::new(), |i| i.count.to_string()); }
    for (icon, mut image, mut node) in &mut icons {
        if let Some(item) = &side(icon.0.mine).items[usize::from(icon.0.slot)] {
            node.display = Display::Flex; image.image = assets.load(m2_assets::paths::asset_source_uri(&crate::refine::icon_path(&catalog, item.vnum)));
            node.height = Val::Px(catalog.get(item.vnum).map_or(1, |d| d.table.size.clamp(1, 3)) as f32 * 32.);
        } else { node.display = Display::None; }
    }
    // `uiexchange.OverInItem` -> `SetExchangeOwnerItem`/`SetExchangeTargetItem`: the item tooltip.
    let lines = |item: &m2_game::player::ItemStack| catalog.get(item.vnum).map_or_else(|| crate::refine::item_name(&catalog, item.vnum), |data| crate::item_tooltip::item_lines(data, item, &catalog, &locale, player.level).into_iter().map(|(line, _)| line).collect::<Vec<_>>().join("\n"));
    for (cell, mut tooltip) in &mut cells { let value = side(cell.mine).items[usize::from(cell.slot)].as_ref().map_or(String::new(), |i| if i.count > 1 { format!("{}\nQuantity: {}", lines(i), i.count) } else { lines(i) }); if tooltip.0 != value { tooltip.0 = value; } }
}
