use anyhow::{Context, Result, anyhow, bail, ensure};
use rusb::{DeviceHandle, GlobalContext};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::lighting::{Frame, LightingBackend, Rgb};

const VID: u16 = 0x2f4c;
const PID: u16 = 0x1024;

const RGB_IFACE: u8 = 1;
const RGB_REPORT_ID: u8 = 0x01;
const RGB_COMMAND: u8 = 0x88;
const RGB_REPORT_SIZE: usize = 604;
const LED_COUNT: usize = 602;
const LEDS_PER_PAGE: usize = 200;
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const FRAME_PAGES: [(u8, usize, usize); 4] = [
    (0x00, 0, 200),
    (0x01, 200, 400),
    (0x02, 400, 600),
    (0x03, 600, 602),
];

pub struct ColorfulBackend {
    device: Arc<Mutex<DeviceHandle<GlobalContext>>>,
}

impl ColorfulBackend {
    pub async fn open() -> Result<Self> {
        tokio::task::spawn_blocking(Self::open_blocking)
            .await
            .context("failed to join RGB backend setup task")?
    }

    fn open_blocking() -> Result<Self> {
        let device = rusb::open_device_with_vid_pid(VID, PID)
            .context("Colorful RGB controller 2f4c:1024 not found")?;

        let _ = device.set_auto_detach_kernel_driver(true);
        device
            .claim_interface(RGB_IFACE)
            .with_context(|| format!("failed to claim RGB HID interface {RGB_IFACE}"))?;

        Ok(Self {
            device: Arc::new(Mutex::new(device)),
        })
    }

    pub fn led_count() -> usize {
        LED_COUNT
    }

    pub fn frame_interval() -> Duration {
        FRAME_INTERVAL
    }

    fn send_feature(device: &DeviceHandle<GlobalContext>, report: &[u8]) -> Result<()> {
        ensure!(
            report.first() == Some(&RGB_REPORT_ID),
            "RGB feature report must start with report id {RGB_REPORT_ID}"
        );

        let request_type = 0x21; // Host -> Device, Class, Interface
        let value = (3u16 << 8) | RGB_REPORT_ID as u16; // HID feature report
        let written = device
            .write_control(
                request_type,
                0x09, // SET_REPORT
                value,
                RGB_IFACE as u16,
                report,
                Duration::from_secs(1),
            )
            .context("failed to send RGB feature report")?;

        ensure!(
            written == report.len(),
            "short RGB feature report write: wrote {written} of {} bytes",
            report.len()
        );
        Ok(())
    }

    fn make_report(index: u8, pixels: &[Rgb]) -> Result<[u8; RGB_REPORT_SIZE]> {
        let mut report = [0u8; RGB_REPORT_SIZE];
        report[0] = RGB_REPORT_ID;
        report[2] = RGB_COMMAND;
        report[3] = index;

        match index {
            0x00..=0x02 => {
                ensure!(
                    pixels.len() == LEDS_PER_PAGE,
                    "RGB page {index:02x} requires {LEDS_PER_PAGE} LEDs, got {}",
                    pixels.len()
                );
                Self::write_pixels(&mut report, pixels);
            }
            0x03 => {
                ensure!(
                    pixels.len() == 2,
                    "RGB page 03 requires 2 LEDs, got {}",
                    pixels.len()
                );
                Self::write_pixels(&mut report, pixels);
            }
            0xff => ensure!(
                pixels.is_empty(),
                "RGB commit page must not contain LED data"
            ),
            _ => bail!("unexpected RGB page index 0x{index:02x}"),
        }

        Ok(report)
    }

    fn write_pixels(report: &mut [u8; RGB_REPORT_SIZE], pixels: &[Rgb]) {
        for (slot, pixel) in pixels.iter().enumerate() {
            let offset = 4 + slot * 3;
            report[offset] = pixel.red;
            report[offset + 1] = pixel.green;
            report[offset + 2] = pixel.blue;
        }
    }
}

impl LightingBackend for ColorfulBackend {
    fn led_count(&self) -> usize {
        LED_COUNT
    }

    fn min_frame_interval(&self) -> Duration {
        FRAME_INTERVAL
    }

    async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        ensure!(
            frame.len() == LED_COUNT,
            "Colorful backend requires {LED_COUNT} LEDs, got {}",
            frame.len()
        );

        let mut reports = Vec::with_capacity(FRAME_PAGES.len() + 1);
        for (index, start, end) in FRAME_PAGES {
            let report = Self::make_report(index, &frame.pixels()[start..end])?;
            reports.push((index, report));
        }

        let commit = Self::make_report(0xff, &[])?;
        reports.push((0xff, commit));

        let device = Arc::clone(&self.device);
        tokio::task::spawn_blocking(move || {
            let device = device
                .lock()
                .map_err(|_| anyhow!("RGB backend device lock is poisoned"))?;
            for (index, report) in reports {
                Self::send_feature(&device, &report)
                    .with_context(|| format!("failed to send RGB page 0x{index:02x}"))?;
            }
            Ok(())
        })
        .await
        .context("failed to join RGB frame task")?
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

    #[test]
    fn page_report_contains_each_rgb_pixel() {
        let pixels = vec![RED; LEDS_PER_PAGE];
        let report = ColorfulBackend::make_report(0x00, &pixels).unwrap();

        assert_eq!(&report[0..4], &[RGB_REPORT_ID, 0, RGB_COMMAND, 0]);
        assert_eq!(&report[4..7], &[255, 0, 0]);
        assert_eq!(&report[601..604], &[255, 0, 0]);
    }

    #[test]
    fn commit_report_has_no_led_data() {
        let report = ColorfulBackend::make_report(0xff, &[]).unwrap();

        assert_eq!(&report[0..4], &[RGB_REPORT_ID, 0, RGB_COMMAND, 0xff]);
        assert!(report[4..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn page_lengths_are_validated() {
        assert!(ColorfulBackend::make_report(0x00, &[]).is_err());
        assert!(ColorfulBackend::make_report(0x03, &[RED]).is_err());
    }
}
