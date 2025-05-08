#![allow(dead_code)]
/* This file defines the Host <-> FPGA (NIOS II) packet formats for
 * retune messages. This packet is formatted, as follows. All values are
 * little-endian.
 *
 *                              Request
 *                      ----------------------
 *
 * +================+=========================================================+
 * |  Byte offset   |                       Description                       |
 * +================+=========================================================+
 * |        0       | Magic Value                                             |
 * +----------------+---------------------------------------------------------+
 * |        1       | 64-bit timestamp denoting when to retune. (Note 1)      |
 * +----------------+---------------------------------------------------------+
 * |        9       | 32-bit LMS6002D n_int & n_frac register values (Note 2) |
 * +----------------+---------------------------------------------------------+
 * |       13       | RX/TX bit, FREQSEL LMS6002D reg value  (Note 3)         |
 * +----------------+---------------------------------------------------------+
 * |       14       | Bit 7:        Band-selection (Note 4)                   |
 * |                | Bit 6:        1=Quick tune, 0=Normal tune               |
 * |                | Bits [5:0]    VCOCAP[5:0] Hint                          |
 * +----------------+---------------------------------------------------------+
 * |       15       | 8-bit reserved word. Should be set to 0x00.             |
 * +----------------+---------------------------------------------------------+
 *
 * (Note 1) Special Timestamp Values:
 *
 * Tune "Now":          0x0000000000000000
 * Clear Retune Queue:  0xffffffffffffffff
 *
 * When the "Clear Retune Queue" value is used, all of the other tuning
 * parameters are ignored.
 *
 * (Note 2) Packed as follows:
 *
 * +================+=======================+
 * |   Byte offset  | (MSB)   Value    (LSB)|
 * +================+=======================+
 * |       0        |        NINT[8:1]      |
 * +----------------+-----------------------+
 * |       1        | NINT[0], NFRAC[22:16] |
 * +----------------+-----------------------+
 * |       2        |       NFRAC[15:8]     |
 * +----------------+-----------------------+
 * |       3        |       NFRAC[7:0]      |
 * +----------------+-----------------------+
 *
 * (Note 3) Packed as follows:
 *
 * +================+=======================+
 * |      Bit(s)    |         Value         |
 * +================+=======================+
 * |        7       |          TX           |
 * +----------------+-----------------------+
 * |        6       |          RX           |
 * +----------------+-----------------------+
 * |      [5:0]     |        FREQSEL        |
 * +----------------+-----------------------+
 *
 * (Notes 4) Band-selection bit = 1 implies "Low band". 0 = "High band"
 */
use crate::NiosPktMagic;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use std::fmt::{Debug, Formatter};
//pub const PACK_TXRX_FREQSEL(module_, freqsel_) \ (freqsel_ & 0x3f)

pub struct NiosPktRetuneRequest {
    buf: Vec<u8>,
}
#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Band {
    Low = 0,
    High = 1,
}
#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Tune {
    Normal = 0,
    Quick = 1,
}
impl NiosPktRetuneRequest {
    pub(crate) const IDX_MAGIC: usize = 0;
    pub(crate) const IDX_TIME: usize = 1;
    pub(crate) const IDX_INTFRAC: usize = 9;
    pub(crate) const IDX_FREQSEL: usize = 13;
    pub(crate) const IDX_BANDSEL: usize = 14;
    pub(crate) const IDX_RESV: usize = 15;

    pub(crate) const FLAG_RX: u8 = 1 << 6;
    pub(crate) const FLAG_TX: u8 = 1 << 7;
    pub(crate) const FLAG_QUICK_TUNE: u8 = 1 << 6;
    pub(crate) const FLAG_LOW_BAND: u8 = 1 << 7;

    /* Specify this value instead of a timestamp to clear the retune queue */
    pub const CLEAR_QUEUE: u64 = u64::MAX; // -1

    /* Denotes no tune word is supplied. */
    pub const NO_HINT: u8 = 0xff;

    /* Denotes that the retune should not be scheduled - it should occur "now" */
    pub const RETUNE_NOW: u64 = 0x00;

