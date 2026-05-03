mod bandwidth;
mod calibration;
pub(crate) mod corrections;
mod dac_trim;
mod frequency;
mod gain;
mod loopback;
pub use loopback::Loopback;
pub(crate) mod rx_mux;
mod sample_rate;
pub mod stream;
pub mod xb;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::nios_client::NiosClient;
use crate::channel::Channel;
use crate::error::Error;
use crate::transport::usb::{
    BladeRf1DeviceCommands, BladeRf1UsbInterfaceCommands, DeviceCommands, UsbInterfaceCommands,
};
pub use corrections::Correction;
pub use frequency::TuningMode;
pub use gain::GainMode;
use nusb::transfer::{In, Out};
use nusb::{Device, DeviceInfo, MaybeFuture, Speed};
pub use rx_mux::RxMux;
pub use stream::{
    BLADERF_GPIO_8BIT_MODE, BLADERF_GPIO_HIGHLY_PACKED_MODE, BLADERF_GPIO_PACKET,
    BLADERF_GPIO_TIMESTAMP, BLADERF_GPIO_TIMESTAMP_DIV2, METADATA_HEADER_SIZE, MetadataHeader,
    RxStreamBuilder, SampleFormat, TxStreamBuilder,
};
pub const BLADERF1_USB_VID: u16 = 0x2CF0;
pub const BLADERF1_USB_PID: u16 = 0x5246;
pub const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;
pub struct RxStream {
    pool: stream::BufferPool<In>,
}
pub struct TxStream {
    pool: stream::BufferPool<Out>,
}
pub struct BladeRf1 {
    device: Device,
    nios: NiosClient,
    chunk_size: usize,
}
impl BladeRf1 {
    pub fn list_bladerf1() -> crate::Result<impl Iterator<Item = DeviceInfo>> {
        Ok(nusb::list_devices().wait()?.filter(|dev: &DeviceInfo| {
            dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID
        }))
    }
    fn build(device: Device) -> crate::Result<Self> {
        log::debug!("Manufacturer: {}", device.manufacturer()?);
        log::debug!("Product: {}", device.product()?);
        log::debug!("Serial: {}", device.serial()?);
        log::debug!("Speed: {:?}", device.speed());
        log::debug!("Languages: {:x?}", device.get_supported_languages()?);
        let speed = device.speed().ok_or(Error::UnsupportedSpeed)?;
        if speed < Speed::High {
            log::error!("BladeRF requires High/Super/SuperPlus speeds");
            return Err(Error::UnsupportedSpeed);
        }
        let nios = NiosClient::from(device.detach_and_claim_interface(0).wait()?);
        let chunk_size = crate::bladerf1::hardware::spi_flash::get_chunk_size(speed)?;
        Ok(Self {
            device,
            nios,
            chunk_size,
        })
    }
    pub fn from_first() -> crate::Result<Self> {
        let device = Self::list_bladerf1()?
            .next()
            .ok_or(Error::NotFound)?
            .open()
            .wait()?;
        Self::build(device)
    }
    pub fn from_serial(serial: &str) -> crate::Result<Self> {
        let device = Self::list_bladerf1()?
            .find(|dev| dev.serial_number() == Some(serial))
            .ok_or(Error::NotFound)?
            .open()
            .wait()?;
        Self::build(device)
    }
    pub fn from_bus_addr(bus_number: &str, bus_addr: u8) -> crate::Result<Self> {
        let device = Self::list_bladerf1()?
            .find(|dev| dev.bus_id() == bus_number && dev.device_address() == bus_addr)
            .ok_or(Error::NotFound)?
            .open()
            .wait()?;
        Self::build(device)
    }
    #[cfg(target_os = "linux")]
    pub fn from_fd(fd: std::os::fd::OwnedFd) -> crate::Result<Self> {
        let device = Device::from_fd(fd).wait()?;
        Self::build(device)
    }
    pub fn serial(&self) -> crate::Result<String> {
        self.device.serial()
    }

    pub fn speed(&self) -> crate::Result<Speed> {
        self.device.speed().ok_or(Error::UnsupportedSpeed)
    }

