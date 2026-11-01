//! The EMOTION page of the character window (`Emoticon_Page` in
//! `uiscript/characterwindow.py`) and the emotion quick-slot overlay.
//!
//! The table is transcribed from `root/emotion.py` (`EMOTION_VERSION == 2`):
//! `EMOTION_DICT` gives the names and chat commands, `ICON_DICT` the artwork.
//! Clicking an emotion sends its command through `net.SendChatPacket`
//! (`uicharacter.__ClickEmotionSlot`); Ctrl+click binds it to the first empty
//! quick slot (`__SelectEmotion`'s `RequestAddToEmptyLocalQuickSlot`).

use std::collections::HashMap;

use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use m2_assets::subimage::SubImage;
use m2_game::net::Net;
use m2_game::player::PlayerState;
use m2_net::phase::Phase;

use crate::character::{CharacterTab, PageRoot};
use crate::core::{
    self, Art, Click, Consumed, Control, Surface, Tooltip, UiSet, WindowManager,
};
use crate::hud::ChatState;
use crate::inventory::{InventoryUi, Slot};
use crate::quickslots::{self, QuickSlotUse};

/// One entry of `emotion.EMOTION_DICT`/`ICON_DICT` (`root/emotion.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emotion {
    /// `EMOTION_*`, also the slot index of the page's grid table.
    pub id: u32,
    /// `localeInfo.EMOTION_*` (English locale).
    pub name: &'static str,
    /// The chat command sent by `net.SendChatPacket`.
    pub command: &'static str,
    /// The `ICON_DICT` artwork (a `.sub` region or a `.tga`).
    pub icon: &'static str,
}

/// The 21 emotions of `emotion.py` (`EMOTION_VERSION == 2`), in grid order:
/// ids 1..18 fill `SoloEmotionSlot`, ids 51..53 fill `DualEmotionSlot`.
pub const EMOTIONS: [Emotion; 21] = [
    Emotion { id: 1, name: "Claps", command: "/clap", icon: "d:/ymir work/ui/game/windows/emotion_clap.sub" },
    Emotion { id: 2, name: "Approval", command: "/congratulation", icon: "icon/action/congratulation.tga" },
    Emotion { id: 3, name: "Forgiveness", command: "/forgive", icon: "icon/action/forgive.tga" },
    Emotion { id: 4, name: "Aggravating", command: "/angry", icon: "icon/action/angry.tga" },
    Emotion { id: 5, name: "Seduction", command: "/attractive", icon: "icon/action/attractive.tga" },
    Emotion { id: 6, name: "Sad", command: "/sad", icon: "icon/action/sad.tga" },
    Emotion { id: 7, name: "Rejection", command: "/shy", icon: "icon/action/shy.tga" },
    Emotion { id: 8, name: "Cheer", command: "/cheerup", icon: "icon/action/cheerup.tga" },
    Emotion { id: 9, name: "Sneering", command: "/banter", icon: "icon/action/banter.tga" },
    Emotion { id: 10, name: "Joy", command: "/joy", icon: "icon/action/joy.tga" },
    Emotion { id: 11, name: "Cheers 1", command: "/cheer1", icon: "d:/ymir work/ui/game/windows/emotion_cheers_1.sub" },
    Emotion { id: 12, name: "Cheers 2", command: "/cheer2", icon: "d:/ymir work/ui/game/windows/emotion_cheers_2.sub" },
    Emotion { id: 13, name: "Dance 1", command: "/dance1", icon: "icon/action/dance1.tga" },
    Emotion { id: 14, name: "Dance 2", command: "/dance2", icon: "icon/action/dance2.tga" },
    Emotion { id: 15, name: "Dance 3", command: "/dance3", icon: "icon/action/dance3.tga" },
    Emotion { id: 16, name: "Dance 4", command: "/dance4", icon: "icon/action/dance4.tga" },
    Emotion { id: 17, name: "Dance 5", command: "/dance5", icon: "icon/action/dance5.tga" },
    Emotion { id: 18, name: "Gumsan Style", command: "/dance6", icon: "icon/action/dance6.tga" },
    Emotion { id: 51, name: "Kiss", command: "/kiss", icon: "d:/ymir work/ui/game/windows/emotion_kiss.sub" },
    Emotion { id: 52, name: "French Kiss", command: "/french_kiss", icon: "d:/ymir work/ui/game/windows/emotion_french_kiss.sub" },
    Emotion { id: 53, name: "Hit", command: "/slap", icon: "d:/ymir work/ui/game/windows/emotion_slap.sub" },
];

