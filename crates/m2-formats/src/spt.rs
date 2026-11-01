//! SpeedTree `.spt` trees: port of the clean-room `CSpeedTreeRT`
//! (`neoeter/src/model/speedtree/SpeedTreeRTCompat.cpp`).
//!
//! The shipped `.spt` files only hold generation parameters for the
//! proprietary SpeedTreeRT generator. The C++ reimplementation reads just the
//! texture names and the tree size from them, picks a species archetype
//! ([`Shape`]) from the bark texture name and builds a deterministic tree of
//! tapered tubes and leaf cards ([`Spt::generate`]). When a baked geometry
//! cache (`treecache/<fnv1a32 of the .spt>.tree`, written from the original
//! SDK) exists, its geometry is used instead ([`load_baked`]).
//!
//! Everything here is what `CSpeedTreeWrapper` reads back through
//! `GetGeometry`, `GetTextures`, `GetBoundingBox` and `GetCollisionObject`.
//! Leaf cards are camera-facing quads recomputed whenever the geometry is
//! queried; [`leaf_billboards`] computes their corners for a tree position and
//! camera.

use std::fmt;

/// Default material (diffuse, ambient, specular, emissive RGB, power) that
/// `CSpeedTreeRT` returns for branches, leaves and fronds: nothing overrides
/// it.
pub const DEFAULT_MATERIAL: [f32; 13] = [1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.2, 0.2, 0.2, 0.0, 0.0, 0.0, 5.0];

/// `TreeCache::kMagic` (`MT2T` little-endian).
pub const TREE_CACHE_MAGIC: u32 = 0x4D54_3254;

/// Directories `CSpeedTreeRT::LoadBakedGeometry` searches, relative to the
/// working directory, in order; then `$MT2_TREECACHE/` ([`TREE_CACHE_ENV`]).
pub const TREE_CACHE_DIRS: [&str; 3] = ["treecache/", "../treecache/", "dist/treecache/"];

/// Environment variable naming the last cache directory to search.
pub const TREE_CACHE_ENV: &str = "MT2_TREECACHE";

/// `CSpeedTreeRT::ECollisionObjectType` values.
pub const CO_SPHERE: i32 = 0;
pub const CO_CYLINDER: i32 = 1;
pub const CO_BOX: i32 = 2;

/// `TreeCache::Fnv1a32`: the hash naming a tree's cache file.
pub fn fnv1a32(data: &[u8]) -> u32 {
    data.iter().fold(2_166_136_261, |h, &b| (h ^ u32::from(b)).wrapping_mul(16_777_619))
}

/// The cache file name for a hash, `%08x.tree`.
pub fn tree_cache_file_name(hash: u32) -> String {
    format!("{hash:08x}.tree")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `LoadTree` on an empty block.
    Empty,
    /// A cache file's magic is not [`TREE_CACHE_MAGIC`].
    BadMagic(u32),
    /// A cache file is for another `.spt`.
    HashMismatch { expected: u32, found: u32 },
    /// A cache file ends before the layout does.
    Truncated,
    /// A cache file has a count `LoadBakedGeometry` rejects.
    BadCount(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Empty => write!(f, "empty SPT block"),
            Error::BadMagic(m) => write!(f, "tree cache: bad magic {m:08x}"),
            Error::HashMismatch { expected, found } => {
                write!(f, "tree cache: hash {found:08x}, expected {expected:08x}")
            }
            Error::Truncated => write!(f, "tree cache: truncated"),
            Error::BadCount(what) => write!(f, "tree cache: bad {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// `CSpeedTreeRT::ETreeShape`: the archetype the generator builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Broadleaf,
    Birch,
    Conifer,
    Palm,
    Baobab,
    Small,
}

/// `CSpeedTreeRT::STextures`, as file names (base names, `#` suffix
/// removed). `None` is a null pointer in C++.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Textures {
    pub branch: Option<Vec<u8>>,
    pub composite: Option<Vec<u8>>,
    /// Never set: self-shadow maps are deliberately ignored.
    pub self_shadow: Option<Vec<u8>>,
    pub leaves: Vec<Vec<u8>>,
    pub fronds: Vec<Vec<u8>>,
}

/// What `CSpeedTreeRT::LoadTree` keeps of an `.spt` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Spt {
    /// [`fnv1a32`] of the file, naming its cache file.
    pub hash: u32,
    pub shape: Shape,
    pub textures: Textures,
    /// `GetTreeSize`; also `SetTreeSize` before [`Spt::generate`].
    pub size: f32,
    pub size_variance: f32,
}

/// `CSpeedTreeRT::SCollisionObj`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collision {
    /// [`CO_SPHERE`], [`CO_CYLINDER`] or [`CO_BOX`] (any value from a cache).
    pub kind: i32,
    pub position: [f32; 3],
    pub dimensions: [f32; 3],
}

