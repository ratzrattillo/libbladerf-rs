use anyhow::anyhow;
use bladerf_nios::packet::NiosPkt8x8;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, NiosPktRetuneResponse, Tune};
use futures_lite::future::block_on;
use nusb::Interface;
use nusb::transfer::RequestBuffer;

pub trait Nios {
    fn nios_send(&self, endpoint_in: u8, endpoint_out: u8, pkt: Vec<u8>)
    -> anyhow::Result<Vec<u8>>; // pub fn nios_retune(&self, bladerf_channel ch, uint64_t timestamp, uint16_t nint, uint32_t nfrac, uint8_t freqsel, uint8_t vcocap, bool low_band, uint8_t xb_gpio, bool quick_tune) {
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
    ) -> anyhow::Result<()>;
}
impl Nios for Interface {
    fn nios_send(
        &self,
        endpoint_in: u8,
        endpoint_out: u8,
        pkt: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        println!("BulkOut: {pkt:x?}");
        let response = block_on(self.bulk_out(endpoint_out, pkt)).into_result()?;

        let response =
            block_on(self.bulk_in(endpoint_in, RequestBuffer::reuse(response.reuse(), 16)))
                .into_result()?;

        // This could be a generic NIOS packet type, or just a plain Vec,
        // where we check the index of the flags byte for a set success bit.
        type NiosPkt = NiosPkt8x8;
        let nios_pkt = NiosPkt::from(response);
        if !nios_pkt.is_success() {
            return Err(anyhow!("operation was unsuccessful!"));
        }
        println!("BulkIn:  {nios_pkt:x?}");
        let response_vec = nios_pkt.into();
        // println!("BulkIn:  {:x?}", response_vec);
        Ok(response_vec)
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
    ) -> anyhow::Result<()> {
        if timestamp == NiosPktRetuneRequest::RETUNE_NOW {
            println!("Clearing Retune Queue");
        } else {
            println!("Log tuning parameters here...");
        }

        let pkt = NiosPktRetuneRequest::new(
            module, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        );

        let response_vec = self.nios_send(
            bladerf_globals::ENDPOINT_IN,
            bladerf_globals::ENDPOINT_OUT,
            pkt.into(),
        )?;
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
