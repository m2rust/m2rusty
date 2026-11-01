//! Differential tests: the Rust race, motion, sound and property loaders
//! against the C++ ones (`tools/differential/race`). Both sides load every
//! registered `.msm` / `.msa` (with its `.mss`) / `property/` file and print
//! what the C++ objects hold afterwards.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::property::{Property, PropertyManager};
use m2_formats::race::{
    AttachingPayload, CollisionData, MotionEvent, MotionEventData, RaceData, RaceMotionData, Skin,
    sound_script_file_name,
};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

fn extension(name: &[u8]) -> Vec<u8> {
    match name.iter().rposition(|&b| b == b'.') {
        Some(dot) => name[dot + 1..].to_ascii_lowercase(),
        None => Vec::new(),
    }
}

/// Whether C++ `CMemoryTextFileLoader::Bind` reads past the end of `data`
/// (a lead byte >= 0x80 as the last byte); such files are skipped.
fn bind_overreads(data: &[u8]) -> bool {
    let mut pos = 0;
    while pos < data.len() {
        pos += 1;
        if data[pos - 1] >= 0x80 {
            if pos == data.len() {
                return true;
            }
            pos += 1;
        }
    }
    false
}

fn file_overreads(manager: &PackManager, name: &[u8]) -> bool {
    manager.get(name).is_ok_and(|d| bind_overreads(&d))
}

/// The dump being built, with the C++ dumper's field helpers.
#[derive(Default)]
struct Out(Vec<u8>);

impl Out {
    fn line(&mut self, kind: &str) -> &mut Self {
        self.0.extend_from_slice(kind.as_bytes());
        self
    }
    fn s(&mut self, s: &[u8]) -> &mut Self {
        self.0.push(b'\t');
        self.0.extend_from_slice(s);
        self
    }
    fn f(&mut self, f: f32) -> &mut Self {
        write!(self.0, "\t{:08x}", f.to_bits()).unwrap();
        self
    }
    fn v(&mut self, v: [f32; 3]) -> &mut Self {
        self.f(v[0]).f(v[1]).f(v[2])
    }
    fn i(&mut self, i: impl Into<i64>) -> &mut Self {
        write!(self.0, "\t{}", i.into()).unwrap();
        self
    }
    fn x(&mut self, x: u32) -> &mut Self {
        write!(self.0, "\t{x:08x}").unwrap();
        self
    }
    fn end(&mut self) {
        self.0.push(b'\n');
    }
    fn file(&mut self, name: &[u8], status: &str) {
        self.line("F").s(name).s(status.as_bytes()).end();
    }
}

fn manager(client: &Path, index: &str) -> PackManager {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", index).unwrap();
    manager
}

fn sorted_names(manager: &PackManager, keep: impl Fn(&[u8]) -> bool) -> Vec<&[u8]> {
    let mut names: Vec<&[u8]> = manager.file_names().filter(|n| keep(n)).collect();
    names.sort_unstable();
    names
}

fn dump_collision(out: &mut Out, c: &CollisionData) {
    out.line("COL").i(c.collision_type).i(c.spheres.len() as i64).end();
    for sphere in &c.spheres {
        out.line("SPH").f(sphere.radius).v(sphere.position).end();
    }
}

fn dump_skins(out: &mut Out, skins: &[Skin]) {
    for skin in skins {
        out.line("SKIN").i(skin.part).s(&skin.src_file_name).s(&skin.dst_file_name).end();
    }
}

fn dump_race(out: &mut Out, race: &RaceData) {
    out.line("S\tbase").s(&race.base_model_file_name).end();
    out.line("S\ttree").s(&race.tree_file_name).end();
    out.line("S\tattribute").s(&race.attribute_file_name).end();
    out.line("S\tsmokebone").s(&race.smoke_bone_name).end();
    out.line("S\tmotionlist").s(&race.motion_list_file_name).end();
    out.line("SMOKE");
    for &id in &race.smoke_effect_ids {
        out.x(id);
    }
    out.end();
    for (&index, shape) in &race.shapes {
        out.line("SHAPE").i(index).s(&shape.model_file_name).end();
        dump_skins(out, &shape.skins);
    }
    for (&index, hair) in &race.hairs {
        out.line("HAIR").i(index).s(&hair.model_file_name).end();
        dump_skins(out, &hair.skins);
    }
    for (i, a) in race.attaching_data.iter().enumerate() {
        out.line("A")
            .i(i as i64)
            .i(a.data_type)
            .i(a.is_attaching)
            .i(a.attaching_model_index)
            .s(&a.attaching_bone_name)
            .end();
        match &a.payload {
            AttachingPayload::None => {}
            AttachingPayload::Collision(c) => dump_collision(out, c),
            AttachingPayload::Effect(e) => out.line("AEFF").s(&e.file_name).v(e.position).v(e.rotation).end(),
            AttachingPayload::Object(o) => out.line("AOBJ").s(&o.file_name).end(),
        }
    }
}