/// One branch or frond vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    /// `unsigned long` in C++, always a 32-bit value.
    pub color: u32,
    pub tex_coords0: [f32; 2],
    pub tex_coords1: [f32; 2],
}

/// `SGeometry::SIndexed` for branches or fronds: one composited strip.
#[derive(Debug, Clone, PartialEq)]
pub struct Indexed {
    pub vertices: Vec<Vertex>,
    /// The whole strip. C++ reports its length as a `u16`, so the engine sees
    /// only [`Indexed::strip_indices`].
    pub strip: Vec<u16>,
    pub alpha_test: f32,
}

impl Indexed {
    fn empty(alpha_test: f32) -> Self {
        Indexed { vertices: Vec::new(), strip: Vec::new(), alpha_test }
    }

    /// `m_usNumStrips`: 1 when there is a strip.
    pub fn num_strips(&self) -> u16 {
        u16::from(!self.strip.is_empty())
    }

    /// `m_pStripLengths[0]`, the strip length truncated to `u16` as in C++.
    pub fn strip_length(&self) -> u16 {
        self.strip.len() as u16
    }

    /// The indices the engine copies (`m_pStrips[0]`, `strip_length()` long).
    pub fn strip_indices(&self) -> &[u16] {
        &self.strip[..usize::from(self.strip_length())]
    }

    /// `m_usVertexCount`.
    pub fn vertex_count(&self) -> u16 {
        self.vertices.len() as u16
    }
}

/// One leaf card of `SGeometry::m_sLeaves0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leaf {
    pub center: [f32; 3],
    pub color: u32,
    pub size: f32,
    pub half_width: f32,
    pub half_height: f32,
    /// Rotation of the card in the billboard plane, radians.
    pub angle: f32,
    /// Four `u, v` corners: bottom-left, bottom-right, top-right, top-left
    /// for generated leaves.
    pub tex_coords: [f32; 8],
    pub map_index: u8,
    pub cluster_index: u8,
}

/// The geometry `CSpeedTreeRT::GetGeometry` reports for a computed tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// `GetBoundingBox`: min xyz, max xyz.
    pub bounding_box: [f32; 6],
    pub collisions: Vec<Collision>,
    pub branches: Indexed,
    /// Empty for generated trees; then reported with LOD -1 and alpha 0.
    pub fronds: Indexed,
    /// `m_sLeaves0`; `m_sLeaves1` is always inactive.
    pub leaves: Vec<Leaf>,
    pub leaf_alpha_test: f32,
}

impl Geometry {
    /// `GetBranchTriangleCount`.
    pub fn branch_triangle_count(&self) -> u32 {
        let len = self.branches.strip_length();
        if len > 2 { u32::from(len - 2) } else { 0 }
    }

    /// `GetLeafTriangleCount` (the leaf count is reported as `u16`).
    pub fn leaf_triangle_count(&self) -> u32 {
        self.leaves.len() as u32 * 2
    }
}

/// The inputs of `CSpeedTreeWrapper::LoadTree` that shape the geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadParams {
    /// `Compute(nullptr, seed)`; the engine passes 1.
    pub seed: u32,
    /// `SetTreePosition` before `Compute`; the engine computes main trees at
    /// the origin and only moves instances.
    pub position: [f32; 3],
    /// `SetTreeSize(size, variance)` before `Compute`; the wrapper does it
    /// only when both are given as >= 0, which the engine never does.
    pub size: Option<(f32, f32)>,
}

impl Default for LoadParams {
    fn default() -> Self {
        LoadParams { seed: 1, position: [0.0; 3], size: None }
    }
}

/// A tree as `CSpeedTreeWrapper::LoadTree` sets it up.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub spt: Spt,
    pub geometry: Geometry,
    /// Whether the geometry came from the cache.
    pub baked: bool,
}

impl Tree {
    /// `LoadTree(block)`, `SetTreeSize` and `Compute`. `cache` is the
    /// tree's cache file ([`tree_cache_file_name`]) if one was found; an
    /// invalid one is ignored and the geometry generated, as in C++.
    pub fn load(spt: &[u8], cache: Option<&[u8]>, params: &LoadParams) -> Result<Tree, Error> {
        let mut parsed = Spt::parse_textures(spt)?;
        if let Some(mut geometry) = cache.and_then(|c| load_baked(c, parsed.hash).ok()) {
            // LoadTree returns before reading the size (GetTreeSize stays
            // 400), and Compute keeps the baked geometry; SetTreeSize still
            // overwrites the bounding box.
            if let Some((size, variance)) = params.size {
                parsed.size = size;
                parsed.size_variance = variance;
                if size > 0.0 {
                    let q = size * 0.25;
                    geometry.bounding_box = [-q, -q, 0.0, q, q, size];
                }
            }
            return Ok(Tree { spt: parsed, geometry, baked: true });
        }
        parsed.scan_size(spt);
        if let Some((size, variance)) = params.size {
            parsed.size = size;
            parsed.size_variance = variance;
        }
        let geometry = parsed.generate(params.seed, params.position);
        Ok(Tree { spt: parsed, geometry, baked: false })
    }
}

