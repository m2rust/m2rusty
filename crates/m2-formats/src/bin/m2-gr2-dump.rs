//! Rust counterpart of `m2diff_gr2` (`tools/differential/gr2/Gr2Dump.cpp`).
//!
//!   m2-gr2-dump [--full] pack <client dir> <index file>
//!   m2-gr2-dump [--full] files <path>...
//!
//! Parses every `.gr2` (all registered virtual names ending in `.gr2`,
//! sorted, or the given files in order) and prints everything
//! [`m2_formats::gr2::parse`] returns, floats as the hex of their bits. Bulk
//! arrays print as count + fnv1a64 of their bytes, or element by element
//! with `--full`.

use std::ffi::OsString;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use m2_formats::gr2::{self, Animation, Curve, Indices, Material, Mesh, Model, Transform, Vertices};
use m2_formats::pack::{PackManager, SearchMode, load_index};

struct Dumper<W: Write> {
    out: W,
    full: bool,
}

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

fn floats(f: &[f32]) -> String {
    f.iter()
        .map(|v| format!("{:08x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}

fn transform(t: &Transform) -> String {
    let ss: Vec<f32> = t.scale_shear.iter().flatten().copied().collect();
    format!(
        "{:08x},{},{},{}",
        t.flags,
        floats(&t.position),
        floats(&t.orientation),
        floats(&ss)
    )
}

impl<W: Write> Dumper<W> {
    /// Bytes as they are, except control bytes and `\`, which print as `\xHH`.
    fn name(&mut self, s: &[u8]) -> io::Result<()> {
        for &b in s {
            if b < 0x20 || b == b'\\' {
                write!(self.out, "\\x{b:02x}")?;
            } else {
                self.out.write_all(&[b])?;
            }
        }
        Ok(())
    }

    /// ` <fnv1a64>` of `bytes` or ` <values>`; ` -` for a null pointer.
    fn array(&mut self, present: bool, bytes: &[u8], values: impl FnOnce() -> String) -> io::Result<()> {
        if !present {
            write!(self.out, " -")
        } else if self.full {
            write!(self.out, " {}", values())
        } else {
            write!(self.out, " {:016x}", fnv1a64(bytes))
        }
    }

    fn curve(&mut self, label: &str, c: &Curve) -> io::Result<()> {
        write!(self.out, " {label}={},{}", c.degree, c.knot_count)?;
        let bytes: Vec<u8> = c.knots.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.array(!c.knots.is_empty(), &bytes, || floats(&c.knots))?;
        write!(self.out, " {}", c.control_count)?;
        let bytes: Vec<u8> = c.controls.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.array(!c.controls.is_empty(), &bytes, || floats(&c.controls))
    }

    fn material(&mut self, i: usize, m: Option<&Material>) -> io::Result<()> {
        let Some(m) = m else {
            return writeln!(self.out, "  material {i} none");
        };
        write!(self.out, "  material {i} ")?;
        self.name(&m.name)?;
        writeln!(self.out, "\t{} {}", m.map_count, m.maps.len())?;
        for (j, map) in m.maps.iter().enumerate() {
            write!(self.out, "   map {j} ")?;
            self.name(&map.usage)?;
            match map.material.as_deref().map(|sub| &sub.texture) {
                None => writeln!(self.out, "\tnone")?,
                Some(None) => writeln!(self.out, "\tnotexture")?,
                Some(Some(tex)) => {
                    write!(self.out, "\ttexture ")?;
                    self.name(&tex.from_file_name)?;
                    writeln!(self.out)?;
                }
            }
        }
        Ok(())
    }

    fn mesh(&mut self, i: usize, mesh: Option<&Mesh>) -> io::Result<()> {
        let Some(mesh) = mesh else {
            return writeln!(self.out, " mesh {i} none");
        };
        write!(self.out, " mesh {i} ")?;
        self.name(&mesh.name)?;
        writeln!(self.out)?;

        match &mesh.vertex_data {
            None => writeln!(self.out, "  vertexdata none")?,
            Some(vd) => {
                let kind = if mesh.is_rigid() { "pnt332" } else { "pwnt3432" };
                write!(self.out, "  vertexdata {kind} {}", vd.vertex_count)?;
                match &vd.vertices {
                    None => writeln!(self.out, " -")?,
                    Some(v) => self.vertices(v)?,
                }
            }
        }

        match &mesh.topology {
            None => writeln!(self.out, "  topology none")?,
            Some(t) => {
                writeln!(self.out, "  topology groups {}", t.groups.len())?;
                for (g, grp) in t.groups.iter().enumerate() {
                    writeln!(
                        self.out,
                        "   group {g} {} {} {}",
                        grp.material_index, grp.tri_first, grp.tri_count
                    )?;
                }
                let (kind, bytes, text): (_, Vec<u8>, Vec<String>) = match &t.indices {
                    Indices::None => ("none", Vec::new(), Vec::new()),
                    Indices::U16(v) => (
                        "u16",
                        v.iter().flat_map(|i| i.to_le_bytes()).collect(),
                        v.iter().map(u16::to_string).collect(),
                    ),
                    Indices::I32(v) => (
                        "i32",
                        v.iter().flat_map(|i| i.to_le_bytes()).collect(),
                        v.iter().map(i32::to_string).collect(),
                    ),
                };
                write!(self.out, "  indices {kind} {}", t.indices.len())?;
                if !matches!(t.indices, Indices::None) {
                    self.array(true, &bytes, || text.join(","))?;
                }
                writeln!(self.out)?;
            }
        }

        writeln!(
            self.out,
            "  materials {} {}",
            mesh.material_binding_count,
            mesh.material_bindings.len()
        )?;
        for (m, mat) in mesh.material_bindings.iter().enumerate() {
            self.material(m, mat.as_ref())?;
        }

        writeln!(
            self.out,
            "  bonebindings {} {}",
            mesh.bone_binding_count,
            mesh.bone_bindings.len()
        )?;
        for (b, bb) in mesh.bone_bindings.iter().enumerate() {
            write!(self.out, "   bonebinding {b} ")?;
            self.name(&bb.bone_name)?;
            writeln!(self.out, "\t{} {}", floats(&bb.obb_min), floats(&bb.obb_max))?;
        }
        Ok(())
    }

    fn vertices(&mut self, v: &Vertices) -> io::Result<()> {
        if !self.full {
            let mut bytes = Vec::new();
            match v {
                Vertices::Rigid(v) => {
                    for x in v {
                        for f in x.position.iter().chain(&x.normal).chain(&x.uv) {
                            bytes.extend_from_slice(&f.to_le_bytes());
                        }
                    }
                }
                Vertices::Skinned(v) => {
                    for x in v {
                        for f in &x.position {
                            bytes.extend_from_slice(&f.to_le_bytes());
                        }
                        bytes.extend_from_slice(&x.bone_weights);
                        bytes.extend_from_slice(&x.bone_indices);
                        for f in x.normal.iter().chain(&x.uv) {
                            bytes.extend_from_slice(&f.to_le_bytes());
                        }
                    }
                }
            }
            return writeln!(self.out, " {:016x}", fnv1a64(&bytes));
        }
        writeln!(self.out)?;
        match v {
            Vertices::Rigid(v) => {
                for x in v {
                    writeln!(
                        self.out,
                        "   v {},{},{}",
                        floats(&x.position),
                        floats(&x.normal),
                        floats(&x.uv)
                    )?;
                }
            }
            Vertices::Skinned(v) => {
                for x in v {
                    let hex = |b: &[u8; 4]| b.iter().map(|b| format!("{b:02x}")).collect::<String>();
                    writeln!(
                        self.out,
                        "   v {} {} {} {},{}",
                        floats(&x.position),
                        hex(&x.bone_weights),
                        hex(&x.bone_indices),
                        floats(&x.normal),
                        floats(&x.uv)
                    )?;
                }
            }
        }
        Ok(())
    }

    fn model(&mut self, i: usize, model: &Model) -> io::Result<()> {
        write!(self.out, "model {i} ")?;
        self.name(&model.name)?;
        writeln!(self.out, "\t{}", transform(&model.initial_placement))?;
        match &model.skeleton {
            None => writeln!(self.out, " skeleton none")?,
            Some(s) => {
                write!(self.out, " skeleton ")?;
                self.name(&s.name)?;
                writeln!(self.out, "\t{} {}", s.bone_count, s.bones.len())?;
                for (b, bone) in s.bones.iter().enumerate() {
                    write!(self.out, "  bone {b} ")?;
                    self.name(&bone.name)?;
                    let inv: Vec<f32> = bone.inverse_world.iter().flatten().copied().collect();
                    writeln!(
                        self.out,
                        "\t{} {} {}",
                        bone.parent_index,
                        transform(&bone.local_transform),
                        floats(&inv)
                    )?;
                }
            }
        }
        writeln!(
            self.out,
            " meshbindings {} {}",
            model.mesh_binding_count,
            model.mesh_bindings.len()
        )?;
        for (m, mesh) in model.mesh_bindings.iter().enumerate() {
            self.mesh(m, mesh.as_deref())?;
        }
        Ok(())
    }

    fn animation(&mut self, i: usize, a: &Animation) -> io::Result<()> {
        write!(self.out, "animation {i} ")?;
        self.name(&a.name)?;
        writeln!(
            self.out,
            "\t{:08x} {:08x} {}",
            a.duration.to_bits(),
            a.time_step.to_bits(),
            a.track_groups.len()
        )?;
        for (g, tg) in a.track_groups.iter().enumerate() {
            write!(self.out, " trackgroup {g} ")?;
            self.name(&tg.name)?;
            writeln!(
                self.out,
                "\t{} {}",
                floats(&tg.loop_translation),
                tg.transform_tracks.len()
            )?;
            for (t, tt) in tg.transform_tracks.iter().enumerate() {
                write!(self.out, "  track {t} ")?;
                self.name(&tt.name)?;
                write!(self.out, "\t")?;
                self.curve("pos", &tt.position)?;
                self.curve("rot", &tt.orientation)?;
                self.curve("ss", &tt.scale_shear)?;
                writeln!(self.out)?;
            }
        }
        Ok(())
    }

    fn file(&mut self, name: &[u8], data: Option<&[u8]>) -> io::Result<()> {
        self.out.write_all(b"file ")?;
        self.out.write_all(name)?;
        writeln!(self.out)?;
        let Some(data) = data else {
            return writeln!(self.out, "unreadable");
        };
        let Ok(file) = gr2::parse(data) else {
            return writeln!(self.out, "error");
        };
        writeln!(self.out, "models {} {}", file.model_count, file.models.len())?;
        for (m, model) in file.models.iter().enumerate() {
            self.model(m, model)?;
        }
        writeln!(
            self.out,
            "animations {} {}",
            file.animation_count,
            file.animations.len()
        )?;
        for (a, anim) in file.animations.iter().enumerate() {
            self.animation(a, anim)?;
        }
        Ok(())
    }
}

fn run(args: &[OsString]) -> Result<ExitCode, String> {
    let (full, rest) = match args.first().and_then(|a| a.to_str()) {
        Some("--full") => (true, &args[1..]),
        _ => (false, args),
    };
    let mut d = Dumper {
        out: BufWriter::new(io::stdout().lock()),
        full,
    };
    let io_err = |e: io::Error| e.to_string();
    match rest {
        [mode, client, index] if mode == "pack" => {
            let index = index.to_str().ok_or("index file name is not UTF-8")?;
            let mut manager = PackManager::new(client);
            manager.set_search_mode(SearchMode::PackFirst);
            let report = load_index(&mut manager, "pack", index).map_err(|e| format!("{index}: {e}"))?;
            for (pack, e) in &report.failed {
                eprintln!("cannot register {pack}: {e}");
            }
            let mut names: Vec<Vec<u8>> = manager
                .file_names()
                .filter(|n| n.ends_with(b".gr2"))
                .map(<[u8]>::to_vec)
                .collect();
            names.sort_unstable();
            for name in &names {
                let data = manager.get(name).ok();
                d.file(name, data.as_deref()).map_err(io_err)?;
            }
        }
        [mode, paths @ ..] if mode == "files" => {
            for path in paths {
                let data = std::fs::read(path).ok();
                d.file(path.as_encoded_bytes(), data.as_deref()).map_err(io_err)?;
            }
        }
        _ => return Ok(ExitCode::from(2)),
    }
    d.out.flush().map_err(io_err)?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(&args) {
        Ok(code) => {
            if code != ExitCode::SUCCESS {
                eprintln!("usage: m2-gr2-dump [--full] pack <client dir> <index file>");
                eprintln!("       m2-gr2-dump [--full] files <path>...");
            }
            code
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
