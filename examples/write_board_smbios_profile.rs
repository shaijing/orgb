use anyhow::{Context, Result, ensure};
use clap::Parser;
use orgb::smbios::{BoardIdentity, read_board_identity};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Parser, Debug)]
#[command(about = "Read current SMBIOS board identity and write it into a board profile TOML")]
struct Args {
    /// Directory containing board profile TOML files.
    #[arg(long, default_value = "configs/colorful")]
    config_dir: PathBuf,
    /// Board profile name or TOML file stem. Defaults to matching the current SMBIOS board.
    #[arg(long)]
    profile: Option<String>,
    /// Print the matched profile and detected SMBIOS values without writing.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let identity = read_board_identity()?;
    let profile_path = find_profile(&args.config_dir, args.profile.as_deref(), &identity)?;

    println!("SMBIOS vendor: {}", identity.vendor);
    println!("SMBIOS model: {}", identity.model);
    println!(
        "SMBIOS revision: {}",
        identity.revision.as_deref().unwrap_or("unknown")
    );
    println!("Profile: {}", profile_path.display());

    if args.dry_run {
        return Ok(());
    }

    let contents = fs::read_to_string(&profile_path)
        .with_context(|| format!("failed to read profile {}", profile_path.display()))?;
    let mut document = contents
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse profile {}", profile_path.display()))?;

    write_identity(&mut document, &identity)?;
    fs::write(&profile_path, document.to_string())
        .with_context(|| format!("failed to write profile {}", profile_path.display()))?;

    println!("Updated {}", profile_path.display());
    Ok(())
}

fn find_profile(
    config_dir: &Path,
    requested: Option<&str>,
    identity: &BoardIdentity,
) -> Result<PathBuf> {
    let mut paths = fs::read_dir(config_dir)
        .with_context(|| format!("failed to read config directory {}", config_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to enumerate config directory {}",
                config_dir.display()
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
        config_dir.display()
    );

    if let Some(requested) = requested {
        return paths
            .into_iter()
            .find(|path| profile_name_matches(path, requested))
            .with_context(|| format!("board profile {requested:?} was not found"));
    }

    let mut matches = Vec::new();
    for path in paths {
        if profile_matches_identity(&path, identity)? {
            matches.push(path);
        }
    }

    ensure!(
        matches.len() == 1,
        "no unique board profile matches SMBIOS board {} / {}",
        identity.vendor,
        identity.model
    );
    Ok(matches[0].clone())
}

fn profile_name_matches(path: &Path, requested: &str) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(requested))
}

fn profile_matches_identity(path: &Path, identity: &BoardIdentity) -> Result<bool> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read profile {}", path.display()))?;
    let document = contents
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse profile {}", path.display()))?;
    let vendor = document["vendor"].as_str().unwrap_or_default();
    let model = document["model"].as_str().unwrap_or_default();

    Ok(normalize(model) == normalize(&identity.model) && vendor_matches(vendor, &identity.vendor))
}

fn write_identity(document: &mut DocumentMut, identity: &BoardIdentity) -> Result<()> {
    let revision = identity
        .revision
        .as_deref()
        .context("current SMBIOS board revision is unavailable")?;

    document["revision"] = value(revision);

    let smbios = document["smbios"].or_insert(Item::Table(Table::new()));
    let smbios = smbios
        .as_table_mut()
        .context("profile field smbios must be a TOML table")?;
    smbios["vendor"] = value(identity.vendor.as_str());
    smbios["model"] = value(identity.model.as_str());
    smbios["revision"] = value(revision);

    Ok(())
}

fn vendor_matches(profile_vendor: &str, smbios_vendor: &str) -> bool {
    let profile_vendor = normalize(profile_vendor);
    let smbios_vendor = normalize(smbios_vendor);
    !profile_vendor.is_empty() && smbios_vendor.contains(&profile_vendor)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
