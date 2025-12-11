// /// Stream channel layout
// #[derive(PartialEq)]
// pub enum BladeRfChannelLayout {
//     RxX1 = 0, // x1 RX (SISO)
//     TxX1 = 1, // x1 TX (SISO)
//     RxX2 = 2, // x2 RX (MIMO)
//     TxX2 = 3, // x2 TX (MIMO)
// }

///  Stream direction
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    Rx = 0, // Receive1
    Tx = 1, // Transmit1
}

impl Channel {
    pub fn is_tx(&self) -> bool {
        *self == Channel::Tx
    }
}

impl TryFrom<u8> for Channel {
    type Error = Error;
    fn try_from(value: u8) -> crate::Result<Self> {
        match value {
            0 => Ok(Channel::Rx),
            1 => Ok(Channel::Tx),
            _ => {
                log::error!("unsupported channel!");
                Err(Error::Invalid)
            }
        }
    }
}
//
// // #[macro_export]
// macro_rules! bladerf_channel_rx {
//     ($ch:expr) => {
//         ((($ch) << 1) | 0x0) as u8
//     };
// }
// pub(crate) use bladerf_channel_rx;
//
// // #[macro_export]
// macro_rules! bladerf_channel_tx {
//     ($ch:expr) => {
//         ((($ch) << 1) | 0x1) as u8
//     };
// }
// pub(crate) use bladerf_channel_tx;
//
// ///  Convenience macro: true if argument is a TX channel
// // #[macro_export]
// macro_rules! bladerf_channel_is_tx {
//     ($ch:expr) => {
//         (($ch) & crate::bladerf::Direction::Tx as u8) != 0
//     };
// }
// pub(crate) use bladerf_channel_is_tx;

// #[macro_export]
macro_rules! khz {
    ($value:expr) => {
        ($value * 1000u32)
    };
}
pub(crate) use khz;

// #[macro_export]
macro_rules! mhz {
    ($value:expr) => {
        ($value * 1000000u32)
    };
}
pub(crate) use mhz;

// #[macro_export]
// macro_rules! ghz {
//     ($value:expr) => {
//         ($value * 1000000000u32)
//     };
// }
// pub(crate) use ghz;

use crate::Error;
use std::time::Duration;

///  Stream direction
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Direction {
    Rx = 0, // Receive direction
    Tx = 1, // Transmit direction
}

#[repr(u8)]
pub enum StringDescriptors {
    /// Don't want to start with 0 as 0 is reserved for the language table
    Manufacturer = 0x1,
    Product,
    Serial,
    Fx3Firmware,
}

#[repr(u8)]
pub enum DescriptorTypes {
    /// Don't want to start with 0 as 0 is reserved for the language table
    // Device = 0x01,
    Configuration = 0x2,
    // String = 0x03,
    // Default = 0x06,
    // BOS = 0x0f,
}

// pub const BLADE_USB_CMD_QUERY_VERSION: u8 = 0;
// pub const BLADE_USB_CMD_QUERY_FPGA_STATUS: u8 = 1;
// pub const BLADE_USB_CMD_BEGIN_PROG: u8 = 2;
// pub const BLADE_USB_CMD_END_PROG: u8 = 3;
pub const BLADE_USB_CMD_RF_RX: u8 = 4;
pub const BLADE_USB_CMD_RF_TX: u8 = 5;
// pub const BLADE_USB_CMD_QUERY_DEVICE_READY: u8 = 6;
// pub const BLADE_USB_CMD_QUERY_FLASH_ID: u8 = 7;
// pub const BLADE_USB_CMD_QUERY_FPGA_SOURCE: u8 = 8;
// pub const BLADE_USB_CMD_FLASH_READ: u8 = 100;
// pub const BLADE_USB_CMD_FLASH_WRITE: u8 = 101;
// pub const BLADE_USB_CMD_FLASH_ERASE: u8 = 102;
// pub const BLADE_USB_CMD_READ_OTP: u8 = 103;
// pub const BLADE_USB_CMD_WRITE_OTP: u8 = 104;
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

pub const TIMEOUT: Duration = Duration::from_millis(1);

// pub const BLADERF_MODULE_RX: u8 = bladerf_channel_rx!(0);
// pub const BLADERF_MODULE_TX: u8 = bladerf_channel_tx!(0);
