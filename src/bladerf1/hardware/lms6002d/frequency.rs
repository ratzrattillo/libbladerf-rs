use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d::bandwidth::BLADERF1_BAND_HIGH;
use crate::bladerf1::hardware::lms6002d::{
    LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND, LMS_FREQ_XB_200_ENABLE,
    LMS_FREQ_XB_200_FILTER_SW_SHIFT, LMS_FREQ_XB_200_MODULE_RX, LMS_FREQ_XB_200_PATH_SHIFT,
    LMS6002D, VCO_HIGH, VCO_LOW, VCO_NORM, VCOCAP_EST_MIN, VCOCAP_EST_RANGE, VCOCAP_MAX_LOW_HIGH,
    VCOCAP_MAX_VALUE, VTUNE_DELAY_LARGE, VTUNE_DELAY_SMALL, VTUNE_MAX_ITERATIONS,
};
use crate::{Channel, Error};
use std::thread::sleep;
use std::time::Duration;
pub const BLADERF_FREQUENCY_MIN_XB200: u32 = 0;
pub const BLADERF_FREQUENCY_MIN: u32 = 237500000;
pub const BLADERF_FREQUENCY_MAX: u32 = 3800000000;
const LMS_REFERENCE_HZ: u32 = 38400000;
pub struct QuickTune {
    pub freqsel: u8,
    pub vcocap: u8,
    pub nint: u16,
    pub nfrac: u32,
    pub flags: u8,
    pub xb_gpio: u8,
}
pub const VCO4_LOW: u64 = 3800000000;
pub const VCO4_HIGH: u64 = 4535000000;
pub const VCO3_LOW: u64 = VCO4_HIGH;
pub const VCO3_HIGH: u64 = 5408000000;
pub const VCO2_LOW: u64 = VCO3_HIGH;
pub const VCO2_HIGH: u64 = 6480000000;
pub const VCO1_LOW: u64 = VCO2_HIGH;
pub const VCO1_HIGH: u64 = 7600000000;
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
#[derive(Debug, Default)]
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
        ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div
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
            .ok_or(Error::Argument("Could not determine frequency range"))?;
        f.freqsel = freq_range.value;
        log::trace!("f.freqsel: {}", f.freqsel);
        f.vcocap = estimate_vcocap(freq as u32, freq_range.low as u32, freq_range.high as u32);
        log::trace!("f.vcocap: {}", f.vcocap);
        let vco_x = 1 << ((f.freqsel & 7) - 3);
        log::trace!("vco_x: {vco_x}");
        assert!(vco_x <= u8::MAX as u64);
        f.x = vco_x as u8;
        log::trace!("f.x: {}", f.x);
        let mut temp = (vco_x * freq) / LMS_REFERENCE_HZ as u64;
        assert!(temp <= u16::MAX as u64);
        f.nint = temp as u16;
        log::trace!("f.nint: {}", f.nint);
        temp = (1 << 23) * (vco_x * freq - f.nint as u64 * LMS_REFERENCE_HZ as u64);
        temp = (temp + LMS_REFERENCE_HZ as u64 / 2) / LMS_REFERENCE_HZ as u64;
        assert!(temp <= u32::MAX as u64);
        f.nfrac = temp as u32;
        log::trace!("f.nfrac: {}", f.nfrac);
        assert!(LMS_REFERENCE_HZ as u64 <= u32::MAX as u64);
        if freq < BLADERF1_BAND_HIGH as u64 {
            f.flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
        log::trace!("f.flags: {}", f.flags);
        Ok(f)
    }
}
impl LMS6002D {
    pub const fn get_frequency_min() -> u32 {
        BLADERF_FREQUENCY_MIN
    }
    pub const fn get_frequency_max() -> u32 {
        BLADERF_FREQUENCY_MAX
    }
    pub fn config_charge_pumps(&self, channel: Channel) -> crate::Result<()> {
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let mut data = self.read(base + 6)?;
        data &= !0x1f;
        data |= 0x0c;
        self.write(base + 6, data)?;
        data = self.read(base + 7)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 7, data)?;
        data = self.read(base + 8)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 8, data)
    }
    pub fn get_vtune(&self, base: u8, delay: u8) -> crate::Result<u8> {
        if delay != 0 {
            sleep(Duration::from_micros(delay as u64));
        }
        let vtune = self.read(base + 10)?;
        Ok(vtune >> 6)
    }
    pub fn write_vcocap(&self, base: u8, vcocap: u8, vcocap_reg_state: u8) -> crate::Result<()> {
        assert!(vcocap <= VCOCAP_MAX_VALUE);
        log::trace!("Writing VCOCAP={vcocap}");
        self.write(base + 9, vcocap | vcocap_reg_state)
    }
    pub fn set_precalculated_frequency(
        &self,
        channel: Channel,
        f: &mut LmsFreq,
    ) -> crate::Result<()> {
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let pll_base: u8 = base | 0x80;
        f.vcocap_result = 0xff;
        let mut data = self.read(0x09)?;
        data |= 0x05;
        self.write(0x09, data)?;
        let result = self.read(base + 9);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to read vcocap regstate!");
            return Err(Error::Invalid);
        }
        let mut vcocap_reg_state = result?;
        vcocap_reg_state &= !0x3f;
        let result = self.write_vcocap(base, f.vcocap, vcocap_reg_state);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to write vcocap_reg_state!");
            return Err(Error::Invalid);
        }
        let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
        let result = self.write_pll_config(channel, f.freqsel, low_band);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to write pll_config!");
            return Err(Error::Invalid);
        }
        let mut freq_data = [0u8; 4];
        freq_data[0] = (f.nint >> 1) as u8;
        freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
        freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
        freq_data[3] = (f.nfrac & 0xff) as u8;
        for (idx, value) in freq_data.iter().enumerate() {
            let result = self.write(pll_base + idx as u8, *value);
            if result.is_err() {
                self.turn_off_dsms()?;
                log::error!("Failed to write pll {}!", pll_base + idx as u8);
                return Err(Error::Invalid);
            }
        }
        if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            f.vcocap_result = f.vcocap;
        } else {
            log::trace!("Tuning VCOCAP...");
            f.vcocap_result = self.tune_vcocap(f.vcocap, base, vcocap_reg_state)?;
        }
        Ok(())
    }
    pub fn set_frequency(&self, channel: Channel, frequency: u64) -> crate::Result<()> {
        let mut f = frequency.try_into()?;
        log::trace!("{f:?}");
        self.set_precalculated_frequency(channel, &mut f)
    }
    pub fn get_frequency(&self, channel: Channel) -> crate::Result<LmsFreq> {
        let mut f = LmsFreq::default();
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let mut data = self.read(base)?;
        f.nint = (data as u16) << 1;
        data = self.read(base + 1)?;
        f.nint |= ((data & 0x80) >> 7) as u16;
        f.nfrac = (data as u32 & 0x7f) << 16;
        data = self.read(base + 2)?;
        f.nfrac |= (data as u32) << 8;
        data = self.read(base + 3)?;
        f.nfrac |= data as u32;
        data = self.read(base + 5)?;
        f.freqsel = data >> 2;
        f.x = 1 << ((f.freqsel & 7) - 3);
        data = self.read(base + 9)?;
        f.vcocap = data & 0x3f;
        Ok(f)
    }
    pub fn peakdetect_enable(&self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x44)?;
        if enable {
            data &= !(1 << 0);
        } else {
            data |= 1;
        }
        self.write(0x44, data)
    }
    pub fn get_quick_tune(&self, channel: Channel) -> crate::Result<QuickTune> {
        let f = &self.get_frequency(channel)?;
        let mut quick_tune = QuickTune {
            freqsel: f.freqsel,
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags: 0,
            xb_gpio: 0,
        };
        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        if BladeRf1::xb200_is_enabled(&self.interface)? {
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
            if f_hz < BLADERF1_BAND_HIGH as u64 {
                quick_tune.flags |= LMS_FREQ_FLAGS_LOW_BAND;
            }
        }
        Ok(quick_tune)
    }
    pub fn write_pll_config(
        &self,
        channel: Channel,
        freqsel: u8,
        low_band: bool,
    ) -> crate::Result<()> {
        let addr = if channel == Channel::Tx { 0x15 } else { 0x25 };
        let mut regval = self.read(addr)?;
        let lb_enabled: bool = self.is_loopback_enabled()?;
        if !lb_enabled {
            let selout = if low_band { 1 } else { 2 };
            regval = (freqsel << 2) | selout;
        } else {
            regval = (regval & !0xfc) | (freqsel << 2);
        }
        self.write(addr, regval)
    }
    pub fn vtune_high_to_norm(
        &self,
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
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == VCO_NORM {
                log::trace!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap - 1);
            }
        }
        log::error!("VTUNE High->Norm loop failed to converge.");
        Err(Error::Invalid)
    }
    pub fn vtune_norm_to_high(
        &self,
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
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            log::trace!("vtune: {vtune}");
            if vtune == VCO_HIGH {
                log::debug!("VTUNE HIGH @ VCOCAP={vcocap}");
                return Ok(vcocap);
            }
        }
        log::error!("VTUNE Norm->High loop failed to converge.");
        Err(Error::Invalid)
    }
    pub fn vtune_low_to_norm(
        &self,
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
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == VCO_NORM {
                log::debug!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap + 1);
            }
        }
        log::error!("VTUNE Low->Norm loop failed to converge.");
        Err(Error::Invalid)
    }
    pub fn wait_for_vtune_value(
        &self,
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
        assert!(target_value == VCO_HIGH || target_value == VCO_LOW);
        for i in 0..MAX_RETRIES {
            let vtune = self.get_vtune(base, 0)?;
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
            self.write_vcocap(base, *vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == target_value {
                log::debug!("VTUNE={vtune} reached with VCOCAP={vcocap}");
                return Ok(());
            }
        }
        log::debug!("VTUNE did not reach {target_value}. Tuning may not be nominal.");
        Ok(())
    }
    pub fn tune_vcocap(&self, vcocap_est: u8, base: u8, vcocap_reg_state: u8) -> crate::Result<u8> {
        let mut vcocap: u8 = vcocap_est;
        let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE;
        let mut vtune_low_limit: u8 = 0;
        let mut vtune = self.get_vtune(base, VTUNE_DELAY_LARGE)?;
        match vtune {
            VCO_HIGH => {
                log::trace!("Estimate HIGH: Walking down to NORM.");
                vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
            }
            VCO_NORM => {
                log::trace!("Estimate NORM: Walking up to HIGH.");
                vtune_high_limit = self.vtune_norm_to_high(base, vcocap, vcocap_reg_state)?;
            }
            VCO_LOW => {
                log::trace!("Estimate LOW: Walking down to NORM.");
                vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
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
                    return Err(Error::Invalid);
                }
            }
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            log::trace!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_LOW, &mut vcocap, vcocap_reg_state)?;
            log::trace!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
            vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
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
                    return Err(Error::Invalid);
                }
            }
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            log::trace!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_HIGH, &mut vcocap, vcocap_reg_state)?;
            log::trace!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
            vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
        }
        vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;
        log::trace!("VTUNE LOW:   {vtune_low_limit}");
        log::trace!("VTUNE NORM:  {vcocap}");
        log::trace!("VTUNE Est:   {vcocap_est}");
        log::trace!("VTUNE HIGH:  {vtune_high_limit}");
        self.write_vcocap(base, vcocap, vcocap_reg_state)?;
        vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
        if vtune != VCO_NORM {
            log::error!("Final VCOCAP={vcocap} is not in VTUNE NORM region.");
            return Err(Error::Invalid);
        }
        Ok(vcocap)
    }
    pub fn turn_off_dsms(&self) -> crate::Result<()> {
        let mut data = self.read(0x09)?;
        data &= !0x05;
        self.write(0x09, data)
    }
}