/// The table entry for an `EMOTION_*` id.
pub fn emotion(id: u32) -> Option<&'static Emotion> {
    EMOTIONS.iter().find(|emotion| emotion.id == id)
}

/// The page-grid position of an emotion (`characterwindow.py`'s
/// `SoloEmotionSlot`: x 30, y 33, 6x3, 32-px steps, start index 1;
/// `DualEmotionSlot`: x 30, y 180, start index 51).
fn grid_position(id: u32) -> Option<(f32, f32)> {
    let (start, y) = if (1..=18).contains(&id) {
        (1, 33.)
    } else if (51..=53).contains(&id) {
        (51, 180.)
    } else {
        return None;
    };
    let index = id - start;
    Some((30. + (index % 6) as f32 * 32., y + (index / 6) as f32 * 32.))
}

/// The emotion bound to a visible quick slot, from `TQuickSlot` (`Type == 3`)
/// and the task bar's page (`InventoryUi::quick_page`, 8 slots per page).
fn quick_slot_emotion(player: &PlayerState, quick_page: u8, local: u8) -> Option<u32> {
    if local >= 8 {
        return None;
    }
    let index = usize::from(quick_page) * 8 + usize::from(local);
    let quick = player.quickslots.get(index)?;
    (quick.Type == quickslots::EMOTION).then(|| u32::from(quick.Position))
}

/// `player.RequestAddToEmptyLocalQuickSlot`: the first global slot with `Type 0`.
fn empty_quick_slot(player: &PlayerState) -> Option<u8> {
    (0..32u8).find(|slot| player.quickslots[usize::from(*slot)].Type == 0)
}

#[derive(Component)]
struct EmotionButton(u32);

/// The emotion icon drawn over a task bar quick slot (its own child, so the
/// inventory's item `Icon` is untouched).
#[derive(Component)]
struct QuickEmotion {
    slot: Entity,
    /// `0` when the slot is not an emotion any more.
    id: u32,
}

pub struct EmotesPlugin;
impl Plugin for EmotesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, actions.in_set(UiSet::Actions))
            .add_systems(Update, quick_overlay.in_set(UiSet::Draw));
    }
}

pub fn build(commands: &mut Commands, assets: &AssetServer, parent: Entity) -> Entity {
    let page = commands
        .spawn((PageRoot(CharacterTab::Emotion), core::rect(0., 24., 250., 304.)))
        .id();
    commands.entity(parent).add_child(page);

    // `Action_Bar`/`Action_Bar_Text`: "Normal Emotion" above `SoloEmotionSlot`.
    header(commands, assets, page, 11., "Normal Emotion");
    // `Reaction_Bar`/`Reaction_Bar_Text`: "Mutual Emotion" above the second grid.
    header(commands, assets, page, 158., "Mutual Emotion");

    for entry in &EMOTIONS {
        let Some((x, y)) = grid_position(entry.id) else { continue };
        emotion_slot(commands, assets, page, entry, x, y);
    }

    page
}

/// `ui.HorizontalBar`: a 32-px left cap, a tiled centre and a 32-px right cap
/// (`horizontalbar_left/center/right.tga`, 17 px tall).
fn bar(commands: &mut Commands, assets: &AssetServer, parent: Entity, x: f32, y: f32, width: f32) {
    core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_left.tga",
        core::rect(x, y, 32., 17.),
    );
    let center = core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_center.tga",
        core::rect(x + 32., y, (width - 64.).max(0.), 17.),
    );
    commands.entity(center).insert(ImageNode {
        image_mode: bevy::ui::widget::NodeImageMode::Tiled { tile_x: true, tile_y: false, stretch_value: 1. },
        ..default()
    });
    core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_right.tga",
        core::rect(x + width - 32., y, 32., 17.),
    );
}

fn header(commands: &mut Commands, assets: &AssetServer, parent: Entity, y: f32, caption: &str) {
    bar(commands, assets, parent, 12., y, 223.);
    let label = core::text(commands, parent, caption, core::rect(15., y + 2., 200., 15.));
    commands.entity(label).insert(TextColor(Color::srgb(0.85, 0.78, 0.6)));
}

