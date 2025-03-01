#![allow(private_interfaces)]

use std::cmp::PartialEq;
use std::time::Duration;
//use crate::backend::nusb::NusbBackend;
//use crate::backend::rusb::RusbBackend;
//use crate::backend::UsbBackend;
use anyhow::{anyhow, Result};
use futures_lite::future::block_on;
use nusb::descriptors::Configuration;
use nusb::transfer::{ControlOut, ControlType, Recipient};
use nusb::{Device, Interface};

use crate::bladerf::BladerfGainMode::{BladerfGainDefault, BladerfGainMgc};
use crate::bladerf::{
    BladeRf, BladerfGainMode, DescriptorTypes, BLADERF_MODULE_RX, BLADERF_MODULE_TX,
};
use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use crate::nios::constants::{
    NIOS_PKT_8X32_TARGET_CONTROL, NIOS_PKT_FLAG_READ, NIOS_PKT_FLAG_WRITE,
};
use crate::nios::packet8x32::NiosPacket8x32;
use crate::nios::Nios;
use crate::usb::UsbBackend;
use crate::{bladerf_channel_rx, bladerf_channel_tx};

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
}

/**
 * Enable LMS receive
 *
 * @note This bit is set/cleared by bladerf_enable_module()
 */
const BLADERF_GPIO_LMS_RX_ENABLE: u8 = 1 << 1;

/**
 * Enable LMS transmit
 *
 * @note This bit is set/cleared by bladerf_enable_module()
 */
const BLADERF_GPIO_LMS_TX_ENABLE: u8 = 1 << 2;

/**
 * Switch to use TX low band (300MHz - 1.5GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_TX_LB_ENABLE: u8 = 2 << 3;

/**
 * Switch to use TX high band (1.5GHz - 3.8GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_TX_HB_ENABLE: u8 = 1 << 3;

/**
 * Counter mode enable
 *
 * Setting this bit to 1 instructs the FPGA to replace the (I, Q) pair in sample
 * data with an incrementing, little-endian, 32-bit counter value. A 0 in bit
 * specifies that sample data should be sent (as normally done).
 *
 * This feature is useful when debugging issues involving dropped samples.
 */
const BLADERF_GPIO_COUNTER_ENABLE: u16 = 1 << 9;

/**
 * Bit mask representing the rx mux selection
 *
 * @note These bits are set using bladerf_set_rx_mux()
 */
const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;

/**
 * Starting bit index of the RX mux values in FX3 <-> FPGA GPIO bank
 */
const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;

/**
 * Switch to use RX low band (300M - 1.5GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_RX_LB_ENABLE: u16 = 2 << 5;

/**
 * Switch to use RX high band (1.5GHz - 3.8GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_RX_HB_ENABLE: u16 = 1 << 5;

/**
 * This GPIO bit configures the FPGA to use smaller DMA transfers (256 cycles
 * instead of 512). This is required when the device is not connected at Super
 * Speed (i.e., when it is connected at High Speed).
 *
 * However, the caller need not set this in bladerf_config_gpio_write() calls.
 * The library will set this as needed; callers generally do not need to be
 * concerned with setting/clearing this bit.
 */
const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;

/**
 * Enable Packet mode
 */
const BLADERF_GPIO_PACKET: u32 = 1 << 19;

/**
 * Enable 8bit sample mode
 */
const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;

/**
 * AGC enable control bit
 *
 * @note This is set using bladerf_set_gain_mode().
 */
const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

/**
 * Enable-bit for timestamp counter in the FPGA
 */
const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;

/**
 * Timestamp 2x divider control.
 *
 * @note <b>Important</b>: This bit has no effect and is always enabled (1) in
 * FPGA versions >= v0.3.0.
 *
 * @note The remainder of the description of this bit is presented here for
 * historical purposes only. It is only relevant to FPGA versions <= v0.1.2.
 *
 * By default, (value = 0), the sample counter is incremented with I and Q,
 * yielding two counts per sample.
 *
 * Set this bit to 1 to enable a 2x timestamp divider, effectively achieving 1
 * timestamp count per sample.
 * */
const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;

/**
 * Packet capable core present bit.
 *
 * @note This is a read-only bit. The FPGA sets its value, and uses it to inform
 *  host that there is a core capable of using packets in the FPGA.
 */
