//! Edge cases of the effect loaders (`m2_formats::effect`). The same
//! behaviours are checked against C++ by `differential_effect.rs`.

use m2_formats::effect::{
    EffectPosition, ElementBase, Error, MOVING_TYPE_BEZIER_CURVE, MOVING_TYPE_DIRECT, MeshError, TimeEvent, case_crc32,
    color_to_dword, only_path_name, parse_effect, parse_effect_mesh, parse_flying_data, time_event_blend_value,
};
use m2_formats::pack::PackManager;
use m2_formats::script;

fn empty_manager() -> (tempfile::TempDir, PackManager) {
    let dir = tempfile::tempdir().unwrap();
    let manager = PackManager::new(dir.path());
    (dir, manager)
}

fn effect(text: &str) -> Result<m2_formats::effect::EffectData, Error> {
    let (_dir, manager) = empty_manager();
    parse_effect(&manager, b"d:/ymir work/fx/a.mse", &script::parse(text.as_bytes()))
}

#[test]
fn missing_group_moves_the_cursor_up() {
    // No EmitterProperty: the emitter keys are read from the particle group,
    // then CGotoChild climbs to the root; the light after it still loads.
    let d = effect(
        "Group Particle\n{\nMaxEmissionCount 3\nEmitterShape 1\n}\n\
         Group Light\n{\nAmbientColor 1 1 1 1\nDiffuseColor 0 0 0 0\nMaxRange 7\n}\n",
    )
    .unwrap();
    assert_eq!(d.particles.len(), 1);
    assert_eq!(d.particles[0].emitter.max_emission_count, 3);
    // EmitterAdvancedType is missing, and C++ resets the shape instead.
    assert_eq!(d.particles[0].emitter.emitter_shape, 0);
    assert_eq!(d.lights.len(), 1);
    assert_eq!(d.lights[0].max_range, 7.0);
    // Stopped before the attenuations.
    assert_eq!(d.lights[0].attenuation1, 0.1);
}

#[test]
fn odd_time_event_lists_throw_in_cpp() {
    let r = effect(
        "Group Light\n{\nAmbientColor 1 1 1 1\nDiffuseColor 1 1 1 1\nMaxRange 1\nAttenuation0 0\nAttenuation1 0\nAttenuation2 0\nList TimeEventRange\n{\n0 1 2\n}\n}\n",
    );
    assert!(matches!(r, Err(Error::Throws)));
}

#[test]
fn unknown_moving_type_skips_the_element_body() {
    let d =
        effect("Group Light\n{\nList TimeEventPosition\n{\n0 MOVING_TYPE_DIRECT 1 2 3\n1 SIDEWAYS\n}\nMaxRange 5\n}\n")
            .unwrap();
    let light = &d.lights[0];
    assert_eq!(light.base.positions.len(), 1);
    assert_eq!(light.max_range, 300.0, "OnLoadScript must not run");
}

#[test]
fn unbalanced_scripts_do_not_load() {
    assert!(matches!(effect("a 1\n}\n"), Err(Error::Unbalanced)));
}

#[test]
fn color_channels_merge_at_every_time() {
    let channel = |events: &[(f32, f32)]| -> String { events.iter().map(|(t, v)| format!("{t} {v} ")).collect() };
    let text = format!(
        "Group Particle\n{{\nGroup EmitterProperty\n{{\nMaxEmissionCount 1\nEmitterShape 0\n\
         List TimeEventEmissionCountPerSecond\n{{\n0 1\n}}\nList TimeEventLifeTime\n{{\n0 1\n}}\n\
         List TimeEventSizeX\n{{\n0 1\n}}\nList TimeEventSizeY\n{{\n0 1\n}}\n}}\n\
         Group ParticleProperty\n{{\nBillboardType 0\nRotationType 0\nRotationSpeed 0\n\
         RotationRandomStartingBegin 0\nRotationRandomStartingEnd 0\nStretchEnable 0\nTexAniType 0\n\
         TexAniDelay 0\nTexAniRandomStartFrameEnable 0\nList TimeEventScaleX\n{{\n0 1\n}}\n\
         List TimeEventScaleY\n{{\n0 1\n}}\nList TimeEventColorRed\n{{\n{}\n}}\n\
         List TimeEventColorGreen\n{{\n{}\n}}\nList TimeEventColorBlue\n{{\n}}\n\
         List TimeEventAlpha\n{{\n-0 1 1 0.5\n}}\nList TimeEventRotation\n{{\n}}\n\
         List TextureFiles\n{{\nx.dds\n}}\n}}\n}}\n",
        channel(&[(0.0, 1.0), (1.0, 0.0)]),
        channel(&[(0.5, 0.5)]),
    );
    let d = effect(&text).unwrap();
    let p = &d.particles[0].particle;
    let times: Vec<u32> = p.color.iter().map(|e| e.time.to_bits()).collect();
    // 0 from red keeps -0 from alpha out.
    assert_eq!(times, [0.0f32.to_bits(), 0.5f32.to_bits(), 1.0f32.to_bits()]);
    assert_eq!(p.color[0].value, 0xff_ff_80_00);
    assert_eq!(p.color[2].value, 0x80_00_80_00);
    assert_eq!(p.textures, [b"d:/ymir work/fx/x.dds".to_vec()]);
}

