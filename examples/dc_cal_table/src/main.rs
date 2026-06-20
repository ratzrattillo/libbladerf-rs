use anyhow::Result;
use libbladerf_rs::bladerf1::calibration::{DcCalEntry, DcCalTable};
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, RxCal};
use libbladerf_rs::bladerf1::hardware::lms6002d::frequency::get_frequency_min;
use libbladerf_rs::bladerf1::hardware::lms6002d::gain::{GainStage, LnaGainCode};
use libbladerf_rs::bladerf1::hardware::lms6002d::loopback::Loopback;
use libbladerf_rs::bladerf1::hardware::si5338::RationalRate;
use libbladerf_rs::bladerf1::{
    BladeRf1, Correction, DcPair, RfLinkSession, RxStream, SampleFormat, TuningMode, TxStream,
};
use libbladerf_rs::{Channel, Result as LibResult};
use num_complex::Complex;
use std::path::Path;
use std::time::Duration;

type ComplexF = Complex<f32>;

const TX_CAL_RATE: u32 = 4_000_000;
const TX_CAL_RX_BW: u32 = 3_000_000;
const TX_CAL_RX_VGA1: i8 = 25;
const TX_CAL_RX_VGA2: i8 = 0;
const TX_CAL_TS_INC: u64 = ms_to_samples(15, 4_000_000u64);
const TX_CAL_COUNT: u64 = ms_to_samples(5, 4_000_000u64);
const TX_CAL_CORR_SWEEP_LEN: usize = 4096 / 16;

const RX_CAL_RATE: u64 = 3_000_000;
const RX_CAL_BW: u64 = 1_500_000;
const RX_CAL_TS_INC: u64 = ms_to_samples(15, RX_CAL_RATE);
const RX_CAL_COUNT: u64 = ms_to_samples(5, RX_CAL_RATE);
const RX_CAL_MAX_SWEEP_LEN: u64 = 2 * 2_048 / 32;

#[allow(clippy::excessive_precision)]
const TX_CAL_FILT: [f32; 16] = [
    0.000327949366768,
    0.002460188536582,
    0.009842382390924,
    0.027274728394777,
    0.057835200476419,
    0.098632713294830,
    0.139062540460741,
    0.164562494987592,
    0.164562494987592,
    0.139062540460741,
    0.098632713294830,
    0.057835200476419,
    0.027274728394777,
    0.009842382390924,
    0.002460188536582,
    0.000327949366768,
];

const fn ms_to_samples(ms: u64, rate: u64) -> u64 {
    (ms * rate) / 1_000
}

#[derive(Debug, Clone)]
struct DcCalParams {
    frequency: u64,
    corr_i: i16,
    corr_q: i16,
    error_i: f32,
    error_q: f32,
    max_dc: DcPair,
    mid_dc: DcPair,
    min_dc: DcPair,
}

struct AgcGainMode {
    lna_gain: LnaGainCode,
    rxvga1: i8,
    rxvga2: i8,
}

const AGC_GAIN_MAX: AgcGainMode = AgcGainMode {
    lna_gain: LnaGainCode::MaxAllLnas,
    rxvga1: 30,
    rxvga2: 15,
};
const AGC_GAIN_MID: AgcGainMode = AgcGainMode {
    lna_gain: LnaGainCode::MidAllLnas,
    rxvga1: 30,
    rxvga2: 0,
};
const AGC_GAIN_MIN: AgcGainMode = AgcGainMode {
    lna_gain: LnaGainCode::MidAllLnas,
    rxvga1: 12,
    rxvga2: 0,
};

struct TxCalBackup {
    rx_freq: u64,
    rx_sample_rate: RationalRate,
    rx_bandwidth: u32,
    rx_lna: LnaGainCode,
    rx_vga1: i8,
    rx_vga2: i8,
    tx_sample_rate: RationalRate,
    loopback: Loopback,
}

