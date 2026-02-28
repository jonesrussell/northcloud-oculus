//! Interaction events and hover/selection systems

use bevy::prelude::*;

use crate::node_marker::{NodeMarker, NodeMarkerMaterials};

use super::RaycastHit;

/// Component to track if an entity is currently hovered
#[derive(Component)]
pub struct Hovered;

/// Component to track if an entity is currently selected
#[derive(Component)]
pub struct Selected;

/// Resource tracking current hover state
#[derive(Resource, Default)]
pub struct HoverState {
    pub hovered_entity: Option<Entity>,
    pub previous_hovered: Option<Entity>,
}

/// Resource tracking selection state
#[derive(Resource, Default)]
pub struct SelectionState {
    pub selected_entity: Option<Entity>,
}

/// Trigger input state (from VR controller)
#[derive(Resource, Default)]
pub struct TriggerInput {
    pub right_trigger_pressed: bool,
    pub right_trigger_just_pressed: bool,
}

/// System to update hover state based on raycast hits
pub fn update_hover_state(
    mut commands: Commands,
    hit: Res<RaycastHit>,
    mut hover_state: ResMut<HoverState>,
    markers: Query<Entity, With<NodeMarker>>,
) {
    hover_state.previous_hovered = hover_state.hovered_entity;

    if let Some(entity) = hit.entity {
        if markers.contains(entity) {
            hover_state.hovered_entity = Some(entity);

            if hover_state.previous_hovered != Some(entity) {
                if let Some(prev) = hover_state.previous_hovered {
                    commands.entity(prev).remove::<Hovered>();
                }
                commands.entity(entity).insert(Hovered);
            }
        } else {
            hover_state.hovered_entity = None;
            if let Some(prev) = hover_state.previous_hovered {
                commands.entity(prev).remove::<Hovered>();
            }
        }
    } else {
        hover_state.hovered_entity = None;
        if let Some(prev) = hover_state.previous_hovered {
            commands.entity(prev).remove::<Hovered>();
        }
    }
}

/// System to update selection based on trigger input
pub fn update_selection(
    mut commands: Commands,
    trigger: Res<TriggerInput>,
    hover_state: Res<HoverState>,
    mut selection_state: ResMut<SelectionState>,
) {
    if trigger.right_trigger_just_pressed {
        if let Some(hovered) = hover_state.hovered_entity {
            if let Some(prev_selected) = selection_state.selected_entity {
                if prev_selected != hovered {
                    commands.entity(prev_selected).remove::<Selected>();
                }
            }
            commands.entity(hovered).insert(Selected);
            selection_state.selected_entity = Some(hovered);
        }
    }
}

/// System to apply hover highlight to NodeMarkers
pub fn apply_hover_highlight(
    marker_materials: Option<Res<NodeMarkerMaterials>>,
    mut query: Query<
        (&NodeMarker, &mut MeshMaterial3d<StandardMaterial>, Option<&Hovered>),
        Changed<Hovered>,
    >,
) {
    let Some(materials) = marker_materials else {
        return;
    };

    for (marker, mut mat, hovered) in query.iter_mut() {
        if hovered.is_some() {
            mat.0 = materials.hover.clone();
        } else {
            mat.0 = match marker.health {
                crate::node_marker::NodeHealth::Healthy => materials.healthy.clone(),
                crate::node_marker::NodeHealth::Warning => materials.warning.clone(),
                crate::node_marker::NodeHealth::Critical => materials.critical.clone(),
            };
        }
    }
}

/// System to restore material when hover ends
pub fn restore_material_on_unhover(
    marker_materials: Option<Res<NodeMarkerMaterials>>,
    mut removed: RemovedComponents<Hovered>,
    mut query: Query<(&NodeMarker, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(materials) = marker_materials else {
        return;
    };

    for entity in removed.read() {
        if let Ok((marker, mut mat)) = query.get_mut(entity) {
            mat.0 = match marker.health {
                crate::node_marker::NodeHealth::Healthy => materials.healthy.clone(),
                crate::node_marker::NodeHealth::Warning => materials.warning.clone(),
                crate::node_marker::NodeHealth::Critical => materials.critical.clone(),
            };
        }
    }
}
