use crate::nios::Nios;
use anyhow::Result;
use bladerf_globals::{ENDPOINT_IN, ENDPOINT_OUT};
use bladerf_nios::NIOS_PKT_8X16_TARGET_VCTCXO_DAC;
use bladerf_nios::packet::NiosPkt8x16;
use nusb::Interface;

pub struct DAC161S055 {
    interface: Interface,
    // ep_bulk_out: &'a Endpoint<Bulk, Out>,
    // ep_bulk_in: &'a Endpoint<Bulk, In>,
}

impl DAC161S055 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub async fn write(&self, value: u16) -> Result<u16> {
        type NiosPkt = NiosPkt8x16;

        // /* Ensure the device is in write-through mode */
        // status = dev->backend->vctcxo_dac_write(dev, 0x28, 0x0000);
        // if (status < 0) {
        //     return status;
        // }

        /* Ensure the device is in write-through mode */
        let mut request = NiosPkt::new(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            NiosPkt::FLAG_WRITE,
            0x28,
            0x0000,
        );

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        //let response = self.interface.nios_send(request.into()).await?;

        // /* Write DAC value to channel 0 */
        // status = dev->backend->vctcxo_dac_write(dev, 0x08, value);
        // if (status < 0) {
        //     return status;
        // }

        /* Write DAC value to channel 0 */
        request = NiosPkt::from(response);
        request.set(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            NiosPkt::FLAG_WRITE,
            0x8,
            value,
        );

        // let response = self
        //     .interface
        //     .nios_send(ENDPOINT_IN, ENDPOINT_OUT, request.into())?;
        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;

        Ok(NiosPkt::from(response).data())
    }
}
