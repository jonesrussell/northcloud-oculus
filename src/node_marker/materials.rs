//! Color-coded materials for NodeMarker

use bevy::prelude::*;

/// Resource holding pre-created materials for each health state
#[derive(Resource)]
pub struct NodeMarkerMaterials {
    pub healthy: Handle<StandardMaterial>,
    pub warning: Handle<StandardMaterial>,
    pub critical: Handle<StandardMaterial>,
    pub hover: Handle<StandardMaterial>,
}

/// System to create NodeMarker materials at startup
pub fn setup_node_marker_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let healthy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.3),
        emissive: LinearRgba::new(0.1, 0.5, 0.15, 1.0),
        ..default()
    });

    let warning = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.8, 0.2),
        emissive: LinearRgba::new(0.5, 0.4, 0.1, 1.0),
        ..default()
    });

    let critical = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.2, 0.2),
        emissive: LinearRgba::new(0.5, 0.1, 0.1, 1.0),
        ..default()
    });

    let hover = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 1.0),
        emissive: LinearRgba::new(0.15, 0.3, 0.6, 1.0),
        ..default()
    });

    commands.insert_resource(NodeMarkerMaterials {
        healthy,
        warning,
        critical,
        hover,
    });
}
