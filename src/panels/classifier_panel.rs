//! ClassifierPanel - Displays log entries from Grafana/Loki

use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::VecDeque;
use std::time::Instant;

use crate::world_panel::{
    configure_vr_egui_style, draw_panel_ui, spawn_world_panel, WorldPanel,
    WorldPanelDefaults, WorldPanelParams,
};

/// A single log entry from Loki
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: Instant,
    pub source: String,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Info => egui::Color32::from_rgb(180, 180, 180),
            LogLevel::Warning => egui::Color32::from_rgb(255, 200, 50),
            LogLevel::Error => egui::Color32::from_rgb(255, 80, 80),
        }
    }
}

/// Buffer holding recent log entries for display
#[derive(Resource)]
pub struct LogBuffer {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
    pub last_fetch: Option<Instant>,
    pub fetch_error: Option<String>,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100,
            last_fetch: None,
            fetch_error: None,
        }
    }
}

impl LogBuffer {
    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push_front(entry);
        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Marker component for the classifier panel
#[derive(Component)]
pub struct ClassifierPanel;

/// Spawns the classifier panel
pub fn spawn_classifier_panel(
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
            size: bevy::math::Vec2::new(1.2, 0.8),
            transform: Transform::from_xyz(0.6, 1.2, -1.5)
                .looking_at(bevy::math::Vec3::new(0.0, 1.2, 0.0), bevy::math::Vec3::Y)
                * Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            camera_order: -2,
            ..default()
        },
    );

    commands.entity(panel_entity).insert(ClassifierPanel);

    panel_entity
}

/// System that renders the classifier panel UI
pub fn render_classifier_panel_ui(
    mut egui_contexts: bevy_egui::EguiContexts,
    panels: Query<&WorldPanel, With<ClassifierPanel>>,
    log_buffer: Option<Res<LogBuffer>>,
) {
    for panel in panels.iter() {
        draw_panel_ui(&mut egui_contexts, panel, |ctx| {
            configure_vr_egui_style(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Classifier Logs");
                ui.separator();

                if let Some(ref buffer) = log_buffer {
                    // Status line
                    ui.horizontal(|ui| {
                        ui.label(format!("Entries: {}", buffer.entries.len()));
                        ui.separator();
                        if let Some(last) = buffer.last_fetch {
                            let ago = last.elapsed().as_secs();
                            ui.label(format!("Updated: {}s ago", ago));
                        } else {
                            ui.label("Waiting for data...");
                        }
                    });

                    if let Some(ref err) = buffer.fetch_error {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                    }

                    ui.separator();

                    // Log entries
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if buffer.entries.is_empty() {
                                ui.label("No log entries yet.");
                                ui.add_space(8.0);
                                ui.label("Logs will appear here when");
                                ui.label("fetched from Grafana/Loki.");
                            } else {
                                for entry in buffer.entries.iter() {
                                    ui.horizontal(|ui| {
                                        let age = entry.timestamp.elapsed().as_secs();
                                        ui.colored_label(
                                            egui::Color32::GRAY,
                                            format!("[{}s]", age),
                                        );
                                        ui.colored_label(
                                            entry.level.color(),
                                            &entry.source,
                                        );
                                    });
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.message)
                                                .color(entry.level.color())
                                                .size(12.0),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(4.0);
                                }
                            }
                        });
                } else {
                    ui.label("Log buffer not initialized.");
                    ui.label("Check DataIngestionPlugin.");
                }
            });
        });
    }
}
