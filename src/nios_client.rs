//! NiosCore — central register I/O hub.
//!
//! All SPI/I2C bridge commands and NIOS packet operations flow through
//! `NiosCore`. It wraps the USB transport and tracks the
//! `active_streams` counter to prevent USB alternate setting changes
//! while streaming endpoints are active.
//!
//! All register I/O methods return [`MaybeFuture`]: call `.wait()` to block
//! (native only) or `.await` from async code.

use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::bladerf1::protocol::{RetuneResult, nios_encode_retune};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use crate::protocol::nios::packet_generic::NiosNum;
use crate::protocol::nios::targets::NiosPkt8x16AddrAgcCorr;
use crate::protocol::nios::{
    NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target, NiosPkt8x64Target,
    NiosPkt8x64TimestampAddr, NiosPkt32x32Target, nios_decode_read, nios_decode_write,
    nios_encode_read, nios_encode_write, validate_response_address,
};
use crate::usb::UsbAltSetting;
use crate::usb::UsbTransport;
use crate::version::SemanticVersion;
use nusb::MaybeFuture;
pub(crate) mod streams;
use streams::StreamClaims;
mod restoration;
use restoration::{Restoration, RestorationLease};

/// Central NIOS register I/O hub.
///
/// Wraps a `UsbTransport` and provides typed methods for all NIOS
/// register access, including config GPIO, expansion GPIO, IQ/AGC
/// corrections, FPGA version queries, and retune commands. Tracks
/// the active stream count to guard USB alt setting transitions.
pub struct NiosCore {
    /// The underlying USB transport for device communication.
    transport: UsbTransport,
    pub(crate) streams: StreamClaims,
    restoration: Option<Restoration>,
}

impl std::fmt::Debug for NiosCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiosCore")
            .field("speed", &self.transport.speed())
            .field("alternate_setting", &self.transport.current_alt_setting())
            .field("open", &self.transport.is_open())
            .field("streams", &self.streams)
            .field("restoration_pending", &self.restoration.is_some())
            .finish_non_exhaustive()
    }
}
impl NiosCore {
    /// Creates a new `NiosCore` wrapping the given USB transport.
    pub fn new(transport: UsbTransport) -> Self {
        Self {
            transport,
            streams: StreamClaims::default(),
            restoration: None,
        }
    }
    /// Returns a shared reference to the underlying `UsbTransport`.
    pub fn transport(&self) -> &UsbTransport {
        &self.transport
    }
    /// Returns the serialized control transport.
    pub async fn control(&mut self) -> Result<&mut UsbTransport> {
        self.recover().await?;
        Ok(&mut self.transport)
    }

    pub fn device_reset(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        use crate::usb::BladeRf1UsbInterfaceCommands;
        Op::new(async move {
            self.streams.require_no_live_streams()?;
            self.transport.usb_device_reset().await
        })
    }

