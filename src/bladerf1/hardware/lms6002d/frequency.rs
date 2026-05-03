use crate::Channel;
use crate::bladerf1::hardware::lms6002d::Band;
use crate::bladerf1::hardware::lms6002d::{
    LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND, LMS_FREQ_XB_200_ENABLE,
    LMS_FREQ_XB_200_FILTER_SW_SHIFT, LMS_FREQ_XB_200_MODULE_RX, LMS_FREQ_XB_200_PATH_SHIFT,
    VCO_HIGH, VCO_LOW, VCO_NORM, VCOCAP_EST_MIN, VCOCAP_EST_RANGE, VCOCAP_MAX_LOW_HIGH,
    VCOCAP_MAX_VALUE, VTUNE_DELAY_LARGE, VTUNE_DELAY_SMALL, VTUNE_MAX_ITERATIONS,
};
use crate::bladerf1::nios_client::NiosClient;
use crate::error::Error;
use std::thread::sleep;
use std::time::Duration;
pub const BLADERF_FREQUENCY_MIN_XB200: u32 = 0;
pub const BLADERF_FREQUENCY_MIN: u32 = 237_500_000;
pub const BLADERF_FREQUENCY_MAX: u32 = 3_800_000_000;
const LMS_REFERENCE_HZ: u32 = 38_400_000;
pub struct QuickTune {
    pub freqsel: u8,
    pub vcocap: u8,
    pub nint: u16,
    pub nfrac: u32,
    pub flags: u8,
    pub xb_gpio: u8,
}
pub const VCO4_LOW: u64 = 3_800_000_000;
pub const VCO4_HIGH: u64 = 4_535_000_000;
pub const VCO3_LOW: u64 = VCO4_HIGH;
pub const VCO3_HIGH: u64 = 5_408_000_000;
pub const VCO2_LOW: u64 = VCO3_HIGH;
pub const VCO2_HIGH: u64 = 6_480_000_000;
pub const VCO1_LOW: u64 = VCO2_HIGH;
pub const VCO1_HIGH: u64 = 7_600_000_000;
pub const VCO4: u8 = 4 << 3;
pub const VCO3: u8 = 5 << 3;
pub const VCO2: u8 = 6 << 3;
pub const VCO1: u8 = 7 << 3;
pub const DIV2: u8 = 0x4;
pub const DIV4: u8 = 0x5;
pub const DIV8: u8 = 0x6;
pub const DIV16: u8 = 0x7;
pub struct FreqRange {
    low: u64,
    high: u64,
    value: u8,
}
pub const BANDS: [FreqRange; 16] = [
    FreqRange {
        low: BLADERF_FREQUENCY_MIN as u64,
        high: VCO4_HIGH / 16,
        value: VCO4 | DIV16,
    },
    FreqRange {
        low: VCO3_LOW / 16,
        high: VCO3_HIGH / 16,
        value: VCO3 | DIV16,
    },
    FreqRange {
        low: VCO2_LOW / 16,
        high: VCO2_HIGH / 16,
        value: VCO2 | DIV16,
    },
    FreqRange {
        low: VCO1_LOW / 16,
        high: VCO1_HIGH / 16,
        value: VCO1 | DIV16,
    },
    FreqRange {
        low: VCO4_LOW / 8,
        high: VCO4_HIGH / 8,
        value: VCO4 | DIV8,
    },
    FreqRange {
        low: VCO3_LOW / 8,
        high: VCO3_HIGH / 8,
        value: VCO3 | DIV8,
    },
    FreqRange {
        low: VCO2_LOW / 8,
        high: VCO2_HIGH / 8,
        value: VCO2 | DIV8,
    },
    FreqRange {
        low: VCO1_LOW / 8,
        high: VCO1_HIGH / 8,
        value: VCO1 | DIV8,
    },
    FreqRange {
        low: VCO4_LOW / 4,
        high: VCO4_HIGH / 4,
        value: VCO4 | DIV4,
    },
    FreqRange {
        low: VCO3_LOW / 4,
        high: VCO3_HIGH / 4,
        value: VCO3 | DIV4,
    },
    FreqRange {
        low: VCO2_LOW / 4,
        high: VCO2_HIGH / 4,
        value: VCO2 | DIV4,
    },
    FreqRange {
        low: VCO1_LOW / 4,
        high: VCO1_HIGH / 4,
        value: VCO1 | DIV4,
    },
    FreqRange {
        low: VCO4_LOW / 2,
        high: VCO4_HIGH / 2,
        value: VCO4 | DIV2,
    },
    FreqRange {
        low: VCO3_LOW / 2,
        high: VCO3_HIGH / 2,
        value: VCO3 | DIV2,
    },
    FreqRange {
        low: VCO2_LOW / 2,
        high: VCO2_HIGH / 2,
        value: VCO2 | DIV2,
    },
    FreqRange {
        low: VCO1_LOW / 2,
        high: BLADERF_FREQUENCY_MAX as u64,
        value: VCO1 | DIV2,
    },
];
#[derive(Debug, Default, Clone)]
pub struct LmsFreq {
    pub freqsel: u8,
    pub vcocap: u8,
    pub nint: u16,
    pub nfrac: u32,
    pub flags: u8,
    pub xb_gpio: u8,
    pub x: u8,
    pub vcocap_result: u8,
}
impl From<&LmsFreq> for u64 {
    fn from(value: &LmsFreq) -> Self {
        let pll_coeff = ((value.nint as u64) << 23) + value.nfrac as u64;
        let div = (value.x as u64) << 23;
        let numerator =
            (LMS_REFERENCE_HZ as u128 * pll_coeff as u128 + (div as u128 >> 1)) / div as u128;
        numerator as u64
    }
}
impl TryFrom<u64> for LmsFreq {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        fn estimate_vcocap(f_target: u32, f_low: u32, f_high: u32) -> u8 {
            let denom: f32 = (f_high - f_low) as f32;
            let num: f32 = VCOCAP_EST_RANGE as f32;
            let f_diff: f32 = (f_target - f_low) as f32;
            let vcocap = (num / denom * f_diff) + 0.5 + VCOCAP_EST_MIN as f32;
            if vcocap > VCOCAP_MAX_VALUE as f32 {
                log::debug!("Clamping VCOCAP estimate from {vcocap} to {VCOCAP_MAX_VALUE}");
                VCOCAP_MAX_VALUE
            } else {
                log::debug!("VCOCAP estimate: {vcocap}");
                vcocap as u8
            }
        }
        let mut f: LmsFreq = LmsFreq::default();
        let freq = value.clamp(BLADERF_FREQUENCY_MIN as u64, BLADERF_FREQUENCY_MAX as u64);
        let freq_range = BANDS
            .iter()
            .find(|freq_range| (freq >= freq_range.low) && (freq <= freq_range.high))
            .ok_or(Error::Argument(
                "Could not determine frequency range".into(),
            ))?;
        f.freqsel = freq_range.value;
        log::trace!("f.freqsel: {}", f.freqsel);
        f.vcocap = estimate_vcocap(freq as u32, freq_range.low as u32, freq_range.high as u32);
        log::trace!("f.vcocap: {}", f.vcocap);
        let vco_x = 1 << ((f.freqsel & 7) - 3);
        log::trace!("vco_x: {vco_x}");
        if vco_x > u8::MAX as u64 {
            return Err(Error::HardwareState("VCO divider out of u8 range"));
        }
        f.x = vco_x as u8;
        log::trace!("f.x: {}", f.x);
        let mut temp = (vco_x * freq) / LMS_REFERENCE_HZ as u64;
        if temp > u16::MAX as u64 {
            return Err(Error::Argument(
                "frequency results in nint exceeding u16 range".into(),
            ));
        }
        f.nint = temp as u16;
        log::trace!("f.nint: {}", f.nint);
        let nfrac_num = (1u64 << 23) * (vco_x * freq - f.nint as u64 * LMS_REFERENCE_HZ as u64);
        temp = (nfrac_num + LMS_REFERENCE_HZ as u64 / 2) / LMS_REFERENCE_HZ as u64;
        if temp > u32::MAX as u64 {
            return Err(Error::HardwareState("nfrac exceeds u32 range"));
        }
        f.nfrac = temp as u32;
        log::trace!("f.nfrac: {}", f.nfrac);
        if Band::from(freq) == Band::Low {
            f.flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
        log::trace!("f.flags: {}", f.flags);
        Ok(f)
    }
}
pub const fn get_frequency_min() -> u32 {
    BLADERF_FREQUENCY_MIN
}
pub const fn get_frequency_max() -> u32 {
    BLADERF_FREQUENCY_MAX
}
pub(crate) fn config_charge_pumps(nios: &mut NiosClient, channel: Channel) -> crate::Result<()> {
    let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
    let mut data = super::read(nios, base + 6)?;
    data &= !0x1f;
    data |= 0x0c;
    super::write(nios, base + 6, data)?;
    data = super::read(nios, base + 7)?;
    data &= !0x1f;
    data |= 0x03;
    super::write(nios, base + 7, data)?;
    data = super::read(nios, base + 8)?;
    data &= !0x1f;
    data |= 0x03;
    super::write(nios, base + 8, data)
}
pub(crate) fn write_vcocap(
    nios: &mut NiosClient,
    base: u8,
    vcocap: u8,
    vcocap_reg_state: u8,
) -> crate::Result<()> {
    if vcocap > VCOCAP_MAX_VALUE {
        return Err(Error::Argument("vcocap exceeds maximum value".into()));
    }
    log::trace!("Writing VCOCAP={vcocap}");
    super::write(nios, base + 9, vcocap | vcocap_reg_state)
}
pub(crate) fn get_vtune(nios: &mut NiosClient, base: u8, delay: u8) -> crate::Result<u8> {
    if delay != 0 {
        sleep(Duration::from_micros(delay as u64));
    }
    let vtune = super::read(nios, base + 10)?;
    Ok(vtune >> 6)
}
pub(crate) fn set_precalculated_frequency(
    nios: &mut NiosClient,
    channel: Channel,
    f: &mut LmsFreq,
) -> crate::Result<()> {
    let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
    let pll_base: u8 = base | 0x80;
    f.vcocap_result = 0xff;
    let mut data = super::read(nios, 0x09)?;
    data |= 0x05;
    super::write(nios, 0x09, data)?;
    let vcocap_reg_state = match super::read(nios, base + 9) {
        Ok(v) => v,
        Err(e) => {
            turn_off_dsms(nios)?;
            log::error!(
                "Failed to read vcocap regstate! Device requires re-initialization (call initialize()) to restore DSM state."
            );
            return Err(e);
        }
    };
    let vcocap_reg_state = vcocap_reg_state & !0x3f;
    if let Err(e) = write_vcocap(nios, base, f.vcocap, vcocap_reg_state) {
        turn_off_dsms(nios)?;
        log::error!(
            "Failed to write vcocap_reg_state! Device requires re-initialization (call initialize()) to restore DSM state."
        );
        return Err(e);
    }
    let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
    let lben_lbrfen = super::read(nios, 0x08)?;
    let loopbben = super::read(nios, 0x46)?;
    let lb_enabled =
        matches!(lben_lbrfen & 0x7, 1..=3) || (lben_lbrfen & 0x70 != 0 && loopbben & 0x0c != 0);
    if let Err(e) = write_pll_config(nios, channel, f.freqsel, low_band, lb_enabled) {
        turn_off_dsms(nios)?;
        log::error!(
            "Failed to write pll_config! Device requires re-initialization (call initialize()) to restore DSM state."
        );
        return Err(e);
    }
    let mut freq_data = [0u8; 4];
    freq_data[0] = (f.nint >> 1) as u8;
    freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
    freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
    freq_data[3] = (f.nfrac & 0xff) as u8;
    for (idx, value) in freq_data.iter().enumerate() {
        if let Err(e) = super::write(nios, pll_base + idx as u8, *value) {
            turn_off_dsms(nios)?;
            log::error!(
                "Failed to write pll {}! Device requires re-initialization (call initialize()) to restore DSM state.",
                pll_base + idx as u8
            );
            return Err(e);
        }
    }
    if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
        f.vcocap_result = f.vcocap;
    } else {
        log::trace!("Tuning VCOCAP...");
        f.vcocap_result = tune_vcocap(nios, f.vcocap, base, vcocap_reg_state)?;
    }
    Ok(())
}
pub(crate) fn set_frequency(
    nios: &mut NiosClient,
    channel: Channel,
    frequency: u64,
) -> crate::Result<()> {
    let mut f = frequency.try_into()?;
    log::trace!("{f:?}");
    set_precalculated_frequency(nios, channel, &mut f)
}
pub(crate) fn get_frequency(nios: &mut NiosClient, channel: Channel) -> crate::Result<LmsFreq> {
    let mut f = LmsFreq::default();
    let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
    let mut data = super::read(nios, base)?;
    f.nint = (data as u16) << 1;
    data = super::read(nios, base + 1)?;
    f.nint |= ((data & 0x80) >> 7) as u16;
    f.nfrac = (data as u32 & 0x7f) << 16;
    data = super::read(nios, base + 2)?;
    f.nfrac |= (data as u32) << 8;
    data = super::read(nios, base + 3)?;
    f.nfrac |= data as u32;
    data = super::read(nios, base + 5)?;
    f.freqsel = data >> 2;
    f.x = 1 << ((f.freqsel & 7) - 3);
    data = super::read(nios, base + 9)?;
    f.vcocap = data & 0x3f;
    Ok(f)
}
#[allow(dead_code)]
pub(crate) fn peakdetect_enable(nios: &mut NiosClient, enable: bool) -> crate::Result<()> {
    let mut data = super::read(nios, 0x44)?;
    if enable {
        data &= !(1 << 0);
    } else {
        data |= 1;
    }
    super::write(nios, 0x44, data)
}
#[allow(dead_code)]
pub(crate) fn get_quick_tune(
    nios: &mut NiosClient,
    channel: Channel,
    xb200_enabled: bool,
) -> crate::Result<QuickTune> {
    let f = &get_frequency(nios, channel)?;
    let mut quick_tune = QuickTune {
        freqsel: f.freqsel,
        vcocap: f.vcocap,
        nint: f.nint,
        nfrac: f.nfrac,
        flags: 0,
        xb_gpio: 0,
    };
    if xb200_enabled {
        let val = nios.nios_expansion_gpio_read()?;
        quick_tune.xb_gpio |= LMS_FREQ_XB_200_ENABLE;
        match channel {
            Channel::Rx => {
                quick_tune.xb_gpio |= LMS_FREQ_XB_200_MODULE_RX;
                quick_tune.xb_gpio |= (((val & 0x30) >> 4) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                quick_tune.xb_gpio |=
                    (((val & 0x30000000) >> 28) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
            }
            Channel::Tx => {
                quick_tune.xb_gpio |=
                    (((val & 0x0C) >> 2) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
                quick_tune.xb_gpio |=
                    (((val & 0x0C000000) >> 26) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
            }
        }
        quick_tune.flags = LMS_FREQ_FLAGS_FORCE_VCOCAP;
        let f_hz: u64 = f.into();
        if Band::from(f_hz) == Band::Low {
            quick_tune.flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
    }
    Ok(quick_tune)
}
pub(crate) fn write_pll_config(
    nios: &mut NiosClient,
    channel: Channel,
    freqsel: u8,
    low_band: bool,
    lb_enabled: bool,
) -> crate::Result<()> {
    let addr = if channel == Channel::Tx { 0x15 } else { 0x25 };
    let mut regval = super::read(nios, addr)?;
    if !lb_enabled {
        let selout = if low_band { 1 } else { 2 };
        regval = (freqsel << 2) | selout;
    } else {
        regval = (regval & !0xfc) | (freqsel << 2);
    }
    super::write(nios, addr, regval)
}
pub(crate) fn vtune_high_to_norm(
    nios: &mut NiosClient,
    base: u8,
    mut vcocap: u8,
    vcocap_reg_state: u8,
) -> crate::Result<u8> {
    for _ in 0..VTUNE_MAX_ITERATIONS {
        if vcocap >= VCOCAP_MAX_VALUE {
            log::trace!("vtune_high_to_norm: VCOCAP hit max value.");
            return Ok(VCOCAP_MAX_VALUE);
        }
        vcocap += 1;
        write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
        let vtune = get_vtune(nios, base, VTUNE_DELAY_SMALL)?;
        if vtune == VCO_NORM {
            log::trace!("VTUNE NORM @ VCOCAP={vcocap}");
            return Ok(vcocap - 1);
        }
    }
    log::error!("VTUNE High->Norm loop failed to converge.");
    Err(Error::CalibrationFailed(
        "VTUNE High->Norm loop failed to converge",
    ))
}
pub(crate) fn vtune_norm_to_high(
    nios: &mut NiosClient,
    base: u8,
    mut vcocap: u8,
    vcocap_reg_state: u8,
) -> crate::Result<u8> {
    for _ in 0..VTUNE_MAX_ITERATIONS {
        log::trace!("base: {base}, vcocap: {vcocap}, vcocap_reg_state: {vcocap_reg_state}");
        if vcocap == 0 {
            log::debug!("vtune_norm_to_high: VCOCAP hit min value.");
            return Ok(0);
        }
        vcocap -= 1;
        write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
        let vtune = get_vtune(nios, base, VTUNE_DELAY_SMALL)?;
        log::trace!("vtune: {vtune}");
        if vtune == VCO_HIGH {
            log::debug!("VTUNE HIGH @ VCOCAP={vcocap}");
            return Ok(vcocap);
        }
    }
    log::error!("VTUNE Norm->High loop failed to converge.");
    Err(Error::CalibrationFailed(
        "VTUNE Norm->High loop failed to converge",
    ))
}
pub(crate) fn vtune_low_to_norm(
    nios: &mut NiosClient,
    base: u8,
    mut vcocap: u8,
    vcocap_reg_state: u8,
) -> crate::Result<u8> {
    for _ in 0..VTUNE_MAX_ITERATIONS {
        if vcocap == 0 {
            log::debug!("vtune_low_to_norm: VCOCAP hit min value.");
            return Ok(0);
        }
        vcocap -= 1;
        write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
        let vtune = get_vtune(nios, base, VTUNE_DELAY_SMALL)?;
        if vtune == VCO_NORM {
            log::debug!("VTUNE NORM @ VCOCAP={vcocap}");
            return Ok(vcocap + 1);
        }
    }
    log::error!("VTUNE Low->Norm loop failed to converge.");
    Err(Error::CalibrationFailed(
        "VTUNE Low->Norm loop failed to converge",
    ))
}
pub(crate) fn wait_for_vtune_value(
    nios: &mut NiosClient,
    base: u8,
    target_value: u8,
    vcocap: &mut u8,
    vcocap_reg_state: u8,
) -> crate::Result<()> {
    const MAX_RETRIES: u32 = 15;
    let limit: u8 = if target_value == VCO_HIGH {
        0
    } else {
        VCOCAP_MAX_VALUE
    };
    let inc: i8 = if target_value == VCO_HIGH { -1 } else { 1 };
    if target_value != VCO_HIGH && target_value != VCO_LOW {
        return Err(Error::Argument(
            "wait_for_vtune_value: target must be VCO_HIGH or VCO_LOW".into(),
        ));
    }
    for i in 0..MAX_RETRIES {
        let vtune = get_vtune(nios, base, 0)?;
        if vtune == target_value {
            log::debug!("VTUNE reached {target_value} at iteration {i}");
            return Ok(());
        } else {
            log::trace!("VTUNE was {vtune}. Waiting and retrying...");
            sleep(Duration::from_micros(10));
        }
    }
    log::trace!("Timed out while waiting for VTUNE={target_value}. Walking VCOCAP...");
    while *vcocap != limit {
        *vcocap = (*vcocap as i8 + inc) as u8;
        write_vcocap(nios, base, *vcocap, vcocap_reg_state)?;
        let vtune = get_vtune(nios, base, VTUNE_DELAY_SMALL)?;
        if vtune == target_value {
            log::debug!("VTUNE={vtune} reached with VCOCAP={vcocap}");
            return Ok(());
        }
    }
    log::debug!("VTUNE did not reach {target_value}. Tuning may not be nominal.");
    Ok(())
}
pub(crate) fn tune_vcocap(
    nios: &mut NiosClient,
    vcocap_est: u8,
    base: u8,
    vcocap_reg_state: u8,
) -> crate::Result<u8> {
    let mut vcocap: u8 = vcocap_est;
    let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE;
    let mut vtune_low_limit: u8 = 0;
    let mut vtune = get_vtune(nios, base, VTUNE_DELAY_LARGE)?;
    match vtune {
        VCO_HIGH => {
            log::trace!("Estimate HIGH: Walking down to NORM.");
            vtune_high_limit = vtune_high_to_norm(nios, base, vcocap, vcocap_reg_state)?;
        }
        VCO_NORM => {
            log::trace!("Estimate NORM: Walking up to HIGH.");
            vtune_high_limit = vtune_norm_to_high(nios, base, vcocap, vcocap_reg_state)?;
        }
        VCO_LOW => {
            log::trace!("Estimate LOW: Walking down to NORM.");
            vtune_low_limit = vtune_low_to_norm(nios, base, vcocap, vcocap_reg_state)?;
        }
        _ => {}
    }
    if vtune_high_limit != VCOCAP_MAX_VALUE {
        match vtune {
            VCO_NORM | VCO_HIGH => {
                if (vtune_high_limit + VCOCAP_MAX_LOW_HIGH) < VCOCAP_MAX_VALUE {
                    vcocap = vtune_high_limit + VCOCAP_MAX_LOW_HIGH;
                } else {
                    vcocap = VCOCAP_MAX_VALUE;
                    log::debug!("Clamping VCOCAP to {vcocap}.");
                }
            }
            _ => {
                log::error!("Invalid state");
                return Err(Error::HardwareState(
                    "VTUNE state mismatch after high_limit",
                ));
            }
        }
        write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
        log::trace!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
        wait_for_vtune_value(nios, base, VCO_LOW, &mut vcocap, vcocap_reg_state)?;
        log::trace!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
        vtune_low_limit = vtune_low_to_norm(nios, base, vcocap, vcocap_reg_state)?;
    } else {
        match vtune {
            VCO_LOW | VCO_NORM => {
                if (vtune_low_limit - VCOCAP_MAX_LOW_HIGH) > 0 {
                    vcocap = vtune_low_limit - VCOCAP_MAX_LOW_HIGH;
                } else {
                    vcocap = 0;
                    log::debug!("Clamping VCOCAP to {vcocap}.");
                }
            }
            _ => {
                log::error!("Invalid state");
                return Err(Error::HardwareState("VTUNE state mismatch after low_limit"));
            }
        }
        write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
        log::trace!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
        wait_for_vtune_value(nios, base, VCO_HIGH, &mut vcocap, vcocap_reg_state)?;
        log::trace!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
        vtune_high_limit = vtune_high_to_norm(nios, base, vcocap, vcocap_reg_state)?;
    }
    vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;
    log::trace!("VTUNE LOW:   {vtune_low_limit}");
    log::trace!("VTUNE NORM:  {vcocap}");
    log::trace!("VTUNE Est:   {vcocap_est}");
    log::trace!("VTUNE HIGH:  {vtune_high_limit}");
    write_vcocap(nios, base, vcocap, vcocap_reg_state)?;
    vtune = get_vtune(nios, base, VTUNE_DELAY_SMALL)?;
    if vtune != VCO_NORM {
        log::error!("Final VCOCAP={vcocap} is not in VTUNE NORM region.");
        return Err(Error::TuningFailed);
    }
    Ok(vcocap)
}
pub(crate) fn turn_off_dsms(nios: &mut NiosClient) -> crate::Result<()> {
    let mut data = super::read(nios, 0x09)?;
    data &= !0x05;
    super::write(nios, 0x09, data)
}