fn dump_event(out: &mut Out, index: usize, e: &MotionEvent) {
    out.line("E")
        .i(index as i64)
        .i(e.event_type)
        .i(e.frame)
        .f(e.starting_time)
        .f(e.duration_time)
        .end();
    match &e.data {
        MotionEventData::Effect(p) => out
            .line("EEFF")
            .i(p.is_independent)
            .i(p.is_attaching)
            .i(p.is_following)
            .s(&p.attaching_bone_name)
            .s(&p.effect_file_name)
            .v(p.effect_position)
            .x(p.effect_index)
            .end(),
        MotionEventData::EffectToTarget(p) => out
            .line("ETGT")
            .s(&p.effect_file_name)
            .v(p.effect_position)
            .i(p.is_following)
            .i(p.is_fishing_effect)
            .x(p.effect_index)
            .end(),
        MotionEventData::Fly(p) => out
            .line("EFLY")
            .i(p.is_attaching)
            .s(&p.attaching_bone_name)
            .s(&p.fly_file_name)
            .v(p.fly_position)
            .x(p.fly_index)
            .end(),
        MotionEventData::ScreenWaving(p) => out.line("EWAV").i(p.power).i(p.affecting_range).end(),
        MotionEventData::SpecialAttacking(p) => {
            let a = &p.attack;
            out.line("EATK")
                .i(p.is_enable_hit_process)
                .i(a.attack_type)
                .i(a.hitting_type)
                .f(a.invisible_time)
                .f(a.external_force)
                .f(a.stiffen_time)
                .i(a.hit_limit_count)
                .end();
            dump_collision(out, &p.collision);
        }
        MotionEventData::Sound(p) => out.line("ESND").s(&p.sound_file_name).end(),
        MotionEventData::CharacterShow | MotionEventData::CharacterHide | MotionEventData::Warp => {}
    }
}

fn dump_motion(out: &mut Out, m: &RaceMotionData) {
    out.line("M").s(&m.motion_file_name).f(m.motion_duration).end();
    out.line("ACC").i(m.is_accumulation_motion).v(m.accumulation_position).end();
    let c = &m.combo_input;
    out.line("COMBO")
        .i(m.is_combo_motion)
        .f(c.input_start_time)
        .f(c.next_combo_time)
        .f(c.input_end_time)
        .end();
    out.line("LOOP")
        .i(m.is_loop_motion)
        .i(m.loop_count)
        .i(m.cancel_enable_skill)
        .f(m.loop_start_time)
        .f(m.loop_end_time)
        .end();
    out.line("SPLASH").i(m.has_splash_event).end();
    out.line("ATK").i(m.is_attacking_motion);
    let d = &m.motion_attack_data;
    if m.is_attacking_motion {
        let a = &d.attack;
        out.i(a.attack_type)
            .i(a.hitting_type)
            .f(a.invisible_time)
            .f(a.external_force)
            .f(a.stiffen_time)
            .i(a.hit_limit_count)
            .i(d.motion_type)
            .i(d.hit_data.len() as i64);
    }
    out.end();
    if m.is_attacking_motion {
        for hit in &d.hit_data {
            out.line("HIT")
                .f(hit.attack_start_time)
                .f(hit.attack_end_time)
                .f(hit.weapon_length)
                .s(&hit.bone_name)
                .i(hit.hit_positions.len() as i64)
                .end();
            for (time, pos) in &hit.hit_positions {
                out.line("HP").f(*time).v(pos.last_position).v(pos.position).end();
            }
        }
    }
    for (i, e) in m.events.iter().enumerate() {
        dump_event(out, i, e);
    }
    out.line("SND").s(&m.sound_script_file_name).i(m.sound_instances.len() as i64).end();
    for s in &m.sound_instances {
        out.line("SI").i(s.frame).s(&s.file_name).end();
    }
}