/// A 32-px grid cell with `Slot_Base.sub` and the emotion icon.
fn emotion_slot(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    entry: &Emotion,
    x: f32,
    y: f32,
) -> Entity {
    let slot = commands
        .spawn((
            EmotionButton(entry.id),
            Control,
            Surface,
            Hovered::default(),
            core::rect(x, y, 32., 32.),
            BackgroundColor(Color::NONE),
            Tooltip(entry.name.to_string()),
            ZIndex(1),
        ))
        .id();
    commands.entity(parent).add_child(slot);
    core::picture(
        commands,
        assets,
        slot,
        "d:/ymir work/ui/public/slot_base.sub",
        core::rect(0., 0., 32., 32.),
    );
    let icon = core::picture(commands, assets, slot, entry.icon, core::rect(0., 0., 32., 32.));
    commands.entity(icon).insert(ZIndex(1));
    slot
}

/// Send an emotion's command (`net.SendChatPacket(command)`); mutual emotions
/// (ids > 50) need a target in the original client, so only the plain command
/// is sent here.
fn send(net: &mut Net, entry: &Emotion) {
    if net.game.phase() == Phase::Game {
        net.game.send_chat(entry.command, 0);
    }
}

#[allow(clippy::too_many_arguments)]
fn actions(
    mut clicks: MessageReader<Click>,
    buttons: Query<&EmotionButton>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    chat: Res<ChatState>,
    manager: Res<WindowManager>,
    player: Res<PlayerState>,
    mut net: ResMut<Net>,
    mut quick_uses: MessageReader<QuickSlotUse>,
    mut consumed: ResMut<Consumed>,
) {
    let typing = focus.get().is_some() || chat.typing || manager.modal.is_some();
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    for click in clicks.read() {
        if typing || click.button != PointerButton::Primary {
            continue;
        }
        let Ok(button) = buttons.get(click.entity) else { continue };
        let Some(entry) = emotion(button.0) else { continue };
        if ctrl {
            // `__SelectEmotion`: add to the first empty local quick slot.
            if let Some(slot) = empty_quick_slot(&player) {
                quickslots::bind(&mut net, slot, quickslots::EMOTION, entry.id as u8);
            }
        } else {
            send(&mut net, entry);
        }
        consumed.pointer = true;
    }

    // A task bar emotion slot (`inventory::Slot::Quick` with `Type == 3`).
    for used in quick_uses.read() {
        let QuickSlotUse::Emotion(id) = *used else { continue };
        if typing {
            continue;
        }
        if let Some(entry) = emotion(id) {
            send(&mut net, entry);
        }
    }
}

