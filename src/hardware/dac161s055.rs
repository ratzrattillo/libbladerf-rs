use crate::nios::Nios;
use crate::nios::constants::{
    NIOS_PKT_8X8_TARGET_LMS6, NIOS_PKT_8X16_TARGET_VCTCXO_DAC, NIOS_PKT_FLAG_WRITE,
};
use crate::nios::packet8x16::NiosPacket8x16;
use anyhow::Result;
use nusb::Interface;

const PERIPHERAL_ENDPOINT_OUT: u8 = 0x02;
const PERIPHERAL_ENDPOINT_IN: u8 = 0x82;

pub struct DAC161S055 {
    interface: Interface,
}

impl DAC161S055 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub fn write(&self, value: u16) -> Result<u16> {
        /* Ensure device is in write-through mode */
        let mut request = NiosPacket8x16::new();
        request.set(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            NIOS_PKT_FLAG_WRITE,
            0x28,
            0x0000,
        );

        let response = self.interface.nios_send(
            PERIPHERAL_ENDPOINT_IN,
            PERIPHERAL_ENDPOINT_OUT,
            request.into_vec(),
        )?;

        //Ok(NiosPacket8x16::reuse(response).data())
        /* Write DAC value to channel 0 */
        request = NiosPacket8x16::reuse(response);
        request.set(
            NIOS_PKT_8X16_TARGET_VCTCXO_DAC,
            NIOS_PKT_FLAG_WRITE,
            0x8,
            value,
        );

        let response = self.interface.nios_send(
            PERIPHERAL_ENDPOINT_IN,
            PERIPHERAL_ENDPOINT_OUT,
            request.into_vec(),
        )?;

        Ok(NiosPacket8x16::reuse(response).data())

        // /* Ensure device is in write-through mode */
        // status = dev->backend->vctcxo_dac_write(dev, 0x28, 0x0000);
        // if (status < 0) {
        //     return status;
        // }
        //
        // /* Write DAC value to channel 0 */
        // status = dev->backend->vctcxo_dac_write(dev, 0x08, value);
        // if (status < 0) {
        //     return status;
        // }
    }
}