fn dump_property(out: &mut Out, p: &Property) {
    out.line("P")
        .s(&p.file_name)
        .s(p.short_name())
        .s(&p.crc_string)
        .x(p.crc)
        .i(p.tokens.len() as i64)
        .end();
    for (key, tokens) in &p.tokens {
        out.line("K").s(key).i(tokens.len() as i64).end();
        for token in tokens {
            out.line("T").s(token).end();
        }
    }
}

type Counts = BTreeMap<&'static str, usize>;

/// Same output as `m2diff_race <mode>`, with counts of what was compared.
/// `cpp` is the C++ dump, read only for the property map's walk order.
fn rust_dump(client: &Path, index: &str, mode: &str, cpp: &[u8]) -> (Vec<u8>, Counts) {
    let manager = manager(client, index);
    let mut out = Out::default();
    let mut counts = Counts::new();
    let mut count = |key: &'static str| *counts.entry(key).or_default() += 1;

    match mode {
        "race" => {
            for name in sorted_names(&manager, |n| extension(n) == b"msm") {
                if file_overreads(&manager, name) {
                    out.file(name, "skip");
                    count("skipped");
                    continue;
                }
                let mut race = RaceData::default();
                // `CEffectManager::RegisterEffect2`: whether the effect loads.
                let ok = race.load(&manager, name, |effect| manager.exists(effect));
                out.file(name, if ok { "1" } else { "0" });
                count(if ok { "msm ok" } else { "msm failed" });
                for _ in &race.shapes {
                    count("shapes");
                }
                for _ in &race.hairs {
                    count("hairs");
                }
                for _ in &race.attaching_data {
                    count("attaching");
                }
                dump_race(&mut out, &race);
            }
        }
        "motion" => {
            for name in sorted_names(&manager, |n| extension(n) == b"msa") {
                let sound = sound_script_file_name(name);
                if file_overreads(&manager, name) || sound.is_some_and(|s| file_overreads(&manager, &s)) {
                    out.file(name, "skip");
                    count("skipped");
                    continue;
                }
                match RaceMotionData::load(&manager, name) {
                    Some(motion) => {
                        out.file(name, "1");
                        count("msa ok");
                        for _ in &motion.events {
                            count("events");
                        }
                        if !motion.sound_instances.is_empty() {
                            count("with sounds");
                        }
                        dump_motion(&mut out, &motion);
                    }
                    None => {
                        out.file(name, "0");
                        count("msa failed");
                    }
                }
            }
        }
        "property" => {
            for name in sorted_names(&manager, |n| n.starts_with(b"property/")) {
                let data = manager.get(name).unwrap();
                if data.len() < 5 {
                    out.file(name, "short");
                    continue;
                }
                if data.starts_with(b"YPRT") && bind_overreads(&data) {
                    out.file(name, "skip");
                    count("skipped");
                    continue;
                }
                match Property::read_from_memory(name, &data) {
                    Some(p) => {
                        out.file(name, "1");
                        count("properties");
                        dump_property(&mut out, &p);
                    }
                    None => {
                        out.file(name, "0");
                        count("not properties");
                    }
                }
            }

            let order: Vec<&[u8]> = cpp.split(|&b| b == b'\n').filter_map(|l| l.strip_prefix(b"ORDER\t")).collect();
            for name in &order {
                out.line("ORDER").s(name).end();
            }
            let mut properties = PropertyManager::default();
            properties.initialize_in_order(&manager, order.iter().copied());
            for (&crc, p) in &properties.by_crc {
                out.line("CRC").x(crc).s(&p.file_name).end();
                count("by crc");
            }
            for &crc in &properties.reserved_crcs {
                out.line("RESERVED").x(crc).end();
                count("reserved");
            }

            // Name order differs from the C++ walk only in which duplicate
            // CRC wins.
            let mut sorted = PropertyManager::default();
            sorted.initialize(&manager);
            assert_eq!(sorted.by_crc.keys().collect::<Vec<_>>(), properties.by_crc.keys().collect::<Vec<_>>());
            assert_eq!(sorted.reserved_crcs, properties.reserved_crcs);
        }
        _ => unreachable!(),
    }
    (out.0, counts)
}