const BLADERF_GPIO_PACKET_CORE_PRESENT: u32 = 1 << 28;

pub const BLADERF_SAMPLERATE_MIN: u64 = 80000;

/** Minimum tunable frequency (without an XB-200 attached), in Hz
*
* \deprecated Use bladerf_get_frequency_range()
 */
pub const BLADERF_FREQUENCY_MIN: u32 = 237500000;

/** Maximum tunable frequency, in Hz
*
* \deprecated Use bladerf_get_frequency_range()
 */
pub const BLADERF_FREQUENCY_MAX: u32 = 3800000000;

/**
 * Maximum output frequency on SMB connector, if no expansion board attached.
 */
pub const BLADERF_SMB_FREQUENCY_MAX: u32 =  200000000;

/**
 * Minimum output frequency on SMB connector, if no expansion board attached.
 */
pub const BLADERF_SMB_FREQUENCY_MIN: u32 =  (38400000 * 66) / (32 * 567);

/**
 * LNA gain options
 *
 * \deprecated Use bladerf_get_gain_stage_range()
 */
pub enum BladerfLnaGain {
    /**< Invalid LNA gain */
    BladerfLnaGainUnknown,
    /**< LNA bypassed - 0dB gain */
    BladerfLnaGainBypass,
    /**< LNA Mid Gain (MAX-6dB) */
    BladerfLnaGainMid,
    /**< LNA Max Gain */
    BladerfLnaGainMax,
}

/// BladeRF1 USB vendor ID.
pub const BLADERF1_USB_VID: u16 = 0x2CF0;
/// BladeRF1 USB product ID.
pub const BLADERF1_USB_PID: u16 = 0x5246;

pub struct BladeRf1 {
    #[allow(dead_code)]
    device: Device,
    #[allow(dead_code)]
    pub interface: Interface,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    //xb200: Option<XB200>,
}
// We use the Builder pattern together with the type-state pattern here to model the flow of creating a BladeRf1 instance.
// See for example: https://cliffle.com/blog/rust-typestate/
impl BladeRf1 {
    pub fn builder() -> BladeRf1Builder<Initial> {
        BladeRf1Builder {
            data: Initial {
                backend: UsbBackend {},
            },
        }
    }

    fn config_gpio_read(&self) -> Result<u32> {
        const ENDPOINT_OUT: u8 = 0x02;
        const ENDPOINT_IN: u8 = 0x82;

        let mut request = NiosPacket8x32::new();
        request.set(NIOS_PKT_8X32_TARGET_CONTROL, NIOS_PKT_FLAG_READ, 0x0, 0x0);
        let response = self
            .interface
            .nios_send(ENDPOINT_IN, ENDPOINT_OUT, request.into_vec())?;
        Ok(NiosPacket8x32::reuse(response).data())
    }

    fn config_gpio_write(&self, mut data: u32) -> Result<()> {
        const ENDPOINT_OUT: u8 = 0x02;
        const ENDPOINT_IN: u8 = 0x82;

        enum DeviceSpeed {
            BladerfDeviceSpeedUnknown,
            BladerfDeviceSpeedHigh,
            BladerfDeviceSpeedSuper,
        }

        // TODO: Get usb speed dynamically
        let device_speed: DeviceSpeed = DeviceSpeed::BladerfDeviceSpeedSuper;
        match device_speed {
            DeviceSpeed::BladerfDeviceSpeedUnknown => {
                println!("DeviceSpeed::BladerfDeviceSpeedUnknown");
            }
            DeviceSpeed::BladerfDeviceSpeedHigh => {
                println!("DeviceSpeed::BladerfDeviceSpeedUnknown");
                data = data | (BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32);
            }
            DeviceSpeed::BladerfDeviceSpeedSuper => {
                println!("DeviceSpeed::BladerfDeviceSpeedUnknown");
                data = data & (!BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32);
            }
        }

        let mut request = NiosPacket8x32::new();
        request.set(NIOS_PKT_8X32_TARGET_CONTROL, NIOS_PKT_FLAG_WRITE, 0x0, data);
        let _response = self
            .interface
            .nios_send(ENDPOINT_IN, ENDPOINT_OUT, request.into_vec())?;
        Ok(())
    }

