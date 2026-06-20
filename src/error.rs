use crate::protocol::nios::NiosPacketError;

/// Result type alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for all operations in this crate.
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

    /// The device is not in the required state (e.g. not initialized).
    #[error("board state error: {0}")]
    BoardState(&'static str),

    /// A USB control transfer returned fewer bytes than expected.
    #[error("USB control response too short: expected {expected} bytes, got {actual}")]
    UsbControlResponseTooShort { expected: usize, actual: usize },

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
        byte_offset: usize,
        expected: u8,
        actual: u8,
    },

    /// The stream has already been closed or was never opened.
    #[error("stream already closed")]
    StreamClosed,

    /// Cannot switch USB alt setting while streams are active.
    #[error("cannot switch mode while streams are active")]
    StreamsActive,
}
