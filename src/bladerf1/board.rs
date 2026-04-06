mod bandwidth;
mod corrections;
mod frequency;
mod gain;
mod loopback;
mod rx_mux;
mod sample_rate;
mod stream;
pub mod xb;
use crate::bladerf1::hardware::dac161s055::DAC161S055;
use crate::bladerf1::hardware::lms6002d::{Band, LMS6002D};
use crate::bladerf1::hardware::si5338::SI5338;
use crate::bladerf1::nios_client::NiosInterface;
use crate::channel::Channel;
use crate::error::Error;
use crate::transport::usb::{
    BladeRf1DeviceCommands, BladeRf1UsbInterfaceCommands, DeviceCommands, UsbInterfaceCommands,
};
pub use corrections::Correction;
pub use frequency::TuningMode;
pub use gain::GainMode;
use nusb::transfer::{Buffer, Bulk, In, Out};
use nusb::{Device, DeviceInfo, Endpoint, MaybeFuture, Speed};
pub use rx_mux::RxMux;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
pub use stream::SampleFormat;
pub const BLADERF1_USB_VID: u16 = 0x2CF0;
pub const BLADERF1_USB_PID: u16 = 0x5246;
pub const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;
#[derive(Clone)]
struct BoardData {
    tuning_mode: TuningMode,
}
pub struct BladeRf1RxStreamer {
    device: BladeRf1,
    endpoint: Endpoint<Bulk, In>,
    available: VecDeque<Buffer>,
    completed: VecDeque<Buffer>,
    in_flight_count: usize,
    buffer_size: usize,
    format: SampleFormat,
    is_active: bool,
}
pub struct BladeRf1TxStreamer {
    device: BladeRf1,
    endpoint: Endpoint<Bulk, Out>,
    available: VecDeque<Buffer>,
    completed: VecDeque<Buffer>,
    in_flight_count: usize,
    buffer_size: usize,
    format: SampleFormat,
    is_active: bool,
}
#[derive(Clone)]
pub struct BladeRf1 {
    device: Device,
    pub interface: Arc<Mutex<NiosInterface>>,
    board_data: BoardData,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
}
impl BladeRf1 {
    #[allow(dead_code)]
    #[inline]
    fn with_interface<F, T>(&self, f: F) -> crate::Result<T>
    where
        F: FnOnce(&mut NiosInterface) -> crate::Result<T>,
    {
        f(&mut self.interface.lock().expect("interface mutex poisoned"))
    }
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
        let interface = Arc::new(Mutex::new(NiosInterface::from(
            device.detach_and_claim_interface(0).wait()?,
        )));
        let board_data = BoardData {
            tuning_mode: TuningMode::Fpga,
        };
        if device.speed().ok_or(Error::Invalid)? < Speed::High {
            log::error!("BladeRF requires High/Super/SuperPlus speeds");
            return Err(Error::Invalid);
        }
        let lms = LMS6002D::new(interface.clone());
        let si5338 = SI5338::new(interface.clone());
        let dac = DAC161S055::new(interface.clone());
        Ok(Self {
            device,
            interface,
            board_data,
            lms,
            si5338,
            dac,
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
    pub fn fx3_firmware_version(&self) -> crate::Result<String> {
        self.device.fx3_firmware_version()
    }
    pub fn fpga_version(&self) -> crate::Result<String> {
        let version = self.interface.lock().unwrap().nios_get_fpga_version()?;
        Ok(format!("{version}"))
    }
    pub(crate) fn config_gpio_read(&self) -> crate::Result<u32> {
        self.interface.lock().unwrap().nios_config_read()
    }
    pub(crate) fn config_gpio_write(&self, mut data: u32) -> crate::Result<()> {
        log::trace!("[config_gpio_write] data: {data}");
        let speed = self.device.speed().ok_or(Error::Invalid)?;
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
                return Err(Error::Invalid);
            }
        }
        log::trace!("[config_gpio_write] data after speed check: {data}");
        self.interface.lock().unwrap().nios_config_write(data)
    }
    pub fn initialize(&self) -> crate::Result<()> {
        let alt_setting = self.interface.lock().unwrap().get_alt_setting();
        log::trace!("[*] Init - Default Alt Setting {alt_setting}");
        self.interface.lock().unwrap().usb_change_setting(0x01)?;
        log::trace!("[*] Init - Set Alt Setting to 0x01");
        let cfg = self.config_gpio_read()?;
        if (cfg & 0x7f) == 0 {
            log::trace!("[*] Init - Default GPIO value \"{cfg}\" found - initializing device");
            self.config_gpio_write(0x57)?;
            log::trace!("[*] Init - Disabling RX and TX Frontend");
            self.lms.enable_rffe(Channel::Tx, false)?;
            log::trace!("Channel::Tx");
            self.lms.enable_rffe(Channel::Rx, false)?;
            log::trace!("Channel::Rx");
            log::trace!("[*] Init - Set LMS register to enable RX and TX");
            self.lms.write(0x05, 0x3e)?;
            log::trace!("[*] Init - Set LMS register to enable RX and TX");
            self.lms.write(0x47, 0x40)?;
            log::trace!("[*] Init - Set register to improve ADC performance");
            self.lms.write(0x59, 0x29)?;
            log::trace!("[*] Init - Set Common mode voltage for ADC");
            self.lms.write(0x64, 0x36)?;
            log::trace!("[*] Init - Set Higher LNA Gain");
            self.lms.write(0x79, 0x37)?;
            log::trace!("[*] Init - Power down TX LPF DC cal comparator");
            self.lms.set(0x3f, 0x80)?;
            log::debug!("[*] Init - Power down RX LPF DC cal comparator");
            self.lms.set(0x5f, 0x80)?;
            log::trace!("[*] Init - Power down RXVGA2A/B DC cal comparators");
            self.lms.set(0x6e, 0xc0)?;
            log::trace!("[*] Init - Configure TX charge pump current offsets");
            self.lms.config_charge_pumps(Channel::Tx)?;
            log::trace!("[*] Init - Configure RX charge pump current offsets");
            self.lms.config_charge_pumps(Channel::Rx)?;
            log::trace!("[*] Init - Set TX SampleRate");
            let _actual_tx = self.si5338.set_sample_rate(Channel::Tx, 1000000)?;
            log::trace!("[*] Init - Set RX SampleRate");
            let _actual_rx = self.si5338.set_sample_rate(Channel::Rx, 1000000)?;
            log::trace!("self.set_frequency(Channel::Tx, 2447000000)?;");
            self.set_frequency(Channel::Tx, 2447000000)?;
            log::trace!("self.set_frequency(Channel::Rx, 2484000000)?;");
            self.set_frequency(Channel::Rx, 2484000000)?;
            self.dac.write(0)?;
            self.set_gain_mode(Channel::Rx, GainMode::Mgc)?;
        } else {
            log::trace!("[*] Init - Device already initialized: {cfg:#04x}");
        }
        Ok(())
    }
    pub fn enable_module(&self, channel: Channel, enable: bool) -> crate::Result<()> {
        if !enable {
            self.perform_format_deconfig(channel)?;
        }
        self.lms.enable_rffe(channel, enable)?;
        self.interface
            .lock()
            .unwrap()
            .usb_enable_module(channel, enable)
    }
    pub fn band_select(&self, channel: Channel, band: Band) -> crate::Result<()> {
        let band_value = match band {
            Band::Low => 2,
            Band::High => 1,
        };
        log::trace!("Selecting %s band. {band:?}");
        self.lms.select_band(channel, band)?;
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