    /*
    bladerf1_initialize is wrapped in bladerf1_open
     */
    pub fn initialize(&self) -> Result<()> {
        self.interface.set_alt_setting(0x01)?;
        println!("[*] Init - Set Alt Setting to 0x01");

        let cfg = self.config_gpio_read()?;
        if (cfg & 0x7f) == 0 {
            println!("[*] Init - Default GPIO value \"{cfg}\" found - initializing device");
            /* Set the GPIO pins to enable the LMS and select the low band */
            self.config_gpio_write(0x57)?;

            /* Disable the front ends */
            println!("[*] Init - Disabling RX and TX Frontend");
            self.lms.enable_rffe(BLADERF_MODULE_TX, false)?;
            self.lms.enable_rffe(BLADERF_MODULE_RX, false)?;

            /* Set the internal LMS register to enable RX and TX */
            println!("[*] Init - Set LMS register to enable RX and TX");
            self.lms.write(0x05, 0x3e)?;

            /* LMS FAQ: Improve TX spurious emission performance */
            println!("[*] Init - Set LMS register to enable RX and TX");
            self.lms.write(0x47, 0x40)?;

            /* LMS FAQ: Improve ADC performance */
            println!("[*] Init - Set register to improve ADC performance");
            self.lms.write(0x59, 0x29)?;

            /* LMS FAQ: Common mode voltage for ADC */
            println!("[*] Init - Set Common mode voltage for ADC");
            self.lms.write(0x64, 0x36)?;

            /* LMS FAQ: Higher LNA Gain */
            println!("[*] Init - Set Higher LNA Gain");
            self.lms.write(0x79, 0x37)?;

            /* Power down DC calibration comparators until they are need, as they
             * have been shown to introduce undesirable artifacts into our signals.
             * (This is documented in the LMS6 FAQ). */

            println!("[*] Init - Power down TX LPF DC cal comparator");
            self.lms.set(0x3f, 0x80)?; /* TX LPF DC cal comparator */

            println!("[*] Init - Power down RX LPF DC cal comparator");
            self.lms.set(0x5f, 0x80)?; /* RX LPF DC cal comparator */

            println!("[*] Init - Power down RXVGA2A/B DC cal comparators");
            self.lms.set(0x6e, 0xc0)?; /* RXVGA2A/B DC cal comparators */

            /* Configure charge pump current offsets */
            println!("[*] Init - Configure TX charge pump current offsets");
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_TX)?;
            println!("[*] Init - Configure RX charge pump current offsets");
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_RX)?;

            // SI5338 Packet: Magic: 0x54, 8x 0xff, Channel (int), 4Byte Frequency
            // With TX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x40, 0x0, 0x0};
            // With RX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x80, 0x0, 0x0};
            // Basically  nios_si5338_read == nios 8x8 read

            /* Set a default samplerate */

            // BUG: Actual:
            // BulkOut: [41, 1, 1, 0, 0, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            // BulkIn:  [41, 1, 3, 0, 0, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            // Should be:
            // BulkOut: [41, 1, 1, 0, 4b, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            // BulkIn:  [41, 1, 3, 0, 4b, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            // Seems like ms.base in line 127 of si5338.rs is 0 instead of 0x4b

            println!("[*] Init - Set TX Samplerate");
            let _actual_tx = self
                .si5338
                .set_sample_rate(bladerf_channel_tx!(0), 1000000)?;

            println!("[*] Init - Set RX Samplerate");
            let _actual_rx = self
                .si5338
                .set_sample_rate(bladerf_channel_rx!(0), 1000000)?;

            //board_data->tuning_mode = tuning_get_default_mode(dev);

            self.set_frequency(bladerf_channel_tx!(0), 2447000000)?;

            self.set_frequency(bladerf_channel_rx!(0), 2484000000)?;
            // status = dev->board->set_frequency(dev, BLADERF_CHANNEL_RX(0), 2484000000U);
            // if (status != 0) {
            //     return status;
            // }

            // /* Set the calibrated VCTCXO DAC value */
            // TODO: board_data.dac_trim instead of 0
            self.dac.write(0)?;

            // status = dac161s055_write(dev, board_data->dac_trim);
            // if (status != 0) {
            //     return status;
            // }

