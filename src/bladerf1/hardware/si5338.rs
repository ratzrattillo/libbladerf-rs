//! Si5338 clock generator driver.
//!
//! Generates the sample clock for the LMS6002D via fractional-N PLL synthesis.
//! Provides VCTCXO trim control and SMB clock output.
//! The Si5338 output range is 0.16–710 MHz, achieved through MultiSynth
//! fractional dividers driven by a fixed VCO at 2.5344 GHz.

use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::nios_client::NiosCore;
use crate::protocol::nios::NiosPkt8x8Target;
use crate::range::{Range, RangeItem};

/// Rational rate with integer part and numerator/denominator for fractional-N synthesis.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalRate {
    /// Integer component of the rate.
    integer: u64,
    /// Numerator of the fractional part.
    num: u64,
    /// Denominator of the fractional part.
    den: u64,
}
impl RationalRate {
    pub fn new(integer: u64, num: u64, den: u64) -> Self {
        Self { integer, num, den }
    }

    pub fn integer(&self) -> u64 {
        self.integer
    }

    pub fn numerator(&self) -> u64 {
        self.num
    }

    pub fn denominator(&self) -> u64 {
        self.den
    }
}

const SI5338_F_VCO: u64 = 38_400_000 * 66;
const SI5338_EN_A: u8 = 0x01;
const SI5338_EN_B: u8 = 0x02;
/// Minimum supported sample rate in Hz.
pub const BLADERF_SAMPLERATE_MIN: u32 = 80_000;
/// Recommended maximum sample rate in Hz.
pub const BLADERF_SAMPLERATE_REC_MAX: u32 = 40_000_000;
/// Maximum SMB clock frequency in Hz.
pub const BLADERF_SMB_FREQUENCY_MAX: u32 = 200_000_000;
/// Minimum SMB clock frequency in Hz.
pub const BLADERF_SMB_FREQUENCY_MIN: u32 = (38_400_000 * 66) / (32 * 567);

/// SMB clock operational mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmbMode {
    /// SMB clock is disabled.
    Disabled,
    /// SMB clock is configured as an output.
    Output,
    /// SMB clock is configured as an input reference.
    Input,
    /// SMB clock is unavailable on this device.
    Unavailable,
}

const DEFAULT_CONFIG: &[(u8, u8)] = &[
    (6, 0x08),
    (28, 0x0b),
    (29, 0x08),
    (30, 0xb0),
    (34, 0xe3),
    (39, 0x00),
    (86, 0x00),
    (87, 0x00),
    (88, 0x00),
    (89, 0x00),
    (90, 0x00),
    (91, 0x00),
    (92, 0x00),
    (93, 0x00),
    (94, 0x00),
    (95, 0x00),
];

const INPUT_CONFIG: &[(u8, u8)] = &[(6, 0x04), (28, 0x2b), (29, 0x28), (30, 0xa8)];

const OUTPUT_CONFIG: &[(u8, u8)] = &[(34, 0x22)];

#[derive(Clone, Default)]
pub(crate) struct Multisynth {
    index: u8,
    base: u16,
    enable: u8,
    a: u32,
    b: u32,
    c: u32,
    r: u32,
    p1: u32,
    p2: u32,
    p3: u32,
    regs: [u8; 10],
}

/// Si5338 clock generator interface.
pub struct Si5338<'a> {
    pub(crate) nios: &'a mut NiosCore,
}

impl<'a> Si5338<'a> {
    pub(crate) fn read(&mut self, addr: u8) -> Result<u8> {
        self.nios
            .nios_read::<u8, u8>(NiosPkt8x8Target::Si5338, addr)
    }

    pub(crate) fn write(&mut self, addr: u8, data: u8) -> Result<()> {
        self.nios
            .nios_write::<u8, u8>(NiosPkt8x8Target::Si5338, addr, data)
    }

    fn read_multisynth(&mut self, ms: &mut Multisynth) -> Result<()> {
        let val = self.read(36 + ms.index)?;
        ms.enable = val & 7;
        log::trace!("Read enable register: {val:x}");
        for i in 0..ms.regs.len() {
            ms.regs[i] = self.read(ms.base as u8 + i as u8)?
        }
        let mut val = self.read(31 + ms.index)?;
        val = (val >> 2) & 7;
        ms.r = 1 << val;
        ms.unpack_regs()?;
        Ok(())
    }

    fn write_multisynth(&mut self, ms: &Multisynth) -> Result<()> {
        let mut val = self.read(36 + ms.index)?;
        val |= ms.enable;
        log::trace!("Wrote enable register: {val:x}");
        self.write(36 + ms.index, val)?;
        for i in 0..ms.regs.len() {
            self.write((ms.base + i as u16) as u8, ms.regs[i])?;
            log::trace!("Wrote regs[{i}]: {}", ms.regs[i]);
        }
        let r_power = ms.r.checked_ilog2().unwrap_or(0) as u8;
        let mut val = 0xc0;
        val |= r_power << 2;
        log::trace!("Wrote r register: {val:x}");
        self.write(ms.index + 31, val)
    }

