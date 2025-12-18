#![allow(clippy::too_many_arguments)]

mod packet_base;
pub mod packet_generic;
pub mod packet_retune;
pub mod packet_retune2;

use crate::hardware::lms6002d::{Band, Tune};
use crate::nios::packet_generic::{NiosPkt, NumToByte};
use crate::nios::packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};
//use crate::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use crate::bladerf::Channel;
use crate::{Error, Result, SemanticVersion};
use nusb::Interface;
use nusb::transfer::{Buffer, Bulk, In, Out};
use std::fmt::{Debug, Display, LowerHex};
use std::time::Duration;

#[repr(u8)]
#[derive(Debug)]
pub enum NiosPktMagic {
    // Invalid = 0x00, // 'INVALID'
    _8X8 = 0x41,   // 'A'
    _8X16 = 0x42,  // 'B'
    _8X32 = 0x43,  // 'C'
    _8X64 = 0x44,  // 'D'
    _16X64 = 0x45, // 'E'
    _32X32 = 0x4B, // 'K'
    // Legacy = 0x4E,  // 'N'
    Retune = 0x54,  // 'T'
    Retune2 = 0x55, // 'U'
}

// #[repr(u8)]
// #[derive(Debug)]
// pub enum NiosPktFlags {
//     ReadFailure = 0x0,
//     WriteFailure = 0x1,
//     ReadSuccess = 0x2,
//     WriteSuccess = 0x3,
//}

// #[repr(u8)]
// #[derive(Debug)]
// pub enum NiosPktFlags {
//     Read = 0x0,
//     Write = 0x1,
// }

// use thiserror::Error;
// #[derive(Debug, Error, PartialEq)]
// pub enum ValidationError {
//     #[error("Invalid Magic Number {0}!")]
//     InvalidMagic(u8),
//     #[error("Invalid Reserved Byte {0}!")]
//     InvalidReserved(u8),
//     #[error("Nonzero Padding!")]
//     InvalidPadding(Vec<u8>),
//     #[error("Invalid Packet Length {0}!")]
//     InvalidLength(usize),
//     #[error("Nint too big {0}!")]
//     NintOverflow(u16),
//     #[error("Nfrac too big {0}!")]
//     NfracOverflow(u32),
//     #[error("Freqsel too big {0}!")]
//     FreqselOverflow(u8),
//     #[error("Vcocap too big {0}!")]
//     VcocapOverflow(u8),
// }

/* IDs 0x80 through 0xff will not be assigned by Nuand. These are reserved
 * for user customizations */
// pub const NIOS_PKT_TARGET_USR1: u8 = 0x80;
// pub const NIOS_PKT_TARGET_USR128: u8 = 0xff;

/* Target IDs */
pub const NIOS_PKT_8X8_TARGET_LMS6: u8 = 0x00; /* LMS6002D register access */
pub const NIOS_PKT_8X8_TARGET_SI5338: u8 = 0x01; /* Si5338 register access */
// pub const NIOS_PKT_8X8_TARGET_VCTCXO_TAMER: u8 = 0x02; /* VCTCXO Tamer control */
// pub const NIOS_PKT_8X8_TX_TRIGGER_CTL: u8 = 0x03; /* TX trigger control */
// pub const NIOS_PKT_8X8_RX_TRIGGER_CTL: u8 = 0x04; /* RX trigger control */
/* Target IDs */
pub const NIOS_PKT_8X16_TARGET_VCTCXO_DAC: u8 = 0x00;
pub const NIOS_PKT_8X16_TARGET_IQ_CORR: u8 = 0x01;
// pub const NIOS_PKT_8X16_TARGET_AGC_CORR: u8 = 0x02;
// pub const NIOS_PKT_8X16_TARGET_AD56X1_DAC: u8 = 0x03;
// pub const NIOS_PKT_8X16_TARGET_INA219: u8 = 0x04;

/* Sub-addresses for the IQ Correction target block */
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN: u8 = 0x00;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE: u8 = 0x01;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN: u8 = 0x02;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE: u8 = 0x03;

