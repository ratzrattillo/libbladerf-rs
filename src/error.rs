//! Error type shared by every layer of the crate.
//!
//! [`Error`] carries the detail (USB error, transfer status, NIOS packet
//! problem, ...); [`ErrorKind`] is the stable coarse category applications
//! match on. Both enums are `#[non_exhaustive]`.

use crate::protocol::nios::NiosPacketError;
use std::fmt;

/// Result type alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Stable, coarse category of an [`Error`], returned by [`Error::kind`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// USB device, descriptor, endpoint or transfer failure.
    Usb,
    /// Malformed NIOS request or response packet.
    Protocol,
    /// An operation did not complete within its deadline.
    Timeout,
    /// A non-blocking operation has nothing to return yet.
    WouldBlock,
    /// No matching device was found.
    NotFound,
    /// A caller-supplied value is out of range or otherwise invalid.
    InvalidArgument,
    /// The feature is not available on this hardware or configuration.
    Unsupported,
    /// The operation is not valid in the current device or stream state.
    State,
    /// The device reported an unexpected or inconsistent hardware state.
    Hardware,
    /// A calibration routine did not converge.
    Calibration,
    /// Flash contents could not be verified or decoded.
    Flash,
    /// File or serialization failure outside the device.
    Io,
    /// An internal invariant was violated; please report this.
    Internal,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Usb => "usb",
            Self::Protocol => "protocol",
            Self::Timeout => "timeout",
            Self::WouldBlock => "would block",
            Self::NotFound => "not found",
            Self::InvalidArgument => "invalid argument",
            Self::Unsupported => "unsupported",
            Self::State => "invalid state",
            Self::Hardware => "hardware",
            Self::Calibration => "calibration",
            Self::Flash => "flash",
            Self::Io => "io",
            Self::Internal => "internal",
        };
        f.write_str(name)
    }
}

