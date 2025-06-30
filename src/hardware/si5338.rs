#![allow(dead_code)]

use crate::nios::Nios;
use anyhow::anyhow;
use bladerf_globals::bladerf1::{
    BLADERF_SAMPLERATE_MIN, BLADERF_SMB_FREQUENCY_MAX, BLADERF_SMB_FREQUENCY_MIN,
};
use bladerf_globals::{
    BladerfRationalRate, ENDPOINT_IN, ENDPOINT_OUT, bladerf_channel_rx, bladerf_channel_tx,
};
use bladerf_nios::NIOS_PKT_8X8_TARGET_SI5338;
use bladerf_nios::packet_generic::{NiosReq8x8, NiosResp8x8};
use nusb::Interface;

const SI5338_F_VCO: u64 = 38400000 * 66;
const SI5338_EN_A: u8 = 0x01;
const SI5338_EN_B: u8 = 0x02;

/**
 * This is used set or recreate the si5338 frequency
 * Each si5338 multisynth module can be set independently
 */
#[derive(Clone, Default)]
pub struct Si5338Multisynth {
    /* Multisynth to program (0-3) */
    index: u8,

    /* Base address of the multisynth */
    base: u16,

    /* Requested and actual sample rates */
    requested: BladerfRationalRate,
    actual: BladerfRationalRate,

    /* Enables for A and/or B outputs */
    enable: u8,

    /* f_out = fvco / (a + b/c) / r */
    a: u32,
    b: u32,
    c: u32,
    r: u32,

    /* (a, b, c) in multisynth (p1, p2, p3) form */
    p1: u32,
    p2: u32,
    p3: u32,

    /* (p1, p2, p3) in register form */
    regs: [u8; 10],
}

pub struct SI5338 {
    interface: Interface,
}

impl SI5338 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub async fn read(&self, addr: u8) -> anyhow::Result<u8> {
        type ReqType = NiosReq8x8;