struct TxCalState {
    ts: u64,
    loopback: Loopback,
    rx_low: bool,
}

fn lna_gain_code_from_db(db: i8) -> LnaGainCode {
    match db {
        0 => LnaGainCode::BypassLna1Lna2,
        1..=3 => LnaGainCode::MidAllLnas,
        _ => LnaGainCode::MaxAllLnas,
    }
}

fn sample_mean_f32(samples: &[i16]) -> (f32, f32) {
    let count = samples.len() / 2;
    if count == 0 {
        return (0.0, 0.0);
    }
    let acc_i = samples.iter().step_by(2).fold(0i64, |a, &v| a + v as i64);
    let acc_q = samples
        .iter()
        .skip(1)
        .step_by(2)
        .fold(0i64, |a, &v| a + v as i64);
    (acc_i as f32 / count as f32, acc_q as f32 / count as f32)
}

fn rx_samples_sync(stream: &mut RxStream, buf: &mut [i16], num_samples: u64) -> Result<()> {
    let max_i16 = num_samples as usize * 2;
    let mut total_i16 = 0usize;
    while total_i16 < max_i16 {
        let buffer = stream.read(Some(Duration::from_secs(2)))?;
        let available_i16 = buffer.len() / 2;
        let copy_i16 = available_i16.min(max_i16 - total_i16);
        for k in 0..copy_i16 {
            buf[total_i16 + k] = i16::from_le_bytes([buffer[k * 2], buffer[k * 2 + 1]]);
        }
        total_i16 += copy_i16;
        stream.recycle(buffer);
        if total_i16 >= max_i16 {
            break;
        }
    }
    Ok(())
}

fn rx_cal_coarse_means(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    corr_value: &mut i16,
    samples: &mut [i16],
) -> Result<()> {
    let mean_limit: f32 = 2000.0;
    let corr_limit: i16 = 128;
    loop {
        rf.set_rx_dc_corr(*corr_value, *corr_value)?;
        rx_samples_sync(stream, samples, RX_CAL_COUNT)?;
        let (mean_i, mean_q) = sample_mean_f32(samples);
        if (mean_i.abs() > mean_limit || mean_q.abs() > mean_limit)
            && corr_value.unsigned_abs() >= corr_limit as u16
        {
            log::debug!(
                "Coarse estimate point Corr={corr_value:4} yields extreme means: ({mean_i:.1}, {mean_q:.1}). Retrying..."
            );
            *corr_value /= 2;
            continue;
        }
        return Ok(());
    }
}

fn rx_cal_coarse_estimate(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    samples: &mut [i16],
) -> Result<(i16, i16)> {
    let mut x1: i16 = -2048;
    let mut x2: i16 = 2048;
    rx_cal_coarse_means(rf, stream, &mut x1, samples)?;
    let (y1i, y1q) = sample_mean_f32(samples);
    log::debug!("Means for x1={x1}: y1i={y1i:.2}, y1q={y1q:.2}");
    rx_cal_coarse_means(rf, stream, &mut x2, samples)?;
    let (y2i, y2q) = sample_mean_f32(samples);
    log::debug!("Means for x2={x2}: y2i={y2i:.2}, y2q={y2q:.2}");

    let mi = (y2i - y1i) / (x2 as f32 - x1 as f32);
    let mq = (y2q - y1q) / (x2 as f32 - x1 as f32);
    let bi = y1i - mi * x1 as f32;
    let bq = y1q - mq * x1 as f32;

    let i_guess = (-bi / mi).clamp(-2048.0, 2048.0).round() as i16;
    let q_guess = (-bq / mq).clamp(-2048.0, 2048.0).round() as i16;
    log::debug!("Coarse estimate: I={i_guess}, Q={q_guess}");
    Ok((i_guess, q_guess))
}

