use crate::NiosPktMagic;
use crate::packet_base::GenericNiosPkt;
use bladerf_globals::BladeRfDirection;
use bladerf_globals::bladerf_channel_is_tx;
use std::fmt::{Debug, Formatter};

struct NiosPktRetune2 {
    pub buf: Vec<u8>,
}

impl NiosPktRetune2 {
    const IDX_NIOS_PROFILE: usize = 9;
    const IDX_RFFE_PROFILE: usize = 11;
    const IDX_RFFE_PORT: usize = 12;
    const IDX_SPDT: usize = 13;
    // const IDX_RESERVED: usize = 14;

    // Specify this value instead of a timestamp to clear the retune2 queue
    // const CLEAR_QUEUE: u64 = u64::MAX;

    // Denotes that the retune2 should not be scheduled - it should occur "now"
    // const NOW: u64 = u64::MIN;

    // The IS_RX bit embedded in the 'port' parameter of the retune2 packet
    const MASK_PORT_IS_RX: u8 = 1 << 7;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: u8,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> Self {
        let mut pkt: NiosPktRetune2 = vec![0u8; 16].into();
        pkt.set(module, timestamp, nios_profile, rffe_profile, port, spdt);
        pkt
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &mut self,
        module: u8,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> &mut Self {
        self.set_magic(NiosPktMagic::Retune2 as u8)
            .set_timestamp(timestamp)
            .set_nios_profile(nios_profile)
            .set_rffe_profile(rffe_profile)
            .set_port(port, module)
            .set_spdt(spdt)
    }

    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        self.buf.set_magic(magic);
        self
    }

    pub fn set_timestamp(&mut self, timestamp: u64) -> &mut Self {
        self.buf.set_duration_or_timestamp(timestamp);
        self
    }

    pub fn set_nios_profile(&mut self, nios_profile: u16) -> &mut Self {
        self.buf[Self::IDX_NIOS_PROFILE] = nios_profile as u8;
        self.buf[Self::IDX_NIOS_PROFILE + 1] = (nios_profile >> 8) as u8;
        self
    }

    pub fn set_rffe_profile(&mut self, rffe_profile: u8) -> &mut Self {
        self.buf[Self::IDX_RFFE_PROFILE] = rffe_profile;
        self
    }

    pub fn set_port(&mut self, port: u8, module: u8) -> &mut Self {
        // Clear the IS_RX bit of the port parameter
        let mut pkt_port = port & !Self::MASK_PORT_IS_RX;

        // Set the IS_RX bit (if needed)
        pkt_port |= if bladerf_channel_is_tx!(module) {
            0x0
        } else {
            Self::MASK_PORT_IS_RX
        };

        self.buf[Self::IDX_RFFE_PORT] = pkt_port;
        self
    }

    pub fn set_spdt(&mut self, spdt: u8) -> &mut Self {
        self.buf[Self::IDX_SPDT] = spdt;
        self
    }

    pub fn magic(&self) -> u8 {
        self.buf.magic()
    }

    pub fn timestamp(&self) -> u64 {
        self.buf.duration_or_timestamp()
    }

    pub fn nios_profile(&self) -> u16 {
        let pkt_mem = &self.buf[Self::IDX_NIOS_PROFILE..Self::IDX_NIOS_PROFILE + 2];
        u16::from_le_bytes(pkt_mem.try_into().unwrap())
    }

    pub fn rffe_profile(&self) -> u8 {
        self.buf[Self::IDX_RFFE_PROFILE]
    }

    pub fn port(&self) -> u8 {
        self.buf[Self::IDX_RFFE_PORT]
    }

    pub fn spdt(&self) -> u8 {
        self.buf[Self::IDX_SPDT]
    }

    // use crate::ValidationError;
    // pub fn validate(&self) -> Result<(), ValidationError> {
    //     if self.magic() != NiosPktMagic::Retune2 as u8 {
    //         return Err(ValidationError::InvalidMagic(self.magic()));
    //     }
    //     if self.buf.len() != 16 {
    //         return Err(ValidationError::InvalidLength(self.buf.len()));
    //     }
    //     Ok(())
    // }
}

