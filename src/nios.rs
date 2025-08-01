#![allow(clippy::too_many_arguments)]

use crate::{Error, Result};
use bladerf_globals::bladerf1::BladeRfVersion;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, ENDPOINT_IN, ENDPOINT_OUT};
use bladerf_nios::packet_generic::{NiosPkt, NumToByte};
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, NiosPktRetuneResponse, Tune};
use bladerf_nios::*;
use nusb::transfer::{Buffer, Bulk, In, Out};
use nusb::{Endpoint, Interface};
use std::fmt::{Debug, Display, LowerHex};
use std::time::Duration;

pub trait NiosEP {}

impl NiosEP for Endpoint<Bulk, Out> {}

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
        module: u8,
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
    fn nios_get_fpga_version(&self) -> Result<BladeRfVersion>;
    fn nios_get_iq_gain_correction(&self, ch: u8) -> Result<i16>;
    fn nios_get_iq_phase_correction(&self, ch: u8) -> Result<i16>;
    fn nios_set_iq_gain_correction(&self, ch: u8, value: i16) -> Result<()>;
    fn nios_set_iq_phase_correction(&self, ch: u8, value: i16) -> Result<()>;
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
        let mut ep_bulk_out = self.endpoint::<Bulk, Out>(ep_bulk_out_id)?;
        let mut ep_bulk_in = self.endpoint::<Bulk, In>(ep_bulk_in_id)?;

        // TODO: Nusb specifically requires the buffer to be a nonzero multiple of endpoint.max_packet_size()
        // TODO: This could be performance optimized, by leaving out these checks, if we can be sure,
        // TODO: that all the packets given to this method have a reserved size of max_packet_len().
        let additional = if pkt.capacity() < ep_bulk_in.max_packet_size() {
            ep_bulk_in.max_packet_size() - pkt.capacity()
        } else {
            pkt.capacity() % ep_bulk_in.max_packet_size()
        };
        // reserve does nothing, if capacity is already sufficient
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
        module: u8,
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
            module, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
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

    fn nios_get_fpga_version(&self) -> Result<BladeRfVersion> {
        let regval = self.nios_read::<u8, u32>(NIOS_PKT_8X32_TARGET_VERSION, 0)?;
        log::trace!("Read FPGA version word: {regval:#010x}");

        let version = BladeRfVersion {
            major: ((regval >> 24) & 0xff) as u16,
            minor: ((regval >> 16) & 0xff) as u16,
            // #[cfg(target_endian = "big")]
            // patch: ((regval & 0xffff) as u16).to_be(),
            // #[cfg(target_endian = "little")]
            patch: ((regval & 0xffff) as u16).to_be(),
        };
        Ok(version)
    }

    fn nios_get_iq_gain_correction(&self, ch: u8) -> Result<i16> {
        let addr = match ch {
            BLADERF_MODULE_RX => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN,
            BLADERF_MODULE_TX => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN,
            _ => {
                log::error!("Invalid channel: {ch}");
                return Err(Error::Invalid);
            }
        };
        Ok(self.nios_read::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr)? as i16)
    }

    fn nios_get_iq_phase_correction(&self, ch: u8) -> Result<i16> {
        let addr = match ch {
            BLADERF_MODULE_RX => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE,
            BLADERF_MODULE_TX => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE,
            _ => {
                log::error!("Invalid channel: {ch}");
                return Err(Error::Invalid);
            }
        };
        Ok(self.nios_read::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr)? as i16)
    }

    fn nios_set_iq_gain_correction(&self, ch: u8, value: i16) -> Result<()> {
        let addr = match ch {
            BLADERF_MODULE_RX => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN,
            BLADERF_MODULE_TX => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN,
            _ => {
                log::error!("Invalid channel: {ch}");
                return Err(Error::Invalid);
            }
        };
        self.nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr, value as u16)
    }

    fn nios_set_iq_phase_correction(&self, ch: u8, value: i16) -> Result<()> {
        let addr = match ch {
            BLADERF_MODULE_RX => NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE,
            BLADERF_MODULE_TX => NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE,
            _ => {
                log::error!("Invalid channel: {ch}");
                return Err(Error::Invalid);
            }
        };
        self.nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_IQ_CORR, addr, value as u16)
    }
}
