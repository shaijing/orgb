use anyhow::{Context, Result, anyhow, ensure};
use rusb::{DeviceHandle, GlobalContext};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[allow(async_fn_in_trait)]
pub trait Transport: Send + Sync {
    async fn write(&self, data: &[u8]) -> Result<()>;
}

pub struct HidFeatureTransport {
    device: Arc<Mutex<DeviceHandle<GlobalContext>>>,
    interface: u8,
}

impl HidFeatureTransport {
    pub async fn open(vendor_id: u16, product_id: u16, interface: u8) -> Result<Self> {
        tokio::task::spawn_blocking(move || Self::open_blocking(vendor_id, product_id, interface))
            .await
            .context("failed to join HID transport setup task")?
    }

    fn open_blocking(vendor_id: u16, product_id: u16, interface: u8) -> Result<Self> {
        let device = rusb::open_device_with_vid_pid(vendor_id, product_id).with_context(|| {
            format!("RGB controller {vendor_id:04x}:{product_id:04x} not found")
        })?;

        let _ = device.set_auto_detach_kernel_driver(true);
        device
            .claim_interface(interface)
            .with_context(|| format!("failed to claim HID interface {interface}"))?;

        Ok(Self {
            device: Arc::new(Mutex::new(device)),
            interface,
        })
    }

    fn write_blocking(
        device: &DeviceHandle<GlobalContext>,
        interface: u8,
        data: &[u8],
    ) -> Result<()> {
        let written = device
            .write_control(
                0x21,      // Host -> Device, Class, Interface
                0x09,      // SET_REPORT
                3u16 << 8, // HID feature report
                interface as u16,
                data,
                Duration::from_secs(1),
            )
            .context("failed to write HID feature report")?;

        ensure!(
            written == data.len(),
            "short HID feature report write: wrote {written} of {} bytes",
            data.len()
        );
        Ok(())
    }
}

impl Transport for HidFeatureTransport {
    async fn write(&self, data: &[u8]) -> Result<()> {
        let device = Arc::clone(&self.device);
        let interface = self.interface;
        let data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            let device = device
                .lock()
                .map_err(|_| anyhow!("HID transport device lock is poisoned"))?;
            Self::write_blocking(&device, interface, &data)
        })
        .await
        .context("failed to join HID write task")?
    }
}
