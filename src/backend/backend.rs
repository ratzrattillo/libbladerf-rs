use crate::backend::nusb::NusbError;
use crate::backend::rusb::RusbError;
use anyhow::{Context, Result};

mod constants;

#[cfg(feature = "nusb")]
pub mod nusb;
#[cfg(feature = "rusb")]
pub mod rusb;

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[cfg(feature = "rusb")]
    #[error("rusb")]
    RusbBackendError(#[from] RusbError),

    #[cfg(feature = "nusb")]
    #[error("nusb")]
    NusbBackendError(#[from] NusbError),

    /// Device not found.
    #[error("ConversionError")]
    ConversionError,
}

#[derive(Copy, Clone, Eq, PartialOrd, Ord, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum UsbSpeed {
    /// Low speed (1.5 Mbit)
    Low,

    /// Full speed (12 Mbit)
    Full,

    /// High speed (480 Mbit)
    High,

    /// Super speed (5000 Mbit)
    Super,

    /// Super speed (10000 Mbit)
    SuperPlus,
}

/// A three-part version consisting of major, minor, and sub minor components.
///
/// The intended use case of `Version` is to extract meaning from the version fields in USB
/// descriptors, such as `bcdUSB` and `bcdDevice` in device descriptors.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub struct UsbVersion{pub major: u8, minor: u8, sub_minor: u8}

impl UsbVersion {
    /// Extracts a version from a binary coded decimal (BCD) field. BCD fields exist in USB
    /// descriptors as 16-bit integers encoding a version as `0xJJMN`, where `JJ` is the major
    /// version, `M` is the minor version, and `N` is the sub minor version. For example, 2.0 is
    /// encoded as `0x0200` and 1.1 is encoded as `0x0110`.
    pub fn from_bcd(mut raw: u16) -> Self {
        let sub_minor: u8 = (raw & 0x000F) as u8;
        raw >>= 4;

        let minor: u8 = (raw & 0x000F) as u8;
        raw >>= 4;

        let mut major: u8 = (raw & 0x000F) as u8;
        raw >>= 4;

        major += (10 * raw) as u8;

        UsbVersion{major, minor, sub_minor}
    }

    /// Returns the major version.
    pub fn major(&self) -> u8 {
        self.major
    }

    /// Returns the minor version.
    pub fn minor(&self) -> u8 {
        self.minor
    }

    /// Returns the sub minor version.
    pub fn sub_minor(&self) -> u8 {
        self.sub_minor
    }
}

impl std::fmt::Display for UsbVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major(), self.minor(), self.sub_minor())
    }
}

pub trait UsbDeviceInfoTrait {
    fn open(&self) -> Result<Box<dyn UsbDeviceTrait>>;
    fn class(&self) -> u8;
    fn subclass(&self) -> u8;
    fn protocol(&self) -> u8;
    fn product_string(&self) -> Option<&str>;
    fn manufacturer_string(&self) -> Option<&str>;
    fn serial_number(&self) -> Option<&str>;
    fn device_version(&self) -> UsbVersion;
    fn vendor_id(&self) -> u16;
    fn product_id(&self) -> u16;
    fn device_address(&self) -> u8;
    fn bus_number(&self) -> u8;
    //fn interfaces(&self) -> impl Iterator<Item = Box<dyn UsbInterfaceInfoTrait>>;
}

pub trait UsbInterfaceInfoTrait {
    fn class(&self) -> u8;
    fn subclass(&self) -> u8;
    fn protocol(&self) -> u8;
    fn interface_number(&self) -> u8;
    //fn interface_string(&self) -> Option<&str>;
}
pub trait UsbDeviceTrait {
    fn hello(&self) -> &str {
        "hello"
    }

    fn test_me(&self);
    fn reset(&self) -> Result<()>;
}

pub trait UsbBackend: UsbBackendMarker {
    fn list_devices(&self) -> Result<impl Iterator<Item = Box<dyn UsbDeviceInfoTrait>>>;
    // fn open_by_fd(&self, fd: std::os::fd::OwnedFd) -> Result<Device>;
}

pub trait UsbBackendMarker {
    fn find_by_bus_addr(&self, bus_number: u8, address: u8) -> Result<Box<dyn UsbDeviceInfoTrait>>;
    fn find_by_serial(&self, serial: &str) -> Result<Box<dyn UsbDeviceInfoTrait>>;
}
impl<T> UsbBackendMarker for T
where
    T: UsbBackend,
{
    fn find_by_bus_addr(&self, bus_number: u8, address: u8) -> Result<Box<dyn UsbDeviceInfoTrait>> {
        Ok(self
            .list_devices()?
            .find(|dev| dev.bus_number() == bus_number && dev.device_address() == address)
            .context("Not Found")?) //.ok_or(Error::NotFound)?)
    }

    fn find_by_serial(&self, serial: &str) -> Result<Box<dyn UsbDeviceInfoTrait>> {
        Ok(self
            .list_devices()?
            .find(|dev| {
                dev.serial_number()
                    .is_some_and(|device_serial| device_serial == serial)
            })
            .context("Not Found")?) //.ok_or(Error::NotFound)?)
    }
}
