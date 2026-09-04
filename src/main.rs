use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use orgb::core::{Rgb, RgbDevice};
use orgb::drivers::BoardDriver;
use orgb::effects::EffectKind;
use orgb::scheduler;
use orgb::smbios::read_board_identity;

#[derive(Parser, Debug)]
#[command(version, about = "Control Colorful motherboard RGB lighting")]
struct Cli {
    /// Directory containing board profile TOML files.
    #[arg(long, global = true, default_value = "configs/colorful")]
    config_dir: PathBuf,
    /// Board profile name or TOML file stem. Required when multiple profiles are installed.
    #[arg(long, global = true)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print device capabilities and logical LED zones.
    Info,
    /// Print the raw transport layout used by the Colorful driver.
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
    /// Run an animated effect through the scheduler.
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
        /// Output brightness, from 0.0 to 1.0.
        #[arg(long, default_value_t = 1.0)]
        brightness: f32,
        /// Reverse the spatial direction of the rainbow effect.
        #[arg(long)]
        reverse: bool,
        /// Requested frame rate. Device capabilities are always respected.
        #[arg(long)]
        fps: Option<f64>,
        /// Stop after this many seconds. Without it, run until interrupted.
        #[arg(long)]
        duration: Option<f64>,
    },
    /// Turn all LEDs off.
    Off,
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

fn print_info(driver: &BoardDriver) -> Result<()> {
    let profile = driver.profile();
    let topology = driver.topology()?;
    let capabilities = &profile.capabilities;

    println!("Brand: {:?}", profile.brand);
    println!("Board: {}", profile.model);
    println!(
        "Revision: {}",
        profile.revision.as_deref().unwrap_or("unknown")
    );
    println!(
        "USB: {:04x}:{:04x}, interface {}",
        profile.usb_match.vendor_id, profile.usb_match.product_id, profile.usb_match.interface
    );
    println!("LED count: {}", topology.led_count());
    println!("Zones:");
    for zone in topology.zones() {
        println!(
            "  {}: offset={}, capacity={}, active={}",
            zone.name, zone.offset, zone.capacity, zone.active_led_count
        );
    }
    let first_position = topology.leds().first().map(|(_, position)| position);
    let last_position = topology.leds().last().map(|(_, position)| position);
    if let (Some(first), Some(last)) = (first_position, last_position) {
        println!(
            "Logical position range: ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2})",
            first.x, first.y, first.z, last.x, last.y, last.z
        );
    }
    println!(
        "Capabilities: direct_rgb={}, per_led={}, max_leds={}, readback={}",
        capabilities.direct_rgb,
        capabilities.per_led,
        capabilities.max_leds,
        capabilities.supports_readback
    );
    println!(
        "Minimum frame interval: {} us",
        capabilities.min_update_interval.as_micros()
    );
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let identity = read_board_identity()?;
    let driver =
        BoardDriver::load_for_identity(&cli.config_dir, &identity, cli.profile.as_deref())?;

    match cli.command {
        Command::Info => print_info(&driver)?,
        Command::Probe => driver.print_probe(),
        Command::On {
            color,
            red,
            green,
            blue,
        } => {
            let color = resolve_on_color(color, red, green, blue)?;
            let mut device = driver.open().await?;
            device.set_color(color).await?;
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
            brightness,
            reverse,
            fps,
            duration,
        } => {
            let primary = parse_rgb(&color)?;
            let secondary = parse_rgb(&secondary)?;
            let mut device = driver.open().await?;
            scheduler::run(
                &mut device,
                scheduler::EffectConfig {
                    kind,
                    primary,
                    secondary,
                    speed,
                    brightness,
                    reverse,
                    fps,
                    duration,
                },
            )
            .await?;
        }
        Command::Off => {
            let mut device = driver.open().await?;
            device.set_color(Rgb::BLACK).await?;
            println!("Turned RGB off");
        }
    }

    Ok(())
}
