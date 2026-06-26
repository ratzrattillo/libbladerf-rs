//! NiosCore — central register I/O hub.
//!
//! All SPI/I2C bridge commands and NIOS packet operations flow through
//! `NiosCore`. It wraps the USB transport and tracks the
//! `active_streams` counter to prevent USB alternate setting changes
//! while streaming endpoints are active.

use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::bladerf1::protocol::{nios_decode_retune, nios_encode_retune};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::packet_generic::NiosNum;
use crate::protocol::nios::targets::NiosPkt8x16AddrAgcCorr;
use crate::protocol::nios::{
    NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target, NiosPkt8x64Target,
    NiosPkt8x64TimestampAddr, NiosPkt32x32Target, nios_decode_read, nios_decode_write,
    nios_encode_read, nios_encode_write,
};
use crate::usb::UsbTransport;
use crate::usb::{
    BladeRf1UsbInterfaceCommands, UsbAltSetting, UsbInterfaceCommands, VendorRequest,
};
use crate::version::SemanticVersion;
use std::time::Duration;

/// Central NIOS register I/O hub.
///
/// Wraps a `UsbTransport` and provides typed methods for all NIOS
/// register access, including config GPIO, expansion GPIO, IQ/AGC
/// corrections, FPGA version queries, and retune commands. Tracks
/// the active stream count to guard USB alt setting transitions.
pub struct NiosCore {
    /// The underlying USB transport for device communication.
    transport: UsbTransport,
    /// Number of active RX/TX streams. Prevents alt setting changes when > 0.
    active_streams: u8,
}
impl NiosCore {
    /// Creates a new `NiosCore` wrapping the given USB transport.
    pub fn new(transport: UsbTransport) -> Self {
        Self {
            transport,
            active_streams: 0,
        }
    }
    /// Returns a shared reference to the underlying `UsbTransport`.
    pub fn transport(&self) -> &UsbTransport {
        &self.transport
    }
    /// Returns a mutable reference to the underlying `UsbTransport`.
    pub fn transport_mut(&mut self) -> &mut UsbTransport {
        &mut self.transport
    }
    /// Returns the current number of active streams.
    pub(crate) fn active_streams(&self) -> u8 {
        self.active_streams
    }
    /// Increments the active stream counter. Called when a stream is built.
    pub(crate) fn stream_started(&mut self) {
        self.active_streams += 1;
    }
    /// Decrements the active stream counter. Called when a stream is closed.
    pub(crate) fn stream_stopped(&mut self) {
        self.active_streams -= 1;
    }
    /// Issues a generic NIOS register read.
    ///
    /// Encodes a read packet for the given `id` and `addr`, submits it
    /// via USB bulk transfer, and decodes the response data.
    pub fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
    ) -> Result<D> {
        let out_buf = self.transport.out_buffer()?;
        log::trace!("nios_read: DMA buffer len = {} bytes", out_buf.len());
        nios_encode_read::<A, D>(out_buf, id.into(), addr)?;
        let response = self.transport.submit(None)?;
        log::trace!("nios_read: response len = {} bytes", response.len());
        nios_decode_read::<A, D>(response)
    }
    /// Issues a generic NIOS register write.
    ///
    /// Encodes a write packet for the given `id`, `addr`, and `data`,
    /// submits it via USB bulk transfer, and verifies the success status.
    pub fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
        data: D,
    ) -> Result<()> {
        let out_buf = self.transport.out_buffer()?;
        nios_encode_write::<A, D>(out_buf, id.into(), addr, data)?;
        let response = self.transport.submit(None)?;
        nios_decode_write::<A, D>(response)
    }
    /// Reads the config GPIO register.
    pub fn nios_config_read(&mut self) -> Result<u32> {
        self.nios_read::<u8, u32>(NiosPkt8x32Target::Control, 0)
    }
    /// Writes the config GPIO register.
    pub fn nios_config_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Control, 0, value)
    }
    /// Performs an atomic read-modify-write on the config GPIO register.
    ///
    /// Reads the current value, applies `f`, then writes the result back.
    pub fn nios_config_modify(&mut self, f: impl FnOnce(u32) -> u32) -> Result<()> {
        let data = self.nios_config_read()?;
        self.nios_config_write(f(data))
    }
    /// Reads the expansion GPIO data register.
    pub fn nios_expansion_gpio_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::Exp, u32::MAX)
    }
    /// Writes the expansion GPIO data register with masking.
    ///
    /// Bits set in `mask` are updated to the corresponding values in `val`.
    pub fn nios_expansion_gpio_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::Exp, mask, val)
    }
    /// Reads the expansion GPIO direction register.
    pub fn nios_expansion_gpio_dir_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::ExpDir, u32::MAX)
    }
    /// Writes the expansion GPIO direction register with masking.
    ///
    /// Bits set in `mask` are updated to the corresponding values in `val`.
    pub fn nios_expansion_gpio_dir_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::ExpDir, mask, val)
    }
    /// Reads the FPGA version as a `SemanticVersion`.
    pub fn nios_get_fpga_version(&mut self) -> Result<SemanticVersion> {
        let regval = self.nios_read::<u8, u32>(NiosPkt8x32Target::Version, 0)?;
        log::trace!("Read FPGA version word: {regval:#010x}");
        // The FPGA builds this word as (major | minor << 8 | patch << 16), see
        // hdl/.../bladeRF_nios/src/fpga_version.h. The NIOS packet transmits it
        // little-endian, so `regval` (decoded via `from_le_bytes`) holds the
        // same layout: major in bits 0-7, minor in bits 8-15, patch in 16-31.
        let version = SemanticVersion::new(
            (regval & 0xff) as u16,
            ((regval >> 8) & 0xff) as u16,
            ((regval >> 16) & 0xffff) as u16,
        );
        Ok(version)
    }
    /// Reads the IQ gain correction coefficient for the given channel.
    pub fn nios_get_iq_gain_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }
    /// Reads the IQ phase correction coefficient for the given channel.
    pub fn nios_get_iq_phase_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }
    /// Sets the IQ gain correction coefficient for the given channel.
    pub fn nios_set_iq_gain_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
    /// Sets the IQ phase correction coefficient for the given channel.
    pub fn nios_set_iq_phase_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
    /// Returns the current USB alternate setting.
    ///
    /// Maps the raw integer to a `UsbAltSetting` variant; falls back
    /// to `Null` with a warning log if the value is unrecognized.
    pub fn get_alt_setting(&self) -> UsbAltSetting {
        let raw = self.transport.interface().get_alt_setting();
        UsbAltSetting::try_from(raw).unwrap_or_else(|_| {
            log::warn!("unknown USB alt setting {raw:#x}, treating as Null");
            UsbAltSetting::Null
        })
    }
    /// Issues an LMS6002D retune command.
    ///
    /// Encodes and submits a retune packet with the given synthesizer
    /// parameters. Returns the retune duration on success. Returns
    /// `Error::TuningFailed` for immediate retune failures or
    /// `Error::RetuneQueueFull` for scheduled retune queue overflow.
    #[allow(clippy::too_many_arguments)]
    pub fn nios_retune(
        &mut self,
        channel: Channel,
        timestamp: crate::bladerf1::protocol::RetuneTimestamp,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<crate::bladerf1::protocol::RetuneResult> {
        if timestamp == crate::bladerf1::protocol::RetuneTimestamp::Now {
            log::trace!("Clearing Retune Queue");
        }
        let out_buf = self.transport.out_buffer()?;
        nios_encode_retune(
            out_buf, channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        )?;
        let response = self.transport.submit(None)?;
        let response_pkt = nios_decode_retune(response)?;
        if !response_pkt.is_success() {
            let is_immediate = response_pkt.duration()
                == u64::from(crate::bladerf1::protocol::RetuneTimestamp::Now);
            return if is_immediate {
                Err(Error::TuningFailed)
            } else {
                Err(Error::RetuneQueueFull)
            };
        }
        Ok(crate::bladerf1::protocol::RetuneResult::new(
            response_pkt.duration(),
        ))
    }
    /// Writes a value to the ADF4351 synthesizer (XB-200 expansion board).
    pub fn nios_xb200_synth_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Adf4_351, 0, value)
    }

    /// Reads the hardware timestamp counter for the given channel.
    pub fn nios_get_timestamp(&mut self, channel: Channel) -> Result<u64> {
        let addr = match channel {
            Channel::Rx => NiosPkt8x64TimestampAddr::Rx,
            Channel::Tx => NiosPkt8x64TimestampAddr::Tx,
        };
        self.nios_read::<u8, u64>(NiosPkt8x64Target::Timestamp, addr.into())
    }
}