            // /* Set the default gain mode */
            self.set_gain_mode(bladerf_channel_rx!(0), BladerfGainDefault)?;
        } else {
            println!("[*] Init - Device already initialized: {:#04x}", cfg);
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

    pub fn set_frequency(&self, channel: u8, frequency: u64) -> Result<()> {
        //let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        println!(
            "Setting Frequency on channel {} to {}Hz",
            channel, frequency
        );

        // Ommit XB200 settings here

        self.lms.set_frequency(channel, frequency as u32)?;
        Ok(())
    }

    pub fn set_gain_mode(&self, channel: u8, mode: BladerfGainMode) -> Result<()> {
        if channel != BLADERF_MODULE_RX {
            return Err(anyhow!("Operation only supported on RX channel"));
        }

        let mut config_gpio = self.config_gpio_read()?;
        if mode == BladerfGainDefault {
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == BladerfGainMgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        Ok(self.config_gpio_write(config_gpio)?)
    }

    // static int bladerf1_set_frequency(struct bladerf *dev,
    // bladerf_channel ch,
    // bladerf_frequency frequency)
    // {
    // struct bladerf1_board_data *board_data = dev->board_data;
    // const bladerf_xb attached              = dev->xb;
    // int status;
    // int16_t dc_i, dc_q;
    // struct dc_cal_entry entry;
    // const struct dc_cal_tbl *dc_cal = (ch == BLADERF_CHANNEL_RX(0))
    // ? board_data->cal.dc_rx
    // : board_data->cal.dc_tx;
    //
    // CHECK_BOARD_STATE(STATE_FPGA_LOADED);
    //
    // log_debug("Setting %s frequency to %" BLADERF_PRIuFREQ "\n",
    // channel2str(ch), frequency);
    //
    // if (attached == BLADERF_XB_200) {
    // if (frequency < BLADERF_FREQUENCY_MIN) {
    // status = xb200_set_path(dev, ch, BLADERF_XB200_MIX);
    // if (status) {
    // return status;
    // }
    //
    // status = xb200_auto_filter_selection(dev, ch, frequency);
    // if (status) {
    // return status;
    // }
    //
    // frequency = 1248000000 - frequency;
    // } else {
    // status = xb200_set_path(dev, ch, BLADERF_XB200_BYPASS);
    // if (status) {
    // return status;
    // }
    // }
    // }
    //
    // switch (board_data->tuning_mode) {
    // case BLADERF_TUNING_MODE_HOST:
    // status = lms_set_frequency(dev, ch, (uint32_t)frequency);
    // if (status != 0) {
    // return status;
    // }
    //
    // status = band_select(dev, ch, frequency < BLADERF1_BAND_HIGH);
    // break;
    //
    // case BLADERF_TUNING_MODE_FPGA: {
    // status = dev->board->schedule_retune(dev, ch, BLADERF_RETUNE_NOW,
    // frequency, NULL);
    // break;
    // }
    //
    // default:
    // log_debug("Invalid tuning mode: %d\n", board_data->tuning_mode);
    // status = BLADERF_ERR_INVAL;
    // break;
    // }
    // if (status != 0) {
    // return status;
    // }
    //
    // if (dc_cal != NULL) {
    // dc_cal_tbl_entry(dc_cal, (uint32_t)frequency, &entry);
    //
    // dc_i = entry.dc_i;
    // dc_q = entry.dc_q;
    //
    // status = lms_set_dc_offset_i(dev, ch, dc_i);
    // if (status != 0) {
    // return status;
    // }
    //
    // status = lms_set_dc_offset_q(dev, ch, dc_q);
    // if (status != 0) {
    // return status;
    // }
    //
    // if (ch == BLADERF_CHANNEL_RX(0) &&
    // have_cap(board_data->capabilities, BLADERF_CAP_AGC_DC_LUT)) {
    // status = dev->backend->set_agc_dc_correction(
    // dev, entry.max_dc_q, entry.max_dc_i, entry.mid_dc_q,
    // entry.mid_dc_i, entry.min_dc_q, entry.min_dc_i);
    // if (status != 0) {
    // return status;
    // }
    //
    // log_verbose("Set AGC DC offset cal (I, Q) to: Max (%d, %d) "
    // " Mid (%d, %d) Min (%d, %d)\n",
    // entry.max_dc_q, entry.max_dc_i, entry.mid_dc_q,
    // entry.mid_dc_i, entry.min_dc_q, entry.min_dc_i);
    // }
    //
    // log_verbose("Set %s DC offset cal (I, Q) to: (%d, %d)\n",
    // (ch == BLADERF_CHANNEL_RX(0)) ? "RX" : "TX", dc_i, dc_q);
    // }
    //
    // return 0;
    // }

    // Get BladeRf frequency
    // https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
    // const BLADERF_CHANNEL_RX(ch) (bladerf_channel)(((ch) << 1) | 0x0)
    // https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
    // const BLADERF_MODULE_RX BLADERF_CHANNEL_RX(0)
    // https://github.com/Nuand/bladeRF/blob/fe3304d75967c88ab4f17ff37cb5daf8ff53d3e1/host/libraries/libbladeRF/src/board/bladerf1/bladerf1.c#L2121
    // static int bladerf1_get_frequency(struct bladerf *dev, bladerf_channel ch, bladerf_frequency *frequency);
    // https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
    // int lms_get_frequency(struct bladerf *dev, bladerf_module mod, struct lms_freq *f)
    // lms_freq struct: https://github.com/Nuand/bladeRF/blob/master/fpga_common/include/lms.h#L101
    // https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
    // const uint8_t base = (mod == BLADERF_MODULE_RX) ? 0x20 : 0x10;
    // sudo usermod -a -G wireshark jl
    // sudo modprobe usbmon
    // sudo setfacl -m u:jl:r /dev/usbmon*
    // Wireshark Display filter depending on lsusb output: usb.bus_id == 2 and usb.device_address == 6
    // pub fn get_freq(&self, module: u8) -> Result<LmsFreq> {
    //     //self.device.set_configuration(1)?;
    //     // TODO: FPGA must be loaded!
    //     self.interface.set_alt_setting(1)?;
    //
    //     let addr = if module == crate::bladerf::BLADERF_MODULE_RX {
    //         0x20u8
    //     } else {
    //         0x10u8
    //     };
    //
    //     let mut lms_freq = LmsFreq {
    //         freqsel: 0,
    //         vcocap: 0,
    //         nint: 0,
    //         nfrac: 0,
    //         //flags: 0,
    //         //xb_gpio: 0,
    //         x: 0,
    //         //vcocap_result: 0,
    //     };
    //
    //     let mut request = NiosPkt::<u8, u8>::new(
    //         NIOS_PKT_8X8_TARGET_LMS6,
    //         NIOS_PKT_FLAG_READ,
    //         addr | 0x0u8,
    //         0x0,
    //     );
    //
    //     let mut response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //     lms_freq.nint = u16::from(response_pkt.data()) << 1;
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x1u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.nint = lms_freq.nint | ((u16::from(response_pkt.data()) & 0x80) >> 7);
    //     lms_freq.nfrac = (u32::from(response_pkt.data()) & 0x7f) << 16;
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x2u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.nfrac = lms_freq.nfrac | (u32::from(response_pkt.data()) << 8);
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x3u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //     lms_freq.nfrac = lms_freq.nfrac | u32::from(response_pkt.data());
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x5u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.freqsel = response_pkt.data() >> 2;
    //     if lms_freq.freqsel >= 3 {
    //         lms_freq.x = 1 << ((lms_freq.freqsel & 7) - 3);
    //     }
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x9u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.vcocap = response_pkt.data() & 0x3f;
    //
    //     Ok(lms_freq)
    // }

    // pub fn lms_frequency_to_hz(lms_freq: &LmsFreq) -> u64 {
    //     let pll_coeff = ((lms_freq.nint as u64) << 23) + lms_freq.nfrac as u64;
    //     let div = (lms_freq.x as u64) << 23;
    //
    //     if div > 0 {
    //         ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div
    //     } else {
    //         0
    //     }
    // }

    /// Get BladeRf1 String descriptor
    pub fn get_string_descriptor(&self, descriptor_index: u8) -> Result<String> {
        let descriptor =
            self.device
                .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))?;
        Ok(descriptor)
    }

    /// Get BladeRf1 Serial number
    pub fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>> {
        let descriptor = self.device.get_descriptor(
            DescriptorTypes::Configuration.into(),
            descriptor_index,
            0x00,
            Duration::from_secs(1),
        )?;
        Ok(descriptor)
    }

    pub fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .device
            .get_string_descriptor_supported_languages(Duration::from_secs(1))?
            .collect();

        Ok(languages)
    }

    pub fn get_configurations(&self) -> Vec<Configuration> {
        self.device.configurations().collect()
    }

    pub fn set_configuration(&self, configuration: u16) -> Result<()> {
        //self.device.set_configuration(configuration)?;
        block_on(self.interface.control_out(ControlOut {
            control_type: ControlType::Standard,
            recipient: Recipient::Device,
            request: 0x09, //Request::VersionStringRead as u8,
            value: configuration,
            index: 0x00,
            data: &[],
        }))
        .into_result()?;
        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        //self.check_api_version(UsbVersion::from_bcd(0x0102))?;
        //self.write_control(Request::Reset, 0, 0, &[])?;
        self.device.set_configuration(0)?;

        Ok(())
    }
}

