//! Differential test: the Rust effect runtime against the real C++ effect
//! library (`tools/differential/effect_runtime`, `m2diff_effect_runtime`).
//!
//! Both sides run every `.mse` of the client through two scenarios from the
//! same seed and fold what they would draw into a hash per file and scenario:
//!
//! - `regular`: 240 frames at the timer's 17/16 ms steps;
//! - `irregular`: 400 frames with gaps of 0..250 ms (and one of 5 s), a
//!   stop (`SetDeactive`) at frames 160 and 220, a `SetActive` at 190 and a
//!   restart (`Clear` + `SetEffectDataPointer`) at 280.
//!
//! A sample of effects is also compared record by record. Needs the client
//! and the C++ tool, so the tests are ignored by default; run them with
//! `scripts/differential.sh`.

mod support;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

use m2_effect::math::{Mat4, to_radian};
use m2_effect::{Camera, EffectInstance, GlibcRandom, MeshSource};
use m2_formats::effect::{EffectData, EffectMesh, load_effect, load_effect_mesh};
use m2_formats::pack::{PackManager, SearchMode, load_index};
use support::differential::{client_dir, cpp_tool, run};

const SEED: u32 = 1_234_567;

struct Scenario {
    name: &'static str,
    frames: u32,
    irregular: bool,
}

const SCENARIOS: [Scenario; 2] = [
    Scenario { name: "regular", frames: 240, irregular: false },
    Scenario { name: "irregular", frames: 400, irregular: true },
];

fn gap_for(sc: &Scenario, frame: u32) -> u32 {
    if !sc.irregular {
        return 0;
    }
    const TABLE: [u32; 8] = [0, 0, 3, 40, 1, 250, 7, 100];
    if frame == 10 {
        return 5000;
    }
    TABLE[((frame * 7 + 3) % 8) as usize]
}

/// The record writer: FNV-1a over the same bytes as the C++ `Rec`, and
/// optionally the text line.
struct Sink {
    hash: u64,
    text: Option<String>,
}

struct Rec<'a> {
    sink: &'a mut Sink,
}

impl Sink {
    fn byte(&mut self, b: u8) {
        self.hash ^= u64::from(b);
        self.hash = self.hash.wrapping_mul(0x100000001b3);
    }

    fn rec(&mut self, tag: &str) -> Rec<'_> {
        for b in tag.bytes() {
            self.byte(b);
        }
        if let Some(t) = &mut self.text {
            t.push_str(tag);
        }
        Rec { sink: self }
    }
}

impl Rec<'_> {
    fn u(&mut self, v: u32) -> &mut Self {
        for i in 0..4 {
            self.sink.byte((v >> (8 * i)) as u8);
        }
        if let Some(t) = &mut self.sink.text {
            write!(t, "\t{v:08x}").unwrap();
        }
        self
    }
    fn b(&mut self, v: bool) -> &mut Self {
        self.u(u32::from(v))
    }
    fn f(&mut self, v: f32) -> &mut Self {
        self.u(v.to_bits())
    }
    fn v3(&mut self, v: [f32; 3]) -> &mut Self {
        self.f(v[0]).f(v[1]).f(v[2])
    }
    fn m(&mut self, m: &Mat4) -> &mut Self {
        for v in m.to_array() {
            self.f(v);
        }
        self
    }
}

impl Drop for Rec<'_> {
    fn drop(&mut self) {
        self.sink.byte(b'\n');
        if let Some(t) = &mut self.sink.text {
            t.push('\n');
        }
    }
}

struct Meshes<'a> {
    manager: &'a PackManager,
    cache: std::cell::RefCell<HashMap<Vec<u8>, Option<Arc<EffectMesh>>>>,
}

impl MeshSource for Meshes<'_> {
    fn effect_mesh(&self, name: &[u8]) -> Option<Arc<EffectMesh>> {
        let mut cache = self.cache.borrow_mut();
        cache.entry(name.to_vec()).or_insert_with(|| load_effect_mesh(self.manager, name).ok().map(Arc::new)).clone()
    }
}

struct Env {
    camera: Camera,
    view: Mat4,
    global: Mat4,
}

