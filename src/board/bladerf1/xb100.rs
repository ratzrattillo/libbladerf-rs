use crate::BladeRf1;
use crate::board::bladerf1::expansion_boards::{
    BLADERF_XB100_LED_D1, BLADERF_XB100_LED_D2, BLADERF_XB100_LED_D3, BLADERF_XB100_LED_D4,
    BLADERF_XB100_LED_D5, BLADERF_XB100_LED_D6, BLADERF_XB100_LED_D7, BLADERF_XB100_LED_D8,
    BLADERF_XB100_TLED_BLUE, BLADERF_XB100_TLED_GREEN, BLADERF_XB100_TLED_RED,
};
use anyhow::Result;
use crate::nios::Nios;

impl BladeRf1 {
    pub fn xb100_attach(&self) -> Result<()> {
        Ok(())
    }

    pub fn xb100_detach(&self) -> Result<()> {
        Ok(())
    }

    pub async fn xb100_enable(&self, enable: bool) -> Result<()> {
        let mask: u32 = (BLADERF_XB100_LED_D1
            | BLADERF_XB100_LED_D2
            | BLADERF_XB100_LED_D3
            | BLADERF_XB100_LED_D4
            | BLADERF_XB100_LED_D5
            | BLADERF_XB100_LED_D6
            | BLADERF_XB100_LED_D7
            | BLADERF_XB100_LED_D8
            | BLADERF_XB100_TLED_RED
            | BLADERF_XB100_TLED_GREEN
            | BLADERF_XB100_TLED_BLUE) as u32;

        let outputs: u32 = mask;
        let default_values: u32 = mask;

        if enable {
            self.interface.nios_expansion_gpio_write(mask, outputs).await?;
            self.interface.nios_expansion_gpio_write(mask, default_values).await?;
        }
        
        Ok(())
    }

    // int xb100_init(struct bladerf *dev)
    // {
    // return 0;
    // }
    //
    // int xb100_gpio_read(struct bladerf *dev, uint32_t *val)
    // {
    // return dev->backend->expansion_gpio_read(dev, val);
    // }
    //
    // int xb100_gpio_write(struct bladerf *dev, uint32_t val)
    // {
    // return dev->backend->expansion_gpio_write(dev, 0xffffffff, val);
    // }
    //
    // int xb100_gpio_masked_write(struct bladerf *dev, uint32_t mask, uint32_t val)
    // {
    // return dev->backend->expansion_gpio_write(dev, mask, val);
    // }
    //
    // int xb100_gpio_dir_read(struct bladerf *dev, uint32_t *val)
    // {
    // return dev->backend->expansion_gpio_dir_read(dev, val);
    // }
    //
    // int xb100_gpio_dir_write(struct bladerf *dev, uint32_t val)
    // {
    // return xb100_gpio_dir_masked_write(dev, 0xffffffff, val);
    // }
    //
    // int xb100_gpio_dir_masked_write(struct bladerf *dev, uint32_t mask, uint32_t val)
    // {
    // return dev->backend->expansion_gpio_dir_write(dev, mask, val);
    // }
}