/// Core USB interface commands available on types that wrap an interface.
///
/// Implemented for `NiosCore` to delegate to the underlying transport.
impl UsbInterfaceCommands for NiosCore {
    /// Issues a vendor command and returns a 32-bit integer response.
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> Result<u32> {
        self.transport.usb_vendor_cmd_int(cmd)
    }
    /// Issues a vendor command with a `wValue` parameter and returns a 32-bit integer response.
    fn usb_vendor_cmd_int_w_value(&self, cmd: VendorRequest, wvalue: u16) -> Result<u32> {
        self.transport.usb_vendor_cmd_int_w_value(cmd, wvalue)
    }
    /// Issues a vendor command with a `wIndex` parameter and returns a 32-bit integer response.
    fn usb_vendor_cmd_int_w_index(&self, cmd: VendorRequest, windex: u16) -> Result<u32> {
        self.transport.usb_vendor_cmd_int_w_index(cmd, windex)
    }
    /// Issues an output vendor command with a `wIndex` parameter and data payload.
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        windex: u16,
        data: &[u8],
    ) -> Result<()> {
        self.transport.usb_vendor_cmd_out_w_index(cmd, windex, data)
    }
    /// Issues an input vendor command with a `wIndex` parameter and fills `buf` with the response.
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        windex: u16,
        buf: &mut [u8],
    ) -> Result<()> {
        self.transport
            .usb_vendor_cmd_in_w_index_data(cmd, windex, buf)
    }
    /// Switches the USB interface to the specified alternate setting.
    fn usb_change_setting(&mut self, setting: UsbAltSetting) -> Result<()> {
        self.transport.usb_change_setting(setting)
    }
}