fn init_rx_cal_sweep(i_est: i16, q_est: i16) -> Vec<i16> {
    let min_est = i_est.min(q_est);
    let max_est = i_est.max(q_est);
    let sweep_min = ((min_est - 12 * 32) / 32 * 32).max(-2048);
    let sweep_max = ((max_est + 12 * 32) / 32 * 32 + 32).min(2048 + 32);
    (sweep_min..sweep_max)
        .step_by(32)
        .take(RX_CAL_MAX_SWEEP_LEN as usize)
        .collect()
}

fn rx_cal_sweep(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    corr: &[i16],
    samples: &mut [i16],
) -> Result<(i16, i16, f32, f32)> {
    let mut min_corr_i: i16 = 0;
    let mut min_corr_q: i16 = 0;
    let mut min_val_i: f32 = 2048.0;
    let mut min_val_q: f32 = 2048.0;

    for &val in corr {
        rf.set_rx_dc_corr(val, val)?;
        rx_samples_sync(stream, samples, RX_CAL_COUNT)?;
        let (mean_i, mean_q) = sample_mean_f32(samples);
        let abs_i = mean_i.abs();
        let abs_q = mean_q.abs();
        if abs_i < min_val_i {
            min_val_i = abs_i;
            min_corr_i = val;
        }
        if abs_q < min_val_q {
            min_val_q = abs_q;
            min_corr_q = val;
        }
    }
    Ok((min_corr_i, min_corr_q, min_val_i, min_val_q))
}

fn rx_cal_dc_off(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    gains: &AgcGainMode,
    samples: &mut [i16],
) -> Result<DcPair> {
    rf.set_gain_stage(GainStage::Lna, (gains.lna_gain as i8).into())?;
    rf.set_gain_stage(GainStage::RxVga1, (gains.rxvga1).into())?;
    rf.set_gain_stage(GainStage::RxVga2, (gains.rxvga2).into())?;
    rx_samples_sync(stream, samples, RX_CAL_COUNT)?;
    let (mean_i, mean_q) = sample_mean_f32(samples);
    Ok(DcPair::new(mean_i.round() as i16, mean_q.round() as i16))
}

fn perform_rx_cal(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    cal: &mut RxCal,
    params: &mut DcCalParams,
    samples: &mut [i16],
) -> Result<()> {
    rf.rx_cal_update_frequency(cal, params.frequency)?;

    let (i_est, q_est) = rx_cal_coarse_estimate(rf, stream, samples)?;
    let sweep = init_rx_cal_sweep(i_est, q_est);
    cal.set_timestamp(cal.timestamp() + RX_CAL_TS_INC);

    let (corr_i, corr_q, error_i, error_q) = rx_cal_sweep(rf, stream, &sweep, samples)?;
    params.corr_i = corr_i;
    params.corr_q = corr_q;
    params.error_i = error_i;
    params.error_q = error_q;

    rf.set_rx_dc_corr(corr_i, corr_q)?;

    let saved_lna = rf.get_gain_stage(GainStage::Lna).ok();
    let saved_vga1 = rf.get_gain_stage(GainStage::RxVga1).ok();
    let saved_vga2 = rf.get_gain_stage(GainStage::RxVga2).ok();

    params.min_dc = rx_cal_dc_off(rf, stream, &AGC_GAIN_MIN, samples)?;
    params.mid_dc = rx_cal_dc_off(rf, stream, &AGC_GAIN_MID, samples)?;
    params.max_dc = rx_cal_dc_off(rf, stream, &AGC_GAIN_MAX, samples)?;

    if let (Some(lna), Some(vga1), Some(vga2)) = (saved_lna, saved_vga1, saved_vga2) {
        let _ = rf.set_gain_stage(GainStage::Lna, lna);
        let _ = rf.set_gain_stage(GainStage::RxVga1, vga1);
        let _ = rf.set_gain_stage(GainStage::RxVga2, vga2);
    }

    Ok(())
}