    pub fn shutdown(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.streams.require_idle()?;
            if self.transport.is_open() {
                self.recover().await?;
            }
            self.transport.shutdown().await
        })
    }
    /// Switches the USB alternate setting, releasing NIOS endpoints first.
    pub fn usb_change_setting(
        &mut self,
        setting: UsbAltSetting,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.recover().await?;
            self.transport.usb_change_setting(setting).await
        })
    }
    /// Sets the firmware loopback mode and cycles the USB alt setting.
    pub fn usb_set_firmware_loopback(
        &mut self,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.recover().await?;
            self.transport.usb_set_firmware_loopback(enable).await
        })
    }

    pub(crate) async fn recover(&mut self) -> Result<()> {
        if let Some(restoration) = self.restoration.as_mut()
            && restoration.restore(&mut self.transport).await?
        {
            self.restoration = None;
        }
        Ok(())
    }

    pub(crate) async fn save_lms_registers(
        &mut self,
        addresses: &[u8],
    ) -> Result<RestorationLease> {
        self.recover().await?;
        if self.restoration.is_some() {
            return Err(Error::Internal("nested temporary-register operation"));
        }
        let mut writes = std::collections::VecDeque::with_capacity(addresses.len());
        for &address in addresses {
            let value = self
                .nios_read::<u8, u8>(crate::protocol::nios::NiosPkt8x8Target::Lms6, address)
                .await?;
            writes.push_back((address, value));
        }
        let (restoration, lease) = Restoration::new(writes);
        self.restoration = Some(restoration);
        Ok(lease)
    }

    pub(crate) async fn finish_restoration<T>(
        &mut self,
        lease: RestorationLease,
        result: Result<T>,
    ) -> Result<T> {
        drop(lease);
        restoration::with_cleanup(result, self.recover().await)
    }
    /// Issues a generic NIOS register read.
    ///
    /// Encodes a read packet for the given `id` and `addr`, submits it
    /// via USB bulk transfer, and decodes the response data.
    pub fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8> + Send,
        addr: A,
    ) -> impl MaybeFuture<Output = Result<D>> {
        Op::new(async move {
            self.recover().await?;
            let id = id.into();
            let mut request = [0; 16];
            nios_encode_read::<A, D>(&mut request, id, addr)?;
            let response = self.transport.exchange(&request, None).await?;
            log::trace!("nios_read: response len = {} bytes", response.len());
            validate_response_address(&response, id, addr)?;
            nios_decode_read::<A, D>(&response)
        })
    }
    /// Issues a generic NIOS register write.
    ///
    /// Encodes a write packet for the given `id`, `addr`, and `data`,
    /// submits it via USB bulk transfer, and verifies the success status.
    pub fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8> + Send,
        addr: A,
        data: D,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.recover().await?;
            let id = id.into();
            let mut request = [0; 16];
            nios_encode_write::<A, D>(&mut request, id, addr, data)?;
            let response = self.transport.exchange(&request, None).await?;
            validate_response_address(&response, id, addr)?;
            nios_decode_write::<A, D>(&response)
        })
    }
    /// Reads the config GPIO register.
    pub fn nios_config_read(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        self.nios_read::<u8, u32>(NiosPkt8x32Target::Control, 0)
    }
    /// Writes the config GPIO register.
    pub fn nios_config_write(&mut self, value: u32) -> impl MaybeFuture<Output = Result<()>> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Control, 0, value)
    }
    /// Performs an atomic read-modify-write on the config GPIO register.
    ///
    /// Reads the current value, applies `f`, then writes the result back.
    pub fn nios_config_modify(
        &mut self,
        f: impl FnOnce(u32) -> u32 + Send,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let data = self.nios_config_read().await?;
            self.nios_config_write(f(data)).await
        })
    }
    /// Reads the expansion GPIO data register.
    pub fn nios_expansion_gpio_read(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::Exp, u32::MAX)
    }
    /// Writes the expansion GPIO data register with masking.
    ///
    /// Bits set in `mask` are updated to the corresponding values in `val`.
    pub fn nios_expansion_gpio_write(
        &mut self,
        mask: u32,
        val: u32,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::Exp, mask, val)
    }
    /// Reads the expansion GPIO direction register.
    pub fn nios_expansion_gpio_dir_read(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::ExpDir, u32::MAX)
    }
    /// Writes the expansion GPIO direction register with masking.
    ///
    /// Bits set in `mask` are updated to the corresponding values in `val`.
    pub fn nios_expansion_gpio_dir_write(
        &mut self,
        mask: u32,
        val: u32,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::ExpDir, mask, val)
    }
    /// Reads the FPGA version as a `SemanticVersion`.
    pub fn nios_get_fpga_version(&mut self) -> impl MaybeFuture<Output = Result<SemanticVersion>> {
        Op::new(async move {
            let regval = self
                .nios_read::<u8, u32>(NiosPkt8x32Target::Version, 0)
                .await?;
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
        })
    }
    /// Reads the IQ gain correction coefficient for the given channel.
    pub fn nios_get_iq_gain_correction(
        &mut self,
        ch: Channel,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())
            .map_ok(|v| v as i16)
    }
    /// Reads the IQ phase correction coefficient for the given channel.
    pub fn nios_get_iq_phase_correction(
        &mut self,
        ch: Channel,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())
            .map_ok(|v| v as i16)
    }
    /// Sets the IQ gain correction coefficient for the given channel.
    pub fn nios_set_iq_gain_correction(
        &mut self,
        ch: Channel,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
    /// Sets the IQ phase correction coefficient for the given channel.
    pub fn nios_set_iq_phase_correction(
        &mut self,
        ch: Channel,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
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
    pub fn get_alt_setting(&self) -> Option<UsbAltSetting> {
        self.transport.current_alt_setting()
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
    ) -> impl MaybeFuture<Output = Result<crate::bladerf1::protocol::RetuneResult>> {
        Op::new(async move {
            self.recover().await?;
            if timestamp == crate::bladerf1::protocol::RetuneTimestamp::Now {
                log::trace!("Clearing Retune Queue");
            }
            let mut request = [0; 16];
            nios_encode_retune(
                &mut request,
                channel,
                timestamp,
                nint,
                nfrac,
                freqsel,
                vcocap,
                band,
                tune,
                xb_gpio,
            )?;
            let response = self.transport.exchange(&request, None).await?;
            RetuneResult::decode(timestamp, &response)
        })
    }
    /// Writes a value to the ADF4351 synthesizer (XB-200 expansion board).
    pub fn nios_xb200_synth_write(&mut self, value: u32) -> impl MaybeFuture<Output = Result<()>> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Adf4_351, 0, value)
    }

    /// Reads the hardware timestamp counter for the given channel.
    pub fn nios_get_timestamp(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = Result<u64>> {
        let addr = match channel {
            Channel::Rx => NiosPkt8x64TimestampAddr::Rx,
            Channel::Tx => NiosPkt8x64TimestampAddr::Tx,
        };
        self.nios_read::<u8, u64>(NiosPkt8x64Target::Timestamp, addr.into())
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
    ) -> impl MaybeFuture<Output = Result<()>> {
        let writes = [
            (NiosPkt8x16AddrAgcCorr::DcQMax, corr.max.q),
            (NiosPkt8x16AddrAgcCorr::DcIMax, corr.max.i),
            (NiosPkt8x16AddrAgcCorr::DcQMid, corr.mid.q),
            (NiosPkt8x16AddrAgcCorr::DcIMid, corr.mid.i),
            (NiosPkt8x16AddrAgcCorr::DcQMin, corr.min.q),
            (NiosPkt8x16AddrAgcCorr::DcIMin, corr.min.i),
        ];
        Op::new(async move {
            for (addr, value) in writes {
                self.nios_write::<u8, u16>(NiosPkt8x16Target::AgcCorr, addr.into(), value as u16)
                    .await?;
            }
            Ok(())
        })
    }
}
