use clap::ValueEnum;
use std::time::Duration;

use crate::core::{Frame, Rgb, Topology};

const TAU: f32 = std::f32::consts::TAU;
const RAINBOW_SPATIAL_HUE_STEP: f32 = 18.0 / 360.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EffectKind {
    /// Keep every LED on with the primary color.
    Solid,
    /// Move a full-spectrum gradient across the LEDs.
    Rainbow,
    /// Fade the primary color in and out.
    #[value(alias = "breath")]
    Breathing,
    /// Move a primary/secondary color wave across the LEDs.
    Wave,
    /// Change the whole array through the color wheel.
    Cycle,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub brightness: f32,
    pub reverse: bool,
}

pub fn render_frame(
    kind: EffectKind,
    topology: &Topology,
    elapsed: Duration,
    primary: Rgb,
    secondary: Rgb,
    speed: f32,
) -> Frame {
    render_frame_with_options(
        kind,
        topology,
        elapsed,
        primary,
        secondary,
        speed,
        RenderOptions {
            brightness: 1.0,
            reverse: false,
        },
    )
}

pub fn render_frame_with_options(
    kind: EffectKind,
    topology: &Topology,
    elapsed: Duration,
    primary: Rgb,
    secondary: Rgb,
    speed: f32,
    options: RenderOptions,
) -> Frame {
    let time = elapsed.as_secs_f32();
    let brightness = options.brightness.clamp(0.0, 1.0);

    let pixels = topology
        .leds()
        .iter()
        .map(|(info, layout)| {
            let position = layout.x;

            let color = match kind {
                EffectKind::Solid => primary,
                EffectKind::Rainbow => rainbow_color(
                    topology,
                    info.id,
                    info.zone_id,
                    position,
                    time,
                    speed,
                    options.reverse,
                ),
                EffectKind::Breathing => scale_color(
                    primary,
                    0.1 + 0.9 * (0.5 + 0.5 * (time * speed * TAU).cos()),
                ),
                EffectKind::Wave => {
                    let spatial = zone_spatial_position(
                        topology,
                        info.id,
                        info.zone_id,
                        position,
                        options.reverse,
                    );
                    wave_color((time * speed + spatial).rem_euclid(1.0), primary, secondary)
                }
                EffectKind::Cycle => {
                    let direction = if options.reverse { -1.0 } else { 1.0 };
                    hsv_to_rgb((direction * time * speed).rem_euclid(1.0))
                }
            };

            scale_color(color, brightness)
        })
        .collect();

    Frame::from_pixels(pixels)
}

fn rainbow_color(
    topology: &Topology,
    led_id: usize,
    zone_id: usize,
    fallback_position: f32,
    time: f32,
    speed: f32,
    reverse: bool,
) -> Rgb {
    let direction = if reverse { -1.0 } else { 1.0 };
    let phase = time * speed;

    let hue = topology
        .zones()
        .get(zone_id)
        .map(|zone| {
            let local_led = led_id.saturating_sub(zone.offset) as f32;
            match zone.kind {
                crate::core::ZoneKind::Argb => {
                    phase + direction * local_led * RAINBOW_SPATIAL_HUE_STEP
                }
                crate::core::ZoneKind::Rgb => phase,
            }
        })
        .unwrap_or(phase + direction * fallback_position);

    hsv_to_rgb(hue.rem_euclid(1.0))
}

fn zone_spatial_position(
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
            crate::core::ZoneKind::Argb => {
                let local_led = led_id.saturating_sub(zone.offset) as f32;
                direction * local_led * RAINBOW_SPATIAL_HUE_STEP
            }
            crate::core::ZoneKind::Rgb => 0.0,
        })
        .unwrap_or(direction * fallback_position)
}

fn wave_color(phase: f32, primary: Rgb, secondary: Rgb) -> Rgb {
    let blend = 0.5 - 0.5 * (phase * TAU).cos();

    blend_colors(primary, secondary, blend)
}

fn blend_colors(first: Rgb, second: Rgb, amount: f32) -> Rgb {
    Rgb {
        red: blend_channel(first.red, second.red, amount),
        green: blend_channel(first.green, second.green, amount),
        blue: blend_channel(first.blue, second.blue, amount),
    }
}

