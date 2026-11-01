//! Fixtures shared by the pack tests.

#![allow(dead_code)]

pub mod differential;

use std::fs;
use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Same generator as `DeterministicBytes` in the C++ test support.
pub fn deterministic_bytes(len: usize, mut seed: u32) -> Vec<u8> {
    (0..len)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        })
        .collect()
}

/// Writes a zip with the `zip` crate. Names ending in `/` become directory
/// entries; entries alternate between stored and deflated.
pub fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    for (i, (name, data)) in entries.iter().enumerate() {
        if name.ends_with('/') {
            zip.add_directory(*name, SimpleFileOptions::default()).unwrap();
            continue;
        }
        let method = if i % 2 == 0 { CompressionMethod::Deflated } else { CompressionMethod::Stored };
        zip.start_file(*name, SimpleFileOptions::default().compression_method(method)).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

/// Writes a stored-only zip by hand, so names can be arbitrary bytes without
/// the UTF-8 flag (as Windows tools wrote CP949 names).
pub fn write_raw_name_zip(path: &Path, entries: &[(&[u8], &[u8])]) {
    let mut out = Vec::new();
    let mut central = Vec::new();

    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32fast::hash(data);
        let (n, d) = (name.len() as u16, data.len() as u32);

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // version, flags, stored, time, date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&d.to_le_bytes());
        out.extend_from_slice(&d.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // made by, needed, flags, stored, time, date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&d.to_le_bytes());
        central.extend_from_slice(&d.to_le_bytes());
        central.extend_from_slice(&n.to_le_bytes());
        central.extend_from_slice(&[0; 8]); // extra, comment, disk, internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }

    let central_offset = out.len() as u32;
    let count = entries.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // disk numbers
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());

    fs::write(path, out).unwrap();
}
