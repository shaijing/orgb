use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::time::{Duration, Instant};

mod backend;
mod effects;
mod lighting;

use backend::ColorfulBackend;
use effects::{EffectKind, render_frame};
use lighting::{LightingBackend, Rgb};

#[derive(Parser, Debug)]
#[command(version, about = "Control Colorful motherboard RGB lighting")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the backend and framebuffer layout.
    Info,
    /// Print the raw transport layout used by the Colorful backend.
    Probe,
    /// Set all LEDs to one color.
    On {
        /// Color as #RRGGBB, RRGGBB, or R,G,B. Defaults to #FF0000.
        color: Option<String>,
        /// Red channel, 0..255.
        #[arg(long)]
        red: Option<u8>,
        /// Green channel, 0..255.
        #[arg(long)]
        green: Option<u8>,
        /// Blue channel, 0..255.
        #[arg(long)]
        blue: Option<u8>,
    },
    /// Run an animated effect through the lighting backend.
    Effect {
        /// Effect name: solid, rainbow, breathing, wave, or cycle.
        #[arg(value_enum)]
        kind: EffectKind,
        /// Primary color as #RRGGBB, RRGGBB, or R,G,B.
        #[arg(long, default_value = "#FF0000")]
        color: String,
        /// Secondary color used by the wave effect.
        #[arg(long, default_value = "#0000FF")]
        secondary: String,
        /// Animation cycles per second.
        #[arg(long, default_value_t = 0.2)]
        speed: f32,
        /// Requested frame rate. The backend minimum interval is always respected.
        #[arg(long)]
        fps: Option<f64>,
        /// Stop after this many seconds. Without it, run until interrupted.
        #[arg(long)]
        duration: Option<f64>,
    },
    /// Turn all LEDs off.
    Off,
}

struct EffectOptions {
    kind: EffectKind,
    primary: Rgb,
    secondary: Rgb,
    speed: f32,
    fps: Option<f64>,
    duration: Option<f64>,
}

fn parse_rgb(value: &str) -> Result<Rgb> {
    let value = value.trim();

    if value.contains(',') {
        let parts = value.split(',').collect::<Vec<_>>();
        ensure!(parts.len() == 3, "RGB format should be R,G,B");

        let red = parts[0]
            .trim()
            .parse::<u8>()
            .context("invalid red channel")?;
        let green = parts[1]
            .trim()
            .parse::<u8>()
            .context("invalid green channel")?;
        let blue = parts[2]
            .trim()
            .parse::<u8>()
            .context("invalid blue channel")?;

        return Ok(Rgb { red, green, blue });
    }

    let value = value.strip_prefix('#').unwrap_or(value);
    ensure!(
        value.len() == 6,
        "HEX color should look like FF0000 or #FF0000"
    );

    let red = u8::from_str_radix(&value[0..2], 16).context("invalid red hex channel")?;
    let green = u8::from_str_radix(&value[2..4], 16).context("invalid green hex channel")?;
    let blue = u8::from_str_radix(&value[4..6], 16).context("invalid blue hex channel")?;

    Ok(Rgb { red, green, blue })
}

fn resolve_on_color(
    color: Option<String>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
) -> Result<Rgb> {
    ensure!(
        color.is_none() || (red.is_none() && green.is_none() && blue.is_none()),
        "use either a positional color or --red/--green/--blue, not both"
    );

    if let Some(color) = color {
        return parse_rgb(&color);
    }

    if red.is_some() || green.is_some() || blue.is_some() {
        return Ok(Rgb {
            red: red.unwrap_or(0),
            green: green.unwrap_or(0),
            blue: blue.unwrap_or(0),
        });
    }

    parse_rgb("#FF0000")
}