    /* Maximum field sizes / masks */
    const NINT_MASK: u16 = 0x1ff; // Max 9bit in size
    const NFRAC_MASK: u32 = 0x7fffff; // Max 23bit in size
    const FREQSEL_MASK: u8 = 0x3f; // Max 5bit in size
    const VCOCAP_MASK: u8 = 0x3f; // Max 5bit in size

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: u8,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Self {
        let mut pkt: NiosPktRetuneRequest = vec![0u8; 16].into();
        pkt.set(
            module, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        );
        pkt
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &mut self,
        module: u8,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> &mut Self {
        self.set_magic(NiosPktMagic::Retune as u8)
            .set_timestamp(timestamp)
            .set_nint(nint)
            .set_nfrac(nfrac)
            .set_freqsel(freqsel, module)
            .set_vcocap(vcocap)
            .set_band(band)
            .set_tune(tune)
            .set_xb_gpio(xb_gpio)
    }

    // TODO: Improve validation
    pub fn validate(&self) -> Result<(), String> {
        if self.magic() != NiosPktMagic::Retune as u8 {
            return Err("Invalid magic number")?;
        }
        // if self.reserved() != 0x00 {
        //     return Err("Invalid reserved byte")?;
        // }
        // if self.padding().iter().any(|x| *x != 0) {
        //     return Err("Invalid padding")?;
        // }
        if self.buf.len() != 16 {
            return Err("Invalid length")?;
        }
        Ok(())
    }

    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        self.buf[Self::IDX_MAGIC] = magic;
        self
    }

    pub fn set_timestamp(&mut self, timestamp: u64) -> &mut Self {
        self.buf[Self::IDX_TIME..Self::IDX_TIME + 8]
            .copy_from_slice(timestamp.to_le_bytes().as_slice());
        self
    }

    pub fn set_nint(&mut self, nint: u16) -> &mut Self {
        // Todo: weird case: Decimal number 1019 = 0x3fb is given to this method.
        // Todo: Resulting packet is containing 0x3fb as nint even though according th the packet layout,
        // Todo: this number should not be possible, as it needs 10 bit and not 9bit.
        // assert!(nint <= Self::NINT_MASK);

        // self.buf[Self::IDX_INTFRAC] = 0x00; // Clear out first byte
        // self.buf[Self::IDX_INTFRAC + 1] &= !(0x1 << 7) as u8; // Clear the first bit of second byte

        self.buf[Self::IDX_INTFRAC] = (nint >> 1) as u8; // 1019 >> 1 = 509 = 0x1FD as u8 = 0xFD
        println!("Self::IDX_INTFRAC + 0: {:#x}", self.buf[Self::IDX_INTFRAC]);
        self.buf[Self::IDX_INTFRAC + 1] |= ((nint & 0x1) << 7) as u8;
        println!(
            "Self::IDX_INTFRAC + 1: {:#x}",
            self.buf[Self::IDX_INTFRAC + 1]
        );
        self
    }

    pub fn set_nfrac(&mut self, nfrac: u32) -> &mut Self {
        assert!(nfrac <= Self::NFRAC_MASK);

        // self.buf[Self::IDX_INTFRAC + 1] &= !0x7f; // Clear out all bits except the first bit
        // self.buf[Self::IDX_INTFRAC + 2] = 0x00; // Clear second byte
        // self.buf[Self::IDX_INTFRAC + 3] = 0x00; // Clear third byte

        self.buf[Self::IDX_INTFRAC + 1] |= ((nfrac >> 16) & 0x7f) as u8; // 4893355 >> 16 = 74(0x4A), 0x4A & 0x7f = 0x4A
        println!(
            "Self::IDX_INTFRAC + 1: {:#x}",
            self.buf[Self::IDX_INTFRAC + 1]
        );
        self.buf[Self::IDX_INTFRAC + 2] = (nfrac >> 8) as u8; // 4893355 >> 8 = 19114(0x4AAA), as u8 = 0xAA
        println!(
            "Self::IDX_INTFRAC + 2: {:#x}",
            self.buf[Self::IDX_INTFRAC + 2]
        );
        self.buf[Self::IDX_INTFRAC + 3] = nfrac as u8; // 4893355 as u8 = 0xAB
        println!(
            "Self::IDX_INTFRAC + 3: {:#x}",
            self.buf[Self::IDX_INTFRAC + 3]
        );
        self
    }

    fn set_rx_flag(&mut self, flag: bool) -> &mut Self {
        if flag {
            self.buf[Self::IDX_FREQSEL] |= Self::FLAG_RX;
        } else {
            self.buf[Self::IDX_FREQSEL] &= !Self::FLAG_RX;
        }
        self
    }

    fn set_tx_flag(&mut self, flag: bool) -> &mut Self {
        if flag {
            self.buf[Self::IDX_FREQSEL] |= Self::FLAG_TX;
        } else {
            self.buf[Self::IDX_FREQSEL] &= !Self::FLAG_TX;
        }

        self
    }

