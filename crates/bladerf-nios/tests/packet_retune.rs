#[cfg(test)]
mod tests {
    use bladerf_globals::BLADERF_MODULE_RX;
    use bladerf_nios::NiosPktMagic;
    use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, Tune};

    #[test]
    fn packet_retune_request() {
        let module: u8 = BLADERF_MODULE_RX;
        let timestamp: u64 = u64::MAX;
        let nint: u16 = 0x01ff;
        let nfrac: u32 = 0x007fffff;
        let freqsel: u8 = 0x3f;
        let vcocap: u8 = 0x3f;
        let band = Band::Low;
        let tune = Tune::Normal;
        let xb_gpio: u8 = 0xff;

        let pkt = NiosPktRetuneRequest::new(
            module, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        );

        assert_eq!(pkt.magic(), NiosPktMagic::Retune as u8);
        assert_eq!(pkt.timestamp(), timestamp);
        assert_eq!(pkt.nint(), nint);
        assert_eq!(pkt.nfrac(), nfrac);
        assert_eq!(pkt.freqsel(), freqsel);
        assert_eq!(pkt.vcocap(), vcocap);
        assert_eq!(pkt.band(), Band::Low);
        assert_eq!(pkt.tune(), Tune::Normal);
        assert_eq!(pkt.xb_gpio(), xb_gpio);
    }
}
