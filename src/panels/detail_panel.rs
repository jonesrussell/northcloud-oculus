//! DetailPanel - Node details displayed via WorldPanel

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::data::NodeStatusBuffer;
use crate::interaction::{RaycastBounds, RaycastTarget, SelectionState};
use crate::node_marker::NodeMarker;
use crate::world_panel::{
    configure_vr_egui_style, draw_panel_ui, spawn_world_panel, EguiPanel, WorldPanel,
    WorldPanelDefaults, WorldPanelParams,
};

/// Component for a detail panel showing node information
#[derive(Component)]
pub struct DetailPanel {
    /// The NodeMarker entity this panel is associated with
    pub node_entity: Entity,
    /// The node ID for looking up data
    pub node_id: String,
}

/// Configuration for DetailPanel spawning
#[derive(Resource)]
pub struct DetailPanelConfig {
    pub size: Vec2,
    pub offset: Vec3,
}

impl Default for DetailPanelConfig {
    fn default() -> Self {
        Self {
            size: Vec2::new(0.4, 0.3),
            offset: Vec3::new(0.25, 0.15, 0.0),
        }
    }
}

/// Spawns a DetailPanel next to a NodeMarker
pub fn spawn_detail_panel(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    defaults: &WorldPanelDefaults,
    config: &DetailPanelConfig,
    node_entity: Entity,
    node_id: &str,
    node_position: Vec3,
    look_at: Vec3,
) -> Entity {
    let position = node_position + config.offset;
    let transform = Transform::from_translation(position).looking_at(look_at, Vec3::Y);

    let panel_entity = spawn_world_panel(
        commands,
        images,
        meshes,
        materials,
        defaults,
        WorldPanelParams {
            size: config.size,
            transform,
            camera_order: -2,
            ..default()
        },
    );

    commands.entity(panel_entity).insert((
        DetailPanel {
            node_entity,
            node_id: node_id.to_string(),
        },
        EguiPanel,
        RaycastTarget,
        RaycastBounds {
            half_extents: Vec3::new(config.size.x / 2.0, config.size.y / 2.0, 0.02),
        },
    ));

    panel_entity
}

/// System to spawn DetailPanel when a node is selected
pub fn spawn_detail_on_selection(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    defaults: Res<WorldPanelDefaults>,
    config: Res<DetailPanelConfig>,
    selection: Res<SelectionState>,
    markers: Query<(&Transform, &NodeMarker)>,
    existing_panels: Query<(Entity, &DetailPanel)>,
) {
    if !selection.is_changed() {
        return;
    }

    for (panel_entity, detail) in existing_panels.iter() {
        if Some(detail.node_entity) != selection.selected_entity {
            commands.entity(panel_entity).despawn();
        }
    }

    if let Some(selected) = selection.selected_entity {
        let already_has_panel = existing_panels
            .iter()
            .any(|(_, d)| d.node_entity == selected);

        if !already_has_panel {
            if let Ok((transform, marker)) = markers.get(selected) {
                spawn_detail_panel(
                    &mut commands,
                    &mut images,
                    &mut meshes,
                    &mut materials,
                    &defaults,
                    &config,
                    selected,
                    &marker.id,
                    transform.translation,
                    Vec3::new(0.0, transform.translation.y, 0.0),
                );
            }
        }
    }
}

/// System to render DetailPanel UI with egui
pub fn render_detail_panel_ui(
    mut egui_contexts: EguiContexts,
    panels: Query<(&WorldPanel, &DetailPanel)>,
    markers: Query<&NodeMarker>,
    node_buffer: Option<Res<NodeStatusBuffer>>,
) {
    use bevy_egui::egui;

    for (world_panel, detail_panel) in panels.iter() {
        let marker = markers.get(detail_panel.node_entity).ok();
        let node_status = node_buffer
            .as_ref()
            .and_then(|buf| buf.get(&detail_panel.node_id));

        draw_panel_ui(&mut egui_contexts, world_panel, |ctx| {
            configure_vr_egui_style(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading(&detail_panel.node_id);
                ui.separator();

                if let Some(marker) = marker {
                    ui.horizontal(|ui| {
                        ui.label("Status:");
                        let (color, text) = match marker.health {
                            crate::node_marker::NodeHealth::Healthy => {
                                (egui::Color32::GREEN, "Healthy")
                            }
                            crate::node_marker::NodeHealth::Warning => {
                                (egui::Color32::YELLOW, "Warning")
                            }
                            crate::node_marker::NodeHealth::Critical => {
                                (egui::Color32::RED, "Critical")
                            }
                        };
                        ui.colored_label(color, text);
                    });
                }

                if let Some(status) = node_status {
                    ui.add_space(8.0);
                    ui.label(format!("Lat: {:.4}", status.lat));
                    ui.label(format!("Lon: {:.4}", status.lon));

                    if !status.metrics.is_empty() {
                        ui.add_space(8.0);
                        ui.label("Metrics:");
                        for (key, value) in &status.metrics {
                            ui.horizontal(|ui| {
                                ui.label(format!("  {}: {:.2}", key, value));
                            });
                        }
                    }
                } else {
                    ui.add_space(8.0);
                    ui.label("No data available");
                }
            });
        });
    }
}
