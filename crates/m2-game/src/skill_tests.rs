//! Regression coverage using the actual PC skill tables and motion scripts.
use super::*;
use m2_assets::race::{RaceMotionAsset, RaceMotionEventKind};
use m2_formats::{
    pack::{PackManager, load_index},
    proto::SkillTables,
};
use m2_net::{
    transport::Machine,
    wire::{self, Wire},
};

fn packed_skills() -> Option<(PackManager, SkillTables)> {
    let dir = std::env::var_os("M2_CLIENT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
    if !dir.join("pack/Index.dev").exists() {
        return None;
    }
    let mut packs = PackManager::new(&dir);
    load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
    let mut skills = SkillTables::default();
    skills
        .register_skill_desc(&packs.get("locale/en/skilldesc.txt").unwrap())
        .unwrap();
    skills.register_skill_table(&packs.get("locale/en/skilltable.txt").unwrap(), false);
    Some((packs, skills))
}

fn motion(packs: &PackManager, path: &str) -> RaceMotionAsset {
    let bytes = packs
        .get(paths::virtual_name(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    let parsed = m2_formats::race::RaceMotionData::from_script(&m2_formats::script::parse(&bytes))
        .unwrap_or_else(|| panic!("invalid motion: {path}"));
    m2_assets::race::motion_from_data(&parsed)
}

fn online_net() -> Net {
    let mut net = Net::default();
    let packet = wire::TPacketGCPhase {
        header: wire::gc::PHASE,
        phase: m2_net::phase::phase_id::GAME,
    }
    .encode();
    net.game.stream().feed(&packet);
    net.game.poll(0);
    net.game.stream().take_output(4096);
    net
}

#[test]
fn every_available_pc_skill_has_valid_motions_and_delivery_events_at_every_grade() {
    let Some((packs, skills)) = packed_skills() else {
        return;
    };
    let mut checked = 0;
    let points = combat::Points {
        hp: 10000,
        sp: 10000,
        casting_speed: 100,
        ..default()
    };
    for race in 0..8 {
        for group in 1..=2 {
            for &vnum in combat::active_table(race, group).iter().take(6) {
                // The English locale omits the YMIR-only sixth skills.
                let Some(skill) = skills.skills.get(&vnum) else {
                    continue;
                };
                for grade in 0..=3 {
                    let index = combat::skill_motion_index(skill, grade);
                    assert!(
                        index < 128,
                        "skill {vnum} grade {grade} exceeds FUNC_SKILL's motion field"
                    );
                    let path =
                        crate::render::character::skill_motion_file(race as u8, index).unwrap();
                    let data = motion(&packs, &path);
                    assert!(data.duration > 0.0, "{path}: empty motion");
                    assert!(
                        packs.get(paths::virtual_name(&data.motion_file)).is_ok(),
                        "{path}: animation {} missing",
                        data.motion_file
                    );
                    let power = combat::skill_power(1, grade);
                    assert!(combat::skill_cooldown(skill, power, &points, 100).is_finite());
                    assert!(
                        combat::skill_target_count(skill, power, &points) <= 16,
                        "skill {vnum}: target count"
                    );
                    assert!(
                        combat::skill_motion_loop_count(skill, power, &points) <= 15,
                        "skill {vnum}: loop count"
                    );
                    for event in &data.events {
                        assert!(event.start <= data.duration, "{path}: event beyond motion");
                        match &event.kind {
                            RaceMotionEventKind::Fly { file, .. } => {
                                assert!(
                                    m2_formats::effect::load_flying_data(
                                        &packs,
                                        paths::virtual_name(file)
                                    )
                                    .1,
                                    "{path}: missing projectile {file}"
                                );
                            }
                            RaceMotionEventKind::Effect { file, .. }
                            | RaceMotionEventKind::EffectToTarget { file, .. } => {
                                crate::render::effect::load_instance(&packs, file.as_bytes(), Mat4::IDENTITY.to_cols_array())
                                    .unwrap_or_else(|error| panic!("{path}: {error}"));
                            }
                            RaceMotionEventKind::SpecialAttacking(splash) => {
                                assert!(!splash.spheres.is_empty(), "{path}: empty splash")
                            }
                            _ => {}
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    eprintln!("Audited {checked} PC skill/race/grade combinations");
    assert!(checked >= 320);
}

#[test]
fn deadly_cloud_shoots_its_skill_id_on_the_fly_event_without_attack_packets() {
    let Some((packs, skills)) = packed_skills() else {
        return;
    };
    let mut world = World::new();
    world.insert_resource(online_net());
    for grade in 0..=3 {
        let index = combat::skill_motion_index(&skills.skills[&35], grade);
        let path = crate::render::character::skill_motion_file(1, index).unwrap();
        let data = motion(&packs, &path);
        let tick = MotionTick {
            events: data.events,
            ..default()
        };
        dispatch_motion_shots(&mut world, &tick, 35, false);
        assert!(
            world
                .resource_mut::<Net>()
                .game
                .stream()
                .take_output(4096)
                .is_empty()
        );
        dispatch_motion_shots(&mut world, &tick, 35, true);
        let output = world.resource_mut::<Net>().game.stream().take_output(4096);
        let expected = wire::TPacketCGShoot {
            bHeader: wire::cg::SHOOT,
            bType: 35,
        }
        .encode();
        assert!(output.starts_with(&expected), "skill shot: {output:?}");
        assert!(output.len() <= expected.len() + 1);
    }
}

#[test]
fn skill_power_uses_mastery_and_floor_formulas_for_arrow_counts() {
    assert_eq!(combat::skill_power(1, 0), 0.05);
    assert_eq!(combat::skill_power(1, 1), 0.5);
    assert_eq!(combat::skill_power(1, 2), 0.82);
    assert_eq!(combat::skill_power(1, 3), 1.25);
    let Some((_, skills)) = packed_skills() else {
        return;
    };
    let points = combat::Points::default();
    assert_eq!(
        combat::skill_motion_loop_count(&skills.skills[&46], 0.05, &points),
        2
    );
    assert_eq!(
        combat::skill_motion_loop_count(&skills.skills[&46], 1.25, &points),
        9
    );
    assert_eq!(
        combat::skill_target_count(&skills.skills[&47], 1.25, &points),
        9
    );
}

#[test]
fn great_hits_interrupt_attacks_and_block_queued_movement_until_standing() {
    let mut world = World::new();
    world.init_resource::<Actors>();
    world.init_resource::<CombatFx>();
    let victim = ActorCombat {
        vid: 7,
        kind: 0,
        ..default()
    };
    world
        .resource_mut::<Actors>()
        .by_vid
        .insert(7, super::tests::remote_pc(100));
    world.resource_mut::<Actors>().fights.insert(
        7,
        ActorFight {
            attacking: true,
            ..default()
        },
    );
    let attack = m2_assets::race::MotionAttack {
        hitting_type: battle::HIT_TYPE_GREAT,
        ..default()
    };
    attack_feedback(
        &mut world,
        &attack,
        Vec3::ZERO,
        0.0,
        &victim,
        Vec3::X * 100.0,
        1.0,
    );
    assert!(matches!(
        world.resource::<Actors>().fights[&7].pending,
        Some(ActorAction::Knockdown { .. })
    ));
    let command = super::tests::command(FUNC_MOVE, 500, 0);
    world
        .resource_mut::<Actors>()
        .by_vid
        .get_mut(&7)
        .unwrap()
        .remote
        .commands
        .push_back((0, command));
    advance_actor_states(&mut world.resource_mut::<Actors>(), 0.1, Some(10));
    assert_eq!(
        world.resource::<Actors>().by_vid[&7].remote.commands.len(),
        1
    );
    assert!(matches!(
        world.resource::<Actors>().fights[&7].pending,
        Some(ActorAction::Knockdown { .. })
    ));
}

#[test]
fn shaman_buff_targets_self_and_cancels_the_preceding_click_action() {
    use bevy::asset::{AssetApp, AssetPlugin};
    let Some((_, skills)) = packed_skills() else {
        return;
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<RaceMotionAsset>();
    let world = app.world_mut();
    world.insert_resource(online_net());
    world.init_resource::<Actors>();
    world.init_resource::<FlyTargets>();
    world.init_resource::<PlayerState>();
    world.insert_resource(combat::SkillCatalog(skills));
    let mut levels = combat::SkillState::default();
    levels.levels[94] = 1;
    world.insert_resource(levels);
    let mut character = Character::spawn_actor(world, 3, "", Transform::IDENTITY, None);
    character.look.race = 3;
    let mut control = PlayerControl {
        main_vid: 1,
        reserved_attack: Some(7),
        reserved_talk: Some(8),
        reserved_item: Some(9),
        smart_held: true,
        destination: Some(Vec2::splat(500.0)),
        ..default()
    };
    let me = ActorCombat {
        vid: 1,
        kind: 6,
        ..default()
    };
    let points = combat::Points {
        hp: 10000,
        sp: 10000,
        casting_speed: 100,
        ..default()
    };
    use_skill(
        world,
        &mut control,
        &mut character,
        94,
        &me,
        &default(),
        &points,
        0,
        (0, 0),
        0,
        &[],
        &|_| false,
        Vec2::ZERO,
        1.0,
        (0, 0),
    );
    assert_eq!(control.state, MoveState::Casting);
    assert_eq!(
        character.motion_file(),
        Some("d:/ymir work/pc/shaman/skill/hosin_target.msa")
    );
    assert!(
        control.reserved_attack.is_none()
            && control.reserved_talk.is_none()
            && control.reserved_item.is_none()
    );
    assert!(control.destination.is_none() && !control.smart_held);
    assert_eq!(
        world.resource::<FlyTargets>().peek(1),
        Some(ShotTarget::Actor(1))
    );
    let expected = wire::TPacketCGUseSkill {
        bHeader: wire::cg::USE_SKILL,
        dwVnum: 94,
        dwTargetVID: 1,
    }
    .encode();
    let output = world.resource_mut::<Net>().game.stream().take_output(4096);
    assert!(
        output
            .windows(expected.len())
            .any(|bytes| bytes == expected)
    );
}

#[test]
fn quest_targets_follow_actor_positions_and_fall_back_to_global_updates() {
    let mut actors = Actors::default();
    let mut target = crate::social::QuestTarget {
        name: "Captain".into(),
        vid: 7,
        position: Some((10050, 20100)),
    };
    assert_eq!(
        quest_target_position(&target, &actors, (10000, 20000)),
        Some(Vec2::new(50.0, 100.0))
    );
    let mut actor = super::tests::remote_pc(10100);
    actor.y = 20200;
    actors.by_vid.insert(7, actor);
    assert_eq!(
        quest_target_position(&target, &actors, (10000, 20000)),
        Some(Vec2::new(100.0, 200.0))
    );
    target.vid = 0;
    assert_eq!(
        quest_target_position(&target, &actors, (10000, 20000)),
        Some(Vec2::new(50.0, 100.0))
    );
}

#[test]
fn camera_drag_locks_relative_input_without_warping_the_wayland_pointer() {
    use bevy::window::{CursorGrabMode, CursorOptions};
    let mut world = World::new();
    let mut window = Window::default();
    window.set_cursor_position(Some(Vec2::new(100.0, 200.0)));
    let entity = world
        .spawn((PrimaryWindow, window, CursorOptions::default()))
        .id();
    set_cursor_lock(&mut world, Some(Vec2::new(10.0, 20.0)));
    assert_eq!(
        world.get::<Window>(entity).unwrap().cursor_position(),
        Some(Vec2::new(100.0, 200.0))
    );
    assert_eq!(
        world.get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    assert!(!world.get::<CursorOptions>(entity).unwrap().visible);
    set_cursor_lock(&mut world, None);
    assert_eq!(
        world.get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::None
    );
    assert!(world.get::<CursorOptions>(entity).unwrap().visible);
}

#[test]
fn mounted_and_guild_skill_motion_paths_match_the_python_registrations() {
    use crate::render::character::{MotionMode as Mode, skill_motion_file_in};
    let Some((packs, _)) = packed_skills() else {
        return;
    };
    for race in 0..8 {
        for index in 101..=106 {
            let path = skill_motion_file_in(race, Mode::General, index).unwrap();
            let asset = motion(&packs, &path);
            assert!(
                packs.get(paths::virtual_name(&asset.motion_file)).is_ok(),
                "{path}"
            );
        }
        let modes: &[Mode] = match race % 4 {
            0 => &[Mode::HorseOneHandSword, Mode::HorseTwoHandSword],
            1 => &[
                Mode::HorseOneHandSword,
                Mode::HorseDualHandSword,
                Mode::HorseBow,
            ],
            2 => &[Mode::HorseOneHandSword],
            _ => &[Mode::HorseFan, Mode::HorseBell],
        };
        for &mode in modes {
            for index in 121..=123 {
                let path = skill_motion_file_in(race, mode, index).unwrap();
                let asset = motion(&packs, &path);
                assert!(
                    packs.get(paths::virtual_name(&asset.motion_file)).is_ok(),
                    "{path}"
                );
            }
        }
    }
}

#[test]
fn dash_starts_without_a_target_then_finishes_on_the_reserved_target() {
    use bevy::asset::{AssetApp, AssetPlugin};
    let Some((_, skills)) = packed_skills() else {
        return;
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<RaceMotionAsset>()
        .init_asset::<m2_assets::gr2::Gr2Model>();
    let world = app.world_mut();
    world.insert_resource(online_net());
    world.init_resource::<Actors>();
    world.init_resource::<FlyTargets>();
    world.init_resource::<PlayerState>();
    world.insert_resource(combat::SkillCatalog(skills));
    let mut levels = combat::SkillState::default();
    levels.levels[5] = 1;
    world.insert_resource(levels);
    let mut character = Character::spawn_actor(world, 0, "", Transform::IDENTITY, None);
    character.set_weapon(world, 1, "sword.gr2", "", combat::weapon::SWORD, 0.0);
    let mut control = PlayerControl {
        main_vid: 1,
        target: Some(7),
        ..default()
    };
    let mut me = ActorCombat {
        vid: 1,
        kind: 6,
        ..default()
    };
    let mut victim = ActorCombat {
        vid: 7,
        kind: 0,
        x: 1000,
        ..default()
    };
    let points = combat::Points {
        hp: 10000,
        sp: 10000,
        casting_speed: 100,
        ..default()
    };
    use_skill(
        world,
        &mut control,
        &mut character,
        5,
        &me,
        &default(),
        &points,
        0,
        (0, 0),
        0,
        &[(7, victim)],
        &|_| true,
        Vec2::ZERO,
        1.0,
        (0, 0),
    );
    assert_eq!(control.reserved_skill, Some((5, 7, 170)));
    assert_ne!(control.state, MoveState::Casting);
    let output = world.resource_mut::<Net>().game.stream().take_output(4096);
    let start = wire::TPacketCGUseSkill {
        bHeader: wire::cg::USE_SKILL,
        dwVnum: 5,
        dwTargetVID: 0,
    }
    .encode();
    assert!(output.windows(start.len()).any(|bytes| bytes == start));
    assert!(
        world
            .resource::<combat::SkillState>()
            .cool_time_left(5, 1.1)
            > 0.0
    );
    me.affect_flags = 1 << combat::AFFECT_DASH;
    victim.x = 100;
    use_skill(
        world,
        &mut control,
        &mut character,
        5,
        &me,
        &default(),
        &combat::Points { sp: 0, ..points },
        0,
        (0, 0),
        0,
        &[(7, victim)],
        &|_| true,
        Vec2::ZERO,
        1.1,
        (0, 0),
    );
    assert_eq!(control.state, MoveState::Casting);
    assert!(control.reserved_skill.is_none());
    let output = world.resource_mut::<Net>().game.stream().take_output(4096);
    let strike = wire::TPacketCGUseSkill {
        bHeader: wire::cg::USE_SKILL,
        dwVnum: 5,
        dwTargetVID: 7,
    }
    .encode();
    assert!(output.windows(strike.len()).any(|bytes| bytes == strike));
}

#[test]
fn local_actor_speed_updates_apply_the_server_dash_speed() {
    let mut player = PlayerState {
        vid: 1,
        ..default()
    };
    player.apply(&NetEvent::ActorUpdate(m2_net::phase::ActorUpdate {
        vid: 1,
        move_speed: 230,
        attack_speed: 150,
        state_flags: 0,
        affect_flags: 1 << combat::AFFECT_DASH,
        alignment: 0,
        pk_mode: 0,
        guild_id: 0,
        armor: 0,
        weapon: 0,
        hair: 0,
        mount_vnum: 0,
    }));
    assert_eq!(player.points[combat::point::MOV_SPEED], 230);
    assert_eq!(player.points[combat::point::ATT_SPEED], 150);
    assert!(
        remote_run_speed(player.points[combat::point::MOV_SPEED] as u8)
            > remote_run_speed(100) * 2.0
    );
}
