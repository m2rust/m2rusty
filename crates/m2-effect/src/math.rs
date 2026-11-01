//! The D3DX math the effect code uses, as `Platform/DirectXMathCompat.h`
//! implements it: row-major matrices with row vectors (`v * M`), single
//! precision, and the exact operation order of the C++ so results match bit
//! for bit.

/// `D3DXVECTOR3`.
pub type Vec3 = [f32; 3];

/// `D3DX_PI`.
pub const PI: f32 = std::f32::consts::PI;

/// `D3DXToRadian`.
pub fn to_radian(degree: f32) -> f32 {
    degree * (PI / 180.0f32)
}

/// `D3DXToDegree`.
pub fn to_degree(radian: f32) -> f32 {
    radian * (180.0f32 / PI)
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn neg(a: Vec3) -> Vec3 {
    [-a[0], -a[1], -a[2]]
}

/// `v * f` (and `f * v`).
pub fn scale(a: Vec3, f: f32) -> Vec3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `D3DXVec3LengthSq`.
pub fn length_sq(v: Vec3) -> f32 {
    (v[0] * v[0]) + (v[1] * v[1]) + (v[2] * v[2])
}

/// `D3DXVec3Length`.
pub fn length(v: Vec3) -> f32 {
    length_sq(v).sqrt()
}

/// `D3DXVec3Dot`.
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    (a[0] * b[0]) + (a[1] * b[1]) + (a[2] * b[2])
}

/// `D3DXVec3Cross`.
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [(a[1] * b[2]) - (a[2] * b[1]), (a[2] * b[0]) - (a[0] * b[2]), (a[0] * b[1]) - (a[1] * b[0])]
}

/// `D3DXVec3Normalize`: zero for (nearly) zero vectors.
pub fn normalize(v: Vec3) -> Vec3 {
    let len = length(v);
    if len > 1e-8f32 {
        let inv = 1.0f32 / len;
        [v[0] * inv, v[1] * inv, v[2] * inv]
    } else {
        [0.0; 3]
    }
}

