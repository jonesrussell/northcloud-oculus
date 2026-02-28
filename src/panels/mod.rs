//! Feature panels built on WorldPanel

mod map_panel;
mod detail_panel;

pub use map_panel::*;
pub use detail_panel::*;

use bevy::prelude::*;

/// Plugin that adds feature panel functionality
pub struct PanelsPlugin;

impl Plugin for PanelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DetailPanelConfig>()
            .add_systems(Update, (spawn_detail_on_selection, render_detail_panel_ui));
    }
}
