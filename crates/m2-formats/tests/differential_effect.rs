//! Differential tests: the Rust effect loaders against the C++ ones
//! (`tools/differential/effect`). Both sides load every registered `.mse`,
//! `.msf` and `.mde` and print what the data objects hold, including the
//! sound scripts and texture lists they pull in.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use m2_formats::effect::{
    EffectMesh, ElementBase, Error, MeshError, TimeEventTableColor, TimeEventTableFloat, load_effect, load_effect_mesh,
    load_flying_data,
};
use m2_formats::pack::{PackManager, SearchMode, load_index};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

fn extension(name: &[u8]) -> Vec<u8> {
    match name.iter().rposition(|&b| b == b'.') {
        Some(dot) => name[dot + 1..].to_ascii_lowercase(),
        None => Vec::new(),
    }
}

/// Builds one dump line of tab-separated fields.
struct Line<'a>(&'a mut Vec<u8>);

impl Line<'_> {
    fn s(self, s: &[u8]) -> Self {
        self.0.push(b'\t');
        self.0.extend_from_slice(s);
        self
    }
    fn f(self, v: f32) -> Self {
        write!(self.0, "\t{:08x}", v.to_bits()).unwrap();
        self
    }
    fn fs(mut self, vs: &[f32]) -> Self {
        for &v in vs {
            self = self.f(v);
        }
        self
    }
    fn i(self, v: impl Into<i64>) -> Self {
        write!(self.0, "\t{}", v.into()).unwrap();
        self
    }
    fn u(self, v: impl Into<u64>) -> Self {
        write!(self.0, "\t{}", v.into()).unwrap();
        self
    }
    fn x(self, v: u32) -> Self {
        write!(self.0, "\t{v:08x}").unwrap();
        self
    }
    fn end(self) {
        self.0.push(b'\n');
    }
}

fn line<'a>(out: &'a mut Vec<u8>, tag: &str) -> Line<'a> {
    out.extend_from_slice(tag.as_bytes());
    Line(out)
}

fn table(out: &mut Vec<u8>, label: &str, t: &TimeEventTableFloat) {
    let mut l = line(out, "T").s(label.as_bytes()).u(t.len() as u64);
    for e in t {
        l = l.f(e.time).f(e.value);
    }
    l.end();
}

fn color_table(out: &mut Vec<u8>, label: &str, t: &TimeEventTableColor) {
    let mut l = line(out, "T").s(label.as_bytes()).u(t.len() as u64);
    for e in t {
        l = l.f(e.time).x(e.value);
    }
    l.end();
}

fn element(out: &mut Vec<u8>, e: &ElementBase) {
    line(out, "E").f(e.start_time).u(e.positions.len() as u64).end();
    for p in &e.positions {
        line(out, "EP").f(p.time).i(p.moving_type).fs(&p.position).fs(&p.control_point).end();
    }
}

fn file_line(out: &mut Vec<u8>, name: &[u8], status: &str) {
    line(out, "F").s(name).s(status.as_bytes()).end();
}

fn requests(out: &mut Vec<u8>, names: impl IntoIterator<Item = Vec<u8>>) {
    for name in names {
        line(out, "R").s(&name).end();
    }
}