// /* Sub-addresses for the AGC DC Correction target block */
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MAX: u8 = 0x00;
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MAX: u8 = 0x01;
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MID: u8 = 0x02;
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MID: u8 = 0x03;
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MIN: u8 = 0x04;
// pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MIN: u8 = 0x05;
/* Target IDs */
pub const NIOS_PKT_8X32_TARGET_VERSION: u8 = 0x00; /* FPGA version (read only) */
pub const NIOS_PKT_8X32_TARGET_CONTROL: u8 = 0x01; /* FPGA control/config register */
pub const NIOS_PKT_8X32_TARGET_ADF4351: u8 = 0x02; /* XB-200 ADF4351 register access (write-only) */
// pub const NIOS_PKT_8X32_TARGET_RFFE_CSR: u8 = 0x03; /* RFFE control & status GPIO */
// pub const NIOS_PKT_8X32_TARGET_ADF400X: u8 = 0x04; /* ADF400x config */
// pub const NIOS_PKT_8X32_TARGET_FASTLOCK: u8 = 0x05; /* Save AD9361 fast lock profile
//  * to Nios */
//
// /* Target IDs */
//
// pub const NIOS_PKT_8X64_TARGET_TIMESTAMP: u8 = 0x00; /* Timestamp readback (read only) */
//
// /* Sub-addresses for timestamp target */
// pub const NIOS_PKT_8X64_TIMESTAMP_RX: u8 = 0x00;
// pub const NIOS_PKT_8X64_TIMESTAMP_TX: u8 = 0x01;
//
// /* Target IDs */
// pub const NIOS_PKT_16X64_TARGET_AD9361: u8 = 0x00;
// pub const NIOS_PKT_16X64_TARGET_RFIC: u8 = 0x01; /* RFIC control */
/* Target IDs */

/* For the EXP and EXP_DIR targets, the address is a bitmask of values
 * to read/write */
pub const NIOS_PKT_32X32_TARGET_EXP: u8 = 0x00; /* Expansion I/O */
pub const NIOS_PKT_32X32_TARGET_EXP_DIR: u8 = 0x01; /* Expansion I/O Direction reg */
// pub const NIOS_PKT_32X32_TARGET_ADI_AXI: u8 = 0x02; /* ADI AXI Interface */
// pub const NIOS_PKT_32X32_TARGET_WB_MSTR: u8 = 0x03; /* Wishbone Master */
pub const ENDPOINT_OUT: u8 = 0x02;
pub const ENDPOINT_IN: u8 = 0x82;

