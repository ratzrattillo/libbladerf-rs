use crate::bladerf1::nios_client::NiosInterface;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPkt8x8Target;
use std::sync::{Arc, Mutex};
#[derive(Default, Clone, Copy)]
pub struct RationalRate {
    pub integer: u64,
    pub num: u64,
    pub den: u64,
}
const SI5338_F_VCO: u64 = 38400000 * 66;
const SI5338_EN_A: u8 = 0x01;
const SI5338_EN_B: u8 = 0x02;
pub const BLADERF_SAMPLERATE_MIN: u32 = 80000;
pub const BLADERF_SAMPLERATE_REC_MAX: u32 = 40000000;
pub const BLADERF_SMB_FREQUENCY_MAX: u32 = 200000000;
pub const BLADERF_SMB_FREQUENCY_MIN: u32 = (38400000 * 66) / (32 * 567);
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct Multisynth {
    index: u8,
    base: u16,
    requested: RationalRate,
    actual: RationalRate,
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
#[derive(Clone)]
pub struct SI5338 {
    interface: Arc<Mutex<NiosInterface>>,
}
impl SI5338 {
    pub fn new(interface: Arc<Mutex<NiosInterface>>) -> Self {
        Self { interface }
    }
    pub fn read(&self, addr: u8) -> Result<u8> {
        self.interface
            .lock()
            .unwrap()
            .nios_read::<u8, u8>(NiosPkt8x8Target::Si5338, addr)
    }
    pub fn write(&self, addr: u8, data: u8) -> Result<()> {
        self.interface
            .lock()
            .unwrap()
            .nios_write::<u8, u8>(NiosPkt8x8Target::Si5338, addr, data)
    }
    pub fn update_base(ms: &mut Multisynth) {
        ms.base = 53 + ms.index as u16 * 11;
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
    pub fn rational_reduce(r: &mut RationalRate) {
        if (r.den > 0) && (r.num >= r.den) {
            let whole: u64 = r.num / r.den;
            r.integer += whole;
            r.num -= whole * r.den;
        }
        let val = Self::gcd(r.num, r.den);
        if val > 0 {
            r.num /= val;
            r.den /= val;
        }
    }
    fn rational_double(r: &mut RationalRate) {
        r.integer *= 2;
        r.num *= 2;
        Self::rational_reduce(r);
    }
    pub fn calculate_ms_freq(ms: &mut Multisynth, rate: &mut RationalRate) {
        let abc = RationalRate {
            integer: ms.a as u64,
            num: ms.b as u64,
            den: ms.c as u64,
        };
        rate.integer = 0;
        rate.num = SI5338_F_VCO * abc.den;
        rate.den = ms.r as u64 * (abc.integer * abc.den + abc.num);
        if ms.index == 1 || ms.index == 2 {
            rate.den *= 2;
        }
        Self::rational_reduce(rate);
    }
    pub fn pack_regs(ms: &mut Multisynth) {
        let mut temp = ms.a as u64 * ms.c as u64 + ms.b as u64;
        temp *= 128;
        temp /= ms.c as u64;
        temp -= 512;
        assert!(temp <= u32::MAX as u64);
        ms.p1 = temp as u32;
        temp = ms.b as u64 * 128;
        temp %= ms.c as u64;
        assert!(temp <= u32::MAX as u64);
        ms.p2 = temp as u32;
        ms.p3 = ms.c;
        log::trace!("{:016x} {:016x} {:016x}", ms.p1, ms.p2, ms.p3);
        ms.regs[0] = ms.p1 as u8;
        ms.regs[1] = (ms.p1 >> 8) as u8;
        ms.regs[2] = (((ms.p2 & 0x3f) << 2) | ((ms.p1 >> 16) & 0x3)) as u8;
        ms.regs[3] = (ms.p2 >> 6) as u8;
        ms.regs[4] = (ms.p2 >> 14) as u8;
        ms.regs[5] = (ms.p2 >> 22) as u8;
        ms.regs[6] = ms.p3 as u8;
        ms.regs[7] = (ms.p3 >> 8) as u8;
        ms.regs[8] = (ms.p3 >> 16) as u8;
        ms.regs[9] = (ms.p3 >> 24) as u8;
    }
    pub fn unpack_regs(ms: &mut Multisynth) {
        ms.p1 =
            (((ms.regs[2] as u32) & 3) << 16) | ((ms.regs[1] as u32) << 8) | (ms.regs[0] as u32);
        ms.p2 = ((ms.regs[5] as u32) << 22)
            | ((ms.regs[4] as u32) << 14)
            | ((ms.regs[3] as u32) << 6)
            | ((ms.regs[2] as u32 >> 2) & 0x3f);
        ms.p3 = (((ms.regs[9] as u32) & 0x3f) << 24)
            | ((ms.regs[8] as u32) << 16)
            | ((ms.regs[7] as u32) << 8)
            | (ms.regs[6] as u32);
        ms.c = ms.p3;
        ms.a = (ms.p1 + 512) / 128;
        let mut temp = (ms.p1 as u64 + 512) - 128 * (ms.a as u64);
        temp = (temp * ms.c as u64) + ms.p2 as u64;
        temp = (temp + 64) / 128;
        assert!(temp <= u32::MAX as u64);
        ms.b = temp as u32;
    }
    pub fn read_multisynth(&self, ms: &mut Multisynth) -> Result<()> {
        let mut val = self.read(36 + ms.index)?;
        ms.enable = val & 7;
        log::trace!("Read enable register: {val:x}");
        for i in 0..ms.regs.len() {
            ms.regs[i] = self.read(ms.base as u8 + i as u8)?
        }
        val = self.read(31 + ms.index)?;
        val = (val >> 2) & 7;
        ms.r = 1 << val;
        Self::unpack_regs(ms);
        Ok(())
    }
    pub fn write_multisynth(&self, ms: &Multisynth) -> Result<()> {
        let mut val = self.read(36 + ms.index)?;
        val |= ms.enable;
        log::trace!("Wrote enable register: {val:x}");
        self.write(36 + ms.index, val)?;
        for i in 0..ms.regs.len() {
            self.write((ms.base + i as u16) as u8, ms.regs[i])?;
            log::trace!("Wrote regs[{i}]: {}", ms.regs[i]);
        }
        let mut r_power = 0;
        let mut r_count = ms.r >> 1;
        while r_count > 0 {
            r_count >>= 1;
            r_power += 1;
        }
        val = 0xc0;
        val |= r_power << 2;
        log::trace!("Wrote r register: {val:x}");
        self.write(ms.index + 31, val)
    }
    pub fn calculate_multisynth(ms: &mut Multisynth, rate: &RationalRate) {
        let mut req = RationalRate {
            integer: rate.integer,
            num: rate.num,
            den: rate.den,
        };
        if ms.index == 1 || ms.index == 2 {
            Self::rational_double(&mut req);
        }
        let mut r_value = 1;
        while req.integer < 5000000 && r_value < 32 {
            Self::rational_double(&mut req);
            r_value <<= 1;
        }
        assert!(!(r_value == 32 && req.integer < 5000000));
        let mut abc = RationalRate {
            integer: 0,
            num: SI5338_F_VCO * req.den,
            den: req.integer * req.den + req.num,
        };
        Self::rational_reduce(&mut abc);
        log::trace!("MSx a + b/c: {} + {}/{}", abc.integer, abc.num, abc.den);
        assert!(abc.integer > 7);
        assert!(abc.integer < 568);
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
        log::trace!("MSx a + b/c: {} + {}/{}", abc.integer, abc.num, abc.den);
        assert!(abc.integer <= u32::MAX as u64);
        assert!(abc.num <= u32::MAX as u64);
        assert!(abc.den <= u32::MAX as u64);
        ms.a = abc.integer as u32;
        ms.b = abc.num as u32;
        ms.c = abc.den as u32;
        ms.r = r_value as u32;
        Self::pack_regs(ms);
    }
    pub fn set_rational_multisynth(
        &self,
        index: u8,
        channel: u8,
        rate: &mut RationalRate,
    ) -> Result<RationalRate> {
        let mut ms = Multisynth::default();
        let mut actual = RationalRate::default();
        Self::rational_reduce(rate);
        ms.index = index;
        ms.enable = channel;
        Self::update_base(&mut ms);
        Self::calculate_multisynth(&mut ms, rate);
        Self::calculate_ms_freq(&mut ms, &mut actual);
        self.write_multisynth(&ms)?;
        Ok(RationalRate {
            integer: actual.integer,
            num: actual.num,
            den: actual.den,
        })
    }
    pub fn set_rational_sample_rate(
        &self,
        channel: Channel,
        rate: &mut RationalRate,
    ) -> Result<RationalRate> {
        let rate_reduced = rate;
        let index: u8 = if channel == Channel::Rx { 0x1 } else { 0x2 };
        let mut si_channel: u8 = SI5338_EN_A;
        Self::rational_reduce(rate_reduced);
        assert!(rate_reduced.integer >= BLADERF_SAMPLERATE_MIN as u64);
        if channel == Channel::Tx {
            si_channel |= SI5338_EN_B;
        }
        self.set_rational_multisynth(index, si_channel, rate_reduced)
    }
    pub fn set_sample_rate(&self, channel: Channel, rate_requested: u32) -> Result<u32> {
        let mut req = RationalRate {
            integer: rate_requested as u64,
            num: 0,
            den: 1,
        };
        log::trace!("Setting integer sample rate: {rate_requested}");
        let act = self.set_rational_sample_rate(channel, &mut req)?;
        if act.num != 0 {
            log::debug!("Non-integer sample rate set from integer sample rate, truncating output.");
        }
        assert!(act.integer <= u32::MAX as u64);
        log::trace!("Set actual integer sample rate: {}", act.integer);
        Ok(act.integer as u32)
    }
    pub fn get_rational_sample_rate(&self, channel: Channel) -> Result<RationalRate> {
        let mut ms = Multisynth {
            index: if channel == Channel::Rx { 1 } else { 2 },
            ..Default::default()
        };
        Self::update_base(&mut ms);
        self.read_multisynth(&mut ms)?;
        let mut rate = RationalRate::default();
        Self::calculate_ms_freq(&mut ms, &mut rate);
        Ok(rate)
    }
    pub fn get_sample_rate(&self, channel: Channel) -> Result<u32> {
        let actual = self.get_rational_sample_rate(channel)?;
        if actual.num != 0 {
            log::debug!("Fractional sample rate truncated during integer sample rate retrieval");
        }
        assert!(actual.integer <= u32::MAX as u64);
        Ok(actual.integer as u32)
    }
    pub fn set_rational_smb_freq(&self, rate: RationalRate) -> Result<RationalRate> {
        let mut rate_reduced = rate;
        Self::rational_reduce(&mut rate_reduced);
        if rate_reduced.integer < BLADERF_SMB_FREQUENCY_MIN as u64 {
            log::error!("provided SMB freq violates minimum");
            return Err(Error::Invalid);
        } else if rate_reduced.integer > BLADERF_SMB_FREQUENCY_MAX as u64 {
            log::error!("provided SMB freq violates maximum");
            return Err(Error::Invalid);
        }
        self.set_rational_multisynth(3, SI5338_EN_A, &mut rate_reduced)
    }
    pub fn set_smb_freq(&self, rate: u32) -> Result<u32> {
        let mut req = RationalRate::default();
        log::trace!("Setting integer SMB frequency: {rate}");
        req.integer = rate as u64;
        req.num = 0;
        req.den = 1;
        let act = self.set_rational_smb_freq(req)?;
        if act.num != 0 {
            log::trace!("Non-integer SMB frequency set from integer frequency, truncating output.");
        }
        assert!(act.integer <= u32::MAX as u64);
        log::trace!("Set actual integer SMB frequency: {}", act.integer);
        Ok(act.integer as u32)
    }
    pub fn get_rational_smb_freq(&self) -> Result<RationalRate> {
        let mut ms = Multisynth::default();
        let mut rate = RationalRate::default();
        ms.index = 3;
        Self::update_base(&mut ms);
        self.read_multisynth(&mut ms)?;
        Self::calculate_ms_freq(&mut ms, &mut rate);
        Ok(rate)
    }
    pub fn get_smb_freq(&self) -> Result<u32> {
        let actual = self.get_rational_smb_freq()?;
        if actual.num != 0 {
            log::trace!(
                "Fractional SMB frequency truncated during integer SMB frequency retrieval"
            );
        }
        assert!(actual.integer <= u32::MAX as u64);
        Ok(actual.integer as u32)
    }
}