fn dump_effect(out: &mut Vec<u8>, manager: &PackManager, name: &[u8]) {
    let d = match load_effect(manager, name) {
        Ok(d) => d,
        Err(Error::Throws) => return file_line(out, name, "throw"),
        Err(_) => return file_line(out, name, "0"),
    };
    file_line(out, name, "1");
    line(out, "B").s(&d.file_name).f(d.bounding_sphere_radius).fs(&d.bounding_sphere_position).end();
    for p in &d.particles {
        out.extend_from_slice(b"P\n");
        element(out, &p.base);
        let e = &p.emitter;
        line(out, "PE")
            .u(e.max_emission_count)
            .f(e.cycle_length)
            .i(e.cycle_loop)
            .i(e.loop_count)
            .u(e.emitter_shape)
            .u(e.emitter_advanced_type)
            .i(e.emit_from_edge)
            .fs(&e.emitting_size)
            .f(e.emitting_radius)
            .fs(&e.emitting_direction)
            .end();
        table(out, "emittingsize", &e.emitting_size_events);
        table(out, "emittingangularvelocity", &e.emitting_angular_velocity);
        table(out, "emittingdirectionx", &e.emitting_direction_x);
        table(out, "emittingdirectiony", &e.emitting_direction_y);
        table(out, "emittingdirectionz", &e.emitting_direction_z);
        table(out, "emittingvelocity", &e.emitting_velocity);
        table(out, "emissioncountpersecond", &e.emission_count_per_second);
        table(out, "lifetime", &e.life_time);
        table(out, "sizex", &e.size_x);
        table(out, "sizey", &e.size_y);
        let q = &p.particle;
        line(out, "PP")
            .u(q.src_blend_type)
            .u(q.dest_blend_type)
            .u(q.color_operation_type)
            .u(q.billboard_type)
            .u(q.rotation_type)
            .f(q.rotation_speed)
            .u(q.rotation_random_starting_begin)
            .u(q.rotation_random_starting_end)
            .i(q.attach)
            .i(q.stretch)
            .u(q.tex_ani_type)
            .f(q.tex_ani_delay)
            .i(q.tex_ani_random_start_frame)
            .u(q.textures.len() as u64)
            .end();
        table(out, "gravity", &q.gravity);
        table(out, "airresistance", &q.air_resistance);
        table(out, "scalex", &q.scale_x);
        table(out, "scaley", &q.scale_y);
        color_table(out, "color", &q.color);
        table(out, "rotation", &q.rotation);
    }
    for m in &d.meshes {
        out.extend_from_slice(b"M\n");
        element(out, &m.base);
        line(out, "MS")
            .s(&m.mesh_file_name)
            .i(m.mesh_animation_loop)
            .i(m.mesh_animation_loop_count)
            .f(m.mesh_animation_frame_delay)
            .u(m.mesh_data.len() as u64)
            .end();
        for md in &m.mesh_data {
            line(out, "MD")
                .u(md.billboard_type)
                .i(md.blending_enable)
                .u(md.blending_src_type)
                .u(md.blending_dest_type)
                .i(md.texture_alpha_enable)
                .u(md.color_operation_type)
                .fs(&md.color_factor)
                .i(md.texture_animation_loop_enable)
                .f(md.texture_animation_frame_delay)
                .u(md.texture_animation_start_frame)
                .end();
            table(out, "alpha", &md.alpha);
        }
    }
    for l in &d.lights {
        out.extend_from_slice(b"L\n");
        element(out, &l.base);
        line(out, "LD")
            .f(l.duration)
            .i(l.loop_flag)
            .i(l.loop_count)
            .fs(&l.ambient)
            .fs(&l.diffuse)
            .f(l.max_range)
            .f(l.attenuation0)
            .f(l.attenuation1)
            .f(l.attenuation2)
            .end();
        table(out, "range", &l.range);
    }
    for s in &d.sounds {
        line(out, "S").u(s.frame).s(&s.file_name).end();
    }
    // Texture requests happen in particle order.
    requests(out, d.particles.into_iter().flat_map(|p| p.particle.textures));
}

fn dump_flying(out: &mut Vec<u8>, manager: &PackManager, name: &[u8]) {
    let (d, ok) = load_flying_data(manager, name);
    file_line(out, name, if ok { "1" } else { "0" });
    line(out, "FD")
        .s(&d.file_name)
        .i(i32::from(d.spreading))
        .i(i32::from(d.maintain_parallel))
        .f(d.initial_velocity)
        .f(d.cone_angle)
        .f(d.roll_angle)
        .fs(&d.angular_velocity)
        .f(d.gravity)
        .f(d.bomb_range)
        .s(&d.bomb_effect_name)
        .x(d.bomb_effect_id)
        .i(i32::from(d.homing))
        .f(d.homing_max_angle)
        .f(d.homing_start_time)
        .i(i32::from(d.hit_on_background))
        .i(i32::from(d.hit_on_another_monster))
        .i(d.pierce_count)
        .f(d.collision_sphere_radius)
        .f(d.range)
        .fs(&d.acceleration)
        .end();
    for a in &d.attach {
        line(out, "A")
            .i(a.kind)
            .i(a.fly_type)
            .s(&a.file_name)
            .i(i32::from(a.has_tail))
            .x(a.tail_color)
            .f(a.tail_length)
            .f(a.tail_size)
            .i(i32::from(a.rect_shape))
            .f(a.roll)
            .f(a.distance)
            .f(a.period)
            .f(a.amplitude)
            .end();
    }
}

