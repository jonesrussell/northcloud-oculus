//! OpenXR → Vulkan math: pose and FOV to view/projection matrices.
//!
//! Shared infrastructure for view-proj, diagram placement, debug cube,
//! locomotion, and future UI. OpenXR uses right-handed, +Y up; we output
//! matrices that work with Vulkan NDC (Y down, Z in [0, 1]).

use glam::{Mat4, Quat, Vec3};
use openxr as xr;

/// Builds a 4x4 model matrix from an OpenXR pose (position + orientation).
/// OpenXR: right-handed, +Y up. Result is column-major (glam default).
#[inline]
pub fn pose_to_matrix(pose: &xr::Posef) -> Mat4 {
    let p = pose.position;
    let o = pose.orientation;
    let t = Mat4::from_translation(Vec3::new(p.x, p.y, p.z));
    // OpenXR quat: (x, y, z, w). glam::Quat::from_xyzw is (x, y, z, w).
    let q = Quat::from_xyzw(o.x, o.y, o.z, o.w);
    t * Mat4::from_quat(q)
}

/// Inverts a 4x4 affine (rigid body) matrix. Use for view = inverse(pose_to_matrix(pose)).
#[inline]
pub fn matrix_inverse(m: Mat4) -> Mat4 {
    m.inverse()
}

/// Vulkan NDC correction: post-multiply with an OpenGL-style projection so that
/// NDC becomes Y down and Z in [0, 1]. Returns the correction matrix.
#[inline]
pub fn vulkan_ndc_correction() -> Mat4 {
    // [1 0 0 0]
    // [0 -1 0 0]
    // [0 0 0.5 0.5]
    // [0 0 0 1]
    Mat4::from_cols(
        glam::Vec4::new(1.0, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, -1.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 0.5, 0.0),
        glam::Vec4::new(0.0, 0.0, 0.5, 1.0),
    )
}

/// Builds an asymmetric perspective projection from OpenXR FOV (radians).
/// Near and far are positive distances. Result is OpenGL-style NDC; multiply
/// by vulkan_ndc_correction() to get Vulkan NDC, or use projection_from_fov_vulkan.
#[inline]
pub fn projection_from_fov(fov: &xr::Fovf, near: f32, far: f32) -> Mat4 {
    let tan_l = fov.angle_left.tan();
    let tan_r = fov.angle_right.tan();
    let tan_d = fov.angle_down.tan();
    let tan_u = fov.angle_up.tan();

    let w = tan_r - tan_l;
    let h = tan_u - tan_d;
    let d = far - near;

    // Column-major. OpenGL-style: Y up, NDC z in [-1, 1].
    let c0 = glam::Vec4::new(2.0 / w, 0.0, (tan_r + tan_l) / w, 0.0);
    let c1 = glam::Vec4::new(0.0, 2.0 / h, (tan_u + tan_d) / h, 0.0);
    let c2 = glam::Vec4::new(0.0, 0.0, -(far + near) / d, -1.0);
    let c3 = glam::Vec4::new(0.0, 0.0, -(2.0 * far * near) / d, 0.0);
    Mat4::from_cols(c0, c1, c2, c3)
}

/// Projection from OpenXR FOV with Vulkan NDC baked in (Y down, Z in [0, 1]).
#[inline]
pub fn projection_from_fov_vulkan(fov: &xr::Fovf, near: f32, far: f32) -> Mat4 {
    vulkan_ndc_correction() * projection_from_fov(fov, near, far)
}

/// View matrix from eye pose: view = inverse(pose_to_matrix(pose)).
#[inline]
pub fn view_from_pose(pose: &xr::Posef) -> Mat4 {
    matrix_inverse(pose_to_matrix(pose))
}

/// Full view-projection for one eye: (projection * view) for Vulkan.
#[inline]
pub fn view_proj_vulkan(pose: &xr::Posef, fov: &xr::Fovf, near: f32, far: f32) -> Mat4 {
    projection_from_fov_vulkan(fov, near, far) * view_from_pose(pose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_ndc_correction_is_4x4() {
        let c = vulkan_ndc_correction();
        assert_eq!(c.x_axis.x, 1.0);
        assert_eq!(c.y_axis.y, -1.0);
        assert_eq!(c.z_axis.z, 0.5);
        assert_eq!(c.w_axis.z, 0.5);
        assert_eq!(c.w_axis.w, 1.0);
    }

    #[test]
    fn pose_identity_gives_translation_identity() {
        let pose = xr::Posef::IDENTITY;
        let m = pose_to_matrix(&pose);
        let v = glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
        let out = m * v;
        assert!((out.x.abs() + out.y.abs() + out.z.abs()) < 1e-6);
    }
}
