//! OpenXR → Vulkan math: pose and FOV to view/projection matrices.
//!
//! Shared infrastructure for view-projection and pose matrices.
//! Used by the diagram renderer and debug cube. OpenXR uses right-handed,
//! +Y up; we output matrices that work with Vulkan NDC (Y down, Z in [0, 1]).

use glam::{Mat4, Quat, Vec3, Vec4};
use openxr as xr;

/// Builds a 4x4 model matrix from an OpenXR pose (position + orientation).
/// OpenXR: right-handed, +Y up. Result is column-major (glam default).
#[inline]
pub fn pose_to_matrix(pose: &xr::Posef) -> Mat4 {
    let p = pose.position;
    let o = pose.orientation;
    // OpenXR quat: (x, y, z, w). glam::Quat::from_xyzw is (x, y, z, w).
    Mat4::from_rotation_translation(
        Quat::from_xyzw(o.x, o.y, o.z, o.w),
        Vec3::new(p.x, p.y, p.z),
    )
}

/// Vulkan NDC correction matrix. Pre-multiply with an OpenGL-style
/// projection to convert Y up, Z [-1,1] to Vulkan's Y down, Z [0,1].
/// Usage: `vulkan_ndc_correction() * opengl_projection`
#[inline]
fn vulkan_ndc_correction() -> Mat4 {
    // [1  0   0   0]
    // [0 -1   0   0]
    // [0  0  0.5  0]
    // [0  0  0.5  1]
    Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.5, 0.0),
        Vec4::new(0.0, 0.0, 0.5, 1.0),
    )
}

/// Builds an asymmetric perspective projection from OpenXR FOV (radians).
/// Near and far are positive distances. Result is OpenGL-style NDC; multiply
/// by `vulkan_ndc_correction()` to get Vulkan NDC, or use `projection_from_fov_vulkan`.
#[inline]
fn projection_from_fov(fov: &xr::Fovf, near: f32, far: f32) -> Mat4 {
    debug_assert!(
        near > 0.0 && far > near,
        "projection_from_fov: need 0 < near < far (near={near}, far={far})"
    );

    let tan_l = fov.angle_left.tan();
    let tan_r = fov.angle_right.tan();
    let tan_d = fov.angle_down.tan();
    let tan_u = fov.angle_up.tan();

    let w = tan_r - tan_l;
    let h = tan_u - tan_d;
    let d = far - near;

    // Column-major (from_cols). OpenGL-style: Y up, NDC z in [-1, 1].
    // Off-axis terms go in column 2 (c2.x, c2.y), NOT column 0/1 row 2.
    let c0 = Vec4::new(2.0 / w, 0.0, 0.0, 0.0);
    let c1 = Vec4::new(0.0, 2.0 / h, 0.0, 0.0);
    let c2 = Vec4::new(
        (tan_r + tan_l) / w,
        (tan_u + tan_d) / h,
        -(far + near) / d,
        -1.0,
    );
    let c3 = Vec4::new(0.0, 0.0, -(2.0 * far * near) / d, 0.0);
    Mat4::from_cols(c0, c1, c2, c3)
}

/// Projection from OpenXR FOV with Vulkan NDC baked in (Y down, Z in [0, 1]).
#[inline]
pub fn projection_from_fov_vulkan(fov: &xr::Fovf, near: f32, far: f32) -> Mat4 {
    vulkan_ndc_correction() * projection_from_fov(fov, near, far)
}

/// View matrix from eye pose: view = inverse(pose_to_matrix(pose)).
#[inline]
fn view_from_pose(pose: &xr::Posef) -> Mat4 {
    pose_to_matrix(pose).inverse()
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
        let v = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let out = m * v;
        assert!((out.x.abs() + out.y.abs() + out.z.abs()) < 1e-6);
    }

    #[test]
    fn projection_off_axis_terms_in_column_2() {
        // Asymmetric FOV: off-axis terms must land in column 2 (c2.x, c2.y),
        // not in column 0 row 2 / column 1 row 2 (which would be transposed).
        let fov = xr::Fovf {
            angle_left: -0.8,
            angle_right: 1.0,
            angle_down: -0.7,
            angle_up: 0.9,
        };
        let m = projection_from_fov(&fov, 0.01, 100.0);
        // Column 0 row 2 and column 1 row 2 must be zero.
        assert_eq!(m.x_axis.z, 0.0, "c0.z should be 0, got {}", m.x_axis.z);
        assert_eq!(m.y_axis.z, 0.0, "c1.z should be 0, got {}", m.y_axis.z);
        // Column 2 row 0 and column 2 row 1 should be non-zero for asymmetric FOV.
        assert!(
            m.z_axis.x.abs() > 0.01,
            "c2.x (off-axis) should be non-zero"
        );
        assert!(
            m.z_axis.y.abs() > 0.01,
            "c2.y (off-axis) should be non-zero"
        );
    }
}
