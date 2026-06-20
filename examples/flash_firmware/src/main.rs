use anyhow::{Context, Result, bail};
use clap::Parser;
use libbladerf_rs::bladerf1::BladeRf1;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

const VERSIONS_URL: &str = "https://nuand.com/versions.json";
const FIRMWARE_BASE_URL: &str = "https://www.nuand.com/fx3";

#[derive(Parser)]
#[command(name = "flash_firmware", about = "Flash FX3 firmware to bladeRF1")]
struct Cli {
    /// Flash firmware from a local file instead of downloading the latest
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Flash a specific version instead of the latest (e.g. v2.4.0)
    #[arg(long)]
    version: Option<String>,
}

fn fetch_versions() -> Result<serde_json::Value> {
    let body: String = ureq::get(VERSIONS_URL)
        .call()
        .context("failed to fetch versions.json")?
        .into_body()
        .read_to_string()
        .context("failed to read versions.json body")?;
    serde_json::from_str(&body).context("failed to parse versions.json")
}

fn download_firmware(filename: &str) -> Result<Vec<u8>> {
    let url = format!("{}/{}", FIRMWARE_BASE_URL, filename);
    log::info!("Downloading firmware from {}", url);
    let data: Vec<u8> = ureq::get(&url)
        .call()
        .context("failed to download firmware")?
        .into_body()
        .read_to_vec()
        .context("failed to read firmware body")?;
    log::info!("Downloaded {} bytes", data.len());
    Ok(data)
}

fn verify_sha256(data: &[u8], expected_hex: &str) -> Result<()> {
    let expected = hex::decode(expected_hex).context("invalid hex string")?;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let actual = hasher.finalize();
    if actual.as_slice() != expected.as_slice() {
        bail!(
            "SHA-256 mismatch: expected {}, got {}",
            expected_hex,
            hex::encode(actual)
        );
    }
    Ok(())
}

fn reopen_with_retry(attempts: u32, delay: Duration) -> Result<BladeRf1> {
    for i in 0..attempts {
        log::info!("Attempt {}/{} to reconnect device...", i + 1, attempts);
        match BladeRf1::from_first() {
            Ok(dev) => return Ok(dev),
            Err(_) => {
                if i + 1 < attempts {
                    thread::sleep(delay);
                }
            }
        }
    }
    bail!("failed to reconnect to device after {} attempts", attempts)
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("nusb", log::LevelFilter::Warn)
        .init();

    let cli = Cli::parse();

    let (firmware, label) = if let Some(path) = cli.file {
        let data = std::fs::read(&path).context("failed to read firmware file")?;
        (data, path.display().to_string())
    } else {
        let versions = fetch_versions()?;

        let ver_key = cli
            .version
            .as_deref()
            .or_else(|| {
                versions
                    .get("firmware")
                    .and_then(|v| v.get("latest"))
                    .and_then(|v| v.as_str())
            })
            .context("missing firmware version")?;

        let version_entry = versions
            .get("firmware")
            .and_then(|v| v.get("versions"))
            .and_then(|v| v.get(ver_key))
            .context("missing firmware version entry")?;
        let filename = version_entry
            .get("filename")
            .and_then(|v| v.as_str())
            .context("missing filename")?;
        let sha256_hex = version_entry
            .get("sha256")
            .and_then(|v| v.as_str())
            .context("missing sha256")?;

        log::info!("Firmware version: {} ({})", ver_key, filename);

        let data = download_firmware(filename)?;

        verify_sha256(&data, sha256_hex)?;
        log::info!("SHA-256 verified");

        (data, ver_key.to_string())
    };

    let mut bladerf = BladeRf1::from_first()?;
    let current_version = bladerf.fx3_firmware_version()?;
    log::info!("Current firmware: {}", current_version);

    log::info!("Flashing firmware...");
    let mut flash = bladerf.flash_session()?;
    flash.flash_firmware(&firmware)?;
    log::info!("Firmware written and verified");

    log::info!("Resetting device...");
    bladerf.device_reset()?;

    drop(bladerf);

    log::info!("Waiting for device to reconnect...");
    let bladerf = reopen_with_retry(10, Duration::from_secs(2))?;

    let new_version = bladerf.fx3_firmware_version()?;
    log::info!("Firmware after flash: {}", new_version);

    let expected = label.trim_start_matches('v');
    if new_version == expected || new_version.starts_with(&format!("{expected}-")) {
        log::info!("Firmware update successful!");
    } else {
        log::warn!(
            "Firmware version mismatch: expected {}, got {}",
            expected,
            new_version
        );
    }

    Ok(())
}
