use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::{
    BoardProfile, BrandId, DeviceCapabilities, ProtocolKind, UsbMatch, ZoneKind, ZoneProfile,
};
use crate::smbios::BoardIdentity;

#[derive(Debug, Deserialize)]
struct RawBoardProfile {
    vendor: String,
    model: String,
    #[serde(default)]
    revision: Option<String>,
    usb_match: RawUsbMatch,
    protocol: String,
    zones: Vec<RawZoneProfile>,
    #[serde(default)]
    capabilities: RawCapabilities,
}

#[derive(Debug, Deserialize)]
struct RawUsbMatch {
    vendor_id: u16,
    product_id: u16,
    interface: u8,
}

#[derive(Debug, Default, Deserialize)]
struct RawZoneProfile {
    name: String,
    kind: String,
    capacity: usize,
    #[serde(default)]
    active_led_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilities {
    #[serde(default = "default_true")]
    direct_rgb: bool,
    #[serde(default = "default_true")]
    per_led: bool,
    max_leds: Option<usize>,
    min_update_interval_us: Option<u64>,
    #[serde(default)]
    supports_readback: bool,
}

impl Default for RawCapabilities {
    fn default() -> Self {
        Self {
            direct_rgb: true,
            per_led: true,
            max_leds: None,
            min_update_interval_us: None,
            supports_readback: false,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug)]
struct LoadedProfile {
    source: PathBuf,
    profile: BoardProfile,
}

#[derive(Debug)]
pub struct ProfileCatalog {
    profiles: Vec<LoadedProfile>,
}

impl ProfileCatalog {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self> {
        let directory = directory.as_ref();
        let mut paths = fs::read_dir(directory)
            .with_context(|| format!("failed to read config directory {}", directory.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| {
                format!(
                    "failed to enumerate config directory {}",
                    directory.display()
                )
            })?;
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        });
        paths.sort();
        ensure!(
            !paths.is_empty(),
            "no TOML board profiles found in {}",
            directory.display()
        );

