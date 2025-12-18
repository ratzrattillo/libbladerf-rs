use crate::{Direction, Error};
use nusb::MaybeFuture;
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::{Device, Interface};
use std::num::NonZero;
use std::time::Duration;

#[repr(u8)]
pub enum StringDescriptors {
    /// Don't want to start with 0 as 0 is reserved for the language table
    Manufacturer = 0x1,
    Product,
    Serial,
    Fx3Firmware,
}
#[repr(u8)]
#[allow(dead_code)]
pub enum DescriptorTypes {
    /// Don't want to start with 0 as 0 is reserved for the language table
    #[allow(dead_code)]
    Device = 0x01,
    Configuration = 0x2,
    #[allow(dead_code)]
    String = 0x03,
    #[allow(dead_code)]
    Default = 0x06,
    #[allow(dead_code)]
    Bos = 0x0f,
}

// pub const BLADE_USB_CMD_QUERY_VERSION: u8 = 0;
// pub const BLADE_USB_CMD_QUERY_FPGA_STATUS: u8 = 1;
// pub const BLADE_USB_CMD_BEGIN_PROG: u8 = 2;
// pub const BLADE_USB_CMD_END_PROG: u8 = 3;
pub const BLADE_USB_CMD_RF_RX: u8 = 4;
pub const BLADE_USB_CMD_RF_TX: u8 = 5;
#[allow(dead_code)]
pub const BLADE_USB_CMD_QUERY_DEVICE_READY: u8 = 6;
// pub const BLADE_USB_CMD_QUERY_FLASH_ID: u8 = 7;
// pub const BLADE_USB_CMD_QUERY_FPGA_SOURCE: u8 = 8;
// pub const BLADE_USB_CMD_FLASH_READ: u8 = 100;
// pub const BLADE_USB_CMD_FLASH_WRITE: u8 = 101;
// pub const BLADE_USB_CMD_FLASH_ERASE: u8 = 102;
// pub const BLADE_USB_CMD_READ_OTP: u8 = 103;
// pub const BLADE_USB_CMD_WRITE_OTP: u8 = 104;
#[allow(dead_code)]
pub const BLADE_USB_CMD_RESET: u8 = 105;
// pub const BLADE_USB_CMD_JUMP_TO_BOOTLOADER: u8 = 106;
// pub const BLADE_USB_CMD_READ_PAGE_BUFFER: u8 = 107;
// pub const BLADE_USB_CMD_WRITE_PAGE_BUFFER: u8 = 108;
// pub const BLADE_USB_CMD_LOCK_OTP: u8 = 109;
// pub const BLADE_USB_CMD_READ_CAL_CACHE: u8 = 110;
// pub const BLADE_USB_CMD_INVALIDATE_CAL_CACHE: u8 = 111;
// pub const BLADE_USB_CMD_REFRESH_CAL_CACHE: u8 = 112;
pub const BLADE_USB_CMD_SET_LOOPBACK: u8 = 113;
pub const BLADE_USB_CMD_GET_LOOPBACK: u8 = 114;
// pub const BLADE_USB_CMD_READ_LOG_ENTRY: u8 = 115;

//
// /// String descriptor indices
// /// Manufacturer
// pub const BLADE_USB_STR_INDEX_MFR: u8 = 1;
// /// Product
// pub const BLADE_USB_STR_INDEX_PRODUCT: u8 = 2;
// /// Serial number
// pub const BLADE_USB_STR_INDEX_SERIAL: u8 = 3;
// /// Firmware version
// pub const BLADE_USB_STR_INDEX_FW_VER: u8 = 4;
//
// pub const CAL_BUFFER_SIZE: u16 = 256;
// pub const CAL_PAGE: u16 = 768;
//
// pub const AUTOLOAD_BUFFER_SIZE: u16 = 256;
// pub const AUTOLOAD_PAGE: u16 = 1024;