/// BladeRF1-specific USB interface commands.
///
/// Implemented for `NiosCore` to delegate to the underlying transport.
impl BladeRf1UsbInterfaceCommands for NiosCore {
    /// Enables or disables the USB streaming module for the given channel.
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        self.transport.usb_enable_module(channel, enable)
    }
    /// Sets the firmware loopback mode and cycles the USB alt setting.
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.transport.usb_set_firmware_loopback(enable)
    }
    /// Queries whether firmware loopback is currently enabled.
    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        self.transport.usb_get_firmware_loopback()
    }
    /// Resets the FX3 USB controller.
    fn usb_device_reset(&self) -> Result<()> {
        self.transport.usb_device_reset()
    }
    /// Returns `true` if the firmware has reported readiness.
    fn usb_is_firmware_ready(&self) -> Result<bool> {
        self.transport.usb_is_firmware_ready()
    }
    /// Returns `true` if the FPGA has finished configuration.
    fn usb_is_fpga_configured(&self) -> Result<bool> {
        self.transport.usb_is_fpga_configured()
    }
    /// Signals the firmware to begin FPGA programming.
    fn usb_begin_fpga_prog(&self) -> Result<()> {
        self.transport.usb_begin_fpga_prog()
    }
    /// Sends a bulk OUT transfer to the specified endpoint.
    fn usb_bulk_out(&self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()> {
        self.transport.usb_bulk_out(endpoint, data, timeout)
    }
}

impl NiosCore {
    /// Writes all six AGC DC correction coefficients to the device.
    ///
    /// Programs the DC offset correction values for I and Q channels
    /// at the max, mid, and min gain settings via the AgcCorr NIOS target.
    pub fn nios_set_agc_dc_correction(
        &mut self,
        corr: &crate::bladerf1::hardware::lms6002d::dc_calibration::AgcDcCorrection,
    ) -> Result<()> {
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcQMax.into(),
            corr.max.q as u16,
        )?;
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcIMax.into(),
            corr.max.i as u16,
        )?;
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcQMid.into(),
            corr.mid.q as u16,
        )?;
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcIMid.into(),
            corr.mid.i as u16,
        )?;
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcQMin.into(),
            corr.min.q as u16,
        )?;
        self.nios_write::<u8, u16>(
            NiosPkt8x16Target::AgcCorr,
            NiosPkt8x16AddrAgcCorr::DcIMin.into(),
            corr.min.i as u16,
        )
    }
}
