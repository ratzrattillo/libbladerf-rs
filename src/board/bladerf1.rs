#![allow(private_interfaces, dead_code)]

use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::{BLADERF1_BAND_HIGH, LMS6002D};
use crate::hardware::si5338::SI5338;
use crate::nios::Nios;
use anyhow::{Result, anyhow};
use bladerf_globals::bladerf1::{__scale_int, __unscale_int};
pub(crate) use bladerf_globals::bladerf1::{
    _apportion_gain, BLADERF_LNA_GAIN_MAX_DB, BLADERF_LNA_GAIN_MID_DB, BLADERF_RXVGA1_GAIN_MAX,
    BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX, BLADERF_RXVGA2_GAIN_MIN,
    BLADERF_TXVGA1_GAIN_MAX, BLADERF_TXVGA1_GAIN_MIN, BLADERF_TXVGA2_GAIN_MAX,
    BLADERF_TXVGA2_GAIN_MIN, BLADERF1_RX_GAIN_OFFSET, BLADERF1_TX_GAIN_OFFSET, BladerfLnaGain,
};
pub(crate) use bladerf_globals::bladerf1::{
    BLADERF_FREQUENCY_MAX, BLADERF_FREQUENCY_MIN, BLADERF_GPIO_AGC_ENABLE,
    BLADERF_GPIO_FEATURE_SMALL_DMA_XFER, BLADERF_GPIO_PACKET, BLADERF_GPIO_TIMESTAMP,
    BLADERF_GPIO_TIMESTAMP_DIV2, BLADERF1_USB_PID, BLADERF1_USB_VID,
};
pub use bladerf_globals::{
    BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladerfFormat, BladerfGainMode, DescriptorTypes,
    ENDPOINT_IN, ENDPOINT_OUT, StringDescriptors, TIMEOUT, bladerf_channel_is_tx,
    bladerf_channel_rx, bladerf_channel_tx,
};
pub use bladerf_globals::{BladeRfDirection, SdrRange, bladerf1};
use bladerf_nios::NIOS_PKT_8X32_TARGET_CONTROL;
use bladerf_nios::packet_generic::NiosPkt8x32;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest};
use nusb::descriptors::ConfigurationDescriptor;
use nusb::transfer::{Bulk, ControlIn, ControlOut, ControlType, In, Recipient};
use nusb::{Device, DeviceInfo, Interface, Speed};
use std::num::NonZero;
use std::ops::Range;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
}

pub struct BladeRf1 {
    device: Device,
    interface: Interface,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    // xb200: Option<XB200>,
}

impl BladeRf1 {
    async fn list_bladerf1() -> Result<impl Iterator<Item = DeviceInfo>> {
        Ok(nusb::list_devices().await?.filter(|dev| {
            dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID
        }))
    }

    async fn build(device: Device) -> Result<Box<Self>> {
        let interface = device.detach_and_claim_interface(0).await?;
        // TODO Have a reference to a backend instance that holds the endpoints needed
        // TODO Give this reference to the individual Hardware...
        // TODO: Fix this with RefCell<BackendTest> with interior mutability or Mutex?.
        // Question:: Is it better to claim an endpoint from an interface in each method,
        // where we need to write data or is it better to have the whole Backend behind a mutex?
        let lms = LMS6002D::new(interface.clone());
        let si5338 = SI5338::new(interface.clone());
        let dac = DAC161S055::new(interface.clone());

        Ok(Box::new(Self {
            device,
            interface,
            lms,
            si5338,
            dac,
        }))
    }

