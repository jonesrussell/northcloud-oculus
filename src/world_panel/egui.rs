//! bevy_egui integration for WorldPanel

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use super::WorldPanel;

/// Marker component for panels that use egui rendering
#[derive(Component)]
pub struct EguiPanel;

/// Helper function to draw egui content to a WorldPanel
pub fn draw_panel_ui(
    egui_contexts: &mut EguiContexts,
    panel: &WorldPanel,
    draw_fn: impl FnOnce(&egui::Context),
) {
    match egui_contexts.ctx_for_entity_mut(panel.ui_camera) {
        Ok(ctx) => draw_fn(ctx),
        Err(e) => {
            bevy::log::warn!(
                "Failed to get egui context for panel camera {:?}: {e}",
                panel.ui_camera
            );
        }
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
