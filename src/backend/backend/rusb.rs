#![allow(dead_code)]

use crate::backend::{BackendError, UsbBackend, UsbDeviceInfoTrait, UsbDeviceTrait, UsbInterfaceInfoTrait, UsbSpeed, UsbVersion};
use anyhow::Result;
use rusb::{
    ConfigDescriptor, Device, DeviceDescriptor, DeviceHandle, DeviceList, GlobalContext,
    InterfaceDescriptor, Language, Speed, Version,
};
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum RusbError {
    /// I/O error occurred.
    #[error("libsusb")]
    Rusb(#[from] rusb::Error),
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

// impl TryFrom<Device<GlobalContext>> for UsbDeviceInfo {
//     type Error = RusbError;
//
//     fn try_from(value: Device<GlobalContext>) -> std::result::Result<Self, Self::Error> {
//         let timeout = Duration::from_secs(1);
//         let dev_desc = value.device_descriptor()?;
//
//         let mut info = RusbDeviceInfo {
//             //bus_number: value.bus_number(),
//             //device_address: value.address(),
//             //vendor_id: dev_desc.vendor_id(),
//             //product_id: dev_desc.product_id(),
//             //device_version: 0, // dev_desc.device_version(),
//             //class: dev_desc.class_code(),
//             //subclass: dev_desc.sub_class_code(),
//             //protocol: dev_desc.protocol_code(),
//             //speed: None,
//             device_descriptor: (),
//             device_info: (),
//             manufacturer_string: None,
//             product_string: None,
//             serial_number: None,
//             //interfaces: Vec::<UsbInterfaceInfo>::new(),
//         };
//
//         info.speed = UsbSpeed::try_from(value.speed()).ok();
//
//         if let Ok(handle) = value.open() {
//             info.manufacturer_string = handle.read_manufacturer_string_ascii(&dev_desc).ok();
//             info.product_string = handle.read_product_string_ascii(&dev_desc).ok();
//             info.serial_number = handle.read_serial_number_string_ascii(&dev_desc).ok();
//
//             let opt_usb_device = handle
//                 .read_languages(timeout)
//                 .ok()
//                 .map(|languages| RusbDevice {
//                     handle,
//                     language: languages[0],
//                     timeout,
//                 });
//
//             if let Some(usb_device) = opt_usb_device {
//                 for n in 0..dev_desc.num_configurations() {
//                     if let Some(config_desc) = value.config_descriptor(n).ok() {
//                         for interface in config_desc.interfaces() {
//                             for interface_descriptor in interface.descriptors() {
//                                 let if_info = UsbInterfaceInfo {
//                                     interface_number: interface_descriptor.interface_number(),
//                                     class: interface_descriptor.class_code(),
//                                     subclass: interface_descriptor.sub_class_code(),
//                                     protocol: interface_descriptor.protocol_code(),
//                                     interface_string: usb_device
//                                         .handle
//                                         .read_interface_string(
//                                             usb_device.language,
//                                             &interface_descriptor,
//                                             usb_device.timeout,
//                                         )
//                                         .ok(),
//                                 };
//                                 info.interfaces.push(if_info);
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//
//         Ok(info)
//     }
// }

impl TryFrom<Device<GlobalContext>> for RusbDeviceInfo {
    type Error = RusbError;

    fn try_from(value: Device<GlobalContext>) -> std::result::Result<Self, Self::Error> {
        let dev_desc = value.device_descriptor()?;
        let mut info = Self {
            device_descriptor: dev_desc,
            config_descriptors: vec![],
            device_info: value,
            manufacturer_string: None,
            product_string: None,
            serial_number: None,
            languages: vec![],
        };

        for n in 0..info.device_descriptor.num_configurations() {
            if let Ok(config_desc) = info.device_info.config_descriptor(n) {
                info.config_descriptors.push(config_desc);
            }
        }

        if let Ok(device_handle) = info.device_info.open() {
            info.manufacturer_string = device_handle
                .read_manufacturer_string_ascii(&info.device_descriptor)
                .ok();
            info.product_string = device_handle
                .read_product_string_ascii(&info.device_descriptor)
                .ok();
            info.serial_number = device_handle
                .read_serial_number_string_ascii(&info.device_descriptor)
                .ok();
            if let Ok(languages) = device_handle.read_languages(Duration::from_secs(1)) {
                info.languages = languages;
            }
        }

        Ok(info)
    }
}

impl From<Version> for UsbVersion {
    fn from(value: Version) -> Self {
        Self {
            major: value.major(),
            minor: value.minor(),
            sub_minor: value.sub_minor(),
        }
    }
}

struct RusbDeviceInfo {
    device_descriptor: DeviceDescriptor,
    config_descriptors: Vec<ConfigDescriptor>,
    device_info: Device<GlobalContext>,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    languages: Vec<Language>,
    //interfaces: Interfaces<'static>,
}

// struct RusbDevice<T: UsbContext> {
//     handle: DeviceHandle<T>,
//     language: Language,
//     timeout: Duration,
// }

#[derive(Clone)]
pub struct RusbBackend {}

impl RusbBackend {}

impl UsbBackend for RusbBackend {
    fn list_devices(&self) -> Result<impl Iterator<Item = Box<dyn UsbDeviceInfoTrait>>> {
        let mut devices = Vec::<Box<dyn UsbDeviceInfoTrait>>::new();
        for dev in DeviceList::new()?.iter() {
            let info = RusbDeviceInfo::try_from(dev)?;
            //let info = UsbDeviceInfo::try_from(dev)?;
            devices.push(Box::new(info));
        }
        Ok(devices.into_iter())
    }

    // /// Open device by DevInfo Struct
    // #[cfg(any(target_os = "android", target_os = "linux"))]
    // fn open_by_device_info(&self, info: UsbDeviceInfo) -> Result<RusbDevice<T>> {
    //     let dev_info = Device::<GlobalContext>::try_from(info)?;
    //     Ok(dev_info.open()?)
    // }
}

impl UsbDeviceInfoTrait for RusbDeviceInfo {
    fn open(&self) -> Result<Box<dyn UsbDeviceTrait>> {
        Ok(Box::new(self.device_info.open()?))
    }

    fn class(&self) -> u8 {
        self.device_descriptor.class_code()
    }

    fn subclass(&self) -> u8 {
        self.device_descriptor.sub_class_code()
    }

    fn protocol(&self) -> u8 {
        self.device_descriptor.protocol_code()
    }

    fn product_string(&self) -> Option<&str> {
        self.product_string.as_deref()
    }

    fn manufacturer_string(&self) -> Option<&str> {
        self.manufacturer_string.as_deref()
    }

    fn serial_number(&self) -> Option<&str> {
        self.serial_number.as_deref()
    }

    fn device_version(&self) -> UsbVersion {
        UsbVersion::from(self.device_descriptor.device_version())
    }

    fn vendor_id(&self) -> u16 {
        self.device_descriptor.vendor_id()
    }

    fn product_id(&self) -> u16 {
        self.device_descriptor.product_id()
    }

    fn device_address(&self) -> u8 {
        self.device_info.address()
    }

    fn bus_number(&self) -> u8 {
        self.device_info.bus_number()
    }

    // fn interfaces(&self) -> impl Iterator<Item = Box<dyn UsbInterfaceInfoTrait>> {
    //     self.config_descriptors
    //         .iter()
    //         .map(|cfg_desc| cfg_desc.interfaces())
    //         .flatten()
    //         .map(|interface| {
    //             interface
    //                 .descriptors()
    //                 .map(|interface_descriptor| Box::new(interface_descriptor))
    //         })
    //         .flatten().collect::<Vec<_>>()
    //
    // }
}

impl UsbInterfaceInfoTrait for InterfaceDescriptor<'_> {
    fn class(&self) -> u8 {
        self.class_code()
    }

    fn subclass(&self) -> u8 {
        self.sub_class_code()
    }

    fn protocol(&self) -> u8 {
        self.protocol_code()
    }

    fn interface_number(&self) -> u8 {
        self.interface_number()
    }

    // fn interface_string(&self) -> Option<&str> {
    //     usb_device
    //         .handle
    //         .read_interface_string(
    //             usb_device.language,
    //             &interface_descriptor,
    //             usb_device.timeout,
    //         )
    //         .ok(),
    // }
}
impl UsbDeviceTrait for DeviceHandle<GlobalContext> {
    fn test_me(&self) {
        //self.detach_kernel_driver(0).unwrap();
        self.claim_interface(0).unwrap();
        // self.attach_kernel_driver(0).unwrap()
    }
    fn reset(&self) -> Result<()> {
        Ok(self.reset()?)
    }
}