    pub fn set_freqsel(&mut self, freqsel: u8, module: u8) -> &mut Self {
        assert!(freqsel <= Self::FREQSEL_MASK); // Make sure that freqsel does not consume more than 5 bits.
        self.buf[Self::IDX_FREQSEL] = freqsel;
        match module {
            BLADERF_MODULE_RX => {
                self.set_rx_flag(true);
            }
            BLADERF_MODULE_TX => {
                self.set_tx_flag(true);
            }
            _ => {
                panic!("invalid module")
            }
        }
        self
    }

    pub fn set_band(&mut self, band: Band) -> &mut Self {
        match band {
            Band::Low => {
                self.buf[Self::IDX_BANDSEL] |= Self::FLAG_LOW_BAND; // Set LowBand Flag
            }
            Band::High => {
                self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_LOW_BAND; // Clear LowBand Flag
            }
        }
        self
    }

    pub fn set_tune(&mut self, tune: Tune) -> &mut Self {
        match tune {
            Tune::Quick => {
                self.buf[Self::IDX_BANDSEL] |= Self::FLAG_QUICK_TUNE; // Set QuickTune Flag
            }
            Tune::Normal => {
                self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_QUICK_TUNE; // Clear QuickTune Flag
            }
        }
        self
    }

    pub fn set_vcocap(&mut self, vcocap: u8) -> &mut Self {
        assert!(vcocap <= Self::VCOCAP_MASK); // Make sure that vcocap does not consume more than 5 bits.
        self.buf[Self::IDX_BANDSEL] &= !Self::VCOCAP_MASK; // Clear bits 0:5
        self.buf[Self::IDX_BANDSEL] |= vcocap & Self::VCOCAP_MASK; // Set vcocap value and limit it to the allowed 5 bits
        self
    }

    pub fn set_xb_gpio(&mut self, xb_gpio: u8) -> &mut Self {
        self.buf[Self::IDX_RESV] = xb_gpio;
        self
    }

    pub fn buf_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }
    pub fn magic(&self) -> u8 {
        self.buf[Self::IDX_MAGIC]
    }

    pub fn timestamp(&self) -> u64 {
        let pkt_mem = &self.buf[Self::IDX_TIME..Self::IDX_TIME + 8];
        u64::from_le_bytes(pkt_mem.try_into().unwrap())
    }

    pub fn nint(&self) -> u16 {
        let mut nint = (self.buf[Self::IDX_INTFRAC] as u16) << 1;
        nint |= (self.buf[Self::IDX_INTFRAC + 1] as u16) >> 7;
        println!("nint: {nint:#x?}");
        nint
    }

    pub fn nfrac(&self) -> u32 {
        let mut nfrac: u32 = ((&self.buf[Self::IDX_INTFRAC + 1] & 0x7f) as u32) << 16;
        nfrac |= (self.buf[Self::IDX_INTFRAC + 2] as u32) << 8;
        nfrac |= self.buf[Self::IDX_INTFRAC + 3] as u32;
        nfrac
    }

    pub fn freqsel(&self) -> u8 {
        self.buf[Self::IDX_FREQSEL] & Self::FREQSEL_MASK
    }

    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_BANDSEL] & Self::VCOCAP_MASK
    }

    pub fn rx_flag(&self) -> u8 {
        self.buf[Self::IDX_FREQSEL] & Self::FLAG_RX
    }
    pub fn tx_flag(&self) -> u8 {
        self.buf[Self::IDX_FREQSEL] & Self::FLAG_TX
    }

    pub fn tune(&self) -> Tune {
        if self.buf[Self::IDX_BANDSEL] & Self::FLAG_QUICK_TUNE == 0 {
            Tune::Normal
        } else {
            Tune::Quick
        }
    }

    pub fn band(&self) -> Band {
        if self.buf[Self::IDX_BANDSEL] & Self::FLAG_LOW_BAND == 0 {
            Band::High
        } else {
            Band::Low
        }
    }

    pub fn xb_gpio(&self) -> u8 {
        self.buf[Self::IDX_RESV]
    }

    pub fn module(&self) -> u8 {
        if self.rx_flag() != 0 {
            BLADERF_MODULE_RX
        } else if self.tx_flag() != 0 {
            BLADERF_MODULE_TX
        } else {
            u8::MAX
        }
    }
}

impl From<Vec<u8>> for NiosPktRetuneRequest {
    fn from(value: Vec<u8>) -> Self {
        Self { buf: value }
    }
}

impl From<NiosPktRetuneRequest> for Vec<u8> {
    fn from(value: NiosPktRetuneRequest) -> Self {
        value.buf
    }
}