impl From<Vec<u8>> for NiosPktRetune2 {
    fn from(value: Vec<u8>) -> Self {
        Self { buf: value }
    }
}

impl From<NiosPktRetune2> for Vec<u8> {
    fn from(value: NiosPktRetune2) -> Self {
        value.buf
    }
}

/// This file defines the Host <-> FPGA (NIOS II) packet formats for
/// retune2 messages. This packet is formatted, as follows. All values are
/// little-endian.
///
///                              Request
///                      ----------------------
///
/// +================+=========================================================+
/// |  Byte offset   |                       Description                       |
/// +================+=========================================================+
/// |        0       | Magic Value                                             |
/// +----------------+---------------------------------------------------------+
/// |        1       | 64-bit timestamp denoting when to retune. (Note 1)      |
/// +----------------+---------------------------------------------------------+
/// |        9       | 16-bit Nios fast lock profile number to load (Note 2)   |
/// +----------------+---------------------------------------------------------+
/// |       11       | 8-bit RFFE fast lock profile slot to use                |
/// +----------------+---------------------------------------------------------+
/// |       12       | Bit  7:     RX bit (set if this is an RX profile        |
/// |                | Bits 6:     TX output port selection                    |
/// |                | Bits \[5:0\]: RX input port selection                     |
/// +----------------+---------------------------------------------------------+
/// |       13       | Bits \[7:6\]: External TX2 SPDT switch setting            |
/// |                | Bits \[5:4\]: External TX1 SPDT switch setting            |
/// |                | Bits \[3:2\]: External RX2 SPDT switch setting            |
/// |                | Bits \[1:0\]: External RX1 SPDT switch setting            |
/// +----------------+---------------------------------------------------------+
/// |       14-15    | 8-bit reserved words. Should be set to 0x00.            |
/// +----------------+---------------------------------------------------------+
///
/// (Note 1) Special Timestamp Values:
///
/// Tune "Now":          0x0000000000000000
/// Clear Retune Queue:  0xffffffffffffffff
///
/// When the "Clear Retune Queue" value is used, all of the other tuning
/// parameters are ignored.
///
/// (Note 2) Packed as follows:
///
/// +================+=======================+
/// |   Byte offset  | (MSB)   Value    (LSB)|
/// +================+=======================+
/// |       0        |  NIOS_PROFILE\[7:0\]    |
/// +----------------+-----------------------+
/// |       1        |  NIOS_PROFILE\[15:8\]   |
/// +----------------+-----------------------+
pub struct NiosPktRetune2Request {
    pkt: NiosPktRetune2,
}

impl NiosPktRetune2Request {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: u8,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> Self {
        Self {
            pkt: NiosPktRetune2::new(module, timestamp, nios_profile, rffe_profile, port, spdt),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &mut self,
        module: u8,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> &mut Self {
        self.pkt
            .set(module, timestamp, nios_profile, rffe_profile, port, spdt);
        self
    }

    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        self.pkt.set_magic(magic);
        self
    }

    pub fn set_timestamp(&mut self, timestamp: u64) -> &mut Self {
        self.pkt.set_timestamp(timestamp);
        self
    }

    pub fn set_nios_profile(&mut self, nios_profile: u16) -> &mut Self {
        self.pkt.set_nios_profile(nios_profile);
        self
    }

    pub fn set_rffe_profile(&mut self, rffe_profile: u8) -> &mut Self {
        self.pkt.set_rffe_profile(rffe_profile);
        self
    }

    pub fn set_port(&mut self, port: u8, module: u8) -> &mut Self {
        self.pkt.set_port(port, module);
        self
    }

    pub fn set_spdt(&mut self, spdt: u8) -> &mut Self {
        self.pkt.set_spdt(spdt);
        self
    }

    pub fn magic(&self) -> u8 {
        self.pkt.magic()
    }

    pub fn timestamp(&self) -> u64 {
        self.pkt.timestamp()
    }

    pub fn nios_profile(&self) -> u16 {
        self.pkt.nios_profile()
    }

    pub fn rffe_profile(&self) -> u8 {
        self.pkt.rffe_profile()
    }

