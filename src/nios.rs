#![allow(clippy::too_many_arguments)]

use anyhow::{Result, anyhow};
use bladerf_globals::{ENDPOINT_IN, ENDPOINT_OUT};
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, NiosPktRetuneResponse, Tune};
use nusb::Interface;
use nusb::transfer::{Buffer, Bulk, In, Out};
use bladerf_nios::packet_generic::{NiosReq32x32, NiosResp32x32};
use bladerf_nios::*;

pub trait Nios {
    fn nios_send(
        &self,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        pkt: Vec<u8>,
    ) -> impl Future<Output = Result<Vec<u8>>>;
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
    ) -> impl Future<Output = Result<()>> + Send;
    fn nios_32x32_masked_write(&self, id: u8, mask: u32, val: u32) -> impl Future<Output = Result<()>> + Send;
    fn nios_expansion_gpio_write(&self, mask: u32, val: u32) -> impl Future<Output = Result<()>> + Send;
}

impl Nios for Interface {
    async fn nios_send(
        &self,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        mut pkt: Vec<u8>,
    ) -> Result<Vec<u8>> {
        // println!("BulkOut: {pkt:x?}");

        let mut ep_bulk_out = self.endpoint::<Bulk, Out>(ep_bulk_out_id)?;
        let mut ep_bulk_in = self.endpoint::<Bulk, In>(ep_bulk_in_id)?;

        // Nusb specifically requires the buffer to be a nonzero multiple of endpoint.max_packet_size()
        // TODO: This could be performance optimized, by leaving out these checks, if we can be sure,
        // TODO: that all the packets given to this method have a reserved size of max_packet_len().
        let additional = if pkt.len() < ep_bulk_in.max_packet_size() {
            ep_bulk_in.max_packet_size() - pkt.len()
        } else {
            pkt.len() % ep_bulk_in.max_packet_size()
        };
        pkt.reserve(additional);

        ep_bulk_out.submit(Buffer::from(pkt));
        let mut response = ep_bulk_out.next_complete().await;
        response.status?;

        // Nusb requires the buffer for an IN transfer to be at least ep_bulk_in.max_packet_size() big
        response
            .buffer
            .set_requested_len(ep_bulk_in.max_packet_size());
        ep_bulk_in.submit(response.buffer);
        response = ep_bulk_in.next_complete().await;
        response.status?;

        // Todo: This should be a generic NIOS packet type, or just a plain Vec,
        // Todo: We might not be able to easily check for a success flag, as we do not
        // Todo: know which kind of packet was sent.
        // type NiosPkt = NiosPkt8x8;
        // let nios_pkt = NiosPkt::from(response);
        // if !nios_pkt.is_success() {
        //     return Err(anyhow!("operation was unsuccessful!"));
        // }
        // println!("BulkIn:  {nios_pkt:x?}");
        // let response_vec = nios_pkt.into();

        // println!("BulkIn:  {:?}", response);
        Ok(response.buffer.into_vec())
    }

    async fn nios_retune(
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
            println!("Clearing Retune Queue");
        } else {
            println!("Log tuning parameters here...");
        }

        let pkt = NiosPktRetuneRequest::new(
            module, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        );

        let response_vec = self
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, pkt.into())
            .await?;
        let resp_pkt = NiosPktRetuneResponse::from(response_vec);

        if resp_pkt.duration_and_vcocap_valid() {
            println!(
                "Retune operation: {vcocap}, Duration: {}",
                resp_pkt.duration()
            );
        } else {
            println!("Duration: {}", resp_pkt.duration());
        }

        if !resp_pkt.is_success() {
            if timestamp == NiosPktRetuneRequest::RETUNE_NOW {
                println!("FPGA tuning reported failure.");
                return Err(anyhow!("Unexpected error"));
            } else {
                println!(
                    "The FPGA's retune queue is full. Try again after a previous request has completed."
                );
                return Err(anyhow!("Queue full"));
            }
        }

        Ok(())
    }

    async fn nios_32x32_masked_write(&self, id: u8, mask: u32, val: u32) -> Result<()> {
        type PktType = NiosReq32x32;
        /* The address is used as a mask of bits to read and return */
        let pkt = PktType::new(id, PktType::FLAG_WRITE, mask, val);
        let resp = self.nios_send(ENDPOINT_OUT, ENDPOINT_IN, pkt.into()).await?;
        let resp_pkt: NiosResp32x32 = resp.into();
        resp_pkt.is_success()
    }

    async fn nios_expansion_gpio_write(&self, mask: u32, val: u32) -> Result<()> {
        self.nios_32x32_masked_write(NIOS_PKT_32X32_TARGET_EXP, mask, val).await
    }
}