impl Debug for NiosPktRetuneRequest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let band = match self.band() {
            Band::Low => "Band::Low",
            Band::High => "Band::High",
        };
        let tune = match self.tune() {
            Tune::Normal => "Tune::Normal",
            Tune::Quick => "Tune::Quick",
        };
        f.debug_struct("NiosPktRetuneRequest")
            .field("magic", &format_args!("{:#x}", self.magic()))
            .field("timestamp", &format_args!("{:#x}", self.timestamp()))
            .field("nint", &format_args!("{:#x}", self.nint()))
            .field("nfrac", &format_args!("{:#x}", self.nfrac()))
            .field("freqsel", &format_args!("{:#x}", self.freqsel()))
            .field("vcocap", &format_args!("{:#x}", self.vcocap()))
            .field("band", &String::from(band))
            .field("tune", &String::from(tune))
            .field("xb_gpio", &format_args!("{:#x}", self.xb_gpio()))
            .finish()
    }
}

/*
 *                             Response
 *                      ----------------------
 *
 * +================+=========================================================+
 * |  Byte offset   |                       Description                       |
 * +================+=========================================================+
 * |        0       | Magic Value                                             |
 * +----------------+---------------------------------------------------------+
 * |        1       | 64-bit duration denoting how long the operation took to |
 * |                | complete, in units of timestamp ticks. (Note 1)         |
 * +----------------+---------------------------------------------------------+
 * |        9       | Bits [7:6]    Reserved, set to 0.                       |
 * |                | Bits [5:0]    VCOCAP value used. (Note 2)               |
 * +----------------+---------------------------------------------------------+
 * |       10       | Status Flags (Note 3)                                   |
 * +----------------+---------------------------------------------------------+
 * |      11-15     | Reserved. All bits set to 0.                            |
 * +----------------+---------------------------------------------------------+
 *
 * (Note 1) This value will be zero if timestamps are not running for the
 *          associated module.
 *
 * (Note 2) This field's value should be ignored when reading a response for
 *          a request to clear the retune queue.
 *
 * (Note 3) Description of Status Flags:
 *
 *      flags[0]: 1 = Timestamp and VCOCAP are valid. This is only the case for
 *                    "Tune NOW" requests. It is not possible to return this
 *                    information for scheduled retunes, as the event generally
 *                    does not occur before the response is set.
 *
 *                0 = This was a scheduled retune. Timestamp and VCOCAP Fields
 *                    should be ignored.
 *
 *
 *      flags[1]: 1 = Operation completed successfully.
 *                0 = Operation failed.
 *
 *                For "Tune NOW" requests, a failure may occur as the result
 *                of the tuning algorithm failing to occur, and such other
 *                unexpected failurs.
 *
 *                The scheduled tune request will failure if the retune queue
 *                is full.
 *
 *      flags[7:2]    Reserved. Set to 0.
 */

pub struct NiosPktRetuneResponse {
    buf: Vec<u8>,
}
impl NiosPktRetuneResponse {
    const IDX_MAGIC: usize = 0;
    const IDX_TIME: usize = 1;
    const IDX_VCOCAP: usize = 9;
    const IDX_FLAGS: usize = 10;
    // const IDX_RESV: usize = 11;

    const VCOCAP_MASK: u8 = 0x3f; // Max 5bit in size
    const FLAG_DURATION_VCOCAP_VALID: u8 = 0x1;
    const FLAG_SUCCESS: u8 = 0x2;

    pub fn magic(&self) -> u8 {
        self.buf[Self::IDX_MAGIC]
    }

    pub fn duration(&self) -> u64 {
        let pkt_mem = &self.buf[Self::IDX_TIME..Self::IDX_TIME + 8];
        u64::from_le_bytes(pkt_mem.try_into().unwrap())
    }

    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_VCOCAP] & Self::VCOCAP_MASK
    }

    pub fn duration_and_vcocap_valid(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_DURATION_VCOCAP_VALID != 0
    }

    pub fn is_success(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_SUCCESS != 0
    }
}

impl From<Vec<u8>> for NiosPktRetuneResponse {
    fn from(value: Vec<u8>) -> Self {
        Self { buf: value }
    }
}

impl From<NiosPktRetuneResponse> for Vec<u8> {
    fn from(value: NiosPktRetuneResponse) -> Self {
        value.buf
    }
}

impl Debug for NiosPktRetuneResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiosPktRetuneRequest")
            .field("magic", &format_args!("{:#x}", self.magic()))
            .field("duration", &format_args!("{:#x}", self.duration()))
            .field("vcocap", &format_args!("{:#x}", self.vcocap()))
            .field(
                "duration_and_vcocap_valid",
                &format_args!("{}", self.duration_and_vcocap_valid()),
            )
            .field("is_success", &format_args!("{}", self.is_success()))
            .finish()
    }
}
