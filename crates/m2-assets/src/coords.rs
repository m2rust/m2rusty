//! The one conversion between Metin2 / Granny / SpeedTree space and Bevy
//! space. Every loader in this crate uses these functions and nothing else.
//!
//! # Source space
//!
//! Metin2 data (`.gr2` models and motions, `.spt` trees, map coordinates) is
//! **Z-up and left-handed** (Direct3D): `+X` east, `+Z` up, and, for the
//! frame to be left-handed, `+Y` pointing *south* (this is why the game's
//! in-world Y is negated for display). Triangles are front-facing when they
//! appear **clockwise**.
//!
//! # Target space
//!
//! Bevy is **Y-up and right-handed**: `+X` right, `+Y` up, `-Z` forward
//! (`Transform::forward()`), so `+Z` points to the viewer / south. Triangles
//! are front-facing when they appear **counter-clockwise**.
//!
//! # The mapping
//!
//! ```text
//! (x, y, z)_metin2  ->  (x, z, y)_bevy
//! ```
//!
//! i.e. swap Y and Z. East stays `+X`, up becomes `+Y`, south stays south
//! (`+Z`). A pure axis swap is a reflection (determinant -1), which is exactly
//! what turns a left-handed frame into a right-handed one without mirroring
//! the picture: a model looks the same on screen as in the original client.
//! Consequences, all implemented below:
//!
//! * positions, normals, directions, bone translations: `(x, z, y)`;
//! * unit quaternions `(x, y, z, w)`: rotations are pseudo-vectors, so the
//!   vector part is swapped *and negated*: `(-x, -z, -y, w)`;
//! * 3x3 / 4x4 matrices (column-vector form): `M' = S M S` with `S` the swap;
//! * triangle winding: **reversed** (`[a, b, c]` becomes `[a, c, b]`), because
//!   the reflection turns clockwise-front into counter-clockwise-back.
//!
//! A character modelled facing forward in Metin2 (towards `-Y`, north) faces
//! `-Z` in Bevy. The conversion contains no extra rotation, so a map loaded
//! with it is oriented the way the minimap shows it (north is `-Z`).
//!
//! Row-vector (D3D) matrices as stored by Granny (`v' = v * M`, translation in
//! the fourth row) are read with [`mat4_from_d3d`], which accounts for that.

use bevy::math::{Mat3, Mat4, Quat, Vec3};

/// The swap `(x, y, z) -> (x, z, y)`; its own inverse.
pub const SWAP_YZ: Mat3 = Mat3::from_cols(Vec3::X, Vec3::Z, Vec3::Y);

/// Position, direction or normal.
#[inline]
pub fn vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], v[1])
}

/// The inverse of [`vec3`] (the swap is its own inverse).
#[inline]
pub fn vec3_back(v: Vec3) -> [f32; 3] {
    [v.x, v.z, v.y]
}

/// Unit quaternion `(x, y, z, w)`.
#[inline]
pub fn quat(q: [f32; 4]) -> Quat {
    Quat::from_xyzw(-q[0], -q[2], -q[1], q[3])
}

/// Per-axis scale `(sx, sy, sz)`.
#[inline]
pub fn scale(s: [f32; 3]) -> Vec3 {
    Vec3::new(s[0], s[2], s[1])
}

/// A 3x3 matrix given as rows `m[row][col]` acting on column vectors.
pub fn mat3(m: [[f32; 3]; 3]) -> Mat3 {
    // Rows to columns.
    let a = Mat3::from_cols(
        Vec3::new(m[0][0], m[1][0], m[2][0]),
        Vec3::new(m[0][1], m[1][1], m[2][1]),
        Vec3::new(m[0][2], m[1][2], m[2][2]),
    );
    SWAP_YZ * a * SWAP_YZ
}

/// A Granny / Direct3D 4x4 matrix (`m[row][col]`, row vectors, translation in
/// `m[3][0..3]`) as a Bevy column-vector matrix in Bevy space.
pub fn mat4_from_d3d(m: [[f32; 4]; 4]) -> Mat4 {
    // glam's columns are the rows of the D3D matrix (see module docs).
    let a = Mat4::from_cols_array_2d(&m);
    let s = Mat4::from_mat3(SWAP_YZ);
    s * a * s
}

/// Reverses the winding of a triangle list in place.
///
/// # Panics
/// If `indices.len()` is not a multiple of 3.
pub fn flip_winding<T>(indices: &mut [T]) {
    assert!(indices.len() % 3 == 0, "triangle list length {}", indices.len());
    for tri in indices.chunks_exact_mut(3) {
        tri.swap(1, 2);
    }
}