/// Draw the emotion icon on each visible task bar quick slot. The overlay is a
/// separate child component so `inventory`'s item `Icon` keeps its behaviour.
fn quick_overlay(
    mut commands: Commands,
    assets: Res<AssetServer>,
    subs: Res<Assets<SubImage>>,
    player: Res<PlayerState>,
    ui: Res<InventoryUi>,
    slots: Query<(Entity, &Slot)>,
    mut overlays: Query<(Entity, &mut QuickEmotion, &mut Node, &mut ImageNode)>,
) {
    let existing: HashMap<Entity, Entity> = overlays
        .iter()
        .map(|(entity, marker, _, _)| (marker.slot, entity))
        .collect();

    for (slot_entity, slot) in &slots {
        let Slot::Quick(local) = *slot else { continue };
        let id = quick_slot_emotion(&player, ui.quick_page, local);
        let Some(overlay) = existing.get(&slot_entity).copied() else {
            let Some(id) = id else { continue };
            let Some(entry) = emotion(id) else { continue };
            let (art, image) = load_art(&assets, &subs, entry.icon);
            let helper = commands
                .spawn((
                    QuickEmotion { slot: slot_entity, id },
                    Node { display: Display::Flex, ..core::rect(0., 0., 32., 32.) },
                    image,
                    art,
                    ZIndex(3),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(slot_entity).add_child(helper);
            continue;
        };
        let Ok((entity, mut marker, mut node, mut image)) = overlays.get_mut(overlay) else { continue };
        match id {
            Some(id) if marker.id == id => node.display = Display::Flex,
            Some(id) => match emotion(id) {
                Some(entry) => {
                    marker.id = id;
                    let (art, loaded) = load_art(&assets, &subs, entry.icon);
                    *image = loaded;
                    node.display = Display::Flex;
                    commands.entity(entity).insert(art);
                }
                None => {
                    marker.id = 0;
                    node.display = Display::None;
                }
            },
            None => {
                marker.id = 0;
                node.display = Display::None;
            }
        }
    }
}

/// `Art::load`, resolved once up front so a changed slot does not wait a frame
/// for `core::resolve_art` (as `inventory::refresh` does for item icons).
fn load_art(assets: &AssetServer, subs: &Assets<SubImage>, path: &str) -> (Art, ImageNode) {
    let art = Art::load(assets, path);
    let image = match &art {
        Art::Image(handle) => ImageNode { image: handle.clone(), ..default() },
        Art::Sub(handle) => subs.get(handle).map_or_else(ImageNode::default, |sub| ImageNode {
            image: sub.image.clone(),
            rect: Some(sub.rect),
            ..default()
        }),
    };
    (art, image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_net::wire::TQuickSlot;

    #[test]
    fn table_matches_emotion_py() {
        let ids: Vec<u32> = EMOTIONS.iter().map(|entry| entry.id).collect();
        assert_eq!(
            ids,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 51, 52, 53]
        );
        assert_eq!(emotion(1), Some(&Emotion {
            id: 1,
            name: "Claps",
            command: "/clap",
            icon: "d:/ymir work/ui/game/windows/emotion_clap.sub",
        }));
        assert_eq!(emotion(13).map(|entry| entry.icon), Some("icon/action/dance1.tga"));
        assert_eq!(emotion(18).map(|entry| entry.command), Some("/dance6"));
        assert_eq!(emotion(51).map(|entry| entry.icon), Some("d:/ymir work/ui/game/windows/emotion_kiss.sub"));
        assert_eq!(emotion(52).map(|entry| entry.command), Some("/french_kiss"));
        assert_eq!(emotion(53).map(|entry| entry.icon), Some("d:/ymir work/ui/game/windows/emotion_slap.sub"));
        // Every command is `/name` with no spaces, and every icon is artwork.
        for entry in &EMOTIONS {
            assert!(entry.command.starts_with('/'));
            assert!(!entry.command.contains(' '));
            assert!(entry.icon.ends_with(".sub") || entry.icon.ends_with(".tga"));
        }
        assert!(emotion(19).is_none());
        assert!(emotion(50).is_none());
    }

    #[test]
    fn grid_places_solo_and_mutual_slots() {
        assert_eq!(grid_position(1), Some((30., 33.)));
        assert_eq!(grid_position(6), Some((190., 33.)));
        assert_eq!(grid_position(7), Some((30., 65.)));
        assert_eq!(grid_position(18), Some((190., 97.)));
        assert_eq!(grid_position(51), Some((30., 180.)));
        assert_eq!(grid_position(53), Some((94., 180.)));
        assert_eq!(grid_position(19), None);
    }

    #[test]
    fn quick_slots_resolve_emotions_by_page() {
        let mut player = PlayerState::default();
        player.quickslots[0] = TQuickSlot { Type: quickslots::EMOTION, Position: 1 };
        player.quickslots[8] = TQuickSlot { Type: quickslots::EMOTION, Position: 51 };
        player.quickslots[9] = TQuickSlot { Type: quickslots::SKILL, Position: 3 };

        assert_eq!(quick_slot_emotion(&player, 0, 0), Some(1));
        assert_eq!(quick_slot_emotion(&player, 0, 1), None);
        assert_eq!(quick_slot_emotion(&player, 1, 0), Some(51));
        assert_eq!(quick_slot_emotion(&player, 1, 1), None);
        assert_eq!(quick_slot_emotion(&player, 1, 8), None);
        assert_eq!(quick_slot_emotion(&player, 3, 7), None);
    }

    #[test]
    fn the_first_empty_quick_slot_is_used_for_ctrl_click() {
        let mut player = PlayerState::default();
        assert_eq!(empty_quick_slot(&player), Some(0));
        player.quickslots[0] = TQuickSlot { Type: quickslots::EMOTION, Position: 1 };
        assert_eq!(empty_quick_slot(&player), Some(1));
    }
}
