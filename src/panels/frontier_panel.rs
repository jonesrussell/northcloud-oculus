//! FrontierPanel - Displays frontier queue stats from Grafana/Loki
//!
//! Shows key metrics from the north-cloud crawler's frontier operations:
//! - Submit/Fetch events
//! - Queue depth (pending/fetching)
//! - Failure counts

use bevy::prelude::*;
use bevy_egui::egui;
use std::time::Instant;

use crate::world_panel::{
    configure_vr_egui_style, draw_panel_ui, spawn_world_panel, WorldPanel, WorldPanelDefaults,
    WorldPanelParams,
};

/// Frontier statistics from Loki log aggregation
#[derive(Resource, Default)]
pub struct FrontierStats {
    pub submit_events: u64,
    pub new_urls_queued: u64,
    pub fetch_success: u64,
    pub fetch_failures: u64,
    pub robots_blocked: u64,
    pub dead_urls: u64,
    pub pending: u64,
    pub fetching: u64,
    pub last_updated: Option<Instant>,
    pub fetch_error: Option<String>,
}

/// Marker component for the frontier panel
#[derive(Component)]
pub struct FrontierPanel;

/// Spawns the frontier panel
pub fn spawn_frontier_panel(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    defaults: &WorldPanelDefaults,
) -> Entity {
    let panel_entity = spawn_world_panel(
        commands,
        images,
        meshes,
        materials,
        defaults,
        WorldPanelParams {
            size: bevy::math::Vec2::new(1.0, 0.7),
            transform: Transform::from_xyz(-0.6, 1.2, -1.5)
                .looking_at(bevy::math::Vec3::new(0.0, 1.2, 0.0), bevy::math::Vec3::Y)
                * Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            camera_order: -3,
            ..default()
        },
    );

    commands.entity(panel_entity).insert(FrontierPanel);

    panel_entity
}

/// System that renders the frontier panel UI
pub fn render_frontier_panel_ui(
    mut egui_contexts: bevy_egui::EguiContexts,
    panels: Query<&WorldPanel, With<FrontierPanel>>,
    stats: Option<Res<FrontierStats>>,
) {
    for panel in panels.iter() {
        draw_panel_ui(&mut egui_contexts, panel, |ctx| {
            configure_vr_egui_style(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Frontier Operations");
                ui.separator();

                if let Some(ref stats) = stats {
                    // Status line
                    ui.horizontal(|ui| {
                        if let Some(last) = stats.last_updated {
                            let ago = last.elapsed().as_secs();
                            ui.label(format!("Updated: {}s ago", ago));
                        } else {
                            ui.label("Waiting for data...");
                        }
                    });

                    if let Some(ref err) = stats.fetch_error {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                    }

                    ui.add_space(8.0);

                    // Queue section
                    ui.label(egui::RichText::new("Queue").strong().size(14.0));
                    ui.horizontal(|ui| {
                        stat_box(ui, "Pending", stats.pending, egui::Color32::from_rgb(70, 130, 180));
                        stat_box(ui, "Fetching", stats.fetching, egui::Color32::from_rgb(218, 165, 32));
                    });

                    ui.add_space(8.0);

                    // Activity section
                    ui.label(egui::RichText::new("Activity").strong().size(14.0));
                    ui.horizontal(|ui| {
                        stat_box(ui, "Submitted", stats.submit_events, egui::Color32::from_rgb(100, 149, 237));
                        stat_box(ui, "Queued", stats.new_urls_queued, egui::Color32::from_rgb(138, 43, 226));
                    });
                    ui.horizontal(|ui| {
                        stat_box(ui, "Fetched", stats.fetch_success, egui::Color32::from_rgb(50, 205, 50));
                        stat_box(ui, "Failed", stats.fetch_failures, egui::Color32::from_rgb(220, 20, 60));
                    });

                    ui.add_space(8.0);

                    // Blocks section
                    ui.label(egui::RichText::new("Blocked").strong().size(14.0));
                    ui.horizontal(|ui| {
                        stat_box(ui, "Robots", stats.robots_blocked, egui::Color32::from_rgb(255, 140, 0));
                        stat_box(ui, "Dead", stats.dead_urls, egui::Color32::from_rgb(139, 0, 0));
                    });
                } else {
                    ui.label("FrontierStats not initialized.");
                }
            });
        });
    }
}

fn stat_box(ui: &mut egui::Ui, label: &str, value: u64, color: egui::Color32) {
    ui.group(|ui| {
        ui.set_min_width(80.0);
        ui.vertical(|ui| {
            ui.colored_label(color, egui::RichText::new(format_number(value)).size(18.0).strong());
            ui.label(egui::RichText::new(label).size(11.0).color(egui::Color32::GRAY));
        });
    });
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