/// `std::max(a, b)`: `a < b ? b : a`, keeping C++'s NaN and signed-zero
/// behaviour.
fn cpp_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `static_cast<unsigned int>(float)` as x86-64 GCC compiles it (`cvttss2si`
/// to 64 bits, low half kept). Undefined in C++ for negative or huge values;
/// this mirrors the shipped build.
fn float_to_u32_x86(x: f32) -> u32 {
    if x.is_finite() && x.abs() < 9.223_372e18 { (x as i64) as u32 } else { 0 }
}

fn ascii_lower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// `ExtractBaseName`: the part after the last `/` or `\`.
fn base_name(path: &[u8]) -> &[u8] {
    match path.iter().rposition(|&b| b == b'/' || b == b'\\') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// Drops SpeedTree's texture-set suffix (`CompositeMapB1#N.tga` →
/// `CompositeMapB1.tga`): from the first `#` up to the last `.`.
fn strip_hash_suffix(tok: &[u8]) -> Vec<u8> {
    let Some(hash) = tok.iter().position(|&b| b == b'#') else {
        return tok.to_vec();
    };
    let mut out = tok[..hash].to_vec();
    if let Some(dot) = tok.iter().rposition(|&b| b == b'.')
        && dot > hash
    {
        out.extend_from_slice(&tok[dot..]);
    }
    out
}

/// Runs of at least four printable non-space bytes other than quotes, as
/// `LoadTree` scans them (`isprint` in the C locale).
fn tokens(data: &[u8]) -> Vec<&[u8]> {
    data.split(|&c| !(c.is_ascii_graphic() && c != b'"' && c != b'\''))
        .filter(|t| t.len() >= 4)
        .collect()
}

impl Spt {
    /// `CSpeedTreeRT::LoadTree(block)` without a cache: texture names, shape
    /// and size.
    pub fn parse(data: &[u8]) -> Result<Spt, Error> {
        let mut spt = Spt::parse_textures(data)?;
        spt.scan_size(data);
        Ok(spt)
    }

    /// The part of `LoadTree` before the cache lookup.
    fn parse_textures(data: &[u8]) -> Result<Spt, Error> {
        if data.is_empty() {
            return Err(Error::Empty);
        }

        let mut tex = Textures::default();
        for raw in tokens(data) {
            let tok = strip_hash_suffix(base_name(raw));
            let lower = ascii_lower(&tok);
            let has = |s: &[u8]| contains(&lower, s);

            if has(b"compositeshadowmap") || (has(b"shadow") && has(b".tga")) {
                // Ignored: the geometry has no matching self-shadow texcoords.
            } else if has(b"compositemap") || has(b"composite") {
                if tex.composite.is_none() {
                    tex.composite = Some(tok);
                }
            } else if has(b"bark")
                || (tex.branch.is_none() && has(b".tga") && !has(b"leaf") && !has(b"frond"))
            {
                if tex.branch.is_none() {
                    tex.branch = Some(tok);
                }
            } else if has(b"leaf") || has(b"leaves") {
                tex.leaves.push(tok);
            } else if has(b"frond") {
                tex.fronds.push(tok);
            }
        }

        let bark = ascii_lower(tex.branch.as_deref().unwrap_or_default());
        let has = |s: &[u8]| contains(&bark, s);
        let shape = if has(b"palm") || has(b"banana") || has(b"palmetto") {
            Shape::Palm
        } else if [&b"spruce"[..], b"cypress", b"pine", b"cedar", b"juniper", b"christmas"].iter().any(|s| has(s)) {
            Shape::Conifer
        } else if has(b"baobab") {
            Shape::Baobab
        } else if has(b"fern") || has(b"aloe") {
            Shape::Small
        } else if has(b"birch") {
            Shape::Birch
        } else {
            Shape::Broadleaf
        };

        Ok(Spt { hash: fnv1a32(data), shape, textures: tex, size: 400.0, size_variance: 0.0 })
    }

    /// Property 2001 (tree size, `D1 07 00 00` then an `f32`): the first
    /// value in (20, 4000) becomes `SetTreeSize(max(200, size), 0)`.
    fn scan_size(&mut self, data: &[u8]) {
        for w in data.windows(8) {
            if w[..4] == [0xD1, 0x07, 0x00, 0x00] {
                let size = f32::from_le_bytes([w[4], w[5], w[6], w[7]]);
                if size > 20.0 && size < 4000.0 {
                    self.size = cpp_max(200.0, size);
                    self.size_variance = 0.0;
                    break;
                }
            }
        }
    }

    /// `CSpeedTreeRT::GenerateGeometry` (via `Compute(nullptr, seed)`) with
    /// the tree at `position`; only x and y feed the random seed.
    pub fn generate(&self, seed: u32, position: [f32; 3]) -> Geometry {
        let h = if self.size > 1.0 { self.size } else { 400.0 };
        let seed = 0x9E37_79B9u32
            ^ seed
            ^ float_to_u32_x86(position[0] * 13.0).wrapping_mul(2_654_435_761)
            ^ float_to_u32_x86(position[1] * 7.0).wrapping_mul(40_503);
        let mut g = Generator { vertices: Vec::new(), strip: Vec::new(), leaves: Vec::new(), seed };

        match self.shape {
            Shape::Conifer => g.conifer(h),
            Shape::Palm => g.palm(h),
            Shape::Baobab => g.baobab(h),
            Shape::Small => g.small(h),
            Shape::Birch => g.birch(h),
            Shape::Broadleaf => g.broadleaf(h),
        }

        let trunk_radius = if self.shape == Shape::Baobab { 0.10 * h } else { 0.045 * h };
        Geometry {
            bounding_box: [-0.40 * h, -0.40 * h, 0.0, 0.40 * h, 0.40 * h, 1.0 * h],
            collisions: vec![Collision {
                kind: CO_CYLINDER,
                position: [0.0; 3],
                dimensions: [trunk_radius, 0.60 * h, 0.0],
            }],
            branches: Indexed { vertices: g.vertices, strip: g.strip, alpha_test: 1.0 },
            fronds: Indexed::empty(0.0),
            leaves: g.leaves,
            leaf_alpha_test: 128.0,
        }
    }
}

/// Leaf-cluster cells of the composite atlas (`kUvRects`: u0, v0, u1, v1).
const UV_RECTS: [[f32; 4]; 6] = [
    [0.00, 0.25, 0.25, 0.50],
    [0.25, 0.25, 0.50, 0.50],
    [0.00, 0.50, 0.25, 0.75],
    [0.25, 0.50, 0.50, 0.75],
    [0.00, 0.75, 0.25, 1.00],
    [0.25, 0.75, 0.50, 1.00],
];

const PI: f32 = std::f32::consts::PI;

/// The C++ literals `6.2831853f` and `1.5707963f`, which round to the same
/// `f32` as `TAU` and `FRAC_PI_2`; kept as written.
#[allow(clippy::approx_constant)]
const TWO_PI_LITERAL: f32 = 6.283_185_3;
#[allow(clippy::approx_constant)]
const HALF_PI_LITERAL: f32 = 1.570_796_3;

/// The lambdas and locals of `GenerateGeometry`.
struct Generator {
    vertices: Vec<Vertex>,
    strip: Vec<u16>,
    leaves: Vec<Leaf>,
    seed: u32,
}

fn normalized(dx: f32, dy: f32, dz: f32) -> [f32; 3] {
    let dl = ((dx * dx) + (dy * dy) + (dz * dz)).sqrt();
    [dx / dl, dy / dl, dz / dl]
}

impl Generator {
    /// `rnd`: an LCG, 24 bits per value in [0, 1).
    fn rnd(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.seed >> 8) & 0xFF_FFFF) as f32 / 16_777_216.0
    }

    /// `addTube`: a six-sided tube from `s` to `e`, stitched onto the strip.
    #[allow(clippy::too_many_arguments)]
    fn tube(&mut self, s: [f32; 3], e: [f32; 3], r0: f32, r1: f32, v_tile: f32, flip: bool) {
        let (mut dx, mut dy, mut dz) = (e[0] - s[0], e[1] - s[1], e[2] - s[2]);
        let len = ((dx * dx) + (dy * dy) + (dz * dz)).sqrt();
        if len < 1e-4 {
            return;
        }
        dx /= len;
        dy /= len;
        dz /= len;

        let (mut rx, ry, mut rz) = (0.0f32, 0.0f32, 1.0f32);
        if dz.abs() > 0.9 {
            rx = 1.0;
            rz = 0.0;
        }
        let mut right = [(ry * dz) - (rz * dy), (rz * dx) - (rx * dz), (rx * dy) - (ry * dx)];
        let rl = ((right[0] * right[0]) + (right[1] * right[1]) + (right[2] * right[2])).sqrt();
        if rl < 1e-6 {
            return;
        }
        for c in &mut right {
            *c /= rl;
        }
        let up = [
            (dy * right[2]) - (dz * right[1]),
            (dz * right[0]) - (dx * right[2]),
            (dx * right[1]) - (dy * right[0]),
        ];

        const SIDES: u16 = 6;
        let base = self.vertices.len() as u16;
        let sign = if flip { 1.0f32 } else { -1.0 };
        for i in 0..=SIDES {
            let a = sign * 2.0 * PI * f32::from(i) / f32::from(SIDES);
            let (ca, sa) = (a.cos(), a.sin());
            let o = [
                (right[0] * ca) + (up[0] * sa),
                (right[1] * ca) + (up[1] * sa),
                (right[2] * ca) + (up[2] * sa),
            ];
            let u = f32::from(i) / f32::from(SIDES);
            self.vertices.push(Vertex {
                position: [s[0] + (o[0] * r0), s[1] + (o[1] * r0), s[2] + (o[2] * r0)],
                color: 0xFFFF_FFFF,
                tex_coords0: [u, 0.0],
                tex_coords1: [u, 0.0],
            });
            self.vertices.push(Vertex {
                position: [e[0] + (o[0] * r1), e[1] + (o[1] * r1), e[2] + (o[2] * r1)],
                color: 0xFFFF_FFFF,
                tex_coords0: [u, v_tile],
                tex_coords1: [u, v_tile],
            });
        }

        let end = base.wrapping_add(2 * (SIDES + 1));
        if let Some(&last) = self.strip.last() {
            // Degenerate stitch.
            self.strip.extend_from_slice(&[last, base, base]);
            self.strip.extend(base.wrapping_add(1)..end);
        } else {
            self.strip.extend(base..end);
        }
    }

    /// `addTaperedTube`: both windings, since branches are backface culled.
    fn tapered_tube(&mut self, s: [f32; 3], e: [f32; 3], r0: f32, r1: f32, v_tile: f32) {
        self.tube(s, e, r0, r1, v_tile, false);
        self.tube(s, e, r0, r1, v_tile, true);
    }

    /// `addLeaf`. Callers draw the arguments' random numbers first: C++
    /// evaluates the arguments before the body's `rnd()`.
    fn leaf(&mut self, cx: f32, cy: f32, cz: f32, size: f32) {
        let angle = self.rnd() * TWO_PI_LITERAL;
        let index = self.leaves.len();
        let uv = UV_RECTS[index % 6];
        self.leaves.push(Leaf {
            center: [cx, cy, cz],
            color: 0xFFFF_FFFF,
            size,
            half_width: size * 0.5,
            half_height: size * 0.5,
            angle,
            tex_coords: [uv[0], uv[3], uv[2], uv[3], uv[2], uv[1], uv[0], uv[1]],
            map_index: 0,
            cluster_index: (index % 4) as u8,
        });
    }

    fn conifer(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.94 * h], 0.030 * h, 0.004 * h, 0.94 * h / 250.0);

        // Branch whorls, longer at the bottom.
        for t in 0..8 {
            let z = (0.22 + 0.09 * t as f32) * h;
            let f = 1.0 - (0.72 * (z / h));
            let len = 0.30 * h * f;
            for b in 0..5 {
                let a = (2.0 * PI * b as f32 / 5.0) + (0.7 * t as f32);
                let d = normalized(a.cos(), a.sin(), -0.40);
                self.tapered_tube(
                    [0.0, 0.0, z],
                    [d[0] * len, d[1] * len, z + (d[2] * len)],
                    0.006 * h,
                    0.0015 * h,
                    0.22 * h / 250.0,
                );
            }
        }

        // Cone of needle cards.
        for _ in 0..150 {
            let t = self.rnd();
            let z = (0.24 + 0.72 * t) * h;
            let rad = 0.32 * h * (1.0 - t) * (0.70 + 0.55 * self.rnd());
            let a = self.rnd() * TWO_PI_LITERAL;
            let size = (0.09 + 0.05 * self.rnd()) * h;
            self.leaf(a.cos() * rad, a.sin() * rad, z, size);
        }
    }

    fn palm(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.86 * h], 0.034 * h, 0.018 * h, 0.86 * h / 250.0);

        // Crown of drooping fronds made of leaf cards along arcs.
        for f in 0..9 {
            let a = (2.0 * PI * f as f32 / 9.0) + 0.3;
            for k in 0..4 {
                let t = 0.18 + (0.24 * k as f32);
                let rad = t * 0.46 * h;
                let z = (0.86 * h) + (0.10 * h * (1.0 - t)) - (0.22 * h * t * t);
                self.leaf(a.cos() * rad, a.sin() * rad, z, (0.22 - 0.03 * k as f32) * h);
            }
        }
        for _ in 0..8 {
            // GCC evaluates these arguments right to left.
            let z = (0.86 * h) + (0.05 * h * self.rnd());
            let y = (self.rnd() - 0.5) * 0.14 * h;
            let x = (self.rnd() - 0.5) * 0.14 * h;
            self.leaf(x, y, z, 0.16 * h);
        }
    }

    fn baobab(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.34 * h], 0.11 * h, 0.05 * h, 0.60 * h / 250.0);

        for i in 0..5 {
            let a = (2.0 * PI * i as f32 / 5.0) + 0.4;
            let h0 = (0.28 + 0.02 * i as f32) * h;
            let d = normalized(a.cos() * 0.80, a.sin() * 0.80, 0.60);
            let len = 0.32 * h;
            self.tapered_tube(
                [0.0, 0.0, h0],
                [d[0] * len, d[1] * len, h0 + (d[2] * len)],
                0.030 * h,
                0.008 * h,
                0.30 * h / 250.0,
            );
        }

        self.sphere_of_leaves(70, h, 0.38, 0.62, 0.22, 0.18, 0.08);
    }

    fn small(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.10 * h], 0.012 * h, 0.006 * h, 0.10 * h / 250.0);

        for i in 0..30 {
            let a = (2.0 * PI * i as f32 / 30.0) + (self.rnd() * 0.25);
            let rad = (0.10 + 0.24 * self.rnd()) * h;
            let z = (0.08 + 0.38 * self.rnd()) * h;
            let size = (0.18 + 0.12 * self.rnd()) * h;
            self.leaf(a.cos() * rad, a.sin() * rad, z, size);
        }
    }

    fn birch(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.60 * h], 0.026 * h, 0.010 * h, 0.60 * h / 250.0);

        for i in 0..4 {
            let a = (HALF_PI_LITERAL * i as f32) + 0.5;
            let h0 = (0.42 + 0.04 * i as f32) * h;
            let d = normalized(a.cos() * 0.55, a.sin() * 0.55, 0.85);
            let len = 0.24 * h;
            self.tapered_tube(
                [0.0, 0.0, h0],
                [d[0] * len, d[1] * len, h0 + (d[2] * len)],
                0.010 * h,
                0.004 * h,
                0.24 * h / 250.0,
            );
        }

        self.sphere_of_leaves(84, h, 0.25, 0.72, 0.26, 0.12, 0.06);
    }

    fn broadleaf(&mut self, h: f32) {
        self.tapered_tube([0.0; 3], [0.0, 0.0, 0.52 * h], 0.040 * h, 0.020 * h, 0.52 * h / 250.0);

        let mut tips = [[0.0f32; 3]; 5];
        for (i, tip) in tips.iter_mut().enumerate() {
            let a = (2.0 * PI * i as f32 / 5.0) + 0.35;
            let h0 = (0.36 + 0.035 * i as f32) * h;
            let d = normalized(a.cos() * 0.62, a.sin() * 0.62, 0.78);
            let len = 0.32 * h;
            *tip = [d[0] * len, d[1] * len, h0 + (d[2] * len)];
            self.tapered_tube([0.0, 0.0, h0], *tip, 0.016 * h, 0.005 * h, 0.32 * h / 250.0);
        }

        for i in 0..10 {
            let tip = tips[i % 5];
            let s = [tip[0] * 0.6, tip[1] * 0.6, tip[2] - (0.04 * h)];
            let a = (1.256_637_1 * i as f32) + 0.9;
            let d = normalized(a.cos() * 0.9, a.sin() * 0.9, 0.5);
            let len = 0.18 * h;
            self.tapered_tube(
                s,
                [s[0] + (d[0] * len), s[1] + (d[1] * len), s[2] + (d[2] * len)],
                0.006 * h,
                0.002 * h,
                0.18 * h / 250.0,
            );
        }

        self.sphere_of_leaves(96, h, 0.32, 0.70, 0.24, 0.15, 0.09);
    }

    /// The leaf loop shared by baobab, birch and broadleaf: `count` cards on
    /// a sphere of radius `radius * h` around height `z0 * h`, squashed to
    /// `z_scale * h`, sized `(size0 + size_var * rnd()) * h`.
    #[allow(clippy::too_many_arguments)]
    fn sphere_of_leaves(&mut self, count: usize, h: f32, radius: f32, z0: f32, z_scale: f32, size0: f32, size_var: f32) {
        for _ in 0..count {
            let a = self.rnd() * TWO_PI_LITERAL;
            let zc = (self.rnd() * 2.0) - 1.0;
            let rr = cpp_max(0.0, 1.0 - (zc * zc)).sqrt();
            let size = (size0 + size_var * self.rnd()) * h;
            self.leaf(
                radius * h * rr * a.cos(),
                radius * h * rr * a.sin(),
                (z0 * h) + (z_scale * h * zc),
                size,
            );
        }
    }
}

