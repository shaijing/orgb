use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapabilities {
    pub direct_rgb: bool,
    pub per_led: bool,
    pub max_leds: usize,
    pub min_update_interval: Duration,
    pub supports_readback: bool,
}