        let request = ReqType::new(NIOS_PKT_8X8_TARGET_SI5338, ReqType::FLAG_READ, addr, 0x0);

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosResp8x8::from(response).data())
    }

    pub async fn write(&self, addr: u8, data: u8) -> anyhow::Result<u8> {
        type ReqType = NiosReq8x8;

        let request = ReqType::new(NIOS_PKT_8X8_TARGET_SI5338, ReqType::FLAG_WRITE, addr, data);

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosResp8x8::from(response).data())
    }

    /**
     * Update the base address of the selected multisynth
     */
    pub fn update_base(ms: &mut Si5338Multisynth) {
        ms.base = 53 + ms.index as u16 * 11;
    }

    // TODO: Use normal gcd from gcd crate
    fn gcd(mut a: u64, mut b: u64) -> u64 {
        let mut t: u64;
        while b != 0 {
            t = b;
            b = a % t;
            a = t;
        }
        a
    }

    pub fn rational_reduce(r: &mut BladerfRationalRate) {
        if (r.den > 0) && (r.num >= r.den) {
            /* Get whole number */
            let whole: u64 = r.num / r.den;
            r.integer += whole;
            r.num -= whole * r.den;
        }

        /* Reduce fraction */
        let val = Self::gcd(r.num, r.den);
        if val > 0 {
            r.num /= val;
            r.den /= val;
        }
    }

    pub fn rational_double(r: &mut BladerfRationalRate) {
        r.integer *= 2;
        r.num *= 2;
        Self::rational_reduce(r);
    }

    pub fn calculate_ms_freq(ms: &mut Si5338Multisynth, rate: &mut BladerfRationalRate) {
        let abc = BladerfRationalRate {
            integer: ms.a as u64,
            num: ms.b as u64,
            den: ms.c as u64,
        };

        rate.integer = 0;
        rate.num = SI5338_F_VCO * abc.den;
        rate.den = ms.r as u64 * (abc.integer * abc.den + abc.num);

        /* Compensate for doubling of frequency for LMS sampling clocks */
        if ms.index == 1 || ms.index == 2 {
            rate.den *= 2;
        }

        Self::rational_reduce(rate);
    }

    /*
     * Pack (a, b, c, r) into (p1, p2, p3) and regs[]
     */
    pub fn pack_regs(ms: &mut Si5338Multisynth) {
        /* Precondition:
         *  (a, b, c) and r have been populated
         *
         * Post-condition:
         *  (p1, p2, p3) and regs[10] are populated
         */

        /* p1 = (a * c + b) * 128 / c - 512 */
        let mut temp = ms.a as u64 * ms.c as u64 + ms.b as u64;
        temp *= 128;
        temp /= ms.c as u64;
        temp -= 512;
        assert!(temp <= u32::MAX as u64);
        ms.p1 = temp as u32;

        /* p2 = (b * 128) % c */
        temp = ms.b as u64 * 128;
        temp %= ms.c as u64;
        assert!(temp <= u32::MAX as u64);
        ms.p2 = temp as u32;

        /* p3 = c */
        ms.p3 = ms.c;

        // log_verbose("MSx P1: 0x%8.8x (%u) P2: 0x%8.8x (%u) P3: 0x%8.8x (%u)\n", ms.p1, ms.p1, ms.p2, ms.p2, ms.p3, ms.p3);
        println!("{:016x} {:016x} {:016x}", ms.p1, ms.p2, ms.p3);

        /* Regs */
        /* Could probably be also expressed with to_le_bytes() or unsafe copy operations */
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

    /// Unpack the recently read registers into (p1, p2, p3) and (a, b, c)
    ///
    /// Precondition:
    ///  regs\[10\] and r have been read
    ///
    /// Post-condition:
    ///  (p1, p2, p3), (a, b, c) and actual are populated
    pub fn unpack_regs(ms: &mut Si5338Multisynth) {
        // let mut temp: u64 = 0;

        /* Zeroize */
        ms.p1 = 0;
        ms.p2 = 0;
        ms.p3 = 0;

        /* Populate */
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

        // log_verbose("Unpacked P1: 0x%8.8x (%u) P2: 0x%8.8x (%u) P3: 0x%8.8x (%u)\n", ms->p1, ms->p1, ms->p2, ms->p2, ms->p3, ms->p3);

        /* c = p3 */
        ms.c = ms.p3;

        /* a =  (p1+512)/128
         *
         * NOTE: The +64 is for rounding purposes.
         */
        ms.a = (ms.p1 + 512) / 128;

        /* b = (((p1+512)-128*a)*c + (b % c) + 64)/128 */
        let mut temp = (ms.p1 as u64 + 512) - 128 * (ms.a as u64);
        temp = (temp * ms.c as u64) + ms.p2 as u64;
        temp = (temp + 64) / 128;
        assert!(temp <= u32::MAX as u64);
        ms.b = temp as u32;

        //log_verbose("Unpacked a + b/c: %d + %d/%d\n", ms->a, ms->b, ms->c);
        //log_verbose("Unpacked r: %d\n", ms->r);
    }

    pub async fn read_multisynth(&self, ms: &mut Si5338Multisynth) -> anyhow::Result<()> {
        /* Read the enable bits */
        let mut val = self.read(36 + ms.index).await?;

        ms.enable = val & 7;
        // log_verbose("Read enable register: 0x%2.2x\n", val);

        /* Read all the multisynth registers */
        for i in 0..ms.regs.len() {
            ms.regs[i] = self.read(ms.base as u8 + i as u8).await?
        }

        /* Populate the RxDIV value from the register */
        val = self.read(31 + ms.index).await?;

        /* RxDIV is stored as a power of 2, so restore it on readback */
        // log_verbose("Read r register: 0x%2.2x\n", val);
        val = (val >> 2) & 7;
        ms.r = 1 << val;

        /* Unpack the regs into appropriate values */
        Self::unpack_regs(ms);

        Ok(())
    }

    pub async fn write_multisynth(&self, ms: &Si5338Multisynth) -> anyhow::Result<u8> {
        let mut val = self.read(36 + ms.index).await?;
        val |= ms.enable;
        println!("Wrote enable register: {val:x}");
        self.write(36 + ms.index, val).await?;

        /* Write out the registers */
        for i in 0..ms.regs.len() {
            self.write((ms.base + i as u16) as u8, ms.regs[i]).await?;
            println!("Wrote regs[{i}]: {}", ms.regs[i]);
        }

        /* Calculate r_power from c_count */
        let mut r_power = 0;
        let mut r_count = ms.r >> 1;
        while r_count > 0 {
            r_count >>= 1;
            r_power += 1;
        }

        /* Set the r value to the log2(r_count) to match Figure 18 */
        val = 0xc0;
        val |= r_power << 2;

        println!("Wrote r register: {val:x}");

        self.write(ms.index + 31, val).await
    }

    pub fn calculate_multisynth(ms: &mut Si5338Multisynth, rate: &BladerfRationalRate) {
        let mut r_value: u8;

        /* Don't mess with the users data */
        let mut req = BladerfRationalRate {
            integer: rate.integer,
            num: rate.num,
            den: rate.den,
        };

        /* Double requested frequency for sample clocks since LMS requires
         * 2:1 clock:sample rate
         */
        if ms.index == 1 || ms.index == 2 {
            Self::rational_double(&mut req);
        }

        /* Find a suitable R value */
        r_value = 1;
        while req.integer < 5000000 && r_value < 32 {
            Self::rational_double(&mut req);
            r_value <<= 1;
        }

        assert!(!(r_value == 32 && req.integer < 5000000));

        /* Find suitable MS (a, b, c) values */
        let mut abc = BladerfRationalRate {
            integer: 0,
            num: SI5338_F_VCO * req.den,
            den: req.integer * req.den + req.num,
        };
        Self::rational_reduce(&mut abc);

        // log_verbose("MSx a + b/c: %"PRIu64" + %"PRIu64"/%"PRIu64"\n", abc.integer, abc.num, abc.den);

        /* Check values to make sure they are OK */
        assert!(abc.integer > 7);
        assert!(abc.integer < 568);
        // if abc.integer < 8 {
        //     //log_debug("Integer portion too small: %"PRIu64"\n", abc.integer);
        //     return BLADERF_ERR_INVAL;
        // } else if abc.integer > 567 {
        //     // log_debug("Integer portion too large: %"PRIu64"\n", abc.integer);
        //     return BLADERF_ERR_INVAL;
        // }

        /* Loss of precision if num or den are greater than 2^30-1 */
        while abc.num > (1 << 30) || abc.den > (1 << 30) {
            // log_debug("Loss of precision in reducing fraction from %"PRIu64"/%"PRIu64" to %"PRIu64"/%"PRIu64"\n", abc.num, abc.den, abc.num>>1, abc.den>>1);
            abc.num >>= 1;
            abc.den >>= 1;
        }

        // log_verbose("MSx a + b/c: %"PRIu64" + %"PRIu64"/%"PRIu64"\n", abc.integer, abc.num, abc.den);

        /* Set it in the multisynth */
        assert!(abc.integer <= u32::MAX as u64);
        assert!(abc.num <= u32::MAX as u64);
        assert!(abc.den <= u32::MAX as u64);
        ms.a = abc.integer as u32;
        ms.b = abc.num as u32;
        ms.c = abc.den as u32;
        ms.r = r_value as u32;

        /* Pack the registers */
        Self::pack_regs(ms);
    }

    pub async fn set_rational_multisynth(
        &self,
        index: u8,
        channel: u8,
        mut rate: BladerfRationalRate,
    ) -> anyhow::Result<BladerfRationalRate> {
        let mut ms = Si5338Multisynth::default();
        // let mut req = BladerfRationalRate::default();
        let mut actual = BladerfRationalRate::default();

        Self::rational_reduce(&mut rate);

        /* Save off the value */
        // let mut req = rate.clone();

        /* Set up the multisynth enables and index */
        ms.index = index;
        ms.enable = channel;

        /* Update the base address register */
        Self::update_base(&mut ms);

        /* Calculate multisynth values */
        Self::calculate_multisynth(&mut ms, &rate);

        /* Get the actual rate */
        Self::calculate_ms_freq(&mut ms, &mut actual);

        /* Program it to the part */
        self.write_multisynth(&ms).await?;
        Ok(BladerfRationalRate {
            integer: actual.integer,
            num: actual.num,
            den: actual.den,
        })
    }
    pub async fn set_rational_sample_rate(
        &self,
        ch: u8,
        rate: &mut BladerfRationalRate,
    ) -> anyhow::Result<BladerfRationalRate> {
        let mut rate_reduced = rate.clone();
        let index: u8 = if ch == bladerf_channel_rx!(0) {
            0x1
        } else {
            0x2
        };
        let mut channel: u8 = SI5338_EN_A;

        /* Enforce minimum sample rate */
        Self::rational_reduce(&mut rate_reduced);
        assert!(rate_reduced.integer >= BLADERF_SAMPLERATE_MIN as u64);

        if ch == bladerf_channel_tx!(0) {
            channel |= SI5338_EN_B;
        }

        self.set_rational_multisynth(index, channel, rate_reduced)
            .await
    }

    pub async fn set_sample_rate(&self, channel: u8, rate_requested: u32) -> anyhow::Result<u32> {
        let mut req = BladerfRationalRate {
            integer: rate_requested as u64,
            num: 0,
            den: 1,
        };
        // let mut act = BladerfRationalRate::default();

        // log_verbose("Setting integer sample rate: %d\n", rate);

        let act = self.set_rational_sample_rate(channel, &mut req).await?;

        if act.num != 0 {
            println!("Non-integer sample rate set from integer sample rate, truncating output.\n");
        }

        assert!(act.integer <= u32::MAX as u64);

        Ok(act.integer as u32)
        //log_verbose("Set actual integer sample rate: %d\n", act.integer);
    }

    pub async fn get_rational_sample_rate(
        &self,
        channel: u8,
    ) -> anyhow::Result<BladerfRationalRate> {
        /* Select the multisynth we want to read */
        let mut ms = Si5338Multisynth {
            index: if channel == bladerf_channel_rx!(0) {
                1
            } else {
                2
            },
            ..Default::default()
        };

        /* Update the base address */
        Self::update_base(&mut ms);

        /* Readback */
        self.read_multisynth(&mut ms).await?;

        let mut rate = BladerfRationalRate::default();
        Self::calculate_ms_freq(&mut ms, &mut rate);
        Ok(rate)
    }

    pub async fn get_sample_rate(&self, channel: u8) -> anyhow::Result<u32> {
        let actual = self.get_rational_sample_rate(channel).await?;

        if actual.num != 0 {
            println!("Fractional sample rate truncated during integer sample rate retrieval");
        }

        assert!(actual.integer <= u32::MAX as u64);
        Ok(actual.integer as u32)
    }

    pub async fn set_rational_smb_freq(
        &self,
        rate: &BladerfRationalRate,
    ) -> anyhow::Result<BladerfRationalRate> {
        let mut rate_reduced = rate.clone();

        /* Enforce minimum and maximum frequencies */
        Self::rational_reduce(&mut rate_reduced);

        if rate_reduced.integer < BLADERF_SMB_FREQUENCY_MIN as u64 {
            return Err(anyhow!("provided SMB freq violates minimum"));
        } else if rate_reduced.integer > BLADERF_SMB_FREQUENCY_MAX as u64 {
            return Err(anyhow!("provided SMB freq violates maximum"));
        }

        self.set_rational_multisynth(3, SI5338_EN_A, rate_reduced)
            .await
    }

    pub async fn set_smb_freq(&self, rate: u32) -> anyhow::Result<u32> {
        let mut req = BladerfRationalRate::default();
        println!("Setting integer SMB frequency: {rate}");
        req.integer = rate as u64;
        req.num = 0;
        req.den = 1;

        let act = self.set_rational_smb_freq(&req).await?;

        if act.num != 0 {
            println!("Non-integer SMB frequency set from integer frequency, truncating output.");
        }

        assert!(act.integer <= u32::MAX as u64);

        println!("Set actual integer SMB frequency: {}", act.integer);

        Ok(act.integer as u32)
    }

    pub async fn get_rational_smb_freq(&self) -> anyhow::Result<BladerfRationalRate> {
        let mut ms = Si5338Multisynth::default();
        let mut rate = BladerfRationalRate::default();

        /* Select MS3 for the SMB output */
        ms.index = 3;
        Self::update_base(&mut ms);

        self.read_multisynth(&mut ms).await?;

        Self::calculate_ms_freq(&mut ms, &mut rate);

        Ok(rate)
    }

    pub async fn get_smb_freq(&self) -> anyhow::Result<u32> {
        let actual = self.get_rational_smb_freq().await?;

        if actual.num != 0 {
            println!("Fractional SMB frequency truncated during integer SMB frequency retrieval");
        }

        assert!(actual.integer <= u32::MAX as u64);

        Ok(actual.integer as u32)
    }
}