fn blend_channel(first: u8, second: u8, amount: f32) -> u8 {
    (first as f32 + (second as f32 - first as f32) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_color(color: Rgb, amount: f32) -> Rgb {
    Rgb {
        red: (color.red as f32 * amount).round().clamp(0.0, 255.0) as u8,
        green: (color.green as f32 * amount).round().clamp(0.0, 255.0) as u8,
        blue: (color.blue as f32 * amount).round().clamp(0.0, 255.0) as u8,
    }
}

fn hsv_to_rgb(hue: f32) -> Rgb {
    let scaled = hue.rem_euclid(1.0) * 6.0;
    let sector = scaled.floor() as u8;
    let fraction = scaled - sector as f32;
    let up = (fraction * 255.0).round() as u8;
    let down = ((1.0 - fraction) * 255.0).round() as u8;

    match sector {
        0 => Rgb {
            red: 255,
            green: up,
            blue: 0,
        },
        1 => Rgb {
            red: down,
            green: 255,
            blue: 0,
        },
        2 => Rgb {
            red: 0,
            green: 255,
            blue: up,
        },
        3 => Rgb {
            red: 0,
            green: down,
            blue: 255,
        },
        4 => Rgb {
            red: up,
            green: 0,
            blue: 255,
        },
        _ => Rgb {
            red: 255,
            green: 0,
            blue: down,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb {
        red: 255,
        green: 0,
        blue: 0,
    };
    const BLUE: Rgb = Rgb {
        red: 0,
        green: 0,
        blue: 255,
    };

    fn topology(led_count: usize) -> Topology {
        Topology::new(
            (0..led_count)
                .map(|id| {
                    (
                        crate::core::LedInfo { id, zone_id: 0 },
                        crate::core::Position {
                            x: id as f32 / led_count.max(1) as f32,
                            y: 0.0,
                            z: 0.0,
                        },
                    )
                })
                .collect(),
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn solid_fills_every_led() {
        let topology = topology(3);
        let frame = render_frame(EffectKind::Solid, &topology, Duration::ZERO, RED, BLUE, 1.0);

        assert_eq!(
            frame,
            Frame::from_pixels(vec![
                Rgb {
                    red: 255,
                    green: 0,
                    blue: 0,
                };
                3
            ])
        );
    }

    #[test]
    fn rainbow_has_different_colors_across_the_array() {
        let topology = Topology::new(
            (0..20)
                .map(|id| {
                    (
                        crate::core::LedInfo { id, zone_id: 0 },
                        crate::core::Position {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    )
                })
                .collect(),
            vec![crate::core::Zone {
                id: 0,
                name: "ARGB".to_owned(),
                kind: crate::core::ZoneKind::Argb,
                offset: 0,
                capacity: 20,
                active_led_count: 20,
            }],
        )
        .unwrap();
        let frame = render_frame(
            EffectKind::Rainbow,
            &topology,
            Duration::ZERO,
            RED,
            BLUE,
            1.0,
        );

        assert_eq!(frame.pixels()[0].red, 255);
        assert_eq!(frame.pixels()[7].green, 255);
        assert_eq!(frame.pixels()[14].blue, 255);
    }

    #[test]
    fn rainbow_restarts_at_each_argb_zone_and_keeps_rgb_zones_in_phase() {
        let topology = Topology::new(
            vec![
                (
                    crate::core::LedInfo { id: 0, zone_id: 0 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo { id: 1, zone_id: 0 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo { id: 2, zone_id: 1 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo { id: 3, zone_id: 2 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ],
            vec![
                crate::core::Zone {
                    id: 0,
                    name: "ARGB".to_owned(),
                    kind: crate::core::ZoneKind::Argb,
                    offset: 0,
                    capacity: 2,
                    active_led_count: 2,
                },
                crate::core::Zone {
                    id: 1,
                    name: "ARGB2".to_owned(),
                    kind: crate::core::ZoneKind::Argb,
                    offset: 2,
                    capacity: 1,
                    active_led_count: 1,
                },
                crate::core::Zone {
                    id: 2,
                    name: "RGB".to_owned(),
                    kind: crate::core::ZoneKind::Rgb,
                    offset: 3,
                    capacity: 1,
                    active_led_count: 1,
                },
            ],
        )
        .unwrap();
        let frame = render_frame(
            EffectKind::Rainbow,
            &topology,
            Duration::ZERO,
            RED,
            BLUE,
            1.0,
        );

        assert_eq!(frame.pixels()[0], frame.pixels()[2]);
        assert_eq!(frame.pixels()[0], frame.pixels()[3]);
        assert_ne!(frame.pixels()[0], frame.pixels()[1]);
    }

    #[test]
    fn rainbow_options_apply_brightness_and_reverse_direction() {
        let topology = topology(2);
        let forward = render_frame_with_options(
            EffectKind::Rainbow,
            &topology,
            Duration::ZERO,
            RED,
            BLUE,
            1.0,
            RenderOptions {
                brightness: 0.5,
                reverse: false,
            },
        );
        let reverse = render_frame_with_options(
            EffectKind::Rainbow,
            &topology,
            Duration::ZERO,
            RED,
            BLUE,
            1.0,
            RenderOptions {
                brightness: 1.0,
                reverse: true,
            },
        );

        assert_eq!(
            forward.pixels()[0],
            Rgb {
                red: 128,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            reverse.pixels()[1],
            Rgb {
                red: 0,
                green: 255,
                blue: 255
            }
        );
    }

    #[test]
    fn wave_uses_both_colors() {
        let topology = topology(4);
        let frame = render_frame(EffectKind::Wave, &topology, Duration::ZERO, RED, BLUE, 1.0);

        assert_eq!(frame.pixels()[0].red, 255);
        assert_eq!(frame.pixels()[0].blue, 0);
        assert_eq!(frame.pixels()[2].red, 0);
        assert_eq!(frame.pixels()[2].blue, 255);
    }

    #[test]
    fn wave_restarts_at_argb_zones_and_keeps_rgb_zones_in_phase() {
        let topology = Topology::new(
            vec![
                (
                    crate::core::LedInfo { id: 0, zone_id: 0 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo { id: 10, zone_id: 0 },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo {
                        id: 100,
                        zone_id: 1,
                    },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    crate::core::LedInfo {
                        id: 101,
                        zone_id: 2,
                    },
                    crate::core::Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ],
            vec![
                crate::core::Zone {
                    id: 0,
                    name: "ARGB".to_owned(),
                    kind: crate::core::ZoneKind::Argb,
                    offset: 0,
                    capacity: 100,
                    active_led_count: 2,
                },
                crate::core::Zone {
                    id: 1,
                    name: "ARGB2".to_owned(),
                    kind: crate::core::ZoneKind::Argb,
                    offset: 100,
                    capacity: 1,
                    active_led_count: 1,
                },
                crate::core::Zone {
                    id: 2,
                    name: "RGB".to_owned(),
                    kind: crate::core::ZoneKind::Rgb,
                    offset: 101,
                    capacity: 1,
                    active_led_count: 1,
                },
            ],
        )
        .unwrap();
        let frame = render_frame(EffectKind::Wave, &topology, Duration::ZERO, RED, BLUE, 1.0);

        assert_eq!(frame.pixels()[0], frame.pixels()[2]);
        assert_eq!(frame.pixels()[0], frame.pixels()[3]);
        assert_ne!(frame.pixels()[0], frame.pixels()[1]);
    }

    #[test]
    fn cycle_changes_over_time() {
        let topology = topology(2);
        let first = render_frame(EffectKind::Cycle, &topology, Duration::ZERO, RED, BLUE, 1.0);
        let later = render_frame(
            EffectKind::Cycle,
            &topology,
            Duration::from_millis(250),
            RED,
            BLUE,
            1.0,
        );

        assert_eq!(first.pixels()[0], first.pixels()[1]);
        assert_ne!(first.pixels()[0], later.pixels()[0]);
    }

    #[test]
    fn cycle_reverse_changes_color_wheel_direction() {
        let topology = topology(1);
        let forward = render_frame_with_options(
            EffectKind::Cycle,
            &topology,
            Duration::from_millis(250),
            RED,
            BLUE,
            1.0,
            RenderOptions {
                brightness: 1.0,
                reverse: false,
            },
        );
        let reverse = render_frame_with_options(
            EffectKind::Cycle,
            &topology,
            Duration::from_millis(250),
            RED,
            BLUE,
            1.0,
            RenderOptions {
                brightness: 1.0,
                reverse: true,
            },
        );

        assert_ne!(forward.pixels()[0], reverse.pixels()[0]);
    }
}
