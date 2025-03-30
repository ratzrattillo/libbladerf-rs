use crate::nios::packet_generic::NiosPacket;
use anyhow::anyhow;
use futures_lite::future::block_on;
use nusb::Interface;
use nusb::transfer::RequestBuffer;

pub mod constants;
pub mod packet16x64;
pub mod packet32x32;
pub mod packet8x16;
pub mod packet8x32;
pub mod packet8x64;
pub mod packet8x8;
mod packet_generic;

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
        println!("BulkOut: {:x?}", pkt);
        let response = block_on(self.bulk_out(endpoint_out, pkt)).into_result()?;

        let response =
            block_on(self.bulk_in(endpoint_in, RequestBuffer::reuse(response.reuse(), 16)))
                .into_result()?;

        let nios_pkt = NiosPacket::from_vec(response);
        if !nios_pkt.success() {
            return Err(anyhow!("operation was unsuccessful!"));
        }
        let response_vec = nios_pkt.into_vec();
        println!("BulkIn:  {:x?}", response_vec);
        Ok(response_vec)
    }
}