fn dc_calibration_rx(
    rf: &mut RfLinkSession<'_>,
    f_min: u64,
    f_max: u64,
    f_inc: u64,
) -> Result<Vec<DcCalParams>> {
    let mut backup = rf.get_rx_cal_backup()?;

    rf.set_sample_rate(Channel::Rx, RX_CAL_RATE as u32)?;
    rf.set_bandwidth(Channel::Rx, RX_CAL_BW as u32)?;

    let buf_size = RX_CAL_COUNT as usize * 4;
    let mut rx_stream = RxStream::builder(rf)
        .buffer_size(buf_size)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;

    rx_stream.start(rf)?;

    let mut cal = RxCal::new(
        0,
        rf.get_timestamp(Channel::Rx)? + 20 * RX_CAL_TS_INC,
        backup.tx_frequency(),
    );

    let num_samples = RX_CAL_COUNT as usize;
    let mut samples = vec![0i16; num_samples * 2];
    let mut results = Vec::new();

    let mut freq = f_min;
    while freq <= f_max {
        let mut params = DcCalParams {
            frequency: freq,
            corr_i: 0,
            corr_q: 0,
            error_i: 0.0,
            error_q: 0.0,
            max_dc: DcPair::default(),
            mid_dc: DcPair::default(),
            min_dc: DcPair::default(),
        };
        perform_rx_cal(rf, &mut rx_stream, &mut cal, &mut params, &mut samples)?;
        log::info!(
            "Calibrated @ {:10} Hz: I={:4} (Error: {:.2}), Q={:4} (Error: {:.2})",
            params.frequency,
            params.corr_i,
            params.error_i,
            params.corr_q,
            params.error_q,
        );
        results.push(params);
        if freq == f_max {
            break;
        }
        freq = (freq + f_inc).min(f_max);
    }

    rx_stream.stop(rf)?;
    rf.set_rx_cal_backup(&mut backup)?;
    Ok(results)
}

fn get_tx_cal_backup(rf: &mut RfLinkSession<'_>) -> Result<TxCalBackup> {
    Ok(TxCalBackup {
        rx_freq: rf.get_frequency(Channel::Rx)?,
        rx_sample_rate: rf.get_rational_sample_rate(Channel::Rx)?,
        rx_bandwidth: rf.get_bandwidth(Channel::Rx)?,
        rx_lna: lna_gain_code_from_db(rf.get_gain_stage(GainStage::Lna)?.db()),
        rx_vga1: rf.get_gain_stage(GainStage::RxVga1)?.db(),
        rx_vga2: rf.get_gain_stage(GainStage::RxVga2)?.db(),
        tx_sample_rate: rf.get_rational_sample_rate(Channel::Tx)?,
        loopback: rf.get_loopback()?,
    })
}

fn set_tx_cal_backup(rf: &mut RfLinkSession<'_>, backup: &TxCalBackup) -> Result<()> {
    let mut retval: Result<()> = Ok(());
    let mut try_set = |res: LibResult<()>| {
        if let Err(e) = res {
            if retval.is_ok() {
                retval = Err(e.into());
            }
        }
    };
    try_set(rf.set_loopback(backup.loopback.clone()));
    try_set(rf.set_frequency(Channel::Rx, backup.rx_freq, TuningMode::Fpga));
    let mut rate = backup.rx_sample_rate;
    let _ = rf.set_rational_sample_rate(Channel::Rx, &mut rate);
    let _ = rf.set_bandwidth(Channel::Rx, backup.rx_bandwidth);
    try_set(rf.set_gain_stage(GainStage::Lna, (backup.rx_lna as i8).into()));
    try_set(rf.set_gain_stage(GainStage::RxVga1, (backup.rx_vga1).into()));
    try_set(rf.set_gain_stage(GainStage::RxVga2, (backup.rx_vga2).into()));
    let mut tx_rate = backup.tx_sample_rate;
    let _ = rf.set_rational_sample_rate(Channel::Tx, &mut tx_rate);
    retval
}