/// `D3DXMATRIX`: `m[row][column]`, `_41.._43` is the translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    /// `D3DXMatrixIdentity`.
    pub const IDENTITY: Mat4 = Mat4 { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

    /// The translation row (`_41`, `_42`, `_43`).
    pub fn translation(&self) -> Vec3 {
        [self.m[3][0], self.m[3][1], self.m[3][2]]
    }

    pub fn set_translation(&mut self, t: Vec3) {
        self.m[3][0] = t[0];
        self.m[3][1] = t[1];
        self.m[3][2] = t[2];
    }

    /// The 16 floats in `_11, _12, … _44` order.
    pub fn to_array(&self) -> [f32; 16] {
        std::array::from_fn(|i| self.m[i / 4][i % 4])
    }

    pub fn from_array(a: [f32; 16]) -> Self {
        Self { m: std::array::from_fn(|r| std::array::from_fn(|c| a[r * 4 + c])) }
    }

    /// `D3DXMATRIX::operator*`.
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let mut out = [[0.0f32; 4]; 4];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (self.m[i][0] * rhs.m[0][j])
                    + (self.m[i][1] * rhs.m[1][j])
                    + (self.m[i][2] * rhs.m[2][j])
                    + (self.m[i][3] * rhs.m[3][j]);
            }
        }
        Mat4 { m: out }
    }

    /// `D3DXVec3TransformCoord`.
    pub fn transform_coord(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        let mut x = (v[0] * m[0][0]) + (v[1] * m[1][0]) + (v[2] * m[2][0]) + m[3][0];
        let mut y = (v[0] * m[0][1]) + (v[1] * m[1][1]) + (v[2] * m[2][1]) + m[3][1];
        let mut z = (v[0] * m[0][2]) + (v[1] * m[1][2]) + (v[2] * m[2][2]) + m[3][2];
        let w = (v[0] * m[0][3]) + (v[1] * m[1][3]) + (v[2] * m[2][3]) + m[3][3];
        if w.abs() > 1e-8f32 && w != 1.0f32 {
            let inv_w = 1.0f32 / w;
            x *= inv_w;
            y *= inv_w;
            z *= inv_w;
        }
        [x, y, z]
    }

    /// `D3DXVec3TransformNormal`.
    pub fn transform_normal(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        [
            (v[0] * m[0][0]) + (v[1] * m[1][0]) + (v[2] * m[2][0]),
            (v[0] * m[0][1]) + (v[1] * m[1][1]) + (v[2] * m[2][1]),
            (v[0] * m[0][2]) + (v[1] * m[1][2]) + (v[2] * m[2][2]),
        ]
    }

    /// `D3DXMatrixInverse`; `None` where it returns null (a determinant
    /// under 1e-12).
    pub fn inverse(&self) -> Option<Mat4> {
        let m = self.to_array();
        let s0 = (m[0] * m[5]) - (m[4] * m[1]);
        let s1 = (m[0] * m[6]) - (m[4] * m[2]);
        let s2 = (m[0] * m[7]) - (m[4] * m[3]);
        let s3 = (m[1] * m[6]) - (m[5] * m[2]);
        let s4 = (m[1] * m[7]) - (m[5] * m[3]);
        let s5 = (m[2] * m[7]) - (m[6] * m[3]);

        let c5 = (m[10] * m[15]) - (m[14] * m[11]);
        let c4 = (m[9] * m[15]) - (m[13] * m[11]);
        let c3 = (m[9] * m[14]) - (m[13] * m[10]);
        let c2 = (m[8] * m[15]) - (m[12] * m[11]);
        let c1 = (m[8] * m[14]) - (m[12] * m[10]);
        let c0 = (m[8] * m[13]) - (m[12] * m[9]);

        let det = (s0 * c5) - (s1 * c4) + (s2 * c3) + (s3 * c2) - (s4 * c1) + (s5 * c0);
        if det.abs() < 1e-12f32 {
            return None;
        }
        let inv = 1.0f32 / det;
        let r = [
            (m[5] * c5 - m[6] * c4 + m[7] * c3) * inv,
            (-m[1] * c5 + m[2] * c4 - m[3] * c3) * inv,
            (m[13] * s5 - m[14] * s4 + m[15] * s3) * inv,
            (-m[9] * s5 + m[10] * s4 - m[11] * s3) * inv,
            (-m[4] * c5 + m[6] * c2 - m[7] * c1) * inv,
            (m[0] * c5 - m[2] * c2 + m[3] * c1) * inv,
            (-m[12] * s5 + m[14] * s2 - m[15] * s1) * inv,
            (m[8] * s5 - m[10] * s2 + m[11] * s1) * inv,
            (m[4] * c4 - m[5] * c2 + m[7] * c0) * inv,
            (-m[0] * c4 + m[1] * c2 - m[3] * c0) * inv,
            (m[12] * s4 - m[13] * s2 + m[15] * s0) * inv,
            (-m[8] * s4 + m[9] * s2 - m[11] * s0) * inv,
            (-m[4] * c3 + m[5] * c1 - m[6] * c0) * inv,
            (m[0] * c3 - m[1] * c1 + m[2] * c0) * inv,
            (-m[12] * s3 + m[13] * s1 - m[14] * s0) * inv,
            (m[8] * s3 - m[9] * s1 + m[10] * s0) * inv,
        ];
        Some(Mat4::from_array(r))
    }

    /// `D3DXMatrixRotationX`.
    pub fn rotation_x(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        let mut r = Mat4::IDENTITY;
        r.m[1][1] = c;
        r.m[1][2] = s;
        r.m[2][1] = -s;
        r.m[2][2] = c;
        r
    }

    /// `D3DXMatrixRotationY`.
    pub fn rotation_y(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][2] = -s;
        r.m[2][0] = s;
        r.m[2][2] = c;
        r
    }

    /// `D3DXMatrixRotationZ`.
    pub fn rotation_z(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][1] = s;
        r.m[1][0] = -s;
        r.m[1][1] = c;
        r
    }

    /// `D3DXMatrixRotationAxis`.
    pub fn rotation_axis(axis: Vec3, angle: f32) -> Mat4 {
        let v = normalize(axis);
        let (s, c) = (angle.sin(), angle.cos());
        let t = 1.0f32 - c;
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = (t * v[0] * v[0]) + c;
        r.m[0][1] = (t * v[0] * v[1]) + (s * v[2]);
        r.m[0][2] = (t * v[0] * v[2]) - (s * v[1]);
        r.m[1][0] = (t * v[0] * v[1]) - (s * v[2]);
        r.m[1][1] = (t * v[1] * v[1]) + c;
        r.m[1][2] = (t * v[1] * v[2]) + (s * v[0]);
        r.m[2][0] = (t * v[0] * v[2]) + (s * v[1]);
        r.m[2][1] = (t * v[1] * v[2]) - (s * v[0]);
        r.m[2][2] = (t * v[2] * v[2]) + c;
        r
    }

    /// `D3DXMatrixRotationYawPitchRoll` (radians): roll, then pitch, then yaw.
    pub fn rotation_yaw_pitch_roll(yaw: f32, pitch: f32, roll: f32) -> Mat4 {
        Mat4::rotation_z(roll).mul(&Mat4::rotation_x(pitch)).mul(&Mat4::rotation_y(yaw))
    }

    /// `D3DXMatrixRotationQuaternion`.
    pub fn rotation_quaternion(q: &Quat) -> Mat4 {
        let xx = q.x * q.x;
        let yy = q.y * q.y;
        let zz = q.z * q.z;
        let xy = q.x * q.y;
        let xz = q.x * q.z;
        let yz = q.y * q.z;
        let wx = q.w * q.x;
        let wy = q.w * q.y;
        let wz = q.w * q.z;
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = 1.0f32 - (2.0f32 * (yy + zz));
        r.m[0][1] = 2.0f32 * (xy + wz);
        r.m[0][2] = 2.0f32 * (xz - wy);
        r.m[1][0] = 2.0f32 * (xy - wz);
        r.m[1][1] = 1.0f32 - (2.0f32 * (xx + zz));
        r.m[1][2] = 2.0f32 * (yz + wx);
        r.m[2][0] = 2.0f32 * (xz + wy);
        r.m[2][1] = 2.0f32 * (yz - wx);
        r.m[2][2] = 1.0f32 - (2.0f32 * (xx + yy));
        r
    }
}

