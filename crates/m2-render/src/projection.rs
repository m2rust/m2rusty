//! Camera projections with the clip-space conventions of the original
//! renderer, expressed for Bevy's reverse-Z depth buffer.

use bevy::camera::{CameraProjection, Projection, SubCameraView};
use bevy::math::{Mat4, Vec3A, Vec4};

/// A fixed projection matrix. Depth is reverse-Z like every Bevy projection
/// (1 at the near plane, 0 at the far plane), so depth functions are mapped
/// by [`crate::CompareFunc::to_reverse_z_wgpu`].
#[derive(Clone, Debug, PartialEq)]
pub struct FixedFunctionProjection {
    clip_from_view: Mat4,
    far: f32,
}

impl FixedFunctionProjection {
    /// `D3DXMatrixPerspectiveFovRH`: vertical field of view in radians. The
    /// near plane is honoured exactly (the GL reference clips at about half
    /// the near distance because it loads D3D's `[0, 1]` depth matrix
    /// unchanged).
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        // Depth 1 at `near`, 0 at `far` (reverse-Z): ndc z = a + b / distance.
        let f = 1.0 / (fov_y * 0.5).tan();
        let a = near / (far - near);
        let b = near * far / (far - near);
        Self {
            clip_from_view: Mat4::from_cols(
                Vec4::new(f / aspect, 0.0, 0.0, 0.0),
                Vec4::new(0.0, f, 0.0, 0.0),
                Vec4::new(0.0, 0.0, a, -1.0),
                Vec4::new(0.0, 0.0, b, 0.0),
            ),
            far,
        }
    }

    /// The UI projection `CGraphicBase::SetOrtho2D` + the GL backend's
    /// "half-pixel" replacement: view-space x/y are pixels with the origin at
    /// the top left, y pointing down, and pixel centres on integers.
    pub fn orthographic_2d(width: f32, height: f32) -> Self {
        // glOrtho(-0.5, w - 0.5, h - 0.5, -0.5, -1000, 1000) with reversed depth:
        // x_ndc = 2x/w + 1/w - 1, y_ndc = -2y/h + (h - 1)/h, depth = z/2000 + 0.5.
        Self {
            clip_from_view: Mat4::from_cols(
                Vec4::new(2.0 / width, 0.0, 0.0, 0.0),
                Vec4::new(0.0, -2.0 / height, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0 / 2000.0, 0.0),
                Vec4::new(1.0 / width - 1.0, (height - 1.0) / height, 0.5, 1.0),
            ),
            far: 1000.0,
        }
    }

    pub fn into_projection(self) -> Projection {
        Projection::custom(self)
    }

    pub fn matrix(&self) -> Mat4 {
        self.clip_from_view
    }
}

impl CameraProjection for FixedFunctionProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.clip_from_view
    }

    fn get_clip_from_view_for_sub(&self, _sub_view: &SubCameraView) -> Mat4 {
        self.clip_from_view
    }

    fn update(&mut self, _width: f32, _height: f32) {}

    fn far(&self) -> f32 {
        self.far
    }

    fn get_frustum_corners(&self, _z_near: f32, _z_far: f32) -> [Vec3A; 8] {
        let view_from_clip = self.clip_from_view.inverse();
        let corner = |x: f32, y: f32, z: f32| {
            let p = view_from_clip * Vec4::new(x, y, z, 1.0);
            Vec3A::from_vec4(p / p.w)
        };
        // Reverse-Z: the near plane is at depth 1, the far plane at 0.
        [
            corner(1.0, -1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, 1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 0.0),
            corner(1.0, 1.0, 0.0),
            corner(-1.0, 1.0, 0.0),
            corner(-1.0, -1.0, 0.0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    fn ndc(m: Mat4, p: Vec3) -> Vec3 {
        let c = m * p.extend(1.0);
        c.truncate() / c.w
    }

    #[test]
    fn perspective_maps_near_to_one_and_far_to_zero() {
        let p = FixedFunctionProjection::perspective(60f32.to_radians(), 4.0 / 3.0, 0.5, 100.0);
        let m = p.matrix();
        assert!((ndc(m, Vec3::new(0.0, 0.0, -0.5)).z - 1.0).abs() < 1e-5);
        assert!(ndc(m, Vec3::new(0.0, 0.0, -100.0)).z.abs() < 1e-5);
        // Nearer is a larger depth value.
        assert!(ndc(m, Vec3::new(0.0, 0.0, -2.0)).z > ndc(m, Vec3::new(0.0, 0.0, -5.0)).z);
    }

    #[test]
    fn ortho_2d_puts_integer_coordinates_on_pixel_centres() {
        let m = FixedFunctionProjection::orthographic_2d(160.0, 120.0).matrix();
        // Pixel (0, 0) centre: NDC x = -1 + 1/160, y = 1 - 1/120.
        let p = ndc(m, Vec3::new(0.0, 0.0, -0.5));
        assert!((p.x - (-1.0 + 1.0 / 160.0)).abs() < 1e-6, "{p:?}");
        assert!((p.y - (1.0 - 1.0 / 120.0)).abs() < 1e-6, "{p:?}");
        let q = ndc(m, Vec3::new(159.0, 119.0, -0.5));
        assert!((q.x - (1.0 - 1.0 / 160.0)).abs() < 1e-6);
        assert!((q.y - (-1.0 + 1.0 / 120.0)).abs() < 1e-6);
        // Larger z (towards the viewer) is nearer.
        assert!(ndc(m, Vec3::new(0.0, 0.0, 0.0)).z > ndc(m, Vec3::new(0.0, 0.0, -1.0)).z);
    }

    #[test]
    fn frustum_corners_are_finite() {
        let p = FixedFunctionProjection::perspective(1.0, 1.0, 0.5, 10.0);
        for c in p.get_frustum_corners(0.5, 10.0) {
            assert!(c.is_finite());
        }
    }
}
