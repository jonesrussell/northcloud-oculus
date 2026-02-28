//! NodeMarker - Color-coded status indicators for map nodes

mod materials;
mod animation;

pub use materials::*;
pub use animation::*;

use bevy::prelude::*;

use crate::interaction::{RaycastBounds, RaycastTarget};

/// Plugin that adds NodeMarker functionality
pub struct NodeMarkerPlugin;

impl Plugin for NodeMarkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_node_marker_materials)
            .add_systems(Update, (animate_warning_pulse, update_node_marker_materials));
    }
}

/// Health status for a node
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NodeHealth {
    #[default]
    Healthy,
    Warning,
    Critical,
}

/// Component for a node status marker
#[derive(Component)]
pub struct NodeMarker {
    pub id: String,
    pub health: NodeHealth,
}

/// Configuration for NodeMarker spawning
#[derive(Resource)]
pub struct NodeMarkerConfig {
    pub radius: f32,
}

impl Default for NodeMarkerConfig {
    fn default() -> Self {
        Self { radius: 0.02 }
    }
}

/// Spawns a NodeMarker at the given position
pub fn spawn_node_marker(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    marker_materials: &NodeMarkerMaterials,
    id: impl Into<String>,
    position: Vec3,
    health: NodeHealth,
    radius: f32,
) -> Entity {
    let material = match health {
        NodeHealth::Healthy => marker_materials.healthy.clone(),
        NodeHealth::Warning => marker_materials.warning.clone(),
        NodeHealth::Critical => marker_materials.critical.clone(),
    };

    let mesh = meshes.add(Sphere::new(radius));

    let mut entity = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(position),
        NodeMarker {
            id: id.into(),
            health,
        },
        RaycastTarget,
        RaycastBounds {
            half_extents: Vec3::splat(radius * 1.5),
        },
    ));

    if health != NodeHealth::Healthy {
        let pulse = match health {
            NodeHealth::Warning => PulseAnimation::warning(),
            NodeHealth::Critical => PulseAnimation::critical(),
            _ => PulseAnimation::default(),
        };
        entity.insert(pulse);
    }

    entity.id()
}

/// System to update NodeMarker materials when health changes
pub fn update_node_marker_materials(
    mut commands: Commands,
    marker_materials: Option<Res<NodeMarkerMaterials>>,
    mut query: Query<(Entity, &NodeMarker, &mut MeshMaterial3d<StandardMaterial>), Changed<NodeMarker>>,
) {
    let Some(materials) = marker_materials else {
        return;
    };

    for (entity, marker, mut mat) in query.iter_mut() {
        let new_material = match marker.health {
            NodeHealth::Healthy => materials.healthy.clone(),
            NodeHealth::Warning => materials.warning.clone(),
            NodeHealth::Critical => materials.critical.clone(),
        };
        mat.0 = new_material;

        match marker.health {
            NodeHealth::Healthy => {
                commands.entity(entity).remove::<PulseAnimation>();
            }
            NodeHealth::Warning => {
                commands.entity(entity).insert(PulseAnimation::warning());
            }
            NodeHealth::Critical => {
                commands.entity(entity).insert(PulseAnimation::critical());
            }
        }
    }
}
