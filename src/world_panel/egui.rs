//! bevy_egui integration for WorldPanel

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use super::WorldPanel;

/// Marker component for panels that use egui rendering
#[derive(Component)]
pub struct EguiPanel;

/// Trait for drawing egui content to a WorldPanel
pub trait WorldPanelUi {
    fn draw_ui(&self, ctx: &egui::Context);
}

/// System parameter for accessing a WorldPanel's egui context
pub struct WorldPanelEgui<'w, 's> {
    egui_contexts: EguiContexts<'w, 's>,
}

impl<'w, 's> WorldPanelEgui<'w, 's> {
    /// Get the egui context for a specific panel's UI camera
    pub fn ctx_for_panel(&mut self, panel: &WorldPanel) -> Option<&mut egui::Context> {
        self.egui_contexts.ctx_for_entity_mut(panel.ui_camera).ok()
    }
}

/// Helper function to draw egui content to a WorldPanel
pub fn draw_panel_ui(
    egui_contexts: &mut EguiContexts,
    panel: &WorldPanel,
    draw_fn: impl FnOnce(&egui::Context),
) {
    if let Ok(ctx) = egui_contexts.ctx_for_entity_mut(panel.ui_camera) {
        draw_fn(ctx);
    }
}

/// Example system showing how to draw egui content to WorldPanels
pub fn example_panel_ui_system(
    mut egui_contexts: EguiContexts,
    panels: Query<&WorldPanel, With<EguiPanel>>,
) {
    for panel in panels.iter() {
        draw_panel_ui(&mut egui_contexts, panel, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("World Panel");
                ui.label("This is rendered to a texture in 3D space!");
            });
        });
    }
}

/// Configuration for WorldPanel egui styling optimized for VR
pub fn configure_vr_egui_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(48.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(32.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(36.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(24.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(28.0, egui::FontFamily::Monospace),
    );

    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(16.0, 12.0);

    ctx.set_style(style);
}