fn snapshot(sink: &mut Sink, env: &Env, inst: &EffectInstance, frame: u32, ms: u32, render_elapsed: f32, max: &mut usize) {
    sink.rec("FR").u(frame).u(ms).f(render_elapsed).b(inst.is_alive());
    let mut total = 0;
    let draw = inst.render(&env.camera, &env.view, render_elapsed);

    for (s, ps) in inst.particle_systems().iter().enumerate() {
        sink.rec("PS")
            .u(s as u32)
            .u(ps.emission_count())
            .f(ps.local_time())
            .u(ps.loop_count() as u32)
            .f(ps.emission_residue())
            .b(ps.is_started())
            .b(ps.is_active());
        for f in 0..ps.frame_count() {
            for p in ps.particles(f) {
                total += 1;
                sink.rec("P")
                    .u(f as u32)
                    .v3(p.position)
                    .v3(p.last_position)
                    .v3(p.velocity)
                    .v3(p.start_position)
                    .f(p.half_size[0])
                    .f(p.half_size[1])
                    .f(p.scale[0])
                    .f(p.scale[1])
                    .f(p.rotation)
                    .u(p.color)
                    .u(u32::from(p.texture_animation_type))
                    .u(u32::from(p.frame_index))
                    .f(p.life_time)
                    .f(p.last_life_time);
            }
        }
        for q in &draw.particle_systems[s].quads {
            let mut r = sink.rec("Q");
            r.u(q.texture as u32).u(q.color);
            for c in q.positions {
                r.v3(c);
            }
        }
    }
    *max = (*max).max(total);

    for (i, mi) in inst.meshes().iter().enumerate() {
        let mut r = sink.rec("MI");
        r.u(i as u32).b(mi.is_started()).b(mi.is_active()).f(mi.local_time());
        r.b(mi.has_mesh()).b(mi.is_playing()).u(u32::from(mi.mesh_frame())).b(mi.mesh_controller_active());
        for (cur, active) in mi.texture_frames() {
            r.u(u32::from(cur)).b(active);
        }
        drop(r);
        let n = mi.render(&env.view, render_elapsed, inst.global_matrix());
        for d in n {
            let mut vh = 0xcbf29ce484222325u64;
            for v in d.vertices {
                for f in [v.position[0], v.position[1], v.position[2], v.tex_coord[0], v.tex_coord[1]] {
                    for k in 0..4 {
                        vh ^= u64::from((f.to_bits() >> (8 * k)) as u8);
                        vh = vh.wrapping_mul(0x100000001b3);
                    }
                }
            }
            let has_texture = d.texture.is_some();
            let (blend, src, dst) = match d.blend {
                Some((s, t)) => (true, u32::from(s), u32::from(t)),
                None => (false, 0, 0),
            };
            sink.rec("MD")
                .u(d.mesh_index as u32)
                .b(has_texture)
                .u(if has_texture { d.texture_frame as u32 } else { 0 })
                .m(&d.world)
                .b(blend)
                .u(src)
                .u(dst)
                .b(d.color_operation.is_some())
                .u(u32::from(d.color_operation.unwrap_or(0)))
                .u(d.color)
                .u(d.vertices.len() as u32)
                .u(vh as u32)
                .u((vh >> 32) as u32);

        }
    }

    let _ = draw.meshes.len();

    for (i, li) in inst.lights().iter().enumerate() {
        let mut r = sink.rec("LI");
        r.u(i as u32).b(li.is_started()).b(li.is_active()).f(li.local_time()).b(li.light().is_some());
        if let Some(l) = li.light() {
            r.v3(l.position).f(l.range);
            for c in l.ambient {
                r.f(c);
            }
            for c in l.diffuse {
                r.f(c);
            }
            for a in l.attenuation {
                r.f(a);
            }
        }
    }
}

struct Summary {
    line: String,
}