fn apply_tx_cal_settings(rf: &mut RfLinkSession<'_>) -> Result<()> {
    rf.set_sample_rate(Channel::Rx, TX_CAL_RATE)?;
    rf.set_bandwidth(Channel::Rx, TX_CAL_RX_BW)?;
    rf.set_gain_stage(GainStage::Lna, (LnaGainCode::MaxAllLnas as i8).into())?;
    rf.set_gain_stage(GainStage::RxVga1, (TX_CAL_RX_VGA1).into())?;
    rf.set_gain_stage(GainStage::RxVga2, (TX_CAL_RX_VGA2).into())?;
    rf.set_sample_rate(Channel::Tx, TX_CAL_RATE)?;
    rf.set_loopback(Loopback::Lna1)?;
    Ok(())
}

fn tx_cal_update_frequency(
    rf: &mut RfLinkSession<'_>,
    cal: &mut TxCalState,
    freq: u64,
) -> Result<()> {
    rf.set_frequency(Channel::Tx, freq, TuningMode::Fpga)?;
    let rx_freq = freq - 1_000_000;
    cal.rx_low = rx_freq >= get_frequency_min() as u64;
    let actual_rx_freq = if cal.rx_low {
        rx_freq
    } else {
        freq + 1_000_000
    };
    rf.set_frequency(Channel::Rx, actual_rx_freq, TuningMode::Fpga)?;
    let lb = if freq < 1_500_000_000 {
        Loopback::Lna1
    } else {
        Loopback::Lna2
    };
    if cal.loopback != lb {
        rf.set_loopback(lb.clone())?;
        cal.loopback = lb;
    }
    Ok(())
}

fn tx_cal_mix(samples: &[i16], rx_low: bool) -> Vec<ComplexF> {
    let phase_inc: f32 = if rx_low {
        std::f32::consts::FRAC_PI_2
    } else {
        -std::f32::consts::FRAC_PI_2
    };
    samples
        .chunks_exact(2)
        .enumerate()
        .map(|(n, iq)| {
            let s = ComplexF::new(iq[0] as f32 / 2048.0, iq[1] as f32 / 2048.0);
            s * ComplexF::cis(phase_inc * n as f32)
        })
        .collect()
}

fn tx_cal_filter(post_mix: &[ComplexF]) -> Vec<ComplexF> {
    let n_taps = TX_CAL_FILT.len();
    post_mix
        .windows(n_taps)
        .map(|window| {
            TX_CAL_FILT
                .iter()
                .rev()
                .zip(window)
                .map(|(c, s)| ComplexF::new(*c, 0.0) * *s)
                .sum()
        })
        .collect()
}

fn tx_cal_avg_magnitude(stream: &mut RxStream, cal: &mut TxCalState) -> Result<f32> {
    let num_samples = TX_CAL_COUNT as usize;
    let mut samples = vec![0i16; num_samples * 2];
    rx_samples_sync(stream, &mut samples, TX_CAL_COUNT)?;
    let post_mix = tx_cal_mix(&samples, cal.rx_low);
    let filt_out = tx_cal_filter(&post_mix);
    let start = (TX_CAL_FILT.len() - 1) / 2;
    let count = filt_out.len() - start;
    let avg_mag = filt_out[start..].iter().map(|s| s.norm()).sum::<f32>() / count as f32;
    Ok(avg_mag * 2048.0)
}

fn tx_cal_measure_correction(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    cal: &mut TxCalState,
    corr: &Correction,
    value: i16,
) -> Result<f32> {
    rf.set_correction(Channel::Tx, corr, value)?;
    cal.ts += TX_CAL_TS_INC;
    let mag = tx_cal_avg_magnitude(stream, cal)?;
    log::debug!("  Corr={value:5}, Avg_magnitude={mag:.2}");
    Ok(mag)
}