/// `D3DXQUATERNION`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// `D3DXQuaternionRotationAxis`.
    pub fn rotation_axis(axis: Vec3, angle: f32) -> Quat {
        let v = normalize(axis);
        let half = angle * 0.5f32;
        let sin_half = half.sin();
        Quat { x: v[0] * sin_half, y: v[1] * sin_half, z: v[2] * sin_half, w: half.cos() }
    }

    /// `D3DXQuaternionConjugate`.
    pub fn conjugate(&self) -> Quat {
        Quat { x: -self.x, y: -self.y, z: -self.z, w: self.w }
    }

    /// `D3DXQuaternionMultiply(out, self, rhs)`.
    pub fn mul(&self, rhs: &Quat) -> Quat {
        let (q1, q2) = (self, rhs);
        Quat {
            x: (q2.w * q1.x) + (q2.x * q1.w) + (q2.y * q1.z) - (q2.z * q1.y),
            y: (q2.w * q1.y) - (q2.x * q1.z) + (q2.y * q1.w) + (q2.z * q1.x),
            z: (q2.w * q1.z) + (q2.x * q1.y) - (q2.y * q1.x) + (q2.z * q1.w),
            w: (q2.w * q1.w) - (q2.x * q1.x) - (q2.y * q1.y) - (q2.z * q1.z),
        }
    }
}

/// `SafeRotationNormalizedArc` (`Render/GrpMath.h`).
pub fn safe_rotation_normalized_arc(from: Vec3, to: Vec3) -> Quat {
    if from == to {
        return Quat::new(0.0, 0.0, 0.0, 1.0);
    }
    if from == neg(to) {
        return Quat::new(0.0, 0.0, 1.0, 0.0);
    }
    let c = cross(from, to);
    let d = dot(from, to);
    let s = ((1.0f32 + d) * 2.0f32).sqrt();
    Quat::new(c[0] / s, c[1] / s, c[2] / s, s * 0.5f32)
}

/// `static_cast<int>(float)` as x86-64 compiles it (`cvttss2si`): NaN and
/// out-of-range values give `i32::MIN`.
pub fn float_to_int(f: f32) -> i32 {
    if f.is_nan() || f >= 2147483648.0f32 || f < -2147483648.0f32 { i32::MIN } else { f as i32 }
}

/// `static_cast<DWORD>(float)` as gcc compiles it on x86-64: a 64-bit
/// truncating conversion whose low 32 bits are kept.
pub fn float_to_dword(f: f32) -> u32 {
    const TWO_POW_63: f32 = 9_223_372_036_854_775_808.0;
    if f.is_nan() || f.abs() >= TWO_POW_63 {
        0 // i64::MIN, whose low half is 0
    } else {
        f as i64 as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_inverse_and_mul() {
        let m = Mat4::rotation_yaw_pitch_roll(0.3, -0.2, 0.9);
        let inv = m.inverse().unwrap();
        let p = m.mul(&inv);
        for (i, row) in p.m.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn conversions_follow_x86() {
        assert_eq!(float_to_int(f32::NAN), i32::MIN);
        assert_eq!(float_to_int(3.9), 3);
        assert_eq!(float_to_int(-3.9), -3);
        assert_eq!(float_to_dword(-1.0), u32::MAX);
        assert_eq!(float_to_dword(300.7), 300);
    }

    #[test]
    fn normalize_zero() {
        assert_eq!(normalize([0.0, 0.0, 0.0]), [0.0; 3]);
        let n = normalize([3.0, 0.0, 4.0]);
        assert!((length(n) - 1.0).abs() < 1e-6);
    }
}
