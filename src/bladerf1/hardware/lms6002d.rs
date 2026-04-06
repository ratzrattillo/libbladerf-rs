pub mod bandwidth;
pub mod dc_calibration;
pub mod filters;
pub mod frequency;
pub mod gain;
pub mod loopback;
use crate::bladerf1::hardware::lms6002d::bandwidth::LmsBandwidth;
use crate::bladerf1::hardware::lms6002d::gain::{LmsLowNoiseAmplifier, LmsPowerAmplifier};
use crate::bladerf1::hardware::lms6002d::loopback::Loopback;
use crate::bladerf1::nios_client::NiosInterface;
use crate::channel::Channel;
use crate::error::Result;
use crate::protocol::nios::NiosPkt8x8Target;
use std::sync::{Arc, Mutex};
#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Band {
    Low = 0,
    High = 1,
}
#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Tune {
    Normal = 0,
    Quick = 1,
}
pub const LMS_REG_DUMPSET: [u8; 107] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0E, 0x0F, 0x10, 0x11,
    0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21,
    0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31,
    0x32, 0x33, 0x34, 0x35, 0x36, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A,
    0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A,
    0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x70, 0x71,
    0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C,
];
pub const VCOCAP_MAX_VALUE: u8 = 0x3f;
pub const VCOCAP_EST_MIN: u8 = 15;
pub const VCOCAP_EST_MAX: u8 = 55;
pub const VCOCAP_EST_RANGE: u8 = VCOCAP_EST_MAX - VCOCAP_EST_MIN;
pub const VCOCAP_EST_THRESH: u8 = 7;
pub const LMS_FREQ_FLAGS_LOW_BAND: u8 = 1 << 0;
pub const LMS_FREQ_FLAGS_FORCE_VCOCAP: u8 = 1 << 1;
pub const LMS_FREQ_XB_200_ENABLE: u8 = 1 << 7;
pub const LMS_FREQ_XB_200_MODULE_RX: u8 = 1 << 6;
pub const LMS_FREQ_XB_200_FILTER_SW: u8 = 3 << 4;
pub const LMS_FREQ_XB_200_FILTER_SW_SHIFT: u8 = 4;
pub const LMS_FREQ_XB_200_PATH: u8 = 3 << 2;
pub const LMS_FREQ_XB_200_PATH_SHIFT: u8 = 2;
pub const VTUNE_DELAY_LARGE: u8 = 50;
pub const VTUNE_DELAY_SMALL: u8 = 25;
pub const VTUNE_MAX_ITERATIONS: u8 = 20;
pub const VCO_HIGH: u8 = 0x02;
pub const VCO_NORM: u8 = 0x00;
pub const VCO_LOW: u8 = 0x01;
pub const VCOCAP_MAX_LOW_HIGH: u8 = 12;
#[allow(dead_code)]
pub struct LmsTransceiverConfig {
    tx_freq_hz: u32,
    rx_freq_hz: u32,
    loopback_mode: Loopback,
    lna: LmsLowNoiseAmplifier,
    pa: LmsPowerAmplifier,
    tx_bw: LmsBandwidth,
    rx_bw: LmsBandwidth,
}
#[derive(Clone)]
pub struct LMS6002D {
    interface: Arc<Mutex<NiosInterface>>,
}
impl LMS6002D {
    pub fn new(interface: Arc<Mutex<NiosInterface>>) -> Self {
        Self { interface }
    }
    pub fn read(&self, addr: u8) -> Result<u8> {
        self.interface
            .lock()
            .unwrap()
            .nios_read::<u8, u8>(NiosPkt8x8Target::Lms6, addr)
    }
    pub fn write(&self, addr: u8, data: u8) -> Result<()> {
        self.interface
            .lock()
            .unwrap()
            .nios_write::<u8, u8>(NiosPkt8x8Target::Lms6, addr, data)
    }
    pub fn set(&self, addr: u8, mask: u8) -> Result<()> {
        let data = self.read(addr)?;
        self.write(addr, data | mask)
    }
    pub fn clear(&self, addr: u8, mask: u8) -> Result<()> {
        let data = self.read(addr)?;
        self.write(addr, data & !mask)
    }
    pub fn soft_reset(&self) -> Result<()> {
        self.write(0x05, 0x12)?;
        self.write(0x05, 0x32)
    }
    pub fn enable_rffe(&self, channel: Channel, enable: bool) -> Result<()> {
        let (addr, shift) = if channel == Channel::Tx {
            (0x40u8, 1u8)
        } else {
            (0x70u8, 0u8)
        };
        let mut data = self.read(addr)?;
        if enable {
            data |= 1 << shift;
        } else {
            data &= !(1 << shift);
        }
        self.write(addr, data)
    }
    pub fn select_band(&self, channel: Channel, band: Band) -> Result<()> {
        if self.is_loopback_enabled()? {
            log::debug!("Loopback enabled!");
            return Ok(());
        }
        match channel {
            Channel::Tx => {
                let lms_pa = if band == Band::Low {
                    LmsPowerAmplifier::Pa1
                } else {
                    LmsPowerAmplifier::Pa2
                };
                self.select_pa(lms_pa)
            }
            Channel::Rx => {
                let lms_lna = if band == Band::Low {
                    LmsLowNoiseAmplifier::Lna1
                } else {
                    LmsLowNoiseAmplifier::Lna2
                };
                self.select_lna(lms_lna)
            }
        }
    }
}