    pub fn port(&self) -> u8 {
        self.pkt.port()
    }

    pub fn spdt(&self) -> u8 {
        self.pkt.spdt()
    }
}

impl From<Vec<u8>> for NiosPktRetune2Request {
    fn from(value: Vec<u8>) -> Self {
        Self {
            pkt: NiosPktRetune2::from(value),
        }
    }
}

impl From<NiosPktRetune2Request> for Vec<u8> {
    fn from(value: NiosPktRetune2Request) -> Self {
        value.pkt.buf
    }
}

impl Debug for NiosPktRetune2Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiosPktRetune2Request")
            .field("magic", &format_args!("{:#x}", self.magic()))
            .field("timestamp", &format_args!("{:#x}", self.timestamp()))
            .field("nios_profile", &format_args!("{:#x}", self.nios_profile()))
            .field("rffe_profile", &format_args!("{:#x}", self.rffe_profile()))
            .field("port", &format_args!("{:#x}", self.port()))
            .field("spdt", &format_args!("{:#x}", self.spdt()))
            .finish()
    }
}

///                             Response
///                      ----------------------
///
/// +================+=========================================================+
/// |  Byte offset   |                       Description                       |
/// +================+=========================================================+
/// |        0       | Magic Value                                             |
/// +----------------+---------------------------------------------------------+
/// |        1       | 64-bit duration denoting how long the operation took to |
/// |                | complete, in units of timestamp ticks. (Note 1)         |
/// +----------------+---------------------------------------------------------+
/// |        9       | Status Flags (Note 2)                                   |
/// +----------------+---------------------------------------------------------+
/// |      10-15     | Reserved. All bits set to 0.                            |
/// +----------------+---------------------------------------------------------+
///
/// (Note 1) This value will be zero if timestamps are not running for the
///          associated module.
///
/// (Note 2) Description of Status Flags:
///
///      flags\[0\]: 1 = Timestamp is valid. This is only the case for "Tune NOW"
///                    requests. It is not possible to return this information
///                    for scheduled retunes, as the event generally does not
///                    occur before the response is set.
///
///                0 = This was a scheduled retune. Timestamp fields should be
///                    ignored.
///
///      flags\[1\]: 1 = Operation completed successfully.
///                0 = Operation failed.
///
///                For "Tune NOW" requests, a failure may occur as the result
///                of the tuning algorithm failing to occur, and such other
///                unexpected failurs.
///
///                The scheduled tune request will failure if the retune queue
///                is full.
///
///      flags\[7:2\]    Reserved. Set to 0.


pub struct NiosPktRetune2Response {
    pkt: NiosPktRetune2,
}
impl NiosPktRetune2Response {
    const IDX_FLAGS: usize = 9;

    const FLAG_TSVTUNE_VALID: u8 = 0x1;
    const FLAG_SUCCESS: u8 = 0x2;

    pub fn magic(&self) -> u8 {
        self.pkt.magic()
    }

    pub fn duration(&self) -> u64 {
        self.pkt.timestamp()
    }

    pub fn flags(&self) -> u8 {
        self.pkt.buf[Self::IDX_FLAGS]
    }

    pub fn timestamp_valid(&self) -> bool {
        self.flags() & Self::FLAG_TSVTUNE_VALID != 0
    }

    pub fn is_success(&self) -> bool {
        self.flags() & Self::FLAG_SUCCESS != 0
    }
}

impl From<Vec<u8>> for NiosPktRetune2Response {
    fn from(value: Vec<u8>) -> Self {
        Self {
            pkt: NiosPktRetune2::from(value),
        }
    }
}

impl From<NiosPktRetune2Response> for Vec<u8> {
    fn from(value: NiosPktRetune2Response) -> Self {
        value.pkt.buf
    }
}

impl Debug for NiosPktRetune2Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiosPktRetune2Response")
            .field("magic", &format_args!("{:#x}", self.magic()))
            .field("duration", &format_args!("{:#x}", self.duration()))
            .field(
                "timestamp_valid",
                &format_args!("{}", self.timestamp_valid()),
            )
            .field("is_success", &format_args!("{}", self.is_success()))
            .finish()
    }
}
