use anyhow::{Context, Result, ensure};
use smbioslib::{SMBiosBaseboardInformation, SMBiosData, SMBiosString, table_load_from_device};
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardIdentity {
    pub vendor: String,
    pub model: String,
    pub revision: Option<String>,
}

pub fn read_board_identity() -> Result<BoardIdentity> {
    let data = match table_load_from_device() {
        Ok(data) => data,
        Err(error) => {
            #[cfg(target_os = "linux")]
            {
                return read_linux_sysfs_identity()
                    .with_context(|| format!("smbios-lib failed to load SMBIOS data: {error}"));
            }
            #[cfg(not(target_os = "linux"))]
            {
                return Err(error).context("failed to load SMBIOS data");
            }
        }
    };
    identity_from_data(&data)
}

fn identity_from_data(data: &SMBiosData) -> Result<BoardIdentity> {
    let baseboard = data
        .first::<SMBiosBaseboardInformation>()
        .context("SMBIOS baseboard information (type 2) was not found")?;

    let vendor = required_string(baseboard.manufacturer(), "baseboard manufacturer")?;
    let model = required_string(baseboard.product(), "baseboard product")?;
    let revision = optional_string(baseboard.version());

    Ok(BoardIdentity {
        vendor,
        model,
        revision,
    })
}

#[cfg(target_os = "linux")]
fn read_linux_sysfs_identity() -> Result<BoardIdentity> {
    let vendor = read_linux_dmi_field("board_vendor")?;
    let model = read_linux_dmi_field("board_name")?;
    let revision = read_linux_dmi_field("board_version").ok();

    Ok(BoardIdentity {
        vendor,
        model,
        revision,
    })
}

#[cfg(target_os = "linux")]
fn read_linux_dmi_field(field: &str) -> Result<String> {
    let path = format!("/sys/class/dmi/id/{field}");
    let value = fs::read_to_string(&path)
        .with_context(|| format!("failed to read Linux DMI field {path}"))?;
    let value = value.trim().to_owned();
    ensure!(!value.is_empty(), "Linux DMI field {path} is empty");
    Ok(value)
}

fn required_string(value: SMBiosString, field: &str) -> Result<String> {
    let value = value
        .to_utf8_lossy()
        .with_context(|| format!("SMBIOS {field} is missing or invalid"))?;
    let value = value.trim().to_owned();
    ensure!(!value.is_empty(), "SMBIOS {field} is empty");
    Ok(value)
}

fn optional_string(value: SMBiosString) -> Option<String> {
    value
        .to_utf8_lossy()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_board_identity_fields() {
        let identity = BoardIdentity {
            vendor: "Colorful Technology".to_owned(),
            model: "BATTLE-AX B860M-PLUS S WIFI7".to_owned(),
            revision: Some("1.0".to_owned()),
        };

        assert_eq!(identity.model, "BATTLE-AX B860M-PLUS S WIFI7");
        assert_eq!(identity.revision.as_deref(), Some("1.0"));
    }
}
