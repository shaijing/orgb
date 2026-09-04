use crate::core::{Topology, ZoneKind};

pub(super) const SPATIAL_HUE_STEP: f32 = 18.0 / 360.0;

pub(super) fn zone_spatial_position(
    topology: &Topology,
    led_id: usize,
    zone_id: usize,
    fallback_position: f32,
    reverse: bool,
) -> f32 {
    let direction = if reverse { -1.0 } else { 1.0 };

    topology
        .zones()
        .get(zone_id)
        .map(|zone| match zone.kind {
            ZoneKind::Argb => {
                let local_led = led_id.saturating_sub(zone.offset) as f32;
                direction * local_led * SPATIAL_HUE_STEP
            }
            ZoneKind::Rgb => 0.0,
        })
        .unwrap_or(direction * fallback_position)
}
