pub mod colorful;

use anyhow::{Result, bail};
use std::path::Path;

use crate::core::{BoardProfile, RgbDevice, Topology};
use crate::drivers::colorful::{ColorfulDevice, ColorfulFamilyDriver};
use crate::smbios::BoardIdentity;

pub enum BoardDriver {
    Colorful(ColorfulFamilyDriver),
}

pub enum BoardDevice {
    Colorful(ColorfulDevice),
}

impl BoardDriver {
    pub fn load_for_identity(
        config_dir: impl AsRef<Path>,
        identity: &BoardIdentity,
        requested_profile: Option<&str>,
    ) -> Result<Self> {
        if vendor_is_colorful(&identity.vendor) {
            return Ok(Self::Colorful(ColorfulFamilyDriver::load_for_identity(
                config_dir,
                identity,
                requested_profile,
            )?));
        }

        bail!(
            "unsupported SMBIOS board vendor {:?} for board {:?}",
            identity.vendor,
            identity.model
        );
    }

    pub fn profile(&self) -> &BoardProfile {
        match self {
            Self::Colorful(driver) => driver.profile(),
        }
    }

    pub fn topology(&self) -> Result<Topology> {
        match self {
            Self::Colorful(driver) => driver.topology(),
        }
    }

    pub async fn open(self) -> Result<BoardDevice> {
        match self {
            Self::Colorful(driver) => Ok(BoardDevice::Colorful(driver.open().await?)),
        }
    }

    pub fn print_probe(&self) {
        match self {
            Self::Colorful(_) => {
                println!("Transport: HID feature report on interface 1");
                println!("Protocol: Colorful 0x88 framebuffer");
                println!("Report size: 604 bytes");
                println!("RGB payload: 600 + 2 LEDs across pages 0x00..0x03");
                println!("Commit: page 0xff with an empty payload");
            }
        }
    }
}

impl RgbDevice for BoardDevice {
    fn profile(&self) -> &BoardProfile {
        match self {
            Self::Colorful(device) => device.profile(),
        }
    }

    fn topology(&self) -> &Topology {
        match self {
            Self::Colorful(device) => device.topology(),
        }
    }

    fn capabilities(&self) -> &crate::core::DeviceCapabilities {
        match self {
            Self::Colorful(device) => device.capabilities(),
        }
    }

    async fn submit(&mut self, frame: &crate::core::Frame) -> Result<()> {
        match self {
            Self::Colorful(device) => device.submit(frame).await,
        }
    }
}

fn vendor_is_colorful(vendor: &str) -> bool {
    normalize(vendor).contains("colorful")
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