/// `CSpeedTreeRT::UpdateLeafBillboardsAt`: each leaf's four corner offsets
/// (top-left, top-right, bottom-right, bottom-left; `x, y, z, 0` each) facing
/// the camera at `camera` (`SetCamera`) for a tree at `tree_position`. The
/// engine adds them to the leaf centre.
pub fn leaf_billboards(leaves: &[Leaf], tree_position: [f32; 3], camera: [f32; 3]) -> Vec<[f32; 16]> {
    let mut v = [tree_position[0] - camera[0], tree_position[1] - camera[1], tree_position[2] - camera[2]];
    let vl = ((v[0] * v[0]) + (v[1] * v[1]) + (v[2] * v[2])).sqrt();
    if vl < 1e-4 {
        v = [0.0, 1.0, 0.0];
    } else {
        for c in &mut v {
            *c /= vl;
        }
    }
    let [vx, vy, vz] = v;

    // right = normalize(cross(worldUp, view))
    let mut right = [(0.0 * vz) - (1.0 * vy), (1.0 * vx) - (0.0 * vz), (0.0 * vy) - (0.0 * vx)];
    let mut rl = ((right[0] * right[0]) + (right[1] * right[1]) + (right[2] * right[2])).sqrt();
    if rl < 1e-4 {
        right = [1.0, 0.0, 0.0];
        rl = 1.0;
    }
    for c in &mut right {
        *c /= rl;
    }
    // up = cross(view, right)
    let up = [
        (vy * right[2]) - (vz * right[1]),
        (vz * right[0]) - (vx * right[2]),
        (vx * right[1]) - (vy * right[0]),
    ];

    leaves
        .iter()
        .map(|leaf| {
            let (ca, sa) = (leaf.angle.cos(), leaf.angle.sin());
            let r = [
                (right[0] * ca) + (up[0] * sa),
                (right[1] * ca) + (up[1] * sa),
                (right[2] * ca) + (up[2] * sa),
            ];
            let u = [
                (up[0] * ca) - (right[0] * sa),
                (up[1] * ca) - (right[1] * sa),
                (up[2] * ca) - (right[2] * sa),
            ];
            let (hw, hh) = (leaf.half_width, leaf.half_height);
            let mut c = [0.0f32; 16];
            for k in 0..3 {
                c[k] = (-r[k] * hw) + (u[k] * hh);
                c[4 + k] = (r[k] * hw) + (u[k] * hh);
                c[8 + k] = (r[k] * hw) - (u[k] * hh);
                c[12 + k] = (-r[k] * hw) - (u[k] * hh);
            }
            c
        })
        .collect()
}