// #ifdef _MSC_VER
// #   define PACK(decl_to_pack_) \
// __pragma(pack(push,1)) \
// decl_to_pack_ \
// __pragma(pack(pop))
// #elif defined(__GNUC__)
// #   define PACK(decl_to_pack_) \
// decl_to_pack_ __attribute__((__packed__))
// #else
// #error "Unexpected compiler/environment"
// #endif
//
// PACK(
// struct bladerf_fx3_version {
//     unsigned short major;
//     unsigned short minor;
// });
//
// struct bladeRF_firmware {
//     unsigned int len;
//     unsigned char/// ptr;
// };
//
// struct bladeRF_sector {
//     unsigned int idx;
//     unsigned int len;
//     unsigned char/// ptr;
// };
//
// ///
// ///  FPGA configuration source
// ///
// ///  Note: the numbering of this enum must match bladerf_fpga_source in
// ///  libbladeRF.h
// /// /
// typedef enum {
//     NUAND_FPGA_CONFIG_SOURCE_INVALID = 0, /// < Uninitialized/invalid/// /
//     NUAND_FPGA_CONFIG_SOURCE_FLASH   = 1, /// < Last FPGA load was from flash/// /
//     NUAND_FPGA_CONFIG_SOURCE_HOST    = 2  /// < Last FPGA load was from host/// /
// } NuandFpgaConfigSource;
//
// #define USB_CYPRESS_VENDOR_ID   0x04b4
// #define USB_FX3_PRODUCT_ID      0x00f3
//
// #define BLADE_USB_TYPE_OUT      0x40
// #define BLADE_USB_TYPE_IN       0xC0
// #define BLADE_USB_TIMEOUT_MS    1000
//
// #define USB_NUAND_VENDOR_ID                         0x2cf0
// #define USB_NUAND_BLADERF_PRODUCT_ID                0x5246
// #define USB_NUAND_BLADERF_BOOT_PRODUCT_ID           0x5247
// #define USB_NUAND_BLADERF2_PRODUCT_ID               0x5250
//
// #define USB_NUAND_LEGACY_VENDOR_ID                  0x1d50
// #define USB_NUAND_BLADERF_LEGACY_PRODUCT_ID         0x6066
// #define USB_NUAND_BLADERF_LEGACY_BOOT_PRODUCT_ID    0x6080
//
// #define USB_NUAND_BLADERF_MINOR_BASE 193
// #define NUM_CONCURRENT  8
// #define NUM_DATA_URB    (1024)
// #define DATA_BUF_SZ     (1024*4)

/// Interface numbers
// pub const USB_IF_LEGACY_CONFIG: u8 = 0;
pub const USB_IF_NULL: u8 = 0;
pub const USB_IF_RF_LINK: u8 = 1;
// pub const USB_IF_SPI_FLASH: u8 = 2;
// pub const USB_IF_CONFIG: u8 = 3;

#[allow(dead_code)]
const TIMEOUT: Duration = Duration::from_millis(1);

// pub const BLADERF_MODULE_RX: u8 = bladerf_channel_rx!(0);
// pub const BLADERF_MODULE_TX: u8 = bladerf_channel_tx!(0);

pub trait DeviceCommands {
    fn get_supported_languages(&self) -> crate::Result<Vec<u16>>;
    #[allow(dead_code)]
    fn get_configuration_descriptor(&self, descriptor_index: u8) -> crate::Result<Vec<u8>>;
    /// Get BladeRf1 USB String descriptor identified by an Index number
    /// Valid indices are given in: ```rust StringDescriptors```
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> crate::Result<String>;
    // /// Returns the USB speed which is used by the BladeRf1.
    // fn speed(&self) -> crate::Result<Speed>;
    /// Return the devices' serial number
    fn serial(&self) -> crate::Result<String>;
    /// Return the devices' manufacturer (Nuand)
    fn manufacturer(&self) -> crate::Result<String>;
    /// Return the devices' product name (BladeRf1)
    fn product(&self) -> crate::Result<String>;
    /// Return the devices' FX3 firmware version
    fn fx3_firmware_version(&self) -> crate::Result<String>;
}

impl DeviceCommands for Device {
    /// Get a list of supported languages of the BladeRF1. Returns a Vector with Language codes.
    /// TODO: How can these language codes be translated to a str representation? nusb offers something?
    fn get_supported_languages(&self) -> crate::Result<Vec<u16>> {
        let languages = self
            .get_string_descriptor_supported_languages(Duration::from_secs(1))
            .wait()
            .map_err(|_| Error::Invalid)?
            .collect();

        Ok(languages)
    }

    /// Get BladeRf1 Configuration Descriptor
    /// TODO: What is a configuration descriptor?
    fn get_configuration_descriptor(&self, descriptor_index: u8) -> crate::Result<Vec<u8>> {
        let descriptor = self
            .get_descriptor(
                DescriptorTypes::Configuration as u8,
                descriptor_index,
                0x00,
                Duration::from_secs(1),
            )
            .wait()
            .map_err(|_| Error::Invalid)?;
        Ok(descriptor)
    }

    /// Get BladeRf1 USB String descriptor identified by an Index number
    /// Valid indices are given in: ```rust StringDescriptors```
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> crate::Result<String> {
        let descriptor = self
            .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))
            .wait()
            .map_err(|_| Error::Invalid)?;
        Ok(descriptor)
    }

    /// Return the devices' serial number
    fn serial(&self) -> crate::Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Serial as u8).map_err(|_| Error::Invalid)?,
        )
    }

    /// Return the devices' manufacturer (Nuand)
    fn manufacturer(&self) -> crate::Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Manufacturer as u8).map_err(|_| Error::Invalid)?,
        )
    }

    /// Return the devices' product name (BladeRf1)
    fn product(&self) -> crate::Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Product as u8).map_err(|_| Error::Invalid)?,
        )
    }

    /// Return the devices' FX3 firmware version
    fn fx3_firmware_version(&self) -> crate::Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Fx3Firmware as u8).map_err(|_| Error::Invalid)?,
        )
    }
}