// S is the state parameter. We require it to impl
// our ResponseState trait (below) to prevent users
// from trying weird types like HttpResponse<u8>.
pub struct BladeRf1Builder<S: State> {
    //state: Box<ActualState>,
    data: S,
}

// State type options.
// zero-variant enum pattern to ensure that types exist only as types, and not as values
// Types like this are broadly referred to as phantom types

//struct ActualState {  }
pub struct Initial {
    backend: UsbBackend,
}

// pub struct WithBackend {
//     backend: UsbBackend,
// }
pub struct WithDevice {
    device: Device,
}

pub trait State {}
impl State for Initial {}
//impl State for WithBackend {}
impl State for WithDevice {}

// impl BladeRf1Builder<Initial> {
//     #[cfg(feature = "nusb")]
//     pub fn with_nusb_backend(&mut self) -> BladeRf1Builder<WithBackend> {
//         BladeRf1Builder {
//             //state: self.state.clone(),
//             data: WithBackend {
//                 backend: Arc::new(Box::new(NusbBackend {})),
//             },
//         }
//     }
//     #[cfg(feature = "rusb")]
//     pub fn with_rusb_backend(&mut self) -> BladeRf1Builder<WithBackend> {
//         BladeRf1Builder {
//             //state: self.state.clone(),
//             data: WithBackend {
//                 backend: Arc::new(Box::new(RusbBackend {})),
//             },
//         }
//     }
// }