/// Little-endian reader that fails instead of reading past the end.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if n > self.remaining() {
            return Err(Error::Truncated);
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32s<const N: usize>(&mut self) -> Result<[f32; N], Error> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
}

/// One indexed part of a cache file (branches or fronds), before the
/// "no vertices, no part" rule.
fn read_indexed(r: &mut Reader, default_alpha: f32) -> Result<Indexed, Error> {
    let vertex_count = r.u32()?;
    let num_strips = r.u32()?;
    r.u16()?; // lod
    let alpha = r.f32()?;
    let index_count = r.u32()?;

    if vertex_count > 0xFFFF || num_strips > 0xFFFF {
        return Err(Error::BadCount("vertex or strip count"));
    }
    if index_count > 0 && num_strips == 0 {
        return Err(Error::BadCount("strip count"));
    }
    // u16 indices plus 44 bytes per vertex (coords, colour, two UVs, normals).
    if index_count as usize * 2 + vertex_count as usize * 44 > r.remaining() {
        return Err(Error::Truncated);
    }

    // Strips are stitched into one with degenerate indices; the last strip
    // takes the remainder.
    let mut strip: Vec<u16> = Vec::with_capacity(index_count as usize + 8);
    let per_strip = index_count.checked_div(num_strips).unwrap_or(0);
    for s in 0..num_strips {
        let count = if s + 1 == num_strips { index_count - s * per_strip } else { per_strip };
        let mut indices = Vec::with_capacity(count as usize);
        for _ in 0..count {
            indices.push(r.u16()?);
        }
        let Some(&first) = indices.first() else { continue };
        if let Some(&last) = strip.last() {
            strip.extend_from_slice(&[last, first, first]);
            strip.extend_from_slice(&indices[1..]);
        } else {
            strip = indices;
        }
    }

    let n = vertex_count as usize;
    let mut vertices = vec![Vertex { position: [0.0; 3], color: 0, tex_coords0: [0.0; 2], tex_coords1: [0.0; 2] }; n];
    for v in &mut vertices {
        v.position = r.f32s()?;
    }
    for v in &mut vertices {
        v.color = r.u32()?;
    }
    for v in &mut vertices {
        v.tex_coords0 = r.f32s()?;
    }
    for v in &mut vertices {
        v.tex_coords1 = r.f32s()?;
    }
    // Normals, unused. C++ counts them with an `unsigned short` and never
    // finishes for more than 21845 vertices; we just skip them.
    r.take(n * 12)?;

    if n == 0 {
        // The part keeps its cleared state; its indices and alpha are dropped.
        return Ok(Indexed::empty(default_alpha));
    }
    Ok(Indexed { vertices, strip, alpha_test: alpha })
}

