//! Controller tracking using bevy_xr_utils

use bevy::prelude::*;
use bevy_mod_xr::session::XrTracker;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

/// Marker for the pointer ray visualization
#[derive(Component)]
pub struct PointerRay;

/// Marker for the right controller visual
#[derive(Component)]
pub struct RightControllerVisual;

/// Marker for the left controller visual
#[derive(Component)]
pub struct LeftControllerVisual;

/// Resource to track pointer ray configuration
#[derive(Resource)]
pub struct PointerRayConfig {
    pub length: f32,
    pub color: Color,
    pub hover_color: Color,
}

impl Default for PointerRayConfig {
    fn default() -> Self {
        Self {
            length: 5.0,
            color: Color::srgba(0.0, 0.8, 1.0, 0.5),
            hover_color: Color::srgba(0.0, 1.0, 0.5, 0.7),
        }
    }
}

/// Spawns visual representations for VR controllers
pub fn spawn_controller_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<PointerRayConfig>,
) {
    let controller_mesh = meshes.add(Cuboid::new(0.04, 0.02, 0.1));

    let left_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.4, 0.9),
        emissive: LinearRgba::new(0.1, 0.2, 0.5, 1.0),
        ..default()
    });

    let right_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.4, 0.2),
        emissive: LinearRgba::new(0.5, 0.2, 0.1, 1.0),
        ..default()
    });

    commands.spawn((
        Mesh3d(controller_mesh.clone()),
        MeshMaterial3d(left_material),
        Transform::default(),
        XrTrackedLeftGrip,
        XrTracker,
        LeftControllerVisual,
    ));

    commands.spawn((
        Mesh3d(controller_mesh),
        MeshMaterial3d(right_material),
        Transform::default(),
        XrTrackedRightGrip,
        XrTracker,
        RightControllerVisual,
    ));

    let ray_mesh = meshes.add(Cylinder::new(0.002, config.length));
    let ray_material = materials.add(StandardMaterial {
        base_color: config.color,
        emissive: LinearRgba::new(0.0, 0.4, 0.5, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        Mesh3d(ray_mesh),
        MeshMaterial3d(ray_material),
        Transform::default(),
        PointerRay,
    ));
}

/// Updates the pointer ray to follow the right controller
pub fn update_pointer_ray(
    controller_q: Query<&Transform, (With<XrTrackedRightGrip>, Without<PointerRay>)>,
    mut ray_q: Query<&mut Transform, With<PointerRay>>,
    config: Res<PointerRayConfig>,
) {
    let mut controller_iter = controller_q.iter();
    let Some(controller) = controller_iter.next() else {
        return;
    };
    let mut ray_iter = ray_q.iter_mut();
    let Some(mut ray_transform) = ray_iter.next() else {
        return;
    };

    *ray_transform = *controller;
    ray_transform.rotation *= Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    ray_transform.translation += controller.forward() * (config.length / 2.0);
}