#[test]
fn blend_value_matches_get_time_event_blend_value() {
    let t = |events: &[(f32, f32)]| events.iter().map(|&(time, value)| TimeEvent { time, value }).collect::<Vec<_>>();
    assert_eq!(time_event_blend_value(1.0, &[]), 0.0);
    assert_eq!(time_event_blend_value(9.0, &t(&[(0.0, 4.0)])), 4.0);
    let table = t(&[(0.0, 0.0), (1.0, 10.0), (2.0, 30.0)]);
    assert_eq!(time_event_blend_value(-1.0, &table), 0.0);
    assert_eq!(time_event_blend_value(3.0, &table), 30.0);
    assert_eq!(time_event_blend_value(1.0, &table), 10.0);
    assert_eq!(time_event_blend_value(1.5, &table), 20.0);
}

#[test]
fn dword_colors_round_and_clamp() {
    assert_eq!(color_to_dword([1.0, 0.0, 0.5, 2.0]), 0xff_ff_00_80);
    assert_eq!(color_to_dword([-1.0, f32::NAN, 0.001, 0.999]), 0xff_00_00_00);
}

#[test]
fn positions_interpolate() {
    let base = ElementBase {
        start_time: 0.0,
        positions: vec![
            EffectPosition {
                time: 0.0,
                position: [0.0, 0.0, 0.0],
                moving_type: MOVING_TYPE_DIRECT,
                control_point: [0.0; 3],
            },
            EffectPosition {
                time: 1.0,
                position: [10.0, 0.0, 0.0],
                moving_type: MOVING_TYPE_BEZIER_CURVE,
                control_point: [0.0, 10.0, 0.0],
            },
            EffectPosition {
                time: 2.0,
                position: [10.0, 0.0, 10.0],
                moving_type: MOVING_TYPE_DIRECT,
                control_point: [0.0; 3],
            },
        ],
    };
    assert_eq!(base.position_at(0.5), [5.0, 0.0, 0.0]);
    assert_eq!(base.position_at(1.5), [10.0, 5.0, 2.5]);
    assert_eq!(base.position_at(5.0), [10.0, 0.0, 10.0]);
}

#[test]
fn flying_tail_flag_reuses_the_last_integer() {
    let s = script::parse(b"InitialVelocity 1\nHomingFlag 1\nRange 1\nGroup AttachData\n{\nType 1\n}\n");
    let (d, ok) = parse_flying_data(b"D:\\Fx\\A.msf", &s);
    assert!(ok);
    assert_eq!(d.file_name, b"d:/fx/a.msf");
    assert!(d.attach[0].has_tail, "TailFlag missing: temp is still HomingFlag's 1");
}

#[test]
fn flying_bomb_effect_id_is_the_case_crc() {
    let s = script::parse(b"InitialVelocity 1\nBombEffect \"Bomb.MSE\"\n");
    let (d, ok) = parse_flying_data(b"d:/fx/a.msf", &s);
    assert!(!ok, "Range is missing");
    assert_eq!(d.bomb_effect_name, b"d:/fx/Bomb.MSE");
    assert_eq!(d.bomb_effect_id, case_crc32(b"d:/fx/bomb.mse"));
    assert_eq!(d.bomb_effect_id, crc32fast::hash(b"d:/fx/bomb.mse"));
}

#[test]
fn path_helpers() {
    assert_eq!(only_path_name(b"a\\b/c.mse"), b"a\\b/");
    assert_eq!(only_path_name(b"c.mse"), b"");
}

fn mde_v2(index: u32) -> Vec<u8> {
    let mut b = b"MDEData002\0".to_vec();
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&[b'o'; 32]);
    let mut diffuse = b"t.dds".to_vec();
    diffuse.resize(128, 0);
    b.extend_from_slice(&diffuse);
    b.push(7);
    b.extend_from_slice(&0.5f32.to_le_bytes());
    for count in [1u32, 1, 1] {
        b.extend_from_slice(&count.to_le_bytes());
    }
    for v in [1.0f32, 2.0, 3.0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&index.to_le_bytes());
    for v in [0.25f32, 0.0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

#[test]
fn effect_mesh_reads_frames() {
    let (_dir, manager) = empty_manager();
    let mesh = parse_effect_mesh(&manager, &mde_v2(0)).unwrap();
    let m = &mesh.meshes[0];
    assert_eq!(m.object_name(), [b'o'; 32]);
    assert_eq!(m.textures, [b"t.dds".to_vec()]);
    let f = &m.frames[0];
    assert_eq!((f.changed_frame, f.visibility, f.index_count), (7, 0.5, 1));
    assert_eq!(f.vertices[0].position, [1.0, 2.0, 3.0]);
    assert_eq!(f.vertices[0].tex_coord[1].to_bits(), (-0.0f32).to_bits());
}

#[test]
fn effect_mesh_rejects_bad_input() {
    let (_dir, manager) = empty_manager();
    assert!(matches!(parse_effect_mesh(&manager, &mde_v2(1)), Err(MeshError::Malformed)));
    let full = mde_v2(0);
    assert!(matches!(parse_effect_mesh(&manager, &full[..full.len() - 1]), Err(MeshError::Malformed)));
    assert!(matches!(parse_effect_mesh(&manager, b"EffectData"), Err(MeshError::Malformed)));
    assert!(matches!(parse_effect_mesh(&manager, b"Something\0\0\0\0\0"), Err(MeshError::BadHeader)));
}