        let profiles = paths
            .into_iter()
            .map(|path| {
                let contents = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read board profile {}", path.display()))?;
                let raw = toml::from_str::<RawBoardProfile>(&contents)
                    .with_context(|| format!("failed to parse board profile {}", path.display()))?;
                Ok(LoadedProfile {
                    source: path,
                    profile: raw.into_profile()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { profiles })
    }

    pub fn select(
        &self,
        requested: Option<&str>,
        brand: BrandId,
        identity: Option<&BoardIdentity>,
    ) -> Result<BoardProfile> {
        let candidates = self
            .profiles
            .iter()
            .filter(|loaded| loaded.profile.brand == brand)
            .collect::<Vec<_>>();
        ensure!(
            !candidates.is_empty(),
            "no board profile found for {brand:?}"
        );

        if let Some(requested) = requested {
            let requested = requested.to_ascii_lowercase();
            let profile = candidates.iter().find(|loaded| {
                loaded.profile.model.to_ascii_lowercase() == requested
                    || loaded
                        .source
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem.to_ascii_lowercase() == requested)
            });
            return profile
                .map(|loaded| loaded.profile.clone())
                .with_context(|| format!("board profile {requested} was not found"));
        }

        let identity =
            identity.context("automatic board profile selection requires SMBIOS data")?;
        let matches = candidates
            .iter()
            .filter(|loaded| profile_matches_identity(&loaded.profile, identity))
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "no unique {brand:?} board profile matches SMBIOS board {} / {}",
            identity.vendor,
            identity.model
        );
        Ok(matches[0].profile.clone())
    }
}

fn profile_matches_identity(profile: &BoardProfile, identity: &BoardIdentity) -> bool {
    normalize(&profile.model) == normalize(&identity.model)
        && brand_matches(profile.brand, &identity.vendor)
        && profile.revision.as_deref().is_none_or(|revision| {
            identity
                .revision
                .as_deref()
                .is_some_and(|actual| normalize(revision) == normalize(actual))
        })
}

fn brand_matches(brand: BrandId, vendor: &str) -> bool {
    let vendor = normalize(vendor);
    match brand {
        BrandId::Colorful => vendor.contains("colorful"),
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl RawBoardProfile {
    fn into_profile(self) -> Result<BoardProfile> {
        let brand = match self.vendor.trim().to_ascii_lowercase().as_str() {
            "colorful" => BrandId::Colorful,
            vendor => anyhow::bail!("unsupported board vendor {vendor:?}"),
        };
        let protocol = match self.protocol.trim().to_ascii_lowercase().as_str() {
            "colorful-088" | "colorful088" | "colorful 088" => ProtocolKind::Colorful088,
            protocol => anyhow::bail!("unsupported board protocol {protocol:?}"),
        };
        ensure!(
            !self.model.trim().is_empty(),
            "board profile model cannot be empty"
        );

        let zones = self
            .zones
            .into_iter()
            .map(RawZoneProfile::into_profile)
            .collect::<Result<Vec<_>>>()?;
        let led_count = zones.iter().map(|zone| zone.capacity).sum::<usize>();
        ensure!(led_count > 0, "board profile must define at least one LED");

        let capabilities = DeviceCapabilities {
            direct_rgb: self.capabilities.direct_rgb,
            per_led: self.capabilities.per_led,
            max_leds: self.capabilities.max_leds.unwrap_or(led_count),
            min_update_interval: Duration::from_micros(
                self.capabilities.min_update_interval_us.unwrap_or(16_667),
            ),
            supports_readback: self.capabilities.supports_readback,
        };
        ensure!(
            capabilities.max_leds >= led_count,
            "profile max_leds {} is smaller than zone capacity {}",
            capabilities.max_leds,
            led_count
        );

        Ok(BoardProfile {
            brand,
            model: self.model,
            revision: self.revision,
            usb_match: UsbMatch {
                vendor_id: self.usb_match.vendor_id,
                product_id: self.usb_match.product_id,
                interface: self.usb_match.interface,
            },
            protocol,
            zones,
            capabilities,
        })
    }
}

impl RawZoneProfile {
    fn into_profile(self) -> Result<ZoneProfile> {
        let kind = match self.kind.trim().to_ascii_lowercase().as_str() {
            "argb" => ZoneKind::Argb,
            "rgb" => ZoneKind::Rgb,
            kind => anyhow::bail!("unsupported zone kind {kind:?}"),
        };
        ensure!(!self.name.trim().is_empty(), "zone name cannot be empty");
        ensure!(
            self.capacity > 0,
            "zone {} must have positive capacity",
            self.name
        );
        let active_led_count = self.active_led_count.unwrap_or(self.capacity);
        ensure!(
            active_led_count <= self.capacity,
            "zone {} has more active LEDs than its capacity",
            self.name
        );

        Ok(ZoneProfile {
            name: self.name,
            kind,
            capacity: self.capacity,
            active_led_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_board_profile_from_toml() {
        let catalog = ProfileCatalog::load("configs/colorful").unwrap();
        let profile = catalog
            .select(
                Some("battle-ax_b860m-plus_s_wifi7"),
                BrandId::Colorful,
                None,
            )
            .unwrap();

        assert_eq!(profile.model, "BATTLE-AX B860M-PLUS S WIFI7");
        assert_eq!(profile.zones.len(), 8);
        assert_eq!(profile.topology().unwrap().led_count(), 602);
    }

    #[test]
    fn defaults_active_led_count_to_capacity() {
        let raw = r#"
            vendor = "Colorful"
            model = "Test Board"
            protocol = "colorful-088"

            [usb_match]
            vendor_id = 1
            product_id = 2
            interface = 3

            [[zones]]
            name = "ARGB_1"
            kind = "argb"
            capacity = 12
        "#;

        let profile = toml::from_str::<RawBoardProfile>(raw)
            .unwrap()
            .into_profile()
            .unwrap();
        assert_eq!(profile.zones[0].active_led_count, 12);
        assert_eq!(profile.capabilities.max_leds, 12);
    }

    #[test]
    fn selects_profile_by_smbios_identity() {
        let catalog = ProfileCatalog::load("configs/colorful").unwrap();
        let identity = BoardIdentity {
            vendor: "Colorful Technology And Development Co.,Ltd".to_owned(),
            model: "BATTLE-AX B860M-PLUS S WIFI7".to_owned(),
            revision: Some("V20".to_owned()),
        };

        let profile = catalog
            .select(None, BrandId::Colorful, Some(&identity))
            .unwrap();
        assert_eq!(profile.model, identity.model);
    }
}
