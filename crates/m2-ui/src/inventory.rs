//! The inventory, equipment and costume windows, from
//! `locale/en/ui/inventorywindow.py` and `costumewindow.py`.
//!
//! The server owns every item: the UI reads [`PlayerState`] and only sends
//! [`PlayerAction`]s. Icons are the original `icon/item/NNNNN.tga` artwork,
//! resolved through `item_list.txt`/`item_proto` in [`ItemCatalog`].

use bevy::{
    input_focus::{FocusCause, InputFocus},
    picking::{
        events::{PointerDragDrop, PointerDragEnd, PointerDragStart},
        hover::Hovered,
        pointer::PointerButton,
    },
    prelude::*,
    text::EditableText,
    ui_widgets::TextInput,
    window::PrimaryWindow,
};
use m2_game::player::{ItemCatalog, ItemPosition, PlayerAction, PlayerState};

use crate::{
    quickslots::QuickSlotUse,
    core::{
        self, Art, Click, Consumed, Control, FloatingWindow, Selected, Surface, Tooltip, UiSet,
        WindowManager,
    },
    hud::{ChatState, GameUiRoot},
};

/// `InventoryWindow.py`: two pages of 45 cells.
const PAGE_CELLS: u16 = 45;
/// `EQUIPMENT_START_INDEX`: the item array is shared, equipment starts at 90.
const EQUIPMENT_START: u16 = 90;
/// `COSTUME_SLOT_START` (`c_Equipment_Start + 19`).
const COSTUME_START: u16 = 109;
const ICON: f32 = 32.0;

/// The slot a widget shows.
#[derive(Component, Clone, Copy)]
pub enum Slot {
    /// A cell of the visible page (0..45).
    Bag(u16),
    /// An item-array cell (equipment and costume, 90 and up).
    Equipment(u16),
    /// A key of the visible quick-slot page (0..8).
    Quick(u8),
}

#[derive(Component)]
struct Icon {
    slot: Entity,
    vnum: u32,
}
#[derive(Component)]
struct Count(Entity);
#[derive(Component, Clone, Copy)]
enum Action {
    Page(u16),
    QuickPage(i8),
    Toggle(&'static str),
    Accept,
    Cancel,
}
#[derive(Component)]
struct Money;
#[derive(Component)]
struct Quantity;
#[derive(Component)]
struct Prompt;
#[derive(Component)]
struct Ghost(u32);
#[derive(Component)]
struct ItemTip;
#[derive(Component)]
struct ItemTipText;
/// The task bar's quick-slot page number.
#[derive(Component)]
pub struct QuickPageLabel;

#[derive(Clone, Copy)]
struct Held {
    pos: ItemPosition,
    vnum: u32,
    /// `0` means the whole stack.
    count: u8,
}
#[derive(Clone, Copy)]
enum Pending {
    Split(Held),
    Use(Held),
}

/// The inventory's view state (the server's copy is in [`PlayerState`]).
#[derive(Resource, Default)]
pub struct InventoryUi {
    pub page: u16,
    pub quick_page: u8,
    held: Option<Held>,
    pending: Option<Pending>,
    /// A skill or emotion on the cursor (`mouseController.AttachObject` with
    /// `SLOT_TYPE_SKILL`/`_EMOTION`, or a task bar slot picked up), dropped on
    /// a quick slot to bind it.
    pub carried: Option<Carried>,
}

/// What [`InventoryUi::carried`] holds: the binding and, when it was picked
/// up from the task bar, the global quick slot it came from (a drop then
/// swaps, `SLOT_TYPE_QUICK_SLOT`'s `RequestMoveGlobalQuickSlotToLocalQuickSlot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carried {
    pub kind: u8,
    pub position: u8,
    pub from_quick: Option<u8>,
}

impl InventoryUi {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The item currently carried by the inventory cursor.
    pub fn held_item(&self) -> Option<(ItemPosition, u32)> {
        self.held.map(|held| (held.pos, held.vnum))
    }

    /// Clears the carried item after another window accepts it.
    pub fn clear_held(&mut self) {
        self.held = None;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InventoryActions;

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUi>()
            .add_systems(Update, (actions, drop_carried_on_world).chain().in_set(UiSet::Actions).in_set(InventoryActions))
            .add_systems(Update, (refresh, tooltip).chain().in_set(UiSet::Draw))
            .add_observer(drag_start)
            .add_observer(drag_drop)
            .add_observer(drag_end);
    }
}

