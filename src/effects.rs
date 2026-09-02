use clap::ValueEnum;
use std::time::Duration;

use crate::core::{Frame, Rgb, Topology};

const TAU: f32 = std::f32::consts::TAU;

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

pub fn render_frame(
    kind: EffectKind,
    topology: &Topology,
    elapsed: Duration,
    primary: Rgb,
    secondary: Rgb,
    speed: f32,
) -> Frame {
    let time = elapsed.as_secs_f32();

    let pixels = topology
        .leds()
        .iter()
        .map(|(_, layout)| {
            let position = layout.x;

            match kind {
                EffectKind::Solid => primary,
                EffectKind::Rainbow => hsv_to_rgb((position + time * speed).rem_euclid(1.0)),
                EffectKind::Breathing => scale_color(
                    primary,
                    0.1 + 0.9 * (0.5 + 0.5 * (time * speed * TAU).cos()),
                ),
                EffectKind::Wave => {
                    let phase = (position + time * speed).rem_euclid(1.0);
                    let blend = 0.5 - 0.5 * (phase * TAU).cos();
                    blend_colors(primary, secondary, blend)
                }
                EffectKind::Cycle => hsv_to_rgb((time * speed).rem_euclid(1.0)),
            }
        })
        .collect();

    Frame::from_pixels(pixels)
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
        let topology = topology(6);
        let frame = render_frame(
            EffectKind::Rainbow,
            &topology,
            Duration::ZERO,
            RED,
            BLUE,
            1.0,
        );

        assert_eq!(frame.pixels()[0].red, 255);
        assert_eq!(frame.pixels()[2].green, 255);
        assert_eq!(frame.pixels()[4].blue, 255);
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
}
