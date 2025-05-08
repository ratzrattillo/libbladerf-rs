#![allow(dead_code)]
use anyhow::Result;
use nusb::{Device, DeviceInfo};

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
}

pub struct UsbBackend {}
impl UsbBackend {
    pub async fn list_devices(&self) -> Result<impl Iterator<Item = DeviceInfo>> {
        Ok(nusb::list_devices().await?)
    }
    pub async fn find_by_bus_addr(&self, bus_number: u8, address: u8) -> Result<DeviceInfo> {
        Ok(self
            .list_devices()
            .await?
            .find(|dev| dev.busnum() == bus_number && dev.device_address() == address)
            .ok_or(BackendError::NotFound)?)
    }

    pub async fn find_by_serial(&self, serial: &str) -> Result<DeviceInfo> {
        Ok(self
            .list_devices()
            .await?
            .find(|dev| dev.serial_number() == Some(serial))
            .ok_or(BackendError::NotFound)?)
    }

    pub async fn open_by_device_info(&self, info: DeviceInfo) -> Result<Device> {
        Ok(info.open().await?)
    }

    pub async fn open_by_bus_addr(&self, bus_number: u8, address: u8) -> Result<Device> {
        Ok(self
            .find_by_bus_addr(bus_number, address)
            .await?
            .open()
            .await?)
    }

    pub async fn open_by_serial(&self, serial: &str) -> Result<Device> {
        Ok(self.find_by_serial(serial).await?.open().await?)
    }

    pub async fn open_by_fd(&self, fd: std::os::fd::OwnedFd) -> Result<Device> {
        Ok(Device::from_fd(fd).await?)
    }
}