    /// Sets the sample rate for the given channel and returns the actual configured rate.
    ///
    /// May return a fractional rate internally; the returned `u32` is the integer part.
    /// Returns `Error::BoardState` if the actual rate exceeds `u32::MAX`.
    pub fn set_sample_rate(&mut self, channel: Channel, rate: u32) -> Result<u32> {
        let mut req = RationalRate {
            integer: rate as u64,
            num: 0,
            den: 1,
        };
        log::trace!("Setting integer sample rate: {rate}");
        let act = self.set_rational_sample_rate(channel, &mut req)?;
        if act.numerator() != 0 {
            log::debug!("Non-integer sample rate set from integer sample rate, truncating output.");
        }
        if act.integer() > u32::MAX as u64 {
            return Err(Error::BoardState("actual sample rate exceeds u32 range"));
        }
        log::trace!("Set actual integer sample rate: {}", act.integer());
        Ok(act.integer() as u32)
    }

    /// Returns the current integer sample rate for the given channel.
    ///
    /// Truncates any fractional component.
    /// Returns `Error::BoardState` if the actual rate exceeds `u32::MAX`.
    pub fn get_sample_rate(&mut self, channel: Channel) -> Result<u32> {
        let actual = self.get_rational_sample_rate(channel)?;
        if actual.numerator() != 0 {
            log::debug!("Fractional sample rate truncated during integer sample rate retrieval");
        }
        if actual.integer() > u32::MAX as u64 {
            return Err(Error::BoardState("actual sample rate exceeds u32 range"));
        }
        Ok(actual.integer() as u32)
    }

    /// Returns the range of supported sample rates.
    pub fn get_sample_rate_range() -> Range {
        Range::new(vec![RangeItem::Step(
            BLADERF_SAMPLERATE_MIN as f64,
            BLADERF_SAMPLERATE_REC_MAX as f64,
            1f64,
            1f64,
        )])
    }

    /// Sets a rational sample rate and returns the actual configured rate.
    ///
    /// Reduces the input fraction before programming the MultiSynth.
    /// Returns `Error::InvalidSampleRate` if the rate is below the minimum.
    pub fn set_rational_sample_rate(
        &mut self,
        channel: Channel,
        rate: &mut RationalRate,
    ) -> Result<RationalRate> {
        rate.reduce();
        if rate.integer() < BLADERF_SAMPLERATE_MIN as u64 {
            return Err(Error::InvalidSampleRate("sample rate below minimum"));
        }
        let index: u8 = if channel == Channel::Rx { 0x1 } else { 0x2 };
        let mut si_channel: u8 = SI5338_EN_A;
        if channel == Channel::Tx {
            si_channel |= SI5338_EN_B;
        }
        self.rational_multisynth(index, si_channel, rate)
    }

    /// Returns the current rational sample rate for the given channel.
    pub fn get_rational_sample_rate(&mut self, channel: Channel) -> Result<RationalRate> {
        let mut ms = Multisynth {
            index: if channel == Channel::Rx { 1 } else { 2 },
            ..Default::default()
        };
        ms.update_base();
        self.read_multisynth(&mut ms)?;
        let mut rate = RationalRate::default();
        ms.calculate_freq(&mut rate);
        Ok(rate)
    }

    /// Sets the rational SMB clock frequency and returns the actual configured frequency.
    ///
    /// Returns `Error::Argument` if the frequency is outside the supported range.
    pub fn set_rational_smb_freq(&mut self, mut rate: RationalRate) -> Result<RationalRate> {
        rate.reduce();
        if rate.integer() < BLADERF_SMB_FREQUENCY_MIN as u64 {
            log::error!("provided SMB freq violates minimum");
            return Err(Error::Argument("SMB frequency below minimum".into()));
        } else if rate.integer() > BLADERF_SMB_FREQUENCY_MAX as u64 {
            log::error!("provided SMB freq violates maximum");
            return Err(Error::Argument("SMB frequency above maximum".into()));
        }
        self.rational_multisynth(3, SI5338_EN_A, &mut rate)
    }

    /// Sets the SMB clock frequency and returns the actual configured frequency.
    ///
    /// Returns `Error::Argument` if the frequency is out of range, or `Error::BoardState`
    /// if the actual frequency exceeds `u32::MAX`.
    pub fn set_smb_freq(&mut self, rate: u32) -> Result<u32> {
        let req = RationalRate::new(rate as u64, 0, 1);
        log::trace!("Setting integer SMB frequency: {rate}");
        let act = self.set_rational_smb_freq(req)?;
        if act.numerator() != 0 {
            log::trace!("Non-integer SMB frequency set from integer frequency, truncating output.");
        }
        if act.integer() > u32::MAX as u64 {
            return Err(Error::BoardState("actual SMB frequency exceeds u32 range"));
        }
        log::trace!("Set actual integer SMB frequency: {}", act.integer());
        Ok(act.integer() as u32)
    }

