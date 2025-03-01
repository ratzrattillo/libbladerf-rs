use crate::backend::{BackendError, UsbBackend, UsbDeviceInfoTrait, UsbDeviceTrait, UsbInterfaceInfoTrait, UsbSpeed, UsbVersion};
use anyhow::Result;
use nusb::{DeviceInfo, InterfaceInfo};
use nusb::{Device, Speed};

#[derive(thiserror::Error, Debug)]
pub enum NusbError {
    /// I/O error occurred.
    #[error("nusb")]
    Nusb(#[from] nusb::Error),
}

impl TryFrom<Speed> for UsbSpeed {
    type Error = BackendError;

    fn try_from(value: Speed) -> std::result::Result<Self, Self::Error> {
        match value {
            Speed::Low => Ok(UsbSpeed::Low),
            Speed::Full => Ok(UsbSpeed::Full),
            Speed::High => Ok(UsbSpeed::High),
            Speed::Super => Ok(UsbSpeed::Super),
            Speed::SuperPlus => Ok(UsbSpeed::SuperPlus),
            _ => Err(BackendError::ConversionError),
        }
    }
}

// impl TryFrom<DeviceInfo> for UsbDeviceInfo {
//     type Error = BackendError;
//
//     fn try_from(value: DeviceInfo) -> std::result::Result<Self, Self::Error> {
//         let interfaces = value
//             .interfaces()
//             .map(|if_info| UsbInterfaceInfo {
//                 interface_number: if_info.interface_number(),
//                 class: if_info.class(),
//                 subclass: if_info.subclass(),
//                 protocol: if_info.protocol(),
//                 interface_string: if_info.interface_string().map(|s| s.to_string()),
//             })
//             .collect::<Vec<_>>();
//
//         let mut info = UsbDeviceInfo {
//             bus_number: value.bus_number(),
//             device_address: value.device_address(),
//             vendor_id: value.vendor_id(),
//             product_id: value.product_id(),
//             device_version: value.device_version(),
//             class: value.class(),
//             subclass: value.subclass(),
//             protocol: value.protocol(),
//             speed: None,
//             manufacturer_string: None,
//             product_string: None,
//             serial_number: None,
//             interfaces,
//         };
//
//         if let Some(speed) = value.speed() {
//             info.speed = UsbSpeed::try_from(speed).ok();
//         };
//         if let Some(manufacturer_string) = value.manufacturer_string() {
//             info.manufacturer_string = Some(manufacturer_string.to_string())
//         }
//         if let Some(product_string) = value.product_string() {
//             info.product_string = Some(product_string.to_string())
//         }
//         if let Some(serial_number) = value.serial_number() {
//             info.serial_number = Some(serial_number.to_string())
//         }
//
//         Ok(info)
//     }
// }

#[derive(Clone)]
pub struct NusbBackend {}

impl NusbBackend {}

impl UsbBackend for NusbBackend {
    fn list_devices(&self) -> Result<impl Iterator<Item = Box<dyn UsbDeviceInfoTrait>>> {
        let mut devices = Vec::<Box<dyn UsbDeviceInfoTrait>>::new();

        for device_info in nusb::list_devices()? {
            devices.push(Box::new(device_info));
        }
        Ok(devices.into_iter())
    }
}

impl UsbDeviceInfoTrait for DeviceInfo {
    fn open(&self) -> Result<Box<dyn UsbDeviceTrait>> {
        Ok(Box::new(self.open()?))
    }

    fn class(&self) -> u8 {
        self.class()
    }

    fn subclass(&self) -> u8 {
        self.subclass()
    }

    fn protocol(&self) -> u8 {
        self.protocol()
    }

    fn product_string(&self) -> Option<&str> {
        self.product_string()
    }

    fn manufacturer_string(&self) -> Option<&str> {
        self.manufacturer_string()
    }

    fn serial_number(&self) -> Option<&str> {
        self.serial_number()
    }

    fn device_version(&self) -> UsbVersion {
        UsbVersion::from_bcd(self.device_version())
    }

    fn vendor_id(&self) -> u16 {
        self.vendor_id()
    }

    fn product_id(&self) -> u16 {
        self.product_id()
    }

    fn device_address(&self) -> u8 {
        self.device_address()
    }

    fn bus_number(&self) -> u8 {
        self.bus_number()
    }

    // fn interfaces(&self) -> impl Iterator<Item = Box<dyn UsbInterfaceInfoTrait>> {
    //     self.interfaces()
    // }
}

impl UsbInterfaceInfoTrait for InterfaceInfo {
    fn class(&self) -> u8 {
        self.class()
    }

    fn subclass(&self) -> u8 {
        self.subclass()
    }

    fn protocol(&self) -> u8 {
        self.protocol()
    }

    fn interface_number(&self) -> u8 {
        self.interface_number()
    }

    // fn interface_string(&self) -> Option<&str> {
    //     self.interface_string()
    // }
}

impl UsbDeviceTrait for Device {
    fn test_me(&self) {
        self.attach_kernel_driver(0).unwrap()
    }
    fn reset(&self) -> Result<()> {
        Ok(self.reset()?)
    }
}
