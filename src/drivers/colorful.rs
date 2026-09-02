use anyhow::{Result, ensure};

use crate::config::ProfileCatalog;
use crate::core::{BoardProfile, Frame, Rgb, RgbDevice, Topology};
use crate::protocol::ProtocolCodec;
use crate::smbios::BoardIdentity;
use crate::transport::{HidFeatureTransport, Transport};

const REPORT_ID: u8 = 0x01;
const COMMAND: u8 = 0x88;
const REPORT_SIZE: usize = 604;
const LEDS_PER_PAGE: usize = 200;

#[derive(Debug, Default, Clone, Copy)]
pub struct Colorful088Codec;

impl Colorful088Codec {
    fn make_report(index: u8, pixels: &[Rgb]) -> Result<Vec<u8>> {
        let mut report = vec![0u8; REPORT_SIZE];
        report[0] = REPORT_ID;
        report[2] = COMMAND;
        report[3] = index;

        match index {
            0x00..=0xfe => {
                ensure!(
                    !pixels.is_empty() && pixels.len() <= LEDS_PER_PAGE,
                    "RGB page {index:02x} requires 1..={LEDS_PER_PAGE} LEDs, got {}",
                    pixels.len()
                );
                Self::write_pixels(&mut report, pixels);
            }
            0xff => ensure!(pixels.is_empty(), "RGB commit page must be empty"),
        }

        Ok(report)
    }

    fn write_pixels(report: &mut [u8], pixels: &[Rgb]) {
        for (slot, pixel) in pixels.iter().enumerate() {
            let offset = 4 + slot * 3;
            report[offset] = pixel.red;
            report[offset + 1] = pixel.green;
            report[offset + 2] = pixel.blue;
        }
    }
}

impl ProtocolCodec for Colorful088Codec {
    fn encode_frame(&self, frame: &Frame) -> Result<Vec<Vec<u8>>> {
        ensure!(!frame.pixels().is_empty(), "RGB frame must contain LEDs");

        let page_count = frame.len().div_ceil(LEDS_PER_PAGE);
        ensure!(
            page_count <= 0xff,
            "RGB frame has too many pages: {page_count}"
        );

        let mut packets = Vec::with_capacity(page_count + 1);
        for (index, chunk) in frame.pixels().chunks(LEDS_PER_PAGE).enumerate() {
            packets.push(Self::make_report(index as u8, chunk)?);
        }
        packets.push(Self::make_report(0xff, &[])?);
        Ok(packets)
    }
}

pub struct ColorfulDevice {
    profile: BoardProfile,
    topology: Topology,
    transport: HidFeatureTransport,
    codec: Colorful088Codec,
}

pub struct ColorfulFamilyDriver {
    profile: BoardProfile,
}

impl ColorfulFamilyDriver {
    pub fn load_for_identity(
        config_dir: impl AsRef<std::path::Path>,
        identity: &BoardIdentity,
        requested: Option<&str>,
    ) -> Result<Self> {
        Self::load_matching_profile(config_dir, requested, Some(identity))
    }

    pub fn load_profile(config_dir: impl AsRef<std::path::Path>, requested: &str) -> Result<Self> {
        Self::load_matching_profile(config_dir, Some(requested), None)
    }

    fn load_matching_profile(
        config_dir: impl AsRef<std::path::Path>,
        requested: Option<&str>,
        identity: Option<&crate::smbios::BoardIdentity>,
    ) -> Result<Self> {
        let profile = ProfileCatalog::load(config_dir)?.select(
            requested,
            crate::core::BrandId::Colorful,
            identity,
        )?;
        ensure!(
            profile.protocol == crate::core::ProtocolKind::Colorful088,
            "Colorful driver does not support protocol {:?}",
            profile.protocol
        );
        Ok(Self { profile })
    }

    pub fn profile(&self) -> &BoardProfile {
        &self.profile
    }

    pub fn topology(&self) -> Result<Topology> {
        self.profile.topology()
    }

    pub async fn open(self) -> Result<ColorfulDevice> {
        let usb = self.profile.usb_match;
        let transport =
            HidFeatureTransport::open(usb.vendor_id, usb.product_id, usb.interface).await?;
        let topology = self.profile.topology()?;
        Ok(ColorfulDevice {
            profile: self.profile,
            topology,
            transport,
            codec: Colorful088Codec,
        })
    }
}

impl RgbDevice for ColorfulDevice {
    fn profile(&self) -> &BoardProfile {
        &self.profile
    }

    fn topology(&self) -> &Topology {
        &self.topology
    }

    fn capabilities(&self) -> &crate::core::DeviceCapabilities {
        &self.profile.capabilities
    }

    async fn submit(&mut self, frame: &Frame) -> Result<()> {
        ensure!(
            frame.len() == self.topology.led_count(),
            "board profile requires {} LEDs, got {}",
            self.topology.led_count(),
            frame.len()
        );
        for packet in self.codec.encode_frame(frame)? {
            self.transport.write(&packet).await?;
        }
        Ok(())
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
    fn codec_maps_all_602_leds_to_five_packets() {
        let driver =
            ColorfulFamilyDriver::load_profile("configs/colorful", "battle-ax_b860m-plus_s_wifi7")
                .unwrap();
        let frame = Frame::solid(driver.topology().unwrap().led_count(), RED);
        let packets = Colorful088Codec.encode_frame(&frame).unwrap();

        assert_eq!(packets.len(), 5);
        assert_eq!(
            packets.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![604; 5]
        );
        assert_eq!(&packets[0][4..7], &[255, 0, 0]);
        assert_eq!(&packets[3][4..7], &[255, 0, 0]);
        assert_eq!(packets[4][3], 0xff);
        assert!(packets[4][4..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn topology_has_six_5v_and_two_12v_zones() {
        let driver =
            ColorfulFamilyDriver::load_profile("configs/colorful", "battle-ax_b860m-plus_s_wifi7")
                .unwrap();
        let topology = driver.topology().unwrap();

        assert_eq!(topology.led_count(), 602);
        assert_eq!(topology.zones().len(), 8);
        assert_eq!(topology.zones()[0].name, "5V_1");
        assert_eq!(topology.zones()[6].name, "12V_1");
    }

    #[test]
    fn codec_pages_follow_frame_length() {
        let frame = Frame::solid(302, RED);
        let packets = Colorful088Codec.encode_frame(&frame).unwrap();

        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0][3], 0x00);
        assert_eq!(packets[1][3], 0x01);
        assert_eq!(packets[2][3], 0xff);
    }
}