    /// Returns the current rational SMB clock frequency.
    pub fn get_rational_smb_freq(&mut self) -> Result<RationalRate> {
        let mut ms = Multisynth::default();
        let mut rate = RationalRate::default();
        ms.index = 3;
        ms.update_base();
        self.read_multisynth(&mut ms)?;
        ms.calculate_freq(&mut rate);
        Ok(rate)
    }

    /// Returns the current integer SMB clock frequency.
    ///
    /// Truncates any fractional component.
    /// Returns `Error::BoardState` if the actual frequency exceeds `u32::MAX`.
    pub fn get_smb_freq(&mut self) -> Result<u32> {
        let actual = self.get_rational_smb_freq()?;
        if actual.numerator() != 0 {
            log::trace!(
                "Fractional SMB frequency truncated during integer SMB frequency retrieval"
            );
        }
        if actual.integer() > u32::MAX as u64 {
            return Err(Error::BoardState("actual SMB frequency exceeds u32 range"));
        }
        Ok(actual.integer() as u32)
    }

    /// Sets the SMB clock mode.
    ///
    /// Returns `Error::Argument` if the mode is `SmbMode::Unavailable`.
    pub fn set_smb_mode(&mut self, mode: SmbMode) -> Result<()> {
        match mode {
            SmbMode::Disabled | SmbMode::Output | SmbMode::Input => {}
            SmbMode::Unavailable => {
                return Err(Error::Argument("cannot set SMB mode to Unavailable".into()));
            }
        }

        for &(addr, data) in DEFAULT_CONFIG {
            self.write(addr, data)?;
        }

        match mode {
            SmbMode::Disabled => Ok(()),
            SmbMode::Output => {
                let mut val = self.read(39)?;
                val |= 1;
                self.write(39, val)?;
                for &(addr, data) in OUTPUT_CONFIG {
                    self.write(addr, data)?;
                }
                Ok(())
            }
            SmbMode::Input => {
                for &(addr, data) in INPUT_CONFIG {
                    self.write(addr, data)?;
                }
                let mut val = self.read(39)?;
                val &= !1;
                self.write(39, val)
            }
            SmbMode::Unavailable => unreachable!(),
        }
    }

    /// Returns the current SMB clock mode.
    ///
    /// Returns `Error::Unsupported` if an unexpected register value is read.
    pub fn get_smb_mode(&mut self) -> Result<SmbMode> {
        let val = self.read(39)?;
        match val & 0x7 {
            0x00 => {}
            0x01 => return Ok(SmbMode::Output),
            0x02 => return Ok(SmbMode::Unavailable),
            _ => return Err(Error::Unsupported("unexpected Si5338 register 39 value")),
        }

        let val = self.read(28)?;
        if (val & (1 << 5)) != 0 {
            Ok(SmbMode::Input)
        } else {
            Ok(SmbMode::Disabled)
        }
    }

    fn rational_multisynth(
        &mut self,
        index: u8,
        channel: u8,
        rate: &mut RationalRate,
    ) -> Result<RationalRate> {
        let mut ms = Multisynth::default();
        let mut actual = RationalRate::default();
        rate.reduce();
        ms.index = index;
        ms.enable = channel;
        ms.update_base();
        ms.calculate(rate)?;
        ms.calculate_freq(&mut actual);
        self.write_multisynth(&ms)?;
        Ok(RationalRate::new(
            actual.integer(),
            actual.numerator(),
            actual.denominator(),
        ))
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    let mut t: u64;
    while b != 0 {
        t = b;
        b = a % t;
        a = t;
    }
    a
}

impl RationalRate {
    fn reduce(&mut self) {
        if (self.den > 0) && (self.num >= self.den) {
            let whole: u64 = self.num / self.den;
            self.integer += whole;
            self.num -= whole * self.den;
        }
        let val = gcd(self.num, self.den);
        if let (Some(n), Some(d)) = (self.num.checked_div(val), self.den.checked_div(val)) {
            self.num = n;
            self.den = d;
        }
    }

    fn double(&mut self) {
        self.integer *= 2;
        self.num *= 2;
        self.reduce();
    }
}

impl Multisynth {
    fn update_base(&mut self) {
        self.base = 53 + self.index as u16 * 11;
    }

