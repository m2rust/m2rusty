//! `.mdatr` building attribute files: `CAttributeData::OnLoad`
//! (`neoeter/src/render/AttributeData.cpp`).
//!
//! A building model `x.gr2` may have an `x.mdatr` next to it with the
//! collision shapes characters cannot walk through and the height meshes
//! they walk on:
//!
//! ```text
//! "AttributeData\0"                 14 bytes
//! u32 collision count, u32 height count
//! collision × count:
//!     u32 type, char[32] name, f32[3] position,
//!     f32 × (2 plane, 3 box/AABB/OBB, 1 sphere, 2 cylinder) dimensions,
//!     f32[4] rotation quaternion (x, y, z, w)
//! height × count:
//!     char[32] name, u32 vertex count, f32[3] × count
//! ```
//!
//! Everything is little-endian and in the model's own Metin2 space.

/// `COLLISION_TYPE_PLANE`.
pub const COLLISION_TYPE_PLANE: u32 = 0;
/// `COLLISION_TYPE_BOX`.
pub const COLLISION_TYPE_BOX: u32 = 1;
/// `COLLISION_TYPE_SPHERE`.
pub const COLLISION_TYPE_SPHERE: u32 = 2;
/// `COLLISION_TYPE_CYLINDER`.
pub const COLLISION_TYPE_CYLINDER: u32 = 3;
/// `COLLISION_TYPE_AABB`.
pub const COLLISION_TYPE_AABB: u32 = 4;
/// `COLLISION_TYPE_OBB`.
pub const COLLISION_TYPE_OBB: u32 = 5;

const HEADER: &[u8; 14] = b"AttributeData\0";

/// `CStaticCollisionData`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StaticCollisionData {
    /// `dwType` (`COLLISION_TYPE_*`).
    pub kind: u32,
    /// `szName` up to its first NUL.
    pub name: [u8; 32],
    pub position: [f32; 3],
    /// `fDimensions`; only the type's leading values are read, the rest stay 0.
    pub dimensions: [f32; 3],
    /// `quatRotation` (`x, y, z, w`).
    pub rotation: [f32; 4],
}

/// `THeightData`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeightData {
    pub name: [u8; 32],
    pub vertices: Vec<[f32; 3]>,
}

/// What `CAttributeData` holds after a successful load.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttributeData {
    pub collisions: Vec<StaticCollisionData>,
    pub heights: Vec<HeightData>,
    /// `m_fMaximizeRadius`: the largest `|coordinate| + 50` of the height
    /// meshes.
    pub maximize_radius: f32,
}