fn dump_mesh(out: &mut Vec<u8>, manager: &PackManager, name: &[u8]) {
    let mesh: EffectMesh = match load_effect_mesh(manager, name) {
        Ok(m) => m,
        Err(MeshError::Malformed) => return file_line(out, name, "skip"),
        Err(_) => return file_line(out, name, "0"),
    };
    file_line(out, name, "1");
    line(out, "H").i(mesh.geom_count).i(mesh.frame_count).end();
    for m in &mesh.meshes {
        line(out, "G")
            .s(m.object_name())
            .s(m.diffuse_map_file_name())
            .u(m.frames.len() as u64)
            .u(m.textures.len() as u64)
            .end();
        for f in &m.frames {
            line(out, "FR")
                .u(f.changed_frame)
                .f(f.visibility)
                .u(f.vertex_count)
                .u(f.texture_vertex_count)
                .u(f.index_count)
                .u(f.vertices.len() as u64)
                .end();
            for v in &f.vertices {
                line(out, "V").fs(&v.position).fs(&v.tex_coord).end();
            }
        }
    }
    requests(out, mesh.meshes.into_iter().flat_map(|m| m.textures));
}

/// Same output as `m2diff_effect`; also returns the count of each status.
fn rust_dump(client: &Path, index: &str, kind: &str) -> (Vec<u8>, BTreeMap<String, usize>) {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", index).unwrap();

    let mut names: Vec<&[u8]> = manager.file_names().filter(|n| extension(n) == kind.as_bytes()).collect();
    names.sort_unstable();

    let mut out = Vec::new();
    for name in names {
        match kind {
            "mse" => dump_effect(&mut out, &manager, name),
            "msf" => dump_flying(&mut out, &manager, name),
            _ => dump_mesh(&mut out, &manager, name),
        }
    }

    let mut counts = BTreeMap::new();
    for l in out.split(|&b| b == b'\n').filter(|l| l.starts_with(b"F\t")) {
        let status = l.rsplit(|&b| b == b'\t').next().unwrap();
        *counts.entry(String::from_utf8_lossy(status).into_owned()).or_default() += 1;
    }
    (out, counts)
}

fn assert_same_dump(kind: &str) -> BTreeMap<String, usize> {
    assert_same_dump_in(&client_dir(), "pack/Index.dev", kind)
}