    pub fn fx3_firmware_version(&self) -> crate::Result<String> {
        self.device.fx3_firmware_version()
    }
    pub fn fpga_version(&mut self) -> crate::Result<String> {
        let version = self.nios.nios_get_fpga_version()?;
        Ok(format!("{version}"))
    }
    pub fn config_gpio_read(&mut self) -> crate::Result<u32> {
        self.nios.nios_config_read()
    }
    pub fn config_gpio_write(&mut self, mut data: u32) -> crate::Result<()> {
        log::trace!("[config_gpio_write] data: {data}");
        let speed = self.device.speed().ok_or(Error::UnsupportedSpeed)?;
        log::trace!("[config_gpio_write] speed: {speed:?}");
        match speed {
            Speed::High => {
                data |= BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
            Speed::Super => {
                data &= !(BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32);
            }
            _ => {
                log::error!("speed {speed:?} not supported");
                return Err(Error::UnsupportedSpeed);
            }
        }
        log::trace!("[config_gpio_write] data after speed check: {data}");
        self.nios.nios_config_write(data)
    }
    pub fn initialize(&mut self, force: bool) -> crate::Result<()> {
        let alt_setting = self.nios.get_alt_setting();
        log::trace!("[*] Init - Default Alt Setting {alt_setting}");
        if alt_setting != 1 {
            self.nios.usb_change_setting(0x01)?;
            log::trace!("[*] Init - Set Alt Setting to 0x01");
        }
        let cfg = self.config_gpio_read()?;
        if force || (cfg & 0x7f) == 0 {
            log::trace!(
                "[*] Init - {}initializing device (GPIO={cfg:#04x})",
                if force { "Force " } else { "" }
            );
            self.config_gpio_write(0x57)?;
            lms6002d::enable_rffe(&mut self.nios, Channel::Tx, false)?;
            lms6002d::enable_rffe(&mut self.nios, Channel::Rx, false)?;
            lms6002d::write(&mut self.nios, 0x05, 0x3e)?;
            lms6002d::write(&mut self.nios, 0x47, 0x40)?;
            lms6002d::write(&mut self.nios, 0x59, 0x29)?;
            lms6002d::write(&mut self.nios, 0x64, 0x36)?;
            lms6002d::write(&mut self.nios, 0x79, 0x37)?;
            lms6002d::set(&mut self.nios, 0x3f, 0x80)?;
            lms6002d::set(&mut self.nios, 0x5f, 0x80)?;
            lms6002d::set(&mut self.nios, 0x6e, 0xc0)?;
            lms6002d::frequency::config_charge_pumps(&mut self.nios, Channel::Tx)?;
            lms6002d::frequency::config_charge_pumps(&mut self.nios, Channel::Rx)?;
            let _actual_tx = crate::bladerf1::hardware::si5338::set_sample_rate(
                &mut self.nios,
                Channel::Tx,
                1_000_000,
            )?;
            let _actual_rx = crate::bladerf1::hardware::si5338::set_sample_rate(
                &mut self.nios,
                Channel::Rx,
                1_000_000,
            )?;
            self.set_frequency(Channel::Tx, 2_447_000_000, TuningMode::Fpga)?;
            self.set_frequency(Channel::Rx, 2_484_000_000, TuningMode::Fpga)?;
            crate::bladerf1::hardware::dac161s055::write(&mut self.nios, 0)?;
            self.set_gain_mode(Channel::Rx, GainMode::Mgc)?;
        } else {
            log::trace!("[*] Init - Device already initialized: {cfg:#04x}");
        }
        Ok(())
    }
    pub fn enable_module(&mut self, channel: Channel, enable: bool) -> crate::Result<()> {
        if !enable {
            self.perform_format_deconfig(channel)?;
        }
        lms6002d::enable_rffe(&mut self.nios, channel, enable)?;
        self.nios.usb_enable_module(channel, enable)
    }
    pub fn band_select(&mut self, channel: Channel, band: lms6002d::Band) -> crate::Result<()> {
        let band_value = match band {
            lms6002d::Band::Low => 2,
            lms6002d::Band::High => 1,
        };
        log::trace!("Selecting {band:?} band");
        lms6002d::select_band(&mut self.nios, channel, band)?;
        let mut gpio = self.config_gpio_read()?;
        let shift = if channel == Channel::Tx {
            3 << 3
        } else {
            3 << 5
        };
        gpio &= !shift;
        let shift = if channel == Channel::Tx {
            band_value << 3
        } else {
            band_value << 5
        };
        gpio |= shift;
        self.config_gpio_write(gpio)
    }
}