pub trait Nios {
    fn nios_send(
        &self,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        pkt: Vec<u8>,
        timeout: Option<Duration>,
    ) -> Result<Vec<u8>>;
    fn nios_retune(
        &self,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<()>;
    fn nios_read<
        A: NumToByte + Debug + Display + LowerHex + Send,
        D: NumToByte + Debug + Display + LowerHex + Send,
    >(
        &self,
        id: u8,
        addr: A,
    ) -> Result<D>;

    fn nios_write<
        A: NumToByte + Debug + Display + LowerHex + Send,
        D: NumToByte + Debug + Display + LowerHex + Send,
    >(
        &self,
        id: u8,
        addr: A,
        data: D,
    ) -> Result<()>;
    fn nios_config_read(&self) -> Result<u32>;
    fn nios_config_write(&self, value: u32) -> Result<()>;
    fn nios_xb200_synth_write(&self, value: u32) -> Result<()>;
    fn nios_expansion_gpio_read(&self) -> Result<u32>;
    fn nios_expansion_gpio_write(&self, mask: u32, val: u32) -> Result<()>;
    fn nios_expansion_gpio_dir_read(&self) -> Result<u32>;
    fn nios_expansion_gpio_dir_write(&self, mask: u32, val: u32) -> Result<()>;
    fn nios_get_fpga_version(&self) -> Result<SemanticVersion>;
    fn nios_get_iq_gain_correction(&self, ch: Channel) -> Result<i16>;
    fn nios_get_iq_phase_correction(&self, ch: Channel) -> Result<i16>;
    fn nios_set_iq_gain_correction(&self, ch: Channel, value: i16) -> Result<()>;
    fn nios_set_iq_phase_correction(&self, ch: Channel, value: i16) -> Result<()>;
}

impl Nios for Interface {
    fn nios_send(
        &self,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        mut pkt: Vec<u8>,
        timeout: Option<Duration>,
    ) -> Result<Vec<u8>> {
        // TODO: An endpoint handle should probably not be acquired on every call to nios_send!!
        // When running tests, this fails sometimes even for cargo test -- --test-threads=1 --no-capture
        // let mut ep_bulk_out = self.endpoint::<Bulk, Out>(ep_bulk_out_id)?;
        // let mut ep_bulk_in = self.endpoint::<Bulk, In>(ep_bulk_in_id)?;

        // Endpoint might be in use, so we constantly retry...
        // When running tests, this does not fail for cargo test -- --test-threads=1 --no-capture
        // But it sometimes fails for parallel calls e.g:  cargo test -- --test-threads=12 --no-capture
        // TODO: Find a fix, to allow concurrent nios commands...
        let mut ep_bulk_out = loop {
            if let Ok(e) = self.endpoint::<Bulk, Out>(ep_bulk_out_id) {
                break e;
            }
        };
        let mut ep_bulk_in = loop {
            if let Ok(e) = self.endpoint::<Bulk, In>(ep_bulk_in_id) {
                break e;
            }
        };

        // TODO: Nusb specifically requires the buffer to be a nonzero multiple of endpoint.max_packet_size()
        // TODO: This could be performance optimized, by leaving out these checks, if we can be sure,
        // TODO: that all the packets given to this method have a reserved size of max_packet_len().
        let additional = if pkt.capacity() < ep_bulk_in.max_packet_size() {
            ep_bulk_in.max_packet_size() - pkt.capacity()
        } else {
            pkt.capacity() % ep_bulk_in.max_packet_size()
        };
        // reserve does nothing if capacity is already sufficient
        pkt.reserve(additional);

        let t = timeout.unwrap_or(Duration::from_millis(100));
        ep_bulk_out.submit(Buffer::from(pkt));
        let mut response = ep_bulk_out.wait_next_complete(t).unwrap();
        response.status?;

        // Nusb requires the buffer for an IN transfer to be at least ep_bulk_in.max_packet_size() big
        response
            .buffer
            .set_requested_len(ep_bulk_in.max_packet_size());
        ep_bulk_in.submit(response.buffer);
        response = ep_bulk_in.wait_next_complete(t).unwrap();
        response.status?;

        // Todo: This should be a generic NIOS packet type, or just a plain Vec,
        // Todo: We might not be able to easily check for a success flag, as we do not
        // Todo: know which kind of packet was sent.
        // type NiosPkt = NiosPkt8x8;
        // let nios_pkt = NiosPktResponse::<u8,u8>::from(response.buffer);
        // if !nios_pkt.is_success() {
        //     return Err(anyhow!("operation was unsuccessful!"));
        // }
        // log::debug!("BulkIn:  {nios_pkt:x?}");
        // let response_vec = nios_pkt.into();

        // log::debug!("BulkIn:  {:?}", response);
        Ok(response.buffer.into_vec())
    }