fn assert_same_dump(client: &Path, index: &str, mode: &str) -> Counts {
    let args = [client.as_os_str(), index.as_ref(), mode.as_ref()];
    let cpp = run(&cpp_tool("race"), &args);
    let (rust, counts) = rust_dump(client, index, mode, &cpp);
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!("{mode}: {lines} lines identical; {counts:?}");
    counts
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_race_data_matches_cpp() {
    let counts = assert_same_dump(&client_dir(), "pack/Index.dev", "race");
    assert!(counts.get("msm ok").is_some_and(|&n| n > 0), "no .msm files loaded");
    assert!(counts.get("shapes").is_some_and(|&n| n > 0), "no shapes loaded");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_motion_data_matches_cpp() {
    let counts = assert_same_dump(&client_dir(), "pack/Index.dev", "motion");
    assert!(counts.get("msa ok").is_some_and(|&n| n > 0), "no .msa files loaded");
    assert!(counts.get("with sounds").is_some_and(|&n| n > 0), "no sound scripts loaded");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_properties_match_cpp() {
    let counts = assert_same_dump(&client_dir(), "pack/Index.dev", "property");
    assert!(counts.get("properties").is_some_and(|&n| n > 0), "no properties found");
    assert!(counts.get("reserved").is_some_and(|&n| n > 0), "no reserved CRCs found");
}

/// Shapes, hair and attaching data in odd shapes, and the failure paths.
const SYNTHETIC_RACE: &[(&str, &[u8])] = &[
    (
        "shapes.msm",
        b"BaseModelFileName \"D:/a.gr2\"\nTreeFileName \"D:\\T\\Tree.SPT\"\nMotionListFileName \"m.txt\"\n\
          Group ShapeData\n{\nPathName \"d:/p/\"\nShapeDataCount 4\n\
          Group ShapeData00\n{\nShapeIndex 3\nModel \"m.gr2\"\nSourceSkin \"s/a.dds\"\nTargetSkin \"t.dds\"\n\
          SourceSkin2 \"s2.dds\"\nTargetSkin2 \"t2.dds\"\nLocal_SourceSkin \"ls/x\"\nLocal_TargetSkin \"lt\"\n}\n\
          Group ShapeData02\n{\nSpecialPath \"e/\"\nShapeIndex 4\nLocal_Model \"l.gr2\"\nSourceSkin \"q\"\n}\n\
          Group ShapeData03\n{\nShapeIndex 3\nModel \"again.gr2\"\nSourceSkin \"z\"\nTargetSkin \"y\"\n}\n}\n\
          Group HairData\n{\nPathName \"h/\"\nHairDataCount 2\n\
          Group HairData00\n{\nHairIndex 1\nModel \"m\"\nSourceSkin \"s/1\"\nTargetSkin \"t\"\n}\n\
          Group HairData01\n{\nHairIndex 1\nModel \"m2\"\nSourceSkin \"s/2\"\nTargetSkin \"t\"\n}\n}\n",
    ),
    (
        "bad_shape.msm",
        b"Group ShapeData\n{\nPathName \"\"\nShapeDataCount 2\nGroup ShapeData00\n{\nModel \"a\"\n}\n\
          Group ShapeData01\n{\nShapeIndex 1\nModel \"b\"\n}\n}\nGroup AttachingData\n{\nAttachingDataCount 0\n}\n",
    ),
    (
        "no_model.msm",
        b"Group HairData\n{\nPathName \"\"\nHairDataCount 2\nGroup HairData00\n{\nHairIndex 1\nModel \"a\"\n}\n\
          Group HairData01\n{\nHairIndex 2\nModel \"b\"\nSourceSkin c\nTargetSkin d\n}\n}\n\
          Group ShapeData\n{\nPathName \"x\"\n}\n",
    ),
    (
        "attaching.msm",
        b"Group AttachingData\n{\nAttachingDataCount 5\n\
          Group AttachingData00\n{\nAttachingDataType 1\nisAttaching 7\nAttachingModelIndex -1\nAttachingBoneName \"b\"\n\
          CollisionType 2\nSphereDataCount 2\nGroup SphereData00\n{\nRadius 5\nPosition 1 2 3\n}\n\
          Group SphereData01\n{\nRadius 1e40\nPosition -0 .5 1e-45\n}\n}\n\
          Group AttachingData01\n{\nAttachingDataType 2\nisAttaching 0\nAttachingModelIndex 1\nAttachingBoneName \"\"\n\
          EffectScriptName \"none.mse\"\nEffectPosition 1 2\nEffectRotation 4 5 6\n}\n\
          Group AttachingData02\n{\nAttachingDataType 3\nisAttaching 0\nAttachingModelIndex 1\nAttachingBoneName x\nObjectScriptName o.obj\n}\n\
          Group AttachingData03\n{\nAttachingDataType 9\nisAttaching 0\nAttachingModelIndex 1\nAttachingBoneName x\n}\n\
          Group AttachingData04\n{\nAttachingDataType 1\nisAttaching 1\nAttachingModelIndex 0\nAttachingBoneName x\n\
          CollisionType 1\nSphereDataCount 3\nGroup SphereData00\n{\nRadius 2\nPosition 0 0 0\n}\n}\n}\n",
    ),
    (
        "attaching_short.msm",
        b"Group AttachingData\n{\nAttachingDataCount 3\nGroup AttachingData00\n{\nAttachingDataType 3\n\
          isAttaching 1\nAttachingModelIndex 0\nAttachingBoneName x\n}\n}\nBaseModelFileName after\n",
    ),
    ("smoke_missing.msm", b"List SmokeFileName\n{\n2 \"missing.mse\"\n}\nGroup ShapeData\n{\n}\n"),
    ("smoke_odd.msm", b"List SmokeFileName\n{\n0\n}\nBaseModelFileName x\n"),
    ("smoke_range.msm", b"List SmokeFileName\n{\n-1 a.mse\n}\n"),
    ("smoke_empty.msm", b"List SmokeFileName\n{\n}\nSmokeBoneName bone\n"),
    ("unbalanced.msm", b"BaseModelFileName x\nGroup A\n{\n"),
    ("empty.msm", b""),
];

/// Motions: every event type, partial event loads, cursor strands, and the
/// failure paths. Under `ymir work/` so their sound scripts are looked up.
const SYNTHETIC_MOTION: &[(&str, &[u8])] = &[
    (
        "ymir work/t/full.msa",
        b"MotionFileName \"D:/a.GR2\"\nMotionDuration 1.5\nAccumulation 0 -10.5 1e3\n\
          Group ComboInputData\n{\nPreInputTime 0.1\nDirectInputTime 0.2\nInputLimitTime 0.3\n}\n\
          Group LoopData\n{\nMotionLoopCount 3\nLoopCancelEnable 2\nLoopStartTime 0.1\nLoopEndTime 0.9\n}\n\
          Group AttackingData\n{\nAttackType 1\nHittingType 2\nStiffenTime 0.5\nInvisibleTime 0.25\nExternalForce 10\n\
          HitLimitCount 4\nMotionType 3\nHitDataCount 2\n\
          Group HitData00\n{\nAttackingStartTime 0.1\nAttackingEndTime 0.2\nAttackingBone \"bone\"\nWeaponLength 50\n\
          List HitPosition\n{\n0.5 1 2 3 4 5 6\n0.25 0 0 0 0 0 0\n0.5 9 9 9 9 9 9\n-0 1 1 1 1 1 1\n0 2 2 2 2 2 2\n}\n}\n\
          Group Other\n{\nAttackingStartTime 1\nAttackingEndTime 2\n}\n}\n\
          Group MotionEventData\n{\nMotionEventDataCount 10\n\
          Group Event00\n{\nMotionEventType 1\nStartingTime 0.5\nIndependentFlag 3\nAttachingEnable 1\n\
          AttachingBoneName \"b\"\nEffectFileName \"D:/E/x.mse\"\nEffectPosition 1 2 3\nFollowingEnable 2\n}\n\
          Group Event01\n{\nMotionEventType 2\nStartingTime 1\nDuringTime 0.25\nPower 3\nAffectingRange 9\n}\n\
          Group Event02\n{\nMotionEventType 4\nStartingTime -1.5\nDuringTime 1\nHittingType 0\nStiffenTime 0\n\
          InvisibleTime 0\nExternalForce 0\nCollisionType 2\nSphereDataCount 1\nGroup SphereData00\n{\nRadius 5\nPosition 1 2 3\n}\n}\n\
          Group Event03\n{\nMotionEventType 5\nStartingTime 0.3\nSoundFileName \"s.wav\"\n}\n\
          Group Event04\n{\nMotionEventType 6\nStartingTime 0.4\nAttachingEnable 0\nAttachingBoneName x\n\
          FlyFileName \"F.msf\"\nFlyPosition 0 0 1\n}\n\
          Group Event05\n{\nMotionEventType 7\nStartingTime 0.6\n}\n\
          Group Event06\n{\nMotionEventType 8\nStartingTime 0.7\n}\n\
          Group Event07\n{\nMotionEventType 9\nStartingTime 1e10\n}\n\
          Group Event08\n{\nMotionEventType 10\nStartingTime 0.9\nEffectFileName \"t.mse\"\nEffectPosition 1 1 1\nFishingEffectFlag 1\n}\n\
          Group Event09\n{\nMotionEventType 1\nStartingTime 2\nAttachingEnable 1\nEffectFileName \"e\"\n}\n}\n",
    ),
    (
        "sound/t/full.mss",
        b"ScriptType CharacterSoundInformation\nSoundDataCount 3\nSoundData00 0.5 \"a.wav\"\n\
          SoundData01 1.0 b.wav\nSoundData02 -0.1 \"c.wav\"\n",
    ),
    (
        "ymir work/t/partial_events.msa",
        b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 3\n\
          Group Event00\n{\nMotionEventType 2\nStartingTime 0\nPower 1\n}\n\
          Group Event01\n{\nMotionEventType 4\nStartingTime 0\nDuringTime 1\nHittingType 1\n}\n\
          Group Event02\n{\nMotionEventType 10\nStartingTime 0\nEffectFileName x\n}\n}\nGroup LoopData\n{\nLoopStartTime 0\nLoopEndTime 1\n}\n",
    ),
    ("sound/t/partial_events.mss", b"SoundDataCount 2\nSoundData00 0.5 a\n"),
    (
        "ymir work/t/stranded1.msa",
        b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 1\n\
          Group Event00\n{\nMotionEventType 4\nStartingTime 0\nDuringTime 1\nHittingType 0\nStiffenTime 0\n\
          InvisibleTime 0\nExternalForce 0\nCollisionType 2\nSphereDataCount 1\nGroup SphereData00\n{\nRadius 5\nStartingTime 2\n}\n}\n}\n\
          Group LoopData\n{\nLoopStartTime 0\nLoopEndTime 1\n}\n",
    ),
    (
        "ymir work/t/stranded2.msa",
        b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 2\n\
          Group Event00\n{\nMotionEventType 4\nStartingTime 0\nDuringTime 1\nHittingType 0\nStiffenTime 0\n\
          InvisibleTime 0\nExternalForce 0\nCollisionType 2\nSphereDataCount 1\nGroup SphereData00\n{\nRadius 5\nStartingTime 2\n}\n}\n\
          Group Event01\n{\nMotionEventType 7\nStartingTime 0\n}\n}\n",
    ),
    (
        "ymir work/t/two_event_groups.msa",
        b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 1\n\
          Group Event00\n{\nMotionEventType 7\nStartingTime 0\n}\n}\nGroup MotionEventData\n{\n}\n\
          Group motioneventdata\n{\nMotionEventDataCount 0\n}\nGroup LoopData\n{\nMotionLoopCount 5\nLoopStartTime 1\nLoopEndTime 2\n}\n",
    ),
    ("ymir work/t/strange_type.msa", b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 1\nGroup Event00\n{\nMotionEventType 3\nStartingTime 0\n}\n}\n"),
    ("ymir work/t/missing_event.msa", b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 2\nGroup Event00\n{\nMotionEventType 7\nStartingTime 0\n}\n}\n"),
    ("ymir work/t/no_start.msa", b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 1\nGroup Event00\n{\nMotionEventType 7\n}\n}\n"),
    ("ymir work/t/bad_combo.msa", b"MotionFileName a\nMotionDuration 1\nGroup ComboInputData\n{\nPreInputTime 1\n}\n"),
    ("ymir work/t/bad_attack.msa", b"MotionFileName a\nMotionDuration 1\nGroup AttackingData\n{\nHittingType 1\nStiffenTime 0\nInvisibleTime 0\nExternalForce 0\n}\n"),
    ("ymir work/t/old_attack.msa", b"MotionFileName a\nMotionDuration 1\nGroup AttackingData\n{\nHittingType 1\nStiffenTime 0\nInvisibleTime 0\nExternalForce 0\nAttackingType 2\nAttackingStartTime 0\nAttackingEndTime 1\n}\n"),
    ("ymir work/t/bad_accumulation.msa", b"MotionFileName a\nMotionDuration 1\nAccumulation 1 2\n"),
    ("ymir work/t/no_duration.msa", b"MotionFileName a\n"),
    ("ymir work/t/unbalanced.msa", b"MotionFileName a\nMotionDuration 2\nGroup LoopData\n{\nLoopStartTime 0\nLoopEndTime 1\n"),
    ("ymir work/t/missing_value.msa", b"MotionFileName a\nMotionDuration 2\nlonely\nGroup LoopData\n{\nLoopStartTime 0\nLoopEndTime 1\n}\n"),
    ("ymir work/t/bad_sound.msa", b"MotionFileName a\nMotionDuration 2\n"),
    ("sound/t/bad_sound.mss", b"SoundDataCount 1\nSoundData00 0.5 a b\n"),
    ("ymir work/t/no_sound_count.msa", b"MotionFileName a\nMotionDuration 2\n"),
    ("sound/t/no_sound_count.mss", b"SoundData00 0.5 a\n"),
    ("ymir work/t/unbalanced_sound.msa", b"MotionFileName a\nMotionDuration 2\n"),
    ("sound/t/unbalanced_sound.mss", b"SoundDataCount 1\nSoundData00 0.5 a\nGroup X\n{\n"),
    ("ymir work/t/zero_sounds.msa", b"MotionFileName a\nMotionDuration 2\n"),
    ("sound/t/zero_sounds.mss", b"SoundDataCount 0\n"),
    ("short.msa", b"MotionFileName a\nMotionDuration 2\n"),
];

