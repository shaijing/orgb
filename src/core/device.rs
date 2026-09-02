use anyhow::Result;

use super::capabilities::DeviceCapabilities;
use super::frame::{Frame, Rgb};
use super::topology::Topology;
use super::zone::ZoneProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandId {
    Colorful,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    Colorful088,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbMatch {
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardProfile {
    pub brand: BrandId,
    pub model: String,
    pub revision: Option<String>,
    pub usb_match: UsbMatch,
    pub protocol: ProtocolKind,
    pub zones: Vec<ZoneProfile>,
    pub capabilities: DeviceCapabilities,
}

impl BoardProfile {
    pub fn topology(&self) -> Result<Topology> {
        Topology::from_profiles(&self.zones)
    }
}

#[allow(async_fn_in_trait)]
pub trait RgbDevice: Send {
    fn profile(&self) -> &BoardProfile;

    fn topology(&self) -> &Topology;

    fn capabilities(&self) -> &DeviceCapabilities;

    async fn submit(&mut self, frame: &Frame) -> Result<()>;

    async fn set_color(&mut self, color: Rgb) -> Result<()> {
        let frame = Frame::solid(self.topology().led_count(), color);
        self.submit(&frame).await
    }
}
