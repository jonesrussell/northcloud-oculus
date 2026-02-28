//! northcloud-oculus — UML diagram VR viewer
//!
//! Renders a static (read-only) UML diagram (class boxes + edges as thin cuboids) in VR
//! via Bevy + bevy_mod_openxr. Optional debug cube at (0, 0, -2): set NORTHCLOUD_DEBUG_CUBE=0 or false to disable.
//!
//! Optional live feed from north-cloud Redis: set REDIS_ADDR, REDIS_CHANNELS (e.g. articles:crime,articles:mining).
//!
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

mod redis_feed;
mod text_texture;

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Text2d;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;
use std::path::Path;

use redis_feed::{LiveArticle, LiveFeedBuffer, RedisConnectionStatus, RedisFeedConfig, spawn_subscriber};
use text_texture::{load_font, render_lines_to_rgba, render_text_to_rgba};

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

/// Font for VR text rasterization (loaded from assets/fonts at startup).
#[derive(Resource)]
struct VrTextFont(pub Option<ab_glyph::FontRef<'static>>);

/// Handles to the VR text textures so update systems can refresh their content.
#[derive(Resource)]
struct VrTextTextureHandles {
    redis_status: Handle<Image>,
    live_feed: Handle<Image>,
}

/// 3D quad showing Redis status as rasterized text (visible in XR).
#[derive(Component)]
struct VrRedisStatusTextQuad;

/// 3D quad showing live feed lines as rasterized text (visible in XR).
#[derive(Component)]
struct VrLiveFeedTextQuad;

/// Message-bus animation: a card flying from path start toward the feed panel.
#[derive(Component)]
struct AnimatedMessageCard {
    progress: f32,
    duration: f32,
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
        .add_systems(
            Startup,
            (
                setup_diagram,
                setup_feed_panel,
                setup_redis_status_panel,
                setup_vr_text_font,
                setup_vr_text_quads,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                drain_redis_feed,
                spawn_animated_messages,
                update_animated_messages,
                update_feed_panel,
                update_redis_status_panel,
                update_vr_text_textures,
            )
                .chain(),
        )
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

// Left-to-right: start left of feed, end right
const ANIMATED_CARD_PATH_START: Vec3 = Vec3::new(-0.5, 0.0, -1.95);
const ANIMATED_CARD_PATH_END: Vec3 = Vec3::new(0.3, 0.2, -1.9);
const ANIMATED_CARD_DURATION: f32 = 1.5;
const ANIMATED_CARD_TEXT_W: u32 = 512;
const ANIMATED_CARD_TEXT_H: u32 = 96;
const ANIMATED_CARD_TITLE_LEN: usize = 40;

fn spawn_animated_messages(
    mut commands: Commands,
    vr_font: Option<Res<VrTextFont>>,
    mut buffer: ResMut<LiveFeedBuffer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(vr) = vr_font else {
        return;
    };
    let Some(ref font) = vr.0 else {
        return;
    };
    let articles: Vec<LiveArticle> = buffer.recently_received.drain(..).collect();
    for article in articles {
        let title = article
            .title
            .chars()
            .take(ANIMATED_CARD_TITLE_LEN)
            .collect::<String>();
        let label = if article.title.len() > ANIMATED_CARD_TITLE_LEN {
            format!("[{}] {}…", article.channel, title)
        } else {
            format!("[{}] {}", article.channel, title)
        };
        let data = render_text_to_rgba(
            font,
            &label,
            ANIMATED_CARD_TEXT_W,
            ANIMATED_CARD_TEXT_H,
            28.0,
            220,
            220,
            220,
        );
        let image = Image::new(
            Extent3d {
                width: ANIMATED_CARD_TEXT_W,
                height: ANIMATED_CARD_TEXT_H,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            default(),
        );
        let image_handle = images.add(image);
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.4, 0.08, 0.002))),
            MeshMaterial3d(mat),
            Transform::from_translation(ANIMATED_CARD_PATH_START),
            AnimatedMessageCard {
                progress: 0.0,
                duration: ANIMATED_CARD_DURATION,
            },
        ));
    }
}

fn update_animated_messages(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut AnimatedMessageCard, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut card, mut transform) in query.iter_mut() {
        card.progress += dt / card.duration;
        if card.progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let t = card.progress.min(1.0);
        transform.translation = ANIMATED_CARD_PATH_START.lerp(ANIMATED_CARD_PATH_END, t);
    }
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

fn setup_vr_text_font(mut commands: Commands) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/FiraSans-Bold.ttf");
    let font = load_font(&path);
    if font.is_none() {
        eprintln!("[vr-text] No font at {:?}, VR text quads will not be created", path);
        eprintln!("[vr-text] Add FiraSans-Bold.ttf to assets/fonts/ for text in headset");
    }
    commands.insert_resource(VrTextFont(font));
}

