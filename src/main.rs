//! northcloud-oculus — UML diagram VR viewer
//!
//! Renders a static (read-only) UML diagram (class boxes + edges as thin cuboids) in VR
//! via Bevy + bevy_mod_openxr. Optional debug cube at (0, 0, -2): set NORTHCLOUD_DEBUG_CUBE=0 or false to disable.
//!
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

// --- Marker components (cleanup, future interaction) ---

#[derive(Component)]
struct DiagramNode;

#[derive(Component)]
struct DiagramEdge;

#[derive(Component)]
struct DebugCube;

// --- Diagram model (same as prior Vulkan version) ---

#[derive(Clone, Debug)]
struct Node {
    #[allow(dead_code)]
    id: u32,
    pos: Vec3,
    size: Vec2,
    color: Vec3,
}

#[derive(Clone, Debug)]
struct Edge {
    from: usize,
    to: usize,
}

fn sample_diagram() -> (Vec<Node>, Vec<Edge>) {
    let nodes = vec![
        Node {
            id: 0,
            pos: Vec3::new(-0.5, 0.2, -2.0),
            size: Vec2::new(0.4, 0.3),
            color: Vec3::new(0.2, 0.4, 0.9),
        },
        Node {
            id: 1,
            pos: Vec3::new(0.0, -0.2, -2.0),
            size: Vec2::new(0.35, 0.25),
            color: Vec3::new(0.9, 0.35, 0.2),
        },
        Node {
            id: 2,
            pos: Vec3::new(0.6, 0.15, -2.0),
            size: Vec2::new(0.4, 0.28),
            color: Vec3::new(0.2, 0.75, 0.4),
        },
    ];
    let edges = vec![Edge { from: 0, to: 1 }, Edge { from: 1, to: 2 }];
    (nodes, edges)
}

fn main() -> AppExit {
    App::new()
        .add_plugins(add_xr_plugins(
            DefaultPlugins.build().disable::<PipelinedRenderingPlugin>(),
        ))
        .insert_resource(OxrSessionConfig {
            blend_mode_preference: vec![
                EnvironmentBlendMode::ALPHA_BLEND,
                EnvironmentBlendMode::ADDITIVE,
                EnvironmentBlendMode::OPAQUE,
            ],
            ..default()
        })
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup_diagram)
        .run()
}

fn setup_diagram(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (nodes, edges) = sample_diagram();
    const RIBBON_HALF_WIDTH: f32 = 0.008;

    // 1. Debug cube at (0, 0, -2)
    let draw_cube = std::env::var("NORTHCLOUD_DEBUG_CUBE")
        .map(|v| v != "0" && v != "false")
        .unwrap_or(true);
    if draw_cube {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.2, 0.2, 0.2))),
            MeshMaterial3d(materials.add(Color::srgb(0.0, 0.8, 0.2))),
            Transform::from_xyz(0.0, 0.0, -2.0),
            DebugCube,
        ));
    }

    // 2. Nodes (quads in XY plane at node.pos)
    for node in &nodes {
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(node.size.x, node.size.y))),
            MeshMaterial3d(materials.add(Color::srgb(node.color.x, node.color.y, node.color.z))),
            Transform::from_translation(node.pos)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            DiagramNode,
        ));
    }

    // 3. Edges (thin cuboids between node centers)
    let gray = materials.add(Color::srgb(0.5, 0.5, 0.5));
    for edge in &edges {
        let a = nodes[edge.from].pos;
        let b = nodes[edge.to].pos;
        let d = b - a;
        let length = d.length().max(1e-6);
        let center = (a + b) * 0.5;
        let dir = d / length;
        let half_thick = RIBBON_HALF_WIDTH * 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(half_thick, length, half_thick))),
            MeshMaterial3d(gray.clone()),
            Transform::from_translation(center)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
            DiagramEdge,
        ));
    }

    // 4. Light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
}
