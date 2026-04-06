use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::protocol::NiosPktRetuneRequest;
use libbladerf_rs::bladerf1::{Band, Tune};

#[test]
fn packet_retune_request() {
    let channel: Channel = Channel::Rx;
    let timestamp: u64 = u64::MAX;
    let nint: u16 = 0x01ff;
    let nfrac: u32 = 0x007fffff;
    let freqsel: u8 = 0x3f;
    let vcocap: u8 = 0x3f;
    let band = Band::Low;
    let tune = Tune::Normal;
    let xb_gpio: u8 = 0xff;

    let mut buf = [0u8; 16];
    let mut pkt = NiosPktRetuneRequest::new(&mut buf);
    pkt.prepare(
        channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
    )
    .expect("valid packet");

    assert_eq!(pkt.timestamp(), timestamp);
    assert_eq!(pkt.nint(), nint);
    assert_eq!(pkt.nfrac(), nfrac);
    assert_eq!(pkt.freqsel(), freqsel);
    assert_eq!(pkt.vcocap(), vcocap);
    assert_eq!(pkt.band(), Band::Low);
    assert_eq!(pkt.tune(), Tune::Normal);
    assert_eq!(pkt.xb_gpio(), xb_gpio);
}
