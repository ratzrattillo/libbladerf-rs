use super::{BladeRf1, ConfigSession, FlashSession, RfLinkSession};
use std::fmt;

impl fmt::Debug for BladeRf1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BladeRf1")
            .field("connection", &self.nios)
            .field(
                "rx_calibration_entries",
                &self.dc_rx_table.as_ref().map(|table| table.entries().len()),
            )
            .field(
                "tx_calibration_entries",
                &self.dc_tx_table.as_ref().map(|table| table.entries().len()),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RfLinkSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RfLinkSession")
            .field("connection", &self.nios)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for FlashSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlashSession")
            .field("connection", &self.nios)
            .field("geometry", &self.flash_meta)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ConfigSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigSession")
            .field("connection", &self.nios)
            .finish_non_exhaustive()
    }
}