fn frame_interval<B: LightingBackend>(backend: &B, fps: Option<f64>) -> Result<Duration> {
    let requested = match fps {
        Some(fps) => {
            ensure!(
                fps.is_finite() && fps > 0.0,
                "--fps must be a positive number"
            );
            let seconds = 1.0 / fps;
            ensure!(
                seconds <= Duration::MAX.as_secs_f64(),
                "--fps is too small to represent a frame interval"
            );
            Duration::from_secs_f64(seconds)
        }
        None => Duration::from_millis(16),
    };

    Ok(requested.max(backend.min_frame_interval()))
}

fn run_effect<B: LightingBackend>(backend: &mut B, options: EffectOptions) -> Result<()> {
    ensure!(
        options.speed.is_finite() && options.speed >= 0.0,
        "--speed must be a non-negative number"
    );

    let interval = frame_interval(backend, options.fps)?;
    let stop_after = match options.duration {
        Some(seconds) => {
            ensure!(
                seconds.is_finite() && seconds >= 0.0,
                "--duration must be a non-negative number"
            );
            ensure!(
                seconds <= Duration::MAX.as_secs_f64(),
                "--duration is too large"
            );
            Some(Duration::from_secs_f64(seconds))
        }
        None => None,
    };

    println!(
        "Running {:?} effect on {} LEDs at approximately {:.1} FPS",
        options.kind,
        backend.led_count(),
        1.0 / interval.as_secs_f64()
    );
    if stop_after.is_none() {
        println!("Press Ctrl+C to stop.");
    }

    let started = Instant::now();
    let mut next_frame = started;
    let mut frames_sent = 0u64;

    loop {
        let elapsed = started.elapsed();
        if frames_sent > 0 && stop_after.is_some_and(|limit| elapsed >= limit) {
            break;
        }

        let frame = render_frame(
            options.kind,
            backend.led_count(),
            elapsed,
            options.primary,
            options.secondary,
            options.speed,
        );
        backend.send_frame(&frame)?;
        frames_sent += 1;

        let now = Instant::now();
        if next_frame > now {
            std::thread::sleep(next_frame.duration_since(now));
        } else {
            next_frame = now;
        }
        next_frame += interval;
    }

    println!("Effect stopped after {frames_sent} frames.");
    Ok(())
}

fn print_info() {
    println!("Backend: Colorful HID framebuffer");
    println!("LED count: {}", ColorfulBackend::led_count());
    println!("Framebuffer: 6 pages x 100 LEDs + 2 LEDs");
    println!("Pages: 0x00, 0x01, 0x02, 0x03; commit: 0xff");
    println!(
        "Minimum frame interval: {} us",
        ColorfulBackend::frame_interval().as_micros()
    );
}

fn print_probe() {
    println!("Transport: HID feature report on interface 1");
    println!("Command: 0x88");
    println!("Report size: 604 bytes");
    println!("RGB payload: 600 + 2 LEDs across pages 0x00..0x03");
    println!("Commit: page 0xff with an empty payload");
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Info => print_info(),
        Command::Probe => print_probe(),
        Command::On {
            color,
            red,
            green,
            blue,
        } => {
            let color = resolve_on_color(color, red, green, blue)?;
            let mut backend = ColorfulBackend::open()?;
            backend.set_color(color)?;
            println!(
                "Set RGB to ({}, {}, {}) #{:02X}{:02X}{:02X}",
                color.red, color.green, color.blue, color.red, color.green, color.blue
            );
        }
        Command::Effect {
            kind,
            color,
            secondary,
            speed,
            fps,
            duration,
        } => {
            let primary = parse_rgb(&color)?;
            let secondary = parse_rgb(&secondary)?;
            let mut backend = ColorfulBackend::open()?;
            run_effect(
                &mut backend,
                EffectOptions {
                    kind,
                    primary,
                    secondary,
                    speed,
                    fps,
                    duration,
                },
            )?;
        }
        Command::Off => {
            let mut backend = ColorfulBackend::open()?;
            backend.set_color(Rgb::BLACK)?;
            println!("Turned RGB off");
        }
    }

    Ok(())
}
