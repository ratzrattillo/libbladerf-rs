mod arfcn;
mod factory;
mod fcch;
mod offset;
mod source;

use crate::factory::factory_dac_trim;
use crate::source::SAMPLE_RATE;
use anyhow::{Context, Result};
use clap::Parser;
use source::Source;

use libbladerf_rs::Channel::Rx;

/// Calibrate bladeRF VCTCXO using GSM FCCH signals
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// DAC trim value (decimal or 0x hex).
    /// With --write: write to flash immediately, skip calibration.
    /// Otherwise: use as starting point for auto-calibration.
    #[arg(short = 'C', long = "dac-trim")]
    dac_trim: Option<String>,

    /// Write DAC trim to flash.
    #[arg(short = 'w', long = "write")]
    write: bool,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    let mut source = Source::open().context("Failed to open bladeRF device")?;
    source.set_gain(40).context("Failed to set gain")?;

    println!("[*] Current DAC Trim: {:#04x?}", source.get_dac_trim().ok());
    println!(
        "[*] Factory DAC Trim: {:#04x?}",
        factory_dac_trim(source.serial()?.as_str())
    );

    // Write mode: write DAC value to flash, skip calibration
    if args.write {
        let dac = args
            .dac_trim
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--write requires --dac-trim"))?;
        let dac = parse_dac_trim(dac).context("Invalid DAC trim value")?;
        source.set_dac_trim(dac).context("Failed to set DAC trim")?;
        source
            .save_dac_trim(dac)
            .context("Failed to save DAC trim to flash")?;
        println!("DAC trim {dac} (0x{:04X}) written to flash.", dac);
        return Ok(());
    }

    // Calibrate mode: auto-calibrate
    let start_dac = if let Some(ref dac_str) = args.dac_trim {
        let dac = parse_dac_trim(dac_str).context("Invalid DAC trim value")?;
        source
            .set_dac_trim(dac)
            .context("Failed to set initial DAC trim")?;
        println!("[*] Initial DAC trim set to {dac} (0x{:04X})", dac);
        dac
    } else {
        source
            .get_dac_trim()
            .context("Failed to read current DAC trim")?
    };

    auto_calibrate(&mut source, start_dac)?;

    Ok(())
}

/// Auto-calibrate: scan all bands, find the strongest signal, calibrate on it.
fn auto_calibrate(source: &mut Source, start_dac: u16) -> Result<()> {
    eprintln!("[*] Scanning all bands for strongest signal...");

    let frequencies: Vec<f64> = arfcn::Band::all()
        .iter()
        .flat_map(|band| band.arfcns().flat_map(|arfcn| band.arfcn_to_freq(arfcn)))
        .collect();

    let results = source
        .streaming_power_scan(&frequencies)
        .context("Streaming power scan failed")?;

    let mut indexed: Vec<(usize, &(f64, f64))> = results.iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.1.partial_cmp(&a.1.1).unwrap());

    if indexed.is_empty() {
        anyhow::bail!("No channels detected during scan");
    }

    eprintln!("[*] Top 3 strongest channels:");
    for (rank, &(_idx, &(freq, power))) in indexed.iter().enumerate().take(3) {
        let arfcn_str = if let Some((band, arfcn)) = arfcn::Band::from_freq(freq) {
            format!("{:?}-{:?}", band, arfcn)
        } else {
            "unknown".into()
        };
        eprintln!(
            "  #{} {:>6}  {:.3} MHz  power: {:.2}",
            rank + 1,
            arfcn_str,
            freq / 1e6,
            power
        );
    }

    let top = indexed[0].1;
    eprintln!("[*] Calibrating on {:.3} MHz...", top.0 / 1e6);

    source
        .device()
        .set_sample_rate(Rx, SAMPLE_RATE)
        .context("Failed to set sample rate")?;

    let (best_dac, best_offset) = calibrate(source, start_dac)?;

    let ppm = best_offset as f64 / top.0 * 1e6;
    println!(
        "offset: {:.0} Hz  freq: {:.6} MHz  ppm: {:.2}  DAC: {best_dac} (0x{:04X})",
        best_offset,
        top.0 / 1e6 + best_offset as f64 / 1e6,
        ppm,
        best_dac,
    );

    Ok(())
}