fn run_scenario(
    name: &str,
    data: &Arc<EffectData>,
    meshes: &dyn MeshSource,
    env: &Env,
    sc: &Scenario,
    text: bool,
) -> (Summary, Option<String>) {
    let mut sink = Sink { hash: 0xcbf29ce484222325, text: text.then(String::new) };
    if text {
        writeln!(sink.text.as_mut().unwrap(), "B\t{name}\t{}", sc.name).unwrap();
    }
    let mut rng = GlibcRandom::new(SEED);
    // The timer: 17, 16, 17, ... ms per Advance, plus the gap.
    let (mut ms, mut step) = (0u32, 17u32);
    let time = |ms: u32| ms as f32 / 1000.0f32;
    let mut inst = EffectInstance::new(Arc::clone(data), meshes, time(0));
    inst.set_global_matrix(env.global);
    let mut max = 0;
    for frame in 0..sc.frames {
        ms += gap_for(sc, frame) + step;
        let render_elapsed = step as f32 / 1000.0f32;
        if sc.irregular {
            if frame == 160 || frame == 220 {
                inst.set_deactive();
            }
            if frame == 190 {
                inst.set_active();
            }
            if frame == 280 {
                inst.restart(meshes, time(ms));
                inst.set_global_matrix(env.global);
            }
        }
        inst.update_to(time(ms), &mut rng);
        snapshot(&mut sink, env, &inst, frame, ms, render_elapsed, &mut max);
        step = if step == 17 { 16 } else { 17 };
    }
    let line = format!(
        "S\t{name}\t{}\t{:016x}\t{}\t{max}\t{}\t{}\t{}\t{}",
        sc.name,
        sink.hash,
        sc.frames,
        u32::from(inst.is_alive()),
        inst.particle_systems().len(),
        inst.meshes().len(),
        inst.lights().len()
    );
    (Summary { line }, sink.text)
}

fn parse_floats(line: &str) -> Vec<f32> {
    line.split('\t').skip(1).map(|t| f32::from_bits(u32::from_str_radix(t, 16).unwrap())).collect()
}

fn setup(client: &Path) -> (PackManager, Vec<u8>) {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", "pack/Index.dev").unwrap();
    (manager, Vec::new())
}

/// The camera, view matrix and global matrix of the C++ header, after
/// checking the global matrix and the random stream against our own.
fn parse_header(cpp: &str) -> Env {
    let mut camera = None;
    let mut view = None;
    let mut global = None;
    for line in cpp.lines() {
        match line.split('\t').next().unwrap() {
            "CAM" => {
                let v = parse_floats(line);
                camera = Some(Camera { up: [v[0], v[1], v[2]], cross: [v[3], v[4], v[5]], view: [v[6], v[7], v[8]] });
            }
            "VIEW" => view = Some(Mat4::from_array(parse_floats(line).try_into().unwrap())),
            "GLOBAL" => global = Some(Mat4::from_array(parse_floats(line).try_into().unwrap())),
            "RAND" => {
                let mut rng = GlibcRandom::new(SEED);
                let expect: Vec<u32> = line.split('\t').skip(1).map(|t| t.parse().unwrap()).collect();
                let got: Vec<u32> = (0..expect.len()).map(|_| rng.random()).collect();
                assert_eq!(got, expect, "glibc random() stream differs");
            }
            _ => {}
        }
    }
    let global = global.expect("GLOBAL");
    let mut mine = Mat4::rotation_yaw_pitch_roll(to_radian(30.0), to_radian(10.0), to_radian(-20.0));
    mine.set_translation([100.0, 200.0, 50.0]);
    assert_eq!(mine, global, "yaw/pitch/roll matrix differs");
    Env { camera: camera.expect("CAM"), view: view.expect("VIEW"), global }
}

fn extension(name: &[u8]) -> Vec<u8> {
    match name.iter().rposition(|&b| b == b'.') {
        Some(dot) => name[dot + 1..].to_ascii_lowercase(),
        None => Vec::new(),
    }
}

/// Features that make an effect worth a full comparison.
fn features(d: &EffectData) -> Vec<String> {
    let mut f = Vec::new();
    if !d.meshes.is_empty() {
        f.push("mesh".into());
    }
    if !d.lights.is_empty() {
        f.push("light".into());
    }
    for p in &d.particles {
        f.push(format!("bb{}", p.particle.billboard_type));
        f.push(format!("shape{}", p.emitter.emitter_shape));
        f.push(format!("adv{}", p.emitter.emitter_advanced_type));
        f.push(format!("rot{}", p.particle.rotation_type));
        f.push(format!("tex{}", p.particle.tex_ani_type));
        if p.particle.attach != 0 {
            f.push("attach".into());
        }
        if p.particle.stretch != 0 {
            f.push("stretch".into());
        }
        if p.particle.textures.len() > 1 {
            f.push("multitex".into());
        }
        if p.emitter.cycle_loop != 0 {
            f.push("cycleloop".into());
        }
        if p.emitter.emitting_angular_velocity.iter().any(|e| e.value != 0.0) {
            f.push("angular".into());
        }
    }
    for m in &d.meshes {
        for md in &m.mesh_data {
            f.push(format!("mbb{}", md.billboard_type));
        }
    }
    f
}

