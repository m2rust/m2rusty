//! Rust counterpart of `m2diff_image` (`tools/differential/image/ImageDump.cpp`).
//!
//!   m2-image-dump [--full] pack <client dir> <index file>
//!   m2-image-dump [--full] files <path>...
//!
//! Prints the same canonical dump as the C++ tool; see there for the format.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use m2_formats::image::dxt::DxtcImage;
use m2_formats::image::{TgaImage, decode_image, get_image_info};
use m2_formats::pack::{PackManager, SearchMode, load_index};

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn words_to_bytes(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|p| p.to_le_bytes()).collect()
}

fn put_hex(out: &mut impl Write, data: &[u8]) -> std::io::Result<()> {
    write!(out, "px ")?;
    for b in data {
        write!(out, "{b:02x}")?;
    }
    writeln!(out)
}

fn ends_with(name: &[u8], suffix: &str) -> bool {
    name.len() >= suffix.len() && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

fn dump_tga(out: &mut impl Write, bytes: &[u8], full: bool) -> std::io::Result<()> {
    let Some(h) = m2_formats::image::TgaHeader::parse(bytes) else {
        return writeln!(out, "tga fail");
    };
    let supported = h.img_type == 3
        || (h.img_type == 2 && matches!(h.color_bits, 16 | 24 | 32))
        || (h.img_type == 10 && matches!(h.color_bits, 24 | 32));
    if !supported {
        return writeln!(out, "tga unsupported");
    }
    if u64::from(h.width) * u64::from(h.height) * 4 > 64 << 20 {
        return writeln!(out, "tga big");
    }
    match TgaImage::decode(bytes) {
        None => writeln!(out, "tga fail"),
        Some(t) => {
            let px = words_to_bytes(t.image().pixels());
            writeln!(
                out,
                "tga {} {} {} {} {} {:016x}",
                t.image().width(),
                t.image().height(),
                h.img_type,
                h.color_bits,
                h.desc,
                fnv1a64(&px)
            )?;
            if full { put_hex(out, &px) } else { Ok(()) }
        }
    }
}

fn dump_dds(out: &mut impl Write, bytes: &[u8], full: bool) -> std::io::Result<()> {
    let Some(d) = DxtcImage::load_header(bytes) else {
        return writeln!(out, "dds fail");
    };
    writeln!(
        out,
        "dds {} {} {} {} {} {:08x} {} {}",
        d.format_name,
        d.width,
        d.height,
        d.mip_map_count,
        u8::from(d.mip_texture),
        d.flags,
        d.pitch,
        d.comp_format as i32
    )?;
    for i in 0..d.mip_map_count {
        write!(out, "level {i}")?;
        let Some(start) = d.level_offsets[i as usize] else {
            writeln!(out, " off=- copy=- pix=-")?;
            continue;
        };
        write!(out, " off={start}")?;

        if d.flags & 0x0002_0000 == 0 && i != 0 {
            write!(out, " copy=none")?;
        } else {
            match d.copy(bytes, i as usize) {
                Some(c) => write!(out, " copy={:016x}", fnv1a64(c))?,
                None => write!(out, " copy=oob")?,
            }
        }

        let big = d.width < 0 || d.height < 0 || {
            let w = (d.width as u32).checked_shr(i).unwrap_or(0);
            let h = (d.height as u32).checked_shr(i).unwrap_or(0);
            u64::from(w) * u64::from(h) > 1 << 26
        };
        if big {
            writeln!(out, " pix=big")?;
            continue;
        }
        let need = d.level_data_size(i).unwrap_or(0);
        match bytes.get(start..start + need).and_then(|c| d.decompress(i, c)) {
            None => writeln!(out, " pix=oob")?,
            Some(px) => {
                let px = words_to_bytes(&px);
                writeln!(out, " pix={:016x}", fnv1a64(&px))?;
                if full {
                    put_hex(out, &px)?;
                }
            }
        }
    }
    Ok(())
}

fn dump(out: &mut impl Write, name: &[u8], bytes: &[u8], full: bool) -> std::io::Result<()> {
    out.write_all(b"file ")?;
    out.write_all(name)?;
    writeln!(out, " {}", bytes.len())?;

    match get_image_info(bytes) {
        Some(i) => writeln!(out, "info {} {} {}", i.width, i.height, i.format)?,
        None => writeln!(out, "info -")?,
    }
    match decode_image(bytes) {
        Some(d) => {
            writeln!(out, "decode {} {} {:016x}", d.width, d.height, fnv1a64(&d.bgra))?;
            if full {
                put_hex(out, &d.bgra)?;
            }
        }
        None => writeln!(out, "decode -")?,
    }

    if ends_with(name, ".tga") {
        dump_tga(out, bytes, full)?;
    }
    if ends_with(name, ".dds") {
        dump_dds(out, bytes, full)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    let prog = args.remove(0);
    let full = args.first().is_some_and(|a| a == "--full");
    if full {
        args.remove(0);
    }
    let mut out = BufWriter::new(std::io::stdout().lock());

    let result = match args.first().map(String::as_str) {
        Some("pack") if args.len() >= 3 => {
            let mut manager = PackManager::new(&args[1]);
            manager.set_search_mode(SearchMode::PackFirst);
            if let Err(e) = load_index(&mut manager, "pack", &args[2]) {
                eprintln!("{}: {e}", args[2]);
                return ExitCode::FAILURE;
            }
            let mut names: Vec<&[u8]> = manager
                .file_names()
                .filter(|n| [".dds", ".tga", ".jpg", ".jpeg", ".png", ".bmp"].iter().any(|e| ends_with(n, e)))
                .collect();
            names.sort_unstable();
            names.into_iter().try_for_each(|name| match manager.get(name) {
                Ok(data) => dump(&mut out, name, &data, full),
                Err(_) => {
                    out.write_all(b"file ")?;
                    out.write_all(name)?;
                    writeln!(out, "\nunreadable")
                }
            })
        }
        Some("files") => args[1..].iter().try_for_each(|path| match std::fs::read(path) {
            Ok(data) => dump(&mut out, path.as_bytes(), &data, full),
            Err(_) => writeln!(out, "file {path}\nunreadable"),
        }),
        _ => {
            eprintln!("usage: {prog} [--full] pack <client dir> <index file>\n       {prog} [--full] files <path>...");
            return ExitCode::from(2);
        }
    };
    match result.and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::SUCCESS, // closed pipe
    }
}