/// Error type for all operations in this crate.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An I/O error occurred (e.g. reading a DC calibration table from disk).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON deserialization error (e.g. malformed DC calibration table file).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A USB error from the nusb transport layer.
    #[error("USB error: {0}")]
    Nusb(#[from] nusb::Error),

    /// A USB transfer error (submission or completion failure).
    #[error("USB transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),

    /// A USB descriptor query error.
    #[error("USB descriptor error: {0}")]
    Descriptor(#[from] nusb::GetDescriptorError),

    /// An invalid argument was supplied (e.g. out-of-range frequency or gain).
    #[error("invalid argument: {0}")]
    Argument(String),

    /// The device does not support the required USB speed (must be High or above).
    #[error("device requires High speed or above")]
    UnsupportedSpeed,

    /// No BladeRF device was found matching the search criteria.
    #[error("device not found")]
    NotFound,

    /// A USB operation or device readiness check timed out.
    #[error("operation timed out")]
    Timeout,

    /// A NIOS packet encode/decode error (malformed request or response).
    #[error("NIOS packet error: {0}")]
    NiosPacket(#[from] NiosPacketError),

    /// A USB endpoint is already claimed or busy.
    #[error("endpoint busy")]
    EndpointBusy(#[source] nusb::Error),

    /// The requested USB endpoint is not available in the current alt setting.
    #[error("endpoint not available")]
    EndpointNotAvailable,

    /// FPGA-assisted tuning failed (VCOCAP did not converge).
    #[error("FPGA tuning failed")]
    TuningFailed,

    /// The FPGA retune queue is full; cannot schedule another retune.
    #[error("FPGA retune queue is full")]
    RetuneQueueFull,

    /// The requested feature is not supported on this hardware or configuration.
    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    /// A DC calibration sub-module failed to converge.
    #[error("calibration failed: {0}")]
    CalibrationFailed(&'static str),

    /// The device reported an unexpected or inconsistent hardware state
    /// (e.g. an invalid register value or a VCO that did not settle).
    #[error("unexpected hardware state: {0}")]
    BoardState(&'static str),

    /// The board has not been initialized; call `RfLinkSession::initialize`.
    #[error("device not initialized")]
    NotInitialized,

    /// A USB control transfer returned fewer bytes than expected.
    #[error("USB control response too short: expected {expected} bytes, got {actual}")]
    UsbControlResponseTooShort {
        /// Number of bytes the command is defined to return.
        expected: usize,
        /// Number of bytes actually received.
        actual: usize,
    },

    /// A fixed-size USB transfer completed with an unexpected length.
    #[error("USB transfer length mismatch: expected {expected} bytes, got {actual}")]
    UsbTransferLength {
        /// Required number of bytes.
        expected: usize,
        /// Completed number of bytes.
        actual: usize,
    },

    /// A firmware operation returned a failing status or unexpected acknowledgement.
    #[error("firmware request {request:#04x} returned {status:#010x}")]
    FirmwareStatus {
        /// USB vendor request number.
        request: u8,
        /// Raw firmware response.
        status: u32,
    },

    /// The requested sample rate is invalid for the current configuration.
    #[error("invalid sample rate: {0}")]
    InvalidSampleRate(&'static str),

    /// A non-blocking operation would block; no data is available yet.
    #[error("operation would block")]
    WouldBlock,

    /// Flash verification failed after a write operation.
    #[error(
        "flash verification failed at byte {byte_offset}: expected 0x{expected:02x}, got 0x{actual:02x}"
    )]
    FlashVerificationFailed {
        /// Offset of the first mismatching byte.
        byte_offset: usize,
        /// Byte value that was written.
        expected: u8,
        /// Byte value that was read back.
        actual: u8,
    },

    /// Data stored in flash (calibration region, BINKV fields) is malformed.
    #[error("malformed flash data: {0}")]
    FlashData(&'static str),

    /// The stream has already been closed or was never opened.
    #[error("stream already closed")]
    StreamClosed,

    /// The stream must be started before data can be transferred.
    #[error("stream not started")]
    StreamNotStarted,

    /// `start()` was called on a stream that is already running.
    #[error("stream already started")]
    StreamAlreadyStarted,

    /// Nothing can complete: the caller holds every buffer of the pool (RX)
    /// or no buffer is available and none is in flight (TX).
    #[error("no transfers in flight; recycle buffers first")]
    NoTransfersInFlight,

    /// Cannot switch USB alt setting while streams are active.
    #[error("cannot switch mode while streams are active")]
    StreamsActive,

    /// A stream already owns the endpoint for this direction.
    #[error("the {0:?} streaming endpoint is already claimed")]
    StreamClaimed(crate::Channel),

    /// A detached stream was used with another device instance.
    #[error("stream and session belong to different device instances")]
    WrongDevice,

    /// Duplex streams requested incompatible global FPGA formats.
    #[error("stream format conflicts with another direction's format")]
    IncompatibleStreamFormat,

    /// A stream lifecycle transition must finish before this operation.
    #[error("stream transition is incomplete; retry start, stop, or close")]
    StreamTransition,

    /// Protocol synchronization was lost; reset and reopen the device before further I/O.
    #[error("device recovery required; reset and reopen the connection")]
    RecoveryRequired,

    /// The trigger must be armed before it can be fired or disarmed.
    #[error("trigger not armed")]
    TriggerNotArmed,

    /// Only the trigger master may fire the trigger.
    #[error("only the trigger master can fire the trigger")]
    TriggerNotMaster,

    /// An internal invariant was violated (e.g. a static table is
    /// incomplete or a computed register value overflowed).
    #[error("internal error: {0}")]
    Internal(&'static str),
}

impl Error {
    /// Returns the coarse category of this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Io(_) | Self::Json(_) => ErrorKind::Io,
            Self::Nusb(_)
            | Self::Transfer(_)
            | Self::Descriptor(_)
            | Self::EndpointBusy(_)
            | Self::EndpointNotAvailable
            | Self::UnsupportedSpeed
            | Self::UsbTransferLength { .. }
            | Self::UsbControlResponseTooShort { .. } => ErrorKind::Usb,
            Self::NiosPacket(_) => ErrorKind::Protocol,
            Self::Timeout => ErrorKind::Timeout,
            Self::WouldBlock => ErrorKind::WouldBlock,
            Self::NotFound => ErrorKind::NotFound,
            Self::Argument(_) | Self::InvalidSampleRate(_) => ErrorKind::InvalidArgument,
            Self::Unsupported(_) => ErrorKind::Unsupported,
            Self::NotInitialized
            | Self::StreamClosed
            | Self::StreamNotStarted
            | Self::StreamAlreadyStarted
            | Self::NoTransfersInFlight
            | Self::StreamsActive
            | Self::StreamClaimed(_)
            | Self::WrongDevice
            | Self::IncompatibleStreamFormat
            | Self::StreamTransition
            | Self::RecoveryRequired
            | Self::TriggerNotArmed
            | Self::TriggerNotMaster => ErrorKind::State,
            Self::BoardState(_)
            | Self::TuningFailed
            | Self::RetuneQueueFull
            | Self::FirmwareStatus { .. } => ErrorKind::Hardware,
            Self::CalibrationFailed(_) => ErrorKind::Calibration,
            Self::FlashVerificationFailed { .. } | Self::FlashData(_) => ErrorKind::Flash,
            Self::Internal(_) => ErrorKind::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_stable() {
        assert_eq!(Error::Timeout.kind(), ErrorKind::Timeout);
        assert_eq!(Error::NotInitialized.kind(), ErrorKind::State);
        assert_eq!(Error::StreamNotStarted.kind(), ErrorKind::State);
        assert_eq!(Error::BoardState("x").kind(), ErrorKind::Hardware);
        assert_eq!(
            Error::Argument("x".into()).kind(),
            ErrorKind::InvalidArgument
        );
        assert_eq!(Error::FlashData("x").kind(), ErrorKind::Flash);
        assert_eq!(Error::Internal("x").kind(), ErrorKind::Internal);
        assert_eq!(ErrorKind::State.to_string(), "invalid state");
    }
}