/// Spawns the inventory, costume and item dialogs. Called once when the game
/// screen opens; every entity is tagged [`GameUiRoot`].
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    // `x = SCREEN_WIDTH - 176 - 200`, `y = SCREEN_HEIGHT - 37 - 565`.
    let root = core::window(
        commands,
        assets,
        "inventory",
        "Inventory",
        Vec2::new(176., 565.),
        Vec2::new(200., 37.),
        core::Anchor::BottomRight(Vec2::new(200., 37.)),
        true,
    );
    commands.entity(root).insert(GameUiRoot);

    // `Equipment_Base` and the eleven equipment slots of `InventoryWindow.py`.
    core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/game/windows/equipment_base.sub",
        core::rect(10., 33., 156., 188.),
    );
    for (offset, (x, y, width, height)) in [
        (39., 37., 32., 64.), // weapon
        (39., 2., 32., 32.),  // body
        (39., 145., 32., 32.),// shield
        (75., 67., 32., 32.), // wrist
        (3., 3., 32., 96.),   // hairstyle
        (114., 84., 32., 32.),// pendant
        (114., 52., 32., 32.),// earring
        (2., 113., 32., 32.), // ring
        (75., 113., 32., 32.),// gloves
        (114., 1., 32., 32.), // boots
        (75., 35., 32., 32.), // necklace/armor accessory
    ]
    .into_iter()
    .enumerate()
    {
        slot(
            commands,
            assets,
            root,
            Slot::Equipment(EQUIPMENT_START + offset as u16),
            core::rect(13. + x, 36. + y, width, height),
            false,
        );
    }

    // `Inventory_Tab_01/02`: 78x19 at x 10 and 88, y 224.
    for page in 0..2u16 {
        let tab = core::button(
            commands,
            assets,
            root,
            core::rect(10. + f32::from(page) * 78., 224., 78., 19.),
            "d:/ymir work/ui/game/windows/tab_button_large",
            if page == 0 { "I" } else { "II" },
        );
        commands.entity(tab).insert((Action::Page(page), Selected(page == 0)));
    }

    // `ItemSlot`: 5x9 grid at (8, 246), 32 px steps.
    for cell in 0..PAGE_CELLS {
        let x = 8. + f32::from(cell % 5) * ICON;
        let y = 246. + f32::from(cell / 5) * ICON;
        slot(commands, assets, root, Slot::Bag(cell), core::rect(x, y, ICON, ICON), true);
    }

    // `Money_Slot`: centred (x 31) and 28 px from the bottom (top 537).
    core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/public/parameter_slot_05.sub",
        core::rect(31., 537., 130., 18.),
    );
    core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/game/windows/money_icon.sub",
        core::rect(13., 557., 16., 16.),
    );
    let money = core::text(commands, root, "", core::rect(34., 539., 124., 16.));
    commands
        .entity(money)
        .insert((Money, TextLayout::justify(Justify::Right)));

    // `CostumeButton` on the equipment paper doll (the original `.tga` art).
    let costume = core::art_button(
        commands,
        assets,
        root,
        core::rect(88., 38., 32., 32.),
        "d:/ymir work/ui/game/costume_button",
        "",
    );
    commands.entity(costume).insert((Action::Toggle("costume"), Tooltip("Costume".into())));

    // `CostumeWindow.py` (body, hair and the third costume slot).
    let costume = core::window(
        commands,
        assets,
        "costume",
        "Costume",
        Vec2::new(140., 180.),
        Vec2::new(175., 37.),
        core::Anchor::BottomRight(Vec2::new(175., 37.)),
        false,
    );
    commands.entity(costume).insert(GameUiRoot);
    core::picture(
        commands,
        assets,
        costume,
        "locale/en/ui/costume/costume_bg.jpg",
        core::rect(13., 38., 114., 132.),
    );
    for (offset, (x, y, height)) in
        [(61., 45., 64.), (61., 8., 32.), (5., 145., 32.)].into_iter().enumerate()
    {
        slot(
            commands,
            assets,
            costume,
            Slot::Equipment(COSTUME_START + offset as u16),
            core::rect(16. + x, 41. + y, ICON, height),
            false,
        );
    }

    // The quantity/confirmation dialog (`PickMoneyDialog`'s OK/Cancel frame).
    let dialog = core::window(
        commands,
        assets,
        "item_dialog",
        "Item",
        Vec2::new(260., 130.),
        Vec2::new(270., 200.),
        core::Anchor::TopLeft,
        false,
    );
    commands.entity(dialog).insert(GameUiRoot);
    let prompt = core::text(commands, dialog, "", core::rect(16., 34., 228., 30.));
    commands.entity(prompt).insert(Prompt);
    let quantity = commands
        .spawn((
            Quantity,
            TextInput,
            Surface,
            Hovered::default(),
            core::rect(22., 65., 80., 20.),
            EditableText {
                max_characters: Some(3),
                ..EditableText::new("1")
            },
            bevy::text::EditableTextFilter::new(|c| c.is_ascii_digit()),
            TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() },
            TextColor(Color::WHITE),
            bevy::text::TextCursorStyle { color: Color::WHITE, ..default() },
            BackgroundColor(Color::BLACK),
        ))
        .id();
    commands.entity(dialog).add_child(quantity);
    for (x, title, action) in [(36., "OK", Action::Accept), (142., "Cancel", Action::Cancel)] {
        let button = core::button(
            commands,
            assets,
            dialog,
            core::rect(x, 94., 80., 24.),
            "d:/ymir work/ui/public/middle_button",
            title,
        );
        commands.entity(button).insert(action);
    }

    // The item under the cursor while it is attached, and its tooltip.
    commands.spawn((
        GameUiRoot,
        Ghost(0),
        Node { display: Display::None, ..core::rect(0., 0., ICON, ICON) },
        ImageNode::default(),
        GlobalZIndex(100_002),
        Pickable::IGNORE,
    ));
    let tip = commands
        .spawn((
            GameUiRoot,
            ItemTip,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(0.),
                top: Val::Px(0.),
                width: Val::Px(280.),
                padding: UiRect::all(Val::Px(8.)),
                border: UiRect::all(Val::Px(1.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.9)),
            BorderColor::all(Color::srgb(0.7, 0.65, 0.5)),
            GlobalZIndex(100_001),
            Pickable::IGNORE,
        ))
        .id();
    let label = core::text(commands, tip, "", Node { width: Val::Percent(100.), ..default() });
    commands.entity(label).insert(ItemTipText);
}

