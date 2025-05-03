#[cfg(test)]
mod tests {
    use bladerf_globals::BLADERF_MODULE_RX;
    use bladerf_nios::NiosPktMagic;
    use bladerf_nios::packet_retune2::NiosPktRetune2Request;

    #[test]
    fn packet_retune2() {
        let module: u8 = BLADERF_MODULE_RX;
        let timestamp: u64 = u64::MAX;
        let nios_profile: u16 = 0xffff;
        let rffe_profile: u8 = 0xff;
        let port: u8 = 0xff;
        let spdt: u8 = 0xff;

        let pkt =
            NiosPktRetune2Request::new(module, timestamp, nios_profile, rffe_profile, port, spdt);

        assert_eq!(pkt.magic(), NiosPktMagic::Retune2 as u8);
        assert_eq!(pkt.timestamp(), timestamp);
        assert_eq!(pkt.nios_profile(), nios_profile);
        assert_eq!(pkt.rffe_profile(), rffe_profile);
        assert_eq!(pkt.port(), port);
        assert_eq!(pkt.spdt(), spdt);
    }
}
