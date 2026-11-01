//! Character select (egui stopgap, D18) with the account's characters drawn in
//! 3D, and entering the game. The Rust counterpart of `introselect.py` and the
//! network side of `introloading.py`.
//!
//! The characters stand in a row facing the camera, each dressed in its armor and
//! hair and looping its intro wait motion; the selected one steps forward. "Start"
//! plays the intro *selected* motion on it and *not selected* on the others, and
//! three seconds later (`introselect.py`'s `startReservingTime`) asks the network
//! to enter the game ([`EnterGame`]). When the server switches to the Game phase
//! the screen hands the character and its global position to `m2-game`'s in-game
//! scene ([`EnterWorld`]: the map from `AtlasInfo.txt`, the character, the
//! third-person camera).

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use m2_assets::M2Packs;
use m2_game::SceneCamera;
use m2_game::ingame::{Actors, EnterWorld, GameWorld, LOCALE_PATH, PlayerControl, UiFocus};
use m2_game::net::{EnterGame, Net, NetMessage};
use m2_game::render::character::{ArmorShapes, Character, IntroMotion, Look};
use m2_net::phase::{NetEvent, Phase};

use crate::login::{Autologin, CharacterSlot, LoginScreen, RunDone, Status, empire_name, race_name};

/// `introselect.py`: the start is sent 3 seconds after the button.
const START_DELAY: f32 = 3.0;
/// How long an unattended run waits for the in-game scene before giving up.
const SCENE_WAIT: f32 = 180.0;
/// World units between two characters in the row.
const SPACING: f32 = 130.0;
/// How far the selected character stands in front of the others.
const STEP_FORWARD: f32 = 60.0;

/// The select screen's scene and choice.
#[derive(Resource, Default)]
pub struct SelectScreen {
    /// Index into the account's characters.
    pub selected: usize,
    root: Option<Entity>,
    characters: Vec<(CharacterSlot, Character)>,
    /// When "Start" was pressed (app seconds) and whether [`EnterGame`] was sent.
    start: Option<(f32, bool)>,
    /// `MainCharacter` while entering: name, position and VID for the in-game screen.
    main: Option<(String, i32, i32, u32)>,
}

impl SelectScreen {
    /// Every character's model is spawned (or failed).
    fn ready(&self) -> bool {
        self.root.is_some() && self.characters.iter().all(|(_, c)| c.is_ready() || c.error.is_some())
    }
}

/// The armor→shape table loads off the main thread at startup.
#[derive(Resource)]
struct ArmorShapesTask(Task<ArmorShapes>);

pub struct SelectScreenPlugin;

impl Plugin for SelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectScreen>()
            .add_systems(Startup, load_armor_shapes)
            .add_systems(Update, (poll_armor_shapes, track_entering, finish_when_loaded, drive_scene).chain())
            .add_systems(EguiPrimaryContextPass, select_ui);
    }
}

fn load_armor_shapes(mut commands: Commands, packs: Option<Res<M2Packs>>) {
    let Some(packs) = packs else {
        // No client data: characters use their armor value as the shape.
        commands.insert_resource(ArmorShapes::default());
        return;
    };
    let packs = packs.packs.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        ArmorShapes::load(&packs, LOCALE_PATH).unwrap_or_else(|error| {
            warn!("armor shapes: {error}");
            ArmorShapes::default()
        })
    });
    commands.insert_resource(ArmorShapesTask(task));
}

fn poll_armor_shapes(mut commands: Commands, task: Option<ResMut<ArmorShapesTask>>) {
    let Some(mut task) = task else { return };
    if let Some(shapes) = check_ready(&mut task.0) {
        info!("armor shapes: {} items", shapes.0.len());
        commands.insert_resource(shapes);
        commands.remove_resource::<ArmorShapesTask>();
    }
}

/// Follows the server from "Start" to the Game phase.
fn track_entering(
    mut messages: MessageReader<NetMessage>,
    mut screen: ResMut<LoginScreen>,
    mut select: ResMut<SelectScreen>,
    mut net: ResMut<Net>,
    auto: Res<Autologin>,
    mut enter_world: MessageWriter<EnterWorld>,
) {
    for message in messages.read() {
        let Status::Entering { slot, step } = &mut screen.status else { continue };
        match message {
            NetMessage::Game(NetEvent::PhaseChanged { to: Phase::HandShake, .. }) => {
                *step = "Connecting to the game server…".into()
            }
            NetMessage::Game(NetEvent::PhaseChanged { to: Phase::Select, .. }) => {
                *step = "Selecting the character…".into()
            }
            NetMessage::Game(NetEvent::PhaseChanged { to: Phase::Loading, .. }) => *step = "Loading…".into(),
            NetMessage::Game(NetEvent::MainCharacter(main)) => {
                info!("main character {} (vid {}) at {},{}", main.name, main.vid, main.x, main.y);
                select.main = Some((main.name.clone(), main.x, main.y, main.vid));
                *step = "Entering the game…".into();
                // The game window's `net.SendEnterGamePacket()`; see the module docs.
                net.start_game();
            }
            NetMessage::Game(NetEvent::PhaseChanged { to: Phase::Game, .. }) => {
                let slot = *slot;
                let (name, x, y, vid) = select.main.clone().unwrap_or_else(|| (format!("slot {slot}"), 0, 0, 0));
                let look = select.characters.iter().find(|(c, _)| c.slot == slot).map(|(_, c)| c.look);
                info!("in game as {name}");
                // `introloading.py`: `net.Warp(playerX, playerY)` and the game window.
                match look {
                    Some(look) => {
                        enter_world.write(EnterWorld { x, y, look, vid, demo: auto.demo });
                    }
                    None => warn!("no look for slot {slot}; the in-game character stays unspawned"),
                }
                screen.status = Status::InGame { name, x, y };
            }
            _ => {}
        }
    }
}

