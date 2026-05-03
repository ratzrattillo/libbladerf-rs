use crate::bladerf1::nios_client::NiosClient;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPkt8x8Target;
#[derive(Default, Clone, Copy)]
pub struct RationalRate {
    pub integer: u64,
    pub num: u64,
    pub den: u64,
}
const SI5338_F_VCO: u64 = 38_400_000 * 66;
const SI5338_EN_A: u8 = 0x01;
const SI5338_EN_B: u8 = 0x02;
pub const BLADERF_SAMPLERATE_MIN: u32 = 80_000;
pub const BLADERF_SAMPLERATE_REC_MAX: u32 = 40_000_000;
pub const BLADERF_SMB_FREQUENCY_MAX: u32 = 200_000_000;
pub const BLADERF_SMB_FREQUENCY_MIN: u32 = (38_400_000 * 66) / (32 * 567);
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
pub(crate) fn read(nios: &mut NiosClient, addr: u8) -> Result<u8> {
    nios.nios_read::<u8, u8>(NiosPkt8x8Target::Si5_338, addr)
}
pub(crate) fn write(nios: &mut NiosClient, addr: u8, data: u8) -> Result<()> {
    nios.nios_write::<u8, u8>(NiosPkt8x8Target::Si5_338, addr, data)
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
    let val = gcd(r.num, r.den);
    if let (Some(n), Some(d)) = (r.num.checked_div(val), r.den.checked_div(val)) {
        r.num = n;
        r.den = d;
    }
}
fn rational_double(r: &mut RationalRate) {
    r.integer *= 2;
    r.num *= 2;
    rational_reduce(r);
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
    rational_reduce(rate);
}
fn pack_regs(ms: &mut Multisynth) -> Result<()> {
    let mut temp = ms.a as u64 * ms.c as u64 + ms.b as u64;
    temp *= 128;
    temp /= ms.c as u64;
    temp -= 512;
    if temp > u32::MAX as u64 {
        return Err(Error::InvalidSampleRate("multisynth P1 value out of range"));
    }
    ms.p1 = temp as u32;
    temp = ms.b as u64 * 128;
    temp %= ms.c as u64;
    if temp > u32::MAX as u64 {
        return Err(Error::InvalidSampleRate("multisynth P2 value out of range"));
    }
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
    Ok(())
}
fn unpack_regs(ms: &mut Multisynth) -> Result<()> {
    ms.p1 = (((ms.regs[2] as u32) & 3) << 16) | ((ms.regs[1] as u32) << 8) | (ms.regs[0] as u32);
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
    if temp > u32::MAX as u64 {
        return Err(Error::HardwareState(
            "multisynth B value out of range from device",
        ));
    }
    ms.b = temp as u32;
    Ok(())
}
pub(crate) fn read_multisynth(nios: &mut NiosClient, ms: &mut Multisynth) -> Result<()> {
    let mut val = read(nios, 36 + ms.index)?;
    ms.enable = val & 7;
    log::trace!("Read enable register: {val:x}");
    for i in 0..ms.regs.len() {
        ms.regs[i] = read(nios, ms.base as u8 + i as u8)?
    }
    val = read(nios, 31 + ms.index)?;
    val = (val >> 2) & 7;
    ms.r = 1 << val;
    unpack_regs(ms)?;
    Ok(())
}
pub(crate) fn write_multisynth(nios: &mut NiosClient, ms: &Multisynth) -> Result<()> {
    let mut val = read(nios, 36 + ms.index)?;
    val |= ms.enable;
    log::trace!("Wrote enable register: {val:x}");
    write(nios, 36 + ms.index, val)?;
    for i in 0..ms.regs.len() {
        write(nios, (ms.base + i as u16) as u8, ms.regs[i])?;
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
    write(nios, ms.index + 31, val)
}
pub fn calculate_multisynth(ms: &mut Multisynth, rate: &RationalRate) -> Result<()> {
    let mut req = RationalRate {
        integer: rate.integer,
        num: rate.num,
        den: rate.den,
    };
    if ms.index == 1 || ms.index == 2 {
        rational_double(&mut req);
    }
    let mut r_value = 1;
    while req.integer < 5_000_000 && r_value < 32 {
        rational_double(&mut req);
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
    rational_reduce(&mut abc);
    log::trace!("MSx a + b/c: {} + {}/{}", abc.integer, abc.num, abc.den);
    if abc.integer <= 7 {
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
    ms.a = abc.integer as u32;
    ms.b = abc.num as u32;
    ms.c = abc.den as u32;
    ms.r = r_value as u32;
    pack_regs(ms)?;
    Ok(())
}
pub(crate) fn set_rational_multisynth(
    nios: &mut NiosClient,
    index: u8,
    channel: u8,
    rate: &mut RationalRate,
) -> Result<RationalRate> {
    let mut ms = Multisynth::default();
    let mut actual = RationalRate::default();
    rational_reduce(rate);
    ms.index = index;
    ms.enable = channel;
    update_base(&mut ms);
    calculate_multisynth(&mut ms, rate)?;
    calculate_ms_freq(&mut ms, &mut actual);
    write_multisynth(nios, &ms)?;
    Ok(RationalRate {
        integer: actual.integer,
        num: actual.num,
        den: actual.den,
    })
}
pub fn set_rational_sample_rate(
    nios: &mut NiosClient,
    channel: Channel,
    rate: &mut RationalRate,
) -> Result<RationalRate> {
    let rate_reduced = rate;
    let index: u8 = if channel == Channel::Rx { 0x1 } else { 0x2 };
    let mut si_channel: u8 = SI5338_EN_A;
    rational_reduce(rate_reduced);
    if rate_reduced.integer < BLADERF_SAMPLERATE_MIN as u64 {
        return Err(Error::InvalidSampleRate("sample rate below minimum"));
    }
    if channel == Channel::Tx {
        si_channel |= SI5338_EN_B;
    }
    set_rational_multisynth(nios, index, si_channel, rate_reduced)
}
pub fn set_sample_rate(
    nios: &mut NiosClient,
    channel: Channel,
    rate_requested: u32,
) -> Result<u32> {
    let mut req = RationalRate {
        integer: rate_requested as u64,
        num: 0,
        den: 1,
    };
    log::trace!("Setting integer sample rate: {rate_requested}");
    let act = set_rational_sample_rate(nios, channel, &mut req)?;
    if act.num != 0 {
        log::debug!("Non-integer sample rate set from integer sample rate, truncating output.");
    }
    if act.integer > u32::MAX as u64 {
        return Err(Error::HardwareState("actual sample rate exceeds u32 range"));
    }
    log::trace!("Set actual integer sample rate: {}", act.integer);
    Ok(act.integer as u32)
}
pub fn get_rational_sample_rate(nios: &mut NiosClient, channel: Channel) -> Result<RationalRate> {
    let mut ms = Multisynth {
        index: if channel == Channel::Rx { 1 } else { 2 },
        ..Default::default()
    };
    update_base(&mut ms);
    read_multisynth(nios, &mut ms)?;
    let mut rate = RationalRate::default();
    calculate_ms_freq(&mut ms, &mut rate);
    Ok(rate)
}
pub fn get_sample_rate(nios: &mut NiosClient, channel: Channel) -> Result<u32> {
    let actual = get_rational_sample_rate(nios, channel)?;
    if actual.num != 0 {
        log::debug!("Fractional sample rate truncated during integer sample rate retrieval");
    }
    if actual.integer > u32::MAX as u64 {
        return Err(Error::HardwareState("actual sample rate exceeds u32 range"));
    }
    Ok(actual.integer as u32)
}
pub fn set_rational_smb_freq(nios: &mut NiosClient, rate: RationalRate) -> Result<RationalRate> {
    let mut rate_reduced = rate;
    rational_reduce(&mut rate_reduced);
    if rate_reduced.integer < BLADERF_SMB_FREQUENCY_MIN as u64 {
        log::error!("provided SMB freq violates minimum");
        return Err(Error::Argument("SMB frequency below minimum".into()));
    } else if rate_reduced.integer > BLADERF_SMB_FREQUENCY_MAX as u64 {
        log::error!("provided SMB freq violates maximum");
        return Err(Error::Argument("SMB frequency above maximum".into()));
    }
    set_rational_multisynth(nios, 3, SI5338_EN_A, &mut rate_reduced)
}
pub fn set_smb_freq(nios: &mut NiosClient, rate: u32) -> Result<u32> {
    let mut req = RationalRate::default();
    log::trace!("Setting integer SMB frequency: {rate}");
    req.integer = rate as u64;
    req.num = 0;
    req.den = 1;
    let act = set_rational_smb_freq(nios, req)?;
    if act.num != 0 {
        log::trace!("Non-integer SMB frequency set from integer frequency, truncating output.");
    }
    if act.integer > u32::MAX as u64 {
        return Err(Error::HardwareState(
            "actual SMB frequency exceeds u32 range",
        ));
    }
    log::trace!("Set actual integer SMB frequency: {}", act.integer);
    Ok(act.integer as u32)
}
pub fn get_rational_smb_freq(nios: &mut NiosClient) -> Result<RationalRate> {
    let mut ms = Multisynth::default();
    let mut rate = RationalRate::default();
    ms.index = 3;
    update_base(&mut ms);
    read_multisynth(nios, &mut ms)?;
    calculate_ms_freq(&mut ms, &mut rate);
    Ok(rate)
}
pub fn get_smb_freq(nios: &mut NiosClient) -> Result<u32> {
    let actual = get_rational_smb_freq(nios)?;
    if actual.num != 0 {
        log::trace!("Fractional SMB frequency truncated during integer SMB frequency retrieval");
    }
    if actual.integer > u32::MAX as u64 {
        return Err(Error::HardwareState(
            "actual SMB frequency exceeds u32 range",
        ));
    }
    Ok(actual.integer as u32)
}