/// A 32-px item cell with its icon and count, from `ui.SlotWindow`.
pub fn slot(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    kind: Slot,
    node: Node,
    background: bool,
) -> Entity {
    let entity = commands.spawn((kind, Control, Surface, Hovered::default(), node, BackgroundColor(Color::NONE), ZIndex(0))).id();
    commands.entity(parent).add_child(entity);
    if background {
        core::picture(commands, assets, entity, "d:/ymir work/ui/public/slot_base.sub", core::rect(0., 0., ICON, ICON));
    }
    let icon = commands
        .spawn((
            Icon { slot: entity, vnum: 0 },
            Node { display: Display::None, ..core::rect(0., 0., ICON, ICON) },
            ImageNode::default(),
            ZIndex(3),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(entity).add_child(icon);
    let count = core::text(commands, entity, "", core::rect(0., 17., 30., 15.));
    commands
        .entity(count)
        .insert((Count(entity), ZIndex(4), TextLayout::justify(Justify::Right)));
    entity
}

/// A quick-slot page arrow (`QuickPageUpButton` / `QuickPageDownButton`).
pub fn page_button(commands: &mut Commands, assets: &AssetServer, parent: Entity, delta: i8, node: Node) {
    let prefix = if delta < 0 {
        "d:/ymir work/ui/game/taskbar/quickslot_upbutton"
    } else {
        "d:/ymir work/ui/game/taskbar/quickslot_downbutton"
    };
    let id = core::button(commands, assets, parent, node, prefix, "");
    commands.entity(id).insert(Action::QuickPage(delta));
}

/// `CPythonPlayer` maps both INVENTORY and EQUIPMENT onto the same item array;
/// [`PlayerState`] stores them in window 1.
pub fn position(slot: Slot, ui: &InventoryUi, player: &PlayerState, catalog: &ItemCatalog) -> Option<ItemPosition> {
    match slot {
        Slot::Equipment(cell) => Some(ItemPosition::inventory(cell)),
        Slot::Quick(index) => {
            let quick = player.quickslots[(ui.quick_page * 8 + index) as usize];
            (quick.Type == 1).then(|| ItemPosition::inventory(quick.Position as u16))
        }
        Slot::Bag(local) => {
            let cell = ui.page * PAGE_CELLS + local;
            let page_start = ui.page * PAGE_CELLS;
            for (pos, item) in &player.items {
                if pos.window != 1 || pos.cell >= 90 {
                    continue;
                }
                // Multi-cell items occupy `size` rows in one column.
                let size = u16::from(catalog.get(item.vnum).map_or(1, |data| data.table.size.clamp(1, 3)));
                if pos.cell < page_start || pos.cell >= page_start + PAGE_CELLS {
                    continue;
                }
                if cell >= pos.cell && cell % 5 == pos.cell % 5 && (cell - pos.cell) / 5 < size {
                    return Some(*pos);
                }
            }
            Some(ItemPosition::inventory(cell))
        }
    }
}

fn empty_quickslot(player: &PlayerState) -> Option<u8> {
    (0..32u8).find(|index| player.quickslots[*index as usize].Type == 0)
}

fn confirm_when_use(item: &m2_game::player::ItemStack, catalog: &ItemCatalog) -> bool {
    item.flags & (1 << 9) != 0 || catalog.get(item.vnum).is_some_and(|data| data.table.flags & (1 << 9) != 0)
}

#[allow(clippy::too_many_arguments)]
fn apply_slot(
    slot: Slot,
    right: bool,
    double: bool,
    shift: bool,
    ctrl: bool,
    ui: &mut InventoryUi,
    player: &PlayerState,
    catalog: &ItemCatalog,
    requests: &mut MessageWriter<PlayerAction>,
    quick: &mut MessageWriter<QuickSlotUse>,
) {
    if let Slot::Quick(index) = slot {
        let global = ui.quick_page * 8 + index;
        let quick_slot = player.quickslots[global as usize];
        if let Some(carried) = ui.carried.take() {
            match carried.from_quick {
                Some(source) => requests.write(PlayerAction::SwapQuickslot { source, destination: global }),
                None => requests.write(PlayerAction::BindQuickslot { slot: global, kind: carried.kind, position: carried.position }),
            };
            return;
        }
        if let Some(held) = ui.held.take() {
            if player.items.get(&held.pos).is_some_and(|item| item.vnum == held.vnum) {
                requests.write(PlayerAction::BindItem { slot: ui.quick_page * 8 + index, position: held.pos });
            }
            return;
        }
        // `uitaskbar.SelectItemQuickSlot`: a left click picks the slot up
        // (`SLOT_TYPE_QUICK_SLOT`) to move it; `UnselectItemQuickSlot` (the
        // right click) and the number keys use it (`RequestUseLocalQuickSlot`).
        if !right {
            if quick_slot.Type != 0 {
                ui.carried = Some(Carried { kind: quick_slot.Type, position: quick_slot.Position, from_quick: Some(global) });
            }
            return;
        }
        match quick_slot.Type {
            crate::quickslots::ITEM => {
                let pos = ItemPosition::inventory(quick_slot.Position as u16);
                if player.items.contains_key(&pos) {
                    requests.write(PlayerAction::UseItem(pos));
                }
            }
            crate::quickslots::SKILL => {
                quick.write(QuickSlotUse::Skill(u32::from(quick_slot.Position)));
            }
            crate::quickslots::EMOTION => {
                quick.write(QuickSlotUse::Emotion(u32::from(quick_slot.Position)));
            }
            _ => {}
        }
        return;
    }
    let Some(pos) = position(slot, ui, player, catalog) else { return };
    if right || double {
        ui.held = None;
        if let Some(item) = player.items.get(&pos) {
            let held = Held { pos, vnum: item.vnum, count: 0 };
            if confirm_when_use(item, catalog) {
                ui.pending = Some(Pending::Use(held));
            } else {
                requests.write(PlayerAction::UseItem(pos));
            }
        }
        return;
    }
    if let Some(held) = ui.held.take() {
        if player.items.get(&held.pos).is_none_or(|item| item.vnum != held.vnum) {
            return;
        }
        if pos != held.pos {
            let count = player.items.get(&held.pos).map_or(0, |item| {
                if held.count == 0 { item.count } else { held.count.min(item.count) }
            });
            requests.write(PlayerAction::MoveItem { source: held.pos, destination: pos, count });
        }
        return;
    }
    let Some(item) = player.items.get(&pos) else { return };
    if shift && item.count > 1 {
        ui.pending = Some(Pending::Split(Held { pos, vnum: item.vnum, count: 0 }));
    } else if ctrl {
        if let Some(slot) = empty_quickslot(player) {
            requests.write(PlayerAction::BindItem { slot, position: pos });
        }
    } else {
        ui.held = Some(Held { pos, vnum: item.vnum, count: 0 });
    }
}

#[allow(clippy::too_many_arguments)]
fn actions(
    mut clicks: MessageReader<Click>,
    slots: Query<&Slot>,
    buttons: Query<&Action>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<InventoryUi>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    shop: (Res<m2_game::player::ShopState>, Res<crate::shop::ShopUi>),
    mut requests: MessageWriter<PlayerAction>,
    mut quick: MessageWriter<QuickSlotUse>,
    mut windows: Query<(Entity, &mut FloatingWindow)>,
    mut quantity: Query<(Entity, &mut EditableText, &mut Node), With<Quantity>>,
    mut manager: ResMut<WindowManager>,
    mut focus: ResMut<InputFocus>,
    mut consumed: ResMut<Consumed>,
    chat: Res<ChatState>,
) {
    if windows.is_empty() {
        return;
    }
    let typing = focus.get().is_some() || chat.typing;
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !typing && manager.modal.is_none() {
        if keys.just_pressed(KeyCode::KeyI) {
            for (_, mut window) in &mut windows {
                if window.id == "inventory" {
                    window.open = !window.open;
                }
            }
            consumed.keyboard = true;
        }
        // 1-4 and F1-F4 use the visible quick-slot page; Shift+1-4 changes it.
        for (index, key) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
        ]
        .into_iter()
        .enumerate()
        {
            if keys.just_pressed(key) {
                if shift && index < 4 {
                    ui.quick_page = index as u8;
                } else {
                    apply_slot(Slot::Quick(index as u8), true, false, false, false, &mut ui, &player, &catalog, &mut requests, &mut quick);
                }
                consumed.keyboard = true;
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) && (ui.held.is_some() || ui.pending.is_some() || ui.carried.is_some()) {
        ui.held = None;
        ui.carried = None;
        ui.pending = None;
        manager.modal = None;
        focus.clear();
        consumed.keyboard = true;
    }
    if keys.just_pressed(KeyCode::Enter) && ui.pending.is_some() && focus.get().is_some() {
        accept(&mut ui, &player, &mut requests, &mut quantity, &mut manager, &mut focus);
        consumed.keyboard = true;
    }

    let drag_dropped = consumed.drag_dropped;
    for click in clicks.read() {
        if drag_dropped {
            continue;
        }
        if let Ok(slot) = slots.get(click.entity) {
            if manager.modal.is_none() {
                if shop.0.open && (click.button == PointerButton::Secondary
                    || (shop.1.sell_mode && click.button == PointerButton::Primary)) {
                    if let Some(pos) = position(*slot, &ui, &player, &catalog)
                        && pos.window == 1
                        && pos.cell < 90
                        && player.items.contains_key(&pos)
                    {
                        requests.write(PlayerAction::ShopSell { position: pos.cell as u8, count: 1 });
                    }
                    ui.held = None;
                    continue;
                }
                apply_slot(
                    *slot,
                    click.button == PointerButton::Secondary,
                    click.count >= 2,
                    shift,
                    ctrl,
                    &mut ui,
                    &player,
                    &catalog,
                    &mut requests,
                    &mut quick,
                );
            }
            continue;
        }
        if click.button != PointerButton::Primary {
            continue;
        }
        let Ok(action) = buttons.get(click.entity) else { continue };
        match *action {
            Action::Page(page) => ui.page = page,
            Action::QuickPage(delta) => {
                ui.quick_page = (i16::from(ui.quick_page) + i16::from(delta)).rem_euclid(4) as u8;
            }
            Action::Toggle(id) => {
                for (_, mut window) in &mut windows {
                    if window.id == id {
                        window.open = !window.open;
                    }
                }
            }
            Action::Cancel => {
                ui.pending = None;
                manager.modal = None;
                focus.clear();
            }
            Action::Accept => accept(&mut ui, &player, &mut requests, &mut quantity, &mut manager, &mut focus),
        }
    }

    // The dialog follows the pending action; closing it cancels.
    for (entity, mut window) in &mut windows {
        if window.id != "item_dialog" {
            continue;
        }
        if ui.pending.is_some() {
            if manager.modal == Some(entity) && !window.open {
                // The title bar's close button cancelled the dialog.
                ui.pending = None;
                manager.modal = None;
                focus.clear();
                continue;
            }
            if manager.modal != Some(entity) {
                window.open = true;
                manager.modal = Some(entity);
                for (_, mut editor, mut node) in &mut quantity {
                    editor.editor.set_text("1");
                    node.display = if matches!(ui.pending, Some(Pending::Split(_))) {
                        Display::Flex
                    } else {
                        Display::None
                    };
                }
                if matches!(ui.pending, Some(Pending::Split(_))) {
                    if let Some((field, _, _)) = quantity.iter_mut().next() {
                        focus.set(field, FocusCause::Navigated);
                    }
                }
            }
        } else if window.open {
            window.open = false;
            manager.modal = None;
        }
    }
    if ui.held.is_some() {
        consumed.pointer = true;
    }
}

fn accept(
    ui: &mut InventoryUi,
    player: &PlayerState,
    requests: &mut MessageWriter<PlayerAction>,
    quantity: &mut Query<(Entity, &mut EditableText, &mut Node), With<Quantity>>,
    manager: &mut WindowManager,
    focus: &mut InputFocus,
) {
    let Some(pending) = ui.pending.take() else { return };
    let held = match pending {
        Pending::Split(held) | Pending::Use(held) => held,
    };
    let Some(item) = player.items.get(&held.pos).filter(|item| item.vnum == held.vnum) else {
        manager.modal = None;
        focus.clear();
        return;
    };
    match pending {
        Pending::Use(_) => {
            requests.write(PlayerAction::UseItem(held.pos));
        }
        Pending::Split(_) => {
            let count = quantity
                .iter_mut()
                .next()
                .and_then(|(_, editor, _)| editor.value().to_string().parse::<u8>().ok())
                .filter(|count| *count > 0 && *count <= item.count);
            if let Some(count) = count {
                ui.held = Some(Held { count, ..held });
            }
        }
    }
    manager.modal = None;
    focus.clear();
}

fn drag_start(
    mut event: On<PointerDragStart>,
    slots: Query<&Slot>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    mut ui: ResMut<InventoryUi>,
    mut consumed: ResMut<Consumed>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    if let Ok(slot) = slots.get(event.entity) {
        // A skill or emotion on the task bar is picked up to move it.
        if let Slot::Quick(index) = *slot {
            let global = ui.quick_page * 8 + index;
            let quick = player.quickslots[global as usize];
            if quick.Type != 0 {
                ui.carried = Some(Carried { kind: quick.Type, position: quick.Position, from_quick: Some(global) });
                consumed.pointer = true;
                event.propagate(false);
                return;
            }
        }
        if let Some(pos) = position(*slot, &ui, &player, &catalog) {
            if let Some(item) = player.items.get(&pos) {
                ui.held = Some(Held { pos, vnum: item.vnum, count: 0 });
                consumed.pointer = true;
                event.propagate(false);
            }
        }
    }
}

fn drag_drop(
    mut event: On<PointerDragDrop>,
    slots: Query<&Slot>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    mut ui: ResMut<InventoryUi>,
    mut requests: MessageWriter<PlayerAction>,
    mut quick: MessageWriter<QuickSlotUse>,
    mut consumed: ResMut<Consumed>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    if let Ok(slot) = slots.get(event.entity) {
        apply_slot(*slot, false, false, false, false, &mut ui, &player, &catalog, &mut requests, &mut quick);
        consumed.pointer = true;
        consumed.drag_dropped = true;
        event.propagate(false);
    }
}

/// The end of a drag that no quick slot accepted: a skill or emotion taken off
/// the task bar is removed from it (`uitaskbar`'s `SLOT_TYPE_QUICK_SLOT`
/// dropped on the world), one dragged from its window is let go.
/// `game.OnMouseLeftButtonUp` with a `SLOT_TYPE_QUICK_SLOT` attached: a task
/// bar slot dropped on the world is removed (`RequestDeleteGlobalQuickSlot`);
/// a skill from its window is let go.
fn drop_carried_on_world(
    buttons: Res<ButtonInput<MouseButton>>,
    surfaces: Query<&Hovered, With<crate::core::Surface>>,
    mut ui: ResMut<InventoryUi>,
    mut requests: MessageWriter<PlayerAction>,
) {
    if (ui.carried.is_none() && ui.held.is_none()) || !buttons.just_pressed(MouseButton::Left) || surfaces.iter().any(|hovered| hovered.0) {
        return;
    }
    // A bag item let go over the world goes back (the drop question of
    // `__PutItem` is not ported).
    ui.held = None;
    if let Some(slot) = ui.carried.take().and_then(|carried| carried.from_quick) {
        requests.write(PlayerAction::ClearQuickslot(slot));
    }
}

fn drag_end(
    event: On<PointerDragEnd>,
    mut ui: ResMut<InventoryUi>,
    mut requests: MessageWriter<PlayerAction>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    if let Some(carried) = ui.carried.take()
        && let Some(slot) = carried.from_quick
    {
        requests.write(PlayerAction::ClearQuickslot(slot));
    }
}

fn icon_path(catalog: &ItemCatalog, vnum: u32) -> String {
    catalog
        .get(vnum)
        .map(|data| String::from_utf8_lossy(&data.icon_file_name).into_owned())
        .filter(|path| !path.is_empty())
        // `CItemManager::LoadItemTable`'s fallback.
        .unwrap_or_else(|| format!("icon/item/{vnum:05}.tga"))
}

fn item_size(catalog: &ItemCatalog, vnum: u32) -> f32 {
    let size = catalog.get(vnum).map_or(1, |data| data.table.size.clamp(1, 3));
    f32::from(size)
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut commands: Commands,
    assets: Res<AssetServer>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    ui: Res<InventoryUi>,
    slots: Query<&Slot>,
    mut icons: Query<(Entity, &mut Icon, &mut Node, &mut ImageNode), Without<Count>>,
    mut slot_z: Query<&mut ZIndex, With<Slot>>,
    mut counts: Query<(&Count, &mut Text, &mut Node), (Without<Icon>, Without<Money>, Without<Prompt>, Without<QuickPageLabel>)>,
    mut money: Query<&mut Text, (With<Money>, Without<Count>, Without<Prompt>, Without<QuickPageLabel>)>,
    mut prompts: Query<&mut Text, (With<Prompt>, Without<Money>, Without<Count>, Without<QuickPageLabel>)>,
    mut tabs: Query<(&Action, &mut Selected)>,
    mut pages: Query<&mut Text, (With<QuickPageLabel>, Without<Money>, Without<Count>, Without<Prompt>)>,
) {
    // Newly spawned icons still need initialization when reopening the UI.
    if !player.is_changed() && !catalog.is_changed() && !ui.is_changed()
        && icons.iter_mut().all(|(_, icon, _, _)| !icon.is_added())
    {
        return;
    }
    for (entity, mut icon, mut node, mut image) in &mut icons {
        let Ok(slot) = slots.get(icon.slot) else { continue };
        let pos = position(*slot, &ui, &player, &catalog);
        let item = pos.and_then(|pos| player.items.get(&pos));
        let anchored = match *slot {
            Slot::Bag(local) => pos.is_some_and(|pos| pos.cell == ui.page * PAGE_CELLS + local),
            _ => true,
        };
        let vnum = match item.filter(|_| anchored) {
            Some(item) => item.vnum,
            None => {
                core::set_display(&mut node, Display::None);
                icon.vnum = 0;
                if let Ok(mut z) = slot_z.get_mut(icon.slot) {
                    z.set_if_neq(ZIndex(0));
                }
                continue;
            }
        };
        // A multi-cell item draws over the cells below its origin.
        if let Ok(mut z) = slot_z.get_mut(icon.slot) {
            z.set_if_neq(ZIndex(5));
        }
        if icon.vnum != vnum {
            let art = Art::load(&assets, &icon_path(&catalog, vnum));
            if let Art::Image(handle) = &art {
                // Set the image immediately too: `resolve_art` runs in the same
                // UI set, and this slot must not wait a frame on ordering.
                image.image = handle.clone();
                image.rect = None;
            }
            commands.entity(entity).insert(art);
            icon.vnum = vnum;
        }
        let height = if matches!(slot, Slot::Quick(_)) { ICON } else { item_size(&catalog, vnum) * ICON };
        core::edit_node(&mut node, |node| {
            node.display = Display::Flex;
            node.height = Val::Px(height);
        });
        let color = if ui.held.is_some_and(|held| Some(held.pos) == pos) {
            Color::srgba(1., 1., 1., 0.45)
        } else {
            Color::WHITE
        };
        if image.color != color {
            image.color = color;
        }
    }
    for (count, mut text, mut node) in &mut counts {
        let value = slots
            .get(count.0)
            .ok()
            .and_then(|slot| {
                let pos = position(*slot, &ui, &player, &catalog)?;
                if matches!(slot, Slot::Bag(local) if pos.cell != ui.page * PAGE_CELLS + local) {
                    return None;
                }
                player.items.get(&pos).filter(|item| item.count > 1).map(|item| (item.count.to_string(), item.vnum))
            });
        let quick = slots.get(count.0).is_ok_and(|slot| matches!(slot, Slot::Quick(_)));
        let (value, height) = value.map_or((String::new(), ICON), |(value, vnum)| {
            (value, if quick { ICON } else { item_size(&catalog, vnum) * ICON })
        });
        if **text != value {
            **text = value;
        }
        // The count sits at the bottom-right of the whole item icon.
        let top = Val::Px((height - 15.).max(0.));
        if node.top != top {
            node.top = top;
        }
    }
    for mut text in &mut money {
        let value = money_string(player.points[11]);
        if **text != value {
            **text = value;
        }
    }
    for mut text in &mut prompts {
        let value = match ui.pending {
            Some(Pending::Split(held)) => {
                format!("Quantity (1-{})", player.items.get(&held.pos).map_or(0, |item| item.count))
            }
            Some(Pending::Use(held)) => match catalog.get(held.vnum) {
                Some(data) => format!("Use {}?", data.table.locale_name_utf8(1252)),
                None => format!("Use item {}?", held.vnum),
            },
            None => String::new(),
        };
        if **text != value {
            **text = value;
        }
    }
    for (action, mut selected) in &mut tabs {
        if let Action::Page(page) = action {
            selected.set_if_neq(Selected(*page == ui.page));
        }
    }
    for mut text in &mut pages {
        let value = format!("{}", ui.quick_page + 1);
        if **text != value {
            **text = value;
        }
    }
}

fn money_string(value: i32) -> String {
    let digits = value.max(0).to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tooltip(
    mut commands: Commands,
    assets: Res<AssetServer>,
    primary: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    time: Res<Time>,
    player: Res<PlayerState>,
    catalog: Res<ItemCatalog>,
    ui: Res<InventoryUi>,
    hover: Query<(Entity, &Slot, &Hovered)>,
    mut tip: Query<&mut Node, With<ItemTip>>,
    mut text: Query<(Entity, &mut Text), With<ItemTipText>>,
    mut ghost: Query<(Entity, &mut Ghost, &mut Node, &mut ImageNode), Without<ItemTip>>,
    (mut last, mut shown, locale): (
        Local<Option<(Entity, f32)>>,
        Local<Vec<crate::item_tooltip::Line>>,
        Res<crate::item_tooltip::LocaleGame>,
    ),
) {
    let Ok(window) = primary.single() else { return };
    let Some(cursor) = window.cursor_position().map(|point| point / scale.0) else { return };
    let viewport = Vec2::new(window.width(), window.height()) / scale.0;

    for (entity, mut ghost, mut node, mut image) in &mut ghost {
        // An item from the bag, or the item of a task bar slot picked up.
        let carried_item = ui.carried.filter(|carried| carried.kind == crate::quickslots::ITEM).and_then(|carried| {
            player.items.get(&ItemPosition::inventory(u16::from(carried.position))).map(|item| item.vnum)
        });
        if let Some(vnum) = ui.held.map(|held| held.vnum).or(carried_item) {
            if ghost.0 != vnum {
                commands.entity(entity).insert(Art::load(&assets, &icon_path(&catalog, vnum)));
                ghost.0 = vnum;
                image.color = Color::srgba(1., 1., 1., 0.85);
            }
            let height = if carried_item.is_some() { ICON } else { item_size(&catalog, vnum) * ICON };
            core::edit_node(&mut node, |node| {
                node.display = Display::Flex;
                node.left = Val::Px(cursor.x + 8.);
                node.top = Val::Px(cursor.y + 8.);
                node.height = Val::Px(height);
            });
        } else {
            core::set_display(&mut node, Display::None);
        }
    }

    let hovered = hover.iter().find(|(_, _, hovered)| hovered.0);
    let current = hovered.map(|(entity, _, _)| entity);
    if current != last.as_ref().map(|(entity, _)| *entity) {
        *last = current.map(|entity| (entity, time.elapsed_secs()));
    }
    let item = hovered
        .and_then(|(_, slot, _)| position(*slot, &ui, &player, &catalog))
        .and_then(|pos| player.items.get(&pos));
    let show = ui.held.is_none() && last.is_some_and(|(_, at)| time.elapsed_secs() - at > 0.3) && item.is_some();
    for mut node in &mut tip {
        core::edit_node(&mut node, |node| {
            node.display = if show { Display::Flex } else { Display::None };
            node.left = Val::Px((cursor.x + 16.).min(viewport.x - 280.).max(0.));
            node.top = Val::Px((cursor.y + 18.).max(0.));
        });
    }
    if let (true, Some(item)) = (show, item) {
        let mut lines = match catalog.get(item.vnum) {
            Some(data) => crate::item_tooltip::item_lines(data, item, &catalog, &locale, player.level),
            None => vec![(format!("Item {}", item.vnum), crate::item_tooltip::color::NORMAL)],
        };
        if item.count > 1 {
            lines.push((format!("Quantity: {}", item.count), crate::item_tooltip::color::NORMAL));
        }
        if *shown != lines {
            for (entity, mut label) in &mut text {
                // The first line is the root text; the rest are coloured spans.
                **label = String::new();
                commands.entity(entity).despawn_related::<Children>();
                for (index, (line, color)) in lines.iter().enumerate() {
                    let value = if index == 0 { line.clone() } else { format!("\n{line}") };
                    commands.entity(entity).with_child((
                        TextSpan::new(value),
                        TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() },
                        TextColor(*color),
                    ));
                }
            }
            *shown = lines;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_net::wire::TQuickSlot;

    fn item(vnum: u32, count: u8) -> m2_game::player::ItemStack {
        m2_game::player::ItemStack {
            vnum,
            count,
            flags: 0,
            anti_flags: 0,
            sockets: [0; 3],
            attributes: Default::default(),
        }
    }

    #[test]
    fn money_uses_thousands_separators() {
        assert_eq!(money_string(0), "0");
        assert_eq!(money_string(999), "999");
        assert_eq!(money_string(1_000), "1,000");
        assert_eq!(money_string(1_234_567), "1,234,567");
        assert_eq!(money_string(-5), "0");
    }

    #[test]
    fn slots_map_to_the_shared_item_array() {
        let catalog = ItemCatalog::default();
        let mut player = PlayerState::default();
        player.items.insert(ItemPosition::inventory(0), item(10, 1));
        let mut ui = InventoryUi::default();

        assert_eq!(position(Slot::Bag(0), &ui, &player, &catalog), Some(ItemPosition::inventory(0)));
        assert_eq!(position(Slot::Bag(1), &ui, &player, &catalog), Some(ItemPosition::inventory(1)));
        assert_eq!(position(Slot::Equipment(90), &ui, &player, &catalog), Some(ItemPosition::inventory(90)));

        // Page II starts at cell 45.
        ui.page = 1;
        assert_eq!(position(Slot::Bag(0), &ui, &player, &catalog), Some(ItemPosition::inventory(45)));
        assert_eq!(position(Slot::Bag(44), &ui, &player, &catalog), Some(ItemPosition::inventory(89)));
    }

    #[test]
    fn quick_slots_follow_the_page() {
        let catalog = ItemCatalog::default();
        let mut player = PlayerState::default();
        player.quickslots[0] = TQuickSlot { Type: 1, Position: 7 };
        player.quickslots[8] = TQuickSlot { Type: 1, Position: 8 };
        let mut ui = InventoryUi::default();

        assert_eq!(position(Slot::Quick(0), &ui, &player, &catalog), Some(ItemPosition::inventory(7)));
        assert_eq!(position(Slot::Quick(1), &ui, &player, &catalog), None);
        ui.quick_page = 1;
        assert_eq!(position(Slot::Quick(0), &ui, &player, &catalog), Some(ItemPosition::inventory(8)));
    }

    #[test]
    fn a_server_quickslot_assignment_reaches_the_taskbar() {
        let catalog = ItemCatalog::default();
        let mut player = PlayerState::default();
        player.items.insert(ItemPosition::inventory(3), item(10, 1));
        player.apply(&m2_net::phase::NetEvent::Packet(m2_net::wire::GcPacket::QuickslotAdd(
            m2_net::wire::TPacketGCQuickSlotAdd {
                header: 0,
                pos: 0,
                slot: TQuickSlot { Type: 1, Position: 3 },
            },
        )));
        assert_eq!(
            position(Slot::Quick(0), &InventoryUi::default(), &player, &catalog),
            Some(ItemPosition::inventory(3))
        );
    }

    #[test]
    fn the_first_empty_quick_slot_is_used() {
        let mut player = PlayerState::default();
        assert_eq!(empty_quickslot(&player), Some(0));
        player.quickslots[0] = TQuickSlot { Type: 1, Position: 7 };
        assert_eq!(empty_quickslot(&player), Some(1));
    }
}
