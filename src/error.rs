use crate::protocol::nios::NiosPacketError;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("USB error: {0}")]
    Nusb(#[from] nusb::Error),
    #[error("USB transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error("USB descriptor error: {0}")]
    Descriptor(#[from] nusb::GetDescriptorError),
    #[error("invalid argument: {0}")]
    Argument(String),
    #[error("device requires High speed or above")]
    UnsupportedSpeed,
    #[error("device not found")]
    NotFound,
    #[error("operation timed out")]
    Timeout,
    #[error("NIOS packet error: {0}")]
    NiosPacket(#[from] NiosPacketError),
    #[error("endpoint busy")]
    EndpointBusy(#[source] nusb::Error),
    #[error("endpoint not available")]
    EndpointNotAvailable,
    #[error("FPGA tuning failed")]
    TuningFailed,
    #[error("FPGA retune queue is full")]
    RetuneQueueFull,
    #[error("flash operation failed: status {0:#x}")]
    FlashError(u32),
    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),
    #[error("calibration failed: {0}")]
    CalibrationFailed(&'static str),
    #[error("hardware state error: {0}")]
    HardwareState(&'static str),
    #[error("USB control response too short: expected {expected} bytes, got {actual}")]
    UsbControlResponseTooShort { expected: usize, actual: usize },
    #[error("invalid sample rate: {0}")]
    InvalidSampleRate(&'static str),
    #[error("operation would block")]
    WouldBlock,
}
