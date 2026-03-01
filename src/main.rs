//! northcloud-oculus — VR-native observability cockpit
//!
//! Renders world-space UI panels with egui content in VR via Bevy + bevy_mod_openxr.
//! Run with: `cargo run --release` (requires Oculus runtime + openxr_loader.dll).

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_egui::{egui, EguiPlugin};
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

use northcloud_oculus::data::{DataIngestionConfig, DataIngestionPlugin};
use northcloud_oculus::interaction::InteractionPlugin;
use northcloud_oculus::node_marker::NodeMarkerPlugin;
use northcloud_oculus::panels::PanelsPlugin;
use northcloud_oculus::world_panel::{
    configure_vr_egui_style, draw_panel_ui, spawn_world_panel, EguiPanel, WorldPanel,
    WorldPanelDefaults, WorldPanelParams, WorldPanelPlugin,
};

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
        .add_systems(Update, demo_panel_ui)
        .run()
}

fn setup_scene(
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

    // Ambient light for the scene
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
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
