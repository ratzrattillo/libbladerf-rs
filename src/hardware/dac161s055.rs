use crate::nios::Nios;
use anyhow::Result;
use bladerf_globals::{ENDPOINT_IN, ENDPOINT_OUT};
use bladerf_nios::{NIOS_PKT_8X16_TARGET_VCTCXO_DAC};
use bladerf_nios::packet_generic::{NiosReq8x16, NiosResp8x16};
use nusb::Interface;

pub struct DAC161S055 {
    interface: Interface,
}

impl DAC161S055 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub async fn write(&self, value: u16) -> Result<u16> {
        type ReqType = NiosReq8x16;

        /* Ensure the device is in write-through mode */
        let mut request = ReqType::new(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            ReqType::FLAG_WRITE,
            0x28,
            0x0,
        );

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;

        /* Write DAC value to channel 0 */
        request = ReqType::from(response);
        request.set(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            ReqType::FLAG_WRITE,
            0x8,
            value,
        );

        let response_vec = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;

        Ok(NiosResp8x16::from(response_vec).data())
    }
}