/// An unattended run ends when the in-game scene is loaded (or failed), so the
/// screenshot shows the character in the map.
fn finish_when_loaded(
    mut done: ResMut<RunDone>,
    auto: Res<Autologin>,
    screen: Res<LoginScreen>,
    game: Res<GameWorld>,
    time: Res<Time>,
    mut waited: Local<f32>,
) {
    if auto.enter.is_none() || !matches!(screen.status, Status::InGame { .. }) || done.0 {
        return;
    }
    *waited += time.delta_secs();
    if game.is_loaded(time.elapsed_secs()) || game.error().is_some() {
        done.0 = true;
    } else if *waited >= SCENE_WAIT {
        warn!("the in-game scene did not load in {SCENE_WAIT} s; taking the screenshot anyway");
        done.0 = true;
    }
}

/// Builds the 3D select scene while the select screen (or the start) is shown,
/// and tears it down afterwards.
fn drive_scene(world: &mut World) {
    let status = world.resource::<LoginScreen>().status.clone();
    world.resource_scope(|world, mut select: Mut<SelectScreen>| {
        let shown = matches!(status, Status::Select { .. } | Status::Entering { .. });
        if !shown {
            if let Some(root) = select.root.take() {
                world.despawn(root);
            }
            select.characters.clear();
            select.start = None;
            return;
        }
        if select.root.is_none() {
            let (Status::Select { characters, .. }, Some(shapes)) =
                (&status, world.get_resource::<ArmorShapes>().cloned())
            else {
                return;
            };
            build_scene(world, &mut select, characters, &shapes);
        }

        let selected = select.selected;
        let count = select.characters.len();
        for (index, (_, character)) in select.characters.iter_mut().enumerate() {
            character.update(world);
            if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                transform.translation = slot_position(index, count, index == selected);
            }
        }

        // "Start": the motions now, the network after the delay.
        let now = world.resource::<Time>().elapsed_secs();
        if let (Status::Entering { slot, .. }, None) = (&status, select.start) {
            select.start = Some((now, false));
            for (index, (_, character)) in select.characters.iter_mut().enumerate() {
                let motion = if index == selected { IntroMotion::Selected } else { IntroMotion::NotSelected };
                if let Some(file) = motion.file(character.look.race) {
                    character.play(world, &file, false);
                }
            }
            info!("start with slot {slot}");
        }
        if let (Status::Entering { slot, .. }, Some((pressed, false))) = (&status, select.start)
            && now - pressed >= START_DELAY
        {
            select.start = Some((pressed, true));
            world.write_message(EnterGame { slot: *slot });
        }

        // Unattended runs.
        if matches!(status, Status::Select { .. }) && select.ready() {
            let enter = world.resource::<Autologin>().enter;
            match enter {
                Some(slot) => {
                    if let Some(index) = select.characters.iter().position(|(c, _)| c.slot == slot) {
                        select.selected = index;
                        world.resource_mut::<LoginScreen>().status =
                            Status::Entering { slot, step: "Starting…".into() };
                    }
                }
                None => world.resource_mut::<RunDone>().0 = true,
            }
        }
    });
}

fn build_scene(world: &mut World, select: &mut SelectScreen, characters: &[CharacterSlot], shapes: &ArmorShapes) {
    let root = world.spawn((Name::new("select scene"), Transform::IDENTITY, Visibility::Visible)).id();
    select.root = Some(root);
    select.selected = select.selected.min(characters.len().saturating_sub(1));
    select.main = None;
    let count = characters.len();
    for (index, slot) in characters.iter().enumerate() {
        let look = Look { race: slot.race, shape: shapes.shape(u32::from(slot.armor)), hair: u32::from(slot.hair) };
        let transform = Transform::from_translation(slot_position(index, count, index == select.selected));
        let mut character = Character::spawn(world, look, transform, Some(root));
        if let Some(file) = IntroMotion::Wait.file(slot.race) {
            character.play(world, &file, true);
        }
        info!("select: {} is race {} shape {} hair {}", slot.name, look.race, look.shape, look.hair);
        select.characters.push((slot.clone(), character));
    }
    // The models face -Z: look at them from that side, from about chest height.
    let mut camera = world.query_filtered::<&mut Transform, With<SceneCamera>>();
    if let Ok(mut transform) = camera.single_mut(world) {
        *transform = Transform::from_xyz(0.0, 150.0, -520.0).looking_at(Vec3::new(0.0, 90.0, 0.0), Vec3::Y);
    }
}

