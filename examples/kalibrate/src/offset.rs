use anyhow::{Context, Result};

use crate::fcch::{FcchDetector, GSM_RATE};
use crate::source::Source;

/// Number of FCCH detections for quick (binary search) measurements
const QUICK_COUNT: usize = 20;

/// Number of FCCH detections for precise (final) measurements
const AVG_COUNT: usize = 100;

/// Maximum acceptable offset for sanity checking (40 * 70 kHz = 2.8 MHz)
const OFFSET_MAX: f32 = 40.0 * 70_000.0;

/// Maximum consecutive misses before giving up (quick mode)
const QUICK_NOTFOUND_MAX: u32 = 500;

/// Maximum consecutive misses before giving up (precise mode)
const NOTFOUND_MAX: u32 = 1_000;

/// Quick offset measurement for binary search — collects fewer samples.
/// Returns `None` if no FCCH signal is detected after many attempts.
pub fn offset_detect_quick(source: &mut Source) -> Result<Option<f32>> {
    offset_detect_inner(source, QUICK_COUNT, QUICK_NOTFOUND_MAX, false)
}

/// Precise offset measurement — collects 100 FCCH detections, trimmed mean.
/// Returns `None` if no FCCH signal is detected after many attempts.
pub fn offset_detect(source: &mut Source) -> Result<Option<f32>> {
    offset_detect_inner(source, AVG_COUNT, NOTFOUND_MAX, true)
}

/// Core offset detection logic.
fn offset_detect_inner(
    source: &mut Source,
    count: usize,
    notfound_max: u32,
    verbose: bool,
) -> Result<Option<f32>> {
    let sample_rate = source.sample_rate();
    let sps = sample_rate / GSM_RATE;
    let s_len = ((12.0 * 8.0 * 156.25 + 156.25) * sps).ceil() as usize;

    let mut detector = FcchDetector::new(sample_rate as f32);
    let fcch_offset = (GSM_RATE / 4.0) as f32;

    source.start().context("Failed to start source")?;
    source.flush().context("Flush failed")?;

    let mut offsets: Vec<f32> = Vec::with_capacity(count);
    let mut notfound: u32 = 0;

    while offsets.len() < count {
        let samples = source
            .read_samples(s_len)
            .context("Failed to read samples")?;

        match detector.scan(&samples) {
            Some(freq) => {
                let offset = freq - fcch_offset;
                if offset.abs() < OFFSET_MAX {
                    offsets.push(offset);
                    if verbose {
                        print!(".");
                        if offsets.len().is_multiple_of(20) {
                            println!();
                        }
                    }
                }
                notfound = 0;
            }
            None => {
                notfound += 1;
                if notfound >= notfound_max {
                    source.stop().ok();
                    eprintln!("No FCCH signal detected after {notfound_max} attempts");
                    return Ok(None);
                }
            }
        }
    }
    if verbose {
        println!();
    }

    source.stop().context("Failed to stop source")?;

    // Trimmed-mean statistics
    let trim = if count >= 10 { count / 10 } else { 0 };
    offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let trimmed = &offsets[trim..count - trim];
    let mean: f32 = trimmed.iter().sum::<f32>() / trimmed.len() as f32;

    if verbose {
        let variance: f32 =
            trimmed.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / trimmed.len() as f32;
        let stddev = variance.sqrt();
        let min = trimmed[0];
        let max = trimmed[trimmed.len() - 1];

        println!("average\t\t[min, max]\t(range, stddev)");
        println!(
            "{}\t\t[{:.0}, {:.0}]\t({:.0}, {:.2})",
            display_freq(mean),
            min,
            max,
            max - min,
            stddev
        );
        println!("not found: {notfound}");
    }

    Ok(Some(mean))
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
