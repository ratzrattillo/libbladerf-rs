use anyhow::{Context, Result, bail};
use clap::Parser;
use libbladerf_rs::bladerf1::BladeRf1;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const VERSIONS_URL: &str = "https://nuand.com/versions.json";
const FPGA_BASE_URL: &str = "https://www.nuand.com/fpga";

#[derive(Parser)]
#[command(name = "flash_fpga", about = "Flash FPGA bitstream to bladeRF1")]
struct Cli {
    /// Flash FPGA from a local file instead of downloading
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Flash a specific version instead of the latest (e.g. v0.15.0)
    #[arg(long)]
    version: Option<String>,

    /// Load the bitstream to the FPGA after flashing
    #[arg(long)]
    load: bool,

    /// Skip FPGA initialization after loading
    #[arg(long)]
    no_init: bool,
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

fn download_fpga(version: &str, variant: &str) -> Result<Vec<u8>> {
    let filename = format!("{variant}.rbf");
    let url = format!("{FPGA_BASE_URL}/{version}/{filename}");
    log::info!("Downloading FPGA bitstream from {url}");
    let data: Vec<u8> = ureq::get(&url)
        .call()
        .context("failed to download FPGA bitstream")?
        .into_body()
        .read_to_vec()
        .context("failed to read FPGA bitstream body")?;
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

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("nusb", log::LevelFilter::Warn)
        .init();

    let cli = Cli::parse();

    let mut bladerf = BladeRf1::from_first()?;
    let mut flash = bladerf.flash_session()?;
    let fpga_size = flash.read_flash_fpga_size()?;
    let variant = fpga_size.variant_label()?;
    log::info!("Detected FPGA variant: {variant} ({fpga_size:?})");

    let (bitstream, _label) = if let Some(path) = cli.file {
        let data = std::fs::read(&path).context("failed to read FPGA bitstream file")?;
        (data, path.display().to_string())
    } else {
        let versions = fetch_versions()?;

        let fpga_version = cli
            .version
            .as_deref()
            .or_else(|| {
                versions
                    .get("fpga")
                    .and_then(|v| v.get("latest"))
                    .and_then(|v| v.as_str())
            })
            .context("missing FPGA version")?;

        let version_entry = versions
            .get("fpga")
            .and_then(|v| v.get("versions"))
            .and_then(|v| v.get(fpga_version))
            .context("missing FPGA version entry")?;

        let sha256_hex = version_entry
            .get("files")
            .and_then(|v| v.get(variant))
            .and_then(|v| v.get("sha256"))
            .and_then(|v| v.as_str())
            .context("missing sha256 for variant")?;

        log::info!("FPGA version: {fpga_version} ({variant}.rbf)");

        let data = download_fpga(fpga_version, variant)?;

        verify_sha256(&data, sha256_hex)?;
        log::info!("SHA-256 verified");

        (data, fpga_version.to_string())
    };

    let expected = flash.get_fpga_bytes()?;
    if bitstream.len() != expected {
        bail!(
            "FPGA bitstream size mismatch: expected {expected} bytes for {variant}, got {} bytes",
            bitstream.len()
        );
    }

    log::info!("Flashing FPGA bitstream...");
    flash.flash_fpga(&bitstream)?;
    log::info!("FPGA bitstream written and verified");
    drop(flash);

    if cli.load {
        log::info!("Loading FPGA bitstream...");
        let mut config = bladerf.config_session()?;
        config.load_fpga(&bitstream)?;
        log::info!("FPGA loaded");
        drop(config);

        if !cli.no_init {
            let mut rf = bladerf.rf_link_session()?;
            rf.initialize(false)?;
            log::info!("FPGA initialized");
        }
    }

    Ok(())
}