    fn nios_retune(
        &self,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<()> {
        if timestamp == NiosPktRetuneRequest::RETUNE_NOW {
            log::trace!("Clearing Retune Queue");
        } else {
            log::trace!("Log tuning parameters here...");
        }

        let pkt = NiosPktRetuneRequest::new(
            channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        );

        let response_vec = self.nios_send(ENDPOINT_OUT, ENDPOINT_IN, pkt.into(), None)?;
        let resp_pkt = NiosPktRetuneResponse::from(response_vec);

        if resp_pkt.duration_and_vcocap_valid() {
            log::trace!(
                "Retune operation: {vcocap}, Duration: {}",
                resp_pkt.duration()
            );
        } else {
            log::trace!("Duration: {}", resp_pkt.duration());
        }

        if !resp_pkt.is_success() {
            return if timestamp == NiosPktRetuneRequest::RETUNE_NOW {
                log::error!("FPGA tuning reported failure.");
                Err(Error::Invalid)
            } else {
                log::error!(
                    "The FPGA's retune queue is full. Try again after a previous request has completed."
                );
                Err(Error::Invalid)
            };
        }

        Ok(())
    }

    fn nios_read<A, D>(&self, id: u8, addr: A) -> Result<D>
    where
        A: NumToByte + Debug + Display + LowerHex + Send,
        D: NumToByte + Debug + Display + LowerHex + Send,
    {
        // The address is used as a mask of bits to read and return
        let mut pkt = NiosPkt::<A, D>::from(vec![0u8; 16]);
        pkt.set_magic(NiosPkt::<A, D>::MAGIC);
        pkt.set_target_id(id);
        pkt.set_flags(NiosPkt::<A, D>::FLAG_READ);
        pkt.set_addr(addr);

        // let pkt = NiosPkt::<A, D>::new(id, NiosPkt::<A, D>::FLAG_WRITE, addr, data);

        let response_vec = self.nios_send(ENDPOINT_OUT, ENDPOINT_IN, pkt.into(), None)?;
        Ok(NiosPkt::<A, D>::from(response_vec).data())
    }

    fn nios_write<A, D>(&self, id: u8, addr: A, data: D) -> Result<()>
    where
        A: NumToByte + Debug + Display + LowerHex + Send,
        D: NumToByte + Debug + Display + LowerHex + Send,
    {
        // The address is used as a mask of bits to read and return
        let mut pkt = NiosPkt::<A, D>::from(vec![0u8; 16]);
        pkt.set_magic(NiosPkt::<A, D>::MAGIC);
        pkt.set_target_id(id);
        pkt.set_flags(NiosPkt::<A, D>::FLAG_WRITE);
        pkt.set_addr(addr);
        pkt.set_data(data);

        // let pkt = PktType::new(id, PktType::FLAG_WRITE, addr, data);
        let resp = self.nios_send(ENDPOINT_OUT, ENDPOINT_IN, pkt.into(), None)?;
        let resp_pkt: NiosPkt<A, D> = resp.into();
        if resp_pkt.is_success() {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }

    // fn nios_32x32_masked_read(&self, id: u8, mask: u32) -> Result<u32> {
    //     self.nios_read::<u32, u32>(id, mask)
    // }
    //
    // fn nios_32x32_masked_write(&self, id: u8, mask: u32, val: u32) -> Result<()> {
    //     self.nios_write::<u32, u32>(id, mask, val)
    // }

    fn nios_config_read(&self) -> Result<u32> {
        self.nios_read::<u8, u32>(NIOS_PKT_8X32_TARGET_CONTROL, 0)
    }

    fn nios_config_write(&self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NIOS_PKT_8X32_TARGET_CONTROL, 0, value)
    }

    fn nios_xb200_synth_write(&self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NIOS_PKT_8X32_TARGET_ADF4351, 0, value)
    }

    fn nios_expansion_gpio_read(&self) -> Result<u32> {
        self.nios_read::<u32, u32>(NIOS_PKT_32X32_TARGET_EXP, 0xffffffff)
    }

    fn nios_expansion_gpio_write(&self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NIOS_PKT_32X32_TARGET_EXP, mask, val)
    }

    fn nios_expansion_gpio_dir_read(&self) -> Result<u32> {
        self.nios_read::<u32, u32>(NIOS_PKT_32X32_TARGET_EXP_DIR, 0xffffffff)
    }

    fn nios_expansion_gpio_dir_write(&self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NIOS_PKT_32X32_TARGET_EXP_DIR, mask, val)
    }

    fn nios_get_fpga_version(&self) -> Result<SemanticVersion> {
        let regval = self.nios_read::<u8, u32>(NIOS_PKT_8X32_TARGET_VERSION, 0)?;
        log::trace!("Read FPGA version word: {regval:#010x}");

        let version = SemanticVersion {
            major: ((regval >> 24) & 0xff) as u16,
            minor: ((regval >> 16) & 0xff) as u16,
            // #[cfg(target_endian = "big")]
            // patch: ((regval & 0xffff) as u16).to_be(),
            // #[cfg(target_endian = "little")]
            patch: ((regval & 0xffff) as u16).to_be(),
        };
        Ok(version)
    }

    fn nios_get_iq_gain_correction(&self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN,
            Channel::Tx => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN,
        };
        Ok(self.nios_read::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr)? as i16)
    }

    fn nios_get_iq_phase_correction(&self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE,
            Channel::Tx => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE,
        };
        Ok(self.nios_read::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr)? as i16)
    }

    fn nios_set_iq_gain_correction(&self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN,
            Channel::Tx => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN,
        };
        self.nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr, value as u16)
    }

    fn nios_set_iq_phase_correction(&self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE,
            Channel::Tx => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE,
        };
        self.nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr, value as u16)
    }
}