fn assert_same_dump_in(client: &Path, index: &str, kind: &str) -> BTreeMap<String, usize> {
    let cpp = run(&cpp_tool("effect"), &[client.as_os_str(), index.as_ref(), kind.as_ref()]);
    let (rust, counts) = rust_dump(client, index, kind);
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!(".{kind}: {lines} lines identical, files by status {counts:?}");
    counts
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_effect_scripts_match_cpp() {
    let counts = assert_same_dump("mse");
    assert!(counts.values().sum::<usize>() > 0, "no .mse files found");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_effect_flying_data_matches_cpp() {
    let counts = assert_same_dump("msf");
    assert!(counts.values().sum::<usize>() > 0, "no .msf files found");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_effect_meshes_match_cpp() {
    let counts = assert_same_dump("mde");
    assert!(counts.values().sum::<usize>() > 0, "no .mde files found");
}

/// A particle element with every required key, `extra` lines appended.
fn particle(emitter_extra: &str, particle_extra: &str) -> String {
    format!(
        "Group Particle\n{{\n\
         \tGroup EmitterProperty\n\t{{\n\
         \t\tMaxEmissionCount 10\n\t\tEmitterShape 2\n\
         \t\tList TimeEventEmissionCountPerSecond\n\t\t{{\n\t\t\t0 5\n\t\t}}\n\
         \t\tList TimeEventLifeTime\n\t\t{{\n\t\t\t0 1\n\t\t}}\n\
         \t\tList TimeEventSizeX\n\t\t{{\n\t\t\t0 1\n\t\t}}\n\
         \t\tList TimeEventSizeY\n\t\t{{\n\t\t\t0 1\n\t\t}}\n\
         {emitter_extra}\t}}\n\
         \tGroup ParticleProperty\n\t{{\n\
         \t\tBillboardType 1\n\t\tRotationType 2\n\t\tRotationSpeed 1.5\n\
         \t\tRotationRandomStartingBegin 70000\n\t\tRotationRandomStartingEnd 3\n\
         \t\tStretchEnable 2\n\t\tTexAniType 1\n\t\tTexAniDelay 0.1\n\t\tTexAniRandomStartFrameEnable 0\n\
         \t\tList TimeEventScaleX\n\t\t{{\n\t\t\t0 1\n\t\t}}\n\
         \t\tList TimeEventScaleY\n\t\t{{\n\t\t\t0 1\n\t\t}}\n\
         \t\tList TimeEventColorRed\n\t\t{{\n\t\t\t0 1 0.5 0.25\n\t\t}}\n\
         \t\tList TimeEventColorGreen\n\t\t{{\n\t\t\t0 0.5\n\t\t}}\n\
         \t\tList TimeEventColorBlue\n\t\t{{\n\t\t}}\n\
         \t\tList TimeEventAlpha\n\t\t{{\n\t\t\t0.75 1 0.1 0.3 -0 2\n\t\t}}\n\
         \t\tList TimeEventRotation\n\t\t{{\n\t\t\t0 0\n\t\t}}\n\
         \t\tList TextureFiles\n\t\t{{\n\t\t\ta.dds\n\t\t\t\"D:\\Ymir Work\\b.DDS\"\n\t\t}}\n\
         {particle_extra}\t}}\n}}\n"
    )
}

const MESH: &str = "Group Mesh\n{\n\tStartTime 0.5\n\tMeshFileName \"m.mde\"\n\tMeshAnimationLoopEnable 2\n\
    \tMeshAnimationFrameDelay 0.033\n\tMeshElementCount 3\n\
    \tGroup MeshData00\n\t{\n\t\tBillboardType 1\n\t\tBlendingEnable 1\n\t\tBlendingSrcType 5\n\
    \t\tBlendingDestType 2\n\t\tTextureAnimationLoopEnable 1\n\t\tTextureAnimationFrameDelay 0.5\n\
    \t\tColorFactor 0.1 0.2 0.3 0.4\n\t\tList TimeEventAlpha\n\t\t{\n\t\t\t0 1 1 0\n\t\t}\n\t}\n\
    \tGroup MeshData01\n\t{\n\t\tBillboardType 3\n\t}\n}\n";

const LIGHT: &str = "Group Light\n{\n\tList TimeEventPosition\n\t{\n\t\t0 MOVING_TYPE_DIRECT 1 2 3\n\
    \t\t0.5 MOVING_TYPE_BEZIER_CURVE 4 5 6 7 8 9\n\t}\n\tDuration 2\n\tLoopFlag 1\n\
    \tAmbientColor 1 1 1 1\n\tDiffuseColor 0.5 0.5 0.5 1\n\tMaxRange 200\n\tAttenuation0 0\n\
    \tAttenuation1 0.2\n\tAttenuation2 0\n\tList TimeEventRange\n\t{\n\t\t0 0 1 1\n\t}\n}\n";

/// An `.mde`: `(object name, diffuse map, frames)` per geom, each frame
/// `(visibility, positions, indices, uvs, uv indices)`.
type Frame<'a> = (f32, &'a [[f32; 3]], &'a [u32], &'a [[f32; 2]], &'a [u32]);

fn mde(v2: bool, geoms: &[(&[u8], &[u8], Vec<Frame>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(if v2 { b"MDEData002\0" } else { b"EffectData\0" });
    out.extend_from_slice(&(geoms.len() as u32).to_le_bytes());
    out.extend_from_slice(&(geoms.first().map_or(0, |g| g.2.len()) as u32).to_le_bytes());
    for (object, diffuse, frames) in geoms {
        let mut fixed = |s: &[u8], n: usize| {
            let mut b = s.to_vec();
            b.resize(n, 0);
            out.extend_from_slice(&b);
        };
        fixed(object, 32);
        fixed(diffuse, 128);
        let counts = |f: &Frame| [f.1.len() as u32, f.2.len() as u32, f.3.len() as u32];
        if !v2 {
            for c in counts(&frames[0]) {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        for (i, f) in frames.iter().enumerate() {
            if v2 {
                out.push(i as u8 + 1);
                out.extend_from_slice(&f.0.to_le_bytes());
                for c in counts(f) {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            } else {
                out.extend_from_slice(&f.0.to_le_bytes());
            }
            out.extend(f.1.iter().flatten().flat_map(|v| v.to_le_bytes()));
            out.extend(f.2.iter().flat_map(|v| v.to_le_bytes()));
            out.extend(f.3.iter().flatten().flat_map(|v| v.to_le_bytes()));
            out.extend(f.4.iter().flat_map(|v| v.to_le_bytes()));
        }
    }
    out
}

/// Inputs the real data does not have: the failure paths of every loader.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_effects_match_cpp() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("pack/synth");
    let fx = root.join("ymir work/fx");
    std::fs::create_dir_all(&fx).unwrap();
    std::fs::create_dir_all(root.join("sound/fx")).unwrap();
    std::fs::write(fixture.path().join("pack/Index"), "synth\tFOLDER\n").unwrap();
    let put = |path: &Path, data: &[u8]| std::fs::write(path, data).unwrap();

    let full = particle(
        "\t\tCycleLength 0.2\n\t\tCycleLoopEnable 3\n\t\tEmitterAdvancedType 2\n\t\tEmittingSize 1 2 3\n",
        "\t\tGravity 9.5\n\t\tList TimeEventAirResistance\n\t\t{\n\t\t\t0 0.1 1 0.2\n\t\t}\n",
    );
    // No EmitterAdvancedType (resets the shape), timed gravity.
    let shape_bug = particle("", "\t\tList TimeEventGravity\n\t\t{\n\t\t\t0 1 0.5 2\n\t\t}\n");
    let effects: &[(&str, String)] = &[
        (
            "all.mse",
            format!("BoundingSphereRadius 50\nBoundingSpherePosition 1 2 3\n{full}{MESH}{LIGHT}{shape_bug}"),
        ),
        // No EmitterProperty group: the cursor climbs to the root, so
        // ParticleProperty and the next elements are looked up from there.
        (
            "climb.mse",
            format!(
                "Group Particle\n{{\n\tMaxEmissionCount 3\n\tEmitterShape 1\n}}\n{LIGHT}{full}Group Other\n{{\n}}\n"
            ),
        ),
        ("bad_moving.mse", format!("Group Light\n{{\n\tStartTime 1\n\tList TimeEventPosition\n\t{{\n\t\t0 MOVING_TYPE_DIRECT 1 2 3\n\t\t1 SIDEWAYS 1 2 3\n\t}}\n\tMaxRange 5\n}}\n{full}")),
        ("odd_event.mse", "Group Light\n{\n\tAmbientColor 1 1 1 1\n\tDiffuseColor 1 1 1 1\n\tMaxRange 1\n\tAttenuation0 0\n\tAttenuation1 0\n\tAttenuation2 0\n\tList TimeEventRange\n\t{\n\t\t0 1 2\n\t}\n}\n".into()),
        ("short_position.mse", "Group Mesh\n{\n\tList TimeEventPosition\n\t{\n\t\t0 MOVING_TYPE_DIRECT 1 2\n\t}\n}\n".into()),
        ("unbalanced.mse", format!("{full}}}\n")),
        ("light_partial.mse", "Group Light\n{\n\tAmbientColor 1 1 1\n\tDuration 4\n}\nGroup Light\n{\n\tAmbientColor 1 1 1 1\n\tDiffuseColor 1 0 0 1\n}\n".into()),
        ("mesh_missing.mse", "Group Mesh\n{\n\tMeshAnimationLoopEnable 1\n}\nGroup Mesh\n{\n\tMeshFileName \"d:\\x.mde\"\n\tMeshAnimationLoopEnable 1\n\tMeshAnimationFrameDelay 1\n}\n".into()),
        ("sound.mse", full.clone()),
        ("sound_bad.mse", "BoundingSphereRadius 1\n".into()),
        ("sound_negative.mse", "BoundingSphereRadius 1\n".into()),
        ("empty.mse", String::new()),
        ("CASE.MSE", "BoundingSphereRadius 2\n".into()),
    ];
    for (name, text) in effects {
        put(&fx.join(name), text.as_bytes());
    }
    put(
        &root.join("sound/fx/sound.mss"),
        b"ScriptType CharacterSoundInformation\nSoundDataCount 3\nSoundData00 0.5 \"a.wav\"\nSoundData01 0.0166 b.wav\nSoundData02 -1 c.wav\n",
    );
    put(&root.join("sound/fx/sound_bad.mss"), b"SoundDataCount 2\nSoundData00 0.5 a.wav\nSoundData01 1 2 3\n");
    put(&root.join("sound/fx/sound_negative.mss"), b"SoundDataCount -1\n");
    put(&root.join("sound/fx/empty.mss"), b"SoundDataCount 1\nSoundData00 1 x.wav\n}\n");
    put(&root.join("loose.mse"), b"BoundingSphereRadius 3\n");

    let flying: &[(&str, &str)] = &[
        (
            "all.msf",
            "SpreadingFlag 2\nMaintainParallelFlag 1\nInitialVelocity 300\nConeAngle 5\nAngularVelocity 1 2 3\n\
             Gravity 9\nHitOnBackground 1\nPierceCount 2\nBombRange 20\nBombEffect \"bomb.mse\"\nHomingFlag 1\n\
             HomingStartTime 0.5\nRange 1000\nAcceleration 0 0 -10\n\
             Group AttachData\n{\n\tType 1\n\tFlyType 3\n\tAttachFile \"fly.mse\"\n\tTailFlag 1\n\
             \tTailColor 4278190335\n\tTailLength 2\n\tTailShapeRect 1\n\tPeriod 0.5\n}\n\
             Group AttachData\n{\n\tType 1\n\tAttachFile \"\"\n}\n\
             Group Junk\n{\n\tType 1\n}\n\
             Group AttachData\n{\n\tFlyType 2\n}\n\
             Group AttachData\n{\n\tType 2\n\tAttachFile \"D:/Other/x.mse\"\n\tTailFlag 0\n\tRoll 3\n}\n",
        ),
        // `temp` is still 1 from HomingFlag when TailFlag is missing.
        ("stale.msf", "InitialVelocity 1\nHomingFlag 1\nRange 1\nBombEffect \"\"\nGroup AttachData\n{\n\tType 0\n}\n"),
        ("no_velocity.msf", "SpreadingFlag 1\nRange 5\n"),
        ("no_range.msf", "InitialVelocity 5\nBombEffect \"D:\\Ymir Work\\B.mse\"\nAcceleration 1 2\n"),
        ("unbalanced.msf", "InitialVelocity 5\nRange 5\n{\n"),
    ];
    for (name, text) in flying {
        put(&fx.join(name), text.as_bytes());
    }

    let quad: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, -0.0], [0.0, 2.5, 1.0e-40]];
    let uv: [[f32; 2]; 2] = [[0.0, 0.0], [0.5, 1.0]];
    let frame: Frame = (0.75, &quad, &[0, 1, 2], &uv, &[1, 0, 1]);
    let empty: Frame = (1.0, &[], &[], &[], &[]);
    put(
        &fx.join("v1.mde"),
        &mde(false, &[(b"obj", b"tex.dds", vec![frame, frame]), (b"second", b"anim.ifl", vec![frame, frame])]),
    );
    put(
        &fx.join("v2.mde"),
        &mde(true, &[(b"o", b"D:\\Ymir Work\\fx\\Anim.IFL", vec![frame, empty]), (b"", b"", vec![empty, frame])]),
    );
    put(&fx.join("missing_ifl.mde"), &mde(true, &[(b"o", b"none.ifl", vec![frame])]));
    put(&fx.join("noframes.mde"), &mde(true, &[]));
    let mut long_name = mde(false, &[([b'n'; 32].as_slice(), [b'd'; 127].as_slice(), vec![frame])]);
    long_name[11 + 8 + 32 + 127] = 0;
    put(&fx.join("long_name.mde"), &long_name);
    put(&fx.join("bad_header.mde"), b"MDEData003\0\0\0\0\0\0\0\0\0");
    let mut truncated = mde(true, &[(b"o", b"t.dds", vec![frame])]);
    truncated.truncate(truncated.len() - 3);
    put(&fx.join("truncated.mde"), &truncated);
    put(&fx.join("bad_index.mde"), &mde(true, &[(b"o", b"t.dds", vec![(1.0, &quad, &[0, 3, 1], &uv, &[0, 0, 0])])]));
    put(&fx.join("short.mde"), b"EffectData");
    put(&fx.join("anim.ifl"), b"a.dds\r\n\r\nsub/b.dds\nc.dds");

    assert_same_dump_in(fixture.path(), "pack/Index", "mse");
    assert_same_dump_in(fixture.path(), "pack/Index", "msf");
    assert_same_dump_in(fixture.path(), "pack/Index", "mde");
}
