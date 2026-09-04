use crate::core::Rgb;

use super::blend_colors;

pub(super) fn wave_color(phase: f32, primary: Rgb, secondary: Rgb) -> Rgb {
    let blend = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();

    blend_colors(primary, secondary, blend)
}
