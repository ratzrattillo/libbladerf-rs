use crate::protocol::nios::packet_generic::NiosPacket;
use crate::{Channel, Error, NiosPacketError, Result};

pub struct NiosPktRetuneRequest {
    buf: Vec<u8>,
}

impl NiosPacket for NiosPktRetuneRequest {
    fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn into_packet(self) -> Vec<u8> {
        self.buf
    }
}

impl NiosPktRetuneRequest {
    const NIOS_PKT_RETUNE2_MAGIC: u8 = 0x55;
    const IDX_MAGIC: usize = 0;
    const IDX_TIMESTAMP: usize = 1;
    const IDX_NIOS_PROFILE: usize = 9;
    const IDX_RFFE_PROFILE: usize = 11;
    const IDX_RFFE_PORT: usize = 12;
    const IDX_SPDT: usize = 13;

    const MASK_PORT_IS_RX: u8 = 1 << 7;

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        self,
        channel: Channel,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> Self {
        self.set_magic()
            .set_timestamp(timestamp)
            .set_nios_profile(nios_profile)
            .set_rffe_profile(rffe_profile)
            .set_port(port, channel)
            .set_spdt(spdt)
    }

    fn set_magic(mut self) -> Self {
        self.buf[Self::IDX_MAGIC] = Self::NIOS_PKT_RETUNE2_MAGIC;
        self
    }

    fn set_timestamp(mut self, timestamp: u64) -> Self {
        self.write_u64(Self::IDX_TIMESTAMP, timestamp);
        self
    }

    fn set_nios_profile(mut self, nios_profile: u16) -> Self {
        self.write_u16(Self::IDX_NIOS_PROFILE, nios_profile);
        self
    }

    fn set_rffe_profile(mut self, rffe_profile: u8) -> Self {
        self.buf[Self::IDX_RFFE_PROFILE] = rffe_profile;
        self
    }

    fn set_port(mut self, port: u8, channel: Channel) -> Self {
        let pkt_port = (port & !Self::MASK_PORT_IS_RX)
            | if channel.is_tx() {
                0x0
            } else {
                Self::MASK_PORT_IS_RX
            };
        self.buf[Self::IDX_RFFE_PORT] = pkt_port;
        self
    }

    fn set_spdt(mut self, spdt: u8) -> Self {
        self.buf[Self::IDX_SPDT] = spdt;
        self
    }

    pub fn timestamp(&self) -> u64 {
        self.read_u64(Self::IDX_TIMESTAMP)
    }

    pub fn nios_profile(&self) -> u16 {
        self.read_u16(Self::IDX_NIOS_PROFILE)
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
}

impl TryFrom<Vec<u8>> for NiosPktRetuneRequest {
    type Error = Error;

    fn try_from(buf: Vec<u8>) -> Result<Self> {
        Ok(Self { buf })
    }
}

pub struct NiosPktRetuneResponse {
    buf: Vec<u8>,
}

impl NiosPacket for NiosPktRetuneResponse {
    fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn into_packet(self) -> Vec<u8> {
        self.buf
    }
}

impl NiosPktRetuneResponse {
    const IDX_TIMESTAMP: usize = 1;
    const IDX_FLAGS: usize = 9;

    const FLAG_TSVTUNE_VALID: u8 = 0x1;
    const FLAG_SUCCESS: u8 = 0x2;

    pub fn duration(&self) -> u64 {
        self.read_u64(Self::IDX_TIMESTAMP)
    }

    pub fn timestamp_valid(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_TSVTUNE_VALID != 0
    }

    pub fn is_success(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_SUCCESS != 0
    }
}

impl TryFrom<Vec<u8>> for NiosPktRetuneResponse {
    type Error = Error;

    fn try_from(mut buf: Vec<u8>) -> Result<Self> {
        if buf.len() < 16 {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        buf.truncate(16);
        Ok(Self { buf })
    }
}
