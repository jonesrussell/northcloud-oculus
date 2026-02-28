//! Pulse animation for warning/critical NodeMarkers

use bevy::prelude::*;

use super::{NodeHealth, NodeMarker};

/// Component to track pulse animation state
#[derive(Component)]
pub struct PulseAnimation {
    pub phase: f32,
    pub speed: f32,
    pub min_scale: f32,
    pub max_scale: f32,
}

impl Default for PulseAnimation {
    fn default() -> Self {
        Self {
            phase: 0.0,
            speed: 2.0,
            min_scale: 0.9,
            max_scale: 1.2,
        }
    }
}

impl PulseAnimation {
    pub fn warning() -> Self {
        Self {
            phase: 0.0,
            speed: 2.0,
            ..default()
        }
    }

    pub fn critical() -> Self {
        Self {
            phase: 0.0,
            speed: 4.0,
            min_scale: 0.85,
            max_scale: 1.3,
        }
    }
}

/// System to animate warning/critical markers with a pulsing effect
pub fn animate_warning_pulse(
    time: Res<Time>,
    mut query: Query<(&NodeMarker, &mut Transform, &mut PulseAnimation)>,
) {
    for (marker, mut transform, mut pulse) in query.iter_mut() {
        if marker.health == NodeHealth::Healthy {
            continue;
        }

        pulse.phase += time.delta_secs() * pulse.speed;
        if pulse.phase > std::f32::consts::TAU {
            pulse.phase -= std::f32::consts::TAU;
        }
        let t = (pulse.phase.sin() + 1.0) * 0.5;
        let scale = pulse.min_scale + t * (pulse.max_scale - pulse.min_scale);

        transform.scale = Vec3::splat(scale);
    }
}
