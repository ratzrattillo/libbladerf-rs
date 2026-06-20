//! RF port selection for BladeRF1.
//!
//! Selects the active RF front-end path by choosing which LNA (RX) or
//! PA (TX) on the LMS6002D to route the signal through. Each port maps
//! to a specific LNA or PA with different frequency coverage, noise
//! figure, and output power characteristics.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::lms6002d::gain::{LmsLowNoiseAmplifier, LmsPowerAmplifier};
use crate::channel::Channel;
use crate::error::{Error, Result};

/// RF front-end port selection for BladeRF1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfPort {
    /// No amplifier or LNA selected.
    None,
    /// LNA 1 — RX low-noise amplifier with general-purpose frequency coverage.
    Lna1,
    /// LNA 2 — RX low-noise amplifier optimized for VHF/L-band frequencies.
    Lna2,
    /// LNA 3 — RX low-noise amplifier optimized for UHF/microwave frequencies.
    Lna3,
    /// PA 1 — TX power amplifier.
    Pa1,
    /// PA 2 — TX power amplifier with higher output.
    Pa2,
    /// Auxiliary TX output port.
    Aux,
}

impl RfPort {
    /// Valid RF ports for the RX channel.
    pub const RX_PORTS: [RfPort; 4] = [RfPort::None, RfPort::Lna1, RfPort::Lna2, RfPort::Lna3];
    /// Valid RF ports for the TX channel.
    pub const TX_PORTS: [RfPort; 4] = [RfPort::Aux, RfPort::Pa1, RfPort::Pa2, RfPort::None];

    /// Returns true if this port is valid for the given channel.
    pub fn is_valid_for(self, channel: Channel) -> bool {
        match channel {
            Channel::Rx => matches!(
                self,
                RfPort::None | RfPort::Lna1 | RfPort::Lna2 | RfPort::Lna3
            ),
            Channel::Tx => matches!(self, RfPort::None | RfPort::Pa1 | RfPort::Pa2 | RfPort::Aux),
        }
    }
}

/// Maps an LMS6002D low-noise amplifier selection to the corresponding `RfPort`.
impl From<LmsLowNoiseAmplifier> for RfPort {
    fn from(lna: LmsLowNoiseAmplifier) -> RfPort {
        match lna {
            LmsLowNoiseAmplifier::LnaNone => RfPort::None,
            LmsLowNoiseAmplifier::Lna1 => RfPort::Lna1,
            LmsLowNoiseAmplifier::Lna2 => RfPort::Lna2,
            LmsLowNoiseAmplifier::Lna3 => RfPort::Lna3,
        }
    }
}

/// Maps an LMS6002D power amplifier selection to the corresponding `RfPort`.
impl From<LmsPowerAmplifier> for RfPort {
    fn from(pa: LmsPowerAmplifier) -> RfPort {
        match pa {
            LmsPowerAmplifier::PaNone => RfPort::None,
            LmsPowerAmplifier::Pa1 => RfPort::Pa1,
            LmsPowerAmplifier::Pa2 => RfPort::Pa2,
            LmsPowerAmplifier::PaAux => RfPort::Aux,
        }
    }
}

/// Converts an `RfPort` to an LMS6002D LNA selection.
///
/// Fails with `Error::Argument` if the port is a TX port (PA or Aux).
impl TryFrom<RfPort> for LmsLowNoiseAmplifier {
    type Error = Error;
    fn try_from(port: RfPort) -> Result<Self> {
        match port {
            RfPort::None => Ok(LmsLowNoiseAmplifier::LnaNone),
            RfPort::Lna1 => Ok(LmsLowNoiseAmplifier::Lna1),
            RfPort::Lna2 => Ok(LmsLowNoiseAmplifier::Lna2),
            RfPort::Lna3 => Ok(LmsLowNoiseAmplifier::Lna3),
            _ => Err(Error::Argument("RX port does not map to LNA".into())),
        }
    }
}

/// Converts an `RfPort` to an LMS6002D PA selection.
///
/// Fails with `Error::Argument` if the port is an RX port (LNA).
impl TryFrom<RfPort> for LmsPowerAmplifier {
    type Error = Error;
    fn try_from(port: RfPort) -> Result<Self> {
        match port {
            RfPort::None => Ok(LmsPowerAmplifier::PaNone),
            RfPort::Pa1 => Ok(LmsPowerAmplifier::Pa1),
            RfPort::Pa2 => Ok(LmsPowerAmplifier::Pa2),
            RfPort::Aux => Ok(LmsPowerAmplifier::PaAux),
            _ => Err(Error::Argument("TX port does not map to PA".into())),
        }
    }
}

/// Parses an `RfPort` from a lowercase string name.
///
/// Returns `Error::Argument` for unrecognized port names.
impl TryFrom<&str> for RfPort {
    type Error = Error;
    fn try_from(name: &str) -> Result<Self> {
        match name.to_lowercase().as_str() {
            "none" => Ok(RfPort::None),
            "lna1" => Ok(RfPort::Lna1),
            "lna2" => Ok(RfPort::Lna2),
            "lna3" => Ok(RfPort::Lna3),
            "pa1" => Ok(RfPort::Pa1),
            "pa2" => Ok(RfPort::Pa2),
            "aux" => Ok(RfPort::Aux),
            _ => Err(Error::Argument("unknown RF port".into())),
        }
    }
}

/// Converts an `RfPort` to its lowercase string representation.
impl From<RfPort> for &'static str {
    fn from(port: RfPort) -> Self {
        match port {
            RfPort::None => "none",
            RfPort::Lna1 => "lna1",
            RfPort::Lna2 => "lna2",
            RfPort::Lna3 => "lna3",
            RfPort::Pa1 => "pa1",
            RfPort::Pa2 => "pa2",
            RfPort::Aux => "aux",
        }
    }
}

impl RfLinkSession<'_> {
    /// Sets the active RF port for the given channel.
    ///
    /// On RX, the port is converted to an LNA selection. On TX, it is
    /// converted to a PA selection. Returns `Error::Argument` if the port
    /// is not valid for the channel.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rf_port(&mut self, channel: Channel, port: RfPort) -> Result<()> {
        self.require_initialized()?;
        if !port.is_valid_for(channel) {
            return Err(Error::Argument("RF port not valid for channel".into()));
        }
        match channel {
            Channel::Rx => {
                let lna = LmsLowNoiseAmplifier::try_from(port)?;
                self.lms().select_lna(lna)
            }
            Channel::Tx => {
                let pa = LmsPowerAmplifier::try_from(port)?;
                self.lms().select_pa(pa)
            }
        }
    }

    /// Returns the current RF port for the given channel.
    ///
    /// Reads the active LNA selection on RX or PA selection on TX and
    /// converts it to the corresponding `RfPort`.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_rf_port(&mut self, channel: Channel) -> Result<RfPort> {
        self.require_initialized()?;
        match channel {
            Channel::Rx => self.lms().get_lna().map(RfPort::from),
            Channel::Tx => self.lms().get_pa().map(RfPort::from),
        }
    }

    /// Returns the list of valid RF ports for the given channel.
    pub fn get_rf_ports(channel: Channel) -> &'static [RfPort] {
        match channel {
            Channel::Rx => &RfPort::RX_PORTS,
            Channel::Tx => &RfPort::TX_PORTS,
        }
    }
}
