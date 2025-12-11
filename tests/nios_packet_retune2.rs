#[cfg(test)]
mod tests {
    use libbladerf_rs::Channel;
    use libbladerf_rs::nios::NiosPktMagic;
    use libbladerf_rs::nios::packet_retune2::NiosPktRetune2Request;

    #[test]
    fn packet_retune2_request() {
        let channel: Channel = Channel::Rx;
        let timestamp: u64 = u64::MAX;
        let nios_profile: u16 = 0xffff;
        let rffe_profile: u8 = 0xff;
        let port: u8 = 0xff;
        let spdt: u8 = 0xff;

        let pkt =
            NiosPktRetune2Request::new(channel, timestamp, nios_profile, rffe_profile, port, spdt);

        assert_eq!(pkt.magic(), NiosPktMagic::Retune2 as u8);
        assert_eq!(pkt.timestamp(), timestamp);
        assert_eq!(pkt.nios_profile(), nios_profile);
        assert_eq!(pkt.rffe_profile(), rffe_profile);
        assert_eq!(pkt.port(), port);
        assert_eq!(pkt.spdt(), spdt);
    }
}
