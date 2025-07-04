use crate::board::bladerf1::BoardData;
use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use crate::nios::Nios;
use crate::{BladeRf1, BladeRfError};
use anyhow::{Result, anyhow};
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_FEATURE_SMALL_DMA_XFER, BLADERF1_USB_PID, BLADERF1_USB_VID,
};
use bladerf_globals::{
    BLADE_USB_CMD_GET_LOOPBACK, BLADE_USB_CMD_RF_RX, BLADE_USB_CMD_RF_TX,
    BLADE_USB_CMD_SET_LOOPBACK, BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladeRfDirection,
    BladerfGainMode, DescriptorTypes, StringDescriptors, TIMEOUT, USB_IF_NULL, USB_IF_RF_LINK,
    bladerf_channel_is_tx, bladerf_channel_rx, bladerf_channel_tx,
};
use bladerf_nios::packet_retune::Band;
use nusb::descriptors::ConfigurationDescriptor;
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::{Device, DeviceInfo, Speed};
use std::num::NonZero;
use std::time::Duration;

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

        let board_data = BoardData {
            speed: device.speed().expect("Could not determine device speed!"),
        };

        if board_data.speed < Speed::High {
            return Err(anyhow!("BladeRF requires High/Super/SuperPlus speeds"));
        }

        let lms = LMS6002D::new(interface.clone());
        let si5338 = SI5338::new(interface.clone());
        let dac = DAC161S055::new(interface.clone());

        Ok(Box::new(Self {
            device,
            interface,
            board_data,
            lms,
            si5338,
            dac,
            xb100: None,
            xb200: None,
            xb300: None,
        }))
    }

    /// Opens the first BladeRf1 it can find
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libbladerf_rs::BladeRf1;
    /// let dev = BladeRf1::from_first().await?;
    /// ```
    pub async fn from_first() -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .next()
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    /// Opens a specific BladeRf1 identified by its serial number
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libbladerf_rs::BladeRf1;
    /// let dev = BladeRf1::from_serial("0123456789abcdef").await?;
    /// ```
    pub async fn from_serial(serial: &str) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.serial_number() == Some(serial))
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    /// Opens a BladeRf1 identified by its USB bus address
    pub async fn from_bus_addr(bus_number: u8, bus_addr: u8) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.busnum() == bus_number && dev.device_address() == bus_addr)
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    /// Opens a BladeRf1 from a file descriptor. This method is the only available option
    /// on Android devices, as listing USB devices etc. is not possible.
    /// This method does not check, if the file descriptor really belongs to a BladeRf1.
    /// Undefined behaviour is expected, if a file descriptor to a device is given, that is not a BladeRf1.
    pub async fn from_fd(fd: std::os::fd::OwnedFd) -> Result<Box<Self>> {
        let device = Device::from_fd(fd).await?;
        // TODO: Do check on device, if it really is a bladerf
        Self::build(device).await
    }

    /// Returns the USB speed which is used by the BladeRf1.
    pub fn speed(&self) -> Speed {
        self.board_data.speed
    }

    /// Return the devices' serial number
    pub async fn serial(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Serial as u8)?)
            .await
    }

    /// Return the devices' manufacturer (Nuand)
    pub async fn manufacturer(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Manufacturer as u8)?)
            .await
    }

    /// Return the devices' FX3 firmware version
    pub async fn fx3_firmware(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Fx3Firmware as u8)?)
            .await
    }

    pub async fn fpga_version(&self) -> Result<String> {
        let version = self.interface.nios_get_fpga_version().await?;
        Ok(format!("{version}"))
    }

    /// Return the devices' product name (BladeRf1)
    pub async fn product(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Product as u8)?)
            .await
    }

    /// Read from the configuration GPIO register.
    pub(crate) async fn config_gpio_read(&self) -> Result<u32> {
        self.interface.nios_config_read().await
    }

    /// Write to the configuration GPIO register.
    /// Callers should be sure to perform a read-modify-write sequence to avoid accidentally
    /// clearing other GPIO bits that may be set by the library internally.
    pub(crate) async fn config_gpio_write(&self, mut data: u32) -> Result<()> {
        log::debug!("[config_gpio_write] data: {data}");
        log::debug!("[config_gpio_write] speed: {:?}", self.board_data.speed);
        /* If we're connected at HS, we need to use smaller DMA transfers */
        match self.board_data.speed {
            Speed::High => {
                data |= BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
            Speed::Super => {
                data &= !(BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32);
            }
            _ => return Err(anyhow!("speed {:?} not supported", self.board_data.speed)),
        }
        log::debug!("[config_gpio_write] data after speedcheck: {data}");

        self.interface.nios_config_write(data).await
    }

    /// Initialize device registers - required after power-up, but safe
    /// to call multiple times after power-up (e.g., multiple close and reopens)
    pub async fn initialize(&mut self) -> Result<()> {
        self.interface.set_alt_setting(0x01).await?;
        log::debug!("[*] Init - Set Alt Setting to 0x01");

        // Out: 43010000000000000000000000000000
        // In:  43010200000000000000000000000000
        let cfg = self.config_gpio_read().await?;
        if (cfg & 0x7f) == 0 {
            log::debug!("[*] Init - Default GPIO value \"{cfg}\" found - initializing device");
            /* Set the GPIO pins to enable the LMS and select the low band */
            // Out: 43010100005700000000000000000000
            // In:  43010300005700000000000000000000
            self.config_gpio_write(0x57).await?;

            /* Disable the front ends */
            log::debug!("[*] Init - Disabling RX and TX Frontend");
            // Out: 41000000400000000000000000000000
            // In:  41000200400200000000000000000000
            // Out: 41000100400000000000000000000000
            // In:  41000300400000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_TX, false).await?;
            log::debug!("{BLADERF_MODULE_TX}");

            // Out: 41000000700000000000000000000000
            // In:  41000200700200000000000000000000
            // Out: 41000100700000000000000000000000
            // In:  41000300700000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_RX, false).await?;
            log::debug!("{BLADERF_MODULE_RX}");

            /* Set the internal LMS register to enable RX and TX */
            log::debug!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100053e00000000000000000000
            // In:  41000300053e00000000000000000000
            self.lms.write(0x05, 0x3e).await?;

            /* LMS FAQ: Improve TX spurious emission performance */
            log::debug!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100474000000000000000000000
            // In:  41000300474000000000000000000000
            self.lms.write(0x47, 0x40).await?;

            /* LMS FAQ: Improve ADC performance */
            log::debug!("[*] Init - Set register to improve ADC performance");
            // Out: 41000100592900000000000000000000
            // In:  41000300592900000000000000000000
            self.lms.write(0x59, 0x29).await?;

            /* LMS FAQ: Common mode voltage for ADC */
            log::debug!("[*] Init - Set Common mode voltage for ADC");
            // Out: 41000100643600000000000000000000
            // In:  41000300643600000000000000000000
            self.lms.write(0x64, 0x36).await?;

            /* LMS FAQ: Higher LNA Gain */
            log::debug!("[*] Init - Set Higher LNA Gain");
            // Out: 41000100793700000000000000000000
            // In:  41000300793700000000000000000000
            self.lms.write(0x79, 0x37).await?;

            /* Power down DC calibration comparators until they are need, as they
             * have been shown to introduce undesirable artifacts into our signals.
             * (This is documented in the LMS6 FAQ). */

            log::debug!("[*] Init - Power down TX LPF DC cal comparator");
            // Out: 410000003f0000000000000000000000
            // In:  410002003f0000000000000000000000
            // Out: 410001003f8000000000000000000000
            // In:  410003003f8000000000000000000000
            self.lms.set(0x3f, 0x80).await?; /* TX LPF DC cal comparator */

            log::debug!("[*] Init - Power down RX LPF DC cal comparator");
            // Out: 410000005f0000000000000000000000
            // In:  410002005f1f00000000000000000000
            // Out: 410001005f9f00000000000000000000
            // In:  410003005f9f00000000000000000000
            self.lms.set(0x5f, 0x80).await?; /* RX LPF DC cal comparator */

            log::debug!("[*] Init - Power down RXVGA2A/B DC cal comparators");
            // Out: 410000006e0000000000000000000000
            // In:  410002006e0000000000000000000000
            // Out: 410001006ec000000000000000000000
            // In:  410003006ec000000000000000000000
            self.lms.set(0x6e, 0xc0).await?; /* RXVGA2A/B DC cal comparators */

            /* Configure charge pump current offsets */
            log::debug!("[*] Init - Configure TX charge pump current offsets");
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
            self.lms.config_charge_pumps(BLADERF_MODULE_TX).await?;
            log::debug!("[*] Init - Configure RX charge pump current offsets");

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
            self.lms.config_charge_pumps(BLADERF_MODULE_RX).await?;

            log::debug!("[*] Init - Set TX Samplerate");
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

            log::debug!("[*] Init - Set RX Samplerate");
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

            log::debug!("self.set_frequency(bladerf_channel_tx!(0), 2447000000)?;");
            // Out: 5400000000000000003fb95555ac1f00
            // In:  5400000000000000001e030000000000
            self.set_frequency(bladerf_channel_tx!(0), 2447000000)
                .await?;

            log::debug!("self.set_frequency(bladerf_channel_rx!(0), 2484000000)?;");
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
            log::debug!("[*] Init - Device already initialized: {cfg:#04x}");
            //board_data->tuning_mode = tuning_get_default_mode(dev);
        }

        /* Check if we have an expansion board attached */
        //let xb = self.expansion_get_attached();

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

    /* Vendor command that sets a 32-bit integer value */
    // async fn set_vendor_cmd_int(&self, cmd: u8, val: u32) -> Result<()> {
    //     let pkt = ControlOut {
    //         control_type: ControlType::Vendor,
    //         recipient: Recipient::Device,
    //         request: cmd,
    //         value: 0,
    //         index: 0,
    //         data: &val.to_le_bytes(),
    //     };
    //     Ok(self
    //         .interface
    //         .control_out(pkt, Duration::from_secs(5))
    //         .await?)
    // }

    /* Vendor command that gets a 32-bit integer value */
    async fn get_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: 0,
            index: 0,
            length: 0x4,
        };
        let vec = self
            .interface
            .control_in(pkt, Duration::from_secs(5))
            .await?;

        // TODO: Examine return value and return it
        log::debug!("get_vendor_cmd_int response data: {vec:?}");
        Ok(u32::from_le_bytes(vec.as_slice()[0..4].try_into()?))
    }
    /// Vendor command wrapper to get a 32-bit integer and supplies wValue */
    /// TODO: Return u32 value
    async fn vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        // struct bladerf_usb *usb = dev->backend_data;
        //
        // return usb->fn->control_transfer(usb->driver,
        // USB_TARGET_DEVICE,
        // USB_REQUEST_VENDOR,
        // USB_DIR_DEVICE_TO_HOST,
        // cmd, wvalue, 0,
        // val, sizeof(uint32_t),
        // CTRL_TIMEOUT_MS);
        // }

        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: wvalue,
            index: 0,
            length: 0x4,
        };
        let vec = self
            .interface
            .control_in(pkt, Duration::from_secs(5))
            .await?;
        // TODO: Examine return value and return it
        log::debug!("vendor_cmd_int_wvalue response data: {vec:?}");
        Ok(u32::from_le_bytes(vec.as_slice()[0..4].try_into()?))
    }

    /// Enable/Disable RF Module via the USB backend.
    /// This method should probably be moved to some USB backend dedicated source file.
    async fn usb_enable_module(&self, direction: &BladeRfDirection, enable: bool) -> Result<()> {
        let val = enable as u16;

        let cmd = if *direction == BladeRfDirection::Rx {
            BLADE_USB_CMD_RF_RX
        } else {
            BLADE_USB_CMD_RF_TX
        };

        let _fx3_ret = self.vendor_cmd_int_wvalue(cmd, val).await?;
        // if fx3_ret {
        //     log::debug!("FX3 reported error={fx3_ret:?} when {} RF {direction:?}", if enable {"enabling"} else { "disabling"});
        //
        //         /* FIXME: Work around what seems to be a harmless failure.
        //      *        It appears that in firmware or in the lib, we may be
        //      *        attempting to disable an already disabled channel, or
        //      *        enabling an already enabled channel.
        //      *
        //      *        Further investigation required
        //      *
        //      *        0x44 corresponds to CY_U3P_ERROR_ALREADY_STARTED
        //      */
        //         if fx3_ret != 0x44 {
        //                Err(BladeRfError::Unexpected)
        //         }
        // }

        Ok(())
    }

    pub async fn change_setting(&self, setting: u8) -> Result<()> {
        Ok(self.interface.set_alt_setting(setting).await?)
    }
    pub async fn usb_set_firmware_loopback(&self, enable: bool) -> Result<()> {
        self.vendor_cmd_int_wvalue(BLADE_USB_CMD_SET_LOOPBACK, enable as u16)
            .await?;
        self.change_setting(USB_IF_NULL).await?;
        self.change_setting(USB_IF_RF_LINK).await?;
        Ok(())
    }

    pub async fn usb_get_firmware_loopback(&self) -> Result<bool> {
        let result = self.get_vendor_cmd_int(BLADE_USB_CMD_GET_LOOPBACK).await?;

        Ok(result != 0)
    }

    /// Enable/Disable RF Module
    pub async fn enable_module(&self, module: u8, enable: bool) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let direction = if bladerf_channel_is_tx!(module) {
            BladeRfDirection::Tx
        } else {
            BladeRfDirection::Rx
        };

        //
        // if (ch != BLADERF_CHANNEL_RX(0) && ch != BLADERF_CHANNEL_TX(0)) {
        //     return BLADERF_ERR_INVAL;
        // }
        //
        // log_debug("Enable channel: %s - %s\n",
        //           BLADERF_CHANNEL_IS_TX(ch) ? "TX" : "RX",
        //           enable ? "True" : "False");

        if !enable {
            // sync_deinit(&board_data->sync[ch]);
            self.perform_format_deconfig(&direction)?;
        }

        self.lms.enable_rffe(module, enable).await?;

        self.usb_enable_module(&direction, enable).await
    }

    /// FPGA Band Selection
    pub async fn band_select(&self, module: u8, band: Band) -> Result<()> {
        let band_value = match band {
            Band::Low => 2,
            Band::High => 1,
        };

        log::debug!("Selecting %s band. {band:?}");

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

    /// Get BladeRf1 USB String descriptor identified by an Index number
    /// Valid indices are given in: ```rust StringDescriptors```
    pub async fn get_string_descriptor(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .device
            .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))
            .await?;
        Ok(descriptor)
    }

    /// Get BladeRf1 Configuration Descriptor
    /// TODO: What is a configuration descriptor?
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

    /// Get a list of supported languages of the BladeRF1. Retuns a Vector with Language codes.
    /// TODO: How can these language codes be translated to a str representation? nusb offers something?
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

    /// TODO: set which configuration???
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

    /// Reset the BladeRF1
    /// TODO Find out if this is soft reset or hard reset?
    pub async fn reset(&self) -> Result<()> {
        //self.check_api_version(UsbVersion::from_bcd(0x0102))?;
        //self.write_control(Request::Reset, 0, 0, &[])?;
        self.device.set_configuration(0).await?;

        Ok(())
    }
}
