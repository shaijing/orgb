use crate::core::{Rgb, Topology};

use super::color::hsv_to_rgb;

pub(super) fn rainbow_color(
    topology: &Topology,
    led_id: usize,
    zone_id: usize,
    fallback_position: f32,
    time: f32,
    speed: f32,
    reverse: bool,
) -> Rgb {
    let phase = time * speed;
    let spatial = super::spatial::zone_spatial_position(
        topology,
        led_id,
        zone_id,
        fallback_position,
        reverse,
    );

    hsv_to_rgb((phase + spatial).rem_euclid(1.0))
}