pub trait UsbCommands {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> crate::Result<u32>;
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> crate::Result<u32>;
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> crate::Result<()>;
    fn usb_change_setting(&self, setting: u8) -> crate::Result<()>;
    fn usb_set_firmware_loopback(&self, enable: bool) -> crate::Result<()>;
    fn usb_get_firmware_loopback(&self) -> crate::Result<bool>;
    #[allow(dead_code)]
    fn usb_set_configuration(&self, configuration: u16) -> crate::Result<()>;
    #[allow(dead_code)]
    fn usb_device_reset(&self) -> crate::Result<()>;
    #[allow(dead_code)]
    fn usb_is_firmware_ready(&self) -> crate::Result<bool>;
}

impl UsbCommands for Interface {
    /// Vendor command that gets a 32-bit integer value
    fn usb_vendor_cmd_int(&self, cmd: u8) -> crate::Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: 0,
            index: 0,
            length: 0x4,
        };
        let vec = self.control_in(pkt, Duration::from_secs(5)).wait()?;

        // TODO: Examine return value and return it
        log::debug!("get_vendor_cmd_int response data: {vec:?}");
        Ok(u32::from_le_bytes(
            vec.as_slice()[0..4]
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ))
    }

    /// Vendor command wrapper to get a 32-bit integer and supplies wValue
    /// TODO: Return u32 value
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> crate::Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: wvalue,
            index: 0,
            length: 0x4,
        };
        let vec = self.control_in(pkt, Duration::from_secs(5)).wait()?;
        // TODO: Examine return value and return it
        log::trace!("vendor_cmd_int_wvalue response data: {vec:?}");
        Ok(u32::from_le_bytes(
            vec.as_slice()[0..4]
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ))
    }

    /// Enable/Disable RF Module via the USB backend.
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> crate::Result<()> {
        let val = enable as u16;

        let cmd = if direction == Direction::Rx {
            BLADE_USB_CMD_RF_RX
        } else {
            BLADE_USB_CMD_RF_TX
        };

        let _fx3_ret = self.usb_vendor_cmd_int_wvalue(cmd, val)?;
        // TODO:
        // if fx3_ret {
        //     log::trace!("FX3 reported error={fx3_ret:?} when {} RF {direction:?}", if enable {"enabling"} else { "disabling"});
        //
        //      // FIXME: Work around what seems to be a harmless failure.
        //      //        It appears that in firmware or in the lib, we may be
        //      //        attempting to disable an already disabled channel, or
        //      //        enabling an already enabled channel.
        //      //
        //      //        Further investigation required
        //      //
        //      //        0x44 corresponds to CY_U3P_ERROR_ALREADY_STARTED
        //
        //         if fx3_ret != 0x44 {
        //                Err(BladeRfError::Unexpected)
        //         }
        // }

        Ok(())
    }

    /// Change USB Alternate Setting
    fn usb_change_setting(&self, setting: u8) -> crate::Result<()> {
        Ok(self.set_alt_setting(setting).wait()?)
    }

    /// TODO:
    fn usb_set_firmware_loopback(&self, enable: bool) -> crate::Result<()> {
        self.usb_vendor_cmd_int_wvalue(BLADE_USB_CMD_SET_LOOPBACK, enable as u16)?;
        self.usb_change_setting(USB_IF_NULL)?;
        self.usb_change_setting(USB_IF_RF_LINK)?;
        Ok(())
    }

    /// TODO:
    fn usb_get_firmware_loopback(&self) -> crate::Result<bool> {
        let result = self.usb_vendor_cmd_int(BLADE_USB_CMD_GET_LOOPBACK)?;
        Ok(result != 0)
    }

    /// TODO: set which configuration???
    fn usb_set_configuration(&self, configuration: u16) -> crate::Result<()> {
        // self.device.set_configuration(configuration)?;
        Ok(self
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
            .wait()?)
    }

    /// Reset the BladeRF1
    /// TODO Find out if this is soft reset or hard reset?
    fn usb_device_reset(&self) -> crate::Result<()> {
        // TODO: Dont know what this is doing
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: BLADE_USB_CMD_RESET,
            value: 0x0,
            index: 0x0,
            data: &[],
        };

        self.control_out(pkt, Duration::from_secs(100)).wait()?;
        // self.device.set_configuration(0).wait()?;
        // self.interface.set_alt_setting(0).wait()?;

        Ok(())
    }

    fn usb_is_firmware_ready(&self) -> crate::Result<bool> {
        Ok(self.usb_vendor_cmd_int(BLADE_USB_CMD_QUERY_DEVICE_READY)? != 0)
    }
}
