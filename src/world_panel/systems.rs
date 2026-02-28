//! WorldPanel lifecycle systems

use bevy::prelude::*;

use super::{WorldPanel, WorldPanelCamera};

/// Cleans up UI cameras when their associated panels are despawned
pub fn cleanup_orphaned_ui_cameras(
    mut commands: Commands,
    cameras: Query<(Entity, &WorldPanelCamera)>,
    panels: Query<Entity, With<WorldPanel>>,
) {
    for (camera_entity, camera) in cameras.iter() {
        if !panels.contains(camera.panel) {
            commands.entity(camera_entity).despawn();
        }
    }
}

/// Despawns a WorldPanel and its associated UI camera
pub fn despawn_world_panel(commands: &mut Commands, panel_entity: Entity, panel: &WorldPanel) {
    commands.entity(panel.ui_camera).despawn();
    commands.entity(panel_entity).despawn();
}
