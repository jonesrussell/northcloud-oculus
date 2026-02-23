//! northcloud-oculus — UML diagram VR viewer
//!
//! Renders a static (read-only) UML diagram (class boxes + edges as thin cuboids) in VR
//! via Bevy + bevy_mod_openxr. Optional debug cube at (0, 0, -2): set NORTHCLOUD_DEBUG_CUBE=0 or false to disable.
//!
//! Optional live feed from north-cloud Redis: set REDIS_ADDR, REDIS_CHANNELS (e.g. articles:crime,articles:mining).
//!
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

mod redis_feed;

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::sprite::Text2d;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

use redis_feed::{LiveFeedBuffer, RedisConnectionStatus, RedisFeedConfig, spawn_subscriber};

// --- Marker components (cleanup, future interaction) ---

#[derive(Component)]
struct DiagramNode;

#[derive(Component)]
struct DiagramEdge;

#[derive(Component)]
struct DebugCube;

#[derive(Component)]
struct LiveFeedPanel;

#[derive(Component)]
struct RedisStatusPanel;

/// 3D quad at Redis panel position; color shows connection status (visible in XR; Text2d is not)
#[derive(Component)]
struct RedisStatusQuad;

/// Material handles for Redis status colors so the 3D quad updates in VR
#[derive(Resource, Clone)]
struct RedisStatusMaterials {
    disabled: Handle<StandardMaterial>,
    connecting: Handle<StandardMaterial>,
    connected: Handle<StandardMaterial>,
    disconnected: Handle<StandardMaterial>,
}

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
    let live_feed_buffer = init_live_feed_buffer();

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
        .insert_resource(live_feed_buffer)
        .add_systems(Startup, (setup_diagram, setup_feed_panel, setup_redis_status_panel).chain())
        .add_systems(Update, (drain_redis_feed, update_feed_panel, update_redis_status_panel).chain())
        .run()
}

fn init_live_feed_buffer() -> LiveFeedBuffer {
    if let Some(config) = RedisFeedConfig::from_env() {
        eprintln!("[redis] REDIS_CHANNELS set, spawning subscriber to {} …", config.addr);
        if let Some(receiver) = spawn_subscriber(config.clone()) {
            eprintln!("[redis] Subscriber thread started, status will show Connecting then Connected/Disconnected");
            LiveFeedBuffer::new(receiver, config.max_items)
        } else {
            eprintln!("[redis] Subscriber thread failed to spawn, feed disabled");
            LiveFeedBuffer::disabled(config.max_items)
        }
    } else {
        eprintln!("[redis] REDIS_CHANNELS not set, feed disabled");
        LiveFeedBuffer::disabled(20)
    }
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

fn drain_redis_feed(mut buffer: ResMut<LiveFeedBuffer>) {
    buffer.drain_receiver();
}

fn update_feed_panel(
    buffer: Res<LiveFeedBuffer>,
    mut panel_query: Query<&mut Text2d, With<LiveFeedPanel>>,
) {
    let Some(mut text) = panel_query.iter_mut().next() else {
        return;
    };
    const MAX_LINES: usize = 10;
    const TITLE_LEN: usize = 50;
    let lines: Vec<String> = buffer
        .items
        .iter()
        .rev()
        .take(MAX_LINES)
        .map(|a| {
            let title = a.title.chars().take(TITLE_LEN).collect::<String>();
            let suffix = if a.title.len() > TITLE_LEN { "…" } else { "" };
            let q = a
                .quality_score
                .map(|s| format!(" q{}", s))
                .unwrap_or_default();
            format!("[{}] {}{} | {}", a.channel, title, suffix, q)
        })
        .collect();
    let content = if lines.is_empty() {
        "Live feed (set REDIS_CHANNELS to subscribe)".to_string()
    } else {
        lines.join("\n")
    };
    *text = Text2d::new(content);
}

fn setup_feed_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    eprintln!("[debug] setup_feed_panel running");
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        Text2d::new("Live feed (set REDIS_CHANNELS to subscribe)"),
        TextFont {
            font: font.into(),
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Transform::from_xyz(-0.8, 0.0, -2.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        LiveFeedPanel,
    ));
    eprintln!("[debug] LiveFeedPanel entity spawned at (-0.8, 0.0, -2.0)");
}

fn setup_redis_status_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[debug] setup_redis_status_panel running");
    let pos = Vec3::new(0.0, 0.55, -1.95);
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        Text2d::new("Redis: disconnected"),
        TextFont {
            font: font.into(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.3, 0.2)),
        Transform::from_xyz(pos.x, pos.y, pos.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        RedisStatusPanel,
    ));
    eprintln!("[debug] RedisStatusPanel entity spawned at {:?}", pos);

    // 3D status quad (visible in XR; Text2d only renders to desktop window)
    let status_materials = RedisStatusMaterials {
        disabled: materials.add(Color::srgb(0.9, 0.3, 0.2)),
        connecting: materials.add(Color::srgb(0.9, 0.8, 0.2)),
        connected: materials.add(Color::srgb(0.2, 0.85, 0.4)),
        disconnected: materials.add(Color::srgb(0.9, 0.3, 0.2)),
    };
    commands.insert_resource(status_materials.clone());
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 0.1, 0.02))),
        MeshMaterial3d(status_materials.disconnected.clone()),
        Transform::from_translation(pos),
        RedisStatusQuad,
    ));
    eprintln!("[debug] RedisStatusQuad spawned at {:?} (color = connection status in VR)", pos);
}

fn update_redis_status_panel(
    buffer: Res<LiveFeedBuffer>,
    materials: Res<RedisStatusMaterials>,
    mut text_query: Query<(&mut Text2d, &mut TextColor), With<RedisStatusPanel>>,
    mut quad_query: Query<&mut MeshMaterial3d<StandardMaterial>, With<RedisStatusQuad>>,
    mut did_log: Local<bool>,
) {
    let (label, c, material_handle) = match buffer.connection_status {
        RedisConnectionStatus::Disabled => (
            "Redis: disconnected",
            Color::srgb(0.9, 0.3, 0.2),
            materials.disabled.clone(),
        ),
        RedisConnectionStatus::Connecting => (
            "Redis: connecting…",
            Color::srgb(0.9, 0.8, 0.2),
            materials.connecting.clone(),
        ),
        RedisConnectionStatus::Connected => (
            "Redis: connected",
            Color::srgb(0.2, 0.85, 0.4),
            materials.connected.clone(),
        ),
        RedisConnectionStatus::Disconnected => (
            "Redis: disconnected",
            Color::srgb(0.9, 0.3, 0.2),
            materials.disconnected.clone(),
        ),
    };

    if let Some((mut text, mut color)) = text_query.iter_mut().next() {
        if !*did_log {
            eprintln!("[debug] update_redis_status_panel: RedisStatusPanel entity found, updating text");
            *did_log = true;
        }
        *text = Text2d::new(label.to_string());
        *color = TextColor(c);
    }
    for mut mesh_mat in quad_query.iter_mut() {
        *mesh_mat = MeshMaterial3d::<StandardMaterial>(material_handle.clone());
    }
}
