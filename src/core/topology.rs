use anyhow::{Result, ensure};

use super::lamp::{LedInfo, Position};
use super::zone::{Zone, ZoneProfile};

#[derive(Debug, Clone, PartialEq)]
pub struct Topology {
    leds: Vec<(LedInfo, Position)>,
    zones: Vec<Zone>,
}

impl Topology {
    pub fn new(leds: Vec<(LedInfo, Position)>, zones: Vec<Zone>) -> Result<Self> {
        ensure!(!leds.is_empty(), "topology must contain at least one LED");
        for zone in &zones {
            ensure!(
                zone.active_led_count <= zone.capacity,
                "zone {} has more active LEDs than its capacity",
                zone.name
            );
        }
        Ok(Self { leds, zones })
    }

    pub fn led_count(&self) -> usize {
        self.leds.len()
    }

    pub fn leds(&self) -> &[(LedInfo, Position)] {
        &self.leds
    }

    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    pub(crate) fn from_profiles(profiles: &[ZoneProfile]) -> Result<Self> {
        let total_capacity = profiles.iter().map(|zone| zone.capacity).sum::<usize>();
        ensure!(
            total_capacity > 0,
            "board profile must define at least one LED"
        );

        let last_led = total_capacity.saturating_sub(1).max(1) as f32;
        let mut offset = 0;
        let mut leds = Vec::new();
        let mut zones = Vec::with_capacity(profiles.len());

        for (zone_id, profile) in profiles.iter().enumerate() {
            ensure!(
                profile.active_led_count <= profile.capacity,
                "zone {} has more active LEDs than its capacity",
                profile.name
            );
            zones.push(Zone {
                id: zone_id,
                name: profile.name.clone(),
                kind: profile.kind,
                offset,
                capacity: profile.capacity,
                active_led_count: profile.active_led_count,
            });

            for local_id in 0..profile.active_led_count {
                let id = offset + local_id;
                leds.push((
                    LedInfo { id, zone_id },
                    Position {
                        x: id as f32 / last_led,
                        y: 0.0,
                        z: 0.0,
                    },
                ));
            }
            offset += profile.capacity;
        }

        Self::new(leds, zones)
    }
}