fn first_diff(cpp: &str, rust: &str) -> String {
    for (i, (a, b)) in cpp.lines().zip(rust.lines()).enumerate() {
        if a != b {
            return format!("line {}:\n  c++:  {a}\n  rust: {b}", i + 1);
        }
    }
    format!("length: c++ {} lines, rust {} lines", cpp.lines().count(), rust.lines().count())
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_effects_match_cpp() {
    let client = client_dir();
    let tool = cpp_tool("effect_runtime");
    let (manager, _) = setup(&client);
    let meshes = Meshes { manager: &manager, cache: Default::default() };

    let cpp = String::from_utf8(run(&tool, &[client.as_os_str(), "pack/Index.dev".as_ref(), "summary".as_ref()])).unwrap();
    let env = parse_header(&cpp);
    let cpp_summary: HashMap<(String, String), String> = cpp
        .lines()
        .filter(|l| l.starts_with("S\t"))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            ((f[1].to_string(), f[2].to_string()), l.to_string())
        })
        .collect();
    let cpp_meshes: usize = cpp.lines().find_map(|l| l.strip_prefix("MESHES\t")).unwrap().parse().unwrap();

    let mut names: Vec<Vec<u8>> = manager.file_names().filter(|n| extension(n) == b"mse").map(|n| n.to_vec()).collect();
    names.sort_unstable();

    let mut mismatches: Vec<String> = Vec::new();
    let mut compared = 0;
    let mut failed_load = 0;
    let mut sample: Vec<(String, Arc<EffectData>)> = Vec::new();
    let mut seen_features: std::collections::HashSet<String> = Default::default();
    let mut totals = (0usize, 0usize, 0usize, 0usize); // frames, max particles seen, files with meshes, with lights
    for (n, name_bytes) in names.iter().enumerate() {
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        let Ok(data) = load_effect(&manager, name_bytes) else {
            failed_load += 1;
            let c = cpp_summary.get(&(name.clone(), "load".into()));
            assert!(c.is_some(), "{name}: Rust fails to load, C++ does not");
            continue;
        };
        assert!(!cpp_summary.contains_key(&(name.clone(), "load".into())), "{name}: C++ fails to load, Rust does not");
        let data = Arc::new(data);
        let new_features: Vec<String> = features(&data).into_iter().filter(|f| seen_features.insert(f.clone())).collect();
        if !new_features.is_empty() || n % 40 == 0 {
            sample.push((name.clone(), Arc::clone(&data)));
        }
        for sc in &SCENARIOS {
            let (s, _) = run_scenario(&name, &data, &meshes, &env, sc, false);
            let expect = &cpp_summary[&(name.clone(), sc.name.to_string())];
            compared += 1;
            totals.0 += sc.frames as usize;
            if let Some(m) = s.line.split('\t').nth(5).and_then(|m| m.parse::<usize>().ok()) {
                totals.1 = totals.1.max(m);
            }
            if &s.line != expect {
                mismatches.push(format!("c++:  {expect}\nrust: {}", s.line));
            }
        }
        totals.2 += usize::from(!data.meshes.is_empty());
        totals.3 += usize::from(!data.lights.is_empty());
    }
    let rust_meshes = meshes.cache.borrow().values().filter(|m| m.is_some()).count();
    eprintln!(
        "{compared} runs ({} effects, {failed_load} unloadable, {} frames, max {} particles at once, {} with meshes, {} with lights), {} mismatches, meshes loaded rust {rust_meshes} c++ {cpp_meshes}",
        names.len(),
        totals.0,
        totals.1,
        totals.2,
        totals.3,
        mismatches.len()
    );

    // Where the hash differs, find the first differing record.
    if !mismatches.is_empty() {
        let first = &mismatches[0];
        let name = first.lines().next().unwrap().split('\t').nth(1).unwrap().to_string();
        let sc = first.lines().next().unwrap().split('\t').nth(2).unwrap().to_string();
        let data = sample.iter().find(|(n, _)| *n == name).map(|(_, d)| Arc::clone(d)).unwrap_or_else(|| {
            Arc::new(load_effect(&manager, name.as_bytes()).unwrap())
        });
        let scn = SCENARIOS.iter().find(|s| s.name == sc).unwrap();
        let (_, text) = run_scenario(&name, &data, &meshes, &env, scn, true);
        let full = String::from_utf8(run(&tool, &[client.as_os_str(), "pack/Index.dev".as_ref(), "full".as_ref(), std::ffi::OsStr::new(&name)])).unwrap();
        let start = full.find(&format!("B\t{name}\t{sc}\n")).unwrap();
        let end = full[start..].find("\nS\t").map_or(full.len(), |e| start + e + 1);
        let diff = first_diff(&full[start..end], &text.unwrap());
        panic!("{} of {compared} runs differ; first:\n{first}\n{diff}", mismatches.len());
    }
    assert_eq!(rust_meshes, cpp_meshes);

    // Record by record on a sample.
    let mut full_checked = 0;
    for chunk in sample.chunks(16) {
        full_checked += compare_full(&tool, &client, chunk, &meshes, &env);
    }
    eprintln!("{full_checked} full runs compared record by record ({} effects)", sample.len());
}

