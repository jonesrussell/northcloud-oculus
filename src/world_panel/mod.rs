//! WorldPanel - Render UI to texture displayed on 3D quads in VR
//!
//! This module provides the core WorldPanel system for creating world-space UI panels
//! that render egui content to textures and display them on 3D quads.

mod render;
mod systems;
mod egui;

pub use render::*;
pub use systems::*;
pub use egui::*;

use bevy::prelude::*;

/// Plugin that adds WorldPanel functionality
pub struct WorldPanelPlugin;

impl Plugin for WorldPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldPanelDefaults>()
            .add_systems(Update, cleanup_orphaned_ui_cameras);
    }
}

/// Component for a world-space UI panel
#[derive(Component)]
pub struct WorldPanel {
    /// Panel size in meters (width, height)
    pub size: Vec2,
    /// Texture resolution in pixels (calculated from size * pixels_per_meter)
    pub resolution: UVec2,
    /// Entity of the associated UI camera that renders to this panel's texture
    pub ui_camera: Entity,
    /// Handle to the render target texture
    pub texture: Handle<Image>,
}

/// Marker component for the UI camera associated with a WorldPanel
#[derive(Component)]
pub struct WorldPanelCamera {
    /// The panel entity this camera renders to
    pub panel: Entity,
}

/// Configuration defaults for WorldPanel creation
#[derive(Resource)]
pub struct WorldPanelDefaults {
    /// Pixels per meter for texture resolution calculation
    pub pixels_per_meter: u32,
    /// Background color for panels
    pub clear_color: Color,
}

impl Default for WorldPanelDefaults {
    fn default() -> Self {
        Self {
            pixels_per_meter: 1024,
            clear_color: Color::srgba(0.1, 0.1, 0.15, 0.95),
        }
    }
}