/// Slot `index` of `count` in the row, left to right as seen from the camera
/// (which looks along +Z, so +X is on its left).
fn slot_position(index: usize, count: usize, selected: bool) -> Vec3 {
    let x = -(index as f32 - (count as f32 - 1.0) / 2.0) * SPACING;
    Vec3::new(x, 0.0, if selected { -STEP_FORWARD } else { 0.0 })
}

#[allow(clippy::too_many_arguments)]
fn select_ui(
    mut contexts: EguiContexts,
    mut screen: ResMut<LoginScreen>,
    mut select: ResMut<SelectScreen>,
    mut net: ResMut<Net>,
    mut focus: ResMut<UiFocus>,
    game: Res<GameWorld>,
    control: Res<PlayerControl>,
    actors: Res<Actors>,
    time: Res<Time>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    // The game must not move or attack while an egui window has the input.
    focus.pointer = ctx.egui_wants_pointer_input();
    focus.keyboard = ctx.egui_wants_keyboard_input();
    if !matches!(screen.status, Status::Select { .. } | Status::Entering { .. } | Status::InGame { .. }) {
        return Ok(());
    }
    let id = screen.id.clone();
    let mut log_out = false;
    let mut start = None;
    match &screen.status {
        Status::Select { empire, characters } => {
            egui::Window::new("Select a character")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, egui::Vec2::new(16.0, 16.0))
                .show(ctx, |ui| {
                    ui.label(format!("{id} · empire: {}", empire_name(*empire)));
                    ui.separator();
                    if characters.is_empty() {
                        ui.label("This account has no characters yet.");
                    }
                    for (index, c) in characters.iter().enumerate() {
                        let text = format!("{}  ·  {}  ·  Lv. {}", c.name, race_name(c.race), c.level);
                        if ui.selectable_label(select.selected == index, text).clicked() {
                            select.selected = index;
                        }
                    }
                    if let Some(c) = characters.get(select.selected) {
                        ui.separator();
                        ui.label(format!("Play time: {}h {:02}m", c.play_minutes / 60, c.play_minutes % 60));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        let chosen = characters.get(select.selected);
                        if ui.add_enabled(chosen.is_some(), egui::Button::new("Start")).clicked() {
                            start = chosen.map(|c| c.slot);
                        }
                        if ui.button("Log out").clicked() {
                            log_out = true;
                        }
                    });
                });
        }
        Status::Entering { step, .. } => {
            egui::Window::new("Entering the game")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0, -32.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(step);
                    });
                });
        }
        Status::InGame { name, .. } => {
            egui::Window::new("In game")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, egui::Vec2::new(16.0, 16.0))
                .show(ctx, |ui| {
                    ui.heading(name);
                    if let (Some(map), Some((lx, ly))) = (&game.map, game.local) {
                        let (gx, gy) = (lx + game.base.0 as i32, ly + game.base.1 as i32);
                        ui.label(format!("{map} at {lx}, {ly} (global {gx}, {gy})"));
                    } else {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading the map…");
                        });
                    }
                    if let Some(vid) = control.target
                        && let Some(target) = actors.by_vid.get(&vid)
                    {
                        ui.label(format!("Target: {} (VID {vid})", target.name));
                    }
                    ui.separator();
                    ui.label("Right-drag: camera · Wheel: zoom");
                    ui.label("Left-click / WASD: move · Space: attack");
                    if let Some(error) = game.error() {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("The map could not be loaded: {error}"));
                    } else if game.is_ready() && !game.is_loaded(time.elapsed_secs()) {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading objects…");
                        });
                    }
                    if ui.button("Log out").clicked() {
                        log_out = true;
                    }
                });
        }
        _ => {}
    }
    if let Some(slot) = start {
        screen.status = Status::Entering { slot, step: "Starting…".into() };
    }
    if log_out {
        net.disconnect();
        screen.status = Status::Idle;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_is_centred_and_the_selection_steps_forward() {
        assert_eq!(slot_position(0, 1, true), Vec3::new(0.0, 0.0, -STEP_FORWARD));
        // Two characters: the first one on the camera's left (+X).
        assert_eq!(slot_position(0, 2, false).x, SPACING / 2.0);
        assert_eq!(slot_position(1, 2, false).x, -SPACING / 2.0);
    }
}
