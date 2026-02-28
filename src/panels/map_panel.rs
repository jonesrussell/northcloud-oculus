//! MapPanel - World-space map with NodeMarkers

use bevy::prelude::*;

use crate::interaction::{RaycastBounds, RaycastTarget};
use crate::node_marker::{spawn_node_marker, NodeHealth, NodeMarkerMaterials};
use crate::world_panel::{spawn_world_panel, WorldPanelDefaults, WorldPanelParams};

/// Geographic bounds for coordinate conversion
#[derive(Clone, Debug)]
pub struct GeoBounds {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lon: f64,
    pub max_lon: f64,
}

impl Default for GeoBounds {
    fn default() -> Self {
        Self {
            min_lat: -90.0,
            max_lat: 90.0,
            min_lon: -180.0,
            max_lon: 180.0,
        }
    }
}

impl GeoBounds {
    /// Create bounds for a specific region
    pub fn new(min_lat: f64, max_lat: f64, min_lon: f64, max_lon: f64) -> Self {
        Self {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
        }
    }

    /// Convert geographic coordinates to local panel coordinates
    /// Returns coordinates relative to panel center (origin at center)
    pub fn geo_to_local(&self, lat: f64, lon: f64, panel_size: Vec2) -> Vec2 {
        let x = ((lon - self.min_lon) / (self.max_lon - self.min_lon)) as f32;
        let y = ((lat - self.min_lat) / (self.max_lat - self.min_lat)) as f32;

        Vec2::new((x - 0.5) * panel_size.x, (y - 0.5) * panel_size.y)
    }
}

/// Component for a map panel that displays NodeMarkers
#[derive(Component)]
pub struct MapPanel {
    pub size: Vec2,
    pub bounds: GeoBounds,
}

impl Default for MapPanel {
    fn default() -> Self {
        Self {
            size: Vec2::new(1.2, 0.8),
            bounds: GeoBounds::default(),
        }
    }
}

/// Parameters for spawning a MapPanel
pub struct MapPanelParams {
    pub size: Vec2,
    pub position: Vec3,
    pub look_at: Vec3,
    pub bounds: GeoBounds,
    pub map_texture: Option<Handle<Image>>,
}

impl Default for MapPanelParams {
    fn default() -> Self {
        Self {
            size: Vec2::new(1.2, 0.8),
            position: Vec3::new(0.0, 1.5, -2.0),
            look_at: Vec3::new(0.0, 1.5, 0.0),
            bounds: GeoBounds::default(),
            map_texture: None,
        }
    }
}

/// Spawns a MapPanel as a WorldPanel with an optional map texture
pub fn spawn_map_panel(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    defaults: &WorldPanelDefaults,
    params: MapPanelParams,
) -> Entity {
    let transform = Transform::from_translation(params.position).looking_at(params.look_at, Vec3::Y);

    let panel_entity = spawn_world_panel(
        commands,
        images,
        meshes,
        materials,
        defaults,
        WorldPanelParams {
            size: params.size,
            transform,
            ..default()
        },
    );

    commands.entity(panel_entity).insert((
        MapPanel {
            size: params.size,
            bounds: params.bounds,
        },
        RaycastTarget,
        RaycastBounds {
            half_extents: Vec3::new(params.size.x / 2.0, params.size.y / 2.0, 0.05),
        },
    ));

    panel_entity
}

/// Node data for spawning markers on a map
pub struct MapNode {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub health: NodeHealth,
}

/// Spawns NodeMarkers on a MapPanel based on geographic coordinates
pub fn spawn_map_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    marker_materials: &NodeMarkerMaterials,
    map_entity: Entity,
    map_panel: &MapPanel,
    map_transform: &Transform,
    nodes: &[MapNode],
    marker_radius: f32,
) -> Vec<Entity> {
    let mut marker_entities = Vec::new();

    for node in nodes {
        let local_pos = map_panel.bounds.geo_to_local(node.lat, node.lon, map_panel.size);

        let world_offset = map_transform.rotation * Vec3::new(local_pos.x, local_pos.y, 0.05);
        let world_pos = map_transform.translation + world_offset;

        let marker = spawn_node_marker(
            commands,
            meshes,
            marker_materials,
            node.id.clone(),
            world_pos,
            node.health,
            marker_radius,
        );

        commands.entity(map_entity).add_child(marker);
        marker_entities.push(marker);
    }

    marker_entities
}