/// Run the binary-search calibration loop at the given frequency.
/// Returns (best_dac, best_offset).
fn calibrate(source: &mut Source, start_dac: u16) -> Result<(u16, f32)> {
    let mut dac: u16 = start_dac;
    let mut delta: u16 = 0x4000;
    let mut best_dac: Option<u16> = None;
    let mut best_offset: f32 = f32::MAX;
    let max_iterations = 16;

    for _ in 0..max_iterations {
        source.set_dac_trim(dac).context("Failed to set DAC trim")?;
        let Some(off) = offset::offset_detect(source).context("Offset detection failed")? else {
            if best_dac.is_some() {
                eprintln!("FCCH signal lost during calibration, using best DAC so far");
            }
            break;
        };

        eprintln!("DAC: {dac} (0x{:04X})  offset: {}", dac, display_freq(off));

        best_offset = off;
        best_dac = Some(dac);

        if off.abs() < 200.0 {
            let base = dac as i32;
            let mut fine_best_dac = dac;
            let mut fine_best_offset = off;
            for d in -6..=-1 {
                let candidate = base + d;
                if !(0..=65_535).contains(&candidate) {
                    continue;
                }
                let candidate_u16 = candidate as u16;
                source
                    .set_dac_trim(candidate_u16)
                    .context("Failed to set DAC trim")?;
                let Some(fine_off) =
                    offset::offset_detect_quick(source).context("Offset detection failed")?
                else {
                    continue;
                };
                if fine_off.abs() < fine_best_offset.abs() {
                    fine_best_offset = fine_off;
                    fine_best_dac = candidate_u16;
                }
            }
            for d in 1..=6 {
                let candidate = base + d;
                if !(0..=65_535).contains(&candidate) {
                    continue;
                }
                let candidate_u16 = candidate as u16;
                source
                    .set_dac_trim(candidate_u16)
                    .context("Failed to set DAC trim")?;
                let Some(fine_off) =
                    offset::offset_detect_quick(source).context("Offset detection failed")?
                else {
                    continue;
                };
                if fine_off.abs() < fine_best_offset.abs() {
                    fine_best_offset = fine_off;
                    fine_best_dac = candidate_u16;
                }
            }
            best_dac = Some(fine_best_dac);
            best_offset = fine_best_offset;
            break;
        }

        if off < 0.0 {
            dac = dac.saturating_sub(delta);
        } else {
            dac = dac.saturating_add(delta);
        }
        delta /= 2;
    }

    let Some(best_dac) = best_dac else {
        anyhow::bail!("No FCCH signal detected — is there a GSM basestation on this channel?");
    };

    source
        .set_dac_trim(best_dac)
        .context("Failed to set DAC trim")?;

    let best_offset = if let Some(off) =
        offset::offset_detect(source).context("Final offset detection failed")?
    {
        off
    } else {
        best_offset
    };

    Ok((best_dac, best_offset))
}

/// Parse a DAC trim value from a string, accepting decimal or 0x-prefixed hex.
fn parse_dac_trim(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).context(format!("Invalid hex DAC trim: {s}"))
    } else {
        s.parse::<u16>().context(format!("Invalid DAC trim: {s}"))
    }
}

/// Format a frequency offset with appropriate SI prefix.
fn display_freq(f: f32) -> String {
    let (sign, val) = if f >= 0.0 { ("+", f) } else { ("-", -f) };

    if val >= 1e9 {
        format!("{sign}{:.3}GHz", val / 1e9)
    } else if val >= 1e6 {
        format!("{sign}{:.1}MHz", val / 1e6)
    } else if val >= 1e3 {
        format!("{sign}{:.3}kHz", val / 1e3)
    } else {
        format!("{sign}{:.0}Hz", val)
    }
}
