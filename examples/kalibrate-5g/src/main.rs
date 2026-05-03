mod detect;
mod factory;
mod gscn;
mod pss;
mod source;

use anyhow::{Context, Result};
use gscn::Band;
use source::Source;

const GAIN_DB: i8 = 40;

fn main() -> Result<()> {
    env_logger::init();

    let mut source = Source::open().context("Failed to open bladeRF device")?;
    source.set_gain(GAIN_DB).context("Failed to set gain")?;

    match source.get_dac_trim() {
        Ok(v) => println!("[*] Current DAC Trim: 0x{v:04x}"),
        Err(_) => println!("[*] Current DAC Trim: unknown"),
    }
    let factory_trim = factory::factory_dac_trim(source.serial()?.as_str());
    match factory_trim {
        Some(v) => println!("[*] Factory DAC Trim: 0x{v:04x}"),
        None => println!("[*] Factory DAC Trim: unknown"),
    }

    if let Some(ft) = factory_trim {
        source
            .set_dac_trim(ft)
            .context("Failed to restore factory DAC trim")?;
        eprintln!("[*] Restored factory DAC trim 0x{ft:04x}");
    }

    let all_gscn_freqs: Vec<f64> = Band::all()
        .iter()
        .flat_map(|band| {
            band.gscns()
                .into_iter()
                .flat_map(|gscn| band.gscn_to_freq(gscn))
        })
        .collect();

    let speed = source
        .device()
        .speed()
        .context("Failed to get device speed")?;
    let usable_bw_hz = match speed {
        nusb::Speed::High => 12e6,
        _ => 24e6,
    };

    let tune_centers = gscn::group_frequencies(&all_gscn_freqs, usable_bw_hz);
    eprintln!(
        "[*] Scanning {} GSCN positions across {} NR bands ({} tuning steps)...",
        all_gscn_freqs.len(),
        Band::all().len(),
        tune_centers.len()
    );

    let results = source
        .streaming_ssb_scan(&tune_centers)
        .context("Streaming SSB scan failed")?;

    let mut detected: Vec<(f64, f64, detect::SsbDetectResult)> = Vec::new();
    for (tune_center, results) in &results {
        for result in results {
            if !result.detected {
                continue;
            }
            let abs_freq = tune_center + source::TUNING_OFFSET_HZ + result.freq_offset_hz;
            detected.push((*tune_center, abs_freq, result.clone()));
        }
    }

    let mut mapped: Vec<(f64, Band, u32, detect::SsbDetectResult)> = Vec::new();
    for (tune_center, abs_freq, result) in &detected {
        if let Some((band, gscn)) = gscn::nearest_gscn(*abs_freq) {
            let gscn_freq = gscn::gscn_to_freq(gscn);
            if (abs_freq - gscn_freq).abs() < 2_000_000.0 {
                mapped.push((*tune_center, band, gscn, result.clone()));
            }
        }
    }

    let mut deduped: Vec<(f64, Band, u32, detect::SsbDetectResult)> = Vec::new();
    {
        let mut seen_gscns: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (tune_center, band, gscn, result) in &mapped {
            if seen_gscns.insert(*gscn) {
                deduped.push((*tune_center, *band, *gscn, result.clone()));
            } else if let Some(existing) = deduped.iter_mut().find(|(_, _, g, _)| *g == *gscn)
                && result.fill_rate > existing.3.fill_rate
            {
                *existing = (*tune_center, *band, *gscn, result.clone());
            }
        }
    }

    deduped.sort_by(|a, b| {
        (b.3.par_db * b.3.fill_rate)
            .partial_cmp(&(a.3.par_db * a.3.fill_rate))
            .unwrap()
    });

    eprintln!("[*] Top detected signals (by PAR × fill rate):");
    for (i, (tune_center, band, gscn, result)) in deduped.iter().enumerate().take(10) {
        let abs_freq = tune_center + source::TUNING_OFFSET_HZ + result.freq_offset_hz;
        let n_id_str = match result.n_id_2 {
            Some(n) => format!("n_id_2={n}"),
            None => "n_id_2=?".to_string(),
        };
        let snr_str = match result.pss_snr_db {
            Some(s) => format!("PSS_SNR={s:.1}dB"),
            None => "PSS_SNR=?".to_string(),
        };
        eprintln!(
            "  #{:<3} {:?}/GSCN-{}  {:.3} MHz  PAR: {:.1} dB  offset: {:.1} kHz  fill: {:.0}%  {n_id_str}  {snr_str}",
            i + 1,
            band,
            gscn,
            abs_freq / 1e6,
            result.par_db,
            (result.freq_offset_hz + source::TUNING_OFFSET_HZ) / 1e3,
            result.fill_rate * 100.0,
        );
    }

    println!("\nDetected signals (freq MHz, PAR dB, offset kHz, fill_rate, n_id_2, pss_snr_db):");
    for (tune_center, band, gscn, result) in &deduped {
        let abs_freq = tune_center + source::TUNING_OFFSET_HZ + result.freq_offset_hz;
        let n_id_2 = result.n_id_2.map_or("?".to_string(), |n| n.to_string());
        let pss_snr = result
            .pss_snr_db
            .map_or("?".to_string(), |s| format!("{s:.1}"));
        println!(
            "{:.3}\t{:.1}\t{:.1}\t{:.2}\t{n_id_2}\t{pss_snr}\t{:?}/GSCN-{}",
            abs_freq / 1e6,
            result.par_db,
            (result.freq_offset_hz + source::TUNING_OFFSET_HZ) / 1e3,
            result.fill_rate,
            band,
            gscn,
        );
    }

    let found_pss = deduped
        .iter()
        .filter_map(|(tune_center, band, gscn, result)| {
            let abs_freq = tune_center + source::TUNING_OFFSET_HZ + result.freq_offset_hz;
            source
                .measure_pss_offset(abs_freq)
                .ok()
                .flatten()
                .map(|pr| {
                    let gscn_freq = gscn::gscn_to_freq(*gscn);
                    let pss_abs_freq = abs_freq + (pr.freq_offset_hz - (-1_000_000.0));
                    let vctxo_error = pss_abs_freq - gscn_freq;
                    let ppm = vctxo_error / gscn_freq * 1e6;
                    (*band, *gscn, gscn_freq, pr, vctxo_error, ppm)
                })
        })
        .collect::<Vec<_>>();

    if let Some(best) = found_pss.first() {
        let (band, gscn, gscn_freq, pr, vctxo_error, ppm) = best;
        eprintln!(
            "\n[*] Best: {:?}/GSCN-{}  {:.3} MHz  PSS offset: {:+.1} Hz ({:+.3} ppm)  n_id_2={}  PSS SNR: {:.1} dB",
            band,
            gscn,
            gscn_freq / 1e6,
            vctxo_error,
            ppm,
            pr.n_id_2,
            pr.snr_db
        );
        return Ok(());
    }

    eprintln!("[*] No PSS-verified signals found in scan. Trying known GSCN positions directly...");
    let known_gscns: Vec<(Band, u32)> = vec![
        (Band::N1, 5_344),
        (Band::N1, 5_337),
        (Band::N1, 5_340),
        (Band::N3, 4_673),
        (Band::N3, 4_676),
        (Band::N3, 4_613),
        (Band::N3, 4_593),
        (Band::N3, 4_629),
    ];

    for (band, gscn) in &known_gscns {
        let gscn_freq = gscn::gscn_to_freq(*gscn);
        eprintln!(
            "[*] Trying {:?}/GSCN-{} at {:.3} MHz...",
            band,
            gscn,
            gscn_freq / 1e6
        );
        match source.measure_pss_offset(gscn_freq) {
            Ok(Some(pr)) => {
                let pss_abs_freq = gscn_freq + (pr.freq_offset_hz - (-1_000_000.0));
                let vctxo_error = pss_abs_freq - gscn_freq;
                let ppm = vctxo_error / gscn_freq * 1e6;
                eprintln!(
                    "[*] FOUND: {:?}/GSCN-{}  {:.3} MHz  PSS offset: {:+.1} Hz ({:+.3} ppm)  n_id_2={}  PSS SNR: {:.1} dB",
                    band,
                    gscn,
                    gscn_freq / 1e6,
                    vctxo_error,
                    ppm,
                    pr.n_id_2,
                    pr.snr_db
                );
                println!(
                    "{:.3}\t{:.1}\t{:.1}\t1.00\t{}\t{:.1}\t{:?}/GSCN-{}",
                    pss_abs_freq / 1e6,
                    0.0,
                    vctxo_error / 1e3,
                    pr.n_id_2,
                    pr.snr_db,
                    band,
                    gscn,
                );
                return Ok(());
            }
            Ok(None) => {
                eprintln!("    PSS not found");
            }
            Err(e) => {
                eprintln!("    Error: {e:#}");
            }
        }
    }

    eprintln!("[*] No 5G NR signals found on any known GSCN position.");

    Ok(())
}