fn tx_cal_get_corr(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    cal: &mut TxCalState,
    i_ch: bool,
) -> Result<(i16, f32)> {
    let x: [i16; 4] = [-1800, -1000, 1000, 1800];
    let corr = if i_ch {
        &Correction::DcOffI
    } else {
        &Correction::DcOffQ
    };

    log::debug!(
        "Getting coarse estimate for {}",
        if i_ch { 'I' } else { 'Q' }
    );

    let mut mag = [0.0f32; 4];
    for n in 0..4 {
        mag[n] = tx_cal_measure_correction(rf, stream, cal, corr, x[n])?;
    }

    let m1 = (mag[1] - mag[0]) / (x[1] as f32 - x[0] as f32);
    let b1 = mag[0] - m1 * x[0] as f32;
    let m2 = (mag[3] - mag[2]) / (x[3] as f32 - x[2] as f32);
    let b2 = mag[2] - m2 * x[2] as f32;

    let (range_min, range_max) = if m1 < 0.0 && m2 > 0.0 {
        let tmp = ((b2 - b1) / (m1 - m2) + 0.5) as i16;
        let corr_est = (tmp / 16) * 16;
        let n_sweep: i16 = 10;
        log::debug!("  corr_est={corr_est}");
        let r_min = (corr_est - 16 * n_sweep).max(-2048);
        let r_max = (corr_est + 16 * n_sweep).min(2048);
        (r_min, r_max)
    } else {
        log::debug!("  Could not compute estimate. Performing full sweep.");
        (-2048i16, 2048i16)
    };

    log::debug!("Performing correction value sweep: [{range_min:5} : 16 :{range_max:5}]");

    let sweep: Vec<i16> = (range_min..=range_max)
        .step_by(16)
        .take(TX_CAL_CORR_SWEEP_LEN)
        .collect();
    let mut min_corr: i16 = 0;
    let mut min_mag: f32 = 2048.0;

    for &c in &sweep {
        let tmp = tx_cal_measure_correction(rf, stream, cal, corr, c)?;
        let abs_tmp = tmp.abs();
        if abs_tmp < min_mag {
            min_corr = c;
            min_mag = abs_tmp;
        }
    }

    rf.set_correction(Channel::Tx, corr, min_corr)?;
    Ok((min_corr, min_mag))
}

fn perform_tx_cal(
    rf: &mut RfLinkSession<'_>,
    stream: &mut RxStream,
    cal: &mut TxCalState,
    params: &mut DcCalParams,
) -> Result<()> {
    tx_cal_update_frequency(rf, cal, params.frequency)?;
    cal.ts += TX_CAL_TS_INC;

    let (corr_i, error_i) = tx_cal_get_corr(rf, stream, cal, true)?;
    params.corr_i = corr_i;
    params.error_i = error_i;

    let (corr_q, error_q) = tx_cal_get_corr(rf, stream, cal, false)?;
    params.corr_q = corr_q;
    params.error_q = error_q;

    let (corr_i2, error_i2) = tx_cal_get_corr(rf, stream, cal, true)?;
    params.corr_i = corr_i2;
    params.error_i = error_i2;

    rf.set_correction(Channel::Tx, &Correction::DcOffI, corr_i2)?;
    rf.set_correction(Channel::Tx, &Correction::DcOffQ, corr_q)?;
    Ok(())
}

