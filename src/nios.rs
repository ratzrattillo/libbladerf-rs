pub(crate) mod constants;

use anyhow::anyhow;
use futures_lite::future::block_on;
use libnios_rs::packet::NiosPkt8x8;
use nusb::Interface;
use nusb::transfer::RequestBuffer;

pub trait Nios {
    fn nios_send(&self, endpoint_in: u8, endpoint_out: u8, pkt: Vec<u8>)
    -> anyhow::Result<Vec<u8>>;
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
}