const SYNTHETIC_PROPERTY: &[(&str, &[u8])] = &[
    ("property/a/b.prb", b"YPRT\r\n12345\r\nBuildingFile \"x.gr2\"\r\nbuildingfile other\r\nPropertyName \"b\"\r\n\r\n\"q\" \"\"\r\nempty\r\n"),
    ("property/a/dup.prb", b"YPRT\n12345\nBuildingFile \"dup.gr2\"\n"),
    ("property/c.prt", b"YPRT\r-7\rTreeSize 1 2 3"),
    ("property/d.pre", b"YPRT\r\n\r\nk v"),
    ("property/e.pra", b"YPRTx\n1\n"),
    ("property/f.prd", b"DDS \x00\x01"),
    ("property/g.prd", b"YP"),
    ("property/h.prd", b"YPRT\n99999999999\nk \"unterminated\nk2 v\n"),
    ("property/reserve", b"1\r\n-2\r\n\r\n12345x\r\nabc\r\n4294967295\r\n"),
    ("property/reserve2", b"77\n"),
    ("other/x.prb", b"YPRT\n5\nk v\n"),
];

fn write_fixture(root: &Path, files: &[(&str, &[u8])]) {
    let pack = root.join("pack/synth");
    for (name, data) in files {
        let path = pack.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
    fs::write(root.join("pack/Index"), "synth\tFOLDER\n").unwrap();
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_race_data_matches_cpp() {
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), SYNTHETIC_RACE);
    let counts = assert_same_dump(fixture.path(), "pack/Index", "race");
    assert_eq!(counts["msm ok"] + counts["msm failed"], SYNTHETIC_RACE.len());
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_motion_data_matches_cpp() {
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), SYNTHETIC_MOTION);
    let counts = assert_same_dump(fixture.path(), "pack/Index", "motion");
    let msa = SYNTHETIC_MOTION.iter().filter(|(n, _)| n.ends_with(".msa")).count();
    assert_eq!(counts["msa ok"] + counts["msa failed"], msa);
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_properties_match_cpp() {
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), SYNTHETIC_PROPERTY);
    assert_same_dump(fixture.path(), "pack/Index", "property");
}
