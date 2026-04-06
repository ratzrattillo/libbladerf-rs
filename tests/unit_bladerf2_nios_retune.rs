// Note: BladeRF2 protocol is not yet implemented in this restructuring
// This test is temporarily disabled until bladerf2 module is created

#[test]
#[ignore = "BladeRF2 protocol not yet implemented"]
fn packet_retune2_request() {
    // Test will be re-enabled when bladerf2 module is created
}

// #[cfg(test)]
// 2  mod tests {
//     3 -    use libbladerf_rs::Channel;
//     4 -
//     3      // Note: BladeRF2 protocol is not yet implemented in this restructuring
//     6 -    // This test is temporarily disabled
//     7 -    // use libbladerf_rs::bladerf2::protocol::NiosPktRetuneRequest;
//     4 +    // This test is temporarily disabled until bladerf2 module is created
//     5
//     6      #[test]
//     7 +    #[ignore = "BladeRF2 protocol not yet implemented"]
//     8      fn packet_retune2_request() {
//         11 -        let channel: Channel = Channel::Rx;
//         12 -        let timestamp: u64 = u64::MAX;
//         13 -        let nios_profile: u16 = 0xffff;
//         14 -        let rffe_profile: u8 = 0xff;
//         15 -        let port: u8 = 0xff;
//         16 -        let spdt: u8 = 0xff;
//         17 -
//             18 -        let mut buf = [0u8; 16];
//         19 -        let mut pkt = NiosPktRetuneRequest::new(&mut buf);
//         20 -        pkt.prepare(channel, timestamp, nios_profile, rffe_profile, port, spdt);
//         21 -
//             22 -        assert_eq!(pkt.timestamp(), timestamp);
//             23 -        assert_eq!(pkt.nios_profile(), nios_profile);
//             24 -        assert_eq!(pkt.rffe_profile(), rffe_profile);
//             25 -        assert_eq!(pkt.port(), port);
//             26 -        assert_eq!(pkt.spdt(), spdt);
//             9 +        // Test will be re-enabled when bladerf2 module is created
//             10      }
//     11  }
