use crate::core::Rgb;

pub(super) fn hsv_to_rgb(hue: f32) -> Rgb {
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

pub(super) fn scale_color(color: Rgb, amount: f32) -> Rgb {
    Rgb {
        red: (color.red as f32 * amount).round().clamp(0.0, 255.0) as u8,
        green: (color.green as f32 * amount).round().clamp(0.0, 255.0) as u8,
        blue: (color.blue as f32 * amount).round().clamp(0.0, 255.0) as u8,
    }
}