fn setup_vr_text_quads(
    mut commands: Commands,
    vr_font: Option<Res<VrTextFont>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(vr) = vr_font else {
        return;
    };
    let Some(ref font) = vr.0 else {
        return;
    };
    const REDIS_W: u32 = 320;
    const REDIS_H: u32 = 64;
    const FEED_W: u32 = 512;
    const FEED_H: u32 = 256;

    let redis_data = render_text_to_rgba(font, "Redis: disconnected", REDIS_W, REDIS_H, 28.0, 230, 76, 51);
    let redis_image = Image::new(
        Extent3d {
            width: REDIS_W,
            height: REDIS_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        redis_data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );
    let redis_handle = images.add(redis_image);

    let feed_lines = vec!["Live feed (set REDIS_CHANNELS)".to_string()];
    let feed_data = render_lines_to_rgba(font, &feed_lines, FEED_W, FEED_H, 20.0, 230, 230, 230);
    let feed_image = Image::new(
        Extent3d {
            width: FEED_W,
            height: FEED_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        feed_data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );
    let feed_handle = images.add(feed_image);

    commands.insert_resource(VrTextTextureHandles {
        redis_status: redis_handle.clone(),
        live_feed: feed_handle.clone(),
    });

    // Slightly in front of the colored Redis status quad (z -1.95) so text is visible on top
    let redis_pos = Vec3::new(0.0, 0.55, -1.93);
    let redis_material = materials.add(StandardMaterial {
        base_color_texture: Some(redis_handle),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.32, 0.064, 0.002))),
        MeshMaterial3d(redis_material),
        Transform::from_translation(redis_pos),
        VrRedisStatusTextQuad,
    ));

    let feed_pos = Vec3::new(-0.8, 0.0, -2.0);
    let feed_material = materials.add(StandardMaterial {
        base_color_texture: Some(feed_handle),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.25, 0.002))),
        MeshMaterial3d(feed_material),
        Transform::from_translation(feed_pos).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        VrLiveFeedTextQuad,
    ));
    eprintln!("[vr-text] VR text quads spawned (visible in headset)");
}

fn setup_feed_panel(mut commands: Commands) {
    eprintln!("[debug] setup_feed_panel running");
    commands.spawn((
        Text2d::new("Live feed (set REDIS_CHANNELS to subscribe)"),
        TextFont {
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[debug] setup_redis_status_panel running");
    let pos = Vec3::new(0.0, 0.55, -1.95);
    commands.spawn((
        Text2d::new("Redis: disconnected"),
        TextFont {
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

const VR_REDIS_TEXT_W: u32 = 320;
const VR_REDIS_TEXT_H: u32 = 64;
const VR_FEED_TEXT_W: u32 = 512;
const VR_FEED_TEXT_H: u32 = 256;
const VR_FEED_MAX_LINES: usize = 10;
const VR_FEED_TITLE_LEN: usize = 50;

fn update_vr_text_textures(
    vr_font: Option<Res<VrTextFont>>,
    handles: Option<Res<VrTextTextureHandles>>,
    buffer: Res<LiveFeedBuffer>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(vr) = vr_font else {
        return;
    };
    let Some(ref font) = vr.0 else {
        return;
    };
    let Some(handles) = handles else {
        return;
    };
    let redis_label = match buffer.connection_status {
        RedisConnectionStatus::Disabled => "Redis: disconnected",
        RedisConnectionStatus::Connecting => "Redis: connecting…",
        RedisConnectionStatus::Connected => "Redis: connected",
        RedisConnectionStatus::Disconnected => "Redis: disconnected",
    };
    let (r, g, b) = match buffer.connection_status {
        RedisConnectionStatus::Connected => (51, 217, 102),
        RedisConnectionStatus::Connecting => (230, 204, 51),
        _ => (230, 76, 51),
    };
    if let Some(img) = images.get_mut(&handles.redis_status) {
        let data = render_text_to_rgba(font, redis_label, VR_REDIS_TEXT_W, VR_REDIS_TEXT_H, 28.0, r, g, b);
        img.data = Some(data);
    }
    let lines: Vec<String> = buffer
        .items
        .iter()
        .rev()
        .take(VR_FEED_MAX_LINES)
        .map(|a| {
            let title = a.title.chars().take(VR_FEED_TITLE_LEN).collect::<String>();
            let suffix = if a.title.len() > VR_FEED_TITLE_LEN { "…" } else { "" };
            let q = a.quality_score.map(|s| format!(" q{}", s)).unwrap_or_default();
            format!("[{}] {}{} | {}", a.channel, title, suffix, q)
        })
        .collect();
    let lines_ref: Vec<String> = if lines.is_empty() {
        vec!["Live feed (set REDIS_CHANNELS to subscribe)".to_string()]
    } else {
        lines
    };
    if let Some(img) = images.get_mut(&handles.live_feed) {
        let data = render_lines_to_rgba(
            font,
            &lines_ref,
            VR_FEED_TEXT_W,
            VR_FEED_TEXT_H,
            20.0,
            230,
            230,
            230,
        );
        img.data = Some(data);
    }
}
