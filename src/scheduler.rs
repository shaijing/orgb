use anyhow::{Result, ensure};
use std::time::Duration;

use crate::core::Rgb;
use crate::core::RgbDevice;
use crate::effects::{EffectKind, RenderOptions, render_frame_with_options};

const DEFAULT_FPS: f64 = 20.0;

pub struct EffectConfig {
    pub kind: EffectKind,
    pub primary: Rgb,
    pub secondary: Rgb,
    pub speed: f32,
    pub brightness: f32,
    pub reverse: bool,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
}

fn frame_interval<D: RgbDevice>(device: &D, fps: Option<f64>) -> Result<Duration> {
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
        None => Duration::from_secs_f64(1.0 / DEFAULT_FPS),
    };

    Ok(requested.max(device.capabilities().min_update_interval))
}

pub async fn run<D: RgbDevice>(device: &mut D, config: EffectConfig) -> Result<()> {
    ensure!(
        config.speed.is_finite() && config.speed >= 0.0,
        "--speed must be a non-negative number"
    );
    ensure!(
        config.brightness.is_finite() && (0.0..=1.0).contains(&config.brightness),
        "--brightness must be between 0.0 and 1.0"
    );

    let interval = frame_interval(device, config.fps)?;
    let stop_after = match config.duration {
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
        "Running {:?} effect on {} ({:04x}:{:04x}), {} LEDs at approximately {:.1} FPS",
        config.kind,
        device.profile().model,
        device.profile().usb_match.vendor_id,
        device.profile().usb_match.product_id,
        device.topology().led_count(),
        1.0 / interval.as_secs_f64()
    );
    if stop_after.is_none() {
        println!("Press Ctrl+C to stop.");
    }

    let started = tokio::time::Instant::now();
    let mut next_frame = started;
    let mut frames_sent = 0u64;

    loop {
        let elapsed = started.elapsed();
        if frames_sent > 0 && stop_after.is_some_and(|limit| elapsed >= limit) {
            break;
        }

        let frame = render_frame_with_options(
            config.kind,
            device.topology(),
            elapsed,
            config.primary,
            config.secondary,
            config.speed,
            RenderOptions {
                brightness: config.brightness,
                reverse: config.reverse,
            },
        );
        device.submit(&frame).await?;
        frames_sent += 1;

        let now = tokio::time::Instant::now();
        if next_frame > now {
            tokio::time::sleep_until(next_frame).await;
        } else {
            next_frame = now;
        }
        next_frame += interval;
    }

    println!("Effect stopped after {frames_sent} frames.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        BoardProfile, BrandId, DeviceCapabilities, Frame, LedInfo, Position, ProtocolKind,
        Topology, UsbMatch,
    };

    struct FakeDevice {
        profile: BoardProfile,
        topology: Topology,
        capabilities: DeviceCapabilities,
        submitted_frames: usize,
    }

    impl FakeDevice {
        fn new() -> Self {
            let topology = Topology::new(
                vec![
                    (
                        LedInfo { id: 0, zone_id: 0 },
                        Position {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    ),
                    (
                        LedInfo { id: 1, zone_id: 0 },
                        Position {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    ),
                ],
                Vec::new(),
            )
            .unwrap();
            Self {
                profile: BoardProfile {
                    brand: BrandId::Colorful,
                    model: "fake".to_owned(),
                    revision: None,
                    usb_match: UsbMatch {
                        vendor_id: 0,
                        product_id: 0,
                        interface: 0,
                    },
                    protocol: ProtocolKind::Colorful088,
                    zones: Vec::new(),
                    capabilities: DeviceCapabilities {
                        direct_rgb: true,
                        per_led: true,
                        max_leds: 2,
                        min_update_interval: Duration::ZERO,
                        supports_readback: false,
                    },
                },
                topology,
                capabilities: DeviceCapabilities {
                    direct_rgb: true,
                    per_led: true,
                    max_leds: 2,
                    min_update_interval: Duration::ZERO,
                    supports_readback: false,
                },
                submitted_frames: 0,
            }
        }
    }

    impl RgbDevice for FakeDevice {
        fn profile(&self) -> &BoardProfile {
            &self.profile
        }

        fn topology(&self) -> &Topology {
            &self.topology
        }

        fn capabilities(&self) -> &DeviceCapabilities {
            &self.capabilities
        }

        async fn submit(&mut self, frame: &Frame) -> Result<()> {
            assert_eq!(frame.len(), self.topology.led_count());
            self.submitted_frames += 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn scheduler_can_render_to_any_rgb_device() {
        let mut device = FakeDevice::new();
        run(
            &mut device,
            EffectConfig {
                kind: EffectKind::Solid,
                primary: Rgb::BLACK,
                secondary: Rgb::BLACK,
                speed: 0.0,
                brightness: 1.0,
                reverse: false,
                fps: None,
                duration: Some(0.0),
            },
        )
        .await
        .unwrap();

        assert_eq!(device.submitted_frames, 1);
    }

    #[test]
    fn default_frame_interval_targets_twenty_fps() {
        let device = FakeDevice::new();

        let interval = frame_interval(&device, None).unwrap();

        assert_eq!(interval, Duration::from_millis(50));
    }

    #[test]
    fn frame_interval_respects_device_minimum() {
        let mut device = FakeDevice::new();
        device.capabilities.min_update_interval = Duration::from_millis(100);

        let interval = frame_interval(&device, Some(60.0)).unwrap();

        assert_eq!(interval, Duration::from_millis(100));
    }
}
