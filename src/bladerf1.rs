//! BladeRF1 driver module.
//!
//! Re-exports the primary API surface for the BladeRF1 (x40/x115) SDR,
//! including the [`BladeRf1`] device handle, session types, streaming
//! builders, hardware abstraction types, and DC calibration table management.

pub mod board;
pub mod calibration;
pub mod hardware;
pub mod protocol;
pub use crate::nios_client::NiosCore;
pub use crate::usb::BladeRf1UsbInterfaceCommands;
pub use board::QuickTune;
pub use board::rf_port::RfPort;
pub use board::xb::ExpansionBoard;
#[cfg(feature = "xb200")]
pub use board::xb::xb200::{Xb200Filter, Xb200Path};
pub use board::{BLADERF1_USB_PID, BLADERF1_USB_VID};
pub use board::{BladeRf1, ConfigSession, FlashSession, RfLinkSession, RxStream, TxStream};
pub use board::{
    Correction, FpgaSource, GainMode, METADATA_HEADER_SIZE, MetadataHeader, RxMux, RxStreamBuilder,
    SampleFormat, TuningMode, TxStreamBuilder,
};
pub use calibration::{DcCalEntry, DcCalTable};
pub use hardware::lms6002d::dc_calibration::{AgcDcCorrection, DcPair};
pub use hardware::lms6002d::gain::GainDb;
pub use hardware::lms6002d::{Band, LpfMode, Tune};
pub use hardware::si5338::{RationalRate, SmbMode};
pub use protocol::RetuneResult;
