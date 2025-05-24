#![allow(clippy::too_many_arguments)]
use anyhow::anyhow;
use bladerf_globals::{ENDPOINT_IN, ENDPOINT_OUT};
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, NiosPktRetuneResponse, Tune};
use nusb::Interface;
use nusb::transfer::{Buffer, Bulk, In, Out};

pub trait Nios {
    fn nios_send(
        &self,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        //ep_bulk_in: Endpoint<Bulk, In>,
        //ep_bulk_out: Endpoint<Bulk, Out>,
        pkt: Vec<u8>,
    ) -> impl Future<Output = anyhow::Result<Vec<u8>>>; // pub fn nios_retune(&self, bladerf_channel ch, uint64_t timestamp, uint16_t nint, uint32_t nfrac, uint8_t freqsel, uint8_t vcocap, bool low_band, uint8_t xb_gpio, bool quick_tune) {
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
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}

impl Nios for Interface {
    async fn nios_send(
        &self,
        //mut ep_bulk_in: Endpoint<Bulk, In>,
        //mut ep_bulk_out: Endpoint<Bulk, Out>,
        ep_bulk_out_id: u8,
        ep_bulk_in_id: u8,
        pkt: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        println!("BulkOut: {pkt:x?}");

        let mut ep_bulk_out = self.endpoint::<Bulk, Out>(ep_bulk_out_id)?;
        let mut ep_bulk_in = self.endpoint::<Bulk, In>(ep_bulk_in_id)?;

        ep_bulk_out.submit(Buffer::from(pkt));
        let mut response = ep_bulk_out.next_complete().await;
        response.status?;

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
        println!("BulkIn:  {:?}", response);
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
    ) -> anyhow::Result<()> {
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
}

// pub async fn nios_send(
//     ep_bulk_out: &mut Endpoint<Bulk, Out>,
//     ep_bulk_in: &mut Endpoint<Bulk, In>,
//     pkt: Vec<u8>,
// ) -> anyhow::Result<Vec<u8>> {
//     println!("BulkOut: {pkt:x?}");
//
//     ep_bulk_out.submit(Buffer::from(pkt));
//     let mut response = ep_bulk_out.next_complete().await;
//     response.status?;
//
//     ep_bulk_in.submit(response.buffer);
//     response = ep_bulk_in.next_complete().await;
//     response.status?;
//
//     println!("BulkIn:  {:x?}", response);
//     Ok(response.buffer.into_vec())
// }
