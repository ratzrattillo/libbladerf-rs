use crate::protocol::nios::NiosPacketError;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("USB error")]
    Nusb(#[from] nusb::Error),
    #[error("USB transfer error")]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error("{0}")]
    Argument(&'static str),
    #[error("invalid state or value")]
    Invalid,
    #[error("device not found")]
    NotFound,
    #[error("operation timed out")]
    Timeout,
    #[error("NIOS packet error: {0}")]
    NiosPacket(#[from] NiosPacketError),
    #[error("endpoint busy")]
    EndpointBusy,
    #[error("FPGA tuning failed")]
    TuningFailed,
    #[error("FPGA retune queue is full")]
    RetuneQueueFull,
    #[error("NIOS write failed")]
    NiosWriteFailed,
}