    pub async fn from_first() -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .next()
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }
    pub async fn from_serial(serial: &str) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.serial_number() == Some(serial))
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    pub async fn from_bus_addr(bus_number: u8, bus_addr: u8) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.busnum() == bus_number && dev.device_address() == bus_addr)
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    pub async fn from_fd(fd: std::os::fd::OwnedFd) -> Result<Box<Self>> {
        let device = Device::from_fd(fd).await?;
        // TODO: Do check on device, if it really is a bladerf
        Self::build(device).await
    }

    pub fn speed(&self) -> Option<Speed> {
        self.device.speed()
    }

    pub async fn serial(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Serial as u8)?)
            .await
    }

    pub async fn manufacturer(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Manufacturer as u8)?)
            .await
    }

    pub async fn fx3_firmware(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Fx3Firmware as u8)?)
            .await
    }

    pub async fn product(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Product as u8)?)
            .await
    }

    async fn config_gpio_read(&self) -> Result<u32> {
        type NiosPkt = NiosPkt8x32;

        let request = NiosPkt::new(NIOS_PKT_8X32_TARGET_CONTROL, NiosPkt::FLAG_READ, 0x0, 0x0);
        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosPkt::from(response).data())
    }

    async fn config_gpio_write(&self, mut data: u32) -> Result<u32> {
        type NiosPkt = NiosPkt8x32;

        // TODO: Speed info should not be determined on every call of gpio_write, but rather at global board_data level.
        let device_speed = self.device.speed().unwrap_or(Speed::Low);
        match device_speed {
            Speed::Super | Speed::SuperPlus => {
                data &= !BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
            _ => {
                data |= BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
        }

        let request = NiosPkt::new(NIOS_PKT_8X32_TARGET_CONTROL, NiosPkt::FLAG_WRITE, 0x0, data);
        let response_vec = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        let response = NiosPkt::from(response_vec);
        Ok(response.data())
    }

    /*
    bladerf1_initialize is wrapped in bladerf1_open
     */
    pub async fn initialize(&self) -> Result<()> {
        self.interface.set_alt_setting(0x01).await?;
        println!("[*] Init - Set Alt Setting to 0x01");

        // Out: 43010000000000000000000000000000
        // In:  43010200000000000000000000000000
        let cfg = self.config_gpio_read().await?;
        if (cfg & 0x7f) == 0 {
            println!("[*] Init - Default GPIO value \"{cfg}\" found - initializing device");
            /* Set the GPIO pins to enable the LMS and select the low band */
            // Out: 43010100005700000000000000000000
            // In:  43010300005700000000000000000000
            self.config_gpio_write(0x57).await?;

            /* Disable the front ends */
            println!("[*] Init - Disabling RX and TX Frontend");
            // Out: 41000000400000000000000000000000
            // In:  41000200400200000000000000000000
            // Out: 41000100400000000000000000000000
            // In:  41000300400000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_TX, false).await?;
            println!("{BLADERF_MODULE_TX}");

            // Out: 41000000700000000000000000000000
            // In:  41000200700200000000000000000000
            // Out: 41000100700000000000000000000000
            // In:  41000300700000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_RX, false).await?;
            println!("{BLADERF_MODULE_RX}");

            /* Set the internal LMS register to enable RX and TX */
            println!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100053e00000000000000000000
            // In:  41000300053e00000000000000000000
            self.lms.write(0x05, 0x3e).await?;

            /* LMS FAQ: Improve TX spurious emission performance */
            println!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100474000000000000000000000
            // In:  41000300474000000000000000000000
            self.lms.write(0x47, 0x40).await?;

            /* LMS FAQ: Improve ADC performance */
            println!("[*] Init - Set register to improve ADC performance");
            // Out: 41000100592900000000000000000000
            // In:  41000300592900000000000000000000
            self.lms.write(0x59, 0x29).await?;

            /* LMS FAQ: Common mode voltage for ADC */
            println!("[*] Init - Set Common mode voltage for ADC");
            // Out: 41000100643600000000000000000000
            // In:  41000300643600000000000000000000
            self.lms.write(0x64, 0x36).await?;

            /* LMS FAQ: Higher LNA Gain */
            println!("[*] Init - Set Higher LNA Gain");
            // Out: 41000100793700000000000000000000
            // In:  41000300793700000000000000000000
            self.lms.write(0x79, 0x37).await?;

            /* Power down DC calibration comparators until they are need, as they
             * have been shown to introduce undesirable artifacts into our signals.
             * (This is documented in the LMS6 FAQ). */

            println!("[*] Init - Power down TX LPF DC cal comparator");
            // Out: 410000003f0000000000000000000000
            // In:  410002003f0000000000000000000000
            // Out: 410001003f8000000000000000000000
            // In:  410003003f8000000000000000000000
            self.lms.set(0x3f, 0x80).await?; /* TX LPF DC cal comparator */

            println!("[*] Init - Power down RX LPF DC cal comparator");
            // Out: 410000005f0000000000000000000000
            // In:  410002005f1f00000000000000000000
            // Out: 410001005f9f00000000000000000000
            // In:  410003005f9f00000000000000000000
            self.lms.set(0x5f, 0x80).await?; /* RX LPF DC cal comparator */

            println!("[*] Init - Power down RXVGA2A/B DC cal comparators");
            // Out: 410000006e0000000000000000000000
            // In:  410002006e0000000000000000000000
            // Out: 410001006ec000000000000000000000
            // In:  410003006ec000000000000000000000
            self.lms.set(0x6e, 0xc0).await?; /* RXVGA2A/B DC cal comparators */

            /* Configure charge pump current offsets */
            println!("[*] Init - Configure TX charge pump current offsets");
            // Out: 41000000160000000000000000000000
            // In:  41000200168c00000000000000000000
            // Out: 41000100160000000000000000000000
            // In:  41000300168c00000000000000000000
            // Out: 41000000170000000000000000000000
            // In:  4100020017e000000000000000000000
            // Out: 4100010017e300000000000000000000
            // In:  4100030017e300000000000000000000
            // Out: 41000000180000000000000000000000
            // In:  41000200184000000000000000000000
            // Out: 41000100184300000000000000000000
            // In:  41000300184300000000000000000000
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_TX).await?;
            println!("[*] Init - Configure RX charge pump current offsets");

            // Out: 41000000260000000000000000000000
            // In:  41000200268c00000000000000000000
            // Out: 41000100260000000000000000000000
            // In:  41000300268c00000000000000000000
            // Out: 41000000270000000000000000000000
            // In:  4100020027e000000000000000000000
            // Out: 4100010027e300000000000000000000
            // In:  4100030027e300000000000000000000
            // Out: 41000000280000000000000000000000
            // In:  41000200284000000000000000000000
            // Out: 41000100184300000000000000000000
            // In:  41000300284300000000000000000000
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_RX).await?;

            println!("[*] Init - Set TX Samplerate");
            // Out: 41010000260000000000000000000000
            // In:  41010200260000000000000000000000
            // Out: 41010100260300000000000000000000
            // In:  41010300260300000000000000000000
            // Out: 410101004b6600000000000000000000
            // In:  410103004b6600000000000000000000
            // Out: 410101004c9c00000000000000000000
            // In:  410103004c9c00000000000000000000
            // Out: 410101004d0800000000000000000000
            // In:  410103004d0800000000000000000000
            // Out: 410101004e0000000000000000000000
            // In:  410103004e0000000000000000000000
            // Out: 410101004f0000000000000000000000
            // In:  410103004f0000000000000000000000
            // Out: 41010100500000000000000000000000
            // In:  41010300500000000000000000000000
            // Out: 41010100510500000000000000000000
            // In:  41010300510500000000000000000000
            // Out: 41010100520000000000000000000000
            // In:  41010300520000000000000000000000
            // Out: 41010100530000000000000000000000
            // In:  41010300530000000000000000000000
            // Out: 41010100540000000000000000000000
            // In:  41010300540000000000000000000000
            // Out: 4101010021c800000000000000000000
            // In : 4101030021c800000000000000000000
            let _actual_tx = self
                .si5338
                .set_sample_rate(bladerf_channel_tx!(0), 1000000)
                .await?;

            println!("[*] Init - Set RX Samplerate");
            // Out: As above but slightly different (Matches original packets)
            // In:  As above but slightly different (Matches original packets)
            let _actual_rx = self
                .si5338
                .set_sample_rate(bladerf_channel_rx!(0), 1000000)
                .await?;

            // SI5338 Packet: Magic: 0x54, 8x 0xff, Channel (int), 4Byte Frequency
            // With TX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x40, 0x0, 0x0};
            // With RX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x80, 0x0, 0x0};
            // Basically  nios_si5338_read == nios 8x8 read

            // board_data->tuning_mode = tuning_get_default_mode(dev);

            println!("self.set_frequency(bladerf_channel_tx!(0), 2447000000)?;");
            // Out: 5400000000000000003fb95555ac1f00
            // In:  5400000000000000001e030000000000
            self.set_frequency(bladerf_channel_tx!(0), 2447000000)
                .await?;

            println!("self.set_frequency(bladerf_channel_rx!(0), 2484000000)?;");
            // Out: 54000000000000000040b000006c2300
            // In:  54000000000000000021030000000000
            self.set_frequency(bladerf_channel_rx!(0), 2484000000)
                .await?;

            // /* Set the calibrated VCTCXO DAC value */
            // TODO: board_data.dac_trim instead of 0
            // Out: 42000100280000000000000000000000
            // In:  42000300280000000000000000000000
            self.dac.write(0).await?;

            // status = dac161s055_write(dev, board_data->dac_trim);
            // if (status != 0) {
            //     return status;
            // }

            // /* Set the default gain mode */
            // Out expected: 4200010008d1ab000000000000000000
            // Out actual:   42000100080000000000000000000000
            // In: expected: 4200030008d1ab000000000000000000
            // In actual:    42000300080000000000000000000000
            // Todo: Implement AGC table and set mode to BladerfGainDefault
            self.set_gain_mode(bladerf_channel_rx!(0), BladerfGainMode::Mgc)
                .await?;
        } else {
            println!("[*] Init - Device already initialized: {cfg:#04x}");
            //board_data->tuning_mode = tuning_get_default_mode(dev);
        }

        // /* Check if we have an expansion board attached */
        // status = dev->board->expansion_get_attached(dev, &dev->xb);
        // if (status != 0) {
        //     return status;
        // }
        //
        // /* Update device state */
        // board_data->state = STATE_INITIALIZED;
        //
        // /* Set up LMS DC offset register calibration and initial IQ settings,
        //  * if any tables have been loaded already.
        //  *
        //  * This is done every time the device is opened (with an FPGA loaded),
        //  * as the user may change/update DC calibration tables without reloading the
        //  * FPGA.
        //  */
        // status = bladerf1_apply_lms_dc_cals(dev);
        // if (status != 0) {
        //     return status;
        // }

        Ok(())
    }

    pub async fn bladerf_enable_module(&self, module: u8, enable: bool) -> Result<u8> {
        self.lms.enable_rffe(module, enable).await
    }

    // Todo: Implement band select for set_frequency
    pub async fn band_select(&self, module: u8, band: Band) -> Result<u32> {
        //const uint32_t band = low_band ? 2 : 1;
        let band_value = match band {
            Band::Low => 2,
            Band::High => 1,
        };

        println!("Selecting %s band. {band:?}");

        self.lms.select_band(module, band).await?;

        let mut gpio = self.config_gpio_read().await?;

        let shift = if module == BLADERF_MODULE_TX {
            3 << 3
        } else {
            3 << 5
        };
        gpio &= !shift;

        let shift = if module == BLADERF_MODULE_TX {
            band_value << 3
        } else {
            band_value << 5
        };
        gpio |= !shift;

        self.config_gpio_write(gpio).await
    }

    pub async fn set_frequency(&self, channel: u8, frequency: u64) -> Result<()> {
        //let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        println!("Setting Frequency on channel {channel} to {frequency}Hz");

        // Ommit XB200 settings here

        // TODO: The tuning mode should be read from the board config
        // In the packet captures, this is where the changes happen:
        // -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
        // -  Packet No. 230 in rx-rusttool-filtered.pcapng
        // This is maybe due to the tuning mode being FPGA and not Host
        enum TuningMode {
            Host,
            Fpga,
        }
        let mode = TuningMode::Fpga;
        // For tuning HOST Tuning Mode:
        match mode {
            TuningMode::Host => {
                self.lms.set_frequency(channel, frequency as u32).await?;
                let band = if frequency < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.band_select(channel, band).await?;
            }
            TuningMode::Fpga => {
                self.lms
                    .schedule_retune(
                        channel,
                        NiosPktRetuneRequest::RETUNE_NOW,
                        frequency as u32,
                        None,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    fn _convert_gain_to_lna_gain(gain: i8) -> BladerfLnaGain {
        if gain >= BLADERF_LNA_GAIN_MAX_DB {
            BladerfLnaGain::Max
        } else if gain >= BLADERF_LNA_GAIN_MID_DB {
            BladerfLnaGain::Mid
        } else {
            BladerfLnaGain::Bypass
        }
    }

    fn _convert_lna_gain_to_gain(lna_gain: BladerfLnaGain) -> i8 {
        match lna_gain {
            BladerfLnaGain::Max => BLADERF_LNA_GAIN_MAX_DB,
            BladerfLnaGain::Mid => BLADERF_LNA_GAIN_MID_DB,
            BladerfLnaGain::Bypass => 0,
            _ => -1,
        }
    }

    pub fn get_gain_range(&self, channel: u8) -> SdrRange {
        if bladerf_channel_is_tx!(channel) {
            /* Overall TX gain range */
            SdrRange {
                min: BLADERF_TXVGA1_GAIN_MIN
                    + BLADERF_TXVGA2_GAIN_MIN
                    + BLADERF1_TX_GAIN_OFFSET.round() as i8,
                max: BLADERF_TXVGA1_GAIN_MAX
                    + BLADERF_TXVGA2_GAIN_MAX
                    + BLADERF1_TX_GAIN_OFFSET.round() as i8,
                step: 1,
                scale: 1,
            }
            // *range = &bladerf1_tx_gain_range;
        } else {
            /* Overall RX gain range */
            SdrRange {
                min: BLADERF_RXVGA1_GAIN_MIN
                    + BLADERF_RXVGA2_GAIN_MIN
                    + BLADERF1_RX_GAIN_OFFSET.round() as i8,
                max: BLADERF_LNA_GAIN_MAX_DB
                    + BLADERF_RXVGA1_GAIN_MAX
                    + BLADERF_RXVGA2_GAIN_MAX
                    + BLADERF1_RX_GAIN_OFFSET.round() as i8,
                step: 1,
                scale: 1,
            }
            // *range = &bladerf1_rx_gain_range;
        }
    }

    pub async fn get_gain_stage(&self, channel: u8, stage: &str) -> Result<i8> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        if channel == BLADERF_MODULE_TX {
            match stage {
                "txvga1" => self.lms.txvga1_get_gain().await,
                "txvga2" => self.lms.txvga2_get_gain().await,
                _ => Err(anyhow!("invalid stage {stage}")),
            }
        } else if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => {
                    let lna_gain = self.lms.lna_get_gain().await?;
                    Ok(Self::_convert_lna_gain_to_gain(lna_gain))
                }
                "rxvga1" => self.lms.rxvga1_get_gain().await,
                "rxvga2" => self.lms.rxvga2_get_gain().await,
                _ => Err(anyhow!("invalid stage {stage}")),
            }
        } else {
            Err(anyhow!("invalid channel {channel}"))
        }
    }

    pub async fn set_gain_stage(&self, channel: u8, stage: &str, gain: i8) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        /* TODO implement gain clamping */
        match channel {
            BLADERF_MODULE_TX => match stage {
                "txvga1" => Ok(self.lms.txvga1_set_gain(gain).await?),
                "txvga2" => Ok(self.lms.txvga2_set_gain(gain).await?),
                _ => Err(anyhow!("invalid stage {stage}")),
            },
            BLADERF_MODULE_RX => match stage {
                "rxvga1" => Ok(self.lms.rxvga1_set_gain(gain).await?),
                "rxvga2" => Ok(self.lms.rxvga2_set_gain(gain).await?),
                "lna" => Ok(self
                    .lms
                    .lna_set_gain(Self::_convert_gain_to_lna_gain(gain))
                    .await?),
                _ => Err(anyhow!("invalid stage {stage}")),
            },
            _ => Err(anyhow!("Invalid channel {channel}")),
        }
    }

    pub fn get_gain_stages(channel: u8) -> Vec<String> {
        if bladerf_channel_is_tx!(channel) {
            vec!["txvga1".to_string(), "txvga2".to_string()]
        } else {
            vec![
                "lna".to_string(),
                "rxvga1".to_string(),
                "rxvga2".to_string(),
            ]
        }
    }

    /** Use bladerf_get_gain_range(), bladerf_set_gain(), and
     *             bladerf_get_gain() to control total system gain. For direct
     *             control of individual gain stages, use bladerf_get_gain_stages(),
     *             bladerf_get_gain_stage_range(), bladerf_set_gain_stage(), and
     *             bladerf_get_gain_stage().
     **/
    pub fn get_gain_stage_range(channel: u8, stage: &str) -> Result<SdrRange> {
        if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => Ok(SdrRange {
                    min: 0,
                    max: BLADERF_LNA_GAIN_MAX_DB,
                    step: 3,
                    scale: 1,
                }),
                "rxvga1" => Ok(SdrRange {
                    min: BLADERF_RXVGA1_GAIN_MIN,
                    max: BLADERF_RXVGA1_GAIN_MAX,
                    step: 1,
                    scale: 1,
                }),
                "rxvga2" => Ok(SdrRange {
                    min: BLADERF_RXVGA2_GAIN_MIN,
                    max: BLADERF_RXVGA2_GAIN_MAX,
                    step: 3,
                    scale: 1,
                }),
                _ => Err(anyhow!("Invalid stage: {stage}")),
            }
        } else {
            match stage {
                "txvga1" => Ok(SdrRange {
                    min: BLADERF_TXVGA1_GAIN_MIN,
                    max: BLADERF_TXVGA1_GAIN_MAX,
                    step: 1,
                    scale: 1,
                }),
                "txvga2" => Ok(SdrRange {
                    min: BLADERF_TXVGA2_GAIN_MIN,
                    max: BLADERF_TXVGA2_GAIN_MAX,
                    step: 3,
                    scale: 1,
                }),
                _ => Err(anyhow!("Invalid stage: {stage}")),
            }
        }
    }

    pub async fn get_tx_gain(&self) -> Result<i8> {
        let txvga1 = self.lms.txvga1_get_gain().await?;
        let txvga2 = self.lms.txvga2_get_gain().await?;

        Ok(txvga1 + txvga2 + BLADERF1_TX_GAIN_OFFSET.round() as i8)
    }

    pub async fn get_rx_gain(&self) -> Result<i8> {
        let lna_gain = self.lms.lna_get_gain().await?;
        let rxvga1_gain = self.lms.rxvga1_get_gain().await?;
        let rxvga2_gain = self.lms.rxvga2_get_gain().await?;

        let lna_gain_db = match lna_gain {
            // BladerfLnaGain::Bypass => 0,
            BladerfLnaGain::Mid => BLADERF_LNA_GAIN_MID_DB,
            BladerfLnaGain::Max => BLADERF_LNA_GAIN_MAX_DB,
            _ => 0,
        };

        Ok(lna_gain_db + rxvga1_gain + rxvga2_gain + BLADERF1_RX_GAIN_OFFSET.round() as i8)
    }

    pub async fn get_gain(&self, channel: u8) -> Result<i8> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.get_tx_gain().await
        } else {
            self.get_rx_gain().await
        }
    }

    pub async fn set_gain(&self, channel: u8, gain: i8) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.set_tx_gain(gain).await
        } else {
            self.set_rx_gain(gain).await
        }
    }

    pub async fn set_tx_gain(&self, mut gain: i8) -> Result<()> {
        let orig_gain = gain;

        let txvga1_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga1")?;
        let txvga2_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga2")?;

        let mut txvga1 = bladerf1::__unscale_int(&txvga1_range, txvga1_range.min as f32);
        let mut txvga2 = bladerf1::__unscale_int(&txvga2_range, txvga2_range.min as f32);

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_TX_GAIN_OFFSET.round() as i8 + txvga1 + txvga2;

        // apportion gain to TXVGA2
        (txvga2, gain) = _apportion_gain(&txvga2_range, txvga2, gain);

        // apportion gain to TXVGA1
        (txvga1, gain) = _apportion_gain(&txvga1_range, txvga1, gain);

        // verification
        if gain != 0 {
            println!(
                "unable to achieve requested gain {} (missed by {})\n",
                orig_gain, gain
            );
            println!(
                "gain={} -> txvga2={} txvga1={} remainder={}\n",
                orig_gain, txvga2, txvga1, gain
            );
        }

        self.lms.txvga1_set_gain(txvga1).await?;
        self.lms.txvga2_set_gain(txvga2).await?;
        Ok(())
    }

    pub async fn set_rx_gain(&self, mut gain: i8) -> Result<()> {
        let orig_gain = gain;

        let lna_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "lna")?;
        let rxvga1_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "rxvga1")?;
        let rxvga2_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "rxvga2")?;

        let mut lna = __unscale_int(&lna_range, lna_range.min as f32);
        let mut rxvga1 = __unscale_int(&rxvga1_range, rxvga1_range.min as f32);
        let mut rxvga2 = __unscale_int(&rxvga2_range, rxvga2_range.min as f32);

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_RX_GAIN_OFFSET.round() as i8 + lna + rxvga1 + rxvga2;

        // apportion some gain to RXLNA (but only half of it for now)
        (lna, gain) = _apportion_gain(&lna_range, lna, gain);
        if lna > BLADERF_LNA_GAIN_MID_DB {
            gain += lna - BLADERF_LNA_GAIN_MID_DB;
            lna -= lna - BLADERF_LNA_GAIN_MID_DB;
        }

        // apportion gain to RXVGA1
        (rxvga1, gain) = _apportion_gain(&rxvga1_range, rxvga1, gain);

        // apportion more gain to RXLNA
        (lna, gain) = _apportion_gain(&lna_range, lna, gain);

        // apportion gain to RXVGA2
        (rxvga2, gain) = _apportion_gain(&rxvga2_range, rxvga2, gain);

        // if we still have remaining gain, it's because rxvga2 has a step size of
        // 3 dB. Steal a few dB from rxvga1...
        if gain > 0 && rxvga1 >= __unscale_int(&rxvga1_range, rxvga1_range.max as f32) {
            rxvga1 -= __unscale_int(&rxvga2_range, rxvga2_range.step as f32);
            gain += __unscale_int(&rxvga2_range, rxvga2_range.step as f32);

            (rxvga2, gain) = _apportion_gain(&rxvga2_range, rxvga2, gain);
            (rxvga1, gain) = _apportion_gain(&rxvga1_range, rxvga1, gain);
        }

        // verification
        if gain != 0 {
            println!(
                "unable to achieve requested gain {} (missed by {})\n",
                orig_gain, gain
            );
            println!(
                "gain={} -> 1xvga1={} lna={} rxvga2={} remainder={}\n",
                orig_gain, rxvga1, lna, rxvga2, gain
            );
        }

        // that should do it. actually apply the changes:
        self.lms
            .lna_set_gain(Self::_convert_gain_to_lna_gain(lna))
            .await?;
        self.lms
            .rxvga1_set_gain(__scale_int(&rxvga1_range, rxvga1 as f32))
            .await?;
        self.lms
            .rxvga2_set_gain(__scale_int(&rxvga2_range, rxvga2 as f32))
            .await?;

        Ok(())
    }

    #[allow(unreachable_code)] // TODO: Only while AGC table is not implemented
    pub async fn set_gain_mode(&self, channel: u8, mode: BladerfGainMode) -> Result<u32> {
        if channel != BLADERF_MODULE_RX {
            return Err(anyhow!("Operation only supported on RX channel"));
        }

        let mut config_gpio = self.config_gpio_read().await?;
        if mode == BladerfGainMode::Default {
            // Default mode is the same as Automatic mode
            return Err(anyhow!("Todo: Implement AGC Table"));
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_AGC_DC_LUT)) {
            //     log_warning("AGC not supported by FPGA. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s, then %s\n", FPGA_STR, DCCAL_STR);
            //     log_debug("%s: expected FPGA >= v0.7.0, got v%u.%u.%u\n",
            //               __FUNCTION__, board_data->fpga_version.major,
            //               board_data->fpga_version.minor,
            //               board_data->fpga_version.patch);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (!board_data->cal.dc_rx) {
            //     log_warning("RX DC calibration table not found. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (board_data->cal.dc_rx->version != TABLE_VERSION) {
            //     log_warning("RX DC calibration table is out-of-date. %s\n",
            //                 MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     log_debug("%s: expected version %u, got %u\n", __FUNCTION__,
            //               TABLE_VERSION, board_data->cal.dc_rx->version);
            //
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == BladerfGainMode::Mgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        self.config_gpio_write(config_gpio).await
    }

    pub async fn get_frequency(&self, channel: u8) -> Result<u32> {
        let f = self.lms.get_frequency(channel).await?;
        if f.x == 0 {
            /* If we see this, it's most often an indication that communication
             * with the LMS6002D is not occuring correctly */
            return Err(anyhow!("LMSFreq.x was zero!"));
        }
        let frequency_hz = LMS6002D::frequency_to_hz(&f);

        // if (dev->xb == BLADERF_XB_200) {
        //     status = xb200_get_path(dev, ch, &path);
        //     if (status != 0) {
        //         return status;
        //     }
        //     if (path == BLADERF_XB200_MIX) {
        //         *frequency = 1248000000 - *frequency;
        //     }
        // }

        Ok(frequency_hz)
    }

    pub fn get_frequency_range() -> Range<u32> {
        BLADERF_FREQUENCY_MIN..BLADERF_FREQUENCY_MAX
        // if (dev->xb == BLADERF_XB_200) {
        //     *range = &bladerf1_xb200_frequency_range;
        //     0.. BLADERF_FREQUENCY_MAX
        // } else {
        //     *range = &bladerf1_frequency_range;
        //     BLADERF_FREQUENCY_MIN.. BLADERF_FREQUENCY_MAX
        // }
    }

    /// Get BladeRf1 String descriptor
    pub async fn get_string_descriptor(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .device
            .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))
            .await?;
        Ok(descriptor)
    }

    /// Get BladeRf1 Serial number
    pub async fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>> {
        let descriptor = self
            .device
            .get_descriptor(
                DescriptorTypes::Configuration as u8,
                descriptor_index,
                0x00,
                Duration::from_secs(1),
            )
            .await?;
        Ok(descriptor)
    }

    pub async fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .device
            .get_string_descriptor_supported_languages(Duration::from_secs(1))
            .await?
            .collect();

        Ok(languages)
    }

    pub fn get_configurations(&self) -> Vec<ConfigurationDescriptor> {
        self.device.configurations().collect()
    }

    pub async fn set_configuration(&self, configuration: u16) -> Result<()> {
        //self.device.set_configuration(configuration)?;
        Ok(self
            .interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Standard,
                    recipient: Recipient::Device,
                    request: 0x09, //Request::VersionStringRead as u8,
                    value: configuration,
                    index: 0x00,
                    data: &[],
                },
                TIMEOUT,
            )
            .await?)
    }

    /**
     * Perform the neccessary device configuration for the specified format
     * (e.g., enabling/disabling timestamp support), first checking that the
     * requested format would not conflict with the other stream direction.
     *
     * @param           dev     Device handle
     * @param[in]       dir     Direction that is currently being configured
     * @param[in]       format  Format the channel is being configured for
     *
     * @return 0 on success, BLADERF_ERR_* on failure
     */
    pub async fn perform_format_config(
        &self,
        dir: BladeRfDirection,
        format: BladerfFormat,
    ) -> Result<()> {
        // BladerfFormatPacketMeta
        //struct bladerf1_board_data *board_data = dev->board_data;

        //int status = 0;
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;

        // status = requires_timestamps(format, &use_timestamps);
        // if (status != 0) {
        //     log_debug("%s: Invalid format: %d\n", __FUNCTION__, format);
        //     return status;
        // }

        let _other = match dir {
            BladeRfDirection::Rx => BladeRfDirection::Tx,
            BladeRfDirection::Tx => BladeRfDirection::Rx,
        };

        // status = requires_timestamps(board_data->module_format[other],
        //     &other_using_timestamps);

        // if ((status == 0) && (other_using_timestamps != use_timestamps)) {
        //     log_debug("Format conflict detected: RX=%d, TX=%d\n");
        //     return BLADERF_ERR_INVAL;
        // }

        let mut gpio_val = self.config_gpio_read().await?;

        println!("gpio_val {gpio_val:#08x}");
        if format == BladerfFormat::PacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            use_timestamps = true;
            println!("BladerfFormat::PacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
            println!("else");
        }
        println!("gpio_val {gpio_val:#08x}");

        if use_timestamps {
            println!("use_timestamps");
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            println!("dont use_timestamps");
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }

        println!("gpio_val {gpio_val:#08x}");

        self.config_gpio_write(gpio_val).await?;
        // if (status == 0) {
        //     board_data->module_format[dir] = format;
        // }

        //return status;
        Ok(())
    }

    /**
     * Deconfigure and update any state pertaining what a format that a stream
     * direction is no longer using.
     *
     * @param       dev     Device handle
     * @param[in]   dir     Direction that is currently being deconfigured
     *
     * @return 0 on success, BLADERF_ERR_* on failure
     */
    pub fn perform_format_deconfig(&self, dir: BladeRfDirection) -> Result<()> {
        //struct bladerf1_board_data *board_data = dev->board_data;

        match dir {
            BladeRfDirection::Rx | BladeRfDirection::Tx => {
                /* We'll reconfigure the HW when we call perform_format_config, so
                 * we just need to update our stored information */
                //board_data -> module_format[dir] = - 1;
            }
        }

        Ok(())
    }

    pub async fn experimental_control_urb(&self) -> Result<()> {
        // TODO: Dont know what this is doing
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: 0x4,
            value: 0x1,
            index: 0,
            length: 0x4,
        };
        let vec = self
            .interface
            .control_in(pkt, Duration::from_secs(5))
            .await?;
        println!("Control Response Data: {vec:?}");
        Ok(())
    }

    pub async fn async_run_stream(&self) -> Result<()> {
        // TODO: In_ENDPOINT is 0x81 here, not 0x82
        let mut ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;

        let n_transfers = 8;
        let factor = 32;
        // let factor = match self.device.speed().unwrap_or(Speed::Low) {
        //     // TODO: These numbers are completely made up.
        //     // TODO: They should be based on real USB Frame sizes depending on the given Speed
        //     Speed::Low => 8,
        //     Speed::Full => 16,
        //     Speed::High => 32,
        //     Speed::Super => 32, // This factor is used by the original libusb libbladerf implementation.
        //     Speed::SuperPlus => 96,
        //     _ => 8,
        // };

        let max_packet_size = ep_bulk_in.max_packet_size();
        let max_frame_size = max_packet_size * factor;
        println!("Max Packet Size: {max_packet_size}");

        for _i in 0..n_transfers {
            let buffer = ep_bulk_in.allocate(max_frame_size);
            ep_bulk_in.submit(buffer);
            // println!("submitted_transfers: {i}");
        }

        loop {
            let result = ep_bulk_in.next_complete().await;
            // println!("{result:?}");
            if result.status.is_err() {
                break;
            }
            ep_bulk_in.submit(result.buffer);
        }
        Ok(())
    }

    // pub async fn bladerf1_stream(&self, stream: &bladerf_stream, layout: BladeRfChannelLayout) -> Result<()> {
    //     let dir: BladeRfDirection = layout & BLADERF_DIRECTION_MASK;
    //     let stream_status: i32;
    //
    //     // if layout != BladeRfChannelLayout::BladerfRxX1 && layout != BladeRfChannelLayout::BladerfTxX1 {
    //     //     return Err(anyhow!("Invalid ChannelLayout"));
    //     // }
    //
    //     self.perform_format_config(dir, stream->format)?;
    //
    //     stream_status = self.async_run_stream(stream, layout).await;
    //     // TODO: static void LIBUSB_CALL lusb_stream_cb
    //
    //     self.perform_format_deconfig(dir)?;
    // }

    pub async fn reset(&self) -> Result<()> {
        //self.check_api_version(UsbVersion::from_bcd(0x0102))?;
        //self.write_control(Request::Reset, 0, 0, &[])?;
        self.device.set_configuration(0).await?;

        Ok(())
    }
}