/// Converts an axis-aligned box `[min x, y, z, max x, y, z]`.
pub fn aabb(b: [f32; 6]) -> (Vec3, Vec3) {
    (vec3([b[0], b[1], b[2]]), vec3([b[3], b[4], b[5]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < EPS
    }

    #[test]
    fn axes() {
        // East stays east, up becomes +Y, south stays south (+Z).
        assert_eq!(vec3([1.0, 0.0, 0.0]), Vec3::X);
        assert_eq!(vec3([0.0, 0.0, 1.0]), Vec3::Y);
        assert_eq!(vec3([0.0, 1.0, 0.0]), Vec3::Z);
        // North (-Y in Metin2) is Bevy's forward (-Z).
        assert_eq!(vec3([0.0, -1.0, 0.0]), Vec3::NEG_Z);
        assert_eq!(vec3_back(vec3([1.0, 2.0, 3.0])), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn handedness_is_flipped_once() {
        // Cross products: a left-handed frame has x cross y = -z.
        let (x, y, z) = (vec3([1.0, 0.0, 0.0]), vec3([0.0, 1.0, 0.0]), vec3([0.0, 0.0, 1.0]));
        assert!(close(x.cross(y), -z)); // Bevy x cross (Metin2 y) = -(Metin2 z).
        assert!(close(Vec3::X.cross(Vec3::Y), Vec3::Z));
        assert_eq!(SWAP_YZ.determinant(), -1.0);
    }

    /// Rotating a vector in Metin2 space and converting equals converting the
    /// vector and rotating it with the converted quaternion.
    #[test]
    fn quaternion_commutes_with_vectors() {
        // Matrix-from-quaternion in the source frame, column vectors, standard formula.
        fn rotate_src(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
            let r = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize() * Vec3::from(v);
            r.to_array()
        }
        let cases = [
            ([0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2], [1.0, 0.0, 0.0]),
            ([0.3, -0.5, 0.2, 0.8], [0.4, 1.5, -2.0]),
            ([0.5, 0.5, 0.5, 0.5], [3.0, -1.0, 2.0]),
            ([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 1.0]),
        ];
        for (q, v) in cases {
            let qn = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize();
            let q = qn.to_array();
            let src = rotate_src(q, v);
            let dst = quat(q).normalize() * vec3(v);
            // The rotation in the target frame turns the converted vector into
            // the converted result of the source rotation... with the sense of
            // rotation mirrored, which is what negating the vector part does.
            let expected = vec3(src);
            assert!(close(dst, expected), "{q:?} {v:?}: {dst:?} != {expected:?}");
        }
    }

    #[test]
    fn matrices_conjugate_by_the_swap() {
        // A D3D translation matrix: translation in the fourth row.
        let t = mat4_from_d3d([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 2.0, 3.0, 1.0]]);
        assert!(close(t.transform_point3(Vec3::ZERO), vec3([1.0, 2.0, 3.0])));
        // A D3D rotation about +Z (up) by 90 degrees in a left-handed frame:
        // row vectors, [cos sin; -sin cos].
        let (s, c) = (1.0f32, 0.0f32);
        let r = mat4_from_d3d([[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        // D3D: (1,0,0) * M = (c, s, 0) = (0, 1, 0) (east to south).
        assert!(close(r.transform_vector3(Vec3::X), vec3([0.0, 1.0, 0.0])));
        // Up stays up.
        assert!(close(r.transform_vector3(Vec3::Y), Vec3::Y));
    }

    #[test]
    fn mat3_matches_vec3() {
        let m = mat3([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 2.0]]);
        // Column-vector product of the source matrix with (1, 0, 0) is the first column (0, -1, 0).
        assert!(close(m * vec3([1.0, 0.0, 0.0]), vec3([0.0, -1.0, 0.0])));
        assert!(close(m * vec3([0.0, 0.0, 1.0]), vec3([0.0, 0.0, 2.0])));
    }

    #[test]
    fn winding_flips_front_faces() {
        // A front-facing (clockwise, left-handed) triangle in Metin2 space,
        // facing the camera at -Y... the outward normal by the left-handed
        // rule is the component-wise cross product.
        let a = [0.0, 0.0, 0.0];
        let b = [0.0, 0.0, 1.0];
        let c = [1.0, 0.0, 0.0];
        let n_src = (Vec3::from(b) - Vec3::from(a)).cross(Vec3::from(c) - Vec3::from(a));
        // Outward normal in the source frame (left-handed convention) is the cross product.
        let outward = vec3(n_src.to_array());
        // Same indices after conversion: the right-handed cross product points the other way.
        let (pa, pb, pc) = (vec3(a), vec3(b), vec3(c));
        let n_same = (pb - pa).cross(pc - pa);
        assert!(close(n_same, -outward));
        // Flipped indices agree with the converted outward normal.
        let mut tri = [pa, pb, pc];
        flip_winding(&mut tri);
        let n_flipped = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
        assert!(close(n_flipped, outward));
    }

    #[test]
    fn flip_winding_swaps_pairs() {
        let mut v = [0u32, 1, 2, 3, 4, 5];
        flip_winding(&mut v);
        assert_eq!(v, [0, 2, 1, 3, 5, 4]);
    }
}