fn dc_calibration_tx(
    rf: &mut RfLinkSession<'_>,
    f_min: u64,
    f_max: u64,
    f_inc: u64,
) -> Result<Vec<DcCalParams>> {
    let backup = get_tx_cal_backup(rf)?;
    apply_tx_cal_settings(rf)?;

    let buf_size = TX_CAL_COUNT as usize * 4;
    let mut tx_stream = TxStream::builder(rf)
        .buffer_size(buf_size)
        .buffer_count(4)
        .format(SampleFormat::Sc16Q11)
        .build()?;

    tx_stream.start(rf)?;

    let mut zero_buf = tx_stream.get_buffer(None)?;
    zero_buf.clear();
    zero_buf.extend_from_slice(&[0u8; 512]);
    tx_stream.submit(zero_buf, 512)?;

    let mut rx_stream = RxStream::builder(rf)
        .buffer_size(buf_size)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;

    rx_stream.start(rf)?;

    let mut cal = TxCalState {
        ts: rf.get_timestamp(Channel::Rx)? + 20 * TX_CAL_TS_INC,
        loopback: Loopback::Lna1,
        rx_low: true,
    };

    let mut results = Vec::new();
    let mut freq = f_min;

    while freq <= f_max {
        let mut params = DcCalParams {
            frequency: freq,
            corr_i: 0,
            corr_q: 0,
            error_i: 0.0,
            error_q: 0.0,
            max_dc: DcPair::default(),
            mid_dc: DcPair::default(),
            min_dc: DcPair::default(),
        };
        perform_tx_cal(rf, &mut rx_stream, &mut cal, &mut params)?;
        log::info!(
            "Calibrated @ {:10} Hz: I={:4} (Error: {:.2}), Q={:4} (Error: {:.2})",
            params.frequency,
            params.corr_i,
            params.error_i,
            params.corr_q,
            params.error_q,
        );
        results.push(params);
        if freq == f_max {
            break;
        }
        freq = (freq + f_inc).min(f_max);
    }

    rx_stream.stop(rf)?;
    tx_stream.stop(rf)?;
    set_tx_cal_backup(rf, &backup)?;
    Ok(results)
}

fn calibrate_and_save_table(
    bladerf: &mut BladeRf1,
    channel: Channel,
    f_min: u64,
    f_max: u64,
    f_inc: u64,
) -> Result<DcCalTable> {
    let mut rf = bladerf.rf_link_session()?;
    rf.initialize(true)?;

    rf.calibrate_dc(DcCalModule::LpfTuning)?;

    let dc_cals = rf.get_dc_cals()?;

    let params = if channel == Channel::Rx {
        dc_calibration_rx(&mut rf, f_min, f_max, f_inc)?
    } else {
        dc_calibration_tx(&mut rf, f_min, f_max, f_inc)?
    };

    let mut entries = Vec::new();
    for p in &params {
        let mut e = DcCalEntry::new(p.frequency as u32, DcPair::new(p.corr_i, p.corr_q));
        if channel == Channel::Rx {
            e = e.with_agc(p.max_dc, p.mid_dc, p.min_dc);
        }
        entries.push(e);
    }
    let table = DcCalTable::new(dc_cals, entries);

    let serial = bladerf.serial()?;
    let filename = if channel == Channel::Rx {
        format!("{serial}_dc_rx.json")
    } else {
        format!("{serial}_dc_tx.json")
    };
    table.save(Path::new(&filename))?;

    bladerf.set_dc_cal_table(channel, table.clone());

    Ok(table)
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf = BladeRf1::from_first()?;

    let channel = match std::env::args().nth(1).as_deref() {
        Some("tx") => Channel::Tx,
        _ => Channel::Rx,
    };

    let f_min: u64 = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("usage: dc_cal_table <rx|tx> <f_min> <f_max> <f_inc>"))?
        .parse()?;
    let f_max: u64 = std::env::args()
        .nth(3)
        .ok_or_else(|| anyhow::anyhow!("missing f_max"))?
        .parse()?;
    let f_inc: u64 = std::env::args()
        .nth(4)
        .ok_or_else(|| anyhow::anyhow!("missing f_inc"))?
        .parse()?;

    log::info!(
        "Calibrating {} DC table: {}-{} Hz, step {} Hz",
        if channel == Channel::Rx { "RX" } else { "TX" },
        f_min,
        f_max,
        f_inc,
    );

    let table = calibrate_and_save_table(&mut bladerf, channel, f_min, f_max, f_inc)?;
    log::info!("Table saved with {} entries", table.entries().len());

    Ok(())
}
