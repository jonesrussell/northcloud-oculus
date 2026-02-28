//! Raycast system for VR interaction
//!
//! Simple AABB-based raycasting for VR interaction.

use bevy::prelude::*;
use bevy_xr_utils::tracking_utils::XrTrackedRightGrip;

use super::{PointerRay, PointerRayConfig};

/// Resource storing the current raycast hit, if any
#[derive(Resource, Default)]
pub struct RaycastHit {
    pub entity: Option<Entity>,
    pub point: Option<Vec3>,
    pub distance: Option<f32>,
}

/// Marker component for entities that can be raycast against
#[derive(Component)]
pub struct RaycastTarget;

/// AABB for simple raycast testing
#[derive(Component)]
pub struct RaycastBounds {
    pub half_extents: Vec3,
}

impl Default for RaycastBounds {
    fn default() -> Self {
        Self {
            half_extents: Vec3::splat(0.1),
        }
    }
}

/// System that performs simple AABB raycasting from the right controller
pub fn perform_raycast(
    controller_q: Query<&Transform, With<XrTrackedRightGrip>>,
    targets: Query<(Entity, &Transform, &RaycastBounds), With<RaycastTarget>>,
    mut hit: ResMut<RaycastHit>,
    config: Res<PointerRayConfig>,
) {
    *hit = RaycastHit::default();

    let mut controller_iter = controller_q.iter();
    let Some(controller) = controller_iter.next() else {
        return;
    };

    let ray_origin = controller.translation;
    let ray_dir = controller.forward().normalize();

    let mut closest_hit: Option<(Entity, Vec3, f32)> = None;

    for (entity, target_transform, bounds) in targets.iter() {
        if let Some((hit_point, distance)) = ray_aabb_intersection(
            ray_origin,
            ray_dir,
            target_transform.translation,
            bounds.half_extents * target_transform.scale,
        ) {
            if distance <= config.length {
                if closest_hit.is_none() || distance < closest_hit.as_ref().unwrap().2 {
                    closest_hit = Some((entity, hit_point, distance));
                }
            }
        }
    }

    if let Some((entity, point, distance)) = closest_hit {
        hit.entity = Some(entity);
        hit.point = Some(point);
        hit.distance = Some(distance);
    }
}

/// Simple ray-AABB intersection test using the slab method.
/// Handles rays parallel to slab planes by checking if the origin is within the slab.
fn ray_aabb_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    aabb_center: Vec3,
    half_extents: Vec3,
) -> Option<(Vec3, f32)> {
    let min = aabb_center - half_extents;
    let max = aabb_center + half_extents;

    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;

    let dirs = [ray_dir.x, ray_dir.y, ray_dir.z];
    let origins = [ray_origin.x, ray_origin.y, ray_origin.z];
    let mins = [min.x, min.y, min.z];
    let maxs = [max.x, max.y, max.z];

    for i in 0..3 {
        if dirs[i].abs() < 1e-6 {
            // Ray is parallel to this slab. Check if origin is within the slab.
            if origins[i] < mins[i] || origins[i] > maxs[i] {
                return None;
            }
            // Origin is within slab; this axis doesn't constrain t.
        } else {
            let inv_d = 1.0 / dirs[i];
            let mut t1 = (mins[i] - origins[i]) * inv_d;
            let mut t2 = (maxs[i] - origins[i]) * inv_d;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            tmin = tmin.max(t1);
            tmax = tmax.min(t2);

            if tmin > tmax {
                return None;
            }
        }
    }

    // tmax < 0 means the AABB is entirely behind the ray origin
    if tmax < 0.0 {
        return None;
    }

    let t = if tmin < 0.0 { tmax } else { tmin };
    if t < 0.0 {
        return None;
    }

    let hit_point = ray_origin + ray_dir * t;
    Some((hit_point, t))
}

/// Updates the pointer ray appearance based on hit state
pub fn update_ray_appearance(
    hit: Res<RaycastHit>,
    config: Res<PointerRayConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ray_q: Query<&MeshMaterial3d<StandardMaterial>, With<PointerRay>>,
) {
    let mut ray_iter = ray_q.iter();
    let Some(ray_material) = ray_iter.next() else {
        return;
    };

    if let Some(material) = materials.get_mut(&ray_material.0) {
        material.base_color = if hit.entity.is_some() {
            config.hover_color
        } else {
            config.color
        };
    }
}