    fn calculate_freq(&mut self, rate: &mut RationalRate) {
        let abc = RationalRate {
            integer: self.a as u64,
            num: self.b as u64,
            den: self.c as u64,
        };
        rate.integer = 0;
        rate.num = SI5338_F_VCO * abc.den;
        rate.den = self.r as u64 * (abc.integer * abc.den + abc.num);
        if self.index == 1 || self.index == 2 {
            rate.den *= 2;
        }
        rate.reduce();
    }

    fn pack_regs(&mut self) -> Result<()> {
        let mut temp = self.a as u64 * self.c as u64 + self.b as u64;
        temp *= 128;
        temp /= self.c as u64;
        temp -= 512;
        if temp > u32::MAX as u64 {
            return Err(Error::InvalidSampleRate("multisynth P1 value out of range"));
        }
        self.p1 = temp as u32;
        temp = self.b as u64 * 128;
        temp %= self.c as u64;
        if temp > u32::MAX as u64 {
            return Err(Error::InvalidSampleRate("multisynth P2 value out of range"));
        }
        self.p2 = temp as u32;
        self.p3 = self.c;
        log::trace!("{:016x} {:016x} {:016x}", self.p1, self.p2, self.p3);
        self.regs[0] = self.p1 as u8;
        self.regs[1] = (self.p1 >> 8) as u8;
        self.regs[2] = (((self.p2 & 0x3f) << 2) | ((self.p1 >> 16) & 0x3)) as u8;
        self.regs[3] = (self.p2 >> 6) as u8;
        self.regs[4] = (self.p2 >> 14) as u8;
        self.regs[5] = (self.p2 >> 22) as u8;
        self.regs[6] = self.p3 as u8;
        self.regs[7] = (self.p3 >> 8) as u8;
        self.regs[8] = (self.p3 >> 16) as u8;
        self.regs[9] = (self.p3 >> 24) as u8;
        Ok(())
    }

    fn unpack_regs(&mut self) -> Result<()> {
        self.p1 = (((self.regs[2] as u32) & 3) << 16)
            | ((self.regs[1] as u32) << 8)
            | (self.regs[0] as u32);
        self.p2 = ((self.regs[5] as u32) << 22)
            | ((self.regs[4] as u32) << 14)
            | ((self.regs[3] as u32) << 6)
            | ((self.regs[2] as u32 >> 2) & 0x3f);
        self.p3 = (((self.regs[9] as u32) & 0x3f) << 24)
            | ((self.regs[8] as u32) << 16)
            | ((self.regs[7] as u32) << 8)
            | (self.regs[6] as u32);
        self.c = self.p3;
        self.a = (self.p1 + 512) / 128;
        let mut temp = (self.p1 as u64 + 512) - 128 * (self.a as u64);
        temp = (temp * self.c as u64) + self.p2 as u64;
        temp = (temp + 64) / 128;
        if temp > u32::MAX as u64 {
            return Err(Error::BoardState(
                "multisynth B value out of range from device",
            ));
        }
        self.b = temp as u32;
        Ok(())
    }

    fn calculate(&mut self, rate: &RationalRate) -> Result<()> {
        let mut req = RationalRate {
            integer: rate.integer,
            num: rate.num,
            den: rate.den,
        };
        if self.index == 1 || self.index == 2 {
            req.double();
        }
        let mut r_value = 1;
        while req.integer < 5_000_000 && r_value < 32 {
            req.double();
            r_value <<= 1;
        }
        if r_value == 32 && req.integer < 5_000_000 {
            return Err(Error::InvalidSampleRate(
                "sample rate too low for SI5338 multisynth",
            ));
        }
        let mut abc = RationalRate {
            integer: 0,
            num: SI5338_F_VCO * req.den,
            den: req.integer * req.den + req.num,
        };
        abc.reduce();
        log::trace!("MSx a + b/c: {} + {}/{}", abc.integer, abc.num, abc.den);
        if abc.integer < 8 && abc.integer != 4 && abc.integer != 6 {
            return Err(Error::InvalidSampleRate(
                "SI5338 multisynth integer part too low",
            ));
        }
        if abc.integer >= 568 {
            return Err(Error::InvalidSampleRate(
                "SI5338 multisynth integer part too high",
            ));
        }
        while abc.num > (1 << 30) || abc.den > (1 << 30) {
            log::debug!(
                "Loss of precision in reducing fraction from {}/{} to {}/{}",
                abc.num,
                abc.den,
                abc.num >> 1,
                abc.den >> 1
            );
            abc.num >>= 1;
            abc.den >>= 1;
        }
        if abc.integer > u32::MAX as u64 || abc.num > u32::MAX as u64 || abc.den > u32::MAX as u64 {
            return Err(Error::InvalidSampleRate(
                "SI5338 multisynth parameters out of u32 range",
            ));
        }
        self.a = abc.integer as u32;
        self.b = abc.num as u32;
        self.c = abc.den as u32;
        self.r = r_value as u32;
        self.pack_regs()?;
        Ok(())
    }
}
