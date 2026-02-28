//! northcloud-oculus — VR-native observability cockpit
//!
//! Renders world-space UI panels with egui content in VR via Bevy + bevy_mod_openxr.
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_egui::EguiPlugin;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

use northcloud_oculus::data::DataIngestionPlugin;
use northcloud_oculus::interaction::InteractionPlugin;
use northcloud_oculus::node_marker::NodeMarkerPlugin;
use northcloud_oculus::panels::PanelsPlugin;
use northcloud_oculus::world_panel::{
    configure_vr_egui_style, draw_panel_ui, spawn_world_panel, EguiPanel, WorldPanel,
    WorldPanelDefaults, WorldPanelParams, WorldPanelPlugin,
};

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
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldPanelPlugin)
        .add_plugins(InteractionPlugin)
        .add_plugins(NodeMarkerPlugin)
        .add_plugins(PanelsPlugin)
        .add_plugins(DataIngestionPlugin)
        .insert_resource(OxrSessionConfig {
            blend_mode_preference: vec![
                EnvironmentBlendMode::ALPHA_BLEND,
                EnvironmentBlendMode::ADDITIVE,
                EnvironmentBlendMode::OPAQUE,
            ],
            ..default()
        })
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (setup_diagram, setup_demo_panel, setup_demo_markers))
        .add_systems(Update, demo_panel_ui)
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

fn setup_demo_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    defaults: Res<WorldPanelDefaults>,
) {
    let panel = spawn_world_panel(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        &defaults,
        WorldPanelParams {
            size: Vec2::new(0.8, 0.6),
            transform: Transform::from_xyz(-0.6, 1.2, -1.5)
                .looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
            ..default()
        },
    );

    commands.entity(panel).insert(EguiPanel);
}

fn demo_panel_ui(
    mut egui_contexts: bevy_egui::EguiContexts,
    panels: Query<&WorldPanel, With<EguiPanel>>,
    time: Res<Time>,
) {
    for panel in panels.iter() {
        draw_panel_ui(&mut egui_contexts, panel, |ctx| {
            configure_vr_egui_style(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Northcloud Oculus");
                ui.separator();
                ui.label("VR Observability Cockpit");
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.colored_label(egui::Color32::GREEN, "Connected");
                });

                ui.horizontal(|ui| {
                    ui.label("Uptime:");
                    ui.label(format!("{:.1}s", time.elapsed_secs()));
                });

                ui.add_space(16.0);
                ui.label("This panel is rendered to a texture");
                ui.label("and displayed in 3D world space!");
            });
        });
    }
}

use bevy_egui::egui;

use northcloud_oculus::node_marker::{spawn_node_marker, NodeHealth, NodeMarkerMaterials};

fn setup_demo_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    marker_materials: Option<Res<NodeMarkerMaterials>>,
) {
    let Some(materials) = marker_materials else {
        return;
    };

    let demo_nodes = [
        ("node-1", Vec3::new(-0.3, 0.8, -1.5), NodeHealth::Healthy),
        ("node-2", Vec3::new(0.0, 0.9, -1.6), NodeHealth::Warning),
        ("node-3", Vec3::new(0.3, 0.7, -1.4), NodeHealth::Critical),
        ("node-4", Vec3::new(0.5, 1.0, -1.7), NodeHealth::Healthy),
    ];

    for (id, pos, health) in demo_nodes {
        spawn_node_marker(&mut commands, &mut meshes, &materials, id, pos, health, 0.03);
    }
}