impl BladeRf1Builder<Initial> {
    pub fn with_first(&self) -> Result<BladeRf1Builder<WithDevice>> {
        Ok(BladeRf1Builder {
            // state: self.state.clone(),
            data: WithDevice {
                device: self
                    .data
                    .backend
                    .list_devices()?
                    .find(|dev| {
                        dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID
                    })
                    .ok_or(BladeRfError::NotFound)?
                    .open()?,
            },
        })
    }
    pub fn with_serial(&self, serial: &str) -> Result<BladeRf1Builder<WithDevice>> {
        Ok(BladeRf1Builder {
            //state: self.state.clone(),
            data: WithDevice {
                device: self.data.backend.open_by_serial(serial)?,
            },
        })
    }

    pub fn with_bus_addr(
        &self,
        bus_number: u8,
        bus_addr: u8,
    ) -> Result<BladeRf1Builder<WithDevice>> {
        Ok(BladeRf1Builder {
            // state: self.state.clone(),
            data: WithDevice {
                device: self.data.backend.open_by_bus_addr(bus_number, bus_addr)?,
            },
        })
    }

    pub fn with_file_descriptor(
        &self,
        fd: std::os::fd::OwnedFd,
    ) -> Result<BladeRf1Builder<WithDevice>> {
        Ok(BladeRf1Builder {
            // state: self.state.clone(),
            data: WithDevice {
                device: self.data.backend.open_by_fd(fd)?,
            },
        })
    }
}

impl BladeRf1Builder<WithDevice> {
    pub fn build(&self) -> Result<Box<BladeRf1>> {
        //Box<dyn BladeRf>
        let device = self.data.device.clone();
        let interface = device.detach_and_claim_interface(0)?;
        let lms = LMS6002D::new(interface.clone());
        let si5338 = SI5338::new(interface.clone());
        let dac = DAC161S055::new(interface.clone());

        Ok(Box::new(BladeRf1 {
            device,
            interface,
            lms,
            si5338,
            dac,
        }))
    }
}

impl BladeRf for BladeRf1 {}
