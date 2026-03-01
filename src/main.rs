//! northcloud-oculus — VR-native observability cockpit
//!
//! Renders world-space UI panels with egui content in VR via Bevy + bevy_mod_openxr.
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_egui::EguiPlugin;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

use northcloud_oculus::data::{DataIngestionConfig, DataIngestionPlugin};
use northcloud_oculus::interaction::InteractionPlugin;
use northcloud_oculus::node_marker::NodeMarkerPlugin;
use northcloud_oculus::panels::{spawn_classifier_panel, spawn_frontier_panel, PanelsPlugin};
use northcloud_oculus::world_panel::WorldPanelPlugin;

fn main() -> AppExit {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    App::new()
        .add_plugins(add_xr_plugins(
            DefaultPlugins.build().disable::<PipelinedRenderingPlugin>(),
        ))
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldPanelPlugin)
        .add_plugins(InteractionPlugin)
        .add_plugins(NodeMarkerPlugin)
        .add_plugins(PanelsPlugin)
        .insert_resource(DataIngestionConfig::from_env())
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
        .add_systems(Startup, setup_scene)
        .run()
}

fn setup_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    defaults: Res<WorldPanelDefaults>,
) {
    // Spawn frontier operations panel (left)
    spawn_frontier_panel(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        &defaults,
    );

    // Spawn classifier panel for Loki logs (right)
    spawn_classifier_panel(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        &defaults,
    );

    // Ambient light for the scene
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
}

use northcloud_oculus::world_panel::WorldPanelDefaults;
