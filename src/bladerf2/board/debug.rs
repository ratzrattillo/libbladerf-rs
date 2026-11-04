use super::{BladeRf2, FlashSession, RfLinkSession};
use std::fmt;

impl fmt::Debug for BladeRf2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BladeRf2")
            .field("connection", &self.nios)
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
