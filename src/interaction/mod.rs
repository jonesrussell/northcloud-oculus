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
use bevy_xr_utils::actions::{
    ActionType, ActiveSet, XRUtilsAction, XRUtilsActionSet, XRUtilsActionsPlugin, XRUtilsBinding,
    XRUtilsActionSystems,
};
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
            .add_plugins(XRUtilsActionsPlugin)
            .add_systems(OxrSendActionBindings, suggest_action_bindings)
            .add_systems(XrSessionCreated, spawn_controller_visuals)
            .add_systems(Startup, setup_trigger_actions)
            .add_systems(
                Update,
                update_trigger_input.after(XRUtilsActionSystems::SyncActionStates),
            )
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
                    .chain()
                    .after(update_trigger_input),
            );
    }
}

/// Sets up the OpenXR actions for trigger input
fn setup_trigger_actions(mut commands: Commands) {
    let set = commands
        .spawn((
            XRUtilsActionSet {
                name: "interaction".into(),
                pretty_name: "Interaction Actions".into(),
                priority: 0,
            },
            ActiveSet,
        ))
        .id();

    let action = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_trigger".into(),
                localized_name: "Right Trigger".into(),
                action_type: ActionType::Float,
            },
            RightTriggerAction,
        ))
        .id();

    let binding_oculus = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/right/input/trigger/value".into(),
        })
        .id();

    let binding_valve = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/valve/index_controller".into(),
            binding: "/user/hand/right/input/trigger/value".into(),
        })
        .id();

    commands.entity(action).add_children(&[binding_oculus, binding_valve]);
    commands.entity(set).add_child(action);
}
