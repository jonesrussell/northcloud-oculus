//! VR interaction system - controller tracking and raycasting

mod controller;
mod raycast;
mod events;

pub use controller::*;
pub use raycast::*;
pub use events::*;

use bevy::prelude::*;
use bevy_mod_openxr::action_binding::OxrSendActionBindings;
use bevy_mod_xr::session::XrSessionCreated;
use bevy_xr_utils::tracking_utils::{suggest_action_bindings, TrackingUtilitiesPlugin};

/// Plugin that adds VR interaction functionality
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RaycastHit>()
            .init_resource::<PointerRayConfig>()
            .init_resource::<HoverState>()
            .init_resource::<SelectionState>()
            .init_resource::<TriggerInput>()
            .add_plugins(TrackingUtilitiesPlugin)
            .add_systems(OxrSendActionBindings, suggest_action_bindings)
            .add_systems(XrSessionCreated, spawn_controller_visuals)
            .add_systems(
                Update,
                (
                    update_pointer_ray,
                    perform_raycast,
                    update_ray_appearance,
                    update_hover_state,
                    update_selection,
                    apply_hover_highlight,
                    restore_material_on_unhover,
                )
                    .chain(),
            );
    }
}
