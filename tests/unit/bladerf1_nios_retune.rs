use libbladerf_rs::Channel;
use libbladerf_rs::Error;
use libbladerf_rs::bladerf1::protocol::NiosPktRetuneRequest;
use libbladerf_rs::bladerf1::protocol::{RetuneResult, RetuneTimestamp, TimestampTicks};
use libbladerf_rs::bladerf1::{Band, Tune};

#[test]
fn response_measurements_follow_request_and_validity_flags() {
    let mut response = [0u8; 16];
    response[0] = 0x54;
    response[1..9].copy_from_slice(&1234u64.to_le_bytes());
    response[9] = 31;
    response[10] = 3;
    assert_eq!(
        RetuneResult::decode(RetuneTimestamp::Now, &response).unwrap(),
        RetuneResult::Immediate {
            duration: TimestampTicks(1234),
            vcocap: 31
        }
    );
    response[10] = 2;
    assert!(RetuneResult::decode(RetuneTimestamp::Now, &response).is_err());
    assert_eq!(
        RetuneResult::decode(RetuneTimestamp::Scheduled(42), &response)
            .unwrap()
            .duration(),
        None
    );
    assert_eq!(
        RetuneResult::decode(RetuneTimestamp::ClearQueue, &response).unwrap(),
        RetuneResult::QueueCleared
    );
    response[10] = 0;
    assert!(matches!(
        RetuneResult::decode(RetuneTimestamp::Now, &response),
        Err(Error::TuningFailed)
    ));
    assert!(matches!(
        RetuneResult::decode(RetuneTimestamp::Scheduled(42), &response),
        Err(Error::RetuneQueueFull)
    ));
    response[0] = 0;
    assert!(RetuneResult::decode(RetuneTimestamp::Now, &response).is_err());
}

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
    let mut pkt = NiosPktRetuneRequest::new(&mut buf).expect("valid packet");
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