/// Runs the C++ `full` dump for `entries` and compares it with the Rust
/// records line by line. Returns the number of runs.
fn compare_full(
    tool: &Path,
    client: &Path,
    entries: &[(String, Arc<EffectData>)],
    meshes: &dyn MeshSource,
    env: &Env,
) -> usize {
    let mut args: Vec<&std::ffi::OsStr> = vec![client.as_os_str(), "pack/Index.dev".as_ref(), "full".as_ref()];
    args.extend(entries.iter().map(|(n, _)| std::ffi::OsStr::new(n.as_str())));
    let full = String::from_utf8(run(tool, &args)).unwrap();
    let mut rust = String::new();
    let mut runs = 0;
    for (name, data) in entries {
        for sc in &SCENARIOS {
            let (s, text) = run_scenario(name, data, meshes, env, sc, true);
            rust.push_str(&text.unwrap());
            rust.push_str(&s.line);
            rust.push('\n');
            runs += 1;
        }
    }
    let skip = ["CAM", "VIEW", "GLOBAL", "RAND", "MESHES"];
    let body: String =
        full.lines().filter(|l| !skip.iter().any(|s| l.starts_with(s))).map(|l| format!("{l}\n")).collect();
    assert!(body == rust, "full dumps differ: {}", first_diff(&body, &rust));
    runs
}

/// Effects written for this test: no real effect has a light, so lights (and
/// delayed starts, loops, the `SetDeactive` quirk) are only covered here.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_lights_match_cpp() {
    let dir = std::env::temp_dir().join(format!("m2_effect_synth_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("pack")).unwrap();
    std::fs::create_dir_all(dir.join("synth")).unwrap();
    std::fs::create_dir_all(dir.join("pack/dummy")).unwrap();
    std::fs::write(dir.join("pack/Index.dev"), b"dummy\tFOLDER\n").unwrap();

    let light = |extra: &str| {
        format!(
            "Group Light\n{{\n{extra}\nAmbientColor 0.5 0.25 0.125 1\nDiffuseColor 1 0.5 0.25 1\nMaxRange 100\n\
             Attenuation0 0\nAttenuation1 0.1\nAttenuation2 0\nList TimeEventRange\n{{\n0 0 0.5 1 1 0.5\n}}\n\
             List TimeEventPosition\n{{\n0 MOVING_TYPE_DIRECT 0 0 0\n1 MOVING_TYPE_BEZIER_CURVE 10 20 30 5 5 5\n\
             2 MOVING_TYPE_DIRECT 0 0 50\n}}\n}}\n"
        )
    };
    let script = [
        // Plays once and ends; delayed start.
        light("StartTime 0.2\nDuration 1.5"),
        // Loops twice.
        light("Duration 0.5\nLoopFlag 1\nLoopCount 2"),
        // Loops with a zero count: forever (the DWORD counter wraps).
        light("Duration 0.7\nLoopFlag 1\nLoopCount 0"),
    ]
    .concat();
    std::fs::write(dir.join("synth/light.mse"), script).unwrap();

    let (manager, _) = setup(&dir);
    let meshes = Meshes { manager: &manager, cache: Default::default() };
    let tool = cpp_tool("effect_runtime");
    let header = String::from_utf8(run(&tool, &[dir.as_os_str(), "pack/Index.dev".as_ref(), "full".as_ref()])).unwrap();
    let env = parse_header(&header);

    let data = Arc::new(load_effect(&manager, b"synth/light.mse").unwrap());
    assert_eq!(data.lights.len(), 3);
    let runs = compare_full(&tool, &dir, &[("synth/light.mse".to_string(), data)], &meshes, &env);
    eprintln!("synthetic light effect: {runs} runs identical");
    std::fs::remove_dir_all(&dir).ok();
}