/// Why a file does not load. C++ reads unchecked past the end of a short
/// file; here that is [`AttributeError::Truncated`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeError {
    /// The header is not `AttributeData` (`OnLoad` returns `FALSE`).
    Header,
    Truncated,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], AttributeError> {
        let end = self.at.checked_add(n).filter(|end| *end <= self.data.len()).ok_or(AttributeError::Truncated)?;
        let out = &self.data[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u32(&mut self) -> Result<u32, AttributeError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, AttributeError> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn name(&mut self) -> Result<[u8; 32], AttributeError> {
        Ok(self.bytes(32)?.try_into().unwrap())
    }
}

/// `CAttributeData::OnLoad` on the file's bytes. The C++ compares the header
/// with `strcmp`, so only the bytes up to the first NUL of the file's 14
/// header bytes have to match.
pub fn parse_attribute_data(data: &[u8]) -> Result<AttributeData, AttributeError> {
    let mut r = Reader { data, at: 0 };
    let header = r.bytes(HEADER.len())?;
    let header = &header[..header.iter().position(|b| *b == 0).unwrap_or(header.len())];
    if header != &HEADER[..13] {
        return Err(AttributeError::Header);
    }
    let collision_count = r.u32()? as usize;
    let height_count = r.u32()? as usize;
    // Every record is at least 64 bytes; refuse counts the file cannot hold.
    if collision_count > data.len() / 64 || height_count > data.len() / 36 {
        return Err(AttributeError::Truncated);
    }
    let mut out = AttributeData::default();
    for _ in 0..collision_count {
        let mut c = StaticCollisionData { kind: r.u32()?, name: r.name()?, ..Default::default() };
        c.position = [r.f32()?, r.f32()?, r.f32()?];
        let dimensions = match c.kind {
            COLLISION_TYPE_PLANE | COLLISION_TYPE_CYLINDER => 2,
            COLLISION_TYPE_BOX | COLLISION_TYPE_AABB | COLLISION_TYPE_OBB => 3,
            COLLISION_TYPE_SPHERE => 1,
            _ => 0,
        };
        for d in &mut c.dimensions[..dimensions] {
            *d = r.f32()?;
        }
        c.rotation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        out.collisions.push(c);
    }
    for _ in 0..height_count {
        let name = r.name()?;
        let count = r.u32()? as usize;
        if count > (data.len() - r.at) / 12 {
            return Err(AttributeError::Truncated);
        }
        let mut vertices = Vec::with_capacity(count);
        for _ in 0..count {
            let v = [r.f32()?, r.f32()?, r.f32()?];
            for k in v {
                out.maximize_radius = out.maximize_radius.max(k.abs() + 50.0);
            }
            vertices.push(v);
        }
        out.heights.push(HeightData { name, vertices });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: u32, dims: &[f32]) -> Vec<u8> {
        let mut out = kind.to_le_bytes().to_vec();
        let mut name = [0u8; 32];
        name[..3].copy_from_slice(b"box");
        out.extend(name);
        for f in [1.0f32, 2.0, 3.0].iter().chain(dims).chain(&[0.0, 0.0, 0.0, 1.0]) {
            out.extend(f.to_le_bytes());
        }
        out
    }

    #[test]
    fn reads_collisions_and_heights() {
        let mut data = HEADER.to_vec();
        data.extend(2u32.to_le_bytes());
        data.extend(1u32.to_le_bytes());
        data.extend(record(COLLISION_TYPE_CYLINDER, &[40.0, 300.0]));
        data.extend(record(COLLISION_TYPE_OBB, &[10.0, 20.0, 30.0]));
        data.extend([0u8; 32]);
        data.extend(1u32.to_le_bytes());
        for f in [-100.0f32, 5.0, 7.0] {
            data.extend(f.to_le_bytes());
        }
        let parsed = parse_attribute_data(&data).unwrap();
        assert_eq!(parsed.collisions.len(), 2);
        assert_eq!(parsed.collisions[0].kind, COLLISION_TYPE_CYLINDER);
        assert_eq!(parsed.collisions[0].dimensions, [40.0, 300.0, 0.0]);
        assert_eq!(parsed.collisions[1].dimensions, [10.0, 20.0, 30.0]);
        assert_eq!(parsed.collisions[1].rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(parsed.heights[0].vertices, vec![[-100.0, 5.0, 7.0]]);
        assert_eq!(parsed.maximize_radius, 150.0);
        assert_eq!(parse_attribute_data(b"NotAttributes\0"), Err(AttributeError::Header));
        assert_eq!(parse_attribute_data(&data[..data.len() - 1]), Err(AttributeError::Truncated));
    }

    /// Every shipped `.mdatr` loads. `cargo test -p m2-formats -- --ignored
    /// mdatr` with the client in `bin/`.
    #[test]
    #[ignore]
    fn every_shipped_mdatr_loads() {
        let mut packs = crate::pack::PackManager::new("../../bin");
        crate::pack::load_index(&mut packs, "pack", "pack/Index.dev").expect("bin/pack");
        let mut counts = [0usize; 6];
        let mut files = 0;
        for name in packs.file_names().filter(|name| name.ends_with(b".mdatr")) {
            let data = packs.get(name).unwrap();
            let parsed = parse_attribute_data(&data)
                .unwrap_or_else(|e| panic!("{}: {e:?}", String::from_utf8_lossy(name)));
            for c in &parsed.collisions {
                counts[c.kind.min(5) as usize] += 1;
            }
            files += 1;
        }
        eprintln!("{files} files; plane/box/sphere/cylinder/aabb/obb = {counts:?}");
        assert!(files > 1000);
    }
}