/// `CSpeedTreeRT::LoadBakedGeometry` (and `ValidateBakedGeometry`) on the
/// contents of a cache file for the `.spt` with hash `hash`. Billboards and
/// texture names after the leaf sets are not read; trailing data is ignored.
pub fn load_baked(data: &[u8], hash: u32) -> Result<Geometry, Error> {
    let mut r = Reader { data, pos: 0 };
    let magic = r.u32()?;
    if magic != TREE_CACHE_MAGIC {
        return Err(Error::BadMagic(magic));
    }
    let found = r.u32()?;
    if found != hash {
        return Err(Error::HashMismatch { expected: hash, found });
    }
    if r.remaining() < 28 {
        return Err(Error::Truncated);
    }
    let bounding_box = r.f32s()?;

    let collision_count = r.u32()?;
    if collision_count as usize > r.remaining() / 28 {
        return Err(Error::BadCount("collision count"));
    }
    let mut collisions = Vec::with_capacity(collision_count as usize);
    for _ in 0..collision_count {
        collisions.push(Collision { kind: r.i32()?, position: r.f32s()?, dimensions: r.f32s()? });
    }

    let branches = read_indexed(&mut r, 1.0)?;
    // GetGeometry reports fronds without vertices with alpha 0, the default.
    let fronds = read_indexed(&mut r, 0.0)?;

    let mut leaves = Vec::new();
    let mut leaf_alpha_test = 128.0;
    for set in 0..2 {
        let active = r.u8()? != 0;
        r.i32()?; // lod
        let alpha = r.f32()?;
        let count = r.u32()?;
        // 114 bytes per leaf: centre, colour, 16 corner floats, 8 UV floats
        // and two one-byte indices.
        if count > 0xFFFF {
            return Err(Error::BadCount("leaf count"));
        }
        let n = count as usize;
        if n * 114 > r.remaining() {
            return Err(Error::Truncated);
        }

        let mut centers = Vec::with_capacity(n);
        for _ in 0..n {
            centers.push(r.f32s::<3>()?);
        }
        let mut colors = Vec::with_capacity(n);
        for _ in 0..n {
            colors.push(r.u32()?);
        }
        let mut set_leaves = Vec::with_capacity(n);
        for (center, color) in centers.into_iter().zip(colors) {
            let c: [f32; 16] = r.f32s()?;
            let tex_coords = r.f32s()?;
            let map_index = r.u8()?;
            let cluster_index = r.u8()?;

            let (dx, dy, dz) = (c[4] - c[0], c[5] - c[1], c[6] - c[2]);
            let (ux, uy, uz) = (c[12] - c[0], c[13] - c[1], c[14] - c[2]);
            let w = ((dx * dx) + (dy * dy) + (dz * dz)).sqrt();
            let h = ((ux * ux) + (uy * uy) + (uz * uz)).sqrt();
            set_leaves.push(Leaf {
                center,
                color,
                size: cpp_max(w, h),
                half_width: w * 0.5,
                half_height: h * 0.5,
                angle: dz.atan2(dx),
                tex_coords,
                map_index,
                cluster_index,
            });
        }

        // Only an active, non-empty first set is used.
        if set == 0 && active && n > 0 {
            leaves = set_leaves;
            leaf_alpha_test = alpha;
        }
    }

    Ok(Geometry { bounding_box, collisions, branches, fronds, leaves, leaf_alpha_test })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_suffix() {
        assert_eq!(strip_hash_suffix(b"CompositeMapB1#N.tga"), b"CompositeMapB1.tga");
        assert_eq!(strip_hash_suffix(b"a.b#c"), b"a.b");
        assert_eq!(strip_hash_suffix(b"plain.tga"), b"plain.tga");
    }

    #[test]
    fn token_scan() {
        assert_eq!(tokens(b"ab\0abcd\x01\"x.tga\"  long_one"), [&b"abcd"[..], b"x.tga", b"long_one"]);
        assert_eq!(base_name(b"D:\\Tree\\\\Bark.tga"), b"Bark.tga");
        assert_eq!(base_name(b"a/b/"), b"");
    }

    #[test]
    fn float_cast_mirrors_x86() {
        assert_eq!(float_to_u32_x86(5.9), 5);
        assert_eq!(float_to_u32_x86(-1.0), 0xFFFF_FFFF);
        assert_eq!(float_to_u32_x86(4_294_967_296.0 + 512.0), 512);
        assert_eq!(float_to_u32_x86(f32::NAN), 0);
        assert_eq!(float_to_u32_x86(1e30), 0);
    }

    #[test]
    fn cpp_max_semantics() {
        assert_eq!(cpp_max(0.0, -0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(cpp_max(0.0, f32::NAN), 0.0);
        assert_eq!(cpp_max(1.0, 2.0), 2.0);
    }
}
